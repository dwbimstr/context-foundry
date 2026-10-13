//! 005 compiler facts: scoped SCIP producers, scopes and occurrences, their
//! identities and rows, and the `Engine` import, selection and listing.
//! `OccurrenceKind` and the storage key codec, shared with the references query,
//! stay in the parent module.
use super::*;

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
