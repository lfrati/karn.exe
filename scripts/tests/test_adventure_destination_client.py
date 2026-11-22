"""715.3d/616.1: choosing Adventure's exile is what grants its play permission."""
import unittest
import play
from client_support import ClientTestCase


class AdventureDestinationClientTests(ClientTestCase):
    def compete(self, player, replacement):
        for effect in ['adventure', 'graveyard:0']:
            with self.subTest(effect=effect):
                seats = [{'hand': ["Grabby Giant // That's Mine"], 'battlefield': ['Mountain'] * 6}, {}]
                seats[player].setdefault('battlefield', []).append(replacement)
                self.new(seats)
                mountains = self.ids('Mountain', zone='battlefield')
                self.cast('Grabby Giant', cast_as='adventure', mana=mountains[:2], spend={'R': 2})
                self.pass_round()
                asked = self.engine.state['asked']
                self.assertEqual((asked['kind'], asked.get('event'), asked['player']),
                                 ('replacement', 'spell_destination', 0))
                self.assertEqual(len(asked['among']), 2)
                adventure, = [e for e in asked['among'] if e['effect'] == 'adventure']
                self.assertEqual(adventure['rule'], '715.3d')
                on = asked['on']['object']
                before = self.view(0)
                with self.assertRaises(play.Refused) as refusal:
                    self.engine.act({'kind': 'choose', 'answer': {'spell_destination': {'on': on, 'effect': 'buyback'}}})
                self.assertEqual(refusal.exception.refusal_class, 'illegal')
                self.assertEqual(self.view(0), before)
                self.engine.act({'kind': 'choose', 'answer': {'spell_destination': {'on': on, 'effect': effect}}})
                for seat in [0, 1]:
                    self.assertIn({'player': 0, 'spell_destination': {'on': on, 'effect': effect}},
                                  self.view(seat)['choices_made'])
                exiled, = self.engine.state['exile']
                if effect == 'adventure':
                    self.cast('Grabby Giant', card=exiled, mana=mountains[2:], spend={'R': 4})
                    self.assertEqual(len(self.engine.state['stack']), 1)
                else:
                    before = self.view(0)
                    with self.assertRaises(play.Refused) as refusal:
                        self.cast('Grabby Giant', card=exiled, mana=mountains[2:], spend={'R': 4})
                    self.assertEqual(refusal.exception.refusal_class, 'illegal')
                    self.assertEqual(self.view(0), before)

    def test_rest_in_peace_or_adventure_are_distinct_choices(self):
        self.compete(0, 'Rest in Peace')

    def test_the_adventure_controller_chooses_under_an_opponents_leyline(self):
        self.compete(1, 'Leyline of the Void')


if __name__ == '__main__':
    unittest.main()
