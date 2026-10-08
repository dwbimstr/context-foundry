## What changed

<!-- One behavior per pull request. -->

## Why

<!-- The user-visible effect, and the issue or spec it belongs to (specs/NNN-.../spec.md). -->

## Evidence

<!-- Tests added or run (for example `cargo test --locked --test <file>`), output that shows the change,
     and anything you did not verify. Label fixtures as fixtures. -->

## Checklist

- [ ] `cargo fmt --check`
- [ ] `cargo clippy --locked --all-targets -- -D warnings`
- [ ] Tests for the changed behavior pass; a bug fix includes a regression test that fails without it
- [ ] Docs touched: `node scripts/check-links.mjs` reports no errors
- [ ] No private source code, datasets, transcripts, credentials or model files
