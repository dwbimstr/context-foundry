# Bounded pilot v1

Version: **1** · Contract date: **2026-10-31** · Status: **policy frozen; execution locked**.
`execution_authorized=false`; `execution_ready=false`.
Critical contract author: **OpenAI GPT-6 Astra (`openai-codex/gpt-6-astra`)**.
Normative owner: `specs/003-agent-retrieval-context/contracts/bounded-pilot-v1.md`; T006 and handoff link here rather than duplicate policy.
“MUST”, “MUST NOT” and “SHOULD” are normative. Requirements below describe future authorized behavior, not implemented capabilities.

## 1. Authority, decision and claim ceiling

- The owner selected **USD10 calculated/reservation admission envelope, four matched pairs/eight coding runs, and at most one transport-only probe**.
- This freeze authorizes no implementation, model execution, embedding, provider request, credential access, fixture/compiler run, configuration change or commit.
- The decision this pilot may inform is whether real capture/admission works and yields interpretable paired correctness and aggregate-usage observations.
- The sample is **one already-used public `percent-encoding` 2.3.2 malformed-escape repair, repeated four times per arm**, not four independent tasks or held-out samples.
- A is ordinary read/edit without injected Foundry evidence; B uses the same task/profile/tools plus frozen Foundry evidence in the initial-user evidence slot.
- No significance, inferential confidence interval, noninferiority, causal savings, generalization, T006 acceptance, default promotion or invoiced-cost claim is permitted.
- This single contract solves stable, hashable handoff ownership; duplicating its values in mutable T006 prose is the rejected simpler-looking alternative. No new numbered spec, benchmark framework or proxy platform is introduced.
- The captain archives the integrated document and records its SHA-256 **outside its body**. Any policy revision requires a new version, digest, review and authorization.

## 2. Evidence roots and static pins

Private aliases identify existing read-only evidence, not future writable execution directories:
- `R` = `/Users/satishlomte/VSC_DEV/datasets/context-foundry-task-validation/paired-lFCMwZ`.
- `A` = `/Users/satishlomte/VSC_DEV/datasets/context-foundry-task-validation/apparatus-v2-DkGJgT`.
- `F` = `/Users/satishlomte/VSC_DEV/datasets/context-foundry-task-validation/pilot-freeze-v1-AU3BfH`.
- `P` = `/Users/satishlomte/VSC_DEV/datasets/context-foundry-g2-work/profile-default.json`.
- `FOUNDRY` = `/Users/satishlomte/VSC_DEV/context-foundry/target/release/foundry`; `OMP` = `/Users/satishlomte/.local/bin/omp`; `RUSTC` = `/opt/homebrew/bin/rustc`.

`C` means current static SHA supplied by the 2026-10-31 pin-evidence map; `H` means accepted historical evidence; `D` means declared pin, not rehashed in this freeze.

Mechanical static-pin lookup: `F/input-pins.json`; the normalized map resolves absolute paths and distinguishes selected pilot-model metadata from researcher identity. It does not replace the self-contained pins below or establish live execution.

| Input | SHA-256 | Basis |
| --- | --- | --- |
| `R/protocol.json` | `3e55bde1588953f40e6986b9d97c1404c31a2ec6cfdd5fe2d48bb038a6fcd3da` | C |
| `R/task.txt` | `d634da1c6b74ab5bc247f4457109742213880e051f9fa68fe81f62de3ce26c31` | C |
| `R/inputs/seed-lib.rs` | `bb97304a4602082df298e81d6ac0b500e8d6d89a525d8585a16c267ef6c84ac5` | C |
| `R/oracle/check.rs` | `f5ac14d370d852090c49ead61e23f4d9185eed491b33b123ba6e6ff2dec09bb9` | C |
| `R/oracle/control/Cargo.toml` | `e4ea043c47bbd5551fc5fcf8151eab94428b4be6f2337834a0f9020a4392fb19` | C |
| `R/oracle/control/src/ascii_set.rs` | `f75552e4f446ed240cef64c1367e848cf3ae65f77602085459566d835a1d8587` | C |
| `R/oracle/control/LICENSE-MIT` | `b38f11f6096706e6de553dabe2a7ed142d59b6fa8c97e290c67496154745cdd5` | C |
| `R/oracle/control/LICENSE-APACHE` | `a60eea817514531668d7e00765731449fe14d059d3249e0bc93b36de45f759f2` | C |
| Untouched upstream `src/lib.rs` | `641903d385581e76f31ef8c991d83c2a5d7fbbc2879d93909c539037e4e1e3e2` | D/H |
| Upstream crate archive | `9b4f627cb1b25917193a259e49bdad08f671f8d9708acfd5fe0a8c1455d87220` | D/H |
| `OMP`, version 18.13.2 | `38c3645283ceba06f66462830ae15f27270cc7f84575e40a4c6bd68452a63d0f` | C |
| `FOUNDRY`, release binary | `9dcbec8fdfff79d491410c82ab6e3ea922d3c19d5c9397f9d231fc08713c8614` | C |
| `RUSTC`, Rust 1.97.1 Homebrew | `0239590431f4f2dd0e8499f6502a08ebdb2848455efc2951620398dccb685bbf` | C/H |
| `P`, profile file | `cd3a5326579bc720f3ba1e6849be0e53ff9de3dc635eb51cd3212f4f13043e2a` | C |
| `F/allocation.json` | `fd3add3fc280d4a512a3c4fe58983a001849bd0ffe007efa26b62809c11bbf4a` | C |
| Accepted fixture controller `A/preflight.mjs` | `112d1aa19abd58ed1841388d3b69b75cc26376eb6455e440779e0e548ef675ec` | H |
| Accepted fixture guard `A/guard-extension.ts` | `d8999597340d3c5696c161bfc8ef9fe49c32028e68aa09dadc5d26dc6ebff05d` | H |
| Profile-declared embedding worker executable | `b0f7e02009ae31d622240469f9eddb4d580a170141f537e76e09e35ba31b0cb3` | D |
| Profile-declared `embeddinggemma-2-BF16.gguf` | `68bae29d62fb8c7d23e98d21fd4662753ddd636e6b62b8a70dfe059a9844f216` | D |
| Profile-declared `tokenizer.json` | `4d777ef5bdc1aa36227abdfb77c3e49e7b9c892d16e1b6bda41c393504828be4` | D |

- `P` is v2, `g2-head-gemma2-default`, 768-dimensional mean-pooled embeddinggemma-2 BF16, adapter revision 4, with a 4 GiB worker ceiling; its full descriptor remains authoritative.
- The worker and model-artifact pins above were not freshly rehashed here. Future admitted local preparation must establish their identity using accepted integrity evidence; this freeze grants no permission to run them.
- The seed corpus is the five-file control corpus with only `src/lib.rs` replaced by the pinned seed. The other four files remain protected.
- `A/final-disposition.json` and the accepted `preflight-runs/run-mvthtmxg-vpxs7/manifest.json` own fixture disposition; stale candidate/blocker fields in `A/status.json` do not override them.
- The accepted **25/25 scripted fixture result is not live readiness**. Preserve historical A/R artifacts and failed attempts unchanged; never silently upgrade a missing or mismatching admitted input.

## 3. Sampling and frozen context intervention

- The one pre-outcome allocation draw was `crypto.randomInt(6)=5` over six balanced schedules, recorded at `2026-10-31T14:58:54.825Z`; it is operator provenance, not external attestation.
- Execute pair orders **BA, BA, AB, AB**: slots `1B,1A,2B,2A,3A,3B,4A,4B`. Never redraw, reorder, replace a slot or add an arm.
- All eight slots use the identical task bytes from `R/task.txt` and initial five-file seed. Previous repairs, diagnostics and oracle feedback MUST NOT become later inputs.
- Before any new provider HTTP request, materialize and freeze **all four** pair-specific semantic views from their initial seed, using release Foundry, `P` and `--development-isolation`.
- The exact context query is `preserve malformed percent escapes without dropping bytes in iteration and borrowed UTF-8 conversions`.
- The delivery allowance is **2048 o200k tokens** under the existing profile/recipe; no retuning, summarizer, enrichment, additional source or post-outcome selection is allowed.
- Preserve the existing `Initial repository evidence:` initial-user framing. A has no injected evidence; B receives the frozen view verbatim in that designated slot.
- Each view’s private manifest entry binds bytes/SHA, exact delivery count, source hashes, canonical source root, store revision, recipe/profile and exact citations.
- Require `semantic:ready` and valid source/citation bindings for every view. Missing readiness, changed sources or failed bindings prevent live launch.
- Use one canonical owned workspace path per pair; restore it from the seed between arms. Archive the preceding arm before reset.
- Each arm has fresh private host/session/task-memory state. Each pair’s preparation uses a fresh isolated local store; archived view bytes remain separate from mutable arm state.
- No global profile, store, history or cross-arm task memory is admitted. The coding model cannot access preparation stores, other arms, private evidence or the checker.
- Pair-specific workspace/store IDs and view bytes are setup-derived facts under these rules, not discretionary policy changes. Pin them before the first provider request.
- No human or model feedback changes prompts, settings, tools, query, oracle or sampling between pairs.

## 4. Runtime comparability and natural cache condition

- The coding provider/model is **Anthropic `claude-sonnet-5-5`**, selected through OMP 18.13.2 as `anthropic/claude-sonnet-5-5`, with adaptive thinking, `xhigh` effort and standard service.
- No provider, model, effort, service-tier or endpoint fallback is permitted. Critical plan/gate review uses the approved Opus 5.5/GPT-6 Astra lane; the removed Anthropic-only author restriction stays removed.
- Both coding arms have identical **30,720 estimated-input admission threshold / 32,768 input reservation / 4096 inclusive-output cap / 600-second run wall**.
- These are comparable treatment constraints, not equal realized work or proof of equal difficulty. B’s injected context consumes its input allowance and may affect admissible followups.
- The 32,768 reservation is not a guaranteed hard input-token limit. The 4096 limit includes thinking; compatibility of this output cap with the selected adaptive/xhigh profile remains a live-readiness gate.
- These bounds define this pilot, not equivalence to the earlier USD5-per-arm calibration. Report truncation and cap exhaustion, never relax bounds to reproduce an earlier trajectory.
- Final serialized requests MUST advertise exactly native `read` and `edit`; no Prakarana, shell, build/test, network, delegation, server tools, advisors, images or MCP connector.
- Use replace edits with fuzzy editing disabled. Canonical read admission covers only the five corpus files; edit admission covers only `src/lib.rs`.
- Refuse URI/internal-device paths, escaping paths, symlinks and selector-shaped literal/symlink shadows; numeric selectors are admissible only when their actual native target is the admitted file.
- Disable ambient extensions, skills, rules, context files, title generation, LSP, PTY and prewalk; only the separately accepted scoped capture/admission implementation may be loaded.
- Within a pair, actual rendered base instructions, tool schemas, common task bytes and resolved settings MUST match. Only the declared evidence slot and explicitly recorded volatile metadata may differ.
- Cache treatment is **observed natural hosted cache**, not cold, reset or equally warm. Fresh HOME/local stores do not reset the hosted cache.
- Use ordinary ephemeral **five-minute caching** in both arms. Omitted TTL is allowed only with documented provider-default five-minute semantics; preserve absent versus explicit TTL.
- No one-hour cache, separate warmup, deliberate KV-cache manipulation, priority/batch service or account switching is allowed.
- Stable project instructions/catalog remain unchanged; mutable paths, session IDs, task/context material and telemetry stay outside the common reusable prefix.
- Hash the actual serialized stable-prefix bytes and cache-control layout; normalized text is not proof of prefix identity. Undeclared prefix/layout differences stop admission.
- Record cache counters, TTL/layout, order and elapsed gaps. The optional fixed-first probe may warm shared prefixes; report that possibility and its cost without claiming a cache-independent effect.

## 5. Transport, capture and request identity

- The existing fixture controller/guard cannot run live unchanged. A separately approved, reviewed and hash-pinned **scoped OMP final-request/transport capture and admission implementation** is required.
- Permit direct HTTPS only to `https://api.anthropic.com`: `POST /v1/messages`, `POST /v1/messages/count_tokens`, and optional declared `GET /v1/models`; no redirects, proxies or alternate services.
- All provider HTTP traffic, including startup metadata, errors, cancellations and attempted retries, must be observed and classified. Unknown routes, auth redirection or unobserved transport fail closed.
- Capture-ready acknowledgement, resolved launch-pin checks, durable attempt identity and reservation MUST precede generation transmission. A log/capture failure cannot fall back to an uncaptured request.
- There is at most one generation in flight. Disable automatic retries for every allowed HTTP route, or refuse retries before transmission; tool followups require new single-use generation permits.
- Bind each permit to the actual final serialized request digest after all hooks, its protected initial-user/task/context location, tools, model, settings, output cap and CountTokens input projection.
- CountTokens must inspect the actual final structured input using a pinned provider-compatible projection. Any later input mutation invalidates admission; do not recount or resend that consumed permit.
- Each actual HTTP attempt has a distinct local identity, route, timestamps, status and outcome. Preserve HTTP request ID, SSE message ID and native `responseId` mapping where applicable.
- Missing IDs remain unknown. Never join or deduplicate by counter equality, array position or native record count.
- Observe streamed usage without changing SDK bytes; require valid final usage, terminal evidence and completed observer/transport cleanup. Earlier deltas cannot substitute for an invalid actual final delta.
- Retain the existing bounded 1 MiB request-body and observed-SSE limits; exceeding either refuses or terminates observation, records the fault and prevents further paid requests.
- Capture whitelisted identity/usage/shape metadata and hashes only, retaining exact raw usage-number bytes; do not create raw request, response-body or thinking-text logs.
- Final outbound-byte evidence establishes the controlled client boundary, not independent access to provider internals or invoice truth.
- The optional probe is fixed first, one generation, same host/model/settings/catalog/capture and input/output bounds, with no tool execution or coding edit.
- Its exact instruction is: `Reply exactly PILOT_TRANSPORT_OK. Do not use tools, read files, edit files, or solve a coding task.`
- Probe success requires verified requested and returned provider/model identity and resolved settings, no tool use or coding effect, successful terminal evidence, complete primary four-category aggregate usage, unambiguous identity reconciliation and clean shutdown. Visible reply wording is not an acceptance criterion; do not add response-body or thinking-text capture. A failed transport/capture/admission criterion stops the session; no retry.
- A probe-only authorization never authorizes coding. A successful probe is transport evidence, not coding-quality, economics or full T006 evidence.

## 6. Frozen request, time and spending bounds

| Bound | Frozen value |
| --- | --- |
| Coding slots / matched pairs | 8 / 4 |
| Optional transport-only probe | At most 1, fixed first |
| Generation permits per coding slot | 6, including initial request and tool followups |
| Total generation permits / Messages attempts | At most 49 |
| CountTokens calls | At most 49; at most one per generation permit |
| Catalog GET | At most one per host invocation; at most 9 overall |
| Total provider HTTP attempts | At most 107 |
| Coding-slot admission envelope | USD1 = 1,000,000 microUSD |
| Probe admission envelope | USD0.50 = 500,000 microUSD |
| Whole-pilot admission envelope | USD10 = 10,000,000 microUSD |
| Frozen budget-model input / output rates | USD10 / USD25 per million tokens |
| Estimated input threshold / reserved input | 30,720 / 32,768 provider tokens |
| Inclusive maximum output | 4096 provider tokens |
| Full generation reservation | 430,080 microUSD = USD0.43008 |
| Coding-slot wall / generation or probe deadline | 600s / 120s |
| Local context preparation | 180s per pair; 900s total no-provider materialization |
| Independent post-check | 60s per coding slot |
| Paid-phase wall | 7200s |

- Failed/invalid attempts consume their respective caps; a consumed generation permit cannot be recycled. No hidden title/helper/warmup request, seventh permit, replacement arm or manual rerun.
- The 120-second generation deadline includes its counting/admission sequence; each metadata request must also terminate within 120 seconds and all enclosing deadlines.
- Coding wall covers host startup, counting, generation and tools through host shutdown; post-check has its separate 60-second ceiling.
- Paid-phase wall starts at the first authorized provider HTTP request and ends after final verification/cleanup; it includes probe, counting, failures, gaps and any intervening approval wait. Its clock never pauses.
- Local preparation before that boundary remains measured; pre-provider approval wait is separately named, not assigned zero or included in coding latency.

## 7. Reservation arithmetic and price provenance

- **USD10 is a calculated/reservation admission envelope, not an invoice guarantee, provider-account cap or promise against provider breaches, taxes, fees or unrelated activity.**
- The USD1 and USD0.50 subcaps remain binding; unused allowance does not permit larger arms or more requests. No automatic top-up or reallocation raises a subcap.
- Before live authorization, pin standard-service account/model/ratecard evidence showing every applicable input/cache-write/cache-read price is at most USD10/M and inclusive output at most USD25/M; otherwise HOLD.
- The old source catalog’s USD/M figures—input 2, output 10, cache read 0.2, five-minute write 2.5—are reference-only, not OMP 18.13.2 effective prices or a current account quote.
- CountTokens is documented as **free**, separately RPM-limited, estimate-producing and non-cache-priming; revalidate those properties and selected-model/client-tool compatibility at launch.
- A free counting call still consumes its HTTP/count permit. No chars/4 or o200k substitution, guessed zero price or retry is allowed.
- Admit a generation only when its valid CountTokens estimate is at most **30,720**; reserve **32,768**, a **2048-token margin**, plus all **4096** possible output tokens.
- This margin is conservative policy, not proof that the estimate cannot vary. Actual input beyond 32,768 or output beyond 4096 is a global `provider_limit_breach`.
- Full reservation is `(32768 × 10) + (4096 × 25) = 430080 microUSD`; all token/cost arithmetic uses checked integers or exact rational/BigInt intermediates with checked range and upward monetary rounding.
- Before counting/transmission, require `spent_at_budget_rates + outstanding_reservations + new_full_reservation` to fit both the slot/probe envelope and whole-pilot envelope.
- Reconcile once per actual generation attempt using observed inclusive input/output at frozen budget rates; release only the supported difference from that attempt’s reservation.
- Unknown, failed or lost generation responses retain the **full** reservation and halt further paid requests. Client timeout, cancellation or likely free provider rejection is not refund evidence.
- A conclusively pre-transmission refusal may release an unspent provisional reservation only with proof no generation bytes were sent; it does not restore a consumed request/permit cap.
- Report budget-model expenditure separately from actual-rate calculated charges. Actual-rate charges require the pinned ratecard and observed categories; bills, discounts, taxes and fees remain separately evidenced or unknown.

## 8. Pilot-only aggregate usage and unchanged native semantics

- This contract alone owns **`pilot-only live wire aggregate observation`**: a separately reported, identity-bound four-category transport observation for this pilot.
- This scoped label is not a delivered-token-tranche or general economic-acceptance claim and does not redefine the economics contract’s existing measured-usage/import label.
- Primary completeness requires valid raw **uncached input, cache-creation input, cache-read input and inclusive output**, successful terminal evidence and unambiguous attempt/response identity.
- `inclusive_input = uncached_input + cache_creation_input + cache_read_input`; `total = inclusive_input + inclusive_output`.
- Reasoning, when observed, is an optional output subset satisfying `0 ≤ reasoning ≤ inclusive_output`; never add it again or infer its count from thinking text.
- Input/cache contributors may come from valid message-start evidence; output and optional reasoning must follow the actual final usage semantics, without carrying omitted final fields forward.
- Parse raw integer lexemes losslessly and check the u64 domain, additions and aggregates. Unsafe JavaScript integers require tagged decimal preservation and BigInt processing, never rounding through `Number`.
- Missing, null, malformed, negative, fractional or overflowing counters are unknown/invalid, never zero. Any unknown primary aggregate retains its reservation and halts further paid requests.
- Preserve original private native sessions and complete raw import output bytes/hashes unchanged. The `omp-v1` five-category rule, entry-ID deduplication and recorded native arithmetic remain unchanged.
- Missing native reasoning still means native `complete:false`, even when the separately labeled wire aggregate is complete. Do not enrich or rewrite native sessions to manufacture completeness.
- Native duplicate usage-bearing entries are not additional provider requests or bills. Reconcile through independent identity mappings; retain documented representational duplication, and halt on unexplained conflicting evidence.
- Wire reports, native imports, delivery-token measurements and provider-attempt ledgers stay distinctly labeled. Do not sum o200k delivery/local-embedding tokens into provider totals.

## 9. Candidate, oracle and source binding

- Correctness is independently determined, never model self-reported: no-alloc and alloc compilation, the pinned external oracle, unchanged protected files and no out-of-scope effects are required.
- Reuse the accepted untouched-positive/seeded-negative control evidence; do not rerun the original pair/checker merely to reconfirm it. Changed relevant inputs invalidate that evidence and require separately approved replacement evidence before launch.
- Stop the coding host, freeze the candidate inventory/diff and hash every candidate source before copying it into a separate checker directory outside agent access.
- Bind each check to slot identity, candidate source hashes, oracle hash, compiler binary/version, exact argv/environment, generated artifact hashes, exit/signal/timeout and output hashes.
- Use the accepted command recipe in `A/preflight-runs/run-mvthtmxg-vpxs7/case-ctrl-oracle/positive/steps.json`, substituting only the private candidate/output root.
- Compile `src/lib.rs` as edition-2018 `records` library once without alloc and once with `--cfg feature="alloc"`; compile the edition-2021 checker against that candidate’s alloc rlib, then execute that checker.
- The oracle covers 131,598 byte cases, iterator/allocation agreement, unchanged borrowing, UTF-8 and encoding consumers; its source remains private and inaccessible to the coding model.
- The checker runs with a minimal no-credential environment and no network or host-account access. Candidate code is untrusted; executable checking requires the separately accepted containment boundary.
- Oracle failure, normal output truncation or a blocked forbidden-tool attempt is retained as behavior failure, not an excuse to regenerate. Checker infrastructure/provenance faults halt the session.
- Never check a stale build or different arm’s repair. Missing source/argv/output binding is a provenance fault, not a correctness pass.

## 10. Phase ledger, privacy and reporting units

- Record materialization/reset, index/prepare/query/local embedding, capture/counting, host generation/tools, independent checks, maintenance, cleanup, probe and every failed/invalid attempt.
- Primary per-slot observations are **instrumented host-only wall**, exact primary aggregate provider usage and independently bound correctness; record turns, tool calls, constraint violations and terminal reason.
- Capture instrumentation is common to both arms. Its overhead is not assumed zero or subtracted; full retrieval tracing is OFF, and no extra uninstrumented paid comparison is allowed.
- The secondary allocated pipeline subtotal assigns measured fresh preparation/query cost to B with **no default amortization**; shared setup/probe is separate.
- Report both allocated subtotals and actual whole-session wall, avoiding double-counting overlapping phases. Approval waits and operator/apparatus work remain visibly separate.
- Report observed local document/query embedding counts separately; unavailable query-token, energy or prior apparatus/operator-model costs remain unknown, not zero.
- Private directories/files use 0700/0600 permissions. Keep source, prompts, native sessions, oracle, credentials, model weights and raw evidence out of Git/public receipts.
- Whitelisted capture may retain raw usage fragments/IDs/hashes, never auth values or extra thinking logs. Original native sessions are retained as emitted under the private evidence policy.
- Admit credentials explicitly and minimally to the authorized host transport only; never expose them in argv/logs, compiler environments, local-model workers or task files.
- Pin the authorized account identity without publishing secrets; obtain approval for task-data rights and actual provider retention/training terms. This experiment grants no training consent or host security/account reconfiguration.

## 11. Stop rules and immutable outcomes

- Expected slot-local exhaustion of USD1, six permits or 600 seconds closes that slot as partial; do not send a seventh request/count call or replace the slot.
- The next scheduled slot may start only after every provider attempt is conclusively reconciled, owned activity is stopped and session bounds remain available. Unknown/in-flight/lost usage instead requires global halt.
- Normal `stop_reason=max_tokens` within 4096 is retained truncation/behavior failure; actual output above 4096 or inclusive input above 32,768 is a global breach, not ordinary truncation.
- Global halt applies to total budget/time exhaustion, probe failure, transport errors, unobserved/bypassing traffic, capture/append failure, unexpected retry, unauthorized route/settings/tool inventory, pin/source/identity/counter faults or unresolved cleanup.
- A failed, missing or malformed CountTokens response is a global counting/admission fault.
  A valid estimate above 30,720 is a pre-Messages input-cap refusal: close that coding slot as resource-limited; the next scheduled slot may start only after full attempt reconciliation and clean shutdown, with session bounds still available.
  For the sole compatibility probe, that refusal ends the probe and halts the session; it supplies no compatibility evidence and cannot authorize coding.
  No alternate counter, prompt reduction, retry, replacement or permit recycling is allowed. Actual inclusive input above 32,768 after an admitted send remains a global `provider_limit_breach`.
- A properly blocked forbidden-tool attempt is a behavior failure; actual unauthorized access/effect is a global containment breach.
- A safety halt forbids new paid requests even if later evidence resolves a reservation. Preserve late evidence and original stop disposition; no automatic resume.
- Never stop because A or B appears better/worse. Retain all eight planned slot rows, including unstarted slots, and every paid/invalid attempt.
- Any interrupted trajectory is explicitly partial; a stopped/unfinished schedule is an incomplete pilot. No outcome-selected “completed sample” may replace the frozen eight slots.

## 12. Execution locks, acceptance and handoff

- **Current missing gate 1:** separately approved implementation and review of the live final-request/admission/capture/containment boundary; the loopback/dummy fixture guard is evidence only.
- **Current missing gate 2:** accepted compatibility evidence for the actual selected provider/model, CountTokens projection, adaptive/xhigh/4096 settings, usage/ID parser, retry refusal and real transport.
- **Current missing gate 3:** approved task/credential/provider-retention rights, actual account identity, standard-service ratecard/pricing evidence and non-secret resolved launch configuration.
- **Current missing gate 4:** authorized local materialization of all four views and a frozen launch manifest binding their sources/citations, allocation, workspaces, executable/profile/oracle pins, settings, prefixes, permissions and request inventory.
- **Current missing gate 5:** explicit owner authorization identifying this contract’s external digest, accepted implementation digest and launch-manifest/ratecard pins, with scope stated as probe-only or coding.
- A probe may establish real compatibility only after capture-ready and all other applicable admission gates pass; absent sufficient existing real-wire evidence, coding stays locked until that probe is accepted and coding is explicitly authorized.
- Future runtime paths, generated IDs and manifest hashes are unresolved execution facts under frozen rules—not claimed existing implementations or discretionary placeholders.
- The dated false authorization/readiness fields above are not permission toggles; a separate scoped authorization receipt records any future grant. Policy changes require a new contract revision.
- A completed interpretable pilot requires eight retained terminal slot outcomes, all actual HTTP attempts accounted for, complete primary usage for transmitted generations, valid source/oracle bindings and no unresolved capture/containment/provenance breach.
- Correctness failures remain genuine results: report A/B success counts, each pair’s B−A differences, paired validity, and medians/ranges descriptively; show denominators and missing/partial observations explicitly.
- Apparatus/capture acceptance and coding success are separate verdicts. Even eight successful repairs and complete aggregate usage cannot establish T006 acceptance, causal savings or default promotion.
- The receipt names actual author/reviewer and coding provider/model identities; model nicknames do not establish independent labs. Publish only authorized aggregates, pins and limitations.
- Authority references: [T006](../spec.md#t006--validate-retrieval-repairs-in-real-coding-tasks), [adapter economics](adapter-economics.md), [constitution](../../../.specify/memory/constitution.md), and [accepted fixture receipt](../../../docs/validation.md#no-provider-apparatus-preflight--2026-10-29).
- Counting/cache documentation to revalidate before launch: [token counting](https://platform.claude.com/docs/en/build-with-claude/token-counting) and [prompt caching](https://platform.claude.com/docs/en/build-with-claude/prompt-caching). Their described estimate/free/default-TTL properties are not a fresh live compatibility test.
