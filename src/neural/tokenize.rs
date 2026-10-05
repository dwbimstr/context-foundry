//! 009 Rust tokenization boundary: the profile's verified `tokenizer.json`
//! loaded through `tokenizers` (pinned 0.23.2 `onig`), counting the exact
//! model input. Foundry alone applies the prefix, counts and refuses; the
//! worker receives token IDs, never text.
//!
//! The loader's own truncation/padding settings (restored from the artifact
//! by `Tokenizer::from_file`) are cleared here: a unit that would exceed a
//! limit is refused upstream, never silently shortened.
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
        let path = profile.model_dir.join(TOKENIZER_ARTIFACT);
        let mut tokenizer = tokenizers::Tokenizer::from_file(&path)
            .map_err(|e| ProviderError::ProfileInvalid(format!("{}: {e}", path.display())))?;
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
