"""Specification extraction and honest coverage accounting; no reference engine needed."""
import json
import re
import sys
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import rules


class RuleInventoryTests(unittest.TestCase):
    def test_local_rulebook_counts_and_examples(self):
        data = rules.inventory()
        self.assertEqual(len(data["rules"]), 3166)
        # a subrule after "z" takes two letters: 704.5aa follows 704.5z
        self.assertIn("start your engines!", data["rules"]["704.5aa"]["text"])
        self.assertEqual(sum(r.startswith("9") for r in data["rules"]), 131)
        self.assertIn("all objects", data["rules"]["800.4a"]["text"])
        self.assertTrue(data["rules"]["800.4a"]["examples"])
        lines = rules.RULES.read_text(encoding="utf-8-sig").splitlines()
        for rid, entry in data["rules"].items():
            self.assertTrue(lines[entry["line"] - 1].startswith(rid))
            self.assertGreaterEqual(entry["end_line"], entry["line"])

    def extract(self, body):
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "rules.txt"
            path.write_text("These rules are effective as of today.\n1. Game Concepts\n"
                            "Glossary\n1. Game Concepts\n100. General\n" + body + "\nGlossary\n100.9 not a rule\n")
            return rules.inventory(path)

    def test_continuations_examples_and_glossary_boundary(self):
        data = self.extract("100.1. First sentence.\n  Continued text.\nExample: A card.\n  Continued example.\n100.1a A subrule.")
        self.assertEqual(data["rules"]["100.1"]["text"], "First sentence.\nContinued text.")
        self.assertEqual(data["rules"]["100.1"]["examples"], ["A card.\nContinued example."])
        self.assertEqual(list(data["rules"]), ["100.1", "100.1a"])

    def test_duplicate_rule_identifiers_are_not_silently_overwritten(self):
        with self.assertRaisesRegex(ValueError, "duplicate rule"):
            self.extract("100.1. First.\n100.1. Second.")

    def test_test_references_never_award_credit_without_review(self):
        with tempfile.TemporaryDirectory() as tmp:
            data = rules.ledger(Path(tmp) / "absent.json")
        self.assertGreater(data["metrics"]["referenced_by_tests"], 0)
        self.assertEqual(data["metrics"]["verified"], 0)
        self.assertEqual(data["metrics"]["reviewed"], 0)

    def test_credit_needs_passing_current_evidence_and_completed_subrules(self):
        with tempfile.TemporaryDirectory() as tmp:
            overrides, results = Path(tmp) / "overrides.json", Path(tmp) / "run.json"
            overrides.write_text(json.dumps({"rules_sha256": rules.inventory()["source"]["sha256"], "rules": {
                "112.1": {"status": "implemented", "revision": "reviewed-commit", "rationale": "fixture",
                          "evidence_tests": ["r112_1_case"]},
                "115.5": {"status": "implemented", "revision": "reviewed-commit", "rationale": "fixture",
                          "evidence_tests": ["r115_5_case"]}}}))
            execution = {"suite_sha256": "current", "unchanged": True, "exit_code": 0,
                         "tests": {"r112_1_case": ["ok"], "r115_5_case": ["ok"]}}
            with patch.object(rules, "suite_hash", return_value="current"):
                results.write_text(json.dumps(execution))
                data = rules.ledger(overrides, results)
                self.assertFalse(data["rules"]["112.1"]["verified"], "unreviewed spell-copy subrules")
                self.assertTrue(data["rules"]["115.5"]["verified"])
                for bad in ("FAILED", "ignored"):
                    execution["tests"]["r115_5_case"] = [bad]
                    results.write_text(json.dumps(execution))
                    self.assertEqual(rules.ledger(overrides, results)["metrics"]["verified"], 0)
                execution["tests"]["r115_5_case"] = ["ok"]
                execution["suite_sha256"] = "old-source"
                results.write_text(json.dumps(execution))
                self.assertEqual(rules.ledger(overrides, results)["metrics"]["verified"], 0)

    def test_interactions_are_discoverable_without_leaking_to_the_next_test(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            path = root / "crates/game/tests/r603/interactions.rs"
            path.parent.mkdir(parents=True)
            path.write_text("// Rules: 603.2e, 603.6b, 614.1c, 603.2e\n"
                            "// Why: entry does not cause becoming tapped.\n"
                            "#[test]\nfn r603_2e_case() {}\n"
                            "#[test]\nfn r603_4_other() {}\n"
                            "// Rules: 400.7\n#[test] #[should_panic]\nfn identity_case() {}\n"
                            "// Rules: 101.1\nuse unrelated::Thing;\n#[test]\nfn no_metadata() {}\n")
            refs = rules.test_references(path)
            self.assertEqual([r["rules"] for r in refs], [["603.2e", "603.6b", "614.1c"], ["603.4"], ["400.7"]])
            with patch.object(rules, "ROOT", root):
                named, citations = rules.evidence()
                self.assertEqual(named["603.2e"], named["614.1c"])
                self.assertNotIn("101.1", named)
                self.assertIn("101.1", citations)
                self.assertEqual(rules.tests()["614.1c"], named["614.1c"])

    def test_cli_and_ledger_share_unit_binary_and_integration_evidence(self):
        named, _ = rules.evidence()
        self.assertEqual(rules.tests(), named)
        references = {ref["test"]: ref for refs in named.values() for ref in refs}
        cases = {
            "r614_5_applied_replacements_distinguish_grants_and_components": "-p game --lib",
            "r614_5_replacement_wire_identity_round_trips_a_granted_instance": "-p game --bin game-serve",
            "r614_5_a_replacement_cannot_apply_again_to_its_own_child_draws": "-p game --test rules",
        }
        for name, target in cases.items():
            with self.subTest(name=name):
                self.assertEqual(rules.test_command(references[name]), f"cargo test {target} {name}")
        # Source evidence from the other workspace packages must also be runnable.
        for refs in named.values():
            for ref in refs:
                if ref["path"].startswith("crates/grammar/src/") and "/bin/" not in ref["path"]:
                    self.assertEqual(rules.test_command(ref), f"cargo test -p grammar --lib {ref['test']}")
                    return
        self.fail("expected grammar unit-test evidence")

    def test_only_test_attributes_award_discovery_and_quoted_brackets_are_safe(self):
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "tests.rs"
            path.write_text('fn r702_19b_helper() {}\n'
                            '//! Rules: 101.1\n#[test] fn plain() {}\n'
                            '// Rules: 608.2b\n#[test]\n'
                            '#[should_panic(expected = "index [0]")]\nfn example() {}\n')
            refs = rules.test_references(path)
            self.assertEqual([(r["test"], r["rules"]) for r in refs], [("example", ["608.2b"])])

    def test_source_layout_has_one_engine_integration_target_and_no_orphan_modules(self):
        import tomllib
        root = rules.ROOT / "crates/game"
        manifest = tomllib.loads((root / "Cargo.toml").read_text())
        self.assertFalse(manifest["package"]["autotests"])
        self.assertEqual(manifest["test"], [{"name": "rules", "path": "tests/rules.rs"}])
        # Prevent a new unregistered .rs test from silently being skipped by autotests=false.
        included = set()
        def visit(path):
            included.add(path)
            text = path.read_text()
            # `test_modules::scenarios!()` / `engine!()` declare every .rs of their folder but
            # mod.rs (crates/test-modules/build.rs)
            for folder in re.findall(r"^test_modules::(\w+)!\(\);", text, re.M):
                self.assertEqual(path, root / "tests" / folder / "mod.rs")
                for child in sorted(path.parent.glob("*.rs")):
                    if child.name != "mod.rs":
                        visit(child)
            for name in re.findall(r"^(?:pub(?:\(\w+\))? )?mod (\w+);", text, re.M):
                parent = path.parent
                child = parent / f"{name}.rs"
                if not child.exists():
                    child = parent / name / "mod.rs"
                self.assertTrue(child.exists(), str(child))
                visit(child)
        visit(root / "tests/rules.rs")
        self.assertEqual(included, set((root / "tests").rglob("*.rs")))

    def test_changed_rulebook_requires_reassessment(self):
        with tempfile.TemporaryDirectory() as tmp:
            overrides = Path(tmp) / "overrides.json"
            overrides.write_text(json.dumps({"rules_sha256": "old", "rules": {"112.1": {"status": "partial"}}}))
            with self.assertRaisesRegex(ValueError, "rulebook hash differs"):
                rules.ledger(overrides)


class RulesCliTests(unittest.TestCase):
    def test_check_and_evidence_check_run_the_actual_checker_and_fail_on_findings(self):
        import contextlib
        import io
        import rule_evidence
        for flags in (['--check'], ['--evidence', '--check']):
            for problems, expected in (([], 0), (['unknown rule119.3a at fixture.rs:8'], 1)):
                output = io.StringIO()
                with self.subTest(flags=flags, problems=problems), contextlib.redirect_stdout(output), \
                        patch.object(rules, 'inventory', return_value={'rules': {'119.3': {}}}), \
                        patch.object(rule_evidence, 'discover', return_value={'sites': [1], 'problems': problems}) as discover, \
                        patch.object(rule_evidence, 'run', side_effect=AssertionError('check must not run Rust')):
                    self.assertEqual(rules.main(flags), expected)
                    discover.assert_called_once_with(known={'119.3'})
                self.assertIn(f'1 sites; {len(problems)} problems', output.getvalue())
                if problems:
                    self.assertIn(problems[0], output.getvalue())

    def test_invalid_flags_and_conflicting_modes_fail_before_scanning(self):
        import contextlib
        import io
        for flags in (['--chek'], ['--evidence', '--bogus'], ['--check', '701'],
                      ['--check', '--run'], ['--run'], ['--untested', '--report'],
                      ['--results', 'unused.json']):
            with self.subTest(flags=flags), contextlib.redirect_stderr(io.StringIO()), \
                    patch.object(rules, 'inventory', side_effect=AssertionError('must reject before scanning')), \
                    self.assertRaises(SystemExit) as error:
                rules.main(flags)
            self.assertEqual(error.exception.code, 2)


if __name__ == "__main__":
    unittest.main()
