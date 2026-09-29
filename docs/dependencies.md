# Dependencies and licenses

This inventory records Cargo metadata for the locked dependency graph. It is not a legal or vulnerability audit. No dependency sources are vendored. Binary distribution must retain the applicable license/notice texts from the selected graph.

## Direct dependencies

| Package | Locked version | Declared license |
| --- | --- | --- |
| anyhow | 1.0.104 | MIT OR Apache-2.0 |
| clap | 4.6.7 | MIT OR Apache-2.0 |
| ignore | 0.4.33 | Unlicense OR MIT |
| redb | 4.3.0 | MIT OR Apache-2.0 |
| serde | 1.0.229 | MIT OR Apache-2.0 |
| serde_json | 1.0.151 | MIT OR Apache-2.0 |
| sha2 | 0.10.9 | MIT OR Apache-2.0 |
| tantivy | 0.26.2 | MIT |
| tempfile | 3.27.0 | MIT OR Apache-2.0 |
| tiktoken-rs | 0.12.1 | MIT |
| ureq | 3.4.2 | MIT OR Apache-2.0 |

## Legacy prototype and planned optional components

The current prototype can contact a separately installed Laya server; it is not a
Cargo dependency. The owned-learning plan replaces this path instead of distributing
Laya or editing its repository. Laya remains a separately licensed research reference.
The product Cargo graph still excludes ML/sandbox dependencies. A separate
[probe crate](../tools/feasibility/README.md) pins tch 0.24.0, PyO3 0.29.2, rmcp 3.5.0
and USearch 2.26.2; its lock is not the product lock. Contract 013 v4 selects
LibTorch 2.11.0 for ModernBERT/head work. USearch has a third-party native C++ core;
this does not introduce first-party C++ code. Libkrun remains conditional.
The selected third-party MLX loader was exercised through Rust PyO3, but private
runtime distribution remains unaccepted. Record transitive libraries, notices and any VM image/kernel license
inventory when those packages are actually selected. No weights or private data are
redistributed by this plan; no dependency has been added merely by documenting it.
The proposed optional gateway also needs locked Rust HTTP/TLS/SSE libraries and a
verified provider schema/counting implementation. The current HTTP client dependency
alone does not prove server/streaming/TLS compatibility. No proxy package is installed.

Use `cargo metadata --locked` for the full transitive graph. `Cargo.lock` pins this initial build.
