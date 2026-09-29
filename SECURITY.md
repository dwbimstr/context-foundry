# Data and security boundaries

The first slice is a local tool for an operator-controlled workspace. It does not
execute indexed source, load repository plugins, start a listener or collect
telemetry. An explicit Laya port permits a request containing the query to a local
HTTP server. Training feedback is retained locally; export is an explicit command.

Indexed source and imported graph text are untrusted data. Their presence in a
cited bundle does not turn them into instructions. This tool cannot guarantee that
a consuming model ignores malicious prose, nor does it sanitize arbitrary prose
secrets or PII. Filename and private-key-marker exclusions are narrow deny rules.
Choose the indexed workspace and ignore rules accordingly.

Use an owner-private location for stores and exported datasets. Filesystem
permissions and disk encryption remain the operator's responsibility. There is
no encryption-at-rest layer, user authentication system, multi-tenant isolation or
protection against a hostile process concurrently replacing workspace path components.
Do not expose an unauthenticated Laya server beyond loopback.

Ignore rules prevent ordinary Git additions of stores, models, runs and local
review material; they do not prevent `git add --force` or deliberate export. Training
consent withdrawal affects future exports, not previously distributed data or weights.

The preceding behavior describes the current prototype. The proposed
[owned learning design](specs/013-owned-learning/spec.md) retires its Laya HTTP path
and requires verified platform isolation for optional Rust workers. The
[deployment contract](docs/deployment.md) defines explicit inputs, denied network/
credentials/store access, resource enforcement and package lifecycle. These jails
are not implemented or validated yet. No claim of isolation follows from Rust,
process separation or a VM alone. Baseline indexing still executes no workspace code.
The proposed gateway explicitly processes permitted model requests and holds a
provider key in its own process; those grants never reach training/inference workers.
Its authenticated loopback boundary is not multi-user isolation. Receipt logs omit
bodies and disclose unknown usage. No gateway or network configuration has been applied.

Report vulnerabilities through
[GitHub private vulnerability reporting](https://github.com/dwbimstr/context-foundry/security/advisories/new).
Private reporting is enabled for this repository. Never put a private dataset,
credential or exploitable secret in a public issue.
