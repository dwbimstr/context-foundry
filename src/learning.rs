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

// ---------------------------------------------------------------------------
// Strict row parsing
// ---------------------------------------------------------------------------

/// The v4 feedback row (contract § Feedback and permission). Stored under
/// its example ID; a correction replaces label/consent transactionally and
/// the same input is idempotent.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FeedbackRowV4 {
    pub task_id: String,
    pub task_group_id: String,
    pub family: String,
    pub state: String,
    pub option_ids: Vec<String>,
    pub correct_option_id: String,
    pub label_source: String,
    pub label_evidence: String,
    pub rights_ref: String,
    pub allow_training: bool,
}

impl FeedbackRowV4 {
    /// Parse one row strictly: bounded, no duplicate JSON keys, no unknown
    /// or null fields, exact family/options/label-source, byte limits.
    /// Rejects BEFORE any mutation.
    pub fn parse(raw: &str) -> FResult<Self> {
        if raw.len() > WHOLE_ROW_MAX_BYTES {
            return Err(fail(
                "row_invalid",
                format!(
                    "row is {} bytes; the limit is {WHOLE_ROW_MAX_BYTES}",
                    raw.len()
                ),
            ));
        }
        let row: FeedbackRowV4 = strict_json(raw.as_bytes(), "row_invalid", "v4 feedback row")?;
        row.validate()?;
        Ok(row)
    }

    pub(crate) fn validate(&self) -> FResult<()> {
        let id = |name: &str, value: &str| -> FResult<()> {
            if value.trim().is_empty() || value.len() > 256 {
                return Err(fail(
                    "row_invalid",
                    format!("{name} must be nonblank and at most 256 UTF-8 bytes"),
                ));
            }
            Ok(())
        };
        id("task_id", &self.task_id)?;
        id("task_group_id", &self.task_group_id)?;
        if self.family != FAMILY {
            return Err(fail(
                "row_invalid",
                format!(
                    "unknown family {:?}; only {FAMILY} is supported",
                    self.family
                ),
            ));
        }
        if self.state.is_empty() || self.state.len() > STATE_MAX_BYTES {
            return Err(fail(
                "row_invalid",
                format!(
                    "state must be nonempty and at most {STATE_MAX_BYTES} bytes before tokenization"
                ),
            ));
        }
        // Exactly the two stable options: neither omitted, nor duplicated.
        if self.option_ids.len() != 2 || self.option_ids[0] == self.option_ids[1] {
            return Err(fail(
                "row_invalid",
                "option_ids must be the two distinct stable option ids in order",
            ));
        }
        for option in &self.option_ids {
            if !decision_model::OPTIONS.iter().any(|def| def.id == *option) {
                return Err(fail(
                    "row_invalid",
                    format!("unknown option id {option:?} for family {FAMILY}"),
                ));
            }
        }
        if !self.option_ids.contains(&self.correct_option_id) {
            return Err(fail(
                "row_invalid",
                "correct_option_id must be one of option_ids (labels map by stable id)",
            ));
        }
        if !["operator", "task_checker"].contains(&self.label_source.as_str()) {
            return Err(fail(
                "row_invalid",
                "label_source must be operator or task_checker",
            ));
        }
        for (name, value) in [
            ("label_evidence", &self.label_evidence),
            ("rights_ref", &self.rights_ref),
        ] {
            if value.trim().is_empty() || value.len() > 1024 {
                return Err(fail(
                    "row_invalid",
                    format!("{name} must be nonblank and at most 1024 UTF-8 bytes"),
                ));
            }
        }
        let encoded = serde_json::to_string(self).map_err(FoundryError::from)?;
        if encoded.len() > WHOLE_ROW_MAX_BYTES {
            return Err(fail(
                "row_invalid",
                format!(
                    "whole row is {} bytes; the limit is {WHOLE_ROW_MAX_BYTES}",
                    encoded.len()
                ),
            ));
        }
        Ok(())
    }

    /// The ordered option pair for rendering.
    pub fn ordered_options(&self) -> [&str; 2] {
        [self.option_ids[0].as_str(), self.option_ids[1].as_str()]
    }

    /// `input_sha256` over the ORIGINAL state and the ordered option IDs.
    pub fn input_sha256(&self) -> String {
        decision_model::input_sha256(&self.state, self.ordered_options())
    }

    /// Example ID: `SHA256(compact JSON [task_id,input_sha256])`.
    pub fn example_id(&self) -> String {
        compact_digest(&serde_json::json!([self.task_id, self.input_sha256()]))
    }

    /// Permission identity: `SHA256(compact JSON [allow_training,
    /// rights_ref])`. A changed rights assertion is a changed permission even
    /// when consent, input and label stay the same.
    pub fn permission_sha256(&self) -> String {
        compact_digest(&serde_json::json!([self.allow_training, self.rights_ref]))
    }

    /// Duplicate fingerprint: SHA256 of the family, whitespace-normalized
    /// state and options sorted by stable ID. CRLF becomes LF, runs of ASCII
    /// whitespace collapse to one space and the ends trim; no case folding.
    pub fn fingerprint(&self) -> String {
        let normalized = self.state.replace("\r\n", "\n");
        let mut collapsed = String::with_capacity(normalized.len());
        let mut pending_space = false;
        for ch in normalized.chars() {
            if ch.is_ascii_whitespace() {
                pending_space = true;
            } else {
                if pending_space && !collapsed.is_empty() {
                    collapsed.push(' ');
                }
                pending_space = false;
                collapsed.push(ch);
            }
        }
        let mut options: Vec<&str> = self.option_ids.iter().map(String::as_str).collect();
        options.sort_unstable();
        compact_digest(&serde_json::json!([FAMILY, collapsed, options]))
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
// Strict JSON
// ---------------------------------------------------------------------------

/// One JSON value read through serde_json's own parser, so malformed syntax
/// fails at once. A duplicate object key at ANY depth is refused (serde
/// would otherwise keep the last one silently) and nesting is bounded.
struct StrictValue {
    depth: usize,
}

impl StrictValue {
    fn nested<E: serde::de::Error>(&self) -> Result<StrictValue, E> {
        if self.depth >= JSON_MAX_DEPTH {
            return Err(E::custom("JSON nests too deep"));
        }
        Ok(StrictValue {
            depth: self.depth + 1,
        })
    }
}

impl<'de> serde::de::DeserializeSeed<'de> for StrictValue {
    type Value = serde_json::Value;

    fn deserialize<D: serde::Deserializer<'de>>(
        self,
        deserializer: D,
    ) -> Result<Self::Value, D::Error> {
        deserializer.deserialize_any(self)
    }
}

impl<'de> serde::de::Visitor<'de> for StrictValue {
    type Value = serde_json::Value;

    fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("a JSON value")
    }

    fn visit_unit<E>(self) -> Result<Self::Value, E> {
        Ok(serde_json::Value::Null)
    }

    fn visit_bool<E>(self, value: bool) -> Result<Self::Value, E> {
        Ok(value.into())
    }

    fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E> {
        Ok(value.into())
    }

    fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E> {
        Ok(value.into())
    }

    fn visit_f64<E: serde::de::Error>(self, value: f64) -> Result<Self::Value, E> {
        serde_json::Number::from_f64(value)
            .map(serde_json::Value::Number)
            .ok_or_else(|| E::custom("JSON number is not finite"))
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E> {
        Ok(value.into())
    }

    fn visit_string<E>(self, value: String) -> Result<Self::Value, E> {
        Ok(value.into())
    }

    fn visit_seq<A: serde::de::SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
        let mut items = Vec::new();
        while let Some(item) = seq.next_element_seed(self.nested::<A::Error>()?)? {
            items.push(item);
        }
        Ok(serde_json::Value::Array(items))
    }

    fn visit_map<A: serde::de::MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
        use serde::de::Error as _;
        let mut object = serde_json::Map::new();
        while let Some(key) = map.next_key::<String>()? {
            if object.contains_key(&key) {
                return Err(A::Error::custom(format!("duplicate JSON key {key:?}")));
            }
            let value = map.next_value_seed(self.nested::<A::Error>()?)?;
            object.insert(key, value);
        }
        Ok(serde_json::Value::Object(object))
    }
}

/// Parse `bytes` strictly into `T`: serde_json's parser through
/// [`StrictValue`] (no duplicate keys, bounded depth, nothing trailing),
/// then the typed struct, whose `deny_unknown_fields` and non-optional
/// fields refuse unknown and null members. Every failure is `code`.
pub(crate) fn strict_json<T: serde::de::DeserializeOwned>(
    bytes: &[u8],
    code: &'static str,
    what: &str,
) -> FResult<T> {
    use serde::de::DeserializeSeed as _;
    let mut deserializer = serde_json::Deserializer::from_slice(bytes);
    let value = StrictValue { depth: 0 }
        .deserialize(&mut deserializer)
        .and_then(|value| deserializer.end().map(|()| value))
        .map_err(|e| fail(code, format!("{what} is not strict JSON: {e}")))?;
    serde_json::from_value(value).map_err(|e| fail(code, format!("{what}: {e}")))
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
// State composition (contract § Exact input and identity)
// ---------------------------------------------------------------------------

/// How many lexical locator lines the state carries.
pub const STATE_LOCATORS: usize = 3;

/// The `graph:` coverage word of the current compiler graph, judged against
/// the ONE source revision the ranking was computed at.
///
/// * No producer has a selected snapshot: `graph_unavailable`.
/// * Selected snapshots exist but none is for `revision` (every compiler fact
///   is ineligible): `graph_stale`.
/// * `complete` only when EVERY selected producer is current, its snapshot
///   state is `Complete` and the report for that exact snapshot says its
///   aggregate import coverage is `complete`; otherwise `partial` (an
///   unresolved, unknown, failed or interrupted import, a stale sibling
///   producer, or no matching report - the graph never certifies more than
///   it proved).
fn graph_coverage(
    producers: &[(String, crate::graph::ProducerRow)],
    revision: u64,
) -> FResult<&'static str> {
    let selected: Vec<_> = producers
        .iter()
        .filter_map(|(_, row)| row.selected.as_ref().map(|s| (s, row.latest.as_ref())))
        .collect();
    if selected.is_empty() {
        return Err(fail(
            "graph_unavailable",
            "no compiler graph snapshot is selected for this store",
        ));
    }
    if selected
        .iter()
        .all(|(snapshot, _)| snapshot.tuple.source_revision != revision)
    {
        return Err(fail(
            "graph_stale",
            "every selected compiler graph snapshot predates the current source revision",
        ));
    }
    let complete = selected.iter().all(|(snapshot, latest)| {
        snapshot.tuple.source_revision == revision
            && snapshot.state == crate::graph::SnapshotState::Complete
            && latest.is_some_and(|report| {
                report.snapshot_id == snapshot.tuple.snapshot_id && report.coverage == "complete"
            })
    });
    Ok(if complete { "complete" } else { "partial" })
}

/// The composed state is held to the same 16 KiB pre-tokenization guard as a
/// stored row: refused with a named code, never truncated.
fn guard_state_size(state: String) -> FResult<String> {
    if state.len() > STATE_MAX_BYTES {
        return Err(fail(
            "state_too_large",
            format!(
                "the composed state is {} bytes; the pre-tokenization guard is {STATE_MAX_BYTES}",
                state.len()
            ),
        ));
    }
    Ok(state)
}

impl Engine {
    /// The core-composed `state` of the `retrieval-route-v1` family: the
    /// admitted query text verbatim; LF; `graph: <complete|partial>` from the
    /// current selected compiler graph; then up to [`STATE_LOCATORS`] LF-led
    /// locator lines `<path> <kind> <qualified name>` naming the top delivery
    /// units of 001's two-tier lexical ranking for the same query (a block or
    /// unnamed unit renders its kind alone, as in v2 labels). No handles, no
    /// source text, no trailing LF.
    ///
    /// This is the ONE composer: row creation (`learning compose-state`) and
    /// inference (013 T003) both call it, so a row stores exactly the state
    /// the policy would see. The coverage line is judged at the revision the
    /// ranking was computed at, so both halves describe one snapshot.
    ///
    /// Refusals, all before any model call (the caller then uses
    /// deterministic routing): `graph_unavailable`, `graph_stale` and
    /// `state_too_large` (the composed state is over the 16 KiB
    /// pre-tokenization guard - never truncated), plus the lexical search's
    /// own refusals for a blank or oversized query and an unusable index.
    pub fn compose_route_state(&self, query: &str, control: &Control) -> FResult<String> {
        control.check()?;
        let batch = self.search_candidates(query, None, STATE_LOCATORS, control)?;
        let producers = self.compiler_producers()?;
        let coverage = graph_coverage(&producers, batch.freshness.source_revision)?;
        let mut state = format!("{query}\ngraph: {coverage}");
        for item in batch.items.iter().take(STATE_LOCATORS) {
            let Some(handle) = &item.handle else {
                continue;
            };
            // A locator line is data, never structure: the same sanitizer v2
            // output applies to labels turns every ASCII control character
            // except TAB into `?`, so indexed text (a Markdown heading that
            // decodes to a line break, say) cannot forge a `graph:` line or
            // a locator.
            state.push('\n');
            state.push_str(&crate::response::single_line(&handle.path));
            state.push(' ');
            state.push_str(&crate::response::single_line(&item.label));
        }
        guard_state_size(state)
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
// Bounded input files
// ---------------------------------------------------------------------------

/// Open one operator-named input file (policy, tokenizer) for reading
/// without following a final-component symlink or blocking on a FIFO;
/// anything but a regular file is refused.
fn open_regular(path: &Path) -> std::io::Result<File> {
    use std::os::unix::fs::OpenOptionsExt as _;
    let file = std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)?;
    if !file.metadata()?.is_file() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "not a regular file",
        ));
    }
    Ok(file)
}

/// Read a whole regular file of at most `limit` bytes: the descriptor's own
/// length is checked BEFORE anything is read, and the read is capped at
/// `limit + 1` bytes so a file that grows meanwhile is still refused.
/// `None` when the file is over the limit.
fn read_capped(file: File, limit: u64) -> std::io::Result<Option<Vec<u8>>> {
    if file.metadata()?.len() > limit {
        return Ok(None);
    }
    let mut body = Vec::new();
    file.take(limit + 1).read_to_end(&mut body)?;
    Ok((body.len() as u64 <= limit).then_some(body))
}

// ---------------------------------------------------------------------------
// Policy
// ---------------------------------------------------------------------------

/// The run policy (v2): one file serves `prepare` and `train`. T001 reads
/// recipe, seed and the pinned tokenizer identity; T002 adds the fitting
/// fields (contract § Fitting, artifacts and evaluation, 158-168) and the
/// selection policy (200-202). The dataset manifest binds the policy's
/// exact bytes by `policy_sha256`, and training refuses a dataset prepared
/// under any other policy.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LearningPolicy {
    pub v: u32,
    pub recipe: String,
    /// The seed string ordering base-replay selection, each epoch's training
    /// order and the head dropout.
    pub seed: String,
    pub tokenizer: TokenizerPin,
    /// The checkpoint the model function pins (its location is the isolation
    /// profile's `checkpoint_dir`, which the worker is granted).
    pub model: decision_model::CheckpointPin,
    /// `null`: the pinned initial checkpoint is the base. Otherwise the
    /// SHA-256 of the exact manifest bytes of the accepted candidate that
    /// `train --base` must name.
    pub base: Option<String>,
    pub optimizer: OptimizerPin,
    /// Updates in this round, 1..=1000000; each epoch visits the training
    /// rows in seeded order until this many steps are done.
    pub max_steps: u64,
    /// The wall clock of the whole fitting run, 1..=7200 seconds.
    pub wall_seconds: u64,
    /// The worker's supervised physical-footprint ceiling, 4..=8 GiB.
    pub memory_bytes: u64,
    /// The worker's output ceiling (per file and in total), 128 MiB..=2 GiB.
    pub output_bytes: u64,
    /// LibTorch intra-op threads, 1..=4.
    pub cpu_threads: u32,
    /// Absolute path of the learning-worker isolation profile.
    pub isolation_profile: PathBuf,
    /// Requested enforcement per resource; never downgraded.
    pub enforcement: Enforcement,
    #[serde(default)]
    pub selection: SelectionPolicy,
}

/// The recipe's optimizer, stated in the policy; it must equal
/// [`decision_model::recipe`] exactly.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OptimizerPin {
    pub name: String,
    pub learning_rate: f64,
    pub beta1: f64,
    pub beta2: f64,
    pub epsilon: f64,
    pub weight_decay: f64,
    pub clip_global_norm: f64,
}

impl OptimizerPin {
    /// The recipe's constants.
    pub fn recipe() -> Self {
        use decision_model::recipe::*;
        Self {
            name: OPTIMIZER.to_owned(),
            learning_rate: LEARNING_RATE,
            beta1: BETA1,
            beta2: BETA2,
            epsilon: EPSILON,
            weight_decay: WEIGHT_DECAY,
            clip_global_norm: CLIP_GLOBAL_NORM,
        }
    }
}

/// An enforcement level. `hard` is a kernel limit the worker cannot exceed;
/// `supervised` is the owner measuring and stopping the worker. A request
/// is satisfied only by the level requested or a stronger one.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Level {
    Supervised,
    Hard,
}

/// Requested enforcement per resource (contract 167-169).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Enforcement {
    pub memory: Level,
    pub cpu: Level,
    pub output: Level,
    pub process_count: Level,
}

/// The predeclared selection policy (contract 200-202): fractions finite
/// in [0, 1]; defaults for the initial trial 0.8 / 0.5 / 0.9 / 0 and no
/// critical group.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct SelectionPolicy {
    /// Accept a prediction exactly when its unrounded answer confidence is
    /// at least this.
    pub threshold: f64,
    /// Accepted rows over all evaluation rows must reach this.
    pub coverage_floor: f64,
    /// Correct accepted rows over accepted rows must reach this.
    pub accepted_accuracy_floor: f64,
    /// The candidate's macro accuracy may trail either comparator by at
    /// most this (also per critical group).
    pub max_macro_accuracy_drop: f64,
    /// Evaluation group IDs that must each be present and must not regress
    /// past the drop allowance.
    pub critical_groups: Vec<String>,
}

impl Default for SelectionPolicy {
    fn default() -> Self {
        Self {
            threshold: 0.8,
            coverage_floor: 0.5,
            accepted_accuracy_floor: 0.9,
            max_macro_accuracy_drop: 0.0,
            critical_groups: Vec::new(),
        }
    }
}

/// Policy bounds (contract 163, 167-168).
pub const MAX_STEPS: u64 = 1_000_000;
pub const MAX_WALL_SECONDS: u64 = 7200;
pub const MIN_MEMORY_BYTES: u64 = 4 << 30;
pub const MAX_MEMORY_BYTES: u64 = 8 << 30;
pub const MIN_OUTPUT_BYTES: u64 = 128 << 20;
pub const MAX_OUTPUT_BYTES: u64 = 2 << 30;
pub const MAX_CPU_THREADS: u32 = 4;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TokenizerPin {
    /// Directory holding `tokenizer.json` and `tokenizer_config.json`.
    pub dir: PathBuf,
    pub json_sha256: String,
    pub config_sha256: String,
}

pub(crate) fn hex64(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

fn fraction(value: f64) -> bool {
    value.is_finite() && (0.0..=1.0).contains(&value)
}

impl LearningPolicy {
    /// Strict load plus the exact-bytes digest the manifest binds. The file
    /// is opened without following a link and refused by its length before
    /// it is read.
    pub fn load(path: &Path) -> FResult<(Self, String)> {
        let unavailable = |e: std::io::Error| {
            FoundryError::ArtifactUnavailable(format!("policy {}: {e}", path.display()))
        };
        let raw = read_capped(open_regular(path).map_err(unavailable)?, POLICY_MAX_BYTES)
            .map_err(unavailable)?
            .ok_or_else(|| fail("policy_invalid", "policy exceeds 64 KiB"))?;
        let policy: LearningPolicy = strict_json(&raw, "policy_invalid", "policy")?;
        if policy.v != 2 {
            return Err(fail(
                "policy_invalid",
                format!("policy v must be 2, not {}", policy.v),
            ));
        }
        if policy.recipe != RECIPE {
            return Err(fail(
                "policy_invalid",
                format!("unknown recipe {:?}; expected {RECIPE}", policy.recipe),
            ));
        }
        if policy.seed.trim().is_empty() || policy.seed.len() > 256 {
            return Err(fail(
                "policy_invalid",
                "seed must be nonblank and at most 256 bytes",
            ));
        }
        for (name, hash) in [
            ("json_sha256", &policy.tokenizer.json_sha256),
            ("config_sha256", &policy.tokenizer.config_sha256),
        ] {
            if !hex64(hash) {
                return Err(fail(
                    "policy_invalid",
                    format!("tokenizer.{name} must be 64 lowercase hex characters"),
                ));
            }
        }
        policy.validate_fitting()?;
        Ok((policy, crate::digest(&raw)))
    }

    /// The T002 fields: exact checkpoint pins, the recipe's optimizer, every
    /// bound (refused, never clamped), an absolute profile path and the
    /// selection fractions.
    fn validate_fitting(&self) -> FResult<()> {
        let invalid = |message: String| Err(fail("policy_invalid", message));
        for (name, hash) in [
            ("model.weights_sha256", &self.model.weights_sha256),
            (
                "model.encoder_config_sha256",
                &self.model.encoder_config_sha256,
            ),
        ] {
            if !hex64(hash) {
                return invalid(format!("{name} must be 64 lowercase hex characters"));
            }
        }
        if decision_model::safetensors::Dtype::parse(&self.model.source_dtype).is_none() {
            return invalid(format!(
                "model.source_dtype {:?} is not F16, BF16 or F32",
                self.model.source_dtype
            ));
        }
        if let Some(base) = &self.base
            && !hex64(base)
        {
            return invalid("base must be null or a 64-hex candidate manifest SHA-256".into());
        }
        if self.optimizer != OptimizerPin::recipe() {
            return invalid(format!(
                "optimizer must be exactly the {RECIPE} recipe: {:?}",
                OptimizerPin::recipe()
            ));
        }
        let bounded = |name: &str, value: u64, low: u64, high: u64| -> FResult<()> {
            if !(low..=high).contains(&value) {
                return Err(fail(
                    "policy_invalid",
                    format!("{name} is {value}; it must be {low}..={high}"),
                ));
            }
            Ok(())
        };
        bounded("max_steps", self.max_steps, 1, MAX_STEPS)?;
        bounded("wall_seconds", self.wall_seconds, 1, MAX_WALL_SECONDS)?;
        bounded(
            "memory_bytes",
            self.memory_bytes,
            MIN_MEMORY_BYTES,
            MAX_MEMORY_BYTES,
        )?;
        bounded(
            "output_bytes",
            self.output_bytes,
            MIN_OUTPUT_BYTES,
            MAX_OUTPUT_BYTES,
        )?;
        bounded(
            "cpu_threads",
            u64::from(self.cpu_threads),
            1,
            u64::from(MAX_CPU_THREADS),
        )?;
        if !self.isolation_profile.is_absolute()
            || self
                .isolation_profile
                .components()
                .any(|c| matches!(c, std::path::Component::ParentDir))
        {
            return invalid("isolation_profile must be an absolute path without `..`".into());
        }
        let selection = &self.selection;
        for (name, value) in [
            ("threshold", selection.threshold),
            ("coverage_floor", selection.coverage_floor),
            ("accepted_accuracy_floor", selection.accepted_accuracy_floor),
            ("max_macro_accuracy_drop", selection.max_macro_accuracy_drop),
        ] {
            if !fraction(value) {
                return invalid(format!("selection.{name} must be finite in [0, 1]"));
            }
        }
        let mut seen = BTreeSet::new();
        if selection.critical_groups.len() > MAX_GROUPS {
            return invalid("selection.critical_groups lists too many groups".into());
        }
        for group in &selection.critical_groups {
            if group.trim().is_empty() || group.len() > 256 || !seen.insert(group.as_str()) {
                return invalid(
                    "selection.critical_groups must be distinct nonblank ids of at most 256 bytes"
                        .into(),
                );
            }
        }
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

fn encode_jsonl_row<T: Serialize>(row: &T) -> FResult<String> {
    let mut line = serde_json::to_string(row).map_err(FoundryError::from)?;
    line.push('\n');
    if line.len() > ROW_MAX_BYTES {
        return Err(fail(
            "dataset_bounds",
            format!(
                "a dataset row is {} bytes; the limit is {ROW_MAX_BYTES}",
                line.len()
            ),
        ));
    }
    Ok(line)
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
pub mod profile;
#[cfg(target_os = "macos")]
pub mod supervisor;
pub mod train;
#[cfg(target_os = "macos")]
pub mod worker;

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

    /// Append one compact-JSON row plus LF. `total` is the running size of
    /// every data file, refused (never truncated) past the combined bound.
    fn append<T: Serialize>(&mut self, row: &T, total: &mut u64) -> FResult<()> {
        let line = encode_jsonl_row(row)?;
        *total += line.len() as u64;
        if *total > FILES_MAX_BYTES {
            return Err(fail(
                "dataset_bounds",
                format!("dataset files exceed the {FILES_MAX_BYTES}-byte bound"),
            ));
        }
        self.hasher.update(line.as_bytes());
        self.writer
            .write_all(line.as_bytes())
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
            sha256: format!("{:x}", self.hasher.finalize()),
            bytes: self.bytes,
            rows: self.rows,
        })
    }
}

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
fn refuse_inside_root(engine: &Engine, held: &Dir, out: &Path) -> FResult<()> {
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
fn verify_lineage(engine: &Engine, parent: &VerifiedDataset, control: &Control) -> FResult<()> {
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
        sink.append(&dataset_row, &mut total_bytes)
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

    let file_sha = |name: &str| -> String {
        file_entries
            .iter()
            .find(|f| f.name == name)
            .map(|f| f.sha256.clone())
            .unwrap_or_default()
    };
    let parent_manifest_sha256 = base.as_ref().map(|b| b.manifest_sha256.clone());
    let dataset_id = compact_digest(&serde_json::json!([
        workspace_id,
        parent_manifest_sha256,
        model_function,
        policy_sha256,
        file_sha("train.jsonl"),
        file_sha("calibration.jsonl"),
        file_sha("evaluation.jsonl"),
    ]));
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
    let dir = &policy.tokenizer.dir;
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
    if json_sha != policy.tokenizer.json_sha256 || config_sha != policy.tokenizer.config_sha256 {
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
    let file_sha = |name: &str| -> FResult<String> {
        manifest_entry(&manifest, name).map(|entry| entry.sha256.clone())
    };
    let recomputed = compact_digest(&serde_json::json!([
        manifest.workspace_id,
        manifest.parent_manifest_sha256,
        manifest.model_function_sha256,
        manifest.policy_sha256,
        file_sha("train.jsonl")?,
        file_sha("calibration.jsonl")?,
        file_sha("evaluation.jsonl")?,
    ]));
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
    let declared_groups = manifest_entry(&manifest, "groups.jsonl")?.rows;
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

fn manifest_entry<'a>(manifest: &'a Manifest, name: &str) -> FResult<&'a ManifestFile> {
    manifest
        .files
        .iter()
        .find(|entry| entry.name == name)
        .ok_or_else(|| fail("dataset_invalid", format!("manifest lacks {name}")))
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
        manifest_entry(&manifest, "groups.jsonl")?,
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
        stream_member(&dir, manifest_entry(&manifest, &name)?, control, |line| {
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
    if format!("{:x}", hasher.finalize()) != entry.sha256 {
        return Err(fail(
            "dataset_invalid",
            format!("{name} does not match the manifest hash"),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::{
        ProducerIdentity, ProducerRow, SelectedSnapshot, SnapshotState, SnapshotTuple,
    };
    use crate::scip::ImportReport;

    fn identity() -> ProducerIdentity {
        ProducerIdentity {
            name: "rust-analyzer".into(),
            release_tag: "2026-08-31".into(),
            commit: "f8996691e991a4dc3c6f135e0fc04fc5561e4e9a".into(),
            version_output: "test-producer 1.0".into(),
            binary_sha256: "0".repeat(64),
        }
    }

    /// A producer row selected at `revision`, in `state`, whose latest report
    /// is for the same snapshot with aggregate `coverage` (or no report).
    fn row(revision: u64, state: SnapshotState, report: Option<&str>) -> (String, ProducerRow) {
        let tuple = SnapshotTuple::new(
            identity(),
            "invocation",
            "config",
            "a".repeat(64),
            "b".repeat(64),
            revision,
        );
        let latest = report.map(|coverage| ImportReport {
            snapshot_id: tuple.snapshot_id.clone(),
            coverage: coverage.to_owned(),
            ..ImportReport::default()
        });
        (
            "rust-analyzer".into(),
            ProducerRow {
                selected: Some(SelectedSnapshot { tuple, state }),
                latest,
            },
        )
    }

    fn code(result: FResult<&'static str>) -> String {
        result.map_or_else(|e| e.code().to_owned(), str::to_owned)
    }

    #[test]
    fn no_selected_snapshot_is_graph_unavailable() {
        assert_eq!(code(graph_coverage(&[], 7)), "graph_unavailable");
        // A producer row that never selected a snapshot is not a graph.
        let unselected = ("rust-analyzer".to_owned(), ProducerRow::default());
        assert_eq!(code(graph_coverage(&[unselected], 7)), "graph_unavailable");
    }

    #[test]
    fn every_snapshot_predating_the_revision_is_graph_stale() {
        let rows = [row(6, SnapshotState::Complete, Some("complete"))];
        assert_eq!(code(graph_coverage(&rows, 7)), "graph_stale");
        // A snapshot for a NEWER revision than the ranking's is also not current.
        let rows = [row(8, SnapshotState::Complete, Some("complete"))];
        assert_eq!(code(graph_coverage(&rows, 7)), "graph_stale");
    }

    #[test]
    fn complete_requires_a_complete_snapshot_and_a_complete_matching_report() {
        let rows = [row(7, SnapshotState::Complete, Some("complete"))];
        assert_eq!(code(graph_coverage(&rows, 7)), "complete");
    }

    #[test]
    fn anything_short_of_proven_complete_is_partial() {
        for (name, rows) in [
            (
                "report says partial",
                vec![row(7, SnapshotState::Complete, Some("partial"))],
            ),
            (
                "snapshot partial",
                vec![row(7, SnapshotState::Partial, Some("complete"))],
            ),
            (
                "snapshot still importing",
                vec![row(7, SnapshotState::Importing, Some("complete"))],
            ),
            (
                "no report at all",
                vec![row(7, SnapshotState::Complete, None)],
            ),
        ] {
            assert_eq!(code(graph_coverage(&rows, 7)), "partial", "{name}");
        }
        // A report for a DIFFERENT snapshot proves nothing about the selected one.
        let (name, mut mismatched) = row(7, SnapshotState::Complete, Some("complete"));
        mismatched.latest.as_mut().unwrap().snapshot_id = "f".repeat(64);
        assert_eq!(code(graph_coverage(&[(name, mismatched)], 7)), "partial");
    }

    #[test]
    fn the_worst_producer_decides_and_a_stale_sibling_is_partial() {
        let complete = row(7, SnapshotState::Complete, Some("complete"));
        let partial = row(7, SnapshotState::Complete, Some("partial"));
        assert_eq!(
            code(graph_coverage(&[complete.clone(), partial], 7)),
            "partial"
        );
        // One current producer plus one that predates the revision: the
        // stale producer's facts are ineligible, so the graph is incomplete.
        let stale = row(6, SnapshotState::Complete, Some("complete"));
        assert_eq!(code(graph_coverage(&[complete, stale], 7)), "partial");
    }

    #[test]
    fn locator_text_cannot_forge_structure() {
        assert_eq!(
            crate::response::single_line("alpha\ngraph: complete\r\u{0}\u{1b}tail\tkept"),
            "alpha?graph: complete???tail\tkept"
        );
        // Non-ASCII text, including Unicode line separators, is untouched:
        // the state's only line separator is LF.
        assert_eq!(
            crate::response::single_line("café 日本語 \u{2028}"),
            "café 日本語 \u{2028}"
        );
    }

    #[test]
    fn the_composed_state_is_refused_not_truncated_past_16_kib() {
        let exactly = "x".repeat(STATE_MAX_BYTES);
        assert_eq!(guard_state_size(exactly.clone()).unwrap(), exactly);
        let over = "x".repeat(STATE_MAX_BYTES + 1);
        assert_eq!(
            guard_state_size(over).unwrap_err().code(),
            "state_too_large"
        );
    }
}
