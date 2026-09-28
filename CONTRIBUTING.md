# Contributing

Keep changes small enough to explain through a user-visible behavior, its owner,
and a meaningful check. Discuss changes to source identity, durable state or the
public protocol before implementing them. A separate spec hierarchy is unnecessary
for a small local change.

Use Rust 1.90 or newer, `cargo fmt --check`,
`cargo clippy --locked --all-targets -- -D warnings`, and `cargo test --locked`.
Include a focused regression test for a correctness bug. Do not add a long
benchmark to validate a change that an acceptance test can decide.

Keep datasets, transcripts, private repositories, model weights and real credentials
out of patches. Use small original/public fixtures with clear rights. Contributions
to this project are under its MIT license; dependencies keep their own licenses.
No contributor agreement or public contribution endpoint has been established yet.

Before the first public source release, the owner should review the actual Git
contents and destination. `publish = false` is intentional until the package name,
release metadata and distribution are finalized. Creating this local repository
does not publish it or create an account/organization on a hosting service.
