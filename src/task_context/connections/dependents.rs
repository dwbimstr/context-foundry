//! A feature owns its declaration that a requirement relies on another decision.
//! Reading those declarations needs no reverse index or cross-task write.
use super::*;
use std::ops::Bound;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DecisionRef {
    pub task_id: String,
    pub entry_id: String,
    pub token: String,
}

impl DecisionRef {
    pub(in crate::task_context) fn validate(&self, kind: Kind) -> FResult<()> {
        id(&self.task_id)?;
        id(&self.entry_id)?;
        if !kind.protected()
            || self.token.len() != 64
            || !self
                .token
                .bytes()
                .all(|b| b.is_ascii_digit() || matches!(b, b'a'..=b'f'))
        {
            return Err(invalid(
                "relies_on requires a requirement or constraint and a decision_ref token",
            ));
        }
        Ok(())
    }
}

/// An indexed rule can be adopted independently by two features. A link
/// to the same file or a copied sentence without that source is insufficient.
fn exact_rule_sources<'a>(
    tx: &redb::ReadTransaction,
    decision: &'a Entry,
) -> FResult<BTreeSet<&'a str>> {
    let mut exact = BTreeSet::new();
    for link in &decision.source_links {
        let h = store::HandleRef::parse(link)?;
        if h.end - h.start == decision.text.len() as u64
            && let Some((meta, body)) = crate::boundary::source_in(tx, &h.path)?
            && meta.hash.starts_with(&h.sha32)
            && body.get(h.start as usize..h.end as usize) == Some(decision.text.as_str())
        {
            exact.insert(link.as_str());
        }
    }
    Ok(exact)
}

impl Task {
    pub(in crate::task_context) fn current_decision(
        &self,
        entry_id: &str,
    ) -> FResult<&StoredEntry> {
        id(entry_id)?;
        let item = self
            .entries
            .iter()
            .find(|e| e.entry.id == entry_id)
            .ok_or(FoundryError::NotFound)?;
        if item.entry.kind != Kind::Decision || !item.view.selected || self.superseded(entry_id) {
            return Err(invalid(
                "select a current decision in the originating feature",
            ));
        }
        Ok(item)
    }

    pub(in crate::task_context) fn decision_ref(&self, item: &StoredEntry) -> FResult<DecisionRef> {
        // The workspace counter distinguishes a decision recreated under the
        // same IDs after forget, even when its words and attribution are identical.
        // Decisions from before this field existed retain their legacy identity.
        let identity = serde_json::to_vec(&(
            &self.workspace_id,
            &self.task_id,
            item.decision_revision,
            &item.entry,
        ))?;
        Ok(DecisionRef {
            task_id: self.task_id.clone(),
            entry_id: item.entry.id.clone(),
            token: crate::digest(&identity),
        })
    }

    pub(in crate::task_context) fn decision_uses(
        &self,
        tx: &redb::ReadTransaction,
        entry_id: &str,
        after: Option<&str>,
        control: &Control,
    ) -> FResult<Value> {
        let decision = self.current_decision(entry_id)?;
        let links = source_links(tx, &decision.entry)?;
        if links.iter().any(|l| l["status"] != "fresh") {
            return Err(FoundryError::StaleHandle);
        }
        let reference = self.decision_ref(decision)?;
        let words = NoteWords::new(&decision.entry.text);
        let exact = exact_rule_sources(tx, &decision.entry)?;
        if let Some(cursor) = after {
            id(cursor)?;
        }
        let tasks = tx.open_table(TASKS)?;
        let mut rows = tasks.range::<&str>((
            after.map_or(Bound::Unbounded, Bound::Excluded),
            Bound::Unbounded,
        ))?;
        let mut dependents = Vec::new();
        let mut possible = Vec::new();
        let mut last = None;
        let mut stale = 0;
        for row in rows.by_ref().take(8) {
            control.check()?;
            let (key, raw) = row?;
            last = Some(key.value().to_owned());
            if key.value() == self.task_id {
                continue;
            }
            let other = decode(raw.value(), key.value(), &self.workspace_id)?;
            if let Some((confirmed, mut reason)) =
                other.reason_for_decision(tx, &reference, &exact, &words, control, &mut stale)?
            {
                reason["feature"] = json!(other.title);
                if confirmed {
                    dependents.push(reason);
                } else {
                    reason["clarification"] = json!(format!(
                        "Does “{}” rely on this decision from “{}”, or do the passages only use similar wording?",
                        other.title, self.title
                    ));
                    possible.push(reason);
                }
            }
        }
        let next = if rows.next().transpose()?.is_some() {
            last
        } else {
            None
        };
        let quote = prefix(&decision.entry.text, PASSAGE_BYTES);
        let mut original = citation(self, decision, 0, quote, links);
        original["original_bytes"] = json!(decision.entry.text.len());
        Ok(
            json!({"workspace_id":self.workspace_id,"feature":self.title,"decision":original,
            "decision_ref":reference,"dependents":dependents,"wording_matches":possible,"stale_evidence_omitted":stale,"next_after_id":next}),
        )
    }

    /// Prefer a declared, fresh requirement regardless of entry order. A
    /// wording match has its own output lane and never becomes a declaration.
    fn reason_for_decision(
        &self,
        tx: &redb::ReadTransaction,
        reference: &DecisionRef,
        exact: &BTreeSet<&str>,
        words: &NoteWords,
        control: &Control,
        stale: &mut usize,
    ) -> FResult<Option<(bool, Value)>> {
        let mut adopted = None;
        let mut suggestion = None;
        let mut best_words = 0;
        for item in &self.entries {
            control.check()?;
            let e = &item.entry;
            if !item.view.selected || self.superseded(&e.id) {
                continue;
            }
            let declared = e.relies_on.as_deref() == Some(reference);
            if !declared && (e.relies_on.is_some() || prior_kind(e).is_none()) {
                continue;
            }
            let shared_source = e
                .source_links
                .iter()
                .find(|link| exact.contains(link.as_str()))
                .filter(|_| e.kind.protected() && item.sha256 == words.digest);
            let (offset, quote, score) = if declared || shared_source.is_some() {
                (0, prefix(&e.text, PASSAGE_BYTES), 0)
            } else {
                let Some((offset, quote, shared)) =
                    words.best_passage(prefix(&e.text, QUERY_BYTES))
                else {
                    continue;
                };
                if shared.len() <= best_words {
                    continue;
                }
                (offset, quote, shared.len())
            };
            let links = source_links(tx, e)?;
            if links.iter().any(|l| l["status"] != "fresh") {
                *stale += 1;
                continue;
            }
            let mut reason = citation(self, item, offset, quote, links);
            reason["original_bytes"] = json!(e.text.len());
            reason["kind"] = json!(e.kind);
            if declared {
                reason["basis"] = json!("declared_dependency");
                return Ok(Some((true, reason)));
            }
            if let Some(link) = shared_source {
                reason["basis"] = json!("same_required_source_passage");
                reason["shared_source"] = json!(link);
                adopted.get_or_insert((true, reason));
                continue;
            }
            best_words = score;
            suggestion = Some((false, reason));
        }
        Ok(adopted.or(suggestion))
    }
}
