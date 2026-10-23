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

mod compiler;
mod edges;
mod references;
pub(crate) use compiler::{
    COMPILER_BY_SYMBOL, COMPILER_OCCURRENCES, COMPILER_PRODUCERS, COMPILER_SCOPES, NewOccurrence,
    ScopeWrite, check_compiler_tables, init_compiler,
};
pub use compiler::{
    EdgeEnd, OccurrenceValue, ProducerIdentity, ProducerRow, ScopeRow, ScopeStatus,
    SelectedSnapshot, SnapshotState, SnapshotTuple, edge_id, occurrence_id, symbol_id,
};
pub(crate) use edges::init;
pub use edges::{
    Edge, Endpoint, Execution, ExecutionFreshness, ExecutionSnapshot, GraphBundle, GraphEvidence,
    GraphResult, ObservedEvidence,
};
pub use references::{
    Coverage, DEFINITION_CANDIDATES, DefinitionCandidate, REFERENCES_DEFAULT_LIMIT,
    REFERENCES_MAX_EXAMINED, REFERENCES_MAX_FILES, REFERENCES_MAX_LIMIT, ReferenceItem,
    ReferencesOutcome, ReferencesRequest, ReferencesSeed, TargetResolution,
};
pub(crate) use references::{DoorReads, ExactResolution, FileView};

fn invalid(message: impl Into<String>) -> FoundryError {
    FoundryError::InvalidArgument(message.into())
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
