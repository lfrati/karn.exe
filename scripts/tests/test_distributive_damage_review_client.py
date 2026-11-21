"""Each member of a distributive damage source deals its own
power (608.2c, 120.3), for full Alpha Brawl and Band Together through the real audited client."""
import unittest
from test_grant_client import GrantClientTests as Helpers


class DistributiveDamageReviewClientTests(unittest.TestCase):
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

    def dealt(self):
        return sorted((e["source"], e["to"].get("object", ("player", e["to"].get("player"))), e["amount"])
                      for e in self.engine.state["events"] if e["event"] == "damage")

    def seen(self, obj, field):
        return [self.view(v)["objects"][str(obj)][field] for v in (0, 1)]

    def test_alpha_brawl_lethally_damaged_creatures_still_deal_their_own_power(self):
        # no state-based actions mid-resolution (704.3): Bears and Giant, already lethally
        # damaged, deal 2 and 3 back; the indestructible target ends with 5, not 5 + 5
        self.new([{"hand": ["Alpha Brawl"], "battlefield": ["Llanowar Elves"] + ["Mountain"] * 8},
                  {"battlefield": ["Spearbreaker Behemoth", "Grizzly Bears", "Hill Giant"]}])
        behemoth, = self.ids("Spearbreaker Behemoth")
        bears, = self.ids("Grizzly Bears")
        giant, = self.ids("Hill Giant")
        elves, = self.ids("Llanowar Elves")
        self.cast("Alpha Brawl", [[{"object": behemoth}]], mana=self.ids('Mountain', zone="battlefield", controller=self.engine.state["asked"]["player"])[0:8], spend={'R': 8})
        self.pass_round()
        self.engine.act({"kind":"choose","answer":{"objects":[bears,giant]}})
        self.assertEqual(self.dealt(), sorted([(behemoth, bears, 5), (behemoth, giant, 5),
                                               (bears, behemoth, 2), (giant, behemoth, 3)]))
        self.assertEqual(self.seen(behemoth, "damage"), [5, 5])
        self.assertEqual(self.object(elves)["damage"], 0)
        for viewer in (0, 1):
            battlefield = self.view(viewer)["battlefield"]
            self.assertIn(behemoth, battlefield)
            self.assertNotIn(bears, battlefield)
            self.assertNotIn(giant, battlefield)

    def band(self, sources):
        self.new([{"hand": ["Band Together"], "battlefield": ["Hill Giant", "Grizzly Bears"] + ["Forest"] * 3},
                  {"battlefield": ["Spearbreaker Behemoth"]}])
        giant, = self.ids("Hill Giant")
        bears, = self.ids("Grizzly Bears")
        behemoth, = self.ids("Spearbreaker Behemoth")
        chosen = {"giant": giant, "bears": bears}
        self.cast("Band Together", [[{"object": chosen[s]} for s in sources], [{"object": behemoth}]], mana=self.ids('Forest', zone="battlefield", controller=self.engine.state["asked"]["player"])[0:3], spend={'G': 3})
        self.pass_round()
        return giant, bears, behemoth

    def test_band_together_each_source_deals_its_own_power(self):
        giant, bears, behemoth = self.band(["giant", "bears"])
        self.assertEqual(self.dealt(), sorted([(giant, behemoth, 3), (bears, behemoth, 2)]))
        self.assertEqual(self.seen(behemoth, "damage"), [5, 5])

    def test_band_together_with_one_source(self):
        giant, bears, behemoth = self.band(["bears"])
        self.assertEqual(self.dealt(), [(bears, behemoth, 2)])
        self.assertEqual(self.seen(behemoth, "damage"), [2, 2])


del Helpers
