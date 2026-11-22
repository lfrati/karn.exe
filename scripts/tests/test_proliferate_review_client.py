"""Proliferate (701.34a) through the
real audited client with full pinned cards. The proliferating player chooses any set of
players and permanents that have a counter, without targeting; each chosen one gets one more
counter of each kind it has, all at once. A counter replacement is chosen by the affected
object's controller (616.1), and a spell whose only target is gone does nothing (608.2b)."""
import unittest
from client_support import ClientTestCase


class ProliferateReviewClientTests(ClientTestCase):
    def one(self, name):
        found = [o for o in self.ids(name) if o in self.engine.state["battlefield"]]
        self.assertEqual(len(found), 1, name)
        return found[0]

    def counters(self, holder):
        if "player" in holder:
            return self.engine.state["players"][holder["player"]].get("counters", {})
        return self.engine.state["objects"][str(holder["object"])].get("counters", {})

    def asked(self):
        return self.engine.state["asked"]

    def settle(self):
        """Pass until the stack is empty or something other than priority is asked."""
        for _ in range(40):
            if self.asked() is None or self.asked()["kind"] != "priority" or not self.engine.state["stack"]:
                return self.asked()
            self.engine.act({"kind": "pass"})
        self.fail("the stack did not settle")

    def answer(self, holders):
        self.engine.act({"kind": "choose", "answer": {"holders": holders}})

    def placed(self):
        return [(e["on"], e["counter"], e["count"]) for e in self.engine.state["events"] if e.get("event") == "counters"]

    def poisoned_bears(self):
        """P1 has hexproof (Leyline of Sanctity) and a poison counter; P1's Bears have a -1/-1
        counter from Grim Affliction, whose proliferate is now offered."""
        self.new([{"hand": ["Prologue to Phyresis", "Grim Affliction"], "battlefield": ["Island"] * 3 + ["Swamp"] * 3},
                  {"battlefield": ["Leyline of Sanctity", "Grizzly Bears"]}])
        self.cast("Prologue to Phyresis", mana=self.ids("Island")[:2], spend={'U': 2})
        self.settle()
        bears = self.one("Grizzly Bears")
        self.cast("Grim Affliction", [[{"object": bears}]], mana=self.ids("Swamp")[:3], spend={'B': 3})
        self.assertEqual(self.settle()["kind"], "holders")
        return {"object": bears}

    def test_mixed_offer_is_non_targeting_validated_and_canonical(self):
        bears = self.poisoned_bears()
        asked = self.asked()
        # players from the active player, then permanents; the hexproof player is offered
        self.assertEqual((asked["player"], asked["verb"], asked["among"]), (0, "Proliferate", [{"player": 1}, bears]))
        self.assertNotIn("among", self.view(1)["asked"])
        leyline = {"object": self.one("Leyline of Sanctity")}
        for wrong in [{"holders": [{"player": 0}]}, {"holders": [leyline]}, {"holders": [bears, bears]}, "yes", {"objects": [bears["object"]]}]:
            self.reject(0, {"kind": "choose", "answer": wrong})
        self.reject(1, {"kind": "choose", "answer": {"holders": []}})
        self.assertEqual((self.counters({"player": 1}), self.counters(bears)), ({"poison": 1}, {"-1/-1": 1}), "written before the answer")
        self.answer([bears, {"player": 1}])
        self.assertEqual(self.placed(), [({"player": 1}, "poison", 1), (bears, "-1/-1", 1)], "offer order, not answer order")
        self.assertEqual(self.counters({"player": 1}), {"poison": 2})
        self.assertNotIn(bears["object"], self.engine.state["battlefield"], "0/0 after the batch (704.5f)")

    def test_choosing_none_changes_nothing(self):
        bears = self.poisoned_bears()
        self.answer([])
        self.assertEqual(self.placed(), [])
        self.assertEqual((self.counters({"player": 1}), self.counters(bears)), ({"poison": 1}, {"-1/-1": 1}))
        self.assertEqual((self.asked()["kind"], self.engine.state["stack"]), ("priority", []))

    def test_no_holder_with_a_counter_asks_nothing_and_the_rest_resolves(self):
        self.new([{"hand": ["Steady Progress"], "battlefield": ["Island"] * 3}, {"battlefield": ["Grizzly Bears"]}])
        hand = len(self.view(0)["players"][0]["hand"])
        self.cast("Steady Progress", mana=self.ids("Island")[:3], spend={'U': 3})
        self.assertEqual(self.settle()["kind"], "priority")
        self.assertEqual(len(self.view(0)["players"][0]["hand"]), hand, "drew for Steady Progress")

    def replaced(self, scales_first):
        """P1's Bears get counters from P0's Courage in Crisis while P1 controls Hardened Scales
        and Corpsejack Menace: P1 orders them each time (616.1), never P0."""
        self.new([{"hand": ["Courage in Crisis"], "battlefield": ["Forest"] * 3},
                  {"battlefield": ["Hardened Scales", "Corpsejack Menace", "Grizzly Bears"]}])
        bears = {"object": self.one("Grizzly Bears")}
        scales, menace = self.one("Hardened Scales"), self.one("Corpsejack Menace")
        self.cast("Courage in Crisis", [[bears]], mana=self.ids("Forest", zone="battlefield")[:3], spend={'G': 3})
        committed = {}
        for step in range(2):
            asked = self.settle()
            if step == 1:
                self.assertEqual(asked["kind"], "holders")
                self.assertEqual(asked["among"], [bears])
                self.answer([bears])
                asked = self.asked()
            self.assertEqual((asked["kind"], asked["player"]), ("replacement", 1))
            self.assertEqual(asked["on"], bears)
            first = next(e for e in asked["among"] if e["source"] == (scales if scales_first else menace))
            self.assertEqual(self.counters(bears), committed, "written while its replacement is being chosen")
            self.reject(0, {"kind": "choose", "answer": {"replacement": first}})
            # the other replacement then applies by itself, and the placement is made
            self.engine.act({"kind": "choose", "answer": {"replacement": first}})
            self.settle()
            committed = self.counters(bears)
        return self.counters(bears)

    def test_affected_controller_orders_replacements_for_the_proliferated_counter(self):
        # Scales then Menace: (1+1)*2 = 4, twice. Menace then Scales: 1*2+1 = 3, twice.
        self.assertEqual(self.replaced(True), {"+1/+1": 8})

    def test_the_other_replacement_order_gives_its_own_count(self):
        self.assertEqual(self.replaced(False), {"+1/+1": 6})

    def test_sequential_proliferates_recompute_and_reach_saga_chapters(self):
        self.new([{"hand": ["History of Benalia", "Steady Progress", "Contentious Plan", "Volt Charge"],
                   "battlefield": ["Plains"] * 3 + ["Island"] * 6 + ["Mountain"] * 3},
                  {"battlefield": ["Jace Beleren"]}])
        self.cast("History of Benalia", mana=self.ids("Plains")[:3], spend={'W': 3})
        self.settle()
        saga = {"object": self.one("History of Benalia")}
        self.assertEqual((self.counters(saga), len(self.ids("Knight Token"))), ({"lore": 1}, 1))
        jace = {"object": self.one("Jace Beleren")}
        self.cast("Volt Charge", [[jace]], mana=self.ids("Mountain")[:3], spend={'R': 3})
        self.assertEqual(self.settle()["among"], [saga], "a planeswalker whose last loyalty was removed")
        self.answer([saga])
        self.settle()
        self.assertNotIn(jace["object"], self.engine.state["battlefield"])
        self.assertEqual((self.counters(saga), len([k for k in self.ids("Knight Token") if k in self.engine.state["battlefield"]])), ({"lore": 2}, 2), "chapter II (714.2b)")
        self.cast("Steady Progress", mana=self.ids("Island")[:3], spend={'U': 3})
        self.assertEqual(self.settle()["among"], [saga])
        self.answer([saga])
        self.settle()
        self.assertNotIn(saga["object"], self.engine.state["battlefield"], "sacrificed after chapter III (714.4)")
        self.cast("Contentious Plan", mana=self.ids("Island")[3:5], spend={'U': 2})
        self.assertEqual(self.settle()["kind"], "priority", "no holder with a counter is left")

    def test_poison_reaching_ten_by_proliferate_loses(self):
        self.new([{"hand": ["Caress of Phyrexia"] * 3 + ["Steady Progress"], "battlefield": ["Swamp"] * 15 + ["Island"] * 3},
                  {"library": ["Forest"] * 20}])
        for payment in range(3):
            self.cast("Caress of Phyrexia", [[{"player": 1}]], mana=self.ids("Swamp")[payment*5:payment*5+5], spend={'B': 5})
            self.settle()
        self.assertEqual(self.counters({"player": 1}), {"poison": 9})
        self.cast("Steady Progress", mana=self.ids("Island")[:3], spend={'U': 3})
        self.settle()
        self.answer([{"player": 1}])
        self.settle()
        state = self.engine.state
        self.assertEqual(state["players"][1].get("counters"), {"poison": 10})
        self.assertTrue(state["players"][1].get("lost") or state.get("winner") == 0, state["players"][1])

    def test_optional_cost_gates_the_proliferate(self):
        for pay in ["no", "yes"]:
            self.new([{"hand": ["Prologue to Phyresis", "Gitaxian Anatomist"], "battlefield": ["Island"] * 6}, {}])
            self.cast("Prologue to Phyresis", mana=self.ids("Island")[:2], spend={'U': 2})
            self.settle()
            self.cast("Gitaxian Anatomist", mana=self.ids("Island")[2:6], spend={'U': 4})
            asked = self.settle()
            self.assertEqual((asked["kind"], asked["player"]), ("may", 0))
            self.engine.act({"kind": "choose", "answer": pay})
            anatomist = self.one("Gitaxian Anatomist")
            if pay == "yes":
                self.assertEqual(self.asked()["kind"], "holders")
                self.answer([{"player": 1}])
            self.settle()
            self.assertEqual(self.asked()["kind"], "priority")
            self.assertEqual(self.counters({"player": 1}), {"poison": 2 if pay == "yes" else 1})
            self.assertEqual(bool(self.engine.state["objects"][str(anatomist)].get("tapped")), pay == "yes")

    def test_a_spell_whose_only_target_is_gone_does_not_proliferate(self):
        self.new([{"hand": ["Prologue to Phyresis", "Grim Affliction", "Lightning Bolt"], "battlefield": ["Island"] * 2 + ["Swamp"] * 3 + ["Mountain"]},
                  {"battlefield": ["Grizzly Bears"]}])
        self.cast("Prologue to Phyresis", mana=self.ids("Island")[:2], spend={'U': 2})
        self.settle()
        bears = self.one("Grizzly Bears")
        self.cast("Grim Affliction", [[{"object": bears}]], mana=self.ids("Swamp")[:3], spend={'B': 3})
        self.cast("Lightning Bolt", [[{"object": bears}]], mana=self.ids("Mountain")[:1], spend={'R': 1})
        self.assertEqual(self.settle()["kind"], "priority")
        self.assertEqual(self.counters({"player": 1}), {"poison": 1})

    def test_a_returned_creature_is_offered_as_its_new_object(self):
        self.new([{"hand": ["Vat Emergence"], "graveyard": ["Shrewd Hatchling"], "battlefield": ["Swamp"] * 5}, {}])
        card = self.ids("Shrewd Hatchling")[0]
        self.cast("Vat Emergence", [[{"object": card}]], mana=self.ids("Swamp")[:5], spend={'B': 5})
        asked = self.settle()
        hatchling = {"object": self.one("Shrewd Hatchling")}
        self.assertNotEqual(hatchling["object"], card)
        self.assertEqual((asked["kind"], asked["among"]), ("holders", [hatchling]))
        self.answer([hatchling])
        self.settle()
        self.assertEqual(self.counters(hatchling), {"-1/-1": 5})


if __name__ == "__main__":
    unittest.main()
