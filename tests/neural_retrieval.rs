//! 009 T002/T004 acceptance (FR-003, FR-005, FR-006 / SC-002): semantic
//! evidence reaches the final context response inside the existing budget,
//! with source freshness, the dense-first placement and named baseline
//! fallbacks.
//!
//! The vocabulary-gap cases run the real preparation path, the real Rust
//! tokenizer, the real USearch index and the real request path
//! (`mcp::context_primary_with_options`/`search_primary`, the functions the
//! MCP owner and the CLI both call). The embedding function is a test
//! provider with a fixed concept lexicon: it makes the ORDERING, budget and
//! fallback assertions exact, and it is NOT a retrieval-quality result — the
//! actual model runs the same fixture through the production binary and
//! records its own evidence. Every provider call is counted.
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
use context_foundry::neural::prepare::{self, Acquire, PrepareOptions, PrepareReport};
use context_foundry::neural::profile::SemanticProfile;
use context_foundry::neural::provider::{
    self, EmbeddingProvider, FunctionDescriptor, ProviderError, TokenizedInput,
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

/// The fixture profile's output dimension.
const DIMENSIONS: usize = testkit::FIXTURE_DIMENSIONS as usize;

// ---------------------------------------------------------------------------
// The fixture: tests/fixtures/semantic-retrieval, with FROZEN expected spans.
// ---------------------------------------------------------------------------

/// The normal response budget (tokens).
const BUDGET: usize = 2048;

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
        .strip_prefix("doc: ")
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
    document_batches: Arc<Mutex<Vec<Vec<usize>>>>,
    document_keys: Arc<Mutex<Vec<String>>>,
    publication_path: Arc<Mutex<Option<PathBuf>>>,
    publication_before_call: Arc<Mutex<Vec<(u64, u64)>>>,
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
        provider::check_document_batch(batch, self.descriptor.input_limits())?;
        if let Some(path) = self.probe.publication_path.lock().unwrap().as_ref() {
            let count = std::fs::read(path)
                .ok()
                .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok())
                .and_then(|manifest| manifest["count"].as_u64())
                .unwrap_or(0);
            self.probe
                .publication_before_call
                .lock()
                .unwrap()
                .push((self.probe.document_inputs(), count));
        }
        let digest = self.descriptor.digest();
        self.probe.document_keys.lock().unwrap().extend(
            batch
                .iter()
                .map(|input| provider::input_key(&digest, &model_input(&input.ids))),
        );
        self.probe
            .document_batches
            .lock()
            .unwrap()
            .push(batch.iter().map(|input| input.ids.len()).collect());
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
        provider::check_query_with_limit(input, self.descriptor.input_limits().tokens)?;
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
        let acquire: Acquire = Box::new(move |_profile, _artifacts, _development, _control| {
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
                priority_paths: &[],
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
        let combined = mcp::context_primary_with_options(
            slot,
            self.engine(),
            query,
            Strategy::Search,
            &Self::control(),
            context_foundry::store::ContextOptions::default(),
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
        .map(|unit| String::from(&*unit.path))
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
// required model): retrieval, placement or packing.
// ---------------------------------------------------------------------------

#[derive(Debug, PartialEq, Eq)]
enum Owner {
    /// Neither the lexical nor the dense window carried the candidate.
    Retrieval,
    /// A window carried it; the placed, capped and cut selection dropped it.
    Placement,
    /// Selected and ranked, then omitted by the budget.
    Packing,
    /// Delivered (verbatim or in its signature form).
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
        return Owner::Placement;
    }
    match item_of(parsed, path) {
        None => Owner::Packing,
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
        assert!(item.semantic, "{path}: a placed dense unit is tagged");
        // The card's unit is the definition or section the evidence is: the
        // delivered range is exactly the frozen evidence, no wider.
        let (_, start, end) = span_of(&item.handle);
        assert_eq!(start..end, evidence, "{path}");
        // Source freshness: the delivered bytes are the live file's bytes and
        // the handle names the live content hash.
        let content = corpus.source(path);
        if item.form.is_none() {
            assert_eq!(item.body, content[start as usize..end as usize], "{path}");
        }
        let sha32 = &context_foundry::digest(content.as_bytes())[..32];
        assert!(
            item.handle.contains(&format!("@{sha32}.")),
            "{}",
            item.handle
        );
        // Dense units come first: the evidence is the first item.
        assert_eq!(
            span_of(&answer.parsed.items[0].handle).0,
            path,
            "{}",
            answer.packed.text
        );
        // The exact output budget.
        assert!(answer.packed.tokens <= BUDGET);
        assert_eq!(
            answer.packed.tokens,
            response::count_tokens(&answer.packed.text)
        );
        // The provider saw the exact templated query once.
        assert_eq!(
            corpus.probe.queries().last().map(String::as_str),
            Some(&*format!("query: {query}"))
        );
        delivered.push(serde_json::json!({
            "query": query, "path": path,
            "frozen_evidence": [evidence.start, evidence.end],
            "delivered": [start, end],
            "form": item.form,
            "delivered_tokens": answer.packed.tokens,
        }));
    }
    // The long document delivers only its last section and the code file
    // only the retry function, never the whole file.
    const { assert!(LEDGER_EVIDENCE.start > 0 && CODE_EVIDENCE.end < CODE_LEN) };
    let ledger = corpus.context(&slot, LEDGER_QUERY, BUDGET);
    assert!(
        !ledger
            .item(LEDGER_PATH)
            .unwrap()
            .body
            .contains("Shipping labels")
    );
    record("vocabulary-gap-fixture", serde_json::json!(delivered));
}

#[test]
fn search_returns_locators_and_never_a_selection_tag() {
    let corpus = Corpus::new(&[]);
    let slot = corpus.slot();
    let answer = corpus.search(&slot, DUSK_QUERY, 10);
    assert_eq!(answer.word(), Some("ready"));
    let item = answer.item(DUSK_PATH).expect("the dense hit is a locator");
    assert_eq!(item.label.as_deref(), Some("semantic"));
    assert!(!item.semantic, "a locator carries no selection tag");
    assert!(!answer.packed.text.contains("[semantic]"));
    // Dense units are placed first in search too.
    assert_eq!(span_of(&answer.parsed.items[0].handle).0, DUSK_PATH);
}

/// Prose intent must agree at model admission and final lexical selection.
#[test]
fn experimental_prose_uses_the_same_anchor_intent_for_model_admission_and_selection() {
    use context_foundry::store::ContextOptions;
    let corpus = Corpus::new(&[
        ("worker.go".into(), "package sample\nfunc Go() {}\n".into()),
        ("cpu.rs".into(), "pub fn Cpu() {}\n".into()),
    ]);
    for shared_anchors in [false, true] {
        let slot = corpus.slot();
        let read = |query: &str, enabled| {
            let anchors = context_foundry::roots::select_anchors(
                &[corpus.engine()],
                query,
                None,
                &Corpus::control(),
            )
            .unwrap();
            mcp::context_primary_with_options(
                &slot,
                corpus.engine(),
                query,
                Strategy::Search,
                &Corpus::control(),
                ContextOptions {
                    anchors: shared_anchors.then_some(anchors.as_slice()),
                    experimental_behavior: enabled,
                    ..Default::default()
                },
            )
            .unwrap()
        };
        for query in [
            "refund timing for a Go string",
            "refund timing for Cpu-heavy source",
        ] {
            let calls = corpus.probe.query_calls();
            assert!(read(query, false).batch.semantic.is_none());
            assert_eq!(corpus.probe.query_calls(), calls);
            assert_eq!(read(query, true).batch.semantic.as_deref(), Some("ready"));
            assert_eq!(
                corpus.probe.query_calls(),
                calls + 1,
                "prose must reach the provider"
            );
        }
        for query in ["`Go`", "call Go string helper", "call `Go` with a callback"] {
            let calls = corpus.probe.query_calls();
            assert!(read(query, true).batch.semantic.is_none(), "{query}");
            assert_eq!(
                corpus.probe.query_calls(),
                calls,
                "explicit symbol must not call the model"
            );
        }
    }
}

/// 009 T004 "Placement": a query with an anchor never calls the model; its
/// search and context answers are the answers without a profile, byte for
/// byte (no `semantic:` word, no tag).
#[test]
fn an_anchored_query_never_calls_the_model_and_is_served_as_without_a_profile() {
    let corpus = Corpus::new(&[]);
    let slot = corpus.slot();
    for query in ["parse_record", "parse_record twilight onset"] {
        let calls = corpus.probe.query_calls();
        let plain_search = corpus.search(&None, query, 10);
        let search = corpus.search(&slot, query, 10);
        assert_eq!(search.packed.text, plain_search.packed.text, "{query}");
        assert_eq!(search.facts.tiers[0], 1, "an exact definition first");
        let plain = corpus.context(&None, query, BUDGET);
        let context = corpus.context(&slot, query, BUDGET);
        assert_eq!(context.packed.text, plain.packed.text, "{query}");
        assert!(!context.packed.text.contains("semantic:"), "{query}");
        assert!(context.parsed.items.iter().all(|item| !item.semantic));
        assert_eq!(corpus.probe.query_calls(), calls, "{query}: no model call");
    }
    // The same words without the identifier are anchor-less: one embedding.
    let calls = corpus.probe.query_calls();
    let answer = corpus.context(&slot, "parse record twilight onset", BUDGET);
    assert_eq!(answer.word(), Some("ready"));
    assert_eq!(corpus.probe.query_calls(), calls + 1);
}

// ---------------------------------------------------------------------------
// The placement: dense units first in similarity order, then today's order.
// ---------------------------------------------------------------------------

#[test]
fn dense_units_come_first_in_similarity_order_then_the_lexical_ones_in_todays_order() {
    let corpus = Corpus::new(&[]);
    let slot = corpus.slot();
    // Anchor-less, with lexical hits (`shift`, `dock`, …) and a dense match.
    let query = "twilight onset shift dock desk badge lot";
    let window = corpus
        .runtime()
        .window(
            corpus.engine(),
            query,
            Instant::now() + Duration::from_secs(30),
            &Control::unbounded(),
        )
        .unwrap();
    let baseline = corpus
        .engine()
        .search_candidates(query, None, 64, &Control::unbounded())
        .unwrap();
    let placed = mcp::search_primary(
        &slot,
        corpus.engine(),
        query,
        None,
        64,
        &Corpus::control(),
        None,
    )
    .unwrap();
    let unit = |item: &context_foundry::store::RankedItem| {
        let handle = item.handle.as_ref().unwrap();
        (handle.path.clone(), handle.start, handle.end)
    };
    let dense: Vec<_> = placed
        .items
        .iter()
        .take_while(|item| item.semantic.is_some())
        .collect();
    assert!(!dense.is_empty());
    assert!(
        placed.items[dense.len()..]
            .iter()
            .all(|item| item.semantic.is_none()),
        "every dense unit precedes every lexical one"
    );
    // The dense units follow the window's similarity order.
    let window_order: Vec<(String, u64, u64)> = window
        .hits
        .iter()
        .flat_map(|hit| window.units(hit))
        .map(|location| (String::from(&*location.path), location.start, location.end))
        .collect();
    let positions: Vec<usize> = dense
        .iter()
        .map(|item| {
            window_order
                .iter()
                .position(|location| *location == unit(item))
                .expect("a placed dense unit is a window location")
        })
        .collect();
    assert!(
        positions.windows(2).all(|pair| pair[0] < pair[1]),
        "{positions:?}"
    );
    // Then the lexical units in the baseline's order: the whole placed list
    // is the placement rule applied to the window and the baseline, never a
    // tail cut to the observed length.
    assert!(!baseline.counters.truncated, "the whole lexical order");
    let lexical: Vec<_> = baseline.items.iter().map(unit).collect();
    let (expected, capped) = expected_placement(&window_order, &lexical, 64);
    let all: Vec<_> = placed.items.iter().map(unit).collect();
    assert_eq!(all, expected);
    assert_eq!(placed.counters.capped, capped);
    let unique: BTreeSet<_> = all.iter().cloned().collect();
    assert_eq!(all.len(), unique.len(), "no unit twice");

    // Experimental prose preserves the diverse first pass, then may backfill
    // unused slots from this same semantic window in its retained order.
    let prose = corpus
        .engine()
        .context_candidates_with(
            query,
            Strategy::Search,
            &Control::unbounded(),
            &context_foundry::store::ContextOptions {
                experimental_behavior: true,
                anchors: Some(&[]),
                dense: Some(&window),
                ..Default::default()
            },
        )
        .unwrap();
    let prose_dense: Vec<_> = prose
        .batch
        .items
        .iter()
        .take_while(|item| item.semantic.is_some())
        .map(unit)
        .collect();
    assert!(prose_dense.len() >= dense.len());
    assert_eq!(
        &prose_dense[..dense.len()],
        dense.iter().map(|i| unit(i)).collect::<Vec<_>>().as_slice()
    );
    for item in prose.batch.items.iter().filter(|i| i.semantic.is_some()) {
        assert!(window_order.contains(&unit(item)));
    }
}

/// One placed unit: `(path, start, end)`.
type PlacedUnit = (String, u64, u64);

/// The baseline's per-file cap (`store::PER_FILE_CAP`).
const PER_FILE_CAP: usize = 4;

/// The 009 T004 placement over independent inputs: the dense window's units
/// in similarity order, then the lexical units in today's order; each unit
/// once, at its first place, at most [`PER_FILE_CAP`] per file (dense units
/// counted), at most `limit`. Returns the placed units and how many units
/// the per-file cap dropped.
fn expected_placement(
    dense: &[PlacedUnit],
    lexical: &[PlacedUnit],
    limit: usize,
) -> (Vec<PlacedUnit>, u64) {
    let mut seen = BTreeSet::new();
    let mut per_file = std::collections::BTreeMap::<&str, usize>::new();
    let (mut placed, mut capped) = (Vec::new(), 0u64);
    for unit in dense.iter().chain(lexical) {
        if !seen.insert(unit) {
            continue;
        }
        let count = per_file.entry(unit.0.as_str()).or_default();
        if *count == PER_FILE_CAP {
            capped += 1;
            continue;
        }
        *count += 1;
        if placed.len() < limit {
            placed.push(unit.clone());
        }
    }
    (placed, capped)
}

/// Review m1: many of the nearest cards share one file, so its dense units
/// hit the per-file cap, and a known lexical-only candidate (a file without
/// cards) must follow the dense units. The exact order is derived from
/// independent inputs (the window's similarity order, today's lexical
/// order), never from the observed residual.
#[test]
fn capped_dense_units_are_followed_by_a_known_lexical_only_candidate_in_exact_order() {
    const GLOW: &str = "docs/glow.md";
    const HARBOR: &str = "notes/harbor.txt";
    let sections: String = (1..=6)
        .map(|n| {
            format!(
                "# Amber hour {n}\n\nSundown, dusk and twilight glow on the horizon at \
                 sunset, hour {n}.\n\n"
            )
        })
        .collect();
    let corpus = Corpus::new(&[
        (GLOW.to_owned(), sections),
        (
            HARBOR.to_owned(),
            "The quayside crane log lists every quayside shift change.\n".to_owned(),
        ),
    ]);
    let slot = corpus.slot();
    let query = "twilight quayside";
    let window = corpus
        .runtime()
        .window(
            corpus.engine(),
            query,
            Instant::now() + Duration::from_secs(30),
            &Control::unbounded(),
        )
        .unwrap();
    let baseline = corpus
        .engine()
        .search_candidates(query, None, 64, &Control::unbounded())
        .unwrap();
    let trace = context_foundry::retrieval_trace::RetrievalTrace::default();
    let placed = mcp::search_primary(
        &slot,
        corpus.engine(),
        query,
        None,
        64,
        &Corpus::control().with_trace(Some(trace.clone())),
        None,
    )
    .unwrap();
    assert_eq!(
        placed.semantic.as_deref(),
        Some("ready"),
        "anchor-less: placed"
    );
    let unit = |item: &context_foundry::store::RankedItem| {
        let handle = item.handle.as_ref().unwrap();
        (handle.path.clone(), handle.start, handle.end)
    };
    let dense: Vec<PlacedUnit> = window
        .hits
        .iter()
        .flat_map(|hit| window.units(hit))
        .map(|location| (String::from(&*location.path), location.start, location.end))
        .collect();
    assert_eq!(
        dense.iter().filter(|unit| unit.0 == GLOW).count(),
        6,
        "all six glow cards are among the nearest"
    );
    assert!(
        dense.iter().all(|unit| unit.0 != HARBOR),
        "a file without cards has no dense unit"
    );
    assert!(!baseline.counters.truncated, "the whole lexical order");
    let lexical: Vec<PlacedUnit> = baseline.items.iter().map(unit).collect();
    assert!(
        lexical.iter().any(|unit| unit.0 == HARBOR),
        "a lexical candidate"
    );
    let (expected, capped) = expected_placement(&dense, &lexical, 64);
    let all: Vec<PlacedUnit> = placed.items.iter().map(unit).collect();
    assert_eq!(all, expected);
    assert_eq!(placed.counters.capped, capped);
    // The glow file: six nearest cards, four placed, each from the window.
    let glow: Vec<_> = placed
        .items
        .iter()
        .filter(|item| unit(item).0 == GLOW)
        .collect();
    assert_eq!(glow.len(), PER_FILE_CAP);
    assert!(glow.iter().all(|item| item.semantic.is_some()));
    assert!(capped >= 2, "the fifth and sixth glow cards were capped");
    let data = trace.snapshot();
    let events = data["events"].as_array().unwrap();
    assert!(events.iter().any(|e| e["stage"] == "selection"
        && e["origin"] == "dense"
        && e["input_key"].is_string()));
    assert_eq!(
        events
            .iter()
            .filter(|e| e["stage"] == "placement" && e["outcome"] == "per_file_cap")
            .count(),
        capped as usize
    );
    assert_eq!(data["truncated"], false);
    // The lexical-only candidate is placed, untagged, after every dense unit.
    let first_lexical = placed
        .items
        .iter()
        .position(|item| item.semantic.is_none())
        .expect("lexical units follow the dense ones");
    assert!(
        placed.items[first_lexical..]
            .iter()
            .all(|item| item.semantic.is_none())
    );
    let harbor = all
        .iter()
        .position(|unit| unit.0 == HARBOR)
        .expect("the lexical-only candidate is placed");
    assert!(harbor >= first_lexical, "{all:?}");
}

/// A unit both windows carry is placed once, at its dense place, tagged
/// `[semantic]` before its own label (context-v2 § Evidence items).
#[test]
fn a_unit_both_dense_and_lexical_is_placed_once_with_its_label_after_the_tag() {
    let corpus = Corpus::new(&[]);
    let slot = corpus.slot();
    // `evening observation` is lexical for the dusk section; `twilight` is
    // its concept.
    let query = "evening observation twilight";
    let lexical = corpus.baseline(query, BUDGET);
    assert!(lexical.item(DUSK_PATH).is_some(), "a lexical hit too");
    let answer = corpus.context(&slot, query, BUDGET);
    let first = &answer.parsed.items[0];
    assert_eq!(
        span_of(&first.handle).0,
        DUSK_PATH,
        "{}",
        answer.packed.text
    );
    assert!(first.semantic);
    assert_eq!(
        first.label.as_deref(),
        Some("section Evening observation notes")
    );
    let line = answer.packed.text.lines().nth(1).unwrap();
    assert!(
        line.ends_with(" [semantic] section Evening observation notes"),
        "{line}"
    );
    let copies = answer
        .parsed
        .items
        .iter()
        .filter(|item| item.handle == first.handle)
        .count();
    assert_eq!(copies, 1, "delivered once");
    let item = answer
        .batch
        .items
        .iter()
        .find(|item| {
            item.handle
                .as_ref()
                .is_some_and(|handle| handle.path == DUSK_PATH)
        })
        .unwrap();
    assert!(!item.is_dense_only(), "it seeds like the lexical hit it is");
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
            .any(|unit| &*unit.path == DUSK_PATH),
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
    let combined = mcp::context_primary_with_options(
        &slot,
        corpus.engine(),
        query,
        Strategy::Search,
        &control,
        context_foundry::store::ContextOptions::default(),
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
        descriptor.model.push_str(" Q8_0");
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
    assert!(!kiln.semantic, "a lexical candidate carries no tag");
    let dusk = answer
        .item(DUSK_PATH)
        .expect("prepared vectors still serve");
    assert!(dusk.semantic);
}

#[test]
fn partial_semantics_preserves_distinct_lexical_passages_and_dense_evidence() {
    use context_foundry::store::ContextOptions;
    let mut corpus = Corpus::new(&[]);
    let body = format!(
        "fn fire() {{\n    twilight_onset_kiln_prepare();\n{}    twilight_onset_kiln_finish();\n}}\n",
        "    ordinary_step();\n".repeat(400)
    );
    corpus
        .engine_mut()
        .replace_source("kiln.rs", &body)
        .unwrap();
    corpus.engine_mut().refresh(&Control::unbounded()).unwrap();
    let runtime = corpus.runtime();
    let query = "twilight onset kiln";
    let window = runtime
        .window(
            corpus.engine(),
            query,
            Instant::now() + Duration::from_secs(30),
            &Corpus::control(),
        )
        .unwrap();
    let candidates = corpus
        .engine()
        .context_candidates_with(
            query,
            Strategy::Search,
            &Corpus::control(),
            &ContextOptions {
                dense: Some(&window),
                experimental_behavior: true,
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(candidates.batch.semantic.as_deref(), Some("partial"));
    let passages: Vec<_> = candidates
        .batch
        .items
        .iter()
        .filter(|i| i.label == "excerpt fn fire")
        .collect();
    assert_eq!(passages.len(), 2);
    let positions: Vec<_> = candidates
        .batch
        .items
        .iter()
        .enumerate()
        .filter(|(_, item)| item.label == "excerpt fn fire")
        .map(|(index, _)| index)
        .collect();
    assert_eq!(
        positions[1],
        positions[0] + 1,
        "dense interleaving must not separate already discovered owner evidence"
    );
    for item in passages {
        let handle = item.handle.as_ref().unwrap();
        assert!(item.semantic.is_none());
        assert_eq!(
            item.forms[0],
            context_foundry::store::RenderedForm::Verbatim(
                body[handle.start as usize..handle.end as usize].into()
            )
        );
    }
    assert!(candidates.batch.items.iter().any(|i| i.semantic.is_some()));
    let packed = response::pack_enriched_context(
        &candidates,
        None,
        response::Budget::request(2048),
        &response::stdout_bytes,
    )
    .unwrap();
    assert!(packed.text.contains("twilight_onset_kiln_prepare"));
    assert!(packed.text.contains("twilight_onset_kiln_finish"));
    assert!(packed.tokens <= 2048);
}

#[test]
fn experimental_partial_prose_preserves_unprepared_lexical_evidence_early() {
    use context_foundry::store::ContextOptions;
    let mut corpus = Corpus::new(&[]);
    corpus
        .engine_mut()
        .replace_source(
            "docs/kiln.md",
            "# Kiln log\n\nThe kiln was fired twice this week.\n",
        )
        .unwrap();
    corpus.engine_mut().refresh(&Control::unbounded()).unwrap();
    let runtime = corpus.runtime();
    let query = "twilight onset kiln";
    let window = runtime
        .window(
            corpus.engine(),
            query,
            Instant::now() + Duration::from_secs(30),
            &Corpus::control(),
        )
        .unwrap();
    let context = |enabled| {
        corpus
            .engine()
            .context_candidates_with(
                query,
                Strategy::Search,
                &Corpus::control(),
                &ContextOptions {
                    dense: Some(&window),
                    experimental_behavior: enabled,
                    ..Default::default()
                },
            )
            .unwrap()
    };
    let ordinary = context(false);
    assert_eq!(
        ordinary.batch.items[0].handle.as_ref().unwrap().path,
        DUSK_PATH
    );
    let experimental = context(true);
    assert_eq!(experimental.batch.semantic.as_deref(), Some("partial"));
    assert_eq!(
        experimental.batch.items[0].handle.as_ref().unwrap().path,
        "docs/kiln.md"
    );
    assert!(experimental.batch.items[0].semantic.is_none());
    assert!(
        experimental
            .batch
            .items
            .iter()
            .any(|i| i.semantic.is_some())
    );
    let packed = response::pack_enriched_context(
        &experimental,
        None,
        response::Budget::request(512),
        &response::stdout_bytes,
    )
    .unwrap();
    assert!(packed.tokens <= 512 && packed.text.contains("kiln was fired"));
    // Stale lexical bytes remain ineligible even when they would win placement.
    corpus
        .engine()
        .replace_source("docs/kiln.md", "# Changed\nUnrelated text.\n")
        .unwrap();
    let current = corpus
        .engine()
        .context_candidates_with(
            query,
            Strategy::Search,
            &Corpus::control(),
            &ContextOptions {
                dense: Some(&window),
                experimental_behavior: true,
                ..Default::default()
            },
        )
        .unwrap();
    assert!(
        current
            .batch
            .items
            .iter()
            .all(|i| i.handle.as_ref().is_none_or(|h| h.path != "docs/kiln.md"))
    );
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
    assert_eq!(position, 0, "the nearest dense unit is placed first");
    assert!(answer.packed.tokens <= BUDGET);
}

/// The budgets a sweep tries: wide enough to cross the ladder rungs of one
/// unit (signature, verbatim), coarse enough to stay quick.
fn sweep() -> impl Iterator<Item = usize> {
    (40..=700).step_by(4)
}

/// A dense unit that does not fit verbatim takes its signature form, still
/// tagged: the ordinary unit ladder, no preview and no continuation.
#[test]
fn a_dense_unit_that_does_not_fit_takes_its_signature_form_still_tagged() {
    let corpus = Corpus::new(&[]);
    let slot = corpus.slot();
    let whole = corpus.context(&slot, CODE_QUERY, BUDGET);
    let unit = whole
        .item(CODE_PATH)
        .expect("the retry function")
        .handle
        .clone();
    let mut signatures = 0;
    for tokens in sweep() {
        let Some((packed, parsed)) = pack(&whole.batch, tokens) else {
            continue;
        };
        assert!(packed.tokens <= tokens);
        assert!(!packed.text.contains("\nnext: "), "no continuation");
        let Some(item) = parsed.items.iter().find(|item| item.handle == unit) else {
            continue;
        };
        assert!(item.semantic);
        if item.form.as_deref() == Some("signature") {
            signatures += 1;
            assert!(item.body.contains("pub fn pause_before_next_attempt"));
            let line = packed
                .text
                .lines()
                .find(|line| line.starts_with(&unit))
                .unwrap();
            assert!(
                line.ends_with(" [semantic] fn pause_before_next_attempt [signature]")
                    || line.ends_with(" [semantic] [signature]"),
                "{line}"
            );
        }
    }
    assert!(signatures >= 1, "some budget delivers the signature form");
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

    // 2. Placement: with one result slot the nearest dense unit holds it;
    //    every other unit the dense window carried is dropped by the cut,
    //    not by retrieval or packing.
    let corpus = Corpus::new(&[]);
    let slot = corpus.slot();
    let query = DUSK_QUERY;
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
    let cut = corpus.search(&slot, query, 1);
    assert_eq!(
        cut.facts.paths,
        [DUSK_PATH],
        "the nearest unit holds the slot"
    );
    assert!(cut.facts.truncated);
    let dropped = windows
        .iter()
        .find(|path| path.as_str() != DUSK_PATH)
        .expect("the window holds other units");
    assert_eq!(
        attribute(dropped, &windows, &lexical, &cut.facts, &cut.parsed),
        Owner::Placement
    );

    // 3. Packing: ranked first and selected, then omitted by a budget that
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
        let combined = mcp::context_primary_with_options(
            &None,
            corpus.engine(),
            query,
            Strategy::Search,
            &Control::unbounded(),
            context_foundry::store::ContextOptions::default(),
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
    let index_path = testkit::semantic_file(&directory, index::INDEX_FILE);
    std::fs::remove_file(&index_path).unwrap();
    std::fs::write(&index_path, &buffer).unwrap();
    let hash = context_foundry::digest(&buffer);
    std::fs::rename(&index_path, directory.join(format!("ann-{hash}.usearch"))).unwrap();
    manifest["index_sha256"] = serde_json::Value::String(hash);
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
    let batch = corpus.profile.document_limits().inputs as u64;
    assert!(
        report.document_calls >= report.embedded_units.div_ceil(batch)
            && report.document_calls <= report.embedded_units,
        "{report:?}"
    );
    // Every model input is one card within the card limit, one token per
    // byte: far fewer tokens than the sources' bytes.
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
    assert!(
        corpus.probe.document_tokens()
            <= u64::from(corpus.profile.card_tokens) * report.embedded_units
    );
    assert!(corpus.probe.document_tokens() < source_bytes);
    // The preparation report carries the same input-token total itself.
    assert_eq!(report.input_tokens, corpus.probe.document_tokens());
    let index_file = testkit::semantic_file(
        &index::generation_dir(&corpus.store, &corpus.digest()),
        index::INDEX_FILE,
    );
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
            model_resources: None,
            knowledge: None,
            retrieval_trace_dir: None,
            experimental_behavior_context: false,
            store: corpus.store.clone(),
            root: corpus.root.clone(),
            references: Vec::new(),
            no_memory: false,
            experimental_task_context: false,
            watch: false,
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
    assert!(item.semantic);
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
    assert!(item.semantic, "it came from the dense window");
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
    let target = testkit::semantic_file(&directory, index::INDEX_FILE);
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
    let labels_path = testkit::semantic_file(&directory, index::LABELS_FILE);
    let manifest_path = directory.join(index::MANIFEST_FILE);
    let read = |path: &Path| -> serde_json::Value {
        serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap()
    };
    // A v1 label map: ordinals and keys only, no locations.
    let v1: Vec<_> = read(&labels_path)["labels"]
        .as_array()
        .unwrap()
        .iter()
        .enumerate()
        .map(|(label, row)| serde_json::json!({"label": label, "input_key": row[0]}))
        .collect();
    let labels_bytes = serde_json::to_vec(&serde_json::json!({ "labels": v1 })).unwrap();
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

/// A minimal unsigned bundle around the fake worker and a v2 profile whose
/// artifacts are the empty files the fake descriptor pins.
#[cfg(target_os = "macos")]
fn fake_worker_profile(dir: &Path) -> SemanticProfile {
    use context_foundry::neural::profile::{DEFAULT_BATCH, PROFILE_VERSION, WorkerSpec};
    use context_foundry::neural::worker_runtime::fake_descriptor;
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
    let descriptor = fake_descriptor();
    for file in &descriptor.artifact_files {
        std::fs::write(model_dir.join(&file.name), b"").unwrap();
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
        descriptor,
        query_template: "query: {text}".into(),
        representation: context_foundry::neural::partition::Representation::Address,
        card_tokens: context_foundry::testkit::FIXTURE_CARD_TOKENS,
        batch: DEFAULT_BATCH,
        memory_ceiling_bytes: 3 << 30,
        cpu: Default::default(),
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
    let acquire: Acquire = Box::new(move |_profile, _artifacts, _development, _control| {
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
        self.serve_stdio_watching(semantic, false).await
    }

    async fn serve_stdio_watching(
        &mut self,
        semantic: Option<SemanticServing>,
        watch: bool,
    ) -> Served {
        self.engine = None;
        let (client_io, server_io) = tokio::io::duplex(1 << 20);
        let (input, output) = tokio::io::split(server_io);
        let server = tokio::spawn(mcp::serve_streams(
            ServerOptions {
                model_resources: None,
                knowledge: None,
                retrieval_trace_dir: None,
                experimental_behavior_context: false,
                store: self.store.clone(),
                root: self.root.clone(),
                references: Vec::new(),
                no_memory: false,
                experimental_task_context: false,
                watch,
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
    assert!(baseline.items.iter().all(|item| !item.semantic));

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
        item_of(&partial, DUSK_PATH).is_some_and(|item| item.semantic),
        "semantic evidence from the partial coverage"
    );

    hold.open();
    served.prepare().await;
    let done = served.stopped().await;
    assert_eq!(done["missing_units"], 0, "{done}");
    let ready = served.context(DUSK_QUERY).await;
    assert_eq!(header_word(&ready), Some("ready"));
    let item = item_of(&ready, DUSK_PATH).expect("dense evidence");
    assert!(item.semantic);

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
    // A path-restricted search: the dense units the placement puts first
    // (from the older generation) cannot crowd the edited file's own hit.
    let text = served
        .ok(
            "search",
            serde_json::json!({"query": "kiln", "limit": 10, "path": "docs/schedule.md"}),
        )
        .await;
    assert!(
        item_of(&parse_v2(&text).unwrap(), "docs/schedule.md").is_some(),
        "the edit is served at once"
    );
    // The appended line lies past the card's lines: no card text changed,
    // so preparation re-partitions and re-publishes without embedding.
    served.prepare().await;
    served.stopped().await;
    assert_eq!(corpus.probe.document_calls(), calls, "no card changed");
    assert_eq!(
        header_word(&served.context(DUSK_QUERY).await),
        Some("ready")
    );
    // A heading edit changes that card's text: only its input is embedded.
    let renamed = edited.replacen("# Weekly schedule", "# Weekly kiln schedule", 1);
    corpus.write("docs/schedule.md", &renamed);
    served.index().await;
    served.prepare().await;
    served.stopped().await;
    assert_eq!(corpus.probe.document_calls() - calls, 1);
    assert_eq!(
        corpus.probe.document_inputs() - inputs,
        1,
        "only the edited card"
    );
    assert_eq!(
        header_word(&served.context(DUSK_QUERY).await),
        Some("ready")
    );
    served.close().await;
}

/// A cold priority wave is searchable before the rest of the repository is
/// partitioned. A zero-card tail cannot turn the one prefetch into a full pass.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn progressive_bootstrap_serves_priority_before_the_unprepared_tail_and_resumes() {
    let extra: Vec<_> = (0..80)
        .map(|n| {
            (
                format!("00-background/{n:03}.rs"),
                format!("// Empty source {n}\n"),
            )
        })
        .collect();
    let mut corpus = Corpus::unprepared(&extra);
    let hold = Hold::gated();
    let served = corpus.serve_stdio(Some(corpus.held(&hold))).await;
    for arguments in [
        serde_json::json!({"priority_paths": [DUSK_PATH]}),
        serde_json::json!({"semantic": "pause", "priority_paths": []}),
        serde_json::json!({"semantic": "prepare", "priority_paths": ["../escape"]}),
        serde_json::json!({"semantic": "prepare", "priority_paths": null}),
        serde_json::json!({"semantic": "prepare", "priority_paths": vec!["docs"; 17]}),
    ] {
        let (error, text) = served.raw("index", arguments).await;
        assert!(error && text.contains("invalid_argument"), "{text}");
    }
    served
        .ok(
            "index",
            serde_json::json!({
                "semantic": "prepare", "priority_paths": [DUSK_PATH]
            }),
        )
        .await;
    hold.wait_entered(1).await;
    let active = served
        .until("a fair background wave", |s| {
            s["bootstrap"]["waves"].as_u64().is_some_and(|n| n >= 2)
        })
        .await;
    assert!(
        active["unpartitioned_sources"].as_u64().unwrap() >= 77,
        "{active}"
    );
    assert_eq!(active["bootstrap"]["priority_waves"], 1);
    assert!(item_of(&served.search("parse_record").await, "src/records.rs").is_some());
    let (error, pause) = served.semantic_action("pause").await;
    assert!(!error, "{pause}");
    hold.release();
    // The driver publishes the generation before it snapshots bootstrap
    // progress, so a status read between the two can count the searchable
    // unit before `first_searchable_ms` is recorded. Wait for the snapshot.
    let partial = served
        .until("first coverage", |s| {
            s["searchable_current_units"] == 1 && s["bootstrap"]["first_searchable_ms"].is_u64()
        })
        .await;
    assert!(
        partial["unpartitioned_sources"].as_u64().unwrap() >= 77,
        "{partial}"
    );
    let context = served.context(DUSK_QUERY).await;
    assert_eq!(header_word(&context), Some("partial"));
    assert!(item_of(&context, DUSK_PATH).is_some_and(|item| item.semantic));
    served.close().await;
    corpus.reopen().await;
    assert!(
        corpus
            .engine()
            .semantic_partition(DUSK_PATH)
            .unwrap()
            .is_some()
    );
    assert!(
        corpus
            .engine()
            .semantic_partition("00-background/000.rs")
            .unwrap()
            .is_some()
    );
    assert!(
        corpus
            .engine()
            .semantic_partition("00-background/079.rs")
            .unwrap()
            .is_none()
    );
    assert!(
        corpus
            .engine()
            .semantic_state()
            .unwrap()
            .unwrap()
            .bootstrap
            .unwrap()
            .first_searchable_ms
            .is_some()
    );

    // A new owner recovers accepted vectors without submitting them again.
    let inputs = corpus.probe.document_inputs();
    let resumed = Hold::default();
    let served = corpus.serve_stdio(Some(corpus.held(&resumed))).await;
    served.prepare().await;
    let done = served.stopped().await;
    assert_eq!(done["unpartitioned_sources"], 0, "{done}");
    assert_eq!(done["missing_units"], 0, "{done}");
    assert_eq!(
        corpus.probe.document_inputs() - inputs,
        done["eligible_units"].as_u64().unwrap() - 1
    );
    assert_eq!(
        header_word(&served.context(DUSK_QUERY).await),
        Some("ready")
    );
    let calls = resumed.entered();
    served.prepare().await;
    served.stopped().await;
    assert_eq!(
        resumed.entered(),
        calls,
        "a completed restart is cache-only"
    );
    assert_eq!(resumed.max_in_call(), 1);
    served.close().await;
}

/// A priority file can occur in the background prefetch while its vector is
/// still in flight. It must be embedded once, even across wave boundaries.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn progressive_priority_overlap_does_not_duplicate_in_flight_inputs() {
    let extra: Vec<_> = (0..20)
        .map(|n| {
            (
                format!("00-notes/{n:03}.md"),
                format!("# Note {n}\n\nUnique topic {n}.\n"),
            )
        })
        .collect();
    let mut corpus = Corpus::unprepared(&extra);
    let hold = Hold::gated();
    let served = corpus.serve_stdio(Some(corpus.held(&hold))).await;
    served
        .ok(
            "index",
            serde_json::json!({
                "semantic": "prepare", "priority_paths": ["00-notes/000.md"]
            }),
        )
        .await;
    hold.wait_entered(1).await;
    served
        .until("overlapping background wave", |s| {
            s["bootstrap"]["waves"] == 2
        })
        .await;
    hold.open();
    let done = served.stopped().await;
    assert_eq!(done["missing_units"], 0, "{done}");
    assert_eq!(
        corpus.probe.document_inputs(),
        done["eligible_units"].as_u64().unwrap()
    );
    assert_eq!(hold.max_in_call(), 1);
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
    // The dusk section's one card, keyed exactly as preparation keys it.
    let dusk_key = {
        use context_foundry::neural::partition::{self, CardRecipe};
        let profile = &corpus.profile;
        let tokenizer =
            context_foundry::neural::tokenize::DocumentTokenizer::load(profile).unwrap();
        let digest = corpus.digest();
        let cards = partition::cards(
            &corpus.source(DUSK_PATH),
            DUSK_PATH,
            context_foundry::syntax::Lang::from_path(DUSK_PATH),
            &CardRecipe {
                template: &profile.descriptor.document_template,
                function_digest: &digest,
                card_tokens: profile.card_tokens as usize,
            },
            &tokenizer,
        )
        .unwrap();
        assert_eq!(cards.len(), 1);
        cards[0].input_key.clone()
    };
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
    let audited: serde_json::Value = serde_json::from_str(
        &served
            .ok("status", serde_json::json!({"semantic_audit": true}))
            .await,
    )
    .unwrap();
    assert!(
        audited["semantic"]["cache"]["orphan_entries"]
            .as_u64()
            .unwrap()
            >= 1,
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
    assert_eq!(
        malformed["index"]["validation"], "not_checked",
        "{malformed}"
    );
    let audited: serde_json::Value = serde_json::from_str(
        &served
            .ok("status", serde_json::json!({"semantic_audit": true}))
            .await,
    )
    .unwrap();
    assert_eq!(
        audited["semantic"]["index"]["available"], false,
        "{audited}"
    );
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
    other.model.push_str(" Q8_0");
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

/// Gap 6: semantic items over CRLF, multibyte and fence-like source. Every
/// tagged item at every budget names the lines its range touches; a
/// verbatim one carries the exact bytes of its handle (CR kept) on
/// character boundaries and is fenced longer than any fence-like line in
/// its body, and a literal `next:` body line stays body content.
#[test]
fn semantic_items_keep_crlf_multibyte_and_fence_like_bytes_exact() {
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
    let mut forms: BTreeSet<&'static str> = BTreeSet::new();
    let (mut widest_fence, mut literal_next) = (0, false);
    // Concepts only, and a word only the fence section's card holds.
    for query in ["twilight onset", "wombat"] {
        let whole = corpus.context(&slot, query, BUDGET);
        // This document's candidates only: quick, and no unrelated candidate
        // decides which rung fits.
        let mut batch = whole.batch.clone();
        batch.items.retain(|item| {
            item.handle
                .as_ref()
                .is_some_and(|handle| handle.path == GLYPHS_PATH)
        });
        assert!(
            batch.items.iter().any(|item| item.semantic.is_some()),
            "{query}: a glyph section is dense evidence"
        );
        for tokens in sweep().chain([BUDGET]) {
            let Some((packed, parsed)) = pack(&batch, tokens) else {
                continue;
            };
            assert!(packed.tokens <= tokens);
            for item in parsed.items.iter().filter(|item| item.semantic) {
                let (path, start, end) = span_of(&item.handle);
                assert_eq!(path, GLYPHS_PATH);
                // The lines the range touches, counted by LF.
                assert_eq!(
                    item.lines.as_deref(),
                    Some(format!("L{}-{}", line_of(start), line_of(end - 1)).as_str()),
                    "{}",
                    item.handle
                );
                if item.form.is_some() {
                    forms.insert("signature");
                    continue;
                }
                forms.insert("verbatim");
                // Exact bytes (CR kept) on character boundaries.
                assert!(content.is_char_boundary(start as usize));
                assert!(content.is_char_boundary(end as usize));
                assert_eq!(item.body, content[start as usize..end as usize]);
                // The fence outruns every fence-like body line.
                let longest = item.body.split('\n').map(fence_run).max().unwrap_or(0);
                let fence = fence_after(&packed.text, &item.handle);
                assert_eq!(fence, 3.max(longest + 1), "{}", item.handle);
                widest_fence = widest_fence.max(fence);
                literal_next |= item.body.contains("\r\nnext: docs/glyphs.md#0-1@");
            }
        }
    }
    assert!(forms.contains("verbatim"), "{forms:?}");
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
            model_resources: None,
            knowledge: None,
            retrieval_trace_dir: None,
            experimental_behavior_context: false,
            store: store.to_path_buf(),
            root: root.to_path_buf(),
            references,
            no_memory: false,
            experimental_task_context: false,
            watch: false,
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
    let batch = mcp::context_primary_with_options(
        &slot,
        corpus.engine(),
        QUERY,
        Strategy::Auto,
        &Corpus::control(),
        context_foundry::store::ContextOptions::default(),
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
    assert!(dusk.semantic);
    assert!(
        parsed.items.iter().filter(|item| item.semantic).count() > 1,
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
    assert!(dusk.semantic);
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
        // The secondary's lexical hits follow the primary's placed dense
        // units: a 64-hit search reaches them.
        let mut arguments = op.arguments(QUERY, None);
        if matches!(op, Op::Search) {
            arguments["limit"] = 64.into();
        }
        let text = served.ok(op.tool(), arguments).await;
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
            let semantic = item.semantic || item.label.as_deref() == Some("semantic");
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
        if matches!(op, Op::Search) {
            assert!(
                secondary_items > 0,
                "{op:?}: the secondary's lexical hits:\n{text}"
            );
        }
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

/// Continuous maintenance uses the ordinary MCP owner and durable source seam.
/// Selection work is observed separately from E3's still-full index publication.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn continuous_maintenance_selects_edits_and_reuses_reverts_then_pauses_and_restarts() {
    let extra: Vec<_> = (0..40)
        .map(|n| {
            (
                format!("src/extra{n}.rs"),
                format!("pub fn extra_{n}() {{}}\n"),
            )
        })
        .collect();
    let mut corpus = Corpus::unprepared(&extra);
    let hold = Hold::default();
    let semantic = corpus.held(&hold);
    let served = corpus.serve_stdio(Some(semantic)).await;
    let (error, reply) = served.semantic_action("maintain").await;
    assert!(!error, "{reply}");
    let first = served
        .until("caught up", |s| {
            s["state"] == "waiting" && s["pending"]["paths"] == 0
        })
        .await;
    let selected = first["bootstrap"]["selected_sources"].as_u64().unwrap();
    let base_path = testkit::semantic_file(
        &index::generation_dir(&corpus.store, &corpus.digest()),
        index::INDEX_FILE,
    );
    let base_before = std::fs::metadata(&base_path).unwrap();
    let calls = corpus.probe.document_calls();
    corpus.write("src/extra0.rs", "pub fn changed_entry() {}\n");
    served.index().await;
    let edited = served
        .until("edit maintained", |s| {
            s["state"] == "waiting"
                && s["pending"]["paths"] == 0
                && s["source_revision"] != first["source_revision"]
        })
        .await;
    assert_eq!(
        edited["bootstrap"]["selected_sources"].as_u64().unwrap() - selected,
        1,
        "{edited}"
    );
    assert_eq!(corpus.probe.document_calls(), calls + 1);
    assert_eq!(edited["missing_units"], 0);
    assert_eq!(
        edited["bootstrap"]["generation_builds"],
        first["bootstrap"]["generation_builds"]
    );
    assert!(
        edited["bootstrap"]["delta_publications"].as_u64().unwrap()
            > first["bootstrap"]["delta_publications"].as_u64().unwrap()
    );
    use std::os::unix::fs::MetadataExt;
    assert_eq!(
        std::fs::metadata(&base_path).unwrap().ino(),
        base_before.ino(),
        "an ordinary maintained edit does not replace the base ANN"
    );
    corpus.write("src/extra0.rs", "pub fn extra_0() {}\n");
    served.index().await;
    let reverted = served
        .until("revert maintained", |s| {
            s["state"] == "waiting"
                && s["pending"]["paths"] == 0
                && s["source_revision"] != edited["source_revision"]
        })
        .await;
    assert_eq!(
        corpus.probe.document_calls(),
        calls + 1,
        "exact input reuse on revert"
    );
    std::fs::rename(
        corpus.root.join("src/extra0.rs"),
        corpus.root.join("src/moved.rs"),
    )
    .unwrap();
    served.index().await;
    let renamed = served
        .until("rename maintained", |s| {
            s["state"] == "waiting"
                && s["pending"]["paths"] == 0
                && s["source_revision"] != reverted["source_revision"]
        })
        .await;
    assert_eq!(
        corpus.probe.document_calls(),
        calls + 2,
        "the new address is a new input"
    );
    std::fs::remove_file(corpus.root.join("src/moved.rs")).unwrap();
    served.index().await;
    let deleted = served
        .until("delete maintained", |s| {
            s["state"] == "waiting"
                && s["pending"]["paths"] == 0
                && s["source_revision"] != renamed["source_revision"]
        })
        .await;
    assert_eq!(corpus.probe.document_calls(), calls + 2);
    assert_eq!(
        deleted["sources"].as_u64().unwrap() + 1,
        first["sources"].as_u64().unwrap()
    );
    assert!(!served.semantic_action("pause").await.0);
    served.until("paused", |s| s["state"] == "paused").await;
    corpus.write("src/extra1.rs", "pub fn paused_edit() {}\n");
    served.index().await;
    let pending = served.semantic().await;
    assert_eq!(pending["pending"]["paths"], 1);
    assert_eq!(pending["coverage_observation"], "stale");
    assert!(pending["cached_current_units"].is_null());
    served.close().await;
    // Restart without the explicit mode leaves work pending and starts no documents.
    let semantic = corpus.held(&hold);
    let served = corpus.serve_stdio(Some(semantic)).await;
    assert_eq!(served.semantic().await["pending"]["paths"], 1);
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert_eq!(corpus.probe.document_calls(), calls + 2);
    served.close().await;
    let mut semantic = corpus.held(&hold);
    semantic.maintain = true;
    let served = corpus.serve_stdio(Some(semantic)).await;
    let resumed = served
        .until("explicit startup catches up", |s| {
            s["state"] == "waiting" && s["pending"]["paths"] == 0
        })
        .await;
    assert_eq!(resumed["bootstrap"]["selected_sources"], 1, "{resumed}");
    assert_eq!(corpus.probe.document_calls(), calls + 3);
    // Switching a caught-up continuous owner to one-shot must execute the
    // requested full reconciliation, then stop; it must not just disable wakeups.
    served.prepare().await;
    let once = served.stopped().await;
    assert!(once["bootstrap"]["selected_sources"].as_u64().unwrap() > 1);
    assert_eq!(corpus.probe.document_calls(), calls + 3);
    served.close().await;
    corpus.reopen().await;
    assert!(
        corpus
            .engine()
            .derivation_pending(context_foundry::neural::work::Lane::Knowledge)
            .unwrap()
            .paths
            > 0
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn watching_wakes_continuous_semantics_and_status_does_not_shrink_batches() {
    let extra: Vec<_> = (0..18)
        .map(|n| {
            (
                format!("src/batch{n}.rs"),
                format!("pub fn batch_{n}() {{}}\n"),
            )
        })
        .collect();
    let mut corpus = Corpus::unprepared(&extra);
    let hold = Hold::gated();
    let mut semantic = corpus.held(&hold);
    semantic.maintain = true;
    let served = corpus.serve_stdio_watching(Some(semantic), true).await;
    hold.wait_entered(1).await;
    for _ in 0..4 {
        served.status().await;
    }
    hold.release();
    hold.wait_entered(2).await;
    assert!(
        hold.batch(2) > 2,
        "progress polling imposed the foreground throttle"
    );
    hold.open();
    hold.release();
    let first = served
        .until("watched bootstrap", |s| {
            s["state"] == "waiting" && s["pending"]["paths"] == 0
        })
        .await;
    corpus.write("src/watched.rs", "pub fn automatically_observed() {}\n");
    let current = served
        .until("watch-triggered semantic work", |s| {
            s["state"] == "waiting"
                && s["pending"]["paths"] == 0
                && s["source_revision"] != first["source_revision"]
        })
        .await;
    assert_eq!(current["missing_units"], 0);
    assert_eq!(
        current["bootstrap"]["selected_sources"].as_u64().unwrap(),
        first["bootstrap"]["selected_sources"].as_u64().unwrap() + 1
    );
    served.close().await;
}

#[test]
fn ordinary_semantic_status_is_a_cheap_observation_with_explicit_stale_and_audit() {
    let corpus = Corpus::new(&[]);
    let engine = corpus.engine();
    for name in [
        context_foundry::neural::fault_names::PARTITION_READ,
        context_foundry::neural::fault_names::CACHE_RECONCILE,
        context_foundry::neural::status::fault_names::BEFORE_VALIDATION,
    ] {
        fault::arm(
            name,
            1,
            Action::Fail("ordinary status must not perform a census".into()),
        );
    }
    for _ in 0..20 {
        let value = engine.semantic_progress(&Control::unbounded()).unwrap();
        assert_eq!(value["coverage_observation"], "current");
        assert_eq!(value["missing_units"], 0);
        assert_eq!(value["index"]["validation"], "not_checked");
        assert!(value["index"]["available"].is_null());
    }
    assert!(
        engine.semantic_status(&Control::unbounded()).is_err(),
        "the explicit audit still validates"
    );
    fault::disarm_all();
    engine
        .replace_source("src/new.rs", "fn new_source() {}\n")
        .unwrap();
    let stale = engine.semantic_progress(&Control::unbounded()).unwrap();
    assert_eq!(stale["coverage_observation"], "stale");
    assert!(stale["cached_current_units"].is_null());
    assert!(
        stale["last_observed"]["cached_current_units"]
            .as_u64()
            .unwrap()
            > 0
    );
    assert_eq!(stale["index"]["validation"], "not_checked");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn continuous_edit_during_inference_and_pause_keep_the_new_version_pending() {
    let mut corpus = Corpus::unprepared(&[]);
    let hold = Hold::gated();
    let mut semantic = corpus.held(&hold);
    semantic.maintain = true;
    let served = corpus.serve_stdio(Some(semantic)).await;
    hold.wait_entered(1).await;
    corpus.write(
        DUSK_PATH,
        "# Changed while embedding\n\nTwilight starts here.\n",
    );
    served.index().await;
    assert!(!served.semantic_action("pause").await.0);
    hold.release();
    served
        .until("paused after its in-flight call", |s| {
            s["state"] == "paused" && s["provider"]["state"] == "ready"
        })
        .await;
    assert!(
        served.semantic().await["pending"]["reconcile_required"]
            .as_bool()
            .unwrap()
    );
    hold.open();
    assert!(!served.semantic_action("maintain").await.0);
    served
        .until("new version caught up", |s| {
            s["state"] == "waiting" && s["pending"]["paths"] == 0
        })
        .await;
    served.close().await;
    corpus.reopen().await;
    let engine = corpus.engine();
    assert_eq!(
        engine
            .semantic_partition(DUSK_PATH)
            .unwrap()
            .unwrap()
            .source_hash,
        engine.source(DUSK_PATH).unwrap().unwrap().hash
    );
    assert_eq!(
        engine
            .semantic_status(&Control::unbounded())
            .unwrap()
            .missing_units,
        0
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn continuous_semantics_progresses_during_filesystem_traffic_and_converges() {
    let mut corpus = Corpus::unprepared(&[]);
    let hold = Hold::default();
    let mut semantic = corpus.held(&hold);
    semantic.maintain = true;
    let served = corpus.serve_stdio_watching(Some(semantic), true).await;
    served
        .until("initial continuous coverage", |s| s["state"] == "waiting")
        .await;
    let calls = corpus.probe.document_calls();
    let stop = Arc::new(AtomicBool::new(false));
    // Keep one genuinely pending version stable while other paths churn.
    // Continuously replacing every candidate can legitimately reject every
    // stale partition before inference; that is not a scheduler starvation test.
    std::fs::write(
        corpus.root.join("src/settled.rs"),
        "pub fn settled_during_traffic() {}\n",
    )
    .unwrap();
    let writer = std::thread::spawn({
        let stop = Arc::clone(&stop);
        let root = corpus.root.clone();
        move || {
            let end = Instant::now() + Duration::from_secs(15);
            let mut n = 0;
            while !stop.load(Ordering::SeqCst) && Instant::now() < end {
                std::fs::write(
                    root.join(format!("src/stream{}.rs", n % 2)),
                    format!("pub fn streamed_{n}() {{}}\n"),
                )
                .unwrap();
                n += 1;
                std::thread::sleep(Duration::from_millis(20));
            }
            n
        }
    });
    let deadline = Instant::now() + Duration::from_secs(12);
    while corpus.probe.document_calls() == calls {
        assert!(
            Instant::now() < deadline,
            "no semantic progress during source traffic"
        );
        assert!(!writer.is_finished());
        served.status().await;
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    assert!(
        !writer.is_finished(),
        "progress must precede the end of source traffic"
    );
    assert!(
        !served.search("parse_record").await.items.is_empty(),
        "foreground source retrieval remains serviceable"
    );
    stop.store(true, Ordering::SeqCst);
    assert!(writer.join().unwrap() > 1);
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        let status = served.status().await;
        if status["watch"]["phase"] == "idle"
            && status["watch"]["pending_paths"] == 0
            && status["semantic"]["state"] == "waiting"
            && status["semantic"]["pending"]["paths"] == 0
        {
            break;
        }
        assert!(Instant::now() < deadline, "did not converge: {status}");
        tokio::time::sleep(Duration::from_millis(30)).await;
    }
    served.close().await;
    corpus.reopen().await;
    for path in ["src/stream0.rs", "src/stream1.rs", "src/settled.rs"] {
        let hash = context_foundry::digest(&std::fs::read(corpus.root.join(path)).unwrap());
        assert_eq!(corpus.engine().source(path).unwrap().unwrap().hash, hash);
        assert_eq!(
            corpus
                .engine()
                .semantic_partition(path)
                .unwrap()
                .unwrap()
                .source_hash,
            hash
        );
    }
}

/// Advance one source through the genuine renderer/cache acceptance without
/// reopening the owner. The vector function is the same deterministic fixture
/// used by ConceptProvider; these checks establish work selection, not quality.
fn accept_fixture_edit(corpus: &Corpus, path: &str, body: &str) {
    use context_foundry::neural::{cache, partition, tokenize::DocumentTokenizer};
    let engine = corpus.engine();
    engine.replace_source(path, body).unwrap();
    let profile = &corpus.profile;
    let digest = corpus.digest();
    let tokenizer = DocumentTokenizer::load(profile).unwrap();
    let cards = partition::cards(
        body,
        path,
        context_foundry::syntax::Lang::from_path(path),
        &partition::CardRecipe {
            template: &profile.descriptor.document_template,
            function_digest: &digest,
            card_tokens: profile.card_tokens as usize,
        },
        &tokenizer,
    )
    .unwrap();
    let units: Vec<_> = cards
        .iter()
        .map(|card| cache::PartitionUnit {
            kind: card.kind,
            start: card.start,
            end: card.end,
            input_key: card.input_key.clone(),
        })
        .collect();
    engine
        .semantic_record_partition(
            path,
            &engine.source(path).unwrap().unwrap().hash,
            &partition::recipe_id(&profile.descriptor.tokenizer, profile.card_tokens),
            &digest,
            Some(&units),
            &|_| Ok(units.clone()),
        )
        .unwrap();
    let entries: Vec<_> = cards
        .iter()
        .filter(|card| {
            engine
                .semantic_cache_probe(&card.input_key, &digest)
                .unwrap()
                != cache::CacheProbe::Current
        })
        .map(|card| {
            (
                card.input_key.clone(),
                concept_vector(&model_input(&card.ids)),
            )
        })
        .collect();
    engine
        .semantic_cache_commit(&entries, &digest, DIMENSIONS, DEFAULT_CACHE_CAP_BYTES)
        .unwrap();
}

#[test]
fn a_v4_generation_republishes_immutable_metadata_from_cache_without_inference() {
    let mut corpus = Corpus::new(&[]);
    corpus
        .engine()
        .semantic_rebuild_index(&Control::unbounded())
        .unwrap();
    let directory = index::generation_dir(&corpus.store, &corpus.digest());
    let path = directory.join(index::MANIFEST_FILE);
    let bytes = std::fs::read(&path).unwrap();
    let current: index::GenerationManifest = serde_json::from_slice(&bytes).unwrap();
    let mut prior: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    prior["v"] = 4.into();
    let keys: serde_json::Value =
        serde_json::from_slice(&std::fs::read(directory.join(current.members_file())).unwrap())
            .unwrap();
    prior["base_entries"] = keys.clone();
    prior["entries"] = keys;
    for name in [
        "base_count",
        "members_sha256",
        "locations_delta",
        "mapping_revision",
        "coverage_stats",
    ] {
        prior.as_object_mut().unwrap().remove(name);
    }
    std::fs::write(&path, serde_json::to_vec(&prior).unwrap()).unwrap();
    let calls = corpus.probe.document_calls();
    let report = corpus.prepare();
    assert!(report.index_published && !report.partial, "{report:?}");
    assert_eq!(report.document_calls, 0);
    assert_eq!(corpus.probe.document_calls(), calls);
    let current: index::GenerationManifest =
        serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    assert_eq!(current.v, index::GENERATION_VERSION);
    assert!(current.mapping_revision.is_some());
    assert!(directory.join(current.members_file()).exists());
    let slot = corpus.slot();
    assert_eq!(
        corpus.context(&slot, DUSK_QUERY, BUDGET).word(),
        Some("ready")
    );
}

#[test]
fn clean_restart_and_changed_inputs_reuse_the_durable_mapping_lookup() {
    use context_foundry::neural::fault_names::{
        MAPPING_CACHE_PROBE, MAPPING_FULL_PAGE, METADATA_READ, PARTITION_READ,
    };
    let mut corpus = Corpus::new(&[]);
    let control = Control::unbounded();
    corpus.engine().semantic_publish_pending(&control).unwrap();
    let calls = corpus.probe.document_calls.load(Ordering::SeqCst);
    corpus.engine = None;
    corpus.engine = Some(Engine::open_existing(&corpus.store).unwrap());
    fault::disarm_all();
    assert!(matches!(
        corpus.engine().semantic_publish_pending(&control).unwrap(),
        index::Publication::Current(_)
    ));
    assert!(
        fault::reached(METADATA_READ) >= 2,
        "cold owner still verifies artifacts"
    );
    for point in [MAPPING_FULL_PAGE, MAPPING_CACHE_PROBE, PARTITION_READ] {
        assert_eq!(fault::reached(point), 0, "clean restart: {point}");
    }
    let reads = fault::reached(METADATA_READ);
    corpus.engine().semantic_publish_pending(&control).unwrap();
    assert_eq!(fault::reached(METADATA_READ), reads);
    // Partition changes at the same source revision must invalidate reuse too.
    let engine = corpus.engine();
    let revision = engine.source_revision().unwrap();
    let state = engine.semantic_state().unwrap().unwrap();
    engine
        .semantic_record_partition(
            CODE_PATH,
            &engine.source(CODE_PATH).unwrap().unwrap().hash,
            state.recipe_id.as_deref().unwrap(),
            state.function_digest.as_deref().unwrap(),
            Some(&[]),
            &|_| Ok(vec![]),
        )
        .unwrap();
    assert_eq!(engine.source_revision().unwrap(), revision);
    let changed = engine.semantic_publish_pending(&control).unwrap();
    assert!(!matches!(changed, index::Publication::Current(_)));
    assert_eq!(
        fault::reached(MAPPING_FULL_PAGE),
        0,
        "one partition change resumes the lookup"
    );
    assert_eq!(fault::reached(PARTITION_READ), 1);
    // A durable source change made before closing cannot borrow an older fence.
    engine
        .replace_source(DUSK_PATH, "# Offline change\n")
        .unwrap();
    corpus.engine = None;
    corpus.engine = Some(Engine::open_existing(&corpus.store).unwrap());
    fault::disarm_all();
    assert!(!matches!(
        corpus.engine().semantic_publish_pending(&control).unwrap(),
        index::Publication::Current(_)
    ));
    assert_eq!(fault::reached(MAPPING_FULL_PAGE), 0);
    assert_eq!(fault::reached(PARTITION_READ), 1);
    assert_eq!(corpus.probe.document_calls.load(Ordering::SeqCst), calls);
}

#[test]
fn checkpoints_never_share_a_key_with_the_earlier_byte_accounting() {
    use context_foundry::neural::fault_names::MAPPING_FULL_PAGE;
    use redb::{ReadableDatabase, TableDefinition};
    // Binaries before 2026-10-17 restore this key and sum view bytes with
    // worst-case escapes. Restoring an exact-accounting checkpoint, they would
    // subtract larger rows than were added and report a corrupt store.
    const LEGACY: &str = "semantic_mapping_checkpoint";
    const CURRENT: &str = "semantic_mapping_checkpoint_exact";
    let meta = TableDefinition::<&str, &str>::new("meta");
    let mut corpus = Corpus::new(&[]);
    let control = Control::unbounded();
    corpus.engine().semantic_publish_pending(&control).unwrap();
    corpus.engine = None;
    let db = redb::Database::open(corpus.store.join("knowledge.redb")).unwrap();
    {
        let tx = db.begin_read().unwrap();
        let table = tx.open_table(meta).unwrap();
        assert!(table.get(CURRENT).unwrap().is_some());
        assert!(
            table.get(LEGACY).unwrap().is_none(),
            "an earlier binary finds none"
        );
    }
    // A store an earlier binary checkpointed last holds only the legacy key.
    let tx = db.begin_write().unwrap();
    {
        let mut table = tx.open_table(meta).unwrap();
        let checkpoint = table.remove(CURRENT).unwrap().unwrap().value().to_owned();
        table.insert(LEGACY, checkpoint.as_str()).unwrap();
    }
    tx.commit().unwrap();
    drop(db);
    corpus.engine = Some(Engine::open_existing(&corpus.store).unwrap());
    fault::disarm_all();
    let engine = corpus.engine();
    let state = engine.semantic_state().unwrap().unwrap();
    engine
        .semantic_record_partition(
            CODE_PATH,
            &engine.source(CODE_PATH).unwrap().unwrap().hash,
            state.recipe_id.as_deref().unwrap(),
            state.function_digest.as_deref().unwrap(),
            Some(&[]),
            &|_| Ok(vec![]),
        )
        .unwrap();
    engine.semantic_publish_pending(&control).unwrap();
    assert!(
        fault::reached(MAPPING_FULL_PAGE) > 0,
        "a legacy checkpoint is rebuilt from authority, never restored"
    );
    corpus.engine = None;
    let db = redb::Database::open(corpus.store.join("knowledge.redb")).unwrap();
    let tx = db.begin_read().unwrap();
    let table = tx.open_table(meta).unwrap();
    assert!(table.get(CURRENT).unwrap().is_some());
    assert!(table.get(LEGACY).unwrap().is_none());
}

#[test]
fn location_publication_shares_unchanged_metadata_and_failed_edits_remain_retryable() {
    use context_foundry::neural::fault_names::{
        LOCATION_COMPARE, LOCATION_WRITE, MAPPING_FULL_PAGE, METADATA_READ, PUBLISH_BEFORE_MANIFEST,
    };
    let corpus = Corpus::new(&[]);
    let engine = corpus.engine();
    let control = Control::unbounded();
    engine.semantic_rebuild_index(&control).unwrap();
    engine.semantic_publish_pending(&control).unwrap();
    let runtime = corpus.runtime();
    let window = || {
        runtime
            .window(
                engine,
                DUSK_QUERY,
                Instant::now() + Duration::from_secs(30),
                &control,
            )
            .unwrap()
    };
    window();
    let original = corpus.source(CODE_PATH);
    accept_fixture_edit(&corpus, CODE_PATH, &format!("\n{original}"));
    fault::disarm_all();
    fault::arm(
        PUBLISH_BEFORE_MANIFEST,
        0,
        Action::Fail("location publication failed".into()),
    );
    assert!(engine.semantic_publish_pending(&control).is_err());
    fault::disarm_all();
    assert!(matches!(
        engine.semantic_publish_pending(&control).unwrap(),
        index::Publication::Mapped(_)
    ));
    assert_eq!(
        fault::reached(MAPPING_FULL_PAGE),
        0,
        "retry retains the warm mapping view"
    );
    assert_eq!(
        fault::reached(LOCATION_WRITE),
        1,
        "only the bounded overlay is written"
    );
    let compares = fault::reached(LOCATION_COMPARE);
    assert!(compares > 0);
    let reads = fault::reached(METADATA_READ);
    window();
    assert_eq!(
        fault::reached(METADATA_READ) - reads,
        1,
        "only the new overlay is read/hashed/decoded"
    );
    let reads = fault::reached(METADATA_READ);
    // A zero-card source changes coverage, with no vector or location changes.
    accept_fixture_edit(&corpus, "empty.rs", "\n");
    assert!(matches!(
        engine.semantic_publish_pending(&control).unwrap(),
        index::Publication::Mapped(_)
    ));
    window();
    assert_eq!(fault::reached(LOCATION_COMPARE), compares);
    assert_eq!(fault::reached(LOCATION_WRITE), 1);
    assert_eq!(fault::reached(METADATA_READ), reads);
    // The receipt must not bless a same-length edit of a retained metadata file.
    let directory = index::generation_dir(&corpus.store, &corpus.profile.descriptor.digest());
    let manifest: index::GenerationManifest =
        serde_json::from_slice(&std::fs::read(directory.join(index::MANIFEST_FILE)).unwrap())
            .unwrap();
    let path = directory.join(manifest.location_delta_file().unwrap());
    let mut bytes = std::fs::read(&path).unwrap();
    bytes[0] ^= 1;
    std::fs::write(&path, bytes).unwrap();
    runtime.forget_index();
    let error = runtime
        .window(
            engine,
            DUSK_QUERY,
            Instant::now() + Duration::from_secs(30),
            &control,
        )
        .expect_err("changed metadata refused");
    assert!(error.detail.contains("manifest hash"), "{error:?}");
}

#[test]
fn warm_publications_reuse_verification_and_resident_segments_with_fresh_locations() {
    use context_foundry::neural::fault_names::{
        MAPPING_FULL_PAGE, PARTITION_READ, SEGMENT_HASH, SEGMENT_RESTORE,
    };
    let corpus = Corpus::new(&[]);
    let engine = corpus.engine();
    let control = Control::unbounded();
    // Begin with a compacted base: progressive preparation may leave a delta.
    engine.semantic_rebuild_index(&control).unwrap();
    // First publication warms the mapping and receipts without loading USearch.
    engine.semantic_publish_pending(&control).unwrap();
    let runtime = corpus.runtime();
    let window = || {
        runtime
            .window(
                engine,
                DUSK_QUERY,
                Instant::now() + Duration::from_secs(30),
                &control,
            )
            .unwrap()
    };
    let hash = fault::reached(SEGMENT_HASH);
    let initial = window();
    let initial_revision = engine.source_revision().unwrap();
    assert_eq!(
        fault::reached(SEGMENT_HASH),
        hash,
        "publisher verification reused by serving"
    );
    let restores = fault::reached(SEGMENT_RESTORE);
    let pages = fault::reached(MAPPING_FULL_PAGE);
    let original = corpus.source(CODE_PATH);
    accept_fixture_edit(&corpus, CODE_PATH, &format!("\n\n{original}"));
    let reads = fault::reached(PARTITION_READ);
    assert!(matches!(
        engine.semantic_publish_pending(&control).unwrap(),
        index::Publication::Mapped(_)
    ));
    assert_eq!(fault::reached(PARTITION_READ) - reads, 1);
    // Deliberately omit forget_index: the manifest receipt must notice publication.
    let mapped = window();
    assert_eq!(
        mapped.complete_at(),
        Some(engine.source_revision().unwrap())
    );
    assert_ne!(initial.complete_at(), mapped.complete_at());
    assert_eq!(
        fault::reached(SEGMENT_RESTORE),
        restores,
        "same immutable ANN"
    );
    assert_eq!(
        fault::reached(SEGMENT_HASH),
        hash,
        "unchanged inode/version"
    );
    accept_fixture_edit(
        &corpus,
        CODE_PATH,
        &format!("{original}\npub fn novel_delta_card() {{}}\n"),
    );
    assert!(matches!(
        engine.semantic_publish_pending(&control).unwrap(),
        index::Publication::Delta(_)
    ));
    let delta = window();
    assert_eq!(delta.complete_at(), Some(engine.source_revision().unwrap()));
    assert_eq!(
        fault::reached(SEGMENT_RESTORE),
        restores + 1,
        "only new delta restored"
    );
    assert_eq!(
        fault::reached(SEGMENT_HASH),
        hash + 1,
        "only new delta verified"
    );
    runtime.forget_index();
    window();
    assert_eq!(
        fault::reached(SEGMENT_RESTORE),
        restores + 1,
        "explicit refresh also keeps base and delta"
    );
    accept_fixture_edit(&corpus, CODE_PATH, &original);
    assert!(matches!(
        engine.semantic_publish_pending(&control).unwrap(),
        index::Publication::Delta(_)
    ));
    let reverted = window();
    assert_eq!(
        reverted.complete_at(),
        Some(engine.source_revision().unwrap())
    );
    assert_eq!(
        fault::reached(SEGMENT_RESTORE),
        restores + 1,
        "reversion reuses the base"
    );
    assert_eq!(fault::reached(MAPPING_FULL_PAGE), pages);
    assert_eq!(
        initial.complete_at(),
        Some(initial_revision),
        "old requests retain their own mapping"
    );
    // Exceed the bounded overlay: a compacted base must restore exactly once.
    let mut compacted_body = original.clone();
    for i in 0..65 {
        compacted_body.push_str(&format!("\npub fn compaction_card_{i}() {{}}\n"));
    }
    accept_fixture_edit(&corpus, CODE_PATH, &compacted_body);
    assert!(matches!(
        engine.semantic_publish_pending(&control).unwrap(),
        index::Publication::Rebuilt(_)
    ));
    let compacted = window();
    assert_eq!(
        compacted.complete_at(),
        Some(engine.source_revision().unwrap())
    );
    assert_eq!(
        fault::reached(SEGMENT_RESTORE),
        restores + 2,
        "new compacted base restored once"
    );
    runtime.forget_index();
    window();
    assert_eq!(fault::reached(SEGMENT_RESTORE), restores + 2);
    assert_eq!(fault::reached(MAPPING_FULL_PAGE), pages);
    // Audits intentionally rehash even when warm serving already verified the file.
    let audit_hash = fault::reached(SEGMENT_HASH);
    engine.semantic_status(&control).unwrap();
    assert!(fault::reached(SEGMENT_HASH) > audit_hash);
}

#[test]
fn resident_generation_rechecks_owner_profile_and_manifest_without_invalidation_hints() {
    use context_foundry::neural::fault_names::SEGMENT_RESTORE;
    let mut corpus = Corpus::new(&[]);
    let runtime = corpus.runtime();
    let window = |corpus: &Corpus| {
        runtime.window(
            corpus.engine(),
            DUSK_QUERY,
            Instant::now() + Duration::from_secs(30),
            &Control::unbounded(),
        )
    };
    let before = fault::reached(SEGMENT_RESTORE);
    window(&corpus).unwrap();
    let restores = fault::reached(SEGMENT_RESTORE);
    let segment_count = restores - before;
    corpus.engine = None;
    corpus.engine = Some(Engine::open_existing(&corpus.store).unwrap());
    window(&corpus).unwrap();
    assert_eq!(
        fault::reached(SEGMENT_RESTORE),
        restores + segment_count,
        "new owner cannot reuse old owner's cache"
    );
    let path = index::generation_dir(&corpus.store, &corpus.digest()).join(index::MANIFEST_FILE);
    let original = std::fs::read(&path).unwrap();
    let mut manifest: serde_json::Value = serde_json::from_slice(&original).unwrap();
    manifest["v"] = 99.into();
    std::fs::write(&path, serde_json::to_vec(&manifest).unwrap()).unwrap();
    assert_eq!(window(&corpus).unwrap_err().code, "index_unavailable");
    std::fs::write(&path, original).unwrap();
    window(&corpus).unwrap();
    let mut state = corpus.engine().semantic_state().unwrap().unwrap();
    state.function_digest = Some("cd".repeat(32));
    corpus.engine().semantic_set_state(&state).unwrap();
    assert_eq!(window(&corpus).unwrap_err().code, "profile_mismatch");
}

#[test]
fn segment_receipts_refuse_in_place_replacement_and_symlink_changes_on_refresh() {
    use context_foundry::neural::fault_names::{SEGMENT_HASH, SEGMENT_RESTORE};
    for mode in ["in-place", "replacement", "symlink"] {
        let corpus = Corpus::new(&[]);
        let runtime = corpus.runtime();
        let window = || {
            runtime.window(
                corpus.engine(),
                DUSK_QUERY,
                Instant::now() + Duration::from_secs(30),
                &Control::unbounded(),
            )
        };
        window().unwrap();
        let path = testkit::semantic_file(
            &index::generation_dir(&corpus.store, &corpus.digest()),
            index::INDEX_FILE,
        );
        let mut bytes = std::fs::read(&path).unwrap();
        let last = bytes.len() - 1;
        bytes[last] ^= 1;
        if mode == "replacement" {
            std::fs::remove_file(&path).unwrap();
        }
        if mode == "symlink" {
            let outside = corpus.dir.path().join("foreign-ann");
            std::fs::write(&outside, bytes).unwrap();
            std::fs::remove_file(&path).unwrap();
            std::os::unix::fs::symlink(outside, path).unwrap();
        } else {
            std::fs::write(path, bytes).unwrap();
        }
        let hashes = fault::reached(SEGMENT_HASH);
        let restores = fault::reached(SEGMENT_RESTORE);
        runtime.forget_index();
        assert_eq!(window().unwrap_err().code, "index_unavailable", "{mode}");
        assert_eq!(
            fault::reached(SEGMENT_RESTORE),
            restores,
            "corrupt bytes never restored"
        );
        if mode != "symlink" {
            assert_eq!(fault::reached(SEGMENT_HASH), hashes + 1);
        }
    }
}

fn select_body_recipe(
    corpus: &mut Corpus,
    tokens: u32,
    parent_tokens: u32,
    geometry: Option<provider::InputGeometry>,
) {
    let profile = Arc::make_mut(&mut corpus.profile);
    profile.representation =
        context_foundry::neural::partition::Representation::Body { parent_tokens };
    profile.card_tokens = tokens;
    profile.descriptor.geometry = geometry;
    if geometry.is_some() {
        profile.descriptor.adapter_revision = provider::GEOMETRY_ADAPTER_REVISION;
    }
    profile.validate().unwrap();
    std::fs::write(&corpus.profile_path, serde_json::to_vec(&*profile).unwrap()).unwrap();
}

#[test]
fn body_recipe_migration_edits_rename_and_revert_reuse_exact_inputs_and_deliver_child_evidence() {
    let path = "src/opaque.rs";
    let body = "impl Billing {\n    fn xyz() {\n        // reimbursement repayment credit refund cancel\n        transfer(400);\n    }\n}\n";
    let mut corpus = Corpus::new(&[(path.into(), body.into())]);
    let old_profile = corpus.profile.clone();
    let old_key = corpus
        .engine()
        .semantic_partition(path)
        .unwrap()
        .unwrap()
        .units[0]
        .input_key
        .clone();
    select_body_recipe(&mut corpus, 512, 64, None);
    let report = corpus.prepare();
    assert!(!report.partial && report.document_calls > 0, "{report:?}");
    assert_eq!(
        old_profile.descriptor.digest(),
        corpus.digest(),
        "same embedding function, distinct recipe/input text"
    );
    assert!(matches!(
        corpus
            .engine()
            .semantic_cache_lookup(&old_key, &corpus.digest(), DIMENSIONS)
            .unwrap(),
        context_foundry::neural::cache::CacheLookup::Hit(_)
    ));
    let first = corpus.engine().semantic_partition(path).unwrap().unwrap();
    assert!(first.recipe_id.starts_with("body-v1"));
    let slot = corpus.slot();
    let answer = corpus.context(&slot, LEDGER_QUERY, 4096);
    assert_eq!(answer.word(), Some("ready"));
    assert!(
        answer.item(path).is_some(),
        "body-only behavior reaches final context: {}",
        answer.packed.text
    );
    let runtime = corpus.runtime();
    let window = runtime
        .window(
            corpus.engine(),
            LEDGER_QUERY,
            Instant::now() + Duration::from_secs(30),
            &Control::unbounded(),
        )
        .unwrap();
    let target = window
        .hits
        .iter()
        .flat_map(|hit| window.units(hit))
        .find(|unit| {
            &*unit.path == path
                && body[unit.start as usize..unit.end as usize].starts_with("fn xyz")
        })
        .unwrap();
    assert_eq!(
        &body[target.start as usize..target.end as usize],
        "fn xyz() {\n        // reimbursement repayment credit refund cancel\n        transfer(400);\n    }"
    );
    drop(runtime);
    drop(slot);
    assert_eq!(corpus.prepare().document_calls, 0, "unchanged restart");
    let edited = body.replace("transfer(400)", "transfer(800)");
    std::fs::write(corpus.root.join(path), &edited).unwrap();
    corpus.engine().replace_source(path, &edited).unwrap();
    assert_eq!(
        corpus.prepare().embedded_units,
        1,
        "only the changed child input"
    );
    let renamed = "src/moved.rs";
    std::fs::rename(corpus.root.join(path), corpus.root.join(renamed)).unwrap();
    corpus.engine().delete_source(path).unwrap();
    corpus.engine().replace_source(renamed, &edited).unwrap();
    assert_eq!(
        corpus.prepare().document_calls,
        0,
        "path is provenance, not body input"
    );
    let parent_edit = edited.replace("impl Billing", "impl Payment");
    corpus
        .engine()
        .replace_source(renamed, &parent_edit)
        .unwrap();
    let changed = corpus.prepare();
    assert!(
        changed.embedded_units >= 2,
        "included parent context invalidates its child: {changed:?}"
    );
    corpus.engine().delete_source(renamed).unwrap();
    corpus.engine().replace_source(path, body).unwrap();
    assert_eq!(
        corpus.prepare().document_calls,
        0,
        "body/context revert reuses retained vectors"
    );
    corpus.profile = old_profile;
    std::fs::write(
        &corpus.profile_path,
        serde_json::to_vec(&*corpus.profile).unwrap(),
    )
    .unwrap();
    assert_eq!(
        corpus.prepare().document_calls,
        0,
        "switching back preserves address caches"
    );
}

#[test]
fn hierarchy_area_preparation_deduplicates_exact_inputs_and_reuses_on_restart() {
    use context_foundry::{brief, neural::partition::Representation};
    let a = "fn refund() { credit(4); }\n";
    let b = "fn check() { refund(); }\n";
    let mut corpus = Corpus::new(&[("a.rs".into(), a.into()), ("b.rs".into(), b.into())]);
    select_body_recipe(&mut corpus, 512, 64, None);
    Arc::make_mut(&mut corpus.profile).representation =
        Representation::Hierarchy { parent_tokens: 64 };
    std::fs::write(
        &corpus.profile_path,
        serde_json::to_vec(&*corpus.profile).unwrap(),
    )
    .unwrap();
    let wid = corpus.engine().workspace_id().unwrap();
    let handle = |path: &str, body: &str| {
        format!(
            "{path}#0-{}@{}.{}",
            body.len(),
            &context_foundry::digest(body.as_bytes())[..32],
            &wid[..16]
        )
    };
    for spans in [
        [handle("a.rs", a), handle("b.rs", b)],
        [handle("b.rs", b), handle("a.rs", a)],
    ] {
        let input = serde_json::json!({"cited_spans":spans,"recipe_sha256":"a".repeat(64),"model":"fixture/derived",
            "artifact_sha256":"b".repeat(64),"prompt_sha256":"c".repeat(64),"generated_at_unix_ms":1,
            "purpose":[{"text":"Refund reimbursement","citations":[0],"identifiers":[]}],
            "behavior":[{"text":"Credit an account.","citations":[0],"identifiers":[]}],
            "provides":[],"consumes":[],"constraints":[],"tests":[],"unknowns":[]});
        let request = brief::parse(
            &serde_json::to_vec(
                &serde_json::json!({"workspace_id":wid,"command":{"op":"put","brief":input}}),
            )
            .unwrap(),
        )
        .unwrap();
        corpus
            .engine()
            .brief(&request, &Control::unbounded())
            .unwrap();
    }
    let first = corpus.prepare();
    assert!(!first.partial);
    assert_eq!(
        (first.area_vectors, first.embedded_areas),
        (2, 1),
        "two provenance bindings share one exact area input"
    );
    let restart = corpus.prepare();
    assert_eq!(
        (
            restart.document_calls,
            restart.area_vectors,
            restart.embedded_areas
        ),
        (0, 2, 0)
    );
    assert!(
        !corpus
            .engine()
            .derivation_pending(context_foundry::neural::work::Lane::Semantic)
            .unwrap()
            .overviews_pending
    );
}

#[test]
fn coarse_semantic_context_focuses_current_evidence_beyond_the_file_prefix() {
    use context_foundry::neural::partition::Representation;
    use context_foundry::store::ContextOptions;
    let path = "src/payments.rs";
    let body = format!(
        "// repayment credit reimbursement\n{}fn settle() {{\n    // refund cancelled payments\n    return_credit();\n}}\n{}",
        "// ordinary unrelated setup\n".repeat(200),
        "// ordinary unrelated cleanup\n".repeat(200),
    );
    let mut corpus = Corpus::unprepared(&[(path.into(), body.clone())]);
    select_body_recipe(&mut corpus, 512, 64, None);
    Arc::make_mut(&mut corpus.profile).representation = Representation::Overview;
    std::fs::write(
        &corpus.profile_path,
        serde_json::to_vec(&*corpus.profile).unwrap(),
    )
    .unwrap();
    assert!(!corpus.prepare().partial);
    let runtime = corpus.runtime();
    let query = "refund cancelled payments";
    let window = runtime
        .window(
            corpus.engine(),
            query,
            Instant::now() + Duration::from_secs(30),
            &Corpus::control(),
        )
        .unwrap();
    let trace = context_foundry::retrieval_trace::RetrievalTrace::default();
    let candidates = corpus
        .engine()
        .context_candidates_with(
            query,
            Strategy::Search,
            &Corpus::control().with_trace(Some(trace.clone())),
            &ContextOptions {
                dense: Some(&window),
                experimental_behavior: true,
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(candidates.batch.semantic.as_deref(), Some("ready"));
    assert!(
        candidates.batch.items[0].semantic.is_none(),
        "complete file coverage keeps precise lexical evidence first"
    );
    let item = candidates
        .batch
        .items
        .iter()
        .find(|item| item.is_dense_only() && item.handle.as_ref().is_some_and(|h| h.path == path))
        .expect("independent file vector is served");
    let handle = item.handle.as_ref().unwrap();
    assert!(handle.start > 4000, "the evidence is past the prefix");
    assert!(handle.end - handle.start <= 1536);
    assert_eq!(
        item.forms[0],
        context_foundry::store::RenderedForm::Verbatim(
            body[handle.start as usize..handle.end as usize].into()
        )
    );
    let packed = response::pack_enriched_context(
        &candidates,
        None,
        response::Budget::request(1024),
        &response::stdout_bytes,
    )
    .unwrap();
    assert!(packed.tokens <= 1024);
    assert!(packed.text.contains("return_credit();"), "{}", packed.text);
    assert!(
        trace.snapshot()["events"]
            .as_array()
            .unwrap()
            .iter()
            .any(|event| {
                event["stage"] == "materialization"
                    && event["origin"] == "dense"
                    && event["unit"]["path"] == path
                    && event["unit"]["start"] == 0
                    && event["excerpt"]["start"] == handle.start
                    && event["passage_independently_embedded"] == false
            })
    );
    // A loaded coarse generation cannot cite the old passage after an edit,
    // including an edit not yet published to the lexical index.
    corpus
        .engine_mut()
        .replace_source(path, "fn updated() {}\n")
        .unwrap();
    let current = corpus
        .engine()
        .context_candidates_with(
            query,
            Strategy::Search,
            &Corpus::control(),
            &ContextOptions {
                dense: Some(&window),
                experimental_behavior: true,
                ..Default::default()
            },
        )
        .unwrap();
    assert!(
        !current
            .batch
            .items
            .iter()
            .any(|item| item.handle.as_ref().is_some_and(|h| h.path == path))
    );
}

#[test]
fn overview_bootstrap_bounds_inputs_and_reuses_them_during_hierarchy_expansion() {
    use context_foundry::neural::partition::Representation;
    let original: String = (0..80)
        .map(|n| format!("/// Transfer record {n}.\nfn transfer_{n}() {{ save({n}); }}\n"))
        .collect();
    let path = "src/billing.rs";
    let mut corpus = Corpus::new(&[
        (path.into(), original.clone()),
        ("empty.rs".into(), " \n".into()),
    ]);
    select_body_recipe(&mut corpus, 512, 64, None);
    Arc::make_mut(&mut corpus.profile).representation = Representation::Overview;
    let save_profile = |corpus: &Corpus| {
        std::fs::write(
            &corpus.profile_path,
            serde_json::to_vec(&*corpus.profile).unwrap(),
        )
        .unwrap()
    };
    save_profile(&corpus);
    let prepared = corpus.prepare();
    assert!(!prepared.partial, "{prepared:?}");
    assert_eq!(
        prepared.embedded_units, 7,
        "one bounded input plus six built-in nonblank fixture files, not eighty bodies"
    );
    let overview = corpus.engine().semantic_partition(path).unwrap().unwrap();
    assert_eq!(overview.units.len(), 1);
    assert_eq!(
        (overview.units[0].start, overview.units[0].end),
        (0, original.len())
    );
    assert!(
        corpus
            .engine()
            .semantic_partition("empty.rs")
            .unwrap()
            .unwrap()
            .units
            .is_empty()
    );
    assert_eq!(
        corpus.prepare().document_calls,
        0,
        "unchanged restart reuses the file vector"
    );
    Arc::make_mut(&mut corpus.profile).representation =
        Representation::Hierarchy { parent_tokens: 64 };
    save_profile(&corpus);
    let expanded = corpus.prepare();
    assert!(!expanded.partial);
    let hierarchy = corpus.engine().semantic_partition(path).unwrap().unwrap();
    assert!(hierarchy.units.contains(&overview.units[0]));
    assert!(
        expanded.reused_cached_units >= 1,
        "file vector reused during expansion"
    );
    Arc::make_mut(&mut corpus.profile).representation = Representation::Overview;
    save_profile(&corpus);
    assert_eq!(
        corpus.prepare().document_calls,
        0,
        "switching back reuses the cache"
    );
    corpus
        .engine()
        .replace_source(
            path,
            &original.replace("Transfer record 0", "Refund record 0"),
        )
        .unwrap();
    assert_eq!(
        corpus.prepare().embedded_units,
        1,
        "only the changed overview"
    );
    corpus.engine().replace_source(path, &original).unwrap();
    assert_eq!(corpus.prepare().document_calls, 0, "revert reuse");
    corpus.engine().delete_source(path).unwrap();
    corpus.prepare();
    let mapping = corpus
        .engine()
        .semantic_current_mapping(
            &Control::unbounded(),
            &corpus.digest(),
            &corpus.profile.recipe_id(),
        )
        .unwrap();
    assert!(
        mapping
            .units
            .values()
            .all(|locations| locations.iter().all(|location| &*location.path != path))
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn hierarchy_cli_and_resident_bootstrap_cover_files_before_body_inference() {
    use context_foundry::neural::partition::Representation;
    let extra: Vec<_> = (0..21)
        .map(|n| {
            (
                format!("first/{n:02}.rs"),
                (0..30)
                    .map(|i| {
                        format!("/// Evidence {n} {i}.\nfn action_{n}_{i}() {{ perform({i}); }}\n")
                    })
                    .collect(),
            )
        })
        .collect();
    for resident in [false, true] {
        let mut corpus = Corpus::unprepared(&extra);
        select_body_recipe(&mut corpus, 512, 64, None);
        Arc::make_mut(&mut corpus.profile).representation =
            Representation::Hierarchy { parent_tokens: 64 };
        std::fs::write(
            &corpus.profile_path,
            serde_json::to_vec(&*corpus.profile).unwrap(),
        )
        .unwrap();
        *corpus.probe.publication_path.lock().unwrap() =
            Some(index::generation_dir(&corpus.store, &corpus.digest()).join(index::MANIFEST_FILE));
        if resident {
            let served = corpus
                .serve_stdio(Some(corpus.held(&Hold::default())))
                .await;
            served.prepare().await;
            let state = served.stopped().await;
            assert_eq!(state["unpartitioned_sources"], 0, "{state}");
            assert_eq!(state["missing_units"], 0, "{state}");
            served.close().await;
            corpus.reopen().await;
        } else {
            assert!(!corpus.prepare().partial);
        }
        let mapping = corpus
            .engine()
            .semantic_current_mapping(
                &Control::unbounded(),
                &corpus.digest(),
                &corpus.profile.recipe_id(),
            )
            .unwrap();
        let paths: BTreeSet<_> = mapping
            .units
            .values()
            .flat_map(|locations| locations.iter())
            .map(|unit| unit.path.clone())
            .collect();
        let first: BTreeSet<_> = paths
            .iter()
            .map(|path| {
                corpus
                    .engine()
                    .semantic_partition(path)
                    .unwrap()
                    .unwrap()
                    .units[0]
                    .input_key
                    .clone()
            })
            .collect();
        let keys = corpus.probe.document_keys.lock().unwrap().clone();
        assert_eq!(
            keys[..first.len()].iter().cloned().collect::<BTreeSet<_>>(),
            first,
            "resident={resident}: no body input may displace the first view of a later file"
        );
        assert!(keys.len() > first.len());
        assert_eq!(
            corpus
                .probe
                .publication_before_call
                .lock()
                .unwrap()
                .iter()
                .find(|(before, _)| *before == first.len() as u64)
                .copied(),
            Some((first.len() as u64, first.len() as u64)),
            "resident={resident}: publish every initial view before admitting any body refinement"
        );
        assert_eq!(
            keys.len(),
            keys.iter().collect::<BTreeSet<_>>().len(),
            "phase changes cannot resubmit accepted or in-flight keys"
        );
        assert_eq!(
            corpus.prepare().document_calls,
            0,
            "unchanged restart retains both phases"
        );
    }
}

#[test]
fn representation_metadata_upgrade_reuses_exact_vectors() {
    let mut corpus = Corpus::unprepared(&[]);
    select_body_recipe(&mut corpus, 512, 64, None);
    assert!(!corpus.prepare().partial);
    let mut record = serde_json::to_value(
        corpus
            .engine()
            .semantic_partition(CODE_PATH)
            .unwrap()
            .unwrap(),
    )
    .unwrap();
    record["recipe_id"] = record["recipe_id"]
        .as_str()
        .unwrap()
        .replace("+kinds1", "")
        .replace("+kinds2", "")
        .into();
    for unit in record["units"].as_array_mut().unwrap() {
        unit.as_object_mut().unwrap().remove("kind");
    }
    let legacy: context_foundry::neural::cache::PartitionRecord =
        serde_json::from_value(record.clone()).unwrap();
    assert!(
        legacy
            .units
            .iter()
            .all(|unit| unit.kind == context_foundry::neural::cache::CardKind::Unknown)
    );
    corpus.engine = None;
    testkit::write_raw_partition(
        &corpus.store,
        CODE_PATH,
        &serde_json::to_string(&record).unwrap(),
    );
    corpus.engine = Some(Engine::open_existing(&corpus.store).unwrap());
    let upgraded = corpus.prepare();
    assert!(!upgraded.partial);
    assert_eq!(
        upgraded.document_calls, 0,
        "metadata versioning must not re-embed identical inputs"
    );
    let record = corpus
        .engine()
        .semantic_partition(CODE_PATH)
        .unwrap()
        .unwrap();
    assert!(
        record
            .units
            .iter()
            .all(|unit| unit.kind == context_foundry::neural::cache::CardKind::Body)
    );
    assert_eq!(corpus.prepare().document_calls, 0);
}

#[test]
fn container_role_upgrade_reuses_vectors_and_republishes_exact_roles() {
    use context_foundry::neural::cache::{CardKind, PartitionRecord};
    let path = "src/container_roles.rs";
    let source = "impl Billing {\n    fn adjust() { settle(); }\n}\n";
    let mut corpus = Corpus::unprepared(&[(path.into(), source.into())]);
    select_body_recipe(&mut corpus, 512, 64, None);
    assert!(!corpus.prepare().partial);
    let current = corpus.engine().semantic_partition(path).unwrap().unwrap();
    assert!(
        current
            .units
            .iter()
            .any(|unit| unit.kind == CardKind::ContainerFragment)
    );
    let digest = corpus.digest();
    let previous_recipe = current.recipe_id.replace("+kinds2", "+kinds1");
    assert_ne!(previous_recipe, current.recipe_id);
    let calls = corpus.probe.document_calls();
    let control = Control::unbounded();

    // Seed the complete previous metadata recipe, then publish its real ANN
    // generation through the ordinary cache-backed rebuild. The rendered inputs
    // are unchanged; kinds1 classified every body-recipe location as Body.
    corpus.engine = None;
    let previous: Vec<(String, PartitionRecord)> =
        testkit::table_rows(&corpus.store, "semantic_partitions")
            .into_iter()
            .map(|(path, raw)| {
                let mut record: PartitionRecord = serde_json::from_str(&raw).unwrap();
                assert_eq!(record.recipe_id, current.recipe_id);
                record.recipe_id = previous_recipe.clone();
                for unit in &mut record.units {
                    unit.kind = CardKind::Body;
                }
                (path, record)
            })
            .collect();
    corpus.engine = Some(Engine::open_existing(&corpus.store).unwrap());
    for (previous_path, record) in &previous {
        corpus
            .engine()
            .semantic_record_partition(
                previous_path,
                &record.source_hash,
                &record.recipe_id,
                &digest,
                Some(&record.units),
                &|body| {
                    assert_eq!(context_foundry::digest(body.as_bytes()), record.source_hash);
                    Ok(record.units.clone())
                },
            )
            .unwrap();
    }
    corpus
        .engine()
        .semantic_begin_run(|state| {
            state.recipe_id = Some(previous_recipe.clone());
            state.coverage = None;
            state.bootstrap = None;
            state.state = "stopped".into();
        })
        .unwrap();
    let published = corpus.engine().semantic_rebuild_index(&control).unwrap();
    assert!(published.rebuilt && published.reason.is_none() && published.entries > 0);
    let store = context_foundry::neural::anchor::Dir::open_path(&corpus.store).unwrap();
    let prior_generation =
        index::validate_generation_with(&store, &digest, &previous_recipe, DIMENSIONS, &control)
            .unwrap();
    assert_eq!(prior_generation.manifest.coverage, index::COVERAGE_COMPLETE);
    assert!(
        prior_generation
            .labels
            .values()
            .flat_map(|locations| locations.iter())
            .all(|location| location.kind == CardKind::Body)
    );
    for unit in &current.units {
        assert!(
            prior_generation.labels[&unit.input_key]
                .iter()
                .any(|location| {
                    &*location.path == path
                        && location.start == unit.start as u64
                        && location.end == unit.end as u64
                        && location.kind == CardKind::Body
                })
        );
    }
    assert_eq!(corpus.probe.document_calls(), calls);

    let upgraded = corpus.prepare();
    assert!(!upgraded.partial && upgraded.index_published);
    assert_eq!(upgraded.document_calls, 0);
    assert_eq!(upgraded.input_tokens, 0);
    assert_eq!(corpus.probe.document_calls(), calls);
    let restored = corpus.engine().semantic_partition(path).unwrap().unwrap();
    assert_eq!(restored.recipe_id, current.recipe_id);
    assert_eq!(
        restored.units, current.units,
        "metadata changes neither source coordinates nor exact input keys"
    );
    let mapping = corpus
        .engine()
        .semantic_current_mapping(&control, &digest, &corpus.profile.recipe_id())
        .unwrap();
    assert!(
        mapping
            .units
            .values()
            .flat_map(|locations| locations.iter())
            .any(|location| {
                &*location.path == path && location.kind == CardKind::ContainerFragment
            })
    );
    let upgraded_generation =
        index::validate_generation_with(&store, &digest, &current.recipe_id, DIMENSIONS, &control)
            .unwrap();
    assert_eq!(
        upgraded_generation.manifest.coverage,
        index::COVERAGE_COMPLETE
    );
    assert_eq!(
        upgraded_generation.manifest.count,
        prior_generation.manifest.count
    );
    assert_eq!(
        upgraded_generation.labels.keys().collect::<BTreeSet<_>>(),
        prior_generation.labels.keys().collect::<BTreeSet<_>>(),
        "republication retains every exact cached input"
    );
    for unit in &current.units {
        assert!(
            upgraded_generation.labels[&unit.input_key]
                .iter()
                .any(|location| {
                    &*location.path == path
                        && location.start == unit.start as u64
                        && location.end == unit.end as u64
                        && location.kind == unit.kind
                })
        );
    }
    assert_eq!(corpus.prepare().document_calls, 0);
    assert_eq!(corpus.probe.document_calls(), calls);
}

#[test]
fn hierarchy_refines_implementation_bodies_after_container_only_discovery() {
    use context_foundry::neural::{cache::CardKind, partition::Representation};
    use context_foundry::store::{ContextOptions, RenderedForm};
    let path = "src/refund_containers.rs";
    let source: String = (0..12)
        .map(|i| {
            format!(
                "impl RefundTiming{i} {{\n    fn operation_{i}() {{ apply_adjustment({i}); }}\n}}\n"
            )
        })
        .collect();
    let mut corpus = Corpus::unprepared(&[(path.into(), source.clone())]);
    select_body_recipe(&mut corpus, 512, 64, None);
    Arc::make_mut(&mut corpus.profile).representation =
        Representation::Hierarchy { parent_tokens: 64 };
    std::fs::write(
        &corpus.profile_path,
        serde_json::to_vec(&*corpus.profile).unwrap(),
    )
    .unwrap();
    assert!(!corpus.prepare().partial);
    let partition = corpus.engine().semantic_partition(path).unwrap().unwrap();
    let fragments: BTreeSet<_> = partition
        .units
        .iter()
        .filter(|unit| unit.kind == CardKind::ContainerFragment)
        .map(|unit| (unit.start as u64, unit.end as u64))
        .collect();
    let bodies: BTreeSet<_> = partition
        .units
        .iter()
        .filter(|unit| unit.kind == CardKind::Body)
        .map(|unit| (unit.start as u64, unit.end as u64))
        .collect();
    assert!(fragments.len() >= 12 && bodies.len() == 12);
    let runtime = corpus.runtime();
    let mut window = runtime
        .window(
            corpus.engine(),
            "refund timing rules",
            Instant::now() + Duration::from_secs(30),
            &Corpus::control(),
        )
        .unwrap();
    // A valid coarse/discovery window may consist entirely of high-scoring
    // container fragments. They must not consume independently scored Body slots.
    window.hits = window
        .hits
        .iter()
        .filter(|hit| {
            window
                .units(hit)
                .iter()
                .any(|unit| &*unit.path == path && unit.kind == CardKind::ContainerFragment)
        })
        .cloned()
        .collect();
    assert!(!window.hits.is_empty());
    let trace = context_foundry::retrieval_trace::RetrievalTrace::default();
    let result = corpus
        .engine()
        .context_candidates_with(
            "refund timing rules",
            Strategy::Search,
            &Corpus::control().with_trace(Some(trace.clone())),
            &ContextOptions {
                dense: Some(&window),
                experimental_behavior: true,
                ..Default::default()
            },
        )
        .unwrap();
    let events = trace.snapshot();
    // Diversity reserves the first four file slots, then unused capacity
    // recovers additional independently scored bodies. Outlines are separate.
    let ranked: Vec<_> = result
        .batch
        .items
        .iter()
        .filter(|item| {
            item.handle
                .as_ref()
                .is_some_and(|handle| handle.path == path)
                && !matches!(item.forms.first(), Some(RenderedForm::Outline(_)))
        })
        .collect();
    assert!(ranked.len() > 4 && ranked.len() <= 32);
    assert!(ranked.iter().any(|item| item.semantic.is_some()));
    assert!(ranked.iter().any(|item| item.semantic.is_none()));
    assert!(
        bodies.iter().all(|span| ranked.iter().any(|item| {
            let h = item.handle.as_ref().unwrap();
            (h.start, h.end) == *span
        })),
        "every independent body survives before coarse backfill"
    );
    for item in ranked {
        let handle = item.handle.as_ref().unwrap();
        assert_eq!(handle.sha256, partition.source_hash);
        assert_eq!(
            item.forms[0],
            RenderedForm::Verbatim(source[handle.start as usize..handle.end as usize].into())
        );
        if item.semantic.is_some() {
            assert!(
                bodies.contains(&(handle.start, handle.end))
                    || fragments.contains(&(handle.start, handle.end))
            );
        } else {
            assert!(
                events["events"].as_array().unwrap().iter().any(|event| {
                    event["stage"] == "materialization"
                        && event["origin"] == "lexical"
                        && event["excerpt"] == serde_json::to_value(handle).unwrap()
                }),
                "the other file slots retain current lexical evidence: {events}"
            );
        }
    }
    let refined: Vec<_> = events["events"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|event| {
            event["stage"] == "selection"
                && event["refined"] == true
                && event["outcome"] == "eligible"
                && event["unit"]["path"] == path
        })
        .collect();
    assert_eq!(refined.len(), bodies.len());
    assert!(refined.iter().all(|event| event["representation"] == "body"
        && bodies.contains(&(
            event["unit"]["start"].as_u64().unwrap(),
            event["unit"]["end"].as_u64().unwrap(),
        ))));
    let packed = response::pack_enriched_context(
        &result,
        None,
        Budget::request(BUDGET),
        &response::stdout_bytes,
    )
    .unwrap();
    assert!(packed.tokens <= BUDGET);
    assert!(
        packed.text.contains("apply_adjustment"),
        "body evidence crosses the ordinary response boundary"
    );
    let before = corpus.probe.document_calls();
    assert_eq!(corpus.prepare().document_calls, 0);
    assert_eq!(corpus.probe.document_calls(), before);
}

#[test]
fn experimental_body_placement_survives_real_partial_to_complete_publication() {
    use context_foundry::neural::{cache::CardKind, partition::Representation};
    use context_foundry::store::ContextOptions;

    let query = "quasar zephyr lantern";
    let lexical_path = "src/target_reconciliation.rs";
    let noise_paths: BTreeSet<_> = (0..40)
        .map(|n| format!("src/ordinary_inventory_{n:02}.rs"))
        .collect();
    let mut extra: Vec<_> = noise_paths
        .iter()
        .enumerate()
        .map(|(n, path)| {
            (
                path.clone(),
                format!("fn ordinary_inventory_{n}() {{ record_serial({n}); }}\n"),
            )
        })
        .collect();
    extra.push((
        lexical_path.into(),
        "fn target_reconciliation() {\n    // quasar zephyr lantern\n    preserve_transaction();\n}\n"
            .into(),
    ));

    for representation in [
        Representation::Body { parent_tokens: 0 },
        Representation::Hierarchy { parent_tokens: 0 },
        Representation::Address,
    ] {
        let address = representation == Representation::Address;
        let hierarchy = matches!(representation, Representation::Hierarchy { .. });
        let retained_kind = if address {
            CardKind::Address
        } else {
            CardKind::Body
        };
        let mut corpus = Corpus::unprepared(&extra);
        select_body_recipe(&mut corpus, 512, 0, None);
        Arc::make_mut(&mut corpus.profile).representation = representation;
        corpus.profile.validate().unwrap();
        std::fs::write(
            &corpus.profile_path,
            serde_json::to_vec(&*corpus.profile).unwrap(),
        )
        .unwrap();
        assert!(!corpus.prepare().partial);
        let digest = corpus.digest();
        let recipe = corpus.profile.recipe_id();
        let original = corpus
            .engine()
            .semantic_current_mapping(&Corpus::control(), &digest, &recipe)
            .unwrap();
        assert!(original.scope.complete);
        for path in &noise_paths {
            assert_eq!(
                original
                    .units
                    .values()
                    .flat_map(|units| units.iter())
                    .filter(|unit| { *unit.path == **path && unit.kind == retained_kind })
                    .count(),
                1,
                "one complete small callable per dense file: {representation:?} {path}"
            );
        }

        // This is a placement regression, not an embedding-quality or ANN
        // repeatability claim. Control scores while preserving all real
        // prepared input keys, source hashes, roles and exact byte spans.
        let query_vector = concept_vector(&provider::render(&corpus.profile.query_template, query));
        let opposite: Vec<_> = query_vector.iter().map(|value| -*value).collect();
        let entries: Vec<_> = original
            .units
            .iter()
            .map(|(key, locations)| {
                let noise = locations
                    .iter()
                    .any(|unit| noise_paths.contains(&*unit.path) && unit.kind == retained_kind);
                (
                    key.clone(),
                    if noise {
                        query_vector.clone()
                    } else {
                        opposite.clone()
                    },
                )
            })
            .collect();
        corpus
            .engine()
            .semantic_cache_commit(&entries, &digest, DIMENSIONS, DEFAULT_CACHE_CAP_BYTES)
            .unwrap();

        // The new source has no query terms and cannot enter the retained
        // ordinary window or the first16 lexical discovery documents. Its
        // missing partition changes actual coverage, not existing candidates.
        let late_path = "src/late_checkpoint.rs";
        let late_source = "fn late_checkpoint() { remember_checkpoint(711); }\n";
        assert!(
            corpus
                .engine()
                .replace_source(late_path, late_source)
                .unwrap()
        );
        corpus.engine_mut().refresh(&Corpus::control()).unwrap();
        let revision = corpus.engine().source_revision().unwrap();
        assert_eq!(revision, original.scope.source_revision + 1);
        let rebuilt = corpus
            .engine()
            .semantic_rebuild_index(&Corpus::control())
            .unwrap();
        assert!(rebuilt.rebuilt && rebuilt.reason.is_none(), "{rebuilt:?}");
        let partial = corpus
            .engine()
            .semantic_current_mapping(&Corpus::control(), &digest, &recipe)
            .unwrap();
        assert!(!partial.scope.complete);
        assert_eq!(partial.scope.source_revision, revision);
        assert_eq!(partial.units, original.units);
        let store = context_foundry::neural::anchor::Dir::open_path(&corpus.store).unwrap();
        let published = index::validate_generation_with(
            &store,
            &digest,
            &recipe,
            DIMENSIONS,
            &Corpus::control(),
        )
        .unwrap();
        assert_eq!(published.manifest.coverage, index::COVERAGE_PARTIAL);
        assert_eq!(published.manifest.source_revision, revision);
        let hashes: Vec<_> = extra
            .iter()
            .map(|(path, _)| {
                (
                    path.clone(),
                    corpus.engine().source(path).unwrap().unwrap().hash,
                )
            })
            .collect();

        let queries_before = corpus.probe.query_calls();
        let runtime = Arc::new(corpus.runtime());
        let slot: SemanticSlot = Some(Ok(Arc::clone(&runtime)));
        let mut identities = Vec::new();
        let mut experimental_orders = Vec::new();
        let mut default_orders = Vec::new();
        for complete in [false, true] {
            if complete {
                let inputs_before = corpus.probe.document_inputs();
                let report = corpus.prepare();
                assert!(!report.partial && report.index_published, "{report:?}");
                let current = corpus
                    .engine()
                    .semantic_current_mapping(&Corpus::control(), &digest, &recipe)
                    .unwrap();
                assert!(current.scope.complete);
                assert_eq!(current.scope.source_revision, revision);
                for (key, locations) in &original.units {
                    assert_eq!(current.units.get(key), Some(locations));
                }
                let new_keys: BTreeSet<_> = current
                    .units
                    .keys()
                    .filter(|key| !original.units.contains_key(*key))
                    .collect();
                assert!(!new_keys.is_empty());
                assert!(new_keys.iter().all(|key| {
                    current.units[*key]
                        .iter()
                        .all(|unit| &*unit.path == late_path)
                }));
                assert_eq!(report.embedded_units, new_keys.len() as u64);
                assert_eq!(
                    corpus.probe.document_inputs() - inputs_before,
                    new_keys.len() as u64
                );
                assert!(report.document_calls > 0);
                let published = index::validate_generation_with(
                    &store,
                    &digest,
                    &recipe,
                    DIMENSIONS,
                    &Corpus::control(),
                )
                .unwrap();
                assert_eq!(published.manifest.coverage, index::COVERAGE_COMPLETE);
                assert_eq!(published.manifest.source_revision, revision);
            }
            let documents_before = corpus.probe.document_calls();
            assert_eq!(corpus.engine().source_revision().unwrap(), revision);
            for (path, hash) in &hashes {
                assert_eq!(&corpus.engine().source(path).unwrap().unwrap().hash, hash);
            }
            let mut window = runtime
                .window(
                    corpus.engine(),
                    query,
                    Instant::now() + Duration::from_secs(30),
                    &Corpus::control(),
                )
                .unwrap();
            let coverage = if complete { "ready" } else { "partial" };
            assert_eq!(window.coverage_at(revision), coverage);
            assert_eq!(window.complete_at(), complete.then_some(revision));

            // Hold the original 40 candidate inputs/locations fixed across
            // two real immutable publications. Numeric ANN label IDs may
            // change on rebuild; input identity and scores must not change.
            let mut retained: Vec<_> = window
                .hits
                .iter()
                .filter(|hit| {
                    window
                        .units(hit)
                        .iter()
                        .any(|unit| noise_paths.contains(&*unit.path) && unit.kind == retained_kind)
                })
                .cloned()
                .collect();
            retained.sort_by(|left, right| {
                window.units(left)[0]
                    .path
                    .cmp(&window.units(right)[0].path)
                    .then_with(|| window.input_key(left).cmp(window.input_key(right)))
            });
            assert_eq!(retained.len(), 40, "{representation:?} {coverage}");
            assert!(retained.iter().all(|hit| {
                let locations = window.units(hit);
                locations.len() == 1
                    && locations[0].kind == retained_kind
                    && noise_paths.contains(&*locations[0].path)
            }));
            window.hits = retained;
            identities.push(
                window
                    .hits
                    .iter()
                    .map(|hit| {
                        (
                            window.input_key(hit).to_owned(),
                            hit.distance.to_bits(),
                            window.units(hit).to_vec(),
                        )
                    })
                    .collect::<Vec<_>>(),
            );

            for experimental in [false, true] {
                let trace = context_foundry::retrieval_trace::RetrievalTrace::default();
                let result = corpus
                    .engine()
                    .context_candidates_with(
                        query,
                        Strategy::Search,
                        &Corpus::control().with_trace(Some(trace.clone())),
                        &ContextOptions {
                            dense: Some(&window),
                            experimental_behavior: experimental,
                            ..Default::default()
                        },
                    )
                    .unwrap();
                assert_eq!(result.batch.semantic.as_deref(), Some(coverage));
                assert_eq!(result.batch.freshness.source_revision, revision);
                let order: Vec<_> = result
                    .batch
                    .items
                    .iter()
                    .filter_map(|item| {
                        item.handle.as_ref().map(|handle| {
                            (
                                handle.path.clone(),
                                handle.start,
                                handle.end,
                                handle.sha256.clone(),
                                item.semantic.is_some(),
                            )
                        })
                    })
                    .collect();
                assert_eq!(order.len(), 32, "the candidate bound remains unchanged");
                if !address {
                    for (path, start, end, hash, semantic) in &order {
                        if *semantic {
                            assert!(
                                original.units.values().flat_map(|units| units.iter()).any(
                                    |unit| {
                                        unit.kind == CardKind::Body
                                            && *unit.path == **path
                                            && unit.start == *start
                                            && unit.end == *end
                                            && *unit.source_sha256 == **hash
                                    }
                                ),
                                "delivered semantic evidence preserves its scored span"
                            );
                        }
                    }
                }
                let lexical_position = order.iter().position(|item| item.0 == lexical_path);
                let preserved = experimental && (!address || !complete);
                assert_eq!(
                    lexical_position,
                    preserved.then_some(usize::from(hierarchy)),
                    "{representation:?} {coverage} experimental={experimental}"
                );
                assert!(!order.iter().any(|item| item.0 == late_path));
                let snapshot = trace.snapshot();
                let policies: Vec<_> = snapshot["events"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .filter(|event| event["stage"] == "placement_policy")
                    .collect();
                if experimental && (!address || !complete) {
                    assert_eq!(policies.len(), 1, "{snapshot}");
                    assert_eq!(
                        policies[0]["policy"],
                        if address {
                            "partial_prose_interleave_v1"
                        } else {
                            "body_prose_interleave_v1"
                        }
                    );
                    assert_eq!(policies[0]["coverage"], coverage);
                    assert_eq!(policies[0]["lexical_first"], !hierarchy);
                    assert_eq!(
                        policies[0]["eligible_refined_locations"].as_u64().unwrap() > 0,
                        hierarchy
                    );
                } else {
                    assert!(policies.is_empty(), "{snapshot}");
                }
                let packed = response::pack_enriched_context(
                    &result,
                    None,
                    Budget::request(BUDGET),
                    &response::stdout_bytes,
                )
                .unwrap();
                assert!(packed.tokens <= BUDGET);
                let parsed = parse_v2(&packed.text).unwrap();
                assert_eq!(
                    item_of(&parsed, lexical_path).is_some(),
                    preserved,
                    "{representation:?} {coverage} experimental={experimental}: {}",
                    packed.text
                );
                for item in parsed.items.iter().filter(|item| !item.handle.is_empty()) {
                    let (path, start, end) = span_of(&item.handle);
                    assert!(start < end);
                    assert!(
                        order
                            .iter()
                            .any(|unit| unit.0 == path && unit.1 == start && unit.2 == end)
                    );
                }
                if experimental {
                    experimental_orders.push(order);
                } else {
                    default_orders.push(order);
                }
            }

            // The production owner still bypasses query inference for an
            // anchored identifier, regardless of coverage or experimental mode.
            let anchored_calls = corpus.probe.query_calls();
            let options = ContextOptions {
                experimental_behavior: true,
                ..Default::default()
            };
            let anchored = mcp::context_primary_with_options(
                &slot,
                corpus.engine(),
                "target_reconciliation",
                Strategy::Search,
                &Corpus::control(),
                options,
            )
            .unwrap();
            let lexical = mcp::context_primary_with_options(
                &None,
                corpus.engine(),
                "target_reconciliation",
                Strategy::Search,
                &Corpus::control(),
                options,
            )
            .unwrap();
            let pack = |result: &context_foundry::memory::MemoryContext| {
                response::pack_enriched_context(
                    result,
                    None,
                    Budget::request(BUDGET),
                    &response::stdout_bytes,
                )
                .unwrap()
            };
            let anchored = pack(&anchored);
            assert!(anchored.tokens <= BUDGET);
            assert_eq!(anchored.text, pack(&lexical).text);
            let anchored = parse_v2(&anchored.text).unwrap();
            assert!(item_of(&anchored, lexical_path).is_some());
            assert!(anchored.items.iter().all(|item| !item.semantic));
            assert_eq!(corpus.probe.query_calls(), anchored_calls);
            assert_eq!(corpus.probe.document_calls(), documents_before);
        }
        assert_eq!(
            identities[0], identities[1],
            "candidate geometry/scores changed"
        );
        assert_eq!(
            default_orders[0], default_orders[1],
            "default placement changed"
        );
        if !address {
            assert_eq!(
                experimental_orders[0], experimental_orders[1],
                "coverage alone must not change Body/Hierarchy candidate ordering"
            );
        }
        assert_eq!(corpus.probe.query_calls() - queries_before, 2);
        runtime.shutdown();
    }
}

#[test]
fn hierarchy_refinement_scores_competing_bodies_outside_a_lexical_owner() {
    use context_foundry::neural::{cache::CardKind, partition::Representation};
    use context_foundry::store::{ContextOptions, RenderedForm};
    use context_foundry::syntax::{self, Lang, UnitKind};

    let path = "src/owner_scope.rs";
    let query = "quasar zephyr balance";
    let lines: String = (0..400)
        .map(|n| format!("    let carried_{n} = balance + {n};\n"))
        .collect();
    let target = format!(
        "fn reconcile_order() {{\n    // quasar zephyr balance\n{lines}    preserve_transaction();\n}}\n"
    );
    let siblings: String = (0..8)
        .map(|n| format!("    fn unrelated_{n}() {{ unrelated_action({n}); }}\n"))
        .collect();
    let source = format!("{target}impl Neighbors {{\n{siblings}}}\n");
    let owner = syntax::units(&source, Lang::Rust)
        .into_iter()
        .find(|unit| unit.kind == UnitKind::Fn && unit.start == 0)
        .expect("the lexical match belongs to a complete callable");
    assert!(owner.end - owner.start > 8192);
    let owner_range = owner.start as u64..owner.end as u64;
    let mut corpus = Corpus::unprepared(&[(path.into(), source.clone())]);
    select_body_recipe(&mut corpus, 512, 0, None);
    Arc::make_mut(&mut corpus.profile).representation =
        Representation::Hierarchy { parent_tokens: 0 };
    std::fs::write(
        &corpus.profile_path,
        serde_json::to_vec(&*corpus.profile).unwrap(),
    )
    .unwrap();
    assert!(!corpus.prepare().partial);
    let partition = corpus.engine().semantic_partition(path).unwrap().unwrap();
    let owned: BTreeSet<_> = partition
        .units
        .iter()
        .filter(|unit| {
            unit.kind == CardKind::Body && owner.start <= unit.start && unit.end <= owner.end
        })
        .map(|unit| (unit.start as u64, unit.end as u64))
        .collect();
    assert!(
        owned.len() > 4,
        "the callable has several exact body windows"
    );
    let preferred: BTreeSet<_> = owned.iter().rev().take(4).copied().collect();
    let competing: BTreeSet<_> = partition
        .units
        .iter()
        .filter(|unit| unit.kind == CardKind::Body && unit.start >= owner.end)
        .map(|unit| (unit.start as u64, unit.end as u64))
        .collect();
    assert!(preferred.iter().all(|(start, _)| *start > 8192));
    assert!(
        partition
            .units
            .iter()
            .filter(|unit| unit.kind == CardKind::Body && unit.start >= owner.end)
            .count()
            >= 8,
        "the same file has enough siblings to exhaust its four slots"
    );

    // Controlled fixture vectors make every sibling strictly closer than
    // every owned Body. Within the callable, its last four windows win, well
    // beyond the first lexical document that contains the query words. This
    // tests that owner priority is not a hard relevance filter. Both channels
    // must be scored, and the independently better sibling bodies must win.
    // These controlled vectors test admission/ranking, not model quality.
    // All keys, roles and source spans remain those produced by preparation.
    let query_vector = concept_vector(&provider::render(&corpus.profile.query_template, query));
    let opposite: Vec<_> = query_vector.iter().map(|value| -*value).collect();
    assert_eq!(
        query_vector[0], 0.0,
        "the query has no first-dimension concept"
    );
    let mut tail_vector = opposite.clone();
    tail_vector[0] = 1.0;
    let norm = tail_vector
        .iter()
        .map(|value| value * value)
        .sum::<f32>()
        .sqrt();
    tail_vector.iter_mut().for_each(|value| *value /= norm);
    let entries: Vec<_> = partition
        .units
        .iter()
        .map(|unit| {
            (
                unit.input_key.clone(),
                if unit.kind == CardKind::Body
                    && owned.contains(&(unit.start as u64, unit.end as u64))
                {
                    if preferred.contains(&(unit.start as u64, unit.end as u64)) {
                        tail_vector.clone()
                    } else {
                        opposite.clone()
                    }
                } else {
                    query_vector.clone()
                },
            )
        })
        .collect();
    corpus
        .engine()
        .semantic_cache_commit(
            &entries,
            &corpus.digest(),
            DIMENSIONS,
            DEFAULT_CACHE_CAP_BYTES,
        )
        .unwrap();
    let rebuilt = corpus
        .engine()
        .semantic_rebuild_index(&Corpus::control())
        .unwrap();
    assert!(rebuilt.rebuilt && rebuilt.reason.is_none(), "{rebuilt:?}");
    let documents_before = corpus.probe.document_calls();
    let queries_before = corpus.probe.query_calls();
    let runtime = corpus.runtime();
    let mut window = runtime
        .window(
            corpus.engine(),
            query,
            Instant::now() + Duration::from_secs(30),
            &Corpus::control(),
        )
        .unwrap();
    let original_hits = window.hits.clone();
    for discovery_kind in [CardKind::File, CardKind::Body, CardKind::ContainerFragment] {
        // Tiny Body/fragment hits discover the file but cannot claim ownership
        // of its refinement. The current lexical callable gets admission
        // priority, but must not veto the rest of the discovered file.
        window.hits = original_hits
            .iter()
            .filter(|hit| {
                window.units(hit).iter().any(|unit| {
                    &*unit.path == path
                        && unit.kind == discovery_kind
                        && (discovery_kind == CardKind::File
                            || (unit.start >= owner_range.end && unit.end - unit.start < 128))
                })
            })
            .take(1)
            .cloned()
            .collect();
        assert!(
            !window.hits.is_empty(),
            "missing {discovery_kind:?} discovery"
        );
        let trace = context_foundry::retrieval_trace::RetrievalTrace::default();
        let result = corpus
            .engine()
            .context_candidates_with(
                query,
                Strategy::Search,
                &Corpus::control().with_trace(Some(trace.clone())),
                &ContextOptions {
                    dense: Some(&window),
                    experimental_behavior: true,
                    ..Default::default()
                },
            )
            .unwrap();
        let events = trace.snapshot();
        assert!(
            events["events"].as_array().unwrap().iter().any(|event| {
                event["stage"] == "selection"
                    && event["origin"] == "lexical"
                    && event["outcome"] == "eligible"
                    && event["unit"]["path"] == path
                    && event["unit"]["start"] == owner_range.start
                    && event["unit"]["end"] == owner_range.end
                    && event["document"]["end"]
                        .as_u64()
                        .is_some_and(|end| end < preferred.first().unwrap().0)
            }),
            "the current lexical document preserves its larger callable: {events}"
        );
        let scope = events["events"]
            .as_array()
            .unwrap()
            .iter()
            .find(|event| event["stage"] == "refinement_scope" && event["path"] == path)
            .expect("the owner decision is auditable");
        assert!(scope["fallback"].is_null());
        assert_eq!(
            scope["ranges"],
            serde_json::json!([[owner_range.start, owner_range.end]])
        );
        let work = &events["events"]
            .as_array()
            .unwrap()
            .iter()
            .find(|event| event["stage"] == "refinement" && event["outcome"] == "scored")
            .unwrap()["work"];
        assert_eq!(work["owner_scoped_paths"], 1);
        assert!(work["units_considered"].as_u64().unwrap() <= 32_768);
        assert!(work["vectors_scored"].as_u64().unwrap() <= 4096);
        assert!(work["retained_locations"].as_u64().unwrap() <= 80 * 4);
        let refined: Vec<_> = events["events"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|event| {
                event["stage"] == "selection"
                    && event["refined"] == true
                    && event["outcome"] == "eligible"
                    && event["unit"]["path"] == path
            })
            .collect();
        assert_eq!(
            refined.len(),
            owned.len() + competing.len(),
            "{discovery_kind:?}: {events}"
        );
        assert_eq!(scope["policy"], "owner_prioritized_file_refinement_v2");
        assert_eq!(work["owner_prioritized_locations"], owned.len());
        assert_eq!(work["file_lane_locations"], competing.len());
        assert!(
            refined.iter().all(|event| {
                event["representation"] == "body"
                    && owned.union(&competing).any(|span| {
                        *span
                            == (
                                event["unit"]["start"].as_u64().unwrap(),
                                event["unit"]["end"].as_u64().unwrap(),
                            )
                    })
            }),
            "every independently scored body must retain its exact provenance: {events}"
        );
        assert!(
            refined.iter().take(4).all(|event| competing.contains(&(
                event["unit"]["start"].as_u64().unwrap(),
                event["unit"]["end"].as_u64().unwrap(),
            ))),
            "best independently scored bodies stay first"
        );
        let owned_order: Vec<_> = refined
            .iter()
            .filter_map(|event| {
                let span = (
                    event["unit"]["start"].as_u64().unwrap(),
                    event["unit"]["end"].as_u64().unwrap(),
                );
                owned.contains(&span).then_some(span)
            })
            .collect();
        assert_eq!(owned_order.len(), owned.len());
        assert!(
            owned_order
                .iter()
                .take(4)
                .all(|span| preferred.contains(span)),
            "owner priority still admits every owned body, with its own cosine order"
        );
        // Context appends a separate whole-file outline after the ranked
        // units. It is not a fifth independently scored refinement passage.
        let (outlines, delivered): (Vec<_>, Vec<_>) = result
            .batch
            .items
            .iter()
            .filter(|item| {
                item.handle
                    .as_ref()
                    .is_some_and(|handle| handle.path == path)
            })
            .partition(|item| matches!(item.forms.first(), Some(RenderedForm::Outline(_))));
        assert_eq!(outlines.len(), 1, "{discovery_kind:?}");
        let outline = outlines[0];
        assert!(outline.semantic.is_none());
        let handle = outline.handle.as_ref().unwrap();
        assert_eq!((handle.start, handle.end), (0, source.len() as u64));
        assert_eq!(handle.sha256, partition.source_hash);
        assert!(
            outline
                .forms
                .iter()
                .all(|form| matches!(form, RenderedForm::Outline(_) | RenderedForm::OutlineMin(_)))
        );
        assert!(
            delivered.len() > 4 && delivered.len() <= 32,
            "{discovery_kind:?}"
        );
        assert!(delivered.iter().any(|item| item.semantic.is_some()));
        assert!(delivered.iter().any(|item| item.semantic.is_none()));
        let mut lexical_spans = BTreeSet::new();
        for item in delivered {
            let handle = item.handle.as_ref().unwrap();
            if item.semantic.is_some() {
                assert!(
                    owned.contains(&(handle.start, handle.end))
                        || competing.contains(&(handle.start, handle.end))
                );
            } else {
                assert!(owner_range.start <= handle.start && handle.end <= owner_range.end);
                assert!(
                    events["events"].as_array().unwrap().iter().any(|event| {
                        event["stage"] == "materialization"
                            && event["origin"] == "lexical"
                            && event["unit"]["start"] == owner_range.start
                            && event["unit"]["end"] == owner_range.end
                            && event["excerpt"] == serde_json::to_value(handle).unwrap()
                    }),
                    "lexical slots must retain the current callable's exact excerpt: {events}"
                );
                lexical_spans.insert((handle.start, handle.end));
                for form in &item.forms {
                    if let RenderedForm::Excerpt { start, body, .. } = form {
                        let end = start + body.len() as u64;
                        assert!(handle.start <= *start && end <= handle.end);
                        assert_eq!(body, &source[*start as usize..end as usize]);
                        lexical_spans.insert((*start, end));
                    }
                }
            }
            assert_eq!(handle.sha256, partition.source_hash);
            assert_eq!(
                item.forms[0],
                RenderedForm::Verbatim(source[handle.start as usize..handle.end as usize].into())
            );
        }
        let packed = response::pack_enriched_context(
            &result,
            None,
            Budget::request(BUDGET),
            &response::stdout_bytes,
        )
        .unwrap();
        assert!(packed.tokens <= BUDGET);
        let parsed = parse_v2(&packed.text).unwrap();
        let mut packed_bodies = 0;
        for item in parsed.items.iter().filter(|item| !item.handle.is_empty()) {
            let (item_path, start, end) = span_of(&item.handle);
            if item_path != path {
                continue;
            }
            if item.form.as_deref() == Some("outline") {
                assert!(!item.semantic);
                assert_eq!((start, end), (0, source.len() as u64));
            } else if item.semantic {
                if packed_bodies == 0 {
                    assert!(
                        competing.contains(&(start, end)),
                        "better independently scored siblings must survive packing"
                    );
                }
                packed_bodies += 1;
                assert!(owned.contains(&(start, end)) || competing.contains(&(start, end)));
            } else {
                assert!(
                    lexical_spans.contains(&(start, end)),
                    "the packed lexical body must match a verified preferred or fallback form"
                );
            }
        }
        assert!(packed_bodies > 0, "{discovery_kind:?}: {}", packed.text);
    }
    assert_eq!(corpus.probe.query_calls(), queries_before + 1);
    assert_eq!(corpus.probe.document_calls(), documents_before);
}

#[test]
fn serving_rejects_a_rehashed_mapping_that_claims_the_wrong_representation() {
    let corpus = Corpus::new(&[]);
    let runtime = corpus.runtime();
    let window = || {
        runtime.window(
            corpus.engine(),
            DUSK_QUERY,
            Instant::now() + Duration::from_secs(30),
            &Control::unbounded(),
        )
    };
    assert!(window().is_ok());
    let directory = index::generation_dir(&corpus.store, &corpus.digest());
    let manifest_path = directory.join(index::MANIFEST_FILE);
    let mut manifest: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&manifest_path).unwrap()).unwrap();
    let old_labels = directory.join(format!(
        "locations-{}.json",
        manifest["labels_sha256"].as_str().unwrap()
    ));
    let mut labels: serde_json::Value =
        serde_json::from_slice(&std::fs::read(old_labels).unwrap()).unwrap();
    // The first key's first location: `[file, kind, start, end]`.
    labels["labels"][0][1][0][1] = "body".into();
    let bytes = serde_json::to_vec(&labels).unwrap();
    let hash = context_foundry::digest(&bytes);
    std::fs::write(directory.join(format!("locations-{hash}.json")), bytes).unwrap();
    manifest["labels_sha256"] = hash.into();
    std::fs::write(manifest_path, serde_json::to_vec(&manifest).unwrap()).unwrap();
    let calls = corpus.probe.query_calls();
    let refused = window().unwrap_err();
    assert_eq!(refused.code, "index_unavailable");
    assert_eq!(
        corpus.probe.query_calls(),
        calls,
        "reject malformed provenance before embedding"
    );
    runtime.shutdown();
}

#[test]
fn hierarchy_refinement_preserves_scored_body_spans_and_rechecks_freshness() {
    use context_foundry::neural::cache::CardKind;
    use context_foundry::neural::partition::Representation;
    use context_foundry::store::ContextOptions;
    let path = "src/settlement.rs";
    let source = format!(
        "/// Reimbursement timing for cancelled transfers.\nfn settle() {{\n{}    issue_credit();\n}}\n{}",
        "    // Preserve the original transaction state.\n".repeat(48),
        "// refund timing is discussed elsewhere\n".repeat(70),
    );
    let shared_path = "src/settlement_copy.rs";
    let mut extra = vec![(path.into(), source.clone()), (shared_path.into(), source)];
    extra.extend((0..128).map(|i| {
        (
            format!("src/unrelated_{i}.rs"),
            format!("fn inventory_{i}() {{ record_serial({i}); }}\n"),
        )
    }));
    let mut corpus = Corpus::unprepared(&extra);
    select_body_recipe(&mut corpus, 2048, 64, None);
    Arc::make_mut(&mut corpus.profile).representation =
        Representation::Hierarchy { parent_tokens: 64 };
    std::fs::write(
        &corpus.profile_path,
        serde_json::to_vec(&*corpus.profile).unwrap(),
    )
    .unwrap();
    assert!(!corpus.prepare().partial);
    let partition = corpus.engine().semantic_partition(path).unwrap().unwrap();
    let bodies: BTreeSet<_> = partition
        .units
        .iter()
        .filter(|unit| unit.kind == CardKind::Body)
        .map(|unit| (unit.start as u64, unit.end as u64))
        .collect();
    assert!(!bodies.is_empty());
    let runtime = corpus.runtime();
    let query = "refund timing rules";
    let mut window = runtime
        .window(
            corpus.engine(),
            query,
            Instant::now() + Duration::from_secs(30),
            &Corpus::control(),
        )
        .unwrap();
    // Exercise the refinement boundary with a coarse-only initial ANN window.
    // The body inputs exist in the generation but none is in these initial hits.
    window.hits = window
        .hits
        .iter()
        .filter(|hit| {
            window
                .units(hit)
                .iter()
                .any(|unit| unit.kind == CardKind::File)
        })
        .cloned()
        .collect();
    assert!(!window.hits.is_empty());
    let trace = context_foundry::retrieval_trace::RetrievalTrace::default();
    let select = || {
        corpus
            .engine()
            .context_candidates_with(
                query,
                Strategy::Search,
                &Corpus::control().with_trace(Some(trace.clone())),
                &ContextOptions {
                    dense: Some(&window),
                    experimental_behavior: true,
                    ..Default::default()
                },
            )
            .unwrap()
    };
    let result = select();
    assert!(
        trace.snapshot()["events"]
            .as_array()
            .unwrap()
            .iter()
            .any(|event| event["stage"] == "selection"
                && event["refined"] == true
                && event["representation"] == "body"
                && event["outcome"] == "eligible"
                && event["unit"]["path"] == path),
        "bounded scoring must discover a published body beyond the initial ANN window"
    );
    assert!(
        result.batch.items.iter().any(|item| {
            item.semantic.is_some()
                && item.handle.as_ref().is_some_and(|handle| {
                    handle.path == path && bodies.contains(&(handle.start, handle.end))
                })
        }),
        "an independently scored body passage must survive materialization"
    );
    // The generation has many unrelated vectors, but refinement reads only
    // selected mappings and never needs to traverse those ANN nodes.
    let mapping = corpus
        .engine()
        .semantic_current_mapping(
            &Corpus::control(),
            &corpus.digest(),
            &corpus.profile.recipe_id(),
        )
        .unwrap();
    assert!(mapping.units.len() > DENSE_WINDOW * 2);
    let paths: BTreeSet<_> = mapping
        .units
        .values()
        .flat_map(|units| units.iter().map(|unit| String::from(&*unit.path)))
        .collect();
    let saved: Vec<_> = paths
        .iter()
        .map(|path| {
            (
                path.clone(),
                corpus.engine().semantic_partition(path).unwrap().unwrap(),
            )
        })
        .collect();
    let replace_mapping =
        |path: &str,
         record: &context_foundry::neural::cache::PartitionRecord,
         units: &[context_foundry::neural::cache::PartitionUnit]| {
            corpus
                .engine()
                .semantic_record_partition(
                    path,
                    &record.source_hash,
                    &record.recipe_id,
                    &record.function_digest,
                    Some(units),
                    &|_| Ok(units.to_vec()),
                )
                .unwrap();
        };
    let shared = saved
        .iter()
        .find(|(path, _)| path == shared_path)
        .unwrap()
        .1
        .clone();
    assert!(
        shared
            .units
            .iter()
            .filter(|unit| unit.kind == CardKind::Body)
            .all(|unit| partition
                .units
                .iter()
                .any(|other| other.input_key == unit.input_key))
    );
    let coarse: Vec<_> = shared
        .units
        .iter()
        .filter(|unit| unit.kind == CardKind::File)
        .cloned()
        .collect();
    let assert_joined_only = |before: usize| {
        let snapshot = trace.snapshot();
        let events = &snapshot["events"].as_array().unwrap()[before..];
        assert!(events.iter().any(|event| event["stage"] == "selection"
            && event["refined"] == true
            && event["outcome"] == "eligible"
            && event["unit"]["path"] == path));
        assert!(
            !events.iter().any(|event| event["stage"] == "selection"
                && event["refined"] == true
                && event["unit"]["path"] == shared_path),
            "a shared input key cannot promote an unjoined location"
        );
    };
    replace_mapping(shared_path, &shared, &coarse);
    let before = trace.snapshot()["events"].as_array().unwrap().len();
    select();
    assert_joined_only(before);
    // A row too large for bounded query decoding must not piggyback on the
    // same input key's successfully joined provenance in the other file.
    let mut oversized = shared.units.clone();
    oversized.extend((0..corpus.source(shared_path).len()).map(|start| {
        context_foundry::neural::cache::PartitionUnit {
            kind: CardKind::Body,
            start,
            end: start + 1,
            input_key: "ab".repeat(32),
        }
    }));
    oversized.sort_by(|a, b| {
        a.start
            .cmp(&b.start)
            .then_with(|| b.end.cmp(&a.end))
            .then_with(|| a.input_key.cmp(&b.input_key))
    });
    assert!(serde_json::to_vec(&oversized).unwrap().len() > 512 * 1024);
    replace_mapping(shared_path, &shared, &oversized);
    let before = trace.snapshot()["events"].as_array().unwrap().len();
    let bounded = select();
    assert!(bounded.batch.counters.candidates_full);
    let packed = response::pack_enriched_context(
        &bounded,
        None,
        response::Budget::request(2048),
        &response::stdout_bytes,
    )
    .unwrap();
    assert!(
        packed
            .text
            .lines()
            .next()
            .unwrap()
            .contains("candidates:full")
    );
    assert_joined_only(before);
    assert!(
        trace.snapshot()["events"]
            .as_array()
            .unwrap()
            .iter()
            .rev()
            .find(|event| event["stage"] == "refinement")
            .unwrap()["work"]["mapping_rows_skipped"]
            .as_u64()
            .unwrap()
            > 0
    );
    replace_mapping(shared_path, &shared, &shared.units);
    // Keep every selected source current but remove its fine partition work.
    // The retained generation still contains body vectors: this specifically
    // tests the selected metadata gate, not an empty global ANN shortcut.
    for (path, record) in &saved {
        let coarse: Vec<_> = record
            .units
            .iter()
            .filter(|unit| unit.kind == CardKind::File)
            .cloned()
            .collect();
        replace_mapping(path, record, &coarse);
    }
    select();
    let last_work = || {
        trace.snapshot()["events"]
            .as_array()
            .unwrap()
            .iter()
            .rev()
            .find(|event| event["stage"] == "refinement")
            .unwrap()["work"]
            .clone()
    };
    assert_eq!(last_work()["vectors_scored"], 0);
    assert_eq!(last_work()["unpublished_units"], 0);
    for (path, record) in &saved {
        replace_mapping(path, record, &record.units);
    }
    // A current Body mapping alone is insufficient: its exact key must also
    // occur in this retained publication before any resident vector is read.
    let mut unpublished = partition.units.clone();
    unpublished
        .iter_mut()
        .find(|unit| unit.kind == CardKind::Body)
        .unwrap()
        .input_key = "ef".repeat(32);
    replace_mapping(path, &partition, &unpublished);
    select();
    assert!(last_work()["unpublished_units"].as_u64().unwrap() > 0);
    replace_mapping(path, &partition, &partition.units);
    corpus
        .engine()
        .replace_source(path, "fn replaced() {}\n")
        .unwrap();
    let fresh = select();
    assert!(
        fresh.batch.items.iter().all(|item| item
            .handle
            .as_ref()
            .is_none_or(|handle| handle.path != path)),
        "the retained query/generation cannot bypass the final source hash check"
    );
    runtime.shutdown();
    assert!(
        corpus.prepare().document_calls > 0,
        "the changed source needs new body and overview inputs"
    );
    assert_eq!(
        corpus.prepare().document_calls,
        0,
        "unchanged replay has no inference"
    );
}

#[test]
fn exact_body_keeps_matching_lexical_label_without_refocusing() {
    use context_foundry::neural::{cache::CardKind, partition::Representation};
    use context_foundry::store::{ContextOptions, RenderedForm};
    let path = "src/refund_rule.rs";
    let source = "fn refund_rule() { issue_credit(); }";
    let mut corpus = Corpus::unprepared(&[(path.into(), source.into())]);
    select_body_recipe(&mut corpus, 2048, 64, None);
    Arc::make_mut(&mut corpus.profile).representation =
        Representation::Hierarchy { parent_tokens: 64 };
    std::fs::write(
        &corpus.profile_path,
        serde_json::to_vec(&*corpus.profile).unwrap(),
    )
    .unwrap();
    assert!(!corpus.prepare().partial);
    assert!(
        corpus
            .engine()
            .semantic_partition(path)
            .unwrap()
            .unwrap()
            .units
            .iter()
            .any(|unit| unit.kind == CardKind::Body && unit.start == 0 && unit.end == source.len())
    );
    let runtime = corpus.runtime();
    let query = "refund timing rules";
    let window = runtime
        .window(
            corpus.engine(),
            query,
            Instant::now() + Duration::from_secs(30),
            &Corpus::control(),
        )
        .unwrap();
    let context = corpus
        .engine()
        .context_candidates_with(
            query,
            Strategy::Search,
            &Corpus::control(),
            &ContextOptions {
                dense: Some(&window),
                experimental_behavior: true,
                ..Default::default()
            },
        )
        .unwrap();
    let item = context
        .batch
        .items
        .iter()
        .find(|item| {
            item.handle.as_ref().is_some_and(|handle| {
                handle.path == path && handle.start == 0 && handle.end == source.len() as u64
            })
        })
        .unwrap();
    assert_eq!(item.label, "fn refund_rule");
    assert!(item.semantic.is_some());
    assert!(
        !item.is_dense_only(),
        "the exact lexical delivery unit supports this body span"
    );
    assert_eq!(item.forms, vec![RenderedForm::Verbatim(source.into())]);
    runtime.shutdown();
}

#[test]
fn hierarchy_vectors_are_independent_and_incremental_with_shared_source_ranges() {
    use context_foundry::neural::partition::Representation;
    let path = "src/billing.rs";
    let original = "impl Billing {\n fn refund() { transfer(400); }\n fn keep() { save(7); }\n}\n";
    let mut corpus = Corpus::new(&[(path.into(), original.into())]);
    select_body_recipe(&mut corpus, 512, 64, None);
    corpus.prepare();
    let bodies = corpus.engine().semantic_partition(path).unwrap().unwrap();
    Arc::make_mut(&mut corpus.profile).representation =
        Representation::Hierarchy { parent_tokens: 64 };
    std::fs::write(
        &corpus.profile_path,
        serde_json::to_vec(&*corpus.profile).unwrap(),
    )
    .unwrap();
    let prepared = corpus.prepare();
    assert!(!prepared.partial);
    assert!(
        prepared.embedded_units >= 2,
        "independent overviews are new"
    );
    let hierarchy = corpus.engine().semantic_partition(path).unwrap().unwrap();
    for body in &bodies.units {
        assert!(
            hierarchy.units.contains(body),
            "body key/range remains reusable"
        );
    }
    assert_eq!(hierarchy.units.len(), bodies.units.len() + 2);
    assert_eq!(corpus.prepare().document_calls, 0, "unchanged restart");
    let edited = original.replace("transfer(400)", "transfer(800)");
    corpus.engine().replace_source(path, &edited).unwrap();
    let changed = corpus.prepare();
    assert_eq!(
        changed.embedded_units, 3,
        "changed body plus independent parent and file"
    );
    let mapping = corpus
        .engine()
        .semantic_current_mapping(
            &Control::unbounded(),
            &corpus.digest(),
            &corpus.profile.recipe_id(),
        )
        .unwrap();
    assert!(mapping.scope.complete);
    corpus.engine().replace_source(path, original).unwrap();
    assert_eq!(
        corpus.prepare().document_calls,
        0,
        "revert reuses all independent inputs"
    );
    corpus.engine().delete_source(path).unwrap();
    corpus.prepare();
    assert!(
        corpus
            .engine()
            .semantic_current_mapping(
                &Control::unbounded(),
                &corpus.digest(),
                &corpus.profile.recipe_id()
            )
            .unwrap()
            .units
            .values()
            .all(|units| units.iter().all(|unit| &*unit.path != path))
    );
}

#[test]
fn body_recipe_uses_explicit_large_geometry_and_packs_short_and_long_inputs_by_actual_tokens() {
    let source = format!(
        "fn opaque() {{\n{}\n}}\n",
        "    send_credit_with_backoff();\n".repeat(180)
    );
    let mut corpus = Corpus::unprepared(&[("src/long.rs".into(), source)]);
    select_body_recipe(
        &mut corpus,
        4096,
        64,
        Some(provider::InputGeometry {
            v: 1,
            input_tokens: 8192,
            batch_tokens: 8192,
            sequences: 8,
        }),
    );
    let report = corpus.prepare();
    assert!(!report.partial, "{report:?}");
    let batches = corpus.probe.document_batches.lock().unwrap().clone();
    assert!(
        batches.iter().flatten().any(|&n| n > 2048),
        "large body reaches the provider intact"
    );
    assert!(
        batches.iter().any(|b| b.len() > 2),
        "short inputs are not capped by batch × worst-case size"
    );
    assert!(
        batches.iter().all(|b| b.len() <= 8
            && b.iter().sum::<usize>() <= 8192
            && b.iter().all(|&n| n <= 4096))
    );
    assert_eq!(
        batches
            .iter()
            .map(|b| b.iter().sum::<usize>() as u64)
            .sum::<u64>(),
        report.input_tokens
    );
    let runtime = corpus.runtime();
    let query = "credit ".repeat(500);
    runtime
        .embed(&query, Instant::now() + Duration::from_secs(30))
        .unwrap();
    assert_eq!(
        corpus.probe.queries().last(),
        Some(&provider::render(&corpus.profile.query_template, &query)),
        "the query also uses the explicit input limit without truncation"
    );
    let calls = corpus.probe.query_calls();
    assert!(matches!(
        runtime.embed(
            &"credit ".repeat(1300),
            Instant::now() + Duration::from_secs(30)
        ),
        Err(ProviderError::InputTooLarge(_))
    ));
    assert_eq!(corpus.probe.query_calls(), calls);
    drop(runtime);
    assert_eq!(corpus.prepare().document_calls, 0);
}

#[tokio::test]
async fn body_recipe_mcp_maintenance_keeps_token_caps_and_reuses_unchanged_context() {
    let extra: Vec<_> = (0..7)
        .map(|i| {
            (
                format!("src/b{i}.rs"),
                format!("fn b{i}() {{\n{}\n}}", "    send_credit();\n".repeat(48)),
            )
        })
        .collect();
    let mut corpus = Corpus::unprepared(&extra);
    select_body_recipe(&mut corpus, 1536, 64, None);
    let semantic = SemanticServing::with_provider(
        corpus.profile_path.clone(),
        maker(corpus.profile.descriptor.clone(), corpus.probe.clone()),
    );
    let served = corpus.serve_stdio(Some(semantic)).await;
    assert!(!served.semantic_action("maintain").await.0);
    served
        .until("body coverage", |s| {
            s["state"] == "waiting" && s["pending"]["paths"] == 0
        })
        .await;
    let calls = corpus.probe.document_calls();
    let original = corpus.source("src/b0.rs");
    std::fs::write(
        corpus.root.join("src/b0.rs"),
        original.replace("send_credit", "return_credit"),
    )
    .unwrap();
    served.index().await;
    served
        .until("body edit", |s| {
            s["state"] == "waiting"
                && s["pending"]["paths"] == 0
                && corpus.probe.document_calls() > calls
        })
        .await;
    let answer = served.context(LEDGER_QUERY).await;
    assert_eq!(header_word(&answer), Some("ready"));
    let batches = corpus.probe.document_batches.lock().unwrap().clone();
    assert!(
        batches
            .iter()
            .all(|b| b.len() <= 8 && b.iter().sum::<usize>() <= 2048)
    );
    assert!(batches.iter().flatten().any(|&n| n > 512));
    served.close().await;
}

#[test]
fn changed_restart_refreshes_only_affected_paths_and_matches_an_independent_audit() {
    use context_foundry::neural::fault_names::{
        MAPPING_FULL_PAGE, MAPPING_READ_SNAPSHOT, PARTITION_READ,
    };
    let extra: Vec<_> = (0..260)
        .map(|n| {
            (
                format!("notes/{n:04}.md"),
                "# Shared note\nbody\n".to_owned(),
            )
        })
        .collect();
    let mut corpus = Corpus::new(&extra);
    let body = corpus.source(CODE_PATH);
    accept_fixture_edit(&corpus, CODE_PATH, &format!("\n{body}"));
    accept_fixture_edit(&corpus, "new.rs", "pub fn new_only() {}\n");
    accept_fixture_edit(&corpus, "empty.rs", "\n");
    corpus.engine().delete_source(DUSK_PATH).unwrap();
    corpus.engine = None;
    corpus.engine = Some(Engine::open_existing(&corpus.store).unwrap());
    fault::disarm_all();
    corpus
        .engine()
        .semantic_publish_pending(&Control::unbounded())
        .unwrap();
    assert_eq!(fault::reached(MAPPING_FULL_PAGE), 0);
    assert_eq!(
        fault::reached(PARTITION_READ),
        3,
        "only extant dirty sources"
    );
    assert_eq!(fault::reached(MAPPING_READ_SNAPSHOT), 1);
    let store = context_foundry::neural::anchor::Dir::open_path(&corpus.store).unwrap();
    let state = corpus.engine().semantic_state().unwrap().unwrap();
    let actual = index::validate_generation_with(
        &store,
        &corpus.digest(),
        state.recipe_id.as_deref().unwrap(),
        DIMENSIONS,
        &Control::unbounded(),
    )
    .unwrap();
    let expected = corpus
        .engine()
        .semantic_current_mapping(
            &Control::unbounded(),
            &corpus.digest(),
            state.recipe_id.as_deref().unwrap(),
        )
        .unwrap();
    assert_eq!(actual.labels, expected.units);
    assert_eq!(
        actual.manifest.source_revision,
        expected.scope.source_revision
    );
    assert_eq!(
        actual
            .manifest
            .coverage_stats
            .as_ref()
            .unwrap()
            .cached_current_units,
        expected.units.values().map(|u| u.len() as u64).sum::<u64>()
    );
    assert!(
        fault::reached(MAPPING_FULL_PAGE) > 0,
        "audit remains independent"
    );
}

#[test]
fn publication_checkpoint_crash_child() {
    let Ok(store) = std::env::var("FOUNDRY_MAPPING_CRASH_STORE") else {
        return;
    };
    let point = std::env::var("FOUNDRY_MAPPING_CRASH_POINT").unwrap();
    let engine = Engine::open_existing(std::path::Path::new(&store)).unwrap();
    fault::arm(&point, 0, Action::Abort);
    engine
        .semantic_publish_pending(&Control::unbounded())
        .unwrap();
    panic!("crash point was not reached");
}

#[test]
fn publication_checkpoint_and_manifest_crashes_replay_without_losing_changed_work() {
    use context_foundry::neural::fault_names::{
        MAPPING_CHECKPOINT_BEFORE_COMMIT, PUBLISH_AFTER_MANIFEST, PUBLISH_BEFORE_MANIFEST,
    };
    for point in [
        MAPPING_CHECKPOINT_BEFORE_COMMIT,
        PUBLISH_BEFORE_MANIFEST,
        PUBLISH_AFTER_MANIFEST,
    ] {
        let mut corpus = Corpus::new(&[]);
        let body = corpus.source(CODE_PATH);
        accept_fixture_edit(&corpus, CODE_PATH, &format!("\n{body}"));
        let calls = corpus.probe.document_calls();
        corpus.engine = None;
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "publication_checkpoint_crash_child",
                "--nocapture",
            ])
            .env("FOUNDRY_MAPPING_CRASH_STORE", &corpus.store)
            .env("FOUNDRY_MAPPING_CRASH_POINT", point)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .unwrap();
        assert!(!status.success(), "child must crash at {point}");
        #[cfg(unix)]
        {
            use std::os::unix::process::ExitStatusExt;
            assert_eq!(status.signal(), Some(libc::SIGABRT));
        }
        corpus.engine = Some(Engine::open_existing(&corpus.store).unwrap());
        corpus
            .engine()
            .semantic_publish_pending(&Control::unbounded())
            .unwrap();
        let state = corpus.engine().semantic_state().unwrap().unwrap();
        let store = context_foundry::neural::anchor::Dir::open_path(&corpus.store).unwrap();
        let actual = index::validate_generation_with(
            &store,
            &corpus.digest(),
            state.recipe_id.as_deref().unwrap(),
            DIMENSIONS,
            &Control::unbounded(),
        )
        .unwrap();
        let expected = corpus
            .engine()
            .semantic_current_mapping(
                &Control::unbounded(),
                &corpus.digest(),
                state.recipe_id.as_deref().unwrap(),
            )
            .unwrap();
        assert_eq!(actual.labels, expected.units, "replay at {point}");
        assert_eq!(corpus.probe.document_calls(), calls);
    }
}

#[test]
fn restarted_partial_mapping_refreshes_shared_cache_keys_without_a_partition_census() {
    use context_foundry::neural::fault_names::{MAPPING_FULL_PAGE, PARTITION_READ};
    let mut corpus = Corpus::new(&[]);
    let state = corpus.engine().semantic_state().unwrap().unwrap();
    let key = "ef".repeat(32);
    for path in ["late-a.md", "late-b.md"] {
        let e = corpus.engine();
        e.replace_source(path, "late shared body\n").unwrap();
        e.semantic_record_partition(
            path,
            &e.source(path).unwrap().unwrap().hash,
            state.recipe_id.as_deref().unwrap(),
            &corpus.digest(),
            None,
            &|body| {
                Ok(vec![context_foundry::neural::cache::PartitionUnit {
                    kind: context_foundry::neural::cache::CardKind::Address,
                    start: 0,
                    end: body.len(),
                    input_key: key.clone(),
                }])
            },
        )
        .unwrap();
    }
    corpus
        .engine()
        .semantic_publish_pending(&Control::unbounded())
        .unwrap();
    corpus.engine = None;
    corpus.engine = Some(Engine::open_existing(&corpus.store).unwrap());
    let mut vector = vec![0.0; DIMENSIONS];
    vector[0] = 1.0;
    corpus
        .engine()
        .semantic_cache_commit(
            &[(key.clone(), vector)],
            &corpus.digest(),
            DIMENSIONS,
            DEFAULT_CACHE_CAP_BYTES,
        )
        .unwrap();
    // Restart again after a cache-only commit; no source revision changed.
    corpus.engine = None;
    corpus.engine = Some(Engine::open_existing(&corpus.store).unwrap());
    fault::disarm_all();
    corpus
        .engine()
        .semantic_publish_pending(&Control::unbounded())
        .unwrap();
    assert_eq!(fault::reached(MAPPING_FULL_PAGE), 0);
    assert_eq!(fault::reached(PARTITION_READ), 2);
    let store = context_foundry::neural::anchor::Dir::open_path(&corpus.store).unwrap();
    let actual = index::validate_generation_with(
        &store,
        &corpus.digest(),
        state.recipe_id.as_deref().unwrap(),
        DIMENSIONS,
        &Control::unbounded(),
    )
    .unwrap();
    assert_eq!(actual.labels[&key].len(), 2);
    assert_eq!(actual.manifest.coverage, "complete");
    let expected = corpus
        .engine()
        .semantic_current_mapping(
            &Control::unbounded(),
            &corpus.digest(),
            state.recipe_id.as_deref().unwrap(),
        )
        .unwrap();
    assert_eq!(actual.labels, expected.units);
}

#[test]
fn schema_eleven_upgrade_preserves_cached_publication_and_is_atomic() {
    use redb::{MultimapTableDefinition, TableDefinition};
    let mut corpus = Corpus::new(&[]);
    let original = corpus.engine().source(CODE_PATH).unwrap().unwrap();
    corpus.engine = None;
    let db = redb::Database::open(corpus.store.join("knowledge.redb")).unwrap();
    let tx = db.begin_write().unwrap();
    tx.delete_table(TableDefinition::<&str, &str>::new("semantic_mapping_rows"))
        .unwrap();
    tx.delete_table(TableDefinition::<&str, u64>::new("semantic_mapping_work"))
        .unwrap();
    tx.delete_multimap_table(MultimapTableDefinition::<&str, &str>::new(
        "semantic_mapping_keys",
    ))
    .unwrap();
    let mut meta = tx
        .open_table(TableDefinition::<&str, &str>::new("meta"))
        .unwrap();
    meta.insert("schema", "11").unwrap();
    meta.remove("semantic_mapping_checkpoint_exact").unwrap();
    drop(meta);
    tx.commit().unwrap();
    drop(db);
    assert!(matches!(
        Engine::open_existing(&corpus.store),
        Err(context_foundry::FoundryError::UpgradeRequired { .. })
    ));
    fault::arm(
        context_foundry::fault::names::UPGRADE_BEFORE_COMMIT,
        0,
        Action::Fail("upgrade interrupted".into()),
    );
    assert!(
        Engine::upgrade_store(
            &corpus.store,
            context_foundry::SCHEMA_VERSION,
            &Control::unbounded()
        )
        .is_err()
    );
    fault::disarm_all();
    assert!(matches!(
        Engine::open_existing(&corpus.store),
        Err(context_foundry::FoundryError::UpgradeRequired { .. })
    ));
    Engine::upgrade_store(
        &corpus.store,
        context_foundry::SCHEMA_VERSION,
        &Control::unbounded(),
    )
    .unwrap();
    corpus.engine = Some(Engine::open_existing(&corpus.store).unwrap());
    assert_eq!(
        corpus.engine().source(CODE_PATH).unwrap().unwrap().hash,
        original.hash
    );
    assert!(matches!(
        corpus
            .engine()
            .semantic_publish_pending(&Control::unbounded())
            .unwrap(),
        index::Publication::Current(_)
    ));
    accept_fixture_edit(&corpus, "after-upgrade.rs", "pub fn upgraded() {}\n");
    corpus
        .engine()
        .semantic_publish_pending(&Control::unbounded())
        .unwrap();
    let calls = corpus.probe.document_calls();
    assert_eq!(corpus.prepare().document_calls, 0);
    assert_eq!(corpus.probe.document_calls(), calls);
}

#[test]
fn explicit_repair_reconstructs_missing_disposable_mapping_tables_from_cached_authority() {
    let mut corpus = Corpus::new(&[]);
    let source = corpus.engine().source(CODE_PATH).unwrap().unwrap().hash;
    let calls = corpus.probe.document_calls();
    corpus.engine = None;
    let db = redb::Database::open(corpus.store.join("knowledge.redb")).unwrap();
    let tx = db.begin_write().unwrap();
    tx.delete_table(redb::TableDefinition::<&str, &str>::new(
        "semantic_mapping_rows",
    ))
    .unwrap();
    tx.commit().unwrap();
    drop(db);
    assert!(
        Engine::open_existing(&corpus.store).is_err(),
        "ordinary open does not invent a missing lookup"
    );
    Engine::repair_index(&corpus.store, &Control::unbounded()).unwrap();
    corpus.engine = Some(Engine::open_existing(&corpus.store).unwrap());
    assert_eq!(
        corpus.engine().source(CODE_PATH).unwrap().unwrap().hash,
        source
    );
    assert!(matches!(
        corpus
            .engine()
            .semantic_publish_pending(&Control::unbounded())
            .unwrap(),
        index::Publication::Current(_)
    ));
    assert_eq!(corpus.probe.document_calls(), calls);
}

#[test]
fn offline_cli_source_commit_preserves_incremental_publication_obligations() {
    use context_foundry::neural::fault_names::{MAPPING_FULL_PAGE, PARTITION_READ};
    let mut corpus = Corpus::new(&[]);
    std::fs::write(
        corpus.root.join(CODE_PATH),
        "pub fn offline_lexical_change() {}\n",
    )
    .unwrap();
    corpus.engine = None;
    let result = std::process::Command::new(env!("CARGO_BIN_EXE_foundry"))
        .arg("--store")
        .arg(&corpus.store)
        .arg("index")
        .arg(&corpus.root)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    corpus.engine = Some(Engine::open_existing(&corpus.store).unwrap());
    fault::disarm_all();
    corpus
        .engine()
        .semantic_publish_pending(&Control::unbounded())
        .unwrap();
    assert_eq!(fault::reached(MAPPING_FULL_PAGE), 0);
    assert_eq!(fault::reached(PARTITION_READ), 1);
    let status = corpus
        .engine()
        .semantic_status(&Control::unbounded())
        .unwrap();
    assert!(
        status.unpartitioned_sources > 0,
        "offline source requires new partition work"
    );
}
