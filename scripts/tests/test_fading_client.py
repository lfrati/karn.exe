"""Fading's mandatory upkeep payment and ordinary counter costs through the client."""
import unittest
from test_round_keywords_client import RoundKeywordsClientTests as Helpers


class FadingClientTests(unittest.TestCase):
    setUpClass = classmethod(Helpers.setUpClass.__func__)
    setUp = Helpers.setUp
    close_engine = Helpers.close_engine
    new = Helpers.new
    view = Helpers.view
    hand = Helpers.hand
    ids = Helpers.ids
    object = Helpers.object
    pass_round = Helpers.pass_round
    cast = Helpers.cast
    step = Helpers.step
    advance = Helpers.advance

    def test_hydra_spends_its_last_counter_and_is_sacrificed_only_on_a_later_upkeep(self):
        # CR 702.32a, 118.12: removing the last fade counter has no sacrifice
        # trigger; the later mandatory upkeep removal fails and sacrifices it.
        self.new([{'hand': ['Ancient Hydra'], 'battlefield': ['Mountain'] * 10}, {}])
        mountains = self.ids('Mountain')
        self.cast('Ancient Hydra', mana=mountains[:5], spend={'R': 5})
        self.pass_round()
        source, = self.ids('Ancient Hydra', zone='battlefield')
        self.assertEqual(self.object(source)['counters'], {'fade': 5})
        for left, mountain in zip([4, 3, 2, 1, 0], mountains[5:]):
            self.engine.act({'kind': 'activate', 'source': source, 'ability': 1,
                             'targets': [[{'player': 1}]], 'plan': [{'source': mountain, 'ability': 0}],
                             'mana': {'R': 1}})
            self.pass_round()
            self.assertEqual(self.object(source)['counters'].get('fade', 0), left)
            self.assertIn(source, self.ids('Ancient Hydra', zone='battlefield'))
            self.assertEqual(self.engine.state['stack'], [])
        self.assertEqual(self.engine.state['players'][1]['life'], 15)
        turn = self.engine.state['turn']
        self.advance(lambda s: s['turn'] > turn and s['active'] == 0 and s['part'] == 'Upkeep')
        self.assertEqual(len(self.engine.state['stack']), 1)
        self.pass_round()
        self.assertEqual(self.ids('Ancient Hydra', zone='battlefield'), [])
        self.assertEqual(self.engine.state['asked']['kind'], 'priority')


del Helpers
