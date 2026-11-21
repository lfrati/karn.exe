"""A loyalty ability that
adds mana is not a mana ability (605.1a), so it uses the stack and players may respond before
the mana is added (606, 405). Full pinned Chandra, Novice Pyromancer through the real client."""
import unittest
import test_toxic_review_client as base

HELPERS = {k: v for k, v in vars(base.ToxicReviewClientTests).items() if not k.startswith("test_") and not k.startswith("__")}


class LoyaltyManaReviewClientTests(unittest.TestCase):
    locals().update(HELPERS)

    def test_loyalty_mana_ability_uses_the_stack(self):
        # 605.1a: a loyalty ability is never a mana ability, so P1 may respond before {R}{R} is added
        self.new([{"battlefield": ["Chandra, Novice Pyromancer"]}, {}])
        self.main()
        chandra = self.one("Chandra, Novice Pyromancer")
        self.engine.act({"kind": "activate", "source": chandra, "ability": 1, "targets": [], "plan": [], "paying": []})
        self.assertEqual(self.obj(chandra).get("counters", {}).get("loyalty"), 4, "the cost is paid on activation")
        self.assertEqual(len(self.engine.state["stack"]), 1)
        self.assertEqual(self.engine.state["players"][0]["pool"].get("R", 0), 0)
        self.engine.act({"kind": "pass"})
        self.assertEqual(self.asked()["player"], 1, "the opponent gets priority")
        self.engine.act({"kind": "pass"})
        self.assertEqual((self.engine.state["stack"], self.engine.state["players"][0]["pool"].get("R", 0)), ([], 2))


if __name__ == "__main__":
    unittest.main()
