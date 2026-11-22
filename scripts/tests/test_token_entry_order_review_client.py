"""A predefined Treasure token's entry can genuinely suspend on
a 616.1 order question with full admitted cards, so token-alternative creation has a real
child-suspension witness. Root Maze ("Artifacts and lands enter tapped") and Orb of Dreams
("Permanents enter tapped") both apply to the entering Treasure; its controller chooses which
applies first, then it enters tapped. Kismet alone applies once and asks nothing.
"""
import unittest
import test_toxic_review_client as base
from client_support import ClientTestCase

HELPERS = {k: v for k, v in vars(base.ToxicReviewClientTests).items() if not k.startswith("test_") and not k.startswith("__")}


class TokenEntryOrderReviewClientTests(ClientTestCase):
    locals().update(HELPERS)

    def strike_it_rich(self, mine, theirs=()):
        self.new([{"hand": ["Strike It Rich"], "battlefield": ["Mountain", *mine], "library": ["Mountain"] * 8},
                  {"battlefield": list(theirs), "library": ["Forest"] * 8}])
        self.main()
        self.cast("Strike It Rich", mana=self.ids("Mountain", zone="battlefield", controller=0)[:1], spend={'R': 1})
        for _ in range(6):
            a = self.asked()
            if a["kind"] != "priority" or not self.engine.state["stack"]:
                return a
            self.engine.act({"kind": "pass"})
        self.fail("the stack did not settle")

    def treasures(self):
        return [self.obj(o) for o in self.engine.state["battlefield"] if self.obj(o)["name"] == "Treasure Token"]

    def test_two_entry_effects_ask_their_order_then_the_treasure_enters_tapped(self):
        a = self.strike_it_rich(["Root Maze", "Orb of Dreams"])
        self.assertEqual((a["kind"], a.get("event"), a["player"]), ("replacement", "entry", 0), a)
        self.assertEqual(sorted(e["name"] for e in a["among"]), ["Orb of Dreams", "Root Maze"])
        self.assertEqual(self.treasures(), [], "nothing has entered while the order is asked")
        effect = {k: v for k, v in a["among"][0].items() if k != "name"}
        self.engine.act({"kind": "choose", "answer": {"replacement": effect}})
        self.assertEqual([t["tapped"] for t in self.treasures()], [True])

    def test_one_entry_effect_applies_without_a_question(self):
        a = self.strike_it_rich([], theirs=["Kismet"])
        self.assertEqual(a["kind"], "priority", a)
        self.assertEqual([t["tapped"] for t in self.treasures()], [True])


if __name__ == "__main__":
    unittest.main()
