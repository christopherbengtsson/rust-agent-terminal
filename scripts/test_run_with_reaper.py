from pathlib import Path
import ctypes
import os
import signal
import subprocess
import sys
import tempfile
import time
import unittest


@unittest.skipUnless(sys.platform == "linux", "Linux subreaper behavior")
class ReaperTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        # Own leaked synthetic children too, so a failing regression test can
        # clean them without handing zombies to container init.
        libc = ctypes.CDLL(None, use_errno=True)
        if libc.prctl(36, 1, 0, 0, 0) != 0:
            raise OSError(ctypes.get_errno(), "Cannot own regression fixtures")

    def exercise_shutdown(self, cancellation=None, command_status=0):
        runner = Path(__file__).with_name("run-with-reaper.py")
        with tempfile.TemporaryDirectory() as directory:
            marker = Path(directory) / "pids"
            code = """
import os, pathlib, signal, sys, time
parent = os.getpid()
marker = pathlib.Path(sys.argv[1])
if os.fork() == 0:
    os.setsid()
    signal.signal(signal.SIGTERM, signal.SIG_IGN)
    marker.write_text(str(parent) + ' ' + str(os.getpid()))
    time.sleep(60)
    os._exit(0)
while not marker.exists():
    time.sleep(0.01)
if sys.argv[2] == 'exit':
    os._exit(int(sys.argv[3]))
time.sleep(60)
"""
            process = subprocess.Popen(
                [sys.executable, str(runner), sys.executable, "-c", code,
                 str(marker), "wait" if cancellation else "exit", str(command_status)],
                start_new_session=True, stdout=subprocess.DEVNULL,
                stderr=subprocess.DEVNULL,
            )
            unrelated = subprocess.Popen(
                [sys.executable, "-c", "import time; time.sleep(60)"]
            )
            pids = []
            try:
                deadline = time.monotonic() + 5
                while not marker.exists() or not marker.read_text().strip():
                    self.assertLess(time.monotonic(), deadline, "fixture did not start")
                    time.sleep(0.01)
                pids = [int(value) for value in marker.read_text().split()]
                if cancellation:
                    # Signal only the runner: it must settle detached fixtures.
                    process.send_signal(cancellation)
                expected = 128 + cancellation if cancellation else command_status or 1
                self.assertEqual(process.wait(timeout=10), expected)
                self.assertIsNone(unrelated.poll(), "unrelated process was signalled")
                for pid in pids:
                    self.assertFalse(Path(f"/proc/{pid}").exists(),
                                     f"fixture {pid} still running or unreaped")
            finally:
                unrelated.terminate()
                unrelated.wait()
                # Reap the command and its known adopted children on failures.
                if process.poll() is None:
                    process.kill()
                process.wait()
                for pid in pids:
                    try:
                        child, _ = os.waitpid(pid, os.WNOHANG)
                        if child == 0:
                            os.kill(pid, signal.SIGKILL)
                            os.waitpid(pid, 0)
                    except ChildProcessError:
                        pass

    def test_cancellation_reaps_detached_term_resistant_fixture(self):
        for cancellation in (signal.SIGINT, signal.SIGTERM):
            with self.subTest(signal=cancellation):
                self.exercise_shutdown(cancellation=cancellation)

    def test_cleanup_timeout_reaps_fixture_and_preserves_command_failure(self):
        for status in (0, 17):
            with self.subTest(status=status):
                self.exercise_shutdown(command_status=status)

    def test_adopts_and_reaps_orphan_while_preserving_command_status(self):
        runner = Path(__file__).with_name("run-with-reaper.py")
        with tempfile.TemporaryDirectory() as directory:
            marker = Path(directory) / "parent"
            for status in (0, 17):
                code = """
import os, sys, time
parent = os.getpid()
if os.fork() == 0:
    while os.getppid() == parent:
        time.sleep(0.01)
    with open(sys.argv[1], 'w') as file:
        file.write(str(os.getppid()))
    os._exit(0)
os._exit(int(sys.argv[2]))
"""
                process = subprocess.Popen(
                    [sys.executable, str(runner), sys.executable, "-c", code,
                     str(marker), str(status)]
                )
                self.assertEqual(process.wait(timeout=10), status)
                self.assertEqual(int(marker.read_text()), process.pid)
                marker.unlink()


if __name__ == "__main__":
    unittest.main()
