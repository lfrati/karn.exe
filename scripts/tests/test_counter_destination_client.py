"""701.6/614.15: complete counterspells through supplied JSON moves and seat views."""
import unittest
import play
from client_support import ClientTestCase


class CounterDestinationClientTests(ClientTestCase):
    def respond(self, counter, target):
        self.engine.act({'kind': 'pass'})
        self.cast(counter, [[{'object': target}]], mana=self.ids('Island', zone='battlefield', controller=1), spend={'U': 2})
        self.pass_round()

    def test_remand_returns_the_card_and_draws_without_exposing_other_hands(self):
        self.new([{'hand': ['Shock'], 'battlefield': ['Mountain']},
                  {'hand': ['Remand'], 'battlefield': ['Island'] * 2}, {}])
        initial_hand = len(self.hand(0))
        self.cast('Shock', [[{'player': 2}]], mana=self.ids('Mountain', zone='battlefield'), spend={'R': 1})
        target, = self.engine.state['stack']
        self.engine.act({'kind': 'pass'})
        before = self.view(1)
        mountain, = self.ids('Mountain', zone='battlefield')
        with self.assertRaises(play.Refused) as refusal:
            self.cast('Remand', [[{'object': mountain}]], mana=self.ids('Island', zone='battlefield', controller=1), spend={'U': 2})
        self.assertEqual(refusal.exception.refusal_class, 'illegal')
        self.assertEqual(self.view(1), before)
        self.cast('Remand', [[{'object': target}]], mana=self.ids('Island', zone='battlefield', controller=1), spend={'U': 2})
        self.pass_round()
        own = self.view(0)
        self.assertEqual(sum(own['objects'][str(card)]['name'] == 'Shock' for card in own['players'][0]['hand']), 1)
        for viewer in [0, 1, 2]:
            state = self.view(viewer)
            self.assertEqual(state['stack'], [])
            self.assertEqual(state['players'][0]['hand_size'], initial_hand)
            self.assertEqual(state['players'][1]['hand_size'], 1)
            self.assertEqual(state['players'][2]['life'], 20)
            for player in set([0, 1, 2]) - {viewer}:
                self.assertNotIn('hand', state['players'][player])

    def test_memory_lapse_self_replacement_precedes_rest_in_peace(self):
        self.new([{'hand': ['Shock'], 'battlefield': ['Mountain', 'Rest in Peace']},
                  {'hand': ['Memory Lapse'], 'battlefield': ['Island'] * 2}, {}])
        self.cast('Shock', [[{'player': 2}]], mana=self.ids('Mountain', zone='battlefield'), spend={'R': 1})
        target, = self.engine.state['stack']
        self.respond('Memory Lapse', target)
        for viewer in [0, 1, 2]:
            state = self.view(viewer)
            self.assertEqual(state['asked']['kind'], 'priority')
            self.assertEqual(state['stack'], [])
            self.assertEqual(len(state['exile']), 1)
            self.assertEqual(state['objects'][str(state['exile'][0])]['name'], 'Memory Lapse')
        # The recorded next draw, rather than a privileged library query, proves top placement.
        before_hand = set(self.hand(0))
        self.advance(lambda state: state['players'][0]['hand_size'] > len(before_hand))
        card, = set(self.hand(0)) - before_hand
        self.assertEqual(self.view(0)['objects'][str(card)]['name'], 'Shock')

    def test_uncounterable_target_remains_on_stack_while_remand_draws(self):
        self.new([{'hand': ['Supreme Verdict'], 'battlefield': ['Plains'] * 2 + ['Island'] * 2},
                  {'hand': ['Remand'], 'battlefield': ['Island'] * 2}, {}])
        self.cast('Supreme Verdict', mana=self.ids('Plains', zone='battlefield', controller=0) + self.ids('Island', zone='battlefield', controller=0), spend={'W': 2, 'U': 2})
        target, = self.engine.state['stack']
        self.respond('Remand', target)
        for viewer in [0, 1, 2]:
            state = self.view(viewer)
            self.assertEqual(state['stack'], [target])
            self.assertEqual(state['players'][1]['hand_size'], 1)


if __name__ == '__main__':
    unittest.main()
