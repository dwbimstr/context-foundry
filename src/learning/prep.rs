//! Dataset preparation and read-back (`learning prepare`, `learning check`):
//! the outcome types, the eligible rows and their checks, the parent dataset
//! and its lineage, the two passes that write the dataset, and the read-back
//! that verifies every member of one. Output publication, the renderer and
//! the `Engine` surface stay in the parent module.
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

/// Lifecycle floors over a COMPLETE coverage of `(group, split, correct
/// option)` entries: at least 20 train, 10 calibration and 20 evaluation
/// groups, with both correct-option labels in each split. Floors describe
/// permitted coverage, never the bounded replay subset; they are lifecycle
/// floors, not statistical significance. Returns the per-split group counts.
fn check_floors<'a>(
    entries: impl Iterator<Item = (&'a str, &'a str, &'a str)>,
) -> FResult<BTreeMap<String, usize>> {
    let mut groups: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
    let mut labels: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
    for (group, split, label) in entries {
        groups.entry(split).or_default().insert(group);
        labels.entry(split).or_default().insert(label);
    }
    let mut counts = BTreeMap::new();
    for (split, floor) in [
        ("train", TRAIN_GROUPS_FLOOR),
        ("calibration", CALIBRATION_GROUPS_FLOOR),
        ("evaluation", EVALUATION_GROUPS_FLOOR),
    ] {
        let present = labels.get(split);
        let missing: Vec<&str> = decision_model::OPTIONS
            .iter()
            .map(|def| def.id)
            .filter(|id| present.is_none_or(|labels| !labels.contains(id)))
            .collect();
        if !missing.is_empty() {
            return Err(fail(
                "group_floors",
                format!("split {split} has no correct-option label for {missing:?}"),
            ));
        }
        let count = groups.get(split).map_or(0, BTreeSet::len);
        if count < floor {
            return Err(fail(
                "group_floors",
                format!("{split} has {count} groups, needs {floor}"),
            ));
        }
        counts.insert(split.to_owned(), count);
    }
    Ok(counts)
}

// ---------------------------------------------------------------------------
// Read-back check
// ---------------------------------------------------------------------------

/// A dataset read back completely: every member byte matched its manifest
/// entry, every row matched the exact renderer and its group's coverage, and
/// the coverage met the lifecycle floors. Rows are verified as they stream;
/// only metadata is kept.
pub(super) struct VerifiedDataset {
    pub(super) manifest: Manifest,
    pub(super) manifest_sha256: String,
    /// Example ID → (group, contribution): the COMPLETE coverage, inherited
    /// contributions the bounded train file omits included.
    pub(super) coverage: BTreeMap<String, (String, Contribution)>,
    /// Group → split.
    splits: BTreeMap<String, String>,
    /// Data rows across the three data files.
    rows: usize,
}

/// What `learning check` verified.
#[derive(Debug, Serialize)]
pub struct CheckReport {
    pub manifest_sha256: String,
    pub dataset_id: String,
    pub rows: usize,
    pub groups: usize,
}

/// `learning check --manifest FILE --policy FILE`. The pinned tokenizer is a
/// prerequisite, not an option: every row's token IDs and markers are
/// re-rendered and compared, and an unavailable or mismatched tokenizer is a
/// named refusal — never a structural pass. Also verifies the manifest's
/// exact bytes, bounds (before reading), sort order, dataset identity, the
/// group each row belongs to, and the coverage floors.
pub fn check(manifest_path: &Path, policy_path: &Path, control: &Control) -> FResult<CheckReport> {
    control.check()?;
    let (policy, _) = LearningPolicy::load(policy_path)?;
    let loaded = load_renderer(&policy)?;
    let function = model_function_of(&policy, &loaded);
    let opened = open_dataset(manifest_path)?;
    if function != opened.manifest.model_function_sha256 {
        return Err(fail(
            "dataset_invalid",
            "manifest model_function_sha256 does not match the policy's tokenizer",
        ));
    }
    let dataset = verify_dataset(opened, &loaded, control)?;
    Ok(CheckReport {
        manifest_sha256: dataset.manifest_sha256,
        dataset_id: dataset.manifest.dataset_id,
        rows: dataset.rows,
        groups: dataset.splits.len(),
    })
}

/// Open a dataset through its directory, held ONCE: a symlinked dataset
/// directory or manifest is refused (`O_NOFOLLOW`); see [`open_dataset_in`].
pub(super) fn open_dataset(manifest_path: &Path) -> FResult<OpenedDataset> {
    let name = manifest_path.file_name().ok_or_else(|| {
        FoundryError::InvalidArgument(format!(
            "manifest {} names no file",
            manifest_path.display()
        ))
    })?;
    let dir_path = manifest_path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let dir = Dir::open_path(dir_path)
        .map_err(|e| dataset_open_error(&format!("dataset directory {}", dir_path.display()), e))?;
    open_dataset_in(dir, name, &format!("manifest {}", manifest_path.display()))
}

/// NotFound is an unavailable artifact; a symlink, a non-directory or a
/// non-regular file where the dataset needs one is refused.
fn dataset_open_error(what: &str, e: std::io::Error) -> FoundryError {
    if e.kind() == std::io::ErrorKind::NotFound {
        FoundryError::ArtifactUnavailable(format!("{what}: {e}"))
    } else {
        fail(
            "lineage_missing",
            format!("{what} is a symlink, not a regular entry or unreadable: {e}"),
        )
    }
}

/// Read a dataset's manifest `name` through its held directory: opened
/// without following a link, bounded by its descriptor's length before it is
/// read and parsed strictly; its identity is recomputed, and the DECLARED
/// member sizes and row counts are held to the bounds before any member is
/// read.
fn open_dataset_in(dir: Dir, name: &std::ffi::OsStr, what: &str) -> FResult<OpenedDataset> {
    let raw = read_capped(
        dir.open_file(name)
            .map_err(|e| dataset_open_error(what, e))?,
        MANIFEST_MAX_BYTES as u64,
    )
    .map_err(|e| dataset_open_error(what, e))?
    .ok_or_else(|| {
        fail(
            "dataset_bounds",
            format!("manifest exceeds the {MANIFEST_MAX_BYTES}-byte bound"),
        )
    })?;
    let manifest_sha256 = crate::digest(&raw);
    let manifest: Manifest = strict_json(&raw, "dataset_invalid", "manifest")?;
    if manifest.schema != 4 {
        return Err(fail(
            "dataset_invalid",
            format!(
                "manifest schema is {}; only 4 is supported",
                manifest.schema
            ),
        ));
    }
    if manifest.recipe != RECIPE {
        return Err(fail(
            "dataset_invalid",
            format!("manifest recipe {:?} is not {RECIPE}", manifest.recipe),
        ));
    }
    // Fixed basenames only: no path component ever comes from the manifest.
    let names: BTreeSet<&str> = manifest.files.iter().map(|f| f.name.as_str()).collect();
    if manifest.files.len() != FILES.len()
        || names != FILES.iter().copied().collect::<BTreeSet<&str>>()
    {
        return Err(fail(
            "dataset_invalid",
            "manifest must bind exactly the four fixed basenames",
        ));
    }
    // Recompute the dataset identity from the manifest's own fields.
    let recomputed = dataset_id_of(
        &manifest.workspace_id,
        manifest.parent_manifest_sha256.as_deref(),
        &manifest.model_function_sha256,
        &manifest.policy_sha256,
        &manifest.files,
    )?;
    if recomputed != manifest.dataset_id {
        return Err(fail(
            "dataset_invalid",
            "dataset_id does not match the manifest's fields and file hashes",
        ));
    }
    // Bounds from the DECLARED sizes and counts, before any member is read.
    let mut declared_bytes = 0u64;
    for entry in &manifest.files {
        declared_bytes = declared_bytes
            .checked_add(entry.bytes)
            .filter(|total| *total <= FILES_MAX_BYTES)
            .ok_or_else(|| {
                fail(
                    "dataset_bounds",
                    format!(
                        "the manifest declares more than the {FILES_MAX_BYTES}-byte combined file bound"
                    ),
                )
            })?;
    }
    let declared_rows = manifest
        .files
        .iter()
        .filter(|entry| entry.name != "groups.jsonl")
        .try_fold(0u64, |total, entry| total.checked_add(entry.rows))
        .unwrap_or(u64::MAX);
    if declared_rows > MAX_ROWS as u64 {
        return Err(fail(
            "dataset_bounds",
            format!("the manifest declares {declared_rows} rows; the bound is {MAX_ROWS}"),
        ));
    }
    let declared_groups = manifest_entry(&manifest.files, "groups.jsonl")?.rows;
    if declared_groups > MAX_GROUPS as u64 {
        return Err(fail(
            "dataset_bounds",
            format!("the manifest declares {declared_groups} groups; the bound is {MAX_GROUPS}"),
        ));
    }
    Ok(OpenedDataset {
        dir,
        manifest,
        manifest_sha256,
    })
}

fn manifest_entry<'a>(files: &'a [ManifestFile], name: &str) -> FResult<&'a ManifestFile> {
    files
        .iter()
        .find(|entry| entry.name == name)
        .ok_or_else(|| fail("dataset_invalid", format!("manifest lacks {name}")))
}

/// The dataset identity: the compact digest of the workspace, the parent
/// manifest, the model function, the policy and the three data files'
/// hashes. `prepare` writes it; every read-back recomputes it.
fn dataset_id_of(
    workspace_id: &str,
    parent_manifest_sha256: Option<&str>,
    model_function_sha256: &str,
    policy_sha256: &str,
    files: &[ManifestFile],
) -> FResult<String> {
    Ok(compact_digest(&serde_json::json!([
        workspace_id,
        parent_manifest_sha256,
        model_function_sha256,
        policy_sha256,
        manifest_entry(files, "train.jsonl")?.sha256,
        manifest_entry(files, "calibration.jsonl")?.sha256,
        manifest_entry(files, "evaluation.jsonl")?.sha256,
    ])))
}

/// Read back every member of an opened dataset. `groups.jsonl` comes first:
/// its COMPLETE coverage is what every data row must belong to, what the
/// split counts and the lifecycle floors are judged on, and what a repeat
/// round validates inherited contributions against.
fn verify_dataset(
    opened: OpenedDataset,
    loaded: &LoadedRenderer,
    control: &Control,
) -> FResult<VerifiedDataset> {
    verify_dataset_with(opened, loaded, control, &mut |_, _| {})
}

/// [`verify_dataset`], handing every data row to `keep` (with its split)
/// once it has been verified: training reads the exact bytes it verified,
/// in one pass, never a second copy of the files.
pub(super) fn verify_dataset_with(
    opened: OpenedDataset,
    loaded: &LoadedRenderer,
    control: &Control,
    keep: &mut dyn FnMut(&'static str, DatasetRow),
) -> FResult<VerifiedDataset> {
    let OpenedDataset {
        dir,
        manifest,
        manifest_sha256,
    } = opened;
    let mut coverage: BTreeMap<String, (String, Contribution)> = BTreeMap::new();
    let mut splits: BTreeMap<String, String> = BTreeMap::new();
    let mut prior_group: Option<String> = None;
    stream_member(
        &dir,
        manifest_entry(&manifest.files, "groups.jsonl")?,
        control,
        |line| {
            let group: GroupRow = strict_json(line, "dataset_invalid", "groups.jsonl row")?;
            if prior_group
                .as_deref()
                .is_some_and(|prior| group.group_id.as_str() <= prior)
            {
                return Err(fail(
                    "dataset_invalid",
                    "groups.jsonl rows are not sorted by unique group id",
                ));
            }
            let rule = split_of(&group.group_id);
            if group.split != rule {
                return Err(fail(
                    "dataset_invalid",
                    format!(
                        "group {} is recorded as {:?} but the split rule says {rule:?}",
                        group.group_id, group.split
                    ),
                ));
            }
            if group.examples.is_empty() {
                return Err(fail(
                    "dataset_invalid",
                    format!("group {} lists no example", group.group_id),
                ));
            }
            if group
                .examples
                .windows(2)
                .any(|pair| pair[1].example_id <= pair[0].example_id)
            {
                return Err(fail(
                    "dataset_invalid",
                    format!(
                        "group {} examples are not sorted by unique example id",
                        group.group_id
                    ),
                ));
            }
            prior_group = Some(group.group_id.clone());
            for contribution in group.examples {
                if !decision_model::OPTIONS
                    .iter()
                    .any(|def| def.id == contribution.correct_option_id)
                {
                    return Err(fail(
                        "dataset_invalid",
                        format!(
                            "example {} has an unknown correct option",
                            contribution.example_id
                        ),
                    ));
                }
                if coverage.len() >= MAX_ROWS {
                    return Err(fail(
                        "dataset_bounds",
                        format!("groups.jsonl covers more than {MAX_ROWS} examples"),
                    ));
                }
                let id = contribution.example_id.clone();
                if coverage
                    .insert(id.clone(), (group.group_id.clone(), contribution))
                    .is_some()
                {
                    return Err(fail(
                        "dataset_invalid",
                        format!("example {id} appears in two groups"),
                    ));
                }
            }
            splits.insert(group.group_id, group.split);
            Ok(())
        },
    )?;
    let counts = check_floors(coverage.values().map(|(group, contribution)| {
        (
            group.as_str(),
            split_of(group),
            contribution.correct_option_id.as_str(),
        )
    }))?;
    if counts != manifest.split_group_counts {
        return Err(fail(
            "dataset_invalid",
            "split_group_counts does not match groups.jsonl",
        ));
    }
    let mut rows = 0usize;
    let mut held_out = 0usize;
    for split in ["train", "calibration", "evaluation"] {
        let name = format!("{split}.jsonl");
        let what = format!("{name} row");
        let mut prior: Option<String> = None;
        let member = manifest_entry(&manifest.files, &name)?;
        stream_member(&dir, member, control, |line| {
            let read: DatasetRow = strict_json(line, "dataset_invalid", &what)?;
            if prior
                .as_deref()
                .is_some_and(|prior| read.example_id.as_str() <= prior)
            {
                return Err(fail(
                    "dataset_invalid",
                    format!("{name} rows are not sorted by unique example id"),
                ));
            }
            verify_row(&read, split, &coverage, loaded)?;
            prior = Some(read.example_id.clone());
            keep(split, read);
            rows += 1;
            if split != "train" {
                held_out += 1;
            }
            Ok(())
        })?;
    }
    // The train file may be a bounded subset (new rows plus replay) of the
    // train coverage; the held-out files hold ALL of the held-out coverage.
    let listed = coverage
        .values()
        .filter(|(group, _)| split_of(group) != "train")
        .count();
    if held_out != listed {
        return Err(fail(
            "dataset_invalid",
            "the held-out files and groups.jsonl disagree about the example ids",
        ));
    }
    Ok(VerifiedDataset {
        manifest,
        manifest_sha256,
        coverage,
        splits,
        rows,
    })
}

/// One data row: a valid permitted v4 row whose identity derives from its
/// own bytes; owned by exactly the group `groups.jsonl` lists it under, in
/// this file's split, with the listed input digest, label and permission;
/// and carrying exactly the token IDs and markers the pinned renderer
/// produces.
fn verify_row(
    read: &DatasetRow,
    split: &str,
    coverage: &BTreeMap<String, (String, Contribution)>,
    loaded: &LoadedRenderer,
) -> FResult<()> {
    let id = &read.example_id;
    let feedback = &read.feedback;
    let invalid = |message: String| fail("dataset_invalid", format!("example {id} {message}"));
    feedback
        .validate()
        .map_err(|e| invalid(format!("is not a valid row: {e}")))?;
    if !feedback.allow_training {
        return Err(invalid("carries no training consent".into()));
    }
    if read.input_sha256 != feedback.input_sha256() {
        return Err(invalid("has an input digest mismatch".into()));
    }
    if *id != feedback.example_id() {
        return Err(invalid("has an id mismatch".into()));
    }
    let Some((group, listed)) = coverage.get(id) else {
        return Err(invalid("is listed by no groups.jsonl group".into()));
    };
    if *group != feedback.task_group_id {
        return Err(invalid(format!(
            "declares group {} but groups.jsonl lists it under {group}",
            feedback.task_group_id
        )));
    }
    if split_of(group) != split {
        return Err(invalid(format!(
            "sits in {split} but its group is {}",
            split_of(group)
        )));
    }
    if listed.input_sha256 != read.input_sha256
        || listed.correct_option_id != feedback.correct_option_id
        || listed.permission_sha256 != feedback.permission_sha256()
    {
        return Err(invalid(
            "disagrees with its groups.jsonl contribution".into(),
        ));
    }
    let exact = loaded
        .renderer
        .render(&feedback.state, feedback.ordered_options())
        .map_err(|e| invalid(format!("does not render: {e}")))?;
    if exact.ids != read.token_ids || exact.markers != read.markers {
        return Err(invalid(
            "token ids or markers differ from the exact renderer".into(),
        ));
    }
    Ok(())
}

/// Stream one member's rows through the held dataset directory. The member
/// is opened without following a link; its descriptor's length must equal
/// the declared bytes BEFORE anything is read; the read is capped at the
/// declared size plus one byte, so growth is detected; each row is at most
/// 48 KiB with its LF; rows are counted against the declared count while
/// streaming; and the bytes handed to `row` (without the LF) are exactly the
/// bytes hashed.
fn stream_member(
    dir: &Dir,
    entry: &ManifestFile,
    control: &Control,
    mut row: impl FnMut(&[u8]) -> FResult<()>,
) -> FResult<()> {
    let name = entry.name.as_str();
    let missing = |e: std::io::Error| fail("lineage_missing", format!("{name}: {e}"));
    let file = dir.open_file(name).map_err(missing)?;
    let length = file.metadata().map_err(missing)?.len();
    if length != entry.bytes {
        return Err(fail(
            "dataset_invalid",
            format!(
                "{name} is {length} bytes; the manifest says {}",
                entry.bytes
            ),
        ));
    }
    let mut reader = std::io::BufReader::new(file.take(entry.bytes + 1));
    let mut hasher = Sha256::new();
    let (mut bytes, mut rows) = (0u64, 0u64);
    let mut line = Vec::new();
    loop {
        line.clear();
        let read = (&mut reader)
            .take(ROW_MAX_BYTES as u64 + 1)
            .read_until(b'\n', &mut line)
            .map_err(missing)?;
        if read == 0 {
            break;
        }
        if line.len() > ROW_MAX_BYTES {
            return Err(fail(
                "dataset_bounds",
                format!("{name} has a row over {ROW_MAX_BYTES} bytes"),
            ));
        }
        bytes += read as u64;
        if bytes > entry.bytes {
            return Err(fail(
                "dataset_invalid",
                format!("{name} grew past the manifest's {} bytes", entry.bytes),
            ));
        }
        if line.last() != Some(&b'\n') {
            return Err(fail(
                "dataset_invalid",
                format!("{name} has a row without a trailing LF"),
            ));
        }
        rows += 1;
        if rows > entry.rows {
            return Err(fail(
                "dataset_invalid",
                format!("{name} has more rows than the manifest's {}", entry.rows),
            ));
        }
        hasher.update(&line);
        control.check()?;
        row(&line[..line.len() - 1])?;
    }
    if bytes != entry.bytes || rows != entry.rows {
        return Err(fail(
            "dataset_invalid",
            format!(
                "{name} has {rows} rows in {bytes} bytes; the manifest says {} rows in {} bytes",
                entry.rows, entry.bytes
            ),
        ));
    }
    if crate::lower_hex(&hasher.finalize()) != entry.sha256 {
        return Err(fail(
            "dataset_invalid",
            format!("{name} does not match the manifest hash"),
        ));
    }
    Ok(())
}
