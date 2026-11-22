"""Retained public selections keep their explicit chooser through the player client."""
import os
import unittest

import play
import test_player_prompt_client as fixtures
from mgn.adapters.karn import KarnAdapter
from test_mgn_adapter import ORACLE, MAIN_A, PRIORITY_A, position_record


class PilesCardTypesClientTests(unittest.TestCase):
    new = fixtures.PlayerPromptClientTests.new
    waiting = fixtures.PlayerPromptClientTests.waiting
    named = staticmethod(fixtures.PlayerPromptClientTests.named)
    close_engine = fixtures.PlayerPromptClientTests.close_engine
    refused = fixtures.PlayerPromptClientTests.refused

    @classmethod
    def setUpClass(cls):
        fixtures.PlayerPromptClientTests.setUpClass()

    def setUp(self):
        self.engine = play.Engine(executable=os.environ.get('GAME_SERVE_REVIEW'))
        self.addCleanup(self.close_engine)

    def choice(self, kind):
        for _ in range(20):
            seat = self.waiting()
            if seat.last['decision']['kind'] == kind:
                return seat
            self.assertEqual(seat.last['decision']['kind'], 'priority')
            self.assertTrue(seat.last['stack'])
            seat.answer({'kind': 'pass'})
        self.fail(f'no {kind} choice')

    def test_liliana_controller_separates_and_target_chooses_without_a_second_sacrifice_selection(self):
        # CR 700.3a/c/d, 608.2d, 701.21a. The position supplies seven loyalty.
        cards = {'lili': ('Liliana of the Veil', 'A'), 'bear': ('Grizzly Bears', 'B'),
                 'forest': ('Forest', 'B'), 'ring': ('Sol Ring', 'B')}
        record = position_record(cards, {'battlefield': list(cards)}, MAIN_A, PRIORITY_A,
                                 objects={'lili': {'counters': {'loyalty': 7}}})
        adapter = KarnAdapter(os.environ.get('GAME_SERVE_REVIEW', play.ROOT / 'target/fast/game-serve'), ORACLE)
        adapter.seats = record['game']['seats']
        adapter.cards = record['cards']
        native = adapter._native_position(record['start'], record['game'])
        self.engine.send({'cmd': 'new', 'position': native})
        self.seats = [play.Seat(self.engine, i) for i in range(2)]
        me, them = self.seats
        p = me.prompt()
        lili = self.named(p, 'Liliana of the Veil')[0]
        me.answer({'kind': 'activate', 'ability': f'{lili}.a2', 'targets': [['p1']]})
        self.assertIs(self.choice('select'), me)
        p = me.last
        bear = self.named(p, 'Grizzly Bears')[0]
        forest = self.named(p, 'Forest')[0]
        ring = self.named(p, 'Sol Ring')[0]
        self.assertEqual(set(p['decision']['among']), {bear, forest, ring})
        self.assertEqual((p['decision']['min'], p['decision']['max']), (0, 3))
        self.assertIn('not among', self.refused(me, {'cmd': 'answer', 'answer': {'kind': 'choose', 'answer': {'objects': [lili]}}}))
        me.answer({'kind': 'choose', 'answer': {'objects': [bear]}})
        p = them.prompt()
        self.assertEqual(p['decision']['kind'], 'pile')
        self.assertEqual(p['decision']['piles'][0], self.named(p, 'Grizzly Bears'))
        self.assertEqual(set(p['decision']['piles'][1]), set(self.named(p, 'Forest') + self.named(p, 'Sol Ring')))
        self.assertIsNone(me.prompt()['decision'])
        them.answer({'kind': 'choose', 'answer': {'pile': 0}})
        p = me.prompt()
        self.assertEqual(len(self.named(p, 'Grizzly Bears', zone='graveyard')), 1)
        self.assertEqual(len(self.named(p, 'Forest', zone='battlefield')), 1)
        self.assertEqual(len(self.named(p, 'Sol Ring', zone='battlefield')), 1)
        self.assertEqual(p['decision']['kind'], 'priority')
        self.assertEqual(p['stack'], [])

    def test_atraxa_reveal_stays_public_through_choices_and_ends_after_the_remainder(self):
        # CR 701.20a, 608.2c/d; the pinned ruling allows assigning a multi-type card
        # to one type and choosing a different card for its other type.
        me, them = self.new({'hand': ['Atraxa, Grand Unifier'],
                             'battlefield': ['Forest'] * 4 + ['Plains', 'Island', 'Swamp'],
                             'library': ['Sol Ring', 'Ornithopter', 'Forest']}, {})
        p = me.prompt()
        mana = [o for name in ['Forest', 'Plains', 'Island', 'Swamp'] for o in self.named(p, name, controller='p0')]
        me.answer({'kind': 'cast', 'card': self.named(p, 'Atraxa, Grand Unifier')[0],
                   'plan': [{'ability': f'{o}.a0'} for o in mana], 'mana': {'G': 4, 'W': 1, 'U': 1, 'B': 1}})
        self.assertIs(self.choice('select'), me)
        p = me.last
        ring = self.named(p, 'Sol Ring')[0]
        ornithopter = self.named(p, 'Ornithopter')[0]
        self.assertEqual(set(p['decision']['among']), {ring, ornithopter})
        enemy = them.prompt()
        self.assertEqual(sorted(o['name'] for o in enemy['objects'].values() if o['zone'] == 'library'), ['Forest', 'Ornithopter', 'Sol Ring'])
        self.assertIsNone(enemy['decision'])
        me.answer({'kind': 'choose', 'answer': {'objects': [ring]}})
        p = me.prompt()
        self.assertEqual(p['decision']['among'], [ornithopter])
        self.assertEqual(p['objects'][ring]['zone'], 'library', 'all choices precede the shared move')
        me.answer({'kind': 'choose', 'answer': {'objects': [ornithopter]}})
        p = me.prompt()
        self.assertEqual(len(p['decision']['among']), 1)
        self.assertEqual(p['objects'][ring]['zone'], 'library')
        me.answer({'kind': 'choose', 'answer': {'objects': []}})
        p = me.prompt()
        self.assertEqual(len(self.named(p, 'Sol Ring', zone='hand')), 1)
        self.assertEqual(len(self.named(p, 'Ornithopter', zone='hand')), 1)
        self.assertEqual(p['players'][0]['library'], 1)
        self.assertEqual(p['stack'], [])
        self.assertEqual(p['decision']['kind'], 'priority')
        self.assertNotIn('Forest', [o['name'] for o in them.prompt()['objects'].values() if o['zone'] == 'library'])
