"""Native lifecycle regressions; run with python3 tests/native_supervisor.py."""
import importlib.util
from pathlib import Path
import sys
import tempfile
import unittest

spec = importlib.util.spec_from_file_location(
    "native_acceptance", Path(__file__).resolve().parents[1] / "scripts/native-macos-acceptance.py")
native = importlib.util.module_from_spec(spec)
spec.loader.exec_module(native)


@unittest.skipUnless(sys.platform == "darwin", "Darwin process identity boundary")
class SupervisorLifecycle(unittest.TestCase):
    def test_child_reaped_between_snapshot_and_poll_is_a_natural_exit(self):
        class FinishAfterSnapshot(native.NativeFullCommand):
            forced_boundary = False

            @classmethod
            def _observe(cls, process, root, known):
                root, remaining = super()._observe(process, root, known)
                if remaining and not cls.forced_boundary:
                    # Real child and parent exit after this snapshot, before
                    # run() polls. Return the genuine, now older observation.
                    process.wait(timeout=5)
                    cls.forced_boundary = True
                return root, remaining

        with tempfile.TemporaryDirectory() as directory:
            runner = FinishAfterSnapshot(Path(directory) / "out", directory)
            runner.run("reaped", [sys.executable, "-c",
                       "import subprocess,sys; subprocess.run([sys.executable, '-c', "
                       "'import time; time.sleep(0.5)'], check=True)"],
                       10, expect_json=False)
            self.assertTrue(FinishAfterSnapshot.forced_boundary)
            self.assertTrue(runner.last_receipt["natural_exit"])
            self.assertFalse(runner.last_receipt["forced_cleanup"])
            self.assertEqual(runner.last_receipt["owned_processes_remaining_after_cleanup"], [])

    def test_live_orphan_still_fails_and_is_cleaned_up(self):
        with tempfile.TemporaryDirectory() as directory:
            runner = native.NativeFullCommand(Path(directory) / "out", directory)
            with self.assertRaises(native.NativeFullCommandError):
                runner.run("orphan", [sys.executable, "-c",
                           "import subprocess,sys,time; subprocess.Popen([sys.executable, '-c', "
                           "'import time; time.sleep(60)']); time.sleep(0.5)"],
                           10, expect_json=False)
            receipt = runner.last_receipt
            self.assertIn("descendants alive", receipt["failure"])
            self.assertTrue(receipt["forced_cleanup"])
            self.assertFalse(receipt["natural_exit"])
            self.assertEqual(receipt["owned_processes_remaining_after_cleanup"], [])


if __name__ == "__main__":
    unittest.main()
