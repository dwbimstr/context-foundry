# Spec and task audit

## Native discovery requirement — current documentation pass

2026-10-01. The owner requires Foundry to precede grep/ripgrep in ordinary agent
source discovery. Added 003 FR-008 and its canonical selection/fallback rules,
bootstrap project guidance and actual tool-order acceptance inside existing T001/T003.
Updated architecture, deployment, roadmap, portfolio and contributor guidance.
MCP availability, instruction preference and enforced host routing are distinguished;
the amendment does not implement MCP or imply a supported multi-repository join.

Checked documentation paths/anchors, requirement/task mapping and whitespace.
No product/probe code, dependencies, models, stores, host configuration or upstream
repositories changed. No runtime host task, performance or token-savings acceptance
was executed. Existing graph/neural/learning/gateway readiness boundaries remain.

## Decision-ecosystem traceability — preceding documentation pass

2026-09-29. Added [the source-to-contract map](references/laya-decision-ecosystem.md):
Laya source commit, checkpoint revision, numerical-library versions/commits, functions,
regression tests, notebook cell/hash references, retained/adapted/deferred behavior,
existing proof and proposed Rust owners. All mapped Laya source was inspected at its
clean pinned checkout; notebook JSON was read, not executed. Checked the published
`common.py` bytes against that checkout and resolved the reference Transformers tag.

Made v4 answer-probability/abstention/tie semantics explicit, removed ambiguous private
reply confidence, specified calibration-override rejection and per-case evaluation
diagnostics, and attached source rows to T001–T004. Retained broader typed decisions
as explicit extension targets without claiming that the first choice family implements
them. Corrected stale roadmap/architecture statements about the completed scratch probes.
These are documentation/contracts/reference changes; no product/probe code, model runs,
upstream tests or services changed. Structural/link checks are recorded with the commit;
they do not establish learning or deployment acceptance.

## Executed feasibility — preceding runtime pass

2026-09-29. Owner authorized bounded sandboxed probes and direct main publication.
[The feasibility report](review/feasibility.md) records real Rust/MLX execution,
ModernBERT/head/selected-QKV reference parity, a head update/read-back, native process
limit checks, Rust 1.90 MCP exchange and a vector-index smoke. Product source and root
dependencies are unchanged. Probe sources/locks and public synthetic results are in
`tools/feasibility`; weights, private data and third-party source remain outside Git.

Replaced 013's obsolete vector-head contract/tasks with ModernBERT v4 and propagated
the current disposition through architecture, deployment and portfolio. Complete
model recipe, distributed package, actual host/provider and scale/quality evidence
remain explicit, scoped prerequisites. No full-product readiness or savings claim.

Checks: probe Rust formatting and documentation links/task structure; unchanged
protected product/source/build/license files. The legacy checker has a hardcoded old
013→009 dependency and therefore cannot validate the new semantic dependency graph;
that boundary was reviewed directly. Structural counts are not readiness evidence.
The historical entries below retain their original time and scope.

## Team handoff simulation — preceding paper review

2026-09-29. After pushing `f51e6c6`, traced the fifteen current spec dispositions,
six active workflows and adjacent source owners as a paper implementation handoff.
The [report](review/handoff-simulation.md) distinguishes contract conflicts, missing
decisions and unexecuted integration proofs. It is an in-session review, not code
execution, independent approval or an exhaustive predecessor/source audit.

Corrected remaining Nemotron-vector assumptions in 001/009, architecture, deployment
and the dependency inventory. ModernBERT policy availability follows its own inputs;
009 failure alone does not disable it. Moved combined-policy deadline acceptance to
013's later integration so semantic-only release does not wait for learning. Added
combined model-residency planning to the existing deployment owner. 013's replacement
contract/tasks, real Rust/runtime bridges and packaged profiles remain incomplete.

Inspected the selected publisher's MLX loader at
`d0408b94c50fc327b6ea37dce7409c51e020a4d8` without executing it or obtaining weights.
Its Python load/encode API is not a verified Rust service. No application code, tests,
dependencies, models, corpora, services or host configuration were modified. Only
documentation and Git publication were performed; runtime claims remain untested.
Structural/documentation checks passed: 15 spec files, 276 relative links, nine
heading links and unchanged hashes for 13 source/build/license files. The checker
counts retained superseded task entries and does not prove semantic task readiness;
the handoff report supplies that assessment. `git diff --check` also passed.

## ModernBERT review — current scope correction

2026-09-29. Read Laya's actual joint-input/typed-head implementation, checkpoint
loader and source cells of the fine-tuning notebook at local revision
`4066d5d5fbf08b66c6757ddeedbd797bd7655bc0`. The notebook assigns nonzero learning
rates to encoder and head and backpropagates through both; the earlier small fixed
query-vector head is not an equivalent capability. Details and primary links are
in [learning](learning.md#modernbert-and-the-cost-of-matching-laya--2026-09-29).

Inspected Candle ModernBERT→RoPE→custom-op code at
`5ba5d5b468b5b1df40e82dd3d556987bedeea041`: the Q/K rotary path drops the backward
graph. Inspected Burn's official model catalog and tch's LibTorch/autograd/import
surface. These establish integration options and a concrete training-path gap,
not local gradient correctness, speed, backend/MSRV compatibility or model quality.

The owner clarified ModernBERT **with a decision head** as the target. 013 D001 now
pins trainable parameters and the model/head contract before library selection;
v3 and its four implementation task entries are superseded, retained for review only.
D001 must replace affected clauses before implementation; this is not a build-ready
ModernBERT plan or permission to substitute the earlier query-vector classifier.
Preserved Rust ownership, Nemotron retrieval, consent, deterministic hard budgets,
independent source/gateway releases and the existing task slots without new stages.

This was source/documentation review only: no Laya/notebook execution, model download,
dependency installation, training, service operation or provider request. Production
code and Cargo dependencies are unchanged. Eight preceding planning files were
archived locally with hashes; no archive is public. The following older entries
retain their original scope and do not establish ModernBERT support.

Documentation checks pass for 15 specs and 265 relative links plus seven anchor links;
`git diff --check` passes. The 19 counted task entries include four superseded 013
entries, so this count does not establish implementation readiness. All 13 checked
source/build/license files remain unchanged. No Rust build or model test was run.

## Current amendment — owned Rust learning, bootstrap and economics

2026-09-29. The owner's latest answers select all first-party implementation in Rust
and both context budgeting and optional model-request forwarding/metering. Revised
013 into owned learning (renamed directory), replaced its v2 external-Laya handoff
with v3 feature/head/worker contracts, and added 013 D001 for a real Rust-library and
packaged-isolation proof. Laya stays read-only reference material; its current HTTP
client and legacy format fixture remain unchanged until implementation cutover.

003 now covers explicit bootstrap, connection config, budget boundaries, usage
receipts and a separately accepted Rust gateway task. 001 retains store/packing
ownership; 005 distinguishes artifact import from isolated producer execution;
009 owns the selected MLX runtime's still-unproven Rust bridge and isolation. Updated
architecture, constitution, roadmap, deployment, release and security boundaries.
Reviewed the current active ownership/dependencies and all nine inactive dispositions
for contradictions. This does not rerun or claim exhaustive coverage of 71 predecessor
specs/codepaths. Findings, hypotheses and tradeoffs are in the existing subtraction review.

Read current Rust inference/feedback code and Laya reference material. Checked official
Burn, Linux, Apple and libkrun documentation; consulted OpenAI Docs for Codex provider
configuration, Responses streaming and input counting. Documentation establishes
possible interfaces, not installed compatibility, model quality, OS enforcement or
provider economics. No credentials were read, model/provider calls made, packages
installed, hosts configured, repositories published or Prakarana/Laya state changed.

Documentation consistency checks pass: 15 specs, six active/nine inactive, 19
implementation tasks, three recorded decisions, 33 FR and 21 SC IDs; the explicit
dependency map is acyclic. Relative links/anchors and whitespace pass. All 13 protected
source/test/Cargo/CI/license files match the prior baseline; all 53 files in the new
ignored pre-amendment archive match their captured hashes. The old public Laya-format
fixture is unchanged. Checks use temporary read-only scripts, not product tooling.
No Rust build/test, sandbox test, fine-tuning or gateway integration was executed.

Remaining concrete proofs: 009 model/runtime/bridge/profile; 013 library/MSRV and
train/save/load/predict under an actual packaged jail; 003 gateway host/schema/counting/
stream/tool compatibility; and each selected package's install/upgrade/rollback.
No proposed feature is marked implemented or independently approved. Older entries
below retain their original scope and counts; they do not override this amendment.

Date: 2026-09-28. Scope: all 15 current spec files, every active task, both shared
contracts and the adjacent implementation. This replaces the earlier structural-only
readiness record. Single-session review as requested, not independent counsel.

Earlier follow-up: 009 became an active proposed preparation/retrieval workflow.
The portfolio has six active specs, nine dispositions, eighteen implementation tasks
and two explicit decision tasks. Earlier counts below are preserved as historical
review scope. No new implementation is implied by reactivation.

## Earlier pass: reuse, composition and hidden work

The second requested adjacent-pattern review found seven additional failures in
source/proposed contracts: reused memory revisions, a filesystem replacement race,
learning novelty tied to group novelty, graph cardinality multiplication, status
requiring unperformed tokenization, abandoned inference escaping client bounds, and
serialized handles exceeding their input cap. Updated the existing 001/005/008/009/013
tasks and shared contracts; checked 003 and all nine inactive dispositions against
those changes. Findings and retained tradeoffs are in the existing
[subtraction review](review/subtraction.md).

The JSON bound counterexample was executed in isolation: a valid 4096-byte escaped
path makes an 8353-byte handle, exceeding the former 8192 limit. No Foundry code,
filesystem race, provider admission or learning behavior was executed. Source files,
tests, build files, services, models, private stores and external repositories remain
unchanged. Documentation checks and code/contract reasoning are the evidence for
this pass; implementation acceptance is still outstanding.

Current consistency checks pass: all 15 specs, six active/nine inactive, eighteen
implementation tasks and two recorded decisions, 30 FR/20 SC IDs, 231 relative links
and two anchors. The reviewed dependency map is acyclic; whitespace checks pass.
All thirteen protected source/test/build/license files and archived drafts match
the captured baseline. The checks do not execute the proposed behavior or establish
that all future interactions have been discovered.

## Earlier adjacent-pattern review

Expanded the review from model/chunk examples to initialization, read side effects,
failure isolation, repeated repair/cleanup, artifact identity, real caller reachability,
schema ownership, response snapshots and input truncation. The concrete failures,
source/design evidence, accepted tradeoffs and existing task owners are recorded in
[the subtraction review](review/subtraction.md). Six active specs and their shared
contracts were amended; all nine inactive dispositions remain inactive. No task or
spec count increased. Review/analyze/template prompts now carry the adjacent-boundary
question without imposing a new portfolio review or measurement stage.

Read-only source evidence includes `Engine::open` and CLI dispatch, graph/source/cache
ownership boundaries, and Laya `build_sequence` at the recorded local revision. Its
512-token sequence can truncate state after question/options; identical train/serve
formatting alone would preserve that loss. The planned adapter now rejects oversized
labeled inputs before fitting and returns a named serving fallback before inference.
This is source evidence, not a tokenizer/model execution result.

The amended specs also make staged graph import reachable through the existing MCP
owner, keep source reads/exports usable after a derived-index failure, prevent mixing
compiler artifacts at one source revision, bound repeated repair artifacts and require
exclusive vector purge. These are proposed acceptance contracts, not implemented fixes.
No Rust build/test, compiler/model execution, training, package install, live service
operation, Prakarana/Laya mutation or publication occurred in this pass.

That pass's consistency checks passed: 15 specs, six active/nine inactive, 18 implementation
tasks, two recorded decision tasks, 30 FR/20 SC IDs, 229 relative Markdown links and
two anchors; no cycles in the reviewed task dependency map. All 13 protected
source/test/build/license files and archived drafts match the prior captured hashes.
Whitespace checks pass. These checks verify document structure and preservation;
the semantic findings above come from manual review, not the counting script.

## Coverage and substantive findings

Read every spec: five active workflows and ten Superseded/Deferred dispositions.
Reviewed every task in 001/003/005/008/013, the shared source/response contract and
Laya handoff. Traced current `store.rs`, `ingest.rs`, `graph.rs`, `laya.rs`, CLI/module
entry points and existing core/CLI/protocol tests. In Laya, verified the current commit,
package entry points, sequence/option helpers, Agent preparation/loading, evaluation
parser and training/calibration sections of notebook cell 8. No training code ran.

| Ambiguity/failure found | Concrete correction |
| --- | --- |
| Recovery task could not be reached when index open itself failed | Authoritative-only status/export and explicit restartable derived-index repair, with interruption points and preservation assertions |
| Bounded scan had no bounds or partial-deletion semantics | Fixed source/page/sample limits, last-seen scan ownership, interrupted-sweep behavior and exact counters |
| Shared handles lived under a later adapter spec | Moved contract ownership to 001; 003 consumes it without a reverse dependency |
| Budgeting left escaping, continuation progress and source starvation unclear | Exact emitted-byte boundaries, bounded prefix trials, named errors and first-source packing witness |
| MCP work omitted admission/cancellation and a concrete user task | One active/zero waiting operation, explicit cooperative deadlines and partial writes; a specific checked parser edit in a real host |
| Compiler freshness checked only edge endpoints | Bind compiler output to an immutable input snapshot and whole source revision; test a third-file edit |
| Graph replacement had no defined scope or accepted-empty distinction | Producer/origin-document transaction, reverse-index atomicity, unknown versus empty coverage and explicit import/query bounds |
| Memory tasks lacked API/conflict/deletion behavior | Exact request fields/limits, revisions, retry/conflict semantics, logical forgetting and source/memory separation |
| Feedback CLI could not write beside the agent owner | Planned MCP observation route, explicit offline training permission/preparation, no competing writer |
| Training rules used undefined sufficiency and selection language | Typed dataset/lineage limits, execution floors, fixed recipe, exact metric/eligibility rules and incomplete/rejected outcomes |
| Laya task combined nonexistent training and serving prerequisites | Four separately verifiable tasks; real checkpoint loading and second round cannot be replaced by JSONL/protocol fixtures |
| Dataset first-manifest field could create a self-hash cycle | Compute dataset identity from input file/policy hashes before group-map/manifest hashes |
| Deferred stubs could be mistaken for executable plans | Concrete re-entry evidence for 007/009/010/014; no tasks/contracts in any inactive bundle |

The behavior pass tightened contracts; the ownership pass checked adjacent code;
the consistency pass checked consumers, dependencies, failure/restart and acceptance.
More words are not evidence of better architecture: no new product worker, scheduler,
protocol or storage layer was implemented. Substantial safeguards stay with the five
workflows rather than becoming new feature specs or measurement stages.

## Remaining prerequisites, explicitly owned

001 D001 is now resolved by the ecosystem pass below: retain redb/Tantivy within the
one-pending-table/one-rebuild-marker bound; recovery acceptance is still unexecuted. 003 T001
pins an MSRV-compatible SDK, and T003 needs an installed authorized host. 005 T001 needs
an actual permitted producer; T003 needs a declared corpus/hardware profile with numeric
limits before running. 013 needs permitted local model/tokenizer/data, hardware and
separately selected external Laya changes. These are execution/selection prerequisites,
not vague success criteria or evidence that every task can run immediately.

## Follow-up: external references and neural ownership

The session raised an additional boundary: mentioning or reading another repo must
not silently enroll it in source reconciliation, watching or training. Tightened
the existing 001/003 scope and 013 learning contracts and their acceptance cases;
At that pass 007/009 remained deferred dispositions, with no new task, service or
registry. The later neural-preparation correction below supersedes 009's deferral.
Tests now specified cover wrong-root refusal without source changes, launch-directory
independence, foreign handles, separate-store reconciliation, opaque external paths
and no implicit cross-store training data. These tests have not been implemented.

Source verification: Foundry `ingest.rs::sync` binds the canonical root and disables
symlink traversal; `store.rs::bind_workspace` rejects a different root; `laya.rs::predict`
sends the query string and does not resolve paths or record feedback. In Prakarana,
`workspace.cpp::scan_run` refuses a root change before mutation; its comment records
the prior umbrella-store deletion failure. `pas_serve.cpp::pas_scan_cycle` passes the
configured root to that scan. `session_ingest.cpp::index_episode` stores scrubbed
session bytes under a session URI, and `enrich_worker.cpp::process_embed` reads stored
record bytes and caches the result by content hash/model tuple. Thus session evidence
about a file is not evidence that the file itself is indexed or fresh. These are
source conclusions, not a newly executed live-session or cross-repo experiment.

The history tool returned `query-gen transport` after about 90 seconds; no answer
from that call was used. The checked-in M26 result was read directly: on its deliberately
constructed vocabulary-gap corpus, sole-neural delivered 35/120 hits and fused dense
delivered 22/120 at k=5. This supports preserving semantic retrieval and examining
composition losses; it does not establish current live quality or select Nemotron.
The configured neural output was 768, distinct from the 256 lexical dimension.
Official model cards linked in architecture confirm model-specific output sizes.
No model download, training, new benchmark or running-service change occurred.

## Checks and proof limits

### Neural preparation correction

The previous deferral preserved model identity but omitted a preparation lifecycle.
Read predecessor specs 032/042/071, the M42 result, neural worker/cache interfaces and
the reuse contract test; checked Foundry's chunking, context strategy and 003's owner
contract. Preparation on the single MCP operation slot would block queries, while
rebuilding document vectors on ordinary index repair would repeat expensive inference.
009 now owns a cache in the selected transactional library, a derived search index,
one bounded inference worker, source-owner publication, partial coverage and explicit
pause/resume. Source ownership still never expands from an outside path mention.

Three proposed tasks cover durable preparation, actual semantic context and progressive
agent use. 009 D001 requires exact permitted artifacts/runtime/library, schemas and
numeric bounds before code; it is not complete. Query/ranking-only changes explicitly
reuse document vectors. Laya is outside the preparation dependency chain, and its
outcome evidence must name retrieval readiness rather than treat cold-index failure
as a learned-strategy label. Existing prototype code and public fixtures are unchanged.

This is a scoped design correction, not another all-spec or live-runtime audit. The
M42 figures are historical; no new throughput/latency estimate, model download,
training or background worker ran. Dimension facts were checked against the primary
Google/NVIDIA model cards linked in architecture. Documentation checks include the
new task dependencies and links; runtime acceptance remains unexecuted.

Follow-up consistency results: 15 spec files checked, six active/nine inactive;
18 implementation tasks and two decision tasks; 30 FR and 20 SC IDs; 218 relative
links/two anchors resolve. The task dependency graph has no cycles. All 13 protected
source/test/build/license files match the earlier baseline. Whitespace checks pass.
These counts establish document consistency only, not neural implementation readiness.

### Selected embedding checkpoint

The owner supplied the exact `nvidia/Nemotron-3-Embed-1B-BF16` model page and selected
it over the earlier Llama Nemotron suggestion. Updated 009, architecture and portfolio:
initial 2048-dimensional output, explicit prompting/normalization and separately
licensed weights. D001 no longer owns a cross-model choice; it still must pin artifact
revision, runtime/index integration, schemas and bounds. Verified the official model
card, including the distinction between model input ceiling and serving limits.
No model download/inference or hardware compatibility claim was made. Existing
documentation checks were rerun; source/test/build/license files remain unchanged.

The owner then supplied `mlx-community/Nemotron-3-Embed-1B-BF16-4bit` for local MLX
execution. Read its card and bundled loader without executing it. The loader has a
4096-token truncation default and can download a missing model path implicitly;
009 now requires preflight length checks and a pinned existing local directory.
Quantization/loader identities remain distinct from upstream BF16. The publisher's
reported throughput is not local validation. Updated the selected local profile in
architecture/portfolio/roadmap and reran documentation checks; no new runtime ran.

### Document chunking correction

Read the predecessor's current `src/internal/raw_projector.cpp::project_spans`,
`manthan.cpp::stage_raw_source_delta`, `enrich_worker.cpp::process_embed` and
`embed_neural_gemma.cpp` input guards. Source projection admits hard cuts from 1024
bytes, probes a soft cut at 3200 and forces a boundary at 5000, with no overlap.
The embedding adapter independently limits its first single input to at most 2400
bytes and can halve it again on HTTP 500. This demonstrates two different text
boundaries and possible loss of a record's tail from the model input; it does not
establish how often that occurs in the current live corpus or its measured effect.
Foundry's `store.rs::chunks` also uses fixed 2048-byte blocks in its prototype.

Revised 009 to separate authoritative source, embedding unit and delivered evidence:
whole documents within a selected tested token limit, grouped sections/paragraphs
for larger sources, no overlap or duplicate document/section vector hierarchy.
The loader's 4096 default is no longer a permanent architectural size. Larger limits
require one bounded check on the selected runtime; no weights were downloaded/run.
Source coordinates and input-key cache reuse survive storage-boundary/offset changes;
whole-unit edits and repartitioning can still incur new inference. Context distinguishes
the matched unit from returned bytes and labels an unlocalized prefix as a preview.
T001/T002 now cover complete source coverage, tail evidence, edit reuse and actual
input/output costs. A filename hit alone cannot pass span-delivery acceptance.

Aligned architecture, the shared 001 response contract, 005 graph seed handling and
013 outcome provenance. No new numbered spec, task, service or release gate was added.
This is source/design evidence; document consistency checks do not prove the MLX
runtime's usable long-input limit, semantic quality or preparation-time improvement.

The check results below describe documentation consistency, not runtime acceptance.
The historical crosswalk still inventories all 71 predecessor specs; this turn did
not re-audit every predecessor implementation. Original review coverage included
full reads and scoped sampling, not exhaustive requirement verification.

No Context Foundry source/test/Cargo/CI file changed in this pass. No Rust build/test,
Prakarana/Laya mutation, live-store probe, host configuration, model training or
publication was performed. Earlier first-slice runtime evidence remains in
[validation](validation.md). Original and immediately preceding plan drafts are
preserved in ignored local archives. OMP file layout is present; host command
registration/execution is untested. Public Laya examples remain format-only fixtures.

## Earlier task-pass consistency checks

Latest ecosystem follow-up: reread all 15 current specs and both shared contracts;
reviewed adjacent Rust source and sampled local Laya interfaces. The detailed findings
and all-ID dispositions are in [the subtraction review](review/subtraction.md).
Updated existing contracts for deterministic ranking versus optional Laya, one
embedding profile, shared provider deadlines, graph-availability bypass, truthful
graph criteria, and mixed strategy labels within a task group. 001 D001 is resolved
as an architectural decision, not a new test result. No spec/task count increased.

The two public format rows now describe definition/reference exploration rather than
resolved callers. Their JSON shape is checked; no new Laya parser/model acceptance
is claimed. All production source/test/build/license files remain unchanged. No
model execution, service operation, training, package install or publication occurred.
That earlier pass's checks passed: 15 spec files, six active/nine inactive, 18 implementation tasks,
two recorded decisions (one resolved), 30 FR/20 SC IDs, 224 relative links and two
anchors; no dependency cycles. All 13 protected implementation/build/license files
match the prior baseline. Whitespace and both revised format rows' JSON/choice shape
pass. These checks do not execute the proposed Rust behavior or real Laya integration.
The following older counts describe their original pass, not the current portfolio.

- 15/15 current spec files inspected; five active specs contain 15 implementation
  tasks plus one explicit decision task. The ten inactive dispositions contain no
  executable plans/tasks/contracts.
- 24 functional requirement IDs and 17 acceptance IDs map to active task behavior;
  each task states dependencies, scope, outcome, verification and review/cutover.
- Task dependencies are acyclic; shared context ownership is 001→003, with no
  lower-level dependency on building the adapter. Learning data can precede graph
  acceptance, while claims about useful learned graph selection cannot.
- All Git-visible relative Markdown paths and heading anchors resolve. The first
  check found a stale historical anchor and two inconsistent task field labels;
  these were fixed and the check rerun successfully.
- The 71-entry historical crosswalk and ten command/playbook pairs still match their
  inventories. No inactive bundle gained implementation work during this refinement.
- SHA-256 comparison to the start-of-turn snapshot confirms all 13 protected source,
  test, build/CI and license files are unchanged. Archived prior drafts also match
  their captured hashes. Public format fixtures remain unchanged.
- `git diff --check` and whitespace checks over tracked/untracked public Markdown
  pass. These are documentation checks, not the new specs' runtime acceptance.

Checks used a temporary read-only local Python script, not new product tooling or a
new mandatory gate. The spec review did not declare future hardware, model quality,
SDK integration, graph scale or learned savings already validated.
