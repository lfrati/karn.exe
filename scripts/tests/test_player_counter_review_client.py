"""Poison thresholds and simultaneous losses, through the
real audited JSON client."""
import unittest
import test_multiplayer_outcome_client as helpers

H = helpers.MultiplayerOutcomeClientTests


class PlayerCounterReviewClientTests(unittest.TestCase):
    setUpClass = classmethod(H.setUpClass.__func__)
    setUp = H.setUp
    close_engine = H.close_engine
    new = H.new
    view = H.view
    hand = H.hand
    ids = H.ids
    cast = H.cast
    pass_round = H.pass_round

    def counters(self, viewer):
        return [p["counters"] for p in self.view(viewer)["players"]]

    def test_nine_poison_survives_and_two_opponents_reaching_ten_lose_together(self):
        # 122.1f / 704.5c: ten or more; 704.3: both losses in one state-based check
        lib = {"library": ["Forest"] * 20}
        self.new([{"hand": ["Caress of Phyrexia"] * 6 + ["Prologue to Phyresis"],
                   "battlefield": ["Swamp"] * 30 + ["Island"] * 2}, dict(lib), dict(lib)])
        swamps = self.ids("Swamp", controller=0)
        for index, p in enumerate((1, 1, 1, 2, 2, 2)):
            self.cast("Caress of Phyrexia", [[{"player": p}]], mana=swamps[index * 5:(index + 1) * 5], spend={'B': 5})
            self.pass_round()
        players = self.engine.state["players"]
        self.assertEqual([(q["life"], q["lost"]) for q in players], [(20, False), (11, False), (11, False)])
        for viewer in (0, 1, 2):
            self.assertEqual(self.counters(viewer), [{}, {"poison": 9}, {"poison": 9}])
        self.cast("Prologue to Phyresis", mana=self.ids("Island", controller=0), spend={'U': 2})
        later = []
        for _ in self.engine.state["players"]:
            self.engine.act({"kind": "pass"})
            later += self.engine.state["events"]
        players = self.engine.state["players"]
        self.assertEqual([q["lost"] for q in players], [False, True, True])
        print(sorted({e["event"] for e in later}))
        puts = [(e["on"], e["count"]) for e in later if e["event"] == "counters"]
        self.assertEqual(puts, [({"player": 1}, 1), ({"player": 2}, 1)])
        for viewer in (0, 1, 2):
            self.assertEqual(self.counters(viewer)[1:], [{"poison": 10}, {"poison": 10}])


del H
