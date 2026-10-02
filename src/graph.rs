use crate::error::{FResult, FoundryError};
use crate::store::{Engine, SOURCES, SourceMeta, validate_path};
use redb::{
    MultimapTableDefinition, ReadableDatabase, ReadableTable, TableDefinition, WriteTransaction,
};
use serde::{Deserialize, Serialize};
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
