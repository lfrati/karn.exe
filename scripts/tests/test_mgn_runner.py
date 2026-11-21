"""The MGN runner on Karn (docs/mgn/verification.md): a clean record passes every level and its
report validates; a divergence is reported at the transition where it appears with expected and
actual values, and the run goes on; a boundary the engine never reaches is an L2 divergence; an
outcome the engine never asked for and an accepted illegal attempt fail L1; a refused action stops
the run; every active record of the inventory replays clean and is vetted over all its transitions."""
from copy import deepcopy
import json
import os
from pathlib import Path
import subprocess
import sys
import unittest
from tempfile import TemporaryDirectory
from unittest.mock import patch

import mgn.runner as runner
from mgn.identity import record_identity

from mgn.canonical import loads
from mgn.oracle import Oracle
from mgn.reviews import query
from mgn.runner import LEVELS, Run, compare, inventory_records, karn_adapter_command
from mgn.schema import validate as schema_validate
from mgn.validate import validate

ROOT = Path(__file__).resolve().parents[2]
ENGINE = Path(os.environ.get("KARN_CONFORMANCE_ENGINE", ROOT / "target/fast/game-serve"))
FIXTURES = ROOT / "tests/fixtures/mgn"
ORACLE = Oracle()


def record(name: str) -> dict:
    return loads((FIXTURES / name / "record.json").read_text(encoding="utf-8"))


def run(rec: dict) -> dict:
    errors = [str(p) for p in validate(rec, ORACLE) if p.severity == "error"]
    assert not errors, errors  # every altered record stays valid: the runner, not the validator, is under test
    report = Run(rec, ORACLE, karn_adapter_command(ENGINE)).run()
    assert [str(e) for e in schema_validate(report, "divergence-report.schema.json")] == []
    return report


class SparseProjectionTests(unittest.TestCase):
    def test_foretell_adds_tier_two_facts_to_an_existing_card_without_a_row(self):
        # Spec 3.4 omits default rows; 702.143a makes the exiled card face down
        # and foretold, both tier 2 under spec 3.2, without creating a token/copy.
        expected = {"zones": {"exile": ["doom"]}, "objects": {}}
        actual = {"zones": {"exile": ["doom"]}, "objects": {"doom": {"face_down": True, "designations": ["foretold"]}}}
        diffs, inconclusive = compare(expected, actual, [], 2)
        self.assertEqual([(d["level"], d["path"]) for d in diffs],
                         [("L4", "objects.doom.designations"), ("L4", "objects.doom.face_down")])
        self.assertEqual(inconclusive, [])
        self.assertEqual(compare(actual, expected, [], 2)[0][0]["level"], "L4")

    def test_missing_physical_card_controller_fact_still_fails_tier_one(self):
        expected = {"zones": {"battlefield": ["bear"]}, "objects": {}}
        actual = {"zones": {"battlefield": ["bear"]}, "objects": {"bear": {"controller": "B"}}}
        self.assertEqual(compare(expected, actual, [], 2)[0],
                         [{"level": "L3", "path": "objects.bear.controller", "expected": None, "actual": "B"}])

    def test_generated_identity_requires_its_row_even_when_both_zones_list_it(self):
        for object_id, facts in [("T1", {"token": {"name": "Bear Token"}, "controller": "A"}),
                                 ("S1", {"kind": "copy", "copy_of": "spell", "controller": "A"}),
                                 ("E1", {"emblem": {"text": "Example"}, "controller": "A"})]:
            expected = {"zones": {"battlefield": [object_id]}, "objects": {}}
            actual = {"zones": {"battlefield": [object_id]}, "objects": {object_id: facts}}
            diffs, _ = compare(expected, actual, [], 2)
            self.assertEqual([(d["level"], d["path"]) for d in diffs], [("L3", f"objects.{object_id}")])

    def test_a_new_object_with_tier_two_facts_still_has_a_tier_one_presence_difference(self):
        diffs, _ = compare({"objects": {}, "zones": {"exile": []}},
                           {"objects": {"doom": {"face_down": True}}, "zones": {"exile": ["doom"]}}, [], 2)
        self.assertEqual([d["level"] for d in diffs], ["L3", "L3"])


@unittest.skipUnless(ENGINE.is_file(), "build target/fast/game-serve to replay MGN records")
class RunnerTests(unittest.TestCase):
    def test_a_clean_record_passes_every_level(self):
        report = run(record("scenario-announcements"))  # X announced (107.3a), a mode chosen (700.2), damage divided (601.2d)
        self.assertEqual({level: report["levels"][level] for level in LEVELS},
                         {"L0": "pass", "L1": "pass", "L2": "pass", "L3": "pass", "L4": "pass", "L5": "not_applicable"})
        self.assertEqual(report["replayed"], report["transitions"])
        self.assertIsNone(report["stopped_at"])
        self.assertEqual(report["divergences"], [])
        self.assertEqual(report["engine"]["name"], "karn")
        # the facts the engine does not model are declared once, not defaulted (verification.md)
        self.assertIn("game.initiative", report["unsupported"])

    def test_a_wrong_life_total_is_an_l3_divergence_and_the_run_goes_on(self):
        rec = record("scenario-turn-history/land-played-v2")
        rec["transitions"][0]["delta"]["players"] = {"A": {"life": 17}}  # nothing in the stretch changes life
        report = run(rec)
        self.assertEqual(report["levels"]["L3"], "fail")
        self.assertEqual(report["levels"]["L2"], "pass")
        first = [d for d in report["divergences"] if d["first"] and d["level"] == "L3"]
        self.assertEqual(first, [{"after": 1, "level": "L3", "path": "players.A.life", "expected": 17, "actual": 20, "first": True}])
        self.assertEqual(report["replayed"], 2)  # the second decision was still played
        self.assertIsNone(report["stopped_at"])

    def test_a_boundary_the_engine_never_reaches_is_an_l2_divergence(self):
        rec = record("scenario-announcements")
        last = rec["transitions"][-1]["delta"]
        # the record claims the next question is a block declaration by the active seat in its own
        # declare attackers step, which the rules never ask (509.1a): no state agrees on `pending`
        last["turn"] = {"number": 3, "step": "declare_attackers", "active": "A"}
        last["pending"] = {"seat": "A", "kind": "declare_blockers"}
        report = run(rec)
        self.assertEqual(report["levels"]["L2"], "fail")
        paths = {d["path"] for d in report["divergences"] if d["level"] == "L2"}
        self.assertIn("pending", paths)
        self.assertEqual(report["stopped_at"], 3)

    def test_an_outcome_the_engine_never_asked_for_fails_l1(self):
        rec = record("scenario-announcements")
        rec["transitions"][0]["random"] = [{"kind": "shuffle", "seat": "A", "order": ["drawA"]}]  # nothing shuffles when a creature spell resolves
        report = run(rec)
        self.assertEqual(report["levels"]["L1"], "fail")
        self.assertEqual([(d["after"], d["path"], d["actual"]) for d in report["divergences"] if d["level"] == "L1"],
                         [(1, "random", "never asked for")])
        self.assertEqual(report["levels"]["L3"], "pass")
        self.assertEqual(report["replayed"], 3)

    def test_an_accepted_attempt_is_reported_and_the_boundary_restored(self):
        rec = record("scenario-announcements")
        legal = deepcopy(rec["transitions"][0]["do"])
        rec["transitions"].insert(0, {"seat": "A", "attempt": legal, "illegal": True})  # the record is wrong: this cast is legal
        report = run(rec)
        self.assertEqual(report["levels"]["L1"], "fail")
        self.assertEqual([d for d in report["divergences"] if d["level"] == "L1"],
                         [{"after": 1, "level": "L1", "path": "attempt", "expected": "refused", "actual": "accepted", "first": True}])
        # the adapter replayed its journal into a fresh engine: the real decisions then played out
        self.assertEqual(report["levels"]["L3"], "pass")
        self.assertEqual(report["replayed"], 4)
        self.assertIsNone(report["stopped_at"])

    def test_a_refused_action_stops_the_run_with_an_l1_refusal(self):
        rec = record("scenario-turn-history/land-played-v2")
        second = {"seat": "A", "do": {"kind": "play_land", "card": "second"}, "delta": {"objects": {"second": {"zone": "battlefield"}}}}
        rec["transitions"].insert(1, second)  # a second land in one turn (305.2)
        report = run(rec)
        self.assertEqual(report["levels"]["L1"], "fail")
        self.assertEqual([(r["after"], r["class"], r["code"]) for r in report["refusals"]], [(2, "illegal", "illegal_action")])
        self.assertEqual(report["stopped_at"], 2)
        self.assertEqual(report["replayed"], 1)

    def test_every_active_record_replays_clean(self):
        # Archival records retain their old bytes and may intentionally diverge.
        # Ordinary fixtures must still pass every applicable level; inconclusive
        # starts and newly exposed failures are never accepted as active successes.
        for name, path in inventory_records(active=True):
            with self.subTest(record=name):
                report = run(loads(path.read_text()))
                self.assertTrue(all(v in ('pass', 'not_applicable')
                                    for v in report['levels'].values()), report)
                self.assertIsNone(report['stopped_at'])
                self.assertEqual(report['replayed'], report['transitions'])

    def test_script_entrypoint_replays_and_writes_a_clean_report(self):
        # Exercise the documented executable entrypoint, not only the imported
        # module: both must compare the same rule-authored causal scenario.
        with TemporaryDirectory() as directory:
            result = subprocess.run(
                [sys.executable, str(ROOT / "scripts/mgn/runner.py"),
                 str(FIXTURES / "causal-growth-expiration/record.json"),
                 "--engine", str(ENGINE), "--out", directory],
                cwd=ROOT, capture_output=True, text=True, timeout=30)
            self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
            report = loads((Path(directory) / "causal-growth-expiration/report.json").read_text())
            self.assertTrue(all(value in ("pass", "not_applicable")
                                for value in report["levels"].values()), report)
            self.assertEqual(report["replayed"], 1)
            self.assertIsNone(report["stopped_at"])


class InventorySelectionTests(unittest.TestCase):
    def test_active_selection_excludes_archives_and_refused_sources(self):
        with TemporaryDirectory() as directory:
            manifest = Path(directory) / 'inventory.json'
            manifest.write_text(json.dumps({'records': [
                {'record': 'nested/current/record.json', 'disposition': 'active'},
                {'record': 'old/record.json', 'disposition': 'archived', 'reason': 'superseded'},
                {'source': 'all-implied', 'refused': 'no decisions', 'disposition': 'active'},
            ]}))
            self.assertEqual(inventory_records(manifest, active=True),
                             [('nested/current', manifest.parent / 'nested/current/record.json')])
            self.assertEqual([name for name, _ in inventory_records(manifest)],
                             ['nested/current', 'old'])

    def test_archival_successor_links_authenticate_immutable_predecessors(self):
        items = json.loads((FIXTURES / 'inventory.json').read_text())['records']
        by_path = {item['record']: item for item in items if 'record' in item}
        self.assertEqual(len(by_path), sum('record' in item for item in items))
        for name, item in by_path.items():
            with self.subTest(record=name):
                self.assertIn(item['disposition'], ('active', 'archived'))
                self.assertTrue((FIXTURES / name).is_file())
                if item['disposition'] == 'archived':
                    self.assertTrue(item.get('reason'))
                if 'superseded_by' in item:
                    self.assertEqual(item['disposition'], 'archived')
                    self.assertIn(item['superseded_by'], by_path)
                    successor = loads((FIXTURES / item['superseded_by']).read_text())
                    predecessor = loads((FIXTURES / name).read_text())
                    self.assertEqual(successor['lineage']['supersedes'], record_identity(predecessor))

    def test_every_active_record_is_vetted_over_all_its_transitions(self):
        # Spec section 9 admits a record when `vetted` entries for its exact identity
        # cover every transition at its tier; a converter's or replay's entry claims no
        # semantic review, so a record without one must be archived instead.
        for name, path in inventory_records(active=True):
            with self.subTest(record=name):
                rec = loads(path.read_text(encoding='utf-8'))
                log = path.parent / 'review.jsonl'
                self.assertTrue(log.is_file())
                covered = set()
                for match in query([log], record=record_identity(rec), verdicts=['vetted']):
                    entry = match['entry']
                    if rec['tier'] not in entry['tiers']:
                        continue
                    if entry['scope'] == 'all':
                        covered = set(range(1, len(rec['transitions']) + 1))
                        break
                    low, high = entry['scope']['transitions']
                    covered |= set(range(low, high + 1))
                self.assertLessEqual(set(range(1, len(rec['transitions']) + 1)), covered)

    def test_active_cli_writes_nested_reports_and_all_is_mutually_exclusive(self):
        with TemporaryDirectory() as directory:
            root = Path(directory)
            fixture = root / 'tests/fixtures/mgn'
            fixture.mkdir(parents=True)
            (fixture / 'inventory.json').write_text(json.dumps({'records': [
                {'record': 'nested/current/record.json', 'disposition': 'active'},
                {'record': 'old/record.json', 'disposition': 'archived'},
            ]}))
            report = {'levels': {level: 'pass' for level in LEVELS}, 'replayed': 1,
                      'transitions': 1, 'stopped_at': None, 'divergences': [],
                      'inconclusive': [], 'refusals': []}
            with patch.object(runner, 'ROOT', root), patch.object(runner, 'Oracle'), \
                    patch.object(runner, 'run_record', return_value=report) as replay, \
                    patch('builtins.print'):
                self.assertEqual(runner.main(['--active', '--out', str(root / 'reports')]), 0)
                self.assertEqual(replay.call_args.args[0], fixture / 'nested/current/record.json')
                self.assertEqual(replay.call_count, 1)
                self.assertTrue((root / 'reports/nested/current/report.json').is_file())
                replay.reset_mock()
                self.assertEqual(runner.main(['--all']), 0)
                self.assertEqual(replay.call_count, 2)
                replay.reset_mock()
                self.assertEqual(runner.main(['--all', '--inventory', str(fixture / 'inventory.json')]), 0)
                self.assertEqual(replay.call_count, 2)
                report['levels']['L2'] = 'inconclusive'
                self.assertEqual(runner.main(['--active']), 1)
                with patch('sys.stderr'), self.assertRaises(SystemExit) as refused:
                    runner.main(['--active', '--all'])
                self.assertEqual(refused.exception.code, 2)
                with patch('sys.stderr'), self.assertRaises(SystemExit) as refused:
                    runner.main(['--inventory', str(fixture / 'inventory.json')])
                self.assertEqual(refused.exception.code, 2)


if __name__ == "__main__":
    unittest.main()
