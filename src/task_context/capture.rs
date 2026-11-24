//! Plain source capture and workspace-scoped discovery use the existing task record.
use super::*;
use std::ops::Bound;

pub(super) fn excerpt(original: &str) -> Option<&str> {
    const PREVIEW: usize = 1024;
    if original.len() <= PREVIEW {
        return None;
    }
    let mut end = PREVIEW;
    while !original.is_char_boundary(end) {
        end -= 1;
    }
    Some(&original[..end])
}

impl Entry {
    fn capture_id(&self) -> String {
        let identity = serde_json::to_vec(&(
            &self.provenance,
            &self.author,
            &self.text,
            &self.supersedes,
            &self.source_links,
        ))
        .expect("string tuple is serializable");
        format!("capture-{}", crate::digest(&identity))
    }

    pub(crate) fn is_captured_source(&self) -> bool {
        self.kind == Kind::Observation
            && self.id.starts_with("capture-")
            && self.id == self.capture_id()
    }
}

impl StoredEntry {
    pub(super) fn has_source_excerpt(&self) -> bool {
        self.view.summary.is_some()
            && self.entry.is_captured_source()
            && self.view.summary.as_deref() == excerpt(&self.entry.text)
    }
}

impl Engine {
    pub(super) fn capture_original(&self, request: &Request) -> FResult<Option<Entry>> {
        let Command::Capture {
            source,
            text: supplied,
            lines,
            author,
            supersedes,
            ..
        } = &request.command
        else {
            return Ok(None);
        };
        text(source, 1024)?;
        let (original, links) = if let Some(supplied) = supplied {
            if lines.is_some() {
                return Err(invalid(
                    "lines selects indexed source only; omit it for supplied text",
                ));
            }
            (supplied.clone(), Vec::new())
        } else {
            let meta = self.source(source)?.ok_or(FoundryError::NotFound)?;
            let handle = crate::SourceHandle {
                workspace_id: request.workspace_id.clone(),
                path: source.clone(),
                sha256: meta.hash,
                start: 0,
                end: meta.bytes as u64,
            };
            let read = self.retrieve(&handle.to_v2(), lines.as_deref(), 1024)?;
            let body = String::from_utf8(read.span)
                .map_err(|_| FoundryError::CorruptStore("invalid indexed UTF-8".into()))?;
            (body, vec![read.requested.to_v2()])
        };
        let mut entry = Entry {
            id: String::new(),
            kind: Kind::Observation,
            text: original,
            author: author.as_deref().unwrap_or("unknown").to_owned(),
            provenance: source.clone(),
            supersedes: supersedes.clone(),
            source_links: links,
            assertions: Vec::new(),
            action_key: None,
        };
        entry.id = entry.capture_id();
        entry_valid(&entry)?;
        Ok(Some(entry))
    }

    pub(super) fn list_tasks(
        &self,
        workspace_id: &str,
        after_id: Option<&str>,
        control: &Control,
    ) -> FResult<Value> {
        const PAGE: usize = 8;
        if let Some(after) = after_id {
            id(after)?;
        }
        let tx = self.db.begin_read()?;
        let table = tx.open_table(TASKS)?;
        let start = after_id.map_or(Bound::Unbounded, Bound::Excluded);
        let mut rows = table.range::<&str>((start, Bound::Unbounded))?;
        let mut tasks = Vec::new();
        let mut last = None;
        for row in rows.by_ref().take(PAGE) {
            control.check()?;
            let (key, value) = row?;
            let task = decode(value.value(), key.value(), workspace_id)?;
            tasks.push(json!({"task_id":task.task_id,"title":task.title,"revision":task.revision}));
            last = Some(task.task_id);
        }
        let next = if rows.next().transpose()?.is_some() {
            last
        } else {
            None
        };
        Ok(json!({"workspace_id":workspace_id,"tasks":tasks,"next_after_id":next}))
    }
}
