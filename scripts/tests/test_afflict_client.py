"""Afflict life loss through explicit multiplayer combat choices."""
import unittest
from test_round_keywords_client import RoundKeywordsClientTests as Helpers


class AfflictClientTests(unittest.TestCase):
    setUpClass = classmethod(Helpers.setUpClass.__func__)
    setUp = Helpers.setUp
    close_engine = Helpers.close_engine
    new = Helpers.new
    view = Helpers.view
    hand = Helpers.hand
    ids = Helpers.ids
    object = Helpers.object
    pass_round = Helpers.pass_round
    step = Helpers.step
    advance = Helpers.advance

    def test_two_blockers_cause_one_afflict_occurrence(self):
        # CR 702.130a,509.3c: becoming blocked occurs once for this attacker.
        self.new([{'battlefield':['Khenra Eternal']},{'battlefield':['Grizzly Bears']*2}])
        source, = self.ids('Khenra Eternal',zone='battlefield')
        self.advance(lambda s:s['asked']['kind']=='attackers')
        self.engine.act({'kind':'attack','attacks':[[source,{'player':1}]]})
        self.advance(lambda s:s['asked']['kind']=='blockers')
        blockers=self.ids('Grizzly Bears',zone='battlefield')
        self.engine.act({'kind':'block','blocks':[[b,source] for b in blockers]})
        self.assertEqual(len(self.engine.state['stack']),1)
        self.pass_round()
        self.assertEqual(self.engine.state['players'][1]['life'],19)
        self.assertEqual(self.engine.state['players'][0]['life'],20)


del Helpers
