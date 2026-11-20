//! 013 T001: permitted feedback rows, group history and the exact dataset
//! (contract `learning-loop.md` §§ Exact input and identity, Feedback and
//! permission, Dataset and repeat rounds).
//!
//! Rows carry the exact state the policy saw; consent (`allow_training`)
//! comes only from the trusted operator CLI. Legacy `feedback` rows (001)
//! stay exportable and are NEVER eligible: they lack the exact inputs and
//! rights this contract requires, and nothing upgrades them automatically.
//!
//! The dataset is immutable and content-addressed: files sorted by example
//! ID, compact JSON, trailing LF; the manifest binds exact bytes and is
//! written last. Preparation refuses on every bound — it never truncates or
//! drops excess data. Output goes to a new owned partial sibling created
//! under the destination's parent directory, held as a descriptor, and is
//! published by a no-replace rename; only that sibling is ever removed.
//! `groups.jsonl` carries the COMPLETE coverage of every contributing
//! example (identity, input digest, label and permission digest), so a
//! repeat round validates inherited contributions that its bounded train
//! file omits; the store records every published dataset's lineage.
use crate::control::Control;
use crate::decision_model::{self, FAMILY, STATE_MAX_BYTES, SpecialIds};
use crate::error::{FResult, FoundryError};
use crate::neural::anchor::Dir;
use crate::store::Engine;
use redb::{ReadableDatabase, ReadableTable};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsString;
use std::fs::File;
use std::io::{BufRead as _, Read as _, Write as _};
use std::ops::Bound;
use std::path::{Path, PathBuf};

mod feedback;
mod io;
mod run_policy;
mod state;
mod strict;
pub use feedback::{Feedback, FeedbackRowV4};
pub(crate) use io::{open_regular, read_capped};
pub use run_policy::{
    Enforcement, LearningPolicy, Level, MAX_CPU_THREADS, MAX_MEMORY_BYTES, MAX_OUTPUT_BYTES,
    MAX_STEPS, MAX_WALL_SECONDS, MIN_MEMORY_BYTES, MIN_OUTPUT_BYTES, OptimizerPin, SelectionPolicy,
    TokenizerPin,
};
pub use state::STATE_LOCATORS;
pub(crate) use strict::strict_json;

/// The training recipe this dataset feeds (contract § One model and one
/// decision).
pub const RECIPE: &str = "foundry-modernbert-choice-v1";

/// Fixed dataset file basenames.
pub const FILES: [&str; 4] = [
    "train.jsonl",
    "calibration.jsonl",
    "evaluation.jsonl",
    "groups.jsonl",
];

const ROW_MAX_BYTES: usize = 48 * 1024;
const WHOLE_ROW_MAX_BYTES: usize = 24 * 1024;
const MANIFEST_MAX_BYTES: usize = 1024 * 1024;
const POLICY_MAX_BYTES: u64 = 64 * 1024;
const FILES_MAX_BYTES: u64 = 256 * 1024 * 1024;
const MAX_ROWS: usize = 100_000;
const MAX_GROUPS: usize = 100_000;
/// Rounds a lineage walk may span before the store is presumed corrupt.
const MAX_LINEAGE: usize = 100_000;
/// JSON nesting is refused past this depth; no learning document nests
/// deeper than four levels.
const JSON_MAX_DEPTH: usize = 32;
/// Feedback is read in pages of 128 with a control checkpoint per page.
const PAGE: usize = 128;

/// Group-split lifecycle floors; both correct-option labels are required in
/// each split on top of these.
const TRAIN_GROUPS_FLOOR: usize = 20;
const CALIBRATION_GROUPS_FLOOR: usize = 10;
const EVALUATION_GROUPS_FLOOR: usize = 20;

pub(crate) const LEARNING_FEEDBACK: redb::TableDefinition<&str, &str> =
    redb::TableDefinition::new("learning_feedback");
pub(crate) const LEARNING_HISTORY: redb::TableDefinition<&str, &str> =
    redb::TableDefinition::new("learning_history");
/// Every PUBLISHED dataset, keyed by the SHA-256 of its exact manifest
/// bytes: the lineage a later round resolves without the old directory.
pub(crate) const LEARNING_DATASETS: redb::TableDefinition<&str, &str> =
    redb::TableDefinition::new("learning_datasets");

/// Create the empty learning tables (store initialization and upgrade).
pub fn init_tables(tx: &redb::WriteTransaction) -> FResult<()> {
    tx.open_table(LEARNING_FEEDBACK)?;
    tx.open_table(LEARNING_HISTORY)?;
    tx.open_table(LEARNING_DATASETS)?;
    Ok(())
}

/// A schema-6 store must carry the learning tables; missing tables are
/// corruption, not something an open recreates.
pub fn check_tables(db: &redb::Database) -> FResult<()> {
    use redb::TableHandle as _;
    let tx = db.begin_read()?;
    for definition in [LEARNING_FEEDBACK, LEARNING_HISTORY, LEARNING_DATASETS] {
        tx.open_table(definition).map_err(|e| {
            FoundryError::CorruptStore(format!("{} table unreadable: {e}", definition.name()))
        })?;
    }
    Ok(())
}

fn fail(code: &'static str, message: impl Into<String>) -> FoundryError {
    FoundryError::Learning {
        code,
        message: message.into(),
    }
}

fn compact_digest(value: &serde_json::Value) -> String {
    crate::digest(
        serde_json::to_string(value)
            .expect("compact JSON of strings")
            .as_bytes(),
    )
}

// ---------------------------------------------------------------------------
// Group history
// ---------------------------------------------------------------------------

/// The persisted per-group trail: split, the datasets that used the group,
/// and its example IDs and fingerprints. No text is retained here, so
/// withdrawn groups keep their split/example/fingerprint history without
/// their content.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GroupHistory {
    pub group_id: String,
    pub split: String,
    pub datasets: Vec<String>,
    pub example_ids: Vec<String>,
    pub fingerprints: Vec<String>,
}

/// The split of `group_id`: first eight SHA256(group-ID) bytes big-endian
/// modulo 10 → 0 evaluation, 1 calibration, 2..9 train.
pub fn split_of(group_id: &str) -> &'static str {
    let hash = crate::digest(group_id.as_bytes());
    let mut prefix = [0u8; 8];
    for (slot, i) in prefix.iter_mut().zip(0..8) {
        *slot = u8::from_str_radix(&hash[i * 2..i * 2 + 2], 16).unwrap_or(0);
    }
    match u64::from_be_bytes(prefix) % 10 {
        0 => "evaluation",
        1 => "calibration",
        _ => "train",
    }
}

// ---------------------------------------------------------------------------
// Engine surface
// ---------------------------------------------------------------------------

/// One page of at most [`PAGE`] rows of a string table strictly after
/// `after`, as owned strings. Every page is its own short read transaction,
/// so no long read transaction ever spans a preparation.
fn page_of(
    db: &redb::Database,
    definition: redb::TableDefinition<&str, &str>,
    after: Option<&str>,
) -> FResult<Vec<(String, String)>> {
    let tx = db.begin_read()?;
    let table = tx.open_table(definition)?;
    let rows = match after {
        None => table.range::<&str>(..)?,
        Some(key) => table.range::<&str>((Bound::Excluded(key), Bound::Unbounded))?,
    };
    let mut page = Vec::with_capacity(PAGE);
    for row in rows.take(PAGE) {
        let (key, value) = row?;
        page.push((key.value().to_owned(), value.value().to_owned()));
    }
    Ok(page)
}

/// The persisted group history in lookup form: each group's recorded split
/// and which groups hold each fingerprint. Loaded once per preparation, in
/// pages.
#[derive(Default)]
struct HistoryIndex {
    splits: BTreeMap<String, String>,
    fingerprint_groups: BTreeMap<String, BTreeSet<String>>,
}

impl Engine {
    /// Record (or correct) one v4 feedback row under sole ownership. The
    /// same input is idempotent; a different label/consent for the same
    /// example ID replaces the row transactionally (`replaced`); identical
    /// bytes are `unchanged`. Changed state or options produce a NEW example
    /// (`created`), never a mislabeled correction.
    pub fn record_learning_feedback(&self, raw: &str) -> FResult<(String, &'static str)> {
        let row = FeedbackRowV4::parse(raw)?;
        let example_id = row.example_id();
        let encoded = serde_json::to_string(&row).map_err(FoundryError::from)?;
        let tx = self.db.begin_write()?;
        let status = {
            let mut table = tx.open_table(LEARNING_FEEDBACK)?;
            let status = match table.get(example_id.as_str())? {
                None => "created",
                Some(existing) if existing.value() == encoded => "unchanged",
                Some(_) => "replaced",
            };
            if status != "unchanged" {
                table.insert(example_id.as_str(), encoded.as_str())?;
            }
            status
        };
        tx.commit()?;
        Ok((example_id, status))
    }

    /// Visit every stored v4 row in example-ID order (the table's key
    /// order), read in pages of 128 with a control checkpoint per page. Each
    /// row is re-validated and its identity re-derived: a stored row that no
    /// longer matches its key is corruption, never data.
    pub fn learning_for_each_row(
        &self,
        control: &Control,
        mut visit: impl FnMut(String, FeedbackRowV4) -> FResult<()>,
    ) -> FResult<()> {
        let mut after: Option<String> = None;
        loop {
            control.check()?;
            let page = page_of(&self.db, LEARNING_FEEDBACK, after.as_deref())?;
            let Some((last, _)) = page.last() else {
                return Ok(());
            };
            after = Some(last.clone());
            for (example_id, raw) in page {
                let row = FeedbackRowV4::parse(&raw).map_err(|e| {
                    FoundryError::CorruptStore(format!("learning row {example_id}: {e}"))
                })?;
                if row.example_id() != example_id {
                    return Err(FoundryError::CorruptStore(format!(
                        "learning row {example_id} is stored under the wrong example id"
                    )));
                }
                visit(example_id, row)?;
            }
        }
    }

    /// Every stored v4 row, collected (tests and small tools; preparation
    /// streams instead).
    pub fn learning_feedback_rows(
        &self,
        control: &Control,
    ) -> FResult<Vec<(String, FeedbackRowV4)>> {
        let mut rows = Vec::new();
        self.learning_for_each_row(control, |example_id, row| {
            rows.push((example_id, row));
            Ok(())
        })?;
        Ok(rows)
    }

    /// The CURRENT stored row of one example, read in its own short
    /// transaction: the pre-fit permission gate re-reads every intended
    /// contribution this way. A stored row that no longer matches its key
    /// is corruption, never data.
    pub fn learning_current_row(&self, example_id: &str) -> FResult<Option<FeedbackRowV4>> {
        let tx = self.db.begin_read()?;
        let table = tx.open_table(LEARNING_FEEDBACK)?;
        let Some(raw) = table.get(example_id)? else {
            return Ok(None);
        };
        let row = FeedbackRowV4::parse(raw.value())
            .map_err(|e| FoundryError::CorruptStore(format!("learning row {example_id}: {e}")))?;
        if row.example_id() != example_id {
            return Err(FoundryError::CorruptStore(format!(
                "learning row {example_id} is stored under the wrong example id"
            )));
        }
        Ok(Some(row))
    }

    fn learning_history_index(&self, control: &Control) -> FResult<HistoryIndex> {
        let mut index = HistoryIndex::default();
        let mut after: Option<String> = None;
        loop {
            control.check()?;
            let page = page_of(&self.db, LEARNING_HISTORY, after.as_deref())?;
            let Some((last, _)) = page.last() else {
                return Ok(index);
            };
            after = Some(last.clone());
            for (group_id, raw) in page {
                let history: GroupHistory = serde_json::from_str(&raw).map_err(|e| {
                    FoundryError::CorruptStore(format!("learning history for {group_id}: {e}"))
                })?;
                for fingerprint in history.fingerprints {
                    index
                        .fingerprint_groups
                        .entry(fingerprint)
                        .or_default()
                        .insert(group_id.clone());
                }
                index.splits.insert(group_id, history.split);
            }
        }
    }

    /// Extend the group history after a dataset is computed: it names
    /// `dataset_id`, keeping split/example/fingerprint trails without text.
    fn extend_learning_history(
        &self,
        dataset_id: &str,
        entries: &BTreeMap<String, GroupHistory>,
    ) -> FResult<()> {
        let tx = self.db.begin_write()?;
        {
            let mut table = tx.open_table(LEARNING_HISTORY)?;
            for (group_id, entry) in entries {
                let merged = match table.get(group_id.as_str())? {
                    Some(raw) => {
                        let mut existing: GroupHistory = serde_json::from_str(raw.value())
                            .map_err(|e| {
                                FoundryError::CorruptStore(format!(
                                    "learning history for {group_id}: {e}"
                                ))
                            })?;
                        if existing.split != entry.split {
                            return Err(fail(
                                "split_conflict",
                                format!(
                                    "group {group_id} is recorded as {} but the split rule says {}",
                                    existing.split, entry.split
                                ),
                            ));
                        }
                        for id in &entry.example_ids {
                            if !existing.example_ids.contains(id) {
                                existing.example_ids.push(id.clone());
                            }
                        }
                        for fingerprint in &entry.fingerprints {
                            if !existing.fingerprints.contains(fingerprint) {
                                existing.fingerprints.push(fingerprint.clone());
                            }
                        }
                        if !existing.datasets.iter().any(|d| d == dataset_id) {
                            existing.datasets.push(dataset_id.to_owned());
                        }
                        existing
                    }
                    None => {
                        let mut fresh = entry.clone();
                        fresh.datasets = vec![dataset_id.to_owned()];
                        fresh
                    }
                };
                let encoded = serde_json::to_string(&merged).map_err(FoundryError::from)?;
                table.insert(group_id.as_str(), encoded.as_str())?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    /// The recorded lineage of a published dataset, by the SHA-256 of its
    /// exact manifest bytes; `None` when this store never published it.
    fn learning_dataset(&self, manifest_sha256: &str) -> FResult<Option<DatasetRecord>> {
        let tx = self.db.begin_read()?;
        let table = tx.open_table(LEARNING_DATASETS)?;
        let Some(raw) = table.get(manifest_sha256)? else {
            return Ok(None);
        };
        serde_json::from_str(raw.value()).map(Some).map_err(|e| {
            FoundryError::CorruptStore(format!("learning dataset record {manifest_sha256}: {e}"))
        })
    }

    /// Record a dataset AFTER it was published: a partial left behind by a
    /// dead process is never recorded, so it can never serve as a parent.
    fn record_learning_dataset(
        &self,
        manifest_sha256: &str,
        record: &DatasetRecord,
    ) -> FResult<()> {
        let encoded = serde_json::to_string(record).map_err(FoundryError::from)?;
        let tx = self.db.begin_write()?;
        tx.open_table(LEARNING_DATASETS)?
            .insert(manifest_sha256, encoded.as_str())?;
        tx.commit()?;
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Dataset rows and manifest
// ---------------------------------------------------------------------------

/// One data-file row: the exact feedback row plus the rendered identity.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DatasetRow {
    pub example_id: String,
    pub input_sha256: String,
    pub feedback: FeedbackRowV4,
    pub token_ids: Vec<u32>,
    pub markers: [usize; 2],
}

/// One contributing example as `groups.jsonl` lists it: enough to validate
/// an inherited contribution in a later round without its text.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Contribution {
    pub example_id: String,
    pub input_sha256: String,
    pub correct_option_id: String,
    /// [`FeedbackRowV4::permission_sha256`]: consent and rights assertion.
    pub permission_sha256: String,
}

/// The first way the current `row` no longer grants the permission it was
/// used under (`recorded`, a [`FeedbackRowV4::permission_sha256`]): training
/// consent withdrawn, then a changed permission. `None` when it still does.
pub(crate) fn consent_change(row: &FeedbackRowV4, recorded: &str) -> Option<&'static str> {
    if !row.allow_training {
        Some("its training consent was withdrawn")
    } else if row.permission_sha256() != recorded {
        Some("its permission (rights assertion) changed")
    } else {
        None
    }
}

/// [`consent_change`] against `contribution`'s permission, then a changed
/// exact input, then a changed label. A digest is computed only when every
/// earlier check passed.
pub(crate) fn contribution_change(
    row: &FeedbackRowV4,
    contribution: &Contribution,
) -> Option<&'static str> {
    consent_change(row, &contribution.permission_sha256).or_else(|| {
        if row.input_sha256() != contribution.input_sha256 {
            Some("its exact input changed")
        } else if row.correct_option_id != contribution.correct_option_id {
            Some("its label changed")
        } else {
            None
        }
    })
}

/// One groups-file row: the group's split and its COMPLETE coverage, sorted
/// by example ID. The train file may hold only new rows plus bounded replay;
/// this coverage still names every contributing example, inherited ones
/// included.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GroupRow {
    pub group_id: String,
    pub split: String,
    pub examples: Vec<Contribution>,
}

/// The store's record of one published dataset, keyed by the SHA-256 of its
/// exact manifest bytes.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DatasetRecord {
    pub dataset_id: String,
    pub parent_manifest_sha256: Option<String>,
}

/// The manifest (schema 4). Written last; binds every file's exact bytes.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub schema: u32,
    pub recipe: String,
    pub workspace_id: String,
    pub dataset_id: String,
    pub parent_manifest_sha256: Option<String>,
    pub model_function_sha256: String,
    pub policy_sha256: String,
    pub base_candidate_sha256: Option<String>,
    pub files: Vec<ManifestFile>,
    pub split_group_counts: BTreeMap<String, usize>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManifestFile {
    pub name: String,
    pub sha256: String,
    pub bytes: u64,
    pub rows: u64,
}

// ---------------------------------------------------------------------------
// Named fault points (test-faults only)
// ---------------------------------------------------------------------------

/// Fault points of the preparation and training boundaries. Release builds
/// carry no hook code and no fault-name strings.
#[cfg(feature = "test-faults")]
pub mod fault_names {
    pub const BEFORE_HISTORY: &str = "ctxfoundry-fault/learning.before_history";
    pub const BEFORE_RENAME: &str = "ctxfoundry-fault/learning.before_rename";
    /// After the rename and the parent fsync, before the dataset is recorded.
    pub const AFTER_PUBLISH: &str = "ctxfoundry-fault/learning.after_publish";
    /// Training: before each update is sent (detail: the step number).
    pub const BEFORE_STEP: &str = "ctxfoundry-fault/learning.before_step";
    /// Training: the candidate's head and manifest writes, flushes and
    /// fsyncs. An armed point fails that operation with `ENOSPC`, exactly
    /// as a full disk would.
    pub const HEAD_WRITE: &str = "ctxfoundry-fault/learning.head_write";
    pub const HEAD_FLUSH: &str = "ctxfoundry-fault/learning.head_flush";
    pub const HEAD_FSYNC: &str = "ctxfoundry-fault/learning.head_fsync";
    pub const MANIFEST_WRITE: &str = "ctxfoundry-fault/learning.manifest_write";
    pub const MANIFEST_FLUSH: &str = "ctxfoundry-fault/learning.manifest_flush";
    pub const MANIFEST_FSYNC: &str = "ctxfoundry-fault/learning.manifest_fsync";
    /// Training: the candidate was published, before its read-back.
    pub const CANDIDATE_PUBLISHED: &str = "ctxfoundry-fault/learning.candidate_published";
    /// Training: a reply was received, before it is judged (detail: its
    /// kind and the milliseconds left before the earliest stop).
    pub const REPLY_RECEIVED: &str = "ctxfoundry-fault/learning.reply_received";
}

#[cfg(feature = "test-faults")]
fn hit_fault(name: &str, control: &Control, detail: &str) -> FResult<()> {
    crate::fault::hit(
        name,
        &crate::fault::Ctx {
            engine: None,
            control: Some(control),
            detail,
        },
    )
}

#[cfg(feature = "test-faults")]
macro_rules! learning_fault {
    ($name:ident, $control:expr, $detail:expr) => {
        $crate::learning::hit_fault($crate::learning::fault_names::$name, $control, $detail)
    };
}

#[cfg(not(feature = "test-faults"))]
macro_rules! learning_fault {
    ($name:ident, $control:expr, $detail:expr) => {
        Ok::<(), $crate::FoundryError>(())
    };
}

/// An I/O fault point: an armed point fails the operation with `ENOSPC`.
#[cfg(feature = "test-faults")]
macro_rules! learning_io_fault {
    ($name:ident, $control:expr, $detail:expr) => {
        $crate::learning::hit_fault($crate::learning::fault_names::$name, $control, $detail)
            .map_err(|_| std::io::Error::from_raw_os_error(libc::ENOSPC))
    };
}

#[cfg(not(feature = "test-faults"))]
macro_rules! learning_io_fault {
    ($name:ident, $control:expr, $detail:expr) => {
        Ok::<(), std::io::Error>(())
    };
}

// T002: fitting, calibration, evaluation and the candidate. Declared after
// the fault macros so they reach these modules.
pub mod candidate;
pub mod eval;
pub mod ipc;
mod prep;
pub mod profile;
#[cfg(target_os = "macos")]
pub mod serve;
#[cfg(target_os = "macos")]
pub mod supervisor;
pub mod train;
#[cfg(target_os = "macos")]
pub mod worker;
use prep::verify_lineage;
pub use prep::{PrepareOutcome, Prepared, prepare};

/// The destination of `learning prepare` and `learning train`, held through
/// descriptors: its parent directory is opened ONCE, and every later
/// create, write, rename, fsync and cleanup is relative to that descriptor,
/// so substituting a path component afterwards cannot redirect the output.
struct OutputTarget {
    parent: Dir,
    name: OsString,
}

impl OutputTarget {
    /// Preflight: the output names a new directory, its parent is not (and
    /// does not lie under) the admitted source root, and the name is free —
    /// unless it may hold this run's own earlier publication whose response
    /// was lost: a directory whose manifest `may_adopt` admits. The caller
    /// adopts it only when it reads back as exactly the output the run
    /// produces.
    fn open(
        engine: &Engine,
        out: &Path,
        may_adopt: &dyn Fn(&[u8]) -> FResult<bool>,
    ) -> FResult<Self> {
        let name = match out.components().next_back() {
            Some(std::path::Component::Normal(name)) => name.to_owned(),
            _ => {
                return Err(FoundryError::InvalidArgument(format!(
                    "output {} must name a new directory",
                    out.display()
                )));
            }
        };
        let parent_path = out
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        let write_error =
            |what: &str, e: std::io::Error| fail("output_write", format!("{what}: {e}"));
        // Containment is judged from the nearest EXISTING ancestor, so an
        // output under a not-yet-created directory of the root is refused
        // as such; nothing above the output is ever created.
        let mut probe =
            std::path::absolute(parent_path).map_err(|e| write_error("output parent", e))?;
        let mut parent_exists = true;
        let resolved = loop {
            match probe.canonicalize() {
                Ok(resolved) => break resolved,
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                    parent_exists = false;
                    probe = match probe.parent() {
                        Some(up) => up.to_path_buf(),
                        None => return Err(write_error("output parent", e)),
                    };
                }
                Err(e) => return Err(write_error("output parent", e)),
            }
        };
        let parent = Dir::open_path(&resolved).map_err(|e| write_error("open output parent", e))?;
        refuse_inside_root(engine, &parent, out)?;
        if !parent_exists {
            return Err(FoundryError::InvalidArgument(format!(
                "output parent {} does not exist",
                parent_path.display()
            )));
        }
        if parent
            .kind_of(&name)
            .map_err(|e| write_error("output destination", e))?
            .is_some()
        {
            let lost_response = match manifest_at(&parent, &name) {
                Some(bytes) => may_adopt(&bytes)?,
                None => false,
            };
            if !lost_response {
                return Err(fail(
                    "output_exists",
                    format!("output destination {} already exists", out.display()),
                ));
            }
        }
        Ok(Self { parent, name })
    }

    /// Create the owned staging directory (`mkdirat` 0700) under the held
    /// parent, hold it, and build the dataset as its child [`STAGED`]: the
    /// publish rename resolves its source through the held staging
    /// descriptor, never through a name in the shared parent.
    fn create_partial(&self) -> FResult<(Dir, PartialGuard<'_>)> {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let name = OsString::from(format!(".learning-partial-{}-{nanos}", std::process::id()));
        let write_error = |e: std::io::Error| fail("output_write", format!("partial output: {e}"));
        let vanished = || std::io::Error::new(std::io::ErrorKind::NotFound, "vanished");
        self.parent.create_dir(&name).map_err(write_error)?;
        let staging = match self.parent.open_dir(&name) {
            Ok(Some(staging)) => staging,
            Ok(None) => return Err(write_error(vanished())),
            Err(e) => return Err(write_error(e)),
        };
        let staged = staging.identity().and_then(|staging_identity| {
            staging.create_dir(STAGED)?;
            let dir = staging.open_dir(STAGED)?.ok_or_else(vanished)?;
            Ok((staging_identity, dir.identity()?, dir))
        });
        match staged {
            Ok((staging_identity, identity, dir)) => Ok((
                dir,
                PartialGuard {
                    parent: &self.parent,
                    name,
                    staging,
                    staging_identity,
                    identity,
                },
            )),
            Err(e) => {
                // Both are empty directories this call just created.
                let _ = staging.remove_dir(STAGED);
                let _ = self.parent.remove_dir(&name);
                Err(write_error(e))
            }
        }
    }

    /// Publish the staged dataset under the destination name WITHOUT
    /// replacing anything that holds the name (left untouched, `Occupied`),
    /// then fsync the parent. The source must still be the directory this
    /// run built immediately before the rename, and the destination must be
    /// that directory immediately after it; otherwise `output_ownership`
    /// and whatever foreign directory is involved stays untouched.
    fn publish(&self, guard: &PartialGuard<'_>) -> FResult<Publication> {
        let ownership = |message: &str| fail("output_ownership", message.to_owned());
        if !guard.staged_is_ours() {
            return Err(ownership(
                "the staged dataset was replaced before publication; nothing was published",
            ));
        }
        match guard
            .staging
            .rename_noreplace_into(STAGED, &self.parent, &self.name)
        {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                return Ok(Publication::Occupied);
            }
            Err(e) => return Err(fail("output_write", format!("publish the output: {e}"))),
        }
        let published = match self.parent.open_dir(&self.name) {
            Ok(Some(dir)) => dir.identity().ok(),
            _ => None,
        };
        if published != Some(guard.identity) {
            return Err(ownership(
                "the published directory is not the one this run built; it was left untouched",
            ));
        }
        guard.remove_staging();
        self.sync_parent()?;
        Ok(Publication::Published)
    }

    fn sync_parent(&self) -> FResult<()> {
        self.parent
            .sync_all()
            .map_err(|e| fail("output_write", format!("fsync the output parent: {e}")))
    }
}

/// The output (a dataset or a candidate) is built as this child of the
/// held staging directory.
const STAGED: &str = "dataset";

/// What publication found at the destination name.
enum Publication {
    /// The partial is now the destination.
    Published,
    /// Something already holds the name; the partial is still ours.
    Occupied,
}

/// The exact `manifest.json` bytes of the directory `name` under `parent`,
/// opened without following a link at either level and bounded before it is
/// read; `None` for anything else (absent, a symlink, not a directory, no
/// regular manifest, oversized or unreadable).
fn manifest_at(parent: &Dir, name: &OsString) -> Option<Vec<u8>> {
    let dir = parent.open_dir(name).ok()??;
    let file = dir.open_file("manifest.json").ok()?;
    read_capped(file, MANIFEST_MAX_BYTES as u64).ok()?
}

/// Refuse an output whose held parent IS, or lies under, the admitted
/// source root. The walk follows `..` through descriptors from the
/// directory actually opened and compares `(st_dev, st_ino)` with the
/// root's, so relative paths, `..` components and symlinked parents cannot
/// hide the containment.
pub(crate) fn refuse_inside_root(engine: &Engine, held: &Dir, out: &Path) -> FResult<()> {
    use std::os::unix::fs::MetadataExt as _;
    let Some(root) = engine.workspace_root() else {
        return Ok(());
    };
    let root_identity = match std::fs::metadata(root) {
        Ok(meta) => (meta.dev(), meta.ino()),
        // A root that no longer exists contains nothing.
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(fail("output_write", format!("admitted source root: {e}"))),
    };
    let walk_error =
        |e: std::io::Error| fail("output_write", format!("walk the output parent: {e}"));
    let mut current = held.try_clone().map_err(walk_error)?;
    // `..` of `/` is `/`; the bound only guards a filesystem that never
    // reaches its root.
    for _ in 0..4096 {
        let identity = current.identity().map_err(walk_error)?;
        if identity == root_identity {
            return Err(fail(
                "output_in_source_root",
                format!(
                    "output {} is under the admitted source root {root}",
                    out.display()
                ),
            ));
        }
        let up = current.open_parent().map_err(walk_error)?;
        if up.identity().map_err(walk_error)? == identity {
            return Ok(());
        }
        current = up;
    }
    Err(fail(
        "output_write",
        "the output parent never reaches the filesystem root",
    ))
}

/// The run's staging directory, held by descriptor, and the identities of
/// it and of the staged dataset. Cleanup removes ONLY what this run built:
/// the staged dataset while it is still the same directory (it is gone once
/// published), then the staging directory while its name still names it and
/// it is empty. A foreign directory or foreign contents are never removed.
struct PartialGuard<'a> {
    parent: &'a Dir,
    name: OsString,
    staging: Dir,
    staging_identity: (u64, u64),
    identity: (u64, u64),
}

impl PartialGuard<'_> {
    fn staged_is_ours(&self) -> bool {
        matches!(
            self.staging.open_dir(STAGED),
            Ok(Some(dir)) if dir.identity().is_ok_and(|identity| identity == self.identity)
        )
    }

    fn remove_staging(&self) {
        let ours = matches!(
            self.parent.open_dir(&self.name),
            Ok(Some(dir)) if dir.identity().is_ok_and(|identity| identity == self.staging_identity)
        );
        if ours {
            let _ = self.parent.remove_dir(&self.name);
        }
    }
}

impl Drop for PartialGuard<'_> {
    fn drop(&mut self) {
        if self.staged_is_ours() {
            let _ = self.staging.remove_tree(STAGED);
        }
        self.remove_staging();
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

/// The policy's model function: its tokenizer (as loaded and verified) and
/// its checkpoint pin.
fn model_function_of(policy: &LearningPolicy, loaded: &LoadedRenderer) -> String {
    decision_model::model_function_sha256(
        &loaded.json_sha,
        &loaded.config_sha,
        loaded.special,
        &policy.model,
    )
}

/// The loaded renderer plus its pinned file hashes.
#[cfg(feature = "semantic")]
pub struct LoadedRenderer {
    pub renderer: decision_model::Renderer,
    pub json_sha: String,
    pub config_sha: String,
    pub special: SpecialIds,
}

#[cfg(feature = "semantic")]
fn load_renderer(policy: &LearningPolicy) -> FResult<LoadedRenderer> {
    load_pinned_renderer(&policy.tokenizer)
}

/// Load the renderer from a pinned tokenizer directory: both files opened
/// without following a link and bounded, then their hashes against the
/// pin (`tokenizer_mismatch`). 013 T003 serving loads the selected
/// candidate's tokenizer through this same path.
#[cfg(feature = "semantic")]
pub(crate) fn load_pinned_renderer(pin: &TokenizerPin) -> FResult<LoadedRenderer> {
    let dir = &pin.dir;
    // Opened without following a link and refused by length before reading.
    let read = |name: &str, cap: u64| -> FResult<Vec<u8>> {
        let path = dir.join(name);
        let unavailable = |e: std::io::Error| {
            FoundryError::ArtifactUnavailable(format!("{}: {e}", path.display()))
        };
        read_capped(open_regular(&path).map_err(unavailable)?, cap)
            .map_err(unavailable)?
            .ok_or_else(|| {
                fail(
                    "tokenizer_invalid",
                    format!("{} is over the {cap}-byte cap", path.display()),
                )
            })
    };
    let json = read("tokenizer.json", 64 * 1024 * 1024)?;
    let config = read("tokenizer_config.json", 64 * 1024)?;
    let json_sha = crate::digest(&json);
    let config_sha = crate::digest(&config);
    if json_sha != pin.json_sha256 || config_sha != pin.config_sha256 {
        return Err(fail(
            "tokenizer_mismatch",
            "tokenizer files do not match the policy's pinned hashes",
        ));
    }
    let renderer = decision_model::Renderer::load(&json, SpecialIds::PINNED)?;
    Ok(LoadedRenderer {
        renderer,
        json_sha,
        config_sha,
        special: SpecialIds::PINNED,
    })
}

/// Without the `semantic` feature there is no tokenizer: the renderer type
/// is uninhabited and loading always fails with a named code, so the rest of
/// preparation compiles unchanged and nothing can render.
#[cfg(not(feature = "semantic"))]
pub enum NoRenderer {}

#[cfg(not(feature = "semantic"))]
impl NoRenderer {
    pub fn render(&self, _state: &str, _ordered: [&str; 2]) -> FResult<decision_model::Rendered> {
        match *self {}
    }
}

#[cfg(not(feature = "semantic"))]
pub struct LoadedRenderer {
    pub renderer: NoRenderer,
    pub json_sha: String,
    pub config_sha: String,
    pub special: SpecialIds,
}

#[cfg(not(feature = "semantic"))]
fn load_renderer(_policy: &LearningPolicy) -> FResult<LoadedRenderer> {
    Err(fail(
        "learning_unavailable",
        "this build has no learning renderer; rebuild with the `semantic` feature",
    ))
}

// ---------------------------------------------------------------------------
// Read-back check
// ---------------------------------------------------------------------------

/// A dataset whose manifest was read through its held directory; no member
/// has been read yet.
struct OpenedDataset {
    dir: Dir,
    manifest: Manifest,
    manifest_sha256: String,
}

/// A dataset read back completely: every member byte matched its manifest
/// entry, every row matched the exact renderer and its group's coverage, and
/// the coverage met the lifecycle floors. Rows are verified as they stream;
/// only metadata is kept.
struct VerifiedDataset {
    manifest: Manifest,
    manifest_sha256: String,
    /// Example ID → (group, contribution): the COMPLETE coverage, inherited
    /// contributions the bounded train file omits included.
    coverage: BTreeMap<String, (String, Contribution)>,
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
fn open_dataset(manifest_path: &Path) -> FResult<OpenedDataset> {
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
fn verify_dataset_with(
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
