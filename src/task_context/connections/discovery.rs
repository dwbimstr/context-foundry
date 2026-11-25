//! Workspace lookup can use another feature's own explanation of why it
//! relies on a decision. A declaration supplies a route, never a new policy.
use super::*;

const DECLARATION_LIMIT: usize = 8;

struct Reason {
    target: DecisionRef,
    citation: Value,
    shared: Vec<String>,
}

pub(super) struct DecisionDiscovery {
    phrases: BTreeSet<(String, String)>,
    reasons: Vec<Reason>,
    pub(super) omitted: usize,
    pub(super) weak_wording: bool,
}

fn adjacent_words(text: &str) -> impl Iterator<Item = (&str, &str)> {
    let tokens = text
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty());
    tokens.clone().zip(tokens.skip(1))
}

impl DecisionDiscovery {
    pub(super) fn new(note: &str, words: &mut NoteWords) -> Self {
        for filler in ["its", "may", "after", "before", "what", "who", "during"] {
            words.words.remove(words.stemmer.stem(filler).as_ref());
        }
        let lower = note.to_lowercase();
        let informative = |w: &str| words.words.contains_key(words.stemmer.stem(w).as_ref());
        let phrases = adjacent_words(&lower)
            .filter(|(a, b)| a != b && informative(a) && informative(b))
            .map(|(a, b)| (a.to_owned(), b.to_owned()))
            .collect();
        Self {
            phrases,
            reasons: Vec::new(),
            omitted: 0,
            weak_wording: false,
        }
    }

    pub(super) fn admits(&self, words: &NoteWords, passage: &str, shared: usize) -> bool {
        shared >= 2
            && (shared >= 3
                || shared * 2 >= words.words.len()
                || adjacent_words(&passage.to_lowercase())
                    .any(|(a, b)| self.phrases.contains(&(a.to_owned(), b.to_owned()))))
    }

    pub(super) fn consider(&mut self, target: &DecisionRef, citation: &Value, shared: &[String]) {
        self.reasons.push(Reason {
            target: target.clone(),
            citation: citation.clone(),
            shared: shared.to_vec(),
        });
        self.reasons
            .sort_by_key(|r| std::cmp::Reverse(r.shared.len()));
        if self.reasons.len() > DECLARATION_LIMIT {
            self.reasons.pop();
            self.omitted += 1;
        }
    }

    pub(super) fn follow(
        &mut self,
        tx: &redb::ReadTransaction,
        workspace: &str,
        words: &NoteWords,
        results: &mut Suggestions,
        control: &Control,
    ) -> FResult<()> {
        let table = tx.open_table(TASKS)?;
        let mut added = BTreeSet::new();
        for reason in std::mem::take(&mut self.reasons) {
            control.check()?;
            let reference = &reason.target;
            let Some(row) = table.get(reference.task_id.as_str())? else {
                continue;
            };
            let task = decode(row.value(), &reference.task_id, workspace)?;
            let Ok(item) = task.current_decision(&reference.entry_id) else {
                continue;
            };
            if task.decision_ref(item)? != *reference {
                continue;
            }
            let links = source_links(tx, &item.entry)?;
            if links.iter().any(|l| l["status"] != "fresh") {
                results.stale += 1;
                continue;
            }
            let (offset, quote, direct) = words
                .passage_with_minimum(&item.entry.text, 1)
                .unwrap_or_else(|| (0, prefix(&item.entry.text, PASSAGE_BYTES), Vec::new()));
            let joint: BTreeSet<_> = direct.iter().chain(&reason.shared).collect();
            // The other feature must actually add wording. For two weak
            // passages, require distinct contributions on both ends.
            if joint.len() == direct.len() {
                continue;
            }
            let reason_quote = reason.citation["quote"]
                .as_str()
                .expect("constructed citation");
            let complementary =
                joint.len() > reason.shared.len() && joint.len() * 4 >= words.words.len();
            if !self.admits(words, reason_quote, reason.shared.len()) && !complementary {
                continue;
            }
            if !self.admits(words, quote, direct.len()) && added.insert(reference.token.clone()) {
                results.matching += 1;
            }
            let mut candidate = citation(&task, item, offset, quote, links);
            candidate["feature"] = json!(task.title);
            candidate["kind"] = json!(PriorKind::Decision);
            candidate["shared_words"] = json!(direct);
            candidate["via_requirement"] = reason.citation;
            candidate["via_requirement"]["relies_on"] = json!(reference);
            results.offer(joint.len(), true, candidate);
        }
        Ok(())
    }
}
