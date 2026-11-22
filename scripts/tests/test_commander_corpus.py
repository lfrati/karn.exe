"""Pinned authored Commander lists must remain legal targets, including designations."""
from collections import Counter
import hashlib
import json
from pathlib import Path
import unittest

import blocker_report
import deck_archive
from test_blocker_report import row

ROOT = Path(__file__).resolve().parents[2]


class CommanderCorpusTests(unittest.TestCase):
    def test_pinned_lists_have_100_legal_color_consistent_singleton_cards(self):
        folder = ROOT / 'resources/decks'
        manifest = json.loads((folder / 'manifest.json').read_text())
        for name in ('commander.jsonl', 'commander-legality.json'):
            data = (folder / name).read_bytes()
            self.assertEqual(manifest['files'][name], {'sha256': hashlib.sha256(data).hexdigest(), 'bytes': len(data)})
        decks = deck_archive.load(folder / 'commander.jsonl')
        self.assertEqual(len(decks), manifest['commander']['lists'])
        metadata = json.loads((folder / 'commander-legality.json').read_text())
        legal = {c['name']: c for c in metadata['cards']}
        faces = [json.loads(l) for l in (ROOT / 'resources/oracle/cards.jsonl').read_text().splitlines()]
        front = {f['name']: f for f in faces if not f.get('face', 0)}
        needed = set().union(*(deck_archive.deck_cards(d) for d in decks))
        self.assertEqual(set(legal), needed, 'every required card, including commanders, has pinned metadata')
        self.assertEqual({c['oracle_id'] for c in legal.values()},
                         {i for request in metadata['sources'] for i in request['oracle_ids']})
        for deck in decks:
            with self.subTest(deck=deck['id']):
                self.assertEqual(deck['source_kind'], 'authored')
                self.assertEqual(deck['license'], 'MIT')
                self.assertEqual(deck['format'], 'commander')
                self.assertEqual(sum(deck['commanders'].values()), 1)
                self.assertFalse(deck['sideboard'])
                self.assertIsNone(deck['companion'])
                commander = next(iter(deck['commanders']))
                self.assertNotIn(commander, deck['main'])
                self.assertIn('Legendary Creature', front[commander]['type_line'])  # CR 903.3
                identity = set(legal[commander]['color_identity'])
                cards = Counter(deck['main']) + Counter(deck['commanders'])
                self.assertEqual(sum(cards.values()), 100)  # CR 903.5a
                for name, quantity in cards.items():
                    self.assertIs(type(quantity), int)
                    self.assertGreater(quantity, 0)
                    self.assertTrue(quantity == 1 or 'Basic' in front[name]['type_line'])  # 903.5b
                    self.assertEqual(legal[name]['oracle_id'], front[name]['oracle_id'])
                    self.assertEqual(legal[name]['commander'], 'legal')
                    self.assertLessEqual(set(legal[name]['color_identity']), identity)  # 903.5c/d

    def test_a_refused_or_missing_commander_cannot_receive_whole_deck_credit(self):
        cards = [{'oracle_id': c, 'name': c} for c in ('Island', 'Commander')]
        admitted = row('Island', 0, 'admitted')
        refused = row('Commander', 0, 'engine', [('engine', 'commander mechanism', None, ())])
        deck = {'id': 'designation', 'format': 'commander', 'main': {'Island': 99}, 'commanders': {'Commander': 1}}
        labels, totals = blocker_report.deck_report([admitted, refused], [deck], cards)
        self.assertEqual(totals['main_decks_that_run'], 0)
        self.assertEqual(labels[0]['examples'], ['Commander'])
        missing = {**deck, 'commanders': {'Absent': 1}}
        _, totals = blocker_report.deck_report([admitted, refused], [missing], cards)
        self.assertEqual((totals['main_decks_that_run'], totals['unresolved_decks']), (0, 1))
