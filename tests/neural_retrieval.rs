//! 009 T002 acceptance (FR-003, FR-005, FR-006 / SC-002): semantic evidence
//! reaches the final context response inside the existing budget, with
//! source freshness, a deterministic merge and named baseline fallbacks.
//!
//! The vocabulary-gap cases run the real preparation path, the real Rust
//! tokenizer, the real USearch index and the real request path
//! (`mcp::context_primary`/`search_primary`, the functions the MCP owner and
//! the CLI both call). The embedding function is a test provider with a
//! fixed concept lexicon: it makes the ORDERING, budget and fallback
//! assertions exact, and it is NOT a retrieval-quality result — the actual
//! model runs the same fixture through the production binary and records
//! its own evidence. Every provider call is counted.
#![cfg(feature = "semantic")]

use context_foundry::config::BudgetConfig;
use context_foundry::fault::{self, Action};
use context_foundry::mcp::{self, HttpOptions, SemanticServing, SemanticSlot, ServerOptions};
use context_foundry::neural::cache::DEFAULT_CACHE_CAP_BYTES;
use context_foundry::neural::index;
use context_foundry::neural::merge::{self, MergeUnit, RRF_K};
use context_foundry::neural::prepare::{self, Acquire, PrepareOptions, PrepareReport};
use context_foundry::neural::profile::SemanticProfile;
use context_foundry::neural::provider::{
    self, DIMENSIONS, EmbeddingProvider, FunctionDescriptor, ProviderError, TokenizedInput,
};
use context_foundry::neural::query::{
    DENSE_WINDOW, DenseWindow, MakeProvider, QUERY_CEILING, QueryRuntime,
};
use context_foundry::response::{self, Budget, PackedText};
use context_foundry::store::CandidateBatch;
use context_foundry::testkit::{self, V2Item, V2Response, parse_v2};
use context_foundry::{Control, Engine, Strategy};
use rmcp::{
    ServiceExt, model::CallToolRequestParams,
    transport::streamable_http_client::StreamableHttpClientTransportConfig,
};
use std::collections::BTreeSet;
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

// ---------------------------------------------------------------------------
// The fixture: tests/fixtures/semantic-retrieval, with FROZEN expected spans.
// ---------------------------------------------------------------------------

/// The normal response budget (tokens).
const BUDGET: usize = 2048;
/// One embedding unit holds at most 1024 tokens including the 9-byte
/// `passage: ` prefix; the fixture tokenizer is byte level.
const UNIT_BYTES: u64 = 1024 - 9;

const DUSK_PATH: &str = "docs/dusk.md";
const DUSK_QUERY: &str = "twilight onset";
/// The whole short document.
const DUSK_EVIDENCE: Range<u64> = 0..320;

const LEDGER_PATH: &str = "docs/ledger.md";
const LEDGER_QUERY: &str = "refund timing";
/// `## Reimbursement window` through the end of the long document, which is
/// longer than one real-model embedding unit (about 5.7 KB of prose).
const LEDGER_EVIDENCE: Range<u64> = 5522..5740;
const LEDGER_LEN: u64 = 5740;

const CODE_PATH: &str = "src/widgets.rs";
const CODE_QUERY: &str = "retry backoff";
/// `pause_before_next_attempt` with its doc comment, through its closing
/// brace, among twenty-odd functions that have nothing to do with retrying.
const CODE_EVIDENCE: Range<u64> = 517..809;
const CODE_LEN: u64 = 4517;
/// The code file spans more than one embedding unit.
const _: () = assert!(CODE_LEN > UNIT_BYTES);

fn fixture_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/semantic-retrieval")
}

fn copy_tree(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for entry in std::fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let target = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_tree(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), target).unwrap();
        }
    }
}

// ---------------------------------------------------------------------------
// The test embedding function: a fixed concept lexicon over the exact model
// input (the byte-level fixture tokenizer makes id == byte).
// ---------------------------------------------------------------------------

const CONCEPTS: [&[&str]; 3] = [
    &[
        "sundown",
        "dusk",
        "twilight",
        "nightfall",
        "horizon",
        "amber",
        "sunset",
        "glow",
    ],
    &["refund", "reimburs", "repayment", "credit", "cancel"],
    &["retry", "backoff", "pause", "attempt", "doubl", "failure"],
];

fn model_input(ids: &[u32]) -> String {
    let bytes: Vec<u8> = ids.iter().map(|&id| id as u8).collect();
    String::from_utf8_lossy(&bytes).into_owned()
}

/// Concept counts in the leading dimensions plus a small hashed word
/// signature, L2-normalized.
fn concept_vector(rendered: &str) -> Vec<f32> {
    let text = rendered
        .strip_prefix("passage: ")
        .or_else(|| rendered.strip_prefix("query: "))
        .unwrap_or(rendered)
        .to_ascii_lowercase();
    let mut vector = vec![0f32; DIMENSIONS];
    for (dimension, words) in CONCEPTS.iter().enumerate() {
        vector[dimension] = words
            .iter()
            .map(|word| text.matches(word).count() as f32)
            .sum();
    }
    for word in text
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|word| word.len() >= 3)
    {
        let mut hash = 0xcbf2_9ce4_8422_2325u64;
        for byte in word.bytes() {
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(0x0100_0000_01b3);
        }
        let slot = CONCEPTS.len() + (hash % (DIMENSIONS - CONCEPTS.len()) as u64) as usize;
        vector[slot] += 0.03;
    }
    let norm = vector.iter().map(|v| v * v).sum::<f32>().sqrt();
    if norm == 0.0 {
        vector[DIMENSIONS - 1] = 1.0;
        return vector;
    }
    vector.iter().map(|v| v / norm).collect()
}

/// How the next query embeddings behave.
#[derive(Clone, Debug, Default)]
enum QueryMode {
    #[default]
    Answer,
    /// Sleep this long first, then answer (a stalled provider).
    Sleep(Duration),
    Fail(ProviderError),
    /// Answer with a vector one component short.
    Short,
}

/// Live counters shared by every provider the test builds.
#[derive(Clone, Default)]
struct Probe {
    document_calls: Arc<AtomicU64>,
    document_inputs: Arc<AtomicU64>,
    document_tokens: Arc<AtomicU64>,
    query_calls: Arc<AtomicU64>,
    queries: Arc<Mutex<Vec<String>>>,
    mode: Arc<Mutex<QueryMode>>,
    /// How long every document call takes (a slow model for budget stops).
    document_pause: Arc<Mutex<Duration>>,
}

impl Probe {
    fn document_calls(&self) -> u64 {
        self.document_calls.load(Ordering::SeqCst)
    }
    fn document_inputs(&self) -> u64 {
        self.document_inputs.load(Ordering::SeqCst)
    }
    fn document_tokens(&self) -> u64 {
        self.document_tokens.load(Ordering::SeqCst)
    }
    fn query_calls(&self) -> u64 {
        self.query_calls.load(Ordering::SeqCst)
    }
    fn queries(&self) -> Vec<String> {
        self.queries.lock().unwrap().clone()
    }
    fn set(&self, mode: QueryMode) {
        *self.mode.lock().unwrap() = mode;
    }
    fn pause_documents(&self, pause: Duration) {
        *self.document_pause.lock().unwrap() = pause;
    }
}

struct ConceptProvider {
    descriptor: FunctionDescriptor,
    probe: Probe,
}

impl EmbeddingProvider for ConceptProvider {
    fn descriptor(&self) -> &FunctionDescriptor {
        &self.descriptor
    }

    fn embed_documents(
        &mut self,
        batch: &[TokenizedInput],
        _control: &Control,
    ) -> Result<Vec<Vec<f32>>, ProviderError> {
        provider::check_document_batch(batch)?;
        let pause = *self.probe.document_pause.lock().unwrap();
        std::thread::sleep(pause);
        self.probe.document_calls.fetch_add(1, Ordering::SeqCst);
        self.probe
            .document_inputs
            .fetch_add(batch.len() as u64, Ordering::SeqCst);
        let tokens: usize = batch.iter().map(|input| input.ids.len()).sum();
        self.probe
            .document_tokens
            .fetch_add(tokens as u64, Ordering::SeqCst);
        Ok(batch
            .iter()
            .map(|input| concept_vector(&model_input(&input.ids)))
            .collect())
    }

    fn embed_query(
        &mut self,
        input: &TokenizedInput,
        _deadline: Instant,
    ) -> Result<Vec<f32>, ProviderError> {
        provider::check_query(input)?;
        let rendered = model_input(&input.ids);
        self.probe.query_calls.fetch_add(1, Ordering::SeqCst);
        self.probe.queries.lock().unwrap().push(rendered.clone());
        let mode = self.probe.mode.lock().unwrap().clone();
        match mode {
            QueryMode::Answer => {}
            QueryMode::Sleep(pause) => std::thread::sleep(pause),
            QueryMode::Fail(error) => return Err(error),
            QueryMode::Short => {
                let mut vector = concept_vector(&rendered);
                vector.pop();
                return Ok(vector);
            }
        }
        Ok(concept_vector(&rendered))
    }
}

fn maker(descriptor: FunctionDescriptor, probe: Probe) -> MakeProvider {
    Box::new(move || {
        Ok(Box::new(ConceptProvider { descriptor, probe }) as Box<dyn EmbeddingProvider>)
    })
}

// ---------------------------------------------------------------------------
// A prepared workspace.
// ---------------------------------------------------------------------------

struct Corpus {
    dir: tempfile::TempDir,
    root: PathBuf,
    store: PathBuf,
    profile_path: PathBuf,
    profile: Arc<SemanticProfile>,
    engine: Option<Engine>,
    probe: Probe,
    report: PrepareReport,
    prepare_ms: u128,
}

impl Corpus {
    /// The fixture workspace plus `extra` files, indexed (lexical), not yet
    /// prepared.
    fn unprepared(extra: &[(String, String)]) -> Self {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("ws");
        copy_tree(&fixture_dir(), &root);
        for (relative, text) in extra {
            let path = root.join(relative);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, text).unwrap();
        }
        let store = dir.path().join("store");
        let mut engine = Engine::initialize(&store, &root).unwrap();
        engine.index(&root, &Control::unbounded()).unwrap();
        let profile_path = testkit::write_semantic_profile(dir.path(), "concept", |_| {});
        let profile = Arc::new(SemanticProfile::load(&profile_path).unwrap());
        Self {
            dir,
            root,
            store,
            profile_path,
            profile,
            engine: Some(engine),
            probe: Probe::default(),
            report: PrepareReport::default(),
            prepare_ms: 0,
        }
    }

    /// The fixture workspace plus `extra` files, indexed and prepared.
    fn new(extra: &[(String, String)]) -> Self {
        let mut corpus = Self::unprepared(extra);
        let started = Instant::now();
        corpus.report = corpus.prepare();
        corpus.prepare_ms = started.elapsed().as_millis();
        corpus
    }

    /// One bounded preparation run under the test provider; the engine is
    /// released for the run like the CLI process boundary.
    fn prepare(&mut self) -> PrepareReport {
        self.prepare_within(300)
    }

    /// [`Self::prepare`] under a `budget_seconds` budget.
    fn prepare_within(&mut self, budget_seconds: u64) -> PrepareReport {
        let descriptor = self.profile.descriptor.clone();
        let probe = self.probe.clone();
        let acquire: Acquire = Box::new(move |_profile, _development, _control| {
            Ok(Box::new(ConceptProvider {
                descriptor: descriptor.clone(),
                probe: probe.clone(),
            }) as Box<dyn EmbeddingProvider>)
        });
        self.prepare_through(acquire, budget_seconds)
    }

    /// One preparation run through `acquire`.
    fn prepare_through(&mut self, acquire: Acquire, budget_seconds: u64) -> PrepareReport {
        self.engine = None;
        let report = prepare::run(
            &self.store,
            &PrepareOptions {
                profile_path: &self.profile_path,
                budget_seconds,
                development: false,
                cache_cap_bytes: DEFAULT_CACHE_CAP_BYTES,
                started: Instant::now(),
                control: &Control::unbounded(),
            },
            acquire,
        )
        .expect("preparation runs");
        self.engine = Some(Engine::open_existing(&self.store).unwrap());
        report
    }

    fn engine(&self) -> &Engine {
        self.engine.as_ref().expect("the engine is open")
    }

    fn engine_mut(&mut self) -> &mut Engine {
        self.engine.as_mut().expect("the engine is open")
    }

    /// The live bytes of a workspace file.
    fn source(&self, path: &str) -> String {
        std::fs::read_to_string(self.root.join(path)).unwrap()
    }

    fn digest(&self) -> String {
        self.profile.descriptor.digest()
    }

    /// The owner's resident runtime over the test provider: exactly what the
    /// MCP owner and the CLI build, minus the worker process.
    fn slot(&self) -> SemanticSlot {
        mcp::semantic_slot(Some(SemanticServing::with_provider(
            self.profile_path.clone(),
            maker(self.profile.descriptor.clone(), self.probe.clone()),
        )))
        .expect("the slot starts")
    }

    fn runtime(&self) -> QueryRuntime {
        QueryRuntime::start(
            Arc::clone(&self.profile),
            maker(self.profile.descriptor.clone(), self.probe.clone()),
        )
        .expect("the provider starts")
    }

    fn control() -> Control {
        Control::with_deadline(Instant::now() + Duration::from_secs(30))
    }

    /// A context request through the production decision path.
    fn context(&self, slot: &SemanticSlot, query: &str, tokens: usize) -> Answer {
        let combined = mcp::context_primary(
            slot,
            self.engine(),
            query,
            Strategy::Search,
            &Self::control(),
            false,
        )
        .expect("context candidates");
        Answer::of(combined.batch, tokens)
    }

    /// The lexical baseline context: no profile, no provider.
    fn baseline(&self, query: &str, tokens: usize) -> Answer {
        let batch = self
            .engine()
            .context_candidates(query, Strategy::Search, &Control::unbounded())
            .expect("baseline candidates");
        Answer::of(batch, tokens)
    }

    /// A search request through the production decision path.
    fn search(&self, slot: &SemanticSlot, query: &str, limit: usize) -> SearchAnswer {
        self.search_at(slot, query, None, limit)
    }

    /// A path-restricted search request through the production path.
    fn search_at(
        &self,
        slot: &SemanticSlot,
        query: &str,
        path: Option<&str>,
        limit: usize,
    ) -> SearchAnswer {
        let batch = mcp::search_primary(slot, self.engine(), query, path, limit, &Self::control())
            .expect("search candidates");
        SearchAnswer::of(batch)
    }
}

/// What a candidate batch held, before packing.
#[derive(Clone, Debug)]
struct Facts {
    paths: Vec<String>,
    tiers: Vec<u8>,
    truncated: bool,
}

fn facts(batch: &CandidateBatch) -> Facts {
    Facts {
        paths: batch
            .items
            .iter()
            .filter_map(|item| item.handle.as_ref().map(|handle| handle.path.clone()))
            .collect(),
        tiers: batch.items.iter().map(|item| item.tier).collect(),
        truncated: batch.counters.truncated,
    }
}

struct Answer {
    batch: CandidateBatch,
    facts: Facts,
    packed: PackedText,
    parsed: V2Response,
}

impl Answer {
    fn of(batch: CandidateBatch, tokens: usize) -> Self {
        let facts = facts(&batch);
        let (packed, parsed) = pack(&batch, tokens).expect("the budget holds the header");
        Self {
            batch,
            facts,
            packed,
            parsed,
        }
    }

    fn word(&self) -> Option<&str> {
        header_word(&self.parsed)
    }

    fn item(&self, path: &str) -> Option<&V2Item> {
        item_of(&self.parsed, path)
    }
}

struct SearchAnswer {
    facts: Facts,
    packed: PackedText,
    parsed: V2Response,
}

impl SearchAnswer {
    fn of(batch: CandidateBatch) -> Self {
        let facts = facts(&batch);
        let outcome = Engine::search_outcome(batch);
        let packed =
            response::pack_search(&outcome, Budget::request(BUDGET), &response::stdout_bytes)
                .expect("search packs");
        let parsed = parse_v2(&packed.text)
            .unwrap_or_else(|error| panic!("not v2 ({error}):\n{}", packed.text));
        Self {
            facts,
            packed,
            parsed,
        }
    }

    fn word(&self) -> Option<&str> {
        header_word(&self.parsed)
    }

    fn item(&self, path: &str) -> Option<&V2Item> {
        item_of(&self.parsed, path)
    }
}

/// Pack one context batch at `tokens`; `None` when the header cannot fit.
fn pack(batch: &CandidateBatch, tokens: usize) -> Option<(PackedText, V2Response)> {
    match response::pack_context(batch, Budget::request(tokens), &response::stdout_bytes) {
        Ok(packed) => {
            let parsed = parse_v2(&packed.text)
                .unwrap_or_else(|error| panic!("not v2 ({error}):\n{}", packed.text));
            Some((packed, parsed))
        }
        Err(error) => {
            assert_eq!(error.code(), "budget_too_small");
            None
        }
    }
}

fn header_word(parsed: &V2Response) -> Option<&str> {
    parsed
        .header
        .iter()
        .find_map(|segment| segment.strip_prefix("semantic:"))
}

/// `(path, start, end)` of a v2 handle `path#start-end@sha32.ws16`.
fn span_of(handle: &str) -> (String, u64, u64) {
    let (head, _) = handle.rsplit_once('@').expect("a handle identity");
    let (path, range) = head.rsplit_once('#').expect("a handle range");
    let (start, end) = range.split_once('-').expect("a range");
    (
        path.to_owned(),
        start.parse().unwrap(),
        end.parse().unwrap(),
    )
}

fn handle_with_range(handle: &str, start: u64, end: u64) -> String {
    let (head, identity) = handle.rsplit_once('@').expect("a handle identity");
    let (path, _) = head.rsplit_once('#').expect("a handle range");
    format!("{path}#{start}-{end}@{identity}")
}

fn item_of<'a>(parsed: &'a V2Response, path: &str) -> Option<&'a V2Item> {
    parsed
        .items
        .iter()
        .find(|item| !item.handle.is_empty() && span_of(&item.handle).0 == path)
}

fn record(name: &str, value: serde_json::Value) {
    let text = serde_json::to_string_pretty(&value).unwrap();
    eprintln!("RECORD {name}: {text}");
    if let Ok(dir) = std::env::var("CF_T002_RECORD_DIR") {
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(Path::new(&dir).join(format!("{name}.json")), text).unwrap();
    }
}

fn keys(window: &DenseWindow) -> Vec<String> {
    window
        .hits
        .iter()
        .map(|hit| window.input_key(hit).to_owned())
        .collect()
}

/// The paths of the units the dense window named (through the serving
/// generation's label map, before any freshness check).
fn window_paths(window: &DenseWindow) -> BTreeSet<String> {
    window
        .hits
        .iter()
        .flat_map(|hit| window.units(hit))
        .map(|unit| unit.path.clone())
        .collect()
}

fn fillers(count: usize) -> Vec<(String, String)> {
    (0..count)
        .map(|n| {
            (
                format!("fill/note{n:02}.md"),
                format!(
                    "# Note {n}\n\nrefund refund refund refund refund refund slip number {n}.\n"
                ),
            )
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Who owns a missing piece of evidence (spec 009 § Ordering without another
// required model): retrieval, merge, delivery or packing.
// ---------------------------------------------------------------------------

#[derive(Debug, PartialEq, Eq)]
enum Owner {
    /// Neither the lexical nor the dense window carried the candidate.
    Retrieval,
    /// A window carried it; the fused, capped selection dropped it.
    Merge,
    /// It was delivered, but as a preview of an unlocalized unit.
    Delivery,
    /// Selected and ranked, then omitted by the budget.
    Packing,
    /// Delivered whole (or as its lexical span).
    Delivered,
}

fn attribute(
    path: &str,
    windows: &BTreeSet<String>,
    lexical: &BTreeSet<String>,
    selected: &Facts,
    parsed: &V2Response,
) -> Owner {
    if !windows.contains(path) && !lexical.contains(path) {
        return Owner::Retrieval;
    }
    if !selected.paths.iter().any(|selected| selected == path) {
        return Owner::Merge;
    }
    match item_of(parsed, path) {
        None => Owner::Packing,
        Some(item)
            if item
                .neural
                .as_ref()
                .is_some_and(|n| n.selection == "preview") =>
        {
            Owner::Delivery
        }
        Some(_) => Owner::Delivered,
    }
}

// ---------------------------------------------------------------------------
// FR-003 / SC-002: the vocabulary-gap fixture through the final response.
// ---------------------------------------------------------------------------

#[test]
fn the_fixture_spans_are_frozen_to_the_fixture_bytes() {
    let corpus = Corpus::unprepared(&[]);
    let dusk = corpus.source(DUSK_PATH);
    assert_eq!(dusk.len() as u64, DUSK_EVIDENCE.end);
    let ledger = corpus.source(LEDGER_PATH);
    assert_eq!(ledger.len() as u64, LEDGER_LEN);
    assert_eq!(
        ledger.find("## Reimbursement window"),
        Some(LEDGER_EVIDENCE.start as usize)
    );
    assert_eq!(LEDGER_EVIDENCE.end, LEDGER_LEN);
    let code = corpus.source(CODE_PATH);
    assert_eq!(code.len() as u64, CODE_LEN);
    assert_eq!(
        code.find("/// Milliseconds to pause"),
        Some(CODE_EVIDENCE.start as usize)
    );
    assert!(code[..CODE_EVIDENCE.end as usize].ends_with("doubled.min(30_000)\n}"));
    // The lexical baseline finds NOTHING for any of the three queries: the
    // evidence below can only arrive through the dense window.
    for query in [DUSK_QUERY, LEDGER_QUERY, CODE_QUERY] {
        let baseline = corpus.baseline(query, BUDGET);
        assert!(
            baseline.batch.items.is_empty(),
            "{query:?} must have no lexical candidates"
        );
    }
}

#[test]
fn vocabulary_gap_evidence_reaches_the_final_response_within_the_normal_budget() {
    let corpus = Corpus::new(&[]);
    assert!(corpus.report.index_published, "{:?}", corpus.report);
    let slot = corpus.slot();
    let cases = [
        (DUSK_QUERY, DUSK_PATH, DUSK_EVIDENCE),
        (LEDGER_QUERY, LEDGER_PATH, LEDGER_EVIDENCE),
        (CODE_QUERY, CODE_PATH, CODE_EVIDENCE),
    ];
    let mut delivered = Vec::new();
    for (query, path, evidence) in cases {
        let answer = corpus.context(&slot, query, BUDGET);
        assert_eq!(answer.word(), Some("ready"), "{query:?}");
        let item = answer.item(path).unwrap_or_else(|| {
            panic!("{path}: no evidence for {query:?}:\n{}", answer.packed.text)
        });
        let neural = item.neural.as_ref().expect("a neural item carries a tag");
        assert_eq!(neural.selection, "whole_unit", "{path}");
        assert_eq!(neural.matched, None, "whole_unit repeats no handle");
        let (_, start, end) = span_of(&item.handle);
        assert!(
            start <= evidence.start && evidence.end <= end,
            "{path}: delivered {start}..{end} must cover the frozen evidence {evidence:?}"
        );
        // Source freshness: the delivered bytes are the live file's bytes and
        // the handle names the live content hash.
        let content = corpus.source(path);
        assert_eq!(item.body, content[start as usize..end as usize], "{path}");
        let sha32 = &context_foundry::digest(content.as_bytes())[..32];
        assert!(
            item.handle.contains(&format!("@{sha32}.")),
            "{}",
            item.handle
        );
        // The exact output budget.
        assert!(answer.packed.tokens <= BUDGET);
        assert_eq!(
            answer.packed.tokens,
            response::count_tokens(&answer.packed.text)
        );
        // The provider saw the exact prefixed query once.
        assert_eq!(
            corpus.probe.queries().last().map(String::as_str),
            Some(&*format!("query: {query}"))
        );
        delivered.push(serde_json::json!({
            "query": query, "path": path,
            "frozen_evidence": [evidence.start, evidence.end],
            "delivered": [start, end],
            "selection": neural.selection,
            "delivered_tokens": answer.packed.tokens,
        }));
    }
    // The short document fits whole; the long document delivers only the
    // unit holding its tail, not the whole file; the code file delivers
    // the unit around the retry function, not the whole file.
    let dusk = corpus.context(&slot, DUSK_QUERY, BUDGET);
    assert_eq!(
        span_of(&dusk.item(DUSK_PATH).unwrap().handle).1
            ..span_of(&dusk.item(DUSK_PATH).unwrap().handle).2,
        DUSK_EVIDENCE
    );
    let ledger = corpus.context(&slot, LEDGER_QUERY, BUDGET);
    let (_, start, end) = span_of(&ledger.item(LEDGER_PATH).unwrap().handle);
    assert!(end - start <= UNIT_BYTES, "{start}..{end}");
    assert!(start > 0 && end == LEDGER_LEN, "{start}..{end}");
    assert!(
        !ledger
            .item(LEDGER_PATH)
            .unwrap()
            .body
            .contains("Shipping labels")
    );
    let code = corpus.context(&slot, CODE_QUERY, BUDGET);
    let (_, start, end) = span_of(&code.item(CODE_PATH).unwrap().handle);
    // The code file is longer than one embedding unit (checked at compile
    // time beside the constants), so the delivered unit is part of the file.
    assert!(end - start <= UNIT_BYTES, "{start}..{end}");
    record("vocabulary-gap-fixture", serde_json::json!(delivered));
}

#[test]
fn search_returns_locators_and_never_claims_a_whole_unit() {
    let corpus = Corpus::new(&[]);
    let slot = corpus.slot();
    let answer = corpus.search(&slot, DUSK_QUERY, 10);
    assert_eq!(answer.word(), Some("ready"));
    let item = answer.item(DUSK_PATH).expect("the dense hit is a locator");
    assert_eq!(item.label.as_deref(), Some("semantic"));
    assert!(item.neural.is_none(), "a locator carries no selection tag");
    assert!(!answer.packed.text.contains("[whole_unit]"));
    assert!(!answer.packed.text.contains("[preview"));
    assert!(!answer.packed.text.contains("[lexical_span"));
}

#[test]
fn exact_locator_fixtures_keep_passing_with_semantics_on() {
    let corpus = Corpus::new(&[]);
    let slot = corpus.slot();
    let baseline = corpus
        .engine()
        .search_candidates("parse_record", None, 10, &Control::unbounded())
        .unwrap();
    let base_first = baseline.items[0].handle.clone().unwrap();
    assert_eq!(baseline.items[0].tier, 1, "an exact definition");
    assert_eq!(base_first.path, "src/records.rs");

    // search: the exact definition is the first locator, byte for byte.
    let search = corpus.search(&slot, "parse_record", 10);
    let base_outcome = Engine::search_outcome(baseline);
    let base_text = response::pack_search(
        &base_outcome,
        Budget::request(BUDGET),
        &response::stdout_bytes,
    )
    .unwrap()
    .text;
    assert_eq!(
        search.packed.text.lines().nth(1),
        base_text.lines().nth(1),
        "the first locator line is the baseline's"
    );
    assert_eq!(search.facts.tiers[0], 1);
    assert!(
        search.facts.tiers.windows(2).all(|pair| pair[0] <= pair[1]),
        "every exact definition precedes every other candidate: {:?}",
        search.facts.tiers
    );

    // context: the exact definition delivers the same bytes first.
    let base_context = corpus.baseline("parse_record", BUDGET);
    let context = corpus.context(&slot, "parse_record", BUDGET);
    let base_item = &base_context.parsed.items[0];
    let item = &context.parsed.items[0];
    assert_eq!(item.handle, base_item.handle);
    assert_eq!(item.body, base_item.body);
    assert_eq!(context.facts.tiers[0], 1);
}

// ---------------------------------------------------------------------------
// The merge: exact definitions first, then k = 60 over lexical 256 + dense 64.
// ---------------------------------------------------------------------------

fn unit(path: &str, start: u64, end: u64) -> MergeUnit {
    MergeUnit {
        path: path.to_owned(),
        start,
        end,
    }
}

#[test]
fn the_merge_orders_exact_definitions_then_reciprocal_rank_fusion_with_stable_ties() {
    assert_eq!(RRF_K, 60);
    assert_eq!(DENSE_WINDOW, 64);
    let exact = [unit("z.rs", 0, 9)];
    let lexical = [unit("m.rs", 0, 9), unit("b.rs", 0, 9), unit("z.rs", 0, 9)];
    let dense = [unit("b.rs", 0, 9), unit("a.rs", 4, 9), unit("a.rs", 0, 9)];
    let fused = merge::fuse(&exact, &lexical, &dense);
    let order: Vec<(String, u64)> = fused
        .iter()
        .map(|candidate| (candidate.unit.path.clone(), candidate.unit.start))
        .collect();
    assert_eq!(
        order,
        [
            ("z.rs".to_owned(), 0), // the exact definition, once
            ("b.rs".to_owned(), 0), // 1/62 (lexical rank 1) + 1/61 (dense rank 0)
            ("m.rs".to_owned(), 0), // 1/61
            ("a.rs".to_owned(), 4), // 1/62
            ("a.rs".to_owned(), 0), // 1/63
        ]
    );
    assert_eq!(fused[0].tier, 1);
    let b = fused.iter().find(|c| c.unit.path == "b.rs").unwrap();
    assert!(
        (b.score - (1.0 / 62.0 + 1.0 / 61.0)).abs() < 1e-6,
        "{}",
        b.score
    );
    assert_eq!(b.dense_rank, Some(0));
    assert!(b.lexical);
    // Equal scores order by path, then start: deterministic.
    let tied = merge::fuse(&[], &[unit("q.rs", 7, 9)], &[unit("p.rs", 3, 9)]);
    assert_eq!(tied[0].unit.path, "p.rs");
    assert_eq!(tied[1].unit.path, "q.rs");
}

#[test]
fn repeated_semantic_requests_deliver_the_same_order() {
    let corpus = Corpus::new(&[]);
    let slot = corpus.slot();
    let query = "twilight onset shift dock desk badge lot";
    let first = corpus.context(&slot, query, BUDGET);
    for _ in 0..3 {
        let again = corpus.context(&slot, query, BUDGET);
        assert_eq!(again.packed.text, first.packed.text);
    }
}

// ---------------------------------------------------------------------------
// Cold and warm queries; the 1500 ms / remaining-deadline cutoff; fallbacks.
// ---------------------------------------------------------------------------

#[test]
fn cold_and_warm_queries_load_the_index_once_and_spend_one_embedding_each() {
    let corpus = Corpus::new(&[]);
    let runtime = corpus.runtime();
    let after_prepare = corpus.probe.document_calls();
    let control = Control::unbounded();
    let deadline = Instant::now() + Duration::from_secs(30);
    let cold_started = Instant::now();
    let cold = runtime
        .window(corpus.engine(), DUSK_QUERY, deadline, &control)
        .unwrap();
    let cold_ms = cold_started.elapsed().as_millis();
    let warm_started = Instant::now();
    let warm = runtime
        .window(corpus.engine(), DUSK_QUERY, deadline, &control)
        .unwrap();
    let warm_ms = warm_started.elapsed().as_millis();
    assert_eq!(keys(&cold), keys(&warm));
    assert!(!cold.hits.is_empty() && cold.hits.len() <= DENSE_WINDOW);
    assert!(
        cold.hits
            .windows(2)
            .all(|pair| pair[0].distance <= pair[1].distance),
        "nearest first"
    );
    assert_eq!(corpus.probe.query_calls(), 2, "one embedding per request");
    assert_eq!(
        corpus.probe.document_calls(),
        after_prepare,
        "no document call per query"
    );
    assert_eq!(
        corpus.probe.queries(),
        vec![format!("query: {DUSK_QUERY}"); 2]
    );
    assert!(
        cold.units(&cold.hits[0])
            .iter()
            .any(|unit| unit.path == DUSK_PATH),
        "the nearest vector is the dusk document"
    );
    record(
        "cold-warm-query",
        serde_json::json!({"cold_ms": cold_ms, "warm_ms": warm_ms, "window_hits": cold.hits.len()}),
    );
}

#[test]
fn a_stalled_provider_is_cut_at_the_ceiling_and_keeps_its_slot_busy_until_it_finishes() {
    let corpus = Corpus::new(&[]);
    let slot = corpus.slot();
    let query = "shift lead rota";
    let baseline = corpus.baseline(query, BUDGET);
    assert!(!baseline.batch.items.is_empty());

    corpus
        .probe
        .set(QueryMode::Sleep(Duration::from_millis(2600)));
    let started = Instant::now();
    let cut = corpus.context(&slot, query, BUDGET);
    let elapsed = started.elapsed();
    assert!(
        elapsed >= QUERY_CEILING - Duration::from_millis(100)
            && elapsed < Duration::from_millis(2400),
        "cut at the 1500 ms ceiling, not at the provider's 2600 ms: {elapsed:?}"
    );
    assert_eq!(cut.word(), Some("fallback:provider_timeout"));
    assert_eq!(
        cut.facts.paths, baseline.facts.paths,
        "baseline results intact"
    );
    assert_eq!(
        cut.packed.text.lines().skip(1).collect::<Vec<_>>(),
        baseline.packed.text.lines().skip(1).collect::<Vec<_>>()
    );

    // The late call still holds the one slot: the next request is refused
    // by name at once, queues nothing and still answers from the baseline.
    let started = Instant::now();
    let busy = corpus.context(&slot, query, BUDGET);
    assert!(
        started.elapsed() < Duration::from_millis(500),
        "{:?}",
        started.elapsed()
    );
    assert_eq!(busy.word(), Some("fallback:provider_busy"));
    assert_eq!(busy.facts.paths, baseline.facts.paths);
    assert_eq!(corpus.probe.query_calls(), 1, "no second job was queued");

    // After the old call really ends the runtime answers again.
    corpus.probe.set(QueryMode::Answer);
    let deadline = Instant::now() + Duration::from_secs(6);
    loop {
        let again = corpus.context(&slot, query, BUDGET);
        if again.word() == Some("ready") {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "the slot never drained: {:?}",
            again.word()
        );
        std::thread::sleep(Duration::from_millis(100));
    }
    assert_eq!(corpus.probe.query_calls(), 2);
}

#[test]
fn the_query_embedding_waits_at_most_half_the_remaining_read_deadline() {
    let corpus = Corpus::new(&[]);
    let runtime = corpus.runtime();
    corpus.probe.set(QueryMode::Sleep(Duration::from_secs(3)));
    let started = Instant::now();
    let remaining = Duration::from_millis(600);
    let fallback = runtime
        .window(
            corpus.engine(),
            DUSK_QUERY,
            started + remaining,
            &Control::unbounded(),
        )
        .unwrap_err();
    let elapsed = started.elapsed();
    assert!(
        elapsed >= remaining / 2 - Duration::from_millis(20) && elapsed < remaining,
        "min(1500 ms, half of 600 ms remaining), never the whole remainder: {elapsed:?}"
    );
    assert_eq!(fallback.code, "provider_timeout");
    // A deadline that already passed spends no model call at all.
    let calls = corpus.probe.query_calls();
    let fallback = runtime
        .window(
            corpus.engine(),
            DUSK_QUERY,
            Instant::now() - Duration::from_millis(1),
            &Control::unbounded(),
        )
        .unwrap_err();
    assert_eq!(fallback.code, "provider_timeout");
    assert_eq!(corpus.probe.query_calls(), calls);
}

/// The remaining-deadline cut through the REAL request path: the request's
/// own expiring control bounds the embedding, and the named fallback's
/// baseline retrieval, final read and delivery still happen before that
/// control expires.
#[test]
fn an_expiring_request_still_delivers_the_baseline_with_the_named_fallback() {
    let corpus = Corpus::new(&[]);
    let slot = corpus.slot();
    let query = "shift lead rota";
    let baseline = corpus.baseline(query, BUDGET);
    assert!(!baseline.batch.items.is_empty());
    corpus.probe.set(QueryMode::Sleep(Duration::from_secs(3)));
    let started = Instant::now();
    let remaining = Duration::from_millis(800);
    let control = Control::with_deadline(started + remaining);
    let combined = mcp::context_primary(
        &slot,
        corpus.engine(),
        query,
        Strategy::Search,
        &control,
        false,
    )
    .expect("the fallback is delivered before the request deadline");
    let elapsed = started.elapsed();
    assert!(
        elapsed >= remaining / 2 - Duration::from_millis(20) && elapsed < remaining,
        "{elapsed:?}"
    );
    assert!(
        control.check().is_ok(),
        "time remained after the final read"
    );
    let answer = Answer::of(combined.batch, BUDGET);
    assert_eq!(answer.word(), Some("fallback:provider_timeout"));
    assert_eq!(answer.facts.paths, baseline.facts.paths);
    assert_eq!(
        answer.packed.text.lines().skip(1).collect::<Vec<_>>(),
        baseline.packed.text.lines().skip(1).collect::<Vec<_>>()
    );
    assert_eq!(corpus.probe.query_calls(), 1);
}

#[test]
fn provider_failures_fall_back_by_name_with_baseline_results() {
    let corpus = Corpus::new(&[]);
    let query = "shift lead rota";
    let baseline = corpus.baseline(query, BUDGET);
    for (mode, expected) in [
        (
            QueryMode::Fail(ProviderError::WorkerExited("the worker is gone".into())),
            "provider_exited",
        ),
        (QueryMode::Short, "provider_malformed"),
        (QueryMode::Fail(ProviderError::Cancelled), "cancelled"),
    ] {
        corpus.probe.set(mode);
        let slot = corpus.slot();
        let answer = corpus.context(&slot, query, BUDGET);
        let word = answer.word().expect("a semantic segment");
        assert!(word.starts_with(&format!("fallback:{expected}")), "{word}");
        assert_eq!(answer.facts.paths, baseline.facts.paths);
        let search = corpus.search(&slot, query, 10);
        assert!(
            search
                .word()
                .unwrap()
                .starts_with(&format!("fallback:{expected}"))
        );
    }
}

#[test]
fn a_refused_provider_start_is_the_named_fallback_of_every_request() {
    let corpus = Corpus::new(&[]);
    let slot = mcp::semantic_slot(Some(SemanticServing::with_provider(
        corpus.profile_path.clone(),
        Box::new(|| {
            Err(ProviderError::IsolationUnavailable(
                "the fixture refuses".into(),
            ))
        }),
    )))
    .unwrap();
    let baseline = corpus.baseline("shift lead rota", BUDGET);
    let answer = corpus.context(&slot, "shift lead rota", BUDGET);
    let word = answer.word().expect("a semantic segment");
    assert!(word.starts_with("fallback:isolation_unavailable"), "{word}");
    assert!(
        !word.contains(" · "),
        "a provider message cannot forge a header segment"
    );
    assert_eq!(answer.facts.paths, baseline.facts.paths);
}

// ---------------------------------------------------------------------------
// Profile mismatch, source edit/delete, partial coverage.
// ---------------------------------------------------------------------------

#[test]
fn a_store_prepared_for_another_profile_is_named_unavailable_and_baseline_is_intact() {
    let corpus = Corpus::new(&[]);
    let other_path = testkit::write_semantic_profile(corpus.dir.path(), "other", |descriptor| {
        descriptor.quantization = "affine bits=8".into();
    });
    let other = SemanticProfile::load(&other_path).unwrap();
    assert_ne!(other.descriptor.digest(), corpus.digest());
    let probe = Probe::default();
    let slot = mcp::semantic_slot(Some(SemanticServing::with_provider(
        other_path,
        maker(other.descriptor.clone(), probe.clone()),
    )))
    .unwrap();
    let baseline = corpus.baseline("shift lead rota", BUDGET);
    let answer = corpus.context(&slot, "shift lead rota", BUDGET);
    let word = answer.word().expect("a semantic segment");
    assert!(word.starts_with("fallback:profile_mismatch"), "{word}");
    assert_eq!(answer.facts.paths, baseline.facts.paths);
    assert_eq!(
        probe.query_calls(),
        0,
        "a refused store spends no embedding"
    );
    // The store still serves the profile it was prepared for.
    let own = corpus.slot();
    assert_eq!(
        corpus.context(&own, DUSK_QUERY, BUDGET).word(),
        Some("ready")
    );
}

#[test]
fn a_source_edit_never_serves_stale_bytes_and_reprepare_embeds_only_the_edit() {
    let mut corpus = Corpus::new(&[]);
    let slot = corpus.slot();
    assert!(
        corpus
            .context(&slot, LEDGER_QUERY, BUDGET)
            .item(LEDGER_PATH)
            .is_some()
    );

    // Edit the long document: its prepared units no longer describe it.
    let edited = format!(
        "{}\n## Appendix\n\nNothing else changed.\n",
        corpus.source(LEDGER_PATH)
    );
    std::fs::write(corpus.root.join(LEDGER_PATH), &edited).unwrap();
    corpus
        .engine_mut()
        .replace_source(LEDGER_PATH, &edited)
        .unwrap();
    corpus.engine_mut().refresh(&Control::unbounded()).unwrap();
    let stale = corpus.context(&slot, LEDGER_QUERY, BUDGET);
    assert!(
        stale.item(LEDGER_PATH).is_none(),
        "stale prepared bytes are never delivered"
    );
    assert_eq!(
        stale.word(),
        Some("partial"),
        "the serving generation predates this source revision"
    );
    // Every dropped prepared unit is attributed in the emitted header.
    assert!(stale.batch.counters.stale >= 1);
    assert!(
        stale
            .parsed
            .header
            .contains(&format!("stale:{}", stale.batch.counters.stale)),
        "{:?}",
        stale.parsed.header
    );
    // An untouched document keeps serving from its unchanged vectors.
    let dusk = corpus.context(&slot, DUSK_QUERY, BUDGET);
    assert!(dusk.item(DUSK_PATH).is_some());
    assert_eq!(dusk.word(), Some("partial"));

    // Re-prepare: only the edited document's units are embedded.
    let calls_before = corpus.probe.document_calls();
    let started = Instant::now();
    let report = corpus.prepare();
    let edit_ms = started.elapsed().as_millis();
    let slot = corpus.slot();
    assert!(report.embedded_units >= 1);
    assert!(
        report.embedded_units < corpus.report.embedded_units,
        "unchanged documents reuse their vectors: {report:?}"
    );
    assert_eq!(
        corpus.probe.document_calls() - calls_before,
        report.document_calls
    );
    let fresh = corpus.context(&slot, LEDGER_QUERY, BUDGET);
    let item = fresh
        .item(LEDGER_PATH)
        .expect("the edited document is served again");
    let (_, start, end) = span_of(&item.handle);
    assert_eq!(item.body, edited[start as usize..end as usize]);
    assert_eq!(fresh.word(), Some("ready"));
    record(
        "edit-cost",
        serde_json::json!({
            "edit_prepare_ms": edit_ms,
            "embedded_units": report.embedded_units,
            "document_calls": report.document_calls,
            "full_prepare_embedded_units": corpus.report.embedded_units,
        }),
    );
}

#[test]
fn a_deleted_source_is_never_delivered_from_its_prepared_vectors() {
    let mut corpus = Corpus::new(&[]);
    let slot = corpus.slot();
    assert!(
        corpus
            .context(&slot, DUSK_QUERY, BUDGET)
            .item(DUSK_PATH)
            .is_some()
    );
    corpus.engine().delete_source(DUSK_PATH).unwrap();
    corpus.engine_mut().refresh(&Control::unbounded()).unwrap();
    let gone = corpus.context(&slot, DUSK_QUERY, BUDGET);
    assert!(gone.item(DUSK_PATH).is_none(), "{}", gone.packed.text);
    assert!(gone.batch.items.iter().all(|item| {
        item.handle
            .as_ref()
            .is_none_or(|handle| handle.path != DUSK_PATH)
    }));
    // The source revision moved past the serving generation.
    assert_eq!(gone.word(), Some("partial"));
    // The deleted document's vector still sits in the prepared index and the
    // dense window still names its location; the final read drops it and
    // the response attributes the drop as stale.
    let window = corpus
        .runtime()
        .window(
            corpus.engine(),
            DUSK_QUERY,
            Instant::now() + Duration::from_secs(30),
            &Control::unbounded(),
        )
        .unwrap();
    assert!(window_paths(&window).contains(DUSK_PATH));
    assert_eq!(gone.batch.counters.stale, 1, "{:?}", gone.batch.counters);
    assert!(
        gone.parsed.header.contains(&"stale:1".to_owned()),
        "{:?}",
        gone.parsed.header
    );
    // A stale drop is never a packing omission: every remaining candidate
    // is either shown or counted omitted.
    assert_eq!(
        gone.parsed.items.len() + gone.packed.omitted,
        gone.batch.items.len()
    );
    let search = corpus.search(&slot, DUSK_QUERY, 10);
    assert!(search.item(DUSK_PATH).is_none());
    assert!(search.parsed.header.contains(&"stale:1".to_owned()));
}

#[test]
fn partial_coverage_serves_prepared_vectors_beside_baseline_candidates_and_says_so() {
    let mut corpus = Corpus::new(&[]);
    let slot = corpus.slot();
    let added = "# Kiln log\n\nThe kiln was fired twice this week.\n";
    corpus
        .engine_mut()
        .replace_source("docs/kiln.md", added)
        .unwrap();
    corpus.engine_mut().refresh(&Control::unbounded()).unwrap();
    let answer = corpus.context(&slot, "twilight onset kiln", BUDGET);
    assert_eq!(answer.word(), Some("partial"));
    let kiln = answer
        .item("docs/kiln.md")
        .expect("the unprepared source stays lexical");
    assert!(kiln.neural.is_none(), "a lexical candidate carries no tag");
    let dusk = answer
        .item(DUSK_PATH)
        .expect("prepared vectors still serve");
    assert_eq!(dusk.neural.as_ref().unwrap().selection, "whole_unit");
}

// ---------------------------------------------------------------------------
// Crowding, preview, lexical span, attribution.
// ---------------------------------------------------------------------------

#[test]
fn neural_candidates_crowded_by_other_evidence_are_still_delivered_in_budget() {
    let corpus = Corpus::new(&[]);
    let slot = corpus.slot();
    let query = "twilight onset shift dock desk badge lot";
    let baseline = corpus.baseline(query, BUDGET);
    assert!(
        baseline.batch.items.len() >= 3,
        "{} lexical candidates crowd the response",
        baseline.batch.items.len()
    );
    let answer = corpus.context(&slot, query, BUDGET);
    assert!(answer.batch.items.len() > baseline.batch.items.len());
    let position = answer
        .parsed
        .items
        .iter()
        .position(|item| !item.handle.is_empty() && span_of(&item.handle).0 == DUSK_PATH)
        .unwrap_or_else(|| panic!("dusk not delivered:\n{}", answer.packed.text));
    assert!(
        position < 6,
        "dense evidence kept its fused rank: {position}"
    );
    assert!(answer.packed.tokens <= BUDGET);
}

/// The budgets a sweep tries: wide enough to cross every ladder rung of one
/// unit (preview prefixes, lexical span, whole unit), coarse enough to stay
/// quick.
fn sweep() -> impl Iterator<Item = usize> {
    (40..=700).step_by(4)
}

/// The candidates of `query` and the whole-unit handle delivered for `path`.
fn unit_of(
    corpus: &Corpus,
    slot: &SemanticSlot,
    query: &str,
    path: &str,
) -> (CandidateBatch, String) {
    let whole = corpus.context(slot, query, BUDGET);
    let handle = whole.item(path).expect("the unit").handle.clone();
    (whole.batch, handle)
}

/// The preview item whose matched handle is `unit`, if the response has one.
fn preview_of<'a>(parsed: &'a V2Response, unit: &str) -> Option<&'a V2Item> {
    parsed.items.iter().find(|item| {
        item.neural.as_ref().is_some_and(|neural| {
            neural.selection == "preview" && neural.matched.as_deref() == Some(unit)
        })
    })
}

#[test]
fn a_forced_unlocalized_oversized_hit_is_a_preview_with_truthful_handles_and_a_continuation() {
    let corpus = Corpus::new(&[]);
    let slot = corpus.slot();
    // The retry function sits inside a 942-byte embedding unit that no
    // lexical candidate localizes (the query shares no word with it).
    let (batch, unit_handle) = unit_of(&corpus, &slot, CODE_QUERY, CODE_PATH);
    let (path, unit_start, unit_end) = span_of(&unit_handle);
    assert!(unit_start <= CODE_EVIDENCE.start && CODE_EVIDENCE.end <= unit_end);
    let content = corpus.source(&path);
    let mut previews = 0;
    for tokens in sweep() {
        let Some((packed, parsed)) = pack(&batch, tokens) else {
            continue;
        };
        assert!(packed.tokens <= tokens);
        let Some(item) = preview_of(&parsed, &unit_handle) else {
            continue;
        };
        let neural = item.neural.as_ref().unwrap();
        previews += 1;
        // The matched handle names the whole unit; the item handle names the
        // returned prefix; `next:` names exactly the remainder.
        assert_eq!(neural.matched.as_deref(), Some(unit_handle.as_str()));
        let (_, start, end) = span_of(&item.handle);
        assert_eq!(start, unit_start);
        assert!(
            start < end && end < unit_end,
            "{start}..{end} of {unit_start}..{unit_end}"
        );
        assert_eq!(item.body, content[start as usize..end as usize]);
        let next = neural
            .next
            .as_deref()
            .expect("bytes remain: a continuation");
        assert_eq!(next, handle_with_range(&unit_handle, end, unit_end));
        // Forward progress: the continuation is strictly inside the unit and
        // retrieving it returns exactly the rest; the two reassemble the unit.
        let rest = corpus.engine().retrieve(next, None, 4096).unwrap();
        assert_eq!(
            [item.body.as_bytes(), rest.span.as_slice()].concat(),
            content.as_bytes()[unit_start as usize..unit_end as usize]
        );
    }
    assert!(
        previews >= 3,
        "budgets between header-only and the whole unit preview, got {previews}"
    );
}

#[test]
fn a_lexical_span_inside_a_dense_unit_is_delivered_once_when_the_whole_unit_does_not_fit() {
    let corpus = Corpus::new(&[]);
    let slot = corpus.slot();
    // "fire lane" is a lexical hit inside the Parking and access section; the
    // embedding unit holding it also holds the Warehouse shifts section (the
    // greedy partition joins sections up to 1015 bytes), so the dense unit
    // is wider than the lexical span it contains.
    let query = "fire lane";
    let content = corpus.source(LEDGER_PATH);
    let hit = content.find("fire lane").expect("the fixture text") as u64;
    let whole = corpus.context(&slot, query, BUDGET);
    let units: Vec<&V2Item> = whole
        .parsed
        .items
        .iter()
        .filter(|item| {
            !item.handle.is_empty()
                && span_of(&item.handle).0 == LEDGER_PATH
                && item
                    .neural
                    .as_ref()
                    .is_some_and(|n| n.selection == "whole_unit")
                && span_of(&item.handle).1 <= hit
                && hit < span_of(&item.handle).2
        })
        .collect();
    assert_eq!(
        units.len(),
        1,
        "one dense unit holds the hit:\n{}",
        whole.packed.text
    );
    let unit = units[0].handle.clone();
    let (_, unit_start, unit_end) = span_of(&unit);
    let mut spans = 0;
    for tokens in sweep() {
        let Some((_, parsed)) = pack(&whole.batch, tokens) else {
            continue;
        };
        for item in &parsed.items {
            let Some(neural) = item.neural.as_ref() else {
                continue;
            };
            if neural.selection != "lexical_span" || neural.matched.as_deref() != Some(&unit) {
                continue;
            }
            spans += 1;
            let (_, start, end) = span_of(&item.handle);
            assert!(unit_start <= start && end <= unit_end);
            assert!(start <= hit && hit < end, "{start}..{end} holds {hit}");
            assert!(
                end - start < unit_end - unit_start,
                "narrower than its unit"
            );
            assert_eq!(item.body, content[start as usize..end as usize]);
            // 001 § Deduplication: one handle names these bytes once, even
            // though a lexical candidate carries the same span.
            let same = parsed
                .items
                .iter()
                .filter(|other| other.handle == item.handle)
                .count();
            assert_eq!(same, 1, "{}", item.handle);
        }
    }
    assert!(
        spans >= 1,
        "some budget selects the lexical span:\n{}",
        whole.packed.text
    );
}

#[test]
fn each_missing_evidence_class_is_attributed_to_its_owner() {
    // 1. Retrieval: 70 documents the model finds nearer than the ledger tail
    //    push it out of the 64-hit dense window, and the lexical window never
    //    held it (no shared word). Nothing downstream can recover it.
    let crowded = Corpus::new(&fillers(70));
    let slot = crowded.slot();
    let runtime = crowded.runtime();
    let window = runtime
        .window(
            crowded.engine(),
            LEDGER_QUERY,
            Instant::now() + Duration::from_secs(30),
            &Control::unbounded(),
        )
        .unwrap();
    assert_eq!(window.hits.len(), DENSE_WINDOW);
    let windows = window_paths(&window);
    let lexical: BTreeSet<String> = crowded
        .baseline(LEDGER_QUERY, BUDGET)
        .facts
        .paths
        .into_iter()
        .collect();
    let absent = crowded.context(&slot, LEDGER_QUERY, BUDGET);
    assert!(absent.item(LEDGER_PATH).is_none());
    assert_eq!(
        attribute(
            LEDGER_PATH,
            &windows,
            &lexical,
            &absent.facts,
            &absent.parsed
        ),
        Owner::Retrieval
    );

    // 2. Merge: with one result slot the exact definition (tier 1) wins the
    //    cut; the dusk unit was in the dense window and is dropped by the
    //    fused, capped selection, not by retrieval or packing.
    let corpus = Corpus::new(&[]);
    let slot = corpus.slot();
    let query = "parse_record twilight onset";
    let runtime = corpus.runtime();
    let window = runtime
        .window(
            corpus.engine(),
            query,
            Instant::now() + Duration::from_secs(30),
            &Control::unbounded(),
        )
        .unwrap();
    let windows = window_paths(&window);
    let lexical: BTreeSet<String> = corpus
        .baseline(query, BUDGET)
        .facts
        .paths
        .into_iter()
        .collect();
    let demoted = corpus.search(&slot, query, 1);
    assert_eq!(
        demoted.facts.tiers,
        [1],
        "the exact definition holds the only slot"
    );
    assert!(demoted.facts.truncated);
    assert!(windows.contains(DUSK_PATH));
    assert_eq!(
        attribute(
            DUSK_PATH,
            &windows,
            &lexical,
            &demoted.facts,
            &demoted.parsed
        ),
        Owner::Merge
    );

    // 3. Delivery: an unlocalized unit that does not fit whole is a preview.
    let (batch, unit_handle) = unit_of(&corpus, &slot, CODE_QUERY, CODE_PATH);
    let (_, unit_start, unit_end) = span_of(&unit_handle);
    let windows: BTreeSet<String> = [CODE_PATH.to_owned()].into();
    let lexical = BTreeSet::new();
    let preview = sweep()
        .filter_map(|tokens| pack(&batch, tokens))
        .find(|(_, parsed)| preview_of(parsed, &unit_handle).is_some())
        .expect("some budget previews the unit");
    let selected = facts(&batch);
    assert_eq!(
        attribute(CODE_PATH, &windows, &lexical, &selected, &preview.1),
        Owner::Delivery
    );
    let (_, start, end) = span_of(&preview_of(&preview.1, &unit_handle).unwrap().handle);
    assert!(unit_start <= start && end < unit_end);

    // 4. Packing: ranked first and selected, then omitted by a budget that
    //    holds the header and nothing else.
    let dusk = corpus.context(&slot, DUSK_QUERY, BUDGET);
    let omitted = (1..=400)
        .filter_map(|tokens| pack(&dusk.batch, tokens))
        .find(|(packed, parsed)| packed.omitted > 0 && parsed.items.is_empty())
        .expect("some budget holds only the header");
    let dusk_windows: BTreeSet<String> = [DUSK_PATH.to_owned()].into();
    assert_eq!(
        attribute(
            DUSK_PATH,
            &dusk_windows,
            &BTreeSet::new(),
            &dusk.facts,
            &omitted.1
        ),
        Owner::Packing
    );
    assert!(omitted.0.omitted >= 1);
    // The same evidence at the normal budget is delivered.
    assert_eq!(
        attribute(
            DUSK_PATH,
            &dusk_windows,
            &BTreeSet::new(),
            &dusk.facts,
            &dusk.parsed
        ),
        Owner::Delivered
    );
}

// ---------------------------------------------------------------------------
// No reranker; disabling semantics; baseline unchanged.
// ---------------------------------------------------------------------------

#[test]
fn the_normal_path_spends_one_query_embedding_per_request_and_no_other_model_call() {
    let corpus = Corpus::new(&[]);
    let slot = corpus.slot();
    let documents = corpus.probe.document_calls();
    let inputs = corpus.probe.document_inputs();
    for query in [DUSK_QUERY, LEDGER_QUERY, CODE_QUERY] {
        corpus.context(&slot, query, BUDGET);
        corpus.search(&slot, query, 10);
    }
    // The provider interface has exactly two operations; a reranker would be
    // a third. The six requests produced six query embeddings, nothing else.
    assert_eq!(corpus.probe.query_calls(), 6);
    assert_eq!(corpus.probe.document_calls(), documents);
    assert_eq!(corpus.probe.document_inputs(), inputs);
}

#[test]
fn without_a_profile_requests_are_byte_identical_and_disabling_loses_no_source_or_cache() {
    let mut corpus = Corpus::unprepared(&[]);
    let queries = ["parse_record", "shift lead rota", DUSK_QUERY];
    let before: Vec<(String, String)> = queries
        .iter()
        .map(|query| {
            (
                corpus.baseline(query, BUDGET).packed.text,
                corpus.search(&None, query, 10).packed.text,
            )
        })
        .collect();
    corpus.report = corpus.prepare();
    // The profile-less request path (what an owner without
    // `--semantic-profile` runs) is exactly the baseline, prepared or not.
    for (query, (context, search)) in queries.iter().zip(&before) {
        let combined = mcp::context_primary(
            &None,
            corpus.engine(),
            query,
            Strategy::Search,
            &Control::unbounded(),
            false,
        )
        .unwrap();
        let packed = response::pack_context(
            &combined.batch,
            Budget::request(BUDGET),
            &response::stdout_bytes,
        )
        .unwrap();
        assert_eq!(&packed.text, context, "{query}");
        assert!(!packed.text.contains("semantic:"));
        assert_eq!(
            &corpus.search(&None, query, 10).packed.text,
            search,
            "{query}"
        );
    }
    // Serving never writes: source, partitions, state and vectors are the
    // same rows after any number of semantic requests.
    corpus.engine = None;
    let rows = |store: &Path| {
        (
            testkit::table_rows(store, "semantic_partitions"),
            testkit::table_rows(store, "semantic_state"),
            testkit::semantic_cache_rows(store),
            testkit::knowledge(&testkit::snapshot(store)),
        )
    };
    let before_rows = rows(&corpus.store);
    corpus.engine = Some(Engine::open_existing(&corpus.store).unwrap());
    let slot = corpus.slot();
    for query in queries {
        corpus.context(&slot, query, BUDGET);
        corpus.search(&slot, query, 10);
    }
    corpus.engine = None;
    let after_rows = rows(&corpus.store);
    assert_eq!(before_rows.0, after_rows.0, "partitions");
    assert_eq!(before_rows.1, after_rows.1, "state");
    assert_eq!(before_rows.2, after_rows.2, "cached vectors");
    assert_eq!(before_rows.3, after_rows.3, "every other table");
}

// ---------------------------------------------------------------------------
// The USearch header check T001 deferred to this task.
// ---------------------------------------------------------------------------

#[test]
fn a_usearch_header_that_disagrees_with_the_manifest_is_refused_by_name() {
    let corpus = Corpus::new(&[]);
    let directory = index::generation_dir(&corpus.store, &corpus.digest());
    let manifest_path = directory.join(index::MANIFEST_FILE);
    let mut manifest: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&manifest_path).unwrap()).unwrap();
    let count = manifest["count"].as_u64().unwrap() as usize;
    // A structurally valid index of the right size, dimensions and scalar
    // kind but another metric, written as the manifest's index.
    let options = usearch::IndexOptions {
        dimensions: DIMENSIONS,
        metric: usearch::MetricKind::L2sq,
        quantization: usearch::ScalarKind::F16,
        multi: false,
        ..usearch::IndexOptions::default()
    };
    let foreign = usearch::Index::new(&options).unwrap();
    foreign.reserve(count).unwrap();
    for label in 0..count {
        foreign
            .add(label as u64, &concept_vector(&format!("filler {label}")))
            .unwrap();
    }
    let mut buffer = vec![0u8; foreign.serialized_length()];
    foreign.save_to_buffer(&mut buffer).unwrap();
    let index_path = directory.join(index::INDEX_FILE);
    std::fs::remove_file(&index_path).unwrap();
    std::fs::write(&index_path, &buffer).unwrap();
    manifest["index_sha256"] = serde_json::Value::String(context_foundry::digest(&buffer));
    std::fs::remove_file(&manifest_path).unwrap();
    std::fs::write(&manifest_path, serde_json::to_vec(&manifest).unwrap()).unwrap();

    // Manifest, labels and file hashes now agree: only the library's own
    // header says otherwise.
    let slot = corpus.slot();
    let baseline = corpus.baseline("shift lead rota", BUDGET);
    let answer = corpus.context(&slot, "shift lead rota", BUDGET);
    let word = answer.word().expect("a semantic segment");
    assert!(word.starts_with("fallback:index_geometry"), "{word}");
    assert_eq!(answer.facts.paths, baseline.facts.paths);
    assert_eq!(
        corpus.probe.query_calls(),
        0,
        "the refusal precedes any embedding"
    );
}

// ---------------------------------------------------------------------------
// Cost record: units, input tokens, document calls, preparation and edit
// cost, vector bytes and delivered tokens on this fixture.
// ---------------------------------------------------------------------------

#[test]
fn the_fixture_costs_are_exact_and_recorded() {
    let corpus = Corpus::new(&[]);
    let report = &corpus.report;
    assert!(report.index_published);
    assert_eq!(report.embedded_units, report.eligible_units);
    assert_eq!(corpus.probe.document_inputs(), report.embedded_units);
    assert_eq!(corpus.probe.document_calls(), report.document_calls);
    assert!(
        report.document_calls
            >= report
                .embedded_units
                .div_ceil(provider::DOCUMENT_BATCH as u64)
            && report.document_calls <= report.embedded_units,
        "{report:?}"
    );
    // Units tile every source exactly: the model input is the source bytes
    // plus the 9-byte prefix per unit, one token per byte.
    let source_bytes: u64 = [
        "docs/dusk.md",
        "docs/faq.md",
        LEDGER_PATH,
        "docs/schedule.md",
        CODE_PATH,
        "src/records.rs",
    ]
    .iter()
    .map(|path| corpus.source(path).len() as u64)
    .sum();
    assert_eq!(
        corpus.probe.document_tokens(),
        source_bytes + 9 * report.embedded_units
    );
    // The preparation report carries the same input-token total itself.
    assert_eq!(report.input_tokens, corpus.probe.document_tokens());
    let index_file = index::generation_dir(&corpus.store, &corpus.digest()).join(index::INDEX_FILE);
    let vector_bytes = std::fs::metadata(index_file).unwrap().len();
    let slot = corpus.slot();
    let delivered: Vec<usize> = [DUSK_QUERY, LEDGER_QUERY, CODE_QUERY]
        .iter()
        .map(|query| corpus.context(&slot, query, BUDGET).packed.tokens)
        .collect();
    record(
        "fixture-costs",
        serde_json::json!({
            "units": report.embedded_units,
            "input_tokens": report.input_tokens,
            "document_calls": report.document_calls,
            "cache_bytes": report.cache_bytes,
            "index_entries": report.index_entries,
            "index_file_bytes": vector_bytes,
            "prepare_ms": corpus.prepare_ms,
            "delivered_tokens": delivered,
        }),
    );
}

// ---------------------------------------------------------------------------
// MCP: the owner's resident runtime serves `context` and `search`.
// ---------------------------------------------------------------------------

const TOKEN_ENV: &str = "FOUNDRY_TEST_NEURAL_RETRIEVAL_TOKEN";
const TOKEN: &str = "test-bearer-not-a-secret";

fn set_token_env() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        // SAFETY: called once, before any server thread reads it; the value
        // never changes afterwards.
        unsafe { std::env::set_var(TOKEN_ENV, TOKEN) };
    });
}

fn text_of(result: &rmcp::model::CallToolResult) -> String {
    assert_eq!(result.is_error, Some(false), "tool error: {result:?}");
    let rmcp::model::ContentBlock::Text(text) = &result.content[0] else {
        panic!("expected one text block");
    };
    text.text.to_string()
}

async fn call(
    client: &rmcp::service::RunningService<rmcp::RoleClient, ()>,
    tool: &'static str,
    arguments: serde_json::Value,
) -> String {
    let result = client
        .call_tool(
            CallToolRequestParams::new(tool).with_arguments(arguments.as_object().unwrap().clone()),
        )
        .await
        .unwrap();
    text_of(&result)
}

async fn serve(
    corpus: &mut Corpus,
    semantic: SemanticServing,
) -> (
    rmcp::service::RunningService<rmcp::RoleClient, ()>,
    tokio_util::sync::CancellationToken,
) {
    // The owner opens the store itself: release this process's handle.
    corpus.engine = None;
    set_token_env();
    let shutdown = tokio_util::sync::CancellationToken::new();
    let serve = mcp::serve_http(
        ServerOptions {
            store: corpus.store.clone(),
            root: corpus.root.clone(),
            references: Vec::new(),
            no_memory: false,
            semantic: Some(semantic),
            budget: BudgetConfig::default(),
        },
        HttpOptions {
            port: 0,
            token_env: TOKEN_ENV.to_owned(),
            keep_alive: Duration::from_secs(300),
            shutdown: shutdown.clone(),
        },
    )
    .await
    .unwrap();
    let config = StreamableHttpClientTransportConfig::with_uri(format!(
        "http://127.0.0.1:{}/mcp",
        serve.address.port()
    ))
    .auth_header(TOKEN);
    let transport =
        rmcp::transport::StreamableHttpClientTransport::with_client(reqwest::Client::new(), config);
    let client = ().serve(transport).await.unwrap();
    (client, shutdown)
}

#[tokio::test]
async fn the_mcp_owner_serves_semantic_context_and_search_from_one_resident_runtime() {
    let mut corpus = Corpus::new(&[]);
    let semantic = SemanticServing::with_provider(
        corpus.profile_path.clone(),
        maker(corpus.profile.descriptor.clone(), corpus.probe.clone()),
    );
    let (client, shutdown) = serve(&mut corpus, semantic).await;
    let calls_at_start = corpus.probe.query_calls();
    assert_eq!(
        calls_at_start, 0,
        "the resident provider is built, not queried, at startup"
    );

    let text = call(
        &client,
        "context",
        serde_json::json!({"query": DUSK_QUERY, "tokens": BUDGET}),
    )
    .await;
    let parsed = parse_v2(&text).unwrap();
    assert_eq!(header_word(&parsed), Some("ready"));
    let item = item_of(&parsed, DUSK_PATH).expect("dense evidence over MCP");
    assert_eq!(item.neural.as_ref().unwrap().selection, "whole_unit");
    assert_eq!(
        span_of(&item.handle).1..span_of(&item.handle).2,
        DUSK_EVIDENCE
    );
    assert!(response::count_tokens(&text) <= BUDGET);

    let text = call(
        &client,
        "search",
        serde_json::json!({"query": DUSK_QUERY, "limit": 10}),
    )
    .await;
    let parsed = parse_v2(&text).unwrap();
    assert_eq!(header_word(&parsed), Some("ready"));
    assert_eq!(
        item_of(&parsed, DUSK_PATH).unwrap().label.as_deref(),
        Some("semantic")
    );
    assert_eq!(
        corpus.probe.query_calls(),
        2,
        "one embedding per request, one worker"
    );
    client.cancel().await.unwrap();
    shutdown.cancel();
}

#[tokio::test]
async fn the_mcp_owner_reports_a_refused_worker_by_name_and_keeps_serving_baseline() {
    let mut corpus = Corpus::new(&[]);
    let semantic = SemanticServing::with_provider(
        corpus.profile_path.clone(),
        Box::new(|| {
            Err(ProviderError::IsolationUnavailable(
                "the fixture refuses".into(),
            ))
        }),
    );
    let (client, shutdown) = serve(&mut corpus, semantic).await;
    let text = call(
        &client,
        "context",
        serde_json::json!({"query": "shift lead rota", "tokens": BUDGET}),
    )
    .await;
    let parsed = parse_v2(&text).unwrap();
    let word = header_word(&parsed).expect("a semantic segment");
    assert!(word.starts_with("fallback:isolation_unavailable"), "{word}");
    assert!(!parsed.items.is_empty(), "baseline results are intact");
    client.cancel().await.unwrap();
    shutdown.cancel();
}

// ---------------------------------------------------------------------------
// Review round 1 (Review009T2 M1-M8, the v1 format, supervisor fail-closed).
// ---------------------------------------------------------------------------

/// M1: the dense window honours the request's path restriction exactly as
/// the lexical tiers do: the file itself or a component-boundary subtree.
#[test]
fn semantic_search_honours_file_and_component_boundary_path_filters() {
    let corpus = Corpus::new(&[
        (
            "src/a/glow.md".to_owned(),
            "# Glow\n\nThe amber glow at sundown, dusk over the horizon.\n".to_owned(),
        ),
        (
            "src/ab.md".to_owned(),
            "# Glow notes\n\nSunset and nightfall: amber dusk on the horizon.\n".to_owned(),
        ),
    ]);
    let slot = corpus.slot();
    let paths = |answer: &SearchAnswer| -> BTreeSet<String> {
        answer.facts.paths.iter().cloned().collect()
    };
    // Unrestricted, the dense window reaches all three dusk-like documents.
    let open = paths(&corpus.search(&slot, DUSK_QUERY, 10));
    for path in [DUSK_PATH, "src/a/glow.md", "src/ab.md"] {
        assert!(open.contains(path), "{path}: {open:?}");
    }
    // A subtree stops at the component boundary: `src/a` never admits
    // `src/ab.md`, and nothing outside it is returned.
    let subtree = corpus.search_at(&slot, DUSK_QUERY, Some("src/a"), 10);
    assert_eq!(subtree.word(), Some("ready"));
    assert_eq!(
        paths(&subtree),
        BTreeSet::from(["src/a/glow.md".to_owned()])
    );
    // A file filter admits that file only.
    let file = corpus.search_at(&slot, DUSK_QUERY, Some("src/ab.md"), 10);
    assert_eq!(paths(&file), BTreeSet::from(["src/ab.md".to_owned()]));
    // Filtered-out locations are not candidates, so none is counted stale.
    assert!(
        !subtree
            .parsed
            .header
            .iter()
            .any(|segment| segment.starts_with("stale:"))
    );
}

/// M2: a definition split into many search documents counts once — in the
/// fusion and against the per-file cap — so a second definition in the
/// same file is still returned, and neither is repeated.
#[test]
fn a_split_definition_is_one_result_beside_a_second_definition_in_its_file() {
    let mut source = String::from("pub fn split_alpha() -> u64 {\n    let mut total = 0u64;\n");
    for i in 0..1800 {
        source.push_str(&format!("    total += {i};\n"));
    }
    source.push_str("    total\n}\n\npub fn split_beta() -> u64 {\n    7\n}\n");
    // Regions over 8192 bytes become 4096-byte search documents: far more
    // copies of `split_alpha` than the four-per-file cap.
    assert!(source.len() > 8 * 4096);
    let beta = source.find("pub fn split_beta").unwrap() as u64;
    let corpus = Corpus::new(&[("src/split.rs".to_owned(), source)]);
    let slot = corpus.slot();
    let answer = corpus.search(&slot, "split_alpha split_beta", 10);
    let spans: Vec<(u64, u64)> = answer
        .parsed
        .items
        .iter()
        .filter(|item| !item.handle.is_empty())
        .map(|item| span_of(&item.handle))
        .filter(|(path, ..)| path == "src/split.rs")
        .map(|(_, start, end)| (start, end))
        .collect();
    let unique: BTreeSet<(u64, u64)> = spans.iter().copied().collect();
    assert_eq!(spans.len(), unique.len(), "no unit twice: {spans:?}");
    assert!(
        spans
            .iter()
            .any(|&(start, end)| start == 0 && end > 8 * 4096 && end <= beta),
        "split_alpha once: {spans:?}"
    );
    assert!(
        spans
            .iter()
            .any(|&(start, end)| start <= beta && beta < end && end - start < 64),
        "split_beta beside it: {spans:?}"
    );
}

/// M3: a stale lexical document of the same span neither hides nor
/// coalesces with the current dense unit; it is dropped and attributed.
#[test]
fn a_stale_lexical_document_never_hides_a_current_dense_unit_of_the_same_span() {
    const BEFORE: &str = "fn freshness_probe() { let x = 1; }";
    const AFTER: &str = "fn freshness_probe() { let x = 2; }";
    let mut corpus = Corpus::new(&[("src/fresh.rs".to_owned(), BEFORE.to_owned())]);
    // Same byte span, new bytes, lexical refresh left pending: the lexical
    // index still holds the old version's document.
    corpus
        .engine_mut()
        .replace_source("src/fresh.rs", AFTER)
        .unwrap();
    // Preparation reads the current source and leaves the lexical index.
    corpus.prepare();
    let slot = corpus.slot();
    let answer = corpus.context(&slot, "freshness_probe", BUDGET);
    let item = answer
        .item("src/fresh.rs")
        .unwrap_or_else(|| panic!("the current dense unit is served:\n{}", answer.packed.text));
    assert_eq!(item.body, AFTER);
    assert!(item.neural.is_some(), "it came from the dense window");
    assert!(!answer.packed.text.contains("let x = 1"));
    assert!(
        answer.batch.counters.stale >= 1,
        "{:?}",
        answer.batch.counters
    );
    assert!(
        answer
            .parsed
            .header
            .contains(&format!("stale:{}", answer.batch.counters.stale))
    );
}

/// The "fire lane" candidates: the batch, the dense unit whose selected
/// lexical span is exactly a lexical candidate's unit, and that candidate.
fn fire_lane(corpus: &Corpus, slot: &SemanticSlot) -> (CandidateBatch, usize, usize) {
    let batch = corpus.context(slot, "fire lane", BUDGET).batch;
    for (unit, item) in batch.items.iter().enumerate() {
        let Some((span, ..)) = item.semantic.as_ref().and_then(|e| e.span.as_ref()) else {
            continue;
        };
        let span = span.to_v2();
        let lexical = batch.items.iter().position(|other| {
            other.semantic.is_none()
                && other
                    .handle
                    .as_ref()
                    .is_some_and(|handle| handle.to_v2() == span)
        });
        if let Some(lexical) = lexical {
            return (batch, unit, lexical);
        }
    }
    panic!("no dense unit's span is a lexical candidate");
}

/// M4: dense first, lexical second — the later lexical candidate whose
/// identity the dense unit already delivered as its span merges into it:
/// it is neither delivered again nor counted omitted.
#[test]
fn a_lexical_candidate_already_delivered_as_a_neural_span_is_merged_not_charged_twice() {
    let corpus = Corpus::new(&[]);
    let slot = corpus.slot();
    let (mut batch, unit, lexical) = fire_lane(&corpus, &slot);
    let span = batch.items[lexical].handle.as_ref().unwrap().to_v2();
    // Rank the dense unit ahead of the lexical candidate it contains.
    if unit > lexical {
        let dense = batch.items.remove(unit);
        batch.items.insert(lexical, dense);
    }
    let mut merged = 0;
    for tokens in sweep() {
        let Some((packed, parsed)) = pack(&batch, tokens) else {
            continue;
        };
        let copies: Vec<&V2Item> = parsed
            .items
            .iter()
            .filter(|item| item.handle == span)
            .collect();
        assert!(copies.len() <= 1, "{tokens}:\n{}", packed.text);
        if copies.first().is_some_and(|item| {
            item.neural
                .as_ref()
                .is_some_and(|neural| neural.selection == "lexical_span")
        }) {
            merged += 1;
            // The lexical candidate is neither shown nor counted omitted.
            assert!(
                parsed.items.len() + packed.omitted < batch.items.len(),
                "{tokens}: the lexical candidate merged"
            );
        }
    }
    assert!(
        merged >= 1,
        "some budget delivers the span through the dense unit"
    );
}

/// M5: every source partitioned, only part of the units embedded (a
/// budget-stopped preparation) — known coverage is `partial`, not `ready`.
#[test]
fn a_fully_partitioned_but_partly_embedded_generation_is_partial_not_ready() {
    let mut corpus = Corpus::unprepared(&[]);
    // A slow model under a 7 s budget: the first batch is admitted; after
    // it less than the 5 s publication reserve remains, so the run stops and
    // publishes what it committed.
    corpus.probe.pause_documents(Duration::from_secs(3));
    let report = corpus.prepare_within(7);
    assert_eq!(report.reason_code, Some("budget_exhausted"), "{report:?}");
    assert_eq!(report.document_calls, 1, "{report:?}");
    assert!(report.index_published, "{report:?}");
    let status = corpus
        .engine()
        .semantic_status(&Control::unbounded())
        .unwrap();
    assert_eq!(status.unpartitioned_sources, 0, "{status:?}");
    assert!(
        status.searchable_current_units < status.eligible_units,
        "{status:?}"
    );
    let slot = corpus.slot();
    assert_eq!(
        corpus.context(&slot, DUSK_QUERY, BUDGET).word(),
        Some("partial")
    );
    assert_eq!(corpus.search(&slot, DUSK_QUERY, 10).word(), Some("partial"));
    // Finishing the same preparation, with no source change, is `ready`.
    corpus.probe.pause_documents(Duration::ZERO);
    let report = corpus.prepare();
    assert!(!report.partial, "{report:?}");
    let slot = corpus.slot();
    assert_eq!(
        corpus.context(&slot, DUSK_QUERY, BUDGET).word(),
        Some("ready")
    );
}

/// M6: a request reads no partition row: dense hits expand through the
/// serving generation's label map (counted at the partition-read point).
#[test]
fn a_semantic_request_reads_no_partition_row() {
    use context_foundry::neural::fault_names::PARTITION_READ;
    let corpus = Corpus::new(&[]);
    let slot = corpus.slot();
    // Positive control: the status census reads every partition row.
    let before = fault::reached(PARTITION_READ);
    corpus
        .engine()
        .semantic_status(&Control::unbounded())
        .unwrap();
    assert!(fault::reached(PARTITION_READ) > before);
    let before = fault::reached(PARTITION_READ);
    let context = corpus.context(&slot, DUSK_QUERY, BUDGET);
    let search = corpus.search(&slot, DUSK_QUERY, 10);
    assert_eq!(
        fault::reached(PARTITION_READ),
        before,
        "no per-request partition walk"
    );
    assert!(context.item(DUSK_PATH).is_some());
    assert!(search.item(DUSK_PATH).is_some());
}

/// M7: bytes replaced between verification and restore never reach the
/// loaded index; the next load refuses the replaced file by name.
#[test]
fn bytes_replaced_after_verification_never_reach_the_loaded_index() {
    use context_foundry::neural::fault_names::LOAD_AFTER_VERIFY;
    let corpus = Corpus::new(&[]);
    let window = |runtime: &QueryRuntime| {
        runtime.window(
            corpus.engine(),
            DUSK_QUERY,
            Instant::now() + Duration::from_secs(30),
            &Control::unbounded(),
        )
    };
    let validated = keys(&window(&corpus.runtime()).unwrap());
    // A structurally valid index with the manifest's geometry (cos, f16,
    // same count) whose vectors all point at the ledger concept.
    let directory = index::generation_dir(&corpus.store, &corpus.digest());
    let manifest: serde_json::Value =
        serde_json::from_slice(&std::fs::read(directory.join(index::MANIFEST_FILE)).unwrap())
            .unwrap();
    let count = manifest["count"].as_u64().unwrap() as usize;
    let options = usearch::IndexOptions {
        dimensions: DIMENSIONS,
        metric: usearch::MetricKind::Cos,
        quantization: usearch::ScalarKind::F16,
        multi: false,
        ..usearch::IndexOptions::default()
    };
    let foreign = usearch::Index::new(&options).unwrap();
    foreign.reserve(count).unwrap();
    for label in 0..count {
        foreign
            .add(
                label as u64,
                &concept_vector(&format!("refund credit {label}")),
            )
            .unwrap();
    }
    let mut replacement = vec![0u8; foreign.serialized_length()];
    foreign.save_to_buffer(&mut replacement).unwrap();
    let target = directory.join(index::INDEX_FILE);
    fault::arm(
        LOAD_AFTER_VERIFY,
        0,
        Action::Call(Box::new(move |_| {
            std::fs::remove_file(&target).unwrap();
            std::fs::write(&target, &replacement).unwrap();
        })),
    );
    let before = fault::reached(LOAD_AFTER_VERIFY);
    let loaded = window(&corpus.runtime()).unwrap();
    assert_eq!(fault::reached(LOAD_AFTER_VERIFY), before + 1);
    assert_eq!(
        keys(&loaded),
        validated,
        "the restored index is the verified one"
    );
    let calls = corpus.probe.query_calls();
    let refused = window(&corpus.runtime()).unwrap_err();
    assert_eq!(refused.code, "index_unavailable");
    assert_eq!(
        corpus.probe.query_calls(),
        calls,
        "refused before any embedding"
    );
}

/// A v1 generation (labels without unit locations, no coverage record) is
/// unavailable by name; preparation republishes v2 from the cache with ZERO
/// document calls.
#[test]
fn a_v1_generation_is_unavailable_by_name_and_prepare_republishes_it_without_document_calls() {
    let mut corpus = Corpus::new(&[]);
    let directory = index::generation_dir(&corpus.store, &corpus.digest());
    let labels_path = directory.join(index::LABELS_FILE);
    let manifest_path = directory.join(index::MANIFEST_FILE);
    let read = |path: &Path| -> serde_json::Value {
        serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap()
    };
    let mut labels = read(&labels_path);
    for entry in labels["labels"].as_array_mut().unwrap() {
        entry.as_object_mut().unwrap().remove("units");
    }
    let labels_bytes = serde_json::to_vec(&labels).unwrap();
    let mut manifest = read(&manifest_path);
    manifest["v"] = 1.into();
    manifest["labels_sha256"] = context_foundry::digest(&labels_bytes).into();
    let object = manifest.as_object_mut().unwrap();
    object.remove("source_revision");
    object.remove("coverage");
    std::fs::remove_file(&labels_path).unwrap();
    std::fs::write(&labels_path, &labels_bytes).unwrap();
    std::fs::remove_file(&manifest_path).unwrap();
    std::fs::write(&manifest_path, serde_json::to_vec(&manifest).unwrap()).unwrap();

    let slot = corpus.slot();
    let baseline = corpus.baseline("shift lead rota", BUDGET);
    let answer = corpus.context(&slot, "shift lead rota", BUDGET);
    let word = answer.word().expect("a semantic segment");
    assert!(word.starts_with("fallback:index_unavailable"), "{word}");
    assert_eq!(answer.facts.paths, baseline.facts.paths);
    assert_eq!(corpus.probe.query_calls(), 0, "no embedding for a v1 set");
    drop(slot);

    let calls = corpus.probe.document_calls();
    let report = corpus.prepare();
    assert_eq!(report.document_calls, 0, "{report:?}");
    assert_eq!(corpus.probe.document_calls(), calls);
    assert!(report.index_published && !report.partial, "{report:?}");
    let manifest = read(&manifest_path);
    assert_eq!(manifest["v"], index::GENERATION_VERSION);
    assert_eq!(manifest["coverage"], index::COVERAGE_COMPLETE);
    let slot = corpus.slot();
    let answer = corpus.context(&slot, DUSK_QUERY, BUDGET);
    assert_eq!(answer.word(), Some("ready"));
    assert!(answer.item(DUSK_PATH).is_some());
}

/// A minimal unsigned bundle around the fake worker and a profile whose
/// loader inputs are the empty files the fake descriptor pins.
#[cfg(target_os = "macos")]
fn fake_worker_profile(dir: &Path) -> SemanticProfile {
    use context_foundry::neural::profile::{PROFILE_VERSION, RuntimeSpec, WorkerSpec};
    use context_foundry::neural::worker_runtime::{REAL_LOADER_INPUTS, fake_descriptor};
    let app = dir.join("FoundryEmbedFake.app");
    let macos = app.join("Contents/MacOS");
    std::fs::create_dir_all(&macos).unwrap();
    let exe = macos.join("foundry-embed");
    std::fs::copy(env!("CARGO_BIN_EXE_foundry-embed-fake"), &exe).unwrap();
    std::fs::write(
        app.join("Contents/Info.plist"),
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<plist version=\"1.0\"><dict>\
         <key>CFBundleIdentifier</key><string>org.context-foundry.embed.fake</string>\
         <key>CFBundleExecutable</key><string>foundry-embed</string>\
         <key>CFBundlePackageType</key><string>APPL</string></dict></plist>\n",
    )
    .unwrap();
    let model_dir = dir.join("model");
    std::fs::create_dir_all(&model_dir).unwrap();
    for name in REAL_LOADER_INPUTS {
        std::fs::write(model_dir.join(name), b"").unwrap();
    }
    let requirements = dir.join("requirements.txt");
    std::fs::write(&requirements, b"").unwrap();
    for extra in ["python-home", "site-packages"] {
        std::fs::create_dir_all(dir.join(extra)).unwrap();
    }
    SemanticProfile {
        v: PROFILE_VERSION,
        name: "fake-worker".into(),
        model_dir,
        worker: WorkerSpec {
            bundle: app,
            executable_sha256: context_foundry::digest(&std::fs::read(&exe).unwrap()),
            scratch_root: dir.join("scratch-root"),
        },
        runtime: RuntimeSpec {
            python_home: dir.join("python-home"),
            site_packages: dir.join("site-packages"),
            requirements,
        },
        descriptor: fake_descriptor(),
        memory_ceiling_bytes: 3 << 30,
        load_timeout_seconds: 30,
    }
}

/// The supervised fake worker presented under the corpus profile's
/// descriptor, so preparation drives a real worker process.
#[cfg(target_os = "macos")]
struct Relabeled {
    worker: context_foundry::neural::supervisor::WorkerProvider,
    descriptor: FunctionDescriptor,
}

#[cfg(target_os = "macos")]
impl EmbeddingProvider for Relabeled {
    fn descriptor(&self) -> &FunctionDescriptor {
        &self.descriptor
    }

    fn embed_documents(
        &mut self,
        batch: &[TokenizedInput],
        control: &Control,
    ) -> Result<Vec<Vec<f32>>, ProviderError> {
        self.worker.embed_documents(batch, control)
    }

    fn embed_query(
        &mut self,
        input: &TokenizedInput,
        deadline: Instant,
    ) -> Result<Vec<f32>, ProviderError> {
        self.worker.embed_query(input, deadline)
    }
}

/// Supervisor fail-closed (spec 009 Resources): once the footprint of the
/// LIVE worker can no longer be measured, the worker is stopped exactly
/// like a ceiling breach and preparation reports `resource_limit`.
#[cfg(target_os = "macos")]
#[test]
fn a_failed_footprint_measurement_of_a_live_worker_stops_it_with_resource_limit() {
    use context_foundry::fault::GlobalAction;
    use context_foundry::neural::fault_names::FOOTPRINT_MEASURE;
    use context_foundry::neural::supervisor::WorkerProvider;
    let mut corpus = Corpus::unprepared(&[]);
    let work = tempfile::tempdir().unwrap();
    let worker_profile = fake_worker_profile(work.path());
    // Each document call would hold the worker 2 s: far longer than one
    // 250 ms memory poll.
    let hooks = vec!["--slow-ms".to_owned(), "2000".to_owned()];
    let descriptor = corpus.profile.descriptor.clone();
    let acquire: Acquire = Box::new(move |_profile, _development, _control| {
        let worker = WorkerProvider::launch(&worker_profile, hooks.clone())?;
        // The worker is up; from now on every measurement fails. The poll
        // runs on the supervisor's own thread, hence process-wide arming
        // (no other test in this binary runs a supervised worker).
        fault::arm_global(
            FOOTPRINT_MEASURE,
            0,
            GlobalAction::Fail("the footprint cannot be read".into()),
        );
        Ok(Box::new(Relabeled {
            worker,
            descriptor: descriptor.clone(),
        }) as Box<dyn EmbeddingProvider>)
    });
    let report = corpus.prepare_through(acquire, 120);
    fault::disarm_all();
    assert!(report.partial, "{report:?}");
    assert_eq!(report.reason_code, Some("resource_limit"), "{report:?}");
    assert_eq!(report.provider_code, Some("resource_limit"), "{report:?}");
    assert_eq!(
        report.embedded_units, 0,
        "the stopped call committed nothing"
    );
}

/// The context's coverage word describes its OWN final read: a source
/// committed after the search read (here at the final-validation seam)
/// moves the revision past a complete generation, so the emitted header
/// carries the new revision with `semantic:partial`, never `ready`.
#[test]
fn a_source_committed_before_the_final_read_makes_the_context_word_partial() {
    use context_foundry::fault::names::CONTEXT_BEFORE_FINAL_VALIDATION;
    let corpus = Corpus::new(&[]);
    let slot = corpus.slot();
    let ready = corpus.context(&slot, DUSK_QUERY, BUDGET);
    assert_eq!(ready.word(), Some("ready"));
    let before = ready.batch.freshness.source_revision;
    // Armed on this thread only; not disarmed (that would also clear the
    // process-wide arming another test may hold).
    fault::arm(
        CONTEXT_BEFORE_FINAL_VALIDATION,
        0,
        Action::Call(Box::new(|ctx| {
            ctx.engine
                .expect("the point carries the engine")
                .replace_source("docs/late.md", "# Late\n\nAdded after the search read.\n")
                .unwrap();
        })),
    );
    let answer = corpus.context(&slot, DUSK_QUERY, BUDGET);
    let after = answer.batch.freshness.source_revision;
    assert!(after > before, "{before} -> {after}");
    assert!(
        answer.parsed.header.contains(&format!("r{after}")),
        "{:?}",
        answer.parsed.header
    );
    assert_eq!(answer.word(), Some("partial"));
    assert!(
        answer.item(DUSK_PATH).is_some(),
        "prepared evidence still serves"
    );
}
