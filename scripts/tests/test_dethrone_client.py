"""Dethrone's event-time life-rank condition through the client."""
import unittest
from test_round_keywords_client import RoundKeywordsClientTests as Helpers


class DethroneClientTests(unittest.TestCase):
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

    def test_tied_defender_triggers_even_when_a_response_lowers_their_life(self):
        # CR 702.105a, 603.2: tied for most life when attacked suffices; the
        # ability has no intervening-if recheck when the defender later loses life.
        self.new([{'battlefield': ["Grenzo's Cutthroat", 'Mountain'], 'hand': ['Lightning Bolt']}, {}, {}])
        source, = self.ids("Grenzo's Cutthroat", zone='battlefield')
        self.advance(lambda s: s['asked']['kind'] == 'attackers')
        self.engine.act({'kind': 'attack', 'attacks': [[source, {'player': 1}]]})
        self.assertEqual(len(self.engine.state['stack']), 1)
        self.cast('Lightning Bolt', [[{'player': 1}]], mana=self.ids('Mountain'), spend={'R': 1})
        self.pass_round()
        self.assertEqual(self.engine.state['players'][1]['life'], 17)
        self.pass_round()
        self.assertEqual(self.object(source)['counters'], {'+1/+1': 1})


del Helpers
