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
    assert not value["dirty"], ("source must be clean", value)
    if args.pin:
        expected = json.loads(args.pin.read_text())
        assert not expected.get("dirty"), "pinned source must be clean"
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


def parse_darwin_procargs2(payload):
    """Decode only argc, kernel executable path and exact NUL-separated argv."""
    import struct
    if len(payload) < 5:
        raise ValueError("truncated KERN_PROCARGS2 header")
    argc = struct.unpack_from("=i", payload)[0]
    if not 1 <= argc <= 4096:
        raise ValueError("invalid KERN_PROCARGS2 argc")
    end = payload.find(b"\0", 4)
    if end < 0 or end == 4:
        raise ValueError("missing KERN_PROCARGS2 executable")
    executable = payload[4:end]
    cursor = end + 1
    while cursor < len(payload) and payload[cursor] == 0:
        cursor += 1
    argv = []
    for _ in range(argc):
        end = payload.find(b"\0", cursor)
        if end < 0:
            raise ValueError("truncated KERN_PROCARGS2 argv")
        argv.append(payload[cursor:end])
        cursor = end + 1
    if not argv[0]:
        raise ValueError("missing KERN_PROCARGS2 argv[0]")
    return executable, argv


def darwin_editor_identity(pid):
    """Read a fresh foreground leader's executable/argv; never signal it."""
    import ctypes
    import sys
    if sys.platform != "darwin":
        raise RuntimeError("exact editor ownership requires Darwin process APIs")
    if not isinstance(pid, int) or pid <= 1:
        raise ValueError("invalid foreground process-group leader")
    proc = ctypes.CDLL("/usr/lib/libproc.dylib", use_errno=True)
    proc.proc_pidpath.argtypes = [ctypes.c_int, ctypes.c_void_p, ctypes.c_uint32]
    proc.proc_pidpath.restype = ctypes.c_int
    path = ctypes.create_string_buffer(4096)
    if proc.proc_pidpath(pid, path, len(path)) <= 0:
        raise OSError(ctypes.get_errno(), "proc_pidpath unavailable")
    executable = path.value
    libc = ctypes.CDLL(None, use_errno=True)
    libc.sysctl.argtypes = [ctypes.POINTER(ctypes.c_int), ctypes.c_uint,
                           ctypes.c_void_p, ctypes.POINTER(ctypes.c_size_t),
                           ctypes.c_void_p, ctypes.c_size_t]
    libc.sysctl.restype = ctypes.c_int
    mib = (ctypes.c_int * 3)(1, 49, pid)  # CTL_KERN, KERN_PROCARGS2, pid
    size = ctypes.c_size_t()
    if libc.sysctl(mib, 3, None, ctypes.byref(size), None, 0) != 0:
        raise OSError(ctypes.get_errno(), "KERN_PROCARGS2 size unavailable")
    if not 1 <= size.value <= 2 * 1024 * 1024:
        raise ValueError("KERN_PROCARGS2 exceeds explicit 2 MiB observation cap")
    data = ctypes.create_string_buffer(size.value)
    if libc.sysctl(mib, 3, data, ctypes.byref(size), None, 0) != 0:
        raise OSError(ctypes.get_errno(), "KERN_PROCARGS2 argv unavailable")
    kernel_executable, argv = parse_darwin_procargs2(data.raw[:size.value])
    # Recheck the executable after argv acquisition; unavailable/changed evidence
    # never authorizes subsequent editor keystrokes. Foreground pgrp is rechecked
    # by the caller. This is an observation, not an atomic lifetime guarantee.
    again = ctypes.create_string_buffer(4096)
    if proc.proc_pidpath(pid, again, len(again)) <= 0 or again.value != executable:
        raise RuntimeError("editor executable changed during ownership observation")
    return {"pid_observed_only": pid,
            "executable": executable.decode("utf-8", "backslashreplace"),
            "executable_bytes": list(executable),
            "kernel_executable_bytes": list(kernel_executable),
            "argv": [part.decode("utf-8", "backslashreplace") for part in argv],
            "argv_bytes": [list(part) for part in argv],
            "method": "Darwin proc_pidpath + KERN_PROCARGS2; environment not retained"}


def editor_identity_matches(identity, expected):
    """Exact resolved executable and exact raw argv operand, with no substrings."""
    try:
        executable = bytes(identity["executable_bytes"])
        kernel_executable = bytes(identity["kernel_executable_bytes"])
        expected_executable = os.fsencode(expected["executable"])
        operand = os.fsencode(expected["operand"])
        argv = [bytes(part) for part in identity["argv_bytes"]]
        return (bool(operand) and os.path.isabs(expected_executable)
                and os.path.isabs(executable) and os.path.isabs(kernel_executable)
                and os.path.realpath(executable) == os.path.realpath(expected_executable)
                and os.path.realpath(kernel_executable) == os.path.realpath(expected_executable)
                and operand in argv[1:])
    except (KeyError, TypeError, ValueError):
        return False


def drive(args):
    runner_sha256 = sha(Path(__file__))
    before = boundary(args)
    plan = json.loads(args.plan.read_text())
    rows, cols = plan.get("geometry", [40, 160])
    master, slave = pty.openpty()
    original = termios.tcgetattr(slave)
    fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", rows, cols, 0, 0))
    argv = [str(args.binary.resolve()), str(args.repo.resolve()),
            "--state-dir", str(args.state.resolve())]
    env = dict(os.environ, TERM="xterm-256color")
    if plan.get("dashboard_color"):
        env.pop("NO_COLOR", None)
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
        send(b"\x1d")
        drain(.2)
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
            if "expect_editor" in step:
                expected = step["expect_editor"]
                deadline = time.monotonic()+step.get("timeout", 8)
                last = None
                while True:
                    group = os.tcgetpgrp(master)
                    try:
                        identity = darwin_editor_identity(group) if group > 1 and group != child.pid else None
                        last = {"group": group, "identity": identity}
                        if (child.poll() is None and group == os.tcgetpgrp(master)
                                and editor_identity_matches(identity, expected)):
                            record["editor_ownership"] = {"foreground_pgrp": group,
                                                          "identity": identity,
                                                          "expected": expected,
                                                          "exact_executable_and_operand": True}
                            break
                    except (OSError, ValueError, RuntimeError) as error:
                        last = {"group": group, "identity_unavailable": repr(error)}
                    if time.monotonic() >= deadline:
                        raise TimeoutError("expected exact editor foreground/executable/operand unavailable: "+repr({"last": last, "expected": expected}))
                    drain(.03)
            if "expect_visible" in step:
                assert args.parser, "visible assertions require the pinned real terminal parser"
                deadline = time.monotonic() + step.get("timeout", 30)
                assertion_capture = folder / ("visible-assertion-"+str(number)+".raw")
                while True:
                    assertion_capture.write_bytes(raw)
                    subprocess.run([str(args.parser), str(assertion_capture), str(rows), str(cols)], check=True)
                    parsed = json.loads(assertion_capture.with_suffix(".cells.json").read_text())
                    if step["expect_visible"] in parsed["text"]:
                        record["visible_assertion"] = {"needle": step["expect_visible"], "cells": str(assertion_capture.with_suffix(".cells.json"))}
                        break
                    if time.monotonic() >= deadline:
                        raise TimeoutError("missing current visible cells: "+step["expect_visible"])
                    drain(.03)
            if "capture" in step:
                capture = folder / (step["capture"] + ".raw")
                capture.write_bytes(raw)
                if args.parser:
                    subprocess.run([str(args.parser), str(capture), str(rows), str(cols)], check=True)
                    record["cell_capture"] = str(capture.with_suffix(".cells.json"))
            try:
                record["foreground_pgrp"] = os.tcgetpgrp(master)
                record["termios_sha256"] = hashlib.sha256(repr(termios.tcgetattr(master)).encode()).hexdigest()
            except OSError as error:
                record["terminal_state_observation_error"] = repr(error)
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
        try:
            restored = termios.tcgetattr(slave) == original
            restoration_error = None
        except termios.error as error:
            restored = None
            restoration_error = repr(error)
        os.close(slave)
        os.close(master)
        (folder / "terminal.raw").write_bytes(raw)
        if failure and observation is None:
            try:
                observation = observe(args.state, timeout=3)
            except Exception as error:
                failure += "; post-failure observation unavailable: "+repr(error)
        after = boundary(args)
        result = {"runner_path": str(Path(__file__).resolve()), "runner_sha256": runner_sha256, "before": before, "after": after, "boundary_stable": before == after,
                  "argv": argv, "terminal_env": {key: env.get(key) for key in ["TERM", "NO_COLOR"]}, "initial_dashboard_focus_escape": True, "plan": plan, "steps": steps, "failure": failure,
                  "parser": {"path": str(args.parser), "sha256": sha(args.parser)} if args.parser else None,
                  "dashboard_pid": child.pid, "dashboard_exit": child.poll(),
                  "terminal_restored": restored, "restoration_observation_error": restoration_error, "observation": observation,
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
