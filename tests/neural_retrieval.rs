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
//!
//! 009 T003 acceptance (SC-003), at the end: progressive preparation inside
//! the MCP owner over the stdio transport (and the shared HTTP owner), with
//! a provider the tests gate, fail and count call by call. The real
//! lifecycle exercise on a permitted declared corpus is the ignored
//! measurement-phase test there; it is written, not run.
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
use std::sync::atomic::AtomicBool;
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
            None,
            self.engine(),
            query,
            Strategy::Search,
            &Self::control(),
            false,
            None,
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
        let batch = mcp::search_primary(
            slot,
            self.engine(),
            query,
            path,
            limit,
            &Self::control(),
            None,
        )
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
        None,
        corpus.engine(),
        query,
        Strategy::Search,
        &control,
        false,
        None,
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
            None,
            corpus.engine(),
            query,
            Strategy::Search,
            &Control::unbounded(),
            false,
            None,
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
            policy: None,
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
    // An anchor-less query: an anchored context places no dense units
    // (context-v2 § Anchored context).
    let answer = corpus.context(&slot, "freshness probe", BUDGET);
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
    // Never beside the other supervised-worker test: the arming below is
    // process-wide.
    let _worker = SUPERVISED_WORKERS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
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

// ---------------------------------------------------------------------------
// 009 T003: progressive preparation inside the MCP owner. The owner serves
// over the stdio transport itself (`mcp::serve_streams`, which `serve_stdio`
// runs on the process's stdin/stdout) on an in-process pipe, so the test
// provider can be gated, failed and counted call by call; the concept
// provider above computes every vector.
// ---------------------------------------------------------------------------

/// How one document call fails.
#[derive(Clone, Debug)]
enum Failure {
    Error(ProviderError),
    /// The first vector comes back one component short.
    Short,
}

/// Document-call controls shared with the owner's provider: a gate the test
/// opens call by call, one failing call, and concurrency and drop probes.
#[derive(Clone, Default)]
struct Hold {
    gated: Arc<AtomicBool>,
    /// Document calls that started (held or not).
    entered: Arc<AtomicU64>,
    released: Arc<AtomicU64>,
    /// Model calls of either kind inside the provider now, and at most.
    in_call: Arc<AtomicU64>,
    max_in_call: Arc<AtomicU64>,
    failure: Arc<Mutex<Option<(u64, Failure)>>>,
    /// The provider was dropped: the worker is gone.
    dropped: Arc<AtomicBool>,
    /// A held document call saw its control cancelled (owner shutdown).
    cancel_seen: Arc<AtomicBool>,
    /// The input count of each document call, in call order.
    batches: Arc<Mutex<Vec<u64>>>,
}

impl Hold {
    /// Every document call waits for its own release.
    fn gated() -> Self {
        let hold = Self::default();
        hold.gated.store(true, Ordering::SeqCst);
        hold
    }
    fn entered(&self) -> u64 {
        self.entered.load(Ordering::SeqCst)
    }
    /// The input count of the `call`-th document call (1-based): the batch
    /// size the driver chose, smaller while foreground requests are recent.
    fn batch(&self, call: usize) -> u64 {
        self.batches.lock().unwrap()[call - 1]
    }
    /// Let the next held document call finish.
    fn release(&self) {
        self.released.fetch_add(1, Ordering::SeqCst);
    }
    /// Hold no further document call.
    fn open(&self) {
        self.gated.store(false, Ordering::SeqCst);
    }
    /// The `call`-th document call (1-based) fails.
    fn fail(&self, call: u64, failure: Failure) {
        *self.failure.lock().unwrap() = Some((call, failure));
    }
    fn max_in_call(&self) -> u64 {
        self.max_in_call.load(Ordering::SeqCst)
    }
    fn dropped(&self) -> bool {
        self.dropped.load(Ordering::SeqCst)
    }
    fn call(&self) -> InCall<'_> {
        let now = self.in_call.fetch_add(1, Ordering::SeqCst) + 1;
        self.max_in_call.fetch_max(now, Ordering::SeqCst);
        InCall(self)
    }
    /// Wait until `count` document calls started.
    async fn wait_entered(&self, count: u64) {
        let deadline = Instant::now() + Duration::from_secs(30);
        while self.entered() < count {
            assert!(
                Instant::now() < deadline,
                "only {} document calls started",
                self.entered()
            );
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    }
}

struct InCall<'a>(&'a Hold);

impl Drop for InCall<'_> {
    fn drop(&mut self) {
        self.0.in_call.fetch_sub(1, Ordering::SeqCst);
    }
}

/// The owner's provider: the concept embedding under a [`Hold`], claiming
/// `claimed` as its document function.
struct HeldProvider {
    inner: ConceptProvider,
    claimed: FunctionDescriptor,
    hold: Hold,
}

impl Drop for HeldProvider {
    fn drop(&mut self) {
        self.hold.dropped.store(true, Ordering::SeqCst);
    }
}

impl EmbeddingProvider for HeldProvider {
    fn descriptor(&self) -> &FunctionDescriptor {
        &self.claimed
    }

    fn embed_documents(
        &mut self,
        batch: &[TokenizedInput],
        control: &Control,
    ) -> Result<Vec<Vec<f32>>, ProviderError> {
        let _call = self.hold.call();
        self.hold.batches.lock().unwrap().push(batch.len() as u64);
        let call = self.hold.entered.fetch_add(1, Ordering::SeqCst) + 1;
        while self.hold.gated.load(Ordering::SeqCst)
            && self.hold.released.load(Ordering::SeqCst) < call
        {
            if control.is_cancelled() {
                self.hold.cancel_seen.store(true, Ordering::SeqCst);
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        let failure = self
            .hold
            .failure
            .lock()
            .unwrap()
            .clone()
            .filter(|(at, _)| *at == call);
        match failure {
            Some((_, Failure::Error(error))) => Err(error),
            Some((_, Failure::Short)) => {
                let mut vectors = self.inner.embed_documents(batch, control)?;
                vectors[0].pop();
                Ok(vectors)
            }
            None => self.inner.embed_documents(batch, control),
        }
    }

    fn embed_query(
        &mut self,
        input: &TokenizedInput,
        deadline: Instant,
    ) -> Result<Vec<f32>, ProviderError> {
        let _call = self.hold.call();
        self.inner.embed_query(input, deadline)
    }
}

/// One MCP owner and its SDK client. `server` is the in-process stdio
/// owner's task; the shared HTTP owner has none.
struct Served {
    client: rmcp::service::RunningService<rmcp::RoleClient, ()>,
    server: Option<tokio::task::JoinHandle<context_foundry::adapter_error::AResult<()>>>,
}

impl Corpus {
    /// The owner's semantic configuration: the concept provider under `hold`.
    fn held(&self, hold: &Hold) -> SemanticServing {
        self.held_as(hold, self.profile.descriptor.clone())
    }

    /// [`Self::held`], the provider claiming `claimed` as its function.
    fn held_as(&self, hold: &Hold, claimed: FunctionDescriptor) -> SemanticServing {
        let (descriptor, probe, hold) = (
            self.profile.descriptor.clone(),
            self.probe.clone(),
            hold.clone(),
        );
        SemanticServing::with_provider(
            self.profile_path.clone(),
            Box::new(move || {
                Ok(Box::new(HeldProvider {
                    inner: ConceptProvider { descriptor, probe },
                    claimed,
                    hold,
                }) as Box<dyn EmbeddingProvider>)
            }),
        )
    }

    /// This corpus's MCP owner over the stdio transport; it opens the store.
    async fn serve_stdio(&mut self, semantic: Option<SemanticServing>) -> Served {
        self.engine = None;
        let (client_io, server_io) = tokio::io::duplex(1 << 20);
        let (input, output) = tokio::io::split(server_io);
        let server = tokio::spawn(mcp::serve_streams(
            ServerOptions {
                store: self.store.clone(),
                root: self.root.clone(),
                references: Vec::new(),
                no_memory: false,
                semantic,
                policy: None,
                budget: BudgetConfig::default(),
            },
            input,
            output,
        ));
        let client = ().serve(client_io).await.unwrap();
        Served {
            client,
            server: Some(server),
        }
    }

    /// Reopen the store once the owner released it.
    async fn reopen(&mut self) {
        let deadline = Instant::now() + Duration::from_secs(30);
        loop {
            match Engine::open_existing(&self.store) {
                Ok(engine) => {
                    self.engine = Some(engine);
                    return;
                }
                Err(error) => {
                    assert_eq!(error.code(), "store_busy", "{error}");
                    assert!(Instant::now() < deadline, "the owner kept the store");
                    tokio::time::sleep(Duration::from_millis(20)).await;
                }
            }
        }
    }

    /// Write a workspace file; the owner learns of it through `index`.
    fn write(&self, path: &str, content: &str) {
        let path = self.root.join(path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, content).unwrap();
    }
}

impl Served {
    /// One tool call: `(is_error, text)`.
    async fn raw(&self, tool: &'static str, arguments: serde_json::Value) -> (bool, String) {
        let mut params = CallToolRequestParams::new(tool);
        if let Some(object) = arguments.as_object().filter(|object| !object.is_empty()) {
            params = params.with_arguments(object.clone());
        }
        let result = self.client.call_tool(params).await.unwrap();
        let rmcp::model::ContentBlock::Text(text) = &result.content[0] else {
            panic!("one text block: {result:?}");
        };
        (result.is_error == Some(true), text.text.to_string())
    }

    /// A successful tool result. The adapter's retryable `busy` (the
    /// driver's brief store steps hold the engine slot) is retried.
    async fn ok(&self, tool: &'static str, arguments: serde_json::Value) -> String {
        let deadline = Instant::now() + Duration::from_secs(30);
        loop {
            let (error, text) = self.raw(tool, arguments.clone()).await;
            if !error {
                return text;
            }
            assert!(
                text.contains(r#""code":"busy""#) && Instant::now() < deadline,
                "{tool}: {text}"
            );
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }

    async fn status(&self) -> serde_json::Value {
        serde_json::from_str(&self.ok("status", serde_json::json!({})).await).unwrap()
    }

    /// The `semantic` object of `status`.
    async fn semantic(&self) -> serde_json::Value {
        self.status().await["semantic"].clone()
    }

    /// Poll `status` until `done` holds of its `semantic` object.
    async fn until(
        &self,
        what: &str,
        done: impl Fn(&serde_json::Value) -> bool,
    ) -> serde_json::Value {
        let deadline = Instant::now() + Duration::from_secs(60);
        loop {
            let semantic = self.semantic().await;
            if done(&semantic) {
                return semantic;
            }
            assert!(Instant::now() < deadline, "never {what}: {semantic}");
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    }

    async fn stopped(&self) -> serde_json::Value {
        self.until("stopped", |semantic| semantic["state"] == "stopped")
            .await
    }

    /// `index {semantic: action}`: `(is_error, reply)`.
    async fn semantic_action(&self, action: &str) -> (bool, serde_json::Value) {
        let (error, text) = self
            .raw("index", serde_json::json!({ "semantic": action }))
            .await;
        (error, serde_json::from_str(&text).unwrap())
    }

    /// Start or resume preparation: the reply says it runs.
    async fn prepare(&self) {
        let (error, reply) = self.semantic_action("prepare").await;
        assert!(!error, "{reply}");
        assert_eq!(
            reply,
            serde_json::json!({"semantic": {"state": "running", "reason": null}})
        );
    }

    async fn context(&self, query: &str) -> V2Response {
        let text = self
            .ok(
                "context",
                serde_json::json!({"query": query, "tokens": BUDGET}),
            )
            .await;
        parse_v2(&text).unwrap()
    }

    async fn search(&self, query: &str) -> V2Response {
        let text = self
            .ok("search", serde_json::json!({"query": query, "limit": 10}))
            .await;
        parse_v2(&text).unwrap()
    }

    /// `index` of the primary root: a source write.
    async fn index(&self) -> serde_json::Value {
        serde_json::from_str(&self.ok("index", serde_json::json!({})).await).unwrap()
    }

    /// EOF: the client closes its end, and the stdio owner exits.
    async fn close(self) {
        self.client.cancel().await.unwrap();
        if let Some(server) = self.server {
            tokio::time::timeout(Duration::from_secs(30), server)
                .await
                .expect("the owner exits after EOF")
                .expect("the owner task")
                .expect("a clean exit");
        }
    }
}

fn starts_with(parsed: &V2Response, prefix: &str) -> bool {
    header_word(parsed).is_some_and(|word| word.starts_with(prefix))
}

/// Acceptance 1: while a slow document call is in flight, search, context,
/// status and a source `index` (a write that commits) all complete. The
/// probe INSIDE the provider call (`in_flight_engine` 0, the slot free, a
/// write committed) is `mcp::tests::inference_holds_no_engine_slot_and_no_transaction`.
/// The write moved the revision behind the walk, so the driver walks again
/// and prepares the new file before it stops complete.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_slow_document_call_leaves_source_status_and_index_operations_serviceable() {
    let mut corpus = Corpus::unprepared(&[]);
    let hold = Hold::gated();
    let semantic = corpus.held(&hold);
    let served = corpus.serve_stdio(Some(semantic)).await;
    served.prepare().await;
    hold.wait_entered(1).await;

    let before = served.status().await;
    assert_eq!(before["semantic"]["state"], "running", "{before}");
    let search = served.search("parse_record").await;
    assert!(!search.items.is_empty());
    let context = served.context("shift lead rota").await;
    assert!(!context.items.is_empty());
    assert!(starts_with(&context, "fallback:"), "{:?}", context.header);
    corpus.write(
        "docs/added.md",
        "# Added\n\nThe kiln was fired twice this week.\n",
    );
    served.index().await;
    let after = served.status().await;
    assert!(
        after["source_revision"].as_u64() > before["source_revision"].as_u64(),
        "the write committed: {before} -> {after}"
    );
    // All of that while the first document call was still held.
    assert_eq!(hold.entered(), 1);
    assert_eq!(corpus.probe.document_calls(), 0);

    hold.open();
    hold.release();
    let done = served.stopped().await;
    assert_eq!(done["reason"], serde_json::Value::Null, "{done}");
    assert_eq!(done["sources"], 7, "{done}");
    assert_eq!(done["unpartitioned_sources"], 0, "{done}");
    assert_eq!(done["missing_units"], 0, "{done}");
    assert!(item_of(&served.search("kiln").await, "docs/added.md").is_some());
    served.close().await;
}

/// Acceptance 2: baseline context while preparing, semantic evidence once
/// coverage arrives (`partial`, then `ready`), a fresh baseline after an
/// indexed edit, then preparation of ONLY the edited input.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn context_is_baseline_while_preparing_then_partial_then_ready_and_an_edit_reembeds_only_itself()
 {
    let mut corpus = Corpus::unprepared(&[]);
    let hold = Hold::gated();
    let semantic = corpus.held(&hold);
    let served = corpus.serve_stdio(Some(semantic)).await;
    served.prepare().await;
    hold.wait_entered(1).await;
    let baseline = served.context(DUSK_QUERY).await;
    assert!(starts_with(&baseline, "fallback:"), "{:?}", baseline.header);
    assert!(baseline.items.iter().all(|item| item.neural.is_none()));

    // Pause: the batch in flight (the dusk document is first in path order)
    // is committed and published, and nothing more.
    let (error, reply) = served.semantic_action("pause").await;
    assert!(!error, "{reply}");
    hold.release();
    let first = hold.batch(1);
    let paused = served
        .until("the first batch searchable", |semantic| {
            semantic["searchable_current_units"] == first
        })
        .await;
    assert_eq!(paused["state"], "paused", "{paused}");
    let partial = served.context(DUSK_QUERY).await;
    assert_eq!(header_word(&partial), Some("partial"));
    assert!(
        item_of(&partial, DUSK_PATH).is_some_and(|item| item.neural.is_some()),
        "semantic evidence from the partial coverage"
    );

    hold.open();
    served.prepare().await;
    let done = served.stopped().await;
    assert_eq!(done["missing_units"], 0, "{done}");
    let ready = served.context(DUSK_QUERY).await;
    assert_eq!(header_word(&ready), Some("ready"));
    let item = item_of(&ready, DUSK_PATH).expect("dense evidence");
    assert_eq!(item.neural.as_ref().unwrap().selection, "whole_unit");

    // An indexed edit: a fresh baseline at once, then only its input.
    let (calls, inputs) = (
        corpus.probe.document_calls(),
        corpus.probe.document_inputs(),
    );
    let edited = format!(
        "{}\nKiln inspection moves to Thursday.\n",
        corpus.source("docs/schedule.md")
    );
    corpus.write("docs/schedule.md", &edited);
    served.index().await;
    assert_eq!(
        header_word(&served.context(DUSK_QUERY).await),
        Some("partial")
    );
    assert!(
        item_of(&served.search("kiln").await, "docs/schedule.md").is_some(),
        "the edit is served at once"
    );
    served.prepare().await;
    served.stopped().await;
    assert_eq!(corpus.probe.document_calls() - calls, 1);
    assert_eq!(
        corpus.probe.document_inputs() - inputs,
        1,
        "only the edited input"
    );
    assert_eq!(
        header_word(&served.context(DUSK_QUERY).await),
        Some("ready")
    );
    served.close().await;
}

/// Acceptance 3: pause admits no new batch; the batch in flight finishes and
/// is committed and published; an explicit prepare resumes from it.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn pause_admits_no_new_batch_and_commits_the_batch_in_flight() {
    let mut corpus = Corpus::unprepared(&[]);
    let hold = Hold::gated();
    let semantic = corpus.held(&hold);
    let served = corpus.serve_stdio(Some(semantic)).await;
    served.prepare().await;
    hold.wait_entered(1).await;
    let (error, reply) = served.semantic_action("pause").await;
    assert!(!error, "{reply}");
    assert_eq!(
        reply,
        serde_json::json!({"semantic": {"state": "paused", "reason": "paused"}})
    );
    hold.open();
    hold.release();
    let first = hold.batch(1);
    let paused = served
        .until("the batch in flight committed", |semantic| {
            semantic["committed_units"] == first && semantic["searchable_current_units"] == first
        })
        .await;
    assert_eq!(paused["state"], "paused", "{paused}");
    assert_eq!(paused["reason"], "paused", "{paused}");
    // The gate is open, yet no new batch starts.
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert_eq!(hold.entered(), 1);
    assert_eq!(corpus.probe.document_calls(), 1);
    let still = served.semantic().await;
    assert_eq!(still["committed_units"], first, "{still}");
    assert!(still["missing_units"].as_u64().unwrap() > 0, "{still}");

    served.prepare().await;
    let done = served.stopped().await;
    assert_eq!(done["missing_units"], 0, "{done}");
    assert_eq!(done["reason"], serde_json::Value::Null, "{done}");
    assert_eq!(
        done["cache"]["entries"].as_u64(),
        Some(corpus.probe.document_inputs()),
        "no input was embedded twice: {done}"
    );
    served.close().await;
}

/// Acceptance 4 (fixed after review M3): EOF in the middle of a batch keeps
/// the commits, discards the uncommitted call and never leaves `running`;
/// the owner keeps its store until the worker is stopped, so no replacement
/// owner can open the store while the worker still runs.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn eof_mid_batch_keeps_commits_discards_the_call_and_stops_the_worker() {
    let mut corpus = Corpus::unprepared(&[]);
    let hold = Hold::gated();
    let semantic = corpus.held(&hold);
    let served = corpus.serve_stdio(Some(semantic)).await;
    served.prepare().await;
    hold.release();
    hold.wait_entered(2).await;
    let first = hold.batch(1);
    assert_eq!(served.semantic().await["committed_units"], first);

    // EOF while the second call is held in the provider.
    let closing = tokio::spawn(served.close());
    // Barrier: the owner's shutdown reached the driver, which cancelled the
    // call it abandons. The worker still runs that call, so the owner keeps
    // its store and is not done.
    let deadline = Instant::now() + Duration::from_secs(30);
    while !hold.cancel_seen.load(Ordering::SeqCst) {
        assert!(
            Instant::now() < deadline,
            "the shutdown never reached the call"
        );
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    match Engine::open_existing(&corpus.store) {
        Err(error) => assert_eq!(error.code(), "store_busy", "{error}"),
        Ok(_) => panic!("a replacement owner opened the store while the worker ran"),
    }
    assert!(!hold.dropped(), "the call still runs");
    assert!(!closing.is_finished(), "the owner waits for its worker");
    // The call ends; its late reply is discarded; the worker goes, then the
    // owner exits.
    hold.open();
    closing.await.unwrap();
    assert!(
        hold.dropped(),
        "the worker was gone before the owner exited"
    );
    assert_eq!(corpus.probe.document_calls(), 2);

    corpus.reopen().await;
    let state = corpus.engine().semantic_state().unwrap().unwrap();
    assert_eq!(state.state, "paused", "never `running`: {state:?}");
    assert_eq!(state.last_error.as_ref().unwrap().code, "cancelled");
    assert_eq!(state.committed_units, first);
    corpus.engine = None;
    assert_eq!(
        testkit::semantic_cache_rows(&corpus.store).len(),
        first as usize,
        "the uncommitted batch was discarded"
    );
}

/// Acceptance 5: the full handoff under concurrent queries: never more than
/// one outstanding model call of either kind. Barrier: the first document
/// call is held while a query arrives; the query is refused `provider_busy`
/// with baseline results and nothing is queued. Queries then run beside the
/// remaining batches; a refused document admission pauses by name
/// (`provider_busy`) and an explicit prepare resumes.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn one_model_call_is_ever_outstanding_across_queries_and_document_batches() {
    // Prepared by the CLI, so queries embed whenever the slot is free; the
    // fillers, indexed by the owner, are the preparation work.
    let mut corpus = Corpus::new(&[]);
    for (path, text) in fillers(24) {
        corpus.write(&path, &text);
    }
    corpus.probe.pause_documents(Duration::from_millis(30));
    let hold = Hold::gated();
    let semantic = corpus.held(&hold);
    let served = corpus.serve_stdio(Some(semantic)).await;
    served.index().await;
    let (queries_before, documents_before) =
        (corpus.probe.query_calls(), corpus.probe.document_calls());
    served.prepare().await;
    hold.wait_entered(1).await;
    let busy = served.context("refund timing").await;
    assert!(
        starts_with(&busy, "fallback:provider_busy"),
        "{:?}",
        busy.header
    );
    assert!(!busy.items.is_empty(), "baseline retrieval still works");
    assert_eq!(
        corpus.probe.query_calls(),
        queries_before,
        "nothing was queued"
    );

    let stop = Arc::new(AtomicBool::new(false));
    let queries: Vec<_> = (0..2)
        .map(|n| {
            let (peer, stop) = (served.client.peer().clone(), Arc::clone(&stop));
            tokio::spawn(async move {
                let mut busy = 0u64;
                while !stop.load(Ordering::SeqCst) {
                    let arguments = serde_json::json!({"query": format!("refund timing {n}"), "tokens": BUDGET});
                    let result = peer
                        .call_tool(
                            CallToolRequestParams::new("context")
                                .with_arguments(arguments.as_object().unwrap().clone()),
                        )
                        .await
                        .unwrap();
                    let rmcp::model::ContentBlock::Text(text) = &result.content[0] else {
                        panic!("one text block");
                    };
                    if text.text.contains("semantic:fallback:provider_busy") {
                        busy += 1;
                    }
                    tokio::time::sleep(Duration::from_millis(10)).await;
                }
                busy
            })
        })
        .collect();
    hold.open();
    hold.release();
    let mut refusals = 0u64;
    let deadline = Instant::now() + Duration::from_secs(60);
    loop {
        let semantic = served.semantic().await;
        if semantic["state"] == "stopped" {
            break;
        }
        if semantic["state"] == "paused" {
            assert_eq!(semantic["reason"], "provider_busy", "{semantic}");
            refusals += 1;
            // May itself be refused while a query holds the slot.
            let _ = served.semantic_action("prepare").await;
        }
        assert!(Instant::now() < deadline, "{semantic}");
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    stop.store(true, Ordering::SeqCst);
    let mut busy_queries = 0;
    for query in queries {
        busy_queries += query.await.unwrap();
    }
    assert_eq!(
        header_word(&served.context("refund timing").await),
        Some("ready")
    );
    assert!(
        corpus.probe.query_calls() > queries_before,
        "queries reached the model"
    );
    assert!(
        corpus.probe.document_calls() > documents_before,
        "document batches ran"
    );
    // Finish without load.
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        let semantic = served.semantic().await;
        if semantic["state"] == "stopped" {
            assert_eq!(semantic["missing_units"], 0, "{semantic}");
            break;
        }
        if semantic["state"] == "paused" {
            let _ = served.semantic_action("prepare").await;
        }
        assert!(Instant::now() < deadline, "{semantic}");
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    assert_eq!(hold.max_in_call(), 1, "two model calls were outstanding");
    record(
        "t003-handoff",
        serde_json::json!({"busy_queries": busy_queries, "busy_admissions": refusals}),
    );
    served.close().await;
}

/// Acceptance 6: another writer — CLI preparation or indexing against the
/// owner's store — is `store_busy` while the owner prepares.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_second_writer_is_store_busy_while_the_owner_prepares() {
    let mut corpus = Corpus::unprepared(&[]);
    let hold = Hold::gated();
    let semantic = corpus.held(&hold);
    let served = corpus.serve_stdio(Some(semantic)).await;
    served.prepare().await;
    hold.wait_entered(1).await;
    let profile = corpus.profile_path.display().to_string();
    let root = corpus.root.display().to_string();
    for arguments in [
        vec![
            "semantic",
            "prepare",
            "--profile",
            profile.as_str(),
            "--budget-seconds",
            "5",
        ],
        vec!["index", root.as_str()],
    ] {
        let output = std::process::Command::new(env!("CARGO_BIN_EXE_foundry"))
            .arg("--store")
            .arg(&corpus.store)
            .args(&arguments)
            .output()
            .unwrap();
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(!output.status.success(), "{arguments:?}");
        assert!(stderr.contains("store_busy"), "{arguments:?}: {stderr}");
    }
    assert_eq!(hold.entered(), 1, "the owner's preparation went on");
    hold.open();
    hold.release();
    assert_eq!(served.stopped().await["missing_units"], 0);
    served.close().await;
}

/// Acceptance 7: a result that arrives after its source was deleted may
/// populate the cache, but the deleted source is never eligible or served.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_late_result_after_deletion_may_be_cached_but_its_source_is_never_eligible_or_served() {
    let mut corpus = Corpus::unprepared(&[]);
    let dusk_key = provider::input_key(
        &corpus.digest(),
        &provider::render_document(&corpus.source(DUSK_PATH)),
    );
    let hold = Hold::gated();
    let semantic = corpus.held(&hold);
    let served = corpus.serve_stdio(Some(semantic)).await;
    served.prepare().await;
    // The first batch holds the dusk input; delete its source meanwhile.
    hold.wait_entered(1).await;
    std::fs::remove_file(corpus.root.join(DUSK_PATH)).unwrap();
    served.index().await;
    hold.open();
    hold.release();
    let done = served.stopped().await;
    assert_eq!(done["sources"], 5, "{done}");
    assert_eq!(done["missing_units"], 0, "{done}");
    assert!(
        done["cache"]["orphan_entries"].as_u64().unwrap() >= 1,
        "the late result is retained, not eligible: {done}"
    );
    let context = served.context(DUSK_QUERY).await;
    assert_eq!(header_word(&context), Some("ready"));
    assert!(item_of(&context, DUSK_PATH).is_none());
    assert!(item_of(&served.search(DUSK_QUERY).await, DUSK_PATH).is_none());
    served.close().await;
    corpus.reopen().await;
    corpus.engine = None;
    assert!(
        testkit::semantic_cache_rows(&corpus.store)
            .iter()
            .any(|(key, _)| *key == dusk_key),
        "the cache holds the late result"
    );
}

/// Acceptance 8: a restarted owner never resumes — a dead owner's `running`
/// reads back stopped/interrupted and nothing runs until told — and an
/// explicit prepare over unchanged sources makes ZERO document calls.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_restarted_owner_never_resumes_and_an_unchanged_prepare_makes_zero_document_calls() {
    let mut corpus = Corpus::unprepared(&[]);
    let hold = Hold::default();
    let semantic = corpus.held(&hold);
    let served = corpus.serve_stdio(Some(semantic)).await;
    served.prepare().await;
    let first = served.stopped().await;
    let calls = corpus.probe.document_calls();
    assert!(calls > 0);
    served.close().await;

    // A dead owner's leftover row says `running`.
    corpus.reopen().await;
    let mut state = corpus.engine().semantic_state().unwrap().unwrap();
    state.state = "running".into();
    corpus.engine().semantic_set_state(&state).unwrap();

    let semantic = corpus.held(&hold);
    let served = corpus.serve_stdio(Some(semantic)).await;
    let restarted = served.semantic().await;
    assert_eq!(restarted["state"], "stopped", "{restarted}");
    assert_eq!(restarted["reason"], "interrupted", "{restarted}");
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert_eq!(corpus.probe.document_calls(), calls, "startup resumed work");
    assert_eq!(served.semantic().await["state"], "stopped");

    served.prepare().await;
    let again = served.stopped().await;
    assert_eq!(again["reason"], serde_json::Value::Null, "{again}");
    assert_eq!(
        corpus.probe.document_calls(),
        calls,
        "an unchanged prepare makes zero document calls"
    );
    assert_eq!(
        again["searchable_current_units"],
        first["searchable_current_units"]
    );
    served.close().await;
}

/// Acceptance 9: server inference outlives the client's timeout (a stalled
/// query); the next query gets baseline with `provider_busy`, an explicit
/// prepare is refused `provider_busy` and queues nothing; once the old call
/// really ends, prepare proceeds.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn server_inference_outliving_a_client_timeout_keeps_the_slot_for_queries_and_prepare() {
    let mut corpus = Corpus::new(&[]);
    let documents = corpus.probe.document_calls();
    corpus.probe.set(QueryMode::Sleep(Duration::from_secs(3)));
    let hold = Hold::default();
    let semantic = corpus.held(&hold);
    let served = corpus.serve_stdio(Some(semantic)).await;
    let timed_out = served.context(DUSK_QUERY).await;
    assert!(
        starts_with(&timed_out, "fallback:provider_timeout"),
        "{:?}",
        timed_out.header
    );
    let busy = served.context("shift lead rota").await;
    assert!(
        starts_with(&busy, "fallback:provider_busy"),
        "{:?}",
        busy.header
    );
    assert!(!busy.items.is_empty(), "baseline retrieval still works");
    let (error, refused) = served.semantic_action("prepare").await;
    assert!(error, "{refused}");
    assert_eq!(refused["code"], "provider_busy", "{refused}");
    assert_eq!(refused["retryable"], true, "{refused}");
    assert_eq!(
        served.semantic().await["state"],
        "stopped",
        "nothing started"
    );
    assert_eq!(corpus.probe.query_calls(), 1, "no second query was queued");
    assert_eq!(
        corpus.probe.document_calls(),
        documents,
        "no job was queued"
    );

    corpus.probe.set(QueryMode::Answer);
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let (error, reply) = served.semantic_action("prepare").await;
        if !error {
            assert_eq!(reply["semantic"]["state"], "running", "{reply}");
            break;
        }
        assert_eq!(reply["code"], "provider_busy", "{reply}");
        assert!(Instant::now() < deadline, "the old call never ended");
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    assert_eq!(served.stopped().await["reason"], serde_json::Value::Null);
    assert_eq!(
        corpus.probe.document_calls(),
        documents,
        "nothing was missing"
    );
    assert_eq!(
        header_word(&served.context(DUSK_QUERY).await),
        Some("ready")
    );
    served.close().await;
}

/// Acceptance 10: another store's preparation leaves this one's rows and
/// cache unchanged, and a path mention in a query starts no work.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn another_stores_preparation_leaves_this_store_unchanged_and_a_path_mention_starts_no_work()
{
    let mut a = Corpus::new(&[]);
    let a_state = serde_json::to_value(a.engine().semantic_state().unwrap()).unwrap();
    a.engine = None;
    let a_cache = testkit::semantic_cache_rows(&a.store);
    let a_calls = a.probe.document_calls();
    let mut b = Corpus::unprepared(&[]);
    let (hold_a, hold_b) = (Hold::default(), Hold::default());
    let (semantic_a, semantic_b) = (a.held(&hold_a), b.held(&hold_b));
    let served_a = a.serve_stdio(Some(semantic_a)).await;
    let served_b = b.serve_stdio(Some(semantic_b)).await;
    served_b.prepare().await;
    for query in [
        format!("{DUSK_PATH} twilight"),
        format!("{} dusk", b.root.join(DUSK_PATH).display()),
    ] {
        assert!(!served_a.context(&query).await.items.is_empty());
    }
    assert_eq!(served_b.stopped().await["missing_units"], 0);
    let status_a = served_a.semantic().await;
    assert_eq!(status_a["state"], "stopped", "{status_a}");
    assert_eq!(a.probe.document_calls(), a_calls, "no work in A");
    served_a.close().await;
    served_b.close().await;
    a.reopen().await;
    assert_eq!(
        serde_json::to_value(a.engine().semantic_state().unwrap()).unwrap(),
        a_state
    );
    a.engine = None;
    assert_eq!(testkit::semantic_cache_rows(&a.store), a_cache);
    b.reopen().await;
    b.engine = None;
    assert!(!testkit::semantic_cache_rows(&b.store).is_empty());
}

/// Acceptance 11: model failures — worker death, a malformed reply, a
/// timeout, a worker computing another function — stop preparation by
/// name, publish nothing from the failed batch, and never wedge foreground
/// operations; an explicit prepare resumes.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn model_failures_stop_preparation_by_name_and_never_wedge_foreground_operations() {
    // Worker death on the second call: the first batch stays committed and
    // searchable; search, context, status and index keep working.
    let mut corpus = Corpus::unprepared(&[]);
    let hold = Hold::default();
    hold.fail(
        2,
        Failure::Error(ProviderError::WorkerExited("the worker was killed".into())),
    );
    let semantic = corpus.held(&hold);
    let served = corpus.serve_stdio(Some(semantic)).await;
    served.prepare().await;
    let died = served
        .until("stopped by the dead worker", |semantic| {
            semantic["state"] == "paused"
        })
        .await;
    assert_eq!(died["reason"], "provider_exited", "{died}");
    assert_eq!(died["provider"]["state"], "failed", "{died}");
    assert_eq!(died["provider"]["code"], "provider_exited", "{died}");
    assert!(died["provider"]["observed_at_unix"].as_u64().is_some());
    let first = hold.batch(1);
    assert_eq!(died["committed_units"], first, "{died}");
    assert_eq!(died["searchable_current_units"], first, "{died}");
    assert!(!served.search("parse_record").await.items.is_empty());
    assert_eq!(
        header_word(&served.context(DUSK_QUERY).await),
        Some("partial")
    );
    served.index().await;
    served.prepare().await;
    assert_eq!(served.stopped().await["missing_units"], 0);
    served.close().await;

    // A malformed reply: that batch commits and publishes nothing.
    let mut corpus = Corpus::unprepared(&[]);
    let hold = Hold::default();
    hold.fail(1, Failure::Short);
    let semantic = corpus.held(&hold);
    let served = corpus.serve_stdio(Some(semantic)).await;
    served.prepare().await;
    let malformed = served
        .until("stopped by the malformed reply", |semantic| {
            semantic["state"] == "paused"
        })
        .await;
    assert_eq!(malformed["reason"], "provider_malformed", "{malformed}");
    assert_eq!(malformed["committed_units"], 0, "{malformed}");
    assert_eq!(malformed["index"]["available"], false, "{malformed}");
    assert!(starts_with(&served.context(DUSK_QUERY).await, "fallback:"));
    served.close().await;

    // A provider timeout is a named, resumable stop.
    let mut corpus = Corpus::unprepared(&[]);
    let hold = Hold::default();
    hold.fail(1, Failure::Error(ProviderError::Timeout));
    let semantic = corpus.held(&hold);
    let served = corpus.serve_stdio(Some(semantic)).await;
    served.prepare().await;
    let timed_out = served
        .until("stopped by the timeout", |semantic| {
            semantic["state"] == "paused"
        })
        .await;
    assert_eq!(timed_out["reason"], "provider_timeout", "{timed_out}");
    served.prepare().await;
    assert_eq!(served.stopped().await["missing_units"], 0);
    served.close().await;

    // A worker computing another document function makes no call at all.
    let mut corpus = Corpus::unprepared(&[]);
    let mut other = corpus.profile.descriptor.clone();
    other.quantization = "affine bits=8".into();
    let hold = Hold::default();
    let semantic = corpus.held_as(&hold, other);
    let served = corpus.serve_stdio(Some(semantic)).await;
    served.prepare().await;
    let wrong = served
        .until("stopped by the wrong function", |semantic| {
            semantic["state"] == "paused"
        })
        .await;
    assert_eq!(wrong["reason"], "provider_malformed", "{wrong}");
    assert_eq!(hold.entered(), 0);
    assert!(!served.search("parse_record").await.items.is_empty());
    served.close().await;
}

/// `index {semantic}` validates its arguments before anything starts, and
/// an owner without a runtime refuses it by name with the fallback reason.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn index_semantic_validates_its_arguments_and_names_a_missing_runtime() {
    let mut corpus = Corpus::new(&[]);
    let calls = corpus.probe.document_calls();
    let hold = Hold::default();
    let semantic = corpus.held(&hold);
    let served = corpus.serve_stdio(Some(semantic)).await;
    for arguments in [
        serde_json::json!({"semantic": "prepare", "root": "primary"}),
        serde_json::json!({"semantic": "pause", "scip": {"index_file": "a", "snapshot_file": "b"}}),
        serde_json::json!({"semantic": "resume"}),
        serde_json::json!({"semantic": null}),
        serde_json::json!({"semantic": "prepare", "timeout_ms": 0}),
    ] {
        let (error, text) = served.raw("index", arguments.clone()).await;
        assert!(error, "{arguments}: {text}");
        let reply: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert_eq!(reply["code"], "invalid_argument", "{arguments}: {text}");
    }
    // A pause with nothing running answers from the committed row.
    let (error, reply) = served.semantic_action("pause").await;
    assert!(!error, "{reply}");
    assert_eq!(
        reply,
        serde_json::json!({"semantic": {"state": "stopped", "reason": null}})
    );
    let semantic = served.semantic().await;
    assert_eq!(semantic["runtime"], "ready", "{semantic}");
    assert_eq!(semantic["missing_units"], 0, "{semantic}");
    assert_eq!(corpus.probe.document_calls(), calls);
    served.close().await;

    // No semantic profile: refused by name; the status is unchanged.
    let served = corpus.serve_stdio(None).await;
    let (error, reply) = served.semantic_action("prepare").await;
    assert!(error, "{reply}");
    assert_eq!(reply["code"], "semantic_unavailable", "{reply}");
    assert!(served.status().await.get("semantic").is_none());
    served.close().await;

    // A refused start: refused by name with the fallback reason.
    let refused = SemanticServing::with_provider(
        corpus.profile_path.clone(),
        Box::new(|| {
            Err(ProviderError::IsolationUnavailable(
                "the fixture refuses".into(),
            ))
        }),
    );
    let served = corpus.serve_stdio(Some(refused)).await;
    let (error, reply) = served.semantic_action("prepare").await;
    assert!(error, "{reply}");
    assert_eq!(reply["code"], "semantic_unavailable", "{reply}");
    assert!(
        reply["message"]
            .as_str()
            .unwrap()
            .contains("isolation_unavailable"),
        "{reply}"
    );
    let semantic = served.semantic().await;
    assert!(
        semantic["runtime"]
            .as_str()
            .unwrap()
            .starts_with("fallback:isolation_unavailable"),
        "{semantic}"
    );
    assert_eq!(semantic["state"], "stopped", "{semantic}");
    served.close().await;
}

/// The shared HTTP owner prepares progressively through the same driver.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_shared_http_owner_prepares_progressively_too() {
    let mut corpus = Corpus::unprepared(&[]);
    let hold = Hold::default();
    let semantic = corpus.held(&hold);
    let (client, shutdown) = serve(&mut corpus, semantic).await;
    let served = Served {
        client,
        server: None,
    };
    served.prepare().await;
    let done = served.stopped().await;
    assert_eq!(done["missing_units"], 0, "{done}");
    assert_eq!(
        header_word(&served.context(DUSK_QUERY).await),
        Some("ready")
    );
    served.close().await;
    shutdown.cancel();
}

/// 009 T003's real lifecycle exercise — MEASUREMENT PHASE: written, not run
/// (owner directive 2026-10-05). The production `foundry mcp` over real
/// stdio, with the development-isolated worker and the actual model, on a
/// permitted declared corpus: cold preparation, a useful partial query,
/// steady queries, edit catch-up and restart, each with committed-unit
/// counts (the model's document inputs) and timestamps.
///
/// Inputs, all required; nothing is downloaded or installed:
/// - `CF_T003_PROFILE`: the verified development profile;
/// - `CF_T003_CORPUS`: the permitted declared corpus root (copied; the
///   original is never modified);
/// - `CF_T003_QUERY`: a vocabulary-gap question about that corpus;
/// - `CF_T003_EDIT`: a corpus-relative file the exercise edits in its copy;
/// - `CF_T003_RECORD_DIR`: where the timeline is written.
///
/// Run budget and bounds, declared before any run:
/// - the whole exercise: at most 30 minutes, cold preparation included;
/// - every query: answered within the 5 s read deadline, never an error;
/// - the partial query, during preparation: `partial` (or a named
///   `fallback:` word) with baseline results;
/// - steady queries after completion: `ready`;
/// - edit catch-up: at least one and at most the edited file's units are
///   committed, then `ready`;
/// - restart: the owner does not resume, and an unchanged `prepare`
///   commits zero units.
///
/// Command: `CF_T003_PROFILE=… CF_T003_CORPUS=… CF_T003_QUERY=… CF_T003_EDIT=…
/// CF_T003_RECORD_DIR=… cargo test --test neural_retrieval
/// real_lifecycle_exercise -- --ignored --nocapture`.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "measurement phase: the actual model on a permitted declared corpus (owner directive 2026-10-05)"]
async fn real_lifecycle_exercise_on_a_permitted_declared_corpus() {
    const RUN_BUDGET: Duration = Duration::from_secs(30 * 60);

    let input = |name: &str| std::env::var(name).unwrap_or_else(|_| panic!("{name} is required"));
    let profile = PathBuf::from(input("CF_T003_PROFILE"));
    let query = input("CF_T003_QUERY");
    let edit = input("CF_T003_EDIT");
    let record_dir = PathBuf::from(input("CF_T003_RECORD_DIR"));
    let work = tempfile::tempdir().unwrap();
    let root = work.path().join("corpus");
    copy_tree(Path::new(&input("CF_T003_CORPUS")), &root);
    let store = work.path().join("store");
    let bin = env!("CARGO_BIN_EXE_foundry");
    let indexed = std::process::Command::new(bin)
        .arg("--store")
        .arg(&store)
        .arg("index")
        .arg(&root)
        .output()
        .unwrap();
    assert!(indexed.status.success(), "{indexed:?}");

    let started = Instant::now();
    let mut timeline: Vec<serde_json::Value> = Vec::new();
    let mut mark = |event: &str, detail: serde_json::Value| {
        timeline.push(serde_json::json!({
            "t_ms": started.elapsed().as_millis() as u64,
            "event": event,
            "detail": detail,
        }));
    };
    /// The production owner over real stdio; a previous owner may still be
    /// releasing the store.
    async fn owner(bin: &str, store: &Path, root: &Path, profile: &Path) -> Served {
        let deadline = Instant::now() + Duration::from_secs(60);
        while let Err(error) = Engine::open_existing(store) {
            assert_eq!(error.code(), "store_busy", "{error}");
            assert!(Instant::now() < deadline);
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        let mut command = tokio::process::Command::new(bin);
        command
            .arg("--store")
            .arg(store)
            .arg("mcp")
            .arg("--root")
            .arg(root)
            .arg("--semantic-profile")
            .arg(profile)
            .arg("--development-isolation")
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::inherit());
        Served {
            client:
                ().serve(rmcp::transport::TokioChildProcess::new(command).unwrap())
                    .await
                    .unwrap(),
            server: None,
        }
    }
    // One timed query: (elapsed, header word, item count).
    async fn timed(served: &Served, query: &str) -> (Duration, Option<String>, usize) {
        let started = Instant::now();
        let parsed = served.context(query).await;
        (
            started.elapsed(),
            header_word(&parsed).map(str::to_owned),
            parsed.items.len(),
        )
    }
    let committed = |semantic: &serde_json::Value| semantic["committed_units"].as_u64().unwrap();

    // Cold preparation, with one useful partial query on the way.
    let served = owner(bin, &store, &root, &profile).await;
    served.prepare().await;
    mark("prepare", serde_json::json!({}));
    let mut partial_asked = false;
    loop {
        let semantic = served.semantic().await;
        mark("status", semantic.clone());
        if !partial_asked && semantic["searchable_current_units"].as_u64().unwrap_or(0) > 0 {
            let (elapsed, word, items) = timed(&served, &query).await;
            assert!(elapsed <= mcp::READ_DEADLINE, "{elapsed:?}");
            assert!(items > 0);
            assert!(
                word.as_deref()
                    .is_some_and(|word| word == "partial" || word.starts_with("fallback:")),
                "{word:?}"
            );
            mark(
                "partial_query",
                serde_json::json!({"ms": elapsed.as_millis() as u64, "word": word}),
            );
            partial_asked = true;
        }
        if semantic["state"] != "running" {
            assert_eq!(semantic["state"], "stopped", "{semantic}");
            break;
        }
        assert!(
            started.elapsed() < RUN_BUDGET,
            "cold preparation overran the budget"
        );
        tokio::time::sleep(Duration::from_secs(2)).await;
    }

    // Steady queries.
    for _ in 0..5 {
        let (elapsed, word, _) = timed(&served, &query).await;
        assert!(elapsed <= mcp::READ_DEADLINE, "{elapsed:?}");
        assert_eq!(word.as_deref(), Some("ready"));
        mark(
            "steady_query",
            serde_json::json!({"ms": elapsed.as_millis() as u64}),
        );
    }

    // Edit catch-up.
    let before = committed(&served.semantic().await);
    let edited_path = root.join(&edit);
    let mut edited = std::fs::read_to_string(&edited_path).unwrap();
    edited.push_str("\nLifecycle exercise edit.\n");
    std::fs::write(&edited_path, edited).unwrap();
    served.index().await;
    let (elapsed, word, _) = timed(&served, &query).await;
    mark(
        "fresh_baseline_query",
        serde_json::json!({"ms": elapsed.as_millis() as u64, "word": word}),
    );
    served.prepare().await;
    let caught_up = served
        .until("caught up", |semantic| semantic["state"] != "running")
        .await;
    assert_eq!(caught_up["state"], "stopped", "{caught_up}");
    let embedded = committed(&caught_up) - before;
    assert!(embedded >= 1, "the edit was embedded");
    mark(
        "edit_catch_up",
        serde_json::json!({"embedded_units": embedded}),
    );
    assert_eq!(timed(&served, &query).await.1.as_deref(), Some("ready"));
    served.close().await;

    // Restart: nothing resumes; an unchanged prepare commits nothing.
    let served = owner(bin, &store, &root, &profile).await;
    let restarted = served.semantic().await;
    assert_eq!(restarted["state"], "stopped", "{restarted}");
    let before = committed(&restarted);
    served.prepare().await;
    let again = served
        .until("restart prepared", |semantic| {
            semantic["state"] != "running"
        })
        .await;
    assert_eq!(
        committed(&again),
        before,
        "an unchanged restart embedded something"
    );
    mark("restart", serde_json::json!({"embedded_units": 0}));
    served.close().await;
    assert!(
        started.elapsed() < RUN_BUDGET,
        "the exercise overran its budget"
    );

    std::fs::create_dir_all(&record_dir).unwrap();
    std::fs::write(
        record_dir.join("t003-lifecycle.json"),
        serde_json::to_string_pretty(&timeline).unwrap(),
    )
    .unwrap();
}

// ---------------------------------------------------------------------------
// 009 T003 review round 1: the admission interleaving, at its barriers. The
// supervised-worker case (M1) is in tests/embed_worker.rs: a worker launch
// forks, and a fork can briefly hold another test's store lock here.
// ---------------------------------------------------------------------------

/// Review M4: a foreground query that registers while a document admission
/// is under way wins the model slot. Barriers: the admission is held at its
/// start (`semantic.document_admission`) until the query registered, and the
/// query is held right after registering, before it claims the slot
/// (`semantic.query_registered`). The admission is refused, nothing is
/// queued, and the query embeds instead of falling back `provider_busy`.
#[test]
fn a_query_registered_during_a_document_admission_wins_the_model_slot() {
    use context_foundry::neural::fault_names::{DOCUMENT_ADMISSION, QUERY_REGISTERED};
    type Query = std::thread::JoinHandle<Result<Vec<f32>, ProviderError>>;
    let corpus = Corpus::new(&[]);
    let runtime = Arc::new(corpus.runtime());
    let documents = corpus.probe.document_calls();
    let (registered_tx, registered_rx) = std::sync::mpsc::channel::<()>();
    let (go_tx, go_rx) = std::sync::mpsc::channel::<()>();
    let query: Arc<Mutex<Option<Query>>> = Arc::default();
    let start = Mutex::new(Some((registered_tx, go_rx)));
    let (for_query, handle) = (Arc::clone(&runtime), Arc::clone(&query));
    fault::arm(
        DOCUMENT_ADMISSION,
        0,
        Action::Call(Box::new(move |_| {
            let Some((registered, go)) = start.lock().unwrap().take() else {
                return;
            };
            let runtime = Arc::clone(&for_query);
            *handle.lock().unwrap() = Some(std::thread::spawn(move || {
                fault::arm(
                    QUERY_REGISTERED,
                    0,
                    Action::Call(Box::new(move |_| {
                        let _ = registered.send(());
                        let _ = go.recv();
                    })),
                );
                runtime.embed(DUSK_QUERY, Instant::now() + Duration::from_secs(30))
            }));
            registered_rx.recv().unwrap();
        })),
    );
    let input = TokenizedInput {
        ids: "passage: dusk".bytes().map(u32::from).collect(),
    };
    let refused = runtime.dispatch_documents(vec![input.clone()], Control::unbounded());
    assert!(
        matches!(refused, Err(ProviderError::Busy)),
        "the registered query must win"
    );
    go_tx.send(()).unwrap();
    let embedded = query.lock().unwrap().take().unwrap().join().unwrap();
    assert!(embedded.is_ok(), "the query embeds: {embedded:?}");
    assert_eq!(corpus.probe.document_calls(), documents, "nothing queued");
    // Once the query ended, a document batch is admitted again.
    let call = runtime
        .dispatch_documents(vec![input], Control::unbounded())
        .expect("admitted once the slot is free");
    assert!(
        call.wait(Duration::from_secs(10))
            .expect("the call ends")
            .is_ok()
    );
    assert_eq!(corpus.probe.document_calls(), documents + 1);
}

// ---------------------------------------------------------------------------
// 009 T002 accepted gaps, closed 2026-10-06 (docs/validation.md, 009 T002
// "Known gaps"): semantic evidence crowded by graph and compiler evidence,
// the localized lexical-span preview, multibyte/CRLF/fence-like forms, the
// semantic MCP byte cap and allowance accounting, the multi-root matrix,
// the no-profile byte-identity matrix with positive hits, and a worker
// dying mid-query.
// ---------------------------------------------------------------------------

impl Corpus {
    /// A context request through the production decision path under an
    /// explicit strategy.
    fn context_as(
        &self,
        slot: &SemanticSlot,
        query: &str,
        strategy: Strategy,
        tokens: usize,
    ) -> Answer {
        let combined = mcp::context_primary(
            slot,
            None,
            self.engine(),
            query,
            strategy,
            &Self::control(),
            false,
            None,
        )
        .expect("context candidates");
        Answer::of(combined.batch, tokens)
    }

    /// Every indexed workspace file as `(path, sha256)`, sorted by path.
    fn indexed_inputs(&self) -> Vec<(String, String)> {
        fn walk(root: &Path, dir: &Path, out: &mut Vec<String>) {
            for entry in std::fs::read_dir(dir).unwrap() {
                let path = entry.unwrap().path();
                if path.is_dir() {
                    walk(root, &path, out);
                } else {
                    let relative = path.strip_prefix(root).unwrap();
                    out.push(relative.to_str().unwrap().to_owned());
                }
            }
        }
        let mut paths = Vec::new();
        walk(&self.root, &self.root, &mut paths);
        paths.sort();
        paths
            .into_iter()
            .filter_map(|path| {
                let meta = self.engine().source(&path).unwrap()?;
                Some((path, meta.hash))
            })
            .collect()
    }
}

fn owned(sources: &[(&str, &str)]) -> Vec<(String, String)> {
    sources
        .iter()
        .map(|(path, text)| ((*path).to_owned(), (*text).to_owned()))
        .collect()
}

/// Two seeds for 005's graph context. `hostseed` (a lexical hit) calls
/// `target_fn`, which `caller_a` calls too; the evening unit (sundown
/// concepts, no query word) calls `lantern_mark`, which `lantern_user` calls
/// too. Each referring file holds a second function, so the delivery unit
/// enclosing the reference is narrower than the file's embedding unit and an
/// expansion delivers it as its own compiler item.
const GRAPH_SOURCES: [(&str, &str); 6] = [
    ("src/host.rs", "pub fn hostseed() { crate::target_fn(); }\n"),
    ("src/target.rs", "pub fn target_fn() {}\n"),
    (
        "src/caller.rs",
        "pub fn caller_a() { crate::target_fn(); }\n\npub fn caller_spare() -> u8 {\n    1\n}\n",
    ),
    (
        "src/evening.rs",
        "/// Sundown glow at dusk: amber horizon, nightfall.\npub fn evening_bell() {\n    crate::lantern_mark();\n}\n",
    ),
    ("src/lantern.rs", "pub fn lantern_mark() {}\n"),
    (
        "src/lantern_user.rs",
        "pub fn lantern_user() { crate::lantern_mark(); }\n\npub fn lantern_spare() -> u8 {\n    2\n}\n",
    ),
];
const TARGET_SYMBOL: &str = "rust-analyzer cargo gaps 0.1.0 target_fn().";
const LANTERN_SYMBOL: &str = "rust-analyzer cargo gaps 0.1.0 lantern_mark().";

fn graph_source(path: &str) -> &'static str {
    GRAPH_SOURCES
        .iter()
        .find(|(candidate, _)| *candidate == path)
        .map(|(_, text)| *text)
        .expect("a graph fixture source")
}

/// One UTF-8 SCIP occurrence of `symbol` on the first line of `path` that
/// holds `needle`.
fn scip_occurrence(
    path: &str,
    needle: &str,
    symbol: &str,
    definition: bool,
) -> scip::types::Occurrence {
    let (line, column) = graph_source(path)
        .lines()
        .enumerate()
        .find_map(|(line, text)| text.find(needle).map(|column| (line, column)))
        .expect("the needle is in the source");
    let mut occurrence = scip::types::Occurrence::new();
    occurrence.range = vec![line as i32, column as i32, (column + needle.len()) as i32];
    occurrence.symbol = symbol.to_owned();
    occurrence.symbol_roles = i32::from(definition);
    occurrence
}

fn scip_document(path: &str, occurrences: Vec<scip::types::Occurrence>) -> scip::types::Document {
    let mut document = scip::types::Document::new();
    document.relative_path = path.to_owned();
    document.language = "rust".to_owned();
    document.position_encoding = protobuf::EnumOrUnknown::new(
        scip::types::PositionEncoding::UTF8CodeUnitOffsetFromLineStart,
    );
    document.occurrences = occurrences;
    document
}

impl Corpus {
    /// 005 compiler facts (a SCIP artifact bound to the store as indexed
    /// now) and one manual `calls` edge from each seed.
    fn import_graphs(&self) {
        use context_foundry::graph::{Edge, Endpoint, GraphBundle};
        let mut index = scip::types::Index::new();
        index.documents = vec![
            scip_document(
                "src/host.rs",
                vec![scip_occurrence(
                    "src/host.rs",
                    "target_fn",
                    TARGET_SYMBOL,
                    false,
                )],
            ),
            scip_document(
                "src/target.rs",
                vec![scip_occurrence(
                    "src/target.rs",
                    "target_fn",
                    TARGET_SYMBOL,
                    true,
                )],
            ),
            scip_document(
                "src/caller.rs",
                vec![scip_occurrence(
                    "src/caller.rs",
                    "target_fn",
                    TARGET_SYMBOL,
                    false,
                )],
            ),
            scip_document(
                "src/evening.rs",
                vec![scip_occurrence(
                    "src/evening.rs",
                    "lantern_mark",
                    LANTERN_SYMBOL,
                    false,
                )],
            ),
            scip_document(
                "src/lantern.rs",
                vec![scip_occurrence(
                    "src/lantern.rs",
                    "lantern_mark",
                    LANTERN_SYMBOL,
                    true,
                )],
            ),
            scip_document(
                "src/lantern_user.rs",
                vec![scip_occurrence(
                    "src/lantern_user.rs",
                    "lantern_mark",
                    LANTERN_SYMBOL,
                    false,
                )],
            ),
        ];
        let artifact = protobuf::Message::write_to_bytes(&index).unwrap();
        let engine = self.engine();
        let inputs: Vec<serde_json::Value> = self
            .indexed_inputs()
            .into_iter()
            .map(|(path, sha256)| serde_json::json!({"path": path, "sha256": sha256}))
            .collect();
        let manifest = serde_json::json!({
            "v": 1,
            "workspace_id": engine.workspace_id().unwrap(),
            "source_revision": engine.source_revision().unwrap(),
            "producer": {
                "name": "rust-analyzer",
                "release_tag": "2026-08-31",
                "commit": "f8996691e991a4dc3c6f135e0fc04fc5561e4e9a",
                "version_output": "test-producer 1.0",
                "binary_sha256": context_foundry::digest(b"test-producer-binary"),
            },
            "invocation": "test-producer scip <snapshot> --output index.scip",
            "config": "gaps",
            "artifact_sha256": context_foundry::digest(&artifact),
            "inputs": inputs,
        });
        let index_path = self.dir.path().join("gaps-index.scip");
        let manifest_path = self.dir.path().join("gaps-manifest.json");
        std::fs::write(&index_path, &artifact).unwrap();
        std::fs::write(&manifest_path, serde_json::to_vec(&manifest).unwrap()).unwrap();
        engine
            .import_scip(&index_path, &manifest_path, &Control::unbounded())
            .expect("the compiler graph imports");
        let endpoint = |path: &str, line: usize, symbol: &str| Endpoint {
            path: path.to_owned(),
            line,
            symbol: symbol.to_owned(),
            hash: context_foundry::digest(graph_source(path).as_bytes()),
        };
        let calls = |from: Endpoint, to: Endpoint| Edge {
            from,
            to,
            kind: "calls".into(),
            evidence: "manual".into(),
        };
        engine
            .import_graph(&GraphBundle {
                provider: "gaps".into(),
                revision: "r1".into(),
                edges: vec![
                    calls(
                        endpoint("src/host.rs", 1, "hostseed"),
                        endpoint("src/target.rs", 1, "target_fn"),
                    ),
                    calls(
                        endpoint("src/evening.rs", 2, "evening_bell"),
                        endpoint("src/lantern.rs", 1, "lantern_mark"),
                    ),
                ],
            })
            .expect("the manual edges import");
    }
}

/// The paths of a batch's 005 compiler units.
fn compiler_paths(batch: &CandidateBatch) -> Vec<String> {
    batch
        .items
        .iter()
        .filter(|item| item.tier == context_foundry::store::TIER_COMPILER)
        .filter_map(|item| item.handle.as_ref().map(|handle| handle.path.clone()))
        .collect()
}

/// The delivered `edge` lines, without their `edge ` prefix.
fn edges_of(parsed: &V2Response) -> Vec<String> {
    parsed
        .items
        .iter()
        .filter(|item| item.kind == testkit::V2Kind::Edge)
        .map(|item| item.body.clone())
        .collect()
}

/// Gap 1: semantic evidence crowded by GRAPH and COMPILER evidence
/// (`strategy: graph` with a 005 compiler graph and manual edges present)
/// is still delivered whole inside the normal budget, and a dense-only hit
/// ranked where a seed is taken seeds neither expansion. The control shows
/// the very same unit seeding both once it is a lexical hit.
#[test]
fn dense_only_hits_seed_no_graph_or_compiler_expansion_and_survive_that_crowding() {
    use context_foundry::store::{RankedItem, TIER_COMPILER, TIER_OUTLINE};
    let corpus = Corpus::new(&owned(&GRAPH_SOURCES));
    corpus.import_graphs();
    let slot = corpus.slot();

    // Control: `evening bell` names the evening unit lexically. Queries here
    // are anchor-less: an anchored context has no edge lines and places no
    // dense units (context-v2 § Anchored context).
    let seeded = corpus.context_as(&slot, "evening bell", Strategy::Graph, BUDGET);
    assert_eq!(seeded.batch.counters.graph, Some("ok"));
    assert!(
        compiler_paths(&seeded.batch)
            .iter()
            .any(|path| path == "src/lantern_user.rs"),
        "a lexical seed expands through the compiler graph: {:?}",
        compiler_paths(&seeded.batch)
    );
    assert!(
        edges_of(&seeded.parsed)
            .iter()
            .any(|edge| edge
                .starts_with("src/evening.rs:2 (evening_bell) --calls--> src/lantern.rs:1")),
        "a lexical seed expands through the manual graph:\n{}",
        seeded.packed.text
    );

    // Crowded: `hostseed` (one word, so a tier-1 run but no anchor) is the
    // only lexical hit; the evening unit follows it on concepts alone,
    // inside the first three units both expansions seed from.
    let answer = corpus.context_as(&slot, "hostseed twilight", Strategy::Graph, BUDGET);
    let path_of = |item: &RankedItem| item.handle.as_ref().unwrap().path.clone();
    let units: Vec<&RankedItem> = answer
        .batch
        .items
        .iter()
        .filter(|item| {
            item.handle.is_some() && item.tier != TIER_COMPILER && item.tier != TIER_OUTLINE
        })
        .collect();
    assert_eq!(path_of(units[0]), "src/host.rs");
    let evening = units
        .iter()
        .position(|item| path_of(item) == "src/evening.rs")
        .expect("the evening unit is a candidate");
    assert!(
        evening < 3,
        "the evening unit ranks at a seed position: {evening}"
    );
    assert!(units[evening].is_dense_only());
    // Neither expansion followed it; both followed the lexical seed.
    assert_eq!(answer.batch.counters.graph, Some("ok"));
    assert_eq!(compiler_paths(&answer.batch), ["src/caller.rs"]);
    let edges = edges_of(&answer.parsed);
    assert_eq!(edges.len(), 1, "{edges:?}");
    assert!(
        edges[0].starts_with("src/host.rs:1 (hostseed) --calls--> src/target.rs:1"),
        "{edges:?}"
    );

    // The delivered response: the edge and the compiler unit are placed
    // ahead of the dense evidence, which is still delivered whole within the
    // normal budget.
    assert!(answer.packed.tokens <= BUDGET);
    assert!(
        answer
            .parsed
            .header
            .iter()
            .any(|segment| segment == "graph:ok"),
        "{:?}",
        answer.parsed.header
    );
    assert_eq!(answer.word(), Some("ready"));
    let at = |found: &dyn Fn(&V2Item) -> bool| {
        answer
            .parsed
            .items
            .iter()
            .position(found)
            .unwrap_or_else(|| panic!("not delivered:\n{}", answer.packed.text))
    };
    let edge = at(&|item| item.kind == testkit::V2Kind::Edge);
    let compiler = at(&|item| {
        item.neural.is_none()
            && !item.handle.is_empty()
            && span_of(&item.handle).0 == "src/caller.rs"
    });
    let whole = |path: &'static str| {
        move |item: &V2Item| {
            !item.handle.is_empty()
                && span_of(&item.handle).0 == path
                && item
                    .neural
                    .as_ref()
                    .is_some_and(|neural| neural.selection == "whole_unit")
        }
    };
    let evening = at(&whole("src/evening.rs"));
    let dusk = at(&whole(DUSK_PATH));
    assert!(
        edge < compiler && compiler < evening && compiler < dusk,
        "{edge} {compiler} {evening} {dusk}:\n{}",
        answer.packed.text
    );
    assert_eq!(
        answer.parsed.items[dusk].body,
        corpus.source(DUSK_PATH),
        "the crowded dense unit is delivered whole"
    );
}

const QUOKKA_PATH: &str = "docs/quokka.md";

/// Three sections that the greedy partition joins into ONE embedding unit;
/// only the middle one says `quokka`, so the lexical hit localizes the unit
/// to a span that ends well before the unit does.
fn quokka_doc() -> String {
    [
        "## Alpha shelf\n\n",
        "The alpha shelf holds spare cables, two label printers and a box of\n",
        "fuses. Nothing on it is urgent and nothing on it moves often.\n\n",
        "## Bravo shelf\n\n",
        "The quokka enclosure log starts on this shelf. Keepers write the\n",
        "feeding times, the water checks and the weight of every animal in\n",
        "the green binder, then initial the page before the evening round.\n",
        "Visitors may read the binder but never write in it, and a torn page\n",
        "is copied into the spare binder the same day. The vet signs the\n",
        "monthly summary on the last page, and the summary is photographed\n",
        "and filed with the shelf inventory before the binder is closed.\n\n",
        "## Charlie shelf\n\n",
        "The charlie shelf holds the visitor badges and the spare radio\n",
        "batteries, counted every Friday afternoon by the lead on duty.\n",
    ]
    .concat()
}

/// Gap 5: an oversized LOCALIZED lexical span. When neither the dense unit
/// nor its lexical span fits, the preview is a prefix of the SPAN, and its
/// continuation ends at the span's end, never at the larger unit's end.
#[test]
fn an_oversized_localized_span_previews_the_span_and_continues_to_the_span_end() {
    let corpus = Corpus::new(&[(QUOKKA_PATH.to_owned(), quokka_doc())]);
    let slot = corpus.slot();
    let content = corpus.source(QUOKKA_PATH);
    let whole = corpus.context(&slot, "quokka", BUDGET);
    let evidence = whole
        .batch
        .items
        .iter()
        .filter_map(|item| item.semantic.as_ref())
        .find(|evidence| evidence.matched.path == QUOKKA_PATH && evidence.span.is_some())
        .expect("a dense unit localized by the lexical hit");
    let unit = evidence.matched.clone();
    let (span, _, _) = evidence.span.clone().unwrap();
    // The geometry the case needs: the unit is the whole document, the span
    // its middle section, ending before the unit does.
    let hit = content.find("quokka").unwrap() as u64;
    let charlie = content.find("## Charlie").unwrap() as u64;
    assert_eq!((unit.start, unit.end), (0, content.len() as u64));
    assert!(
        span.start <= hit && hit < span.end && span.end <= charlie,
        "{}..{} holds {hit} and stops before {charlie}",
        span.start,
        span.end
    );
    assert!(span.end < unit.end);
    let unit_handle = unit.to_v2();
    // The sweep packs this document's candidates only (the dense unit and
    // the lexical section), through the production packer: quick, and no
    // unrelated candidate decides which rung fits.
    let mut batch = whole.batch.clone();
    batch.items.retain(|item| {
        item.handle
            .as_ref()
            .is_some_and(|handle| handle.path == QUOKKA_PATH)
    });
    let mut previews = 0;
    for tokens in sweep() {
        let Some((packed, parsed)) = pack(&batch, tokens) else {
            continue;
        };
        assert!(packed.tokens <= tokens);
        let Some(item) = preview_of(&parsed, &unit_handle) else {
            continue;
        };
        previews += 1;
        // The item handle names a prefix of the span; `next:` names exactly
        // the rest of the SPAN.
        let (_, start, end) = span_of(&item.handle);
        assert_eq!(start, span.start, "a preview of the span, not the unit");
        assert!(start < end && end < span.end, "{start}..{end}");
        assert_eq!(item.body, content[start as usize..end as usize]);
        let next = item
            .neural
            .as_ref()
            .unwrap()
            .next
            .as_deref()
            .expect("a continuation");
        assert_eq!(next, handle_with_range(&item.handle, end, span.end));
        let rest = corpus.engine().retrieve(next, None, 4096).unwrap();
        assert_eq!(
            [item.body.as_bytes(), rest.span.as_slice()].concat(),
            content.as_bytes()[span.start as usize..span.end as usize]
        );
    }
    assert!(
        previews >= 1,
        "some budget previews the localized span:\n{}",
        whole.packed.text
    );
}

const GLYPHS_PATH: &str = "docs/glyphs.md";

/// CRLF lines, multibyte text throughout, and fence-like lines: a balanced
/// four-tick block holding a three-tick line and a literal `next:` line, a
/// six-tick block indented three spaces, and a nine-tick run indented four
/// (which the fence rule does not count). Only the middle section says
/// `wombat`; every section carries sundown concepts.
fn glyphs_doc() -> String {
    [
        "## Glyph register\r\n",
        "\r\n",
        "Café crème at sundown — the amber glow ✓ fades; 😀 nightfall.\r\n",
        "漢字 dusk 漢字 horizon 漢字 glow 漢字 ünïcödé façade naïve.\r\n",
        "\r\n",
        "## Fence notes\r\n",
        "\r\n",
        "The wombat log keeps its fences — é, 😀, ✓:\r\n",
        "\r\n",
        "````text\r\n",
        "``` inner fence é\r\n",
        "next: docs/glyphs.md#0-1@00000000000000000000000000000000.0000000000000000\r\n",
        "````\r\n",
        "\r\n",
        "   ``````\r\n",
        "six ticks 漢字 😀\r\n",
        "``````\r\n",
        "\r\n",
        "    ````````` nine ticks indented four\r\n",
        "\r\n",
        "Über 😀😀 ✓✓ naïve façade 漢字漢字 wombat.\r\n",
        "\r\n",
        "## Closing glyphs\r\n",
        "\r\n",
        "Été — 😀 — ✓ — 漢字 — sunset glow at dusk.\r\n",
    ]
    .concat()
}

/// The backtick run that begins `line` after at most three spaces.
fn fence_run(line: &str) -> usize {
    let trimmed = line.trim_start_matches(' ');
    if line.len() - trimmed.len() > 3 {
        return 0;
    }
    trimmed.bytes().take_while(|&byte| byte == b'`').count()
}

/// The raw opening fence of the item whose line starts with `handle`.
fn fence_after(text: &str, handle: &str) -> usize {
    let line = text
        .find(&format!("\n{handle} L"))
        .unwrap_or_else(|| panic!("no item line for {handle}"));
    let fence = &text[line + 1..][text[line + 1..].find('\n').unwrap() + 1..];
    fence.bytes().take_while(|&byte| byte == b'`').count()
}

/// Gap 6: semantic forms over CRLF, multibyte and fence-like source. Every
/// whole-unit, lexical-span and preview item at every budget carries the
/// exact bytes of its handle (CR kept), ends on a character boundary, names
/// the lines its range touches, is fenced longer than any fence-like line in
/// its body, and a preview's continuation reassembles the selected range.
#[test]
fn semantic_forms_keep_crlf_multibyte_and_fence_like_bytes_exact() {
    let corpus = Corpus::new(&[(GLYPHS_PATH.to_owned(), glyphs_doc())]);
    let slot = corpus.slot();
    let content = corpus.source(GLYPHS_PATH);
    assert!(content.contains("\r\n") && !content.is_ascii());
    let line_of = |at: u64| {
        1 + content.as_bytes()[..at as usize]
            .iter()
            .filter(|&&b| b == b'\n')
            .count()
    };
    let mut seen: BTreeSet<&'static str> = BTreeSet::new();
    let mut multibyte_splits = 0;
    let (mut widest_fence, mut literal_next) = (0, false);
    // Unlocalized (concepts only) and localized (`wombat`).
    for query in ["twilight onset", "wombat"] {
        let whole = corpus.context(&slot, query, BUDGET);
        let evidence = whole
            .batch
            .items
            .iter()
            .filter_map(|item| item.semantic.as_ref())
            .find(|evidence| evidence.matched.path == GLYPHS_PATH)
            .unwrap_or_else(|| panic!("{query}: the glyph unit is dense evidence"));
        let unit = evidence.matched.clone();
        let span = evidence.span.as_ref().map(|(span, _, _)| span.clone());
        assert_eq!(span.is_some(), query == "wombat", "{query}");
        // A preview continues to the end of the selected range.
        let selected_end = span.as_ref().map_or(unit.end, |span| span.end);
        let unit_handle = unit.to_v2();
        // This document's candidates only, as in the localized-span case.
        let mut batch = whole.batch.clone();
        batch.items.retain(|item| {
            item.handle
                .as_ref()
                .is_some_and(|handle| handle.path == GLYPHS_PATH)
        });
        for tokens in sweep().chain([BUDGET]) {
            let Some((packed, parsed)) = pack(&batch, tokens) else {
                continue;
            };
            assert!(packed.tokens <= tokens);
            for item in &parsed.items {
                let Some(neural) = item.neural.as_ref() else {
                    continue;
                };
                let (path, start, end) = span_of(&item.handle);
                if path != GLYPHS_PATH {
                    continue;
                }
                let selection: &'static str = match neural.selection.as_str() {
                    "whole_unit" => {
                        assert_eq!((start, end), (unit.start, unit.end));
                        "whole_unit"
                    }
                    "lexical_span" => {
                        let span = span.as_ref().expect("a span only when localized");
                        assert_eq!((start, end), (span.start, span.end));
                        assert_eq!(neural.matched.as_deref(), Some(unit_handle.as_str()));
                        "lexical_span"
                    }
                    "preview" => {
                        assert_eq!(neural.matched.as_deref(), Some(unit_handle.as_str()));
                        let base = span.as_ref().map_or(unit.start, |span| span.start);
                        assert_eq!(start, base, "{query}");
                        let next = neural.next.as_deref().expect("a continuation");
                        assert_eq!(next, handle_with_range(&item.handle, end, selected_end));
                        let rest = corpus.engine().retrieve(next, None, 4096).unwrap();
                        assert_eq!(
                            [item.body.as_bytes(), rest.span.as_slice()].concat(),
                            content.as_bytes()[base as usize..selected_end as usize]
                        );
                        let bytes = content.as_bytes();
                        if !bytes[end as usize - 1].is_ascii() || !bytes[end as usize].is_ascii() {
                            multibyte_splits += 1;
                        }
                        "preview"
                    }
                    other => panic!("unknown selection {other}"),
                };
                seen.insert(selection);
                // Exact bytes (CR kept) on character boundaries.
                assert!(content.is_char_boundary(start as usize));
                assert!(content.is_char_boundary(end as usize));
                assert_eq!(item.body, content[start as usize..end as usize]);
                // The lines the range touches, counted by LF.
                assert_eq!(
                    item.lines.as_deref(),
                    Some(format!("L{}-{}", line_of(start), line_of(end - 1)).as_str()),
                    "{}",
                    item.handle
                );
                // The fence outruns every fence-like body line.
                let longest = item.body.split('\n').map(fence_run).max().unwrap_or(0);
                let fence = fence_after(&packed.text, &item.handle);
                assert_eq!(fence, 3.max(longest + 1), "{}", item.handle);
                widest_fence = widest_fence.max(fence);
                literal_next |= item.body.contains("\r\nnext: docs/glyphs.md#0-1@");
            }
        }
    }
    assert_eq!(
        seen,
        BTreeSet::from(["whole_unit", "lexical_span", "preview"]),
        "every form was exercised"
    );
    assert!(
        multibyte_splits >= 1,
        "some preview split retreated to a multibyte boundary"
    );
    assert_eq!(
        widest_fence, 7,
        "a delivered body held the six-tick fence-like line"
    );
    assert!(
        literal_next,
        "a literal `next:` body line stayed body content"
    );
}

// --- The MCP owner: byte cap, allowance, multi-root, byte identity --------

/// One in-process MCP owner over the stdio transport (a duplex pipe), with
/// the given references, semantic configuration and budget policy. It opens
/// every store itself.
async fn owner_with(
    store: &Path,
    root: &Path,
    references: Vec<context_foundry::roots::ReferenceSpec>,
    semantic: Option<SemanticServing>,
    budget: BudgetConfig,
) -> Served {
    let (client_io, server_io) = tokio::io::duplex(1 << 21);
    let (input, output) = tokio::io::split(server_io);
    let server = tokio::spawn(mcp::serve_streams(
        ServerOptions {
            store: store.to_path_buf(),
            root: root.to_path_buf(),
            references,
            no_memory: false,
            semantic,
            policy: None,
            budget,
        },
        input,
        output,
    ));
    let client = ().serve(client_io).await.unwrap();
    Served {
        client,
        server: Some(server),
    }
}

impl Corpus {
    /// This corpus's concept provider as an owner's semantic configuration.
    fn serving(&self) -> SemanticServing {
        SemanticServing::with_provider(
            self.profile_path.clone(),
            maker(self.profile.descriptor.clone(), self.probe.clone()),
        )
    }

    /// This corpus as a `--reference ROOT=STORE` admission.
    fn reference(&self) -> context_foundry::roots::ReferenceSpec {
        context_foundry::roots::ReferenceSpec {
            root: self.root.clone(),
            store: self.store.clone(),
        }
    }

    /// The `ws16` this store's handles carry.
    fn ws16(&self) -> String {
        self.engine().workspace_id().unwrap()[..16].to_owned()
    }
}

/// The `ws16` of a v2 handle `path#start-end@sha32.ws16`.
fn ws16_of(handle: &str) -> &str {
    handle.rsplit_once('.').expect("a handle identity").1
}

fn budget_of(policy: serde_json::Value) -> BudgetConfig {
    BudgetConfig::from_object(&policy).expect("a valid budget policy")
}

/// The bytes the MCP boundary measures for a success text: the typed
/// `CallToolResult` the owner emits (one text block, `resultType` cleared),
/// serialized.
fn emitted_len(text: &str) -> usize {
    let mut result = rmcp::model::CallToolResult::success(vec![rmcp::model::ContentBlock::text(
        text.to_owned(),
    )]);
    result.result_type = None;
    serde_json::to_string(&result).unwrap().len()
}

/// The header's one `semantic:` segment and the text without it.
fn split_semantic_word(text: &str) -> (String, String) {
    let (header, body) = text.split_once('\n').expect("a header line");
    let segments: Vec<&str> = header.split(" · ").collect();
    let words: Vec<&str> = segments
        .iter()
        .copied()
        .filter(|segment| segment.starts_with("semantic:"))
        .collect();
    assert_eq!(words.len(), 1, "one semantic segment: {header}");
    let rest: Vec<&str> = segments
        .into_iter()
        .filter(|segment| !segment.starts_with("semantic:"))
        .collect();
    (words[0].to_owned(), format!("{}\n{body}", rest.join(" · ")))
}

/// The `semantic:` word of a v2 text, if its header carries one.
fn word_of(text: &str) -> Option<String> {
    header_word(&parse_v2(text).unwrap()).map(str::to_owned)
}

/// Tab-heavy functions: about 16 source bytes per o200k token, but every
/// tab and line feed doubles under JSON escaping.
fn tab_functions() -> Vec<(String, String)> {
    let body = format!("{}\n", "\t".repeat(200)).repeat(230);
    ["a", "b", "c"]
        .iter()
        .map(|name| {
            (
                format!("src/tabs_{name}.rs"),
                format!("pub fn tabfill_{name}() -> &'static str {{\n    r\"\n{body}\"\n}}\n"),
            )
        })
        .collect()
}

/// Gap 6 (MCP byte cap): a semantic context at the 32768-token maximum whose
/// stdout measure delivers a result that serializes past 256 KiB. Packed
/// against the MCP measure — the typed result the owner emits, escaped and
/// serialized, as [`emitted_len`] reproduces it — the very same candidates
/// stay within the cap, every form measured with its selection tag, and the
/// result still carries the semantic word and the dense evidence. The owner
/// runs exactly this packing (`response::pack_context` with its emitted
/// measure) under its 5 s read deadline, which a debug build cannot meet for
/// a quarter-megabyte result, so the packing is driven directly here.
#[test]
fn a_semantic_context_packed_for_mcp_is_capped_on_its_serialized_bytes() {
    const QUERY: &str = "tabfill twilight onset";
    let corpus = Corpus::new(&tab_functions());
    let slot = corpus.slot();
    let batch = mcp::context_primary(
        &slot,
        None,
        corpus.engine(),
        QUERY,
        Strategy::Auto,
        &Corpus::control(),
        false,
        None,
    )
    .unwrap()
    .batch;
    let budget = Budget::request(32768);
    let stdout = response::pack_context(&batch, budget, &response::stdout_bytes).unwrap();
    assert!(
        emitted_len(&stdout.text) > mcp::OUTPUT_BYTE_CAP,
        "the cap binds: {} serialized bytes",
        emitted_len(&stdout.text)
    );
    let measured = response::pack_context(&batch, budget, &emitted_len).unwrap();
    assert!(
        emitted_len(&measured.text) <= mcp::OUTPUT_BYTE_CAP,
        "{} serialized bytes",
        emitted_len(&measured.text)
    );
    assert!(measured.tokens <= 32768);
    assert!(
        measured.text.len() < stdout.text.len(),
        "the cap, not the token budget, shortened the result"
    );
    let parsed = parse_v2(&measured.text).unwrap();
    assert_eq!(header_word(&parsed), Some("ready"));
    let dusk = item_of(&parsed, DUSK_PATH).expect("the dense evidence is delivered");
    assert_eq!(dusk.neural.as_ref().unwrap().selection, "whole_unit");
    assert!(
        parsed
            .items
            .iter()
            .filter(|item| item.neural.is_some())
            .count()
            > 1,
        "{:?}",
        parsed.header
    );
    for item in parsed.items.iter().filter(|item| item.form.is_none()) {
        let (path, start, end) = span_of(&item.handle);
        assert_eq!(
            item.body.as_bytes(),
            &corpus.source(&path).as_bytes()[start as usize..end as usize],
            "{}",
            item.handle
        );
    }
}

/// Gap 6 (MCP allowance): semantic deliveries are charged exactly the tokens
/// counted on their emitted text (the `semantic:` word and the selection
/// tags included), and a refused semantic request is charged nothing: the
/// delivery right after the refusal MUST succeed with exactly the uncharged
/// rest as its session budget, and so must every later one until the
/// allowance is exhausted by name.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn semantic_deliveries_charge_the_session_allowance_exactly_their_counted_tokens() {
    const ALLOWANCE: usize = 2600;
    let mut corpus = Corpus::new(&[]);
    let semantic = corpus.serving();
    corpus.engine = None;
    let served = owner_with(
        &corpus.store,
        &corpus.root,
        Vec::new(),
        Some(semantic),
        budget_of(serde_json::json!({
            "v": 1, "max_context_tokens": 2048, "session_context_tokens": ALLOWANCE
        })),
    )
    .await;
    let ask = |tokens: usize| serde_json::json!({"query": DUSK_QUERY, "tokens": tokens});
    let first = served.ok("context", ask(BUDGET)).await;
    let parsed = parse_v2(&first).unwrap();
    assert_eq!(header_word(&parsed), Some("ready"));
    assert!(parsed.header.iter().any(|segment| segment == "budget:2048"));
    let dusk = item_of(&parsed, DUSK_PATH).expect("dense evidence");
    assert_eq!(dusk.neural.as_ref().unwrap().selection, "whole_unit");
    let charged = response::count_tokens(&first);
    assert!(
        charged > ALLOWANCE - BUDGET,
        "the next request is allowance-limited ({charged} charged)"
    );
    // A semantic request that cannot fit even its header is refused and
    // charged nothing.
    let (refused, refusal) = served.raw("context", ask(1)).await;
    assert!(
        refused && refusal.contains(r#""code":"budget_too_small""#),
        "{refusal}"
    );
    // The refusal charged nothing: the next semantic request is delivered,
    // limited by exactly the allowance the first delivery left.
    let mut remaining = ALLOWANCE - charged;
    let (error, text) = served.raw("context", ask(BUDGET)).await;
    assert!(!error, "the refusal charged nothing: {text}");
    let mut delivered = charged;
    let mut text = text;
    let exhausted = loop {
        let parsed = parse_v2(&text).unwrap();
        assert!(
            parsed
                .header
                .contains(&format!("budget:{remaining}(session)")),
            "exactly the uncharged rest remains ({remaining}): {:?}",
            parsed.header
        );
        assert_eq!(header_word(&parsed), Some("ready"));
        let tokens = response::count_tokens(&text);
        assert!(tokens <= remaining);
        delivered += tokens;
        remaining -= tokens;
        let (error, next) = served.raw("context", ask(BUDGET)).await;
        if error {
            break next;
        }
        text = next;
    };
    assert!(
        exhausted.contains(r#""code":"budget_exhausted""#),
        "{exhausted}"
    );
    assert!(delivered <= ALLOWANCE);
    // The refusal charged nothing either: the same refusal again.
    let (error, again) = served.raw("context", ask(BUDGET)).await;
    assert!(
        error && again.contains(r#""code":"budget_exhausted""#),
        "{again}"
    );
    served.close().await;
}

#[derive(Clone, Copy, Debug)]
enum Op {
    Search,
    Context,
}

impl Op {
    fn tool(self) -> &'static str {
        match self {
            Op::Search => "search",
            Op::Context => "context",
        }
    }

    /// Budgets wide enough that no candidate is omitted, so a header word
    /// cannot change which items fit.
    fn tokens(self) -> usize {
        match self {
            Op::Search => 4000,
            Op::Context => 8000,
        }
    }

    fn arguments(self, query: &str, roots: Option<&[&str]>) -> serde_json::Value {
        let mut arguments = match self {
            Op::Search => serde_json::json!({"query": query, "limit": 10, "tokens": self.tokens()}),
            Op::Context => serde_json::json!({"query": query, "tokens": self.tokens()}),
        };
        if let Some(roots) = roots {
            arguments["roots"] = serde_json::json!(roots);
        }
        arguments
    }
}

/// The plain baseline a build without semantics serves for one single-root
/// request: the lexical engine path and the packer.
fn plain_single(engine: &Engine, op: Op, query: &str) -> String {
    let budget = Budget::request(op.tokens());
    match op {
        Op::Search => response::pack_search(
            &engine.search_in(query, None, 10).unwrap(),
            budget,
            &response::stdout_bytes,
        ),
        Op::Context => response::pack_context(
            &engine
                .context_candidates(query, Strategy::Auto, &Control::unbounded())
                .unwrap(),
            budget,
            &response::stdout_bytes,
        ),
    }
    .unwrap()
    .text
}

/// The same for the listed roots of a multi-root owner: the anchors chosen
/// once over every root, each root's lexical batch with them, the 007 merge
/// and the per-root header segments.
fn plain_roots(
    roots: &[(&context_foundry::roots::AdmittedRoot, &Engine)],
    op: Op,
    query: &str,
) -> String {
    use context_foundry::roots::{RootBatch, merge_context, merge_search, select_anchors};
    use context_foundry::store::ContextOptions;
    let unbounded = Control::unbounded();
    let engines: Vec<&Engine> = roots.iter().map(|(_, engine)| *engine).collect();
    let anchors = select_anchors(&engines, query, None, &unbounded).unwrap();
    let options = ContextOptions {
        anchors: Some(&anchors),
        ..ContextOptions::default()
    };
    let mut batches = Vec::new();
    let mut headers = Vec::new();
    for (root, engine) in roots {
        let batch = match op {
            Op::Search => {
                engine.search_candidates_with(query, None, 10, &unbounded, Some(&anchors))
            }
            Op::Context => engine
                .context_candidates_with(query, Strategy::Auto, &unbounded, &options)
                .map(|context| context.batch),
        }
        .unwrap();
        headers.push(response::RootHeader {
            alias: root.alias.clone(),
            label: root.label.clone(),
            serving: Some((
                batch.freshness.source_revision,
                batch.freshness.scan_state.clone(),
                batch.freshness.pending_sources,
            )),
            coverage: None,
        });
        batches.push(RootBatch {
            alias: root.alias.clone(),
            batch,
        });
    }
    let budget = Budget::request(op.tokens());
    match op {
        Op::Search => response::pack_search_roots(
            &merge_search(&batches, 10),
            &headers,
            budget,
            &response::stdout_bytes,
        ),
        Op::Context => response::pack_context_roots(
            &merge_context(&batches),
            &headers,
            budget,
            &response::stdout_bytes,
        ),
    }
    .unwrap()
    .text
}

/// One `foundry` CLI request's stdout.
fn cli_text(store: &Path, op: Op, query: &str, profile: Option<&Path>) -> String {
    let mut command = std::process::Command::new(env!("CARGO_BIN_EXE_foundry"));
    command.arg("--store").arg(store);
    let tokens = op.tokens().to_string();
    match op {
        Op::Search => command.args(["search", query, "--limit", "10", "--tokens", &tokens]),
        Op::Context => command.args(["context", query, "--tokens", &tokens]),
    };
    if let Some(profile) = profile {
        command.arg("--semantic-profile").arg(profile);
    }
    let output = command.output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}

/// Gap 3: the no-profile byte-identity matrix WITH positive hits. On this
/// build, which has semantics, a command or owner without a profile serves
/// exactly the plain baseline: CLI search and context, MCP search and
/// context, single- and multi-root. A profile configured but not usable for
/// the store (the CLI without development isolation; an MCP runtime over an
/// unprepared store) changes the bytes by its one documented header word
/// only, and the unprepared store spends no embedding.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn without_a_usable_profile_cli_and_mcp_answers_are_the_plain_baseline_byte_for_byte() {
    let mut primary = Corpus::unprepared(&[]);
    let mut secondary = Corpus::unprepared(&[]);
    let admitted =
        context_foundry::roots::validate_admission(&primary.root, &[secondary.reference()])
            .unwrap();
    let mut cases = Vec::new();
    // Anchor-less queries: an anchored context counts every candidate
    // outside its selection in `omitted:` (context-v2 § Anchored context).
    for query in ["parse record", "shift lead rota", "fire lane"] {
        for op in [Op::Search, Op::Context] {
            let single = plain_single(primary.engine(), op, query);
            let multi = plain_roots(
                &[
                    (&admitted[0], primary.engine()),
                    (&admitted[1], secondary.engine()),
                ],
                op,
                query,
            );
            for text in [&single, &multi] {
                let parsed = parse_v2(text).unwrap();
                assert!(!parsed.items.is_empty(), "positive hits: {op:?} {query}");
                assert!(
                    !parsed
                        .header
                        .iter()
                        .any(|segment| segment.starts_with("omitted:")),
                    "{op:?} {query}: {:?}",
                    parsed.header
                );
            }
            cases.push((op, query, single, multi));
        }
    }
    primary.engine = None;
    secondary.engine = None;

    // The CLI (single-root).
    for (op, query, single, _) in &cases {
        assert_eq!(
            &cli_text(&primary.store, *op, query, None),
            single,
            "CLI {op:?} {query}"
        );
        let configured = cli_text(&primary.store, *op, query, Some(&primary.profile_path));
        let (word, rest) = split_semantic_word(&configured);
        assert!(word.starts_with("semantic:fallback:"), "{word}");
        assert_eq!(&rest, single, "CLI with a profile, {op:?} {query}");
    }

    // MCP, single- and multi-root, without and with the profile.
    for multi in [false, true] {
        for profiled in [false, true] {
            let references = if multi {
                vec![secondary.reference()]
            } else {
                Vec::new()
            };
            let served = owner_with(
                &primary.store,
                &primary.root,
                references,
                profiled.then(|| primary.serving()),
                budget_of(serde_json::json!({"v": 1, "max_context_tokens": 8192})),
            )
            .await;
            for (op, query, single, multi_text) in &cases {
                let want = if multi { multi_text } else { single };
                let text = served.ok(op.tool(), op.arguments(query, None)).await;
                if profiled {
                    let (word, rest) = split_semantic_word(&text);
                    assert!(
                        word.starts_with("semantic:fallback:semantic_unprepared"),
                        "{word}"
                    );
                    assert_eq!(word.ends_with("; primary root only"), multi, "{word}");
                    assert_eq!(&rest, want, "MCP multi={multi} profiled {op:?} {query}");
                } else {
                    assert_eq!(&text, want, "MCP multi={multi} {op:?} {query}");
                }
            }
            served.close().await;
        }
    }
    assert_eq!(
        primary.probe.query_calls(),
        0,
        "an unprepared store is refused before any embedding"
    );
}

/// Gap 2: the multi-root matrix. Both roots are prepared for the SAME
/// profile, so any secondary-root use of the runtime would show. The header
/// word covers the primary root only and says so whenever another root
/// serves; no secondary item is ever dense evidence (the secondary's own
/// prepared dusk unit never appears); a secondary-only request spends no
/// embedding and is the plain baseline byte for byte.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn semantic_evidence_stays_primary_only_in_a_multi_root_owner() {
    // Anchor-less: an anchored context places no dense units (context-v2
    // § Anchored context).
    const QUERY: &str = "parse record twilight onset";
    let mut primary = Corpus::new(&[]);
    let mut secondary = Corpus::new(&[]);
    assert!(
        secondary.report.index_published,
        "the secondary store is prepared too"
    );
    let (primary_ws, secondary_ws) = (primary.ws16(), secondary.ws16());
    let admitted =
        context_foundry::roots::validate_admission(&primary.root, &[secondary.reference()])
            .unwrap();
    let secondary_alias = admitted[1].alias.clone();
    let plain: Vec<(Op, String)> = [Op::Search, Op::Context]
        .into_iter()
        .map(|op| {
            (
                op,
                plain_roots(&[(&admitted[1], secondary.engine())], op, QUERY),
            )
        })
        .collect();
    primary.engine = None;
    secondary.engine = None;
    let served = owner_with(
        &primary.store,
        &primary.root,
        vec![secondary.reference()],
        Some(primary.serving()),
        budget_of(serde_json::json!({"v": 1, "max_context_tokens": 8192})),
    )
    .await;

    // Both roots serve: one embedding, for the primary; the word says so.
    for op in [Op::Search, Op::Context] {
        let before = primary.probe.query_calls();
        let text = served.ok(op.tool(), op.arguments(QUERY, None)).await;
        assert_eq!(primary.probe.query_calls(), before + 1, "{op:?}");
        assert_eq!(
            word_of(&text).as_deref(),
            Some("ready; primary root only"),
            "{op:?}"
        );
        let (mut dense, mut secondary_items) = (0, 0);
        for item in parse_v2(&text).unwrap().items {
            if item.handle.is_empty() {
                continue;
            }
            let semantic = item.neural.is_some() || item.label.as_deref() == Some("semantic");
            if ws16_of(&item.handle) == secondary_ws {
                secondary_items += 1;
                assert!(
                    !semantic,
                    "a secondary item is dense evidence: {}",
                    item.handle
                );
                assert_ne!(span_of(&item.handle).0, DUSK_PATH, "{op:?}");
            } else {
                assert_eq!(ws16_of(&item.handle), primary_ws);
                dense += usize::from(semantic);
            }
        }
        assert!(dense > 0, "{op:?}: the primary's dense evidence:\n{text}");
        assert!(
            secondary_items > 0,
            "{op:?}: the secondary's lexical hits:\n{text}"
        );
    }
    // The primary alone, and a selection listing the secondary first.
    let text = served
        .ok("context", Op::Context.arguments(QUERY, Some(&["primary"])))
        .await;
    assert_eq!(word_of(&text).as_deref(), Some("ready"));
    let text = served
        .ok(
            "context",
            Op::Context.arguments(QUERY, Some(&[secondary_alias.as_str(), "primary"])),
        )
        .await;
    assert_eq!(word_of(&text).as_deref(), Some("ready; primary root only"));
    // The secondary alone: no embedding, no word, the plain bytes.
    let before = primary.probe.query_calls();
    for (op, plain) in &plain {
        let text = served
            .ok(
                op.tool(),
                op.arguments(QUERY, Some(&[secondary_alias.as_str()])),
            )
            .await;
        assert_eq!(&text, plain, "{op:?}");
        assert_eq!(word_of(&text), None);
    }
    assert_eq!(
        primary.probe.query_calls(),
        before,
        "a secondary-only request spends no embedding"
    );
    served.close().await;
}

// --- A worker dying mid-query ----------------------------------------------

/// The tests in this binary that run a supervised worker serialize here: the
/// footprint test arms the measurement fault process-wide.
#[cfg(target_os = "macos")]
static SUPERVISED_WORKERS: Mutex<()> = Mutex::new(());

/// Gap 4: the supervised worker dies in the middle of a query embedding,
/// through `QueryRuntime`. The fake worker kills itself (`--die-in-call`)
/// once the query entered its compute phase, which it records first
/// (`--phase-file`): no helper thread races the 1500 ms query ceiling. The
/// request falls back by name (`provider_exited`, never a timeout) with the
/// baseline intact; the runtime then stays in that named unavailable
/// state — every later request falls back the same way, nothing restarts
/// behind a query — and the dead worker (its `--pid-file` PID) was reaped.
#[cfg(target_os = "macos")]
#[test]
fn a_worker_dying_mid_query_falls_back_by_name_and_stays_named_unavailable() {
    use context_foundry::neural::supervisor::WorkerProvider;
    let _worker = SUPERVISED_WORKERS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let corpus = Corpus::new(&[]);
    let work = tempfile::tempdir().unwrap();
    let worker_profile = fake_worker_profile(work.path());
    let pid_file = work.path().join("worker.pid");
    let phase_file = work.path().join("worker.phase");
    let hooks: Vec<String> = [
        "--pid-file",
        pid_file.to_str().unwrap(),
        "--phase-file",
        phase_file.to_str().unwrap(),
        "--die-in-call",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect();
    let descriptor = corpus.profile.descriptor.clone();
    let slot = mcp::semantic_slot(Some(SemanticServing::with_provider(
        corpus.profile_path.clone(),
        Box::new(move || {
            let worker = WorkerProvider::launch(&worker_profile, hooks)?;
            Ok(Box::new(Relabeled { worker, descriptor }) as Box<dyn EmbeddingProvider>)
        }),
    )))
    .unwrap();
    assert!(matches!(slot, Some(Ok(_))), "the worker started");
    let pid: libc::pid_t = std::fs::read_to_string(&pid_file)
        .unwrap()
        .trim()
        .parse()
        .unwrap();
    let baseline = corpus.baseline(DUSK_QUERY, BUDGET);
    let died = corpus.context(&slot, DUSK_QUERY, BUDGET);
    assert_eq!(
        std::fs::read_to_string(&phase_file).unwrap().lines().last(),
        Some("call"),
        "the worker died inside the query call"
    );
    let (word, rest) = split_semantic_word(&died.packed.text);
    assert!(
        word.starts_with("semantic:fallback:provider_exited"),
        "{word}"
    );
    assert_eq!(rest, baseline.packed.text, "the baseline is intact");
    // The named unavailable state: every later request, context or search.
    for _ in 0..2 {
        let later = corpus.context(&slot, DUSK_QUERY, BUDGET);
        let (word, rest) = split_semantic_word(&later.packed.text);
        assert!(
            word.starts_with("semantic:fallback:provider_exited"),
            "{word}"
        );
        assert_eq!(rest, baseline.packed.text);
        let search = corpus.search(&slot, DUSK_QUERY, 10);
        assert!(
            search
                .word()
                .is_some_and(|word| word.starts_with("fallback:provider_exited")),
            "{:?}",
            search.word()
        );
    }
    // The dead worker was reaped, not left a zombie behind the runtime.
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        // SAFETY: signal 0 only probes whether the PID still exists.
        let gone = unsafe { libc::kill(pid, 0) } == -1
            && std::io::Error::last_os_error().raw_os_error() == Some(libc::ESRCH);
        if gone {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "the dead worker was never reaped"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}
