"""Refusal classes on the wire and request validation before acting, through the
actual game-serve binary over both routes: the pipe (`play.Engine`) and the shared host
(`shared_host.SharedHost`). Expectations are the protocol's (game-serve.rs module docs) and the
rules a refusal cites, not engine output."""
import json
import os
import subprocess
import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import play
from shared_host import SharedHost


def setUpModule():
    if not os.environ.get("GAME_SERVE_REVIEW"):
        subprocess.run(["cargo", "build", "--locked", "--profile", "fast", "-p", "game", "--bin", "game-serve"],
                       cwd=play.ROOT, check=True, stdout=subprocess.DEVNULL)


class Routes:
    """The same checks through a route; `engine()` starts a game client on it."""

    def engine(self) -> play.Engine:
        raise NotImplementedError

    def refused(self, engine, request: dict) -> dict:
        with self.assertRaises(play.Refused) as caught:
            engine.send(request)
        return caught.exception.answer

    def act(self, engine, state: dict, move: dict, **fields) -> dict:
        return {"cmd": "act", "session_id": state["session_id"], "request_id": state["request_id"],
                "player": state["asked"]["player"], "move": move, **fields}

    def test_an_invalid_viewer_is_refused_before_the_action_is_applied(self):
        # two empty fixture players, session 1 / request 1 / player 0's priority
        engine = self.engine()
        first = engine.send({"cmd": "new", "players": [{}, {}]})
        self.assertEqual((first["request_id"], first["asked"]["kind"], first["asked"]["player"]), (1, "priority", 0))
        whole = engine.send({"cmd": "state"})
        refusals = []
        for viewer in (999, "everyone"):
            refusals.append(self.refused(engine, self.act(engine, first, {"kind": "pass"}, viewer=viewer)))
            # not consumed: the same request is pending and the game unchanged
            self.assertEqual(engine.send({"cmd": "state"}), whole, viewer)
        self.assertEqual([(r.get("class"), r["error"]) for r in refusals],
                         [("unreferable", 'a viewer is a player or "asked", not 999'),
                          ("malformed", 'a viewer is a player or "asked", not "everyone"')])
        # and the retry is accepted
        after = engine.send(self.act(engine, first, {"kind": "pass"}))
        self.assertEqual(after["request_id"], 2)
        # the answered request is now stale
        self.assertEqual(self.refused(engine, self.act(engine, first, {"kind": "pass"}))["class"], "stale")

    def test_each_refusal_says_its_class_and_an_illegal_one_its_rules(self):
        engine = self.engine()
        state = engine.send({"cmd": "new", "players": [{"hand": ["Forest", "Forest"]}, {}]})
        # a land is played in a main phase of its player's turn (305.1)
        while state["part"] != "PrecombatMain":
            state = engine.send(self.act(engine, state, {"kind": "pass"}))
        forests = [int(o) for o, obj in state["objects"].items() if obj["name"] == "Forest"]
        played = engine.send(self.act(engine, state, {"kind": "play", "card": forests[0]}))
        # illegal: one land a turn (305.2); it cites the rule it says
        refusal = self.refused(engine, self.act(engine, played, {"kind": "play", "card": forests[1]}))
        self.assertEqual((refusal["class"], refusal["rules"]), ("illegal", ["305.2"]), refusal)
        # unreferable: an object the game does not have
        refusal = self.refused(engine, self.act(engine, played, {"kind": "play", "card": 999_999}))
        self.assertEqual(refusal["class"], "unreferable", refusal)
        self.assertNotIn("rules", refusal)
        # stale: an outdated session id
        refusal = self.refused(engine, self.act(engine, played, {"kind": "pass"}) | {"session_id": played["session_id"] + 7})
        self.assertEqual(refusal["class"], "stale", refusal)
        # malformed: no move, a misspelled field, a command there is none of; each changes nothing
        whole = engine.send({"cmd": "state"})
        for request in ({k: v for k, v in self.act(engine, played, {"kind": "pass"}).items() if k != "move"},
                        self.act(engine, played, {"kind": "pass", "card": forests[1]}),
                        {"cmd": "dance"}):
            self.assertEqual(self.refused(engine, request)["class"], "malformed", request)
            self.assertEqual(engine.send({"cmd": "state"}), whole, request)
        engine.send(self.act(engine, played, {"kind": "pass"}))

        # unsupported: a concession by the player whose decision is pending, in a game that
        # goes on (104.3a, 800.4a), is something the engine does not carry out; nothing changes
        three = engine.send({"cmd": "new", "players": [{}, {}, {}]})
        whole = engine.send({"cmd": "state"})
        refusal = self.refused(engine, self.act(engine, three, {"kind": "concede"}))
        self.assertEqual(refusal["class"], "unsupported", refusal)
        self.assertNotIn("rules", refusal)
        self.assertEqual(engine.send({"cmd": "state"}), whole)

    def test_a_misspelled_field_or_a_flag_that_is_no_boolean_is_refused_before_acting(self):
        # a misspelled `viewer` is no viewer; were it ignored, the pass would be applied and
        # answered with the whole game. Neither request is consumed.
        engine = self.engine()
        first = engine.send({"cmd": "new", "players": [{}, {}]})
        whole = engine.send({"cmd": "state"})
        for request in (self.act(engine, first, {"kind": "pass"}, veiwer=0),
                        self.act(engine, first, {"kind": "pass"}, capture_random="yes"),
                        {"cmd": "quote_payment", "session_id": first["session_id"], "request_id": first["request_id"],
                         "player": 0, "move": {"kind": "pass"}, "veiwer": 0}):
            refusal = self.refused(engine, request)
            self.assertEqual(refusal["class"], "malformed", refusal)
            self.assertEqual(engine.send({"cmd": "state"}), whole, request)
        self.assertEqual(engine.send(self.act(engine, first, {"kind": "pass"}, viewer=0))["request_id"], 2)

    def test_a_player_the_game_does_not_have_is_unreferable_and_changes_nothing(self):
        # Shock: "Shock deals 2 damage to any target." A target is a player or object of the game
        # (115.1); a two-player game has no player 9, so the move names nothing it can refer to.
        engine = self.engine()
        state = engine.send({"cmd": "new", "players": [{"hand": ["Shock"], "battlefield": ["Mountain"]}, {}]})
        while state["part"] != "PrecombatMain":
            state = engine.send(self.act(engine, state, {"kind": "pass"}))
        named = {obj["name"]: int(o) for o, obj in state["objects"].items()}
        cast = lambda target: {"kind": "cast", "card": named["Shock"], "targets": [[target]], "mana": {"R": 1},
                               "plan": [{"source": named["Mountain"], "ability": 0}]}
        whole = engine.send({"cmd": "state"})
        refusal = self.refused(engine, self.act(engine, state, cast({"player": 9})))
        self.assertEqual((refusal["class"], refusal.get("rules")), ("unreferable", None), refusal)
        self.assertEqual(engine.send({"cmd": "state"}), whole)
        # a seat names players by label: p9 is no player of this game either
        seat = play.Seat(engine, 0)
        prompt = seat.prompt()
        [shock] = [label for label, o in prompt["objects"].items() if o["name"] == "Shock"]
        [mountain] = [label for label, o in prompt["objects"].items() if o["name"] == "Mountain"]
        answer = lambda target: {"kind": "cast", "card": shock, "targets": [[target]], "mana": {"R": 1},
                                 "plan": [{"ability": f"{mountain}.a0"}]}
        refusal = self.refused(engine, {"cmd": "answer", "seat": "p0", "session_id": prompt["session_id"],
                                        "request_id": prompt["request_id"], "answer": answer("p9")})
        self.assertEqual(refusal["class"], "unreferable", refusal)
        self.assertEqual(engine.send({"cmd": "state"}), whole)
        # not consumed: the same request takes the answer naming the opponent
        after = seat.answer(answer("p1"))
        self.assertEqual(len(after["stack"]), 1)

    def test_a_quote_says_the_class_of_the_quoted_moves_refusal(self):
        # a quote is answered (ok); its proof says the quoted move's refusal as the move's own
        # refusal would: a land outside a main phase (305.1) is illegal and cites it, an object
        # the game does not have is unreferable, a concession while that player's decision is
        # pending in a game that goes on (104.3a, 800.4a) is not something the engine runs
        engine = self.engine()
        state = engine.send({"cmd": "new", "players": [{"hand": ["Forest"]}, {}, {}]})
        self.assertEqual(state["part"], "Upkeep")
        [forest] = [int(o) for o, obj in state["objects"].items() if obj["name"] == "Forest"]
        whole = engine.send({"cmd": "state"})
        for move, class_ in (({"kind": "play", "card": forest}, "illegal"), ({"kind": "play", "card": 999_999}, "unreferable"),
                             ({"kind": "concede"}, "unsupported")):
            proof = engine.send({"cmd": "quote_payment", "session_id": state["session_id"], "request_id": state["request_id"],
                                 "player": 0, "move": move})
            self.assertEqual((proof["complete"], proof["error_class"]), (False, class_), proof)
            self.assertEqual(proof.get("error_rules"), ["305.1"] if class_ == "illegal" else None, proof)
            acted = self.refused(engine, self.act(engine, state, move))
            self.assertEqual((acted["class"], acted["error"]), (class_, proof["error"]), move)
            self.assertEqual(engine.send({"cmd": "state"}), whole, move)

    def test_a_commander_deck_the_rules_do_not_allow_is_illegal(self):
        # "Each Commander deck must contain exactly 100 cards, including its commander" (903.5a);
        # a refused setup leaves the game there was
        engine = self.engine()
        engine.send({"cmd": "new", "players": [{}, {}]})
        whole = engine.send({"cmd": "state"})
        isamaru = "Isamaru, Hound of Konda"
        refusal = self.refused(engine, {"cmd": "start", "format": "commander", "decks": [["Plains"] * 98, ["Plains"] * 99],
                                        "commanders": [isamaru, isamaru]})
        self.assertEqual((refusal["class"], refusal["rules"]), ("illegal", ["903.5a"]), refusal)
        self.assertEqual(engine.send({"cmd": "state"}), whole)
        # a 99-card library and its commander is a deck of 100; Plains are within white Isamaru's
        # color identity (903.5c, 903.5d)
        started = engine.send({"cmd": "start", "format": "commander", "decks": [["Plains"] * 99] * 2,
                               "commanders": [isamaru, isamaru]})
        self.assertEqual(started["players"][0]["life"], 40)


class PipeRefusals(Routes, unittest.TestCase):
    def engine(self) -> play.Engine:
        engine = play.Engine()
        self.addCleanup(self.close, engine)
        return engine

    @staticmethod
    def close(engine):
        engine.proc.stdin.close()
        engine.proc.wait(timeout=10)
        engine.proc.stdout.close()

    def test_a_line_that_is_not_json_is_malformed_and_changes_nothing(self):
        engine = self.engine()
        engine.send({"cmd": "new", "players": [{}, {}]})
        whole = engine.send({"cmd": "state"})
        engine.proc.stdin.write("{not json\n")
        engine.proc.stdin.flush()
        answer = json.loads(engine.proc.stdout.readline())
        self.assertEqual((answer["ok"], answer["class"]), (False, "malformed"), answer)
        self.assertEqual(engine.send({"cmd": "state"}), whole)


class SharedHostRefusals(Routes, unittest.TestCase):
    def setUp(self):
        self.host = SharedHost(executable=os.environ.get("GAME_SERVE_REVIEW"), workers=2)
        self.addCleanup(self.host.close)

    # an envelope the host cannot read has no id to route its reply by; the host's own
    # tests (game-serve/host.rs) check that it says `malformed`
    def engine(self) -> play.Engine:
        return self.host.session()


if __name__ == "__main__":
    unittest.main()
