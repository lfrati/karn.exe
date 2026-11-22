"""Full pinned player hexproof/shroud cards
through the real audited client. Hexproof on a player: its opponents' spells and abilities
can't target it (702.11c); shroud: no spell or ability can (702.18a); the target is checked
again on resolution (608.2b)."""
import unittest
from client_support import ClientTestCase


class PlayerHexproofReviewClientTests(ClientTestCase):
    def bolt(self):
        state = self.engine.state
        actor = state["asked"]["player"]
        card = next(o for o in state["players"][actor]["hand"] if state["objects"][str(o)]["name"] == "Lightning Bolt")
        mountain = next(o for o in self.ids("Mountain", zone="battlefield", controller=actor)
                        if not state["objects"][str(o)].get("tapped"))
        return {"kind": "cast", "card": card, "plan": [{"source": mountain, "ability": 0}], "mana": {"R": 1}}

    def assert_player_targets(self, move, allowed):
        # Expectations enumerate the finite player domain independently, without move discovery.
        for player in range(len(self.engine.state["players"])):
            self.validate_action(move | {"targets": [[{"player": player}]]}, player in allowed)

    def test_leyline_keeps_opponent_spells_off_its_controller_and_the_move_is_refused(self):
        self.new([{"hand": ["Lightning Bolt"], "battlefield": ["Mountain"]}, {"battlefield": ["Leyline of Sanctity"]}])
        move = self.bolt()
        self.assert_player_targets(move, [0])
        self.reject(0, {"kind": "cast", "card": move["card"], "plan": move["plan"], "mana": {"R": 1},
                        "targets": [[{"player": 1}]], "paying": []})

    def test_leyline_controller_may_still_target_itself(self):
        self.new([{"hand": ["Lightning Bolt"], "battlefield": ["Mountain", "Leyline of Sanctity"]}, {}])
        self.assert_player_targets(self.bolt(), [0, 1])
        self.cast("Lightning Bolt", [[{"player": 0}]], mana=self.ids("Mountain", zone="battlefield", controller=0)[:1], spend={'R': 1})
        self.pass_round()
        self.assertEqual(self.engine.state["players"][0]["life"], 17)

    def test_teyo_stops_an_opponent_activated_ability(self):
        self.new([{"battlefield": ["Prodigal Pyromancer"]}, {"battlefield": ["Teyo, the Shieldmage"]}])
        pyromancer, = self.ids("Prodigal Pyromancer")
        teyo, = self.ids("Teyo, the Shieldmage")
        move = {"kind": "activate", "source": pyromancer, "ability": 0, "plan": []}
        self.assert_player_targets(move, [0])
        # the planeswalker itself remains targetable: only its controller has hexproof
        self.validate_action(move | {"targets": [[{"object": teyo}]]})
        self.validate_action(move | {"targets": [[{"object": pyromancer}]]})

    def test_ivory_mask_shroud_stops_its_own_controller_too(self):
        self.new([{"hand": ["Lightning Bolt"], "battlefield": ["Mountain", "Ivory Mask"]}, {}])
        self.assert_player_targets(self.bolt(), [1])

    def test_stolen_true_believer_moves_shroud_to_its_new_controller(self):
        self.new([{"hand": ["Act of Treason", "Lightning Bolt"], "battlefield": ["Mountain"] * 4},
                  {"battlefield": ["True Believer"]}])
        believer, = self.ids("True Believer")
        self.assert_player_targets(self.bolt(), [0])
        self.cast("Act of Treason", [[{"object": believer}]], mana=self.ids("Mountain", zone="battlefield", controller=0)[:3], spend={'R': 3})
        self.pass_round()
        self.assertEqual(self.engine.state["objects"][str(believer)]["controller"], 0)
        self.assert_player_targets(self.bolt(), [1])

    def flash_believer_in_response(self, spell, targets, mana, spend):
        """Player 0 targets player 1; player 1 flashes in True Believer before it resolves."""
        self.cast(spell, targets, mana=mana, spend=spend)
        self.engine.act({"kind": "pass"})
        self.assertEqual(self.engine.state["asked"]["player"], 1)
        self.cast("True Believer", mana=self.ids("Plains", zone="battlefield", controller=1)[:2], spend={'W': 2})
        self.pass_round()
        self.assertEqual(self.ids("True Believer")[0] in self.engine.state["battlefield"], True)
        self.pass_round()

    def test_mogg_whose_player_gained_shroud_does_nothing(self):
        self.new([{"hand": ["Mogg Infestation"], "battlefield": ["Mountain"] * 5},
                  {"hand": ["True Believer"], "battlefield": ["Plains", "Plains", "Leyline of Anticipation", "Grizzly Bears"]}])
        bears, = self.ids("Grizzly Bears")
        self.flash_believer_in_response("Mogg Infestation", [[{"player": 1}]], self.ids("Mountain", zone="battlefield", controller=0)[:5], {"R": 5})
        self.assertIn(bears, self.engine.state["battlefield"])
        self.assertFalse([o for o in self.engine.state["battlefield"] if self.engine.state["objects"][str(o)].get("token")])

    def test_twisted_fates_keeps_its_legal_permanent_target_but_counts_no_creatures(self):
        self.new([{"hand": ["Twisted Fates"], "battlefield": ["Plains", "Plains", "Swamp", "Swamp", "Swamp"]},
                  {"hand": ["True Believer"], "battlefield": ["Plains", "Plains", "Leyline of Anticipation", "Grizzly Bears"]}])
        bears, = self.ids("Grizzly Bears")
        anticipation, = self.ids("Leyline of Anticipation")
        self.flash_believer_in_response("Twisted Fates", [[{"object": anticipation}], [{"player": 1}]], self.ids("Plains", zone="battlefield", controller=0)[:2] + self.ids("Swamp", zone="battlefield", controller=0)[:3], {"W": 2, "B": 3})
        self.assertNotIn(anticipation, self.engine.state["battlefield"])
        self.assertEqual(self.engine.state["objects"][str(bears)].get("counters", {}), {})


if __name__ == "__main__":
    unittest.main()
