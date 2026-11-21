"""Delney regressions through the actual Python/JSON client."""
import unittest
from test_grant_client import GrantClientTests as Helpers


class AdditionalTriggerClientTests(unittest.TestCase):
    setUpClass = classmethod(Helpers.setUpClass.__func__)
    setUp = Helpers.setUp
    close_engine = Helpers.close_engine
    new = Helpers.new
    view = Helpers.view
    hand = Helpers.hand
    ids = Helpers.ids
    cast = Helpers.cast
    pass_round = Helpers.pass_round
    choose = Helpers.choose
    reject = Helpers.reject
    object = Helpers.object
    activate = Helpers.activate
    step = Helpers.step
    advance = Helpers.advance

    def tokens(self, name):
        return [self.object(o) for o in self.engine.state["battlefield"] if name in self.object(o)["name"]]

    def settle(self):
        for _ in range(100):
            if not self.engine.state["stack"] and self.engine.state["asked"]["kind"] == "priority":
                return
            self.step()
        self.fail("did not settle")

    def test_separate_printed_triggers_order_and_keep_captured_low_power_eligibility(self):
        self.new([{"battlefield": ["Delney, Streetwise Lookout", "Monastery Swiftspear", "Young Pyromancer", "Mountain"],
                   "hand": ["Shock"]}, {}, {}])
        self.cast("Shock", [[{"player": 2}]], mana=self.ids('Mountain', zone="battlefield", controller=self.engine.state["asked"]["player"])[0:1], spend={'R': 1})
        asked = self.engine.state["asked"]
        self.assertEqual((asked["kind"], asked["player"]), ("order", 0))
        self.assertEqual(len(asked["abilities"]), 4)
        command = {"kind": "order", "order": [3, 2, 1, 0]}
        self.reject(1, command)
        self.engine.act(command)
        self.assertEqual(len(self.engine.state["stack"]), 5)
        self.settle()
        self.assertEqual(self.object(self.ids("Monastery Swiftspear")[0])["power"], 3)
        self.assertEqual(len(self.tokens("Elemental")), 2)
        self.assertEqual(self.engine.state["players"][2]["life"], 18)

    def test_tataru_additional_entry_instances_choose_different_opponents_but_treasure_cap_stays_one(self):
        self.new([{"battlefield": ["Delney, Streetwise Lookout", "Plains", "Plains"], "hand": ["Tataru Taru"]}, {}, {}])
        self.cast("Tataru Taru", mana=self.ids('Plains', zone="battlefield", controller=self.engine.state["asked"]["player"])[0:2], spend={'W': 2})
        self.pass_round()
        for player in [1, 2]:
            self.assertEqual((self.engine.state["asked"]["kind"], self.engine.state["asked"]["player"]), ("targets", 0))
            command = {"kind": "targets", "targets": [[{"player": player}]]}
            self.reject(1, command)
            self.engine.act(command)
        before = len(self.hand(0))
        self.pass_round()
        self.assertEqual((self.engine.state["asked"]["kind"], self.engine.state["asked"]["player"]), ("may", 2))
        self.reject(0, {"kind": "choose", "answer": "yes"})
        self.choose("yes")
        self.pass_round()  # the one capped Treasure trigger
        self.assertEqual(len(self.tokens("Treasure")), 1)
        self.pass_round()
        self.assertEqual((self.engine.state["asked"]["kind"], self.engine.state["asked"]["player"]), ("may", 1))
        self.choose("yes")
        self.assertEqual(len(self.hand(0)), before + 2)
        self.assertEqual(len(self.tokens("Treasure")), 1)
        self.assertEqual(self.engine.state["asked"]["kind"], "priority")

    def test_graha_additional_pending_payment_decline_or_completion_rechecks_action_limit(self):
        for decline in [False, True]:
            with self.subTest(decline=decline):
                self.new([{"battlefield": ["Delney, Streetwise Lookout", "G'raha Tia, Scion Reborn", "Mountain"],
                           "hand": ["Shock"]}, {}, {}])
                self.cast("Shock", [[{"player": 2}]], mana=self.ids('Mountain', zone="battlefield", controller=self.engine.state["asked"]["player"])[0:1], spend={'R': 1})
                self.assertEqual(len(self.engine.state["stack"]), 3)
                self.pass_round()
                self.assertEqual(self.engine.state["asked"]["kind"], "pay")
                self.reject(1, {"kind": "choose", "answer": "yes"})
                self.choose("no" if decline else {"pay": {"plan": []}})
                self.pass_round()
                if decline:
                    self.assertEqual(self.engine.state["asked"]["kind"], "pay")
                    self.choose({"pay": {"plan": []}})
                else:
                    self.assertEqual(self.engine.state["asked"]["kind"], "priority")
                self.assertEqual(len(self.tokens("Hero")), 1)
                self.assertEqual(self.tokens("Hero")[0]["counters"]["+1/+1"], 1)
                self.assertEqual(self.engine.state["players"][0]["life"], 19)
                self.settle()

    def test_helm_granted_damage_trigger_is_an_ordinary_eligible_ability(self):
        self.new([{"battlefield": ["Delney, Streetwise Lookout", "Storm Crow", "Viridian Longbow", "Island"] + ["Forest"] * 6,
                   "hand": ["Helm of the Ghastlord"]}, {}, {}])
        crow = self.ids("Storm Crow")[0]
        self.cast("Helm of the Ghastlord", [[{"object": crow}]], mana=self.ids('Island', zone="battlefield", controller=self.engine.state["asked"]["player"])[0:1] + self.ids('Forest', zone="battlefield", controller=self.engine.state["asked"]["player"])[0:3], spend={'U': 1, 'G': 3})
        self.pass_round()
        self.assertEqual(self.object(crow)["power"], 2)
        self.activate(self.ids("Viridian Longbow")[0], [[{"object": crow}]], ability=2, mana=self.ids("Forest", zone="battlefield", controller=0)[3:6], spend={'G': 3})
        self.pass_round()
        self.activate(crow, [[{"player": 2}]], ability=2)
        self.pass_round()
        self.assertEqual(len(self.engine.state["stack"]), 2)
        before = len(self.hand(0))
        self.pass_round()
        self.pass_round()
        self.assertEqual(len(self.hand(0)), before + 2)

    def blocking(self):
        self.new([{"battlefield": ["Delney, Streetwise Lookout", "Grizzly Bears", "Forest"], "hand": ["Giant Growth"]},
                  {"battlefield": ["Grizzly Bears", "Hill Giant", "Forest"], "hand": ["Giant Growth"]}, {}])
        self.advance(lambda s: s["turn"] > 1 and s["active"] == 0 and s["asked"]["kind"] == "attackers")
        bear = next(o for o in self.ids("Grizzly Bears") if self.object(o)["controller"] == 0)
        delney = self.ids("Delney, Streetwise Lookout")[0]
        self.engine.act({"kind": "attack", "attacks": [[bear, {"player": 1}], [delney, {"player": 1}]]})
        return bear, delney

    def test_blocking_offers_reject_large_blocker_then_preserve_existing_block_after_growth(self):
        bear, delney = self.blocking()
        self.advance(lambda s: s["asked"]["kind"] == "blockers")
        asked = self.engine.state["asked"]
        giant = self.ids("Hill Giant")[0]
        blocker = next(o for o in self.ids("Grizzly Bears") if self.object(o)["controller"] == 1)
        self.assertTrue(all(not x["blocks"] for x in asked["able"] if x["creature"] == giant))
        self.assertEqual(set(next(x["blocks"] for x in asked["able"] if x["creature"] == blocker)), {bear, delney})
        self.reject(1, {"kind": "block", "blocks": [[giant, bear]]})
        self.engine.act({"kind": "block", "blocks": [[blocker, bear]]})
        self.advance(lambda s: s["asked"]["kind"] == "priority" and s["asked"]["player"] == 1)
        self.cast("Giant Growth", [[{"object": blocker}]], mana=self.ids('Forest', zone="battlefield", controller=self.engine.state["asked"]["player"])[0:1], spend={'G': 1})
        self.pass_round()
        self.assertEqual(self.object(blocker)["power"], 5)
        self.advance(lambda s: s["part"] == "PostcombatMain")
        self.assertEqual(self.engine.state["players"][1]["life"], 18, "only Delney connects; Bear stays blocked")

    def test_power_change_before_declaration_updates_large_blocker_offer(self):
        bear, delney = self.blocking()
        self.cast("Giant Growth", [[{"object": bear}]], mana=self.ids('Forest', zone="battlefield", controller=self.engine.state["asked"]["player"])[0:1], spend={'G': 1})
        self.pass_round()
        self.advance(lambda s: s["asked"]["kind"] == "blockers")
        giant = self.ids("Hill Giant")[0]
        option = next(x for x in self.engine.state["asked"]["able"] if x["creature"] == giant)
        self.assertIn(bear, option["blocks"])
        self.assertNotIn(delney, option["blocks"])
        self.engine.act({"kind": "block", "blocks": [[giant, bear]]})


del Helpers
