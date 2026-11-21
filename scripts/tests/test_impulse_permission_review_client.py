"""Seat checks of 611.2a permissions, 305.2/3 timing and 733.1 reversal."""
import os
import unittest

import play
import test_player_prompt_client as fixtures


class ImpulsePermissionReviewClientTests(unittest.TestCase):
    new = fixtures.PlayerPromptClientTests.new
    waiting = fixtures.PlayerPromptClientTests.waiting
    named = staticmethod(fixtures.PlayerPromptClientTests.named)
    close_engine = fixtures.PlayerPromptClientTests.close_engine
    refused = fixtures.PlayerPromptClientTests.refused

    @classmethod
    def setUpClass(cls):
        if not os.environ.get('IMPULSE_REVIEW_ENGINE'):
            fixtures.PlayerPromptClientTests.setUpClass()

    def setUp(self):
        self.engine = play.Engine(executable=os.environ.get('IMPULSE_REVIEW_ENGINE'))
        self.addCleanup(self.close_engine)

    def setup(self, *players):
        seats = self.new(*players)
        return seats

    def advance(self, turn, part, waiting='p0'):
        for _ in range(200):
            seat = self.waiting()
            p = seat.last
            if (p['turn'], p['part'], p['waiting_for']) == (turn, part, waiting) and not p['stack']:
                return seat
            d = p['decision']
            if d['kind'] == 'attackers':
                seat.answer({'kind': 'attack', 'attacks': []})
            else:
                self.assertEqual(d['kind'], 'priority', d)
                seat.answer({'kind': 'pass'})
        self.fail(f'never reached {turn} {part} {waiting}')

    def clear_stack(self):
        for _ in range(30):
            seat = self.waiting()
            if not seat.last['stack']:
                return
            self.assertEqual(seat.last['decision']['kind'], 'priority', seat.last['decision'])
            seat.answer({'kind': 'pass'})
        self.fail('stack did not clear')

    def cast(self, seat, name, mana, *, spend, **kwargs):
        p = seat.prompt()
        lands = []
        for land, count in mana:
            lands.extend([o for o in self.named(p, land, controller=seat.label, zone='battlefield')
                          if not p['objects'][o]['tapped']][:count])
        return seat.answer({'kind': 'cast', 'card': self.named(p, name)[0],
                            'plan': [{'ability': f'{o}.a0'} for o in lands], 'mana': spend, **kwargs})

    def test_act_on_impulse_has_public_labels_but_ordinary_costs_timing_and_land_limit(self):
        # The first player skips their initial draw; the last library card is on top.
        me, them = self.setup({'hand': ['Act on Impulse'], 'battlefield': ['Mountain'] * 5 + ['Forest'],
                               'library': ['Island'] * 6 + ['Grizzly Bears', 'Mountain', 'Lightning Bolt']}, {})
        self.cast(me, 'Act on Impulse', [('Mountain', 3)], spend={'R': 3})
        self.clear_stack()
        p = me.prompt()
        self.assertEqual({p['objects'][o]['name'] for o in p['exile']},
                         {'Lightning Bolt', 'Mountain', 'Grizzly Bears'})
        for seat in (me, them):
            q = seat.prompt()
            for o in q['exile']:
                self.assertEqual(q['objects'][o]['may_play'],
                                 {'player': 'p0', 'land': True, 'until': 'end_of_turn'})
        land = self.named(p, 'Mountain', zone='exile')[0]
        bolt = self.named(p, 'Lightning Bolt', zone='exile')[0]
        bear = self.named(p, 'Grizzly Bears', zone='exile')[0]
        # Permission does not waive mana costs; an ordinary unpaid announcement is refused.
        self.assertTrue(self.refused(me, {'cmd': 'answer', 'answer':
                                         {'kind': 'cast', 'card': bolt, 'targets': [['p1']]}}))
        me.prompt()
        me.answer({'kind': 'play', 'card': land})
        p = me.prompt()
        me.answer({'kind': 'pass'})
        them.prompt()
        other_bolt = self.named(them.last, 'Lightning Bolt', zone='exile')[0]
        self.assertTrue(self.refused(them, {'cmd': 'answer', 'answer':
                                           {'kind': 'cast', 'card': other_bolt, 'targets': [['p0']]}}))
        them.prompt()
        them.answer({'kind': 'pass'})
        self.advance(1, 'BeginningOfCombat')
        me.prompt()
        p = me.prompt()
        plan = [{'ability': f'{self.named(p, "Forest", zone="battlefield")[0]}.a0'},
                {'ability': f'{[o for o in self.named(p, "Mountain", zone="battlefield") if not p["objects"][o]["tapped"]][0]}.a0'}]
        error = self.refused(me, {'cmd': 'answer', 'answer':
                                 {'kind': 'cast', 'card': bear, 'plan': plan, 'mana': {'G': 1, 'R': 1}}})
        self.assertIn('117.1a', error)
        # The instant can still be paid for and cast during combat.
        me.prompt()
        self.cast(me, 'Lightning Bolt', [('Mountain', 1)], targets=[['p1']], spend={'R': 1})
        self.clear_stack()
        self.assertEqual(me.prompt()['players'][1]['life'], 17)
        self.advance(2, 'Upkeep')
        p = me.prompt()
        self.assertIn(bear, p['exile'])
        self.assertNotIn('may_play', p['objects'][bear])

    def test_inspired_tinkering_spans_the_next_turn_then_expires_with_cards_still_exiled(self):
        me, them = self.setup({'hand': ['Inspired Tinkering'], 'battlefield': ['Mountain'] * 6,
                               'library': ['Island'] * 8 + ['Forest', 'Shock', 'Mountain']}, {})
        self.cast(me, 'Inspired Tinkering', [('Mountain', 5)], spend={'R': 5})
        self.clear_stack()
        p = me.prompt()
        forest = self.named(p, 'Forest', zone='exile')[0]
        self.assertEqual(len(self.named(p, 'Treasure Token', zone='battlefield')), 3)
        for o in p['exile']:
            self.assertEqual(p['objects'][o]['may_play']['until'], 'end_of_their_next_turn')
        self.advance(2, 'Upkeep')
        p = me.prompt()
        self.assertEqual(p['objects'][forest]['may_play']['until'], 'end_of_their_next_turn')
        self.assertTrue(self.refused(me, {'cmd': 'answer', 'answer': {'kind': 'play', 'card': forest}}))
        me.prompt()
        self.cast(me, 'Shock', [('Mountain', 1)], targets=[['p1']], spend={'R': 1})
        self.clear_stack()
        self.assertEqual(me.prompt()['players'][1]['life'], 18)
        self.advance(3, 'PrecombatMain')
        p = me.prompt()
        self.assertEqual(p['objects'][forest]['may_play']['until'], 'end_of_turn')
        mountain = self.named(p, 'Mountain', zone='exile')[0]
        me.answer({'kind': 'play', 'card': mountain})
        me.prompt()
        self.assertIn('305.2', self.refused(me, {'cmd': 'answer', 'answer': {'kind': 'play', 'card': forest}}))
        self.advance(4, 'Upkeep')
        p = me.prompt()
        self.assertIn(forest, p['exile'])
        self.assertNotIn('may_play', p['objects'][forest])
        self.advance(5, 'PrecombatMain')
        self.assertTrue(self.refused(me, {'cmd': 'answer', 'answer': {'kind': 'play', 'card': forest}}))

    def test_elkin_bottle_end_step_activation_expires_before_its_controllers_upkeep_priority(self):
        me, them = self.setup({'battlefield': ['Elkin Bottle'] + ['Mountain'] * 3,
                               'library': ['Island'] * 8 + ['Lightning Bolt']}, {})
        self.advance(2, 'End')
        p = me.prompt()
        bottle = self.named(p, 'Elkin Bottle')[0]
        me.answer({'kind': 'activate', 'ability': f'{bottle}.a0', 'mana': {'R': 3},
                   'plan': [{'ability': f'{o}.a0'} for o in self.named(p, 'Mountain')]})
        self.clear_stack()
        p = me.prompt()
        bolt = self.named(p, 'Lightning Bolt', zone='exile')[0]
        self.assertEqual(p['objects'][bolt]['may_play'],
                         {'player': 'p0', 'land': True, 'until': 'their_next_upkeep'})
        self.advance(3, 'Upkeep')
        p = me.prompt()
        self.assertNotIn('may_play', p['objects'][bolt])
        self.assertTrue(self.refused(me, {'cmd': 'answer', 'answer':
                                         {'kind': 'cast', 'card': bolt, 'targets': [['p1']],
                                          'plan': [{'ability': f'{self.named(p, "Mountain")[0]}.a0'}]}}))

    def test_abbot_permission_survives_a_mana_payment_reversal_and_can_be_used_again(self):
        # 733.1 restores the exact original exile object, unlike another ordinary zone move.
        me, them = self.setup({'hand': ['Abbot of Keral Keep'],
                               'battlefield': ['Mountain'] * 2 + ['Empowered Autogenerator', 'Sol Ring'],
                               'library': ['Island'] * 8 + ['Grizzly Bears']}, {})
        self.cast(me, 'Abbot of Keral Keep', [('Mountain', 2)], spend={'R': 2})
        self.clear_stack()
        p = me.prompt()
        bear = self.named(p, 'Grizzly Bears', zone='exile')[0]
        auto = self.named(p, 'Empowered Autogenerator')[0]
        step = {'ability': f'{auto}.a1'}
        me.answer({'kind': 'cast', 'card': bear, 'plan': [step, step]})
        self.assertEqual(me.last['decision']['kind'], 'mana')
        me.answer({'kind': 'choose', 'answer': {'mana': ['G']}})
        self.assertEqual(me.last['decision']['kind'], 'payment')
        p = me.answer({'kind': 'reverse_payment'})
        self.assertIn(bear, p['exile'])
        self.assertEqual(p['objects'][bear]['may_play'],
                         {'player': 'p0', 'land': True, 'until': 'end_of_turn'})
        self.assertEqual(p['players'][0]['pool']['G'], 1)
        # Reannounce that same label, paying the remaining generic cost with Sol Ring.
        ring = self.named(p, 'Sol Ring')[0]
        me.answer({'kind': 'cast', 'card': bear, 'plan': [{'ability': f'{ring}.a0'}], 'mana': {'G': 1, 'C': 1}})
        self.clear_stack()
        p = me.prompt()
        self.assertNotIn(bear, p['exile'])
        self.assertEqual(len(self.named(p, 'Grizzly Bears', zone='battlefield')), 1)

    def test_impulse_uses_the_selected_modal_face_and_the_adventure_gets_a_new_permission(self):
        # 715.3d / 712.8: ordinary impulse permission permits either cast identity and
        # the modal land face, with their existing payment/entry procedures.
        me, them = self.setup({'hand': ['Act on Impulse'],
                               'battlefield': ['Mountain'] * 3 + ['Forest'] * 2,
                               'library': ['Island'] * 8 + ['Grizzly Bears', 'Curious Pair', 'Sink into Stupor']}, {})
        self.cast(me, 'Act on Impulse', [('Mountain', 3)], spend={'R': 3})
        self.clear_stack()
        p = me.prompt()
        modal = self.named(p, 'Sink into Stupor', zone='exile')[0]
        adventurer = self.named(p, 'Curious Pair', zone='exile')[0]
        me.answer({'kind': 'play', 'card': modal, 'face': 1})
        self.assertEqual(me.last['decision']['kind'], 'may', 'land entry still offers its life payment')
        me.answer({'kind': 'choose', 'answer': 'no'})
        p = me.prompt()
        spring = self.named(p, 'Soporific Springs', zone='battlefield')[0]
        self.assertTrue(p['objects'][spring]['tapped'])
        self.assertEqual(p['players'][0]['life'], 20)
        self.cast(me, 'Curious Pair', [('Forest', 1)], cast_as='adventure', spend={'G': 1})
        self.clear_stack()
        p = me.prompt()
        again = self.named(p, 'Curious Pair', zone='exile')[0]
        self.assertNotEqual(again, adventurer)
        self.assertEqual(p['objects'][again]['may_play'], {'player': 'p0', 'land': True})
        self.assertEqual(len(self.named(p, 'Food Token', zone='battlefield')), 1)
        self.advance(3, 'PrecombatMain')
        p = me.prompt()
        self.assertEqual(p['objects'][again]['may_play'], {'player': 'p0', 'land': True})
        self.assertTrue(self.refused(me, {'cmd': 'answer', 'answer':
                                         {'kind': 'cast', 'card': again, 'cast_as': 'adventure',
                                          'plan': [{'ability': f'{self.named(p, "Forest")[0]}.a0'}]}}))
        self.cast(me, 'Curious Pair', [('Forest', 2)], spend={'G': 2})
        self.clear_stack()
        self.assertEqual(len(self.named(me.prompt(), 'Curious Pair', zone='battlefield')), 1)


if __name__ == '__main__':
    unittest.main()
