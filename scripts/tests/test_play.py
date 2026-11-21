"""Explicit client privacy checks (CR 402.3, 701.20a, 111.6, 707.10)."""
import unittest
from unittest.mock import Mock
import play

class HandPrivacyTests(unittest.TestCase):
    def engine(self):
        engine = play.Engine.__new__(play.Engine)
        engine.disclosed = {}
        engine.state = {"viewer": 0, "asked": {"kind": "objects", "player": 0},
                        "players": [{"hand_size": 0, "hand": []}, {"hand_size": 2}],
                        "objects": {"10": {"name": "Shown"}}, "events": [],
                        "observations": [{"kind": "revealed", "cards": [{"object": 10}]}]}
        engine.send = Mock(return_value={"players": [{"hand": []}, {"hand": [10, 11]}],
                                       "objects": {"10": {"is_card": True}, "11": {"is_card": True}}})
        return engine

    def test_paused_resolution_counts_cards_but_preserves_every_hand_identity(self):
        for token in [True, False]:
            with self.subTest(token=token):
                engine = self.engine()
                # A returned token, or a bounced spell copy, is not a card (111.6, 707.10).
                objects = {"20": {"is_card": True, "token": False},
                           "21": {"is_card": False, "token": token}}
                engine.state["players"][0] = {"hand_size": 1, "hand": [20, 21]}
                engine.state["objects"].update(objects)
                engine.send.return_value["players"][0]["hand"] = [20, 21]
                engine.send.return_value["objects"].update(objects)
                engine.check()
                engine.state["players"][0]["hand_size"] = 2
                with self.assertRaisesRegex(AssertionError, "differs from 1 cards"):
                    engine.check()
                engine.state["players"][0]["hand_size"] = 1
                engine.state["players"][0]["hand"] = [20]
                with self.assertRaises(AssertionError):
                    engine.check()

    def test_noncard_opponent_hand_identities_are_still_private(self):
        engine = self.engine()
        engine.send.return_value["objects"]["11"]["is_card"] = False
        engine.state["players"][1]["hand_size"] = 1
        engine.check()
        engine.state["objects"]["11"] = {"is_card": False, "token": True}
        with self.assertRaisesRegex(AssertionError, "shown cards of the hand"):
            engine.check()

    def test_revealed_cards_survive_later_choices_but_not_priority(self):
        engine = self.engine()
        engine.check()
        engine.state["observations"] = []
        engine.check()
        engine.state["asked"]["kind"] = "priority"
        with self.assertRaises(AssertionError):
            engine.check()

    def test_unrevealed_cards_and_wrong_viewers_still_fail(self):
        engine = self.engine()
        engine.state["objects"]["11"] = {"name": "Secret"}
        with self.assertRaises(AssertionError):
            engine.check()
        engine = self.engine()
        engine.disclosed = {1: {10}}
        engine.state["observations"] = []
        with self.assertRaises(AssertionError):
            engine.check()

    def test_typed_target_events_are_checked_without_confusing_players_and_objects(self):
        engine = self.engine()
        engine.state["events"] = [{"kind": "became_target", "object": {"object": 10}},
                                  {"kind": "became_target", "object": {"player": 11}}]
        engine.check()
        engine.state["events"].append({"kind": "became_target", "object": {"object": 11}})
        with self.assertRaises(AssertionError):
            engine.check()


class ExplicitClientTests(unittest.TestCase):
    def test_priority_is_returned_without_a_discovery_query(self):
        engine = play.Engine.__new__(play.Engine)
        engine.state = {'session_id': 2, 'request_id': 3}
        engine.disclosed, engine.audit = {}, False
        pending = {'session_id': 2, 'request_id': 4, 'asked': {'kind': 'priority', 'player': 0}}
        engine.send = Mock(return_value=pending)
        action = {'cmd': 'act', 'player': 0, 'move': {'kind': 'pass'}}
        self.assertIs(engine.ask(action), pending)
        engine.send.assert_called_once_with(action | {'session_id': 2, 'request_id': 3, 'viewer': 'asked'})
