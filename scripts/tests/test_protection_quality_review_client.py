"""Protection from a quality, through the audited Python/JSON client at a named executable.

702.16b/c/e/j, 303.4d, 608.2b, 615.12, and the printed full-card text determine
the expected results. MAGIC_REVIEW_ENGINE selects a separate checkout's build.
"""
import os
import subprocess
import unittest

import test_multiplayer_outcome_client as helpers
import test_player_hexproof_review_client as targeting


class ProtectionQualityReviewClientTests(unittest.TestCase):
    new = helpers.MultiplayerOutcomeClientTests.new
    close_engine = helpers.MultiplayerOutcomeClientTests.close_engine
    ids = helpers.MultiplayerOutcomeClientTests.ids
    cast = helpers.MultiplayerOutcomeClientTests.cast
    validate_action = helpers.MultiplayerOutcomeClientTests.validate_action
    bolt = targeting.PlayerHexproofReviewClientTests.bolt
    assert_player_targets = targeting.PlayerHexproofReviewClientTests.assert_player_targets
    reject = helpers.MultiplayerOutcomeClientTests.reject
    view = helpers.MultiplayerOutcomeClientTests.view

    @classmethod
    def setUpClass(cls):
        cls.executable = os.environ.get("MAGIC_REVIEW_ENGINE") or os.environ.get("GAME_SERVE_REVIEW")
        if not cls.executable:
            subprocess.run(["cargo", "build", "--profile", "fast", "-p", "game", "--bin", "game-serve"],
                           cwd=helpers.play.ROOT, check=True, stdout=subprocess.DEVNULL)

    def setUp(self):
        self.engine = helpers.play.Engine(audit=True, executable=self.executable)
        self.addCleanup(self.close_engine)

    def settle(self):
        for _ in range(80):
            state = self.engine.state
            if not state["stack"] or state["asked"]["kind"] != "priority":
                return
            self.engine.act({"kind": "pass"})
        self.fail("resolution did not reach a boundary")

    def color(self, who, color):
        self.settle()
        asked = self.engine.state["asked"]
        self.assertEqual((asked["kind"], asked["player"]), ("value", who))
        self.assertIn({"color": color}, asked["options"])
        self.engine.act({"kind": "choose", "answer": {"value": {"color": color}}})
        self.settle()

    def activate(self, name, targets=None, *, ability, mana=(), spend=None):
        source = self.ids(name, zone="battlefield")[0]
        self.engine.act({"kind": "activate", "source": source, "ability": ability,
                         "targets": targets or [], "plan": [{"source": o, "ability": 0} for o in mana],
                         **({"mana": spend} if spend is not None else {})})

    def advance(self, predicate):
        for _ in range(160):
            if predicate(self.engine.state):
                return
            asked = self.engine.state["asked"]
            if asked["kind"] == "attackers":
                self.engine.act({"kind": "attack", "attacks": []})
            elif asked["kind"] == "may":
                self.engine.act({"kind": "choose", "answer": "no"})
            else:
                self.assertEqual(asked["kind"], "priority")
                self.engine.act({"kind": "pass"})
        self.fail("did not reach requested phase")

    def main(self, player):
        self.advance(lambda s: s["active"] == player and s["part"] == "PrecombatMain"
                     and not s["stack"] and s["asked"]["kind"] == "priority"
                     and s["asked"]["player"] == player)

    def test_tiger_in_response_rechecks_bolt_and_refuses_an_explicit_illegal_target(self):
        self.new([{"hand": ["Lightning Bolt", "Lightning Bolt"], "battlefield": ["Mountain", "Mountain"]},
                  {"hand": ["Seht's Tiger"], "battlefield": ["Plains"] * 4}])
        self.cast("Lightning Bolt", [[{"player": 1}]], mana=self.ids('Mountain', zone="battlefield", controller=self.engine.state["asked"]["player"])[0:1], spend={'R': 1})
        self.engine.act({"kind": "pass"})
        self.cast("Seht's Tiger", mana=self.ids('Plains', zone="battlefield", controller=self.engine.state["asked"]["player"])[0:4], spend={'W': 4})
        self.color(1, "red")
        self.assertEqual(self.engine.state["players"][1]["life"], 20)
        self.assertEqual(self.engine.state["stack"], [])
        self.assertEqual(len(self.engine.state["players"][0]["graveyard"]), 1)
        move = self.bolt()
        self.validate_action(move | {"targets": [[{"player": 1}]]}, False)
        self.reject(0, move | {"targets": [[{"player": 1}]]})

    def test_friendly_curse_can_enchant_its_controller_then_protection_removes_it(self):
        self.new([{"hand": ["Curse of the Nightly Hunt", "Seht's Tiger"],
                   "battlefield": ["Plains"] * 4 + ["Mountain"] * 3}, {}])
        self.cast("Curse of the Nightly Hunt", [[{"player": 0}]], mana=self.ids('Mountain', zone="battlefield", controller=self.engine.state["asked"]["player"])[0:3], spend={'R': 3})
        self.settle()
        curse, = self.ids("Curse of the Nightly Hunt")
        self.assertIn(curse, self.engine.state["battlefield"])
        self.cast("Seht's Tiger", mana=self.ids('Plains', zone="battlefield", controller=self.engine.state["asked"]["player"])[0:4], spend={'W': 4})
        self.color(0, "red")
        self.assertNotIn(curse, self.engine.state["battlefield"])
        self.assertEqual(len(self.engine.state["players"][0]["graveyard"]), 1)

    def test_unpreventable_untargeted_damage_bypasses_coffin_without_enabling_targeting(self):
        for unpreventable in (False, True):
            with self.subTest(unpreventable=unpreventable):
                self.new([{"battlefield": ["The Stasis Coffin", "Plains", "Plains"]},
                          {"hand": ["Flaring Pain", "Flame Rift", "Lightning Bolt"],
                           "battlefield": ["Mountain"] * 6}])
                self.activate("The Stasis Coffin", ability=0, mana=self.ids("Plains", zone="battlefield", controller=0)[:2], spend={'W': 2})
                self.settle()
                self.main(1)
                self.validate_action(self.bolt() | {"targets": [[{"player": 0}]]}, False)
                if unpreventable:
                    self.cast("Flaring Pain", mana=self.ids('Mountain', zone="battlefield", controller=self.engine.state["asked"]["player"])[0:2], spend={'R': 2})
                    self.settle()
                self.validate_action(self.bolt() | {"targets": [[{"player": 0}]]}, False)
                self.reject(1, self.bolt() | {"targets": [[{"player": 0}]]})
                self.cast("Flame Rift", mana=self.ids('Mountain', zone="battlefield", controller=self.engine.state["asked"]["player"])[2:4], spend={'R': 2})
                self.settle()
                self.assertEqual([p["life"] for p in self.engine.state["players"]],
                                 [16 if unpreventable else 20, 16])

    def test_gilded_light_shroud_stops_self_targeting_and_expires_at_cleanup(self):
        self.new([{"hand": ["Gilded Light", "Lightning Bolt"],
                   "battlefield": ["Plains"] * 3 + ["Mountain"] * 2},
                  {"hand": ["Lightning Bolt"], "battlefield": ["Mountain"]}])
        self.cast("Gilded Light", mana=self.ids('Plains', zone="battlefield", controller=self.engine.state["asked"]["player"])[0:2], spend={'W': 2})
        self.settle()
        self.assert_player_targets(self.bolt(), [1])
        self.main(1)
        self.validate_action(self.bolt() | {"targets": [[{"player": 0}]]})

    def test_blossoming_calm_hexproof_allows_self_and_expires_before_rebound(self):
        self.new([{"hand": ["Blossoming Calm", "Lightning Bolt"],
                   "battlefield": ["Plains", "Mountain"]},
                  {"hand": ["Lightning Bolt"], "battlefield": ["Mountain"]}])
        self.cast("Blossoming Calm", mana=self.ids('Plains', zone="battlefield", controller=self.engine.state["asked"]["player"])[0:1], spend={'W': 1})
        self.settle()
        self.assertEqual(self.engine.state["players"][0]["life"], 22)
        self.validate_action(self.bolt() | {"targets": [[{"player": 0}]]})
        self.main(1)
        self.validate_action(self.bolt() | {"targets": [[{"player": 0}]]}, False)
        self.advance(lambda s: s["active"] == 0 and s["part"] == "Upkeep"
                     and s["asked"]["kind"] == "priority" and s["asked"]["player"] == 0)
        self.validate_action(self.bolt() | {"targets": [[{"player": 0}]]})

    def test_linked_blue_protection_distinguishes_the_actual_ability_sources(self):
        self.new([{"hand": ["Order of the Stars"], "battlefield": ["Plains"]},
                  {"battlefield": ["Prodigal Sorcerer", "Prodigal Pyromancer"]}])
        self.cast("Order of the Stars", mana=self.ids('Plains', zone="battlefield", controller=self.engine.state["asked"]["player"])[0:1], spend={'W': 1})
        self.color(0, "blue")
        holder, = self.ids("Order of the Stars")
        self.engine.act({"kind": "pass"})
        self.validate_action({"kind": "activate", "source": self.ids("Prodigal Sorcerer")[0], "ability": 0, "targets": [[{"object": holder}]]}, False)
        self.validate_action({"kind": "activate", "source": self.ids("Prodigal Pyromancer")[0], "ability": 0, "targets": [[{"object": holder}]]})
        self.activate("Prodigal Pyromancer", [[{"object": holder}]], ability=0)
        self.settle()
        self.assertNotIn(holder, self.engine.state["battlefield"])

    def test_hunted_horror_tokens_keep_black_protection_on_the_wire(self):
        self.new([{"hand": ["Hunted Horror", "Dismember", "Lightning Bolt"],
                   "battlefield": ["Swamp"] * 4 + ["Mountain"] * 2}, {}])
        self.cast("Hunted Horror", mana=self.ids('Swamp', zone="battlefield", controller=self.engine.state["asked"]["player"])[0:2], spend={'B': 2})
        self.settle()
        self.assertEqual(self.engine.state["asked"]["kind"], "targets")
        self.engine.act({"kind": "targets", "targets": [[{"player": 1}]]})
        self.settle()
        centaurs = [o for o in self.engine.state["battlefield"]
                    if self.engine.state["objects"][str(o)].get("token")]
        self.assertEqual(len(centaurs), 2)
        for centaur in centaurs:
            self.validate_action({"kind": "cast", "card": self.ids("Dismember")[0], "life": [True, True], "plan": [{"source": self.ids("Swamp", zone="battlefield", controller=0)[2], "ability": 0}], "targets": [[{"object": centaur}]]}, False)
            self.validate_action(self.bolt() | {"targets": [[{"object": centaur}]]})
        self.cast("Lightning Bolt", [[{"object": centaurs[0]}]], mana=self.ids('Mountain', zone="battlefield", controller=self.engine.state["asked"]["player"])[0:1], spend={'R': 1})
        self.settle()
        self.assertNotIn(centaurs[0], self.engine.state["battlefield"])


if __name__ == "__main__":
    unittest.main()
