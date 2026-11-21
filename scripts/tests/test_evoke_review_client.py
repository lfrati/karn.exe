"""Evoke through card text, alternative costs and labelled seat prompts."""
import os
import unittest
import play
import test_player_prompt_client as fixtures


class EvokeReviewClientTests(unittest.TestCase):
    new = fixtures.PlayerPromptClientTests.new
    waiting = fixtures.PlayerPromptClientTests.waiting
    named = staticmethod(fixtures.PlayerPromptClientTests.named)
    close_engine = fixtures.PlayerPromptClientTests.close_engine
    refused = fixtures.PlayerPromptClientTests.refused

    @classmethod
    def setUpClass(cls):
        if not os.environ.get('EVOKE_REVIEW_ENGINE'):
            fixtures.PlayerPromptClientTests.setUpClass()

    def setUp(self):
        self.engine = play.Engine(executable=os.environ.get('EVOKE_REVIEW_ENGINE'))
        self.addCleanup(self.close_engine)

    def pause(self):
        for _ in range(40):
            seat = self.waiting()
            if seat.last['decision']['kind'] != 'priority' or not seat.last['stack']:
                return seat
            seat.answer({'kind': 'pass'})
        self.fail('stack did not settle')

    def evoke_row(self, prompt, card):
        rows = prompt['objects'][card]['text']
        return next(i for i, text in enumerate(rows) if text.startswith('Evoke'))

    def ordered_entry(self, seat, first):
        d = seat.last['decision']
        self.assertEqual((d['kind'], d['reason'], d['chooser']), ('order', 'trigger_order', seat.label))
        self.assertEqual(len(d['ordered']), 2)
        sacrifice = next(i for i, a in enumerate(d['ordered']) if 'sacrifices' in a['text'])
        normal = 1 - sacrifice
        self.assertTrue('evoke cost was paid' in d['ordered'][sacrifice]['text'])
        self.assertTrue(all(a['kind'] == 'triggered' for a in d['ordered']))
        # Order is bottom to top: last selected resolves first.
        order = [normal, sacrifice] if first == 'sacrifice' else [sacrifice, normal]
        return seat.answer({'kind': 'order', 'order': order})

    def test_solitude_evoke_pays_by_a_private_white_card_choice(self):
        # Rules: 702.74a, 601.2b, 601.2h, 603.3b, 400.2
        for first in ('sacrifice', 'entry'):
            with self.subTest(first=first):
                me, them = self.new({'hand': ['Solitude', 'Swords to Plowshares', 'Path to Exile', 'Shock']},
                                    {'battlefield': ['Grizzly Bears']})
                p = me.prompt()
                card = self.named(p, 'Solitude')[0]
                row = self.evoke_row(p, card)
                # The normal cost cannot be paid; the caller chooses evoke explicitly.
                self.assertIn('refused', self.refused(me, {'cmd': 'answer', 'answer':
                    {'kind': 'cast', 'card': card}}))
                other = them.prompt()
                self.assertIsNone(other['decision'])
                self.assertFalse(self.named(other, 'Swords to Plowshares') + self.named(other, 'Path to Exile'))
                bad = self.named(p, 'Shock')[0]
                self.assertIn('refused', self.refused(me, {'cmd': 'answer', 'answer':
                    {'kind': 'cast', 'card': card, 'cost': {'alternative': row}, 'paying': [[bad]]}}))
                chosen = self.named(me.prompt(), 'Swords to Plowshares')[0]
                me.answer({'kind': 'cast', 'card': card, 'cost': {'alternative': row}, 'paying': [[chosen]]})
                self.assertIs(self.pause(), me)
                self.ordered_entry(me, first)
                self.assertEqual(me.last['decision']['kind'], 'targets')
                bear = self.named(me.last, 'Grizzly Bears')[0]
                me.answer({'kind': 'targets', 'targets': [[bear]]})
                self.pause()
                p = me.prompt()
                self.assertEqual(len(self.named(p, 'Solitude', zone='graveyard')), 1)
                self.assertEqual(len(self.named(p, 'Grizzly Bears', zone='exile')), 1)
                self.assertEqual(len(self.named(p, 'Swords to Plowshares', zone='exile')), 1)
                self.assertEqual(len(self.named(p, 'Path to Exile', zone='hand')), 1)
                self.assertEqual(p['players'][1]['life'], 22)

    def test_mulldrifter_evoke_and_hard_cast_use_different_costs_and_triggers(self):
        # Rules: 702.74a, 118.9c, 603.3b
        for evoke in (True, False):
            with self.subTest(evoke=evoke):
                me, them = self.new({'hand': ['Mulldrifter'], 'battlefield': ['Island'] * (3 if evoke else 5)}, {})
                p = me.prompt()
                card = self.named(p, 'Mulldrifter')[0]
                move = {'kind': 'cast', 'card': card, 'mana': {'U': 3 if evoke else 5},
                        'plan': [{'ability': f'{o}.a0'} for o in self.named(p, 'Island', zone='battlefield')]}
                if evoke:
                    move['cost'] = {'alternative': self.evoke_row(p, card)}
                me.answer(move)
                chooser = self.pause()
                if evoke:
                    self.assertIs(chooser, me)
                    self.ordered_entry(me, 'sacrifice')
                    self.pause()
                p = me.prompt()
                self.assertEqual(len(self.named(p, 'Island', zone='hand')), 2, 'the starting player skips the first draw step')
                self.assertEqual(len(self.named(p, 'Mulldrifter', zone='battlefield')), int(not evoke))
                self.assertEqual(len(self.named(p, 'Mulldrifter', zone='graveyard')), int(evoke))

    def test_blink_between_evoke_triggers_keeps_the_new_solitude(self):
        # Rules: 702.74a, 400.7, 603.4
        me, them = self.new({'hand': ['Solitude', 'Swords to Plowshares', 'Ephemerate'],
                             'battlefield': ['Plains']}, {'battlefield': ['Grizzly Bears']})
        p = me.prompt()
        card = self.named(p, 'Solitude')[0]
        me.answer({'kind': 'cast', 'card': card, 'cost': {'alternative': self.evoke_row(p, card)},
                   'paying': [[self.named(p, 'Swords to Plowshares')[0]]]})
        self.assertIs(self.pause(), me)
        self.ordered_entry(me, 'entry')
        bear = self.named(me.last, 'Grizzly Bears')[0]
        me.answer({'kind': 'targets', 'targets': [[bear]]})
        p = me.prompt()
        old = self.named(p, 'Solitude', zone='battlefield')[0]
        me.answer({'kind': 'cast', 'card': self.named(p, 'Ephemerate')[0], 'targets': [[old]], 'mana': {'W': 1},
                   'plan': [{'ability': f'{self.named(p, "Plains")[0]}.a0'}]})
        self.assertIs(self.pause(), me)
        self.assertEqual(me.last['decision']['kind'], 'targets', 'returned Solitude only has its normal entry trigger')
        me.answer({'kind': 'targets', 'targets': [[self.named(me.last, 'Grizzly Bears')[0]]]})
        self.pause()
        p = me.prompt()
        returned = self.named(p, 'Solitude', zone='battlefield')
        self.assertEqual(len(returned), 1)
        self.assertNotEqual(returned[0], old)
        self.assertFalse(self.named(p, 'Solitude', zone='graveyard'))
        self.assertEqual(len(self.named(p, 'Grizzly Bears', zone='exile')), 1)
        self.assertEqual(p['players'][1]['life'], 22)
