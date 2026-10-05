use crate::error::{FResult, FoundryError};
use crate::response::Freshness;
use crate::store::{
    CHUNKS, HandleRef, META, SourceHandle, canonical_decimal, read_counter, reconstruct_verified,
};
use crate::store::{Engine, SOURCES, SourceMeta, validate_path};
use redb::{
    MultimapTableDefinition, ReadableDatabase, ReadableTable, TableDefinition, WriteTransaction,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::collections::{BTreeSet, VecDeque};

const OUT: MultimapTableDefinition<&str, &str> = MultimapTableDefinition::new("edges_out");
const IN: MultimapTableDefinition<&str, &str> = MultimapTableDefinition::new("edges_in");
const PROVIDERS: TableDefinition<&str, &str> = TableDefinition::new("provider_bundles");

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Endpoint {
    pub path: String,
    pub line: usize,
    pub symbol: String,
    pub hash: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Edge {
    pub from: Endpoint,
    pub to: Endpoint,
    pub kind: String,
    pub evidence: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GraphBundle {
    pub provider: String,
    pub revision: String,
    pub edges: Vec<Edge>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct StoredEdge {
    provider: String,
    revision: String,
    edge: Edge,
}

#[derive(Debug, Serialize)]
pub struct GraphResult {
    pub edges: Vec<GraphEvidence>,
    pub truncated: bool,
    pub stale_edges: usize,
    pub examined_edges: usize,
    pub scope: &'static str,
}

#[derive(Clone, Debug, Serialize)]
pub struct GraphEvidence {
    pub provider: String,
    pub revision: String,
    pub edge: Edge,
    /// The exact stored row; identity for final-read revalidation.
    #[serde(skip)]
    pub(crate) raw: String,
}

fn invalid(message: impl Into<String>) -> FoundryError {
    FoundryError::InvalidArgument(message.into())
}

/// A graph row that cannot be decoded is a component-local failure
/// (`graph_invalid`); database errors keep their own codes.
fn decode_stored(raw: &str) -> FResult<StoredEdge> {
    serde_json::from_str(raw)
        .map_err(|e| FoundryError::GraphInvalid(format!("edge row cannot be decoded: {e}")))
}

/// True when the exact stored edge row is still present under `from_path` in
/// the transaction. Used by the final context read to revalidate selected
/// graph rows, not only their endpoint hashes.
pub(crate) fn edge_row_present(
    tx: &redb::ReadTransaction,
    from_path: &str,
    raw: &str,
) -> FResult<bool> {
    let edges = tx.open_multimap_table(OUT)?;
    for row in edges.get(from_path)? {
        if row?.value() == raw {
            return Ok(true);
        }
    }
    Ok(false)
}

pub(crate) fn init(tx: &WriteTransaction) -> FResult<()> {
    tx.open_multimap_table(OUT)?;
    tx.open_multimap_table(IN)?;
    tx.open_table(PROVIDERS)?;
    Ok(())
}

impl Engine {
    /// Replace one producer's entire bundle atomically; other producers remain intact.
    pub fn import_graph(&self, bundle: &GraphBundle) -> FResult<usize> {
        if bundle.provider.trim().is_empty() || bundle.provider.len() > 128 {
            return Err(invalid("invalid provider"));
        }
        if bundle.revision.trim().is_empty() || bundle.revision.len() > 128 {
            return Err(invalid("invalid provider revision"));
        }
        if bundle.edges.len() > 100_000 {
            return Err(invalid("bundle exceeds 100000 edges"));
        }
        let tx = self.db.begin_write()?;
        let mut encoded = BTreeSet::new();
        {
            let sources = tx.open_table(SOURCES)?;
            for edge in &bundle.edges {
                if !["calls", "references", "imports", "contains", "depends_on"]
                    .contains(&edge.kind.as_str())
                {
                    return Err(invalid("unknown edge kind"));
                }
                if !["resolved", "syntactic", "inferred", "manual"]
                    .contains(&edge.evidence.as_str())
                {
                    return Err(invalid("unknown evidence class"));
                }
                for endpoint in [&edge.from, &edge.to] {
                    validate_path(&endpoint.path)?;
                    if endpoint.line == 0 || endpoint.symbol.len() > 1024 {
                        return Err(invalid("invalid endpoint"));
                    }
                    let meta = sources.get(endpoint.path.as_str())?.ok_or_else(|| {
                        invalid(format!("graph source absent: {}", endpoint.path))
                    })?;
                    let meta: SourceMeta = serde_json::from_str(meta.value())
                        .map_err(|e| FoundryError::CorruptStore(format!("source record: {e}")))?;
                    if meta.hash != endpoint.hash {
                        return Err(invalid(format!("stale graph source: {}", endpoint.path)));
                    }
                    if endpoint.line > meta.lines {
                        return Err(invalid(format!(
                            "graph line outside source: {}",
                            endpoint.path
                        )));
                    }
                }
                encoded.insert(serde_json::to_string(&StoredEdge {
                    provider: bundle.provider.clone(),
                    revision: bundle.revision.clone(),
                    edge: edge.clone(),
                })?);
            }
            let mut providers = tx.open_table(PROVIDERS)?;
            let previous = providers
                .get(bundle.provider.as_str())?
                .and_then(|v| serde_json::from_str::<Vec<String>>(v.value()).ok())
                .unwrap_or_default();
            let mut out = tx.open_multimap_table(OUT)?;
            let mut incoming = tx.open_multimap_table(IN)?;
            for raw in previous {
                let stored = decode_stored(&raw)?;
                out.remove(stored.edge.from.path.as_str(), raw.as_str())?;
                incoming.remove(stored.edge.to.path.as_str(), raw.as_str())?;
            }
            for raw in &encoded {
                let stored = decode_stored(raw)?;
                out.insert(stored.edge.from.path.as_str(), raw.as_str())?;
                incoming.insert(stored.edge.to.path.as_str(), raw.as_str())?;
            }
            providers.insert(
                bundle.provider.as_str(),
                serde_json::to_string(&encoded)?.as_str(),
            )?;
        }
        tx.commit()?;
        Ok(encoded.len())
    }

    /// File-neighborhood traversal. Symbols label evidence; this is not symbol resolution.
    pub fn graph(
        &self,
        seed: &str,
        reverse: bool,
        depth: usize,
        max_edges: usize,
    ) -> FResult<GraphResult> {
        validate_path(seed)?;
        if depth > 4 || !(1..=256).contains(&max_edges) {
            return Err(invalid("graph bounds: depth 0..4, edges 1..256"));
        }
        let tx = self.db.begin_read()?;
        let sources = tx.open_table(SOURCES)?;
        let edges = tx.open_multimap_table(if reverse { IN } else { OUT })?;
        let mut frontier = VecDeque::from([(seed.to_owned(), 0)]);
        let mut visited = BTreeSet::from([seed.to_owned()]);
        let mut emitted = BTreeSet::new();
        let mut result = GraphResult {
            edges: vec![],
            truncated: false,
            stale_edges: 0,
            examined_edges: 0,
            scope: "file-neighborhood; supplied symbol labels",
        };
        while let Some((path, hop)) = frontier.pop_front() {
            if hop >= depth {
                continue;
            }
            for row in edges.get(path.as_str())? {
                if result.examined_edges == max_edges {
                    result.truncated = true;
                    return Ok(result);
                }
                result.examined_edges += 1;
                let raw = row?;
                let stored = decode_stored(raw.value())?;
                let mut fresh = true;
                for endpoint in [&stored.edge.from, &stored.edge.to] {
                    let current = sources.get(endpoint.path.as_str())?;
                    fresh &= current.is_some_and(|m| {
                        serde_json::from_str::<SourceMeta>(m.value())
                            .is_ok_and(|meta| meta.hash == endpoint.hash)
                    });
                }
                if !fresh {
                    result.stale_edges += 1;
                    continue;
                }
                let next = if reverse {
                    &stored.edge.from.path
                } else {
                    &stored.edge.to.path
                };
                if !visited.contains(next) {
                    if visited.len() >= 64 {
                        result.truncated = true;
                    } else {
                        visited.insert(next.clone());
                        frontier.push_back((next.clone(), hop + 1));
                    }
                }
                if emitted.insert(raw.value().to_owned()) {
                    result.edges.push(GraphEvidence {
                        provider: stored.provider,
                        revision: stored.revision,
                        edge: stored.edge,
                        raw: raw.value().to_owned(),
                    });
                }
            }
        }
        Ok(result)
    }
}

// ===========================================================================
// 005 compiler facts: scoped SCIP definitions and references.
//
// Independent of the manual file-neighborhood edges above: separate tables,
// a separate evidence class and no shared rows or writer. Rules (spec 005
// § Publication, queries and limits):
//
// - the transaction scope is `(producer_namespace, origin_document_path)`;
// - every occurrence is stored ONCE with a symbol -> occurrence lookup; no
//   reference x definition pair is ever materialized, so a generated symbol
//   with 300 definitions and 300 references stores 600 rows, not 90,000;
// - a coverage row per scope and ONE selected snapshot tuple per producer
//   decide eligibility at read time, so replacing an artifact or its
//   configuration can never mix old and new document facts.
// ===========================================================================

/// One row per compiler producer namespace, key = namespace, value = JSON
/// [`ProducerRow`]: the producer's SELECTED snapshot tuple (artifact digest,
/// input-manifest digest, source revision, producer/config identity) with its
/// import state, plus the LATEST import report (which names a failure that
/// left the selection alone). Needed because eligibility is a property of the
/// producer's selection, not of any single scope.
pub(crate) const COMPILER_PRODUCERS: TableDefinition<&str, &str> =
    TableDefinition::new("compiler_producers");
/// One coverage row per scope, key = `<ns>\0<path>`, value = JSON
/// [`ScopeRow`]: the snapshot the scope was published under, the source hash
/// it was read from, its coverage status and counts. Needed so a scope is
/// eligible only under the selected snapshot, and so an accepted-empty scope
/// is distinguishable from a never-imported (unknown) one.
pub(crate) const COMPILER_SCOPES: TableDefinition<&str, &str> =
    TableDefinition::new("compiler_scopes");
/// Every occurrence exactly once, key =
/// `<ns>\0<path>\0<start:020>\0<end:020>\0<d|r>\0<symbol_id>`, value = JSON
/// [`OccurrenceValue`]. The key orders a scope by position (position seeds
/// seek it) and a scope is exactly one key prefix (replacement removes only
/// that prefix and its reverse entries).
pub(crate) const COMPILER_OCCURRENCES: TableDefinition<&str, &str> =
    TableDefinition::new("compiler_occurrences");
/// The reverse lookup, key = `<symbol_id>\0<d|r>\0<path>\0<start:020>\0<end:020>`,
/// value = the namespace. Definitions sort before references and references
/// by path then start, so a symbol's definition lookup is one short prefix
/// range and `after=<path>#<start>` is a range seek. Needed to address
/// references by logical symbol without scanning every scope.
pub(crate) const COMPILER_BY_SYMBOL: TableDefinition<&str, &str> =
    TableDefinition::new("compiler_by_symbol");

/// Create the empty compiler-fact tables (initialization and the v4 upgrade).
pub(crate) fn init_compiler(tx: &WriteTransaction) -> FResult<()> {
    tx.open_table(COMPILER_PRODUCERS)?;
    tx.open_table(COMPILER_SCOPES)?;
    tx.open_table(COMPILER_OCCURRENCES)?;
    tx.open_table(COMPILER_BY_SYMBOL)?;
    Ok(())
}

/// A schema-4 store without its compiler tables is corrupt; an open never
/// recreates them.
pub(crate) fn check_compiler_tables(tx: &redb::ReadTransaction) -> FResult<()> {
    let missing = |name: &str, error: redb::TableError| {
        FoundryError::CorruptStore(format!("{name} table: {error}"))
    };
    tx.open_table(COMPILER_PRODUCERS)
        .map_err(|e| missing("compiler_producers", e))?;
    tx.open_table(COMPILER_SCOPES)
        .map_err(|e| missing("compiler_scopes", e))?;
    tx.open_table(COMPILER_OCCURRENCES)
        .map_err(|e| missing("compiler_occurrences", e))?;
    tx.open_table(COMPILER_BY_SYMBOL)
        .map_err(|e| missing("compiler_by_symbol", e))?;
    Ok(())
}

/// Definition or reference. A reference is any occurrence without the SCIP
/// `Definition` role; it is never relabeled `calls`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OccurrenceKind {
    Definition,
    Reference,
}

impl OccurrenceKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Definition => "definition",
            Self::Reference => "reference",
        }
    }

    fn tag(self) -> char {
        match self {
            Self::Definition => 'd',
            Self::Reference => 'r',
        }
    }

    fn from_tag(tag: &str) -> Option<Self> {
        match tag {
            "d" => Some(Self::Definition),
            "r" => Some(Self::Reference),
            _ => None,
        }
    }
}

fn json_digest(value: &serde_json::Value) -> String {
    // `Value`'s Display is compact JSON.
    crate::digest(value.to_string().as_bytes())
}

/// Lowercase SHA-256 of the compact UTF-8 JSON array
/// `[producer_namespace,"global",SCIP_symbol]`, or for a local symbol
/// `[producer_namespace,"local",document_path,SCIP_symbol]`.
pub fn symbol_id(producer_namespace: &str, document_path: &str, symbol: &str) -> String {
    if scip::symbol::is_local_symbol(symbol) {
        json_digest(&serde_json::json!([
            producer_namespace,
            "local",
            document_path,
            symbol
        ]))
    } else {
        json_digest(&serde_json::json!([producer_namespace, "global", symbol]))
    }
}

/// Lowercase SHA-256 of the compact UTF-8 JSON array
/// `[producer_namespace,kind,path,source_hash,start,end,symbol_id]` with kind
/// `definition` or `reference`: the stable deduplication identity of
/// reference evidence.
pub fn occurrence_id(
    producer_namespace: &str,
    kind: OccurrenceKind,
    path: &str,
    source_hash: &str,
    start: u64,
    end: u64,
    symbol_id: &str,
) -> String {
    json_digest(&serde_json::json!([
        producer_namespace,
        kind.as_str(),
        path,
        source_hash,
        start,
        end,
        symbol_id
    ]))
}

/// One side of a resolved edge: the source identity (its workspace-relative
/// path), that source's full SHA-256 and the byte range.
#[derive(Clone, Copy, Debug)]
pub struct EdgeEnd<'a> {
    pub identity: &'a str,
    pub hash: &'a str,
    pub start: u64,
    pub end: u64,
}

/// Lowercase SHA-256 of the compact UTF-8 JSON array
/// `[producer,kind,from_identity,from_hash,from_start,from_end,to_identity,
/// to_hash,to_start,to_end]`. It exists only for a uniquely resolved target;
/// ambiguous and unresolved results never manufacture one.
pub fn edge_id(producer: &str, kind: &str, from: EdgeEnd<'_>, to: EdgeEnd<'_>) -> String {
    json_digest(&serde_json::json!([
        producer,
        kind,
        from.identity,
        from.hash,
        from.start,
        from.end,
        to.identity,
        to.hash,
        to.start,
        to.end
    ]))
}

/// The producer identity recorded in the snapshot manifest.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProducerIdentity {
    pub name: String,
    pub release_tag: String,
    pub commit: String,
    pub version_output: String,
    pub binary_sha256: String,
}

/// The selected snapshot identity of one producer: artifact digest,
/// input-manifest digest, source revision and producer/config identity. A fact
/// is eligible only when its scope belongs to this tuple AND the store's
/// source revision still equals `source_revision`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SnapshotTuple {
    pub snapshot_id: String,
    pub artifact_sha256: String,
    pub manifest_sha256: String,
    pub source_revision: u64,
    pub producer: ProducerIdentity,
    /// SHA-256 of compact JSON over the full producer identity, the literal
    /// invocation and the literal config.
    pub config_id: String,
}

impl SnapshotTuple {
    pub fn new(
        producer: ProducerIdentity,
        invocation: &str,
        config: &str,
        artifact_sha256: String,
        manifest_sha256: String,
        source_revision: u64,
    ) -> Self {
        let config_id = json_digest(&serde_json::json!([
            producer.name,
            producer.release_tag,
            producer.commit,
            producer.version_output,
            producer.binary_sha256,
            invocation,
            config
        ]));
        let snapshot_id = json_digest(&serde_json::json!([
            config_id,
            artifact_sha256,
            manifest_sha256,
            source_revision
        ]));
        Self {
            snapshot_id,
            artifact_sha256,
            manifest_sha256,
            source_revision,
            producer,
            config_id,
        }
    }
}

/// How far the selected snapshot's import got.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SnapshotState {
    /// Selected, documents still being published (or the run was cut off
    /// without a clean ending): a named partial snapshot.
    Importing,
    /// Fully consumed with every document committed.
    Complete,
    /// Fully or partly consumed with at least one failed or unpublished
    /// document, or interrupted.
    Partial,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SelectedSnapshot {
    pub tuple: SnapshotTuple,
    pub state: SnapshotState,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ProducerRow {
    pub selected: Option<SelectedSnapshot>,
    pub latest: Option<crate::scip::ImportReport>,
}

/// Coverage of one scope.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScopeStatus {
    /// Occurrences stored and every reference resolved at import time.
    Complete,
    /// A validated document (or a profile-listed source) with no occurrences.
    AcceptedEmpty,
    /// Occurrences stored; some references have an external, unknown or
    /// ambiguous target (their count is `unresolved`).
    Partial,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScopeRow {
    pub snapshot_id: String,
    /// Full SHA-256 of the origin source the scope was read from.
    pub source_hash: String,
    pub status: ScopeStatus,
    pub definitions: u32,
    pub references: u32,
    pub unresolved: u32,
    /// The longest stored occurrence in bytes: lets a position lookup stop
    /// walking back exactly.
    pub max_span: u64,
}

/// The stored value of one occurrence. Identity fields live in the key.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OccurrenceValue {
    pub symbol: String,
    pub roles: i32,
}

/// One validated, deduplicated occurrence ready to store.
#[derive(Clone, Debug)]
pub(crate) struct NewOccurrence {
    pub kind: OccurrenceKind,
    pub start: u64,
    pub end: u64,
    pub symbol_id: String,
    pub symbol: String,
    pub roles: i32,
}

/// What one scope transaction writes.
pub(crate) struct ScopeWrite<'a> {
    pub namespace: &'a str,
    pub path: &'a str,
    pub snapshot: &'a SnapshotTuple,
    pub source_hash: &'a str,
    pub occurrences: &'a [NewOccurrence],
    pub unresolved: u32,
}

fn stored_row<T: serde::de::DeserializeOwned>(raw: &str, what: &str) -> FResult<T> {
    serde_json::from_str(raw)
        .map_err(|e| FoundryError::GraphInvalid(format!("{what} row cannot be decoded: {e}")))
}

fn stale_artifact(message: impl Into<String>) -> FoundryError {
    crate::scip::fail(crate::scip::code::STALE_ARTIFACT, message)
}

fn scope_key(namespace: &str, path: &str) -> String {
    format!("{namespace}\0{path}")
}

fn occurrence_key(
    namespace: &str,
    path: &str,
    start: u64,
    end: u64,
    kind: OccurrenceKind,
    symbol_id: &str,
) -> String {
    format!(
        "{namespace}\0{path}\0{start:020}\0{end:020}\0{}\0{symbol_id}",
        kind.tag()
    )
}

fn symbol_key(symbol_id: &str, kind: OccurrenceKind, path: &str, start: u64, end: u64) -> String {
    format!(
        "{symbol_id}\0{}\0{path}\0{start:020}\0{end:020}",
        kind.tag()
    )
}

struct OccurrenceKey {
    start: u64,
    end: u64,
    kind: OccurrenceKind,
    symbol_id: String,
}

fn corrupt_key(what: &str) -> FoundryError {
    FoundryError::GraphInvalid(format!("{what} key cannot be decoded"))
}

/// `<ns>\0<path>\0<start>\0<end>\0<tag>\0<symbol_id>`; neither the namespace
/// nor a path contains NUL, so the split is exact.
fn parse_occurrence_key(key: &str) -> FResult<OccurrenceKey> {
    let mut parts = key.split('\0');
    let (_namespace, _path) = (parts.next(), parts.next());
    let (Some(start), Some(end)) = (key_u64(parts.next()), key_u64(parts.next())) else {
        return Err(corrupt_key("compiler occurrence"));
    };
    if start >= end {
        return Err(corrupt_key("compiler occurrence"));
    }
    let kind = parts.next().and_then(OccurrenceKind::from_tag);
    let symbol_id = parts.next().filter(|id| is_symbol_id(id));
    match (kind, symbol_id, parts.next()) {
        (Some(kind), Some(symbol_id), None) => Ok(OccurrenceKey {
            start,
            end,
            kind,
            symbol_id: symbol_id.to_owned(),
        }),
        _ => Err(corrupt_key("compiler occurrence")),
    }
}

/// A stored symbol id is exactly 64 lowercase hex characters.
fn is_symbol_id(text: &str) -> bool {
    text.len() == 64 && text.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

struct SymbolKey {
    path: String,
    start: u64,
    end: u64,
}

/// `<symbol_id>\0<tag>\0<path>\0<start>\0<end>`.
fn parse_symbol_key(key: &str) -> FResult<SymbolKey> {
    let mut parts = key.split('\0');
    let (_symbol, _tag) = (parts.next(), parts.next());
    let path = parts.next();
    match (
        path,
        key_u64(parts.next()),
        key_u64(parts.next()),
        parts.next(),
    ) {
        (Some(path), Some(start), Some(end), None) if start < end => Ok(SymbolKey {
            path: path.to_owned(),
            start,
            end,
        }),
        _ => Err(corrupt_key("compiler symbol")),
    }
}

/// One zero-padded 20-digit offset field of a compiler key. Anything but the
/// exact canonical shape is corruption, never a silently different value.
fn key_u64(part: Option<&str>) -> Option<u64> {
    let part = part?;
    (part.len() == 20 && part.bytes().all(|b| b.is_ascii_digit()))
        .then(|| part.parse().ok())
        .flatten()
}

/// Remove one scope's occurrences, their reverse entries and its coverage
/// row inside the caller's transaction. Returns the occurrences removed.
fn remove_scope(tx: &WriteTransaction, namespace: &str, path: &str) -> FResult<u64> {
    let mut occurrences = tx.open_table(COMPILER_OCCURRENCES)?;
    let mut by_symbol = tx.open_table(COMPILER_BY_SYMBOL)?;
    let low = format!("{namespace}\0{path}\0");
    let high = format!("{namespace}\0{path}\u{1}");
    let keys: Vec<String> = occurrences
        .range(low.as_str()..high.as_str())?
        .map(|row| -> FResult<String> { Ok(row?.0.value().to_owned()) })
        .collect::<FResult<_>>()?;
    for key in &keys {
        let parsed = parse_occurrence_key(key)?;
        by_symbol.remove(
            symbol_key(
                &parsed.symbol_id,
                parsed.kind,
                path,
                parsed.start,
                parsed.end,
            )
            .as_str(),
        )?;
        occurrences.remove(key.as_str())?;
    }
    tx.open_table(COMPILER_SCOPES)?
        .remove(scope_key(namespace, path).as_str())?;
    Ok(keys.len() as u64)
}

fn read_producers(tx: &redb::ReadTransaction) -> FResult<Vec<(String, ProducerRow)>> {
    let table = tx.open_table(COMPILER_PRODUCERS)?;
    table
        .iter()?
        .map(|row| -> FResult<(String, ProducerRow)> {
            let (key, value) = row?;
            Ok((
                key.value().to_owned(),
                stored_row(value.value(), "compiler producer")?,
            ))
        })
        .collect()
}

/// The completed manifest's verdict on one stored scope path: `true` when the
/// source is proven absent.
pub(crate) type ProvenAbsent<'a> = dyn FnMut(&str) -> FResult<bool> + 'a;

impl Engine {
    /// Every compiler producer's selected snapshot and latest import report,
    /// in namespace order.
    pub fn compiler_producers(&self) -> FResult<Vec<(String, ProducerRow)>> {
        read_producers(&self.db.begin_read()?)
    }

    /// Read-modify-write one producer row in its own transaction. `edit`
    /// runs inside the write transaction and may refuse by returning an error.
    pub(crate) fn update_producer(
        &self,
        namespace: &str,
        edit: impl FnOnce(&mut ProducerRow, u64) -> FResult<()>,
    ) -> FResult<()> {
        let tx = self.db.begin_write()?;
        {
            let revision = read_counter(&tx.open_table(META)?, "source_revision")?;
            let mut producers = tx.open_table(COMPILER_PRODUCERS)?;
            let mut row: ProducerRow = producers
                .get(namespace)?
                .map(|raw| stored_row(raw.value(), "compiler producer"))
                .transpose()?
                .unwrap_or_default();
            edit(&mut row, revision)?;
            producers.insert(namespace, serde_json::to_string(&row)?.as_str())?;
        }
        tx.commit()?;
        Ok(())
    }

    /// Select `tuple` as its producer's snapshot (state `importing`) after
    /// the global preflight and definition lookup succeeded, and before the
    /// first document is published. The source revision is checked again
    /// inside this transaction.
    pub(crate) fn select_snapshot(&self, tuple: &SnapshotTuple) -> FResult<()> {
        self.update_producer(&tuple.producer.name, |row, revision| {
            if revision != tuple.source_revision {
                return Err(stale_artifact(
                    "the source revision changed before the snapshot was selected",
                ));
            }
            row.selected = Some(SelectedSnapshot {
                tuple: tuple.clone(),
                state: SnapshotState::Importing,
            });
            Ok(())
        })
    }

    /// Publish one scope in a single transaction: validate the source
    /// revision, the origin source hash and the selected snapshot, remove only
    /// that scope's prior facts and reverse entries, insert the new set and
    /// its coverage row. A refusal or an uncommitted drop leaves the scope
    /// exactly as it was.
    pub(crate) fn publish_scope(
        &self,
        write: &ScopeWrite<'_>,
        control: &crate::Control,
    ) -> FResult<ScopeRow> {
        let tx = self.db.begin_write()?;
        let row = {
            let revision = read_counter(&tx.open_table(META)?, "source_revision")?;
            if revision != write.snapshot.source_revision {
                return Err(stale_artifact(format!(
                    "the source revision changed during the import: {}",
                    write.path
                )));
            }
            let current: SourceMeta = tx
                .open_table(SOURCES)?
                .get(write.path)?
                .map(|raw| crate::store::decode(raw.value(), "source"))
                .transpose()?
                .ok_or_else(|| stale_artifact(format!("source is absent: {}", write.path)))?;
            if current.hash != write.source_hash {
                return Err(stale_artifact(format!(
                    "source changed during the import: {}",
                    write.path
                )));
            }
            let selected: Option<ProducerRow> = tx
                .open_table(COMPILER_PRODUCERS)?
                .get(write.namespace)?
                .map(|raw| stored_row(raw.value(), "compiler producer"))
                .transpose()?;
            let still_selected = selected
                .and_then(|row| row.selected)
                .is_some_and(|s| s.tuple.snapshot_id == write.snapshot.snapshot_id);
            if !still_selected {
                return Err(stale_artifact(
                    "the selected snapshot changed during the import",
                ));
            }
            remove_scope(&tx, write.namespace, write.path)?;
            let mut occurrences = tx.open_table(COMPILER_OCCURRENCES)?;
            let mut by_symbol = tx.open_table(COMPILER_BY_SYMBOL)?;
            let (mut definitions, mut references, mut max_span) = (0u32, 0u32, 0u64);
            for o in write.occurrences {
                match o.kind {
                    OccurrenceKind::Definition => definitions += 1,
                    OccurrenceKind::Reference => references += 1,
                }
                max_span = max_span.max(o.end - o.start);
                let value = serde_json::to_string(&OccurrenceValue {
                    symbol: o.symbol.clone(),
                    roles: o.roles,
                })?;
                occurrences.insert(
                    occurrence_key(
                        write.namespace,
                        write.path,
                        o.start,
                        o.end,
                        o.kind,
                        &o.symbol_id,
                    )
                    .as_str(),
                    value.as_str(),
                )?;
                by_symbol.insert(
                    symbol_key(&o.symbol_id, o.kind, write.path, o.start, o.end).as_str(),
                    write.namespace,
                )?;
            }
            let status = if write.occurrences.is_empty() {
                ScopeStatus::AcceptedEmpty
            } else if write.unresolved > 0 {
                ScopeStatus::Partial
            } else {
                ScopeStatus::Complete
            };
            let row = ScopeRow {
                snapshot_id: write.snapshot.snapshot_id.clone(),
                source_hash: write.source_hash.to_owned(),
                status,
                definitions,
                references,
                unresolved: write.unresolved,
                max_span,
            };
            tx.open_table(COMPILER_SCOPES)?.insert(
                scope_key(write.namespace, write.path).as_str(),
                serde_json::to_string(&row)?.as_str(),
            )?;
            row
        };
        // Written and uncommitted: an interruption here leaves the scope as it was.
        fault!(
            SCIP_SCOPE_BEFORE_COMMIT,
            Some(self),
            Some(control),
            write.path
        )?;
        control.check()?;
        tx.commit()?;
        Ok(row)
    }

    /// The ONE finalization transaction of an import. When `proven_absent`
    /// is given (the completed manifest, with no failed document, proved
    /// which sources are gone) it removes every scope of `namespace` whose
    /// path it names - a page of 128 scope rows at a time, all inside this
    /// transaction - and publishes the final report (with the true
    /// `retired` count) and the selected snapshot's final state in the same
    /// commit. Cancellation is checked before the commit, so a cancelled
    /// finalization retires nothing, reports nothing and leaves the
    /// previous producer row untouched. Document publication stays
    /// per-source; only this step is all-or-none.
    pub(crate) fn finalize_import(
        &self,
        namespace: &str,
        snapshot_id: &str,
        state: SnapshotState,
        report: &mut crate::scip::ImportReport,
        mut proven_absent: Option<&mut ProvenAbsent<'_>>,
        control: &crate::Control,
    ) -> FResult<()> {
        let tx = self.db.begin_write()?;
        let mut retired = 0u64;
        if let Some(proven_absent) = proven_absent.as_deref_mut() {
            let high = format!("{namespace}\u{1}");
            let mut from = format!("{namespace}\0");
            loop {
                control.check()?;
                let mut doomed: Vec<String> = Vec::new();
                let mut scanned = 0usize;
                let mut last: Option<String> = None;
                {
                    let scopes = tx.open_table(COMPILER_SCOPES)?;
                    for row in scopes.range(from.as_str()..high.as_str())? {
                        let (key, _) = row?;
                        let key = key.value().to_owned();
                        let path = key
                            .split_once('\0')
                            .map(|(_, path)| path.to_owned())
                            .ok_or_else(|| corrupt_key("compiler scope"))?;
                        if proven_absent(&path)? {
                            doomed.push(path);
                        }
                        last = Some(key);
                        scanned += 1;
                        if scanned == 128 {
                            break;
                        }
                    }
                }
                for path in &doomed {
                    remove_scope(&tx, namespace, path)?;
                    retired += 1;
                    // Proposed in the open transaction, not yet committed.
                    fault!(SCIP_RETIRE_PROPOSED, Some(self), Some(control), path)?;
                    control.check()?;
                }
                match last {
                    Some(key) if scanned == 128 => from = format!("{key}\u{1}"),
                    _ => break,
                }
            }
        }
        let mut finished = report.clone();
        finished.retired = retired;
        {
            let mut producers = tx.open_table(COMPILER_PRODUCERS)?;
            let mut row: ProducerRow = producers
                .get(namespace)?
                .map(|raw| stored_row(raw.value(), "compiler producer"))
                .transpose()?
                .unwrap_or_default();
            if let Some(selected) = row.selected.as_mut()
                && selected.tuple.snapshot_id == snapshot_id
            {
                selected.state = state;
            }
            row.latest = Some(finished.clone());
            producers.insert(namespace, serde_json::to_string(&row)?.as_str())?;
        }
        if proven_absent.is_some() {
            fault!(SCIP_FINALIZE_BEFORE_COMMIT, Some(self), Some(control), "")?;
            control.check()?;
        }
        tx.commit()?;
        *report = finished;
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Budgeted references query (spec 005 § Publication, queries and limits).
// ---------------------------------------------------------------------------

/// Default and maximum `limit` of one references request.
pub const REFERENCES_DEFAULT_LIMIT: usize = 64;
pub const REFERENCES_MAX_LIMIT: usize = 256;
/// The shared examination allowance: seed resolution, definition lookup and
/// reference records all draw from it.
pub const REFERENCES_MAX_EXAMINED: usize = 256;
/// Distinct eligible origin files one window may read.
pub const REFERENCES_MAX_FILES: usize = 64;
/// Definition candidates a query names.
pub const DEFINITION_CANDIDATES: usize = 8;
/// A position seed's containing-occurrence scan leaves the last two records
/// of the shared allowance to the definition lookup and to at least one
/// record of reference work, so a uniquely resolved target can always make
/// progress.
const SEED_SCAN_LIMIT: usize = REFERENCES_MAX_EXAMINED - 2;
/// The definition lookup never consumes the last record of the shared
/// allowance: that record stays reserved for reference work. A lookup that
/// cannot conclude within its share leaves the target `Unfinished` (partial,
/// never certified), but the reference loop still delivers the next
/// reference with a cursor that continues losslessly.
const DEFINITION_SCAN_LIMIT: usize = REFERENCES_MAX_EXAMINED - 1;

/// Exactly one seed form: a 16-hex symbol id prefix, or a source handle plus
/// an absolute byte offset within it.
#[derive(Clone, Debug)]
pub enum ReferencesSeed {
    SymbolId(String),
    Position { handle: String, byte_offset: u64 },
}

#[derive(Clone, Debug)]
pub struct ReferencesRequest {
    pub seed: ReferencesSeed,
    /// 1..=256 reference lines.
    pub limit: usize,
    /// `<path>#<start>`: continue strictly after that reference position.
    pub after: Option<String>,
}

/// Coverage of the answer, for the producer that owns the seed symbol.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Coverage {
    /// Selected snapshot current, fully consumed, and the seed resolved to a
    /// unique definition: complete for supported indexed input, inside the
    /// examined window.
    Complete,
    /// Current but incomplete: the import is unfinished or failed documents,
    /// or the seed has unresolved/ambiguous targets, or some examined record
    /// belongs to another snapshot.
    Partial,
    /// A source changed after the selected snapshot's revision: every
    /// compiler fact is ineligible.
    Stale,
    /// No compiler snapshot is selected.
    Unavailable,
}

impl Coverage {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Complete => "complete",
            Self::Partial => "partial",
            Self::Stale => "stale",
            Self::Unavailable => "unavailable",
        }
    }
}

/// Whether the seed symbol's definition was resolved inside the allowance.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TargetResolution {
    /// Exactly one eligible definition was found and no other exists.
    Unique,
    /// Two or more eligible definitions.
    Ambiguous,
    /// No eligible definition: external or unknown.
    Unknown,
    /// The examination allowance ran out before uniqueness was known.
    Unfinished,
}

#[derive(Clone, Debug, Serialize)]
pub struct DefinitionCandidate {
    pub path: String,
    pub sha256: String,
    pub start: u64,
    pub end: u64,
}

/// One reference occurrence with its enclosing delivery unit.
#[derive(Clone, Debug, Serialize)]
pub struct ReferenceItem {
    pub occurrence_id: String,
    pub path: String,
    pub sha256: String,
    pub start: u64,
    pub end: u64,
    /// One-based line of `start`.
    pub line: u64,
    /// The enclosing delivery unit (full identities).
    pub unit: SourceHandle,
    /// `<kind> <qualified name>`, or the kind alone for a block or an
    /// unnamed unit.
    pub label: String,
    /// Only for a uniquely resolved target.
    pub edge_id: Option<String>,
}

impl ReferenceItem {
    /// The `after` cursor that continues strictly after this reference.
    pub fn cursor(&self) -> String {
        format!("{}#{}-{}", self.path, self.start, self.end)
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct ReferencesOutcome {
    pub freshness: Freshness,
    pub producer: Option<String>,
    /// The selected snapshot tuple the answer was judged against: artifact
    /// digest, manifest digest, source revision and producer/config identity.
    pub snapshot: Option<SnapshotTuple>,
    /// Full 64-hex id of the resolved seed symbol.
    pub symbol_id: Option<String>,
    pub target: Option<TargetResolution>,
    pub definitions: Vec<DefinitionCandidate>,
    pub definitions_truncated: bool,
    pub items: Vec<ReferenceItem>,
    /// Graph records examined in this window, definition lookups included.
    pub examined: usize,
    /// Examined reference occurrences whose target is external, unknown or
    /// ambiguous.
    pub unresolved: usize,
    /// Examined records dropped by the final read as ineligible.
    pub stale: usize,
    pub candidates_full: bool,
    pub coverage: Coverage,
    /// More references exist after the window.
    pub more: bool,
    /// `<path>#<start>` after the last examined reference, when `more`.
    pub resume: Option<String>,
}

enum Seed {
    Symbol(String),
    Position(HandleRef, u64),
}

enum Seeded {
    Found {
        symbol_id: String,
        namespace: String,
    },
    /// A successful line-less answer; `coverage` says why. `truncated` marks
    /// an examination window that ended before the answer was known.
    Unanswerable {
        coverage: Coverage,
        symbol_id: Option<String>,
        producer: Option<String>,
        truncated: bool,
    },
}

fn parse_cursor(raw: &str) -> FResult<(String, u64, u64)> {
    let bad = || invalid("after must be `<path>#<start>-<end>`");
    let (path, span) = raw.rsplit_once('#').ok_or_else(bad)?;
    validate_path(path)?;
    let (start, end) = span.split_once('-').ok_or_else(bad)?;
    let start = canonical_decimal(start.as_bytes()).ok_or_else(bad)?;
    let end = canonical_decimal(end.as_bytes()).ok_or_else(bad)?;
    if start >= end {
        return Err(bad());
    }
    Ok((path.to_owned(), start, end))
}

fn symbol_prefix(raw: &str) -> FResult<String> {
    if raw.len() == 16 && raw.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f')) {
        Ok(raw.to_owned())
    } else {
        Err(invalid("symbol_id must be 16 lowercase hex characters"))
    }
}

fn ambiguous_symbol(ids: &[String], truncated: bool) -> FoundryError {
    let shown: Vec<&str> = ids
        .iter()
        .take(8)
        .map(|id| id.get(..16).unwrap_or(id))
        .collect();
    crate::scip::fail(
        crate::scip::code::AMBIGUOUS_SYMBOL,
        format!(
            "the seed names several symbols; candidates (at most 8): {}{}",
            shown.join(" "),
            if truncated { "; more exist" } else { "" }
        ),
    )
}

fn symbol_not_found(message: &str) -> FoundryError {
    crate::scip::fail(crate::scip::code::SYMBOL_NOT_FOUND, message)
}

/// A scope row is eligible when it was published under the selected snapshot.
struct EligibleScopes {
    scopes: redb::ReadOnlyTable<&'static str, &'static str>,
    namespace: String,
    snapshot_id: String,
    cache: HashMap<String, Option<ScopeRow>>,
}

impl EligibleScopes {
    fn get(&mut self, path: &str) -> FResult<Option<ScopeRow>> {
        if let Some(row) = self.cache.get(path) {
            return Ok(row.clone());
        }
        let row = self
            .scopes
            .get(scope_key(&self.namespace, path).as_str())?
            .map(|raw| stored_row::<ScopeRow>(raw.value(), "compiler scope"))
            .transpose()?
            .filter(|row| row.snapshot_id == self.snapshot_id);
        self.cache.insert(path.to_owned(), row.clone());
        Ok(row)
    }
}

/// One origin source, verified and indexed for line and delivery-unit lookups.
struct FileView {
    body: String,
    line_starts: Vec<usize>,
    documents: Vec<crate::syntax::Document>,
}

impl FileView {
    fn new(path: &str, body: String) -> Self {
        let mut line_starts = vec![0];
        line_starts.extend(body.match_indices('\n').map(|(at, _)| at + 1));
        let documents = crate::syntax::documents(&body, crate::syntax::Lang::from_path(path));
        Self {
            body,
            line_starts,
            documents,
        }
    }

    fn line_of(&self, offset: usize) -> u64 {
        self.line_starts.partition_point(|&start| start <= offset) as u64
    }

    fn unit_at(&self, offset: usize) -> Option<&crate::syntax::DeliveryUnit> {
        let index = self.documents.partition_point(|d| d.start <= offset);
        let document = self.documents.get(index.checked_sub(1)?)?;
        (offset < document.end).then_some(&document.unit)
    }
}

/// Load `path` once into `files`: `None` when the source is absent or no
/// longer has the hash the scope was published from (stale).
fn ensure_file(
    files: &mut HashMap<String, Option<FileView>>,
    sources: &redb::ReadOnlyTable<&'static str, &'static str>,
    chunks: &redb::ReadOnlyTable<&'static str, &'static str>,
    path: &str,
    expected_hash: &str,
) -> FResult<()> {
    if files.contains_key(path) {
        return Ok(());
    }
    let view = match sources.get(path)? {
        None => None,
        Some(raw) => {
            let meta: SourceMeta = crate::store::decode(raw.value(), "source")?;
            if meta.hash == expected_hash {
                Some(FileView::new(
                    path,
                    reconstruct_verified(chunks, path, &meta)?.body,
                ))
            } else {
                None
            }
        }
    };
    files.insert(path.to_owned(), view);
    Ok(())
}

/// The loaded body of `path`, or `None` when its source is absent or no
/// longer carries the hash the scope was published from.
fn body_of<'a>(files: &'a HashMap<String, Option<FileView>>, path: &str) -> Option<&'a str> {
    files
        .get(path)
        .and_then(Option::as_ref)
        .map(|view| view.body.as_str())
}

fn unit_label(unit: &crate::syntax::DeliveryUnit) -> String {
    match unit.qname.as_deref() {
        Some(qname) if !qname.is_empty() => format!("{} {qname}", unit.kind.as_str()),
        _ => unit.kind.as_str().to_owned(),
    }
}

/// A prefix seed: the stored symbol ids starting with it (at most 9 probed).
fn resolve_prefix(
    tx: &redb::ReadTransaction,
    prefix: &str,
    producers: &[(String, ProducerRow)],
) -> FResult<Seeded> {
    if !producers.iter().any(|(_, row)| row.selected.is_some()) {
        return Ok(Seeded::Unanswerable {
            coverage: Coverage::Unavailable,
            symbol_id: None,
            producer: None,
            truncated: false,
        });
    }
    let by_symbol = tx.open_table(COMPILER_BY_SYMBOL)?;
    let mut found: Vec<(String, String)> = Vec::new();
    let mut from = prefix.to_owned();
    while found.len() <= 8 {
        let Some(entry) = by_symbol.range(from.as_str()..)?.next() else {
            break;
        };
        let (key, value) = entry?;
        let key = key.value();
        if !key.starts_with(prefix) {
            break;
        }
        let id = key
            .get(..64)
            .filter(|id| is_symbol_id(id) && key.as_bytes().get(64) == Some(&0))
            .ok_or_else(|| corrupt_key("compiler symbol"))?
            .to_owned();
        from = format!("{id}\u{1}");
        found.push((id, value.value().to_owned()));
    }
    match found.len() {
        0 => Err(symbol_not_found(
            "no stored compiler symbol has that id prefix",
        )),
        1 => {
            let (symbol_id, namespace) = found.remove(0);
            Ok(Seeded::Found {
                symbol_id,
                namespace,
            })
        }
        n => {
            let ids: Vec<String> = found.into_iter().map(|(id, _)| id).collect();
            Err(ambiguous_symbol(&ids, n > 8))
        }
    }
}

/// A position seed: 001's handle precedence (workspace was checked first),
/// then the NARROWEST occurrence containing `offset` across every producer's
/// scope for that source. Several distinct symbols sharing that narrowest
/// span are `ambiguous_symbol` (at most eight candidates). A search the
/// shared allowance did not finish is reported truncated — never as a
/// resolution and never as an absence.
#[allow(clippy::too_many_arguments)] // the seed shares the response's one window state
fn resolve_position(
    tx: &redb::ReadTransaction,
    handle: &HandleRef,
    offset: u64,
    producers: &[(String, ProducerRow)],
    revision: u64,
    examined: &mut usize,
    files: &mut HashMap<String, Option<FileView>>,
    visited: &mut std::collections::BTreeSet<String>,
) -> FResult<Seeded> {
    let sources = tx.open_table(SOURCES)?;
    let meta: SourceMeta = match sources.get(handle.path.as_str())? {
        None => return Err(FoundryError::NotFound),
        Some(raw) => crate::store::decode(raw.value(), "source")?,
    };
    if !meta.hash.starts_with(&handle.sha32) {
        return Err(FoundryError::StaleHandle);
    }
    // The seed's own source read is part of the response's shared file
    // budget: it goes through the same cache every later file uses.
    ensure_file(
        files,
        &sources,
        &tx.open_table(CHUNKS)?,
        &handle.path,
        &meta.hash,
    )?;
    visited.insert(handle.path.clone());
    let Some(body) = body_of(files, &handle.path) else {
        return Err(FoundryError::CorruptSource(format!(
            "{}: the indexed source could not be loaded",
            handle.path
        )));
    };
    let valid_empty = handle.start == 0 && handle.end == 0 && meta.bytes == 0;
    let valid_span = handle.start < handle.end
        && handle.end <= body.len() as u64
        && body.is_char_boundary(handle.start as usize)
        && body.is_char_boundary(handle.end as usize);
    if !valid_empty && !valid_span {
        return Err(FoundryError::InvalidRange);
    }
    if !(handle.start <= offset && offset < handle.end) {
        return Err(FoundryError::InvalidRange);
    }
    let selected_rows: Vec<&(String, ProducerRow)> = producers
        .iter()
        .filter(|(_, row)| row.selected.is_some())
        .collect();
    if selected_rows.is_empty() {
        return Ok(Seeded::Unanswerable {
            coverage: Coverage::Unavailable,
            symbol_id: None,
            producer: None,
            truncated: false,
        });
    }
    // Every selected snapshot predates the source revision: all compiler
    // facts are ineligible, which is decided BEFORE any deep occurrence
    // scan so that scan's exhaustion can never relabel a stale graph.
    if selected_rows.iter().all(|(_, row)| {
        row.selected
            .as_ref()
            .is_some_and(|selected| selected.tuple.source_revision != revision)
    }) {
        return Ok(Seeded::Unanswerable {
            coverage: Coverage::Stale,
            symbol_id: None,
            producer: (selected_rows.len() == 1).then(|| selected_rows[0].0.clone()),
            truncated: false,
        });
    }
    let scopes = tx.open_table(COMPILER_SCOPES)?;
    let occurrences = tx.open_table(COMPILER_OCCURRENCES)?;
    // `(span, symbol id, namespace, eligible)` of every containing
    // occurrence the allowance could reach, across all producers.
    let mut entries: Vec<(u64, String, String, bool)> = Vec::new();
    let mut exhausted = false;
    // Each producer's scope for this source, eligible producers first: once
    // an eligible containing occurrence is known, ineligible producers can
    // no longer contribute to the pool and are not scanned at all.
    let mut scanned: Vec<(&String, ScopeRow, bool)> = Vec::new();
    for (namespace, row) in producers {
        let Some(scope) = scopes
            .get(scope_key(namespace, &handle.path).as_str())?
            .map(|raw| stored_row::<ScopeRow>(raw.value(), "compiler scope"))
            .transpose()?
        else {
            continue;
        };
        // Facts read from other bytes cannot describe this handle.
        if scope.source_hash != meta.hash {
            continue;
        }
        let eligible = row.selected.as_ref().is_some_and(|selected| {
            selected.tuple.source_revision == revision
                && selected.tuple.snapshot_id == scope.snapshot_id
        });
        scanned.push((namespace, scope, eligible));
    }
    scanned.sort_by_key(|(_, _, eligible)| !eligible);
    // The narrowest span among ELIGIBLE containing occurrences found so far.
    let mut narrowest_eligible: Option<u64> = None;
    for (namespace, scope, eligible) in &scanned {
        if !eligible && narrowest_eligible.is_some() {
            continue;
        }
        let low = format!("{namespace}\0{}\0", handle.path);
        let high = format!("{namespace}\0{}\0{:020}", handle.path, offset + 1);
        for entry in occurrences.range(low.as_str()..high.as_str())?.rev() {
            if *examined >= SEED_SCAN_LIMIT {
                exhausted = true;
                break;
            }
            let (key, _) = entry?;
            *examined += 1;
            let parsed = parse_occurrence_key(key.value())?;
            // The parsed range is checked against the already-verified seed
            // body before its span or either stop rule is used: a stored
            // key that does not name a real range of this source is named
            // corruption, never a resolved seed.
            if parsed.end > body.len() as u64
                || !body.is_char_boundary(parsed.start as usize)
                || !body.is_char_boundary(parsed.end as usize)
            {
                return Err(FoundryError::GraphInvalid(format!(
                    "a stored occurrence lies outside its source: {}",
                    handle.path
                )));
            }
            // Descending starts: an occurrence that starts at or before
            // `offset - L` and still contains `offset` is strictly wider
            // than L, so no narrower occurrence and no equal-span tie can
            // remain (ties start after `offset - L` and are still scanned).
            if narrowest_eligible
                .is_some_and(|narrowest| parsed.start.saturating_add(narrowest) <= offset)
            {
                break;
            }
            if parsed.end > offset {
                let span = parsed.end - parsed.start;
                if *eligible {
                    narrowest_eligible = Some(narrowest_eligible.map_or(span, |n| n.min(span)));
                }
                entries.push((span, parsed.symbol_id, (*namespace).clone(), *eligible));
            }
            // Nothing earlier can contain `offset` once the longest stored
            // occurrence starting here would still end at or before it.
            if parsed.start.saturating_add(scope.max_span) <= offset {
                break;
            }
        }
        if exhausted {
            break;
        }
    }
    if exhausted {
        // The containing set was not fully searched: neither the narrowest
        // winner nor an absence may be certified.
        return Ok(Seeded::Unanswerable {
            coverage: Coverage::Partial,
            symbol_id: None,
            producer: None,
            truncated: true,
        });
    }
    // The narrowest span wins among the eligible pool; several symbols of
    // exactly that span are ambiguous.
    let has_eligible = entries.iter().any(|e| e.3);
    let pool: Vec<&(u64, String, String, bool)> =
        entries.iter().filter(|e| e.3 || !has_eligible).collect();
    let narrowest = pool.iter().map(|e| e.0).min();
    let mut matches: BTreeMap<String, (String, bool)> = BTreeMap::new();
    for entry in pool.iter().filter(|e| Some(e.0) == narrowest) {
        let found = matches
            .entry(entry.1.clone())
            .or_insert_with(|| (entry.2.clone(), false));
        found.1 |= entry.3;
    }
    let eligible: Vec<(&String, &String)> = matches
        .iter()
        .filter(|(_, (_, eligible))| *eligible)
        .map(|(id, (namespace, _))| (id, namespace))
        .collect();
    if eligible.len() == 1 {
        return Ok(Seeded::Found {
            symbol_id: eligible[0].0.clone(),
            namespace: eligible[0].1.clone(),
        });
    }
    if eligible.len() > 1 {
        let ids: Vec<String> = eligible.iter().map(|(id, _)| (*id).clone()).collect();
        return Err(ambiguous_symbol(&ids, ids.len() > 8));
    }
    let any_stale = producers.iter().any(|(_, row)| {
        row.selected
            .as_ref()
            .is_some_and(|selected| selected.tuple.source_revision != revision)
    });
    if any_stale || !matches.is_empty() {
        let only = (matches.len() == 1)
            .then(|| matches.iter().next())
            .flatten();
        return Ok(Seeded::Unanswerable {
            coverage: if any_stale {
                Coverage::Stale
            } else {
                Coverage::Partial
            },
            symbol_id: only.map(|(id, _)| id.clone()),
            producer: only.map(|(_, (namespace, _))| namespace.clone()),
            truncated: false,
        });
    }
    Err(symbol_not_found(
        "no compiler occurrence at that byte offset",
    ))
}

impl Engine {
    /// Budget-independent references query: seed resolution, definition
    /// lookup and a bounded, ordered reference window, all inside one final
    /// read transaction. Every fact is revalidated there: its scope must
    /// belong to the producer's selected snapshot, the store's source
    /// revision must still equal that snapshot's, and the origin and
    /// definition sources are re-verified (chunks, hash, range) from the
    /// same transaction and the same shared file budget. Ordering and the
    /// `after` cursor are `(path, start, end)`; `limit` and the shared
    /// 256-record allowance are enforced per record, so a page never
    /// overshoots them.
    pub fn references(&self, request: &ReferencesRequest) -> FResult<ReferencesOutcome> {
        if !(1..=REFERENCES_MAX_LIMIT).contains(&request.limit) {
            return Err(invalid("references limit must be 1..256"));
        }
        let after = request.after.as_deref().map(parse_cursor).transpose()?;
        let seed = match &request.seed {
            ReferencesSeed::SymbolId(raw) => Seed::Symbol(symbol_prefix(raw)?),
            ReferencesSeed::Position {
                handle,
                byte_offset,
            } => Seed::Position(HandleRef::parse(handle)?, *byte_offset),
        };
        let bound = self.workspace_id().ok_or(FoundryError::WorkspaceUnbound)?;
        if let Seed::Position(handle, _) = &seed
            && !bound.starts_with(&handle.ws16)
        {
            return Err(FoundryError::WrongWorkspace);
        }
        let tx = self.db.begin_read()?;
        let freshness = self.freshness_in(&tx)?;
        let producers = read_producers(&tx)?;
        // One cache and one visited-file budget serve the seed read, the
        // definition verification and the reference labeling alike.
        let mut files: HashMap<String, Option<FileView>> = HashMap::new();
        let mut visited: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
        let mut examined = 0usize;
        let seeded = match &seed {
            Seed::Symbol(prefix) => resolve_prefix(&tx, prefix, &producers)?,
            Seed::Position(handle, offset) => resolve_position(
                &tx,
                handle,
                *offset,
                &producers,
                freshness.source_revision,
                &mut examined,
                &mut files,
                &mut visited,
            )?,
        };
        let line_less = |freshness: Freshness,
                         coverage: Coverage,
                         symbol_id: Option<String>,
                         producer: Option<String>,
                         examined: usize,
                         truncated: bool| ReferencesOutcome {
            freshness,
            snapshot: producer
                .as_deref()
                .and_then(|name| producers.iter().find(|(n, _)| n == name))
                .and_then(|(_, row)| row.selected.as_ref())
                .map(|selected| selected.tuple.clone()),
            producer,
            symbol_id,
            target: None,
            definitions: Vec::new(),
            definitions_truncated: truncated,
            items: Vec::new(),
            examined,
            unresolved: 0,
            stale: 0,
            candidates_full: truncated,
            coverage,
            more: false,
            resume: None,
        };
        let (symbol_id, namespace) = match seeded {
            Seeded::Found {
                symbol_id,
                namespace,
            } => (symbol_id, namespace),
            Seeded::Unanswerable {
                coverage,
                symbol_id,
                producer,
                truncated,
            } => {
                return Ok(line_less(
                    freshness, coverage, symbol_id, producer, examined, truncated,
                ));
            }
        };
        let selected = producers
            .iter()
            .find(|(name, _)| *name == namespace)
            .and_then(|(_, row)| row.selected.as_ref());
        let Some(selected) = selected else {
            return Ok(line_less(
                freshness,
                Coverage::Unavailable,
                Some(symbol_id),
                Some(namespace),
                examined,
                false,
            ));
        };
        if selected.tuple.source_revision != freshness.source_revision {
            return Ok(line_less(
                freshness,
                Coverage::Stale,
                Some(symbol_id),
                Some(namespace),
                examined,
                false,
            ));
        }

        let by_symbol = tx.open_table(COMPILER_BY_SYMBOL)?;
        let sources = tx.open_table(SOURCES)?;
        let chunks = tx.open_table(CHUNKS)?;
        let mut scopes = EligibleScopes {
            scopes: tx.open_table(COMPILER_SCOPES)?,
            namespace: namespace.clone(),
            snapshot_id: selected.tuple.snapshot_id.clone(),
            cache: HashMap::new(),
        };
        let mut stale = 0usize;

        // Definition lookup: at most nine eligible records (eight candidates
        // and one more to know that more exist; stale drops draw too), taken
        // from the shared allowance but never from its last record
        // (`DEFINITION_SCAN_LIMIT`).
        // Every candidate that survives is re-verified against its indexed
        // bytes before it can make the target unique.
        let mut definitions: Vec<DefinitionCandidate> = Vec::new();
        let mut eligible_definitions = 0usize;
        let mut definitions_truncated = false;
        let mut lookup_finished = true;
        let low = format!("{symbol_id}\0d\0");
        let high = format!("{symbol_id}\0d\u{1}");
        for entry in by_symbol.range(low.as_str()..high.as_str())? {
            if examined >= DEFINITION_SCAN_LIMIT {
                lookup_finished = false;
                break;
            }
            let (key, _) = entry?;
            examined += 1;
            let parsed = parse_symbol_key(key.value())?;
            let Some(scope) = scopes.get(&parsed.path)? else {
                stale += 1;
                continue;
            };
            if definitions.len() == DEFINITION_CANDIDATES {
                // A ninth eligible candidate only proves that more exist.
                eligible_definitions += 1;
                definitions_truncated = true;
                break;
            }
            // Verify the candidate's source from the same transaction and
            // the same file budget BEFORE it can count as a target: an
            // absent or re-hashed source is a stale drop, an unverifiable
            // range is a named corruption.
            ensure_file(
                &mut files,
                &sources,
                &chunks,
                &parsed.path,
                &scope.source_hash,
            )?;
            let Some(body) = body_of(&files, &parsed.path) else {
                stale += 1;
                continue;
            };
            let verified = parsed.end <= body.len() as u64
                && body.is_char_boundary(parsed.start as usize)
                && body.is_char_boundary(parsed.end as usize);
            if !verified {
                return Err(FoundryError::GraphInvalid(format!(
                    "a stored definition lies outside its source: {}",
                    parsed.path
                )));
            }
            eligible_definitions += 1;
            visited.insert(parsed.path.clone());
            definitions.push(DefinitionCandidate {
                path: parsed.path,
                sha256: scope.source_hash,
                start: parsed.start,
                end: parsed.end,
            });
        }
        if !lookup_finished {
            definitions_truncated = true;
        }
        let target = match eligible_definitions {
            0 if lookup_finished => TargetResolution::Unknown,
            1 if lookup_finished => TargetResolution::Unique,
            n if n >= 2 => TargetResolution::Ambiguous,
            _ => TargetResolution::Unfinished,
        };

        // References in (path, start, end) order, strictly after the cursor.
        // Every cap is checked per record, before that record is examined.
        let low = match &after {
            Some((path, start, end)) => {
                format!("{symbol_id}\0r\0{path}\0{start:020}\0{end:020}\u{1}")
            }
            None => format!("{symbol_id}\0r\0"),
        };
        let high = format!("{symbol_id}\0r\u{1}");
        let mut items: Vec<ReferenceItem> = Vec::new();
        let (mut unresolved, mut want_more) = (0usize, false);
        let mut consumed: Option<(String, u64, u64)> = None;
        for entry in by_symbol.range(low.as_str()..high.as_str())? {
            // The caps are checked on the EXISTENCE of the next key, before
            // it is decoded: an exhausted window never reads (and so never
            // trips over) a record it could not deliver anyway.
            let (key, _) = entry?;
            if items.len() >= request.limit || examined >= REFERENCES_MAX_EXAMINED {
                want_more = true;
                break;
            }
            let parsed = parse_symbol_key(key.value())?;
            let Some(scope) = scopes.get(&parsed.path)? else {
                examined += 1;
                stale += 1;
                consumed = Some((parsed.path, parsed.start, parsed.end));
                continue;
            };
            if !visited.contains(&parsed.path) && visited.len() >= REFERENCES_MAX_FILES {
                want_more = true;
                break;
            }
            examined += 1;
            consumed = Some((parsed.path.clone(), parsed.start, parsed.end));
            ensure_file(
                &mut files,
                &sources,
                &chunks,
                &parsed.path,
                &scope.source_hash,
            )?;
            let Some(Some(view)) = files.get(&parsed.path) else {
                stale += 1;
                continue;
            };
            visited.insert(parsed.path.clone());
            let (start, end) = (parsed.start as usize, parsed.end as usize);
            if end > view.body.len()
                || !view.body.is_char_boundary(start)
                || !view.body.is_char_boundary(end)
            {
                return Err(FoundryError::GraphInvalid(format!(
                    "a stored occurrence lies outside its source: {}",
                    parsed.path
                )));
            }
            if target != TargetResolution::Unique {
                unresolved += 1;
            }
            let (unit_start, unit_end, label) = match view.unit_at(start) {
                Some(unit) => (unit.start as u64, unit.end as u64, unit_label(unit)),
                None => (parsed.start, parsed.end, "block".to_owned()),
            };
            let edge = (target == TargetResolution::Unique)
                .then(|| definitions.first())
                .flatten()
                .map(|definition| {
                    edge_id(
                        &namespace,
                        "references",
                        EdgeEnd {
                            identity: &parsed.path,
                            hash: &scope.source_hash,
                            start: parsed.start,
                            end: parsed.end,
                        },
                        EdgeEnd {
                            identity: &definition.path,
                            hash: &definition.sha256,
                            start: definition.start,
                            end: definition.end,
                        },
                    )
                });
            items.push(ReferenceItem {
                occurrence_id: occurrence_id(
                    &namespace,
                    OccurrenceKind::Reference,
                    &parsed.path,
                    &scope.source_hash,
                    parsed.start,
                    parsed.end,
                    &symbol_id,
                ),
                path: parsed.path.clone(),
                sha256: scope.source_hash.clone(),
                start: parsed.start,
                end: parsed.end,
                line: view.line_of(start),
                unit: SourceHandle {
                    workspace_id: bound.clone(),
                    path: parsed.path,
                    sha256: scope.source_hash,
                    start: unit_start,
                    end: unit_end,
                },
                label,
                edge_id: edge,
            });
        }
        // `more` promises a continuation cursor. A window that stopped with
        // a reference still pending but before consuming ANY record has no
        // cursor to hand back: that is a budget cliff, not an end. It is
        // reported as a filled window (candidates:full) with partial
        // coverage, never as completion and never by clearing a real `more`.
        // The seed scan and the definition lookup both stop short of the
        // reserved record, so this is a backstop: a future change to those
        // limits cannot turn an exhausted window into a false end.
        let no_progress = want_more && consumed.is_none();
        let candidates_full = no_progress
            || examined >= REFERENCES_MAX_EXAMINED
            || visited.len() >= REFERENCES_MAX_FILES;
        let resume = consumed
            .clone()
            .map(|(path, start, end)| format!("{path}#{start}-{end}"));
        let more = want_more && consumed.is_some();
        let coverage = if selected.state == SnapshotState::Complete
            && stale == 0
            && unresolved == 0
            && !no_progress
            && target == TargetResolution::Unique
        {
            Coverage::Complete
        } else {
            Coverage::Partial
        };
        Ok(ReferencesOutcome {
            freshness,
            snapshot: Some(selected.tuple.clone()),
            producer: Some(namespace),
            symbol_id: Some(symbol_id),
            target: Some(target),
            definitions,
            definitions_truncated,
            items,
            examined,
            unresolved,
            stale,
            candidates_full,
            coverage,
            more,
            resume: more.then_some(resume).flatten(),
        })
    }
}

// ---------------------------------------------------------------------------
// Context graph expansion (005 § context(strategy=graph), T003).
// ---------------------------------------------------------------------------

/// Retrieval spans a context's compiler expansion seeds from.
pub(crate) const CONTEXT_GRAPH_SPANS: usize = 3;
/// Delivery units the compiler graph may contribute to one context. With the
/// lexical units they are bounded again by the context's one 32-unit total.
pub(crate) const CONTEXT_GRAPH_UNITS: usize = 32;
/// Graph records one context expansion may examine: seed-span occurrences,
/// definition lookups and reference records all draw from it.
pub(crate) const CONTEXT_GRAPH_EXAMINED: usize = 256;
/// The share of that window the seed-span scan may take, so the symbols it
/// collected can still be resolved.
const CONTEXT_SEED_SCAN: usize = CONTEXT_GRAPH_EXAMINED / 2;

/// The relation one compiler reference was collected under: the snapshot it
/// was read from, the `(path, source sha256)` of its seed, definition and
/// reference scopes, and the three stored rows that state it (the seed
/// occurrence, the unique definition and the reference). The final read must
/// find ALL of it again: a same-revision replacement or a re-import under a
/// new snapshot is a different relation even when some scope row for the
/// same path and hash still exists.
#[derive(Clone, Debug)]
pub(crate) struct ContextWitness {
    pub(crate) namespace: String,
    pub(crate) snapshot_id: String,
    pub(crate) seed: (String, String),
    pub(crate) definition: (String, String),
    pub(crate) reference: (String, String),
    /// Full id of the seed symbol the relation was resolved for.
    pub(crate) symbol_id: String,
    /// `COMPILER_OCCURRENCES` key of the seed occurrence.
    pub(crate) seed_key: String,
    /// `COMPILER_BY_SYMBOL` keys of the unique definition and the reference.
    pub(crate) definition_key: String,
    pub(crate) reference_key: String,
    /// The delivery unit `(path, start, end)` this witness speaks for. The
    /// final pass keeps ONE witness per delivered unit, so a unit holding
    /// many occurrences of the symbol is verified once.
    pub(crate) unit: (String, u64, u64),
}

/// One delivery unit contributed by the compiler graph: the unit that
/// encloses an eligible reference occurrence, exactly as `references`
/// reports it, with the relation that put it here.
pub(crate) struct ContextUnit {
    pub(crate) path: String,
    pub(crate) sha256: String,
    pub(crate) start: u64,
    pub(crate) end: u64,
    pub(crate) line: u64,
    pub(crate) label: String,
    pub(crate) witness: ContextWitness,
}

/// The compiler half of a graph context: evidence still to be revalidated in
/// the caller's final read.
pub(crate) struct ContextGraphUnits {
    /// Units no search unit already covers.
    pub(crate) units: Vec<ContextUnit>,
    /// Relations whose reference unit was ALREADY selected as a search unit:
    /// they add no unit, but they are evidence the eligible graph was used,
    /// so the graph is `ok`, never `unavailable`.
    pub(crate) already_selected: Vec<ContextWitness>,
    pub(crate) examined: usize,
    pub(crate) stale: usize,
    /// A bound was hit (the examination window, the seed-scan share or the
    /// unit cap).
    pub(crate) full: bool,
    /// Every selected snapshot predates the current source revision, the
    /// same state `references` answers `coverage:stale` for.
    pub(crate) state_stale: bool,
}

impl ContextGraphUnits {
    pub(crate) fn empty() -> Self {
        Self {
            units: Vec::new(),
            already_selected: Vec::new(),
            examined: 0,
            stale: 0,
            full: false,
            state_stale: false,
        }
    }
}

/// What a final-read check found about one source range.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SourceProof {
    /// The source is absent or no longer carries the expected hash.
    Missing,
    /// The source's chunks reconstruct to its recorded hash and the range
    /// lies inside the verified body on UTF-8 boundaries.
    Verified,
    /// The source verifies but the stored range does not lie inside it.
    Outside,
}

/// The final read's verdicts: one per witness, in order, and whether the
/// shared record allowance ran out anywhere (an unfinished proof).
pub(crate) struct WitnessVerdicts {
    pub(crate) holds: Vec<bool>,
    pub(crate) unfinished: bool,
}

/// The outcome of re-establishing one symbol's resolution in the final read.
#[derive(Clone, Debug)]
enum FinalResolution {
    /// Exactly one eligible, source-verified definition: its `by_symbol` key.
    Unique(String),
    /// None, or more than one: the symbol no longer resolves uniquely.
    NotUnique,
    /// The allowance ran out before uniqueness could be concluded.
    Unfinished,
}

/// Graph records the final pass may read across ALL delivered units and
/// symbols: witness membership rows (seed, definition and reference) and
/// uniqueness-walk rows alike. Source-body verification is separate source
/// work and is not counted here.
const CONTEXT_FINAL_RECORDS: usize = CONTEXT_GRAPH_EXAMINED;

/// Count one final-pass graph record. A release build carries no counter.
#[cfg(feature = "test-faults")]
fn count_final_record() {
    crate::fault::count_final_graph_record();
}

/// Count one final-pass graph record. A release build carries no counter.
#[cfg(not(feature = "test-faults"))]
fn count_final_record() {}

/// Re-establish `symbol_id`'s resolution under `snapshot_id` in THIS
/// transaction: walk its definition records (each one counted against
/// `allowance`, checked before the record is decoded) and require exactly one
/// whose scope belongs to the snapshot and whose source verifies through
/// `proof`. A definition range outside its verified source is `graph_invalid`.
fn final_resolution(
    by_symbol: &redb::ReadOnlyTable<&'static str, &'static str>,
    scopes: &redb::ReadOnlyTable<&'static str, &'static str>,
    proof: &mut dyn FnMut(&str, &str, u64, u64) -> FResult<SourceProof>,
    namespace: &str,
    snapshot_id: &str,
    symbol_id: &str,
    allowance: &mut usize,
) -> FResult<FinalResolution> {
    let low = format!("{symbol_id}\0d\0");
    let high = format!("{symbol_id}\0d\u{1}");
    let mut found: Option<String> = None;
    for entry in by_symbol.range(low.as_str()..high.as_str())? {
        if *allowance == 0 {
            return Ok(FinalResolution::Unfinished);
        }
        let (key, _) = entry?;
        *allowance -= 1;
        count_final_record();
        let parsed = parse_symbol_key(key.value())?;
        let Some(raw) = scopes.get(scope_key(namespace, &parsed.path).as_str())? else {
            continue;
        };
        let scope: ScopeRow = stored_row(raw.value(), "compiler scope")?;
        if scope.snapshot_id != snapshot_id {
            continue;
        }
        match proof(&parsed.path, &scope.source_hash, parsed.start, parsed.end)? {
            SourceProof::Missing => continue,
            SourceProof::Outside => {
                return Err(FoundryError::GraphInvalid(format!(
                    "a stored definition lies outside its source: {}",
                    parsed.path
                )));
            }
            SourceProof::Verified => {}
        }
        if found.is_some() {
            return Ok(FinalResolution::NotUnique);
        }
        found = Some(key.value().to_owned());
    }
    Ok(found.map_or(FinalResolution::NotUnique, FinalResolution::Unique))
}

/// Whether each witness still holds in THIS transaction:
///
/// * its producer's selected snapshot is still the one it was collected
///   under and is current for the source revision;
/// * its seed, definition and reference scopes still belong to that
///   snapshot and name the same source bytes, and the seed occurrence,
///   definition and reference rows still exist. Those three membership rows
///   are charged against the shared record allowance (cached, so a repeated
///   witness is not re-read), as is every uniqueness-walk record below:
///   EVERY graph record the final pass reads counts, and a depleted
///   allowance leaves the proof unfinished;
/// * the seed, definition and reference SOURCES verify through `proof`
///   (chunks, hash and the stored range) - the caller shares one
///   verified-body cache with context rendering, so each file is
///   reconstructed once. That is separate source work; it is not counted
///   against the record allowance;
/// * the seed symbol STILL resolves to exactly the witnessed unique,
///   source-verified definition. The same snapshot can gain a second
///   definition after collection (a partial import completed by a replay of
///   the same artifact), so presence of the witnessed row is not enough. The
///   verdict is cached per `(namespace, snapshot, symbol)`; an ambiguous or
///   unfinished check drops the symbol's expansion.
///
/// The caller passes ONE witness per delivered unit (at most 32), so many
/// occurrences of a symbol inside one delivered unit cost one membership
/// check, not one per occurrence.
///
/// A malformed producer or scope row, or a range outside its source, is
/// `graph_invalid` (component-local at the caller). Chunk corruption and
/// database errors keep their own named codes: they are never an empty graph.
pub(crate) fn witnesses_hold(
    tx: &redb::ReadTransaction,
    revision: u64,
    witnesses: &[&ContextWitness],
    proof: &mut dyn FnMut(&str, &str, u64, u64) -> FResult<SourceProof>,
) -> FResult<WitnessVerdicts> {
    let producers = read_producers(tx)?;
    let scopes = tx.open_table(COMPILER_SCOPES)?;
    let occurrences = tx.open_table(COMPILER_OCCURRENCES)?;
    let by_symbol = tx.open_table(COMPILER_BY_SYMBOL)?;
    let mut resolutions: HashMap<(String, String, String), FinalResolution> = HashMap::new();
    let mut memberships: HashMap<[String; 5], bool> = HashMap::new();
    let mut allowance = CONTEXT_FINAL_RECORDS;
    let mut unfinished = false;
    let mut holds_all = Vec::with_capacity(witnesses.len());
    // Charge one record before it is read; a depleted allowance leaves the
    // rest of that witness unfinished.
    let charge = |allowance: &mut usize| -> bool {
        if *allowance == 0 {
            return false;
        }
        *allowance -= 1;
        count_final_record();
        true
    };
    for witness in witnesses {
        let current = producers
            .iter()
            .find(|(name, _)| *name == witness.namespace)
            .and_then(|(_, row)| row.selected.as_ref())
            .is_some_and(|selected| {
                selected.tuple.snapshot_id == witness.snapshot_id
                    && selected.tuple.source_revision == revision
            });
        if !current {
            holds_all.push(false);
            continue;
        }
        // Membership: the three stored rows the relation was collected
        // under, charged and cached per witness identity.
        let membership_key = [
            witness.namespace.clone(),
            witness.snapshot_id.clone(),
            witness.seed_key.clone(),
            witness.definition_key.clone(),
            witness.reference_key.clone(),
        ];
        let mut holds = match memberships.get(&membership_key) {
            Some(&known) => known,
            None => {
                let mut known = true;
                for _ in 0..3 {
                    if !charge(&mut allowance) {
                        unfinished = true;
                        known = false;
                        break;
                    }
                }
                if known {
                    known &= occurrences.get(witness.seed_key.as_str())?.is_some();
                    for key in [&witness.definition_key, &witness.reference_key] {
                        known &= by_symbol
                            .get(key.as_str())?
                            .is_some_and(|namespace| namespace.value() == witness.namespace);
                    }
                    for (path, hash) in [&witness.seed, &witness.definition, &witness.reference] {
                        let scope =
                            match scopes.get(scope_key(&witness.namespace, path).as_str())? {
                                Some(raw) => {
                                    Some(stored_row::<ScopeRow>(raw.value(), "compiler scope")?)
                                }
                                None => None,
                            };
                        known &= scope.is_some_and(|row| {
                            row.snapshot_id == witness.snapshot_id && row.source_hash == *hash
                        });
                    }
                }
                memberships.insert(membership_key, known);
                known
            }
        };
        if !holds {
            holds_all.push(false);
            continue;
        }
        // The sources the relation stands on, verified in this transaction.
        let seed = parse_occurrence_key(&witness.seed_key)?;
        let definition = parse_symbol_key(&witness.definition_key)?;
        let reference = parse_symbol_key(&witness.reference_key)?;
        for ((path, hash), start, end) in [
            (&witness.seed, seed.start, seed.end),
            (&witness.definition, definition.start, definition.end),
            (&witness.reference, reference.start, reference.end),
        ] {
            match proof(path, hash, start, end)? {
                SourceProof::Verified => {}
                SourceProof::Missing => holds = false,
                SourceProof::Outside => {
                    return Err(FoundryError::GraphInvalid(format!(
                        "a stored occurrence lies outside its source: {path}"
                    )));
                }
            }
        }
        if !holds {
            holds_all.push(false);
            continue;
        }
        // The symbol must still resolve uniquely to the witnessed definition.
        let cache_key = (
            witness.namespace.clone(),
            witness.snapshot_id.clone(),
            witness.symbol_id.clone(),
        );
        let resolution = match resolutions.get(&cache_key) {
            Some(known) => known.clone(),
            None => {
                let fresh = final_resolution(
                    &by_symbol,
                    &scopes,
                    proof,
                    &witness.namespace,
                    &witness.snapshot_id,
                    &witness.symbol_id,
                    &mut allowance,
                )?;
                resolutions.insert(cache_key, fresh.clone());
                fresh
            }
        };
        match resolution {
            FinalResolution::Unique(key) => holds = key == witness.definition_key,
            FinalResolution::NotUnique => holds = false,
            FinalResolution::Unfinished => {
                holds = false;
                unfinished = true;
            }
        }
        holds_all.push(holds);
    }
    Ok(WitnessVerdicts {
        holds: holds_all,
        unfinished,
    })
}

/// A stored occurrence range must lie inside the verified source body, on
/// UTF-8 boundaries: the guard `resolve_position` applies, reused here.
fn lies_in(body: &str, start: u64, end: u64) -> bool {
    end <= body.len() as u64
        && body.is_char_boundary(start as usize)
        && body.is_char_boundary(end as usize)
}

/// A seed occurrence collected from a retrieval span.
struct ContextSeed {
    namespace: String,
    symbol_id: String,
    seed_key: String,
    path: String,
    hash: String,
}

impl Engine {
    /// Compiler delivery units for `context(strategy=graph)`, seeded by the
    /// first retrieved spans.
    ///
    /// * Seeds: for each span (retrieval order), the eligible occurrences
    ///   overlapping it - those starting inside it, then the earlier ones
    ///   that reach into it - symbol ids ordered within a span, each symbol
    ///   once. Every examined occurrence range is validated against the
    ///   verified source before it counts as overlapping.
    /// * Resolution: only a symbol with a UNIQUE eligible, source-verified
    ///   definition expands. Every inspected definition record is counted
    ///   against the shared window; an ambiguous symbol, or one whose
    ///   uniqueness the window cannot conclude, is skipped (the latter marks
    ///   the window full).
    /// * Units: the delivery units enclosing the symbol's eligible reference
    ///   occurrences, deduplicated against each other and against `taken`.
    ///
    /// Everything shares one examination window; hitting it (or the unit
    /// cap) sets `full`. The result is NOT final: the caller revalidates
    /// every witness in its own final read transaction. A stored occurrence
    /// outside its source is `graph_invalid`, component-local at the caller.
    pub(crate) fn context_graph_units(
        &self,
        spans: &[(String, u64, u64)],
        taken: &BTreeSet<(String, u64, u64)>,
    ) -> FResult<ContextGraphUnits> {
        let tx = self.db.begin_read()?;
        let freshness = self.freshness_in(&tx)?;
        let revision = freshness.source_revision;
        let producers = read_producers(&tx)?;
        let mut outcome = ContextGraphUnits::empty();
        let selected_rows: Vec<&(String, ProducerRow)> = producers
            .iter()
            .filter(|(_, row)| row.selected.is_some())
            .collect();
        if selected_rows.is_empty() {
            return Ok(outcome);
        }
        // The same early state `references` reports `coverage:stale` for:
        // every selected snapshot predates the source revision, so no
        // compiler fact is eligible. Decided before any deep scan.
        outcome.state_stale = selected_rows.iter().all(|(_, row)| {
            row.selected
                .as_ref()
                .is_some_and(|selected| selected.tuple.source_revision != revision)
        });
        if outcome.state_stale {
            return Ok(outcome);
        }
        let occurrences = tx.open_table(COMPILER_OCCURRENCES)?;
        let by_symbol = tx.open_table(COMPILER_BY_SYMBOL)?;
        let scope_table = tx.open_table(COMPILER_SCOPES)?;
        let sources = tx.open_table(SOURCES)?;
        let chunks = tx.open_table(CHUNKS)?;
        let mut files: HashMap<String, Option<FileView>> = HashMap::new();
        let mut ordered: Vec<ContextSeed> = Vec::new();
        let mut done: BTreeSet<String> = BTreeSet::new();
        // Units this expansion already added (`taken` holds the search units).
        let mut seen: BTreeSet<(String, u64, u64)> = BTreeSet::new();
        'spans: for (path, span_start, span_end) in spans.iter().take(CONTEXT_GRAPH_SPANS) {
            // symbol id -> its first seed occurrence, so ids sort per span.
            let mut per_span: BTreeMap<String, ContextSeed> = BTreeMap::new();
            for (namespace, row) in &producers {
                let Some(selected) = &row.selected else {
                    continue;
                };
                if selected.tuple.source_revision != revision {
                    continue;
                }
                let Some(scope) = scope_table
                    .get(scope_key(namespace, path).as_str())?
                    .map(|raw| stored_row::<ScopeRow>(raw.value(), "compiler scope"))
                    .transpose()?
                else {
                    continue;
                };
                if scope.snapshot_id != selected.tuple.snapshot_id {
                    continue;
                }
                // Only a scope published from the current bytes of this
                // source can contribute eligible occurrences.
                ensure_file(&mut files, &sources, &chunks, path, &scope.source_hash)?;
                let Some(body) = body_of(&files, path) else {
                    continue;
                };
                let outside = || {
                    FoundryError::GraphInvalid(format!(
                        "a stored occurrence lies outside its source: {path}"
                    ))
                };
                let mut collect = |key: &str, parsed: OccurrenceKey| {
                    per_span
                        .entry(parsed.symbol_id)
                        .or_insert_with(|| ContextSeed {
                            namespace: namespace.clone(),
                            symbol_id: String::new(),
                            seed_key: key.to_owned(),
                            path: path.clone(),
                            hash: scope.source_hash.clone(),
                        });
                };
                // Occurrences STARTING inside the span all overlap it.
                let inside_low = format!("{namespace}\0{path}\0{span_start:020}");
                let inside_high = format!("{namespace}\0{path}\0{span_end:020}");
                for entry in occurrences.range(inside_low.as_str()..inside_high.as_str())? {
                    if outcome.examined >= CONTEXT_SEED_SCAN {
                        outcome.full = true;
                        break;
                    }
                    let (key, _) = entry?;
                    outcome.examined += 1;
                    let parsed = parse_occurrence_key(key.value())?;
                    if !lies_in(body, parsed.start, parsed.end) {
                        return Err(outside());
                    }
                    collect(key.value(), parsed);
                }
                // Earlier occurrences that reach into it, nearest first,
                // until none can: the longest stored occurrence bounds how
                // far back one can start.
                let before_low = format!("{namespace}\0{path}\0");
                for entry in occurrences
                    .range(before_low.as_str()..inside_low.as_str())?
                    .rev()
                {
                    if outcome.examined >= CONTEXT_SEED_SCAN {
                        outcome.full = true;
                        break;
                    }
                    let (key, _) = entry?;
                    outcome.examined += 1;
                    let parsed = parse_occurrence_key(key.value())?;
                    if !lies_in(body, parsed.start, parsed.end) {
                        return Err(outside());
                    }
                    let (start, end) = (parsed.start, parsed.end);
                    if end > *span_start {
                        collect(key.value(), parsed);
                    }
                    if start.saturating_add(scope.max_span) <= *span_start {
                        break;
                    }
                }
            }
            // What the span collected is kept even when its scan stopped at
            // the window: those symbols still resolve below.
            for (symbol_id, mut seed) in per_span {
                if done.insert(symbol_id.clone()) {
                    seed.symbol_id = symbol_id;
                    ordered.push(seed);
                }
            }
            if outcome.full {
                break 'spans;
            }
        }
        'symbols: for seed in ordered {
            let Some(selected) = producers
                .iter()
                .find(|(name, _)| *name == seed.namespace)
                .and_then(|(_, row)| row.selected.as_ref())
            else {
                continue;
            };
            let mut scopes = EligibleScopes {
                scopes: tx.open_table(COMPILER_SCOPES)?,
                namespace: seed.namespace.clone(),
                snapshot_id: selected.tuple.snapshot_id.clone(),
                cache: HashMap::new(),
            };
            // Definition lookup under the same eligibility as `references`,
            // but the context only expands a UNIQUE eligible definition:
            // the second eligible record ends the check as ambiguous, and a
            // window that runs out first leaves it unfinished.
            let mut definition: Option<(String, String, String)> = None;
            let mut eligible = 0usize;
            let mut finished = true;
            let low = format!("{}\0d\0", seed.symbol_id);
            let high = format!("{}\0d\u{1}", seed.symbol_id);
            for entry in by_symbol.range(low.as_str()..high.as_str())? {
                if outcome.examined >= CONTEXT_GRAPH_EXAMINED {
                    finished = false;
                    break;
                }
                let (key, _) = entry?;
                outcome.examined += 1;
                let parsed = parse_symbol_key(key.value())?;
                let Some(scope) = scopes.get(&parsed.path)? else {
                    outcome.stale += 1;
                    continue;
                };
                ensure_file(
                    &mut files,
                    &sources,
                    &chunks,
                    &parsed.path,
                    &scope.source_hash,
                )?;
                let Some(body) = body_of(&files, &parsed.path) else {
                    outcome.stale += 1;
                    continue;
                };
                if !lies_in(body, parsed.start, parsed.end) {
                    return Err(FoundryError::GraphInvalid(format!(
                        "a stored definition lies outside its source: {}",
                        parsed.path
                    )));
                }
                eligible += 1;
                if eligible > 1 {
                    break;
                }
                definition = Some((key.value().to_owned(), parsed.path, scope.source_hash));
            }
            if !finished {
                outcome.full = true;
                break 'symbols;
            }
            if eligible != 1 {
                continue;
            }
            let Some(definition) = definition else {
                continue;
            };
            let low = format!("{}\0r\0", seed.symbol_id);
            let high = format!("{}\0r\u{1}", seed.symbol_id);
            for entry in by_symbol.range(low.as_str()..high.as_str())? {
                // Both caps are checked on the EXISTENCE of the next record,
                // before it is decoded.
                if outcome.examined >= CONTEXT_GRAPH_EXAMINED
                    || outcome.units.len() >= CONTEXT_GRAPH_UNITS
                {
                    outcome.full = true;
                    break;
                }
                let (key, _) = entry?;
                outcome.examined += 1;
                let parsed = parse_symbol_key(key.value())?;
                let Some(scope) = scopes.get(&parsed.path)? else {
                    outcome.stale += 1;
                    continue;
                };
                ensure_file(
                    &mut files,
                    &sources,
                    &chunks,
                    &parsed.path,
                    &scope.source_hash,
                )?;
                let Some(Some(view)) = files.get(&parsed.path) else {
                    outcome.stale += 1;
                    continue;
                };
                if !lies_in(&view.body, parsed.start, parsed.end) {
                    return Err(FoundryError::GraphInvalid(format!(
                        "a stored occurrence lies outside its source: {}",
                        parsed.path
                    )));
                }
                let start = parsed.start as usize;
                let (unit_start, unit_end, label) = match view.unit_at(start) {
                    Some(unit) => (unit.start as u64, unit.end as u64, unit_label(unit)),
                    None => (parsed.start, parsed.end, "block".to_owned()),
                };
                let unit_key = (parsed.path.clone(), unit_start, unit_end);
                let witness = ContextWitness {
                    namespace: seed.namespace.clone(),
                    snapshot_id: selected.tuple.snapshot_id.clone(),
                    symbol_id: seed.symbol_id.clone(),
                    seed: (seed.path.clone(), seed.hash.clone()),
                    definition: (definition.1.clone(), definition.2.clone()),
                    reference: (parsed.path.clone(), scope.source_hash.clone()),
                    seed_key: seed.seed_key.clone(),
                    definition_key: definition.0.clone(),
                    reference_key: key.value().to_owned(),
                    unit: unit_key.clone(),
                };
                if taken.contains(&unit_key) {
                    // One witness per unit: later occurrences of the same
                    // symbol in the SAME already-selected unit add nothing.
                    if !outcome
                        .already_selected
                        .iter()
                        .any(|kept| kept.unit == unit_key)
                    {
                        outcome.already_selected.push(witness);
                    }
                } else if seen.insert(unit_key) {
                    outcome.units.push(ContextUnit {
                        path: parsed.path,
                        sha256: scope.source_hash,
                        start: unit_start,
                        end: unit_end,
                        line: view.line_of(start),
                        label,
                        witness,
                    });
                }
            }
        }
        Ok(outcome)
    }
}

impl ReferencesRequest {
    /// The request's pure syntax and bounds, checked with no store access:
    /// `limit`, the `after` cursor grammar, the symbol-id prefix or the
    /// handle's v2 grammar. Existence, digest and range stay in the
    /// authoritative read. The MCP adapter runs this before routing,
    /// reservation and engine admission.
    pub fn validate(&self) -> FResult<()> {
        if !(1..=REFERENCES_MAX_LIMIT).contains(&self.limit) {
            return Err(invalid("references limit must be 1..256"));
        }
        self.after.as_deref().map(parse_cursor).transpose()?;
        match &self.seed {
            ReferencesSeed::SymbolId(raw) => {
                symbol_prefix(raw)?;
            }
            ReferencesSeed::Position { handle, .. } => {
                HandleRef::parse(handle)?;
            }
        }
        Ok(())
    }
}

#[cfg(feature = "test-faults")]
impl Engine {
    /// Test seam (feature `test-faults` only; release builds carry none):
    /// overwrite one compiler scope row of an OPEN engine with `raw`, so a
    /// barrier test can corrupt a row between candidate collection and the
    /// final read - something a closed-store rewrite cannot reach.
    pub fn overwrite_compiler_scope_for_tests(
        &self,
        namespace: &str,
        path: &str,
        raw: &str,
    ) -> FResult<()> {
        let tx = self.db.begin_write()?;
        {
            let mut scopes = tx.open_table(COMPILER_SCOPES)?;
            scopes.insert(scope_key(namespace, path).as_str(), raw)?;
        }
        tx.commit()?;
        Ok(())
    }
}

#[cfg(feature = "test-faults")]
impl Engine {
    /// Test seam (feature `test-faults` only): replace the `body` of one
    /// stored chunk of an OPEN engine, keeping its JSON shape, path and
    /// hash, so the source no longer reconstructs to its recorded hash. A
    /// barrier test uses it to corrupt a file AFTER candidate collection,
    /// which a closed-store rewrite cannot reach.
    pub fn overwrite_chunk_body_for_tests(
        &self,
        path: &str,
        ordinal: usize,
        body: &str,
    ) -> FResult<()> {
        let tx = self.db.begin_write()?;
        {
            let mut chunks = tx.open_table(CHUNKS)?;
            let key = format!("{path}\0{ordinal:010}");
            let raw = chunks
                .get(key.as_str())?
                .map(|value| value.value().to_owned())
                .ok_or(FoundryError::NotFound)?;
            let mut value: serde_json::Value = serde_json::from_str(&raw)?;
            value["body"] = body.into();
            chunks.insert(key.as_str(), value.to_string().as_str())?;
        }
        tx.commit()?;
        Ok(())
    }
}

#[cfg(feature = "test-faults")]
impl Engine {
    /// Test seam (feature `test-faults` only): insert raw `compiler_by_symbol`
    /// rows of an OPEN engine, each valued `namespace`. A barrier test uses it
    /// to add records AFTER candidate collection (for example hundreds of
    /// ineligible definition rows that exhaust the final uniqueness allowance).
    pub fn insert_compiler_by_symbol_for_tests(
        &self,
        keys: &[String],
        namespace: &str,
    ) -> FResult<()> {
        let tx = self.db.begin_write()?;
        {
            let mut by_symbol = tx.open_table(COMPILER_BY_SYMBOL)?;
            for key in keys {
                by_symbol.insert(key.as_str(), namespace)?;
            }
        }
        tx.commit()?;
        Ok(())
    }
}
