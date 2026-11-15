"""Ghidra script failures must not promote stale or partial candidates."""
import importlib.util
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location("lift", Path(__file__).resolve().parents[1] / "lift.py")
lift = importlib.util.module_from_spec(spec)
spec.loader.exec_module(lift)


class GhidraOutputs(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        (self.root / "tools/jdk-25-test").mkdir(parents=True)
        self.out = self.root / "out"
        self.out.mkdir()
        self.addresses = self.root / "addresses.txt"
        self.addresses.write_text("00B40F28\n00B49860\n")
        (self.out / "lifted.c").write_text("previous candidate")
        (self.out / "report.txt").write_text("previous report")

    def run_lift(self, fake):
        with patch.object(lift, "ROOT", self.root), patch.object(lift.subprocess, "run", side_effect=fake):
            lift.ghidra(self.addresses, self.out)

    def output(self, command, report):
        at = command.index("-postScript")
        Path(command[at + 3]).write_text("new candidate")
        Path(command[at + 4]).write_text(report)
        return subprocess.CompletedProcess(command, 0, "completed\n", "")

    def test_script_error_with_zero_exit_preserves_previous_candidate(self):
        def failed(command, **kwargs):
            return subprocess.CompletedProcess(command, 0, "ERROR executable SHA-256 mismatch\n", "")
        with self.assertRaisesRegex(RuntimeError, "did not complete"):
            self.run_lift(failed)
        self.assertEqual((self.out / "lifted.c").read_text(), "previous candidate")
        self.assertIn("SHA-256 mismatch", (self.out / "ghidra.log").read_text())

    def test_partial_report_is_not_promoted(self):
        with self.assertRaisesRegex(RuntimeError, "did not complete"):
            self.run_lift(lambda command, **kwargs: self.output(command, "00B40F28 lifted\n"))
        self.assertEqual((self.out / "report.txt").read_text(), "previous report")
        self.assertEqual((self.out / "lifted.c").read_text(), "previous candidate")

    def test_complete_report_promotes_the_new_candidate(self):
        self.run_lift(lambda command, **kwargs: self.output(command, "00B40F28 lifted\n00B49860 skip unsupported\n"))
        self.assertEqual((self.out / "lifted.c").read_text(), "new candidate")
        self.assertIn("00B49860 skip unsupported", (self.out / "report.txt").read_text())


if __name__ == "__main__":
    unittest.main()
