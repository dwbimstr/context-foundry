# Release the scope that works

A release declares its version, included behavior and supported environments. Future
specs do not gate it. This checklist replaces spec 015; it is not another product.

1. Record the selected scope, actual acceptance evidence and known limitations. Verify
   advertised integrations with their real consumer; do not make untested claims.
2. Run the applicable full Rust checks and build/install smoke on declared platforms.
   Include relevant restart/data-preservation checks. Reuse valid feature evidence;
   no blanket predecessor ladder or close-time benchmark is inherited.
3. Document compatibility, preserved user data, recovery/rollback and install/remove
   ownership. Until upgrade support exists, say so; do not silently delete feedback
   or memories to rebuild a store. A source-only preview can use fresh-store installs.
4. Inspect the actual artifact/Git contents, dependency licenses, notices and public
   fixtures. No private datasets/transcripts/weights/credentials or incompatible copies.
5. Prepare release notes and the concrete distribution artifact. Publish only to the
   owner-selected destination with authorization. Local preparation is not publication.

For advertised bootstrap, adapter budgeting or owned workers, use the
[deployment contract](deployment.md): install the actual package, verify target-specific
isolation and resource controls, demonstrate bootstrap/connect/budget behavior, then
upgrade/rollback/uninstall without user-data loss. Core-only distributions need no ML
runtime or VM. Failed optional isolation blocks that feature, not the baseline release.
The learning package must use the owned Rust worker; the legacy Laya client is not
evidence for it. A raw subprocess is not a successful jail test.
An advertised gateway additionally proves its actual host/API streaming/tool round,
private credential boundary, input/output admission, unknown-usage handling and exact
host-config restoration on opt-out. Missing integration proof blocks that gateway
claim, not CLI/MCP. Gateway credentials never enter packaged data or worker grants.

The current repository has no published release. `v0.1.0` is a local tag with a
macOS arm64 artifact (owner decision 2026-10-04; not published or pushed); its
checklist evidence is the first section of [validation](validation.md). Planned
features beyond its scope remain unimplemented.
The CLI can ship before MCP; MCP before compiler-backed graphs; each before learning.
