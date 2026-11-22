"""601.3d: conditional self-flash is checked when casting begins."""
import unittest
import play
from client_support import ClientTestCase


WURM = 'Colossal Rattlewurm'
DESERT = 'Sunscorched Desert'


class ConditionalFlashClientTests(ClientTestCase):
    def upkeep(self, active, desert_controller):
        seats = [{'hand': [WURM], 'battlefield': ['Forest'] * 4}, {}]
        if desert_controller is not None:
            seats[desert_controller].setdefault('battlefield', []).append(DESERT)
        self.new(seats)
        self.advance(lambda s: s['active'] == active and s['part'] == 'Upkeep'
                     and s['asked']['kind'] == 'priority' and s['asked']['player'] == 0)

    def test_own_desert_allows_cast_in_either_players_upkeep(self):
        for active in [0, 1]:
            with self.subTest(active=active):
                self.upkeep(active, 0)
                self.cast(WURM, mana=self.ids('Forest', zone='battlefield'), spend={'G': 4})
                self.assertEqual(len(self.engine.state['stack']), 1)
                spell = self.engine.state['stack'][0]
                self.assertEqual(self.engine.state['objects'][str(spell)]['name'], WURM)

    def test_absent_or_opponents_desert_refuses_without_changing_state(self):
        for desert in [None, 1]:
            with self.subTest(desert=desert):
                self.upkeep(0, desert)
                before = self.view(0)
                with self.assertRaises(play.Refused) as refusal:
                    self.cast(WURM, mana=self.ids('Forest', zone='battlefield'), spend={'G': 4})
                self.assertEqual(refusal.exception.refusal_class, 'illegal')
                self.assertEqual(self.view(0), before)


if __name__ == '__main__':
    unittest.main()
