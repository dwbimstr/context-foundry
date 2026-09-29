# Shared context contract v1

Status: Proposed. Source/CLI owner: [001](../spec.md); MCP adapter owner:
[003](../../003-agent-retrieval-context/spec.md). This file moves the former 003
response contract to its lower-level owner; 001 does not depend on implementing MCP.
Limits are selected engineering bounds, not measured capacity promises.

## Identity and reference validation

`workspace_id` is lowercase SHA-256 of the bound canonical absolute root's UTF-8
bytes. It scopes references to a logical local workspace, not a global registration
service. A moved root requires a new store; no implicit rebind. A copied store retains
its binding. `source_revision` is a checked u64 incremented atomically for each source
add/change/delete, beginning at 0 on schema-2 initialization. Search refresh, graph
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
freshness guarantee. Serving another repo currently means explicitly indexing it
in a separate store and addressing that store. This does not imply automatic result
fusion, cross-repo symbol resolution or a shared snapshot. Reconciliation/deletion
in one store cannot affect another. Watchers remain deferred; any future watcher
must use the same bound root and ingestion owner, never infer roots from text.

A source handle is a JSON object with exactly:
`{v:1, workspace_id, path, sha256, start, end}`. Path is normalized workspace-relative
UTF-8 using `/`, 1..4096 bytes, no empty/`.`/`..` components, absolute prefix, NUL,
CR or LF. Hashes are lowercase 64-hex SHA-256. Start/end are u64 half-open byte offsets.
Ranges must satisfy `0 <= start < end <= source_bytes` with UTF-8 boundaries. The
only empty range is `[0,0)` for an empty source. Citations show one-based inclusive
line numbers; byte ranges are authoritative. Preserve CRLF and all source bytes.

One response validates source hashes, memory revisions, selected graph snapshot and
semantic mappings against one final authoritative read transaction. Report that
transaction's source revision. Provider work runs before this transaction; never pin
it while waiting for a model. Candidates collected earlier must be revalidated here.
A changed/deleted candidate is omitted with the existing stale/coverage reason, not
relabeled with the new revision. No retry loop to chase a moving workspace, global
publication epoch or promise of current-disk freshness is added. Old successful replies
remain historical observations; later `retrieve` revalidates their handles.

Validate in order: field/type/version/bounds → workspace match → source existence →
source hash → range. Errors: `invalid_argument`, `wrong_workspace`, `not_found`,
`stale_handle`, `invalid_range`, respectively. No partial evidence on these failures.
Reconstruct source in stored chunk ordinal order (at most 2 MiB) and verify full
length/hash before slicing. Missing/mismatched chunks are `corrupt_source`, never
an empty successful result. No new offset table is required for this bound.

## Inputs, defaults and outputs

Reject missing required fields, null, wrong types, blank queries, non-finite numbers,
unknown application fields and out-of-range integers before mutation. Optional means
omittable, not null. Inputs are UTF-8; do not reinterpret query text as a shell command.

| Operation | Input/default | Successful application value |
| --- | --- | --- |
| search | `query` 1..4096 bytes after nonblank check; `limit` default 10, 1..64 | ordered `hits` with handle, line citation and verbatim text; `pending_sources`, `stale_candidates`, `candidate_limit:256`, `candidate_limit_reached`, `truncated` |
| context | `query`; `tokens` default 2048, 1..32768; `strategy` default `auto`, enum auto/search/graph | ordered evidence items, omissions and common metadata below |
| retrieve | one `handle`; `tokens` default 2048, 1..32768 | one exact span, its returned handle, and `next` handle or null for the remainder |
| index | bound root; explicit CLI root must match; MCP root is startup configuration only | 001 scan report; partial results include committed counts, reason and pending work |
| status | no application arguments | store schema, workspace ID/null if unbound, source revision/count, pending count, index state ready/lagging/repair_required, scan state never/running/complete/incomplete |

Search examines at most 256 candidates. Ties sort by source path then byte start.
`candidate_limit_reached` means the candidate window filled, not proof of additional
matches. `truncated` signals requested-hit/output/candidate limits; an empty search
never proves no matching source exists outside the examined window. Direct search
has a 256 KiB output cap, not a token claim; drop trailing hits to fit and signal it.

Storage chunks are not public reference identities or required embedding units.
Optional 009 semantic retrieval may nominate a whole source or section and select a
smaller verbatim range for delivery. Its `match_handle` describes the encoded range;
the normal `handle` always describes the returned bytes. Its selection/preview and
continuation metadata participate in the same accounting below. This extension does
not require 001 to implement a model or change the authoritative source representation.

CLI `retrieve --handle JSON --tokens N` accepts at most 32768 handle bytes; no shell
interpretation. Existing context stdout stays text, diagnostics on stderr. New handle
fields in search JSON are additive and tagged `format_version:1`. All successful
context/retrieve outputs include freshness `indexed_snapshot`, pending count, scan
state, tokenizer `o200k_base`, boundary, requested budget and `budget_satisfied:true`.
For text these are rendered metadata lines; MCP wraps the application result below.

CLI errors: exit 2 for invalid arguments/unsupported version or mode, 3 for store
busy, 1 for other runtime failures, 130 for cooperative user cancellation. Error JSON
on stderr is at most 1024 bytes and names `code`, `message`, `retryable`; source bodies
are never placed in errors. Partial index report may be on stdout but exit is nonzero.
Budget failure leaves stdout empty. Querying unbound workspace is `workspace_unbound`;
status remains available. `verify_current` is `unsupported_mode`.

## Failure scope

Open existing authoritative state independently of optional indexes/providers, as
specified by 001. A read never repairs, downloads, trains, enrolls a path or records
feedback implicitly. Operational counters may remain in memory; they do not join
source commit eligibility or create a durability plane.

| Failure | Behavior |
| --- | --- |
| Missing store, unsupported store schema, unreadable authoritative database | Named error; no invented empty state or automatic upgrade |
| Lexical index missing/corrupt/rebuilding | Search/context return `repair_required`; status and direct authoritative reads remain available |
| Graph absent/stale or a graph record cannot be decoded | Context can return valid source with `graph_unavailable`, `graph_stale` or `graph_invalid`; direct graph request names that failure |
| Neural runtime/profile/cache unavailable or invalid | Baseline retrieval with the named semantic reason; no implicit preparation |
| Policy configuration invalid or worker unavailable | Learned routing disabled with the named reason; deterministic retrieval still works |
| A requested memory record cannot be decoded | `corrupt_memory`; source operations remain available if the database itself is healthy |

Component-local decode failures do not certify database health. A database-reported
corruption/error is not downgraded to one of the optional fallbacks. Never label a
failed component an empty successful result. Include relevant degradation metadata
before output accounting, and preserve bytes for explicit recovery/export where readable.

## Packing and exact accounting

The CLI boundary is **all stdout bytes**, including any final newline. MCP boundary
is the actual compact UTF-8 serialized tool result value, including `content` wrappers,
`isError` and application metadata; JSON-RPC ID/framing, catalog and host/provider
wrapping are excluded and explicitly unknown. Count with `encode_ordinary` from the
locked o200k tokenizer. There is no estimated characters-per-token fallback.

MCP success is one text content block containing compact application JSON, with
`isError:false`, and no duplicated `structuredContent`. Count the actual outer result,
including escaping the inner JSON string. One serializer produces both counted and
emitted bytes. Do not attach fields after counting. Exact measured token count is
out of band; putting the count in its own counted message is unnecessary.

Build candidates deterministically: up to 32 source search hits, highest-ranked
source first, then bounded graph evidence and remaining source spans. For `auto`,
ASCII-lowercase the query and tokenize maximal runs of ASCII letters, digits or `_`.
Any whole token in `{calls,caller,callers,depends,impact,dependency,dependencies,
reference,references,usage,usages}` selects graph, otherwise search. This replaces
the prototype's substring rule: `preferences` and `calls_tracker` are not graph
keywords, while a question about references can use 005's supported relation. The owned
policy can replace that choice only under 013's identity/threshold contract. Explicit
search or graph never invokes the policy. Graph without eligible edges still returns source results
and a graph coverage reason. No claim that this heuristic is optimal.

Before optional policy inference, check graph coverage metadata against the indexed
source revision. If no supported graph scope is current, use search without a model
call and report graph stale/unavailable; do not query a router to choose an unavailable
branch. Current partial graph coverage permits bounded expansion, with its limitation
retained. Explicit graph still reports the same limitation and never invents relations.
This rule does not add graph storage to 001; before 005, compiler graph is unavailable.
When 009 is enabled, the order is query embedding → bounded lexical/dense merge →
optional head prediction using that same query vector → bounded graph expansion →
packing/final freshness validation. If the query vector is unavailable, skip the head
and use the deterministic rule. Never make an additional embedding call just to route.
All model work shares the read deadline; none occurs in the final read transaction.
003 adds a delivery `context_id` to context/retrieve envelopes under its
[adapter contract](../../003-agent-retrieval-context/contracts/adapter-economics.md).
That metadata counts toward the same limits; it is not an authoritative store record.

Deduplicate source by `(workspace_id,path,sha256,start,end)` and graph by its stable
fact identity: `edge_id` for resolved edges, `occurrence_id` for 005 occurrence evidence.
Reserve the metadata envelope, then include candidates in order only
when the fully serialized trial fits **both** token and 256 KiB byte caps. Remove
last-added items if final omission metadata changes the fit. Do not rewrite or
summarize source. `omitted_count` counts constructed candidates dropped by packing,
not all unknown matches; candidate/traversal/stale limits have separate named fields.
009 may construct a smaller source candidate under its explicit excerpt rules before
this packing step; it cannot silently truncate evidence or call a preview localized.

For context, zero fitting evidence with a fitting envelope returns an empty evidence
bundle with omissions. If the envelope cannot fit, return `budget_too_small` and its
exact minimum budget; no success is over budget. For retrieve, start with at most 128 KiB of the requested range, ending at a UTF-8
boundary. If the fully rendered result/continuation does not fit, halve that byte
length, retreat to a UTF-8 boundary and retry; always try the first complete codepoint
before failure. This is at most 19 nonempty trials and does not assume BPE monotonicity
or promise the longest possible prefix. Return the first fitting tested prefix and
advance `next.start` exactly to its end. Requested ranges remain bounded to one 2 MiB
source; no byte-by-byte quadratic tokenization loop. If no nonempty tested prefix fits,
return `budget_too_small` with a sufficient budget for the one-codepoint result; never
an unchanged continuation loop. `[0,0)` on an empty file returns an empty span if its
envelope fits. The sufficient hint is not advertised as a mathematical token minimum.

MCP errors are a bounded `isError:true` tool result with one text block containing
`{code,message,retryable}`; total serialized value <=1024 bytes. Such errors are
outside successful context budgets. Protocol errors use SDK error semantics. Escape
untrusted paths/text as data; no source text becomes an instruction or tool request.

## Contract checks

At budgets 1,32,64,256,1024,32768, assert exact serialized count and byte cap for every
success, bounded named errors otherwise, UTF-8/source equality and forward-progress
continuations. Include escaping, CRLF, multi-byte identifiers, maximum-length paths,
empty sources, stale/deleted/wrong-workspace handles, tiny budgets and omitted graph
items. Test graph-first starvation specifically: a fitting highest-ranked source
must precede graph annotations. No provider-cost or complete recall claim follows.
Round-trip a maximum-length valid path containing JSON-escaped characters through
search output and CLI/MCP retrieve; the handle-input bound includes serialization
overhead, not only raw path bytes. Invalid source paths are rejected at admission.
Inject an indexed edit/forget/graph replacement between candidate collection and final
validation: each response contains only data eligible in its reported read snapshot.
Break one optional component at a time and assert this failure-scope table. Known
unavailable graph or invalid routing configuration must not invoke the policy; a read must
not create a store or start repair. These extend the owning feature's focused cases;
they are not a new portfolio-wide validation stage.
