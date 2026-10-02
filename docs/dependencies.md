# Dependencies and licenses

This inventory records Cargo metadata for the locked dependency graph. It is not a legal or vulnerability audit. No dependency sources are vendored. Binary distribution must retain the applicable license/notice texts from the selected graph.

## Direct dependencies

Generated from `cargo metadata --locked` on 2026-10-01 (001 + 003 implementation).
"dev" packages build tests only and are not linked into the shipped binary; rmcp and
tokio add client/process features only for tests.

| Package | Locked version | Declared license | Use |
| --- | --- | --- | --- |
| anyhow | 1.0.104 | MIT OR Apache-2.0 | normal |
| axum | 0.8.9 | MIT | normal (shared-owner HTTP) |
| clap | 4.6.7 | MIT OR Apache-2.0 | normal |
| futures-util | 0.3.34 | MIT OR Apache-2.0 | normal |
| http | 1.5.0 | MIT OR Apache-2.0 | normal |
| ignore | 0.4.33 | Unlicense OR MIT | normal |
| libc | 0.2.189 | MIT OR Apache-2.0 | normal (held-root opens) |
| redb | 4.3.0 | MIT OR Apache-2.0 | normal |
| rmcp | 3.5.0 | Apache-2.0 | normal (server, stdio, streamable HTTP server); dev adds client |
| serde | 1.0.229 | MIT OR Apache-2.0 | normal |
| serde_json | 1.0.151 | MIT OR Apache-2.0 | normal |
| sha2 | 0.10.9 | MIT OR Apache-2.0 | normal |
| tantivy | 0.26.2 | MIT | normal |
| tiktoken-rs | 0.12.1 | MIT | normal |
| tokio | 1.53.1 | MIT | normal; dev adds process |
| tokio-util | 0.7.19 | MIT | normal |
| ureq | 3.4.2 | MIT OR Apache-2.0 | normal (legacy Laya protocol module) |
| uuid | 1.26.1 | Apache-2.0 OR MIT | normal |
| reqwest | 0.13.5 | MIT OR Apache-2.0 | dev |
| tempfile | 3.27.0 | MIT OR Apache-2.0 | dev |
| toml | 0.8.23 | MIT OR Apache-2.0 | dev |

The locked graph has 286 third-party packages. Declared expressions are permissive
(MIT/Apache-2.0 variants, Unlicense, Unicode-3.0, Zlib, BSD-3-Clause, BSL-1.0) or offer
a permissive choice (one `MIT OR Apache-2.0 OR LGPL-2.1-or-later`, one
`Apache-2.0 / MIT / MPL-2.0`). rmcp is Apache-2.0 only: a binary distribution must
carry its license text and any NOTICE. This is metadata, not a legal audit.

## Legacy prototype and planned optional components

The legacy Laya protocol module (`ureq` client) remains in the library with its
protocol test, but the CLI no longer reaches it; Laya is not a Cargo dependency and
remains a separately licensed research reference. The product Cargo graph excludes
ML/sandbox dependencies. A separate [probe crate](../tools/feasibility/README.md)
pins tch 0.24.0, PyO3 0.29.2 and USearch 2.26.2 (plus the rmcp 3.5.0 version the
product now uses); its lock is not the product lock. Contract 013 v4 selects
LibTorch 2.11.0 for ModernBERT/head work. USearch has a third-party native C++ core;
this does not introduce first-party C++ code. Libkrun remains conditional.
The selected third-party MLX loader was exercised through Rust PyO3, but private
runtime distribution remains unaccepted. Record transitive libraries, notices and any VM image/kernel license
inventory when those packages are actually selected. No weights or private data are
redistributed by this plan; no dependency has been added merely by documenting it.
The proposed optional gateway (003 T004, not implemented) also needs locked Rust
HTTP/TLS/SSE client libraries and a verified provider schema/counting implementation.
The shared MCP owner's loopback HTTP server stack does not establish that client path.

Use `cargo metadata --locked` for the full transitive graph. `Cargo.lock` pins it.
