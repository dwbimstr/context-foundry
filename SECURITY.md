# Data and security boundaries

The current implementation is a local tool for an operator-controlled workspace. It
does not execute indexed source, load repository plugins or collect telemetry. It
starts a network listener only when explicitly run as a shared MCP owner
(`mcp --transport streamable-http`). The legacy Laya HTTP client is no longer
reachable from the CLI. Training feedback is retained locally; export is explicit.

Indexed source and imported graph text are untrusted data. Their presence in a
cited bundle does not turn them into instructions. This tool cannot guarantee that
a consuming model ignores malicious prose, nor does it sanitize arbitrary prose
secrets or PII. Filename and private-key-marker exclusions are narrow deny rules.
Choose the indexed workspace and ignore rules accordingly.

Use an owner-private location for stores and exported datasets. Filesystem
permissions and disk encryption remain the operator's responsibility. There is
no encryption-at-rest layer, user authentication system or multi-tenant isolation.
Candidate bytes are read relative to a held root with no-follow opens, but a
same-user process that swaps and restores workspace directories during a scan can
make that scan retire source records of files that still exist (see 001).

The shared MCP owner binds IPv4 loopback only and requires a bearer token read from
a named environment variable on every request before any session is allocated; it
rejects a mismatched `Host`, any `Origin`, oversized bodies and newer stateless
protocol requests. Printed host configuration carries the variable name, never the
token. This is a same-machine boundary, not multi-user isolation: a process that can
read the token can use the store's five tools. Opt-in usage receipt logs are created
`0600` and refused if readable by others; they carry identities and counts, not bodies.

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
