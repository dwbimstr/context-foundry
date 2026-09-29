# Optional tasks — NNN

Use only when the spec's work section is too large. Status and authorization live in
spec.md. Do not require a fixed number of tasks or create one for every process stage.

- **T001 — User-visible slice:** scope, actual dependency, FR/SC references, observable
  pass/failure, meaningful verification and any data-preserving cutover. Tests belong
  with their behavior. Record completion evidence here when executed.

Keep a coherent next step. Future research does not gate unrelated implementation.

A task is detailed enough when a fresh implementer knows the concrete input/output,
error/partial-state behavior, defaults/limits, relevant source owners, dependencies,
observable pass cases, real-versus-fixture verification and data-preserving cutover.
Replace “adequate”, “bounded” or “works” with the actual condition. Missing external
inputs need a named failure and an owner; they cannot silently count as acceptance.
