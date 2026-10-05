# Adapter economics contract v1

Status: Delivery budgets and receipts implemented 2026-10-01 (003 T002); host-request
mode proposed (contract proposed 2026-09-29). The owned model gateway (§ Owned model
gateway) was implemented and accepted 2026-10-04 (003 T004), meter mode only.
Delivered-token economics
(§ below) was approved 2026-10-03: 001 T004–T006 (wire, atomic allowance, catalog text,
syntax-unit ranking and outlines) are locally implemented, accepted and committed
(`5edf32c`), unreleased; 003 T005's usage import, economics tests and operator hook and
007 T001 are implemented and accepted locally on 2026-10-04 (`5e99ffd`, `cc402e0`), with the
real-host runbook recorded in [validation](../../../docs/validation.md). Owned by
[003](../spec.md); core packing/identity remains in [001](../../001-source-state-recovery/contracts/context-v2.md).
This adds practical budget control, usage receipts and the owner's subsequently
requested optional model gateway. The gateway is a narrow Rust forwarding command, not
a fleet cost governor. Delivery allowances were exercised on a real host in 003 T003.
The gateway's metering was exercised against real OMP and Z.ai in 003 T004
([evidence](../../../docs/review/t004-gateway-2026-10-04.json)). No savings result has
been claimed from it.

## What Foundry controls

| Integration | Enforceable boundary | Evidence available |
| --- | --- | --- |
| User-driven CLI | Exact search/context/retrieve stdout under the caller's budget (search from 001 T004) | Owned bytes/tokens, citations and omissions; provider use unknown |
| MCP-only host | Foundry tool-result budget on the final text block, serialized result byte-capped (v1 counts the whole serialized result until 001 T004), and an optional connection-local delivery allowance | Emitted text-block tokens and result bytes; host wrapping, other tools/history/provider costs unknown |
| Adapter with actual pre-send and usage hooks | Host-authorized context allocation inside its complete model request; request/session limits on calls it controls | Exact rendered request under model tokenizer and actual provider usage when supplied |
| Host explicitly routed through Foundry's gateway | Admission and output ceilings for every supported generation request reaching that gateway | Forwarded request identity, provider-reported usage and visible unknown attempts |

The adapter declares observed capabilities at initialization. Configuration cannot
invent a hook the host lacks. A requested full-request cap without the hook returns
`budget_scope_unsupported`; it does not silently downgrade to an MCP payload cap.
No claim to control another adapter, direct host API calls or independent sessions.
Baseline context remains usable without usage visibility. Gateway routing adds the
missing request boundary when the host supports a custom provider endpoint. It does
not intercept bypass traffic or confer control over a user's provider account.

## Deterministic budget policy

Host/project config v1 contains `max_context_tokens` (1..32768, default 2048), optional
`session_context_tokens` (positive integer), `tokenizer_id`, and `scope` (`delivery` or
`host_request`). Host-request mode additionally requires `max_input_tokens`,
`max_output_tokens` and optional `session_provider_tokens`, positive checked u64s in
the provider's units, plus a pinned model/counting recipe. Delivery mode rejects those
host-only fields. Unknown/null fields refuse. The core's o200k contract remains named;
a host-request integration additionally pins the actual provider tokenizer and its
counting recipe. An unknown provider tokenizer is not replaced with chars/4.

Effective context allowance is the minimum of caller request, configured context
ceiling and remaining connection/session allowance, all in the core's named tokenizer.
It applies to search, context and retrieve (search from 001 T004). v2 headers report
that effective value as `budget:<n>`, suffixed `(ceiling)` or `(session)` when that
bound produced it; ties report the request, unsuffixed. A refusal names the limiting
bound (`request`, `context_ceiling` or `session_allowance`), and its hint is sufficient
under any label (added 2026-10-01 after a real host misread an allowance-limited budget
as a defect; the v1 envelope carried the same facts as `requested_budget` and
`budget_limited_by`, counted inside the envelope).
Host-request mode adds a separate provider-token fit constraint, not a minimum of
incomparable token counts. Host residual = model input window minus exact fixed/history/tool envelope
and reserved maximum output tokens. Zero/negative residual refuses before retrieval
or inference. If the model cannot supply that boundary, request-scope enforcement is
unavailable rather than guessed. Counts for different tokenizers are never subtracted;
the adapter measures provider-token fit separately from the core's output cap. Compare
the complete final input against max_input_tokens and the model's supported input/
output limits. Reserve counted input plus requested max output against any provider
session cap. Reconcile only with actual usage; an unknown attempt keeps its reservation.
Verified usage above its bound disables further admission with `provider_limit_breach`;
Foundry cannot retroactively prevent a provider violating its own limit contract.

Delivery mode reserves atomically. One session-budget critical section computes the
effective allowance and reserves it:
`Shared::reserve_effective(session, requested) -> Result<(u64, BudgetLimiter), Refusal>`.
A refused reservation changes no counter and returns `budget_exhausted`. Each response
owns exactly one reservation: busy, packing failure (`budget_too_small`) and a
pre-delivery deadline or cancellation refund it exactly once; success charges the
counted tokens of the final text block and refunds the rest. This replaces the
implemented `remaining_allowance`, `effective_with_session` and `reserve` sequence,
whose separate critical sections let a concurrent same-session call act on a stale
remainder and whose refused `reserve` set the remainder to 0 (a latent defect found by
source review on 2026-10-03, not observed on a host). An interrupted write with unknown
delivery keeps the reservation charged conservatively for that session. No per-query
write into source/cache tables. Confidence cannot raise a budget. The session counter
uses checked u64 arithmetic; exhaustion/overflow refuses before dispatch. CLI calls have
no cross-process session cap unless a host adapter owns it.

Host-request mode reserves before dispatch and counts the exact final request once:
the adapter performs its final complete-request check after all host transformations
and before the model call; later added data invalidates the count. Core success cannot
override that check. Deadline/packing failure releases unspent reservation.

No delivery IDs are emitted while only delivery scope is supported: `host_request` is
refused at startup, so no delivery can be attributed to a provider request, and the
v1 per-delivery UUIDv4 `context_id` cost tokens on every delivery while no host consumed
it. 001 T004 removes it. A future host-request integration owns conditional delivery
IDs, their attribution in receipts' `context_ids` and the gateway's optional
`X-Foundry-Context-Ids`; such IDs would be observations, not stable source handles,
consent or durable receipts, with no source-store write and no place in warm prefixes.

MCP-only session caps cover Foundry deliveries on that connection, including repeated
deliveries. They reset with the connection and are not durable monthly dollar caps.
Host-request caps require the adapter to mediate every request in the claimed session.
A retry that actually calls the provider is new usage. Never refund a provider attempt
merely because a local request timed out or the response was lost.

Preserve caller-owned warm prefixes and stable tool catalogs. Put changing retrieved
evidence in the designated context slot; do not rewrite history/system text or append
telemetry into a cacheable prefix. Use verbatim evidence or deterministic outlines with
explicit elision markers, deduplication within one response, continuations and
omissions; no summarizer or compression model. The user may explicitly change the
budget or retrieve more. 013 selects graph expansion within the same allowance.

## Usage receipts and cost

Optional local receipt after an actual provider response:
`{v:1,session_id,request_id,adapter_id,model_id,context_ids?,input_tokens?,output_tokens?,
cached_input_tokens?,cost_microunits?,currency?,cost_basis?,ratecard_id?,outcome,
observation?}`. Optional observation is `{mode,elapsed_ms,count_elapsed_ms?,
provider_request_id?,provider_response_id?,delivery?}`; mode delivery/host_request/meter/enforce,
times nonnegative u64 milliseconds, optional provider IDs nonblank <=256 bytes.
`delivery` (gateway only) is `delivered`, `client_closed` or `local_failure` and records
the response's delivery to the host separately from `outcome`, which describes what
the provider observation established; a closed client never erases known counts.
IDs nonblank <=256 UTF-8 bytes; context_ids <=64 distinct delivery UUIDs; receipt <=16 KiB.
Receipts omit `context_ids` until a host-request integration emits delivery IDs.
`outcome` is `complete`, `failed` or `unknown`. Counts/cost are
nonnegative checked integers; unknown/null fields refuse. Missing counts stay unknown.
Normalize input_tokens to include cached input; cached_input_tokens must not exceed
input_tokens. Pin the adapter's normalization recipe rather than summing different
provider categories blindly. Receipts are attributed observations, not cryptographic
certification that a manually supplied counter is truthful.

Cost is a mutually present triple of `cost_microunits`, `currency` (three uppercase
ASCII currency-code letters) and `cost_basis` (`reported` or `calculated`); no partial
triple. `ratecard_id` is allowed only for calculated cost and is required there. Calculated amounts
also require a versioned ratecard covering actual model/cache/output categories.
Missing categories/rates mean unknown total cost, not zero. Reported charges can
differ from local calculation; taxes/discounts/billing totals outside the observation
remain unknown. Neither a token cap nor a post-hoc receipt is a hard dollar cap.

Receipt key is `(adapter_id,session_id,request_id)`. Identical retry counts once;
changed values under that key are `receipt_conflict`. One context may feed several
model requests: charge each request, but do not add context-delivery tokens again to
provider totals. Lost provider replies mean unknown usage until actual evidence arrives.

Receipts carry counters/identities, not raw prompts, source, keys or training labels.
Keep them in session memory or an explicitly enabled private JSONL file, not another
engine ledger. A configured log cap/write failure names `usage_log_full`/write failure;
it cannot corrupt source or fabricate known totals. User-driven
`foundry usage summarize --input FILE` reads at most 16 MiB/10,000 rows and reports known
totals plus missing/conflicting coverage; it opens no store or provider connection.
Excess refuses rather than truncates. Receipt logging grants no training consent.
`foundry usage import` (§ below) reads a host's own session record instead of receipts.

## Delivered-token economics

Approved 2026-10-03 (token-economics spec pass); implemented locally 2026-10-04. The
owner's top priority is that Foundry tools displace grep/ripgrep and exploratory file
reads at the fewest delivered tokens, with deterministic compression only. Implementation: 001
T004–T006 (wire, ranking, outlines), 003 T005 (catalog, hook, usage import, evidence)
and 007 T001 (multi-root). The wire rules belong to the
[shared context contract v2](../../001-source-state-recovery/contracts/context-v2.md);
this section owns the allowance rule above, the catalog ceiling, usage import, payload
measurement, targets and claim labels.

### Evidence behind the rules

- Source review on 2026-10-03 (oh-my-pi `5b8d5b8`, pi `a276dab`, opencode `907b3bc`):
  each host forwards an MCP text block unchanged while it stays under the host's own
  limits (OMP `packages/coding-agent/src/mcp/tool-bridge.ts` lines 211–227). Above
  them the host intervenes: pi middle-cuts MCP text over 20 KiB and saves the full text
  to a file (`packages/coding-agent/src/extensions/mcp/tools.ts` lines 46 and 118–143);
  opencode truncates at 2000 lines or 50 KiB (`packages/opencode/src/tool/truncate.ts`
  lines 14–15, applied at `packages/opencode/src/session/tools.ts` line 196); OMP
  subjects text blocks to its spill and byte cap (`tool-bridge.ts` lines 233–241,
  `packages/coding-agent/src/tools/output-meta.ts` lines 493–500). The counted text
  block is therefore Foundry's delivered payload, not host or provider cost. OMP also
  echoes `structuredContent` as an extra fenced JSON block unless it is already in the
  text; host compaction drops or rewrites old tool results; every request re-sends the
  tool catalog.
- v1 measured on 2026-10-03 ([validation](../../../docs/validation.md)): `search` at
  `limit` 10 cost 1337–4640 tokens per identifier query and 5934–6803 per subsystem
  question, each hit carrying its whole storage block of at most 2048 bytes; the
  5-tool catalog cost 711 tokens. The minimum v1 context envelope was 199 tokens
  (2026-10-01 allowance run).
- prakarana at `ae159e8500310070937db33a62608c30cdadc7ed`: tool payloads were 76.0%
  of the weighted token ledger and a displaced `read` also saved a whole turn
  (`src/internal/dwar.cpp` lines 101–105, measurement T008 over 24 runs); per-entry
  citation metadata exceeded compression gains at small entry counts
  (`docs/measurement/m58-adoption/README.md`: raw 1783 → wire 1928 tokens on a 5-entry
  probe); ADR-0052 (`docs/adr/0052-subtract-rakshak-and-sankocha-theater.md`) kept the
  net-cost-gated deterministic lanes — the code skeletonizer, whitespace-only JSON
  crush, log-line collapse and the compile-time fidelity gate — removing the ML prose
  lane and dead surface.

### Rules and owners

| Rule | Normative owner | Economic reason |
| --- | --- | --- |
| Search, context and retrieve successes are v2 text; status, index, errors, connect, usage and export outputs are unchanged; no `structuredContent` | v2 § Wire v2 | No second JSON escaping and no duplicated fenced JSON in OMP |
| One header line; default segments omitted; v1 metadata fields removed | v2 § Header line | v1's minimum envelope was 199 tokens; the target is at most 40 |
| Fenced verbatim or outline bodies, one-line graph items, a `next:` line | v2 § Evidence items | Source travels unescaped; outlines replace bodies under budget |
| Search locator lines with delivery-unit handles and excerpts of at most 160 bytes | v2 § Search locator lines | A hit costs one line instead of a 2 KiB block |
| Count the final text block or complete stdout; cap the serialized result at 256 KiB | v2 § Counting boundary | Count exactly what Foundry delivers; hosts forward it unchanged only below their own limits |
| Search is budgeted (default 1024) and charged to the allowance | v2 § Budgets | Search was the largest v1 payload |
| One atomic reservation per response | This contract, § Deterministic budget policy | Concurrent calls cannot overspend; refusals never charge |
| Retrieve narrows with `lines` or `view:"outline"` | v2 § Retrieve views | Read only the needed lines or a skeleton |
| v2 handles carry `sha32` and `ws16` prefixes | v2 § Source handles | T004 fixture measurement: 33 rather than 94 o200k tokens per handle (68 versus 207 bytes), not a universal fixed cost |
| Deduplicate within one response only; no cross-call suppression; deterministic elision only | v2 § Deduplication, no cross-call suppression | Compaction drops earlier results, and a forced extra retrieve turn costs more than resending (prakarana T008) |

Catalog ceiling: the serialized `tools/list` result stays within 800 o200k tokens,
asserted by a test. It rises to 1000 when 005 T003 adds `references` (owner decisions
2026-10-04: first 900, then 1000 after measuring full schemas). The exact descriptions and instruction text are owned by
[003](../spec.md) (§ Catalog and instruction text).

### Usage import

`foundry usage import --host omp|codex --session FILE` (`src/usage.rs`,
`UsageAction::Import` in `src/adapter_cli.rs`) runs offline and opens no store or
provider. It reads at most 512 MiB with lines of at most 32 MiB; larger input is
refused with the existing `usage_input_too_large`, never truncated. Arithmetic is
checked u64: overflow is `usage_overflow` (exit 1). Output is one JSON object on stdout,
counters only:

~~~text
{"v":1,"host","host_version"|null,"session_sha256","recipe":"omp-v1"|"codex-v1","models":[…],
 "assistant_messages","usage_records",
 "provider_usage":{"input_tokens","cached_input_tokens","cache_write_tokens","output_tokens","reasoning_tokens","total_tokens"},
 "missing":{"<category>":<records lacking it>},"complete":bool,
 "tools":{"<name>":{"calls","result_bytes","result_o200k_estimate"}},
 "unattributed_results","unparsed_lines"}
~~~

`total_tokens = input_tokens + output_tokens`; cached, cache-write and reasoning tokens
are subsets and are never added again. `session_sha256` hashes the session file.
No content bytes are printed. Zero usage records is `usage_unavailable` (exit 1).
`result_o200k_estimate` is labeled an o200k estimate: the host's model may use another
tokenizer.

- **`omp-v1`:** OMP session JSONL records assistant usage as
  `{"type":"message","message":{"role":"assistant","usage":{input,output,cacheRead,cacheWrite,totalTokens,reasoningTokens,cost}}}`,
  where `input` excludes `cacheRead`. Per assistant message: `input_tokens = input +
  cacheRead + cacheWrite`, `cached = cacheRead`, `cache_write = cacheWrite`,
  `output = output`, `reasoning = reasoningTokens`. Lines with an entry `id` count once
  per id: identical repeats are deduplicated, and differing usage under one id is
  `usage_conflict` (exit 1). Categories missing from a record are counted in `missing`
  and make `complete:false`. `role:"toolResult"` entries join `toolCallId` to the
  assistant's tool call; a `write` to `xd://mcp__context_foundry_<op>` is classified
  once as `foundry.<op>`, ordinary writes stay `write`, and unmatched results count in
  `unattributed_results`.
- **`codex-v1`:** the last `event_msg` `token_count` with non-null
  `info.total_token_usage` is used once and never summed, because its values are
  cumulative: `input_tokens` (which already includes cached input),
  `cached_input_tokens`, `cache_write_input_tokens`, `output_tokens` and
  `reasoning_output_tokens` map to the normalized categories. The host version comes
  from `session_meta.cli_version`. Tool results are `response_item`
  `function_call_output`/`custom_tool_call_output` entries joined by `call_id` to
  `function_call`/`custom_tool_call` names.

### Payload measurement

The frozen corpus `tests/fixtures/.economics/` holds the exact bytes of
`src/{store,response,mcp,ingest,bootstrap,receipts,config,error}.rs` and
`docs/architecture.md` at `6bb81e6` (`git show 6bb81e6:<path>`). The directory is
hidden so dogfooding ignores it; tests copy it to a temporary root before indexing.

| Query | Kind | Expected unit |
| --- | --- | --- |
| `reconstruct_verified`, `pack_ordered`, `BudgetConfig`, `native_discovery_block`, `fit_prefix`, `take_context_id` | identifier | its definition |
| `stale handle rejected on retrieve` | question | `retrieve` (`impl Engine`) |
| `repair index quarantine marker` | question | `repair_index` |
| `inbound frame byte limit` | question | `MAX_INBOUND_BYTES` |
| `session allowance refund` | question | `refund` |
| `receipt deduplication conflict` | question | `ReceiptDedup` |
| `sweep unseen sources after complete scan` | question | `sweep_unseen` |

Permanent `tests/economics.rs`: each identifier's definition is search hit #1, and
retrieving its handle returns the whole unit verbatim within 2048 tokens; each question's
unit appears, verbatim or as a signature, in `context` at 2048. An `#[ignore]`
`payload_report` prints per-query tokens. The v1 baseline for the same queries is the
2026-10-03 table in [validation](../../../docs/validation.md).

### Targets

Recorded in [validation](../../../docs/validation.md) when measured: v2 search at most
20% of v1 search tokens per identifier query; v2 search plus unit retrieve at most 35%
of v1 search plus hit-#1 retrieve; a context header of at most 40 tokens (v1 floor
199); the expected unit present for 12/12 queries; `tools/list` at most 800 tokens. A
miss is investigated by root-cause analysis of the rendered output before any decision
changes; it is never tuned away.

### Claims

Measured: payload tokens at the counting boundary, and provider usage only from
`usage import` of real host records. T004 measured the `examples/workspace` handle
at 33 versus 94 o200k tokens; the earlier 29/85 handle estimate is superseded. The
serialized five-tool catalog measured 568 at T004, 598 after T005/T006 and 661 after
007 T001 (v1 711). The six-tool catalog with 008's `memory` measures 793, under the
800 ceiling. Estimated:
`result_o200k_estimate`. Unknown: provider cache behavior when usage categories
are missing; such runs support payload claims only. Real-host results of the
token-economics tranche are labeled "bundled Foundry adoption (v2 + instructions +
hook), n=1 per task/arm, these tasks only; not an isolated v2 causal or general savings
result".

## Acceptance and improvement claims

Functional cases: tiny budgets, exhaustion, duplicate/unknown receipts, cache categories,
final host-envelope overflow, timeout/retry and secret-free logs through one actual
host/version. MCP fixtures cannot establish host-hook capability. Full-request mode
must prevent a call after its final budget check fails; delivery mode labels its limit.

For an improvement claim, predeclare the task/correctness check and compare matching
source/graph/model/profile/budget, actual complete-request usage and latency. Include
extra embedding/policy work; show preparation/training cost separately with observed
amortization. Report cold and warm cache cases separately: fewer delivered tokens can
cost more when reuse is lost. Unknown provider usage supports a payload result only.
The token-economics real-host runs (003 T005) carry the bundled-adoption label above;
they are not an improvement claim in this sense.

Evidence belongs to 003/009/013's actual workflow. No new mandatory benchmark, fleet
ledger or autonomous spend controller is selected. Without host hooks or explicit
gateway routing, user-driven context and MCP delivery budgets remain supported.

## Owned model gateway — explicit additional scope

The owner's second answer requests both context control and forwarding/metering.
`foundry gateway --config FILE` is therefore an optional foreground Rust command in
the same crate. It never opens the source database, fetches context itself, launches
learning or rewrites prompts. The host requests context over MCP and includes it in
the provider request; the gateway admits/forwards that completed request. Source and
gateway processes have different side effects and credentials, not competing store owners.

Pinned profile v1 (owner decision 2026-10-04, replacing the earlier Codex/OpenAI
Responses target, which had no consumer): **OMP 18.6.0 on Z.ai `glm-5.3-flash`**
over OpenAI-compatible chat completions with SSE. Upstream origin
`https://api.z.ai/api/coding/paas/v4` (Z.ai's documented GLM Coding Plan base), path
`POST /chat/completions`. OMP's catalog entry `zai.glm-5.3-flash` uses that wire,
sends `stream: true` with `stream_options.include_usage`, and accepts a per-provider
`baseUrl`/`apiKey` override in `models.yml`. The host runs in a dedicated OMP profile
(`omp --profile <name> --model zai/glm-5.3-flash`), whose own
`~/.omp/profiles/<name>/agent/models.yml` carries
`providers.zai: {baseUrl: "http://127.0.0.1:<port>/v1", apiKey: <ENV_NAME>}`; OMP
resolves an `apiKey` naming a set environment variable to its value. A global
`~/.omp/agent/models.yml` override is out of scope: it would redirect every `zai` model,
including Anthropic-wire GLM sessions the gateway cannot serve. Other zai models,
Claude Messages, OpenAI Responses, ChatGPT/Codex subscription traffic, arbitrary
OpenAI-compatible servers and WebSockets are not claimed; other hosts use MCP. A new
protocol or host amends this contract for an actual consumer.

Configuration v1 keys: `v:1`, `port` (0..65535, 0 for OS-assigned), `upstream`, `model`,
`mode`, `credential_env`, `run_dir`, `log_bytes`. The pinned profile accepts only
`upstream = "https://api.z.ai/api/coding/paas/v4"`, `model = "glm-5.3-flash"` and
`mode = "meter"`; `enforce` refuses at startup with `gateway_feature_unsupported`
(below), and `limits` are refused. Pin the
model's supported input/output windows in the installed protocol profile; an unknown
model fails startup. Model IDs/env names <=256 bytes, run_dir <=4096 bytes; variable
names match `[A-Za-z_][A-Za-z0-9_]*`. Config holds names/paths, never secret values.
Unknown/null fields refuse; config <=64 KiB,
log cap 64 KiB..16 MiB. Receipt-file output is opt-in: `log_bytes` is omitted when
disabled. Bind only
127.0.0.1; no public/LAN listener, remote bind or transparent interception. One gateway
run is one accounting session, even across HTTP reconnects. A fresh run has a fresh
session ID and token; any receipt-log cap is storage-only. Restart is not durable
monthly spending enforcement.

Startup generates a 256-bit random local bearer token in a newly owned mode-0600
file within a mode-0700 run directory. The selected host reads that token through an
environment reference; printed configuration includes names/paths, never secrets.
The gateway reads the upstream key once from the configured environment and removes
it from any child environment; it spawns no model/tool children. No key in argv,
config, source store, receipts, debug logs or model-worker grants. Client local bearer
tokens are validated before body processing and replaced for upstream authentication;
caller-supplied authorization/organization/routing headers cannot choose credentials.
Reject browser Origin headers, enforce the exact loopback Host/port and disable CORS.
This protects the endpoint from accidental/browser use, not a hostile same-user process.

The supported launcher owns a fresh, non-default OMP profile and refuses an existing
profile rather than overwriting it. Before launching OMP it verifies that the
profile's effective `zai/glm-5.3-flash` endpoint is the generated loopback URL and that
the local-token environment variable is nonempty and holds this run's token; a
missing, unreadable or invalid profile, a missing token or a failed authenticated
`GET /health` aborts the launch. OMP receives no upstream Z.ai credential through its
environment, profile login storage, auth broker or `--api-key`; the launcher does not
pass those credential routes on (an OMP registry that silently drops the override
would otherwise reach Z.ai directly). The profile configuration stays intact for the
host's lifetime; the host is stopped before cleanup, and a failed routed run is never
resumed against the bundled upstream endpoint. The supported profile is single-flight:
automatic title generation is disabled (`PI_NO_TITLE=1`) and no concurrent
side-model or subagent calls use this gateway; this is a declared compatibility
restriction.

Only `POST /v1/chat/completions` and authenticated `GET /health` are exposed initially.
The health reply contains readiness/session/mode, never credentials. Other paths,
methods and upgrades refuse locally; query strings cannot select an upstream. TLS
verification stays enabled; disable redirects and ambient proxy variables. Allowlist
upstream request headers required by the pinned protocol; strip all Foundry-only
correlation/auth headers. Forward only allowlisted response headers. For a
non-success upstream response, return a bounded gateway-owned error code and status
with the approved provider request ID, not the raw upstream error body or headers;
for an upstream SSE error, close the stream and record the failure without forwarding
raw diagnostic payloads, and never synthesize success. Neither the upstream key nor
the local bearer may appear in an error delivered to OMP; synthetic credential
canaries exercise these paths. Use maintained Rust HTTP/TLS/SSE libraries, not a new
HTTP parser.

At most eight HTTP connections with a 5-second header deadline; excess connections
close before allocating request bodies. One active generation including body reading
and validation, zero queued generations; acquire that slot after header
authentication, before body allocation, and hold it through terminal observation and
receipt finalization, including any transition to admission-closed after a log
failure. Excess is 429 `gateway_busy` with the response header `rate_limit_type:
max_parallel_requests`, which OMP 18.6.0 treats as an admission refusal and does not
retry inside its HTTP transport; a busy refusal sends nothing upstream and is not
billed usage. Bound headers to 16 KiB, request JSON to 4 MiB, decoded depth to
64, stream event to 1 MiB, response total to 64 MiB and unread forwarding buffer to
256 KiB. Slow clients apply backpressure and share a 60-second idle timeout; the
whole attempt including body reading and validation has a 600-second deadline. JSON
duplicate keys, unsupported compression and malformed UTF-8 fail before upstream
send. Disable transparent decompression or apply the same decoded-byte limits. Limits
are failure bounds, not throughput/latency promises. They are explicit compatibility
constraints.

The first accepted request subset is a streaming chat completion for the configured
model: `stream: true`; `messages` with system/user/assistant/tool roles, text content
or role-specific text-part arrays, and nullable assistant `content` alongside
`tool_calls`; function `tools`, `tool_choice` and tool results; replayed
`reasoning_content`; and an explicit top-level field allowlist (`max_tokens`,
`stream_options`, sampling, thinking/`reasoning_effort` and conditional `tool_stream`)
derived in T004 from the effective loopback-configured OMP model's request builder.
Extensions or extra-body overrides that change that payload are unsupported unless
included in the pin. After validation the original request bytes are forwarded
unchanged, preserving message order, text, tool catalog and reasoning content. Each
request replays the whole conversation; there is no server-side state to reference.
Refuse another model, `stream` absent or false, image/file/URL content parts and
top-level fields outside the pinned set with `gateway_feature_unsupported`, so hidden
charges are not mislabeled as metered. T004 verifies that OMP operates inside this
subset; a `baseUrl` setting alone is insufficient.

**Pinned OMP 18.6.0 request profile** (recorded 2026-10-04). Source: the installed
`omp/18.6.0` bundle (`pi-coding-agent/dist/cli.js`), not the 18.1.11 TypeScript
sources installed beside it. Method: actual OMP ran in a throwaway home against a
credential-free local capture endpoint. Sanitized shapes are in
`tests/fixtures/gateway/omp-18.6.0-*.json`.

- Wire: `POST <baseUrl>/chat/completions` with headers `accept: text/event-stream`,
  `accept-encoding`, `authorization: Bearer <apiKey>`, `content-type:
  application/json`, `content-length`, `host`, `connection` and `user-agent:
  omp/18.6.0`.
- Top-level fields: `model`, `messages`, `stream: true`, `stream_options:
  {include_usage: true}`, `tools`, `max_tokens` and `reasoning_effort`.
  - `max_tokens` is 131,072 on turns and 4,096 on judge side requests.
  - `reasoning_effort` is one of `low`, `high` or `max`. `--thinking`
    `off|minimal|low|medium` sends `low`; `high|xhigh` sends `high`; `max` and the
    catalog default send `max`.
  - Side requests add `temperature` and `tool_choice`.
  - The builder can also emit `top_p`, `tool_stream: true` (zai reasoning-effort
    dialect with tools) and `thinking: {type}`, though no capture showed them.
  - These ten names are the whole allowlist.
- Messages:
  - `system`: string content.
  - `user`: string or text-part array.
  - `assistant`: string content (`""` beside `tool_calls`; `null` is also accepted),
    with optional `reasoning_content` (replayed reasoning) and optional `tool_calls`.
  - `tool`: content plus `tool_call_id`.
- Windows: catalog `zai.glm-5.3-flash` has a 1,000,000-token context and `maxTokens`
  131,072. Its input modalities include images, which the subset refuses.
- Side requests: `--thinking auto` sends three judge side requests before the turn.
  The launcher therefore passes an explicit `--thinking` level (default `max`) and
  `PI_NO_TITLE=1`, which leaves one gateway request per model turn.
- Usage and stopping: OMP reads top-level `usage` or `choices[0].usage`. It stops
  reading at a choices-less chunk after finish and usage, or after a finish whose usage
  has positive cached tokens. It aborts 2,500 ms after finish.
- Retries: HTTP makes up to 6 attempts, with no retry after `rate_limit_type:
  max_parallel_requests` or that marker in the body. The stream wrapper adds up to 2
  empty-completion retries and 1 provider-error retry.

**Meter mode** forwards supported requests and records actual usage; it promises no
preflight input/session token cap. **Enforce mode** is defined but not available for
the pinned profile: it requires a positive `max_tokens` no greater than policy and,
before generation, a provider-documented count for the identical model, messages,
tools and thinking fields, reserved atomically with the maximum output against the
run's cap. Z.ai's documented tokenizer (`/api/paas/v4/tokenizer`) lists other models,
sits outside the coding-plan base and omits generation fields, so no contract-grade
counting endpoint has been verified for this profile; locally counted tokens are not
the provider's count. Until a
counting API is verified for the configured model, `mode: enforce` refuses at
startup. Meter mode is never an automatic fallback from enforce mode. The final
usage object is authoritative for observation: `prompt_tokens` is input with
`prompt_tokens_details.cached_tokens` as its cached subset, and `completion_tokens`
is output including reasoning; reasoning and cache-write categories are not reported
separately and stay unknown, never zero. A host-side zero for an absent category (OMP
defaults missing numbers to zero) does not make the gateway's category known.

Forward SSE without collecting a complete stream in memory. Preserve chunk order,
tool-call deltas and `finish_reason`, and forward `[DONE]` when received and the
client is still connected; never require the client to consume it as proof of usage.
Usage comes from the usage-bearing chunk (the final chunk or a trailing usage-only
chunk before `[DONE]`), never from counting visible deltas. Failed or incomplete
responses can consume tokens. Missing usage remains unknown, not zero. A disconnect,
truncated delivery or local failure does not erase a valid terminal usage observation
already received; the delivery outcome is recorded separately while known counts are
kept. OMP 18.6.0 stops reading after a trailing usage-only chunk, or after
`finish_reason` with positive cache information, and waits only 2,500 ms after
finishing for usage; T004 pins and tests both behaviors. Before terminal usage is
observed, a disconnect or failure cancels/closes upstream best-effort and records
unknown usage; the gateway never retries automatically. After response headers are
sent, local failure closes the stream and records failure; it cannot forge a
provider-completed event. No raw error/prompt body in local diagnostics.

Every new upstream generation attempt gets a gateway request UUID and is charged
separately, including host retries of identical JSON. A repeated receipt for that same
attempt is deduplicated. The gateway does not implement exactly-once generation or
pretend that closing TCP cancelled billed work. OMP 18.6.0 has layered retries: up to
six HTTP attempts per transport invocation, up to two empty-completion retries and one
provider-error retry in its stream wrapper, plus host recovery paths. These are not
configurable and are not a session-wide attempt bound, and the stream wrapper discards
retried attempts from the session it persists. The gateway records every request that
reaches it and every actual upstream send; a local admission refusal is not an
upstream attempt. Each known attempt is recorded in memory before it is sent, so a
later logging failure keeps its counts. Optional
`X-Foundry-Context-Ids` (<=4 KiB, <=64 UUIDs) provides attribution only;
strip it upstream. Missing IDs leave attribution unknown without losing usage counts.
Such IDs exist only once a host-request integration emits them.

Receipts use the schema above with the gateway's session/request IDs and observation
object. Keep counters
in memory; optional bounded private JSONL records counters, timing, mode and provider
request/response IDs, never bodies. Capacity checks and admission-close decisions
happen before another upstream send is permitted: admission stops before the
configured receipt log cannot fit another <=16 KiB result including its newline. A
post-send disk error keeps the attempt's known counts in memory, records the failure
and stops further admission; it cannot undo spend. No automatic log rotation or source
ledger writes. On shutdown, admission closes immediately, pending connections and
active upstream work are cancelled, and best-effort receipt handling finishes within
five seconds of the signal without extending any earlier request deadline. A crash
can lose an in-memory attempt entirely, so receipt-only reports always state that
whole-run coverage is unverified and may omit crash-lost attempts, even when every
surviving receipt is complete; there is no durable cross-restart cap. This is a
deliberate first-cut limitation, not a reason to invent another transactional request
ledger. Cleanup removes only this run's token and generated configuration, never
receipts or OMP session evidence.

In-memory receipt deduplication is bounded to 10,000 attempts per run, consistent
with offline summary limits. Refuse the next admission as `session_full`; no eviction
that could double-count a later duplicate. `run_dir` must be new and owner-private;
restart uses a new directory/session and never clears an existing run implicitly.

The learned head has no access to keys/HTTP and cannot raise limits. Model switching,
cached-answer serving, prompt rewriting and learned spend allocation are outside this
first gateway. Preserve useful future economics goals through observed evidence; do
not promise savings merely from installing a forwarding hop.

### Implemented surface (T004, 2026-10-04)

**Commands.** `src/gateway.rs` and `src/gateway_launch.rs` are unix-only.
- `foundry gateway --config FILE` is the foreground server.
- `foundry gateway-omp --config FILE --key-file FILE --profile NAME [--thinking low|high|max] [--omp PATH] [-- OMP_ARGS]`
  is the launcher.
  - `--thinking` defaults to `max`, the catalog default. It is always passed
    explicitly, so OMP never runs `auto` and its judge side requests.
  - `run_dir`'s parent must exist; a failed startup removes only the directory it
    created.
  - Test-only overrides exist only under the `test-faults` feature:
    `FOUNDRY_GATEWAY_TEST_UPSTREAM` (plain-http loopback upstream),
    `FOUNDRY_GATEWAY_TEST_IDLE_MS` and `FOUNDRY_GATEWAY_TEST_DEADLINE_MS`. Release
    builds read none of them.

**Local refusals and gateway errors.** Every row but the last refuses before any
upstream send. Each carries a bounded `{"error":{"code","message"}}` body.

| Condition | Status | Code |
| --- | --- | --- |
| `Origin` present; wrong `Host` | 403 | `gateway_forbidden_origin`; `gateway_bad_host` |
| Missing or wrong bearer | 401 | `gateway_unauthorized` |
| Admission closed (shutdown, log full, log write failure) | 403 | `gateway_admission_closed` |
| 10,000 attempts already recorded | 403 | `session_full` |
| Generation slot busy | 429 + `rate_limit_type: max_parallel_requests` | `gateway_busy` |
| Body over 4 MiB | 413 | `request_too_large` |
| Content encoding other than identity | 415 | `gateway_feature_unsupported` |
| Wrong content type; malformed JSON, duplicate keys, depth over 64 or invalid UTF-8; query string; invalid `X-Foundry-Context-Ids` | 415 / 400 | `invalid_argument` |
| Unknown path; wrong method; `Upgrade` header | 404 / 405 / 400 | `gateway_feature_unsupported` |
| Outside the pinned subset (model, stream, fields, image parts) | 400 | `gateway_feature_unsupported` |
| Body read deadline; attempt deadline; upstream silent before response headers (idle or deadline) | 408 / 504 | `deadline_exceeded` |

403 is used for permanent local refusals because OMP 18.6.0 retries only statuses
of 500 and above, 408 and 429.

**Upstream replies.**
- A non-success upstream status, 3xx included, is returned with the same status and a
  gateway-owned `upstream_error` body carrying `upstream_status` and an allowlisted
  provider request ID. Redirects are never followed.
- A 2xx reply must be `text/event-stream` with no content encoding; anything else is a
  502 `upstream_error`.
- An event is forwarded unchanged only when two conditions hold:
  - every line is blank, a `:` comment, or one of the fields `data`, `event`, `id` and
    `retry`;
  - if the event has data, that data is `[DONE]` or a duplicate-key-free JSON object
    with a `choices` array. A data-less event of comments or metadata fields, such as
    `: keep-alive`, forwards.
  A stream-leading UTF-8 BOM is ignored for this check and forwarded as received. A
  non-null top-level `error`, any other line or data shape, or a duplicate key ends the
  stream with that event withheld and outcome `failed`. A local failure or timeout
  flushes already-forwarded events, then closes the connection.

**Final summary line** (stdout of `gateway`; stderr of `gateway-omp`):
- `attempts`, plus `complete`/`failed`/`unknown`;
- known `input_tokens`/`cached_input_tokens`/`output_tokens` totals, `null` when no
  attempt reported a category;
- per-code local `refused` counts;
- `log_failed`, `totals_overflowed`, and `coverage:"whole_run_unverified"`.

**Launcher refusals** (exit 2 for invocation errors, 1 otherwise; OMP never starts):
- `invalid_argument`: a bad or `default` profile name, or a trailing OMP argument that
  would choose credentials, routing, models, thinking or extensions. Covered:
  `--api-key`, `--profile`, `--alias`, `--model`, `--models`, `--provider`, `--smol`,
  `--slow`, `--plan`, `--prewalk`, `--prewalk-into`, `--plan-yolo`, `--plan-yolo-into`,
  `--thinking`, `--config`, `--extension`/`-e`, `--hook`, `--plugin-dir`,
  `--external-thinking` and `--service-tier`, each in both `--flag value` and
  `--flag=value` forms.
- `profile_exists`;
- `profile_invalid`: the generated `models.yml` is missing or differs;
- `token_missing`;
- `gateway_unavailable`: no ready line, or the authenticated `/health` failed;
- `omp_unavailable`;
- `cancelled`.

**Signals.** The launcher forwards SIGTERM and a *directed* SIGINT to OMP.
Directed means sent by another process: `si_pid != 0` on macOS, `si_code <= 0` on
Linux. A terminal Ctrl-C already reaches OMP through the shared foreground process
group, so it is not forwarded a second time. After OMP exits, the launcher stops the
gateway and removes only the generated `models.yml`, then exits with OMP's status.

`session_full` is verified at unit level; the process cannot lower the 10,000 cap.
