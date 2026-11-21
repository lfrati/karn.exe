"""Seat checks for batches, participant keys and once-per-turn limits."""
import os
import unittest

import play
import test_player_prompt_client as fixtures


class OneOrMoreReviewClientTests(unittest.TestCase):
    new = fixtures.PlayerPromptClientTests.new
    waiting = fixtures.PlayerPromptClientTests.waiting
    named = staticmethod(fixtures.PlayerPromptClientTests.named)
    close_engine = fixtures.PlayerPromptClientTests.close_engine

    @classmethod
    def setUpClass(cls):
        if not os.environ.get('ONE_OR_MORE_REVIEW_ENGINE'):
            fixtures.PlayerPromptClientTests.setUpClass()

    def setUp(self):
        self.engine = play.Engine(executable=os.environ.get('ONE_OR_MORE_REVIEW_ENGINE'))
        self.addCleanup(self.close_engine)

    def until(self, kind=None, part=None):
        for _ in range(80):
            seat = self.waiting()
            p, d = seat.last, seat.last['decision']
            if (kind is None or d['kind'] == kind) and (part is None or p['part'] == part):
                return seat
            if d['kind'] == 'order':
                seat.answer({'kind': 'order', 'order': list(range(len(d['ordered'])))})
            elif d['kind'] == 'pay':
                seat.answer({'kind': 'choose', 'answer': 'no'})
            else:
                self.assertEqual(d['kind'], 'priority', d)
                seat.answer({'kind': 'pass'})
        self.fail(f'never reached {kind} / {part}')

    def clear_stack(self):
        for _ in range(50):
            seat = self.waiting()
            d = seat.last['decision']
            if d['kind'] == 'order':
                seat.answer({'kind': 'order', 'order': list(range(len(d['ordered'])))})
            elif not seat.last['stack']:
                return
            else:
                self.assertEqual(d['kind'], 'priority', d)
                seat.answer({'kind': 'pass'})
        self.fail('stack did not clear')

    def cast(self, seat, name, lands=(), *, spend, **fields):
        p = seat.prompt()
        mana = []
        for name_land, count in lands:
            mana.extend([o for o in self.named(p, name_land, controller=seat.label, zone='battlefield')
                         if not p['objects'][o]['tapped']][:count])
        return seat.answer({'kind': 'cast', 'card': self.named(p, name, zone='hand')[0],
                            'plan': [{'ability': f'{o}.a0'} for o in mana], 'mana': spend, **fields})

    def test_face_breaker_batches_by_player_and_separates_damage_steps(self):
        # Rules: 603.2c, 510.2, 510.4, 702.4b
        me, p1, p2 = self.new({'battlefield': ['Professional Face-Breaker', 'Fencing Ace',
                                             'Grizzly Bears', 'Grizzly Bears']}, {}, {})
        self.assertIs(self.until(kind='attackers'), me)
        p = me.last
        ace = self.named(p, 'Fencing Ace')[0]
        a, b = self.named(p, 'Grizzly Bears')
        me.answer({'kind': 'attack', 'attacks': [[ace, 'p1'], [a, 'p1'], [b, 'p2']]})
        self.until(kind='priority', part='EndOfCombat')
        p = me.prompt()
        self.assertEqual([x['life'] for x in p['players']], [20, 16, 18])
        self.assertEqual(len(self.named(p, 'Treasure Token', zone='battlefield')), 3,
                         'one in first strike; one per damaged player in the regular step')

    def test_prevented_combat_damage_does_not_make_an_empty_batch_trigger(self):
        # Rules: 615.6, 603.2c, 510.2
        me, them = self.new({'battlefield': ['Professional Face-Breaker', 'Fencing Ace', 'Grizzly Bears']},
                            {'hand': ['Holy Day'], 'battlefield': ['Plains']})
        me.answer({'kind': 'pass'})
        self.cast(them, 'Holy Day', [('Plains', 1)], spend={'W': 1})
        self.clear_stack()
        self.assertIs(self.until(kind='attackers'), me)
        p = me.last
        attackers = [self.named(p, n)[0] for n in ['Fencing Ace', 'Grizzly Bears']]
        me.answer({'kind': 'attack', 'attacks': [[o, 'p1'] for o in attackers]})
        self.until(kind='priority', part='EndOfCombat')
        p = me.prompt()
        self.assertEqual([x['life'] for x in p['players']], [20, 20])
        self.assertEqual(self.named(p, 'Treasure Token', zone='battlefield'), [])

    def test_morbid_opportunist_batches_deaths_and_takes_only_one_use_per_turn(self):
        # Rules: 603.2c, 704.3
        me, them = self.new({'hand': ['Pyroclasm', 'Ornithopter', 'Lightning Bolt'],
                             'battlefield': ['Morbid Opportunist', 'Grizzly Bears'] + ['Mountain'] * 3},
                            {'battlefield': ['Grizzly Bears']})
        self.cast(me, 'Pyroclasm', [('Mountain', 2)], spend={'R': 2})
        self.clear_stack()
        p = me.prompt()
        self.assertEqual(len(self.named(p, 'Island', zone='hand')), 1)
        self.assertEqual(len(self.named(p, 'Grizzly Bears', zone='graveyard')), 2)
        self.cast(me, 'Ornithopter', spend={})
        self.clear_stack()
        thopter = self.named(me.prompt(), 'Ornithopter', zone='battlefield')[0]
        self.cast(me, 'Lightning Bolt', [('Mountain', 1)], targets=[[thopter]], spend={'R': 1})
        self.clear_stack()
        self.assertEqual(len(self.named(me.prompt(), 'Island', zone='hand')), 1, 'later death uses no second trigger')

    def test_two_tokens_are_one_entry_batch_for_each_once_per_turn_ability(self):
        # Rules: 603.2c, 608.2c
        me, them = self.new({'hand': ['Raise the Alarm', 'Ornithopter'],
                             'battlefield': ['Welcoming Vampire', "Tocasia's Welcome", 'Plains', 'Plains']}, {})
        self.cast(me, 'Raise the Alarm', [('Plains', 2)], spend={'W': 2})
        self.clear_stack()
        p = me.prompt()
        self.assertEqual(len(self.named(p, 'Soldier Token', zone='battlefield')), 2)
        self.assertEqual(len(self.named(p, 'Island', zone='hand')), 2, 'one draw per ability, each with the whole entry batch')
        self.cast(me, 'Ornithopter', spend={})
        self.clear_stack()
        self.assertEqual(len(self.named(me.prompt(), 'Island', zone='hand')), 2, 'later entry cannot take another use')

    def test_known_consumers_keep_their_unrelated_refusals(self):
        for name, reason in [('Neyith of the Dire Hunt', 'grammar does not read'),
                             ('Sidisi, Brood Tyrant', 'trigger: PutIntoGraveyard')]:
            with self.subTest(card=name), self.assertRaises(SystemExit) as refused:
                self.new({'battlefield': [name]}, {})
            self.assertIn(reason, str(refused.exception))

    def test_deeproot_pilgrimage_sees_declaration_taps_as_one_batch(self):
        # Rules: 508.1f, 508.2b, 603.2c
        me, them = self.new({'battlefield': ['Deeproot Pilgrimage', 'Merfolk of the Pearl Trident',
                                           'Merfolk of the Pearl Trident']}, {})
        self.assertIs(self.until(kind='attackers'), me)
        attackers = self.named(me.last, 'Merfolk of the Pearl Trident')
        me.answer({'kind': 'attack', 'attacks': [[o, 'p1'] for o in attackers]})
        self.clear_stack()
        p = me.prompt()
        tokens = self.named(p, 'Merfolk Token', zone='battlefield')
        self.assertEqual(len(tokens), 1, 'the two declared attackers became tapped together')
        self.assertTrue(all(p['objects'][o]['tapped'] for o in attackers))
        self.assertFalse(p['objects'][tokens[0]]['tapped'])


if __name__ == '__main__':
    unittest.main()
