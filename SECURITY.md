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

No public security contact has been configured because the repository has not
been published. Before publication, configure a private vulnerability-reporting
channel on the selected host. Never put a private dataset or exploitable secret in
a public issue.
