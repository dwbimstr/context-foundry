# Validation

## 001 T004 local closure — final4

001 T004 is locally implemented and verified, **unreleased**. The owner reports that
all six final4 gates exited 0 on the unchanged source manifest retained at
`/private/tmp/cf-t004-gates-final/manifest.txt`; logs are in that directory. The same
existing `T004CutoverReview` (OpenAI `gpt-6.1-sol:xhigh`) returned **SHIP** after its
reported finding/delta closures. The author lane is Anthropic. This is the existing
in-session cross-lab review, not a fresh independent review. This docs-only closure
does not rerun or independently re-certify the six checks.

| Reported final4 gate | Exit |
| --- | ---: |
| `cargo fmt --check` | 0 |
| Locked all-targets clippy, warnings denied | 0 |
| Locked full test run, no fail-fast | 0 |
| Rust 1.90.0 check | 0 |
| Rust 1.90.0 clippy | 0 |
| CLI smoke | 0 |

The retained MSRV version log names rustc/cargo 1.90.0 and clippy 0.1.90. The retained
smoke shows index/search and `retrieve --lines 1-3` succeeding with v2 text and exact
`parse_record` bytes. Its negative `--lines 4-6` returns `invalid_range`, exit 2,
as invalid input under the existing CLI contract; that is not a changed exit rule
or a failed smoke gate.

| Measurement | T004 v2 | v1 comparison | Boundary and scope |
| --- | ---: | ---: | --- |
| Handle on `examples/workspace` | 33 o200k tokens; 68 bytes | 94 tokens; 207 bytes | Isolated v2 handle versus v1 JSON handle; fixture-specific, superseding the 29/85 estimate |
| Five-tool `tools/list` | 568 o200k tokens | Historical baseline: 711 | Serialized catalog; the 800-token ceiling is met |

These are handle/catalog measurements, not the complete twelve-query payload
comparison, provider usage, a universal per-handle cost or host-session savings.
T005 has slice-1 work in progress but no search integration or whole-task acceptance;
T006, remaining 003 T005 work and 007 T001 remain unimplemented.

**Reviewed ambiguity limitation:** a valid path embedding a complete handle suffix
and valid item tail can give an item line two complete readings. The test parser
`testkit::parse_v2` refuses both without attributing either handle; production
rendering and direct retrieve are unaffected. No universal item-line round trip is
claimed. The required 4096-byte special-path CLI/MCP/core cases passed in final4.
Zero-allowance `budget_exhausted` uses an outcome-free sufficient budget, not a
proved minimal budget, as the existing v2 refusal-floor contract allows.

## Token-economics v1 payload baseline — 2026-10-03

This is historical v1-era evidence. Its frozen queries and byte ranges describe
`6bb81e6`, including the subsequently deleted `take_context_id`; they are not
claims about current symbol availability.

Phase 0 capture for the approved token-economics tranche (001 T004–T006, 003 T005,
007 T001), taken on baseline `6bb81e6` before any code change, on the local macOS
arm64 workstation. One run of a temporary `#[ignore]` test appended to `tests/mcp.rs`,
reusing that suite's real rmcp client helpers:
`cargo test --locked --test mcp -- --ignored --nocapture v1_payload_report` → 1 passed,
65 filtered out, 2.93 s. The test was then removed byte-exactly (`tests/mcp.rs` is back
to its 4686 baseline lines at capture time, with no diff against `6bb81e6` then).
One observation per cell; no repetition or variance claim.

- **Corpus:** the frozen nine-file economics corpus, exact bytes at `6bb81e6` of
  `src/{store,response,mcp,ingest,bootstrap,receipts,config,error}.rs` and
  `docs/architecture.md`, indexed as one workspace from a temporary copy.
- **Boundary:** exact `o200k_base` `encode_ordinary` count of the MCP text-block
  content Foundry emits, which the
  [v2 contract](../specs/001-source-state-recovery/contracts/context-v2.md) counts.
  It is the delivered payload, not host or provider cost: hosts forward it unchanged
  only below their own size limits.
  v1 itself budgets the whole serialized `CallToolResult`, including a second JSON
  escaping, so a 2048-token v1 request delivers fewer text-block tokens than 2048.
- **Requests:** `search` with `limit` 10; `retrieve` of search hit #1's handle with
  `tokens` 2048; `context` with `tokens` 2048. No request returned an error.
- **Catalog:** serialized `tools/list` catalog, 5 tools: 3044 bytes, 711 tokens.

| Query | Kind | Search hits | Search tokens | Retrieve hit #1 tokens | Context tokens | Hit #1 v1 block (bytes) |
| --- | --- | ---: | ---: | ---: | ---: | --- |
| `reconstruct_verified` | identifier | 4 | 2651 | 748 | 1457 | `src/store.rs` 52648–54672 |
| `pack_ordered` | identifier | 3 | 2056 | 750 | 1498 | `src/response.rs` 8077–10069 |
| `BudgetConfig` | identifier | 7 | 4640 | 794 | 1567 | `src/config.rs` 0–2028 |
| `native_discovery_block` | identifier | 2 | 1337 | 786 | 1439 | `src/bootstrap.rs` 20047–22048 |
| `fit_prefix` | identifier | 3 | 2079 | 801 | 1808 | `src/response.rs` 14136–16142 |
| `take_context_id` | identifier | 2 | 1382 | 780 | 1483 | `src/mcp.rs` 38555–40586 |
| `stale handle rejected on retrieve` | question | 10 | 6483 | 748 | 1478 | `src/store.rs` 58701–60738 |
| `repair index quarantine marker` | question | 10 | 6069 | 689 | 1853 | `src/store.rs` 64784–66820 |
| `inbound frame byte limit` | question | 10 | 6478 | 796 | 1836 | `src/mcp.rs` 4061–6076 |
| `session allowance refund` | question | 10 | 5934 | 732 | 1784 | `src/mcp.rs` 12188–14200 |
| `receipt deduplication conflict` | question | 10 | 6803 | 767 | 1460 | `src/receipts.rs` 14222–16240 |
| `sweep unseen sources after complete scan` | question | 10 | 6328 | 800 | 1445 | `src/store.rs` 34366–36411 |

Byte ranges are half-open. Every hit #1 is one v1 storage block of at most 2048 bytes,
whatever the size of the matched code; this probe did not check whether that block
holds the expected definition. These rows are the comparison basis for the targets in
the [adapter economics contract](../specs/003-agent-retrieval-context/contracts/adapter-economics.md):
v2 search at most 20% of the identifier rows' search tokens, v2 search plus unit
retrieve at most 35% of search plus hit-#1 retrieve, a context header of at most 40
tokens against v1's 199-token minimum envelope (2026-10-01 allowance run below),
12/12 expected units and `tools/list` at most 800 tokens. Full-tranche payload/host
comparisons await the remaining tasks; T004's handle/catalog measurements are
recorded above. The historical v1 table remains unchanged.

## 001 + 003 first implementation — 2026-10-01

This section records the historical v1 wire and counting boundary; its envelope
fields and measurements are not descriptions of current T004 output.

Owner-approved scope: 001 T001–T003 and 003 T001–T003, including the optional shared
MCP owner for concurrent OMP/Codex sessions. Verified locally on macOS 26.5 arm64
(rustc 1.97.1) and in a Rust 1.90.0 Linux aarch64 container. Not published, signed or
packaged; no hosted CI run. No predecessor store, service or global host configuration
was touched; real-host runs used temporary public fixtures and fixture-scoped config.

| Check | Result |
| --- | --- |
| `cargo fmt --check` | Pass |
| `cargo clippy --locked --all-targets -- -D warnings` | Pass |
| `cargo build --locked --all-targets` | Pass, 0 warnings |
| `cargo test --locked --no-fail-fast` | Pass: 157 tests (unit 10, cli 13, core 20, integrity 15, laya_protocol 3, mcp 65, repair 8, response 8, scan_faults 15) plus 12 child-process recovery scenarios |
| `cargo build --locked --release` | Pass; SHA-256 `c36d0fb3975168b115309946954f8f673dc8252e2682420fae4865373c62c996`; 0 fault-hook strings |
| Rust 1.90.0, Linux aarch64 (`rust:1.90-slim-bookworm`) | `cargo check --all-targets --locked` pass; core 20, integrity 15, recovery, repair 8 and scan_faults 15 pass, including the non-UTF-8 filename branch APFS cannot create |
| README commands on the release binary | Pass, including `store_not_found` on a read before `index` |

Fault and recovery tests use named fault points behind the non-default `test-faults`
feature (enabled for tests by a self dev-dependency) and a separate `foundry-faults`
test binary. The shipped `foundry` binary has no arming path in any build; release
builds contain no fault-point strings. `--all-targets` debug builds contain four inert
fault-point names because Cargo unifies the test feature there.

**Real hosts (003 T003),** shared HTTP owner, fixture `tests/fixtures/agent-task`, host
instructions printed by `connect`. The host runs below used release `8968e70b…`; the
only later v1-era change added the counted `budget_limited_by` JSON-envelope field
and limiter-named refusals, rechecked by the mcp suite and the allowance run on `c36d0fb3…`:

- OMP 18.4.9 (model `zai/glm-5.3`), five runs: every first eligible discovery call was
  Foundry `search`, with no grep or ripgrep before it; the agent edited `parse_record`,
  the checker passed 4/4, it re-indexed through MCP and the pre-edit handle returned
  `stale_handle` (revision 6). An exact byte-pattern request used host `grep`
  (permitted). With the owner stopped, OMP's own diagnostic named `failed to connect`
  and the agent fell back to host tools. In both fallback cases the agent's reply did
  not restate the reason. Instruction-based preference only; no enforced routing hook.
- Codex CLI 0.159.2 (`gpt-6.1-sol`): native Streamable HTTP attachment (negotiated
  2025-06-18), tool listing, `search` and `retrieve` succeed. Alone, the discovery task
  listed the directory, then used Foundry `search`/`retrieve` and cited
  `src/records.rs:5`. Codex requires MCP tool approval by default; without the printed
  approval configuration non-interactive runs hang or refuse every call.
- Concurrent, same shared owner and fixture store: OMP performed the edit task while
  Codex answered a read-only discovery task. Their Foundry calls interleaved (OMP
  `search` 24.4 s, Codex `search` 31.1 s, OMP `retrieve` 35.5 s, Codex `retrieve` 44.9 s,
  OMP `index` 88.4 s); both exited 0, the owner exited 0 and the checker passed. Codex
  used only Foundry tools and cited `src/records.rs:5` and both callers. Three earlier
  concurrent attempts stalled Codex because the captain's harness never drained
  Codex's verbose stderr pipe; with stderr sent to a file the stall did not recur.
  Product code was unchanged by that harness fix. Evidence:
  [real-host record](review/real-host-t003-2026-10-01.json).
- Delivery allowance on a real host (release `c36d0fb3…`, OMP, owner launched from the
  printed `--budget` with `session_context_tokens: 500`): the first context request
  for 2000 tokens delivered with the v1 JSON fields `requested_budget: 500` and
  `budget_limited_by: session_allowance`; the next two were refused with
  `budget_exhausted` ("cannot fit within 73 tokens (limited by session_allowance);
  minimum 199 tokens"), and the unchanged 73 shows refusals are not charged.
  Host-request mode stays `budget_scope_unsupported`; neither host offers the hooks.

**Reviews.** Recorded session events, not self-reports: source review round 1 and
adapter review round 1 ran on OpenAI `gpt-6.1-sol`, cross-lab to the Z.ai GLM-5.3 and
Anthropic Sonnet 5.5 authors; source round 2 fell back to Sonnet 5.5, same lab as that
revision, so a final OpenAI pass (`codex exec`, `gpt-6.1-sol`) covered the integrated
tree. Its three new defects were fixed; it could not bind HTTP sockets in its sandbox,
so HTTP behavior rests on the native test run above.

**Accepted limitations.** Names are enumerated by path while bytes are read through
the held root: a same-user process that swaps and restores a directory during one scan
can retire records of still-present files until the next scan (001 records the
re-entry). The 16-handler cap is proven at its admission owners, not with 16 truly
stalled handlers (one engine slot makes that unreachable). Cancellation-registration
ordering is fixed by construction without an interleaving test. Several MCP tests
observe timing and fail closed on an unusually loaded machine. One continuation test
was made full-suite-safe after a single unreproduced load failure.

## First prototype slice — 2026-09-28

Local validation on 2026-09-28, macOS, Rust 1.97.1. No predecessor build, benchmark
or live-store experiment was part of this validation.

| Check | Result |
| --- | --- |
| `cargo fmt --check` | Pass |
| `cargo clippy --locked --all-targets -- -D warnings` | Pass |
| `cargo test --locked` | Pass: 10 integration tests |
| `cargo build --locked --release` | Pass |
| Release CLI help → index → graph import → context on the bundled fixture | Pass: 2 sources, 1 manual edge, 439 context-text tokens under a 1024-token budget |
| Public documentation relative links | Pass |

Tests cover source replacement/deletion and stale-search rejection, reopen with
pending index work, rebuilding a missing search index across another restart,
graph producer isolation/hash validity/traversal bounds, UTF-8 token-budget
accounting, workspace binding, feedback consent/splitting, the CLI lifecycle, and
Laya response parsing/fallback through a real local HTTP transport fixture.

The HTTP fixture is not a model. No Laya inference, fine-tuning, candidate promotion,
provider billing or task-quality experiment ran. No million-file scale, power-loss
fault injection or cross-platform result is claimed. The configured Linux/macOS
CI and Rust 1.90 minimum-toolchain jobs have not run on a hosted runner yet.

Release executable SHA-256 for this local build:
`5589bc3018c4149e9bd62804a0eab13e27a5d91ba966fefea54ed69efc16da78`.
This is a local build fingerprint, not a signed distribution artifact.

## Subsequent feasibility, 2026-09-29

[Bounded feasibility results](review/feasibility.md) add actual sandboxed model,
gradient, vector-index and Rust 1.90 MCP probes. They live in a separate scratch-probe
crate and do not change the root product or retroactively complete its proposed specs.
No production test suite rerun was needed for these documentation/probe-only edits.
