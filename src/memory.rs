//! 008 explicit project memory: attributed records the caller creates,
//! corrects, retrieves, exports and forgets. No automatic ingestion, no
//! transcripts, no model reads or writes; every record comes from an explicit
//! `put`/`update` and dies only by an explicit `forget`.
//!
//! One `memory` table lives in the authoritative store beside sources. The
//! row, the store-owned revision counter and a typed pending-index key
//! (`memory:<id>`; source work uses `source:<path>`) commit in ONE
//! transaction, so a crash leaves the old row or the new row, never a
//! half-state. Derived search documents are rebuilt by the shared drain and
//! repair paths and are validated against the live row (same id AND same
//! revision) before any delivery, so a forgotten or superseded record cannot
//! leak through a stale index.
//!
//! Multi-root owners (007): memory lives only in the primary (writable)
//! store. `workspace_id` must equal the primary root's bound ID; a reference
//! root's ID or any other value is `wrong_workspace`, and `include_memory`
//! reads only the primary. This limit is deliberate: reference stores are
//! read-only for this owner.
use crate::Engine;
use crate::error::{FResult, FoundryError};
use crate::response::Freshness;
use crate::store::{self, META, SOURCES, check_token_budget};
use redb::{ReadableDatabase, ReadableTable, ReadableTableMetadata, TableDefinition};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::ops::Bound;

/// The authoritative memory table: key = record id, value = record JSON.
pub(crate) const MEMORY: TableDefinition<&str, &str> = TableDefinition::new("memory");

/// Text bound: nonblank UTF-8 up to 16 KiB.
pub(crate) const MAX_TEXT_BYTES: usize = 16 * 1024;
/// Author bound in bytes.
pub(crate) const MAX_AUTHOR_BYTES: usize = 256;
/// Provenance bound: nonblank caller attribution up to 1024 bytes.
pub(crate) const MAX_PROVENANCE_BYTES: usize = 1024;
/// Source links per record: 0..=8 full v2 source handles.
pub(crate) const MAX_LINKS: usize = 8;
/// Record id bound and charset `[A-Za-z0-9_-]{1,128}`.
pub(crate) const MAX_ID_BYTES: usize = 128;
/// Per-field bounds sum to well under this, so every parsed request is too:
/// 8 handles (33 600) + text (16 384) + provenance (1024) + author (256).
pub(crate) const MAX_REQUEST_BYTES: usize = 64 * 1024;
/// Export page caps: at most 128 rows and 4 MiB of JSONL per page.
pub(crate) const EXPORT_PAGE_ROWS: usize = 128;
pub(crate) const EXPORT_PAGE_BYTES: usize = 4 * 1024 * 1024;
/// A compact `mem:` line's first line is cut at a UTF-8 boundary to this.
pub(crate) const MEM_FIRST_LINE_BYTES: usize = 120;
/// `context {include_memory:true}` draws at most this many memory hits.
pub(crate) const CONTEXT_MEMORY_HITS: usize = 10;

/// The typed pending-index key of one source path. macOS allows `:` in file
/// names, so a raw path key could collide with `memory:<id>`; the prefixes
/// keep the namespaces disjoint (`source:memory:x` vs `memory:x`).
pub(crate) fn source_pending_key(path: &str) -> String {
    format!("source:{path}")
}

/// The typed pending-index key of one memory record.
pub(crate) fn memory_pending_key(id: &str) -> String {
    format!("memory:{id}")
}

/// One memory row exactly as stored (and as export emits it).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MemoryRecord {
    pub id: String,
    pub workspace_id: String,
    pub revision: u64,
    pub text: String,
    pub author: String,
    pub provenance: String,
    pub source_links: Vec<String>,
}

/// The creation/replacement fields of `put` and `update`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecordFields {
    pub id: String,
    pub text: String,
    pub author: String,
    pub provenance: String,
    pub source_links: Vec<String>,
}

/// Every request names its workspace explicitly (FR-002): a value other than
/// the bound workspace is `wrong_workspace` before any existence check.
#[derive(Clone, Debug)]
pub struct PutInput {
    pub fields: RecordFields,
    pub workspace_id: String,
}

#[derive(Clone, Debug)]
pub struct UpdateInput {
    pub fields: RecordFields,
    pub workspace_id: String,
    pub expected_revision: u64,
}

#[derive(Clone, Debug)]
pub struct ForgetInput {
    pub id: String,
    pub workspace_id: String,
    pub expected_revision: u64,
}

#[derive(Clone, Debug)]
pub struct SearchInput {
    pub query: String,
    pub workspace_id: String,
    pub limit: usize,
    pub tokens: usize,
}

/// One parsed `memory` request, shared by the CLI subcommands and the MCP
/// tool (the CLI injects `op` from the subcommand name).
pub enum MemoryRequest {
    Put(PutInput),
    Update(UpdateInput),
    Get { id: String, workspace_id: String },
    Forget(ForgetInput),
    Search(SearchInput),
}

/// One derived memory search candidate before live-row validation (the
/// document's id and the revision it was indexed at).
pub(crate) struct MemoryCandidate {
    pub id: String,
    pub revision: u64,
}

/// The derived memory candidates of one query, collected before the final
/// read: whether the 256-document selection window filled, and the (id,
/// indexed revision) pairs it produced.
pub(crate) struct MemoryPlan {
    pub window_full: bool,
    pub candidates: Vec<MemoryCandidate>,
}

/// The combined context outcome (008): the source/graph batch plus the
/// memory hits validated in the batch's own final read transaction.
pub struct MemoryContext {
    pub batch: crate::store::CandidateBatch,
    pub hits: Vec<MemoryHit>,
}

/// The result of `put`/`update`: `{id, revision, outcome}` — no record text.
#[derive(Debug, Serialize)]
pub struct PutReport {
    pub id: String,
    pub revision: u64,
    pub outcome: &'static str,
}

/// The three forget outcomes of the content-free report (008 § Inputs and
/// operations). `conflict` and `already_absent` are outcomes of the forget
/// result, not failed commands.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ForgetOutcome {
    Deleted,
    AlreadyAbsent,
    Conflict,
}

/// The content-free forget report: id, outcome, the removed revision when
/// deleted, and counts. Never text, author or provenance.
#[derive(Debug, Serialize)]
pub struct ForgetReport {
    pub id: String,
    pub outcome: ForgetOutcome,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub removed_revision: Option<u64>,
    pub memory_revision: u64,
    pub live_records: u64,
}

/// One source link of a `get` result with its status against the indexed
/// snapshot: `fresh` (same digest), `stale` (source changed) or `missing`
/// (source gone). A changed status never edits the record.
#[derive(Debug, Serialize)]
pub struct LinkStatus {
    pub handle: String,
    pub status: &'static str,
}

/// The full live record of a `get`, with exact text and `kind:"memory"`.
#[derive(Debug, Serialize)]
pub struct MemoryGet {
    pub id: String,
    pub workspace_id: String,
    pub revision: u64,
    pub kind: &'static str,
    pub text: String,
    pub author: String,
    pub provenance: String,
    pub source_links: Vec<LinkStatus>,
}

/// One validated memory hit for rendering: the live row carries the whole
/// text; the compact line cuts its first line at the boundary.
#[derive(Clone, Debug)]
pub struct MemoryHit {
    pub id: String,
    pub revision: u64,
    pub author: String,
    pub text: String,
}

/// The outcome of a memory search: hits validated against one final read,
/// plus the freshness of that read and how many candidates it dropped.
#[derive(Debug)]
pub struct MemorySearchOutcome {
    pub freshness: Freshness,
    pub hits: Vec<MemoryHit>,
    pub stale_candidates: u64,
    /// The 256-document candidate window filled (context-v2 `candidates:full`).
    pub candidates_full: bool,
}

/// One export page: sorted JSONL rows on stdout, counts-only metadata for
/// stderr. Each page is one read snapshot.
pub struct ExportPage {
    pub rows: Vec<String>,
    pub bytes: usize,
    pub next_after_id: Option<String>,
}

const OPS: [&str; 5] = ["put", "update", "get", "forget", "search"];

fn invalid(detail: impl Into<String>) -> FoundryError {
    FoundryError::InvalidArgument(detail.into())
}

fn field_str(map: &Map<String, Value>, key: &str) -> FResult<Option<String>> {
    match map.get(key) {
        None => Ok(None),
        Some(Value::Null) => Err(invalid(format!("field `{key}` must not be null"))),
        Some(Value::String(s)) => Ok(Some(s.clone())),
        Some(_) => Err(invalid(format!("field `{key}` must be a string"))),
    }
}

fn require_str(map: &Map<String, Value>, key: &str) -> FResult<String> {
    field_str(map, key)?.ok_or_else(|| invalid(format!("missing required field `{key}`")))
}

fn require_u64(map: &Map<String, Value>, key: &str) -> FResult<u64> {
    match map.get(key) {
        None | Some(Value::Null) => Err(invalid(format!("missing required field `{key}`"))),
        Some(value) => value
            .as_u64()
            .ok_or_else(|| invalid(format!("field `{key}` must be a nonnegative integer"))),
    }
}

/// Reject any field the op does not take, including contradictory op fields
/// (`put` with `expected_revision`, `update`/`forget` without it).
fn reject_fields(map: &Map<String, Value>, allowed: &[&str]) -> FResult<()> {
    for key in map.keys() {
        if !allowed.contains(&key.as_str()) {
            return Err(invalid(format!(
                "field `{key}` is not valid for this memory op"
            )));
        }
    }
    Ok(())
}

fn parse_id(map: &Map<String, Value>) -> FResult<String> {
    let id = require_str(map, "id")?;
    let valid = !id.is_empty()
        && id.len() <= MAX_ID_BYTES
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-');
    if !valid {
        return Err(invalid("id must match [A-Za-z0-9_-]{1,128}"));
    }
    Ok(id)
}

fn parse_workspace_id(map: &Map<String, Value>) -> FResult<String> {
    let ws = require_str(map, "workspace_id")?;
    if ws.len() != 64 || !ws.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f')) {
        return Err(invalid("workspace_id must be 64 lowercase hex digits"));
    }
    Ok(ws)
}

fn parse_fields(map: &Map<String, Value>) -> FResult<RecordFields> {
    let id = parse_id(map)?;
    let text = require_str(map, "text")?;
    if text.len() > MAX_TEXT_BYTES {
        return Err(invalid(format!("text exceeds {MAX_TEXT_BYTES} bytes")));
    }
    if text.trim().is_empty() {
        return Err(invalid("text must not be blank"));
    }
    let author = require_str(map, "author")?;
    if author.is_empty() || author.len() > MAX_AUTHOR_BYTES {
        return Err(invalid(format!(
            "author must be 1..={MAX_AUTHOR_BYTES} bytes"
        )));
    }
    let provenance = require_str(map, "provenance")?;
    if provenance.len() > MAX_PROVENANCE_BYTES {
        return Err(invalid(format!(
            "provenance exceeds {MAX_PROVENANCE_BYTES} bytes"
        )));
    }
    if provenance.trim().is_empty() {
        return Err(invalid("provenance must not be blank"));
    }
    let links = match map.get("source_links") {
        None | Some(Value::Null) => {
            return Err(invalid(
                "missing required field `source_links` (use [] when none)",
            ));
        }
        Some(Value::Array(items)) => items.clone(),
        Some(_) => return Err(invalid("field `source_links` must be an array of handles")),
    };
    if links.len() > MAX_LINKS {
        return Err(invalid(format!(
            "source_links holds at most {MAX_LINKS} handles"
        )));
    }
    let mut source_links = Vec::with_capacity(links.len());
    for item in &links {
        let Value::String(handle) = item else {
            return Err(invalid("field `source_links` must be an array of handles"));
        };
        // Grammar is field validation: a malformed handle never reaches the
        // store. Workspace and freshness are checked at execution time.
        store::HandleRef::parse(handle)?;
        source_links.push(handle.clone());
    }
    Ok(RecordFields {
        id,
        text,
        author,
        provenance,
        source_links,
    })
}

/// Strictly parse one memory request object. `op` is required; unknown
/// fields, nulls, type mismatches, out-of-range values and contradictory op
/// fields are `invalid_argument` before any store is touched.
pub fn parse_request(map: &Map<String, Value>) -> FResult<MemoryRequest> {
    if serde_json::to_string(map).unwrap_or_default().len() > MAX_REQUEST_BYTES {
        return Err(invalid(format!(
            "memory request exceeds {MAX_REQUEST_BYTES} bytes"
        )));
    }
    let op = require_str(map, "op")?;
    if !OPS.contains(&op.as_str()) {
        return Err(invalid(format!("op must be one of {}", OPS.join(", "))));
    }
    let workspace_id = parse_workspace_id(map)?;
    match op.as_str() {
        "put" => {
            reject_fields(
                map,
                &[
                    "op",
                    "id",
                    "text",
                    "author",
                    "provenance",
                    "source_links",
                    "workspace_id",
                ],
            )?;
            Ok(MemoryRequest::Put(PutInput {
                fields: parse_fields(map)?,
                workspace_id,
            }))
        }
        "update" => {
            reject_fields(
                map,
                &[
                    "op",
                    "id",
                    "text",
                    "author",
                    "provenance",
                    "source_links",
                    "workspace_id",
                    "expected_revision",
                ],
            )?;
            Ok(MemoryRequest::Update(UpdateInput {
                fields: parse_fields(map)?,
                workspace_id,
                expected_revision: require_u64(map, "expected_revision")?,
            }))
        }
        "get" => {
            reject_fields(map, &["op", "id", "workspace_id"])?;
            Ok(MemoryRequest::Get {
                id: parse_id(map)?,
                workspace_id,
            })
        }
        "forget" => {
            reject_fields(map, &["op", "id", "workspace_id", "expected_revision"])?;
            Ok(MemoryRequest::Forget(ForgetInput {
                id: parse_id(map)?,
                workspace_id,
                expected_revision: require_u64(map, "expected_revision")?,
            }))
        }
        _ => {
            reject_fields(map, &["op", "query", "workspace_id", "limit", "tokens"])?;
            let query = require_str(map, "query")?;
            if query.trim().is_empty() || query.len() > 4096 {
                return Err(invalid("query must contain 1..4096 nonblank bytes"));
            }
            let limit = match map.get("limit") {
                None => 10,
                Some(Value::Null) => {
                    return Err(invalid("field `limit` must not be null"));
                }
                Some(value) => value
                    .as_u64()
                    .filter(|n| (1..=64).contains(n))
                    .ok_or_else(|| invalid("limit must be an integer in 1..=64"))?,
            };
            let tokens = match map.get("tokens") {
                None => 1024,
                Some(Value::Null) => {
                    return Err(invalid("field `tokens` must not be null"));
                }
                Some(value) => value
                    .as_u64()
                    .filter(|n| (1..=32768).contains(n))
                    .ok_or_else(|| invalid("tokens must be an integer in 1..=32768"))?,
            } as usize;
            check_token_budget(tokens)?;
            Ok(MemoryRequest::Search(SearchInput {
                query,
                workspace_id,
                limit: limit as usize,
                tokens,
            }))
        }
    }
}

/// Decode one stored row; an undecodable row is `corrupt_memory` naming the
/// id, never a silent absence.
pub(crate) fn decode_row(id: &str, raw: &str) -> FResult<MemoryRecord> {
    serde_json::from_str(raw)
        .map_err(|e| FoundryError::CorruptMemory(format!("{id}: {e}")))
        .and_then(|record: MemoryRecord| {
            if record.id != id {
                return Err(FoundryError::CorruptMemory(format!(
                    "{id}: row names id {}",
                    record.id
                )));
            }
            Ok(record)
        })
}

/// The bound workspace, or the refusal for a caller-named different one.
/// The wrong-workspace check runs before any existence check: a forget of an
/// absent id still fails with `wrong_workspace`.
fn check_workspace(engine: &Engine, workspace_id: &str) -> FResult<String> {
    let bound = engine
        .workspace_id()
        .ok_or(FoundryError::WorkspaceUnbound)?;
    if workspace_id != bound {
        return Err(FoundryError::WrongWorkspace);
    }
    Ok(bound)
}

impl Engine {
    /// Field and bounds validation at the authoritative boundary, mirroring
    /// `replace_source`: the parser may have checked, but the engine never
    /// trusts its callers. An id outside `[A-Za-z0-9_-]{1,128}` could forge
    /// wire lines or collide with source search keys, so it is refused here.
    fn validate_record_fields(fields: &RecordFields) -> FResult<()> {
        let valid_id = !fields.id.is_empty()
            && fields.id.len() <= MAX_ID_BYTES
            && fields
                .id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-');
        if !valid_id {
            return Err(FoundryError::InvalidArgument(
                "id must match [A-Za-z0-9_-]{1,128}".into(),
            ));
        }
        if fields.text.len() > MAX_TEXT_BYTES || fields.text.trim().is_empty() {
            return Err(FoundryError::InvalidArgument(format!(
                "text must be nonblank and at most {MAX_TEXT_BYTES} bytes"
            )));
        }
        if fields.author.is_empty() || fields.author.len() > MAX_AUTHOR_BYTES {
            return Err(FoundryError::InvalidArgument(format!(
                "author must be 1..={MAX_AUTHOR_BYTES} bytes"
            )));
        }
        if fields.provenance.len() > MAX_PROVENANCE_BYTES || fields.provenance.trim().is_empty() {
            return Err(FoundryError::InvalidArgument(format!(
                "provenance must be nonblank and at most {MAX_PROVENANCE_BYTES} bytes"
            )));
        }
        if fields.source_links.len() > MAX_LINKS {
            return Err(FoundryError::InvalidArgument(format!(
                "source_links holds at most {MAX_LINKS} handles"
            )));
        }
        for handle in &fields.source_links {
            store::HandleRef::parse(handle)?;
        }
        Ok(())
    }

    fn validate_id(id: &str) -> FResult<()> {
        let valid = !id.is_empty()
            && id.len() <= MAX_ID_BYTES
            && id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-');
        if valid {
            Ok(())
        } else {
            Err(FoundryError::InvalidArgument(
                "id must match [A-Za-z0-9_-]{1,128}".into(),
            ))
        }
    }

    /// Validate the source links of an actual create/replace inside the
    /// mutation's own write transaction: workspace, existence, digest and
    /// range against that transaction's authoritative source state.
    fn validate_links_in_tx(
        tx: &redb::WriteTransaction,
        bound: &str,
        links: &[String],
    ) -> FResult<()> {
        // Full verified-source validation (chunk reconstruct + hash + range +
        // UTF-8 boundaries) through the shared retrieve-grade checks.
        for handle in links {
            store::validate_link_span(tx, bound, handle)?;
        }
        Ok(())
    }

    /// `put`: absent id → new revision; identical fields → the existing row
    /// unchanged (no revision consumed); different fields → `conflict`. Row,
    /// counter and pending key commit in ONE transaction.
    pub fn memory_put(&self, input: &PutInput) -> FResult<PutReport> {
        Self::validate_record_fields(&input.fields)?;
        let bound = check_workspace(self, &input.workspace_id)?;
        let tx = self.db.begin_write()?;
        let report = {
            let mut meta = tx.open_table(META)?;
            let mut memory = tx.open_table(MEMORY)?;
            // The read guard is dropped at the end of this match, before any
            // write to the same table.
            let existing = match memory.get(input.fields.id.as_str())? {
                Some(raw) => Some(decode_row(&input.fields.id, raw.value())?),
                None => None,
            };
            match existing {
                Some(record) => {
                    let same = record.text == input.fields.text
                        && record.author == input.fields.author
                        && record.provenance == input.fields.provenance
                        && record.source_links == input.fields.source_links;
                    if !same {
                        return Err(FoundryError::Conflict(format!(
                            "record `{}` exists with different fields",
                            input.fields.id
                        )));
                    }
                    PutReport {
                        id: record.id,
                        revision: record.revision,
                        outcome: "unchanged",
                    }
                }
                None => {
                    // A real create: every link validates against THIS
                    // transaction's source state, after the idempotency
                    // decision, so a retried identical put whose source later
                    // changed still reports `unchanged`.
                    Self::validate_links_in_tx(&tx, &bound, &input.fields.source_links)?;
                    // Counter exhaustion is refused before any write in this
                    // transaction; a refused put writes nothing and consumes
                    // no revision.
                    let next = store::read_counter(&meta, "memory_revision")?
                        .checked_add(1)
                        .ok_or(FoundryError::RevisionExhausted)?;
                    let record = MemoryRecord {
                        id: input.fields.id.clone(),
                        workspace_id: bound,
                        revision: next,
                        text: input.fields.text.clone(),
                        author: input.fields.author.clone(),
                        provenance: input.fields.provenance.clone(),
                        source_links: input.fields.source_links.clone(),
                    };
                    memory.insert(record.id.as_str(), serde_json::to_string(&record)?.as_str())?;
                    meta.insert("memory_revision", next.to_string().as_str())?;
                    tx.open_table(store::PENDING)?.insert(
                        memory_pending_key(&record.id).as_str(),
                        next.to_string().as_str(),
                    )?;
                    PutReport {
                        id: record.id,
                        revision: next,
                        outcome: "created",
                    }
                }
            }
        };
        tx.commit()?;
        Ok(report)
    }

    /// `update`: exact `expected_revision` → one atomic replacement with a
    /// new revision; else `conflict`; absent id → `not_found`.
    pub fn memory_update(&self, input: &UpdateInput) -> FResult<PutReport> {
        Self::validate_record_fields(&input.fields)?;
        let bound = check_workspace(self, &input.workspace_id)?;
        let tx = self.db.begin_write()?;
        let report = {
            let mut meta = tx.open_table(META)?;
            let mut memory = tx.open_table(MEMORY)?;
            let current = match memory.get(input.fields.id.as_str())? {
                Some(raw) => decode_row(&input.fields.id, raw.value())?,
                None => return Err(FoundryError::NotFound),
            };
            if current.revision != input.expected_revision {
                return Err(FoundryError::Conflict(format!(
                    "record `{}` is at revision {}",
                    input.fields.id, current.revision
                )));
            }
            // A real replacement: links validate inside this transaction,
            // after the CAS check, so a consumed revision conflicts even when
            // its links have since gone stale.
            Self::validate_links_in_tx(&tx, &bound, &input.fields.source_links)?;
            let next = store::read_counter(&meta, "memory_revision")?
                .checked_add(1)
                .ok_or(FoundryError::RevisionExhausted)?;
            let record = MemoryRecord {
                id: input.fields.id.clone(),
                workspace_id: bound,
                revision: next,
                text: input.fields.text.clone(),
                author: input.fields.author.clone(),
                provenance: input.fields.provenance.clone(),
                source_links: input.fields.source_links.clone(),
            };
            memory.insert(record.id.as_str(), serde_json::to_string(&record)?.as_str())?;
            meta.insert("memory_revision", next.to_string().as_str())?;
            tx.open_table(store::PENDING)?.insert(
                memory_pending_key(&record.id).as_str(),
                next.to_string().as_str(),
            )?;
            PutReport {
                id: record.id,
                revision: next,
                outcome: "updated",
            }
        };
        tx.commit()?;
        Ok(report)
    }

    /// `get`: the live row with exact text, `kind:"memory"` and per-link
    /// status. Links never cause an edit. Authoritative-only: works when the
    /// lexical index is broken or missing.
    pub fn memory_get(&self, id: &str, workspace_id: &str) -> FResult<MemoryGet> {
        check_workspace(self, workspace_id)?;
        let tx = self.db.begin_read()?;
        let memory = tx.open_table(MEMORY)?;
        let sources = tx.open_table(SOURCES)?;
        let record = match memory.get(id)? {
            Some(raw) => decode_row(id, raw.value())?,
            None => return Err(FoundryError::NotFound),
        };
        let mut source_links = Vec::with_capacity(record.source_links.len());
        for handle in &record.source_links {
            // A stored link was a valid handle at put time; one that no
            // longer parses is corruption of this row, named as such.
            let parsed = store::HandleRef::parse(handle).map_err(|_| {
                FoundryError::CorruptMemory(format!("{id}: stored source link is malformed"))
            })?;
            let status = match sources.get(parsed.path.as_str())? {
                None => "missing",
                Some(meta) => {
                    // A linked source row that cannot be decoded is store
                    // corruption, not a stale link.
                    let meta: store::SourceMeta =
                        serde_json::from_str(meta.value()).map_err(|e| {
                            FoundryError::CorruptStore(format!(
                                "source record cannot be decoded: {e}"
                            ))
                        })?;
                    if meta.hash.starts_with(&parsed.sha32) {
                        "fresh"
                    } else {
                        "stale"
                    }
                }
            };
            source_links.push(LinkStatus {
                handle: handle.clone(),
                status,
            });
        }
        Ok(MemoryGet {
            id: record.id,
            workspace_id: record.workspace_id,
            revision: record.revision,
            kind: "memory",
            text: record.text,
            author: record.author,
            provenance: record.provenance,
            source_links,
        })
    }

    /// The committed core of a forget: on success the row is gone and the
    /// index deletion is queued; `already_absent` and `conflict` name the
    /// refusal without writing anything. Wrong workspace fails even for an
    /// absent id (checked by [`check_workspace`] first).
    fn memory_forget_core(
        &self,
        input: &ForgetInput,
        control: &crate::Control,
    ) -> FResult<ForgetReport> {
        Self::validate_id(&input.id)?;
        check_workspace(self, &input.workspace_id)?;
        control.check()?;
        let tx = self.db.begin_write()?;
        let (removed_revision, memory_revision, live_records) = {
            let mut memory = tx.open_table(MEMORY)?;
            let mut pending = tx.open_table(store::PENDING)?;
            let current = match memory.get(input.id.as_str())? {
                Some(raw) => decode_row(&input.id, raw.value())?,
                None => return Err(FoundryError::AlreadyAbsent(input.id.clone())),
            };
            if current.revision != input.expected_revision {
                return Err(FoundryError::Conflict(format!(
                    "record `{}` is at revision {}",
                    input.id, current.revision
                )));
            }
            memory.remove(input.id.as_str())?;
            pending.insert(memory_pending_key(&input.id).as_str(), "deleted")?;
            // A failure here drops the transaction: the row stays intact.
            fault!(
                MEMORY_FORGET_BEFORE_COMMIT,
                Some(self),
                Some(control),
                &input.id
            )?;
            (
                Some(current.revision),
                store::read_counter(&tx.open_table(META)?, "memory_revision")?,
                memory.len()?,
            )
        };
        tx.commit()?;
        // The row is durably gone; only the derived deletion may lag.
        fault!(
            MEMORY_FORGET_AFTER_COMMIT,
            Some(self),
            Some(control),
            &input.id
        )?;
        Ok(ForgetReport {
            id: input.id.clone(),
            outcome: ForgetOutcome::Deleted,
            removed_revision,
            memory_revision,
            live_records,
        })
    }

    /// `forget` as the surface delivers it: `deleted`, `already_absent` and
    /// `conflict` are all outcomes of one content-free report (008), so the
    /// engine's refusal codes convert here instead of failing the command.
    pub fn memory_forget(
        &self,
        input: &ForgetInput,
        control: &crate::Control,
    ) -> FResult<ForgetReport> {
        match self.memory_forget_core(input, control) {
            Ok(report) => Ok(report),
            Err(FoundryError::AlreadyAbsent(_)) => {
                let (memory_revision, live_records) = self.memory_counts()?;
                Ok(ForgetReport {
                    id: input.id.clone(),
                    outcome: ForgetOutcome::AlreadyAbsent,
                    removed_revision: None,
                    memory_revision,
                    live_records,
                })
            }
            Err(FoundryError::Conflict(_)) => {
                let (memory_revision, live_records) = self.memory_counts()?;
                Ok(ForgetReport {
                    id: input.id.clone(),
                    outcome: ForgetOutcome::Conflict,
                    removed_revision: None,
                    memory_revision,
                    live_records,
                })
            }
            Err(other) => Err(other),
        }
    }

    fn memory_counts(&self) -> FResult<(u64, u64)> {
        let tx = self.db.begin_read()?;
        let memory = tx.open_table(MEMORY)?;
        Ok((
            store::read_counter(&tx.open_table(META)?, "memory_revision")?,
            memory.len()?,
        ))
    }

    /// `export` (CLI only): one sorted-by-id JSONL page of live records,
    /// at most `limit` (1..=128) rows and 4 MiB, stopping earlier with a
    /// cursor. Authoritative-only; the stderr metadata carries counts only.
    pub fn memory_export(
        &self,
        workspace_id: &str,
        after_id: Option<&str>,
        limit: usize,
    ) -> FResult<ExportPage> {
        check_workspace(self, workspace_id)?;
        if !(1..=EXPORT_PAGE_ROWS).contains(&limit) {
            return Err(invalid(format!("limit must be 1..={EXPORT_PAGE_ROWS}")));
        }
        if let Some(after) = after_id
            && (after.is_empty() || after.len() > MAX_ID_BYTES)
        {
            return Err(invalid("after_id must be 1..128 bytes"));
        }
        let tx = self.db.begin_read()?;
        let memory = tx.open_table(MEMORY)?;
        let mut rows = Vec::new();
        let mut bytes = 0usize;
        let mut last: Option<String> = None;
        let mut more = false;
        for row in memory.range::<&str>((
            after_id.map_or(Bound::Unbounded, Bound::Excluded),
            Bound::Unbounded,
        ))? {
            let (key, raw) = row?;
            if rows.len() == limit {
                more = true;
                break;
            }
            let record = decode_row(key.value(), raw.value())?;
            let line = format!(
                "{}\n",
                serde_json::to_string(&record).map_err(FoundryError::from)?
            );
            if bytes + line.len() > EXPORT_PAGE_BYTES {
                more = true;
                break;
            }
            bytes += line.len();
            last = Some(key.value().to_owned());
            rows.push(line);
        }
        // A cursor only when a page actually stopped early with rows: an
        // empty page (impossible under the field bounds, but safe) ends the
        // walk instead of looping.
        let next_after_id = if more && !rows.is_empty() { last } else { None };
        Ok(ExportPage {
            rows,
            bytes,
            next_after_id,
        })
    }

    /// Memory search: the derived documents matching `query`, each validated
    /// against the live row (same id AND revision) in one final read
    /// transaction. A forgotten or superseded document never leaks. This
    /// needs the lexical index; `get`/`export` do not.
    pub fn memory_search(&self, input: &SearchInput) -> FResult<MemorySearchOutcome> {
        if !(1..=32768).contains(&input.tokens) {
            return Err(FoundryError::InvalidArgument(
                "token budget must be 1..32768".into(),
            ));
        }
        check_workspace(self, &input.workspace_id)?;
        self.memory_hits(&input.query, input.limit)
    }

    /// The validated hits of [`Self::memory_search`] without the scope check,
    /// for `context {include_memory}` (which has no workspace argument; it
    /// reads the bound, primary store only).
    pub fn memory_hits(&self, query: &str, limit: usize) -> FResult<MemorySearchOutcome> {
        if !(1..=64).contains(&limit) {
            return Err(FoundryError::InvalidArgument("limit must be 1..64".into()));
        }
        // The whole candidate window is examined before the cut to `limit`:
        // forgotten or superseded documents still in the derived index must
        // not crowd out live records.
        let plan = self.memory_plan(query)?;
        let tx = self.db.begin_read()?;
        let memory = tx.open_table(MEMORY)?;
        let mut hits = Vec::with_capacity(limit.min(plan.candidates.len()));
        let mut stale = 0u64;
        for candidate in plan.candidates {
            let record = match memory.get(candidate.id.as_str())? {
                Some(raw) => decode_row(&candidate.id, raw.value())?,
                None => {
                    stale += 1;
                    continue;
                }
            };
            if record.revision != candidate.revision {
                stale += 1;
                continue;
            }
            if hits.len() < limit {
                hits.push(MemoryHit {
                    id: record.id,
                    revision: record.revision,
                    author: record.author,
                    text: record.text,
                });
            }
        }
        let mut freshness = self.freshness_in(&tx)?;
        // The memory header's `pending:` counts memory work only (008): the
        // source namespaces report their own.
        freshness.pending_sources = store::count_pending_prefix(&tx, "memory:")?;
        Ok(MemorySearchOutcome {
            freshness,
            hits,
            stale_candidates: stale,
            candidates_full: plan.window_full,
        })
    }
}
