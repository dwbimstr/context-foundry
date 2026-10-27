# Dependencies and licenses

This inventory records Cargo metadata for the locked dependency graph. It is not a legal or vulnerability audit. No dependency sources are vendored. Binary distribution must retain the applicable license/notice texts from the selected graph.

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
PR's tree-sitter-scala 0.26.3 is not applied: a grammar change can change units, which
[§ Index version gate](../specs/001-source-state-recovery/contracts/context-v2.md#index-version-gate)
and the partition recipes' `SOURCE_UNIT_SCHEMA` tie to schema bumps, an owner decision.

The 2026-10-26 maintenance subset of PR #37 updates rmcp to 3.6.0
(rmcp-macros 3.7.0), Tokio to 1.54.1, and ureq to 3.5.0 (ureq-proto 0.6.5).
Licenses are unchanged; rmcp adds an edge to the already locked libc.

On 2026-10-26 pulldown-cmark moved to 0.13.5 (security remediation, default
features still off): 0.13.4's default parser exits 101 on a 13-byte
Markdown list/indentation input, and its complexity fixes are upstream, not
locally timed. Because the fix changes Markdown units, the same change bumps
the derived search schema to `"8"` and `SOURCE_UNIT_SCHEMA` to `"6"`
([§ Unit-grammar repair](../specs/001-source-state-recovery/contracts/context-v2.md#unit-grammar-repair--2026-10-26));
older stores repair explicitly and semantic partitions under `units5` refit
at the next `semantic prepare`. tree-sitter-scala 0.26.3 remains held: its
grammar change can change units the same way, an owner decision.

| Package | Locked version | Declared license | Use |
| --- | --- | --- | --- |
| anyhow | 1.0.104 | MIT OR Apache-2.0 | normal |
| axum | 0.8.9 | MIT | normal (shared-owner HTTP) |
| clap | 4.6.7 | MIT OR Apache-2.0 | normal |
| futures-util | 0.3.34 | MIT OR Apache-2.0 | normal |
| http | 1.5.0 | MIT OR Apache-2.0 | normal |
| ignore | 0.4.33 | Unlicense OR MIT | normal |
| imbl | 7.0.2 | MPL-2.0+ | normal (009 T008 persistent location snapshots; unmodified) |
| libc | 0.2.191 | MIT OR Apache-2.0 | normal (held-root opens) |
| pulldown-cmark | 0.13.5 | MIT | normal (Markdown sections; default features off) |
| redb | 4.4.0 | MIT OR Apache-2.0 | normal |
| rmcp | 3.6.0 | Apache-2.0 | normal (server and live-host client); dev adds child-process transport |
| serde | 1.0.229 | MIT OR Apache-2.0 | normal |
| serde_json | 1.0.154 | MIT OR Apache-2.0 | normal |
| sha2 | 0.11.1 | MIT OR Apache-2.0 | normal |
| tantivy | 0.26.2 | MIT | normal |
| tiktoken-rs | 0.12.1 | MIT | normal |
| tokio | 1.54.1 | MIT | normal; dev adds process |
| tokio-util | 0.7.20 | MIT | normal |
| tree-sitter | 0.26.13 | MIT | normal (syntax runtime; third-party C) |
| tree-sitter-c | 0.24.2 | MIT | normal (C grammar) |
| tree-sitter-cpp | 0.23.4 | MIT | normal (C/C++ grammar) |
| tree-sitter-go | 0.25.0 | MIT | normal (Go grammar) |
| tree-sitter-java | 0.23.5 | MIT | normal (Java grammar) |
| tree-sitter-javascript | 0.25.0 | MIT | normal (JavaScript/JSX grammar) |
| tree-sitter-python | 0.25.0 | MIT | normal (Python grammar) |
| tree-sitter-rust | 0.24.2 | MIT | normal (Rust grammar) |
| tree-sitter-typescript | 0.23.2 | MIT | normal (TypeScript/TSX grammars) |
| ureq | 3.5.0 | MIT OR Apache-2.0 | normal (bounded gateway-launcher health check) |
| uuid | 1.28.0 | Apache-2.0 OR MIT | normal |
| reqwest | 0.13.5 | MIT OR Apache-2.0 | dev |
| tempfile | 3.27.0 | MIT OR Apache-2.0 | dev |
| toml | 1.1.8 | MIT OR Apache-2.0 | dev |

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
The feature adds one optional build dependency, `bindgen` 0.73 (BSD-3-Clause), for the
C bindings; default and Rust 1.90 builds do not compile it. PyO3 and the MLX runtime
closure are no longer product dependencies.


## Legacy prototype and planned optional components

Laya remains a separately licensed, read-only research reference, not a Cargo
dependency or deployed service. The product uses ureq for the gateway launcher's
bounded health check. The product Cargo graph excludes
LibTorch from default-feature builds. Its optional `learning-worker` and the separate
[probe crate](../tools/feasibility/README.md) now pin experimental tch 0.26.0 with
LibTorch 2.13.0; see the [compatibility review](review/tch-026.md).
The probe also pins PyO3 0.29.2, USearch 2.26.2 and rmcp 3.5.0 (the product has
since moved to rmcp 3.6.0 and USearch 2.26.5); its lock is not the product lock. Contract 013 v4's accepted
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
