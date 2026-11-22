"""Audited JSON/client checks of 118.3 and 118.12a."""
import os
import sys
import unittest
from pathlib import Path
sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import play
import client_support as helpers

class UnlessObjectReviewClientTests(unittest.TestCase):
    new = helpers.ClientTestCase.new
    view = helpers.ClientTestCase.view
    hand = helpers.ClientTestCase.hand
    ids = helpers.ClientTestCase.ids
    cast = helpers.ClientTestCase.cast
    pass_round = helpers.ClientTestCase.pass_round
    choose = helpers.ClientTestCase.choose
    reject = helpers.ClientTestCase.reject
    close_engine = helpers.ClientTestCase.close_engine

    def setUp(self):
        self.engine = play.Engine(audit=True, executable=os.environ.get("MAGIC_REVIEW_ENGINE"))
        self.addCleanup(self.close_engine)

    def advance_until(self, kind):
        for _ in range(180):
            asked = self.engine.state["asked"]
            if asked["kind"] == kind:
                return asked
            self.engine.act({"kind": "attack", "attacks": []} if asked["kind"] == "attackers" else {"kind": "pass"})
        self.fail(f"did not reach {kind}")

    def test_return_choice_has_move_verb_and_preserves_owner(self):
        self.engine.ask({"cmd": "new", "players": [{"library": ["Forest"] * 8, "battlefield": ["Waterspout Djinn", "Island", "Island"]}, {"library": ["Forest"] * 8}]})
        self.advance_until("may")
        self.choose("yes")
        asked = self.engine.state["asked"]
        self.assertEqual((asked["kind"], asked["verb"], asked["count"]), ("objects", "Move", 1))
        chosen = asked["among"][1]
        self.reject(1, {"kind": "choose", "answer": {"objects": [chosen]}})
        self.choose({"objects": [chosen]})
        self.assertNotIn(chosen, self.engine.state["battlefield"])
        self.assertTrue(any(self.engine.state["objects"][str(c)]["name"] == "Island" for c in self.hand(0)))
        self.assertTrue(any(self.engine.state["objects"][str(c)]["name"] == "Waterspout Djinn" for c in self.engine.state["battlefield"]))

    def test_tap_choice_has_tap_verb_and_excludes_already_tapped_bridge(self):
        self.new([{"battlefield": ["Forest", "Island"], "hand": ["Command Bridge"]}, {}])
        card = next(c for c in self.hand(0) if self.engine.state["objects"][str(c)]["name"] == "Command Bridge")
        self.engine.act({"kind": "play", "card": card})
        self.pass_round()
        self.choose("yes")
        asked = self.engine.state["asked"]
        self.assertEqual((asked["kind"], asked["verb"], asked["count"]), ("objects", "Tap", 1))
        bridge, = [c for c in self.ids("Command Bridge") if c in self.engine.state["battlefield"]]
        self.assertNotIn(bridge, asked["among"])
        chosen = asked["among"][0]
        self.reject(0, {"kind": "choose", "answer": {"objects": [bridge]}})
        self.choose({"objects": [chosen]})
        self.assertTrue(self.engine.state["objects"][str(chosen)]["tapped"])
        self.assertIn(bridge, self.engine.state["battlefield"])

    def test_enchanters_bane_target_controller_pays_by_sacrificing_that_enchantment(self):
        for pay in [False, True]:
            with self.subTest(pay=pay):
                self.new([{"battlefield": ["Enchanter's Bane"]}, {"battlefield": ["Glorious Anthem"]}, {}])
                asked = self.advance_until("targets")
                target, = self.ids("Glorious Anthem")
                self.engine.act({"kind": "targets", "targets": [[{"object": target}]]})
                self.pass_round()
                self.assertEqual((self.engine.state["asked"]["kind"], self.engine.state["asked"]["player"]), ("may", 1))
                self.reject(0, {"kind": "choose", "answer": "yes"})
                self.choose("yes" if pay else "no")
                self.assertEqual(target in self.engine.state["battlefield"], not pay)
                self.assertEqual([p["life"] for p in self.engine.state["players"]], [20, 20 if pay else 17, 20])

    def test_reality_smasher_discard_is_private_and_payment_goes_to_the_caster(self):
        self.new([{"battlefield": ["Mountain"], "hand": ["Lightning Bolt", "Forest", "Island"]}, {"battlefield": ["Reality Smasher"]}, {}])
        target, = self.ids("Reality Smasher")
        self.cast("Lightning Bolt", [[{"object": target}]], mana=self.ids("Mountain", controller=0), spend={'R': 1})
        self.pass_round()
        self.assertEqual((self.engine.state["asked"]["kind"], self.engine.state["asked"]["player"]), ("may", 0))
        self.choose("yes")
        asked = self.engine.state["asked"]
        self.assertEqual((asked["kind"], asked["verb"], asked["player"]), ("objects", "Discard", 0))
        chosen = asked["among"][0]
        self.assertNotIn("among", self.view(2)["asked"])
        self.assertNotIn(str(chosen), self.view(2)["objects"])
        self.choose({"objects": [chosen]})
        self.assertEqual(len(self.engine.state["stack"]), 1)
        self.pass_round()
        self.assertEqual(self.engine.state["objects"][str(target)]["damage"], 3)

    def test_hecatomb_short_payment_is_not_offered_and_leaves_creatures_alone(self):
        self.new([{"battlefield": ["Swamp"] * 3 + ["Grizzly Bears"] * 3, "hand": ["Hecatomb"]}, {}])
        creatures = self.ids("Grizzly Bears")
        self.cast("Hecatomb", mana=self.ids("Swamp", controller=0), spend={'B': 3})
        self.pass_round()
        self.pass_round()
        self.assertEqual(self.engine.state["asked"]["kind"], "priority")
        self.assertTrue(all(c in self.engine.state["battlefield"] for c in creatures))
        self.assertFalse(any(self.engine.state["objects"][str(c)]["name"] == "Hecatomb" for c in self.engine.state["battlefield"]))

if __name__ == "__main__":
    unittest.main()
