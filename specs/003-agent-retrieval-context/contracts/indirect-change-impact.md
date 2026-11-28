# Indirect change impact

An operator can paste a change request in their own words and ask what else
would be affected. They need to see the current rule, the other feature that
uses it, the reason that feature depends on it, and where to check the code.
The request itself remains a proposal until the operator makes a policy decision.

The task-enabled project and MCP instructions apply this distinction to an
ordinary request to change product behavior. Before a code or spec edit, the
client surfaces an open or conflicting choice with the saved rule, attributed
dependencies and enforcing code. It asks which policy to implement and waits
for the answer. The client proceeds on an explicit choice and omits unrelated
decisions when there is no applicable conflict. If meeting accounts occupy
both passage slots, proposal impact can still offer a directly matching saved
decision and its declared dependents. The two-candidate response cap and
explicit scope confirmation still apply.

## Product contract

1. Accept the original words and their attribution at workspace scope. The
   caller supplies no feature or decision ID. Preserve the original and return
   a durable receipt. Repeating the same input and attribution recovers it.
2. Use the existing bounded workspace passage search to locate leads. A
   matching meeting note can lead through its current accepted passage review
   to a decision in that feature. Show the quoted lead, reviewed passage,
   reviewer and candidate rule. If the rule is adjacent in the same indexed
   capture, say so explicitly. Shared words and adjacent sentences can suggest
   applicability. Only the operator can confirm that interpretation.
3. Read the decision's existing dependency declarations and exact adopted
   sources. Name a second feature and quote its own reason with attribution.
   Distinguish this documented dependency from the proposed change's possible
   applicability. Provide a revision-bound context call for locating the
   implementation. A client retrieves the code before describing its behavior.
4. Ask one focused question when the proposal's scope is unclear. An explicit
   answer identifies the relevant decision. A dismissal closes the question.
   Persist the operator's words and identity. Neither action approves a change
   to the rule. A fresh session can reopen the receipt and recover the outcome.
5. Recheck the supporting entries, review and source identities during reads.
   If support expires, keep the original and answer, withdraw the supported
   interpretation and surface the gap. Do not silently transfer an answer to
   a different decision. Report bounded search and pagination limits.

## Implementation sequence

1. Extend the experimental task-context interface with a workspace operation
   to save a proposal and receipt-based operations to revisit and clarify it.
   Use the existing transactional task
   store, size bounds and revision checks. Store operator input separately from
   decisions and feature requirements. Reuse existing source and decision IDs.
2. Follow the current accepted review from a matched note. Match a decision
   against the exact indexed bytes inside the reviewed capture. For adjacency,
   allow only a short whitespace gap. Deduplicate candidate decisions; expose
   alternatives and incomplete discovery. Reuse the shared-decision reader
   for feature impact and scoped context for implementation discovery.
3. Exercise the public request parser and durable store. Check an exact retry,
   ambiguous or missing support, a dismissed or replaced review, a recorded
   choice and a fresh read. Check that the proposal never becomes policy.
4. Replay the saved correction and cleanup features over local MCP. Start with
   the ordinary-language request, quote the second feature's evidence and
   retrieve the current Rust guard. Exercise a recorded answer and dismissal
   in isolated copies, then reconnect. Record what was actually demonstrated.
5. Run the existing repository gates on the code commit. Install the qualified
   desktop executable, replay over a fresh local connection, update the handoff
   and dated evidence and verify the experimental branch by remote readback.

## Acceptance and scope

Completion requires a durable original, a traceable conditional answer about
both saved features, retrieved implementation bytes, one actionable question
and cross-session disposition with stale evidence withdrawn. The negative
control is similar wording without a supported relationship. Missing or
ambiguous evidence must leave the operator in control. Existing task stores
remain readable and ordinary retrieval keeps its existing behavior.

This slice applies the roundup's priorities of useful memory, source-backed
relationships, clarification and continuity. It runs locally with the existing
experimental switch. Broader semantic discovery and automatic capture across
applications remain separate product work.

## Operator interface

With experimental task context enabled, use the workspace ID from status.
Send the following command in `task_context`, without a task ID.

```json
{"op":"propose_change","text":"Someone rewrote the team's shared guidance. Can I clean up the original without losing their work?","source":"Planning chat","author":"Operator"}
```

The receipt gives the proposal's task ID and revision. It includes the original
words, current candidates, the scope question and a separate policy status.
For each candidate, follow its decision and dependent citations. Pass
`impact.implementation_context` to `context`, then pass the returned code
handle to `retrieve`. For subsequent pages of dependents, use `decision_uses`
with the candidate's originating feature, revision, decision ID and cursor.

To reopen, use the proposal's task ID and `{"op":"proposal_impact"}`. To
answer, use `clarify_proposal` on that same task with `expected_revision`,
`author`, the operator's verbatim answer in `text` and `disposition`. An
acceptance includes the chosen candidate's `scope_token`; dismissal omits it.
The response acknowledges the write. Reopening returns the current impact.
On a revision conflict or interrupted acknowledgement, reopen before retrying.
Prior answers travel with the ordinary task export. Explicit task forget
removes the proposal and its answers together.

Each distinct original, source and author uses one ordinary task slot. The
existing limit is 128 live tasks per workspace. A proposal accepts 4,096 UTF-8
bytes, stores up to 128 attributed responses and shares the existing task size
limit. Initial discovery examines the two leading passages and returns up to
two distinct candidate decisions. Each dependency read uses the existing
eight-task page. A confirmed scope is checked directly by its original
identities even when new text outranks it.
