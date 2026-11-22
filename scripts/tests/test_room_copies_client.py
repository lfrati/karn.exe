"""Room copies use their copied halves and their own designations (707.2, 709.5)."""
import unittest
import play
from client_support import ClientTestCase


POOL = 'Bottomless Pool // Locker Room'
MEAT = 'Meat Locker // Drowned Diner'


class RoomCopiesClientTests(ClientTestCase):
    def copy(self, original, model):
        self.new([{'hand': ['Mirrorform'], 'battlefield': [original] + ['Island'] * 7},
                  {'battlefield': [model]}])
        islands = self.ids('Island', zone='battlefield')
        permanents = [(int(i), o) for i, o in self.engine.state['objects'].items()
                      if int(i) in self.engine.state['battlefield'] and 'Land' not in o['types']]
        mine, = [i for i, o in permanents if o['controller'] == 0]
        theirs, = [i for i, o in permanents if o['controller'] == 1]
        self.cast('Mirrorform', [[{'object': theirs}]], mana=islands[:6], spend={'U': 6})
        self.pass_round()
        return mine, islands[6]

    def test_a_room_copying_a_creature_refuses_unlock_without_changing_state(self):
        room, island = self.copy(POOL, 'Grizzly Bears')
        self.assertNotIn('unlocked', self.engine.state['objects'][str(room)])
        before = self.view(0)
        with self.assertRaises(play.Refused) as refusal:
            self.engine.act({'kind': 'unlock', 'room': room, 'half': 0,
                             'plan': [{'source': island, 'ability': 0}], 'mana': {'U': 1}})
        self.assertEqual(refusal.exception.refusal_class, 'illegal')
        self.assertEqual(self.view(0), before)

    def check_copied_room(self, original):
        room, island = self.copy(original, POOL)
        self.assertEqual(self.engine.state['objects'][str(room)]['unlocked'], [False, False])
        self.engine.act({'kind': 'unlock', 'room': room, 'half': 0,
                         'plan': [{'source': island, 'ability': 0}], 'mana': {'U': 1}})
        self.assertEqual(self.engine.state['asked']['kind'], 'targets')
        self.engine.act({'kind': 'targets', 'targets': [[]]})
        self.pass_round()
        for seat in [0, 1]:
            obj = self.view(seat)['objects'][str(room)]
            self.assertEqual((obj['name'], obj['colors'], obj['unlocked']),
                             ('Bottomless Pool', ['blue'], [True, False]))

    def test_a_room_copy_pays_the_copied_doors_cost(self):
        self.check_copied_room(MEAT)

    def test_a_creature_copying_a_room_can_unlock_it(self):
        self.check_copied_room('Grizzly Bears')


if __name__ == '__main__':
    unittest.main()
