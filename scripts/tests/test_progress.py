"""Progress must distinguish admission, test references and actual current evidence."""
import copy
import json
import re
import sys
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import progress


class ProgressTests(unittest.TestCase):
    def test_corpus_admission_does_not_count_parsed_only_faces(self):
        result = progress.corpus_counts("SUMMARY faces=100 read=80 run=30 deck_cards=10 deck_read=8 deck_run=3\n")
        self.assertEqual((result['faces'], result['parsed'], result['admitted']), (100, 80, 30))
        self.assertEqual(result['parsed_not_admitted'], 50)
        self.assertEqual(result['not_parsed'], 20)
        for text in ("", "SUMMARY faces=100 read=80 run=90", "SUMMARY faces=0 read=0 run=0"):
            with self.assertRaises(ValueError):
                progress.corpus_counts(text)

    def test_route_census_counts_admitted_faces_once_without_claiming_complete_migration(self):
        def row(name, stage, routes):
            return {"name": name, "stage": stage, "compiled_abilities": routes}
        inventory = {
            ("split", 0): row("Split", "admitted", {"whole": [0], "half:0": [0], "half:1": []}),
            ("legacy", 0): row("Legacy", "admitted", {"whole": []}),
            ("refused", 0): row("Refused", "engine", {"whole": [0]}),
            ("unparsed", 0): row("Unparsed", "parse", None),
        }
        result = progress.route_counts(inventory)
        self.assertEqual(result["admitted_faces"], 2)
        self.assertEqual(result["faces_with_compiled_program"], 1)
        self.assertIn("not classified", result["scope"])
        old = copy.deepcopy(inventory)
        inventory[("legacy", 0)]["compiled_abilities"]["whole"] = [0]
        changes = progress.route_changes(old, inventory)
        self.assertEqual([row["name"] for row in changes["faces"]], ["Legacy"])
        self.assertEqual(changes["faces"][0]["before"]["compiled_abilities"], {"whole": []})
        # A count-preserving route change must still be visible.
        old = copy.deepcopy(inventory)
        inventory[("legacy", 0)]["compiled_abilities"]["whole"] = [1]
        self.assertEqual(len(progress.route_changes(old, inventory)["faces"]), 1)
        del old[("unparsed", 0)]
        self.assertEqual(progress.route_changes(old, inventory)["status"], "inventory changed")
        old = copy.deepcopy(inventory)
        del old[("legacy", 0)]["compiled_abilities"]
        self.assertEqual(progress.route_changes(old, inventory)["status"], "baseline lacks route evidence")
        with self.assertRaises(ValueError):
            progress.route_counts(old)

    def test_route_inventory_rejects_duplicate_face_evidence(self):
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "admission.jsonl"
            row = json.dumps({"oracle_id": "same", "face": 0}) + "\n"
            path.write_text(row + row)
            with self.assertRaises(ValueError):
                progress.route_inventory(path)

    def test_deck_completion_includes_sideboard_and_excludes_fixtures(self):
        def parts(main, side):
            return {"main": {"total": 3, "accepted": main}, "sideboard": {"total": 1, "accepted": side}}
        result = progress.deck_counts({'decks': {'decks/a.dck': parts(3, 0), 'decks/b.dck': parts(3, 1),
                                                'decks/lands.dck': parts(3, 1), 'decks/starter/red.dck': parts(3, 1)},
                                       'cards': {'Shared': {'accepted': True}, 'Missing': {'accepted': False}}})
        self.assertEqual(result['main_decks_total'], 2)
        self.assertEqual(result['main_decks_admitted'], 2)
        self.assertEqual(result['whole_decks_admitted'], 1)
        self.assertEqual((result['admitted_names'], result['distinct_names']), (1, 2))

    def test_stale_failed_and_missing_checks_never_pass(self):
        inputs = {'engine': 'a', 'tooling': 'python1'}
        record = {'passed': True, 'inputs': inputs.copy(), 'unchanged': True}
        keys = tuple(inputs)
        self.assertEqual(progress.evidence_status(None, inputs, keys), 'MISSING')
        self.assertEqual(progress.evidence_status(record, inputs, keys), 'PASS')
        self.assertEqual(progress.evidence_status({**record, 'passed': False}, inputs, keys), 'FAIL')
        self.assertEqual(progress.evidence_status({**record, 'unchanged': False}, inputs, keys), 'STALE')
        for key in keys:
            self.assertEqual(progress.evidence_status(record, {**inputs, key: 'changed'}, keys), 'STALE')
        self.assertEqual(progress.evidence_status({'passed': True}, inputs, keys), 'STALE')


    def test_grammar_breakdown_keeps_round_trips_separate_from_gate_success(self):
        text = ("selectors   RoundTrip: 58862 (91.1%) GATE no mismatch, no ambiguity: ok\n"
                "effects     RoundTrip: 44372 (91.0%) GATE mismatch: FAILED\n"
                "cards       faces: 32872  with text: 32494  all rows round-trip: 28294 (87.1%) "
                "GATE no mismatch, no ambiguity: ok\n")
        result = progress.check_result('grammar', 1, text)
        self.assertFalse(result['passed'])
        self.assertEqual(result['rows']['cards']['count'], 28294)
        self.assertEqual(result['rows']['cards']['percent'], 87.1)
        self.assertFalse(result['rows']['effects']['passed'])
        self.assertNotIn('amounts', result['rows'])
        # a failing gate is named with what it printed; a gate that printed no result says why
        self.assertIn('effects: RoundTrip: 44372 (91.0%) GATE mismatch: FAILED', result['failures'])
        missing = progress.check_result('grammar', 1, 'selectors   NO RESULT: cannot read x.jsonl\n')
        self.assertIn('selectors: NO RESULT: cannot read x.jsonl', missing['failures'])
        self.assertIn('amounts', missing['failures'])
        output = '\n'.join(progress.render_grammar({**result, 'status': 'FAIL'}))
        self.assertIn('check: FAILED', output)
        self.assertIn('UNKNOWN  check: missing', output)
        stale = '\n'.join(progress.render_grammar({**result, 'status': 'STALE'}))
        self.assertNotIn('check: ok', stale)
        self.assertIn('check: stale', stale)

    def test_empty_or_partial_validation_output_is_not_green(self):
        for text in ('', 'Ran 0 tests in 0 seconds\nOK'):
            self.assertFalse(progress.check_result('tooling', 0, text)['passed'])
        self.assertTrue(progress.check_result('tooling', 0, 'Ran 12 tests in 0.1s\nOK')['passed'])
        self.assertFalse(progress.check_result('tooling', 1, 'Ran 12 tests in 0.1s')['passed'])
        nested = progress.check_result('tooling', 1, 'Ran 99 tests in 0.1s\n\nOK\nFAIL: x (m.T.x)\n\nRan 12 tests in 0.1s\n\nFAILED (failures=1)')
        self.assertEqual((nested['passed'], nested['counts']['executed'], nested['counts']['failed']), (False, 12, 1),
                         'the last footer is the run; a test may print one')
        gate = lambda name: f'{name} RoundTrip: 42 GATE no mismatch, no ambiguity: ok\n'
        self.assertFalse(progress.check_result('grammar', 0, ''.join(map(gate, progress.GATES[:-1])))['passed'])
        output = ''.join(map(gate, progress.GATES))
        self.assertTrue(progress.check_result('grammar', 0, output)['passed'])
        self.assertFalse(progress.check_result('grammar', 1, output)['passed'])

    WORKSPACE = ("   Compiling game v0.1.0\n"
                 "     Running unittests src/lib.rs (target/debug/deps/game-0123)\n"
                 "running 2 tests\ntest a::one ... ok\ntest a::two ... ignored\n"
                 "test result: ok. 1 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.1s\n"
                 "     Running tests/rules.rs (target/debug/deps/rules-4567)\n"
                 "running 3 tests\ntest scenarios::x::good ... ok\ntest scenarios::x::bad ... FAILED\n"
                 "test scenarios::x::panics - should panic ... ok\n"
                 "test result: FAILED. 2 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.2s\n"
                 "error: test failed, to rerun pass `-p game --test rules`\n"
                 "   Doc-tests game\nrunning 0 tests\n"
                 "test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.0s\n"
                 "error: 1 target failed:\n")

    def test_workspace_log_counts_every_binary_and_names_its_failures(self):
        with patch.object(progress.rules, 'revision', lambda: {}):
            report = progress.rules.test_report(self.WORKSPACE, 101, 'h', 'h')
        self.assertEqual(report['totals'], {'passed': 3, 'failed': 1, 'ignored': 1, 'binaries': 3})
        self.assertEqual(report['failures'], ['rules scenarios::x::bad'])
        # cargo's summary of a failing binary is no build error and leaves the run complete
        self.assertEqual(report['errors'], [])
        self.assertTrue(report['complete'])
        self.assertIn('--no-fail-fast', report['command'])

    def test_a_build_error_or_a_binary_without_its_result_is_incomplete(self):
        with patch.object(progress.rules, 'revision', lambda: {}):
            broken = progress.rules.test_report("error[E0308]: mismatched types\n   --> crates/ir/src/lib.rs:3:5\n"
                                                "error: could not compile `game`\n", 101, 'h', 'h')
            self.assertFalse(broken['complete'])
            self.assertEqual(broken['errors'], ['error[E0308]: mismatched types at crates/ir/src/lib.rs:3:5',
                                                'error: could not compile `game`'])
            crashed = self.WORKSPACE.replace("test result: ok. 0 passed", "process didn't exit successfully")
            report = progress.rules.test_report(crashed, 101, 'h', 'h')
            self.assertFalse(report['complete'])
            # the binary that stopped is named, beside any failing test
            stopped = [name for name in report['unfinished']]
            self.assertEqual(len(stopped), 1)
            data = {'checks': {'workspace': {'status': 'INCOMPLETE', 'failures': ['rules a_test'], 'unfinished': stopped,
                                             'errors': [], 'log': 'w.log'}, 'tooling': {'status': 'PASS'}, 'grammar': {'status': 'PASS'}},
                    'rules': {'attributed': {'run_state': 'CURRENT', 'problems': []}}}
            lines = progress.failure_lines(data)
            self.assertIn('    Rust: rules a_test', lines)
            self.assertIn(f'    Rust: {stopped[0]}: no result line', lines)
            self.assertFalse(progress.rules.test_report(self.WORKSPACE, 0, 'h', 'changed')['unchanged'])

    def test_tooling_counts_separate_failures_errors_and_skips(self):
        text = ("test_a (test_x.T.test_a) ... ok\ntest_b (test_x.T.test_b) ... FAIL\n"
                "======\nFAIL: test_b (test_x.T.test_b)\n------\nERROR: test_c (test_y.T.test_c)\n"
                "Ran 10 tests in 0.5s\n\nFAILED (failures=1, errors=1, skipped=2)\n")
        result = progress.check_result('tooling', 1, text)
        self.assertEqual(result['counts'], {'executed': 10, 'failed': 2, 'skipped': 2, 'expected_failures': 0, 'passed': 6})
        self.assertEqual(result['failures'], ['test_b (test_x.T.test_b)', 'test_c (test_y.T.test_c)'])
        self.assertFalse(result['passed'])
        ok = progress.check_result('tooling', 0, 'Ran 6 tests in 0.1s\n\nOK (skipped=1, expected failures=2)\n')
        self.assertEqual(ok['counts'], {'executed': 6, 'failed': 0, 'skipped': 1, 'expected_failures': 2, 'passed': 3})
        self.assertTrue(ok['passed'])
        # an unexpected success fails the run; it is counted and named as a failure
        surprise = progress.check_result('tooling', 1, 'test_d (test_z.T.test_d) ... unexpected success\n'
                                         'Ran 3 tests in 0.1s\n\nFAILED (unexpected successes=1)\n')
        self.assertEqual(surprise['counts']['failed'], 1)
        self.assertEqual(surprise['counts']['passed'], 2)
        self.assertEqual(surprise['failures'], ['test_d (test_z.T.test_d): unexpected success'])

    def test_one_workspace_run_also_records_the_rules_receipts_and_failure_propagates(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            calls, batches = [], []
            def command(args, log, verbose=False, env=None, label=None):
                calls.append((args, env))
                log.write_text('')
                return 0, ''
            def commands(jobs, verbose=False):
                batches.append([job.args for job in jobs])
                tree['hash'] = 'edited during the run'
                out = []
                for job in jobs:
                    calls.append((job.args, job.env))
                    text = self.WORKSPACE if job.args[0] == 'cargo' else (
                        'Ran 3 tests in 0.1s\n\nOK\n' if 'scripts/tooling_tests.py' in job.args else
                        ''.join(f'{g} RoundTrip: 1 GATE no mismatch, no ambiguity: ok\n' for g in progress.GATES))
                    job.log.write_text(text)
                    out.append(((101 if job.args[0] == 'cargo' else 0), text))
                return out
            recorded, tree = {}, {'hash': 'h'}
            with patch.object(progress, 'command', command), patch.object(progress, 'commands', commands), \
                    patch.object(progress, 'CACHE', root), \
                    patch.object(progress.rule_evidence, 'CACHE', root), patch.object(progress.rule_evidence, 'ROOT', root), \
                    patch.object(progress.rule_evidence, 'RUN', root / 'run.json'), \
                    patch.object(progress.rules, 'suite_hash', lambda: tree['hash']), patch.object(progress.rules, 'revision', lambda: {}):
                run, failed = progress.run_suites(root, False, lambda name, result, keys: recorded.update({name: result}), workers=3)
                evidence = progress.rule_evidence.load_run(root / 'run.json')
            cargo = [args for args, _ in calls if args[:2] == ['cargo', 'test'] and '--no-run' not in args]
            self.assertEqual(cargo, [progress.rules.WORKSPACE_TESTS], 'one cargo test run, of the whole workspace')
            self.assertEqual([args for args, _ in calls][:len(progress.TEST_BUILDS)], progress.TEST_BUILDS, 'built first, one at a time')
            self.assertEqual(len(batches), 1, 'then every suite at once')
            self.assertEqual(batches[0][0], progress.rules.WORKSPACE_TESTS)
            self.assertIn('--workers', batches[0][1])
            env = next(env for args, env in calls if args == progress.rules.WORKSPACE_TESTS)
            self.assertEqual(env['RULE_RUN'], evidence['run'])
            gates = next(env for args, env in calls if args == ['bash', 'scripts/gates.sh'])
            self.assertEqual(gates['PYTHON'], sys.executable, 'the gates regenerate corpus dumps with this interpreter')
            # the rules target's own outcomes, qualified, and its own failure
            self.assertEqual(evidence['tests'], {'scenarios::x::good': 'ok', 'scenarios::x::bad': 'FAILED',
                                                 'scenarios::x::panics': 'ok'})
            self.assertEqual(evidence['exit_code'], 1)
            self.assertTrue(evidence['complete'])
            self.assertEqual(failed, 1)
            self.assertEqual(run['totals']['failed'], 1)
            self.assertFalse(run['unchanged'], 'the suite hash is taken before the run, so an edit during it shows')
            self.assertEqual(set(recorded), {'tooling', 'grammar'})
            self.assertTrue(recorded['grammar']['passed'])

    def report(self):
        attributed = {'total': 3165, 'rules_with_sites': 390, 'sites': 1441, 'run_state': 'CURRENT', 'problems': [],
                      'execution': {'passing': 384, 'failing': 1, 'diagnostic': 0, 'not_run': 5}}
        data = {'revision': {'commit': 'abcdef0123456789', 'worktree': ''}, 'measurement_unchanged': True,
                'rules': {'attributed': attributed},
                'checks': {'workspace': {'status': 'FAIL', 'totals': {'passed': 3, 'failed': 1, 'ignored': 1},
                                         'failures': ['rules scenarios::x::bad'], 'log': 'w.log'},
                           'tooling': {'status': 'PASS', 'counts': {'executed': 6, 'failed': 0, 'skipped': 1, 'expected_failures': 2, 'passed': 3}},
                           'grammar': {'status': 'PASS', 'counts': {'passed': 7, 'expected': 7}}},
                'dashboard': {'cards': {'total': 4, 'parsed': 3, 'supported': 2},
                              'legal': {'format': 'standard', 'total': 3, 'supported': 1, 'parsed': 2, 'distinct_blockers': 2,
                                        'refused': {'parse': 1, 'engine': 1}, 'source': 'resources/oracle/legal-standard.jsonl',
                                        'first_blockers': [{'reason': 'keyword: Ward', 'cards': 1, 'examples': 'C'}]},
                              'decks': {'overall': {'decks': 2, 'percent': 99.999, 'main_fully_admitted': 1, 'fully_admitted': 0}}}}
        return data

    def test_summary_shows_counts_and_failures_and_leaves_the_detail_out(self):
        data = self.report()
        text = progress.render(data)
        self.assertIn('Rust workspace', text)
        self.assertRegex(text, r'Rust workspace\s+60\.00%\s+\(3/5\)\s+1\s+1  FAIL')
        self.assertRegex(text, r'Python tooling\s+50\.00%\s+\(3/6\)\s+0\s+3  \(skipped includes 2 expected failures\)\n')
        self.assertRegex(text, r'Grammar gates\s+100\.00%\s+\(7/7\)')
        self.assertRegex(text, r'with a passing expectation\s+12\.13%\s+\(384/3,165\)')
        # the columns line up: every percentage ends at the same column
        rows = [line for line in text.splitlines() if re.search(r'\d%\s+\(', line)]
        self.assertEqual(len({line.index('%') for line in rows}), 1, rows)
        self.assertIn('1,441 expectations', text)
        self.assertRegex(text, r'Standard-legal supported\s+33\.33%\s+\(1/3\)  refused: 1 not read, 1 by the engine, 2 first blockers')
        self.assertNotIn('keyword: Ward', text, 'the blockers are detail')
        data['dashboard']['legal'].update(supported=None, refused=None)
        self.assertRegex(progress.render(data), r'Standard-legal supported\s+UNKNOWN')
        self.assertIn('99.99%', text, 'truncated, never rounded up')
        self.assertIn('Rust: rules scenarios::x::bad', text)
        self.assertIn('Rust log: w.log', text)
        for detail in ('Legacy', 'SHA-256', 'pauper', 'assessments'):
            self.assertNotIn(detail, text)
        self.assertLessEqual(len(text.splitlines()), 25)
        passing = copy.deepcopy(data)
        passing['checks']['workspace'].update(status='PASS', totals={'passed': 5, 'failed': 0, 'ignored': 0}, failures=[])
        self.assertNotIn('Failures', progress.render(passing))
        unavailable = copy.deepcopy(passing)
        unavailable['rules']['attributed'].update(execution=None, run_state='INCOMPLETE')
        text = progress.render(unavailable)
        self.assertIn('UNAVAILABLE', text)
        self.assertIn('rule receipts: INCOMPLETE', text)

    def test_a_snapshot_run_is_named_as_such_and_a_live_run_is_not(self):
        with tempfile.TemporaryDirectory() as tmp:
            manifest = Path(tmp) / 'manifest.json'
            manifest.write_text(json.dumps({'root': '/checkout', 'snapshot': '/checkout/.cache/progress-snapshot/tree',
                                            'revision': 'abc', 'status': ' M x', 'captured_at': '2026-10-10T18:30:01+00:00',
                                            'digest': 'd' * 64, 'files': {'x': [1, 2, 'h']}}))
            with patch.dict(progress.os.environ, {progress.SNAPSHOT_OF: '/checkout', progress.SNAPSHOT_MANIFEST: str(manifest)}):
                identity = progress.snapshot_identity()
            with patch.dict(progress.os.environ, {progress.SNAPSHOT_OF: ''}):
                self.assertIsNone(progress.snapshot_identity())
        self.assertEqual(identity['root'], '/checkout')
        self.assertNotIn('files', identity, 'the report keeps the identity, not the file list')
        data = {**self.report(), 'measured': identity, 'snapshot': '/checkout/.cache/progress-runs/x/report.json'}
        self.assertIn('  a snapshot of /checkout captured 2026-10-10 18:30:01 UTC', progress.summary(data).splitlines()[1])
        self.assertNotIn('snapshot of', progress.summary({**data, 'measured': None}))

    def test_rule_references_do_not_become_verified_credit(self):
        data = {'metrics': {'total': 2, 'verified': 1}, 'execution_current': True,
                'execution': {'tests': {'good': ['ok'], 'ignored': ['ignored']}},
                'rules': {'100.1': {'tests': [{'test': 'good'}], 'status': 'partial', 'verified': False},
                          '100.2': {'tests': [{'test': 'ignored'}], 'status': 'unreviewed', 'verified': False}},
                'source': {}, 'limits': []}
        self.assertEqual(progress.rule_counts(data)['with_passing_named_test'], 1)
        stale = progress.rule_counts({**data, 'execution_current': False})
        self.assertIsNone(stale['verified'])
        self.assertIsNone(stale['with_passing_named_test'])

    def test_changed_inventories_and_stale_evidence_do_not_produce_false_deltas(self):
        old = {'schema': 1, 'inputs': {'corpus': 'a', 'rules': 'b', 'decks': 'c'},
               'corpus': {'parsed': 80, 'admitted': 30}, 'rules': {'reviewed': 10, 'referenced_by_tests': 8, 'verified': 3},
               'decks': {'admitted_names': 5, 'main_decks_admitted': 1, 'whole_decks_admitted': 0}}
        new = copy.deepcopy(old)
        new['corpus']['admitted'] = 33
        new['rules']['verified'] = None
        self.assertIn('corpus.admitted: +3', progress.delta(old, new))
        self.assertFalse(any('verified' in line for line in progress.delta(old, new)))
        # attributed passing compares only between two available measurements
        old['rules'].update(attributed_scope='s', attributed_rules=10, attributed_passing=None)
        new['rules'].update(attributed_scope='s', attributed_rules=12, attributed_passing=9)
        self.assertIn('rules.attributed_rules: +2', progress.delta(old, new))
        self.assertFalse(any('attributed_passing' in line for line in progress.delta(old, new)))
        old['rules']['attributed_passing'] = 7
        self.assertIn('rules.attributed_passing: +2', progress.delta(old, new))
        # a different measure, or none before it, is not comparable
        for scope in ('other', None):
            old['rules']['attributed_scope'] = scope
            self.assertFalse(any('attributed' in line for line in progress.delta(old, new)), scope)
        new['inputs']['corpus'] = 'new-corpus'
        lines = progress.delta(old, new)
        self.assertIn('corpus: inventory changed; numerical delta omitted', lines)
        self.assertFalse(any('corpus.admitted' in line for line in lines))

    def test_census_cache_reuses_only_matching_complete_measurements(self):
        with tempfile.TemporaryDirectory() as temp:
            snapshot = Path(temp) / 'admission.jsonl'
            snapshot.write_text('snapshot')
            old = {'measurement_unchanged': True, 'inputs': {'census': 'same'},
                   'routing': {'snapshot': str(snapshot)}}
            self.assertEqual(progress.reusable_census(old, {'census': 'same'}), snapshot)
            self.assertIsNone(progress.reusable_census(old, {'census': 'new'}))
            self.assertIsNone(progress.reusable_census(old, {'census': 'same'}, refresh=True))
            self.assertIsNone(progress.reusable_census({**old, 'measurement_unchanged': False}, {'census': 'same'}))
            snapshot.unlink()
            self.assertIsNone(progress.reusable_census(old, {'census': 'same'}))

    def test_census_hash_ignores_test_changes_but_tracks_engine_source(self):
        with tempfile.TemporaryDirectory() as temp, patch.object(progress, 'ROOT', Path(temp)):
            root = Path(temp)
            source = root / 'crates/game/src/lib.rs'
            test = root / 'crates/game/tests/rules.rs'
            source.parent.mkdir(parents=True)
            test.parent.mkdir(parents=True)
            source.write_text('engine')
            test.write_text('test')
            before = progress.census_hash()
            test.write_text('more tests')
            self.assertEqual(before, progress.census_hash())
            source.write_text('new engine')
            self.assertNotEqual(before, progress.census_hash())


    def test_fingerprints_detect_edits_and_missing_files(self):
        with tempfile.TemporaryDirectory() as tmp:
            p = Path(tmp) / 'file'
            missing = progress.fingerprint([p])
            p.write_text('one')
            one = progress.fingerprint([p])
            p.write_text('two')
            two = progress.fingerprint([p])
            self.assertEqual(len({missing, one, two}), 3)

    def test_grammar_freshness_tracks_its_inputs_but_not_game_only_changes(self):
        with tempfile.TemporaryDirectory() as tmp, patch.object(progress, 'ROOT', Path(tmp)):
            root = Path(tmp)
            paths = ['Cargo.toml', 'Cargo.lock', 'scripts/gates.sh', 'resources/oracle/cards.jsonl',
                     'crates/grammar/src/lib.rs', 'crates/oracle/src/lib.rs', 'crates/ir/src/lib.rs',
                     'crates/game/src/lib.rs']
            for name in paths:
                path = root / name
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_text('initial')
            initial = progress.grammar_hash()
            (root / 'crates/game/src/lib.rs').write_text('engine optimization')
            self.assertEqual(initial, progress.grammar_hash())
            for name in paths[:-1]:
                path = root / name
                before = progress.grammar_hash()
                path.write_text('changed')
                self.assertNotEqual(before, progress.grammar_hash(), name)


if __name__ == '__main__':
    unittest.main()
