use crate::store::{Engine, SOURCES, SourceMeta, validate_path};
use anyhow::{Result, ensure};
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
}

pub(crate) fn init(tx: &WriteTransaction) -> Result<()> {
    tx.open_multimap_table(OUT)?;
    tx.open_multimap_table(IN)?;
    tx.open_table(PROVIDERS)?;
    Ok(())
}

impl Engine {
    /// Replace one producer's entire bundle atomically; other producers remain intact.
    pub fn import_graph(&self, bundle: &GraphBundle) -> Result<usize> {
        ensure!(
            !bundle.provider.trim().is_empty() && bundle.provider.len() <= 128,
            "invalid provider"
        );
        ensure!(
            !bundle.revision.trim().is_empty() && bundle.revision.len() <= 128,
            "invalid provider revision"
        );
        ensure!(bundle.edges.len() <= 100_000, "bundle exceeds 100000 edges");
        let tx = self.db.begin_write()?;
        let mut encoded = BTreeSet::new();
        {
            let sources = tx.open_table(SOURCES)?;
            for edge in &bundle.edges {
                ensure!(
                    ["calls", "references", "imports", "contains", "depends_on"]
                        .contains(&edge.kind.as_str()),
                    "unknown edge kind"
                );
                ensure!(
                    ["resolved", "syntactic", "inferred", "manual"]
                        .contains(&edge.evidence.as_str()),
                    "unknown evidence class"
                );
                for endpoint in [&edge.from, &edge.to] {
                    validate_path(&endpoint.path)?;
                    ensure!(
                        endpoint.line > 0 && endpoint.symbol.len() <= 1024,
                        "invalid endpoint"
                    );
                    let meta = sources
                        .get(endpoint.path.as_str())?
                        .ok_or_else(|| anyhow::anyhow!("graph source absent: {}", endpoint.path))?;
                    let meta: SourceMeta = serde_json::from_str(meta.value())?;
                    ensure!(
                        meta.hash == endpoint.hash,
                        "stale graph source: {}",
                        endpoint.path
                    );
                    ensure!(
                        endpoint.line <= meta.lines,
                        "graph line outside source: {}",
                        endpoint.path
                    );
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
                .map(|v| serde_json::from_str::<Vec<String>>(v.value()))
                .transpose()?
                .unwrap_or_default();
            let mut out = tx.open_multimap_table(OUT)?;
            let mut incoming = tx.open_multimap_table(IN)?;
            for raw in previous {
                let stored: StoredEdge = serde_json::from_str(&raw)?;
                out.remove(stored.edge.from.path.as_str(), raw.as_str())?;
                incoming.remove(stored.edge.to.path.as_str(), raw.as_str())?;
            }
            for raw in &encoded {
                let stored: StoredEdge = serde_json::from_str(raw)?;
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
    ) -> Result<GraphResult> {
        validate_path(seed)?;
        ensure!(
            depth <= 4 && (1..=256).contains(&max_edges),
            "graph bounds: depth 0..4, edges 1..256"
        );
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
                let stored: StoredEdge = serde_json::from_str(raw.value())?;
                let mut fresh = true;
                for endpoint in [&stored.edge.from, &stored.edge.to] {
                    let current = sources
                        .get(endpoint.path.as_str())?
                        .map(|v| serde_json::from_str::<SourceMeta>(v.value()))
                        .transpose()?;
                    fresh &= current.is_some_and(|m| m.hash == endpoint.hash);
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
                    });
                }
            }
        }
        Ok(result)
    }
}
