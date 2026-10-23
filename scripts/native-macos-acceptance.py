#!/usr/bin/env python3
"""Explicit native acceptance; writes evidence before failing or reporting blocked.

Run from a checkout with Rust 1.90.0, Python 3, CMake and Xcode/Metal tools:
  python3 scripts/native-macos-acceptance.py --out /absolute/new-results \
      --work /absolute/new-work [--generation] [--scale-calibration]
Upload only --out, including on failure. Work contains public pinned weights and
build outputs. No credentials, private corpus or model-quality claims are needed.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import re
import resource
import ctypes
import shutil
import signal
import subprocess
import sys
import tarfile
import time


LLAMA = "b9acf138a1e28ce1fc23b5a4fc4b12444b50f7ea"
GGUF_REV = "bfcd298762cc34d0357ece5ebdd31791a3a374d8"
TOKENIZER_REV = "914f7f89142e33e77833254d9c9b90c3cef7303b"
GGUF_SHA = "68bae29d62fb8c7d23e98d21fd4662753ddd636e6b62b8a70dfe059a9844f216"
TOKENIZER_SHA = "4d777ef5bdc1aa36227abdfb77c3e49e7b9c892d16e1b6bda41c393504828be4"
GENERATION_REV = "ebb2015119c907b064c512bf053e945850b5875f"
GENERATION_FILE = "qwen2.5-coder-0.5b-instruct-q4_k_m.gguf"
SCALE_SAMPLE_SHA = "76b510031972d619b01c0f767b5c0d66c6da4899ff8628c62ff9943a7b1b7a26"
SCALE_PROJECTS = [
    ("rust", "1159e78c4747b02ef996e55082b704c09b970588",
     "https://codeload.github.com/rust-lang/rust/tar.gz/1159e78c4747b02ef996e55082b704c09b970588",
     "8dc476ed3ccd405304236e3ee737b365b49168f1cfebd4da537ee0b4380e96bf"),
    ("go", "6e676ab2b809d46623acb5988248d95d1eb7939c",
     "https://codeload.github.com/golang/go/tar.gz/refs/tags/go1.25.0",
     "7e5813e23f221dc193dec557379fe22607baa6fc2e321a1b8e39b658848a5f5f"),
]
GENERATION_SHA = "1d9614638d18024d0fbb36575a15f1302a3adf044df10345688ec4f6e1c4ff32"
FIXTURES = [
    "owner_death_on_liveness_pipe_eof_ends_the_worker",
    "owner_death_during_load_ends_the_worker",
    "owner_death_during_long_call_ends_the_worker",
    "owner_death_while_ipc_blocked_ends_the_worker",
    "memory_ceiling_breach_during_a_call_stops_with_resource_limit",
    "term_ignoring_worker_is_reaped_after_the_5s_kill",
]
METAL_PROBE = r'''
import Foundation
import Metal
import Darwin
let output = CommandLine.arguments[1]
func finish(_ fields: [String: Any], _ code: Int32) -> Never {
    let data = try! JSONSerialization.data(withJSONObject: fields, options: [.prettyPrinted, .sortedKeys])
    try! data.write(to: URL(fileURLWithPath: output))
    print(String(data: data, encoding: .utf8)!)
    exit(code)
}
guard let device = MTLCreateSystemDefaultDevice() else {
    finish(["status": "blocked", "reason": "no_native_metal_device"], 2)
}
do {
    let source = """
    #include <metal_stdlib>
    using namespace metal;
    kernel void increment(device uint *value [[buffer(0)]]) { value[0] += 1; }
    """
    let library = try device.makeLibrary(source: source, options: nil)
    guard let function = library.makeFunction(name: "increment"),
          let queue = device.makeCommandQueue(),
          let buffer = device.makeBuffer(length: 4, options: .storageModeShared),
          let command = queue.makeCommandBuffer(),
          let encoder = command.makeComputeCommandEncoder() else {
        finish(["status": "blocked", "reason": "metal_compute_objects_unavailable", "device": device.name], 2)
    }
    let pipeline = try device.makeComputePipelineState(function: function)
    buffer.contents().storeBytes(of: UInt32(41), as: UInt32.self)
    encoder.setComputePipelineState(pipeline)
    encoder.setBuffer(buffer, offset: 0, index: 0)
    encoder.dispatchThreads(MTLSize(width: 1, height: 1, depth: 1), threadsPerThreadgroup: MTLSize(width: 1, height: 1, depth: 1))
    encoder.endEncoding()
    command.commit()
    command.waitUntilCompleted()
    let value = buffer.contents().load(as: UInt32.self)
    guard command.status == .completed && value == 42 else {
        finish(["status": "failed", "reason": "metal_dispatch_failed", "device": device.name, "value": value], 1)
    }
    finish(["status": "passed", "device": device.name, "registry_id": String(device.registryID), "unified_memory": device.hasUnifiedMemory, "kernel_result": value], 0)
} catch {
    finish(["status": "failed", "reason": "metal_probe_error", "error": String(describing: error), "device": device.name], 1)
}
'''


def sha256(path):
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def scale_sample(archives, corpus, output):
    """128 fixed public code files, stratified by project and byte-size quartile.

    This is a diagnostic sample, not a population-weighted corpus estimate.
    No model, expected answer or relevance label participates in selection.
    """
    rows = []
    for project, revision, url, archive_sha in SCALE_PROJECTS:
        archive = archives / (project + ".tar.gz")
        if sha256(archive) != archive_sha:
            raise RuntimeError("scale archive checksum mismatch: " + project)
        candidates = []
        with tarfile.open(archive, "r:gz") as source:
            for member in source:
                if not member.isfile() or not 64 <= member.size <= 2 * 1024**2:
                    continue
                parts = Path(member.name).parts
                if len(parts) < 2 or member.name.startswith("/") or ".." in parts:
                    raise RuntimeError("invalid archive member")
                relative = Path(project, *parts[1:]).as_posix()
                if any(part.startswith(".") or part in ("target", "node_modules") for part in parts[1:]):
                    continue
                if not relative.endswith(".rs" if project == "rust" else ".go"):
                    continue
                data = source.extractfile(member).read()
                try:
                    body = data.decode("utf-8")
                except UnicodeDecodeError:
                    continue
                if ("\0" in body or not body.strip()
                        or ("-----BEGIN " + "PRIVATE KEY-----") in body
                        or ("-----BEGIN " + "RSA PRIVATE KEY-----") in body):
                    continue
                candidates.append({"path": relative, "bytes": len(data),
                                   "sha256": hashlib.sha256(data).hexdigest()})
        candidates.sort(key=lambda row: (row["bytes"], row["path"]))
        for quartile in range(4):
            group = candidates[quartile * len(candidates) // 4:(quartile + 1) * len(candidates) // 4]
            group.sort(key=lambda row: hashlib.sha256(("native-scale-v1\0" + row["path"]).encode()).hexdigest())
            if len(group) < 16:
                raise RuntimeError("scale sample stratum incomplete")
            rows.extend(row | {"size_quartile": quartile + 1} for row in group[:16])
    rows.sort(key=lambda row: row["path"])
    encoded = (json.dumps(rows, separators=(",", ":"), ensure_ascii=True) + "\n").encode()
    if len(rows) != 128 or hashlib.sha256(encoded).hexdigest() != SCALE_SAMPLE_SHA:
        raise RuntimeError("fixed scale sample identity mismatch")
    selected = {row["path"]: row for row in rows}
    corpus.mkdir()
    for project, _, _, _ in SCALE_PROJECTS:
        with tarfile.open(archives / (project + ".tar.gz"), "r:gz") as source:
            for member in source:
                if not member.isfile():
                    continue
                relative = Path(project, *Path(member.name).parts[1:]).as_posix()
                if relative not in selected:
                    continue
                data = source.extractfile(member).read()
                row = selected[relative]
                if len(data) != row["bytes"] or hashlib.sha256(data).hexdigest() != row["sha256"]:
                    raise RuntimeError("selected source identity changed")
                destination = corpus / relative
                destination.parent.mkdir(parents=True, exist_ok=True)
                with destination.open("xb") as target:
                    target.write(data)
    manifest = {
        "v": 1, "files": rows, "sample_files_sha256": SCALE_SAMPLE_SHA,
        "selection": "16 lowest SHA256(native-scale-v1\\0 + relative path) per byte-size quartile per project; 64 Rust .rs and 64 Go .go regular UTF-8 nonblank files, 64 bytes through 2 MiB; ties sort by path; skip hidden/target/node_modules paths and normal private-key-marker exclusions",
        "sample_sha256_encoding": "sorted files array, compact ensure_ascii JSON plus LF",
        "projects": [{"project": project, "commit": revision, "url": url, "archive_sha256": checksum}
                     for project, revision, url, checksum in SCALE_PROJECTS],
        "scope": "source-size-stratified .rs/.go archive sample; upstream ignore controls are not extracted; membership in the full 67663 admitted-source population is unverified; not a population-weighted estimate",
    }
    with output.open("x") as target:
        json.dump(manifest, target, indent=2)
        target.write("\n")


class NativeFullCommandError(RuntimeError):
    def __init__(self, message, receipt):
        super().__init__(message)
        self.receipt = receipt


class NativeFullCommand:
    """Run sequential phases. Caller validates report semantics and partial results.

    run(kind, command, timeout, require_worker=None, forbid_worker=False,
        expect_json=True, allowed_exit_codes=(0,)) returns JSON or raw text.
    Receipts are also available through .last_receipt and .receipts.
    """
    DISK_RESERVE = 512 * 1024**2
    OUTPUT_LIMIT = 16 * 1024**2
    INTERRUPT_GRACE = 60

    def __init__(self, out, scratch):
        self.out, self.scratch = Path(out), Path(scratch)
        if not self.out.is_absolute() or not self.scratch.is_absolute():
            raise ValueError("out and scratch must be absolute paths")
        self.out.mkdir(parents=True, exist_ok=True)
        if not self.scratch.is_dir():
            raise ValueError("scratch must already exist")
        self.receipts, self.last_receipt = [], None
        self.counter = 0

    class _Usage(ctypes.Structure):
        # Darwin rusage_info_v0, matching libc's public RUSAGE_INFO_V0 ABI.
        _fields_ = [("uuid", ctypes.c_uint8 * 16)] + [
            (field, ctypes.c_uint64) for field in (
                "user_time", "system_time", "pkg_idle_wkups", "interrupt_wkups",
                "pageins", "wired_size", "resident_size", "phys_footprint",
                "proc_start_abstime", "proc_exit_abstime")]

    _native_library = None

    @classmethod
    def _native_memory(cls, pid):
        if cls._native_library is None:
            cls._native_library = ctypes.CDLL("/usr/lib/libproc.dylib", use_errno=True)
            cls._native_library.proc_pid_rusage.argtypes = [ctypes.c_int, ctypes.c_int, ctypes.c_void_p]
            cls._native_library.proc_pid_rusage.restype = ctypes.c_int
        if ctypes.sizeof(cls._Usage) != 96:
            raise RuntimeError("unexpected Darwin rusage_info_v0 size")
        usage = cls._Usage()
        if cls._native_library.proc_pid_rusage(pid, 0, ctypes.byref(usage)) != 0:
            raise OSError(ctypes.get_errno(), "proc_pid_rusage")
        if usage.resident_size == 0 or usage.phys_footprint == 0:
            raise OSError("native process memory observation is empty")
        return {"rss_bytes": int(usage.resident_size), "physical_footprint_bytes": int(usage.phys_footprint)}

    def _sample_resources(self, root, descendants, resources, elapsed):
        free = {"output": shutil.disk_usage(self.out).free, "scratch": shutil.disk_usage(self.scratch).free}
        for key, value in free.items():
            resources["minimum_disk_free_bytes"][key] = min(resources["minimum_disk_free_bytes"].get(key, value), value)
        resources["last_disk_free_bytes"] = free
        resources["polls"] += 1
        targets = [("driver", os.getpid())]
        if root is not None:
            targets.append(("cli", root["pid"]))
        targets.extend(("worker", state["pid"]) for state in descendants if self._worker(state))
        observed = []
        for role, pid in targets:
            totals = resources[role]
            try:
                memory = self._native_memory(pid)
                totals["samples"] += 1
                totals["sampled_peak_rss_bytes"] = max(totals["sampled_peak_rss_bytes"], memory["rss_bytes"])
                totals["sampled_peak_physical_footprint_bytes"] = max(totals["sampled_peak_physical_footprint_bytes"], memory["physical_footprint_bytes"])
                observed.append({"role": role, "pid": pid, **memory})
            except OSError as error:
                # A process may disappear between the owned identity snapshot and
                # sampling. Count all misses; never replace unknown with zero.
                totals["missed_samples"] += 1
                if len(resources["sample_errors"]) < 8:
                    resources["sample_errors"].append({"role": role, "pid": pid, "error": str(error)})
        return {"elapsed_seconds": round(elapsed, 3), "disk_free_bytes": free, "processes": observed}

    @staticmethod
    def _parse_ps(text):
        states = {}
        for line in text.splitlines():
            fields = line.split(None, 7)
            if len(fields) != 8:
                raise RuntimeError("cannot parse Darwin process identity: " + line)
            pid, parent = int(fields[0]), int(fields[1])
            if pid in states:
                raise RuntimeError("duplicate PID in process snapshot")
            states[pid] = {"pid": pid, "parent": parent,
                           "started": " ".join(fields[2:7]), "command": fields[7]}
        return states

    @classmethod
    def _snapshot(cls, pid=None):
        command = ["/bin/ps"] + (["-p", str(pid)] if pid is not None else ["-ax"])
        command += ["-o", "pid=,ppid=,lstart=,comm="]
        env = dict(os.environ, LC_ALL="C")
        result = subprocess.run(command, capture_output=True, text=True,
                                timeout=5, env=env)
        # Darwin ps returns 1 for a requested PID that no longer exists.
        if pid is not None and result.returncode == 1 and not result.stdout.strip():
            return {}
        if result.returncode != 0:
            raise RuntimeError("process observation failed: " + result.stderr.strip())
        return cls._parse_ps(result.stdout)

    @staticmethod
    def _matches(saved, live):
        return live is not None and saved["pid"] == live["pid"] and saved["started"] == live["started"]

    @staticmethod
    def _worker(state):
        return Path(state["command"]).name == "foundry-embed"

    @classmethod
    def _observe(cls, process, root, known):
        states = cls._snapshot()
        direct = states.get(process.pid)
        if root is None and direct is not None:
            if direct["parent"] != os.getpid():
                raise RuntimeError("direct child ownership could not be established")
            root = dict(direct)
        if root is not None and direct is not None and not cls._matches(root, direct):
            raise RuntimeError("direct child PID identity changed")
        owners = {saved["pid"] for saved in known.values()
                  if cls._matches(saved, states.get(saved["pid"]))}
        if root is not None and cls._matches(root, direct):
            owners.add(process.pid)
        while True:
            children = {pid for pid, state in states.items()
                        if state["parent"] in owners and pid not in owners}
            if not children:
                break
            for pid in children:
                state = states[pid]
                known[(pid, state["started"])] = dict(state)
            owners.update(children)
        live = [dict(states[saved["pid"]]) for saved in known.values()
                if cls._matches(saved, states.get(saved["pid"]))]
        return root, live

    @classmethod
    def _signal_known(cls, known, sig, errors):
        for saved in list(known.values()):
            try:
                live = cls._snapshot(saved["pid"]).get(saved["pid"])
                if cls._matches(saved, live):
                    # No group signaling: every target has its own identity check.
                    os.kill(saved["pid"], sig)
            except ProcessLookupError:
                pass
            except Exception as error:
                errors.append("owned PID signal observation: " + str(error))

    @staticmethod
    def _signal_direct(process, sig, errors):
        # Popen exclusively owns this child and polls before sending a signal.
        try:
            if process.poll() is None:
                process.send_signal(sig)
        except ProcessLookupError:
            pass
        except Exception as error:
            errors.append("direct child signal: " + str(error))

    @classmethod
    def _cleanup(cls, process, root, known, errors):
        remaining = None
        for sig, grace in ((signal.SIGTERM, 10), (signal.SIGKILL, 10)):
            cls._signal_known(known, sig, errors)
            cls._signal_direct(process, sig, errors)
            deadline = time.monotonic() + grace
            while time.monotonic() < deadline:
                try:
                    root, remaining = cls._observe(process, root, known)
                except Exception as error:
                    errors.append("cleanup observation: " + str(error))
                    remaining = None
                    break
                if process.poll() is not None and not remaining:
                    break
                time.sleep(0.1)
            if process.poll() is not None and remaining == []:
                break
        # A broken ps/helper must never bypass direct-owner termination/reaping.
        if process.poll() is None:
            try:
                process.kill()
            except ProcessLookupError:
                pass
            except Exception as error:
                errors.append("direct child kill fallback: " + str(error))
        try:
            process.wait(timeout=10)
        except Exception as error:
            errors.append("direct child wait fallback: " + str(error))
        try:
            root, remaining = cls._observe(process, root, known)
        except Exception as error:
            errors.append("final cleanup observation: " + str(error))
            remaining = None
        return root, remaining

    def _save(self, path, receipt):
        temporary = path.with_suffix(path.suffix + ".tmp")
        temporary.write_text(json.dumps(receipt, indent=2) + "\n")
        temporary.replace(path)

    def run(self, kind, command, timeout, require_worker=None, forbid_worker=False,
            expect_json=True, allowed_exit_codes=(0,)):
        if sys.platform != "darwin":
            raise RuntimeError("NativeFullCommand requires Darwin process identities")
        if not re.fullmatch(r"[a-zA-Z0-9_-]+", kind) or timeout <= 0:
            raise ValueError("invalid phase kind or timeout")
        if require_worker and forbid_worker:
            raise ValueError("worker cannot be both required and forbidden")
        command = [os.fspath(part) for part in command]
        if not command:
            raise ValueError("empty command")
        while True:
            prefix = self.out / f"{self.counter:03d}-{kind}"
            self.counter += 1
            if not any(self.out.glob(prefix.name + ".*")):
                break
        stdout_path = Path(str(prefix) + (".stdout.json" if expect_json else ".stdout.txt"))
        stderr_path = Path(str(prefix) + ".stderr.log")
        receipt_path = Path(str(prefix) + ".receipt.json")
        resource_path = Path(str(prefix) + ".resources.jsonl")
        resources = {"source": "Darwin proc_pid_rusage RUSAGE_INFO_V0 plus shutil.disk_usage",
                     "scope": "owned CLI and observed workers, plus this driver; 0.1s target poll; sampled maxima are lower bounds on true peaks, not GPU/global memory",
                     "identity_scope": "ps PID/lstart identities have one-second resolution",
                     "polls": 0, "minimum_disk_free_bytes": {}, "last_disk_free_bytes": {}, "sample_errors": []}
        for role in ("driver", "cli", "worker"):
            resources[role] = {"samples": 0, "missed_samples": 0, "sampled_peak_rss_bytes": 0,
                               "sampled_peak_physical_footprint_bytes": 0}
        receipt = {"v": 1, "kind": kind, "command": command, "timeout_seconds": timeout,
                   "stdout": str(stdout_path), "stderr": str(stderr_path),
                   "receipt": str(receipt_path), "scratch": str(self.scratch),
                   "resources": resources, "resource_samples": str(resource_path),
                   "status": "running", "exit_code": None, "usage_complete": False,
                   "document_calls": None, "input_tokens": None,
                   "unreported_document_calls": None, "unreported_input_tokens": None}
        self.last_receipt = receipt
        self.receipts.append(receipt)
        self._save(receipt_path, receipt)
        started = time.monotonic()
        before = resource.getrusage(resource.RUSAGE_CHILDREN)
        process, root, known, remaining = None, None, {}, None
        interrupted, stop_reason, stop_at = [], None, None
        errors, failure, forced_cleanup = [], None, False
        natural_exit, report, raw = False, None, None
        previous_handlers = {}

        def signal_request(signum, _frame):
            interrupted.append(signum)

        try:
            for signum in (signal.SIGINT, signal.SIGTERM):
                previous_handlers[signum] = signal.signal(signum, signal_request)
            if min(shutil.disk_usage(self.out).free, shutil.disk_usage(self.scratch).free) < self.DISK_RESERVE:
                raise RuntimeError("less than 512 MiB free before phase")
            # Fail before launching the phase when the native API itself is unavailable.
            self._native_memory(os.getpid())
            with stdout_path.open("xb") as stdout, stderr_path.open("xb") as stderr, resource_path.open("x") as samples:
                next_resource_log = 0.0
                process = subprocess.Popen(command, stdout=stdout, stderr=stderr, start_new_session=True)
                receipt["pid"] = process.pid
                self._save(receipt_path, receipt)
                while True:
                    root, remaining = self._observe(process, root, known)
                    now = time.monotonic()
                    sample = self._sample_resources(root, remaining, resources, now - started)
                    if now - started >= next_resource_log:
                        sample["sampled_peaks_so_far"] = {role: dict(resources[role]) for role in ("driver", "cli", "worker")}
                        samples.write(json.dumps(sample) + "\n")
                        samples.flush()
                        next_resource_log = now - started + 1.0
                    free = min(sample["disk_free_bytes"].values())
                    if stop_reason is None:
                        if interrupted:
                            stop_reason = "operator_cancelled"
                        elif free < self.DISK_RESERVE:
                            stop_reason = "disk_reserve"
                        elif now - started >= timeout:
                            stop_reason = "command_deadline"
                        if stop_reason is not None:
                            stop_at = now
                            self._signal_direct(process, signal.SIGINT, errors)
                    code = process.poll()
                    if code is not None and not remaining:
                        natural_exit = stop_reason is None
                        break
                    if code is not None and stop_reason is None:
                        raise RuntimeError("normal direct-child exit left observed descendants alive")
                    if len(interrupted) > 1:
                        raise RuntimeError("repeated operator signal; finishing owned-process cleanup")
                    if stop_at is not None and now - stop_at >= self.INTERRUPT_GRACE:
                        raise RuntimeError("phase did not finish during 60-second SIGINT grace")
                    time.sleep(0.1)
                process.wait(timeout=10)
            if stop_reason is not None:
                raise RuntimeError("phase stopped: " + stop_reason)
            if errors:
                raise RuntimeError("process signaling/observation error")
        except BaseException as error:
            failure = f"{type(error).__name__}: {error}"
        finally:
            if process is not None:
                try:
                    root, remaining = self._observe(process, root, known)
                except Exception as error:
                    errors.append("final ownership observation: " + str(error))
                    remaining = None
                if process.poll() is None or remaining != []:
                    forced_cleanup = True
                    try:
                        root, remaining = self._cleanup(process, root, known, errors)
                    except BaseException as error:
                        errors.append("cleanup helper failure: " + str(error))
                        remaining = None
                        try:
                            process.kill()
                        except ProcessLookupError:
                            pass
                        except Exception as kill_error:
                            errors.append("direct kill fallback: " + str(kill_error))
                        try:
                            process.wait(timeout=10)
                        except Exception as wait_error:
                            errors.append("direct wait fallback: " + str(wait_error))
                else:
                    try:
                        process.wait(timeout=10)
                    except Exception as error:
                        errors.append("direct child final wait: " + str(error))
            for signum, handler in previous_handlers.items():
                signal.signal(signum, handler)
            after = resource.getrusage(resource.RUSAGE_CHILDREN)
            workers = [state for state in known.values() if self._worker(state)]
            if stdout_path.exists():
                try:
                    if stdout_path.stat().st_size > self.OUTPUT_LIMIT:
                        raise RuntimeError("phase stdout exceeds 16 MiB receipt bound")
                    raw = stdout_path.read_text()
                    report = json.loads(raw) if expect_json else raw
                    if expect_json and not isinstance(report, dict):
                        raise RuntimeError("CLI JSON report must be an object")
                except Exception as error:
                    errors.append("output decoding: " + str(error))
            calls = report.get("document_calls") if isinstance(report, dict) else None
            tokens = report.get("input_tokens") if isinstance(report, dict) else None
            auto_required = type(calls) is int and calls > 0
            required = auto_required if require_worker is None else require_worker
            code = process.returncode if process is not None else None
            if required and not workers:
                errors.append("model calls/phase require an independently observed worker")
            if required and resources["worker"]["samples"] == 0:
                errors.append("model phase has no native worker memory sample")
            if required and resources["cli"]["samples"] == 0:
                errors.append("model phase has no native parent CLI memory sample")
            if forbid_worker and workers:
                errors.append("phase forbids any observed worker")
            if code not in allowed_exit_codes:
                errors.append("unexpected command exit code: " + str(code))
            if process is not None and root is None:
                errors.append("direct child PID/start identity was not observed")
            if remaining != [] and process is not None:
                errors.append("owned descendant absence could not be established")
            if forced_cleanup:
                errors.append("normal lifecycle acceptance requires no forced cleanup")
            if report is None:
                errors.append("no complete command output report")
            scratch_remaining = sorted(path.name for path in self.scratch.glob("w-*"))
            if scratch_remaining:
                errors.append("worker scratch was not naturally reclaimed")
            complete_usage = (failure is None and not errors and type(calls) is int and calls >= 0
                              and type(tokens) is int and tokens >= 0)
            receipt.update({"status": "passed" if failure is None and not errors else "failed",
                            "exit_code": code, "elapsed_seconds": time.monotonic() - started,
                            "user_seconds": after.ru_utime - before.ru_utime,
                            "system_seconds": after.ru_stime - before.ru_stime,
                            "cpu_accounting_scope": "cumulative children delta includes ps observers",
                            "max_rss_bytes_cumulative_children": after.ru_maxrss,
                            "stop_reason": stop_reason, "signals_requested": interrupted,
                            "failure": failure, "errors": errors, "root_identity": root,
                            "observed_descendants": list(known.values()),
                            "observed_workers": workers, "natural_exit": natural_exit,
                            "forced_cleanup": forced_cleanup,
                            "owned_processes_remaining_after_cleanup": remaining,
                            "observed_workers_absent_after_normal_cli_exit": bool(workers) and natural_exit and not forced_cleanup and remaining == [],
                            "scratch_runs_remaining": scratch_remaining,
                            "lifecycle_scope": "observed PID/start absence after direct CLI wait; no Darwin subreaper attribution",
                            "usage_complete": complete_usage,
                            "document_calls": calls if type(calls) is int else None,
                            "input_tokens": tokens if type(tokens) is int else None,
                            "unreported_document_calls": 0 if complete_usage else None,
                            "unreported_input_tokens": 0 if complete_usage else None,
                            "report": report if isinstance(report, dict) else None})
            self._save(receipt_path, receipt)
        if receipt["status"] != "passed":
            raise NativeFullCommandError(failure or "; ".join(errors), receipt)
        return report


FULL_MANIFEST_SHA = "f3748bda2b6ea83ba249e43525d7631262cc5e8eddc0526e6e75f7726b48df63"
FULL_RECIPE = "overview-v1+overview3+kinds1+units5:512+tokenizers 0.23.2 onig"


def full_corpus(archives, corpus, manifest_path):
    """Export every regular member, including normal upstream ignore controls."""
    corpus.mkdir()
    rows = []
    for project, _, _, expected in SCALE_PROJECTS:
        archive = archives / (project + ".tar.gz")
        if sha256(archive) != expected:
            raise RuntimeError("full corpus archive checksum mismatch: " + project)
        with tarfile.open(archive, "r:gz") as source:
            for member in source:
                if not member.isfile():
                    continue
                parts = Path(member.name).parts
                if len(parts) < 2 or member.name.startswith("/") or ".." in parts:
                    raise RuntimeError("invalid full corpus archive member")
                relative = Path(project, *parts[1:])
                data = source.extractfile(member).read()
                destination = corpus / relative
                destination.parent.mkdir(parents=True, exist_ok=True)
                with destination.open("xb") as output:
                    output.write(data)
                rows.append([relative.as_posix(), len(data), hashlib.sha256(data).hexdigest()])
    rows.sort()
    encoded = (json.dumps({"files": rows}, separators=(",", ":"), ensure_ascii=True) + "\n").encode()
    if (len(rows), sum(row[1] for row in rows), hashlib.sha256(encoded).hexdigest()) != (
            69565, 324780308, FULL_MANIFEST_SHA):
        raise RuntimeError("full corpus population differs from the pinned Rust/Go manifest")
    with manifest_path.open("xb") as output:
        output.write(encoded)
    return rows


def verify_full_corpus(corpus, rows):
    expected = {row[0] for row in rows}
    actual = {path.relative_to(corpus).as_posix() for path in corpus.rglob("*")
              if path.is_file() and not path.is_symlink()}
    if actual != expected:
        raise RuntimeError("full corpus regular-file set changed")
    for relative, size, expected_sha in rows:
        path = corpus / relative
        if path.is_symlink() or path.stat().st_size != size or sha256(path) != expected_sha:
            raise RuntimeError("full corpus bytes changed: " + relative)


def native_full_scale(root, work, out, native_profile, calibration, bootstrap_seconds, env, cargo, stage):
    """Ordinary CLI phases; private artifacts retain every report and accepted store."""
    full = out / "full-scale"
    full.mkdir()
    full_work = work / "full-scale"
    full_work.mkdir()
    state = {"v": 1, "status": "running", "complete": False,
             "scope": "all 67663 admitted pinned Rust/Go sources, overview-only real Metal inference",
             "bootstrap_budget_seconds": bootstrap_seconds, "verification_budget_seconds": 600,
             "manifest_sha256": FULL_MANIFEST_SHA, "recipe_id": FULL_RECIPE,
             "phase_reports": [], "usage_complete": True,
             "not_accepted": ["full body coverage", "retrieval/task quality", "peak/global GPU pressure", "signed installed package"]}
    began = time.monotonic()

    def save_full():
        state["elapsed_seconds"] = round(time.monotonic() - began, 3)
        temporary = full / "result.json.tmp"
        temporary.write_text(json.dumps(state, indent=2) + "\n")
        temporary.replace(full / "result.json")

    def require(ok, reason):
        if not ok:
            raise RuntimeError(reason)

    def check_index(report, changed):
        require(report["scan_complete"] and not report["partial"] and report["failures"] == 0
                and report["pending_sources"] == 0 and not report["deletions_deferred"]
                and report["deleted"] == 0 and report["reason_code"] is None,
                "full corpus indexing incomplete")
        require(report["changed"] == changed and report["changed"] + report["unchanged"] == 67663,
                "unexpected admitted or changed source population")

    def check_prepare(report, complete=False):
        require(report["recipe_id"] == FULL_RECIPE and report["dimensions"] == 768
                and report["cache_cap_bytes"] == 512 * 1024**2, "full preparation identity changed")
        require(report["provider_code"] is None and report["corrupt_cache_rows"] == 0
                and report["embedded_areas"] == report["area_vectors"] == 0,
                "full preparation provider/cache/area failure")
        deadline_reason = (f"publication was cut off by the {report['budget_seconds']}s budget; "
                           "the vectors stay cached and the index is not published")
        deadline = (report["partial"] and report["reason_code"] == "budget_exhausted"
                    and not report["index_published"] and report["index_reason"] == deadline_reason)
        require(report["reason_code"] in (None, "budget_exhausted")
                and (deadline or (report["index_published"] and report["index_reason"] is None)),
                "unexpected preparation stop or publication failure")
        coverage = report["coverage"]
        require((coverage is not None and coverage["sources"] == 67663) or deadline,
                "missing full corpus coverage")
        if complete:
            require(not report["partial"] and report["reason_code"] is None
                    and coverage["unpartitioned_sources"] == 0
                    and coverage["eligible_units"] == coverage["cached_current_units"]
                    == coverage["searchable_current_units"] == 67585, "full overview coverage incomplete")

    def check_audit(report, expected_orphans=0):
        require(report["sources"] == 67663 and report["partition_coverage"] == "complete"
                and report["unpartitioned_sources"] == report["missing_units"] == 0
                and report["eligible_units"] == report["cached_current_units"]
                == report["searchable_current_units"] == 67585, "current full overview audit incomplete")
        require(report["index"]["available"] and report["index"]["reason"] is None
                and report["cache"]["corrupt_entries"] == report["cache"]["legacy_entries"] == 0
                and report["cache"]["orphan_entries"] == expected_orphans, "full overview audit integrity failure")

    runner = None
    try:
        save_full()
        state["disk_bytes_before_full_setup"] = dict(zip(("total", "used", "free"), shutil.disk_usage(work)))
        save_full()
        require(state["disk_bytes_before_full_setup"]["free"] >= 5 * 1024**3,
                "full scale needs at least 5 GiB free after native setup/calibration")
        require(calibration["status"] == "passed" and calibration["document_calls"] == 128
                and calibration["worker_stopped_and_reaped"] and calibration["unchanged_document_calls"] == 0,
                "successful real native calibration and cleanup required")
        provider_seconds = calibration["provider_ns"] / 1e9
        token_rate = calibration["input_tokens"] / provider_seconds
        state["calibration_projection"] = {
            "provider_tokens_per_second": token_rate,
            "provider_seconds_for_upper_token_bound": 15857818 / token_rate,
            "nonprovider_allowance_seconds": 900,
            "scope": "unweighted 128-code-file sample estimate; not a bound or full-population acceptance"}
        save_full()
        require(15857818 / token_rate + 900 <= bootstrap_seconds,
                "measured Metal calibration does not support the declared full bootstrap budget; keep partial evidence and use the CPU route")
        require(stage("full-scale-cli-build", cargo + ["build", "--locked", "--bin", "foundry"], 1800),
                "native full scale CLI build failed")
        cli = full / "foundry"
        shutil.copy2(work / "target/debug/foundry", cli)
        corpus = full_work / "corpus"
        rows = full_corpus(work / "scale-calibration", corpus, full / "manifest.json")
        verify_full_corpus(corpus, rows)
        bundle = full / "FoundryEmbed.app"
        shutil.copytree(Path(native_profile["worker"]["bundle"]), bundle, symlinks=True)
        require(stage("full-scale-copied-bundle", ["/usr/bin/codesign", "--verify", "--strict", bundle], 60),
                "relocated development bundle signature failed verification")
        profile = json.loads(json.dumps(native_profile))
        scratch = Path(native_profile["worker"]["scratch_root"]) / "full-scale"
        scratch.mkdir(mode=0o700)
        profile.update(name="native-full-gemma2-bf16-overview-512", representation={"kind": "overview-v1"},
                       card_tokens=512, batch=1, cpu={"threads": 2, "seconds": 86400})
        profile["worker"].update(bundle=str(bundle), scratch_root=str(scratch))
        require(profile["descriptor"]["adapter_revision"] == 4 and profile["descriptor"]["dimensions"] == 768
                and profile["memory_ceiling_bytes"] == 4 * 1024**3,
                "full scale requires the calibrated Metal4 function and 4 GiB guard")
        require(sha256(bundle / "Contents/MacOS/foundry-embed") == profile["worker"]["executable_sha256"],
                "copied signed worker bytes changed")
        profile_path = full / "profile.json"
        profile_path.write_text(json.dumps(profile, indent=2) + "\n")
        store = full / "store"
        require(not store.exists(), "full scale requires a fresh store; no imported CPU/sample vectors")
        require(shutil.disk_usage(full).free >= int(2.75 * 1024**3), "insufficient fresh-store disk headroom")
        state.update(binary_sha256=sha256(cli), profile_sha256=sha256(profile_path),
                     worker_sha256=profile["worker"]["executable_sha256"],
                     descriptor=profile["descriptor"], corpus_directory=str(corpus), store_directory=str(store),
                     native_resource_scope={"worker_threads": 2, "worker_threads_source": "native worker default",
                                            "cpu_profile_applies": False, "cpu_seconds_enforced": None,
                                            "wall_enforcement": "CLI control plus monitored command deadlines",
                                            "memory_ceiling_bytes": profile["memory_ceiling_bytes"],
                                            "memory_enforcement": "supervised native physical footprint; not global GPU headroom"},
                     disk_free_before_bootstrap=shutil.disk_usage(full).free,
                     source_archives=[{"project": project, "revision": revision, "url": url, "sha256": digest}
                                      for project, revision, url, digest in SCALE_PROJECTS])
        runner = NativeFullCommand(full / "commands", scratch)
        base = [str(cli), "--store", str(store)]
        bootstrap_started = time.monotonic()
        bootstrap_deadline = bootstrap_started + bootstrap_seconds
        state["bootstrap_started_unix"] = time.time()
        save_full()
        indexed = runner.run("index", base + ["index", str(corpus)], min(900, bootstrap_seconds))
        check_index(indexed, 67663)
        runs = []
        while True:
            remaining = bootstrap_deadline - time.monotonic()
            require(remaining > 30, "declared full bootstrap wall budget exhausted; accepted store retained")
            seconds = min(1800, int(remaining))
            prepared = runner.run("prepare", base + ["semantic", "prepare", "--profile", str(profile_path),
                                  "--budget-seconds", str(seconds), "--development-isolation", "--cache-cap", "536870912"],
                                  seconds + 60, allowed_exit_codes=(0, 1))
            check_prepare(prepared)
            runs.append(prepared)
            state["phase_reports"].append(runner.last_receipt)
            state["document_calls_accounted"] = sum(row["document_calls"] for row in runs)
            state["document_inputs_accounted"] = sum(row["embedded_units"] for row in runs)
            state["input_tokens_accounted"] = sum(row["input_tokens"] for row in runs)
            save_full()
            if not prepared["partial"]:
                break
        check_prepare(prepared, complete=True)
        state["bootstrap_elapsed_seconds"] = time.monotonic() - bootstrap_started
        require(time.monotonic() <= bootstrap_deadline,
                "full bootstrap exceeded its declared wall budget; accepted coverage and reports retained")
        verification_deadline = time.monotonic() + 600

        def verify(kind, arguments, seconds=180, **kwargs):
            remaining = verification_deadline - time.monotonic()
            require(remaining > 0, "full verification wall budget exhausted; completion not claimed")
            return runner.run(kind, base + arguments, min(seconds, remaining), **kwargs)

        audited = verify("audit", ["semantic", "status", "--audit"])
        check_audit(audited)
        count = audited["cache"]["entries"]
        require(60735 <= count <= 65959 and state["document_inputs_accounted"] == count
                and state["document_calls_accounted"] == count
                and 14651529 <= state["input_tokens_accounted"] <= 15857818,
                "actual complete inference contradicts pinned fitting/accounting bounds")
        replay = verify("unchanged-replay", ["semantic", "prepare", "--profile", str(profile_path),
                        "--budget-seconds", "180", "--development-isolation", "--cache-cap", "536870912"],
                        forbid_worker=True)
        check_prepare(replay, complete=True)
        require(replay["provider_state"] is None and all(replay[key] == 0 for key in
                ["document_calls", "input_tokens", "embedded_units", "partitioned_sources"]), "unchanged replay did work")
        generation_path = store / "semantic" / prepared["function_digest"] / "generation.json"
        generation = json.loads(generation_path.read_text())
        require(generation["coverage"] == "complete" and generation["recipe_id"] == FULL_RECIPE
                and generation["function_digest"] == prepared["function_digest"]
                and generation["count"] == count == audited["index"]["entries"]
                and generation["source_revision"] == audited["source_revision"], "published generation identity mismatch")
        (full / "published-generation-before-churn.json").write_text(json.dumps(generation, indent=2) + "\n")
        state["initial_complete_overview_verified"] = True
        state["initial_complete_distinct_vectors"] = count
        save_full()
        trace = full / "serving-trace"
        delivered = verify("serve", ["context", "how are goroutines scheduled for concurrent work", "--strategy", "search",
                           "--tokens", "1024", "--semantic-profile", str(profile_path), "--development-isolation",
                           "--experimental-behavior-context", "--retrieval-trace-dir", str(trace)], require_worker=True, expect_json=False)
        records = [json.loads(path.read_text()) for path in trace.glob("*.json")]
        require(len(records) == 1 and not records[0]["trace"]["truncated"]
                and records[0]["outcome"]["status"] == "success"
                and records[0]["outcome"]["tokens"] <= 1024
                and any(event.get("stage") == "semantic" and event.get("decision", {}).get("state") == "dense"
                        and event["decision"]["hits"] > 0 for event in records[0]["trace"]["events"]),
                "serving did not execute real dense retrieval")
        handle = re.search(r"[^\s`]+#[0-9]+-[0-9]+@[0-9a-f]{32}\.[0-9a-f]{16}", delivered)
        require(handle is not None, "serving delivered no current source handle")
        verify("retrieve", ["retrieve", "--handle", handle.group(0), "--tokens", "1024"],
               forbid_worker=True, expect_json=False)
        changed_path = corpus / "go/README.md"
        original = changed_path.read_bytes()
        old_title = b"# The Go Programming Language"
        require(original.startswith(old_title), "changed-input fixture source identity differs")
        changed = original.replace(old_title, b"# The Go Programming Language: Native Overview Change", 1)
        state["churn"] = {"path": "go/README.md", "original_sha256": hashlib.sha256(original).hexdigest(),
                          "changed_sha256": hashlib.sha256(changed).hexdigest(), "restored_file": False,
                          "restored_store": False}
        save_full()
        try:
            changed_path.write_bytes(changed)
            check_index(verify("changed-index", ["index", str(corpus)]), 1)
            stale = verify("changed-status", ["semantic", "status", "--audit"])
            require(stale["sources"] == 67663 and stale["unpartitioned_sources"] == 1
                    and stale["searchable_current_units"] == 67584,
                    "changed source was not immediately excluded from current semantic coverage")
            changed_report = verify("changed-prepare", ["semantic", "prepare", "--profile", str(profile_path),
                                    "--budget-seconds", "120", "--development-isolation", "--cache-cap", "536870912"],
                                    require_worker=True)
            check_prepare(changed_report, complete=True)
            require(changed_report["embedded_units"] == changed_report["document_calls"] == 1
                    and changed_report["partitioned_sources"] == 1, "one changed overview did not infer exactly one input")
            state["churn"]["changed_document_calls"] = 1
        finally:
            changed_path.write_bytes(original)
            state["churn"]["restored_file"] = True
            save_full()
        check_index(verify("revert-index", ["index", str(corpus)]), 1)
        reverted = verify("revert-prepare", ["semantic", "prepare", "--profile", str(profile_path),
                          "--budget-seconds", "120", "--development-isolation", "--cache-cap", "536870912"],
                          forbid_worker=True)
        check_prepare(reverted, complete=True)
        require(reverted["document_calls"] == reverted["embedded_units"] == reverted["input_tokens"] == 0
                and reverted["provider_state"] is None and reverted["partitioned_sources"] == 1,
                "revert failed to reuse the original cached overview")
        final_audit = verify("final-audit", ["semantic", "status", "--audit"])
        check_audit(final_audit, expected_orphans=1)
        require(final_audit["index"]["entries"] == count and final_audit["cache"]["entries"] == count + 1,
                "final generation/cache changed beyond the one intentional orphan")
        verify_full_corpus(corpus, rows)
        require(time.monotonic() <= verification_deadline, "final corpus verification exceeded its wall budget")
        final_generation = json.loads(generation_path.read_text())
        require(final_generation["coverage"] == "complete" and final_generation["count"] == count
                and final_generation["source_revision"] == final_audit["source_revision"],
                "final reverted generation is not current")
        (full / "published-generation-final.json").write_text(json.dumps(final_generation, indent=2) + "\n")
        state["churn"].update(restored_store=True, reverted_document_calls=0, retained_orphan_vectors=1)
        observed_worker_peak = max(receipt["resources"]["worker"]["sampled_peak_physical_footprint_bytes"]
                                   for receipt in runner.receipts)
        require(0 < observed_worker_peak <= profile["memory_ceiling_bytes"],
                "sampled worker footprint exceeded the profile guard or was not observed")
        state.update(status="passed", complete=True, admitted_sources=67663, file_locations=67585,
                     sources_without_cards=78, distinct_vectors=count, function_digest=prepared["function_digest"],
                     unchanged_document_calls=0, real_dense_serving=True, real_source_retrieve=True,
                     generation_sha256=sha256(generation_path), disk_free_final=shutil.disk_usage(full).free)
        return state
    except BaseException as error:
        state.update(status="partial_or_failed", complete=False, reason=str(error))
        if runner is not None:
            state["usage_complete"] = all(receipt.get("usage_complete", False) for receipt in runner.receipts
                                          if receipt["kind"] in ("prepare", "unchanged-replay", "changed-prepare", "revert-prepare"))
        raise
    finally:
        if runner is not None:
            state["command_receipts"] = runner.receipts
            observed_resources = [receipt["resources"] for receipt in runner.receipts]
            state["sampled_resources"] = {
                "scope": "per-command native samples, including loading; true peaks and global/GPU pressure remain unknown",
                "minimum_disk_free_bytes": min((min(row["minimum_disk_free_bytes"].values()) for row in observed_resources
                                                if row["minimum_disk_free_bytes"]), default=None),
                "processes": {role: {
                    "samples": sum(row[role]["samples"] for row in observed_resources),
                    "missed_samples": sum(row[role]["missed_samples"] for row in observed_resources),
                    "sampled_peak_rss_bytes": max((row[role]["sampled_peak_rss_bytes"] for row in observed_resources), default=None),
                    "sampled_peak_physical_footprint_bytes": max((row[role]["sampled_peak_physical_footprint_bytes"] for row in observed_resources), default=None)
                } for role in ("driver", "cli", "worker")}}
            document_phases = [receipt for receipt in runner.receipts if receipt["kind"] in
                               ("prepare", "unchanged-replay", "changed-prepare", "revert-prepare")]
            state["usage_complete"] = all(receipt.get("usage_complete", False) for receipt in document_phases)
            state["all_document_phases_calls_accounted"] = sum(receipt.get("document_calls") or 0 for receipt in document_phases)
            state["all_document_phases_tokens_accounted"] = sum(receipt.get("input_tokens") or 0 for receipt in document_phases)
            state["unreported_document_calls"] = 0 if state["usage_complete"] else None
            state["unreported_document_tokens"] = 0 if state["usage_complete"] else None
            state["serving_accounting"] = {
                "context_invocations": sum(receipt["kind"] == "serve" for receipt in runner.receipts),
                "query_calls": None, "query_input_tokens": None,
                "scope": "CLI dense trace and worker lifetime verified; this CLI boundary exposes no per-query model-call/token counter"}
        save_full()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--out", required=True, type=Path)
    parser.add_argument("--work", required=True, type=Path)
    parser.add_argument("--generation", action="store_true",
                        help="also run bounded owned Qwen 0.5B Metal generation after embedding cleanup")
    parser.add_argument("--scale-calibration", action="store_true",
                        help="after selected native checks, measure 128 fixed public Rust/Go overview inputs in a fresh store")
    parser.add_argument("--full-scale", action="store_true",
                        help="after real 128-input calibration, run all pinned Rust/Go overview inputs and verification in a fresh store")
    parser.add_argument("--full-scale-budget-seconds", type=int, default=7200,
                        help="full corpus bootstrap wall budget, excluding native setup and the separate 600-second verification budget")
    args = parser.parse_args()
    if not 1200 <= args.full_scale_budget_seconds <= 10800:
        parser.error("full-scale-budget-seconds must be 1200..10800")
    if args.full_scale:
        args.scale_calibration = True
    root = Path(__file__).resolve().parents[1]
    out, work = args.out.resolve(), args.work.resolve()
    for path in (out, work):
        if path.exists() or path == root or root in path.parents:
            parser.error("out/work must be fresh paths outside the checkout")
    if out in work.parents or work in out.parents or out == work:
        parser.error("out/work must be separate directories")
    out.mkdir(parents=True)
    work.mkdir(parents=True)
    result = {
        "v": 1, "status": "running", "os": platform.system(),
        "arch": platform.machine(), "stages": [], "llama_cpp": LLAMA,
        "adapter_revision": 4, "memory_ceiling_bytes": 4 * 1024**3,
        "scope": "native CPU admission, synthetic lifecycle, Metal embedding receipt and normal cleanup",
        "not_accepted": ["CPU/Metal cross-host numerical comparison", "real-model owner-death under pressure", "generation worker", "global GPU headroom", "signed installed package", "retrieval or task quality"],
    }
    env = dict(os.environ)
    env.update(CARGO_TARGET_DIR=str(work / "target"), CARGO_BUILD_JOBS="2",
               CARGO_PROFILE_DEV_DEBUG="0", CARGO_PROFILE_TEST_DEBUG="0")
    cargo = ["cargo", "+1.90.0"]
    observed_workers = {}

    def processes():
        listing = subprocess.check_output(
            ["/bin/ps", "-axo", "pid=,ppid=,lstart=,comm="], text=True, timeout=5)
        rows = {}
        for line in listing.splitlines():
            fields = line.split(None, 7)
            if len(fields) == 8:
                rows[int(fields[0])] = {
                    "pid": int(fields[0]), "parent_pid": int(fields[1]),
                    "started": " ".join(fields[2:7]), "command": fields[7],
                }
        return rows

    def host_cpu_snapshot(label):
        # Diagnostic only, taken outside the CPU test's measured window so it never
        # loads the host the test samples. top's second sample covers the current
        # one-second interval; it lists short process names, never arguments.
        try:
            listing = subprocess.run(
                ["/usr/bin/top", "-l", "2", "-s", "1", "-n", "12", "-o", "cpu",
                 "-stats", "pid,ppid,cpu,time,state,command"],
                capture_output=True, text=True, timeout=20).stdout
            current = listing[listing.rfind("Processes:"):] if "Processes:" in listing else listing
            text = "\n".join(current.splitlines()[:40])
        except (OSError, subprocess.SubprocessError) as error:
            text = f"unavailable: {error}"
        with (out / "cpu-pressure-host-processes.log").open("a") as log:
            log.write(f"=== {label} {time.strftime('%Y-%m-%dT%H:%M:%SZ', time.gmtime())}\n{text}\n")
        result.setdefault("cpu_diagnostics", {"log": "cpu-pressure-host-processes.log",
                                              "window": "before and after the CPU policy test, never inside it",
                                              "snapshots": []})["snapshots"].append(label)

    def observe_workers(owner, worker_name):
        rows = processes()
        for pid, row in rows.items():
            if Path(row["command"]).name != worker_name:
                continue
            ancestor, visited = pid, set()
            while ancestor in rows and ancestor not in visited:
                if ancestor == owner:
                    observed_workers[(pid, row["started"])] = row
                    break
                visited.add(ancestor)
                ancestor = rows[ancestor]["parent_pid"]

    def save():
        temporary = out / "result.json.tmp"
        temporary.write_text(json.dumps(result, indent=2) + "\n")
        temporary.replace(out / "result.json")

    def stage(name, command, timeout=1800, test=False, extra_env=None, observe=None):
        item = {"name": name, "status": "running", "command": list(map(str, command))}
        result["stages"].append(item)
        save()
        print(f"Starting {name}", flush=True)
        started = time.monotonic()
        process = None
        try:
            with (out / f"{name}.log").open("w") as log:
                process = subprocess.Popen(item["command"], cwd=root,
                    env=env | (extra_env or {}), stdout=log, stderr=subprocess.STDOUT,
                    start_new_session=True)
                try:
                    if observe:
                        deadline = time.monotonic() + timeout
                        while process.poll() is None:
                            observe_workers(process.pid, observe)
                            remaining = deadline - time.monotonic()
                            if remaining <= 0:
                                raise subprocess.TimeoutExpired(item["command"], timeout)
                            try:
                                process.wait(timeout=min(0.1, remaining))
                            except subprocess.TimeoutExpired:
                                pass
                        code = process.returncode
                    else:
                        code = process.wait(timeout=timeout)
                except subprocess.TimeoutExpired:
                    os.killpg(process.pid, signal.SIGTERM)
                    try:
                        process.wait(timeout=10)
                    except subprocess.TimeoutExpired:
                        os.killpg(process.pid, signal.SIGKILL)
                        process.wait()
                    code = 124
            item["exit_code"] = code
            passed = code == 0
            if test and passed:
                log_text = (out / f"{name}.log").read_text(errors="replace")
                passed = re.search(r"test result: ok\. 1 passed; 0 failed;", log_text) is not None
                item["exactly_one_test_passed"] = passed
            item["status"] = "passed" if passed else "failed"
        except (OSError, subprocess.SubprocessError) as error:
            item.update(status="failed", error=str(error), exit_code=127)
        finally:
            if process is not None and process.poll() is None:
                os.killpg(process.pid, signal.SIGKILL)
                process.wait()
            if observe:
                item["observed_workers"] = [row for row in observed_workers.values()
                                            if Path(row["command"]).name == observe]
            item["elapsed_seconds"] = round(time.monotonic() - started, 3)
            save()
        print(f"Finished {name}: {item['status']}", flush=True)
        return item["status"] == "passed"

    def test(name, target, ignored=False, extra_env=None, observe=None):
        command = cargo + ["test", "--locked", "--features", "test-faults"] + target + [name, "--", "--exact", "--nocapture", "--test-threads=1"]
        if ignored:
            command.append("--ignored")
        return stage(name.replace("::", "-"), command, test=True, extra_env=extra_env, observe=observe)

    def require(ok, reason):
        if not ok:
            raise RuntimeError(reason)

    def cleanup(worker_name, scratch=None, minimum=1):
        # Only identities actually observed below this test's process tree count.
        # A later absent process without prior observation never proves cleanup.
        observed = {key: row for key, row in observed_workers.items()
                    if Path(row["command"]).name == worker_name}
        require(len(observed) >= minimum,
                f"expected {minimum} observed {worker_name} workers; cleanup evidence unavailable")
        deadline = time.monotonic() + 10
        while True:
            current = processes()
            workers = [row for (pid, started), row in observed.items()
                       if pid in current and current[pid]["started"] == started]
            runs = sorted(path.name for path in scratch.glob("w-*")) if scratch else []
            if not workers and not runs:
                break
            require(time.monotonic() < deadline,
                    f"{worker_name} cleanup incomplete: workers={workers}, scratch={runs}")
            time.sleep(0.1)
        item = {"name": worker_name + "-cleanup", "status": "passed",
                "observed_workers": list(observed.values()), "remaining_observed_workers": 0}
        if scratch:
            item["remaining_scratch_runs"] = 0
        result["stages"].append(item)
        save()

    try:
        save()
        if platform.system() != "Darwin" or platform.machine() != "arm64" or os.geteuid() == 0:
            result.update(status="blocked", reason="requires_nonroot_native_macos_arm64")
            return 2
        result["checkout_commit"] = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=root, text=True).strip()
        require(stage("host", ["/usr/sbin/sysctl", "hw.memsize", "hw.logicalcpu"], 60), "host observation failed")
        require(stage("rust", ["rustc", "+1.90.0", "-vV"], 60), "Rust 1.90.0 unavailable")
        # Recorded, not changed: indexing can occupy the host the CPU test samples.
        stage("spotlight-status", ["/usr/bin/mdutil", "-a", "-s"], 30)
        cpu_ok = True
        for name in ("native_macos_cpu_observation", "native_macos_cpu_pressure_pauses_and_resumes_both_lanes"):
            if name.startswith("native_macos_cpu_pressure"):
                host_cpu_snapshot("before " + name)
            cpu_ok = test("model_resources::tests::" + name, ["--lib"], True) and cpu_ok
            if name.startswith("native_macos_cpu_pressure"):
                host_cpu_snapshot("after " + name)
        # CPU policy tests keep their own acceptance result. A busy host may
        # lack their idle prerequisite; preserve that failure while independently
        # collecting model evidence behind the held-file and lifecycle gates.
        result["cpu_policy_acceptance"] = "passed" if cpu_ok else "failed"
        if not cpu_ok:
            result["not_accepted"].append("native CPU policy acceptance")
        save()
        readiness_ok = True
        for name in (
            "held_gguf_check_uses_the_open_file_and_keeps_its_descriptor",
            "held_gguf_check_refuses_split_and_malformed_files_without_closing_owner",
        ):
            readiness_ok = test("neural::worker_runtime::tests::" + name, ["--lib"]) and readiness_ok
        for name in FIXTURES:
            readiness_ok = test(name, ["--test", "embed_worker"]) and readiness_ok

        # Actual compute dispatch, not a CPU architecture or display enumeration.
        probe = work / "metal-probe.swift"
        probe.write_text(METAL_PROBE)
        metal_ok = stage("metal-probe", ["/usr/bin/xcrun", "swift", "-module-cache-path", work / "swift-cache", probe, out / "metal-probe.json"], 120)
        if not metal_ok:
            blocked = (out / "metal-probe.json").exists() and json.loads((out / "metal-probe.json").read_text()).get("status") == "blocked"
            result.update(status="blocked" if blocked and cpu_ok and readiness_ok else "failed", reason="Metal compute probe did not pass; models were not downloaded")
            return 2 if result["status"] == "blocked" else 1
        require(readiness_ok, "held-model validation or native synthetic lifecycle failed; models were not downloaded")
        require(shutil.disk_usage(work).free >= 5 * 1024**3, "less than 5 GiB free for native backend and artifacts")

        model = work / "models"
        model.mkdir()
        # Both public URLs are immutable revisions; hashes remain mandatory.
        for name, url, expected in (
            ("embeddinggemma-2-BF16.gguf", f"https://huggingface.co/ggml-org/embeddinggemma-2-GGUF/resolve/{GGUF_REV}/embeddinggemma-2-BF16.gguf", GGUF_SHA),
            ("tokenizer.json", f"https://huggingface.co/google/embeddinggemma-2/resolve/{TOKENIZER_REV}/tokenizer.json", TOKENIZER_SHA),
        ):
            destination = model / name
            require(stage("download-" + name, ["curl", "--fail", "--location", "--proto", "=https", "--proto-redir", "=https", "--retry", "3", "--max-time", "600", "--output", destination, url], 650), "artifact download failed")
            require(sha256(destination) == expected, "artifact checksum mismatch: " + name)
        backend = work / "llama.cpp"
        require(stage("backend-init", ["git", "init", backend], 30), "backend init failed")
        require(stage("backend-fetch", ["git", "-C", backend, "fetch", "--depth", "1", "https://github.com/ggml-org/llama.cpp.git", LLAMA], 300), "pinned backend fetch failed")
        env["LLAMA_CPP_DIR"] = str(backend)
        require(stage("native-worker-build", cargo + ["build", "--locked", "--features", "embed-worker", "--bin", "foundry-embed"], 2700), "native embedding build failed")

        scratch = work / "scratch"
        scratch.mkdir(mode=0o700)
        bundle = work / "FoundryEmbed.app"
        profile = {
            "v": 2, "name": "native-acceptance-gemma2-bf16-8192", "model_dir": str(model),
            "worker": {"bundle": str(bundle), "executable_sha256": "0" * 64, "scratch_root": str(scratch)},
            "descriptor": {
                "v": 2, "model": f"google/embeddinggemma-2@{TOKENIZER_REV} via ggml-org/embeddinggemma-2-GGUF@{GGUF_REV} BF16",
                "artifact_files": [{"name": "embeddinggemma-2-BF16.gguf", "sha256": GGUF_SHA}, {"name": "tokenizer.json", "sha256": TOKENIZER_SHA}],
                "gguf": "embeddinggemma-2-BF16.gguf", "llama_cpp": LLAMA, "tokenizer": "tokenizers 0.23.2 onig",
                "add_special_tokens": True, "pooling": "mean", "dimensions": 768, "output": "f32",
                "adapter_revision": 4, "document_template": "title: none | text: {text}",
                "geometry": {"v": 1, "input_tokens": 8192, "batch_tokens": 8192, "sequences": 8},
            },
            "query_template": "task: search result | query: {text}", "card_tokens": 8192, "batch": 8,
            "representation": {"kind": "cards-v1"},
            "memory_ceiling_bytes": 4 * 1024**3, "load_timeout_seconds": 120,
        }
        profile_path = out / "native-profile.json"
        profile_path.write_text(json.dumps(profile, indent=2) + "\n")
        require(stage("bundle", ["sh", root / "scripts/embed-worker-bundle.sh", "--binary", work / "target/debug/foundry-embed", "--out", bundle, "--profile", profile_path], 120), "development signing failed")
        executable = bundle / "Contents/MacOS/foundry-embed"
        profile["worker"]["executable_sha256"] = sha256(executable)
        profile_path.write_text(json.dumps(profile, indent=2) + "\n")
        require(stage("linked-libraries", ["otool", "-L", executable], 30), "native binary inspection failed")
        receipt = out / "metal-receipt.json"
        require(test("real_native_embedding_receipt", ["--test", "embedding_parity"], True,
                     {"FOUNDRY_EMBED_PROFILE": str(profile_path), "FOUNDRY_EMBED_RECEIPT": str(receipt)},
                     observe="foundry-embed"), "real Metal receipt failed")
        evidence = json.loads(receipt.read_text())
        require((evidence["os"], evidence["arch"], evidence["descriptor"]["adapter_revision"]) == ("macos", "aarch64", 4), "receipt backend identity mismatch")
        require(evidence["worker_sha256"] == profile["worker"]["executable_sha256"], "receipt executable mismatch")
        require(len(evidence["vectors"]) == 5 and any(len(ids) == 8192 for ids in evidence["input_ids"]), "receipt workload incomplete")

        cleanup("foundry-embed", scratch=scratch)
        result["model_acceptance"] = "embedding_passed"
        save()
        if args.generation:
            # Separate acquisitions, after embedding teardown. This is a small
            # generation fixture, not a combined-model or large-context claim.
            result["generation_budget"] = {
                "model_revision": GENERATION_REV, "artifact_sha256": GENERATION_SHA,
                "artifact_bytes": 491400064, "context_tokens": 4096,
                "batch_tokens": 128, "threads": 2, "output_tokens": 512,
                "memory_bytes": 2 * 1024**3, "memory_enforcement": "supervised_footprint",
                "gpu_layers": 25, "gpu_allocation_bytes": 1024**3,
                "gpu_reserve_bytes": 256 * 1024**2,
                "cpu_seconds": 180, "load_seconds": 90, "call_seconds": 90,
                "estimated_f16_kv_bytes": 48 * 1024**2,
                "gpu_limit_scope": "monitored process allocation; not a hard quota or global free memory",
            }
            save()
            generation_model = model / GENERATION_FILE
            url = f"https://huggingface.co/Qwen/Qwen2.5-Coder-0.5B-Instruct-GGUF/resolve/{GENERATION_REV}/{GENERATION_FILE}"
            require(stage("download-generation-gguf", ["curl", "--fail", "--location", "--proto", "=https", "--proto-redir", "=https", "--retry", "3", "--max-time", "600", "--output", generation_model, url], 650), "generation artifact download failed")
            require(generation_model.stat().st_size == 491400064 and sha256(generation_model) == GENERATION_SHA, "generation artifact identity mismatch")
            require(stage("generation-worker-build", cargo + ["build", "--locked", "--features", "generate-worker", "--bin", "foundry-generate"], 2700), "native generation build failed")
            generation_bundle = work / "FoundryGenerate.app"
            require(stage("generation-bundle", ["sh", root / "scripts/generate-worker-bundle.sh", work / "target/debug/foundry-generate", generation_bundle], 120), "generation development signing failed")
            generation_executable = generation_bundle / "Contents/MacOS/foundry-generate"
            generation_hash = sha256(generation_executable)
            require(stage("generation-linked-libraries", ["otool", "-L", generation_executable], 30), "generation binary inspection failed")
            generation_receipt = out / "generation-receipt.json"
            require(test("local_real_metal_generation_accepts_a_cited_brief_and_reuses_it", ["--test", "knowledge"], True,
                         {"FOUNDRY_GENERATION_SMOKE_WORKER": str(generation_executable),
                          "FOUNDRY_GENERATION_SMOKE_GGUF": str(generation_model),
                          "FOUNDRY_GENERATION_SMOKE_RECEIPT": str(generation_receipt)},
                         observe="foundry-generate"), "real Metal generation lifecycle failed")
            generated = json.loads(generation_receipt.read_text())
            require((generated["status"], generated["os"], generated["arch"]) == ("passed", "macos", "aarch64"), "generation receipt platform mismatch")
            require(generated["worker_sha256"] == generation_hash and generated["artifact_sha256"] == GENERATION_SHA, "generation receipt artifact identity mismatch")
            config = generated["config"]
            require(config["executable_sha256"] == generation_hash and config["llama_cpp"] == LLAMA, "generation receipt backend identity mismatch")
            require(config["device"] == {"kind": "metal", "layers": 25, "allocation_bytes": 1024**3, "reserve_bytes": 256 * 1024**2}, "generation device budget mismatch")
            require((config["memory_enforcement"], config["memory_bytes"], config["context_tokens"]) == ("supervised_footprint", 2 * 1024**3, 4096), "generation resource budget mismatch")
            for name in ("initial", "changed"):
                report = generated[name]
                worker = report["local_worker"]
                require(report["complete"] and report["calls"] == report["generated"] == worker["launches"] == worker["calls"] == 1, "generation call receipt incomplete")
                require(report["usage_unknown_calls"] == 0 and report["observed_prompt_tokens"] > 0 and report["observed_completion_tokens"] > 0, "generation usage unavailable")
                require(0 < worker["peak_gpu_bytes"] <= 1024**3 and 0 < worker["peak_memory_bytes"] <= 2 * 1024**3, "generation native resource observation unavailable or over budget")
                require(worker["pid"] is None and worker["last_stop"] == "stopped" and worker["isolation"] == "macos-app-sandbox-development-v1", "generation teardown or isolation mismatch")
            for name in ("current", "restored"):
                report = generated[name]
                worker = report["local_worker"]
                require(report["complete"] and report["calls"] == worker["launches"] == worker["calls"] == worker["verified_bytes"] == worker["peak_memory_bytes"] == worker["peak_gpu_bytes"] == 0 and worker["pid"] is None, "generation replay acquired model resources")
            require(generated["restored"]["reused_cached"] == 1 and generated["http_calls"] == 0, "generation restart/revert or owned-provider mismatch")
            cleanup("foundry-generate", minimum=2)
            result["model_acceptance"] = "embedding_and_generation_passed"
            result["scope"] += "; owned Metal generation, change and current/restart/revert reuse"
            result["not_accepted"].remove("generation worker")
            result["not_accepted"].extend(["generation prose correctness or coding-task benefit", "all generation operations on GPU", "combined embedding and generation pressure", "native generator owner-death and adversarial isolation"])
        if args.scale_calibration:
            # Every preceding owner has been reaped. Reuse only the verified
            # model/backend/bundle, never a store or any CPU/Metal cache rows.
            calibration = work / "scale-calibration"
            calibration.mkdir()
            calibration_out = out / "scale-calibration"
            calibration_out.mkdir()
            require(shutil.disk_usage(work).free >= 2 * 1024**3,
                    "less than 2 GiB free for bounded calibration artifacts")
            for project, _, url, _ in SCALE_PROJECTS:
                require(stage("scale-download-" + project,
                              ["curl", "--fail", "--location", "--proto", "=https", "--proto-redir", "=https",
                               "--retry", "3", "--max-time", "600", "--output", calibration / (project + ".tar.gz"), url], 650),
                        "scale corpus archive download failed")
            scale_sample(calibration, calibration / "corpus", calibration_out / "sample.json")
            scale_profile = json.loads(json.dumps(profile))
            scale_profile.update(name="native-scale-gemma2-bf16-overview-512", card_tokens=512, batch=1,
                                 representation={"kind": "overview-v1"})
            # The signed bundle already grants the enclosing private scratch root.
            scale_scratch = scratch / "scale-calibration"
            scale_scratch.mkdir(mode=0o700)
            scale_profile["worker"]["scratch_root"] = str(scale_scratch)
            scale_profile_path = calibration_out / "profile.json"
            scale_profile_path.write_text(json.dumps(scale_profile, indent=2) + "\n")
            observed_workers.clear()
            require(test("real_metal_overview_scale_calibration", ["--test", "native_scale"], True,
                         {"FOUNDRY_SCALE_PROFILE": str(scale_profile_path),
                          "FOUNDRY_SCALE_CORPUS": str(calibration / "corpus"),
                          "FOUNDRY_SCALE_STORE": str(calibration / "store"),
                          "FOUNDRY_SCALE_OUT": str(calibration_out)}, observe="foundry-embed"),
                    "bounded native scale calibration failed; retain partial call evidence")
            calibration_receipt = json.loads((calibration_out / "receipt.json").read_text())
            require((calibration_receipt["status"], calibration_receipt["os"], calibration_receipt["arch"])
                    == ("passed", "macos", "aarch64"), "scale calibration platform mismatch")
            require(calibration_receipt["worker_sha256"] == profile["worker"]["executable_sha256"]
                    and calibration_receipt["descriptor"] == profile["descriptor"],
                    "scale calibration function identity mismatch")
            require(calibration_receipt["document_calls"] == calibration_receipt["distinct_inputs"] == 128
                    and calibration_receipt["input_tokens"] > 0 and calibration_receipt["provider_ns"] > 0
                    and calibration_receipt["worker_stopped_and_reaped"]
                    and calibration_receipt["unchanged_document_calls"] == 0,
                    "scale calibration accounting incomplete")
            require(calibration_receipt["cache_cap_bytes"] == 512 * 1024**2,
                    "scale calibration cache limit mismatch")
            require(calibration_receipt["sample_manifest_sha256"] == sha256(calibration_out / "sample.json"),
                    "scale sample receipt mismatch")
            cleanup("foundry-embed", scratch=scale_scratch)
            result["scope"] += "; fixed 128-input public overview calibration and unchanged replay"
            result["scale_measurement_gate"] = {
                "status": "measured_sample_only", "receipt": "scale-calibration/receipt.json",
                "full_scale_authorized_or_completed": False,
                "next_decision": "review actual inputs, tokens, provider and command time, observed memory, and disk before declaring a separate full-corpus wall budget",
                "not_established": ["67663-source completion", "weighted full-corpus throughput", "true peak or global GPU headroom"],
            }
        if args.full_scale:
            full_receipt = native_full_scale(root, work, out, profile, calibration_receipt,
                                             args.full_scale_budget_seconds, env, cargo, stage)
            result["full_scale"] = {"status": full_receipt["status"], "complete": full_receipt["complete"],
                                    "receipt": "full-scale/result.json"}
            result["scope"] += "; full pinned Rust/Go overview inference, replay, serving and one edit/revert sanity"
        if not cpu_ok:
            result.update(status="failed", reason="selected model and lifecycle checks passed; native CPU policy acceptance remains failed; see retained CPU stage logs")
            return 1
        result.update(status="passed", reason="all selected native checks passed; broader acceptance remains open")
        return 0
    except Exception as error:
        result.update(status="failed", reason=str(error))
        print(str(error), file=sys.stderr, flush=True)
        return 1
    finally:
        save()
        print(f"Native acceptance status: {result['status']}; evidence: {out}", flush=True)


if __name__ == "__main__":
    sys.exit(main())
