//! Candidate links from an ordinary note to earlier attributed passages.
//! Only a human disposition is persisted; word matches never revise a source.
use super::*;
use rust_stemmers::{Algorithm, Stemmer};

const QUERY_BYTES: usize = 4096;
const PASSAGE_BYTES: usize = 512;
const MAX_REVIEWS: usize = 128;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConnectionChoice {
    pub expected_revision: u64,
    pub entry_id: String,
    pub prior_id: String,
    pub prior_span: [usize; 2],
    pub disposition: Disposition,
    pub author: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ConnectionReview {
    entry_id: String,
    prior_id: String,
    prior_span: [usize; 2],
    disposition: Disposition,
    author: String,
    revision: u64,
}

fn prefix(s: &str, bytes: usize) -> &str {
    let mut end = s.len().min(bytes);
    while !s.is_char_boundary(end) {
        end -= 1;
    }
    &s[..end]
}

struct NoteWords {
    stemmer: Stemmer,
    words: BTreeMap<String, String>,
    digest: String,
}

fn words(text: &str) -> impl Iterator<Item = &str> {
    text.split(|c: char| !c.is_alphanumeric())
        .filter(|w| w.len() >= 3)
}

impl NoteWords {
    fn new(note: &str) -> Self {
        let stemmer = Stemmer::create(Algorithm::English);
        let mut terms = BTreeMap::new();
        for word in words(prefix(note, QUERY_BYTES)) {
            let lower = word.to_lowercase();
            if crate::store::prose_stopword(&lower)
                || [
                    "have", "has", "had", "not", "then", "they", "them", "our", "will", "still",
                    "someone",
                ]
                .contains(&lower.as_str())
            {
                continue;
            }
            terms
                .entry(stemmer.stem(&lower).into_owned())
                .or_insert_with(|| word.to_owned());
        }
        Self {
            stemmer,
            words: terms,
            digest: crate::digest(note.as_bytes()),
        }
    }

    /// Score contiguous sentences so that a nearby exception can accompany
    /// its rule. Distant words in the same document cannot form a promise.
    fn best_passage<'a>(&self, original: &'a str) -> Option<(usize, &'a str, Vec<String>)> {
        let mut best = None;
        let mut best_rank = (1, std::cmp::Reverse(0));
        let mut consider = |start: usize, end: usize| {
            let raw = &original[start..end];
            let quote = raw.trim();
            let shared: BTreeSet<_> = words(quote)
                .filter_map(|w| {
                    self.words
                        .get(self.stemmer.stem(&w.to_lowercase()).as_ref())
                })
                .collect();
            let rank = (shared.len(), std::cmp::Reverse(quote.len()));
            if rank > best_rank {
                best_rank = rank;
                best = Some((
                    start + raw.len() - raw.trim_start().len(),
                    quote,
                    shared.into_iter().cloned().collect(),
                ));
            }
        };
        let mut at = 0;
        let mut previous = 0;
        for sentence in original.split_inclusive(['.', '?', '!']) {
            let mut remaining = sentence;
            while !remaining.is_empty() {
                let window = prefix(remaining, PASSAGE_BYTES);
                let len = if window.len() < remaining.len() {
                    window
                        .rfind(char::is_whitespace)
                        .filter(|&i| i > 0)
                        .unwrap_or(window.len())
                } else {
                    window.len()
                };
                let end = at + len;
                consider(at, end);
                if previous < at
                    && end - previous <= PASSAGE_BYTES
                    && !original[previous..end].contains("\n\n")
                {
                    consider(previous, end);
                }
                previous = at;
                at = end;
                remaining = &remaining[len..];
            }
        }
        best
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
enum PriorKind {
    Decision,
    ExplicitPromise,
    IndexedPassage,
    AttributedAccount,
}

fn prior_kind(e: &Entry) -> Option<PriorKind> {
    match e.kind {
        Kind::Decision => Some(PriorKind::Decision),
        Kind::Constraint | Kind::Requirement => Some(PriorKind::ExplicitPromise),
        Kind::Observation if e.is_captured_source() => Some(if e.source_links.is_empty() {
            PriorKind::AttributedAccount
        } else {
            PriorKind::IndexedPassage
        }),
        _ => None,
    }
}

fn citation(
    task: &Task,
    item: &StoredEntry,
    offset: usize,
    quote: &str,
    links: Vec<Value>,
) -> Value {
    json!({"task_id":task.task_id,"task_revision":task.revision,"entry_id":item.entry.id,
        "sha256":item.sha256,"author":item.entry.author,"source":item.entry.provenance,
        "offset":offset,"end":offset+quote.len(),"quote":quote,"source_links":links})
}

#[derive(Default)]
struct Suggestions {
    // Stable sort preserves task key and entry order for otherwise equal passages.
    best: Vec<(usize, bool, Value)>,
    matching: usize,
    stale: usize,
}

impl Suggestions {
    fn collect(
        &mut self,
        tx: &redb::ReadTransaction,
        task: &Task,
        words: &NoteWords,
        note: Option<&StoredEntry>,
        control: &Control,
    ) -> FResult<()> {
        if words.words.len() < 2 {
            return Ok(());
        }
        let superseded: BTreeSet<_> = task
            .entries
            .iter()
            .filter_map(|e| e.entry.supersedes.as_deref())
            .collect();
        for item in &task.entries {
            control.check()?;
            let e = &item.entry;
            if note.is_some_and(|n| e.id == n.entry.id) {
                break;
            }
            if !item.view.selected
                || superseded.contains(e.id.as_str())
                || note.is_some_and(|n| task.latest_review(&n.entry.id, &e.id).is_some())
            {
                continue;
            }
            if e.kind == Kind::Observation && item.sha256 == words.digest {
                continue;
            }
            let Some(kind) = prior_kind(e) else {
                continue;
            };
            let Some((offset, quote, shared)) = words.best_passage(&e.text) else {
                continue;
            };
            let links = source_links(tx, e)?;
            if links.iter().any(|l| l["status"] != "fresh") {
                self.stale += 1;
                continue;
            }
            self.matching += 1;
            let mut source = citation(task, item, offset, quote, links);
            source["kind"] = json!(kind);
            source["shared_words"] = json!(shared);
            source["feature"] = json!(task.title);
            self.best
                .push((shared.len(), kind != PriorKind::AttributedAccount, source));
            self.best
                .sort_by_key(|(score, grounded, _)| std::cmp::Reverse((*score, *grounded)));
            self.best.truncate(2);
        }
        Ok(())
    }

    fn result(self) -> Value {
        let candidates: Vec<_> = self.best.into_iter().map(|(_, _, value)| value).collect();
        // The passages immediately above the question carry the exact words
        // and byte ranges. Multiple leads can support the same interpretation.
        let clarification = match candidates.as_slice() {
            [first, second] if first["source"] != second["source"] => Some(format!(
                "Does the note affect the passage from {}, the one from {}, or both?",
                first["source"].as_str().unwrap_or_default(),
                second["source"].as_str().unwrap_or_default()
            )),
            [_, _] => Some(
                "Which of these two passages does the note affect, or does it apply to both?"
                    .into(),
            ),
            _ => None,
        };
        json!({"basis":"shared_word_stems","candidates":candidates,"matching":self.matching,
               "stale_omitted":self.stale,"clarification":clarification})
    }
}

impl Engine {
    pub(super) fn find_decisions(
        &self,
        workspace: &str,
        note: &str,
        control: &Control,
    ) -> FResult<Value> {
        text(note, QUERY_BYTES)?;
        let tx = self.db.begin_read()?;
        let tasks = tx.open_table(TASKS)?;
        let words = NoteWords::new(note);
        let mut matches = Suggestions::default();
        for row in tasks.iter()? {
            control.check()?;
            let (key, value) = row?;
            let task = decode(value.value(), key.value(), workspace)?;
            matches.collect(&tx, &task, &words, None, control)?;
        }
        let mut result = matches.result();
        result["workspace_id"] = json!(workspace);
        Ok(result)
    }
}

impl Task {
    fn latest_review(&self, note: &str, prior: &str) -> Option<&ConnectionReview> {
        self.connection_reviews
            .iter()
            .rev()
            .find(|r| r.entry_id == note && r.prior_id == prior)
    }

    pub(super) fn accepted_priors(
        &self,
        tx: &redb::ReadTransaction,
        note: &str,
    ) -> FResult<Vec<&str>> {
        let mut seen = BTreeSet::new();
        let mut accepted = Vec::new();
        for r in self
            .connection_reviews
            .iter()
            .rev()
            .filter(|r| r.entry_id == note && seen.insert(&r.prior_id))
        {
            if r.disposition == Disposition::Accept
                && !self.superseded(&r.prior_id)
                && let Some(prior) = self
                    .entries
                    .iter()
                    .find(|e| e.entry.id == r.prior_id && e.view.selected)
                && source_links(tx, &prior.entry)?
                    .iter()
                    .all(|l| l["status"] == "fresh")
            {
                accepted.push(r.prior_id.as_str());
            }
        }
        Ok(accepted)
    }

    fn note(&self, id: &str) -> FResult<(usize, &StoredEntry)> {
        let (i, item) = self
            .entries
            .iter()
            .enumerate()
            .find(|(_, e)| e.entry.id == id)
            .ok_or(FoundryError::NotFound)?;
        if !item.entry.is_captured_source() {
            return Err(invalid("connections requires a captured note"));
        }
        Ok((i, item))
    }

    pub(super) fn connections(
        &self,
        tx: &redb::ReadTransaction,
        id: &str,
        control: &Control,
    ) -> FResult<Value> {
        let (_, note) = self.note(id)?;
        let links = source_links(tx, &note.entry)?;
        let historical = self.superseded(id);
        let mut matches = Suggestions::default();
        if !historical && note.view.selected && links.iter().all(|l| l["status"] == "fresh") {
            matches.collect(
                tx,
                self,
                &NoteWords::new(&note.entry.text),
                Some(note),
                control,
            )?;
        }
        let mut result = matches.result();
        let body = prefix(&note.entry.text, QUERY_BYTES);
        result["note"] = citation(self, note, 0, body, links);
        result["note"]["original_bytes"] = json!(note.entry.text.len());
        result["note"]["historical"] = json!(historical);
        result["reviews"] = json!(
            self.connection_reviews
                .iter()
                .filter(|r| r.entry_id == id)
                .collect::<Vec<_>>()
        );
        let mut seen = BTreeSet::new();
        let mut reviewed = Vec::new();
        for review in self
            .connection_reviews
            .iter()
            .rev()
            .filter(|r| r.entry_id == id && seen.insert(&r.prior_id))
            .take(2)
        {
            let prior = self
                .entries
                .iter()
                .find(|e| e.entry.id == review.prior_id)
                .ok_or(FoundryError::NotFound)?;
            let [start, end] = review.prior_span;
            // decode validated this immutable original and its reviewed range.
            let quote = &prior.entry.text[start..end];
            let prior_citation =
                citation(self, prior, start, quote, source_links(tx, &prior.entry)?);
            reviewed.push(json!({"review":review,"prior":prior_citation,"historical":self.superseded(&review.prior_id)}));
        }
        result["reviewed_passages"] = json!(reviewed);
        Ok(result)
    }

    pub(super) fn validate_connection_reviews(&self) -> FResult<()> {
        if self.connection_reviews.len() > MAX_REVIEWS {
            return Err(invalid("too many connection reviews"));
        }
        if self.connection_reviews.is_empty() {
            return Ok(());
        }
        let entries: BTreeMap<_, _> = self
            .entries
            .iter()
            .enumerate()
            .map(|(i, e)| (e.entry.id.as_str(), (i, prior_kind(&e.entry))))
            .collect();
        let mut last = 0;
        for r in &self.connection_reviews {
            text(&r.author, 256)?;
            let sources = entries
                .get(r.entry_id.as_str())
                .zip(entries.get(r.prior_id.as_str()));
            let valid_pair = sources.is_some_and(|((n, note), (p, prior))| {
                p < n
                    && matches!(
                        note,
                        Some(PriorKind::IndexedPassage | PriorKind::AttributedAccount)
                    )
                    && prior.is_some()
                    && r.prior_span[0] < r.prior_span[1]
                    && r.prior_span[1] - r.prior_span[0] <= PASSAGE_BYTES
                    && self.entries[*p]
                        .entry
                        .text
                        .get(r.prior_span[0]..r.prior_span[1])
                        .is_some()
            });
            if r.revision <= last || r.revision > self.revision || !valid_pair {
                return Err(invalid("invalid connection review"));
            }
            last = r.revision;
        }
        Ok(())
    }

    pub(super) fn record_connection(
        &mut self,
        tx: &redb::WriteTransaction,
        choice: &ConnectionChoice,
        revision: u64,
    ) -> FResult<bool> {
        let id = choice.entry_id.as_str();
        let prior = choice.prior_id.as_str();
        text(&choice.author, 256)?;
        let (i, note) = self.note(id)?;
        let earlier = self.entries[..i]
            .iter()
            .find(|e| e.entry.id == prior)
            .ok_or_else(|| invalid("prior_id must identify an earlier entry in this feature"))?;
        if self.superseded(id)
            || self.superseded(prior)
            || !note.view.selected
            || !earlier.view.selected
            || (earlier.entry.kind == Kind::Observation && earlier.sha256 == note.sha256)
            || prior_kind(&earlier.entry).is_none()
        {
            return Err(invalid("review requires two current related entries"));
        }
        let previous = self.latest_review(id, prior);
        if previous.is_none_or(|r| r.prior_span != choice.prior_span) {
            let current_span = NoteWords::new(&note.entry.text)
                .best_passage(&earlier.entry.text)
                .map(|(start, quote, _)| [start, start + quote.len()]);
            if current_span != Some(choice.prior_span) {
                return Err(FoundryError::Conflict(
                    "the cited passage differs; read connections again".into(),
                ));
            }
        }
        for h in note
            .entry
            .source_links
            .iter()
            .chain(&earlier.entry.source_links)
        {
            store::validate_link_span(tx, &self.workspace_id, h)?;
        }
        if previous.is_some_and(|r| {
            r.author == choice.author
                && r.disposition == choice.disposition
                && r.prior_span == choice.prior_span
        }) {
            return Ok(false);
        }
        if self.connection_reviews.len() == MAX_REVIEWS {
            return Err(invalid("task has 128 connection reviews"));
        }
        self.connection_reviews.push(ConnectionReview {
            entry_id: id.into(),
            prior_id: prior.into(),
            prior_span: choice.prior_span,
            disposition: choice.disposition,
            author: choice.author.clone(),
            revision,
        });
        Ok(true)
    }
}
