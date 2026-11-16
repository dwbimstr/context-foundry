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
   comparisons test each of the six tasks. 008 T004 now implements the owner's
   explicit structured assertion triggers locally under the no-model rule. Those
   comparisons need declared assertions and actual-host qualification; path
   reminders alone cannot be credited as contradiction/constraint detection.
5. Within a task block, keep source, instructions, catalog, model, settings,
   request/time limits and tool authority equal except for the named treatment.
   The control gets the same ordinary task instructions, without v1's asymmetric
   placeholder telling only that arm to inspect the workspace. Record actual
   final-request bytes and remaining admission headroom for each arm.
6. Freeze randomized, balanced arm order within strata, its seed and all planned
   repetitions before outcomes. Never replace a failed, partial or inconvenient
   task. Retain every scheduled row, including unstarted rows after a halt.

### Capability-to-contrast preparation

This matrix identifies the comparisons that a freeze must bind; it does not select
extra runs, sample sizes, budgets or a host configuration. The ordinary/passive/
selective three-arm comparison estimates a bundle. It cannot independently accept
all six task-context capabilities. Any additional confirmatory contrast must enter
the powered analysis, multiplicity and complete costed schedule before launch.

| Owning task | Contrast to bind to actual implemented configurations | Necessary task/control evidence |
| --- | --- | --- |
| 003 T007, explicit delivery | Ordinary tool delivery versus explicitly scoped task-view delivery, with durable operations, recovery search, joint retrieval, composition and intervention held fixed | Record complete delivered and final host-request bytes, exact revision/digest and host acknowledgment where claimed. An MCP reply alone is not host application. |
| 008 T003, durable state/views | The same delivery interface and initial facts, with ordinary retention versus explicit checkpoint/fold/restore and exact-original recovery | Predetermine restart/continuation points and the externally checked observation/constraint needed after them. Preserve host history/tool linkage. Do not count multiple restarts or seeds of one task as independent tasks. |
| 003 T008, joint delivery | The same retained state and source candidate configuration, with separate task/source delivery versus one revision-bound joint task/source response | Cap/account the complete control delivery too; preserve every protected task entry. Trace final-snapshot consistency, budget refusals, first complete supporting evidence and verified patch. Smaller replies alone do not pass. |
| 008 T005, content recovery | The same stored originals/views and joint-delivery policy, with explicit-ID peek versus content search over retained originals | Include folded/deselected evidence whose IDs are not supplied to either arm; freeze the correct original and expected freshness. Keep task instructions and initial information equal, and separately count unsuccessful searches/exploratory reads. |
| 008 T004, selective review | The same records/assertions and passive delivery, with structured review absent versus the declared selective-review integration | Freeze scoped single-valued keys, proposed assertion source, exact relations, the host call/admission seam and expected intervention/silence cases. Use fresh, conflicting, satisfying, missing-key and stale-source cases; oracle-check both missed and unnecessary interventions. The local comparison engine does not establish this host integration or interpret prose. |
| 003 T009, evidence composition | The same joint task/source view and review/recovery configuration, with `compose_evidence` disabled versus enabled | Freeze required implementation/consumer/constraint/test facets and actual citations. Measure complete support and verified outcome, including missing/conflicting facets. Hold optional briefs fixed; any selected auxiliary generation needs its own authorized usage and preparation accounting. |

The freeze must resolve shared controls and order without changing the named
treatment, bind each contrast's concrete prompts/tools/flags and endpoint, and name
which contrasts remain descriptive. Reusing a baseline does not make task blocks
independent. All arms receive the same task instructions and initial facts except
for their declared capability treatment. V2 still needs concrete tasks/oracles and
rights, current-host qualification, available retrieval repairs, the selected
statistical design and launch pins; this matrix supplies none of those approvals.

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

**Selected real-host no-provider continuation, 2026-11-14 (owner).** Qualify on
owned copies without modifying the installed host, user configuration, frozen v1
contract or live pin. The observed installed artifact is OMP 18.18.1, SHA-256
`ce80099f10c5ea4367523767230cb5b920f30cc139b9cf8758c1cc4b2f1f9409`.
Its network-denied `--version` check is runtime evidence, not source/build
correspondence. Two boundaries must remain distinct:

- Exercise the actual current artifact through existing `Host::launch`, embedded
  bridge/channel, settings readback and final-request validator. Capture one
  synthetic prompt's actual serialized request, refuse it locally and reap the
  host. No Count/Messages transport or real credential is constructed. Hash/recheck
  the selected binary at use; preserve the request privately, exact refusal and
  cleanup/mode evidence. Current-native extraction is checked only at its declared
  `home/.omp/natives/18.18.1/pi_natives.darwin-arm64.node` path. This test does not
  widen v1's native-path exception or admit this host to v1 CaptureSession/Live.
- Run the full existing nonignored actual-host integration suite on current
  Foundry source with the separately archived exact v1 artifact (`38c36452…`),
  existing owned numeric-loopback TLS fixtures and synthetic credentials. Retain
  every failed run. A pass qualifies that pinned consumer boundary, not current
  installed-host admission, a new v2 pin or a paid compatibility probe.

One shared hour includes preparation/build/qualification, with a 120-second cleanup
reserve, one heavy job, offline dependencies, 20 GiB free-disk and 8 GiB sampled
individual-process RSS guards. A source change gets the applicable full local
gates. Mismatches are reported without changing request rules, outcomes, host pins
or budgets to pass. No provider calls, signing, model downloads or training.
Complete early startup observation/barrier, transient library modes and concrete
v2 task/arm/host qualification remain separate requirements; a Ready frame or one
accepted synthetic request does not close them.

Before either qualification boundary ran, the owner prioritized publishing current
work and cleaning `target`, `runs` and the Foundry datasets (2026-11-14). Only the
contained version check above has executed. Preserve the selected artifact and
inputs; resume qualification with a fresh stated deadline after storage cleanup.
The preparation deadline is not evidence of a live or completed test.

**Qualification outcome, 2026-11-14.** The selected continuation ran under a fresh
shared deadline after cleanup. The complete nonignored frozen-host suite passes.
The current artifact reaches the embedded bridge and settings boundary, but its
actual serialized request adds `message-threads-2026-08-12`; the unchanged frozen
validator refuses it as `beta_not_admitted`. Current-host acceptance remains
**HOLD**. This is a locally refused request with normal cleanup, not a new v2 pin
or CaptureSession/Live admission. Closed-file permissions pass at the declared
current-native path; complete startup observation and transient modes remain open.
See the [runtime receipt](../../../docs/validation.md#real-host-no-provider-qualification--2026-11-14).

Before freezing the apparatus, demonstrate these with the actual installed host
and local no-provider fixtures. These remain freeze requirements; completed narrow
repairs are identified in the relevant row:

**Selected full-history delegate repair, 2026-11-14 — qualified at the selected request boundary, 2026-11-15.**
The retained current-host request contains both `thread: {type: "create"}` and the
new message-threads beta. Its associated upstream
[`wantsAnthropicThreads`](https://github.com/can1357/oh-my-pi/blob/24439066040e006eb08da7b204f0ac41c79d2de9/packages/ai/src/providers/anthropic-threads.ts#L140)
honors an explicit `statefulResponses` value before environment/model defaults;
[`streamAnthropic`](https://github.com/can1357/oh-my-pi/blob/24439066040e006eb08da7b204f0ac41c79d2de9/packages/ai/src/providers/anthropic.ts#L2262)
uses that choice. Set `statefulResponses: false` in the existing embedded bridge's
delegate options so the native serializer retains complete request history. Do not
strip actual request fields/headers, widen beta/body admission, add server-history
state or change the v1 host pin. This reduces state/accounting responsibilities
instead of introducing a second history owner. This repair preserves the existing
full-request invariant and grants no new execution authority.

First commit the actual-host regression with this contract and confirm red on the
unchanged bridge. The ignored `current_host_first_request_preserves_full_history`
test requires an explicit owned OMP 18.18.1 artifact with the recorded SHA; it uses
`Host::launch`, actual setup/readback and the unchanged final-request validator,
refuses the request locally, and reaps the host before asserting. Optional private
evidence preserves exact request bytes and failure/pass metadata. No provider
transport is constructed. After the bridge change, require this test green, the
complete nonignored frozen-host suite and all eight local gates. The ordinary
suite does not require an installed newer host; explicit current-host diagnostics
do not grant CaptureSession/Live admission. Preserve the original failed request,
all red/gate failures and frozen policy. The shared one-hour qualification includes
builds, with a 120-second cleanup reserve, offline dependencies, one heavy job,
20 GiB disk floor and 8 GiB sampled individual RSS ceiling. Complete startup
observation and transient-mode coverage remain separate requirements.

Contract/test commit `50c571c` precedes the bridge edit. The regression is red on
that commit: the actual request contains `thread`, fails unchanged beta admission,
and the host exits normally with its group gone. The 92.031-second receipt includes
offline compilation. The original current-host failure is preserved separately.
Private evidence: `runs/pilot-stateless-host-2026-11-14/red-result.json` and
`red-evidence/`; no provider request or policy change was involved.

On `98157db`, the current-host regression passes unchanged request admission, the
complete nonignored frozen-host suite passes, and all eight local gates exit 0.
The private gate wrapper incorrectly rejected the expected fake workers; the
original failure is retained and qualified by separate complete gate/log/fixture
readback, without rerunning or changing any product gate. See the
[repair receipt](../../../docs/validation.md#stateless-native-request-repair--2026-11-15).
The exact v1 host pin and request policy remain unchanged. First-request
qualification does not accept complete current-host behavior or pilot v2.

Repeat only the explicitly selected current-host diagnostic with an owned artifact
matching the test's fixed SHA, without changing v1 admission:

```sh
CF_PILOT_CURRENT_OMP=/abs/owned/omp cargo test --locked --offline --features pilot-capture --test pilot_capture current_host_first_request_preserves_full_history -- --ignored --exact --nocapture --test-threads=1
```

`CF_PILOT_CURRENT_EVIDENCE_ROOT=/abs/new/private-dir` optionally retains the exact
synthetic request and result under owner-only permissions. It must name a fresh
directory. The accepted run and cache-retention paths are in the linked receipt;
no installed host, model or credential is acquired by this command.

**Selected current-host continuation qualification, 2026-11-15 — qualified at the owned-fixture boundary.**
Extend the existing ignored diagnostic with one two-generation tool conversation
on the same owned OMP 18.18.1 artifact. Keep application code, v1 pin, codec and
request/SSE admission unchanged. Reuse the owned numeric-loopback TLS fixture and
`Transport` with `Endpoint::Verification` and the synthetic fixture credential: count each actual
request's exact projection, send its unchanged body to the local scripted Messages
endpoint, and relay only events accepted by the existing `MessagesStream`. No Live
transport, real credential or external-provider request is authorized.

The synthetic first response asks for one read of the owned `src/lib.rs` fixture.
Admit only that exact call through the existing tool policy, observe completion,
and bind the next actual request to the first request's prior history and observed
tool ID. Both requests must omit `thread` and pass unchanged admission; the followup
must preserve the original message and include the actual source result. A control
using the same followup bytes without completion evidence must fail. Complete the
second scripted response and require native prompt completion, exact four-request
fixture order/exhaustion, unchanged source bytes, owner-only closed outputs and
normal host reaping with no extra fetch. Fixture counters/usage are synthetic,
never real-provider usage or cost evidence.

Share the existing diagnostic setup so the first-request case stays exercised.
Run both explicit current-host tests and the full local gates on the test-only
change; keep all failures and the original first-request evidence. The shared hour
includes builds, with 120 seconds for cleanup, one heavy job, offline dependencies,
20 GiB disk floor and 8 GiB sampled individual RSS ceiling. Complete early startup
observation, current-host Live admission, pilot-v2 outcomes and signing stay gated.

Both explicit current-host tests and all eight local gates pass on `bb17708`;
application inputs remain unchanged from `98157db`. The actual two-generation
conversation completes, the four owned-fixture HTTP bodies pass identity checks,
and the missing-completion negative control rejects. The full
[continuation receipt](../../../docs/validation.md#current-host-tool-continuation--2026-11-15)
separates scripted usage from unknown real-provider economics and leaves native
transcript usage readback and complete early startup observation unqualified.
Select both diagnostics with `current_host_ -- --ignored --nocapture
--test-threads=1` in the command above, omitting `--exact`. Optional evidence uses
the named fresh directory for the first case and its `-followup` sibling for the
continuation; no real credential or model is acquired.

**Selected native usage readback, 2026-11-15 — qualified at the owned-fixture boundary.**
Extend that same owned-fixture continuation after normal host exit. Read the single
native session through the existing private/bounded reader, retain its exact bytes
when evidence output is selected, and join both assistant entries to the admitted
wire response IDs in order. Require unchanged model and stop disposition, no extra
assistant or out-of-transcript usage entry, no unparsed line, and equality of the
four scripted wire counters and their checked total. An absent reasoning subset
must remain absent. Do not treat the host's calculated price as observed spend.
Exercise altered response identity, omitted/null/negative/changed counters, a
fabricated reasoning zero, duplicate assistant and out-of-transcript usage controls
against that actual native readback. Missing or disagreeing evidence fails this
diagnostic; it cannot be repaired by mutating the native transcript or relaxing v1.

This is test-only no-provider qualification, not CaptureSession/Live or v2
admission. Run both explicit current-host diagnostics, focused lint and applicable
local gates under a shared hour including offline builds, 120-second cleanup
reserve, one heavy job, 20 GiB disk floor and 8 GiB sampled individual RSS ceiling.
Publish only public tests/records; retain private evidence and prune new build
intermediates after publication and readback. Startup coverage, real-provider
economics, paid authority and signing remain separate gates.

Both current-host diagnostics and all eight gates pass on `40b75c6`. The exact
native transcript joins both wire response identities and all four scripted usage
buckets/totals, with no extra assistant or usage entry. All 29 native-row mutation
controls reject, including fabricated reasoning zero. The initial compiler failure
is retained. See the [native usage receipt](../../../docs/validation.md#native-four-category-fixture-usage--2026-11-15).
This closes the selected fixture readback gap; it does not measure real-provider
economics, supply unreported categories, admit the host to Live or accept v2.

The existing owners are [`CaptureSession` and `IndependentCheckReceipt`](../../../src/pilot_capture.rs),
[`Host::launch` and `HostEvent`](../../../src/pilot_capture/host.rs), and
[`run_independent_check`, `restore_workspace` and `run_coding`](../../../src/pilot_capture/apparatus.rs).
Extend their hash-chained journal and the
[`full_pilot_pipeline_qualifies_without_provider` integration test](../../../tests/pilot_capture.rs).
The current source pointers establish ownership, not completed qualification.

**Startup preload feasibility, 2026-11-15 — earlier seam demonstrated, not implemented.**
The same owned current artifact executes an absolute owned preload supplied through
`BUN_OPTIONS` before its ordinary entrypoint. Sentinel and missing-preload controls
pass. A network-denied RPC startup records and locally refuses 15 fetch invocations,
then exits normally. Its original destination-specific assertion remains failed;
the [runtime finding](../../../docs/validation.md#current-host-startup-preload-feasibility--2026-11-15)
qualifies only the actual observations. This refines the earlier source finding:
a new upstream build is not yet established as necessary for an early hook.

Evaluate an owned preload through the existing `Host::launch`/environment owner
under a separately qualified v2 configuration, rather than adding a wrapper service.
It must precede every captured fetch reference, bind exact preload/host bytes and
use the existing journal/channel for durable refusal and explicit acknowledgment.
This is design preparation, not an environment exception for frozen v1. Complete
owner coverage, non-fetch bypass controls, drained/cancelled startup work and the
parent-prefix acknowledgment still need implementation and actual-host proof;
ordinary RPC readiness or process exit does not replace them. No v2 pin or execution
authority is selected here.

### Proposed startup observation and admission

**Unfunded design preparation; not implemented or frozen.** Preserve complete
early observation and explicit settlement. The
[actual-host counterexample](../../../docs/validation.md#startup-settlement-counterexample--2026-11-15)
shows RPC readiness while a controlled discovery fetch is pending, followed by
abort and a later callback. The preload is an available early seam; readiness,
a timeout race, an aborted waiter or a quiet interval cannot establish drain.
The existing `ModelRegistry` source interfaces alone have not qualified that
complete boundary. A new upstream build is not assumed necessary, but the selected
artifact must expose source-grounded control of every admitted startup owner.

**Configuration preparation.** The owned current-artifact
[profile control](../../../docs/validation.md#restricted-startup-provider-profile--2026-11-15)
qualifies the existing `disabledProviders` setting for reducing built-in discovery
while retaining the fixed native model. Prefer this existing owner for excluding
providers unused by a future fixed-model pilot, rather than a new discovery loop.
Bind the exact artifact, compiled provider inventory, explicit list and config
digest at freeze; do not infer provider identity or silently carry this list to a
new host. The retained model's shared catalog still fetches. Extension-registered
runtime managers and non-fetch routes need separate coverage, so the setting
cannot replace observation, drain or admission checks. Qualify actual generation
requests/tool behavior/headroom under the selected profile, not only state readback.
This is preparation, not a selected v2 execution profile or a v1 configuration edit.

**Selected owned fixed-catalog qualification, 2026-11-15 — implemented and locally qualified.**
Use the supported provider-registration API to own later catalog refreshes for the
existing canonical provider and fixed bundled model. A late factory cannot prevent
an already-started ambient request; zero initial fetches is not the acceptance
criterion. The initial request still needs early observation and settlement under
the complete startup contract. Reject the native-alias alternative: it changes
intrinsic/compatibility metadata and does not establish startup admission.

Add an explicit `HostSpec` catalog profile for no-provider qualification on the
owned 18.18.1 artifact only. Verify its SHA before creating a host root. Bind the
disabled list to the shipped SDK's bundled/shared-catalog/descriptor/implicit
inventory, including `minimax-cn`; the earlier compiled-entry list is not complete
for this purpose. Ordinary CaptureSession launches explicitly retain host defaults;
ambient environment cannot select the qualification profile. V1 pin, contract,
wire rules and paid authority remain unchanged; this is not a v2 execution profile.

In that profile, the existing bridge registers an owned callback that returns a
fresh copy of one bounded serialized bundled-model definition. The callback has no
IO, timer, retry or credential operation. Preserve canonical provider/model and
native capabilities rather than inventing metadata. Unsupported/malformed bundled
metadata fails before bridge readiness. The host's derived cache is an observation
surface, never a new admission authority or independent product log.

First require the actual-current-host two-generation diagnostic to fail on a
bridge without the callback: read the bounded private SDK cache after normal exit
and require an authoritative single-model row for the fixed canonical model.
Then implement the callback and require that test green, all current-host
diagnostics, the complete nonignored frozen-host suite and all eight local gates.
Retain cache/native/request evidence and the original red result. Use existing
numeric-loopback fixtures and synthetic credentials only; no Live transport.
The shared hour includes offline builds, 120 seconds for cleanup, one heavy job,
20 GiB disk floor and 8 GiB sampled individual RSS. Publish reviewed code/records,
then prune new build/native extraction caches after exact evidence readback.

The [dated qualification receipt](../../../docs/validation.md#owned-canonical-host-catalog-qualification--2026-11-15)
records the original cache red, actual-host green, frozen-host compatibility and
eight gates on `0b19098`. The complete early startup/settlement/parent-acknowledgment
boundary below remains unimplemented and unfrozen.

**Selected actual-bridge startup diagnostic, 2026-11-15 — implemented and locally qualified.**
Make startup observation reproducible through the existing explicit owned-catalog
qualifier on the pinned artifact, using the actual embedded capture bridge, custom
API, xhigh setting and tool conversation. Install a hash-bound embedded preload
before SDK startup. Load its fixed relative path from the private host root and
restore the canonical workspace before SDK initialization; the native runtime does
not accept the tested quoted/file-URI preload paths containing spaces. Exercise a
host root containing spaces. Ordinary CaptureSession/HostDefault launches retain
their current environment and working directory.

Keep the diagnostic's metadata-only output in the owned host root, bounded to
20 observed attempts and 8 KiB including installation/normal-exit records. Refuse
global fetch/preconnect locally; observe ClientRequest.end without calling it a
before-IO guard. Retain only bounded method, scheme/host/port and a fixed route
classification, never headers, bodies, query strings, credentials or exception
text. Verify ordered, complete trace format and the pinned runtime after normal
host exit, rejecting missing/partial/malformed output. Bind the preload digest in
the launch record. Capture a complete byte prefix before the first user prompt
and require the trace after exit to start with those exact bytes. Report later
observations separately. This is qualification evidence, not the product journal, a
startup-settlement proof, or a source of admission authority. The proposed product
observer must still share the bridge's channel/sequence owner and existing journal.

First require the actual owned-host conversation to fail solely for the missing
trace on an implementation without the preload. Then require all current-host
diagnostics, complete nonignored frozen-host compatibility, focused lint and eight
gates. Retain raw trace, requests, native usage and the red result. Do not infer
complete coverage from zero observed calls. Use the shared offline hour, 120-second
cleanup reserve, one heavy job, 20 GiB disk floor and 8 GiB sampled individual RSS
ceiling. Publish reviewed changes, then prune new build intermediates while
preserving runnable diagnostics and exact evidence. Paid runs and signing stay gated.

The [dated actual-bridge receipt](../../../docs/validation.md#actual-bridge-startup-diagnostic--2026-11-15)
records the committed missing-trace red, real spaced-root conversation, pre-prompt
prefix, six trace mutation controls, frozen compatibility and eight gates on
`1357756`. This accepts the diagnostic only; complete startup admission remains
unimplemented.

**Early capture-API configuration candidate, 2026-11-15 — rejected and removed.**
The committed paired red used the original model configuration in both profiles:
both complete tool conversations passed with identical complete native model
digests, then the zero-startup-attempt assertion failed on the observed HEAD. The
candidate set only `modelOverrides.claude-sonnet-5-5.api` to the custom capture API
before SDK selection. It failed before the first request with the closed bridge
fault `model_selection`; a HEAD was still observed. Its full metadata comparison
and zero-attempt acceptance did not run. Neither assertion was relaxed to accept it.

Matching source at `can1357/oh-my-pi` commit
`24439066040e006eb08da7b204f0ac41c79d2de9` exposes `applyModelOverride`, but
[`models-config-schema-bundle.ts::ApiSchema/ModelOverrideSchema`](https://github.com/can1357/oh-my-pi/blob/24439066040e006eb08da7b204f0ac41c79d2de9/packages/coding-agent/src/config/models-config-schema-bundle.ts#L122)
accepts a closed API set that excludes the capture API. This is a source-supported
explanation, not a runtime trace of the exact fault caller or reproducible build
proof. The upstream `model-registry.test.ts` test "an api override keeps a kind the
new api still serves" covers supported built-in APIs; it was inspected, not run.
Keep the rejected source commits, executable and native failure evidence. Ordinary
capture, the qualified catalog, bridge and preload retain their existing behavior.

Retain the useful diagnostic additions: hash the complete parsed native model
object without storing its headers, check the owned qualifier against its observed
artifact-bound model digest, verify the materialized models.yml digest, and retain
only closed bridge fault codes on failed capture. Require current-host diagnostics,
the complete nonignored frozen-host suite, focused lint and all eight gates on the
final source. Use the same offline hour, cleanup reserve and resource limits; then
publish the reviewed public payload and prune new build/extraction caches while
preserving runnable diagnostics and exact evidence. This diagnostic hardening does
not repair warm-up, prove settlement or enable startup admission. Provider calls,
paid authority and signing remain gated.

The [dated rejection and diagnostic receipt](../../../docs/validation.md#early-capture-api-rejection-and-native-model-identity--2026-11-15)
records the committed paired red, rejected native candidate, final diagnostic
qualification, frozen compatibility and eight gates on `5d9aa0e`. The unsupported
configuration is absent from active code; the complete admission boundary below
remains unimplemented.

**Agent-route refusal, 2026-11-15 — implemented and qualified at the named TCP boundary.**
Native primitive controls on the pinned current artifact observe one accepted
owned-loopback connection for stock namespace HTTPS and zero for both earlier
Agent hooks, with identical native model metadata. Select `Agent.addRequest`,
the earlier boundary. Replace the existing owned qualifier's end observation with
a metadata-only refusal there; ordinary capture remains unchanged. This refuses
the known SDK route before the tested TCP connection without selecting a proxy, changing
the canonical model or adding a product configuration/transport owner.

First commit a durable native test against the existing observation-only preload.
Use the actual launcher to obtain its owned configuration, then exercise the same
embedded preload in pinned SDK controls with an independent raw TCP listener and
one exact numeric-loopback sandbox exception. Stock HTTPS must connect once;
guarded namespace HTTPS with a custom Agent must connect zero times. Compare the
complete native model digests and require normal exit, no prompt and private
bounded metadata. An explicit raw `Bun.connect` control must still connect once
without becoming a claimed observed route: this guard cannot grant general
transport coverage or startup admission. The original preload must fail at the
guarded connection-count assertion before the implementation changes.

Then require that test, all current-host diagnostics (including the actual capture
bridge's strict tool conversation, model digest, native usage and trace controls),
the complete nonignored frozen-host suite, focused lint and all eight local gates.
Keep primitive controls, red and green evidence. One offline hour covers these
checks with a 120-second cleanup reserve, one heavy job, a 20 GiB disk floor and
8 GiB sampled individual RSS ceiling. Publish reviewed public code/records, then
prune the new build and duplicate SDK extraction caches after evidence readback.
Full owner inventory, non-Agent routes, settlement and the shared-channel durable
acknowledgment below remain required. No paid execution, signing or v1 change.

The [dated Agent-route receipt](../../../docs/validation.md#native-agent-startup-refusal--2026-11-15)
records the independent-listener red on `dbe7033`, the `[1, 0, 1]` stock/guard/raw
controls, actual bridge consumers, frozen compatibility and eight gates on
`7f54889`. The raw-socket bypass remains accepted evidence of incomplete coverage,
not an exception that can enable v2 admission.

**Selected owned settings quiescence, 2026-11-15 — implemented and locally qualified.**
The current RPC host starts config watching before `session_start`. For the
explicit owned qualification profile only, finish the bridge's intended startup
settings writes, then use the pinned SDK Settings owner to cancel future saves
and watching, join existing saves and serialize a final strict disk reload before
the bridge reads settings/model state and sends Ready. Verify the singleton's
agent directory equals the launcher's private directory; an unavailable or wrong
owner fails the existing settings stage. Keep this operation in the bridge,
whose existing embedded-source digest binds it; no additional owner or protocol.
Ordinary capture and frozen v1 launches retain their behavior.

The source basis is oh-my-pi `24439066040e006eb08da7b204f0ac41c79d2de9`,
`config/settings.ts::Settings::{flush,cancelPendingSaves,reloadFromDisk}` and its
private `#exclusive` reload serialization (blob
`8d427132126850002b94dbf6fd897f1ae6062e29`). `flush` alone does not join a reload.
Cancellation is normally used for discarded instances; this profile deliberately
uses it for an immutable owned run. Explicit future settings changes and unrelated
owners remain outside this settlement claim.

First commit a native regression against the unchanged bridge: after actual RPC
and bridge readiness, change only the private compaction-enabled flag and read
native state. The ordinary profile must apply the disk edit; the owned profile
must retain the original setting and complete model digest. The unchanged bridge
must fail the owned assertion. Then require the repaired test, all current-host
diagnostics, the nonignored frozen-host suite, focused lint and eight local gates.
Use an offline hour with one heavy job, a 120-second cleanup reserve, 20 GiB disk
floor and 8 GiB sampled individual RSS limit. Retain red/green evidence, publish
reviewed code/records and prune only new verified build/extraction caches. No
prompt or provider call is needed for this regression. Full owner coverage,
transport coverage and durable startup acknowledgment remain open.

The [dated settings receipt](../../../docs/validation.md#owned-native-settings-quiescence--2026-11-15)
records the proper native red on `db47f71`, five current consumers, frozen
compatibility and all eight gates on `6a9bb15`. Full startup admission remains open.

**Selected Agent-refusal lifetime repair, 2026-11-15 — implemented and locally qualified.**
The pinned SDK registers an abort callback before constructing its HTTPS request.
A synchronous Agent refusal bypasses the transport's release path; an actual
no-prompt host reaches Ready and then exits with an uncaught ReferenceError at
the native timeout. Immediate and microtask-deferred destroy(error) candidates
survive the deadline but do not release the listener; the immediate candidate also
fails the custom-Agent refusal witness by timing out. Reject them. A further
native control queues request destruction and explicit local error emission after
construction. It observes the SDK's listener removal, a second state response
beyond the unmodified deadline and normal exit. Retain all outcomes and the first
controller's rejected missing-exit record. Source basis:
oh-my-pi `24439066040e006eb08da7b204f0ac41c79d2de9`,
`providers/connection-prewarm.ts::prewarmModelConnection` and
`providers/cowork-fetch.ts::sendCoworkRequest` (request construction and release).

Repair the existing qualifier's known-route refusal through the request error
channel, keeping the original Agent queue uncalled and the TCP count zero. The
socketless request on this pinned Bun runtime does not surface destroy(error);
explicitly emit the queued local error after retiring it. Do not catch or suppress
uncaught exceptions, shorten the native timeout, synthesize a successful response,
or change ordinary capture/v1. No additional module, transport or protocol is
needed. Other transports and complete startup settlement remain open.

First extend the actual owned capture-bridge conversation: after its initial
native settings readback, cross the pinned 10-second deadline and require another
state response with the same complete model before the first prompt. The existing
guard must fail before any Count/Messages fixture call, with a reaped nonzero host
and no successful trace. Then repair the refusal, require the full tool conversation,
all current diagnostics including independent TCP controls, frozen compatibility,
focused lint and all eight gates. Use the offline hour, 120-second cleanup reserve,
one heavy job, 20 GiB disk floor and 8 GiB sampled individual RSS ceiling. Publish
reviewed public changes and prune new build/duplicate extraction caches after exact
readback. This repairs a demonstrated callback lifetime failure; it does not prove
every owner settled or enable v2 admission, paid runs or signing.

The [dated lifetime receipt](../../../docs/validation.md#native-warm-up-refusal-lifetime-repair--2026-11-15)
records the actual-bridge red on `91d0c34`, rejected destruction candidate, native
listener removal, all current/frozen consumers and eight gates on `61a286f`.

**Selected known native warm-up deadline gate, 2026-11-15 — implemented and locally qualified.**
The native listener-release probe now binds the original 10,000 ms AbortSignal to
exactly one refused canonical Anthropic HEAD. Baseline session-start observation
still sees it pending. An awaiting native control observes its original timeout
and detaches its own listener; per-instance listener observation also passes.
RPC Ready arrives independently, so qualification must inspect the capture bridge's
Ready itself. This is one known native deadline, not complete owner settlement.

Extend the existing preload only: preserve the native timeout's receiver,
arguments and returned signal; observe add/remove on that signal alone. Bind the
SDK callback's exact identity only during synchronous error delivery for the known
request, after its real removal. Await the original timeout event, verify native
TimeoutError and remove the observer before capture Ready. A bounded 12-second
helper deadline must be cleared in every outcome. Missing/duplicate/foreign work,
an unfinished deadline or a repeat seal refuses; a known restart after sealing
exits 75 before IO. Do not replace or shorten the native timer, prime the SDK
throttle cache, swallow exceptions or add a successful network response.

Reuse the existing bounded diagnostic sequence for three closed metadata states:
listener_released, deadline_elapsed, sealed, each naming native_timeout_ms=10000.
Keep 20 attempts and 8192 bytes; allow at most 25 total rows. No new channel,
worker, log, dependency or v1 protocol. This joins the identified signal's deadline
and observers, not the discarded prewarm promise or every native timer.

First require those states at the actual capture-bridge Ready, before any prompt.
The existing code must fail with zero fixture POSTs and normal, reaped cleanup.
Then repair and require the complete owned tool conversation, same native model,
trace mutation controls, actual SDK controls for early/duplicate/missing/unfinished
work and restart, all current/frozen consumers, lint and eight gates. Reuse the
offline hour, 120-second cleanup reserve, one heavy job, 20 GiB disk floor and 8 GiB
sampled individual RSS bound. Retain red/green, publish reviewed public changes,
and prune only verified new build/duplicate extraction caches. Complete startup
admission, paid runs, signing and releases stay gated.

The [dated native-deadline receipt](../../../docs/validation.md#known-native-warm-up-deadline-before-capture-ready--2026-11-15) records actual capture-Ready red/green, five native controls, all current/frozen consumers and eight gates on `13811cf`. This joins the identified signal and observer, not the discarded prewarm promise.
The existing [Rust host](../../../src/pilot_capture/host.rs) materializes the
[preload](../../../src/pilot_capture/startup_probe.ts) and
[bridge](../../../src/pilot_capture/bridge.ts); the
[native Rust consumer](../../../tests/pilot_capture.rs) checks capture Ready.

**Selected project MCP exclusion for the fixed qualifier, 2026-11-15 — implemented and locally qualified.**
The selected diagnostic profile exposes only read/edit tools and has no admitted
project MCP server. Actual SDK controls nevertheless execute an owned project
MCP command with those tool names; disabling the existing
`mcp.enableProjectConfig` setting prevents that execution while preserving the
native model. Set this flag before SDK construction in the explicit owned
qualification configuration only. Ordinary capture and frozen v1 remain unchanged.
This excludes an unadmitted startup owner instead of attempting late teardown.

Source basis is oh-my-pi `24439066040e006eb08da7b204f0ac41c79d2de9`:
`mcp/settings.ts::cfgMcpEnableProjectConfig` (blob
`23e41ba77c247256287ec38a0c46e53ff5dfaa45`) and
`mcp/config.ts::loadAllMCPConfigs` (blob
`1e4f839bd63c630e8d7fa79dbb58bc6de881c50a`). The source excludes project entries
before deduplication/connection. `MCPManager.disconnectAll` clears pending maps
after aborting; that source alone does not prove all pending promises/callbacks
joined. No map snapshot or late disconnect is an admission authority.

Extend the existing actual-launcher settings regression with a private
`.mcp.json` whose owned command only creates a marker inside its workspace.
Require the ordinary profile to execute this positive control and the owned
profile not to execute it. Keep the native model, later disk-edit assertions,
normal reaping and private-mode checks. Before implementation, both profiles must
execute the marker and the owned assertion must fail. No prompt or provider call
is needed. Then require all current diagnostics, frozen compatibility, focused
lint and eight local gates within the offline hour, 120-second cleanup reserve,
one heavy job, 20 GiB disk floor and 8 GiB sampled individual RSS ceiling. Publish
reviewed changes and prune only new verified caches.

This does not remove the pilot's future admitted MCP/task-context integrations or
accept their outcomes. Those require an explicit server/tool inventory and fresh
qualification; no general MCP cancellation, transport coverage, complete startup
settlement or parent acknowledgment is claimed. V2 remains unfunded/unfrozen.

The [dated exclusion receipt](../../../docs/validation.md#fixed-qualifier-project-mcp-exclusion--2026-11-15)
records the actual marker red on `b83a696`, native setting controls, all current/
frozen consumers and eight gates on `ecdf962`.

**Selected credential-prefetch owner check, 2026-11-15 — implemented and locally qualified.**
The pinned `KeyCascade.prefetchUsage` returns before usage selection unless the
observed source is OAuth. A preload observer could not install; its zero counts
are rejected. The existing trusted-extension factory is an earlier supported
hook for this owner: main preloads trusted factories before createAgentSession.
An actual SDK control there observes the original call on the actual namespace,
one non-OAuth source check and original-promise completion before session_start,
with the same complete model as its uninstrumented baseline. Do not substitute a
second invocation or a late empty pending-count snapshot.

Add one small embedded host-extension module for this domain operation. The bridge
factory and real SDK control consume the same implementation; this avoids copying
the owner rule or adding a product test hook. The existing Host materialization
owner binds exact bytes, owner-only mode and digest, only for OwnedQualification.
Ordinary capture/v1 never load it. Hook the actual keys namespace via the shared
AuthStorage class, return original values/arguments/promise unchanged for the
admitted non-OAuth path, and keep only bounded counters and namespace identity.
Never retain keys, headers, account/session identifiers or exception text.
Undefined is this pinned source API's explicit no-configured-auth result;
unrecognized metadata shapes or kinds are not treated as that known absence.

The owned bridge requires exactly one completed original fixed-provider call,
one non-OAuth source observation, no rejection/pending work and the same namespace
before Ready. Unknown metadata or OAuth eligibility, wrong identity, missing/duplicate call,
late installation or an unsettled result fails the existing settings stage.
Block OAuth usage selection at its source-classification boundary, before the
selector can start IO. Once sealed, another prefetch call halts the host. No new
channel, journal, worker, provider route or v1 protocol field is introduced.

Source basis: oh-my-pi `24439066040e006eb08da7b204f0ac41c79d2de9`,
`main.ts::loadTrustedSessionExtensions`/trusted preload before session creation,
`sdk.ts::createAgentSessionScoped` startup prefetch, and
`auth/cascade.ts::KeyCascade::{source,prefetchUsage}` (blob
`cf4e827bf6c631243da445609d392af2d11d8b40`). Source association is not a
reproducible-build proof. Credential refresh/polling, OAuth behavior and all other
owners remain unqualified; this check cannot grant full startup admission.

First require the actual owned capture bridge to fail before its first fixture
request on an implementation that cannot record its original prefetch. Retain
that executable/result. Then implement tracking and require actual SDK controls
for original-call success, missing/late installation, injected OAuth metadata and
restart after sealing; synthetic faults are not OAuth integration evidence.
Require the actual full tool conversation, all current diagnostics, frozen-host
compatibility, focused lint and eight gates. Use the offline hour, 120-second
cleanup reserve, one heavy job, 20 GiB disk floor and 8 GiB sampled individual RSS
ceiling. Publish reviewed changes and prune verified generated caches, preserving
raw evidence and runnable red/green consumers. Paid runs and signing stay gated.

The [dated original-prefetch receipt](../../../docs/validation.md#original-credential-prefetch-owner-qualification--2026-11-15)
records the original-call factory witness, actual-bridge expected red and current/
frozen consumers plus all eight gates on `5ed922b`. This qualification closes
only the original non-OAuth prefetch; full startup admission remains open.

**Selected fixed-profile memory owner check, 2026-11-15 — implemented and locally qualified.**
The native baseline/observer pair selects the actual `offBackend` singleton and
observes one original `start` call with the actual model registry, completed before
session-start readback. Complete model objects match and auto-learn is disabled.
This is the existing no-provider read/edit qualifier's profile, not a policy that
removes future declared memory/task-context integration.

Install one small embedded memory-owner check in the trusted factory before SDK
construction. It must require the actual native Settings instance, `memory.backend`
off and `autolearn.enabled=false`, then observe the actual off-backend original
start without changing its receiver, arguments or returned promise. Require exactly
one completed call on the same Settings/model-registry identities before Ready,
with no rejected/pending work. Unknown/active settings, late installation, foreign
identity, duplicate call or unsettled completion refuse the existing settings stage.
Once sealed, a subsequent start halts the host. No model, loop, generic interceptor,
worker, v1 protocol field or alternative memory implementation is added. The bridge
and real SDK controls consume the same helper; reuse the existing Host byte/mode/
digest owner and factor its now-repeated embedded-source write/readback operation.

First retain an actual-bridge red that cannot observe its original memory start,
with zero fixture requests and normal cleanup. Then require actual SDK success,
late-installation, active-backend/auto-learn eligibility faults, foreign identity,
unsettled completion and restart controls, preserving the complete native model.
Run all current diagnostics, frozen compatibility, lint and eight gates. Use an
offline hour, 120-second cleanup reserve, one heavy job, 20 GiB disk floor and
8 GiB sampled individual RSS ceiling; publish reviewed changes and prune only
verified generated caches while preserving runnable red/green consumers/evidence.

Source association is oh-my-pi `24439066040e006eb08da7b204f0ac41c79d2de9`,
`sdk.ts::startMemoryBackend`, `memory-backend/resolve.ts` (blob
`c38c4aef395a661664c6cd4354fd8d1e272dc525`) and
`memory-backend/off-backend.ts` (blob `f354947d9da408bb9496b9cceba1e8f6d87373f9`).
The no-op start contains no downstream work; this does not qualify other backends,
auto-learn execution, original credential refresh/polling or full startup admission.
`AuthStorage.close()` returns void before an existing refresh drains and its instance
must not be reused; it is not selected as an admission barrier. The native empty
pool/no-broker metadata does not prove every credential callback is settled. The
late telemetry import is unavailable, so no exporter state is inferred from it.
The [dated off-memory receipt](../../../docs/validation.md#fixed-off-memory-startup-owner-qualification--2026-11-15)
records native baseline/observer, actual-bridge expected red, nine SDK memory
controls and all current/frozen consumers plus eight gates on `48822e9`.
V2 remains unfunded/unfrozen; paid runs and signing remain gated.

**Selected fixed-profile telemetry exclusion, 2026-11-15 — implemented and locally qualified.**
The native on/off control now proves the existing `telemetry.otlpExportEnabled`
setting before initialization. With the same owned OTLP endpoint supplied, enabled
startup registers real trace, metric and log providers and delivers three synthetic
markers to an independently instrumented loopback collector. Disabled startup
registers none and sends nothing. Complete native models match; no prompt, provider
request or real credential is used. Earlier address/framing/schema controls remain
retained and are not full three-signal evidence. Late telemetry-module import is
not the observation seam: the compiled registry omits coding-agent's root catch-all
exports. Read actual versioned globals; the qualified logs API selector is numeric
1. Other selector inputs remain unqualified. The collector must handle chunked requests.

Set the existing export flag false in OwnedQualification's private configuration
before host construction, and require its actual native Settings readback to remain
false before bridge Ready. Ordinary capture/frozen v1 retain their configuration.
Reuse the existing launcher, byte/digest owner, registry lookup and settings
quiescence; no new exporter, startup module, proxy, protocol or runtime worker.
This excludes registration of the current fixed profile's exporters; it neither
drains an already active graph nor qualifies telemetry in future admitted profiles.

Extend the real SDK owner control with an owned plain-HTTP OTLP collector using
the existing Hyper library and bounded bodies. Its sole extra route is its exact
numeric loopback address and the three OTLP paths under a test-only preload;
keep provider warm-up refused. Record only route/method, framing, byte count,
digest and marker presence, never headers or telemetry payloads. Obtain current
configuration bytes from the actual Host launcher, then compare explicitly enabled,
unmodified owned and explicitly disabled cases with endpoints held equivalent.
First require the unmodified owned case to fail exclusion while both exporter
controls are valid. Retain that executable, normal cleanup and all native rows.
The green must preserve the complete model and all prior owner/conversation checks,
pass frozen compatibility, lint and eight gates, then publish reviewed changes and
prune only verified generated caches, retaining evidence/runnable red and green.
Use the offline hour, 120-second cleanup reserve, one heavy job, 20 GiB disk floor
and 8 GiB sampled individual RSS ceiling. Paid runs and signing remain gated.

Source association: oh-my-pi `24439066040e006eb08da7b204f0ac41c79d2de9`,
`main.ts::initTelemetryExport` before trusted factories,
`telemetry-export.ts::initTelemetryExport` (blob
`2b14605ed5ca47c66300fe787ba35b4e89ba03c3`),
`telemetry-export-otlp.ts::registerProviders` (blob
`4ab4a333b868fca4ac9d2c24d2a1189566e86831`) and
`scripts/legacy-pi-virtual-module.ts::BUNDLED_PACKAGES` (blob
`46e636cfcf05c365e93955b4a08346d161cd8378`). This is not reproducible-build
proof or complete startup/transport settlement. Credential lifetimes, other owners,
native timers and durable parent acknowledgment remain open; v2 stays unfunded/
unfrozen and execution locked.
The [dated telemetry receipt](../../../docs/validation.md#fixed-profile-telemetry-exporter-exclusion--2026-11-15)
records successful native trace/log/metric deliveries, actual-launcher red/green,
all current/frozen consumers and eight gates on `2bc27c0`.

**Selected fixed-profile logging quiescence, 2026-11-15 — implemented and locally qualified.**
The associated native logger owns a one-second buffered-file timer and an unref'd
retention immediate. A native scoped observer establishes actual pending handles:
enabled logging writes both synthetic markers; setTransports(false/false) flushes
the first and cancels that timer, but returns with retention still pending. One
native immediate turn joins the already-queued retention callback; later SDK log
calls produce no further file writes. All three native controls exit normally with
the same complete model. Preserve the first rejected lazy-construction observation.

Use the existing native logger.setTransports({console:false,file:false}) in the
owned trusted factory before SDK construction, then await one native immediate
turn. Keep the operation in a small first-party extension helper, because native
package imports are rewritten only in the extension graph and the real SDK
controls must exercise the same helper. This adds no logging implementation,
worker, table, service or configuration. Materialize/hash-bind it only for the
artifact-pinned owned qualifier; ordinary capture and frozen v1 keep their logging.
Record its completed operation through the existing preload's metadata owner,
and require it before that owner's final seal and capture Ready. One closed
logging/quiesced row raises the total cap from 25 to 26; retain 20 attempts and
8192 bytes. Missing or duplicate completion refuses. This retires the existing
local file/console transport and its queued retention work. It is not a fence
against arbitrary injected reconfiguration or every host logging implementation.

First require this completion at actual capture Ready: the old launcher must fail
before any Count/Messages fixture call, with normal reaped cleanup. Then require
native enabled/stop-only/actual-owned-helper controls, independent timer/immediate
observations and real marker bytes, the complete actual owned conversation, all
current/frozen consumers, focused lint and eight gates. Run in an offline hour
with 120 seconds for cleanup, one heavy job, 20 GiB disk floor and 8 GiB sampled
individual RSS ceiling. Publish reviewed public changes and prune only verified
new build/duplicate native-library caches, retaining evidence and runnable red/green.

Source association is oh-my-pi 24439066040e006eb08da7b204f0ac41c79d2de9:
utils/src/logger.ts::setTransports/schedulePruneStaleProcessLogs (blob
22c731efb1939c584f091d943338f31db3083071),
utils/src/logger/rotating-file.ts::flush/close (blob
a927106076e5afa3ac89d983f5b375c1e1382649), and the relevant native
utils/test/fixtures/logger-contract-probe.ts contract controls. The verified
program-source census covers 6,816 regular source/manifest blobs; the only
production setTransports import/caller is cli/auth-broker-cli.ts, outside this RPC
path. Literal-reference census is not whole-program absence proof. The native
logger's supported sink also observes the original warm-up catch and a subsequent
microtask, but original promise identity remains unobserved; do not claim it joined.
Bun.plugin is non-writable/non-configurable and redefining the native warm-up export
is rejected; do not replace either. Complete startup/transport admission, paid
runs, signing and release remain gated.

The [dated logging receipt](../../../docs/validation.md#fixed-profile-native-logging-quiescence--2026-11-15) records actual capture-Ready red/green, native timer/retention/marker controls, all current/frozen consumers and eight gates on `a989a1f`. The [Rust host](../../../src/pilot_capture/host.rs) binds the [helper](../../../src/pilot_capture/startup_logging.ts); the [native consumer](../../../tests/pilot_capture.rs) checks its actual effect.

The first gates stop at the disk reserve and are not accepted. Unchanged native,
frozen and lint evidence is reused; after verified cache recovery, all eight gates
pass in a fresh offline-hour continuation with RUST_TEST_THREADS=4. Explicit
concurrency tests and all assertions/deadlines remain intact. The receipt keeps
both stages separate and records the forced cleanup of the interrupted run.

**Selected fixed-profile marketplace exclusion, 2026-11-15 — implemented and locally qualified.**
The actual current SDK ran refreshStaleMarketplaces and checkForUpdates with the
prior owned configuration, whose initial marketplace.autoUpdate defaulted to
notify. Both original promises complete in the native observation. Explicit off
excludes those calls with the same complete model, two state readbacks, empty
stderr and normal exit. A trusted-factory change from notify to off still runs
both calls: main captures the options before trusted factories. These private
no-prompt controls are feasibility, not full startup admission.

Set the existing marketplace.autoUpdate to off in the launcher's initial
OwnedQualification configuration, before main captures it. Verify the actual
native setting remains off in the bridge's existing final settings readback before
Ready. Ordinary capture and frozen v1 keep their configuration. Reuse the existing
launcher, byte/digest binding, registry lookup and settings quiescence. Add no
startup owner, module, worker, table, transport or protocol. Excluding this fixed
profile's unadmitted update task does not accept arbitrary marketplace use or
drain an already running task.

First extend the existing native owner regression using configuration from the
real Host launcher: explicitly notify, unmodified owned, explicitly off and a late
trusted-factory off override. A test-only observer wraps the actual exported
MarketplaceManager methods through process exit, preserves receiver, arguments
and original promise returns, and checks completion. The old owned case must fail
exclusion after valid positive/off/late controls, with normal reaped cleanup, no
prompt and network denied. Retain that runnable red. Then require repaired native
controls, the complete owned two-generation conversation, all five current
diagnostics, the complete nonignored frozen suite, focused lint and all eight
gates. Use one offline hour, 120-second cleanup reserve, one heavy job, four outer
test threads, a 20 GiB disk floor and 8 GiB sampled individual RSS ceiling; retain
all failures. Publish reviewed public changes to experimental, then prune only
verified new build/duplicate extraction caches while retaining evidence and
runnable red/green. Paid runs and release signing remain gated.

Source association: oh-my-pi 24439066040e006eb08da7b204f0ac41c79d2de9,
main.ts::marketplaceAutoUpdateOptions/scheduleMarketplaceAutoUpdate (blob
2bfe0b7230e9b3e30aebfd493a03bffb652cdfb3),
modes/settings.ts::cfgMarketplaceAutoUpdate (blob
f70b96cbab595a105f735f61eaac22fe065d6575), and
extensibility/plugins/marketplace-auto-update.ts::scheduleMarketplaceAutoUpdate
(blob 4f589ea7aaa636e6970ebdc56d3aa6c9b12388be). The off branch returns before
dynamic import/construction; the other branch discards its async task. The native
controls observe actual manager calls, not a quiet network proxy. Source
association is not reproducible-build proof. Other startup owners, complete
timer/transport coverage and durable parent acknowledgment remain open; v2 stays
unfunded/unfrozen and execution locked.

The [dated marketplace receipt](../../../docs/validation.md#fixed-profile-marketplace-update-exclusion--2026-11-15)
records the actual-launcher red on `7dab0cd`, original-call controls, all five
current diagnostics, frozen compatibility and eight gates on `7863a44`. The initial
disk-stopped build and private model-pin correction are retained separately.
The [launcher](../../../src/pilot_capture/host.rs) owns the initial setting; the
[bridge](../../../src/pilot_capture/bridge.ts) checks native readback, and the
[native consumer](../../../tests/pilot_capture.rs) observes actual SDK calls.
This excludes the fixed qualifier's task; it does not drain an active marketplace
or establish full startup admission.

**Selected native startup watchdog qualification, 2026-11-15 — qualified 2026-11-16.**
The actual pinned host's original 10,000 ms interval is pending in the early
trusted factory and its exact handle is cleared by the initial session-start readback. The callback
and timer arguments are passed unchanged. Holding that factory for eleven seconds
lets the actual callback print its startup notice before ordinary cancellation;
two native state readbacks and normal exit still succeed. Source already stops
the watchdog before entering RPC; no runtime repair is selected.

Add test-only preload/witness controls to the existing native owner regression:
ordinary startup, delayed factory and deliberately suppressed native cancellation
for this exact identified handle. The last must keep its real interval pending
and emit at least a second notice. Require the callback's observed fingerprint,
initial pending handle, one cancellation attempt, actual cancellation only in the
healthy cases, snapshots at handler entry before any registry await, and a second equal native model readback
after a 22-second hold. Keep all existing deadlines; these new cases have their
own 45-second bound. Parse only the exact paired native notice/log-path format;
other controls still require empty stderr. No callback is invoked manually or
replaced, no successful connection is added, and no product timer hook or protocol
is introduced. This qualifies one original owner, not the complete inventory.

Run the complete current native owner controls, focused lint and eight gates.
Reuse the existing conversation/settings/native-usage and frozen33/0/7 evidence
only after checking all application/bridge/launcher/gate inputs and every other
consumer body are byte-identical; the changed helper is called only by the owner
test. State reused evidence explicitly. Use an offline hour, 120-second cleanup
reserve, one heavy job, two outer harness threads, 20 GiB disk floor and 8 GiB
sampled individual RSS ceiling. Internal concurrency assertions remain intact.
Publish reviewed public tests/records to experimental, then prune only verified
new build/extraction caches and rerun the owner controls directly from the retained
executable. Preserve all evidence; paid runs and release signing remain gated.

Source association is oh-my-pi 24439066040e006eb08da7b204f0ac41c79d2de9,
main.ts::{startStartupWatchdog,armStartupWatchdog,stopStartupWatchdog,runRootCommand}
(blob 2bfe0b7230e9b3e30aebfd493a03bffb652cdfb3). The native callback fingerprint is
749865907ec3a454d6eadc0146e603c933a980d0863ae36cd253682f311bcccc.
The [Foundry native consumer](../../../tests/pilot_capture.rs) owns the durable
test; no directly named watchdog test was found by the bounded literal search of
the retained coding-agent test sources, and no upstream test was run. Source
association remains separate from reproducible-build proof.

The promise-hook feasibility control cannot identify original promises on this
artifact. node:async_hooks.createHook enables but emits no init/resolve events even
for the controlled Promise; node:v8.promiseHooks.createHook throws
NotImplementedError. Neither API presence nor zero events proves no original
work. Do not introduce a generic promise tracker or claim a join. The original
credential resolver/settings-sync/refresh lifetimes, discarded warm-up promise,
full transport coverage and durable parent acknowledgment remain open; v2 stays
unfunded/unfrozen and execution locked.

The [dated watchdog receipt](../../../docs/validation.md#native-startup-watchdog-and-promise-hook-limits--2026-11-15)
qualifies exact handler entry on `b9c7104` with original [preload observation](../../../tests/pilot_capture/watchdog_probe.ts),
[SDK witness](../../../tests/pilot_capture/watchdog_witness.mjs) and the native
consumer. The initial after-await readback and complete eight-gate run stay
separate; five unaffected checks are reused, with three affected checks fresh.
The helper also retires only hash/mode/owner-verified generated libraries after
reaping, preserving all other private output checks. No application source changes.

**Owners and transport.** Reuse `CaptureSession`, `Host::launch`, the existing
private channel and hash-chained journal. Freeze the artifact/source association
and a complete startup-owner inventory: catalog/discovery, credential refresh,
connection preparation, telemetry/update, MCP/memory startup, settings watching
and extension initialization where present.
These are inspection categories, not an asserted complete current inventory.
Unknown owners and bypass routes fail closed. Install observation before any owner
captures a transport reference. The preload and later bridge share one child
channel/sequence owner; do not open a second connection or reset its sequence.
Do not add a worker, proxy, service, database or independent log.

Source inspection at `24439066040e006eb08da7b204f0ac41c79d2de9` distinguishes
`main.ts::buildSessionOptions` assigning `toolNames` for `--tools` from
`sdk.ts::createAgentSessionScoped` guarding MCP and memory work with the separate
`restrictToolNames` option. Tool names alone do not establish restricted startup.
RPC also starts settings watching, and telemetry initialization has settings/env
eligibility. Trace the final selected options, backends, owners and lifetimes
before freezing the inventory; this source census proves neither active model
work nor absence of these owners at runtime. The known warm-up source constructs
an abort deadline, whose timer/callback settlement is not qualified by the Agent
TCP control or a quiet trace.

The [native HTTPS controls](../../../docs/validation.md#native-https-startup-boundaries--2026-11-15)
now demonstrate a concrete alternate SDK path: Anthropic connection warm-up uses
`coworkFetch`/`node:https.request` outside global fetch in the diagnostic profile.
Replacing the default HTTPS export does not replace the namespace call on the
owned runtime. A `ClientRequest.end` refusal still permits an independently
observed loopback connection, so neither patch qualifies the required before-IO
boundary. Existing `PI_PROXY_ANTHROPIC` routing exposes the known warm-up to global
fetch, but the direct-HTTPS bypass remains. It is a configuration candidate, not
a selected fence or new proxy service. Qualify every admitted owner/route before
construction/IO at the actual bridge/profile boundary; do not enable admission
from a zero-fetch snapshot or the narrow routing control. Ordinary configuration,
frozen v1 and execution authority remain unchanged.

A future closed protocol revision carries metadata-only messages, using existing
identity, sequence, length and journal limits:

- Child `startup_attempt`: host/session identity, unique bounded attempt identity,
  declared frozen owner, bounded method and destination classification. Keep only
  scheme/host/port and a frozen route class; never credentials, bodies, query strings
  or raw error text. Unclassifiable input is refused with bounded private identity.
- Parent `startup_refused`: binds that exact attempt after its refusal is appended
  and synced. The child rejects locally without starting network IO. This message
  grants no provider route or generation permission.
- Child `startup_settled`: binds the inventory digest and terminal state of every
  owner. It is valid only after their work, callbacks, timers and outstanding
  refusals are drained or cancelled and those owners cannot start again. Required
  owner controls must prove this; a wrapper's pending-count snapshot is insufficient.
- Parent `startup_ack`: binds the durable startup-prefix journal sequence/digest,
  launch identity and owner inventory. The subsequent child `Ready` echoes this
  exact digest alongside ordinary settings/model/tool evidence. Enqueueing the ack
  is not host acknowledgment; the matching echo is required.

These are proposed fields in a new closed revision, not optional/unknown-message
exceptions for v1. Frozen v1 protocol, pin, policy and historical receipts retain
their meaning. Finite startup wall/attempt/metadata limits are required freeze
inputs within existing bounds; missing values receive no permissive default.

**Ordering and failure.** Rust starts unready. It classifies and durably refuses
every attempt, verifies complete owner settlement, appends/syncs/readbacks the
prefix, then sends the ack. Only a matching Ready echo and ordinary settings
readback can enable preview/count/generation admission. No per-generation counter,
usage reservation, permit or provider transmission is available earlier. A startup attempt after
settlement halts the host; it cannot silently begin another discovery pass.

The journal owns durable truth. A dropped frame, unknown owner, premature fetch,
append/sync/readback failure, overflow, timeout, EOF or digest/identity mismatch
halts and reaps the host through existing cleanup. If halt persistence also fails,
return the persistence error and publish no successful seal. Preserve consumed
authority and unknown spend. Restart uses fresh launch/channel/prefix identities;
journal replay cannot create a new execution authority. No fallback to v1 or an
unobserved route can convert failed v2 startup into readiness.

**Qualification.** Protocol/state tests reject ordering, replay, duplicate, stale,
conflicting, truncated and exhausted inputs. Actual-host controls exercise every
frozen owner, ignored abort, late callback and a transport bypass. Independently
instrument the owned counter/Messages fixture: incomplete-startup and crash/write-
failure controls between refusal, prefix sync, ack send and Ready echo must observe
zero calls and no permit. Retain normal cleanup, destination classifications,
private-mode checks and journal readback. A scripted child or the present RPC-only
counterexample cannot qualify this complete parent/host boundary. Freeze inventory,
owner drain controls, source/artifact pins and this revision before v2 readiness;
paid authority, task/sample freeze and signing remain separate gates.

| Disclosed gap | Required evidence |
| --- | --- |
| Host startup network denials absent from journal | Capture denied startup attempts and their destination/classification before any paid request; disallow unobserved routes. A sandbox denial is not evidence of journal completeness. Matching-release source establishes only partial diagnostics; a complete early observer and completion boundary remain needed ([source finding](../../../docs/validation.md#pilot-startup-network-source-feasibility--2026-11-11)). |
| Checker binds environment names only | Bind exact sanitized environment values, sandbox profile bytes and freshly hashed oracle/compiler bytes to each checker receipt. Credentials are absent from the checker environment. The selected local checks pass ([record](../../../docs/validation.md#pilot-checker-provenance-and-retained-diagnostics--2026-11-11)); the limits in the selected repair below and broader v2 qualification remain. |
| Compile stderr only hashed | Retain bounded private stderr bytes and their digest, with explicit overflow/failure behavior. Inspect errors; a matching binary hash cannot explain omitted diagnostics. The selected local checks pass ([record](../../../docs/validation.md#pilot-checker-provenance-and-retained-diagnostics--2026-11-11)); the limits in the selected repair below and broader v2 qualification remain. |
| Coding authority outside chain; declared/actual paths differ | Append the authorization digest to the durable chain before a paid attempt and require the canonical actual run root to equal the authorized root. The selected metadata/path and chained-authority fixtures pass ([record](../../../docs/validation.md#pilot-authority-and-run-path-binding--2026-11-11)); historical v1 evidence is unchanged. Current installed-host compatibility and actual Live-compatible probe history remain unproved. |
| Some private artifacts have wider permissions | Enforce and verify 0700 directories/0600 private files, including host/store outputs; fail before transmission if this cannot be established. Executables need explicitly declared private executable modes. The selected local metadata, child-launcher, lexical-store, pinned-host and actual-checker checks pass ([record](../../../docs/validation.md#pilot-private-output-permissions-and-terminal-publication--2026-11-11)); the real semantic-producer check qualifies its closed outputs ([record](../../../docs/validation.md#pilot-semantic-producer-permissions--2026-11-13)); the frozen-host private pipeline also passes after closed-preview archival ([record](../../../docs/validation.md#private-full-pipeline-and-preview-archival--2026-11-13)). Transient worker scratch-file modes, current installed-host compatibility, v2's declared task/arm/host qualification and complete startup-network observation remain unqualified. |
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
The later real semantic-producer check qualifies closed store/output modes after
indexing, preparation and context through the production bounded launcher; its
ignored test uses explicit local inputs without replacing v1's executable pin
([receipt](../../../docs/validation.md#pilot-semantic-producer-permissions--2026-11-13)).
The later frozen-host full-pipeline regression also passes with the exact recovered
executable and explicit closed-preview archival
([receipt](../../../docs/validation.md#private-full-pipeline-and-preview-archival--2026-11-13)).
The initial supplied-path mismatch and first consumer failure remain history.
The shared worker pre-exec setup also enforces umask 0077 in the child; selected
direct fake-worker writes, the pinned native write probe and normal local model
calls pass under caller masks 0000/0022, without changing the caller mask
([receipt](../../../docs/validation.md#worker-scratch-creation-defaults--2026-11-13)).
That checks default creation and named writes, without enumerating or normalizing
every transient library file. Complete transient-file modes, current installed-host
compatibility and v2's declared task/arm/host qualification stay open. These checks
neither authorize a paid request
nor freeze or accept pilot v2/T006; startup-network observation remains separate.

**Preview archival repair, 2026-11-13 — qualified on the frozen-host no-provider pipeline.** The
first fresh full-pipeline regression reaches capture admission after four real
views and nine previews, then correctly refuses the render hosts' retained 0700
native modules outside the declared capture-run exception. A successful preview
must confirm that its host group is gone, validate its newly owned tree under the
existing live-run rules, and archive only its exact native-module copies as 0600
data. Preserve bytes; use checked descriptors, sync file and parent, validate the
closed tree with ordinary data rules and append `preview_archived` before returning
the binding. Wrong-mode inputs, shared native-file inodes, unknown executables,
unresolved cleanup or failed
persistence refuse. Active capture exceptions and frozen pins/fields stay unchanged;
historical artifacts are not normalized. The retained original failure, four focused
checks, fresh full consumer run and eight gates establish this selected boundary
([receipt](../../../docs/validation.md#private-full-pipeline-and-preview-archival--2026-11-13));
no paid execution is authorized.

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
