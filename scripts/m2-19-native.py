#!/usr/bin/env python3
"""Explicit ticket-19 native keyboard proof driver (no builds or credential edits).

init creates three disposable Git repositories. drive executes a JSON keyboard
plan in one real outer PTY, saving raw bytes and the exact actions. It detaches
by default. observe reads the supervisor's public protocol after the dashboard
has released control. Raw captures are NOT parsed-cell screenshots and do not
establish semantic status; use observation.status and actual native evidence.
No physical reboot, sleep, inferred-PID signalling, or automatic native approval.
"""
import argparse
import fcntl
import hashlib
import json
import os
from pathlib import Path
import pty
import select
import shutil
import socket
import struct
import subprocess
import termios
import time
import uuid


def sha(path):
    with open(path, "rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def run(*args):
    return subprocess.check_output(args, stderr=subprocess.STDOUT).decode().strip()


def save(path, value):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, ensure_ascii=False, indent=2) + "\n")


def boundary(args):
    source = run("git", "-C", str(args.source), "rev-parse", "HEAD")
    value = {"source": source, "binary": str(args.binary.resolve()),
             "binary_sha256": sha(args.binary),
             "dirty": run("git", "-C", str(args.source), "status", "--porcelain")}
    if args.pin:
        expected = json.loads(args.pin.read_text())
        assert value["source"] == expected["source"], ("source changed", value)
        assert value["binary_sha256"] == expected["binary_sha256"], ("binary changed", value)
    return value


def observe(state, timeout=10):
    deadline = time.monotonic() + timeout
    while True:
        sock = socket.socket(socket.AF_UNIX)
        sock.settimeout(5)
        try:
            sock.connect(str(state / "supervisor.sock"))
            reader = sock.makefile("rb")
            greeting = json.loads(reader.readline())
            if not greeting.get("control_denied"):
                sock.sendall(b'"Screen"\n')
                result = json.loads(reader.readline())
                reader.close()
                return result
        except (OSError, ValueError):
            if time.monotonic() >= deadline:
                raise
        finally:
            sock.close()
        if time.monotonic() >= deadline:
            raise TimeoutError("dashboard has not released supervisor control")
        time.sleep(.05)


def process_evidence(observation):
    """Read-only attribution. These observed PIDs never grant signal authority."""
    evidence = []
    for item in [observation, *observation.get("sessions", [])]:
        execution = item.get("execution") or {}
        pid = execution.get("pid")
        if not isinstance(pid, int):
            continue
        evidence.append({"pid": pid, "target": item.get("target"),
                         "ps": subprocess.run(["ps", "-p", str(pid), "-o",
                                                "pid,ppid,pgid,lstart,command"],
                                               capture_output=True, text=True).stdout,
                         "cwd": subprocess.run(["lsof", "-a", "-p", str(pid),
                                                 "-d", "cwd", "-Fn"],
                                                capture_output=True, text=True).stdout})
    return evidence


def drive(args):
    before = boundary(args)
    plan = json.loads(args.plan.read_text())
    rows, cols = plan.get("geometry", [40, 160])
    master, slave = pty.openpty()
    original = termios.tcgetattr(slave)
    fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", rows, cols, 0, 0))
    argv = [str(args.binary.resolve()), str(args.repo.resolve()),
            "--state-dir", str(args.state.resolve())]
    env = dict(os.environ)
    if "editor" in plan:
        env["RAT_EDITOR_JSON"] = json.dumps(plan["editor"])
    def claim_terminal():
        os.setsid()
        fcntl.ioctl(slave, termios.TIOCSCTTY, 0)
        os.tcsetpgrp(slave, os.getpgrp())

    child = subprocess.Popen(argv, stdin=slave, stdout=slave, stderr=slave,
                             preexec_fn=claim_terminal, env=env)
    raw = bytearray()
    steps = []
    label = args.label or str(uuid.uuid4())
    folder = args.output / label
    folder.mkdir(parents=True, exist_ok=False)
    start = time.monotonic()

    def drain(seconds):
        deadline = time.monotonic() + seconds
        while time.monotonic() < deadline:
            if select.select([master], [], [], min(.02, max(0, deadline-time.monotonic())))[0]:
                try:
                    data = os.read(master, 65536)
                    if not data:
                        return
                    raw.extend(data)
                    if len(raw) > 64 * 1024 * 1024:
                        raise RuntimeError("outer PTY capture exceeds explicit 64 MiB cap")
                except OSError:
                    return

    def send(data):
        offset = 0
        while offset < len(data):
            offset += os.write(master, data[offset:])
            drain(.005)

    failure = None
    observation = None
    try:
        drain(plan.get("startup_seconds", 2))
        for number, step in enumerate(plan["steps"]):
            record = {"step": number, "action": step,
                      "start_ms": (time.monotonic()-start)*1000,
                      "raw_before": len(raw), "geometry": [rows, cols]}
            if "text" in step:
                # Dashboard text fields take real keys; pace them to avoid
                # exceeding the documented bounded raw-key command transport.
                for character in step["text"]:
                    send(character.encode())
                    drain(step.get("key_seconds", .015))
            if "hex" in step:
                send(bytes.fromhex(step["hex"]))
            if "paste" in step:
                send(b"\x1b[200~" + step["paste"].encode() + b"\x1b[201~")
            if "resize" in step:
                rows, cols = step["resize"]
                fcntl.ioctl(master, termios.TIOCSWINSZ,
                            struct.pack("HHHH", rows, cols, 0, 0))
            drain(step.get("wait", .5))
            if "expect_raw" in step:
                deadline = time.monotonic() + step.get("timeout", 15)
                needle = step["expect_raw"].encode()
                while needle not in raw[record["raw_before"]:]:
                    if time.monotonic() >= deadline:
                        raise TimeoutError("missing fresh raw byte subsequence: " + step["expect_raw"])
                    drain(.05)
                # This is explicitly a byte check, not a visible-cell assertion.
                record["raw_subsequence_seen"] = True
            if "capture" in step:
                capture = folder / (step["capture"] + ".raw")
                capture.write_bytes(raw)
                if args.parser:
                    subprocess.run([str(args.parser), str(capture), str(rows), str(cols)], check=True)
                    record["cell_capture"] = str(capture.with_suffix(".cells.json"))
            record["raw_after"] = len(raw)
            record["end_ms"] = (time.monotonic()-start)*1000
            steps.append(record)
        ending = plan.get("ending", "detach")
        if ending == "loss":
            # This is the current live Popen child, never a saved/inferred PID.
            child.kill()
        elif ending == "detach":
            send(b"\x1d")
            drain(.2)
            send(b"d")
        elif ending == "quit":
            send(b"\x1d")
            drain(.2)
            send(b"q")
        elif ending != "already-exited":
            raise ValueError("unsupported ending")
        deadline = time.monotonic() + plan.get("exit_timeout", 30)
        while child.poll() is None:
            drain(.05)
            if time.monotonic() >= deadline:
                raise TimeoutError("dashboard did not exit; owned child will be killed, native execution retained")
        drain(.1)
        if ending != "quit":
            observation = observe(args.state)
    except Exception as error:
        failure = repr(error)
    finally:
        if child.poll() is None:
            child.kill()
            deadline = time.monotonic() + 5
            while child.poll() is None and time.monotonic() < deadline:
                drain(.02)
        restored = termios.tcgetattr(slave) == original
        os.close(slave)
        os.close(master)
        (folder / "terminal.raw").write_bytes(raw)
        after = boundary(args)
        result = {"before": before, "after": after, "boundary_stable": before == after,
                  "argv": argv, "plan": plan, "steps": steps, "failure": failure,
                  "parser": {"path": str(args.parser), "sha256": sha(args.parser)} if args.parser else None,
                  "dashboard_pid": child.pid, "dashboard_exit": child.poll(),
                  "terminal_restored": restored, "observation": observation,
                  "processes": process_evidence(observation) if observation else []}
        save(folder / "result.json", result)
        print(json.dumps({"evidence": str(folder), "failure": failure,
                          "boundary_stable": before == after,
                          "terminal_restored": restored}))
    if failure:
        raise SystemExit(1)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("action", choices=["init", "pin", "drive", "observe"])
    parser.add_argument("--source", type=Path, default=Path(__file__).resolve().parent.parent)
    parser.add_argument("--binary", type=Path)
    parser.add_argument("--root", type=Path, required=True)
    parser.add_argument("--repo", type=Path)
    parser.add_argument("--plan", type=Path)
    parser.add_argument("--pin", type=Path)
    parser.add_argument("--label")
    parser.add_argument("--parser", type=Path, help="Prebuilt vt100 raw-to-cell parser; never compiled by this driver")
    args = parser.parse_args()
    args.binary = args.binary or args.source / "target/debug/agent-terminal"
    args.state = args.root / "state"
    args.output = args.root / "evidence"
    if args.action == "init":
        args.root.mkdir(parents=True, exist_ok=False)
        args.state.mkdir(mode=0o700)
        projects = []
        for name in ["amber", "violet", "cobalt"]:
            repo = args.root / name
            run("git", "init", "-b", "main", str(repo))
            run("git", "-C", str(repo), "config", "user.name", "M2 qualification")
            run("git", "-C", str(repo), "config", "user.email", "proof@example.invalid")
            (repo / "proof.txt").write_text("initial tracked content\n")
            run("git", "-C", str(repo), "add", "proof.txt")
            run("git", "-C", str(repo), "commit", "-qm", "Qualification base")
            projects.append({"name": name, "path": str(repo.resolve()),
                             "head": run("git", "-C", str(repo), "rev-parse", "HEAD")})
        save(args.root / "projects.json", projects)
    elif args.action == "pin":
        value = boundary(args)
        value["versions"] = {name: run(shutil.which(name) or name, "--version")
                             for name in ["claude", "codex", "git"]}
        value["macos"] = run("sw_vers")
        value["architecture"] = run("uname", "-m")
        save(args.root / "pin.json", value)
    elif args.action == "observe":
        value = observe(args.state)
        save(args.output / ((args.label or str(uuid.uuid4())) + ".json"), value)
        print(json.dumps(value, ensure_ascii=False))
    else:
        if not args.repo or not args.plan:
            parser.error("drive requires --repo and --plan")
        drive(args)


if __name__ == "__main__":
    main()
