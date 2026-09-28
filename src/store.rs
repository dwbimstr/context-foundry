use anyhow::{Context, Result, bail, ensure};
use redb::{Database, ReadableDatabase, ReadableTable, ReadableTableMetadata, TableDefinition};
use serde::{Deserialize, Serialize};
use std::path::{Component, Path};
use tantivy::collector::TopDocs;
use tantivy::query::QueryParser;
use tantivy::schema::{Field, STORED, STRING, Schema, TEXT, Value};
use tantivy::{Index, IndexReader, IndexWriter, ReloadPolicy, TantivyDocument, Term, doc};

pub(crate) const SOURCES: TableDefinition<&str, &str> = TableDefinition::new("sources");
const CHUNKS: TableDefinition<&str, &str> = TableDefinition::new("chunks");
const PENDING: TableDefinition<&str, &str> = TableDefinition::new("pending_index");
const META: TableDefinition<&str, &str> = TableDefinition::new("meta");
pub(crate) const FEEDBACK: TableDefinition<&str, &str> = TableDefinition::new("feedback");

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

#[derive(Debug, Serialize)]
pub struct SearchResult {
    pub hits: Vec<Chunk>,
    pub pending_sources: u64,
    pub stale_candidates: usize,
    pub candidate_limit: usize,
}

#[derive(Debug, Serialize)]
pub struct ContextBundle {
    pub text: String,
    pub tokens: usize,
    pub tokenizer: &'static str,
    pub omitted: usize,
    pub pending_sources: u64,
}

struct Fields {
    key: Field,
    path: Field,
    hash: Field,
    body: Field,
}

pub struct Engine {
    pub(crate) db: Database,
    pub(crate) directory: std::path::PathBuf,
    index: Index,
    reader: IndexReader,
    writer: IndexWriter,
    fields: Fields,
}

pub fn validate_path(path: &str) -> Result<()> {
    ensure!(
        !path.is_empty() && !path.contains(['\0', '\n', '\r']),
        "invalid source path"
    );
    ensure!(
        Path::new(path)
            .components()
            .all(|p| matches!(p, Component::Normal(_))),
        "source path must be a normalized relative path"
    );
    Ok(())
}

fn chunk_key(path: &str, ordinal: usize) -> String {
    format!("{path}\0{ordinal:010}")
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

impl Engine {
    pub fn open(dir: &Path) -> Result<Self> {
        std::fs::create_dir_all(dir)?;
        let db = Database::create(dir.join("knowledge.redb"))?;
        let tx = db.begin_write()?;
        {
            let mut meta = tx.open_table(META)?;
            if let Some(version) = meta.get("schema")? {
                ensure!(version.value() == "1", "unsupported store schema");
            }
            meta.insert("schema", "1")?;
            tx.open_table(SOURCES)?;
            tx.open_table(CHUNKS)?;
            tx.open_table(PENDING)?;
            tx.open_table(FEEDBACK)?;
            crate::graph::init(&tx)?;
        }
        tx.commit()?;
        let mut schema = Schema::builder();
        let fields = Fields {
            key: schema.add_text_field("key", STRING | STORED),
            path: schema.add_text_field("path", STRING | STORED),
            hash: schema.add_text_field("hash", STRING | STORED),
            body: schema.add_text_field("body", TEXT),
        };
        let schema = schema.build();
        let index_dir = dir.join("search");
        let exists = index_dir.join("meta.json").exists();
        // Queue the rebuild before creating a new index's metadata. Otherwise a
        // crash between creation and enqueue could leave an empty, apparently ready index.
        if !exists {
            let tx = db.begin_write()?;
            {
                let sources = tx.open_table(SOURCES)?;
                let mut pending = tx.open_table(PENDING)?;
                for row in sources.iter()? {
                    let (path, value) = row?;
                    let source: SourceMeta = serde_json::from_str(value.value())?;
                    pending.insert(path.value(), source.hash.as_str())?;
                }
            }
            tx.commit()?;
        }
        std::fs::create_dir_all(&index_dir)?;
        let index = if exists {
            Index::open_in_dir(&index_dir)?
        } else {
            Index::create_in_dir(&index_dir, schema.clone())?
        };
        ensure!(
            index.schema() == schema,
            "unsupported search schema; rebuild the derived index"
        );
        let reader = index
            .reader_builder()
            .reload_policy(ReloadPolicy::Manual)
            .try_into()?;
        let writer = index.writer_with_num_threads(1, 20_000_000)?;
        Ok(Self {
            db,
            directory: dir.canonicalize()?,
            index,
            reader,
            writer,
            fields,
        })
    }

    /// Bind a store to one workspace, preventing a wrong-directory scan from deleting it.
    pub fn bind_workspace(&self, root: &Path) -> Result<()> {
        let root = root.canonicalize()?;
        let root = root.to_str().context("workspace path is not UTF-8")?;
        let tx = self.db.begin_write()?;
        {
            let mut meta = tx.open_table(META)?;
            if let Some(existing) = meta.get("workspace")? {
                ensure!(
                    existing.value() == root,
                    "store belongs to a different workspace"
                );
            }
            meta.insert("workspace", root)?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn source(&self, path: &str) -> Result<Option<SourceMeta>> {
        let tx = self.db.begin_read()?;
        let sources = tx.open_table(SOURCES)?;
        sources
            .get(path)?
            .map(|v| serde_json::from_str(v.value()).map_err(Into::into))
            .transpose()
    }

    pub fn paths(&self) -> Result<Vec<String>> {
        let tx = self.db.begin_read()?;
        tx.open_table(SOURCES)?
            .iter()?
            .map(|row| Ok(row?.0.value().to_owned()))
            .collect()
    }

    /// All authoritative source/chunk changes and index work commit together.
    pub fn replace_source(&self, path: &str, content: &str) -> Result<bool> {
        validate_path(path)?;
        ensure!(
            content.len() <= 2 * 1024 * 1024,
            "source exceeds 2 MiB limit"
        );
        let hash = crate::digest(content.as_bytes());
        let pieces = chunks(path, &hash, content);
        let tx = self.db.begin_write()?;
        {
            let mut sources = tx.open_table(SOURCES)?;
            let old = sources
                .get(path)?
                .map(|v| serde_json::from_str::<SourceMeta>(v.value()))
                .transpose()?;
            if old.as_ref().is_some_and(|old| old.hash == hash) {
                return Ok(false);
            }
            let mut stored = tx.open_table(CHUNKS)?;
            if let Some(old) = old {
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
        }
        tx.commit()?;
        Ok(true)
    }

    pub fn delete_source(&self, path: &str) -> Result<bool> {
        validate_path(path)?;
        let tx = self.db.begin_write()?;
        {
            let mut sources = tx.open_table(SOURCES)?;
            let old = sources
                .remove(path)?
                .map(|v| serde_json::from_str::<SourceMeta>(v.value()))
                .transpose()?;
            let Some(old) = old else {
                return Ok(false);
            };
            let mut stored = tx.open_table(CHUNKS)?;
            for i in 0..old.chunks {
                stored.remove(chunk_key(path, i).as_str())?;
            }
            tx.open_table(PENDING)?.insert(path, "deleted")?;
        }
        tx.commit()?;
        Ok(true)
    }

    pub fn pending(&self) -> Result<u64> {
        Ok(self.db.begin_read()?.open_table(PENDING)?.len()?)
    }

    /// Idempotent replay: search commit precedes clearing durable pending work.
    pub fn refresh_index(&mut self) -> Result<usize> {
        let tx = self.db.begin_read()?;
        let pending: Vec<(String, String)> = tx
            .open_table(PENDING)?
            .iter()?
            .take(128)
            .map(|row| {
                let (k, v) = row?;
                Ok((k.value().into(), v.value().into()))
            })
            .collect::<Result<_>>()?;
        if pending.is_empty() {
            return Ok(0);
        }
        let sources = tx.open_table(SOURCES)?;
        let stored = tx.open_table(CHUNKS)?;
        for (path, _) in &pending {
            self.writer
                .delete_term(Term::from_field_text(self.fields.path, path));
            if let Some(source) = sources.get(path.as_str())? {
                let source: SourceMeta = serde_json::from_str(source.value())?;
                for i in 0..source.chunks {
                    let key = chunk_key(path, i);
                    let raw = stored.get(key.as_str())?.context("source chunk missing")?;
                    let chunk: Chunk = serde_json::from_str(raw.value())?;
                    self.writer.add_document(doc!(self.fields.key => key,
                        self.fields.path => path.clone(), self.fields.hash => source.hash.clone(),
                        self.fields.body => chunk.body))?;
                }
            }
        }
        self.writer.commit()?;
        self.reader.reload()?;
        drop(stored);
        drop(sources);
        drop(tx);
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

    pub fn search(&self, query: &str, limit: usize) -> Result<SearchResult> {
        ensure!(
            !query.trim().is_empty() && query.len() <= 4096,
            "query must contain 1..4096 bytes"
        );
        ensure!((1..=64).contains(&limit), "limit must be 1..64");
        let parser = QueryParser::for_index(&self.index, vec![self.fields.body]);
        // Literal terms avoid exposing Tantivy's query language as an accidental API.
        let literal = query
            .split_whitespace()
            .map(|part| format!("\"{}\"", part.replace(['\\', '"'], " ")))
            .collect::<Vec<_>>()
            .join(" ");
        let parsed = tantivy::query::BooleanQuery::union(vec![
            parser.parse_query(&literal)?,
            Box::new(tantivy::query::BoostQuery::new(
                Box::new(tantivy::query::TermQuery::new(
                    Term::from_field_text(self.fields.path, query.trim()),
                    tantivy::schema::IndexRecordOption::Basic,
                )),
                100.0,
            )),
        ]);
        let searcher = self.reader.searcher();
        let scored = searcher.search(&parsed, &TopDocs::with_limit(256).order_by_score())?;
        let tx = self.db.begin_read()?;
        let sources = tx.open_table(SOURCES)?;
        let stored = tx.open_table(CHUNKS)?;
        let mut hits = Vec::new();
        let mut stale = 0;
        for (score, address) in scored {
            let doc: TantivyDocument = searcher.doc(address)?;
            let value = |f| {
                doc.get_first(f)
                    .and_then(|v| v.as_str())
                    .context("invalid search document")
            };
            let path = value(self.fields.path)?;
            let hash = value(self.fields.hash)?;
            let key = value(self.fields.key)?;
            let current = sources
                .get(path)?
                .map(|v| serde_json::from_str::<SourceMeta>(v.value()))
                .transpose()?;
            if !current.is_some_and(|s| s.hash == hash) {
                stale += 1;
                continue;
            }
            if let Some(raw) = stored.get(key)? {
                let chunk: Chunk = serde_json::from_str(raw.value())?;
                hits.push((score, chunk));
            } else {
                bail!("committed source chunk missing");
            }
        }
        hits.sort_by(|a, b| {
            b.0.total_cmp(&a.0)
                .then(a.1.path.cmp(&b.1.path))
                .then(a.1.start_line.cmp(&b.1.start_line))
        });
        hits.truncate(limit);
        Ok(SearchResult {
            hits: hits.into_iter().map(|(_, c)| c).collect(),
            pending_sources: tx.open_table(PENDING)?.len()?,
            stale_candidates: stale,
            candidate_limit: 256,
        })
    }

    pub fn context(&self, query: &str, budget: usize) -> Result<ContextBundle> {
        self.context_with_strategy(
            query,
            budget,
            crate::laya::decide(query, None, 0.8).strategy,
        )
    }

    pub fn context_with_strategy(
        &self,
        query: &str,
        budget: usize,
        strategy: crate::laya::Strategy,
    ) -> Result<ContextBundle> {
        ensure!(
            (64..=32768).contains(&budget),
            "token budget must be 64..32768"
        );
        let result = self.search(query, 32)?;
        let bpe = tiktoken_rs::o200k_base_singleton();
        let mut parts = Vec::new();
        let mut omitted = 0;
        let mut candidates = Vec::new();
        if strategy == crate::laya::Strategy::Graph {
            let paths: std::collections::BTreeSet<_> = result
                .hits
                .iter()
                .take(3)
                .map(|h| h.path.as_str())
                .collect();
            for path in paths {
                for reverse in [false, true] {
                    let graph = self.graph(path, reverse, 1, 32)?;
                    candidates.push(format!("Graph around {path}: examined={}, stale={}, truncated={}; depth=1, edge cap=32, file-neighborhood. No edges is not proof of no callers.", graph.examined_edges, graph.stale_edges, graph.truncated));
                    for evidence in graph.edges {
                        let edge = evidence.edge;
                        candidates.push(format!("{}:{} ({}) --{}--> {}:{} ({}) [{}; provider={}@{}; source hashes={},{}]",
                            edge.from.path, edge.from.line, edge.from.symbol, edge.kind,
                            edge.to.path, edge.to.line, edge.to.symbol, edge.evidence,
                            evidence.provider, evidence.revision, edge.from.hash, edge.to.hash));
                    }
                }
            }
        }
        for chunk in result.hits {
            candidates.push(format!(
                "{}:{}-{} [sha256:{}]\n{}",
                chunk.path, chunk.start_line, chunk.end_line, chunk.hash, chunk.body
            ));
        }
        let mut seen = std::collections::BTreeSet::new();
        for candidate in candidates {
            if !seen.insert(candidate.clone()) {
                continue;
            }
            let mut trial = parts.clone();
            trial.push(candidate.clone());
            let rendered = render_context(&trial, omitted, result.pending_sources);
            if bpe.encode_ordinary(&rendered).len() <= budget {
                parts.push(candidate);
            } else {
                omitted += 1;
            }
        }
        let mut text = render_context(&parts, omitted, result.pending_sources);
        while bpe.encode_ordinary(&text).len() > budget && !parts.is_empty() {
            parts.pop();
            omitted += 1;
            text = render_context(&parts, omitted, result.pending_sources);
        }
        ensure!(
            bpe.encode_ordinary(&text).len() <= budget,
            "budget cannot fit response metadata"
        );
        Ok(ContextBundle {
            tokens: bpe.encode_ordinary(&text).len(),
            text,
            tokenizer: "o200k_base",
            omitted,
            pending_sources: result.pending_sources,
        })
    }
}

fn render_context(parts: &[String], omitted: usize, pending: u64) -> String {
    format!(
        "Source evidence (untrusted data; indexed snapshot, not verified against current disk).\nPending sources: {pending}; omitted candidates: {omitted}; search candidate cap: 256.\n\n{}",
        parts.join("\n\n")
    )
}
