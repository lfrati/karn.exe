"""README metrics must not turn admission or short scenarios into stronger claims."""
import json
from pathlib import Path
import tempfile
import unittest

from mgn.runner import inventory_records
import readme_progress as progress


class ReadmeProgressTests(unittest.TestCase):
    def test_only_active_deck_started_terminal_records_count_as_complete_games(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            entries = []
            for name, start_kind, terminal, disposition in [
                ('full', 'decks', True, 'active'),
                ('drawn-game', 'decks', True, 'active'),
                ('prefix', 'decks', False, 'active'),
                ('terminal-scenario', 'position', True, 'active'),
                ('old-full', 'decks', True, 'archived'),
            ]:
                folder = root / name
                folder.mkdir()
                result = {'kind': 'draw' if name == 'drawn-game' else 'win'}
                if not terminal:
                    result = {'kind': 'unfinished'}
                record = {'start': {'kind': start_kind},
                          'transitions': [{'delta': {'result': result}}, {'delta': {}}]}
                (folder / 'record.json').write_text(json.dumps(record))
                entries.append({'record': name + '/record.json', 'disposition': disposition})
            manifest = root / 'inventory.json'
            manifest.write_text(json.dumps({'format': 'mgn-inventory/1', 'records': entries}))
            paths = [path for _, path in inventory_records(manifest, active=True)]
            self.assertEqual(progress.record_counts(paths),
                             {'scenarios_or_prefixes': 2, 'complete_games': 2})

    def test_position_terminal_result_and_illegal_attempt_do_not_make_a_complete_game(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / 'record.json'
            path.write_text(json.dumps({'start': {'kind': 'position', 'state': {'result': {'kind': 'win'}}},
                                        'transitions': [{'illegal': True}]}))
            self.assertEqual(progress.record_counts([path]),
                             {'scenarios_or_prefixes': 1, 'complete_games': 0})

    def test_refresh_preserves_user_text_and_refuses_ambiguous_markers(self):
        original = f'logo and quote\n{progress.START}\nold\n{progress.END}\ncomparison table'
        block = progress.START + '\nnew\n' + progress.END
        updated = progress.replace_block(original, block)
        self.assertEqual(updated, 'logo and quote\n' + block + '\ncomparison table')
        for bad in [original + progress.START, original.replace(progress.END, ''),
                    progress.END + '\n' + progress.START]:
            with self.subTest(bad=bad), self.assertRaises(ValueError):
                progress.replace_block(bad, block)

    def test_render_distinguishes_admission_from_verification_and_test_inventory(self):
        report = {'date': '2026-10-16', 'fingerprint': 'test',
                  'cards': {'total': 10, 'read': 8, 'run': 6, 'dated': '2026-10-11'},
                  'decks': {'total': 5, 'main_read': 3, 'main_run': 2, 'run': 1, 'dated': '2026-10-08'},
                  'commander': {'total': 3, 'main_read': 1, 'main_run': 0, 'dated': '2026-11-22'},
                  'rule_cited_tests': 5, 'mgn_records': {'scenarios_or_prefixes': 2, 'complete_games': 1}}
        result = progress.render(report)
        self.assertIn('| Read every card | Cards in the pinned corpus | 10 |', result)
        self.assertIn('| | Cards the grammar reads, every face | 8 |', result)
        self.assertIn('| Run most of Magic | Cards the engine runs, every face | 6 |', result)
        self.assertIn('| | Published decks in the roster | 5 |', result)
        self.assertIn('| | Decks whose main deck is read in full | 3 |', result)
        self.assertIn('| | Decks whose main deck runs in full | 2 |', result)
        self.assertIn('| | Decks that run in full with their sideboard | 1 |', result)
        self.assertIn('Card corpus dated 2026-10-11; deck roster dated 2026-10-08', result)
        self.assertIn('does not certify correct play', result)
        self.assertNotIn('rule-cited tests', result)
        self.assertIn('not current test results', progress.render_testing(report))
        self.assertIn('active MGN scenarios', progress.render_testing(report))

    def test_measure_counts_whole_cards_and_the_decks_whose_every_main_card_is_read_or_runs(self):
        from unittest.mock import patch
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            for folder in ('resources/oracle', 'resources/decks'):
                (root / folder).mkdir(parents=True)
            # a card counts when every face does: the back face of "two" is read but does not run
            faces = [('runs', 0, 'admitted'), ('two', 0, 'admitted'), ('two', 1, 'engine'), ('unread', 0, 'parse')]
            (root / 'resources/oracle/cards.jsonl').write_text(''.join(
                json.dumps({'oracle_id': card, 'face': face, 'name': f'{card} {face}' if face else card}) + '\n'
                for card, face, _ in faces))
            snapshot = root / 'admission.jsonl'
            snapshot.write_text(''.join(json.dumps({'oracle_id': card, 'face': face, 'stage': stage}) + '\n'
                                        for card, face, stage in faces))
            decks = [{'main': {'runs': 4}, 'sideboard': {}},              # read and runs, whole list too
                     {'main': {'runs': 4}, 'sideboard': {'two': 1}},      # the main deck runs, the sideboard does not
                     {'main': {'runs': 4, 'two': 1}, 'sideboard': {}},    # read in full, does not run
                     {'main': {'runs': 4, 'unread': 1}, 'sideboard': {}}] # not read
            (root / 'resources/decks/selected-decks.jsonl').write_text(''.join(
                json.dumps({'id': f'deck {n}', 'format': 'modern', **deck}) + '\n' for n, deck in enumerate(decks)))
            (root / 'resources/manifest.json').write_text(json.dumps(
                {'sources': {'oracle_cards': {'verification': {'bulk_updated_at': '2026-10-11T09:01:56+00:00'}}}}))
            (root / 'resources/decks/manifest.json').write_text(json.dumps({'snapshot': '2026-10-08', 'commander': {'snapshot': '2026-11-22'}}))
            (root / 'resources/decks/commander.jsonl').write_text(''.join(json.dumps({
                'id': c, 'format': 'commander', 'main': {'runs': 99}, 'commanders': {c: 1}}) + '\n'
                for c in ('two', 'unread')))
            with patch.object(progress, 'ROOT', root), patch.object(progress, 'inventory_records', return_value=[]):
                report = progress.measure(snapshot)
        self.assertEqual(report['cards'], {'total': 3, 'read': 2, 'run': 1, 'dated': '2026-10-11'})
        self.assertEqual(report['commander'], {'total': 2, 'main_read': 1, 'main_run': 0, 'dated': '2026-11-22'})
        self.assertEqual(report['decks'], {'total': 4, 'main_read': 3, 'main_run': 2, 'run': 1, 'dated': '2026-10-08'})


class ReadmeFreshnessTests(unittest.TestCase):
    def test_check_is_read_only_and_distinguishes_stale_inputs(self):
        import contextlib
        import io
        from unittest.mock import patch
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            original = (f'{progress.START}\n<!-- Measurement inputs: {"a" * 64} -->\n'
                        f'{progress.END}\n{progress.TEST_START}\ncounts\n{progress.TEST_END}\n')
            path = root / 'README.md'
            path.write_text(original)
            for fingerprint, expected in (('a' * 64, 0), ('b' * 64, 1)):
                with self.subTest(fingerprint=fingerprint), contextlib.redirect_stdout(io.StringIO()), \
                        patch.object(progress, 'ROOT', root), \
                        patch.object(progress, 'source_fingerprint', return_value=fingerprint), \
                        patch.object(progress.rust_tools, 'build', side_effect=AssertionError('no build')), \
                        patch.object(progress.subprocess, 'run', side_effect=AssertionError('no scan')):
                    self.assertEqual(progress.main(['--check']), expected)
                self.assertEqual(path.read_text(), original)
                self.assertFalse((root / '.cache').exists())

    def test_check_refuses_a_missing_fingerprint_instead_of_claiming_freshness(self):
        import contextlib
        import io
        with contextlib.redirect_stdout(io.StringIO()):
            self.assertEqual(progress.check_freshness(
                f'{progress.START}{progress.END}{progress.TEST_START}{progress.TEST_END}'), 1)


if __name__ == "__main__":
    unittest.main()
