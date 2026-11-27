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
    pub(super) has_saved_decision: bool,
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
            has_saved_decision: false,
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

const TITLE_OPTION_LIMIT: usize = 3;

fn title_words(title: &str) -> Vec<String> {
    title
        .split(|c: char| !c.is_alphanumeric())
        .filter(|word| !word.is_empty())
        .map(str::to_lowercase)
        .collect()
}

/// A title supplied by the operator narrows the scope; it makes no claim
/// about which decision applies. Detect duplicate titles before showing text.
pub(super) fn in_named_feature(
    tx: &redb::ReadTransaction,
    workspace: &str,
    title: &str,
    words: &NoteWords,
    control: &Control,
) -> FResult<Value> {
    let mut feature = None;
    let fragment = title_words(title);
    let distinctive = fragment
        .iter()
        .any(|word| word.chars().count() >= 3 && !crate::store::prose_stopword(word));
    let mut partial_count = 0;
    // Only offer titles that can be selected unambiguously on the next call.
    let mut partial: Vec<(String, bool)> = Vec::new();
    let mut result = json!({"workspace_id": workspace, "basis": "operator_named_feature",
        "candidates": [], "matching": 0, "stale_omitted": 0});
    for row in tx.open_table(TASKS)?.iter()? {
        control.check()?;
        let (key, raw) = row?;
        let task = decode(raw.value(), key.value(), workspace)?;
        if task.proposal.is_some() {
            continue;
        }
        let saved = task.title.trim();
        if saved.eq_ignore_ascii_case(title) {
            if feature.is_some() {
                result["clarification"] = json!(
                    "Several features have that title. What words do you remember from the decision itself?"
                );
                return Ok(result);
            }
            feature = Some(task);
        } else if feature.is_none()
            && distinctive
            && title_words(saved)
                .windows(fragment.len())
                .any(|w| w == fragment)
        {
            partial_count += 1;
            if let Some((_, duplicate)) = partial
                .iter_mut()
                .find(|(name, _)| name.trim().eq_ignore_ascii_case(saved))
            {
                *duplicate = true;
            } else if partial.len() < TITLE_OPTION_LIMIT {
                partial.push((task.title, false));
            }
        }
    }
    let Some(task) = feature else {
        let titles: Vec<_> = partial
            .into_iter()
            .filter_map(|(name, duplicate)| (!duplicate).then_some(name))
            .collect();
        result["clarification"] = json!(match (titles.len(), partial_count) {
            (0, 0) =>
                "I couldn't find a feature with those title words. What other title words or phrase from the decision do you remember?",
            (0, _) =>
                "Several matching features have the same title. What other title words or phrase from the decision do you remember?",
            (1, 1) => "Is this the feature you meant?",
            _ =>
                "Which of these features did you mean? If yours isn't shown, what other title word do you remember?",
        });
        result["matching_feature_titles"] = json!(partial_count);
        result["feature_title_options"] = json!(titles);
        return Ok(result);
    };
    let mut options = Suggestions::default();
    for item in &task.entries {
        control.check()?;
        let e = &item.entry;
        if e.kind != Kind::Decision || !item.view.selected || task.superseded(&e.id) {
            continue;
        }
        let links = source_links(tx, e)?;
        if links.iter().any(|l| l["status"] != "fresh") {
            options.stale += 1;
            continue;
        }
        let (at, passage, shared) = words
            .passage_with_minimum(&e.text, 1)
            .unwrap_or_else(|| (0, prefix(&e.text, PASSAGE_BYTES), Vec::new()));
        let mut candidate = citation(&task, item, at, passage, links);
        candidate["feature"] = json!(task.title);
        candidate["kind"] = json!(PriorKind::Decision);
        candidate["shared_words"] = json!(shared);
        options.offer(shared.len(), true, candidate);
        options.matching += 1;
    }
    let mut candidates = Vec::new();
    for (_, _, mut option) in options.best {
        source_review::mark_cited_originals(tx, workspace, &mut option, true, control)?;
        candidates.push(option);
    }
    result["matching"] = json!(options.matching);
    result["stale_omitted"] = json!(options.stale);
    result["clarification"] = json!(match candidates.len() {
        0 =>
            "I couldn't offer a current decision from that feature. Do you remember another feature or a phrase from the decision?",
        1 => "Is this the saved decision you meant?",
        _ => "Which of these saved decisions did you mean, or do you remember other words from it?",
    });
    result["candidates"] = json!(candidates);
    Ok(result)
}
