"""Fixed energy costs, through the
real audited client with full pinned cards. To pay {E}, a player removes one energy counter
from themselves (107.14); the payer is the player activating the ability, not the card's owner
(602.2, 107.14). A cost can't be paid without the full resources and partial payments aren't
allowed (118.3, 601.2h); one counter pays only once (118.10). Energy isn't mana and doesn't
empty (Aether Hub ruling 414b6b24); you can't pay more than you have (7ea9ff64). A mana ability
resolves immediately (605.3b)."""
import unittest
import test_toxic_review_client as base
import test_multiplayer_outcome_client as explicit

HELPERS = {k: v for k, v in vars(base.ToxicReviewClientTests).items() if not k.startswith("test_") and not k.startswith("__")}


class EnergyPaymentReviewClientTests(unittest.TestCase):
    locals().update(HELPERS)
    validate_action = explicit.MultiplayerOutcomeClientTests.validate_action

    def energy(self, player, viewer=None):
        state = self.engine.state if viewer is None else self.view(viewer)
        return state["players"][player].get("counters", {}).get("energy", 0)

    def pool(self, player):
        return {k: v for k, v in self.engine.state["players"][player]["pool"].items() if k != "restricted" and v}

    def play_land(self, name):
        card = next(o for o in self.engine.state["players"][self.asked()["player"]]["hand"] if self.obj(o)["name"] == name)
        self.engine.act({"kind": "play", "card": card})
        self.settle()
        return card

    def mana(self, source, ability, adding=None):
        self.engine.act({"kind": "activate", "source": source, "ability": ability, "targets": [], "plan": [], "paying": [],
                         **({"adding": adding} if adding else {})})

    def removed(self, player):
        return [e["count"] for e in self.seen if e.get("event") == "counters_removed"
                and e.get("from") == {"player": player} and e.get("counter") == "energy"]

    def live_fast(self):
        """The fixture chooses three untapped Swamps, preserving all Hubs."""
        player = self.asked()["player"]
        swamps = [o for o in self.mine("Swamp", player) if not self.obj(o)["tapped"]][:3]
        self.cast("Live Fast", mana=swamps, spend={"B": 3})
        self.settle()

    def test_hub_entry_energy_colored_mana_and_exhaustion(self):
        self.new([{"hand": ["Aether Hub"]}, {}])
        self.main()
        self.assertEqual(self.energy(0), 0)
        self.play_land("Aether Hub")
        hub = self.one("Aether Hub")
        self.assertEqual([self.energy(0, viewer=v) for v in (0, 1)], [1, 1], "entry trigger; energy is public")
        self.validate_action({"kind": "activate", "source": hub, "ability": 1, "plan": []})
        for color in ("W", "U", "B", "R", "G"):
            self.validate_action({"kind": "activate", "source": hub, "ability": 2, "adding": [[color]], "plan": []})
        self.validate_action({"kind": "activate", "source": hub, "ability": 2, "adding": [["C"]], "plan": []}, False)
        self.mana(hub, 2, [["G"]])
        self.assertEqual((self.energy(0), self.pool(0), self.obj(hub)["tapped"]), (0, {"G": 1}, True))
        self.assertEqual(self.removed(0), [1], "exactly one removal of one counter")
        self.assertEqual(self.engine.state["stack"], [], "605.3b")
        # next own turn: untapped, but no energy left for the colored ability
        self.advance(lambda s: s["turn"] == 3 and s["part"] == "PrecombatMain" and s["asked"]["kind"] == "priority")
        self.assertFalse(self.obj(hub)["tapped"])
        self.reject(0, {"kind": "activate", "source": hub, "ability": 2, "adding": [["G"]], "plan": []})
        self.mana(hub, 1)
        self.assertEqual((self.pool(0), self.energy(0)), ({"C": 1}, 0))

    def test_refused_colored_activation_without_energy_leaves_everything(self):
        self.new([{"battlefield": ["Aether Hub"]}, {}])
        self.main()
        hub = self.one("Aether Hub")
        self.assertEqual(self.energy(0), 0)
        self.validate_action({"kind": "activate", "source": hub, "ability": 1, "plan": []})
        self.reject(0, {"kind": "activate", "source": hub, "ability": 2, "targets": [], "plan": [], "paying": [], "adding": [["G"]]})
        self.assertEqual((self.obj(hub)["tapped"], self.pool(0), self.energy(0)), (False, {}, 0))

    def test_two_hubs_share_one_energy_pool(self):
        self.new([{"hand": ["Aether Hub", "Kalonian Tusker"], "battlefield": ["Aether Hub"]}, {}])
        self.main()
        self.play_land("Aether Hub")
        hubs = self.mine("Aether Hub", 0)
        self.assertEqual(self.energy(0), 1)
        for hub in hubs:
            self.validate_action({"kind": "activate", "source": hub, "ability": 1, "plan": []})
            self.validate_action({"kind": "activate", "source": hub, "ability": 2, "adding": [["G"]], "plan": []})
        tusker = self.ids("Kalonian Tusker")[0]
        explicit = [{"source": h, "ability": 2, "paying": [], "adding": [["G"]]} for h in hubs]
        self.reject(0, {"kind": "cast", "card": tusker, "mana": {"G": 2}, "targets": [], "plan": explicit, "paying": []})
        self.assertEqual((self.energy(0), [self.obj(h)["tapped"] for h in hubs], self.pool(0)), (1, [False, False], {}),
                         "no partial payment of the first Hub")

    def test_two_hubs_pay_green_green_with_two_energy(self):
        self.new([{"hand": ["Live Fast", "Kalonian Tusker"], "battlefield": ["Aether Hub", "Aether Hub"] + ["Swamp"] * 3}, {}])
        self.main()
        hubs = self.mine("Aether Hub", 0)
        tusker = self.ids("Kalonian Tusker")[0]
        self.reject(0, {"kind": "cast", "card": tusker, "mana": {"G": 2}, "plan": [{"source": h, "ability": 2, "adding": [["G"]]} for h in hubs]})
        self.live_fast()
        self.assertEqual(self.energy(0), 2)
        self.cast("Kalonian Tusker", spend={"G": 2}, plan=[{"source": h, "ability": 2, "adding": [["G"]]} for h in hubs])
        self.settle()
        self.assertEqual(self.energy(0), 0)
        self.assertEqual(self.removed(0), [1, 1])
        self.assertIn(self.one("Kalonian Tusker"), self.engine.state["battlefield"])

    def test_hub_pays_inside_casting_and_keeps_the_rest(self):
        self.new([{"hand": ["Live Fast", "Llanowar Elves"], "battlefield": ["Aether Hub"] + ["Swamp"] * 3}, {}])
        self.main()
        hub = self.one("Aether Hub")
        elves = next(o for o in self.engine.state["players"][self.asked()["player"]]["hand"] if self.obj(o)["name"] == "Llanowar Elves")
        self.reject(self.asked()["player"], {"kind": "cast", "card": elves, "mana": {"G": 1}, "plan": [{"source": hub, "ability": 2, "adding": [["G"]]}]})
        self.live_fast()
        self.cast("Llanowar Elves", spend={"G": 1}, plan=[{"source": hub, "ability": 2, "adding": [["G"]]}])
        self.settle()
        self.assertEqual(self.energy(0), 1, "one of two spent")
        self.assertIn(self.one("Llanowar Elves"), self.engine.state["battlefield"])

    def test_stolen_servant_spends_its_controllers_energy_not_its_owners(self):
        self.new([{"hand": ["Live Fast"], "battlefield": ["Servant of the Conduit"] + ["Swamp"] * 3},
                  {"hand": ["Act of Treason", "Live Fast", "Llanowar Elves"], "battlefield": ["Mountain"] * 3 + ["Swamp"] * 3}])
        self.main()
        servant = self.one("Servant of the Conduit")
        self.live_fast()
        self.assertEqual((self.energy(0), self.energy(1)), (2, 0))
        self.advance(lambda s: s["turn"] == 2 and s["part"] == "PrecombatMain" and s["asked"]["kind"] == "priority" and s["active"] == 1)
        self.cast("Act of Treason", [[{"object": servant}]], mana=self.ids("Mountain", zone="battlefield", controller=1)[:3], spend={'R': 3})
        self.settle()
        self.assertEqual(self.obj(servant)["controller"], 1)
        self.validate_action({"kind": "activate", "source": servant, "ability": 1, "adding": [["G"]]}, False)
        elves = next(o for o in self.engine.state["players"][self.asked()["player"]]["hand"] if self.obj(o)["name"] == "Llanowar Elves")
        self.reject(self.asked()["player"], {"kind": "cast", "card": elves, "mana": {"G": 1}, "plan": [{"source": servant, "ability": 1, "adding": [["G"]]}]})
        self.reject(1, {"kind": "activate", "source": servant, "ability": 1, "targets": [], "plan": [], "paying": [], "adding": [["G"]]})
        self.live_fast()
        self.assertEqual((self.energy(0), self.energy(1)), (2, 2))
        self.cast("Llanowar Elves", spend={"G": 1}, plan=[{"source": servant, "ability": 1, "adding": [["G"]]}])
        self.settle()
        self.assertEqual((self.energy(0), self.energy(1)), (2, 1), "the controller paid")
        self.assertEqual(self.removed(1), [1])
        self.assertEqual(self.removed(0), [])

    def test_longtusk_cub_repeats_until_energy_runs_out(self):
        self.new([{"hand": ["Live Fast", "Live Fast"], "battlefield": ["Longtusk Cub"] + ["Swamp"] * 6}, {}])
        self.main()
        cub = self.one("Longtusk Cub")
        self.reject(0, {"kind": "activate", "source": cub, "ability": 1, "targets": [], "plan": []})
        self.live_fast()
        self.live_fast()
        self.assertEqual(self.energy(0), 4)
        index = 1
        for left in (2, 0):
            self.activate(cub, [], ability=index)
            self.assertEqual(self.energy(0), left, "paid on activation, before resolution")
            self.settle()
        self.assertEqual(self.obj(cub).get("counters", {}).get("+1/+1"), 2)
        self.reject(0, {"kind": "activate", "source": cub, "ability": 1, "targets": [], "plan": []})
        self.reject(0, {"kind": "activate", "source": cub, "ability": index, "targets": [], "plan": [], "paying": []})


if __name__ == "__main__":
    unittest.main()
