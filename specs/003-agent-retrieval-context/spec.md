# 003 — Context inside coding agents

Status: T001–T003 and the optional shared owner implemented and verified locally,
2026-10-01, including concurrent OMP 18.4.9 and Codex 0.159.2 service on one store
([validation](../../docs/validation.md)). Not released. T004 gateway remains proposed,
outside this tranche. No production stores, services or global host configuration changed.

## Outcome and requirements

A real agent locates `parse_record`, makes the stated fixture edit, checks that edit,
re-indexes through its existing store owner and retrieves the changed source.
The 2026-09-29 amendment adds explicit repository bootstrap and adapter budgeting
through the same owners. The owner's subsequent answer also selects optional request
forwarding/metering: one Rust gateway for explicitly configured host traffic, with
separate acceptance from MCP. This is not transparent interception of all providers.

The 2026-10-01 amendment
makes Foundry the default source-discovery route before grep/ripgrep in an explicitly
configured agent session, with named exceptions and real-host acceptance below.

- **FR-001:** A maintained SDK exposes one store owner through default stdio MCP or
  explicitly selected loopback Streamable HTTP MCP, using ordinary engine operations:
  `search`, `context`, `retrieve`, `index`, `status`. No custom socket/shim or shell tool.
- **FR-002:** Use 001's [shared source/response contract](../001-source-state-recovery/contracts/context-v1.md)
  for handles, errors, scope, source identity, defaults, bounds and snapshot freshness.
- **FR-003:** Budget the actual serialized server-owned result including MCP wrappers;
  host/provider additions remain unknown without consumer receipts. Source and graph
  evidence are data, not instructions. Keep diagnostics off protocol stdout.
- **FR-004:** Admission, cancellation, disconnect and lost-response behavior follow
  the rules below. A response never confuses accepted work with committed state.
- **FR-005:** Inspection/apply bootstrap and printed per-project host configuration
  follow [deployment](../../docs/deployment.md). Partial optional setup preserves useful
  source context; paths in ordinary queries never initiate bootstrap.
- **FR-006:** Enforce and label the actual budget boundary under the
  [adapter economics contract](contracts/adapter-economics.md). Full-request limits
  require actual host hooks; receipts never become training permission or source truth.
- **FR-007:** An explicitly enabled Rust gateway forwards supported model requests,
  preserves streaming/tool semantics, enforces configured admission/output bounds and
  meters actual provider usage. Credentials, retries, unknown outcomes and deployment
  follow the same contract. Gateway failure never silently bypasses policy upstream.

- **FR-008:** The configured agent uses Foundry first for eligible source discovery,
  without requiring the user to name a Foundry tool in each prompt. Bootstrap exposes
  the tools and project guidance; T003 verifies actual ordering. Available MCP tools
  alone do not establish native adoption or enforcement. Fallback follows the rules
  below; no shell interception, command shadowing or new routing service.

## Native source discovery and fallback

Owner: the existing 003 host adapter/setup; core source and budget rules stay in 001.
"Native" means the supported host exposes the existing tools as ordinary agent tools
and selects them by default for this workflow. It does not mean Foundry replaces the
host's filesystem, shell or all search semantics. Install no second discovery router.

Bootstrap prints the supported host's root/store configuration plus a small, stable
project instruction block defining this preference and its exceptions. Printed setup
does not edit host files. Applying host instructions/configuration requires existing
authorization, preserves unrelated operator bytes and changes only a positively owned
block; an edited/conflicting block is displayed for manual integration, not overwritten.
Use the selected host's existing project/session instruction facility. No generic
configuration manager, global prompt injection or extra instruction file format.

| User intent | Default agent operation |
| --- | --- |
| Locate an identifier or relevant source in an admitted indexed root | `search` before repository grep/ripgrep |
| Understand a subsystem, behavior or available relationships | One budgeted `context` request; no mandatory search/context/graph sequence |
| Follow an existing source handle | `retrieve`, validating its workspace/hash/range |
| Inspect a known current file, unsaved editor buffer, exact byte/regex pattern, or perform an exhaustive current-filesystem scan | Existing host read/search facility; Foundry has no implied parity or live-disk guarantee |

An exception follows the actual task's required semantics. Rewriting an ordinary
identifier lookup as a regex does not exempt it from using Foundry first.

An eligible discovery request attempts Foundry once before shell search. If useful
evidence is returned, follow its citations and do not automatically repeat the same
discovery with grep. A later distinct question or explicitly requested exhaustive
check remains permitted. Optional models are not required for this ordering: use
ready baseline retrieval when graph/semantic/policy components are unavailable.

Fallback is permitted when the required search semantics are unsupported, the source
is outside indexed coverage, or the attempt is empty, limited/incomplete, busy, expired
or unavailable. Name that reason in the host transcript from the tool result or, when
startup/connection failure leaves no tools, the host's MCP status or bounded server
diagnostic. If the host exposes no cause, report unavailability without inventing one.
Do not add a durable fallback ledger, health-poll loop or telemetry to a warm prefix.
An empty/limited result never proves absence. Use the existing shared read deadline;
no hidden retry, automatic index/repair/preparation, extra model router or waiting for
the corpus to finish. Explicit refresh still goes through the existing store owner.

Host fallback is independently authorized, scoped to the selected roots and labeled
host/current-file evidence, not Foundry-validated evidence. A permission refusal,
unsafe path or foreign-workspace handle cannot widen that scope or be bypassed by
another tool. Authoritative corruption remains visible and cannot be reported as a
healthy Foundry result. Mentioned external paths still do not enroll a root. The same
preference applies to each explicitly admitted repository; combined root selection
and joins remain 007's separate scope, not implemented by this adapter amendment.

Record whether the selected host supports project instructions only or an actual
native tool-selection hook. Instructions establish a preference; claim enforced
routing only when the real hook controls eligible calls and its bypass cases pass.
T003's transcript is acceptance for the named host/version/workflow, not proof that
all hosts or every future model response obey the preference. A host without that
evidence is advertised as tool availability, not verified native source discovery.

Fallback payloads and calls enter existing complete-request accounting when the host
exposes them. MCP-only Foundry delivery counters cannot establish their cost or savings;
reuse the [adapter economics contract](contracts/adapter-economics.md), not new receipts.

## Interface and lifecycle

Command: `foundry --store DIR mcp --root ROOT`. Root is canonicalized and
bound once; requests cannot switch it. Startup fails before serving on store busy,
wrong workspace or unsupported schema. The [official Rust MCP SDK](https://github.com/modelcontextprotocol/rust-sdk)
provides stdio/lifecycle/framing; T001 locked rmcp 3.5.0, which builds and passes
`cargo check` on Rust 1.90. No compatible release would be a named `sdk_incompatible`
prerequisite failure; do not handwrite MCP.
Use 001's explicit initialization only for a new store; an existing store is never
upgraded by serving. A broken lexical index permits authoritative-only startup and
the shared failure-scope behavior. Register the implemented tool set once per process;
graph/model readiness changes status/results, not the tool catalog or its schemas.
Listing tools and health/status never loads a model or initiates maintenance.

Use one serialized engine worker with **one active operation and zero waiting engine
operations**. A valid concurrent engine request returns `busy`, retryable true, without
mutation; it is never silently queued. SDK initialization/cancellation messages remain
serviceable while the worker runs. Limit incoming stdio JSON frames to 64 KiB before
full allocation/decoding; close that stdio session on oversized/malformed frames with
a bounded stderr diagnostic. HTTP rejects the offending request only. Normal
malformed tool arguments return contract errors.
Cap SDK in-flight handlers at 16; overload closes/refuses through SDK-supported behavior,
not an application protocol. T001 must prove these limits with the chosen SDK.

`index` takes optional `timeout_ms` default 30,000, range 1..1,200,000. Read operations
have a 5,000 ms cooperative deadline. Validate before dispatch; check cancellation/
deadline between files, batches and library calls. Never return a partial read as
success after cancellation or deadline. Named optional-feature fallback under 001 is
a completed baseline response, not a claim that the failed component succeeded.
A library/OS call may exceed the deadline: the worker remains occupied until
it returns, then returns `deadline_exceeded`. This is not a hard latency guarantee.
Optional 009/013 provider calls consume this same deadline, starting at admission;
their individual ceilings cannot extend it. A model failure may produce the complete
baseline result with an explicit fallback before expiry, not a success after expiry.

A cancelled/expired index stops future work. Where a reply can be delivered, return
the shared counts-only partial error with committed counts and pending work; after
lost/cancelled delivery the client inspects status. `scan_complete` means enumeration
and sweep both finished;
`deletions_deferred` is true only when sweep was withheld or incomplete. An index-drain
timeout after a completed sweep does not falsely mark source enumeration incomplete.
Already committed source batches are not rolled back. Once sweep begins after
complete enumeration, completed deletions remain durable. On stdio EOF/disconnect stop
admission, request cancellation, finish the current transaction and exit. A forced
termination is recovered under 001. A lost reply is an unknown client outcome:
inspect status after reconnect and repeat idempotent `index`; no receipt journal.
HTTP DELETE, SDK session expiry, MCP cancellation or owner shutdown cancels that
session's work at the next control check. A dropped response stream alone does not:
work continues to its own deadline and retains the engine slot. Status while another
engine operation runs returns `busy` too.

No memory/graph/feedback tools are silently added by this spec. Their owning specs
may extend this exact worker. CLI/store opens cannot run alongside the MCP owner;
there is no lock retry or hidden second writer. Independent clients may share the
explicit HTTP owner below; a stdio session still excludes competing store owners.
005's artifact import and 008/013's selected writes use this worker too. Offline-only
maintenance and learning preparation require stopping the store owner, not just one
of its HTTP clients.

009 explicitly extends this topology for neural preparation: one bounded external-
inference worker returns results to this same store owner. It does not become another
store writer or occupy the engine slot while waiting on a model. Keep 003 alone small;
the preparation tools, lifecycle and coexistence proof belong to 009 T003.

### Optional shared owner — approved amendment, 2026-10-01

The owner requires simultaneous OMP and Codex service against one repository store.
Default stdio remains useful for one host and borrowed subagents. An explicit foreground
`foundry --store DIR mcp --root ROOT --transport streamable-http --bind 127.0.0.1:PORT
--auth-token-env NAME` adds a standard SDK listener at `/mcp`; port 0 selects a free
local port. It never starts automatically, discovers repositories, installs a launch
agent or adds a registry/federation coordinator. One persistent engine owns one root;
all clients share the same one-active/zero-queued admission. No per-host duplicate
authoritative stores or operation-scoped rotating writers.

Both transports advertise session-bearing MCP versions through `2025-11-25` only:
SDK `known_up_to(V_2025_11_25)` and HTTP `legacy_session_mode=true`. An `initialize`
naming a newer version receives standard MCP version negotiation to `2025-11-25`
with a session; any request that would use the `2026-07-28` initialize-less stateless
path is refused, never served. T001 tests both; T003 records each host's negotiated
version. A host unable to negotiate a supported session-bearing version is
`host_unsupported`. No stateless allowance/cancellation scope is implied.

Pin SDK session `keep_alive` to 300 seconds and completed-reply cache to 60 seconds.
An abandoned session occupies one of the 16 slots until expiry; idle clients must
re-initialize when the SDK expires their session. T001 verifies refusal while full
and admission after expiry; T003 records each host's post-idle recovery or named
limitation. Stream loss can resume only inside the SDK cache window; otherwise
the client inspects status, without automatic index or a durable receipt journal.


Require a nonempty secret bearer token from the named environment variable before
opening the store; never print it or put it in source, logs or generated configuration.
Bind IPv4 loopback only. Authentication applies to every MCP HTTP method before
protocol/session allocation. Reject mismatched Host and any Origin header: browser
clients are not selected. Enforce the 64-KiB incoming body bound before full buffering
or JSON decoding, including chunked input. Reject oversized/malformed HTTP requests
without killing other clients. Cap live SDK sessions at 16 and in-flight handlers at
16 globally; refuse overload without a waiting engine queue or unbounded spawn.
Use SDK session IDs/DELETE/cancellation semantics, not an application protocol.

Neither stream loss nor cancellation releases executing work or its admission
permit early. Shared-owner shutdown stops admission and cancels/closes SDK sessions;
process exit waits for the current engine transaction, and in-flight replies can be
lost. Another stdio/HTTP owner or CLI writer gets `store_busy`, with no takeover.
Missing listener/auth/unsupported native transport is named host unavailability;
no stdio-to-HTTP shim, secret query parameter or silent bypass.

Connection-local delivery caps/IDs follow the same economics contract on either
transport. HTTP clients do not share allowance counters. The listener introduces no
source writes, model loads or maintenance from initialize/list/status. Printed
OMP/Codex configuration is project/session-scoped and references the exact URL and
token environment name, not the token. Default configuration remains stdio.

The SDK's legacy-peer result rewriting must be a no-op on already-counted bytes:
clear `CallToolResult.result_type` before serialization/counting. T002 captures and
compares the actual emitted result value. Generated host request timeouts exceed the
largest advertised index timeout, so an ordinary default index does not routinely
lose its reply at the same client deadline. Session allowances remain connection-local,
not cross-reconnect quotas.


T001/T002 cover real SDK clients on both transports: authentication, Host/Origin,
body/session/handler limits, disconnect/cancel/owner shutdown, concurrent callers,
scope and exact emitted result bytes. T003 additionally records actual installed
OMP and Codex versions and native HTTP attachment to the same fixture store at once.
No simultaneous-service claim without both native-client proofs. If the maintained
SDK or a named host cannot satisfy this boundary, stop at that named prerequisite;
do not substitute fixtures or revive 002's relay.


## Tasks

### T001 — Serve bounded operations over real SDK MCP

Feasibility input: rmcp 3.5.0 built with Rust 1.90 and passed real stdio
initialize/list/call/close in a jailed Linux fixture. Use its explicitly capped codec
through the sink/stream adapter; default AsyncRwTransport has unbounded line buffering.
The 64-KiB decoder refusal is demonstrated, but the production 16-handler admission,
cancellation and engine-ownership cases below remain acceptance. See
[probe evidence](../../docs/review/feasibility.md); do not infer them from the smoke.

- **Depends:** 001 T001–T003. **Scope:** MCP/bootstrap/adapter modules, registration in
  `src/lib.rs`, CLI commands, pinned Cargo dependencies and `tests/mcp.rs`. Standard
  loopback HTTP is included; custom sockets, host hooks, model setup and global config are not.
- **Outcome/acceptance (FR-001, FR-004, FR-005 / SC-001):** a real SDK client initializes,
  lists exactly five tools, indexes the configured fixture root, searches and shuts
  down. Worker admission and frame/handler bounds match the contract above.
- **Verification:** missing/wrong typed/unknown tool args; frame size limit and limit+1;
  two concurrent engine calls (one executes, one busy); cancellation during scan and
  between index batches; EOF and forced process exit; second store owner refused.
  Bootstrap inspect writes nothing; apply indexes one explicit root; interrupted apply
  reuses committed state; unavailable optional components leave baseline ready with
  named setup work. Printed host config preserves unrelated bytes and pins root/store;
  it includes the native-discovery instruction block and supported host capability.
  Exercise print-only setup, authorized owned-block insertion, identical reapplication
  and edited-block conflict without overwriting operator changes.
  Reopen and compare all acknowledged source changes and partial counters to disk.
  Start with damaged derived search: status and direct retrieve work, search names
  repair_required, and the tool catalog remains unchanged across readiness changes.
- **Review/cutover:** verify SDK cancellation does not drop a transaction midway or
  release the worker while it still runs. Retain CLI commands for offline use; remove
  no user config. A failed smoke leaves MCP unadvertised and the CLI usable.

### T002 — Deliver exactly budgeted cited results through that adapter

- **Depends:** T001 and 001 shared response implementation. **Scope:** adapter result
  rendering and `tests/mcp.rs`; packing/handle validation stays owned by the core.
- **Outcome/acceptance (FR-002, FR-003, FR-006 / SC-002):** actual emitted MCP result satisfies
  the shared contract, including double JSON escaping and no duplicate content field.
- **Verification:** all shared boundary cases through the real stdio stream. Capture
  emitted result bytes before host transformation; count using the locked tokenizer.
  Old handle after edit→stale, after delete→not_found, wrong root→wrong_workspace.
  A prompt/query mentioning another repo leaves the bound workspace and source
  bookkeeping unchanged. The MCP index tool has no per-request root override;
  unknown root arguments are `invalid_argument`. Repeat with the server launched
  from a different directory and with a foreign-workspace handle.
  Inject instruction-like source text and confirm it is quoted data with citations.
  Enforce delivery/session caps before optional inference. Test receipt deduplication,
  missing usage, cached-input categories and final host-envelope overflow. A requested
  host-request mode without actual hooks is budget_scope_unsupported, not a claimed pass.
- **Review/cutover:** ensure counting and emission use identical serializer output and
  no post-count metadata. Version the adapter result recipe; preserve CLI text mode.
  Failure returns a bounded tool error, not an over-budget result or engine mutation.

### T003 — Complete one actual coding-agent task

- **Depends:** T001/T002 and an explicitly selected installed MCP-capable host. Record
  host/version, local launch command and session-only configuration before execution.
  Missing host/access is `integration_not_run`, never a fixture substituted as a pass.
- **Scope:** a fresh temporary copy of `examples/workspace`; add a small Rust fixture
  harness in `tests/fixtures/agent-task` (new) with no third-party dependencies. Agent
  tools/configuration touch this fixture only; restore exact prior host settings if
  a temporary session configuration was not supported.
- **Outcome/acceptance (FR-001–FR-006, FR-008 / SC-003):** with the printed project
  guidance active, use an ordinary task prompt that does not name Foundry tools.
  Ask the agent to make `parse_record` trim
  whitespace on both sides of `=` and reject an empty key, preserving the Option tuple
  interface. The checker asserts `" mode = local "→Some(("mode","local"))`,
  `" = x"→None`, `"mode"→None`, and `"mode=a=b"→Some(("mode","a=b"))`.
  Its first eligible source-discovery call must be Foundry search/context, before
  repository grep/ripgrep. Availability alone, or an eligible grep first, fails this
  adoption check even if the edit is correct. It must use context/retrieve citations,
  make the edit, pass the checker, re-index
  through MCP, and return the new hash/bytes; the pre-edit handle must then fail.
  Start from bootstrap inspection/application and printed host setup. Record actual
  adapter capabilities; exercise user-driven/delivery mode and host-request mode only
  if the selected host supplies the required hook. Show an exhausted allowance refusing
  additional work within its declared boundary. No full-host control claim from MCP alone.
- **Verification/Review:** in the same bounded fixture workflow, exercise one
  unsupported exact-pattern request and one unavailable/busy Foundry attempt. Record
  the permitted host fallback and reason; no repeated identical discovery, hidden
  indexing/model setup or outside-root expansion. Missing optional models must leave
  baseline discovery usable. If enforced routing is claimed, exercise its real host
  hook and bypass/refusal cases; instruction-only setup cannot pass that claim.
  Record tool transcript with public fixture data and actual
  checker exit code; remove only positively owned temporary files/configuration.
  One successful task proves this integration, not task-success uplift or savings.
  A cost claim additionally needs actual whole-provider usage/caching and a correctness-
  matched comparison; it is not required for functional acceptance.

**Implementation record (2026-10-01).** T001/T002: `cargo test --locked` mcp suite 65
tests over real rmcp clients on stdio and HTTP (admission, frames/bodies, sessions,
protocol pin, cancellation/deadline/EOF/kill, exact emitted-byte accounting, bootstrap,
owned config blocks, receipts). T003: the [real-host record](../../docs/review/real-host-t003-2026-10-01.json)
shows OMP using Foundry `search` before any grep, editing, passing the checker,
re-indexing through MCP and getting `stale_handle` for the pre-edit citation, while
Codex concurrently answered a discovery task through Foundry alone. Host facts: OMP
exposes MCP tools as `xd://` devices and honors the project instruction block as a
preference; Codex needs the printed approval configuration (`writes`, `approve` for
`index`) because it otherwise requires per-call MCP approval. Neither host offers an
enforced routing hook, so no enforcement is claimed. Delivery allowances were exercised on a real host;
host-request mode remains `budget_scope_unsupported` without hooks.

### T004 — Forward and meter an actual supported model workflow

- **Depends:** T002 accounting types; explicit permitted provider/model credentials
  and a host with custom endpoint support. No dependency on 009/013 or source-store
  ownership. Current first target is Codex over Responses HTTP/SSE; pin the actual
  versions, request subset, provider limits and token-counting projection before code.
  Missing access/schema compatibility is `gateway_integration_not_run`; no silent
  substitution with a mock or a claim of universal adapter support.
- **Scope:** Rust `src/gateway.rs`, CLI/config printing and focused gateway tests;
  maintained HTTP/TLS/SSE dependencies. No generic proxy framework, public listener,
  account credential database, persisted prompt history or per-query source writes.
- **Outcome (FR-006, FR-007 / SC-004):** a real host streams a response, completes a
  local tool-call/result turn and reports gateway-observed usage. Exercise meter and
  enforce modes; an over-cap or unverifiable request makes zero generation calls.
  Unknown terminal usage remains charged/unknown. Gateway and MCP can coexist without
  sharing a store lock. No quality/cost improvement follows from forwarding alone.
- **Verification:** narrow protocol fixtures cover split SSE frames, tool deltas,
  malformed/oversize input/output, auth/Origin/Host/redirect refusal, slow clients,
  count failure, cap/exhaustion, retries, disconnect after send, log full, process exit
  and restart scope. Real-provider evidence checks count projection, terminal cached/
  reasoning usage and API-key auth; actual host configuration must support the defined
  stateless subset. Disable host retries and WebSockets where supported and verify
  behavior; unsupported required host features keep the integration unadvertised.
  Bound the live check to one declared fixture workflow and explicit token/time caps;
  no paid call is implied by writing this task. Provider charges require execution
  authority and configured resources at that later boundary.
- **Review/cutover:** install the actual local gateway package, create session config,
  verify listener/token permissions and isolated model-worker credential denial.
  Stop admission on shutdown, terminate pending connections by the stated deadline,
  preserve receipt coverage, and remove only this run's token/config artifacts.
  Restore exact prior host config on opt-out; never fall back to direct upstream
  requests while a required gateway is unavailable. Release the gateway separately
  from CLI/MCP; no new measurement-close or fleet-governance stage.

SC-001..SC-004 are the task pass conditions above. Implementation, SDK/provider
compatibility and consuming-host evidence are unexecuted. T004 is one additional task
because the owner added credential-bearing forwarding, not hidden work inside packing.
