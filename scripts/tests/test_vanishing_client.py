"""Vanishing entry, upkeep and last-counter suspension through the player client."""
from client_support import ClientTestCase


class VanishingClientTests(ClientTestCase):
    def test_riftwatcher_sacrifice_is_a_separate_stack_occurrence(self):
        # CR 702.63a, 603.3, 701.21a; full Oracle gains2 on both entry and leave.
        self.new([{'hand': ['Aven Riftwatcher'], 'battlefield': ['Plains'] * 3}, {}])
        self.cast('Aven Riftwatcher', mana=self.ids('Plains'), spend={'W': 3})
        self.pass_round()
        bird, = self.ids('Aven Riftwatcher', zone='battlefield')
        self.assertEqual(self.object(bird)['counters'], {'time': 3})
        self.pass_round()
        self.assertEqual(self.engine.state['players'][0]['life'], 22)
        for count in [2, 1, 0]:
            turn = self.engine.state['turn']
            self.advance(lambda s: s['turn'] > turn and s['active'] == 0 and s['part'] == 'Upkeep')
            self.assertEqual(len(self.engine.state['stack']), 1)
            self.pass_round()
            self.assertEqual(self.object(bird)['counters'].get('time', 0), count)
            self.assertIn(bird, self.ids('Aven Riftwatcher', zone='battlefield'))
        self.assertEqual(len(self.engine.state['stack']), 1)
        self.pass_round()
        self.assertEqual(self.ids('Aven Riftwatcher', zone='battlefield'), [])
        self.pass_round()
        self.assertEqual(self.engine.state['players'][0]['life'], 24)
