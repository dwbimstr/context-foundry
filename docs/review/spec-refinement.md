# Historical spec refinement: superseded portfolio

Historical record only. The [subtraction review](subtraction.md) and [current portfolio](../../specs/README.md)
supersede this document's contract locations, scope and readiness conclusions. Counts,
FR/SC IDs and commands below describe earlier drafts; they are not current instructions.

Date: 2026-09-28. Scope: all 15 Context Foundry specs, their plans and task acceptance/
verification mappings; deeper current-source tracing for source/index recovery, context
packing, feedback and Laya. The owner requested multiple passes and confirmation of
continuous fine-tuning. This is an in-session self-review, not independent counsel.

## Conclusion and coverage

The initial portfolio was a useful first design, not 15 fully implementation-ready
specifications. The earlier link/ID/dependency checks could not detect prose cycles,
training-runtime gaps or a task whose acceptance depended on later work. The revised
portfolio now has concrete scenarios throughout and detailed boundary contracts for
agent responses, learning data and the recurring Laya loop. Future capabilities retain
named selections rather than pretending unknown models/hardware/providers were chosen.

All 15 spec and plan bodies were examined. Task scopes, dependencies, acceptance IDs
and verification were inspected across all bundles; repetitive budget/rollback prose
was not treated as evidence of implementation readiness. Current Rust source was
traced selectively in store.rs, laya.rs and lib.rs. Laya source, package entry points,
evaluation docs/parser and selected notebook code cells were read without executing
training. This is not another full audit of the 71 predecessor implementations.

## Pass 1 — product scenarios and completeness

Replaced generic “perform the operation, see SC” scenarios in all 15 specs with
concrete given/when/result cases. Tasks now include their corresponding observable
witness. Examples cover stale handles, incomplete scans, missing providers, source
updates, wrong memory scopes, partial migration, lost responses and unknown usage.

Specs 012/013 gained explicit requirements and acceptance for token/label order,
bounded atomic datasets, split profiles, interrupted jobs, prediction identity,
round-to-round leakage and regression/forgetting. Continuous fine-tuning is explicitly
part of the ecosystem; the first learned decision is search-versus-graph routing.
It does not imply fine-tuning a coding language model or the embedding model.

## Pass 2 — architecture, runtime boundaries and simplification

| Finding | Concrete failure | Disposition |
| --- | --- | --- |
| High: hidden 005/006 cycle | 005 waited to publish its graph contract until 006 ran a producer, while 006 required 005 | Removed the reverse gate; 005 uses a captured lawful fixture and 006 supplies live adapter proof |
| High: Laya training entry point unspecified | A scheduler could invoke a notebook with installation/download/Hub cells and hardcoded paths as if it were an unattended local trainer | Added external L1 training adapter and L2 serving adapter contracts and task prerequisites |
| High: first-round acceptance depended on later work | 013 SC-001 demanded two real rounds while T003 owned the second one | SC-001 now owns round one; SC-006/T003 owns repeatability and round two |
| High: split policy could collapse a single-repo dataset | Grouping every row by repository as well as task puts all local examples into one partition | Default task-held-out profile; separate repository-held-out profile and explicit claim boundary |
| High: health-only candidate binding had a race | A model could change after health check and before prediction, yet the answer could be attributed to the old candidate | Bind each prediction to the captured selection/recipe identity; mismatch falls back |
| Medium: unused operation journal | First serving commands already have source/feedback identity, yet the plan added a generic durable outcome registry | Use entity identity/revision and named unknown/conflict results; defer extra journal until a real operation needs it |
| Medium: Laya validator wording contradicted its plan | The spec implied a ready training validator while the plan denied any validator CLI | Distinguish existing evaluation-format validation, training sequence parity and missing unattended training command |
| Medium: exact count inside counted response | Serializing the count changes the bytes being counted | Final result is checked exactly; count is recorded out of band, with the accounting boundary explicit |
| Medium: new scenarios were not executable handoffs | ID coverage passed even though tasks only repeated task names | Added concrete observable cases and boundary/fault contracts; readiness still reports remaining selections |

These are source/plan findings, not reproduced runtime incidents. Recommendations
are strong where they remove the demonstrated ambiguity/cycle; selected libraries
remain hypotheses subject to the first recovery test. No alternative storage engine
or new orchestration layer was introduced by this review.

## Laya source check

Read-only checkout: `4066d5d5fbf08b66c6757ddeedbd797bd7655bc0`; working tree clean at
inspection. Notebook SHA-256:
`6b81f290bbd213008d3e79c80d207b9abc1d1b5ab0a23f3f4bd9a289611433ed`.

- `pyproject.toml` registers `laya`, `laya-serve`, `laya-evals` and MCP; no training CLI.
- `laya/evals_cli.py` and `laya/evals.py` supply real evaluation-format validation.
  Its permissive row parser is insufficient for consent, recipe and split admission.
- `laya/common.py::build_sequence` uses supplied option order and token budgets;
  training/serving conversion must use the same recipe and reject incompatible input.
- Notebook cell 6 creates training items without task-group lineage. Cell 8 withholds
  calibration rows before training (lines 86–99 in that cell), trains encoder/head,
  and exports per-type temperatures after removing inherited bucket overrides.
- The README still says calibration comes from training items. The code disproves
  that statement at the inspected revision. The remaining issue is **row-level**
  rather than **task-group-level** separation. The initial commentary inference from
  the README was corrected after tracing the code.
- Notebook rolling checkpoints contain weights/config/tokenizer and epoch metadata;
  these do not establish exact optimizer/RNG/data-position resume. The planned first
  adapter restarts interrupted attempts from a fixed base instead of claiming resume.
- `Agent(model_id_or_path=...)` supports local checkpoint loading; the current server
  resolver uses packaged-model aliases. Custom-candidate serving and identity-bound
  predictions therefore remain explicit L2 work, not an existing integration claim.

No predecessor source, private data or notebook code was copied into the new application.

## Pass 3 — handoff and consistency

Checked revised requirement→task→contract mappings, local links, command/playbook
pairs, dependency metadata and actual prose ordering. Retained the same 15 capabilities
and release cuts; no new feature spec or measurement campaign was added.

The public evaluation example was exercised against the real local parser:

```sh
# From the separately installed/pinned Laya checkout; FOUNDRY_ROOT is your checkout path.
PYTHONDONTWRITEBYTECODE=1 HF_HUB_OFFLINE=1 TRANSFORMERS_OFFLINE=1 \
  python3 -m laya.evals_cli validate \
  "$FOUNDRY_ROOT/specs/012-learning-data-contract/contracts/eval.public.jsonl"
```

Result: exit 0, `2 examples, 1 question id(s): strategy`. The executed invocation used
the explicit local fixture path. No model was loaded, no dependencies were installed,
and no network or training command ran. This proves parser compatibility only; the
real tokenizer check and the learning acceptance criteria remain open.

Structural recheck: 15 bundles, 45 tasks and all 163 locally declared requirement/
acceptance/scenario IDs covered; 71 predecessor dispositions retained; 10 command/
playbook pairs; relative links resolve and declared dependencies are acyclic. These
counts show consistency only, not a proof that a proposed system works.

The [readiness table](../../specs/README.md#task-detail-and-remaining-execution-inputs) records
which selections remain. All feature statuses remain Proposed. No Rust application,
build or test files were changed, so the prior Rust suite was not rerun. The structural
checks and whitespace check were rerun for the changed planning artifacts.

## Stopping rule

This round used three lenses: completeness, architecture/runtime ownership, then
handoff consistency. Further review should target a named unresolved decision and
the selected spec, with a time budget. A second substantive review may assess the
chosen implementation contract; do not restart a full-portfolio audit for every fix.
A remaining safety defect is fixed or scope is reduced, never waived by a review limit.
