"""Papalymo regressions through the real Python/JSON client."""
from client_support import ClientTestCase


class ExtremaClientTests(ClientTestCase):
    def fixture(self):
        self.new([{"battlefield": ["Papalymo Totolymo", "Sol Ring", "Sol Ring", "Forest", "Forest"],
                   "hand": ["Ornithopter", "Sol Ring", "Giant Growth"]},
                  {"battlefield": ["Grizzly Bears", "Grizzly Bears", "Llanowar Elves"]},
                  {"battlefield": ["Hill Giant", "Hill Giant"]}, {}])
        source = self.ids("Papalymo Totolymo")[0]
        self.cast("Ornithopter")
        self.assertEqual(len(self.engine.state["stack"]), 1)
        self.pass_round()
        self.cast("Sol Ring", mana=self.ids("Forest", zone="battlefield", controller=0)[:1], spend={'G': 1})
        self.assertEqual(len(self.engine.state["stack"]), 2)
        self.pass_round()
        self.assertEqual([p["life"] for p in self.engine.state["players"]], [21, 19, 19, 19])
        self.assertEqual(len(self.engine.state["stack"]), 1)
        self.pass_round()
        return source

    def activate(self, source):
        self.engine.act({"kind": "activate", "source": source, "ability": 1,
                         "targets": [], "mana": {"C": 4}, "plan": [{"source": o, "ability": 0}
                                                   for o in self.ids("Sol Ring", zone="battlefield", controller=0)[:2]]})
        self.assertNotIn(source, self.engine.state["battlefield"])

    def test_ties_owners_rejected_answers_and_simultaneous_public_sacrifice(self):
        source = self.fixture()
        bears, giants = self.ids("Grizzly Bears"), self.ids("Hill Giant")
        elf = self.ids("Llanowar Elves")[0]
        self.activate(source)
        self.pass_round()
        asked = self.engine.state["asked"]
        self.assertEqual((asked["kind"], asked["player"], asked["count"]), ("objects", 1, 1))
        self.assertEqual(set(asked["among"]), set(bears))
        self.reject(0, {"kind": "choose", "answer": {"objects": [bears[0]]}})
        self.reject(1, {"kind": "choose", "answer": {"objects": [elf]}})
        self.choose({"objects": [bears[0]]})
        self.assertIn(bears[0], self.engine.state["battlefield"])
        self.assertEqual(self.engine.state["asked"]["player"], 2)
        self.assertEqual(set(self.engine.state["asked"]["among"]), set(giants))
        self.choose({"objects": [giants[0]]})
        for viewer in range(4):
            state = self.view(viewer)
            self.assertNotIn(bears[0], state["battlefield"])
            self.assertNotIn(giants[0], state["battlefield"])
            self.assertEqual(len(state["players"][1]["graveyard"]), 1)
            self.assertEqual(len(state["players"][2]["graveyard"]), 1)
        self.assertIn(elf, self.engine.state["battlefield"])
        self.assertEqual(self.engine.state["asked"]["kind"], "priority")

    def test_effective_power_changes_before_resolution(self):
        source = self.fixture()
        bears = self.ids("Grizzly Bears")
        self.activate(source)
        self.advance(lambda s: s["asked"]["player"] == 0)
        self.cast("Giant Growth", [[{"object": bears[0]}]], mana=self.ids("Forest", zone="battlefield", controller=0)[1:2], spend={'G': 1})
        self.pass_round()
        self.assertEqual(self.object(bears[0])["power"], 5)
        self.pass_round()
        self.assertEqual(self.engine.state["asked"]["player"], 2)
        self.assertIn(bears[0], self.engine.state["battlefield"], "forced selection waits for simultaneous commit")
        self.choose({"objects": [self.ids("Hill Giant")[0]]})
        self.assertIn(bears[1], self.engine.state["battlefield"])

    def test_new_turn_clears_life_loss_qualification(self):
        source = self.fixture()
        self.advance(lambda s: s["active"] == 0 and s["turn"] > 1 and s["part"] == "PrecombatMain")
        self.activate(source)
        self.pass_round()
        self.assertEqual(self.engine.state["asked"]["kind"], "priority")
        self.assertEqual(len(self.engine.state["players"][1]["graveyard"]), 0)
        self.assertEqual(len(self.engine.state["players"][2]["graveyard"]), 0)
