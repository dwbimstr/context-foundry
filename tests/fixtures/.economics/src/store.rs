//! Authoritative source store (redb) plus a disposable derived search index
//! (Tantivy). One owner per store; explicit initialization, explicit schema
//! upgrade and explicit repair. Reads never mutate authoritative state.
use crate::Strategy;
use crate::error::{FResult, FoundryError};
use crate::graph;
use crate::response::{self, Freshness};
use redb::{
    Database, ReadableDatabase, ReadableTable, ReadableTableMetadata, TableDefinition,
    WriteTransaction,
};
use serde::{Deserialize, Serialize};
use std::ops::Bound;
use std::path::{Path, PathBuf};
use tantivy::collector::TopDocs;
use tantivy::query::QueryParser;
use tantivy::schema::{Field, STORED, STRING, Schema, TEXT, Value};
use tantivy::{Index, IndexReader, IndexWriter, ReloadPolicy, TantivyDocument, Term, doc};

pub(crate) const SOURCES: TableDefinition<&str, &str> = TableDefinition::new("sources");
const CHUNKS: TableDefinition<&str, &str> = TableDefinition::new("chunks");
const PENDING: TableDefinition<&str, &str> = TableDefinition::new("pending_index");
pub(crate) const META: TableDefinition<&str, &str> = TableDefinition::new("meta");
pub(crate) const SEEN: TableDefinition<&str, &str> = TableDefinition::new("scan_seen");
pub(crate) const FEEDBACK: TableDefinition<&str, &str> = TableDefinition::new("feedback");

pub const SCHEMA_VERSION: u32 = 2;
const MAX_SOURCE_BYTES: usize = 2 * 1024 * 1024;
const PAGE: usize = 128;
/// Search examines at most this many candidates.
const CANDIDATE_LIMIT: usize = 256;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SourceMeta {
    pub hash: String,
    pub chunks: usize,
    pub bytes: usize,
    pub lines: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Chunk {
    pub path: String,
    pub hash: String,
    pub start_line: usize,
    pub end_line: usize,
    pub body: String,
}

/// Shared source reference: `{v:1, workspace_id, path, sha256, start, end}`.
/// Byte ranges are half-open; the only empty range is `[0,0)` on empty sources.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SourceHandle {
    pub v: u8,
    pub workspace_id: String,
    pub path: String,
    pub sha256: String,
    pub start: u64,
    pub end: u64,
}

impl SourceHandle {
    /// Field/type/version/bounds validation only. Workspace, existence, hash
    /// and range checks happen later, in that order, against the store.
    pub fn from_json(raw: &str) -> FResult<Self> {
        if raw.len() > 32768 {
            return Err(FoundryError::InvalidArgument(
                "handle exceeds 32768 bytes".into(),
            ));
        }
        let value: serde_json::Value = serde_json::from_str(raw)
            .map_err(|e| FoundryError::InvalidArgument(format!("handle is not valid JSON: {e}")))?;
        let Some(object) = value.as_object() else {
            return Err(FoundryError::InvalidArgument(
                "handle must be a JSON object".into(),
            ));
        };
        let expected = ["v", "workspace_id", "path", "sha256", "start", "end"];
        if object.len() != expected.len() {
            return Err(FoundryError::InvalidArgument(
                "handle must contain exactly v, workspace_id, path, sha256, start, end".into(),
            ));
        }
        for key in expected {
            if !object.contains_key(key) {
                return Err(FoundryError::InvalidArgument(format!(
                    "handle is missing field {key}"
                )));
            }
        }
        let invalid = |field: &str| FoundryError::InvalidArgument(format!("handle field {field}"));
        let v = object["v"].as_u64().ok_or_else(|| invalid("v"))?;
        if v != 1 {
            return Err(FoundryError::InvalidArgument(format!(
                "unsupported handle version {v}"
            )));
        }
        let workspace_id = object["workspace_id"]
            .as_str()
            .ok_or_else(|| invalid("workspace_id"))?
            .to_owned();
        if !is_lower_hex64(&workspace_id) {
            return Err(FoundryError::InvalidArgument(
                "workspace_id must be 64 lowercase hex characters".into(),
            ));
        }
        let path = object["path"]
            .as_str()
            .ok_or_else(|| invalid("path"))?
            .to_owned();
        validate_path(&path)
            .map_err(|e| FoundryError::InvalidArgument(format!("handle path: {e}")))?;
        let sha256 = object["sha256"]
            .as_str()
            .ok_or_else(|| invalid("sha256"))?
            .to_owned();
        if !is_lower_hex64(&sha256) {
            return Err(FoundryError::InvalidArgument(
                "sha256 must be 64 lowercase hex characters".into(),
            ));
        }
        let start = object["start"].as_u64().ok_or_else(|| invalid("start"))?;
        let end = object["end"].as_u64().ok_or_else(|| invalid("end"))?;
        if end < start {
            return Err(FoundryError::InvalidArgument(
                "handle end precedes start".into(),
            ));
        }
        Ok(SourceHandle {
            v: 1,
            workspace_id,
            path,
            sha256,
            start,
            end,
        })
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }
}

fn is_lower_hex64(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

#[derive(Clone, Debug, Serialize)]
pub struct Hit {
    pub path: String,
    pub start_line: u64,
    pub end_line: u64,
    pub handle: SourceHandle,
    pub text: String,
}

#[derive(Debug, Serialize)]
pub struct SearchOutcome {
    pub workspace_id: String,
    pub source_revision: u64,
    pub hits: Vec<Hit>,
    pub pending_sources: u64,
    pub stale_candidates: u64,
    pub candidate_limit: usize,
    pub candidate_limit_reached: bool,
    pub truncated: bool,
    pub scan_state: String,
}

/// One ordered evidence candidate for a context bundle. Sources come first
/// (highest ranked), then bounded graph evidence, then remaining spans.
#[derive(Clone, Debug)]
pub enum Evidence {
    Source {
        path: String,
        /// One-based inclusive line citation of the span.
        start_line: u64,
        end_line: u64,
        handle: SourceHandle,
        text: String,
    },
    Graph {
        text: String,
        /// The exact stored edge row; revalidated in the final read.
        raw: String,
        from_path: String,
        /// (path, sha256) endpoints revalidated in the final read.
        endpoints: Vec<(String, String)>,
    },
}

impl Evidence {
    pub(crate) fn source(hit: Hit) -> Self {
        Evidence::Source {
            path: hit.path,
            start_line: hit.start_line,
            end_line: hit.end_line,
            handle: hit.handle,
            text: hit.text,
        }
    }

    pub(crate) fn graph(evidence: &graph::GraphEvidence) -> Self {
        let edge = &evidence.edge;
        Evidence::Graph {
            text: format!(
                "{}:{} ({}) --{}--> {}:{} ({}) [{}; provider={}@{}]",
                edge.from.path,
                edge.from.line,
                edge.from.symbol,
                edge.kind,
                edge.to.path,
                edge.to.line,
                edge.to.symbol,
                edge.evidence,
                evidence.provider,
                evidence.revision
            ),
            raw: evidence.raw.clone(),
            from_path: edge.from.path.clone(),
            endpoints: vec![
                (edge.from.path.clone(), edge.from.hash.clone()),
                (edge.to.path.clone(), edge.to.hash.clone()),
            ],
        }
    }

    pub fn text(&self) -> &str {
        match self {
            Evidence::Source { text, .. } | Evidence::Graph { text, .. } => text,
        }
    }

    /// Deduplication identity: `(workspace_id, path, sha256, start, end)` for
    /// source spans and the stable stored edge row for graph evidence.
    fn dedup_key(&self) -> String {
        match self {
            Evidence::Source { handle, .. } => format!(
                "{}/{}/{}/{}/{}",
                handle.workspace_id, handle.path, handle.sha256, handle.start, handle.end
            ),
            Evidence::Graph { raw, .. } => format!("edge/{raw}"),
        }
    }
}

#[derive(Clone, Debug)]
pub struct ContextOutcome {
    pub query: String,
    pub requested_tokens: usize,
    pub strategy: Strategy,
    /// `graph_unavailable`, `graph_stale` or `graph_invalid` when graph
    /// evidence was requested but is not (fully) available.
    pub graph_reason: Option<&'static str>,
    pub freshness: Freshness,
    pub candidates: Vec<Evidence>,
    pub stale_candidates: u64,
    pub candidate_limit: usize,
    pub candidate_limit_reached: bool,
    /// Search stopped at the requested-hit or candidate-window limit.
    pub search_truncated: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct RetrieveOutcome {
    pub requested: SourceHandle,
    pub requested_tokens: usize,
    /// Full requested span bytes; packers may deliver a fitting prefix.
    pub span: Vec<u8>,
    pub source_bytes: u64,
    pub freshness: Freshness,
}

#[derive(Clone, Debug, Serialize)]
pub struct StoreStatus {
    pub schema: u32,
    pub workspace_id: Option<String>,
    pub source_revision: u64,
    pub source_count: u64,
    pub pending_count: u64,
    pub index_state: String,
    pub index_reason: Option<String>,
    pub scan_state: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct RebuildMarker {
    pub owner_id: String,
    /// Basename of the single retained quarantine of the original derived
    /// directory. Recorded as intent before the move.
    pub quarantine: Option<String>,
    /// True once the original derived directory has been quarantined or was
    /// positively absent. From then on `search/` can only be this repair's
    /// replacement, so an unrecognized directory there is a conflict.
    pub original_handled: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct RepairReport {
    pub repaired: bool,
    pub quarantined_to: Option<PathBuf>,
    pub drained_sources: usize,
    pub reason: Option<String>,
}

struct Fields {
    key: Field,
    path: Field,
    hash: Field,
    body: Field,
}

struct SearchHandles {
    index: Index,
    reader: IndexReader,
    writer: IndexWriter,
    fields: Fields,
}

pub struct Engine {
    pub(crate) db: Database,
    pub(crate) directory: PathBuf,
    search: Option<SearchHandles>,
    repair_reason: Option<String>,
    workspace: Option<String>,
    workspace_id: Option<String>,
    schema: u32,
}

impl std::fmt::Debug for Engine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Engine")
            .field("directory", &self.directory)
            .field("schema", &self.schema)
            .field("workspace_id", &self.workspace_id)
            .field("search_available", &self.search.is_some())
            .finish_non_exhaustive()
    }
}

/// Normalized workspace-relative path rules shared by handles, graph
/// endpoints and admitted scan paths. Validation runs on the RAW `/`-separated
/// components before any normalized interpretation, so `a//b`, `a/./b`,
/// `a/b/` and `/a` are rejected instead of being normalized into aliases.
pub fn validate_path(path: &str) -> FResult<()> {
    if path.is_empty() || path.len() > 4096 {
        return Err(FoundryError::InvalidArgument(
            "source path must be 1..4096 UTF-8 bytes".into(),
        ));
    }
    if path.contains(['\0', '\n', '\r']) {
        return Err(FoundryError::InvalidArgument(
            "source path contains forbidden control characters".into(),
        ));
    }
    if path
        .split('/')
        .any(|c| c.is_empty() || c == "." || c == "..")
    {
        return Err(FoundryError::InvalidArgument(
            "source path must be normalized: no absolute prefix and no empty, . or .. components"
                .into(),
        ));
    }
    Ok(())
}

fn chunk_key(path: &str, ordinal: usize) -> String {
    format!("{path}\0{ordinal:010}")
}

/// Decode stored authoritative JSON; a decode failure is corruption, never an
/// invented empty value.
fn decode<T: for<'de> Deserialize<'de>>(raw: &str, what: &str) -> FResult<T> {
    serde_json::from_str(raw)
        .map_err(|e| FoundryError::CorruptStore(format!("{what} record cannot be decoded: {e}")))
}

/// Required schema-2 counter: absent or malformed means authoritative
/// corruption, never an invented zero. Zero defaults belong only in
/// initialization and the explicit upgrade.
fn read_counter<T: ReadableTable<&'static str, &'static str>>(meta: &T, key: &str) -> FResult<u64> {
    let raw = meta
        .get(key)?
        .ok_or_else(|| FoundryError::CorruptStore(format!("{key} metadata missing")))?;
    raw.value().parse::<u64>().map_err(|_| {
        FoundryError::CorruptStore(format!("{key} metadata is not an unsigned integer"))
    })
}

/// Required scan state. A persisted `running` means a scan died before it
/// finished; report it conservatively as `incomplete`.
fn read_scan_state<T: ReadableTable<&'static str, &'static str>>(meta: &T) -> FResult<String> {
    let raw = meta
        .get("scan_status")?
        .ok_or_else(|| FoundryError::CorruptStore("scan_status metadata missing".into()))?;
    match raw.value() {
        state @ ("never" | "complete" | "incomplete") => Ok(state.to_owned()),
        "running" => Ok("incomplete".to_owned()),
        other => Err(FoundryError::CorruptStore(format!(
            "scan_status metadata has unknown value {other:?}"
        ))),
    }
}

/// A source reconstructed from its stored chunks in ordinal order and
/// verified (keys, ownership, length, SHA-256) against its metadata.
struct VerifiedSource {
    body: String,
    /// Byte range of each chunk in `body`, by ordinal.
    offsets: Vec<(usize, usize)>,
    /// One-based inclusive line range of each chunk, by ordinal.
    lines: Vec<(u64, u64)>,
}

impl VerifiedSource {
    /// The chunk at `ordinal` as a hit. Line citations are computed from the
    /// verified bytes, never trusted from stored chunk records.
    fn hit(&self, workspace_id: &str, path: &str, sha256: &str, ordinal: usize) -> Option<Hit> {
        let (start, end) = *self.offsets.get(ordinal)?;
        let (start_line, end_line) = *self.lines.get(ordinal)?;
        Some(Hit {
            path: path.to_owned(),
            start_line,
            end_line,
            handle: SourceHandle {
                v: 1,
                workspace_id: workspace_id.to_owned(),
                path: path.to_owned(),
                sha256: sha256.to_owned(),
                start: start as u64,
                end: end as u64,
            },
            text: self.body[start..end].to_owned(),
        })
    }
}

/// Reconstruct a source inside the caller's transaction and verify every
/// chunk key/ownership plus the full length and hash before anything is
/// sliced. Any inconsistency is `corrupt_source`, never partial evidence.
fn reconstruct_verified<C: ReadableTable<&'static str, &'static str>>(
    stored: &C,
    path: &str,
    meta: &SourceMeta,
) -> FResult<VerifiedSource> {
    let corrupt = |what: &str| FoundryError::CorruptSource(format!("{path}: {what}"));
    if meta.bytes > MAX_SOURCE_BYTES {
        return Err(corrupt("recorded length exceeds the 2 MiB bound"));
    }
    let first = chunk_key(path, 0);
    let end = chunk_key(path, meta.chunks);
    let mut body = String::with_capacity(meta.bytes);
    let mut offsets = Vec::new();
    let mut lines = Vec::new();
    let mut next_line = 1u64;
    for row in stored.range(first.as_str()..end.as_str())? {
        let (key, raw) = row?;
        let ordinal = offsets.len();
        if key.value() != chunk_key(path, ordinal) {
            return Err(corrupt(
                "chunk keys are not the contiguous ordinal sequence",
            ));
        }
        let chunk: Chunk = serde_json::from_str(raw.value())
            .map_err(|_| corrupt("chunk record cannot be decoded"))?;
        if chunk.path != path || chunk.hash != meta.hash {
            return Err(corrupt("chunk belongs to a different source version"));
        }
        let start = body.len();
        body.push_str(&chunk.body);
        if body.len() > meta.bytes {
            return Err(corrupt("chunks exceed the recorded length"));
        }
        offsets.push((start, body.len()));
        let newlines = chunk.body.bytes().filter(|b| *b == b'\n').count() as u64;
        let ends_line = u64::from(chunk.body.ends_with('\n'));
        lines.push((next_line, next_line + newlines.saturating_sub(ends_line)));
        next_line += newlines;
    }
    if offsets.len() != meta.chunks {
        return Err(corrupt("chunk count differs from the source record"));
    }
    if body.len() != meta.bytes || crate::digest(body.as_bytes()) != meta.hash {
        return Err(corrupt(
            "reconstructed bytes do not match the recorded hash",
        ));
    }
    Ok(VerifiedSource {
        body,
        offsets,
        lines,
    })
}

fn chunks(path: &str, hash: &str, content: &str) -> Vec<Chunk> {
    let mut result = Vec::new();
    let mut offset = 0;
    let mut line = 1;
    while offset < content.len() {
        let tail = &content[offset..];
        let mut end = tail.len().min(2048);
        while !tail.is_char_boundary(end) {
            end -= 1;
        }
        if end < tail.len()
            && let Some(nl) = tail[..end].rfind('\n')
        {
            end = nl + 1;
        }
        let body = &tail[..end];
        let newlines = body.bytes().filter(|b| *b == b'\n').count();
        result.push(Chunk {
            path: path.into(),
            hash: hash.into(),
            start_line: line,
            end_line: line + newlines - usize::from(body.ends_with('\n')),
            body: body.into(),
        });
        line += newlines;
        offset += end;
    }
    result
}

fn search_schema() -> Schema {
    let mut schema = Schema::builder();
    schema.add_text_field("key", STRING | STORED);
    schema.add_text_field("path", STRING | STORED);
    schema.add_text_field("hash", STRING | STORED);
    schema.add_text_field("body", TEXT);
    schema.build()
}

fn fields_of(schema: &Schema) -> Fields {
    Fields {
        key: schema.get_field("key").expect("key field"),
        path: schema.get_field("path").expect("path field"),
        hash: schema.get_field("hash").expect("hash field"),
        body: schema.get_field("body").expect("body field"),
    }
}

fn open_search(dir: &Path) -> Result<SearchHandles, String> {
    let index_dir = dir.join("search");
    if !index_dir.join("meta.json").is_file() {
        return Err("derived index is missing".into());
    }
    if index_dir.is_symlink() {
        return Err("derived index path is a symlink".into());
    }
    let index = Index::open_in_dir(&index_dir).map_err(|e| format!("derived index open: {e}"))?;
    let schema = search_schema();
    if index.schema() != schema {
        return Err("derived index schema mismatch; rebuild required".into());
    }
    let fields = fields_of(&schema);
    let reader = index
        .reader_builder()
        .reload_policy(ReloadPolicy::Manual)
        .try_into()
        .map_err(|e| format!("derived index reader: {e}"))?;
    let writer = index
        .writer_with_num_threads(1, 20_000_000)
        .map_err(|e| format!("derived index writer: {e}"))?;
    Ok(SearchHandles {
        index,
        reader,
        writer,
        fields,
    })
}

/// Write schema last in the initializing transaction.
fn initialize_tables(tx: &WriteTransaction) -> FResult<()> {
    tx.open_table(SOURCES)?;
    tx.open_table(CHUNKS)?;
    tx.open_table(PENDING)?;
    tx.open_table(FEEDBACK)?;
    tx.open_table(SEEN)?;
    crate::graph::init(tx)?;
    Ok(())
}

impl Engine {
    /// Explicit initializer: accepts a missing or empty directory, refuses a
    /// nonempty directory without a recognized store, binds the workspace
    /// root and creates the empty derived index.
    pub fn initialize(store_dir: &Path, workspace_root: &Path) -> FResult<Self> {
        let db_path = store_dir.join("knowledge.redb");
        // Validate arguments/root before initializing anything: invalid index
        // arguments — or a root that cannot be held as a descriptor — must not
        // create a store at all.
        let root = workspace_root
            .canonicalize()
            .map_err(|e| FoundryError::InvalidArgument(format!("workspace root: {e}")))?;
        if !root.is_dir() {
            return Err(FoundryError::InvalidArgument(
                "workspace root must be a directory".into(),
            ));
        }
        // A not-yet-existing store directory cannot equal an existing root.
        if store_dir
            .canonicalize()
            .is_ok_and(|existing_store| existing_store == root)
        {
            return Err(FoundryError::InvalidArgument(
                "store directory cannot be the workspace root".into(),
            ));
        }
        crate::ingest::ensure_holdable_root(&root)?;
        if store_dir.exists() {
            let nonempty = std::fs::read_dir(store_dir)
                .map_err(FoundryError::from)?
                .any(|_| true);
            if nonempty && !db_path.exists() {
                return Err(FoundryError::UnrecognizedStore(
                    store_dir.display().to_string(),
                ));
            }
        } else {
            std::fs::create_dir_all(store_dir)?;
        }
        let db = Database::create(&db_path)?;
        let tx = db.begin_write()?;
        {
            let mut meta = tx.open_table(META)?;
            if let Some(version) = meta.get("schema")? {
                return Err(FoundryError::InvalidArgument(format!(
                    "store already initialized with schema {}",
                    version.value()
                )));
            }
            initialize_tables(&tx)?;
            meta.insert("schema", SCHEMA_VERSION.to_string().as_str())?;
            meta.insert("source_revision", "0")?;
            meta.insert("scan_id", "0")?;
            meta.insert("scan_status", "never")?;
            let root_str = root.to_str().ok_or_else(|| {
                FoundryError::InvalidArgument("workspace path is not UTF-8".into())
            })?;
            meta.insert("workspace", root_str)?;
            meta.insert("workspace_id", crate::digest(root_str.as_bytes()).as_str())?;
        }
        tx.commit()?;
        std::fs::create_dir_all(store_dir.join("search"))?;
        let index = Index::create_in_dir(store_dir.join("search"), search_schema())?;
        let fields = fields_of(&index.schema());
        let reader = index
            .reader_builder()
            .reload_policy(ReloadPolicy::Manual)
            .try_into()?;
        let writer = index.writer_with_num_threads(1, 20_000_000)?;
        let (workspace, workspace_id) = Self::read_binding(&db)?;
        Ok(Self {
            db,
            directory: store_dir.canonicalize()?,
            search: Some(SearchHandles {
                index,
                reader,
                writer,
                fields,
            }),
            repair_reason: None,
            workspace,
            workspace_id,
            schema: SCHEMA_VERSION,
        })
    }

    fn read_binding(db: &Database) -> FResult<(Option<String>, Option<String>)> {
        let tx = db.begin_read()?;
        let meta = tx.open_table(META)?;
        Ok((
            meta.get("workspace")?.map(|v| v.value().to_owned()),
            meta.get("workspace_id")?.map(|v| v.value().to_owned()),
        ))
    }

    /// Open existing state only. Never creates files, tables or directories,
    /// never upgrades schema. Derived-index trouble degrades search to
    /// `repair_required`; authoritative reads stay available.
    pub fn open_existing(store_dir: &Path) -> FResult<Self> {
        Self::open_with(store_dir, true)
    }

    /// Authoritative-only open used by repair: no Tantivy construction first.
    fn open_authoritative(store_dir: &Path) -> FResult<Self> {
        Self::open_with(store_dir, false)
    }

    fn open_with(store_dir: &Path, construct_search: bool) -> FResult<Self> {
        let db_path = store_dir.join("knowledge.redb");
        if !db_path.is_file() {
            return Err(FoundryError::StoreNotFound);
        }
        let db = Database::open(&db_path)?;
        let schema = {
            let tx = db.begin_read()?;
            let meta = tx
                .open_table(META)
                .map_err(|e| FoundryError::CorruptStore(format!("meta table unreadable: {e}")))?;
            let Some(version) = meta.get("schema")? else {
                return Err(FoundryError::UnrecognizedStore(
                    "store has no schema marker".into(),
                ));
            };
            match version.value() {
                "1" => return Err(FoundryError::UpgradeRequired { found: "1".into() }),
                "2" => 2u32,
                other => {
                    return Err(FoundryError::UnsupportedSchema {
                        found: other.to_owned(),
                    });
                }
            }
        };
        // Confirm the schema-2 tables exist; missing authoritative tables in a
        // schema-2 store are corruption, not something an open recreates.
        {
            let tx = db.begin_read()?;
            tx.open_table(SEEN).map_err(|e| {
                FoundryError::CorruptStore(format!("scan_seen table unreadable: {e}"))
            })?;
            tx.open_table(SOURCES)
                .map_err(|e| FoundryError::CorruptStore(format!("sources table: {e}")))?;
            tx.open_table(CHUNKS)
                .map_err(|e| FoundryError::CorruptStore(format!("chunks table: {e}")))?;
            tx.open_table(PENDING)
                .map_err(|e| FoundryError::CorruptStore(format!("pending table: {e}")))?;
        }
        let (workspace, workspace_id) = Self::read_binding(&db)?;
        let marker = Self::read_marker(&db)?;
        let mut repair_reason = marker
            .is_some()
            .then(|| "search_rebuild_required marker is set".to_owned());
        let mut search = None;
        if marker.is_none() && construct_search {
            match open_search(store_dir) {
                Ok(handles) => search = Some(handles),
                Err(reason) => repair_reason = Some(reason),
            }
        }
        Ok(Self {
            db,
            directory: store_dir.canonicalize()?,
            search,
            repair_reason,
            workspace,
            workspace_id,
            schema,
        })
    }

    fn read_marker(db: &Database) -> FResult<Option<RebuildMarker>> {
        let tx = db.begin_read()?;
        let meta = tx.open_table(META)?;
        match meta.get("search_rebuild_required")? {
            None => Ok(None),
            Some(raw) => decode::<RebuildMarker>(raw.value(), "rebuild marker")
                .map(Some)
                .map_err(|_| FoundryError::CorruptStore("rebuild marker cannot be decoded".into())),
        }
    }

    /// Explicit v1 -> v2 transaction under exclusive ownership. Preserves all
    /// records, initializes revision/scan metadata, publishes the schema last.
    /// Interrupted upgrade is wholly v1 or v2.
    pub fn upgrade_store(store_dir: &Path, to: u32, control: &crate::Control) -> FResult<()> {
        if to != SCHEMA_VERSION {
            return Err(FoundryError::UnsupportedMode(format!(
                "only upgrade-store --to {SCHEMA_VERSION} is supported"
            )));
        }
        control.check()?;
        let db_path = store_dir.join("knowledge.redb");
        if !db_path.is_file() {
            return Err(FoundryError::StoreNotFound);
        }
        let db = Database::open(&db_path)?;
        {
            let tx = db.begin_read()?;
            let meta = tx.open_table(META)?;
            match meta.get("schema")? {
                None => {
                    return Err(FoundryError::UnrecognizedStore(
                        "store has no schema marker".into(),
                    ));
                }
                Some(v) if v.value() == "2" => return Ok(()), // already upgraded
                Some(v) if v.value() != "1" => {
                    return Err(FoundryError::UnsupportedSchema {
                        found: v.value().to_owned(),
                    });
                }
                _ => {}
            }
        }
        control.check()?;
        let tx = db.begin_write()?;
        {
            tx.open_table(SEEN)?;
            let mut meta = tx.open_table(META)?;
            meta.insert("source_revision", "0")?;
            meta.insert("scan_id", "0")?;
            meta.insert("scan_status", "never")?;
            let bound_root = meta.get("workspace")?.map(|v| v.value().to_owned());
            if let Some(root) = bound_root {
                let id = crate::digest(root.as_bytes());
                meta.insert("workspace_id", id.as_str())?;
            }
            // Publish the schema last inside the same transaction.
            meta.insert("schema", SCHEMA_VERSION.to_string().as_str())?;
        }
        // The upgrade transaction is live and fully written but uncommitted:
        // an exit here must leave the store wholly v1.
        fault!(UPGRADE_BEFORE_COMMIT, None, Some(control), "")?;
        control.check()?;
        tx.commit()?;
        fault!(UPGRADE_AFTER_COMMIT, None, Some(control), "")?;
        Ok(())
    }

    pub fn status(&self) -> FResult<StoreStatus> {
        let (revision, sources, pending, scan_state) = {
            let tx = self.db.begin_read()?;
            let meta = tx.open_table(META)?;
            let revision = read_counter(&meta, "source_revision")?;
            let sources = tx.open_table(SOURCES)?.len()?;
            let pending = tx.open_table(PENDING)?.len()?;
            let scan_state = read_scan_state(&meta)?;
            (revision, sources, pending, scan_state)
        };
        let (index_state, index_reason) = if self.schema != SCHEMA_VERSION {
            ("unsupported".to_owned(), None)
        } else if let Some(reason) = &self.repair_reason {
            ("repair_required".to_owned(), Some(reason.clone()))
        } else if self.search.is_some() {
            if pending == 0 {
                ("ready".to_owned(), None)
            } else {
                ("lagging".to_owned(), None)
            }
        } else {
            (
                "repair_required".to_owned(),
                Some("derived index unavailable".into()),
            )
        };
        Ok(StoreStatus {
            schema: self.schema,
            workspace_id: self.workspace_id.clone(),
            source_revision: revision,
            source_count: sources,
            pending_count: pending,
            index_state,
            index_reason,
            scan_state,
        })
    }

    pub fn workspace_id(&self) -> Option<String> {
        self.workspace_id.clone()
    }

    fn require_workspace_id(&self) -> FResult<String> {
        self.workspace_id
            .clone()
            .ok_or(FoundryError::WorkspaceUnbound)
    }

    fn require_search(&self) -> FResult<&SearchHandles> {
        if let Some(reason) = &self.repair_reason {
            return Err(FoundryError::RepairRequired(reason.clone()));
        }
        self.search
            .as_ref()
            .ok_or_else(|| FoundryError::RepairRequired("derived index unavailable".into()))
    }

    /// Verify a scan root against the bound workspace before any mutation.
    pub(crate) fn verify_binding(&self, root: &Path) -> FResult<PathBuf> {
        let canonical = root
            .canonicalize()
            .map_err(|e| FoundryError::InvalidArgument(format!("workspace root: {e}")))?;
        let canonical_str = canonical
            .to_str()
            .ok_or_else(|| FoundryError::InvalidArgument("workspace path is not UTF-8".into()))?;
        if let Some(existing) = &self.workspace
            && existing != canonical_str
        {
            return Err(FoundryError::WrongWorkspace);
        }
        if canonical == self.directory {
            return Err(FoundryError::InvalidArgument(
                "cannot index the store directory".into(),
            ));
        }
        Ok(canonical)
    }

    /// Bind (or confirm) the workspace and update this live engine's cached
    /// identity, so a long-lived owner of an upgraded unbound store can
    /// search/retrieve right after its first explicit index.
    pub(crate) fn bind_workspace(&mut self, root: &Path) -> FResult<()> {
        let canonical = root
            .canonicalize()
            .map_err(|e| FoundryError::InvalidArgument(format!("workspace root: {e}")))?;
        let root_str = canonical
            .to_str()
            .ok_or_else(|| FoundryError::InvalidArgument("workspace path is not UTF-8".into()))?;
        let id = crate::digest(root_str.as_bytes());
        let tx = self.db.begin_write()?;
        {
            let mut meta = tx.open_table(META)?;
            if let Some(existing) = meta.get("workspace")? {
                if existing.value() != root_str {
                    return Err(FoundryError::WrongWorkspace);
                }
            } else {
                meta.insert("workspace", root_str)?;
                meta.insert("workspace_id", id.as_str())?;
            }
        }
        tx.commit()?;
        self.workspace = Some(root_str.to_owned());
        self.workspace_id = Some(id);
        Ok(())
    }

    pub fn source(&self, path: &str) -> FResult<Option<SourceMeta>> {
        let tx = self.db.begin_read()?;
        let sources = tx.open_table(SOURCES)?;
        match sources.get(path)? {
            None => Ok(None),
            Some(v) => decode::<SourceMeta>(v.value(), "source").map(Some),
        }
    }

    /// Begin a scan: checked scan-id increment and `running` status, before
    /// any tree mutation.
    pub(crate) fn begin_scan(&self) -> FResult<u64> {
        let tx = self.db.begin_write()?;
        let scan_id = {
            let mut meta = tx.open_table(META)?;
            // Required state is validated, never silently overwritten.
            read_scan_state(&meta)?;
            let current = read_counter(&meta, "scan_id")?;
            let next = current
                .checked_add(1)
                .ok_or(FoundryError::ScanIdExhausted)?;
            meta.insert("scan_id", next.to_string().as_str())?;
            meta.insert("scan_status", "running")?;
            next
        };
        tx.commit()?;
        Ok(scan_id)
    }

    pub(crate) fn finish_scan(&self, complete: bool) -> FResult<()> {
        let tx = self.db.begin_write()?;
        {
            let mut meta = tx.open_table(META)?;
            meta.insert(
                "scan_status",
                if complete { "complete" } else { "incomplete" },
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    /// Record a bounded page of encountered paths as seen in `scan_id` with one
    /// commit. Callers flush before the sweep, so the sweep reads a complete set.
    pub(crate) fn mark_seen_batch(&self, paths: &[String], scan_id: u64) -> FResult<()> {
        if paths.is_empty() {
            return Ok(());
        }
        fault!(SEEN_FLUSH, Some(self), None, &paths.len().to_string())?;
        let id = scan_id.to_string();
        let tx = self.db.begin_write()?;
        {
            let mut seen = tx.open_table(SEEN)?;
            for path in paths {
                seen.insert(path.as_str(), id.as_str())?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    /// Paged sweep of source rows not seen in `scan_id`. Each page commits
    /// atomically; an interruption retires only already-committed pages.
    pub(crate) fn sweep_unseen(
        &self,
        scan_id: u64,
        control: &crate::Control,
    ) -> FResult<(u64, bool)> {
        let mut deleted = 0u64;
        let mut after: Option<String> = None;
        loop {
            if control.check().is_err() {
                return Ok((deleted, true));
            }
            let page: Vec<String> = {
                let tx = self.db.begin_read()?;
                let sources = tx.open_table(SOURCES)?;
                let rows = match &after {
                    None => sources.range::<&str>(..)?,
                    Some(key) => {
                        sources.range::<&str>((Bound::Excluded(key.as_str()), Bound::Unbounded))?
                    }
                };
                rows.take(PAGE)
                    .map(|row| row.map(|(k, _)| k.value().to_owned()))
                    .collect::<Result<Vec<String>, redb::StorageError>>()
                    .map_err(FoundryError::from)?
            };
            if page.is_empty() {
                return Ok((deleted, false));
            }
            // A cancel armed here lets this page commit and stops before the next.
            fault!(
                SWEEP_PAGE,
                Some(self),
                Some(control),
                &page.len().to_string()
            )?;
            let mut retired = 0u64;
            let tx = self.db.begin_write()?;
            {
                let mut sources = tx.open_table(SOURCES)?;
                let mut stored = tx.open_table(CHUNKS)?;
                let mut pending = tx.open_table(PENDING)?;
                let mut seen = tx.open_table(SEEN)?;
                let mut meta = tx.open_table(META)?;
                for path in &page {
                    let seen_now = seen
                        .get(path.as_str())?
                        .and_then(|v| v.value().parse::<u64>().ok());
                    if seen_now != Some(scan_id)
                        && let Some(old) = sources.remove(path.as_str())?
                    {
                        let old: SourceMeta = decode(old.value(), "source")?;
                        for i in 0..old.chunks {
                            stored.remove(chunk_key(path, i).as_str())?;
                        }
                        pending.insert(path.as_str(), "deleted")?;
                        seen.remove(path.as_str())?;
                        retired += 1;
                    }
                }
                if retired > 0 {
                    let revision = read_counter(&meta, "source_revision")?;
                    let next = revision
                        .checked_add(retired)
                        .ok_or(FoundryError::RevisionExhausted)?;
                    meta.insert("source_revision", next.to_string().as_str())?;
                }
            }
            tx.commit()?;
            deleted += retired;
            after = page.last().cloned();
        }
    }

    /// All authoritative source/chunk changes, pending index work and the
    /// source revision commit together. Acknowledgement follows commit.
    pub fn replace_source(&self, path: &str, content: &str) -> FResult<bool> {
        validate_path(path)?;
        if content.len() > MAX_SOURCE_BYTES {
            return Err(FoundryError::InvalidArgument(
                "source exceeds 2 MiB limit".into(),
            ));
        }
        let hash = crate::digest(content.as_bytes());
        let pieces = chunks(path, &hash, content);
        let tx = self.db.begin_write()?;
        {
            let mut sources = tx.open_table(SOURCES)?;
            let old = match sources.get(path)? {
                Some(v) => Some(decode::<SourceMeta>(v.value(), "source")?),
                None => None,
            };
            if old.as_ref().is_some_and(|old| old.hash == hash) {
                return Ok(false);
            }
            let mut stored = tx.open_table(CHUNKS)?;
            if let Some(old) = &old {
                for i in 0..old.chunks {
                    stored.remove(chunk_key(path, i).as_str())?;
                }
            }
            for (i, chunk) in pieces.iter().enumerate() {
                stored.insert(
                    chunk_key(path, i).as_str(),
                    serde_json::to_string(chunk)?.as_str(),
                )?;
            }
            sources.insert(
                path,
                serde_json::to_string(&SourceMeta {
                    hash: hash.clone(),
                    chunks: pieces.len(),
                    bytes: content.len(),
                    lines: content.lines().count(),
                })?
                .as_str(),
            )?;
            tx.open_table(PENDING)?.insert(path, hash.as_str())?;
            bump_revision(&tx, 1)?;
        }
        // A failure here drops the transaction: prior source bytes stay intact.
        fault!(SOURCE_BEFORE_COMMIT, Some(self), None, path)?;
        tx.commit()?;
        fault!(SOURCE_AFTER_COMMIT, Some(self), None, path)?;
        Ok(true)
    }

    pub fn delete_source(&self, path: &str) -> FResult<bool> {
        validate_path(path)?;
        let tx = self.db.begin_write()?;
        {
            let mut sources = tx.open_table(SOURCES)?;
            let old = match sources.remove(path)? {
                Some(v) => Some(decode::<SourceMeta>(v.value(), "source")?),
                None => return Ok(false),
            };
            let Some(old) = old else {
                return Ok(false);
            };
            let mut stored = tx.open_table(CHUNKS)?;
            for i in 0..old.chunks {
                stored.remove(chunk_key(path, i).as_str())?;
            }
            tx.open_table(PENDING)?.insert(path, "deleted")?;
            bump_revision(&tx, 1)?;
        }
        fault!(SOURCE_BEFORE_COMMIT, Some(self), None, path)?;
        tx.commit()?;
        fault!(SOURCE_AFTER_COMMIT, Some(self), None, path)?;
        Ok(true)
    }

    pub fn pending(&self) -> FResult<u64> {
        Ok(self.db.begin_read()?.open_table(PENDING)?.len()?)
    }

    pub fn source_revision(&self) -> FResult<u64> {
        let tx = self.db.begin_read()?;
        read_counter(&tx.open_table(META)?, "source_revision")
    }

    /// One index batch: at most `PAGE` pending keys. Search commit precedes
    /// clearing durable pending work; only the indexed version is cleared.
    pub fn refresh_index(&mut self, control: &crate::Control) -> FResult<usize> {
        if let Some(reason) = &self.repair_reason {
            return Err(FoundryError::RepairRequired(reason.clone()));
        }
        // Field-disjoint borrows: the read transaction owns self.db while the
        // writer mutably owns self.search.
        let tx = self.db.begin_read()?;
        let pending: Vec<(String, String)> = tx
            .open_table(PENDING)?
            .iter()?
            .take(PAGE)
            .map(|row| {
                let (k, v) = row?;
                Ok((k.value().into(), v.value().into()))
            })
            .collect::<Result<Vec<(String, String)>, redb::StorageError>>()
            .map_err(FoundryError::from)?;
        if pending.is_empty() {
            return Ok(0);
        }
        let sources = tx.open_table(SOURCES)?;
        let stored = tx.open_table(CHUNKS)?;
        let Some(handles) = self.search.as_mut() else {
            return Err(FoundryError::RepairRequired(
                "derived index unavailable".into(),
            ));
        };
        for (path, _) in &pending {
            handles
                .writer
                .delete_term(Term::from_field_text(handles.fields.path, path));
            if let Some(source) = sources.get(path.as_str())? {
                let source: SourceMeta = decode(source.value(), "source")?;
                for i in 0..source.chunks {
                    let key = chunk_key(path, i);
                    let raw = stored.get(key.as_str())?.ok_or_else(|| {
                        FoundryError::CorruptSource(format!("chunk missing for {path}"))
                    })?;
                    let chunk: Chunk = decode(raw.value(), "chunk")?;
                    handles.writer.add_document(doc!(
                        handles.fields.key => key,
                        handles.fields.path => path.clone(),
                        handles.fields.hash => source.hash.clone(),
                        handles.fields.body => chunk.body
                    ))?;
                }
            }
        }
        handles.writer.commit()?;
        handles.reader.reload()?;
        drop(stored);
        drop(sources);
        drop(tx);
        // Boundary: search documents committed, pending work not yet cleared.
        fault!(
            INDEX_AFTER_SEARCH_COMMIT,
            Some(&*self),
            Some(control),
            &pending.len().to_string()
        )?;
        // Cooperative checkpoint between search commit and pending clear:
        // cancellation here leaves committed search plus durable pending work,
        // and replay is idempotent.
        control.check()?;
        let tx = self.db.begin_write()?;
        {
            let mut table = tx.open_table(PENDING)?;
            for (path, hash) in &pending {
                let matches = table.get(path.as_str())?.is_some_and(|v| v.value() == hash);
                if matches {
                    table.remove(path.as_str())?;
                }
            }
        }
        tx.commit()?;
        Ok(pending.len())
    }

    /// Drain pending index work in bounded batches until empty or cancelled.
    pub fn refresh(&mut self, control: &crate::Control) -> FResult<usize> {
        let mut total = 0usize;
        loop {
            // Cooperative cancellation between index batches.
            control.check()?;
            match self.refresh_index(control) {
                Ok(0) => return Ok(total),
                Ok(n) => total += n,
                Err(FoundryError::Cancelled(_) | FoundryError::DeadlineExceeded(_)) => {
                    return Err(FoundryError::Cancelled(None));
                }
                Err(e) => return Err(e),
            }
        }
    }

    pub fn search(&self, query: &str, limit: usize) -> FResult<SearchOutcome> {
        if query.trim().is_empty() || query.len() > 4096 {
            return Err(FoundryError::InvalidArgument(
                "query must contain 1..4096 nonblank bytes".into(),
            ));
        }
        if !(1..=64).contains(&limit) {
            return Err(FoundryError::InvalidArgument("limit must be 1..64".into()));
        }
        let workspace_id = self.require_workspace_id()?;
        let handles = self.require_search()?;
        let parser = QueryParser::for_index(&handles.index, vec![handles.fields.body]);
        // Literal terms avoid exposing Tantivy's query language as an API.
        let literal = query
            .split_whitespace()
            .map(|part| format!("\"{}\"", part.replace(['\\', '"'], " ")))
            .collect::<Vec<_>>()
            .join(" ");
        let parsed = tantivy::query::BooleanQuery::union(vec![
            parser
                .parse_query(&literal)
                .map_err(|e| FoundryError::InvalidArgument(format!("query: {e}")))?,
            Box::new(tantivy::query::BoostQuery::new(
                Box::new(tantivy::query::TermQuery::new(
                    Term::from_field_text(handles.fields.path, query.trim()),
                    tantivy::schema::IndexRecordOption::Basic,
                )),
                100.0,
            )),
        ]);
        let searcher = handles.reader.searcher();
        let scored = searcher
            .search(
                &parsed,
                &TopDocs::with_limit(CANDIDATE_LIMIT).order_by_score(),
            )
            .map_err(FoundryError::from)?;
        let candidate_limit_reached = scored.len() >= CANDIDATE_LIMIT;
        let tx = self.db.begin_read()?;
        let sources = tx.open_table(SOURCES)?;
        let stored = tx.open_table(CHUNKS)?;
        // Phase 1: drop stale candidates by source hash. A replaced source may
        // have fewer chunks, so a stale candidate's chunk row may be gone.
        let mut current: Vec<(f32, String, usize, SourceMeta)> = Vec::new();
        let mut stale = 0u64;
        for (score, address) in scored {
            let doc: TantivyDocument = searcher.doc(address).map_err(FoundryError::from)?;
            let field = |f| {
                doc.get_first(f)
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| FoundryError::CorruptStore("invalid search document".into()))
            };
            let path = field(handles.fields.path)?;
            let hash = field(handles.fields.hash)?;
            let key = field(handles.fields.key)?;
            let Some(meta_raw) = sources.get(path)? else {
                stale += 1;
                continue;
            };
            let meta: SourceMeta = decode(meta_raw.value(), "source")?;
            if meta.hash != hash {
                stale += 1;
                continue;
            }
            let ordinal = key
                .rsplit('\0')
                .next()
                .and_then(|o| o.parse::<usize>().ok())
                .filter(|ordinal| chunk_key(path, *ordinal) == key)
                .ok_or_else(|| FoundryError::CorruptStore("invalid chunk key".into()))?;
            current.push((score, path.to_owned(), ordinal, meta));
        }
        // Deterministic order: score, then source path, then byte start (the
        // chunk ordinal is monotonic with the byte start within a path).
        current.sort_by(|a, b| {
            b.0.total_cmp(&a.0)
                .then_with(|| a.1.cmp(&b.1))
                .then_with(|| a.2.cmp(&b.2))
        });
        let truncated_by_limit = current.len() > limit;
        current.truncate(limit);
        // Phase 2: transaction-local verified reconstruction, once per
        // distinct selected source, before any span is emitted.
        let mut by_path: std::collections::BTreeMap<String, Vec<(f32, usize, SourceMeta)>> =
            std::collections::BTreeMap::new();
        for (score, path, ordinal, meta) in current {
            by_path
                .entry(path)
                .or_default()
                .push((score, ordinal, meta));
        }
        let mut hits: Vec<(f32, Hit)> = Vec::new();
        for (path, wanted) in by_path {
            let verified = reconstruct_verified(&stored, &path, &wanted[0].2)?;
            for (score, ordinal, meta) in wanted {
                let Some(hit) = verified.hit(&workspace_id, &path, &meta.hash, ordinal) else {
                    return Err(FoundryError::CorruptSource(format!(
                        "{path}: chunk ordinal {ordinal} is outside the stored chunks"
                    )));
                };
                hits.push((score, hit));
            }
        }
        hits.sort_by(|a, b| {
            b.0.total_cmp(&a.0)
                .then_with(|| a.1.path.cmp(&b.1.path))
                .then_with(|| a.1.handle.start.cmp(&b.1.handle.start))
        });
        let freshness = self.freshness_in(&tx)?;
        Ok(SearchOutcome {
            workspace_id,
            source_revision: freshness.source_revision,
            hits: hits.into_iter().map(|(_, h)| h).collect(),
            pending_sources: freshness.pending_sources,
            stale_candidates: stale,
            candidate_limit: CANDIDATE_LIMIT,
            candidate_limit_reached,
            truncated: truncated_by_limit || candidate_limit_reached,
            scan_state: freshness.scan_state,
        })
    }

    fn freshness_in(&self, tx: &redb::ReadTransaction) -> FResult<Freshness> {
        let meta = tx.open_table(META)?;
        let revision = read_counter(&meta, "source_revision")?;
        let scan_state = read_scan_state(&meta)?;
        Ok(Freshness {
            workspace_id: self.workspace_id.clone().unwrap_or_default(),
            source_revision: revision,
            scan_state: scan_state.clone(),
            pending_sources: tx.open_table(PENDING)?.len()?,
            indexed_snapshot: format!("revision={revision}; scan={scan_state}"),
        })
    }

    /// The verified chunk that follows each given hit, in one read
    /// transaction. These are the remaining source spans of a context bundle.
    fn following_chunks(&self, after: &[Hit]) -> FResult<Vec<Evidence>> {
        let tx = self.db.begin_read()?;
        let sources = tx.open_table(SOURCES)?;
        let stored = tx.open_table(CHUNKS)?;
        let mut out = Vec::new();
        for hit in after {
            let Some(raw) = sources.get(hit.path.as_str())? else {
                continue;
            };
            let meta: SourceMeta = decode(raw.value(), "source")?;
            if meta.hash != hit.handle.sha256 {
                continue;
            }
            let verified = reconstruct_verified(&stored, &hit.path, &meta)?;
            let Some(ordinal) = verified
                .offsets
                .iter()
                .position(|(start, _)| *start as u64 == hit.handle.start)
            else {
                continue;
            };
            if let Some(next) =
                verified.hit(&hit.handle.workspace_id, &hit.path, &meta.hash, ordinal + 1)
            {
                out.push(Evidence::source(next));
            }
        }
        Ok(out)
    }

    /// Build ordered context candidates, then revalidate every candidate
    /// against one final authoritative read transaction. Sources come first
    /// (highest ranked), then bounded graph evidence, then remaining source
    /// spans; a fitting highest-ranked source is never crowded out by graph
    /// annotations. Packing happens at the boundary.
    pub fn context(
        &self,
        query: &str,
        tokens: usize,
        strategy: Strategy,
        control: &crate::Control,
    ) -> FResult<ContextOutcome> {
        if query.trim().is_empty() || query.len() > 4096 {
            return Err(FoundryError::InvalidArgument(
                "query must contain 1..4096 nonblank bytes".into(),
            ));
        }
        if !(1..=32768).contains(&tokens) {
            return Err(FoundryError::InvalidArgument(
                "token budget must be 1..32768".into(),
            ));
        }
        let resolved = match strategy {
            Strategy::Auto => response::strategy_for_query(query),
            explicit => explicit,
        };
        let search = self.search(query, 32)?;
        control.check()?;
        let mut candidates: Vec<Evidence> = Vec::new();
        let mut seen_keys = std::collections::BTreeSet::new();
        let mut push = |evidence: Evidence| {
            if seen_keys.insert(evidence.dedup_key()) {
                candidates.push(evidence);
            }
        };
        for hit in &search.hits {
            push(Evidence::source(hit.clone()));
        }
        let mut graph_reason: Option<&'static str> = None;
        let mut graph_candidates = 0usize;
        if resolved == Strategy::Graph {
            let mut seeds: Vec<&str> = Vec::new();
            for hit in &search.hits {
                if seeds.len() < 3 && !seeds.contains(&hit.path.as_str()) {
                    seeds.push(&hit.path);
                }
            }
            let (mut fresh_edges, mut stale_edges, mut invalid) = (0usize, 0usize, false);
            for path in seeds {
                for reverse in [false, true] {
                    match self.graph(path, reverse, 1, 32) {
                        Ok(graph) => {
                            fresh_edges += graph.edges.len();
                            stale_edges += graph.stale_edges;
                            for evidence in graph.edges {
                                graph_candidates += 1;
                                push(Evidence::graph(&evidence));
                            }
                        }
                        // Component-local: baseline source context survives.
                        // Database errors keep their own named codes.
                        Err(FoundryError::GraphInvalid(_)) => invalid = true,
                        Err(other) => return Err(other),
                    }
                    control.check()?;
                }
            }
            graph_reason = if invalid {
                Some("graph_invalid")
            } else if fresh_edges > 0 {
                None
            } else if stale_edges > 0 {
                Some("graph_stale")
            } else {
                Some("graph_unavailable")
            };
        }
        let top: Vec<Hit> = search.hits.iter().take(3).cloned().collect();
        for evidence in self.following_chunks(&top)? {
            push(evidence);
        }
        control.check()?;
        // Candidates are collected. Anything may commit before the final read.
        fault!(
            CONTEXT_BEFORE_FINAL_VALIDATION,
            Some(self),
            Some(control),
            ""
        )?;
        let tx = self.db.begin_read()?;
        let sources = tx.open_table(SOURCES)?;
        let current_hash = |path: &str| -> FResult<Option<String>> {
            match sources.get(path)? {
                Some(raw) => Ok(Some(decode::<SourceMeta>(raw.value(), "source")?.hash)),
                None => Ok(None),
            }
        };
        let mut kept = Vec::with_capacity(candidates.len());
        let mut stale_dropped = 0u64;
        let mut graph_dropped = 0usize;
        for candidate in candidates {
            let valid = match &candidate {
                Evidence::Source { handle, .. } => {
                    current_hash(&handle.path)?.is_some_and(|hash| hash == handle.sha256)
                }
                Evidence::Graph {
                    raw,
                    from_path,
                    endpoints,
                    ..
                } => {
                    let mut valid = graph::edge_row_present(&tx, from_path, raw)?;
                    for (path, hash) in endpoints {
                        valid &= current_hash(path)?.is_some_and(|current| current == *hash);
                    }
                    valid
                }
            };
            if valid {
                kept.push(candidate);
            } else {
                stale_dropped += 1;
                if matches!(candidate, Evidence::Graph { .. }) {
                    graph_dropped += 1;
                }
            }
        }
        let graph_kept = kept
            .iter()
            .any(|candidate| matches!(candidate, Evidence::Graph { .. }));
        if resolved == Strategy::Graph
            && graph_reason.is_none()
            && graph_dropped > 0
            && graph_candidates > 0
            && !graph_kept
        {
            graph_reason = Some("graph_stale");
        }
        let freshness = self.freshness_in(&tx)?;
        drop(sources);
        drop(tx);
        Ok(ContextOutcome {
            query: query.to_owned(),
            requested_tokens: tokens,
            strategy: resolved,
            graph_reason,
            freshness,
            candidates: kept,
            stale_candidates: search.stale_candidates + stale_dropped,
            candidate_limit: CANDIDATE_LIMIT,
            candidate_limit_reached: search.candidate_limit_reached,
            search_truncated: search.truncated,
        })
    }

    /// Direct authoritative read: field validation, workspace match, source
    /// existence, source hash, then range — each against one final read
    /// transaction. Never touches the derived index.
    pub fn retrieve(&self, handle_json: &str, tokens: usize) -> FResult<RetrieveOutcome> {
        if !(1..=32768).contains(&tokens) {
            return Err(FoundryError::InvalidArgument(
                "token budget must be 1..32768".into(),
            ));
        }
        let handle = SourceHandle::from_json(handle_json)?;
        let bound = self.require_workspace_id()?;
        if handle.workspace_id != bound {
            return Err(FoundryError::WrongWorkspace);
        }
        // Boundary after validation, before the final authoritative read.
        fault!(RETRIEVE_BEFORE_FINAL_READ, Some(self), None, &handle.path)?;
        let tx = self.db.begin_read()?;
        let sources = tx.open_table(SOURCES)?;
        let stored = tx.open_table(CHUNKS)?;
        let Some(raw) = sources.get(handle.path.as_str())? else {
            return Err(FoundryError::NotFound);
        };
        let meta: SourceMeta = decode(raw.value(), "source")?;
        if meta.hash != handle.sha256 {
            return Err(FoundryError::StaleHandle);
        }
        let verified = reconstruct_verified(&stored, &handle.path, &meta)?;
        let body = &verified.body;
        let len = body.len() as u64;
        let valid_empty = handle.start == 0 && handle.end == 0 && meta.bytes == 0;
        let valid_span = handle.start < handle.end
            && handle.end <= len
            && body.is_char_boundary(handle.start as usize)
            && body.is_char_boundary(handle.end as usize);
        if !valid_empty && !valid_span {
            return Err(FoundryError::InvalidRange);
        }
        // Boundaries were validated above; slicing the byte view is equivalent.
        let span = body.as_bytes()[handle.start as usize..handle.end as usize].to_vec();
        let freshness = self.freshness_in(&tx)?;
        drop(stored);
        drop(sources);
        drop(tx);
        Ok(RetrieveOutcome {
            requested: handle,
            requested_tokens: tokens,
            span,
            source_bytes: len,
            freshness,
        })
    }

    fn persist_marker(&self, marker: &RebuildMarker) -> FResult<()> {
        let tx = self.db.begin_write()?;
        tx.open_table(META)?.insert(
            "search_rebuild_required",
            serde_json::to_string(marker)?.as_str(),
        )?;
        tx.commit()?;
        Ok(())
    }

    /// Explicit derived-index repair under exclusive ownership: durable
    /// marker, one retained quarantine, paged enqueue, drained replacement.
    pub fn repair_index(store_dir: &Path, control: &crate::Control) -> FResult<RepairReport> {
        control.check()?;
        let mut engine = Self::open_authoritative(store_dir)?;
        if engine.schema != SCHEMA_VERSION {
            return Err(FoundryError::UpgradeRequired {
                found: engine.schema.to_string(),
            });
        }
        let existing = Self::read_marker(&engine.db)?;
        let search_dir = store_dir.join("search");
        let mut marker = existing.unwrap_or_else(|| {
            let nonce = format!(
                "{}:{}:{}",
                std::process::id(),
                engine.directory.display(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_nanos())
                    .unwrap_or(0)
            );
            RebuildMarker {
                owner_id: crate::digest(nonce.as_bytes()),
                quarantine: None,
                original_handled: false,
            }
        });
        // Decide the quarantine action BEFORE persisting a marker, so a refused
        // path (symlink, conflicting quarantine) leaves a healthy index usable.
        enum Quarantine {
            Done,
            Move { basename: String },
            AdoptCompletedMove,
            NothingToRetain,
        }
        let quarantine = if marker.original_handled {
            Quarantine::Done
        } else {
            if search_dir.is_symlink() {
                return Err(FoundryError::RepairPathConflict(
                    "search path is a symlink".into(),
                ));
            }
            let basename = marker
                .quarantine
                .clone()
                .unwrap_or_else(|| format!("search.quarantine-{}", &marker.owner_id[..12]));
            let search_exists = search_dir.exists();
            let target_exists = store_dir.join(&basename).symlink_metadata().is_ok();
            match (search_exists, target_exists) {
                (true, false) => Quarantine::Move { basename },
                // The rename finished before an earlier crash: adopt it.
                (false, true) if marker.quarantine.is_some() => Quarantine::AdoptCompletedMove,
                // No original derived directory existed: nothing to retain.
                (false, false) => Quarantine::NothingToRetain,
                _ => {
                    return Err(FoundryError::RepairPathConflict(format!(
                        "quarantine {basename} conflicts with the current search path"
                    )));
                }
            }
        };
        // Persist the marker before touching any derived file.
        engine.persist_marker(&marker)?;
        fault!(REPAIR_AFTER_MARKER, Some(&engine), Some(control), "")?;
        control.check()?;
        match quarantine {
            Quarantine::Done => {}
            Quarantine::Move { basename } => {
                // Record the move as intent first, so a crash on either side
                // of the rename is recognizable on retry.
                marker.quarantine = Some(basename.clone());
                engine.persist_marker(&marker)?;
                fault!(
                    REPAIR_BEFORE_QUARANTINE_RENAME,
                    Some(&engine),
                    Some(control),
                    ""
                )?;
                std::fs::rename(&search_dir, store_dir.join(&basename)).map_err(|e| {
                    FoundryError::RepairPathConflict(format!("quarantine move failed: {e}"))
                })?;
                fault!(
                    REPAIR_AFTER_QUARANTINE_RENAME,
                    Some(&engine),
                    Some(control),
                    ""
                )?;
                marker.original_handled = true;
                engine.persist_marker(&marker)?;
            }
            Quarantine::AdoptCompletedMove => {
                marker.original_handled = true;
                engine.persist_marker(&marker)?;
            }
            Quarantine::NothingToRetain => {
                marker.quarantine = None;
                marker.original_handled = true;
                engine.persist_marker(&marker)?;
            }
        }
        let quarantined_to = marker.quarantine.as_ref().map(|b| store_dir.join(b));
        control.check()?;
        // The replacement directory is reset only when it positively carries
        // this repair's identity: a regular `rebuild_id` file whose nonempty
        // content equals the marker's owner id. A completely empty directory
        // holds nothing to lose (a crash between creating it and writing the
        // identity) and is adopted. Everything else, including a missing,
        // empty, malformed or symlinked identity beside other files, is a
        // conflict: nothing is moved or deleted.
        let id_file = search_dir.join("rebuild_id");
        if search_dir.is_symlink() {
            return Err(FoundryError::RepairPathConflict(
                "replacement search path is a symlink".into(),
            ));
        }
        if search_dir.exists() {
            let matched = std::fs::symlink_metadata(&id_file)
                .ok()
                .filter(|meta| meta.file_type().is_file())
                .and_then(|_| std::fs::read_to_string(&id_file).ok())
                .is_some_and(|content| {
                    let content = content.trim();
                    !content.is_empty() && content == marker.owner_id
                });
            let empty = std::fs::read_dir(&search_dir)
                .map(|mut entries| entries.next().is_none())
                .unwrap_or(false);
            if matched {
                for entry in std::fs::read_dir(&search_dir)? {
                    let entry = entry?;
                    if entry.file_name() == "rebuild_id" {
                        continue;
                    }
                    // file_type() does not follow symlinks: a link is removed
                    // as a link, never traversed.
                    if entry.file_type()?.is_dir() {
                        std::fs::remove_dir_all(entry.path())?;
                    } else {
                        std::fs::remove_file(entry.path())?;
                    }
                }
            } else if empty {
                std::fs::write(&id_file, &marker.owner_id)?;
            } else {
                return Err(FoundryError::RepairPathConflict(
                    "replacement directory does not carry this repair's identity".into(),
                ));
            }
        } else {
            std::fs::create_dir_all(&search_dir)?;
            std::fs::write(&id_file, &marker.owner_id)?;
        }
        // Enqueue every source version in pages.
        let mut after: Option<String> = None;
        loop {
            control.check()?;
            let page: Vec<(String, String)> = {
                let tx = engine.db.begin_read()?;
                let sources = tx.open_table(SOURCES)?;
                let rows = match &after {
                    None => sources.range::<&str>(..)?,
                    Some(key) => {
                        sources.range::<&str>((Bound::Excluded(key.as_str()), Bound::Unbounded))?
                    }
                };
                rows.take(PAGE)
                    .map(|row| {
                        let (k, v) = row?;
                        Ok((k.value().to_owned(), v.value().to_owned()))
                    })
                    .collect::<Result<Vec<(String, String)>, redb::StorageError>>()
                    .map_err(FoundryError::from)?
            };
            if page.is_empty() {
                break;
            }
            let tx = engine.db.begin_write()?;
            {
                let mut pending = tx.open_table(PENDING)?;
                for (path, hash) in &page {
                    pending.insert(path.as_str(), hash.as_str())?;
                }
            }
            tx.commit()?;
            fault!(REPAIR_AFTER_ENQUEUE_PAGE, Some(&engine), Some(control), "")?;
            after = page.last().map(|(k, _)| k.clone());
        }
        control.check()?;
        // Construct the replacement index and drain.
        let index = Index::create_in_dir(&search_dir, search_schema())?;
        let fields = fields_of(&index.schema());
        let reader = index
            .reader_builder()
            .reload_policy(ReloadPolicy::Manual)
            .try_into()?;
        let writer = index.writer_with_num_threads(1, 20_000_000)?;
        engine.search = Some(SearchHandles {
            index,
            reader,
            writer,
            fields,
        });
        engine.repair_reason = None;
        let mut drained = 0usize;
        loop {
            control.check()?;
            match engine.refresh_index(control)? {
                0 => break,
                n => drained += n,
            }
        }
        // Clear the marker only after successful index commit/reload and an
        // empty pending table. Cancellation here leaves the marker set.
        if engine.pending()? != 0 {
            return Err(FoundryError::RepairRequired(
                "pending work remains after drain; marker retained".into(),
            ));
        }
        fault!(REPAIR_BEFORE_MARKER_CLEAR, Some(&engine), Some(control), "")?;
        control.check()?;
        {
            let tx = engine.db.begin_write()?;
            tx.open_table(META)?.remove("search_rebuild_required")?;
            tx.commit()?;
        }
        Ok(RepairReport {
            repaired: true,
            quarantined_to,
            drained_sources: drained,
            reason: None,
        })
    }

    pub fn directory(&self) -> &Path {
        &self.directory
    }
}
/// The expected workspace identity for a root: lowercase SHA-256 of the
/// canonical absolute root's UTF-8 bytes. One definition for bind, status and
/// handle checks; callers never mirror the rule.
pub fn workspace_id_for_root(root: &Path) -> FResult<String> {
    let canonical = root
        .canonicalize()
        .map_err(|e| FoundryError::InvalidArgument(format!("workspace root: {e}")))?;
    canonical
        .to_str()
        .map(|s| crate::digest(s.as_bytes()))
        .ok_or_else(|| FoundryError::InvalidArgument("workspace path is not UTF-8".into()))
}

fn bump_revision(tx: &WriteTransaction, by: u64) -> FResult<()> {
    let mut meta = tx.open_table(META)?;
    let current = read_counter(&meta, "source_revision")?;
    let next = current
        .checked_add(by)
        .ok_or(FoundryError::RevisionExhausted)?;
    meta.insert("source_revision", next.to_string().as_str())?;
    Ok(())
}

impl Engine {
    /// Explicit indexing entry point shared by CLI and MCP.
    pub fn index(
        &mut self,
        root: &Path,
        control: &crate::Control,
    ) -> FResult<crate::ingest::IndexReport> {
        crate::ingest::index(self, root, control)
    }
}
