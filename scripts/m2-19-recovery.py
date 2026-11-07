#!/usr/bin/env python3
"""Read-only ticket-19 pre/post physical-reboot evidence, with explicit owner start.

capture records committed history prefixes and live observations in persistent
qualification state. verify requires a changed real macOS boot UUID. It never
reboots, sleeps, resumes coding, deletes state, or signals a recorded PID.
--start-supervisor is an explicit additional action after a verified boot change;
starting that owner is not native coding resume. Without it, verification only
observes an existing endpoint and records unavailable live evidence as an open
check, never inferring current process state from saved lifecycle labels.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import socket
import sqlite3
import stat
import subprocess
import time
import urllib.parse

CHUNK = 65536
MAX_FILE = 16 * 1024 * 1024


def output(argv, timeout=10):
    completed = subprocess.run(argv, capture_output=True, timeout=timeout)
    if completed.returncode:
        raise RuntimeError(f"{argv[0]} exited {completed.returncode}: " + completed.stderr.decode(errors="replace"))
    return completed.stdout


def sha(path):
    with open(path, "rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def save(path, value):
    # Never overwrite an earlier failed/successful checkpoint or verification.
    with path.open("x") as stream:
        json.dump(value, stream, indent=2, ensure_ascii=False)
        stream.write("\n")
        stream.flush()
        os.fsync(stream.fileno())
    descriptor = os.open(path.parent, os.O_RDONLY | os.O_DIRECTORY)
    try:
        os.fsync(descriptor)
    finally:
        os.close(descriptor)


def boot():
    result = {}
    for name in ["kern.bootsessionuuid", "kern.boottime"]:
        try:
            result[name] = output(["/usr/sbin/sysctl", "-n", name]).decode().strip()
        except Exception as error:
            result[name] = None
            result[name + "_error"] = repr(error)
    return result


def public_observation(state):
    sock = socket.socket(socket.AF_UNIX)
    sock.settimeout(3)
    reader = None
    try:
        sock.connect(str(state / "supervisor.sock"))
        reader = sock.makefile("rb")
        greeting = json.loads(reader.readline())
        if greeting.get("control_denied"):
            return {"available": False, "reason": "another dashboard controls the owner"}
        sock.sendall(b'"Screen"\n')
        return {"available": True, "observation": json.loads(reader.readline())}
    except Exception as error:
        return {"available": False, "reason": repr(error)}
    finally:
        if reader:
            reader.close()
        sock.close()


def prefix(state, identity, length):
    # Hash exactly SQLite's committed extent. Appended/unsynced tails are neither
    # read nor credited. Session UUIDs are parsed before becoming path operands.
    import uuid
    assert str(uuid.UUID(identity)) == identity
    digest = hashlib.sha256()
    chunks = []
    remaining = length
    chunk = 0
    while remaining:
        path = state / f"{identity}-{chunk:08}.transcript"
        descriptor = os.open(path, os.O_RDONLY | os.O_NOFOLLOW)
        try:
            info = os.fstat(descriptor)
            assert stat.S_ISREG(info.st_mode), "transcript is not a regular file"
            take = min(CHUNK, remaining)
            assert info.st_size >= take, "committed transcript prefix is missing"
            data = bytearray()
            while len(data) < take:
                block = os.read(descriptor, take-len(data))
                if not block:
                    raise RuntimeError("transcript shortened while reading committed prefix")
                data.extend(block)
            digest.update(data)
            chunks.append({"name": path.name, "committed_bytes": take,
                           "sha256": hashlib.sha256(data).hexdigest(),
                           "physical_size_observed": info.st_size})
        finally:
            os.close(descriptor)
        chunk += 1
        remaining -= take
    return {"committed_bytes": length, "sha256": digest.hexdigest(), "chunks": chunks}


def database(state, old_prefixes=None):
    uri = "file:" + urllib.parse.quote(str(state / "history.sqlite3"), safe="/") + "?mode=ro"
    connection = sqlite3.connect(uri, uri=True, timeout=5)
    connection.execute("PRAGMA query_only=ON")
    connection.execute("BEGIN")
    try:
        sessions = []
        for identity, label, encoded, durable, retained in connection.execute(
                "SELECT id,identity,state,durable,history_state FROM sessions ORDER BY rowid"):
            record = json.loads(encoded)
            item = {"id": identity, "identity": label,
                    "session_identity": record.get("session_identity"),
                    "native_conversation": record.get("native_conversation"),
                    "agent": record.get("agent"),
                    "saved_lifecycle_historical_only": record.get("lifecycle"),
                    "history_state": retained, "durable": durable}
            if retained == "retained":
                requested = (old_prefixes or {}).get(identity, durable)
                assert durable >= requested, "durable prefix decreased"
                item["prefix"] = prefix(state, identity, requested)
            sessions.append(item)
        workspaces = [{"id": row[0], "task": row[1], "name": row[2], "project": row[3],
                       "path": list(row[4]) if row[4] else None,
                       "checkout": json.loads(row[5]) if row[5] else None}
                      for row in connection.execute("SELECT id,task,name,project,path,checkout FROM workspaces ORDER BY rowid")]
        projects = [{"id": row[0], "name": row[1], "common_dir": list(row[2])}
                    for row in connection.execute("SELECT id,name,common_dir FROM projects ORDER BY rowid")]
        membership = [list(row) for row in connection.execute("SELECT session,workspace FROM workspace_sessions ORDER BY rowid")]
        return {"schema": connection.execute("PRAGMA user_version").fetchone()[0],
                "sessions": sessions, "workspaces": workspaces,
                "projects": projects, "membership": membership}
    finally:
        connection.rollback()
        connection.close()


def code_snapshot(registry):
    result = {"workspaces": [], "recovery_refs": []}
    for workspace in registry["workspaces"]:
        if not workspace["path"]:
            continue
        root = os.fsdecode(bytes(workspace["path"]))
        item = {"id": workspace["id"], "path": workspace["path"], "present": os.path.isdir(root)}
        if item["present"]:
            item["head"] = output(["git", "-C", root, "rev-parse", "HEAD"]).decode().strip()
            paths = output(["git", "-C", root, "ls-files", "-z", "--cached", "--others", "--exclude-standard"]).split(b"\0")
            files = []
            for relative in sorted(set(paths)-{b""}):
                path = os.path.join(os.fsencode(root), relative)
                if not os.path.lexists(path):
                    files.append({"path": list(relative), "missing": True})
                    continue
                info = os.lstat(path)
                if stat.S_ISLNK(info.st_mode):
                    files.append({"path": list(relative), "symlink": list(os.readlink(path))})
                elif stat.S_ISREG(info.st_mode) and info.st_size <= MAX_FILE:
                    files.append({"path": list(relative), "size": info.st_size, "sha256": sha(path)})
                else:
                    files.append({"path": list(relative), "unverified": "nonregular or exceeds explicit16MiB cap"})
            item["files"] = files
        result["workspaces"].append(item)
    for project in registry["projects"]:
        directory = os.fsdecode(bytes(project["common_dir"]))
        references = output(["git", "--git-dir", directory, "for-each-ref", "--format=%(refname) %(objectname)", "refs/rat/"]).decode().splitlines()
        for reference in references:
            name, commit = reference.split()
            output(["git", "--git-dir", directory, "cat-file", "-e", commit+"^{commit}"])
            result["recovery_refs"].append({"project": project["id"], "reference": name,
                                           "commit": commit, "commit_present": True})
    return result


def owned_processes(root):
    # Read live cwd attribution, filtering before persistence; no unrelated
    # process argv/environment is captured and no observed PID is signalled.
    raw = output(["lsof", "-n", "-d", "cwd", "-Fpcn"], timeout=20).decode(errors="replace")
    result = []
    pid = command = None
    for line in raw.splitlines():
        if line.startswith("p"):
            pid = line[1:]
        elif line.startswith("c"):
            command = line[1:]
        elif line.startswith("n"):
            cwd = line[1:]
            if cwd == str(root) or cwd.startswith(str(root)+"/"):
                result.append({"pid_observed_only": pid, "command_name": command, "cwd": cwd})
    return result


def source_identity(repository, commit):
    output(["git", "-C", str(repository), "cat-file", "-e", commit+"^{commit}"])
    return {"commit": commit, "commit_object_present": True,
            "persistent_repository": str(repository.resolve())}


def current(args, old_prefixes=None):
    data = database(args.state, old_prefixes)
    return {"format": 1, "captured_unix_ns": time.time_ns(), "root": str(args.root),
            "state": str(args.state), "boot": boot(),
            "binary": {"path": str(args.binary), "sha256": sha(args.binary)},
            "source": source_identity(args.git_repo, args.source_commit),
            "database": data, "code": code_snapshot(data),
            "public": public_observation(args.state),
            "live_cwd_processes": owned_processes(args.root)}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("action", choices=["capture", "verify"])
    parser.add_argument("--root", type=Path, required=True)
    parser.add_argument("--binary", type=Path)
    parser.add_argument("--git-repo", type=Path, required=True)
    parser.add_argument("--source-commit", required=True)
    parser.add_argument("--checkpoint", type=Path)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--start-supervisor", action="store_true")
    args = parser.parse_args()
    args.root = args.root.resolve(strict=True)
    assert not str(args.root).startswith(("/tmp/", "/private/tmp/")), "reboot state must be persistent"
    assert args.root.stat().st_uid == os.getuid(), "qualification root has another owner"
    args.state = args.root / "state"
    args.binary = (args.binary or args.root / "bin/agent-terminal").resolve(strict=True)
    assert os.path.commonpath([str(args.root), str(args.binary)]) == str(args.root)
    if args.action == "capture":
        assert not args.start_supervisor, "capture never starts processes"
        data = current(args)
        save(args.output, data)
        print(json.dumps({"checkpoint": str(args.output), "sessions": len(data["database"]["sessions"]), "boot": data["boot"]}))
        return
    if not args.checkpoint:
        parser.error("verify requires --checkpoint")
    previous = json.loads(args.checkpoint.read_text())
    assert previous["root"] == str(args.root) and previous["state"] == str(args.state)
    assert previous["binary"]["sha256"] == sha(args.binary), "frozen binary changed"
    assert previous["source"]["commit"] == args.source_commit, "source commit changed"
    now = boot()
    old_uuid = previous["boot"].get("kern.bootsessionuuid")
    boot_changed = bool(old_uuid and now.get("kern.bootsessionuuid") and old_uuid != now["kern.bootsessionuuid"])
    started = None
    if args.start_supervisor:
        assert boot_changed, "explicit owner startup here requires an actual changed boot UUID"
        endpoint = public_observation(args.state)
        assert not endpoint.get("available"), "live owner exists; inspect it instead of starting another"
        assert endpoint.get("reason") != "another dashboard controls the owner", "live dashboard controls the owner"
        # A reboot can retain a refused Unix socket inode. Only the product
        # owner may validate its lock and remove that stale endpoint; this
        # verifier neither unlinks it nor infers ownership from a saved PID.
        logfile = args.root / ("postboot-owner-"+str(time.time_ns())+".log")
        with logfile.open("xb") as stream:
            child = subprocess.Popen([str(args.binary), "--supervisor", str(args.state)],
                                     stdout=stream, stderr=subprocess.STDOUT, start_new_session=True)
        deadline = time.monotonic()+15
        while not public_observation(args.state).get("available"):
            assert child.poll() is None, "explicit supervisor startup failed; inspect log"
            assert time.monotonic() < deadline, "explicit supervisor startup unavailable"
            time.sleep(.05)
        started = {"explicit_start": True, "live_handle_pid_at_start": child.pid,
                   "log": str(logfile), "coding_launch_requested": False}
    prefixes = {row["id"]: row["durable"] for row in previous["database"]["sessions"]
                if row["history_state"] == "retained"}
    data = current(args, prefixes)
    checks = {"actual_boot_uuid_changed": boot_changed,
              "actual_boottime_changed": bool(previous["boot"].get("kern.boottime") and now.get("kern.boottime") and previous["boot"]["kern.boottime"] != now["kern.boottime"]),
              "binary_unchanged": data["binary"]["sha256"] == previous["binary"]["sha256"],
              "source_commit_object_present": data["source"]["commit_object_present"],
              "projects_unchanged": data["database"]["projects"] == previous["database"]["projects"],
              "workspaces_unchanged": data["database"]["workspaces"] == previous["database"]["workspaces"],
              "membership_unchanged": data["database"]["membership"] == previous["database"]["membership"],
              "code_and_recovery_refs_unchanged": data["code"] == previous["code"]}
    before = {row["id"]: row for row in previous["database"]["sessions"]}
    after = {row["id"]: row for row in data["database"]["sessions"]}
    checks["no_new_managed_segments"] = before.keys() == after.keys()
    checks["no_unexpected_live_checkout_processes"] = not any(
        row["pid_observed_only"] != str(os.getpid()) and not (row.get("command_name") or "").startswith("agent-term")
        for row in data["live_cwd_processes"])
    checks["original_session_native_ids_preserved"] = all(identity in after and
        all(after[identity][key] == row[key] for key in ["session_identity", "native_conversation", "agent", "history_state"])
        for identity, row in before.items())
    checks["original_committed_prefixes_preserved"] = all(identity in after and after[identity].get("prefix", {}).get("sha256") == row.get("prefix", {}).get("sha256")
        for identity, row in before.items() if row["history_state"] == "retained")
    if data["public"].get("available"):
        observation = data["public"]["observation"]
        checks["no_automatic_running_coding_observed"] = not any(row.get("lifecycle") == "Running" for row in observation.get("sessions", []))
    else:
        checks["no_automatic_running_coding_observed"] = None
    result = {"checkpoint": str(args.checkpoint), "checks": checks,
              "all_required_checks_passed": all(value is True for value in checks.values()),
              "unverified_checks": [key for key, value in checks.items() if value is None],
              "failed_checks": [key for key, value in checks.items() if value is False],
              "explicit_owner_start": started, "current": data,
              "scope": "actual macOS boot identity and committed-prefix/local recovery comparison; no sleep, reboot, coding launch, PID signalling, or native resume performed"}
    save(args.output, result)
    print(json.dumps({"verification": str(args.output), "checks": checks,
                      "all_required_checks_passed": result["all_required_checks_passed"]}))
    if not result["all_required_checks_passed"]:
        raise SystemExit(1)


if __name__ == "__main__":
    try:
        main()
    except Exception as error:
        # Keep an exact failed attempt without overwriting an earlier record.
        import sys
        if "--output" in sys.argv:
            failure_path = Path(sys.argv[sys.argv.index("--output")+1])
            if failure_path.parent.exists() and not failure_path.exists():
                save(failure_path, {"all_required_checks_passed": False,
                                    "failure": repr(error), "boot": boot(),
                                    "scope": "failed verifier attempt; no reboot/resume/PID signalling requested"})
        raise
