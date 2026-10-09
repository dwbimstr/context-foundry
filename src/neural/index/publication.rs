//! One immutable base, one bounded overlay, and a content-bound location map.
//! redb remains authoritative. Only generation.json is replaced at commit.
use super::*;
use std::collections::{BTreeMap, BTreeSet};

pub(crate) struct Plan {
    coverage: cache::CoverageSnapshot,
    pub scope: GenerationScope,
    units: BTreeMap<String, Vec<UnitLocation>>,
    previous: Option<GenerationManifest>,
    delta_keys: Vec<String>,
    delta_changed: bool,
}

impl Plan {
    pub fn new(mapping: CurrentMapping, mut previous: Option<GenerationManifest>) -> Self {
        let mut delta_keys = Vec::new();
        if let Some(base) = &previous {
            delta_keys = mapping
                .units
                .keys()
                .filter(|key| base.base_entries.binary_search(key).is_err())
                .cloned()
                .collect();
            let retired = base
                .base_entries
                .iter()
                .filter(|key| !mapping.units.contains_key(*key))
                .count();
            if delta_keys.len() > churn_limit(base.base_entries.len())
                || retired > churn_limit(base.base_entries.len())
                || delta_keys.len() as u64 * base.dimensions as u64 * 4 > MAX_DELTA_BYTES
            {
                previous = None;
                delta_keys.clear();
            }
        }
        let old_delta = previous
            .as_ref()
            .and_then(|base| base.delta.as_ref())
            .map(|delta| delta.entries.as_slice())
            .unwrap_or_default();
        let delta_changed = old_delta != delta_keys;
        Self {
            coverage: mapping.coverage,
            scope: mapping.scope,
            units: mapping.units,
            previous,
            delta_keys,
            delta_changed,
        }
    }

    /// Only a compaction reads the full vector cache. Mapping-only plans read none.
    pub fn needed(&self) -> BTreeMap<String, Vec<UnitLocation>> {
        if self.previous.is_none() {
            return self.units.clone();
        }
        if !self.delta_changed {
            return BTreeMap::new();
        }
        self.delta_keys
            .iter()
            .map(|key| (key.clone(), self.units[key].clone()))
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
        // Failed cache decodes cannot leave labels pointing at absent vectors.
        let loaded: BTreeSet<_> = entries
            .iter()
            .map(|entry| entry.input_key.as_str())
            .collect();
        if full {
            self.units.retain(|key, _| loaded.contains(key.as_str()));
        } else if self.delta_changed {
            self.units.retain(|key, _| {
                self.delta_keys.binary_search(key).is_err() || loaded.contains(key.as_str())
            });
        }
        let labels = labels(&self.units);
        let mut artifacts = Vec::new();
        let mut manifest = match self.previous {
            Some(base) => base,
            None => {
                let bytes = encode_index(identity, entries, control, built)?;
                let hash = crate::digest(&bytes);
                artifacts.push((format!("ann-{hash}.usearch"), bytes));
                GenerationManifest {
                    v: GENERATION_VERSION,
                    function_digest: identity.digest.clone(),
                    recipe_id: identity.recipe.clone(),
                    dimensions: identity.dimensions,
                    metric: METRIC.into(),
                    scalar: SCALAR_KIND.into(),
                    count: 0,
                    index_sha256: hash,
                    base_entries: self.units.keys().cloned().collect(),
                    delta: None,
                    labels_sha256: String::new(),
                    entries: Vec::new(),
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
                    entries: self
                        .units
                        .keys()
                        .filter(|key| loaded.contains(key.as_str()))
                        .cloned()
                        .collect(),
                })
            };
        }
        let labels_bytes = serde_json::to_vec(&labels)?;
        manifest.labels_sha256 = crate::digest(&labels_bytes);
        manifest.count = labels.labels.len();
        manifest.entries = self.units.keys().cloned().collect();
        manifest.source_revision = self.scope.source_revision;
        manifest.coverage = self.scope.coverage().into();
        let result = if full {
            Publication::Rebuilt(manifest.count)
        } else if self.delta_changed {
            Publication::Delta(manifest.count)
        } else {
            Publication::Mapped(manifest.count)
        };
        artifacts.push((manifest.labels_file(), labels_bytes));
        self.coverage.cached_current_units =
            self.units.values().map(|units| units.len() as u64).sum();
        self.coverage.searchable_current_units = self.coverage.cached_current_units;
        self.coverage.index_entries = manifest.count as u64;
        let staged = Staged::write(store, &manifest, artifacts, control)?;
        Ok((staged, self.coverage, result))
    }
}

fn labels(units: &BTreeMap<String, Vec<UnitLocation>>) -> LabelMap {
    LabelMap {
        labels: units
            .iter()
            .enumerate()
            .map(|(ordinal, (key, units))| LabelEntry {
                label: ordinal as u64,
                input_key: key.clone(),
                units: units.clone(),
            })
            .collect(),
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
    check_entries(entries)?;
    let units = entries
        .iter()
        .map(|entry| (entry.input_key.clone(), entry.units.clone()))
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
            eligible_units: 0,
            cached_current_units: 0,
            searchable_current_units: 0,
            index_entries: 0,
        },
    };
    Plan::new(mapping, None)
        .stage(store, identity, entries, control, built)
        .map(|result| result.0)
}

fn check_entries(entries: &[GenerationEntry]) -> FResult<()> {
    let mut metadata = 64u64;
    let mut vectors = 0u64;
    let mut seen = BTreeSet::new();
    for entry in entries {
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
    check_entries(entries)?;
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
            .flat_map(|m| [Some(m.index_file()), Some(m.labels_file()), m.delta_file()])
            .flatten()
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
            units: entries
                .iter()
                .map(|e| (e.input_key.clone(), e.units.clone()))
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
                eligible_units: entries.len() as u64,
                cached_current_units: entries.len() as u64,
                searchable_current_units: entries.len() as u64,
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
        let plan = Plan::new(mapping(entries, revision), Some(prior.manifest));
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
        let plan = Plan::new(mapping(&current, 2), Some(old.manifest.clone()));
        assert_eq!(plan.needed().len(), 1, "only the one new input is decoded");
        assert_eq!(publish(&store, &current, 2), Publication::Delta(321));
        assert_eq!(std::fs::metadata(&base).unwrap().ino(), inode);
        let generation = validate(&store);
        assert_eq!(generation.manifest.base_entries.len(), 400);
        assert_eq!(generation.manifest.delta.as_ref().unwrap().entries.len(), 1);
        let mut query = vec![0.0; 128];
        query[0] = 1.0;
        let hits = load(&store).search(&query, DENSE_WINDOW).unwrap();
        assert_eq!(
            hits.len(),
            DENSE_WINDOW,
            "retired neighbors do not consume the window"
        );
        assert_eq!(
            generation.labels.labels[hits[0].label].input_key,
            format!("{:064x}", 400)
        );
        let keys: BTreeSet<_> = hits
            .iter()
            .map(|h| &generation.labels.labels[h.label].input_key)
            .collect();
        assert_eq!(
            keys.len(),
            hits.len(),
            "segments merge without duplicate inputs"
        );
        assert!(keys.iter().all(|key| **key >= format!("{:064x}", 80)));
        assert_eq!(
            resident.search(&query, DENSE_WINDOW).unwrap().len(),
            DENSE_WINDOW,
            "an already loaded request keeps its generation"
        );
        let delta_path = directory.join(generation.manifest.delta_file().unwrap());
        let delta_inode = std::fs::metadata(&delta_path).unwrap().ino();
        current[0].units[0].path = "renamed.rs".into();
        let duplicate = current[0].units[0].clone();
        current[0].units.push(duplicate);
        let plan = Plan::new(mapping(&current, 3), Some(generation.manifest));
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
        assert_eq!(validate(&store).labels.labels[0].units.len(), 2);
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
        let plan = Plan::new(mapping(&changed, 2), Some(prior.manifest.clone()));
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
        assert_eq!(validate(&store).manifest.entries, prior.manifest.entries);
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
            std::fs::read_dir(directory).unwrap().count() <= 6,
            "scratch is bounded across failures and compactions"
        );
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
                0 => bad.base_entries.push(bad.base_entries[0].clone()),
                1 => {
                    bad.delta = Some(DeltaManifest {
                        sha256: "ab".repeat(32),
                        entries: vec![bad.base_entries[0].clone()],
                    })
                }
                2 => bad.entries.push("ff".repeat(32)),
                3 => bad.index_sha256 = "../outside".into(),
                4 => {
                    let entries: Vec<_> = (100..165).map(|i| format!("{i:064x}")).collect();
                    bad.entries.extend(entries.clone());
                    bad.delta = Some(DeltaManifest {
                        sha256: "ab".repeat(32),
                        entries,
                    });
                }
                _ => {
                    bad.entries.drain(..65);
                }
            }
            assert!(validate_membership(&bad).is_err());
        }
    }
}
