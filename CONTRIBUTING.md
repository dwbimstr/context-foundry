# Contributing

Keep changes small enough to explain through a user-visible behavior, its owner,
and a meaningful check. Discuss changes to source identity, durable state or the
public protocol before implementing them. One spec can hold behavior, design, work and evidence. The [portfolio](specs/README.md) and
[modified Spec Kit workflow](.specify/README.md) organize feature work. A small repair
updates its existing contract rather than creating a new numbered spec. Proposed
roadmap bundles are not claims that their implementation has been approved or built.

Use Rust 1.90 or newer, `cargo fmt --check`,
`cargo clippy --locked --all-targets -- -D warnings`, and `cargo test --locked`.
Include a focused regression test for a correctness bug. Apply the
[constitution's evidence policy](.specify/memory/constitution.md) to the changed scope;
the commands above are the full Rust check set, not a requirement for documentation
edits. Do not add a long benchmark to validate a change that an acceptance test can decide.

Keep datasets, transcripts, private repositories, model weights and real credentials
out of patches. Use small original/public fixtures with clear rights. Contributions
to this project are under its MIT license; dependencies keep their own licenses.
No contributor agreement or public contribution endpoint has been established yet.

Before the first public source release, the owner should review the actual Git
contents and destination. `publish = false` is intentional until the package name,
release metadata and distribution are finalized. Creating this local repository
does not publish it or create an account/organization on a hosting service.
