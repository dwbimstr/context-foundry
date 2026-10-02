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
pub const KNOWLEDGE_TABLES: [&str; 6] = [
    "sources",
    "chunks",
    "feedback",
    "provider_bundles",
    "edges_out",
    "edges_in",
];

const TABLES: [&str; 8] = [
    "sources",
    "chunks",
    "pending_index",
    "meta",
    "feedback",
    "scan_seen",
    "provider_bundles",
    "edges_out",
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
