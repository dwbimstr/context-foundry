//! 013: the one exact renderer shared by training and prediction for the
//! `retrieval-route-v1` decision family (contract `learning-loop.md` § Exact
//! input and identity). T001 owns rendering and tokenization only: no weight
//! load, no tensor engine, no checkpoint execution.
//!
//! The pinned reference is the UNCHANGED upstream
//! `laya.common.build_sequence` at `4066d5d5` (fixture
//! `tests/fixtures/learning/render.json`, generated with the publisher
//! tokenizer under transformers 5.18.0). Within every limit this renderer
//! reproduces the upstream ID sequence and marker positions exactly; where
//! upstream would truncate (state over budget, over-long header or option)
//! Foundry REFUSES, per the contract: never truncate.
//!
//! Sequence: `[CLS] choice question: <instruction> [SEP] [MASK] <opt0>
//! [MASK] <opt1> [SEP] <state> [SEP]`. Components are tokenized with
//! `add_special_tokens=false` and the special IDs are inserted explicitly;
//! literal `[MASK]` strings in the state are replaced by one space before
//! tokenization. Both marker positions (the option `[MASK]`s) are recorded.
#[cfg(feature = "semantic")]
use crate::error::{FResult, FoundryError};

/// The only supported decision family in T001.
pub const FAMILY: &str = "retrieval-route-v1";
/// Upstream question type token for a choice question.
pub const QUESTION_TYPE: &str = "choice";
/// The fixed family instruction (contract § Exact input and identity).
pub const QUESTION_INSTRUCTION: &str = "Choose a retrieval strategy.";
/// The literal mask-token string replaced by one space in state (and, for
/// parity, in the instruction and option descriptions) before tokenization.
pub const MASK_LITERAL: &str = "[MASK]";

/// One stable option: an ID and its fixed description.
pub struct OptionDef {
    pub id: &'static str,
    pub description: &'static str,
}

/// The two options in stable-ID (label-index) order. `option_ids` on a row
/// is a permutation of exactly these two IDs.
pub const OPTIONS: [OptionDef; 2] = [
    OptionDef {
        id: "search",
        description: "find source text",
    },
    OptionDef {
        id: "graph",
        description: "follow symbol relationships",
    },
];

/// Maximum 1024 tokens including specials; refuse, never truncate.
pub const MAX_TOTAL_TOKENS: usize = 1024;
/// Header budget (`[CLS] instruction [SEP]`), matching upstream
/// `head_max_len`.
pub const HEADER_MAX_TOKENS: usize = 256;
/// Each rendered option (space-prefixed `id: description`) is at most this
/// many tokens.
pub const OPTION_MAX_TOKENS: usize = 48;
/// State byte guard, checked BEFORE tokenization; the 1024-token total is
/// the binding limit, so a smaller state can still be refused.
pub const STATE_MAX_BYTES: usize = 16 * 1024;

/// Render identity version: covers the template, the state composition
/// (query, `graph:` coverage line, up to three locator lines), the limits
/// and the mask-literal substitution. Bump when any of those change.
pub const RENDER_VERSION: u32 = 1;

/// The pinned decision checkpoint the tokenizer belongs to. Weights are not
/// loaded in T001; the identity fields below are what the contract lists for
/// the function digest.
pub const CHECKPOINT_SOURCE: &str = "convaiinnovations/laya-typed-decisions";
pub const CHECKPOINT_REVISION: &str = "1a793eb568e6718f15941d08f85432581df534e3";

/// The tokenizer's special IDs, pinned in `model_function_sha256` and
/// verified against the loaded tokenizer before any rendering.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
pub struct SpecialIds {
    pub cls: u32,
    pub sep: u32,
    pub pad: u32,
    pub mask: u32,
}

impl SpecialIds {
    /// The checkpoint tokenizer's special IDs (fixture `special_ids`).
    pub const PINNED: SpecialIds = SpecialIds {
        cls: 50281,
        sep: 50282,
        pad: 50283,
        mask: 50284,
    };
}

/// The rendered decision input: exact token IDs and both marker positions.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Rendered {
    pub ids: Vec<u32>,
    pub markers: [usize; 2],
}

/// `input_sha256 = SHA256(compact JSON [family,state,ordered_option_ids])`
/// over the ORIGINAL state (mask literals, original line endings) and the
/// row's ordered option IDs. Option permutation changes the identity.
pub fn input_sha256(state: &str, ordered_option_ids: [&str; 2]) -> String {
    crate::digest(
        serde_json::to_string(&serde_json::json!([FAMILY, state, ordered_option_ids]))
            .expect("compact JSON of strings")
            .as_bytes(),
    )
}

/// The T001 `model_function_sha256`: tokenizer JSON/config hashes, special
/// IDs, render version (including the state composition) and the pinned
/// checkpoint identity fields the contract lists. T002 extends this with
/// architecture/starting-weights/dtype/trainable-set identity; T001 loads
/// no weights.
pub fn model_function_sha256(
    tokenizer_json_sha256: &str,
    tokenizer_config_sha256: &str,
    special: SpecialIds,
) -> String {
    crate::digest(
        serde_json::to_string(&serde_json::json!({
            "recipe": "foundry-modernbert-choice-v1",
            "family": FAMILY,
            "render_version": RENDER_VERSION,
            "template": "[CLS] type question: instruction [SEP] [MASK] option0 [MASK] option1 [SEP] state [SEP]",
            "max_total_tokens": MAX_TOTAL_TOKENS,
            "header_max_tokens": HEADER_MAX_TOKENS,
            "option_max_tokens": OPTION_MAX_TOKENS,
            "state_max_bytes": STATE_MAX_BYTES,
            "mask_literal": MASK_LITERAL,
            "tokenizer_json_sha256": tokenizer_json_sha256,
            "tokenizer_config_sha256": tokenizer_config_sha256,
            "special_ids": special,
            "checkpoint": {
                "source": CHECKPOINT_SOURCE,
                "revision": CHECKPOINT_REVISION,
            },
        }))
        .expect("compact JSON of strings and integers")
        .as_bytes(),
    )
}

#[cfg(feature = "semantic")]
fn learning_error(code: &'static str, message: String) -> FoundryError {
    FoundryError::Learning { code, message }
}

/// The exact renderer over a loaded pinned tokenizer. The tokenizer is
/// immutable shared state: encoding always runs with
/// `add_special_tokens=false`, no truncation and no padding, so the object
/// is never mutated after load.
#[cfg(feature = "semantic")]
pub struct Renderer {
    tokenizer: tokenizers::Tokenizer,
    special: SpecialIds,
}

#[cfg(feature = "semantic")]
impl Renderer {
    /// Load from the exact `tokenizer.json` bytes. Verifies the pinned
    /// special IDs resolve to the tokens the contract names; a mismatched
    /// tokenizer is refused before any rendering.
    pub fn load(tokenizer_json: &[u8], special: SpecialIds) -> FResult<Self> {
        let mut tokenizer = tokenizers::Tokenizer::from_bytes(tokenizer_json)
            .map_err(|e| learning_error("tokenizer_invalid", format!("tokenizer.json: {e}")))?;
        // Never truncate or pad: limits are enforced by refusal here, not by
        // the library.
        let _ = tokenizer.with_truncation(None);
        tokenizer.with_padding(None);
        let resolved = [
            (special.cls, "[CLS]"),
            (special.sep, "[SEP]"),
            (special.pad, "[PAD]"),
            (special.mask, MASK_LITERAL),
        ];
        for (id, token) in resolved {
            if tokenizer.token_to_id(token) != Some(id) {
                return Err(learning_error(
                    "tokenizer_invalid",
                    format!("special token {token:?} is not id {id}"),
                ));
            }
        }
        Ok(Self { tokenizer, special })
    }

    fn encode(&self, text: &str) -> FResult<Vec<u32>> {
        let encoding = self
            .tokenizer
            .encode(text, false)
            .map_err(|e| learning_error("tokenizer_invalid", format!("encode: {e}")))?;
        Ok(encoding.get_ids().to_vec())
    }

    /// Render the exact decision input for `state` and the ordered option
    /// IDs. Within every limit the IDs and markers equal the pinned upstream
    /// fixture; any limit violation is a named refusal — never a truncation.
    pub fn render(&self, state: &str, ordered: [&str; 2]) -> FResult<Rendered> {
        let special = self.special;
        // The state byte guard runs BEFORE tokenization (contract); the
        // 1024-token total below is the binding limit.
        if state.len() > STATE_MAX_BYTES {
            return Err(learning_error(
                "state_too_large",
                format!(
                    "state is {state_len} bytes; the pre-tokenization guard is {STATE_MAX_BYTES}",
                    state_len = state.len()
                ),
            ));
        }
        if ordered[0] == ordered[1] {
            return Err(learning_error(
                "row_invalid",
                "option ids are duplicated".into(),
            ));
        }
        let lookup = |id: &str| OPTIONS.iter().find(|def| def.id == id);
        let (Some(first), Some(second)) = (lookup(ordered[0]), lookup(ordered[1])) else {
            return Err(learning_error(
                "row_invalid",
                format!("unknown option ids {ordered:?} for family {FAMILY}"),
            ));
        };
        let defs = [first, second];

        // Instruction header: `choice question: <instruction>` with mask
        // literals replaced (a no-op for the fixed instruction, kept for
        // upstream parity).
        let instruction = QUESTION_INSTRUCTION.replace(MASK_LITERAL, " ");
        let head_text = format!("{QUESTION_TYPE} question: {instruction}");
        let head_ids = self.encode(&head_text)?;
        if head_ids.len() + 2 > HEADER_MAX_TOKENS {
            return Err(learning_error(
                "header_too_long",
                format!(
                    "rendered header is {} tokens; the limit is {HEADER_MAX_TOKENS}",
                    head_ids.len() + 2
                ),
            ));
        }

        // Options: ` <id>: <description>` tokenized without specials and
        // prefixed by one explicit [MASK] (the marker).
        let mut blocks: [Vec<u32>; 2] = [Vec::new(), Vec::new()];
        let mut option_tokens = 0usize;
        for (slot, def) in defs.iter().enumerate() {
            let text = format!(
                " {}: {}",
                def.id,
                def.description.replace(MASK_LITERAL, " ")
            );
            let ids = self.encode(&text)?;
            if ids.len() > OPTION_MAX_TOKENS {
                return Err(learning_error(
                    "option_too_long",
                    format!(
                        "option {id} renders to {n} tokens; the limit is {OPTION_MAX_TOKENS}",
                        id = def.id,
                        n = ids.len()
                    ),
                ));
            }
            let mut block = Vec::with_capacity(ids.len() + 1);
            block.push(special.mask);
            block.extend_from_slice(&ids);
            option_tokens += block.len();
            blocks[slot] = block;
        }
        // Where upstream would re-slice the options or the header to fit the
        // 256-token head budget, the contract refuses instead.
        let option_budget = HEADER_MAX_TOKENS.saturating_sub(option_tokens);
        if option_budget < 16 || head_ids.len() > option_budget.max(8) {
            return Err(learning_error(
                "header_too_long",
                format!(
                    "header ({n} tokens) and options ({option_tokens} tokens) do not fit \
                     the {HEADER_MAX_TOKENS}-token head budget",
                    n = head_ids.len()
                ),
            ));
        }

        let mut ids = Vec::with_capacity(MAX_TOTAL_TOKENS + 8);
        ids.push(special.cls);
        ids.extend_from_slice(&head_ids);
        ids.push(special.sep);
        let mut markers = [0usize; 2];
        for (slot, block) in blocks.iter().enumerate() {
            markers[slot] = ids.len();
            ids.extend_from_slice(block);
        }
        ids.push(special.sep);

        let state_text = state.replace(MASK_LITERAL, " ");
        let state_ids = self.encode(&state_text)?;
        if ids.len() + state_ids.len() + 1 > MAX_TOTAL_TOKENS {
            return Err(learning_error(
                "input_too_long",
                format!(
                    "rendered input is {total} tokens; the limit is {MAX_TOTAL_TOKENS}",
                    total = ids.len() + state_ids.len() + 1
                ),
            ));
        }
        ids.extend_from_slice(&state_ids);
        ids.push(special.sep);
        Ok(Rendered { ids, markers })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn input_digest_binds_order_and_exact_state() {
        let a = input_sha256("q\ngraph: complete", ["search", "graph"]);
        let b = input_sha256("q\ngraph: complete", ["graph", "search"]);
        let c = input_sha256("q\r\ngraph: complete", ["search", "graph"]);
        assert_ne!(a, b, "option order changes the identity");
        assert_ne!(a, c, "exact state bytes change the identity");
        assert_eq!(a.len(), 64);
    }

    #[test]
    fn function_digest_is_stable_and_binds_the_tokenizer() {
        let a = model_function_sha256("a*64", "b*64", SpecialIds::PINNED);
        let b = model_function_sha256("a*64", "b*64", SpecialIds::PINNED);
        let c = model_function_sha256("c*64", "b*64", SpecialIds::PINNED);
        assert_eq!(a, b);
        assert_ne!(a, c);
    }
}
