//! 009 embedding boundary shared by core preparation (T001) and the worker
//! supervisor: the document-function descriptor, cache input keys, tokenized
//! inputs and the provider trait. Only these definitions cross between the
//! two halves; nothing here loads a model, a tokenizer or Python.
use crate::control::Control;
use serde::{Deserialize, Serialize};
use std::time::Instant;

/// Model output dimension of the selected profile (D001).
pub const DIMENSIONS: usize = 2048;
/// Document embedding-unit limit, rendered prefix and special tokens included.
pub const DOCUMENT_UNIT_TOKENS: usize = 1024;
/// Serving limit for any single model input, prefix included (D001).
pub const SERVING_LIMIT_TOKENS: usize = 2048;
/// Inputs per document call; queries embed one input.
pub const DOCUMENT_BATCH: usize = 8;
/// Applied by Foundry exactly once; the worker never adds a prefix.
pub const DOCUMENT_PREFIX: &str = "passage: ";
/// Query recipe; part of the retrieval profile, never of the document function.
pub const QUERY_PREFIX: &str = "query: ";
/// Version of the descriptor layout below. Changing any field's meaning or the
/// digest encoding bumps it, which re-keys every cached vector.
pub const DESCRIPTOR_VERSION: u32 = 1;

/// One artifact file the publisher loader reads, verified before load.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArtifactFile {
    /// Path relative to the model directory (`/`-separated, no `..`).
    pub name: String,
    /// Lowercase hex SHA-256 of the file bytes.
    pub sha256: String,
}

/// The numerical runtime closure the worker executes with. The supervisor
/// compares the worker's `hello` against these expected values; the worker
/// never selects them.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeClosure {
    pub python: String,
    pub mlx: String,
    pub mlx_metal: String,
    pub mlx_lm: String,
    pub transformers: String,
    pub numpy: String,
    /// SHA-256 of the runtime's frozen requirements listing.
    pub requirements_sha256: String,
}

/// Everything that changes document vectors (spec 009 T001 "Document-function
/// descriptor"). Worker location, signing identity, labels, the query recipe,
/// ranking/packing, partition grammar/limits and ANN scalar settings are
/// deliberately absent: changing them re-embeds nothing.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FunctionDescriptor {
    pub v: u32,
    /// Upstream model and the local artifact identity, e.g.
    /// `nvidia/Nemotron-3-Embed-1B-BF16 via mlx-community/Nemotron-3-Embed-1B-BF16-4bit@d0408b94…`.
    pub model: String,
    /// Sorted by `name`; weights, configs, tokenizer files and loader source.
    pub artifact_files: Vec<ArtifactFile>,
    /// e.g. `affine bits=4 group_size=64`.
    pub quantization: String,
    /// Rust tokenizer implementation and version, e.g. `tokenizers 0.23.2 onig`.
    pub tokenizer: String,
    pub add_special_tokens: bool,
    /// `right`.
    pub padding_side: String,
    pub pad_id: u32,
    /// Revision of the first-party adapter that builds input arrays and
    /// materializes output. Bump on any change to that code path.
    pub adapter_revision: u32,
    /// e.g. `int32`.
    pub input_dtype: String,
    /// e.g. `int32`.
    pub mask_dtype: String,
    /// `publisher mean + l2` (pooling and normalization inside the model).
    pub pooling: String,
    pub dimensions: u32,
    /// `f32`.
    pub output: String,
    pub runtime: RuntimeClosure,
    /// Must equal [`DOCUMENT_PREFIX`].
    pub document_prefix: String,
}

impl FunctionDescriptor {
    /// Structural checks that need no files: version, prefix, dimensions,
    /// sorted unique artifact names and hex digests.
    pub fn validate(&self) -> Result<(), String> {
        if self.v != DESCRIPTOR_VERSION {
            return Err(format!(
                "descriptor version {} is not {DESCRIPTOR_VERSION}",
                self.v
            ));
        }
        if self.document_prefix != DOCUMENT_PREFIX {
            return Err("document prefix must be `passage: `".into());
        }
        if self.dimensions as usize != DIMENSIONS {
            return Err(format!("dimensions must be {DIMENSIONS}"));
        }
        if self.artifact_files.is_empty() {
            return Err("descriptor names no artifact files".into());
        }
        for pair in self.artifact_files.windows(2) {
            if pair[0].name >= pair[1].name {
                return Err("artifact files must be sorted by name without duplicates".into());
            }
        }
        let hex64 =
            |s: &str| s.len() == 64 && s.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'));
        for file in &self.artifact_files {
            if file.name.is_empty()
                || file.name.starts_with('/')
                || file
                    .name
                    .split('/')
                    .any(|c| c.is_empty() || c == "." || c == "..")
            {
                return Err(format!(
                    "artifact file name {:?} is not a relative path",
                    file.name
                ));
            }
            if !hex64(&file.sha256) {
                return Err(format!("artifact file {:?} has no SHA-256", file.name));
            }
        }
        if !hex64(&self.runtime.requirements_sha256) {
            return Err("runtime requirements digest must be a SHA-256".into());
        }
        Ok(())
    }

    /// Lowercase hex SHA-256 of the compact JSON of this descriptor. Field
    /// order is the struct order above, so the encoding is deterministic.
    pub fn digest(&self) -> String {
        let json = serde_json::to_vec(self).expect("descriptor serializes");
        crate::digest(&json)
    }
}

/// The document-cache key: SHA-256 over an unambiguous length-prefixed
/// encoding of the function digest and the exact rendered input bytes.
pub fn input_key(function_digest: &str, rendered: &str) -> String {
    let mut bytes = Vec::with_capacity(16 + function_digest.len() + rendered.len());
    for part in [function_digest.as_bytes(), rendered.as_bytes()] {
        bytes.extend_from_slice(&(part.len() as u64).to_le_bytes());
        bytes.extend_from_slice(part);
    }
    crate::digest(&bytes)
}

/// The rendered document input for a unit's exact source bytes.
pub fn render_document(unit_text: &str) -> String {
    let mut rendered = String::with_capacity(DOCUMENT_PREFIX.len() + unit_text.len());
    rendered.push_str(DOCUMENT_PREFIX);
    rendered.push_str(unit_text);
    rendered
}

/// Token IDs of one rendered input, special tokens included, never truncated.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TokenizedInput {
    pub ids: Vec<u32>,
}

/// Why a provider call produced no vectors. Codes are contract names.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProviderError {
    /// The worker's one admission slot is held; nothing was queued.
    Busy,
    /// The caller's deadline or budget ended first; the slot may still be held.
    Timeout,
    /// The worker process ended or broke the protocol.
    WorkerExited(String),
    /// A reply failed validation (shape, length, finiteness, identity).
    Malformed(String),
    /// The supervised memory ceiling was exceeded; the worker was stopped.
    ResourceLimit(String),
    /// No accepted isolation profile admits model execution here.
    IsolationUnavailable(String),
    /// The profile, worker, runtime or artifact failed verification.
    ProfileInvalid(String),
    /// An input exceeded a token or batch limit before any model call.
    InputTooLarge(String),
    /// Cooperative cancellation.
    Cancelled,
}

impl ProviderError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::Busy => "provider_busy",
            Self::Timeout => "provider_timeout",
            Self::WorkerExited(_) => "provider_exited",
            Self::Malformed(_) => "provider_malformed",
            Self::ResourceLimit(_) => "resource_limit",
            Self::IsolationUnavailable(_) => "isolation_unavailable",
            Self::ProfileInvalid(_) => "profile_invalid",
            Self::InputTooLarge(_) => "input_too_large",
            Self::Cancelled => "cancelled",
        }
    }
}

impl std::fmt::Display for ProviderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Busy | Self::Timeout | Self::Cancelled => f.write_str(self.code()),
            Self::WorkerExited(m)
            | Self::Malformed(m)
            | Self::ResourceLimit(m)
            | Self::IsolationUnavailable(m)
            | Self::ProfileInvalid(m)
            | Self::InputTooLarge(m) => write!(f, "{}: {m}", self.code()),
        }
    }
}

/// Validate one returned vector: exact dimension and finite values.
pub fn validate_vector(vector: &[f32]) -> Result<(), ProviderError> {
    if vector.len() != DIMENSIONS {
        return Err(ProviderError::Malformed(format!(
            "vector has {} values, expected {DIMENSIONS}",
            vector.len()
        )));
    }
    if vector.iter().any(|v| !v.is_finite()) {
        return Err(ProviderError::Malformed(
            "vector has a nonfinite value".into(),
        ));
    }
    Ok(())
}

/// A thread-safe probe of a call the provider already returned from that
/// still runs in the model (see [`EmbeddingProvider::late_call`]).
pub type LateCall = std::sync::Arc<dyn Fn() -> bool + Send + Sync>;

/// One embedding function behind one admission slot. Implementations: the
/// supervised worker (`WorkerProvider`) and a deterministic test provider.
pub trait EmbeddingProvider {
    /// The verified descriptor this provider computes.
    fn descriptor(&self) -> &FunctionDescriptor;
    /// Embed at most [`DOCUMENT_BATCH`] inputs of at most
    /// [`DOCUMENT_UNIT_TOKENS`] IDs each; one vector per input, in order.
    fn embed_documents(
        &mut self,
        batch: &[TokenizedInput],
        control: &Control,
    ) -> Result<Vec<Vec<f32>>, ProviderError>;
    /// Embed one query of at most [`SERVING_LIMIT_TOKENS`] IDs.
    fn embed_query(
        &mut self,
        input: &TokenizedInput,
        deadline: Instant,
    ) -> Result<Vec<f32>, ProviderError>;
    /// 009 T003: a probe that stays true while a call this provider already
    /// returned from still runs in the model: a query abandoned at its
    /// deadline whose late reply has not arrived. The resident runtime counts
    /// it as an occupied slot. Providers whose calls end when they return
    /// have none.
    fn late_call(&self) -> Option<LateCall> {
        None
    }
}

/// Pre-call limits shared by every provider: batch size, per-input length
/// and nonempty inputs. Refusal happens before any model work.
pub fn check_document_batch(batch: &[TokenizedInput]) -> Result<(), ProviderError> {
    if batch.is_empty() || batch.len() > DOCUMENT_BATCH {
        return Err(ProviderError::InputTooLarge(format!(
            "document batch of {} inputs; 1..={DOCUMENT_BATCH} allowed",
            batch.len()
        )));
    }
    for (i, input) in batch.iter().enumerate() {
        if input.ids.is_empty() || input.ids.len() > DOCUMENT_UNIT_TOKENS {
            return Err(ProviderError::InputTooLarge(format!(
                "document input {i} has {} tokens; 1..={DOCUMENT_UNIT_TOKENS} allowed",
                input.ids.len()
            )));
        }
    }
    Ok(())
}

/// Query counterpart of [`check_document_batch`].
pub fn check_query(input: &TokenizedInput) -> Result<(), ProviderError> {
    if input.ids.is_empty() || input.ids.len() > SERVING_LIMIT_TOKENS {
        return Err(ProviderError::InputTooLarge(format!(
            "query has {} tokens; 1..={SERVING_LIMIT_TOKENS} allowed",
            input.ids.len()
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_cache_key_separates_function_and_input_unambiguously() {
        let f = "a".repeat(64);
        // Moving bytes across the boundary changes the key.
        assert_ne!(
            input_key(&f, "passage: x"),
            input_key(&format!("{f}p"), "assage: x")
        );
        assert_ne!(
            input_key(&f, "passage: x"),
            input_key(&"b".repeat(64), "passage: x")
        );
        assert_eq!(input_key(&f, "passage: x"), input_key(&f, "passage: x"));
        assert_eq!(render_document("fn a() {}"), "passage: fn a() {}");
    }
}
