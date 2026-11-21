"""Attack-payment regressions through the actual Python/JSON engine client."""
import subprocess
import os
import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import play


class AttackPaymentClientTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        if os.environ.get("GAME_SERVE_REVIEW"):
            return
        subprocess.run(["cargo", "build", "--profile", "fast", "-p", "game", "--bin", "game-serve"],
                       cwd=play.ROOT, check=True, stdout=subprocess.DEVNULL)

    def setUp(self):
        self.engine = play.Engine(audit=True)
        self.addCleanup(self.close_engine)

    def close_engine(self):
        self.engine.proc.stdin.close()
        self.engine.proc.wait(timeout=10)
        self.engine.proc.stdout.close()

    def new(self, seats):
        self.engine.ask({"cmd": "new", "players": [dict(library=["Forest"] * 5, **seat) for seat in seats]})
        for _ in range(30):
            if self.engine.state["asked"]["kind"] == "attackers":
                return
            self.engine.act({"kind": "pass"})
        self.fail("fixture did not reach attacker declaration")

    def ids(self, name):
        return [int(key) for key, obj in self.engine.state["objects"].items() if obj["name"] == name and int(key) in self.engine.state["battlefield"]]

    def cost(self, asked):
        return sum(int(symbol) for symbol in asked["cost"])

    def attack(self, attacks):
        self.engine.act({"kind": "attack", "attacks": attacks})

    def reject(self, player, move):
        state = self.engine.state
        with self.assertRaises(SystemExit):
            self.engine.send({"cmd": "act", "player": player, "move": move, "viewer": "asked",
                              "session_id": state["session_id"], "request_id": state["request_id"]})
        after = self.engine.send({"cmd": "state", "viewer": state["viewer"]})
        self.assertEqual(after["request_id"], state["request_id"])
        self.assertEqual(after["attacking"], state["attacking"])

    def test_full_cards_offer_player_tax_and_untaxed_planeswalker_and_accept_both(self):
        for tax in ["Ghostly Prison", "Propaganda"]:
            with self.subTest(tax=tax):
                self.new([{"battlefield": ["Grizzly Bears"] + ["Forest"] * 2},
                          {"battlefield": [tax, "Jace Beleren"]}])
                bear, = self.ids("Grizzly Bears")
                walker, = self.ids("Jace Beleren")
                offer, = self.engine.state["asked"]["able"]
                self.assertEqual(offer["attacks"], [{"player": 1}, {"object": walker}])
                self.assertEqual(offer["costs"], [{"target": {"player": 1}, "cost": ["2"]},
                                                 {"target": {"object": walker}, "cost": []}])
                self.attack([[bear, {"player": 1}]])
                self.assertEqual(self.engine.state["asked"]["kind"], "pay")
                self.assertEqual(self.cost(self.engine.state["asked"]), 2)
                self.assertEqual(self.engine.state["attacking"], [])
                self.engine.act({"kind": "choose", "answer": "no"})
                self.assertFalse(self.engine.state["objects"][str(bear)]["tapped"])
                self.attack([[bear, {"object": walker}]])
                self.assertEqual(self.engine.state["attacking"], [bear])
                self.assertEqual(self.engine.state["asked"]["kind"], "priority")
                self.assertFalse(any(self.engine.state["objects"][str(i)]["tapped"] for i in self.ids("Forest")))
                # A separate declaration pays the player's cost using the two fixture Forests.
                self.new([{"battlefield": ["Grizzly Bears"] + ["Forest"] * 2},
                          {"battlefield": [tax]}])
                bear, = self.ids("Grizzly Bears")
                self.attack([[bear, {"player": 1}]])
                self.engine.act({"kind": "choose", "answer": {"pay": {"mana": {"G": 2}, "plan": [
                    {"source": land, "ability": 0} for land in self.ids("Forest")]}}})
                self.assertEqual(self.engine.state["attacking"], [bear])
                self.assertEqual(sum(self.engine.state["objects"][str(i)]["tapped"] for i in self.ids("Forest")), 2)

    def test_multiplayer_cumulative_cost_is_paid_only_by_the_attacker(self):
        self.new([{"battlefield": ["Grizzly Bears"] * 3 + ["Forest"] * 10},
                  {"battlefield": ["Ghostly Prison", "Propaganda"]},
                  {"battlefield": ["Propaganda"]}])
        a, b, c = self.ids("Grizzly Bears")
        self.attack([[a, {"player": 1}], [b, {"player": 1}], [c, {"player": 2}]])
        asked = self.engine.state["asked"]
        self.assertEqual((asked["kind"], asked["player"], self.cost(asked)), ("pay", 0, 10))
        self.reject(1, {"kind": "choose", "answer": "yes"})
        self.reject(0, {"kind": "pass"})
        self.engine.act({"kind": "choose", "answer": {"pay": {"mana": {"G": 10}, "plan": [{"source": land, "ability": 0} for land in self.ids("Forest")]}}})
        self.assertEqual(self.engine.state["attacking"], [a, b, c])
        self.assertEqual(sum(self.engine.state["objects"][str(i)]["tapped"] for i in self.ids("Forest")), 10)

    def test_insufficient_cumulative_payment_is_atomic_and_decline_reopens_declaration(self):
        self.new([{"battlefield": ["Grizzly Bears"] * 2 + ["Forest"] * 2},
                  {"battlefield": ["Ghostly Prison"]}])
        a, b = self.ids("Grizzly Bears")
        self.attack([[a, {"player": 1}], [b, {"player": 1}]])
        self.assertEqual(self.cost(self.engine.state["asked"]), 4)
        lands = self.ids("Forest")
        payment = {"kind": "choose", "answer": {"pay": {"mana": {"G": 2}, "plan": [
            {"source": i, "ability": 0, "paying": [], "adding": []} for i in lands]}}}
        self.reject(0, payment)
        self.assertFalse(any(self.engine.state["objects"][str(i)]["tapped"] for i in lands))
        self.engine.act({"kind": "choose", "answer": "no"})
        self.assertEqual(self.engine.state["asked"]["kind"], "attackers")
        self.assertFalse(any(self.engine.state["objects"][str(i)]["tapped"] for i in [a, b]))
        self.attack([[a, {"player": 1}]])
        self.engine.act({"kind": "choose", "answer": {"pay": {"mana": {"G": 2}, "plan": [{"source": land, "ability": 0} for land in self.ids("Forest")]}}})
        self.assertEqual(self.engine.state["attacking"], [a])

    def test_required_attack_does_not_compel_payment(self):
        self.new([{"battlefield": ["Juggernaut"]}, {"battlefield": ["Ghostly Prison"]}])
        offer, = self.engine.state["asked"]["able"]
        self.assertFalse(offer["must"])
        self.attack([])
        self.assertEqual(self.engine.state["attacking"], [])
        self.assertEqual(self.engine.state["asked"]["kind"], "priority")


if __name__ == "__main__":
    unittest.main()
