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

/// Inventory every admitted source with the production body fitter and exact
/// tokenizer. This measures the whole inference obligation without pretending
/// that tokenization or a small embedded sample completed model coverage.
#[test]
#[ignore = "explicit 60–80k public manifest, profile and new output outside Git"]
fn representative_corpus_workload_inventory() {
    use context_foundry::neural::{partition, tokenize::DocumentTokenizer};
    use std::{collections::BTreeMap, path::PathBuf};
    let path = |key| PathBuf::from(std::env::var_os(key).expect(key));
    let dir = path("FOUNDRY_SEMANTIC_SCALE_DIR");
    let root = path("FOUNDRY_SEMANTIC_SCALE_ROOT").canonicalize().unwrap();
    assert!(
        dir.is_absolute()
            && !dir.starts_with(env!("CARGO_MANIFEST_DIR"))
            && !dir.starts_with(&root)
    );
    std::fs::create_dir(&dir).unwrap();
    let manifest_bytes = std::fs::read(path("FOUNDRY_SEMANTIC_SCALE_MANIFEST")).unwrap();
    let manifest: serde_json::Value = serde_json::from_slice(&manifest_bytes).unwrap();
    let files = manifest["files"].as_array().unwrap();
    assert!((60_000..=80_000).contains(&files.len()));
    let profile = SemanticProfile::load(&path("FOUNDRY_SEMANTIC_SCALE_PROFILE")).unwrap();
    let partition::Representation::Body { parent_tokens } = profile.representation else {
        panic!("body profile required")
    };
    // An explicit continuation reuses only a verified fitted prefix. The full
    // key set is intentionally not persisted, so distinct-input totals become
    // bounds across phases rather than an invented exact union.
    let previous = std::env::var_os("FOUNDRY_SEMANTIC_SCALE_PREVIOUS").map(|path| {
        let path = PathBuf::from(path);
        let bytes = std::fs::read(&path).unwrap();
        let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(
            value["manifest_sha256"],
            context_foundry::digest(&manifest_bytes)
        );
        assert_eq!(value["profile"], serde_json::to_value(&profile).unwrap());
        assert_eq!(value["complete"], false);
        assert!(!path.parent().unwrap().join("failure.json").exists());
        (path, value, context_foundry::digest(&bytes))
    });
    let prior = |key: &str| {
        previous
            .as_ref()
            .map_or(0, |(_, v, _)| v[key].as_u64().unwrap())
    };
    let first = prior("visited_files") as usize;
    assert!(first < files.len());
    let prior_keys = previous.as_ref().map_or(0, |(_, v, _)| {
        v["distinct_model_inputs_lower_bound"]
            .as_u64()
            .unwrap_or_else(|| v["distinct_model_inputs"].as_u64().unwrap())
    });
    let prior_keys_upper = previous.as_ref().map_or(0, |(_, v, _)| {
        v["distinct_model_inputs_upper_bound"]
            .as_u64()
            .unwrap_or(prior_keys)
    });
    let prior_tokens = previous.as_ref().map_or(0, |(_, v, _)| {
        v["distinct_input_tokens_lower_bound"]
            .as_u64()
            .unwrap_or_else(|| v["distinct_input_tokens"].as_u64().unwrap())
    });
    let prior_tokens_upper = previous.as_ref().map_or(0, |(_, v, _)| {
        v["distinct_input_tokens_upper_bound"]
            .as_u64()
            .unwrap_or(prior_tokens)
    });
    let store = previous.as_ref().map_or_else(
        || dir.join("store"),
        |(path, v, _)| {
            v["store_directory"]
                .as_str()
                .map_or_else(|| path.parent().unwrap().join("store"), PathBuf::from)
        },
    );
    let started = Instant::now();
    let control = Control::with_deadline(started + Duration::from_secs(1800));
    let mut engine = if previous.is_some() {
        Engine::open_existing(&store)
    } else {
        Engine::initialize(&store, &root)
    }
    .unwrap();
    let indexed = engine.index(&root, &control).unwrap();
    assert!(!indexed.partial);
    let index_ms = started.elapsed().as_millis();
    if let Some((_, previous, _)) = &previous {
        assert_eq!(
            previous["indexed"]["source_revision"],
            indexed.source_revision
        );
        assert_eq!((indexed.changed, indexed.deleted), (0, 0));
        for file in files.iter().take(first) {
            control.check().unwrap();
            let name = file[0].as_str().unwrap();
            if let Some(source) = engine.source(name).unwrap() {
                assert_eq!(file[2], source.hash);
                assert_eq!(
                    file[2],
                    context_foundry::digest(&std::fs::read(root.join(name)).unwrap())
                );
            }
        }
    }
    let prefix_verification_ms = started.elapsed().as_millis() - index_ms;
    let tokenizer = DocumentTokenizer::load(&profile).unwrap();
    let function = profile.descriptor.digest();
    let recipe = partition::CardRecipe {
        template: &profile.descriptor.document_template,
        function_digest: &function,
        card_tokens: profile.card_tokens as usize,
    };
    let mut keys = BTreeMap::<String, usize>::new();
    let (mut visited, mut admitted, mut excluded, mut empty, mut cards, mut tokens) = (
        first as u64,
        prior("admitted_sources"),
        prior("excluded_files"),
        prior("sources_without_cards"),
        prior("source_card_locations"),
        prior("tokens_including_duplicate_inputs"),
    );
    let mut projects: BTreeMap<String, [u64; 3]> =
        previous.as_ref().map_or_else(BTreeMap::new, |(_, v, _)| {
            serde_json::from_value(v["projects_admitted_cards_tokens"].clone()).unwrap()
        });
    let save = |visited,
                admitted,
                excluded,
                empty,
                cards,
                tokens,
                keys: &BTreeMap<String, usize>,
                projects: &BTreeMap<String, [u64; 3]>| {
        let key_lower = prior_keys.max(keys.len() as u64);
        let key_upper = prior_keys_upper + keys.len() as u64;
        let token_sum = keys.values().sum::<usize>() as u64;
        let report = json!({"scope":"corpus_exact_input_workload_not_embedding_or_retrieval_acceptance",
            "first_manifest_offset":first,"previous_report_sha256":previous.as_ref().map(|(_,_,h)| h),"store_directory":store,
            "distinct_model_inputs_lower_bound":key_lower,"distinct_model_inputs_upper_bound":key_upper,
            "distinct_input_tokens_lower_bound":prior_tokens.max(token_sum),"distinct_input_tokens_upper_bound":prior_tokens_upper+token_sum,
            "f32_vector_payload_bytes_lower_bound":key_lower * profile.descriptor.dims() as u64 * 4,"f32_vector_payload_bytes_upper_bound":key_upper * profile.descriptor.dims() as u64 * 4, "complete":visited == files.len() as u64, "inference_executed":false,
            "manifest_sha256":context_foundry::digest(&manifest_bytes), "regular_files":files.len(),"visited_files":visited,"admitted_sources":admitted,"excluded_files":excluded,"sources_without_cards":empty,
            "source_card_locations":cards,"tokens_including_duplicate_inputs":tokens,"distinct_model_inputs":previous.is_none().then_some(keys.len()),"distinct_input_tokens":previous.is_none().then_some(token_sum),
            "f32_vector_payload_bytes":previous.is_none().then_some(keys.len() * profile.descriptor.dims() * 4),
            "projects_admitted_cards_tokens":projects,"profile":profile,"indexed":indexed,"index_ms":index_ms,"prefix_verification_ms":prefix_verification_ms,"elapsed_ms":started.elapsed().as_millis()});
        std::fs::write(
            dir.join("workload.json"),
            serde_json::to_vec_pretty(&report).unwrap(),
        )
        .unwrap();
    };
    save(
        visited, admitted, excluded, empty, cards, tokens, &keys, &projects,
    );
    for file in files.iter().skip(first) {
        if control.check().is_err() {
            break;
        }
        let name = file[0].as_str().unwrap();
        let Some(source) = engine.source(name).unwrap() else {
            visited += 1;
            excluded += 1;
            continue;
        };
        let body = std::fs::read_to_string(root.join(name)).unwrap();
        assert_eq!(file[1], body.len());
        assert_eq!(file[2], context_foundry::digest(body.as_bytes()));
        assert_eq!(file[2], source.hash, "indexed source differs from manifest");
        admitted += 1;
        let mut count = 0;
        let project = projects
            .entry(name.split('/').next().unwrap().into())
            .or_default();
        project[0] += 1;
        let fitted = partition::body::visit(
            &body,
            context_foundry::syntax::Lang::from_path(name),
            &recipe,
            parent_tokens as usize,
            &tokenizer,
            |_, card| {
                count += 1;
                cards += 1;
                tokens += card.ids.len() as u64;
                project[1] += 1;
                project[2] += card.ids.len() as u64;
                if let Some(previous) = keys.insert(card.input_key, card.ids.len()) {
                    assert_eq!(previous, card.ids.len());
                }
            },
        );
        if let Err(error) = fitted {
            save(
                visited, admitted, excluded, empty, cards, tokens, &keys, &projects,
            );
            std::fs::write(
                dir.join("failure.json"),
                serde_json::to_vec_pretty(
                    &json!({"path":name,"source_sha256":source.hash,"error":error.to_string()}),
                )
                .unwrap(),
            )
            .unwrap();
            panic!("{name}: {error}");
        }
        visited += 1;
        empty += u64::from(count == 0);
        if visited % 1000 == 0 {
            save(
                visited, admitted, excluded, empty, cards, tokens, &keys, &projects,
            );
        }
    }
    save(
        visited, admitted, excluded, empty, cards, tokens, &keys, &projects,
    );
    assert_eq!(
        visited,
        files.len() as u64,
        "deadline: partial workload report retained"
    );
    assert_eq!(admitted, engine.status().unwrap().source_count);
}

/// Real source/tokenizer/isolated worker, with the same exact inputs in every
/// arm. This diagnoses inference geometry; it is not a corpus-completion or
/// retrieval-quality test. The operator supplies frozen source hashes first.
#[test]
#[ignore = "real CPU comparison; explicit root/profile/plan/new output outside Git"]
fn representative_cpu_batch_comparison() {
    use context_foundry::neural::{
        partition, provider::TokenizedInput, supervisor, tokenize::DocumentTokenizer,
    };
    use std::path::PathBuf;
    let env_path = |key| PathBuf::from(std::env::var_os(key).expect(key));
    let dir = env_path("FOUNDRY_SEMANTIC_SCALE_DIR");
    let root = env_path("FOUNDRY_SEMANTIC_SCALE_ROOT")
        .canonicalize()
        .unwrap();
    assert!(
        dir.is_absolute()
            && !dir.starts_with(env!("CARGO_MANIFEST_DIR"))
            && !dir.starts_with(&root)
    );
    std::fs::create_dir(&dir).unwrap();
    let plan: serde_json::Value =
        serde_json::from_slice(&std::fs::read(env_path("FOUNDRY_SEMANTIC_SCALE_PLAN")).unwrap())
            .unwrap();
    let files = plan["selected_files"].as_array().unwrap();
    assert!((4..=16).contains(&files.len()));
    let started = Instant::now();
    let control = Control::with_deadline(started + Duration::from_secs(900));
    let mut profile = SemanticProfile::load(&env_path("FOUNDRY_SEMANTIC_SCALE_PROFILE")).unwrap();
    profile.card_tokens = 512;
    profile.batch = 8;
    profile.representation = partition::Representation::Body { parent_tokens: 64 };
    let tokenizer = DocumentTokenizer::load(&profile).unwrap();
    let digest = profile.descriptor.digest();
    let recipe = partition::CardRecipe {
        template: &profile.descriptor.document_template,
        function_digest: &digest,
        card_tokens: 512,
    };
    let mut inputs = Vec::new();
    let mut sources = Vec::new();
    for file in files {
        let path = file[0].as_str().unwrap();
        context_foundry::store::validate_path(path).unwrap();
        let source = std::fs::read_to_string(root.join(path)).unwrap();
        assert_eq!(file[1], source.len());
        assert_eq!(file[2], context_foundry::digest(source.as_bytes()));
        let mut cards = Vec::new();
        partition::body::visit(
            &source,
            context_foundry::syntax::Lang::from_path(path),
            &recipe,
            64,
            &tokenizer,
            |_, card| cards.push(card),
        )
        .unwrap();
        assert!(!cards.is_empty());
        let card = &cards[cards.len() / 2];
        sources.push(json!({"path":path,"source_sha256":file[2],"card_index":cards.len()/2,"cards":cards.len(),"start":card.start,"end":card.end,"input_key":card.input_key,"tokens":card.ids.len()}));
        inputs.push(TokenizedInput {
            ids: card.ids.clone(),
        });
    }
    assert_eq!(
        sources
            .iter()
            .map(|s| s["input_key"].as_str().unwrap())
            .collect::<std::collections::BTreeSet<_>>()
            .len(),
        inputs.len()
    );
    std::fs::write(dir.join("inputs.json"), serde_json::to_vec_pretty(&json!({"profile":profile,"sources":sources,"selection":"middle body card from each frozen size/area source","tokens":inputs.iter().map(|i|i.ids.len()).sum::<usize>()})).unwrap()).unwrap();
    let mut reference: Option<Vec<Vec<f32>>> = None;
    let mut rows = Vec::new();
    // Reverse the order in the second round; load time is separately reported.
    let variants = [
        (2, 1),
        (4, 8),
        (4, 1),
        (2, 8),
        (2, 8),
        (4, 1),
        (4, 8),
        (2, 1),
    ];
    for (run, (threads, batch)) in variants.into_iter().enumerate() {
        profile.cpu.threads = threads;
        profile.cpu.seconds = 3600;
        let acquisition = Instant::now();
        let mut worker = supervisor::acquire_until(&profile, true, &control).unwrap();
        let acquisition_ms = acquisition.elapsed().as_millis();
        let inference = Instant::now();
        let mut vectors = Vec::new();
        let mut error = None;
        for chunk in inputs.chunks(batch) {
            match worker.embed_documents(chunk, &control) {
                Ok(output) => vectors.extend(output),
                Err(e) => {
                    error = Some(e.to_string());
                    break;
                }
            }
        }
        let inference_ms = inference.elapsed().as_millis();
        drop(worker);
        let mut minimum_cosine = 1.0f64;
        let mut maximum_absolute_error = 0.0f64;
        let valid = vectors.len() == inputs.len()
            && vectors.iter().all(|v| {
                v.len() == profile.descriptor.dims()
                    && v.iter().all(|x| x.is_finite())
                    && (v.iter().map(|x| f64::from(*x).powi(2)).sum::<f64>().sqrt() - 1.0).abs()
                        < 0.0001
            });
        if valid && let Some(expected) = &reference {
            for (a, b) in vectors.iter().zip(expected) {
                let norm = |v: &[f32]| v.iter().map(|x| f64::from(*x).powi(2)).sum::<f64>().sqrt();
                let cosine = a
                    .iter()
                    .zip(b)
                    .map(|(a, b)| f64::from(*a) * f64::from(*b))
                    .sum::<f64>()
                    / (norm(a) * norm(b));
                minimum_cosine = minimum_cosine.min(cosine);
                for (a, b) in a.iter().zip(b) {
                    maximum_absolute_error =
                        maximum_absolute_error.max((f64::from(*a) - f64::from(*b)).abs());
                }
            }
        }
        if valid && reference.is_none() {
            reference = Some(vectors.clone());
        }
        std::fs::write(
            dir.join(format!("vectors-{run}.json")),
            serde_json::to_vec(&vectors).unwrap(),
        )
        .unwrap();
        rows.push(json!({"run":run,"threads":threads,"batch":batch,"acquisition_ms":acquisition_ms,"inference_ms":inference_ms,"completed_inputs":vectors.len(),"valid":valid,"minimum_cosine":minimum_cosine,"maximum_absolute_error":maximum_absolute_error,"error":error}));
        std::fs::write(dir.join("report.json"), serde_json::to_vec_pretty(&json!({"scope":"representative_input_cpu_batch_comparison_not_corpus_completion_or_quality","function_digest":digest,"deadline_seconds":900,"tokens_per_arm":inputs.iter().map(|i|i.ids.len()).sum::<usize>(),"rows":rows,"elapsed_ms":started.elapsed().as_millis()})).unwrap()).unwrap();
        assert!(
            error.is_none() && valid && minimum_cosine >= 0.9999,
            "{}",
            rows.last().unwrap()
        );
    }
}

/// Inspect explicitly named priority files after an external CLI run. Cached
/// inputs are counted separately from complete source and repository coverage.
#[test]
#[ignore = "explicit existing experiment store and frozen source plan"]
fn representative_priority_coverage() {
    use context_foundry::neural::{cache::CacheLookup, partition::Representation};
    use std::path::PathBuf;
    let env_path = |key| PathBuf::from(std::env::var_os(key).expect(key));
    let dir = env_path("FOUNDRY_SEMANTIC_SCALE_DIR");
    assert!(dir.is_absolute() && !dir.starts_with(env!("CARGO_MANIFEST_DIR")));
    let engine = Engine::open_existing(&env_path("FOUNDRY_SEMANTIC_SCALE_EXISTING")).unwrap();
    let plan: serde_json::Value =
        serde_json::from_slice(&std::fs::read(env_path("FOUNDRY_SEMANTIC_SCALE_PLAN")).unwrap())
            .unwrap();
    let mut profile = SemanticProfile::load(&env_path("FOUNDRY_SEMANTIC_SCALE_PROFILE")).unwrap();
    profile.representation = Representation::Body { parent_tokens: 64 };
    profile.card_tokens = 512;
    let mut rows = Vec::new();
    let files = plan["selected_files"].as_array().unwrap();
    assert!((1..=16).contains(&files.len()));
    for file in files {
        let path = file[0].as_str().unwrap();
        let source = engine.source(path).unwrap().unwrap();
        assert_eq!(file[2], source.hash);
        let record = engine.semantic_partition(path).unwrap();
        let mut cached = 0;
        if let Some(record) = &record {
            assert_eq!(record.source_hash, source.hash);
            assert_eq!(record.recipe_id, profile.recipe_id());
            assert_eq!(record.function_digest, profile.descriptor.digest());
            for unit in &record.units {
                if matches!(
                    engine
                        .semantic_cache_lookup(
                            &unit.input_key,
                            &record.function_digest,
                            profile.descriptor.dims()
                        )
                        .unwrap(),
                    CacheLookup::Hit(_)
                ) {
                    cached += 1;
                }
            }
        }
        rows.push(json!({"path":path,"partitioned":record.is_some(),"cards":record.as_ref().map(|r|r.units.len()),"cached_cards":cached}));
    }
    let file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(dir.join("priority-coverage.json"))
        .unwrap();
    serde_json::to_writer_pretty(file, &json!({"scope":"current_priority_input_cache_coverage_not_recall_or_whole_corpus_completion","rows":rows,"source_revision":engine.status().unwrap().source_revision})).unwrap();
}

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
