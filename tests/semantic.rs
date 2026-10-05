//! 005 T001/T002: real SCIP references with scoped, freshness-checked
//! publication. The primary evidence is the committed rust-analyzer
//! 2026-08-31 artifact under `tests/fixtures/semantic/` (produced in a jail;
//! see `producer.json`); the constructed variants below are built with the
//! scip crate's own types and exercise what that artifact cannot: typed
//! ranges, malformed positions, other encodings, caps and hubs.
//!
//! Tests assert codes, counts, byte ranges and line forms, not message prose.
use context_foundry::fault::{self, Action, names};
use context_foundry::graph::{
    Coverage, EdgeEnd, OccurrenceKind, ReferencesOutcome, ReferencesRequest, ReferencesSeed,
    SnapshotState, TargetResolution, edge_id, occurrence_id, symbol_id,
};
use context_foundry::scip::{ImportLimits, ImportReport, code};
use context_foundry::store::SourceHandle;
use context_foundry::testkit::{self, V2Kind, parse_v2};
use context_foundry::{Control, Engine, FResult, FoundryError, digest, response};
use protobuf::{EnumOrUnknown, Message};
use redb::ReadableTable as _;
use scip::types::occurrence::{Typed_enclosing_range, Typed_range};
use scip::types::{Document, Index, MultiLineRange, Occurrence, PositionEncoding, SingleLineRange};
use serde_json::{Value, json};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const BIN: &str = env!("CARGO_BIN_EXE_foundry");
const FAULTS_BIN: &str = env!("CARGO_BIN_EXE_foundry-faults");
const RA: &str = "rust-analyzer";
const ALPHA: &str = "rust-analyzer cargo toy 0.1.0 alpha().";

fn fixture_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/semantic")
}

fn read_json(path: &Path) -> Value {
    serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap()
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
// A store bound to a temp workspace, plus the manifest/artifact plumbing.
// ---------------------------------------------------------------------------

struct World {
    dir: tempfile::TempDir,
    root: PathBuf,
    store: PathBuf,
    engine: Option<Engine>,
    counter: std::cell::Cell<u32>,
    /// Paths this world indexed, for manifest inputs.
    known: std::cell::RefCell<BTreeSet<String>>,
}

impl World {
    fn blank() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("ws");
        std::fs::create_dir(&root).unwrap();
        let store = dir.path().join("store");
        let engine = Engine::initialize(&store, &root).unwrap();
        World {
            dir,
            root,
            store,
            engine: Some(engine),
            counter: std::cell::Cell::new(0),
            known: std::cell::RefCell::new(BTreeSet::new()),
        }
    }

    /// Sources inserted directly (no filesystem scan).
    fn with_sources(sources: &[(&str, &str)]) -> Self {
        let world = Self::blank();
        for (path, body) in sources {
            world.set_source(path, body);
        }
        world
    }

    /// The committed fixture workspace copied into a temp workspace and
    /// indexed by the real scanner.
    fn fixture() -> Self {
        let mut world = Self::blank();
        copy_tree(&fixture_dir().join("workspace"), &world.root);
        let root = world.root.clone();
        let report = world
            .engine
            .as_mut()
            .unwrap()
            .index(&root, &Control::unbounded())
            .unwrap();
        assert!(!report.partial, "{report:?}");
        let mut paths = Vec::new();
        collect_files(&world.root, &world.root, &mut paths);
        world
            .known
            .borrow_mut()
            .extend(paths.into_iter().map(|(path, _)| path));
        world
    }

    /// Add or replace one source directly (no filesystem scan).
    fn set_source(&self, path: &str, body: &str) -> bool {
        self.known.borrow_mut().insert(path.to_owned());
        self.engine().replace_source(path, body).unwrap()
    }

    fn remove_source(&self, path: &str) {
        self.known.borrow_mut().remove(path);
        self.engine().delete_source(path).unwrap();
    }

    fn engine(&self) -> &Engine {
        self.engine.as_ref().expect("the engine is open")
    }

    fn close(&mut self) {
        self.engine = None;
    }

    fn reopen(&mut self) {
        self.engine = None;
        self.engine = Some(Engine::open_existing(&self.store).unwrap());
    }

    fn fresh_name(&self, stem: &str) -> PathBuf {
        let n = self.counter.get() + 1;
        self.counter.set(n);
        let dir = self.dir.path().join("imports");
        std::fs::create_dir_all(&dir).unwrap();
        dir.join(format!("{stem}-{n}"))
    }

    /// Every indexed source as `(path, sha256)`, sorted by path.
    fn inputs(&self) -> Vec<(String, String)> {
        self.known
            .borrow()
            .iter()
            .filter_map(|path| {
                self.engine()
                    .source(path)
                    .unwrap()
                    .map(|meta| (path.clone(), meta.hash))
            })
            .collect()
    }

    /// A manifest bound to this store's current workspace and revision.
    fn manifest(&self, producer: &str, tag: &str, config: &str, artifact: &[u8]) -> Value {
        let inputs: Vec<Value> = self
            .inputs()
            .into_iter()
            .map(|(path, sha256)| json!({"path": path, "sha256": sha256}))
            .collect();
        json!({
            "v": 1,
            "workspace_id": self.engine().workspace_id().unwrap(),
            "source_revision": self.engine().source_revision().unwrap(),
            "producer": {
                "name": producer,
                "release_tag": tag,
                "commit": "f8996691e991a4dc3c6f135e0fc04fc5561e4e9a",
                "version_output": "test-producer 1.0",
                "binary_sha256": digest(b"test-producer-binary"),
            },
            "invocation": "test-producer scip <snapshot> --output index.scip",
            "config": config,
            "artifact_sha256": digest(artifact),
            "inputs": inputs,
        })
    }

    fn write_pair(&self, manifest: &Value, artifact: &[u8]) -> (PathBuf, PathBuf) {
        let index = self.fresh_name("index");
        let snapshot = self.fresh_name("manifest");
        std::fs::write(&index, artifact).unwrap();
        std::fs::write(&snapshot, serde_json::to_vec(manifest).unwrap()).unwrap();
        (index, snapshot)
    }

    fn import_manifest(&self, manifest: &Value, artifact: &[u8]) -> FResult<ImportReport> {
        let (index, snapshot) = self.write_pair(manifest, artifact);
        self.engine()
            .import_scip(&index, &snapshot, &Control::unbounded())
    }

    /// Import `artifact` as `producer` with a manifest built for the current
    /// store state.
    fn import(&self, producer: &str, config: &str, artifact: &[u8]) -> FResult<ImportReport> {
        let manifest = self.manifest(producer, "2026-08-31", config, artifact);
        self.import_manifest(&manifest, artifact)
    }

    fn import_ok(&self, producer: &str, config: &str, artifact: &[u8]) -> ImportReport {
        self.import(producer, config, artifact).unwrap()
    }

    fn references(
        &self,
        seed: ReferencesSeed,
        limit: usize,
        after: Option<&str>,
    ) -> FResult<ReferencesOutcome> {
        self.engine().references(&ReferencesRequest {
            seed,
            limit,
            after: after.map(str::to_owned),
        })
    }

    fn by_symbol(&self, symbol_id: &str) -> ReferencesOutcome {
        self.references(
            ReferencesSeed::SymbolId(symbol_id[..16].to_owned()),
            64,
            None,
        )
        .unwrap()
    }

    fn handle_of(&self, path: &str, start: u64, end: u64) -> String {
        let meta = self.engine().source(path).unwrap().unwrap();
        SourceHandle {
            workspace_id: self.engine().workspace_id().unwrap(),
            path: path.to_owned(),
            sha256: meta.hash,
            start,
            end,
        }
        .to_v2()
    }

    fn whole_file_handle(&self, path: &str) -> String {
        let meta = self.engine().source(path).unwrap().unwrap();
        self.handle_of(path, 0, meta.bytes as u64)
    }

    fn text(&self, outcome: &ReferencesOutcome, tokens: usize) -> String {
        response::pack_references(
            outcome,
            response::Budget::request(tokens),
            &response::stdout_bytes,
        )
        .unwrap()
        .text
    }
}

fn collect_files(root: &Path, dir: &Path, out: &mut Vec<(String, String)>) {
    for entry in std::fs::read_dir(dir).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        if entry.file_type().unwrap().is_dir() {
            collect_files(root, &path, out);
        } else {
            let relative = path
                .strip_prefix(root)
                .unwrap()
                .to_str()
                .unwrap()
                .to_owned();
            out.push((relative, digest(&std::fs::read(&path).unwrap())));
        }
    }
}

// ---------------------------------------------------------------------------
// Constructed SCIP artifacts.
// ---------------------------------------------------------------------------

fn occ(range: &[i32], symbol: &str, roles: i32) -> Occurrence {
    let mut occurrence = Occurrence::new();
    occurrence.range = range.to_vec();
    occurrence.symbol = symbol.to_owned();
    occurrence.symbol_roles = roles;
    occurrence
}

fn doc_with(path: &str, encoding: PositionEncoding, occurrences: Vec<Occurrence>) -> Document {
    let mut document = Document::new();
    document.relative_path = path.to_owned();
    document.language = "rust".to_owned();
    document.position_encoding = EnumOrUnknown::new(encoding);
    document.occurrences = occurrences;
    document
}

fn doc(path: &str, occurrences: Vec<Occurrence>) -> Document {
    doc_with(
        path,
        PositionEncoding::UTF8CodeUnitOffsetFromLineStart,
        occurrences,
    )
}

fn artifact(documents: Vec<Document>) -> Vec<u8> {
    let mut index = Index::new();
    index.documents = documents;
    index.write_to_bytes().unwrap()
}

const DEF: i32 = 1;
const REF: i32 = 0;

/// `src/lib.rs`: `alpha` defined on line 0, called on lines 2 and 3.
const LIB: &str = "pub fn alpha() {}\npub fn beta() {\n    alpha();\n    alpha();\n}\n";
/// `src/other.rs`: one qualified call on line 1.
const OTHER: &str = "pub fn gamma() {\n    crate::alpha();\n}\n";

/// The toy world: `alpha` is defined in `src/lib.rs` and referenced three
/// times (twice there, once in `src/other.rs`).
fn toy() -> World {
    World::with_sources(&[("src/lib.rs", LIB), ("src/other.rs", OTHER)])
}

fn toy_artifact() -> Vec<u8> {
    artifact(vec![
        doc(
            "src/lib.rs",
            vec![
                occ(&[0, 7, 12], ALPHA, DEF),
                occ(&[2, 4, 9], ALPHA, REF),
                occ(&[3, 4, 9], ALPHA, REF),
            ],
        ),
        doc("src/other.rs", vec![occ(&[1, 11, 16], ALPHA, REF)]),
    ])
}

fn alpha_id() -> String {
    symbol_id(RA, "src/lib.rs", ALPHA)
}

// ---------------------------------------------------------------------------
// IDs
// ---------------------------------------------------------------------------

#[test]
fn symbol_occurrence_and_edge_ids_follow_the_spec_formulas() {
    let sha = |text: &str| digest(text.as_bytes());
    // Global: [producer_namespace,"global",SCIP_symbol]
    assert_eq!(
        symbol_id(
            RA,
            "src/a.rs",
            "rust-analyzer cargo f 0.1.0 a/parse_record()."
        ),
        sha(r#"["rust-analyzer","global","rust-analyzer cargo f 0.1.0 a/parse_record()."]"#)
    );
    // Local: [producer_namespace,"local",document_path,SCIP_symbol]; the same
    // spelling in two documents has two ids, and a global id ignores the path.
    assert_eq!(
        symbol_id(RA, "src/a.rs", "local 0"),
        sha(r#"["rust-analyzer","local","src/a.rs","local 0"]"#)
    );
    assert_ne!(
        symbol_id(RA, "src/a.rs", "local 0"),
        symbol_id(RA, "src/b.rs", "local 0")
    );
    assert_eq!(
        symbol_id(RA, "src/a.rs", ALPHA),
        symbol_id(RA, "src/zzz.rs", ALPHA)
    );
    // The namespace separates producers.
    assert_ne!(symbol_id(RA, "p", ALPHA), symbol_id("other", "p", ALPHA));
    // JSON escaping of the symbol text is part of the preimage.
    assert_eq!(
        symbol_id("p", "d", "quo\"te\\ é"),
        sha("[\"p\",\"global\",\"quo\\\"te\\\\ é\"]")
    );
    let id = symbol_id(RA, "src/a.rs", ALPHA);
    assert_eq!(
        occurrence_id(
            RA,
            OccurrenceKind::Reference,
            "src/u.rs",
            &"ab".repeat(32),
            58,
            70,
            &id
        ),
        sha(&format!(
            r#"["rust-analyzer","reference","src/u.rs","{}",58,70,"{id}"]"#,
            "ab".repeat(32)
        ))
    );
    assert_eq!(
        occurrence_id(RA, OccurrenceKind::Definition, "p", "h", 1, 2, "s"),
        sha(r#"["rust-analyzer","definition","p","h",1,2,"s"]"#)
    );
    assert_eq!(
        edge_id(
            RA,
            "references",
            EdgeEnd {
                identity: "src/u.rs",
                hash: "H1",
                start: 58,
                end: 70
            },
            EdgeEnd {
                identity: "src/a.rs",
                hash: "H2",
                start: 27,
                end: 39
            },
        ),
        sha(r#"["rust-analyzer","references","src/u.rs","H1",58,70,"src/a.rs","H2",27,39]"#)
    );
}

// ---------------------------------------------------------------------------
// The real rust-analyzer artifact.
// ---------------------------------------------------------------------------

struct Expected {
    a_definition: (String, u64, u64),
    a_references: Vec<(String, u64, u64, u64)>,
    b_reference_path: String,
}

fn expected() -> Expected {
    let json = read_json(&fixture_dir().join("expected.json"));
    let range = |v: &Value| {
        (
            v["path"].as_str().unwrap().to_owned(),
            v["start"].as_u64().unwrap(),
            v["end"].as_u64().unwrap(),
            v["line"].as_u64().unwrap() + 1, // expected.json lines are zero-based
        )
    };
    let definition = range(&json["definitions"]["a"]);
    Expected {
        a_definition: (definition.0, definition.1, definition.2),
        a_references: json["references_to_a"]
            .as_array()
            .unwrap()
            .iter()
            .map(range)
            .collect(),
        b_reference_path: json["references_to_b"][0]["path"]
            .as_str()
            .unwrap()
            .to_owned(),
    }
}

const A_SYMBOL: &str = "rust-analyzer cargo semantic_fixture 0.1.0 a/parse_record().";
const B_SYMBOL: &str = "rust-analyzer cargo semantic_fixture 0.1.0 b/parse_record().";

/// The fixture's own manifest facts (producer, invocation, config) around the
/// given workspace identity, revision and inputs, with the artifact unchanged.
fn fixture_manifest_for(
    workspace_id: &str,
    source_revision: u64,
    inputs: Vec<(String, String)>,
    artifact_bytes: &[u8],
) -> Value {
    let producer = read_json(&fixture_dir().join("producer.json"));
    let p = &producer["producer"];
    json!({
        "v": 1,
        "workspace_id": workspace_id,
        "source_revision": source_revision,
        "producer": {
            "name": p["name"], "release_tag": p["release_tag"], "commit": p["commit"],
            "version_output": p["version_output"], "binary_sha256": p["binary_sha256"],
        },
        "invocation": producer["invocation"],
        "config": producer["config"],
        "artifact_sha256": digest(artifact_bytes),
        "inputs": inputs.into_iter()
            .map(|(path, sha256)| json!({"path": path, "sha256": sha256}))
            .collect::<Vec<_>>(),
    })
}

/// The manifest for this temp workspace as indexed now.
fn fixture_manifest(world: &World, artifact_bytes: &[u8]) -> Value {
    fixture_manifest_for(
        &world.engine().workspace_id().unwrap(),
        world.engine().source_revision().unwrap(),
        world.inputs(),
        artifact_bytes,
    )
}

fn fixture_artifact() -> Vec<u8> {
    std::fs::read(fixture_dir().join("index.scip")).unwrap()
}

#[test]
fn the_fixture_manifest_inputs_equal_the_producer_record() {
    // The temp workspace is byte-identical to the jailed snapshot, so the
    // run-time manifest inputs equal producer.json's recorded hashes.
    let world = World::fixture();
    let producer = read_json(&fixture_dir().join("producer.json"));
    let recorded: Vec<(String, String)> = producer["snapshot"]["inputs"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| {
            (
                v["path"].as_str().unwrap().to_owned(),
                v["sha256"].as_str().unwrap().to_owned(),
            )
        })
        .collect();
    assert_eq!(world.inputs(), recorded);
    assert_eq!(
        digest(&fixture_artifact()),
        producer["artifact"]["sha256"].as_str().unwrap()
    );
}

#[test]
fn real_artifact_returns_both_a_uses_and_the_pointer_and_excludes_b() {
    let world = World::fixture();
    let bytes = fixture_artifact();
    let manifest = fixture_manifest(&world, &bytes);
    let report = world.import_manifest(&manifest, &bytes).unwrap();
    assert!(
        report.complete && report.selected && report.failed == 0,
        "{report:?}"
    );
    assert_eq!(report.documents, 7);
    // use_one/use_two/use_b/pointer resolve everything; lib.rs, a.rs and b.rs
    // reference std (alloc/core), which is external and counted unresolved.
    assert_eq!(
        (report.completed, report.unresolved, report.accepted_empty),
        (4, 3, 0)
    );
    assert_eq!(
        (report.outside_scope, report.unknown, report.retired),
        (2, 0, 0)
    );

    let expected = expected();
    let a_id = symbol_id(RA, &expected.a_definition.0, A_SYMBOL);
    let outcome = world.by_symbol(&a_id);
    assert_eq!(outcome.symbol_id.as_deref(), Some(a_id.as_str()));
    assert_eq!(outcome.coverage, Coverage::Complete);
    assert_eq!(outcome.target, Some(TargetResolution::Unique));
    assert_eq!(outcome.unresolved, 0);
    assert_eq!(outcome.stale, 0);
    // One definition lookup record plus the three references.
    assert_eq!(outcome.examined, 4);
    assert!(!outcome.more && !outcome.candidates_full && outcome.resume.is_none());
    assert_eq!(outcome.definitions.len(), 1);
    let definition = &outcome.definitions[0];
    assert_eq!(
        (definition.path.as_str(), definition.start, definition.end),
        (
            expected.a_definition.0.as_str(),
            expected.a_definition.1,
            expected.a_definition.2
        )
    );
    // Exactly the expected reference ranges, in path order, and never b's.
    let mut want = expected.a_references.clone();
    want.sort();
    let got: Vec<(String, u64, u64, u64)> = outcome
        .items
        .iter()
        .map(|i| (i.path.clone(), i.start, i.end, i.line))
        .collect();
    assert_eq!(got, want);
    assert_eq!(got.len(), 3);
    assert!(
        got.iter()
            .all(|(path, ..)| *path != expected.b_reference_path)
    );
    // The Unicode before the use_two occurrence: byte start 108 (UTF-8 column
    // 63), not the UTF-16 column 53 the artifact correctly does not use.
    assert!(got.contains(&("src/use_two.rs".to_owned(), 108, 120, 2)));
    // Each item is a unique-target reference with an edge id and an occurrence
    // id computed from the stored identities.
    for item in &outcome.items {
        let target = &outcome.definitions[0];
        assert_eq!(
            item.edge_id.as_deref(),
            Some(
                edge_id(
                    RA,
                    "references",
                    EdgeEnd {
                        identity: &item.path,
                        hash: &item.sha256,
                        start: item.start,
                        end: item.end
                    },
                    EdgeEnd {
                        identity: &target.path,
                        hash: &target.sha256,
                        start: target.start,
                        end: target.end
                    },
                )
                .as_str()
            )
        );
        assert_eq!(
            item.occurrence_id,
            occurrence_id(
                RA,
                OccurrenceKind::Reference,
                &item.path,
                &item.sha256,
                item.start,
                item.end,
                &a_id
            )
        );
    }

    // The wire: header with examined/coverage, one line per reference naming
    // the enclosing delivery unit; the pointer is a reference, never a call.
    let text = world.text(&outcome, 1024);
    let parsed = parse_v2(&text).unwrap();
    let revision = world.engine().source_revision().unwrap();
    assert_eq!(
        parsed.header,
        vec![
            "foundry references".to_owned(),
            format!("r{revision}"),
            "budget:1024".to_owned(),
            "examined:4".to_owned(),
            "coverage:complete".to_owned(),
        ]
    );
    assert!(parsed.next.is_none());
    let lines: Vec<(&str, &str)> = parsed
        .items
        .iter()
        .map(|i| (i.lines.as_deref().unwrap(), i.label.as_deref().unwrap()))
        .collect();
    assert_eq!(
        lines,
        vec![
            ("L2", "fn indirect"),
            ("L2", "fn first"),
            ("L2", "fn second")
        ]
    );
    assert!(parsed.items.iter().all(|i| i.kind == V2Kind::Reference));
    assert!(!text.contains("calls"), "{text}");
    // Every returned handle retrieves the whole enclosing unit, and that unit
    // contains the expected reference range.
    for (item, wire) in outcome.items.iter().zip(&parsed.items) {
        let retrieved = world.engine().retrieve(&wire.handle, None, 2048).unwrap();
        let body = String::from_utf8(retrieved.span.clone()).unwrap();
        assert!(body.starts_with("pub fn "), "{body}");
        assert!(body.contains("parse_record") && body.ends_with('}'));
        assert!(item.unit.start <= item.start && item.end <= item.unit.end);
    }

    // A position seed on the function pointer's identifier resolves to the
    // same symbol and the same references.
    let pointer_handle = world.whole_file_handle("src/pointer.rs");
    let by_position = world
        .references(
            ReferencesSeed::Position {
                handle: pointer_handle.clone(),
                byte_offset: 105,
            },
            64,
            None,
        )
        .unwrap();
    assert_eq!(by_position.symbol_id.as_deref(), Some(a_id.as_str()));
    assert_eq!(by_position.items.len(), 3);
    assert!(
        by_position.examined > outcome.examined,
        "the seed scan is counted in the shared window"
    );
    // b stays separate: its own symbol, its own single reference.
    let b_outcome = world.by_symbol(&symbol_id(RA, "src/b.rs", B_SYMBOL));
    assert_eq!(b_outcome.items.len(), 1);
    assert_eq!(b_outcome.items[0].path, "src/use_b.rs");
    assert_ne!(a_id, symbol_id(RA, "src/b.rs", B_SYMBOL));

    // The producer record names the selected snapshot and the latest report.
    let producers = world.engine().compiler_producers().unwrap();
    assert_eq!(producers.len(), 1);
    let (namespace, row) = &producers[0];
    assert_eq!(namespace, RA);
    let selected = row.selected.as_ref().unwrap();
    assert_eq!(selected.state, SnapshotState::Complete);
    assert_eq!(selected.tuple.artifact_sha256, digest(&bytes));
    assert_eq!(selected.tuple.source_revision, revision);
    assert_eq!(selected.tuple.producer.release_tag, "2026-08-31");
    assert_eq!(
        row.latest.as_ref().unwrap().snapshot_id,
        selected.tuple.snapshot_id
    );
}

// ---------------------------------------------------------------------------
// Shared helpers for the constructed variants.
// ---------------------------------------------------------------------------

impl World {
    /// The raw tables of the closed store (the engine is reopened after).
    fn raw(&mut self) -> testkit::Snapshot {
        self.close();
        let snapshot = testkit::snapshot(&self.store);
        self.reopen();
        snapshot
    }

    fn selected(&self, namespace: &str) -> Option<context_foundry::graph::SelectedSnapshot> {
        self.engine()
            .compiler_producers()
            .unwrap()
            .into_iter()
            .find(|(name, _)| name == namespace)
            .and_then(|(_, row)| row.selected)
    }

    fn latest(&self, namespace: &str) -> Option<ImportReport> {
        self.engine()
            .compiler_producers()
            .unwrap()
            .into_iter()
            .find(|(name, _)| name == namespace)
            .and_then(|(_, row)| row.latest)
    }

    /// `(path, start)` of every reference of `symbol`, in wire order.
    fn starts(&self, symbol: &str) -> Vec<(String, u64)> {
        self.by_symbol(symbol)
            .items
            .iter()
            .map(|item| (item.path.clone(), item.start))
            .collect()
    }
}

/// A snapshot without the producer rows (their latest report carries
/// timings), for "nothing changed" comparisons.
fn without_producers(mut snapshot: testkit::Snapshot) -> testkit::Snapshot {
    snapshot.remove("compiler_producers");
    snapshot
}

fn typed_single(line: i32, start: i32, end: i32) -> Option<Typed_range> {
    Some(Typed_range::SingleLineRange(SingleLineRange {
        line,
        start_character: start,
        end_character: end,
        ..Default::default()
    }))
}

// ---------------------------------------------------------------------------
// T001: ranges, encodings, bindings
// ---------------------------------------------------------------------------

#[test]
fn typed_ranges_win_over_deprecated_ranges_when_both_are_present() {
    let world = toy();
    // Each reference carries a typed range AND a deprecated range that points
    // elsewhere or is malformed: only the typed one is read.
    let mut first = occ(&[3, 4, 9], ALPHA, REF);
    first.typed_range = typed_single(2, 4, 9);
    let mut second = occ(&[99, 0, 1], ALPHA, REF);
    second.typed_range = Some(Typed_range::MultiLineRange(MultiLineRange {
        start_line: 3,
        start_character: 4,
        end_line: 3,
        end_character: 9,
        ..Default::default()
    }));
    let mut definition = occ(&[0, 7, 12], ALPHA, DEF);
    definition.enclosing_range = vec![-5, 0];
    definition.typed_enclosing_range = Some(Typed_enclosing_range::SingleLineEnclosingRange(
        SingleLineRange {
            line: 0,
            start_character: 0,
            end_character: 17,
            ..Default::default()
        },
    ));
    // An empty document is accepted-empty.
    let bytes = artifact(vec![
        doc("src/lib.rs", vec![definition, first, second]),
        doc("src/other.rs", vec![]),
    ]);
    let report = world.import_ok(RA, "typed", &bytes);
    assert!(report.complete && report.failed == 0, "{report:?}");
    assert_eq!(report.coverage, "complete", "every source resolved");
    assert_eq!((report.completed, report.accepted_empty), (1, 1));
    // Line 2 col 4..9 is bytes 38..43; line 3 col 4..9 is bytes 51..56.
    assert_eq!(
        world.starts(&alpha_id()),
        vec![("src/lib.rs".to_owned(), 38), ("src/lib.rs".to_owned(), 51)]
    );
}

fn malformed_cases() -> Vec<(&'static str, &'static str, Occurrence)> {
    const PLAIN: &str = "pub fn f() {}\n";
    let mut bad_enclosing = occ(&[0, 7, 8], ALPHA, DEF);
    bad_enclosing.enclosing_range = vec![0, 0, 99];
    vec![
        ("src/m0.rs", PLAIN, occ(&[1, 2], ALPHA, REF)),
        ("src/m1.rs", PLAIN, occ(&[0, -1, 3], ALPHA, REF)),
        ("src/m2.rs", PLAIN, occ(&[9, 0, 1], ALPHA, REF)),
        ("src/m3.rs", PLAIN, occ(&[0, 5, 99], ALPHA, REF)),
        ("src/m4.rs", PLAIN, occ(&[0, 3, 3], ALPHA, REF)),
        ("src/m5.rs", PLAIN, occ(&[0, 5, 3], ALPHA, REF)),
        // 'é' occupies bytes 3..5: an end inside it splits the codepoint.
        ("src/m6.rs", "// é\n", occ(&[0, 4, 5], ALPHA, REF)),
        ("src/m7.rs", PLAIN, bad_enclosing),
    ]
}

#[test]
fn a_malformed_range_fails_its_document_by_name_and_keeps_the_scope() {
    let cases = malformed_cases();
    let mut sources: Vec<(&str, &str)> = cases.iter().map(|(p, b, _)| (*p, *b)).collect();
    sources.push(("src/ok.rs", "pub fn ok() {}\n"));
    let mut world = World::with_sources(&sources);
    let mut docs: Vec<Document> = cases
        .iter()
        .map(|(path, _, occurrence)| doc(path, vec![occurrence.clone()]))
        .collect();
    docs.push(doc(
        "src/ok.rs",
        vec![occ(&[0, 7, 9], "rust-analyzer cargo t 0 ok().", DEF)],
    ));
    let report = world.import_ok(RA, "malformed", &artifact(docs));
    // Eight failed by path, one committed; never counted accepted-empty.
    assert!(
        !report.complete && report.interrupted.is_none(),
        "{report:?}"
    );
    assert_eq!(report.failed, 8);
    assert_eq!((report.completed, report.accepted_empty), (1, 0));
    assert_eq!(report.failure_samples.len(), 8);
    for (sample, (path, _, _)) in report.failure_samples.iter().zip(&cases) {
        assert_eq!(sample.path, *path);
        assert_eq!(sample.code, code::INVALID_RANGE, "{sample:?}");
    }
    let selected = world.selected(RA).unwrap();
    assert_eq!(selected.state, SnapshotState::Partial);
    // A failed document leaves its scope unchanged: here, never created.
    let raw = world.raw();
    let scopes: Vec<&str> = raw["compiler_scopes"]
        .iter()
        .map(|(k, _)| k.as_str())
        .collect();
    assert_eq!(scopes, vec!["rust-analyzer\0src/ok.rs"]);
    // Samples are capped at 20 while the count stays exact.
    let many: Vec<(String, String)> = (0..25)
        .map(|i| (format!("src/many{i:02}.rs"), "pub fn f() {}\n".to_owned()))
        .collect();
    let more = World::with_sources(
        &many
            .iter()
            .map(|(p, b)| (p.as_str(), b.as_str()))
            .collect::<Vec<_>>(),
    );
    let docs: Vec<Document> = many
        .iter()
        .map(|(path, _)| doc(path, vec![occ(&[7], ALPHA, REF)]))
        .collect();
    let report = more.import_ok(RA, "many", &artifact(docs));
    assert_eq!(report.failed, 25);
    assert_eq!(report.failure_samples.len(), 20);
}

#[test]
fn a_failed_import_preserves_the_prior_scope_bytes() {
    let mut world = toy();
    world.import_ok(RA, "first", &toy_artifact());
    let before = world.raw();
    let malformed = artifact(vec![
        doc("src/lib.rs", vec![occ(&[0, 7, 99], ALPHA, DEF)]),
        doc("src/other.rs", vec![occ(&[1, 11, 16], ALPHA, REF)]),
    ]);
    let report = world.import_ok(RA, "second", &malformed);
    assert!(!report.complete);
    assert_eq!(report.failed, 1);
    assert_eq!(report.failure_samples[0].path, "src/lib.rs");
    let after = world.raw();
    // The failed scope's rows are byte-identical; only src/other.rs moved to
    // the new snapshot.
    let scope_rows = |snapshot: &testkit::Snapshot| -> Vec<(String, String)> {
        snapshot["compiler_occurrences"]
            .iter()
            .filter(|(k, _)| k.starts_with("rust-analyzer\0src/lib.rs\0"))
            .cloned()
            .collect()
    };
    assert_eq!(scope_rows(&before), scope_rows(&after));
    assert_eq!(scope_rows(&before).len(), 3);
    let scope_row = |snapshot: &testkit::Snapshot, path: &str| -> Value {
        let key = format!("rust-analyzer\0{path}");
        let raw = &snapshot["compiler_scopes"]
            .iter()
            .find(|(k, _)| *k == key)
            .unwrap()
            .1;
        serde_json::from_str(raw).unwrap()
    };
    assert_eq!(
        scope_row(&before, "src/lib.rs"),
        scope_row(&after, "src/lib.rs"),
        "the failed scope keeps its old snapshot"
    );
    assert_ne!(
        scope_row(&before, "src/other.rs")["snapshot_id"],
        scope_row(&after, "src/other.rs")["snapshot_id"]
    );
    // The old lib.rs scope is ineligible under the new snapshot: only the
    // other.rs reference answers, with the import named partial.
    let outcome = world.by_symbol(&alpha_id());
    assert_eq!(outcome.coverage, Coverage::Partial);
    assert_eq!(
        outcome
            .items
            .iter()
            .map(|i| i.path.as_str())
            .collect::<Vec<_>>(),
        vec!["src/other.rs"]
    );
}

#[test]
fn a_non_utf8_position_encoding_is_unsupported_encoding() {
    let cases: Vec<(&str, EnumOrUnknown<PositionEncoding>)> = vec![
        (
            "src/e0.rs",
            EnumOrUnknown::new(PositionEncoding::UTF16CodeUnitOffsetFromLineStart),
        ),
        (
            "src/e1.rs",
            EnumOrUnknown::new(PositionEncoding::UTF32CodeUnitOffsetFromLineStart),
        ),
        (
            "src/e2.rs",
            EnumOrUnknown::new(PositionEncoding::UnspecifiedPositionEncoding),
        ),
        ("src/e3.rs", EnumOrUnknown::from_i32(42)),
    ];
    let sources: Vec<(&str, &str)> = cases.iter().map(|(p, _)| (*p, "pub fn f() {}\n")).collect();
    let world = World::with_sources(&sources);
    let docs: Vec<Document> = cases
        .iter()
        .map(|(path, encoding)| {
            let mut document = doc(path, vec![occ(&[0, 7, 8], ALPHA, DEF)]);
            document.position_encoding = *encoding;
            document
        })
        .collect();
    let report = world.import_ok(RA, "encodings", &artifact(docs));
    assert_eq!(report.failed, 4);
    assert!(
        report
            .failure_samples
            .iter()
            .all(|s| s.code == code::UNSUPPORTED_ENCODING)
    );
    assert_eq!(
        report.completed + report.accepted_empty + report.unresolved,
        0
    );
    assert!(!report.complete);
    // Nothing from those documents was stored.
    let outcome = world.by_symbol_or_none(&alpha_id());
    assert!(outcome.is_none());
}

impl World {
    /// `None` when the prefix names no stored symbol.
    fn by_symbol_or_none(&self, symbol_id: &str) -> Option<ReferencesOutcome> {
        match self.references(
            ReferencesSeed::SymbolId(symbol_id[..16].to_owned()),
            64,
            None,
        ) {
            Ok(outcome) => Some(outcome),
            Err(e) if e.code() == code::SYMBOL_NOT_FOUND => None,
            Err(e) => panic!("{e:?}"),
        }
    }
}

// --- bindings and refusals -------------------------------------------------

impl World {
    fn import_files(
        &self,
        index: &Path,
        snapshot: &Path,
        limits: &ImportLimits,
    ) -> FResult<ImportReport> {
        self.engine()
            .import_scip_with(index, snapshot, &Control::unbounded(), limits)
    }

    /// Import with a tweaked manifest and assert the refusal changed no
    /// authoritative row and left the selection as it was (none, in a fresh
    /// world).
    fn refuse(&mut self, bytes: &[u8], tweak: impl FnOnce(&mut Value)) -> FoundryError {
        let before = without_producers(self.raw());
        let selected = self.selected(RA);
        let mut manifest = self.manifest(RA, "2026-08-31", "refused", bytes);
        tweak(&mut manifest);
        let error = self.import_manifest(&manifest, bytes).unwrap_err();
        assert_eq!(without_producers(self.raw()), before, "{error:?}");
        assert_eq!(self.selected(RA), selected, "{error:?}");
        error
    }
}

#[test]
fn wrong_hash_stale_revision_and_unbound_artifacts_each_name_an_error() {
    let mut world = toy();
    let bytes = toy_artifact();
    let revision = world.engine().source_revision().unwrap();
    // A copied artifact whose digest differs from the manifest's.
    let e = world.refuse(&bytes, |m| {
        m["artifact_sha256"] = json!(digest(b"another artifact"))
    });
    assert_eq!(e.code(), code::STALE_ARTIFACT);
    // A manifest produced at another source revision.
    let e = world.refuse(&bytes, |m| m["source_revision"] = json!(revision - 1));
    assert_eq!(e.code(), code::STALE_ARTIFACT);
    let e = world.refuse(&bytes, |m| m["source_revision"] = json!(revision + 1));
    assert_eq!(e.code(), code::STALE_ARTIFACT);
    // An input whose hash differs from the indexed source.
    let e = world.refuse(&bytes, |m| {
        m["inputs"][0]["sha256"] = json!(digest(b"edited"))
    });
    assert_eq!(e.code(), code::STALE_ARTIFACT);
    // An input that is not an indexed source at all.
    let e = world.refuse(&bytes, |m| {
        m["inputs"]
            .as_array_mut()
            .unwrap()
            .push(json!({"path": "src/zzz.rs", "sha256": digest(b"z")}));
    });
    assert_eq!(e.code(), code::STALE_ARTIFACT);
    // A manifest bound to another workspace.
    let e = world.refuse(&bytes, |m| {
        m["workspace_id"] = json!(digest(b"another root"))
    });
    assert_eq!(e.code(), code::UNBOUND_ARTIFACT);
    // A document the manifest does not list: the artifact is not bound.
    let e = world.refuse(&bytes, |m| {
        m["inputs"]
            .as_array_mut()
            .unwrap()
            .retain(|i| i["path"] != "src/other.rs");
    });
    assert_eq!(e.code(), code::UNBOUND_ARTIFACT);
    // The refusals above were named in the producer's latest report.
    let latest = world.latest(RA).unwrap();
    assert_eq!(
        latest.failure.as_ref().unwrap().code,
        code::UNBOUND_ARTIFACT
    );
    assert!(!latest.complete && !latest.selected);
    // With nothing selected every query answers `unavailable`.
    let outcome = world
        .references(
            ReferencesSeed::SymbolId(alpha_id()[..16].to_owned()),
            64,
            None,
        )
        .unwrap();
    assert_eq!(outcome.coverage, Coverage::Unavailable);
    assert!(outcome.items.is_empty());
}

#[test]
fn a_malformed_manifest_is_an_invalid_argument_and_a_missing_file_is_unavailable() {
    let mut world = toy();
    let bytes = toy_artifact();
    let wrong_version = world.refuse(&bytes, |m| m["v"] = json!(2));
    assert_eq!(wrong_version.code(), "invalid_argument");
    for tweak in [
        (|m: &mut Value| m["extra"] = json!(1)) as fn(&mut Value),
        |m| {
            m.as_object_mut().unwrap().remove("config");
        },
        |m| m["workspace_id"] = json!("ABC"),
        |m| m["artifact_sha256"] = json!("00"),
        |m| m["producer"]["name"] = json!(""),
        |m| m["producer"]["extra"] = json!(true),
        |m| m["source_revision"] = json!(-1),
        |m| m["inputs"][0]["path"] = json!("/abs"),
        |m| m["inputs"][0]["sha256"] = json!("nothex"),
        |m| m["inputs"][0]["extra"] = json!(0),
    ] {
        let e = world.refuse(&bytes, tweak);
        assert_eq!(e.code(), "invalid_argument", "{e:?}");
    }
    // Namespace and revision are 1..128 bytes.
    let ok = |name: &str, tag: &str| {
        let mut manifest = world.manifest(name, tag, "x", &bytes);
        manifest["producer"]["name"] = json!(name);
        world.import_manifest(&manifest, &bytes)
    };
    assert_eq!(
        ok(&"n".repeat(129), "t").unwrap_err().code(),
        "invalid_argument"
    );
    assert_eq!(
        ok("n", &"t".repeat(129)).unwrap_err().code(),
        "invalid_argument"
    );
    assert!(ok(&"n".repeat(128), &"t".repeat(128)).unwrap().selected);
    // Unsorted and duplicate inputs, trailing bytes after the JSON value.
    let e = world.refuse(&bytes, |m| m["inputs"].as_array_mut().unwrap().reverse());
    assert_eq!(e.code(), "invalid_argument");
    let (index, snapshot) =
        world.write_pair(&world.manifest(RA, "2026-08-31", "t", &bytes), &bytes);
    let mut text = std::fs::read(&snapshot).unwrap();
    text.extend_from_slice(b" trailing");
    std::fs::write(&snapshot, &text).unwrap();
    let e = world
        .engine()
        .import_scip(&index, &snapshot, &Control::unbounded())
        .unwrap_err();
    assert_eq!(e.code(), "invalid_argument");
    // Missing and non-regular files are artifact_unavailable (exit 2).
    let missing = world.fresh_name("missing");
    let e = world
        .engine()
        .import_scip(&missing, &snapshot, &Control::unbounded())
        .unwrap_err();
    assert_eq!((e.code(), e.exit_code()), ("artifact_unavailable", 2));
    let e = world
        .engine()
        .import_scip(&index, world.dir.path(), &Control::unbounded())
        .unwrap_err();
    assert_eq!(e.code(), "artifact_unavailable");
}

#[test]
fn duplicate_document_and_duplicate_input_paths_fail_before_selection() {
    let mut world = toy();
    let lib = || doc("src/lib.rs", vec![occ(&[0, 7, 12], ALPHA, DEF)]);
    // Same normalized path twice, even though both hash to the same source.
    for second in ["src/lib.rs", "./src/lib.rs"] {
        let bytes = artifact(vec![lib(), doc(second, vec![occ(&[0, 7, 12], ALPHA, DEF)])]);
        let e = world.refuse(&bytes, |_| {});
        assert_eq!(e.code(), code::DUPLICATE_DOCUMENT, "{second}");
    }
    // The same manifest path listed twice, adjacent or not, hashes matching.
    let bytes = toy_artifact();
    let e = world.refuse(&bytes, |m| {
        let inputs = m["inputs"].as_array_mut().unwrap();
        let first = inputs[0].clone();
        inputs.insert(1, first);
    });
    assert_eq!(e.code(), code::DUPLICATE_INPUT);
    let e = world.refuse(&bytes, |m| {
        let inputs = m["inputs"].as_array_mut().unwrap();
        let first = inputs[0].clone();
        inputs.push(first);
    });
    assert_eq!(e.code(), code::DUPLICATE_INPUT);
    // A prior selection is untouched by a refused later import.
    world.import_ok(RA, "good", &bytes);
    let selected = world.selected(RA).unwrap();
    let dup = artifact(vec![lib(), lib()]);
    let manifest = world.manifest(RA, "2026-08-31", "bad", &dup);
    assert_eq!(
        world.import_manifest(&manifest, &dup).unwrap_err().code(),
        code::DUPLICATE_DOCUMENT
    );
    assert_eq!(world.selected(RA).unwrap(), selected);
    let latest = world.latest(RA).unwrap();
    assert_eq!(latest.failure.unwrap().code, code::DUPLICATE_DOCUMENT);
    // The prior facts still answer, still complete.
    assert_eq!(world.by_symbol(&alpha_id()).coverage, Coverage::Complete);
}

#[test]
fn both_parsing_passes_read_the_frozen_copy_not_the_caller_pathname() {
    let world = toy();
    let bytes = toy_artifact();
    let manifest = world.manifest(RA, "2026-08-31", "frozen", &bytes);
    let (index, snapshot) = world.write_pair(&manifest, &bytes);
    let manifest_bytes = std::fs::read(&snapshot).unwrap();
    // After the copy, replace BOTH pathnames with different content: an
    // artifact without alpha, a manifest that would not parse.
    let (i, s) = (index.clone(), snapshot.clone());
    fault::arm(
        names::SCIP_AFTER_COPY,
        0,
        Action::Call(Box::new(move |_| {
            std::fs::write(&i, artifact(vec![doc("src/lib.rs", vec![])])).unwrap();
            std::fs::write(&s, b"{not json").unwrap();
        })),
    );
    let report = world
        .engine()
        .import_scip(&index, &snapshot, &Control::unbounded())
        .unwrap();
    fault::disarm_all();
    assert!(report.complete, "{report:?}");
    assert_eq!(report.artifact_sha256, digest(&bytes));
    assert_eq!(report.manifest_sha256, digest(&manifest_bytes));
    assert_eq!(
        report.copied_bytes,
        (bytes.len() + manifest_bytes.len()) as u64
    );
    assert_eq!(world.by_symbol(&alpha_id()).items.len(), 3);
    // Copy and scratch costs are recorded with the report.
    assert!(report.scratch_peak_bytes >= report.copied_bytes);
    assert!(report.timings.total_ms >= report.timings.copy_ms);
}

// --- size caps at limit and limit+1 ----------------------------------------

/// A manifest of exactly `size` bytes, padded through the `config` string.
fn manifest_of_size(world: &World, bytes: &[u8], size: usize) -> Vec<u8> {
    let mut manifest = world.manifest(RA, "2026-08-31", "", bytes);
    let base = serde_json::to_vec(&manifest).unwrap().len();
    manifest["config"] = Value::String("a".repeat(size - base));
    let out = serde_json::to_vec(&manifest).unwrap();
    assert_eq!(out.len(), size);
    out
}

#[test]
fn the_manifest_cap_is_64_mib_at_the_limit_and_over_it() {
    const CAP: usize = 64 * 1024 * 1024;
    let world = toy();
    let bytes = toy_artifact();
    let index = world.fresh_name("index");
    std::fs::write(&index, &bytes).unwrap();
    let limits = ImportLimits::default();
    assert_eq!(limits.manifest_bytes, CAP as u64);
    let at = world.fresh_name("manifest");
    std::fs::write(&at, manifest_of_size(&world, &bytes, CAP)).unwrap();
    let report = world.import_files(&index, &at, &limits).unwrap();
    assert!(report.complete, "{report:?}");
    let over = world.fresh_name("manifest");
    std::fs::write(&over, manifest_of_size(&world, &bytes, CAP + 1)).unwrap();
    let e = world.import_files(&index, &over, &limits).unwrap_err();
    assert_eq!(e.code(), code::MANIFEST_TOO_LARGE);
    // A manifest that grows past the cap while it is copied cannot be
    // retried: the copy stops, nothing is selected again.
    assert_eq!(world.selected(RA).unwrap().state, SnapshotState::Complete);
}

#[test]
fn the_artifact_cap_is_1_gib_and_scaled_caps_fail_at_limit_plus_one() {
    let world = toy();
    let bytes = toy_artifact();
    let manifest = world.manifest(RA, "2026-08-31", "cap", &bytes);
    let (index, snapshot) = world.write_pair(&manifest, &bytes);
    let defaults = ImportLimits::default();
    assert_eq!(
        (
            defaults.artifact_bytes,
            defaults.manifest_bytes,
            defaults.document_bytes,
            defaults.document_occurrences,
            defaults.symbol_bytes,
            defaults.scratch_bytes
        ),
        (1 << 30, 64 << 20, 8 << 20, 16_384, 1024, 4 << 30)
    );
    // Scaled to the artifact's own length: at the limit imports, one byte
    // over names artifact_too_large.
    let exact = ImportLimits {
        artifact_bytes: bytes.len() as u64,
        ..ImportLimits::default()
    };
    assert!(
        world
            .import_files(&index, &snapshot, &exact)
            .unwrap()
            .complete
    );
    let under = ImportLimits {
        artifact_bytes: bytes.len() as u64 - 1,
        ..ImportLimits::default()
    };
    let e = world.import_files(&index, &snapshot, &under).unwrap_err();
    assert_eq!(e.code(), code::ARTIFACT_TOO_LARGE);
    // The shipped bound: a sparse file one byte over 1 GiB is refused before
    // any bytes are copied.
    let big = world.fresh_name("sparse");
    let file = std::fs::File::create(&big).unwrap();
    file.set_len((1 << 30) + 1).unwrap();
    drop(file);
    let e = world
        .import_files(&big, &snapshot, &ImportLimits::default())
        .unwrap_err();
    assert_eq!(e.code(), code::ARTIFACT_TOO_LARGE);
}

/// A document whose serialized message is exactly `message_len` bytes.
fn document_of_size(path: &str, occurrences: Vec<Occurrence>, message_len: u64) -> Document {
    let mut document = doc(path, occurrences);
    let base = document.compute_size();
    let mut pad = message_len - base - 6;
    loop {
        document.text = "a".repeat(pad as usize);
        let size = document.compute_size();
        if size == message_len {
            return document;
        }
        pad = if size < message_len {
            pad + (message_len - size)
        } else {
            pad - (size - message_len)
        };
    }
}

#[test]
fn the_document_cap_is_8_mib_and_an_oversized_document_fails_by_path() {
    const CAP: u64 = 8 * 1024 * 1024;
    let world = toy();
    let at = document_of_size(
        "src/lib.rs",
        vec![occ(&[0, 7, 12], ALPHA, DEF), occ(&[2, 4, 9], ALPHA, REF)],
        CAP,
    );
    let over = document_of_size("src/other.rs", vec![occ(&[1, 11, 16], ALPHA, REF)], CAP + 1);
    assert_eq!((at.compute_size(), over.compute_size()), (CAP, CAP + 1));
    let report = world.import_ok(RA, "docs", &artifact(vec![at, over]));
    assert!(!report.complete);
    assert_eq!((report.completed, report.failed), (1, 1));
    assert_eq!(report.failure_samples[0].path, "src/other.rs");
    assert_eq!(report.failure_samples[0].code, code::DOCUMENT_TOO_LARGE);
    // The exact-size document published; the oversized one was neither
    // silently dropped nor counted accepted-empty.
    assert_eq!(report.accepted_empty, 0);
    assert_eq!(
        world.starts(&alpha_id()),
        vec![("src/lib.rs".to_owned(), 38)]
    );
}

#[test]
fn an_oversized_document_is_named_by_path_wherever_the_path_field_sits_else_by_ordinal() {
    // Message one: field 5 (`text`) first, then field 1 (`relative_path`); the
    // path is found by skipping the leading field whole. Message two names no
    // path at all and falls back to its ordinal.
    let mut first = vec![0x2a, 40];
    first.extend(std::iter::repeat_n(b'a', 40));
    first.extend([0x0a, 10]);
    first.extend(b"src/lib.rs");
    let mut second = vec![0x2a, 40];
    second.extend(std::iter::repeat_n(b'b', 40));
    let mut bytes = Vec::new();
    for message in [&first, &second] {
        bytes.extend([0x12, message.len() as u8]);
        bytes.extend(message.iter());
    }
    let world = toy();
    let manifest = world.manifest(RA, "2026-08-31", "ordinal", &bytes);
    let (index, snapshot) = world.write_pair(&manifest, &bytes);
    let limits = ImportLimits {
        document_bytes: 32,
        ..ImportLimits::default()
    };
    let report = world.import_files(&index, &snapshot, &limits).unwrap();
    assert_eq!(report.failed, 2);
    let paths: Vec<&str> = report
        .failure_samples
        .iter()
        .map(|s| s.path.as_str())
        .collect();
    assert_eq!(paths, vec!["src/lib.rs", "document #2"]);
    assert!(
        report
            .failure_samples
            .iter()
            .all(|s| s.code == code::DOCUMENT_TOO_LARGE)
    );
    // `src/other.rs` is a profile-listed `.rs` source absent from the
    // artifact: accepted-empty, but nothing failed is.
    assert_eq!(report.accepted_empty, 1);
}

fn long_line_source(tokens: usize) -> String {
    format!("{}\n", "x ".repeat(tokens))
}

fn token_ref(i: usize) -> Occurrence {
    occ(&[0, (2 * i) as i32, (2 * i + 1) as i32], ALPHA, REF)
}

#[test]
fn the_occurrence_cap_is_16384_before_deduplication() {
    let world = World::with_sources(&[
        ("src/lib.rs", &long_line_source(16_385)),
        ("src/other.rs", &long_line_source(16_385)),
        ("src/third.rs", &long_line_source(8)),
    ]);
    let at: Vec<Occurrence> = (0..16_384).map(token_ref).collect();
    let over: Vec<Occurrence> = (0..16_385).map(token_ref).collect();
    // 16,385 occurrences of which only three positions are distinct: the cap
    // counts the input, so deduplication cannot rescue it.
    let duplicated: Vec<Occurrence> = (0..16_385).map(|i| token_ref(i % 3)).collect();
    let bytes = artifact(vec![
        doc("src/lib.rs", at),
        doc("src/other.rs", over),
        doc("src/third.rs", duplicated),
    ]);
    let report = world.import_ok(RA, "occurrences", &bytes);
    assert_eq!(
        (report.completed + report.unresolved, report.failed),
        (1, 2)
    );
    let failed: BTreeSet<&str> = report
        .failure_samples
        .iter()
        .map(|s| s.path.as_str())
        .collect();
    assert_eq!(failed, BTreeSet::from(["src/other.rs", "src/third.rs"]));
    assert!(
        report
            .failure_samples
            .iter()
            .all(|s| s.code == code::DOCUMENT_TOO_LARGE)
    );
    assert_eq!(report.references, 16_384);
    // Exact duplicates at the cap store once: 16,384 inputs, three facts.
    let world = World::with_sources(&[("src/lib.rs", &long_line_source(8))]);
    let duplicated: Vec<Occurrence> = (0..16_384).map(|i| token_ref(i % 3)).collect();
    let report = world.import_ok(RA, "dedup", &artifact(vec![doc("src/lib.rs", duplicated)]));
    assert!(report.complete && report.references == 3, "{report:?}");
    assert_eq!(world.by_symbol(&alpha_id()).items.len(), 3);
}

#[test]
fn the_symbol_cap_is_1024_bytes() {
    let world = World::with_sources(&[
        ("src/lib.rs", "pub fn f() {}\n"),
        ("src/other.rs", "pub fn f() {}\n"),
    ]);
    let at = "s".repeat(1024);
    let over = "s".repeat(1025);
    let bytes = artifact(vec![
        doc("src/lib.rs", vec![occ(&[0, 7, 8], &at, DEF)]),
        doc("src/other.rs", vec![occ(&[0, 7, 8], &over, DEF)]),
    ]);
    let report = world.import_ok(RA, "symbols", &bytes);
    assert_eq!((report.completed, report.failed), (1, 1));
    assert_eq!(report.failure_samples[0].path, "src/other.rs");
    assert_eq!(report.failure_samples[0].code, code::DOCUMENT_TOO_LARGE);
}

// --- scratch: limits and cleanup ownership ---------------------------------

fn scratch_area(world: &World) -> PathBuf {
    world.store.join("import-scratch")
}

fn entries(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .map(|read| {
            read.map(|e| e.unwrap().file_name().to_str().unwrap().to_owned())
                .collect()
        })
        .unwrap_or_default();
    names.sort();
    names
}

#[test]
fn scratch_full_stops_the_import_by_name_and_leaves_nothing_behind() {
    let mut world = toy();
    let bytes = toy_artifact();
    let manifest = world.manifest(RA, "2026-08-31", "scratch", &bytes);
    let (index, snapshot) = world.write_pair(&manifest, &bytes);
    let before = without_producers(world.raw());
    // A budget the frozen copies alone overshoot, and one the lookup file
    // overshoots after the copies fit.
    let copies = bytes.len() as u64 + std::fs::metadata(&snapshot).unwrap().len();
    for budget in [1, copies + 1024] {
        let limits = ImportLimits {
            scratch_bytes: budget,
            ..ImportLimits::default()
        };
        let e = world.import_files(&index, &snapshot, &limits).unwrap_err();
        assert_eq!(e.code(), code::SCRATCH_FULL, "budget {budget}");
        assert!(
            entries(&scratch_area(&world)).is_empty(),
            "the run directory is removed"
        );
    }
    assert_eq!(without_producers(world.raw()), before);
    assert!(world.selected(RA).is_none());
    // The same inputs import once the budget allows, and the peak is recorded.
    let report = world
        .import_files(&index, &snapshot, &ImportLimits::default())
        .unwrap();
    assert!(
        report.complete && report.scratch_peak_bytes > copies,
        "{report:?}"
    );
    assert!(entries(&scratch_area(&world)).is_empty());
}

#[test]
fn only_positively_owned_scratch_is_removed() {
    let world = toy();
    let bytes = toy_artifact();
    let area = scratch_area(&world);
    std::fs::create_dir_all(&area).unwrap();
    let workspace_id = world.engine().workspace_id().unwrap();
    let name = |n: u32| format!("run-{n:032x}");
    let marker = |run: &str, workspace: &str, magic: &str| {
        json!({"magic": magic, "workspace_id": workspace, "run": run}).to_string()
    };
    let magic = "context-foundry/import-scratch/v1";
    // Positively owned leftover of an aborted run: removed.
    let owned = area.join(name(1));
    std::fs::create_dir(&owned).unwrap();
    std::fs::write(owned.join("OWNER"), marker(&name(1), &workspace_id, magic)).unwrap();
    std::fs::write(owned.join("artifact.scip"), b"leftover").unwrap();
    // Look-alikes that are NOT ours: no marker, another workspace, another
    // magic, a marker naming another run, a plain file, a symlink to a
    // directory holding a valid marker.
    let foreign: Vec<PathBuf> = (2..6).map(|n| area.join(name(n))).collect();
    for dir in &foreign {
        std::fs::create_dir(dir).unwrap();
        std::fs::write(dir.join("keep.txt"), b"mine").unwrap();
    }
    std::fs::write(
        foreign[1].join("OWNER"),
        marker(&name(3), &digest(b"other"), magic),
    )
    .unwrap();
    std::fs::write(
        foreign[2].join("OWNER"),
        marker(&name(4), &workspace_id, "other/magic"),
    )
    .unwrap();
    std::fs::write(
        foreign[3].join("OWNER"),
        marker(&name(99), &workspace_id, magic),
    )
    .unwrap();
    std::fs::write(area.join("notes.txt"), b"not a run").unwrap();
    let elsewhere = world.dir.path().join("elsewhere");
    std::fs::create_dir(&elsewhere).unwrap();
    std::fs::write(
        elsewhere.join("OWNER"),
        marker(&name(7), &workspace_id, magic),
    )
    .unwrap();
    std::fs::write(elsewhere.join("precious"), b"data").unwrap();
    std::os::unix::fs::symlink(&elsewhere, area.join(name(7))).unwrap();
    let before = entries(&area);
    let report = world.import_ok(RA, "ownership", &bytes);
    assert!(report.complete);
    // The owned leftover and this run's own directory are gone; everything
    // else is exactly as it was.
    let mut expected = before.clone();
    expected.retain(|n| *n != name(1));
    assert_eq!(entries(&area), expected);
    assert!(!owned.exists());
    for dir in &foreign {
        assert_eq!(std::fs::read(dir.join("keep.txt")).unwrap(), b"mine");
    }
    assert_eq!(std::fs::read(elsewhere.join("precious")).unwrap(), b"data");
    assert_eq!(std::fs::read(area.join("notes.txt")).unwrap(), b"not a run");
}

// ---------------------------------------------------------------------------
// T002: scoped publication
// ---------------------------------------------------------------------------

const THIRD: &str = "pub fn delta() {\n    crate::alpha();\n}\n";

fn toy3() -> World {
    World::with_sources(&[
        ("src/lib.rs", LIB),
        ("src/other.rs", OTHER),
        ("src/third.rs", THIRD),
    ])
}

fn toy3_artifact() -> Vec<u8> {
    artifact(vec![
        doc(
            "src/lib.rs",
            vec![
                occ(&[0, 7, 12], ALPHA, DEF),
                occ(&[2, 4, 9], ALPHA, REF),
                occ(&[3, 4, 9], ALPHA, REF),
            ],
        ),
        doc("src/other.rs", vec![occ(&[1, 11, 16], ALPHA, REF)]),
        doc("src/third.rs", vec![occ(&[1, 11, 16], ALPHA, REF)]),
    ])
}

/// Scope keys (`<ns>\0<path>`) of a raw snapshot.
fn scope_keys(snapshot: &testkit::Snapshot) -> Vec<String> {
    snapshot["compiler_scopes"]
        .iter()
        .map(|(key, _)| key.replace('\0', "|"))
        .collect()
}

fn scope_json(snapshot: &testkit::Snapshot, namespace: &str, path: &str) -> Value {
    let key = format!("{namespace}\0{path}");
    serde_json::from_str(
        &snapshot["compiler_scopes"]
            .iter()
            .find(|(k, _)| *k == key)
            .unwrap_or_else(|| panic!("no scope row {key:?}"))
            .1,
    )
    .unwrap()
}

fn rows_of(snapshot: &testkit::Snapshot, table: &str, prefix: &str) -> Vec<(String, String)> {
    snapshot[table]
        .iter()
        .filter(|(key, _)| key.starts_with(prefix))
        .cloned()
        .collect()
}

#[test]
fn accepted_empty_absent_rs_paths_and_non_rs_paths_follow_the_producer_profile() {
    let mut world = World::with_sources(&[
        ("README.md", "notes\n"),
        ("notes.txt", "n\n"),
        ("src/empty.rs", "pub fn nothing() {}\n"),
        ("src/lib.rs", LIB),
        ("src/other.rs", OTHER),
    ]);
    let report = world.import_ok(RA, "profile", &toy_artifact());
    assert!(report.complete, "{report:?}");
    assert_eq!(
        report.coverage, "complete",
        "non-.rs paths are outside the producer's scope and leave coverage unaffected"
    );
    // The manifest-listed `.rs` path the artifact lacks is accepted-empty;
    // the two non-`.rs` paths are outside the producer's scope.
    assert_eq!(
        (report.accepted_empty, report.outside_scope, report.unknown),
        (1, 2, 0)
    );
    let raw = world.raw();
    assert_eq!(
        scope_keys(&raw),
        vec![
            "rust-analyzer|src/empty.rs",
            "rust-analyzer|src/lib.rs",
            "rust-analyzer|src/other.rs"
        ]
    );
    let empty = scope_json(&raw, RA, "src/empty.rs");
    assert_eq!(empty["status"], "accepted_empty");
    assert_eq!(
        (empty["definitions"].as_u64(), empty["references"].as_u64()),
        (Some(0), Some(0))
    );
    assert!(
        rows_of(
            &raw,
            "compiler_occurrences",
            "rust-analyzer\0src/empty.rs\0"
        )
        .is_empty()
    );
    assert_eq!(world.by_symbol(&alpha_id()).coverage, Coverage::Complete);

    // A producer with no profile: an absent document is UNKNOWN, not empty.
    let other = world.import_ok("scip-other", "generic", &toy_artifact());
    assert!(other.complete);
    assert_eq!(
        other.coverage, "partial",
        "unknown coverage is not complete coverage"
    );
    assert_eq!(
        (other.accepted_empty, other.outside_scope, other.unknown),
        (0, 0, 3)
    );
    let raw = world.raw();
    assert!(
        rows_of(&raw, "compiler_scopes", "scip-other\0src/empty.rs").is_empty(),
        "unknown coverage creates no scope row"
    );
    assert_eq!(rows_of(&raw, "compiler_scopes", "scip-other\0").len(), 2);

    // An accepted-empty replacement clears exactly one scope: the artifact now
    // lacks lib.rs, whose three facts disappear while other.rs is replaced.
    let lacking = artifact(vec![doc(
        "src/other.rs",
        vec![occ(&[1, 11, 16], ALPHA, REF)],
    )]);
    let report = world.import_ok(RA, "lacking-lib", &lacking);
    assert!(report.complete && report.accepted_empty == 2, "{report:?}");
    let raw = world.raw();
    assert!(rows_of(&raw, "compiler_occurrences", "rust-analyzer\0src/lib.rs\0").is_empty());
    assert_eq!(
        rows_of(
            &raw,
            "compiler_occurrences",
            "rust-analyzer\0src/other.rs\0"
        )
        .len(),
        1
    );
    assert_eq!(
        scope_json(&raw, RA, "src/lib.rs")["status"],
        "accepted_empty"
    );
    // With the definition gone the reference is unresolved, never invented.
    let outcome = world.by_symbol(&alpha_id());
    assert_eq!(outcome.target, Some(TargetResolution::Unknown));
    assert_eq!((outcome.items.len(), outcome.unresolved), (1, 1));
    assert_eq!(outcome.coverage, Coverage::Partial);
    assert!(outcome.items[0].edge_id.is_none());
    // Producer B (scip-other) kept every row through A's replacement.
    assert_eq!(rows_of(&raw, "compiler_scopes", "scip-other\0").len(), 2);
}

#[test]
fn replaying_identical_input_leaves_identical_rows_and_counts() {
    let mut world = toy3();
    let bytes = toy3_artifact();
    let first = world.import_ok(RA, "replay", &bytes);
    let rows = without_producers(world.raw());
    let selected = world.selected(RA).unwrap();
    let second = world.import_ok(RA, "replay", &bytes);
    let comparable = |mut report: ImportReport| {
        report.timings = Default::default();
        report.scratch_peak_bytes = 0;
        report
    };
    assert_eq!(comparable(first), comparable(second));
    assert_eq!(without_producers(world.raw()), rows);
    assert_eq!(world.selected(RA).unwrap(), selected);
    assert_eq!(world.by_symbol(&alpha_id()).items.len(), 4);
}

#[test]
fn producer_b_is_untouched_by_producer_a_replacement() {
    let mut world = toy3();
    let b_symbol = "scip-b cargo toy 0.1.0 alpha().";
    let b_artifact = artifact(vec![
        doc(
            "src/lib.rs",
            vec![
                occ(&[0, 7, 12], b_symbol, DEF),
                occ(&[2, 4, 9], b_symbol, REF),
            ],
        ),
        doc("src/other.rs", vec![occ(&[1, 11, 16], b_symbol, REF)]),
    ]);
    world.import_ok(RA, "a-one", &toy3_artifact());
    world.import_ok("scip-b", "b-one", &b_artifact);
    let before = world.raw();
    let b_selected = world.selected("scip-b").unwrap();
    // Replace A with different content for lib.rs only.
    let replacement = artifact(vec![
        doc("src/lib.rs", vec![occ(&[0, 7, 12], ALPHA, DEF)]),
        doc("src/other.rs", vec![occ(&[1, 11, 16], ALPHA, REF)]),
        doc("src/third.rs", vec![occ(&[1, 11, 16], ALPHA, REF)]),
    ]);
    world.import_ok(RA, "a-two", &replacement);
    let after = world.raw();
    for table in ["compiler_scopes", "compiler_occurrences"] {
        assert_eq!(
            rows_of(&before, table, "scip-b\0"),
            rows_of(&after, table, "scip-b\0"),
            "{table}"
        );
    }
    let b_id = symbol_id("scip-b", "src/lib.rs", b_symbol);
    assert_eq!(
        rows_of(&before, "compiler_by_symbol", &b_id),
        rows_of(&after, "compiler_by_symbol", &b_id)
    );
    assert_eq!(world.selected("scip-b").unwrap(), b_selected);
    assert_eq!(world.by_symbol(&b_id).items.len(), 2);
    assert_eq!(world.by_symbol(&b_id).coverage, Coverage::Complete);
    // A's own answer changed: lib.rs lost its two references.
    assert_eq!(world.by_symbol(&alpha_id()).items.len(), 2);
    world.close();
    testkit::compiler_consistency(&world.store).unwrap_or_else(|e| panic!("{e}"));
}

#[test]
fn a_source_edit_makes_every_compiler_fact_stale_and_a_reimport_restores_them() {
    let mut world = toy3();
    let bytes = toy3_artifact();
    world.import_ok(RA, "fresh", &bytes);
    let before = world.by_symbol(&alpha_id());
    assert_eq!(
        (before.coverage, before.items.len()),
        (Coverage::Complete, 4)
    );
    let revision = world.engine().source_revision().unwrap();
    // Feedback, memory, a no-op re-add and a search refresh are not source
    // changes: the facts stay eligible.
    world
        .engine()
        .record_feedback(&context_foundry::laya::Feedback {
            task_id: "t".into(),
            query: "q".into(),
            correct_strategy: context_foundry::laya::Strategy::Graph,
            label_source: "operator".into(),
            allow_training: false,
        })
        .unwrap();
    world
        .engine()
        .memory_put(&context_foundry::memory::PutInput {
            fields: context_foundry::memory::RecordFields {
                id: "note".into(),
                text: "memory does not touch code facts".into(),
                author: "tests".into(),
                provenance: "tests".into(),
                source_links: vec![],
            },
            workspace_id: world.engine().workspace_id().unwrap(),
        })
        .unwrap();
    assert!(
        !world.set_source("src/third.rs", THIRD),
        "an unchanged source is a no-op"
    );
    assert_eq!(world.engine().source_revision().unwrap(), revision);
    assert_eq!(world.by_symbol(&alpha_id()).coverage, Coverage::Complete);

    // Edit a THIRD file: neither the definition's nor any referencing
    // scope's endpoint bytes change, yet all of A's facts go stale.
    world.set_source("README.md", "an unrelated edit\n");
    let stale = world.by_symbol(&alpha_id());
    assert_eq!(stale.coverage, Coverage::Stale);
    assert!(stale.items.is_empty() && stale.examined == 0 && !stale.more);
    let text = world.text(&stale, 1024);
    let parsed = parse_v2(&text).unwrap();
    let new_revision = revision + 1;
    assert_eq!(parsed.header.last().unwrap(), "coverage:stale");
    assert!(
        parsed.header.contains(&format!("r{new_revision}")),
        "{text}"
    );
    assert!(parsed.items.is_empty() && parsed.next.is_none());
    // A position seed on an unchanged file is stale too, not "not found".
    let lib = world.whole_file_handle("src/lib.rs");
    let by_position = world
        .references(
            ReferencesSeed::Position {
                handle: lib,
                byte_offset: 40,
            },
            64,
            None,
        )
        .unwrap();
    assert_eq!(by_position.coverage, Coverage::Stale);
    // The old manifest is stale_artifact now; the new one restores eligibility.
    let old = world.manifest(RA, "2026-08-31", "fresh", &bytes);
    assert_eq!(
        old["source_revision"],
        json!(new_revision),
        "manifests track the live revision"
    );
    let mut at_old = old.clone();
    at_old["source_revision"] = json!(revision);
    assert_eq!(
        world.import_manifest(&at_old, &bytes).unwrap_err().code(),
        code::STALE_ARTIFACT
    );
    let report = world.import_ok(RA, "fresh", &bytes);
    assert!(report.complete, "{report:?}");
    let restored = world.by_symbol(&alpha_id());
    assert_eq!(
        (restored.coverage, restored.items.len()),
        (Coverage::Complete, 4)
    );
    assert_eq!(
        world.selected(RA).unwrap().tuple.source_revision,
        new_revision
    );
    // An added source and a deleted source invalidate the same way.
    world.set_source("src/extra.rs", "pub fn extra() {}\n");
    assert_eq!(world.by_symbol(&alpha_id()).coverage, Coverage::Stale);
    world.import_ok(RA, "fresh", &bytes);
    world.remove_source("src/extra.rs");
    assert_eq!(world.by_symbol(&alpha_id()).coverage, Coverage::Stale);
    // The facts persist across a restart, and so does their staleness.
    world.reopen();
    assert_eq!(world.by_symbol(&alpha_id()).coverage, Coverage::Stale);
    world.import_ok(RA, "fresh", &bytes);
    world.reopen();
    assert_eq!(world.by_symbol(&alpha_id()).coverage, Coverage::Complete);
}

#[test]
fn the_real_artifact_goes_stale_after_a_third_file_edit_and_a_reimport_restores_it() {
    let mut world = World::fixture();
    let bytes = fixture_artifact();
    world
        .import_manifest(&fixture_manifest(&world, &bytes), &bytes)
        .unwrap();
    let a_id = symbol_id(RA, "src/a.rs", A_SYMBOL);
    assert_eq!(world.by_symbol(&a_id).coverage, Coverage::Complete);
    // Append a comment to lib.rs: a third file, positions of every recorded
    // occurrence unchanged.
    let lib = std::fs::read_to_string(world.root.join("src/lib.rs")).unwrap();
    world.set_source("src/lib.rs", &format!("{lib}// edited\n"));
    let stale = world.by_symbol(&a_id);
    assert_eq!((stale.coverage, stale.items.len()), (Coverage::Stale, 0));
    // Re-import the SAME artifact bytes at the new revision, with lib.rs's
    // new hash in the manifest: eligibility returns.
    let report = world
        .import_manifest(&fixture_manifest(&world, &bytes), &bytes)
        .unwrap();
    assert!(report.complete && report.failed == 0, "{report:?}");
    assert_eq!(
        report.coverage, "partial",
        "three sources keep unresolved references"
    );
    let restored = world.by_symbol(&a_id);
    assert_eq!(restored.coverage, Coverage::Complete);
    let expected = expected();
    let mut want: Vec<(String, u64, u64)> = expected
        .a_references
        .iter()
        .map(|(p, s, e, _)| (p.clone(), *s, *e))
        .collect();
    want.sort();
    let got: Vec<(String, u64, u64)> = restored
        .items
        .iter()
        .map(|i| (i.path.clone(), i.start, i.end))
        .collect();
    assert_eq!(got, want);
    testkit::compiler_consistency(&{
        world.close();
        world.store.clone()
    })
    .unwrap();
}

// --- interruptions: no half reverse index ----------------------------------

/// Import `toy3` with `point` armed; the world comes back with the result.
fn interrupted(point: &str, skip: usize, action: Action) -> (World, FResult<ImportReport>) {
    let world = toy3();
    let bytes = toy3_artifact();
    let manifest = world.manifest(RA, "2026-08-31", "cfg", &bytes);
    let (index, snapshot) = world.write_pair(&manifest, &bytes);
    fault::arm(point, skip, action);
    let control = Control::unbounded();
    let result = world.engine().import_scip(&index, &snapshot, &control);
    fault::disarm_all();
    (world, result)
}

fn committed(report: &ImportReport) -> u64 {
    report.completed + report.accepted_empty + report.unresolved
}

#[test]
fn interruptions_commit_whole_scopes_only_and_leave_a_named_partial_snapshot() {
    type Case = (&'static str, usize, fn() -> Action, &'static str, u64);
    // (point, skip, action, expected interruption, scopes committed)
    let cases: Vec<Case> = vec![
        (
            names::SCIP_BETWEEN_DOCUMENTS,
            1,
            || Action::Cancel,
            "cancelled",
            1,
        ),
        (
            names::SCIP_SCOPE_BEFORE_COMMIT,
            1,
            || Action::Fail("disk".into()),
            "internal",
            1,
        ),
        // Cancelled with the first scope built but not committed.
        (
            names::SCIP_SCOPE_BEFORE_COMMIT,
            0,
            || Action::Cancel,
            "cancelled",
            0,
        ),
        (
            names::SCIP_SCOPE_AFTER_COMMIT,
            0,
            || Action::Cancel,
            "cancelled",
            1,
        ),
        (
            names::SCIP_SCOPE_AFTER_COMMIT,
            0,
            || Action::Fail("disk".into()),
            "internal",
            1,
        ),
        (
            names::SCIP_AFTER_SELECTION,
            0,
            || Action::Cancel,
            "cancelled",
            0,
        ),
    ];
    for (point, skip, action, reason, expected) in cases {
        let (mut world, result) = interrupted(point, skip, action());
        let report = result.unwrap_or_else(|e| panic!("{point}: {e:?}"));
        assert!(report.selected && !report.complete, "{point}: {report:?}");
        assert_eq!(report.interrupted.as_deref(), Some(reason), "{point}");
        assert_eq!(report.failure.as_ref().unwrap().code, reason, "{point}");
        assert_eq!(committed(&report), expected, "{point}");
        assert_eq!(world.selected(RA).unwrap().state, SnapshotState::Partial);
        // The scopes that committed are whole; nothing half-written remains.
        let raw = world.raw();
        assert_eq!(raw["compiler_scopes"].len() as u64, expected, "{point}");
        world.close();
        testkit::compiler_consistency(&world.store).unwrap_or_else(|e| panic!("{point}: {e}"));
        world.reopen();
        // The interrupted snapshot answers partially, never completely.
        if let Some(outcome) = world.by_symbol_or_none(&alpha_id()) {
            assert_eq!(outcome.coverage, Coverage::Partial, "{point}");
        }
        // Replay resumes the same snapshot: complete, no duplicates.
        let selected_before = world.selected(RA).unwrap().tuple;
        let bytes = toy3_artifact();
        let replay = world.import_ok(RA, "cfg", &bytes);
        assert!(replay.complete, "{point}: {replay:?}");
        assert_eq!(replay.snapshot_id, selected_before.snapshot_id, "{point}");
        assert_eq!(world.by_symbol(&alpha_id()).items.len(), 4, "{point}");
        let rows = world.raw();
        let (_clean_dir, clean) = clean_import_rows();
        for table in ["compiler_occurrences", "compiler_by_symbol"] {
            assert_eq!(rows[table], clean[table], "{point}: {table}");
        }
    }
}

/// Occurrence/reverse rows of an uninterrupted `toy3` import (those tables do
/// not depend on the workspace identity).
fn clean_import_rows() -> (World, testkit::Snapshot) {
    let mut world = toy3();
    world.import_ok(RA, "cfg", &toy3_artifact());
    let rows = world.raw();
    (world, rows)
}

#[test]
fn cancellation_before_selection_changes_nothing() {
    for point in [names::SCIP_AFTER_COPY, names::SCIP_AFTER_LOOKUP] {
        let (mut world, result) = interrupted(point, 0, Action::Cancel);
        let error = result.unwrap_err();
        assert_eq!(
            (error.code(), error.exit_code()),
            ("cancelled", 130),
            "{point}"
        );
        assert!(world.selected(RA).is_none());
        assert!(
            world.engine().compiler_producers().unwrap().is_empty(),
            "nothing is recorded"
        );
        let raw = world.raw();
        for table in testkit::COMPILER_TABLES {
            assert!(raw[table].is_empty(), "{point}: {table}");
        }
        assert!(entries(&scratch_area(&world)).is_empty());
    }
}

#[test]
fn a_second_artifact_at_the_same_revision_never_mixes_with_the_old_one() {
    let mut world = toy3();
    let a = toy3_artifact();
    world.import_ok(RA, "config-a", &a);
    assert_eq!(world.by_symbol(&alpha_id()).items.len(), 4);
    // B: another artifact/config at the same source revision, dropping the
    // reference in third.rs, interrupted right after its first document.
    let b = artifact(vec![
        doc(
            "src/lib.rs",
            vec![occ(&[0, 7, 12], ALPHA, DEF), occ(&[2, 4, 9], ALPHA, REF)],
        ),
        doc("src/other.rs", vec![occ(&[1, 11, 16], ALPHA, REF)]),
        doc("src/third.rs", vec![]),
    ]);
    let manifest = world.manifest(RA, "2026-08-31", "config-b", &b);
    let (index, snapshot) = world.write_pair(&manifest, &b);
    let control = Control::unbounded();
    fault::arm(names::SCIP_SCOPE_AFTER_COMMIT, 0, Action::Cancel);
    let report = world
        .engine()
        .import_scip(&index, &snapshot, &control)
        .unwrap();
    fault::disarm_all();
    assert_eq!(report.interrupted.as_deref(), Some("cancelled"));
    let selected = world.selected(RA).unwrap();
    assert_eq!(selected.tuple.artifact_sha256, digest(&b));
    assert_eq!(selected.state, SnapshotState::Partial);
    // Only B's lib.rs scope is eligible. A's other.rs and third.rs facts are
    // still stored but invisible: no union with the old configuration.
    let outcome = world.by_symbol(&alpha_id());
    assert_eq!(outcome.coverage, Coverage::Partial);
    let seen: Vec<(&str, u64)> = outcome
        .items
        .iter()
        .map(|i| (i.path.as_str(), i.start))
        .collect();
    assert_eq!(seen, vec![("src/lib.rs", 38)]);
    assert_eq!(
        outcome.stale, 2,
        "the two old-snapshot reference records are counted, not shown"
    );
    let raw = world.raw();
    assert_eq!(
        scope_json(&raw, RA, "src/other.rs")["snapshot_id"],
        json!(report_snapshot_a(&world, &a))
    );
    // Replay resumes B's snapshot and finishes without duplicates.
    let replay = world
        .import_files(&index, &snapshot, &ImportLimits::default())
        .unwrap();
    assert!(replay.complete, "{replay:?}");
    assert_eq!(replay.snapshot_id, report.snapshot_id);
    let outcome = world.by_symbol(&alpha_id());
    assert_eq!(outcome.coverage, Coverage::Complete);
    let seen: Vec<(&str, u64)> = outcome
        .items
        .iter()
        .map(|i| (i.path.as_str(), i.start))
        .collect();
    assert_eq!(seen, vec![("src/lib.rs", 38), ("src/other.rs", 28)]);
    world.close();
    testkit::compiler_consistency(&world.store).unwrap();
}

/// The snapshot id the first (A) import published its scopes under.
fn report_snapshot_a(world: &World, a: &[u8]) -> String {
    let manifest = world.manifest(RA, "2026-08-31", "config-a", a);
    let bytes = serde_json::to_vec(&manifest).unwrap();
    let tuple = context_foundry::graph::SnapshotTuple::new(
        context_foundry::graph::ProducerIdentity {
            name: RA.into(),
            release_tag: "2026-08-31".into(),
            commit: manifest["producer"]["commit"].as_str().unwrap().into(),
            version_output: "test-producer 1.0".into(),
            binary_sha256: digest(b"test-producer-binary"),
        },
        manifest["invocation"].as_str().unwrap(),
        "config-a",
        digest(a),
        digest(&bytes),
        world.engine().source_revision().unwrap(),
    );
    tuple.snapshot_id
}

// --- hubs, locals, prefixes -------------------------------------------------

const HUB: &str = "rust-analyzer cargo toy 0.1.0 Hub#";

fn hub_world() -> World {
    World::with_sources(&[("src/lib.rs", &"hub\n".repeat(600))])
}

/// `defs` definitions then `refs` references of one symbol, one per line.
fn hub_artifact(defs: usize, refs: usize) -> Vec<u8> {
    let mut occurrences = Vec::new();
    for i in 0..defs {
        occurrences.push(occ(&[i as i32, 0, 3], HUB, DEF));
    }
    for i in 0..refs {
        occurrences.push(occ(&[(defs + i) as i32, 0, 3], HUB, REF));
    }
    artifact(vec![doc("src/lib.rs", occurrences)])
}

/// Page through a symbol with `limit` until no more remain.
fn all_pages(world: &World, symbol: &str, limit: usize) -> (Vec<u64>, usize) {
    let (mut starts, mut pages, mut after) = (Vec::new(), 0, None::<String>);
    loop {
        let outcome = world
            .references(
                ReferencesSeed::SymbolId(symbol[..16].to_owned()),
                limit,
                after.as_deref(),
            )
            .unwrap();
        assert!(outcome.examined <= 256, "{}", outcome.examined);
        pages += 1;
        starts.extend(outcome.items.iter().map(|i| i.start));
        match (outcome.more, outcome.resume) {
            (true, Some(cursor)) => after = Some(cursor),
            (false, None) => return (starts, pages),
            other => panic!("inconsistent continuation {other:?}"),
        }
    }
}

#[test]
fn a_300_definition_300_reference_symbol_stores_600_occurrences_not_90000_pairs() {
    let mut world = hub_world();
    let report = world.import_ok(RA, "hub", &hub_artifact(300, 300));
    assert!(report.complete, "{report:?}");
    assert_eq!(
        (report.definitions, report.references, report.occurrences),
        (300, 300, 600)
    );
    let raw = world.raw();
    assert_eq!(raw["compiler_occurrences"].len(), 600);
    assert_eq!(raw["compiler_by_symbol"].len(), 600);
    let hub = symbol_id(RA, "src/lib.rs", HUB);
    // Limit 256: the examination window (nine definition probes, then
    // references) truncates retrieval; nothing resolves to an invented target.
    let first = world
        .references(ReferencesSeed::SymbolId(hub[..16].to_owned()), 256, None)
        .unwrap();
    assert_eq!(first.target, Some(TargetResolution::Ambiguous));
    assert_eq!(
        (first.definitions.len(), first.definitions_truncated),
        (8, true)
    );
    assert_eq!(first.examined, 256);
    assert_eq!(first.items.len(), 247);
    assert!(first.more && first.candidates_full && first.resume.is_some());
    assert_eq!(first.unresolved, 247);
    assert!(first.items.iter().all(|i| i.edge_id.is_none()));
    assert_eq!(first.coverage, Coverage::Partial);
    let text = world.text(&first, 32768);
    let parsed = parse_v2(&text).unwrap();
    assert!(
        parsed.header.contains(&"candidates:full".to_owned()),
        "{text}"
    );
    assert!(parsed.header.contains(&"unresolved:247".to_owned()));
    assert_eq!(parsed.next.as_deref(), Some("src/lib.rs#2184-2187"));
    // Read-back through the cursor finds every stored reference, once.
    let (starts, pages) = all_pages(&world, &hub, 256);
    assert_eq!(starts.len(), 300);
    let expected: Vec<u64> = (300..600).map(|line| line * 4).collect();
    assert_eq!(starts, expected);
    assert_eq!(pages, 2);
    // A definition probe is part of every page's window.
    let (starts, pages) = all_pages(&world, &hub, 64);
    assert_eq!((starts, pages), (expected.clone(), 5));
}

#[test]
fn a_unique_definition_hub_pages_through_its_references_and_keeps_edge_ids() {
    let world = hub_world();
    world.import_ok(RA, "hub", &hub_artifact(1, 300));
    let hub = symbol_id(RA, "src/lib.rs", HUB);
    let first = world
        .references(ReferencesSeed::SymbolId(hub[..16].to_owned()), 256, None)
        .unwrap();
    assert_eq!(first.target, Some(TargetResolution::Unique));
    // One definition probe plus 255 references fill the 256-record window.
    assert_eq!((first.examined, first.items.len()), (256, 255));
    assert!(first.candidates_full && first.more);
    assert!(first.items.iter().all(|i| i.edge_id.is_some()));
    assert_eq!(first.unresolved, 0);
    let (starts, pages) = all_pages(&world, &hub, 256);
    assert_eq!(starts, (1..301).map(|line| line * 4).collect::<Vec<u64>>());
    assert_eq!(pages, 2);
}

#[test]
fn an_unfinished_scan_never_certifies_a_winner_or_an_absence() {
    // A whole-file definition of `Big` (a module-like range) over 300 tokens.
    const BIG: &str = "rust-analyzer cargo toy 0.1.0 Big#";
    let world = World::with_sources(&[
        ("src/lib.rs", &long_line_source(300)),
        ("src/other.rs", OTHER),
    ]);
    let mut tokens: Vec<Occurrence> = (0..300).map(token_ref).collect();
    tokens.push(occ(&[0, 0, 1, 0], BIG, DEF));
    let bytes = artifact(vec![
        doc("src/lib.rs", tokens),
        doc(
            "src/other.rs",
            vec![occ(&[0, 7, 12], BIG, DEF), occ(&[1, 11, 16], BIG, REF)],
        ),
    ]);
    world.import_ok(RA, "big", &bytes);
    let seed = |offset: u64| ReferencesSeed::Position {
        handle: world.whole_file_handle("src/lib.rs"),
        byte_offset: offset,
    };
    // Near the start the containing search finishes: two definitions are
    // seen and the symbol is conclusively ambiguous.
    let near = world.references(seed(3), 64, None).unwrap();
    assert_eq!(near.target, Some(TargetResolution::Ambiguous));
    assert!(near.examined < 10);
    // Deep into the file the scan for the containing occurrences leaves the
    // last two records of the shared allowance to the definition lookup and
    // to reference work, so it stops at 254 and says so: no winner and no
    // absence is certified (the seed is neither found nor not-found).
    let deep = world.references(seed(507), 64, None).unwrap();
    assert_eq!(deep.examined, 254);
    assert_eq!(deep.coverage, Coverage::Partial);
    assert!(deep.symbol_id.is_none() && deep.target.is_none());
    assert!(deep.definitions_truncated && deep.candidates_full);
    assert!(!deep.more && deep.resume.is_none() && deep.items.is_empty());
    let text = world.text(&deep, 1024);
    let parsed = parse_v2(&text).unwrap();
    assert!(
        parsed.header.contains(&"candidates:full".to_owned()),
        "{text}"
    );
    assert_eq!(parsed.header.last().unwrap(), "coverage:partial");
    assert!(parsed.items.is_empty() && parsed.next.is_none());

    // An unfinished DEFINITION lookup with no eligible candidate is
    // unfinished too: every stored definition sits in another snapshot, so
    // the scan of stale records fills the window before uniqueness or
    // absence could be decided.
    let stale = hub_world();
    stale.import_ok(RA, "config-a", &hub_artifact(300, 1));
    let hub = symbol_id(RA, "src/lib.rs", HUB);
    let artifact = hub_artifact(300, 1);
    let manifest = stale.manifest(RA, "2026-08-31", "config-b", &artifact);
    let (index, snapshot) = stale.write_pair(&manifest, &artifact);
    let control = Control::unbounded();
    fault::arm(names::SCIP_AFTER_SELECTION, 0, Action::Cancel);
    stale
        .engine()
        .import_scip(&index, &snapshot, &control)
        .unwrap();
    fault::disarm_all();
    let outcome = stale.by_symbol(&hub);
    assert_eq!(outcome.target, Some(TargetResolution::Unfinished));
    assert!(
        outcome.definitions_truncated,
        "the interrupted lookup is marked"
    );
    assert_eq!(outcome.examined, 256);
    assert_eq!(outcome.coverage, Coverage::Partial);
    assert!(outcome.items.is_empty());
    assert!(
        !outcome.more,
        "the reserved record examined the only (old-snapshot) reference"
    );
}

#[test]
fn local_symbols_with_the_same_spelling_in_different_files_have_different_wire_ids() {
    let world = World::with_sources(&[
        ("src/lib.rs", "fn a(x: u8) { x; }\n"),
        ("src/other.rs", "fn b(x: u8) { x; }\n"),
    ]);
    let local = |path: &str| {
        doc(
            path,
            vec![
                occ(&[0, 5, 6], "local 0", DEF),
                occ(&[0, 14, 15], "local 0", REF),
            ],
        )
    };
    world.import_ok(
        RA,
        "locals",
        &artifact(vec![local("src/lib.rs"), local("src/other.rs")]),
    );
    let lib_id = symbol_id(RA, "src/lib.rs", "local 0");
    let other_id = symbol_id(RA, "src/other.rs", "local 0");
    assert_ne!(lib_id[..16], other_id[..16]);
    for (path, id, other) in [
        ("src/lib.rs", &lib_id, &other_id),
        ("src/other.rs", &other_id, &lib_id),
    ] {
        let by_id = world.by_symbol(id);
        assert_eq!(
            by_id
                .items
                .iter()
                .map(|i| (i.path.as_str(), i.start))
                .collect::<Vec<_>>(),
            vec![(path, 14)]
        );
        assert_ne!(by_id.symbol_id.as_deref(), Some(other.as_str()));
        // The id a position query returns round-trips through `references`
        // with no private importer state.
        let by_position = world
            .references(
                ReferencesSeed::Position {
                    handle: world.whole_file_handle(path),
                    byte_offset: 14,
                },
                64,
                None,
            )
            .unwrap();
        let returned = by_position.symbol_id.clone().unwrap();
        assert_eq!(&returned, id);
        let again = world.by_symbol(&returned);
        assert_eq!(again.items.len(), 1);
        assert_eq!(
            again.items[0].occurrence_id,
            by_position.items[0].occurrence_id
        );
    }
}

#[test]
fn a_16_hex_prefix_shared_by_stored_ids_is_ambiguous_with_at_most_eight_candidates() {
    let mut world = toy();
    world.import_ok(RA, "prefix", &toy_artifact());
    let prefix = alpha_id()[..16].to_owned();
    let insert = |world: &mut World, count: usize| {
        world.close();
        testkit::write_store(&world.store, |tx| {
            let mut table = tx
                .open_table(redb::TableDefinition::<&str, &str>::new(
                    "compiler_by_symbol",
                ))
                .unwrap();
            for n in 0..count {
                let id = format!("{prefix}{:048x}", n + 1);
                table
                    .insert(
                        format!("{id}\0d\0src/lib.rs\0{:020}\0{:020}", 0, 1).as_str(),
                        RA,
                    )
                    .unwrap();
            }
        });
        world.reopen();
    };
    let message = |error: FoundryError| match error {
        FoundryError::Scip { code, message } => (code, message),
        other => panic!("{other:?}"),
    };
    // One more stored id with the same 16 hex characters: ambiguous.
    insert(&mut world, 1);
    let (code_name, text) = message(world.by_symbol_err(&prefix));
    assert_eq!(code_name, code::AMBIGUOUS_SYMBOL);
    assert_eq!(text.matches(&prefix).count(), 2, "{text}");
    assert!(!text.contains("more exist"));
    // Nine more: eight are named and the truncation is flagged.
    insert(&mut world, 9);
    let (code_name, text) = message(world.by_symbol_err(&prefix));
    assert_eq!(code_name, code::AMBIGUOUS_SYMBOL);
    assert_eq!(text.matches(&prefix).count(), 8, "{text}");
    assert!(text.contains("more exist"));
    // An unrelated prefix names no stored symbol.
    let unrelated = world.by_symbol_err("0000000000000000");
    assert_eq!(unrelated.code(), code::SYMBOL_NOT_FOUND);
}

impl World {
    fn by_symbol_err(&self, prefix: &str) -> FoundryError {
        self.references(ReferencesSeed::SymbolId(prefix.to_owned()), 64, None)
            .unwrap_err()
    }
}

#[test]
fn several_symbols_at_one_position_are_ambiguous_and_the_innermost_range_wins() {
    const BETA: &str = "rust-analyzer cargo toy 0.1.0 beta().";
    const GAMMA: &str = "rust-analyzer cargo toy 0.1.0 gamma().";
    let world = toy();
    // Two symbols on exactly the same range [38,43), and a narrower third
    // inside it [39,41).
    let bytes = artifact(vec![
        doc(
            "src/lib.rs",
            vec![
                occ(&[0, 7, 12], ALPHA, DEF),
                occ(&[2, 4, 9], ALPHA, REF),
                occ(&[2, 4, 9], BETA, REF),
                occ(&[2, 5, 7], GAMMA, REF),
            ],
        ),
        doc("src/other.rs", vec![]),
    ]);
    world.import_ok(RA, "ambiguous", &bytes);
    let at = |offset: u64| {
        world.references(
            ReferencesSeed::Position {
                handle: world.whole_file_handle("src/lib.rs"),
                byte_offset: offset,
            },
            64,
            None,
        )
    };
    let error = at(38).unwrap_err();
    let FoundryError::Scip { code: c, message } = &error else {
        panic!("{error:?}")
    };
    assert_eq!(*c, code::AMBIGUOUS_SYMBOL);
    let (alpha, beta) = (alpha_id(), symbol_id(RA, "src/lib.rs", BETA));
    assert!(
        message.contains(&alpha[..16]) && message.contains(&beta[..16]),
        "{message}"
    );
    // Each returned candidate id round-trips as an explicit seed.
    assert_eq!(
        world.by_symbol(&alpha).symbol_id.as_deref(),
        Some(alpha.as_str())
    );
    assert_eq!(world.by_symbol(&beta).items.len(), 1);
    // Inside the narrower range its symbol alone is the innermost.
    let gamma = at(40).unwrap();
    assert_eq!(gamma.symbol_id, Some(symbol_id(RA, "src/lib.rs", GAMMA)));
    // No occurrence covers the closing brace: symbol_not_found.
    let end = LIB.len() as u64 - 1;
    let error = world
        .references(
            ReferencesSeed::Position {
                handle: world.whole_file_handle("src/lib.rs"),
                byte_offset: end,
            },
            64,
            None,
        )
        .unwrap_err();
    assert_eq!(error.code(), code::SYMBOL_NOT_FOUND);
}

// --- budget and continuation -----------------------------------------------

#[test]
fn next_after_continues_exactly_within_every_token_budget() {
    let world = hub_world();
    world.import_ok(RA, "pages", &hub_artifact(1, 60));
    let hub = symbol_id(RA, "src/lib.rs", HUB);
    for budget in [150usize, 260, 1024] {
        let mut after: Option<String> = None;
        let mut lines: Vec<String> = Vec::new();
        let mut pages = 0;
        loop {
            let outcome = world
                .references(
                    ReferencesSeed::SymbolId(hub[..16].to_owned()),
                    64,
                    after.as_deref(),
                )
                .unwrap();
            let packed = response::pack_references(
                &outcome,
                response::Budget::request(budget),
                &response::stdout_bytes,
            )
            .unwrap();
            // The exact o200k count of the final text stays within the budget.
            assert_eq!(packed.tokens, response::count_tokens(&packed.text));
            assert!(packed.tokens <= budget, "{} > {budget}", packed.tokens);
            let parsed = parse_v2(&packed.text).unwrap();
            pages += 1;
            assert!(!parsed.items.is_empty(), "every page makes progress");
            assert_eq!(
                packed.omitted > 0,
                parsed.header.iter().any(|s| s.starts_with("omitted:"))
            );
            lines.extend(parsed.items.iter().map(|i| i.lines.clone().unwrap()));
            match parsed.next {
                Some(cursor) => after = Some(cursor),
                None => break,
            }
            assert!(pages < 100);
        }
        // Every reference line exactly once, in order, through `next: after=`.
        assert_eq!(
            lines,
            (2..=61).map(|n| format!("L{n}")).collect::<Vec<_>>(),
            "budget {budget}"
        );
        assert!(pages > 1, "budget {budget} holds {pages} page(s)");
    }
    // A budget that cannot hold the header plus one line fails with a
    // sufficient hint, never an unchanged continuation.
    let outcome = world.by_symbol(&hub);
    let error = response::pack_references(
        &outcome,
        response::Budget::request(1),
        &response::stdout_bytes,
    )
    .unwrap_err();
    let FoundryError::BudgetTooSmall { minimum_tokens } = error else {
        panic!("{error:?}")
    };
    let packed = response::pack_references(
        &outcome,
        response::Budget::request(minimum_tokens),
        &response::stdout_bytes,
    )
    .unwrap();
    assert!(packed.tokens <= minimum_tokens);
    assert!(!parse_v2(&packed.text).unwrap().items.is_empty());
}

#[test]
fn without_a_graph_or_with_invalid_seeds_references_names_each_case() {
    let world = toy();
    // No compiler snapshot: a success whose coverage says so.
    let none = world.by_symbol(&alpha_id());
    assert_eq!(none.coverage, Coverage::Unavailable);
    assert!(none.items.is_empty() && none.symbol_id.is_none());
    let text = world.text(&none, 1024);
    let parsed = parse_v2(&text).unwrap();
    assert_eq!(parsed.header.last().unwrap(), "coverage:unavailable");
    assert!(parsed.header.contains(&"examined:0".to_owned()), "{text}");
    let lib = world.whole_file_handle("src/lib.rs");
    let unavailable = world
        .references(
            ReferencesSeed::Position {
                handle: lib.clone(),
                byte_offset: 8,
            },
            64,
            None,
        )
        .unwrap();
    assert_eq!(unavailable.coverage, Coverage::Unavailable);
    world.import_ok(RA, "errors", &toy_artifact());
    let seed = |handle: &str, offset: u64| ReferencesSeed::Position {
        handle: handle.to_owned(),
        byte_offset: offset,
    };
    let code_of = |result: FResult<ReferencesOutcome>| result.unwrap_err().code().to_owned();
    // Arguments.
    for bad in [
        "",
        "ABCDEF0123456789",
        "0123456789abcde",
        "0123456789abcdef0",
        "0123456789abcdeg",
    ] {
        assert_eq!(
            code_of(world.references(ReferencesSeed::SymbolId(bad.to_owned()), 64, None)),
            "invalid_argument",
            "{bad:?}"
        );
    }
    let symbol = ReferencesSeed::SymbolId(alpha_id()[..16].to_owned());
    for limit in [0usize, 257] {
        assert_eq!(
            code_of(world.references(symbol.clone(), limit, None)),
            "invalid_argument"
        );
    }
    for after in [
        "nohash",
        "src/lib.rs#01",
        "#5",
        "../x#1",
        "src/lib.rs#",
        "src/lib.rs#-1",
        "src/lib.rs#5",
        "src/lib.rs#5-5",
        "src/lib.rs#7-3",
        "src/lib.rs#05-9",
    ] {
        assert_eq!(
            code_of(world.references(symbol.clone(), 64, Some(after))),
            "invalid_argument",
            "{after:?}"
        );
    }
    // A valid cursor past every reference is an empty, complete page.
    let past = world
        .references(symbol.clone(), 64, Some("src/other.rs#999-1000"))
        .unwrap();
    assert!(past.items.is_empty() && !past.more);
    // 001 precedence for the handle seed: syntax, workspace, existence,
    // digest, range, then the offset.
    assert_eq!(
        code_of(world.references(seed("not a handle", 1), 64, None)),
        "invalid_argument"
    );
    let foreign = SourceHandle {
        workspace_id: digest(b"another root"),
        path: "src/lib.rs".into(),
        sha256: digest(LIB.as_bytes()),
        start: 0,
        end: 5,
    }
    .to_v2();
    assert_eq!(
        code_of(world.references(seed(&foreign, 1), 64, None)),
        "wrong_workspace"
    );
    let handle = |path: &str, sha: &str, start: u64, end: u64| {
        SourceHandle {
            workspace_id: world.engine().workspace_id().unwrap(),
            path: path.into(),
            sha256: sha.into(),
            start,
            end,
        }
        .to_v2()
    };
    let sha = digest(LIB.as_bytes());
    let len = LIB.len() as u64;
    assert_eq!(
        code_of(world.references(seed(&handle("src/nope.rs", &sha, 0, 4), 1), 64, None)),
        "not_found"
    );
    assert_eq!(
        code_of(world.references(
            seed(&handle("src/lib.rs", &digest(b"edited"), 0, 4), 1),
            64,
            None
        )),
        "stale_handle"
    );
    assert_eq!(
        code_of(world.references(seed(&handle("src/lib.rs", &sha, 0, len + 1), 1), 64, None)),
        "invalid_range"
    );
    assert_eq!(
        code_of(world.references(seed(&handle("src/lib.rs", &sha, 0, 0), 0), 64, None)),
        "invalid_range",
        "[0,0) of a non-empty source"
    );
    let first_line = handle("src/lib.rs", &sha, 0, 17);
    assert_eq!(
        code_of(world.references(seed(&first_line, 17), 64, None)),
        "invalid_range"
    );
    assert_eq!(
        code_of(world.references(seed(&first_line, 40), 64, None)),
        "invalid_range"
    );
    assert_eq!(
        world
            .references(seed(&first_line, 8), 64, None)
            .unwrap()
            .items
            .len(),
        3
    );
}

// --- store schema 4 ---------------------------------------------------------

#[test]
fn a_v3_store_upgrades_to_v4_in_one_transaction_preserving_every_other_feature() {
    use context_foundry::graph::{Edge, Endpoint, GraphBundle};
    let dir = tempfile::tempdir().unwrap();
    let (root, store) = (dir.path().join("ws"), dir.path().join("store"));
    std::fs::create_dir(&root).unwrap();
    // A populated store: a source with unfinished index work, a manual graph
    // bundle, feedback and a memory record (the never-reset counter is 1).
    let engine = Engine::initialize(&store, &root).unwrap();
    engine
        .replace_source("kept.rs", "pub fn kept() {}\n")
        .unwrap();
    let endpoint = || Endpoint {
        path: "kept.rs".into(),
        line: 1,
        symbol: "kept".into(),
        hash: digest("pub fn kept() {}\n".as_bytes()),
    };
    engine
        .import_graph(&GraphBundle {
            provider: "manual".into(),
            revision: "r1".into(),
            edges: vec![Edge {
                from: endpoint(),
                to: endpoint(),
                kind: "references".into(),
                evidence: "manual".into(),
            }],
        })
        .unwrap();
    engine
        .record_feedback(&context_foundry::laya::Feedback {
            task_id: "t".into(),
            query: "q".into(),
            correct_strategy: context_foundry::laya::Strategy::Search,
            label_source: "operator".into(),
            allow_training: true,
        })
        .unwrap();
    engine
        .memory_put(&context_foundry::memory::PutInput {
            fields: context_foundry::memory::RecordFields {
                id: "m".into(),
                text: "kept".into(),
                author: "tests".into(),
                provenance: "tests".into(),
                source_links: vec![],
            },
            workspace_id: engine.workspace_id().unwrap(),
        })
        .unwrap();
    assert_eq!(
        engine.pending().unwrap(),
        2,
        "source and memory work is pending"
    );
    drop(engine);
    testkit::downgrade_to_v3(&store);
    let before = testkit::snapshot(&store);
    assert_eq!(testkit::schema_marker(&store), "3");
    assert_eq!(
        Engine::open_existing(&store).unwrap_err().code(),
        "upgrade_required"
    );

    // Interrupted before commit: wholly v3, row for row.
    fault::arm(
        names::UPGRADE_BEFORE_COMMIT,
        0,
        Action::Fail("injected".into()),
    );
    Engine::upgrade_store(&store, 4, &Control::unbounded()).unwrap_err();
    fault::disarm_all();
    assert_eq!(testkit::schema_marker(&store), "3");
    assert_eq!(testkit::snapshot(&store), before);
    // The previous version is no longer a target.
    let error = Engine::upgrade_store(&store, 3, &Control::unbounded()).unwrap_err();
    assert_eq!(error.code(), "unsupported_mode");
    assert_eq!(testkit::snapshot(&store), before);

    Engine::upgrade_store(&store, 4, &Control::unbounded()).unwrap();
    assert_eq!(testkit::schema_marker(&store), "4");
    let after = testkit::snapshot(&store);
    // The v3 steps ran for a v1/v2 store only: pending keys are not
    // re-prefixed and the memory counter is not reset.
    for table in [
        "sources",
        "chunks",
        "pending_index",
        "feedback",
        "provider_bundles",
        "edges_out",
        "edges_in",
        "memory",
        "scan_seen",
    ] {
        assert_eq!(after[table], before[table], "{table}");
    }
    assert!(!after["memory"].is_empty() && !after["edges_out"].is_empty());
    let strip = |mut meta: Vec<(String, String)>| {
        meta.retain(|(key, _)| key != "schema");
        meta
    };
    assert_eq!(strip(after["meta"].clone()), strip(before["meta"].clone()));
    assert_eq!(
        testkit::meta_value(&store, "memory_revision").as_deref(),
        Some("1")
    );
    for table in testkit::COMPILER_TABLES {
        assert!(after[table].is_empty(), "{table} starts empty");
    }
    // Re-running is a no-op, the store opens, and imports now work.
    Engine::upgrade_store(&store, 4, &Control::unbounded()).unwrap();
    assert_eq!(testkit::snapshot(&store), after);
    let engine = Engine::open_existing(&store).unwrap();
    assert_eq!(engine.status().unwrap().schema, 4);
    assert!(engine.compiler_producers().unwrap().is_empty());
    drop(engine);
    // A newer-than-supported schema is refused by this reader and changes nothing.
    testkit::set_meta(&store, "schema", Some("5"));
    let frozen = testkit::snapshot(&store);
    assert_eq!(
        Engine::open_existing(&store).unwrap_err().code(),
        "unsupported_schema"
    );
    assert_eq!(testkit::snapshot(&store), frozen);
}

#[test]
fn a_v1_and_a_v2_store_upgrade_straight_to_v4_in_one_transaction() {
    for (name, craft) in [
        ("v1", testkit::craft_v1_store as fn(&Path, Option<&Path>)),
        ("v2", testkit::craft_v2_store),
    ] {
        let dir = tempfile::tempdir().unwrap();
        let store = dir.path().join(name);
        craft(&store, Some(&dir.path().join("ws")));
        let before = testkit::snapshot(&store);
        fault::arm(
            names::UPGRADE_BEFORE_COMMIT,
            0,
            Action::Fail("injected".into()),
        );
        Engine::upgrade_store(&store, 4, &Control::unbounded()).unwrap_err();
        fault::disarm_all();
        assert_eq!(testkit::snapshot(&store), before, "{name}: wholly old");
        Engine::upgrade_store(&store, 4, &Control::unbounded()).unwrap();
        assert_eq!(testkit::schema_marker(&store), "4");
        let after = testkit::snapshot(&store);
        for table in [
            "sources",
            "chunks",
            "feedback",
            "provider_bundles",
            "edges_out",
            "edges_in",
        ] {
            assert_eq!(after[table], before[table], "{name}: {table}");
        }
        for table in testkit::COMPILER_TABLES {
            assert!(after[table].is_empty());
        }
        assert_eq!(
            testkit::meta_value(&store, "memory_revision").as_deref(),
            Some("0")
        );
    }
}

#[test]
fn manual_graph_bundles_keep_their_own_rules_beside_compiler_facts() {
    use context_foundry::graph::{Edge, Endpoint, GraphBundle};
    let mut world = toy3();
    let endpoint = |path: &str, body: &str| Endpoint {
        path: path.into(),
        line: 1,
        symbol: path.into(),
        hash: digest(body.as_bytes()),
    };
    world
        .engine()
        .import_graph(&GraphBundle {
            provider: "fixture".into(),
            revision: "r1".into(),
            edges: vec![Edge {
                from: endpoint("src/other.rs", OTHER),
                to: endpoint("src/lib.rs", LIB),
                kind: "calls".into(),
                evidence: "manual".into(),
            }],
        })
        .unwrap();
    let manual_before = {
        let raw = world.raw();
        ["edges_out", "edges_in", "provider_bundles"].map(|t| raw[t].clone())
    };
    world.import_ok(RA, "beside", &toy3_artifact());
    let raw = world.raw();
    let manual_after = ["edges_out", "edges_in", "provider_bundles"].map(|t| raw[t].clone());
    assert_eq!(
        manual_before, manual_after,
        "a compiler import never touches manual rows"
    );
    // The file-neighborhood query still reports only the supplied label.
    let graph = world.engine().graph("src/other.rs", false, 1, 8).unwrap();
    assert_eq!(graph.edges.len(), 1);
    assert_eq!(graph.edges[0].edge.evidence, "manual");
    assert_eq!(graph.scope, "file-neighborhood; supplied symbol labels");
    // Different rules: a third-file edit stales the compiler facts but not
    // the manual edge (its endpoints still match); an endpoint edit stales it.
    world.set_source("README.md", "unrelated\n");
    assert_eq!(world.by_symbol(&alpha_id()).coverage, Coverage::Stale);
    let graph = world.engine().graph("src/other.rs", false, 1, 8).unwrap();
    assert_eq!((graph.edges.len(), graph.stale_edges), (1, 0));
    world.set_source("src/lib.rs", &format!("{LIB}// edited\n"));
    let graph = world.engine().graph("src/other.rs", false, 1, 8).unwrap();
    assert_eq!((graph.edges.len(), graph.stale_edges), (0, 1));
}

// ---------------------------------------------------------------------------
// The CLI
// ---------------------------------------------------------------------------

struct CliWorld {
    dir: tempfile::TempDir,
    ws: PathBuf,
    store: PathBuf,
    artifact: PathBuf,
}

fn cli(store: &Path, args: &[&str]) -> Output {
    Command::new(BIN)
        .arg("--store")
        .arg(store)
        .args(args)
        .output()
        .unwrap()
}

fn cli_faulted(store: &Path, args: &[&str], fault: &str) -> Output {
    Command::new(FAULTS_BIN)
        .env("FOUNDRY_TEST_FAULT", fault)
        .arg("--store")
        .arg(store)
        .args(args)
        .output()
        .unwrap()
}

fn stdout_json(out: &Output) -> Value {
    serde_json::from_slice(&out.stdout)
        .unwrap_or_else(|e| panic!("{e}: {}", String::from_utf8_lossy(&out.stdout)))
}

fn stderr_code(out: &Output) -> String {
    let stderr = String::from_utf8_lossy(&out.stderr);
    let line = stderr
        .lines()
        .rev()
        .find(|l| l.starts_with('{'))
        .unwrap_or_default();
    serde_json::from_str::<Value>(line).unwrap_or_else(|_| panic!("no error JSON: {stderr}"))
        ["code"]
        .as_str()
        .unwrap()
        .to_owned()
}

impl CliWorld {
    /// The fixture workspace indexed through the real CLI, plus the real
    /// artifact copied beside it.
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let ws = dir.path().join("ws");
        copy_tree(&fixture_dir().join("workspace"), &ws);
        let store = dir.path().join("store");
        let out = cli(&store, &["index", ws.to_str().unwrap()]);
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        let artifact = dir.path().join("index.scip");
        std::fs::copy(fixture_dir().join("index.scip"), &artifact).unwrap();
        CliWorld {
            dir,
            ws,
            store,
            artifact,
        }
    }

    fn status(&self) -> Value {
        stdout_json(&cli(&self.store, &["status"]))
    }

    /// A manifest for the current store state, written beside the artifact.
    fn manifest(&self, tweak: impl FnOnce(&mut Value)) -> PathBuf {
        let status = self.status();
        let mut inputs = Vec::new();
        collect_files(&self.ws, &self.ws, &mut inputs);
        inputs.sort();
        let mut manifest = fixture_manifest_for(
            status["workspace_id"].as_str().unwrap(),
            status["source_revision"].as_u64().unwrap(),
            inputs,
            &std::fs::read(&self.artifact).unwrap(),
        );
        tweak(&mut manifest);
        let path = self
            .dir
            .path()
            .join(format!("manifest-{}.json", manifest["source_revision"]));
        std::fs::write(&path, serde_json::to_vec(&manifest).unwrap()).unwrap();
        path
    }

    fn import(&self, manifest: &Path) -> Output {
        cli(
            &self.store,
            &[
                "import-scip",
                "--index",
                self.artifact.to_str().unwrap(),
                "--snapshot",
                manifest.to_str().unwrap(),
            ],
        )
    }

    fn handle(&self, path: &str) -> String {
        let bytes = std::fs::read(self.ws.join(path)).unwrap();
        SourceHandle {
            workspace_id: self.status()["workspace_id"].as_str().unwrap().to_owned(),
            path: path.to_owned(),
            sha256: digest(&bytes),
            start: 0,
            end: bytes.len() as u64,
        }
        .to_v2()
    }
}

#[test]
fn the_cli_imports_the_real_artifact_and_prints_budgeted_references() {
    let world = CliWorld::new();
    let manifest = world.manifest(|_| {});
    let out = world.import(&manifest);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let report = stdout_json(&out);
    assert_eq!(report["complete"], true);
    assert_eq!(
        (report["documents"].as_u64(), report["failed"].as_u64()),
        (Some(7), Some(0))
    );

    let a_id = symbol_id(RA, "src/a.rs", A_SYMBOL);
    let by_symbol = cli(&world.store, &["references", "--symbol-id", &a_id[..16]]);
    assert!(
        by_symbol.status.success(),
        "{}",
        String::from_utf8_lossy(&by_symbol.stderr)
    );
    let text = String::from_utf8(by_symbol.stdout.clone()).unwrap();
    let parsed = parse_v2(&text).unwrap();
    assert_eq!(parsed.items.len(), 3);
    assert_eq!(parsed.header.last().unwrap(), "coverage:complete");
    assert!(parsed.header.contains(&"examined:4".to_owned()), "{text}");
    let meta: Value =
        serde_json::from_str(String::from_utf8_lossy(&by_symbol.stderr).trim()).unwrap();
    assert_eq!(
        meta["stdout_tokens"].as_u64(),
        Some(response::count_tokens(&text) as u64)
    );

    // The handle + byte-offset seed gives the same stdout.
    let handle = world.handle("src/use_one.rs");
    let by_position = cli(
        &world.store,
        &["references", "--handle", &handle, "--byte-offset", "60"],
    );
    assert!(by_position.status.success());
    // Same references; the position seed's own scan is counted in `examined`.
    let by_position_text = String::from_utf8(by_position.stdout.clone()).unwrap();
    let by_position = parse_v2(&by_position_text).unwrap();
    assert_eq!(by_position.items, parsed.items);
    let examined = by_position
        .header
        .iter()
        .find_map(|segment| segment.strip_prefix("examined:"))
        .and_then(|n| n.parse::<u32>().ok())
        .unwrap();
    assert!(examined > 4, "{by_position_text}");

    // A tight budget truncates with `next: after=`, and `--after` continues.
    let mut after: Option<String> = None;
    let mut seen = Vec::new();
    for _ in 0..6 {
        let mut args = vec!["references", "--symbol-id", &a_id[..16], "--tokens", "100"];
        if let Some(cursor) = &after {
            args.extend(["--after", cursor]);
        }
        let page = cli(&world.store, &args);
        assert!(
            page.status.success(),
            "{}",
            String::from_utf8_lossy(&page.stderr)
        );
        let parsed = parse_v2(&String::from_utf8(page.stdout).unwrap()).unwrap();
        seen.extend(parsed.items.iter().map(|i| i.handle.clone()));
        match parsed.next {
            Some(cursor) => after = Some(cursor),
            None => break,
        }
    }
    assert_eq!(seen.len(), 3);
    assert!(seen.iter().any(|h| h.starts_with("src/pointer.rs#")));

    // Invalid invocations exit 2: no seed, two seeds, a handle without an
    // offset, out-of-range arguments, a missing import file.
    let invalid: Vec<Vec<&str>> = vec![
        vec!["references"],
        vec![
            "references",
            "--symbol-id",
            "0123456789abcdef",
            "--handle",
            &handle,
            "--byte-offset",
            "1",
        ],
        vec!["references", "--handle", &handle],
        vec!["references", "--byte-offset", "3"],
        vec![
            "references",
            "--symbol-id",
            "0123456789abcdef",
            "--limit",
            "0",
        ],
        vec![
            "references",
            "--symbol-id",
            "0123456789abcdef",
            "--tokens",
            "0",
        ],
        vec!["references", "--symbol-id", "NOTHEX"],
        vec!["import-scip", "--index", "x"],
    ];
    for args in invalid {
        let out = cli(&world.store, &args);
        assert_eq!(
            out.status.code(),
            Some(2),
            "{args:?}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        assert!(out.stdout.is_empty());
    }
    let missing = cli(
        &world.store,
        &[
            "import-scip",
            "--index",
            "/nonexistent/index.scip",
            "--snapshot",
            manifest.to_str().unwrap(),
        ],
    );
    assert_eq!(
        (missing.status.code(), stderr_code(&missing).as_str()),
        (Some(2), "artifact_unavailable")
    );
    // symbol_not_found is a named runtime failure (exit 1).
    let unknown = cli(
        &world.store,
        &["references", "--symbol-id", "0123456789abcdef"],
    );
    assert_eq!(
        (unknown.status.code(), stderr_code(&unknown).as_str()),
        (Some(1), "symbol_not_found")
    );

    // Editing a third file and re-indexing makes the facts stale; the next
    // import at the new revision restores them.
    let lib = std::fs::read_to_string(world.ws.join("src/lib.rs")).unwrap();
    std::fs::write(world.ws.join("src/lib.rs"), format!("{lib}// edited\n")).unwrap();
    assert!(
        cli(&world.store, &["index", world.ws.to_str().unwrap()])
            .status
            .success()
    );
    let stale = cli(&world.store, &["references", "--symbol-id", &a_id[..16]]);
    let text = String::from_utf8(stale.stdout).unwrap();
    assert!(
        text.lines().next().unwrap().ends_with("coverage:stale"),
        "{text}"
    );
    assert_eq!(parse_v2(&text).unwrap().items.len(), 0);
    // The old manifest is now a stale artifact (exit 1).
    let old = world.import(&manifest);
    assert_eq!(
        (old.status.code(), stderr_code(&old).as_str()),
        (Some(1), "stale_artifact")
    );
    let fresh = world.manifest(|_| {});
    assert!(world.import(&fresh).status.success());
    let restored = cli(&world.store, &["references", "--symbol-id", &a_id[..16]]);
    let text = String::from_utf8(restored.stdout).unwrap();
    assert!(
        text.lines().next().unwrap().ends_with("coverage:complete"),
        "{text}"
    );
}

#[test]
fn the_cli_exit_codes_for_partial_cancelled_busy_and_refused_imports() {
    let world = CliWorld::new();
    // Unbound and stale manifests are named failures (exit 1).
    let unbound = world.manifest(|m| m["workspace_id"] = json!(digest(b"elsewhere")));
    let out = world.import(&unbound);
    assert_eq!(
        (out.status.code(), stderr_code(&out).as_str()),
        (Some(1), "unbound_artifact")
    );
    let stale = world.manifest(|m| m["artifact_sha256"] = json!(digest(b"not this one")));
    let out = world.import(&stale);
    assert_eq!(
        (out.status.code(), stderr_code(&out).as_str()),
        (Some(1), "stale_artifact")
    );

    // Cooperative cancellation between documents: exit 130 with the
    // committed counts on stdout.
    let manifest = world.manifest(|_| {});
    let out = cli_faulted(
        &world.store,
        &[
            "import-scip",
            "--index",
            world.artifact.to_str().unwrap(),
            "--snapshot",
            manifest.to_str().unwrap(),
        ],
        "ctxfoundry-fault/scip.between_documents=cancel:2",
    );
    assert_eq!(
        (out.status.code(), stderr_code(&out).as_str()),
        (Some(130), "cancelled")
    );
    let report = stdout_json(&out);
    assert_eq!(report["interrupted"], "cancelled");
    assert_eq!(report["complete"], false);
    let committed = report["completed"].as_u64().unwrap() + report["unresolved"].as_u64().unwrap();
    assert_eq!(committed, 2, "{report}");

    // A partial import (one malformed document) exits 1 with the report.
    let broken = artifact(vec![
        doc("src/lib.rs", vec![occ(&[0, 0, 99], ALPHA, DEF)]),
        doc("src/a.rs", vec![occ(&[2, 7, 19], ALPHA, DEF)]),
    ]);
    std::fs::write(&world.artifact, &broken).unwrap();
    let manifest = world.manifest(|_| {});
    let out = world.import(&manifest);
    assert_eq!(
        (out.status.code(), stderr_code(&out).as_str()),
        (Some(1), "import_incomplete")
    );
    let report = stdout_json(&out);
    assert_eq!(
        (report["failed"].as_u64(), report["complete"].as_bool()),
        (Some(1), Some(false))
    );
    assert_eq!(report["failure_samples"][0]["code"], "invalid_range");
    assert_eq!(report["failure_samples"][0]["path"], "src/lib.rs");

    // A competing owner makes the import and the query store_busy (exit 3).
    let owner = Engine::open_existing(&world.store).unwrap();
    let out = world.import(&manifest);
    assert_eq!(
        (out.status.code(), stderr_code(&out).as_str()),
        (Some(3), "store_busy")
    );
    let out = cli(
        &world.store,
        &["references", "--symbol-id", "0123456789abcdef"],
    );
    assert_eq!(
        (out.status.code(), stderr_code(&out).as_str()),
        (Some(3), "store_busy")
    );
    drop(owner);
}

#[test]
fn an_aborted_import_leaves_consistent_scopes_a_named_partial_snapshot_and_a_resumable_replay() {
    let world = CliWorld::new();
    let manifest = world.manifest(|_| {});
    let out = cli_faulted(
        &world.store,
        &[
            "import-scip",
            "--index",
            world.artifact.to_str().unwrap(),
            "--snapshot",
            manifest.to_str().unwrap(),
        ],
        "ctxfoundry-fault/scip.scope_before_commit=abort:2",
    );
    assert!(
        out.status.code().is_none(),
        "the process was killed: {:?}",
        out.status
    );
    {
        // Two scopes committed; the third was built but never committed.
        let engine = Engine::open_existing(&world.store).unwrap();
        let selected = engine
            .compiler_producers()
            .unwrap()
            .into_iter()
            .next()
            .and_then(|(_, row)| row.selected)
            .unwrap();
        assert_eq!(
            selected.state,
            SnapshotState::Importing,
            "named partial snapshot"
        );
        let a_id = symbol_id(RA, "src/a.rs", A_SYMBOL);
        let partial = engine
            .references(&ReferencesRequest {
                seed: ReferencesSeed::SymbolId(a_id[..16].to_owned()),
                limit: 64,
                after: None,
            })
            .unwrap();
        assert_eq!(partial.coverage, Coverage::Partial);
    }
    testkit::compiler_consistency(&world.store).unwrap();
    assert_eq!(
        testkit::table_rows(&world.store, "compiler_scopes").len(),
        2
    );
    // The aborted run's scratch directory is still there, positively owned.
    let area = world.store.join("import-scratch");
    let leftovers = entries(&area);
    assert_eq!(leftovers.len(), 1, "{leftovers:?}");
    assert!(area.join(&leftovers[0]).join("OWNER").is_file());
    // Replay resumes the same snapshot: idempotent, complete, leftovers gone.
    let out = world.import(&manifest);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let report = stdout_json(&out);
    assert_eq!(report["complete"], true);
    assert!(entries(&area).is_empty());
    testkit::compiler_consistency(&world.store).unwrap();
    assert_eq!(
        testkit::table_rows(&world.store, "compiler_scopes").len(),
        7
    );
}

#[test]
fn only_a_completed_manifest_retires_scopes_of_sources_it_proves_absent() {
    let mut world = toy3();
    world.import_ok(RA, "retire", &toy3_artifact());
    assert_eq!(world.raw()["compiler_scopes"].len(), 3);
    // `src/third.rs` leaves the workspace; the next artifact lacks it.
    world.remove_source("src/third.rs");
    let two = artifact(vec![
        doc(
            "src/lib.rs",
            vec![
                occ(&[0, 7, 12], ALPHA, DEF),
                occ(&[2, 4, 9], ALPHA, REF),
                occ(&[3, 4, 9], ALPHA, REF),
            ],
        ),
        doc("src/other.rs", vec![occ(&[1, 11, 16], ALPHA, REF)]),
    ]);
    let manifest = world.manifest(RA, "2026-08-31", "retire", &two);
    let (index, snapshot) = world.write_pair(&manifest, &two);
    // A cancelled run never infers absence: the old scope is still stored.
    let control = Control::unbounded();
    fault::arm(names::SCIP_BETWEEN_DOCUMENTS, 1, Action::Cancel);
    let report = world
        .engine()
        .import_scip(&index, &snapshot, &control)
        .unwrap();
    fault::disarm_all();
    assert_eq!(report.interrupted.as_deref(), Some("cancelled"));
    assert_eq!(report.retired, 0);
    assert!(scope_keys(&world.raw()).contains(&"rust-analyzer|src/third.rs".to_owned()));
    // The completed manifest proves the source absent: exactly that scope and
    // its reverse entries are retired.
    let report = world
        .import_files(&index, &snapshot, &ImportLimits::default())
        .unwrap();
    assert!(report.complete && report.retired == 1, "{report:?}");
    let raw = world.raw();
    assert_eq!(
        scope_keys(&raw),
        vec!["rust-analyzer|src/lib.rs", "rust-analyzer|src/other.rs"]
    );
    assert!(
        rows_of(
            &raw,
            "compiler_occurrences",
            "rust-analyzer\0src/third.rs\0"
        )
        .is_empty()
    );
    world.close();
    testkit::compiler_consistency(&world.store).unwrap();
}

#[test]
fn cancellation_and_deadlines_stop_the_copy_between_buffers() {
    let mut world = toy();
    // Four 1 MiB buffers of (invalid, never parsed) artifact bytes.
    let big = vec![0u8; 3 * 1024 * 1024 + 1];
    let manifest = world.manifest(RA, "2026-08-31", "copy", &big);
    let (index, snapshot) = world.write_pair(&manifest, &big);
    let before = without_producers(world.raw());
    fault::disarm_all();
    let control = Control::unbounded();
    // Hit 0 is the manifest's only buffer; cancel at the artifact's second.
    fault::arm(names::SCIP_COPY_BUFFER, 2, Action::Cancel);
    let error = world
        .engine()
        .import_scip(&index, &snapshot, &control)
        .unwrap_err();
    let copy_buffers = fault::reached(names::SCIP_COPY_BUFFER);
    fault::disarm_all();
    assert_eq!((error.code(), error.exit_code()), ("cancelled", 130));
    assert_eq!(copy_buffers, 3, "the copy stopped before its third buffer");
    assert!(entries(&scratch_area(&world)).is_empty());
    assert_eq!(without_producers(world.raw()), before);
    // A control already cancelled or past its deadline never starts.
    let engine = world.engine();
    let error = engine
        .import_scip(&index, &snapshot, &Control::cancelled())
        .unwrap_err();
    assert_eq!(error.code(), "cancelled");
    let expired = Control::with_deadline(std::time::Instant::now());
    let error = engine.import_scip(&index, &snapshot, &expired).unwrap_err();
    assert_eq!(
        (error.code(), error.exit_code()),
        ("deadline_exceeded", 130)
    );
    assert!(world.selected(RA).is_none());
}

#[test]
fn manifests_stream_in_batches_of_128_rows_and_still_catch_boundary_duplicates() {
    let mut world = toy();
    for i in 0..300 {
        world.set_source(&format!("notes/n{i:03}.txt"), &format!("note {i}\n"));
    }
    let bytes = toy_artifact();
    // 302 inputs span three 128-row batches; the 300 non-`.rs` paths are
    // outside the rust-analyzer profile.
    let report = world.import_ok(RA, "batches", &bytes);
    assert!(report.complete, "{report:?}");
    assert_eq!((report.outside_scope, report.accepted_empty), (300, 0));
    // A row repeated across the first batch boundary (rows 127 and 128).
    let e = world.refuse(&bytes, |m| {
        let inputs = m["inputs"].as_array_mut().unwrap();
        let row = inputs[127].clone();
        inputs.insert(128, row);
    });
    assert_eq!(e.code(), code::DUPLICATE_INPUT);
    // A repeat after a flushed batch, out of order.
    let e = world.refuse(&bytes, |m| {
        let inputs = m["inputs"].as_array_mut().unwrap();
        let row = inputs[3].clone();
        inputs.insert(200, row);
    });
    assert_eq!(e.code(), code::DUPLICATE_INPUT);
    // A stale hash in a later page is still found.
    let e = world.refuse(&bytes, |m| {
        m["inputs"][290]["sha256"] = json!(digest(b"edited"))
    });
    assert_eq!(e.code(), code::STALE_ARTIFACT);
}

#[test]
fn positions_are_utf8_byte_offsets_across_crlf_lines_and_multibyte_text() {
    // Bytes: line 0 `a\r\n` = 0..3; line 1 `let é = 😀;\r\n` = 3..19 (é 7..9,
    // 😀 12..16); line 2 `let x = é;\n` = 19..30 (é 27..29).
    const SOURCE: &str = "a\r\nlet é = 😀;\r\nlet x = é;\n";
    const E: &str = "rust-analyzer cargo t 0 e().";
    let world = World::with_sources(&[("src/lib.rs", SOURCE)]);
    let good = artifact(vec![doc(
        "src/lib.rs",
        vec![
            occ(&[1, 4, 6], E, DEF),
            occ(&[2, 8, 10], E, REF),
            // A range from line 0 to line 1: multi-line, ends before `é`.
            occ(&[0, 0, 1, 4], "rust-analyzer cargo t 0 span().", REF),
            // The end may sit on the line terminator itself (col 15, the LF).
            occ(&[1, 14, 15], "rust-analyzer cargo t 0 semi().", REF),
        ],
    )]);
    let report = world.import_ok(RA, "unicode", &good);
    assert!(report.complete, "{report:?}");
    let id = symbol_id(RA, "src/lib.rs", E);
    assert_eq!(
        world.starts(&id),
        vec![("src/lib.rs".to_owned(), 27)],
        "the reference is bytes 27..29, not the UTF-16 or char column"
    );
    let outcome = world.by_symbol(&id);
    assert_eq!(
        (
            outcome.items[0].start,
            outcome.items[0].end,
            outcome.items[0].line
        ),
        (27, 29, 3)
    );
    // Each of these splits a codepoint or leaves the line: the document fails.
    for (name, range) in [
        ("inside the emoji", [1, 4, 11]),
        ("inside é", [1, 5, 6]),
        ("past the LF", [1, 14, 16]),
        ("past the last line", [3, 0, 1]),
    ] {
        let bad = artifact(vec![doc("src/lib.rs", vec![occ(&range, E, REF)])]);
        let report = world.import_ok(RA, name, &bad);
        assert_eq!(report.failed, 1, "{name}");
        assert_eq!(
            report.failure_samples[0].code,
            code::INVALID_RANGE,
            "{name}"
        );
    }
    // The final empty line (after the last LF) is addressable by an end
    // position at column 0 of line 3.
    let at_end = artifact(vec![doc("src/lib.rs", vec![occ(&[2, 0, 3, 0], E, REF)])]);
    assert!(world.import_ok(RA, "eof", &at_end).complete);
}

#[test]
fn non_laminar_overlapping_ranges_resolve_to_the_narrowest_one() {
    // A = [0,10) span 10, B = [2,100) span 98: at offset 5 both contain it
    // and the NARROWER range wins, not the later start.
    const A: &str = "rust-analyzer cargo toy 0.1.0 a().";
    const B: &str = "rust-analyzer cargo toy 0.1.0 b().";
    let world = World::with_sources(&[
        ("src/lib.rs", &format!("{}\n", "x".repeat(120))),
        ("src/other.rs", OTHER),
    ]);
    let bytes = artifact(vec![
        doc(
            "src/lib.rs",
            vec![occ(&[0, 0, 10], A, REF), occ(&[0, 2, 100], B, REF)],
        ),
        doc(
            "src/other.rs",
            vec![occ(&[0, 7, 12], A, DEF), occ(&[0, 7, 12], B, DEF)],
        ),
    ]);
    world.import_ok(RA, "overlap", &bytes);
    let seed = |offset: u64| ReferencesSeed::Position {
        handle: world.whole_file_handle("src/lib.rs"),
        byte_offset: offset,
    };
    let at_five = world.references(seed(5), 64, None).unwrap();
    assert_eq!(at_five.symbol_id, Some(symbol_id(RA, "src/other.rs", A)));
    // Past A's end only B contains the offset.
    let at_fifty = world.references(seed(50), 64, None).unwrap();
    assert_eq!(at_fifty.symbol_id, Some(symbol_id(RA, "src/other.rs", B)));
}

#[test]
fn three_hundred_same_start_references_paginate_one_by_one_losslessly() {
    // 300 references of one symbol that all START at byte 0 and end at
    // 1..=300: the cursor's `end` disambiguates, `limit 1` pages through them
    // one record at a time, and nothing is skipped or duplicated.
    let world = World::with_sources(&[
        ("src/lib.rs", &format!("{}\n", "x".repeat(300))),
        ("src/other.rs", "pub fn d() {}\n"),
    ]);
    let occurrences: Vec<Occurrence> = (1..=300).map(|end| occ(&[0, 0, end], HUB, REF)).collect();
    let bytes = artifact(vec![
        doc("src/lib.rs", occurrences),
        doc("src/other.rs", vec![occ(&[0, 7, 8], HUB, DEF)]),
    ]);
    let report = world.import_ok(RA, "same-start", &bytes);
    assert!(report.complete && report.references == 300, "{report:?}");
    let hub = symbol_id(RA, "src/other.rs", HUB);
    let mut after: Option<String> = None;
    let (mut ends, mut pages) = (Vec::new(), 0);
    loop {
        let outcome = world
            .references(
                ReferencesSeed::SymbolId(hub[..16].to_owned()),
                1,
                after.as_deref(),
            )
            .unwrap();
        assert_eq!(outcome.items.len(), 1, "limit 1 holds per record");
        pages += 1;
        ends.push(outcome.items[0].end);
        match outcome.resume {
            Some(cursor) if outcome.more => after = Some(cursor),
            None if !outcome.more => break,
            other => panic!("inconsistent continuation {other:?}"),
        }
    }
    assert_eq!(pages, 300);
    assert_eq!(ends, (1..=300).collect::<Vec<u64>>());
}

#[test]
fn the_position_seeds_own_source_counts_toward_the_shared_file_budget() {
    // The seed file and the definition file join the same visited-file budget
    // as every reference file, so the 64-file window stops one file earlier
    // than a naive count of reference files alone.
    let sources = vec![("src/s.rs", "h\n"), ("src/d.rs", "pub fn d() {}\n")];
    let world = World::blank();
    for (path, body) in &sources {
        world.set_source(path, body);
    }
    for n in 1..=65 {
        world.set_source(&format!("src/r{n:02}.rs"), "pub fn q() { h(); }\n");
    }
    let mut documents: Vec<Document> = (1..=65)
        .map(|n| {
            doc(
                &format!("src/r{n:02}.rs"),
                vec![occ(&[0, 13, 14], HUB, REF)],
            )
        })
        .collect();
    documents.insert(0, doc("src/s.rs", vec![occ(&[0, 0, 1], HUB, REF)]));
    documents.push(doc("src/d.rs", vec![occ(&[0, 7, 8], HUB, DEF)]));
    world.import_ok(RA, "budget", &artifact(documents));
    let outcome = world
        .references(
            ReferencesSeed::Position {
                handle: world.whole_file_handle("src/s.rs"),
                byte_offset: 0,
            },
            256,
            None,
        )
        .unwrap();
    // Visited: the seed file, the definition file and 62 reference files.
    assert_eq!(outcome.items.len(), 62);
    assert!(outcome.more && outcome.candidates_full);
    let resume = outcome.resume.unwrap();
    assert!(resume.starts_with("src/r62.rs#"), "{resume}");
    // Continuing from the cursor delivers the remaining reference files.
    let hub = symbol_id(RA, "src/d.rs", HUB);
    let rest = world
        .references(
            ReferencesSeed::SymbolId(hub[..16].to_owned()),
            256,
            Some(&resume),
        )
        .unwrap();
    assert_eq!(rest.items.len(), 4, "r63..r65 and s.rs");
}

#[test]
fn a_failed_document_never_establishes_resolution() {
    // x.rs defines X but carries a malformed range, so the whole document
    // fails validation: its definition must not enter the lookup, and the
    // reference in y.rs stays unresolved.
    const X: &str = "rust-analyzer cargo toy 0.1.0 x().";
    let mut world = World::with_sources(&[
        ("src/x.rs", "pub fn f() {}\n"),
        ("src/y.rs", "pub fn g() {}\n"),
    ]);
    let bytes = artifact(vec![
        doc(
            "src/x.rs",
            vec![occ(&[0, 7, 8], X, DEF), occ(&[0, 0, 99], X, REF)],
        ),
        doc("src/y.rs", vec![occ(&[0, 7, 8], X, REF)]),
    ]);
    let report = world.import_ok(RA, "failed-def", &bytes);
    assert_eq!(report.failed, 1);
    assert_eq!(report.failure_samples[0].path, "src/x.rs");
    assert!(!report.complete);
    assert_eq!(report.coverage, "partial");
    assert_eq!(report.unresolved_references, 1);
    let raw = world.raw();
    let y = scope_json(&raw, RA, "src/y.rs");
    assert_eq!(y["status"], "partial");
    assert_eq!(y["unresolved"], 1);
    let outcome = world.by_symbol(&symbol_id(RA, "src/x.rs", X));
    assert_eq!(outcome.target, Some(TargetResolution::Unknown));
    assert_eq!((outcome.items.len(), outcome.unresolved), (1, 1));
    assert!(outcome.items[0].edge_id.is_none());
    assert_eq!(outcome.coverage, Coverage::Partial);
}

#[test]
fn no_scope_retirement_while_any_document_failed() {
    let mut world = toy3();
    world.import_ok(RA, "retire", &toy3_artifact());
    world.remove_source("src/third.rs");
    // The completed manifest lacks third.rs, but another document fails: a
    // run with a failed document may not conclude any source is absent.
    let broken = artifact(vec![
        doc("src/lib.rs", vec![occ(&[0, 7, 99], ALPHA, DEF)]),
        doc("src/other.rs", vec![occ(&[1, 11, 16], ALPHA, REF)]),
    ]);
    let report = world.import_ok(RA, "broken", &broken);
    assert_eq!(report.failed, 1);
    assert_eq!(report.retired, 0);
    assert!(scope_keys(&world.raw()).contains(&"rust-analyzer|src/third.rs".to_owned()));
    // A clean run over the same manifest retires exactly that scope.
    let clean = artifact(vec![
        doc("src/lib.rs", vec![occ(&[0, 7, 12], ALPHA, DEF)]),
        doc("src/other.rs", vec![occ(&[1, 11, 16], ALPHA, REF)]),
    ]);
    let report = world.import_ok(RA, "clean", &clean);
    assert!(report.complete && report.retired == 1, "{report:?}");
    assert_eq!(
        scope_keys(&world.raw()),
        vec!["rust-analyzer|src/lib.rs", "rust-analyzer|src/other.rs"]
    );
}

#[test]
fn corrupted_compiler_rows_and_definition_chunks_are_named_never_panics() {
    // A non-canonical definition key inside the scanned range.
    let mut world = toy();
    world.import_ok(RA, "corrupt-key", &toy_artifact());
    let id = alpha_id();
    world.close();
    testkit::write_store(&world.store, |tx| {
        let mut table = tx
            .open_table(redb::TableDefinition::<&str, &str>::new(
                "compiler_by_symbol",
            ))
            .unwrap();
        table
            .insert(
                format!("{id}\0d\0src/lib.rs\0not-canonical\0also-not").as_str(),
                RA,
            )
            .unwrap();
    });
    world.reopen();
    let error = world.by_symbol_err(&id[..16]);
    assert_eq!(error.code(), "graph_invalid");

    // A non-canonical occurrence key inside a position scan.
    let mut world = toy();
    world.import_ok(RA, "corrupt-occ", &toy_artifact());
    world.close();
    testkit::write_store(&world.store, |tx| {
        let mut table = tx
            .open_table(redb::TableDefinition::<&str, &str>::new(
                "compiler_occurrences",
            ))
            .unwrap();
        table
            .insert(
                format!("{RA}\0src/lib.rs\0{:020}\0{:019}\0r\0{id}", 38, 43).as_str(),
                "{}",
            )
            .unwrap();
    });
    world.reopen();
    let error = world
        .references(
            ReferencesSeed::Position {
                handle: world.whole_file_handle("src/lib.rs"),
                byte_offset: 40,
            },
            64,
            None,
        )
        .unwrap_err();
    assert_eq!(error.code(), "graph_invalid");

    // A definition source whose stored chunks no longer verify.
    let mut world = toy();
    world.import_ok(RA, "corrupt-chunks", &toy_artifact());
    world.close();
    testkit::remove_chunk(&world.store, "src/lib.rs", 0);
    world.reopen();
    let error = world.by_symbol_err(&alpha_id()[..16]);
    assert_eq!(error.code(), "corrupt_source");
}

#[test]
#[cfg(unix)]
fn a_fifo_without_a_writer_is_refused_without_blocking() {
    let world = toy();
    let bytes = toy_artifact();
    let manifest = world.manifest(RA, "2026-08-31", "fifo", &bytes);
    let snapshot = world.fresh_name("manifest");
    std::fs::write(&snapshot, serde_json::to_vec(&manifest).unwrap()).unwrap();
    let fifo = world.fresh_name("fifo");
    let name = std::ffi::CString::new(fifo.as_os_str().as_encoded_bytes()).unwrap();
    let code = unsafe { libc::mkfifo(name.as_ptr(), 0o600) };
    assert_eq!(code, 0, "{}", std::io::Error::last_os_error());
    let error = world
        .engine()
        .import_scip(&fifo, &snapshot, &Control::unbounded())
        .unwrap_err();
    assert_eq!(error.code(), "artifact_unavailable");
}

#[test]
fn a_compact_oversized_occurrence_count_is_refused_before_materializing() {
    // 16,385 empty occurrence submessages serialize to well under the 8 MiB
    // message cap: only the streamed count refuses the document, before the
    // occurrences are ever materialized.
    let world = World::with_sources(&[("src/lib.rs", "pub fn f() {}\n")]);
    let over = {
        let mut document = doc("src/lib.rs", Vec::new());
        document.occurrences = vec![Occurrence::default(); 16_385];
        artifact(vec![document])
    };
    assert!(
        over.len() < 1 << 20,
        "{} bytes is far under the message cap",
        over.len()
    );
    let report = world.import_ok(RA, "compact-over", &over);
    assert_eq!(report.failed, 1);
    assert_eq!(report.failure_samples[0].code, code::DOCUMENT_TOO_LARGE);
    assert_eq!(report.failure_samples[0].path, "src/lib.rs");
    assert!(world.by_symbol_or_none(&alpha_id()).is_none());
    // Exactly at the cap the count check passes: the same compact document
    // is decoded and then fails on its own (empty) ranges, not on its size.
    let at = {
        let mut document = doc("src/lib.rs", Vec::new());
        document.occurrences = vec![Occurrence::default(); 16_384];
        artifact(vec![document])
    };
    let report = world.import_ok(RA, "compact-at", &at);
    assert_eq!(report.failed, 1);
    assert_eq!(report.failure_samples[0].code, code::INVALID_RANGE);
}

// ---------------------------------------------------------------------------
// Review round 3: stored-data panics, atomic retirement, budget cliffs,
// cap-before-decode, coverage, slim decoding and the ordinal label.
// ---------------------------------------------------------------------------

/// A thread-local peak-allocation probe: proves what the importer did NOT
/// materialize, not only which code it returned. Measuring is per thread, so
/// the parallel tests of this binary do not disturb each other.
mod alloc_probe {
    use std::alloc::{GlobalAlloc, Layout, System};
    use std::cell::Cell;

    thread_local! {
        static LIVE: Cell<i64> = const { Cell::new(0) };
        static PEAK: Cell<i64> = const { Cell::new(0) };
    }

    pub struct Probe;

    fn note(delta: i64) {
        let _ = LIVE.try_with(|live| {
            let now = live.get() + delta;
            live.set(now);
            let _ = PEAK.try_with(|peak| {
                if now > peak.get() {
                    peak.set(now);
                }
            });
        });
    }

    // SAFETY: every method forwards to the system allocator unchanged; the
    // bookkeeping only touches const-initialized thread-locals.
    unsafe impl GlobalAlloc for Probe {
        unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
            let pointer = unsafe { System.alloc(layout) };
            if !pointer.is_null() {
                note(layout.size() as i64);
            }
            pointer
        }

        unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
            let pointer = unsafe { System.alloc_zeroed(layout) };
            if !pointer.is_null() {
                note(layout.size() as i64);
            }
            pointer
        }

        unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
            unsafe { System.dealloc(pointer, layout) };
            note(-(layout.size() as i64));
        }

        unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
            let moved = unsafe { System.realloc(pointer, layout, new_size) };
            if !moved.is_null() {
                note(new_size as i64 - layout.size() as i64);
            }
            moved
        }
    }

    /// Run `f`; return its value and the peak bytes it held live above the
    /// level at entry, on this thread.
    pub fn peak_during<R>(f: impl FnOnce() -> R) -> (R, i64) {
        let start = LIVE.get();
        PEAK.set(start);
        let value = f();
        (value, PEAK.get() - start)
    }
}

#[global_allocator]
static ALLOCATOR: alloc_probe::Probe = alloc_probe::Probe;

fn varint(mut n: usize) -> Vec<u8> {
    let mut out = Vec::new();
    loop {
        let byte = (n & 0x7f) as u8;
        n >>= 7;
        if n == 0 {
            out.push(byte);
            return out;
        }
        out.push(byte | 0x80);
    }
}

/// An `Index` whose `documents` are the given raw document messages.
fn raw_index(messages: &[Vec<u8>]) -> Vec<u8> {
    let mut out = Vec::new();
    for message in messages {
        out.push(0x12);
        out.extend(varint(message.len()));
        out.extend(message);
    }
    out
}

/// `relative_path` (field 1) as raw bytes.
fn raw_path(path: &str) -> Vec<u8> {
    let mut out = vec![0x0a];
    out.extend(varint(path.len()));
    out.extend(path.as_bytes());
    out
}

#[test]
fn a_definition_whose_source_is_gone_or_rehashed_is_a_stale_drop_not_a_panic() {
    for corruption in ["removed", "rehashed"] {
        let mut world = toy();
        world.import_ok(RA, "def-source", &toy_artifact());
        world.close();
        testkit::write_store(&world.store, |tx| {
            let mut sources = tx
                .open_table(redb::TableDefinition::<&str, &str>::new("sources"))
                .unwrap();
            if corruption == "removed" {
                sources.remove("src/lib.rs").unwrap();
            } else {
                let raw = sources
                    .get("src/lib.rs")
                    .unwrap()
                    .unwrap()
                    .value()
                    .to_owned();
                let mut row: Value = serde_json::from_str(&raw).unwrap();
                row["hash"] = json!("0".repeat(64));
                sources
                    .insert("src/lib.rs", row.to_string().as_str())
                    .unwrap();
            }
        });
        world.reopen();
        // The source revision is unchanged: only the stored rows disagree.
        let outcome = world.by_symbol(&alpha_id());
        // The one definition (and lib.rs's two references) cannot be
        // verified: all three are stale drops, never a unique target.
        assert_eq!(
            outcome.target,
            Some(TargetResolution::Unknown),
            "{corruption}"
        );
        assert!(outcome.definitions.is_empty(), "{corruption}");
        assert_eq!(outcome.stale, 3, "{corruption}");
        assert_eq!(outcome.coverage, Coverage::Partial, "{corruption}");
        assert_eq!(
            outcome
                .items
                .iter()
                .map(|i| i.path.as_str())
                .collect::<Vec<_>>(),
            vec!["src/other.rs"],
            "{corruption}"
        );
        assert!(outcome.items[0].edge_id.is_none());
        let parsed = parse_v2(&world.text(&outcome, 1024)).unwrap();
        assert!(
            parsed.header.contains(&"stale:3".to_owned()),
            "{corruption}"
        );
    }
}

#[test]
fn short_stored_symbol_ids_are_graph_invalid_never_a_slice_panic() {
    let mut world = toy();
    world.import_ok(RA, "short-ids", &toy_artifact());
    world.close();
    testkit::write_store(&world.store, |tx| {
        let mut table = tx
            .open_table(redb::TableDefinition::<&str, &str>::new(
                "compiler_occurrences",
            ))
            .unwrap();
        // Two more symbols on the very range of alpha's first reference
        // [38,43), with ids that are not 64 lowercase hex.
        for short in ["a", "b"] {
            table
                .insert(
                    format!("{RA}\0src/lib.rs\0{:020}\0{:020}\0r\0{short}", 38, 43).as_str(),
                    "{}",
                )
                .unwrap();
        }
    });
    world.reopen();
    let error = world
        .references(
            ReferencesSeed::Position {
                handle: world.whole_file_handle("src/lib.rs"),
                byte_offset: 40,
            },
            64,
            None,
        )
        .unwrap_err();
    assert_eq!(error.code(), "graph_invalid");
    // The same ids in the prefix index are corruption too.
    world.close();
    testkit::write_store(&world.store, |tx| {
        let mut table = tx
            .open_table(redb::TableDefinition::<&str, &str>::new(
                "compiler_by_symbol",
            ))
            .unwrap();
        table
            .insert(
                format!("{}zz\0d\0src/lib.rs\0{:020}\0{:020}", "0".repeat(62), 0, 1).as_str(),
                RA,
            )
            .unwrap();
    });
    world.reopen();
    assert_eq!(
        world.by_symbol_err("0000000000000000").code(),
        "graph_invalid"
    );
}

#[test]
fn a_cancelled_finalization_retires_nothing_and_a_clean_run_retires_exactly_the_absent_scopes() {
    for point in [
        names::SCIP_RETIRE_PROPOSED,
        names::SCIP_FINALIZE_BEFORE_COMMIT,
    ] {
        let mut world = toy3();
        world.import_ok(RA, "fin", &toy3_artifact());
        assert_eq!(world.raw()["compiler_scopes"].len(), 3);
        // other.rs and third.rs leave the workspace: the next completed
        // manifest proves BOTH scopes absent.
        world.remove_source("src/other.rs");
        world.remove_source("src/third.rs");
        let lib_only = artifact(vec![doc(
            "src/lib.rs",
            vec![occ(&[0, 7, 12], ALPHA, DEF), occ(&[2, 4, 9], ALPHA, REF)],
        )]);
        let manifest = world.manifest(RA, "2026-08-31", "fin", &lib_only);
        let (index, snapshot) = world.write_pair(&manifest, &lib_only);
        fault::arm(point, 0, Action::Cancel);
        let report = world
            .engine()
            .import_scip(&index, &snapshot, &Control::unbounded())
            .unwrap();
        fault::disarm_all();
        assert_eq!(report.interrupted.as_deref(), Some("cancelled"), "{point}");
        assert!(!report.complete && report.coverage == "partial", "{point}");
        assert_eq!(report.retired, 0, "{point}: the report is truthful");
        // All-or-none: not even the first proposed retirement survived.
        let raw = world.raw();
        assert_eq!(
            scope_keys(&raw),
            vec![
                "rust-analyzer|src/lib.rs",
                "rust-analyzer|src/other.rs",
                "rust-analyzer|src/third.rs"
            ],
            "{point}"
        );
        let latest = world.latest(RA).unwrap();
        assert_eq!((latest.retired, latest.complete), (0, false), "{point}");
        assert_eq!(latest.interrupted.as_deref(), Some("cancelled"), "{point}");
        assert_eq!(world.selected(RA).unwrap().state, SnapshotState::Partial);
        // A clean replay retires exactly those two scopes and says so in
        // the same commit as the final state.
        let replay = world
            .import_files(&index, &snapshot, &ImportLimits::default())
            .unwrap();
        assert!(
            replay.complete && replay.retired == 2,
            "{point}: {replay:?}"
        );
        assert_eq!(scope_keys(&world.raw()), vec!["rust-analyzer|src/lib.rs"]);
        let latest = world.latest(RA).unwrap();
        assert_eq!((latest.retired, latest.complete), (2, true), "{point}");
        assert_eq!(world.selected(RA).unwrap().state, SnapshotState::Complete);
        world.close();
        testkit::compiler_consistency(&world.store).unwrap();
    }
}

const BIG: &str = "rust-analyzer cargo toy 0.1.0 Big#";

/// `src/lib.rs` is 300 short occurrences under one whole-file definition of
/// `Big`; `src/other.rs` references `Big` (and defines it again when
/// `second_definition`).
fn big_world(second_definition: bool) -> World {
    let world = World::with_sources(&[
        ("src/lib.rs", &long_line_source(300)),
        ("src/other.rs", OTHER),
    ]);
    let mut tokens: Vec<Occurrence> = (0..300).map(token_ref).collect();
    tokens.push(occ(&[0, 0, 1, 0], BIG, DEF));
    let mut other = vec![occ(&[1, 11, 16], BIG, REF)];
    if second_definition {
        other.push(occ(&[0, 7, 12], BIG, DEF));
    }
    world.import_ok(
        RA,
        "big",
        &artifact(vec![doc("src/lib.rs", tokens), doc("src/other.rs", other)]),
    );
    world
}

fn seed_at(world: &World, offset: u64) -> ReferencesSeed {
    ReferencesSeed::Position {
        handle: world.whole_file_handle("src/lib.rs"),
        byte_offset: offset,
    }
}

#[test]
fn a_unique_target_at_the_budget_edge_is_never_complete_and_empty() {
    // One Big definition. Round 2 spent 255 records finding the enclosing
    // definition and 1 proving it unique, left no record for the pending
    // reference and answered complete / zero items / no cursor: a false end.
    let world = big_world(false);
    let deep = world.references(seed_at(&world, 507), 64, None).unwrap();
    assert!(
        !(deep.coverage == Coverage::Complete && deep.items.is_empty() && !deep.more),
        "a false end: {deep:?}"
    );
    assert_eq!(deep.coverage, Coverage::Partial);
    assert!(deep.candidates_full && deep.definitions_truncated);
    assert!(deep.symbol_id.is_none(), "nothing was certified");
    let text = world.text(&deep, 1024);
    assert_eq!(
        parse_v2(&text).unwrap().header.last().unwrap(),
        "coverage:partial"
    );
}

#[test]
fn a_unique_target_found_with_exactly_the_reserve_left_still_makes_progress() {
    // 253 short occurrences and the whole-file definition: the containing
    // scan uses exactly 254 records and finishes, one record proves the
    // single definition unique and one delivers the first of two references.
    const BIG_LOCAL: &str = "rust-analyzer cargo toy 0.1.0 Edge#";
    let world = World::with_sources(&[
        ("src/lib.rs", &long_line_source(253)),
        ("src/other.rs", OTHER),
        ("src/third.rs", THIRD),
    ]);
    let mut tokens: Vec<Occurrence> = (0..253).map(token_ref).collect();
    tokens.push(occ(&[0, 0, 1, 0], BIG_LOCAL, DEF));
    world.import_ok(
        RA,
        "edge",
        &artifact(vec![
            doc("src/lib.rs", tokens),
            doc("src/other.rs", vec![occ(&[1, 11, 16], BIG_LOCAL, REF)]),
            doc("src/third.rs", vec![occ(&[1, 11, 16], BIG_LOCAL, REF)]),
        ]),
    );
    let outcome = world.references(seed_at(&world, 506), 64, None).unwrap();
    assert_eq!(outcome.target, Some(TargetResolution::Unique));
    assert_eq!(outcome.examined, 256);
    assert_eq!(outcome.items.len(), 1);
    assert!(outcome.more && outcome.resume.is_some() && outcome.candidates_full);
    assert_eq!(outcome.coverage, Coverage::Complete);
    let rest = world
        .references(
            ReferencesSeed::SymbolId(outcome.symbol_id.unwrap()[..16].to_owned()),
            64,
            outcome.resume.as_deref(),
        )
        .unwrap();
    assert_eq!(rest.items.len(), 1);
    assert_eq!(rest.items[0].path, "src/third.rs");
    assert!(!rest.more);
}

#[test]
fn a_stale_graph_stays_stale_for_a_deep_position_seed() {
    let world = big_world(true);
    // An unrelated source change: every compiler fact is ineligible. The
    // seed's deep occurrence scan must not get to relabel that as partial.
    world.set_source("README.md", "an unrelated edit\n");
    let deep = world.references(seed_at(&world, 507), 64, None).unwrap();
    assert_eq!(deep.coverage, Coverage::Stale);
    assert!(deep.items.is_empty() && !deep.more);
    assert_eq!(deep.examined, 0, "decided before any deep scan");
    let text = world.text(&deep, 1024);
    assert_eq!(
        parse_v2(&text).unwrap().header.last().unwrap(),
        "coverage:stale"
    );
}

#[test]
fn two_producers_share_one_seed_allowance() {
    const BETA: &str = "scip-b cargo toy 0.1.0 beta().";
    let world = World::with_sources(&[("src/lib.rs", &long_line_source(300))]);
    // 128 short occurrences plus one whole-file occurrence: the whole-file
    // span (601 bytes) keeps each producer's scan from stopping early, so
    // each producer's containing scan costs 129 records.
    let tokens = |symbol: &str, file: &str| -> Vec<Occurrence> {
        let mut all: Vec<Occurrence> = (0..=127)
            .map(|i| occ(&[0, 2 * i, 2 * i + 1], symbol, REF))
            .collect();
        all.push(occ(&[0, 0, 1, 0], file, DEF));
        all
    };
    const A_FILE: &str = "rust-analyzer cargo toy 0.1.0 afile#";
    const B_FILE: &str = "scip-b cargo toy 0.1.0 bfile#";
    world.import_ok(
        RA,
        "a",
        &artifact(vec![doc("src/lib.rs", tokens(ALPHA, A_FILE))]),
    );
    // Producer A alone: 129 containing candidates, resolved to its whole-file definition.
    let alone = world.references(seed_at(&world, 255), 64, None).unwrap();
    assert_eq!(alone.symbol_id, Some(symbol_id(RA, "src/lib.rs", A_FILE)));
    // Producer B adds 129 more over the same span. Fully scanned they would
    // tie (`ambiguous_symbol`); the shared 254-record scan allowance is
    // exhausted inside B, so the answer is truncated: no winner is
    // certified, no ambiguity invented, no absence claimed.
    world.import_ok(
        "scip-b",
        "b",
        &artifact(vec![doc("src/lib.rs", tokens(BETA, B_FILE))]),
    );
    let both = world.references(seed_at(&world, 255), 64, None).unwrap();
    assert_eq!(both.examined, 254);
    assert!(both.symbol_id.is_none() && both.target.is_none());
    assert_eq!(both.coverage, Coverage::Partial);
    assert!(both.definitions_truncated && both.candidates_full && !both.more);
}

#[test]
fn the_examined_cap_is_checked_before_the_next_key_is_decoded() {
    let mut world = hub_world();
    world.import_ok(RA, "cap", &hub_artifact(1, 300));
    let hub = symbol_id(RA, "src/lib.rs", HUB);
    // Reference #256 sits at byte 1024..1027. Corrupt only its key: the end
    // field loses its zero padding.
    world.close();
    testkit::write_store(&world.store, |tx| {
        let mut table = tx
            .open_table(redb::TableDefinition::<&str, &str>::new(
                "compiler_by_symbol",
            ))
            .unwrap();
        let good = format!("{hub}\0r\0src/lib.rs\0{:020}\0{:020}", 1024, 1027);
        let bad = format!("{hub}\0r\0src/lib.rs\0{:020}\0{:019}", 1024, 1027);
        table.remove(good.as_str()).unwrap();
        table.insert(bad.as_str(), RA).unwrap();
    });
    world.reopen();
    // One definition record plus 255 references fill the 256-record window;
    // the 256th reference exists but is NEVER decoded on this page.
    let page = world
        .references(ReferencesSeed::SymbolId(hub[..16].to_owned()), 256, None)
        .unwrap();
    assert_eq!((page.examined, page.items.len()), (256, 255));
    assert!(page.more && page.candidates_full);
    assert_eq!(page.resume.as_deref(), Some("src/lib.rs#1020-1023"));
    // The next page starts at the corrupt key and names it.
    let error = world
        .references(
            ReferencesSeed::SymbolId(hub[..16].to_owned()),
            256,
            page.resume.as_deref(),
        )
        .unwrap_err();
    assert_eq!(error.code(), "graph_invalid");
}

#[test]
fn non_rs_inputs_leave_import_coverage_unchanged() {
    let world = toy();
    let base = world.import_ok(RA, "plain", &toy_artifact());
    assert_eq!(
        (base.coverage.as_str(), base.outside_scope),
        ("complete", 0)
    );
    world.set_source("README.md", "notes\n");
    world.set_source("docs/guide.txt", "guide\n");
    let with = world.import_ok(RA, "plain", &toy_artifact());
    assert_eq!(with.outside_scope, 2, "informational count");
    assert_eq!(with.coverage, "complete", "coverage is unaffected");
    // Unknown coverage still degrades it: a producer without a profile
    // cannot call its absent documents empty or out of scope.
    let unknown = world.import_ok("scip-other", "generic", &toy_artifact());
    assert!(unknown.complete && unknown.unknown == 2);
    assert_eq!(unknown.coverage, "partial");
}

#[test]
fn an_unnamed_oversized_document_is_labeled_by_ordinal_and_never_treated_as_a_path() {
    // 16,385 empty occurrences and no relative_path at all.
    let unnamed: Vec<u8> = std::iter::repeat_n([0x12u8, 0x00], 16_385)
        .flatten()
        .collect();
    let bytes = raw_index(&[unnamed]);
    // The label is not a source: an unlisted "document #1" is no
    // unbound_artifact, the document just fails by that label.
    let world = toy();
    let report = world.import_ok(RA, "unnamed", &bytes);
    assert_eq!(report.failed, 1);
    assert_eq!(report.failure_samples[0].path, "document #1");
    assert_eq!(report.failure_samples[0].code, code::DOCUMENT_TOO_LARGE);
    // A manifest that literally lists a source called `document #1` is not
    // confused with it: that source is simply absent from the artifact.
    let literal = World::with_sources(&[("document #1", "plain text\n"), ("src/lib.rs", LIB)]);
    let report = literal.import_ok(RA, "literal", &bytes);
    assert_eq!(report.failed, 1);
    assert_eq!(report.failure_samples[0].path, "document #1");
    assert_eq!(report.outside_scope, 1, "the real source is not 'present'");
    assert_eq!(
        report.accepted_empty, 1,
        "src/lib.rs is absent from the artifact"
    );
    // The same shape as an unnamed oversized MESSAGE: same label.
    let mut padding = vec![0x2au8, 40];
    padding.extend(std::iter::repeat_n(b'z', 40));
    let tiny = ImportLimits {
        document_bytes: 32,
        ..ImportLimits::default()
    };
    let manifest = world.manifest(RA, "2026-08-31", "message", &raw_index(&[padding.clone()]));
    let (index, snapshot) = world.write_pair(&manifest, &raw_index(&[padding]));
    let report = world.import_files(&index, &snapshot, &tiny).unwrap();
    assert_eq!(report.failure_samples[0].path, "document #1");
}

#[test]
fn millions_of_ignored_symbol_messages_are_skipped_not_materialized() {
    let world = World::with_sources(&[("src/lib.rs", "pub fn f() {}\n")]);
    // path + 4,000,000 empty `symbols` messages (2 bytes each: ~8 MB, under
    // the 8 MiB message cap) + position_encoding UTF8.
    let mut message = raw_path("src/lib.rs");
    message.extend(std::iter::repeat_n([0x1au8, 0x00], 4_000_000).flatten());
    message.extend([0x30, 0x01]);
    assert!(message.len() < 8 << 20);
    let bytes = raw_index(&[message]);
    let manifest = world.manifest(RA, "2026-08-31", "symbols", &bytes);
    let (index, snapshot) = world.write_pair(&manifest, &bytes);
    let (result, peak) = alloc_probe::peak_during(|| {
        world
            .engine()
            .import_scip(&index, &snapshot, &Control::unbounded())
    });
    let report = result.unwrap();
    assert!(report.complete && report.accepted_empty == 1, "{report:?}");
    // Generated merging would hold 4,000,000 SymbolInformation structs
    // (hundreds of MB); the slim decoder holds the one message buffer.
    eprintln!("symbols: peak live bytes {peak}");
    assert!(peak < 40 << 20, "peak live bytes {peak}");
}

#[test]
fn millions_of_ignored_diagnostics_on_one_occurrence_are_skipped_not_materialized() {
    let world = World::with_sources(&[("src/lib.rs", "pub fn f() {}\n")]);
    const SYMBOL: &str = "rust-analyzer cargo toy 0.1.0 f().";
    // One definition occurrence carrying 3,900,000 empty diagnostics.
    let mut occurrence = vec![0x0a, 0x03, 0x00, 0x07, 0x08];
    occurrence.push(0x12);
    occurrence.extend(varint(SYMBOL.len()));
    occurrence.extend(SYMBOL.as_bytes());
    occurrence.extend([0x18, 0x01]);
    occurrence.extend(std::iter::repeat_n([0x32u8, 0x00], 3_900_000).flatten());
    let mut message = raw_path("src/lib.rs");
    message.push(0x12);
    message.extend(varint(occurrence.len()));
    message.extend(occurrence);
    message.extend([0x30, 0x01]);
    assert!(message.len() < 8 << 20);
    let bytes = raw_index(&[message]);
    let manifest = world.manifest(RA, "2026-08-31", "diagnostics", &bytes);
    let (index, snapshot) = world.write_pair(&manifest, &bytes);
    let (result, peak) = alloc_probe::peak_during(|| {
        world
            .engine()
            .import_scip(&index, &snapshot, &Control::unbounded())
    });
    let report = result.unwrap();
    assert!(report.complete && report.definitions == 1, "{report:?}");
    eprintln!("diagnostics: peak live bytes {peak}");
    assert!(peak < 40 << 20, "peak live bytes {peak}");
}

// --- M10: the seed scan stops at the narrowest eligible span -----------------

/// A file of `tokens` one-byte occurrences, each its own symbol, under a
/// whole-file module-style definition (rust-analyzer emits one per document).
fn module_world(tokens: usize) -> (World, Vec<String>) {
    let world = World::with_sources(&[("src/lib.rs", &long_line_source(tokens))]);
    let symbols: Vec<String> = (0..tokens)
        .map(|i| format!("rust-analyzer cargo toy 0.1.0 t{i}()."))
        .collect();
    let mut occurrences: Vec<Occurrence> = symbols
        .iter()
        .enumerate()
        .map(|(i, symbol)| occ(&[0, (2 * i) as i32, (2 * i + 1) as i32], symbol, REF))
        .collect();
    occurrences.push(occ(
        &[0, 0, 1, 0],
        "rust-analyzer cargo toy 0.1.0 module#",
        DEF,
    ));
    world.import_ok(
        RA,
        "module",
        &artifact(vec![doc("src/lib.rs", occurrences)]),
    );
    (world, symbols)
}

#[test]
fn a_deep_identifier_seed_stops_at_its_own_span_instead_of_scanning_the_whole_file() {
    // Round 2 could not stop before the whole-file definition (its span is
    // the file length), so any seed past ~255 earlier occurrences exhausted
    // the allowance: candidates:full, partial, no items.
    let (world, symbols) = module_world(400);
    let outcome = world.references(seed_at(&world, 600), 64, None).unwrap();
    assert_eq!(
        outcome.symbol_id,
        Some(symbol_id(RA, "src/lib.rs", &symbols[300]))
    );
    // The identifier itself, the one before it (to prove nothing narrower or
    // equal remains), then the single reference of that symbol.
    eprintln!("deep identifier seed examined: {}", outcome.examined);
    assert!(outcome.examined < 16, "examined {}", outcome.examined);
    assert_eq!(outcome.items.len(), 1);
    assert!(!outcome.candidates_full && !outcome.more);
    assert_eq!(outcome.target, Some(TargetResolution::Unknown));
}

#[test]
fn an_equal_span_tie_is_still_ambiguous_under_the_early_stop() {
    const ONE: &str = "rust-analyzer cargo toy 0.1.0 one().";
    const TWO: &str = "rust-analyzer cargo toy 0.1.0 two().";
    let world = World::with_sources(&[("src/lib.rs", &long_line_source(400))]);
    let mut occurrences: Vec<Occurrence> = (0..400).map(token_ref).collect();
    // Two different symbols on exactly [600,603), and a whole-file module
    // definition that keeps the scan from ending early any other way.
    occurrences.push(occ(&[0, 600, 603], ONE, REF));
    occurrences.push(occ(&[0, 600, 603], TWO, REF));
    occurrences.push(occ(
        &[0, 0, 1, 0],
        "rust-analyzer cargo toy 0.1.0 module#",
        DEF,
    ));
    world.import_ok(RA, "tie", &artifact(vec![doc("src/lib.rs", occurrences)]));
    let error = world
        .references(seed_at(&world, 601), 64, None)
        .unwrap_err();
    let FoundryError::Scip { code: c, message } = &error else {
        panic!("{error:?}")
    };
    assert_eq!(*c, code::AMBIGUOUS_SYMBOL);
    let (one, two) = (
        symbol_id(RA, "src/lib.rs", ONE),
        symbol_id(RA, "src/lib.rs", TWO),
    );
    assert!(
        message.contains(&one[..16]) && message.contains(&two[..16]),
        "{message}"
    );
    // A narrower one-byte occurrence inside the tie wins outright.
    let mut occurrences: Vec<Occurrence> = (0..400).map(token_ref).collect();
    occurrences.push(occ(&[0, 600, 603], ONE, REF));
    occurrences.push(occ(&[0, 600, 603], TWO, REF));
    occurrences.push(occ(
        &[0, 601, 602],
        "rust-analyzer cargo toy 0.1.0 inner().",
        REF,
    ));
    occurrences.push(occ(
        &[0, 0, 1, 0],
        "rust-analyzer cargo toy 0.1.0 module#",
        DEF,
    ));
    world.import_ok(
        RA,
        "tie-inner",
        &artifact(vec![doc("src/lib.rs", occurrences)]),
    );
    let inner = world.references(seed_at(&world, 601), 64, None).unwrap();
    assert_eq!(
        inner.symbol_id,
        Some(symbol_id(
            RA,
            "src/lib.rs",
            "rust-analyzer cargo toy 0.1.0 inner()."
        ))
    );
}

#[test]
fn a_narrower_occurrence_starting_before_a_wider_later_one_still_wins_deep_in_a_file() {
    // A = [300,310) starts BEFORE B = [302,500): at offset 305 both contain
    // it and A is narrower. Hundreds of short occurrences precede them.
    const A: &str = "rust-analyzer cargo toy 0.1.0 a().";
    const B: &str = "rust-analyzer cargo toy 0.1.0 b().";
    let world = World::with_sources(&[("src/lib.rs", &long_line_source(400))]);
    let mut occurrences: Vec<Occurrence> = (0..140).map(token_ref).collect();
    occurrences.push(occ(&[0, 300, 310], A, REF));
    occurrences.push(occ(&[0, 302, 500], B, REF));
    occurrences.push(occ(
        &[0, 0, 1, 0],
        "rust-analyzer cargo toy 0.1.0 module#",
        DEF,
    ));
    world.import_ok(
        RA,
        "non-laminar",
        &artifact(vec![doc("src/lib.rs", occurrences)]),
    );
    let outcome = world.references(seed_at(&world, 305), 64, None).unwrap();
    assert_eq!(outcome.symbol_id, Some(symbol_id(RA, "src/lib.rs", A)));
    assert!(outcome.examined < 16, "examined {}", outcome.examined);
}

// --- R4-1: the definition lookup never takes the last record -----------------

const TARGET: &str = "rust-analyzer cargo toy 0.1.0 Target#";

/// `src/lib.rs` is `k + 1` one-byte tokens under a whole-file definition of
/// `Target`. A seed on the space after the last token examines every token
/// (none contains it) and then the whole-file definition: exactly `k + 2`
/// seed records. `src/other.rs` and `src/third.rs` hold three references
/// between them and, for `defs` of 2 and 3, one more definition each.
fn target_world(k: usize, defs: usize) -> World {
    let world = World::with_sources(&[
        ("src/lib.rs", &long_line_source(k + 1)),
        ("src/other.rs", OTHER),
        ("src/third.rs", THIRD),
    ]);
    let mut lib: Vec<Occurrence> = (0..=k).map(token_ref).collect();
    lib.push(occ(&[0, 0, 1, 0], TARGET, DEF));
    let mut other = vec![occ(&[1, 4, 9], TARGET, REF), occ(&[1, 11, 16], TARGET, REF)];
    let mut third = vec![occ(&[1, 11, 16], TARGET, REF)];
    if defs >= 2 {
        other.push(occ(&[0, 7, 12], TARGET, DEF));
    }
    if defs >= 3 {
        third.push(occ(&[0, 7, 12], TARGET, DEF));
    }
    world.import_ok(
        RA,
        "target",
        &artifact(vec![
            doc("src/lib.rs", lib),
            doc("src/other.rs", other),
            doc("src/third.rs", third),
        ]),
    );
    world
}

/// Definition records of `Target` whose scope is not the selected snapshot's
/// (what an interrupted same-revision replacement leaves behind). They sort
/// before every real path, so the lookup meets them first.
fn add_stale_definitions(world: &mut World, range: std::ops::Range<usize>) {
    let target = symbol_id(RA, "src/lib.rs", TARGET);
    world.close();
    testkit::write_store(&world.store, |tx| {
        let mut table = tx
            .open_table(redb::TableDefinition::<&str, &str>::new(
                "compiler_by_symbol",
            ))
            .unwrap();
        for i in range {
            let key = format!("{target}\0d\0src/a{i}_old.rs\0{:020}\0{:020}", 0, 1);
            table.insert(key.as_str(), RA).unwrap();
        }
    });
    world.reopen();
}

/// Every reference `Target` has, in cursor order: line 1 of `OTHER` and
/// `THIRD` is `    crate::alpha();` after a 17-byte first line.
fn target_references() -> Vec<(String, u64, u64)> {
    [
        ("src/other.rs", 21, 26),
        ("src/other.rs", 28, 33),
        ("src/third.rs", 28, 33),
    ]
    .map(|(path, start, end)| (path.to_owned(), start, end))
    .to_vec()
}

/// `first`'s references followed through its cursor, page by page.
fn stitched(world: &World, first: &ReferencesOutcome) -> Vec<(String, u64, u64)> {
    let id = first.symbol_id.clone().expect("a certified seed");
    let mut got: Vec<(String, u64, u64)> = first
        .items
        .iter()
        .map(|i| (i.path.clone(), i.start, i.end))
        .collect();
    let mut next = first.resume.clone();
    for _ in 0..8 {
        let Some(cursor) = next.take() else { break };
        let page = world
            .references(
                ReferencesSeed::SymbolId(id[..16].to_owned()),
                64,
                Some(&cursor),
            )
            .unwrap();
        got.extend(page.items.iter().map(|i| (i.path.clone(), i.start, i.end)));
        next = page.resume.clone();
        assert_eq!(page.more, next.is_some());
    }
    assert!(next.is_none(), "the cursor chain ends");
    got
}

#[test]
fn a_stale_and_a_current_definition_after_a_254_record_seed_still_deliver_a_reference() {
    // The reviewer's counterexample: 254 seed records, one old-tuple
    // definition, one current unique definition. Round 3 spent both on the
    // lookup (256 examined, target Unique) and left the reference loop no
    // record: zero items, `more` false, no cursor - a false end.
    let mut world = target_world(252, 1);
    add_stale_definitions(&mut world, 0..1);
    let outcome = world
        .references(seed_at(&world, 2 * 252 + 1), 64, None)
        .unwrap();
    assert_eq!(outcome.symbol_id, Some(symbol_id(RA, "src/lib.rs", TARGET)));
    // The lookup could not conclude within its share: Unfinished, never
    // certified Unique, and said so.
    assert_eq!(outcome.target, Some(TargetResolution::Unfinished));
    assert!(outcome.definitions_truncated && outcome.candidates_full);
    assert_eq!(outcome.coverage, Coverage::Partial);
    assert_eq!(outcome.stale, 1);
    // The reserved record delivered a reference, and the cursor is usable.
    assert_eq!(outcome.examined, 256);
    assert_eq!(outcome.items.len(), 1);
    assert!(outcome.more && outcome.resume.is_some());
    let text = world.text(&outcome, 4096);
    let parsed = parse_v2(&text).unwrap();
    assert_eq!(parsed.next.as_deref(), outcome.resume.as_deref());
    assert!(parsed.header.contains(&"candidates:full".to_owned()));
    // Following the cursor loses and repeats nothing.
    assert_eq!(stitched(&world, &outcome), target_references());
}

#[test]
fn hundreds_of_stale_definitions_still_leave_a_reference_and_a_cursor() {
    let mut world = target_world(10, 1);
    add_stale_definitions(&mut world, 0..300);
    let id = symbol_id(RA, "src/lib.rs", TARGET);
    let outcome = world
        .references(ReferencesSeed::SymbolId(id[..16].to_owned()), 64, None)
        .unwrap();
    // 255 stale records end the lookup; the 256th record is the first
    // reference.
    assert_eq!(outcome.target, Some(TargetResolution::Unfinished));
    assert!(outcome.definitions_truncated && outcome.definitions.is_empty());
    assert_eq!((outcome.examined, outcome.stale), (256, 255));
    assert_eq!(outcome.items.len(), 1);
    assert!(outcome.more && outcome.resume.is_some() && outcome.candidates_full);
    assert_eq!(outcome.coverage, Coverage::Partial);
    assert_eq!(stitched(&world, &outcome), target_references());
}

#[test]
fn whenever_a_reference_remains_after_the_seed_a_reference_and_a_cursor_are_delivered() {
    use TargetResolution::{Ambiguous, Unfinished, Unique};
    const REFERENCES: usize = 3;
    for k in 248..=253 {
        // Records the containing scan needs: every token, then the
        // whole-file definition.
        let seed_cost = k + 2;
        for defs in 1..=3usize {
            let mut world = target_world(k, defs);
            for stale in 0..=2usize {
                if stale > 0 {
                    add_stale_definitions(&mut world, stale - 1..stale);
                }
                let context = format!("seed cost {seed_cost}, {defs} definitions, {stale} stale");
                let outcome = world
                    .references(seed_at(&world, 2 * k as u64 + 1), 64, None)
                    .unwrap();
                if seed_cost > 254 {
                    // The scan itself is out of budget (it leaves two
                    // records): truncated, nothing certified.
                    assert!(outcome.symbol_id.is_none() && outcome.target.is_none());
                    assert!(outcome.definitions_truncated && outcome.candidates_full);
                    assert_eq!(outcome.examined, 254, "{context}");
                    assert!(outcome.items.is_empty() && !outcome.more, "{context}");
                    assert_eq!(outcome.coverage, Coverage::Partial);
                    continue;
                }
                // The lookup may use every record but the last.
                let free = 255 - seed_cost;
                let lookup = defs + stale;
                let used = lookup.min(free);
                let finished = lookup <= free;
                let found = used.saturating_sub(stale);
                let target = if found >= 2 {
                    Ambiguous
                } else if finished {
                    Unique
                } else {
                    Unfinished
                };
                let items = REFERENCES.min(256 - seed_cost - used);
                assert_eq!(outcome.target, Some(target), "{context}");
                assert_eq!(outcome.definitions_truncated, !finished, "{context}");
                assert_eq!(outcome.items.len(), items, "{context}");
                assert!(!outcome.items.is_empty(), "{context}: a reference remains");
                assert_eq!(outcome.examined, seed_cost + used + items, "{context}");
                assert_eq!(outcome.more, items < REFERENCES, "{context}");
                assert_eq!(outcome.resume.is_some(), outcome.more, "{context}");
                assert_eq!(
                    outcome.candidates_full,
                    outcome.examined >= 256,
                    "{context}"
                );
                if target != Unique {
                    assert_eq!(outcome.coverage, Coverage::Partial, "{context}");
                }
                // The cursor continues losslessly: every reference once, in
                // order.
                assert_eq!(stitched(&world, &outcome), target_references(), "{context}");
            }
        }
    }
}

// --- R4-2: the slim decoder refuses what the generated decoder refused -------

const WIRE_SYMBOL: &str = "rust-analyzer cargo toy 0.1.0 f().";

fn wire_world() -> World {
    World::with_sources(&[("src/lib.rs", "pub fn f() {}\n")])
}

/// One definition occurrence of `f` (bytes 7..8), then `tail` INSIDE it.
fn wire_occurrence(tail: &[u8]) -> Vec<u8> {
    let mut out = vec![0x0a, 0x03, 0x00, 0x07, 0x08, 0x12];
    out.extend(varint(WIRE_SYMBOL.len()));
    out.extend(WIRE_SYMBOL.as_bytes());
    out.extend([0x18, 0x01]);
    out.extend(tail);
    out
}

/// A document message: `inside` goes inside its one occurrence, `after` at
/// the end of the document.
fn wire_document(inside: &[u8], after: &[u8]) -> Vec<u8> {
    let occurrence = wire_occurrence(inside);
    let mut out = raw_path("src/lib.rs");
    out.push(0x12);
    out.extend(varint(occurrence.len()));
    out.extend(occurrence);
    out.extend([0x30, 0x01]);
    out.extend(after);
    out
}

/// A well-formed unknown length-delimited field (field 15) of 64 bytes:
/// bytes an unbounded skip or an unbounded nested read could consume.
fn wire_padding() -> Vec<u8> {
    let mut out = vec![0x7a, 64];
    out.extend([0u8; 64]);
    out
}

#[derive(Clone, Copy, Debug)]
enum Level {
    /// Beside `Index.documents`.
    Index,
    /// Inside a `Document`.
    Document,
    /// Inside an `Occurrence`.
    Occurrence,
    /// Among the leading fields of a document over the size cap.
    OversizedDocument,
}

const LEVELS: [Level; 4] = [
    Level::Index,
    Level::Document,
    Level::Occurrence,
    Level::OversizedDocument,
];

/// An artifact whose one document carries `junk` at `level`, with padding
/// after it where a parent bound is what must stop an overrun.
fn wire_artifact(level: Level, junk: &[u8]) -> (Vec<u8>, ImportLimits) {
    let limits = ImportLimits::default();
    match level {
        Level::Index => {
            let mut bytes = raw_index(&[wire_document(&[], &[])]);
            bytes.extend(junk);
            (bytes, limits)
        }
        Level::Document => {
            let mut bytes = raw_index(&[wire_document(&[], junk)]);
            bytes.extend(wire_padding());
            (bytes, limits)
        }
        Level::Occurrence => (raw_index(&[wire_document(junk, &wire_padding())]), limits),
        Level::OversizedDocument => {
            let mut message = junk.to_vec();
            message.extend(raw_path("src/lib.rs"));
            let mut bytes = raw_index(&[message]);
            bytes.extend(wire_padding());
            (
                bytes,
                ImportLimits {
                    document_bytes: 4,
                    ..ImportLimits::default()
                },
            )
        }
    }
}

fn import_wire(world: &World, level: Level, junk: &[u8]) -> FResult<ImportReport> {
    let (bytes, limits) = wire_artifact(level, junk);
    let manifest = world.manifest(RA, "2026-08-31", "wire", &bytes);
    let (index, snapshot) = world.write_pair(&manifest, &bytes);
    world.import_files(&index, &snapshot, &limits)
}

/// The artifact is refused as undecodable and nothing was published.
fn assert_wire_refused(level: Level, label: &str, junk: &[u8]) {
    let mut world = wire_world();
    let before = without_producers(world.raw());
    let error = import_wire(&world, level, junk).unwrap_err();
    assert_eq!(
        error.code(),
        code::PRODUCER_INCOMPLETE,
        "{level:?} / {label}: {error:?}"
    );
    assert_eq!(
        without_producers(world.raw()),
        before,
        "{level:?} / {label}"
    );
    assert!(world.selected(RA).is_none(), "{level:?} / {label}");
}

fn assert_wire_accepted(level: Level, label: &str, junk: &[u8]) {
    let world = wire_world();
    let report = import_wire(&world, level, junk)
        .unwrap_or_else(|error| panic!("{level:?} / {label}: {error:?}"));
    match level {
        Level::OversizedDocument => {
            assert_eq!(report.failed, 1, "{level:?} / {label}: {report:?}");
            assert_eq!(report.failure_samples[0].code, code::DOCUMENT_TOO_LARGE);
        }
        _ => assert!(
            report.complete && report.failed == 0 && report.definitions == 1,
            "{level:?} / {label}: {report:?}"
        ),
    }
}

fn nested_groups(depth: usize) -> Vec<u8> {
    let mut out = vec![0x7b; depth];
    out.extend(vec![0x7c; depth]);
    out
}

#[test]
fn a_field_number_of_zero_is_refused_at_every_nesting_level() {
    let cases: [(&str, &[u8]); 5] = [
        ("varint", &[0x00, 0x01]),
        ("length-delimited", &[0x02, 0x00]),
        ("fixed32", &[0x05, 0, 0, 0, 0]),
        ("fixed64", &[0x01, 0, 0, 0, 0, 0, 0, 0, 0]),
        ("a group numbered 0", &[0x03, 0x04]),
    ];
    for level in LEVELS {
        for (label, junk) in cases {
            assert_wire_refused(level, label, junk);
        }
    }
}

#[test]
fn an_unterminated_or_mismatched_group_is_refused_at_every_nesting_level() {
    let cases: [(&str, Vec<u8>); 7] = [
        ("unterminated", vec![0x7b, 0x08, 0x01]),
        ("closed by another field's end", vec![0x7b, 0x74]),
        ("an end group closing nothing", vec![0x7c]),
        (
            "nested groups closed out of order",
            vec![0x7b, 0x73, 0x7c, 0x74],
        ),
        (
            "the inner group closed, the outer left open",
            vec![0x7b, 0x73, 0x74],
        ),
        ("nested deeper than the limit", nested_groups(101)),
        ("an invalid tag inside a group", vec![0x7b, 0x7e, 0x7c]),
    ];
    for level in LEVELS {
        for (label, junk) in &cases {
            assert_wire_refused(level, label, junk);
        }
    }
}

#[test]
fn wire_types_six_and_seven_are_refused_at_every_nesting_level() {
    let cases: [(&str, &[u8]); 2] = [("six", &[0x7e, 0x00]), ("seven", &[0x7f, 0x00])];
    for level in LEVELS {
        for (label, junk) in cases {
            assert_wire_refused(level, label, junk);
        }
    }
}

#[test]
fn a_length_that_overruns_its_parent_is_refused_at_every_nesting_level() {
    // An unknown length-delimited field longer than what its parent has
    // left. Where a sibling follows (the padding), an unbounded skip would
    // consume it and the document would decode as if nothing were wrong.
    for level in LEVELS {
        assert_wire_refused(level, "unknown field", &[0x7a, 0x20, 0x01, 0x02]);
    }
    // A nested MESSAGE that declares more than its parent holds.
    assert_wire_refused(
        Level::Index,
        "document longer than the file",
        &[0x12, 0x7f, 0x0a, 0x03],
    );
    assert_wire_refused(
        Level::Document,
        "occurrence longer than the document",
        &[0x12, 0x7f, 0x0a, 0x03, 0x00],
    );
    assert_wire_refused(
        Level::Occurrence,
        "typed range longer than the occurrence",
        &[0x42, 0x20, 0x08, 0x01],
    );
    assert_wire_refused(
        Level::Occurrence,
        "packed range longer than the occurrence",
        &[0x0a, 0x40, 0x00],
    );
    assert_wire_refused(
        Level::Occurrence,
        "symbol longer than the occurrence",
        &[0x12, 0x40, b'x'],
    );
}

#[test]
fn well_formed_unknown_fields_are_still_skipped_at_every_nesting_level() {
    let cases: [(&str, Vec<u8>); 8] = [
        ("varint", vec![0x78, 0x05]),
        ("fixed32", vec![0x7d, 1, 2, 3, 4]),
        ("fixed64", vec![0x79, 1, 2, 3, 4, 5, 6, 7, 8]),
        ("length-delimited", vec![0x7a, 0x02, 0xaa, 0xbb]),
        ("empty length-delimited", vec![0x7a, 0x00]),
        ("a high field number", vec![0xc0, 0x3e, 0x01]),
        (
            "a matched group with a nested group",
            vec![0x7b, 0x73, 0x08, 0x01, 0x74, 0x7c],
        ),
        ("groups at the depth limit", nested_groups(100)),
    ];
    for level in LEVELS {
        for (label, junk) in &cases {
            assert_wire_accepted(level, label, junk);
        }
    }
}

// --- R5: seed key ranges and oversized document identity ---------------------

#[test]
fn a_seed_key_that_runs_past_the_end_of_its_source_is_graph_invalid() {
    let mut world = toy();
    world.import_ok(RA, "seed-range", &toy_artifact());
    let alpha = alpha_id();
    // Only the occurrences row of alpha's definition is rewritten: its span
    // now runs past the 62-byte source, while the reverse `by_symbol` key
    // still names [7,12). A position seed inside that inflated span (byte
    // 25, inside beta's name) must refuse the graph, not resolve alpha.
    world.close();
    testkit::write_store(&world.store, |tx| {
        let mut table = tx
            .open_table(redb::TableDefinition::<&str, &str>::new(
                "compiler_occurrences",
            ))
            .unwrap();
        let good = format!("{RA}\0src/lib.rs\0{:020}\0{:020}\0d\0{alpha}", 7, 12);
        let bad = format!("{RA}\0src/lib.rs\0{:020}\0{:020}\0d\0{alpha}", 7, 120);
        let value = table
            .get(good.as_str())
            .unwrap()
            .unwrap()
            .value()
            .to_owned();
        table.remove(good.as_str()).unwrap();
        table.insert(bad.as_str(), value.as_str()).unwrap();
    });
    world.reopen();
    let error = world.references(seed_at(&world, 25), 64, None).unwrap_err();
    assert_eq!(error.code(), "graph_invalid");
    // The reverse path never consults the occurrence scan: it still answers.
    let by_symbol = world.by_symbol(&alpha);
    assert_eq!(by_symbol.target, Some(TargetResolution::Unique));
    assert_eq!(by_symbol.items.len(), 3);
}

#[test]
fn a_seed_key_that_splits_a_codepoint_is_graph_invalid() {
    const BETA: &str = "rust-analyzer cargo toy 0.1.0 beta().";
    // `α` spans bytes 7..9; the rewritten key starts at 8, inside it.
    let mut world = World::with_sources(&[("src/lib.rs", "pub fn α() { α(); }\n")]);
    world.import_ok(
        RA,
        "split",
        &artifact(vec![doc("src/lib.rs", vec![occ(&[0, 7, 9], BETA, DEF)])]),
    );
    let beta = symbol_id(RA, "src/lib.rs", BETA);
    world.close();
    testkit::write_store(&world.store, |tx| {
        let mut table = tx
            .open_table(redb::TableDefinition::<&str, &str>::new(
                "compiler_occurrences",
            ))
            .unwrap();
        let good = format!("{RA}\0src/lib.rs\0{:020}\0{:020}\0d\0{beta}", 7, 9);
        let bad = format!("{RA}\0src/lib.rs\0{:020}\0{:020}\0d\0{beta}", 8, 12);
        let value = table
            .get(good.as_str())
            .unwrap()
            .unwrap()
            .value()
            .to_owned();
        table.remove(good.as_str()).unwrap();
        table.insert(bad.as_str(), value.as_str()).unwrap();
    });
    world.reopen();
    let error = world.references(seed_at(&world, 8), 64, None).unwrap_err();
    assert_eq!(error.code(), "graph_invalid");
}

/// An oversized document message naming `src/x.rs` and then `src/y.rs`, with
/// more than 8 MiB of skippable unknown metadata after them.
fn oversized_two_paths() -> Vec<u8> {
    let mut message = raw_path("src/x.rs");
    message.extend(raw_path("src/y.rs"));
    let payload = (8 << 20) + 64;
    message.push(0x7a);
    message.extend(varint(payload));
    message.extend(std::iter::repeat_n(0u8, payload));
    message
}

/// A bounded document for `src/y.rs`: one definition at bytes 7..8.
fn bounded_y() -> Vec<u8> {
    let occurrence = wire_occurrence(&[]);
    let mut out = raw_path("src/y.rs");
    out.push(0x12);
    out.extend(varint(occurrence.len()));
    out.extend(occurrence);
    out.extend([0x30, 0x01]);
    out
}

fn two_path_world() -> World {
    World::with_sources(&[
        ("src/x.rs", "pub fn x() {}\n"),
        ("src/y.rs", "pub fn y() {}\n"),
    ])
}

#[test]
fn an_oversized_documents_last_path_decides_its_identity_and_duplicates_are_named() {
    let mut world = two_path_world();
    // A prior, clean selection whose state must survive the refusal.
    world.import_ok(
        RA,
        "prior",
        &artifact(vec![doc(
            "src/x.rs",
            vec![occ(&[0, 7, 8], "rust-analyzer cargo toy 0.1.0 x().", DEF)],
        )]),
    );
    let selected = world.selected(RA).unwrap();
    let before = without_producers(world.raw());
    // The oversized document is attributed to its LAST path, `src/y.rs`,
    // so the bounded `src/y.rs` document after it is a duplicate.
    let bytes = raw_index(&[oversized_two_paths(), bounded_y()]);
    let error = world.import(RA, "dup", &bytes).unwrap_err();
    assert_eq!(error.code(), code::DUPLICATE_DOCUMENT);
    assert_eq!(without_producers(world.raw()), before, "{error:?}");
    assert_eq!(world.selected(RA).unwrap().tuple, selected.tuple);
}

#[test]
fn an_oversized_documents_failure_is_attributed_to_its_last_path() {
    let world = two_path_world();
    let bytes = raw_index(&[oversized_two_paths()]);
    let report = world.import_ok(RA, "last-path", &bytes);
    assert_eq!(report.failed, 1);
    assert_eq!(report.failure_samples[0].path, "src/y.rs");
    assert_eq!(report.failure_samples[0].code, code::DOCUMENT_TOO_LARGE);
    assert_eq!(
        report.accepted_empty, 1,
        "src/x.rs is absent from the artifact"
    );
    assert!(!report.complete);
}

#[test]
fn a_truncated_oversized_document_is_producer_incomplete_not_a_failed_document() {
    // The document declares more than the 8 MiB cap, but the file physically
    // ends right after its relative_path. The preflight must refuse the
    // ARTIFACT (`producer_incomplete`, before any selection) instead of
    // selecting a new tuple and reporting a capped document.
    let mut world = two_path_world();
    world.import_ok(
        RA,
        "prior",
        &artifact(vec![doc(
            "src/x.rs",
            vec![occ(&[0, 7, 8], "rust-analyzer cargo toy 0.1.0 x().", DEF)],
        )]),
    );
    let selected = world.selected(RA).unwrap();
    let before = without_producers(world.raw());
    let declared = (8 << 20) + 64;
    let mut bytes = vec![0x12];
    bytes.extend(varint(declared));
    bytes.extend(raw_path("src/x.rs"));
    let error = world.import(RA, "truncated", &bytes).unwrap_err();
    assert_eq!(error.code(), code::PRODUCER_INCOMPLETE);
    assert_eq!(without_producers(world.raw()), before, "{error:?}");
    assert_eq!(world.selected(RA).unwrap().tuple, selected.tuple);
}

#[test]
fn a_truncated_bounded_document_is_producer_incomplete() {
    // A document inside the cap whose declared length runs past the end of
    // the file: the buffered read itself hits end of file and the artifact
    // is refused before selection.
    let mut world = two_path_world();
    world.import_ok(
        RA,
        "prior",
        &artifact(vec![doc(
            "src/x.rs",
            vec![occ(&[0, 7, 8], "rust-analyzer cargo toy 0.1.0 x().", DEF)],
        )]),
    );
    let selected = world.selected(RA).unwrap();
    let before = without_producers(world.raw());
    let mut bytes = vec![0x12];
    bytes.extend(varint(30));
    bytes.extend(raw_path("src/x.rs"));
    let error = world.import(RA, "truncated-bounded", &bytes).unwrap_err();
    assert_eq!(error.code(), code::PRODUCER_INCOMPLETE);
    assert_eq!(without_producers(world.raw()), before, "{error:?}");
    assert_eq!(world.selected(RA).unwrap().tuple, selected.tuple);
}

#[test]
fn a_truncated_unnamed_oversized_document_is_producer_incomplete() {
    // The same truncation with no relative_path at all: the file ends the
    // moment the declared length is read. It must not become "document #1
    // failed its cap" with a fresh selection.
    let mut world = two_path_world();
    world.import_ok(
        RA,
        "prior",
        &artifact(vec![doc(
            "src/x.rs",
            vec![occ(&[0, 7, 8], "rust-analyzer cargo toy 0.1.0 x().", DEF)],
        )]),
    );
    let selected = world.selected(RA).unwrap();
    let before = without_producers(world.raw());
    let mut bytes = vec![0x12];
    bytes.extend(varint((8 << 20) + 64));
    let error = world.import(RA, "truncated-unnamed", &bytes).unwrap_err();
    assert_eq!(error.code(), code::PRODUCER_INCOMPLETE);
    assert_eq!(without_producers(world.raw()), before, "{error:?}");
    assert_eq!(world.selected(RA).unwrap().tuple, selected.tuple);
}

// --- T003: context(strategy=graph) compiler units ----------------------------

fn graph_context(world: &World, query: &str) -> context_foundry::store::CandidateBatch {
    world
        .engine()
        .context_candidates(
            query,
            context_foundry::Strategy::Graph,
            &Control::unbounded(),
        )
        .unwrap()
}

#[test]
fn graph_context_adds_the_units_enclosing_a_symbols_references() {
    // A query whose lexical hits live in a.rs, b.rs and lib.rs only: the
    // compiler graph is what can add the units that ENCLOSE a::parse_record's
    // references in use_one.rs, use_two.rs and pointer.rs.
    let world = World::fixture();
    let bytes = fixture_artifact();
    world
        .import_manifest(&fixture_manifest(&world, &bytes), &bytes)
        .unwrap();
    let batch = graph_context(&world, "raw");
    assert_eq!(batch.counters.graph, Some("ok"), "{:?}", batch.counters);
    let paths: Vec<&str> = batch
        .items
        .iter()
        .filter_map(|item| item.handle.as_ref().map(|handle| handle.path.as_str()))
        .collect();
    for expected in ["src/use_one.rs", "src/use_two.rs", "src/pointer.rs"] {
        assert!(paths.contains(&expected), "{paths:?}");
    }
    // The compiler units are source-first units: verbatim text of the unit.
    let pointer = batch
        .items
        .iter()
        .find(|item| {
            item.handle
                .as_ref()
                .is_some_and(|h| h.path == "src/pointer.rs")
        })
        .unwrap();
    assert!(
        pointer
            .forms
            .iter()
            .any(|form| matches!(form, context_foundry::store::RenderedForm::Verbatim(_)))
    );
    // The packed text carries the graph state and the unit's handle.
    let packed = response::pack_context(
        &batch,
        response::Budget {
            tokens: 4096,
            limited_by: response::BudgetLimiter::Request,
        },
        &response::stdout_bytes,
    )
    .unwrap();
    assert!(
        packed.text.contains("graph:ok"),
        "{}",
        packed.text.lines().next().unwrap()
    );
    assert!(packed.text.contains("src/pointer.rs"));
}

#[test]
fn graph_context_state_spans_unavailable_ok_and_stale() {
    // No import: the compiler graph is unavailable, and the source context
    // still delivers.
    let world = World::fixture();
    let batch = graph_context(&world, "raw");
    assert_eq!(batch.counters.graph, Some("graph_unavailable"));
    assert!(
        batch.items.iter().any(|item| item.handle.is_some()),
        "source context survives the unavailable graph"
    );
    // With a fresh import the same query is ok.
    let bytes = fixture_artifact();
    world
        .import_manifest(&fixture_manifest(&world, &bytes), &bytes)
        .unwrap();
    assert_eq!(graph_context(&world, "raw").counters.graph, Some("ok"));
    // An edit bumps the revision: every selected snapshot predates it, the
    // same state `references` answers `coverage:stale` for.
    world.set_source("Cargo.toml", "[package]\nname = \"fixture\"\n");
    let batch = graph_context(&world, "raw");
    assert_eq!(batch.counters.graph, Some("graph_stale"));
    assert!(
        batch.items.iter().any(|item| item.handle.is_some()),
        "source context survives the stale graph"
    );
}

#[test]
fn graph_context_units_are_deduplicated_against_search_units_and_capped() {
    // The same unit is never delivered twice: the search hit in a.rs stays
    // the search unit, and each symbol contributes its units once.
    let world = World::fixture();
    let bytes = fixture_artifact();
    world
        .import_manifest(&fixture_manifest(&world, &bytes), &bytes)
        .unwrap();
    let batch = graph_context(&world, "raw");
    let mut seen = std::collections::BTreeSet::new();
    for item in &batch.items {
        if let Some(handle) = &item.handle {
            assert!(
                seen.insert((handle.path.clone(), handle.start, handle.end)),
                "a unit is delivered twice: {} {}..{}",
                handle.path,
                handle.start,
                handle.end
            );
        }
    }
    // A query with no lexical hits at all has no spans to seed from: the
    // compiler graph cannot invent candidates, and says unavailable.
    let batch = graph_context(&world, "zzz_no_such_token");
    assert_eq!(batch.counters.graph, Some("graph_unavailable"));
    assert!(batch.items.is_empty());
}

// --- T003 review round 1: provenance, uniqueness, bounds and degradation -----

use context_foundry::store::{CandidateBatch, TIER_COMPILER};

fn compiler_paths(batch: &CandidateBatch) -> Vec<String> {
    batch
        .items
        .iter()
        .filter(|item| item.tier == TIER_COMPILER)
        .filter_map(|item| item.handle.as_ref().map(|handle| handle.path.clone()))
        .collect()
}

const REFERRING_FILES: [&str; 3] = ["src/pointer.rs", "src/use_one.rs", "src/use_two.rs"];

fn imported_fixture() -> (World, Vec<u8>) {
    let world = World::fixture();
    let bytes = fixture_artifact();
    world
        .import_manifest(&fixture_manifest(&world, &bytes), &bytes)
        .unwrap();
    (world, bytes)
}

/// Drain the derived lexical index: sources inserted directly are not
/// searchable until it runs.
fn make_searchable(world: &mut World) {
    world
        .engine
        .as_mut()
        .expect("the engine is open")
        .refresh(&Control::unbounded())
        .unwrap();
}

fn assert_referring_units_collected(world: &World) {
    let before = compiler_paths(&graph_context(world, "raw"));
    for expected in REFERRING_FILES {
        assert!(before.iter().any(|path| path == expected), "{before:?}");
    }
}

#[test]
fn a_same_revision_replacement_at_the_final_barrier_drops_the_old_snapshots_units() {
    let (world, bytes) = imported_fixture();
    assert_referring_units_collected(&world);
    // Artifact B at the SAME source revision: the referring files carry no
    // occurrences any more, so B removed exactly those references. Their
    // scopes still exist (accepted-empty, under B's snapshot) with the same
    // source hashes - which is all round 1 checked.
    let replacement = {
        let mut index = Index::parse_from_bytes(&bytes).unwrap();
        for document in &mut index.documents {
            if REFERRING_FILES.contains(&document.relative_path.as_str()) {
                document.occurrences.clear();
            }
        }
        index.write_to_bytes().unwrap()
    };
    let manifest = fixture_manifest(&world, &replacement);
    let (index_path, snapshot_path) = world.write_pair(&manifest, &replacement);
    fault::arm(
        names::CONTEXT_BEFORE_FINAL_VALIDATION,
        0,
        Action::Call(Box::new(move |ctx| {
            let report = ctx
                .engine
                .unwrap()
                .import_scip(&index_path, &snapshot_path, &Control::unbounded())
                .unwrap();
            assert!(report.complete && report.selected, "{report:?}");
        })),
    );
    let batch = graph_context(&world, "raw");
    fault::disarm_all();
    assert!(
        compiler_paths(&batch).is_empty(),
        "a unit read from the old snapshot was relabeled: {:?}",
        compiler_paths(&batch)
    );
    assert!(batch.counters.stale >= 3, "{:?}", batch.counters);
    assert_eq!(batch.counters.graph, Some("graph_stale"));
    assert!(
        batch.items.iter().any(|item| item.handle.is_some()),
        "source context survives"
    );
}

#[test]
fn a_third_file_edit_and_reimport_at_the_final_barrier_drops_the_old_snapshots_units() {
    let (world, bytes) = imported_fixture();
    assert_referring_units_collected(&world);
    // At the barrier an UNRELATED file changes and the same artifact is
    // imported for the new revision: every scope is republished under a new
    // snapshot with unchanged hashes, so a check against "some current scope
    // for the path" would relabel the old evidence.
    const EDITED: &str = "[package]\nname = \"edited\"\n";
    let revision = world.engine().source_revision().unwrap() + 1;
    let mut inputs = world.inputs();
    for (path, hash) in &mut inputs {
        if path == "Cargo.toml" {
            *hash = digest(EDITED.as_bytes());
        }
    }
    let manifest = fixture_manifest_for(
        &world.engine().workspace_id().unwrap(),
        revision,
        inputs,
        &bytes,
    );
    let (index_path, snapshot_path) = world.write_pair(&manifest, &bytes);
    fault::arm(
        names::CONTEXT_BEFORE_FINAL_VALIDATION,
        0,
        Action::Call(Box::new(move |ctx| {
            let engine = ctx.engine.unwrap();
            assert!(engine.replace_source("Cargo.toml", EDITED).unwrap());
            let report = engine
                .import_scip(&index_path, &snapshot_path, &Control::unbounded())
                .unwrap();
            assert!(report.complete && report.selected, "{report:?}");
        })),
    );
    let batch = graph_context(&world, "raw");
    fault::disarm_all();
    assert!(
        compiler_paths(&batch).is_empty(),
        "{:?}",
        compiler_paths(&batch)
    );
    assert!(batch.counters.stale >= 3, "{:?}", batch.counters);
    assert_eq!(batch.counters.graph, Some("graph_stale"));
}

#[test]
fn a_corrupt_scope_row_at_the_final_barrier_degrades_to_graph_invalid_and_keeps_source_context() {
    let (world, _) = imported_fixture();
    assert_referring_units_collected(&world);
    fault::arm(
        names::CONTEXT_BEFORE_FINAL_VALIDATION,
        0,
        Action::Call(Box::new(|ctx| {
            ctx.engine
                .unwrap()
                .overwrite_compiler_scope_for_tests(RA, "src/use_one.rs", "{not json")
                .unwrap();
        })),
    );
    // The context call itself succeeds: the corruption is component-local.
    let batch = graph_context(&world, "raw");
    fault::disarm_all();
    assert_eq!(batch.counters.graph, Some("graph_invalid"));
    assert!(compiler_paths(&batch).is_empty());
    assert!(
        batch.items.iter().any(|item| item
            .handle
            .as_ref()
            .is_some_and(|handle| handle.path == "src/a.rs" || handle.path == "src/lib.rs")),
        "valid source context survives the invalid graph"
    );
}

const TFN: &str = "rust-analyzer cargo toy 0.1.0 target_fn().";

/// `host_seed` (the lexical hit) calls `target_fn`, defined in `def_one.rs`
/// (and in `def_two.rs` when `second_definition`); `callers.rs` calls it from
/// a unit the query never matches (when `caller`).
fn target_fn_world(second_definition: bool, caller: bool) -> World {
    const LIB_SRC: &str = "pub fn host_seed() { crate::target_fn(); }\n";
    const DEF_SRC: &str = "pub fn target_fn() {}\n";
    const CALLER_SRC: &str = "pub fn caller_a() { crate::target_fn(); }\n";
    let mut world = World::with_sources(&[
        ("src/lib.rs", LIB_SRC),
        ("src/def_one.rs", DEF_SRC),
        ("src/def_two.rs", DEF_SRC),
        ("src/callers.rs", CALLER_SRC),
    ]);
    let col = |text: &str| text.find("target_fn").unwrap() as i32;
    let mut documents = vec![
        doc(
            "src/lib.rs",
            vec![occ(&[0, col(LIB_SRC), col(LIB_SRC) + 9], TFN, REF)],
        ),
        doc("src/def_one.rs", vec![occ(&[0, 7, 16], TFN, DEF)]),
    ];
    if second_definition {
        documents.push(doc("src/def_two.rs", vec![occ(&[0, 7, 16], TFN, DEF)]));
    }
    if caller {
        documents.push(doc(
            "src/callers.rs",
            vec![occ(&[0, col(CALLER_SRC), col(CALLER_SRC) + 9], TFN, REF)],
        ));
    }
    world.import_ok(RA, "target-fn", &artifact(documents));
    make_searchable(&mut world);
    world
}

#[test]
fn a_uniquely_defined_symbol_expands_to_the_units_that_reference_it() {
    let world = target_fn_world(false, true);
    let batch = graph_context(&world, "host_seed");
    assert_eq!(batch.counters.graph, Some("ok"), "{:?}", batch.counters);
    assert_eq!(compiler_paths(&batch), ["src/callers.rs"]);
}

#[test]
fn an_ambiguous_symbol_does_not_expand_in_context() {
    // Two eligible definitions: `references` calls the target Ambiguous
    // (partial), so the context must not present its reference units as
    // resolved evidence.
    let world = target_fn_world(true, true);
    let batch = graph_context(&world, "host_seed");
    assert!(
        compiler_paths(&batch).is_empty(),
        "{:?}",
        compiler_paths(&batch)
    );
    assert_eq!(batch.counters.graph, Some("graph_unavailable"));
}

#[test]
fn a_uniqueness_check_the_window_cannot_conclude_does_not_expand_and_fills_the_window() {
    let mut world = target_fn_world(false, true);
    // 300 old-snapshot definition records sort AFTER the real one: the first
    // eligible definition is found at once, but only walking the rest could
    // conclude that it is the only one - and the 256-record window runs out.
    let id = symbol_id(RA, "src/def_one.rs", TFN);
    world.close();
    testkit::write_store(&world.store, |tx| {
        let mut table = tx
            .open_table(redb::TableDefinition::<&str, &str>::new(
                "compiler_by_symbol",
            ))
            .unwrap();
        for i in 0..300 {
            let key = format!("{id}\0d\0src/zz{i:04}_old.rs\0{:020}\0{:020}", 0, 1);
            table.insert(key.as_str(), RA).unwrap();
        }
    });
    world.reopen();
    let batch = graph_context(&world, "host_seed");
    assert!(
        compiler_paths(&batch).is_empty(),
        "{:?}",
        compiler_paths(&batch)
    );
    assert!(batch.counters.candidates_full, "{:?}", batch.counters);
    assert_ne!(batch.counters.graph, Some("ok"));
}

#[test]
fn a_graph_whose_reference_units_are_all_already_selected_is_ok_not_unavailable() {
    // The only reference to `target_fn` sits in `host_seed` itself, which the
    // lexical ranking already selected: no new unit, but the eligible graph
    // WAS used - reporting it unavailable would be a false empty graph.
    let world = target_fn_world(false, false);
    let batch = graph_context(&world, "host_seed");
    assert!(compiler_paths(&batch).is_empty());
    assert_eq!(batch.counters.graph, Some("ok"), "{:?}", batch.counters);
}

#[test]
fn a_large_seed_span_keeps_its_collected_symbols_and_fills_the_window() {
    const SYM: &str = "rust-analyzer cargo toy 0.1.0 tgt().";
    // One function containing 300 occurrences of the same symbol: the scan
    // of its span stops at the seed-scan share of the window. What it
    // collected is kept (round 1 discarded it) and still resolves.
    let big = format!("pub fn big_seed() {{\n    {}\n}}\n", "t ".repeat(300));
    let caller = "pub fn other_caller() { tgt(); }\n";
    let mut world = World::with_sources(&[
        ("src/lib.rs", big.as_str()),
        ("src/def.rs", "pub fn tgt() {}\n"),
        ("src/caller.rs", caller),
    ]);
    let col = caller.find("tgt").unwrap() as i32;
    let references: Vec<Occurrence> = (0..300)
        .map(|i| occ(&[1, 4 + 2 * i, 5 + 2 * i], SYM, REF))
        .collect();
    world.import_ok(
        RA,
        "large-seed",
        &artifact(vec![
            doc("src/lib.rs", references),
            doc("src/def.rs", vec![occ(&[0, 7, 10], SYM, DEF)]),
            doc("src/caller.rs", vec![occ(&[0, col, col + 3], SYM, REF)]),
        ]),
    );
    make_searchable(&mut world);
    let batch = graph_context(&world, "big_seed");
    assert_eq!(batch.counters.graph, Some("ok"), "{:?}", batch.counters);
    assert!(batch.counters.candidates_full, "the window filled");
    assert_eq!(compiler_paths(&batch), ["src/caller.rs"]);
}

/// `rewrite` replaces one `compiler_occurrences` key of a CLOSED store,
/// keeping its value; the reverse `compiler_by_symbol` keys stay valid.
fn rekey_occurrence(
    world: &mut World,
    path: &str,
    tag: &str,
    symbol: &str,
    from: (u64, u64),
    to: (u64, u64),
) {
    world.close();
    testkit::write_store(&world.store, |tx| {
        let mut table = tx
            .open_table(redb::TableDefinition::<&str, &str>::new(
                "compiler_occurrences",
            ))
            .unwrap();
        let key = |(start, end): (u64, u64)| {
            format!("{RA}\0{path}\0{start:020}\0{end:020}\0{tag}\0{symbol}")
        };
        let value = table
            .get(key(from).as_str())
            .unwrap()
            .unwrap()
            .value()
            .to_owned();
        table.remove(key(from).as_str()).unwrap();
        table.insert(key(to).as_str(), value.as_str()).unwrap();
    });
    world.reopen();
}

/// `alpha` is defined in `lib.rs` and referenced from a unit in `other.rs`;
/// the query matches only the later, unrelated `beta_unit`.
fn alpha_beta_world(lib: &str) -> (World, String) {
    const ALPHA_SYM: &str = "rust-analyzer cargo toy 0.1.0 alpha().";
    let other = "pub fn caller() { crate::alpha(); }\n";
    let mut world = World::with_sources(&[("src/lib.rs", lib), ("src/other.rs", other)]);
    let col = other.find("alpha").unwrap() as i32;
    let end = lib.find("α").map_or(12, |at| at as i32 + 2);
    world.import_ok(
        RA,
        "alpha-beta",
        &artifact(vec![
            doc("src/lib.rs", vec![occ(&[0, 7, end], ALPHA_SYM, DEF)]),
            doc(
                "src/other.rs",
                vec![occ(&[0, col, col + 5], ALPHA_SYM, REF)],
            ),
        ]),
    );
    let id = symbol_id(RA, "src/lib.rs", ALPHA_SYM);
    make_searchable(&mut world);
    (world, id)
}

#[test]
fn a_seed_occurrence_that_runs_past_its_source_is_graph_invalid_in_context() {
    let (mut world, id) = alpha_beta_world("pub fn alpha() {}\npub fn beta_unit() {}\n");
    // Healthy: the query matches only `beta_unit`, which touches no symbol.
    assert_eq!(
        graph_context(&world, "beta_unit").counters.graph,
        Some("graph_unavailable")
    );
    // Only the occurrence row of alpha's definition is rewritten, past the
    // end of the 40-byte source; its reverse key is still valid, so the
    // corrupt range would "overlap" beta's span and pull alpha's external
    // reference units in as if they were evidence.
    rekey_occurrence(&mut world, "src/lib.rs", "d", &id, (7, 12), (7, 120));
    let batch = graph_context(&world, "beta_unit");
    assert_eq!(batch.counters.graph, Some("graph_invalid"));
    assert!(
        compiler_paths(&batch).is_empty(),
        "{:?}",
        compiler_paths(&batch)
    );
    assert!(
        batch.items.iter().any(|item| item
            .handle
            .as_ref()
            .is_some_and(|handle| handle.path == "src/lib.rs")),
        "the lexical source context survives"
    );
}

#[test]
fn a_seed_occurrence_that_splits_a_codepoint_is_graph_invalid_in_context() {
    // `α` is bytes 7..9; the rewritten key starts at 8, inside it.
    let (mut world, id) = alpha_beta_world("pub fn α() {}\npub fn beta_unit() {}\n");
    rekey_occurrence(&mut world, "src/lib.rs", "d", &id, (7, 9), (8, 10));
    let batch = graph_context(&world, "beta_unit");
    assert_eq!(batch.counters.graph, Some("graph_invalid"));
    assert!(compiler_paths(&batch).is_empty());
}

fn many_units_world(files: usize, fns: usize) -> World {
    const SYM: &str = "rust-analyzer cargo toy 0.1.0 target().";
    let line = |j: usize| format!("pub fn f{j}() {{ needle(); target(); }}\n");
    let body: String = (0..fns).map(line).collect();
    let col = line(0).find("target").unwrap() as i32;
    let paths: Vec<String> = (0..files).map(|i| format!("src/m{i}.rs")).collect();
    let mut sources: Vec<(&str, &str)> = paths
        .iter()
        .map(|path| (path.as_str(), body.as_str()))
        .collect();
    sources.push(("src/def.rs", "pub fn target() {}\n"));
    let mut world = World::with_sources(&sources);
    let mut documents: Vec<Document> = paths
        .iter()
        .map(|path| {
            doc(
                path,
                (0..fns)
                    .map(|j| occ(&[j as i32, col, col + 6], SYM, REF))
                    .collect(),
            )
        })
        .collect();
    documents.push(doc("src/def.rs", vec![occ(&[0, 7, 13], SYM, DEF)]));
    world.import_ok(RA, "many-units", &artifact(documents));
    make_searchable(&mut world);
    world
}

#[test]
fn lexical_and_compiler_units_share_one_32_unit_bound() {
    // 10 files x 6 functions, 4 hits per file: 40 lexical candidates cut to
    // 32. Every function also references the uniquely defined `target`, so
    // 28 of the 60 referring units are NEW to the compiler graph - which
    // together with the 32 lexical units would be 60 delivered units.
    let world = many_units_world(10, 6);
    let batch = graph_context(&world, "needle");
    let delivered = |batch: &CandidateBatch| {
        batch
            .items
            .iter()
            .filter(|item| matches!(item.tier, 1 | 2) || item.tier == TIER_COMPILER)
            .count()
    };
    assert_eq!(delivered(&batch), 32);
    assert!(!compiler_paths(&batch).is_empty(), "compiler units kept");
    assert!(batch.counters.candidates_full, "a cut fills the window");
    let mut seen = std::collections::BTreeSet::new();
    for handle in batch.items.iter().filter_map(|item| item.handle.as_ref()) {
        assert!(
            seen.insert((handle.path.clone(), handle.start, handle.end)),
            "a unit is delivered twice: {}",
            handle.path
        );
    }
    // Under the bound nothing is cut: 2 files x 3 functions are 6 lexical
    // units, and every referring unit is one of them.
    let small = graph_context(&many_units_world(2, 3), "needle");
    assert_eq!(delivered(&small), 6);
    assert!(compiler_paths(&small).is_empty());
    assert_eq!(small.counters.graph, Some("ok"));
}

#[test]
fn references_request_validation_is_pure_syntax_and_bounds() {
    let request = |seed, limit, after: Option<&str>| ReferencesRequest {
        seed,
        limit,
        after: after.map(str::to_owned),
    };
    let symbol = |raw: &str| ReferencesSeed::SymbolId(raw.to_owned());
    assert!(
        request(symbol("0123456789abcdef"), 64, None)
            .validate()
            .is_ok()
    );
    assert!(
        request(symbol("0123456789abcdef"), 64, Some("src/a.rs#1-2"))
            .validate()
            .is_ok()
    );
    let refused = [
        request(symbol("NOTHEX"), 64, None),
        request(symbol("0123456789ABCDEF"), 64, None),
        request(symbol("0123456789abcdef"), 0, None),
        request(symbol("0123456789abcdef"), 257, None),
        request(symbol("0123456789abcdef"), 64, Some("no-cursor")),
        request(
            ReferencesSeed::Position {
                handle: "not-a-handle".to_owned(),
                byte_offset: 0,
            },
            64,
            None,
        ),
    ];
    for bad in refused {
        assert_eq!(bad.validate().unwrap_err().code(), "invalid_argument");
    }
}

#[test]
fn the_references_refusal_floor_covers_a_maximum_length_path() {
    // A reference whose handle carries a 4096-byte, tokenizer-expensive path
    // (and whose cursor repeats it): the real packing minimum is thousands of
    // tokens, far above a floor derived from a short synthetic path.
    let path: String = (0..4096usize)
        .map(|i| {
            let n = i.wrapping_mul(2_654_435_761) >> 7;
            b"abcdefghijklmnopqrstuvwxyz0123456789"[n % 36] as char
        })
        .collect();
    let handle = SourceHandle {
        workspace_id: "a".repeat(64),
        path: path.clone(),
        sha256: "b".repeat(64),
        start: 0,
        end: 10,
    };
    let item = context_foundry::graph::ReferenceItem {
        occurrence_id: "o".repeat(64),
        path: path.clone(),
        sha256: "b".repeat(64),
        start: 0,
        end: 5,
        line: 1,
        unit: handle,
        label: "fn example".to_owned(),
        edge_id: None,
    };
    let outcome = ReferencesOutcome {
        freshness: response::Freshness {
            workspace_id: "a".repeat(64),
            source_revision: 1,
            scan_state: "complete".to_owned(),
            pending_sources: 0,
            indexed_snapshot: "revision=1".to_owned(),
        },
        producer: None,
        snapshot: None,
        symbol_id: None,
        target: None,
        definitions: Vec::new(),
        definitions_truncated: false,
        items: vec![item],
        examined: 1,
        unresolved: 0,
        stale: 0,
        candidates_full: false,
        coverage: Coverage::Complete,
        more: true,
        resume: Some(format!("{path}#0-5")),
    };
    let error = response::pack_references(
        &outcome,
        response::Budget::request(1),
        &response::stdout_bytes,
    )
    .unwrap_err();
    let FoundryError::BudgetTooSmall { minimum_tokens } = error else {
        panic!("{error:?}")
    };
    assert!(
        minimum_tokens > 1000,
        "a long path is expensive: {minimum_tokens}"
    );
    assert!(
        response::references_refusal_floor() >= minimum_tokens,
        "the outcome-free hint {} is below the real minimum {minimum_tokens}",
        response::references_refusal_floor()
    );
}

// --- T003 review round 2: the final read re-proves resolution and sources ----

const TARGET_LIB: &str = "pub fn host_seed() { crate::target_fn(); }\n";
const TARGET_DEF: &str = "pub fn target_fn() {}\n";
const TARGET_CALLER: &str = "pub fn caller_a() { crate::target_fn(); }\n";

/// `target_fn_world`'s two definitions, imported in the order lib, callers,
/// def_one, def_two and CANCELLED before def_two: a partial snapshot that
/// holds ONE definition. Returns the staged artifact and manifest so a test
/// can replay the very same import.
fn partially_imported_target_world() -> (World, std::path::PathBuf, std::path::PathBuf) {
    let mut world = World::with_sources(&[
        ("src/lib.rs", TARGET_LIB),
        ("src/def_one.rs", TARGET_DEF),
        ("src/def_two.rs", TARGET_DEF),
        ("src/callers.rs", TARGET_CALLER),
    ]);
    let col = |text: &str| text.find("target_fn").unwrap() as i32;
    let bytes = artifact(vec![
        doc(
            "src/lib.rs",
            vec![occ(&[0, col(TARGET_LIB), col(TARGET_LIB) + 9], TFN, REF)],
        ),
        doc(
            "src/callers.rs",
            vec![occ(
                &[0, col(TARGET_CALLER), col(TARGET_CALLER) + 9],
                TFN,
                REF,
            )],
        ),
        doc("src/def_one.rs", vec![occ(&[0, 7, 16], TFN, DEF)]),
        doc("src/def_two.rs", vec![occ(&[0, 7, 16], TFN, DEF)]),
    ]);
    let manifest = world.manifest(RA, "2026-08-31", "partial-then-complete", &bytes);
    let (index_path, snapshot_path) = world.write_pair(&manifest, &bytes);
    // The fourth document (def_two) is never published.
    fault::arm(names::SCIP_BETWEEN_DOCUMENTS, 3, Action::Cancel);
    let partial = world
        .engine()
        .import_scip(&index_path, &snapshot_path, &Control::unbounded())
        .unwrap();
    fault::disarm_all();
    assert!(
        !partial.complete && partial.interrupted.is_some(),
        "{partial:?}"
    );
    assert_eq!((partial.definitions, partial.references), (1, 2));
    make_searchable(&mut world);
    (world, index_path, snapshot_path)
}

#[test]
fn a_same_snapshot_completion_at_the_final_barrier_drops_an_expansion_that_became_ambiguous() {
    let (world, index_path, snapshot_path) = partially_imported_target_world();
    // Collected from the partial snapshot, the symbol has ONE definition and
    // expands: callers.rs references it, and host_seed (the lexical hit)
    // references it too.
    let before = graph_context(&world, "host_seed");
    assert_eq!(compiler_paths(&before), ["src/callers.rs"]);
    assert_eq!(before.counters.graph, Some("ok"), "{:?}", before.counters);
    let snapshot_before = world.selected(RA).unwrap().tuple.snapshot_id;
    // At the barrier the SAME artifact and manifest are replayed: the second
    // definition is published under the SAME snapshot id, so every witnessed
    // row is still there and every scope still belongs to the snapshot - only
    // the symbol's uniqueness changed.
    fault::arm(
        names::CONTEXT_BEFORE_FINAL_VALIDATION,
        0,
        Action::Call(Box::new(move |ctx| {
            let report = ctx
                .engine
                .unwrap()
                .import_scip(&index_path, &snapshot_path, &Control::unbounded())
                .unwrap();
            assert!(report.complete, "{report:?}");
        })),
    );
    let batch = graph_context(&world, "host_seed");
    fault::disarm_all();
    assert_eq!(
        world.selected(RA).unwrap().tuple.snapshot_id,
        snapshot_before,
        "the same snapshot was completed"
    );
    assert!(
        compiler_paths(&batch).is_empty(),
        "an ambiguous symbol was expanded: {:?}",
        compiler_paths(&batch)
    );
    assert!(batch.counters.stale >= 1, "{:?}", batch.counters);
    assert_eq!(batch.counters.graph, Some("graph_stale"));
    // The completed graph agrees without any barrier.
    let after = graph_context(&world, "host_seed");
    assert!(compiler_paths(&after).is_empty());
}

#[test]
fn a_final_uniqueness_check_the_allowance_cannot_conclude_drops_the_expansion() {
    let world = target_fn_world(false, true);
    let id = symbol_id(RA, "src/def_one.rs", TFN);
    // 300 ineligible definition rows (no scope behind them) appear AFTER
    // collection and sort after the real one: the final walk finds the real
    // definition first, but cannot reach the end of the symbol's records
    // within its allowance, so uniqueness is unfinished.
    let keys: Vec<String> = (0..300)
        .map(|i| format!("{id}\0d\0src/zz{i:04}_old.rs\0{:020}\0{:020}", 0, 1))
        .collect();
    fault::arm(
        names::CONTEXT_BEFORE_FINAL_VALIDATION,
        0,
        Action::Call(Box::new(move |ctx| {
            ctx.engine
                .unwrap()
                .insert_compiler_by_symbol_for_tests(&keys, RA)
                .unwrap();
        })),
    );
    let batch = graph_context(&world, "host_seed");
    fault::disarm_all();
    assert!(
        compiler_paths(&batch).is_empty(),
        "{:?}",
        compiler_paths(&batch)
    );
    assert!(batch.counters.candidates_full, "{:?}", batch.counters);
    assert!(batch.counters.stale >= 1, "{:?}", batch.counters);
    assert_eq!(batch.counters.graph, Some("graph_stale"));
}

#[test]
fn a_corrupt_definition_chunk_at_the_final_barrier_is_corrupt_source_never_graph_ok() {
    let world = target_fn_world(false, true);
    // Clean: the unique definition lives in def_one.rs, which is neither a
    // lexical hit for `host_seed` nor a delivered reference unit.
    assert_eq!(
        graph_context(&world, "host_seed").counters.graph,
        Some("ok")
    );
    fault::arm(
        names::CONTEXT_BEFORE_FINAL_VALIDATION,
        0,
        Action::Call(Box::new(|ctx| {
            ctx.engine
                .unwrap()
                .overwrite_chunk_body_for_tests("src/def_one.rs", 0, "pub fn target_fn() { 1 }\n")
                .unwrap();
        })),
    );
    let error = world
        .engine()
        .context_candidates(
            "host_seed",
            context_foundry::Strategy::Graph,
            &Control::unbounded(),
        )
        .unwrap_err();
    fault::disarm_all();
    assert_eq!(error.code(), "corrupt_source", "{error:?}");
}

// --- T003 review round 3: the final pass's record allowance is shared -------

#[test]
fn final_membership_and_uniqueness_records_are_counted_together() {
    let world = target_fn_world(false, true);
    fault::reset_final_graph_records();
    let batch = graph_context(&world, "host_seed");
    assert_eq!(compiler_paths(&batch), ["src/callers.rs"]);
    assert_eq!(batch.counters.graph, Some("ok"), "{:?}", batch.counters);
    // Delivered compiler unit (callers.rs): 3 membership rows. The
    // already-selected lexical unit (lib.rs, still delivered): 3 more. The
    // uniqueness walk: 1 definition record, cached for the second witness.
    assert_eq!(fault::final_graph_records(), 3 + 3 + 1);
}

#[test]
fn many_occurrences_in_one_delivered_unit_need_one_witness() {
    const TFN_LOCAL: &str = "rust-analyzer cargo toy 0.1.0 target_fn().";
    // One hundred and twenty-seven references of the symbol inside the ONE
    // delivered unit that is also the lexical hit: the final pass must prove
    // that unit once, not once per occurrence.
    let calls = "target_fn(); ".repeat(127);
    let lib = format!("pub fn host_seed() {{ {calls}}}\n");
    let world =
        World::with_sources(&[("src/lib.rs", lib.as_str()), ("src/def_one.rs", TARGET_DEF)]);
    let stride = "target_fn(); ".len() as i32;
    let first = lib.find("target_fn").unwrap() as i32;
    let references: Vec<Occurrence> = (0..127)
        .map(|i| {
            occ(
                &[0, first + i * stride, first + i * stride + 9],
                TFN_LOCAL,
                REF,
            )
        })
        .collect();
    world.import_ok(
        RA,
        "one-unit-many-occurrences",
        &artifact(vec![
            doc("src/lib.rs", references),
            doc("src/def_one.rs", vec![occ(&[0, 7, 16], TFN_LOCAL, DEF)]),
        ]),
    );
    let mut world = world;
    make_searchable(&mut world);
    fault::reset_final_graph_records();
    let batch = graph_context(&world, "host_seed");
    assert_eq!(batch.counters.graph, Some("ok"), "{:?}", batch.counters);
    assert_eq!(batch.counters.stale, 0, "{:?}", batch.counters);
    assert_eq!(fault::final_graph_records(), 3 + 1);
}

#[test]
fn candidates_removed_by_the_unit_cut_are_never_probed() {
    // Forty distinct reference units compete for the 31 compiler slots (the
    // lexical first unit holds one): the cut candidates are never read. The
    // seed file sorts before the callers, so its already-selected witness is
    // collected before the 32-unit collection cap stops the reference scan.
    let caller = |i: usize| format!("pub fn c{i:02}() {{ crate::target_fn(); }}\n");
    let bodies: Vec<String> = (0..40).map(caller).collect();
    let mut owned: Vec<(String, String)> = vec![
        ("src/a_seed.rs".to_owned(), TARGET_LIB.to_owned()),
        ("src/def_one.rs".to_owned(), TARGET_DEF.to_owned()),
    ];
    for (i, body) in bodies.iter().enumerate() {
        owned.push((format!("src/c{i:02}.rs"), body.clone()));
    }
    let sources: Vec<(&str, &str)> = owned
        .iter()
        .map(|(path, body)| (path.as_str(), body.as_str()))
        .collect();
    let mut world = World::with_sources(&sources);
    let col = |text: &str| text.find("target_fn").unwrap() as i32;
    let mut documents = vec![
        doc(
            "src/a_seed.rs",
            vec![occ(&[0, col(TARGET_LIB), col(TARGET_LIB) + 9], TFN, REF)],
        ),
        doc("src/def_one.rs", vec![occ(&[0, 7, 16], TFN, DEF)]),
    ];
    for (i, body) in bodies.iter().enumerate() {
        documents.push(doc(
            &format!("src/c{i:02}.rs"),
            vec![occ(&[0, col(body), col(body) + 9], TFN, REF)],
        ));
    }
    world.import_ok(RA, "cut-never-probed", &artifact(documents));
    make_searchable(&mut world);
    fault::reset_final_graph_records();
    let batch = graph_context(&world, "host_seed");
    // The reference scan collects a_seed's already-selected witness and 32
    // new caller units (its own unit cap); the selection keeps 31 of those
    // for the 32-unit delivery bound. 31 x 3 membership rows, a_seed's 3,
    // and one (cached) uniqueness walk record: nothing is read for the cut
    // candidate.
    assert_eq!(fault::final_graph_records(), 31 * 3 + 3 + 1);
    assert!(batch.counters.candidates_full, "{:?}", batch.counters);
    let delivered = compiler_paths(&batch);
    assert_eq!(delivered.len(), 31, "{delivered:?}");
    assert!(
        !delivered.contains(&"src/c31.rs".to_owned()),
        "{delivered:?}"
    );
    assert_eq!(batch.counters.stale, 0, "{:?}", batch.counters);
}

#[test]
fn a_depleted_final_record_allowance_drops_the_expansion_with_candidates_full() {
    let world = target_fn_world(false, true);
    let id = symbol_id(RA, "src/def_one.rs", TFN);
    // After collection, 300 ineligible definition rows appear: membership
    // (3) plus the walk consume the whole 256-record allowance, so the walk
    // is unfinished and the second witness's membership cannot even charge.
    let keys: Vec<String> = (0..300)
        .map(|i| format!("{id}\0d\0src/zz{i:04}_old.rs\0{:020}\0{:020}", 0, 1))
        .collect();
    fault::arm(
        names::CONTEXT_BEFORE_FINAL_VALIDATION,
        0,
        Action::Call(Box::new(move |ctx| {
            ctx.engine
                .unwrap()
                .insert_compiler_by_symbol_for_tests(&keys, RA)
                .unwrap();
        })),
    );
    fault::reset_final_graph_records();
    let batch = graph_context(&world, "host_seed");
    fault::disarm_all();
    assert_eq!(
        fault::final_graph_records(),
        256,
        "the allowance is fully consumed"
    );
    assert!(
        compiler_paths(&batch).is_empty(),
        "{:?}",
        compiler_paths(&batch)
    );
    assert!(batch.counters.candidates_full, "{:?}", batch.counters);
    assert!(batch.counters.stale >= 1, "{:?}", batch.counters);
    assert_eq!(batch.counters.graph, Some("graph_stale"));
}
