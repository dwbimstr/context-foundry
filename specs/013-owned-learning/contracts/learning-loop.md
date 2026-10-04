# Owned learning contract v4

Proposed implementation contract, 2026-09-29; spec-pass decisions recorded 2026-10-03
(state composition, float16 loading, timeout handling, calibration refusal and the
enablement rule). Replaces the vector-only v3 contract;
Git history retains it. Owned by [013](../spec.md). This is ModernBERT with a decision
head. Nemotron embeddings, Laya services and the historical `eval.public.jsonl` are
not inputs. [Feasibility evidence](../../../docs/review/feasibility.md) distinguishes
working probes from outstanding implementation and package acceptance.
The [source-to-contract map](../../../docs/references/laya-decision-ecosystem.md)
pins upstream modules/tests and intentional differences. This contract is the first
search/graph consumer of the decision ecosystem, not a claim of full Laya parity.

## One model and one decision

Recipe `foundry-modernbert-choice-v1`: ModernBERT-large plus the pretrained two-layer
choice head, type embedding and scorer from `convaiinnovations/laya-typed-decisions`
revision `1a793eb568e6718f15941d08f85432581df534e3`. Download/rights approval is explicit;
weights and upstream code are not distributed under Foundry's MIT license. This is a
specific weight mapping, not a generic Laya artifact import or runtime dependency.
Base encoder weights alone are not an equivalent pretrained decision model.

First implementation: Rust `tch` 0.24.0 with LibTorch 2.11.0, CPU float32, one example
per batch. Do not introduce a tensor engine or additional ML backend. The probe used
a separate Python reference for numerical comparison; production model/training code
is Rust and does not import Laya or Transformers. Dependency/MSRV/package acceptance
still applies. No claim of turnkey AutoModel compatibility or full Laya training parity.

Encoder: hidden 1024, 28 layers, 16 heads of 64, gated GELU intermediate 2624;
layer 0 has no attention pre-normalization; full attention every third layer, local
attention inclusive distance <=64 otherwise; global/local RoPE theta 160000/10000;
normalization epsilon 1e-5. Head: two pre-normalized transformer encoder layers,
16 heads, feedforward 4096 with ReLU, type embedding row `choice=0`, marker pooling,
then LayerNorm→Linear(1024,1024)→GELU→Linear(1024,1). Preserve biases and checkpoint
names/shapes exactly. Reject missing/extra/shape-invalid tensors except the explicitly
unused `act_head.*` and reference `temperature` tensors. Do not execute checkpoint code.
The checkpoint's float16 tensors are upcast to float32 at load; computation stays
float32, and the source dtype plus that upcast belong to `model_function_sha256`.

Initial supported adaptation freezes ModernBERT and updates the two head layers,
choice type embedding and scorer. Keep encoder in eval mode. Head dropout is 0.1 in
training and disabled in evaluation; seed initialization, ordering and dropout. Other
type rows are frozen; expose only row 0 to the optimizer so weight decay cannot alter
unused rows. `act_head` is unused and never trained. This is **head adaptation
on joint ModernBERT inputs**, not encoder fine-tuning. Encoder adaptation remains a
retained capability: enable only after its own full gradient/update/resource fixture
passes and task evidence requires it. The small Q/K gradient probe is not that release.
No runtime menu of training backends or hidden change of trainable parameters.

## Exact input and identity

Supported family `retrieval-route-v1` has question `Choose a retrieval strategy.` and
two options, stable IDs `search`, `graph`, with descriptions `find source text` and
`follow symbol relationships`. The input is a UTF-8 `state` and the ordered two option
IDs. Neither option may be omitted/duplicated. The core composes `state` from three
parts, LF-separated in this order: the admitted query text; `graph: <coverage>`, the
current graph coverage status (005's `complete` or `partial`); and up to three lexical
locator lines `<path> <kind> <qualified name>` naming the top three delivery units of
001's two-tier lexical ranking, without handles or source text (a block or unnamed
unit renders its kind alone, as in 001's v2 labels). A feedback row stores the exact
state the policy saw. State is data, never a path to open. Explicit user strategy,
no current graph, busy/unavailable policy or insufficient time uses deterministic
routing without a model call. Graph availability is checked by the core before state
is built; the coverage line is model input, not the availability gate. No automatic
transcript or source-document inclusion.

Render with pinned tokenizer: `[CLS] choice question: Choose a retrieval strategy.
[SEP] [MASK] <option0>: <description0> [MASK] <option1>: <description1> [SEP] <state>
[SEP]`. Spaces above describe components: tokenize instruction and each space-prefixed
option separately with `add_special_tokens=false`; insert special IDs explicitly.
Replace literal mask-token strings in state with one space before tokenization.
Retain original state and rendered-input digest. Record both marker positions.
Maximum 1024 tokens including specials, header <=256, each rendered option <=48;
**refuse**, never truncate, on any limit. State is also capped at 16 KiB, but only as a
guard checked before tokenization; the 1024-token total is the binding limit, so a
state under 16 KiB can still be refused. No padding for batch size one. Future batched
implementation needs padding/mask parity before using it. Option permutation changes
input identity; labels map by stable option ID, never by an assumed fixed index.

Pin tokenizer JSON/config hashes, all special IDs, render version (including the state
composition above), architecture, starting weights, checkpoint dtype and its float32
upcast, and trainable parameter set in `model_function_sha256`. Prepared
rows store exact IDs and markers, plus `input_sha256 = SHA256(compact JSON
[family,state,ordered_option_ids])`. Tokenized arrays are checked against the exact
renderer on preparation/read-back. No 2048-dimensional feature matrix, query-vector
reuse or persistent hidden-state cache. Recompute frozen encoder output per example
initially; add caching only if measured repeated preparation warrants its storage.

## Feedback and permission

Row: `{task_id,task_group_id,family,state,option_ids,correct_option_id,label_source,
label_evidence,allow_training,rights_ref}`. Family/inputs follow the preceding section.
IDs nonblank <=256 UTF-8 bytes; label_source `operator` or `task_checker`; evidence and
rights nonblank <=1024 bytes; whole row <=24 KiB. Reject unknown/null/duplicate fields,
invalid types and oversized data before mutation. Evidence/rights references are opaque
assertions, not instructions to open paths or URLs. Clicks/model predictions do not
supply correctness labels. All state content needs the asserted training rights.

Example ID is SHA256(compact JSON `[task_id,input_sha256]`). Correction of that ID
replaces label/consent transactionally; same input is idempotent. Used groups never
move between splits. Changing state/options creates a new example, not a mislabeled
correction. MCP may record `allow_training:false`; true requires the trusted operator
CLI and is refused as `training_approval_required` over MCP. Legacy feedback lacks
these exact inputs/rights and stays exportable but ineligible; no automatic upgrading.

Check current consent/labels at preparation, immediately before fitting and at
selection. Withdrawal excludes future use; it does not erase existing exports or
untrain weights. A running job requires explicit stop; a selected model requires
disable/restart. No hidden revocation watcher. Feedback uses 001's sole schema owner.

## Dataset and repeat rounds

Commands: `learning prepare --out DIR --policy FILE [--parent MANIFEST]`,
`learning check --manifest FILE`, `learning train --input MANIFEST --policy FILE
--out DIR`, and `learning select --candidate DIR --out CONFIG`. All operator commands
require exclusive store ownership where they read feedback/permission. No competing
writer or long read transaction during model work; no implicit shutdown of MCP.
Manual permission handoff is not an atomic live revocation mechanism.

Manifest: schema=4, recipe, workspace_id, dataset_id, parent_manifest_sha256 (nullable),
model_function_sha256, policy_sha256, base_candidate_sha256 (nullable), files and
split group counts. Each file entry is `{name,sha256,bytes,rows}`. Files are
`train.jsonl`, `calibration.jsonl`, `evaluation.jsonl`, `groups.jsonl`; rows include
feedback, example ID, input digest, token IDs and markers. Names are fixed basenames;
no symlinks, absolute components or `..`. Manifest <=1 MiB, combined files <=256 MiB,
<=100000 rows/groups, JSONL rows <=48 KiB. Refuse on bounds; never drop excess data.
Sort by example ID, compact JSON and trailing LF. Page feedback by 128, spill sorting
to owned scratch. Artifact hashing uses exact bytes, not reserialized JSON.

dataset_id = SHA256(compact JSON `[workspace_id,parent_manifest_sha256,
model_function_sha256,policy_sha256,train_sha256,calibration_sha256,evaluation_sha256]`).
Group history names this ID after computing it, avoiding a self-referential digest.
Split by first eight SHA256(group-ID) bytes interpreted big-endian modulo 10:
0 evaluation, 1 calibration, 2..9 train. Ancestor assignments never move.
Require >=20 train, >=10 calibration and >=20 evaluation groups, both correct-option
labels in each; these are lifecycle floors, not statistical significance.

Duplicate fingerprint is SHA256 of the family, whitespace-normalized state and
options sorted by stable ID. Normalize CRLF→LF and collapse/trim ASCII whitespace,
without case folding. Same fingerprint across groups or conflicting current labels
is `duplicate_conflict`, including historical held-out fingerprints. This only finds
exact normalized duplicates; related tasks must share a group. Persist group/split,
example IDs and fingerprints for withdrawn groups without retaining withdrawn text.
Missing ancestor metadata is `lineage_missing`; no implicit split reset.

Valid base contributions must still have identical permission, inputs and labels;
otherwise `base_permission_changed` before checking novelty. New eligible training
rows include new inputs in old groups. Select all new rows plus up to one unchanged
permitted base replay row per new row in increasing SHA256(compact JSON `[seed,id]`)
order; no held-out rows. Zero new rows with a valid base is `no_new_data`, no fitting.
A clean base may relearn allowed rows while preserving historical split assignments.
Record inherited/new fitting and calibration contributions. No seed hunting on the
same input/base/recipe, implicit rejected base or claim of fresh improvement from
reused held-out groups.

## Fitting, artifacts and evaluation

Run policy v2 pins recipe/model/base/seed, AdamW learning rate 1e-4, betas 0.9/0.999,
epsilon 1e-8, weight decay 0.01, global gradient norm clip 1.0 and cross-entropy over
the two marker logits. Batch one; no accumulation. Base is the pinned initial artifact
or an exact permitted accepted candidate. Each epoch visits the selected training rows
in seeded order; `max_steps` bounds updates (1..1000000), record actual completed steps.
Reject nonfinite input/logits/loss/gradients/weights immediately. No exact optimizer
resume promise; repeated rounds start a new optimizer from the permitted base weights.

Policy also pins wall_seconds (1..7200), memory_bytes (4..8 GiB), output_bytes
(128 MiB..2 GiB), cpu_threads (1..4), isolation_profile and hard/supervised enforcement
for memory, CPU, output and process count. Requested enforcement cannot be downgraded.
One worker, zero descendants. Training runs offline with online model workers stopped;
no training/retrieval overlap scheduler. App Sandbox alone did not enforce zero
descendants; the pre-exec hard-process-limit probe did. Production training remains
unavailable until the complete installed profile passes its acceptance.
These are requested resource ceilings, not proven hardware capacity.

Fit one scalar temperature on calibration rows only by deterministic grid search over
`T = k/20` for integer k=10..60 (0.5..3.0 inclusive), selecting lowest mean negative
log likelihood calculated from stable log-softmax, not rounded/underflowed probabilities;
ties select lower temperature. Nonfinite/empty calibration refuses. Evaluate raw logits
and calibrated probabilities, never alter model weights on held-out data.
Use the fitted scalar from the candidate manifest; reject any identity mismatch.
It is the only calibration authority for this family; no upstream type/bucket/language overrides.
Candidate validation refuses a candidate that carries an inherited
`temperature_by_options` (or any other per-option or bucket temperature) or whose
fitted scalar is below 0.5. Laya's shipped `choice:11+` temperature of 0.1006, an
over-confidence defect (Laya issue #394, recorded in the source map), is the fixture
for this refusal.
Publish it with the candidate and verify the same probabilities after export/reload.
Changing the fitted scalar invalidates the evaluation report/config identity.

For finite logits `z_i` and fitted positive temperature `T`, compute stable softmax:
`p_i = exp((z_i - max(z))/T) / sum_j exp((z_j - max(z))/T)`. Select the maximum-probability
stable option ID; exact ties select lexicographically smallest UTF-8 option ID.
`answer_confidence = max(p)`; accept exactly when unrounded `answer_confidence >= threshold`.
Below threshold, the core uses deterministic routing with reason `policy_abstained`.
No normalized-entropy value, `act_head` probability, rounded display value or model
suggestion substitutes for this rule. At `[0.8,0.2]` and threshold 0.8, accept; at
`[0.79999,0.20001]`, abstain. Temperature fitting does not itself prove calibration
on a new task distribution; evaluation establishes its measured behavior.

Selection policy predeclares threshold, coverage floor, accepted-accuracy floor,
maximum macro-accuracy drop and critical group slices; fractions finite [0,1]. Defaults
for the initial trial: 0.8, 0.5, 0.9 and 0 respectively. Compare deterministic routing,
candidate and explicitly requested compatible incumbent on identical rows. Missing
requested incumbent or empty critical slice refuses; never drop a comparator.

Macro accuracy averages per-group accuracy. Coverage is accepted/all rows; accepted
accuracy is correct/accepted (zero accepted fails). Fallback-inclusive accuracy uses
deterministic routing on abstention/failure. Eligibility requires declared floors,
critical slices and no excess macro drop against either comparator. Report counts and
paired group differences. Eligibility permits a trial, not an improvement claim.
The evaluation report retains per-case example/group ID, expected option, both logits
and calibrated probabilities, selected option, answer confidence and accepted/fallback
outcome, without duplicating raw state. Abstention/error rows never vanish from counts;
ordinary unavailable-model rows use deterministic routing in fallback-inclusive
accuracy and count as uncovered. Nonfinite/malformed model output or artifact failure
invalidates candidate eligibility. Report mean negative log likelihood and 15-bin ECE
as diagnostics on valid predictions (first bin [0,1/15], then (lo,hi]); give denominators
and error counts. These diagnostics do not add a separate slow release gate.

Normal policy stays off. It may be enabled only after a comparison on the same checked
agent tasks, with provider usage from 003's `foundry usage import`, shows equal task
correctness and lower total provider tokens with the policy than without it. Count
preparation/training, context bytes/tokens and actual provider usage separately, and
report added latency. Do not claim money savings when usage/pricing is unknown.

Candidate contains schema-4 manifest, head.safetensors, exact base encoder/tokenizer
identities, recipe, calibration, contribution IDs and evaluation report. Head is float32;
no pickle, executable code or ambient loader lookup. Final manifest binds all files and
base identity. Validate shapes, finite values and save/load logits before publication.
No full encoder duplicate in every head round. Explicit matching encoder assets are
required at inference; a missing asset is a named failure, never a network download.

Create a new owned partial sibling; fsync files, manifest last, rename same filesystem,
fsync parent. Existing destination is `output_exists`; partial/failed output is not
eligible. Reject output under an admitted source root (`output_in_source_root`). Remove
only owned partial files. Lost response means verify output before repeating work.

## Serving, selection and rollback

Config v2 is disabled or pins candidate_path/hash, model_function_sha256, report hash,
threshold and isolation_profile. Validate at owner startup. No ambient latest directory,
live auto-promotion or threshold edit to reuse eligibility. Invalid config names
`policy_config_invalid`; baseline retrieval still starts. Explicit select writes a new
config; operator installs/restarts. Rollback restores prior immutable config/artifacts
or disables learning. Preserve source, memory, graph, feedback and semantic cache.

Private inherited pipes; little-endian u32 length followed by strict JSON, maximum
64 KiB checked before allocation. Request `{v:2,request_id,candidate_sha256,
model_function_sha256,family,state,option_ids}`; reply `{v:2,request_id,
candidate_sha256,model_function_sha256,input_sha256,choice,probabilities,answer_confidence}`.
`probabilities` has exactly `search` and `graph` keys, both finite in [0,1], sum within
1e-6 of 1. Choice must match the maximum/tie rule above; answer confidence must match
that probability within 1e-6. The core validates the reply and compares the unrounded
maximum of the supplied probability vector to its threshold (not the separately
reported confidence field); the worker cannot declare an action authorized.
IDs <=128 bytes, digests lowercase 64 hex; reject unknown/null/duplicate fields, including the ambiguous legacy
`confidence` field. No arbitrary commands, paths or token-budget grants. This refines
the still-unimplemented v4/IPC-v2 proposal; no deployed reader migration is implied.

One active prediction, zero waiting. Load ceiling 30 seconds outside requests;
prediction ceiling min(2000 ms, remaining 003 deadline). These are supervised failure
bounds, not latency promises. Busy returns deterministic fallback before dispatch.
A prediction timeout returns the deterministic fallback with reason `policy_timeout`
for that request only; the policy stays enabled. The slot stays occupied until the
timed-out work actually completes or the worker is terminated, so later requests
receive busy fallback; capacity is never released while work survives. A late reply to
a timed-out request is discarded, never delivered. Only a valid reply within the
ceiling resets the consecutive-timeout count; busy fallback and other fallbacks are
not predictions and neither count nor reset it. Three consecutive prediction timeouts
terminate the owned worker tree and mark the policy unavailable until explicit
restart. A malformed or wrong-identity reply or a dead worker remains a terminal
failure: owned-tree termination and policy unavailable until explicit restart.
Owner EOF terminates worker. Late replies cannot cross identities. Exact tokenizer
preflight runs before encoder; oversized state/token input uses named baseline fallback.
Combined model residency must pass deployment's aggregate budget before enabling both
009 and 013; separate process limits do not prove this. No automatic unload/reload or
restart loop.

CLI exits: 0 completed/no_new_data (named), 2 invalid/preflight, 3 busy,
1 execution/artifact failure, 130 cooperative cancellation. Diagnostics are bounded and
exclude state/dataset contents. Source and hard budget decisions always stay in the core.
