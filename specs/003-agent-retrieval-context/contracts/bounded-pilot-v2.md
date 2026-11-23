# Bounded pilot v2 — unfunded draft

Draft date: **2026-11-11**. **Not frozen; execution locked.**
`execution_authorized=false`; `execution_ready=false`.

This is the draft selected by owner decision 1 on 2026-11-06. It owns the v2
design; [T006](../spec.md#t006--validate-retrieval-repairs-in-real-coding-tasks)
owns acceptance and the [handoff](../../../docs/resume-context-ecosystem.md#current-status--2026-11-06)
owns current status. It does not reuse v1's credential, budget, launch manifest or
four-category usage exception. [V1](bounded-pilot-v1.md) remains byte-for-byte frozen.
No provider request, credential access, training or execution is authorized here.

The owner's decisions of 2026-11-19 on host memory, `PI_PROXY_ANTHROPIC`, host
identity, reference patches, private-output protection and presence cleanup are
recorded where each topic is discussed
([record](../../../docs/validation.md#owner-decisions--2026-11-19)). They neither
fund nor freeze this draft. The same applies to the owner's decisions of 2026-11-20
(rows 18–20 of that record): "Drop retrieval arms" (item 3), "Disable in every arm"
for the host's implicit `apple` provider (item 5) and "Implement and test it" for the
variance-based sample method
([sample design proposal](#proposed-sample-and-statistical-design--2026-11-20)).

## Outcome and claim boundary

Measure verified coding outcomes on several harder, previously unused tasks,
with evidence-discovery and complete-request economics. V1's single known repair,
repeated four times, cannot provide that evidence. T010's lexical default is
decided separately by its own local gates and G2; this pilot does not gate it or
retroactively establish an agent benefit for it. Owner decision, 2026-11-20, "Drop
retrieval arms": pilot v2 tests task context only and measures no retrieval change
(item 3).

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
3. Owner decision, 2026-11-20, "Drop retrieval arms"
   ([record](../../../docs/validation.md#owner-decisions--2026-11-19)): pilot v2
   tests task context only and has no retrieval arm or retrieval contrast,
   confirmatory or descriptive. This replaces T006's earlier retrieval comparison of
   current Foundry, mixed-intent repair, anchorless repair and both repairs. 009 T005
   closed without promotion (decision 17, 2026-11-19), which left no anchorless repair
   to bind, and 001 T010's mixed-intent repair is already current Foundry's default.
   Every arm runs the same current Foundry retrieval configuration, with T010's
   behavior selection on, pinned by digest; no treatment changes that configuration. The four
   intents of item 2 stay as task strata and are reported per intent. T006's
   acceptance is amended to match
   ([T006](../spec.md#t006--validate-retrieval-repairs-in-real-coding-tasks)). Bind
   actual implemented configurations; an unavailable configuration is a readiness
   blocker, not an arm that can silently be dropped. Reuse one baseline where the
   configurations and tasks permit it.
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
   Owner decision, 2026-11-19, "Off in every arm"
   ([record](../../../docs/validation.md#owner-decisions--2026-11-19)): the host's
   memory backend stays off (`memory.backend` is `off`) and auto-learn stays
   disabled (`autolearn.enabled=false`) in every v2 arm, so no treatment enables
   them. Other memory backends and auto-learn execution are out of scope. This
   covers the host's own memory, not Foundry's 008 task context that item 4
   compares.
   Owner decision, 2026-11-20, "Disable in every arm"
   ([record](../../../docs/validation.md#owner-decisions--2026-11-19)): every v2
   launch disables the host's implicit `apple` provider, Apple's on-device model, by
   listing it in the host's `disabledProviders`. The reason is the
   [crash diagnosis](../../../docs/validation.md#sandbox-profile-segmentation-fault-diagnosis--2026-11-20)
   of the 2026-11-15 settlement runs: all five segmentation faults under ad-hoc
   sandbox profiles had one stack, in the host's check for that model, and their
   sandbox cause is inferred, not confirmed. The owner chose this over binding the
   check's availability result as a pinned startup input.
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
for their declared capability treatment. Under owner decision 18 of 2026-11-20,
"Drop retrieval arms" (item 3), the matrix has no retrieval row, and every row holds
retrieval at the one current Foundry configuration; the "source candidate
configuration" of the 003 T008 row is that configuration. V2 still needs concrete
tasks/oracles and rights, current-host qualification, the selected statistical design
and launch pins; this matrix supplies none of those approvals.

### Proposed arm and interface bindings — 2026-11-20

**Proposed, 2026-11-20, not frozen.** This is a proposal for the owner, which the
implementer wrote against commit `afa7b38`. It does not fund, freeze or authorize
anything, and it does not choose task-data rights, a budget, a credential or the
final sample; those remain the owner's. Paths are repository-relative. Line numbers
cite files at `afa7b38`; for this draft, they cite its text before the 2026-11-20
proposals were inserted. It proposes content for these parts of this draft:

- item 3, lines 45–48: bind the retrieval arms to implemented configurations
  (settled on 2026-11-20 by owner decision 18, "Drop retrieval arms": v2 has none;
  see [Retrieval arms](#retrieval-arms-item-3));
- item 4, lines 49–54, and the capability-to-contrast matrix, lines 71–94: bind each
  contrast's prompts, tools, flags and endpoint, and name which contrasts stay
  descriptive (lines 88–90);
- item 5, lines 55–66: equal tool authority and host memory off;
- lines 820–822 and the "Tools, MCP, memory and credentials" startup row, line 1827:
  the explicit server and tool inventory;
- the "implemented arm matrix" of the protocol-preparation gate, lines 2174–2177.

#### What exists today

1. **The capture apparatus admits exactly two host tools.** `policy::TOOL_NAMES` is
   `["read", "edit"]` (`src/pilot_capture.rs:53-54`). The final-request validator
   refuses any other tool set with `tool_set_mismatch` and admits only the tool keys
   in `TOOL_KEYS` (`src/pilot_capture/wire.rs:99-106`, `:633-653`). The bridge pins
   the same names and activates them (`src/pilot_capture/bridge.ts:41`, `:498-505`,
   `:528`); the host argv passes `--tools read,edit` (`src/pilot_capture/host.rs:460-461`).
   The startup receipt requires `mcp_disabled` and exactly `["edit", "read"]`
   (`src/pilot_capture/startup.rs:252-266`).
2. **The one existing evidence seam is the initial-user slot.** V1's arm B received a
   Foundry view, made by the CLI before any provider request, after the framing
   `\nInitial repository evidence:\n` (`src/pilot_capture/apparatus.rs:79-80`;
   `arm_prompt`, `:856-860`; v1 rules in `bounded-pilot-v1.md:67-76`). The allocation
   admits only arms A and B in pairs 1–4 (`src/pilot_capture/apparatus.rs:842-851`).
3. **The Foundry MCP server** is `foundry --store STORE mcp --root ROOT [flags]`
   (`src/adapter_cli.rs:21-96`). Its tools are defined in `src/mcp/tools.rs`:
   `search` (`:208`), `context` (`:292`), `retrieve` (`:556`), `index` (`:678`),
   `references` (`:1081`), `status` (`:1214`), `task_context` (`:1331`) and `memory`
   (`:1383`). `task_context` is removed unless `--experimental-task-context` is given
   (`:132-134`); `memory` is removed by `--no-memory` (`:188-190`). With the task flag,
   `context` gains `task_id`, `task_revision`, `compose_evidence`, `compact_evidence`
   and `boundary_config` (`:135-150`), and `index` gains `boundary` (`:151-158`);
   without it a task-aware `context` call refuses (`:389-395`). `include_briefs` and
   `index.knowledge` appear only with `--knowledge-profile` (`:159-187`). The comments
   at `src/mcp/tools.rs:111` ("exactly five tools") and `src/adapter_cli.rs:12-14`
   ("seven") are stale: eight tools are defined.
4. **On MCP, server flags cannot separate several treatments.** One `task_context`
   schema advertises every operation: create, apply, inspect, peek, render, forget,
   advise, review, promote and search (`src/mcp/tools.rs:1333`); no flag removes one.
   `compose_evidence` is an argument the model chooses. So flags alone cannot give
   one arm peek without search, passive context without review, or composition off.
5. **The Foundry CLI exposes the same choices as exact flags.** `context`
   (`src/cli.rs:83-135`) takes `--experimental-behavior-context[=false]` (`:90-94`),
   `--task` and `--task-revision` (`:97-102`), `--compose-evidence` (`:103-105`),
   `--compact-evidence` (`:106-108`), `--boundary-config` (`:109-111`),
   `--include-memory` (`:112-114`), `--include-briefs` (`:115-117`),
   `--semantic-profile` (`:118-121`) and `--development-isolation` (`:128-130`).
   `index` is at `:51-53`, `status` at `:217-220`, and `task-context` (one JSON
   request on stdin) at `:238-239`; its operations are listed in
   `docs/experimental-context.md:597-605`.
6. **The mixed-intent repair (001 T010) is already the default.**
   `DEFAULT_BEHAVIOR_CONTEXT` is `true` (`src/store.rs:1661`); the opt-out is
   `--experimental-behavior-context=false`. A supplied dense window disables it at the
   engine boundary (`specs/001-source-state-recovery/spec.md:585-587`).
7. **No anchorless repair is promoted.** 009 T005 closed without promotion (owner
   decision 17, 2026-11-19; `specs/009-optional-semantic-retrieval/spec.md:1374-1386`).
   `--semantic-profile FILE --development-isolation` serves 009 T004's baseline; it is
   not a T005 repair.
8. **Host-acknowledged task-view application exists only in `foundry gateway`**
   (`task_view` and `task_view_owner`; `docs/experimental-context.md:1292-1360`), a
   loopback forwarder for a different host and model profile (`src/adapter_cli.rs:169-176`).
   This draft forbids a new proxy (lines 228–229), and applying a view through ordinary
   MCP is a named limitation (owner, 2026-11-19;
   `specs/008-scoped-durable-memory/spec.md:474-481`).
9. **OMP can expose Foundry tools in two forms.** Foundry's usage import and routing
   hook recognize first-class tools named `mcp__context_foundry_<tool>`
   (`tests/fixtures/usage/omp-session.jsonl:12`;
   `specs/003-agent-retrieval-context/spec.md:212-214`). OMP 18.4.9 was observed using
   devices `xd://mcp__context_foundry_<tool>`, which are read with `read` and called with
   `write` (`docs/review/real-host-t003-2026-10-01.json:94`; `src/usage.rs:49`). Which
   form the v2 host uses under a restricted `--tools` list is unobserved.
10. **Missing apparatus pieces:** no in-slot host restart or session resume and no
    review seam exist in `src/pilot_capture/host.rs` or
    `src/pilot_capture/apparatus.rs`. A per-call tool check does exist: the trusted
    bridge's `tool_call` handler asks the parent (`src/pilot_capture/bridge.ts:580-612`,
    `:709`), which records a `tool_check` row and allows or blocks the call
    (`src/pilot_capture.rs:3715-3754`) using `host::admit_tool`
    (`src/pilot_capture/host.rs:1333-1337`). That function refuses every tool except
    `read` and `edit` with `tool_not_allowed`; it has no rule for Foundry tools or for
    `task_context` operations. OMP offers tool-call interception only to in-process
    extensions (`specs/003-agent-retrieval-context/spec.md:198-200`); the bridge is
    loaded as a trusted extension (`src/pilot_capture/host.rs:472-473`).

#### Two delivery bindings

- **Slot binding (S).** Foundry runs as a CLI on the operator side, before the host
  starts and outside its sandbox. Its exact output fills the initial-user slot after
  the existing framing. The host keeps exactly `read` and `edit`, and no MCP server is
  admitted. This generalizes v1's arm B. It exists today except for the multi-arm
  allocation. Every slot is made before the first provider request, and its bytes,
  SHA-256, Foundry argv, binary digest and store revision are pinned in the launch
  manifest, as v1 pinned its views.
- **MCP binding (M).** The host launches one admitted Foundry stdio MCP server and
  exposes its tools to the model. The apparatus does not admit this today; the work
  it needs is listed below.

Placeholders: `FOUNDRY` is the pinned Foundry binary, `WS` the slot's canonical
workspace, `HOSTROOT` the slot's private host root, and `STORE` its fresh Foundry
store. Under S the store sits outside both the workspace and the host root; under M
it is `HOSTROOT/foundry-store`. `B` is the frozen `render` byte cap.

#### Held fixed in every arm

- **Host.** One pinned host, identified by its executable SHA-256 plus the ordered
  patch list, dependency lock, Bun runtime and native-addon digests (owner decision 12
  of 2026-11-19; [record](../../../docs/validation.md#owner-decisions--2026-11-19)).
  The reference patches stay reference data (decision 11). The model, thinking and
  effort, settings, request and time limits, task instructions and initial facts are
  equal within a task block (item 5); v1's values are not v2 defaults (lines 274–275).
  Host argv as `omp_args` (`src/pilot_capture/host.rs:451-475`), except `--tools`
  under M.
- **Host memory off in every arm (owner decision 9).** `memory.backend` is `off` and
  `autolearn.enabled=false`. The off-memory owner check
  (`src/pilot_capture/startup_memory.ts:18`, `:46-47`) loads in every v2 arm, not only
  in the owned qualifier (`src/pilot_capture/host.rs:49`). This covers the host's own
  memory; Foundry's 008 task context is a treatment here (lines 60–66).
- **Implicit `apple` provider disabled in every arm (owner decision 19, 2026-11-20,
  "Disable in every arm"; item 5).** Reason: the
  [crash diagnosis](../../../docs/validation.md#sandbox-profile-segmentation-fault-diagnosis--2026-11-20).
  The startup-observation and owned profiles already list `apple` in
  `disabledProviders` (`QUALIFICATION_DISABLED_PROVIDERS`,
  `src/pilot_capture/host.rs:156`). Ordinary capture, `CatalogProfile::HostDefault`,
  writes no disabled list (`:558`) and still runs the check on macOS, so the v2
  launch profile must add it. That is not implemented. The launch manifest would bind
  the list and the config digest.
- **`PI_PROXY_ANTHROPIC` unused (owner decision 10).** The host environment is
  cleared and rebuilt (`src/pilot_capture/host.rs:709`); `host_env` does not set it
  (`:477-507`). The manifest binds its absence.
- **Foundry.** One release binary pinned by SHA-256. A fresh store per slot, made with
  `FOUNDRY --store STORE index WS` (`src/cli.rs:51-53`) from the restored seed at the
  slot's canonical workspace path. No semantic profile, policy config, knowledge
  profile, `--reference`, SCIP import, `--watch` or Foundry project memory (`--no-memory`
  under M; no `--include-memory` under S). No `--retrieval-trace-dir` in timed runs;
  attribution runs stay separate (`specs/003-agent-retrieval-context/spec.md:845-850`).
  `--experimental-behavior-context=true` is passed explicitly, so a later default change
  cannot alter an arm. The descriptive pre-repair arms that were the exception are
  dropped (owner decision 18, below), so it is passed in every arm.
- **Query `Q`.** One frozen query per task, hashed with the task inputs (item 1).
  Proposal: the task's issue text verbatim, at most 4,096 bytes, the MCP query limit
  (`src/mcp/tools.rs:400-406`), so S and M accept the same text.
- **Slot cap `N`.** One o200k token cap for every slot arm, frozen with the request
  limits. V1 used 2,048 (`src/pilot_capture/apparatus.rs:78`). Its value affects cost
  and is a freeze choice.
- **Task record** (task-context arms only). Before the slot the operator runs
  `FOUNDRY --store STORE task-context` with `create`, then one `apply` holding the
  task's frozen initial facts: requirements, constraints, observations, rejected
  attempts, questions and work, with author `harness` and provenance naming the task
  input digest. Folds used by recovery cases are frozen edits in the same `apply`. The
  same facts appear as plain text in every arm's instructions. `W` is the workspace ID
  from `FOUNDRY --store STORE status`, `T` the stable task ID (letters, digits, `_` and
  `-`, at most 128 bytes; `src/mcp/tools.rs:362-372`), and `R` the revision `apply`
  returns, recorded in the manifest.
- **Preparation check.** All slot arms are prepared for every task before the freeze.
  A Foundry refusal, for example a task view that does not fit `N`, is resolved then,
  before any model output exists, and never by dropping a task afterwards.

#### Arm configurations

| Arm | Binding | Exact Foundry configuration | Host tools in the final request | Exists today |
| --- | --- | --- | --- | --- |
| A0, host only (dropped, decision 18) | none | None. The slot holds a fixed neutral marker, `(none)`, under the same framing. | `read`, `edit` | yes |
| A1, lexical slot | S | `FOUNDRY --store STORE context "$Q" --tokens N --experimental-behavior-context=true` | `read`, `edit` | yes |
| A1-off, pre-repair slot (dropped, decision 18) | S | A1 with `--experimental-behavior-context=false` | `read`, `edit` | yes |
| A2, separate task and source slot | S | The `render` output, then a `context` output; commands below | `read`, `edit` | yes, except the split rule |
| A3, joint slot | S | `FOUNDRY --store STORE context "$Q" --task T --task-revision R --tokens N --experimental-behavior-context=true` | `read`, `edit` | yes |
| A4, composed slot | S | A3 plus `--compose-evidence`, and `--boundary-config CFG` only for a task with a frozen indexed boundary configuration | `read`, `edit` | yes |
| M0, ordinary Foundry tools | M | Server argv: `FOUNDRY --store HOSTROOT/foundry-store mcp --root WS --no-memory --experimental-behavior-context=true --budget HOSTROOT/foundry-budget.json` | `read`, `edit` and six Foundry tools: `search`, `context`, `retrieve`, `index`, `references`, `status` | no |
| M0-off (dropped, decision 18) | M | M0 with `--experimental-behavior-context=false` | as M0 | no |
| M1, task-context tools | M | M0 plus `--experimental-task-context` | as M0 plus `task_context` | no |

Owner decision 18 of 2026-11-20, "Drop retrieval arms" (item 3), leaves only arms
that a task-context contrast uses. A1-off and M0-off measure the pre-repair retrieval
state, and A0 compares current Foundry retrieval with none, so all three serve only
retrieval contrasts and are dropped. Their rows stay as the record of this proposal.
A1 stays as item 4's ordinary control. No contrast below uses M0 as an arm; it
remains M1's base configuration.

A2's slot is made by two commands. First
`printf '%s' '{"workspace_id":"W","task_id":"T","command":{"op":"render","max_bytes":B}}' | FOUNDRY --store STORE task-context`,
then `FOUNDRY --store STORE context "$Q" --tokens N_src --experimental-behavior-context=true`,
where `N_src` is `N` minus the render output's tokens. Those tokens are counted with
the same locked o200k encoding Foundry packs with; the freeze names the counting
implementation and its digest. Every slot arm therefore has the same total cap `N`.
`render` refuses rather than truncate a protected entry
(`specs/008-scoped-durable-memory/spec.md:267-269`). `--compact-evidence` is absent in
A4, keeping the verbose accounting default. `--boundary-config` requires
`--compose-evidence` (`src/cli.rs:109-111`), so it can only belong to A4's treatment.

M's budget file is `{"foundry_budget":{"v":1,"max_context_tokens":C_call,"session_context_tokens":C_session}}`
(`src/config.rs:107-130`; `C_call` is at most 32,768). `C_call` and `C_session` are
freeze values set with the request limits, identical in every M arm. Foundry's delivery budget does not
control the host request (AGENTS.md); the capture's admission limits do.

#### Retrieval arms (item 3)

**Settled, 2026-11-20.** Owner decision 18, "Drop retrieval arms"
([record](../../../docs/validation.md#owner-decisions--2026-11-19)), removes all four
arms below and every descriptive retrieval arm: v2 tests task context only (item 3).
Current Foundry stays as the one retrieval configuration in every arm, not as an arm
of its own. The table and consequence are kept as the analysis the decision answered.

| Arm in item 3 | Binding | Proposed status |
| --- | --- | --- |
| Current Foundry | A1 under S, M0 under M | Exists under S. |
| Mixed-intent repair alone | Identical to current Foundry since 2026-11-11; no separate configuration exists. A1-off or M0-off measures the pre-repair state instead. | Descriptive only: this pilot neither gates T010 nor establishes an agent benefit for it (lines 23–25). |
| Anchorless repair alone | None promoted (fact 7). | Readiness blocker (lines 46–48). |
| Both repairs | None promoted. A dense window also disables the behavior selection (fact 6), so a semantic arm with and without it is the same arm on queries that use the dense window. | Readiness blocker. |

Consequence: with today's interfaces T006's selected comparison has no retrieval
change that v2 could accept. T010 is excluded by lines 23–25 and T005 closed without
promotion. Unless the owner changes this, T006 stays open after v2. The owner's
options for the two blocked arms are listed at the end. The owner has since changed
it: with decision 18, T006's acceptance no longer asks v2 for a retrieval change
([T006](../spec.md#t006--validate-retrieval-repairs-in-real-coding-tasks)).

#### Task-context contrasts (matrix rows, lines 81–86)

Each contrast changes one element. Under S the arms form a ladder, A1 to A2 to A3 to
A4, and each adjacent pair adds one capability with the others held off. Shared arms
are reused controls (line 48); that does not make the task blocks independent.

| Matrix row | Control | Treatment | Interfaces today | Proposed status |
| --- | --- | --- | --- | --- |
| 003 T007, explicit delivery | A1: the facts as plain instruction text | A2: the same, plus the scoped `render` view (revision and per-entry SHA-256) | S exists. Host acknowledgment is not available in the capture path (fact 8); the delivered bytes are observable in the captured final request. | Descriptive: host application cannot be claimed. |
| 008 T003, durable state and views | M1; at each frozen restart point the slot carries the unfolded view, with no fold, checkpoint or restore | M1; frozen checkpoint and fold edits at the same points, a folded restart view, and the `peek` operation and the `expand` and `restore` edits of `apply` available | Needs M and an in-slot restart (fact 10). | Descriptive. |
| 003 T008, joint delivery | A2: separate `render` and `context` | A3: one revision-bound `context --task` response | Exists. | Could be confirmatory for outcome and discovery. Final-snapshot consistency is not exercised under S, because nothing changes between the two reads, so it stays descriptive. |
| 008 T005, content recovery | M1 with the `task_context` op `search` blocked; IDs come from `inspect` and `peek` | M1 | Needs M and a declared tool-call filter (fact 4). | Descriptive. |
| 008 T004, selective review | M1 with `review` and `advise` blocked, and the view in the slot | M1 plus a declared review integration | Needs M, a filter, a host call or admission seam and a frozen source of proposed assertions; none exists. | Blocked. An instruction-only variant would be descriptive. |
| 003 T009, evidence composition | A3 | A4 | Exists. | Could be confirmatory. |

Item 4's three arms map to: ordinary control A1, passive retained context A3, and
selective intervention with no binding today. Under M, a T007 variant would be M1 with
the view reached only through the model's own `render` or `context` calls, against M1
with the same view also in the slot; it is also descriptive.

#### Server and tool inventory for the startup row (line 1827)

- **A0, A1, A1-off, A2, A3 and A4** (A0 and A1-off dropped by decision 18, above).
  The host exposes exactly `read` and `edit`. No MCP server is admitted, and the
  project source stays disabled
  (`mcp.enableProjectConfig: false`, `src/pilot_capture/host.rs:557-566`). Foundry
  runs and exits before the host launches, so it adds no startup owner. The existing
  read/edit identity in `src/pilot_capture/startup.rs:252-266` applies unchanged.
- **M0 and M0-off** (M0-off dropped by decision 18). `read`, `edit` and the six
  Foundry tools, in the names and order the pinned host actually serializes. In the first-class form they are request
  `tools` entries named `mcp__context_foundry_<tool>` (fact 9). The device form would
  need `write`, which widens tool authority beyond item 5 and is not proposed.
  `references` returns no compiler references in any M arm, because no SCIP is
  imported. `index` writes only Foundry's store (`src/mcp/tools.rs:680-683`); its
  `timeout_ms` of up to 1,200,000 is bounded by the per-call timeout and the slot
  deadline. Without `--watch`, the model refreshes the index after its edits by calling
  `index`. The alternative, `--watch` in every M arm, adds a watcher owner to the
  startup inventory.
- **M1.** The M0 tools plus `task_context`, and the task fields on `context`.
- **Declared entry.** `{"mcpServers":{"context-foundry":{"type":"stdio","command":"<absolute foundry path>","args":[<server argv after the binary>],"timeout":<frozen per-call bound in ms>}}}`.
  The shape is `foundry connect --host omp` (`src/bootstrap.rs:752-767`). `connect`
  cannot print these arms: it does not forward `--no-memory`,
  `--experimental-task-context` or the behavior flag (`src/bootstrap.rs:674-721`), and
  its default timeout is 1,260,000 ms (`src/bootstrap.rs:596`; `src/mcp.rs:130`). The
  entry is written directly and hashed at use.
- **Start.** The capture launcher writes the entry into a non-project MCP configuration
  in the private host root. The location must be anchored in the pinned host's
  `mcp/config.ts::loadAllMCPConfigs` (lines 800–804); it is not verified here. At
  session startup the host's MCP manager spawns the absolute command; `PATH` is
  `/usr/bin:/bin` (`src/pilot_capture/host.rs:487`). The child runs inside the host
  sandbox. Foundry checks that its budget is delivery-scoped
  (`src/mcp/transport.rs:33`), opens the store once (`:36`) and serves stdio. With no
  semantic, policy or knowledge profile it starts no worker
  (`src/adapter_cli.rs:59-95`), and the sandbox denies it network access.
- **Stop.** At host teardown the MCP manager closes the child's stdin
  (`MCPManager.disconnectAll`; see the caveat at lines 805–807). On EOF Foundry stops
  admission, cancels active work, waits for the current transaction, stops any worker
  and exits (`src/mcp/transport.rs:49-55`, `:79-87`). A slot ends only after the
  capture parent has reaped the host's process group including this child, which must
  be verified, and the store lock is released. The store is then archived with the
  slot and the workspace restored.

#### Apparatus work an M binding needs

1. Admit the declared tool list at the request validator, bridge and argv
   (`src/pilot_capture.rs:53-54`; `src/pilot_capture/wire.rs:633-653`;
   `src/pilot_capture/bridge.ts:41`, `:498-505`, `:528`;
   `src/pilot_capture/host.rs:460-461`), and change the startup receipt that requires
   MCP disabled (`src/pilot_capture/startup.rs:252-266`). Extend the per-call tool
   check (`src/pilot_capture/host.rs:1333-1337`), which today refuses every Foundry
   tool call as `tool_not_allowed` (fact 10).
2. Qualify the host's MCP manager as a startup owner. `restrictToolNames` guards both
   MCP and memory work in the pinned SDK (lines 1881–1888), so admitting MCP must
   leave memory off and keep the off-memory check.
3. Sandbox: the policy denies reads and writes under the real home, where the
   checkout's binary lives (`src/pilot_capture/host.rs:414-448`). The pinned Foundry
   binary copy and the store go under the host root. The startup-observation profile
   appends `STARTUP_OBSERVATION_POLICY_SUFFIX` (`:637-639`): `(deny process-fork)`,
   which forbids the stdio child, so the admitting profile must permit exactly that
   one spawn and record it, then a fatal network denial with a non-fatal denial of
   the syslog socket.
4. Reaping, and integrity checks of the binary, entry and store at use and after the
   slot (line 232).
5. For T003, an in-slot restart at frozen points. For T004 and T005, declared
   per-arm rules for `task_context` operations in that existing tool check. For
   T004, also the review seam and the source of proposed assertions.

Both bindings need a multi-arm allocation and launch-manifest format in place of
v1's A/B pairs (`src/pilot_capture/apparatus.rs:842-851`).

#### Choices for the owner

The implementer proposes these options and does not decide them. Task-data rights,
the budget, the credential and the final sample stay with the owner; the choices below
change the arm count and therefore the costed schedule.

1. **Binding scope.**
   - (a) S only: exists today and keeps the qualified read/edit startup inventory.
     T007 (descriptive), T008 and T009 become measurable. T003, T004 and T005 cannot
     be accepted by v2, and Foundry is never used as a tool by the model.
   - (b) S now, M later: as (a), then a separate apparatus round for M with its own
     qualification.
   - (c) M from the start: covers model-driven discovery and makes T003 and T005
     bindable. It needs all the work above before any freeze.
2. **Anchorless and both-repairs arms.** The
   [sample design proposal](#proposed-sample-and-statistical-design--2026-11-20) below poses the same
   question with different options (its options a–c).
   - (a) Amend v2 to record them as unavailable because T005 closed without promotion.
     T006 then gets no confirmatory retrieval contrast.
   - (b) Substitute a declared semantic-serving arm (`--semantic-profile FILE --development-isolation`;
     store prepared with `semantic prepare`, `src/cli.rs:276-300`). It answers
     "semantic versus lexical", not T006's repair question, so accepting T006 would
     need an amendment. It adds Metal preparation time and cost on the Mac, worker and
     profile pins, and under M a worker process inside the host sandbox.
   - (c) Defer v2's retrieval part until a T005 candidate meets its re-entry trigger
     (`specs/009-optional-semantic-retrieval/spec.md:1382-1386`); T006 stays open with
     no date.
   - **Settled, 2026-11-20:** owner decision 18, "Drop retrieval arms", takes (a) and
     extends it to every retrieval arm, which is option b of the sample design
     proposal. T006 gets no retrieval contrast, confirmatory or descriptive, and its
     acceptance is amended to match.
3. **Confirmatory family.** Proposal: T009 (A3 versus A4), with T008 (A2 versus A3)
   only if the owner funds it; everything else descriptive. The draft's tool splits
   alpha across `2 × contrasts` (line 134). The required task count for the same bound
   width grows with `ln(2 × contrasts / alpha)`: at alpha 0.05, 1.19 times for two
   contrasts and 1.30 times for three, compared with one. The split is valid even
   though the ladder shares arms. The sample design proposal below sizes other
   confirmatory families (its Designs A–C).
4. **Optional arms.** A0 (no Foundry) and A1-off (pre-repair) are descriptive
   references. Each adds one slot per task repetition. **Settled, 2026-11-20:** both
   serve only retrieval contrasts, so decision 18 drops them.
5. **Item 4's selective-intervention arm** has no binding today. Either amend v2 to two
   task-context arms, or fund M plus a review seam; that seam's design is itself
   unselected.

### Proposed task manifest, authoring and allocation — 2026-11-20

**Proposed, 2026-11-20, not frozen.** This is a proposal for the owner. It is not
selected and not implemented. This subsection proposes the per-task record that
item 1 requires, the checks each task must pass before launch, an authoring and
sealing protocol modelled on 009 T005's holdouts, and a randomized, balanced
allocation procedure for item 6. It draws no seed. It selects no task, repository,
arm, sample size, repetition count or budget. It authorizes no provider request,
credential access, execution of task code or spend. Task-data rights, execution
authority for task repositories, the budget, the key and the final sample are the
owner's. Where options appear below, this text states their consequences and does
not choose. Line numbers cite files at commit `afa7b38`. For this draft, that is the
2,190-line text last changed in `3dcc8da` (SHA-256 `3c671466…8de6`), before the
2026-11-20 proposals were inserted.

**Contract text this fills**

| Contract text | Lines | Filled by |
| --- | --- | --- |
| Item 1: separate development/calibration from sealed evaluation tasks; exclude v1's repair and all tasks used to select repairs; per-task ID, revision, seed diff, rights record, prompt, permitted files/tools, hidden oracle and intent stratum; hash before running; repetitions are within-task | 34–39 | A, B, D |
| Item 2: four intents, six task-context cases, silent cases, correct behavior defined before model output | 40–44 | A (stratum, cases, targets); C (V11, V12) |
| Item 3: bind actual configurations; an unavailable repair is a readiness blocker | 45–48 | E (inputs); F |
| Item 4 and the capability matrix's task/control evidence | 49–54, 79–94 | A (scripts, assertions, targets, facets) |
| Item 5: equal instructions and tool authority within a task block | 55–59 | A (one prompt per task; tools set by the freeze) |
| Item 6: randomized, balanced order within strata; seed and repetitions frozen before outcomes; no replacement; every scheduled row retained | 67–69 | D (rules); E |
| Fixed, disjoint task strata in the power tool and the freeze | 139–141, 160–168, 207–211 | E (cells) |
| The launch manifest binds task and oracle manifests and the allocation; rehash at use | 230–232 | D (set digest); E (allocation digest) |
| The task manifest distinguishes blocked reads, forbidden tools and unauthorized effects; existing checker; seed fails and reference passes without exposing hidden checks; task-specific execution authority | 2147–2157 | A (scope, oracle); C (V3–V10); G.1 |
| No replacement samples; raw tasks private; rights are not training consent | 2165–2170 | A (rights); D |
| Protocol preparation (task/oracle manifest; rights are the owner's) and launch evidence (task-data rights) | 2174–2177, 2180–2181 | A–D; G.1 |

**Terms used here.**

- *Task record*: one canonical JSON object per task (section A). *Task bundle*: the
  private files its digests name (seed diff, prompt, oracle, reference repair,
  alternative repairs, scripts and targets).
- *Evaluation set*: the sealed held-out tasks. *Development set*: separately sealed
  tasks used for no-provider apparatus qualification and any calibration the owner
  separately authorizes (item 1, line 34). No task belongs to both.
- *Custodian*: the one agent context that holds the private bundles, runs the
  mechanical checks and seals. It does not author tasks or implement arms.
  *Author* and *checker*: fresh agent contexts that write and check tasks. All are
  in-session; none is an independent review ([AGENTS.md](../../../AGENTS.md), line 32).
- *Cluster*: the tasks drawn from one source repository.
- *Cell*: one allocation stratum, meaning one intent stratum together with one frozen
  arm list. *Block*: one repetition of one task, run under every arm of its cell back
  to back (item 5's "task block", line 55). *Wave*: all blocks of one repetition
  number. *Sequence*: the arm order inside a block.

#### A. Per-task record

The record is canonical JSON: UTF-8, keys sorted, `,` and `:` separators without
spaces, integers only, exactly as Python's `json.dumps(record, sort_keys=True,
separators=(",", ":"), ensure_ascii=False)` writes it. `seal.record_sha256` is the
SHA-256 of that encoding with the `seal` object removed. Every file digest is the
SHA-256 of its exact bytes. A *tree digest* is the SHA-256 of the canonical JSON list
of `[path, mode, size, sha256]` rows sorted by path, with relative `/`-separated paths.

"Who" names the supplier: A author, K checker, C custodian (mechanical), F the frozen
launch policy (the same value for every task), O owner. "Seal receipt" says what may
appear in the public metadata-only receipt: the value, its digest, a count, or
nothing. Anything naming a repository, file, symbol or task text stays private unless
the owner authorizes more (lines 2167–2168).

| Group | Field | Content | Who | Seal receipt |
| --- | --- | --- | --- | --- |
| Identity | `schema` | `"pilot-v2-task-1"` | C | value |
| Identity | `task_id` | `v2e-NNN` (evaluation) or `v2d-NNN` (development), assigned in sealed order; never reused or renumbered | C | value |
| Identity | `split` | `evaluation` or `development` | C | value |
| Identity | `cluster_id` | pseudonymous repository label `rNN` | C | value |
| Identity | `author`, `checker` | pseudonymous context labels; private ledger references | C | labels |
| Source | `source.repository` | canonical clone URL, or the owner's label for private source | A | nothing |
| Source | `source.commit` | full commit ID of the pinned revision | A | nothing |
| Source | `source.tree_sha256` | tree digest of the checkout at the pin, without `.git` | C | value |
| Source | `source.acquired_as` | `clone` or `archive`, with the archive's SHA-256 | C | nothing |
| Source | `source.dependencies` | lockfile SHA-256 and tree digest of the offline dependency set the checker uses | C | value |
| Source | `source.toolchain` | compiler or runtime name, version and executable SHA-256 | C | value |
| Seed | `seed.kind` | `upstream_parent` (pin is the parent of a real fix; empty seed diff), `reverted_fix` (seed reverts a real fix; the fix's tests move to the oracle) or `synthetic` (author-written defect) | A | value |
| Seed | `seed.diff_sha256`, `seed.workspace_tree_sha256` | the seed diff, and the workspace every slot of this task starts from | C | value |
| Seed | `seed.upstream_fix` | fix commit and the date it first became public; `null` for `synthetic` | A | nothing |
| Text | `prompt.sha256`, `prompt.bytes` | the per-task text placed into the common ordinary instructions that every arm receives (item 5); the common template's digest is a freeze value | A, C | digest |
| Behavior | `intent_stratum` | exactly one of `anchorless`, `mixed_behavior`, `ambiguous_name`, `exact_locator` (item 2) | A; verified K, C | value |
| Behavior | `context_cases` | subset of `restart_recovery`, `retained_constraint`, `rejected_action`, `conflicting_observation`, `open_obligation`, `folded_evidence_recovery` (lines 41–43) | A | value |
| Behavior | `context_script.sha256` | ordered scripted events with arm-neutral triggers, for example "restart after generation 3", never "after the model finds X", each with the externally checked observation or constraint expected after it (line 82) | A | digest |
| Behavior | `assertions.sha256` | for 008 T004 cases: scoped single-valued keys, the assertion source, relations, and the expected intervention or silence per case, covering fresh, conflicting, satisfying, missing-key and stale-source cases (line 85) | A | digest |
| Behavior | `silence_points` | scripted points at which any intervention is unnecessary (lines 43–44) | A | count |
| Behavior | `evidence_targets.sha256` | spans `[path, start_byte, end_byte, sha256]` in the seed workspace: `first_useful` (any one suffices) and `complete_support` (all required); 003 T009 facets with their citations (line 86); 008 T005 folded originals whose IDs neither arm receives, with expected freshness (line 84) | A | digest |
| Difficulty | `expected_difficulty` | recorded before any model sees the task: files and lines changed by the reference repair; evidence-target count and distinct files; repository file count; whether any edit-site identifier occurs in the prompt; the author's and checker's estimate on a 1–3 scale with a one-line reason | A, K, C | distribution only |
| Scope | `scope.read` | read-admitted paths: files of the seed workspace minus declared exclusions | A | count |
| Scope | `scope.edit` | edit-admitted paths | A | count |
| Scope | `scope.protected` | paths that must be byte-identical after the slot | A | count |
| Scope | `scope.tools` | exact admitted tool names, identical for every arm; host memory and auto-learn are never admitted (lines 60–66) | F | value |
| Scope | `scope.violation_classes` | `blocked_out_of_scope_read` and `forbidden_tool_attempt` are retained results; `unauthorized_effect` halts globally (lines 2147–2151) | F | value |
| Oracle | `oracle.tree_sha256` | the hidden oracle files, never inside the workspace | A, C | digest |
| Oracle | `oracle.recipe` | ordered steps; each an argv whose only substitution is `<ROOT>`, an allowlisted environment, a time and memory limit, network denied | A, C | digest |
| Oracle | `oracle.pass_rule` | every step exits 0 and every `scope.protected` file is unchanged | F | value |
| Oracle | `reference.diff_sha256` | the author's known-good repair | A | digest |
| Oracle | `alternatives` | digests of at least one differently written correct repair and at least one plausible wrong repair | A, K | counts |
| Oracle | `oracle.observed_ms` | seed and reference check durations from V6 | C | value |
| Checks | `exclusions` | each section B check with its verdict and receipt digest | C | value |
| Checks | `validation` | each check V1–V16 with its verdict and receipt digest | C | value |
| Rights | `rights.facts` | license (SPDX expression and license-file SHA-256), whether the source is public, who owns private source | A, C | counts by license |
| Rights | `rights.decision` | `pending_owner` until the owner records it: private storage; sending this task's source and prompt to the named provider under its retention terms; local execution of this repository's build and test code; publication limited to aggregates. `training` is always `not_granted` (line 2169) | O | status only |
| Rights | `rights.decision_ref` | the dated owner decision the record cites | O | value |
| Seal | `seal.record_sha256` | defined above | C | value |
| Seal | `seal.bundle_tree_sha256` | tree digest of this task's bundle | C | value |

Request, time and output limits are launch-manifest values (lines 277–282), not task
fields, so every arm and task in a block shares them (item 5).

#### B. Exclusion checks

Item 1 excludes v1's repair and every task used to select repairs (line 35). Each
check below is mechanical, run by the custodian, and recorded in `exclusions`.

1. **Pilot v1 and earlier host calibration.** The `percent-encoding` crate in any
   version and its upstream repository; v1's task bytes and seed
   ([v1 contract](bounded-pilot-v1.md), lines 36–37); the parser and
   arithmetic-overflow fixture repairs ([spec](../spec.md), lines 1209–1213;
   [record](../../../docs/validation.md), lines 12877–12884); and the managed-host
   repair of 2026-10-16 ([spec](../spec.md), lines 788–794).
2. **Foundry's retrieval-quality corpora.** rust-lang/rust `1.99.0` (`b940084d`) and
   oh-my-pi `5b8d5b8a15`
   ([handoff](../../../docs/measurement-handoff.md#inputs), lines 570–571). They
   carry G1, the 388 G2 questions, F4's 300, the 2026-11-11 T005 holdout consumed
   on 2026-11-13 ([record](../../../docs/validation.md#t005-sealed-final-pair-no-promotion--2026-11-13))
   and the 2026-11-18 holdout that the owner kept sealed for future work
   ([decision 17](../../../docs/validation.md#owner-decisions--2026-11-19)). Proposed
   rule: exclude both repositories entirely, at any revision. No sealed set then
   needs opening for an overlap check, and v2 outcomes reveal nothing about the
   sealed holdout.
3. **Foundry itself and its fixtures.** This repository, including the eight Foundry
   questions in `tests/retrieval_attribution.rs` ([AGENTS.md](../../../AGENTS.md),
   lines 37–39); 001 T010's development and held-out mixed-intent cases
   ([001 spec](../../001-source-state-recovery/spec.md), lines 657–661); the 008 T004
   trigger fixtures; and every other fixture under `tests/`.
4. **Foundry's scale-only corpora.** The checksum-pinned Rust 1.90.0 and Go 1.25.0
   source archives and the representative-workload corpus
   ([handoff](../../../docs/measurement-handoff.md), lines 101–102 and 147–158).
   They selected no repair; excluding them is proposed only so that no Foundry
   corpus remains, at little cost.
5. **Predecessor and Laya repositories**, which Foundry does not touch without
   task-specific authority ([AGENTS.md](../../../AGENTS.md), lines 28–29).

The checks take these forms:

- **Repository identity.** The canonical URL and the root commit(s) of the
  repository's history differ from every excluded repository's, so forks are caught.
- **Content.** No file in the seed workspace's read scope has a SHA-256 in the set of
  file digests of the excluded public corpora at their pins, which catches vendored
  copies. Trivially common files (empty files, standard license texts) are allowed
  only through a list frozen in the authoring selection.
- **Within v2.** No two evaluation tasks share a reference fix commit or an edited
  function. No evaluation task shares a repository with a development task.
- **Cluster caps.** At most `c` evaluation tasks per repository and `m` per module.
  T005 used three per file and six per module
  ([record](../../../docs/validation.md#t005-fresh-holdout-authoring-selection--2026-11-11)).
  `c` and `m` belong with the sample choice (G.5), because tasks from one
  repository are not independent task blocks (lines 197–200).

Not covered: whether the coding model saw an upstream fix during training.
`seed.upstream_fix` makes that reportable; G.2 gives the owner's options.

#### C. Validation before launch

Each check writes a receipt (input digests, argv, exit status, durations, output
digests) and fails closed. A failed check keeps its receipt, and its task does not
enter the sealed set. Checks V4–V7 and V10 run the task repository's build and test
code, so they need the owner's execution authority for that repository (lines
2156–2157; G.1). Every check runs offline, with network denied, one heavy job at a
time and no provider call. None uses Foundry retrieval.

- **V1 Schema.** Every field present, digests well formed, enumerations valid.
- **V2 Source readback.** Materialize the pin again from its source and recompute
  `source.tree_sha256` and the dependency digest.
- **V3 Seed workspace.** The seed applies cleanly and `seed.workspace_tree_sha256`
  matches. The workspace holds no `.git` directory or other history that contains
  the fix, no oracle file, no file containing a hunk of the reference repair, and no
  changelog or test text that the upstream fix added.
- **V4 Seed fails as intended** (line 2155). The oracle fails on the seed workspace,
  at the declared step and the declared failing cases.
- **V5 Reference passes** (line 2155). With the reference repair applied, the oracle
  passes and every protected file is unchanged.
- **V6 Determinism and headroom.** Three runs each of seed and reference give
  identical verdicts and identical step-output digests, excluding declared timing
  lines. The slowest run uses at most half of its step's time limit.
- **V7 Behavior, not text.** Every correct alternative passes, every wrong
  alternative fails, and the empty patch fails.
- **V8 Hidden oracle.** The oracle tree lies outside the workspace and the read
  scope. The checker confirms that the prompt states the required behavior without
  quoting oracle test names or expected values. As an aid, the custodian lists every
  literal of 12 or more characters that occurs in the oracle, not in the seed
  workspace, and in the prompt; each must be removed or justified in the checker
  record.
- **V9 Scope consistency.** The reference repair edits only `scope.edit`. Every
  evidence target and every file the reference repair touches lies in `scope.read`.
  `scope.protected` and `scope.edit` are disjoint.
- **V10 Checker fit.** The recipe runs under the existing bounded checker's rules:
  fresh per-slot workspace (lines 2153–2154), `<ROOT>` as the only substitution
  (`src/pilot_capture.rs:136-138`),
  private 0700/0600 outputs (lines 1990 and 2052–2053), within the per-check bound
  the freeze selects.
- **V11 Intent stratum.** Let the edit-site identifiers be the names, of three or
  more characters, of the functions, methods, types and constants that the reference
  repair modifies or that enclose its edits.
  - `anchorless`: no edit-site identifier, edited file path or file stem occurs in
    the prompt;
  - `exact_locator`: the prompt names the edit-site symbol or path, and that name
    has exactly one definition at the pin;
  - `ambiguous_name`: the prompt names a symbol with two or more definitions at the
    pin, exactly one of which is the edit site;
  - `mixed_behavior`: the prompt names a symbol whose definition is not edited, and
    the edit is in its implementation, a method of it or a function it calls.

  The token tests are mechanical. The checker confirms definition counts by
  inspecting the source and records each definition's path and line; authors and
  checkers do not use Foundry outputs for this (section D).
- **V12 Declared behavior complete.** Every `context_cases` entry has a scripted event
  with an arm-neutral trigger and an externally checkable expectation. Tasks in 008
  T004 contrasts cover the five assertion case kinds of line 85. Tasks in
  intervention contrasts have at least one silence point (lines 43–44). Every
  evidence target resolves byte for byte in the seed workspace.
- **V13 Exclusions.** Every section B check passes.
- **V14 Difficulty floor.** `expected_difficulty` meets the floor that the authoring
  selection declared before drafting (G.3). A task below it is recorded as a skip,
  never edited until it passes.
- **V15 Rights.** `rights.decision` is the owner's approval for this repository before
  the task enters a launch manifest (lines 2180–2181). A record is sealed with
  `pending_owner` only if the owner selects that timing in G.1.
- **V16 Seal readback.** The sealed stage re-hashes to every record and bundle digest.

Set-level checks before sealing: the exact count in each cell matches the authoring
selection; cluster caps hold; no two prompts are identical after lowercasing and
collapsing whitespace; evaluation and development sets share no repository; every
`task_id` is unique.

#### D. Authoring and sealing protocol

The 009 T005 holdouts are the precedent. Their authoring selection is
[recorded](../../../docs/validation.md#t005-fresh-holdout-authoring-selection--2026-11-11)
(validation.md lines 6911–6945), with seals of
[2026-11-11](../../../docs/validation.md#t005-fresh-holdout-seal-and-evaluation-guards--2026-11-11)
(lines 6827–6909) and
[2026-11-18](../../../docs/validation.md#t005-new-blind-holdout-sealed--2026-11-18)
(lines 1009–1045), and encrypted retention of
[2026-11-11](../../../docs/validation.md#t005-sealed-holdout-encrypted-retention--2026-11-11)
(lines 6776–6791) and
[2026-11-19](../../../docs/validation.md#t005-second-holdout-encrypted-retention--2026-11-19)
(lines 177–206).

| T005 precedent (validation.md lines) | Pilot v2 analogue |
| --- | --- |
| Authoring selection frozen before any drafting (6920–6926) | Step 2 |
| Custodian freezes a sampling seed, an eligible catalog and disjoint partitions before authors start (1011–1018) | Step 2 |
| Fresh-context authors without retrieval outputs, traces or candidate design (6928–6931) | Step 3 |
| Every visited candidate and skip reason kept; caps per file and module (6930–6932, 6835–6837) | Step 3; section B caps |
| Exclusion keys from historical sets (1014–1016) | Section B |
| Separate label-side checker inspects every answer and plausible alternatives; identity-preserving wording repairs; append-only amendment (6932–6933, 1028–1031) | Step 4; V7 |
| Unchanged builder and full verifier, no partial mode (6933–6934, 1031–1033) | Step 5 |
| Bound of three hours, then ten minutes mechanical, two minutes cleanup (6938–6940) | Step 1, G.8 |
| Missed counts keep partial evidence and stop; no floor silently lowered (6940–6943) | Step 6 |
| Artifact SHA-256, metadata-only receipt, private stage with readback (1035–1039, 6856–6866) | Step 7 |
| Implementation receives metadata only (6864–6866, 6935–6936) | Step 9 |
| Encrypted to the owner's key, handled as opaque bytes (6778–6791, 182–202) | Step 8 |
| Failed receipt wrapper kept beside the corrected result (6844–6854) | Every step: failures retained |

The main difference is that T005 sealed retrieval questions whose answers are
declarations. V2 seals executable oracles, so each task takes far more authoring
work, its checks run the repository's code, and the checker must run that code as
well as read the source.

1. **Owner inputs before any drafting** (section G): the approved source pool and
   rights, execution authority, contamination stance, per-cell task counts and
   repetitions from the frozen power design (lines 207–211), cluster caps,
   development-set size, difficulty floor and authoring time bound.
2. **Authoring selection, frozen before drafting.** It fixes counts per cell, the
   eligibility rule for candidates (for example, bug-fix commits in approved
   repositories that add a test, within a declared date window, or author-written
   defects for `synthetic`), disjoint repository partitions per author, caps,
   the difficulty floor, the time bound and this record schema. The custodian
   freezes a *sampling seed* that only orders each partition's candidate catalog;
   it is not the allocation seed of section E. The selection's SHA-256 is recorded in
   validation.md and a pushed commit before authors start, as F4's pre-registration
   was committed before any question existed
   ([FOLLOWUP F4](../../../FOLLOWUP.md#f4-sealed-independent-evaluation--passed-2026-10-22),
   line 128).
3. **Authors**, one fresh context per partition, receive the partition's pinned
   source, the authoring rules and their candidate order. They never receive
   Foundry retrieval outputs or traces, arm designs, other partitions or other
   authors' tasks. They walk the catalog in order and record every visited
   candidate as accepted or skipped with a reason. For each accepted candidate they
   draft the prompt, oracle, reference and alternative repairs, scripts, targets and
   difficulty fields.
4. **Checkers**, one fresh context per partition, check every task against source:
   the oracle against the reference and alternatives, the stratum, prompt clarity
   and leakage, scripts and targets. A wording repair must leave the reference,
   oracle and targets unchanged. Amendments are append-only and keep the earlier
   verdict.
5. **Mechanical validation.** The custodian runs V1–V16 and the set-level checks,
   unchanged and complete.
6. **Failure rule.** If any cell cannot reach its exact count with every check
   passing inside the bound, the custodian keeps all evidence, seals nothing and
   returns to the owner. No count, floor or check is relaxed.
7. **Seal.** Write `tasks.jsonl`, one canonical record per line sorted by `task_id`,
   and the bundles. The set digest is the SHA-256 of `tasks.jsonl`. Stage it in a new
   private directory under `/Users/satishlomte/VSC_DEV`, never under `/private/tmp`
   (CLAUDE.md), with 0700 directories and 0600 files, and read back every file.
   Record a metadata-only receipt in validation.md: set digest, counts per cell and
   cluster, validation pass counts, author and checker counts, skip and wording
   repair counts, and every retained failure. Seal the development and evaluation
   sets separately.
8. **Encrypted retention** to the owner's selected key, handled as opaque bytes, as
   in both T005 precedents. Decryption, restoration and any copy off the Mac remain
   owner tasks.
9. **Access after seal.** Whoever implements or qualifies arms receives metadata
   only. Arm configurations are frozen by digest before the evaluation set is opened
   for materialization. The coding host never receives oracles, reference or
   alternative repairs, or targets (line 2156). Development tasks may be opened for
   no-provider apparatus qualification (lines 2178–2179).
10. **After seal.** No sealed record is edited. A defect found before any outcome,
    such as an oracle that flakes when rechecked at launch, is recorded, halts
    launch preparation and returns to the owner (G.6). After any outcome a task is
    never removed or replaced; it is scored with its retained disposition (lines 69
    and 2165).

#### E. Randomized, balanced allocation

**Inputs.** All are written as canonical JSON and hashed into
`allocation_inputs_sha256`:

- `task_set_sha256` of the sealed evaluation set;
- the cells: each has a name, the sorted IDs of its tasks, and its ordered arm list
  of `[arm_id, configuration_sha256]` pairs, each bound to an actual implemented
  configuration (item 3, lines 45–48);
- the repetition count R, one for all tasks unless the freeze selects one per cell,
  as the power tool's stratified form allows (lines 139–141);
- the algorithm version and the SHA-256 of the script that implements it.

Every task in a cell has the same arm list. A task measured in two contrast families
appears in two cells with separate blocks, unless the freeze binds a shared baseline
by an identical configuration digest; then the arms join one cell (line 48; lines
88–91).

**Seed commitment.**

1. After the evaluation set is sealed and the arm configurations and other inputs
   are frozen, one 32-byte seed is drawn (G.7 gives the options for who draws it and
   how).
2. The commitment is the SHA-256 of the ASCII bytes
   `context-foundry/pilot-v2/allocation-seed/v1`, one zero byte, the 32 seed bytes,
   and the raw 32-byte `task_set_sha256` and `allocation_inputs_sha256`. The
   commitment, both input digests and the draw time are recorded in validation.md and
   in a pushed commit before the seed is revealed or any allocation is computed.
3. On reveal, the seed goes into the private launch manifest. The custodian computes
   the schedule, and a second in-session context recomputes it from the seed and
   inputs; the two must match byte for byte. The seed contains no task data and may
   be published with the frozen launch receipt.
4. All of this precedes the first coding slot, and the launch manifest binds the
   allocation's SHA-256 (line 231). There is no redraw. If any input changes after
   commitment, the result is a new allocation version with a new commitment, and the
   old commitment stays on record as void, with its reason.

**Algorithm.** Deterministic and implementable with the Python standard library.

- *Random stream.* For a label `L`, block `i` (`i` = 0, 1, …) is
  HMAC-SHA256 with the seed as key over the ASCII bytes
  `context-foundry/pilot-v2/allocation/v1`, a zero byte, `L`, a zero byte and `i` as
  an 8-byte big-endian integer. Consecutive 8-byte big-endian words are read from
  the concatenated blocks. An integer below `n` is a word below
  `2^64 − (2^64 mod n)`, taken modulo `n`; a larger word is discarded and the next
  word read. A shuffle is Fisher–Yates from the last index down, drawing `j` below
  `i + 1`.
- *Sequence family.* For a cell with `k` arms, use the Williams design: first row
  `[0, 1, k−1, 2, k−2, …]`; row `i` adds `i` modulo `k` to each entry; for odd `k`,
  append each row reversed. That gives `k` rows for even `k` and `2k` for odd `k`.
  Across the family, each arm occupies each position equally often and each ordered
  pair of arms is adjacent equally often, so first-order carryover, such as a cache
  warmed by the preceding arm, is balanced, though not removed (lines 297–299).
  For `k` = 2 the family is AB and BA, as in v1.
- *Assignment.* For each cell, in name order, shuffle the arm-to-symbol map (label
  `arm-map/<cell>`), the family's rows (`rows/<cell>`) and the cell's task IDs
  (`tasks/<cell>`). The task at shuffled position `i`, repetition `r` (both counted
  from 0), gets row `(i·R + r) mod |W|`, where `|W|` is the number of rows.
- *Balance.* When a cell's task count times R is a multiple of `|W|`, every row is
  used equally often, so the cell is exactly balanced. When R ≤ `|W|`, one task's
  repetitions get different rows. A cell whose count is not such a multiple is not
  exactly balanced. The freeze either chooses counts that are (with `k` = 2, an even
  number of blocks; `k` = 3, a multiple of 6; `k` = 4, a multiple of 4) or accepts and
  records the residual imbalance (G.5).
- *Run order.* Wave `r` holds every block with repetition `r`, shuffled with label
  `wave/<r>`; waves run in order. A halt therefore leaves every earlier wave
  complete, every task's first repetition precedes any task's second, and one task's
  repetitions fall in different waves.
- *Within a block.* Arms run back to back in row order. Each starts from a freshly
  restored seed workspace and fresh host state, and no outcome, repair or diagnostic
  of an earlier slot becomes a later input
  ([v1 contract](bounded-pilot-v1.md), lines 66 and 73–74).
- *Output.* One row per slot: global order, `slot_id` (`w<r>-b<block>-p<position>`),
  task, repetition, cell, row index, position, arm ID and configuration digest.
  `allocation_sha256` covers the canonical rows, the inputs and the commitment. Every
  scheduled row is retained, including rows never started after a halt (line 69).

A scratch self-check of this algorithm, using a fixed test key rather than a drawn
seed and two synthetic cells (4 arms with 8 tasks, 3 arms with 6 tasks, R = 2),
confirmed the Williams properties for `k` = 2 to 6, per-position balance, distinct
rows per task, wave order and determinism. It is not a qualified script. A future
`scripts/pilot-v2-allocate.py` (proposed path) would need its own tests, like
`scripts/test_pilot_v2_power.py`.

**Repetitions.** R is part of the owner's final sample. The power tool counts each
task once and treats its repetitions as perfectly correlated (lines 127–128 and
167–168), so repetitions add no independent tasks. The schedule has, summed over
cells, tasks × R × `k` coding slots, each costed at the frozen ratecard with failed
and invalid attempts included (lines 280–282). R = 1 gives the most tasks for a given
spend. R ≥ 2 observes variation within a task and costs proportionally more.

#### F. Consequences outside the record (not decided here)

- **The anchorless arm.** 009 T005 closed without promotion
  ([decision 17](../../../docs/validation.md#owner-decisions--2026-11-19)), so no
  promoted anchorless repair exists to bind. Item 3 (lines 46–48) makes an
  unavailable repair a readiness blocker, not an arm that is silently dropped. No
  allocation can be computed until the owner settles T006's arm list, for example
  by amending item 3 to a comparison without that arm or by binding a different
  configuration under a new decision. The `anchorless` task stratum is unaffected:
  item 2 still requires anchorless tasks, and every arm is measured on them. The
  [arm-binding proposal](#proposed-arm-and-interface-bindings--2026-11-20) (its choice 2) and the
  [sample design proposal](#proposed-sample-and-statistical-design--2026-11-20) (its options a–c) list
  options. **Settled, 2026-11-20:** owner decision 18, "Drop retrieval arms", amends
  item 3 to a comparison without any retrieval arm, so T006's arm list no longer
  blocks allocation. The cells' arm lists are the task-context arms alone.
- **The mixed-intent arm.** 001 T010's lexical behavioral selection is on by default
  for lexical stores and changes nothing on stores that serve semantic results
  ([001 spec](../../001-source-state-recovery/spec.md), lines 571–581 and 671–674).
  Whether "current Foundry" and "mixed-intent repair" are distinct configurations is
  for the freeze to bind by digest. After decision 18 there is no mixed-intent arm:
  current Foundry with the selection on is the one retrieval configuration, bound by
  one digest in every arm.
- **The apparatus.** The existing capture owner holds v1's per-task inputs as
  constants: `SLOT_ORDER`, `READ_SET`, `EDIT_SET`, `ORACLE_SHA256` and the
  `rustc`-only `CHECK_RECIPE` ([`pilot_capture.rs`](../../../src/pilot_capture.rs),
  lines 112–139), used by `run_independent_check` and `run_coding`
  ([`apparatus.rs`](../../../src/pilot_capture/apparatus.rs), lines 1327, 1396 and
  1771–1779). V2 needs them read from the sealed records and the schedule, bound in
  the launch manifest. That is an implementation change needing fresh qualification
  at its boundary (lines 2188–2189). A task whose check needs Cargo with offline
  dependencies, or another language, also needs a recipe and toolchain pin that the
  current checker lacks.
- **Owner decisions of 2026-11-19.** Host memory and auto-learn off in every arm,
  `PI_PROXY_ANTHROPIC` unused, and host identity by digest plus pinned inputs
  ([decisions 9–12](../../../docs/validation.md#owner-decisions--2026-11-19)) are
  launch-manifest values, not task fields. So is decision 19 of 2026-11-20, the
  implicit `apple` provider disabled in every launch (item 5). The record's only
  point of contact is that `scope.tools` never admits host memory.

#### G. Owner choices

Each item is the owner's. Options and consequences are listed; none is selected.

**G.1 Task-data rights and execution authority** (lines 2169, 2174–2177, 2180–2181).

- (a) Public repositories under permissive licenses only (MIT, Apache-2.0, BSD),
  approved repository by repository. This gives the simplest record. A code license
  does not by itself establish data or model rights
  ([AGENTS.md](../../../AGENTS.md), line 27), so the owner's approval still covers
  sending source to the provider under its retention terms. The contamination risk
  is highest (G.2).
- (b) As (a), plus other OSI licenses, including copyleft. The pool is larger; the
  owner judges whether private evaluation and transmission to the provider are
  acceptable under those licenses.
- (c) The owner's own private repositories. The owner holds the rights and the
  contamination risk is lower, but private code goes to the provider under its
  retention terms, and only aggregates may be published.
- (d) Synthetic tasks written from scratch in a new repository. Rights are clearest
  and contamination lowest; realism and difficulty are weakest.
- Timing: approving rights per repository before authoring means no sealed task can
  later be lost to a rights refusal. Sealing with `pending_owner` and deciding
  before launch allows earlier authoring, but a refusal after the seal falls under
  G.6.
- Execution authority: V4–V7 and V10 run each repository's build and tests on the
  owner's Mac. The owner names which repositories may run and under what bounds.

**G.2 Contamination.** (a) Accept public fixes from before the model's training
cutoff and report `seed.upstream_fix` dates. (b) Require fixes first made public
after the pinned model's published cutoff; the pool is smaller, and the cutoff must
be pinned at the freeze. (c) Prefer `synthetic` or private seeds. A memorized fix
raises success in every arm alike, reducing discordant pairs and therefore power.

**G.3 Difficulty floor.** (a) A mechanical floor only, for example: the reference
repair touches two or more files, or its evidence spans two or more files, and
anchorless prompts carry no edit-site identifier. No spend. (b) As (a), plus a paid
calibration on development tasks only, never on evaluation tasks, under its own
authorization. It informs the floor but costs money. V1's 8 of 8 passes show that an
easy task produces no discordance.

**G.4 Development set.** Proposed: repositories separate from the evaluation set.
Its size trades qualification coverage against authoring time.

**G.5 Final sample.** Per-cell task counts, R, the cluster caps `c` and `m`, and
whether exact Williams balance is required. These come from the frozen power design
(lines 207–211); this subsection proposes none of them.

**G.6 A defect after seal, before any outcome.** (a) Record it, drop the task, seal a
new set version and commit a new allocation seed, keeping the old set and commitment
on record. (b) Keep the task and score it invalid. After any outcome only (b)
applies.

**G.7 Who draws the seed.**

- (a) The owner draws 32 bytes locally, for example with
  `python3 -c "import secrets; print(secrets.token_hex(32))"`, and gives the custodian
  only the commitment until reveal. This costs one owner action, and the implementer
  can neither preview nor choose the allocation.
- (b) The custodian draws, as in v1 ([v1 contract](bounded-pilot-v1.md), line 64).
  This is simplest but is operator provenance only: the commitment shows that the
  seed preceded outcomes, not that it was not chosen among several private draws.
- (c) A public randomness beacon round, named in the commitment record before its
  value exists, from which the seed is derived. Nobody can choose it. It needs one
  read-only network fetch at the draw and fails if the beacon is unavailable.

**G.8 Authoring time bound.** T005 used three hours for 400 questions, plus ten
minutes of mechanical validation and two of cleanup (validation.md lines
6938–6940). Tasks with executable oracles need much more time per task. The owner
sets the bound; missing it triggers step 6 of section D.

#### What this subsection does not do

It draws no seed, authors or selects no task or repository, decides no rights, runs
no task code and touches no provider or credential. It does not freeze this draft or
change items 1–6, and it selects no arm, sample size, margin, budget or schedule. It
accepts neither T006 nor any 003 or 008 task. It is in-session self-review only, not
an independent review.

## Sample size and decision rule

### Optional paired-task sensitivity tool

[`scripts/pilot-v2-power.py`](../../../scripts/pilot-v2-power.py) is an unfrozen,
no-provider planning aid for **binary paired task-level** success and discovery
outcomes. It reads a JSON object on stdin; every margin, contrast count, task
count, repetition count and hypothetical scenario must be explicit. For example,
the following is synthetic input only, not a selected design or rate estimate:

```json
{
  "overall_alpha": 0.05,
  "confirmatory_contrasts": 3,
  "noninferiority_margin": 0.1,
  "required_discovery_gain": 0.0,
  "task_counts": [100, 300],
  "repetitions_per_task": 2,
  "scenarios": [{
    "name": "synthetic only",
    "success": {"baseline": 0.7, "gain": 0.2, "loss": 0.1},
    "discovery": {"baseline": 0.4, "gain": 0.3, "loss": 0.1}
  }]
}
```

For each endpoint, `gain` is the chance that only treatment passes and `loss`
the chance that only control passes; both must be compatible with the control's
`baseline` pass probability. To floating-point precision, the tool sums the exact
finite distribution of independent, identically distributed task-block
differences in `{-1, 0, 1}`.
It models repetitions of a task as perfectly correlated and therefore counts each
task once. This is one explicit sensitivity assumption, not a general model of
within-task correlation. For each endpoint it tests whether a conservative
one-sided lower bound exceeds the relevant null boundary: negative success margin
or required discovery gain. The half-width is
`sqrt(2 * ln(1 / tail_alpha) / independent_tasks)`, from
[Hoeffding's bounded-difference inequality](https://doi.org/10.1080/01621459.1963.10500830).
`tail_alpha` is the overall alpha divided by twice the declared contrast count.
The joint-power lower bound is `max(0, success_power + discovery_power - 1)`,
without an assumption about dependence between endpoints. The conservative bound
can be substantially less powerful than a task-specific paired method.

A second input form replaces `task_counts`, `repetitions_per_task` and
`scenarios` with explicit `designs`. Each design names fixed, disjoint task
strata; each stratum supplies its own task count, repetition count and endpoint
assumptions. This example is also synthetic and does not select a design:

```json
{
  "overall_alpha": 0.05,
  "confirmatory_contrasts": 3,
  "noninferiority_margin": 0.1,
  "required_discovery_gain": 0.0,
  "designs": [{"name": "synthetic split", "strata": [
    {"name": "first", "tasks": 40, "repetitions_per_task": 2,
     "success": {"baseline": 0.7, "gain": 0.2, "loss": 0.1},
     "discovery": {"baseline": 0.4, "gain": 0.3, "loss": 0.1}},
    {"name": "second", "tasks": 60, "repetitions_per_task": 3,
     "success": {"baseline": 0.8, "gain": 0.1, "loss": 0.1},
     "discovery": {"baseline": 0.5, "gain": 0.2, "loss": 0.1}}
  ]}]
}
```

This form sums independent but not identically distributed task blocks across
strata, with identical distributions assumed within each stratum. The pooled
result weights strata by their fixed task counts; individual stratum sensitivity
is reported separately. A pooled result does not establish a per-stratum guard.
If any per-stratum or other additional comparison will support a claim, include
it in `confirmatory_contrasts` before examining outcomes; the tool cannot infer
which claims a future study will make. With the `baseline`/`gain`/`loss` form,
repetitions within each task are modeled as perfectly correlated.

To explore a different explicit within-task dependence assumption, either
endpoint in a stratum may instead supply a probability distribution over the sum
of its paired run differences. For example, with `repetitions_per_task: 2`, an
endpoint can be written as:

```json
{"outcomes": [
  {"difference_sum": -2, "probability": 0.1},
  {"difference_sum": -1, "probability": 0.2},
  {"difference_sum": 0, "probability": 0.4},
  {"difference_sum": 1, "probability": 0.2},
  {"difference_sum": 2, "probability": 0.1}
]}
```

Each run difference is treatment minus control in `{-1, 0, 1}`; the task-block
mean is the supplied sum divided by that stratum's repetition count. The tool
uses the supplied block distribution directly, with independent task blocks and
fixed task-count weights. It does not infer within-task correlation or the joint
dependence between success and discovery from these marginal distributions. The
confidence and joint-power bounds remain conservative under those assumptions.
Explicit block distributions permit at most 12 repetitions per task; their common
denominator across a design is capped at 60 and a separate operation bound applies.
Outcome sums must be distinct and in range, and their decimal probabilities must
sum exactly to one. This distribution is a hypothetical sensitivity input, not
observed pilot evidence or an authorization to run it.

The computation rejects malformed, infeasible or oversized inputs. Both forms
are binary per run and assume independent task blocks; neither estimates unknown
rates or block distributions nor establishes the selected tasks' independence or
within-stratum distribution.
They do not address the full six-contrast matrix, other endpoint types, cost,
latency, rights, host readiness or provider use. A future freeze must select and
justify its actual analysis, task/stratum model, margins and schedule; this tool
makes no freeze or execution decision. Tests run with
`python3 scripts/test_pilot_v2_power.py`.

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

### Proposed sample and statistical design — 2026-11-20

**Proposed, 2026-11-20, not frozen.** This is a proposal for the owner and selects
nothing. It was written in-session as input to the freeze blocker in lines 207–211
of this draft. Line numbers cite files at commit `afa7b38`; for this draft, they
cite its text before the 2026-11-20 proposals were inserted. The proposal selects
no sample, margin, contrast, budget, key, task source or task-data rights. It
authorizes no provider call, credential access, spend or execution;
`execution_authorized=false` and `execution_ready=false` are unchanged. Task-data
rights, the budget, the key and the final sample are the owner's choices. Every
number below follows from a stated hypothetical; none is a rate estimate.

It proposes values for these lines:

- lines 207–211: independent task count, repetitions, stratum counts, every
  confirmatory contrast, and a deterministic power calculation with its assumptions
  and sensitivity table;
- lines 213–219: success margin, required improvement, confidence level,
  multiplicity, power for both endpoints, and sensitivity to discordance and baseline
  that is not derived from v1;
- lines 221–224: a written decision rule for each contrast;
- lines 67–69: order balance within strata and frozen repetitions;
- lines 75–78 and 88–94: which contrasts are confirmatory and which stay descriptive;
- lines 277–282: the provider-call count of each schedule (calls only; the ratecard
  and any dollar figure are the owner's).

It does not fill the cost and latency bounds (lines 213–214), the per-run generation
cap (lines 277–279), task selection, oracles, rights records, launch pins or host
qualification.

**Owner decisions, 2026-11-20**
([record](../../../docs/validation.md#owner-decisions--2026-11-19)). Decision 18,
"Drop retrieval arms", selects option b of the retrieval question below: no
retrieval arm. Designs B and C7 assume option a, so they are no longer candidates;
they stay below for comparison. Decision 20, "Implement and test it": the calibrated
variance-based paired method will be implemented in
[`scripts/pilot-v2-power.py`](../../../scripts/pilot-v2-power.py) with tests and
proposed for the freeze. It is now in the tool, behind `"method":
"calibrated_paired"`, with exact enumeration and tests
([record](../../../docs/validation.md#calibrated-paired-sample-size-method--2026-11-20)).
The figures below came from the earlier uncommitted script, whose size search compared
against the uncalibrated critical value. Under the implemented calibrated method,
Design A at margin 0.20 needs 90 tasks, not 96; Design C 110, not 112, and 330 at margin
0.10, not 328. Design A at margin 0.10 (264) and the powers quoted at 96 tasks are
unchanged. Use the tool's output for any freeze. Nothing is frozen: the design, margin,
alpha, planning target and the rest of the questions at the end stay the owner's.

#### Terms used here

- **Task block:** one sealed evaluation task with its paired runs in the arms of a
  contrast. Tasks, not runs, are the independent units (lines 38–39).
- **Gain, loss, discordance, net difference:** for one endpoint, gain is the chance
  that only the treatment run passes and loss the chance that only the control run
  passes (lines 122–126). Discordance is gain + loss; net difference is gain − loss.
- **k:** the number of confirmatory contrasts. The tool gives each one-sided bound
  the tail 0.05/(2k) (`scripts/pilot-v2-power.py:191`).
- **Tool method:** the repository tool's bound, half-width
  `sqrt(2 * ln(1 / tail) / tasks)` (`scripts/pilot-v2-power.py:198`, `:205`). It does
  not use the variance of the differences.
- **Variance-based method:** a comparison computed for this proposal by an
  uncommitted script, kept privately with the proposal's working files in
  `runs/pilot-v2-proposals-2026-11-20/`; it is not in the repository. The success safeguard uses Tango's
  score test for a paired difference (Tango 1998, *Statistics in Medicine*
  17:891–908). Its critical value is raised until its worst size on a 61-point grid of
  discordance at the margin boundary is within the tail. The improvement uses the
  exact one-sided sign test on discordant tasks. Task model, tails and scenarios are
  the same as the tool's. Under owner decision 20 it will be added to the repository
  tool with tests (above).
- **Planning scenario:** H2 below. Success has no true difference with 20 %
  discordance; discovery has net +0.20 (gain 0.25, loss 0.05).
- **G:** the per-run cap on generation requests. It is not frozen for v2 (lines
  277–279); v1 froze 6 (`specs/003-agent-retrieval-context/contracts/bounded-pilot-v1.md:125`).

#### What closing 009 T005 does to the arms — a question for the owner

**Answered, 2026-11-20:** owner decision 18, "Drop retrieval arms"
([record](../../../docs/validation.md#owner-decisions--2026-11-19)), chose option b.
Item 3 and T006's acceptance are amended
([T006](../spec.md#t006--validate-retrieval-repairs-in-real-coding-tasks)). The
question is kept as it was put.

Item 3 (lines 45–48) names four retrieval arms: current Foundry, the mixed-intent
repair (001 T010), the anchorless repair (009 T005) and both repairs. It says an
unavailable repair is a readiness blocker, not an arm to drop. Owner decision 17 of
2026-11-19 closed 009 T005 without promotion
([record](../../../docs/validation.md#owner-decisions--2026-11-19);
`specs/009-optional-semantic-retrieval/spec.md:1374-1386`). There is no anchorless
repair to bind, so the "anchorless repair" arm would equal current Foundry, and "both
repairs" would equal the mixed-intent arm. In addition, 001 T010 has been on by
default since 2026-11-11 (`specs/003-agent-retrieval-context/spec.md:840-842`,
`:895-898`). "Current Foundry" therefore already contains the mixed-intent repair,
and the old pre-repair baseline is the T010-off configuration
(`--experimental-behavior-context=false`). Read literally, line 47 now blocks
readiness permanently. The owner chooses one of:

| Option | Retrieval arms | Retrieval contrasts | Consequence |
| --- | --- | --- | --- |
| a. One T010 contrast | T010 off; T010 on (default) | 1: T010 on vs off, confirmatory (adds 1 to k) or descriptive | A confirmatory contrast contradicts lines 23–25 ("does not gate it or retroactively establish an agent benefit for it"), so those lines need an owner amendment. Either way it adds a two-arm retrieval stratum to the schedule (Design B). A descriptive contrast costs runs but no alpha. |
| b. No retrieval arm | None; every arm runs current Foundry at its defaults | 0 | Item 3 and T006's acceptance of "the selected retrieval change" (`specs/003-agent-retrieval-context/spec.md:886-891`) need amending. The four retrieval intents of lines 40–41 stay as declared strata and are reported per intent. T006's retrieval half stays open or becomes a named limitation, as T005 did. |
| c. Wait for an anchorless candidate | Four, unchanged | Up to 3 | V2 stays blocked until T005's re-entry trigger is met (a candidate with about +30 net located definitions on development G2, or an owner-selected change of embedding representation or query handling, `specs/009-optional-semantic-retrieval/spec.md:1382-1386`) and a repair is promoted. |

Designs A and C below assume option b; Design B and the seven-contrast variant of C
assume option a with a confirmatory T010 contrast. None assumes option c. The
[arm-binding proposal](#proposed-arm-and-interface-bindings--2026-11-20) poses the same question with
different options (its choice 2), and the
[task manifest proposal](#proposed-task-manifest-authoring-and-allocation--2026-11-20) notes that no
allocation can be computed until it is settled (its section F). With option b
chosen, only Design A and Design C with k = 6 remain candidates.

#### Hypothetical scenarios

The author chose these round values to span no difference through large differences.
They are not estimates, and none comes from v1's eight successes (lines 217–218). In
both methods the baseline only limits which gain and loss are possible
(`scripts/pilot-v2-power.py:41-45`); power depends on gain and loss alone.

| Scenario | Success: baseline, gain, loss (net) | Discovery: baseline, gain, loss (net) |
| --- | --- | --- |
| H1 | 0.50, 0.05, 0.05 (0) | 0.30, 0.15, 0.05 (+0.10) |
| H2, planning | 0.50, 0.10, 0.10 (0) | 0.30, 0.25, 0.05 (+0.20) |
| H3 | 0.50, 0.20, 0.20 (0) | 0.30, 0.35, 0.05 (+0.30) |
| H4 | 0.30, 0.10, 0.05 (+0.05) | 0.20, 0.45, 0.05 (+0.40) |
| H5 | 0.70, 0.15, 0.05 (+0.10) | 0.50, 0.30, 0.10 (+0.20) |
| H6 | 0.50, 0.25, 0.05 (+0.20) | 0.10, 0.55, 0.05 (+0.50) |
| H7 | 0.50, 0.05, 0.15 (−0.10) | 0.50, 0.10, 0.10 (0) |

H7 checks error rates. Its discovery row is the null, and its success row sits on the
0.10 margin, so at margin 0.10 both powers must stay within the tail. At margin 0.20
its success row is a true 10-point loss that a 0.20 margin permits.

Every calculation uses overall alpha 0.05, required discovery gain 0 (strictly
better) and one run per arm per task (R = 1) unless stated.

#### Result 1: the tool's bound needs hundreds of tasks per contrast

Half-width from the tool, the same for both endpoints:

| k | Tail per bound | 20 tasks | 40 | 80 | 160 | 320 | 640 |
| --- | --- | --- | --- | --- | --- | --- | --- |
| 2 | 0.01250 | 0.662 | 0.468 | 0.331 | 0.234 | 0.165 | 0.117 |
| 3 | 0.00833 | 0.692 | 0.489 | 0.346 | 0.245 | 0.173 | 0.122 |
| 6 | 0.00417 | 0.740 | 0.523 | 0.370 | 0.262 | 0.185 | 0.131 |
| 7 | 0.00357 | 0.751 | 0.531 | 0.375 | 0.265 | 0.188 | 0.133 |

A contrast passes its success safeguard only if the observed success difference
exceeds the half-width minus the margin. It passes its improvement claim only if the
observed discovery difference exceeds the half-width. At 80 tasks, k = 2 and margin
0.20, the treatment must beat the control on verified success by more than 0.13 just
to show that it is not worse.

Tool success power at margin 0.20 (each cell is k = 2 / k = 7):

| Scenario | 20 tasks | 40 | 80 | 160 | 320 | 640 |
| --- | --- | --- | --- | --- | --- | --- |
| H1 | <0.001 / <0.001 | <0.001 / <0.001 | <0.001 / <0.001 | 0.084 / 0.004 | 0.979 / 0.733 | 1.000 / 1.000 |
| H2 | <0.001 / <0.001 | <0.001 / <0.001 | 0.004 / <0.001 | 0.165 / 0.032 | 0.925 / 0.669 | 1.000 / 1.000 |
| H3 | <0.001 / <0.001 | 0.004 / <0.001 | 0.032 / 0.005 | 0.246 / 0.095 | 0.845 / 0.621 | 1.000 / 0.997 |
| H4 | <0.001 / <0.001 | <0.001 / <0.001 | 0.030 / 0.001 | 0.696 / 0.301 | 1.000 / 0.998 | 1.000 / 1.000 |
| H5 | <0.001 / <0.001 | 0.011 / <0.001 | 0.258 / 0.049 | 0.973 / 0.841 | 1.000 / 1.000 | 1.000 / 1.000 |
| H6 | 0.009 / 0.001 | 0.217 / 0.045 | 0.887 / 0.627 | 1.000 / 1.000 | 1.000 / 1.000 | 1.000 / 1.000 |
| H7 | <0.001 / <0.001 | <0.001 / <0.001 | <0.001 / <0.001 | <0.001 / <0.001 | 0.004 / <0.001 | 0.170 / 0.031 |

Tool success power at margin 0.10 with no true difference (k = 2 / k = 7):

| Scenario | 320 tasks | 640 | 1,000 |
| --- | --- | --- | --- |
| H1 | <0.001 / <0.001 | 0.094 / 0.005 | 0.742 / 0.258 |
| H2 | 0.005 / <0.001 | 0.177 / 0.035 | 0.677 / 0.323 |
| H3 | 0.035 / 0.006 | 0.256 / 0.100 | 0.627 / 0.373 |

At margin 0.10 the tool method reaches 0.80 within its 1,000-task limit
(`scripts/pilot-v2-power.py:178-179`) only when the treatment truly improves success:
H5 (+0.10) gives 0.931 / 0.672 at 320 tasks and H6 (+0.20) gives 0.949 / 0.803 at 160.
H7 stays below 0.001 at every task count, as it must.

Tool discovery power (k = 2 / k = 7; it does not depend on the margin):

| Scenario | 20 tasks | 40 | 80 | 160 | 320 | 640 |
| --- | --- | --- | --- | --- | --- | --- |
| H1 (+0.10) | <0.001 / <0.001 | <0.001 / <0.001 | <0.001 / <0.001 | <0.001 / <0.001 | 0.005 / <0.001 | 0.170 / 0.032 |
| H2 (+0.20) | <0.001 / <0.001 | 0.001 / <0.001 | 0.011 / 0.001 | 0.196 / 0.052 | 0.897 / 0.649 | 1.000 / 1.000 |
| H3 (+0.30) | 0.001 / <0.001 | 0.032 / 0.003 | 0.308 / 0.096 | 0.932 / 0.783 | 1.000 / 1.000 | 1.000 / 1.000 |
| H4 (+0.40) | 0.015 / 0.001 | 0.250 / 0.066 | 0.854 / 0.616 | 1.000 / 0.998 | 1.000 / 1.000 | 1.000 / 1.000 |
| H5 (+0.20) | <0.001 / <0.001 | 0.003 / <0.001 | 0.025 / 0.003 | 0.234 / 0.083 | 0.858 / 0.628 | 1.000 / 0.998 |
| H6 (+0.50) | 0.088 / 0.013 | 0.662 / 0.350 | 0.994 / 0.962 | 1.000 / 1.000 | 1.000 / 1.000 | 1.000 / 1.000 |
| H7 (0) | <0.001 / <0.001 | <0.001 / <0.001 | <0.001 / <0.001 | <0.001 / <0.001 | <0.001 / <0.001 | <0.001 / <0.001 |

The k = 3 and k = 6 tables lie between these two; the input below reproduces them.

#### Result 2: a variance-based paired method needs under a third of the tasks

Planning scenario H2 under both methods (each cell is k = 2 / k = 7):

| Endpoint, margin, method | 20 tasks | 40 | 80 | 160 | 320 | 640 |
| --- | --- | --- | --- | --- | --- | --- |
| Success, 0.20, tool | <0.001 / <0.001 | <0.001 / <0.001 | 0.004 / <0.001 | 0.165 / 0.032 | 0.925 / 0.669 | 1.000 / 1.000 |
| Success, 0.20, variance-based | 0.218 / 0.062 | 0.567 / 0.381 | 0.934 / 0.828 | 0.999 / 0.996 | 1.000 / 1.000 | 1.000 / 1.000 |
| Success, 0.10, tool | <0.001 / <0.001 | <0.001 / <0.001 | <0.001 / <0.001 | <0.001 / <0.001 | 0.005 / <0.001 | 0.177 / 0.035 |
| Success, 0.10, variance-based | 0.051 / 0.008 | 0.153 / 0.050 | 0.337 / 0.173 | 0.681 / 0.492 | 0.951 / 0.882 | 1.000 / 0.998 |
| Discovery, tool | <0.001 / <0.001 | 0.001 / <0.001 | 0.011 / 0.001 | 0.196 / 0.052 | 0.897 / 0.649 | 1.000 / 1.000 |
| Discovery, variance-based | 0.107 / 0.021 | 0.465 / 0.253 | 0.850 / 0.706 | 0.995 / 0.984 | 1.000 / 1.000 | 1.000 / 1.000 |

Smallest task count per contrast at which both planning endpoints reach the target.
The search used steps of 4 tasks, over 20–1,000 for the tool method and 20–640 for the
variance-based method; the design sizes below were rechecked exactly.

| k | Margin | Tool, each ≥ 0.80 | Tool, each ≥ 0.90 | Variance-based, each ≥ 0.80 | Variance-based, each ≥ 0.90 |
| --- | --- | --- | --- | --- | --- |
| 2 | 0.20 | 284 | 328 | 76 | 92 |
| 2 | 0.10 | over 1,000 | over 1,000 | 204 | 264 |
| 3 | 0.20 | 312 | 356 | 80 | 100 |
| 3 | 0.10 | over 1,000 | over 1,000 | 224 | 288 |
| 6 | 0.20 | 348 | 396 | 92 | 112 |
| 6 | 0.10 | over 1,000 | over 1,000 | 260 | 328 |
| 7 | 0.20 | 360 | 404 | 92 | 116 |
| 7 | 0.10 | over 1,000 | over 1,000 | 268 | 336 |

Each endpoint at 0.90 guarantees joint power of at least 0.80 by the tool's union
bound (`scripts/pilot-v2-power.py:222`). Acceptance needs both endpoints, so the
designs below use the 0.90 column.

Calibration check: uncalibrated, the Tango test exceeded its tail on the boundary
grid, for example 0.0214 against 0.0125 at 20 tasks (k = 2, margin 0.10) and 0.0135
at 96 tasks (margin 0.20). The calibrated threshold brought it back within the tail on
the grid: 0.0121 at 96 tasks, with critical value 2.280 instead of 2.241. This is a
check on a grid, not a proof over the whole null hypothesis.

Using this method is a freeze decision with work attached. The frozen analysis code
would need it, with size and power tests like those in
`scripts/test_pilot_v2_power.py`, and lines 127–137 and 197–205 would change. Until
then the tool method is the only executable, deterministic calculation in the
repository, and the variance-based figures cannot be reproduced from it. Owner
decision 20 of 2026-11-20, "Implement and test it", added the method to
`scripts/pilot-v2-power.py` with tests; see the note at the start of this subsection
for the sizes that change. Using it in the frozen analysis remains part of the owner's
design choice.

A second, smaller lever is alpha spending. The tool spends 0.05/(2k) on each bound. If
a contrast's only claim is its joint acceptance (both bounds together), the
intersection-union principle (Berger 1982, *Technometrics* 24:295–300) permits
0.05/k per bound. Emulating that by halving k in the tool lowers the 0.90 size from
328 to 284 tasks for Design A (variance-based: 92 to 80), and from 396 to 356 for
Design C (112 to 100). A standalone success-only or discovery-only claim would then
be descriptive.

#### Result 3: repetitions do not help the tool's bound

Tool method, k = 2, margin 0.20, using the per-task block form (lines 170–195). Per
run, success has gain = loss = 0.10, and discovery has gain 0.30 and loss 0.05 (net
+0.25). ρ is the chance that all R runs of a task share one difference; otherwise
the runs are independent. Probabilities were rounded to six decimals, with the
residual put on the largest entry so that they sum exactly to one.

| Tasks | R | ρ | Success power | Discovery power | Runs per contrast pair |
| --- | --- | --- | --- | --- | --- |
| 80 | 1 | — | 0.004 | 0.088 | 160 |
| 80 | 2 | 0.5 | 0.002 | 0.066 | 320 |
| 80 | 2 | 0 | <0.001 | 0.033 | 320 |
| 80 | 3 | 0 | <0.001 | 0.010 | 480 |
| 160 | 1 | — | 0.165 | 0.643 | 320 |
| 160 | 2 | 0.5 | 0.142 | 0.680 | 640 |
| 160 | 2 | 0 | 0.094 | 0.717 | 640 |
| 160 | 3 | 0 | 0.046 | 0.738 | 960 |

With R = 2 and ρ = 1 the tool gives the R = 1 values exactly. Under the tool method
the half-width depends only on the task count. Repeated runs reduce the spread of each
task's mean without moving the threshold, so they help only where the true effect
already clears it (discovery at 160 tasks). They hurt elsewhere, and they multiply
runs by R. A variance-based method could gain from R > 1 when ρ < 1; that was not
computed here. T006's "repeated paired runs account for model variability"
(`specs/003-agent-retrieval-context/spec.md:869-870`) can be met by a frozen,
descriptive repeated subsample (below), not by counting repetitions as tasks.

#### Result 4: a pooled safeguard can hide a regression in one intent

Tool method, k = 2, margin 0.20, four equal intent strata. In strata 1–3 success has
gain = loss = 0.10. Stratum 4 hides a true success loss of 0.20 (gain 0.05, loss
0.25). Discovery has gain 0.35 and loss 0.05 everywhere.

| Tasks per stratum (total) | Pooled success | Success, strata 1–3 each | Success, stratum 4 | Pooled discovery |
| --- | --- | --- | --- | --- |
| 20 (80) | <0.001 | <0.001 | <0.001 | 0.308 |
| 80 (320) | 0.294 | 0.004 | <0.001 | >0.999 |
| 160 (640) | 0.967 | 0.165 | <0.001 | >0.999 |

At 640 tasks the pooled safeguard passes 97 % of the time although one intent loses
20 points, as line 164 warns. One alternative is a blocking flag: an intent
stratum whose observed success difference falls below the negative margin blocks its
contrast. That costs no alpha, but at Design A's sizes it blocks often when no intent
regresses. The table gives the exact chance of at least one false block under H2's
success row:

| Design A layout | Margin 0.20 | Margin 0.10 |
| --- | --- | --- |
| 96 tasks: five strata of 12 and six of 6 | 0.522 | 0.952 |
| 264 tasks: eleven strata of 24 | 0.196 | 0.765 |
| 330 tasks: eleven strata of 30 | 0.044 | 0.574 |

This proposal therefore reports per-intent results descriptively, as lines 222–223
require. The alternative is to make per-intent guards confirmatory contrasts, which
raises k and needs far larger strata.

#### Elements common to the three designs

- **Unit:** a sealed evaluation task (lines 34–39). Development and calibration tasks
  are separate and are neither counted nor costed here.
- **Repetitions:** R = 1 in every confirmatory contrast. As an owner option, a frozen
  descriptive subsample, for example 10 % of tasks run three times in each arm, would
  estimate within-task variability. It adds 20 % to the runs.
- **Order:** frozen seeded randomization, balanced within each stratum (line 67).
  With three arms, all six orders occur equally often, so each stratum is a multiple of
  6 tasks. With two arms, AB and BA occur equally often, so each stratum is a multiple
  of 2.
- **Clustering (line 216):** tasks count as independent only if their sources are.
  This proposal caps the number of tasks per repository family and reports the
  repository as a factor; the owner or the freeze sets the cap. Without such a cap,
  the effective independent count is below the counts shown here.
- **Endpoints:** per run, verified success means the hidden oracle passes. The freeze
  sets one binary improvement endpoint per contrast before outcomes: complete
  supporting evidence in a model request for retrieval tasks, or the "necessary
  evidence" column of the matrix (lines 79–86) for task-context contrasts. Retrieval
  tasks belong only to Design B's and C7's retrieval stratum, which decision 18
  removes.
- **Failures and gaps:** an oracle failure, behavior failure, exhausted cap or
  resource-limited refusal is a valid failed run, as in v1
  (`specs/003-agent-retrieval-context/contracts/bounded-pilot-v1.md:190`, `:213`). A
  capture, provenance or containment fault, or an unstarted row, makes the contrast
  incomplete and therefore descriptive (lines 221–224). No task is replaced, and the
  sample is never extended or stopped early (lines 68–69, 219).

**Proposed decision rule for each confirmatory contrast** (treatment T, control K):
use every scheduled task of the contrast. Compute the mean paired difference T − K for
verified success and for the contrast's improvement endpoint. Compute each one's
one-sided lower confidence bound at tail 0.05/(2k), using the frozen method. Accept the
contrast only if all of these hold:

- the success bound is above minus the margin (0.10 or 0.20, the owner's choice);
- the improvement bound is above g (g = 0 proposed);
- the contrast's frozen cost and latency bounds hold;
- every scheduled row of the contrast has complete capture and a valid oracle verdict.

Otherwise the contrast is not accepted, and an incomplete contrast is descriptive
(line 224). Report each arm's absolute success, both differences with their bounds,
per-intent results with denominators and every failed attempt (lines 222–223).

#### Design A — task-context bundle (k = 2)

- **Arms:** ordinary control O, passive retained context P and selective
  intervention S (lines 49–50), all on current Foundry at its defaults. Host memory and
  auto-learn stay off in every arm (lines 60–66), and so does the implicit `apple`
  provider (owner decision 19, 2026-11-20; item 5).
- **Bindings:** the [arm-binding proposal](#proposed-arm-and-interface-bindings--2026-11-20) maps O
  and P to its arms A1 and A3 and finds no binding today for S (its choice 5).
- **Confirmatory contrasts:** C1, P vs O; C2, S vs P.
- **Descriptive only:** S vs O, and all six capability rows of lines 79–86. No
  capability can be accepted, so all six stay open
  (`specs/003-agent-retrieval-context/spec.md:880`).
- **Retrieval:** option b, no retrieval arm, as owner decision 18 selected.
- **Strata:** 11 declared intent strata: the four retrieval intents of lines 40–41
  and the seven task-context situations of lines 41–44, including silence cases. Each
  stratum is a whole number of six-task order blocks. The
  [task manifest proposal](#proposed-task-manifest-authoring-and-allocation--2026-11-20)
  instead gives every task exactly one of the four intent strata and a
  non-exclusive set of context cases; the freeze must reconcile the two into one
  fixed, disjoint stratum per task (lines 139–141).
- **Schedule:** one shared suite; every task runs once in each of the three arms, so
  runs = 3 × tasks.

#### Design B — bundle plus a T010 contrast (k = 3)

- **Status:** not a candidate after owner decision 18 of 2026-11-20, "Drop retrieval
  arms", because its retrieval part needs option a. Kept for comparison.
- **Task-context part:** Design A's three arms and two contrasts on a task-context
  suite with the seven situational strata.
- **Retrieval part:** option a with a confirmatory contrast. A separate retrieval
  stratum covers the four retrieval intents in equal parts, each a multiple of 2
  tasks (so the part is a multiple of 8), run in two
  arms: T010 off and T010 on (the O configuration), with no task-context treatment.
  C3 is T010 on vs off.
- **Precondition:** the owner amends lines 23–25 to allow a prospective
  agent-benefit claim for T010.
- **Schedule:** runs = 3 × task-context tasks + 2 × retrieval tasks.

#### Design C — six capability contrasts (k = 6; k = 7 with T010)

- **Contrasts:** six disjoint capability strata, one per row of lines 79–86 (003
  T007, 008 T003, 003 T008, 008 T005, 008 T004 and 003 T009). Each stratum's tasks
  exercise its capability and run in the two configurations of its contrast. Silence
  cases (lines 43–44) belong to the 008 T004 stratum. Optional C7: Design B's retrieval
  stratum as a seventh contrast (option a). C7 is not a candidate after owner decision
  18; Design C with k = 6 remains one.
- **What it gives:** each capability can be accepted or rejected on its own, which
  Designs A and B cannot do (lines 75–78).
- **Bindings:** under the
  [arm-binding proposal](#proposed-arm-and-interface-bindings--2026-11-20), only the
  003 T008 and 003 T009 contrasts are bindable today. Its proposed statuses make 003
  T007 descriptive, leave 008 T003 and 008 T005 descriptive and needing the MCP
  binding, and block 008 T004. Design C therefore needs that apparatus work, or an
  owner amendment, before its six contrasts can all be confirmatory.
- **Schedule:** runs = 12 × tasks per stratum, or 14 × with T010. The binding cost is
  task demand: 672 to 2,828 sealed tasks, each with an oracle and a rights record
  (lines 36–37); 672 to 2,376 without C7.
- **Not computed:** a shared-ladder variant would run every task in seven cumulative
  arms, with each adjacent pair forming one contrast. It needs a sixth of the tasks
  but 7 runs per task, and it works only if every task exercises every capability;
  otherwise each contrast is diluted.

#### Sizes, power and provider calls

Tasks per contrast is the smallest balanced size at or above the 0.90 search result:
a multiple of 6 for three-arm task-context parts, of 8 for Design B's four equal
retrieval strata, and of 2 for Design C's two-arm strata.
Planning power is success / discovery / joint lower bound in H2. For Design B with the
variance-based method at margin 0.20, the first triple is the task-context part (102
tasks) and the second the retrieval part (104 tasks). The B and C7 rows assume option
a, which owner decision 18 did not select; they remain for comparison only.

| Design | k | Method | Margin | Tasks per contrast | Sealed tasks | Runs | Generation requests, G = 6 / G = 24 | All provider HTTP attempts, G = 6 / G = 24 | Worst-case host hours | Planning power |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| A | 2 | Tool | 0.20 | 330 | 330 | 990 | 5,941 / 23,761 | 12,873 / 48,513 | 165 | 0.938 / 0.912 / 0.850 |
| A | 2 | Variance-based | 0.20 | 96 | 96 | 288 | 1,729 / 6,913 | 3,747 / 14,115 | 48 | 0.968 / 0.922 / 0.890 |
| A | 2 | Variance-based | 0.10 | 264 | 264 | 792 | 4,753 / 19,009 | 10,299 / 38,811 | 132 | 0.901 / >0.999 / 0.901 |
| B | 3 | Tool | 0.20 | 360 + 360 | 720 | 1,800 | 10,801 / 43,201 | 23,403 / 88,203 | 300 | 0.944 / 0.919 / 0.863 |
| B | 3 | Variance-based | 0.20 | 102 + 104 | 206 | 514 | 3,085 / 12,337 | 6,685 / 25,189 | 86 | 0.965 / 0.916 / 0.881; 0.963 / 0.922 / 0.885 |
| B | 3 | Variance-based | 0.10 | 288 + 288 | 576 | 1,440 | 8,641 / 34,561 | 18,723 / 70,563 | 240 | 0.902 / >0.999 / 0.902 |
| C | 6 | Tool | 0.20 | 396 | 2,376 | 4,752 | 28,513 / 114,049 | 61,779 / 232,851 | 792 | 0.935 / 0.912 / 0.847 |
| C | 6 | Variance-based | 0.20 | 112 | 672 | 1,344 | 8,065 / 32,257 | 17,475 / 65,859 | 224 | 0.953 / 0.909 / 0.862 |
| C | 6 | Variance-based | 0.10 | 328 | 1,968 | 3,936 | 23,617 / 94,465 | 51,171 / 192,867 | 656 | 0.900 / >0.999 / 0.900 |
| C7 | 7 | Tool | 0.20 | 404 | 2,828 | 5,656 | 33,937 / 135,745 | 73,531 / 277,147 | 943 | 0.934 / 0.903 / 0.837 |
| C7 | 7 | Variance-based | 0.20 | 116 | 812 | 1,624 | 9,745 / 38,977 | 21,115 / 79,579 | 271 | 0.961 / 0.913 / 0.873 |
| C7 | 7 | Variance-based | 0.10 | 336 | 2,352 | 4,704 | 28,225 / 112,897 | 61,155 / 230,499 | 784 | 0.898 / >0.999 / 0.898 |

With the tool method at margin 0.10, no design reaches the target within 1,000 tasks
per contrast. For C7 at margin 0.10, the calibrated success power at 336 tasks is
0.898, just under the 0.90 that the uncalibrated search found.

How the call columns are counted:

- **Caps, not usage.** The counts are caps for the evaluation schedule plus one
  compatibility probe (line 281). Actual calls are at most these, but the reservation
  at the ratecard must cover the caps (lines 280–282).
- **Generation requests** = runs × G + 1.
- **All provider HTTP attempts** = runs × (2G + 1) + 3. This follows v1's request
  structure: one count call per generation and one catalog GET per run
  (`specs/003-agent-retrieval-context/contracts/bounded-pilot-v1.md:126-129`). V2's
  provider, routes and caps are not frozen (lines 270–282), so this structure may
  change.
- **G values.** G = 6 is v1's frozen cap; G = 24 is a hypothetical for harder
  multi-file tasks.
- **Not included:** development and calibration runs, any auxiliary generation (line
  86) and retries, which are not allowed (line 272).
- **Worst-case host hours** = runs × 10 minutes. This is serial host time at v1's
  600-second slot wall
  (`specs/003-agent-retrieval-context/contracts/bounded-pilot-v1.md:137`).

Sensitivity at the design sizes: success power, then discovery power, in each
scenario.

| Design size | H1 | H2 | H3 | H4 | H5 | H6 | H7 |
| --- | --- | --- | --- | --- | --- | --- | --- |
| A, tool, 0.20, 330 | 0.985; 0.005 | 0.938; 0.912 | 0.862; >0.999 | >0.999; >0.999 | >0.999; 0.874 | >0.999; >0.999 | 0.004; <0.001 |
| A, variance-based, 0.20, 96 | 0.999; 0.405 | 0.968; 0.922 | 0.790; 0.998 | >0.999; >0.999 | >0.999; 0.793 | >0.999; >0.999 | 0.404; 0.007 |
| A, variance-based, 0.10, 264 | 0.994; 0.917 | 0.901; >0.999 | 0.619; >0.999 | >0.999; >0.999 | >0.999; 0.999 | >0.999; >0.999 | 0.011; 0.009 |
| B, tool, 0.20, 360 (both parts) | 0.988; 0.004 | 0.944; 0.919 | 0.870; >0.999 | >0.999; >0.999 | >0.999; 0.882 | >0.999; >0.999 | 0.003; <0.001 |
| B, variance-based, 0.20, 102 | 0.999; 0.365 | 0.965; 0.916 | 0.764; 0.997 | >0.999; >0.999 | >0.999; 0.777 | >0.999; >0.999 | 0.371; 0.004 |
| B, variance-based, 0.20, 104 | 0.999; 0.374 | 0.963; 0.922 | 0.771; 0.998 | >0.999; >0.999 | >0.999; 0.788 | >0.999; >0.999 | 0.350; 0.004 |
| B, variance-based, 0.10, 288 | 0.995; 0.920 | 0.902; >0.999 | 0.605; >0.999 | >0.999; >0.999 | >0.999; 0.999 | >0.999; >0.999 | 0.007; 0.006 |
| C, tool, 0.20, 396 | 0.984; 0.002 | 0.935; 0.912 | 0.858; >0.999 | >0.999; >0.999 | >0.999; 0.874 | >0.999; >0.999 | 0.001; <0.001 |
| C, variance-based, 0.20, 112 | 0.998; 0.333 | 0.953; 0.909 | 0.726; 0.998 | >0.999; >0.999 | >0.999; 0.752 | >0.999; >0.999 | 0.292; 0.002 |
| C, variance-based, 0.10, 328 | 0.996; 0.924 | 0.900; >0.999 | 0.577; >0.999 | >0.999; >0.999 | >0.999; 0.999 | >0.999; >0.999 | 0.003; 0.003 |
| C7, tool, 0.20, 404 | 0.983; 0.001 | 0.934; 0.903 | 0.856; >0.999 | >0.999; >0.999 | >0.999; 0.865 | >0.999; >0.999 | 0.001; <0.001 |
| C7, variance-based, 0.20, 116 | 0.999; 0.319 | 0.961; 0.913 | 0.731; 0.998 | >0.999; >0.999 | >0.999; 0.752 | >0.999; >0.999 | 0.295; 0.002 |
| C7, variance-based, 0.10, 336 | 0.996; 0.923 | 0.898; >0.999 | 0.565; >0.999 | >0.999; >0.999 | >0.999; 0.999 | >0.999; >0.999 | 0.003; 0.002 |

What this table shows:

- **A 0.20 margin admits real losses.** With the variance-based method, a design at
  margin 0.20 accepts a true 10-point success loss (H7) with probability 0.29 to
  0.40. The tool method almost never does (at most 0.004), because it is conservative
  everywhere.
- **High success discordance costs power.** In H3, 40 % of tasks change outcome
  between arms with no net difference, and success power falls to 0.57–0.79 under the
  variance-based method.
- **Small gains are hard to detect.** Designs sized for a discovery gain of +0.20
  detect +0.10 (H1) with probability 0.32–0.41 under the variance-based method at
  margin 0.20, and at most 0.005 under the tool method.

#### Questions for the owner

1. **Retrieval arms after 009 T005's closure.** Choose option a, b or c above. Under
   option a, choose a confirmatory or a descriptive T010 contrast; a confirmatory
   contrast needs an amendment to lines 23–25. **Answered, 2026-11-20:** option b
   (owner decision 18, "Drop retrieval arms").
2. **Confirmatory scope.** Choose Design A (task-context bundle, k = 2), Design B
   (bundle plus T010, k = 3) or Design C (six capabilities, k = 6, or 7 with T010).
   Under A and B the six capabilities stay open. After decision 18 the choice is
   between Design A and Design C with k = 6; it remains open.
3. **Success margin.** 0.20 accepts a treatment that loses up to 20 points of verified
   success; under the variance-based method a true 10-point loss passes 29–40 % of the
   time. 0.10 needs about 2.9 times the tasks, and the tool method cannot reach it.
4. **Analysis method.** Keep the tool's conservative bound, which needs about 3.5
   times the tasks at margin 0.20. Or have a variance-based paired method
   implemented, tested and frozen with the analysis code (lines 207, 219).
   **Partly answered, 2026-11-20:** owner decision 20, "Implement and test it": the
   calibrated method is implemented in `scripts/pilot-v2-power.py` with tests and
   proposed for the freeze. Which method the frozen analysis uses is decided with the
   design; nothing is frozen.
5. **Alpha per bound.** Use 0.05/(2k), as the tool does, or 0.05/k under the
   intersection-union reading. The second needs about 10–15 % fewer tasks, and
   single-endpoint claims become descriptive.
6. **Planning target.** Size for H2 with each endpoint at least 0.90 (joint at least
   0.80), or use other values. Also set the required discovery gain g; 0 is proposed.
7. **Repetitions.** R = 1 in confirmatory contrasts, with or without a frozen
   descriptive repeated subsample.
8. **Per-intent results.** Keep them descriptive (proposed), use a blocking flag
   (which blocks 4–95 % of the time with no true regression at Design A's sizes), or
   add confirmatory per-intent guards (which raises k).
9. **Clustering.** Set the maximum number of tasks per repository family.
10. **Calls and spend.** Set the per-run generation cap G and the request structure
    and routes, which set the call counts, and the ratecard and budget that turn calls
    into spend.
11. **Tasks, rights and key.** Decide on sourcing and task-data rights for between 96
    and 2,828 sealed tasks, and on the provider key. Without Designs B and C7 the
    upper end is 2,376.

**Author's suggestion, not a decision:** option b, Design A, and the variance-based
method once it is implemented and tested, with R = 1 and descriptive per-intent
results. Use margin 0.10 (264 tasks, 792 runs) if that many sealed tasks with rights
can be obtained; otherwise use margin 0.20 (96 tasks, 288 runs) and state its weaker
safeguard. The six capabilities and T006's retrieval half would stay open. Design A's
C2 needs the selective-intervention arm, which the
[arm-binding proposal](#proposed-arm-and-interface-bindings--2026-11-20) finds unbound
today (its choice 5); until that arm exists or the owner amends item 4, only C1 can
run. That proposal also suggests a different confirmatory family, 003 T009 with
optional 003 T008 (its choice 3), which none of Designs A–C sizes. The owner may
replace any part of this. On 2026-11-20 the owner took option b (decision 18) and
asked for the variance-based method to be implemented and tested (decision 20).
With decision 18, T006's acceptance no longer has a retrieval half
([T006](../spec.md#t006--validate-retrieval-repairs-in-real-coding-tasks)); the
design, margin and the other questions above stay open.

#### Reproducing the tool figures

The tool tables come from `scripts/pilot-v2-power.py` at `afa7b38`, run unchanged,
after `python3 scripts/test_pilot_v2_power.py` passed its 17 tests. Every output
reported `sensitivity_only_not_frozen`, `execution_authorized: false`,
`provider_calls: 0` and `spend_usd: 0`. The k = 2, margin 0.20 grid input is:

```json
{"overall_alpha": 0.05, "confirmatory_contrasts": 2, "noninferiority_margin": 0.2,
 "required_discovery_gain": 0.0, "task_counts": [20, 40, 80, 160, 320, 640],
 "repetitions_per_task": 1, "scenarios": [
  {"name": "H1", "success": {"baseline": 0.5, "gain": 0.05, "loss": 0.05},
   "discovery": {"baseline": 0.3, "gain": 0.15, "loss": 0.05}},
  {"name": "H2", "success": {"baseline": 0.5, "gain": 0.1, "loss": 0.1},
   "discovery": {"baseline": 0.3, "gain": 0.25, "loss": 0.05}},
  {"name": "H3", "success": {"baseline": 0.5, "gain": 0.2, "loss": 0.2},
   "discovery": {"baseline": 0.3, "gain": 0.35, "loss": 0.05}},
  {"name": "H4", "success": {"baseline": 0.3, "gain": 0.1, "loss": 0.05},
   "discovery": {"baseline": 0.2, "gain": 0.45, "loss": 0.05}},
  {"name": "H5", "success": {"baseline": 0.7, "gain": 0.15, "loss": 0.05},
   "discovery": {"baseline": 0.5, "gain": 0.3, "loss": 0.1}},
  {"name": "H6", "success": {"baseline": 0.5, "gain": 0.25, "loss": 0.05},
   "discovery": {"baseline": 0.1, "gain": 0.55, "loss": 0.05}},
  {"name": "H7", "success": {"baseline": 0.5, "gain": 0.05, "loss": 0.15},
   "discovery": {"baseline": 0.5, "gain": 0.1, "loss": 0.1}}]}
```

To reproduce the other tool tables, change `confirmatory_contrasts` to 3, 6 or 7 and
`noninferiority_margin` to 0.1. The 1,000-task column used H1–H3 alone, to stay within
the operation bound. The repetition and intent tables used the `designs` form with the
values stated beside them. The variance-based figures, the minimum-size search and
the false-block chances came from an uncommitted script, kept privately in
`runs/pilot-v2-proposals-2026-11-20/`, and are not reproducible from the repository.

## One capture and accounting owner

Reuse the Rust `pilot_capture` apparatus and its narrow installed-OMP bridge.
Do not add a new proxy, experiment service, database or generic workflow owner.
The private launch manifest binds the contract digest, apparatus/executable hashes,
host/model/settings, task and oracle manifests, allocation, profile/model inputs,
all permitted routes, permissions and actual run directory. Rehash inputs at use.

Owner decision, 2026-11-19, "Digest plus pinned inputs"
([record](../../../docs/validation.md#owner-decisions--2026-11-19)): the v2 host's
identity is its exact executable SHA-256 plus the ordered patch list, dependency
lock, Bun runtime and native-addon digests. A byte-for-byte reproducible build is
not required. Owner decision, 2026-11-19, "Keep as reference": the oh-my-pi
reference patches in `docs/references` stay published reference data, which
Foundry does not apply automatically. The host built from them stays private and
is pinned by its SHA-256, as the source-built diagnostic hosts already are in
[`host.rs`](../../../src/pilot_capture/host.rs). A future freeze binds these
values; this draft stays unfunded and unfrozen.

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
Owner decision, 2026-11-20, "Disable in every arm"
([record](../../../docs/validation.md#owner-decisions--2026-11-19)): whatever list
the freeze binds, every v2 launch's `disabledProviders` includes the implicit `apple`
provider (item 5). Reason: the
[crash diagnosis](../../../docs/validation.md#sandbox-profile-segmentation-fault-diagnosis--2026-11-20)
found one stack in all five 2026-11-15 segmentation faults, in the host's check for
Apple's on-device model, with their sandbox cause inferred, not confirmed; ordinary
capture still runs that check on macOS.

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
This began as the no-provider read/edit qualifier's profile. Owner decision,
2026-11-19, "Off in every arm", keeps the off backend and disabled auto-learn in
every v2 arm (item 5 of the arms rules). Foundry's own 008 task context is
separate and is not changed by that decision.

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

**Selected original credential-resolver qualification, 2026-11-16 — locally qualified.**
The pinned native extension export identifies `createAuthStorageSettingsSync` as
`aee`, with function-source SHA-256
25f681643c58b0b08a43f839ab6760b76ab1cbd65e5ff3ba61d110fff8c03ca9.
The original call registers `.catch(() => null)` before the trusted factory.
Native feasibility captures its exact source and original derived promises; both
are fulfilled before that factory. An empty credential snapshot, another sync
invocation, or awaiting only the recovered derived promise is insufficient.

Add bounded test-only observation to the existing native owner consumer. Bind the
actual registration to its pinned native frame, callback shape and exported
function fingerprint, retain at most two source/derived pairs (one synthetic),
and restore the exact Promise.catch descriptor at factory entry. Pass every
original callback, receiver and return object unchanged; invoke no extra resolver
or settings sync. Await both original promises without inspecting their values.
Require a synthetic pending pair to stay pending until explicitly resolved, and
a synthetic rejected source to fail the join despite its fulfilled catch return.
Require ordinary, pending and rejected controls, snapshots before the factory
join and at handler entry, two equal native model readbacks separated by twelve
seconds, empty stderr, no prompt and zero connections. These new cases have a
45-second bound; other controls retain their deadlines. No runtime hook or
product admission authority is added.

Association is upstream 24439066040e006eb08da7b204f0ac41c79d2de9,
`session/auth-broker-config.ts::createAuthStorageSettingsSync` (blob
e212c2a0766f5bfc9ce4c843b389431bbc9f7aa8) and `main.ts::runRootCommand`
(blob 2bfe0b7230e9b3e30aebfd493a03bffb652cdfb3). Relevant upstream
`test/auth-broker-live-settings.test.ts` exercises a settings-driven broker switch;
`test/settings-live-routing.test.ts` exercises policy routing changes. Neither is
executed here. Unlike those tests, Foundry observes and joins the CLI's original
initial resolver under the empty fixed no-provider profile, without changing
auth settings or opening a broker. Source association is not reproducible-build
proof. The initial resolver is separate from the discarded settings listener's
later change chain, demand-driven polling and asynchronous refresh/close work;
those lifetimes and complete startup/transport/parent acknowledgment stay open.

Use an offline hour with 120 seconds reserved for cleanup, one heavy job,
20 GiB disk floor and 8 GiB sampled individual RSS ceiling. Run the whole current
native owner regression, formatting, all-target pilot Clippy and actual Rust 1.90
all-target pilot check. Reuse the five unaffected effective gates and unchanged
conversation/settings/usage/frozen consumers only with exact input/body/artifact
checks. Publish reviewed public tests and records to experimental; prune only
verified new build caches and duplicate extractions after reaping. Retain the
runnable consumer and replay after cleanup. Paid runs and release signing remain
gated; this does not freeze or unlock v2/T006.

The [dated resolver receipt](../../../docs/validation.md#original-native-credential-resolver-join--2026-11-16)
qualifies `4682e9e` with the original [registration observer](../../../tests/pilot_capture/resolver_probe.ts),
[SDK witness](../../../tests/pilot_capture/resolver_witness.mjs) and existing native
consumer. All three cases join the original pair; the materialized no-await
mutation fails its pending assertion. The whole owner regression and three
fresh affected gates pass; five unchanged effective checks and other consumer
receipts are explicitly reused. No application source or startup authority changes.

**Selected original settings-policy chain qualification, 2026-11-16 — locally qualified.**
The original CLI listener keeps a private `pending` promise and extends it with
`pending.then(...)` for each coalesced settings change. Its initial resolver join
does not join these later continuations. A native feasibility control captures
the exact source and return, observes the real reserve-policy effect on the
existing AuthStorage and confirms that the second source is the first return.
Select bounded test-only controls in the existing owner regression: unchanged
baseline, serial change/restore, and a two-write coalesced burst followed by restore.
Do not invoke another settings sync or construct another AuthStorage.

Bind the full native sync fingerprint and original continuation callback
73e10fcea65a780ac7077e5857e780e8442086c285b36d9c75c345e4e634602c,
plus its native registration frame. Retain at most two source/return pairs.
Preserve every original receiver, argument, callback and returned promise. Before
each join require registration but no new policy effect; await the exact original
pair and require its completion and actual setter effect. Check serial values
[11,10], burst values [12,10], and source2 === return1. Restore the original value
and runtime-override flag (`hasRuntimeOverride`); broader layer identity is not claimed.
Restore both exact method descriptors at factory
exit and compare the actual setter receiver to the session's existing AuthStorage.
Record only bounded booleans, counts, fingerprints and these synthetic numeric
policy values; never serialize credentials, policy entries or fulfilled values.
Require two equal complete native model readbacks twelve seconds apart, empty
stderr, no prompt and zero connections under the fixed network-denied profile.
The new cases use 45 seconds; other deadlines and assertions remain unchanged.

Association is upstream 24439066040e006eb08da7b204f0ac41c79d2de9,
`session/auth-broker-config.ts::createAuthStorageSettingsSync` (blob
e212c2a0766f5bfc9ce4c843b389431bbc9f7aa8), `config/registry.ts::Derived.listen`
and `AuthStorage::setAccountPolicies`. Relevant upstream
`test/settings-live-routing.test.ts` exercises policy changes with a separate
sync handle. It is not executed here; this consumer observes the CLI's already
registered listener instead. Source association is not reproducible-build proof.
This qualifies the reserve-policy branch only. Broker/store replacement, polling,
refresh/close, complete startup/transport coverage and durable acknowledgment stay
open; no runtime hook or product admission authority is added.

Use the same offline hour, 120-second cleanup reserve, one heavy job, 20 GiB disk
floor and 8 GiB sampled individual RSS ceiling. Run the whole native owner consumer
and three affected gates fresh; reuse five unchanged effective gates and other
consumer receipts only after exact input/body/artifact checks. A materialized
no-await mutation must fail before-effect/after-join assertions. Publish the public
tests/contract/records to experimental, prune only verified new build/extraction
caches, retain the executable/evidence, and replay after cleanup without Cargo.
Paid runs and release signing remain gated; v2/T006 stays execution locked.

The [dated settings-policy receipt](../../../docs/validation.md#original-native-settings-policy-chain--2026-11-16)
qualifies `3ac1e4a` with the original [SDK witness](../../../tests/pilot_capture/settings_chain_witness.mjs)
and native consumer. Original continuations and setter returns are observed on
the existing AuthStorage, with serial/coalesced controls and a failing materialized
no-await mutation. The whole owner regression and three affected checks pass;
five unchanged effective gates and other consumers are explicitly reused.
No application source or startup admission authority changes.

**Selected fixed-profile original auth retirement, 2026-11-16 — implemented and locally qualified.**
Native feasibility retains four distinct original auth listener Sets sharing the
same callback, with exact add receiver/argument/return identities. Removing only
those entries excludes future changes; queued original continuations still run
and must be joined. The session's original AuthStorage closes its actual native
SQLite wrapper synchronously, with a successful metadata-only revision poll
before close and a rejected poll after. Two subsequent complete model readbacks
remain equal. The active control produces policy calls [11,10]; retired has none;
queued-then-retired joins two original calls [10,10], with no later calls.

Select retirement only for the explicit owned fixed-catalog qualification profile.
Reuse the early preload, existing auth helper, bridge and diagnostic trace. Retain
the exact four listener entries, the original initial resolver receiver/return,
and at most eight original pending continuation pairs. Match pinned native frames
and callback fingerprints; preserve valid original arguments, receivers, callbacks
and returned objects. After the existing settings watcher/reload barrier, remove
only those four entries, cross one immediate turn to expose queued callbacks,
await the original pairs, require the pending inventory stable, then close the
session's actual AuthStorage. Require one native SQLite close nested inside that
call, returning synchronously, and the metadata-only closed-wrapper witness.
Any unknown, missing, duplicate, rejected, late or over-cap owner work refuses
before Ready; matching late registrations/continuations after sealing terminate
the qualifier. Never read credential values or construct a replacement sync/store.

Emit bounded registered/retired metadata into the existing diagnostic trace;
validate ordering, shape and counts in its existing native consumer. Capture Ready
requires auth retirement alongside the existing partial owner barriers. This is
not the shared-channel durable startup acknowledgment or complete admission.
The frozen default profile is unchanged. This intentionally excludes unused native
credential reconfiguration in a profile whose actual provider credential/transport
is owned by Rust; it does not remove real coding or usage acceptance.

First run an actual-bridge red control against the current implementation, then
qualify the repaired bridge with all current native conversation/settings/usage
and owner controls and frozen 33/0/7 consumers. Run fmt, pilot all-target Clippy,
pilot unit tests and actual Rust1.90 pilot all-target check fresh. Reuse only four
default-feature gates after exact effective-input and feature-exclusion checks.
Use an offline hour, 120-second cleanup reserve, one heavy job, 20 GiB disk floor
and 8 GiB sampled individual RSS ceiling. Publish reviewed public repair/tests/
records to experimental; prune verified generated caches, preserve the executable
and evidence, and replay current consumers directly after cleanup.

Association remains upstream 24439066040e006eb08da7b204f0ac41c79d2de9,
auth-broker-config.ts::createAuthStorageSettingsSync, registry.ts::Derived.listen,
settings.ts::onEffectiveChange, AuthStorage/LocalAuthStorageBackend::close,
OAuthRefresher::close and SqliteAuthCredentialStore::close/pollExternalChanges.
The settings-live-routing and auth-storage-close-releases-handles upstream tests
are source references, not executed evidence. Unlike their replacement handles or
separate stores, this path retires the CLI's original owner. Source association is
not reproducible-build proof. Logical wrapper closure does not establish physical
file-handle release or general OAuth drain. Complete transport/timer coverage,
original warm-up promise identity and durable acknowledgment stay open; paid runs,
release signing and v2/T006 execution remain gated.

The [dated retirement receipt](../../../docs/validation.md#fixed-profile-original-auth-retirement--2026-11-16)
qualifies `3f972c9` with the actual bridge Ready red/green, original-owner
[controls](../../../tests/pilot_capture/retirement_witness.mjs), all current native
consumers and frozen33/0/7. Four affected checks are fresh; four default-feature
checks reuse unchanged effective inputs. No complete startup admission is granted.

**Source-built caller feasibility, 2026-11-16 — initial call only, no new pin.**
The [native comparison](../../../docs/validation.md#source-built-warm-up-caller-feasibility--2026-11-16)
shows that a source-built SDK can await its original initial warm-up before
session_start without more global promise hooks. Shipped and source-built
baselines enter early under the same held-refusal control; the one-call candidate
waits, with identical complete native model metadata. The current capture consumer
correctly refuses that different artifact digest before launching it. This is
feasibility, not a replacement host, complete repair or current-bridge acceptance.

The next no-spend investigation is one bounded caller owner covering all five SDK
startup warm-up sites and normal/error exits. Preserve original promises and error
semantics; do not substitute a second invocation, quiet timer or pending-count
snapshot. A separately bound profile would need source/patch/lock/runtime/native
addon/binary identity and fresh actual-consumer conversation/usage/owner proof.
Do not relax the existing host pin or carry native callback/frame fingerprints
silently to another build. Current/frozen profiles and execution authority remain
unchanged. Complete transport/timer coverage and durable acknowledgment remain
required, independently of this caller-join feasibility.

**Selected source-built caller-owner evaluation, 2026-11-16 — locally qualified at the SDK caller boundary; not an admitted host.**
Evaluate one per-session SDK owner for all five connection warm-up calls. Keep
their original promises and immediately attach bounded rejection observers; join
both original/observer pairs on every return and error exit. Refuse a sixth or
sealed-owner call before invoking the native warm-up. Keep concurrency with other
startup work. Preserve the body's primary error after all warm-ups settle. If a
completed session cannot be returned because a warm-up failed, dispose it first;
retain both errors if that cleanup also fails. No timeout race grants success.
The existing outer host deadline remains the failure bound for ignored work.

Require source-owner tests for pending normal/error exits, rejection with another
promise pending, deterministic rejection, no-work, cap, late calls and disposal.
The no-await mutation must fail completion assertions. Require the source-built
real SDK under held local refusal on both normal startup and an injected SDK
input-getter failure after its original warm-up starts; compare the immutable
baseline and keep model identity on healthy readbacks. These are source-built
owner controls, not a replacement for the still-required actual capture bridge,
native usage, complete owner/transport inventory or durable acknowledgment.
Keep source/patch/lock/runtime/native/binary pins and raw failures. Current/frozen
host pins stay unchanged. Use an offline hour, a 120-second cleanup reserve, one
heavy job, 20 GiB disk floor and 8 GiB sampled individual RSS ceiling. Publish only
reviewed public patch/test data and records; prune verified new build/extraction
caches afterward, retaining runnable controls and exact evidence. Paid runs and
release signing remain gated.

The [caller-owner receipt](../../../docs/validation.md#source-built-bounded-startup-caller-owner--2026-11-16)
records the five-site source change, eleven owner tests, failing no-join mutation,
coding-agent type/lint/format checks and four native normal/error controls.
The [upstream patch](../../../docs/references/omp-startup-connection-warmups.patch)
contains the proposed helper and durable source-owner tests, with its original
license; Foundry does not apply it or select that binary as a capture pin. Native
controls cover the initial normal/error paths; other model-selection branches are
not separately live-exercised. The separate capture profile and its real consumers,
complete owner/transport inventory and durable acknowledgment remain required.

**Selected source-built capture profile, 2026-11-16 — qualified diagnostic; complete admission remains HOLD.**
Add an explicit diagnostic catalog profile with its own exact artifact pin; current
OwnedQualification and frozen HostDefault keep their pins and behavior. Bind the
source/patch/lock/Bun/native-addon/binary identities in the receipt. This profile
cannot grant Live/v2 execution or paid authority.

The source CLI retains its actual auth-settings sync through its existing owning
module. A one-process CLI owner exposes a one-use binding to the actual Settings
instance and retires only the matching AuthStorage. Extend that original sync with
retirement that stops its real subscriptions, crosses the known queued-listener
turn and joins the original initial resolver, catch return and current pending
chain. Require chain stability and successful originals; reject duplicate/missing,
wrong-identity, rejected and changing work. Do not replay another sync, read
credential values or retain unbounded change history. Preserve the existing stop/
settled API. Already queued callbacks may finish; callbacks after retirement refuse
before reconfiguration. General OAuth/broker-drain and physical FD release remain
outside this fixed empty-auth profile.

The existing prefetch helper selects this source binding for the new profile and
keeps its original-call, namespace and nested-store-close checks. It reuses the
existing diagnostic trace for source-auth bound/retired metadata, without claiming
an observed four-Set count. The preload skips frame-dependent Promise/Set hooks
only for this source profile; its transport/refusal, logging and native-deadline
checks remain. The source SDK's already-qualified caller owns the warm-up join.
Settings, off-memory, telemetry, marketplace, model/tool/usage and final Ready
checks remain required. No extra worker, channel, journal or generic tracker is
introduced. The proposed shared-channel durable startup acknowledgment remains
separate and unimplemented.

First require the actual source-profile capture consumer to fail before prompting
when the native auth binding is absent. Keep that artifact/red result. Then require
the original-sync retirement source tests, native identity/queued-policy controls,
the full two-generation actual capture conversation, unchanged full model and
usage/mutation checks, current-artifact consumers and frozen compatibility. Run
affected Rust formatting/pilot Clippy/unit/MSRV gates; reuse default-feature gates
only on exact effective inputs and explicit feature exclusion. Use the stated
no-provider hour, cleanup reserve and resource bounds; publish reviewed changes,
then prune verified new caches and replay retained consumers. No missing proof
becomes a permissive default. Paid runs, release signing, full startup admission
and v2/T006 execution remain gated.

The [Rust launch owner](../../../src/pilot_capture/host.rs) now implements the
explicit source profile and unchanged current/frozen pins. The proposed upstream
[auth retirement patch/tests](../../../docs/references/omp-startup-auth-retirement.patch)
retain the original owner; Foundry does not apply that patch. The
[dated receipt](../../../docs/validation.md#source-built-capture-profile-qualification--2026-11-16)
records actual source conversation/usage/identity/retirement controls, compatibility,
fresh versus reused gates and the pre-launch resource pause. This qualifies the
fixed source diagnostic only. Transport/timer completeness and durable startup
acknowledgment below remain unimplemented admission conditions.

**Selected journal-readback prerequisite, 2026-11-16 — qualified at seal/finish; startup acknowledgment still unimplemented.**
Before implementing the durable startup acknowledgment, strengthen the existing
Rust `Journal::digest`, already consumed by probe seals and final receipts. Bind
readback to the writer's same device/inode and verify complete bounded JSONL,
sequence, previous-record hashes, record count and the writer's last committed
record digest. Stream the hash using the existing record bound; add no database,
log or new replay authority. A valid unchanged journal retains its exact digest.
On probe-seal append/readback failure, enter the existing monotonic halt and keep
`probe_sealed` false; no successful receipt or later request may follow. Preserve
consumed claims and return halt-persistence failures if that append also fails.

Require red/green file corruption, truncation, appended-data and replacement
controls, including the actual final-receipt consumer and frozen-host probe seal.
Then run current/frozen consumers, the source conversation and affected Rust
checks. Reuse default-feature gates only on exact effective inputs. This work uses
the transport investigation's unchanged one-hour deadline and resource bounds.
The generic transport fence is not selected: actual named DNS emits a loopback
packet outside its hooks, and early loader registration does not intercept that
builtin import. Source-bound transport ownership/inventory and the shared-channel
closed revision remain required; no full startup admission, paid or signing
authority is granted by this readback repair.

The [landed Rust journal/seal owner](../../../src/pilot_capture.rs) and
[actual probe consumer](../../../tests/pilot_capture.rs) now implement/qualify
this prerequisite. The [dated receipt](../../../docs/validation.md#journal-attestation-readback-and-transport-census--2026-11-16)
records native red/green, fresh versus reused checks and the named DNS/loader
counterexamples. The graph is a source census, not complete runtime coverage or
reproducible-build proof. No generic-fence adoption or v2 readiness follows.

**Selected shared-channel implementation, 2026-11-16 — channel slice qualified; exchange open.**
The [shared-owner qualification](../../../docs/validation.md#shared-early-capture-channel-qualification--2026-11-16)
accepts the extraction at its native framing and v1 consumer boundaries. It does
not implement startup messages, before-dispatch refusal or durable acknowledgment,
and grants no startup admission. The remaining exchange below is still required.
Implement the exchange below through the existing Rust host/session/journal and
one child channel object. The early preload and later bridge share its pipe
identities, decoder, sequence numbers, close state and writes; neither can create
a second connection or reset it. Extract transport ownership from the current
bridge without changing v1 messages or native request/usage behavior. Bind the
complete materialized bridge source, including that owner, to existing source
provenance; keep private-mode and exact-source checks.

The closed revision must reject ordinary Ready/capture frames until the required
startup prefix is durable and a matching acknowledgment is echoed. Wire/state
implementation or scripted positives cannot freeze an incomplete source/owner
inventory. Missing qualification stays a named refusal before counters, permits
or transmission. Couple the source warm-up owner to parent refusal before its
transport/signal construction; do not revive the rejected blanket API fence.
All current/frozen pins and authority remain unchanged until actual-consumer
qualification selects a new source candidate. Work is bounded to one no-spend
90-minute round, one heavy job, the existing 20 GiB/8 GiB resource limits and a
120-second cleanup reserve. Retain red/green evidence and keep unfinished work
explicitly unqualified; paid trials and release signing remain gated.

**Known-owner refusal integration qualified, 2026-11-16; full acknowledgment open.**
The [native refusal receipt](../../../docs/validation.md#source-owned-durable-startup-refusal--2026-11-16)
qualifies the source-owned before-dispatch seam and durable refusal at their
specified diagnostic boundaries. This startup-only revision cannot grant Ready.
Continue inside the same
90-minute bound above. The source warm-up function consults an immutable early
no-grant observer before constructing its fetch transport or abort signal. Even
a resolved observer cannot permit IO. Closed revision 2 metadata carries a bounded
attempt, owner, method, scheme, hostname, port and route; the existing Rust journal
must append, sync and read back its refusal before the child is answered. Use at
most 20 attempts, existing 64 KiB headers, zero payload and a 30-second startup
observation wall. Only the exact Anthropic HEAD origin is classified in this
initial owner integration. Other destinations/owners fail visibly. A warm-up-only
`owner_settled` message is not complete `startup_settled`: without the full frozen
inventory the parent records `startup_inventory_unqualified` and closes without
Ready, counters, permits, preview or generation. No v1 fallback is possible in
this explicit diagnostic. This integration is a prerequisite to the full prefix/
acknowledgment exchange below, whose inventory and execution gates remain intact.

**Inventory completion round, 2026-11-16 — restricted-owner slice implemented; complete inventory remains open.** A new no-spend
90-minute round starts after the refusal round ends, with one heavy job, the same
20 GiB disk floor / 8 GiB sampled individual RSS ceiling and 120-second cleanup
reserve. Complete the source-bound owner inventory before allowing its prefix
acknowledgment. The first traced gap is CLI tool selection: expose the SDK's
existing `restrictToolNames` as explicit `--restrict-tools` requiring `--tools`,
and pass it only to the newly pinned owned startup qualifier. The existing SDK
guards then exclude MCP inheritance/discovery and memory startup/controllers;
ordinary tool selection retains its current meaning. A one-use source API binds
its actual resulting session manager, model registry, active tools, disabled MCP
state and `memoryEnabled` to an immutable receipt. Seal prevents another SDK
session from reopening those startup owners. Wrong/ambiguous/repeated bindings
refuse. The child reports those facts over the same closed channel; Rust must
validate and journal them. Qualify positive unrestricted controls, restricted
controls and mutation refusal at the real CLI boundary. This report alone cannot
stand in for catalog/auth/settings/telemetry/update/extension owner evidence.
The same source inspection finds one unowned credential-usage prefetch in SDK
startup. Extend the existing bounded caller owner to join that original promise
and its logging observer for restricted sessions, including primary-error exits.
Keep ordinary unbound prefetch behavior. Failed/unjoined preparation cannot seal
the restricted receipt. A second/concurrent SDK creation makes the single-session
binding ambiguous; sealing must refuse and block later starts. Keep identity
references weak so ordinary SDK use cannot retain a disposed session.
Missing inventory still records HOLD; no paid, signing or v2 execution gate changes.
Evidence and qualification limits live in the
[current handoff](../../../docs/resume-context-ecosystem.md), with dated receipts in
[validation](../../../docs/validation.md#restricted-startup-ownership--2026-11-16).
The catalog/discovery waiter is not a complete lifecycle join. Include the awaited
daemon project-presence registration and its exit cleanup in the remaining owner
inventory; the initial categories above are not an exhaustive list.

**Catalog lifecycle round, 2026-11-16 — original-operation retirement qualified in the fixed startup diagnostic; full inventory remains open.** After the
restricted-owner round's terminal publication/cache receipt, start one new no-spend
90-minute round with the same one-heavy-job, 20 GiB/8 GiB and 120-second cleanup
bounds. Trace the original registry discovery operations, including nested runtime
provider callbacks and their timeout timers. `withModelDiscoveryTimeout` races the
operation without cancelling it; joining its returned race is not original-operation
settlement. Qualify that counterexample with pinned native runtime/owned IO before
repairing the concrete source owner. Retain ordinary timeout behavior. A one-use
catalog retirement must prevent new discovery before work starts, join original
operations and completion observers on success/error/timeout, retire timers and
callbacks, and refuse unknown/mirrored/ambiguous/repeated ownership. Positive and
held-original/ignored-cancellation/late-start/mutation controls must precede any
catalog settlement report. Do not replace this with an empty catalog, static-only
registration, a pending-count snapshot, or changed model identity. Use one opt-in registry owner established at construction; keep the original
operation and its completion observer, including raw work after a timeout race,
response bodies and shared catalog requests after a subscriber stops waiting.
Evidence and remaining acceptance are owned by the
[current handoff](../../../docs/resume-context-ecosystem.md); the dated
[retirement receipt](../../../docs/validation.md#source-owned-startup-discovery-retirement--2026-11-16)
separates native original-operation proof from the still-open complete inventory.
Bound ownership to 4,096 starts. Existing fetch-identity-scoped catalog sessions
carry that same owner; this is source callback ownership, not a global transport
fence. Close public discovery admission before draining. Reject mirrors and
unqualified Cursor native HTTP/2 discovery; the fixed profile retains its existing
disabled-provider/model identities. A retirement receipt reports joined operations
and bounded rejection counts, not successful provider requests or physical FD
shutdown. Bind it to the already sealed actual restricted session/registry.
Rust validates and journals this metadata after restrictions and before its
incomplete-inventory halt; a late attempt after warm-up settlement is a protocol
fault. Persist-failure controls include the new catalog-append boundary.
The complete
startup inventory and prefix acknowledgment/Ready echo remain required; a catalog
receipt alone cannot grant admission. Paid execution and release signing stay gated.

**Selected settings lifecycle round, 2026-11-16 — qualified at the fixed startup boundary.** After the catalog round's
terminal receipt, use one fresh 90-minute no-spend round, one heavy job, a 20 GiB
disk floor, 8 GiB sampled individual RSS ceiling and 120-second cleanup reserve.
The concrete gap is queued settings reload/rescope work surviving timer cancellation.
Own original persistence/reload operations and their completion observers from
construction in the restricted CLI profile; stop watching and close future admission
before joining them. Preserve ordinary settings behavior outside that opt-in profile.
Bind retirement to the exact already-sealed session, registry and settings instance.
Qualify held originals, queued work, errors and late starts at the native boundary;
an empty pending-count snapshot or quiet interval cannot establish settlement.
Keep auth-store lifetime, other startup owners and the durable prefix acknowledgment
open until their own original-work evidence and integration pass. This selected
repair cannot deliver Ready or authorize paid execution or release signing.
The [dated receipt](../../../docs/validation.md#source-owned-settings-retirement--2026-11-16)
owns actual CLI, mutation, gate, resource and cleanup evidence. The incremental
[upstream patch](../../../docs/references/omp-startup-settings-retirement.patch)
retains source/module/test identity and intentional differences.

**Selected fixed empty-auth lifecycle round, 2026-11-16 — qualified at the fixed startup boundary.** After the settings
round's terminal receipt, use a fresh 90-minute no-spend round with one heavy job,
20 GiB free-disk floor, 8 GiB sampled individual RSS ceiling and 120-second cleanup
reserve. Ordinary close may return before OAuth refresh completes; it cannot
attest retirement. General OAuth/broker drain remains outside the selected empty
pilot profile. Enforce that profile from construction through a separate explicit
CLI option requiring restricted startup, preserving ordinary auth behavior.
Keep the actual SQLite credential store and original supported startup calls;
refuse stored credentials, brokers, foreign backends and unsupported credential/
network work before admission. Own original admitted promises and completion
observers, including callers retaining a namespace reference. Close admission,
join those originals and close the original store before reporting retirement.
Preserve bounded metadata reads required by the native model/session interface.
Use a small auth-specific owner; a general promise hook or background drain loop
would add broader coordination without enforcing the selected credential boundary.
Qualify real native initialization/model identity, held originals, failures,
stored-credential/broker refusal, original-operation identity, late calls and
counterfactual controls. A quiet trace or empty pending snapshot is insufficient.
Bind the receipt to the exact sealed SDK session/registry/auth instance and carry
it through the existing channel/journal with append-failure checks. No general
OAuth, physical FD-release or complete startup admission claim follows. Other
owners, complete inventory and durable prefix acknowledgment remain required;
paid execution and release signing stay gated. The
[dated receipt](../../../docs/validation.md#fixed-empty-auth-startup-retirement--2026-11-16)
owns native/source/gate/retention evidence; the
[upstream patch](../../../docs/references/omp-startup-empty-auth.patch) retains
source/module/test identity and intentional differences.

**Selected daemon-presence lifecycle round, 2026-11-16 — qualified at the fixed startup boundary on 2026-11-18.** After the fixed empty-auth
round's terminal receipt, use one fresh 90-minute no-spend round with one heavy
job, a 20 GiB disk floor, an 8 GiB sampled individual RSS ceiling and a 120-second
cleanup reserve. The actual CLI awaits original project-presence registration but
discards its returned handle; ordinary concurrent close can return before the
original file removal finishes. Retain the original registration, real private
presence file and exit cleanup. Give only the explicit restricted startup profile
one source-owned presence lifetime from before registration. Close registration
admission, join the original promise and its completion observer, and join the
original idempotent cleanup before attesting retirement. Freeze the owned returned
handle before exposure so callers cannot replace its original cleanup; ordinary
unowned handles retain their existing API. A mutation attempt must not obtain
retirement without the original real removal. Cached registration and
close references must obey the same owner; failed registration/cleanup, ambiguous
or repeated bindings cannot produce a successful receipt. Preserve the ordinary
CLI's registration behavior. Use the existing source module, SDK binding, channel
and journal rather than another worker, registry or background loop.

Qualify held registration, concurrent/held removal, ordinary positive behavior,
registration/cleanup failures, late-start and wrong-owner controls at the native
CLI boundary, with parent file/normal-exit observations and counterfactual tests.
Carry bounded metadata after auth retirement and before incomplete-inventory HOLD;
include its journal append-failure boundary. Full startup inventory, telemetry/
update/logging and trusted-extension ownership plus durable prefix acknowledgment/
Ready echo remain required. No paid, signing or v2/T006 execution gate changes.
The [dated qualification](../../../docs/validation.md#daemon-presence-immutable-handle-qualification--2026-11-18)
records the corrected source, native controls, fresh gates and remaining admission limits.
The incremental [source patch](../../../docs/references/omp-startup-presence-retirement.patch)
retains the original registration/cleanup, upstream identity and focused tests.

**Selected telemetry exclusion round, 2026-11-18 — qualified at the fixed startup boundary.** After the presence round's
terminal receipt, use a fresh 90-minute no-spend round with one heavy job, a 20 GiB
disk floor, an 8 GiB sampled individual RSS ceiling and a 120-second cleanup
reserve. The source's disabled initialization permits a later enabled call to
register exporters; the bounded network-denied baseline observes that behavior.
Only the explicit restricted diagnostic uses `--startup-no-telemetry`. Establish
one source owner before initialization; retain the original disabled call and its
completion observer, refuse enabled/repeated/late calls before exporter construction,
and join the originals before reporting retirement. Keep ordinary telemetry outside
this option. Bind the exact owner to the sealed session/registry after presence
retirement; validate closed metadata and journal it before incomplete-inventory HOLD.
Qualify ordinary and owned controls, configured endpoint, enabled-setting refusal,
wrong identity, delayed observer, late references and persistence failure at the
actual boundaries. The [upstream reference patch](../../../docs/references/omp-startup-telemetry-exclusion.patch)
records source identity and differences. The [dated qualification](../../../docs/validation.md#source-owned-telemetry-exclusion--2026-11-18) records native controls, gates and limits. Direct OTLP imports, general exporter drain,
other startup owners, complete inventory and prefix acknowledgment/Ready echo remain
open. Paid execution, signing and v2/T006 remain gated.

**Selected marketplace exclusion round, 2026-11-18.** A 90-minute no-provider
round with one heavy job, a 20 GiB disk floor, an 8 GiB sampled per-process RSS
ceiling and cleanup reserve covers the fixed startup profile. The source baseline
showed that a later enabled call after an off call starts detached marketplace
refresh work. The opt-in restricted profile closes that entry point, requires a
frozen own data property set to off and invokes the original synchronous off
branch; exact-session retirement follows telemetry retirement. The Rust journal
records the closed result. Native CLI controls exercise ordinary off, owned off,
enabled refusal and invalid flags; source tests and counterfactuals cover cached
calls and deceptive mutable options. See the [source reference patch](../../../docs/references/omp-startup-marketplace-exclusion.patch).
Other update routes, logging, trusted-extension ownership, the full before-I/O
inventory and durable prefix acknowledgment with matching Ready echo remain open.
This does not admit startup or authorize paid runs, signing or release.

**Selected startup logger ownership round, 2026-11-18.** One 90-minute
no-provider round, one heavy job, a 20 GiB disk floor, an 8 GiB sampled
per-process RSS ceiling and a 120-second cleanup reserve qualifies the explicit
restricted startup profile. The original logger can reopen after a disable; the
source-owned profile closes the real local transport and prevents subsequent
transport, sink and forwarder reconfiguration. It retains and joins the original
queued retention callbacks, temporarily referencing their unreferenced handles
while waiting. Exact-session retirement follows marketplace retirement and is
recorded by the existing journal. Native CLI, buffered-marker, held-callback,
wrong-identity and guard counterfactual controls qualify this selected module;
ordinary logging remains available outside the flag. See the [source patch](../../../docs/references/omp-startup-logging-owner.patch).
This does not qualify arbitrary other logging implementations, trusted-extension
work, plugin re-warm, complete inventory or durable acknowledgment/Ready echo.
Startup admission, paid execution, signing and release remain gated.

**Selected startup plugin-roots ownership round, 2026-11-18.** One 90-minute
no-provider round, one heavy job, a 20 GiB disk floor, an 8 GiB sampled individual
RSS ceiling and a 120-second cleanup reserve covers the restricted startup profile.
A network-denied baseline held the real filesystem work and showed cache clearing
returns before its re-warm completes. The source owner tracks original plugin-root
reads, preloads, injected roots, cache clears and their observers; it drains bounded
work before atomically closing entry points. Synchronous invalidator errors or
reentrant retirement fail, and late reads and clears refuse before cache mutation.
The exact owner retires after logging and is journaled before incomplete-inventory
HOLD. Source and real CLI held-filesystem, wrong-identity, failure and counterfactual
controls qualify this cache owner, while ordinary behavior remains available without
the flag. See the [source patch](../../../docs/references/omp-startup-plugin-roots-owner.patch).
Other discovery and extension work, complete inventory and durable acknowledgment/
Ready echo remain open. Paid execution, signing and release remain gated.

**Extension handler timeout boundary, 2026-11-18.** A network-denied native
control on the pinned host held a synthetic `session_start` handler past its
reported timeout; the RPC state response arrived while the original handler was
still held. The original completed after release. Ordinary RPC Ready precedes
extension initialization and is not startup admission. Before any full startup
settlement or matching Ready echo, own the actual handler and callback lifetime:
prove its original completion outside the handler itself, refuse timeout, failure
and ignored-abort paths, and prevent late work from being mistaken for a settled
owner. The existing incomplete-inventory HOLD remains in force. See the
[dated native receipt](../../../docs/validation.md#native-startup-extension-timeout-boundary--2026-11-18).

**Selected startup extension dispatch ownership round, 2026-11-18.** One
90-minute no-provider round, one heavy job, a 20 GiB disk floor, an 8 GiB sampled
individual RSS ceiling and a 120-second cleanup reserve covers the restricted
startup profile. The source owner binds the exact session runner, retains the
original `session_start` work and observers, and validates failures and timeouts
before issuing a one-use receipt after the handler broadcast. The existing channel
verifies the exact receipt and Rust journals it after plugin-root retirement.
Repeated starts and extension additions refuse. Source counterfactuals, wrong
identity, failure, timeout and native held-handler controls qualify this selected
dispatch lifetime; ordinary behavior remains available without the flag. See the
[source patch](../../../docs/references/omp-startup-extension-owner.patch).
Arbitrary work spawned by extension code, other events, complete startup inventory
and durable acknowledgment/Ready echo remain open. Paid runs, signing and release
remain gated.

**Selected managed extension timer exclusion, 2026-11-18.** A native baseline
on the pinned host observed a sanctioned `ctx.setTimeout` callback after the
extension-dispatch receipt. In one 90-minute no-provider round, with one heavy job,
a 20 GiB disk floor, an 8 GiB sampled individual RSS ceiling and a 120-second
cleanup reserve, the existing restricted extension owner closes managed timer
admission before a timer handle can be constructed. Earlier managed timer starts
refuse owner installation, even after clearing; direct and cached timeout/interval
calls refuse. The identity-bound extension receipt includes this closed fact and
Rust validates it. Ordinary managed timers remain available without the owner.
Source counterfactuals and real-CLI ordinary/owned controls cover this explicit
profile; see the [source patch](../../../docs/references/omp-startup-managed-timer-exclusion.patch).
Raw timers, arbitrary extension work, other events, complete inventory and durable
acknowledgment/Ready echo remain open. Paid runs, signing and release remain gated.

**Selected restricted workspace-scan exclusion, 2026-11-18.** In the diagnostic
profile with the exact restricted extension startup owner, the source host refuses
workspace-tree scan construction at the original SDK entry point, including a later
settings-triggered scan. A supplied tree does not start that scan. Other profiles
retain ordinary scanning. The source host is represented by the
[reference patch](../../../docs/references/omp-startup-workspace-scan-exclusion.patch),
which Foundry does not automatically apply. Its scope is this scan entry point;
other filesystem discoveries, native operations and arbitrary extension work are
not covered. Complete inventory, durable acknowledgment and matching Ready echo
remain open, so no startup admission or paid execution is enabled.

**Ordered refusal receipt repair, 2026-11-18.** The pinned runtime reproduced
acceptance of duplicate or decreasing journal sequences, repeated digests and
out-of-order attempt receipts. The existing child channel now requires the next
pending attempt, a strictly increasing safe journal sequence and a well-formed prefix digest not seen earlier in this launch before resolving each refusal. Gaps in journal sequence
remain valid because other records can intervene. Native framed-channel controls
cover five negative cases and a valid gap. These child checks do not prove a
disk write: the parent still appends, syncs and reads back the journal before each
response. Complete owner inventory and the later durable acknowledgment/Ready echo
remain open; incomplete inventory still halts and cannot grant generation.

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

**RPC startup boundary selected 2026-11-18.** Keep the exact first-party bridge
as the only trusted module in the restricted launch. In the prior host, the
native CLI published its first command catalog after extension retirement and
emitted raw RPC Ready earlier. The source-built observation mode now awaits
the original command catalog and drains its stdout writer. The mode excludes the
metadata subscription and RPC dispatch, and emits its one-use source receipt
after the extension receipt on the same private channel. The parent persists
that receipt and still halts for the incomplete inventory. Ordinary RPC keeps
its initialization and subscription. See the [source reference](../../../docs/references/omp-startup-rpc-observation.patch)
and [native receipt](../../../docs/validation.md#restricted-startup-rpc-retirement--2026-11-18).

The exact first-party bridge's original graph read and the loader's awaited
factory edge now have [native hold and failure controls](../../../docs/validation.md#first-party-startup-extension-loader-controls--2026-11-18).
The selected observer now obtains the startup owner functions from an explicit
SDK export through the existing compiled module registry. The
[native controls](../../../docs/validation.md#direct-sdk-import-for-startup-observation--2026-11-18)
cover the direct import, the legacy root import, and the ordinary RPC path.
The admitting profile still needs qualification for enabled work started by
transitive loads, the runtime subscription and command handoff, and the rest of
the startup inventory.

**Catalog and Spotlight startup boundaries, 2026-11-18.** Use the existing
registry-owned discovery fetch for the classified shared catalog GET refusal.
The diagnostic now handles this before transport through the same private
channel and journal as the connection warm-up. Each owner's retirement closes
its own attempt window. The explicit restricted startup SDK closes Spotlight
eligibility before constructing its client. Ordinary opt-in remains available.
The [native qualification](../../../docs/validation.md#catalog-refusal-and-spotlight-startup-exclusion--2026-11-18)
covers a held catalog refusal, a malformed reply, an unclassified and late
attempt, missing observation and ordinary RPC. Admission still requires the
complete inventory and durable acknowledgment below.

The fixed diagnostic starts the pinned Anthropic-only discovery owner in a
fresh process and private profile. Its parent accepts catalog retirement only
after the classified catalog attempt has been refused and synced. The warm-up
refusal cannot satisfy this requirement. Missing evidence records
`startup_catalog_refusal_missing` before any catalog retirement or partial
inventory attestation. A profile that can skip the catalog attempt needs
explicit evidence of that disposition before using the admitting protocol.
See the [native omission control](../../../docs/validation.md#catalog-refusal-binding-in-the-startup-diagnostic--2026-11-19).

The current startup-only parent holds the RPC input pipe open for teardown and
has no command writer. In the future admitting revision, keep RPC command delivery
closed until the same private channel returns the exact acknowledgment digest in
Ready and the ordinary settings and model checks pass. Settled startup owners,
the parent's synced prefix readback, and the matching child echo are separate
requirements. See the [native RPC evidence](../../../docs/validation.md#startup-rpc-command-boundary--2026-11-18).
The original session event callback stayed live after the RPC retirement
receipt in the [baseline native control](../../../docs/validation.md#rpc-session-callback-after-startup-retirement--2026-11-18).
The qualified observation profile now excludes session and subagent event
subscriptions and goal reconciliation. Its receipt binds that exclusion,
and its persistence owner continues reporting teardown failures. See the
[native qualification](../../../docs/validation.md#rpc-runtime-callbacks-excluded-during-startup-observation--2026-11-18).

Use the ordered receipt prefix in [`startup.rs`](../../../src/pilot_capture/startup.rs)
as the starting inventory for the existing diagnostic. It contains these selected
facts and leaves these boundaries to qualify before the inventory can be frozen.

| Startup area | Current diagnostic evidence | Remaining scope for the admitting profile |
| --- | --- | --- |
| Connection preparation | The original warm-up caller settles after the parent durably refuses the classified Anthropic HEAD route. A source-read route table assigns each startup transport class a disposition; its completeness is not established. Before any host code, a process whose executable's directory is unreadable, as it is for every pilot Bun launch, connects to libSystem's logging socket `/var/run/syslog` (inferred from launch-path controls and static reading); a Bun copy in a readable directory made no such connect, and a test-only library-call witness recorded the connect returning −1 `EPERM`. The profile still denies it. The startup-observation profile now ends the host on any other sandbox `network*` denial and keeps that one denial non-fatal; twelve classes were shown killed on bare fixtures with the same suffix (inferred for the host); in paired runs with and without that exception, normal startup survives only with it, so every sandbox `network*` operation of normal startup matched it (inferred; operations matching the same rule cannot be told apart). In the Python reconstruction of the exact host and bridge, a spliced raw `Bun.connect`, which reached HOLD under the previous profile, is killed under this one. Through the actual launcher, a `SIGUSR1` ends the host with `startup_host_exit` `signal: 9`, `forced: false`; that the killed call is the inspector bind is inferred from a reconstruction witness run ([record](../../../docs/validation.md#startup-early-connect-and-fatal-network-denial--2026-11-21)). | Establish before-I/O ownership for each enabled route and fail closed on unclassified transports. Remaining: Mach and XPC routes to system daemons; name resolution, whose AF_SYSTEM and `mDNSResponder` connects are inferred to end the startup host now; binding the gate-only exclusions (automatic Codex WebSocket prewarm, `SIGUSR1` inspector, Bun crash upload) for the admitting profile; whether the admitting profile keeps the fatal rule; an owner decision on the implementer interpretation that the kernel tripwire is not the rejected API fence (the owner's choice of the round is not that decision). A kill is not journal completeness: it names no destination. |
| Tools, MCP, memory and credentials | The sealed identity reports fixed `edit` and `read`, MCP and memory exclusions, and joined credential-usage work. The extension receipt binds `notifications_excluded`. A runner bound to the startup owner refuses `credential_disabled` and `mcp_notification` before any handler runs. Native controls show this for an event emitted after the bind. Upstream tests show two more cases: events buffered before the bind are dropped at `initialize` without dispatch, and `session_start` fails closed while any stay buffered. Ordinary RPC still delivers both events ([record](../../../docs/validation.md#deferred-startup-notifications-in-the-extension-runner--2026-11-20)). | Bind the final task tools and any admitted MCP integration to actual before-I/O controls. The refusal lasts as long as the bound runner, not only until startup retires. The admitting profile must either keep it or end it at startup retirement under its own receipt, and must decide whether refused notifications may be dropped. No native run yet covers an event buffered before the bind. Keep `memory.backend` off and auto-learn disabled in every arm (owner decision, 2026-11-19, "Off in every arm"); other memory backends and auto-learn execution are out of scope. |
| Catalog, settings and authentication | Source owner receipts cover selected discovery, settings callbacks and the empty authentication profile. The catalog GET is classified and refused before transport. The fixed registry closes runtime registration and binds Anthropic-only discovery eligibility. Native launch and late-change controls verify that the fixed set cannot expand. | Qualify the remaining enabled catalog transport and callbacks in the final profile. Keep the implicit `apple` provider disabled in every arm (owner decision, 2026-11-20, "Disable in every arm"). |
| Presence, telemetry, marketplace and logging | The selected owners close or exclude their own work and record the original joins. | Review enabled update and output paths outside these selected owners. Named limitation with no work planned ([owner decision, 2026-11-19](../../../docs/validation.md#owner-decisions--2026-11-19)): an ordinary non-pilot host RPC exit leaves its presence file ([record](../../../docs/validation.md#original-daemon-presence-candidate--2026-11-16)), and daemon-server shutdown is unexamined. Both are upstream host questions, out of scope for Foundry. Re-entry trigger: a pilot profile that depends on ordinary RPC exit cleanup or a daemon server, or a leftover presence file observed to affect a Foundry run. |
| Plugin roots and first-party extension | The diagnostic reports plugin-root retirement and the original extension dispatch join. It excludes managed timers and the selected workspace scan. Native controls cover the exact bridge graph read and the loader's awaited factory. | Check work started by enabled transitive loads, including native timers, discovery and callbacks. |
| Advisors | The candidate excludes advisor discovery, tool construction, restore and activation in the explicit profile. The existing restricted-session receipt records the exclusion. Native controls exercised ordinary RPC and a late activation and restore attempt. | Preserve the disposition in the final admitting profile. |
| Prompt and command inputs | Source guards close implicit prompt, template, command and active-repo discovery at the selected owners. The restricted prompt skips optional URI scheme descriptions before invoking their owners. Native controls cover input exclusion, ordinary RPC and active-child projection. The prompt builder rejects a preparation timeout in this profile. | Bind the same decisions to the final admitting profile and retain evidence for any enabled prompt or transitive discovery owner. |
| Debugger and process timers | The debugger sweep starts at first session registration. The startup control stayed free of the sweep beyond the baseline tick, and ordinary RPC started it with a fixture-backed session. A static census of the patched source found 1,933 timer-API matches, 718 in runtime code at 713 sites; their reachability classes are inferred from paths and flags, and 84 entries stay unknown (79 timer-API matches and 5 `node:timers/promises` imports), none of them observed. A test-only timer preload on the exact host and bridge, under the startup policy, recorded every wrapped form in bare Bun, `Bun.sleep` included; the bundled call path was not shown for `Bun.sleep`, `setImmediate` and `scheduler.yield`. In six shown runs, all 45 timers created before RPC retirement were fired or cleared by it, none was pending at retirement, and none was created or fired in the retirement window or a 40-second hold; teardown creates four shutdown deadlines through `withTimeout`, all cleared, and the 10-second `postmortem.ts:290` deadline, abandoned at exit, which came at most 17.3 ms after close; with the refusal pending the watchdog interval is also abandoned at exit. Only the wrapped JavaScript entry points are seen, so zero is not absence ([record](../../../docs/validation.md#startup-timer-census--2026-11-23)). | Bind the settled set to the admitting profile and its handoff, and qualify timers created after the handoff. Remaining: the 84 unknown entries; Bun, JavaScriptCore and addon-internal timers; option-driven timeouts; workers and subprocesses. |
| Native addon and process resources | Phased kernel snapshots place three same-process Unix descriptors at the original addon load. A test-only witness records one Unix stream socket-pair call from that image. A Tokio component fixture reproduces the pattern with its signal driver. On the pinned host, a library-call witness recorded only that call from the addon image during normal startup (recorded calls only), and a bare fixture's addon IPC `bind` was denied by the kernel ([record](../../../docs/validation.md#startup-transport-witness--2026-11-20)). The startup launcher binds the process-fork denial and the fatal network denial into its policy hash; the addon load survives the fatal denial with its syslog carve-out ([record](../../../docs/validation.md#startup-early-connect-and-fatal-network-denial--2026-11-21)). | Qualify remaining transient network and IPC activity and any process capability required by the final runtime. |
| RPC initialization and parent admission | Startup observation joins the initial catalog and its stdout writes, excludes runtime callbacks and RPC dispatch, and persists the source-bound receipt. Its parent admits one command catalog envelope on stdout and checks both streams through EOF. The observation parent has no RPC command writer. | Qualify the runtime subscription and dispatch handoff for the admitting profile. Use the complete inventory for the synced acknowledgment and matching Ready echo below. |

**Selected next work, 2026-11-18.** Keep one inventory for the exact restricted
profile. The advisor candidate excludes discovery, tools, cost restore and
activation at their source owners. Ordinary RPC exercises the original paths
in [native controls](../../../docs/validation.md#advisor-exclusion-during-startup-observation--2026-11-18).
Bind the compiled and native artifacts and ordered source patches into the host
identity that the launch manifest pins (owner decision, 2026-11-19, "Digest plus
pinned inputs", under the capture owner above). The
observation journal binds the bridge, effective arguments, settings, sanitized
environment and sandbox policy. Each reachable owner needs a source anchor, an eligibility
decision, a side-effect boundary and evidence of exclusion before construction,
refusal before I/O, or original work and callbacks joined with late starts closed.
Account for the transports reachable through admitted transitive code. Keep
the selected debugger, registry, prompt and command dispositions
bound to the same profile. The diagnostic receipts cover a
partial inventory. An unresolved edge keeps admission on HOLD.

The startup diagnostic fixes model discovery eligibility at the registry. In upstream
`24439066040e006eb08da7b204f0ac41c79d2de9`,
`ModelRegistry.#addImplicitDiscoverableProviders` owns automatic local entries.
`#collectBuiltInModelManagerOptions` separately selects standard descriptors,
special credential-dependent descriptors and catalog-only providers.
`#discoverWithModelManagerStartupBody` constructs the selected manager and owns
its refresh deadline. The restricted launch currently uses an Anthropic
configuration with empty authentication and a disabled-provider list.
That list already names the implicit `apple` provider, which owner decision 19 of
2026-11-20 keeps disabled in every v2 launch (item 5).
The [baseline eligibility controls](../../../docs/validation.md#built-in-model-manager-eligibility-during-startup--2026-11-18)
show that a provider's public discovery status can stay idle when its manager
runs. The [fixed-profile controls](../../../docs/validation.md#fixed-discovery-provider-inventory-at-startup--2026-11-19)
verify the effective set at the registry for this exact diagnostic. The
catalog receipt requires `discovery_providers_fixed`. Preserve these
eligibility decisions in the admitting profile and qualify its remaining
transitive transport and callback ownership.

The [baseline lookup control](../../../docs/validation.md#remote-url-documentation-lookups-during-startup--2026-11-19)
observed `ssh`, `gh` and `glab` executable lookups in
`InternalUrlRouter.describe`. The read/edit-only prompt now excludes that
call at `buildSystemPrompt`, and ordinary RPC exercises it as a positive
control. The startup-only sandbox denies process creation. The
[native controls](../../../docs/validation.md#restricted-startup-scheme-descriptions-and-process-policy--2026-11-19)
verify the source guard, kernel denials and ordinary positive cases.
The [addon controls](../../../docs/validation.md#native-unix-socket-creation-during-addon-load--2026-11-19)
bind the observed socket-pair call to the native image and separately test
the Tokio source path. The component result supplies a source attribution
for further qualification of the compiled addon.

Source inspection at `24439066040e006eb08da7b204f0ac41c79d2de9` distinguishes
`main.ts::buildSessionOptions` assigning `toolNames` for `--tools` from
`sdk.ts::createAgentSessionScoped` guarding MCP and memory work with the separate
`restrictToolNames` option. Tool names alone do not establish restricted startup.
RPC also starts settings watching, and telemetry initialization has settings/env
eligibility. Trace the final selected options, backends, owners and lifetimes
before freezing the inventory; this source census proves neither active model
work nor absence of these owners at runtime. In the selected diagnostic, the
source warm-up refuses before abort deadline construction. The
[named native timer controls](../../../docs/validation.md#rpc-runtime-callbacks-excluded-during-startup-observation--2026-11-18)
also observed both catalog deadline cancellations before its retirement.

The [native HTTPS controls](../../../docs/validation.md#native-https-startup-boundaries--2026-11-15)
now demonstrate a concrete alternate SDK path: Anthropic connection warm-up uses
`coworkFetch`/`node:https.request` outside global fetch in the diagnostic profile.
Replacing the default HTTPS export does not replace the namespace call on the
owned runtime. A `ClientRequest.end` refusal still permits an independently
observed loopback connection, so neither patch qualifies the required before-IO
boundary. Existing `PI_PROXY_ANTHROPIC` routing exposes the known warm-up to global
fetch, but the direct-HTTPS bypass remains. It was a configuration candidate, not
a selected fence or new proxy service. Owner decision, 2026-11-19, "Not used in
v2" ([record](../../../docs/validation.md#owner-decisions--2026-11-19)): v2 does
not use it, and any later use needs full request, tool and headroom qualification
first. Qualify every admitted owner/route before construction/IO at the actual
bridge/profile boundary; do not enable admission from a zero-fetch snapshot or
the narrow routing control. Ordinary configuration, frozen v1 and execution
authority remain unchanged.

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

The startup diagnostic now reads back its synced owner prefix through
`startup_rpc_retired` before binding its record count and SHA-256 on the
`startup_inventory_unqualified` halt row. A prefix readback or halt append
failure reaps the host and returns a persistence error without a successful
observation receipt. The count and digest attest this partial inventory.
The admitting revision still requires the complete inventory, matching private
Ready and durable echo before opening commands.

The startup-only stdout reader accepts one LF-terminated
`available_commands_update` envelope with a commands array. A missing or
repeated envelope, malformed frame, or other RPC message is a protocol fault.
A clean diagnostic receipt also requires normal process-group exit and EOF on
both the private channel and stdout. The host exit record carries the exit code
and terminating signal, so a host the sandbox ends is journaled with its signal
and `forced: false`. A fault during teardown is journaled
after the observation HOLD and host exit records, and the caller receives
the fault. The accepted prefix remains evidence of the observed owners.
See the [native teardown controls](../../../docs/validation.md#startup-rpc-output-through-teardown--2026-11-19).

For the admitting revision, use the existing private channel and preserve its
sequence across the handoff. Await the acknowledgment at the RPC boundary after
extension retirement and the initial output drain. That drain must leave the
runtime writer open. Read readiness facts from the bound session before enabling
RPC commands. Persist and read back the accepted Ready echo before opening the
parent writer or permitting a request. Keep the observation profile's teardown
and journal replay paths closed to admission.

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

### Proposed startup and run limits — 2026-11-20

**Proposed, 2026-11-20, not frozen.** This is a proposal for the owner. It is not
implemented and is not an execution authority. It proposes values for the finite
startup wall-clock, attempt and metadata limits that lines 1930–1931 require as
freeze inputs ("missing values receive no permissive default"). It also lists,
without values, the per-run bounds that lines 277–282 require in the new launch
manifest. It does not fund or freeze v2, does not change the frozen v1 contract,
pins or receipts, and leaves `execution_authorized=false`. Line numbers cite files
at commit `afa7b38`; for this draft, they cite its text before the 2026-11-20
proposals were inserted. Claude Code (Opus 5.5) prepared it from retained
no-provider receipts with a read-only script. Only self-review was done.

**Terms used here.**

- *Startup window*: the time from the monotonic instant the parent takes before
  it claims the run directory and launches the host (today
  [`startup.rs`](../../../src/pilot_capture/startup.rs) line 654) to the parent's
  synced readback of the accepted Ready echo (lines 1965–1970).
- *Startup prefix*: the journal records from `startup_observation` up to, but not
  including, the parent's `startup_ack`. The ack binds this prefix's sequence and
  digest (line 1923).
- *Normal run*: a startup diagnostic run with no injected fault that ends in the
  expected `startup_inventory_unqualified` HOLD, with a normal, not forced exit
  and the process group gone.
- *Current-shape run*: a normal run with all 18 records of today's diagnostic:
  observation, launch, two refused attempts, twelve owner messages, HOLD and host
  exit.

#### Evidence from retained startup journals

The sources are every retained `normal-journal.jsonl` with its `normal-result.json`
under `runs/pilot-startup-*/`. These are private, git-ignored receipts. There are
43 normal runs in 20 rounds from 2026-11-16 to 2026-11-19, all on the owner's Mac
with no provider and one heavy job per round. Of these, 23 are current-shape runs
from 11 rounds, covering 8 host builds and 5 bridge sources. The newest is
`runs/pilot-startup-catalog-refusal-binding-2026-11-19/startup-evidence/`:
18 records, 5,855 bytes, 2 attempts and 2.748 s from the first to the last record.
Its cache-free replay in `cache-free-startup/` took 2.656 s.

| Measure | 23 current-shape runs | All 43 normal runs |
| --- | --- | --- |
| First record to the last owner message (`startup_rpc_retired`) | 2.558–3.080 s, median 2.795 s | 2.558–4.616 s |
| First record to last record | 2.579–3.103 s, median 2.815 s | 2.579–4.673 s, median 2.815 s |
| First record to `host_launched`: run-dir claim, host SHA-256 check, private-tree check, materialization | 1.591–2.027 s | not computed |
| `host_launched` to first refused attempt | 0.898–1.076 s | not computed |
| First refused attempt to `startup_rpc_retired` | 68–94 ms | not computed |
| Last owner message to `startup_host_exit` (HOLD, close, reap) | 19–27 ms | 18–57 ms |
| Attempts per run | Exactly 2 in 23 of 23: one `model_connection_warmup` HEAD to `api.anthropic.com:443` and one `model_catalog_discovery` GET to `catalog.stencil.so:443` | 1 or 2. Rounds before the catalog refusal was routed through the channel show only the warm-up |
| Records before HOLD | 16 | 4–16 |
| Bytes before HOLD | 4,772–5,233 | 1,090–5,233 |
| Whole journal | 5,273–5,855 bytes | 1,589–5,855 bytes |
| Largest single record | 647 bytes (`host_launched`). The largest record carrying a child message is 388 bytes (`startup_catalog_retired`) | not computed |

The two slowest normal runs, at 4.673 s and 4.476 s, are 2026-11-16 runs of
older shapes (`pilot-startup-refusal-2026-11-16/startup-initial-evidence`,
`pilot-startup-inventory-2026-11-16/startup-qualified-evidence`). Their extra
time falls between `host_launched` and the first refused attempt: 2.620 s and
2.515 s, against 0.956–1.086 s in the other 2026-11-16 runs. The journals do not
record the cause.

**Limits of this evidence.** It all comes from the diagnostic profile, which
stops at HOLD. The admitting revision's `startup_settled`, `startup_ack`, Ready
echo and settings/model readback have not run anywhere. The journal times are
wall-clock display values (`display_unix_ms`,
[`pilot_capture.rs`](../../../src/pilot_capture.rs) line 1024) with
millisecond resolution, while the deadline uses a monotonic clock (`startup.rs`
line 654). The figures are approximate and come from one machine.

#### Proposed startup wall-clock limit

**Proposed value: 30 seconds over the startup window.** This keeps the existing
bound (`startup.rs` line 14, contract lines 1481–1483), so the value itself needs
no code change.

**Margin rule.** Let *E* be the largest startup-window elapsed time over at least
20 normal runs of the frozen admitting revision, on the frozen host, on the pilot
machine. The limit must be at least 5 × *E* and at most 30 s. If the frozen
inventory admits an owner whose settlement waits on a native deadline, those
runs must include that wait. Examples are the 10,000 ms warm-up timeout (line
752), if the final profile no longer refuses before constructing it (lines
1888–1889), and the host's five-second deadline on workspace-tree prompt
preparation ([record](../../../docs/validation.md#restricted-prompt-inputs-and-startup-launch-binding--2026-11-18),
validation.md line 591). If 5 × *E* exceeds
30 s, the freeze stops and returns to the owner. The limit is never raised
silently.

**Check against today's evidence.** Using the first-to-last-record time as a
stand-in for *E*, 5 × 3.103 s = 15.5 s for the current shape,
and 5 × 4.673 s = 23.4 s for the slowest normal run ever retained. Both fit
under 30 s. A 30 s limit is 9.7 times the slowest current-shape run and 6.4 times
the slowest run of any shape.

**Rationale.**

- **The costs are asymmetric.** Before the ack there is no counter, reservation,
  permit or provider transmission (lines 1936–1937). A long limit therefore costs
  only the wait on a hung host, with no spend. A false timeout halts and reaps
  the host (lines 1940–1942). Under the no-replacement and no-automatic-retry
  rules (lines 68 and 272) and the failure locks (lines 2161–2165), it is likely
  to end that scheduled row and stop further paid requests until the owner acts.
- **Why 5 ×.** The observed spread is up to 1.8 times across all shapes (2.579 s
  to 4.673 s) and 1.2 times within the current shape. For independent runs, the
  chance that one more run exceeds the largest of 20 is about 1 in 21. The
  multiplier is what keeps such a run, and the admitting revision's extra round
  trips, well below the limit. Five times the current-shape maximum (15.5 s) is
  still 3.3 times the slowest run ever retained.
- **The limit counts against the slot wall.** In v1 the slot wall covers host
  startup through shutdown (`pilot_capture.rs` lines 3051–3058), so the slot wall
  chosen below must budget for this limit.

**Alternative: 20 s.** This also satisfies the rule on current-shape data
(15.5 s ≤ 20 s) and detects a hang 10 s sooner. It leaves only 4.3 times the
slowest retained run (4.673 s). It saves at most 10 s per hung launch, at a
higher risk of a false halt if the admitting revision or a loaded machine
reproduces the 2026-11-16 slow case.

**Observed implementation gap, not a change made here.** The deadline is fixed
before launch but enforced only around the event loop (`startup.rs` lines 654 and
673–676). The synchronous `Host::launch`, which reads and hashes the pinned host
([`host.rs`](../../../src/pilot_capture/host.rs) lines 517–523), counts against
the limit but cannot be cut off by it. The reap after a startup failure has its
own 30 s grace outside the limit (`startup.rs` line 756); observed reaps took
18–57 ms. The admitting revision should either enforce the limit across launch
or name the gap as a limitation.

**Evidence that would justify a change.**

1. At least 20 normal runs of the admitting revision. These replace *E*, and they
   should journal the monotonic elapsed time at Ready-echo readback so the value
   is measured, not inferred from display times.
2. Any change to the host's pinned identity: the executable SHA-256, ordered
   patches, dependency lock, Bun runtime or native-addon digests (owner decision,
   2026-11-19, "Digest plus pinned inputs"). Any such change requires
   re-measurement.
3. A different machine, such as a Linux host.
4. Any startup timeout in a no-provider rehearsal of the frozen schedule. Treat it
   as a failure to explain, then re-measure.
5. Paid outcomes never justify a change. The limit is not changed after they are
   seen (line 219).

#### Proposed startup attempt limit

**Proposed value: per-owner caps equal to the counts seen in every normal run of
the frozen profile, with the total cap equal to their sum.** Today this is one
`model_connection_warmup` HEAD to `api.anthropic.com:443` and one
`model_catalog_discovery` GET to `catalog.stencil.so:443`, a total of 2. The
existing total of 20 (`startup.rs` line 13,
[`startup_observer.ts`](../../../src/pilot_capture/startup_observer.ts) line 68,
contract line 1482) remains the ceiling. The route set contains no proxy route,
because `PI_PROXY_ANTHROPIC` is not used in v2 (owner decision, 2026-11-19).

**Margin rule: no margin.** Each attempt is a discrete event that is classified
and refused before I/O, so its count has no measurement noise. The count is a
property of the frozen inventory. If counts differ between normal runs of the
frozen profile, the freeze stops until the difference is explained. For example,
a profile that can skip the catalog attempt needs explicit evidence of that
disposition (lines 1803–1804). Only after that explanation does the cap become
the observed maximum, with the minimum recorded beside it.

**Rationale.**

- The protocol already requires one attempt from each of these owners before
  its retirement (`startup.rs` lines 556–560 and 574–576). It also refuses an
  attempt after the owner's retirement (lines 540–546).
- A per-owner maximum closes the remaining gap. Today a second warm-up attempt
  before warm-up settlement is accepted up to the total of 20 (`startup.rs`
  line 547).
- An extra attempt means owner behavior that the frozen inventory does not
  describe, and unknown behavior must fail closed (lines 1761 and 1940–1942).
- The evidence shows exactly these two attempts in 23 of 23 current-shape runs.

**Alternatives.**

- **Per-owner cap of 2, total 4.** This tolerates one unobserved retry per owner,
  but the retry is then not detected.
- **Total of 20 only, the status quo.** An owner could make up to 18 extra
  refused attempts without a halt. Every attempt is still journaled.

**Evidence that would justify a change.**

- A newly admitted owner or route. Each one gets its own cap after its own
  qualification runs.
- A changed host identity or startup profile.
- A deterministic retry or skip observed and explained in qualification.

#### Proposed startup metadata limit

**Scope.** The limit covers the bytes of the startup prefix as appended to the
journal, counting each record with its newline, together with that prefix's
record count. This is what the ack binds. Overflow already halts (line 1941).

**Proposed value: a byte cap of 2 × *P*, rounded up to a whole KiB.** *P* is the
largest startup prefix observed over the normal runs of the frozen admitting
revision.

- **Row cap.** The row cap is the exact record count that the frozen inventory
  implies: 2 parent records, plus the attempt cap, plus one record per frozen owner
  message, plus each pre-ack record that the admitting revision adds, such as
  `startup_settled`.
- **Today's figures.** *P* is 5,233 bytes for the 16 records before HOLD
  (`pilot-startup-rpc-output-2026-11-19`), which gives 11 KiB (11,264 bytes) and
  16 rows.
- **Not freezable yet.** The admitting revision adds `startup_settled`, and the
  restrictions record's tool list, today fixed to `edit` and `read` (`startup.rs`
  lines 252–265), changes when the task set fixes the final tools (line 1827).
  *P* must be measured again after both.
- **Existing bounds stay.** The 64 KiB frame header
  ([`protocol.rs`](../../../src/neural/protocol.rs) line 26), zero payload
  (`startup.rs` lines 176 and 183) and 64 KiB journal record cap
  (`pilot_capture.rs` line 936) remain per-message ceilings. A tighter per-frame
  cap by the same rule would be 1 KiB (2 × 388 bytes, rounded up).

**Rationale.**

- **The size is predictable.** The protocol fixes the content except for
  variable-width counters and fixed-width identifiers. In each of the 19 rounds
  with more than one normal run, the whole-journal size varies by at most one
  byte between runs (for example 5,854 against 5,855 bytes in
  `pilot-startup-fixed-discovery-plan-2026-11-19`). The wider range between
  rounds (4,772–5,233 bytes before HOLD) comes from protocol and field changes,
  and those require a new freeze anyway.
- **Why 2 ×.** Doubling absorbs counter-width growth and still bounds what a
  faulty child can make the parent sync before the ack.

**Separate bound.** The diagnostic preload trace limit (8 KiB and 26 rows;
[`startup_probe.ts`](../../../src/pilot_capture/startup_probe.ts) line 29; lines
577–578, 771 and 995–996) is a qualification bound. It is not this
admitting-protocol limit.

**Alternative: a flat 64 KiB total,** the size of one existing record cap. This is
about 12 times today's prefix. It needs no recomputation when the tool list
changes, but it detects unexpected growth less sharply.

**Evidence that would justify a change.**

- A protocol revision, owner set or message field change.
- The final task tool list.
- Measured prefixes from the admitting revision's qualification runs.

#### Per-run bounds the freeze must choose

Lines 277–282 require these values in the launch manifest. V1 values are listed
only as evidence. They are not v2 defaults (lines 274–275), and within a task
block each value is equal across arms (lines 55–59). "Task set" means the value
cannot be chosen until the tasks, oracles and arm matrix are fixed. With 009 T005
closed without promotion (owner decision 17, 2026-11-19), the anchorless arm has
no promoted repair to bind (lines 45–48). That affects the arm count, so it
affects the whole-run and envelope rows. Owner decision 18 of 2026-11-20, "Drop
retrieval arms", settles this: v2 has no retrieval arm, so the arm count is that of
the task-context arms the freeze selects.

| Bound | What the freeze must state | Needs task set | Existing evidence | Proposed way to choose |
| --- | --- | --- | --- | --- |
| Startup window | Seconds, monotonic | No | Evidence above | Proposed above: 30 s |
| Per-generation deadline | Seconds from permit to terminal event, and what a timeout does to the reservation | Partly (output length) | V1 120 s (`pilot_capture.rs` line 108). V1 probe 9 s end to end ([validation](../../../docs/validation.md#paid-transport-probe-and-bounded-coding-pilot--2026-11-06), line 9146). V1 slots 14.4–22.5 s with at most 4 generations (validation lines 9165–9168 and 9190) | Measure on development or calibration tasks and the transport probe, never the sealed set. Set a stated multiple of the largest observed time. A timeout is not evidence of zero spend (line 2163), and unknown spend is not refunded (line 273) |
| Slot wall | Seconds from host launch to shutdown, per slot | Yes | V1 600 s (`pilot_capture.rs` line 107), covering startup through shutdown with a 10 s abort grace and a 15 s exit grace inside it (`pilot_capture.rs` lines 2120–2122 and 3051–3058). V1 used at most 22.5 s | Must contain the startup limit, both graces and the per-slot generation cap at the per-generation deadline. Otherwise state that the wall can end a slot early and record that as the slot's outcome |
| Preparation | Per-task or per-pair view preparation, total materialization, and whether it is fresh or amortized (line 295) | Yes | V1 180 s per pair and 900 s total ([`apparatus.rs`](../../../src/pilot_capture/apparatus.rs) lines 81–82). V1 used 3.8–4.1 s per pair (validation lines 9197–9198), about 2.5 s to prepare and 1.4 s to deliver ([record](../../../docs/validation.md#pilot-apparatus-and-frozen-launch-inputs--2026-11-06), lines 9318–9319) | Measure on the frozen task revisions and arm configurations. Report separately from host time (lines 295–296) |
| Checker | Seconds per compile-plus-oracle check, and the outcome on timeout | Yes | V1 60 s (`pilot_capture.rs` line 109, `apparatus.rs` line 1394, validation lines 9301–9304) | Set per task from the runs where the seed fails and the reference repair passes (line 2155). A timeout is a failed check, never a pass |
| Cleanup | Host exit grace, abort grace, workspace restore and startup reap | No | V1 exit 15 s and abort 10 s. Startup reap grace 30 s (`startup.rs` line 756), observed 18–57 ms. No-provider rounds reserve 120 s (for example lines 558–559) | Keep unless the admitting revision's runs show otherwise. Count inside the slot and whole-run walls |
| Whole run | Wall from first launch to final seal, with approval waits as their own interval | Yes (sample, arms) | V1 7,200 s (`pilot_capture.rs` line 110). V1 used 1,181 s, which includes 1,005 s of approval wait and operator work that were not split (validation lines 9194–9196) | Sum over the frozen schedule: probe, preparation, each slot's wall plus its check, and cleanup. Record approval waits separately (lines 295–296) |
| Generation permits per slot | An integer | Yes | V1 6 per coding slot and 1 for the probe (`pilot_capture.rs` lines 99–100). V1 used at most 4 (validation line 9190) | From calibration tasks |
| Total permits, count calls, HTTP attempts | Integers | Yes (schedule) | V1 49, 49 and 107, including up to 9 catalog GETs (`pilot_capture.rs` lines 101–104). V1 used 30, 30 and 60 (validation line 9189) | Derive from the schedule, with no automatic retries (line 272). The startup catalog GET is refused before I/O; it counts as a startup attempt, not provider traffic. State whether any catalog request is admitted after Ready |
| Startup attempts | Per owner and route, and the total | No | Evidence above | Proposed above: 1 + 1 = 2 |
| Input admission threshold and reservation | Tokens, with each arm's headroom recorded (lines 57–59) | Yes | V1 30,720 and 32,768 (`pilot_capture.rs` lines 85–86). V1's largest count was 17,164 (validation line 9193). Arm B's context limited its headroom (validation lines 9214–9215) | From the largest calibration request per arm plus the arm matrix's delivery budgets |
| Output cap | Tokens per generation | Partly (model, effort) | V1 4,096 (`pilot_capture.rs` line 87). V1 used at most 818 (validation line 9192) | From calibration under the frozen model and effort. It drives the reservation cost |
| Request body and SSE bytes | Bytes | Yes (follows the input threshold) | V1 1 MiB each (`pilot_capture.rs` lines 81–82) | Check against the largest admitted request |
| Reservation envelopes | Per slot, probe and total, at a pinned ratecard | Yes | V1 1,000,000, 500,000 and 10,000,000 microUSD (`pilot_capture.rs` lines 94–96). V1's highest slot spend was 591,300 (validation line 9191) | Cost the frozen caps at a pinned current ratecard, including failed attempts, preparation, the probe and unresolved reservations (lines 280–282). The budget is the owner's choice |
| Disk and memory | Free-disk floor and per-process RSS ceiling | Yes (task builds) | No-provider rounds used a 20 GiB floor, an 8 GiB sampled individual RSS ceiling and one heavy job (for example lines 558–559) | Measure again with the task repositories' builds and checkers |

The [sample design proposal](#proposed-sample-and-statistical-design--2026-11-20) counts generation
requests and provider HTTP attempts for its designs at 6 and 24 generation permits
per slot.

#### Decisions for the owner

1. **Startup wall-clock limit.** 30 s (proposed) or 20 s.
2. **Startup attempts.** Exact per-owner caps (proposed), one extra attempt per
   owner, or the total of 20 only.
3. **Startup metadata.** The 2 × rule with an exact row cap (proposed), or a flat
   64 KiB total.
4. **What a startup-limit halt does before any permit exists.** One option ends
   that scheduled row and halts the schedule; the other allows one restart with
   fresh identities. Line 1944 allows fresh identities on restart, while lines
   68, 272 and 2161–2165 forbid replacement samples and automatic retries. The
   proposal assumes no automatic restart, which is why it uses a wide wall-clock
   margin. If the owner allows a restart, a tighter limit becomes reasonable.
5. **Whether the startup limit must also cut off the synchronous host launch,**
   or whether that gap is recorded as a named limitation.

### Disclosed v1 gaps and selected repairs

| Disclosed gap | Required evidence |
| --- | --- |
| Host startup network denials absent from journal | Capture denied startup attempts and their destination/classification before any paid request; disallow unobserved routes. A sandbox denial is not evidence of journal completeness. Matching-release source establishes only partial diagnostics; a complete early observer and completion boundary remain needed ([source finding](../../../docs/validation.md#pilot-startup-network-source-feasibility--2026-11-11)). |
| Checker binds environment names only | Bind exact sanitized environment values, sandbox profile bytes and freshly hashed oracle/compiler bytes to each checker receipt. Credentials are absent from the checker environment. The selected local checks pass ([record](../../../docs/validation.md#pilot-checker-provenance-and-retained-diagnostics--2026-11-11)); the limits in the selected repair below and broader v2 qualification remain. |
| Compile stderr only hashed | Retain bounded private stderr bytes and their digest, with explicit overflow/failure behavior. Inspect errors; a matching binary hash cannot explain omitted diagnostics. The selected local checks pass ([record](../../../docs/validation.md#pilot-checker-provenance-and-retained-diagnostics--2026-11-11)); the limits in the selected repair below and broader v2 qualification remain. |
| Coding authority outside chain; declared/actual paths differ | Append the authorization digest to the durable chain before a paid attempt and require the canonical actual run root to equal the authorized root. The selected metadata/path and chained-authority fixtures pass ([record](../../../docs/validation.md#pilot-authority-and-run-path-binding--2026-11-11)); historical v1 evidence is unchanged. Current installed-host compatibility and actual Live-compatible probe history remain unproved. |
| Some private artifacts have wider permissions | Enforce and verify 0700 directories/0600 private files, including host/store outputs; fail before transmission if this cannot be established. Executables need explicitly declared private executable modes. The selected local metadata, child-launcher, lexical-store, pinned-host and actual-checker checks pass ([record](../../../docs/validation.md#pilot-private-output-permissions-and-terminal-publication--2026-11-11)); the real semantic-producer check qualifies its closed outputs ([record](../../../docs/validation.md#pilot-semantic-producer-permissions--2026-11-13)); the frozen-host private pipeline also passes after closed-preview archival ([record](../../../docs/validation.md#private-full-pipeline-and-preview-archival--2026-11-13)). Transient worker scratch-file modes, current installed-host compatibility, v2's declared task/arm/host qualification and complete startup-network observation remain unqualified. Same-user races and power-loss durability are named limitations, and forced permission-call and sync-call failures have fault-injection tests in `232a7ad` (owner decision, 2026-11-19, "Accept a and c, test b", below). |
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

Owner decision, 2026-11-19, "Accept a and c, test b"
([record](../../../docs/validation.md#owner-decisions--2026-11-19)), settles three
remaining protection questions:

- (a) A racing process of the same user is a named limitation. The walk refuses
  replaced entries and wrong modes, and the writers create each object
  exclusively, but neither stops a same-user process that changes an output
  between two checks. Re-entry trigger: an observed change to a pilot private
  output by a process the pilot does not own.
- (b) Forced failures of the permission calls and sync calls get fault-injection
  tests. Commit `232a7ad` adds them: a test-only seam fails seven named calls, and
  each test checks the exact fault, refusal of a second exclusive write and no
  widened leftover
  ([record](../../../docs/validation.md#private-writer-fault-injection-tests--2026-11-20)).
- (c) Power-loss durability is a named limitation. Writers sync each new file and
  its parent directory, but no pilot test checks what survives a crash or power
  loss. Re-entry trigger: a synced pilot record found missing or truncated after
  a crash or power loss.

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
