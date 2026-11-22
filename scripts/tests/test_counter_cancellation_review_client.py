"""Counter cancellation as a state-based action: 704.5q through the real
audited client with full pinned cards. A permanent left with both +1/+1 and -1/-1 counters
loses N of each at the next state-based check, so persist (Kitchen Finks) and undying (Young
Wolf) can return it again; P/T is unchanged by the cancellation itself."""
import unittest
# the module, not its class, so that its own tests are not collected here again
import test_proliferate_review_client as base
from client_support import ClientTestCase

HELPERS = {k: v for k, v in vars(base.ProliferateReviewClientTests).items() if not k.startswith("test_") and not k.startswith("__")}


class CounterCancellationReviewClientTests(ClientTestCase):
    locals().update(HELPERS)

    def cards(self, name):
        return [o for o in self.ids(name) if o in self.engine.state["battlefield"]]

    def pt(self, holder):
        o = self.engine.state["objects"][str(holder["object"])]
        return (o.get("power"), o.get("toughness"))

    def bolt(self, holder, mountain):
        self.cast("Lightning Bolt", [[holder]], mana=[mountain], spend={'R': 1})
        self.settle()

    def test_a_persisted_kitchen_finks_given_a_plus_counter_persists_again(self):
        self.new([{"hand": ["Lightning Bolt", "Courage in Crisis", "Lightning Bolt"], "battlefield": ["Kitchen Finks"] + ["Mountain"] * 6 + ["Forest"] * 3}, {}])
        self.bolt({"object": self.one("Kitchen Finks")}, self.cards("Mountain")[0])
        finks = {"object": self.one("Kitchen Finks")}
        self.assertEqual(self.counters(finks), {"-1/-1": 1}, "persist")
        self.cast("Courage in Crisis", [[finks]], mana=self.cards("Forest"), spend={'G': 3})
        self.assertEqual(self.settle()["among"], [finks])
        self.answer([])
        self.settle()
        self.assertEqual(self.counters(finks), {}, "704.5q")
        self.assertEqual(self.pt(finks), (3, 2))
        self.bolt(finks, self.cards("Mountain")[1])
        again = self.cards("Kitchen Finks")
        self.assertEqual(len(again), 1, "it had no -1/-1 counter, so it persists again")
        self.assertNotEqual(again[0], finks["object"])
        self.assertEqual(self.counters({"object": again[0]}), {"-1/-1": 1})

    def test_proliferating_both_kinds_cancels_and_undying_returns_again(self):
        self.new([{"hand": ["Lightning Bolt", "Grim Affliction", "Lightning Bolt"], "battlefield": ["Young Wolf"] + ["Mountain"] * 6 + ["Swamp"] * 3}, {}])
        self.bolt({"object": self.one("Young Wolf")}, self.cards("Mountain")[0])
        wolf = {"object": self.one("Young Wolf")}
        self.assertEqual(self.counters(wolf), {"+1/+1": 1}, "undying")
        self.cast("Grim Affliction", [[wolf]], mana=self.cards("Swamp"), spend={'B': 3})
        self.assertEqual(self.settle()["among"], [wolf])
        self.answer([wolf])
        self.settle()
        self.assertEqual(self.counters(wolf), {}, "two of each cancel")
        self.assertEqual(self.pt(wolf), (1, 1))
        self.bolt(wolf, self.cards("Mountain")[1])
        again = self.cards("Young Wolf")
        self.assertEqual(len(again), 1, "it had no +1/+1 counter, so undying returns it again")
        self.assertEqual(self.counters({"object": again[0]}), {"+1/+1": 1})


if __name__ == "__main__":
    unittest.main()
