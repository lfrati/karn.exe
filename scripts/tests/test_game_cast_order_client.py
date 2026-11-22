"""Actual Once Upon a Time: explicit free route, private look, and honest setup history."""
import os
import unittest

import play
import test_player_prompt_client as fixtures


class GameCastOrderClientTests(unittest.TestCase):
    new = fixtures.PlayerPromptClientTests.new
    waiting = fixtures.PlayerPromptClientTests.waiting
    named = staticmethod(fixtures.PlayerPromptClientTests.named)
    close_engine = fixtures.PlayerPromptClientTests.close_engine
    refused = fixtures.PlayerPromptClientTests.refused
    pass_until_asked = fixtures.PlayerPromptClientTests.pass_until_asked

    @classmethod
    def setUpClass(cls):
        fixtures.PlayerPromptClientTests.setUpClass()

    def setUp(self):
        self.engine = play.Engine(executable=os.environ.get('GAME_SERVE_REVIEW'))
        self.addCleanup(self.close_engine)

    def test_first_free_cast_looks_privately_and_only_reveals_the_chosen_card(self):
        # Oracle; CR 118.9, 601.2i, 701.20a: declining a failed payment has not
        # cast a spell. Looking at five never shows the five to the other seat.
        me, them = self.new({'hand': ['Once Upon a Time', 'Once Upon a Time'],
                             'library': ['Counterspell', 'Forest', 'Grizzly Bears', 'Counterspell', 'Counterspell', 'Counterspell']}, {})
        card = self.named(me.prompt(), 'Once Upon a Time', zone='hand')[0]
        self.assertIn('your mana pool cannot pay that cost', self.refused(me, {'cmd': 'answer', 'answer': {'kind': 'cast', 'card': card}}))
        me.answer({'kind': 'cast', 'card': card, 'cost': {'alternative': 0}})
        self.pass_until_asked()
        p = me.prompt()
        self.assertEqual(p['decision']['kind'], 'may')
        self.assertEqual(len([o for o in p['objects'].values() if o['zone'] == 'library']), 5)
        self.assertFalse([o for o in them.prompt()['objects'].values() if o['zone'] == 'library'])
        me.answer({'kind': 'choose', 'answer': 'yes'})
        p = me.prompt()
        self.assertEqual((p['decision']['kind'], p['decision']['count']), ('objects', 1))
        self.assertEqual(sorted(p['objects'][o]['name'] for o in p['decision']['among']), ['Forest', 'Grizzly Bears'])
        bear = self.named(p, 'Grizzly Bears', zone='library')[0]
        me.answer({'kind': 'choose', 'answer': {'objects': [bear]}})
        p = me.prompt()
        self.assertEqual(len(self.named(p, 'Grizzly Bears', zone='hand')), 1)
        self.assertEqual(p['players'][0]['library'], 5)
        theirs = them.prompt()
        revealed = [e for e in theirs['events'] if e['event'] == 'revealed']
        self.assertEqual(len(revealed), 1)
        self.assertEqual(len(revealed[0]['cards']), 1)
        self.assertFalse([o for o in theirs['objects'].values() if o['zone'] == 'library'])
        second = self.named(p, 'Once Upon a Time', zone='hand')[0]
        self.assertIn('the condition for that alternative cost is not true', self.refused(me, {
            'cmd': 'answer', 'answer': {'kind': 'cast', 'card': second, 'cost': {'alternative': 0}}}))

    @staticmethod
    def position():
        return {'timing': {'turn': 3, 'step': 'precombat_main', 'active': 'A'},
                'next': {'kind': 'priority', 'chooser': 'A'},
                'players': {'A': {'life': 20, 'mana': {'C': 1, 'G': 1}}, 'B': {'life': 20, 'mana': {}}},
                'zones': {'A.library': [], 'A.hand': ['once'], 'A.graveyard': [],
                          'B.library': [], 'B.hand': [], 'B.graveyard': [],
                          'battlefield': [], 'stack': [], 'exile': [], 'command': []},
                'objects': {'once': {'name': 'Once Upon a Time', 'owner': 'A', 'controller': 'A',
                                     'tapped': False, 'counters': {}, 'damage': 0}},
                'history': {'turn_structure': 'ordinary_A_first', 'pending': 'none', 'effects': 'intrinsic_only',
                            'turn_events': 'empty', 'previous_turn_spells': 0,
                            'land_plays_this_turn': {'A': 0, 'B': 0}, 'ability_uses': 'none',
                            'consecutive_passes': 0, 'control_since': {}},
                'causal': {'linked_exile': [], 'prevention': [], 'continuous_effects': [], 'base_controllers': {}}}

    def test_positioned_free_route_requires_known_total_but_paid_route_stays_usable(self):
        for known in [False, True]:
            with self.subTest(known=known):
                position = self.position()
                if known:
                    position['history']['spells_cast_this_game'] = {'A': 0, 'B': None}
                self.engine.send({'cmd': 'new', 'position': position})
                self.seats = [play.Seat(self.engine, i) for i in range(2)]
                me, them = self.seats
                p = me.prompt()
                card = self.named(p, 'Once Upon a Time')[0]
                request = {'kind': 'cast', 'card': card, 'cost': {'alternative': 0}}
                if known:
                    me.answer(request)
                else:
                    with self.assertRaises(play.Refused) as refusal:
                        me.answer(request)
                    self.assertEqual(refusal.exception.refusal_class, 'unsupported')
                    self.assertIn('known whole-game cast count', str(refusal.exception))
                    me.prompt()
                    me.answer({'kind': 'cast', 'card': card, 'mana': {'C': 1, 'G': 1}})
                self.assertEqual(len(me.prompt()['stack']), 1)
