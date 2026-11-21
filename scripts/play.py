"""Explicit engine clients and action recording; no player policy or move search."""

import json
import os
import subprocess
from pathlib import Path

from rust_tools import fast_binary

ROOT = Path(__file__).resolve().parent.parent


OBJECTS = ("card", "object", "was", "now", "spell", "source", "token", "land", "attacker", "blocker", "ability")


class Refused(SystemExit):
    """The engine refused a request, saying why (`error`) and what kind of refusal it is
    (`class`: illegal, unsupported, malformed, unreferable or stale; `rules` cited by an
    illegal one). A failure of the engine (`fault`) says no class."""

    def __init__(self, request: dict, answer: dict):
        super().__init__(f"the engine refused {request}: {answer['error']}")
        self.answer = answer
        self.refusal_class: str | None = answer.get("class")
        self.rules: list[str] = answer.get("rules", [])


class Engine:
    """Explicit JSON-lines client. It never discovers or chooses game actions."""

    def __init__(self, audit: bool = False, executable: str | None = None, record: Path | None = None,
                 fixture_corpus: Path | None = None):
        self.record = record
        if record is not None:
            record.parent.mkdir(parents=True, exist_ok=True)
            if record.exists() or record.with_suffix(".checkpoints.jsonl").exists():
                raise FileExistsError(f"recording or checkpoints already exist: {record}")
            # Explicit recording creates a new baseline; never overwrite a saved game.
            with record.open("x"):
                pass
            with record.with_suffix(".checkpoints.jsonl").open("x"):
                pass
        command = [executable or os.environ.get("GAME_SERVE_REVIEW") or str(fast_binary("game-serve"))]
        if fixture_corpus is not None:
            command += ["--fixture-corpus", str(fixture_corpus)]
        self.proc = subprocess.Popen(command, stdin=subprocess.PIPE, stdout=subprocess.PIPE, text=True, bufsize=1, cwd=ROOT)
        ready = json.loads(self.proc.stdout.readline())
        assert ready["type"] == "ready"
        # which engine this is, as it says (`version`); None from one that says nothing
        self.version: dict | None = ready.get("version")
        self.state: dict = {}
        self.audit = audit
        self.disclosed: dict[int, set[int]] = {}

    def send(self, request: dict) -> dict:
        self.proc.stdin.write(json.dumps(request) + "\n")
        self.proc.stdin.flush()
        line = self.proc.stdout.readline()
        if not line:
            raise SystemExit(f"the engine stopped on {request}")
        answer = json.loads(line)
        if getattr(self, "record", None) is not None:
            saved = request.copy()
            if saved.get("cmd") == "start":
                from deck_check import ordered_main
                saved["decks"] = [ordered_main(ROOT / deck) if isinstance(deck, str) else deck
                                  for deck in saved["decks"]]
            with self.record.open("a") as trace:
                trace.write(json.dumps(saved) + "\n")
            from perf_replay import checkpoint
            with self.record.with_suffix(".checkpoints.jsonl").open("a") as checks:
                checks.write(json.dumps(checkpoint(answer)) + "\n")
        if not answer["ok"]:
            raise Refused(request, answer)
        # a player's prompt (`"cmd":"prompt"`/`"answer"`) is said as one, not as a state;
        # which engine this is (`"cmd":"version"`) and what a seat's labels name (`"labels"`)
        # as themselves
        for said in ("prompt", "version", "labels"):
            if said in answer:
                return answer[said]
        return answer["state"]

    def ask(self, request: dict) -> dict:
        if request.get("cmd") in ("start", "new"):
            self.disclosed.clear()
        if request.get("cmd") == "act":
            request = request | {"session_id": self.state["session_id"], "request_id": self.state["request_id"]}
        self.state = self.send(request | {"viewer": "asked"})
        if self.audit:
            self.check()
        return self.state

    def act(self, move: dict) -> dict:
        return self.ask({"cmd": "act", "player": self.state["asked"]["player"], "move": move})

    def check(self) -> None:
        """Reject undisclosed opponent-hand identities. Explicit disclosures permit their
        cards during a resolving choice. At priority only current stack-owned reveal grants
        permit an opponent's hand card (e.g. ninjutsu, 702.49b). Past disclosures are not grants.
        Fine-grained visibility within resolution is covered by engine observation tests.
        """
        view, viewer = self.state, self.state["viewer"]
        if viewer is None:
            return
        choosing = bool(view["asked"] and view["asked"]["kind"] != "priority")
        if not choosing:
            self.disclosed.clear()
        allowed = self.disclosed.setdefault(viewer, set())
        if choosing:
            allowed.update(card["object"] for shown in view.get("observations", []) for card in shown["cards"])
        # Do not remember these grants: each snapshot must still authorize the reveal.
        allowed = allowed | set(view.get("publicly_revealed", []))
        whole = self.send({"cmd": "state"})
        assert view["asked"]["player"] == viewer, view["asked"]
        # Every player sees a face-down permanent as a nameless 2/2; only its controller,
        # and a player sharing that controller's information, is shown its card (708.2, 708.5, 723).
        for i, seen in view["objects"].items():
            if "face_down" not in seen:
                continue
            controller = whole["objects"][i]["controller"]
            assert seen["name"] == "", f"player {viewer} is shown face-down object {i} by name: {seen['name']}"
            entitled = viewer in (controller, view["players"][controller].get("decision_maker", controller)) or view["players"][viewer].get("decision_maker", viewer) == controller
            card = seen["face_down"]["card"]
            if entitled:
                assert card == whole["objects"][i]["face_down"]["card"], f"player {viewer} is not shown its face-down {i}"
            elif int(i) not in allowed:
                # a face-down permanent an effect reveals is shown while it is revealed (701.20a)
                assert card is None, f"player {viewer} is shown the card of face-down {i} it does not control: {card}"
        # Every player sees a card exiled face down with no name; only a player who may look at
        # it through retained permission (406.3) or control of the permanent that hid it
        # (702.75a), and a player sharing that player's information, is shown its card.
        def shares(viewer, player):
            return viewer == player or view["players"][player].get("decision_maker", player) == viewer
        for i, seen in view["objects"].items():
            if "hidden" not in seen:
                continue
            assert seen["name"] == "", f"player {viewer} is shown hidden exiled card {i} by name: {seen['name']}"
            hidden = whole["objects"][i]["hidden"]
            watcher = whole["objects"].get(str(hidden.get("watcher")))
            entitled = any(shares(viewer, p) for p in hidden["lookers"]) or (
                watcher is not None and int(hidden["watcher"]) in whole["battlefield"] and shares(viewer, watcher["controller"]))
            card = seen["hidden"]["card"]
            if entitled:
                assert card == hidden["card"], f"player {viewer} is not shown hidden exiled card {i}"
            elif int(i) not in allowed:
                assert card is None, f"player {viewer} is shown hidden exiled card {i} it may not look at: {card}"
        for p, (seen, player) in enumerate(zip(view["players"], whole["players"])):
            assert seen.get("counters", {}) == player.get("counters", {}), f"player {p}: public counter maps differ"
            # A token or spell copy can remain here until the next state-based check
            # while the resolving program asks another question (111.6, 704.4).
            cards = sum(whole["objects"][str(o)]["is_card"] for o in player["hand"])
            assert seen["hand_size"] == cards, f"player {p}: hand size {seen['hand_size']} differs from {cards} cards"
            if p == viewer or seen.get("decision_maker",p) == viewer:
                assert seen["hand"] == player["hand"]
                continue
            assert "hand" not in seen, f"player {viewer} is shown the hand of player {p}"
            hidden = set(player["hand"]) - allowed
            shown = hidden & {int(i) for i in view["objects"]}
            assert not shown, f"player {viewer} is shown cards of the hand of player {p}: {shown}"
            for e in view["events"]:
                def object_id(value):
                    if isinstance(value, dict):
                        value = value.get("object")  # a typed player target is not an object
                    return value if isinstance(value, int) and not isinstance(value, bool) else None
                leaked = [k for k in OBJECTS if object_id(e.get(k)) in hidden]
                assert not leaked, f"player {viewer} is told a card of the hand of player {p}: {e}"


class Seat:
    """One player's prompt and explicit answers, using the engine's player labels."""

    def __init__(self, engine: Engine, seat: int):
        self.engine, self.label = engine, f"p{seat}"
        self.last: dict = {}

    def prompt(self) -> dict:
        self.last = self.engine.send({"cmd": "prompt", "seat": self.label})
        return self.last

    def answer(self, move: dict) -> dict:
        """Submit an explicit answer. Refusals do not pass or choose for the seat."""
        self.last = self.engine.send({"cmd": "answer", "seat": self.label, "session_id": self.last["session_id"],
                                      "request_id": self.last["request_id"], "answer": move})
        return self.last
