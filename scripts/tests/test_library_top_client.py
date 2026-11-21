"""Citadel regressions using the actual Python/JSON client."""
import unittest
from test_grant_client import GrantClientTests as Helpers


class LibraryTopClientTests(unittest.TestCase):
    setUpClass = classmethod(Helpers.setUpClass.__func__)
    setUp = Helpers.setUp
    close_engine = Helpers.close_engine
    new = Helpers.new
    view = Helpers.view
    hand = Helpers.hand
    ids = Helpers.ids
    object = Helpers.object
    pass_round = Helpers.pass_round
    choose = Helpers.choose
    reject = Helpers.reject
    cast = Helpers.cast
    advance = Helpers.advance
    step = Helpers.step

    def setup_top(self, name="Shock", battlefield=None, other=None, hand=None):
        self.new([{"library": ["Forest"] * 12 + ["Plains", name, "Forest"],
                   "battlefield": ["Bolas's Citadel"] + (battlefield or []), "hand": hand or []}, other or {}, {}])
        self.assertEqual(self.object(self.top())["name"], name)

    def top(self):
        return self.view(0)["players"][0]["top_card"]

    def private(self, card):
        for observer in (1, 2):
            state = self.view(observer)
            self.assertNotIn("top_card", state["players"][0])
            self.assertNotIn(str(card), state["objects"])

    def top_cast(self, card, **choices):
        return {"kind":"cast","card":card,"route":{"source":self.ids("Bolas's Citadel")[0],"ability":1},
                "targets":[],"plan":[],**choices}

    def test_continuous_private_top_explicit_route_and_land_limit(self):
        self.setup_top()
        card = self.top()
        self.private(card)
        self.reject(0, {"kind": "cast", "card": card, "route": "normal", "targets": [[{"player": 2}]], "plan": []})
        self.reject(1, {"kind": "cast", "card": card, "targets": [[{"player": 2}]], "plan": []})
        command = self.top_cast(card)
        command["targets"] = [[{"player": 2}]]
        self.engine.act(command)
        self.assertEqual(self.engine.state["players"][0]["life"], 19)
        next_top = self.top()
        self.assertEqual(self.object(next_top)["name"], "Plains")
        self.private(next_top)
        self.pass_round()
        self.assertEqual(self.engine.state["players"][2]["life"], 18)
        self.engine.act({"kind":"play","card":next_top})
        self.assertTrue(self.engine.state["players"][0]["land_played"])
        next_top = self.top()
        self.private(next_top)
        self.reject(0, {"kind": "play", "card": next_top})

    def test_modal_land_face_life_entry_and_following_top_remain_private(self):
        self.setup_top("Sink into Stupor")
        card = self.top()
        self.private(card)
        self.engine.act({"kind":"play","card":card,"face":1})
        self.assertEqual(self.engine.state["asked"]["kind"], "may")
        for observer in (1, 2):
            self.assertNotIn("top_card", self.view(observer)["players"][0])
        self.choose("yes")
        self.assertEqual(self.engine.state["players"][0]["life"], 17)
        land, = self.ids("Soporific Springs")
        self.assertFalse(self.object(land)["tapped"])
        self.assertTrue(self.engine.state["players"][0]["land_played"])
        self.private(self.top())

    def test_life_affordability_trinisphere_and_rejected_alternative(self):
        self.setup_top("Sol Ring", battlefield=["Trinisphere", "Forest", "Forest", "Forest"])
        card = self.top()
        plan=[{"source":i,"ability":0} for i in self.ids("Forest") if i in self.engine.state["battlefield"]]
        self.reject(0, self.top_cast(card))
        self.reject(0,self.top_cast(card,plan=plan,mana={"G": 3},cost={"alternative":0}))
        self.engine.act(self.top_cast(card,plan=plan,mana={"G": 3}))
        self.assertEqual(self.engine.state["players"][0]["life"], 19)
        self.assertEqual(self.engine.state["players"][0]["pool"]["C"], 0)
        self.private(self.top())
        self.engine.ask({"cmd": "new", "life": 2, "players": [{"library": ["Forest"] * 8 + ["Sol Ring", "Forest"], "battlefield": ["Bolas's Citadel"]}, {}, {}]})
        self.advance(lambda s: s["part"] == "PrecombatMain")
        card = self.top()
        self.engine.act(self.top_cast(card))
        self.pass_round()
        # Next permanent has mana value six; life total one cannot pay it.
        self.engine.ask({"cmd": "new", "life": 2, "players": [{"library": ["Forest"] * 8 + ["Bolas's Citadel", "Forest"], "battlefield": ["Bolas's Citadel"]}, {}, {}]})
        self.advance(lambda s: s["part"] == "PrecombatMain")
        card = self.top()
        self.reject(0, self.top_cast(card))

    def test_full_ten_nonland_sacrifices_including_citadel_multiplayer_and_atomic_rejection(self):
        self.setup_top(battlefield=["Sol Ring"] * 9 + ["Forest"])
        source = self.ids("Bolas's Citadel")[0]
        chosen = [source] + self.ids("Sol Ring")
        for bad in (chosen[:9], [source] * 10, chosen[:9] + [self.ids("Forest")[0]]):
            self.reject(0, {"kind": "activate", "source": source, "ability": 2, "targets": [], "paying": [bad], "plan": []})
        self.engine.act({"kind": "activate", "source": source, "ability": 2, "targets": [], "paying": [chosen], "plan": []})
        self.assertEqual(self.engine.state["asked"]["kind"],"order_cards")
        self.engine.act({"kind":"choose","answer":{"objects":chosen}})
        self.assertFalse(self.engine.state["battlefield"] and any(i in self.engine.state["battlefield"] for i in chosen))
        self.assertNotIn("top_card", self.view(0)["players"][0])
        self.pass_round()
        self.assertEqual([p["life"] for p in self.engine.state["players"]], [20, 10, 10])

    def test_suspended_payment_hides_next_top_completion_and_reversal_preserve_privacy(self):
        for reverse in (False, True):
            self.setup_top(battlefield=["Trinisphere", "Empowered Autogenerator", "Forest", "Forest"])
            card = self.top()
            auto, = self.ids("Empowered Autogenerator")
            step = {"source": auto, "ability": 1, "paying": [], "adding": []}
            self.engine.act(self.top_cast(card,targets=[[{"player":2}]],plan=[step,step]))
            self.assertEqual(self.engine.state["asked"]["kind"], "mana")
            self.assertNotIn("top_card", self.view(0)["players"][0])
            for viewer in (0, 1, 2):
                self.assertFalse(any(o["name"] == "Plains" for o in self.view(viewer)["objects"].values()))
            self.choose({"mana": ["U"]})
            self.assertEqual(self.engine.state["asked"]["kind"], "payment")
            self.assertNotIn("top_card", self.view(0)["players"][0])
            if reverse:
                self.reject(1, {"kind": "reverse_payment"})
                self.engine.act({"kind": "reverse_payment"})
                self.assertEqual(self.top(), card)
                self.private(card)
                self.assertFalse(self.engine.state["stack"])
                self.assertEqual(self.engine.state["players"][0]["life"], 20)
                self.assertTrue(self.object(auto)["tapped"])
                self.assertEqual(self.object(auto)["counters"]["charge"], 1)
            else:
                plan = [{"source": i, "ability": 0, "paying": [], "adding": []}
                        for i in self.ids("Forest") if i in self.engine.state["battlefield"]]
                self.engine.act({"kind": "payment", "plan": plan, "paying": [], "mana": {"U": 1, "G": 2}})
                self.assertEqual(self.engine.state["players"][0]["life"], 19)
                self.assertEqual(self.object(self.top())["name"], "Plains")
                self.private(self.top())
                self.pass_round()
                self.assertEqual(self.engine.state["players"][2]["life"], 18)

    def test_modal_spell_face_explicit_commands_pay_selected_face_life(self):
        self.setup_top("Sink into Stupor", other={"battlefield": ["Sol Ring"]})
        card = self.top()
        target, = self.ids("Sol Ring")
        self.reject(0,self.top_cast(card,cast_as={"face":1},targets=[[{"object":target}]]))
        command = self.top_cast(card,cast_as={"face":0})
        command["targets"] = [[{"object": target}]]
        self.engine.act(command)
        self.assertEqual(self.engine.state["players"][0]["life"], 17)
        self.private(self.top())
        self.pass_round()
        self.assertTrue(any(self.view(1)["objects"][str(i)]["name"] == "Sol Ring" for i in self.hand(1)))

    def test_normal_timing_and_teferi_restriction_refuse_library_cast(self):
        self.setup_top(other={"battlefield": ["Teferi, Time Raveler"]})
        turn = self.engine.state["turn"]
        self.advance(lambda s: s["turn"] > turn and s["active"] == 0 and s["part"] == "Upkeep")
        card = self.top()
        self.assertEqual(self.object(card)["name"], "Shock")
        self.reject(0,self.top_cast(card,targets=[[{"player":2}]]))
        self.private(card)


del Helpers
if __name__ == "__main__":
    unittest.main()
