"""Training's event-time comparison through actual multiplayer attacks and responses."""
import unittest
from test_round_keywords_client import RoundKeywordsClientTests as Helpers


class TrainingClientTests(unittest.TestCase):
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

    def test_larger_attacker_can_attack_another_player_and_die_before_training_resolves(self):
        # CR 702.149a, 603.2: attack together, then a response kills the larger
        # attacker; the already-triggered ability still adds its counter.
        self.new([{'battlefield': ['Gryff Rider', 'Hill Giant', 'Mountain'],
                   'hand': ['Lightning Bolt']}, {}, {}])
        source, = self.ids('Gryff Rider', zone='battlefield')
        larger, = self.ids('Hill Giant', zone='battlefield')
        self.advance(lambda s: s['asked']['kind'] == 'attackers')
        self.engine.act({'kind': 'attack', 'attacks': [[source, {'player': 1}], [larger, {'player': 2}]]})
        self.assertEqual(len(self.engine.state['stack']), 1)
        self.cast('Lightning Bolt', [[{'object': larger}]], mana=self.ids('Mountain'), spend={'R': 1})
        self.pass_round()
        self.assertEqual(self.ids('Hill Giant', zone='battlefield'), [])
        self.pass_round()
        self.assertEqual(self.object(source)['counters'], {'+1/+1': 1})


del Helpers
