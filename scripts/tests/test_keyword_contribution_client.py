"""Separate keyword contributions through the real Python/JSON client."""
import unittest
from test_grant_client import GrantClientTests as Helpers


class KeywordContributionClientTests(unittest.TestCase):
    setUpClass = classmethod(Helpers.setUpClass.__func__)
    setUp = Helpers.setUp
    close_engine = Helpers.close_engine
    new = Helpers.new
    view = Helpers.view
    hand = Helpers.hand
    ids = Helpers.ids
    object = Helpers.object
    pass_round = Helpers.pass_round
    choose = Helpers.choose
    reject = Helpers.reject
    advance = Helpers.advance
    step = Helpers.step

    def named(self, zone, name):
        return [i for i in zone if self.object(i)["name"] == name]

    def test_ichor_slick_cycling_discards_into_madness_exile_and_resolves_first(self):
        # 702.29a cycling discards from a hand; 702.35a madness replaces that discard; 603.3
        self.new([{"hand": ["Ichor Slick"], "battlefield": ["Swamp"] * 6}, {"battlefield": ["Grizzly Bears"]}])
        slick, = self.hand(0)
        bears, = self.ids("Grizzly Bears")
        swamps = self.ids("Swamp", zone="battlefield", controller=0)
        self.engine.act({"kind": "activate", "source": slick, "ability": 3, "mana": {"B": 2}, "targets": [],
                         "plan": [{"source": o, "ability": 0} for o in swamps[:2]], "paying": []})
        exiled, = self.named(self.engine.state["exile"], "Ichor Slick")
        self.assertFalse(self.engine.state["players"][0]["graveyard"])
        self.assertEqual(len(self.engine.state["stack"]), 2)
        for viewer in (0, 1):
            self.assertIn(str(exiled), self.view(viewer)["objects"], "madness exile is public")
        self.pass_round()
        asked = self.engine.state["asked"]
        self.assertEqual((asked["kind"], asked["player"], asked["cards"]), ("cast", 0, [exiled]))
        command = {"kind": "cast", "card": exiled, "cost": {"alternative": 2}, "mana": {"B": 4},
                   "targets": [[{"object": bears}]],
                   "plan": [{"source": o, "ability": 0} for o in swamps[2:6]]}
        self.reject(1, command)
        self.reject(0, command | {"targets": [[{"player": 1}]]})
        hand = len(self.hand(0))
        self.engine.act(command)
        self.pass_round()
        self.assertNotIn(bears, self.engine.state["battlefield"])
        self.assertEqual(len(self.hand(0)), hand, "the cycling draw waits under the madness spell")
        self.pass_round()
        self.assertEqual(len(self.hand(0)), hand + 1)
        self.assertEqual(len(self.named(self.engine.state["players"][0]["graveyard"], "Ichor Slick")), 1)

    def test_marshaling_cry_granted_rebound_then_flashback_each_only_for_its_own_cast(self):
        # 702.88a rebound needs a hand cast; 702.34a flashback exiles only a flashback cast
        self.new([{"hand": ["Marshaling Cry"], "battlefield": ["Cast Through Time", "Grizzly Bears"] + ["Plains"] * 7}, {}])
        cry, = self.hand(0)
        self.engine.act({"kind": "cast", "card": cry, "targets": [], "mana": {"W": 3},
                         "plan": [{"source": o, "ability": 0} for o in self.ids("Plains", controller=0)[:3]]})
        self.pass_round()
        exiled, = self.named(self.engine.state["exile"], "Marshaling Cry")
        turn = self.engine.state["turn"]
        self.advance(lambda s: s["turn"] > turn and s["active"] == 0 and s["part"] == "Upkeep")
        self.assertEqual(len(self.engine.state["stack"]), 1)
        self.pass_round()
        self.engine.act({"kind": "cast", "card": exiled, "targets": [], "plan": []})
        self.pass_round()
        self.assertFalse(self.named(self.engine.state["exile"], "Marshaling Cry"), "cast from exile: graveyard")
        grave, = self.named(self.engine.state["players"][0]["graveyard"], "Marshaling Cry")
        self.advance(lambda s: s["part"] == "PrecombatMain" and not s["stack"])
        self.engine.act({"kind": "cast", "card": grave, "route": "flashback", "targets": [], "mana": {"W": 4},
                         "plan": [{"source": o, "ability": 0} for o in self.ids("Plains", controller=0)[:4]]})
        self.pass_round()
        self.assertEqual(len(self.named(self.engine.state["exile"], "Marshaling Cry")), 1, "flashback exiles it")
        self.assertFalse(self.named(self.engine.state["players"][0]["graveyard"], "Marshaling Cry"))
        bears, = self.ids("Grizzly Bears")
        self.assertEqual(self.object(bears)["power"], 4, "rebound and flashback casts both resolved this turn")
        turn = self.engine.state["turn"]
        self.advance(lambda s: s["turn"] > turn and s["active"] == 0 and s["part"] == "Upkeep")
        self.assertFalse(self.engine.state["stack"], "a graveyard cast does not rebound")


del Helpers
