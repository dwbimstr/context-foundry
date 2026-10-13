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

mod references;
pub use references::{
    Coverage, DEFINITION_CANDIDATES, DefinitionCandidate, REFERENCES_DEFAULT_LIMIT,
    REFERENCES_MAX_EXAMINED, REFERENCES_MAX_FILES, REFERENCES_MAX_LIMIT, ReferenceItem,
    ReferencesOutcome, ReferencesRequest, ReferencesSeed, TargetResolution,
};
pub(crate) use references::{DoorReads, ExactResolution};

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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    execution: Option<Execution>,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub execution: Option<Execution>,
}

/// An explicit producer's completed execution receipt, not sandbox attestation
/// or a claim that all tests/paths ran. Its artifact digest binds exact import bytes.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Execution {
    pub artifact_sha256: String,
    pub run: String,
    pub command: Vec<String>,
    pub exit_code: i32,
    pub duration_ms: u64,
    pub test: Endpoint,
    /// Legacy receipts validate only caller/target/test endpoints. V2 receipts
    /// additionally bind every observation to one admitted source snapshot.
    #[serde(default)]
    pub freshness: ExecutionFreshness,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub producer: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub snapshot: Option<ExecutionSnapshot>,
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionFreshness {
    #[default]
    EndpointOnly,
    WorkspaceRevision,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExecutionSnapshot {
    pub workspace_id: String,
    pub source_revision: u64,
    pub inputs_sha256: String,
    pub input_count: usize,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ExecutionSource {
    path: String,
    sha256: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ExecutionInput {
    v: u32,
    producer: String,
    run: String,
    command: Vec<String>,
    completed: bool,
    exit_code: i32,
    duration_ms: u64,
    test: Endpoint,
    observations: Vec<Observation>,
    #[serde(default)]
    scope: Option<String>,
    #[serde(default)]
    workspace_id: Option<String>,
    #[serde(default)]
    source_revision: Option<u64>,
    #[serde(default)]
    inputs: Option<Vec<ExecutionSource>>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Observation {
    from: Endpoint,
    to: Endpoint,
    kind: String,
    count: u64,
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

pub(crate) fn init(tx: &WriteTransaction) -> FResult<()> {
    tx.open_multimap_table(OUT)?;
    tx.open_multimap_table(IN)?;
    tx.open_table(PROVIDERS)?;
    Ok(())
}

impl Engine {
    /// Replace one producer's entire bundle atomically; other producers remain intact.
    pub fn import_graph(&self, bundle: &GraphBundle) -> FResult<usize> {
        self.import_graph_execution(bundle, None, None)
    }

    /// Import caller-supplied runtime calls and positively executed source lines.
    /// This operation reads no workspace executable and launches no process.
    pub fn import_execution(&self, bytes: &[u8]) -> FResult<usize> {
        if bytes.len() > 4 * 1024 * 1024 {
            return Err(invalid("execution artifact exceeds 4 MiB"));
        }
        let input: ExecutionInput = serde_json::from_slice(bytes)
            .map_err(|e| invalid(format!("invalid execution artifact: {e}")))?;
        if ![1, 2].contains(&input.v)
            || !input.completed
            || input.run.trim().is_empty()
            || input.run.len() > 128
            || input.producer.trim().is_empty()
            || input.producer.len() > 100
            || input.command.is_empty()
            || input.command.len() > 32
            || input.command.iter().map(String::len).sum::<usize>() > 4096
            || input.observations.len() > 16384
            || input
                .observations
                .iter()
                .any(|o| o.count == 0 || !["calls", "covers"].contains(&o.kind.as_str()))
        {
            return Err(invalid(
                "execution requires a completed bounded run and positive calls/covers observations",
            ));
        }
        let snapshot = if input.v == 2 {
            let scope = input
                .scope
                .as_deref()
                .filter(|s| !s.trim().is_empty() && s.len() <= 128);
            let workspace_id = input.workspace_id.as_ref();
            let revision = input.source_revision;
            let inputs = input.inputs.as_ref();
            let (Some(_), Some(workspace_id), Some(source_revision), Some(inputs)) =
                (scope, workspace_id, revision, inputs)
            else {
                return Err(invalid(
                    "execution v2 requires scope, workspace_id, source_revision and inputs",
                ));
            };
            if inputs.is_empty() || inputs.len() > 16_384 {
                return Err(invalid(
                    "execution input manifest requires 1..16384 sources",
                ));
            }
            Some(ExecutionSnapshot {
                workspace_id: workspace_id.clone(),
                source_revision,
                inputs_sha256: crate::digest(&serde_json::to_vec(inputs)?),
                input_count: inputs.len(),
            })
        } else {
            if input.scope.is_some()
                || input.workspace_id.is_some()
                || input.source_revision.is_some()
                || input.inputs.is_some()
            {
                return Err(invalid(
                    "execution v1 cannot claim snapshot or collection scope",
                ));
            }
            None
        };
        // A collection is replaced, never appended indefinitely. Hash the pair
        // rather than concatenate user strings, and keep legacy keys disjoint.
        let provider = if input.v == 2 {
            format!(
                "execution-v2:{}",
                crate::digest(&serde_json::to_vec(&(&input.producer, &input.scope,))?)
            )
        } else {
            format!("execution:{}", input.producer)
        };
        let digest = crate::digest(bytes);
        let execution = Execution {
            artifact_sha256: digest.clone(),
            run: input.run,
            command: input.command,
            exit_code: input.exit_code,
            duration_ms: input.duration_ms,
            test: input.test,
            freshness: if snapshot.is_some() {
                ExecutionFreshness::WorkspaceRevision
            } else {
                ExecutionFreshness::EndpointOnly
            },
            producer: Some(input.producer),
            scope: input.scope,
            snapshot,
        };
        let bundle = GraphBundle {
            provider,
            revision: digest,
            edges: input
                .observations
                .into_iter()
                .map(|o| Edge {
                    from: o.from,
                    to: o.to,
                    kind: o.kind,
                    evidence: "observed_by_producer".into(),
                })
                .collect(),
        };
        self.import_graph_execution(&bundle, Some(&execution), input.inputs.as_deref())
    }

    fn import_graph_execution(
        &self,
        bundle: &GraphBundle,
        execution: Option<&Execution>,
        inputs: Option<&[ExecutionSource]>,
    ) -> FResult<usize> {
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
            // Validate the complete supplied manifest in the same transaction as
            // replacement, including empty observations. The revision receipt is
            // conservative: any admitted-source edit invalidates every run edge,
            // even an edit to a source absent from its explicit manifest. Retain
            // only a small receipt per edge instead of copying a large manifest.
            if let Some(execution) = execution {
                let mut admitted = BTreeMap::new();
                if let Some(snapshot) = &execution.snapshot {
                    if self.workspace_id().as_deref() != Some(snapshot.workspace_id.as_str())
                        || read_counter(&tx.open_table(META)?, "source_revision")?
                            != snapshot.source_revision
                    {
                        return Err(invalid("stale execution source snapshot"));
                    }
                    for source in inputs.ok_or_else(|| invalid("missing execution inputs"))? {
                        validate_path(&source.path)?;
                        if admitted
                            .insert(source.path.as_str(), source.sha256.as_str())
                            .is_some()
                        {
                            return Err(invalid("duplicate execution input"));
                        }
                        let meta = sources
                            .get(source.path.as_str())?
                            .ok_or_else(|| invalid("execution input is absent"))?;
                        let meta: SourceMeta = crate::store::decode(meta.value(), "source")?;
                        if source.sha256 != meta.hash {
                            return Err(invalid("stale execution input"));
                        }
                    }
                    for endpoint in std::iter::once(&execution.test)
                        .chain(bundle.edges.iter().flat_map(|edge| [&edge.from, &edge.to]))
                    {
                        if admitted.get(endpoint.path.as_str()).copied()
                            != Some(endpoint.hash.as_str())
                        {
                            return Err(invalid(
                                "execution endpoint is absent from its input manifest",
                            ));
                        }
                    }
                }
                // A completed empty observation set may replace this collection,
                // but its observing test must still be a valid current endpoint.
                validate_path(&execution.test.path)?;
                let meta = sources
                    .get(execution.test.path.as_str())?
                    .ok_or_else(|| invalid("execution test is absent"))?;
                let meta: SourceMeta = crate::store::decode(meta.value(), "source")?;
                if execution.test.line == 0
                    || execution.test.line > meta.lines
                    || execution.test.symbol.len() > 1024
                    || execution.test.hash != meta.hash
                {
                    return Err(invalid("invalid or stale execution test"));
                }
            }
            for edge in &bundle.edges {
                if !(["calls", "references", "imports", "contains", "depends_on"]
                    .contains(&edge.kind.as_str())
                    || execution.is_some() && edge.kind == "covers")
                {
                    return Err(invalid("unknown edge kind"));
                }
                if !(["resolved", "syntactic", "inferred", "manual"]
                    .contains(&edge.evidence.as_str())
                    || execution.is_some() && edge.evidence == "observed_by_producer")
                {
                    return Err(invalid("unknown evidence class"));
                }
                for endpoint in [&edge.from, &edge.to]
                    .into_iter()
                    .chain(execution.map(|e| &e.test))
                {
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
                    execution: execution.cloned(),
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
        let source_revision = read_counter(&tx.open_table(META)?, "source_revision")?;
        let workspace_id = self.workspace_id();
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
                let mut fresh = execution_current(
                    stored.execution.as_ref(),
                    workspace_id.as_deref(),
                    source_revision,
                );
                for endpoint in [&stored.edge.from, &stored.edge.to]
                    .into_iter()
                    .chain(stored.execution.as_ref().map(|e| &e.test))
                {
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
                        execution: stored.execution,
                    });
                }
            }
        }
        Ok(result)
    }
}

fn execution_current(
    execution: Option<&Execution>,
    workspace_id: Option<&str>,
    revision: u64,
) -> bool {
    match execution {
        None => true,
        Some(execution) => match (&execution.freshness, &execution.snapshot) {
            (ExecutionFreshness::EndpointOnly, None) => true,
            (ExecutionFreshness::WorkspaceRevision, Some(snapshot)) => {
                workspace_id == Some(snapshot.workspace_id.as_str())
                    && revision == snapshot.source_revision
            }
            _ => false,
        },
    }
}

pub struct ObservedEvidence {
    pub observation: GraphEvidence,
    pub from: SourceHandle,
    pub to: SourceHandle,
    pub test: SourceHandle,
}

impl Engine {
    /// Bounded runtime neighbors, reconstructed in the task's final read snapshot.
    /// Static graph rows never acquire an execution label through this path.
    pub(crate) fn execution_evidence_in(
        &self,
        tx: &redb::ReadTransaction,
        paths: &[String],
        control: &crate::Control,
    ) -> FResult<(Vec<ObservedEvidence>, bool)> {
        let mut result = Vec::new();
        let mut seen = BTreeSet::new();
        let mut examined = 0;
        let wid = self.workspace_id().ok_or(FoundryError::WorkspaceUnbound)?;
        let source_revision = read_counter(&tx.open_table(META)?, "source_revision")?;
        let endpoint = |e: &Endpoint| -> FResult<Option<SourceHandle>> {
            let Some((meta, body)) = crate::boundary::source_in(tx, &e.path)? else {
                return Ok(None);
            };
            if meta.hash != e.hash {
                return Ok(None);
            }
            let mut start = 0usize;
            for (i, line) in body.split_inclusive('\n').enumerate() {
                if i + 1 == e.line {
                    return Ok(Some(SourceHandle {
                        workspace_id: wid.clone(),
                        path: e.path.clone(),
                        sha256: e.hash.clone(),
                        start: start as u64,
                        end: (start + line.len()) as u64,
                    }));
                }
                start += line.len();
            }
            Ok(None)
        };
        for definition in [OUT, IN] {
            let table = tx.open_multimap_table(definition)?;
            for path in paths.iter().take(8) {
                for row in table.get(path.as_str())? {
                    control.check()?;
                    if examined == 128 || result.len() == 16 {
                        return Ok((result, true));
                    }
                    examined += 1;
                    let raw = row?;
                    if !seen.insert(raw.value().to_owned()) {
                        continue;
                    }
                    let stored = decode_stored(raw.value())?;
                    let Some(execution) = &stored.execution else {
                        continue;
                    };
                    if stored.edge.evidence != "observed_by_producer"
                        || !execution_current(Some(execution), Some(&wid), source_revision)
                    {
                        continue;
                    }
                    let (Some(from), Some(to), Some(test)) = (
                        endpoint(&stored.edge.from)?,
                        endpoint(&stored.edge.to)?,
                        endpoint(&execution.test)?,
                    ) else {
                        continue;
                    };
                    result.push(ObservedEvidence {
                        observation: GraphEvidence {
                            provider: stored.provider,
                            revision: stored.revision,
                            edge: stored.edge,
                            execution: stored.execution,
                        },
                        from,
                        to,
                        test,
                    });
                }
            }
        }
        Ok((result, paths.len() > 8))
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
