# Bootstrap, isolated workers and deployment

Status: bootstrap/connect and both MCP transports (003 T001–T003) were implemented and
verified locally on 2026-10-01 ([validation](validation.md)). Worker isolation, 005, 009
and 013 remain proposed. The token-economics tranche (001 T004–T006, 003 T005, 007
T001) was approved on 2026-10-03. 001 T004–T006 are locally implemented, accepted and
committed (`5edf32c`). 003 T005 and 007 T001 were implemented and accepted on
2026-10-04 and committed locally (`5e99ffd`, `cc402e0`). 008 explicit memory and the
003 T004 owned model gateway (`foundry gateway`, `foundry gateway-omp`) were
implemented and accepted on 2026-10-04 as well. None is released.
This is the shared deployment boundary for 003
adapter onboarding, 005 explicit producer execution, 009 embeddings and 013 owned
learning — not another product, daemon or spec stage. No production installation,
worker package or publication is claimed. [Bounded scratch feasibility](review/feasibility.md)
is recorded separately.

## User flow and ownership

1. Install the Rust core for the declared target; ordinary source retrieval works
   without ML packages, weights, virtualization or a provider credential.
2. Explicitly ask to bootstrap a named local repository. `bootstrap` inspection shows
   the canonical root, private store, admitted/excluded scope, requested components,
   existing resources and next action. Applying it creates/updates that store and
   indexes baseline source. It does not discover/enroll roots from session text.
3. Connect the chosen host using printed per-project MCP configuration and the
   [native-discovery guidance](../specs/003-agent-retrieval-context/spec.md#native-source-discovery-and-fallback).
   The configured agent uses Foundry before grep/ripgrep for eligible discovery;
   exact-pattern/current-file and unavailable-retrieval fallbacks remain explicit.
   Setup alone is not real-host adoption proof. Default stdio lets one host session
   own the store; explicit shared-owner HTTP lets independent clients use that same
   owner. Refresh through `index`, not another CLI writer. Each repository gets a
   separate store/configuration; shared mode is not a federation service. One owner
   may also admit outside repositories as references at launch for one budgeted
   multi-root response ([007](../specs/007-multi-workspace-context/spec.md), approved
   2026-10-03, not implemented); each reference keeps its own store.
4. Request semantic preparation under an explicit profile/budget when useful. Baseline
   context stays available while coverage arrives; missing runtime/weights/jail are
   named setup requirements. Compiler artifacts follow the explicit 005 import route.
5. Use budgeted context. A host adapter may additionally reserve complete-request
   capacity and report actual usage. For forwarding/metering, explicitly launch the
   optional Rust gateway and point a supported host's provider configuration at it.
   Display exactly which boundary is controlled; MCP alone does not redirect traffic.
6. Opt approved examples into learning, finish the store-owning session, prepare/train
   a bounded batch, inspect results, select a checkpoint and restart. Repeat with new
   examples; retained artifacts support rollback. No automatic fitting or promotion.

Core/store owns source truth, identity, consent records and context admission.
Adapter owns host integration and context allocation. The optional gateway owns only
forwarded-request admission, upstream credentials and observed provider receipts; it
has no source-store access. Worker supervisor owns lifecycle/access/resource bounds. Workers own only
computation over explicit inputs. A worker cannot open the live store or call back
to broaden its own permissions. External platform/library dependencies retain their
licenses; all first-party glue/trainer/worker implementation stays Rust.

## Bootstrap contract (003 owner)

Proposed CLI: `foundry bootstrap --root ROOT [--store DIR] [--apply]
[--components lexical,semantic,graph] [--profile FILE]
[--graph-index FILE --graph-snapshot FILE]`. Default is inspection only,
components lexical, store `<canonical-root>/.context-foundry`. Validate root/arguments
before writes. Inspection never initializes a store, opens its writer, loads a model,
executes repository code or claims full corpus counts; unknown estimates remain unknown.

`--apply` authorizes the requested indexing/preparation within that explicit root.
It is a thin caller of existing 001/005/009 operations with one owner, not a durable
bootstrap workflow engine. No re-request for permission already granted by the user.
It does not authorize downloads, compiler execution, training, global host edits or
publication. Those remain separately selected operations with concrete resources.

Graph bootstrap requires `--graph-index FILE --graph-snapshot FILE`; omitting both
is a setup requirement, supplying only one is invalid input. The wrapper consumes a
completed artifact; it never runs package/build/proc-macro scripts. Semantic bootstrap
requires a pinned local profile with explicit time/work limits. No implicit Hub access
or dependency installer. Source commits survive optional setup failure. Return component
states `ready`, `partial`, `needs_setup` or `failed`, reasons, pending work and exact
next commands; overall `complete` only when all requested components meet their declared
scope. Exit 1 on incomplete application; inspection can exit 0 with setup requirements.
Never call partial semantic preparation a fully prepared repository.

Interrupted application reuses the existing store and completed cache via each owner's
normal retry; it does not reinitialize or repeat committed inference. Busy is reported,
not resolved by killing another owner. Source/graph/semantic scopes stay explicit.

`foundry connect --host HOST --root ROOT --store DIR --print-config` prints configuration
for an implemented host/version, with absolute binary/root/store paths and bounded
context policy. Unknown host/version is `host_unsupported`. It does not edit global
files, expose arbitrary shell commands or start a daemon. User/agent installation uses
only session-authorized config changes and preserves unrelated prior bytes. 003 T003
must test one actual host; generic MCP is not proof of provider interception.
Implemented hosts (2026-10-01): `omp` (verified with OMP 18.4.9) and `codex` (verified
with Codex CLI 0.159.2); output is JSON with the host config text, project instruction
block, exact launch argv and capability note. Printed tool timeouts exceed the maximum
1,200,000 ms index timeout (OMP `timeout`, Codex `tool_timeout_sec`). Codex requires
per-call approval for MCP tools by default, which blocks non-interactive runs; tools
therefore declare MCP `readOnlyHint` annotations, and the printed Codex config sets
`default_tools_approval_mode = "writes"` with `approve` for `index` (it writes only
Foundry's own store for the bound root); an operator may tighten this. Applying a
config edits only a positively owned block, atomically; JSON-native host files
require manual integration (`manual_integration_required`).
Approved 2026-10-03, not implemented: 007 adds `--reference ROOT=STORE` (at most 8) to
`connect` and `mcp`, so the printed launch argv admits those references; 003 T005
replaces the instruction block with its token-economics text and appends the optional
OMP routing-hook note to the OMP capability note.
Gateway connection additionally takes `--gateway-run DIR` to print the implemented
host's provider endpoint/auth-environment settings for that owned run. Never print a
bearer/API key, transplant subscription credentials or silently change existing
provider/model settings. Refer to the gateway's exact
[protocol and accounting contract](../specs/003-agent-retrieval-context/contracts/adapter-economics.md).

## Ecosystem readiness disposition — 2026-10-01

The reported predecessor availability and call failures were accepted observations,
not remeasured. The decisions below were planned on 2026-10-01; the 001/003 owners
were then implemented and verified locally the same day ([validation](validation.md)).
No predecessor store, service or host-global configuration was migrated or changed.

| Reported concern | Existing owner and decision | Required proof |
| --- | --- | --- |
| Prakarana serves one store; Warp has a store without a live daemon; team-kit and Proxima are unindexed | 001/003: explicitly bootstrap each named repository into its own Foundry store. Existing predecessor stores are not Foundry stores and are neither adopted nor replaced implicitly. Direct stdio needs no always-on daemon | Inspect/apply/connect on admitted fixtures; baseline context survives missing optional resources |
| Collection-root teams cannot reach repository stores | 003: the worker brief supplies its actual canonical repository, absolute store configuration and available transport. A collection cwd or outside-path mention is not admission. Explicit separate-root queries are allowed; 007's joins/registry remain deferred (2026-10-03: 007 reactivated for launch-time references with one budgeted multi-root response; still no registry or cross-root joins) | Start a client from another cwd; immutable binding and foreign-handle refusal; separate-root reconciliation cannot delete another root |
| Wrong arguments and failing history queries | 001 validates types, unknown fields, bounds and error precedence; 003 exposes the same schemas. The selected baseline has five tools, not a generative `ask` loop. Do not require that predecessor tool or add it to fix adoption statistics | Invalid-argument cases before mutation; real SDK catalog/calls; actual host reports unavailable capability rather than retrying another store |
| A busy status returns an old healthy snapshot | 003 returns `busy` when its engine slot is occupied. Completed status identifies the observed store/snapshot, not current-disk or model health. Read deadlines return `deadline_exceeded`, not a partial successful read or cached health certificate | Concurrent status/index, cancellation and delayed-library cases; no successful read after expiry |
| Oversized lexical terms, rewrite/reclaim failures and neural transport churn | 001 owns bounded byte-preserving source/recovery; redb/Tantivy own physical persistence. 009 owns exact-input cache identity, visible partial coverage, disk-cap refusal and explicit offline purge. Do not port custom WAL/slot/checkpoint machinery or predecessor cache bytes | Long unbroken source text under the admitted file bound still retrieves byte-exactly; 001 restart/repair; 009 recipe/cache/index/worker acceptance before advertising semantics |
| Workers use the wrong store or an unavailable route | 003 per-project configuration and transcript-visible fallback; only the task's independently authorized roots. No user-global entry pinning one repository across every session; borrowed subagents use their parent's configured connection | Real host/version tool ordering and fallback; root/hash/range rejection cannot widen permissions |
| Release or optional-resource blockers | Release the actually accepted scope under release.md. 009 and 013 remain active goals with selected models; their incomplete recipe, rights, isolation and package checks block those claims, not a useful baseline. Gateway account/model/spend inputs block forwarding, not MCP | Matching real consumer, installed package or provider evidence for each advertised feature; no scratch-probe promotion |

The owner selected optional shared-owner MCP and the complete 001/003 T001–T003
implementation tranche on 2026-10-01. Default stdio remains; explicit per-root
Streamable HTTP MCP serves independent OMP/Codex clients through one authority.
The [owning 003 amendment](../specs/003-agent-retrieval-context/spec.md#optional-shared-owner--approved-amendment-2026-10-01)
defines loopback authentication, bounds and lifecycle. OMP 18.4.9 and Codex 0.159.2
were verified attached to one shared owner at once, with interleaved Foundry calls
([real-host record](review/real-host-t003-2026-10-01.json)). Adopting it for real
repositories (bootstrap, a started owner, project-scoped host config) is an operator
step per repository; nothing was cut over automatically.

Operation-scoped rotating owners are not the recommended alternative: repeated
library opens include locking/recovery work and conflict with 009's persistent
owner/worker lifecycle. Duplicating authoritative stores per host is also rejected.
The selected scope adds no graph/semantic/learning/gateway implementation in this
tranche; those goals remain active with their owning execution and package gates.


## Isolation is an enforced profile

A subprocess, clean environment, chroot, timeout or Rust implementation alone is not
a jail. An accepted profile lists actual filesystem/network/process/credential rights,
resource controls, IPC and platform/version, then demonstrates them with negative tests.
No supported profile means `isolation_unavailable` for that optional worker; never
silently substitute unrestricted execution. A development process launched manually
cannot be advertised as an isolated production worker.

| Worker | Granted inputs | Writes / denied access |
| --- | --- | --- |
| Policy inference | Pinned ModernBERT/tokenizer/head assets read-only, bounded admitted state/question/options via private IPC | Bounded reply; no repository traversal, home, live store, credentials, network or child execution |
| Policy training | Frozen permitted input/label/group files and exact base assets read-only | New private output/scratch; no ambient source access, live store, home or network |
| Embedding | Pinned model/tokenizer/loader assets and explicit input batch | Private scratch and bounded vectors; no ambient root discovery, store writes or downloads |
| Explicit compiler production | Immutable admitted source/config snapshot and pinned toolchain/dependencies | Private build/artifact scratch; no live workspace mutation or network; build scripts/proc macros execute only within this separately authorized profile |

The policy rows describe the clarified ModernBERT plus decision-head target, not a
verified packaged profile. Exact tokenized joint inputs, checkpoint sizes, native ML
dependencies and numerical resource limits remain 013 D001 work. The superseded
vector-only protocol and CPU limits cannot be reused as acceptance for this model.
013 D001 must specify and verify those grants/bounds before advertising the capability.
GPU access requires its own actual profile; libkrun does not establish GPU training
compatibility. The source and gateway owners still grant no credentials or live-store
access to model workers.

The optional gateway has a different, narrow trusted boundary: explicitly granted
provider credentials and HTTPS to its single configured origin, private run files,
and no repository/store or model-worker access. Its request bodies are transient
private data, not new sources or training examples. Do not mount a home directory to
find credentials. A deployed gateway cannot be advertised as a no-network model jail.

Pass only approved environment values; remove inherited credentials/proxies/agent
sockets; close unrelated descriptors. Input grants refer to verified objects, not
mutable symlink paths. Mounts/copies are read-only where specified. Supervisor validates
all returned sizes, identities and paths; malformed worker output cannot publish facts,
weights or arbitrary files. Do not mount the entire home, host root or live store.
Output/scratch directories are excluded from ingestion and public artifacts.

First native profile to verify on Linux: unprivileged filesystem restrictions using
Landlock, no-new-privileges, reviewed syscall/process restrictions and actual network
denial, with delegated cgroup v2 controls when hard per-job limits are required.
Use maintained interfaces; do not invent a security policy language. TCP-only rules
cannot prove all network access denied. See [Landlock](https://www.kernel.org/doc/html/v6.12/userspace-api/landlock.html)
and [seccomp's scope](https://docs.kernel.org/userspace-api/seccomp_filter.html).
Features missing on the executing kernel refuse the affected profile; best-effort
omission is not an equivalent sandbox. [cgroup v2](https://docs.kernel.org/admin-guide/cgroup-v2.html)
documents subtree resource controls and process-tree termination.

macOS arm64 is a primary deployment target, not presumed Linux-compatible. Verify a
signed App Sandbox worker with its own restrictive grant set and packaged Rust binary;
an unsandboxed CLI parent does not supply a sandbox by inheritance. Input/outputs use
explicit descriptors/private container files. Signing/launch/access must be tested on
the actual distributed package. [Apple's sandbox guidance](https://developer.apple.com/documentation/security/protecting-user-data-with-app-sandbox)
and [helper entitlements](https://developer.apple.com/library/archive/documentation/Miscellaneous/Reference/EntitlementKeyReference/Chapters/EnablingAppSandbox.html)
describe the relevant boundaries; they do not validate this proposed launcher.
Do not ship deprecated ad-hoc profiles or assume a raw CLI child has these protections.

The [2026-09-29 native probe](review/feasibility.md) found App Sandbox alone allowed
child creation. A macOS launcher setting soft/hard `RLIMIT_NPROC=0` before exec denied
tested spawn/fork and limit restoration while permitting threads. The selected MLX
and Rust ModernBERT/head probes still ran under that limit. This is the native profile
candidate to finish before introducing libkrun, not proof of the complete package.
Ad-hoc signatures, development Homebrew read grants and a loopback-listener denial
do not satisfy distribution, all network variants, descriptor hygiene or owner-death
cleanup. Missing required enforcement still disables the affected feature.

**009 embedding worker, implemented 2026-10-05 (009 T001; development profile only).**
Normal semantic admission returns `isolation_unavailable` until signing/notarization
and package acceptance close. `foundry semantic prepare --development-isolation` runs
the development profile, which works as follows:
- **Profile and bundle.** The semantic profile (≤64 KiB, versioned) names the model
  directory, worker bundle and executable hash, the Python home and site-packages, the
  frozen requirements, the expected document-function descriptor and
  `worker.scratch_root`. `scripts/embed-worker-bundle.sh --profile FILE` builds an
  ad-hoc-signed App Sandbox bundle. Every grant comes from that profile: read-only
  grants for its input paths, and one read-write grant for exactly the scratch root.
- **Refusals before launch.** Validation, the script and the launcher all refuse a
  scratch root that equals, contains or lies inside any read-only grant, aliases
  included. Scratch and per-run directories are created 0700. The load timeout is
  capped at 3600 s.
- **Launch.** Before exec the supervisor sets soft and hard `RLIMIT_NPROC=0`, an
  `env -i`-style offline environment, and closes descriptors beyond stdio and the
  liveness pipe.
- **Interpreter.** The worker starts CPython isolated: no site import, no user site,
  no environment, and an explicit search path of stdlib plus the profile's
  site-packages. This happens before any Python runs. It verifies the artifact
  inventory and adapter claims before loading.
- **Resources.** Process count is `hard`. Memory is `supervised`: the supervisor
  polls the physical footprint every 250 ms against the profile's ceiling (3 GiB by
  default), and a breach stops the worker with `resource_limit`.
- **Owner death.** A native watcher on the liveness pipe and a kqueue `NOTE_EXIT`
  calls `_exit` without the GIL.

  Measured on the development bundle under cold and warm load and under CPU and GPU
  pressure: `_exit` within about 1 ms of an owner SIGKILL, and the process gone within
  103 ms. One earlier unacknowledged run exceeded 2 s, cause unknown (see validation).

  A SIGSTOPped worker cannot exit itself; package acceptance needs an OS-level
  guardian.

**Progressive preparation in the MCP owner (009 T003).** An owner started with
`mcp --semantic-profile FILE --development-isolation` runs ONE resident worker for
query embeddings and document batches alike, behind one model slot:
- **Control.** `index {semantic: "prepare"}` starts, or resumes, preparation of the
  primary root and returns at once; `index {semantic: "pause"}` admits no new batch.
  With `semantic`, `index` does nothing else; combined with `root` or `scip` it is
  `invalid_argument`. Without a profile, or after a refused start, the answer is
  `semantic_unavailable` with the fallback reason. Startup never resumes preparation.
- **Foreground first.** Each store step (partition up to 8 sources, select one batch
  of at most 8 missing inputs, commit, publish) takes the engine slot only while no
  foreground operation is in flight. A request arriving during such a step gets the
  usual retryable `busy`. The model call holds no engine slot and no transaction.
- **One model call.** A query while a document batch runs gets baseline results with
  `fallback:provider_busy`. A batch is admitted only while the slot is free and no
  query is being dispatched; a refused admission pauses preparation with
  `provider_busy`, keeping committed work. A query the worker abandoned at its
  deadline holds the slot until its late reply arrives. A `prepare` while an earlier
  call still holds the slot (for example after a client timeout) is refused
  `provider_busy` before anything starts. Nothing is queued.
- **Stops.** `pause` admits no new batch and lets the in-flight batch finish and
  commit. Owner exit or EOF discards the uncommitted batch, also one waiting for the
  engine slot; the owner releases its store only after the worker is stopped and
  reaped. Each document call has 60 s
  plus the supervisor's 30 s in-flight grace; expiry pauses with `provider_timeout`. A
  failed, malformed or wrong-function reply pauses with its provider code and
  publishes nothing from that batch. Completion is `stopped` with no reason. There
  are no hidden retries, job journal, leases or watcher; `prepare` resumes.
- **Publication.** Committed coverage is republished from the cache after the first
  batch, then whenever the vectors not yet published reach the size of the last
  published generation, and at every stop except exit. Context and search use the
  coverage that has arrived and say `partial` or `ready`.
- **Status.** `status` adds a `semantic` object: the metadata-only census of
  `semantic status`, the live state (`running`, `paused`, `stopped`), `reason`, the
  last error, the last provider observation with its time, and the resident `runtime`
  (`ready` or its `fallback:` word). The census gets half the read deadline; when that
  runs out, the object names `census: deadline_exceeded` with the committed state row.
- **Limits.** The real lifecycle on a declared corpus (cold preparation, partial and
  steady queries, edit catch-up, restart) is written as an ignored measurement-phase
  test and has not been run. Each publication rebuilds the generation from the cache
  under the engine slot; its cost on a large corpus is unmeasured.

**013 learning worker, implemented 2026-10-05 (013 T002; development profile only).**
Normal training admission returns `isolation_unavailable`. `foundry learning train
--development-isolation` runs the owner-authorized development profile (spec 013,
"Development isolation, 2026-10-05"). It is never advertised as production isolation.
- **Build.** LibTorch stays out of the default build and of every default gate. The
  worker needs the `learning-worker` feature and the LibTorch 2.11.0 CPU tree:

  ```sh
  export LIBTORCH=$HOME/VSC_DEV/vendor/libtorch-2.11.0/libtorch
  cargo build --release --locked --features learning-worker --bin foundry-learn
  # Rust 1.90 floor:
  PATH=$HOME/.rustup/toolchains/1.90.0-aarch64-apple-darwin/bin:$PATH \
    cargo check --locked --features learning-worker --all-targets
  ```

  `build.rs` records `$LIBTORCH/lib` as the rpath of every target it links (the
  worker, the tests and the parity suite). Nothing depends on `DYLD_LIBRARY_PATH`,
  which SIP strips.
  The official LibTorch 2.11.0 macOS arm64 tree has a packaging defect. Its
  `libtorch_cpu.dylib` loads OpenMP from `/opt/llvm-openmp/lib/libomp.dylib`, a path
  that does not exist; the file ships in `lib/` under that install name. So the build
  also links a symbol-free stub that makes the binary load `@rpath/libomp.dylib`
  first, and `libtorch_cpu` then finds the runtime already loaded. The vendor tree is
  never modified.
- **Profile and bundle.** The learning profile (≤64 KiB, versioned, strict) reuses
  009's worker section: bundle, executable SHA-256 and `worker.scratch_root`. It adds
  `checkpoint_dir`, `libtorch_dir` (LibTorch's `lib`), `load_timeout_seconds` and the
  `ceilings` it admits. `scripts/learn-worker-bundle.sh --binary target/release/foundry-learn
  --out FoundryLearn.app --profile FILE` builds an ad-hoc-signed App Sandbox bundle:
  - read-only grants for exactly `checkpoint_dir` and `libtorch_dir`;
  - one read-write grant for `worker.scratch_root`;
  - no network.

  The script checks the binary's rpath names `libtorch_dir` and adds it if needed
  before signing. It refuses a scratch root that overlaps either read-only grant. The
  policy's `isolation_profile` names the profile file.
- **Launch.** The 009 child setup runs before exec:
  - its own process group;
  - soft = hard `RLIMIT_CPU` = `wall_seconds × cpu_threads` and `RLIMIT_FSIZE` =
    `output_bytes`;
  - `RLIMIT_NPROC=0`;
  - inherited descriptors closed except the liveness pipe;
  - an `env -i`-style environment (`PATH`, `HOME` and `TMPDIR` in the scratch run
    directory, `OMP_NUM_THREADS`).

  The worker reads only its grants and writes only its run directory. The core reads
  `head.safetensors` from there by descriptor, validates it and publishes.
- **Enforcement matrix**, checked before launch. A policy that requests a stronger level
  than this is refused with `enforcement_unavailable`, never run at a weaker one.
  Requested ceilings above the profile's `ceilings` are refused the same way.

  | Resource | Provided | Mechanism and named stop |
  | --- | --- | --- |
  | process count | hard | `RLIMIT_NPROC=0` before exec: spawn and fork fail with `EAGAIN` |
  | output | hard | `RLIMIT_FSIZE` = `output_bytes` per file (`SIGXFSZ`/`EFBIG` → `output_limit`), plus the supervised total of the scratch run directory (`output_limit`) and the candidate's total before writing |
  | CPU | hard | `RLIMIT_CPU` = `wall_seconds × cpu_threads` (`SIGXCPU` → `cpu_limit`); LibTorch intra-op threads = `cpu_threads`, inter-op 1; the wall clock (`worker_timeout`) |
  | memory | supervised | physical footprint polled every 250 ms against `memory_bytes` (`memory_limit`). It fails closed: a live worker whose footprint cannot be read is stopped (`memory_unmeasurable`) |

- **Shutdown and correlation.** Every stop is owner EOF, then TERM, 5 s, KILL. The
  child is reaped before anything is reported. Every request carries a monotonic ID
  and the loaded identity. A reply with another ID or identity, a duplicate or an
  unsolicited frame stops the worker; its output is never used. Every receive is bounded
  by the wall clock, the load timeout and cancellation, and a reply that crosses one of
  them is refused, not consumed. A successful run ends with a checked completion before
  anything is published: one last fail-closed measurement, owner EOF, the worker's own
  clean exit, reaping, and a final check that nothing followed the last reply and that
  no breach or resource limit was recorded. The candidate total, manifest included, is
  held to `output_bytes` before any member is written. Owner death ends the worker
  through 009's watcher.

For every platform declare `resource_enforcement` as `hard` or `supervised`, by resource.
OS-enforced memory/process bounds differ from supervisor RSS polling, which can overshoot.
A job requiring hard limits refuses a merely supervised profile. 013's
[contract v4](../specs/013-owned-learning/contracts/learning-loop.md) selects CPU float32
for ModernBERT/head adaptation; the earlier vector-only head is superseded. No GPU
memory quota is claimed from a host process-memory limit. 009/013 must declare their
combined resident memory and supported overlap, not only per-request allocation limits.
The first combined-inference acceptance ceiling is 8 GiB aggregate supervised resident
memory for the two model workers, separately reporting core and GPU/backend allocations;
it is a proposed ceiling, not a passing result or an OS/GPU quota. Training is offline
with online model workers stopped. Refuse jobs requiring a hard bound until the actual
package demonstrates it. No eviction/reload scheduler is implied by these limits.

## When libkrun belongs

Use a pinned stable libkrun-based CPU microVM only when a native profile cannot meet
the selected job's required isolation/resource boundary, or an actual producer needs
a Linux execution environment. This is a conditional deployment choice, not a default
dependency for retrieval or a second worker scheduler. Choose and support one accepted
profile per advertised target first; do not implement a matrix for its own sake.

The [libkrun security model](https://github.com/libkrun/libkrun#security-model) treats
guest and VMM access together. Its shared-filesystem and socket proxy mechanisms need
host-side restrictions. Absence of a guest network interface can enable TSI rather
than disable networking; prove no egress instead of inferring it from the VM config.
Pin a stable release and guest/kernel/launcher hashes; do not track an unstable API.

A Foundry VM profile uses a read-only versioned image plus bounded job input/output
disks, no broad host directory shares or host socket proxy, and an isolated VMM with
explicit CPU/RAM/disk bounds. A changed image/kernel/VMM updates that profile's identity
and reruns its affected checks, not source-index/learning quality experiments.
The release owns guest patching, license inventory, image verification and cleanup.

Virtualized GPU support does not establish MLX compatibility. The selected MLX artifact
requires its actual supported runtime/loader; never assume it runs in a Linux guest.
009 D001 must demonstrate its chosen native/VM access and execution profile separately.
An unavailable embedding profile leaves lexical retrieval working; availability of
the separate ModernBERT policy follows its own inputs/configuration/profile. Neither
failure causes a silent switch to another model or unrestricted helper.

## Lifecycle and installation

Default package: core `foundry`, locked Rust dependencies and notices. Optional learning
package adds the version-matched owned worker and accepted platform isolation assets;
ML dependencies are feature-gated. Model weights, private datasets and credentials
are separately acquired explicit inputs, never bundled by accident. Conditional VM
images have their own immutable digest/notice inventory. No auto-install at first query.
The optional gateway is another feature-gated command in the Rust crate, not a default
background service. It binds loopback under a private per-run token, keeps an upstream
key only in its process and uses narrowly allowed HTTPS egress. It does not share the
no-network/no-credential model-worker grant set. No jail helper ever receives its key.
Limit gateway inputs and egress even when optional ML workers are disabled.

Implemented 2026-10-06, unsigned; signing is the last release step and the worker
bundles are ad-hoc-signed development bundles. `tests/install.rs` runs the lifecycle
below with the debug binaries and the fake workers; the release-build install with the
real bundles is measured separately.

- **Package.** `scripts/package.sh --out DIR [--with-semantic --semantic-profile FILE]
  [--with-learning]` builds `context-foundry-<version>-macos-arm64.tar.gz` from a release
  build (or `--bin-dir DIR`). It holds `bin/foundry`, `README.md`, `LICENSE`,
  `scripts/install.sh` and `THIRD-PARTY/`: each normal dependency's license and notice
  files, an `INDEX.json`, and a supplied notice with canonical text for a crate that
  ships none. `--with-semantic` adds `libexec/foundry-embed` and
  `scripts/embed-worker-bundle.sh`; `--with-learning` adds `libexec/foundry-learn` and
  `scripts/learn-worker-bundle.sh`. `PACKAGE.json` records the version, git commit,
  target, every file's SHA-256, the versions the core reports and the dependency
  identities: the `Cargo.lock` digest, pyo3 and the semantic profile's runtime closure
  (Python, MLX, frozen-requirements SHA-256), tch, torch-sys and LibTorch 2.11.0 with
  the build's LibTorch directory. Weights, datasets, profiles and credentials are never
  packaged.
- **Layout.** Under a prefix P: `P/bin/foundry` → `../lib/context-foundry/current/bin/foundry`,
  `current` → `<version>`, one `P/lib/context-foundry/<version>/` per installed version,
  and `installed.json` recording every owned file with its SHA-256, the directories the
  installer created, the current and previous version and the disabled components;
  `pending.json` exists only while a command runs or after it was interrupted.
- **Install.** `scripts/install.sh install --package TGZ --prefix P` unpacks the
  package into a private temporary directory and verifies every file against
  `PACKAGE.json`: nothing missing, nothing unlisted, regular files only. This checks
  integrity; authenticity comes with signing. `--semantic-profile FILE
  [--semantic-extra-read DIR]...` and `--learning-profile FILE` build the ad-hoc-signed
  bundles there too, from the packaged workers with the packaged bundle scripts. They
  write installed profile copies with `worker.bundle` and `worker.executable_sha256`
  filled in; these are reached at `P/lib/context-foundry/current/profiles/`. The
  supplied profile is only read. Only then is the version placed and linked.
- **Upgrade.** `upgrade --package TGZ --prefix P [--store DIR]...` installs the new
  version beside the old one and rebuilds the bundles the old version had. It then
  switches `current` atomically. It reports whether the store schema changed and
  whether each named store needs `foundry upgrade-store`; it never runs it.
- **Rollback.** `rollback --prefix P` switches to the previous version after checking
  its files; repeating it switches back. A store already upgraded to a newer schema is
  refused by the older binary itself (`unsupported_schema`).
- **Disable.** `disable-semantic` and `disable-learning` remove only that component's
  owned files, from every installed version. Later upgrades keep it disabled until a
  profile option enables it again.
- **Uninstall.** `uninstall --prefix P [--host-config FILE]...` first runs `foundry
  connect --remove-config FILE` for each named host file. That removes exactly the
  block `--apply-config` wrote and restores the file's prior bytes: a line break apply
  had to insert is recorded in the begin marker (`separator=added`) and removed too.
  A block written before that field existed keeps the line break before it. Only
  whole marker lines count: one begin line, exactly the marker or the marker with
  that field, and one end line, exactly the end marker. Marker text anywhere else
  (inside a value, with other text on its line, duplicated) is refused without
  writing, by apply too. Uninstall then removes the owned files whose hashes still
  match; a modified one is kept and reported. It prunes only the directories the
  installer created, deepest first, with `rmdir`, so a non-empty one stays. Stores,
  caches, memories, datasets, checkpoints, scratch roots and supplied profiles are
  never touched, even inside P.
- **Refusals before any change.**
  - A path argument that is empty, ends in `/` or holds any byte outside printable
    ASCII (0x20–0x7E), a newline or a UTF-8 letter included, is exit 64. Such a path
    is never reinterpreted, and `lsof` (which escapes other bytes) prints the prefix
    verbatim for the running-owner check. A repeatable option's values stay separate
    arguments.
  - A symbolic link where the layout has a directory (P/bin, P/lib, the lock, the
    versions' directories, every directory on an owned file's path), or an owned link
    that is not exactly the installer's (`current`, `bin/foundry`), is exit 65. A
    launcher the installer did not make is never replaced. A `current` link to a
    version that neither `installed.json` nor `pending.json` records is refused,
    never adopted. The directory checks run again right before each file is hashed
    and deleted. A link swapped in after the last check (TOCTOU) is not caught.
  - One lifecycle command runs at a time under `P/lib/context-foundry/.lock` (holder
    pid and start time). A live holder is exit 75; a dead holder's lock is reclaimed.
    A lock that is not a real directory is refused before it is read or removed.
- **Running owners.** Upgrade, rollback, disable and uninstall refuse (exit 75) while
  any process of the user runs an executable under P. Processes are found by
  executable path (`lsof` text mappings), never by name. Upgrade checks again right
  before its cutover. An installed owner does not take the lifecycle lock, so one that
  starts after that last check is not seen.
- **Interruption.** Before its first change, every mutating command writes
  `P/lib/context-foundry/pending.json` atomically, under the lock. It holds the
  operation, the versions switched from and to, and the target's inventory: path →
  SHA-256 from `PACKAGE.json` and from the bundle and profile outputs. It also lists
  every directory and temporary the command creates, the intended disabled set and
  the SHA-256 of the `installed.json` it will commit. A switch then places the
  target, switches `current`, makes the launcher, commits `installed.json` and
  deletes `pending.json` last. The next command recovers from that record, under the
  lock. It refuses (exit 75, the record kept) while an installed executable runs. It
  also refuses (exit 65, nothing touched) unless `current` is absent where the
  operation allows it or names exactly a recorded endpoint.
  - If `installed.json` already has the recorded final hash, the record is
    finalized: the links must agree, a missing launcher is made, and a target file
    edited since is reported as kept, never undone.
  - Otherwise, if `current` already selects the target and every inventory file has
    its recorded hash, the switch is completed, including a missing launcher.
  - Otherwise the switch is rolled back: the links return to the old version, and
    only listed files whose content still matches, listed temporaries, and listed
    directories now empty are removed. An edited file is kept and reported.
  - An interrupted disable or uninstall is completed. Uninstall records in
    `pending.json` when its host blocks are all removed, before it deletes binaries.
    Host cleanup is skipped only when that record says so. Without an executable
    installed foundry it is refused (exit 66, naming the host file) and nothing
    changes.
  Ownership is never inferred by hashing a live directory or matching a name.

`capabilities`/bootstrap reports installed core/worker/protocol/schema versions, accepted
isolation profiles and unknown/unavailable prerequisites without loading models. A
profile is usable only after startup checks confirm its enforcement on this host.
Version mismatch disables the optional worker; it cannot corrupt or upgrade the store.
Implemented: every `foundry bootstrap` report carries `versions`: the core version,
store schema, embed, learn and predict protocol versions, and the `foundry-embed` and
`foundry-learn` executables found beside the running binary (its directory or
`../libexec/`), or null.

The foreground CLI/MCP owner supervises inference. Each worker accepts one active call,
no waiting queue; one retrieval request does not overlap its embedding and policy calls.
Loading happens at startup, never behind an unbounded first query. Offline training is
one supervised job. EOF/shutdown stops admission, cancels/finishes the current store
transaction, then terminates/reaps owned workers: TERM, 5-second grace, KILL of the
verified owned process tree/VM. Wait for disappearance before reporting stopped or
releasing a job lock. PID names alone are not ownership; no broad process-name kills.
Orphan containment must survive supervisor death via the accepted OS/service profile.
Gateway shutdown stops admission, allows an active request up to 5 seconds to finish,
then closes upstream/downstream and marks incomplete usage unknown. It has no child
model process to reap. Only after listener closure remove this run's verified token
file; preserve usage records. Never restore direct upstream routing automatically.

Installer registers no always-on daemon by default. An explicitly selected system
service/timer is later deployment configuration using the same commands, not a new
in-engine scheduler. Under the first single-owner topology, training preparation
requires the agent owner to exit; this limitation is visible in the suggested action.

Upgrade order: stop the selected owner/workers; preserve binary/config/checkpoint and
user data; verify new package; run only explicit supported store upgrade; restart and
read back version/feature identity. A changed binary is not permission to rebuild all
vectors or retrain. Rollback restores compatible prior artifacts; after an irreversible
schema upgrade it requires the pre-upgrade backup or compatible binary, never an old
writer against a new schema. Uninstall removes only owned executables/service/config
entries, preserving stores, caches, memories, datasets and checkpoints by default.
Data purge is a distinct explicit action with the existing ownership checks.

## Acceptance and release ownership

- 003: inspect→apply baseline→connect→budgeted task→edit/reindex, plus interrupted
  bootstrap, wrong root, busy owner, missing optional resources and config preservation.
  T003 verifies an ordinary task selects Foundry before eligible grep/ripgrep and
  exercises named fallbacks; record instruction-based versus hook-enforced routing.
  T004 separately verifies real forwarded streaming/tool calls, provider counts/usage,
  private gateway credentials, admission refusal, unknown outcomes and clean opt-out.
  T005 adds the OMP hook probe and the bundled-adoption host runs.
- 009: actual selected model under its advertised isolation/resource profile; cold,
  partial, warm, edited and restarted preparation without repeated completed calls.
- 013: real jailed train/save/load/predict, second round, selection/refusal/rollback;
  outside-file read/write, symlink escape, network, inherited credential/FD, child
  survival, oversize IPC, resource exhaustion and owner-death denial cases.
- Selected release: install the actual artifact in a clean supported environment,
  inspect effective grants, exercise the workflow, upgrade/rollback and uninstall.
  Unsupported platforms/features are labeled unavailable. Failed optional isolation
  blocks that worker, not the baseline package. No claim of hostile-kernel protection.

These are focused integration checks owned by existing tasks and the
[release checklist](release.md); no new all-roadmap measurement gate. Package/signing/
VM compatibility and the library choices remain unexecuted proof obligations. Explicit
publication authorization and destination are still required to upload any artifact.
