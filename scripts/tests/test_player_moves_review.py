"""Explicit action round trips through player/1, derived from Oracle text and the rules.

Set GAME_SERVE_REVIEW to an independently built executable when reviewing a revision.
"""
import os
import unittest

import test_player_prompt_client as fixtures
import play


class PlayerMovesReviewTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        if not os.environ.get("GAME_SERVE_REVIEW"):
            fixtures.PlayerPromptClientTests.setUpClass()

    def setUp(self):
        self.engine = play.Engine(executable=os.environ.get("GAME_SERVE_REVIEW"))
        self.addCleanup(self.close_engine)

    close_engine = fixtures.PlayerPromptClientTests.close_engine
    new = fixtures.PlayerPromptClientTests.new
    waiting = fixtures.PlayerPromptClientTests.waiting
    pass_until_asked = fixtures.PlayerPromptClientTests.pass_until_asked
    named = staticmethod(fixtures.PlayerPromptClientTests.named)
    refused = fixtures.PlayerPromptClientTests.refused

    def test_convoke_choices_name_two_distinct_creatures_and_pay_the_spell(self):
        # 702.51a / 118.10: one green and one generic mana can be paid by two
        # distinct green creatures, even with no available mana abilities.
        me, _ = self.new({"hand": ["Sprout Swarm"], "battlefield": ["Grizzly Bears"] * 2}, {})
        p = me.prompt()
        bears = self.named(p, "Grizzly Bears")
        after = me.answer({"kind": "cast", "card": self.named(p, "Sprout Swarm")[0],
                           "convoke": ["G", "generic"], "paying": [[bears[0]], [bears[1]]]})
        self.assertTrue(all(after["objects"][b]["tapped"] for b in bears))
        self.pass_until_asked()
        self.assertEqual(len(self.named(me.prompt(), "Saproling Token", zone="battlefield")), 1)

    def test_delve_cost_candidates_are_labelled_graveyard_cards_and_exiled(self):
        # Treasure Cruise / 702.66a: seven exiled cards pay the seven generic mana.
        me, _ = self.new({"hand": ["Treasure Cruise"], "battlefield": ["Island"], "graveyard": ["Forest"] * 7},
                        {"hand": ["Lightning Bolt"]})
        p = me.prompt()
        self.assertFalse(self.named(p, "Lightning Bolt"), "CR 402.3")
        after = me.answer({"kind": "cast", "card": self.named(p, "Treasure Cruise")[0], "mana": {"U": 1},
                           "delve": 7, "paying": [list(p["players"][0]["graveyard"])],
                           "plan": [{"ability": f"{self.named(p, 'Island')[0]}.a0"}]})
        self.assertEqual(len(self.named(after, "Forest", zone="exile")), 7)
        self.assertEqual(after["players"][0]["graveyard"], [])
        self.pass_until_asked()
        self.assertEqual(len(me.prompt()["players"][0]["hand"]), 3)

    def test_escape_type_mask_keys_use_the_same_labels_as_the_cost_candidates(self):
        # Nethergoyf's escape counts four types among the other exiled cards (702.138a).
        me, _ = self.new({"battlefield": ["Swamp"] * 3,
                         "graveyard": ["Nethergoyf", "Perilous Myr", "Forest", "Shock"]}, {})
        p = me.prompt()
        [card] = self.named(p, "Nethergoyf")
        exile = [o for o in p["players"][0]["graveyard"] if o != card]
        self.assertEqual(len(exile), 3)
        after = me.answer({"kind": "cast", "card": card, "route": "escape", "mana": {"B": 3}, "paying": [exile],
                           "plan": [{"ability": f"{o}.a0"} for o in self.named(p, "Swamp")]})
        self.assertEqual(len(after["exile"]), 3)
        self.assertEqual(after["objects"][after["stack"][0]]["name"], "Nethergoyf")

    def test_crew_power_map_keys_are_usable_labels(self):
        # 702.122a: a creature with power two can pay crew 1.
        me, _ = self.new({"battlefield": ["Smuggler's Copter", "Grizzly Bears"]}, {})
        p = me.prompt()
        [bears] = self.named(p, "Grizzly Bears")
        [copter] = self.named(p, "Smuggler's Copter")
        crew = next(a["label"] for a in p["objects"][copter]["abilities"] if a["text"].startswith("Tap any number of other untapped creatures"))
        after = me.answer({"kind": "activate", "ability": crew, "paying": [[bears]]})
        self.assertTrue(after["objects"][bears]["tapped"])
        self.pass_until_asked()
        copter = self.named(me.prompt(), "Smuggler's Copter")[0]
        self.assertEqual(me.last["objects"][copter]["power"], 3)

    def test_library_permission_source_and_top_card_round_trip_without_disclosing_the_next_card(self):
        # Citadel permits looking at the top card and paying life equal to mana value
        # instead of mana. It does not reveal the library to an opponent (401.2).
        me, them = self.new({"battlefield": ["Bolas's Citadel"],
                             "library": ["Island", "Island", "Forest", "Shock", "Grizzly Bears"]},
                            {"hand": ["Lightning Bolt"]})
        p = me.prompt()
        [citadel] = self.named(p, "Bolas's Citadel")
        [card] = self.named(p, "Grizzly Bears", zone="library")
        self.assertFalse(self.named(p, "Shock"))
        self.assertFalse([o for o in them.prompt()["objects"].values() if o["zone"] == "library"])
        after = me.answer({"kind": "cast", "card": card, "route": {"source": citadel, "ability": 1}})
        self.assertEqual(after["players"][0]["life"], 18)
        self.assertEqual(after["objects"][after["stack"][0]]["name"], "Grizzly Bears")
        self.assertEqual(len(self.named(after, "Shock", zone="library")), 1)
        self.assertFalse([o for o in them.prompt()["objects"].values() if o["zone"] == "library"])

    def test_modes_and_target_slots_remain_separate_choices_that_can_be_submitted(self):
        me, _ = self.new({"hand": ["Abrade"], "battlefield": ["Mountain"] * 2},
                        {"battlefield": ["Myr Retriever"]})
        p = me.prompt()
        [target] = self.named(p, "Myr Retriever")
        me.answer({"kind": "cast", "card": self.named(p, "Abrade")[0], "mana": {"R": 2}, "modes": [1],
                   "targets": [[target]], "plan": [{"ability": f"{o}.a0"} for o in self.named(p, "Mountain")]})
        self.pass_until_asked()
        self.assertFalse(self.named(me.prompt(), "Myr Retriever", zone="battlefield"))

    def test_a_direct_answer_chooses_arbitrary_affordable_x(self):
        # CR 107.3a / 601.2b: the player chooses an arbitrary affordable X.
        me, _ = self.new({"hand": ["Blaze"], "battlefield": ["Mountain"] * 18}, {})
        p = me.prompt()
        me.answer({"kind": "cast", "card": self.named(p, "Blaze")[0], "x": 17, "mana": {"R": 18},
                   "targets": [["p1"]], "plan": [{"ability": f"{m}.a0"} for m in self.named(p, "Mountain")]})
        self.pass_until_asked()
        self.assertEqual(me.prompt()["players"][1]["life"], 3)

    def test_a_prompt_delivers_pending_events_once(self):
        me, them = self.new({"hand": ["Lightning Bolt"], "battlefield": ["Mountain"]}, {})
        p = me.prompt()
        me.answer({"kind": "cast", "card": self.named(p, "Lightning Bolt")[0], "mana": {"R": 1}, "targets": [["p1"]],
                   "plan": [{"ability": f"{self.named(p, 'Mountain')[0]}.a0"}]})
        after = me.answer({"kind": "pass"})
        # Correlation identifiers are public; the opponent has not read the cast events.
        them.last = {k: after[k] for k in ("session_id", "request_id")}
        first = them.prompt()
        self.assertEqual(len([e for e in first["events"] if e["event"] == "cast"]), 1)
        repeated = them.prompt()
        self.assertEqual(repeated["decision"], first["decision"])
        self.assertEqual(repeated["events"], [])
        ordinary = them.prompt()
        self.assertEqual(ordinary["events"], [])
        self.assertNotIn("moves", ordinary["decision"])
        self.assertEqual(ordinary["request_id"], first["request_id"])

    def test_another_valid_payment_plan_can_be_answered_directly(self):
        # CR 601.2g: the player chooses which available mana ability to activate.
        me, _ = self.new({"hand": ["Shock"], "battlefield": ["Mountain"] * 2}, {})
        p = me.prompt()
        mountains = self.named(p, "Mountain")
        me.answer({"kind": "cast", "card": self.named(p, "Shock")[0], "mana": {"R": 1}, "targets": [["p1"]],
                   "plan": [{"ability": f"{mountains[1]}.a0"}]})
        self.assertFalse(me.last["objects"][mountains[0]]["tapped"])
        self.assertTrue(me.last["objects"][mountains[1]]["tapped"])
        self.pass_until_asked()
        self.assertEqual(me.prompt()["players"][1]["life"], 18)

    def test_the_numeric_client_returns_priority_without_discovery(self):
        sent = []
        send = self.engine.send
        self.engine.send = lambda request: (sent.append(request["cmd"]), send(request))[1]
        state = self.engine.ask({"cmd": "new", "players": [{"hand": ["Shock"], "battlefield": ["Mountain"]}, {}]})
        self.assertNotIn("moves", state["asked"])
        self.assertEqual(sent, ["new"])
        self.engine.ask({"cmd": "state"})
        self.assertEqual(sent, ["new", "state"])

    def test_controlled_turn_uses_the_actors_cards_and_payment(self):
        # 723.3 / 723.5: the controller decides; the victim is the player acting.
        me, actor, third = self.new(
            {"hand": ["Shock", "Emrakul, the Promised End"], "battlefield": ["Wastes"] * 13},
            {"hand": ["Ponder"], "battlefield": ["Island"], "library": ["Forest"] * 8},
            {"hand": ["Demonic Tutor"]})
        p = me.prompt()
        me.answer({"kind": "cast", "card": self.named(p, "Emrakul, the Promised End")[0], "mana": {"C": 13},
                   "plan": [{"ability": f"{w}.a0"} for w in self.named(p, "Wastes")]})
        me.answer({"kind": "targets", "targets": [["p1"]]})
        for _ in range(120):
            seat = self.waiting()
            p = seat.last
            if p["active"] == "p1" and p["part"] == "PrecombatMain" and p["decision"]["kind"] == "priority":
                break
            seat.answer({"kind": "attack", "attacks": []} if p["decision"]["kind"] == "attackers" else {"kind": "pass"})
        else:
            self.fail("controlled turn did not start")
        self.assertIs(seat, me)
        p = me.prompt()
        self.assertEqual((p["decision"]["chooser"], p["decision"]["acting"]), ("p0", "p1"))
        [ponder] = self.named(p, "Ponder", zone="hand")
        island = self.named(p, "Island", controller="p1")[0]
        actor.prompt()
        self.assertIn("request belongs to player 0", self.refused(actor, {"cmd": "answer", "answer": {"kind": "pass"}}))
        third.prompt()
        self.assertIn("request belongs to player 0", self.refused(third, {"cmd": "answer", "answer": {"kind": "pass"}}))
        after = me.answer({"kind": "cast", "card": ponder, "mana": {"U": 1}, "plan": [{"ability": f"{island}.a0"}]})
        self.assertEqual(after["objects"][after["stack"][0]]["occurrence"]["controller"], "p1")


if __name__ == "__main__":
    unittest.main()
