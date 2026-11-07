"""Run Linux PTY tests with a parent that reaps orphaned fixture descendants."""

import ctypes
import os
import signal
import sys
import time


def signal_children(signum):
    # Only this single-threaded process reaps its children. Between reading
    # this list and signalling, an exited child remains an unreaped zombie,
    # so its PID cannot be reused for an unrelated process.
    # Some cloud kernels omit /proc/.../children; stat's PPID is available.
    with os.scandir("/proc") as processes:
        for process in processes:
            if not process.name.isdecimal():
                continue
            try:
                with open(f"{process.path}/stat") as stat:
                    fields = stat.read().rsplit(")", 1)[1].split()
            except (FileNotFoundError, ProcessLookupError, PermissionError):
                continue
            if int(fields[1]) == os.getpid():
                try:
                    os.kill(int(process.name), signum)
                except ProcessLookupError:
                    pass


def main():
    if len(sys.argv) < 2:
        raise SystemExit("Usage: run-with-reaper.py COMMAND [ARG ...]")
    if sys.platform != "linux":
        os.execvp(sys.argv[1], sys.argv[1:])

    libc = ctypes.CDLL(None, use_errno=True)
    # PR_SET_CHILD_SUBREAPER: adopt this command's orphaned descendants only.
    if libc.prctl(36, 1, 0, 0, 0) != 0:
        raise OSError(ctypes.get_errno(), "Cannot enable fixture child reaping")

    cancelled = None

    def cancel(signum, _frame):
        nonlocal cancelled
        if cancelled is None:
            cancelled = signum

    signal.signal(signal.SIGCHLD, signal.SIG_DFL)
    signal.signal(signal.SIGINT, cancel)
    signal.signal(signal.SIGTERM, cancel)
    command = os.fork()
    if command == 0:
        signal.signal(signal.SIGINT, signal.SIG_DFL)
        signal.signal(signal.SIGTERM, signal.SIG_DFL)
        try:
            os.execvp(sys.argv[1], sys.argv[1:])
        except OSError as error:
            print(error, file=sys.stderr, flush=True)
            os._exit(127)
    result = None
    command_finished = None
    shutdown_started = None
    last_error = None
    while True:
        try:
            child, status = os.waitpid(-1, os.WNOHANG)
        except ChildProcessError:
            return 128 + cancelled if cancelled else result
        if child:
            if child == command:
                result = os.waitstatus_to_exitcode(status)
                if result < 0:
                    result = 128 - result
                command_finished = time.monotonic()
            continue

        now = time.monotonic()
        overdue = command_finished is not None and now - command_finished >= 5
        if shutdown_started is None and (cancelled or overdue):
            shutdown_started = now
            if overdue:
                result = result or 1
            print("Settling owned fixture processes before runner exit", file=sys.stderr)
        if shutdown_started is not None:
            # Killing a parent adopts its remaining descendants here. Repeat
            # until waitpid confirms every child has been reaped, including
            # detached sessions and descendants that resist TERM.
            signum = signal.SIGTERM if now - shutdown_started < 0.5 else signal.SIGKILL
            try:
                signal_children(signum)
                last_error = None
            except OSError as error:
                # Retain reaping ownership if cleanup cannot yet be confirmed.
                if str(error) != last_error:
                    print(f"Fixture cleanup unconfirmed: {error}", file=sys.stderr)
                last_error = str(error)
        time.sleep(0.01)


if __name__ == "__main__":
    sys.exit(main())
