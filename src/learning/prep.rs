//! Dataset preparation (`learning prepare`): the outcome types, the eligible
//! rows and their checks, the parent dataset and its lineage, and the two
//! passes that write the dataset. Output publication, the renderer and the
//! `Engine` surface stay in the parent module.
use super::*;

// ---------------------------------------------------------------------------
// Preparation
// ---------------------------------------------------------------------------

/// What a preparation run produced.
#[derive(Debug)]
pub enum PrepareOutcome {
    Completed(Box<Prepared>),
    /// Zero new rows with a valid base: nothing written, exit code 0.
    NoNewData {
        parent_manifest_sha256: String,
    },
}

#[derive(Debug)]
pub struct Prepared {
    pub manifest_path: PathBuf,
    pub dataset_id: String,
    pub new_rows: usize,
    pub replay_rows: usize,
    pub train_rows: usize,
    pub calibration_rows: usize,
    pub evaluation_rows: usize,
    pub groups: usize,
    /// The destination already held exactly this dataset — published by an
    /// earlier run of the same preparation that died before recording it —
    /// and was verified and recorded instead of written again.
    pub adopted: bool,
}

/// Metadata of one eligible example, kept between the two passes. The text
/// and the token IDs stay in the store until the second pass streams them.
struct Eligible {
    example_id: String,
    group: String,
    label: String,
    input_sha256: String,
    permission_sha256: String,
    fingerprint: String,
}

impl Eligible {
    /// The coverage entry `groups.jsonl` lists for this example.
    fn contribution(&self) -> Contribution {
        Contribution {
            example_id: self.example_id.clone(),
            input_sha256: self.input_sha256.clone(),
            correct_option_id: self.label.clone(),
            permission_sha256: self.permission_sha256.clone(),
        }
    }
}

/// One output file being written: bytes are hashed as they are appended, so
/// the manifest binds the exact bytes without re-reading or re-serializing.
struct Sink {
    name: &'static str,
    writer: std::io::BufWriter<File>,
    hasher: Sha256,
    bytes: u64,
    rows: u64,
}

impl Sink {
    /// A NEW member (`O_EXCL | O_NOFOLLOW`) in the held partial directory.
    fn create(dir: &Dir, name: &'static str) -> FResult<Self> {
        let file = dir
            .create_new(name)
            .map_err(|e| fail("output_write", format!("create {name}: {e}")))?;
        Ok(Self {
            name,
            writer: std::io::BufWriter::new(file),
            hasher: Sha256::new(),
            bytes: 0,
            rows: 0,
        })
    }

    /// Append one compact-JSON row plus LF, serialized into `line` (one
    /// buffer shared by every sink). `total` is the running size of every
    /// data file, refused (never truncated) past the combined bound.
    fn append<T: Serialize>(
        &mut self,
        row: &T,
        line: &mut Vec<u8>,
        total: &mut u64,
    ) -> FResult<()> {
        line.clear();
        serde_json::to_writer(&mut *line, row).map_err(FoundryError::from)?;
        line.push(b'\n');
        if line.len() > ROW_MAX_BYTES {
            return Err(fail(
                "dataset_bounds",
                format!(
                    "a dataset row is {} bytes; the limit is {ROW_MAX_BYTES}",
                    line.len()
                ),
            ));
        }
        *total += line.len() as u64;
        if *total > FILES_MAX_BYTES {
            return Err(fail(
                "dataset_bounds",
                format!("dataset files exceed the {FILES_MAX_BYTES}-byte bound"),
            ));
        }
        self.hasher.update(line.as_slice());
        self.writer
            .write_all(line.as_slice())
            .map_err(|e| fail("output_write", format!("write {}: {e}", self.name)))?;
        self.bytes += line.len() as u64;
        self.rows += 1;
        Ok(())
    }

    /// Flush, fsync and return the manifest entry for the exact bytes.
    fn finish(self) -> FResult<ManifestFile> {
        let name = self.name;
        let file = self
            .writer
            .into_inner()
            .map_err(|e| fail("output_write", format!("flush {name}: {}", e.error())))?;
        file.sync_all()
            .map_err(|e| fail("output_write", format!("fsync {name}: {e}")))?;
        Ok(ManifestFile {
            name: name.to_owned(),
            sha256: crate::lower_hex(&self.hasher.finalize()),
            bytes: self.bytes,
            rows: self.rows,
        })
    }
}

/// Pass 1: page the feedback table and keep only the metadata of the
/// eligible (`allow_training`) rows, in example-ID order. A row denied
/// training at admission, or later withdrawn, stays in the store and in
/// history, never in a dataset.
fn collect_eligible(engine: &Engine, control: &Control) -> FResult<Vec<Eligible>> {
    let mut eligible: Vec<Eligible> = Vec::new();
    engine.learning_for_each_row(control, |example_id, row| {
        if !row.allow_training {
            return Ok(());
        }
        if eligible.len() >= MAX_ROWS {
            return Err(fail(
                "dataset_bounds",
                format!("more than {MAX_ROWS} eligible rows; nothing is dropped"),
            ));
        }
        eligible.push(Eligible {
            fingerprint: row.fingerprint(),
            input_sha256: row.input_sha256(),
            permission_sha256: row.permission_sha256(),
            group: row.task_group_id,
            label: row.correct_option_id,
            example_id,
        });
        Ok(())
    })?;
    Ok(eligible)
}

/// Duplicate fingerprints: exact normalized duplicates across groups or
/// with conflicting current labels — including historical fingerprints,
/// withdrawn groups' included.
fn check_duplicates(eligible: &[Eligible], history: &HistoryIndex) -> FResult<()> {
    let mut by_fingerprint: BTreeMap<&str, &Eligible> = BTreeMap::new();
    for current in eligible {
        match by_fingerprint.get(current.fingerprint.as_str()) {
            Some(first) => {
                if first.group != current.group {
                    return Err(fail(
                        "duplicate_conflict",
                        format!(
                            "examples {} and {} share one fingerprint across groups {} and {}",
                            first.example_id, current.example_id, first.group, current.group
                        ),
                    ));
                }
                if first.label != current.label {
                    return Err(fail(
                        "duplicate_conflict",
                        format!(
                            "examples {} and {} in group {} carry conflicting current labels",
                            first.example_id, current.example_id, first.group
                        ),
                    ));
                }
            }
            None => {
                by_fingerprint.insert(current.fingerprint.as_str(), current);
            }
        }
    }
    for (fingerprint, current) in &by_fingerprint {
        if let Some(other) = history
            .fingerprint_groups
            .get(*fingerprint)
            .and_then(|groups| groups.iter().find(|group| **group != current.group))
        {
            return Err(fail(
                "duplicate_conflict",
                format!(
                    "example {} in group {} repeats a fingerprint recorded for historical \
                     group {other}; exact normalized duplicates are conflicts",
                    current.example_id, current.group
                ),
            ));
        }
    }
    Ok(())
}

/// Base validity, checked BEFORE novelty: every contribution of the base's
/// COMPLETE coverage — inherited ones its bounded train file omits included
/// — must still be an eligible row in the same group with identical
/// permission (consent and rights assertion), inputs and label.
fn check_base(base: &VerifiedDataset, current_by_id: &BTreeMap<&str, &Eligible>) -> FResult<()> {
    for (id, (group, prior)) in &base.coverage {
        let Some(current) = current_by_id.get(id.as_str()) else {
            return Err(fail(
                "base_permission_changed",
                format!("base example {id} is missing or no longer permitted in the current store"),
            ));
        };
        let changed = if current.group != *group {
            Some("group")
        } else if current.input_sha256 != prior.input_sha256 {
            Some("inputs")
        } else if current.label != prior.correct_option_id {
            Some("labels")
        } else if current.permission_sha256 != prior.permission_sha256 {
            Some("permission (consent and rights assertion)")
        } else {
            None
        };
        if let Some(what) = changed {
            return Err(fail(
                "base_permission_changed",
                format!("base example {id} no longer has identical {what}"),
            ));
        }
    }
    Ok(())
}

/// Recorded assignments — the base dataset's coverage and the store's group
/// history — must equal the deterministic split rule for every current
/// group: ancestor assignments never move. Checked BEFORE the no-op return,
/// so `no_new_data` never hides a historical conflict.
fn check_splits(
    eligible: &[Eligible],
    base: Option<&VerifiedDataset>,
    history: &HistoryIndex,
) -> FResult<()> {
    let groups: BTreeSet<&str> = eligible
        .iter()
        .map(|current| current.group.as_str())
        .collect();
    for group in groups {
        let computed = split_of(group);
        let recorded = [
            ("the base dataset", base.and_then(|b| b.splits.get(group))),
            ("the store history", history.splits.get(group)),
        ];
        for (source, split) in recorded {
            if let Some(split) = split.filter(|split| split.as_str() != computed) {
                return Err(fail(
                    "split_conflict",
                    format!(
                        "group {group} is {split} in {source} but the split rule says {computed}"
                    ),
                ));
            }
        }
    }
    Ok(())
}

/// The group-history entries a dataset adds: split, example IDs and
/// distinct fingerprints per group, without text.
fn group_history_entries(
    group_of: &BTreeMap<&str, Vec<&Eligible>>,
) -> BTreeMap<String, GroupHistory> {
    group_of
        .iter()
        .map(|(group, members)| {
            let mut fingerprints: Vec<String> = Vec::new();
            for member in members {
                if !fingerprints.contains(&member.fingerprint) {
                    fingerprints.push(member.fingerprint.clone());
                }
            }
            let entry = GroupHistory {
                group_id: (*group).to_owned(),
                split: split_of(group).to_owned(),
                datasets: Vec::new(),
                example_ids: members
                    .iter()
                    .map(|member| member.example_id.clone())
                    .collect(),
                fingerprints,
            };
            ((*group).to_owned(), entry)
        })
        .collect()
}

/// Load the base dataset of a repeat round. It is read back exactly as
/// `learning check` reads it (descriptor-anchored, bounded, every row
/// re-rendered, ownership, coverage and floors validated), must belong to
/// this workspace and model function, and must resolve through the store's
/// recorded lineage.
fn load_parent(
    engine: &Engine,
    path: &Path,
    workspace_id: &str,
    model_function: &str,
    loaded: &LoadedRenderer,
    control: &Control,
) -> FResult<VerifiedDataset> {
    let unreadable = |e: FoundryError| {
        if e.code() == "artifact_unavailable" {
            fail(
                "lineage_missing",
                format!("parent manifest {}: unreadable", path.display()),
            )
        } else {
            e
        }
    };
    let opened = open_dataset(path).map_err(unreadable)?;
    if opened.manifest.workspace_id != workspace_id {
        return Err(fail(
            "lineage_missing",
            "parent manifest belongs to a different workspace",
        ));
    }
    if opened.manifest.model_function_sha256 != model_function {
        return Err(fail(
            "base_model_changed",
            "parent manifest was prepared by a different model function",
        ));
    }
    let parent = verify_dataset(opened, loaded, control).map_err(unreadable)?;
    verify_lineage(engine, &parent, control)?;
    Ok(parent)
}

/// The parent must be a dataset this store PUBLISHED — its exact manifest
/// bytes are recorded, so a partial left by a dead process or a resealed
/// copy is not — and every ancestor it names must resolve to a recorded
/// entry. Old dataset directories need not stay on disk; their recorded
/// history must. Anything missing is `lineage_missing`.
pub(super) fn verify_lineage(
    engine: &Engine,
    parent: &VerifiedDataset,
    control: &Control,
) -> FResult<()> {
    let Some(record) = engine.learning_dataset(&parent.manifest_sha256)? else {
        return Err(fail(
            "lineage_missing",
            format!(
                "parent manifest {} is not a dataset this store published",
                parent.manifest_sha256
            ),
        ));
    };
    if record.dataset_id != parent.manifest.dataset_id
        || record.parent_manifest_sha256 != parent.manifest.parent_manifest_sha256
    {
        return Err(FoundryError::CorruptStore(format!(
            "learning dataset record {} disagrees with its manifest",
            parent.manifest_sha256
        )));
    }
    let mut next = record.parent_manifest_sha256;
    for _ in 0..MAX_LINEAGE {
        let Some(ancestor) = next else {
            return Ok(());
        };
        control.check()?;
        let Some(entry) = engine.learning_dataset(&ancestor)? else {
            return Err(fail(
                "lineage_missing",
                format!("ancestor manifest {ancestor} has no recorded history in this store"),
            ));
        };
        next = entry.parent_manifest_sha256;
    }
    Err(FoundryError::CorruptStore(format!(
        "learning lineage is longer than {MAX_LINEAGE} rounds"
    )))
}

/// `learning prepare --out DIR --policy FILE [--parent MANIFEST]` under
/// exclusive store ownership.
///
/// The parent, when given, is read back exactly as `learning check` reads
/// it and must resolve through the store's recorded lineage. Pass 1 pages
/// the feedback table by 128 and keeps only metadata for the eligible rows;
/// duplicate fingerprints, base validity against the parent's complete
/// coverage, split assignments and floors are all decided from that BEFORE
/// the `no_new_data` decision, then the training selection. Pass 2 streams
/// the rows again in example-ID order (the table's key order, so no sorting
/// spill is needed), renders each through the exact renderer and appends it
/// to its file while hashing the exact bytes. Output goes to an owned
/// partial sibling under the held parent directory, manifest last, and is
/// published by a no-replace rename; the dataset is recorded only once
/// published. An unrecorded destination that reads back in full as exactly
/// this run's dataset (an earlier run died before recording it) is adopted.
pub fn prepare(
    engine: &Engine,
    out: &Path,
    policy_path: &Path,
    parent: Option<&Path>,
    control: &Control,
) -> FResult<PrepareOutcome> {
    control.check()?;
    let workspace_id = engine
        .workspace_id()
        .ok_or(FoundryError::WorkspaceUnbound)?;
    let (policy, policy_sha256) = LearningPolicy::load(policy_path)?;
    let loaded = load_renderer(&policy)?;
    let model_function = model_function_of(&policy, &loaded);
    let target = OutputTarget::open(engine, out, &|manifest| {
        // A destination holding a manifest this store never recorded may be
        // this preparation's own earlier publication (lost response).
        Ok(engine.learning_dataset(&crate::digest(manifest))?.is_none())
    })?;
    // The parent (base dataset) is verified BEFORE any novelty decision.
    let base = parent
        .map(|path| {
            load_parent(
                engine,
                path,
                &workspace_id,
                &model_function,
                &loaded,
                control,
            )
        })
        .transpose()?;

    // ---- Pass 1: metadata of the eligible rows, paged.
    let history = engine.learning_history_index(control)?;
    let eligible = collect_eligible(engine, control)?;
    check_duplicates(&eligible, &history)?;
    let current_by_id: BTreeMap<&str, &Eligible> = eligible
        .iter()
        .map(|current| (current.example_id.as_str(), current))
        .collect();
    if let Some(base) = &base {
        check_base(base, &current_by_id)?;
    }
    check_splits(&eligible, base.as_ref(), &history)?;
    let mut group_of: BTreeMap<&str, Vec<&Eligible>> = BTreeMap::new();
    for current in &eligible {
        group_of
            .entry(current.group.as_str())
            .or_default()
            .push(current);
    }
    if group_of.len() > MAX_GROUPS {
        return Err(fail(
            "dataset_bounds",
            format!(
                "{} groups exceed the {MAX_GROUPS}-group bound",
                group_of.len()
            ),
        ));
    }
    // A repeat round trains on new rows plus bounded replay but keeps the
    // full coverage, so the floors are judged on every eligible group.
    let split_groups = check_floors(eligible.iter().map(|current| {
        (
            current.group.as_str(),
            split_of(&current.group),
            current.label.as_str(),
        )
    }))?;

    // Novelty: eligible examples the base's complete coverage does not
    // hold — new inputs in old groups included.
    let new_ids: BTreeSet<&str> = eligible
        .iter()
        .filter(|current| {
            base.as_ref()
                .is_none_or(|base| !base.coverage.contains_key(&current.example_id))
        })
        .map(|current| current.example_id.as_str())
        .collect();
    if let Some(base) = &base
        && new_ids.is_empty()
    {
        return Ok(PrepareOutcome::NoNewData {
            parent_manifest_sha256: base.manifest_sha256.clone(),
        });
    }

    // Train contributions: all NEW train-split rows plus up to one unchanged
    // permitted base replay row per new row, drawn from the base's complete
    // train coverage in increasing SHA256(compact JSON [seed,id]) order;
    // never held-out rows.
    let new_train: BTreeSet<&str> = new_ids
        .iter()
        .copied()
        .filter(|id| {
            current_by_id
                .get(*id)
                .is_some_and(|current| split_of(&current.group) == "train")
        })
        .collect();
    let mut replay: Vec<&str> = Vec::new();
    if let Some(base) = &base {
        let mut candidates: Vec<(String, &str)> = base
            .coverage
            .iter()
            .filter(|(_, (group, _))| split_of(group) == "train")
            .map(|(id, _)| {
                (
                    compact_digest(&serde_json::json!([policy.seed, id])),
                    id.as_str(),
                )
            })
            .collect();
        candidates.sort_unstable();
        replay = candidates
            .into_iter()
            .take(new_train.len())
            .map(|(_, id)| id)
            .collect();
    }
    let train_ids: BTreeSet<&str> = new_train.iter().chain(replay.iter()).copied().collect();
    let history_entries = group_history_entries(&group_of);

    // ---- Pass 2: stream the rows, in example-ID order, into the owned
    // partial sibling.
    let (partial, guard) = target.create_partial()?;
    let mut line = Vec::new();
    let mut total_bytes = 0u64;
    let mut train = Sink::create(&partial, "train.jsonl")?;
    let mut calibration = Sink::create(&partial, "calibration.jsonl")?;
    let mut evaluation = Sink::create(&partial, "evaluation.jsonl")?;
    let mut cursor = 0usize;
    engine.learning_for_each_row(control, |example_id, row| {
        if !row.allow_training {
            return Ok(());
        }
        // The store has one owner, so pass 2 must see pass 1's rows exactly.
        let Some(expected) = eligible.get(cursor).filter(|e| e.example_id == example_id) else {
            return Err(FoundryError::CorruptStore(
                "feedback changed under preparation".into(),
            ));
        };
        cursor += 1;
        let sink = match split_of(&expected.group) {
            "train" if train_ids.contains(example_id.as_str()) => &mut train,
            "calibration" => &mut calibration,
            "evaluation" => &mut evaluation,
            _ => return Ok(()),
        };
        let exact = loaded.renderer.render(&row.state, row.ordered_options())?;
        let dataset_row = DatasetRow {
            example_id,
            input_sha256: row.input_sha256(),
            token_ids: exact.ids,
            markers: exact.markers,
            feedback: row,
        };
        sink.append(&dataset_row, &mut line, &mut total_bytes)
    })?;
    if cursor != eligible.len() {
        return Err(FoundryError::CorruptStore(
            "feedback changed under preparation".into(),
        ));
    }
    let mut groups_sink = Sink::create(&partial, "groups.jsonl")?;
    for (group, members) in &group_of {
        groups_sink.append(
            &GroupRow {
                group_id: (*group).to_owned(),
                split: split_of(group).to_owned(),
                examples: members.iter().map(|member| member.contribution()).collect(),
            },
            &mut line,
            &mut total_bytes,
        )?;
    }
    let (train_rows, calibration_rows, evaluation_rows, groups) = (
        train.rows as usize,
        calibration.rows as usize,
        evaluation.rows as usize,
        groups_sink.rows as usize,
    );
    let mut file_entries = vec![
        train.finish()?,
        calibration.finish()?,
        evaluation.finish()?,
        groups_sink.finish()?,
    ];
    file_entries.sort_by(|a, b| a.name.cmp(&b.name));

    let parent_manifest_sha256 = base.as_ref().map(|b| b.manifest_sha256.clone());
    let dataset_id = dataset_id_of(
        &workspace_id,
        parent_manifest_sha256.as_deref(),
        &model_function,
        &policy_sha256,
        &file_entries,
    )?;
    let manifest = Manifest {
        schema: 4,
        recipe: RECIPE.to_owned(),
        workspace_id,
        dataset_id: dataset_id.clone(),
        parent_manifest_sha256: parent_manifest_sha256.clone(),
        model_function_sha256: model_function,
        policy_sha256,
        base_candidate_sha256: None,
        files: file_entries,
        split_group_counts: split_groups,
    };
    let manifest_body = serde_json::to_string(&manifest).map_err(FoundryError::from)?;
    if manifest_body.len() > MANIFEST_MAX_BYTES {
        return Err(fail(
            "dataset_bounds",
            format!(
                "manifest is {} bytes; the limit is {MANIFEST_MAX_BYTES}",
                manifest_body.len()
            ),
        ));
    }
    // The manifest is written last (and fsynced), then the partial
    // directory's entries are flushed before anything can publish it.
    partial
        .write_new("manifest.json", manifest_body.as_bytes())
        .map_err(|e| fail("output_write", format!("write manifest.json: {e}")))?;
    partial
        .sync_all()
        .map_err(|e| fail("output_write", format!("fsync the partial output: {e}")))?;
    let manifest_sha256 = crate::digest(manifest_body.as_bytes());

    learning_fault!(BEFORE_HISTORY, control, &dataset_id)?;
    control.check()?;
    // Group history BEFORE publication: nothing becomes visible whose groups
    // are not already in the trail.
    engine.extend_learning_history(&dataset_id, &history_entries)?;
    learning_fault!(BEFORE_RENAME, control, &dataset_id)?;
    let adopted = match target.publish(&guard)? {
        Publication::Published => false,
        // Lost response (verify output before repeating work): an unrecorded
        // destination that reads back as exactly this run's dataset is this
        // preparation's own earlier publication. Anything else is somebody
        // else's and stays untouched.
        Publication::Occupied => {
            if engine.learning_dataset(&manifest_sha256)?.is_some()
                || !occupied_reads_back_as(&target, &manifest_sha256, &loaded, control)?
            {
                return Err(fail(
                    "output_exists",
                    "the output destination appeared during preparation; it was left untouched",
                ));
            }
            // The dead run may not have reached its fsync.
            target.sync_parent()?;
            true
        }
    };
    learning_fault!(AFTER_PUBLISH, control, &dataset_id)?;
    // Recorded only once published: an unpublished partial never resolves
    // as a parent. An adopted run's own partial is removed when the guard
    // drops.
    engine.record_learning_dataset(
        &manifest_sha256,
        &DatasetRecord {
            dataset_id: dataset_id.clone(),
            parent_manifest_sha256,
        },
    )?;
    drop(guard);
    Ok(PrepareOutcome::Completed(Box::new(Prepared {
        manifest_path: out.join("manifest.json"),
        dataset_id,
        new_rows: new_ids.len(),
        replay_rows: replay.len(),
        train_rows,
        calibration_rows,
        evaluation_rows,
        groups,
        adopted,
    })))
}

/// Whether the occupied destination IS this run's dataset. Opened through
/// the held parent without following links, its manifest must have this
/// run's SHA-256 (identical bytes), and the whole dataset must then read
/// back exactly as `learning check` reads it: every member's length and
/// hash, framing, grouping, floors and exact rendering. Identical manifest
/// bytes alone prove nothing about the members. Cancellation propagates;
/// any other failure is "no".
fn occupied_reads_back_as(
    target: &OutputTarget,
    manifest_sha256: &str,
    loaded: &LoadedRenderer,
    control: &Control,
) -> FResult<bool> {
    let Ok(Some(dir)) = target.parent.open_dir(&target.name) else {
        return Ok(false);
    };
    let read_back = open_dataset_in(
        dir,
        std::ffi::OsStr::new("manifest.json"),
        "the occupied output's manifest",
    )
    .and_then(|opened| {
        if opened.manifest_sha256 != manifest_sha256 {
            return Err(fail(
                "output_exists",
                "the occupied output holds another manifest",
            ));
        }
        verify_dataset(opened, loaded, control)
    });
    match read_back {
        Ok(_) => Ok(true),
        Err(error @ (FoundryError::Cancelled(_) | FoundryError::DeadlineExceeded(_))) => Err(error),
        Err(_) => Ok(false),
    }
}
