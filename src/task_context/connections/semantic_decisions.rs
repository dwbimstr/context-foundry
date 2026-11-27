//! Title-blind recovery on a small workspace. A model ranks quotations;
//! only the original saved record supplies a decision and its provenance.
use super::*;

const TASK_LIMIT: usize = 32;
const DECISION_LIMIT: usize = 8;
const LEAD_LIMIT: usize = 2;

struct Passage {
    reference: DecisionRef,
    revision: u64,
    quote: String,
}

impl Engine {
    pub(crate) fn semantic_decision_leads(
        &self,
        workspace: &str,
        mut lookup: Value,
        control: &Control,
        score: impl FnOnce(&[&str]) -> Result<Vec<f64>, String>,
    ) -> FResult<Value> {
        let mut passages = Vec::new();
        {
            let tx = self.db.begin_read()?;
            let table = tx.open_table(TASKS)?;
            for (i, row) in table.iter()?.enumerate() {
                control.check()?;
                if i == TASK_LIMIT {
                    lookup["semantic_fallback"] = json!("workspace_limit");
                    return Ok(lookup);
                }
                let (key, raw) = row?;
                let task = decode(raw.value(), key.value(), workspace)?;
                if task.proposal.is_some() {
                    continue;
                }
                for item in &task.entries {
                    let entry = &item.entry;
                    if entry.kind != Kind::Decision
                        || !item.view.selected
                        || task.superseded(&entry.id)
                    {
                        continue;
                    }
                    if source_links(&tx, entry)?
                        .iter()
                        .any(|link| link["status"] != "fresh")
                    {
                        continue;
                    }
                    if passages.len() == DECISION_LIMIT {
                        lookup["semantic_fallback"] = json!("decision_limit");
                        return Ok(lookup);
                    }
                    passages.push(Passage {
                        reference: task.decision_ref(item)?,
                        revision: task.revision,
                        quote: prefix(&entry.text, PASSAGE_BYTES).to_owned(),
                    });
                }
            }
        }
        if passages.is_empty() {
            return Ok(lookup);
        }
        control.check()?;
        let texts = passages
            .iter()
            .map(|p| p.quote.as_str())
            .collect::<Vec<_>>();
        let scores = score(&texts);
        control.check()?;
        let scores = match scores {
            Ok(scores)
                if scores.len() == passages.len() && scores.iter().all(|s| s.is_finite()) =>
            {
                scores
            }
            Ok(_) => {
                lookup["semantic_fallback"] = json!("invalid_scores");
                return Ok(lookup);
            }
            Err(reason) => {
                lookup["semantic_fallback"] = json!(reason);
                return Ok(lookup);
            }
        };
        let mut ranked: Vec<_> = scores.into_iter().zip(passages).collect();
        ranked.sort_by(|a, b| b.0.total_cmp(&a.0));
        // Validate the returned identity, view and linked sources after inference.
        let tx = self.db.begin_read()?;
        let table = tx.open_table(TASKS)?;
        let mut leads = Vec::new();
        for (_, passage) in ranked {
            control.check()?;
            let reference = &passage.reference;
            let Some(raw) = table.get(reference.task_id.as_str())? else {
                continue;
            };
            let task = decode(raw.value(), &reference.task_id, workspace)?;
            if task.revision != passage.revision {
                continue;
            }
            let Ok(item) = task.current_decision(&reference.entry_id) else {
                continue;
            };
            if task.decision_ref(item)? != *reference {
                continue;
            }
            let links = source_links(&tx, &item.entry)?;
            if links.iter().any(|link| link["status"] != "fresh") {
                continue;
            }
            let mut lead = citation(&task, item, 0, &passage.quote, links);
            lead["feature"] = json!(task.title);
            lead["kind"] = json!(PriorKind::Decision);
            lead["original_bytes"] = json!(item.entry.text.len());
            task.describe_cited_originals(
                &tx,
                &mut lead,
                source_review::SourceExamples::Changed,
                control,
            )?;
            leads.push(lead);
            if leads.len() == LEAD_LIMIT {
                break;
            }
        }
        lookup["semantic_fallback"] = json!("local_decision_passages");
        lookup["semantic_leads"] = json!(leads);
        Ok(lookup)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ranks_only_current_decisions_and_rechecks_after_scoring() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("root");
        std::fs::create_dir(&root).unwrap();
        let engine = Engine::initialize(&dir.path().join("store"), &root).unwrap();
        let workspace = engine.workspace_id().unwrap();
        let ctl = Control::unbounded();
        let cmd = |command| {
            let r = crate::task_context::parse(
                json!({"workspace_id":workspace,"task_id":"fixture","command":command}),
            )
            .unwrap();
            engine.task_context(&r, &ctl).unwrap()
        };
        let add = |id: &str, kind: &str, words: &str| json!({"op":"add","entry":{"id":id,"kind":kind,"text":words,"author":"owner","provenance":"meeting","source_links":[]}});
        let rev = cmd(json!({"op":"create","title":"Samples"}))["revision"].clone();
        let rev = cmd(json!({"op":"apply","expected_revision":rev,"edits":[add("first","decision","Keep the paper trail"),add("second","decision","Send a weekly report"),add("note","observation","A note")]}))["revision"].clone();
        let lookup = json!({"candidates":[],"matching":0,"recovery_question":"Which feature?"});
        let result = engine
            .semantic_decision_leads(&workspace, lookup.clone(), &ctl, |texts| {
                assert_eq!(texts, &["Keep the paper trail", "Send a weekly report"]);
                Ok(vec![0.1, 0.9])
            })
            .unwrap();
        assert_eq!(result["semantic_leads"][0]["entry_id"], "second");
        assert_eq!(result["semantic_leads"][0]["task_revision"], rev);
        assert_eq!(result["candidates"], json!([]));
        assert_eq!(result["recovery_question"], "Which feature?");
        let unavailable = engine
            .semantic_decision_leads(&workspace, lookup.clone(), &ctl, |_| {
                Err("provider_busy".into())
            })
            .unwrap();
        assert_eq!(unavailable["semantic_fallback"], "provider_busy");
        assert!(unavailable.get("semantic_leads").is_none());
        let changed = engine
            .semantic_decision_leads(&workspace, lookup.clone(), &ctl, |_| {
                let mut replacement = add("correction", "decision", "Keep the signed paper trail");
                replacement["entry"]["supersedes"] = json!("first");
                cmd(json!({"op":"apply","expected_revision":rev,"edits":[replacement]}));
                Ok(vec![1.0, 0.0])
            })
            .unwrap();
        assert_eq!(changed["semantic_leads"], json!([]));
        let refreshed = engine
            .semantic_decision_leads(&workspace, lookup, &ctl, |texts| {
                assert_eq!(
                    texts,
                    &["Send a weekly report", "Keep the signed paper trail"]
                );
                Ok(vec![0.1, 0.9])
            })
            .unwrap();
        assert_eq!(refreshed["semantic_leads"][0]["entry_id"], "correction");
        let extra = (0..7)
            .map(|i| add(&format!("extra-{i}"), "decision", "Another decision"))
            .collect::<Vec<_>>();
        let revision = cmd(json!({"op":"inspect"}))["revision"].clone();
        cmd(json!({"op":"apply","expected_revision":revision,"edits":extra}));
        let limited = engine
            .semantic_decision_leads(
                &workspace,
                json!({"recovery_question":"Which feature?"}),
                &ctl,
                |_| panic!("partial pools cannot go to the model"),
            )
            .unwrap();
        assert_eq!(limited["semantic_fallback"], "decision_limit");
        assert!(limited.get("semantic_leads").is_none());
    }
}
