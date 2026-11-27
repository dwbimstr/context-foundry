//! Explicit feature citations to originals saved once in a project notebook.
//! Review reads both ends at one store snapshot. Wording never creates an edge.
use super::*;
use std::ops::Bound;

const SUPPORTS_PER_CLAIM: usize = 4;
const CLAIMS_PER_PAGE: usize = 4;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CitationChoice {
    pub expected_revision: u64,
    pub entry_id: String,
    pub origin_task_id: String,
    pub origin_entry_id: String,
    pub capture_revision: u64,
    pub quote: String,
    #[serde(default)]
    pub offset: Option<usize>,
    pub author: String,
    #[serde(default)]
    pub wording: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(in crate::task_context) struct SourceCitation {
    entry_id: String,
    origin_task_id: String,
    root_entry_id: String,
    origin_entry_id: String,
    capture_revision: u64,
    offset: usize,
    end: usize,
    author: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    wording: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    withdraws: Option<u64>,
    revision: u64,
}

impl SourceCitation {
    fn key(&self) -> (&str, &str, &str) {
        (&self.entry_id, &self.origin_task_id, &self.root_entry_id)
    }
}

fn claim(entry: &StoredEntry) -> bool {
    entry.entry.kind.protected() || entry.entry.kind == Kind::Decision
}

impl Task {
    fn captured(&self, entry_id: &str) -> Option<&StoredEntry> {
        self.entries
            .iter()
            .find(|e| e.entry.id == entry_id && e.entry.is_captured_source())
    }

    // Follow only explicit corrections. Observations added through apply may
    // supersede a capture but cannot masquerade as a new captured original.
    fn capture_root<'a>(&'a self, mut item: &'a StoredEntry) -> &'a str {
        while let Some(parent) = item
            .entry
            .supersedes
            .as_deref()
            .and_then(|id| self.captured(id))
        {
            item = parent;
        }
        &item.entry.id
    }

    fn active_citations(&self) -> BTreeMap<(&str, &str, &str), &SourceCitation> {
        let mut active = BTreeMap::new();
        for citation in &self.source_citations {
            if citation.withdraws.is_some() {
                active.remove(&citation.key());
            } else {
                active.insert(citation.key(), citation);
            }
        }
        active
    }

    pub(in crate::task_context) fn validate_source_citations(&self) -> FResult<()> {
        if self.source_citations.len() > MAX_REVIEWS {
            return Err(invalid("feature citation history is full"));
        }
        for e in &self.entries {
            if e.capture_revision
                .is_some_and(|r| r == 0 || r > self.revision || !e.entry.is_captured_source())
            {
                return Err(invalid("invalid captured source revision"));
            }
        }
        let mut previous = 0;
        let mut active = BTreeMap::new();
        for link in &self.source_citations {
            id(&link.origin_task_id)?;
            id(&link.origin_entry_id)?;
            id(&link.root_entry_id)?;
            text(&link.author, 256)?;
            if let Some(words) = &link.wording {
                text(words, PASSAGE_BYTES)?;
            }
            if link.revision <= previous
                || link.revision > self.revision
                || link.end <= link.offset
                || link.end - link.offset > PASSAGE_BYTES
                || !self
                    .entries
                    .iter()
                    .any(|e| e.entry.id == link.entry_id && claim(e))
            {
                return Err(invalid("invalid recorded source citation"));
            }
            if let Some(prior) = link.withdraws {
                if active.remove(&link.key()) != Some(prior) {
                    return Err(invalid("withdrawal does not identify the active citation"));
                }
            } else {
                active.insert(link.key(), link.revision);
            }
            previous = link.revision;
        }
        Ok(())
    }

    pub(in crate::task_context) fn record_source_citation(
        &mut self,
        tx: &redb::ReadTransaction,
        cmd: &Command,
        revision: u64,
    ) -> FResult<Option<u64>> {
        let (link, expected) = match cmd {
            Command::CiteSource(pick) => {
                text(&pick.quote, PASSAGE_BYTES)?;
                id(&pick.origin_task_id)?;
                id(&pick.origin_entry_id)?;
                text(&pick.author, 256)?;
                if let Some(words) = &pick.wording {
                    text(words, PASSAGE_BYTES)?;
                }
                if !self
                    .entries
                    .iter()
                    .any(|e| e.entry.id == pick.entry_id && claim(e) && e.view.selected)
                    || self.superseded(&pick.entry_id)
                {
                    return Err(invalid(
                        "select a current requirement, constraint or decision",
                    ));
                }
                let table = tx.open_table(TASKS)?;
                let row = table
                    .get(pick.origin_task_id.as_str())?
                    .ok_or(FoundryError::NotFound)?;
                let origin = decode(row.value(), &pick.origin_task_id, &self.workspace_id)?;
                let item = origin
                    .captured(&pick.origin_entry_id)
                    .ok_or(FoundryError::NotFound)?;
                if item.capture_revision.unwrap_or(0) != pick.capture_revision
                    || origin.superseded(&item.entry.id)
                    || !item.view.selected
                    || source_links(tx, &item.entry)?
                        .iter()
                        .any(|v| v["status"] != "fresh")
                {
                    return Err(FoundryError::Conflict(
                        "original changed; reopen it before citing".into(),
                    ));
                }
                let body = &item.entry.text;
                let offset = if let Some(offset) = pick.offset {
                    if !body
                        .get(offset..)
                        .is_some_and(|s| s.starts_with(&pick.quote))
                    {
                        return Err(invalid("quote does not match the supplied byte offset"));
                    }
                    offset
                } else {
                    match locate_quote(body, &pick.quote) {
                        QuoteMatch::Unique(start) => start,
                        QuoteMatch::Absent => {
                            return Err(invalid("quote is absent from the original"));
                        }
                        QuoteMatch::Repeated => {
                            return Err(invalid("repeated quote requires its byte offset"));
                        }
                    }
                };
                let link = SourceCitation {
                    entry_id: pick.entry_id.clone(),
                    origin_task_id: origin.task_id.clone(),
                    root_entry_id: origin.capture_root(item).to_owned(),
                    origin_entry_id: item.entry.id.clone(),
                    capture_revision: pick.capture_revision,
                    offset,
                    end: offset + pick.quote.len(),
                    author: pick.author.clone(),
                    wording: pick.wording.clone(),
                    revision,
                    withdraws: None,
                };
                (link, pick.expected_revision)
            }
            Command::WithdrawCitation {
                expected_revision,
                citation_revision,
                author,
            } => {
                text(author, 256)?;
                let prior = self
                    .source_citations
                    .iter()
                    .find(|c| c.revision == *citation_revision && c.withdraws.is_none())
                    .ok_or(FoundryError::NotFound)?;
                let mut link = prior.clone();
                link.author = author.clone();
                link.withdraws = Some(*citation_revision);
                link.revision = revision;
                let latest = self
                    .source_citations
                    .iter()
                    .rev()
                    .find(|c| c.key() == link.key())
                    .expect("selected stored citation");
                if latest.revision != *citation_revision
                    && !(latest.withdraws == Some(*citation_revision) && latest.author == *author)
                {
                    return Err(FoundryError::Conflict("citation has been replaced".into()));
                }
                (link, *expected_revision)
            }
            _ => return Err(invalid("expected a feature citation operation")),
        };
        let active = self.active_citations();
        if let Some(prior) = self
            .source_citations
            .iter()
            .rev()
            .find(|c| c.key() == link.key())
        {
            let mut replay = link.clone();
            replay.revision = prior.revision;
            if &replay == prior {
                return Ok(Some(prior.revision));
            }
        }
        if expected != self.revision {
            return Err(FoundryError::Conflict(
                "feature revision changed; inspect and retry".into(),
            ));
        }
        if link.withdraws.is_none()
            && !active.contains_key(&link.key())
            && active
                .values()
                .filter(|c| c.entry_id == link.entry_id)
                .count()
                >= SUPPORTS_PER_CLAIM
        {
            return Err(invalid("a claim supports up to four independent originals"));
        }
        if self.source_citations.len() >= MAX_REVIEWS {
            return Err(invalid("feature citation history is full"));
        }
        self.source_citations.push(link);
        Ok(None)
    }
}

enum QuoteMatch {
    Absent,
    Unique(usize),
    Repeated,
}

// Both callers have a validated, nonempty quote. The first and last matches
// distinguish overlapping repetitions too (e.g. "aba" in "ababa").
fn locate_quote(body: &str, quote: &str) -> QuoteMatch {
    match body.find(quote) {
        None => QuoteMatch::Absent,
        Some(first) if body.rfind(quote) == Some(first) => QuoteMatch::Unique(first),
        Some(_) => QuoteMatch::Repeated,
    }
}

fn revised_excerpt<'a>(
    body: &'a str,
    quote: &str,
    previous_offset: Option<usize>,
) -> (usize, &'a str, Value) {
    let status = match locate_quote(body, quote) {
        QuoteMatch::Unique(at) => {
            let moved = previous_offset.map(|old| at != old);
            return (
                at,
                &body[at..at + quote.len()],
                json!({"status":"unique","moved":moved}),
            );
        }
        QuoteMatch::Absent => "absent",
        QuoteMatch::Repeated => "repeated",
    };
    let (at, preview) = excerpt_at(body, previous_offset.unwrap_or(0));
    (at, preview, json!({"status":status}))
}

fn excerpt_at(body: &str, offset: usize) -> (usize, &str) {
    let mut start = if offset < body.len() { offset } else { 0 };
    while !body.is_char_boundary(start) {
        start -= 1;
    }
    (start, prefix(&body[start..], PASSAGE_BYTES))
}

impl SourceCitation {
    fn evidence(
        &self,
        tx: &redb::ReadTransaction,
        workspace: &str,
        include_excerpt: bool,
    ) -> FResult<Value> {
        let mut v = json!({"origin_task_id":self.origin_task_id,"origin_entry_id":self.origin_entry_id,
            "capture_revision":self.capture_revision,"citation_revision":self.revision,"confirmed_by":self.author,
            "wording":self.wording,"status":"missing_original"});
        let table = tx.open_table(TASKS)?;
        let Some(row) = table.get(self.origin_task_id.as_str())? else {
            return Ok(v);
        };
        let origin = decode(row.value(), &self.origin_task_id, workspace)?;
        let Some(old) = origin.captured(&self.origin_entry_id) else {
            return Ok(v);
        };
        if old.capture_revision.unwrap_or(0) != self.capture_revision {
            v["status"] = json!("recreated_original");
            return Ok(v);
        }
        let quote = old
            .entry
            .text
            .get(self.offset..self.end)
            .ok_or_else(|| FoundryError::CorruptStore("citation outside original".into()))?;
        v["original"] = citation(
            &origin,
            old,
            self.offset,
            quote,
            source_links(tx, &old.entry)?,
        );
        let mut latest = old;
        while let Some(next) = origin
            .entries
            .iter()
            .find(|e| e.entry.supersedes.as_deref() == Some(&latest.entry.id))
        {
            if !next.entry.is_captured_source() {
                v["status"] = json!("superseded");
                return Ok(v);
            }
            latest = next;
        }
        if !latest.view.selected {
            v["status"] = json!("source_deselected");
            return Ok(v);
        }
        let links = source_links(tx, &latest.entry)?;
        let stale = links.iter().find(|l| l["status"] != "fresh");
        let status = match stale {
            Some(link) if link["status"] == "missing" => "indexed_source_missing",
            Some(_) => "indexed_source_changed",
            None if latest.entry.id != old.entry.id => "source_revised",
            None => {
                v["status"] = json!("current");
                return Ok(v);
            }
        };
        v["status"] = json!(status);
        if !include_excerpt {
            return Ok(v);
        }
        let (mut excerpt, comparison) = if let Some(link) = latest.entry.source_links.first() {
            let parsed = store::HandleRef::parse(link)?;
            let Some((meta, body)) = crate::boundary::source_in(tx, &parsed.path)? else {
                return Ok(v);
            };
            let previous_offset = old
                .entry
                .source_links
                .first()
                .map(|link| store::HandleRef::parse(link))
                .transpose()?
                .filter(|h| h.path == parsed.path)
                .map(|h| h.start as usize + self.offset);
            let (offset, passage, comparison) = revised_excerpt(&body, quote, previous_offset);
            let handle = crate::SourceHandle {
                workspace_id: workspace.into(),
                path: parsed.path,
                sha256: meta.hash,
                start: offset as u64,
                end: (offset + passage.len()) as u64,
            };
            // A fresh capture can also be reopened with peek if it contains
            // the returned span. Peek uses capture-relative byte offsets.
            let mut excerpt =
                if stale.is_none() && handle.start >= parsed.start && handle.end <= parsed.end {
                    citation(
                        &origin,
                        latest,
                        (handle.start - parsed.start) as usize,
                        passage,
                        links,
                    )
                } else {
                    json!({"offset":offset})
                };
            excerpt["handle"] = json!(handle.to_v2());
            excerpt["text"] = json!(passage);
            (excerpt, comparison)
        } else {
            let previous_offset = old.entry.source_links.is_empty().then_some(self.offset);
            let (offset, passage, comparison) =
                revised_excerpt(&latest.entry.text, quote, previous_offset);
            (
                citation(&origin, latest, offset, passage, links),
                comparison,
            )
        };
        excerpt["role"] = json!(if comparison["status"] == "unique" {
            "exact_quote"
        } else {
            "position_preview"
        });
        v["current_excerpt"] = excerpt;
        v["quote_match"] = comparison;
        Ok(v)
    }
}

/// Attach the status of this entry's own cited originals to a lookup result.
/// A named-feature lookup also shows one changed original for the option.
pub(super) fn mark_cited_originals(
    tx: &redb::ReadTransaction,
    workspace: &str,
    passage: &mut Value,
    include_changed_support: bool,
    control: &Control,
) -> FResult<()> {
    let table = tx.open_table(TASKS)?;
    let id = passage["task_id"].as_str().expect("constructed citation");
    let row = table.get(id)?.ok_or(FoundryError::NotFound)?;
    let task = decode(row.value(), id, workspace)?;
    let examples = if include_changed_support {
        SourceExamples::Changed
    } else {
        SourceExamples::StatusOnly
    };
    task.describe_cited_originals(tx, passage, examples, control)
        .map(|_| ())
}

pub(super) type CaptureLineages<'a> = BTreeSet<(&'a str, &'a str)>;

pub(super) enum SourceExamples<'a> {
    StatusOnly,
    Changed,
    DecisionAndLineages,
    CurrentForDependent(&'a CaptureLineages<'a>),
}

impl Task {
    pub(super) fn describe_cited_originals(
        &self,
        tx: &redb::ReadTransaction,
        passage: &mut Value,
        examples: SourceExamples<'_>,
        control: &Control,
    ) -> FResult<CaptureLineages<'_>> {
        let mut lineages = CaptureLineages::new();
        let entry_id = passage["entry_id"]
            .as_str()
            .expect("constructed citation")
            .to_owned();
        let mut status = None;
        let mut current = None;
        let mut separate_lineage = false;
        for link in self
            .active_citations()
            .into_values()
            .filter(|c| entry_id == c.entry_id)
        {
            control.check()?;
            let show_change = matches!(
                examples,
                SourceExamples::Changed | SourceExamples::DecisionAndLineages
            ) && passage.get("changed_support").is_none();
            let mut evidence = link.evidence(tx, &self.workspace_id, show_change)?;
            let lineage = (link.origin_task_id.as_str(), link.root_entry_id.as_str());
            // Missing and recreated captures have no verified original. A
            // later capture under reused IDs cannot inherit this identity.
            if matches!(examples, SourceExamples::DecisionAndLineages)
                && evidence.get("original").is_some()
            {
                lineages.insert(lineage);
            }
            if evidence["status"] != "current" {
                status = Some("needs_review");
                if show_change {
                    passage["changed_support"] = evidence;
                }
                if matches!(
                    examples,
                    SourceExamples::Changed | SourceExamples::StatusOnly
                ) {
                    break;
                }
                continue;
            }
            status.get_or_insert("current");
            if let SourceExamples::CurrentForDependent(owner) = examples {
                let shared = owner.contains(&lineage);
                if current.is_none() || (!shared && !separate_lineage) {
                    evidence["same_capture_lineage_as_decision"] = json!(shared);
                    current = Some(evidence);
                    separate_lineage = !shared;
                }
            }
        }
        if let Some(status) = status {
            passage["directly_cited_originals"] = json!(status);
        }
        if let Some(current) = current {
            passage["current_support"] = current;
        }
        Ok(lineages)
    }
}

/// Follow the feature's existing documented decision reference. A recreated
/// same-name decision has a different token and cannot inherit the old support.
fn decision_supports(
    tx: &redb::ReadTransaction,
    workspace: &str,
    reference: &DecisionRef,
    control: &Control,
) -> FResult<(Value, Vec<Value>)> {
    let mut via = json!({"decision_ref":reference,"status":"decision_changed"});
    let table = tx.open_table(TASKS)?;
    let Some(row) = table.get(reference.task_id.as_str())? else {
        return Ok((via, Vec::new()));
    };
    let owner = decode(row.value(), &reference.task_id, workspace)?;
    let Ok(decision) = owner.current_decision(&reference.entry_id) else {
        return Ok((via, Vec::new()));
    };
    if owner.decision_ref(decision)? != *reference {
        return Ok((via, Vec::new()));
    }
    let links = source_links(tx, &decision.entry)?;
    via["status"] = json!(if links.iter().all(|l| l["status"] == "fresh") {
        "current"
    } else {
        "decision_source_changed"
    });
    via["feature"] = json!(owner.title);
    via["decision"] = citation(
        &owner,
        decision,
        0,
        prefix(&decision.entry.text, PASSAGE_BYTES),
        links,
    );
    let mut supports = Vec::new();
    for link in owner
        .active_citations()
        .values()
        .filter(|c| c.entry_id == reference.entry_id)
    {
        control.check()?;
        let mut support = link.evidence(tx, workspace, true)?;
        support["via_decision"] = json!(reference);
        supports.push(support);
    }
    Ok((via, supports))
}

impl Engine {
    pub(in crate::task_context) fn source_review(
        &self,
        workspace: &str,
        query: &str,
        after: Option<&str>,
        control: &Control,
    ) -> FResult<Value> {
        text(query, QUERY_BYTES)?;
        let tx = self.db.begin_read()?;
        let meta = tx.open_table(META)?;
        let task_revision = store::read_counter(&meta, "task_revision")?;
        let source_revision = store::read_counter(&meta, "source_revision")?;
        let page_scope = format!(
            "{task_revision}.{source_revision}.{}",
            crate::digest(query.as_bytes())
        );
        let cursor = after
            .map(|v| -> FResult<_> {
                let (scope, position) = v
                    .split_once('/')
                    .ok_or_else(|| invalid("use the source_review cursor"))?;
                if scope != page_scope {
                    return Err(FoundryError::Conflict(
                        "question or workspace changed; restart source_review".into(),
                    ));
                }
                let (task, entry) = position
                    .split_once(':')
                    .ok_or_else(|| invalid("use the source_review cursor"))?;
                id(task)?;
                if !entry.is_empty() {
                    id(entry)?;
                }
                Ok((task, entry))
            })
            .transpose()?;
        let start = match cursor {
            None => Bound::Unbounded,
            Some((task, "")) => Bound::Excluded(task),
            Some((task, _)) => Bound::Included(task),
        };
        let table = tx.open_table(TASKS)?;
        let words = NoteWords::new(query);
        let mut claims = Vec::new();
        let mut next = None;
        let mut position = None;
        let mut examined = 0;
        for (index, row) in table.range::<&str>((start, Bound::Unbounded))?.enumerate() {
            control.check()?;
            if index == 8 {
                next = position;
                break;
            }
            let (key, raw) = row?;
            let feature = decode(raw.value(), key.value(), workspace)?;
            let active = feature.active_citations();
            let mut ordered: Vec<_> = feature
                .entries
                .iter()
                .filter(|e| claim(e) && e.view.selected && !feature.superseded(&e.entry.id))
                .collect();
            ordered.sort_by_key(|e| &e.entry.id);
            for entry in ordered {
                if cursor.is_some_and(|(task, last)| {
                    task == key.value() && entry.entry.id.as_str() <= last
                }) {
                    continue;
                }
                if examined == 16 || claims.len() == CLAIMS_PER_PAGE {
                    next = position;
                    break;
                }
                examined += 1;
                let mut supports = Vec::new();
                for link in active.values().filter(|c| c.entry_id == entry.entry.id) {
                    control.check()?;
                    supports.push(link.evidence(&tx, workspace, true)?);
                }
                let mut dependency = Value::Null;
                if let Some(reference) = &entry.entry.relies_on {
                    let (via, inherited) = decision_supports(&tx, workspace, reference, control)?;
                    dependency = via;
                    supports.extend(inherited);
                }
                let mut matching = [
                    feature.title.as_str(),
                    prefix(&entry.entry.text, QUERY_BYTES),
                ]
                .iter()
                .any(|s| words.best_passage(s).is_some())
                .then_some("wording_only");
                for support in &supports {
                    if let Some(quote) = support["original"]["quote"].as_str() {
                        let wording = support["wording"].as_str().unwrap_or_default();
                        if !wording.is_empty() && wording.trim().eq_ignore_ascii_case(query.trim())
                        {
                            matching = Some("previously_confirmed_wording");
                        } else if matching.is_none()
                            && [wording, quote]
                                .iter()
                                .any(|s| words.best_passage(s).is_some())
                        {
                            matching = Some("wording_only");
                        }
                    }
                }
                let links = source_links(&tx, &entry.entry)?;
                let changed = supports.iter().any(|s| s["status"] != "current")
                    || links.iter().any(|l| l["status"] != "fresh")
                    || (!dependency.is_null() && dependency["status"] != "current");
                if changed || matching.is_some() {
                    let why = if changed {
                        "needs_review"
                    } else if supports.is_empty() && dependency.is_null() && links.is_empty() {
                        "no_confirmed_source"
                    } else {
                        "current"
                    };
                    claims.push(json!({"feature":feature.title,"claim":citation(&feature,entry,0,prefix(&entry.entry.text,PASSAGE_BYTES),links),
                        "review_status":why,"question_match":matching,"supports":supports,"declared_dependency":dependency}));
                }
                position = Some(format!("{}:{}", key.value(), entry.entry.id));
            }
            if next.is_some() {
                break;
            }
            position = Some(format!("{}:", key.value()));
        }
        let clarification = if claims.iter().any(|c| c["question_match"] == "wording_only") {
            Some("Which quoted feature claim do you mean, or is the similar wording unrelated?")
        } else if claims.iter().any(|c| {
            c["question_match"] == "previously_confirmed_wording"
                && c["review_status"] == "needs_review"
        }) {
            Some("Does the feature still rely on the revised source for this claim?")
        } else {
            None
        };
        Ok(
            json!({"workspace_id":workspace,"scope":"workspace_review_queue_and_question_leads","claims":claims,
            "clarification":clarification,
            "task_revision":task_revision,"source_revision":source_revision,
            "next_after_id":next.map(|v|format!("{page_scope}/{v}"))}),
        )
    }
}
