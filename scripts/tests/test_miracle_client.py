"""Explicit draw-time Miracle choice and linked payment through native Python/JSON."""
import unittest
from test_grant_client import GrantClientTests as Helpers


class MiracleClientTests(unittest.TestCase):
    setUpClass = classmethod(Helpers.setUpClass.__func__)
    setUp = Helpers.setUp
    close_engine = Helpers.close_engine
    new = Helpers.new
    view = Helpers.view
    ids = Helpers.ids
    pass_round = Helpers.pass_round
    reject = Helpers.reject

    def first_draw(self):
        self.new([{"battlefield": ["Mountain"], "library": ["Mountain"] * 8 + ["Thunderous Wrath"]}, {}])
        for _ in range(150):
            asked = self.engine.state["asked"]
            if asked["kind"] == "miracle":
                self.assertEqual((asked["player"], asked["cost"]), (0, ["R"]))
                self.assertEqual(self.engine.state["turn"], 3)
                return asked["card"]
            self.assertIn(asked["kind"], ("priority", "attackers"))
            self.engine.act({"kind": "attack", "attacks": []} if asked["kind"] == "attackers" else {"kind": "pass"})
        self.fail("no first actual draw")

    def test_reveal_is_explicit_and_only_the_linked_price_casts_from_the_draw_offer(self):
        card = self.first_draw()
        hidden = self.view(1)
        self.assertNotIn(str(card), hidden["objects"])
        self.assertEqual(hidden["asked"]["kind"], "waiting")
        self.assertNotIn("cost", hidden["asked"])
        self.assertNotIn("card", hidden["asked"])
        self.reject(1, {"kind": "choose", "answer": "yes"})
        self.reject(0, {"kind": "pass"})
        self.engine.act({"kind": "choose", "answer": "yes"})
        self.assertIn(card, self.view(1)["publicly_revealed"])
        self.pass_round()
        self.assertEqual(self.engine.state["asked"]["kind"], "cast")
        self.assertEqual(self.engine.state["asked"]["cards"], [card])
        self.reject(0, {"kind": "cast", "card": card, "targets": [[{"player": 1}]]})
        mountain, = self.ids("Mountain", zone="battlefield")
        self.engine.act({"kind": "cast", "card": card, "cost": {"alternative": 1},
                         "targets": [[{"player": 1}]], "plan": [{"source": mountain, "ability": 0}], "mana": {"R": 1}})
        self.pass_round()
        self.assertEqual(self.engine.state["players"][1]["life"], 15)
        self.assertEqual(self.engine.state["players"][0]["pool"]["R"], 0)

    def test_declining_the_reveal_does_not_disclose_the_card_or_leave_a_trigger(self):
        card = self.first_draw()
        self.engine.act({"kind": "choose", "answer": "no"})
        self.assertEqual(self.engine.state["stack"], [])
        self.assertNotIn(str(card), self.view(1)["objects"])
        self.assertNotIn(card, self.view(1).get("publicly_revealed", []))

    def test_declining_the_linked_cast_ends_the_continuing_reveal(self):
        card = self.first_draw()
        self.engine.act({"kind": "choose", "answer": "yes"})
        self.pass_round()
        self.assertIn(card, self.view(1)["publicly_revealed"])
        self.engine.act({"kind": "choose", "answer": "no"})
        self.assertNotIn(str(card), self.view(1)["objects"])
        self.assertIn(card, self.engine.state["players"][0]["hand"])
