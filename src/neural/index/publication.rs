//! One immutable base, one bounded overlay, and a content-bound location map.
//! redb remains authoritative. Only generation.json is replaced at commit.
use super::*;
use std::collections::{BTreeMap, BTreeSet};

pub(crate) struct Plan {
    coverage: cache::CoverageSnapshot,
    pub scope: GenerationScope,
    units: Locations,
    previous: Option<Generation>,
    delta_keys: Vec<String>,
    delta_changed: bool,
    mapping_revision: Option<u64>,
}

fn changed_keys(before: &Locations, after: &Locations) -> impl Iterator<Item = String> {
    before.diff(after).map(|change| match change {
        imbl::ordmap::DiffItem::Add(key, _) | imbl::ordmap::DiffItem::Remove(key, _) => key.clone(),
        imbl::ordmap::DiffItem::Update { new: (key, _), .. } => key.clone(),
    })
}
impl Plan {
    pub fn new(mapping: CurrentMapping, mut previous: Option<Generation>) -> Self {
        let mut delta_keys = BTreeSet::new();
        if let Some(generation) = &previous {
            let base = &generation.manifest;
            delta_keys.extend(
                base.delta
                    .iter()
                    .flat_map(|delta| delta.entries.iter().cloned()),
            );
            for key in changed_keys(&generation.labels, &mapping.units) {
                if base.base_entries.binary_search(&key).is_err() {
                    if mapping.units.contains_key(&key) {
                        delta_keys.insert(key);
                    } else {
                        delta_keys.remove(&key);
                    }
                }
            }
            let retired = base
                .base_count
                .saturating_sub(mapping.units.len().saturating_sub(delta_keys.len()));
            if delta_keys.len() > churn_limit(base.base_count)
                || retired > churn_limit(base.base_count)
                || delta_keys.len() as u64 * base.dimensions as u64 * 4 > MAX_DELTA_BYTES
            {
                previous = None;
                delta_keys.clear();
            }
        }
        let delta_keys: Vec<_> = delta_keys.into_iter().collect();
        let old_delta = previous
            .as_ref()
            .and_then(|g| g.manifest.delta.as_ref())
            .map(|d| d.entries.as_slice())
            .unwrap_or_default();
        let delta_changed = old_delta != delta_keys;
        Self {
            coverage: mapping.coverage,
            scope: mapping.scope,
            units: mapping.units,
            previous,
            delta_keys,
            delta_changed,
            mapping_revision: mapping.mapping_revision,
        }
    }
    pub fn needed(&self) -> BTreeMap<String, Vec<UnitLocation>> {
        if self.previous.is_none() {
            return self
                .units
                .iter()
                .map(|(key, units)| (key.clone(), (**units).clone()))
                .collect();
        }
        if !self.delta_changed {
            return BTreeMap::new();
        }
        self.delta_keys
            .iter()
            .map(|key| (key.clone(), (*self.units[key]).clone()))
            .collect()
    }
    pub fn stage(
        mut self,
        store: &Dir,
        identity: &Prepared,
        entries: &[GenerationEntry],
        control: &Control,
        built: &dyn Fn(),
    ) -> FResult<(Staged, cache::CoverageSnapshot, Publication)> {
        let full = self.previous.is_none();
        let loaded: BTreeSet<_> = entries.iter().map(|e| e.input_key.as_str()).collect();
        let missing: Vec<_> = if full {
            self.units
                .keys()
                .filter(|key| !loaded.contains(key.as_str()))
                .cloned()
                .collect()
        } else if self.delta_changed {
            self.delta_keys
                .iter()
                .filter(|key| !loaded.contains(key.as_str()))
                .cloned()
                .collect()
        } else {
            vec![]
        };
        for key in missing {
            if let Some(units) = self.units.remove(&key) {
                self.coverage.cached_current_units -= units.len() as u64;
            }
            self.scope.complete = false;
        }
        let mut artifacts = Vec::new();
        let previous_overlay = self
            .previous
            .as_ref()
            .and_then(|g| g.manifest.locations_delta.clone());
        let mut overlay = self
            .previous
            .as_ref()
            .map(|g| g.location_overlay.clone())
            .unwrap_or_default();
        if let Some(previous) = &self.previous {
            for key in changed_keys(&previous.labels, &self.units) {
                neural_fault!(LOCATION_COMPARE, Some(control), &key)?;
                let wanted = self.units.get(&key);
                if previous.location_base.get(&key) == wanted {
                    overlay.remove(&key);
                } else {
                    overlay.insert(key, wanted.cloned());
                }
            }
        }
        let mut overlay_bytes = if overlay.is_empty() {
            None
        } else {
            Some(locations::encode(&overlay)?)
        };
        let compact_locations = full
            || overlay.len() > locations::MAX_CHANGES
            || overlay_bytes
                .as_ref()
                .is_some_and(|bytes| bytes.len() as u64 > locations::MAX_DELTA_BYTES);
        let mut manifest = match self.previous {
            Some(previous) => previous.manifest,
            None => {
                let bytes = encode_index(identity, entries, control, built)?;
                let hash = crate::digest(&bytes);
                artifacts.push((format!("ann-{hash}.usearch"), bytes));
                let keys: Vec<_> = self.units.keys().cloned().collect();
                let members = serde_json::to_vec(&keys)?;
                let members_sha256 = crate::digest(&members);
                artifacts.push((format!("members-{members_sha256}.json"), members));
                GenerationManifest {
                    v: GENERATION_VERSION,
                    function_digest: identity.digest.clone(),
                    recipe_id: identity.recipe.clone(),
                    dimensions: identity.dimensions,
                    metric: METRIC.into(),
                    scalar: SCALAR_KIND.into(),
                    count: 0,
                    index_sha256: hash,
                    base_count: keys.len(),
                    base_entries: std::sync::Arc::new(keys),
                    members_sha256,
                    delta: None,
                    labels_sha256: String::new(),
                    locations_delta: None,
                    mapping_revision: self.mapping_revision,
                    coverage_stats: None,
                    source_revision: 0,
                    coverage: String::new(),
                }
            }
        };
        if !full && self.delta_changed {
            manifest.delta = if entries.is_empty() {
                None
            } else {
                let bytes = encode_index(identity, entries, control, built)?;
                within_publication_limit(bytes.len() as u64, MAX_DELTA_BYTES, "delta")?;
                let hash = crate::digest(&bytes);
                artifacts.push((format!("ann-{hash}.usearch"), bytes));
                Some(DeltaManifest {
                    sha256: hash,
                    entries: entries.iter().map(|e| e.input_key.clone()).collect(),
                })
            };
        }
        if compact_locations {
            let labels = LabelMap {
                labels: self
                    .units
                    .iter()
                    .enumerate()
                    .map(|(label, (key, units))| LabelEntry {
                        label: label as u64,
                        input_key: key.clone(),
                        units: (**units).clone(),
                    })
                    .collect(),
            };
            let bytes = serde_json::to_vec(&labels)?;
            manifest.labels_sha256 = crate::digest(&bytes);
            neural_fault!(LOCATION_WRITE, Some(control), "base")?;
            artifacts.push((manifest.labels_file(), bytes));
            overlay_bytes = None;
        }
        manifest.locations_delta = overlay_bytes.as_ref().map(|bytes| crate::digest(bytes));
        if let Some(bytes) = overlay_bytes {
            let file = manifest.location_delta_file().expect("overlay named");
            if manifest.locations_delta != previous_overlay {
                neural_fault!(LOCATION_WRITE, Some(control), "overlay")?;
                artifacts.push((file, bytes));
            }
        }
        manifest.count = self.units.len();
        manifest.source_revision = self.scope.source_revision;
        manifest.coverage = self.scope.coverage().into();
        manifest.mapping_revision = self.mapping_revision;
        self.coverage.searchable_current_units = self.coverage.cached_current_units;
        self.coverage.index_entries = manifest.count as u64;
        manifest.coverage_stats = Some(self.coverage.clone());
        let result = if full {
            Publication::Rebuilt(manifest.count)
        } else if self.delta_changed {
            Publication::Delta(manifest.count)
        } else {
            Publication::Mapped(manifest.count)
        };
        let mut staged = Staged::write(store, &manifest, artifacts, control)?;
        staged.snapshot = Some((manifest.mapping_id(), self.units));
        Ok((staged, self.coverage, result))
    }
}

pub(super) fn full(
    store: &Dir,
    identity: &Prepared,
    entries: &[GenerationEntry],
    scope: GenerationScope,
    control: &Control,
    built: &dyn Fn(),
) -> FResult<Staged> {
    // Validate all direct callers before cloning potentially oversized mappings.
    check_entries(entries, &identity.recipe)?;
    let units = entries
        .iter()
        .map(|entry| {
            (
                entry.input_key.clone(),
                std::sync::Arc::new(entry.units.clone()),
            )
        })
        .collect();
    let mapping = CurrentMapping {
        units,
        scope,
        coverage: cache::CoverageSnapshot {
            version: 1,
            source_revision: scope.source_revision,
            function_digest: identity.digest.clone(),
            recipe_id: identity.recipe.clone(),
            sources: 0,
            unpartitioned_sources: 0,
            eligible_units: entries.iter().map(|e| e.units.len() as u64).sum(),
            cached_current_units: entries.iter().map(|e| e.units.len() as u64).sum(),
            searchable_current_units: 0,
            index_entries: 0,
        },
        mapping_revision: None,
    };
    Plan::new(mapping, None)
        .stage(store, identity, entries, control, built)
        .map(|result| result.0)
}

fn check_entries(entries: &[GenerationEntry], recipe: &str) -> FResult<()> {
    let mut metadata = 64u64;
    let mut vectors = 0u64;
    let mut seen = BTreeSet::new();
    for entry in entries {
        // Publication must enforce the same location contract as a fresh load.
        // Refuse before staging can replace the last readable generation.
        if !is_hex64(&entry.input_key) || !locations::valid_units(&entry.units, recipe) {
            return Err(FoundryError::Semantic {
                code: "cache_corrupt",
                message: "invalid input key or locations in generation input".into(),
            });
        }
        if !seen.insert(&entry.input_key) {
            return Err(FoundryError::Semantic {
                code: "cache_corrupt",
                message: "duplicate input key in generation input".into(),
            });
        }
        metadata = metadata.saturating_add(entry.input_key.len() as u64 * 6 + 96);
        for unit in &entry.units {
            metadata = metadata.saturating_add(location_bytes(&unit.path, &unit.source_sha256));
        }
        vectors = vectors.saturating_add(entry.vector.len() as u64 * 4);
        within_publication_limit(metadata, MAX_META_BYTES, "mapping")?;
        within_publication_limit(vectors, MAX_VECTOR_BYTES, "vectors")?;
    }
    Ok(())
}

fn encode_index(
    identity: &Prepared,
    entries: &[GenerationEntry],
    control: &Control,
    built: &dyn Fn(),
) -> FResult<Vec<u8>> {
    use usearch::{Index, IndexOptions, MetricKind, ScalarKind};
    check_entries(entries, &identity.recipe)?;
    let options = IndexOptions {
        dimensions: identity.dimensions,
        metric: MetricKind::Cos,
        quantization: ScalarKind::F16,
        multi: false,
        ..IndexOptions::default()
    };
    let fail = |error: &dyn std::fmt::Display| FoundryError::Semantic {
        code: "index_unavailable",
        message: format!("usearch build: {error}"),
    };
    let index = Index::new(&options).map_err(|e| fail(&e))?;
    index.reserve(entries.len().max(1)).map_err(|e| fail(&e))?;
    let mut sorted: Vec<_> = entries.iter().collect();
    sorted.sort_by(|a, b| a.input_key.cmp(&b.input_key));
    for (ordinal, entry) in sorted.iter().enumerate() {
        if ordinal.is_multiple_of(256) {
            control.check()?;
        }
        if entry.vector.len() != identity.dimensions || entry.vector.iter().any(|v| !v.is_finite())
        {
            return Err(FoundryError::Semantic {
                code: "provider_malformed",
                message: format!("invalid vector for entry {ordinal}"),
            });
        }
        index
            .add(ordinal as u64, entry.vector.as_slice())
            .map_err(|e| fail(&e))?;
    }
    built();
    let len = index.serialized_length();
    within_publication_limit(len as u64, MAX_INDEX_BYTES, "index")?;
    let mut bytes = vec![0; len];
    index.save_to_buffer(&mut bytes).map_err(|e| fail(&e))?;
    Ok(bytes)
}

/// The only mutable publication name is the manifest. An interrupted stage or
/// compaction leaves the previous manifest and all its files intact.
pub(crate) struct Staged {
    generation: Dir,
    build: Dir,
    build_name: String,
    artifacts: Vec<String>,
    count: usize,
    pub(crate) snapshot: Option<(String, Locations)>,
}

impl Staged {
    fn write(
        store: &Dir,
        manifest: &GenerationManifest,
        artifacts: Vec<(String, Vec<u8>)>,
        control: &Control,
    ) -> FResult<Self> {
        validate_membership(manifest).map_err(|error| FoundryError::Semantic {
            code: "index_unavailable",
            message: error.reason(),
        })?;
        let generation =
            open_generation_dir(store, &manifest.function_digest, true)?.ok_or_else(|| {
                FoundryError::RepairPathConflict("generation directory vanished".into())
            })?;
        neural_fault!(
            PUBLISH_AFTER_CHECK,
            Some(control),
            &manifest.function_digest
        )?;
        // Reclaim only this format's content-named artifacts, preserving the
        // current manifest even when it fails payload validation. One writer.
        let keep = match generation.open_file(MANIFEST_FILE) {
            Ok(file) => read_bounded(file, "manifest")
                .ok()
                .and_then(|bytes| serde_json::from_slice::<GenerationManifest>(&bytes).ok()),
            Err(_) => None,
        };
        let keep: BTreeSet<_> = keep
            .iter()
            .flat_map(|m| {
                m.metadata_files()
                    .into_iter()
                    .chain(std::iter::once(m.index_file()))
                    .chain(m.delta_file())
            })
            .collect();
        for (name, kind) in generation
            .entries_while(|| control.check().is_ok())
            .map_err(io_error("generation cleanup"))?
        {
            control.check()?;
            let Some(text) = name.to_str() else {
                continue;
            };
            let artifact = text
                .strip_prefix("ann-")
                .and_then(|s| s.strip_suffix(".usearch"))
                .or_else(|| {
                    text.strip_prefix("locations-")
                        .and_then(|s| s.strip_suffix(".json"))
                })
                .or_else(|| {
                    text.strip_prefix("members-")
                        .and_then(|s| s.strip_suffix(".json"))
                })
                .or_else(|| {
                    text.strip_prefix("location-delta-")
                        .and_then(|s| s.strip_suffix(".json"))
                })
                .is_some_and(is_hex64);
            let scratch = text
                .strip_prefix(".building-")
                .is_some_and(|s| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit()));
            let owned = (artifact && kind == crate::neural::anchor::Kind::File)
                || (scratch && kind == crate::neural::anchor::Kind::Dir);
            if owned && !keep.contains(text) {
                generation
                    .remove_tree(&name)
                    .map_err(io_error("generation cleanup"))?;
            }
        }
        let build_name = format!(".building-{}", std::process::id());
        generation
            .create_dir(&build_name)
            .map_err(io_error("build directory"))?;
        let build = generation
            .open_dir(&build_name)
            .map_err(io_error("build directory"))?
            .ok_or_else(|| FoundryError::RepairPathConflict("build directory vanished".into()))?;
        let staged = Self {
            generation,
            build,
            build_name,
            artifacts: artifacts.iter().map(|(name, _)| name.clone()).collect(),
            count: manifest.count,
            snapshot: None,
        };
        for (name, bytes) in artifacts {
            control.check()?;
            within_publication_limit(
                bytes.len() as u64,
                if name.ends_with(".json") {
                    MAX_META_BYTES
                } else {
                    MAX_INDEX_BYTES
                },
                "artifact",
            )?;
            staged
                .build
                .write_new(name, &bytes)
                .map_err(io_error("generation artifact"))?;
        }
        let bytes = serde_json::to_vec(manifest)?;
        within_publication_limit(bytes.len() as u64, MAX_META_BYTES, "manifest")?;
        staged
            .build
            .write_new(MANIFEST_FILE, &bytes)
            .map_err(io_error("manifest"))?;
        staged.build.sync_all().map_err(io_error("build sync"))?;
        Ok(staged)
    }

    pub(crate) fn directory_identity(&self) -> FResult<(u64, u64)> {
        Ok(self.generation.identity()?)
    }

    pub(crate) fn publish(self, control: &Control) -> FResult<usize> {
        control.check()?;
        for file in &self.artifacts {
            self.build
                .rename_into(file, &self.generation, file)
                .map_err(io_error("generation artifact"))?;
        }
        self.generation
            .sync_all()
            .map_err(io_error("generation sync"))?;
        neural_fault!(PUBLISH_BEFORE_MANIFEST, Some(control), "artifacts durable")?;
        self.build
            .rename_into(MANIFEST_FILE, &self.generation, MANIFEST_FILE)
            .map_err(io_error("generation manifest"))?;
        self.generation
            .sync_all()
            .map_err(io_error("manifest sync"))?;
        neural_fault!(PUBLISH_AFTER_MANIFEST, Some(control), "manifest durable")?;
        Ok(self.count)
    }
}
impl Drop for Staged {
    fn drop(&mut self) {
        let _ = self.generation.remove_tree(&self.build_name);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::neural::query::{DENSE_WINDOW, DenseIndex};
    use std::os::unix::fs::MetadataExt;

    fn identity() -> Prepared {
        Prepared {
            digest: "ab".repeat(32),
            recipe: "delta-test".into(),
            dimensions: 128,
        }
    }
    fn entries(n: usize) -> Vec<GenerationEntry> {
        (0..n)
            .map(|i| {
                let mut vector = vec![0.0; 128];
                vector[0] = 1.0;
                vector[1] = i as f32 / 100.0;
                GenerationEntry {
                    input_key: format!("{i:064x}"),
                    vector,
                    units: vec![UnitLocation {
                        kind: crate::neural::cache::CardKind::Address,
                        path: format!("src/{i}.rs"),
                        start: 0,
                        end: 10,
                        source_sha256: "cd".repeat(32),
                    }],
                }
            })
            .collect()
    }
    fn mapping(entries: &[GenerationEntry], revision: u64) -> CurrentMapping {
        let identity = identity();
        CurrentMapping {
            mapping_revision: None,
            units: entries
                .iter()
                .map(|e| (e.input_key.clone(), std::sync::Arc::new(e.units.clone())))
                .collect(),
            scope: GenerationScope {
                source_revision: revision,
                complete: true,
            },
            coverage: cache::CoverageSnapshot {
                version: 1,
                source_revision: revision,
                function_digest: identity.digest,
                recipe_id: identity.recipe,
                sources: entries.len() as u64,
                unpartitioned_sources: 0,
                eligible_units: entries.iter().map(|e| e.units.len() as u64).sum(),
                cached_current_units: entries.iter().map(|e| e.units.len() as u64).sum(),
                searchable_current_units: entries.iter().map(|e| e.units.len() as u64).sum(),
                index_entries: entries.len() as u64,
            },
        }
    }
    fn validate(store: &Dir) -> Generation {
        let id = identity();
        validate_generation(store, &id.digest, &id.recipe, id.dimensions).unwrap()
    }
    fn load(store: &Dir) -> ArcIndex {
        let id = identity();
        let (generation, bytes) = load_generation_with(
            store,
            &id.digest,
            &id.recipe,
            id.dimensions,
            &Control::unbounded(),
        )
        .unwrap();
        DenseIndex::from_buffers(generation, bytes).unwrap()
    }
    type ArcIndex = std::sync::Arc<DenseIndex>;
    fn publish(store: &Dir, entries: &[GenerationEntry], revision: u64) -> Publication {
        let prior = validate(store);
        let plan = Plan::new(mapping(entries, revision), Some(prior));
        let needed = plan.needed();
        let vectors: Vec<_> = entries
            .iter()
            .filter(|e| needed.contains_key(&e.input_key))
            .map(|e| GenerationEntry {
                input_key: e.input_key.clone(),
                vector: e.vector.clone(),
                units: e.units.clone(),
            })
            .collect();
        let (stage, _, result) = plan
            .stage(store, &identity(), &vectors, &Control::unbounded(), &|| {})
            .unwrap();
        stage.publish(&Control::unbounded()).unwrap();
        result
    }

    #[test]
    fn delta_and_location_publication_reuse_base_and_filter_retired_candidates_before_selection() {
        let tmp = tempfile::tempdir().unwrap();
        let store = Dir::open_path(tmp.path()).unwrap();
        let mut current = entries(400);
        full(
            &store,
            &identity(),
            &current,
            mapping(&current, 1).scope,
            &Control::unbounded(),
            &|| {},
        )
        .unwrap()
        .publish(&Control::unbounded())
        .unwrap();
        let old = validate(&store);
        let directory = generation_dir(tmp.path(), &identity().digest);
        let base = directory.join(old.manifest.index_file());
        let inode = std::fs::metadata(&base).unwrap().ino();
        let resident = load(&store);
        // Retire more vectors than the whole query window, all the nearest ones.
        current.drain(..80);
        let mut added = entries(401).pop().unwrap();
        added.vector[1] = 0.0;
        current.push(added);
        let plan = Plan::new(mapping(&current, 2), Some(old.clone()));
        assert_eq!(plan.needed().len(), 1, "only the one new input is decoded");
        assert_eq!(publish(&store, &current, 2), Publication::Delta(321));
        assert_eq!(std::fs::metadata(&base).unwrap().ino(), inode);
        let generation = validate(&store);
        assert_eq!(generation.manifest.base_entries.len(), 400);
        assert_eq!(generation.manifest.delta.as_ref().unwrap().entries.len(), 1);
        let mut query = vec![0.0; 128];
        query[0] = 1.0;
        let loaded = load(&store);
        let hits = loaded.search(&query, DENSE_WINDOW).unwrap();
        assert_eq!(
            hits.len(),
            DENSE_WINDOW,
            "retired neighbors do not consume the window"
        );
        assert_eq!(loaded.input_key(hits[0].label), format!("{:064x}", 400));
        let keys: BTreeSet<_> = hits.iter().map(|h| loaded.input_key(h.label)).collect();
        assert_eq!(
            keys.len(),
            hits.len(),
            "segments merge without duplicate inputs"
        );
        assert!(
            keys.iter()
                .all(|key| *key >= format!("{:064x}", 80).as_str())
        );
        assert_eq!(
            resident.search(&query, DENSE_WINDOW).unwrap().len(),
            DENSE_WINDOW,
            "an already loaded request keeps its generation"
        );
        let delta_path = directory.join(generation.manifest.delta_file().unwrap());
        let delta_inode = std::fs::metadata(&delta_path).unwrap().ino();
        current[0].units[0].path = "renamed.rs".into();
        let mut shared = current[0].units[0].clone();
        shared.path = "renamed_copy.rs".into();
        current[0].units.push(shared);
        let plan = Plan::new(mapping(&current, 3), Some(generation));
        assert!(
            plan.needed().is_empty(),
            "location publication decodes zero vectors"
        );
        let (stage, _, result) = plan
            .stage(&store, &identity(), &[], &Control::unbounded(), &|| {
                panic!("no ANN build for locations")
            })
            .unwrap();
        assert_eq!(result, Publication::Mapped(321));
        stage.publish(&Control::unbounded()).unwrap();
        assert_eq!(std::fs::metadata(&base).unwrap().ino(), inode);
        assert_eq!(std::fs::metadata(&delta_path).unwrap().ino(), delta_inode);
        assert_eq!(validate(&store).labels.values().next().unwrap().len(), 2);
        // A reverted base key reactivates its existing ordinal, with no delta work.
        let reverted = entries(1).pop().unwrap();
        current.push(reverted);
        assert_eq!(publish(&store, &current, 4), Publication::Mapped(322));
        assert_eq!(std::fs::metadata(&base).unwrap().ino(), inode);
    }

    #[test]
    fn failed_compaction_keeps_previous_generation_and_retry_reclaims_orphans() {
        use crate::fault::{self, Action};
        let tmp = tempfile::tempdir().unwrap();
        let store = Dir::open_path(tmp.path()).unwrap();
        let initial = entries(100);
        full(
            &store,
            &identity(),
            &initial,
            mapping(&initial, 1).scope,
            &Control::unbounded(),
            &|| {},
        )
        .unwrap()
        .publish(&Control::unbounded())
        .unwrap();
        let prior = validate(&store);
        let mut changed = entries(170);
        changed.drain(..70);
        let plan = Plan::new(mapping(&changed, 2), Some(prior.clone()));
        assert_eq!(
            plan.needed().len(),
            changed.len(),
            "churn threshold triggers compaction"
        );
        let (stage, _, outcome) = plan
            .stage(&store, &identity(), &changed, &Control::unbounded(), &|| {})
            .unwrap();
        assert_eq!(outcome, Publication::Rebuilt(100));
        fault::arm(
            crate::neural::fault_names::PUBLISH_BEFORE_MANIFEST,
            0,
            Action::Fail("disk failure".into()),
        );
        assert!(stage.publish(&Control::unbounded()).is_err());
        fault::disarm_all();
        assert_eq!(validate(&store).labels, prior.labels);
        assert_eq!(load(&store).search(&initial[0].vector, 5).unwrap().len(), 5);
        assert_eq!(publish(&store, &changed, 2), Publication::Rebuilt(100));
        // Deleting the last sources publishes a valid empty generation.
        assert_eq!(publish(&store, &[], 3), Publication::Rebuilt(0));
        assert!(
            load(&store)
                .search(&initial[0].vector, 5)
                .unwrap()
                .is_empty()
        );
        let directory = generation_dir(tmp.path(), &identity().digest);
        assert!(
            std::fs::read_dir(directory).unwrap().count() <= 7,
            "scratch is bounded across failures and compactions"
        );
    }

    #[test]
    fn location_overlay_rebases_at_its_bound_without_rebuilding_either_ann_segment() {
        use crate::fault::{self, Action};
        let tmp = tempfile::tempdir().unwrap();
        let store = Dir::open_path(tmp.path()).unwrap();
        let mut current = entries(locations::MAX_CHANGES + 2);
        full(
            &store,
            &identity(),
            &current,
            mapping(&current, 1).scope,
            &Control::unbounded(),
            &|| {},
        )
        .unwrap()
        .publish(&Control::unbounded())
        .unwrap();
        let initial = validate(&store);
        let directory = generation_dir(tmp.path(), &identity().digest);
        let base = directory.join(initial.manifest.index_file());
        let members = directory.join(initial.manifest.members_file());
        let inodes = (
            std::fs::metadata(&base).unwrap().ino(),
            std::fs::metadata(&members).unwrap().ino(),
        );
        current.push(entries(current.len() + 1).pop().unwrap());
        assert!(matches!(
            publish(&store, &current, 2),
            Publication::Delta(_)
        ));
        let delta = validate(&store).manifest.delta_file().unwrap();
        let delta_inode = std::fs::metadata(directory.join(&delta)).unwrap().ino();
        // One addition plus 4,095 shifted inputs fits; the next shift rebases
        // locations while retaining the independent ANN delta and membership.
        for entry in current.iter_mut().take(locations::MAX_CHANGES - 1) {
            entry.units[0].start += 1;
        }
        assert!(matches!(
            publish(&store, &current, 3),
            Publication::Mapped(_)
        ));
        let bounded = validate(&store);
        assert_eq!(bounded.location_overlay.len(), locations::MAX_CHANGES);
        assert_eq!(
            bounded.manifest.labels_sha256,
            initial.manifest.labels_sha256
        );
        let bytes = std::fs::read(directory.join(MANIFEST_FILE)).unwrap();
        let header: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert!(header.get("base_entries").is_none() && header.get("entries").is_none());
        assert!(
            bytes.len() < 2048,
            "manifest size does not track the base corpus"
        );
        current[locations::MAX_CHANGES - 1].units[0].start += 1;
        let plan = Plan::new(mapping(&current, 4), Some(bounded.clone()));
        assert!(plan.needed().is_empty());
        let (stage, _, _) = plan
            .stage(&store, &identity(), &[], &Control::unbounded(), &|| {
                panic!("location rebase must not build ANN")
            })
            .unwrap();
        fault::arm(
            crate::neural::fault_names::PUBLISH_BEFORE_MANIFEST,
            0,
            Action::Fail("rebase failed".into()),
        );
        assert!(stage.publish(&Control::unbounded()).is_err());
        fault::disarm_all();
        assert_eq!(validate(&store).labels, bounded.labels);
        assert!(matches!(
            publish(&store, &current, 4),
            Publication::Mapped(_)
        ));
        let compacted = validate(&store);
        assert!(compacted.manifest.locations_delta.is_none());
        assert_ne!(
            compacted.manifest.labels_sha256,
            bounded.manifest.labels_sha256
        );
        assert_eq!(compacted.labels, mapping(&current, 4).units);
        assert_eq!(compacted.manifest.delta_file(), Some(delta.clone()));
        assert_eq!(
            (
                std::fs::metadata(base).unwrap().ino(),
                std::fs::metadata(members).unwrap().ino()
            ),
            inodes
        );
        assert_eq!(
            std::fs::metadata(directory.join(delta)).unwrap().ino(),
            delta_inode
        );
        // A fresh edit is relative to the new location base, then reverts cleanly.
        current[0].units[0].start += 1;
        publish(&store, &current, 5);
        assert_eq!(validate(&store).location_overlay.len(), 1);
        current[0].units[0].start -= 1;
        publish(&store, &current, 6);
        assert!(validate(&store).manifest.locations_delta.is_none());
        assert!(std::fs::read_dir(directory).unwrap().count() <= 7);
    }

    #[test]
    fn metadata_validation_checks_bound_content_even_when_manifest_hashes_match() {
        let tmp = tempfile::tempdir().unwrap();
        let store = Dir::open_path(tmp.path()).unwrap();
        let current = entries(3);
        full(
            &store,
            &identity(),
            &current,
            mapping(&current, 1).scope,
            &Control::unbounded(),
            &|| {},
        )
        .unwrap()
        .publish(&Control::unbounded())
        .unwrap();
        let prior = validate(&store).manifest;
        let directory = generation_dir(tmp.path(), &identity().digest);
        for mutation in 0..7 {
            let mut manifest = prior.clone();
            let (bytes, is_members) = match mutation {
                0 => (
                    serde_json::to_vec(&vec![current[0].input_key.clone(); 3]).unwrap(),
                    true,
                ),
                1 => (
                    serde_json::to_vec(&vec![format!("{:064x}", 900); 1]).unwrap(),
                    true,
                ),
                _ => {
                    let change = serde_json::json!({"key": current[0].input_key, "units": null});
                    let value = match mutation {
                        2 => serde_json::json!([change, change]),
                        3 => {
                            serde_json::json!([{ "key": format!("{:064x}", 900), "units": current[0].units }])
                        }
                        4 => {
                            serde_json::json!([{ "key": current[0].input_key, "units": current[0].units }])
                        }
                        5 => serde_json::json!([{ "key": current[0].input_key, "units": [] }]),
                        _ => serde_json::Value::Array(vec![change; locations::MAX_CHANGES + 1]),
                    };
                    (serde_json::to_vec(&value).unwrap(), false)
                }
            };
            let hash = crate::digest(&bytes);
            let name = if is_members {
                manifest.members_sha256 = hash;
                manifest.members_file()
            } else {
                manifest.locations_delta = Some(hash);
                manifest.location_delta_file().unwrap()
            };
            std::fs::write(directory.join(name), bytes).unwrap();
            std::fs::write(
                directory.join(MANIFEST_FILE),
                serde_json::to_vec(&manifest).unwrap(),
            )
            .unwrap();
            assert!(
                validate_generation(&store, &identity().digest, &identity().recipe, 128).is_err(),
                "mutation {mutation}"
            );
        }
        std::fs::write(
            directory.join(MANIFEST_FILE),
            serde_json::to_vec(&prior).unwrap(),
        )
        .unwrap();
        validate(&store);
    }

    #[test]
    fn manifest_rejects_unbounded_duplicate_or_unbound_segment_membership() {
        let tmp = tempfile::tempdir().unwrap();
        let store = Dir::open_path(tmp.path()).unwrap();
        let initial = entries(100);
        full(
            &store,
            &identity(),
            &initial,
            mapping(&initial, 1).scope,
            &Control::unbounded(),
            &|| {},
        )
        .unwrap()
        .publish(&Control::unbounded())
        .unwrap();
        let prior = validate(&store).manifest;
        for mutation in 0..6 {
            let mut bad = prior.clone();
            match mutation {
                0 => bad.members_sha256 = "../outside".into(),
                1 => {
                    bad.delta = Some(DeltaManifest {
                        sha256: "ab".repeat(32),
                        entries: vec![bad.base_entries[0].clone()],
                    })
                }
                2 => bad.locations_delta = Some("invalid".into()),
                3 => bad.index_sha256 = "../outside".into(),
                4 => {
                    let entries: Vec<_> = (100..165).map(|i| format!("{i:064x}")).collect();
                    bad.delta = Some(DeltaManifest {
                        sha256: "ab".repeat(32),
                        entries,
                    });
                }
                _ => {
                    bad.base_count = usize::MAX;
                }
            }
            assert!(validate_membership(&bad).is_err());
        }
    }
}
