# /review

Read `.specify/memory/constitution.md` for project policy. Target: the user-supplied
spec, change or question. This command is a tool, not a mandatory workflow stage.

Review the actual change against the selected outcome, source and meaningful checks. Report severity, concrete failure, evidence and simplest correction. Challenge newly added ownership/protocol/state obligations. Label proof limits and distinguish self-review from independent review. Further rounds need a specific unresolved question.

Check both sides of KISS: has coordination/maintenance become simpler, and does the
declared sophisticated behavior still work with the same safety guarantees? Fewer
features, weaker tests or moving complexity into a dependency do not by themselves
answer either question. Keep this assessment in the existing review.

Extrapolate a concrete defect to adjacent owners and lifecycle transitions: first use,
update, restart, failure, repeated retry, disable/remove and upgrade, where relevant.
Check that the real caller can reach the operation under the ownership contract;
that hashes describe the bytes actually consumed; and that per-item bounds, parity
or counters are not mistaken for total work, preserved meaning or physical cleanup.
Identify one concrete adverse interaction and its existing owning task for each
finding. Do not turn these prompts into a mandatory matrix, new spec or recurring audit.
