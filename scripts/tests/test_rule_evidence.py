"""Expectation-level rule evidence: discovery from source and the join with a run's receipts."""
import sys
import io
import tempfile
import unittest
from pathlib import Path
from types import SimpleNamespace
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import rule_evidence as ev

SOURCE = '''use crate::support::*;
fn helper(t: &T) {
    cite!(["704.5g"]; assert!(t.ok()));
}
// a comment with cite!(["999.9"]; nonsense) is not a site
#[test]
fn first() {
    helper(&t);
    let s = "cite!([\\"123.4\\"]; assert!(x))";
    cite!(
        ["601.2d", "603.3d"];
        assert_eq!(a, b, "{}", [1, 2].len())
    );
    if never {
        cite!(["601.2c"]; refused("it takes"): t.act());
    }
    for (rule, why) in [("601.2d", "x")] {
        cite!([rule]; case: why; refused(why): t.act());
    }
    t.expect().pt("A", 1, 1)
        .rules(["302.6"]).tapped("A", true)
        .rules(["613.4c"]).pt("B", 2, 2).check();
    assert!(uncited);
}
#[test]
fn second() { helper(&t); }
#[test]
#[should_panic]
fn panics() { cite!(["120.3a"]; assert!(false)); }
#[test]
fn wrong() {
    cite!(["712.114"]; assert!(x));
    cite!(["601.2d"]; t.g.obj(b).damage == 99);
}
'''


class RuleEvidenceTests(unittest.TestCase):
    def setUp(self):
        self.dir = tempfile.TemporaryDirectory()
        root = Path(self.dir.name)
        self.patches = [patch.object(ev, "ROOT", root), patch.object(ev, "TESTS", root / "crates/game/tests"),
                        patch.object(ev, "FIXTURES", root / "crates/game/tests/tooling")]
        for p in self.patches:
            p.start()
        path = root / "crates/game/tests/r601/sample.rs"
        path.parent.mkdir(parents=True)
        path.write_text(SOURCE)
        fixture = root / "crates/game/tests/tooling/fixture.rs"
        fixture.parent.mkdir(parents=True)
        fixture.write_text('#[test]\nfn f() { cite!(["000.0"]; assert!(true)); }\n')
        self.found = ev.discover(known={"704.5g", "601.2d", "603.3d", "601.2c", "302.6", "613.4c", "120.3a"})
        self.at = {s["line"]: s for s in self.found["sites"] if s["kind"] != "uncited-step"}

    def tearDown(self):
        for p in self.patches:
            p.stop()
        self.dir.cleanup()

    def test_sites_are_found_in_code_only_and_across_lines(self):
        self.assertEqual(sorted(self.at), [3, 10, 15, 18, 21, 22, 29, 32, 33])
        self.assertEqual(self.at[10]["rules"], ["601.2d", "603.3d"])
        self.assertEqual((self.at[15]["kind"], self.at[18]["kind"]), ("refused", "refused"))
        self.assertTrue(all("tooling" not in s["file"] for s in self.found["sites"]), "fixtures are no evidence")

    def test_helpers_rows_and_builder_steps(self):
        self.assertEqual((self.at[3]["function"], self.at[3]["in_test"]), ("r601::sample::helper", False))
        self.assertEqual((self.at[18]["rules"], self.at[18]["row_supplied"], self.at[18]["case"]), ([], ["rule"], True))
        self.assertEqual((self.at[21]["step"], self.at[21]["rules"]), ("tapped", ["302.6"]))
        self.assertEqual((self.at[22]["step"], self.at[22]["rules"]), ("pt", ["613.4c"]))
        uncited = [s for s in self.found["sites"] if s["kind"] == "uncited-step"]
        self.assertEqual([(s["line"], s["step"]) for s in uncited], [(20, "pt")])
        self.assertEqual([u["line"] for u in self.found["uncited"]], [23])

    def test_invalid_attribution_is_a_problem(self):
        problems = "\n".join(self.found["problems"])
        self.assertIn("sample.rs:29: attributed expectation in a #[should_panic] test", problems)
        self.assertIn("sample.rs:32: 712.114 is not in the pinned rulebook", problems)
        self.assertIn("sample.rs:33: cite! of neither an assertion", problems)
        self.assertEqual(len(self.found["problems"]), 3)

    def test_refusal_guard_checks_operands_in_helpers_inline_tests_and_cited_asserts(self):
        path = ev.ROOT / "crates/other/src/lib.rs"
        path.parent.mkdir(parents=True)
        path.write_text('''fn helper() {
    assert!(action().is_err());
    cite!(["404.3"]; assert!(action().is_err()));
    assert_eq!(true, action().is_err(), "not a reason");
    debug_assert_ne! {
        action()
            .is_err(), false
    }
    assert !(action().is_err());
    assert_eq !(true, action::<u32, Vec<u64>>().is_err());
    assert!(action::<fn() -> Result<u32, u64>, ()>().is_err());
}
#[cfg(test)] mod tests {
    #[test] fn rejects() { assert![action().is_err()]; }
}
''')
        problems = ev.refusal_assertion_problems()
        self.assertEqual([p.split(":")[1] for p in problems], ["2", "3", "4", "5", "9", "10", "11", "14"])
        self.assertTrue(all("crates/other/src/lib.rs:" in p and "authenticating the error" in p
                            for p in problems))
        self.assertEqual(ev.discover(known=set())["problems"][:8], problems,
                         "the existing rules.py --check path must receive every refusal problem")

    def test_refusal_guard_ignores_nonasserting_uses_and_accepts_checked_reasons(self):
        path = ev.ROOT / "crates/game/tests/refusal_controls.rs"
        path.write_text(r'''fn controls() {
    // assert!(action().is_err());
    /* outer /* assert!(action().is_err()); */ comment */
    let text = r#"assert!(action().is_err());"#;
    let quoted = "assert!(action().is_err());";
    if action().is_err() { rollback(); }
    assert!(valid, "diagnostic {}", unrelated().is_err());
    assert_eq!(actual, expected, "diagnostic {}", unrelated().is_err());
    assert!(left < right, "diagnostic {}", unrelated().is_err());
    assert_eq!(actual::<u32, Vec<u64>>(), expected, "diagnostic {}", unrelated().is_err());
    assert_eq!(action().unwrap_err(), "wrong player");
    let reason = action().expect_err("must refuse");
    assert_eq!(reason, "wrong player");
    cite!(["117.1"]; refused("wrong player"): action());
}
''')
        self.assertEqual(ev.refusal_assertion_problems([path]), [])
        self.assertEqual(ev.discover(paths=[path], known={"117.1"})["problems"], [])
        path.write_text("fn helper() {\n    assert!(action().is_err());\n}\n")
        import rules
        with patch.object(ev, "TESTS", path.parent / "empty"), patch("sys.stdout", new_callable=io.StringIO) as output:
            self.assertEqual(rules.evidence_main(SimpleNamespace(check=True)), 1)
        self.assertIn("refusal_controls.rs:2: assertion uses is_err()", output.getvalue())
        self.assertIn("0 sites; 1 problems", output.getvalue())

    def test_a_rule_number_may_end_in_two_letters(self):
        # 704.5aa follows 704.5z in the pinned rulebook
        self.assertTrue(ev.RULE_ID.fullmatch("704.5aa"))
        self.assertTrue(ev.RULE_ID.fullmatch("704.5g") and ev.RULE_ID.fullmatch("704.5"))
        self.assertFalse(ev.RULE_ID.fullmatch("704.5aaa") or ev.RULE_ID.fullmatch("704"))

    def joined(self, receipts, tests, **run):
        data = {"run": "r1", "target": "game/rules", "revision": {"commit": "c", "worktree": False}, "suite_sha256": "h",
                "unchanged": True, "complete": True, "exit_code": 0, "tests": tests, "receipts": "x",
                "tooling_sha256": ev.tooling_hash(), **run}
        rows = [{"run": "r1", "file": "crates/game/tests/r601/sample.rs", "column": self.at[line]["column"], "line": line,
                 "case": None, **r} for line, r in receipts]
        with patch.object(ev, "receipts_of", lambda d: rows):
            return ev.join(self.found, data, "h")

    def test_outcomes_are_the_expectations_and_their_tests(self):
        j = self.joined([
            (3, {"test": "r601::sample::first", "rules": ["704.5g"], "outcome": "passed"}),
            (3, {"test": "r601::sample::second", "rules": ["704.5g"], "outcome": "passed"}),
            (10, {"test": "r601::sample::first", "rules": ["601.2d", "603.3d"], "outcome": "passed"}),
            (18, {"test": "r601::sample::first", "rules": ["601.2d"], "outcome": "failed", "case": "x"}),
            (21, {"test": None, "rules": ["302.6"], "outcome": "passed"}),
        ], {"r601::sample::first": "FAILED", "r601::sample::second": "ok"})
        by = {(r["line"], r["test"]): r["outcome"] for rows in j["rules"].values() for r in rows}
        self.assertEqual(by[(3, "r601::sample::second")], "passing")
        self.assertEqual(by[(3, "r601::sample::first")], "diagnostic", "it completed, its test failed later")
        self.assertEqual(by[(18, "r601::sample::first")], "failing")
        self.assertEqual(by[(15, "r601::sample::first")], "not run", "the branch was not taken")
        self.assertEqual(j["status"]["704.5g"], "passing")
        self.assertEqual(j["status"]["601.2d"], "failing")
        self.assertEqual(j["status"]["601.2c"], "not run")
        self.assertEqual(j["status"]["603.3d"], "diagnostic", "completed only in a failing test")
        self.assertEqual(len(j["unattributed"]), 1, "a spawned thread names no test")
        self.assertEqual(j["status"]["302.6"], "not run")

    def test_a_stale_or_incomplete_run_credits_nothing(self):
        for run in ({"suite_sha256": "other"}, {"unchanged": False}, {"complete": False}):
            j = self.joined([(3, {"test": "r601::sample::second", "rules": ["704.5g"], "outcome": "passed"})],
                            {"r601::sample::second": "ok"}, **run)
            self.assertFalse(j["fresh"], run)
            self.assertEqual(j["status"]["704.5g"], "not run", run)

    def test_the_progress_measure_keeps_static_counts_and_names_an_unusable_run(self):
        run = {"run": "r1abcdef", "target": "game/rules", "revision": {"commit": "c0ffee", "worktree": ""}, "suite_sha256": "h",
               "unchanged": True, "complete": True, "exit_code": 0, "tests": {"r601::sample::second": "ok"}, "receipts": "x",
               "tooling_sha256": ev.tooling_hash()}
        rows = [{"run": "r1abcdef", "file": "crates/game/tests/r601/sample.rs", "line": 3, "column": self.at[3]["column"],
                 "test": "r601::sample::second", "rules": ["704.5g"], "outcome": "passed", "case": None}]
        with patch.object(ev, "receipts_of", lambda d: rows):
            current = ev.measure(self.found, run, "h", 100)
            stale = ev.measure(self.found, run, "changed", 100)
            incomplete = ev.measure(self.found, {**run, "complete": False}, "h", 100)
        missing = ev.measure(self.found, None, "h", 100)
        for m in (current, stale, incomplete, missing):
            self.assertEqual((m["rules_with_sites"], m["rules_without_sites"], m["row_supplied_sites"]), (8, 92, 1))
        self.assertEqual(current["execution"]["passing"], 1)
        self.assertEqual(current["execution"]["not_run"], 6, "712.114 is no rule, not one not run")
        for m, state in ((stale, "STALE"), (incomplete, "INCOMPLETE"), (missing, "MISSING")):
            self.assertEqual((m["run_state"], m["execution"]), (state, None))
        text = "\n".join(ev.render(stale))
        self.assertIn("Executed: UNAVAILABLE, last run r1abcdef at c0ffee is STALE", text)
        self.assertNotIn("passing 0", text)
        self.assertIn("PROBLEM", text, "invalid attribution stays visible")
        self.assertIn("passing 1, failing 0", "\n".join(ev.render(current)))

    def test_a_run_reads_should_panic_outcomes_and_needs_its_command_to_finish(self):
        out = ("test r601::a::x ... ok\ntest r601::a::p - should panic ... ok\ntest r601::a::q - should panic ... FAILED\n"
               "\ntest result: FAILED. 2 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out\n")
        for stdout, complete in ((out, True), (out.split("\ntest result")[0], False),
                                 (out + ev.FAILED_RECEIPTS + " r.jsonl: Not a directory\n", False)):
            with patch.object(ev, "CACHE", Path(self.dir.name) / ".cache"), patch.object(ev, "RUN", Path(self.dir.name) / "run.json"), \
                    patch.object(ev.subprocess, "run", lambda cmd, **kw: ev.subprocess.CompletedProcess(cmd, 101, stdout, "")):
                data = ev.run(lambda: "h", lambda: {"commit": "c", "worktree": ""})
            self.assertEqual(data["tests"], {"r601::a::x": "ok", "r601::a::p": "ok", "r601::a::q": "FAILED"} if complete else data["tests"])
            self.assertEqual(data["complete"], complete, stdout[-60:])


if __name__ == "__main__":
    unittest.main()
