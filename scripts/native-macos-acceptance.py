#!/usr/bin/env python3
"""Explicit native acceptance; writes evidence before failing or reporting blocked.

Run from a checkout with Rust 1.90.0, Python 3, CMake and Xcode/Metal tools:
  python3 scripts/native-macos-acceptance.py --out /absolute/new-results \
      --work /absolute/new-work [--generation]
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
import shutil
import signal
import subprocess
import sys
import time


LLAMA = "b9acf138a1e28ce1fc23b5a4fc4b12444b50f7ea"
GGUF_REV = "bfcd298762cc34d0357ece5ebdd31791a3a374d8"
TOKENIZER_REV = "914f7f89142e33e77833254d9c9b90c3cef7303b"
GGUF_SHA = "68bae29d62fb8c7d23e98d21fd4662753ddd636e6b62b8a70dfe059a9844f216"
TOKENIZER_SHA = "4d777ef5bdc1aa36227abdfb77c3e49e7b9c892d16e1b6bda41c393504828be4"
GENERATION_REV = "ebb2015119c907b064c512bf053e945850b5875f"
GENERATION_FILE = "qwen2.5-coder-0.5b-instruct-q4_k_m.gguf"
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


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--out", required=True, type=Path)
    parser.add_argument("--work", required=True, type=Path)
    parser.add_argument("--generation", action="store_true",
                        help="also run bounded owned Qwen 0.5B Metal generation after embedding cleanup")
    args = parser.parse_args()
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
        cpu_ok = True
        for name in ("native_macos_cpu_observation", "native_macos_cpu_pressure_pauses_and_resumes_both_lanes"):
            cpu_ok = test("model_resources::tests::" + name, ["--lib"], True) and cpu_ok
        for name in (
            "held_gguf_check_uses_the_open_file_and_keeps_its_descriptor",
            "held_gguf_check_refuses_split_and_malformed_files_without_closing_owner",
        ):
            cpu_ok = test("neural::worker_runtime::tests::" + name, ["--lib"]) and cpu_ok
        for name in FIXTURES:
            cpu_ok = test(name, ["--test", "embed_worker"]) and cpu_ok

        # Actual compute dispatch, not a CPU architecture or display enumeration.
        probe = work / "metal-probe.swift"
        probe.write_text(METAL_PROBE)
        metal_ok = stage("metal-probe", ["/usr/bin/xcrun", "swift", "-module-cache-path", work / "swift-cache", probe, out / "metal-probe.json"], 120)
        if not metal_ok:
            blocked = (out / "metal-probe.json").exists() and json.loads((out / "metal-probe.json").read_text()).get("status") == "blocked"
            result.update(status="blocked" if blocked and cpu_ok else "failed", reason="Metal compute probe did not pass; models were not downloaded")
            return 2 if result["status"] == "blocked" else 1
        require(cpu_ok, "CPU admission, held-model validation or native synthetic lifecycle failed; models were not downloaded")
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
            result["scope"] += "; owned Metal generation, change and current/restart/revert reuse"
            result["not_accepted"].remove("generation worker")
            result["not_accepted"].extend(["generation prose correctness or coding-task benefit", "all generation operations on GPU", "combined embedding and generation pressure", "native generator owner-death and adversarial isolation"])
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
