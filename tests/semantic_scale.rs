//! Real fitting/cache/ANN/storage paths with a fixture byte tokenizer and vectors.
#![cfg(feature = "semantic")]
use context_foundry::neural::{
    cache::DEFAULT_CACHE_CAP_BYTES,
    fake::{FakeBehavior, FakeFactory},
    prepare::{self, PrepareOptions},
    profile::SemanticProfile,
};
use context_foundry::{Control, Engine, testkit};
use serde_json::json;
use std::time::{Duration, Instant};

#[test]
#[ignore = "bounded scale diagnostic; requires new FOUNDRY_SEMANTIC_SCALE_DIR outside Git"]
fn mixed_size_bootstrap_edit_delete_restart_reuses_vectors() {
    let dir = std::path::PathBuf::from(
        std::env::var_os("FOUNDRY_SEMANTIC_SCALE_DIR").expect("new output directory"),
    );
    assert!(!dir.starts_with(env!("CARGO_MANIFEST_DIR")));
    std::fs::create_dir(&dir).unwrap();
    let root = dir.join("root");
    std::fs::create_dir(&root).unwrap();
    let count: usize = std::env::var("FOUNDRY_SEMANTIC_SCALE_FILES")
        .unwrap_or_else(|_| "8192".into())
        .parse()
        .unwrap();
    assert!((128..=80_000).contains(&count));
    let mut bytes = 0;
    for i in 0..count {
        let folder = root.join(format!("area{:03}", i / 128));
        std::fs::create_dir_all(&folder).unwrap();
        let lines = match i % 64 {
            0 => 256,
            1..=7 => 32,
            _ => 1,
        };
        let body = format!(
            "/// Preserve queue order for customer {i}.\npub fn item_{i:05}(value:u64)->u64 {{\n{}    value + {i}\n}}\n",
            "    // retry only after checking the source identity\n".repeat(lines)
        );
        bytes += body.len();
        std::fs::write(folder.join(format!("item{i:05}.rs")), body).unwrap();
    }
    let started = Instant::now();
    let store = dir.join("store");
    let mut engine = Engine::initialize(&store, &root).unwrap();
    let indexed = engine
        .index(
            &root,
            &Control::with_deadline(Instant::now() + Duration::from_secs(300)),
        )
        .unwrap();
    let index_ms = started.elapsed().as_millis();
    drop(engine);
    let profile_path = testkit::write_semantic_profile(&dir, "scale", |d| {
        d.dimensions = 128;
    });
    let mut profile = SemanticProfile::load(&profile_path).unwrap();
    // Body windows exercise production chunking/fitting and parent-context reuse;
    // the fixture tokenizer has no model-specific segmentation behavior.
    profile.representation =
        context_foundry::neural::partition::Representation::Body { parent_tokens: 64 };
    profile.card_tokens = 512;
    std::fs::write(&profile_path, serde_json::to_vec(&profile).unwrap()).unwrap();
    let factory = FakeFactory::new(profile.descriptor.clone(), FakeBehavior::Ok);
    let run = |budget| {
        prepare::run(
            &store,
            &PrepareOptions {
                profile_path: &profile_path,
                budget_seconds: budget,
                development: false,
                cache_cap_bytes: DEFAULT_CACHE_CAP_BYTES,
                started: Instant::now(),
                control: &Control::unbounded(),
                priority_paths: &[],
            },
            factory.acquire(),
        )
        .unwrap()
    };
    let mut rounds = Vec::new();
    for _ in 0..2 {
        let report = run(240);
        let done = !report.partial;
        rounds.push(report);
        std::fs::write(dir.join("progress.json"),serde_json::to_vec_pretty(&json!({"scope":"fixture_embedding_scale","files":count,"bytes":bytes,"index_ms":index_ms,"rounds":rounds})).unwrap()).unwrap();
        if done {
            break;
        }
    }
    assert!(
        !rounds.last().unwrap().partial,
        "bounded cold run incomplete; progress retained"
    );
    assert!(rounds.last().unwrap().index_published);
    let before = factory.handle().embedded_inputs();
    let unchanged = run(90);
    assert!(!unchanged.partial);
    assert_eq!(
        (
            unchanged.embedded_units,
            unchanged.document_calls,
            unchanged.partitioned_sources
        ),
        (0, 0, 0)
    );
    assert_eq!(factory.handle().embedded_inputs(), before);
    let mut engine = Engine::open_existing(&store).unwrap();
    let edited = "area000/item00001.rs";
    let original = std::fs::read_to_string(root.join(edited)).unwrap();
    let changed = original.replace("value + 1", "value + 7001");
    engine.replace_source(edited, &changed).unwrap();
    engine.delete_source("area000/item00002.rs").unwrap();
    engine.refresh(&Control::unbounded()).unwrap();
    drop(engine);
    let delta = run(90);
    assert!(!delta.partial && delta.index_published);
    assert_eq!(delta.partitioned_sources, 1);
    assert!(
        delta.embedded_units > 0 && delta.embedded_units < 64,
        "only the changed body windows should embed"
    );
    let engine = Engine::open_existing(&store).unwrap();
    engine.replace_source(edited, &original).unwrap();
    drop(engine);
    let reverted = run(90);
    assert!(!reverted.partial && reverted.index_published);
    assert_eq!(
        (reverted.embedded_units, reverted.document_calls),
        (0, 0),
        "reverted input must reuse its cache"
    );
    let report = json!({"scope":"fixture_embedding_scale_not_model_throughput","files":count,"bytes":bytes,"indexed":indexed,"index_ms":index_ms,"dimensions":128,"representation":"body_parent64_limit512","rounds":rounds,"unchanged":unchanged,"changed_and_deleted":delta,"reverted":reverted,"elapsed_ms":started.elapsed().as_millis()});
    std::fs::write(
        dir.join("report.json"),
        serde_json::to_vec_pretty(&report).unwrap(),
    )
    .unwrap();
    println!("{report}");
}
