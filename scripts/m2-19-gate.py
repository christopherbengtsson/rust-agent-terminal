#!/usr/bin/env python3
"""Prebuild or run a source/binary-pinned, serial M2 aggregate qualification.

Use a new evidence directory. Run prepare, commit all source changes, then run.
The run phase never builds; it verifies Cargo's prebuilt executable inventory
before and after the exact locked all-target command.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import time

ROOT = Path(__file__).resolve().parents[1]


def command(args, log):
    started = time.time()
    log.write(("\n$ " + " ".join(args) + "\n").encode())
    log.flush()
    result = subprocess.run(args, cwd=ROOT, stdout=log, stderr=subprocess.STDOUT)
    log.flush()
    return {"argv": args, "exit": result.returncode, "started": started,
            "finished": time.time()}


def git(*args):
    return subprocess.check_output(["git", *args], cwd=ROOT).decode().strip()


def digest(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def boundary(inventory):
    return {"source": git("rev-parse", "HEAD"),
            "status": git("status", "--porcelain"),
            "executables": {p: digest(p) for p in inventory}}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("phase", choices=["prepare", "run"])
    parser.add_argument("evidence", type=Path)
    args = parser.parse_args()
    evidence = args.evidence.resolve()
    evidence.mkdir(parents=True, exist_ok=True)
    inventory_path = evidence / "inventory.json"
    if args.phase == "prepare":
        if inventory_path.exists():
            raise SystemExit("Use a fresh evidence directory")
        results = []
        with (evidence / "prepare.log").open("wb") as log:
            for argv in (["cargo", "+stable", "build", "--locked"],
                         ["cargo", "+stable", "fmt", "--all", "--", "--check"],
                         ["cargo", "+stable", "clippy", "--locked", "--all-targets", "--", "-D", "warnings"]):
                result = command(argv, log)
                results.append(result)
                if result["exit"]:
                    (evidence / "prepare-results.json").write_text(json.dumps(results, indent=2))
                    raise SystemExit(result["exit"])
            argv = ["cargo", "+stable", "test", "--locked", "--all-targets", "--no-run", "--message-format=json"]
            built = subprocess.run(argv, cwd=ROOT, capture_output=True)
            (evidence / "prebuild.jsonl").write_bytes(built.stdout)
            (evidence / "prebuild.stderr").write_bytes(built.stderr)
            if built.returncode:
                raise SystemExit(built.returncode)
            paths = {str(ROOT / "target/debug/agent-terminal")}
            for line in built.stdout.splitlines():
                event = json.loads(line)
                if event.get("reason") == "compiler-artifact" and event.get("executable"):
                    paths.add(event["executable"])
            inventory_path.write_text(json.dumps(sorted(paths), indent=2))
            (evidence / "prepare-results.json").write_text(json.dumps(results, indent=2))
        print("Prebuilt. Commit/freeze source before run; no builds or edits during run.")
        return
    inventory = json.loads(inventory_path.read_text())
    before = boundary(inventory)
    if before["status"]:
        raise SystemExit("Refusing runtime interval with uncommitted source: " + before["status"])
    (evidence / "before.json").write_text(json.dumps(before, indent=2))
    versions = {}
    for name, argv in (("rust", ["rustc", "+stable", "--version"]),
                       ("cargo", ["cargo", "+stable", "--version"]),
                       ("os", ["sw_vers"]), ("git", ["git", "--version"]),
                       ("claude", ["claude", "--version"]), ("codex", ["codex", "--version"])):
        result = subprocess.run(argv, capture_output=True, text=True)
        versions[name] = {"exit": result.returncode, "stdout": result.stdout, "stderr": result.stderr}
    (evidence / "versions.json").write_text(json.dumps(versions, indent=2))
    with (evidence / (before["source"][:12] + "-all-targets-serial-nocapture.log")).open("wb") as log:
        result = command(["cargo", "+stable", "test", "--locked", "--all-targets", "--", "--test-threads=1", "--nocapture"], log)
    after = boundary(inventory)
    (evidence / "after.json").write_text(json.dumps(after, indent=2))
    result["boundary_stable"] = before == after
    (evidence / "result.json").write_text(json.dumps(result, indent=2))
    print(json.dumps(result, indent=2))
    sys.exit(result["exit"] or (0 if result["boundary_stable"] else 1))


if __name__ == "__main__":
    main()
