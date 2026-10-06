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
//!
//! 013 T002 adds the numerical model's identity, which every build carries
//! (architecture constants, the trainable set, the checkpoint pin and the
//! exact tensor tables) and a minimal strict safetensors header parser the
//! core validates candidate heads with, and, behind `learning-worker`, the
//! LibTorch model itself ([`net`]): ModernBERT-large plus the Laya choice
//! head, float32 on the CPU, batch one.
#[cfg(feature = "semantic")]
use crate::error::FResult;
use crate::error::FoundryError;

#[cfg(feature = "learning-worker")]
pub mod net;

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

/// The checkpoint identity a run policy pins (contract § Exact input and
/// identity): the exact starting weights, the encoder configuration and the
/// checkpoint's tensor dtype. Every tensor is upcast to float32 at load;
/// the dtype and that upcast belong to [`model_function_sha256`], so a
/// float32 or bfloat16 re-save of the same checkpoint is another function.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CheckpointPin {
    /// SHA-256 of the exact `model.safetensors` bytes.
    pub weights_sha256: String,
    /// SHA-256 of the exact `encoder/config.json` bytes.
    pub encoder_config_sha256: String,
    /// The dtype every checkpoint tensor is stored in: `F16` for the pinned
    /// checkpoint; `BF16` and `F32` are the other dtypes the loader upcasts.
    pub source_dtype: String,
}

/// Every computation is float32 on the CPU.
pub const COMPUTE_DTYPE: &str = "float32";

/// ModernBERT-large plus the Laya choice head, exactly as the pinned
/// checkpoint's `encoder/config.json` and unchanged Laya `DecisionModel`
/// (`laya/common.py` @4066d5d5) define them. The worker refuses a config
/// that disagrees with any of these.
pub mod arch {
    pub const HIDDEN: usize = 1024;
    pub const VOCAB: usize = 50368;
    pub const LAYERS: usize = 28;
    pub const HEADS: usize = 16;
    pub const HEAD_DIM: usize = 64;
    /// Gated GELU: `Wi` produces twice this, split into input and gate.
    pub const INTERMEDIATE: usize = 2624;
    /// Layers `0, 3, 6, …` attend globally; the rest locally.
    pub const GLOBAL_EVERY: usize = 3;
    /// Local attention admits keys at INCLUSIVE distance <= 64
    /// (`local_attention` 128 / 2).
    pub const LOCAL_WINDOW: usize = 64;
    pub const GLOBAL_ROPE_THETA: f64 = 160_000.0;
    pub const LOCAL_ROPE_THETA: f64 = 10_000.0;
    /// Bias-free encoder norms and every head norm use this epsilon.
    pub const NORM_EPS: f64 = 1e-5;
    /// Two pre-norm `nn.TransformerEncoderLayer`s, 16 heads, ReLU.
    pub const HEAD_LAYERS: usize = 2;
    pub const HEAD_FF: usize = 4096;
    /// Head dropout in training; evaluation disables it.
    pub const HEAD_DROPOUT: f64 = 0.1;
    /// `type_emb` rows: choice, score, noul. Only row 0 (choice) trains.
    pub const TYPE_ROWS: usize = 3;
    /// `act_head` input: the pooled CLS row plus four distribution features.
    pub const ACT_IN: usize = HIDDEN + 4;
    pub const ACT_HIDDEN: usize = 256;
    pub const ACT_OUT: usize = 2;
    /// A masked marker's logit (upstream `masked_fill(~marker_mask, -1e4)`).
    pub const MASKED_LOGIT: f64 = -1e4;
}

/// The fitting recipe's optimizer constants (contract § Fitting, artifacts
/// and evaluation): AdamW with torch semantics (decoupled weight decay on
/// every trainable tensor), global gradient-norm clip, batch one, no
/// accumulation. A run policy states them and must state exactly these.
pub mod recipe {
    pub const OPTIMIZER: &str = "adamw";
    pub const LEARNING_RATE: f64 = 1e-4;
    pub const BETA1: f64 = 0.9;
    pub const BETA2: f64 = 0.999;
    pub const EPSILON: f64 = 1e-8;
    pub const WEIGHT_DECAY: f64 = 0.01;
    pub const CLIP_GLOBAL_NORM: f64 = 1.0;
}

/// The contract's trainable set (contract § One model and one decision,
/// 40-44), in the order the optimizer and the reference list it: both head
/// layers, the choice row of the type embedding (`type_emb.choice_row`, row
/// 0 only — rows 1 and 2 stay frozen, so they get neither gradient nor
/// weight decay) and the scorer. `head.safetensors` holds exactly these,
/// float32. The set belongs to [`model_function_sha256`].
pub fn trainable() -> Vec<(String, Vec<usize>)> {
    use arch::{HEAD_FF, HIDDEN};
    let mut set = Vec::with_capacity(31);
    for layer in 0..arch::HEAD_LAYERS {
        let p = format!("head.layers.{layer}");
        for (name, shape) in [
            ("self_attn.in_proj_weight", vec![3 * HIDDEN, HIDDEN]),
            ("self_attn.in_proj_bias", vec![3 * HIDDEN]),
            ("self_attn.out_proj.weight", vec![HIDDEN, HIDDEN]),
            ("self_attn.out_proj.bias", vec![HIDDEN]),
            ("linear1.weight", vec![HEAD_FF, HIDDEN]),
            ("linear1.bias", vec![HEAD_FF]),
            ("linear2.weight", vec![HIDDEN, HEAD_FF]),
            ("linear2.bias", vec![HIDDEN]),
            ("norm1.weight", vec![HIDDEN]),
            ("norm1.bias", vec![HIDDEN]),
            ("norm2.weight", vec![HIDDEN]),
            ("norm2.bias", vec![HIDDEN]),
        ] {
            set.push((format!("{p}.{name}"), shape));
        }
    }
    set.push((CHOICE_ROW.to_owned(), vec![HIDDEN]));
    for (name, shape) in [
        ("scorer.0.weight", vec![HIDDEN]),
        ("scorer.0.bias", vec![HIDDEN]),
        ("scorer.1.weight", vec![HIDDEN, HIDDEN]),
        ("scorer.1.bias", vec![HIDDEN]),
        ("scorer.3.weight", vec![1, HIDDEN]),
        ("scorer.3.bias", vec![1]),
    ] {
        set.push((name.to_owned(), shape));
    }
    set
}

/// The optimizer-visible choice row of `type_emb.weight` (row 0).
pub const CHOICE_ROW: &str = "type_emb.choice_row";

/// Every tensor the pinned checkpoint must hold, by exact name and shape. A
/// missing, extra or shape-invalid tensor is refused. `act_head.*` and the
/// reference `temperature` buffer are required present (they are part of
/// the checkpoint) but never used or trained: the fitted scalar is the only
/// calibration authority.
pub fn checkpoint_tensors() -> Vec<(String, Vec<usize>)> {
    use arch::{ACT_HIDDEN, ACT_IN, ACT_OUT, HIDDEN, INTERMEDIATE, TYPE_ROWS, VOCAB};
    let mut set = vec![
        (
            "encoder.embeddings.tok_embeddings.weight".to_owned(),
            vec![VOCAB, HIDDEN],
        ),
        ("encoder.embeddings.norm.weight".to_owned(), vec![HIDDEN]),
        ("encoder.final_norm.weight".to_owned(), vec![HIDDEN]),
    ];
    for layer in 0..arch::LAYERS {
        let p = format!("encoder.layers.{layer}");
        set.push((format!("{p}.attn.Wqkv.weight"), vec![3 * HIDDEN, HIDDEN]));
        set.push((format!("{p}.attn.Wo.weight"), vec![HIDDEN, HIDDEN]));
        set.push((format!("{p}.mlp.Wi.weight"), vec![2 * INTERMEDIATE, HIDDEN]));
        set.push((format!("{p}.mlp.Wo.weight"), vec![HIDDEN, INTERMEDIATE]));
        set.push((format!("{p}.mlp_norm.weight"), vec![HIDDEN]));
        // Layer 0 has no attention pre-normalization.
        if layer > 0 {
            set.push((format!("{p}.attn_norm.weight"), vec![HIDDEN]));
        }
    }
    set.extend(
        trainable()
            .into_iter()
            .filter(|(name, _)| name != CHOICE_ROW),
    );
    set.push(("type_emb.weight".to_owned(), vec![TYPE_ROWS, HIDDEN]));
    set.push(("act_head.0.weight".to_owned(), vec![ACT_HIDDEN, ACT_IN]));
    set.push(("act_head.0.bias".to_owned(), vec![ACT_HIDDEN]));
    set.push(("act_head.2.weight".to_owned(), vec![ACT_OUT, ACT_HIDDEN]));
    set.push(("act_head.2.bias".to_owned(), vec![ACT_OUT]));
    set.push(("temperature".to_owned(), vec![TYPE_ROWS]));
    set
}

/// `model_function_sha256`: tokenizer JSON/config hashes, special IDs,
/// render version (including the state composition), the architecture, the
/// starting weights, the checkpoint dtype and its float32 upcast, and the
/// trainable set (contract § Exact input and identity). Computed from the
/// policy's pins: preparation never loads weights; training verifies the
/// actual files against the same pins before and after the worker loads them.
pub fn model_function_sha256(
    tokenizer_json_sha256: &str,
    tokenizer_config_sha256: &str,
    special: SpecialIds,
    checkpoint: &CheckpointPin,
) -> String {
    use arch::*;
    let trainable: Vec<String> = trainable().into_iter().map(|(name, _)| name).collect();
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
                "weights_sha256": checkpoint.weights_sha256,
                "encoder_config_sha256": checkpoint.encoder_config_sha256,
                "source_dtype": checkpoint.source_dtype,
                "upcast": COMPUTE_DTYPE,
            },
            "architecture": {
                "encoder": "modernbert",
                "hidden": HIDDEN,
                "vocab": VOCAB,
                "layers": LAYERS,
                "heads": HEADS,
                "head_dim": HEAD_DIM,
                "intermediate": INTERMEDIATE,
                "mlp": "gated-gelu-erf",
                "layer0_attention_norm": false,
                "global_every": GLOBAL_EVERY,
                "local_window_inclusive": LOCAL_WINDOW,
                "rope_theta": {"global": GLOBAL_ROPE_THETA, "local": LOCAL_ROPE_THETA},
                "norm": "layernorm-no-bias",
                "norm_eps": NORM_EPS,
                "embedding_norm": true,
                "head": {
                    "layers": HEAD_LAYERS,
                    "norm_first": true,
                    "heads": HEADS,
                    "feedforward": HEAD_FF,
                    "activation": "relu",
                    "dropout": HEAD_DROPOUT,
                    "type_row": 0,
                    "pooling": "markers",
                    "scorer": "layernorm-linear-gelu-erf-linear",
                    "masked_logit": MASKED_LOGIT,
                },
                "batch": 1,
                "padding": false,
            },
            "trainable": trainable,
        }))
        .expect("compact JSON of strings and numbers")
        .as_bytes(),
    )
}

fn learning_error(code: &'static str, message: String) -> FoundryError {
    FoundryError::Learning { code, message }
}

/// A minimal strict safetensors reader and writer: the core validates
/// candidate heads with it and the worker loads the checkpoint and writes
/// the head through it. Chosen over the `safetensors` crate because the
/// default core build must validate heads without a new dependency, and
/// because the header goes through the core's strict JSON parser, which
/// refuses duplicate keys (a duplicate tensor name would otherwise silently
/// keep the last entry), unknown fields and nesting; offsets must tile the
/// data region exactly, with no gap, overlap or trailing byte.
pub mod safetensors {
    use super::learning_error;
    use crate::error::FResult;

    /// Both headers this code reads are far smaller (checkpoint 21 KiB,
    /// head about 3 KiB).
    pub const MAX_HEADER_BYTES: u64 = 1024 * 1024;

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum Dtype {
        F16,
        BF16,
        F32,
    }

    impl Dtype {
        pub fn parse(name: &str) -> Option<Self> {
            match name {
                "F16" => Some(Self::F16),
                "BF16" => Some(Self::BF16),
                "F32" => Some(Self::F32),
                _ => None,
            }
        }

        pub fn as_str(self) -> &'static str {
            match self {
                Self::F16 => "F16",
                Self::BF16 => "BF16",
                Self::F32 => "F32",
            }
        }

        pub fn size(self) -> u64 {
            match self {
                Self::F16 | Self::BF16 => 2,
                Self::F32 => 4,
            }
        }
    }

    /// One tensor: its data occupies `start..end` of the data region.
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub struct Entry {
        pub name: String,
        pub dtype: Dtype,
        pub shape: Vec<usize>,
        pub start: u64,
        pub end: u64,
    }

    #[derive(serde::Deserialize)]
    #[serde(deny_unknown_fields)]
    struct RawEntry {
        dtype: String,
        shape: Vec<u64>,
        data_offsets: [u64; 2],
    }

    /// The header length from a file's first eight bytes, bounded.
    pub fn header_len(prefix: [u8; 8], code: &'static str) -> FResult<u64> {
        let len = u64::from_le_bytes(prefix);
        if len == 0 || len > MAX_HEADER_BYTES {
            return Err(learning_error(
                code,
                format!("safetensors header of {len} bytes; 1..={MAX_HEADER_BYTES} allowed"),
            ));
        }
        Ok(len)
    }

    /// Parse `header` (the JSON between the length prefix and the data) of
    /// a file whose data region is `data_len` bytes. Entries come back in
    /// data order; every refusal is `code`.
    pub fn parse_header(header: &[u8], data_len: u64, code: &'static str) -> FResult<Vec<Entry>> {
        let invalid = |message: String| learning_error(code, message);
        let object: serde_json::Map<String, serde_json::Value> =
            crate::learning::strict_json(header, code, "safetensors header")?;
        let mut entries = Vec::with_capacity(object.len());
        for (name, value) in object {
            if name == "__metadata__" {
                let metadata: std::collections::BTreeMap<String, String> =
                    serde_json::from_value(value)
                        .map_err(|e| invalid(format!("safetensors metadata: {e}")))?;
                drop(metadata);
                continue;
            }
            let raw: RawEntry = serde_json::from_value(value)
                .map_err(|e| invalid(format!("tensor {name}: {e}")))?;
            let dtype = Dtype::parse(&raw.dtype).ok_or_else(|| {
                invalid(format!("tensor {name} has unsupported dtype {}", raw.dtype))
            })?;
            let [start, end] = raw.data_offsets;
            let elements = raw
                .shape
                .iter()
                .try_fold(1u64, |total, dim| total.checked_mul(*dim));
            let bytes = elements.and_then(|n| n.checked_mul(dtype.size()));
            if end < start || bytes != Some(end - start) {
                return Err(invalid(format!(
                    "tensor {name}: offsets {start}..{end} do not hold its {:?} {}",
                    raw.shape,
                    dtype.as_str()
                )));
            }
            let shape = raw
                .shape
                .iter()
                .map(|dim| usize::try_from(*dim))
                .collect::<Result<Vec<usize>, _>>()
                .map_err(|_| invalid(format!("tensor {name}: dimension overflow")))?;
            entries.push(Entry {
                name,
                dtype,
                shape,
                start,
                end,
            });
        }
        entries.sort_by_key(|entry| (entry.start, entry.end));
        let mut cursor = 0u64;
        for entry in &entries {
            if entry.start != cursor {
                return Err(invalid(format!(
                    "tensor {} starts at {} where the data is at {cursor}: a gap or overlap",
                    entry.name, entry.start
                )));
            }
            cursor = entry.end;
        }
        if cursor != data_len {
            return Err(invalid(format!(
                "the tensors cover {cursor} of the {data_len} data bytes"
            )));
        }
        Ok(entries)
    }

    /// The entries must be exactly `expected` (name and shape), every one
    /// stored as `dtype`: a missing, extra, reshaped or retyped tensor is
    /// refused by name.
    pub fn check_tensors(
        entries: &[Entry],
        expected: &[(String, Vec<usize>)],
        dtype: Dtype,
        code: &'static str,
    ) -> FResult<()> {
        let by_name: std::collections::BTreeMap<&str, &Entry> = entries
            .iter()
            .map(|entry| (entry.name.as_str(), entry))
            .collect();
        for (name, shape) in expected {
            let Some(entry) = by_name.get(name.as_str()) else {
                return Err(learning_error(code, format!("tensor {name} is missing")));
            };
            if entry.shape != *shape {
                return Err(learning_error(
                    code,
                    format!("tensor {name} has shape {:?}, not {shape:?}", entry.shape),
                ));
            }
            if entry.dtype != dtype {
                return Err(learning_error(
                    code,
                    format!(
                        "tensor {name} is {}, not {}",
                        entry.dtype.as_str(),
                        dtype.as_str()
                    ),
                ));
            }
        }
        if let Some(extra) = entries
            .iter()
            .find(|entry| !expected.iter().any(|(name, _)| *name == entry.name))
        {
            return Err(learning_error(
                code,
                format!("tensor {} is not part of this model", extra.name),
            ));
        }
        Ok(())
    }

    /// The length prefix and header of a file holding `tensors` (name,
    /// shape) as float32, in name order, contiguous from offset 0; the JSON
    /// is space-padded to a multiple of eight bytes. The data follows in
    /// the same order. Returns the bytes and each tensor's data range.
    pub fn encode_f32_header(tensors: &[(String, Vec<usize>)]) -> (Vec<u8>, Vec<(u64, u64)>) {
        let mut order: Vec<usize> = (0..tensors.len()).collect();
        order.sort_by(|a, b| tensors[*a].0.cmp(&tensors[*b].0));
        let mut ranges = vec![(0u64, 0u64); tensors.len()];
        let mut object = serde_json::Map::new();
        let mut cursor = 0u64;
        for index in order {
            let (name, shape) = &tensors[index];
            let bytes = shape.iter().product::<usize>() as u64 * 4;
            ranges[index] = (cursor, cursor + bytes);
            object.insert(
                name.clone(),
                serde_json::json!({
                    "dtype": "F32",
                    "shape": shape,
                    "data_offsets": [cursor, cursor + bytes],
                }),
            );
            cursor += bytes;
        }
        let mut json = serde_json::to_vec(&object).expect("JSON of strings and integers");
        while !json.len().is_multiple_of(8) {
            json.push(b' ');
        }
        let mut out = (json.len() as u64).to_le_bytes().to_vec();
        out.extend_from_slice(&json);
        (out, ranges)
    }
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
    use crate::error::FResult;

    #[test]
    fn input_digest_binds_order_and_exact_state() {
        let a = input_sha256("q\ngraph: complete", ["search", "graph"]);
        let b = input_sha256("q\ngraph: complete", ["graph", "search"]);
        let c = input_sha256("q\r\ngraph: complete", ["search", "graph"]);
        assert_ne!(a, b, "option order changes the identity");
        assert_ne!(a, c, "exact state bytes change the identity");
        assert_eq!(a.len(), 64);
    }

    fn pin(dtype: &str) -> CheckpointPin {
        CheckpointPin {
            weights_sha256: "d".repeat(64),
            encoder_config_sha256: "e".repeat(64),
            source_dtype: dtype.to_owned(),
        }
    }

    #[test]
    fn function_digest_is_stable_and_binds_the_tokenizer() {
        let a = model_function_sha256("a*64", "b*64", SpecialIds::PINNED, &pin("F16"));
        let b = model_function_sha256("a*64", "b*64", SpecialIds::PINNED, &pin("F16"));
        let c = model_function_sha256("c*64", "b*64", SpecialIds::PINNED, &pin("F16"));
        assert_eq!(a, b);
        assert_ne!(a, c);
    }

    #[test]
    fn function_digest_binds_the_source_dtype_and_the_starting_weights() {
        let f16 = model_function_sha256("a", "b", SpecialIds::PINNED, &pin("F16"));
        for other in ["F32", "BF16"] {
            assert_ne!(
                f16,
                model_function_sha256("a", "b", SpecialIds::PINNED, &pin(other)),
                "{other}: the same weights re-saved in another dtype are another function"
            );
        }
        let mut weights = pin("F16");
        weights.weights_sha256 = "f".repeat(64);
        assert_ne!(
            f16,
            model_function_sha256("a", "b", SpecialIds::PINNED, &weights)
        );
        let mut config = pin("F16");
        config.encoder_config_sha256 = "f".repeat(64);
        assert_ne!(
            f16,
            model_function_sha256("a", "b", SpecialIds::PINNED, &config)
        );
    }

    #[test]
    fn the_trainable_set_is_the_contracts_and_excludes_frozen_rows() {
        let set = trainable();
        assert_eq!(set.len(), 31);
        assert!(set.iter().all(|(name, _)| name.starts_with("head.")
            || name.starts_with("scorer.")
            || name == CHOICE_ROW));
        // Only the choice row trains: the full type table, the encoder and
        // the unused act head are never optimizer-visible.
        assert!(!set.iter().any(|(name, _)| name == "type_emb.weight"
            || name.starts_with("encoder.")
            || name.starts_with("act_head.")
            || name == "temperature"));
        let elements: usize = set.iter().map(|(_, s)| s.iter().product::<usize>()).sum();
        assert_eq!(elements, 26_246_145);
        // The checkpoint table is the 206 tensors of the pinned checkpoint.
        let checkpoint = checkpoint_tensors();
        assert_eq!(checkpoint.len(), 206);
        assert!(
            checkpoint
                .iter()
                .any(|(n, s)| n == "type_emb.weight" && *s == [3, 1024])
        );
        assert!(
            !checkpoint
                .iter()
                .any(|(n, _)| n == "encoder.layers.0.attn_norm.weight")
        );
    }

    fn f32_file(tensors: &[(String, Vec<usize>)]) -> Vec<u8> {
        let (mut bytes, ranges) = safetensors::encode_f32_header(tensors);
        for (start, end) in ranges
            .iter()
            .copied()
            .collect::<std::collections::BTreeSet<_>>()
        {
            bytes.extend(std::iter::repeat_n(0u8, (end - start) as usize));
        }
        bytes
    }

    fn parse(bytes: &[u8]) -> FResult<Vec<safetensors::Entry>> {
        let len = safetensors::header_len(bytes[..8].try_into().unwrap(), "artifact_invalid")?;
        let data = bytes.len() as u64 - 8 - len;
        safetensors::parse_header(&bytes[8..8 + len as usize], data, "artifact_invalid")
    }

    #[test]
    fn the_safetensors_header_round_trips_and_checks_the_exact_set() {
        let tensors = vec![
            ("b.weight".to_owned(), vec![2, 3]),
            ("a.bias".to_owned(), vec![3]),
        ];
        let bytes = f32_file(&tensors);
        let entries = parse(&bytes).unwrap();
        assert_eq!(entries[0].name, "a.bias", "name order, contiguous from 0");
        assert_eq!((entries[0].start, entries[0].end), (0, 12));
        assert_eq!((entries[1].start, entries[1].end), (12, 36));
        safetensors::check_tensors(&entries, &tensors, safetensors::Dtype::F32, "x").unwrap();
        let missing = &tensors[..1];
        let code = |r: FResult<()>| r.unwrap_err().code();
        assert_eq!(
            code(safetensors::check_tensors(
                &entries,
                missing,
                safetensors::Dtype::F32,
                "extra"
            )),
            "extra"
        );
        let mut reshaped = tensors.clone();
        reshaped[0].1 = vec![3, 2];
        assert!(
            safetensors::check_tensors(&entries, &reshaped, safetensors::Dtype::F32, "x").is_err()
        );
        assert!(
            safetensors::check_tensors(&entries, &tensors, safetensors::Dtype::F16, "x").is_err()
        );
    }

    #[test]
    fn malformed_safetensors_headers_are_refused() {
        let header = |json: &str, data: u64| {
            safetensors::parse_header(json.as_bytes(), data, "artifact_invalid").map(|_| ())
        };
        // Duplicate names, a gap, an overlap, trailing data, a wrong size,
        // an unknown dtype and an unknown field.
        for (json, data) in [
            (
                r#"{"a":{"dtype":"F32","shape":[1],"data_offsets":[0,4]},"a":{"dtype":"F32","shape":[1],"data_offsets":[4,8]}}"#,
                8,
            ),
            (
                r#"{"a":{"dtype":"F32","shape":[1],"data_offsets":[4,8]}}"#,
                8,
            ),
            (
                r#"{"a":{"dtype":"F32","shape":[2],"data_offsets":[0,8]},"b":{"dtype":"F32","shape":[1],"data_offsets":[4,8]}}"#,
                8,
            ),
            (
                r#"{"a":{"dtype":"F32","shape":[1],"data_offsets":[0,4]}}"#,
                8,
            ),
            (
                r#"{"a":{"dtype":"F32","shape":[2],"data_offsets":[0,4]}}"#,
                4,
            ),
            (
                r#"{"a":{"dtype":"I64","shape":[1],"data_offsets":[0,8]}}"#,
                8,
            ),
            (
                r#"{"a":{"dtype":"F32","shape":[1],"data_offsets":[0,4],"x":1}}"#,
                4,
            ),
        ] {
            assert!(header(json, data).is_err(), "{json}");
        }
        assert!(header(r#"{"__metadata__":{"k":"v"},"a":{"dtype":"F32","shape":[1],"data_offsets":[0,4]}}"#, 4).is_ok());
        assert!(safetensors::header_len([0; 8], "x").is_err());
        assert!(safetensors::header_len((2u64 << 20).to_le_bytes(), "x").is_err());
    }
}
