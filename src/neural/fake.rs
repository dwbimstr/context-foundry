//! The deterministic test `EmbeddingProvider` (009 T001): reproducible
//! vectors derived from the exact model-input token ids, with call counters
//! for the FR-002 reuse assertions and fault behaviors for the
//! validation-preservation tests. Compiled only with `test-faults`.
use crate::control::Control;
use crate::error::FoundryError;
use crate::neural::prepare;
use crate::neural::profile::SemanticProfile;
use crate::neural::provider::{
    DIMENSIONS, DOCUMENT_BATCH, DOCUMENT_UNIT_TOKENS, EmbeddingProvider, FunctionDescriptor,
    ProviderError, SERVING_LIMIT_TOKENS, TokenizedInput,
};
use sha2::{Digest, Sha256};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// How the fake misbehaves, if at all.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum FakeBehavior {
    #[default]
    Ok,
    /// One vector comes back one component short.
    ShortVector,
    /// One vector comes back with a NaN.
    Nonfinite,
    /// The provider serves a DIFFERENT document function than the profile.
    WrongDescriptor,
    /// Every document call takes this many milliseconds (budget-expiry tests).
    SlowMs(u64),
    /// The Nth document call (1-based) fails with the provider timeout; the
    /// calls before it succeed.
    TimeoutOnCall(u64),
    /// Every document call blocks until the CONTROL it received expires or is
    /// cancelled, then answers like the supervisor does (`Timeout` for the
    /// deadline, `Cancelled` for the flag). A control that never ends the
    /// wait makes the call fail `Malformed` after 8 s, so a missing deadline
    /// is a failing assertion, not a hang.
    StallUntilControl,
    /// Acquisition blocks until the control it received expires or is
    /// cancelled, then fails like the supervisor's `acquire_until` (`Timeout`
    /// for the deadline, `Cancelled` for the flag); after 8 s without that it
    /// fails `Malformed`, so a control that never reaches acquisition is a
    /// failing assertion, not a hang.
    StallAcquire,
}

/// Live observation of a fake provider: document-call and query-call counts,
/// embedded-input totals and the exact batch sizes.
#[derive(Clone, Debug, Default)]
pub struct FakeHandle {
    pub document_calls: Arc<AtomicU64>,
    pub query_calls: Arc<AtomicU64>,
    pub embedded_inputs: Arc<AtomicU64>,
    pub batch_sizes: Arc<Mutex<Vec<usize>>>,
    /// Every token-id stream handed to the provider, in order.
    pub seen_batches: Arc<Mutex<Vec<Vec<u32>>>>,
}

impl FakeHandle {
    pub fn document_calls(&self) -> u64 {
        self.document_calls.load(Ordering::SeqCst)
    }
    pub fn query_calls(&self) -> u64 {
        self.query_calls.load(Ordering::SeqCst)
    }
    pub fn embedded_inputs(&self) -> u64 {
        self.embedded_inputs.load(Ordering::SeqCst)
    }
    pub fn batch_sizes(&self) -> Vec<usize> {
        self.batch_sizes.lock().expect("batch log").clone()
    }
    /// No batch may exceed the document-batch bound.
    pub fn assert_batches_bounded(&self) {
        assert!(
            self.batch_sizes()
                .iter()
                .all(|size| (1..=DOCUMENT_BATCH).contains(size)),
            "batch sizes {:?} outside 1..={DOCUMENT_BATCH}",
            self.batch_sizes()
        );
    }
}

struct FakeProvider {
    descriptor: FunctionDescriptor,
    behavior: FakeBehavior,
    handle: FakeHandle,
}

/// Deterministic unit-vector-ish embedding of one id stream: a SHA-256
/// stream keyed by the function digest, the ids and the component index.
fn deterministic_vector(descriptor: &FunctionDescriptor, ids: &[u32]) -> Vec<f32> {
    let mut vector = Vec::with_capacity(DIMENSIONS);
    let mut counter = 0u32;
    while vector.len() < DIMENSIONS {
        let mut hasher = Sha256::new();
        hasher.update(descriptor.digest().as_bytes());
        hasher.update((ids.len() as u64).to_le_bytes());
        for id in ids {
            hasher.update(id.to_le_bytes());
        }
        hasher.update(counter.to_le_bytes());
        let digest = hasher.finalize();
        for chunk in digest.chunks_exact(4) {
            let bits = u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
            vector.push(bits as f32 / u32::MAX as f32 - 0.5);
            if vector.len() == DIMENSIONS {
                break;
            }
        }
        counter += 1;
    }
    // L2-normalize: finite, exact dimension, stable across processes.
    let norm: f64 = vector.iter().map(|v| (*v as f64) * (*v as f64)).sum();
    let norm = norm.sqrt().max(f64::EPSILON) as f32;
    vector.iter().map(|v| v / norm).collect()
}

impl EmbeddingProvider for FakeProvider {
    fn descriptor(&self) -> &FunctionDescriptor {
        &self.descriptor
    }

    fn embed_documents(
        &mut self,
        batch: &[TokenizedInput],
        control: &Control,
    ) -> Result<Vec<Vec<f32>>, ProviderError> {
        if batch.is_empty() || batch.len() > DOCUMENT_BATCH {
            return Err(ProviderError::InputTooLarge(format!(
                "fake refuses a batch of {} inputs",
                batch.len()
            )));
        }
        for input in batch {
            if input.ids.is_empty() || input.ids.len() > DOCUMENT_UNIT_TOKENS {
                return Err(ProviderError::InputTooLarge(
                    "fake refuses an out-of-limit input".into(),
                ));
            }
        }
        let call = self.handle.document_calls.fetch_add(1, Ordering::SeqCst) + 1;
        if let FakeBehavior::SlowMs(ms) = self.behavior {
            std::thread::sleep(Duration::from_millis(ms));
        }
        if self.behavior == FakeBehavior::TimeoutOnCall(call) {
            return Err(ProviderError::Timeout);
        }
        if self.behavior == FakeBehavior::StallUntilControl {
            let started = Instant::now();
            loop {
                match control.check() {
                    Err(FoundryError::DeadlineExceeded(_)) => return Err(ProviderError::Timeout),
                    Err(_) => return Err(ProviderError::Cancelled),
                    Ok(()) => {}
                }
                if started.elapsed() > Duration::from_secs(8) {
                    return Err(ProviderError::Malformed(
                        "the run control never expired the call".into(),
                    ));
                }
                std::thread::sleep(Duration::from_millis(20));
            }
        }
        self.handle
            .embedded_inputs
            .fetch_add(batch.len() as u64, Ordering::SeqCst);
        self.handle
            .batch_sizes
            .lock()
            .expect("batch log")
            .push(batch.len());
        self.handle
            .seen_batches
            .lock()
            .expect("batch log")
            .extend(batch.iter().map(|input| input.ids.clone()));
        let mut vectors: Vec<Vec<f32>> = batch
            .iter()
            .map(|input| deterministic_vector(&self.descriptor, &input.ids))
            .collect();
        if self.behavior == FakeBehavior::ShortVector
            && let Some(first) = vectors.first_mut()
        {
            first.pop();
        }
        if self.behavior == FakeBehavior::Nonfinite
            && let Some(first) = vectors.first_mut()
            && let Some(component) = first.first_mut()
        {
            *component = f32::NAN;
        }
        Ok(vectors)
    }

    fn embed_query(
        &mut self,
        input: &TokenizedInput,
        _deadline: Instant,
    ) -> Result<Vec<f32>, ProviderError> {
        if input.ids.is_empty() || input.ids.len() > SERVING_LIMIT_TOKENS {
            return Err(ProviderError::InputTooLarge(
                "fake refuses an out-of-limit query".into(),
            ));
        }
        self.handle.query_calls.fetch_add(1, Ordering::SeqCst);
        Ok(deterministic_vector(&self.descriptor, &input.ids))
    }
}

/// A factory of [`prepare::Acquire`] seams sharing one set of counters, so
/// a test can run preparation several times (restarts) and still observe
/// exact document-call counts.
pub struct FakeFactory {
    descriptor: FunctionDescriptor,
    behavior: FakeBehavior,
    handle: FakeHandle,
}

impl FakeFactory {
    pub fn new(descriptor: FunctionDescriptor, behavior: FakeBehavior) -> Self {
        Self {
            descriptor,
            behavior,
            handle: FakeHandle::default(),
        }
    }

    /// One acquisition seam; all seams from this factory share the counters.
    pub fn acquire(&self) -> prepare::Acquire {
        let served = if self.behavior == FakeBehavior::WrongDescriptor {
            let mut other = self.descriptor.clone();
            // A different quantization names a different document function.
            other.quantization = format!("{} +fake", other.quantization);
            other
        } else {
            self.descriptor.clone()
        };
        let behavior = self.behavior;
        let wiring = self.handle.clone();
        Box::new(
            move |_profile: &SemanticProfile,
                  _development: bool,
                  control: &Control|
                  -> Result<Box<dyn EmbeddingProvider>, ProviderError> {
                if behavior == FakeBehavior::StallAcquire {
                    let started = Instant::now();
                    loop {
                        match control.check() {
                            Err(FoundryError::DeadlineExceeded(_)) => {
                                return Err(ProviderError::Timeout);
                            }
                            Err(_) => return Err(ProviderError::Cancelled),
                            Ok(()) => {}
                        }
                        if started.elapsed() > Duration::from_secs(8) {
                            return Err(ProviderError::Malformed(
                                "the run control never reached acquisition".into(),
                            ));
                        }
                        std::thread::sleep(Duration::from_millis(20));
                    }
                }
                Ok(Box::new(FakeProvider {
                    descriptor: served.clone(),
                    behavior,
                    handle: wiring.clone(),
                }))
            },
        )
    }

    pub fn handle(&self) -> &FakeHandle {
        &self.handle
    }
}
