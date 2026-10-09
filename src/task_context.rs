//! Explicit, experimental task state. Originals and reversible views share one
//! redb transaction. Task entries are not indexed or used for training. Explicit
//! promotion uses the existing memory index; no transcript capture or host rewriting.
use crate::store::{self, META, SOURCES};
use crate::{Control, Engine, FResult, FoundryError};
use redb::{ReadableDatabase, ReadableTable, ReadableTableMetadata, TableDefinition};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

pub(crate) const TASKS: TableDefinition<&str, &str> = TableDefinition::new("task_context");
pub const MAX_REQUEST_BYTES: usize = 1024 * 1024;
const MAX_RECORD_BYTES: usize = 2 * 1024 * 1024;

/// A primary-root task revision explicitly selected for source context.
#[derive(Clone, Copy)]
pub struct Scope<'a> {
    pub workspace_id: &'a str,
    pub task_id: &'a str,
    pub expected_revision: u64,
}

/// Complete mandatory view, validated in the source batch's final read.
#[derive(Clone, Debug)]
pub struct ContextView {
    pub(crate) line: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub workspace_id: String,
    pub task_id: String,
    pub command: Command,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
pub enum Command {
    Create {
        title: String,
    },
    Apply {
        expected_revision: u64,
        edits: Vec<Edit>,
    },
    Advise {
        action_key: String,
        max_bytes: usize,
    },
    Promote {
        expected_revision: u64,
        entry_id: String,
        memory_id: String,
        author: String,
        provenance: String,
    },
    Search {
        expected_revision: u64,
        query: String,
        limit: usize,
        max_bytes: usize,
        #[serde(default)]
        after_id: Option<String>,
        #[serde(default)]
        include_history: bool,
    },
    Inspect {},
    Peek {
        entry_id: String,
        offset: usize,
        bytes: usize,
    },
    Render {
        max_bytes: usize,
    },
    Export {},
    Forget {
        expected_revision: u64,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
pub enum Edit {
    Add { entry: Entry },
    Select { entry_id: String, selected: bool },
    Pin { entry_id: String, pinned: bool },
    Fold { entry_id: String, summary: String },
    Expand { entry_id: String },
    Dismiss { entry_id: String },
    Checkpoint { name: String },
    Restore { name: String },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    Requirement,
    Constraint,
    Observation,
    Hypothesis,
    Decision,
    RejectedAttempt,
    Question,
    Work,
    Artifact,
}
impl Kind {
    fn protected(self) -> bool {
        matches!(self, Self::Requirement | Self::Constraint)
    }
}

/// Caller assertions with attribution; neither an observation nor a decision is
/// automatically certified true. Supersession is explicit and preserves history.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Entry {
    pub id: String,
    pub kind: Kind,
    pub text: String,
    pub author: String,
    pub provenance: String,
    pub source_links: Vec<String>,
    #[serde(default)]
    pub supersedes: Option<String>,
    #[serde(default)]
    pub action_key: Option<String>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct View {
    selected: bool,
    pinned: bool,
    summary: Option<String>,
}
impl Default for View {
    fn default() -> Self {
        Self {
            selected: true,
            pinned: false,
            summary: None,
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct StoredEntry {
    entry: Entry,
    sha256: String,
    view: View,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Task {
    workspace_id: String,
    task_id: String,
    title: String,
    revision: u64,
    entries: Vec<StoredEntry>,
    checkpoints: BTreeMap<String, BTreeMap<String, View>>,
    dismissed: BTreeSet<String>,
    promotions: BTreeMap<String, Promotion>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Promotion {
    entry_id: String,
    memory_revision: u64,
}

fn invalid(s: &str) -> FoundryError {
    FoundryError::InvalidArgument(s.into())
}
fn id(value: &str) -> FResult<()> {
    if value.is_empty()
        || value.len() > 128
        || !value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
    {
        return Err(invalid("IDs must match [A-Za-z0-9_-]{1,128}"));
    }
    Ok(())
}
fn text(value: &str, max: usize) -> FResult<()> {
    if value.trim().is_empty() || value.len() > max {
        return Err(invalid("blank or oversized text field"));
    }
    Ok(())
}
fn entry_valid(entry: &Entry) -> FResult<()> {
    id(&entry.id)?;
    if let Some(key) = &entry.action_key {
        text(key, 256)?;
        if entry.kind != Kind::RejectedAttempt {
            return Err(invalid("action_key belongs only to a rejected attempt"));
        }
    }
    text(
        &entry.text,
        if entry.kind.protected() {
            16 * 1024
        } else {
            256 * 1024
        },
    )?;
    text(&entry.author, 256)?;
    text(&entry.provenance, 1024)?;
    if entry.source_links.len() > 8 || entry.source_links.iter().any(|s| s.len() > 4200) {
        return Err(invalid("at most eight bounded source handles per entry"));
    }
    for handle in &entry.source_links {
        store::HandleRef::parse(handle)?;
    }
    if let Some(parent) = &entry.supersedes {
        id(parent)?;
    }
    Ok(())
}
impl Task {
    fn superseded(&self, id: &str) -> bool {
        self.entries
            .iter()
            .any(|e| e.entry.supersedes.as_deref() == Some(id))
    }
    fn entry_mut(&mut self, id: &str) -> FResult<&mut StoredEntry> {
        if self.superseded(id) {
            return Err(invalid("superseded entries are historical; use peek"));
        }
        self.entries
            .iter_mut()
            .find(|e| e.entry.id == id)
            .ok_or(FoundryError::NotFound)
    }
    fn valid(&self) -> FResult<()> {
        id(&self.task_id)?;
        if self.workspace_id.len() != 64
            || !self.workspace_id.bytes().all(|b| b.is_ascii_hexdigit())
        {
            return Err(invalid("invalid task workspace identity"));
        }
        text(&self.title, 1024)?;
        if self.revision == 0 || self.entries.len() > 128 || self.checkpoints.len() > 16 {
            return Err(invalid("invalid task revision or capacity"));
        }
        let mut seen: BTreeMap<&str, Kind> = BTreeMap::new();
        let mut superseded = std::collections::BTreeSet::new();
        for item in &self.entries {
            let e = &item.entry;
            entry_valid(e)?;
            for handle in &e.source_links {
                if store::HandleRef::parse(handle)?.ws16 != self.workspace_id[..16] {
                    return Err(invalid("task source link names another workspace"));
                }
            }
            if seen.contains_key(e.id.as_str()) || item.sha256 != crate::digest(e.text.as_bytes()) {
                return Err(invalid("duplicate entry or damaged payload"));
            }
            if let Some(parent) = &e.supersedes
                && (seen.get(parent.as_str()) != Some(&e.kind)
                    || !superseded.insert(parent.as_str()))
            {
                return Err(invalid(
                    "supersession must name an earlier, unsuperseded entry of the same kind",
                ));
            }
            seen.insert(&e.id, e.kind);
            validate_view(&item.view)?;
            if e.kind.protected()
                && (!item.view.selected || !item.view.pinned || item.view.summary.is_some())
            {
                return Err(invalid(
                    "requirements and constraints must stay selected, pinned and expanded",
                ));
            }
        }
        for entry_id in &self.dismissed {
            if seen.get(entry_id.as_str()) != Some(&Kind::RejectedAttempt) {
                return Err(invalid("dismissal names an unknown rejected attempt"));
            }
        }
        let mut promoted_entries = BTreeSet::new();
        for (memory_id, promotion) in &self.promotions {
            id(memory_id)?;
            if promotion.memory_revision == 0
                || seen.get(promotion.entry_id.as_str()) != Some(&Kind::Decision)
                || !promoted_entries.insert(&promotion.entry_id)
            {
                return Err(invalid("invalid or duplicate promotion lineage"));
            }
        }
        for (name, checkpoint) in &self.checkpoints {
            id(name)?;
            for (entry_id, view) in checkpoint {
                if !seen.contains_key(entry_id.as_str()) {
                    return Err(invalid("checkpoint names unknown entry"));
                }
                validate_view(view)?;
                if seen[entry_id.as_str()].protected()
                    && (!view.selected || !view.pinned || view.summary.is_some())
                {
                    return Err(invalid("checkpoint hides a protected entry"));
                }
            }
        }
        Ok(())
    }
    fn edit(&mut self, edit: &Edit) -> FResult<()> {
        match edit {
            Edit::Add { entry } => {
                entry_valid(entry)?;
                if self.entries.len() == 128 {
                    return Err(invalid("task has 128 entries"));
                }
                if self.entries.iter().any(|e| e.entry.id == entry.id) {
                    return Err(FoundryError::Conflict("entry ID already exists".into()));
                }
                if let Some(parent) = &entry.supersedes {
                    let old = self.entry_mut(parent)?;
                    if old.entry.kind != entry.kind {
                        return Err(invalid("supersession kind differs"));
                    }
                }
                self.entries.push(StoredEntry {
                    entry: entry.clone(),
                    sha256: crate::digest(entry.text.as_bytes()),
                    view: View {
                        pinned: entry.kind.protected(),
                        ..View::default()
                    },
                });
            }
            Edit::Dismiss { entry_id } => {
                if self.entry_mut(entry_id)?.entry.kind != Kind::RejectedAttempt {
                    return Err(invalid("only rejected-attempt advice can be dismissed"));
                }
                self.dismissed.insert(entry_id.clone());
            }
            Edit::Select { entry_id, selected } => {
                self.entry_mut(entry_id)?.view.selected = *selected;
            }
            Edit::Pin { entry_id, pinned } => {
                let item = self.entry_mut(entry_id)?;
                item.view.pinned = *pinned;
                if *pinned {
                    item.view.selected = true;
                }
            }
            Edit::Fold { entry_id, summary } => {
                text(summary, 2048)?;
                self.entry_mut(entry_id)?.view.summary = Some(summary.clone());
            }
            Edit::Expand { entry_id } => {
                self.entry_mut(entry_id)?.view.summary = None;
            }
            Edit::Checkpoint { name } => {
                id(name)?;
                if self.checkpoints.contains_key(name) {
                    return Err(FoundryError::Conflict("checkpoint already exists".into()));
                }
                if self.checkpoints.len() == 16 {
                    return Err(invalid("task has 16 checkpoints"));
                }
                self.checkpoints.insert(
                    name.clone(),
                    self.entries
                        .iter()
                        .map(|e| (e.entry.id.clone(), e.view.clone()))
                        .collect(),
                );
            }
            Edit::Restore { name } => {
                let checkpoint = self.checkpoints.get(name).ok_or(FoundryError::NotFound)?;
                for item in &mut self.entries {
                    // New evidence survives an old checkpoint. In particular, new
                    // constraints remain protected and supersession remains current.
                    if let Some(view) = checkpoint.get(&item.entry.id) {
                        item.view = view.clone();
                    }
                }
            }
        }
        self.valid()
    }
}
fn validate_view(view: &View) -> FResult<()> {
    if view.pinned && !view.selected {
        return Err(invalid("a pinned entry cannot be deselected"));
    }
    if let Some(summary) = &view.summary {
        text(summary, 2048)?;
    }
    Ok(())
}
fn decode(raw: &str, task_id: &str, workspace_id: &str) -> FResult<Task> {
    if raw.len() > MAX_RECORD_BYTES {
        return Err(FoundryError::CorruptStore(
            "task_context record exceeds its byte limit".into(),
        ));
    }
    let task: Task = serde_json::from_str(raw)
        .map_err(|_| FoundryError::CorruptStore("task_context record cannot be decoded".into()))?;
    if task.task_id != task_id || task.workspace_id != workspace_id || task.valid().is_err() {
        return Err(FoundryError::CorruptStore(
            "task_context record failed validation".into(),
        ));
    }
    Ok(task)
}
pub fn parse(value: Value) -> FResult<Request> {
    if serde_json::to_vec(&value)?.len() > MAX_REQUEST_BYTES {
        return Err(invalid("task request exceeds 1 MiB"));
    }
    serde_json::from_value(value).map_err(|_| invalid("invalid task request fields"))
}

fn source_links(tx: &redb::ReadTransaction, entry: &Entry) -> FResult<Vec<Value>> {
    let sources = tx.open_table(SOURCES)?;
    entry
        .source_links
        .iter()
        .map(|handle| {
            let parsed = store::HandleRef::parse(handle)?;
            let status = match sources.get(parsed.path.as_str())? {
                None => "missing",
                Some(raw) => {
                    let source: store::SourceMeta =
                        serde_json::from_str(raw.value()).map_err(|_| {
                            FoundryError::CorruptStore("linked source cannot be decoded".into())
                        })?;
                    if source.hash.starts_with(&parsed.sha32) {
                        "fresh"
                    } else {
                        "stale"
                    }
                }
            };
            Ok(json!({"handle":handle,"status":status}))
        })
        .collect()
}
fn render_view(tx: &redb::ReadTransaction, task: &Task) -> FResult<Value> {
    let entries = task.entries.iter().filter(|e| e.view.selected && !task.superseded(&e.entry.id)).map(|e| -> FResult<Value> {
        Ok(json!({"id":e.entry.id,"kind":e.entry.kind,"author":e.entry.author,"provenance":e.entry.provenance,"sha256":e.sha256,"representation":if e.view.summary.is_some() { "caller_summary" } else { "original" },"text":e.view.summary.as_ref().unwrap_or(&e.entry.text),"source_links":source_links(tx,&e.entry)?}))
    }).collect::<FResult<Vec<_>>>()?;
    Ok(
        json!({"workspace_id":task.workspace_id,"task_id":task.task_id,"revision":task.revision,"boundary":"foundry_task_context","host_application":"unknown","title":task.title,"entries":entries}),
    )
}

impl Engine {
    fn task_in(&self, tx: &redb::ReadTransaction, scope: Scope<'_>) -> FResult<Task> {
        id(scope.task_id)?;
        if self.workspace_id().ok_or(FoundryError::WorkspaceUnbound)? != scope.workspace_id {
            return Err(FoundryError::WrongWorkspace);
        }
        let table = tx.open_table(TASKS)?;
        let raw = table.get(scope.task_id)?.ok_or(FoundryError::NotFound)?;
        let task = decode(raw.value(), scope.task_id, scope.workspace_id)?;
        if task.revision != scope.expected_revision {
            return Err(FoundryError::Conflict(
                "task revision changed; inspect before requesting context".into(),
            ));
        }
        Ok(task)
    }
    /// Preflight before optional embedding/route work. Final-read validation
    /// repeats it; this early check cannot authorize a later changed view.
    pub(crate) fn check_task_scope(&self, scope: Scope<'_>) -> FResult<()> {
        self.task_in(&self.db.begin_read()?, scope).map(|_| ())
    }
    pub(crate) fn task_view_in(
        &self,
        tx: &redb::ReadTransaction,
        scope: Scope<'_>,
    ) -> FResult<ContextView> {
        let task = self.task_in(tx, scope)?;
        let view = render_view(tx, &task)?;
        let sha256 = crate::digest(&serde_json::to_vec(&view)?);
        let line = format!(
            "task-view: {}\n",
            serde_json::to_string(&json!({"sha256":sha256,"view":view}))?
        );
        Ok(ContextView { line })
    }
}

impl Engine {
    /// One bounded, explicitly scoped operation. Mutations compare the revision
    /// inside the same transaction that commits original bytes and the working view.
    pub fn task_context(&self, request: &Request, control: &Control) -> FResult<Value> {
        control.check()?;
        if self.workspace_id().ok_or(FoundryError::WorkspaceUnbound)? != request.workspace_id {
            return Err(FoundryError::WrongWorkspace);
        }
        id(&request.task_id)?;
        if serde_json::to_vec(request)?.len() > MAX_REQUEST_BYTES {
            return Err(invalid("task request exceeds 1 MiB"));
        }
        match &request.command {
            Command::Create { .. }
            | Command::Apply { .. }
            | Command::Forget { .. }
            | Command::Promote { .. } => self.task_mutate(request, control),
            _ => {
                let tx = self.db.begin_read()?;
                let table = tx.open_table(TASKS)?;
                let raw = table
                    .get(request.task_id.as_str())?
                    .ok_or(FoundryError::NotFound)?;
                let task = decode(raw.value(), &request.task_id, &request.workspace_id)?;
                let links = |entry: &Entry| source_links(&tx, entry);
                let mut result = json!({"workspace_id":task.workspace_id,"task_id":task.task_id,"revision":task.revision,"boundary":"foundry_task_context","host_application":"unknown"});
                match &request.command {
                    Command::Search {
                        expected_revision,
                        query,
                        limit,
                        max_bytes,
                        after_id,
                        include_history,
                    } => {
                        if task.revision != *expected_revision {
                            return Err(FoundryError::Conflict(
                                "task revision changed; inspect before continuing search".into(),
                            ));
                        }
                        text(query, 256)?;
                        if !(1..=32).contains(limit) || !(512..=65536).contains(max_bytes) {
                            return Err(invalid(
                                "search limit must be 1..32 and max_bytes 512..65536",
                            ));
                        }
                        if let Some(after) = after_id {
                            id(after)?;
                        }
                        let needle = query.to_ascii_lowercase();
                        let mut matches = Vec::new();
                        for item in &task.entries {
                            control.check()?;
                            if !*include_history && task.superseded(&item.entry.id) {
                                continue;
                            }
                            if let Some(at) = item.entry.text.to_ascii_lowercase().find(&needle) {
                                matches.push((item, at));
                            }
                        }
                        matches.sort_by_key(|(item, _)| &item.entry.id);
                        result["matching_entries"] = json!(matches.len());
                        result["hits"] = json!([]);
                        result["next_after_id"] = Value::Null;
                        let eligible: Vec<_> = matches
                            .into_iter()
                            .filter(|(item, _)| {
                                after_id.as_ref().is_none_or(|after| item.entry.id > *after)
                            })
                            .collect();
                        for (index, (item, at)) in eligible.iter().enumerate() {
                            if index == *limit {
                                break;
                            }
                            let body = &item.entry.text;
                            let mut start = at.saturating_sub(80);
                            while !body.is_char_boundary(start) {
                                start += 1;
                            }
                            let mut end = (at + query.len() + 160).min(body.len());
                            while !body.is_char_boundary(end) {
                                end -= 1;
                            }
                            let hit = json!({"entry_id":item.entry.id,"kind":item.entry.kind,"sha256":item.sha256,"author":item.entry.author,"provenance":item.entry.provenance,"historical":task.superseded(&item.entry.id),"source_links":links(&item.entry)?,"offset":start,"end":end,"match_start":at,"match_end":at+query.len(),"excerpt":&body[start..end]});
                            let mut candidate = result.clone();
                            candidate["hits"]
                                .as_array_mut()
                                .expect("hits is an array")
                                .push(hit);
                            candidate["next_after_id"] = if index + 1 < eligible.len() {
                                json!(item.entry.id)
                            } else {
                                Value::Null
                            };
                            if serde_json::to_vec(&candidate)?.len() + 1 > *max_bytes {
                                if index == 0 {
                                    return Err(invalid(
                                        "search hit cannot fit; increase max_bytes",
                                    ));
                                }
                                break;
                            }
                            result = candidate;
                        }
                        if serde_json::to_vec(&result)?.len() + 1 > *max_bytes {
                            return Err(invalid("search envelope exceeds max_bytes"));
                        }
                    }
                    Command::Advise {
                        action_key,
                        max_bytes,
                    } => {
                        text(action_key, 256)?;
                        if !(512..=65536).contains(max_bytes) {
                            return Err(invalid("advice max_bytes must be 512..65536"));
                        }
                        result["silence"] = json!(true);
                        result["advice"] = Value::Null;
                        for item in &task.entries {
                            if item.entry.action_key.as_ref() != Some(action_key)
                                || task.superseded(&item.entry.id)
                                || task.dismissed.contains(&item.entry.id)
                            {
                                continue;
                            }
                            let source_links = links(&item.entry)?;
                            if source_links.iter().any(|link| link["status"] != "fresh") {
                                continue;
                            }
                            let mut candidate = result.clone();
                            candidate["silence"] = json!(false);
                            candidate["advice"] = json!({"trigger":"repeat_failed_action","entry_id":item.entry.id,"sha256":item.sha256,"text":item.entry.text,"author":item.entry.author,"provenance":item.entry.provenance,"source_links":source_links});
                            if serde_json::to_vec(&candidate)?.len() < *max_bytes {
                                result = candidate;
                                break;
                            }
                        }
                        if serde_json::to_vec(&result)?.len() + 1 > *max_bytes {
                            return Err(invalid("advice envelope exceeds byte budget"));
                        }
                    }
                    Command::Inspect {} => {
                        result["dismissed"] = json!(task.dismissed);
                        result["promotions"] = json!(task.promotions);
                        result["title"] = json!(task.title);
                        result["checkpoints"] = json!(task.checkpoints.keys().collect::<Vec<_>>());
                        result["entries"] = Value::Array(task.entries.iter().map(|e| -> FResult<Value> { Ok(json!({"id":e.entry.id,"kind":e.entry.kind,"author":e.entry.author,"provenance":e.entry.provenance,"bytes":e.entry.text.len(),"sha256":e.sha256,"source_links":links(&e.entry)?,"supersedes":e.entry.supersedes,"action_key":e.entry.action_key,"historical":task.superseded(&e.entry.id),"view":e.view})) }).collect::<FResult<_>>()?);
                    }
                    Command::Peek {
                        entry_id,
                        offset,
                        bytes,
                    } => {
                        if !(4..=65536).contains(bytes) {
                            return Err(invalid("peek bytes must be 4..65536"));
                        }
                        let item = task
                            .entries
                            .iter()
                            .find(|e| &e.entry.id == entry_id)
                            .ok_or(FoundryError::NotFound)?;
                        let body = &item.entry.text;
                        if *offset > body.len() || !body.is_char_boundary(*offset) {
                            return Err(invalid(
                                "peek offset must be a UTF-8 boundary inside the payload",
                            ));
                        }
                        let mut end = offset.saturating_add(*bytes).min(body.len());
                        while !body.is_char_boundary(end) {
                            end -= 1;
                        }
                        result["entry_id"] = json!(entry_id);
                        result["sha256"] = json!(item.sha256);
                        result["offset"] = json!(offset);
                        result["next_offset"] = if end == body.len() {
                            Value::Null
                        } else {
                            json!(end)
                        };
                        result["text"] = json!(&body[*offset..end]);
                        result["source_links"] = json!(links(&item.entry)?);
                        result["historical"] = json!(task.superseded(entry_id));
                    }
                    Command::Render { max_bytes } => {
                        if !(256..=MAX_REQUEST_BYTES).contains(max_bytes) {
                            return Err(invalid("render max_bytes must be 256..1048576"));
                        }
                        result = render_view(&tx, &task)?;
                        if serde_json::to_vec(&result)?.len() + 1 > *max_bytes {
                            return Err(invalid(
                                "selected view exceeds byte budget; explicitly fold or deselect unprotected entries",
                            ));
                        }
                    }
                    Command::Export {} => {
                        result["record"] = serde_json::to_value(&task)?;
                    }
                    _ => unreachable!(),
                }
                control.check()?;
                Ok(result)
            }
        }
    }
    pub(crate) fn drain_task_memory(&mut self, result: &Value) {
        if let Some(id) = result["memory_id"].as_str() {
            self.drain_memory_key(id);
        }
        if let Some(ids) = result["forgotten_memory_ids"].as_array() {
            for id in ids.iter().filter_map(Value::as_str) {
                self.drain_memory_key(id);
            }
        }
    }
    fn task_mutate(&self, request: &Request, control: &Control) -> FResult<Value> {
        let tx = self.db.begin_write()?;
        let mut result;
        {
            let mut table = tx.open_table(TASKS)?;
            let mut meta = tx.open_table(META)?;
            let previous = table
                .get(request.task_id.as_str())?
                .map(|v| decode(v.value(), &request.task_id, &request.workspace_id))
                .transpose()?;
            let mut task = match (&request.command, previous) {
                (Command::Create { title }, None) => {
                    text(title, 1024)?;
                    if table.len()? >= 128 {
                        return Err(invalid(
                            "store has 128 live tasks; export/forget explicitly",
                        ));
                    }
                    Task {
                        task_id: request.task_id.clone(),
                        workspace_id: request.workspace_id.clone(),
                        title: title.clone(),
                        revision: 1,
                        entries: Vec::new(),
                        checkpoints: BTreeMap::new(),
                        dismissed: BTreeSet::new(),
                        promotions: BTreeMap::new(),
                    }
                }
                (Command::Create { title }, Some(task)) => {
                    if &task.title == title {
                        return Ok(
                            json!({"task_id":task.task_id,"revision":task.revision,"outcome":"existing"}),
                        );
                    }
                    return Err(FoundryError::Conflict(
                        "task already exists with a different title".into(),
                    ));
                }
                (
                    Command::Apply {
                        expected_revision, ..
                    }
                    | Command::Forget { expected_revision }
                    | Command::Promote {
                        expected_revision, ..
                    },
                    Some(task),
                ) => {
                    if task.revision != *expected_revision {
                        return Err(FoundryError::Conflict(
                            "task revision changed; inspect and retry".into(),
                        ));
                    }
                    task
                }
                (_, None) => return Err(FoundryError::NotFound),
                _ => unreachable!(),
            };
            if let Command::Apply { edits, .. } = &request.command {
                if edits.is_empty() || edits.len() > 32 {
                    return Err(invalid("apply requires 1..32 edits"));
                }
                for edit in edits {
                    control.check()?;
                    if let Edit::Add { entry } = edit {
                        if let Some(parent) = &entry.supersedes {
                            let memory = tx.open_table(crate::memory::MEMORY)?;
                            for (memory_id, promotion) in &task.promotions {
                                if &promotion.entry_id == parent
                                    && memory.get(memory_id.as_str())?.is_some()
                                {
                                    return Err(FoundryError::Conflict(format!(
                                        "explicitly forget promoted memory {memory_id} before superseding its task decision"
                                    )));
                                }
                            }
                        }
                        for handle in &entry.source_links {
                            store::validate_link_span(&tx, &request.workspace_id, handle)?;
                        }
                    }
                    task.edit(edit)?;
                }
            }
            let mut promoted = None;
            if let Command::Promote {
                entry_id,
                memory_id,
                author,
                provenance,
                ..
            } = &request.command
            {
                id(memory_id)?;
                text(author, 256)?;
                text(provenance, 512)?;
                if task.promotions.contains_key(memory_id) {
                    return Err(FoundryError::Conflict(
                        "memory ID already appears in this task's promotion lineage".into(),
                    ));
                }
                if task.superseded(entry_id)
                    || task.promotions.values().any(|p| &p.entry_id == entry_id)
                {
                    return Err(invalid("decision is historical or already promoted"));
                }
                let item = task
                    .entries
                    .iter()
                    .find(|e| &e.entry.id == entry_id)
                    .ok_or(FoundryError::NotFound)?;
                if item.entry.kind != Kind::Decision {
                    return Err(invalid("only an explicit decision can be promoted"));
                }
                let fields = crate::memory::RecordFields {
                    id: memory_id.clone(),
                    text: item.entry.text.clone(),
                    author: author.clone(),
                    provenance: format!(
                        "task:{}/entry:{}@{}; {}",
                        task.task_id, entry_id, item.sha256, provenance
                    ),
                    source_links: item.entry.source_links.clone(),
                };
                Self::validate_record_fields(&fields)?;
                let mut memory = tx.open_table(crate::memory::MEMORY)?;
                if memory.get(memory_id.as_str())?.is_some() {
                    return Err(FoundryError::Conflict("memory ID already exists".into()));
                }
                for handle in &fields.source_links {
                    store::validate_link_span(&tx, &request.workspace_id, handle)?;
                }
                let next = store::read_counter(&meta, "memory_revision")?
                    .checked_add(1)
                    .ok_or(FoundryError::RevisionExhausted)?;
                let record = crate::memory::MemoryRecord {
                    id: fields.id,
                    workspace_id: request.workspace_id.clone(),
                    revision: next,
                    text: fields.text,
                    author: fields.author,
                    provenance: fields.provenance,
                    source_links: fields.source_links,
                };
                memory.insert(memory_id.as_str(), serde_json::to_string(&record)?.as_str())?;
                meta.insert("memory_revision", next.to_string().as_str())?;
                tx.open_table(store::PENDING)?.insert(
                    crate::memory::memory_pending_key(memory_id).as_str(),
                    next.to_string().as_str(),
                )?;
                task.promotions.insert(
                    memory_id.clone(),
                    Promotion {
                        entry_id: entry_id.clone(),
                        memory_revision: next,
                    },
                );
                promoted = Some((memory_id, next));
            }
            let revision = store::read_counter(&meta, "task_revision")?
                .checked_add(1)
                .ok_or(FoundryError::RevisionExhausted)?;
            if matches!(request.command, Command::Forget { .. }) {
                let mut removed = Vec::new();
                let mut memory = tx.open_table(crate::memory::MEMORY)?;
                for (memory_id, promotion) in &task.promotions {
                    let current = memory
                        .get(memory_id.as_str())?
                        .map(|v| crate::memory::decode_row(memory_id, v.value()))
                        .transpose()?;
                    if let Some(record) = current {
                        if record.revision != promotion.memory_revision {
                            return Err(FoundryError::Conflict(format!(
                                "promoted memory {memory_id} was edited or recreated; resolve/forget it explicitly first"
                            )));
                        }
                        memory.remove(memory_id.as_str())?;
                        tx.open_table(store::PENDING)?.insert(
                            crate::memory::memory_pending_key(memory_id).as_str(),
                            "deleted",
                        )?;
                        removed.push(memory_id.clone());
                    }
                }
                table.remove(request.task_id.as_str())?;
                result = json!({"task_id":request.task_id,"revision":revision,"outcome":"deleted","forgotten_memory_ids":removed});
            } else {
                task.revision = revision;
                task.valid()?;
                let encoded = serde_json::to_string(&task)?;
                if encoded.len() > MAX_RECORD_BYTES {
                    return Err(invalid("task exceeds 2 MiB; export/forget explicitly"));
                }
                table.insert(request.task_id.as_str(), encoded.as_str())?;
                result =
                    json!({"task_id":request.task_id,"revision":revision,"outcome":"committed"});
            }
            if let Some((memory_id, memory_revision)) = promoted {
                result["memory_id"] = json!(memory_id);
                result["memory_revision"] = json!(memory_revision);
            }
            meta.insert("task_revision", revision.to_string().as_str())?;
        }
        fault!(
            TASK_BEFORE_COMMIT,
            Some(self),
            Some(control),
            &request.task_id
        )?;
        control.check()?;
        tx.commit()?;
        fault!(
            TASK_AFTER_COMMIT,
            Some(self),
            Some(control),
            &request.task_id
        )?;
        Ok(result)
    }
}
