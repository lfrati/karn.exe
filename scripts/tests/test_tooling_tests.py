"""The parallel tooling runner reports as one unittest run: every count, every failure."""
from pathlib import Path
import re
import sys
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import tooling_tests
import progress

PASSING = '''import unittest
class T(unittest.TestCase):
    def test_a(self): pass
    @unittest.skip("later")
    def test_b(self): pass
'''
FAILING = '''import unittest
class T(unittest.TestCase):
    def test_c(self): self.fail("wrong")
    def test_d(self): raise ValueError("broken")
    @unittest.expectedFailure
    def test_e(self): self.fail("known")
'''
BROKEN = "import no_such_module\n"  # unittest itself reports an import error as one error
DEAD = "import os\nos._exit(3)\n"  # nothing reported at all
NESTED = '''import unittest
class T(unittest.TestCase):
    def test_f(self):
        print("Ran 99 tests in 0.001s\\n\\nOK")  # a nested run's footer, in the test's own output
        self.fail("after it")
'''


class ToolingRunnerTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.tests = Path(self.temp.name)
        for name, source in [("test_pass", PASSING), ("test_fail", FAILING), ("test_broken", BROKEN), ("test_dead", DEAD),
                             ("test_nested", NESTED)]:
            (self.tests / f"{name}.py").write_text(source)

    def run_modules(self, *modules, durations=5):
        with patch.object(tooling_tests, "TESTS", self.tests):
            return [tooling_tests.run_module(m, durations) for m in modules]

    def test_each_module_reports_its_counts_and_a_crash_is_no_result(self):
        passing, failing, broken, dead = self.run_modules("test_pass", "test_fail", "test_broken", "test_dead")
        self.assertEqual((passing["tests"], passing["ok"], passing["exit_code"]), (2, True, 0))
        self.assertEqual(passing["counts"]["skipped"], 1)
        self.assertEqual((failing["tests"], failing["ok"]), (3, False))
        self.assertEqual({k: v for k, v in failing["counts"].items() if v}, {"failures": 1, "errors": 1, "expected failures": 1})
        self.assertIn("FAIL: test_c (test_fail.T.test_c)", failing["body"])
        self.assertIn("ERROR: test_d (test_fail.T.test_d)", failing["body"])
        self.assertNotIn("Ran 3 tests", failing["body"], "the module's own footer is replaced by the run's")
        self.assertNotIn("Slowest test durations", failing["body"])
        self.assertEqual(len(failing["slowest"]), 3, "per-test durations are kept for the run's list")
        self.assertEqual((broken["tests"], broken["ok"], broken["counts"]["errors"]), (1, False, 1))
        self.assertIn("no_such_module", broken["body"])
        self.assertEqual((dead["tests"], dead["exit_code"], dead["body"]), (None, 3, ""))

    def test_a_footer_printed_by_a_test_is_not_the_modules_result(self):
        nested, = self.run_modules("test_nested")
        self.assertEqual((nested["tests"], nested["ok"], nested["counts"]["failures"]), (1, False, 1))
        self.assertIn("Ran 99 tests", nested["body"], "the test's output stays")

    def test_the_footer_is_one_unittest_summary_the_report_reads(self):
        results = self.run_modules("test_pass", "test_fail", "test_broken", "test_dead")
        text = tooling_tests.footer(results, 1.5, durations=2)
        self.assertRegex(text, r"(?m)^Ran 7 tests in 1\.500s$", "a module without a result counts as one error")
        self.assertRegex(text, r"(?m)^FAILED \(failures=1, errors=3, skipped=1, expected failures=1\)$")
        self.assertEqual(len(re.findall(r"^\d+\.\d+s\s+test_", text, re.M)), 2, "the slowest tests, limited")
        ok = tooling_tests.footer(results[:1], 0.2, durations=0)
        self.assertRegex(ok, r"(?m)^Ran 2 tests in 0\.200s$")
        self.assertRegex(ok, r"(?m)^OK \(skipped=1\)$")
        self.assertNotIn("Slowest", ok)
        # what make progress reads from the log: the same counts
        parsed = progress.check_result("tooling", 1, text)
        self.assertEqual(parsed["counts"], {"executed": 7, "failed": 4, "skipped": 1, "expected_failures": 1, "passed": 1})
        self.assertFalse(parsed["passed"])
        self.assertTrue(progress.check_result("tooling", 0, ok)["passed"])

    def test_the_whole_run_lists_every_failure_including_a_module_without_a_result(self):
        with patch.object(tooling_tests, "TESTS", self.tests), patch.object(tooling_tests, "SERIAL", ("test_fail",)), \
                patch.object(sys, "argv", ["tooling_tests.py", "--workers", "2", "--durations", "3",
                                           "test_pass", "test_broken", "test_dead", "test_fail"]):
            with tempfile.TemporaryFile("w+") as out, patch.object(sys, "stdout", out):
                code = tooling_tests.main()
                out.seek(0)
                text = out.read()
        self.assertEqual(code, 1)
        self.assertRegex(text, r"(?m)^Ran 7 tests in \d+\.\d+s$")
        self.assertRegex(text, r"(?m)^FAILED \(failures=1, errors=3, skipped=1, expected failures=1\)$")
        failures = progress.check_result("tooling", code, text)["failures"]
        self.assertEqual(set(failures), {"test_c (test_fail.T.test_c)", "test_d (test_fail.T.test_d)",
                                         "test_broken (unittest.loader._FailedTest.test_broken)", "test_dead (test_dead.import)"})
        self.assertLess(text.index("== test_dead"), text.index("== test_fail"), "serial modules run last")
        self.assertEqual(len(re.findall(r"(?m)^Ran \d+ tests", text)), 1, "one summary, not one per module")

    def test_unknown_modules_are_refused_before_anything_runs(self):
        with patch.object(tooling_tests, "TESTS", self.tests), patch.object(sys, "argv", ["tooling_tests.py", "test_nothing"]):
            with self.assertRaises(SystemExit), tempfile.TemporaryFile("w+") as err, patch.object(sys, "stderr", err):
                tooling_tests.main()


if __name__ == "__main__":
    unittest.main()
