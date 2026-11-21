"""Exchanges through player/1; no legal-move listing or numeric object access."""
import unittest

import test_player_prompt_client as fixtures
import play


class PlayerPromptReviewTests(unittest.TestCase):
    setUpClass = classmethod(fixtures.PlayerPromptClientTests.setUpClass.__func__)
    setUp = fixtures.PlayerPromptClientTests.setUp
    close_engine = fixtures.PlayerPromptClientTests.close_engine
    new = fixtures.PlayerPromptClientTests.new
    waiting = fixtures.PlayerPromptClientTests.waiting
    pass_until_asked = fixtures.PlayerPromptClientTests.pass_until_asked
    named = staticmethod(fixtures.PlayerPromptClientTests.named)
    refused = fixtures.PlayerPromptClientTests.refused

    def abrade(self):
        me, them = self.new({"hand": ["Abrade"], "battlefield": ["Mountain", "Mountain"]},
                            {"battlefield": ["Myr Retriever"]})
        prompt = me.prompt()
        return me, them, prompt, self.named(prompt, "Abrade")[0]

    def test_modal_card_text_includes_the_effect_of_every_mode(self):
        # 700.2a: the caster chooses a mode from these two printed effects.
        _, _, prompt, abrade = self.abrade()
        text = "\n".join(prompt["objects"][abrade]["text"])
        self.assertIn("3 damage to target creature", text)
        self.assertIn("Destroy target artifact", text)

    def test_a_modal_spell_on_the_stack_says_which_mode_was_chosen(self):
        # 700.2a / 601.2b: modes are announced and public. Both Abrade modes can
        # target this artifact creature, so its target does not identify the mode.
        me, them, prompt, abrade = self.abrade()
        me.answer({"kind": "cast", "card": abrade, "mana": {"R": 2}, "modes": [1],
                   "targets": [self.named(prompt, "Myr Retriever")],
                   "plan": [{"ability": f"{m}.a0"} for m in self.named(prompt, "Mountain")]})
        other = them.prompt()
        spell = other["objects"][other["stack"][0]]
        modes = spell["occurrence"].get("modes")
        self.assertEqual([m["index"] for m in modes or []], [1])
        self.assertIn("Destroy target artifact", modes[0]["text"])

    def test_turn_control_shares_private_choices_but_uses_the_actors_resources(self):
        me, actor, third = self.new(
            {"hand": ["Shock", "Emrakul, the Promised End"], "battlefield": ["Wastes"] * 13},
            {"hand": ["Ponder"], "battlefield": ["Island"],
             "library": ["Lightning Bolt", "Grizzly Bears", "Shock", "Island", "Island"]},
            {"hand": ["Demonic Tutor"]})
        prompt = me.prompt()
        emrakul = self.named(prompt, "Emrakul, the Promised End")[0]
        me.answer({"kind": "cast", "card": emrakul, "mana": {"C": 13},
                   "plan": [{"ability": f"{w}.a0"} for w in self.named(prompt, "Wastes")]})
        self.assertEqual(me.last["decision"]["kind"], "targets")
        me.answer({"kind": "targets", "targets": [["p1"]]})
        for _ in range(120):
            seat = self.waiting()
            p = seat.last
            if p["active"] == "p1" and p["part"] == "PrecombatMain" and p["decision"]["kind"] == "priority":
                break
            q = p["decision"]
            if q["kind"] == "attackers":
                seat.answer({"kind": "attack", "attacks": []})
            else:
                self.assertEqual(q["kind"], "priority")
                seat.answer({"kind": "pass"})
        else:
            self.fail("controlled turn did not start")
        self.assertIs(seat, me)
        self.assertEqual((p["decision"]["chooser"], p["decision"]["acting"]), ("p0", "p1"))
        self.assertTrue(self.named(p, "Ponder", controller="p1"))
        self.assertTrue(self.named(p, "Shock", controller="p0"))
        own = actor.prompt()
        self.assertEqual((own["decision"]["chooser"], own["decision"]["acting"]), ("p0", "p1"))
        self.assertIn("request belongs to player 0", self.refused(actor, {"cmd": "answer", "answer": {"kind": "pass"}}))
        self.assertIsNone(third.prompt()["decision"])
        me.answer({"kind": "cast", "card": self.named(p, "Ponder")[0], "mana": {"U": 1},
                   "plan": [{"ability": f"{self.named(p, 'Island')[0]}.a0"}]})
        stacked = me.last
        self.assertEqual(stacked["objects"][stacked["stack"][0]]["occurrence"]["controller"], "p1")
        self.assertEqual(stacked["players"][1]["pool"]["U"], 0)
        self.assertIs(self.pass_until_asked(), me)
        looked = me.last["decision"]["cards"]
        self.assertEqual(len(looked), 3)
        actor_view = actor.prompt()
        self.assertEqual(len(actor_view["decision"]["cards"]), 3)
        self.assertFalse([o for o in third.prompt()["objects"].values() if o["zone"] == "library"])
        order=[self.named(me.last,name,zone="library")[0] for name in ["Island","Shock","Grizzly Bears"]]
        me.answer({"kind": "choose", "answer": {"objects": order}})
        me.answer({"kind": "choose", "answer": "no"})
        victim_cards = me.last["players"][1]["hand"][:]
        turn = me.last["turn"]
        for _ in range(120):
            seat = self.waiting()
            if seat.last["turn"] != turn:
                break
            q = seat.last["decision"]
            seat.answer({"kind": "attack", "attacks": []} if q["kind"] == "attackers" else {"kind": "pass"})
        else:
            self.fail("controlled turn did not end")
        after = me.prompt()
        self.assertNotIn("hand", after["players"][1])
        self.assertTrue(all(c not in after["objects"] for c in victim_cards))

    def test_ward_payment_has_source_cost_and_works_without_a_move_list(self):
        me, _ = self.new({"hand": ["Lightning Bolt"], "battlefield": ["Mountain"] * 3},
                         {"battlefield": ["Tolarian Terror"]})
        p = me.prompt()
        mountains = self.named(p, "Mountain")
        me.answer({"kind": "cast", "card": self.named(p, "Lightning Bolt")[0], "mana": {"R": 1},
                   "targets": [self.named(p, "Tolarian Terror")],
                   "plan": [{"ability": f"{mountains[0]}.a0"}]})
        self.assertIs(self.pass_until_asked(), me)
        q = me.last["decision"]
        self.assertEqual(q["kind"], "pay")
        self.assertEqual(q["for"]["source_name"], "Tolarian Terror")
        self.assertEqual(q["cost"], ["2"])
        self.assertNotIn("moves", q)
        me.answer({"kind": "choose", "answer": {"pay": {"mana": {"R": 2}, "plan": [{"ability": f"{m}.a0"} for m in mountains[1:]]}}})
        self.pass_until_asked()
        p = me.prompt()
        self.assertEqual(p["objects"][self.named(p, "Tolarian Terror")[0]]["damage"], 3)

    def test_transforming_a_source_does_not_change_its_waiting_ability_text(self):
        # 113.7a: the Recruiter's original ability remains on the stack after
        # Waxing Moon transforms its source into Krallenhorde Howler.
        me, them = self.new(
            {"hand": ["Waxing Moon", "Duskwatch Recruiter"], "battlefield": ["Forest"] * 7,
             "library": ["Forest", "Forest", "Grizzly Bears", "Shock", "Forest"]}, {})
        p = me.prompt()
        forests = self.named(p, "Forest", zone="battlefield")
        me.answer({"kind": "cast", "card": self.named(p, "Duskwatch Recruiter")[0], "mana": {"G": 2},
                   "plan": [{"ability": f"{f}.a0"} for f in forests[:2]]})
        me.answer({"kind": "pass"})
        them.prompt()
        them.answer({"kind": "pass"})
        p = me.prompt()
        recruiter = self.named(p, "Duskwatch Recruiter")[0]
        forests = [f for f in forests if not p["objects"][f]["tapped"]]
        activation = next(a for a in p["objects"][recruiter]["abilities"] if "Look at the top three" in a["text"])
        original_text = activation["text"]
        me.answer({"kind": "activate", "ability": activation["label"], "mana": {"G": 3},
                   "plan": [{"ability": f"{f}.a0"} for f in forests[:3]]})
        me.answer({"kind": "cast", "card": self.named(me.last, "Waxing Moon")[0], "mana": {"G": 2}, "targets": [[recruiter]],
                   "plan": [{"ability": f"{f}.a0"} for f in forests[3:]]})
        me.answer({"kind": "pass"})
        them.prompt()
        them.answer({"kind": "pass"})
        p = me.prompt()
        self.assertEqual(p["objects"][recruiter]["name"], "Krallenhorde Howler")
        [ability] = p["stack"]
        occurrence = p["objects"][ability]["occurrence"]
        self.assertEqual(occurrence["source_name"], "Duskwatch Recruiter")
        self.assertEqual(occurrence["text"], original_text)
        self.assertIs(self.pass_until_asked(), me)
        self.assertEqual(me.last["decision"]["for"]["text"], original_text)
