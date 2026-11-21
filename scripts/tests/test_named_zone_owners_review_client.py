"""Named-zone movement through private, labelled seat decisions."""
import os
import unittest
import play
import test_player_prompt_client as fixtures


class NamedZoneOwnersReviewClientTests(unittest.TestCase):
    new = fixtures.PlayerPromptClientTests.new
    waiting = fixtures.PlayerPromptClientTests.waiting
    named = staticmethod(fixtures.PlayerPromptClientTests.named)
    close_engine = fixtures.PlayerPromptClientTests.close_engine
    refused = fixtures.PlayerPromptClientTests.refused

    @classmethod
    def setUpClass(cls):
        if not os.environ.get('NAMED_ZONE_REVIEW_ENGINE'):
            fixtures.PlayerPromptClientTests.setUpClass()

    def setUp(self):
        self.engine = play.Engine(executable=os.environ.get('NAMED_ZONE_REVIEW_ENGINE'))
        self.addCleanup(self.close_engine)

    def cast(self, seat, name, land, count, *, spend, **fields):
        p = seat.prompt()
        mana = [o for o in self.named(p, land, controller=seat.label, zone='battlefield')
                if not p['objects'][o]['tapped']][:count]
        return seat.answer({'kind': 'cast', 'card': self.named(p, name, zone='hand')[0],
                            'plan': [{'ability': f'{o}.a0'} for o in mana], 'mana': spend, **fields})

    def pause(self):
        for _ in range(30):
            seat = self.waiting()
            if seat.last['decision']['kind'] != 'priority' or not seat.last['stack']:
                return seat
            seat.answer({'kind': 'pass'})
        self.fail('stack did not settle')

    def test_rats_opponent_chooses_privately_then_draws_the_card_as_a_new_object(self):
        # Rules: 400.2, 400.7, 608.2d
        me, them, observer = self.new({'hand': ['Chittering Rats'], 'battlefield': ['Swamp'] * 3},
                                      {'hand': ['Shock', 'Naturalize']}, {})
        self.cast(me, 'Chittering Rats', 'Swamp', 3, spend={'B': 3})
        chooser = self.pause()
        self.assertIs(chooser, me)
        self.assertEqual(chooser.last['decision']['kind'], 'targets')
        me.answer({'kind': 'targets', 'targets': [['p1']]})
        self.assertIs(self.pause(), them)
        d = them.last['decision']
        self.assertEqual((d['kind'], d['verb'], d['count'], d['chooser'], d.get('acting', d['chooser'])),
                         ('objects', 'Move', 1, 'p1', 'p1'))
        self.assertEqual(set(d['among']), set(self.named(them.last, 'Shock') + self.named(them.last, 'Naturalize')))
        old = self.named(them.last, 'Shock')[0]
        for other in (me, observer):
            p = other.prompt()
            self.assertIsNone(p['decision'])
            self.assertEqual(p['waiting_for'], 'p1')
            self.assertFalse(self.named(p, 'Shock') + self.named(p, 'Naturalize'))
        self.assertIn('request belongs', self.refused(me, {'cmd': 'answer', 'answer': {'kind': 'choose', 'answer': {'objects': []}}}))
        them.answer({'kind': 'choose', 'answer': {'objects': [old]}})
        self.pause()
        self.assertFalse(self.named(them.prompt(), 'Shock', zone='hand'))
        self.assertNotIn(old, them.last['objects'])
        for _ in range(60):
            seat = self.waiting()
            if seat.last['active'] == 'p1' and seat.last['part'] == 'PrecombatMain':
                break
            kind = seat.last['decision']['kind']
            if kind == 'attackers':
                seat.answer({'kind': 'attack', 'attacks': []})
            elif kind == 'blockers':
                seat.answer({'kind': 'block', 'blocks': []})
            else:
                self.assertEqual(kind, 'priority')
                seat.answer({'kind': 'pass'})
        else:
            self.fail('next player turn not reached')
        new = self.named(them.prompt(), 'Shock', zone='hand')
        self.assertEqual(len(new), 1)
        self.assertNotEqual(new[0], old)
        self.assertFalse(self.named(observer.prompt(), 'Shock'))

    def test_prying_questions_keeps_the_player_actor_after_the_life_loss(self):
        # Rules: 608.2c-d, 609.3, 400.3
        for hand in (['Shock', 'Naturalize'], []):
            with self.subTest(hand=hand):
                me, them = self.new({'hand': ['Prying Questions'], 'battlefield': ['Swamp'] * 3}, {'hand': hand})
                self.cast(me, 'Prying Questions', 'Swamp', 3, targets=[['p1']], spend={'B': 3})
                chooser = self.pause()
                self.assertEqual(them.prompt()['players'][1]['life'], 17)
                if hand:
                    self.assertIs(chooser, them)
                    self.assertEqual((them.last['decision']['chooser'], them.last['decision'].get('acting', them.last['decision']['chooser'])), ('p1', 'p1'))
                    chosen = self.named(them.last, 'Naturalize')[0]
                    them.answer({'kind': 'choose', 'answer': {'objects': [chosen]}})
                    self.pause()
                    self.assertEqual(len(self.named(them.prompt(), 'Shock', zone='hand')), 1)
                else:
                    self.assertFalse(me.prompt()['stack'])
                    self.assertEqual(me.last['decision']['kind'], 'priority')

    def test_eye_spy_look_is_private_and_a_moved_card_becomes_public(self):
        # Rules: 400.2, 701.20, 400.3
        for yes in (True, False):
            with self.subTest(put_in_graveyard=yes):
                me, them, observer = self.new({'hand': ['Eye Spy'], 'battlefield': ['Island']},
                                              {'library': ['Island'] * 4 + ['Shock']}, {})
                self.cast(me, 'Eye Spy', 'Island', 1, targets=[['p1']], spend={'U': 1})
                self.assertIs(self.pause(), me)
                self.assertEqual(me.last['decision']['kind'], 'may')
                self.assertEqual(len(self.named(me.last, 'Shock')), 1)
                self.assertFalse(self.named(them.prompt(), 'Shock'))
                self.assertFalse(self.named(observer.prompt(), 'Shock'))
                me.answer({'kind': 'choose', 'answer': 'yes' if yes else 'no'})
                self.pause()
                for seat in self.seats:
                    cards = self.named(seat.prompt(), 'Shock', zone='graveyard')
                    self.assertEqual(len(cards), int(yes))

    def test_painful_memories_controller_chooses_from_the_opponents_private_hand(self):
        # Rules: 400.2, 608.2d, 400.3
        me, them, observer = self.new({'hand': ['Painful Memories'], 'battlefield': ['Swamp'] * 2},
                                      {'hand': ['Shock', 'Naturalize']}, {})
        self.cast(me, 'Painful Memories', 'Swamp', 2, targets=[['p1']], spend={'B': 2})
        self.assertIs(self.pause(), me)
        d = me.last['decision']
        self.assertEqual((d['kind'], d['chooser'], d.get('acting', d['chooser'])), ('objects', 'p0', 'p0'))
        chosen = self.named(me.last, 'Shock')[0]
        self.assertIn(chosen, d['among'])
        self.assertFalse(self.named(observer.prompt(), 'Shock') + self.named(observer.last, 'Naturalize'))
        me.answer({'kind': 'choose', 'answer': {'objects': [chosen]}})
        self.pause()
        self.assertFalse(self.named(them.prompt(), 'Shock', zone='hand'))
        self.assertEqual(len(self.named(them.last, 'Naturalize', zone='hand')), 1)
        self.assertFalse(self.named(me.prompt(), 'Naturalize'))

    def test_agonizing_memories_controller_orders_the_chosen_cards(self):
        # Rules: 401.4, 101.1
        me, them, observer = self.new({'hand': ['Agonizing Memories'], 'battlefield': ['Swamp'] * 4},
                                      {'hand': ['Shock', 'Naturalize', 'Fog']}, {})
        self.cast(me, 'Agonizing Memories', 'Swamp', 4, targets=[['p1']], spend={'B': 4})
        self.assertIs(self.pause(), me)
        d = me.last['decision']
        self.assertEqual((d['kind'], d['count']), ('objects', 2))
        chosen = self.named(me.last, 'Shock') + self.named(me.last, 'Naturalize')
        me.answer({'kind': 'choose', 'answer': {'objects': chosen}})
        chooser = self.pause()
        self.assertIs(chooser, me, 'the spell instructs its controller to put them in any order')
        self.assertEqual((me.last['decision']['kind'], len(me.last['decision']['cards'])), ('order_cards', 2))
        me.answer({'kind': 'choose', 'answer': {'objects': chosen[::-1]}})
        self.pause()
        self.assertEqual(len(self.named(them.prompt(), 'Fog', zone='hand')), 1)
        self.assertFalse(self.named(them.last, 'Shock', zone='hand') + self.named(them.last, 'Naturalize', zone='hand'))
        self.assertFalse(self.named(observer.prompt(), 'Shock') + self.named(observer.last, 'Naturalize'))
