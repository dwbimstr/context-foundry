# Laya decision ecosystem: source-to-contract map

Reviewed 2026-09-29 against clean Laya commit
[`4066d5d5fbf08b66c6757ddeedbd797bd7655bc0`](https://github.com/NandhaKishorM/laya/tree/4066d5d5fbf08b66c6757ddeedbd797bd7655bc0).
This is the review map for [013](../../specs/013-owned-learning/spec.md), not another
requirements owner. [Contract v4](../../specs/013-owned-learning/contracts/learning-loop.md)
owns Foundry behavior. References below name real upstream modules/functions/tests;
planned Rust modules are explicitly distinguished from existing code.

**Preserve the decision ecosystem, not just its tensor head:** joint typed inputs,
pretrained decisions, calibrated confidence/abstention, externally checked feedback,
repeatable adaptation, evaluation, artifact identity and supervised deployment.
Search/graph is the first consuming workflow, not a claim of full Laya equivalence.
Variable-choice, boolean (`noul`) and ordinal-score decisions remain extension targets;
their workflow/data/output contracts must be selected before implementation. KISS means
one existing learning loop with a concrete consumer, not three new model services.

## Source identities and scope

| Reference | Fixed identity and review use |
| --- | --- |
| Laya implementation and regression tests | Commit above, public [repository](https://github.com/NandhaKishorM/laya); clean local checkout read as source. [License at that commit](https://github.com/NandhaKishorM/laya/blob/4066d5d5fbf08b66c6757ddeedbd797bd7655bc0/LICENSE). No source vendored in Foundry. |
| Pretrained ModernBERT/typed head | [Checkpoint revision `1a793eb568e6718f15941d08f85432581df534e3`](https://huggingface.co/convaiinnovations/laya-typed-decisions/tree/1a793eb568e6718f15941d08f85432581df534e3), separate [encoder config](https://huggingface.co/convaiinnovations/laya-typed-decisions/blob/1a793eb568e6718f15941d08f85432581df534e3/encoder/config.json) and [head/training config](https://huggingface.co/convaiinnovations/laya-typed-decisions/blob/1a793eb568e6718f15941d08f85432581df534e3/rl_agent_config.json). Actual downloaded hashes are in the [probe manifest](../../tools/feasibility/results/artifacts.json). |
| Training implementation | [Typed-decision notebook at the same source commit](https://github.com/NandhaKishorM/laya/blob/4066d5d5fbf08b66c6757ddeedbd797bd7655bc0/notebooks/laya_finetune_typed_decisions_2xT4_kaggle.ipynb), parsed as JSON, never executed in this review. Cell references below are zero-based notebook cell indices and one-based lines in joined `source`; cells have no IDs. Notebook SHA256 `6b81f290bbd213008d3e79c80d207b9abc1d1b5ab0a23f3f4bd9a289611433ed`. |
| Foundry feasibility | [Rust fixture at `6cc234e`](https://github.com/dwbimstr/context-foundry/blob/6cc234e5549ff9b7bc4df658079e70a188656af5/tools/feasibility/decision.rs), [result scope](../review/feasibility.md) and [executable fingerprints](../../tools/feasibility/results/executables.json). Reference used Transformers 5.17.0; Rust used tch 0.24.0/LibTorch 2.11.0. Scratch parity is not a shipped trainer. |
| Actual numerical reference and Rust binding | Transformers 5.17.0 tag resolves to commit [`856157a2f3e9594954310df18fdccc31ffddebe9`, ModernBERT module](https://github.com/huggingface/transformers/blob/856157a2f3e9594954310df18fdccc31ffddebe9/src/transformers/models/modernbert/modeling_modernbert.py): `ModernBertMLP`, `ModernBertRotaryEmbedding`, `ModernBertAttention`, `ModernBertEncoderLayer`. tch 0.24.0 crate metadata identifies commit [`df4c0fdfa37593e6d131b45a015ed41a4adbd5a9`](https://github.com/LaurentMazare/tch-rs/tree/df4c0fdfa37593e6d131b45a015ed41a4adbd5a9); [`src/nn/optimizer.rs`](https://github.com/LaurentMazare/tch-rs/blob/df4c0fdfa37593e6d131b45a015ed41a4adbd5a9/src/nn/optimizer.rs) and [`src/tensor/safetensors.rs`](https://github.com/LaurentMazare/tch-rs/blob/df4c0fdfa37593e6d131b45a015ed41a4adbd5a9/src/tensor/safetensors.rs) own the library optimizer/serialization surfaces. The product owns model integration, not autodiff. |
| Independent embedding subsystem | Nemotron/MLX source and artifacts are separately pinned in [009](../../specs/009-optional-semantic-retrieval/spec.md) and the probe manifest. Publisher [`nemotron3_embed_mlx.py`: `load`, `encode`](https://huggingface.co/mlx-community/Nemotron-3-Embed-1B-BF16-4bit/blob/d0408b94c50fc327b6ea37dce7409c51e020a4d8/nemotron3_embed_mlx.py), Rust [bridge fixture](../../tools/feasibility/mlx.rs). The decision head is neither the embedder nor an automatically enabled reranker. |

Source facts below come from reading the pinned checkout. Linked upstream tests identify
regression intent; they were **not run** in this review. No model, notebook or service
was executed now. Earlier model runs have their separate recorded scope. Upstream
features do not automatically become Foundry requirements, and a source link does not
prove an upstream or Foundry runtime claim.

### Upstream drift recorded 2026-10-03

Upstream `main` has moved from the reviewed commit to
[`fa9a2a7070b1789912a49ae24603bbfb1a78b001`](https://github.com/NandhaKishorM/laya/tree/fa9a2a7070b1789912a49ae24603bbfb1a78b001)
(committed 2026-10-02). Every link in this map is commit-qualified, so each still
resolves to the reviewed 4066d5d5 source, and that commit stays the review scope. Only
two facts were read at the newer commit, from a clean local checkout; no upstream test,
notebook or inference was run, and no other row was re-reviewed:

- The pretrained checkpoint pin is unchanged: upstream
  [`laya/revisions.py` line 30](https://github.com/NandhaKishorM/laya/blob/fa9a2a7070b1789912a49ae24603bbfb1a78b001/laya/revisions.py#L30)
  still pins `convaiinnovations/laya-typed-decisions` to
  `1a793eb568e6718f15941d08f85432581df534e3`, the revision Foundry pins.
- Upstream now documents an over-confidence defect in its shipped calibration
  ([`answer_confidence` and the note after it, `laya/common.py` lines 664–703](https://github.com/NandhaKishorM/laya/blob/fa9a2a7070b1789912a49ae24603bbfb1a78b001/laya/common.py#L664-L703)):
  the shipped `choice:11+` bucket temperature 0.1006 multiplies logits about tenfold.
  Upstream issue #394 was closed by commit
  [`3d762c4`](https://github.com/NandhaKishorM/laya/commit/3d762c4fdb758cc866e77ec70768ccbdf613f65b)
  with per-option-count abstention thresholds. Foundry's guard is its own: contract v4
  refuses a candidate carrying an inherited `temperature_by_options` or a fitted
  temperature below 0.5, with that value as the 013 T002 fixture (L05 below).

## Source map

The L identifiers are lookup labels in this document, not new task IDs or approval stages.

| ID / ecosystem responsibility | Upstream implementation and regression references | Foundry disposition and existing acceptance owner |
| --- | --- | --- |
| L01 — typed state/question/options | [`common.py`: `serialize_state`, `render_options`, `build_sequence`](https://github.com/NandhaKishorM/laya/blob/4066d5d5fbf08b66c6757ddeedbd797bd7655bc0/laya/common.py#L33-L146); [`Agent._check_question`, `_to_internal`, `_encode_state`](https://github.com/NandhaKishorM/laya/blob/4066d5d5fbf08b66c6757ddeedbd797bd7655bc0/laya/agent.py#L556-L677); [criteria normalization tests](https://github.com/NandhaKishorM/laya/blob/4066d5d5fbf08b66c6757ddeedbd797bd7655bc0/tests/test_criteria_normalization.py) | **Retain:** joint candidate-conditioned encoding, stable labels and exact rendered-input identity. **Adapt:** bounded core-composed string state (query, graph coverage, top-3 lexical locators) and fixed first family; refuse oversize instead of upstream truncation. 013 T001 owns token/marker/Unicode/malformed/option-order tests. **Implemented (013 T001):** `decision_model::Renderer::render`, tested ID-for-ID against `tests/fixtures/learning/render.json` by `renderer_reproduces_the_pinned_upstream_fixture_exactly` (limits refuse where upstream truncates). |
| L02 — encoder, choice head and masks | [`DecisionModel.__init__/forward`](https://github.com/NandhaKishorM/laya/blob/4066d5d5fbf08b66c6757ddeedbd797bd7655bc0/laya/common.py#L149-L216); [`collate_items`](https://github.com/NandhaKishorM/laya/blob/4066d5d5fbf08b66c6757ddeedbd797bd7655bc0/laya/common.py#L395); [single-option/head tests](https://github.com/NandhaKishorM/laya/blob/4066d5d5fbf08b66c6757ddeedbd797bd7655bc0/tests/test_decision_model.py), [head checkpointing tests](https://github.com/NandhaKishorM/laya/blob/4066d5d5fbf08b66c6757ddeedbd797bd7655bc0/tests/test_head_checkpointing.py) | **Retain:** pretrained ModernBERT, type embedding, transformer head, per-option marker scoring and masks. Batch one initially; no false claim of padding/batch parity. 013 T002 owns numerical/gradient/train-eval acceptance. |
| L03 — loading and compatibility | [`agent.py: _verify_compatibility`](https://github.com/NandhaKishorM/laya/blob/4066d5d5fbf08b66c6757ddeedbd797bd7655bc0/laya/agent.py#L93-L135), [`build_model`](https://github.com/NandhaKishorM/laya/blob/4066d5d5fbf08b66c6757ddeedbd797bd7655bc0/laya/common.py#L252-L275), [`revisions.py`](https://github.com/NandhaKishorM/laya/blob/4066d5d5fbf08b66c6757ddeedbd797bd7655bc0/laya/revisions.py), [revision tests](https://github.com/NandhaKishorM/laya/blob/4066d5d5fbf08b66c6757ddeedbd797bd7655bc0/tests/test_revision_pinning.py) | **Retain and tighten:** exact weights/tokenizer/config/shape identity, SafeTensors, local-only assets; pinning/digest validation required, not opt-in. Explicit starting-checkpoint mapping only; float16 checkpoint tensors upcast to float32 with the dtype in the model-function identity. 013 T002/T003 verify incompatible/missing/tampered artifacts and restart. |
| L04 — real adaptation objective | Notebook cell 8 lines 100–183 (optimizer groups and loss); [`proper_reward`, `td_lambda_targets`](https://github.com/NandhaKishorM/laya/blob/4066d5d5fbf08b66c6757ddeedbd797bd7655bc0/laya/common.py#L278-L322); [training-helper tests](https://github.com/NandhaKishorM/laya/blob/4066d5d5fbf08b66c6757ddeedbd797bd7655bc0/tests/test_training.py) | **Adapt, not parity:** first Foundry recipe is supervised cross-entropy/AdamW head adaptation. Upstream notebook adapts encoder AND head, with noisy-logit reward plus soft cross-entropy, accumulation and two-GPU DDP. Full encoder adaptation remains retained; multi-GPU/GRPO/trajectory targets need demonstrated task need, not an automatic port. 013 T002 must state exactly what trains. |
| L05 — probability, calibration and abstention | [`answer_confidence`, `confidence_from_probs`, temperature helpers](https://github.com/NandhaKishorM/laya/blob/4066d5d5fbf08b66c6757ddeedbd797bd7655bc0/laya/common.py#L325-L388); [`Agent._decode_answers`](https://github.com/NandhaKishorM/laya/blob/4066d5d5fbf08b66c6757ddeedbd797bd7655bc0/laya/agent.py#L759-L814); [confidence tests](https://github.com/NandhaKishorM/laya/blob/4066d5d5fbf08b66c6757ddeedbd797bd7655bc0/tests/test_confidence.py), [calibration persistence tests](https://github.com/NandhaKishorM/laya/blob/4066d5d5fbf08b66c6757ddeedbd797bd7655bc0/tests/test_calibration_persistence.py) | **Retain:** calibrated answer probability, full option distribution and explicit abstention. **Adapt:** one named confidence (`max(p)`) and one fitted temperature for the single supported family; no inherited bucket/language override, and a candidate with an inherited `temperature_by_options` or a fitted temperature below 0.5 is refused (the #394 lesson above). 013 T002/T003 verify fit→export→reload→threshold behavior. |
| L06 — typed/structured results | [`Agent._decode_answers`](https://github.com/NandhaKishorM/laya/blob/4066d5d5fbf08b66c6757ddeedbd797bd7655bc0/laya/agent.py#L759-L814), [`structured.plan_from_json_schema`, `_project`, `decide`](https://github.com/NandhaKishorM/laya/blob/4066d5d5fbf08b66c6757ddeedbd797bd7655bc0/laya/structured.py#L123-L271); [structured tests](https://github.com/NandhaKishorM/laya/blob/4066d5d5fbf08b66c6757ddeedbd797bd7655bc0/tests/test_structured.py) | **Retain as extension targets:** variable-choice, boolean and ordinal outputs. Current v4 accepts only retrieval-route choice. Select one real consuming workflow, bounded schema and gold labels before enabling another family. Raw expected score, modal level and boolean probability are distinct outputs; no free-form generator or generic schema compiler is implied. |
| L07 — evaluation and regression | [`Dataset`, evaluator types, `EvalReport`, `evaluate`](https://github.com/NandhaKishorM/laya/blob/4066d5d5fbf08b66c6757ddeedbd797bd7655bc0/laya/evals.py#L28-L362); [evaluation tests](https://github.com/NandhaKishorM/laya/blob/4066d5d5fbf08b66c6757ddeedbd797bd7655bc0/tests/test_evals.py) | **Retain:** per-case results, calibration diagnostics and declared slices against a baseline. Foundry additionally requires task-group separation, current consent, explicit comparator inclusion and checked task economics; enabling the policy needs equal correctness and lower total provider tokens from 003's usage import. Errors may not disappear from the denominator. T002/T003 own evaluation/real-consumer proof. |
| L08 — continuous rounds and publication | Notebook cell 6 `build_training_item`; cell 8 lines 88–97 (calibration holdout), 200–263 (save/calibrate/export); [calibration persistence tests](https://github.com/NandhaKishorM/laya/blob/4066d5d5fbf08b66c6757ddeedbd797bd7655bc0/tests/test_calibration_persistence.py) | **Own explicitly:** approval→freeze→fit→calibrate/evaluate→publish→select/restart→feedback→next round. Foundry's correction/withdrawal/group-history/replay/atomic-publication/rollback contract is not claimed to come ready-made from Laya. T001/T004 must demonstrate two real rounds. **Implemented (013 T001):** `learning::prepare`/`learning::check` — consent, correction, withdrawal, group history, replay and `no_new_data`, tested in `tests/learning_data.rs`. |
| L09 — long inputs | [`Agent.predict_long`](https://github.com/NandhaKishorM/laya/blob/4066d5d5fbf08b66c6757ddeedbd797bd7655bc0/laya/agent.py#L956-L1056); [long-input tests](https://github.com/NandhaKishorM/laya/blob/4066d5d5fbf08b66c6757ddeedbd797bd7655bc0/tests/test_predict_long.py), [truncation tests](https://github.com/NandhaKishorM/laya/blob/4066d5d5fbf08b66c6757ddeedbd797bd7655bc0/tests/test_truncation_direction.py) | **Defer automatic decision windowing:** v4 refuses excess input with named fallback. Re-enter if bounded task state loses required evidence; record windows/provenance and calibrate aggregate behavior. A most-confident window is not a calibrated whole-document answer. Nemotron's document partitioning stays in 009. |
| L10 — many-option shortlisting | [`shortlist_choice`, `predict_shortlist`](https://github.com/NandhaKishorM/laya/blob/4066d5d5fbf08b66c6757ddeedbd797bd7655bc0/laya/shortlist.py#L29-L110); [shortlist tests](https://github.com/NandhaKishorM/laya/blob/4066d5d5fbf08b66c6757ddeedbd797bd7655bc0/tests/test_shortlist.py) | **Defer for two options:** if a real larger candidate set requires it, evaluate shortlist recall and state which candidates were excluded. Probabilities after shortlisting are conditional on kept options. Do not silently add embedding work or call this passage reranking. |
| L11 — observation, hooks and model lifecycle | [`PredictContext`/hook events](https://github.com/NandhaKishorM/laya/blob/4066d5d5fbf08b66c6757ddeedbd797bd7655bc0/laya/hooks.py#L22-L86), [`aggregate_usage`](https://github.com/NandhaKishorM/laya/blob/4066d5d5fbf08b66c6757ddeedbd797bd7655bc0/laya/hooks.py#L404-L408), [`Router` load/evict/route](https://github.com/NandhaKishorM/laya/blob/4066d5d5fbf08b66c6757ddeedbd797bd7655bc0/laya/router.py#L279-L457); [hook tests](https://github.com/NandhaKishorM/laya/blob/4066d5d5fbf08b66c6757ddeedbd797bd7655bc0/tests/test_hooks.py) | **Preserve outcomes through simpler ownership:** one supervised worker, ordinary bounded diagnostics and explicit selection; no hook framework, multilingual model fleet or auto-eviction service. Local encoder token counts do not equal provider-billed tokens. T003 plus 003 accounting/deployment own these boundaries. |
| L12 — action/escalation output | [`DecisionModel.forward` action branch](https://github.com/NandhaKishorM/laya/blob/4066d5d5fbf08b66c6757ddeedbd797bd7655bc0/laya/common.py#L199-L216), notebook cell 8 line 173 | **Not an authorization controller:** the inspected objective contributes `0.0 * act.sum()`, no task-learning signal to that branch (optimizer decay is not a learned escalation policy). V4 does not train/serve it. Future escalation prediction needs its own externally labeled objective/calibration; core permissions and spend limits remain deterministic. |

## Details a reviewer must not conflate

**Confidence:** upstream choice/score `confidence` is normalized entropy, while
`answer_confidence` is the winning calibrated probability. They are different scales.
For `[0.8,0.2]`, the latter is 0.8 and entropy confidence is about 0.278. Foundry gates
on unrounded answer probability only. The current Laya structured-details wrapper
reads `confidence`; a Rust port must not inherit that field merely because it is named
confidence. Calibration is an empirical property on held-out data, not a guarantee
conferred by taking `max(p)`.

**Temperature persistence:** upstream bucket values can override type values. Its
notebook explicitly deletes inherited `temperature_by_options` when fitting only type
temperatures. Foundry's single-family recipe stores exactly one newly fitted scalar;
there is no ambient checkpoint override, and a candidate that carries inherited
`temperature_by_options` or a fitted scalar below 0.5 is refused. Upstream's shipped
`choice:11+` value 0.1006 (#394) shows why a sharpening temperature is not a safe
default. Test new candidate export/reload against an old checkpoint with conflicting
bucket values, including the threshold boundary.

**Training parity:** the notebook is evidence for what Laya actually trains. Its
cell 8 source SHA256 is `0e0dcd71e2cb2b073130c417f619f949e8d5273e6caee729da9606ad2e45889f`.
Do not describe Foundry's supervised, frozen-encoder first recipe as a port of that
notebook's training algorithm. We reuse pretrained decision behavior and own the
simpler fitting loop, whose usefulness still requires evaluation. Extending encoder
training or typed tasks must preserve the same data/selection lifecycle.

**Typed values:** upstream ordinal `score` is the probability-weighted expected level;
the structured projection uses a modal level when probabilities are present. Boolean
`noul` carries P(true), not the probability that every downstream action is safe.
Any new Foundry consumer must pick and test the required interpretation. A family
change creates a new renderer/label/calibration identity; equal hidden dimension is
not compatibility. Existing held-out groups and consent cannot be discarded to make
that change appear successful.

**Continuous does not mean autonomous:** repeatable new-data rounds, valid base
lineage and safe deployment are required. Runtime callbacks, a scheduler, recursive
self-labeling and a model registry are not prerequisites. A decision/result/usage
observation is not training approval or a correctness label.

## Foundry code ownership and proof links

These are ordinary modules in the same planned crate, not new services. Only the
existing/probe links below point to code that exists today; proposed module names
are implementation targets and become linked to a specific landed revision when built.

| Responsibility | Current evidence | Planned production owner / task |
| --- | --- | --- |
| Exact rendering, tokenizer IDs, tensor model, mask/logit math | [Scratch `attn`, `encoder`, `head`, `reference`](../../tools/feasibility/decision.rs) and [recorded parity limits](../review/feasibility.md) | `src/decision_model.rs`; shared by preparation/training/inference, 013 T001/T002. Tokenization does not load model weights. No separate per-caller renderer. |
| Feedback truth and schema | [Legacy `record_feedback`/`training_examples`](../../src/laya.rs), [store owner](../../src/store.rs); these do not implement v4 | Existing `src/store.rs` owns durable schema; `src/learning.rs` owns v4 validation, dataset lineage and prepare/check, T001. |
| Fit, calibrate, evaluate, artifacts and repeated rounds | [SGD/parity scratch scope](../../tools/feasibility/results/decision-limited.json); not AdamW or two-round acceptance | `src/learning.rs` with numerical model operations in `src/decision_model.rs`, T002/T004. No independent job database. |
| Selection, core fallback, bounded decision reply | [Legacy `decide`/`predict`](../../src/laya.rs) already distinguishes answer confidence; external HTTP fixture is not an owned worker | `src/policy.rs`, T003, called by 003's existing owner. Remove legacy HTTP path after owned acceptance. |
| Process grants, limits, EOF/deadline and cleanup | [Native limit probe](../../tools/feasibility/limit_probe.rs), [pre-exec launcher](../../tools/feasibility/nproc_launcher.rs); not a shipped supervisor | Shared worker/supervisor responsibility under [deployment](../deployment.md), T002/T003/T004. One Rust feature-gated executable, no public model port or backend fleet. |
| Real context/provider economics | [003 contract](../../specs/003-agent-retrieval-context/contracts/adapter-economics.md); no provider-cost result from a model probe | Existing adapter/gateway owner in 003; T003 supplies measured extra decision work. Training costs and actual observed uses remain separate. |

## How to use this map in a review

For a touched learning behavior, cite its L row, pinned upstream symbol/test, owning
013 requirement/task and actual Foundry diff/test. State whether the behavior is
retained, intentionally different or outside this release. Compare observable input,
output and failure semantics; similarity of class names is not evidence. A reference
change updates only the affected mapping/compatibility fixture, not a whole-system audit.

For example, a confidence review follows L05 → `answer_confidence` and calibration
persistence tests → v4 probability rule → T002 export/reload and T003 threshold test.
A token-format review follows L01 → `build_sequence` → shared Rust renderer → T001
exact IDs/markers and cap+1 refusal. A fit change follows L04/L08 and must not mistake
a different objective for pretrained-head or continuous-round equivalence.

Use commit-qualified links, function names and the applicable test. For notebooks
include cell index, source hash and relevant code lines as above. For weights use
artifact revision **and** hashes; a moving `main` URL or model family name is insufficient.
When code lands, replace proposed owner paths with the actual module/symbol and add
the focused acceptance link in the same change. No reference updater daemon, mirrored
repository, duplicate spec system or extra review ceremony is needed.
