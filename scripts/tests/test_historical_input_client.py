"""608.2h/i: complete Oracle cards read their retained movement inputs through JSON."""
import unittest
import play
from client_support import ClientTestCase


class HistoricalInputClientTests(ClientTestCase):
    def test_ooze_creature_and_noncreature_inputs_and_refusal_atomicity(self):
        for name, creature in [('Grizzly Bears', True), ('Island', False)]:
            with self.subTest(name=name):
                self.new([{'battlefield': ['Scavenging Ooze', 'Forest'], 'hand': ['Mountain']},
                          {'graveyard': [name]}, {}])
                ooze, = self.ids('Scavenging Ooze')
                forest, = self.ids('Forest', zone='battlefield')
                target, = self.engine.state['players'][1]['graveyard']
                mountain, = self.ids('Mountain')
                before = self.view(0)
                with self.assertRaises(play.Refused) as refusal:
                    self.activate(ooze, [[{'object': mountain}]], ability=0, mana=[forest], spend={'G': 1})
                self.assertEqual(refusal.exception.refusal_class, 'illegal')
                self.assertEqual(self.view(0), before)
                self.activate(ooze, [[{'object': target}]], ability=0, mana=[forest], spend={'G': 1})
                self.pass_round()
                for viewer in [0, 1, 2]:
                    state = self.view(viewer)
                    self.assertEqual(state['players'][0]['life'], 20 + int(creature))
                    self.assertEqual(state['objects'][str(ooze)].get('counters', {}).get('+1/+1', 0), int(creature))
                    exiled, = state['exile']
                    self.assertEqual(state['objects'][str(exiled)]['name'], name)

    def test_cling_to_dust_selects_life_or_draw_using_the_original_card(self):
        for name, creature in [('Grizzly Bears', True), ('Island', False)]:
            with self.subTest(name=name):
                self.new([{'battlefield': ['Swamp'], 'hand': ['Cling to Dust']},
                          {'graveyard': [name]}, {}])
                target, = self.engine.state['players'][1]['graveyard']
                before_hand = len(self.hand(0))
                self.cast('Cling to Dust', [[{'object': target}]], mana=self.ids('Swamp', zone='battlefield'), spend={'B': 1})
                self.pass_round()
                self.assertEqual(len(self.hand(0)), before_hand - int(creature))
                for viewer in [0, 1, 2]:
                    state = self.view(viewer)
                    self.assertEqual(state['players'][0]['life'], 23 if creature else 20)
                    self.assertEqual(state['objects'][str(state['exile'][0])]['name'], name)


if __name__ == '__main__':
    unittest.main()
