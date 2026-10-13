//! The manual file-neighborhood edge graph: endpoints and edges, provider
//! bundles, execution evidence and observations, and the `Engine` edge
//! writes and neighborhood queries. The compiler facts, their storage key
//! codec and `invalid` stay in the parent; the references query is a sibling.
use super::*;

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
