# Modified Spec Kit, used as tools

Keep familiar numbered specs, acceptance IDs, `.specify` policy/templates and `.omp`
command names. Use them to make decisions and finish work. Running every command is
not a prerequisite to shipping.

| Location | Purpose |
| --- | --- |
| [memory/constitution.md](memory/constitution.md) | Project principles and evidence policy |
| [templates/](templates/) | Optional document shapes |
| [templates/commands/](templates/commands/) | Procedures for the named task |
| [../.omp/commands/](../.omp/commands/) | Thin prompts forwarding arguments |
| [../specs/README.md](../specs/README.md) | Active workflows and parked dispositions |

## Normal use

1. Select a real user outcome; read/update its `spec.md`.
2. Resolve choices that block the selected slice; implement and verify that slice.
3. Record the result in that spec or the change/PR; release the supported scope.

A single spec can contain behavior, design, work and evidence. `/plan` and `/tasks`
can refine those sections without creating files. Use separate files only for real
complexity or a separately consumed contract. Small fixes do not need new numbers.

`/specify`, `/clarify`, `/plan`, `/tasks`, `/analyze`, `/implement` and `/review` remain
familiar entry points. `/measure` is only for a numerical question that changes a
decision; it is not a universal close step. `/council` is optional focused counsel,
subject to the user's reviewer preference. `/constitution` maintains policy.

`specs/001-source-state-recovery` D001 now retains redb/Tantivy for the first release.
Next, `/implement specs/001-source-state-recovery T001` applies once its design and
implementation scope are selected. A host without these commands can read the corresponding playbook and
use the same argument. The OMP file layout is present; actual discovery/execution
has not been exercised. No installed Spec Kit runtime or other host registration is
claimed. Keep non-command documentation out of `.omp/commands/`.

Preserve existing IDs when merging; leave a short Superseded/Deferred note with the
new owner or re-entry condition. Do not retain executable plans for parked ideas.
The predecessor crosswalk is an audit aid, not a request to implement every old goal.
