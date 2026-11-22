"""Pain for All through the real audited client.
from client_support import ClientTestCase
- Its enters trigger can't target the enchanted creature ("any other target").
- Its damage is the enchanted creature's power at resolution, with responses that pump the
  creature or destroy it. Each case checks exact damage records and both viewers."""


class AttachedDealerReviewClientTests(ClientTestCase):
    def dealt(self):
        return sorted((e["source"], next(iter(e["to"].items())), e["amount"])
                      for e in self.engine.state["events"] if e["event"] == "damage")

    def life(self, player):
        return [self.view(v)["players"][player]["life"] for v in (0, 1)]

    def enchant(self, mine=(), theirs=()):
        self.new([{"hand": ["Pain for All", *mine], "battlefield": ["Hill Giant"] + ["Forest"] * 3 + ["Mountain"] * 3},
                  {"hand": list(theirs), "battlefield": ["Grizzly Bears", "Mountain"]}])
        giant, = self.ids("Hill Giant")
        self.cast("Pain for All", [[{"object": giant}]], mana=self.ids('Mountain', zone="battlefield", controller=self.engine.state["asked"]["player"])[0:3], spend={'R': 3})
        self.pass_round()
        asked = self.engine.state["asked"]
        self.assertEqual((asked["kind"], asked["player"]), ("targets", 0))
        return giant

    def test_trigger_cannot_target_the_enchanted_creature(self):
        giant = self.enchant()
        bears, = self.ids("Grizzly Bears")
        self.reject(0, {"kind": "targets", "targets": [[{"object": giant}]]})
        self.engine.act({"kind": "targets", "targets": [[{"object": bears}]]})
        self.pass_round()
        self.assertEqual(self.dealt(), [(giant, ("object", bears), 3)])
        for viewer in (0, 1):
            self.assertNotIn(bears, self.view(viewer)["battlefield"])
            self.assertEqual(self.view(viewer)["objects"][str(giant)]["damage"], 0)

    def test_pump_in_response_is_read_at_resolution(self):
        giant = self.enchant(mine=["Giant Growth"])
        self.engine.act({"kind": "targets", "targets": [[{"player": 1}]]})
        self.cast("Giant Growth", [[{"object": giant}]], mana=self.ids('Forest', zone="battlefield", controller=self.engine.state["asked"]["player"])[0:1], spend={'G': 1})
        self.pass_round()
        self.pass_round()
        self.assertEqual(self.dealt(), [(giant, ("player", 1), 6)])
        self.assertEqual(self.life(1), [14, 14])

    def test_destroyed_creature_deals_its_last_known_power(self):
        # Pain for All ruling: the enchanted creature's power as it last existed.
        giant = self.enchant(theirs=["Lightning Bolt"])
        self.engine.act({"kind": "targets", "targets": [[{"player": 1}]]})
        self.engine.act({"kind": "pass"})
        self.assertEqual(self.engine.state["asked"]["player"], 1)
        self.cast("Lightning Bolt", [[{"object": giant}]], mana=self.ids('Mountain', zone="battlefield", controller=self.engine.state["asked"]["player"])[0:1], spend={'R': 1})
        bolt = self.engine.state["stack"][-1]
        self.pass_round()
        self.assertEqual(self.dealt(), [(bolt, ("object", giant), 3)])
        for viewer in (0, 1):
            self.assertNotIn(giant, self.view(viewer)["battlefield"])
        self.pass_round()
        self.assertEqual(self.dealt(), [(giant, ("player", 1), 3)])
        self.assertEqual(self.life(1), [17, 17])
