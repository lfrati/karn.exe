"""Fixed mana Echo through the managed Python/JSON client (702.30a)."""
import unittest
from test_grant_client import GrantClientTests as Helpers


class EchoClientTests(unittest.TestCase):
    setUpClass = classmethod(Helpers.setUpClass.__func__)
    setUp = Helpers.setUp
    close_engine = Helpers.close_engine
    new = Helpers.new
    view = Helpers.view
    hand = Helpers.hand
    ids = Helpers.ids
    cast = Helpers.cast
    pass_round = Helpers.pass_round
    reject = Helpers.reject

    def until(self, predicate):
        for _ in range(200):
            if predicate(self.engine.state):
                return
            kind = self.engine.state["asked"]["kind"]
            self.assertIn(kind, ("priority", "attackers"), self.engine.state)
            self.engine.act({"kind": "attack", "attacks": []} if kind == "attackers" else {"kind": "pass"})
        self.fail("did not reach the authored checkpoint")

    def jaguar(self, mana_choice=False):
        self.new([{"hand": ["Pouncing Jaguar"], "battlefield": ["Forest"] +
                   (["Empowered Autogenerator"] if mana_choice else [])}, {}])
        forest, = self.ids("Forest", zone="battlefield")
        self.cast("Pouncing Jaguar", mana=[forest], spend={"G": 1})
        self.pass_round()
        permanent, = self.ids("Pouncing Jaguar", zone="battlefield")
        self.until(lambda state: state["asked"]["kind"] == "pay")
        self.assertEqual(self.engine.state["asked"]["player"], 0)
        return permanent, forest

    def test_echo_asks_its_controller_and_explicit_payment_keeps_the_same_permanent(self):
        permanent, forest = self.jaguar()
        self.reject(1, {"kind": "choose", "answer": "no"})
        self.reject(0, {"kind": "pass"})
        self.engine.act({"kind": "choose", "answer": {"pay": {"plan": [{"source": forest, "ability": 0}],
                                                                  "mana": {"G": 1}}}})
        self.assertIn(permanent, self.engine.state["battlefield"])
        self.assertEqual(self.engine.state["players"][0]["pool"]["G"], 0)
        turn = self.engine.state["turn"]
        self.until(lambda state: state["turn"] > turn and state["active"] == 0 and state["part"] == "Upkeep")
        self.assertEqual(self.engine.state["asked"]["kind"], "priority")
        self.assertEqual(self.engine.state["stack"], [])
        self.assertIn(permanent, self.engine.state["battlefield"])

    def test_declining_echo_sacrifices_the_permanent(self):
        permanent, _ = self.jaguar()
        self.engine.act({"kind": "choose", "answer": "no"})
        self.assertNotIn(permanent, self.engine.state["battlefield"])
        graveyard = self.engine.state["players"][0]["graveyard"]
        self.assertEqual(len(graveyard), 1)
        self.assertEqual(self.engine.state["objects"][str(graveyard[0])]["name"], "Pouncing Jaguar")
        self.assertEqual(self.engine.state["asked"]["kind"], "priority")

    def test_echo_payment_retains_its_window_through_an_explicit_mana_choice_once(self):
        permanent, _ = self.jaguar(mana_choice=True)
        source, = self.ids("Empowered Autogenerator", zone="battlefield")
        self.engine.act({"kind": "choose", "answer": {"pay": {"plan": [{"source": source, "ability": 1}],
                                                                  "mana": {"G": 1}}}})
        self.assertEqual((self.engine.state["asked"]["kind"], self.engine.state["asked"]["player"]), ("mana", 0))
        self.assertEqual(self.engine.state["objects"][str(source)]["counters"], {"charge": 1})
        self.reject(1, {"kind": "choose", "answer": {"mana": ["G"]}})
        self.engine.act({"kind": "choose", "answer": {"mana": ["G"]}})
        self.assertEqual(self.engine.state["objects"][str(source)]["counters"], {"charge": 1})
        self.assertEqual(self.engine.state["players"][0]["pool"]["G"], 0)
        self.assertIn(permanent, self.engine.state["battlefield"])
        self.assertEqual(self.engine.state["asked"]["kind"], "priority")


del Helpers
