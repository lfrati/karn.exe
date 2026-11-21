"""Explicit player actions through the real client: targets, costs and pending rules choices. Expectations come from the cards' Oracle text and the rules cited."""
import unittest
import os

import test_player_prompt_client as fixtures
import play


class PlayerMovesClientTests(unittest.TestCase):
    setUpClass = classmethod(fixtures.PlayerPromptClientTests.setUpClass.__func__)
    close_engine = fixtures.PlayerPromptClientTests.close_engine
    new = fixtures.PlayerPromptClientTests.new
    waiting = fixtures.PlayerPromptClientTests.waiting
    named = staticmethod(fixtures.PlayerPromptClientTests.named)
    refused = fixtures.PlayerPromptClientTests.refused

    def setUp(self):
        # the training path: nothing asks for a listing on the client's behalf
        self.engine = play.Engine(executable=os.environ.get("GAME_SERVE_REVIEW"))
        self.addCleanup(self.close_engine)
        self.sent = []
        send = self.engine.send
        self.engine.send = lambda request: (self.sent.append(request.get("cmd")), send(request))[1]

    def bolt(self):
        me, them = self.new({"hand": ["Lightning Bolt"], "battlefield": ["Mountain"]},
                            {"battlefield": ["Grizzly Bears"], "hand": ["Shock"]})
        return me, them, me.prompt()

    def test_a_player_plays_by_prompt_and_answer_alone(self):
        me, them, prompt = self.bolt()
        [bolt], [mountain], [bears] = (self.named(prompt, n) for n in ("Lightning Bolt", "Mountain", "Grizzly Bears"))
        self.assertNotIn("moves", prompt["decision"])
        # an answer the rules refuse is refused as ever (601.2h: {R} must be paid), and the
        # question stays asked
        self.assertIn("refused", self.refused(me, {"cmd": "answer", "answer": {"kind": "play", "card": bolt}}))
        after = me.answer({"kind": "cast", "card": bolt, "mana": {"R": 1}, "targets": [[bears]], "plan": [{"ability": f"{mountain}.a0"}]})
        self.assertEqual(after["objects"][after["stack"][0]]["name"], "Lightning Bolt")
        me.answer({"kind": "pass"})
        them.prompt()
        them.answer({"kind": "pass"})
        self.assertNotIn(bears, me.prompt()["objects"], "704.5g: the Bears died")
        self.assertNotIn("options", self.sent)
        self.assertNotIn("moves", self.sent)

    def test_refused_answers_leave_priority_and_private_information_unchanged(self):
        me, them, prompt = self.bolt()
        self.assertFalse(self.named(prompt, "Shock"), "CR 402.3")
        for _ in range(4):
            self.refused(me, {"cmd": "answer", "answer": {"kind": "cast", "card": "o999"}})
        after = me.prompt()
        self.assertEqual(after["request_id"], prompt["request_id"])
        self.assertEqual(after["waiting_for"], "p0")
        self.assertEqual(after["objects"], prompt["objects"])
        self.assertNotIn("delegated", after)

    def test_a_question_other_than_priority_is_not_listed(self):
        # Mind Rot: the target player chooses two cards to discard (701.9b); that question says
        # what may be answered, and no listing is made for it
        me, them = self.new({"hand": ["Mind Rot"], "battlefield": ["Swamp"] * 3},
                            {"hand": ["Shock", "Shock", "Lightning Bolt"]})
        prompt = me.prompt()
        me.answer({"kind": "cast", "card": self.named(prompt, "Mind Rot")[0], "mana": {"B": 3}, "targets": [["p1"]],
                   "plan": [{"ability": f"{s}.a0"} for s in self.named(prompt, "Swamp")]})
        me.answer({"kind": "pass"})
        them.prompt()
        them.answer({"kind": "pass"})
        asked = them.prompt()["decision"]
        self.assertEqual((asked["kind"], asked["count"], len(asked["among"])), ("objects", 2, 3))
        selected = self.named(them.last, "Shock")
        self.assertEqual(len(selected),2)
        after = them.answer({"kind": "choose", "answer": {"objects": selected}})
        self.assertEqual(after["decision"]["kind"], "order_cards")
        after = them.answer({"kind":"choose","answer":{"objects":selected}})
        self.assertEqual(after["players"][1]["hand_size"], 1)
        self.assertEqual(len(after["players"][1]["graveyard"]), 2)


if __name__ == "__main__":
    unittest.main()
