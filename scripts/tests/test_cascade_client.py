"""Full-card Cascade through the managed Python/JSON client (702.85a)."""
from client_support import ClientTestCase


class CascadeClientTests(ClientTestCase):
    def bloodbraid(self, library):
        self.new([{"hand": ["Bloodbraid Elf"], "battlefield": ["Mountain"] * 2 + ["Forest"] * 2,
                   "library": library}, {}])
        self.cast("Bloodbraid Elf", mana=self.ids("Mountain") + self.ids("Forest", zone="battlefield"),
                  spend={"R": 2, "G": 2})
        self.assertEqual(len(self.engine.state["stack"]), 2)
        self.pass_round()
        self.assertEqual(self.engine.state["asked"]["kind"], "cast")
        hit, = self.engine.state["asked"]["cards"]
        return hit

    def test_scan_is_public_and_decline_returns_the_hit_without_a_later_offer(self):
        hit = self.bloodbraid(["Forest", "Runeclaw Bear", "Hill Giant", "Mountain"])
        for seat in (0, 1):
            shown = self.view(seat)
            self.assertCountEqual([shown["objects"][str(card)]["name"] for card in shown["exile"]],
                                  ["Runeclaw Bear", "Hill Giant", "Mountain"])
        self.reject(1, {"kind": "choose", "answer": "no"})
        self.reject(0, {"kind": "pass"})
        self.engine.act({"kind": "choose", "answer": "no"})
        self.assertEqual(self.engine.state["exile"], [])
        self.assertEqual(self.hand(0), [])
        self.assertEqual(self.engine.state["players"][0]["library"], 4)
        self.reject(0, {"kind": "cast", "card": hit, "targets": []})
        self.assertEqual(len(self.engine.state["stack"]), 1)

    def test_targeted_free_cast_requires_explicit_targets_then_resolves_above_the_original(self):
        hit = self.bloodbraid(["Forest", "Rift Bolt", "Mountain"])
        self.reject(0, {"kind": "cast", "card": hit, "targets": []})
        self.engine.act({"kind": "cast", "card": hit, "targets": [[{"player": 1}]]})
        self.assertEqual(self.engine.state["exile"], [])
        self.assertEqual(len(self.engine.state["stack"]), 2)
        self.pass_round()
        self.assertEqual(self.engine.state["players"][1]["life"], 17)
        self.assertEqual(len(self.engine.state["stack"]), 1)
        self.pass_round()
        elf, = self.ids("Bloodbraid Elf", zone="battlefield")
        self.assertEqual((self.engine.state["objects"][str(elf)]["power"],
                          self.engine.state["objects"][str(elf)]["toughness"]), (3, 2))
