"""Real players supply scoped loyalty activations and non-generic restricted mana explicitly."""
import os
import unittest

import play
import test_player_prompt_client as fixtures


class SourcePermissionsClientTests(unittest.TestCase):
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

    def clear_stack(self):
        for _ in range(20):
            seat = self.waiting()
            if not seat.last['stack']:
                return
            self.assertEqual(seat.last['decision']['kind'], 'priority')
            seat.answer({'kind': 'pass'})
        self.fail('stack did not clear')

    def test_emperor_can_flash_in_and_use_only_her_own_loyalty_permission(self):
        # Oracle and CR 606.3, 702.8a: entry permits instant timing, once per turn.
        me, them = self.new({'hand': ['The Wandering Emperor'],
                             'battlefield': ['Plains'] * 4 + ['Jace Beleren']}, {})
        for _ in range(120):
            seat = self.waiting()
            if seat.last['active'] == 'p1' and seat.last['part'] == 'PrecombatMain' and seat is me:
                break
            self.assertEqual(seat.last['decision']['kind'], 'priority')
            seat.answer({'kind': 'pass'})
        else:
            self.fail('no opponent main phase')
        p = me.last
        me.answer({'kind': 'cast', 'card': self.named(p, 'The Wandering Emperor')[0],
                   'plan': [{'ability': f'{o}.a0'} for o in self.named(p, 'Plains')], 'mana': {'W': 4}})
        self.clear_stack()
        self.waiting().answer({'kind': 'pass'})
        p = me.prompt()
        old = self.named(p, 'Jace Beleren')[0]
        self.assertIn('606.3', self.refused(me, {'cmd': 'answer', 'answer': {'kind': 'activate', 'ability': f'{old}.a0'}}))
        p = me.prompt()
        emperor = self.named(p, 'The Wandering Emperor', zone='battlefield')[0]
        me.answer({'kind': 'activate', 'ability': f'{emperor}.a3'})
        self.clear_stack()
        self.waiting().answer({'kind': 'pass'})
        p = me.prompt()
        samurai = [o for o in p['objects'].values() if o['name'] == 'Samurai Token' and o['zone'] == 'battlefield']
        self.assertEqual(len(samurai), 1)
        self.assertEqual((samurai[0]['power'], samurai[0]['toughness']), (2, 2))
        self.assertEqual(p['objects'][emperor]['counters']['loyalty'], 2)
        self.assertIn('already been activated this turn', self.refused(me, {'cmd': 'answer', 'answer': {'kind': 'activate', 'ability': f'{emperor}.a3'}}))

    def test_jegantha_units_are_supplied_and_cannot_fill_a_generic_cost(self):
        # CR 106.6, 107.4b, 601.2h, 733.1: a refused supplied payment rolls back
        # its mana plan. A restricted green plus an ordinary green can pay {1}{G}.
        me, them = self.new({'hand': ['Grizzly Bears'],
                             'battlefield': ['Jegantha, the Wellspring', 'Forest']}, {})
        p = me.prompt()
        elk = self.named(p, 'Jegantha, the Wellspring')[0]
        forest = self.named(p, 'Forest')[0]
        bears = self.named(p, 'Grizzly Bears')[0]
        self.assertIn('does not pay that cost', self.refused(me, {'cmd': 'answer', 'answer': {
            'kind': 'cast', 'card': bears, 'plan': [{'ability': f'{elk}.a1'}], 'mana': {'restricted_after': [0, 4]}}}))
        p = me.prompt()
        self.assertFalse(p['objects'][elk]['tapped'])
        self.assertEqual(p['players'][0]['pool']['restricted'], [])
        me.answer({'kind': 'cast', 'card': bears, 'plan': [{'ability': f'{elk}.a1'}, {'ability': f'{forest}.a0'}],
                   'mana': {'G': 1, 'restricted_after': [4]}})
        p = me.prompt()
        self.assertTrue(p['objects'][elk]['tapped'])
        self.assertTrue(p['objects'][forest]['tapped'])
        self.assertEqual([unit['mana'] for unit in p['players'][0]['pool']['restricted']], ['W', 'U', 'B', 'R'])
        self.clear_stack()
        self.assertEqual(len(self.named(me.prompt(), 'Grizzly Bears', zone='battlefield')), 1)
