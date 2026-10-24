//! Source-bound, explicitly imported derived knowledge (010 T001).
//! Citations establish provenance, never the truth of model-written prose.
//! Source hashes and row revisions are checked in the delivery snapshot.
use crate::store::{self, CHUNKS, HandleRef, META, PENDING, SOURCES, SourceMeta};
use crate::{Control, Engine, FResult, FoundryError};
use redb::{ReadableDatabase, ReadableTable, TableDefinition};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

pub(crate) const BRIEFS: TableDefinition<&str, &str> = TableDefinition::new("source_briefs");
pub const MAX_REQUEST_BYTES: usize = 256 * 1024;
pub(crate) const MAX_RECORD_BYTES: usize = 2 * MAX_REQUEST_BYTES;
const MAX_TEXT: usize = 16 * 1024;
const MAX_SPANS: usize = 32;
pub(crate) const CONTEXT_HITS: usize = 8;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Claim {
    pub text: String,
    /// Zero-based indices into `cited_spans`.
    pub citations: Vec<usize>,
    /// Only declared identifiers are checked for byte presence in the cited spans.
    pub identifiers: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Input {
    /// First span owns the brief; later spans are dependencies (including tests).
    pub cited_spans: Vec<String>,
    pub recipe_sha256: String,
    pub model: String,
    pub artifact_sha256: String,
    pub prompt_sha256: String,
    pub generated_at_unix_ms: u64,
    pub purpose: Vec<Claim>,
    pub behavior: Vec<Claim>,
    pub provides: Vec<Claim>,
    pub consumes: Vec<Claim>,
    pub constraints: Vec<Claim>,
    pub tests: Vec<Claim>,
    pub unknowns: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    Derived,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Record {
    pub kind: Kind,
    pub id: String,
    pub workspace_id: String,
    pub revision: u64,
    pub input_sha256: String,
    /// Full authoritative digests, one per unique cited path.
    pub dependencies: BTreeMap<String, String>,
    pub brief: Input,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub derivation: Option<crate::knowledge::Derivation>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub workspace_id: String,
    pub command: Command,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
pub enum Command {
    Put {
        brief: Box<Input>,
        expected_revision: Option<u64>,
    },
    Get {
        id: String,
    },
    /// Explicit historical export may include stale text, labeled ineligible.
    Export {
        id: String,
    },
    Forget {
        id: String,
        expected_revision: u64,
    },
    Search {
        query: String,
        limit: usize,
        max_bytes: usize,
    },
}

fn invalid(message: &str) -> FoundryError {
    FoundryError::InvalidArgument(message.into())
}
fn is_digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
pub fn parse(bytes: &[u8]) -> FResult<Request> {
    if bytes.len() > MAX_REQUEST_BYTES {
        return Err(invalid("brief request exceeds 256 KiB"));
    }
    serde_json::from_slice(bytes).map_err(|_| invalid("expected a strict brief JSON request"))
}
impl Input {
    fn claims(&self) -> impl Iterator<Item = &Claim> {
        [
            &self.purpose,
            &self.behavior,
            &self.provides,
            &self.consumes,
            &self.constraints,
            &self.tests,
        ]
        .into_iter()
        .flatten()
    }
    fn validate(&self) -> FResult<Vec<HandleRef>> {
        if self.cited_spans.is_empty()
            || self.cited_spans.len() > MAX_SPANS
            || self.purpose.is_empty()
            || self.behavior.is_empty()
            || self.model.trim().is_empty()
            || self.model.len() > 256
            || ![
                &self.recipe_sha256,
                &self.prompt_sha256,
                &self.artifact_sha256,
            ]
            .into_iter()
            .all(|v| is_digest(v))
        {
            return Err(invalid(
                "brief requires purpose, behavior, 1..32 spans and generator digests",
            ));
        }
        let spans = self
            .cited_spans
            .iter()
            .map(|h| HandleRef::parse(h))
            .collect::<FResult<Vec<_>>>()?;
        let mut text_bytes = 0usize;
        for claim in self.claims() {
            if claim.text.trim().is_empty()
                || claim.citations.is_empty()
                || claim.citations.len() > MAX_SPANS
                || claim.citations.iter().any(|&n| n >= spans.len())
                || claim.identifiers.len() > 32
            {
                return Err(invalid("every claim needs text and valid citation indices"));
            }
            text_bytes = text_bytes.saturating_add(claim.text.len());
            for identifier in &claim.identifiers {
                if identifier.trim().is_empty() || identifier.len() > 256 {
                    return Err(invalid(
                        "declared identifiers must contain 1..256 nonblank bytes",
                    ));
                }
                text_bytes = text_bytes.saturating_add(identifier.len());
            }
        }
        for unknown in &self.unknowns {
            if unknown.trim().is_empty() {
                return Err(invalid("unknowns must be nonblank"));
            }
            text_bytes = text_bytes.saturating_add(unknown.len());
        }
        if text_bytes > MAX_TEXT || serde_json::to_vec(self)?.len() > MAX_REQUEST_BYTES - 1024 {
            return Err(invalid("brief exceeds 16 KiB of text or 256 KiB of JSON"));
        }
        Ok(spans)
    }
    pub(crate) fn searchable_text(&self) -> String {
        // Unknowns are deliberately excluded: an unknown capability is not a hit.
        self.claims()
            .map(|c| c.text.as_str())
            .collect::<Vec<_>>()
            .join("\n")
    }
}
pub(crate) fn identity(workspace: &str, path: &str, recipe: &str) -> FResult<String> {
    Ok(crate::digest(&serde_json::to_vec(&(
        workspace, path, recipe,
    ))?))
}
fn input_identity(
    workspace: &str,
    brief: &Input,
    dependencies: &BTreeMap<String, String>,
    derivation: Option<&crate::knowledge::Derivation>,
) -> FResult<String> {
    let base = crate::digest(&serde_json::to_vec(&(
        workspace,
        &brief.cited_spans,
        &brief.recipe_sha256,
        &brief.model,
        &brief.artifact_sha256,
        &brief.prompt_sha256,
        dependencies,
    ))?);
    match derivation {
        None => Ok(base),
        Some(derivation) => Ok(crate::digest(&serde_json::to_vec(&(base, derivation))?)),
    }
}
pub(crate) fn pending_key(id: &str) -> String {
    format!("brief:{id}")
}

pub(crate) fn decode_row(id: &str, raw: &str) -> FResult<Record> {
    if raw.len() > MAX_RECORD_BYTES {
        return Err(corrupt_row(id));
    }
    let record: Record = serde_json::from_str(raw).map_err(|_| corrupt_row(id))?;
    check_row(id, &record)?;
    Ok(record)
}
/// A record embedded in another row (a knowledge cache entry), already
/// parsed: `decode_row`'s bound on its encoding, then the same checks. The
/// bound is not implied by the checks: escaped dependency paths can exceed it.
pub(crate) fn check_embedded(record: &Record) -> FResult<()> {
    if serde_json::to_string(record)?.len() > MAX_RECORD_BYTES {
        return Err(corrupt_row(&record.id));
    }
    check_row(&record.id, record)
}
fn corrupt_row(id: &str) -> FoundryError {
    FoundryError::CorruptStore(format!("invalid source brief {id:?}"))
}
fn check_row(id: &str, record: &Record) -> FResult<()> {
    let checked = || -> FResult<()> {
        let spans = record.brief.validate()?;
        if let Some(derivation) = &record.derivation {
            derivation.validate()?;
            if derivation.children.contains_key(id) {
                return Err(invalid("cyclic lineage"));
            }
        }
        if !is_digest(id)
            || !is_digest(&record.workspace_id)
            || record.revision == 0
            || record.id != id
            || identity(
                &record.workspace_id,
                &spans[0].path,
                &record.brief.recipe_sha256,
            )? != id
            || input_identity(
                &record.workspace_id,
                &record.brief,
                &record.dependencies,
                record.derivation.as_ref(),
            )? != record.input_sha256
            || record.dependencies.len()
                != spans.iter().map(|h| &h.path).collect::<BTreeSet<_>>().len()
            || spans.iter().any(|h| {
                !record.workspace_id.starts_with(&h.ws16)
                    || !record
                        .dependencies
                        .get(&h.path)
                        .is_some_and(|sha| is_digest(sha) && sha.starts_with(&h.sha32))
            })
        {
            return Err(invalid("inconsistent identity"));
        }
        Ok(())
    };
    checked().map_err(|_| corrupt_row(id))
}
pub(crate) fn eligible(
    record: &Record,
    sources: &impl ReadableTable<&'static str, &'static str>,
    briefs: &impl ReadableTable<&'static str, &'static str>,
) -> FResult<bool> {
    for (path, digest) in &record.dependencies {
        let Some(raw) = sources.get(path.as_str())? else {
            return Ok(false);
        };
        if store::decode::<SourceMeta>(raw.value(), "source")?.hash != *digest {
            return Ok(false);
        }
    }
    if let Some(lineage) = &record.derivation {
        for (id, expected) in &lineage.children {
            let Some(raw) = briefs.get(id.as_str())? else {
                return Ok(false);
            };
            let child = decode_row(id, raw.value())?;
            if crate::knowledge::fingerprint(&child)? != *expected
                || child
                    .dependencies
                    .iter()
                    .any(|(path, hash)| record.dependencies.get(path) != Some(hash))
                || !child.derivation.as_ref().is_some_and(|d| {
                    d.depth < lineage.depth
                        && d.children
                            .iter()
                            .all(|(id, hash)| lineage.children.get(id) == Some(hash))
                })
            {
                return Ok(false);
            }
        }
    }
    Ok(true)
}

pub(crate) struct Generation<'a> {
    pub derivation: &'a crate::knowledge::Derivation,
    pub usage: &'a crate::knowledge::Usage,
    pub cache_bytes: u64,
}

impl Engine {
    /// Explicit import/control seam. All mutations are durable before derived work;
    /// callers may drain the ordinary index or run `repair-index` later.
    pub fn brief(&self, request: &Request, control: &Control) -> FResult<Value> {
        control.check()?;
        if self.workspace_id().as_deref() != Some(&request.workspace_id) {
            return Err(FoundryError::WrongWorkspace);
        }
        if serde_json::to_vec(request)?.len() > MAX_REQUEST_BYTES {
            return Err(invalid("brief request exceeds 256 KiB"));
        }
        let workspace = &request.workspace_id;
        match &request.command {
            Command::Put {
                brief,
                expected_revision,
            } => self.put_brief(workspace, brief, *expected_revision, None, control),
            Command::Get { id } | Command::Export { id } => {
                if !is_digest(id) {
                    return Err(invalid("brief id must be a SHA-256 digest"));
                }
                let tx = self.db.begin_read()?;
                let table = tx.open_table(BRIEFS)?;
                let raw = table.get(id.as_str())?.ok_or(FoundryError::NotFound)?;
                let record = decode_row(id, raw.value())?;
                if record.workspace_id != *workspace {
                    return Err(FoundryError::CorruptStore(
                        "brief workspace mismatch".into(),
                    ));
                }
                let current = eligible(&record, &tx.open_table(SOURCES)?, &table)?;
                if !current && matches!(request.command, Command::Get { .. }) {
                    return Ok(
                        json!({"id":id,"revision":record.revision,"eligible":false,"status":"stale"}),
                    );
                }
                Ok(json!({"eligible":current,"record":record}))
            }
            Command::Forget {
                id,
                expected_revision,
            } => {
                if !is_digest(id) {
                    return Err(invalid("brief id must be a SHA-256 digest"));
                }
                let tx = self.db.begin_write()?;
                let result = {
                    let mut table = tx.open_table(BRIEFS)?;
                    let record = table
                        .get(id.as_str())?
                        .map(|r| decode_row(id, r.value()))
                        .transpose()?;
                    let Some(record) = record else {
                        return Ok(json!({"id":id,"outcome":"already_absent"}));
                    };
                    if record.revision != *expected_revision {
                        return Err(FoundryError::Conflict("brief revision changed".into()));
                    }
                    let mut meta = tx.open_table(META)?;
                    let next = store::read_counter(&meta, "brief_revision")?
                        .checked_add(1)
                        .ok_or(FoundryError::RevisionExhausted)?;
                    let mut forgotten = BTreeSet::from([id.clone()]);
                    for row in table.iter()? {
                        let (key, raw) = row?;
                        let ancestor = decode_row(key.value(), raw.value())?;
                        if ancestor
                            .derivation
                            .as_ref()
                            .is_some_and(|d| d.children.contains_key(id))
                        {
                            forgotten.insert(key.value().to_owned());
                        }
                    }
                    for removed in &forgotten {
                        table.remove(removed.as_str())?;
                        tx.open_table(PENDING)?
                            .insert(pending_key(removed).as_str(), next.to_string().as_str())?;
                    }
                    crate::knowledge::cache::forget(&tx, &forgotten)?;
                    crate::neural::work::invalidate(&tx, crate::neural::work::Lane::Knowledge)?;
                    meta.insert("brief_revision", next.to_string().as_str())?;
                    json!({"id":id,"revision":next,"outcome":"deleted"})
                };
                fault!(BRIEF_BEFORE_COMMIT, Some(self), Some(control), "")?;
                control.check()?;
                crate::neural::work::overview_changed(&tx)?;
                tx.commit()?;
                self.semantic_wake.notify();
                fault!(BRIEF_AFTER_COMMIT, Some(self), Some(control), "")?;
                Ok(result)
            }
            Command::Search {
                query,
                limit,
                max_bytes,
            } => {
                if !(1..=CONTEXT_HITS).contains(limit)
                    || !(1024..=MAX_REQUEST_BYTES).contains(max_bytes)
                {
                    return Err(invalid(
                        "brief search requires limit 1..8 and max_bytes 1024..262144",
                    ));
                }
                let plan = self.record_plan(query, "brief")?;
                fault!(BRIEF_BEFORE_FINAL_VALIDATION, Some(self), Some(control), "")?;
                let tx = self.db.begin_read()?;
                let (mut hits, stale, omitted) = self.brief_hits_in(&tx, &plan, *limit, control)?;
                let mut omitted = omitted;
                loop {
                    let result = json!({"kind":"derived","hits":hits,"stale_candidates":stale,
                        "candidates_full":plan.window_full,"omitted":omitted,
                        "pending_briefs":store::count_pending_prefix(&tx, "brief:")?,
                        "source_revision":self.freshness_in(&tx)?.source_revision});
                    if serde_json::to_vec(&result)?.len() < *max_bytes {
                        return Ok(result);
                    }
                    if hits.len() <= 1 {
                        return Err(invalid(
                            "brief search budget cannot fit a hit; increase max_bytes or use get/export by id",
                        ));
                    }
                    hits.pop();
                    omitted += 1;
                }
            }
        }
    }
    pub(crate) fn put_brief(
        &self,
        workspace: &str,
        brief: &Input,
        expected: Option<u64>,
        generation: Option<Generation<'_>>,
        control: &Control,
    ) -> FResult<Value> {
        let spans = brief.validate()?;
        let id = identity(workspace, &spans[0].path, &brief.recipe_sha256)?;
        // The hook is before the writer lock: simulate a source commit while a
        // generator was running without re-entrant transaction deadlock.
        fault!(BRIEF_BEFORE_ADMISSION, Some(self), Some(control), "")?;
        let tx = self.db.begin_write()?;
        let mut dependencies = BTreeMap::new();
        let mut bodies = BTreeMap::new();
        {
            let sources = tx.open_table(SOURCES)?;
            let chunks = tx.open_table(CHUNKS)?;
            for span in &spans {
                control.check()?;
                if !workspace.starts_with(&span.ws16) {
                    return Err(FoundryError::WrongWorkspace);
                }
                let raw = sources
                    .get(span.path.as_str())?
                    .ok_or(FoundryError::NotFound)?;
                let meta: SourceMeta = store::decode(raw.value(), "source")?;
                if !meta.hash.starts_with(&span.sha32) {
                    return Err(FoundryError::StaleHandle);
                }
                if !bodies.contains_key(&span.path) {
                    bodies.insert(
                        span.path.clone(),
                        store::reconstruct_verified(&chunks, &span.path, &meta)?.body,
                    );
                }
                let body = &bodies[&span.path];
                if span.start >= span.end
                    || body.get(span.start as usize..span.end as usize).is_none()
                {
                    return Err(FoundryError::InvalidRange);
                }
                dependencies.insert(span.path.clone(), meta.hash);
            }
        }
        for claim in brief.claims() {
            for identifier in &claim.identifiers {
                if !claim.citations.iter().any(|&i| {
                    let span = &spans[i];
                    bodies[&span.path][span.start as usize..span.end as usize].contains(identifier)
                }) {
                    return Err(invalid("declared identifier absent from its cited spans"));
                }
            }
        }
        let derivation = generation.as_ref().map(|g| g.derivation.clone());
        if let Some(lineage) = &derivation {
            lineage.validate()?;
            let table = tx.open_table(BRIEFS)?;
            let mut depth = 0;
            for (child_id, expected) in &lineage.children {
                if child_id == &id {
                    return Err(invalid("cyclic area lineage"));
                }
                let raw = table
                    .get(child_id.as_str())?
                    .ok_or(FoundryError::StaleHandle)?;
                let child = decode_row(child_id, raw.value())?;
                let child_lineage = child.derivation.as_ref().ok_or(FoundryError::StaleHandle)?;
                if crate::knowledge::fingerprint(&child)? != *expected
                    || child
                        .dependencies
                        .iter()
                        .any(|(path, sha)| dependencies.get(path) != Some(sha))
                    || child_lineage
                        .children
                        .iter()
                        .any(|(id, sha)| lineage.children.get(id) != Some(sha))
                    || !eligible(&child, &tx.open_table(SOURCES)?, &table)?
                {
                    return Err(FoundryError::StaleHandle);
                }
                depth = depth.max(child_lineage.depth + 1);
            }
            if lineage.depth != depth {
                return Err(invalid("area depth does not match its children"));
            }
        }
        let input_sha256 = input_identity(workspace, brief, &dependencies, derivation.as_ref())?;
        let result = {
            let mut table = tx.open_table(BRIEFS)?;
            let previous = table
                .get(id.as_str())?
                .map(|raw| decode_row(&id, raw.value()))
                .transpose()?;
            if let Some(previous) = &previous {
                if previous.input_sha256 == input_sha256 {
                    let mut replay = brief.clone();
                    replay.generated_at_unix_ms = previous.brief.generated_at_unix_ms;
                    if replay != previous.brief {
                        return Err(FoundryError::Conflict(
                            "same generator input has different output".into(),
                        ));
                    }
                    return Ok(json!({"id":id,"revision":previous.revision,"outcome":"unchanged"}));
                }
                if expected != Some(previous.revision) {
                    return Err(FoundryError::Conflict(
                        "brief replacement requires current revision".into(),
                    ));
                }
            } else if expected.is_some() {
                return Err(FoundryError::Conflict("brief no longer exists".into()));
            }
            let mut meta = tx.open_table(META)?;
            let next = store::read_counter(&meta, "brief_revision")?
                .checked_add(1)
                .ok_or(FoundryError::RevisionExhausted)?;
            let record = Record {
                kind: Kind::Derived,
                id: id.clone(),
                workspace_id: workspace.into(),
                revision: next,
                input_sha256,
                dependencies,
                brief: brief.clone(),
                derivation,
            };
            let encoded = serde_json::to_string(&record)?;
            if encoded.len() > MAX_RECORD_BYTES {
                return Err(invalid("brief record exceeds 512 KiB"));
            }
            if let Some(generation) = &generation {
                crate::knowledge::cache::put(
                    &tx,
                    &record,
                    generation.usage,
                    generation.cache_bytes,
                )?;
            }
            table.insert(id.as_str(), encoded.as_str())?;
            meta.insert("brief_revision", next.to_string().as_str())?;
            tx.open_table(PENDING)?
                .insert(pending_key(&id).as_str(), next.to_string().as_str())?;
            json!({"id":id,"revision":next,"outcome":if previous.is_some() {"updated"} else {"created"}})
        };
        fault!(BRIEF_BEFORE_COMMIT, Some(self), Some(control), "")?;
        control.check()?;
        crate::neural::work::overview_changed(&tx)?;
        tx.commit()?;
        self.semantic_wake.notify();
        fault!(BRIEF_AFTER_COMMIT, Some(self), Some(control), "")?;
        Ok(result)
    }
    pub(crate) fn brief_hits_in(
        &self,
        tx: &redb::ReadTransaction,
        plan: &store::RecordPlan,
        limit: usize,
        control: &Control,
    ) -> FResult<(Vec<Record>, u64, usize)> {
        let table = tx.open_table(BRIEFS)?;
        let sources = tx.open_table(SOURCES)?;
        let mut paths = BTreeSet::new();
        let mut hits = Vec::new();
        let mut stale = 0;
        let mut omitted = 0;
        for candidate in &plan.candidates {
            control.check()?;
            let Some(raw) = table.get(candidate.id.as_str())? else {
                stale += 1;
                continue;
            };
            let record = decode_row(&candidate.id, raw.value())?;
            if self.workspace_id().as_deref() != Some(&record.workspace_id) {
                return Err(FoundryError::CorruptStore(
                    "brief workspace mismatch".into(),
                ));
            }
            if candidate.revision != record.revision || !eligible(&record, &sources, &table)? {
                stale += 1;
                continue;
            }
            let path = HandleRef::parse(&record.brief.cited_spans[0])?.path;
            if !paths.insert(path) {
                continue;
            }
            if hits.len() < limit {
                hits.push(record);
            } else {
                omitted += 1;
            }
        }
        Ok((hits, stale, omitted))
    }
    /// Primary source evidence, reconstructed inside the final context snapshot.
    pub(crate) fn brief_source_in(
        &self,
        tx: &redb::ReadTransaction,
        record: &Record,
    ) -> FResult<store::RankedItem> {
        let span = HandleRef::parse(&record.brief.cited_spans[0])?;
        let sources = tx.open_table(SOURCES)?;
        let raw = sources
            .get(span.path.as_str())?
            .ok_or(FoundryError::NotFound)?;
        let meta: SourceMeta = store::decode(raw.value(), "source")?;
        let body = store::reconstruct_verified(&tx.open_table(CHUNKS)?, &span.path, &meta)?.body;
        let text = body
            .get(span.start as usize..span.end as usize)
            .ok_or_else(|| FoundryError::CorruptStore("brief span is invalid".into()))?;
        let start_line = 1 + body.as_bytes()[..span.start as usize]
            .iter()
            .filter(|&&b| b == b'\n')
            .count() as u64;
        let end_line = start_line
            + text.as_bytes()[..text.len().saturating_sub(1)]
                .iter()
                .filter(|&&b| b == b'\n')
                .count() as u64;
        Ok(store::RankedItem {
            tier: 2,
            rank: 0,
            score: 0.0,
            handle: Some(store::SourceHandle {
                workspace_id: record.workspace_id.clone(),
                path: span.path,
                sha256: meta.hash,
                start: span.start,
                end: span.end,
            }),
            start_line,
            end_line,
            line: start_line,
            label: "block".into(),
            lang: None,
            forms: vec![store::RenderedForm::Verbatim(text.into())],
            semantic: None,
            resolver: None,
        })
    }
}
