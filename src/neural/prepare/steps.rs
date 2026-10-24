//! The shared preparation steps (009 T003) that the CLI run and the MCP driver
//! compose: the partition CPU workers, partition and card selection, rendering
//! and the batch and partition commits, with their tests. Scheduling and the
//! CLI run are siblings.
use super::*;

/// Lazily started CPU workers, reused across waves (and MCP prepare runs).
/// No store, profile or source is retained here. `install` joins each wave,
/// including errors/panics, before its borrowed inputs can leave the caller.
pub(crate) struct PartitionWorkers {
    count: usize,
    pool: OnceLock<Result<rayon::ThreadPool, String>>,
}

impl Default for PartitionWorkers {
    fn default() -> Self {
        Self::new(std::thread::available_parallelism().map_or(1, usize::from))
    }
}

#[derive(Debug)]
pub(crate) struct ComputedPartition {
    pub partition: cache::PreparedPartition,
    pub retained: Option<PreparedCards>,
}

/// Only produced by rendering a verified PartitionSource. Never accepted
/// from persisted or caller-supplied token ids. Selection checks source,
/// recipe and function identity; rendering still compares every stored tuple.
#[derive(Debug)]
pub(crate) struct PreparedCards {
    path: String,
    hash: String,
    recipe: String,
    function_digest: String,
    cards: Vec<partition::Card>,
}

impl PartitionWorkers {
    fn new(count: usize) -> Self {
        Self {
            count: count.clamp(1, PARTITION_WAVE),
            pool: OnceLock::new(),
        }
    }

    pub(crate) fn prepare(
        &self,
        cards: Cards<'_>,
        sources: Vec<cache::PartitionSource>,
        recipe: &str,
        control: &Control,
    ) -> FResult<Vec<ComputedPartition>> {
        if sources.len() > PARTITION_WAVE {
            return Err(partition_invalid(
                "partition wave exceeds its source bound".into(),
            ));
        }
        if sources.is_empty() {
            return Ok(Vec::new());
        }
        control.check()?;
        prepare_fault!(PARTITION_BEFORE_COMPUTE, Some(control), "")?;
        let pool = self
            .pool
            .get_or_init(|| {
                rayon::ThreadPoolBuilder::new()
                    .num_threads(self.count)
                    .thread_name(|index| format!("foundry-partition-{index}"))
                    .build()
                    .map_err(|error| error.to_string())
            })
            .as_ref()
            .map_err(|message| FoundryError::Semantic {
                code: "partition_worker_failed",
                message: format!("cannot start partition workers: {message}"),
            })?;
        // Indexed parallel collection preserves source order. Collect every
        // result before propagating errors so all borrowed work has finished.
        let results: Vec<_> = pool.install(|| {
            sources
                .into_par_iter()
                .map(|source| {
                    control.check()?;
                    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        cards.prepare_source(source, recipe, RETAINED_CARD_BYTES)
                    })) {
                        Ok(result) => result,
                        Err(_) => Err(FoundryError::Semantic {
                            code: "partition_worker_failed",
                            message:
                                "a partition worker panicked; unpublished mappings were discarded"
                                    .into(),
                        }),
                    }
                })
                .collect()
        });
        results.into_iter().collect()
    }
}

// ---------------------------------------------------------------------------
// Shared steps (009 T003): the CLI above and the MCP driver compose these.
// ---------------------------------------------------------------------------

/// Count one admitted document call and its exact input tokens.
pub(crate) fn count_document_call(report: &mut PrepareReport, inputs: &[TokenizedInput]) {
    report.document_calls += 1;
    report.input_tokens += inputs
        .iter()
        .map(|input| input.ids.len() as u64)
        .sum::<u64>();
}

/// Validate and commit one batch's vectors: one vector per input, each of
/// the profile's exact dimension with finite values, then ONE cache
/// transaction (the committed counts move with it), before any publication.
/// A malformed reply commits nothing and stops `provider_malformed`; the
/// disk cap stops `cache_full` without evicting. Vectors are keyed by exact
/// rendered input: a late result for a source edited or deleted since its
/// selection may populate the cache, but eligibility is always recomputed
/// from the CURRENT sources, so it never restores stale eligibility.
/// (`control` reaches only the test-faults commit points.)
#[cfg_attr(not(feature = "test-faults"), allow(unused_variables))]
#[allow(clippy::too_many_arguments)]
pub(crate) fn commit_batch(
    engine: &Engine,
    keys: Vec<String>,
    vectors: Vec<Vec<f32>>,
    function_digest: &str,
    dims: usize,
    cap_bytes: u64,
    control: &Control,
    report: &mut PrepareReport,
) -> Result<(), StopOrError> {
    if vectors.len() != keys.len() {
        report.provider_state = Some("failed");
        report.provider_code = Some("provider_malformed");
        return Err(StopOrError::Stop(Stop::partial(
            "provider_malformed",
            format!(
                "the provider returned {} vectors for {} inputs",
                vectors.len(),
                keys.len()
            ),
        )));
    }
    for vector in &vectors {
        if let Err(e) = provider::validate_vector(vector, dims) {
            // The batch is NOT committed: valid data stays intact.
            report.provider_state = Some("failed");
            report.provider_code = Some("provider_malformed");
            return Err(StopOrError::Stop(Stop::partial(
                "provider_malformed",
                e.to_string(),
            )));
        }
    }
    let entries: Vec<(String, Vec<f32>)> = keys.into_iter().zip(vectors).collect();
    prepare_fault!(
        CACHE_BEFORE_COMMIT,
        Some(control),
        &entries.len().to_string()
    )
    .map_err(StopOrError::Error)?;
    if let Err(e) = engine.semantic_cache_commit(&entries, function_digest, dims, cap_bytes) {
        return Err(match e.code() {
            "cache_full" => StopOrError::Stop(Stop::partial(
                "cache_full",
                "the workspace cache reached its disk cap; nothing was evicted",
            )),
            _ => StopOrError::Error(e),
        });
    }
    // Commit-adjacent fault point: an exit here leaves the batch committed.
    prepare_fault!(
        CACHE_AFTER_COMMIT,
        Some(control),
        &entries.len().to_string()
    )
    .map_err(StopOrError::Error)?;
    Ok(())
}

/// The fixed inputs of one run's store steps. The CLI builds it once; the
/// MCP driver builds it inside each engine-slot hold.
pub(crate) struct Steps<'a> {
    pub engine: &'a Engine,
    pub cards: Cards<'a>,
    pub recipe: &'a str,
}

/// What card rendering needs, and nothing of the store: the profile, its
/// tokenizer and its function digest. [`Cards::render`] runs with no engine
/// slot and no transaction.
#[derive(Clone, Copy)]
pub(crate) struct Cards<'a> {
    pub tokenizer: &'a (dyn TokenCount + Sync),
    pub profile: &'a SemanticProfile,
    pub function_digest: &'a str,
}

/// What one partition step did.
pub(crate) enum Progress {
    /// Sources remain after `after`.
    More,
    /// Every current source has been examined.
    Done,
    Halted(Stop),
}

/// What one selection step did to the caller's [`Selection`].
pub(crate) enum Selected {
    /// The selection holds the cards asked for; the walk continues after it.
    Full,
    /// The step examined its source bound; the walk (and the selection)
    /// continue.
    Yield,
    /// The walk is over; the selection (possibly empty) is the last one.
    End,
    Halted(Stop),
}

/// The embed walk's position: every source up to `after` is done, `partial`
/// is the source version in progress and its next card. `seen` deduplicates
/// pending inputs; callers replace it at batch boundaries, and accepted
/// inputs deduplicate through the durable cache.
#[derive(Default)]
pub(crate) struct Walk {
    pub(crate) exact: bool,
    pub(crate) overview_first: bool,
    after: Option<String>,
    scope: Option<String>,
    through: Option<String>,
    partial: Option<(Rc<SourceVersion>, usize)>,
    position: Option<CardPosition>,
    pub(super) seen: HashSet<String>,
    retained: Vec<PreparedCards>,
}
impl Walk {
    pub(crate) fn with_retained(mut self, retained: Vec<PreparedCards>) -> Self {
        self.retained = retained;
        self
    }

    /// Keep only scalar progress in the cursor. Parsed hierarchy state may be
    /// kept under the one global budget; a miss changes work, never inputs.
    pub(crate) fn pause(self, paused: &mut PausedSources) -> WalkPosition {
        let partial = self
            .partial
            .map(|(source, next)| {
                let position = CardPosition {
                    path: source.path.clone(),
                    hash: source.hash.clone(),
                    recipe: source.recipe.clone(),
                    function_digest: source.function_digest.clone(),
                    next,
                };
                paused.insert(source);
                position
            })
            .or(self.position);
        WalkPosition {
            after: self.after,
            partial,
        }
    }

    pub(crate) fn restore(mut self, position: WalkPosition, paused: &mut PausedSources) -> Self {
        self.after = position.after;
        if let Some(position) = position.partial {
            if let Some(source) = paused.take(&position) {
                self.partial = Some((source, position.next));
            } else {
                self.position = Some(position);
            }
        }
        self
    }

    pub(crate) fn exclude(&mut self, keys: impl IntoIterator<Item = String>) {
        self.seen.clear();
        self.seen.extend(keys);
    }

    pub(crate) fn wave(
        scope: Option<String>,
        after: Option<String>,
        through: Option<String>,
    ) -> Self {
        Self {
            scope,
            after,
            through,
            ..Self::default()
        }
    }
}

/// A queued cursor owns no source body, parsed plan or token array.
#[derive(Clone)]
pub(crate) struct WalkPosition {
    pub(crate) after: Option<String>,
    partial: Option<CardPosition>,
}

#[derive(Clone)]
struct CardPosition {
    path: String,
    hash: String,
    recipe: String,
    function_digest: String,
    next: usize,
}

impl CardPosition {
    fn matches(&self, path: &str, hash: &str, recipe: &str, function: &str) -> bool {
        self.path == path
            && self.hash == hash
            && self.recipe == recipe
            && self.function_digest == function
    }
}

/// Shared by every cursor in one CLI run / MCP pass. Entries are consumed on
/// resume, so an active walk is not simultaneously charged as paused. The
/// fixed queue allocation and all retained payload capacities fit `limit`.
/// Cancellation drops this local state; accepted inputs stay in durable cache.
pub(crate) struct PausedSources {
    entries: VecDeque<(usize, Rc<SourceVersion>)>,
    bytes: usize,
    limit: usize,
}

impl Default for PausedSources {
    fn default() -> Self {
        Self::new(PAUSED_PLAN_BYTES)
    }
}

impl PausedSources {
    fn new(limit: usize) -> Self {
        let metadata = PAUSED_PLAN_SLOTS * std::mem::size_of::<(usize, Rc<SourceVersion>)>();
        let entries = if metadata <= limit {
            VecDeque::with_capacity(PAUSED_PLAN_SLOTS)
        } else {
            VecDeque::new()
        };
        let bytes = entries.capacity() * std::mem::size_of::<(usize, Rc<SourceVersion>)>();
        if bytes > limit {
            return Self {
                entries: VecDeque::new(),
                bytes: 0,
                limit,
            };
        }
        Self {
            entries,
            bytes,
            limit,
        }
    }

    fn remove(&mut self, path: &str) -> Option<Rc<SourceVersion>> {
        let index = self
            .entries
            .iter()
            .position(|(_, source)| source.path == path)?;
        let (bytes, source) = self.entries.remove(index).expect("known paused source");
        self.bytes -= bytes;
        Some(source)
    }

    fn insert(&mut self, source: Rc<SourceVersion>) {
        self.remove(&source.path); // Same-path replacement invalidates old identity.
        let Some(bytes) = source.paused_bytes() else {
            return;
        };
        let metadata = self.entries.capacity() * std::mem::size_of::<(usize, Rc<SourceVersion>)>();
        if self.entries.capacity() == 0 || bytes > self.limit.saturating_sub(metadata) {
            return;
        }
        while self.entries.len() == self.entries.capacity()
            || self.bytes.saturating_add(bytes) > self.limit
        {
            let (evicted, _) = self.entries.pop_front().expect("bounded paused cache");
            self.bytes -= evicted;
        }
        self.entries.push_back((bytes, source));
        self.bytes += bytes;
    }

    fn take(&mut self, position: &CardPosition) -> Option<Rc<SourceVersion>> {
        self.remove(&position.path).filter(|source| {
            source.matches(
                &position.path,
                &position.hash,
                &position.recipe,
                &position.function_digest,
            )
        })
    }
}

/// One verified source version: either fresh prepared cards or its body
/// and lazily parsed units. A partially consumed source carries this input
/// across batches; unchanged fresh inputs need no second body read or parse.
struct SourceVersion {
    path: String,
    hash: String,
    recipe: String,
    function_digest: String,
    content: SourceCards,
    paused_size: OnceCell<usize>,
}

impl SourceVersion {
    fn matches(&self, path: &str, hash: &str, recipe: &str, function: &str) -> bool {
        self.path == path
            && self.hash == hash
            && self.recipe == recipe
            && self.function_digest == function
    }

    fn paused_bytes(&self) -> Option<usize> {
        if let Some(bytes) = self.paused_size.get() {
            return Some(*bytes);
        }
        // Only parsed hierarchy plans are retained across turns. Fresh card
        // token arrays and other representations use scalar progress alone.
        let SourceCards::Body { body, units } = &self.content else {
            return None;
        };
        let SourcePlan::Hierarchy(spans) = units.get()? else {
            return None;
        };
        let mut bytes = std::mem::size_of::<Self>() + 2 * std::mem::size_of::<usize>(); // Rc header
        for capacity in [
            self.path.capacity(),
            self.hash.capacity(),
            self.recipe.capacity(),
            self.function_digest.capacity(),
            body.capacity(),
        ] {
            bytes = bytes.saturating_add(capacity);
        }
        bytes = bytes.saturating_add(
            spans
                .capacity()
                .saturating_mul(std::mem::size_of::<partition::hierarchy::Span>()),
        );
        for span in spans {
            bytes = bytes.saturating_add(span.retained_heap_bytes());
        }
        let _ = self.paused_size.set(bytes);
        Some(bytes)
    }
}

enum SourceCards {
    Body {
        body: String,
        units: OnceCell<SourcePlan>,
    },
    Prepared(PreparedCards),
}

enum SourcePlan {
    Address(Vec<Unit>),
    Body(Vec<partition::body::Span>),
    Hierarchy(Vec<partition::hierarchy::Span>),
}

impl SourcePlan {
    /// Plan one source under the profile's representation. With `stream`,
    /// every card renders once, in order, into it and no plan is retained
    /// (`None`); Body streams the cards its planning already tokenized.
    /// Without, the plan is returned for [`Self::render_at`].
    fn build(
        cards: &Cards<'_>,
        body: &str,
        path: &str,
        mut stream: Option<&mut Collected>,
    ) -> FResult<Option<Self>> {
        let (recipe, tokens) = (cards.recipe(), cards.tokenizer);
        let lang = Lang::from_path(path);
        let plan = match cards.profile.representation {
            partition::Representation::Address => {
                let units = partition::carded_units(body, lang);
                if let Some(collected) = &mut stream {
                    collected.reserve(units.len());
                }
                Self::Address(units)
            }
            partition::Representation::Body { parent_tokens } => {
                let mut spans = Vec::new();
                partition::body::visit(
                    body,
                    lang,
                    &recipe,
                    parent_tokens as usize,
                    tokens,
                    |span, card| match &mut stream {
                        Some(collected) => collected.accept(card),
                        None => spans.push(span),
                    },
                )?;
                if stream.is_some() {
                    return Ok(None);
                }
                Self::Body(spans)
            }
            partition::Representation::Hierarchy { parent_tokens } => {
                let parent = parent_tokens as usize;
                let spans = partition::hierarchy::plan(body, lang, &recipe, parent, tokens)?;
                Self::Hierarchy(spans)
            }
            partition::Representation::Overview => {
                let spans = partition::hierarchy::file_plan(body, lang, &recipe, tokens)?;
                Self::Hierarchy(spans)
            }
        };
        let Some(collected) = stream else {
            return Ok(Some(plan));
        };
        for index in 0..plan.len() {
            collected.accept(plan.render_at(index, body, path, &recipe, tokens)?);
        }
        Ok(None)
    }

    fn len(&self) -> usize {
        match self {
            Self::Address(units) => units.len(),
            Self::Body(spans) => spans.len(),
            Self::Hierarchy(spans) => spans.len(),
        }
    }

    /// Render card `index` of the plan alone.
    fn render_at(
        &self,
        index: usize,
        body: &str,
        path: &str,
        recipe: &CardRecipe<'_>,
        tokens: &dyn TokenCount,
    ) -> Result<partition::Card, ProviderError> {
        match self {
            Self::Address(units) => partition::card(body, path, &units[index], recipe, tokens),
            Self::Body(spans) => partition::body::card(body, &spans[index], recipe, tokens),
            Self::Hierarchy(spans) => {
                partition::hierarchy::card(body, &spans[index], recipe, tokens)
            }
        }
    }
}

/// The partition collection [`SourcePlan::build`] streams into: every card's
/// tuple and, while within `limit` bytes, the cards themselves for retention.
struct Collected {
    tuples: Vec<PartitionUnit>,
    kept: Option<Vec<partition::Card>>,
    kept_bytes: usize,
    limit: usize,
}

impl Collected {
    fn new(kept_bytes: usize, limit: usize) -> Self {
        Self {
            tuples: Vec::new(),
            kept: (kept_bytes <= limit).then(Vec::new),
            kept_bytes,
            limit,
        }
    }

    /// Size retention for a card count known before the first card.
    fn reserve(&mut self, cards: usize) {
        self.kept_bytes += cards * std::mem::size_of::<partition::Card>();
        self.kept = (self.kept_bytes <= self.limit).then(|| Vec::with_capacity(cards));
    }

    fn accept(&mut self, card: partition::Card) {
        self.tuples.push(PartitionUnit {
            kind: card.kind,
            start: card.start,
            end: card.end,
            input_key: card.input_key.clone(),
        });
        if let Some(cards) = &mut self.kept {
            let capacity = cards.capacity();
            self.kept_bytes = self
                .kept_bytes
                .saturating_add(card.input_key.capacity())
                .saturating_add(
                    card.ids
                        .capacity()
                        .saturating_mul(std::mem::size_of::<u32>()),
                );
            if self.kept_bytes <= self.limit {
                cards.push(card);
                self.kept_bytes = self.kept_bytes.saturating_add(
                    (cards.capacity() - capacity) * std::mem::size_of::<partition::Card>(),
                );
            }
            if self.kept_bytes > self.limit {
                self.kept = None;
            }
        }
    }
}

/// Cards a selection step found missing, not yet rendered: per source
/// version, how many cards its partition records and the missing ones
/// (index and recorded tuple), in walk order. Rendering
/// ([`Cards::render`]) needs no engine and no transaction.
#[derive(Default)]
pub(crate) struct Selection {
    sources: Vec<SelectedSource>,
    len: usize,
}

struct SelectedSource {
    source: Rc<SourceVersion>,
    cards: usize,
    wanted: Vec<(usize, PartitionUnit)>,
}

impl Selection {
    pub(crate) fn len(&self) -> usize {
        self.len
    }
}

/// One document batch being formed: cache keys and the exact model inputs.
#[derive(Default)]
pub(crate) struct Batch {
    pub keys: Vec<String>,
    pub inputs: Vec<TokenizedInput>,
}

impl Batch {
    pub(crate) fn take(&mut self, limits: DocumentLimits, inputs: usize) -> FResult<Self> {
        let mut total = 0usize;
        let mut count = 0;
        for input in self.inputs.iter().take(inputs.min(limits.inputs)) {
            if input.ids.is_empty()
                || input.ids.len() > limits.tokens
                || input.ids.len() > limits.total_tokens
            {
                return Err(ProviderError::InputTooLarge(
                    "rendered input exceeds the profile geometry".into(),
                )
                .into());
            }
            if total + input.ids.len() > limits.total_tokens {
                break;
            }
            total += input.ids.len();
            count += 1;
        }
        Ok(Self {
            keys: self.keys.drain(..count).collect(),
            inputs: self.inputs.drain(..count).collect(),
        })
    }

    pub(crate) fn len(&self) -> usize {
        self.keys.len()
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.keys.is_empty()
    }
}

fn partition_invalid(message: String) -> FoundryError {
    FoundryError::Semantic {
        code: "partition_invalid",
        message,
    }
}

impl Cards<'_> {
    #[cfg(test)]
    fn prepare_partitions(
        &self,
        sources: Vec<cache::PartitionSource>,
        recipe: &str,
        control: &Control,
    ) -> FResult<Vec<cache::PreparedPartition>> {
        self.prepare_partitions_with_workers(sources, recipe, control, PARTITION_WAVE)
    }

    #[cfg(test)]
    fn prepare_partitions_with_workers(
        &self,
        sources: Vec<cache::PartitionSource>,
        recipe: &str,
        control: &Control,
        workers: usize,
    ) -> FResult<Vec<cache::PreparedPartition>> {
        Ok(PartitionWorkers::new(workers)
            .prepare(*self, sources, recipe, control)?
            .into_iter()
            .map(|computed| computed.partition)
            .collect())
    }

    fn prepare_source(
        &self,
        source: cache::PartitionSource,
        recipe: &str,
        retain_limit: usize,
    ) -> FResult<ComputedPartition> {
        let path = source.path().to_owned();
        let hash = source.source_hash().to_owned();
        let mut retained = None;
        let partition = source.prepare(recipe, self.function_digest, None, |body| {
            let mut collected = Collected::new(
                std::mem::size_of::<PreparedCards>()
                    + path.capacity()
                    + hash.capacity()
                    + recipe.len()
                    + self.function_digest.len(),
                retain_limit,
            );
            SourcePlan::build(self, body, &path, Some(&mut collected))?;
            retained = collected.kept;
            Ok(collected.tuples)
        })?;
        Ok(ComputedPartition {
            partition,
            retained: retained.map(|cards| PreparedCards {
                path,
                hash,
                recipe: recipe.to_owned(),
                function_digest: self.function_digest.to_owned(),
                cards,
            }),
        })
    }

    /// The limits of one document call under the run's profile.
    pub(crate) fn limits(&self) -> DocumentLimits {
        self.profile.document_limits()
    }

    /// The profile's output dimension.
    pub(crate) fn dims(&self) -> usize {
        self.profile.descriptor.dims()
    }

    fn recipe(&self) -> CardRecipe<'_> {
        CardRecipe {
            template: &self.profile.descriptor.document_template,
            function_digest: self.function_digest,
            card_tokens: self.profile.card_tokens as usize,
        }
    }

    /// Form the selected batch without an engine or transaction. Reuse
    /// privately prepared cards when available; otherwise render only the
    /// selected units. Both paths require each recorded range/key to match.
    pub(crate) fn render(&self, selection: Selection) -> FResult<Batch> {
        let recipe = self.recipe();
        let mut batch = Batch::default();
        for selected in selection.sources {
            let source = &selected.source;
            if source.recipe != self.profile.recipe_id()
                || source.function_digest != self.function_digest
            {
                return Err(partition_invalid("resumed source identity changed".into()));
            }
            let count = match &source.content {
                SourceCards::Body { body, units } => {
                    if units.get().is_none()
                        && let Some(plan) = SourcePlan::build(self, body, &source.path, None)?
                    {
                        let _ = units.set(plan);
                    }
                    units.get().expect("planned source").len()
                }
                SourceCards::Prepared(prepared) => {
                    if prepared.function_digest != self.function_digest
                        || prepared.recipe != self.profile.recipe_id()
                    {
                        return Err(partition_invalid("retained card identity changed".into()));
                    }
                    prepared.cards.len()
                }
            };
            if count != selected.cards {
                return Err(partition_invalid(format!(
                    "{}: the partition records {} cards, but the body has {} carded units",
                    source.path, selected.cards, count
                )));
            }
            for (index, recorded) in selected.wanted {
                let card = match &source.content {
                    SourceCards::Prepared(prepared) => prepared.cards[index].clone(),
                    SourceCards::Body { body, units } => {
                        let plan = units.get().expect("planned source");
                        plan.render_at(index, body, &source.path, &recipe, self.tokenizer)?
                    }
                };
                if (card.kind, card.start, card.end, card.input_key.as_str())
                    != (
                        recorded.kind,
                        recorded.start,
                        recorded.end,
                        recorded.input_key.as_str(),
                    )
                {
                    return Err(partition_invalid(format!(
                        "{}: card {index} is recorded as {}..{}, but renders as {}..{} under \
                         {} key",
                        source.path,
                        recorded.start,
                        recorded.end,
                        card.start,
                        card.end,
                        if card.input_key == recorded.input_key {
                            "the same"
                        } else {
                            "another"
                        }
                    )));
                }
                batch.keys.push(card.input_key);
                batch.inputs.push(TokenizedInput { ids: card.ids });
            }
        }
        Ok(batch)
    }
}

impl Steps<'_> {
    /// CLI composition of snapshot, parallel computation and ordered commits.
    /// Select at most min(max_new, eight) new sources, respecting `until` at
    /// source boundaries. Recheck cancellation and the source version before
    /// each atomic commit. MCP composes these stages around separate slot holds.
    #[cfg(test)]
    pub(crate) fn partition_page(
        &self,
        after: &mut Option<String>,
        max_new: usize,
        until: Option<Instant>,
        control: &Control,
        report: &mut PrepareReport,
        halt: &mut dyn FnMut() -> Option<Stop>,
    ) -> FResult<Progress> {
        let (progress, sources) = self.partition_inputs(after, max_new, until, report, halt)?;
        if matches!(progress, Progress::Halted(_)) {
            return Ok(progress);
        }
        let prepared = self
            .cards
            .prepare_partitions(sources, self.recipe, control)?;
        for partition in prepared {
            if let Some(stop) = halt() {
                return Ok(Progress::Halted(stop));
            }
            commit_partition(self.engine, partition, control, report)?.committed()?;
        }
        Ok(progress)
    }

    /// Snapshot only. Card parsing/tokenization happens after the caller
    /// releases its engine slot. Source order also determines commit order.
    #[cfg(test)]
    pub(crate) fn partition_inputs(
        &self,
        after: &mut Option<String>,
        max_new: usize,
        until: Option<Instant>,
        report: &mut PrepareReport,
        halt: &mut dyn FnMut() -> Option<Stop>,
    ) -> FResult<(Progress, Vec<cache::PartitionSource>)> {
        self.partition_inputs_in(after, None, false, max_new, until, report, halt)
    }

    fn source_page(
        &self,
        after: Option<&str>,
        scope: Option<&str>,
        exact: bool,
    ) -> FResult<Vec<(String, crate::store::SourceMeta)>> {
        if !exact {
            return cache::source_page_in(&self.engine.db, after, scope);
        }
        if after.is_some() {
            return Ok(Vec::new());
        }
        let path = scope.expect("exact source cursor has a path");
        Ok(self
            .engine
            .source(path)?
            .map(|meta| vec![(path.to_owned(), meta)])
            .unwrap_or_default())
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn partition_inputs_in(
        &self,
        after: &mut Option<String>,
        scope: Option<&str>,
        exact: bool,
        max_new: usize,
        until: Option<Instant>,
        report: &mut PrepareReport,
        halt: &mut dyn FnMut() -> Option<Stop>,
    ) -> FResult<(Progress, Vec<cache::PartitionSource>)> {
        let mut inputs = Vec::new();
        if let Some(stop) = halt() {
            return Ok((Progress::Halted(stop), inputs));
        }
        let page = self.source_page(after.as_deref(), scope, exact)?;
        if page.is_empty() {
            return Ok((Progress::Done, inputs));
        }
        for (examined, (path, meta)) in page.iter().enumerate() {
            if inputs.len() == max_new.min(PARTITION_WAVE)
                || (examined > 0 && until.is_some_and(|until| Instant::now() >= until))
            {
                return Ok((Progress::More, inputs));
            }
            report.sources += 1;
            if let Some(existing) = self.engine.semantic_partition(path)?
                && cache::partition_is_current(
                    &existing,
                    meta,
                    self.recipe,
                    self.cards.function_digest,
                )
            {
                report.reused_partitions += 1;
                *after = Some(path.clone());
                continue;
            }
            if let Some(stop) = halt() {
                return Ok((Progress::Halted(stop), inputs));
            }
            inputs.push(self.engine.semantic_partition_source(path, &meta.hash)?);
            *after = Some(path.clone());
        }
        Ok((Progress::More, inputs))
    }

    /// Add the next missing cards to `selection` (a store step: no
    /// rendering, no tokenization): the walk visits the cards of every
    /// source with a CURRENT partition in path order, skips inputs this run
    /// resolved or the cache holds valid, and records each missing one (its
    /// index and recorded tuple) with its source version's verified body,
    /// until the selection holds `room` cards, the walk ends, or
    /// `max_sources` sources were examined. Pages are read fresh at every
    /// step; the version the walk paused in keeps its body (and parsed
    /// units) for the next step, and a source whose version changed since
    /// restarts at its first card. `halt` is asked before every page.
    pub(crate) fn select_batch(
        &self,
        walk: &mut Walk,
        selection: &mut Selection,
        room: usize,
        max_sources: usize,
        report: &mut PrepareReport,
        halt: &mut dyn FnMut() -> Option<Stop>,
    ) -> FResult<Selected> {
        let dims = self.cards.dims();
        let mut examined = 0usize;
        loop {
            if let Some(stop) = halt() {
                return Ok(Selected::Halted(stop));
            }
            if selection.len >= room {
                return Ok(Selected::Full);
            }
            let page =
                self.source_page(walk.after.as_deref(), walk.scope.as_deref(), walk.exact)?;
            if page.is_empty() {
                return Ok(Selected::End);
            }
            for (path, meta) in &page {
                if walk.through.as_ref().is_some_and(|end| path > end) {
                    return Ok(Selected::End);
                }
                if examined == max_sources {
                    return Ok(Selected::Yield);
                }
                examined += 1;
                let resumed = walk.partial.take().filter(|(source, _)| {
                    source.matches(path, &meta.hash, self.recipe, self.cards.function_digest)
                });
                let position = walk.position.take().filter(|position| {
                    position.matches(path, &meta.hash, self.recipe, self.cards.function_digest)
                });
                let first = resumed.as_ref().map_or_else(
                    || position.as_ref().map_or(0, |position| position.next),
                    |(_, next)| *next,
                );
                if let Some(record) = self.engine.semantic_partition(path)?
                    && cache::partition_is_current(
                        &record,
                        meta,
                        self.recipe,
                        self.cards.function_digest,
                    )
                    && first < record.units.len()
                {
                    let mut wanted = Vec::new();
                    let mut full_at = None;
                    let remaining = if walk.overview_first {
                        1usize.saturating_sub(first)
                    } else {
                        usize::MAX
                    };
                    for (index, unit) in record.units.iter().enumerate().skip(first).take(remaining)
                    {
                        report.eligible_units += 1;
                        if walk.seen.contains(&unit.input_key) {
                            // Identical rendered inputs share this run's result.
                            report.reused_cached_units += 1;
                            continue;
                        }
                        // The servable check scans the payload as the serving
                        // lookup does: a same-length nonfinite (or otherwise
                        // tampered) row is disabled by name here and replaced
                        // by a fresh embedding.
                        let needed = match self.engine.semantic_cache_servable(
                            &unit.input_key,
                            self.cards.function_digest,
                            dims,
                        )? {
                            CacheLookup::Hit(_) => false,
                            CacheLookup::Corrupt(_) => {
                                report.corrupt_cache_rows += 1;
                                true
                            }
                            CacheLookup::Miss => true,
                        };
                        if !needed {
                            report.reused_cached_units += 1;
                            continue;
                        }
                        walk.seen.insert(unit.input_key.clone());
                        wanted.push((index, unit.clone()));
                        if selection.len + wanted.len() == room {
                            full_at = Some(index + 1);
                            break;
                        }
                    }
                    if !wanted.is_empty() {
                        let source = match resumed {
                            Some((source, _)) => source,
                            None => {
                                let retained = walk.retained.iter().position(|cards| {
                                    cards.path == *path
                                        && cards.hash == meta.hash
                                        && cards.recipe == self.recipe
                                        && cards.function_digest == self.cards.function_digest
                                });
                                let content = match retained {
                                    Some(index) => {
                                        SourceCards::Prepared(walk.retained.swap_remove(index))
                                    }
                                    None => SourceCards::Body {
                                        body: self.engine.semantic_source_body(path, meta)?,
                                        units: OnceCell::new(),
                                    },
                                };
                                Rc::new(SourceVersion {
                                    path: path.clone(),
                                    hash: meta.hash.clone(),
                                    recipe: self.recipe.to_owned(),
                                    function_digest: self.cards.function_digest.to_owned(),
                                    content,
                                    paused_size: OnceCell::new(),
                                })
                            }
                        };
                        selection.len += wanted.len();
                        selection.sources.push(SelectedSource {
                            source: Rc::clone(&source),
                            cards: record.units.len(),
                            wanted,
                        });
                        if let Some(next) = full_at {
                            walk.partial = Some((source, next));
                            return Ok(Selected::Full);
                        }
                    }
                }
                walk.after = Some(path.clone());
            }
        }
    }
}

/// Publish a computed mapping only against its still-current source version.
/// Only a commit counts as a partitioned source.
#[cfg_attr(not(feature = "test-faults"), allow(unused_variables))]
pub(crate) fn commit_partition(
    engine: &Engine,
    partition: cache::PreparedPartition,
    control: &Control,
    report: &mut PrepareReport,
) -> FResult<cache::PartitionCommit> {
    control.check()?;
    let path = partition.path().to_owned();
    let commit = engine.semantic_accept_partition(partition)?;
    if let cache::PartitionCommit::Committed = commit {
        report.partitioned_sources += 1;
        prepare_fault!(PARTITION_AFTER_COMMIT, Some(control), &path)?;
    }
    Ok(commit)
}

#[cfg(all(test, feature = "test-faults"))]
mod tests {
    use super::*;

    #[test]
    fn admission_packs_actual_tokens_keeps_leftovers_and_refuses_oversize_inputs() {
        let limits = DocumentLimits {
            inputs: 8,
            tokens: 4096,
            total_tokens: 8192,
        };
        let lengths = [4000, 3900, 4000, 800, 30];
        let mut pending = Batch {
            keys: (0..lengths.len()).map(|i| i.to_string()).collect(),
            inputs: lengths
                .into_iter()
                .map(|n| TokenizedInput { ids: vec![1; n] })
                .collect(),
        };
        let first = pending.take(limits, 8).unwrap();
        assert_eq!(first.keys, ["0", "1"]);
        let second = pending.take(limits, 2).unwrap();
        assert_eq!(
            second.keys,
            ["2", "3"],
            "foreground sequence cap is independent"
        );
        assert_eq!(pending.take(limits, 8).unwrap().keys, ["4"]);
        assert!(pending.is_empty());
        for batch in [first, second] {
            provider::check_document_batch(&batch.inputs, limits).unwrap();
        }
        pending.keys.push("too-long".into());
        pending.inputs.push(TokenizedInput { ids: vec![0; 4097] });
        assert!(pending.take(limits, 8).is_err());
        assert_eq!(
            pending.len(),
            1,
            "a refused input is never silently consumed"
        );
    }

    fn partition_fixture() -> (tempfile::TempDir, Engine, SemanticProfile) {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("root");
        std::fs::create_dir(&root).unwrap();
        for n in 0..12 {
            std::fs::write(
                root.join(format!("note{n:02}.md")),
                format!("# Note {n} é\n\nRetained detail {n}.\n"),
            )
            .unwrap();
        }
        let mut engine = Engine::initialize(&dir.path().join("store"), &root).unwrap();
        engine.index(&root, &Control::unbounded()).unwrap();
        let profile = SemanticProfile::load(&crate::testkit::write_semantic_profile(
            dir.path(),
            "partitions",
            |_| {},
        ))
        .unwrap();
        (dir, engine, profile)
    }

    #[test]
    fn hierarchy_visits_one_whole_file_input_per_source_before_refining() {
        let (_dir, engine, mut profile) = partition_fixture();
        for n in 0..12 {
            engine
                .replace_source(
                    &format!("note{n:02}.md"),
                    &format!(
                        "# Different note {n}\n{}",
                        "Repeated useful detail.\n".repeat(150)
                    ),
                )
                .unwrap();
        }
        profile.representation = partition::Representation::Hierarchy { parent_tokens: 64 };
        profile.card_tokens = 512;
        let tokenizer = DocumentTokenizer::load(&profile).unwrap();
        let digest = profile.descriptor.digest();
        let recipe = profile.recipe_id();
        let cards = Cards {
            tokenizer: &tokenizer,
            profile: &profile,
            function_digest: &digest,
        };
        for range in [0..8, 8..12] {
            let sources = range
                .map(|n| {
                    let path = format!("note{n:02}.md");
                    let hash = engine.source(&path).unwrap().unwrap().hash;
                    engine.semantic_partition_source(&path, &hash).unwrap()
                })
                .collect();
            for partition in cards
                .prepare_partitions(sources, &recipe, &Control::unbounded())
                .unwrap()
            {
                engine
                    .semantic_accept_partition(partition)
                    .and_then(cache::PartitionCommit::committed)
                    .unwrap();
            }
        }
        let steps = Steps {
            engine: &engine,
            cards,
            recipe: &recipe,
        };
        let mut walk = Walk {
            overview_first: true,
            ..Default::default()
        };
        let mut selected = Vec::new();
        let mut report = PrepareReport::default();
        loop {
            let mut selection = Selection::default();
            let result = steps
                .select_batch(&mut walk, &mut selection, 1, 2, &mut report, &mut || None)
                .unwrap();
            for source in selection.sources {
                assert_eq!(source.wanted.len(), 1);
                assert_eq!(source.wanted[0].0, 0);
                let unit = &source.wanted[0].1;
                assert_eq!(
                    (unit.start, unit.end),
                    (
                        0,
                        engine.source(&source.source.path).unwrap().unwrap().bytes
                    )
                );
                selected.push(source.source.path.clone());
            }
            if matches!(result, Selected::End) {
                break;
            }
        }
        assert_eq!(
            selected,
            (0..12)
                .map(|n| format!("note{n:02}.md"))
                .collect::<Vec<_>>()
        );
        let mut refinement = Walk::default();
        let mut remaining = Selection::default();
        steps
            .select_batch(
                &mut refinement,
                &mut remaining,
                64,
                usize::MAX,
                &mut report,
                &mut || None,
            )
            .unwrap();
        assert!(
            remaining.sources[0].wanted.len() > 1,
            "ordinary refinement still includes bodies"
        );
        let mut order = SourceOrder::new(&[])
            .unwrap()
            .for_representation(profile.representation);
        assert!(order.overview_first());
        let cursor = order.next().unwrap();
        order.advance(cursor, true);
        assert!(order.next().is_some());
        assert!(!order.overview_first());
        assert!(
            order.next().is_none(),
            "exactly two passes, no repeated body loop"
        );
    }

    fn inputs(engine: &Engine, count: usize) -> Vec<cache::PartitionSource> {
        (0..count)
            .map(|n| {
                let path = format!("note{n:02}.md");
                let hash = engine.source(&path).unwrap().unwrap().hash;
                engine.semantic_partition_source(&path, &hash).unwrap()
            })
            .collect()
    }

    #[test]
    fn parallel_partitions_equal_serial_and_snapshot_admission_is_bounded() {
        let (_dir, engine, profile) = partition_fixture();
        let tokenizer = DocumentTokenizer::load(&profile).unwrap();
        let digest = profile.descriptor.digest();
        let recipe = profile.recipe_id();
        let cards = Cards {
            tokenizer: &tokenizer,
            profile: &profile,
            function_digest: &digest,
        };
        let control = Control::unbounded();
        let mut after = None;
        let mut report = PrepareReport::default();
        let (_, wave) = Steps {
            engine: &engine,
            cards,
            recipe: &recipe,
        }
        .partition_inputs(&mut after, usize::MAX, None, &mut report, &mut || None)
        .unwrap();
        assert_eq!(wave.len(), PARTITION_WAVE);
        assert_eq!(after.as_deref(), Some("note07.md"));
        assert_eq!(report.partitioned_sources, 0);
        assert!(engine.semantic_partition("note00.md").unwrap().is_none());
        let mut snapshots = Vec::new();
        for workers in [1, 8] {
            let prepared = cards
                .prepare_partitions_with_workers(inputs(&engine, 8), &recipe, &control, workers)
                .unwrap();
            for (n, partition) in prepared.into_iter().enumerate() {
                assert_eq!(partition.path(), format!("note{n:02}.md"));
                engine
                    .semantic_accept_partition(partition)
                    .and_then(cache::PartitionCommit::committed)
                    .unwrap();
            }
            snapshots.push(
                (0..8)
                    .map(|n| {
                        serde_json::to_value(
                            engine
                                .semantic_partition(&format!("note{n:02}.md"))
                                .unwrap(),
                        )
                        .unwrap()
                    })
                    .collect::<Vec<_>>(),
            );
        }
        assert_eq!(snapshots[0], snapshots[1]);
        assert_eq!(
            cards
                .prepare_partitions(inputs(&engine, 9), &recipe, &control)
                .unwrap_err()
                .code(),
            "partition_invalid"
        );
    }

    #[test]
    fn partition_cpu_workers_overlap_without_a_store_transaction() {
        use std::sync::{Condvar, Mutex};
        struct Concurrent<'a> {
            inner: &'a DocumentTokenizer,
            arrivals: Mutex<usize>,
            ready: Condvar,
        }
        impl TokenCount for Concurrent<'_> {
            fn encode(&self, text: &str) -> Result<partition::TokenizedText, ProviderError> {
                let mut arrived = self.arrivals.lock().unwrap();
                *arrived += 1;
                self.ready.notify_all();
                let (arrived, timed) = self
                    .ready
                    .wait_timeout_while(arrived, Duration::from_secs(5), |arrived| *arrived < 2)
                    .unwrap();
                drop(arrived);
                assert!(
                    !timed.timed_out(),
                    "a second CPU worker must enter while the first is blocked"
                );
                self.inner.encode(text)
            }
        }
        let (_dir, engine, profile) = partition_fixture();
        let tokenizer = DocumentTokenizer::load(&profile).unwrap();
        let concurrent = Concurrent {
            inner: &tokenizer,
            arrivals: Mutex::new(0),
            ready: Condvar::new(),
        };
        let digest = profile.descriptor.digest();
        let cards = Cards {
            tokenizer: &concurrent,
            profile: &profile,
            function_digest: &digest,
        };
        let prepared = cards
            .prepare_partitions_with_workers(inputs(&engine, 2), "recipe", &Control::unbounded(), 2)
            .unwrap();
        assert_eq!(prepared.len(), 2);
        assert!(engine.semantic_partition("note00.md").unwrap().is_none());
    }

    #[test]
    fn partition_cancellation_and_worker_panic_publish_nothing() {
        struct Panics;
        impl TokenCount for Panics {
            fn encode(&self, _: &str) -> Result<partition::TokenizedText, ProviderError> {
                panic!("injected tokenizer panic")
            }
        }
        let (_dir, engine, profile) = partition_fixture();
        let tokenizer = DocumentTokenizer::load(&profile).unwrap();
        let digest = profile.descriptor.digest();
        let cards = Cards {
            tokenizer: &tokenizer,
            profile: &profile,
            function_digest: &digest,
        };
        assert_eq!(
            cards
                .prepare_partitions(inputs(&engine, 8), "recipe", &Control::cancelled())
                .unwrap_err()
                .code(),
            "cancelled"
        );
        let mut prepared = cards
            .prepare_partitions(inputs(&engine, 1), "recipe", &Control::unbounded())
            .unwrap();
        assert_eq!(
            commit_partition(
                &engine,
                prepared.pop().unwrap(),
                &Control::cancelled(),
                &mut PrepareReport::default()
            )
            .unwrap_err()
            .code(),
            "cancelled"
        );
        let panics = Cards {
            tokenizer: &Panics,
            ..cards
        };
        let error = panics
            .prepare_partitions_with_workers(inputs(&engine, 8), "recipe", &Control::unbounded(), 8)
            .unwrap_err();
        assert_eq!(error.code(), "partition_worker_failed");
        let mut report = PrepareReport {
            profile: profile.name.clone(),
            function_digest: digest,
            recipe_id: "recipe".into(),
            dimensions: profile.descriptor.dimensions,
            ..PrepareReport::default()
        };
        finalize(&engine, &mut report, &Err(error)).unwrap();
        assert_eq!(
            engine
                .semantic_state()
                .unwrap()
                .unwrap()
                .last_error
                .unwrap()
                .code,
            "partition_worker_failed"
        );
        assert!(engine.semantic_partition("note00.md").unwrap().is_none());
    }

    /// 009 T003, captain decision 2026-10-06: a partition step stops at its
    /// time bound once it examined a source, and at its count bound without
    /// one. The injected bound has already passed, so each step writes
    /// exactly one partition.
    #[test]
    fn a_partition_step_stops_at_its_time_bound() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("workspace");
        std::fs::create_dir_all(&root).unwrap();
        for n in 0..12 {
            std::fs::write(
                root.join(format!("note{n:02}.md")),
                format!("# Note {n}\n\nbody of note {n}\n"),
            )
            .unwrap();
        }
        let mut engine = Engine::initialize(&dir.path().join("store"), &root).unwrap();
        engine.index(&root, &Control::unbounded()).unwrap();
        let path = crate::testkit::write_semantic_profile(dir.path(), "steps", |_| {});
        let profile = SemanticProfile::load(&path).unwrap();
        let tokenizer = DocumentTokenizer::load(&profile).unwrap();
        let digest = profile.descriptor.digest();
        let recipe = profile.recipe_id();
        let steps = Steps {
            engine: &engine,
            cards: Cards {
                tokenizer: &tokenizer,
                profile: &profile,
                function_digest: &digest,
            },
            recipe: &recipe,
        };
        let control = Control::unbounded();
        let mut report = PrepareReport::default();
        let mut after = None;
        let mut step = |until: Option<Instant>, report: &mut PrepareReport| {
            steps
                .partition_page(&mut after, 8, until, &control, report, &mut || None)
                .unwrap()
        };
        for written in 1..=3 {
            assert!(matches!(
                step(Some(Instant::now()), &mut report),
                Progress::More
            ));
            assert_eq!(report.partitioned_sources, written, "one source per step");
        }
        assert!(matches!(step(None, &mut report), Progress::More));
        assert_eq!(report.partitioned_sources, 11, "the count bound: 8 more");
        assert!(matches!(step(None, &mut report), Progress::More));
        assert_eq!(report.partitioned_sources, 12);
        assert!(matches!(step(None, &mut report), Progress::Done));
        assert_eq!(report.sources, 12, "every source examined once");
    }

    /// Counts every tokenization: the cost of rendering cards.
    struct Counting<'a> {
        inner: &'a DocumentTokenizer,
        encodes: std::sync::atomic::AtomicUsize,
    }

    impl<'a> Counting<'a> {
        fn new(inner: &'a DocumentTokenizer) -> Self {
            Self {
                inner,
                encodes: std::sync::atomic::AtomicUsize::new(0),
            }
        }
    }

    impl TokenCount for Counting<'_> {
        fn encode(&self, rendered: &str) -> Result<partition::TokenizedText, ProviderError> {
            self.encodes
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            self.inner.encode(rendered)
        }
    }

    #[test]
    fn cpu_pool_is_lazy_reused_and_recovers_after_a_panicking_wave() {
        use std::sync::Mutex;
        struct Threads<'a> {
            inner: &'a DocumentTokenizer,
            seen: Mutex<HashSet<std::thread::ThreadId>>,
        }
        impl TokenCount for Threads<'_> {
            fn encode(&self, text: &str) -> Result<partition::TokenizedText, ProviderError> {
                self.seen
                    .lock()
                    .unwrap()
                    .insert(std::thread::current().id());
                self.inner.encode(text)
            }
        }
        struct Panics;
        impl TokenCount for Panics {
            fn encode(&self, _: &str) -> Result<partition::TokenizedText, ProviderError> {
                panic!("injected partition panic")
            }
        }
        let (_dir, engine, profile) = partition_fixture();
        let tokenizer = DocumentTokenizer::load(&profile).unwrap();
        let threads = Threads {
            inner: &tokenizer,
            seen: Mutex::new(HashSet::new()),
        };
        let digest = profile.descriptor.digest();
        let cards = Cards {
            tokenizer: &threads,
            profile: &profile,
            function_digest: &digest,
        };
        let pool = PartitionWorkers::new(1);
        pool.prepare(cards, vec![], "recipe", &Control::unbounded())
            .unwrap();
        assert!(pool.pool.get().is_none(), "idle work starts no threads");
        assert!(
            pool.prepare(cards, inputs(&engine, 1), "recipe", &Control::cancelled())
                .is_err()
        );
        assert!(
            pool.pool.get().is_none(),
            "cancelled work starts no threads"
        );
        for _ in 0..3 {
            pool.prepare(cards, inputs(&engine, 2), "recipe", &Control::unbounded())
                .unwrap();
        }
        let error = pool
            .prepare(
                Cards {
                    tokenizer: &Panics,
                    ..cards
                },
                inputs(&engine, 2),
                "recipe",
                &Control::unbounded(),
            )
            .unwrap_err();
        assert_eq!(error.code(), "partition_worker_failed");
        pool.prepare(cards, inputs(&engine, 2), "recipe", &Control::unbounded())
            .unwrap();
        assert_eq!(
            threads.seen.lock().unwrap().len(),
            1,
            "same CPU thread across waves and failure"
        );
        assert!(engine.semantic_partition("note00.md").unwrap().is_none());
    }

    #[test]
    fn retained_cards_avoid_second_tokenization_across_batches_with_bounded_fallback() {
        use std::sync::atomic::Ordering;
        for representation in [
            partition::Representation::Address,
            partition::Representation::Body { parent_tokens: 16 },
            partition::Representation::Hierarchy { parent_tokens: 16 },
            partition::Representation::Overview,
        ] {
            let (_dir, engine, mut profile) = partition_fixture();
            profile.representation = representation;
            let body: String = (0..40)
                .map(|n| format!("/// Item {n}.\npub fn item_{n:02}() -> u32 {{\n    {n}\n}}\n\n"))
                .collect();
            engine.replace_source("lib.rs", &body).unwrap();
            let hash = engine.source("lib.rs").unwrap().unwrap().hash;
            let tokenizer = DocumentTokenizer::load(&profile).unwrap();
            let counting = Counting::new(&tokenizer);
            let digest = profile.descriptor.digest();
            let recipe = profile.recipe_id();
            let cards = Cards {
                tokenizer: &counting,
                profile: &profile,
                function_digest: &digest,
            };
            let expected = match representation {
                partition::Representation::Address => partition::cards(
                    &body,
                    "lib.rs",
                    Lang::from_path("lib.rs"),
                    &cards.recipe(),
                    &counting,
                )
                .unwrap(),
                partition::Representation::Body { parent_tokens } => {
                    let mut expected = Vec::new();
                    partition::body::visit(
                        &body,
                        Lang::from_path("lib.rs"),
                        &cards.recipe(),
                        parent_tokens as usize,
                        &counting,
                        |_, card| expected.push(card),
                    )
                    .unwrap();
                    expected
                }
                partition::Representation::Hierarchy { parent_tokens } => {
                    partition::hierarchy::plan(
                        &body,
                        Lang::from_path("lib.rs"),
                        &cards.recipe(),
                        parent_tokens as usize,
                        &counting,
                    )
                    .unwrap()
                    .iter()
                    .map(|span| {
                        partition::hierarchy::card(&body, span, &cards.recipe(), &counting).unwrap()
                    })
                    .collect()
                }
                partition::Representation::Overview => partition::hierarchy::file_plan(
                    &body,
                    Lang::from_path("lib.rs"),
                    &cards.recipe(),
                    &counting,
                )
                .unwrap()
                .iter()
                .map(|span| {
                    partition::hierarchy::card(&body, span, &cards.recipe(), &counting).unwrap()
                })
                .collect(),
            };
            let once = counting.encodes.load(Ordering::Relaxed);
            let pool = PartitionWorkers::new(2);
            // 512 admits the array poorly; 4096 allows some ids before overflowing.
            // Both must fall back without changing any card or dropping later units.
            let limits: &[usize] = if representation == partition::Representation::Overview {
                &[RETAINED_CARD_BYTES, 0]
            } else {
                &[RETAINED_CARD_BYTES, 0, 512, 4096]
            };
            for &limit in limits {
                counting.encodes.store(0, Ordering::Relaxed);
                let source = engine.semantic_partition_source("lib.rs", &hash).unwrap();
                let computed = if limit == RETAINED_CARD_BYTES {
                    pool.prepare(cards, vec![source], &recipe, &Control::unbounded())
                        .unwrap()
                        .pop()
                        .unwrap()
                } else {
                    cards.prepare_source(source, &recipe, limit).unwrap()
                };
                let kept = computed.retained.is_some();
                assert_eq!(kept, limit == RETAINED_CARD_BYTES);
                engine
                    .semantic_accept_partition(computed.partition)
                    .and_then(cache::PartitionCommit::committed)
                    .unwrap();
                let mut walk = Walk::wave(Some("lib.rs".into()), None, Some("lib.rs".into()))
                    .with_retained(computed.retained.into_iter().collect());
                let steps = Steps {
                    engine: &engine,
                    cards,
                    recipe: &recipe,
                };
                let mut actual = Vec::new();
                loop {
                    let mut selection = Selection::default();
                    let end = matches!(
                        steps
                            .select_batch(
                                &mut walk,
                                &mut selection,
                                8,
                                usize::MAX,
                                &mut PrepareReport::default(),
                                &mut || None
                            )
                            .unwrap(),
                        Selected::End
                    );
                    let batch = cards.render(selection).unwrap();
                    actual.extend(
                        batch
                            .keys
                            .into_iter()
                            .zip(batch.inputs.into_iter().map(|input| input.ids)),
                    );
                    if end {
                        break;
                    }
                }
                assert_eq!(
                    actual,
                    expected
                        .iter()
                        .map(|card| (card.input_key.clone(), card.ids.clone()))
                        .collect::<Vec<_>>()
                );
                assert_eq!(
                    counting.encodes.load(Ordering::Relaxed),
                    once * if kept { 1 } else { 2 }
                        + if !kept
                            && matches!(representation, partition::Representation::Body { .. })
                        {
                            expected.len()
                        } else {
                            0
                        }
                );
            }
        }
    }

    #[test]
    fn retained_cards_recheck_versions_and_reject_differing_recorded_tuples() {
        let (_dir, engine, profile) = partition_fixture();
        let tokenizer = DocumentTokenizer::load(&profile).unwrap();
        let digest = profile.descriptor.digest();
        let recipe = profile.recipe_id();
        let cards = Cards {
            tokenizer: &tokenizer,
            profile: &profile,
            function_digest: &digest,
        };
        let pool = PartitionWorkers::new(1);
        let computed = pool
            .prepare(cards, inputs(&engine, 1), &recipe, &Control::unbounded())
            .unwrap()
            .pop()
            .unwrap();
        engine
            .semantic_accept_partition(computed.partition)
            .and_then(cache::PartitionCommit::committed)
            .unwrap();
        let stale = computed.retained.unwrap();
        engine
            .replace_source("note00.md", "# Changed source\n\nNew behavior.\n")
            .unwrap();
        let computed = pool
            .prepare(cards, inputs(&engine, 1), &recipe, &Control::unbounded())
            .unwrap()
            .pop()
            .unwrap();
        engine
            .semantic_accept_partition(computed.partition)
            .and_then(cache::PartitionCommit::committed)
            .unwrap();
        let current = computed.retained.unwrap();
        let expected = current.cards.clone();
        let counting = Counting::new(&tokenizer);
        let steps = Steps {
            engine: &engine,
            cards: Cards {
                tokenizer: &counting,
                ..cards
            },
            recipe: &recipe,
        };
        let mut walk = Walk::wave(Some("note00.md".into()), None, Some("note00.md".into()))
            .with_retained(vec![stale]);
        let mut selection = Selection::default();
        steps
            .select_batch(
                &mut walk,
                &mut selection,
                8,
                usize::MAX,
                &mut PrepareReport::default(),
                &mut || None,
            )
            .unwrap();
        let batch = steps.cards.render(selection).unwrap();
        assert_eq!(
            batch.keys,
            expected
                .iter()
                .map(|card| card.input_key.clone())
                .collect::<Vec<_>>()
        );
        assert!(
            counting.encodes.load(std::sync::atomic::Ordering::Relaxed) > 0,
            "a newer version is freshly rendered, never taken from the old handoff"
        );

        let mut walk = Walk::wave(Some("note00.md".into()), None, Some("note00.md".into()))
            .with_retained(vec![current]);
        let mut selection = Selection::default();
        steps
            .select_batch(
                &mut walk,
                &mut selection,
                8,
                usize::MAX,
                &mut PrepareReport::default(),
                &mut || None,
            )
            .unwrap();
        selection.sources[0].wanted[0].1.input_key = "00".repeat(32);
        assert_eq!(
            cards.render(selection).err().unwrap().code(),
            "partition_invalid"
        );
    }

    /// Counts real cache examinations and tokenizer calls while two hierarchy
    /// sources alternate eight-input turns. The replay control reproduces the
    /// former cursor reset; no provider or timing assertion is involved.
    #[test]
    fn hierarchy_turns_resume_linearly_without_replanning() {
        use std::collections::BTreeMap;
        use std::sync::atomic::Ordering;

        fn exercise(
            mode: u8,
            limit: usize,
        ) -> (u64, usize, BTreeMap<String, Vec<u32>>, Vec<String>) {
            let (_dir, engine, mut profile) = partition_fixture();
            profile.representation = partition::Representation::Hierarchy { parent_tokens: 16 };
            let tokenizer = DocumentTokenizer::load(&profile).unwrap();
            let counting = Counting::new(&tokenizer);
            let digest = profile.descriptor.digest();
            let recipe = profile.recipe_id();
            let cards = Cards {
                tokenizer: &counting,
                profile: &profile,
                function_digest: &digest,
            };
            let paths = ["a.rs".to_owned(), "b.rs".to_owned()];
            let mut total = 0;
            for (file, path) in paths.iter().enumerate() {
                let body: String = (0..96).map(|n| format!("/// Item {file}_{n}.\npub fn item_{file}_{n:03}() -> u32 {{\n    {}\n}}\n\n", file * 1000 + n)).collect();
                engine.replace_source(path, &body).unwrap();
                let hash = engine.source(path).unwrap().unwrap().hash;
                let computed = cards
                    .prepare_source(
                        engine.semantic_partition_source(path, &hash).unwrap(),
                        &recipe,
                        0,
                    )
                    .unwrap();
                engine
                    .semantic_accept_partition(computed.partition)
                    .and_then(cache::PartitionCommit::committed)
                    .unwrap();
                total += engine
                    .semantic_partition(path)
                    .unwrap()
                    .unwrap()
                    .units
                    .len() as u64;
            }
            counting.encodes.store(0, Ordering::Relaxed);
            let steps = Steps {
                engine: &engine,
                cards,
                recipe: &recipe,
            };
            let mut order = SourceOrder::changed(paths.iter().cloned())
                .for_representation(profile.representation);
            let mut paused = PausedSources::new(limit);
            let mut report = PrepareReport::default();
            let mut actual = BTreeMap::new();
            let mut turns = Vec::new();
            let mut overview_inputs = 0;
            while let Some(mut cursor) = order.next() {
                let path = cursor.scope.clone().unwrap();
                cursor.after = Some(path.clone());
                let resumed = cursor.resume.take();
                let before = resumed.as_ref().and_then(|position| position.after.clone());
                let mut walk = Walk::wave(Some(path.clone()), before, Some(path.clone()));
                walk.exact = true;
                walk.overview_first = order.overview_first();
                if let Some(position) = resumed {
                    walk = walk.restore(position, &mut paused);
                }
                loop {
                    let mut selection = Selection::default();
                    let selected = steps
                        .select_batch(
                            &mut walk,
                            &mut selection,
                            WAVE_INPUTS,
                            usize::MAX,
                            &mut report,
                            &mut || None,
                        )
                        .unwrap();
                    if walk.overview_first {
                        assert!(
                            selection
                                .sources
                                .iter()
                                .all(|source| source.wanted.iter().all(|(index, _)| *index == 0))
                        );
                    } else {
                        assert_eq!(
                            overview_inputs, 2,
                            "file-view pass drains before body refinement"
                        );
                    }
                    let batch = cards.render(selection).unwrap();
                    assert!(batch.len() <= WAVE_INPUTS);
                    if !batch.is_empty() {
                        if walk.overview_first {
                            overview_inputs += batch.len();
                        } else {
                            turns.push(path.clone());
                        }
                        let entries: Vec<_> = batch
                            .keys
                            .iter()
                            .map(|key| (key.clone(), vec![1.0; cards.dims()]))
                            .collect();
                        engine
                            .semantic_cache_commit(&entries, &digest, cards.dims(), u64::MAX)
                            .unwrap();
                        for (key, input) in batch.keys.into_iter().zip(batch.inputs) {
                            assert!(
                                actual.insert(key, input.ids).is_none(),
                                "no repeated model input"
                            );
                        }
                        walk.seen.clear();
                    }
                    if matches!(selected, Selected::End) {
                        order.advance(cursor, true);
                        break;
                    }
                    if mode != 2 && order.waiting() {
                        cursor.resume = Some(if mode == 0 {
                            WalkPosition {
                                after: None,
                                partial: None,
                            } // prior replay behavior
                        } else {
                            walk.pause(&mut paused)
                        });
                        assert!(paused.bytes <= limit, "all cursors share one byte cap");
                        order.advance(cursor, false);
                        break;
                    }
                }
            }
            if mode != 0 {
                assert_eq!(
                    report.eligible_units,
                    total + 2,
                    "each card examined once plus the two cached file views"
                );
            }
            // An unchanged fresh walk selects no inputs and never invokes the tokenizer.
            let before = counting.encodes.load(Ordering::Relaxed);
            for path in &paths {
                let mut walk = Walk::wave(Some(path.clone()), None, Some(path.clone()));
                walk.exact = true;
                let mut selection = Selection::default();
                assert!(matches!(
                    steps
                        .select_batch(
                            &mut walk,
                            &mut selection,
                            WAVE_INPUTS,
                            usize::MAX,
                            &mut PrepareReport::default(),
                            &mut || None
                        )
                        .unwrap(),
                    Selected::End
                ));
                assert!(cards.render(selection).unwrap().is_empty());
            }
            assert_eq!(
                counting.encodes.load(Ordering::Relaxed),
                before,
                "unchanged replay has zero render work"
            );
            (report.eligible_units, before, actual, turns)
        }

        let replay = exercise(0, 0);
        let resumed = exercise(1, PAUSED_PLAN_BYTES);
        let scalar_only = exercise(1, 0);
        let uninterrupted = exercise(2, 0);
        eprintln!(
            "hierarchy resume: units/tokenizer calls replay={}/{}, scalar={}/{}, bounded={}/{}, uninterrupted={}/{}",
            replay.0,
            replay.1,
            scalar_only.0,
            scalar_only.1,
            resumed.0,
            resumed.1,
            uninterrupted.0,
            uninterrupted.1
        );
        assert_eq!(resumed.2, replay.2);
        assert_eq!(resumed.2, scalar_only.2);
        assert_eq!(resumed.2, uninterrupted.2);
        assert!(
            replay.0 > resumed.0 * 4,
            "control really reexamines growing cache prefixes"
        );
        assert_eq!(
            resumed.1, uninterrupted.1,
            "turns add no hierarchy re-planning"
        );
        assert!(
            scalar_only.1 > resumed.1 * 2,
            "positions alone do not remove re-planning"
        );
        assert!(resumed.3.len() > 8);
        assert!(
            resumed.3.windows(2).all(|pair| pair[0] != pair[1]),
            "eight-input turns remain fair"
        );
    }

    #[test]
    fn paused_hierarchy_plans_are_globally_bounded_and_recheck_identity() {
        let (_dir, engine, mut profile) = partition_fixture();
        profile.representation = partition::Representation::Hierarchy { parent_tokens: 16 };
        let tokenizer = DocumentTokenizer::load(&profile).unwrap();
        let counting = Counting::new(&tokenizer);
        let digest = profile.descriptor.digest();
        let recipe = profile.recipe_id();
        let cards = Cards {
            tokenizer: &counting,
            profile: &profile,
            function_digest: &digest,
        };
        let source = |name: &str| {
            (0..40)
                .map(|n| format!("pub fn {name}_{n:02}() -> usize {{ {n} }}\n"))
                .collect::<String>()
        };
        let accept = |body: &str| {
            engine.replace_source("lib.rs", body).unwrap();
            let hash = engine.source("lib.rs").unwrap().unwrap().hash;
            let computed = cards
                .prepare_source(
                    engine.semantic_partition_source("lib.rs", &hash).unwrap(),
                    &recipe,
                    0,
                )
                .unwrap();
            engine
                .semantic_accept_partition(computed.partition)
                .and_then(cache::PartitionCommit::committed)
                .unwrap();
        };
        accept(&source("old"));
        let steps = Steps {
            engine: &engine,
            cards,
            recipe: &recipe,
        };
        let mut walk = Walk::wave(Some("lib.rs".into()), None, Some("lib.rs".into()));
        walk.exact = true;
        let mut selection = Selection::default();
        steps
            .select_batch(
                &mut walk,
                &mut selection,
                WAVE_INPUTS,
                usize::MAX,
                &mut PrepareReport::default(),
                &mut || None,
            )
            .unwrap();
        cards.render(selection).unwrap();
        let version = Rc::clone(&walk.partial.as_ref().unwrap().0);
        let bytes = version.paused_bytes().unwrap();
        let metadata = PAUSED_PLAN_SLOTS * std::mem::size_of::<(usize, Rc<SourceVersion>)>();
        let mut paused = PausedSources::new(metadata + bytes);
        let position = walk.pause(&mut paused);
        assert_eq!(paused.bytes, metadata + bytes);
        // A different path consumes the same global budget and evicts the first.
        let mut other = SourceVersion {
            path: "two.rs".into(),
            hash: version.hash.clone(),
            recipe: recipe.clone(),
            function_digest: digest.clone(),
            content: SourceCards::Body {
                body: source("old"),
                units: OnceCell::new(),
            },
            paused_size: OnceCell::new(),
        };
        if let SourceCards::Body { body, units } = &mut other.content {
            units
                .set(SourcePlan::Hierarchy(
                    partition::hierarchy::plan(
                        body,
                        Lang::from_path("two.rs"),
                        &cards.recipe(),
                        16,
                        &counting,
                    )
                    .unwrap(),
                ))
                .ok()
                .unwrap();
        }
        let other = Rc::new(other);
        paused = PausedSources::new(metadata + bytes.max(other.paused_bytes().unwrap()));
        paused.insert(Rc::clone(&version));
        paused.insert(other);
        assert!(paused.bytes <= paused.limit && paused.entries.len() <= 1);
        assert!(
            paused.take(position.partial.as_ref().unwrap()).is_none(),
            "another cursor cannot extend the global budget"
        );
        paused.insert(Rc::clone(&version));
        for field in 0..3 {
            let mut wrong = position.partial.clone().unwrap();
            match field {
                0 => wrong.hash.push('x'),
                1 => wrong.recipe.push('x'),
                _ => wrong.function_digest.push('x'),
            }
            assert!(paused.take(&wrong).is_none());
            assert!(
                paused.entries.is_empty(),
                "same-path identity mismatch evicts stale plan"
            );
            paused.insert(Rc::clone(&version));
        }
        // Cancellation does no selection/tokenization and preserves the resume point.
        let mut resumed = Walk::wave(Some("lib.rs".into()), None, Some("lib.rs".into()))
            .restore(position.clone(), &mut paused);
        resumed.exact = true;
        let before = counting.encodes.load(std::sync::atomic::Ordering::Relaxed);
        let mut report = PrepareReport::default();
        assert!(matches!(
            steps
                .select_batch(
                    &mut resumed,
                    &mut Selection::default(),
                    WAVE_INPUTS,
                    usize::MAX,
                    &mut report,
                    &mut || Some(Stop::partial("cancelled", "test"))
                )
                .unwrap(),
            Selected::Halted(_)
        ));
        assert_eq!(report.eligible_units, 0);
        assert_eq!(
            counting.encodes.load(std::sync::atomic::Ordering::Relaxed),
            before
        );
        // Replacing a source invalidates both its plan and scalar next-card index.
        accept(&source("new"));
        let mut selection = Selection::default();
        steps
            .select_batch(
                &mut resumed,
                &mut selection,
                WAVE_INPUTS,
                usize::MAX,
                &mut report,
                &mut || None,
            )
            .unwrap();
        assert_eq!(selection.sources[0].wanted[0].0, 0);
        let rendered = cards.render(selection).unwrap();
        assert_eq!(
            rendered.keys[0],
            engine.semantic_partition("lib.rs").unwrap().unwrap().units[0].input_key
        );
        let mut selection = Selection::default();
        steps
            .select_batch(
                &mut resumed,
                &mut selection,
                WAVE_INPUTS,
                usize::MAX,
                &mut report,
                &mut || None,
            )
            .unwrap();
        selection.sources[0].wanted[0].1.input_key = "00".repeat(32);
        assert_eq!(
            cards.render(selection).err().unwrap().code(),
            "partition_invalid",
            "cached plans still verify every selected tuple"
        );
    }

    /// 009 T004 M4: the embed walk renders and tokenizes only the cards it
    /// selected. Five batches of eight cards from ONE source of forty
    /// definitions cost exactly what rendering each card once costs, not
    /// five renderings of the whole source.
    #[test]
    fn a_partially_consumed_source_renders_only_its_selected_cards() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("workspace");
        std::fs::create_dir_all(&root).unwrap();
        let mut source = String::new();
        for n in 0..40 {
            source.push_str(&format!(
                "/// Item {n}.\npub fn item_{n:02}() -> u32 {{\n    {n}\n}}\n\n"
            ));
        }
        std::fs::write(root.join("lib.rs"), &source).unwrap();
        let mut engine = Engine::initialize(&dir.path().join("store"), &root).unwrap();
        engine.index(&root, &Control::unbounded()).unwrap();
        let path = crate::testkit::write_semantic_profile(dir.path(), "steps", |_| {});
        let profile = SemanticProfile::load(&path).unwrap();
        let tokenizer = DocumentTokenizer::load(&profile).unwrap();
        let digest = profile.descriptor.digest();
        let recipe = profile.recipe_id();
        let control = Control::unbounded();
        let mut report = PrepareReport::default();
        let partitioning = Steps {
            engine: &engine,
            cards: Cards {
                tokenizer: &tokenizer,
                profile: &profile,
                function_digest: &digest,
            },
            recipe: &recipe,
        };
        let mut after = None;
        while let Progress::More = partitioning
            .partition_page(
                &mut after,
                usize::MAX,
                None,
                &control,
                &mut report,
                &mut || None,
            )
            .unwrap()
        {}
        // What rendering every card once costs.
        let once = Counting::new(&tokenizer);
        let cards = partition::cards(
            &source,
            "lib.rs",
            Lang::from_path("lib.rs"),
            &partitioning.cards.recipe(),
            &once,
        )
        .unwrap();
        assert_eq!(cards.len(), 40);
        // The embed walk, selection then rendering, batch by batch.
        let counting = Counting::new(&tokenizer);
        let steps = Steps {
            engine: &engine,
            cards: Cards {
                tokenizer: &counting,
                profile: &profile,
                function_digest: &digest,
            },
            recipe: &recipe,
        };
        let mut walk = Walk::default();
        let mut batches = Vec::new();
        loop {
            let mut selection = Selection::default();
            let end = match steps
                .select_batch(
                    &mut walk,
                    &mut selection,
                    8,
                    usize::MAX,
                    &mut report,
                    &mut || None,
                )
                .unwrap()
            {
                Selected::Full => false,
                Selected::End => true,
                Selected::Yield => continue,
                Selected::Halted(_) => unreachable!("nothing halts this walk"),
            };
            let batch = steps.cards.render(selection).unwrap();
            if !batch.is_empty() {
                batches.push(batch.len());
            }
            if end {
                break;
            }
        }
        assert_eq!(batches, [8; 5]);
        assert_eq!(
            counting.encodes.load(std::sync::atomic::Ordering::Relaxed),
            once.encodes.load(std::sync::atomic::Ordering::Relaxed),
            "each selected card is rendered once, nothing else"
        );
    }
}
