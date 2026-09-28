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

## External runtime

Laya is separately installed and is not a Cargo dependency. Its source code is Apache-2.0; checkpoint and training-data rights must be checked separately. No weights, tokenized datasets or predecessor source are redistributed here.

Use `cargo metadata --locked` for the full transitive graph. `Cargo.lock` pins this initial build.
