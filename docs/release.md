# Release when every active spec is complete

A release declares its version, included behavior and supported environments. It ships
only when every task of every active spec in the [portfolio](../specs/README.md) is
implemented and accepted (owner policy 2026-10-04); deferred and superseded specs are
excluded. This checklist replaces spec 015; it is not another product.

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
runtime or VM. Failed optional isolation blocks that feature and therefore the release.
The learning package must use the owned Rust worker; the legacy Laya client is not
evidence for it. A raw subprocess is not a successful jail test.
An advertised gateway additionally proves its actual host/API streaming/tool round,
private credential boundary, input/output admission, unknown-usage handling and exact
host-config restoration on opt-out. Missing integration proof blocks that gateway
and therefore the release. Gateway credentials never enter packaged data or worker grants.

The current repository has no release. A local `v0.1.0` tag and artifact made on
2026-10-04 under the earlier release-the-working-scope rule were withdrawn the same day
when the owner adopted this policy; their checklist record remains in
[validation](validation.md) as history.
