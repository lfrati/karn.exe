"""Composition/retention checks using the actual Python/JSON engine client."""
import unittest

from client_support import ClientTestCase


class MultiplayerOutcomeClientTests(ClientTestCase):
    def rhystic(self, mana):
        self.new([{"battlefield": ["Rhystic Study"]},
                  {"battlefield": ["Mountain"] + (["Forest"] if mana else []), "hand": ["Shock"]}, {}])
        self.engine.act({"kind": "pass"})
        self.cast("Shock", [[{"player": 2}]], mana=self.ids("Mountain")[:1], spend={'R': 1})
        self.pass_round()

    def test_rhystic_payer_decides_before_optional_draw_and_can_prevent_it(self):
        self.rhystic(True)
        before = len(self.hand(0))
        self.assertEqual((self.engine.state["asked"]["kind"], self.engine.state["asked"]["player"]), ("pay", 1))
        self.reject(0, {"kind": "choose", "answer": "no"})
        self.choose({"pay": {"plan": [{"source": self.ids("Forest")[0], "ability": 0}], "mana": {"G": 1}}})
        self.assertEqual(len(self.hand(0)), before)
        self.assertEqual(self.engine.state["asked"]["kind"], "priority")
        self.assertEqual(len(self.engine.state["stack"]), 1)
        self.assertEqual(self.engine.state["players"][2]["life"], 20)

    def test_rhystic_nested_mana_choice_resumes_payment_before_draw_decision(self):
        self.new([{"battlefield": ["Rhystic Study"]},
                  {"battlefield": ["Mountain", "Empowered Autogenerator"], "hand": ["Shock"]}, {}])
        for _ in range(100):
            if self.engine.state["active"] == 1 and self.engine.state["part"] == "PrecombatMain":
                break
            self.engine.act({"kind": "attack", "attacks": []} if self.engine.state["asked"]["kind"] == "attackers" else {"kind": "pass"})
        self.assertEqual(self.engine.state["active"], 1)
        self.cast("Shock", [[{"player": 2}]], mana=self.ids("Mountain")[:1], spend={'R': 1})
        self.pass_round()
        self.assertEqual(self.engine.state["asked"]["kind"], "pay")
        source, = self.ids("Empowered Autogenerator")
        self.choose({"pay": {"plan": [{"source": source, "ability": 1, "paying": [], "adding": []}], "mana": {"W": 1}}})
        asked = self.engine.state["asked"]
        self.assertEqual((asked["kind"], asked["player"]), ("mana", 1))
        self.assertEqual(self.engine.state["objects"][str(source)]["counters"]["charge"], 1)
        self.reject(0, {"kind": "choose", "answer": {"mana": asked["options"][0]}})
        self.choose({"mana": ["W"]})
        self.assertEqual(self.engine.state["asked"]["kind"], "priority")
        self.assertEqual(self.engine.state["objects"][str(source)]["counters"]["charge"], 1)
        self.assertEqual(len(self.engine.state["stack"]), 1)

    def test_rhystic_declined_or_impossible_payment_preserves_controllers_draw_choice(self):
        for mana in [True, False]:
            for draw in [True, False]:
                with self.subTest(mana=mana, draw=draw):
                    self.rhystic(mana)
                    before = len(self.hand(0))
                    self.choose("no")
                    self.assertEqual((self.engine.state["asked"]["kind"], self.engine.state["asked"]["player"]), ("may", 0))
                    self.reject(1, {"kind": "choose", "answer": "yes"})
                    self.choose("yes" if draw else "no")
                    self.assertEqual(len(self.hand(0)), before + draw)
                    self.assertEqual(len(self.engine.state["stack"]), 1)

    def test_fandaniel_collects_public_sacrifices_then_applies_each_failure_separately(self):
        self.new([{"battlefield": ["Fandaniel, Telophoroi Ascian"], "graveyard": ["Shock", "Opt"]},
                  {"battlefield": ["Grizzly Bears", "Hill Giant"]},
                  {"battlefield": ["Grizzly Bears"]}, {}])
        for _ in range(70):
            if self.engine.state["asked"]["kind"] == "may":
                break
            self.engine.act({"kind": "attack", "attacks": []} if self.engine.state["asked"]["kind"] == "attackers" else {"kind": "pass"})
        self.assertEqual(self.engine.state["asked"]["player"], 1)
        self.reject(0, {"kind": "choose", "answer": "yes"})
        self.choose("yes")
        asked = self.engine.state["asked"]
        self.assertEqual((asked["kind"], asked["player"], asked["count"]), ("objects", 1, 1))
        wanted = asked["among"][0]
        self.reject(1, {"kind": "choose", "answer": "yes"})
        self.reject(2, {"kind": "choose", "answer": {"objects": [wanted]}})
        self.choose({"objects": [wanted]})
        self.assertEqual((self.engine.state["asked"]["kind"], self.engine.state["asked"]["player"]), ("may", 2))
        self.assertIn(wanted, self.engine.state["battlefield"])
        observer = self.view(3)
        self.assertTrue(any(c.get("objects") == [wanted] for c in observer["choices_made"]))
        self.choose("no")
        self.assertNotIn(wanted, self.engine.state["battlefield"])
        self.assertEqual([p["life"] for p in self.engine.state["players"]], [20, 20, 16, 16])
        sacrifices = [e for e in self.engine.state["events"] if e["event"] == "sacrificed"]
        self.assertEqual(len(sacrifices), 1)

    def test_fandaniel_replaced_sacrifice_still_pays_the_individual_cost(self):
        self.new([{"battlefield": ["Fandaniel, Telophoroi Ascian"], "graveyard": ["Shock"]},
                  {"battlefield": ["Nissa's Chosen"]}, {"battlefield": ["Grizzly Bears"]}])
        for _ in range(60):
            if self.engine.state["asked"]["kind"] == "may":
                break
            self.engine.act({"kind": "attack", "attacks": []} if self.engine.state["asked"]["kind"] == "attackers" else {"kind": "pass"})
        chosen, = self.ids("Nissa's Chosen")
        self.choose("yes")
        self.assertEqual(self.engine.state["asked"]["player"], 2)
        self.assertIn(chosen, self.engine.state["battlefield"])
        library = self.engine.state["players"][1]["library"]
        self.choose("no")
        self.assertNotIn(chosen, self.engine.state["battlefield"])
        self.assertEqual(self.engine.state["players"][1]["library"], library + 1)
        self.assertFalse(self.engine.state["players"][1]["graveyard"])
        self.assertEqual([p["life"] for p in self.engine.state["players"]], [20, 20, 18])

    def test_fandaniel_surveil_is_private_and_precedes_the_cast_spell(self):
        self.new([{"battlefield": ["Fandaniel, Telophoroi Ascian", "Island"], "hand": ["Opt"]}, {}, {}])
        self.cast("Opt", mana=self.ids("Island")[:1], spend={'U': 1})
        self.pass_round()
        asked = self.engine.state["asked"]
        self.assertEqual((asked["kind"], asked["verb"], asked["player"]), ("arrange", "Surveil", 0))
        top, = asked["cards"]
        for observer in [1, 2]:
            view = self.view(observer)
            self.assertNotIn("cards", view["asked"])
            self.assertNotIn(str(top), view["objects"])
        before = len(self.hand(0))
        self.choose({"top": [], "away": [top]})
        self.assertEqual(len(self.hand(0)), before)
        self.assertEqual(len(self.engine.state["stack"]), 1)
        grave, = self.engine.state["players"][0]["graveyard"]
        for observer in [1, 2]:
            self.assertEqual(self.view(observer)["objects"][str(grave)]["name"], "Forest")

    def test_syphon_keeps_pending_discards_private_then_counts_only_nonempty_hands(self):
        self.new([{"battlefield": ["Swamp"] * 4, "hand": ["Syphon Mind"]},
                  {"hand": ["Grizzly Bears", "Hill Giant"]},
                  {"hand": ["Forest", "Island"]}, {}])
        before = len(self.hand(0)) - 1
        self.cast("Syphon Mind", mana=self.ids("Swamp")[:4], spend={'B': 4})
        self.pass_round()
        first = self.engine.state["asked"]["among"][0]
        self.choose({"objects": [first]})
        asked = self.engine.state["asked"]
        self.assertEqual((asked["kind"], asked["player"]), ("objects", 2))
        self.assertFalse(self.engine.state["players"][1]["graveyard"])
        for observer in [0, 2, 3]:
            view = self.view(observer)
            self.assertNotIn(str(first), view["objects"])
            self.assertFalse(any(first in c.get("objects", []) for c in view["choices_made"]))
        self.reject(1, {"kind": "choose", "answer": {"objects": [asked["among"][0]]}})
        second = asked["among"][0]
        self.choose({"objects": [second]})
        self.assertEqual(len(self.hand(0)), before + 2)
        for observer in [0, 1, 2, 3]:
            view = self.view(observer)
            self.assertEqual([len(p["graveyard"]) for p in view["players"]], [1, 1, 1, 0])
        discards = [e for e in self.engine.state["events"] if e["event"] == "discarded"]
        self.assertEqual(len(discards), 2)

    def test_syphon_counts_a_discard_replaced_by_entry_not_graveyard_growth(self):
        self.new([{"battlefield": ["Swamp"] * 4, "hand": ["Syphon Mind"]},
                  {"hand": ["Obstinate Baloth"]}, {}])
        before = len(self.hand(0)) - 1
        self.cast("Syphon Mind", mana=self.ids("Swamp")[:4], spend={'B': 4})
        self.pass_round()
        self.assertEqual(len(self.hand(0)), before + 1)
        self.assertFalse(self.engine.state["players"][1]["graveyard"])
        self.assertTrue(any(self.engine.state["objects"][str(i)]["name"] == "Obstinate Baloth" for i in self.engine.state["battlefield"]))
        self.assertEqual(self.engine.state["players"][1]["life"], 20, "entry trigger waits until Syphon finishes")


if __name__ == "__main__":
    unittest.main()
