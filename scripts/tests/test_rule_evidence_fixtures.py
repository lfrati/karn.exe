"""Regressions of the `cite!` tooling itself (crates/game/tests/support/cite.rs), through the
real compiler and the real rules target, with the fixtures of crates/game/tests/tooling/.

A cited expectation must be an assertion: a bare condition or result given to `cite!` would be
credited as passing whatever its value, so neither compiles. A receipt that can't be written,
even while a failing assertion unwinds, fails the test normally (no abort, which would lose
every other test's outcome), says so, and leaves the run without complete evidence.
These fixtures are tooling, not rules evidence: the scanner skips their folder."""
import os
import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import rule_evidence as ev
import rules

ROOT = Path(__file__).resolve().parents[2]


class CompileFailTests(unittest.TestCase):
    def test_a_bare_condition_or_result_is_not_an_expectation(self):
        # only the rules crate gets the cfg; a target dir of its own keeps it apart from other builds
        env = {**os.environ, "CARGO_TARGET_DIR": str(ROOT / "target" / "cite-compile-fail")}
        result = subprocess.run(["cargo", "rustc", "-q", "-p", "game", "--test", "rules", "--profile", "check",
                                 "--", "--cfg", "cite_compile_fail"], cwd=ROOT, env=env, capture_output=True, text=True)
        self.assertNotEqual(result.returncode, 0)
        errors = result.stderr
        self.assertEqual(errors.count("error[E0308]: mismatched types"), 2, errors)
        self.assertIn("expected `bool`, found `()`", errors)
        self.assertIn("expected `Result<(), String>`, found `()`", errors)


class ReceiptFailureTests(unittest.TestCase):
    def run_fixture(self, fixture: str) -> tuple[dict, subprocess.CompletedProcess]:
        """rule_evidence.run as it runs, filtered to one fixture, its receipts under a regular
        file (ENOTDIR), its run record and log in a temporary cache (under the root, as run()
        records the receipts path relative to it)."""
        (ROOT / "target").mkdir(exist_ok=True)
        cache = Path(tempfile.mkdtemp(dir=ROOT / "target", prefix="cite-fixture-"))
        self.addCleanup(shutil.rmtree, cache, True)
        blocker = cache / "not-a-directory"
        blocker.write_text("")
        real, seen = subprocess.run, []

        def filtered(cmd, **kw):
            if cmd[:2] == ["cargo", "test"]:
                cut = cmd.index("--") + 1
                cmd = cmd[:cut] + [f"tooling::cite_fixtures::{fixture}", "--exact"] + cmd[cut:]
                kw["env"] = {**kw["env"], "RULE_RECEIPTS": str(blocker / "receipts.jsonl"), "CITE_FIXTURE": fixture}
                seen.append(real(cmd, **kw))
                return seen[-1]
            return real(cmd, **kw)
        with patch.object(ev, "CACHE", cache), patch.object(ev, "RUN", cache / "run.json"), \
                patch.object(ev.subprocess, "run", filtered):
            data = ev.run(rules.suite_hash, rules.revision)
        return data, seen[0]

    def check(self, fixture: str):
        data, result = self.run_fixture(fixture)
        output = result.stdout + result.stderr
        self.assertNotIn("SIGABRT", output, "an abort loses every other test's outcome")
        self.assertNotIn("non-unwinding panic", output)
        # the child's own output explains a missing outcome, such as a build error
        self.assertEqual(data["tests"], {f"tooling::cite_fixtures::{fixture}": "FAILED"}, output[-4000:])
        self.assertIn(ev.FAILED_RECEIPTS, output)
        self.assertIn("Not a directory", output)
        self.assertFalse(data["complete"], "receipts that weren't written leave the run incomplete")
        self.assertFalse(ev.join({"sites": [], "uncited": [], "problems": []}, data, data["suite_sha256"])["fresh"])

    def test_an_unwritable_receipt_while_a_cited_assertion_fails(self):
        self.check("receipt_failure")

    def test_an_unwritable_receipt_of_a_passing_assertion(self):
        self.check("receipt_unwritable_pass")


ATTRIBUTION = '''use crate::support::*;
#[test] #[should_panic] fn one_line() { cite!(["120.3a"]; assert!(false)); }
#[test]
fn any_error() { cite!(["601.2c"]; refused(""): t.act()); }
macro_rules! dead { ($o:expr) => { cite!(["704.5g"]; assert!(gone($o))) } }
#[test]
fn wrapped() { dead!(o); }
#[test]
fn debug_only() { cite!(["119.3"]; debug_assert!(false)); }
#[test]
fn invalid() {
    cite!(["712.114"]; assert!(true));
    for r in ["999.99", "not a rule"] { cite!([r]; case: r; assert!(true)); }
}
'''


class AttributionTests(unittest.TestCase):
    """Attribution that would credit a wrong rule or a check that can't fail."""

    def setUp(self):
        self.dir = tempfile.TemporaryDirectory()
        self.addCleanup(self.dir.cleanup)
        root = Path(self.dir.name)
        for p in (patch.object(ev, "ROOT", root), patch.object(ev, "TESTS", root / "crates/game/tests"),
                  patch.object(ev, "FIXTURES", root / "crates/game/tests/tooling")):
            p.start()
            self.addCleanup(p.stop)
        path = root / "crates/game/tests/r601/attribution.rs"
        path.parent.mkdir(parents=True)
        path.write_text(ATTRIBUTION)
        self.found = ev.discover(known={"120.3a", "601.2c", "704.5g", "119.3"})
        self.problems = "\n".join(self.found["problems"])

    def test_a_one_line_should_panic_test_is_rejected(self):
        self.assertIn("attribution.rs:2:", self.problems)
        # and its attributes are its own, not those of the test written below it
        self.assertNotIn("attribution.rs:4: attributed expectation in a #[should_panic] test", self.problems)

    def test_an_empty_refusal_reason_is_rejected(self):
        # it is contained in every error, so any refusal would pass
        line = [p for p in self.found["problems"] if "attribution.rs:4:" in p]
        self.assertTrue(line and not any("should_panic" in p for p in line), self.problems)

    def test_a_cite_inside_a_macro_is_rejected(self):
        # its receipt is located at the macro's use (line 7), never at the site found (line 5)
        self.assertIn("attribution.rs:5:", self.problems)

    def test_a_debug_assertion_is_rejected(self):
        # with debug assertions off (the fast profile inherits release) it checks nothing
        self.assertIn("attribution.rs:9:", self.problems)

    def test_rule_ids_outside_the_pinned_rulebook_are_never_evidence(self):
        sites = {s["line"]: s for s in self.found["sites"]}
        data = {"run": "r1", "target": "game/rules", "revision": {"commit": "c", "worktree": False}, "suite_sha256": "h",
                "unchanged": True, "complete": True, "exit_code": 0, "tests": {"r601::attribution::invalid": "ok"},
                "receipts": "x", "tooling_sha256": ev.tooling_hash()}
        rows = [{"run": "r1", "file": sites[line]["file"], "line": line, "column": sites[line]["column"],
                 "test": "r601::attribution::invalid", "rules": rules_, "case": case, "outcome": "passed"}
                for line, rules_, case in [(12, ["712.114"], None), (13, ["999.99"], "999.99"), (13, ["not a rule"], "not a rule")]]
        with patch.object(ev, "receipts_of", lambda d: rows):
            joined = ev.join(self.found, data, "h")
        self.assertTrue(joined["fresh"], "a current run, so the rows are judged, not dropped as stale")
        for rid in ("712.114", "999.99", "not a rule"):
            self.assertNotEqual(joined["status"].get(rid), "passing", rid)
            self.assertTrue(any(repr(rid) in p for p in joined["problems"] + self.found["problems"]), rid)


class ProvenanceTests(unittest.TestCase):
    """A run is evidence only of the tooling that recorded it: the scanner decides which
    receipt matches which site, so changing it after a run must leave the run stale."""

    def test_editing_the_evidence_tooling_makes_a_recorded_run_stale(self):
        (ROOT / "target").mkdir(exist_ok=True)
        cache = Path(tempfile.mkdtemp(dir=ROOT / "target", prefix="cite-provenance-"))
        self.addCleanup(shutil.rmtree, cache, True)
        real = subprocess.run

        def fake(cmd, **kw):
            if cmd[:2] == ["cargo", "test"]:
                return subprocess.CompletedProcess(cmd, 0, "test r601::x::y ... ok\n\ntest result: ok. 1 passed\n", "")
            return real(cmd, **kw)
        with patch.object(ev, "CACHE", cache), patch.object(ev, "RUN", cache / "run.json"), \
                patch.object(ev.subprocess, "run", fake):
            data = ev.run(rules.suite_hash, rules.revision)
        found = {"sites": [], "uncited": [], "problems": []}
        self.assertTrue(ev.join(found, data, rules.suite_hash())["fresh"], "unchanged, the run is current")
        for tool in ("scripts/rule_evidence.py", "scripts/rules.py"):
            path = ROOT / tool
            source = path.read_text()
            try:
                path.write_text(source + "\n# a changed tool\n")
                self.assertFalse(ev.join(found, data, rules.suite_hash())["fresh"], tool)
            finally:
                path.write_text(source)


class LegacyReportTests(unittest.TestCase):
    def test_no_recorded_test_run_is_no_legacy_zero(self):
        import progress_dashboard as dashboard
        rule = {'total': 3165, **dashboard.rule_test_coverage({'rules': {}})}
        value = {'cards': {'total': 10, 'parsed': 5, 'supported': 4}, 'rules': rule,
                 'decks': {'overall': {'decks': 0, 'percent': None}, 'by_format': {}}, 'measurement_current': True}
        legacy = next(line for line in dashboard.render({'dashboard': value}).splitlines()
                      if 'Legacy: rules named by passing tests' in line)
        self.assertNotIn('0.00%', legacy, "nothing ran, so nothing was measured")
        self.assertNotIn('(0/', legacy)


if __name__ == "__main__":
    unittest.main()
