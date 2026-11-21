"""Real-seat checks that an object choice says what the objects are chosen for (608.2d)."""
import os
import unittest

import play
import test_player_prompt_client as fixtures


class ObjectChoicePurposeClientTests(unittest.TestCase):
    new = fixtures.PlayerPromptClientTests.new
    waiting = fixtures.PlayerPromptClientTests.waiting
    named = staticmethod(fixtures.PlayerPromptClientTests.named)
    close_engine = fixtures.PlayerPromptClientTests.close_engine
    refused = fixtures.PlayerPromptClientTests.refused

    @classmethod
    def setUpClass(cls):
        if not os.environ.get('OBJECT_CHOICE_ENGINE'):
            fixtures.PlayerPromptClientTests.setUpClass()

    def setUp(self):
        self.engine = play.Engine(executable=os.environ.get('OBJECT_CHOICE_ENGINE'))
        self.addCleanup(self.close_engine)

    def objects_choice(self, targets=None):
        """Pass (answering a target question with `targets`) until an object choice is asked."""
        for _ in range(30):
            seat = self.waiting()
            d = seat.last['decision']
            if d['kind'] == 'objects':
                return seat, d
            if d['kind'] == 'targets':
                seat.answer({'kind': 'targets', 'targets': targets})
            else:
                self.assertEqual(d['kind'], 'priority', d)
                seat.answer({'kind': 'pass'})
        self.fail('no object choice')

    def cast(self, seat, name, swamps, *, spend, **fields):
        p = seat.prompt()
        lands = [o for o in self.named(p, 'Swamp', controller=seat.label, zone='battlefield') if not p['objects'][o]['tapped']]
        return seat.answer({'kind': 'cast', 'card': self.named(p, name, zone='hand')[0],
                            'plan': [{'ability': f'{o}.a0'} for o in lands[:swamps]], 'mana': spend, **fields})

    def test_duress_asks_for_a_card_to_discard_and_refuses_one_not_offered(self):
        # Rules: 608.2d, 701.9a
        me, them = self.new({'hand': ['Duress'], 'battlefield': ['Swamp']}, {'hand': ['Shock', 'Shock', 'Swamp']})
        self.cast(me, 'Duress', 1, targets=[['p1']], spend={'B': 1})
        seat, d = self.objects_choice()
        self.assertIs(seat, me)
        self.assertEqual((d['verb'], d['to'], d['count']), ('Discard', None, 1))
        land = self.named(seat.last, 'Swamp', controller='p1', zone='hand')[0]
        refusal = self.refused(me, {'cmd': 'answer', 'answer': {'kind': 'choose', 'answer': {'objects': [land]}}})
        self.assertIn('not among the offered objects to discard', refusal)
        me.answer({'kind': 'choose', 'answer': {'objects': d['among'][:1]}})
        self.assertEqual(len(self.named(me.prompt(), 'Shock', zone='graveyard')), 1)

    def test_chittering_rats_asks_its_target_for_a_card_to_put_on_top_of_their_library(self):
        # Rules: 608.2d, 401.4
        me, them = self.new({'hand': ['Chittering Rats'], 'battlefield': ['Swamp'] * 3}, {'hand': ['Shock', 'Swamp']})
        self.cast(me, 'Chittering Rats', 3, spend={'B': 3})
        seat, d = self.objects_choice(targets=[['p1']])
        self.assertIs(seat, them)
        self.assertEqual((d['verb'], d['to'], d['count']), ('Move', {'zone': 'library', 'position': 'top'}, 1))

    def test_rakdos_carnarium_asks_for_a_land_to_return_to_its_owners_hand(self):
        # Rules: 608.2d, 400.3
        me, them = self.new({'hand': ['Rakdos Carnarium'], 'battlefield': ['Swamp']}, {})
        p = me.prompt()
        me.answer({'kind': 'play', 'card': self.named(p, 'Rakdos Carnarium', zone='hand')[0]})
        seat, d = self.objects_choice()
        self.assertIs(seat, me)
        self.assertEqual((d['verb'], d['to'], d['count']), ('Move', {'zone': 'hand'}, 1))


if __name__ == '__main__':
    unittest.main()
