# From two originals to a feature review after an edit

## Operator workflow and acceptance

Give a project a meeting export and a specification once, before deciding which
features need them. Use a stable project notebook and the existing create and
capture operations. Supply the original text with its attributed source label
or capture an admitted, indexed file. The receipt identifies its owner, entry
and revision. Retrying a capture returns the same entry. A change to a pasted
document explicitly supersedes its preceding capture. Its lineage survives a
change of label. Indexed evidence becomes stale when the indexed file changes.

A feature's current requirement, constraint or decision can cite an exact
passage of either original. Two features may cite the same passage. Each
citation identifies who confirmed it and which claim it supports. The feature
stores a reference to the passage. Both original documents remain recoverable
through inspect and peek. A source edit never rewrites a feature's claim.

An operator may teach Foundry a phrase when confirming the citation. From a
fresh client, ask in team language which features need review. The answer
names features, quotes the claim, distinguishes each changed or missing support
from every surviving independent support for that claim, and points to the
original, its successor when available, and any linked code. A wording-only
match asks whether the cited claim is what the operator means. Only an explicit
choice can create or replace a citation. The workspace review queue remains
available when the question uses entirely new vocabulary. A matching billing
term supplies no evidence that a billing feature depends on the same decision.

## Implementation plan

1. Extend 008's existing task record with optional, attributed citation
   history. Stamp new captured entries with the existing workspace monotonic
   revision so deleting and recreating the same bytes cannot revive a link.
   Older captures have their original legacy identity. Read them in place.
   Use the existing writer, optimistic feature and source revisions, size and
   capacity limits. Compare the selected quote and current source during the
   write. An exact replay is idempotent. Reconfirmation explicitly selects the
   corrected passage, and withdrawal retires the citation. Keep the history.
2. Add cite_source on the feature and source_review at workspace scope to the
   experimental task_context interface. Read both ends in one snapshot,
   following explicit capture supersession. Display old and current evidence,
   marking excerpts clearly when a passage has changed. Verify indexed handles.
   Enumerate bounded pages of claims with continuation and honor cancellation.
   Resolve source identity before using any learned wording. Reuse Foundry's
   local passage matching. Report wording leads separately from established
   feature dependencies and review state.
3. Use two features with shared spec support, one independently supported by
   a meeting, and an unrelated billing feature. Exercise capture replay,
   cross-feature confirmation, a fresh process, an edit, correction, and
   missing or recreated evidence. Verify recovered quotations, authors and
   revision identities. Open the cited code through existing context/retrieve
   when a claim has code links. The client can use natural questions without
   supplying a feature or source ID for discovery.
4. Run the local repository gates on the implementation and exercise the
   installed executable through fresh local MCP sessions using isolated
   operator data. Record the actual boundary, update the handoff and publish
   the green commits to the experimental branch with remote readback.

The durable record and original capture remain in 008. This contract owns MCP
delivery in 003. The first implementation uses explicit source attribution
and confirmation, with no external service, model, background worker or
new store. The review reflects the latest committed Foundry index or supplied
correction. Source text outside those boundaries is unknown.

## Commands and output

Choose a notebook task ID and create it once. For each original use capture
with its title or indexed path in source. Preserve its receipt. For local
pasted text keep a stable source label and author on retries. Send supersedes
with the old entry ID when explicitly recording an edited or renamed version.
A project notebook can contain many independent originals.

On the feature task, put the decision inside `edits[].entry`. Use the
feature's latest revision for `expected_revision` in each write.

```json
{
  "op": "apply",
  "expected_revision": 6,
  "edits": [{"op": "add", "entry": {
    "id": "preserve-copy", "kind": "decision",
    "text": "Cleanup preserves the editor's separate amendments.",
    "author": "Project owner", "provenance": "Feature decision", "source_links": []
  }}]
}
```

To declare reliance from another feature, use the same nesting with
`kind: "requirement"` and set `entry.relies_on` to the complete
`decision_ref` returned by `decision_uses`. Once a supporting passage is
documented or confirmed, cite it on the feature. Take `origin_entry_id`
and `capture_revision` from the original capture receipt.

```json
{"op":"cite_source","expected_revision":7,"entry_id":"preserve-copy","origin_task_id":"notebook","origin_entry_id":"<capture entry>","capture_revision":3,"quote":"Preserve an independently edited copy.","author":"Project owner","wording":"Can we retire the parent?"}
```

The optional wording is an explicit clarification. The optional offset
disambiguates repeated text by its UTF-8 byte position. An existing
decision_uses declaration lets a second feature inherit the decision's
citations and quote its own reason. It can cite the meeting separately.
Read with workspace_id alone and the following command.

```json
{"op":"source_review","text":"Can we retire the parent?"}
```

When the operator starts with one recovered decision, `decision_uses`
shows its directly cited originals' status and one changed source when
available. Each confirmed dependent quotes its own requirement. If the
requirement directly cites a current original, show one `current_support`
with its attribution and exact range. Prefer a capture lineage the decision
does not already cite. `same_capture_lineage_as_decision` compares the
notebook and original capture lineage. Distinct lineages alone do not
establish independent authorship. `needs_source_review` flags a change in
the decision's direct citations or the returned requirement's direct
citations. Wording-only leads remain clarification questions. The complete
support comparison is available in `source_review`.

Read every page using next_after_id. Each page examines up to eight tasks and
sixteen claims and returns up to four claims. A change to the question,
task records or indexed sources requires a new first page. Each claim names
its feature and review status, the basis of a question match, any declared
dependency and the quoted sources for direct and inherited support. A needs_review
row without a question match belongs to the workspace queue. Current describes
the recorded source revision.

After an edit, compare the exact bytes of the originally cited passage with
the current supplied capture or the complete current indexed file at its path.
A unique occurrence returns `quote_match.status = unique` and a current
excerpt limited to those exact bytes. Its handle or capture byte range lets
the operator reopen it. `moved` compares the byte positions within the two
supplied captures, or within the same indexed file. A change of origin with no
comparable position leaves `moved` null. Zero occurrences give `absent` and
multiple occurrences, including overlaps, give `repeated`. In those two cases
the current excerpt has role `position_preview`. A unique match has role
`exact_quote`. Missing or inaccessible evidence
cannot establish absence. Each independently cited source retains its own
status. Every changed source still needs review because intervening edits
can change the meaning of a verbatim passage.

Each claim can cite four distinct source lineages directly and inherit the
cited decision's direct sources. A new confirmation on the same lineage
replaces that claim's active citation. Withdraw it with withdraw_citation,
expected_revision, citation_revision and author. Inspect shows the full
attributed history, with up to 128 citation events per feature. Ordinary
task export and forget retain their existing ownership boundaries.

For a new change proposal, propose_change can also find a decision through
a previously confirmed question or answer. It rechecks up to four matching
examples when ordinary discovery leaves an open candidate slot. It returns
the prior question and answer with attribution and asks whether the decision
applies to the new proposal. Dismissed examples and changed supporting
evidence cannot establish that interpretation.

## Design references and compatibility

WikiTrace's turn_main recovers original words from a derived view. Foundry
keeps the referenced capture in its existing task record and supports exact
byte reads. [Renderer](https://github.com/yukyeongleee/wikitrace/blob/dc06570f2fc69e20d36fede62128a806e35edfcd/src/wikitrace/memory/rawturns.py#L115) and [traversal test](https://github.com/yukyeongleee/wikitrace/blob/dc06570f2fc69e20d36fede62128a806e35edfcd/tests/unit/test_traverse_generic.py).

Graphiti's resolve_edge_contradictions retains temporal evidence. Foundry
uses a human-confirmed source range, explicit supersession and transactional
revalidation on read. [Resolver](https://github.com/getzep/graphiti/blob/231ebdcf6409c7e9bcf57bea328108cd64297777/graphiti_core/utils/maintenance/edge_operations.py#L795) and [associated tests](https://github.com/getzep/graphiti/blob/231ebdcf6409c7e9bcf57bea328108cd64297777/tests/utils/maintenance/test_edge_operations.py).

Hindsight's reciprocal_rank_fusion and candidate caps motivated keeping a
bounded second source of wording leads. Foundry considers prior human answers
locally and verifies the original decision identity. [Fusion](https://github.com/vectorize-io/hindsight/blob/550ef5e304623be2a456913acbac1b1a834fb2c3/hindsight-api-slim/hindsight_api/engine/search/fusion.py#L29) and [cap tests](https://github.com/vectorize-io/hindsight/blob/550ef5e304623be2a456913acbac1b1a834fb2c3/hindsight-api-slim/tests/test_fusion_cap.py).

Older stores open without a migration. Use an updated reader after writing
new capture stamps or citation history. An older binary rejects unfamiliar
task fields. Historical readback and the existing task export remain available
in the updated reader.
