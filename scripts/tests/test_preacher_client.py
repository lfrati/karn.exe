"""Preacher's life-ranked attack triggers through explicit player decisions."""
import os
import unittest

import play
import test_player_prompt_client as fixtures


class PreacherClientTests(unittest.TestCase):
    new = fixtures.PlayerPromptClientTests.new
    waiting = fixtures.PlayerPromptClientTests.waiting
    named = staticmethod(fixtures.PlayerPromptClientTests.named)
    close_engine = fixtures.PlayerPromptClientTests.close_engine

    @classmethod
    def setUpClass(cls):
        fixtures.PlayerPromptClientTests.setUpClass()

    def setUp(self):
        self.engine = play.Engine(executable=os.environ.get('GAME_SERVE_REVIEW'))
        self.addCleanup(self.close_engine)

    def attackers(self):
        for _ in range(100):
            seat = self.waiting()
            if seat.last['decision']['kind'] == 'attackers':
                return seat
            self.assertEqual(seat.last['decision']['kind'], 'priority')
            seat.answer({'kind': 'pass'})
        self.fail('no attack declaration')

    def clear_stack(self):
        for _ in range(20):
            seat = self.waiting()
            if not seat.last['stack']:
                return
            self.assertEqual(seat.last['decision']['kind'], 'priority')
            seat.answer({'kind': 'pass'})
        self.fail('stack did not clear')

    def test_tied_player_attack_offers_both_triggers_and_while_stays_true_for_that_event(self):
        # Pinned Preacher rulings 2023-11-10; CR 603.2/4: the attack already
        # triggered both abilities. Losing the life lead in response cancels neither.
        me, them = self.new({'battlefield': ['Preacher of the Schism', 'Mountain'], 'hand': ['Lightning Bolt']}, {})
        self.assertIs(self.attackers(), me)
        p = me.prompt()
        source = self.named(p, 'Preacher of the Schism')[0]
        me.answer({'kind': 'attack', 'attacks': [[source, 'p1']]})
        p = me.prompt()
        self.assertEqual(p['decision']['kind'], 'order')
        self.assertEqual(len(p['decision']['abilities']), 2)
        me.answer({'kind': 'order', 'order': [0, 1]})
        p = me.prompt()
        self.assertEqual(len(p['stack']), 2)
        me.answer({'kind': 'cast', 'card': self.named(p, 'Lightning Bolt')[0], 'targets': [['p0']],
                   'plan': [{'ability': f"{self.named(p, 'Mountain')[0]}.a0"}], 'mana': {'R': 1}})
        self.clear_stack()
        p = me.prompt()
        self.assertEqual(p['players'][0]['life'], 16)
        self.assertEqual(len(self.named(p, 'Island', zone='hand')), 1)
        vampire = self.named(p, 'Vampire Token', zone='battlefield')
        self.assertEqual(len(vampire), 1)
        self.assertEqual((p['objects'][vampire[0]]['power'], p['objects'][vampire[0]]['toughness']), (1, 1))
        self.assertIn('lifelink', ' '.join(p['objects'][vampire[0]]['text']).lower())

    def test_a_planeswalker_defender_does_not_count_as_attacking_its_life_leading_controller(self):
        # Pinned Preacher ruling; CR 508.1b: attacking a planeswalker differs
        # from attacking its controller, even when every player is tied for most life.
        me, them = self.new({'battlefield': ['Preacher of the Schism']}, {'battlefield': ['Jace Beleren']})
        self.assertIs(self.attackers(), me)
        p = me.prompt()
        me.answer({'kind': 'attack', 'attacks': [[self.named(p, 'Preacher of the Schism')[0], self.named(p, 'Jace Beleren')[0]]]})
        self.assertEqual(len(me.prompt()['stack']), 1)
        self.clear_stack()
        p = me.prompt()
        self.assertEqual(p['players'][0]['life'], 19)
        self.assertEqual(len(self.named(p, 'Island', zone='hand')), 1)
        self.assertEqual(self.named(p, 'Vampire Token', zone='battlefield'), [])

    def test_undercover_butler_uses_the_same_life_leader_predicate_for_its_blocking_restriction(self):
        # CR 603.2, 509.1b: Butler's matching attack trigger resolves before
        # blockers are declared. The same life-extremum predicate includes ties.
        self.engine.send({'cmd': 'new', 'ask_forced': True, 'players': [
            {'battlefield': ['Undercover Butler'], 'library': ['Island'] * 5},
            {'battlefield': ['Grizzly Bears'], 'library': ['Island'] * 5}]})
        self.seats = [play.Seat(self.engine, i) for i in range(2)]
        me, them = self.seats
        self.assertIs(self.attackers(), me)
        p = me.prompt()
        me.answer({'kind': 'attack', 'attacks': [[self.named(p, 'Undercover Butler')[0], 'p1']]})
        self.assertEqual(len(me.prompt()['stack']), 1)
        self.clear_stack()
        for _ in range(20):
            seat = self.waiting()
            if seat.last['decision']['kind'] == 'blockers':
                break
            self.assertEqual(seat.last['decision']['kind'], 'priority')
            seat.answer({'kind': 'pass'})
        else:
            self.fail('no blocker declaration')
        p = them.prompt()
        bear = self.named(p, 'Grizzly Bears')[0]
        butler = self.named(p, 'Undercover Butler')[0]
        with self.assertRaisesRegex(play.Refused, "can't block"):
            them.answer({'kind': 'block', 'blocks': [[bear, butler]]})
        them.prompt()
        them.answer({'kind': 'block', 'blocks': []})
