"""Shared audited Python/JSON client fixtures, independent of scenario test modules.

Tests supply their actions, payment resources, targets and answers. Scenario-specific
setup and interaction helpers stay with the tests that describe those scenarios.
"""
import os
from pathlib import Path
import subprocess
import sys
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import play


class ClientTestCase(unittest.TestCase):
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
        self.engine.ask({"cmd": "new", "players": [{"library": ["Forest"] * 8, **seat} for seat in seats]})
        for _ in range(25):
            if self.engine.state["part"] == "PrecombatMain":
                return
            self.engine.act({"kind": "pass"})
        self.fail("did not reach main phase")

    def view(self, player):
        return self.engine.send({"cmd": "state", "viewer": player})

    def hand(self, player):
        return self.view(player)["players"][player]["hand"]

    def ids(self, name, zone=None, controller=None):
        return [int(key) for key, obj in self.engine.state["objects"].items() if obj["name"] == name
                and (zone is None or int(key) in self.engine.state.get(zone, []))
                and (controller is None or obj.get("controller") == controller)]

    def cast(self, name, targets=None, *, mana=(), plan=None, card=None, spend=None, **choices):
        # Sources are fixture choices supplied by the test, never inferred from a cost.
        state = self.engine.state
        actor = state["asked"].get("acting", state["asked"]["player"])
        cards = state["players"][actor].get("hand", []) + state.get("command", [])
        card = card if card is not None else next(o for o in cards if state["objects"][str(o)]["name"] == name)
        self.engine.act({"kind": "cast", "card": card, "targets": targets or [],
                         "plan": plan if plan is not None else [{"source": o, "ability": 0} for o in mana],
                         **({"mana": spend} if spend is not None else {}), **choices})

    def validate_action(self, move, accepted=True):
        """Check this supplied action on the existing isolated execution hook."""
        state = self.engine.state
        result = self.engine.send({"cmd": "quote_payment", "session_id": state["session_id"],
                                   "request_id": state["request_id"], "player": state["asked"]["player"], "move": move})
        self.assertEqual(result["error"] is None, accepted, result)
        return result

    def pass_round(self):
        for _ in self.engine.state["players"]:
            self.engine.act({"kind": "pass"})

    def choose(self, answer):
        self.engine.act({"kind": "choose", "answer": answer})

    def reject(self, player, move):
        state = self.engine.state
        with self.assertRaises(SystemExit):
            self.engine.send({"cmd": "act", "player": player, "move": move, "viewer": "asked",
                              "session_id": state["session_id"], "request_id": state["request_id"]})
        self.assertEqual(self.view(state["viewer"])["request_id"], state["request_id"])

    def object(self, obj):
        return self.engine.state["objects"][str(obj)]

    def activate(self, source, targets, *, ability, mana=(), paying=(), spend=None, **choices):
        self.engine.act({"kind": "activate", "source": source, "ability": ability,
                         "targets": targets, "plan": [{"source": o, "ability": 0} for o in mana],
                         "paying": list(paying), **({"mana": spend} if spend is not None else {}), **choices})

    def step(self):
        state = self.engine.state
        asked = state["asked"]
        kind = asked["kind"]
        if kind == "mulligan":
            self.engine.act({"kind": "keep"})
        elif kind == "attackers":
            self.engine.act({"kind": "attack", "attacks": []})
        elif kind == "blockers":
            self.engine.act({"kind": "block", "blocks": []})
        elif kind == "discard":
            self.engine.act({"kind": "discard", "cards": self.hand(asked["player"])[:asked["count"]]})
        else:
            self.assertEqual(kind, "priority", state)
            self.engine.act({"kind": "pass"})

    def advance(self, predicate):
        for _ in range(600):
            if predicate(self.engine.state):
                return
            self.step()
        self.fail("did not reach checkpoint")
