# Bounded pilot v2 — unfunded draft

Draft date: **2026-11-11**. **Not frozen; execution locked.**
`execution_authorized=false`; `execution_ready=false`.

This is the draft selected by owner decision 1 on 2026-11-06. It owns the v2
design; [T006](../spec.md#t006--validate-retrieval-repairs-in-real-coding-tasks)
owns acceptance and the [handoff](../../../docs/resume-context-ecosystem.md#current-status--2026-11-06)
owns current status. It does not reuse v1's credential, budget, launch manifest or
four-category usage exception. [V1](bounded-pilot-v1.md) remains byte-for-byte frozen.
No provider request, credential access, training or execution is authorized here.

## Outcome and claim boundary

Measure verified coding outcomes on several harder, previously unused tasks,
with evidence-discovery and complete-request economics. V1's single known repair,
repeated four times, cannot provide that evidence. T010's lexical default is
decided separately by its own local gates and G2; this pilot does not gate it or
retroactively establish an agent benefit for it.

The shared task suite also measures 003 T007–T009 and 008 T003–T005. Local
implementation tests remain necessary but cannot accept their host outcomes.
Each claimed capability needs a named comparison and an externally checked
outcome. A bundled improvement cannot establish the effect of every component.

## Tasks, arms and allocation

1. Keep development/calibration tasks separate from sealed evaluation tasks.
   Exclude the v1 `percent-encoding` repair and all tasks used to select repairs.
   Assign a stable task ID, repository revision, seed diff, rights record,
   prompt, permitted files/tools, hidden oracle and intent stratum to each task.
   Hash these inputs before running a candidate. Repetitions of one task are
   within-task observations, never additional independent tasks.
2. Include anchorless discovery, mixed behavior, ambiguous names and exact
   locators. Task-context cases additionally exercise restart/recovery, a retained
   constraint, rejected actions, conflicting observations, open obligations and
   recovery of folded evidence. Include cases where an intervention should remain
   silent. Define correct behavior before seeing model output.
3. T006's selected retrieval comparison is current Foundry, mixed-intent repair,
   anchorless repair and both repairs. Bind actual implemented configurations;
   an unavailable repair is a readiness blocker, not an arm that can silently be
   dropped. Reuse one baseline where the configurations and tasks permit it.
4. For task context, compare the ordinary control, passive retained context and
   selective intervention. A capability-to-contrast matrix must identify which
   comparisons test each of the six tasks. The unimplemented 008 T004 semantic
   triggers cannot be credited as path reminders or assumed available.
5. Within a task block, keep source, instructions, catalog, model, settings,
   request/time limits and tool authority equal except for the named treatment.
   The control gets the same ordinary task instructions, without v1's asymmetric
   placeholder telling only that arm to inspect the workspace. Record actual
   final-request bytes and remaining admission headroom for each arm.
6. Freeze randomized, balanced arm order within strata, its seed and all planned
   repetitions before outcomes. Never replace a failed, partial or inconvenient
   task. Retain every scheduled row, including unstarted rows after a halt.

## Sample size and decision rule

The freeze must include an executable, deterministic power calculation, its
assumptions and sensitivity table, the independent task count, repetitions,
stratum counts and every confirmatory contrast. An arbitrary small pilot or many
repetitions of one task does not satisfy the owner-selected powered sample.
No sample count or spend has yet been selected; this is a **freeze blocker**.

For each comparison, predeclare the verified-success noninferiority margin,
required evidence/discovery improvement, total-cost and latency bounds, confidence
level and multiplicity treatment. Power both the success safeguard and the
improvement claim. Model task clustering explicitly; do not treat repeated runs
as independent. Show sensitivity to discordance and baseline success, rather than
deriving optimistic assumptions from v1's eight successes. Freeze analysis code
with the launch manifest. No outcome-driven sample extension or threshold change.

Acceptance requires the preregistered success safeguard and discovery improvement
within the cost/latency limits, plus complete capture and valid oracles. Report
paired uncertainty, absolute success, per-intent regressions and denominators.
An incomplete or underpowered result is descriptive and leaves the task open.

## One capture and accounting owner

Reuse the Rust `pilot_capture` apparatus and its narrow installed-OMP bridge.
Do not add a new proxy, experiment service, database or generic workflow owner.
The private launch manifest binds the contract digest, apparatus/executable hashes,
host/model/settings, task and oracle manifests, allocation, profile/model inputs,
all permitted routes, permissions and actual run directory. Rehash inputs at use.

**Selected authority/run-path repair, 2026-11-11 — qualified at the selected local fixture boundary.** Launch
metadata uses `bounded-pilot-v1-launch-v2` with required canonical `run_dir`, distinct
from `isolation_root`. The existing `manifests` command requires `--run-dir`;
`build_manifests(root, verified, run_dir: &Path)` binds the same explicit capture
root into probe and coding manifests before rendering. `AuthorityReceipt` keeps
its existing shape: its exact manifest digest authorizes that path. Missing or
legacy unbound metadata refuses; no default path or historical-file migration is
allowed. Fresh admission checks the canonical parent/final name before claims and
rechecks the created root before credential access. Coding admission requires the
declared, authorized and supplied/cited-seal roots to resolve to the same canonical
directory before journal access, fresh authority claim or credential loading.

The existing synced `admitted` and `coding_handoff` records bind flat
`authority_receipt_sha256`, `scope` and actual canonical `run_dir` before a runnable
session returns. Live uses the digest of the exact authority bytes already
validated; Verification records null authority and still enforces supplied-manifest
and handoff path identity. Append failure must prevent a runnable session. Live
handoff also requires the probe's bound admission record; missing historical
authorization/path bindings refuse as `probe_seal_not_journaled`. Preserve the
existing credential-refusal terminal shapes, halt ordering and restore repairs.
This is capture metadata revision 2 under unchanged v1 policy, not pilot-v2
execution authorization. Frozen v1 contract, manifests, claims, journals and
receipts remain immutable history; v2 remains unfunded, unfrozen and unaccepted.
[Qualification evidence](../../../docs/validation.md#pilot-authority-and-run-path-binding--2026-11-11); the broader v2 freeze requirements remain.

Preserve v1's essential admission invariants: final serialized request identity
after hooks; a pinned compatible counter projection; one generation in flight;
single-use permits; no automatic retries; checked integer reservations; terminal
usage and attempt/response identity reconciliation; no refund of unknown spend;
and fail-closed capture. Numeric limits, rates and routes must be freshly frozen
for v2. V1's USD10 envelope and Anthropic configuration are not v2 defaults.

The new launch manifest must give concrete per-generation, slot, preparation,
checker and whole-run time bounds; HTTP/count/generation attempt caps; inclusive
input/output limits; per-slot and total reservation envelopes; disk/memory bounds;
and cleanup deadlines. Cost the entire frozen schedule at a pinned current ratecard,
including failed/invalid attempts, preparation, the compatibility probe and
unresolved reservations. No automatic top-up or implicit budget reallocation.

Keep wire usage, native imports, delivered context and observed provider input
distinct. Require the [economics contract](adapter-economics.md#acceptance-and-improvement-claims)
at the claimed boundary. Missing reasoning or other required categories stay
missing. Any narrower observation label would need its own explicit scope; v1's
exception does not establish v2 economic acceptance. Never add local embedding
tokens to provider tokens or infer consumption from delivered text.

Record first useful evidence, complete supporting evidence, exploratory calls,
steps to a verified patch, retries, constraint violations, unnecessary/missed
interventions, success, turns and latency. Count local document **and query**
embedding calls/tokens where observed; otherwise report unknown. Include fresh
preparation and explicit amortization assumptions. Separate host time, local
preparation/checking, operator work and approval waits without overlapping totals.
Natural hosted cache is observed, not reset by a fresh HOME. Report arm order,
prefix/cache layout, gaps and observed cold/warm cases; do not infer cache-independent
savings from pooled naturally cached runs.

## Required v1 gap repairs and qualification

Before freezing the apparatus, demonstrate these with the actual installed host
and local no-provider fixtures. These remain freeze requirements; completed narrow
repairs are identified in the relevant row:

The existing owners are [`CaptureSession` and `IndependentCheckReceipt`](../../../src/pilot_capture.rs),
[`Host::launch` and `HostEvent`](../../../src/pilot_capture/host.rs), and
[`run_independent_check`, `restore_workspace` and `run_coding`](../../../src/pilot_capture/apparatus.rs).
Extend their hash-chained journal and the
[`full_pilot_pipeline_qualifies_without_provider` integration test](../../../tests/pilot_capture.rs).
The current source pointers establish ownership, not completed qualification.

| Disclosed gap | Required evidence |
| --- | --- |
| Host startup network denials absent from journal | Capture denied startup attempts and their destination/classification before any paid request; disallow unobserved routes. A sandbox denial is not evidence of journal completeness. Matching-release source establishes only partial diagnostics; a complete early observer and completion boundary remain needed ([source finding](../../../docs/validation.md#pilot-startup-network-source-feasibility--2026-11-11)). |
| Checker binds environment names only | Bind exact sanitized environment values, sandbox profile bytes and freshly hashed oracle/compiler bytes to each checker receipt. Credentials are absent from the checker environment. The selected local checks pass ([record](../../../docs/validation.md#pilot-checker-provenance-and-retained-diagnostics--2026-11-11)); the limits in the selected repair below and broader v2 qualification remain. |
| Compile stderr only hashed | Retain bounded private stderr bytes and their digest, with explicit overflow/failure behavior. Inspect errors; a matching binary hash cannot explain omitted diagnostics. The selected local checks pass ([record](../../../docs/validation.md#pilot-checker-provenance-and-retained-diagnostics--2026-11-11)); the limits in the selected repair below and broader v2 qualification remain. |
| Coding authority outside chain; declared/actual paths differ | Append the authorization digest to the durable chain before a paid attempt and require the canonical actual run root to equal the authorized root. The selected metadata/path and chained-authority fixtures pass ([record](../../../docs/validation.md#pilot-authority-and-run-path-binding--2026-11-11)); historical v1 evidence is unchanged. Current installed-host compatibility and actual Live-compatible probe history remain unproved. |
| Some private artifacts have wider permissions | Enforce and verify 0700 directories/0600 private files, including host/store outputs; fail before transmission if this cannot be established. Executables need explicitly declared private executable modes. The selected local metadata, child-launcher, lexical-store, pinned-host and actual-checker checks pass ([record](../../../docs/validation.md#pilot-private-output-permissions-and-terminal-publication--2026-11-11)). This permission slice does not qualify semantic-worker output modes, the original private full pipeline, current installed-host compatibility or complete startup-network observation. |
| Rejected credential after claim has no terminal journal outcome | Preserve consumed claim and append a terminal refusal without printing the credential; prove no provider transmission. The selected local repair passes the pinned-artifact fixture checks, including synthetic Live admission ordering ([record](../../../docs/validation.md#pilot-credential-refusal-durability--2026-11-11)). Current installed-host compatibility and actual Live-compatible probe history remain unproved; v2 still requires its declared host/arm qualification. |
| Workspace restore failure exits without halt record | Journal a terminal halt, stop owned processes and prohibit the next slot. The selected local fixtures pass: healthy journal finalization retains the halt and unstarted rows, the post-admission CLI returns failure, and failed halt persistence prevents a later success receipt ([record](../../../docs/validation.md#pilot-workspace-restoration-and-halt-persistence--2026-11-11)). This does not establish live-front-door admission, current installed-host/provider compatibility, private-oracle outcomes or power-loss durability; v2 still requires its declared host/arm qualification. |
| `seal_slot` relies only on exit status | Bind actual expected oracle-completion evidence to the checked candidate and receipt; zero exit without the required completion evidence cannot pass. The selected completion/artifact consumer and actual owned-copy producer checks pass ([record](../../../docs/validation.md#pilot-checker-completion-and-artifact-binding--2026-11-11)). Complete stream capture and terminal check-chain errors are enforced; this does not qualify the private full pipeline, remaining capture gaps or pilot v2/T006. |

**Selected checker-completion repair, 2026-11-11 — qualified at the selected local checker boundary.**
The existing receipt owner must require the pinned oracle's complete successful
stdout digest (`aac03c0ded8403eb8bd3ca53e9880313cde11fa658db4fc7444d737eef33127d`),
recovered from hash-verified historical evidence, for an all-zero-exit pass.
Require each successful compile's exact artifact and bind the actual pre-execution
`out/check` hash to the compile result. A complete observed nonzero check remains
an ordinary failure; missing completion or provenance is a terminal refusal.
Stream read errors cannot certify a prefix as complete output. Hash the same oracle
bytes copied into the new checker root. Capture, receipt-write and sealing errors
use the existing session halt and leave later slots unstarted. This reuses current
receipt fields and preserves frozen v1 policy/history. Qualification separates
synthetic consumer tests from the real fixed compiler/oracle on new owned copies;
it does not rerun the original calibration harness or accept pilot v2/T006.
[Qualification evidence](../../../docs/validation.md#pilot-checker-completion-and-artifact-binding--2026-11-11); the broader v2 freeze requirements remain.

**Selected checker provenance and diagnostics repair, 2026-11-11 — qualified at the selected local checker boundary.**
The required checker receipt metadata revision is `bounded-pilot-v1-check-v2`,
distinct from pilot-v2 execution authority. New receipts bind the exact six-value
sanitized environment and sandbox profile bytes used for the checker, plus the
measured compiler header and fresh pre-execution executable digest at each step.
The copied oracle is rehashed before oracle compilation. The consumer requires
these exact bindings; legacy or incomplete receipts cannot acquire them through
serde defaults or historical-file rewrites. Frozen v1 policy/history stay unchanged.

Retain each complete stderr as the private `diagnostics/<step>.stderr` artifact,
including empty output, with required relative path and byte count beside its
existing digest. The consumer checks the exact named file without following links,
its owner/type/0600 mode, bounded length and digest. Limit stderr to 1 MiB per step
and 4 MiB across the four-step recipe. Overflow or read failure retains at most the
cap as an explicitly named private partial file and digest when persistence succeeds,
then returns a terminal capture error; it cannot become an ordinary checker failure.
Failure to persist diagnostics remains a failure. Complete observed nonzero checks
still produce ordinary Fail. Do not emit raw diagnostics in shared output or Git.

One existing absolute checker deadline covers the process and both pipe readers,
including a leader that exits while an owned descendant keeps a pipe open. Capture
failure or deadline stops the owned group, joins the readers and leaves later steps
and slots unstarted through the existing halt owner. Qualification separates real
consumer/file checks, owned-process lifecycle fixtures, and the fixed compiler/oracle
on fresh owned copies. A pre-execution pathname hash is not atomic exec attestation;
dynamic libraries/sysroot, compiler-version preflight bounds and startup-network
observation remain separate gaps. Broader private-artifact permissions have their
selected qualification below. This repair neither
runs the original calibration harness nor accepts or funds pilot v2/T006.
[Qualification evidence](../../../docs/validation.md#pilot-checker-provenance-and-retained-diagnostics--2026-11-11); broader v2 freeze requirements remain.

**Startup-network source finding, 2026-11-11 — requirement remains open.**
Matching-release source supports an extension logger sink and awaiting registry
refresh, but captured early fetch, direct HTTPS prewarm, discarded prewarm promises
and silent/fallback discovery prevent a complete extension-only claim. The repair
needs an earlier observation/refusal seam covering every allowed startup owner and
an acknowledged complete observation prefix before count/messages admission.
Source inspection is not shipped-runtime coverage or OS-denial proof; no host re-pin
is selected. See the [source finding](../../../docs/validation.md#pilot-startup-network-source-feasibility--2026-11-11).

**Selected private-output permissions repair, 2026-11-11 — qualified at the selected local no-provider boundaries.**
[Qualification evidence](../../../docs/validation.md#pilot-private-output-permissions-and-terminal-publication--2026-11-11) records the candidate, retained failures, exact control boundaries, separate gate-only continuation and unexecuted scope.
The selected repair requires exact owner-only modes on newly owned pilot
outputs: directories 0700 and regular private files 0600, including special bits.
Declared 0700 executables are a checker root's exact `out/check` and the pinned
host's exact `home/.omp/natives/18.13.2/pi_natives.darwin-arm64.node` beneath an
owned host root. The latter is limited to the actual declared capture run's
`hosts/<host>` children or the corresponding fixed pilot `runs/<run>/hosts/<host>`
layout; it grants no workspace/store or arbitrary-suffix executable exception.
The host exception follows the observed native extraction in the local fixture;
it changes no host pin and establishes no new per-request content attestation.
Existing wrong-mode objects are refused rather than repaired. Shared external
executables, models and the public frozen contract retain their existing identity
checks; named private inputs are checked at their specific read sites without
normalizing historical trees or shared ancestors.

One capture-specific, nonfollowing metadata walk checks the selected owned roots
at admission, before transmission and after producer completion. Each logical pass
shares hard limits of 16,384 entries, depth 64 and 8 MiB of relative-path bytes,
and checks an enclosing deadline where one exists. Descriptor-relative traversal
checks type, owner, exact mode and directory identity; missing, replaced or
unverifiable entries refuse the pass. Ordinary active-file growth does not fail
merely because its size or modification time changes. This is a bounded metadata
observation, not atomic protection against arbitrary concurrent same-user changes.

Exclusive new-object writers establish and verify their own exact modes, sync the
artifact and its parent, and never widen a preexisting object to make it pass.
Host, Foundry and checker children receive umask 0077 in their existing pre-exec
operation; the parent mask is unchanged. Checks after each selected closed
producer and before receipt publication establish the observed output modes;
inheritance alone is insufficient evidence. Private source copies preserve bytes
and refuse links or special files. Checker diagnostic and artifact checks keep
their existing identity, completion, retention and failure requirements.

The existing capture owner refuses privacy failures before reserving/sending a
count request and rechecks before sending Messages. A failure after an observed
count preserves that observation, releases only the provisional reservation,
halts and sends no Messages. Post-send failures retain actual usage and halt;
they cannot retroactively prove earlier privacy. Admission, restoration, view
loading and host finalization route failures through their existing terminal
owners. CLI success additionally requires successful finalization and exclusive,
durable seal/report persistence. A sidecar failure after journal finalization is
reported by the CLI without reopening or rewriting the terminal journal.

Qualification must distinguish helper/child-launcher controls, actual current
Foundry stores, pinned-host fixture requests and the real checker producer.
Unexecuted semantic-worker, original private full-pipeline and current-installed-
host boundaries remain explicit. This repair neither authorizes a provider call
nor freezes or accepts pilot v2/T006; startup-network observation remains separate.

Preserve v1's original failures and interpretation. Its two blocked `read .` calls
remain constraint violations with the owner's accepted oracle outcomes. V2's task
manifest must distinguish blocked out-of-scope reads, forbidden tools and actual
unauthorized effects before launch. Unauthorized effects or containment/capture
failure globally halt; ordinary incorrect patches remain results.

Use the existing bounded checker and fresh per-slot workspaces. Candidate hashes
must bind the edited source to the executable and externally specified oracle.
Prove each seed fails its intended check and a reference repair passes, without
exposing hidden checks to the coding host. Keep compiler/oracle execution inside
the declared task-specific authority; indexing alone never authorizes source code.

## Failure, privacy and execution locks

All failures retain their original disposition. Unknown usage, missing terminal
capture, unauthorized traffic, pin drift, append failure, unresolved cleanup and
source/oracle mismatch halt further paid requests. A timeout is not evidence of
zero spend. Late reconciliation does not automatically resume a halted schedule.
No outcome-selected retries, prompt reduction or replacement samples.

Raw tasks, source, native sessions, credentials, oracle inputs and model weights
remain private. Publish only authorized aggregates, public apparatus pins and
limitations. Task-data/retrieval rights do not grant training consent. No secret
appears in argv, logs, checker environments or model-worker inputs.

The following gates remain explicit, with no date or authorization implied:

- **Protocol preparation:** task/oracle manifest, implemented arm matrix, powered
  sample/analysis, request/resource bounds and costed schedule. The implementer
  prepares these without provider calls; task rights and consequential budget
  choices belong to the owner. Do not call this draft frozen until all are concrete.
- **Apparatus qualification:** the repairs above and actual-host no-provider
  qualification on the pinned candidate, preserving failure receipts.
- **Launch evidence:** current host/model compatibility, complete available usage,
  account/ratecard and provider retention terms, task-data rights, private path and
  permissions, model/profile pins and fresh read-back of the launch manifest.
- **Owner authorization:** a new scoped credential and digest-bound probe-only
  authorization, then separately coding authorization for the costed frozen
  schedule. A successful probe supplies compatibility evidence, not coding consent.

Record the final frozen contract digest outside its body. Any subsequent policy
change requires a new version and scope; any implementation change requires fresh
qualification at its affected boundary. Until these gates pass, T006 and the six
task-context outcome tasks remain unaccepted, with no spend authorized.
