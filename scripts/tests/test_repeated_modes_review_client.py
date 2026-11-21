"""Seat checks for repeated modes, copy targets and per-instance payments."""
import os
import unittest

import play
import test_player_prompt_client as fixtures


class RepeatedModesReviewClientTests(unittest.TestCase):
    new = fixtures.PlayerPromptClientTests.new
    waiting = fixtures.PlayerPromptClientTests.waiting
    named = staticmethod(fixtures.PlayerPromptClientTests.named)
    close_engine = fixtures.PlayerPromptClientTests.close_engine
    refused = fixtures.PlayerPromptClientTests.refused

    @classmethod
    def setUpClass(cls):
        if not os.environ.get('REPEATED_MODES_REVIEW_ENGINE'):
            fixtures.PlayerPromptClientTests.setUpClass()

    def setUp(self):
        self.engine = play.Engine(executable=os.environ.get('REPEATED_MODES_REVIEW_ENGINE'))
        self.addCleanup(self.close_engine)

    def setup(self, *players):
        seats = self.new(*players)
        return seats

    def cast(self, seat, name, mana, *, spend, **fields):
        p = seat.prompt()
        lands = []
        for land, count in mana:
            lands.extend([o for o in self.named(p, land, controller=seat.label, zone='battlefield')
                          if not p['objects'][o]['tapped']][:count])
        return seat.answer({'kind': 'cast', 'card': self.named(p, name, zone='hand')[0],
                            'plan': [{'ability': f'{o}.a0'} for o in lands], 'mana': spend, **fields})

    def clear_stack(self):
        for _ in range(30):
            seat = self.waiting()
            if not seat.last['stack']:
                return
            self.assertEqual(seat.last['decision']['kind'], 'priority', seat.last['decision'])
            seat.answer({'kind': 'pass'})
        self.fail('stack did not clear')

    def choice(self, kind):
        for _ in range(20):
            seat = self.waiting()
            if seat.last['decision']['kind'] == kind:
                return seat
            self.assertEqual(seat.last['decision']['kind'], 'priority', seat.last['decision'])
            self.assertTrue(seat.last['stack'])
            seat.answer({'kind': 'pass'})
        self.fail(f'no {kind} choice')

    def test_a_copy_keeps_three_modes_and_retargets_only_two_of_three_groups(self):
        # Rules: 700.2d, 700.2g, 707.10, 115.7d, 608.2b
        me, them = self.setup({'hand': ['Fiery Confluence'],
                               'battlefield': ['Mountain'] * 4 + ['Sol Ring', 'Spellbook', 'Ornithopter']},
                              {'hand': ['Twincast'], 'battlefield': ['Island'] * 2 + ['Sol Ring']})
        p = me.prompt()
        enemy_ring = self.named(p, 'Sol Ring', controller='p1')[0]
        book = self.named(p, 'Spellbook')[0]
        self.cast(me, 'Fiery Confluence', [('Mountain', 4)], modes=[2, 2, 2],
                  targets=[[enemy_ring], [enemy_ring], [book]], spend={'R': 4})
        me.answer({'kind': 'pass'})
        p = them.prompt()
        original = p['stack'][0]
        self.cast(them, 'Twincast', [('Island', 2)], targets=[[original]], spend={'U': 2})
        self.assertIs(self.choice('retarget'), them)
        p = them.last
        d = p['decision']
        enemy_ring = self.named(p, 'Sol Ring', controller='p1')[0]
        book = self.named(p, 'Spellbook')[0]
        my_ring = self.named(p, 'Sol Ring', controller='p0')[0]
        ornithopter = self.named(p, 'Ornithopter')[0]
        self.assertEqual(d['original'], [[enemy_ring], [enemy_ring], [book]])
        self.assertEqual(len(d['slots']), 3)
        self.assertIsNone(me.prompt()['decision'], 'only the copy controller chooses its targets')
        request = p['request_id']
        self.assertTrue(self.refused(them, {'cmd': 'answer', 'answer':
                                           {'kind': 'choose', 'answer': {'targets': [[my_ring]]}}}))
        self.assertEqual(them.prompt()['request_id'], request)
        them.answer({'kind': 'choose', 'answer': {'targets': [[my_ring], [ornithopter], [book]]}})
        p = them.prompt()
        self.assertEqual(len(p['stack']), 2)
        copy = p['stack'][-1]
        self.assertEqual([m['index'] for m in p['objects'][copy]['occurrence']['modes']], [2, 2, 2])
        self.assertEqual([m['index'] for m in p['objects'][original]['occurrence']['modes']], [2, 2, 2])
        self.assertNotEqual(copy, original)
        self.clear_stack()
        p = me.prompt()
        for name, owner in [('Sol Ring', 'p0'), ('Spellbook', 'p0'), ('Ornithopter', 'p0'), ('Sol Ring', 'p1')]:
            self.assertEqual(len(self.named(p, name, controller=owner, zone='graveyard')), 1)
        self.assertEqual([x['life'] for x in p['players']], [20, 20])

    def test_each_counter_payment_is_separate_and_unpaid_second_payment_counters(self):
        # Rules: 700.2d, 118.3, 118.12, 608.2c
        for available, decline_first in [(3, False), (6, False), (6, True)]:
            with self.subTest(available=available, decline_first=decline_first):
                me, them = self.setup({'hand': ['Mystic Confluence'], 'battlefield': ['Island'] * 5},
                                      {'hand': ['Lightning Bolt'], 'battlefield': ['Mountain'] * (available + 1)})
                me.answer({'kind': 'pass'})
                self.cast(them, 'Lightning Bolt', [('Mountain', 1)], targets=[['p0']], spend={'R': 1})
                them.answer({'kind': 'pass'})
                p = me.prompt()
                bolt = p['stack'][0]
                self.cast(me, 'Mystic Confluence', [('Island', 5)], modes=[0, 2, 0], targets=[[bolt], [bolt]], spend={'U': 5})
                self.assertIs(self.choice('pay'), them)
                p = them.last
                self.assertEqual(p['decision']['cost'], ['3'])
                self.assertEqual(p['decision']['for']['source_name'], 'Mystic Confluence')
                self.assertIsNone(me.prompt()['decision'])
                first_request = p['request_id']
                if decline_first:
                    them.answer({'kind': 'choose', 'answer': 'no'})
                else:
                    them.answer({'kind': 'choose', 'answer': {'pay': {'mana': {'R': 3}, 'plan': [{'ability': f'{o}.a0'} for o in self.named(p, 'Mountain') if not p['objects'][o]['tapped']][:3]}}})
                    if available == 6:
                        p = them.prompt()
                        self.assertEqual(p['decision']['kind'], 'pay')
                        self.assertGreater(p['request_id'], first_request)
                        self.assertEqual(p['decision']['cost'], ['3'])
                        self.assertEqual(len([o for o in self.named(p, 'Mountain') if p['objects'][o]['tapped']]), 4)
                        them.answer({'kind': 'choose', 'answer': {'pay': {'mana': {'R': 3}, 'plan': [{'ability': f'{o}.a0'} for o in self.named(p, 'Mountain') if not p['objects'][o]['tapped']][:3]}}})
                    else:
                        p = them.prompt()
                        self.assertEqual(p['decision']['kind'], 'pay')
                        self.assertIn('118.3a', self.refused(them, {'cmd': 'answer', 'answer':
                            {'kind': 'choose', 'answer': {'pay': {'plan': []}}}}))
                        after = them.prompt()
                        for key in ('request_id', 'objects', 'players', 'decision', 'stack'):
                            self.assertEqual(after[key], p[key], 'a refused payment changes nothing (733.1)')
                        them.answer({'kind': 'choose', 'answer': 'no'})
                # Declining the first payment counters Bolt; its later counter mode
                # skips that absent target. Otherwise each mode requests its own payment.
                p = self.waiting().last
                self.assertEqual(p['decision']['kind'], 'priority')
                self.assertEqual(len(p['players'][0]['hand']) if 'hand' in p['players'][0] else p['players'][0]['hand_size'], 1)
                self.clear_stack()
                p = me.prompt()
                self.assertEqual(p['players'][0]['life'], 17 if available == 6 and not decline_first else 20)
                self.assertEqual(len(self.named(p, 'Lightning Bolt', zone='graveyard')), 1)

    def test_repeated_mode_multisets_execute_and_absent_artifact_targets_are_refused(self):
        # Rules: 700.2a, 700.2d, 601.2c: each repeated target mode needs its own target.
        for modes in ([0, 0, 0], [0, 0, 1], [0, 1, 1], [1, 1, 1]):
            with self.subTest(modes=modes):
                me, them = self.setup({'hand': ['Fiery Confluence'], 'battlefield': ['Mountain'] * 4}, {})
                p = me.prompt()
                card = self.named(p, 'Fiery Confluence')[0]
                self.assertTrue(self.refused(me, {'cmd': 'answer', 'answer':
                    {'kind': 'cast', 'card': card, 'modes': [2, 2, 2], 'targets': []}}))
                self.cast(me, 'Fiery Confluence', [('Mountain', 4)], modes=modes, spend={'R': 4})
                p = me.prompt()
                self.assertEqual([m['index'] for m in p['objects'][p['stack'][0]]['occurrence']['modes']], modes)
                self.clear_stack()
                self.assertEqual(them.prompt()['players'][1]['life'], 20 - 2 * modes.count(1))

    def test_copy_can_keep_one_illegal_repeated_target_while_changing_the_other_groups(self):
        # Rules: 707.10c, 700.2d, 700.2g, 400.7, 608.2b
        me, them = self.setup({'hand': ['Fiery Confluence', 'Unsummon', 'Twincast'],
                               'battlefield': ['Mountain'] * 4 + ['Island'] * 3 + ['Sol Ring', 'Spellbook', 'Ornithopter']},
                              {'battlefield': ['Ornithopter']})
        p = me.prompt()
        enemy = self.named(p, 'Ornithopter', controller='p1')[0]
        book = self.named(p, 'Spellbook')[0]
        self.cast(me, 'Fiery Confluence', [('Mountain', 4)], modes=[2, 2, 2],
                  targets=[[enemy], [enemy], [book]], spend={'R': 4})
        original = me.last['stack'][0]
        self.cast(me, 'Unsummon', [('Island', 1)], targets=[[enemy]], spend={'U': 1})
        me.answer({'kind': 'pass'})
        them.prompt()
        them.answer({'kind': 'pass'})
        p = me.prompt()
        self.assertEqual(p['stack'], [original])
        self.assertNotIn(enemy, p['battlefield'])
        self.cast(me, 'Twincast', [('Island', 2)], targets=[[original]], spend={'U': 2})
        self.assertIs(self.choice('retarget'), me)
        p = me.last
        self.assertEqual(p['decision']['original'], [[enemy], [enemy], [book]])
        ring = self.named(p, 'Sol Ring')[0]
        mine = self.named(p, 'Ornithopter', controller='p0', zone='battlefield')[0]
        # The second group keeps its exact now-illegal identity; it is not a new target.
        me.answer({'kind': 'choose', 'answer': {'targets': [[ring], [enemy], [mine]]}})
        p = me.prompt()
        copy = p['stack'][-1]
        self.assertEqual([m['index'] for m in p['objects'][copy]['occurrence']['modes']], [2, 2, 2])
        self.clear_stack()
        p = me.prompt()
        for name in ['Sol Ring', 'Spellbook', 'Ornithopter']:
            self.assertEqual(len(self.named(p, name, controller='p0', zone='graveyard')), 1)
        self.assertEqual(p['players'][1]['hand_size'], 1)


if __name__ == '__main__':
    unittest.main()
