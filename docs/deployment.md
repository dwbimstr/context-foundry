# Bootstrap, isolated workers and deployment

Status: Proposed, 2026-09-29. This is the shared deployment boundary for 003 adapter
onboarding, 005 explicit producer execution, 009 embeddings and 013 owned learning.
It is not another product, daemon or spec stage. No installation, sandbox, signing,
model run or publication has been performed. Existing CLI validation is unchanged.

## User flow and ownership

1. Install the Rust core for the declared target; ordinary source retrieval works
   without ML packages, weights, virtualization or a provider credential.
2. Explicitly ask to bootstrap a named local repository. `bootstrap` inspection shows
   the canonical root, private store, admitted/excluded scope, requested components,
   existing resources and next action. Applying it creates/updates that store and
   indexes baseline source. It does not discover/enroll roots from session text.
3. Connect the chosen host using a printed per-project MCP configuration. One host
   session owns one store. An existing session uses `index` to refresh, not another
   CLI writer. A second repository gets a separate store/configuration.
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
Gateway connection additionally takes `--gateway-run DIR` to print the implemented
host's provider endpoint/auth-environment settings for that owned run. Never print a
bearer/API key, transplant subscription credentials or silently change existing
provider/model settings. Refer to the gateway's exact
[protocol and accounting contract](../specs/003-agent-retrieval-context/contracts/adapter-economics.md).

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

For every platform declare `resource_enforcement` as `hard` or `supervised`, by resource.
OS-enforced memory/process bounds differ from supervisor RSS polling, which can overshoot.
A job requiring hard limits refuses a merely supervised profile. ModernBERT's CPU/GPU
choice is unresolved; the earlier small-head CPU assumption is superseded. No GPU
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

`capabilities`/bootstrap reports installed core/worker/protocol/schema versions, accepted
isolation profiles and unknown/unavailable prerequisites without loading models. A
profile is usable only after startup checks confirm its enforcement on this host.
Version mismatch disables the optional worker; it cannot corrupt or upgrade the store.

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
  T004 separately verifies real forwarded streaming/tool calls, provider counts/usage,
  private gateway credentials, admission refusal, unknown outcomes and clean opt-out.
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
