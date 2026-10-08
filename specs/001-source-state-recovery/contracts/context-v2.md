# Shared context contract v2

Status: approved 2026-10-03 (token-economics spec pass). **001 T004–T006 are locally
implemented, accepted at the reviewer's SHIP and committed in `5edf32c`; unreleased.**
007 T001 (multi-root) and the T005 leading-run amendment are implemented and accepted
locally on 2026-10-04 and committed (`cc402e0`, `bd1d890`). The 2026-10-04 amendments below record owner
decisions and implemented details from T005/T006 review. The 2026-10-06 amendment
(owner-approved after the 013 corpus analysis) changes tier-1 run selection and order,
accepts an MCP `lines` array and makes the empty `lines` refusal name the handle's
lines; five proposed route keywords were measured and withdrawn (§ Context candidates
and routing). A second owner-approved 2026-10-06 amendment adds compact context
(§ Compact context): a context whose marked identifiers each have exactly one
definition returns them plus at most 8 one-line pointers instead of filling its budget.
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
| Tier-1 marked runs and specificity order, MCP `lines` array, empty-selection message (2026-10-06 amendment) | 001 (amendment) | Accepted locally 2026-10-06 (cross-lab SHIP; gates green), committed in `c3437e6`; route keywords withdrawn after measurement; unreleased |
| Compact context: trigger, content, `compact` header segment, multi-root count sum (2026-10-06 amendment) | 001 (amendment) | Accepted locally 2026-10-06 (cross-lab SHIP; gates green), committed in `844796c`; replaced by § Anchored context when 001 T007 is accepted; unreleased |
| City map: roles, definitions and addresses, anchors, resolver, `[address]`, anchored context, doors, languages, parallel indexing (2026-10-07) | 001 T007–T009, 005 T004 | Approved 2026-10-07, revised after cross-lab refutation; proposed |
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

Header segment amendment, 2026-10-06 (compact context): a context packed under
§ Compact context adds the bare segment `compact` last, after `route:` when present.
Every other response is byte-identical to its rendering before this amendment.
Budget-refusal hints do not reserve room for it, as for `route:`.

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
- references ends with `next: after=<path>#<start>-<end>` (005);
- with the city map, a context also carries `[address]` item lines and directory lines
  (navigation, no fence), door lines `<handle> L<n> in <label>: <excerpt>` with optional
  ` (+<n>)` and ` [approx]`, and a closing `⋯ <m> more files` line; none has a `next:`.

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
or context result is the header alone. A compact context (§ Compact context, amended
2026-10-06) follows its definitions with one-line pointers: search locator lines
(§ Search locator lines) and `edge` lines; `testkit::parse_v2` accepts a locator line in
a context only when its header carries `compact`.

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
`…` appended when cut. A compact context's pointer to a source item is the same line,
from the same renderer (2026-10-06): the item's delivery-unit handle, best line, label
(`semantic` for a dense-only unit, as in search) and excerpt.

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
container, carries that unit's `def_name` (until 001 T007: then only the document
holding the unit's name node, § City map). Source bytes are not stored in Tantivy;
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
own restriction (memory documents excluded, the `path` filter's `dir` term); each
marked run's count, zero included, also decides § Compact context. Runs are
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

When 001 T007 is accepted, the value becomes `"4"` (§ City map), and every later change
to search-document content bumps it again.

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
- `CandidateBatch::marked` (2026-10-06): one `MarkedRun { run, definitions }` per marked
  tier-1 run of the query, in order of first appearance, carrying tier 1's exact count
  under its own restriction (§ Two-tier query); empty for an unmarked query. Search
  ignores it. A 007 merge sums each run's count over the merged roots, and
  `CandidateBatch::compact()` is the § Compact context trigger the packer applies.

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
   (When 005 T004 is accepted, this item and the keyword routing below are replaced by
   § Doors: graph and usage-intent requests build doors for a resolved anchor.)
3. up to 3 file outlines for the first distinct files among those units, skipping an
   empty file and a file that one of those units spans (only whitespace lies outside
   the unit). Only mapped languages have outlines, read literally: a mapped
   language without units (`toml`, `json`, `yaml`, `bash`, `sql`, `html`, `css`) has an
   outline equal to its text; an unmapped extension has none.

v1's `following_chunks` candidates are removed.

Compact selection (§ Compact context, 2026-10-06) happens at packing, over this already
ordered list; this order, routing, graph expansion and 013's state composition do not
change.

For `auto`, ASCII-lowercase the query and tokenize maximal runs of ASCII letters,
digits or `_`. Any whole token in `{calls,caller,callers,depends,impact,dependency,
dependencies,reference,references,usage,usages}` selects graph, otherwise search. This
replaces the prototype's substring rule: `preferences` and `calls_tracker` are not
graph keywords, while a question about references can use 005's supported relation.
Adding `uses`, `used`, `break`, `breaks` and `referenced` was tried and withdrawn on
2026-10-06: after the tier-1 amendment, routing those questions to graph lowered the
delivered required evidence on the rust-lang/rust checker tasks (dev 40.6% to 38.0%,
held-out wordings 34.5% to 34.2%), because graph context places path-seeded neighbors,
not the queried symbol's references ([validation](../../../docs/validation.md)).
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
items: the graph-first starvation rule. Search packs its locator lines the same way, and
a compact context (§ Compact context) its compact selection.

Source bytes are never rewritten or summarized. The only non-verbatim forms are the
deterministic outlines above, which keep every shown line verbatim and mark each elided
range explicitly. If even the header cannot fit, return `budget_too_small` with the
existing sufficient-budget hint; no success is over budget, and the hint is not
advertised as a mathematical token minimum.

### Compact context

When 001 T007 is accepted, § Anchored context replaces this section, and the header
word `compact` becomes `anchored`.

Amended 2026-10-06 (owner-approved). Without this rule every `context` fills its budget:
on the rust-lang/rust checker tasks responses averaged about 2,015 of 2,048 o200k tokens
even when the answer was one small definition. An agent host re-sends every delivered
token on each later turn, while a missed answer costs about one extra turn (about
12.4–13k tokens); a measured run at a 512-token budget kept 56–58% of the definition
successes at about 480 tokens. The owner chose a deterministic rule first.

**Trigger.** A `context` (CLI and MCP, every strategy, single- or multi-root) is compact
when its query has at least one marked tier-1 run (§ Two-tier query), at least one
marked run has exactly one definition, and no marked run has more than one. The counts
are tier 1's exact counts under its own restriction: memory documents excluded and,
where the caller passes one, the `path` filter's `dir` term (`context` itself takes no
`path`). They count search documents, so a definition split into several (a container's
residual pieces, a region over 8192 bytes) is never unique. A marked run without
definitions neither triggers nor blocks the rule; an unmarked query is never compact.
In a multi-root owner ([007](../../007-multi-workspace-context/spec.md)) a run's count
is the sum over the roots the response merges, so a name defined once in each of two
roots is not unique. `search` and `retrieve` are unchanged.

**Content.** The ladder (§ Ladder packing) runs over this selection of the already
ordered batch:

1. the unique definitions — the batch's tier-1 items, which under the trigger are at
   most one per marked run — in tier-1 order (the merged tier-1 order in a multi-root
   owner), each in its first form that fits, as for any candidate (verbatim, else
   signature; a neural item keeps the 009 ladder);
2. then at most 8 further candidates in context order (§ Context candidates and
   routing: graph items and compiler units, then the remaining lexical or semantic
   units), each as one line: a source item as its search locator line (§ Search locator
   lines), a graph item as its `edge` line. File outlines are skipped.

Every other candidate — the file outlines and everything past the eighth pointer — is
omitted and counted in `omitted:<n>`. The budget stays an upper bound: a candidate that
does not fit is omitted, a budget below the header is `budget_too_small`, and compact
mode never adds candidates to fill the budget. A definition the final read drops is
counted in `stale:<n>` as before, and the response stays compact. Opt-in 008 memory
lines follow the pointers under their existing rule.

**Header.** A compact response's header ends with the bare segment `compact` (§ Header
line); every other response is byte-identical to its rendering before this amendment.

**Order of operations.** Selection happens at packing, over the already ordered batch:
ranking, routing, graph expansion and 013's state composition are unchanged. The batch
carries each marked run's count (`CandidateBatch::marked`, § Candidate seam); 007
merges batches, summing those counts, and never packed responses.

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

## City map

Approved by the owner 2026-10-07 and revised after cross-lab refutation; proposed
until 001 T007–T009 and 005 T004 are accepted. Evidence is in [validation](../../../docs/validation.md)
(§ City-map evidence, § Embedding model comparison, § Learned-router economics):
one call delivered the required evidence for about three quarters of names with a
single definition and for 2–10% of names with more than 64; given the intended
definition, `references` covered every required call site of the failed usage tasks;
path-seeded graph expansion expanded the wrong symbols; in a Bun monorepo a
19,500-byte `interface ToolSession` was the first tier-1 candidate yet vanished at
2,048 tokens because no form of it fit, so case-folded test helpers named `toolSession`
were what the response delivered. Neither the learned router nor dense fusion improved
these identifier tasks (dense fusion gained only on `graph` for dev, 51 / 21, while
losing on `search`).

A codebase is a city: every definition has an address; a query names a place; the
answer is that building, a short directory of buildings that share the name, or the
building's doors. Everything here is built during ordinary indexing: no model, no
compiler run, no configuration file is read for it, and nothing is executed. When
accepted, § Anchored context replaces § Compact context (its header word `compact`
becomes `anchored`), § Doors replaces the path-seeded graph expansion and the graph
keyword routing of § Context candidates and routing, and search schema `"4"` replaces
`"3"` in § Index version gate. Every later change to what a search document contains
bumps the schema again.

### Roles

Every source path has one role, decided by the first matching rule on its
workspace-relative path (components compared case-sensitively; basename patterns as
written):

1. `lock` (4): basename `bun.lock`, `bun.lockb`, `package-lock.json`,
   `npm-shrinkwrap.json`, `yarn.lock`, `pnpm-lock.yaml`, `Cargo.lock`, `composer.lock`,
   `Gemfile.lock`, `poetry.lock`, `uv.lock`, `Pipfile.lock`, `go.sum`,
   `packages.lock.json`, `Podfile.lock`, `pubspec.lock`, `mix.lock` or `flake.lock`;
2. `snapshot` (5): extension `.snap`, or a `__snapshots__` component;
3. `generated` (2): basename ending `.min.js`, `.min.css`, `.g.cs`, `.Designer.cs`,
   `.designer.cs`, `_pb2.py` or `.pb.go`, or containing `.generated.`; or a
   `generated`, `__generated__` or `obj` component;
4. `vendored` (3): a `vendor`, `third_party`, `third-party` or `Pods` component;
5. `test` (1): a `test`, `tests`, `__tests__`, `testing`, `testdata`, `fixtures`, `e2e`,
   `spec` or `benches` component; or basename `tests.rs`, `conftest.py`, or matching
   `*_test.go`, `test_*.py`, `*_test.py`, `*.test.*`, `*.spec.*`, `*_spec.rb`,
   `*Test.java`, `*Tests.java`, `*Test.kt`, `*Tests.kt`, `*Test.swift`, `*Tests.swift`,
   `*Test.cs`, `*Tests.cs`, `*Test.php`, `*_test.cc`, `*_test.cpp`, `*_unittest.cc`,
   `*.t` or `*.bats`;
6. `source` (0): everything else.

Every search document stores its role. The role orders definitions in § Resolver
order and nothing else: tier 2 and every response without an anchor are unaffected,
and no role filters anything.

### Definitions and addresses

Exactly one search document per definition carries `def_name`: the document whose
range contains the start of the unit's **name node** (the identifier the language's
name rule selects, § Unit forest). Container residuals and the parts of a region over
8192 bytes no longer carry it; they stay searchable through `ident` and `body`. A unit
that extends a type defined elsewhere (a Rust `impl` block, a Swift `extension`) is a
container, not a definition of that type: it carries no `def_name`, and its members'
addresses keep the type as a qualifier (otherwise `struct Foo` and each `impl Foo`
would tie in § Resolver order and no Rust type with an impl could resolve). Counts in
§ Two-tier query and § Compact context are therefore definitions, not documents.
Search schema `"4"` adds:

| Field | Options | Content |
| --- | --- | --- |
| `role` | u64 FAST, STORED | every document: § Roles |
| `name_case_hash` | u64 FAST | definition documents: first 8 bytes of SHA-256 of the name exactly as written |
| `addr_hash` | u64 FAST, multi-valued | definition documents: first 8 bytes of SHA-256 of each address segment |
| `name_start`, `name_end` | u64 STORED | definition documents: the name node's byte range |
| `imports` | STRING, multi-valued | each file's first document: § Doors import keys |

(`unit_head`, stored since the 2026-10-04 leading-run amendment, belongs in the
§ Search index v2 table as well.) **Address segments** are, lowercased and distinct:
each path component, the last without its extension, split on every character outside
`[A-Za-z0-9_$]` (`packages/coding-agent/src/tools/index.ts` gives `packages coding
agent src tools index`); then the unit's qname with every generic argument list
(`<…>`, `[…]`, nested) removed first, split on the language's qname separator, minus the
unit's own name (`UnionFind<Key>::find` gives `unionfind`).

### Anchors and qualifiers

Runs and code spans are those of § Two-tier query, except that inside a code span a run
may also contain `-` between letters (`Get-ChildItem`). A **chain** is runs joined only by
`::` or `->`, or, inside a code span, by `.`; its last run is its name and the others
are its qualifiers. A token containing `/` or `\`, or ending in `.<extension>` where the
extension is one § Dependencies and languages maps or `md txt rst json yaml yml toml lock`,
is a **path**, never a chain or an anchor; its segments (split as for addresses) are
qualifiers.

**Anchors** are taken in three groups, each in order of first appearance, at most four
in all:

1. the names of marked chains (a lone marked run is a chain of one);
2. unmarked runs that contain `_` or `$` (`sleep_ms`, `READY_RECEIVE_ENTERED`) or a
   lowercase letter followed by an uppercase letter (`toolSession`, `HttpServer`), and
   the names of unmarked `::` or `->` chains;
3. unmarked capitalized runs other than the query's first word that have an exact-case
   definition (`where is the Engine struct`).

Unmarked, these never anchor: all-uppercase runs without `_` (`MCP`, `WAL`), runs of
letters then digits (`v2`, `T002`, `utf8`), single letters, `e.g.`/`i.e.` and URLs.
**Qualifiers** are chain qualifiers, path segments and marked runs that are not
anchors, lowercased; plain unmarked words are never qualifiers. When a query has
anchors, its tier-1 runs are its anchors; otherwise the 2026-10-06 rule stands.

### Resolver order

Each anchor has its own window. Its definitions are scored by the tuple: distinct
qualifiers equal to an address segment (descending; compared by hash); name equal to
the anchor exactly as written (`name_case_hash`, exact first); role (ascending). The
window keeps the best 64 by that tuple, then `key_hash` ascending; every matching
definition is scored. Within the window, definitions are ordered by the tuple, then
path and start ascending: the same mechanism as today's tier 1 (a `key_hash` selection
of 64, listed by path and start), on which the G1 baselines were measured; the lists
differ where role or case separates definitions and where one document per definition
and excluded `impl` blocks change which 64 survive. Tier 1 is the windows in anchor
order. An anchor is **resolved** when
it has exactly one definition, or when its first definition's tuple is strictly
better than its second's; otherwise **ambiguous**. `defs:<n>` reports definitions
(after this amendment, one document each).

### Ladder for anchored definitions

An anchored definition takes the first fitting form of verbatim, signature (when it
differs) and `address`: its item line alone, suffixed ` [address]`, with no fence.
`RenderedForm` gains `Address`; § Evidence items gains the suffix; an `[address]` line
is navigation, as a locator line is. Every other candidate keeps its ladder. Within
the anchored selection below, an anchored definition is omitted only when the
remaining budget cannot fit its address line.

### Anchored context

A `context` (CLI and MCP; single- and multi-root) whose query has an anchor with at
least one definition is **anchored**. Its selection, from the anchor windows and the
already ordered batch, is:

1. per anchor, in order: when resolved, its first definition through the ladder above,
   then its other definitions as directory lines (their search locator lines), at most
   8; when ambiguous, the first 16 of its window in two passes: first each takes its
   signature form (verbatim when it has no shorter signature) or, when the remaining
   budget cannot fit that, its address line, so every listed candidate's name line is
   shown; then, in list order, an entry is upgraded to verbatim while the remaining
   budget fits the difference. An ambiguous name has no single answer, so no body is
   shown at the expense of another candidate's name line, and bodies still fill the
   budget as today's context does (2026-10-07, after refutation: three expanded bodies
   followed by directory lines lose today's G1 ambiguous-bucket passes, and signatures
   alone lose its bodies). The materialization cap of 4 per file and the 32-unit
   context limit do not apply to these entries;
2. then the door lines of § Doors when they apply;
3. nothing else. Everything else is omitted and counted in `omitted:<n>`; `search`
   lists it. The pointers of § Compact context are not part of an anchored context:
   the city map answers with the named definition, its namesakes or its doors (G1:
   with pointers, a resolved definition costs more tokens than grep's two calls).

The header's last segments are, in order, `defs:<n>` (when an anchor is ambiguous; the
largest count among them), `doors:<state>` (when doors were requested) and `anchored`;
the 40-token header bound holds at their largest values. A query without an anchor
keeps today's ordering and packing rules, with two changes: its tier 1 sees one
document per definition (§ Definitions and addresses), and when it requests doors it
gets `doors:none` and search packing instead of graph expansion (§ Doors). When 009
T004 is accepted and a semantic profile is enabled, its placement (dense candidates
first, then lexical) overrides these rules for anchor-less queries only. `search` orders
anchored locator lines by the resolver; `retrieve` is unchanged.

### Doors

**Requests.** Under `auto`, a query requests doors when a token, ASCII-lowercased, is one
of `callers caller calls called invokes invoked invokers uses used usage usages
references reference referenced dependents depends dependency dependencies impact
affects break breaks`; `strategy:graph` always requests them; `strategy:search` never
does. "What does `X` use" (callees) is not supported: its words request doors for
`X`'s callers, a named limitation.

**Target.** Doors are built only for the query's first anchor, only when it is
resolved: its definition `D`. When the first anchor is ambiguous, no doors are built
(`doors:ambiguous`) and the directory lines name the candidates. A query that requests
doors but has no anchor gets `doors:none` and search packing: this changes today's
anchor-less `auto` usage-word and `strategy:graph` responses, which lose the path-seeded
graph expansion (`context_graph_units` over span occurrences, removed) and with it the
`graph:<state>` header segment. G1's anchor-less usage wordings measure that change.

**Exact doors.** When a selected compiler scope (005) is current for `D`'s path, the
symbols are those of the definition occurrences whose range equals `D`'s stored
`name_start..name_end`. One symbol, or several split identities of the same source
definition, give exact doors: their references, deduplicated by site, read in the
response's one final read with 005's scope, snapshot, revision and source checks.
No such occurrence gives approximate doors with `doors:approx`.

**Approximate doors.** Each file's **import keys** are computed during indexing from
its parse (the import node kinds of § Languages) and stored in `imports`: for every
import statement, the bound names it introduces (named imports, the last segment of a
`use`/`import` path, `from m import a, b`, `using`/`open` namespaces' last segment,
`#include "x/y.h"` as `y`, `require`/`source` paths' file stem). A file imports `D` when
its keys contain `D`'s name, or the last segment of `D`'s module (its file stem, or the
directory for `index`, `mod`, `lib` and `__init__` stems). Approximate doors draw on
the delivery units, other than `D`'s own, whose `ident` contains `D`'s name, taken in
the order: importing files first, then role, path and start; the first 256 are
examined, and a candidate becomes a door only where its line contains `D`'s name
exactly as written (checked on the line in the final read). Aliased imports,
re-exports, `tsconfig.json` paths and package `exports` are not followed; Rust glob
imports (`use m::*`) give no key; a Go file's module key is its directory (its package),
not its stem; a C#, F# or VB namespace in `using`/`open`/`Imports` matches only when its
last segment equals a file stem or container name; occurrences in comments and strings
are not excluded; identifiers of one character have no doors. These are named
limitations of `[approx]` lines.

**Door lines summarize by file.** One line per file: `<handle> L<line> in <label>:
<excerpt>`, the file's first site (its line, the enclosing unit's label and at most 160
bytes of the line, § Search locator lines' excerpt rules), suffixed ` (+<n>)` when the
file has `n` more sites and ` [approx]` for approximate doors. Files are listed in the
order above (exact: path order; approximate: importing files first, then role and
path), at most 16, then `⋯ <m> more files` when more remain. Doors carry no cursor:
the complete list is `references {handle}` on `D`'s handle (shown as the anchored
definition), whose ordering and `after` pagination are unchanged. A door line is
delivered evidence of its one site's location, not of its enclosing unit's body.

**`references {handle}`** without `byte_offset` (MCP and CLI) means the symbols of
the handle's unit by the exact-doors rule; a unit that defines none is
`invalid_argument` naming `no_compiler_definition`. The catalog stays within 003's
tools/list ceiling.

### Languages

Syntax units, addresses and import keys extend to these languages through these
grammar crates (licenses from crates.io; each depends only on `tree-sitter-language`
unless noted):

| Language | Extensions and basenames | Crate | License |
| --- | --- | --- | --- |
| C# | `.cs` | `tree-sitter-c-sharp` 0.23.5 | MIT |
| F# | `.fs .fsi .fsx` | `tree-sitter-fsharp` 0.3.12 | MIT |
| VB.NET | `.vb` | `tree-sitter-vb-dotnet` 0.1.0 | MIT |
| PHP | `.php .phtml` | `tree-sitter-php` 0.25.1 | MIT |
| Perl | `.pl .pm .t .psgi` | `tree-sitter-perl` 1.1.2 (requires `tree-sitter` 0.26) | MIT |
| shell | `.sh .bash .zsh` | `tree-sitter-bash` 0.25.1 | MIT |
| PowerShell | `.ps1 .psm1 .psd1` | `tree-sitter-powershell` 0.26.4 | MIT |
| Ruby | `.rb .rake`, `Rakefile`, `Gemfile` | `tree-sitter-ruby` 0.23.1 | MIT |
| Kotlin | `.kt .kts` | `tree-sitter-kotlin-ng` 1.1.0 | MIT |
| Swift | `.swift` | `tree-sitter-swift` 0.7.4 | MIT |
| Scala | `.scala .sc` | `tree-sitter-scala` 0.26.2 | MIT |
| Lua | `.lua` | `tree-sitter-lua` 0.5.0 | MIT |
| Dart | `.dart` | `tree-sitter-dart` 0.2.0 | MIT |
| Elixir | `.ex .exs` | `tree-sitter-elixir` 0.3.5 | Apache-2.0 |
| Haskell | `.hs` | `tree-sitter-haskell` 0.24.1 | MIT |

`tree-sitter-perl` has one published release, which requires the `tree-sitter` 0.26
runtime (minimum Rust 1.77): T008 moves the runtime from 0.25 to 0.26 for every
grammar and re-verifies the original eight. A grammar that does not load or parse its
fixtures on that runtime and Rust 1.90 is dropped, and its language stays plain text,
as a named limitation; there is no keyword-line fallback. `.fs`, `.pl` and `.sc` are
taken as F#, Perl and Scala (GLSL, Prolog and SuperCollider sources are mis-parsed, a
named limitation). The basenames `Rakefile` and `Gemfile` amend § Dependencies and
languages' extension-only rule. A name ending in `?`, `!` or `'` (Ruby, Elixir,
Haskell) is stored and matched without that suffix. Rust, C and C++ gain import keys
(`use`, `#include`) with this amendment as well.

A 2026-10-07 feasibility spike (outside the repository) built all 23 grammars on
`tree-sitter` 0.26.13 with Rust 1.90; the original eight produced identical units on
7,357 of 7,358 real files (one mis-parsed C++ file lost 3 units) and their syntax tests
pass unchanged. Grammar archives add about 48 MB to a release binary (F# 14.1 MB, C#
5.3 MB, Swift 4.2 MB, Scala 4.0 MB, Haskell 3.9 MB, Kotlin 3.5 MB, the rest under 3 MB
each). Parsing is bounded by work, not by time: every parse runs under a progress
callback (`parse_with_options`) that counts its checks and stops the parse after a fixed
budget per source, so identical inputs stop identically on any thread count or host
load; T008 calibrates the budget so that no spike-corpus file that parses within a
second on a quiet host is stopped, and a stopped parse is handled like a panic
(§ Parallel indexing), because deeply nested Haskell and F# inputs take tens of
seconds. Constructs the pinned grammars do not parse are named limitations: VB.NET
nested types, alias and XML imports; Perl `require "file"` and fully qualified
`sub A::B::c`; F# signature-file member signatures; C++20 module imports.

### Parallel indexing

Refresh builds the search documents of each page's pending sources on at most
min(available parallelism, 8) threads, with at most 64 MiB of source bytes handed out
and not yet consumed (a source is consumed when its documents are added; one source
always fits). Each source has one outcome: its documents, or, when its parse panics
or is stopped, a named scan failure and the plain-block documents of an unmapped
source, whose first document carries the `kind` term `unparsed` (all are rendered and
searched exactly as blocks); it is never dropped. The one writer adds documents in key
order as their builds finish; after
every thread has finished it commits and reloads, and only then are matching pending
keys cleared, as before. Cancellation is checked at each hand-out and after the commit:
it stops handing out work and waits for the threads; a page whose sources were all
handed out may still be added and committed, and its pending keys then stay pending
until a later refresh, whose replay converges. A refresh on 8 threads yields the same
documents (field values, in key order) and the same store tables as on 1 thread;
physical file bytes are not compared. `status` counts the documents carrying
`unparsed` (one per source, counted without loading them) and samples at most 20 of
them, so its work stays proportional to the failures, until each source is parsed
again: on its next change, or on `repair-index`, which rebuilds every source's
documents. A source with no searchable text (empty or whitespace only) has no
documents either way and is named only in the scan report of the refresh that hit it.

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
