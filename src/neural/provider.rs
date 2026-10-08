//! 009 embedding boundary shared by core preparation (T001) and the worker
//! supervisor: the document-function descriptor, cache input keys, tokenized
//! inputs and the provider trait. Only these definitions cross between the
//! two halves; nothing here loads a model or a tokenizer.
//!
//! 009 T004: the profile owns the output dimension, the document and query
//! templates, the card limit and the batch size; the constants below are
//! only the caps every profile and the worker protocol stay within.
use crate::control::Control;
use serde::{Deserialize, Serialize};
use std::time::Instant;

/// Serving limit for any single model input, template and special tokens
/// included (D001), and the token total of one call: the worker embeds a
/// call's sequences in one llama.cpp context of this many tokens.
pub const SERVING_LIMIT_TOKENS: usize = 2048;
/// The largest number of sequences a profile may embed per document call.
pub const MAX_DOCUMENT_BATCH: usize = 32;
/// Output dimensions a profile may pin: the model's full width or one of its
/// supported Matryoshka truncations (renormalized by the worker).
pub const SUPPORTED_DIMENSIONS: [usize; 6] = [128, 256, 512, 768, 1024, 2048];
/// The widest vector any profile produces (the protocol's payload cap).
pub const MAX_DIMENSIONS: usize = 2048;
/// The one slot of a document or query template the text fills.
pub const TEXT_SLOT: &str = "{text}";
/// Pooling modes the llama.cpp worker can pin (`llama_pooling_type`).
pub const POOLINGS: [&str; 3] = ["mean", "cls", "last"];
/// Version of the descriptor layout below. Changing any field's meaning or the
/// digest encoding bumps it, which re-keys every cached vector. Version 1 was
/// the MLX worker's layout; it is refused with `profile_unsupported`.
pub const DESCRIPTOR_VERSION: u32 = 2;

/// One artifact file the worker or the core tokenizer reads, verified
/// before load.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArtifactFile {
    /// Path relative to the model directory (`/`-separated, no `..`).
    pub name: String,
    /// Lowercase hex SHA-256 of the file bytes.
    pub sha256: String,
}

/// Everything that changes document vectors (spec 009 T001 "Document-function
/// descriptor", T004 descriptor v2). Worker location, signing identity,
/// labels, the query template, the card limit and batch size, ranking and
/// packing and ANN scalar settings are deliberately absent: changing them
/// re-embeds nothing (the card limit re-partitions, and only cards whose
/// rendered text changed get new keys).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FunctionDescriptor {
    pub v: u32,
    /// Upstream model and the local artifact identity, e.g.
    /// `google/embeddinggemma-2@914f7f89 via ggml-org/embeddinggemma-2-GGUF@bfcd2987 BF16`.
    pub model: String,
    /// Sorted by `name`: the GGUF and the core's `tokenizer.json` at least.
    pub artifact_files: Vec<ArtifactFile>,
    /// The artifact file the worker loads (one of `artifact_files`).
    pub gguf: String,
    /// The pinned llama.cpp commit the worker is built from (40 hex).
    pub llama_cpp: String,
    /// Rust tokenizer implementation and version, e.g. `tokenizers 0.23.2 onig`.
    pub tokenizer: String,
    pub add_special_tokens: bool,
    /// One of [`POOLINGS`]; the worker pins it and refuses another.
    pub pooling: String,
    /// The pooled vector's leading `dimensions` values, L2-normalized: one
    /// of [`SUPPORTED_DIMENSIONS`].
    pub dimensions: u32,
    /// `f32`.
    pub output: String,
    /// Revision of the first-party worker code that builds the llama.cpp
    /// batch and materializes output. Bump on any change to that code path.
    pub adapter_revision: u32,
    /// The document template: [`TEXT_SLOT`] exactly once, filled with a card.
    pub document_template: String,
}

/// The template's one [`TEXT_SLOT`], or why it has none.
pub fn check_template(what: &str, template: &str) -> Result<(), String> {
    match template.matches(TEXT_SLOT).count() {
        1 => Ok(()),
        n => Err(format!(
            "{what} must contain {TEXT_SLOT} exactly once (found {n})"
        )),
    }
}

/// `template` with its one [`TEXT_SLOT`] filled by `text`.
pub fn render(template: &str, text: &str) -> String {
    match template.split_once(TEXT_SLOT) {
        Some((before, after)) => {
            let mut rendered = String::with_capacity(before.len() + text.len() + after.len());
            rendered.push_str(before);
            rendered.push_str(text);
            rendered.push_str(after);
            rendered
        }
        None => text.to_owned(),
    }
}

fn hex(s: &str, len: usize) -> bool {
    s.len() == len && s.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

impl FunctionDescriptor {
    /// Structural checks that need no files: version, template, dimensions,
    /// pooling, sorted unique artifact names, hex digests and the pinned
    /// GGUF and llama.cpp commit.
    pub fn validate(&self) -> Result<(), String> {
        if self.v != DESCRIPTOR_VERSION {
            return Err(format!(
                "descriptor version {} is not {DESCRIPTOR_VERSION}",
                self.v
            ));
        }
        check_template("document_template", &self.document_template)?;
        if !SUPPORTED_DIMENSIONS.contains(&(self.dimensions as usize)) {
            return Err(format!(
                "dimensions {} is not one of {SUPPORTED_DIMENSIONS:?}",
                self.dimensions
            ));
        }
        if !POOLINGS.contains(&self.pooling.as_str()) {
            return Err(format!(
                "pooling {:?} is not one of {POOLINGS:?}",
                self.pooling
            ));
        }
        if self.output != "f32" {
            return Err("output must be `f32`".into());
        }
        if !hex(&self.llama_cpp, 40) {
            return Err("llama_cpp must be a 40-hex commit".into());
        }
        if self.artifact_files.is_empty() {
            return Err("descriptor names no artifact files".into());
        }
        for pair in self.artifact_files.windows(2) {
            if pair[0].name >= pair[1].name {
                return Err("artifact files must be sorted by name without duplicates".into());
            }
        }
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
            if !hex(&file.sha256, 64) {
                return Err(format!("artifact file {:?} has no SHA-256", file.name));
            }
        }
        if !self
            .artifact_files
            .iter()
            .any(|file| file.name == self.gguf)
        {
            return Err(format!(
                "the GGUF {:?} is not among the artifact files",
                self.gguf
            ));
        }
        Ok(())
    }

    /// The output dimension as a length.
    pub fn dims(&self) -> usize {
        self.dimensions as usize
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
    /// The profile, worker or artifact failed verification.
    ProfileInvalid(String),
    /// 009 T004: a profile this build no longer serves (descriptor v1, the
    /// MLX worker). Semantic retrieval stays off; its retained cache rows and
    /// generations are kept until an explicit `semantic purge`.
    ProfileUnsupported(String),
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
            Self::ProfileUnsupported(_) => "profile_unsupported",
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
            | Self::ProfileUnsupported(m)
            | Self::InputTooLarge(m) => write!(f, "{}: {m}", self.code()),
        }
    }
}

/// Validate one returned vector: exactly `dims` finite values.
pub fn validate_vector(vector: &[f32], dims: usize) -> Result<(), ProviderError> {
    if vector.len() != dims {
        return Err(ProviderError::Malformed(format!(
            "vector has {} values, expected {dims}",
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
    /// Embed at most the profile's batch of inputs of at most its card limit
    /// each ([`DocumentLimits`]); one vector per input, in order.
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

/// The size limits of one document call: at most `inputs` sequences of at
/// most `tokens` IDs each, and never more than [`SERVING_LIMIT_TOKENS`] IDs
/// in all (one llama.cpp context per call).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DocumentLimits {
    pub inputs: usize,
    pub tokens: usize,
}

impl DocumentLimits {
    /// The protocol's caps: what the worker itself accepts from any profile.
    pub const PROTOCOL: Self = Self {
        inputs: MAX_DOCUMENT_BATCH,
        tokens: SERVING_LIMIT_TOKENS,
    };
}

/// Pre-call limits shared by every provider: batch size, per-input length,
/// the call's token total and nonempty inputs. Refusal happens before any
/// model work.
pub fn check_document_batch(
    batch: &[TokenizedInput],
    limits: DocumentLimits,
) -> Result<(), ProviderError> {
    if batch.is_empty() || batch.len() > limits.inputs {
        return Err(ProviderError::InputTooLarge(format!(
            "document batch of {} inputs; 1..={} allowed",
            batch.len(),
            limits.inputs
        )));
    }
    let mut total = 0usize;
    for (i, input) in batch.iter().enumerate() {
        if input.ids.is_empty() || input.ids.len() > limits.tokens {
            return Err(ProviderError::InputTooLarge(format!(
                "document input {i} has {} tokens; 1..={} allowed",
                input.ids.len(),
                limits.tokens
            )));
        }
        total += input.ids.len();
    }
    if total > SERVING_LIMIT_TOKENS {
        return Err(ProviderError::InputTooLarge(format!(
            "document batch has {total} tokens; at most {SERVING_LIMIT_TOKENS} per call"
        )));
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
    }

    #[test]
    fn a_template_has_exactly_one_text_slot() {
        assert_eq!(
            render("title: none | text: {text}", "fn a()"),
            "title: none | text: fn a()"
        );
        assert_eq!(render("{text} [end]", "x"), "x [end]");
        assert!(check_template("t", "q: {text}").is_ok());
        assert!(check_template("t", "q: ").is_err());
        assert!(check_template("t", "{text}{text}").is_err());
    }

    #[test]
    fn a_document_call_stays_within_its_limits_and_one_context() {
        let input = |n: usize| TokenizedInput { ids: vec![1; n] };
        let limits = DocumentLimits {
            inputs: 2,
            tokens: 4,
        };
        assert!(check_document_batch(&[input(4), input(1)], limits).is_ok());
        assert!(check_document_batch(&[], limits).is_err());
        assert!(check_document_batch(&[input(1), input(1), input(1)], limits).is_err());
        assert!(check_document_batch(&[input(5)], limits).is_err());
        assert!(check_document_batch(&[input(0)], limits).is_err());
        let wide = DocumentLimits {
            inputs: 2,
            tokens: SERVING_LIMIT_TOKENS,
        };
        assert!(
            check_document_batch(&[input(SERVING_LIMIT_TOKENS), input(1)], wide).is_err(),
            "the call's total stays within one context"
        );
    }
}
