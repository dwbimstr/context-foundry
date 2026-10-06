# 007 — One budgeted context across explicitly admitted repositories

Status: Active. Reactivated 2026-10-03 by the owner's selection of the token-economics
tranche. T001 implemented and verified locally 2026-10-04, accepted at the OpenAI Sol
reviewer's delta SHIP; committed in `cc402e0`, unreleased ([validation](../../docs/validation.md)).
Dependencies: 001 T004–T006 (v2 handles, header and candidate seam; committed in
`5edf32c`) and 003 T001–T002 (owner, transports, deadline and allowance). The
2026-09-28 deferral stub is superseded; Git history retains it.

## Outcome

An operator explicitly admits outside repositories when launching the Foundry owner.
An agent then asks one question and receives one cited, budgeted response with
evidence from several roots. Each handle validates against its own root, re-indexing
one root never touches another, and a root that cannot serve is visible rather than
silently missing. Query text never admits a root.

Re-entry record: the deferral asked for two named repositories, a question needing both,
expected references and a recorded failure of separate queries, and required
identity-collision and partial-workspace behavior before implementation. On 2026-10-03
the owner selected reactivation directly, for combined, cited, single-budget context
over three repositories; that selection supersedes the request for a recorded failure.
The economic case: separate stores cost one call, one header and one budget per
repository and leave cross-repository ranking to the agent. Identity collision
(`root_id_collision`, `ws16` routing) and partial-workspace failure (per-root coverage,
`roots_unavailable`) are specified below.

- **FR-001:** Roots are admitted only at owner launch, validated before serving.
- **FR-002:** Admitted roots have stable aliases; labels are for display only.
- **FR-003:** One engine worker owns every opened store, with unchanged admission.
- **FR-004:** One search/context call merges ranked candidates from the selected roots
  under one deadline, one budget, one reservation and one charge.
- **FR-005:** Headers and status name every root's revision or coverage.
- **FR-006:** Identity, freshness and graph evidence stay inside each root.

## Admission at launch

`foundry --store DIR mcp --root ROOT [--reference ROOT=STORE]...` (stdio or HTTP), with
the same flags on `foundry connect`, which records them in `ConnectInfo.references` and
the printed launch argv. At most 8 references. Query text never admits a root; `roots`
and `root` only select already admitted aliases.

Before serving, canonicalize every root and refuse with an invalid-argument exit (2):

- `duplicate_root` when two canonical roots are equal;
- `nested_root` when one root contains another at a path-component boundary;
- `root_id_collision` when two roots share `ws16` (the first 16 hex characters of
  `workspace_id`);
- `too_many_roots` beyond 8 references.

The primary keeps today's fail-fast open: busy, wrong workspace or unsupported schema
stops startup, and a broken lexical index permits authoritative-only startup. Each
reference store is opened once, at startup; the outcome is that root's coverage for the
whole session: `ok`, `missing_store`, `busy`, `unsupported_schema`, `corrupt` or
`wrong_workspace` (a store bound to another root). There are no retries; restart the
owner to retry. A root whose authoritative store opens but whose derived index needs
repair has coverage `repair_required`: retrieve and status still work for it, while
search and context skip it. A reference store held by another live Foundry owner
reports `busy`; the other owner is never stopped.

## Aliases and labels

Aliases are `primary`, then `ref1`…`ref8` in admission (command-line) order; they are
the only addressable names. A root's display label is its basename with control
characters replaced by `?`, truncated at a UTF-8 boundary to 32 bytes. Labels may repeat
(two roots named `foo`) and never address anything.

The tool catalog is identical with or without references: `roots` and `root` are always
present, and `primary` always exists, including when no references are configured.

## One owner

The single engine worker owns every opened store. One-active/zero-queued admission is
unchanged: a multi-root call holds the one engine slot. `index {timeout_ms?, root?}`
re-indexes that admitted root (default `primary`) through its own store; a root whose
coverage is not `ok` returns `root_unavailable`. `status` reports every root.

## Combined search and context

`search` and `context` take `roots?`: a nonempty list of unique aliases (at most 9),
validated before dispatch; an unknown alias is `invalid_argument`. Without `roots`,
every root whose coverage is `ok` is selected.

Roots run sequentially inside the existing 5000 ms read deadline; there are no per-root
time slices. The parent deadline or a cancellation fails the whole request, and the
engine slot stays held until the executing call returns. For each selected root the
engine calls `search_candidates` or `context_candidates`
([context v2](../001-source-state-recovery/contracts/context-v2.md), § Candidate seam);
each batch is already revalidated in that root's final read transaction.

`src/roots.rs::merge` orders the batches:

1. tier-1 items from all roots first, by root order, then in each root's own tier-1
   order (context-v2 § Two-tier query; amended 2026-10-06, replacing path, start);
2. then tier-2 items by reciprocal-rank fusion `1/(60 + rank)`, where `rank` is the
   item's 1-based position in its root's tier-2 list; ties break by root order
   (primary, then command-line order), path, start.

The per-file cap of 4 applies per (root, path) after merging. Context keeps 32 units
and 3 outlines in total: graph items follow the first merged unit, in root order, and
the outlines are those of the first three distinct (root, path) files among the merged
units. One effective budget, one reservation and one charge cover the whole response;
packing is the v2 ladder over the merged list, never a merge of packed responses.
Compact context (context-v2 § Compact context, amended 2026-10-06) sums each marked
run's definition count over the merged roots before applying its rule, so a name
defined once in each of two roots is not unique; the merged batch carries the sums.

Selected roots that cannot serve appear in the header with their coverage. If none can
serve, the request fails with `roots_unavailable`, whose message lists each root's
coverage within the 1024-byte error bound. Authoritative corruption discovered during a
request fails the request.

## Response header and status

In an owner launched with references, segment 2 of the
[v2 header](../001-source-state-recovery/contracts/context-v2.md) lists per-root
segments in root order: `<alias>(<label>) r<rev>`, followed by ` scan:<state>` and
` pending:<n>` when not at their default, or `<alias>(<label>) <coverage>` for a root
that cannot serve. Without `roots`, every admitted root is listed, so unavailable
references stay visible; with `roots`, the selected roots are listed. An owner without
references keeps the single-root `r<rev>` segment. For example:

~~~text
foundry context · primary(foundry) r6 · ref1(laya) r12 · ref2(prakarana) busy · budget:2048 · shown:5
~~~

Graph items carry their seed root's alias: `edge <alias> <text>`. `status` JSON adds
`roots:[{alias,label,root,workspace_id,coverage,source_revision,pending_sources,scan_state,index_state}]`;
fields that need an opened store are null for a root that could not be opened.

## Identity and graph scope

The normative text is the v2 contract's § Multi-root identity. In summary: a handle
names its root through `ws16`; an unknown `ws16` is `wrong_workspace` and a known root
that cannot serve reads is `root_unavailable`. Candidates are revalidated in their own
root's final read transaction, with no global snapshot. Graph expansion and endpoint
validation stay inside the seed's root; graph deduplication keys on `(workspace_id,
stable fact identity)`; no cross-root edges exist until a producer artifact
establishes them. Re-indexing one root never touches another. No registry, federation
daemon, watcher, or cross-root training or consent change is added.

## Limitations

- No cross-root edges, symbol resolution or global snapshot: one response can cite
  roots at different revisions, each reported in the header.
- Reference stores are held exclusively by this owner while it runs; a CLI writer gets
  `store_busy`, and a store held by another owner stays `busy` for the session.
- No per-root time slices: a slow root can consume the shared deadline, failing the
  request with `deadline_exceeded`.
- Unavailable references are not retried within a session; restart the owner.
- Labels are display-only and may collide; only `ws16` collisions are refused.
- 009 semantic candidates and the 013 policy are unimplemented; when they land, their
  owners state how they apply per root.

## Tasks and acceptance

SC-001 is the acceptance of T001.

### T001 — Admit, query and cite several roots in one budgeted response

- **Depends:** 001 T004–T006 and 003 T001–T002. **Scope:** new `src/roots.rs`
  (admission validation, aliases, coverage, `merge`) with `pub mod roots;` in
  `src/lib.rs`; `src/mcp.rs` (`ServerOptions.references`, engine state holding all
  roots, `roots`/`root`, the multi-root header, status JSON `roots`);
  `src/adapter_cli.rs` (`--reference` on `mcp` and `connect`); `src/bootstrap.rs`
  (`ConnectInfo.references`, launch args); `src/response.rs` (multi-root header and
  graph alias); `tests/mcp.rs` helper updates (`start_http` and stdio launch options);
  new `tests/multiroot.rs`. It uses the v2 `CandidateBatch` seam and never merges
  packed responses.
- **Outcome/acceptance (FR-001–FR-006 / SC-001):** launch-time admission, aliases,
  coverage, merged search/context, per-root header and status, and root-scoped
  identity as specified above.
- **Verification:** `tests/multiroot.rs` uses three temporary repositories, each with
  `src/lib.rs`, one identifier defined in two roots, and a reference store held by
  another process. Duplicate, nested, colliding (through a test seam for `ws16`) and
  too-many roots are refused before serving. Aliases are `primary`, `ref1`, `ref2`,
  with labels checked for roots named `foo`, `foo`, `foo~2` and a control-character
  basename. One `context` call returns items from at least 2 roots with distinct
  `ws16` under one budget and one charge. A busy reference shows `refN(label) busy` in
  the header while the others serve; when every selected root is unavailable the call
  fails with `roots_unavailable`. After `index {root:"ref1"}`, a pre-edit ref1 handle is
  `stale_handle` while primary and ref2 handles still retrieve. Identical graph rows in
  two roots stay distinct. An unknown alias is `invalid_argument`. A query naming a
  fourth repository's path admits nothing. A `test-faults` stall beyond 5000 ms in the
  second root, after the first produced hits, returns `deadline_exceeded`, and the slot
  stays busy until the stall returns.
- **Review/cutover:** no registry, federation daemon, watcher or cross-root edge;
  references never change another store's records. Run the checks listed for 001
  T004–T006, including `cargo check --all-targets --locked` under the actual Rust 1.90
  toolchain (see 001's SC paragraph).
