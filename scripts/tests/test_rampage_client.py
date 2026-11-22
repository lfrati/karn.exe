"""Rampage's resolution-time blocker count through the managed combat boundary."""
import unittest
from test_round_keywords_client import RoundKeywordsClientTests as Helpers


class RampageClientTests(unittest.TestCase):
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

    def test_removed_blocker_changes_the_pending_bonus_but_not_the_resolved_bonus(self):
        # CR 702.23b:3 declared blockers→2 at resolution gives+2/+2;
        # removing another after resolution does not shrink the captured bonus.
        self.new([{'battlefield': ['Craw Giant'] + ['Island'] * 2, 'hand': ['Unsummon'] * 2},
                  {'battlefield': ['Wall of Stone'] * 3}])
        source, = self.ids('Craw Giant', zone='battlefield')
        islands = self.ids('Island', zone='battlefield')
        self.advance(lambda state: state['asked']['kind'] == 'attackers')
        self.engine.act({'kind': 'attack', 'attacks': [[source, {'player': 1}]]})
        self.advance(lambda state: state['asked']['kind'] == 'blockers')
        blockers = self.ids('Wall of Stone', zone='battlefield')
        self.engine.act({'kind': 'block', 'blocks': [[b, source] for b in blockers]})
        self.assertEqual(len(self.engine.state['stack']), 1)
        self.cast('Unsummon', [[{'object': blockers[0]}]], mana=islands[:1], spend={'U': 1})
        self.pass_round()
        self.pass_round()
        self.assertEqual((self.object(source)['power'], self.object(source)['toughness']), (8, 6))
        self.cast('Unsummon', [[{'object': blockers[1]}]], mana=islands[1:], spend={'U': 1})
        self.pass_round()
        self.assertEqual((self.object(source)['power'], self.object(source)['toughness']), (8, 6))


del Helpers
