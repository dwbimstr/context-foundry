//! Test support compiled only with the `test-faults` feature: on-disk fixtures,
//! authoritative-state snapshots and tamper helpers shared by the crate's
//! integration tests. Absent from every default and release build.
//!
//! `craft_v1_store` reproduces the schema-1 layout the previous binary wrote:
//! redb tables with a "1" schema marker, a Tantivy `search/` directory, and
//! committed-but-unindexed work in the pending table.
use redb::{
    Database, MultimapTableDefinition, ReadableDatabase, ReadableMultimapTable, ReadableTable,
    TableDefinition,
};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use tantivy::schema::{STORED, STRING, Schema, TEXT};

const SOURCES: TableDefinition<&str, &str> = TableDefinition::new("sources");
const CHUNKS: TableDefinition<&str, &str> = TableDefinition::new("chunks");
const PENDING: TableDefinition<&str, &str> = TableDefinition::new("pending_index");
const META: TableDefinition<&str, &str> = TableDefinition::new("meta");
const FEEDBACK: TableDefinition<&str, &str> = TableDefinition::new("feedback");

pub const KEPT_BODY: &str = "kept\n";

/// Build a v1 store holding one source (`kept.rs`, pending index work) and one
/// opted-in feedback record. `root` binds the workspace when present.
pub fn craft_v1_store(dir: &Path, root: Option<&Path>) {
    std::fs::create_dir_all(dir).unwrap();
    let out: MultimapTableDefinition<&str, &str> = MultimapTableDefinition::new("edges_out");
    let inc: MultimapTableDefinition<&str, &str> = MultimapTableDefinition::new("edges_in");
    let providers: TableDefinition<&str, &str> = TableDefinition::new("provider_bundles");
    let db = Database::create(dir.join("knowledge.redb")).unwrap();
    let hash = crate::digest(KEPT_BODY.as_bytes());
    let tx = db.begin_write().unwrap();
    {
        let mut meta = tx.open_table(META).unwrap();
        meta.insert("schema", "1").unwrap();
        if let Some(root) = root {
            std::fs::create_dir_all(root).unwrap();
            meta.insert("workspace", root.canonicalize().unwrap().to_str().unwrap())
                .unwrap();
        }
        let mut sources = tx.open_table(SOURCES).unwrap();
        let source = serde_json::json!({
            "hash": hash,
            "chunks": 1,
            "bytes": KEPT_BODY.len(),
            "lines": 1,
        });
        sources
            .insert("kept.rs", source.to_string().as_str())
            .unwrap();
        let mut chunks = tx.open_table(CHUNKS).unwrap();
        let chunk = serde_json::json!({
            "path": "kept.rs",
            "hash": hash,
            "start_line": 1,
            "end_line": 1,
            "body": KEPT_BODY,
        });
        chunks
            .insert("kept.rs\u{0}0000000000", chunk.to_string().as_str())
            .unwrap();
        tx.open_table(PENDING)
            .unwrap()
            .insert("kept.rs", hash.as_str())
            .unwrap();
        let mut feedback = tx.open_table(FEEDBACK).unwrap();
        let record = serde_json::json!({
            "task_id": "legacy",
            "query": "legacy query",
            "correct_strategy": "search",
            "label_source": "operator",
            "allow_training": true,
        });
        feedback
            .insert("legacy-id", record.to_string().as_str())
            .unwrap();
        tx.open_multimap_table(out).unwrap();
        tx.open_multimap_table(inc).unwrap();
        tx.open_table(providers).unwrap();
    }
    tx.commit().unwrap();
    drop(db);
    // The v1 binary always created its derived index next to the database.
    let mut schema = Schema::builder();
    schema.add_text_field("key", STRING | STORED);
    schema.add_text_field("path", STRING | STORED);
    schema.add_text_field("hash", STRING | STORED);
    schema.add_text_field("body", TEXT);
    std::fs::create_dir_all(dir.join("search")).unwrap();
    tantivy::Index::create_in_dir(dir.join("search"), schema.build()).unwrap();
}

pub fn schema_marker(dir: &Path) -> String {
    let db = Database::open(dir.join("knowledge.redb")).unwrap();
    let tx = db.begin_read().unwrap();
    tx.open_table(META)
        .unwrap()
        .get("schema")
        .unwrap()
        .unwrap()
        .value()
        .to_owned()
}

/// Bytes written over the derived index metadata to simulate corruption.
pub const CORRUPT_INDEX_BYTES: &[u8] = b"corrupt derived index bytes";

/// Corrupt the derived index in place: the directory stays, its metadata is
/// garbage, so a repair has an original to quarantine.
pub fn corrupt_search_index(store: &Path) {
    std::fs::write(store.join("search").join("meta.json"), CORRUPT_INDEX_BYTES).unwrap();
}

/// Quarantine siblings next to the database, by path.
pub fn quarantine_dirs(store: &Path) -> Vec<std::path::PathBuf> {
    std::fs::read_dir(store)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| {
            p.file_name()
                .is_some_and(|n| n.to_string_lossy().starts_with("search.quarantine-"))
        })
        .collect()
}

/// Every authoritative table of a store as ordered `(key, value)` rows,
/// multimaps included. The store must not be open.
pub type Snapshot = BTreeMap<&'static str, Vec<(String, String)>>;

/// Tables whose rows are authoritative knowledge (not scan/pending/meta bookkeeping).
pub const KNOWLEDGE_TABLES: [&str; 7] = [
    "sources",
    "chunks",
    "feedback",
    "provider_bundles",
    "edges_out",
    "edges_in",
    "memory",
];

const TABLES: [&str; 9] = [
    "sources",
    "chunks",
    "pending_index",
    "meta",
    "feedback",
    "scan_seen",
    "provider_bundles",
    "edges_out",
    "memory",
];

pub fn snapshot(dir: &Path) -> Snapshot {
    let db = Database::open(dir.join("knowledge.redb")).unwrap();
    let tx = db.begin_read().unwrap();
    let mut snapshot = Snapshot::new();
    for name in TABLES.iter().copied().chain(["edges_in"]) {
        let mut rows = Vec::new();
        let plain: TableDefinition<&str, &str> = TableDefinition::new(name);
        if let Ok(table) = tx.open_table(plain) {
            for row in table.iter().unwrap() {
                let (k, v) = row.unwrap();
                rows.push((k.value().to_owned(), v.value().to_owned()));
            }
        } else {
            let multi: MultimapTableDefinition<&str, &str> = MultimapTableDefinition::new(name);
            if let Ok(table) = tx.open_multimap_table(multi) {
                for entry in table.iter().unwrap() {
                    let (k, values) = entry.unwrap();
                    for v in values {
                        rows.push((k.value().to_owned(), v.unwrap().value().to_owned()));
                    }
                }
            }
        }
        snapshot.insert(name, rows);
    }
    snapshot
}

/// Only the rows that are authoritative knowledge.
pub fn knowledge(snapshot: &Snapshot) -> Snapshot {
    snapshot
        .iter()
        .filter(|(name, _)| KNOWLEDGE_TABLES.contains(name))
        .map(|(name, rows)| (*name, rows.clone()))
        .collect()
}

/// Run a write transaction against a closed store.
pub fn write_store<R>(dir: &Path, f: impl FnOnce(&redb::WriteTransaction) -> R) -> R {
    let db = Database::create(dir.join("knowledge.redb")).unwrap();
    let tx = db.begin_write().unwrap();
    let result = f(&tx);
    tx.commit().unwrap();
    result
}

/// Set (or with `None` remove) one `meta` row of a closed store.
pub fn set_meta(dir: &Path, key: &str, value: Option<&str>) {
    write_store(dir, |tx| {
        let mut meta = tx.open_table(META).unwrap();
        match value {
            Some(value) => {
                meta.insert(key, value).unwrap();
            }
            None => {
                meta.remove(key).unwrap();
            }
        }
    });
}

pub fn chunk_key(path: &str, ordinal: usize) -> String {
    format!("{path}\0{ordinal:010}")
}

/// Rewrite one stored chunk's `body` keeping its JSON shape, path and hash.
pub fn tamper_chunk_body(dir: &Path, path: &str, ordinal: usize, body: &str) {
    write_store(dir, |tx| {
        let mut chunks = tx.open_table(CHUNKS).unwrap();
        let key = chunk_key(path, ordinal);
        let mut value: serde_json::Value =
            serde_json::from_str(chunks.get(key.as_str()).unwrap().unwrap().value()).unwrap();
        value["body"] = body.into();
        chunks
            .insert(key.as_str(), value.to_string().as_str())
            .unwrap();
    });
}

pub fn remove_chunk(dir: &Path, path: &str, ordinal: usize) {
    write_store(dir, |tx| {
        tx.open_table(CHUNKS)
            .unwrap()
            .remove(chunk_key(path, ordinal).as_str())
            .unwrap();
    });
}

/// Insert an arbitrary (possibly undecodable) edge row under `from_path`.
pub fn insert_raw_edge(dir: &Path, from_path: &str, raw: &str) {
    write_store(dir, |tx| {
        let out: MultimapTableDefinition<&str, &str> = MultimapTableDefinition::new("edges_out");
        tx.open_multimap_table(out)
            .unwrap()
            .insert(from_path, raw)
            .unwrap();
    });
}

/// Directory entries of a store directory other than the database itself.
pub fn store_entries(store: &Path) -> Vec<PathBuf> {
    let mut entries: Vec<PathBuf> = std::fs::read_dir(store)
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect();
    entries.sort();
    entries
}

/// A uniquely named scratch directory removed on drop (canonical path, so
/// pathname-based fault hooks see the same path the scanner binds).
pub struct Scratch {
    path: PathBuf,
}

impl Scratch {
    pub fn new() -> Self {
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos());
        let path = std::env::temp_dir().join(format!(
            "foundry-test-{}-{nanos}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::SeqCst)
        ));
        std::fs::create_dir_all(&path).unwrap();
        Self {
            path: path.canonicalize().unwrap(),
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Default for Scratch {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

/// A fresh explicitly initialized store bound to `<fixture>/ws`.
pub struct Fixture {
    pub dir: Scratch,
    pub store: PathBuf,
    pub root: PathBuf,
    pub engine: crate::Engine,
}

pub fn new_fixture() -> Fixture {
    let dir = Scratch::new();
    let root = dir.path().join("ws");
    std::fs::create_dir(&root).unwrap();
    let store = dir.path().join("store");
    let engine = crate::Engine::initialize(&store, &root).unwrap();
    Fixture {
        dir,
        store,
        root,
        engine,
    }
}

impl Fixture {
    pub fn drain(&mut self) {
        self.engine.refresh(&crate::Control::unbounded()).unwrap();
    }

    /// Add sources and drain the derived index.
    pub fn add(&mut self, sources: &[(&str, &str)]) {
        for (path, body) in sources {
            self.engine.replace_source(path, body).unwrap();
        }
        self.drain();
    }

    /// Close the engine (releasing the store lock) and return the store path.
    pub fn close(self) -> (Scratch, PathBuf, PathBuf) {
        let Fixture {
            dir, store, root, ..
        } = self;
        (dir, store, root)
    }
}

/// The MCP success serializer shape: one text block, `isError:false`,
/// no `structuredContent`.
pub fn mcp_success(application: &str) -> String {
    serde_json::json!({"content": [{"type": "text", "text": application}], "isError": false})
        .to_string()
}

/// The MCP error serializer shape.
pub fn mcp_error(application: &str) -> String {
    serde_json::json!({"content": [{"type": "text", "text": application}], "isError": true})
        .to_string()
}

/// Build a schema-2 store as the previous binary wrote it: one source
/// (`kept.rs`) whose pending index work is under the UNTYPED raw-path key, one
/// opted-in feedback record and no memory state — the exact input of the
/// v2 -> v3 upgrade. `root` binds the workspace (with its `workspace_id`).
pub fn craft_v2_store(dir: &Path, root: Option<&Path>) {
    craft_v1_store(dir, root);
    // A populated graph row: preservation assertions must not be vacuous.
    insert_raw_edge(
        dir,
        "kept.rs",
        r#"{"provider":"fixture","revision":"r1","edge":"kept -> kept"}"#,
    );
    write_store(dir, |tx| {
        let mut meta = tx.open_table(META).unwrap();
        meta.insert("schema", "2").unwrap();
        meta.insert("source_revision", "3").unwrap();
        meta.insert("scan_id", "1").unwrap();
        meta.insert("scan_status", "complete").unwrap();
        let bound = meta.get("workspace").unwrap().map(|v| v.value().to_owned());
        if let Some(bound) = bound {
            meta.insert("workspace_id", crate::digest(bound.as_bytes()).as_str())
                .unwrap();
        }
        tx.open_table(TableDefinition::<&str, &str>::new("scan_seen"))
            .unwrap();
    });
    // Graph, scan_seen and a prefix chain of pending keys: the upgrade must
    // preserve every row and migrate every key, including a raw `source:a`
    // whose typed destination another raw key also maps onto.
    insert_raw_edge(
        dir,
        "kept.rs",
        r#"{"provider":"fixture","revision":"r1","from":{"path":"kept.rs","line":1,"symbol":"kept","hash":"h"},"to":{"path":"kept.rs","line":1,"symbol":"kept","hash":"h"},"kind":"calls","evidence":"manual"}"#,
    );
    write_store(dir, |tx| {
        tx.open_table(TableDefinition::<&str, &str>::new("scan_seen"))
            .unwrap()
            .insert("kept.rs", "1")
            .unwrap();
        tx.open_table(TableDefinition::<&str, &str>::new("provider_bundles"))
            .unwrap()
            .insert("fixture", r#"{"provider":"fixture","revision":"r1"}"#)
            .unwrap();
        let mut pending = tx
            .open_table(TableDefinition::<&str, &str>::new("pending_index"))
            .unwrap();
        pending.insert("a", "A").unwrap();
        pending.insert("source:a", "B").unwrap();
        pending.insert("source:source:a", "C").unwrap();
    });
}

/// Insert an arbitrary (possibly undecodable) row into the `memory` table of a
/// closed store.
pub fn write_raw_memory_row(dir: &Path, id: &str, raw: &str) {
    write_store(dir, |tx| {
        tx.open_table(TableDefinition::<&str, &str>::new("memory"))
            .unwrap()
            .insert(id, raw)
            .unwrap();
    });
}

/// Every `memory` row of a closed store, ordered by id.
pub fn memory_rows(dir: &Path) -> Vec<(String, String)> {
    snapshot(dir).remove("memory").unwrap_or_default()
}

/// Record a different source hash on one stored chunk (a chunk that claims to
/// belong to another version of the source).
pub fn retag_chunk_hash(dir: &Path, path: &str, ordinal: usize, hash: &str) {
    write_store(dir, |tx| {
        let mut chunks = tx.open_table(CHUNKS).unwrap();
        let key = chunk_key(path, ordinal);
        let mut value: serde_json::Value =
            serde_json::from_str(chunks.get(key.as_str()).unwrap().unwrap().value()).unwrap();
        value["hash"] = hash.into();
        chunks
            .insert(key.as_str(), value.to_string().as_str())
            .unwrap();
    });
}

/// One `meta` row of a closed store.
pub fn meta_value(dir: &Path, key: &str) -> Option<String> {
    let db = Database::open(dir.join("knowledge.redb")).unwrap();
    let tx = db.begin_read().unwrap();
    let meta = tx.open_table(META).unwrap();
    meta.get(key).unwrap().map(|v| v.value().to_owned())
}

/// One `pending_index` row of a closed store.
pub fn pending_value(dir: &Path, key: &str) -> Option<String> {
    let db = Database::open(dir.join("knowledge.redb")).unwrap();
    let tx = db.begin_read().unwrap();
    let pending = tx
        .open_table(TableDefinition::<&str, &str>::new("pending_index"))
        .unwrap();
    pending.get(key).unwrap().map(|v| v.value().to_owned())
}

/// What a parsed context-v2 item is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum V2Kind {
    /// An item line followed by a fenced body (context and retrieve).
    Source,
    /// A search locator line, `<handle> L<line>[ <label>]: <excerpt>`.
    Locator,
    /// A graph item line, `edge <text>`.
    Edge,
}

/// One parsed context-v2 item. `body` is the fenced source bytes (framing LF
/// removed), the locator excerpt or the edge text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct V2Item {
    pub kind: V2Kind,
    /// Empty for edges.
    pub handle: String,
    /// `L<a>-<b>` for fenced items, `L<line>` for locators.
    pub lines: Option<String>,
    pub label: Option<String>,
    /// `signature` or `outline` for non-verbatim forms.
    pub form: Option<String>,
    /// The fence info string.
    pub lang: Option<String>,
    pub body: String,
}

/// A parsed context-v2 success: header segments, items and the optional
/// retrieve continuation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct V2Response {
    pub header: Vec<String>,
    pub items: Vec<V2Item>,
    pub next: Option<String>,
}

enum V2Tail {
    Fenced {
        lines: Option<String>,
        label: Option<String>,
        form: Option<String>,
    },
    Locator {
        lines: String,
        label: Option<String>,
        excerpt: String,
    },
}

/// Strict context-v2 parser for tests. Every line ends with LF; line 1 is the
/// ` · `-joined header naming the operation, which fixes the item grammar:
/// search has locator lines only; context has fenced items and `edge` lines;
/// retrieve has fenced items and may end with `next: <handle>` (a valid
/// handle). Item lines take precedence because a path may itself begin with
/// `edge ` or `next: ` (a fenced item is recognized by the opening fence that
/// must follow it). A verbatim body's length comes from its handle's range,
/// then the framing LF (when the body does not end with LF) and the exact
/// closing fence must follow.
///
/// An item line is tried at every `@<32 hex>.<16 hex>` suffix whose handle and
/// remainder are valid for the operation; a fenced reading must also frame its
/// body. Exactly one complete reading is required. A valid path, label or
/// excerpt may embed suffix-lookalike text, and then the wire alone cannot
/// name the item: such a line is refused, never attributed to either handle.
pub fn parse_v2(text: &str) -> Result<V2Response, String> {
    if !text.ends_with('\n') {
        return Err("the text must end with LF".into());
    }
    let (header, mut pos) = v2_line(text, 0)?;
    let Some(op) = header
        .split(" · ")
        .next()
        .and_then(|first| first.strip_prefix("foundry "))
        .filter(|op| matches!(*op, "search" | "context" | "retrieve"))
    else {
        return Err(format!("not a v2 header: {header:?}"));
    };
    let header: Vec<String> = header.split(" · ").map(str::to_owned).collect();
    let mut items = Vec::new();
    let mut next = None;
    while pos < text.len() {
        let (line, after) = v2_line(text, pos)?;
        if op == "search" {
            let mut readings = v2_splits(line, true);
            let (handle, tail) = match readings.len() {
                1 => readings.remove(0),
                0 => return Err(format!("not a search item line: {line:?}")),
                n => return Err(format!("ambiguous item line ({n} readings): {line:?}")),
            };
            let V2Tail::Locator {
                lines,
                label,
                excerpt,
            } = tail
            else {
                return Err(format!("not a locator: {line:?}"));
            };
            items.push(V2Item {
                kind: V2Kind::Locator,
                handle: handle.to_owned(),
                lines: Some(lines),
                label,
                form: None,
                lang: None,
                body: excerpt,
            });
            pos = after;
            continue;
        }
        let readings = if text[after..].starts_with("```") {
            v2_splits(line, false)
        } else {
            Vec::new()
        };
        if !readings.is_empty() {
            let mut complete = Vec::new();
            let mut first_error = None;
            for (handle, tail) in readings {
                match v2_fenced_item(text, handle, tail, after) {
                    Ok(found) => complete.push(found),
                    Err(e) => {
                        first_error.get_or_insert(e);
                    }
                }
            }
            match complete.len() {
                1 => {
                    let (item, end) = complete.remove(0);
                    items.push(item);
                    pos = end;
                    continue;
                }
                0 => return Err(first_error.unwrap_or_default()),
                n => return Err(format!("ambiguous item line ({n} readings): {line:?}")),
            }
        }
        if op == "retrieve"
            && after == text.len()
            && let Some(handle) = line.strip_prefix("next: ")
        {
            crate::store::HandleRef::parse(handle)
                .map_err(|e| format!("malformed continuation {handle:?}: {e}"))?;
            next = Some(handle.to_owned());
            pos = after;
            continue;
        }
        if op == "context"
            && let Some(edge) = line.strip_prefix("edge ")
        {
            items.push(V2Item {
                kind: V2Kind::Edge,
                handle: String::new(),
                lines: None,
                label: None,
                form: None,
                lang: None,
                body: edge.to_owned(),
            });
            pos = after;
            continue;
        }
        return Err(format!("not a {op} item line: {line:?}"));
    }
    Ok(V2Response {
        header,
        items,
        next,
    })
}

/// The line starting at `pos` (without its LF) and the position after its LF.
fn v2_line(text: &str, pos: usize) -> Result<(&str, usize), String> {
    let end = text[pos..]
        .find('\n')
        .map(|offset| pos + offset)
        .ok_or_else(|| format!("unterminated line at byte {pos}"))?;
    Ok((&text[pos..end], end + 1))
}

/// One fenced reading of an item line whose opening fence starts at `after`:
/// the item and the position after its closing fence.
fn v2_fenced_item(
    text: &str,
    handle: &str,
    tail: V2Tail,
    after: usize,
) -> Result<(V2Item, usize), String> {
    let V2Tail::Fenced { lines, label, form } = tail else {
        return Err("not a fenced item".into());
    };
    let (open, body_start) = v2_line(text, after)?;
    let ticks = open.bytes().take_while(|&b| b == b'`').count();
    if ticks < 3 {
        return Err(format!("expected an opening fence, got {open:?}"));
    }
    let fence = &open[..ticks];
    let lang = (ticks < open.len()).then(|| open[ticks..].to_owned());
    let (body, p) = if form.is_none() {
        let range = crate::store::HandleRef::parse(handle).map_err(|e| e.to_string())?;
        let end = usize::try_from(range.end - range.start)
            .ok()
            .and_then(|len| body_start.checked_add(len))
            .filter(|&end| end <= text.len() && text.is_char_boundary(end))
            .ok_or("the verbatim body exceeds the text")?;
        let body = &text[body_start..end];
        let mut p = end;
        if !body.ends_with('\n') {
            if !text[p..].starts_with('\n') {
                return Err("missing framing LF before the closing fence".into());
            }
            p += 1;
        }
        (body, p)
    } else {
        let mut p = body_start;
        loop {
            let (candidate, next_line) = v2_line(text, p)?;
            if candidate == fence {
                break (&text[body_start..p], p);
            }
            p = next_line;
        }
    };
    let (close, after_close) = v2_line(text, p)?;
    if close != fence {
        return Err(format!("expected closing fence {fence:?}, got {close:?}"));
    }
    let item = V2Item {
        kind: V2Kind::Source,
        handle: handle.to_owned(),
        lines,
        label,
        form,
        lang,
        body: body.to_owned(),
    };
    Ok((item, after_close))
}

/// Every reading of `line` as `<handle><tail>`: a split after a
/// `@<32 hex>.<16 hex>` suffix whose handle parses and whose remainder is a
/// locator tail (`locator`) or a fenced item tail (otherwise).
fn v2_splits(line: &str, locator: bool) -> Vec<(&str, V2Tail)> {
    let bytes = line.as_bytes();
    let hex = |s: &[u8]| s.iter().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'));
    let mut readings = Vec::new();
    for (at, _) in line.match_indices('@') {
        let end = at + 50;
        if end > bytes.len()
            || bytes[at + 33] != b'.'
            || !hex(&bytes[at + 1..at + 33])
            || !hex(&bytes[at + 34..end])
            || (end < bytes.len() && bytes[end] != b' ')
        {
            continue;
        }
        let handle = &line[..end];
        if crate::store::HandleRef::parse(handle).is_err() {
            continue;
        }
        match parse_v2_tail(&line[end..]) {
            Some(tail @ V2Tail::Locator { .. }) if locator => readings.push((handle, tail)),
            Some(tail @ V2Tail::Fenced { .. }) if !locator => readings.push((handle, tail)),
            _ => {}
        }
    }
    readings
}

fn parse_v2_tail(tail: &str) -> Option<V2Tail> {
    if tail.is_empty() {
        return Some(V2Tail::Fenced {
            lines: None,
            label: None,
            form: None,
        });
    }
    let rest = tail.strip_prefix(' ')?;
    if let Some(after_l) = rest.strip_prefix('L') {
        let digits = after_l.bytes().take_while(u8::is_ascii_digit).count();
        if digits > 0 {
            let (first, after_first) = after_l.split_at(digits);
            if let Some(range_rest) = after_first.strip_prefix('-') {
                let more = range_rest.bytes().take_while(u8::is_ascii_digit).count();
                if more == 0 {
                    return None;
                }
                let (last, remainder) = range_rest.split_at(more);
                let (label, form) = v2_label_and_form(remainder)?;
                return Some(V2Tail::Fenced {
                    lines: Some(format!("L{first}-{last}")),
                    label,
                    form,
                });
            }
            let (label, excerpt) = match after_first.strip_prefix(": ") {
                Some(excerpt) => (None, excerpt),
                None => {
                    let (label, excerpt) = after_first.strip_prefix(' ')?.split_once(": ")?;
                    (Some(label.to_owned()), excerpt)
                }
            };
            return Some(V2Tail::Locator {
                lines: format!("L{first}"),
                label,
                excerpt: excerpt.to_owned(),
            });
        }
    }
    let (label, form) = v2_label_and_form(tail)?;
    Some(V2Tail::Fenced {
        lines: None,
        label,
        form,
    })
}

fn v2_label_and_form(remainder: &str) -> Option<(Option<String>, Option<String>)> {
    if remainder.is_empty() {
        return Some((None, None));
    }
    let rest = remainder.strip_prefix(' ')?;
    for (tag, form) in [("[signature]", "signature"), ("[outline]", "outline")] {
        if rest == tag {
            return Some((None, Some(form.to_owned())));
        }
        if let Some(label) = rest.strip_suffix(&format!(" {tag}")) {
            return Some((Some(label.to_owned()), Some(form.to_owned())));
        }
    }
    Some((Some(rest.to_owned()), None))
}
