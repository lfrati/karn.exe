"""A spell's "each other creature that player controls" is other than
the earlier target, not the spell (608.2c), for full Flames of the Raze-Boar and Public
Execution through the real audited JSON client."""
import unittest
from test_grant_client import GrantClientTests as Helpers


class TargetOtherReviewClientTests(unittest.TestCase):
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
        return sorted((e["to"]["object"], e["amount"]) for e in self.engine.state["events"]
                      if e["event"] == "damage" and "object" in e["to"])

    def seen(self, obj, field):
        return [self.view(v)["objects"][str(obj)][field] for v in (0, 1)]

    def flames(self, mine):
        self.new([{"hand": ["Flames of the Raze-Boar"], "battlefield": [mine] + ["Mountain"] * 6},
                  {"battlefield": ["Spearbreaker Behemoth", "Hill Giant", "Grizzly Bears"]}])
        behemoth, = self.ids("Spearbreaker Behemoth")
        giant, = self.ids("Hill Giant")
        bears, = self.ids("Grizzly Bears")
        self.cast("Flames of the Raze-Boar", [[{"object": behemoth}]], mana=self.ids('Mountain', zone="battlefield", controller=0)[:6], spend={'R': 6})
        self.pass_round()
        return behemoth, giant, bears

    def test_flames_target_is_dealt_four_and_each_other_creature_two(self):
        # indestructible target survives with exactly 4 damage; the 2 goes to the others only
        behemoth, giant, bears = self.flames("Craw Wurm")
        wurm, = self.ids("Craw Wurm")
        self.assertEqual(self.dealt(), sorted([(behemoth, 4), (giant, 2), (bears, 2)]))
        self.assertEqual(self.seen(behemoth, "damage"), [4, 4])
        self.assertEqual(self.seen(giant, "damage"), [2, 2])
        self.assertEqual(self.object(wurm)["damage"], 0)
        for viewer in (0, 1):
            battlefield = self.view(viewer)["battlefield"]
            self.assertIn(behemoth, battlefield)
            self.assertIn(giant, battlefield)
            self.assertNotIn(bears, battlefield)

    def test_flames_without_a_power_four_creature_deals_only_the_four(self):
        behemoth, giant, bears = self.flames("Llanowar Elves")
        self.assertEqual(self.dealt(), [(behemoth, 4)])
        self.assertEqual(self.seen(giant, "damage"), [0, 0])
        self.assertIn(bears, self.view(1)["battlefield"])

    def test_public_execution_surviving_target_keeps_its_power(self):
        self.new([{"hand": ["Public Execution"], "battlefield": ["Grizzly Bears"] + ["Swamp"] * 6},
                  {"battlefield": ["Spearbreaker Behemoth", "Hill Giant"]}])
        behemoth, = self.ids("Spearbreaker Behemoth")
        giant, = self.ids("Hill Giant")
        bears, = self.ids("Grizzly Bears")
        self.cast("Public Execution", [[{"object": behemoth}]], mana=self.ids('Swamp', zone="battlefield", controller=0)[:6], spend={'B': 6})
        self.pass_round()
        self.assertIn(behemoth, self.view(1)["battlefield"])
        self.assertEqual(self.seen(behemoth, "power"), [5, 5])
        self.assertEqual(self.seen(giant, "power"), [1, 1])
        self.assertEqual(self.seen(bears, "power"), [2, 2])

    def test_public_execution_destroyed_target_and_the_others_shrink(self):
        self.new([{"hand": ["Public Execution"], "battlefield": ["Swamp"] * 6},
                  {"battlefield": ["Spearbreaker Behemoth", "Hill Giant"]}])
        behemoth, = self.ids("Spearbreaker Behemoth")
        giant, = self.ids("Hill Giant")
        self.cast("Public Execution", [[{"object": giant}]], mana=self.ids('Swamp', zone="battlefield", controller=0)[:6], spend={'B': 6})
        self.pass_round()
        self.assertNotIn(giant, self.view(1)["battlefield"])
        self.assertEqual(self.seen(behemoth, "power"), [3, 3])


del Helpers
