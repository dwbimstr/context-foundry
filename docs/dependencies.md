# Dependencies and licenses

This inventory records Cargo metadata for the locked dependency graph. It is not a legal or vulnerability audit. No dependency sources are vendored. Binary distribution must retain the applicable license/notice texts from the selected graph.

The optional task context workflow uses `rust-stemmers` 1.2.0
(`MIT/BSD-3-Clause` in the package manifest) for English word forms in
meeting notes. The matcher calls `Stemmer::stem` with `Algorithm::English`.
The 008 regression `later_note_finds_two_promises_and_reviews_only_the_connection`
checks that comparisons preserve the original bytes and their attribution.
The locked crate uses the existing serde dependencies.

One section is not about a dependency. The
[oh-my-pi reference patches](#oh-my-pi-reference-patches) are tracked files that
carry lines of upstream source, and that section records their license and notices.

## Direct dependencies

The existing rows retain the `cargo metadata --locked` inventory from 2026-10-01
(001 + 003 implementation). T005 additions were checked against `Cargo.toml`,
`Cargo.lock` and the locally cached crate manifests/license files on 2026-10-04.
The handed-off metadata inventory reports 13 added packages; no Cargo command,
fetch or build was run for this documentation update.
"dev" packages build tests only and are not linked into the shipped binary.
Production rmcp serves MCP and resolves explicitly selected live host task views;
its dev features add the child-process test transport. Tokio adds process support
for tests. On 2026-10-12 the Dependabot
updates #10, #8 and #7 were applied on the experimental line: libc, rmcp, tokio, uuid
and the `toml` dev dependency (0.8 to 1.1) moved, plus hyper, usearch and want
transitively. The declared licenses are unchanged. On 2026-10-13 `sha2` moved to 0.11. Its digest
output type has no `{:x}` formatting, so first all 18 hex sites moved to
`context_foundry::lower_hex`; [validation](validation.md#sha2-011-migration--2026-10-13)
records the byte-identity proof. The bump adds digest 0.11, hybrid-array, block-buffer
0.12, crypto-common 0.2 and const-oid (all MIT OR Apache-2.0). sha2 0.10.9 remains only
under the optional `learning-worker` feature, via `tch` → `zip` → `pbkdf2`. On 2026-10-23
six updates from Dependabot #35 were applied: libc 0.2.191, redb 4.4.0, serde_json
1.0.154, tokio 1.53.3, usearch 2.26.5 (exact pin) and uuid 1.28.0; serde_json now also
depends on the already locked foldhash. The declared licenses are unchanged. The same
PR's tree-sitter-scala 0.26.3 was held at that point: a grammar change can change
units, which
[§ Index version gate](../specs/001-source-state-recovery/contracts/context-v2.md#index-version-gate)
and the partition recipes' `SOURCE_UNIT_SCHEMA` tie to schema bumps, an owner decision.
The owner settled it on 2026-11-06 ("One bump, gated on unit diff", below).

The 2026-10-26 maintenance subset of PR #37 updates rmcp to 3.6.0
(rmcp-macros 3.7.0), Tokio to 1.54.1, and ureq to 3.5.0 (ureq-proto 0.6.5).
Licenses are unchanged; rmcp adds an edge to the already locked libc.

On 2026-11-19 the owner chose "Apply 8, hold 3" for Dependabot PR #55
([decision](validation.md#owner-decisions--2026-11-19)). Applied by hand on the
experimental line, at the PR's versions: ignore 0.4.34, ureq 3.5.2 (ureq-proto 0.7.1),
rmcp 3.10.0 (rmcp-macros 3.11.0, still an exact pin), uuid 1.29.2, notify 8.2.2, toml
1.2.1 (with serde_spanned, toml_datetime and toml_writer), tokio-rustls 0.26.6 and
rustls-pki-types 1.16.0. notify 8.2.2 replaces fsevent-sys with objc2-core-foundation
and objc2-core-services 0.3.2 (`Zlib OR Apache-2.0 OR MIT`) and drops the Windows 0.53
target crates. Every other declared license is unchanged. Held until measured:
tree-sitter-scala 0.26.13 (parse output and schema), usearch 2.26.6 (ANN graph and G2)
and redb 4.6.0 (the store database).

On 2026-10-26 pulldown-cmark moved to 0.13.5 (security remediation, default
features still off): 0.13.4's default parser exits 101 on a 13-byte
Markdown list/indentation input, and its complexity fixes are upstream, not
locally timed. Because the fix changes Markdown units, the same change bumps
the derived search schema to `"8"` and `SOURCE_UNIT_SCHEMA` to `"6"`
([§ Unit-grammar repair](../specs/001-source-state-recovery/contracts/context-v2.md#unit-grammar-repair--2026-10-26));
older stores repair explicitly and semantic partitions under `units5` refit
at the next `semantic prepare`. tree-sitter-scala 0.26.3 was held: its
grammar change can change units the same way, an owner decision, settled on
2026-11-06 (below).

Owner decisions of 2026-11-06 ([validation](validation.md#owner-decisions--2026-11-06))
settle the held and open Dependabot updates:

- **Parser updates** ("One bump, gated on unit diff"). Implemented together on
  `68ed776`: tree-sitter-scala 0.26.4, tree-sitter-powershell 0.26.6 and
  pulldown-cmark 0.13.6, with derived search schema 9 and source-unit schema 7.
  Complete ordered output snapshots are identical for all admitted sources in
  both G2 corpus roots and the frozen parser fixtures, so the conditional fresh
  G2 run is not triggered. Syntax and migration checks and all eight local gates
  pass; this bundle is accepted. Existing stores preserve authoritative and old
  semantic state, requiring explicit `repair-index`/`semantic prepare` under the
  new grammar identity. Authoritative schema 12 and semantic generation 6 are
  unchanged. Only these three lock package blocks change; unrelated resolver
  rewrites were restored. The three packages still declare MIT, and their
  packaged license bytes match their baselines. See the
  [comparison and migration receipt](validation.md#parser-bundle-and-complete-unit-comparison--2026-11-11).
- **Tantivy 0.27.0, PR #47** ("Apply only if neutral"). Done on the experimental
  line: `2929769` needs no Foundry API adaptation or schema bump. Existing 0.26
  stores open without repair; all 10,432 G1 tasks and 14,284 response records per
  arm are byte-identical, and all eight local gates pass, including Rust 1.90.
  See the [compatibility and neutrality receipt](validation.md#tantivy-027-store-compatibility-and-g1-neutrality--2026-11-11).
- **tree-sitter-dart 0.3.0, PR #48** ("Close; stay on 0.2.0"). Done: the PR was
  closed. 0.3.0 renames `class_member` and `enum_constant`, which the Dart unit
  extraction matches, so taking it would need code changes and a schema bump that
  forces every store to repair, and no corpus or G2 question uses Dart. Modern Dart syntax
  and 0.2.0's misparsed arrow-function bodies stay unfixed.
- **rustls** and **zip**: see [003 T004](#additions-for-003-t004-owned-model-gateway)
  and [the legacy components](#legacy-prototype-and-planned-optional-components).

| Package | Locked version | Declared license | Use |
| --- | --- | --- | --- |
| anyhow | 1.0.104 | MIT OR Apache-2.0 | normal |
| axum | 0.8.9 | MIT | normal (shared-owner HTTP) |
| clap | 4.6.7 | MIT OR Apache-2.0 | normal |
| futures-util | 0.3.34 | MIT OR Apache-2.0 | normal |
| http | 1.5.0 | MIT OR Apache-2.0 | normal |
| ignore | 0.4.34 | Unlicense OR MIT | normal |
| imbl | 7.0.2 | MPL-2.0+ | normal (009 T008 persistent location snapshots; unmodified) |
| libc | 0.2.191 | MIT OR Apache-2.0 | normal (held-root opens) |
| pulldown-cmark | 0.13.6 | MIT | normal (Markdown sections; default features off) |
| redb | 4.4.0 | MIT OR Apache-2.0 | normal |
| rmcp | 3.10.0 | Apache-2.0 | normal (server and live-host client); dev adds child-process transport |
| serde | 1.0.229 | MIT OR Apache-2.0 | normal |
| serde_json | 1.0.154 | MIT OR Apache-2.0 | normal |
| sha2 | 0.11.1 | MIT OR Apache-2.0 | normal |
| tantivy | 0.27.0 | MIT | normal |
| tiktoken-rs | 0.12.1 | MIT | normal |
| tokio | 1.54.1 | MIT | normal; dev adds process |
| tokio-util | 0.7.20 | MIT | normal |
| tree-sitter | 0.26.13 | MIT | normal (syntax runtime; third-party C) |
| tree-sitter-c | 0.24.2 | MIT | normal (C grammar) |
| tree-sitter-cpp | 0.23.4 | MIT | normal (C/C++ grammar) |
| tree-sitter-go | 0.25.0 | MIT | normal (Go grammar) |
| tree-sitter-java | 0.23.5 | MIT | normal (Java grammar) |
| tree-sitter-javascript | 0.25.0 | MIT | normal (JavaScript/JSX grammar) |
| tree-sitter-powershell | 0.26.6 | MIT | normal (PowerShell grammar) |
| tree-sitter-python | 0.25.0 | MIT | normal (Python grammar) |
| tree-sitter-rust | 0.24.2 | MIT | normal (Rust grammar) |
| tree-sitter-scala | 0.26.4 | MIT | normal (Scala grammar) |
| tree-sitter-typescript | 0.23.2 | MIT | normal (TypeScript/TSX grammars) |
| ureq | 3.5.2 | MIT OR Apache-2.0 | normal (bounded gateway-launcher health check) |
| uuid | 1.29.2 | Apache-2.0 OR MIT | normal |
| reqwest | 0.13.5 | MIT OR Apache-2.0 | dev |
| tempfile | 3.27.0 | MIT OR Apache-2.0 | dev |
| toml | 1.2.1 | MIT OR Apache-2.0 | dev |

The 2026-11-11 Tantivy update also moves its bitpacker, columnar, common,
query-grammar, sstable and stacker crates to 0.27.0 (MIT), datasketches to 0.5.0
(Apache-2.0), lru to 0.18.5 (MIT), lz4_flex to 0.14.0 (MIT), and murmurhash32 to
0.4.0 (MIT). New package names are serde-tuple-vec-map 1.0.1 (MIT) and
unwrap-infallible 1.0.0 (MIT OR Apache-2.0). The old allocator-api2 0.2.21 and
hashbrown 0.16.1 entries disappear; hashbrown 0.17.1 remains. The bindgen lock
edge is unchanged. These are Cargo license declarations; release notice collection
remains separate.

## Transitive additions for T005

| Package | Locked version | Declared license | Use |
| --- | --- | --- | --- |
| tree-sitter-language | 0.1.8 | MIT | transitive (grammar/runtime language binding; declares Rust 1.90) |
| streaming-iterator | 0.1.9 | MIT OR Apache-2.0 | transitive (tree-sitter iterator interface) |
| unicase | 2.9.0 | MIT OR Apache-2.0 | transitive (pulldown-cmark) |

The tree-sitter runtime and grammars compile bundled C parsers/scanners through
`cc`, already in the held graph. This is third-party C, not first-party C/C++.

001 T008 moves the runtime to tree-sitter 0.26 and adds 15 grammars (context-v2
§ Languages). Three grammars resolve from owner-approved forks through
`[patch.crates-io]` git revs, each MIT like its upstream; packaging reads their
license files from the git checkout Cargo resolved:

| Package | Version | Fork and rev | Change |
| --- | --- | --- | --- |
| tree-sitter-ruby | 0.23.1 | `dwbimstr/tree-sitter-ruby` `1a594bf` | heredoc serialization bound |
| tree-sitter-perl | 1.1.2 | `dwbimstr/tree-sitter-perl` `0686313` | scanner frees its heredoc queues |
| tree-sitter-rust | 0.24.2 | `dwbimstr/tree-sitter-rust` `d43cece` | rustc 1.99 nightly syntax |

The current lock contains 300 packages including Foundry: **299 third-party**
packages, up from the historical 286 by these 13 additions. Declared expressions are permissive
(MIT/Apache-2.0 variants, Unlicense, Unicode-3.0, Zlib, BSD-3-Clause, BSL-1.0) or offer
a permissive choice (one `MIT OR Apache-2.0 OR LGPL-2.1-or-later`, one
`Apache-2.0 / MIT / MPL-2.0`). rmcp is Apache-2.0 only: a binary distribution must
carry its license text and any NOTICE. This is metadata, not a legal audit.

### Local license-text availability

MIT text was present in the cached C, Go, JavaScript, Python, Rust, language-binding
and pulldown-cmark packages. `streaming-iterator` and `unicase` each contain both
`LICENSE-MIT` and `LICENSE-APACHE`, matching their declared alternatives.
The cached `tree-sitter`, `tree-sitter-cpp`, `tree-sitter-java` and
`tree-sitter-typescript` packages contain no standalone main MIT `LICENSE` file;
their MIT rows are verified manifest declarations, not verification of absent text.
The runtime also contains `src/unicode/LICENSE` with ICU/Unicode notices; that file
is present and is not the missing main MIT notice. These are inventory observations,
not a legal audit or a distribution-readiness claim. No license text was fetched.

## Additions for 003 T004 (owned model gateway)

New direct dependencies:
- `reqwest` 0.13 with `default-features = false` and `rustls` + `stream`;
- `hyper` 1 (`server`, `http1`), `hyper-util`, `http-body-util` and `bytes`, all
  already in the lock through axum/rmcp.

The upstream client runs with TLS verification on, through rustls with the aws-lc-rs
provider and the platform verifier, and with no proxy, redirects or decompression.
The lock grew from 300 to 344 entries, a count that includes platform-only crates.

rustls is locked at 0.23.46 on both `experimental/context-ecosystem` and `main`, the
fix for Dependabot alerts #9–#12. Owner decision, 2026-11-06 ("Bump rustls, then
merge"): the bump landed on the experimental line as `8600fc0`, the local gates ran
([receipt](validation.md#owner-decisions--2026-11-06)), and PR #52 carried it to
`main` as merge commit `d94b013` on 2026-11-07. The alerts are closed and Dependabot
PR #49 was closed as superseded.

On `aarch64-apple-darwin`, 27 crates are new: mostly `MIT OR Apache-2.0`, plus ISC,
Zlib, BSD-3-Clause, `CC0-1.0 OR MIT-0 OR Apache-2.0` and two compound expressions:
- `ring`: `Apache-2.0 AND ISC`;
- `aws-lc-sys`: `ISC AND (Apache-2.0 OR ISC) AND Apache-2.0 AND MIT AND BSD-3-Clause AND …`.
All are permissive, but `aws-lc-sys` and `ring` carry several notices that a binary
distribution must reproduce. `aws-lc-sys` also builds a third-party C library;
Foundry adds no first-party C. Source: `cargo metadata --locked --filter-platform
aarch64-apple-darwin`. This is metadata, not a legal audit.

## 009 T004 embedding worker

The optional `embed-worker` feature (default off) builds `foundry-embed` on llama.cpp
`b9acf138` (MIT; its LICENSE also covers the vendored ggml, which has no license file
of its own at that commit). `build.rs` builds it from the pinned commit's tree with
CMake and links it statically, with Apple's Metal, Accelerate and Foundation
frameworks; packaging takes the license from the worker (`foundry-embed --notices`).
The native worker build exposed a lockfile mismatch on 2026-11-10: bindgen selected
syn 2 while prettyplease selected syn 3. The lock now restores bindgen's previous
syn 3.0.6 edge, matching prettyplease 0.3.0; package versions and the default CLI/MCP
graph are unchanged. See the [native receipt](validation.md#native-macos-acceptance--2026-11-10)
for the failed build and subsequent verification.

The feature adds one optional build dependency, `bindgen` 0.73 (BSD-3-Clause), for the
C bindings; default-feature builds and their Rust 1.90 gates do not compile it.
PyO3 and the MLX runtime closure are no longer product dependencies.


## Legacy prototype and planned optional components

Laya remains a separately licensed, read-only research reference, not a Cargo
dependency or deployed service. The product uses ureq for the gateway launcher's
bounded health check. The product Cargo graph excludes
LibTorch from default-feature builds. Its optional `learning-worker` and the separate
[probe crate](../tools/feasibility/README.md) now pin experimental tch 0.26.0 with
LibTorch 2.13.0; see the [compatibility review](review/tch-026.md).
That `tch` path also locks zip 0.6.6 in both lock files, the subject of Dependabot
alerts #13–#16 (two critical, two medium). Owner decision, 2026-11-06 ("Dismiss as
unused"): done, the four alerts were dismissed as not used. Default and release builds
never link `tch`, Foundry calls no npz loader, and the vulnerable npz and build-time
paths (`tch`'s npz reader, `torch-sys`'s build script) never run here. Unfreezing 013
or adding npz loading must trigger a recheck.
The probe also pins PyO3 0.29.2, USearch 2.26.2 and rmcp 3.5.0 (the product has
since moved to rmcp 3.10.0 and USearch 2.26.5); its lock is not the product lock. Contract 013 v4's accepted
baseline was tch 0.24.0/LibTorch 2.11.0; that acceptance does not transfer to this
migration. USearch has a third-party native C++ core;
this does not introduce first-party C++ code. Libkrun remains conditional.
The selected third-party MLX loader was exercised through Rust PyO3, but private
runtime distribution remains unaccepted. Record transitive libraries, notices and any VM image/kernel license
inventory when those packages are actually selected. No weights or private data are
redistributed by this plan; no dependency has been added merely by documenting it.
The optional gateway (003 T004) is implemented with the locked HTTP/TLS client stack
above. It forwards and meters only; no provider token-counting implementation exists,
which is why `enforce` mode stays refused.

Use `cargo metadata --locked` for the full transitive graph. `Cargo.lock` pins it.

009 T008 pins `imbl` 7.0.2 for structurally shared ordered maps. The locked release
requires Rust 1.85 and is built here with Rust 1.90. It adds `archery` 1.2.3,
`imbl-sized-chunks` 0.2.0, `rand_xoshiro` 0.7.0, `bytemuck` 1.25.2,
`safe_arch` 0.7.4 and `wide` 0.7.33. Source/design references and intentional differences
are retained in [009 T008](../specs/009-optional-semantic-retrieval/spec.md#t008--maintain-and-publish-only-changed-semantic-work).
The MPL license/notice and source-availability obligations apply to that upstream
package when distributing; Foundry's own code remains MIT. No upstream source is
modified or vendored.

## oh-my-pi reference patches

`docs/references` tracks 25 patch files (`omp-*.patch`, 364,340 bytes in total at
`85a63c3`). They are not Cargo dependencies, and nothing in Foundry's source, scripts
or tests reads or applies them. They are reference data for the
[proposed startup observation and admission](../specs/003-agent-retrieval-context/contracts/bounded-pilot-v2.md#proposed-startup-observation-and-admission)
section of the bounded pilot v2 draft for 003 T006, which is unfunded and not frozen.
They are unified diffs against
[`can1357/oh-my-pi`](https://github.com/can1357/oh-my-pi/tree/24439066040e006eb08da7b204f0ac41c79d2de9)
at commit `24439066040e006eb08da7b204f0ac41c79d2de9`. The patches form one series, in
which later patches apply on top of earlier ones. The facts below were checked on
2026-11-19 at Foundry commit `85a63c3`, against upstream files read at the pinned
commit through the public GitHub API; the last paragraph records the notice headers
added after that check. This is an inventory, not a legal audit.

**Upstream license.** The upstream root
[`LICENSE`](https://github.com/can1357/oh-my-pi/blob/24439066040e006eb08da7b204f0ac41c79d2de9/LICENSE)
at that commit is the MIT License with three copyright lines: Mario Zechner (2025),
Can Bölük (2025-2026) and Stencil Labs, Inc. (2026). Its Git blob is
`09a0c6c926a81c9d56dc3326cff1e5358c63c6d6`, 1,144 bytes, SHA-256
`16c45f9d667442781f03fa198914cc39abcaa48ec5ed8f644643e554ca2fbf63`. The patches touch
four upstream packages, and each declares `MIT` and version 18.18.1 in its
`package.json`. `packages/ai` and `packages/coding-agent` carry the same `LICENSE`
blob as the root. `packages/catalog` and `packages/utils` carry a `LICENSE` that
differs only by omitting the Mario Zechner line (blob
`f7a04b97f9a0a145a5787e8ea8a36ebd3b1eaef1`). The upstream
`packages/coding-agent/THIRD-PARTY-NOTICES.txt` names five files taken from other
projects, three under Apache-2.0 and two under other authors' MIT terms. None of them
is among the 50 paths the patches touch.

**What the files contain.** The diffs cover 50 upstream paths: 32 that exist at the
pinned commit and 18 new files that the patches add. Their hunks hold 4,823 added
lines, 2,148 context lines and 388 removed lines. The added lines are Foundry's
proposed changes. With leading and trailing whitespace removed, 72 of them are at
least 40 characters long and also occur in one of the 32 upstream files (30 when lines
are compared whole, with their indentation). Of the context and removed lines, 2,014
occur as whole lines in the same upstream file at the pinned commit (1,668 context,
346 removed). The other 522 are lines that a patch in the series adds to the same
file. The tracked files therefore reproduce upstream source text in at least 2,014
hunk lines, of which 113 are blank or hold only whitespace.

**Notice carried by each file.** At `85a63c3`, fourteen files reproduced the upstream
license text, with its three copyright lines, in a header above the diff that also
names the upstream and the pinned commit. Eleven did not.
`omp-startup-rpc-observation.patch` named the upstream, the commit and "MIT" in its
header but did not carry the notice. The other ten had no header: they started at the
diff and named neither the upstream nor the commit, although their paths and context
lines match that commit and the rest of the series. Outside the fourteen headers the
repository held no copy of the upstream notice; Foundry's own [`LICENSE`](../LICENSE)
names only "Context Foundry contributors". The table gives the state at `85a63c3`;
the last paragraph records the headers added since.

| Patch | Dated record | Upstream notice in the file at `85a63c3` |
| --- | --- | --- |
| [omp-restricted-startup.patch](references/omp-restricted-startup.patch) | [2026-11-16](validation.md#restricted-startup-ownership--2026-11-16) | full text |
| [omp-startup-advisor-exclusion.patch](references/omp-startup-advisor-exclusion.patch) | [2026-11-18](validation.md#advisor-exclusion-during-startup-observation--2026-11-18) | none; no header |
| [omp-startup-auth-retirement.patch](references/omp-startup-auth-retirement.patch) | [2026-11-16](validation.md#source-built-capture-profile-qualification--2026-11-16) | full text |
| [omp-startup-catalog-spotlight.patch](references/omp-startup-catalog-spotlight.patch) | [2026-11-18](validation.md#catalog-refusal-and-spotlight-startup-exclusion--2026-11-18) | none; no header |
| [omp-startup-connection-warmups.patch](references/omp-startup-connection-warmups.patch) | [2026-11-16](validation.md#source-built-bounded-startup-caller-owner--2026-11-16) | full text |
| [omp-startup-dap-idle-sweep.patch](references/omp-startup-dap-idle-sweep.patch) | [2026-11-18](validation.md#debugger-sweep-starts-with-session-registration--2026-11-18) | none; no header |
| [omp-startup-discovery-retirement.patch](references/omp-startup-discovery-retirement.patch) | [2026-11-16](validation.md#source-owned-startup-discovery-retirement--2026-11-16) | full text |
| [omp-startup-empty-auth.patch](references/omp-startup-empty-auth.patch) | [2026-11-16](validation.md#fixed-empty-auth-startup-retirement--2026-11-16) | full text |
| [omp-startup-extension-owner.patch](references/omp-startup-extension-owner.patch) | [2026-11-18](validation.md#source-owned-startup-extension-dispatch--2026-11-18) | full text |
| [omp-startup-fixed-discovery-plan.patch](references/omp-startup-fixed-discovery-plan.patch) | [2026-11-19](validation.md#fixed-discovery-provider-inventory-at-startup--2026-11-19) | none; no header |
| [omp-startup-fixed-runtime-providers.patch](references/omp-startup-fixed-runtime-providers.patch) | [2026-11-18](validation.md#fixed-runtime-providers-during-startup-observation--2026-11-18) | none; no header |
| [omp-startup-logging-owner.patch](references/omp-startup-logging-owner.patch) | [2026-11-18](validation.md#source-owned-startup-logger--2026-11-18) | full text |
| [omp-startup-managed-timer-exclusion.patch](references/omp-startup-managed-timer-exclusion.patch) | [2026-11-18](validation.md#restricted-startup-managed-timer-exclusion--2026-11-18) | full text |
| [omp-startup-marketplace-exclusion.patch](references/omp-startup-marketplace-exclusion.patch) | [2026-11-18](validation.md#source-owned-marketplace-exclusion--2026-11-18) | full text |
| [omp-startup-plugin-roots-owner.patch](references/omp-startup-plugin-roots-owner.patch) | [2026-11-18](validation.md#source-owned-startup-plugin-roots--2026-11-18) | full text |
| [omp-startup-presence-retirement.patch](references/omp-startup-presence-retirement.patch) | [2026-11-16](validation.md#original-daemon-presence-candidate--2026-11-16) | full text |
| [omp-startup-prompt-exclusion.patch](references/omp-startup-prompt-exclusion.patch) | [2026-11-18](validation.md#restricted-prompt-inputs-and-startup-launch-binding--2026-11-18) | none; no header |
| [omp-startup-prompt-scheme-exclusion.patch](references/omp-startup-prompt-scheme-exclusion.patch) | [2026-11-19](validation.md#restricted-startup-scheme-descriptions-and-process-policy--2026-11-19) | none; no header |
| [omp-startup-refusal.patch](references/omp-startup-refusal.patch) | [2026-11-16](validation.md#source-owned-durable-startup-refusal--2026-11-16) | full text |
| [omp-startup-rpc-observation.patch](references/omp-startup-rpc-observation.patch) | [2026-11-18](validation.md#restricted-startup-rpc-retirement--2026-11-18) | none; header names MIT |
| [omp-startup-rpc-runtime-exclusion.patch](references/omp-startup-rpc-runtime-exclusion.patch) | [2026-11-18](validation.md#rpc-runtime-callbacks-excluded-during-startup-observation--2026-11-18) | none; no header |
| [omp-startup-sdk-export.patch](references/omp-startup-sdk-export.patch) | [2026-11-18](validation.md#direct-sdk-import-for-startup-observation--2026-11-18) | none; no header |
| [omp-startup-settings-retirement.patch](references/omp-startup-settings-retirement.patch) | [2026-11-16](validation.md#source-owned-settings-retirement--2026-11-16) | full text |
| [omp-startup-telemetry-exclusion.patch](references/omp-startup-telemetry-exclusion.patch) | [2026-11-18](validation.md#source-owned-telemetry-exclusion--2026-11-18) | full text |
| [omp-startup-workspace-scan-exclusion.patch](references/omp-startup-workspace-scan-exclusion.patch) | [2026-11-18](validation.md#restricted-workspace-scan-exclusion--2026-11-18) | none; no header |

**What the license requires.** The MIT text grants permission to use, copy, modify,
merge, publish, distribute, sublicense and sell the software. Its one condition is
that the copyright notice and the permission notice are included in "all copies or
substantial portions of the Software". The text sets no other condition, so on its
wording the patches may stay in this MIT repository as long as the upstream notice
stays with them; it gives no reason to move them out of Git. Tracked files are copied
by every clone, push and source archive, and the selected release destination carries
a source archive ([release](release.md)). The macOS package built by
`scripts/package.sh` does not include `docs/`.

**Notice added.** The eleven files without the notice hold 505 of those upstream
lines (441 context, 64 removed; 24 of the 505 are blank), all from
`packages/coding-agent`, whose `LICENSE` is the root text with all three copyright
lines. The license does not define "substantial portions", and this inventory does not
decide whether those lines are a substantial portion; carrying the notice makes the
question moot. The GitHub repository `dwbimstr/context-foundry` is public (checked
through the GitHub API on 2026-11-19), and the eleven files, committed on 2026-11-18
and 2026-11-19, are on the pushed `experimental/context-ecosystem` branch, so they
were published without the notice and those versions remain in its history. The
change that adds this section gives each of the eleven the upstream notice header in
the form the other fourteen use: `omp-startup-advisor-exclusion.patch`,
`omp-startup-catalog-spotlight.patch`, `omp-startup-dap-idle-sweep.patch`,
`omp-startup-fixed-discovery-plan.patch`,
`omp-startup-fixed-runtime-providers.patch`, `omp-startup-prompt-exclusion.patch`,
`omp-startup-prompt-scheme-exclusion.patch`, `omp-startup-rpc-observation.patch`,
`omp-startup-rpc-runtime-exclusion.patch`, `omp-startup-sdk-export.patch` and
`omp-startup-workspace-scan-exclusion.patch`. Their hunks are unchanged, so the line
counts above still hold, but the header changes each file's bytes. Of the 25 files'
SHA-256 digests at `85a63c3`, the dated records in [validation](validation.md) contain
one, that of `omp-startup-auth-retirement.patch`, which already carried the notice and
is not changed. No other tracked file cites the SHA-256 digest or Git blob id of any
of the 25. The private receipts were not opened for this check, so whether any of them
records a digest of the eleven files is not known. Step 4 of the
[release checklist](release.md), which inspects the Git contents for licenses and
notices, still applies before any release.

**Added 2026-11-20.** A 26th patch,
[omp-startup-extension-notifications.patch](references/omp-startup-extension-notifications.patch),
carries the full upstream notice in the same header form
([record](validation.md#deferred-startup-notifications-in-the-extension-runner--2026-11-20)).
The counts above are for the 25 files at `85a63c3` and do not include it. Its hunks
touch two files that earlier patches already touch, so the series still covers 50
upstream paths.

### Applied order

The 26 patches form one series, each applying on top of the ones before it. The order
below is the one the 2026-11-20 startup round's restore and build scripts use; it
matches the commit that added each file. A check on copies of the 50 files the series
touches reversed all 26 cleanly from 26 down to 1, reapplied them from 1 to 26, and
reproduced the source the round built, matching every earlier round's retained files
along the way. The base source the rounds start from already contains patches 1 and 2;
that is an inference from the reversed state, and the fully reversed series was not
compared with a pristine upstream `24439066` checkout. Digests are of the files as they
are now, after the 2026-11-19 notice headers.

| Order | Patch | SHA-256 | Added in |
| ---: | --- | --- | --- |
| 1 | `omp-startup-connection-warmups.patch` | `87cd31029848b30e07a78f1b694afe98ca18418950c5239f32234691e1342b52` | `bfa5eed` |
| 2 | `omp-startup-auth-retirement.patch` | `1572ad74ef2aa50aa46ece6bd3efcc3eca5dd1c6ec25eb5f91b5b30cfdc445b4` | `9767d4e` |
| 3 | `omp-startup-refusal.patch` | `63972535da7830d22ce87fbf6dd97692359afbdeabef4c8b0ed99be5607c5530` | `6f3b8a1` |
| 4 | `omp-restricted-startup.patch` | `69a577ed4f476a3a8b23681a7f5d89d5559eedd022bed20460e7d76d99111051` | `bccd281` |
| 5 | `omp-startup-discovery-retirement.patch` | `7af5c8aa3c96578b62718a5b8009028c6565dca0c07de682d9438ce9ac54ccd3` | `b8738ef` |
| 6 | `omp-startup-settings-retirement.patch` | `8ecbe6e9c1441bef912aa60d46802624ec8abbd0c99d4342bd0f6a89fcf5ba69` | `a28b729` |
| 7 | `omp-startup-empty-auth.patch` | `14712c04fd62ab04c7c7ef9bfbb071258b4d6ac89ee3a8ebb97fb287d4e19cfb` | `a0797d4` |
| 8 | `omp-startup-presence-retirement.patch` | `f4a825a20935df8577ae7d1836e08533d3350c75dd44638a7e28ce6cf99ba41e` | `a916a4e` |
| 9 | `omp-startup-telemetry-exclusion.patch` | `60012eeff06a95a6b9770bfa775289bdbc4f36cc6286e99c8b4f030f97da86cf` | `06e1a26` |
| 10 | `omp-startup-marketplace-exclusion.patch` | `37d568ebc986c69827a75c2d2864349d56c89c246d0c03f9044c446710cd2e68` | `bdfd183` |
| 11 | `omp-startup-logging-owner.patch` | `c0e11ea59d87ff4a488769385d05980588fdccaab47b7990c021e04937a00d86` | `d61c156` |
| 12 | `omp-startup-plugin-roots-owner.patch` | `a1cd09125f05a838a42b5a3ffb4f1e90e7869f4416c9c548eff1c58f23f78e67` | `1d5a704` |
| 13 | `omp-startup-extension-owner.patch` | `39c85ecb1ac659e61e4d46cc6eda3492fd35fcf20c3fe26c4ca27db4ee237f28` | `e41df2f` |
| 14 | `omp-startup-managed-timer-exclusion.patch` | `c0ee27a10b70f20aedc93ab88f75525de627816ebfe3d0741b94b61897a0857e` | `f30c2ee` |
| 15 | `omp-startup-workspace-scan-exclusion.patch` | `a6ac4ff9c6258f86227ff91bd32a6394a886e7723c6a14632df5c4459c898822` | `6b7bc4b` |
| 16 | `omp-startup-rpc-observation.patch` | `817f456650046691ad4afb4b51030cd4ecb458760a85c24310bb4a745c613a02` | `463707d` |
| 17 | `omp-startup-sdk-export.patch` | `1d369482695346cab7e0972299e1bdd30d25c1c79b623f9ede12c2ce2547553b` | `cca86fb` |
| 18 | `omp-startup-catalog-spotlight.patch` | `c42a9028ce170cf5c5cba4d788b5d8de0efd64e0d228ea3c0948798182baeef0` | `756c742` |
| 19 | `omp-startup-rpc-runtime-exclusion.patch` | `9f982ac9fd455602c400b1267f22f0c1227900e47565a97e9f5808e69a19ceb8` | `a9c1ce5` |
| 20 | `omp-startup-advisor-exclusion.patch` | `3a07eb7dfbdfeee2cc934eab9a953b99b607829e4c7698ea87ffc24a67c9f003` | `32f29db` |
| 21 | `omp-startup-prompt-exclusion.patch` | `5b34236040774f49839c96799178572965a7d334d73fbe4e26a8c2fc2eb8782a` | `7b9b6a0` |
| 22 | `omp-startup-fixed-runtime-providers.patch` | `e6c6cd9a3a9296ae76721585bbd1714710b56715f8b4b05be3855fe80615051d` | `e66f542` |
| 23 | `omp-startup-dap-idle-sweep.patch` | `b0e58e6b5809fac95fb17241c40dc70a92810e860629ad1efe6d115673e2e984` | `e3950d2` |
| 24 | `omp-startup-fixed-discovery-plan.patch` | `2f7bb5cf5812ce8ba9460fd0c74c7b9c2d37caddf3847b3217194045eacf2556` | `ca87f29` |
| 25 | `omp-startup-prompt-scheme-exclusion.patch` | `27192794bcaf198c6bf9f806ff71fe4aab9a0d01ae376011f79a951eb0144388` | `2336350` |
| 26 | `omp-startup-extension-notifications.patch` | `0f25b99202ef77cb7a9413dc96da91bc848f9110eed53421cf032d7f6f2c9d4b` | `06a9714` |
