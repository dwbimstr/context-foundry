# Owned learning contract v3

Status: Proposed, 2026-09-29; owned by [013](../spec.md). This replaces the Laya v2
handoff. The legacy `eval.public.jsonl` is retained only as a historical Laya format
fixture, not an input or acceptance fixture for this model. No Laya module, tokenizer,
server, model or training notebook is required by the new contract.

## Roles and exact first model

`foundry-policy-head-v1` consumes a 2048-element float32 normalized query embedding
from the selected 009 query feature function. The head is Linear(2048,64) → ReLU →
Linear(64,2), with bias in both layers and no dropout. Logit/label order is graph=0,
search=1. Output is softmax(logits / calibrated temperature). This is a trainable
131,266-parameter decision model; its weights occupy 525,064 float32 bytes before
artifact metadata. The encoder remains frozen. Trainable head weights are real
model parameters; head fine-tuning is not advertised as encoder fine-tuning.

Use one maintained Rust ML library under 013 D001. Record initialization algorithm,
library/version, seed and serialized initial tensor hash in the effective recipe;
do not handwrite autodiff, copy GRPO or train a replacement foundation model. CPU
float32 is the first backend. A later architecture/backend changes recipe identity
and requires its own compatibility evidence, not a runtime backend-selection framework.

Serving reuses the exact query vector already computed for semantic retrieval.
It never embeds a query merely to consult the policy. Missing query features, explicit
strategy or no current supported graph skips the worker. The deterministic 001 rule
remains the fallback. A query-function change (weights, tokenizer, prefix, normalization,
dimension) requires compatible newly prepared features/head; equal dimensions are not
compatibility. Document chunking/ranking changes alone need workflow re-evaluation,
not invented feature incompatibility when query vectors are unchanged.

## Feedback and consent

Required object: `{task_id,task_group_id,query,correct_strategy,label_source,
label_evidence,allow_training,rights_ref}`. IDs nonblank UTF-8 <=256 bytes; query
nonblank <=4096 bytes; strategy graph/search; label_source operator/task_checker;
evidence/rights nonblank <=1024 bytes; whole object <=16 KiB. Reject unknown/null/
duplicate fields, invalid types and oversized input before mutation. References are
opaque assertions; never open their paths/URLs or infer labels from clicks/model output.

Example ID is SHA-256 of compact UTF-8 JSON `[task_id,query]`. Correcting that pair
replaces label/consent in one transaction. Identical input is idempotent. Once used,
moving its group is `group_changed`; repair lineage explicitly rather than moving
held-out examples into train. Feedback uses 001's store/schema owner.

MCP feedback may record `allow_training:false`; true is `training_approval_required`.
The operator/trusted-checker CLI under exclusive ownership supplies actual permission
and rights for the exact row. Tool availability is not proof of human approval.
No automatic transcripts, 008 memories, retrieved source bodies or other store's
feedback. Quoted outside content requires permission for that content too. Training
permission, retrieval admission and adapter usage reporting are separate choices.

Withdrawal excludes future preparation/selection, not old exports or fitted weights.
Check permission at preparation, immediately before training and at selection. A
withdrawal during a job needs an explicit stop; an already selected inference worker
needs disable/restart. There is no live revocation monitor or claim of untraining.

## Commands and publication

- `foundry learning prepare --out DIR --policy FILE [--parent MANIFEST]`: freeze
  approved rows under sole store ownership, then prepare missing query features using
  the explicitly configured 009 worker. A live MCP owner is `store_busy`; no second
  writer or hidden service stop. No read transaction is held during model work.
- `foundry learning check --manifest FILE`: verify current permission/labels, hashes,
  group history and selected base contributions. Returns a validated manifest digest;
  no fitting. Repeat before training/selection; the operator must prevent a competing
  feedback writer during the check/handoff. Manual handoff is not atomic revocation.
- `foundry learning train --input MANIFEST --policy FILE --out DIR`: supervise the
  owned isolated Rust worker, report completion/rejection and exit. No network/download.
- `foundry learning select --candidate DIR --out CONFIG`: validate eligibility,
  lineage and artifact identity; create a config file, never start a service or replace
  a live model. The operator installs this file and restarts the existing owner.

Existing output is `output_exists`. Write only a newly owned partial sibling, fsync
its files, write completion manifest last, rename on the same filesystem and fsync
the parent before success. Lost response means inspect/verify that output; do not
overwrite or automatically rerun fitting. No valid final manifest means incomplete.
Cleanup removes only verified owned partial files. Artifacts go outside admitted
source roots or in the store's excluded private area; preparation rejects a proposed
output under an admitted source path as `output_in_source_root`.

## Dataset, feature reuse and grouping

Manifest v3 fields: `schema:3`, `recipe:"foundry-policy-data-v1"`, `dataset_id`,
`workspace_id`, `parent_manifest_sha256` (null initially), `policy_sha256`,
`base_candidate_sha256` (null for seeded initialization), `feature_function_sha256`,
`dimension:2048`, file entries `{name,sha256,bytes,rows}` and group counts by split.
Files: `train.jsonl`, `calibration.jsonl`, `evaluation.jsonl`, `groups.jsonl`,
`features.safetensors`. Use a maintained Rust SafeTensors reader/writer, not executable
pickle or provider-supplied code. Tensor `query_features` is row-major float32 [N,2048].
Each dataset row has feedback fields, `id`, `allow_training:true`, `feature_row` and
`feature_input_sha256`; replace query with `state` containing its exact query text.

Rows sort by ID with one LF after compact JSON. One feature row per current example,
including duplicate query text across permitted identities; preparation may compute
identical rendered queries once. Reuse a parent's exact query-function/input feature
only if the current row is still permitted. No persistent automatic query-history
cache; feature files belong to explicit learning artifacts. Corrected labels can reuse
unchanged features but invalidate a base containing the old label. Missing/invalid
features stop preparation by name, never become zero vectors or silently dropped rows.
Query inputs follow 009's exact prefix/token-limit rules; no inherited 512-token limit.

Manifest <=1 MiB; all files combined <=256 MiB; <=100,000 examples and historical
groups; JSONL row <=16 KiB. The combined byte cap also bounds the feature matrix;
100,000 is not a promise that 100,000 vectors fit. Page metadata in batches of 128;
spill sorting into owned scratch. Validate sizes, finite floats, dimensions, indices,
unique IDs, duplicate JSON keys, hashes and relative basenames before fitting. Paths
have no symlinks, absolute components or `..`. Feature identities are checked against
the profile, not inferred from a filename or endpoint.

dataset_id = SHA-256 of compact JSON `[workspace_id,parent_manifest_sha256,
policy_sha256,feature_function_sha256,train_sha256,calibration_sha256,evaluation_sha256,
features_sha256]`. Group history can then name this ID; no self-referential hashes.
groups.jsonl stores group ID, split, first dataset ID, example IDs and normalized-query
fingerprints, including historical withdrawn groups without retaining their text.
Missing ancestor/group metadata is `lineage_missing`; growth beyond bounds refuses.

Split: first eight SHA-256(group-ID UTF-8) bytes as big-endian u64 modulo 10:
0=evaluation, 1=calibration, 2..9=train. Parent assignments never move. Normalize
duplicate fingerprints by CRLF→LF and collapsed/trimmed ASCII whitespace, without
case folding. Same normalized query across groups or contradictory current labels
is `duplicate_conflict`, including held-out parent fingerprints. Different queries in
one group can have different labels. This is not semantic duplicate detection.

Require >=20 train, >=10 calibration and >=20 evaluation groups, both labels in each.
These are lifecycle floors, not statistical proof. Insufficient data refuses; never
move groups to fill splits. Novelty means an eligible training row's exact content/
label/permission identity is absent from the valid base's contributing set, including
new queries in an old group. Check base permission first: corrected/withdrawn inherited
contributors are `base_permission_changed`, not `no_new_data`.

Select all new training rows and unchanged permitted base-contributing replay rows
in increasing SHA-256(compact JSON `[seed,id]`) order, at most one replay row per new
row. Exclude held-out groups and duplicate selections. Zero new rows under a valid
base is `no_new_data` with no checkpoint. A permitted clean base can relearn current
rows while preserving historical group splits. Every result records inherited and
new fitting/calibration identities. Never choose a partial/rejected base implicitly or
repeat the same input/base/recipe to hunt for a favorable seed.

## Fitting, calibration and completion

Run policy fields: `v:1`, `recipe:"foundry-policy-head-v1"`, `feature_function_sha256`,
`base` (`{kind:"seeded"}` or `{kind:"candidate",path,sha256}`), `rights_ref`, `seed`, `max_steps`,
`wall_seconds`, `memory_bytes`, `output_bytes`, `cpu_threads`, `isolation_profile`,
`resource_enforcement`, `selection`, `incumbent` (`{kind:"disabled"}` or
`{kind:"candidate",path,sha256}`). Candidate paths are explicit local directories,
not URLs, <=4096 UTF-8 bytes; digests lowercase 64-hex. Profile names <=128 bytes.
Resource enforcement maps memory, cpu, output and process_count to `hard` or
`supervised`, with no omitted entries; a profile cannot silently weaken a requested
hard bound. Wall-time cancellation/owned-tree cleanup is always supervised and tested.
The job permits one worker process and zero descendants; library threads stay within
the declared CPU-thread setting. A hard CPU profile limits aggregate CPU time to that
many cores as well; thread configuration alone does not prove a CPU quota. Memory/output
bounds mean resident bytes and private output/scratch bytes, respectively. The platform
profile records how each is enforced, including whether filesystem quotas are available.
All required; reject unknown/null fields. Explicit bounds: wall 1..7200 seconds,
steps 1..1,000,000, memory 256 MiB..8 GiB, output 1 MiB..1 GiB, CPU threads 1..4,
seed u64, rights_ref nonblank <=1024 bytes. Selected OS limits must be enforceable;
`isolation_unavailable`/`resource_limit_unavailable` refuses the job before fitting.

Recipe: CPU float32, cross-entropy, AdamW lr=0.001, betas=(0.9,0.999), epsilon=1e-8,
weight decay=0.01, gradient norm clip=1, batch=32, four epochs, no scheduler/dropout.
Cap updates at min(max_steps,4*ceil(train_rows/32)); use actual final batch length.
Shuffle each epoch with the pinned seeded Rust RNG recorded by D001. Initial weights
use the pinned library initialization; subsequent rounds load the exact valid base.
No encoder gradients, distributed training, reinforcement-learning loop or optimizer
resume. Library numerical determinism is measured, not assumed across machines.

Calibration uses only calibration rows. Test temperatures
exp(ln(0.1)+i*ln(100)/80), i=0..80; choose minimum mean negative log likelihood,
lower temperature on exact ties. Compute log-softmax stably; nonfinite output or
empty data is `calibration_failed`, never default temperature. This bounded scalar
fit replaces the inherited notebook/LBFGS machinery. Report calibrated and raw scores.

One output-root OS lock rejects overlap as `learning_busy` before allocation. Worker
inputs are read-only features/labels/base; outputs are bounded tensors/reports in its
private directory. The supervisor validates the full approved dataset, then supplies
only feature-row indices, numeric labels and split membership in bounded private job
scratch. Query text, rights references and label-evidence prose are not granted to the
training worker. Feature vectors remain private data; they are not anonymization.
Supervisor enforces the [deployment contract](../../../docs/deployment.md), owns its
process tree, retains at most 64 KiB of scrubbed stderr and kills only its own workers
on deadline/EOF. Diagnostics cannot include dataset text, credentials or vector contents.
No auto-retry. Output full, crash, failed validation or jail failure cannot publish a
successful manifest or alter selected configuration.

Checkpoint entries are sorted `[relative_path,sha256]` pairs for head tensors and
effective recipe/config including fitted temperature and feature identity. Candidate
digest hashes compact JSON `[input_manifest_sha256,policy_sha256,checkpoint_entries]`.
Reports refer to that digest and remain outside its hash to avoid a cycle. Final
manifest names version, complete status, digests, byte sizes, reports and eligibility.
Parent verifies safe tensor shapes/hash and requests actual isolated worker load→predict
before final publication; checkpoint loading must not escape the worker's grant set.

## Evaluation and activation

Freeze threshold (proposed 0.8), minimum coverage (0.5), minimum accepted accuracy
(0.9), maximum macro accuracy drop (0) and critical slices before fitting; fractions
finite [0,1]. Selection policy supplies these numbers explicitly. Critical slices
name group IDs and minimum macro accuracy; absent/empty referenced groups refuse.
Compare candidate, compatible eligible incumbent when supplied, and the exact 001
deterministic rule on identical evaluation rows. An incompatible/missing requested
incumbent refuses preflight rather than silently removing the comparator.

Per-group accuracy is correct rows / group rows; macro accuracy weights groups equally.
Coverage = accepted rows / all rows; accepted accuracy = correct accepted / accepted;
zero accepted fails. Fallback-inclusive accuracy applies the deterministic route on
abstention/errors. Eligibility requires no allowed macro drop against either comparator,
coverage/accepted floors, all critical slices and all identity/permission/runtime checks.
Eligibility permits an isolated trial; it is not evidence of workflow improvement.

Report counts and paired group differences. A numerical improvement claim requires a
predeclared uncertainty method, fresh confirmation groups and its actual result; it
is not an additional requirement for a functional learning lifecycle. Reused held-out
groups are regression evidence. Never change policy after viewing evaluation results.

Normal policy remains off until 013 T003's actual checked agent tasks justify its
latency/cost under the same corpus, graph, retrieval profile and budget. Count feature
preparation and training separately from per-query work; report amortization with the
actual number of uses, not an invented lifetime. Full request/provider costs follow
003's adapter boundary. Label-score gains do not prove task success or savings.

## Private inference and selection

Config v1: `{v:1,enabled:false}` or `{v:1,enabled:true,candidate_path,candidate_sha256,
feature_function_sha256,recipe_sha256,threshold,evaluation_report_sha256,isolation_profile}`.
Read once at owner startup; validate all fields/artifacts/eligibility before loading.
Threshold, feature/recipe identities and report digest must match the evaluated
candidate; editing the threshold is not a way to reuse eligibility. The selected
isolation profile must be supported and pass its startup enforcement check.
Invalid config is `policy_config_invalid`: deterministic retrieval still starts.
No ambient model lookup, latest-directory selection or network download.

Owner launches one sandboxed worker, loads the immutable checkpoint through an exact
read-only grant, and exchanges private length-prefixed JSON frames over inherited
pipes. Prefix is little-endian u32; maximum frame 64 KiB, checked before allocation.
Request: `{v:1,request_id,candidate_sha256,feature_function_sha256,features}`; features
is exactly 2048 finite float32 values. Reply: `{v:1,request_id,candidate_sha256,
feature_function_sha256,choice,confidence}`. IDs nonblank <=128 bytes, digests lowercase
64-hex, confidence finite [0,1]; reject unknown/null/duplicate fields. No source paths,
queries, credentials, arbitrary commands or output locations in prediction requests.

Only one request active, zero waiting. Prediction ceiling is min(100 ms, remaining
003 read time); startup/load has a separate 5-second cap and is outside requests.
These are failure bounds, not measured latency promises. On timeout, malformed output,
wrong identity or death, terminate the owned worker, mark policy unavailable and use
deterministic routing while the request deadline allows. Do not auto-restart on every
query; explicit owner restart can retry. Late replies cannot cross request identities.
The head never changes a hard budget or grants an operation; core policy remains final.

Selection explicitly checks current lineage and writes a new config. The user/operator
installs it and restarts the existing owner; rollback restores the retained prior
artifact/config, or disables learning if none existed. No live model switch or registry.
CLI exits: 0 completed/no_new_data (named in result), 2 invalid/preflight input,
3 busy, 1 runtime/training/artifact failure, 130 cooperative cancellation. Bounded
errors follow 001; no raw dataset/query text in diagnostics.
