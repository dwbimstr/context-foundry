//! Opt-in native calibration through the ordinary overview preparation path.
//! No labels, substituted vectors, cross-backend cache imports or scale promise.
#![cfg(all(feature = "semantic", feature = "test-faults", target_os = "macos"))]
use context_foundry::neural::{
    partition::{CardRecipe, Representation, hierarchy},
    prepare::{self, PrepareOptions},
    profile::SemanticProfile,
    provider::{EmbeddingProvider, FunctionDescriptor, ProviderError, TokenizedInput},
    supervisor::WorkerProvider,
    tokenize::DocumentTokenizer,
};
use context_foundry::{Control, Engine, digest, ingest, syntax::Lang};
use serde_json::{Value, json};
use std::{
    cell::RefCell,
    collections::{BTreeMap, BTreeSet},
    fs::{File, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    rc::Rc,
    time::{Duration, Instant},
};

fn write_new(path: &Path, value: &Value) {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .unwrap();
    serde_json::to_writer_pretty(&mut file, value).unwrap();
    writeln!(file).unwrap();
}

fn ids_sha256(ids: &[u32]) -> String {
    digest(
        &ids.iter()
            .flat_map(|id| id.to_le_bytes())
            .collect::<Vec<_>>(),
    )
}

#[derive(Default)]
struct Measurements {
    acquisitions: u64,
    acquisition_ms: u128,
    calls: u64,
    tokens: u64,
    provider_ns: u128,
    footprint_samples: u64,
    observed_peak_footprint_bytes: u64,
    input_hashes: BTreeSet<String>,
    stopped: bool,
}

struct RecordingProvider {
    inner: WorkerProvider,
    expected: Rc<BTreeMap<String, String>>,
    measured: Rc<RefCell<Measurements>>,
    events: Rc<RefCell<File>>,
}

impl RecordingProvider {
    fn footprint(&self) -> u64 {
        // Sample only the live owned worker. These are call-boundary observations,
        // not a continuous peak, acquisition peak, GPU allocation or global headroom.
        let pid = self
            .inner
            .worker_pid()
            .expect("worker must be live at call boundary");
        let mut info = unsafe { std::mem::zeroed::<libc::rusage_info_v0>() };
        let rc = unsafe {
            libc::proc_pid_rusage(
                pid as libc::pid_t,
                libc::RUSAGE_INFO_V0,
                std::ptr::addr_of_mut!(info).cast(),
            )
        };
        assert_eq!(
            rc,
            0,
            "native footprint unavailable: {}",
            std::io::Error::last_os_error()
        );
        assert!(info.ri_phys_footprint > 0);
        let mut measured = self.measured.borrow_mut();
        measured.footprint_samples += 1;
        measured.observed_peak_footprint_bytes = measured
            .observed_peak_footprint_bytes
            .max(info.ri_phys_footprint);
        info.ri_phys_footprint
    }
}

impl EmbeddingProvider for RecordingProvider {
    fn descriptor(&self) -> &FunctionDescriptor {
        self.inner.descriptor()
    }

    fn embed_documents(
        &mut self,
        batch: &[TokenizedInput],
        control: &Control,
    ) -> Result<Vec<Vec<f32>>, ProviderError> {
        assert_eq!(batch.len(), 1);
        let ids = &batch[0].ids;
        assert!(!ids.is_empty() && ids.len() <= 512);
        let hash = ids_sha256(ids);
        let path = self
            .expected
            .get(&hash)
            .expect("actual input differs from normal fitting");
        let before = self.footprint();
        let start = Instant::now();
        let result = self.inner.embed_documents(batch, control);
        let elapsed_ns = start.elapsed().as_nanos();
        let after = result.as_ref().ok().map(|_| self.footprint());
        let mut event = json!({
            "path": path, "input_ids": ids, "input_ids_sha256": hash,
            "input_sha256_encoding": "concatenated little-endian u32 token IDs after ordinary overview fitting",
            "input_tokens": ids.len(), "provider_ns": elapsed_ns,
            "footprint_before_bytes": before, "footprint_after_bytes": after,
            "completed": result.is_ok(),
        });
        if let Err(error) = &result {
            event["error"] = json!(error.to_string());
        }
        let mut events = self.events.borrow_mut();
        serde_json::to_writer(&mut *events, &event).unwrap();
        writeln!(events).unwrap();
        events.flush().unwrap();
        if let Ok(vectors) = &result {
            assert_eq!(vectors.len(), 1);
            assert_eq!(vectors[0].len(), 768);
            assert!(vectors[0].iter().all(|x| x.is_finite()));
            let norm = vectors[0]
                .iter()
                .map(|x| f64::from(*x).powi(2))
                .sum::<f64>()
                .sqrt();
            assert!((norm - 1.0).abs() <= 0.0001);
            let mut measured = self.measured.borrow_mut();
            assert!(
                measured.input_hashes.insert(hash),
                "fresh sample repeated an input"
            );
            measured.calls += 1;
            measured.tokens += ids.len() as u64;
            measured.provider_ns += elapsed_ns;
        }
        result
    }

    fn embed_query(&mut self, _: &TokenizedInput, _: Instant) -> Result<Vec<f32>, ProviderError> {
        panic!("calibration must only prepare document inputs")
    }
}

impl Drop for RecordingProvider {
    fn drop(&mut self) {
        self.inner.shutdown();
        self.measured.borrow_mut().stopped = self.inner.stopped();
    }
}

#[test]
#[ignore = "real Metal calibration; explicit public sample, profile and fresh paths outside Git"]
fn real_metal_overview_scale_calibration() {
    let harness_started = Instant::now();
    let path = |key| PathBuf::from(std::env::var_os(key).expect(key));
    let profile_path = path("FOUNDRY_SCALE_PROFILE");
    let corpus = path("FOUNDRY_SCALE_CORPUS").canonicalize().unwrap();
    let out = path("FOUNDRY_SCALE_OUT").canonicalize().unwrap();
    let store = path("FOUNDRY_SCALE_STORE");
    assert!(store.is_absolute() && !store.exists());
    for directory in [
        &corpus,
        &out,
        &store.parent().unwrap().canonicalize().unwrap(),
    ] {
        assert!(!directory.starts_with(env!("CARGO_MANIFEST_DIR")));
    }
    let profile = SemanticProfile::load(&profile_path).unwrap();
    assert_eq!(profile.descriptor.adapter_revision, 4);
    assert_eq!(profile.descriptor.dimensions, 768);
    assert_eq!(profile.representation, Representation::Overview);
    assert_eq!((profile.card_tokens, profile.batch), (512, 1));
    assert_eq!(profile.descriptor.input_limits().tokens, 8192);
    let manifest_path = out.join("sample.json");
    assert_eq!(
        digest(&std::fs::read(&manifest_path).unwrap()),
        "a59c9a1e08ca36dd2baa1449420b4b803671db6689d18de8835510d6fbf221ad",
        "fixed public sample manifest mismatch"
    );
    let manifest: Value = serde_json::from_slice(&std::fs::read(&manifest_path).unwrap()).unwrap();
    let files = manifest["files"].as_array().unwrap();
    assert_eq!(files.len(), 128);
    let tokenizer = DocumentTokenizer::load(&profile).unwrap();
    let function_digest = profile.descriptor.digest();
    let recipe = CardRecipe {
        template: &profile.descriptor.document_template,
        function_digest: &function_digest,
        card_tokens: 512,
    };
    let mut expected = BTreeMap::new();
    let mut fitted = Vec::new();
    for row in files {
        let relative = row["path"].as_str().unwrap();
        let source = std::fs::read(corpus.join(relative)).unwrap();
        assert_eq!(digest(&source), row["sha256"].as_str().unwrap());
        assert_eq!(source.len() as u64, row["bytes"].as_u64().unwrap());
        let source = std::str::from_utf8(&source).unwrap();
        let plan =
            hierarchy::file_plan(source, Lang::from_path(relative), &recipe, &tokenizer).unwrap();
        assert_eq!(plan.len(), 1);
        let card = hierarchy::card(source, &plan[0], &recipe, &tokenizer).unwrap();
        let hash = ids_sha256(&card.ids);
        assert!(
            expected.insert(hash.clone(), relative.to_owned()).is_none(),
            "sample must contain 128 distinct fitted inputs"
        );
        fitted.push(json!({
            "path": relative, "source_sha256": row["sha256"], "source_bytes": source.len(),
            "input_key": card.input_key, "input_ids": card.ids, "input_ids_sha256": hash,
            "input_tokens": card.ids.len(), "kind": card.kind, "start": card.start, "end": card.end,
        }));
    }
    write_new(
        &out.join("fitted-inputs.json"),
        &json!({
            "function_digest": function_digest, "recipe_id": profile.recipe_id(),
            "input_sha256_encoding": "concatenated little-endian u32 token IDs after ordinary overview fitting",
            "inputs": fitted,
        }),
    );
    let mut engine = Engine::initialize(&store, &corpus).unwrap();
    let indexed = ingest::index(&mut engine, &corpus, &Control::unbounded()).unwrap();
    write_new(
        &out.join("index.json"),
        &serde_json::to_value(&indexed).unwrap(),
    );
    assert!(indexed.scan_complete && !indexed.partial && indexed.failures == 0);
    assert_eq!(indexed.changed, 128);
    assert_eq!(indexed.pending_sources, 0);
    assert_eq!(engine.semantic_cache_totals().unwrap(), (0, 0));
    drop(engine);
    let measured = Rc::new(RefCell::new(Measurements::default()));
    let events = Rc::new(RefCell::new(
        OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(out.join("calls.jsonl"))
            .unwrap(),
    ));
    let captured = Rc::clone(&measured);
    let expected = Rc::new(expected);
    let started = Instant::now();
    let control = Control::with_deadline(started + Duration::from_secs(1200));
    let acquire: prepare::Acquire = Box::new(move |profile, _, development, control| {
        assert!(development);
        captured.borrow_mut().acquisitions += 1;
        let acquired = Instant::now();
        let inner = WorkerProvider::launch_until(profile, vec![], control)?;
        captured.borrow_mut().acquisition_ms += acquired.elapsed().as_millis();
        println!(
            "NATIVE-SCALE-ACQUIRE {}",
            json!({"elapsed_ms": acquired.elapsed().as_millis(), "pid": inner.worker_pid()})
        );
        Ok(Box::new(RecordingProvider {
            inner,
            expected: Rc::clone(&expected),
            measured: Rc::clone(&captured),
            events: Rc::clone(&events),
        }))
    });
    let report = prepare::run(
        &store,
        &PrepareOptions {
            profile_path: &profile_path,
            budget_seconds: 1200,
            development: true,
            cache_cap_bytes: 512 * 1024 * 1024,
            started,
            control: &control,
            priority_paths: &[],
        },
        acquire,
    )
    .unwrap();
    write_new(
        &out.join("prepare.json"),
        &serde_json::to_value(&report).unwrap(),
    );
    assert!(!report.partial && report.index_published && report.index_reason.is_none());
    assert_eq!(report.document_calls, 128);
    assert_eq!(report.embedded_units, 128);
    assert_eq!(report.cache_entries, 128);
    assert_eq!(report.cache_cap_bytes, 512 * 1024 * 1024);
    let engine = Engine::open_existing(&store).unwrap();
    let audit = engine.semantic_status(&Control::unbounded()).unwrap();
    write_new(
        &out.join("audit.json"),
        &serde_json::to_value(&audit).unwrap(),
    );
    assert_eq!(audit.sources, 128);
    assert_eq!(audit.partition_coverage, "complete");
    assert_eq!((audit.unpartitioned_sources, audit.missing_units), (0, 0));
    assert_eq!(
        (audit.cached_current_units, audit.searchable_current_units),
        (128, 128)
    );
    for row in &fitted {
        let partition = engine
            .semantic_partition(row["path"].as_str().unwrap())
            .unwrap()
            .unwrap();
        assert_eq!(
            partition.source_hash,
            row["source_sha256"].as_str().unwrap()
        );
        assert_eq!(partition.function_digest, function_digest);
        assert_eq!(partition.recipe_id, profile.recipe_id());
        assert_eq!(partition.units.len(), 1);
        assert_eq!(
            partition.units[0].input_key,
            row["input_key"].as_str().unwrap()
        );
    }
    drop(engine);
    let replay_started = Instant::now();
    let replay_control = Control::with_deadline(replay_started + Duration::from_secs(120));
    let replay = prepare::run(
        &store,
        &PrepareOptions {
            profile_path: &profile_path,
            budget_seconds: 120,
            development: true,
            cache_cap_bytes: 512 * 1024 * 1024,
            started: replay_started,
            control: &replay_control,
            priority_paths: &[],
        },
        Box::new(|_, _, _, _| panic!("unchanged replay acquired the model")),
    )
    .unwrap();
    write_new(
        &out.join("unchanged.json"),
        &serde_json::to_value(&replay).unwrap(),
    );
    assert!(!replay.partial && replay.index_published && replay.index_reason.is_none());
    assert_eq!(
        (
            replay.document_calls,
            replay.input_tokens,
            replay.embedded_units
        ),
        (0, 0, 0)
    );
    let measured = measured.borrow();
    assert_eq!((measured.acquisitions, measured.calls), (1, 128));
    assert_eq!(measured.tokens, report.input_tokens);
    assert_eq!(measured.footprint_samples, 256);
    assert!(measured.stopped);
    assert!(measured.observed_peak_footprint_bytes <= profile.memory_ceiling_bytes);
    write_new(
        &out.join("receipt.json"),
        &json!({
            "v": 1, "status": "passed", "os": std::env::consts::OS, "arch": std::env::consts::ARCH,
            "worker_sha256": profile.worker.executable_sha256, "descriptor": profile.descriptor,
            "function_digest": function_digest, "recipe_id": profile.recipe_id(),
            "sample_manifest_sha256": digest(&std::fs::read(manifest_path).unwrap()),
            "sources": 128, "distinct_inputs": 128, "document_calls": measured.calls,
            "sample_scope": manifest["scope"],
            "cache_cap_bytes": report.cache_cap_bytes,
            "cache_cap_scope": "maximum permitted durable vector cache bytes; not preallocation",
            "input_tokens": measured.tokens, "provider_ns": measured.provider_ns,
            "acquisition_ms": measured.acquisition_ms,
            "acquisition_scope": "concrete test launch repeats artifact verification to expose owned PID; initial prepare verification is outside this interval",
            "provider_time_scope": "sum of successful supervised embed_documents call wall time; excludes acquisition, fitting, publication and measurement overhead",
            "prepare_elapsed_ms": report.elapsed_ms, "bootstrap": report.bootstrap,
            "total_harness_elapsed_ms": harness_started.elapsed().as_millis(),
            "memory": {"observed_peak_footprint_bytes": measured.observed_peak_footprint_bytes,
                       "samples": measured.footprint_samples, "source": "proc_pid_rusage RUSAGE_INFO_V0 ri_phys_footprint",
                       "scope": "before and after each successful model call; excludes acquisition and between-sample peaks",
                       "profile_ceiling_bytes": profile.memory_ceiling_bytes},
            "worker_stopped_and_reaped": measured.stopped, "unchanged_document_calls": replay.document_calls,
            "not_accepted": ["full 67663-source completion or throughput", "population-weighted performance estimate",
                             "true peak footprint", "GPU allocation or global GPU headroom", "retrieval or task quality",
                             "simultaneous embedding and generation residency"],
        }),
    );
}
