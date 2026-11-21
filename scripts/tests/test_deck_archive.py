import json
from pathlib import Path
import sys
import tempfile
import unittest
from contextlib import redirect_stdout
import io
from collections import Counter

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import deck_archive as archive
import deck_check


class ArchiveTests(unittest.TestCase):
    def test_normalization_never_counts_faces_as_extra_cards(self):
        names = archive.Names()
        parts, changes, unknown = names.normalize({'main': {'Delver of Secrets': 4, 'Fire / Ice': 2}})
        self.assertEqual(sum(parts['main'].values()), 6)
        self.assertEqual(parts['main']['Delver of Secrets // Insectile Aberration'], 4)
        self.assertEqual(len(changes), 2)
        self.assertFalse(unknown)
        self.assertIn('Island', names.basic)
        self.assertNotIn('Steam Vents', names.basic)
        self.assertIn('Wastes', names.basic)
        parts, _, unknown = names.normalize({'main': {
            'Scavenger Regent // Exude Toxin // Scavenger Regent': 3,
            'Clarion Conqueror // Clarion Conqueror': 2, 'Sanctifier <i>en</i>-Vec': 1}})
        self.assertFalse(unknown)
        self.assertEqual(sum(parts['main'].values()), 6)
        self.assertEqual(parts['main']['Clarion Conqueror'], 2)

    def test_merge_keeps_identical_lists_from_different_players(self):
        rows = [{'id': 'player-a', 'main': {'Island': 60}}, {'id': 'player-b', 'main': {'Island': 60}}]
        with tempfile.TemporaryDirectory() as temp:
            paths = [Path(temp) / name for name in ('one.jsonl', 'two.jsonl')]
            for path in paths:
                path.write_text(''.join(json.dumps(row) + '\n' for row in rows))
            self.assertEqual(len(archive.merge(paths)), 2)
            paths[1].write_text(json.dumps({'id': 'player-a', 'main': {'Island': 61}}) + '\n')
            with self.assertRaises(ValueError):
                archive.merge(paths)

    def test_representatives_restore_unique_sideboard_and_designated_cards(self):
        rows = [{'id': 'a', 'format': 'modern', 'event_date_iso': '2023-01-01',
                 'main': {'A': 4, 'B': 4}, 'sideboard': {}},
                {'id': 'b', 'format': 'modern', 'event_date_iso': '2023-01-01',
                 'main': {'A': 4, 'B': 4}, 'sideboard': {'Unique': 1}},
                {'id': 'c', 'format': 'commander', 'event_date_iso': '2023-01-01',
                 'main': {'A': 1}, 'sideboard': {}, 'commanders': {'Commander': 1}}]
        groups, summary = archive.representative_groups(rows, .85)
        self.assertEqual(len(groups), 3)
        self.assertEqual(summary['coverage_promotions'], 1)
        self.assertEqual(summary['cards']['input'], 4)
        self.assertEqual(summary['cards']['lost'], [])
        self.assertEqual(groups[-1]['deck']['id'], 'b')
        self.assertEqual(groups[-1]['reason'], 'preserve_coverage')
        self.assertFalse(any(m['id'] == 'b' for g in groups for m in g['covered']))

    def test_representatives_do_not_merge_similarity_chains(self):
        rows = [{'id': name, 'format': 'modern', 'event_date_iso': '2023-01-01', 'main': main, 'sideboard': {}}
                for name, main in [('a', {'A': 4, 'B': 1}), ('b', {'A': 4, 'B': 1, 'C': 1}),
                                   ('c', {'A': 4, 'C': 1})]]
        groups, summary = archive.representative_groups(rows, .75)
        self.assertEqual([g['deck']['id'] for g in groups], ['a', 'c'])
        self.assertEqual(groups[0]['covered'][0]['id'], 'b')
        self.assertEqual(summary['cards']['lost'], [])

    def test_representatives_keep_eras_and_report_contextual_card_losses(self):
        rows = [{'id': name, 'format': 'modern', 'event_date_iso': date, 'main': {'A': 4, 'B': 4}, 'sideboard': side}
                for name, date, side in [('a', '2001-01-01', {'U': 1}), ('b', '2011-01-01', {}),
                                         ('c', '2011-01-01', {'U': 1})]]
        groups, summary = archive.representative_groups(rows, .85)
        self.assertEqual(len(groups), 2)
        self.assertEqual(summary['cards']['lost'], [])
        self.assertEqual(summary['by_format_era'][1]['lost_cards'], ['U'])
        groups, summary = archive.representative_groups(rows, .85, coverage_scope='format-era')
        self.assertFalse(any(p['lost_cards'] for p in summary['by_format_era']))

    def test_representatives_preserve_unresolved_source_evidence(self):
        rows = [{'id': name, 'format': 'modern', 'event_date_iso': '2023-01-01',
                 'main': {'A': 4, 'B': 4}, 'sideboard': side, 'names_absent_from_pinned_corpus': unknown}
                for name, side, unknown in [('clean', {}, []), ('review', {'Unresolved Name': 1}, ['Unresolved Name'])]]
        groups, summary = archive.representative_groups(rows, .85)
        self.assertEqual(summary['cards']['lost'], [])
        self.assertEqual(groups[-1]['deck']['names_absent_from_pinned_corpus'], ['Unresolved Name'])

    def test_overlap_uses_copies_and_separates_setup(self):
        self.assertAlmostEqual(archive.overlap(Counter({'A': 4}), Counter({'A': 1, 'B': 3})), 1/7)
        rows = [{'id': str(i), 'format': fmt, 'main': {'Island': 20, 'A': q, 'B': 4-q},
                 'sideboard': {}, 'companion': companion}
                for i, (fmt, q, companion) in enumerate([
                    ('modern', 4, None), ('modern', 3, None), ('legacy', 4, None),
                    ('modern', 4, 'Yorion, Sky Nomad')])]
        self.assertEqual(len(list(archive.pairs(rows, .8))), 1)
        self.assertEqual(list(archive.pairs(rows, .8, {'Island'})), [])
        self.assertEqual(archive.overlap(Counter(), Counter()), 0)

    def test_export_retains_main_and_sideboard_and_separates_commander(self):
        row = {'id': 'test', 'source': 'https://example.org/deck', 'main': {'Island': 99},
               'sideboard': {'Negate': 2}, 'commanders': {'Talrand, Sky Summoner': 1}}
        with tempfile.TemporaryDirectory() as temp:
            path = Path(temp) / 'test.dck'
            archive.export(row, path)
            main, sideboard = deck_check.read_deck(path)
            self.assertEqual(main, {'Island': 99})
            self.assertEqual(sideboard, {'Negate': 2})
            setup = json.loads(path.with_suffix('.setup.json').read_text())
            self.assertEqual(setup['commanders'], ['Talrand, Sky Summoner'])
            with self.assertRaises(ValueError):
                archive.export({**row, 'source_issues': ['incomplete publication']}, path)
            with self.assertRaises(ValueError):
                archive.export({**row, 'names_absent_from_pinned_corpus': ['Missing']}, path)


if __name__ == '__main__':
    unittest.main()
