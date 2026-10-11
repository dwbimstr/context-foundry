//! Bounded fitting/cache/ANN/storage runs. An explicit real profile selects the
//! ordinary supervised CPU worker; otherwise vectors/tokenizer are fixture-only.
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

/// Unlike the repeated-family stress fixture below, this indexes an explicitly
/// supplied public repository and preserves partial real-model progress. The
/// caller freezes acquisition identity and priority paths before running it.
#[test]
#[ignore = "real repository bootstrap; explicit root/profile/plan outside Git"]
fn representative_repository_progress_and_resume() {
    use context_foundry::{Strategy, response, store::ContextOptions};
    use std::path::PathBuf;
    let env_path = |key| PathBuf::from(std::env::var_os(key).expect(key));
    let dir = env_path("FOUNDRY_SEMANTIC_SCALE_DIR");
    let root = env_path("FOUNDRY_SEMANTIC_SCALE_ROOT")
        .canonicalize()
        .unwrap();
    assert!(dir.is_absolute() && !dir.starts_with(env!("CARGO_MANIFEST_DIR")));
    assert!(!dir.starts_with(&root));
    std::fs::create_dir(&dir).unwrap();
    let plan: serde_json::Value =
        serde_json::from_slice(&std::fs::read(env_path("FOUNDRY_SEMANTIC_SCALE_PLAN")).unwrap())
            .unwrap();
    let priorities: Vec<String> = serde_json::from_value(plan["priority_paths"].clone()).unwrap();
    prepare::validate_priority_paths(&priorities).unwrap();
    let budgets: Vec<u64> = serde_json::from_value(plan["budgets"].clone()).unwrap();
    assert_eq!(budgets.len(), 4, "cold, resume, edit/delete, revert");
    assert!(budgets.iter().all(|n| (30..=1800).contains(n)) && budgets.iter().sum::<u64>() <= 1800);
    let mut profile = SemanticProfile::load(&env_path("FOUNDRY_SEMANTIC_SCALE_PROFILE")).unwrap();
    profile.representation =
        context_foundry::neural::partition::Representation::Body { parent_tokens: 64 };
    profile.card_tokens = 512;
    let profile_path = dir.join("profile.json");
    std::fs::write(&profile_path, serde_json::to_vec_pretty(&profile).unwrap()).unwrap();
    std::fs::write(
        dir.join("plan.json"),
        serde_json::to_vec_pretty(&plan).unwrap(),
    )
    .unwrap();
    let store = dir.join("store");
    let started = Instant::now();
    let mut engine = Engine::initialize(&store, &root).unwrap();
    let indexed = engine
        .index(
            &root,
            &Control::with_deadline(Instant::now() + Duration::from_secs(600)),
        )
        .unwrap();
    let index_ms = started.elapsed().as_millis();
    let status = engine.status().unwrap();
    let mut report = json!({"scope":"real_model_public_repository_bounded_progress_not_full_coverage_acceptance", "root":root,"plan":plan,
        "profile":profile,"indexed":indexed,"index_ms":index_ms,"store_status":status,"rounds":[],"probes":[]});
    let save = |report: &serde_json::Value| {
        std::fs::write(
            dir.join("report.json"),
            serde_json::to_vec_pretty(report).unwrap(),
        )
        .unwrap()
    };
    save(&report);
    assert!(!indexed.partial, "indexing incomplete; retained report");
    // These paths were selected from the corpus before running, never fabricated
    // or copied into it. Changes affect only this experiment's store snapshot.
    assert!(priorities.len() >= 2);
    let edited = &priorities[0];
    let deleted = &priorities[1];
    let original = std::fs::read_to_string(root.join(edited)).unwrap();
    let deleted_original = std::fs::read_to_string(root.join(deleted)).unwrap();
    assert_eq!(
        engine.source(edited).unwrap().unwrap().hash,
        context_foundry::digest(original.as_bytes())
    );
    assert_eq!(
        engine.source(deleted).unwrap().unwrap().hash,
        context_foundry::digest(deleted_original.as_bytes())
    );
    drop(engine);
    for (phase, budget) in ["cold", "resume", "edit_delete", "revert"]
        .into_iter()
        .zip(budgets)
    {
        let mut before_partition = None;
        let mut before_vectors = Vec::new();
        if phase == "edit_delete" || phase == "revert" {
            let mut engine = Engine::open_existing(&store).unwrap();
            before_partition = engine.semantic_partition(edited).unwrap();
            if let Some(partition) = &before_partition {
                for unit in &partition.units {
                    let cached = engine
                        .semantic_cache_lookup(
                            &unit.input_key,
                            &partition.function_digest,
                            profile.descriptor.dimensions as usize,
                        )
                        .unwrap();
                    before_vectors.push((unit.input_key.clone(), cached));
                }
            }
            if phase == "edit_delete" {
                engine.replace_source(edited, &format!("{original}\n// Representative maintenance probe: source revision changed.\n")).unwrap();
                engine.delete_source(deleted).unwrap();
            } else {
                engine.replace_source(edited, &original).unwrap();
                engine.replace_source(deleted, &deleted_original).unwrap();
            }
            engine.refresh(&Control::unbounded()).unwrap();
        }
        let run = prepare::cli_prepare_with_priorities(
            &store,
            &profile_path,
            budget,
            true,
            DEFAULT_CACHE_CAP_BYTES,
            Instant::now(),
            &Control::unbounded(),
            &priorities,
        )
        .unwrap();
        report["rounds"]
            .as_array_mut()
            .unwrap()
            .push(json!({"phase":phase,"report":run}));
        save(&report); // Failures and budget exhaustion survive later assertions.
        assert!(
            run.provider_code.is_none(),
            "provider failure; retained report"
        );
        assert!(
            run.index_published && run.index_entries > 0,
            "partial coverage must be searchable"
        );
        let engine = Engine::open_existing(&store).unwrap();
        report["rounds"].as_array_mut().unwrap().last_mut().unwrap()["coverage"] =
            serde_json::to_value(
                engine
                    .semantic_state()
                    .unwrap()
                    .and_then(|state| state.coverage),
            )
            .unwrap();
        if phase == "edit_delete" || phase == "revert" {
            let after = engine
                .semantic_partition(edited)
                .unwrap()
                .expect("priority source partitioned");
            assert_eq!(
                after.source_hash,
                engine.source(edited).unwrap().unwrap().hash
            );
            let mut cached_before = 0;
            for (key, before) in before_vectors {
                let after_vector = engine
                    .semantic_cache_lookup(
                        &key,
                        &after.function_digest,
                        profile.descriptor.dimensions as usize,
                    )
                    .unwrap();
                if let context_foundry::neural::cache::CacheLookup::Hit(before) = before {
                    cached_before += 1;
                    let context_foundry::neural::cache::CacheLookup::Hit(after) = after_vector
                    else {
                        panic!("unchanged vector evicted below cache cap")
                    };
                    assert_eq!(
                        before, after,
                        "a source edit must preserve content-addressed vectors"
                    );
                }
            }
            report["rounds"].as_array_mut().unwrap().last_mut().unwrap()["retained_vector_checks"] = json!({"cached_vectors_verified_unchanged":cached_before,"before_partition_present":before_partition.is_some(),"after_partition":after});
            save(&report);
            assert!(
                cached_before > 0,
                "cache reuse check requires previously embedded inputs"
            );
        }
        drop(engine);
        let query = "preserve order while removing elements";
        let load_started = Instant::now();
        let runtime =
            context_foundry::neural::query::QueryRuntime::acquire(&profile_path, true).unwrap();
        let load_ms = load_started.elapsed().as_millis();
        let engine = Engine::open_existing(&store).unwrap();
        let control = Control::with_deadline(Instant::now() + Duration::from_secs(30));
        let query_started = Instant::now();
        let dense = runtime
            .window(
                &engine,
                query,
                Instant::now() + Duration::from_secs(30),
                &control,
            )
            .unwrap();
        let candidates = engine
            .context_candidates_with(
                query,
                Strategy::Search,
                &control,
                &ContextOptions {
                    dense: Some(&dense),
                    experimental_behavior: true,
                    ..Default::default()
                },
            )
            .unwrap();
        for item in &candidates.batch.items {
            if let Some(handle) = &item.handle {
                assert_eq!(
                    handle.sha256,
                    engine.source(&handle.path).unwrap().unwrap().hash
                );
                if phase == "edit_delete" {
                    assert_ne!(&handle.path, deleted);
                }
            }
        }
        let packed = response::pack_enriched_context(
            &candidates,
            None,
            response::Budget::request(2048),
            &response::stdout_bytes,
        )
        .unwrap();
        report["probes"].as_array_mut().unwrap().push(json!({"phase":phase,"query":query,"load_ms":load_ms,"query_and_delivery_ms":query_started.elapsed().as_millis(),"dense_hits":dense.hits.len(),"tokens":packed.tokens,"response":packed.text,"recall_acceptance":false}));
        save(&report);
    }
    report["elapsed_ms"] = json!(started.elapsed().as_millis());
    save(&report);
    println!(
        "representative report: {}",
        dir.join("report.json").display()
    );
}

#[test]
#[ignore = "real query/ANN delivery against a completed outside-Git scale run"]
fn real_model_serves_existing_scale_store() {
    use context_foundry::{Strategy, response, store::ContextOptions};
    let dir = std::path::PathBuf::from(
        std::env::var_os("FOUNDRY_SEMANTIC_SCALE_EXISTING").expect("completed scale directory"),
    );
    assert!(!dir.starts_with(env!("CARGO_MANIFEST_DIR")));
    let report: serde_json::Value =
        serde_json::from_slice(&std::fs::read(dir.join("report.json")).unwrap()).unwrap();
    assert_eq!(
        report["scope"],
        "real_model_synthetic_corpus_not_real_repository_distribution"
    );
    let started = Instant::now();
    let runtime =
        context_foundry::neural::query::QueryRuntime::acquire(&dir.join("profile.json"), true)
            .unwrap();
    let acquisition_ms = started.elapsed().as_millis();
    let engine = Engine::open_existing(&dir.join("store")).unwrap();
    let control = Control::with_deadline(Instant::now() + Duration::from_secs(30));
    let query = "preserve queue order and retry only after checking source identity";
    let started = Instant::now();
    let dense = runtime
        .window(
            &engine,
            query,
            Instant::now() + Duration::from_secs(30),
            &control,
        )
        .unwrap();
    assert!(!dense.hits.is_empty());
    let query_ms = started.elapsed().as_millis();
    let candidates = engine
        .context_candidates_with(
            query,
            Strategy::Search,
            &control,
            &ContextOptions {
                dense: Some(&dense),
                experimental_behavior: true,
                ..Default::default()
            },
        )
        .unwrap();
    assert!(!candidates.batch.items.is_empty());
    for item in &candidates.batch.items {
        if let Some(handle) = &item.handle {
            let source = engine.source(&handle.path).unwrap().unwrap();
            assert_eq!(handle.sha256, source.hash);
        }
    }
    let packed = response::pack_enriched_context(
        &candidates,
        None,
        response::Budget::request(2048),
        &response::stdout_bytes,
    )
    .unwrap();
    assert!(packed.tokens <= 2048 && packed.text.contains("source identity"));
    let result = json!({"scope":"real_query_ann_and_current_source_delivery_not_recall_acceptance", "files":report["files"],
        "distinct_content_families":report["unique_source_families"],"worker_acquisition_ms":acquisition_ms,"query_ms":query_ms,
        "total_query_and_delivery_ms":started.elapsed().as_millis(),"dense_hits":dense.hits.len(),"source_candidates":candidates.batch.items.len(),
        "tokens":packed.tokens,"function_digest":runtime.function_digest(),"response":packed.text});
    let file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(dir.join("serving.json"))
        .unwrap();
    serde_json::to_writer_pretty(file, &result).unwrap();
    println!("{result}");
}

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
    let real_profile =
        std::env::var_os("FOUNDRY_SEMANTIC_SCALE_PROFILE").map(std::path::PathBuf::from);
    let families: usize = std::env::var("FOUNDRY_SEMANTIC_SCALE_FAMILIES")
        .ok()
        .map(|s| s.parse().unwrap())
        .unwrap_or(count);
    assert!((8..=count).contains(&families));
    let budget: u64 = std::env::var("FOUNDRY_SEMANTIC_SCALE_BUDGET")
        .ok()
        .map(|s| s.parse().unwrap())
        .unwrap_or(240);
    assert!((60..=3600).contains(&budget));
    let mut bytes = 0;
    for i in 0..count {
        let folder = root.join(format!("area{:03}", i / 128));
        std::fs::create_dir_all(&folder).unwrap();
        let family = i % families;
        let lines = match family % 64 {
            0 => 256,
            1..=7 => 32,
            _ => 1,
        };
        let body = format!(
            "/// Preserve queue order for customer {family}.\npub fn item_{family:05}(value:u64)->u64 {{\n{}    value + {family}\n}}\n",
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
    let profile_path = if let Some(path) = real_profile.as_ref() {
        path.clone()
    } else {
        testkit::write_semantic_profile(&dir, "scale", |d| {
            d.dimensions = 128;
        })
    };
    let mut profile = SemanticProfile::load(&profile_path).unwrap();
    // Body windows exercise production chunking/fitting and parent-context reuse;
    // the fixture tokenizer has no model-specific segmentation behavior.
    profile.representation =
        context_foundry::neural::partition::Representation::Body { parent_tokens: 64 };
    profile.card_tokens = 512;
    // Never mutate the operator's profile. All experiment settings are retained.
    let profile_path = dir.join("profile.json");
    std::fs::write(&profile_path, serde_json::to_vec(&profile).unwrap()).unwrap();
    let factory = FakeFactory::new(profile.descriptor.clone(), FakeBehavior::Ok);
    let run = |budget| {
        if real_profile.is_some() {
            return prepare::cli_prepare(
                &store,
                &profile_path,
                budget,
                true,
                DEFAULT_CACHE_CAP_BYTES,
                Instant::now(),
                &Control::unbounded(),
            )
            .unwrap();
        }
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
        let report = run(budget);
        let done = !report.partial;
        rounds.push(report);
        std::fs::write(dir.join("progress.json"),serde_json::to_vec_pretty(&json!({"scope":if real_profile.is_some() {"real_model_synthetic_corpus"} else {"fixture_embedding_scale"},"files":count,"unique_source_families":families,"bytes":bytes,"index_ms":index_ms,"rounds":rounds})).unwrap()).unwrap();
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
    let report = json!({"scope":if real_profile.is_some() {"real_model_synthetic_corpus_not_real_repository_distribution"} else {"fixture_embedding_scale_not_model_throughput"},"files":count,"unique_source_families":families,"bytes":bytes,"indexed":indexed,"index_ms":index_ms,"dimensions":profile.descriptor.dimensions,"function_digest":profile.descriptor.digest(),"representation":"body_parent64_limit512","rounds":rounds,"unchanged":unchanged,"changed_and_deleted":delta,"reverted":reverted,"elapsed_ms":started.elapsed().as_millis()});
    std::fs::write(
        dir.join("report.json"),
        serde_json::to_vec_pretty(&report).unwrap(),
    )
    .unwrap();
    println!("{report}");
}
