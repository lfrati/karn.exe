"""Suspend through the managed Python/JSON client (116.2f, 702.62a–d)."""
from client_support import ClientTestCase


class SuspendClientTests(ClientTestCase):
    def until(self, predicate):
        for _ in range(150):
            state = self.engine.state
            if predicate(state):
                return
            kind = state["asked"]["kind"]
            self.assertIn(kind, ("priority", "attackers"), state)
            self.engine.act({"kind": "attack", "attacks": []} if kind == "attackers" else {"kind": "pass"})
        self.fail("did not reach the authored checkpoint")

    def suspend_bolt(self):
        self.new([{"hand": ["Rift Bolt"], "battlefield": ["Mountain"]}, {}])
        card, = self.ids("Rift Bolt")
        mountain, = self.ids("Mountain", zone="battlefield")
        # Supplied resources do not imply supplied spending (601.2h).
        self.reject(0, {"kind": "suspend", "card": card, "plan": [{"source": mountain, "ability": 0}]})
        self.engine.act({"kind": "suspend", "card": card, "mana": {"R": 1},
                         "plan": [{"source": mountain, "ability": 0}]})
        exiled, = self.engine.state["exile"]
        self.assertNotEqual(card, exiled)
        return exiled

    def test_hand_action_is_public_in_exile_and_last_counter_cast_requires_the_owner_answer(self):
        card = self.suspend_bolt()
        self.assertEqual(self.engine.state["stack"], [])
        self.assertEqual(self.engine.state["asked"]["player"], 0)
        for seat in (0, 1):
            shown = self.view(seat)["objects"][str(card)]
            self.assertEqual(shown["name"], "Rift Bolt")
            self.assertEqual(shown["counters"], {"time": 1})
            self.assertTrue(shown["suspended"])
        self.until(lambda state: state["asked"]["kind"] == "cast")
        self.assertEqual(self.engine.state["asked"]["cards"], [card])
        self.assertNotIn("suspended", self.engine.state["objects"][str(card)])
        self.reject(1, {"kind": "choose", "answer": "no"})
        self.reject(0, {"kind": "pass"})
        self.engine.act({"kind": "cast", "card": card, "targets": [[{"player": 1}]]})
        self.pass_round()
        self.assertEqual(self.engine.state["players"][1]["life"], 17)
        self.assertEqual(self.engine.state["exile"], [])
        self.assertEqual(len(self.engine.state["players"][0]["graveyard"]), 1)

    def test_declining_the_cast_does_not_leave_a_later_permission_or_repeat_at_upkeep(self):
        card = self.suspend_bolt()
        self.until(lambda state: state["asked"]["kind"] == "cast")
        self.engine.act({"kind": "choose", "answer": "no"})
        turn = self.engine.state["turn"]
        self.until(lambda state: state["turn"] != turn and state["active"] == 0 and state["part"] == "Upkeep")
        self.assertEqual(self.engine.state["stack"], [])
        self.assertIn(card, self.engine.state["exile"])
        self.reject(0, {"kind": "cast", "card": card, "targets": [[{"player": 1}]]})
        self.assertEqual(self.engine.state["players"][1]["life"], 20)

    def test_greater_gargadon_explicit_sacrifices_remove_time_counters_and_free_creature_has_haste(self):
        self.new([{"hand": ["Greater Gargadon"], "battlefield": ["Mountain"] * 10}, {}])
        card, = self.ids("Greater Gargadon")
        mountains = self.ids("Mountain", zone="battlefield", controller=0)
        self.engine.act({"kind": "suspend", "card": card, "mana": {"R": 1},
                         "plan": [{"source": mountains[0], "ability": 0}]})
        card, = self.engine.state["exile"]
        for remaining, mountain in zip(range(9, -1, -1), mountains):
            self.engine.act({"kind": "activate", "source": card, "ability": 1, "targets": [], "paying": [[mountain]]})
            self.pass_round()
            self.assertEqual(self.engine.state["objects"][str(card)].get("counters", {}).get("time", 0), remaining)
        self.pass_round()
        self.assertEqual(self.engine.state["asked"]["cards"], [card])
        self.engine.act({"kind": "cast", "card": card, "targets": []})
        self.pass_round()
        permanent, = self.ids("Greater Gargadon", zone="battlefield")
        shown = self.engine.state["objects"][str(permanent)]
        self.assertEqual((shown["power"], shown["toughness"]), (9, 7))
        prompt = self.engine.send({"cmd": "prompt", "seat": "p0"})
        readable, = [item for item in prompt["objects"].values() if item.get("name") == "Greater Gargadon"]
        self.assertIn("Haste", readable["text"])
        self.assertEqual(len(self.engine.state["players"][0]["graveyard"]), 10)

    def test_doubling_season_does_not_double_time_counters_placed_on_an_exile_card(self):
        self.new([{"hand": ["Rift Bolt"], "battlefield": ["Mountain", "Doubling Season"]}, {}])
        card, = self.ids("Rift Bolt")
        mountain, = self.ids("Mountain", zone="battlefield")
        self.engine.act({"kind": "suspend", "card": card, "mana": {"R": 1},
                         "plan": [{"source": mountain, "ability": 0}]})
        card, = self.engine.state["exile"]
        self.assertEqual(self.engine.state["objects"][str(card)]["counters"], {"time": 1})

    def test_special_action_resumes_its_mana_choice_once_before_placing_time_counters(self):
        self.new([{"hand": ["Sol Talisman"], "battlefield": ["Empowered Autogenerator"]}, {}])
        card, = self.ids("Sol Talisman")
        source, = self.ids("Empowered Autogenerator")
        self.engine.act({"kind": "suspend", "card": card, "mana": {"W": 1},
                         "plan": [{"source": source, "ability": 1}]})
        self.assertEqual((self.engine.state["asked"]["kind"], self.engine.state["asked"]["player"]), ("mana", 0))
        self.assertEqual(self.engine.state["exile"], [])
        self.assertIn(card, self.hand(0))
        self.assertNotIn("Sol Talisman", str(self.view(1)))
        self.assertEqual(self.engine.state["objects"][str(source)]["counters"], {"charge": 1})
        self.reject(1, {"kind": "choose", "answer": {"mana": ["W"]}})
        self.engine.act({"kind": "choose", "answer": {"mana": ["W"]}})
        card, = self.engine.state["exile"]
        self.assertEqual(self.engine.state["objects"][str(card)]["counters"], {"time": 3})
        self.assertEqual(self.engine.state["objects"][str(source)]["counters"], {"charge": 1})
        pool = self.engine.state["players"][0]["pool"]
        self.assertEqual({color: pool[color] for color in "WUBRGC"}, dict.fromkeys("WUBRGC", 0))
        self.assertEqual(pool["restricted"], [])
        self.assertEqual(self.engine.state["asked"]["kind"], "priority")
