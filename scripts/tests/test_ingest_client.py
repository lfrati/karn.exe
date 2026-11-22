"""Ingest's damaged-player binding through actual multiplayer combat."""
import unittest
from test_round_keywords_client import RoundKeywordsClientTests as Helpers


class IngestClientTests(unittest.TestCase):
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

    def test_four_damage_exiles_one_top_card_from_only_the_damaged_players_library(self):
        # CR 702.115a: each trigger exiles one card, not one per damage point;
        # the damaged player, not the source's controller or every opponent, does it.
        self.new([{'battlefield': ['Mist Intruder', 'Forest'], 'hand': ['Giant Growth']},
                  {'library': ['Island', 'Mountain']}, {'library': ['Plains', 'Swamp']}])
        source, = self.ids('Mist Intruder', zone='battlefield')
        self.cast('Giant Growth', [[{'object': source}]], mana=self.ids('Forest', zone='battlefield'), spend={'G': 1})
        self.pass_round()
        self.advance(lambda state: state['asked']['kind'] == 'attackers')
        self.engine.act({'kind': 'attack', 'attacks': [[source, {'player': 1}]]})
        self.advance(lambda state: state['players'][1]['life'] == 16)
        self.assertEqual(len(self.engine.state['stack']), 1)
        self.pass_round()
        self.assertEqual([self.object(o)['name'] for o in self.engine.state['exile']], ['Mountain'])
        self.assertEqual(self.engine.state['players'][1]['library'], 1)
        self.assertEqual(self.engine.state['players'][2]['library'], 2)


del Helpers
