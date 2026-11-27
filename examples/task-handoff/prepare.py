"""Create the two synthetic desktop exercises in a new local Foundry store."""

import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess


def prepare(binary, directory):
    directory.mkdir(mode=0o700)
    root = directory / "source"
    store = directory / "store"
    root.mkdir()

    def foundry(*args, request=None):
        return subprocess.run(
            [binary, "--store", store, *args],
            input=json.dumps(request) if request is not None else None,
            text=True, check=True, capture_output=True, timeout=60,
        ).stdout

    def task(task_id, op, **fields):
        request = {
            "workspace_id": workspace,
            "task_id": task_id,
            "command": {"op": op, **fields},
        }
        return json.loads(foundry("task-context", request=request))

    fixtures = Path(__file__).resolve().parent
    stale = json.loads((fixtures / "stale-case.json").read_text())
    conflict = json.loads((fixtures / "conflict-case.json").read_text())
    change = stale["source"]
    sources = {item["path"]: item["text"] for item in conflict["source_files"]}
    sources[change["path"]] = change["before"]
    # Also allow a fixture created beneath an ignored runs directory.
    (root / ".ignore").write_text("".join(f"!{name}\n" for name in sources))
    for name, content in sources.items():
        if Path(name).name != name or name in {".", ".."}:
            raise ValueError(f"Expected a fixture filename, got {name!r}")
        (root / name).write_bytes(content.encode())

    foundry("index", root)
    status = json.loads(foundry("status"))
    if status["source_count"] != len(sources) or status["index_state"] != "ready":
        raise ValueError("The index did not accept the three fixture sources")
    workspace = status["workspace_id"]
    handles = {}
    for name, content in sources.items():
        data = content.encode()
        digest = hashlib.sha256(data).hexdigest()
        handle = f"{name}#0-{len(data)}@{digest[:32]}.{workspace[:16]}"
        foundry("retrieve", "--handle", handle)
        handles[name] = handle

    revisions = {}
    for case in (stale, conflict):
        task_id = case["task_id"]
        revision = task(task_id, "create", title=case["title"])["revision"]
        for row in case["entries"]:
            entry = dict(row)
            path = entry.pop("source_path", None)
            entry.pop("original_sha256", None)
            if row["id"] in case.get("link_before_entry_ids", []):
                path = change["path"]
            entry["source_links"] = [handles[path]] if path else []
            revision = task(
                task_id, "apply", expected_revision=revision,
                edits=[{"op": "add", "entry": entry}],
            )["revision"]
        observation = next(e for e in case["entries"] if e["kind"] == "observation")
        fold = {
            "op": "fold", "entry_id": observation["id"],
            "summary": "Recover the attributed original before using this observation.",
        }
        revisions[task_id] = task(
            task_id, "apply", expected_revision=revision, edits=[fold],
        )["revision"]

    (root / change["path"]).write_bytes(change["after"].encode())
    foundry("index", root)
    return {
        "root": str(root), "store": str(store), "workspace_id": workspace,
        "tasks": revisions,
    }


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--bin", type=Path, required=True, help="Installed Foundry executable")
    parser.add_argument("--out", type=Path, required=True, help="New directory for synthetic data")
    args = parser.parse_args()
    os.umask(0o077)
    try:
        print(json.dumps(prepare(args.bin.resolve(strict=True), args.out.resolve()), indent=2))
    except subprocess.CalledProcessError as error:
        raise SystemExit(error.stderr or str(error)) from error
