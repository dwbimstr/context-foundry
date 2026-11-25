//! A proposal preserves the operator's words. Its impact is a fresh view of
//! existing features; answering the scope question never edits a decision.
use super::*;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProposalAnswer {
    pub disposition: Disposition,
    pub author: String,
    pub text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope_token: Option<String>,
}

impl ProposalAnswer {
    fn validate(&self) -> FResult<()> {
        text(&self.author, 256)?;
        text(&self.text, 4096)?;
        let valid = match (&self.disposition, &self.scope_token) {
            (Disposition::Dismiss, None) => true,
            (Disposition::Accept, Some(t)) => {
                t.len() == 64 && t.bytes().all(|b| b.is_ascii_hexdigit())
            }
            _ => false,
        };
        valid
            .then_some(())
            .ok_or_else(|| invalid("accept requires the offered scope_token; dismiss omits it"))
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SavedScope {
    decision: DecisionRef,
    lead_id: String,
    review_revision: Option<u64>,
    decision_quote: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct SavedAnswer {
    #[serde(flatten)]
    answer: ProposalAnswer,
    scope: Option<SavedScope>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(in crate::task_context) struct ProposalState {
    answers: Vec<SavedAnswer>,
}

pub(in crate::task_context) fn proposal_title(original: &str) -> &str {
    prefix(original.trim_start(), 200)
}

impl ProposalState {
    pub(in crate::task_context) fn task_id(
        original: &str,
        source: &str,
        author: &str,
    ) -> FResult<String> {
        text(original, QUERY_BYTES)?;
        text(source, 1024)?;
        text(author, 256)?;
        Ok(format!(
            "proposal-{}",
            crate::digest(&serde_json::to_vec(&(original, source, author))?)
        ))
    }

    pub(in crate::task_context) fn validate(&self, task: &Task) -> FResult<()> {
        let [item] = task.entries.as_slice() else {
            return Err(invalid("invalid proposal original"));
        };
        let e = &item.entry;
        if e.kind != Kind::Question
            || e.id != "original"
            || Self::task_id(&e.text, &e.provenance, &e.author)? != task.task_id
            || self.answers.len() > 128
        {
            return Err(invalid("invalid proposal record"));
        }
        for saved in &self.answers {
            saved.answer.validate()?;
            if saved.scope.is_some() != saved.answer.scope_token.is_some() {
                return Err(invalid("invalid proposal scope"));
            }
            if let Some(scope) = &saved.scope {
                scope.decision.validate(Kind::Requirement)?;
                id(&scope.lead_id)?;
                text(&scope.decision_quote, PASSAGE_BYTES)?;
            }
        }
        Ok(())
    }
}

struct Route {
    token: String,
    origin: Task,
    scope: SavedScope,
    lead: Value,
    basis: &'static str,
    review: Option<Value>,
    example: Option<Value>,
}

impl Route {
    fn token_for(&self, proposal: &Task) -> FResult<String> {
        Ok(crate::digest(&serde_json::to_vec(&(
            &proposal.task_id,
            &self.scope.decision,
            &self.lead["entry_id"],
            &self.lead["sha256"],
            self.basis,
            self.review.as_ref().map(|r| &r["review_revision"]),
        ))?))
    }
}

struct Routes {
    items: Vec<Route>,
    matching: usize,
    stale: usize,
    omitted: usize,
    omitted_examples: usize,
}

impl Routes {
    fn current(tx: &redb::ReadTransaction, proposal: &Task, control: &Control) -> FResult<Self> {
        // A recorded choice is addressed directly even if new wording outranks
        // the original lead. Reopen discovery only if that evidence changed.
        if let Some(saved) = proposal.proposal.as_ref().and_then(|s| s.answers.last())
            && let Some(scope) = &saved.scope
        {
            let routes = Self::discover(tx, proposal, Some(scope), control)?;
            if routes
                .items
                .iter()
                .any(|r| Some(&r.token) == saved.answer.scope_token.as_ref())
            {
                return Ok(routes);
            }
        }
        let mut routes = Self::discover(tx, proposal, None, control)?;
        if routes.items.len() < 2 {
            routes.add_confirmed_examples(tx, proposal, control)?;
        }
        Ok(routes)
    }

    // Prior confirmations supply vocabulary only. Recheck the specific
    // decision/review, then issue a distinct token for this new question.
    fn add_confirmed_examples(
        &mut self,
        tx: &redb::ReadTransaction,
        proposal: &Task,
        control: &Control,
    ) -> FResult<()> {
        let query = &proposal.entries[0].entry.text;
        let words = NoteWords::new(query);
        let table = tx.open_table(TASKS)?;
        let mut examples = Vec::new();
        for row in table.iter()? {
            control.check()?;
            let (key, raw) = row?;
            if !key.value().starts_with("proposal-") || key.value() == proposal.task_id {
                continue;
            }
            let prior = decode(raw.value(), key.value(), &proposal.workspace_id)?;
            let Some(saved) = prior
                .proposal
                .as_ref()
                .and_then(|p| p.answers.last())
                .filter(|a| a.scope.is_some())
            else {
                continue;
            };
            let score = [&saved.answer.text, &prior.entries[0].entry.text]
                .into_iter()
                .map(|v| {
                    if v.trim().eq_ignore_ascii_case(query.trim()) {
                        usize::MAX
                    } else {
                        words.best_passage(v).map_or(0, |(_, _, s)| s.len())
                    }
                })
                .max()
                .unwrap_or(0);
            if score > 0 {
                examples.push((score, prior));
                examples.sort_by_key(|(s, _)| std::cmp::Reverse(*s));
                if examples.len() > 4 {
                    examples.pop();
                    self.omitted_examples += 1;
                }
            }
        }
        for (_, prior) in examples {
            control.check()?;
            let saved = prior
                .proposal
                .as_ref()
                .and_then(|p| p.answers.last())
                .expect("confirmed example");
            let checked = Self::discover(tx, &prior, saved.scope.as_ref(), control)?;
            let Some(mut route) = checked
                .items
                .into_iter()
                .find(|r| Some(&r.token) == saved.answer.scope_token.as_ref())
            else {
                self.stale += 1;
                continue;
            };
            if self
                .items
                .iter()
                .any(|r| r.scope.decision == route.scope.decision)
            {
                continue;
            }
            if self.items.len() == 2 {
                self.omitted += 1;
                continue;
            }
            route.token = route.token_for(proposal)?;
            let question = &prior.entries[0].entry;
            route.example = Some(
                json!({"proposal_id":prior.task_id,"revision":prior.revision,
                "question":prefix(&question.text,PASSAGE_BYTES),"question_author":question.author,"source":question.provenance,
                "answer":prefix(&saved.answer.text,PASSAGE_BYTES),"answered_by":saved.answer.author}),
            );
            self.items.push(route);
        }
        Ok(())
    }

    fn discover(
        tx: &redb::ReadTransaction,
        proposal: &Task,
        saved: Option<&SavedScope>,
        control: &Control,
    ) -> FResult<Self> {
        let table = tx.open_table(TASKS)?;
        let words = NoteWords::new(&proposal.entries[0].entry.text);
        let leads = if let Some(scope) = saved {
            let mut leads = Suggestions {
                include_identical_captures: true,
                ..Suggestions::default()
            };
            if let Some(row) = table.get(scope.decision.task_id.as_str())? {
                let origin = decode(row.value(), &scope.decision.task_id, &proposal.workspace_id)?;
                // A saved choice binds identity; the new proposal may use only
                // team vocabulary. Recover the referenced current lead directly.
                if let Some(item) = origin.entries.iter().find(|e| {
                    e.entry.id == scope.lead_id
                        && e.view.selected
                        && !origin.superseded(&e.entry.id)
                }) && let Some(kind) = prior_kind(&item.entry)
                {
                    let links = source_links(tx, &item.entry)?;
                    if links.iter().all(|l| l["status"] == "fresh") {
                        let (offset, quote, shared) =
                            words.best_passage(&item.entry.text).unwrap_or_else(|| {
                                (0, prefix(&item.entry.text, PASSAGE_BYTES), Vec::new())
                            });
                        let mut lead = citation(&origin, item, offset, quote, links);
                        lead["kind"] = json!(kind);
                        lead["feature"] = json!(origin.title);
                        lead["shared_words"] = json!(shared);
                        leads.best.push((shared.len(), true, lead));
                        leads.matching = 1;
                    } else {
                        leads.stale = 1;
                    }
                }
            }
            leads
        } else {
            Suggestions {
                include_identical_captures: true,
                ..Suggestions::default()
            }
            .workspace(tx, &proposal.workspace_id, &words, control)?
        };
        let mut routes = Self {
            items: Vec::new(),
            matching: leads.matching,
            stale: leads.stale,
            omitted: 0,
            omitted_examples: 0,
        };
        let mut seen = BTreeSet::new();
        for (_, _, lead) in leads.best {
            let task_id = lead["task_id"].as_str().ok_or(FoundryError::NotFound)?;
            let raw = table.get(task_id)?.ok_or(FoundryError::NotFound)?;
            let origin = decode(raw.value(), task_id, &proposal.workspace_id)?;
            let lead_id = lead["entry_id"].as_str().ok_or(FoundryError::NotFound)?;
            let reviews = origin.impact_reviews(tx, lead_id, &mut routes.stale)?;
            for item in &origin.entries {
                control.check()?;
                let decision = &item.entry;
                if decision.kind != Kind::Decision
                    || saved.is_some_and(|s| s.decision.entry_id != decision.id)
                    || !item.view.selected
                    || origin.superseded(&decision.id)
                    || seen.contains(&(task_id.to_owned(), decision.id.clone()))
                {
                    continue;
                }
                let mut support = None;
                for review in &reviews {
                    if saved.is_some_and(|s| s.review_revision != Some(review.review.revision)) {
                        continue;
                    }
                    if let Some(basis) = review.relation(decision)? {
                        support = Some((basis, review));
                        break;
                    }
                }
                if support.is_none() && decision.id != lead_id {
                    continue;
                }
                if source_links(tx, decision)?
                    .iter()
                    .any(|l| l["status"] != "fresh")
                {
                    routes.stale += 1;
                    continue;
                }
                seen.insert((task_id.to_owned(), decision.id.clone()));
                if routes.items.len() == 2 {
                    routes.omitted += 1;
                    continue;
                }
                let (basis, review) = support.map_or(("wording_match", None), |(b, r)| {
                    (b, Some(r.citation(&origin)))
                });
                let reference = origin.decision_ref(item)?;
                // A new review of the same words requires a new answer. Unrelated
                // feature revisions do not invalidate this particular interpretation.
                let mut route = Route {
                    token: String::new(),
                    origin: origin.clone(),
                    scope: SavedScope {
                        decision: reference,
                        lead_id: lead_id.into(),
                        review_revision: support.map(|(_, r)| r.review.revision),
                        decision_quote: prefix(&decision.text, PASSAGE_BYTES).into(),
                    },
                    lead: lead.clone(),
                    basis,
                    review,
                    example: None,
                };
                route.token = route.token_for(proposal)?;
                routes.items.push(route);
            }
        }
        Ok(routes)
    }
}

struct ReviewedPassage<'a> {
    review: &'a ConnectionReview,
    prior: &'a StoredEntry,
    handle: Option<store::HandleRef>,
}

impl ReviewedPassage<'_> {
    fn citation(&self, origin: &Task) -> Value {
        let [start, end] = self.review.prior_span;
        json!({"entry_id":self.prior.entry.id,"task_id":origin.task_id,"author":self.prior.entry.author,
            "source":self.prior.entry.provenance,"offset":start,"end":end,"quote":self.prior.entry.text[start..end],
            "reviewed_by":self.review.author,"review_revision":self.review.revision,"handle":self.handle.as_ref().map(ToString::to_string)})
    }

    fn relation(&self, decision: &Entry) -> FResult<Option<&'static str>> {
        if self.prior.entry.id == decision.id {
            return Ok(Some("reviewed_decision"));
        }
        let Some(passage) = &self.handle else {
            return Ok(None);
        };
        let capture = store::HandleRef::parse(&self.prior.entry.source_links[0])?;
        for link in &decision.source_links {
            let rule = store::HandleRef::parse(link)?;
            if rule.path != capture.path
                || rule.sha32 != capture.sha32
                || rule.ws16 != capture.ws16
                || rule.start < capture.start
                || rule.end > capture.end
                || self
                    .prior
                    .entry
                    .text
                    .get((rule.start - capture.start) as usize..(rule.end - capture.start) as usize)
                    != Some(decision.text.as_str())
            {
                continue;
            }
            if rule.start < passage.end && passage.start < rule.end {
                return Ok(Some("overlapping_reviewed_source"));
            }
            let (left, right) = if rule.end <= passage.start {
                (rule.end, passage.start)
            } else {
                (passage.end, rule.start)
            };
            if right - left <= 16
                && self
                    .prior
                    .entry
                    .text
                    .get((left - capture.start) as usize..(right - capture.start) as usize)
                    .is_some_and(|gap| gap.chars().all(char::is_whitespace))
            {
                return Ok(Some("adjacent_in_reviewed_source"));
            }
        }
        Ok(None)
    }
}

impl Task {
    fn impact_reviews<'a>(
        &'a self,
        tx: &redb::ReadTransaction,
        lead: &str,
        stale: &mut usize,
    ) -> FResult<Vec<ReviewedPassage<'a>>> {
        let mut seen = BTreeSet::new();
        let mut reviews = Vec::new();
        for r in self
            .connection_reviews
            .iter()
            .rev()
            .filter(|r| r.entry_id == lead)
        {
            if !seen.insert(&r.prior_id)
                || r.disposition != Disposition::Accept
                || self.superseded(&r.prior_id)
            {
                continue;
            }
            let prior = self
                .entries
                .iter()
                .find(|e| e.entry.id == r.prior_id)
                .ok_or(FoundryError::NotFound)?;
            if !prior.view.selected {
                continue;
            }
            if source_links(tx, &prior.entry)?
                .iter()
                .any(|l| l["status"] != "fresh")
            {
                *stale += 1;
                continue;
            }
            let [start, end] = r.prior_span;
            let handle = reviewed_source_handle(tx, &prior.entry, start, end)?
                .as_deref()
                .map(store::HandleRef::parse)
                .transpose()?;
            reviews.push(ReviewedPassage {
                review: r,
                prior,
                handle,
            });
        }
        Ok(reviews)
    }

    pub(in crate::task_context) fn save_proposal(
        &mut self,
        original: &str,
        source: &str,
        author: &str,
    ) -> FResult<()> {
        self.edit(&Edit::Add {
            entry: Entry {
                id: "original".into(),
                kind: Kind::Question,
                text: original.into(),
                provenance: source.into(),
                author: author.into(),
                source_links: vec![],
                supersedes: None,
                action_key: None,
                assertions: vec![],
                relies_on: None,
            },
        })?;
        self.proposal = Some(ProposalState::default());
        Ok(())
    }

    pub(in crate::task_context) fn answer_proposal(
        &mut self,
        tx: &redb::ReadTransaction,
        answer: &ProposalAnswer,
        control: &Control,
    ) -> FResult<()> {
        answer.validate()?;
        if self.proposal.is_none() {
            return Err(invalid("clarify_proposal requires a saved change proposal"));
        }
        let scope = if let Some(token) = &answer.scope_token {
            let current = Routes::current(tx, self, control)?;
            Some(
                current
                    .items
                    .into_iter()
                    .find(|route| route.token == *token)
                    .ok_or_else(|| {
                        FoundryError::Conflict("support changed; read proposal_impact again".into())
                    })?
                    .scope,
            )
        } else {
            None
        };
        self.proposal
            .as_mut()
            .ok_or(FoundryError::NotFound)?
            .answers
            .push(SavedAnswer {
                answer: answer.clone(),
                scope,
            });
        Ok(())
    }

    pub(in crate::task_context) fn proposal_impact(
        &self,
        tx: &redb::ReadTransaction,
        control: &Control,
    ) -> FResult<Value> {
        let state = self
            .proposal
            .as_ref()
            .ok_or_else(|| invalid("proposal_impact requires a saved change proposal"))?;
        let original = &self.entries[0];
        let mut result = json!({"workspace_id":self.workspace_id,"task_id":self.task_id,"revision":self.revision,
            "proposal":citation(self,original,0,&original.entry.text,vec![]),"policy_status":"proposed",
            "last_answer":state.answers.last(),"candidates":[],"clarification":null});
        let answer = state.answers.last().map(|s| &s.answer);
        if answer.is_some_and(|a| a.disposition == Disposition::Dismiss) {
            result["scope_status"] = json!("dismissed");
            return Ok(result);
        }
        let routes = Routes::current(tx, self, control)?;
        let chosen = answer.and_then(|a| a.scope_token.as_deref());
        let mut candidates = Vec::new();
        let mut confirmed = false;
        for route in routes.items {
            let selected = Some(route.token.as_str()) == chosen;
            let mut impact =
                route
                    .origin
                    .decision_uses(tx, &route.scope.decision.entry_id, None, control)?;
            let decision = &impact["decision"];
            let query = prefix(decision["quote"].as_str().unwrap_or_default(), 256);
            impact["implementation_context"] = json!({"query":query,"task_id":route.origin.task_id,"task_revision":route.origin.revision,
                "compose_evidence":true,"compact_evidence":true,"tokens":6000});
            candidates.push(json!({"scope_token":route.token,"selected":selected,"basis":route.basis,"lead":route.lead,
                "reviewed_passage":route.review,"previous_clarification":route.example,"impact":impact}));
            confirmed |= selected;
        }
        let (status, question) = if confirmed {
            ("confirmed_scope", None)
        } else {
            let status = if chosen.is_some() {
                "support_changed"
            } else {
                "needs_clarification"
            };
            let question=match candidates.as_slice() {
                [] => "Which saved decision or source passage should govern this request?".into(),
                [one] => format!("Does your request concern the rule “{}” in “{}”?",one["impact"]["decision"]["quote"].as_str().unwrap_or_default(),one["impact"]["feature"].as_str().unwrap_or_default()),
                _ => "Which quoted decision governs your request, or should this proposal be set aside?".into(),
            };
            (status, Some(question))
        };
        result["scope_status"] = json!(status);
        result["clarification"] = json!(question);
        result["candidates"] = json!(candidates);
        result["discovery"] = json!({"wording_matches":routes.matching,"lead_limit":2,"additional_decisions":routes.omitted,"unchecked_wording_examples":routes.omitted_examples,"stale_omitted":routes.stale});
        Ok(result)
    }
}
