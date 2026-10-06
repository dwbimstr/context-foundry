# Shared context contract v2

Status: approved 2026-10-03 (token-economics spec pass). **001 T004–T006 are locally
implemented, accepted at the reviewer's SHIP and committed in `5edf32c`; unreleased.**
007 T001 (multi-root) and the T005 leading-run amendment are implemented and accepted
locally on 2026-10-04 and committed (`cc402e0`, `bd1d890`). The 2026-10-04 amendments below record owner
decisions and implemented details from T005/T006 review. The 2026-10-06 amendment
(owner-approved after the 013 corpus analysis) changes tier-1 run selection and order,
adds five route keywords, accepts an MCP `lines` array and makes the empty `lines`
refusal name the handle's lines.
It replaces [v1](context-v1.md), whose JSON wire is historical at
`6bb81e6`, and carries forward every still-valid v1 rule. Source/CLI owner:
[001](../spec.md); MCP adapter owner:
[003](../../003-agent-retrieval-context/spec.md), whose
[economics contract](../../003-agent-retrieval-context/contracts/adapter-economics.md)
owns session allowances and delivered-token evidence; multi-root admission, aliases
and merging: [007](../../007-multi-workspace-context/spec.md). 001 does not depend on
implementing MCP. Limits are selected engineering bounds, not measured capacity promises.

| Part | Owning task | Current state |
| --- | --- | --- |
| Identity and scope, final-read validation, failure scope, input validation, CLI errors, strategy routing, partial index report | 001 T001–T003, 003 T001–T002 | Implemented and verified 2026-10-01; carried forward |
| v2 handle, text wire, counting boundary, budgeted search, retrieve `lines`, atomic allowance | 001 T004 | Locally implemented and verified on final4; unreleased |
| Syntax units, search index v2, two-tier ranking, `path` filter, locator labels; leading-run units and head line (schema `"3"`) | 001 T005 | Locally implemented and accepted (r3 SHIP; leading-run amendment delta SHIP 2026-10-04, committed in `bd1d890`); unreleased |
| Outlines, forms ladder, candidate seam, retrieve `view` | 001 T006 | Locally implemented and accepted (r3 SHIP); unreleased |
| Multi-root identity, `roots`/`root`, per-root header | 007 T001 | Locally implemented and accepted 2026-10-04 (delta SHIP), committed in `cc402e0`; unreleased |
| Tier-1 marked runs and specificity order, route keywords, MCP `lines` array, empty-selection message (2026-10-06 amendment) | 001 (amendment) | Implemented locally 2026-10-06; awaiting gates and review; unreleased |
| `foundry references` header segments 12–14, `next: after=<path>#<start>-<end>` cursor; MCP `references`, `index.scip`, compiler graph context | 005 T002, T003 | Locally implemented and accepted (CLI 2026-10-04; MCP and graph context 2026-10-05); unreleased |

## Identity and reference validation

`workspace_id` is lowercase SHA-256 of the bound canonical absolute root's UTF-8
bytes. It scopes references to a logical local workspace, not a global registration
service. A moved root requires a new store; no implicit rebind. A copied store retains
its binding. `source_revision` is a checked u64 incremented atomically for each source
add/change/delete, beginning at 0 when the store is initialized. Search refresh, graph
import, memory and feedback do not increment it. Results identify the revision they
read; it is not a claim about unsaved/unindexed filesystem changes.

The bound root, not the caller's current directory or paths mentioned in a session,
defines source ownership. Query text, source imports, graph references and feedback
are never instructions to open another path, widen the root or enroll another repo.
An absolute external path inside `query` is ordinary search text; it is not an error
or a filesystem read. A foreign-workspace handle returns `wrong_workspace` after
field validation. Explicit indexing of a different root refuses before changing
source rows, revision or pending work. A later shell `cd` does not rebind the store.

An external file read by the host remains host-supplied context, outside this store's
freshness guarantee. Another repository is served either from its own separately
addressed store or, inside one response, as a reference root explicitly admitted at
owner launch under [007](../../007-multi-workspace-context/spec.md); query text never
admits a root. Multi-root responses merge rankings under one budget; they add no
cross-repo symbol resolution, cross-root edges or shared snapshot. Reconciliation or
deletion in one store cannot affect another. Watchers remain deferred; any future
watcher must use the same bound root and ingestion owner, never infer roots from text.

### Source handles

A source handle is the string `<path>#<start>-<end>@<sha32>.<ws16>`:

- `path`: the normalized workspace-relative UTF-8 path using `/`, 1..4096 bytes, with
  no empty, `.` or `..` component, leading `/`, NUL, CR or LF (the rule every admitted
  source path already passes). Other bytes, including `#`, `@`, `.`, spaces and
  JSON-special characters, appear unescaped.
- `start`, `end`: u64 half-open byte offsets in decimal without leading zeros (`0` is allowed).
- `sha32`: the first 32 lowercase hex characters (128 bits) of the source's SHA-256.
- `ws16`: the first 16 lowercase hex characters (64 bits) of `workspace_id`.

Parse with the end-anchored regular expression
`#(0|[1-9][0-9]*)-(0|[1-9][0-9]*)@([0-9a-f]{32})\.([0-9a-f]{16})$`; the path is
everything before the match. Handle input is capped at 4200 bytes: a 4096-byte path
plus the longest 92-byte suffix. Uppercase hex fails the pattern and is
`invalid_argument`. A v1 JSON object is `invalid_argument` with the message "handle
must be a v2 string `path#start-end@sha32.ws16`".

Validate in this order; failures return no partial evidence:

1. syntax and bounds, including `start > end` → `invalid_argument`;
2. workspace: `ws16` equals the prefix of the bound root's `workspace_id` (in a
   multi-root owner, of an admitted root's), else `wrong_workspace`;
3. existence, else `not_found`;
4. digest: `sha32` equals the prefix of the stored full SHA-256, else `stale_handle`;
5. range: `0 <= start < end <= source_bytes` on UTF-8 boundaries, where the only empty
   range is `[0,0)` of an empty source, else `invalid_range`.

An inverted interval is malformed independent of any source and fails stage 1; only
checks that need the source (end beyond its length, UTF-8 boundaries, `[0,0)` on a
nonempty source) are `invalid_range`. Citations show one-based inclusive line numbers;
byte ranges are authoritative. Preserve CRLF and all source bytes.

Recorded rationale: digests are references compared with the full stored values, not
credentials. Accidental collision is at most 2^-128 per pair of source versions and
2^-64 per pair of workspaces; a deliberate source collision needs about 2^64 hash
evaluations; a handle grants nothing beyond bytes already readable through an
admitted root. Full 64-hex identities stay in `status` and authoritative rows. If the
owner later prefers full identities in handles, only this grammar and the cap (4300
bytes) change. The T004 `examples/workspace` measurement is 33 o200k tokens
(68 bytes) for the v2 handle against 94 (207 bytes) for the v1 JSON handle,
superseding the earlier 29/85 estimate. This fixture-specific measurement is not a
fixed cost for every path or a host/provider savings claim; see
[validation](../../../docs/validation.md).

### Multi-root identity

In a multi-root owner ([007](../../007-multi-workspace-context/spec.md)) a handle names
its root through `ws16`. A `ws16` matching no admitted root is `wrong_workspace`; a
known root whose store cannot serve reads is `root_unavailable`. Each candidate is
revalidated in its own root's final read transaction; there is no global snapshot
across roots. Graph expansion and endpoint validation stay inside the seed's root, and
graph deduplication keys on `(workspace_id, stable fact identity)`, so identical rows
in two roots stay distinct. No cross-root edge exists until a producer artifact
establishes one. Re-indexing one root never touches another.

### One final read

One response validates source hashes, memory revisions, selected graph snapshot and
semantic mappings against one final authoritative read transaction per root. Report
that transaction's source revision. Provider work runs before this transaction; never
pin it while waiting for a model. Candidates collected earlier must be revalidated here.
A changed/deleted candidate is omitted with the existing stale/coverage reason, not
relabeled with the new revision. No retry loop to chase a moving workspace, global
publication epoch or promise of current-disk freshness is added. Old successful replies
remain historical observations; later `retrieve` revalidates their handles.

Reconstruct source in stored chunk ordinal order (at most 2 MiB) and verify full
length/hash before slicing. Missing/mismatched chunks are `corrupt_source`, never
an empty successful result. No new offset table is required for this bound. Storage
chunks remain internal 2048-byte blocks: they are not reference identities, search
documents or embedding units.

## Inputs, defaults and outputs

Reject missing required fields, null, wrong types, blank queries, non-finite numbers,
unknown application fields and out-of-range integers before mutation. Optional means
omittable, not null. Inputs are UTF-8; do not reinterpret query text as a shell command.

| Operation | Input / default | Successful output |
| --- | --- | --- |
| search | `query` 1..4096 bytes after the nonblank check; `limit` 1..64, default 10; `tokens` 1..32768, default 1024; `path?`; `roots?` (007) | v2 header plus one locator line per shown hit |
| context | `query`; `tokens` 1..32768, default 2048; `strategy` `auto`/`search`/`graph`, default `auto`; `roots?` (007) | v2 header plus evidence items: units, graph edges and file outlines |
| retrieve | `handle` (v2 string); `tokens` 1..32768, default 2048; `lines?`; `view?` `text`/`outline`, default `text` | v2 header plus one item, then `next:` while bytes remain (text view) |
| index | bound root (an explicit CLI root must match); MCP `timeout_ms?`, `root?` (007) | 001 scan report JSON; partial results include committed counts, reason and pending work |
| status | none | JSON: store schema, workspace ID or null if unbound, source revision/count, pending count, index state `ready`/`lagging`/`repair_required` with its reason, scan state `never`/`running`/`complete`/`incomplete`; `roots` (007) |

`roots` and `root` are MCP selectors for already admitted aliases (007), including
`primary` when no references are configured; they never admit or rebind a path.
Optional features extend this grammar rather than add another: 005 adds `references`
and `index.scip`, 008 adds `memory` and `include_memory`, 009 adds semantic items. Each
owning spec states its line form, and its output counts in the same budget.

`path` restricts both search tiers to one file or directory subtree. Strip one leading
`./` and one trailing `/`; the result must satisfy the handle path rules, else
`invalid_argument`.

`lines` is `"A"` or `"A-B"`: 1-based absolute file line numbers in decimal without
leading zeros; a malformed value or 0 is `invalid_argument`. MCP also accepts a JSON
array, read as its elements joined with `-` (2026-10-06): `[A]` is `"A"` and `[A,B]` is
`"A-B"` with the string's validation and errors (`[0]` refuses as `"0"`, `[5,3]` as
`"5-3"`), and any other array (three elements, a non-integer) is malformed, at the
same stage. The CLI is unchanged. Lines are LF-delimited: a
CR before LF belongs to its line, an unterminated last line ends at EOF, a trailing LF
creates no extra line and an empty file has no lines. The selected whole lines (lines
past the end of the file select nothing) are intersected with the handle's range,
never widening it. A > B, an empty intersection or any line of an empty file is
`invalid_range`; its message names the lines the handle's range covers and that a
handle's `#start-end` is a byte range (2026-10-06), for example `lines 650-1459 select
nothing in this handle, which covers lines 176-205; a handle's #start-end is a byte
range`. The returned item's handle names the intersection.

CLI: `foundry --store DIR search QUERY [--limit N] [--tokens N] [--path P]`,
`context QUERY [--tokens N] [--strategy S]` and `retrieve --handle H [--tokens N]
[--lines A[-B]] [--view text|outline]`. `--handle` takes the v2 string as one argument
of at most 4200 bytes, with no shell interpretation. Diagnostics stay on stderr.

CLI errors: exit 2 for invalid arguments or an unsupported version/mode, 3 for store
busy, 1 for other runtime failures, 130 for cooperative user cancellation. Error JSON
on stderr is at most 1024 bytes and names `code`, `message`, `retryable`; source bodies
are never placed in errors. A partial index report may be on stdout but the exit is
nonzero. Budget failure leaves stdout empty. Querying an unbound workspace is
`workspace_unbound`; status remains available. `verify_current` is `unsupported_mode`
(a tested refused strategy), and so is `view:"outline"` on an unmapped language.

## Wire v2

### Scope

`search`, `context` and `retrieve` successes are v2 text, identical at both boundaries:
the MCP result's single text block and the CLI's stdout. Every line, including the
last, ends with LF. An MCP success has `isError:false` and no `structuredContent` (OMP
would echo it as an extra fenced JSON block). v1's JSON success forms —
`format_version`, `search_json`, `application_item` and the `pack_*_application`
paths — are deleted rather than versioned: they were never released. `status` and
`index` keep their bounded JSON reports, including the counts-only partial error;
errors keep `{code,message,retryable}` of at most 1024 bytes; `connect`, `usage` and
`export-training` output is unchanged.

### Header line

Line 1 joins these segments with ` · ` (space, U+00B7, space), in this order; optional
segments appear only when not at their default:

1. `foundry search`, `foundry context`, `foundry retrieve`, `foundry references` (005)
   or `foundry memory` (008 memory search);
2. `r<source_revision>` for a single-root owner, or the per-root segments of a
   multi-root owner defined by [007](../../007-multi-workspace-context/spec.md);
3. `scan:<state>` when the scan state is not `complete`;
4. `pending:<n>` when n > 0;
5. `budget:<n>`, the effective budget, written `budget:<n>(ceiling)` or
   `budget:<n>(session)` when the context ceiling or the session allowance produced it
   (ties report the request, unsuffixed);
6. `shown:<k>`, the items rendered (search and context);
7. `omitted:<n>` when packing dropped constructed candidates;
8. `capped:<n>` when the per-file cap skipped hits;
9. `stale:<n>` when the final read dropped stale source or graph candidates;
10. `candidates:full` when a candidate window filled: search tier 1's 64 slots with
    definitions left over, tier 2 at 256, or a context graph examination window (32
    rows per seed and direction) that filled exactly or was truncated;
11. `graph:<ok|graph_unavailable|graph_stale|graph_invalid>` when the strategy
    resolved to graph;
12. `examined:<n>`: `foundry references` only, always present. Counts the graph records
    examined in this window, definition lookups included;
13. `unresolved:<n>`: `foundry references` only, present when n > 0. Counts examined
    occurrences of the seed symbol whose target is external, unknown or ambiguous;
14. `coverage:<complete|partial|stale|unavailable>`: `foundry references` only, always
    present and last.

For `foundry references` (decided 2026-10-04 for 005 T002; cross-lab refutation
held), segments 7–10 keep their meanings:
- `omitted` counts examined eligible references dropped by token or byte packing, not
  unexamined matches;
- `stale` counts final-read drops;
- `candidates:full` marks a filled 256-record/64-file examination window.

`examined` and `coverage` are mandatory because references is used to reason about
absence. `coverage:complete` still covers only supported indexed input inside the
examined window, so it never proves absence beyond it (§ Deduplication, no cross-call
suppression). Database and store failures stay named errors, never a
`coverage:unavailable` success. `graph:` stays context-only.

Header segments amendment, 2026-10-05 (013 T003): a context whose strategy is `auto`
on an owner or command started with a policy config that is not disabled (an invalid one included) adds one last segment,
after `graph:` and 009's `semantic:` word: `route:policy` when the policy's choice was
accepted, else `route:fallback:<reason>` with deterministic routing, the reason one of
`policy_abstained`, `policy_busy`, `policy_timeout`, `policy_insufficient_time`,
`policy_unavailable`, `policy_input_oversize`, `graph_unavailable` or `graph_stale`. An explicit strategy,
search, retrieve, references, memory, a disabled config and no config emit no segment,
so their output is byte-identical to an owner without a policy. In a multi-root owner
the policy routes the primary root only and its word carries `; primary root only`
when other roots serve, as `semantic:` does. Budget-refusal hints do not reserve room
for it (the hint header's worst-case numbers dominate).

Removed from v1: `format_version`, `tokenizer`, `boundary`, `budget_satisfied`,
`indexed_snapshot`, `candidate_limit`, per-item `workspace_id`, `strategy` (except the
graph segment), `context_id` and `budget_scope`. No delivery ID is emitted: this
contract supports delivery scope only, `host_request` stays refused at startup, and a
future host-request integration owns conditional delivery IDs.

### Evidence items

Each context and retrieve item is an item line followed by a fenced block:

`<handle> L<a>-<b>[ <kind> <qualified name>][ [signature]|[outline]]`

- `L<a>-<b>` names the first and last 1-based lines the handle's range touches (the
  line of `start` and the line of `end - 1`); the empty range `[0,0)` of an empty
  source has no lines, so its item line is the handle alone.
- `<kind> <qualified name>` labels the delivery unit (§ Unit kinds); a block, or a
  unit without a name, renders its kind alone. Retrieve items carry no label.
- `[signature]` marks the signature form; `[outline]` marks the `outline` and
  `outline-min` forms; verbatim bodies carry no tag.

Semantic evidence items (009 T002; decided 2026-10-04, revised after cross-lab
refutation) put a selection tag in a fixed slot: immediately after `L<a>-<b>` and
before the optional label.

- The three forms:
  - `<handle> L<a>-<b> [whole_unit][ <kind> <qualified name>]`;
  - `<handle> L<a>-<b> [lexical_span <matched handle>][ <kind> <qualified name>]`;
  - `<handle> L<a>-<b> [preview <matched handle>][ <kind> <qualified name>]`.
- The item handle names the returned bytes. `whole_unit` returns the matched unit
  itself, so its handle is not repeated.
- Because the tag never trails the label, a source-derived label such as a Markdown
  heading ending in `[whole_unit]` stays label text.
- Only neural candidates carry a selection tag; exact-definition, lexical and graph
  items render as above.
- Semantic bodies are verbatim bytes of the returned handle. The packer tries the whole
  unit, then the selected lexical span, then a bounded prefix labeled `preview`;
  `[signature]`/`[outline]` forms do not participate.
- After a preview's closing fence, a line `next: <handle>` names its remaining range
  while bytes of that range remain. The item, its fence and that line pack as one
  indivisible rendering, and final backtracking removes them together.

Parsers dispatch by operation:
- a context `next:` line belongs to the immediately preceding preview;
- retrieve keeps its single final `next: <handle>`;
- references ends with `next: after=<path>#<start>-<end>` (005).

Framed source items are parsed before continuation prefixes. A literal `next:` inside a
fenced body stays body content.

The fence is backticks of length max(3, 1 + the longest backtick run that begins a
body line after at most three spaces). Its info string is the language tag of the
source path (§ Dependencies and languages), omitted when unknown. The body is the exact
source bytes of the handle's range (verbatim) or its § Outlines rendering. When the
body does not end with LF, one framing LF precedes the closing fence; it is framing,
not source. The handle's range defines the verbatim bytes, so parsers take a verbatim
body's length from the handle (an empty range is an empty body).

Only verbatim bodies are byte-exact on the wire. A `[signature]` or `[outline]` body has
no handle-derived length, so when its rendering does not end with LF a parser cannot
tell the framing LF from content; `testkit::parse_v2` returns it as part of the body.

**Reviewed item-line ambiguity limitation (T004).** A valid path can embed a complete
`@<sha32>.<ws16>` suffix followed by a valid item tail, giving one item line two
complete readings. `testkit::parse_v2` refuses such a line and attributes it to
neither handle; production rendering and direct `retrieve` are unaffected. No
universal item-line round trip is claimed. The required 4096-byte special-path
round trips through CLI, MCP and the core passed in final4.

Graph items are one line, `edge [<alias> ]<text>`, where `<alias>` appears only for a
multi-root owner and `<text>` is the existing edge rendering
`<from path>:<line> (<symbol>) --<kind>--> <to path>:<line> (<symbol>) [<evidence>; provider=<provider>@<revision>]`.
Graph items follow the first source item (§ Context candidates). Retrieve's text view
ends with `next: <handle>` while bytes of the requested range remain. An empty search
or context result is the header alone.

~~~text
foundry context · r6 · budget:2048 · shown:3 · omitted:1
src/lib.rs#120-388@0123456789abcdef0123456789abcdef.fedcba9876543210 L7-18 fn parse_record [signature]
```rust
pub fn parse_record(input: &str) -> Option<(&str, &str)> {
    ⋯ 8-17
}
```
edge src/main.rs:2 (main) --calls--> src/lib.rs:7 (parse_record) [manual; provider=fixture@1]
src/main.rs#0-93@89abcdef0123456789abcdef01234567.fedcba9876543210 L1-4 fn main
```rust
fn main() {
    let record = parse_record("mode=local");
    println!("{record:?}");
}
```
~~~

Hex values and byte offsets above are placeholders. The first item is a unit in its
signature form; the second is a verbatim unit.

### Search locator lines

Search shows one line per hit, without fences:

`<handle> L<line>[ <kind> <qualified name>]: <excerpt>`

The handle covers the hit's delivery unit (§ Search documents), `L<line>` is its best
line (§ Hit materialization) and `<excerpt>` is that line without its LF or CRLF
terminator and leading whitespace, cut at a UTF-8 boundary to at most 160 bytes with
`…` appended when cut.

### Single-line fields

Labels, graph text, excerpts and root labels are rendered on one line: every control
character except TAB (U+0000–U+0008, U+000A–U+001F, U+007F) becomes `?`. Multi-line
source appears only inside fences, so indexed text cannot forge a header, item line or
fence, and instruction-like source stays quoted data with citations.

## Syntax units and search documents

### Dependencies and languages

001 T005 adds `tree-sitter 0.25`, `tree-sitter-rust 0.24`, `tree-sitter-python 0.25`,
`tree-sitter-typescript 0.23` (TypeScript and TSX), `tree-sitter-javascript 0.25`,
`tree-sitter-go 0.25`, `tree-sitter-c 0.24`, `tree-sitter-cpp 0.23`,
`tree-sitter-java 0.23` and `pulldown-cmark 0.13` (`default-features = false`), each
locked to a version compatible with the Rust 1.90 floor. If a grammar cannot satisfy
1.90, T005 stops and reports; dropping a language or raising the MSRV needs explicit
owner approval. New module `src/syntax.rs` owns parsing, the unit forest, outline
rendering and the extension map.

The language is chosen by file extension only:

| Extensions | Language tag | Units |
| --- | --- | --- |
| `rs` | `rust` | yes |
| `py`, `pyi` | `python` | yes |
| `ts`, `mts`, `cts` | `typescript` | yes |
| `tsx` | `tsx` | yes |
| `js`, `mjs`, `cjs`, `jsx` | `javascript` | yes |
| `go` | `go` | yes |
| `c` | `c` | yes |
| `h`, `cc`, `cpp`, `cxx`, `hpp`, `hh`, `hxx` | `cpp` | yes |
| `java` | `java` | yes |
| `md`, `markdown` | `markdown` | yes (heading sections) |
| `toml` / `json` / `yaml`, `yml` / `sh`, `bash` / `sql` / `html` / `css` | `toml` / `json` / `yaml` / `bash` / `sql` / `html` / `css` | no: fence tag only |

Every other extension is unmapped: no tag, no units, blocks only.

### Unit kinds

Rendered kinds are `fn struct enum union trait impl mod macro const static type class
method interface section block`. Unit nodes map to them by node kind:

| Language | Unit nodes (rendered kind) |
| --- | --- |
| Rust | `function_item` (fn), `struct_item` (struct), `enum_item` (enum), `union_item` (union), `trait_item` (trait), `impl_item` (impl), `mod_item` (mod), `macro_definition` (macro), `const_item` (const), `static_item` (static), `type_item` (type) |
| Python | `function_definition` (fn), `class_definition` (class); a wrapping `decorated_definition` supplies the range |
| TypeScript, TSX, JavaScript | `function_declaration`, `generator_function_declaration` (fn); `class_declaration` (class); `method_definition` (method); `interface_declaration` (interface); `type_alias_declaration` (type); `enum_declaration` (enum); `lexical_declaration`/`variable_declaration` with exactly one declarator whose value is `arrow_function`/`function_expression` (fn); a wrapping `export_statement` supplies the range |
| Go | `function_declaration` (fn), `method_declaration` (method), `type_declaration` (type; it has no `name` field, so it renders as an unnamed `type` under the Name rule below) |
| C, C++ | `function_definition` (fn); `struct_specifier` (struct), `class_specifier` (class), `union_specifier` (union) and `enum_specifier` (enum) that have a body; `namespace_definition` (mod); a wrapping `template_declaration` supplies the range |
| Java | `class_declaration` (class), `interface_declaration` (interface), `enum_declaration` (enum), `record_declaration` (class), `method_declaration` and `constructor_declaration` (method) |
| Markdown | heading section (section): from a heading to the next heading of equal or lower level number (equal or higher rank), or EOF; subsections are children; headings inside code fences do not count |

Name: the `name` field; for C/C++ the innermost identifier of the declarator chain,
including through parenthesized declarators; for a Rust `impl` the `type` field's text;
for Markdown the heading text with surrounding whitespace trimmed, cut at a UTF-8
boundary to at most 120 bytes. A unit with no such field is unnamed (accepted
limitation: Go type declarations; changing it needs a contract amendment and a
ranking test). The qualified name joins ancestor unit names and the name with `::`
(rust, cpp) or `.` (all others). When it exceeds 256 bytes it keeps its last 256
bytes, starting at the first UTF-8 boundary at or after that point, so the unit's own
name and its nearest ancestors survive; it carries no cut marker. It is built from the
parent's kept qualified name, which is exact for a tail and keeps deep nesting linear
(owner decision, 2026-10-04).
Body: the `body` field, or else the block/`declaration_list` child. Signature lines run
from the unit's declaration start — the first attribute line of its leading run (§ Unit
forest) when the run has a Rust attribute, otherwise the node's (or wrapper's) start
line — through the line of the body's opening delimiter (`{`, `(` or `[`); for a body
without one (a Python block, a Markdown section) through the line of the last
non-whitespace byte before the body, such as a Python block's `:` or a heading. A unit
without a body has no elidable interior.

### Unit forest

Units form a source-ordered interval forest. A unit strictly nested in another is a
child of its nearest enclosing unit; a wrapper coextensive with its unit yields one
unit; zero-width, invalid or partially overlapping ranges are not units (of two
partially overlapping ranges, the later is dropped). Parse errors are tolerated: the
error-recovered tree still yields units. Zero units, an unmapped language or a source
over 1 MiB falls back to blocks. Parsing is deterministic, with no time-based limit.
For tree-sitter languages, a unit's range is its node's (or wrapper's) byte range,
extended backward over its leading run (below); no following line terminator is
appended to that range. Markdown section ranges run to the next equal-or-higher-rank
heading or EOF and can include a trailing LF. A handle uses the range unchanged.

**Leading run** (owner decision 2026-10-04, amending T005; implemented locally):
the maximal sequence of sibling nodes immediately preceding a unit's node (or wrapper)
in which every node is a documentation comment or attribute of that language, starts
its line (only whitespace before it on that line), and is separated from the next
node of the sequence, and the last from the unit, only by whitespace containing at
most one line terminator (no blank line):

| Language | Leading-run nodes |
| --- | --- |
| Rust | `line_comment` or `block_comment` with an `outer` doc marker (`///`, `/** */`); `attribute_item` (`#[…]`). Inner docs (`//!`) and `inner_attribute_item` (`#![…]`) never attach |
| Java | `block_comment` beginning `/**` (Javadoc); annotations are already inside the declaration |
| TypeScript, TSX, JavaScript | `comment` beginning `/**` (JSDoc) |
| Go | any `comment` (Go doc comments are ordinary comments directly above a declaration) |

Python, C, C++ and Markdown have no leading run. The run joins the unit's range, so
it belongs to the unit's search documents (a container's own run lies in its residual
region) and is retrieved with the unit; names and qualified names are unchanged.

### Search documents

- Each leaf unit is one region.
- Each container's bytes minus its direct children's ranges form residual regions,
  one per contiguous piece.
- Bytes outside all top-level units form blocks: split at blank lines, then merged in
  order up to 2048 bytes.
- Whitespace-only regions produce no document.
- A region over 8192 bytes splits at line boundaries into parts of at most 4096 bytes;
  a single longer line splits at a UTF-8 boundary.

Every document records its **delivery unit**: a leaf is its own; a residual's is its
container; a block is its own; parts keep their region's delivery unit. A document's
`kind`, `name` and `qname` describe its delivery unit. Invariants (tested): document
ranges plus whitespace-only gaps tile `[0, len)` without overlap, and each document's
range lies inside its delivery unit.

### Search index v2

Tantivy schema v2:

| Field | Options | Content |
| --- | --- | --- |
| `key` | STRING, STORED | `path\0start` |
| `key_hash` | u64 FAST, STORED | first 8 bytes of SHA-256 of `key`, big-endian |
| `path` | STRING, STORED | the source path |
| `dir` | STRING | every ancestor directory of the path, and the path itself |
| `hash` | STRING, STORED | full source SHA-256 |
| `start`, `end`, `unit_start`, `unit_end` | u64, STORED | document and delivery-unit byte ranges |
| `kind`, `lang` | STRING, STORED | delivery-unit kind and language tag |
| `name`, `qname` | STORED | delivery-unit name and qualified name |
| `def_name` | TEXT, tokenizer `foundry_lower` | definition documents only |
| `ident` | TEXT, tokenizer `foundry_ident` | identifier runs |
| `body` | TEXT with positions, tokenizer `foundry_code` | code subtokens |

- `foundry_lower`: the whole name as one lowercased token.
- `foundry_ident`: maximal `[A-Za-z_$][A-Za-z0-9_$]*` runs, lowercased, length ≥ 2.
- `foundry_code`: split on non-alphanumerics (including `_`), at lower→Upper and
  Upper→Upper+lower boundaries (`HTTPServer` → `http`, `server`) and at letter↔digit
  boundaries; lowercased; length ≥ 2.

Definition documents are those whose delivery unit is a programming-language unit
(not a Markdown section or block): a leaf's document, or each residual of a
container, carries that unit's `def_name`. Source bytes are not stored in Tantivy;
hits reconstruct verified source from `CHUNKS`.

### Two-tier query

The query's identifier runs are its maximal `[A-Za-z_$][A-Za-z0-9_$]*` runs. A run is
marked when it lies inside a backtick code span. Scanning left to right, a maximal run
of N backticks opens a span that the next maximal run of exactly N backticks closes
(backtick runs of other lengths between them are span text); an opener with no such
closer is literal text, and scanning resumes after it.

**Tier 1, exact definitions** (amended 2026-10-06): the tier-1 runs are the marked runs
when the query has at least one, otherwise all runs, lowercased and deduplicated; only
the first 32 distinct runs by first appearance are used. Each run's count is the exact
number of its definitions (documents whose `def_name` equals the run) under tier 1's
own restriction (memory documents excluded, the `path` filter's `dir` term). Runs are
ordered by count ascending, then run text ascending. Walking that order, each run
contributes its definitions, smallest `key_hash` first
(`TopDocs::with_limit(64).tweak_score(|reader| move |doc, _| Reverse(key_hash))`), up to
the slots remaining of 64; a document matching several runs counts once, under its
earliest run. Tier 1 is ordered by run, then path, start. Cause: every English word of
a query (`find`, `of`, `callers`) matched its own definitions, and the old 64-document
cut ordered by path buried or evicted the intended identifier.

**Tier 2, lexical:** the union of, per whitespace-separated query part, a `body` phrase
(when the part has at least 2 subtokens) or term; each identifier run, marked or not,
as an `ident` term (boost 3); and the trimmed query as an exact `path` term (boost
100). Select with
`TopDocs::with_limit(256).tweak_score(|reader| move |doc, score| (score, Reverse(key_hash)))`
so cutoff ties do not depend on segment order. Skip documents whose delivery unit is
already in tier 1.

Final order: tier 1, then tier 2 by score descending, path ascending, start ascending.
The `path` input adds a must-term on `dir` to both tiers. `candidates:full` reports that
a window filled: tier 1's 64 slots with definitions left over, or tier 2 at 256. Scores
may legitimately change when index statistics change (refresh, repair); determinism
holds per index state.

### Hit materialization

Merge candidates sharing a delivery unit, keeping the best-ranked. At most 4 hits per
file survive (per root and path in a multi-root owner); skips are counted in
`capped:<n>`. For a tier-1 hit the best line is the delivery unit's head line: the
node's (or wrapper's) own start, after its leading run (§ Unit forest), so never a
leading-run documentation or attribute line; otherwise the line with the most
distinct query subtokens (`foundry_code` analysis), the earliest on ties. Context
draws its units from this same materialized ranking.

### Index version gate

META key `search_schema = "3"` (it was `"2"` before the 2026-10-04 leading-run
amendment changed search-document ranges) is written in the store-initialization transaction for
new stores and, for rebuilds, in the same authoritative transaction that clears
`search_rebuild_required` after a successful commit/reload with an empty pending table.
Open also checks the actual Tantivy field set. A missing or other value, or a field
mismatch, makes status report `repair_required` with reason `search_schema` and makes
search/context fail with `repair_required` ("search index format changed; run
`foundry repair-index`"). Ordinary opens never repair or write. The search format is
independent of the store schema. That schema has been 3 since 008, whose upgrade adds
the memory table and types the pending keys; retrieve, status and exports keep working
through the authoritative-only open.

## Outlines and forms

### Outline algorithm

`src/syntax.rs::outline(source, lang, range, unfold_until, unfold_limit) -> Vec<Segment>`
returns `Segment::{Kept{start,end}, Elided{first_line,last_line,indent}}` over source
bytes. It is an independent implementation; the design reference is recorded in 001 T006.

- Mandatory kept lines: every unit's signature lines (so an ancestor's multi-line
  signature stays visible); a body's closing line when its closing delimiter (`}`,
  `)` or `]`) has only whitespace before it on its line; and every line of a
  declaration-only member. Declaration-only members stay non-units, so ranking is
  unchanged: Rust `function_signature_item` and `associated_type`; Go `method_elem`;
  TypeScript/TSX `method_signature`, `property_signature`, `abstract_method_signature`,
  `call_signature`, `construct_signature` and `index_signature`; C/C++ `declaration`
  and `field_declaration` with a function declarator. A member takes the range of its
  directly enclosing wrapper (a C++ `template` keeps its parameter lines).
- Elidable spans are maximal runs of non-mandatory lines, so every mandatory line
  splits them: body interiors of leaf units and container gaps (container interior
  lines outside member units), each at least 4 lines; Markdown section-body runs of
  any length, excluding subsection headings. A body interior runs from the line after
  the signature to the line before the closing line (without a closing line, to the
  body's last line). A unit's leading-run documentation lines before its declaration
  start form one elidable span when they are at least 2 lines; that span replaces any
  block-comment span inside it. Other block comments of at least 6 lines are elidable
  when they occupy whole lines and hide no mandatory line; such a comment nests inside
  the span that contains it.
- A span is considered for a `range` when its first line starts at or after the range
  start and its last line's content (before its LF or CRLF) ends at or before the range
  end; a Python unit range, which ends at its last statement, therefore still contains
  its interior. Elided bytes stop at the range end.
- Start with every outermost considered span folded. Breadth-first (outer before
  inner, then source order), unfold a span — revealing its lines except nested
  considered spans, which become folded — while visible lines are fewer than
  `unfold_until`; skip, without exploring its subtree, any unfold that would push
  visible lines above `unfold_limit`. Visible lines count each marker as one line.
- An elided span renders as one line: the indentation of its first line, then
  `⋯ <first>-<last>` (1-based inclusive line numbers).

### Forms

| Form | Range | `unfold_until` / `unfold_limit` |
| --- | --- | --- |
| `signature` | the unit's range | 0 / 0: every outermost span folded |
| `outline` | the file's range | 60 / 120 |
| `outline-min` | the file's range | 0 / 0 |

Forms are whole-or-nothing: never paginated.

## Candidates and packing

### Counting boundary

MCP: the exact `o200k_base` `encode_ordinary` count of the final text-block content
the server emits; the serialized `CallToolResult` is independently capped at 256 KiB.
This counts Foundry's delivered payload, not the host's or provider's cost: OMP, pi and
opencode forward the block unchanged only while it stays under their own limits, and
may spill, truncate or later compact it (pi cuts MCP text over 20 KiB, opencode
truncates at 2000 lines or 50 KiB, OMP applies its spill and byte cap; pinned citations
in the [economics contract](../../003-agent-retrieval-context/contracts/adapter-economics.md#evidence-behind-the-rules)). CLI: the complete final stdout,
including its trailing newline, constructed before counting and capped at 256 KiB.
One renderer per boundary; nothing is appended after counting. Clear
`CallToolResult.result_type` before serialization so the SDK's legacy-peer rewriting
cannot alter the capped value. There is no characters-per-token fallback. JSON-RPC
framing, the tool catalog and host/provider wrapping are outside this boundary; 003
measures the catalog separately.

### Budgets

Search, context and retrieve are budgeted. On the MCP owner the effective budget is
min(request, configured `max_context_tokens`, remaining session allowance), with the
limiter labels of the [adapter economics contract](../../003-agent-retrieval-context/contracts/adapter-economics.md),
which owns the atomic reservation: one reservation per response, refunded on refusal
paths and charged with the counted tokens on success. The CLI uses the request.
Hits and items that do not fit are omitted and counted.

### Candidate seam

001 T005 introduces `Engine::search_candidates(&self, query, path: Option<&str>, limit,
control) -> FResult<CandidateBatch>`; 001 T006 adds `Engine::context_candidates(&self,
query, strategy, control) -> FResult<CandidateBatch>` and settles the shared types:

- `CandidateBatch { freshness, items: Vec<RankedItem>, counters: CandidateCounters }`;
- `RankedItem { tier, rank, score, handle: Option<SourceHandle>, start_line, end_line,
  line, label, lang, forms: Vec<RenderedForm> }`. `handle` keeps the full 64-hex
  identities (rendering shortens them) and is `None` for a graph item;
  `start_line..=end_line` are the lines the handle's range touches and `line` the
  locator's best line. Tiers: 1 exact definition, 2 lexical, 3 graph, 4 file outline.
- `RenderedForm` is `Verbatim | Signature | Outline | OutlineMin | Line`: verbatim and,
  only when its rendering differs from the verbatim bytes, signature for a unit;
  outline and outline-min for a file outline; the single line for a graph item;
  verbatim alone for a block or a unit of an unmapped language.
- `CandidateCounters { stale, capped, candidates_full, truncated, graph }`, where
  `graph` is `ok`, `graph_unavailable`, `graph_stale` or `graph_invalid` when the
  strategy resolved to graph.

The `tokens` range (1..32768) is validated at the boundaries — CLI before opening the
store, MCP in its argument parser — so `context_candidates` takes no budget.

Items are already revalidated in that store's final read transaction. Packing is
`response::pack(items, header, budget, byte_cap, boundary)` over an already ordered
list. Single-root order is batch order; 007 merges batches, never packed responses.

### Context candidates and routing

Context candidates, in order:

1. up to 32 delivery units from the two-tier ranking (§ Hit materialization);
2. when the strategy resolves to graph, bounded graph items placed after the first
   unit, seeded by the paths of the top 3 units as in v1;
3. up to 3 file outlines for the first distinct files among those units, skipping an
   empty file and a file that one of those units spans (only whitespace lies outside
   the unit). Only mapped languages have outlines, read literally: a mapped
   language without units (`toml`, `json`, `yaml`, `bash`, `sql`, `html`, `css`) has an
   outline equal to its text; an unmapped extension has none.

v1's `following_chunks` candidates are removed.

For `auto`, ASCII-lowercase the query and tokenize maximal runs of ASCII letters,
digits or `_`. Any whole token in `{calls,caller,callers,depends,impact,dependency,
dependencies,reference,references,referenced,usage,usages,uses,used,break,breaks}`
selects graph, otherwise search (`referenced`, `uses`, `used`, `break` and `breaks`
added 2026-10-06). This replaces the prototype's substring rule: `preferences` and
`calls_tracker` are not graph keywords, while a question about references can use
005's supported relation.
The owned policy can replace that choice only under 013's identity/threshold contract.
Explicit search or graph never invokes the policy. Graph without eligible edges still
returns source results and a graph coverage reason in the header. No claim that this
heuristic is optimal.

Before optional policy inference, check graph coverage metadata against the indexed
source revision. If no supported graph scope is current, use search without a model
call and report graph stale/unavailable; do not query a router to choose an unavailable
branch. Current partial graph coverage permits bounded expansion, with its limitation
retained. Explicit graph still reports the same limitation and never invents relations.
This rule does not add graph storage to 001; before 005, compiler graph is unavailable.
Candidate construction is the two-tier lexical ranking plus available 009 semantic
candidates, followed by 009's bounded merge. The optional 013 ModernBERT decision head
receives its own bounded state/question/option input after that merge; it does not
consume Nemotron's query vector. Then perform bounded graph expansion and
packing/final freshness validation. Unavailable semantics alone does not make policy
inference unavailable. Missing valid 013 configuration/input, explicit strategy or
unavailable graph skips that inference. Until 013 replaces its superseded model
contract, only deterministic routing is specified for implementation here. 001 and 009
do not wait for 013 to ship. All model work shares the read deadline; none occurs in
the final read transaction.

### Ladder packing

For each candidate in order, include the first form whose complete rendering — the
whole response, with the header updated for that inclusion — fits both the token
budget and the byte cap; otherwise omit the candidate and count it. After the pass,
render the final header; if the final rendering no longer fits, remove the last-added
items, counting them as omitted, until it fits. A fitting first unit precedes graph
items: the graph-first starvation rule. Search packs its locator lines the same way.

Source bytes are never rewritten or summarized. The only non-verbatim forms are the
deterministic outlines above, which keep every shown line verbatim and mark each elided
range explicitly. If even the header cannot fit, return `budget_too_small` with the
existing sufficient-budget hint; no success is over budget, and the hint is not
advertised as a mathematical token minimum.

### Retrieve views

`view:"text"` keeps v1's bounded prefix fit. Start with at most 128 KiB of the
requested range (after any `lines` clipping), ending at a UTF-8 boundary. If the
rendered result with its `next:` line does not fit, halve that byte length, retreat to
a UTF-8 boundary and retry, always trying the first complete codepoint before failure.
This is at most 19 nonempty trials and does not assume BPE monotonicity or promise the
longest possible prefix. Return the first fitting tested prefix and start `next`
exactly at its end. Requested ranges remain bounded to one 2 MiB source; no
byte-by-byte quadratic tokenization loop. If no nonempty tested prefix fits, return
`budget_too_small` with a sufficient budget for the one-codepoint result; never an
unchanged continuation loop. `[0,0)` on an empty file returns an empty item if its
header fits.

`view:"outline"` renders the requested (clipped) range in the `outline` form, else
`outline-min`. It is never paginated and never returns `next`. If neither fits, return
`budget_too_small` with a budget sufficient for `outline-min`. When `outline-min`
cannot fit 32768 tokens or the 256 KiB result cap under some limiter label, so that no
budget could deliver it, return `unsupported_mode` advising `lines` or `view:"text"`
instead of a hint that cannot succeed. A zero-allowance `budget_exhausted` refusal made
before engine admission advertises 32768, the largest budget: outline-min is
whole-or-nothing and content-dependent, so any deliverable outline fits it (owner
decisions, 2026-10-04). An unmapped language is `unsupported_mode`; a mapped language
without units, or a mapped source over 1 MiB (not parsed), has an outline equal to its
text.

### Deduplication, no cross-call suppression

Within one response, deduplicate source by full identity
`(workspace_id, path, sha256, start, end)` and merge hits inside one delivery unit;
deduplicate graph by `(workspace_id, stable fact identity)`: `edge_id` for resolved
edges, `occurrence_id` for 005 occurrence evidence. Nothing is suppressed across
calls: host compaction in OMP, pi and opencode drops or rewrites earlier tool results,
so a later response cannot assume earlier bytes are still visible. Compression is
deterministic elision with explicit markers; there is no summarizer or compression
model. `omitted:<n>` counts constructed candidates dropped by packing, not all
unknown matches; an empty or limited result never proves absence. 009 may construct a
smaller source candidate under its explicit excerpt rules before packing; it cannot
silently truncate evidence or call a preview localized.

## Failure scope

Open existing authoritative state independently of optional indexes/providers, as
specified by 001. A read never repairs, downloads, trains, enrolls a path or records
feedback implicitly. Operational counters may remain in memory; they do not join
source commit eligibility or create a durability plane.

| Failure | Behavior |
| --- | --- |
| Missing store, unsupported store schema, unreadable authoritative database | Named error; no invented empty state or automatic upgrade |
| Lexical index missing/corrupt/rebuilding or of another search schema | Search/context return `repair_required`; status and direct authoritative reads remain available |
| Graph absent/stale or a graph record cannot be decoded | Context can return valid source with `graph:graph_unavailable`, `graph:graph_stale` or `graph:graph_invalid`; a direct graph request names that failure |
| Neural runtime/profile/cache unavailable or invalid | Baseline retrieval with the named semantic reason; no implicit preparation |
| Policy configuration invalid or worker unavailable | Learned routing disabled with the named reason; deterministic retrieval still works |
| A requested memory record cannot be decoded | `corrupt_memory`; source operations remain available if the database itself is healthy |
| A reference root cannot serve (007) | That root reports its coverage in the header; other roots serve; none serving is `roots_unavailable` |

Component-local decode failures do not certify database health. A database-reported
corruption/error is not downgraded to one of the optional fallbacks. Never label a
failed component an empty successful result. Include relevant degradation metadata
before output accounting, and preserve bytes for explicit recovery/export where readable.

## Errors and partial index reports

MCP errors are a bounded `isError:true` tool result with one text block containing
`{code,message,retryable}`; total serialized value at most 1024 bytes. Such errors are
outside successful budgets. Budget refusals name the limiting bound: `budget_exhausted`
when the session allowance cannot be reserved (a refusal changes no counter) and
`budget_too_small` when even the header cannot fit; both carry a sufficient-budget hint
valid under any limiter label (for `view:"outline"`, § Retrieve views). An admitted `index` interrupted by `deadline_exceeded`,
`cancelled` or `index_incomplete` additionally includes
`partial:{changed,unchanged,deleted,excluded,failed,pending_sources,scan_complete,deletions_deferred}`.
Counters are checked u64s; the final two fields are booleans. Changed/deleted count
committed source transactions only. This counts-only error has a fixed ASCII message
of at most 256 bytes, no source/path/samples, and still fits the same 1024-byte cap.
Deadline/cancel are retryable; incomplete scan is not automatically retryable.
Only deliverable replies expose this report: after explicit client cancellation,
session deletion or owner shutdown, status names durable scan/pending/revision state.
CLI keeps its full bounded partial report on stdout with nonzero exit. MCP-only
failure samples may be written once to bounded stderr, never placed in tool errors.
Protocol errors use SDK semantics. Untrusted paths and text remain data; no source
text becomes an instruction or tool request.

## Contract checks

At budgets 1, 32, 64, 256, 1024 and 32768, assert the exact o200k count of the MCP text
block and of CLI stdout (including its trailing newline), the 256 KiB caps, bounded
named errors otherwise and forward-progress continuations. Every fenced body equals
its handle's bytes, including sources without a final LF, CRLF, empty content,
embedded fences and JSON-escape-heavy text; cover multi-byte identifiers and tiny
budgets. A maximum-length path containing `#`, `@`, `.` and JSON-special characters
round-trips through search output and CLI/MCP retrieve; v1 objects and uppercase hex
are `invalid_argument`; synthetic full IDs sharing no 16-hex prefix are
`wrong_workspace`; edited and deleted sources give `stale_handle` and `not_found`.
`lines` covers mid-line unit boundaries, continuation handles, CRLF, an unterminated
last line, a trailing LF and an empty file (`invalid_range`). Headers contain none of
the removed fields and stay within 40 tokens on the test fixture; no delivery ID
appears anywhere; `host_request` stays refused on both transports. Search charges the
session allowance, and barrier-released same-session HTTP calls leave the exact
expected balance after one refused and one successful delivery. Instruction-like
source stays inside a fence. A fitting highest-ranked source precedes graph items.
Inject an indexed edit/forget/graph replacement between candidate collection and final
validation: each response contains only data eligible in its reported read snapshot.
Break one optional component at a time and assert the failure-scope table. Known
unavailable graph or invalid routing configuration must not invoke the policy; a read
must not create a store or start repair. Syntax, ranking, outline and multi-root
checks are listed in 001 T005/T006 and 007 T001. These extend the owning tasks'
focused cases; they are not a portfolio-wide validation stage, and no provider-cost or
complete-recall claim follows.
