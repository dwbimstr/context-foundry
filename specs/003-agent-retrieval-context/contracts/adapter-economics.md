# Adapter economics contract v1

Status: Delivery budgets and receipts implemented 2026-10-01 (003 T002); host-request
mode and gateway proposed (contract proposed 2026-09-29). Delivered-token economics
(§ below) was approved 2026-10-03: 001 T004's wire, atomic allowance and catalog text
are locally implemented and verified on final4, unreleased. T005 slice 1 is in progress,
without search integration or whole-task acceptance; T006 and remaining 003 T005 work
are unimplemented. Owned by
[003](../spec.md); core packing/identity remains in [001](../../001-source-state-recovery/contracts/context-v2.md).
This adds practical budget control, usage receipts and the owner's subsequently
requested optional model gateway. The gateway is a narrow Rust forwarding command, not
a fleet cost governor. Delivery allowances were exercised on a real host in 003 T003;
no gateway integration or savings result has been executed.

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
provider_request_id?,provider_response_id?}`; mode delivery/host_request/meter/enforce,
times nonnegative u64 milliseconds, optional provider IDs nonblank <=256 bytes.
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

Approved 2026-10-03 (token-economics spec pass); not implemented. The owner's top
priority is that Foundry tools displace grep/ripgrep and exploratory file reads at the
fewest delivered tokens, with deterministic compression only. Implementation: 001
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
asserted by a test; the exact descriptions and instruction text are owned by
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
at 33 versus 94 o200k tokens and the serialized five-tool catalog at 568 versus
the historical v1 711; the earlier 29/85 handle estimate is superseded. Estimated:
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

Initial protocol target: OpenAI Responses over HTTP JSON/SSE, one configured HTTPS
origin (`https://api.openai.com/v1`), one allowlisted model and one local host session.
Codex is the first host to verify because its official
[provider configuration](https://developers.openai.com/codex/config-reference/)
supports base_url, Responses and environment-supplied authentication. This is a
supported configuration surface, not proof of this gateway's compatibility. Pin the
actual host/version and accepted request schema in T004 before advertising support.
No claim to proxy ChatGPT subscription/OAuth traffic, Claude Messages, arbitrary
OpenAI-compatible servers, WebSockets or all installed adapters in this first cut.
Other hosts can use MCP; add a provider protocol only for an actual consumer.

Configuration v1 keys: `v:1`, `port` (0..65535, 0 for OS-assigned), `upstream`, `model`,
`mode` (`meter` or `enforce`), `credential_env`, `run_dir`, `log_bytes` and optional
`limits`. Enforce requires limits `{max_input_tokens,max_output_tokens,
session_provider_tokens?}` with positive u64 values; meter rejects limits. Pin the
model's supported input/output windows in the installed protocol profile; an unknown
model fails startup. Model IDs/env names <=256 bytes, run_dir <=4096 bytes; variable
names match `[A-Za-z_][A-Za-z0-9_]*`. Config holds names/paths, never secret values.
Unknown/null fields refuse; config <=64 KiB,
log cap 64 KiB..16 MiB. Receipt-file output is opt-in: `log_bytes` is omitted when
disabled. Bind only
127.0.0.1; no public/LAN listener, remote bind or transparent interception. One gateway
run is one accounting session, even across HTTP reconnects. A fresh run has a fresh
session ID/token/cap; restart is not durable monthly spending enforcement.

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

Only `POST /v1/responses` and authenticated `GET /health` are exposed initially.
The health reply contains readiness/session/mode, never credentials. Other paths,
methods and upgrades refuse locally; query strings cannot select an upstream. TLS
verification stays enabled; disable redirects and ambient proxy variables. Allowlist
upstream request headers required by the pinned protocol; strip all Foundry-only
correlation/auth headers. Return supported response headers/status without leaking
upstream credentials. Use maintained Rust HTTP/TLS/SSE libraries, not a new HTTP parser.

At most eight HTTP connections with a 5-second header deadline; excess connections
close before allocating request bodies. One active generation including body reading
and preflight, zero queued generations; acquire that slot after header authentication,
before body allocation. Excess is 429 `gateway_busy`. Bound headers to 16 KiB, request JSON to 4 MiB, decoded depth to
64, stream event to 1 MiB, response total to 64 MiB and unread forwarding buffer to
256 KiB. Slow clients apply backpressure and share a 60-second idle timeout; the
whole attempt including preflight has a 600-second deadline. JSON duplicate keys,
unsupported compression and malformed UTF-8 fail before upstream send. Disable
transparent decompression or apply the same decoded-byte limits. Limits are failure
bounds, not throughput/latency promises. They are explicit compatibility constraints.

The first accepted request subset is stateless text/code, local function/custom-tool
definitions, call/result and supported reasoning items carried in input. Preserve
input order, text, tool catalog, cache keys and reasoning items verbatim. Require
explicit store=false. Truncation must be disabled or absent under the pinned API's
documented disabled default; reject auto rather than quietly changing behavior.
Reject conversation/previous_response_id,
background jobs, hosted tools, media/file/URL input and unsupported endpoint features
with `gateway_feature_unsupported`. These exclusions prevent hidden state/tool charges
from being mislabeled fully controlled. T004 must verify the selected host can operate
inside this subset; a base_url setting alone is insufficient. A later supported
extension amends this contract instead of adding a speculative backend framework.

**Meter mode** forwards supported requests and records actual usage; it promises no
preflight input/session token cap. It is never an automatic fallback from enforce mode.
**Enforce mode** requires a positive max_output_tokens no greater than policy. Before
generation, obtain a count for the identical model/input/instructions/tool schema
through the provider's documented
[input-token counting API](https://developers.openai.com/api/docs/guides/token-counting).
Pin/test its request-field projection against the accepted generation schema; every
input-affecting field must be represented or rejected. Do not treat locally counted
JSON/BPE tokens as the provider's exact input count. Count failure/unsupported item
means `budget_unverifiable` with no generation call. The extra provider preflight sees
the permitted input and adds network time; record it separately, with no unverified
claim that it is free. No cross-request prompt cache or request-body journal is added.

After count succeeds, reserve input plus max output atomically against this run's cap,
then forward the original request bytes unchanged. Reject budget overflow before
generation. Never silently reduce output, remove history, switch models or insert
context to pass a cap. The response's terminal usage is authoritative for observation;
include cached input as a subset of input and reasoning as part of output, without
double counting. A request cap is enforced under the provider's documented counting
and max-output behavior; it is not an independent guarantee of the provider's bill.

Forward JSON or SSE without collecting a complete stream in memory. Preserve tool-call
events/order and final provider status. Usage comes from the final JSON/terminal
[Responses event](https://developers.openai.com/api/docs/guides/streaming-responses),
never from counting visible text deltas. Failed/incomplete responses can consume tokens.
Missing usage, truncated stream or client disconnect means unknown usage, not zero;
cancel/close upstream best-effort, keep the full reservation, never retry automatically.
After headers are sent, local failure closes the stream and records failure/unknown;
it cannot forge a provider-completed event. No raw error/prompt body in local diagnostics.

Every new upstream generation attempt gets a gateway request UUID and is charged
separately, including host retries of identical JSON. A repeated receipt for that same
attempt is deduplicated. The gateway does not implement exactly-once generation or
pretend that closing TCP cancelled billed work. Set the verified host's automatic
request/stream retries to zero initially; if it still retries, each observed attempt
counts. Optional `X-Foundry-Context-Ids` (<=4 KiB, <=64 UUIDs) provides attribution only;
strip it upstream. Missing IDs leave attribution unknown without losing usage counts.
Such IDs exist only once a host-request integration emits them.

Receipts use the schema above with the gateway's session/request IDs and observation
object. Keep counters
in memory; optional bounded private JSONL records counters, timing, mode and provider
request/response IDs, never bodies. Admission stops before the configured receipt log
cannot fit another <=16 KiB result. A post-send disk error records an in-memory unknown
and stops further admission; it cannot undo spend. No automatic log rotation or source
ledger writes. A crash can lose the in-memory attempt; reports disclose incomplete
coverage and no durable cross-restart cap. This is a deliberate first-cut limitation,
not a reason to invent another transactional request ledger.

In-memory receipt deduplication is bounded to 10,000 attempts per run, consistent
with offline summary limits. Refuse the next admission as `session_full`; no eviction
that could double-count a later duplicate. `run_dir` must be new and owner-private;
restart uses a new directory/session and never clears an existing run implicitly.

The learned head has no access to keys/HTTP and cannot raise limits. Model switching,
cached-answer serving, prompt rewriting and learned spend allocation are outside this
first gateway. Preserve useful future economics goals through observed evidence; do
not promise savings merely from installing a forwarding hop.
