"""Build entry points must propagate failing checks instead of reporting success."""
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[2]


@unittest.skipUnless(shutil.which("make"), "make is required")
class MakeChecks(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        shutil.copyfile(ROOT / "Makefile", self.root / "Makefile")
        self.bin = self.root / "bin"
        self.bin.mkdir()
        self.log = self.root / "commands"
        self.env = {**os.environ, "PATH": f"{self.bin}{os.pathsep}{os.environ['PATH']}",
                    "CHECK_LOG": str(self.log)}
        self.stub("cargo", 'echo "cargo $*" >> "$CHECK_LOG"\nexit "${CARGO_STATUS:-0}"\n')
        self.stub("python-check", 'echo "python $*" >> "$CHECK_LOG"\n'
                  'case "$*" in *rules.py*) exit "${RULES_STATUS:-0}";; *tooling_tests.py*) exit "${TOOLING_STATUS:-0}";; esac\n')

    def stub(self, name, body):
        path = self.bin / name
        path.write_text("#!/bin/sh\n" + body)
        path.chmod(0o755)
        return path

    def run_make(self, *args):
        return subprocess.run(["make", "--no-print-directory", *args], cwd=self.root,
                              env=self.env, text=True, capture_output=True)

    def test_check_runs_assertion_enabled_tests_before_clients_and_replays(self):
        result = self.run_make("-j4", "check", f"PYTHON={self.bin / 'python-check'}")
        self.assertEqual(result.returncode, 0, result.stderr)
        commands = self.log.read_text().splitlines()
        self.assertEqual(commands[1], "python scripts/rules.py --check")
        self.assertEqual(commands[2], "cargo test --locked --workspace")
        self.assertIn("--bin game-serve", commands[3])
        self.assertIn("tooling_tests.py", commands[4])
        self.assertEqual(len(commands), 5)

    def test_mgn_suite_builds_engine_before_running_the_records(self):
        modules = self.root / "scripts/tests"
        modules.mkdir(parents=True)
        for name in ("test_mgn_runner", "test_mgn_recorder", "test_unrelated_tool"):
            (modules / (name + ".py")).touch()
        result = self.run_make("test-mgn", f"PYTHON={self.bin / 'python-check'}")
        self.assertEqual(result.returncode, 0, result.stderr)
        commands = self.log.read_text().splitlines()
        self.assertIn("scripts/rust_tools.py build", commands[0])
        self.assertIn("test_mgn_runner", commands[1])
        self.assertIn("test_mgn_recorder", commands[1])
        self.assertNotIn("test_unrelated_tool", commands[1])

    def test_citation_failure_stops_before_building_or_running_other_checks(self):
        self.env["RULES_STATUS"] = "1"
        result = self.run_make("check", f"PYTHON={self.bin / 'python-check'}")
        self.assertNotEqual(result.returncode, 0)
        commands = self.log.read_text()
        self.assertIn("scripts/rules.py --check", commands)
        self.assertNotIn("cargo", commands)
        self.assertNotIn("tooling_tests.py", commands)

    def test_profile_defaults_to_the_explicit_mgn_workload(self):
        result = self.run_make("profile", f"PYTHON={self.bin / 'python-check'}")
        self.assertEqual(result.returncode, 0, result.stderr)
        commands = self.log.read_text()
        self.assertIn("perf_replay.py tests/fixtures/performance/bant-ordered-mgn.jsonl", commands)
        self.assertIn("--engine target/profile/release/game-serve", commands)

    def test_rust_failure_stops_before_tooling(self):
        self.env["CARGO_STATUS"] = "7"
        result = self.run_make("check", f"PYTHON={self.bin / 'python-check'}")
        self.assertNotEqual(result.returncode, 0)
        self.assertNotIn("tooling_tests.py", self.log.read_text())

    def test_tooling_failure_fails_the_check(self):
        self.env["TOOLING_STATUS"] = "9"
        result = self.run_make("check", f"PYTHON={self.bin / 'python-check'}")
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("tooling_tests.py", self.log.read_text())



if __name__ == "__main__":
    unittest.main()
