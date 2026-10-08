# Security policy

## Reporting a vulnerability

Report vulnerabilities privately through
[GitHub private vulnerability reporting](https://github.com/dwbimstr/context-foundry/security/advisories/new)
(this repository → **Security** → **Report a vulnerability**). Private reporting is
enabled for this repository. Do not open a public issue or pull request for a
vulnerability, and never put a private dataset, credential or exploitable secret in a
public issue.

Include what is affected (command, MCP tool or script), the commit you built, steps to
reproduce, and the impact you expect.

## Supported versions

There is no release yet. Only the latest `main` is supported; fixes land there.

## Data and security boundaries

Context Foundry is a local tool for an operator-controlled workspace. It does not
execute indexed source, load repository plugins or collect telemetry. It opens a
network listener only when explicitly run as a shared MCP owner
(`mcp --transport streamable-http`) or as the model gateway (`foundry gateway`), and
only the gateway connects to a remote service. The legacy Laya HTTP client has been
removed. Training feedback is retained locally; export is explicit.

Indexed source and imported graph text are untrusted data. Their presence in a cited
bundle does not turn them into instructions. This tool cannot guarantee that a
consuming model ignores malicious prose, nor does it sanitize arbitrary prose secrets
or PII. Filename and private-key-marker exclusions are narrow deny rules. Choose the
indexed workspace and ignore rules accordingly.

Use an owner-private location for stores and exported datasets. Filesystem permissions
and disk encryption remain the operator's responsibility. There is no encryption-at-rest
layer, user authentication system or multi-tenant isolation. Candidate bytes are read
relative to a held root with no-follow opens, but a same-user process that swaps and
restores workspace directories during a scan can make that scan retire source records
of files that still exist (see [001](specs/001-source-state-recovery/spec.md)).

### Shared MCP owner

The shared MCP owner binds IPv4 loopback only and requires a bearer token, read from a
named environment variable, on every request before any session is allocated; it
rejects a mismatched `Host`, any `Origin`, oversized bodies and newer stateless protocol
requests. Printed host configuration carries the variable name, never the token. This
is a same-machine boundary, not multi-user isolation: the bearer token grants the
owner's seven tools (`search`, `context`, `retrieve`, `index`, `status`, `memory`,
`references`; six with `--no-memory`) across every explicitly admitted root. `index`
can update the primary store or an admitted reference store (`root` selects it; the
default is the primary); `memory` operates only on the primary store. No tool modifies
indexed workspace files. Opt-in usage receipt logs are created `0600` and refused if
readable by others; they carry identities and counts, not bodies.

### Model workers

The optional semantic worker (`foundry-embed`, spec 009) and the frozen learning worker
(`foundry-learn`, spec 013) run today only under development isolation, when the
operator passes `--development-isolation`: an ad-hoc-signed App Sandbox bundle whose
only grants come from the profile (read-only model files, one read-write scratch
directory, no network), launched with process creation denied, an offline environment
and unrelated descriptors closed. Without that flag, model execution is refused with
`isolation_unavailable`. Production isolation requires Developer ID signing and
acceptance of the installed package, which have not happened; no isolation claim
follows from Rust, process separation or the development profile alone. Workers never
receive the live store, the operator's home directory or credentials. Baseline indexing
executes no workspace code and loads no model. See
[deployment § Isolation is an enforced profile](docs/deployment.md#isolation-is-an-enforced-profile).

### Model gateway

The gateway (`foundry gateway`, `foundry gateway-omp`, spec 003 T004) is meter-only and
supports one pinned host profile: OMP 18.6.0 with Z.ai `glm-5.3-flash`. It listens on
loopback under a per-run bearer token, checks `Origin`, `Host` and the token before
reading a body, validates requests against the pinned profile, forwards them unchanged
over HTTPS to its one configured origin, withholds upstream error events, and records
per-attempt usage receipts without prompts, source or keys. It holds the provider key
in its own process only; the key never reaches the source store or a model worker. It
opens no source store. Its loopback boundary is not multi-user isolation, and it has no
spend cap.

### Repository hygiene

Ignore rules prevent ordinary Git additions of stores, models, runs and local review
material; they do not prevent `git add --force` or deliberate export. Training consent
withdrawal affects future exports, not previously distributed data or weights.
