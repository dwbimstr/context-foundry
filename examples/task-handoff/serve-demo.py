"""Serve an isolated copy of a prepared, closed synthetic store over MCP stdio."""

import argparse
import json
import os
from pathlib import Path
import shutil
import signal
import subprocess
import tempfile


def serve(binary, demo):
    # The loader prints this manifest when preparing the dedicated demo.
    manifest = json.loads((demo / "manifest.json").read_text())
    root = (demo / "source").resolve(strict=True)
    store = (demo / "store").resolve(strict=True)
    if manifest["root"] != str(root) or manifest["store"] != str(store):
        raise ValueError("Use the manifest from this prepared synthetic workspace")
    if set(manifest["tasks"]) != {"pavo-slip", "palisade-press-handoff"}:
        raise ValueError("Expected the two authored handoff tasks")
    with tempfile.TemporaryDirectory(prefix="mcp-session-", dir=demo) as tmp:
        copy = Path(tmp) / "store"
        shutil.copytree(store, copy)
        with subprocess.Popen([
            binary, "--store", copy, "mcp", "--root", root, "--experimental-task-context",
        ]) as owner:
            def stop(signum, frame):
                if owner.poll() is None:
                    owner.send_signal(signum)

            signal.signal(signal.SIGTERM, stop)
            signal.signal(signal.SIGINT, stop)
            return owner.wait()


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--bin", type=Path, required=True)
    parser.add_argument("--demo", type=Path, required=True)
    args = parser.parse_args()
    os.umask(0o077)
    raise SystemExit(serve(args.bin.resolve(strict=True), args.demo.resolve(strict=True)))
