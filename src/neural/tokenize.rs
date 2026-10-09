//! 009 Rust tokenization boundary: the profile's verified `tokenizer.json`
//! loaded through `tokenizers` (pinned 0.23.2 `onig`), counting the exact
//! model input. Foundry alone applies the profile's templates, counts and
//! refuses; the worker receives token IDs, never text.
//!
//! The loader's own truncation/padding settings (restored from the artifact
//! by `Tokenizer::from_file`) are cleared here: a card that would exceed its
//! limit is cut at a line by the card partition, never silently shortened.
use crate::neural::partition::{TokenCount, TokenizedText};
use crate::neural::profile::SemanticProfile;
use crate::neural::provider::ProviderError;

/// The artifact file this boundary reads; it must be among the descriptor's
/// verified `artifact_files` (and therefore hash-verified) before loading.
pub const TOKENIZER_ARTIFACT: &str = "tokenizer.json";

/// The exact-input tokenizer of one profile.
pub struct DocumentTokenizer {
    tokenizer: tokenizers::Tokenizer,
    add_special_tokens: bool,
    identity: String,
}

impl DocumentTokenizer {
    /// Load `model_dir/tokenizer.json`. The caller runs
    /// [`SemanticProfile::verify_artifacts`] first; this constructor still
    /// refuses a profile whose descriptor does not pin the tokenizer file.
    pub fn load(profile: &SemanticProfile) -> Result<Self, ProviderError> {
        let artifacts = profile.verified_artifacts_until(&crate::Control::unbounded())?;
        Self::load_verified(profile, &artifacts)
    }

    pub fn load_verified(
        profile: &SemanticProfile,
        artifacts: &crate::neural::profile::VerifiedArtifacts,
    ) -> Result<Self, ProviderError> {
        artifacts.check(profile, &crate::Control::unbounded())?;
        if !profile
            .descriptor
            .artifact_files
            .iter()
            .any(|file| file.name == TOKENIZER_ARTIFACT)
        {
            return Err(ProviderError::ProfileInvalid(format!(
                "the descriptor does not pin {TOKENIZER_ARTIFACT}"
            )));
        }
        let mut tokenizer = tokenizers::Tokenizer::from_bytes(artifacts.tokenizer_bytes()?)
            .map_err(|e| ProviderError::ProfileInvalid(format!("verified tokenizer.json: {e}")))?;
        // Never inherit the artifact's truncation or padding: Foundry counts
        // the exact model input and refuses over-limit inputs.
        let _ = tokenizer.with_truncation(None);
        tokenizer.with_padding(None);
        Ok(Self {
            tokenizer,
            add_special_tokens: profile.descriptor.add_special_tokens,
            identity: profile.descriptor.tokenizer.clone(),
        })
    }

    /// The tokenizer identity string of the descriptor (recipe input).
    pub fn identity(&self) -> &str {
        &self.identity
    }
}

impl TokenCount for DocumentTokenizer {
    fn fitting(&self) -> Option<Box<dyn TokenCount + '_>> {
        // A stochastic BPE cannot reuse a previous draw. Deterministic models
        // reuse only identical normalized pre-token segments, never token IDs
        // truncated from a longer string or a monotonic-prefix assumption.
        if matches!(self.tokenizer.get_model(), tokenizers::models::ModelWrapper::BPE(bpe)
            if bpe.dropout.is_some_and(|dropout| dropout > 0.0))
        {
            return None;
        }
        Some(Box::new(CardTokenizer {
            owner: self,
            cache: Default::default(),
        }))
    }

    fn encode(&self, rendered: &str) -> Result<TokenizedText, ProviderError> {
        let encoding = self
            .tokenizer
            .encode(rendered, self.add_special_tokens)
            .map_err(|e| ProviderError::ProfileInvalid(format!("tokenization failed: {e}")))?;
        Ok(TokenizedText {
            ids: encoding.get_ids().to_vec(),
            offsets: encoding
                .get_offsets()
                .iter()
                .map(|&(start, end)| (start, end))
                .collect(),
        })
    }
}

/// One card only: at most 128 KiB of retained segment/token payload and 1,024
/// entries. Overflow falls back to ordinary model tokenization without eviction
/// or changing the selected line prefix. Normalization, added-token extraction,
/// pre-tokenization and post-processing still run for each complete candidate.
struct CardTokenizer<'a> {
    owner: &'a DocumentTokenizer,
    cache: std::cell::RefCell<SegmentCache>,
}

#[derive(Default)]
struct SegmentCache {
    segments: std::collections::HashMap<String, Vec<tokenizers::Token>>,
    bytes: usize,
}

impl TokenCount for CardTokenizer<'_> {
    fn encode(&self, rendered: &str) -> Result<TokenizedText, ProviderError> {
        use tokenizers::{Model, PreTokenizer};
        let tokenizer = &self.owner.tokenizer;
        let encoded = (|| -> tokenizers::Result<tokenizers::Encoding> {
            let mut input = tokenizer
                .get_added_vocabulary()
                .extract_and_normalize(tokenizer.get_normalizer(), rendered);
            if let Some(pre) = tokenizer.get_pre_tokenizer() {
                pre.pre_tokenize(&mut input)?;
            }
            input.tokenize(|normalized| {
                let text = normalized.get();
                if let Some(tokens) = self.cache.borrow().segments.get(text) {
                    return Ok(tokens.clone());
                }
                neural_fault!(CARD_SEGMENT, None, text).map_err(|e| e.to_string())?;
                let tokens = tokenizer.get_model().tokenize(text)?;
                let bytes = text
                    .len()
                    .saturating_add(tokens.capacity() * std::mem::size_of::<tokenizers::Token>())
                    .saturating_add(
                        tokens
                            .iter()
                            .map(|token| token.value.capacity())
                            .sum::<usize>(),
                    );
                let mut cache = self.cache.borrow_mut();
                if cache.segments.len() < 1024 && cache.bytes.saturating_add(bytes) <= 128 * 1024 {
                    cache.bytes += bytes;
                    cache.segments.insert(text.to_owned(), tokens.clone());
                }
                Ok(tokens)
            })?;
            let encoding = input.into_encoding(None, 0, tokenizers::OffsetType::Byte)?;
            tokenizer.post_process(encoding, None, self.owner.add_special_tokens)
        })()
        .map_err(|e| ProviderError::ProfileInvalid(format!("tokenization failed: {e}")))?;
        Ok(TokenizedText {
            ids: encoded.get_ids().to_vec(),
            offsets: encoded.get_offsets().to_vec(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fault::{self, Action};
    use crate::neural::{fault_names, partition};
    use std::cell::RefCell;
    use std::rc::Rc;

    fn tokenizer() -> DocumentTokenizer {
        let dir = tempfile::tempdir().unwrap();
        let (path, _) = crate::testkit::write_byte_tokenizer(dir.path());
        let mut json: serde_json::Value =
            serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        json["normalizer"] = serde_json::json!({"type":"NFKC"});
        json["added_tokens"] = serde_json::json!([{"id":257,"content":"<mark>","single_word":false,"lstrip":true,"rstrip":false,"normalized":false,"special":true}]);
        json["model"]["vocab"]["xy"] = 256.into();
        json["model"]["merges"] = serde_json::json!([["x", "y"]]);
        DocumentTokenizer {
            tokenizer: tokenizers::Tokenizer::from_bytes(serde_json::to_vec(&json).unwrap())
                .unwrap(),
            add_special_tokens: true,
            identity: "fixture".into(),
        }
    }

    #[test]
    fn fitting_reuses_normalized_segments_with_exact_ids_offsets_and_card_keys() {
        let tokenizer = tokenizer();
        let seen = Rc::new(RefCell::new(
            std::collections::HashMap::<String, usize>::new(),
        ));
        let observed = seen.clone();
        fault::arm(
            fault_names::CARD_SEGMENT,
            0,
            Action::Call(Box::new(move |ctx| {
                *observed
                    .borrow_mut()
                    .entry(ctx.detail.to_owned())
                    .or_default() += 1;
            })),
        );
        let fitting = tokenizer.fitting().unwrap();
        for text in [
            "doc: xy",
            "doc: xy\né",
            "doc: xy\né\n<mark> Ａ",
            "doc: xy\né\n<mark> Ａ\n💡",
        ] {
            assert_eq!(
                fitting.encode(text).unwrap(),
                tokenizer.encode(text).unwrap()
            );
        }
        assert!(seen.borrow().values().all(|count| *count == 1));
        fault::disarm_all();
        // Every limit preserves the exact old first-overflow line selection,
        // including template suffixes, Unicode and truncation of long addresses.
        struct Plain<'a>(&'a DocumentTokenizer);
        impl TokenCount for Plain<'_> {
            fn encode(&self, text: &str) -> Result<TokenizedText, ProviderError> {
                self.0.encode(text)
            }
        }
        let source = "/// xy\n/// é <mark> Ａ 💡\npub fn probe(xy: u32) -> u32 {\n    xy\n}\n";
        for limit in 16..180 {
            let recipe = partition::CardRecipe {
                template: "doc: {text} suffix",
                function_digest: "fixture",
                card_tokens: limit,
            };
            let actual = partition::cards(
                source,
                "src/long/é/probe.rs",
                Some(crate::syntax::Lang::Rust),
                &recipe,
                &tokenizer,
            )
            .unwrap();
            let expected = partition::cards(
                source,
                "src/long/é/probe.rs",
                Some(crate::syntax::Lang::Rust),
                &recipe,
                &Plain(&tokenizer),
            )
            .unwrap();
            assert_eq!(actual.len(), expected.len());
            for (actual, expected) in actual.iter().zip(expected.iter()) {
                assert_eq!(
                    (&actual.input_key, &actual.ids, actual.start, actual.end),
                    (
                        &expected.input_key,
                        &expected.ids,
                        expected.start,
                        expected.end
                    )
                );
            }
        }
    }

    #[test]
    fn fitting_payload_overflow_falls_back_without_changing_tokens() {
        let tokenizer = tokenizer();
        let fitting = CardTokenizer {
            owner: &tokenizer,
            cache: Default::default(),
        };
        for i in 0..1100 {
            let text = format!("segment{i}{}", "x".repeat(160));
            assert_eq!(
                fitting.encode(&text).unwrap(),
                tokenizer.encode(&text).unwrap()
            );
        }
        let cache = fitting.cache.borrow();
        assert!(cache.bytes <= 128 * 1024);
        assert!(cache.segments.len() <= 1024);
    }
}
