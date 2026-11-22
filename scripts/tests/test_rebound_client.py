"""Full Transpose and rebound through the actual Python/JSON client."""
from client_support import ClientTestCase


class ReboundClientTests(ClientTestCase):
    def setup_transpose(self, opponent=None, extra=None):
        self.new([{"hand": ["Transpose", "Sol Ring", "Island"] + (extra or []),
                   "battlefield": ["Swamp"] * 3 + ["Plains"]}, opponent or {}, {}])

    def discard(self, name="Island"):
        asked = self.engine.state["asked"]
        self.assertEqual((asked["kind"], asked["verb"], asked["player"]), ("objects", "Discard", 0))
        card = next((i for i in asked["among"] if self.object(i)["name"] == name), asked["among"][0])
        for observer in (1, 2):
            self.assertNotIn("among", self.view(observer)["asked"])
            self.assertNotIn("hand", self.view(observer)["players"][0])
        self.reject(1, {"kind": "choose", "answer": {"objects": [card]}})
        self.choose({"objects": [card]})

    def original(self):
        self.cast("Transpose", mana=self.ids("Swamp", controller=0)[:3], spend={'B': 3})
        self.pass_round()
        self.discard()
        self.assertEqual(self.engine.state["players"][0]["life"], 19)
        wizard, = [i for i in self.engine.state["battlefield"] if self.object(i)["token"]]
        self.assertEqual((self.object(wizard)["power"], self.object(wizard)["toughness"]), (0, 1))
        card, = [i for i in self.engine.state["exile"] if self.object(i)["name"] == "Transpose"]
        return card, wizard

    def upkeep(self):
        turn = self.engine.state["turn"]
        self.advance(lambda s: s["turn"] > turn and s["active"] == 0 and s["part"] == "Upkeep")
        self.assertEqual(len(self.engine.state["stack"]), 1)
        self.pass_round()
        self.assertEqual((self.engine.state["asked"]["kind"], self.engine.state["asked"]["player"]), ("cast", 0))

    def test_full_transpose_private_discard_token_trigger_and_free_cast_without_recursion(self):
        self.setup_transpose()
        card, wizard = self.original()
        self.upkeep()
        asked = self.engine.state["asked"]
        self.assertEqual(asked["cards"], [card])
        self.reject(1, {"kind": "cast", "card": card, "targets": [], "plan": []})
        self.engine.act({"kind": "cast", "card": card, "targets": [], "plan": []})
        self.assertEqual(len(self.engine.state["stack"]), 2)
        self.pass_round()
        self.assertEqual([p["life"] for p in self.engine.state["players"]][1:], [19, 19])
        self.pass_round()
        self.discard("Sol Ring")
        self.assertEqual(self.engine.state["players"][0]["life"], 18)
        self.assertEqual([i for i in self.engine.state["battlefield"] if self.object(i)["token"]], [wizard])
        self.assertFalse(self.engine.state["exile"])
        self.assertTrue(any(self.object(i)["name"] == "Transpose" for i in self.engine.state["players"][0]["graveyard"]))
        turn = self.engine.state["turn"]
        self.advance(lambda s: s["turn"] > turn and s["active"] == 0 and s["part"] == "Upkeep")
        self.assertFalse(self.engine.state["stack"])

    def test_decline_expires_offer_and_leaves_card_exiled(self):
        self.setup_transpose()
        card, _ = self.original()
        self.upkeep()
        self.choose("no")
        self.assertIn(card, self.engine.state["exile"])
        self.reject(0, {"kind": "cast", "card": card, "targets": [], "plan": []})
        turn = self.engine.state["turn"]
        self.advance(lambda s: s["turn"] > turn and s["active"] == 0 and s["part"] == "Upkeep")
        self.assertFalse(self.engine.state["stack"])

    def test_teferi_refuses_explicit_rebound_cast(self):
        self.setup_transpose(opponent={"battlefield": ["Teferi, Time Raveler"]})
        card, _ = self.original()
        self.upkeep()
        self.reject(0, {"kind": "cast", "card": card, "targets": [], "plan": []})
        self.assertEqual(self.engine.state["asked"]["kind"], "cast")
        self.choose("no")
        self.assertIn(card, self.engine.state["exile"])

    def test_countered_original_has_no_draw_token_exile_or_delayed_cast(self):
        self.setup_transpose(opponent={"hand": ["Counterspell"], "battlefield": ["Island"] * 2})
        library = self.engine.state["players"][0]["library"]
        self.cast("Transpose", mana=self.ids("Swamp", controller=0)[:3], spend={'B': 3})
        spell, = self.engine.state["stack"]
        self.engine.act({"kind": "pass"})
        self.cast("Counterspell", [[{"object": spell}]], mana=self.ids("Island", controller=1)[:2], spend={'U': 2})
        self.pass_round()
        self.assertEqual(self.engine.state["players"][0]["library"], library)
        self.assertEqual(self.engine.state["players"][0]["life"], 20)
        self.assertFalse(any(self.object(i)["token"] for i in self.engine.state["battlefield"]))
        self.assertFalse(self.engine.state["exile"])
        turn = self.engine.state["turn"]
        self.advance(lambda s: s["turn"] > turn and s["active"] == 0 and s["part"] == "Upkeep")
        self.assertFalse(self.engine.state["stack"])

    def test_copy_does_not_create_a_hand_token_or_its_own_rebound(self):
        self.new([{"hand": ["Transpose", "Reverberate", "Sol Ring", "Island"],
                   "battlefield": ["Swamp"] * 3 + ["Mountain"] * 2}, {}, {}])
        self.cast("Transpose", mana=self.ids("Swamp", controller=0)[:3], spend={'B': 3})
        spell, = self.engine.state["stack"]
        self.cast("Reverberate", [[{"object": spell}]], mana=self.ids("Mountain", controller=0)[:2], spend={'R': 2})
        self.pass_round()
        self.pass_round()
        self.discard("Island")
        self.assertEqual(self.engine.state["players"][0]["life"], 19)
        self.assertFalse(any(self.object(i)["token"] for i in self.engine.state["battlefield"]))
        self.assertFalse(self.engine.state["exile"])
        self.pass_round()
        self.discard("Sol Ring")
        self.assertEqual(self.engine.state["players"][0]["life"], 18)
        self.assertEqual(len([i for i in self.engine.state["battlefield"] if self.object(i)["token"]]), 1)
        self.assertEqual(len(self.engine.state["exile"]), 1)
        self.upkeep()
        self.choose("no")
