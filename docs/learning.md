# Owned continuous learning

Status: Proposed, 2026-09-29. [013](../specs/013-owned-learning/spec.md) owns the
design and acceptance. The implementation is still the original Rust prototype;
it does not yet train or launch isolated model workers.

Foundry owns its policy model, Rust trainer, feature/data contract, worker protocol,
calibration, checkpoint loading, deployment and rollback. Laya supplied a useful
reference for small typed decisions with abstention. It is not the product runtime,
an API to emulate or a repository contributors must install/edit.

The owner clarified the target: **ModernBERT with a decision head**, using joint
state/question/candidate inputs. Nemotron remains the separate retrieval encoder.
Contract v4 selects frozen-encoder/head adaptation first, using Rust tch/LibTorch.
Encoder adaptation is retained behind its own complete acceptance; head fitting
does not establish full encoder fine-tuning.

The earlier 2048→64→2 classifier on Nemotron query vectors is a superseded proposal,
not an equivalent implementation of this target. Its v3 data/IPC contract and task
details are replaced by v4 and retained only in Git history. Its query-vector reuse
and small-head latency limits do not define the ModernBERT path. The subsequent
[feasibility pass](review/feasibility.md) ran real scratch model/gradient probes;
production learning and complete deployment acceptance remain unimplemented.

The continuous loop is explicit and repeatable:

1. A user/trusted checker supplies labels and permission for exact examples. Ordinary
   context use, usage receipts, memory and outside path mentions create no training data.
2. Freeze grouped data and exact joint inputs. Prepared inputs bind the full state,
   question, ordered options, tokenizer and recipe. Reuse encoder outputs only when
   the encoder is frozen and their identity matches; encoder tuning requires a live
   gradient path. A new example in an old group is new work; groups fix split membership.
3. Train/calibrate/evaluate an owned Rust checkpoint in a worker with restricted access
   and declared limits. A malformed/partial result cannot become a selected model.
4. Compare with deterministic routing and a compatible incumbent. A rejected candidate
   is a valid completed learning round. Actual task/economic benefit is a separate claim.
5. Explicitly select an eligible artifact and restart the store-owning process. Keep
   the prior config/checkpoint for rollback, then repeat on new examples and bounded replay.

[Contract v4](../specs/013-owned-learning/contracts/learning-loop.md) defines joint
inputs, exact pretrained architecture, rights, grouping, fitting, IPC and artifacts.
[Deployment](deployment.md) defines jails, conditional libkrun use, packaging, startup,
upgrade and removal. The historical review below explains the integration cost;
the later feasibility pass selected tch 0.24.0/LibTorch 2.11.0 and a concrete first
recipe. Full model/MSRV and distributed-package acceptance remain D001 inputs.

The model never authorizes repository enrollment, tool execution, provider spend or
larger budgets. [Adapter economics](../specs/003-agent-retrieval-context/contracts/adapter-economics.md)
owns those deterministic boundaries. Learning remains off until actual task benefit
justifies it; source/agent releases can ship earlier. Model preparation and training
costs are reported separately and amortized only over observed use.

The existing `src/laya.rs`/`--laya-port` path is a legacy prototype interface scheduled
for removal in 013 T003. [Legacy notes](laya.md) describe what currently exists; they
do not override the owned design. No automatic Laya checkpoint conversion or
dual-provider router is planned. V4 pins permitted starting weights and exact
head/checkpoint mapping; base ModernBERT alone does not supply trained decisions.
Preserve old feedback without silently granting new permissions.

## ModernBERT and the cost of matching Laya — 2026-09-29

Historical source review, preceding [the executed feasibility pass](review/feasibility.md).
Its recommendations and unexecuted statements below describe that review's boundary;
v4 and the later evidence own the current disposition.

**Rust can train models; ready-made ModernBERT fine-tuning and Laya-equivalent behavior
are separate questions.** The preceding design understated the capability gap by
making the small head concrete before examining the full reference architecture.
The owner subsequently confirmed ModernBERT with a decision head as the target;
the small vector head remains a comparison only. All first-party code remains Rust; this
does not prohibit a separately packaged third-party native ML library.

### What the inspected Laya implementation actually does

Reviewed local revision `4066d5d5fbf08b66c6757ddeedbd797bd7655bc0`: `common.py`
`build_sequence`, `DecisionModel`, `build_model` and reward/calibration helpers;
`agent.py` checkpoint loading; and source cells of the typed-decision training notebook.
No notebook, repository code, weights or training job was executed.

- Input includes the state, question/instructions and rendered option descriptions
  in one token sequence. Option marker positions are scored after a bidirectional
  encoder, type embeddings and a two-layer transformer head. Options are not merely
  fixed output indices. [Reference model code](https://github.com/NandhaKishorM/laya/blob/4066d5d5fbf08b66c6757ddeedbd797bd7655bc0/laya/common.py#L94-L216)
- Its published typed-decisions configuration identifies ModernBERT-large and a
  1024-token sequence. That is a different workload from a pooled Nemotron vector.
  [Checkpoint configuration](https://huggingface.co/convaiinnovations/laya-typed-decisions/blob/main/rl_agent_config.json)
- Notebook source cell 8 puts encoder and head parameters in separate AdamW groups,
  with nonzero learning rates for both, and backpropagates through the model. It uses
  noisy-logit policy-gradient plus soft cross-entropy, accumulation, mixed precision
  and separate calibration. This inspected recipe is not frozen-encoder/head-only
  fitting. Its `0.0 * act.sum()` adds no learning signal for the action head; that
  head's existence alone does not establish a trained spend/escalation controller.
  [Training notebook](https://github.com/NandhaKishorM/laya/blob/4066d5d5fbf08b66c6757ddeedbd797bd7655bc0/notebooks/laya_finetune_typed_decisions_2xT4_kaggle.ipynb)

The benefit to preserve is learned, calibrated decisions over actual task inputs.
We need not port multilingual routing, every integration, GRPO, DDP or custom CUDA
kernels merely because Laya contains them. Conversely, a newly initialized two-class
head has none of Laya's pretrained decision behavior simply because it is trainable.
ModernBERT base weights alone do not supply its trained typed-decision head either.

### Rust routes and remaining ownership

| Route | Existing building blocks | Foundry would still own | Disposition |
| --- | --- | --- | --- |
| Superseded query-vector head | Ordinary Rust autodiff/optimizer operations; 009 features | Fixed-label data, small model, calibration, worker lifecycle | Comparison only; does not meet the clarified ModernBERT plus decision-head target |
| Frozen ModernBERT plus a trained typed head | Candle has ModernBERT forward code and checkpoint loading primitives | Exact joint-input formatting, trained-head architecture/weight mapping, head gradients, output semantics and parity | Worth exploring when task inputs need candidate-conditioned decisions; encoder is an additional runtime cost |
| ModernBERT encoder adaptation through Candle/Burn | Tensor/autodiff infrastructure; Candle forward architecture | Verified differentiable model path, train/eval behavior, checkpoint mapping, backend performance and training recipe | Substantial model integration; not an off-the-shelf fine-tuner established by this review |
| Rust training through `tch`/LibTorch | PyTorch native autograd, optimizers and SafeTensors loading exposed to Rust | ModernBERT/typed-head module definition or a verified reusable implementation, tokenizer parity, bounded training and native packaging | First route to investigate if actual encoder fine-tuning is required; all first-party code can remain Rust |

Concrete source checks:

- Candle revision `5ba5d5b468b5b1df40e82dd3d556987bedeea041` has
  [ModernBERT forward code](https://github.com/huggingface/candle/blob/5ba5d5b468b5b1df40e82dd3d556987bedeea041/candle-transformers/src/models/modernbert.rs#L73-L76).
  Its Q/K rotations call `rope`, which uses
  [`apply_op3_no_bwd`](https://github.com/huggingface/candle/blob/5ba5d5b468b5b1df40e82dd3d556987bedeea041/candle-nn/src/rotary_emb.rs#L537-L579).
  That helper constructs a result with
  [`BackpropOp::none()`](https://github.com/huggingface/candle/blob/5ba5d5b468b5b1df40e82dd3d556987bedeea041/candle-core/src/custom_op.rs#L154-L165).
  Thus the inspected path does not propagate Q/K gradients through RoPE. Other
  parameters can still change: loss reduction or any changed weight is insufficient
  to prove correct encoder training. `rope_slow` provides an ordinary-operation
  alternative to investigate; swapping it alone is not a validated training port.
- The same attention path computes dense QK matrices and applies a local mask.
  Native model context length is not a practical memory guarantee for that path;
  test actual sequence/batch limits rather than inheriting an 8192-token promise.
- Burn provides training infrastructure, but the inspected official
  [model catalog](https://github.com/tracel-ai/models/tree/86f1628d992f4fa80691fd6310586ef5e06a26d1)
  has BERT/RoBERTa rather than a verified ModernBERT training recipe. This is a bounded
  catalog observation, not proof that no community implementation exists. Generic
  weight import does not implement a missing model architecture.
- [`tch`](https://github.com/LaurentMazare/tch-rs/tree/4227b89b72059b0651ff83a38637693574e0a2d6)
  wraps C++ LibTorch; Rust owns application/model/training logic. It is not a Python
  `transformers.AutoModel` loader. Using it avoids writing an autodiff engine but
  adds a native library/version/ABI/distribution obligation. The inspected repository
  does not supply the ModernBERT model port. No backend was installed or tested here.

### KISS decision and effects on the rest of Foundry

Retain Nemotron for retrieval. ModernBERT is the proposed separate decision encoder, not
a replacement embedding profile and not a reason to rebuild document vectors.
Changing a policy encoder invalidates that policy's prepared features, not Nemotron's
document cache. Candidate-conditioned encodings depend on the complete formatted
state/question/options, their order, tokenizer and weights. They cannot reuse a
query-only cache entry merely because the state text matches.

The earlier 131k-parameter head is superseded. For the clarified ModernBERT plus
decision-head target, select one
state/question/candidate decision family and verify a compatible pretrained model
and Rust head path first. Preserve pretrained behavior where permitted; retraining
a randomly initialized replacement is a different data/research commitment. Check
checkpoint terms independently; no third-party model/code is relabeled MIT here.

Prefer frozen-encoder/head adaptation as the first bounded hypothesis if it meets the
chosen task; this still adds an encoder and a nontrivial head port. If encoder updates
are actually required, evaluate `tch` with a pinned LibTorch rather than committing to
a custom Rust training stack. This is a recommendation to test, not a backend selection
or a claim of numerical parity. CPU-first proves semantics; GPU throughput, Apple
support, packaging and jail access need separate evidence for the advertised target.
Do not add both Candle and Burn plus a tensor interchange service by default.

013 D001 now targets the complete ModernBERT plus decision-head path. Declare which parameters
train and which input/decision family is supported; compare one selected path against
fixed reference outputs and gradients before promising a full fine-tuner. No broad
Laya clone, general model backend framework or new numbered spec is selected.

The v3 data/IPC contract, 100 ms prediction ceiling, 5-second startup cap and CPU jail
describe only the small head. A transformer path must replace the affected input,
identity, tensor-size, cache, time/resource and artifact clauses explicitly before
implementation. Model inference cannot run merely to enforce a gateway token cap.
Hard budgets, source truth, consent, selection and rollback remain outside learned
confidence. A ModernBERT failure cannot block source/MCP/gateway operation.
