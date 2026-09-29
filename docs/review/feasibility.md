# Bounded feasibility and team disposition — 2026-09-29

**Rust is viable for the tested model and protocol boundaries. The complete ecosystem
is not yet deployment-proven.** This pass replaces speculation with small executable
probes and replaces 013's obsolete vector-head contract. It does not turn every
remaining feature into a prerequisite for the first release.

Base: `0aa925ee66f667b7ef17fbdac8dc1e73cfd55a90`. Owner authorized sandboxed feasibility
work and direct push to main. Budget: up to 45 minutes total, each model execution
capped at ten minutes. Actual model calls finished in seconds. First-party probe code
is Rust; no production source, root Cargo dependencies, stores, services, credentials
or user-global configuration changed. Synthetic inputs only; separately fetched public
model artifacts stayed in owned scratch. No provider calls or token-savings claims.
This is an in-session audit, not independent review.

## Executed boundaries

macOS 26.5 arm64, 18 GiB RAM, 12 logical CPUs; local Rust 1.97.1. Model probes used
ad-hoc signed `.app` bundles, App Sandbox, minimal environment and offline loader
settings. Native runtime grants were development-only: pinned scratch input/output,
isolated Python environment and Homebrew runtime libraries. These broad development
runtime grants are **not** a distribution entitlement recipe. No signing identity or
notarized package was available. Linux probes used the existing Colima runtime,
aarch64 Rust 1.90 image with explicit resource/privilege restrictions.

| Probe | Actual result | Meaning and limit |
| --- | --- | --- |
| Bare sandboxed Rust executable | Code signature verified; execution trapped because bundle identity metadata was absent | Package worker as a real app bundle; signing an arbitrary CLI is insufficient on this host |
| App Sandbox bundle | Inherited stdin worked; outside sentinel read/write and loopback listener denied; `/usr/bin/true` child **allowed** | App Sandbox alone does not enforce the existing zero-descendant contract |
| App Sandbox plus hard `RLIMIT_NPROC=0` | `posix_spawn` via Rust Command and `fork` denied; raising limit denied; Rust thread worked; file/network-listener denials retained | Promising native process boundary, tested on this host. Set limit before exec/loading third-party libraries. It does not prove denial of all brokered OS services or distributed-package enforcement |
| MLX through Rust PyO3 | Selected 4-bit artifact loaded offline; one synthetic query returned 2048 finite values, norm 0.99999988; 603-token input refused before a 512-token probe encode | Real runtime bridge exists. Also passed when hard process limit was installed before exec. No document relevance, long-context, Metal quota or cancellation claim |
| ModernBERT + choice head | Full Rust encoder exactly matched reference hidden output on 123 tokens; logit max absolute error 8.94e-8; scorer gradient error 2.15e-6; real head SGD update and reload error 0 | Real pretrained model boundary, not a small vector classifier. Also passed with pre-exec process limit. One unpadded fixture, not complete training acceptance |
| ModernBERT encoder gradients | Global layer 0 and local layer 1 QKV gradient errors 1.97e-6 and 7.60e-7; Q/K gradients nonzero | Autograd through tested RoPE/masks works. No full-encoder optimizer/update/resource or general parity claim |
| Rust MCP SDK | rmcp 3.5.0 built with Rust 1.90; real stdio child initialize→list→call→close passed in network-disabled container; 65537-byte codec input rejected at 65536 limit | Concrete SDK path. Default transport is unbounded; use bounded codec. Production 16-handler admission/cancellation still needs proof |
| Vector index | USearch 2.26.2 on macOS: two synthetic 2048-d float32 vectors, search→save→load→remove→re-add→save→load passed in sandbox | Persistence/update API is usable. Native C++ dependency behind Rust binding, not first-party C++; no corpus-scale/crash/integration claim |
| Linux container profile | UID 65534, capabilities zero, no-new-privileges, seccomp active, no host/store/socket mount, read-only root; cgroup memory 256 MiB and pids 32 | Existing container route supplies real limits for this smoke. Does not prove a native Landlock package or Linux MLX compatibility |

The first ModernBERT attempt **failed** its hidden-output tolerance (1.38e-4). Cause:
Rust scalar f64 construction rounded RoPE frequencies differently from reference
float32 tensor arithmetic. Matching float32 construction yielded exact hidden output;
we did not relax the tolerance. Final fixture uses absolute tolerance 1e-5. Head
activation is ReLU while encoder/scorer use GELU; overlooking that would change behavior.
The single fixture covers token positions beyond the local window, but not its complete
boundary/mask/permutation matrix or train-mode dropout.

MLX emitted a Transformers warning about unrecognized `apply_yarn_scaling`; preserve
and investigate it in the long-context reference check before claiming that profile.
A unit norm alone does not prove embedding correctness. The probe's 512-token setting
is a test bound, **not** the selected document chunk size or product serving limit.

`time -l` observed peak footprint approximately 1.14 GiB for initial MLX inference,
3.14 GiB for the Rust head-update fixture and 2.81 GiB for the QKV-gradient fixture.
These were sequential, short-input runs, not aggregate residency, max-length training
or memory-quota proofs. Warm filesystem/backend caches affect the elapsed times.
The final limited MLX call reported 6.17 s load and 6.29 s total; the limited Rust
model/update/reload process reported 2.51 s including weight load. Do not advertise
these as steady-state serving latency or preparation throughput.

## Decisions that the team can use now

1. **013 contract v4 replaces v3.** Preserve ModernBERT-large and pretrained typed
   choice head. Pin Rust tch 0.24.0 with LibTorch 2.11.0; first supported fitting recipe
   freezes encoder and adapts the head, choice type row and scorer. Retain encoder
   adaptation behind its own complete acceptance. The test reference is external and
   not a production Python/Laya dependency. Four detailed tasks now target this model.
2. **009 bridge candidate:** Rust PyO3 0.29.2 calling the unchanged pinned publisher
   loader inside the worker. No first-party Python service. Select USearch 2.26.2 as
   the initial index integration candidate; its Rust 1.90/native build remains a check.
   Rust application code does not remove third-party C/C++ runtime packaging obligations.
3. **macOS native profile candidate:** real app bundle plus App Sandbox and a hard
   process limit set before exec. Continue this narrow profile before adding a VM.
   Distribution rights, owner-death cleanup, descriptor grants, network variants and
   resource enforcement remain acceptance; never silently run unrestricted on failure.
4. **003 SDK candidate:** rmcp 3.5.0 with explicit bounded framing. The codec can be
   passed through the SDK's sink/stream adapter. Default `AsyncRwTransport` uses
   unbounded `read_until`; merely setting a codec elsewhere does not constrain it.
   Do not claim handler admission solved by this happy-path exchange.
5. **Release sequencing stands:** implement 001 first; MCP, graph and explicit memory
   do not wait for learning. A model package waits only for its own unresolved boundary.
   No new numbered spec, orchestration platform or universal measurement ladder.

## Remaining unknowns: owner, decision and bounded exit

“All unknowns solved” would overstate this evidence. Each remaining item below names
what can close it; none is an instruction for an open-ended benchmark campaign. Work
on dependent implementation stops at the named boundary, not at a new global gate.

| Item / owning task | What remains; cheapest decisive exercise | Budget / disposition |
| --- | --- | --- |
| 001 source/recovery | Execute the already specified crash/pending-index/reopen cases on synthetic root; preserve explicit repair | Focused implementation tests; startable, no model feasibility dependency |
| 003 SDK admission | Prove pre-dispatch cap of 16 handlers with flood/cancel/EOF while engine remains one-active/zero-queue. If SDK cannot enforce it, select a maintained version/path that can; do not queue unbounded tasks behind a semaphore | 10-minute fixture after adapter boundary exists; blocks MCP claim only |
| 003 gateway host | Actual isolated Codex 0.144.1 client against loopback Rust stub: streamed text, tool round, cancellation, retries, reasoning and request IDs. No real provider key | 10-minute protocol probe; not run here; pin accepted subset before T004 |
| 003 provider economics | A permitted bounded real API request with input-count projection and terminal usage; missing terminal event remains unknown. Cache fields and retry costs must be observable | Requires explicit account/model and spend cap; no credentials or paid calls used in this pass |
| 005 producer | Pin installed rust-analyzer `0.0.0 (f8996691e9 2026-08-30)` or an immutable official release; run `scip` on a synthetic Rust crate in an execution jail; bind source hashes and decode/import its actual snapshot | 10-minute producer/import fixture once chosen toolchain/package is ready; not run here |
| 005 scale | Declare licensed representative corpus and numeric source/symbol/memory/time limits before one import/context run | Corpus-dependent scale claim only; small graph workflow can proceed separately |
| 008 explicit memory | Existing lifecycle/schema contract is startable; implement edit/forget/export with no implicit training consent | Focused integration after 001; no new research |
| 009 output/input recipe | Reference-vector comparison, exact query/passage prefixes, tested cap and cap+1, warning resolution, batch one, timeout and peak memory at proposed cap | <=10 minutes model execution; no full corpus until pass |
| 009 document usefulness | Small fixed source/query set with expected spans: whole file/section first, compare finer split only on a concrete miss; inspect delivered evidence within normal 2048-token response | One bounded fixture in existing T002; no 32768-token chunk-size assumption or mandatory reranker |
| 009 index/MSRV | Compile native dependency on Rust 1.90 with actual C++ toolchain, then stale-profile/update/rebuild-from-cache and corrupt-index cases | Current Linux slim image lacked `c++`; this was environment failure, not proven library incompatibility. 10-minute build/API fixture |
| 009 job ownership | Actual worker blocked inside encoder: timeout, query during drain, stop, owner EOF; no released slot or hidden queue until process is gone | 5-minute lifecycle fixture after supervisor exists |
| 013 exact recipe | Rust tokenizer rendering; option order/masks; AdamW/train dropout; all intended trainable groups; frozen encoder hash; max sequence. Encoder adaptation additionally requires full selected-parameter updates and memory | <=20 update steps / ten minutes per selected recipe; current SGD/gradient probe is not these results |
| 013 useful continuous learning | Two permitted grouped rounds, correction/withdrawal/replay, calibrated refusal, candidate selection/rollback and checked tasks | Needs real approved labels meeting v4 floors; synthetic labels cannot establish benefit. No self-labeling to manufacture data |
| 009/013 actual package | Private runtime packaging and notices, signing/notarization, all filesystem/network/FD/process denials, owner-death cleanup and upgrade/rollback on installed artifact | Per advertised target, one bounded install/deny/lifecycle exercise; current ad-hoc bundles are not release packages |
| 009/013 aggregate resources | Measure both resident models and selected max-length calls under declared total 8 GiB supervised ceiling; no training/online overlap. Reject unsupported requested hard limits | <=10-minute overlap fixture; no adding individual short-run peaks or claiming GPU quota from RSS |
| 013 Rust 1.90 | Build selected tch/LibTorch/tokenizer integration on supported toolchain and package target | <=10-minute compile/smoke once LibTorch package supplied; MCP's MSRV pass does not transfer to ML |
| Schema integration / H8 | One integrator owns store version and dispatch; separate branches/worktrees, integrate after 001 | Coordination decision, no new schema registry/framework |

No watcher, federation, autonomous trainer, generated-knowledge lane or migration
service is implicitly added. Their existing deferred/re-entry dispositions stand.
Referencing another folder remains literal session data until explicit root admission;
that event creates no cross-root ledger, background work or training rights.

## Reproducibility and provenance

[Probe sources, commands and outputs](../../tools/feasibility/README.md) are retained
outside the product crate. [Artifact hashes](../../tools/feasibility/results/artifacts.json)
contain public identities only; no weights, private input or third-party source is vendored.
The selected model/runtime/reference sources are:

- [MLX artifact and loader at fixed revision](https://huggingface.co/mlx-community/Nemotron-3-Embed-1B-BF16-4bit/tree/d0408b94c50fc327b6ea37dce7409c51e020a4d8).
- [Typed-decision weights/config](https://huggingface.co/convaiinnovations/laya-typed-decisions/tree/1a793eb568e6718f15941d08f85432581df534e3).
- [Unmodified decision reference](https://github.com/NandhaKishorM/laya/blob/4066d5d5fbf08b66c6757ddeedbd797bd7655bc0/laya/common.py).
- [tch 0.24.0](https://docs.rs/tch/0.24.0/tch/), [rmcp 3.5.0](https://docs.rs/rmcp/3.5.0/rmcp/), [USearch 2.26.2](https://docs.rs/usearch/2.26.2/usearch/).

The earlier [handoff simulation](handoff-simulation.md) remains historical paper
analysis. H1 is replaced by v4; H3/H4 have narrowed executable evidence; H5/H7 remain
partial; H2/H6 corrections and H8 ownership remain in force. No passing scratch fixture
marks a product task complete. This report is the disposition of this bounded exercise,
not an extra approval stage for every change.
