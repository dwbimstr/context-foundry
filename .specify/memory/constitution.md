# Context Foundry constitution

Version 0.4.0 · 2026-09-29 · Working project policy. User instructions take precedence.
This is the single owner of project policy; templates are aids, not extra gates.

## Purpose

Help agents correctly work in large codebases using source, graph and explicit memory
at a bounded context cost. Support repeated fine-tuning in an owned Rust subsystem. Preserve
these outcomes; no predecessor subsystem is entitled to be rebuilt.

New Context Foundry application, trainer and supervisor code is Rust. Laya is a research
reference, not a deployed dependency. Owned model workers require an enforced isolation
profile; libkrun is conditional infrastructure, not a default core requirement.
This repository is MIT; dependency, model and dataset
rights are separate. Private corpora, transcripts, credentials and weights stay out
of Git and public artifacts.

## KISS with sophistication intact

**Keep It Simple, Stupid (KISS): choose the least complicated design that satisfies
the declared capability and its failure guarantees.** Preserve sophisticated results:
useful code relationships at scale, trustworthy context, explicit memory, complete
token accounting and repeatable owned learning. Early release scope can be smaller;
that does not silently shrink the product's goals.

Judge simplicity by how much a maintainer or operator must understand and coordinate:
independent state owners, protocols, background loops, recovery paths and configuration.
File count, line count, fewer tests or fewer features are not substitutes. A compact
module with several hidden owners can be harder than a few explicit modules.

Put necessary sophistication behind small, domain-specific operations with one clear
owner. Use established algorithms and libraries when they satisfy the contract. A
dependency does not erase its integration or recovery costs. Add an abstraction for
a real responsibility or consumer, not a hypothetical future backend.

A simplification must preserve the selected outcome and safety acceptance. If it
cannot, name the temporary capability limitation and retained goal. Never obtain a
"simple" pass by weakening graph evidence, source freshness, durability, learning
validity or accounting honesty. Explain added complexity in the existing design;
this principle creates no new document, scoring system or approval stage.

## Design rules

1. Use an ordinary maintained library for durable transactions. Derived indexes are
   replaceable. Do not build a WAL, allocator or checkpoint format. One owner writes
   each fact. The prototype's storage choices can be revised.
2. Start with a usable user workflow in one crate. Before adding a protocol, worker,
   durable table, service or normative document, name the present failure it solves,
   the simpler alternative and what maintenance it adds. Write this in the existing
   spec/design, not a new checklist. Remove superseded mechanisms.
3. Evidence names scope, source identity, coordinates, provenance and freshness.
   Query limits cannot delete semantic truth. Incomplete scans cannot prove absence.
   Corruption/unknown schema fails visibly; derived repair preserves user data.
4. Bound external inputs, output, traversal and asynchronous work. Budget the bytes
   actually delivered under a declared tokenizer/boundary. Provider usage is unknown
   until observed; less source text alone does not prove lower cost or better tasks.
5. Source indexing never silently executes workspace code, trains or downloads models.
   Training consent is separate from retrieval. Models cannot create authoritative
   source facts, label their own correctness or select themselves for production.
6. Optional learning/semantic features cannot block a working baseline release.
   A failed safety contract blocks its affected feature; a failed optimization can
   remain disabled. Named limitations are preferable to invented guarantees.
7. Bootstrap is explicit admission of a named root, not inference from session text.
   Adapters can enforce only the request/delivery boundaries they actually control.
   Hard budgets are deterministic; learned choices cannot grant rights or raise limits.
   Workers receive minimal inputs, not live store/home credentials. Isolation and
   resource enforcement are demonstrated on the distributed target, never inferred
   from process separation, a library name or virtual-machine use.

## Working procedure

Agree on the user outcome, implement a small slice, check it, review and release the
scope that works. Familiar Spec Kit commands remain available, but are not mandatory
stages. No `/measure` or `/council` ceremony is required to close every feature.

A numbered `spec.md` owns behavior, status, design, work and acceptance. Split out a
plan/task file or external contract only when it has a useful reader. Keep repairs
within the existing contract. Deferred ideas get a re-entry condition, not a fully
specified subsystem. Plan detail and link counts do not establish design quality.

Proposed is not implemented or independently approved. Record accepted user scope;
when implementation is already authorized, proceed without asking again. This
constitution does not retroactively approve the prototype. Preserve unrelated work;
one writer per tree. Do not operate external stores/services without task authority.

## Evidence and shipping

- A code change needs focused behavior tests and applicable Rust checks. Durable or
  concurrent changes need the relevant restart/fault checks. Documentation edits
  need consistency/link checks, not Rust builds or live-store probes.
- A claimed integration needs its real consumer; a claimed numerical improvement
  needs a comparison at that actual boundary. Fixtures are labeled as fixtures.
- A release uses [the short release checklist](../../docs/release.md) for the features
  it advertises. Future roadmap items cannot become retroactive release conditions.

Before an experiment over ten minutes, state the decision it can change, inputs,
budget and stop rule. Reuse valid evidence; repeat only what changed inputs invalidate.
There is no mandatory benchmark per spec or phase. Correctness and complete usage
matter for economic claims, including provider caching when known.

Review proportionally. Seek independent review for costly durable/protocol/privacy
commitments when available and authorized; label self-review honestly. Do not install
orchestration or spawn reviewers against user preferences. Further rounds need a
specific unresolved question, not another blanket audit. Publication needs explicit
intent; local preparation does not imply upload.

## Amendments

0.1.0 introduced the fifteen-bundle workflow. 0.2.0 removes mandatory stage traversal,
three documents per capability and universal close-time measurement. Five user
workflows replace the subsystem portfolio; substantive correctness safeguards remain.
0.3.0 makes the owner's KISS requirement explicit: reduce coordination and maintenance
complexity while preserving capability and failure guarantees; fewer features alone
do not demonstrate a simpler architecture.
0.4.0 follows the owner's 2026-09-29 correction: owned Rust learning replaces external
Laya integration; explicit bootstrap, adapter economics and tested worker deployment
are part of the ecosystem. The owner also explicitly selected both context budgets
and optional model-request forwarding/metering, with all first-party code in Rust.
The narrow gateway controls only traffic configured through it; it cannot become
a hidden source owner or a prerequisite for basic retrieval. Earlier Laya contracts
remain historical, not requirements.
