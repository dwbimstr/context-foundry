# Release when every active spec is complete

A release declares its version, included behavior and supported environments. It ships
only when every task of every active spec in the [portfolio](../specs/README.md) is
implemented and accepted (owner policy 2026-10-04); deferred and superseded specs are
excluded. This checklist replaces spec 015; it is not another product.
[003 T006](../specs/003-agent-retrieval-context/spec.md#t006--validate-retrieval-repairs-in-real-coding-tasks)
remains a release condition that bounded pilot v1 cannot accept. Owner decision,
2026-11-06, "Draft pilot v2, unfunded": a pilot v2 contract is to be drafted, and nothing
is spent until the owner separately authorizes a key, rights and a costed budget.

1. Record the selected scope, actual acceptance evidence and known limitations. Verify
   advertised integrations with their real consumer; do not make untested claims.
2. Run the applicable full Rust checks. Build the artifact with `scripts/package.sh` and
   run the install smoke with `scripts/install.sh` on declared platforms: install, run the
   installed `foundry`, upgrade, rollback, disable and uninstall
   ([lifecycle](deployment.md#lifecycle-and-installation)). Include relevant
   restart/data-preservation checks. Reuse valid feature evidence; no blanket
   predecessor ladder or close-time benchmark is inherited.
3. Document compatibility, preserved user data, recovery/rollback and install/remove
   ownership. Until upgrade support exists, say so; do not silently delete feedback
   or memories to rebuild a store. A source-only preview can use fresh-store installs.
   The first release's statement is
   [drafted below](#compatibility-and-data-preservation-for-the-first-release).
4. Inspect the actual artifact/Git contents, dependency licenses, notices and public
   fixtures. No private datasets/transcripts/weights/credentials or incompatible copies.
5. Prepare release notes and the concrete distribution artifact. Publish only to the
   owner-selected destination with authorization. Local preparation is not publication.

For advertised bootstrap, adapter budgeting or owned workers, use the
[deployment contract](deployment.md): install the actual package, verify target-specific
isolation and resource controls, demonstrate bootstrap/connect/budget behavior, then
upgrade/rollback/uninstall without user-data loss. Core-only distributions need no ML
runtime or VM. Failed optional isolation blocks that feature and therefore the release.
The learning package must use the owned Rust worker; the legacy Laya client is not
evidence for it. A raw subprocess is not a successful jail test. While 013 is frozen
off (owner, 2026-10-07), the release package is built without `--with-learning` and
advertises no learned routing, so its learning-worker checks are not release conditions.
Owner decision, 2026-11-06, "Defer 010 real-model acceptance": the release does not
wait on 010's real-model task utility, and 010's implemented code stays experimental
and unadvertised.

Owner decision, 2026-11-06, "Release it separately": as
[spec 003](../specs/003-agent-retrieval-context/spec.md#t004--forward-and-meter-an-actual-supported-model-workflow)
says, the meter-only 003 T004 gateway is released separately from CLI and MCP, and the
CLI/MCP release does not advertise it. The gateway's own release additionally proves
its actual host/API streaming/tool round, private credential boundary, input/output
admission, unknown-usage handling and exact host-config restoration on opt-out.
Missing integration proof blocks the gateway's release, not the CLI/MCP release; the
earlier wording here, under which it blocked the whole release, is superseded.
Gateway credentials never enter packaged data or worker grants.

The current repository has no release. A local `v0.1.0` tag and artifact made on
2026-10-04 under the earlier release-the-working-scope rule were withdrawn the same day
when the owner adopted this policy; their checklist record remains in
[validation](validation.md) as history. The owner selected the destination on
2026-10-08: a GitHub release on `dwbimstr/context-foundry` carrying the macOS arm64
package, the source archive and `SHA256SUMS`, after Developer ID signing; the tag and
publication still need the owner's go-ahead.

G2, G1 with the profile, and vector parity selected EmbeddingGemma 2 on 2026-10-08
([validation](validation.md)). Owner decision, 2026-11-06, "Row 4 blocks release":
the first release ships signed semantic retrieval. Build the release candidate with
`--with-semantic --semantic-profile` and the frozen EmbeddingGemma 2 profile, sign it,
and run the installed-artifact checks (row 4 of the
[measurement handoff](measurement-handoff.md)) on that signed package. A failing check
is fixed before any release; a lexical-only package does not ship in its place. The
2026-10-08 wording here, which built the package without `--with-semantic` until row 4
passed, is superseded because it allowed a lexical-only release.

Owner decision, 2026-11-06, "Once now, unsigned": `scripts/native-macos-acceptance.py`
runs once on the owner's Mac with ad-hoc signing (passed
[2026-11-10](validation.md#native-macos-acceptance--2026-11-10)) and is repeated on
the signed release candidate. Owner decision, 2026-11-06, "Script checks only": device acceptance
is what that script checks, and the release notes name the unbuilt global GPU
pressure, combined embedding and generation residency, thermal/battery and
embedding-eviction policies as limitations. They also state the other limitations the
owner named that day in the [decision record](validation.md#owner-decisions--2026-11-06).

## Compatibility and data preservation for the first release

**Draft, 2026-11-19; not final.** This is the step 3 statement for the first release.
It was written from the code at `85a63c3`; nothing was built, installed or signed for
it. Current release status is in the [ecosystem handoff](resume-context-ecosystem.md).
The statement becomes final on the signed release candidate, after these three things:

1. Read the numbers again at the candidate. Its `PACKAGE.json` (`versions`) and the
   `versions` object of `foundry bootstrap --root ROOT` give the store schema. The
   other formats are the constants the table names, read at the commit that
   `PACKAGE.json` records as `git_commit`.
2. Run the step 2 lifecycle on the signed package. The lifecycle evidence so far is
   from unsigned packages ([2026-10-06](validation.md#installable-package--2026-10-06)).
3. Replace `<version>`. **The version number is an open question for the owner.**
   `Cargo.toml` says `0.1.0`, which is also the number of the withdrawn local tag, and
   no record says whether the first release reuses it. `scripts/package.sh` names the
   package from that number and records it in `PACKAGE.json`, `scripts/install.sh`
   names the version directory from that record, and `scripts/install.sh upgrade`
   refuses a package whose version is already installed (exit 73).

When it is final, it is recorded with the release's checklist record in
[validation](validation.md), as item 3 of the
[withdrawn `v0.1.0` record](validation.md#v010-local-release--2026-10-04-withdrawn) was.

### What is compatible

- **Platform.** The package is `context-foundry-<version>-macos-arm64.tar.gz`, for macOS
  on Apple silicon (`aarch64-apple-darwin`). `scripts/package.sh` refuses to build it on
  any other host (exit 69), and no package is built for another platform.
- **Hosts.** `foundry connect` prints configuration for two hosts, `omp` and `codex`,
  and refuses any other name with `host_unsupported`. The host versions each was
  verified with are in the
  [bootstrap contract](deployment.md#bootstrap-contract-003-owner). Step 1 checks the
  release candidate against the real hosts again.
- **Stores.** A store is one directory: the database `knowledge.redb`, the search index
  `search/` and, once semantic retrieval was prepared, the generations under
  `semantic/`. A store made by this release has the formats in the table. A store from
  an earlier development build may hold other values; the last column says what each
  does and which explicit command, if any, brings it current. Nothing upgrades or
  repairs a store on its own: opening a store, the MCP owner, `bootstrap` and the
  installer never do.

| Format | Value | Constant | A store that holds another value |
| --- | --- | --- | --- |
| Store schema | 12 | `SCHEMA_VERSION` in [`src/store.rs`](../src/store.rs) | Schemas 1–11: a command that opens the store refuses with `upgrade_required` (exit 2) until `foundry --store STORE upgrade-store --to 12` has run. Any other value: `unsupported_schema` (exit 2), and nothing upgrades it. |
| Search index format | 9 | `SEARCH_SCHEMA`, same file | `status` reports `index_state` as `repair_required` and `search` refuses with `repair_required` until `foundry --store STORE repair-index` has run. `retrieve` still reads stored source by handle. |
| Source-unit grammar | 7 | `SOURCE_UNIT_SCHEMA`, same file | Semantic partitions fitted under another grammar are kept as history and are not current. `foundry semantic prepare` refits them and reuses only cached inputs that match exactly ([2026-11-11](validation.md#parser-bundle-and-complete-unit-comparison--2026-11-11)). |
| Semantic generation format | 6 | `GENERATION_VERSION` in [`src/neural/index.rs`](../src/neural/index.rs) | The generation is refused by name and retrieval stays lexical. `semantic prepare` or `repair-index` republishes it from the vector cache without sending a document to the model ([2026-10-22](validation.md#interned-label-map-generation-v6--2026-10-22)). |
| Semantic profile and its descriptor | 2 and 2 | `PROFILE_VERSION` in [`src/neural/profile.rs`](../src/neural/profile.rs), `DESCRIPTOR_VERSION` in [`src/neural/provider.rs`](../src/neural/provider.rs) | A descriptor v1 profile, written for the earlier MLX worker, is refused with `profile_unsupported`. |

The release package is built without `--with-learning`, and the gateway is released
separately (both above), so the learning worker and the gateway are outside this
statement.

### What user data is preserved, and how

A store holds data that Foundry cannot derive again from the workspace alone: explicit
memories, feedback rows, task state, source briefs and imported compiler facts. It also
holds a copy of each indexed source and, once semantic retrieval was prepared, a vector
cache that costs model time to rebuild. Outside the store, the operator supplies
profiles, model files, datasets, checkpoints and host configuration files.

- **The installer does not manage stores.** `scripts/install.sh` runs three `foundry`
  commands and no others: `bootstrap --root` on the prefix, to read back versions;
  `status` on each store named with `upgrade --store DIR`, to say whether it needs
  `upgrade-store`; and `connect --remove-config FILE` for each host file named at
  uninstall. It never runs `upgrade-store` or `repair-index`. `tests/install.rs` checks
  that a memory record reads the same after upgrade and after rollback, that search
  results are the same after upgrade, and that uninstall leaves the store's bytes
  unchanged.
- **Upgrading a store adds tables and removes nothing.** `upgrade-store --to 12` is one
  transaction and needs the store to itself; while another owner holds the store it
  refuses with `store_busy` (exit 3). It creates the tables that later schemas added and
  writes the schema number last, so an interrupted upgrade leaves the store wholly at
  its old schema or wholly at 12. It deletes no source, memory, feedback, graph,
  semantic or learning row, scans no source and calls no model. Apart from the store's
  own metadata, the only existing rows it rewrites are the pending-index keys of a
  store older than schema 3, which get a type prefix.
- **Repairing an index rebuilds only derived data.** `repair-index` rebuilds the search
  index from the stored sources, memories and briefs, and republishes the semantic
  generation from the vector cache. It moves the old index aside inside the store
  directory as `search.quarantine-<id>` and does not delete it. It changes no source,
  memory or feedback row.
- **Nothing rebuilds a store by deleting it.** No install, upgrade, repair or uninstall
  step removes feedback or memories. A memory record is removed only by an explicit
  forget: `memory forget` of that record, or a `task-context` forget of the task that
  promoted it, which refuses if the record was edited since. No command removes a
  feedback row; a correction replaces the row for the same example. Data that mirrors
  or is derived from the workspace does change without such a command: indexing drops
  the stored copy of a source that is gone from the workspace, and the knowledge cache
  evicts entries at its size bound. `semantic purge` removes the semantic partitions,
  vector cache and generations, and `knowledge purge-cache` empties the knowledge cache.
- **Readable copies.** `foundry memory export --workspace-id ID` prints live memory
  records as JSONL, one page at a time. `foundry export-training` prints one JSONL
  training example for each legacy feedback row that allows training. It prints no
  row that withholds that consent and no `feedback v4` row, so it is not a complete
  copy of feedback. These are copies to keep. No command imports them in bulk.

### Recovery and rollback

- **Package.** `scripts/install.sh rollback --prefix P` switches to the previous
  installed version after checking its files, and running it again switches back. The
  next lifecycle command completes or rolls back an interrupted one from its
  `pending.json` record ([lifecycle](deployment.md#lifecycle-and-installation)).
- **Store schema.** There is no downgrade. `upgrade-store` accepts only `--to 12`, and
  a binary refuses a store whose schema it does not know (`unsupported_schema`). Going
  back after a store upgrade means restoring a copy of the whole store directory that
  was made before the upgrade, while no owner had the store open, and using it with the
  earlier binary. Foundry has no backup command and the upgrade makes no copy; making
  and keeping one is the operator's step.
- **Search index.** `repair-index` is the recovery for a damaged or outdated index. An
  interrupted repair leaves a marker in the store, `status` keeps reporting
  `repair_required`, and running `repair-index` again continues it.
- **Indexing and preparation.** `foundry refresh` resumes the index work that an
  interrupted `index` left pending. `semantic prepare` resumes from the vectors already
  committed to the cache.

### Who owns install and remove

- **The installer** owns what it placed under the prefix the operator names: the
  version directories, including the worker bundles and profile copies it built there,
  the `current` link, the `bin/foundry` link and its `installed.json` record. It removes
  only owned files whose hashes still match and only directories it created, once they
  are empty; a modified file is kept and reported. It registers no daemon or service,
  and it refuses to replace a `bin/foundry` it did not make (exit 65).
- **Host configuration** belongs to the host and its operator. Foundry owns only the
  marked block that `foundry connect --apply-config FILE` wrote. Either
  `foundry connect --remove-config FILE` or `scripts/install.sh uninstall` with
  `--host-config FILE` removes that block and restores the file's earlier bytes.
- **The operator** owns the prefix itself, every store, cache, memory, dataset,
  checkpoint, scratch root and supplied profile, and any binary placed another way, for
  example by `cargo install`. Uninstall leaves all of them, including a store that lies
  inside the prefix.

### Where upgrade support does not exist

- **From an earlier release.** There is no earlier release. The local `v0.1.0` tag and
  artifact were withdrawn the day they were made and were never published, so no
  released package and no store written by a released binary exists to upgrade from.
  The first release is installed with `install`, which refuses a prefix that already
  holds an installation (exit 73).
- **Between two releases.** The installer's `upgrade` and `rollback` are implemented
  and tested, but only from a package to a relabelled package of the same version
  ([2026-10-06](validation.md#installable-package--2026-10-06)). `scripts/package.sh`
  allows that label, `<version>-test.<suffix>`, only for binaries built beforehand and
  passed with `--bin-dir`. No upgrade between two different releases has been run.
- **Stores from development builds.** The command covers these stores; what is missing
  is evidence on stores that real earlier binaries wrote. `upgrade-store --to 12`
  accepts schemas 1–11. The tests run it on stores that the tests construct at earlier
  schemas. One dated record upgrades a store that an earlier development binary wrote,
  from schema 7 to 8, and shows that binary then refusing the upgraded store
  ([2026-10-09](validation.md#source-bound-brief-import-and-conceptual-discovery--2026-10-09)).
  No store written by a released binary has been upgraded, because none exists.
- **Downgrade.** No command lowers a store's schema. Rollback of a store is the restored
  copy described above.
- **Automatic upgrade or repair.** There is none, by design: every store change in this
  statement is an explicit command.
- **A moved repository.** A store records the canonical path of its workspace root and
  refuses a root at another path with `wrong_workspace`. No command rebinds a store to
  a repository that moved.
- **Predecessor stores.** Stores of the predecessor services are not Foundry stores, and
  nothing imports or converts them
  ([disposition](deployment.md#ecosystem-readiness-disposition--2026-10-01)). `index`
  refuses a nonempty directory that is not a Foundry store with `unrecognized_store`.
- **Descriptor v1 semantic profiles.** The installer does not carry one forward on
  upgrade: it disables semantic retrieval by name and asks for a v2 profile. The
  vectors made under it stay in the store until `semantic purge`. No build that used a
  v1 profile was distributed ([measurement handoff](measurement-handoff.md#measurements),
  row 8).
