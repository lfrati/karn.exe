#!/usr/bin/env python3
"""The Karn adapter: makes `game-serve` play MGN records over the adapter protocol
(docs/mgn/adapter-protocol.md), one JSON message per line on standard input and output.

    uv run scripts/mgn/adapters/karn.py --engine target/fast/game-serve

The adapter keeps the mapping between the record's ids and the engine's objects (cards by the
engine's physical identity, tokens, stack objects and emblems numbered in creation order when
first reported at a boundary, spec 1.8), translates each recorded answer into the engine's move,
serves MGN `default` with explicit null actions (spec 5.6), asks the runner for
random outcomes ahead of every engine command and hands back the ones the engine did not use,
and projects the engine's privileged `state`, `audit_game` and `library_order` into the explicit
two-tier projection (spec 3, 7.3) through the same `State` the validator uses, so that the
runner compares like with like. It fails closed: a fact it cannot project is listed under
`unsupported`, a question or answer it cannot map is `unsupported_kind`, a position it cannot
construct is `unsupported_start`, and nothing is guessed.

An accepted illegal attempt is undone by replaying the adapter's journal into a fresh engine
process; the engine has no undo of its own.
"""
from __future__ import annotations

import argparse
from collections import Counter
from copy import deepcopy
import hashlib
import json
import os
from pathlib import Path
import re
import select
import signal
import subprocess
import sys
import time

ROOT = Path(__file__).resolve().parents[3]
if __name__ == "__main__" and str(ROOT / "scripts") not in sys.path:
    sys.path.insert(0, str(ROOT / "scripts"))

from mgn.canonical import canonical  # noqa: E402
from mgn.abilities import inherited_ability_ref, source_free_rule  # noqa: E402
from mgn.tokens import distinguish_instances, predefined_tokens  # noqa: E402
from mgn.oracle import Oracle  # noqa: E402
from mgn.randomness import matches_request  # noqa: E402
from mgn.payments import InvalidPayment, PaymentEvidenceError, select_payment, validate_bound_payment, validate_production, payment_nodes, scoped_quote, payment_ready, continuation_payment, continuation_selector  # noqa: E402
from mgn.keywords import TEXT_KEYWORDS, keyword_name, keyword_row, normalize_keyword  # noqa: E402
from mgn.state import State  # noqa: E402
from mgn.vocabulary import STEPS, spelled_keyword, canonical_subtype, native_subtype  # noqa: E402
import pinned  # noqa: E402

ADAPTER = {"name": "karn-mgn", "version": "1"}
RULES = pinned.RULES
CARDS = ROOT / "resources/oracle/cards.jsonl"

# the engine's parts (crates/ir/src/turn.rs) as MGN spells steps (spec 3.1); a combat damage
# step is relabelled first_strike_damage when it is the first of two (510.4)
PARTS = {"Untap": "untap", "Upkeep": "upkeep", "Draw": "draw", "PrecombatMain": "precombat_main",
         "BeginningOfCombat": "begin_combat", "DeclareAttackers": "declare_attackers",
         "DeclareBlockers": "declare_blockers", "CombatDamage": "combat_damage", "EndOfCombat": "end_combat",
         "PostcombatMain": "postcombat_main", "End": "end", "Cleanup": "cleanup"}
PART_OF_STEP = {step: part for part, step in PARTS.items()}
PART_OF_STEP["first_strike_damage"] = "CombatDamage"
STEP_INDEX = {step: i for i, step in enumerate(STEPS)}
ZONES = {"Library": "library", "Hand": "hand", "Graveyard": "graveyard", "Battlefield": "battlefield",
         "Stack": "stack", "Exile": "exile", "Command": "command"}
# the engine's question kinds (game-serve `render_asked`) as MGN pending kinds (spec 5)
KINDS = {"priority": "priority", "attackers": "declare_attackers", "blockers": "declare_blockers",
         "divide": "assign_combat_damage", "order": "order_triggers", "targets": "choose_targets",
         "modes": "choose_modes", "may": "optional", "objects": "choose_objects", "optional_objects": "choose_objects",
         "select": "choose_objects", "search": "search", "arrange": "arrange", "library_destination": "library_destination",
         "pay": "pay", "alternative": "choose_alternative", "discard": "discard", "mulligan": "mulligan",
         "bottom": "bottom", "player": "choose_player", "number": "choose_number", "value": "choose_value",
         "mana": "choose_mana", "holders": "holders", "attach": "attach", "defender": "choose_defender",
         "pile": "pile", "command_zone": "command_zone", "cast": "cast_during_resolution",
         "change_targets": "change_targets", "retarget": "change_targets", "skip": "skip",
         "replacement": "choose_replacement", "order_cards": "order_cards", "coin_call": "coin_call"}
# the answer kinds the adapter translates (spec 5.2 to 5.4); every MGN kind it can be asked
ANSWER_KINDS = {"play_land", "cast", "activate", "special_action", "declare_attackers", "declare_blockers",
                "assign_combat_damage", "order_triggers", "choose_targets", "choose_modes", "optional",
                "choose_objects", "order_cards", "search", "arrange", "library_destination", "choose_replacement", "pay",
                "choose_alternative", "discard", "mulligan", "bottom", "choose_player", "choose_number",
                "choose_value", "choose_mana", "holders", "attach", "choose_defender", "pile", "separate", "spell_destination",
                "skip", "command_zone", "cast_during_resolution", "change_targets", "concede",
                "choose_first", "companion", "opening_hand_action", "coin_call"}
# what a chosen value is of (spec 5.4), as the engine's `value` question spells its options
VALUE_KINDS = {"color": "color", "creature_type": "creature_type", "card_name": "card_name", "card_type": "card_type",
               "basic_land_type": "basic_land_type", "land_type": "land_type"}
# facts of the projection the engine does not model at all: declared once, never defaulted
UNSUPPORTED_FACTS = ["game.initiative", "game.day_night", "players.*.designations",
                     "objects.*.flipped", "objects.*.phased_out", "objects.*.in_combat"]
MANA_TYPES = ("W", "U", "B", "R", "G", "C")
# the steps of a turn after the precombat main phase that a position may be constructed in by
# playing the turn forward from that main phase with null answers
LATER_STEPS = ("begin_combat", "declare_attackers", "declare_blockers", "end_combat", "end", "cleanup")
POSITION_STEPS = ("upkeep", "draw", "precombat_main", "begin_combat", "end_combat", "postcombat_main", "end")
DESIGNATIONS = ("solved", "saddled", "prepared", "suspected", "harnessed")  # as the engine reports them
TOKEN_FIELDS = ("name", "types", "supertypes", "subtypes", "colors", "power", "toughness", "keywords", "text")


def sha256_file(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


class Refusal(Exception):
    """A refusal or setup error of the protocol: class, code, message and an optional path."""

    def __init__(self, class_: str, code: str, message: str, path: str | None = None):
        super().__init__(message)
        self.class_, self.code, self.message, self.path = class_, code, message, path

    def json(self) -> dict:
        out = {"class": self.class_, "code": self.code, "message": self.message}
        if self.path:
            out["path"] = self.path
        return out


def unsupported(code: str, message: str, path: str | None = None) -> Refusal:
    return Refusal("unsupported", code, message, path)


def fault(code: str, message: str) -> Refusal:
    return Refusal("fault", code, message)


# What the engine says a refusal is (`class`), as the protocol says it: a reference the engine
# cannot resolve is an illegal action's (`unknown_reference`); a request the engine cannot read,
# or one answering no pending question, is the adapter's own failure (`protocol`).
ENGINE_CLASSES = {"illegal": ("illegal", "illegal_action"), "unreferable": ("illegal", "unknown_reference"),
                  "unsupported": ("unsupported", "unsupported_kind"),
                  "malformed": ("fault", "protocol"), "stale": ("fault", "protocol")}


def engine_refusal(reply: dict, message: str | None = None) -> Refusal:
    """The protocol's refusal for an engine's `{"ok": false, ...}` reply. A reply naming no
    class is the engine's failure, not a refusal."""
    message = str(reply.get("error", "")) if message is None else message
    if reply.get("code") == "replay_input":
        return fault("random_mismatch", message)
    if reply.get("fault") is not None:
        return fault("engine_fault", f"{message}: {json.dumps(reply['fault'])[:400]}")
    class_, code = ENGINE_CLASSES.get(reply.get("class"), ("fault", "engine_fault"))
    return Refusal(class_, code, message)


def proof_refusal(proof: dict, message: str | None = None) -> Refusal:
    """The protocol's refusal for a quote whose quoted move the engine refused: its class is
    the `error_class` the proof says, as the same move's own refusal would say it."""
    return engine_refusal({"error": proof.get("error"), "class": proof.get("error_class")}, message)


def mana_choice_refusal(message: str) -> bool:
    return "offers no such choice of mana" in message or "offers a choice of mana:" in message


class Engine:
    """One `game-serve` process spoken to in JSON lines, with a deadline per request."""

    def __init__(self, path: Path, timeout: float = 30.0):
        self.path = Path(path)
        self.timeout = timeout
        self.buffer = bytearray()
        self.stderr_tail = bytearray()
        self.process = subprocess.Popen([str(self.path)], stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                                        stderr=subprocess.PIPE, start_new_session=True)
        os.set_blocking(self.process.stdout.fileno(), False)
        os.set_blocking(self.process.stderr.fileno(), False)
        self.stderr_open = True
        self.ready = self._read()
        self.version = self.ready.get("version", {})

    def _read(self) -> dict:
        deadline = time.monotonic() + self.timeout
        while b"\n" not in self.buffer:
            remaining = deadline - time.monotonic()
            if remaining <= 0:
                raise fault("timeout", self.diagnostic("the engine did not answer in time"))
            streams = [self.process.stdout] + ([self.process.stderr] if self.stderr_open else [])
            readable, _, _ = select.select(streams, [], [], remaining)
            if not readable:
                continue
            if self.process.stderr in readable:
                self._drain_stderr()
            if self.process.stdout not in readable:
                continue
            chunk = os.read(self.process.stdout.fileno(), 1 << 16)
            if not chunk:
                raise fault("engine_fault", self.diagnostic("the engine exited"))
            self.buffer.extend(chunk)
        line, _, rest = self.buffer.partition(b"\n")
        self.buffer = bytearray(rest)
        return json.loads(line)

    def _drain_stderr(self):
        for _ in range(16):
            if not self.stderr_open:
                break
            try:
                chunk = os.read(self.process.stderr.fileno(), 1 << 16)
            except BlockingIOError:
                break
            if not chunk:
                self.stderr_open = False
                break
            self.stderr_tail.extend(chunk)
            del self.stderr_tail[:-8192]

    def diagnostic(self, message: str) -> str:
        self._drain_stderr()
        tail = self.stderr_tail.decode('utf-8', errors='replace').strip()
        return f"{message}; engine stderr: {tail}" if tail else message

    def send(self, request: dict) -> dict:
        """A request and its reply, `{"ok": false, "error": ...}` included."""
        try:
            self.process.stdin.write((json.dumps(request) + "\n").encode())
            self.process.stdin.flush()
        except (BrokenPipeError, OSError) as error:
            raise fault("engine_fault", self.diagnostic(f"the engine pipe closed: {error}"))
        return self._read()

    def must(self, request: dict) -> dict:
        reply = self.send(request)
        if not reply.get("ok"):
            raise fault("engine_fault", self.diagnostic(f"{request.get('cmd')}: {reply.get('error')}; {reply.get('fault', '')}"))
        return reply

    def close(self):
        try:
            if self.process.stdin and not self.process.stdin.closed:
                self.process.stdin.close()
            self.process.wait(timeout=2)
        except (OSError, subprocess.TimeoutExpired):
            try:
                os.killpg(self.process.pid, signal.SIGKILL)
                self.process.wait(timeout=2)
            except (OSError, subprocess.TimeoutExpired):
                pass
        finally:
            if self.process.stdout:
                self.process.stdout.close()
            if self.process.stderr:
                self.process.stderr.close()


class KarnAdapter:
    """The adapter proper. `ask` callbacks hand random requests to the runner (or, during a
    restoration, to the journal)."""

    def __init__(self, engine_path: Path, oracle: Oracle | None = None, timeout: float = 30.0):
        self.engine_path = Path(engine_path)
        self.timeout = timeout
        self.oracle = oracle or Oracle(CARDS)
        self.engine: Engine | None = None
        self.setup_request: dict | None = None
        self.journal: list[dict] = []
        self.restoring = False
        self._reset_game_state()

    # ----- state kept per game -----

    def _reset_game_state(self):
        self.seats: list[str] = []
        self.cards: dict = {}
        self.tier = 2
        self.session_id = None
        self.physical: dict[str, int] = {}      # record card id -> engine physical identity
        self.by_physical: dict[int, str] = {}
        self.seen_physical: dict[int, int] = {}  # every engine object id seen -> its physical identity
        self.current: dict[str, int] = {}       # record id -> current engine object id
        self.causal_labels: dict[int, str] = {} # historical labels bind one incarnation only
        self.initial_causal: dict | None = None
        self.causal_refs: dict[int, str] = {}
        self.causal_handles: dict[str, int] = {}
        self.causal_verified = False
        self.imported_continuous: set[int] = set()
        self.imported_delayed: set[tuple] = set()
        self.ids_at_reply: dict[str, int] = {}  # the same at the last reply, for `renewed` (400.7)
        self.token_root: dict[int, int] = {}    # engine id -> the engine id the token was created with
        self.committed_tokens: dict[int, str] = {}
        self.departed_tokens: dict[int, dict] = {}
        self.committed_departed: dict[int, dict] = {}
        self.committed_stack: dict[int, str] = {}
        self.reserved_stack: set[str] = set()  # targeted triggers reported before their native stack object exists
        self.pending_stack: dict | None = None
        self.pending_mana_payment: dict | None = None
        self.last_payment_continuation: dict | None = None
        self.synthetic_stack: dict | None = None
        self.projection_history: State | None = None
        self.last_projection: State | None = None
        self.committed_emblems: dict[int, str] = {}
        self.tokens: dict[int, str] = {}
        self.stack_ids: dict[int, str] = {}
        self.emblems: dict[int, str] = {}
        self.turn_counts: dict[tuple, int] = {}
        self.steps_begun: dict[int, int] = {}       # turn -> steps and main phases begun in it
        self.ordinals: dict[tuple, int] = {}        # (turn, step, occurrence) -> its position in the turn
        self.uncertain_damage_turns: set[int] = set()
        self.turn_seen: int | None = None
        self.state: dict = {}
        self.audit: dict = {}
        self.libraries: list[list[int]] = []
        self.entered: list[int] = []           # permanents that entered in the last engine reply
        self.entering_card: str | None = None  # the land of the last land play, while the engine asks about it
        self.conceded: set[str] = set()         # seats that conceded (104.3a), from the engine's events
        self.life = 20
        self.format = "custom"
        self.pregame: dict | None = None        # before the engine's start: the stage, chooser, starting player, companions
        self.decks: dict[str, list[str]] = {}   # seat -> its main-deck cards in the card table's order
        self.copy_sources: dict[int, str] = {}  # engine id of a spell copy -> the record id of the spell it copies
        self.free_casts: set[int] = set()       # exact stack incarnations authenticated by a native Offered permission
        self.unsupported_paths: list[str] = []
        self.outcomes: list[dict] = []         # transition outcomes held for the next engine command
        self.used_outcomes: list[dict] = []    # outcomes handed to the engine and consumed
        self.prompt_cache: dict = {}

    # ----- protocol: hello -----

    def capabilities(self) -> dict:
        engine = self.engine or Engine(self.engine_path, self.timeout)
        version = engine.version
        if self.engine is None:
            engine.close()
        source = (version.get("source") or {}).get("revision") or "unknown"
        return {"mgn": "1", "engine": {"name": "karn", "version": f"{version.get('package', '?')}+{source[:8]}"},
                "adapter": dict(ADAPTER), "tiers": [1, 2], "starts": ["decks", "position"], "kinds": sorted(ANSWER_KINDS),
                "random": ["shuffle", "random_order", "coin", "starting_player"], "formats": ["standard", "commander", "custom"],
                "unsupported_facts": list(UNSUPPORTED_FACTS)}

    # ----- protocol: setup -----

    def setup(self, setup: dict) -> dict:
        self._reset_game_state()
        self.setup_request = deepcopy(setup)
        self.journal = []
        self.seats = list(setup["game"]["seats"])
        self.cards = deepcopy(setup["cards"])
        self.tier = setup["tier"]
        if len(self.seats) != 2:
            raise unsupported("unsupported_format", "the engine plays two-seat games", "game.seats")
        self.life = setup["game"]["life"]
        self.format = setup["game"].get("format", "custom")
        self._check_pins(setup)
        start = setup["start"]
        if start["kind"] == "decks":
            self._setup_decks(setup)
            self.journal.append({"op": "setup"})
            return self._reply_state()
        if self.format == "commander":
            raise unsupported("unsupported_start", "a Commander game is constructed from its decks only (903.6)", "start.kind")
        self._open_engine()
        if (start["state"]["pending"] or {}).get("kind") == "priority" and start["state"]["turn"]["step"] in POSITION_STEPS:
            self._construct_position(start, setup["game"])
        elif start["state"]["turn"]["step"] in LATER_STEPS and not start.get("causal"):
            self._construct_later(start, setup["game"])
        else:
            self._construct_position(start, setup["game"])
        self.journal.append({"op": "setup"})
        return self._reply_state()

    def _construct_position(self, start: dict, game: dict):
        """Install a settled scenario premise without playing setup actions."""
        native = self._native_position(start, game)
        # every rules-mandated question is asked, a forced one included (spec 5: a creature entering
        # attacking with one possible defender, 508.4)
        reply = self.engine.send({"cmd": "new", "position": native, "ask_forced": True})
        if not reply.get("ok"):
            raise self._setup_refusal(reply)
        self._bind_initial(reply["state"], start["state"])
        self._observe(reply["state"], initial=True)
        from mgn.causal import bind_and_verify
        bind_and_verify(self, start)
        turn = start["state"]["turn"]
        occurrence = turn.get("occurrence", 1)
        self.turn_counts = {(turn["number"], turn["step"]): occurrence}
        self.steps_begun = {turn["number"]: 1}
        self.ordinals = {(turn["number"], turn["step"], occurrence): 1}

    def _construct_later(self, start: dict, game: dict):
        """A position later in the turn than the precombat main phase (a combat declaration, a
        priority window of combat or the end step, the cleanup discard): the same objects are
        constructed in the precombat main phase and the turn is played forward with null answers,
        declaring the attackers the position shows (508.1), since spec 2.4 says nothing happened
        earlier in the turn beyond what the assumptions state. Any other question on the way means
        the position was not reached by null answers and is refused."""
        state = start["state"]
        turn, pending = state["turn"], state["pending"]
        if turn.get("occurrence", 1) != 1:
            raise unsupported("unsupported_start", "an additional step or phase cannot be constructed", "start.state.turn.occurrence")
        if pending is None:
            raise unsupported("unsupported_start", "a finished game cannot be constructed", "start.state.pending")
        main = deepcopy(start)
        main["state"]["turn"] = {"number": turn["number"], "step": "precombat_main", "active": turn["active"]}
        main["state"]["pending"] = {"seat": turn["active"], "kind": "priority"}
        attacks = []
        for object_id, facts in list(main["state"].get("objects", {}).items()):
            if facts.get("attacking") is not None:
                attacks.append([object_id, facts["attacking"]])
                facts.pop("tapped", None)  # the declaration taps it (508.1f): it is constructed untapped
            if facts.get("blocking"):
                raise unsupported("unsupported_start", "a position after blockers were declared cannot be constructed", f"start.state.objects.{object_id}.blocking")
            for fact in ("attacking", "blocked", "blocking", "in_combat"):
                facts.pop(fact, None)
            if not facts:
                del main["state"]["objects"][object_id]
        if attacks and (turn["step"] == "begin_combat" or (turn["step"] == "declare_attackers" and pending["kind"] == "declare_attackers")):
            raise unsupported("unsupported_start", "attackers before the declaration cannot be constructed", "start.state.objects")
        self._construct_position(main, game)
        target_kind = pending["kind"]
        for _ in range(64):
            asked = self.state.get("asked")
            here = self._turn()
            if asked is None or here["number"] != turn["number"]:
                raise unsupported("unsupported_start", "the position is not reached by null answers from the main phase", "start.state.turn")
            kind = KINDS.get(asked["kind"])
            if here["step"] == turn["step"] and kind == target_kind and self.seats[asked["player"]] == pending["seat"]:
                return
            if kind == "priority":
                self._send_move(asked["player"], {"kind": "pass"}, lambda key: None)
            elif kind == "declare_attackers":
                move = {"kind": "attack", "attacks": [[self._engine_id(a), self._target(d)] for a, d in attacks]}
                self._send_move(asked["player"], move, lambda key: None)
            elif kind == "declare_blockers" and not attacks:
                self._send_move(asked["player"], {"kind": "block", "blocks": []}, lambda key: None)
            else:
                raise unsupported("unsupported_start", f"the engine asks {kind} on the way to the position", "start.state.turn")
        raise unsupported("unsupported_start", "the position is not reached from the main phase", "start.state.turn")

    # ----- the pregame (103) -----

    def _setup_decks(self, setup: dict):
        """A `decks` start: nothing exists in the engine until the starting player is chosen
        (103.1) and the companions revealed (103.2b); until then the adapter answers from the
        record's own model of the pregame."""
        start = setup["start"]
        chooser = (setup.get("match") or {}).get("chooser")
        if chooser is None:
            starters = [o for o in start.get("random", []) if o.get("kind") == "starting_player"]
            if len(starters) != 1:
                raise unsupported("unsupported_start", "a decks start names its chooser in match.chooser or in one starting_player outcome", "start.random")
            chooser = starters[0]["result"]
        self.decks = {seat: [c for c, card in self.cards.items() if card["owner"] == seat and card.get("from", "main") == "main"]
                      for seat in self.seats}
        commanders = [c for c, card in self.cards.items() if card.get("from") == "command"]
        if self.format == "commander":
            for seat in self.seats:
                if sum(1 for c in commanders if self.cards[c]["owner"] == seat) != 1:
                    raise unsupported("unsupported_start", "the engine designates one commander per seat (903.3)", "cards")
            if self.life != 40:
                raise unsupported("unsupported_start", "a Commander game begins at 40 life (903.7)", "game.life")
        elif commanders:
            raise unsupported("unsupported_start", "a card in the command zone as the game begins needs the commander format (903.6)", "cards")
        self._open_engine()
        self.pregame = {"stage": "choose_first", "chooser": chooser, "first": None, "queue": [], "companions": {}}

    def _companion_candidates(self, seat: str) -> list[str]:
        """The seat's sideboard cards with companion (702.139a), which 103.2b lets it reveal."""
        out = []
        for card_id, card in self.cards.items():
            if card["owner"] != seat or card.get("from") != "sideboard":
                continue
            face = self.oracle.faces[card["oracle"]][0]
            if any(keyword.lower() == "companion" for keyword in face.raw.get("keywords", [])):
                out.append(card_id)
        return out

    def _turn_order(self, first: str) -> list[str]:
        index = self.seats.index(first)
        return self.seats[index:] + self.seats[:index]

    def _pregame_pending(self) -> dict | None:
        if self.pregame["stage"] == "ended":
            return None  # a concession ended the game before it began (104.3a)
        if self.pregame["stage"] == "choose_first":
            return {"seat": self.pregame["chooser"], "kind": "choose_first"}
        return {"seat": self.pregame["queue"][0], "kind": "companion"}

    def _pregame_state(self) -> dict:
        """The projection before the engine's game exists (spec 2.4): every main-deck card in its
        owner's library in the card table's order, which does not matter yet; a commander in the
        command zone (903.6)."""
        state = {"turn": {"number": 0, "step": "pregame", "active": None}, "pending": self._pregame_pending(),
                 "result": {"kind": "unfinished"}, "players": {seat: {"life": self.life} for seat in self.seats},
                 "zones": {}, "objects": {}}
        if self.pregame["stage"] == "ended":
            winner = next(seat for seat in self.seats if seat not in self.conceded)
            state["result"] = {"kind": "win", "winner": winner, "cause": "concession"}
            for seat in self.conceded:
                state["players"][seat]["left"] = True  # a player who concedes leaves the game (104.3a)
        for seat in self.seats:
            state["zones"][f"{seat}.library"] = list(self.decks[seat])
            state["zones"][f"{seat}.hand"] = []
            state["zones"][f"{seat}.graveyard"] = []
        for zone in ("battlefield", "stack", "exile", "command"):
            state["zones"][zone] = []
        state["zones"]["command"] = [c for c, card in self.cards.items() if card.get("from") == "command"]
        return state

    def _pregame_act(self, seat: str, do: dict, ask):
        saved = deepcopy(self.pregame)
        try:
            self._pregame_step(seat, do, ask)
        except Refusal:
            self.pregame = saved  # a refused answer changes nothing (an attempt relies on that)
            raise

    def _pregame_step(self, seat: str, do: dict, ask):
        kind = do["kind"]
        stage = self.pregame["stage"]
        if stage == "ended":
            raise Refusal("illegal", "illegal_action", "the game ended before it began (104.3a)")
        if kind == "concede":
            # a player can concede at any time (104.3a); before the engine's game exists the
            # adapter projects the end itself, for two seats (a game of more goes on, 800.4a)
            if len(self.seats) != 2:
                raise unsupported("unsupported_kind", "a concession before the game began is projected for two seats only")
            self.conceded.add(seat)
            self.pregame["stage"] = "ended"
            return
        if kind == "choose_first":
            if stage != "choose_first":
                raise Refusal("illegal", "illegal_action", "the starting player has been chosen (103.1)")
            if seat != self.pregame["chooser"]:
                raise Refusal("illegal", "illegal_action", f"{self.pregame['chooser']} chooses who plays first (103.1)")
            if do["first"] not in self.seats:
                raise Refusal("illegal", "unknown_reference", f"{do['first']} is not a seat")
            self.pregame["first"] = do["first"]
            # each seat that owns a card with companion outside the game reveals it or not, in
            # turn order from the starting player, before the shuffle (103.2b)
            self.pregame["queue"] = [s for s in self._turn_order(do["first"]) if self._companion_candidates(s)]
            self.pregame["stage"] = "companion"
        elif kind == "companion":
            if stage != "companion" or not self.pregame["queue"] or self.pregame["queue"][0] != seat:
                raise Refusal("illegal", "illegal_action", "no companion is being chosen for that seat (103.2b)")
            card = do.get("card")
            if card is not None and card not in self._companion_candidates(seat):
                raise Refusal("illegal", "unknown_reference", f"{card} is not a sideboard card of {seat} with companion (702.139a)")
            self.pregame["companions"][seat] = card
            self.pregame["queue"].pop(0)
        else:
            raise Refusal("illegal", "illegal_action", f"{kind} does not answer the pending {self._pregame_pending()['kind']}")
        if not self.pregame["queue"]:
            self._start_engine(ask)

    def _start_engine(self, ask):
        """The engine's `start`: the decks, the starting player (103.1), the companions revealed
        (103.2b) and the recorded order each initial shuffle produced (103.3), in place of a seed."""
        first = self.pregame["first"]
        decks, orders, outcomes = [], [], []
        for seat in self.seats:
            outcome = ask({"kind": "shuffle", "seat": seat})
            if outcome is None:
                raise fault("random_missing", f"the start shuffles {seat}'s library (103.3) and the record has no outcome for it")
            if outcome.get("kind") != "shuffle" or outcome.get("seat") != seat or outcome.get("pile") is not None:
                raise fault("random_mismatch", f"a shuffle of {seat}'s library was asked for")
            if (scope := outcome.get("scope")) is not None:
                if (not isinstance(scope, dict) or set(scope) - {"fixed", "excluding"}
                        or scope.get("fixed") or scope.get("excluding")):
                    raise fault("random_mismatch", "an initial shuffle includes the whole deck (103.3), without cards kept out")
            deck = self.decks[seat]
            if sorted(outcome.get("order", [])) != sorted(deck):
                raise fault("random_mismatch", f"the shuffle outcome of {seat} does not list that seat's deck once each")
            outcomes.append(outcome)
            decks.append([self._face_name(c) for c in deck])
            orders.append([deck.index(c) for c in outcome["order"]])
        request = {"cmd": "start", "decks": decks, "first": self._player(first), "orders": orders, "life": self.life, "ask_forced": True}
        companions = [self.pregame["companions"].get(seat) for seat in self.seats]
        if any(c is not None for c in companions):
            request["companions"] = [self._face_name(c) if c is not None else None for c in companions]
        if self.format == "commander":
            request["format"] = "commander"
            request["commanders"] = [self._face_name(self._commander_of(seat)) for seat in self.seats]
        reply = self.engine.send(request)
        if not reply.get("ok"):
            raise self._setup_refusal(reply)
        self.used_outcomes.extend(outcomes)
        self.pregame = None
        self._bind_decks(reply["state"])
        self.turn_seen = 0
        self.turn_counts, self.steps_begun, self.ordinals = {}, {}, {}
        self._observe(reply["state"], initial=True)

    def _commander_of(self, seat: str) -> str:
        return next(c for c, card in self.cards.items() if card["owner"] == seat and card.get("from") == "command")

    def _bind_decks(self, state: dict):
        """Binds the record's deck cards to the engine's physical identities: the engine creates a
        deck's cards in the order submitted, which is the card table's order, so the owner's cards
        in order of physical identity are the deck in that order; a commander is the owner's card
        in the command zone."""
        self.session_id = state["session_id"]
        audit = self.engine.must({"cmd": "audit_game", "session_id": self.session_id})["state"]
        for index, seat in enumerate(self.seats):
            owned = sorted((item["physical"], key) for key, item in audit["objects"].items()
                           if item["physical"] is not None and item["owner"] == index and item["zone"] in ("Library", "Hand"))
            deck = self.decks[seat]
            if len(owned) != len(deck):
                raise fault("engine_fault", f"the engine holds {len(owned)} cards of {seat}'s deck, the record has {len(deck)}")
            for (physical, key), record_id in zip(owned, deck):
                self._bind(record_id, physical, audit["objects"][key])
            if self.format == "commander":
                held = [(item["physical"], key) for key, item in audit["objects"].items()
                        if item["physical"] is not None and item["owner"] == index and item["zone"] == "Command"]
                if len(held) != 1:
                    raise fault("engine_fault", f"the engine holds {len(held)} commanders of {seat}")
                self._bind(self._commander_of(seat), held[0][0], audit["objects"][held[0][1]])

    def _bind(self, record_id: str, physical: int, item: dict):
        # the engine names a transforming or modal double-faced card by its front face and a split
        # or Room card by its whole name; either is the card of the record
        expected = {self.oracle.name(self.cards[record_id]["oracle"]), self._face_name(record_id)}
        if item["properties"]["name"] not in expected:
            raise fault("engine_fault", f"the engine holds {item['properties']['name']} where the record has {record_id} ({' or '.join(sorted(expected))})")
        if physical in self.by_physical or record_id in self.physical:
            raise fault("engine_fault", f"{record_id} or physical identity {physical} is bound twice")
        self.physical[record_id] = physical
        self.by_physical[physical] = record_id

    def _bind_newcomer(self, physical: int, item: dict):
        """A card that came into the game from outside it (400.11b: a companion put into its owner's
        hand, 702.139a) is bound to an unbound sideboard card of its owner with its name."""
        owner = self.seats[item["owner"]]
        name = item["properties"]["name"]
        for record_id, card in self.cards.items():
            if record_id in self.physical or card["owner"] != owner or card.get("from") != "sideboard":
                continue
            if name in (self.oracle.name(card["oracle"]), self._face_name(record_id)):
                self.physical[record_id] = physical
                self.by_physical[physical] = record_id
                return
        raise unsupported("unsupported_fact", f"the engine holds a {name} of {owner} that the record does not have", "objects")

    @staticmethod
    def _setup_refusal(reply: dict) -> Refusal:
        return engine_refusal(reply)

    def _face_name(self, record_id: str) -> str:
        """The name the engine knows a card by: its front face ("Hearth Elemental", not the Oracle
        data's "Hearth Elemental // Stoke Genius")."""
        return self.oracle.faces[self.cards[record_id]["oracle"]][0].name

    def _check_pins(self, setup: dict):
        pins = setup["pins"]
        rules = sha256_file(RULES)
        if pins["rules"]["sha256"] != rules:
            raise unsupported("pins_mismatch", f"the engine's rules are {rules}", "pins.rules")
        oracle = sha256_file(self.oracle.path)
        if pins["oracle"]["sha256"] != oracle:
            raise unsupported("pins_mismatch", f"the engine's Oracle data is {oracle}", "pins.oracle")
        for card_id, card in setup["cards"].items():
            if card["oracle"] not in self.oracle:
                raise unsupported("pins_mismatch", f"{card['oracle']} is not in the engine's Oracle data", f"cards.{card_id}")
            if "faces" in card and card["faces"] != self.oracle.digests(card["oracle"]):
                raise unsupported("pins_mismatch", "the faces differ from the engine's Oracle data", f"cards.{card_id}")

    def _native_position(self, start: dict, game: dict) -> dict:
        state = start["state"]
        assumptions = start.get("assumptions") or {}
        if state.get("departed"):
            raise unsupported("unsupported_start", "departed token premises cannot be constructed", "start.state.departed")
        turn, pending = state["turn"], state["pending"]
        if turn["step"] not in POSITION_STEPS:
            raise unsupported("unsupported_start", "this step has no supported scenario priority boundary", "start.state.turn.step")
        if turn.get("occurrence", 1) != 1:
            raise unsupported("unsupported_start", "an additional main phase cannot be constructed", "start.state.turn.occurrence")
        if pending is None or pending["kind"] != "priority":
            raise unsupported("unsupported_start", "the engine constructs positions at a priority window only", "start.state.pending")
        if state["result"]["kind"] != "unfinished":
            raise unsupported("unsupported_start", "a finished game cannot be constructed", "start.state.result")
        for seat in self.seats:
            facts = state["players"].get(seat, {})
            for fact, value in facts.items():
                if fact == "life":
                    continue
                if fact == "mana" and all(k in MANA_TYPES for k in value):
                    continue
                if fact in ("left", "counters", "mana_restricted", "designations") and not value:
                    continue
                if fact == "max_hand_size" and value == 7 or fact == "speed" and value is None:
                    continue
                raise unsupported("unsupported_start", f"{fact} cannot be constructed", f"start.state.players.{seat}.{fact}")
        for fact, value in state.get("game", {}).items():
            if value is not None:
                raise unsupported("unsupported_start", f"{fact} cannot be constructed", f"start.state.game.{fact}")
        if state["zones"].get("stack") or state["zones"].get("command"):
            raise unsupported("unsupported_start", "a non-empty stack or command zone cannot be constructed", "start.state.zones")
        constructible = {"controller", "tapped", "counters", "damage"}
        derived = {"power", "toughness", "colors", "types", "subtypes", "supertypes", "keywords"}
        objects = {}
        zone_of = {}
        for zone, members in state["zones"].items():
            for member in members:
                zone_of[member] = zone
        for object_id, zone in zone_of.items():
            if object_id not in self.cards:
                raise unsupported("unsupported_start", "tokens, emblems and stack objects cannot be constructed", f"start.state.objects.{object_id}")
            facts = state.get("objects", {}).get(object_id, {})
            for fact, value in facts.items():
                if fact in constructible or fact in derived:
                    continue
                if fact in ("blocking", "designations", "granted", "chosen", "modes", "targets") and not value:
                    continue
                if fact in ("attached_to", "attacking", "copy_of") and value is None:
                    continue
                if fact in ("blocked", "face_down", "flipped", "phased_out", "face") and not value:
                    continue
                raise unsupported("unsupported_start", f"{fact} cannot be constructed", f"start.state.objects.{object_id}.{fact}")
            card = self.cards[object_id]
            objects[object_id] = {"name": self._face_name(object_id), "owner": card["owner"],
                                  "controller": facts.get("controller", card["owner"]), "tapped": bool(facts.get("tapped", False)),
                                  "counters": dict(facts.get("counters", {})), "damage": facts.get("damage", 0)}
            if zone != "battlefield" and (objects[object_id]["tapped"] or objects[object_id]["counters"] or objects[object_id]["damage"]):
                raise unsupported("unsupported_start", "tapped, counters and damage are constructed on the battlefield only", f"start.state.objects.{object_id}")
        for card_id in self.cards:
            if card_id not in zone_of and self.cards[card_id].get("from", "main") == "main":
                raise unsupported("unsupported_start", f"card {card_id} is in no zone", "start.state.zones")
        land_played = assumptions.get("land_played", {})
        for seat, count in land_played.items():
            if count > 1:
                raise unsupported("unsupported_start", "more than one land played this turn cannot be constructed", "start.assumptions.land_played")
        if any(assumptions.get("spells_cast", {}).values()) and not start.get("causal", {}).get("casts_this_turn"):
            raise unsupported("unsupported_start", "spells cast this turn cannot be constructed", "start.assumptions.spells_cast")
        sick = set(assumptions.get("summoning_sick", []))
        control_since = {object_id: ("this_turn" if object_id in sick else "before_turn")
                         for object_id, zone in zone_of.items() if zone == "battlefield"}
        players = {}
        for seat in self.seats:
            facts = state["players"].get(seat, {})
            mana = {k: v for k, v in facts.get("mana", {}).items() if v}
            players[seat] = {"life": facts.get("life", game["life"]), "mana": mana}
        zones = {zone: list(state["zones"].get(zone, [])) for zone in
                 [f"{seat}.{z}" for seat in self.seats for z in ("library", "hand", "graveyard")] + ["battlefield", "stack", "exile", "command"]}
        history = {"turn_structure": "ordinary_A_first", "pending": "none", "effects": "intrinsic_only", "turn_events": "empty",
                   "previous_turn_spells": 0, "land_plays_this_turn": {seat: land_played.get(seat, 0) for seat in self.seats},
                   "ability_uses": "none", "consecutive_passes": assumptions.get("consecutive_passes", 0), "control_since": control_since}
        native = {"timing": {"turn": turn["number"], "step": turn["step"], "active": turn["active"]},
                "next": {"kind": "priority", "chooser": pending["seat"]}, "players": players, "zones": zones, "objects": objects,
                "history": history, "causal": {"linked_exile": [], "prevention": [], "continuous_effects": [], "base_controllers": {}}}
        from mgn.causal import native_causal
        native_causal(self, start, native)
        return native

    def _open_engine(self):
        if self.engine is not None:
            self.engine.close()
        self.engine = Engine(self.engine_path, self.timeout)

    def _bind_initial(self, state: dict, given: dict):
        """Binds the record's card ids to the engine's physical identities zone by zone."""
        self.session_id = state["session_id"]
        audit = self.engine.must({"cmd": "audit_game", "session_id": self.session_id})["state"]
        libraries = [self.engine.must({"cmd": "library_order", "session_id": self.session_id, "player": p})["state"]["order"]
                     for p in range(len(self.seats))]
        listings = {}
        for index, seat in enumerate(self.seats):
            listings[f"{seat}.library"] = list(libraries[index])
            listings[f"{seat}.hand"] = list(state["players"][index]["hand"])
            listings[f"{seat}.graveyard"] = list(state["players"][index]["graveyard"])
        for zone in ("battlefield", "exile", "command", "stack"):
            listings[zone] = list(state[zone])
        for zone, members in given["zones"].items():
            native = listings.get(zone, [])
            if len(native) != len(members):
                raise fault("engine_fault", f"the engine put {len(native)} objects in {zone}, the position has {len(members)}")
            for engine_id, record_id in zip(native, members):
                item = audit["objects"][str(engine_id)]
                # the engine names a transforming or modal double-faced card by its front face and a
                # split or Room card by its whole name; either is the card of the record
                expected = {self.oracle.name(self.cards[record_id]["oracle"]), self._face_name(record_id)}
                if item["properties"]["name"] not in expected:
                    raise fault("engine_fault", f"{zone}: the engine holds {item['properties']['name']} where the position has {record_id} ({' or '.join(sorted(expected))})")
                physical = item["physical"]
                if physical is None or physical in self.by_physical:
                    raise fault("engine_fault", f"no physical identity for {record_id}")
                self.physical[record_id] = physical
                self.by_physical[physical] = record_id

    # ----- protocol: act, attempt, default, project, end -----

    def act(self, n: int, seat: str, do: dict, ask) -> dict:
        self._require_game()
        self.used_outcomes = []
        if self.pregame is not None:
            self._pregame_act(seat, do, ask)
            self.journal.append({"op": "act", "n": n, "seat": seat, "do": deepcopy(do), "outcomes": list(self.used_outcomes)})
            return self._reply_state(with_unused=True)
        self._commit_generated()
        move = self._move(do)
        player = self._player(seat)
        self._send_move(player, move, ask, do=do)
        self.journal.append({"op": "act", "n": n, "seat": seat, "do": deepcopy(do), "outcomes": list(self.used_outcomes)})
        return self._reply_state(with_unused=True)

    def attempt(self, n: int, seat: str, do: dict, ask) -> dict:
        self._require_game()
        self.used_outcomes = []
        journal_before = list(self.journal)
        if self.pregame is not None:
            self._pregame_act(seat, do, ask)  # refused with class illegal when the record is right
            self.journal = journal_before
            self._restore()
            out = self._reply_state(with_unused=True)
            out["restored"] = True
            return out
        self._commit_generated()
        move = self._move(do)
        player = self._player(seat)
        try:
            self._send_move(player, move, ask, do=do)
        except Refusal as refusal:
            if refusal.class_ == "illegal":
                self._give_back_outcomes()
                raise
            raise
        # the engine accepted what the record marks illegal: undo it by replaying the journal
        self.journal = journal_before
        self._restore()
        out = self._reply_state(with_unused=True)
        out["restored"] = True
        return out

    def default(self, until: dict | None, ask) -> dict:
        self._require_game()
        self.used_outcomes = []
        if self.pregame is not None:
            asked_kind = None
            if self.pregame["stage"] == "choose_first":
                asked_kind = "choose_first"  # no null answer (spec 5.6)
            elif self.pregame["stage"] == "companion":
                self._pregame_act(self.pregame["queue"][0], {"kind": "companion", "card": None}, ask)  # none revealed (103.2b)
            self.journal.append({"op": "default", "until": deepcopy(until), "outcomes": list(self.used_outcomes)})
            out = self._reply_state(with_unused=True)
            if asked_kind is not None:
                out["asked"] = asked_kind
            return out
        stack_before = list(self.state.get("stack", []))
        given = 0
        asked_kind = None
        while True:
            asked = self.state.get("asked")
            if asked is None:
                break
            mgn_kind = KINDS.get(asked["kind"])
            if given and (mgn_kind != "priority" or until is None
                          or self._new_on_stack(stack_before) or self._at_or_past(until)):
                break
            move = self._null_move(asked)
            if move is None:
                asked_kind = self._asked_kind(asked)
                break
            try:
                self._send_move(asked["player"], move, ask)
            except Refusal as refusal:
                if refusal.class_ != "illegal":
                    raise
                # the null answer is illegal here (a block requirement, 509.1c; a cast an effect
                # instructs rather than allows, 608.2g): the question has no null answer at this
                # point (spec 5.6), and the engine, which refused it without changing anything,
                # is the judge of that
                asked_kind = self._asked_kind(asked)
                break
            given += 1
        self.journal.append({"op": "default", "until": deepcopy(until), "outcomes": list(self.used_outcomes)})
        out = self._reply_state(with_unused=True)
        asked = self.state.get("asked")
        if asked_kind is None and asked is not None and self._null_move(asked) is None:
            asked_kind = self._asked_kind(asked)  # the stretch stopped at a question with no null answer
        if asked_kind is not None:
            out["asked"] = asked_kind
        return out

    def project(self, tier: int) -> dict:
        self._require_game()
        saved = self.tier
        self.tier = tier
        try:
            return self._reply_state()
        finally:
            self.tier = saved

    def end(self):
        if self.engine is not None:
            self.engine.close()
            self.engine = None
        self._reset_game_state()

    def _require_game(self):
        if self.engine is None or (self.session_id is None and self.pregame is None):
            raise fault("protocol", "no game is set up")

    # ----- random outcomes -----

    def _hold_outcomes(self, ask) -> bool:
        """Prefetch every supported outcome in this transition, including repeated shuffles.

        One native response can resolve more than one random operation before yielding.
        The engine authenticates each operation and reports the exact unused inputs.
        """
        if not self.outcomes:
            keys = [{"kind": "shuffle", "seat": seat} for seat in self.seats] + [{"kind": "random_order"}]
            # A called flip cannot consume randomness before its call (705.2).
            if (self.state.get("asked") or {}).get("kind") == "coin_call":
                keys.append({"kind": "coin"})
            for key in keys:
                while (outcome := ask(key)) is not None:
                    if not matches_request(outcome, key):
                        raise fault("random_mismatch", f"asked for {key}, received {outcome}")
                    self.outcomes.append(outcome)
        return bool(self.outcomes)

    def _give_back_outcomes(self):
        self.unused_now = list(self.outcomes)
        self.outcomes = []

    def _random_id(self, card: str) -> int:
        try:
            return self._engine_id(card)
        except Refusal as refusal:
            if refusal.class_ == "illegal" and refusal.code == "unknown_reference":
                raise fault("random_mismatch", refusal.message) from refusal
            raise

    def _engine_random_outcomes(self, move: dict | None = None) -> list[dict]:
        """The held outcomes in the engine's form: the recorded order as current object ids. The
        engine keeps the cards a search set aside (701.24b) at their positions when the order leaves
        them out, as a record written after the search does."""
        orders = []
        checked_coin = False
        for outcome in self.outcomes:
            if outcome["kind"] == "coin":
                asked = self.state.get("asked") or {}
                if asked.get("kind") != "coin_call":
                    raise unsupported("unsupported_random", "a coin receipt needs the engine's pending coin call", "random")
                caller = outcome.get("caller", self.seats[asked["player"]])
                player = self._player(caller)
                face = outcome.get("result")
                if face not in ("heads", "tails"):
                    raise fault("random_mismatch", "a coin result is heads or tails")
                # Authenticate only the next occurrence for this caller. Extra receipts
                # are returned by the native consumption indices and checked when used.
                if player == asked["player"] and not checked_coin:
                    answer = (move or {}).get("answer")
                    call = answer.get("call") if isinstance(answer, dict) else None
                    if call in ("heads", "tails"):
                        if "call" in outcome and outcome["call"] != call:
                            raise fault("random_mismatch", "coin receipt call differs from the player's call")
                        winner = caller if call == face else None
                        if "winner" in outcome and outcome["winner"] != winner:
                            raise fault("random_mismatch", "coin receipt winner does not follow from the call")
                    checked_coin = True
                orders.append({"kind":"coin", "player":player, "result":face})
                continue
            if outcome.get("pile") is not None:
                raise unsupported("unsupported_random", "shuffles of piles are not consumed", "random")
            native = {"kind": outcome["kind"], "order": [self._random_id(card) for card in outcome["order"]]}
            if outcome["kind"] == "shuffle":
                native["player"] = self._player(outcome["seat"])
                if (scope := outcome.get("scope")) is not None:
                    if not isinstance(scope, dict) or set(scope) - {"fixed", "excluding"}:
                        raise fault("random_mismatch", "shuffle scope names only fixed and excluding cards")
                    native["scope"] = {}
                    if "excluding" in scope:
                        native["scope"]["excluding"] = [self._random_id(card) for card in scope["excluding"]]
                    if "fixed" in scope:
                        native["scope"]["fixed"] = [[self._random_id(card), deepcopy(position)] for card, position in scope["fixed"]]
            elif (outcome["kind"] != "random_order" or len(set(outcome["order"])) != len(outcome["order"])
                  or set(outcome["order"]) != set(outcome.get("objects", []))):
                raise fault("random_mismatch", "random_order must be a permutation of its objects")
            orders.append(native)
        return orders

    # ----- sending moves -----

    def _send_move(self, player: int, move: dict, ask, do: dict | None = None):
        free_cast_physical = None
        if move.get("kind") == "cast" and (self.state.get("asked") or {}).get("kind") == "cast":
            item = self.audit["objects"][str(move["card"])]
            offers = item.get("offered_casts")
            if offers is None:
                raise unsupported("unsupported_fact", "the engine does not expose the offered casting cost", "objects.*.cast.cost")
            actor = (self.state.get("asked") or {}).get("acting", player)
            selected = [offer for offer in offers if offer["player"] == actor
                        and offer["cast_as"] == move.get("cast_as", "whole")]
            free = any(offer["without_paying"] for offer in selected) and "cost" not in move
            claimed_free = ((do or {}).get("cost") or {}).get("effect") == "without_paying"
            if claimed_free and not free:
                raise Refusal("illegal", "illegal_cost", "this card, player and face have no offered permission to cast without paying its mana cost")
            if free:
                free_cast_physical = item.get("physical")
        proof = None
        continuation = None
        if do is not None and do.get("kind") == "choose_mana" and self.pending_mana_payment is not None:
            move, proof, continuation = self._continue_payment(player, move)
        if (do is not None and do.get("kind") in ("cast", "cast_during_resolution", "activate", "special_action", "pay")
                and not do.get("decline")):
            move, proof = self._explicit_payment(player, move, {**do, "pay": do.get("pay", {})})
        origin_frame = ({"state":deepcopy(self.state), "audit":deepcopy(self.audit)}
                        if proof is not None and (proof.get("suspended") or {}).get("stage") == "mana_choice" else None)
        self._hold_outcomes(ask)
        request = {"cmd": "act", "session_id": self.session_id, "request_id": self.state["request_id"],
                   "player": player, "move": move, "random_outcomes": self._engine_random_outcomes(move)}
        if proof is not None:
            request["payment_expect"] = proof
        if self.outcomes:
            request["random_outcomes_keep_unused"] = True
        reply = self.engine.send(request)
        if not reply.get("ok") and do is not None and proof is None and mana_choice_refusal(str(reply.get("error", ""))):
            # the engine says whether a mana ability offers a choice better than the text does: the
            # mana the record names is said only where the engine asks for it; a refused request
            # changes nothing, so the other form is tried once
            request["move"] = self._with_adding(move, do, "offers a choice of mana" in str(reply["error"]))
            reply = self.engine.send(request)
        if not reply.get("ok"):
            refusal = engine_refusal(reply)
            if refusal.code == "engine_fault":
                refusal.message = self.engine.diagnostic(refusal.message)
            raise refusal
        if do is not None and do.get("kind") == "play_land":
            self.entering_card = do["card"]
        elif do is not None and do.get("kind") in ("cast", "activate", "special_action", "concede"):
            self.entering_card = None  # the questions asked as the land entered are over
        state = reply["state"]
        if self.outcomes and "random_outcomes_unused_indices" not in state:
            raise fault("engine_fault", "missing unused random outcome receipt")
        unused = state.get("random_outcomes_unused_indices", [])
        if (not isinstance(unused, list) or any(type(i) is not int or i < 0 or i >= len(self.outcomes) for i in unused)
                or unused != sorted(set(unused))):
            raise fault("engine_fault", "invalid unused random outcome receipt")
        retained = set(unused)
        self.used_outcomes.extend(outcome for i, outcome in enumerate(self.outcomes) if i not in retained)
        self.outcomes = [self.outcomes[i] for i in unused]
        self._observe(state)
        self.last_payment_continuation = continuation
        if continuation is not None:
            self.pending_mana_payment = None
        if proof is not None and (proof.get("suspended") or {}).get("stage") == "mana_choice":
            if continuation is None:
                self.pending_mana_payment = {"pay":deepcopy(do.get("pay", {})), "quote":deepcopy(proof),
                    "before":origin_frame, "history":[{"quote":deepcopy(proof), "after":{"state":deepcopy(self.state), "audit":deepcopy(self.audit)}}]}
            else:
                self.pending_mana_payment = {"pay":deepcopy(continuation["pay"]),
                    "quote":continuation_payment(continuation["prefix"], proof), "before":deepcopy(continuation["before"]),
                    "history":[*deepcopy(continuation["history"]), {"quote":deepcopy(proof),
                        "after":{"state":deepcopy(self.state), "audit":deepcopy(self.audit)}}]}
        if free_cast_physical is not None:
            self.free_casts.update(native for native in state.get("stack", [])
                                   if self.audit["objects"].get(str(native), {}).get("physical") == free_cast_physical)

    def _continue_payment(self, player, move):
        retained = self.pending_mana_payment
        prefix = retained["quote"]
        boundary = prefix["suspended"]
        asked = {k:v for k,v in (self.state.get("asked") or {}).items() if k != "request_id"}
        if asked != boundary["asked"]:
            raise unsupported("unsupported_kind", "pending payment no longer names its actual mana-choice boundary", "do")
        def quote(command):
            result = self.engine.send({"cmd":"quote_payment", "session_id":self.session_id,
                "request_id":self.state["request_id"], "player":player, "move":command})
            if not result.get("ok"):
                raise engine_refusal(result, str(result.get("error", "mana answer quote failed")))
            proof = result["state"]
            if proof.get("error") is not None:
                raise proof_refusal(proof)
            return proof
        try:
            dry = quote(move)
            joined = continuation_payment(prefix, dry)
            capture = {"pay":deepcopy(retained["pay"]), "prefix":deepcopy(prefix),
                "before":deepcopy(retained["before"]), "history":deepcopy(retained["history"])}
            if (joined.get("suspended") or {}).get("stage") == "mana_choice":
                production = validate_production(retained["pay"], joined)
                validate_bound_payment(joined, joined["move"], boundary["player"], production)
                capture["answer"] = deepcopy(dry)
                return move, dry, capture
            selected = select_payment(retained["pay"], prefix["prior_pools"][boundary["player"]], joined,
                                      restricted_lot=self._restricted_lot, record_id=self._record_id)
            command = deepcopy(move)
            command["pending_payment"] = {"source":boundary["source"], "path":[],
                "mana":continuation_selector(selected, prefix["prior_pools"][boundary["player"]], dry["payments"][-1]["pool"])}
            proof = quote(command)
            completed = continuation_payment(prefix, proof)
            completed["move"]["mana"] = selected
            production = validate_production(retained["pay"], completed)
            validate_bound_payment(completed, completed["move"], boundary["player"], production)
            capture["answer"] = deepcopy(proof)
            return command, proof, capture
        except InvalidPayment as error:
            raise Refusal("illegal", "illegal_cost", str(error), "do") from error
        except PaymentEvidenceError as error:
            raise unsupported("unsupported_kind", str(error), "do") from error

    def _explicit_payment(self, player: int, move: dict, do: dict):
        """Bind the recorded spending to the engine's locked cost (601.2f–h).

        Quotes stop at unpaid locked boundaries on disposable clones. Each
        child and outer selection comes from the frozen MGN payment before its
        cost can execute; no quote allocates floating mana for the caller.
        """
        move = deepcopy(move)
        payment = move["answer"]["pay"] if do["kind"] == "pay" else move
        payment.pop("mana", None)

        activations = {(): do} if do.get("kind") == "activate" else {}
        def activation_tree(component, path=()):
            for i, ability in enumerate(component.get("abilities", [])):
                child = (*path, i)
                activations[child] = ability
                activation_tree(ability.get("pay", {}), child)
        activation_tree(do.get("pay", {}))
        def quote(command):
            seen = set()
            while True:
                reply = self.engine.send({"cmd": "quote_payment", "session_id": self.session_id,
                                          "request_id": self.state["request_id"], "player": player, "move": command})
                if not reply.get("ok"):
                    raise engine_refusal(reply, str(reply.get("error", "payment quote failed")))
                result = reply["state"]
                problems = result.get("mana_answers", [])
                direct_missing = any(p.get("problem") == "missing" and p.get("triggered") is False for p in problems)
                if not mana_choice_refusal(str(result.get("error", ""))) and not direct_missing:
                    return result
                if not problems:
                    raise unsupported("unsupported_kind", "mana-answer refusal lacks its owning activation scope", "do.pay")
                problem = problems[-1]
                path = tuple(problem.get("path") or ())
                node = dict(payment_nodes(command)).get(path)
                if (problem.get("triggered") is not False or path not in activations or node is None
                        or problem.get("source") != node.get("source")):
                    raise unsupported("unsupported_kind", "mana-answer refusal does not name an authored activation", "do.pay")
                marker = (path, problem.get("problem"), json.dumps(node.get("adding")))
                if marker in seen:
                    raise unsupported("unsupported_kind", "authored mana output does not determine the required production instruction choices", "do.pay")
                seen.add(marker)
                if problem["problem"] == "unoffered" and node.get("adding"):
                    node.pop("adding")
                elif problem["problem"] == "missing" and activations[path].get("adding"):
                    node["adding"] = [list(activations[path]["adding"])]
                else:
                    raise unsupported("unsupported_kind", "authored mana output does not supply this scoped choice", "do.pay")

        initial = quote(move)
        if initial.get("error") and not initial.get("payments"):
            raise proof_refusal(initial)
        asked = self.state.get("asked") or {}
        actor = asked.get("acting", player)
        try:
            # An unpaid child stops the quote before its activation produces
            # anything. Bind that exact recorded child payment and resume the
            # same announcement; never try alternate pools or source plans.
            nodes = dict(payment_nodes(move))
            authored = {}
            def walk(component, path=()):
                authored[path] = component
                for i, ability in enumerate(component.get("abilities", [])):
                    walk(ability.get("pay", {}), (*path, i))
            walk(do["pay"])
            if set(nodes) != set(authored):
                raise PaymentEvidenceError("native payment scopes disagree with the MGN activation tree")
            for _ in range(len(nodes) + 1):
                receipts = initial.get("payments", [])
                if not receipts:
                    raise PaymentEvidenceError("quote lacks a scoped locked payment boundary")
                bound = False
                seen = set()
                for receipt in receipts:
                    raw_path = receipt.get("path")
                    if not isinstance(raw_path, list) or any(type(i) is not int or i < 0 for i in raw_path):
                        raise PaymentEvidenceError("quote lacks an explicit payment execution scope")
                    path = tuple(raw_path)
                    if path not in nodes or path in seen:
                        raise PaymentEvidenceError("quote names a duplicate or unauthored payment scope")
                    seen.add(path)
                    node = nodes[path]
                    if receipt.get("player") != actor or (path and (receipt.get("kind") != "ability" or receipt.get("source") != node["source"])):
                        raise PaymentEvidenceError("payment receipt names another activation or payer")
                    if "mana" in node:
                        if receipt.get("left") is None:
                            raise InvalidPayment("explicit nested payment did not pay its locked cost")
                        continue
                    scope = scoped_quote(initial, path)
                    node["mana"] = select_payment(authored[path], receipt["prior_pool"], scope,
                                                   restricted_lot=self._restricted_lot, record_id=self._record_id)
                    bound = True
                if () in seen:
                    break
                if (initial.get("suspended") or {}).get("stage") == "mana_choice":
                    for path, node in nodes.items():
                        if path and path not in seen and "mana" not in node:
                            if authored[path]:
                                raise PaymentEvidenceError("future nonempty child payment needs its exact post-choice cost boundary")
                            node["mana"] = {}
                            bound = True
                    if bound:
                        initial = quote(move)
                    production = validate_production(do["pay"], initial)
                    validate_bound_payment(initial, move, actor, production)
                    return move, initial
                if not bound:
                    raise PaymentEvidenceError("quote did not advance to the next unpaid payment scope")
                initial = quote(move)
            else:
                raise PaymentEvidenceError("quote did not reach the outer locked payment boundary")
            # A refused isolated announcement rolls its event log back. The
            # retained locked receipts suffice to bind selections; authenticate
            # all production only on the final fully paid quote below.
            payment["mana"] = select_payment(do["pay"], initial["prior_pools"][actor], initial,
                                             restricted_lot=self._restricted_lot, record_id=self._record_id)
        except InvalidPayment as error:
            raise Refusal("illegal", "illegal_cost", str(error), "do.pay") from error
        except PaymentEvidenceError as error:
            raise unsupported("unsupported_kind", str(error), "do.pay") from error
        proof = quote(move)
        try:
            if not payment_ready(proof) and proof.get("error"):
                raise proof_refusal(proof)
            if not payment_ready(proof):
                raise Refusal("illegal", "illegal_action", "payment did not complete the announcement")
            production = validate_production(do["pay"], proof)
            selected = select_payment(do["pay"], proof["prior_pools"][actor], proof,
                                      restricted_lot=self._restricted_lot, record_id=self._record_id)
            if selected != payment["mana"]:
                raise PaymentEvidenceError("final quote changes the explicit payment selection")
            validate_bound_payment(proof, move, actor, production)
        except PaymentEvidenceError as error:
            raise unsupported("unsupported_kind", str(error), "do.pay") from error
        if do["kind"] == "activate" and Counter(production["main"]) != Counter(do.get("adding", [])):
            raise Refusal("illegal", "illegal_action", "the activated ability's mana output differs from adding")
        return move, proof

    def _with_adding(self, move: dict, do: dict, say: bool) -> dict:
        """The move with the mana the record says each mana ability added (`say`), or without it."""
        move = deepcopy(move)
        if do.get("kind") == "activate":
            if say and do.get("adding"):
                move["adding"] = [list(do["adding"])]
            else:
                move.pop("adding", None)
        steps = (do.get("pay") or {}).get("abilities", [])
        payment = move.get("answer", {}).get("pay", {}) if do.get("kind") == "pay" else move
        def apply(native, authored):
            for entry, step in zip(native.get("plan", []), authored):
                entry["adding"] = [list(step["adding"])] if say and step.get("adding") else []
                apply(entry, step.get("pay", {}).get("abilities", []))
        apply(payment, steps)
        return move

    def _new_on_stack(self, before: list) -> bool:
        return any(o not in before for o in self.state.get("stack", []))

    def _at_or_past(self, until: dict) -> bool:
        """At the target: its turn, step, occurrence and the seat asked; past it: a later turn, or
        a step of the same turn begun after the target's (steps are ordered by the position they
        began at, since an added phase or step breaks the order of 500.1; 500.8, 500.9)."""
        turn = self._turn()
        number = turn["number"]
        pair = (turn["step"], turn.get("occurrence", 1))
        target = (until["step"], until.get("occurrence", 1))
        asked = self.state.get("asked")
        if number == until["turn"] and pair == target:
            return asked is not None and self.seats[asked["player"]] == until["seat"]
        return self._past_step(until)

    def _past_step(self, until: dict) -> bool:
        """Strictly beyond the requested turn/step, regardless of priority seat."""
        turn = self._turn()
        number = turn["number"]
        if number != until["turn"]:
            return number > until["turn"]
        pair = (turn["step"], turn.get("occurrence", 1))
        target = (until["step"], until.get("occurrence", 1))
        if pair == target:
            return False
        begun = self._ordinal(number, *target)
        return begun is not None and self._ordinal(number, *pair) > begun

    # ----- the engine's moves -----

    def _player(self, seat: str) -> int:
        if seat not in self.seats:
            raise Refusal("illegal", "unknown_reference", f"{seat} is not a seat")
        return self.seats.index(seat)

    def _engine_id(self, record_id: str) -> int:
        if record_id not in self.current:
            raise Refusal("illegal", "unknown_reference", f"{record_id} is not an object the engine has now")
        return self.current[record_id]

    def _target(self, value):
        if value in self.seats:
            return {"player": self._player(value)}
        return {"object": self._engine_id(value)}

    def _groups(self, groups) -> list:
        return [[self._target(t) for t in group] for group in groups]

    def _row_index(self, source: str, ability: dict, adding: list | None = None) -> int:
        """The engine's row index of a record's ability reference on `source` (spec 3.3); a land with
        two basic land types has two intrinsic mana abilities (305.6), told apart by the mana added."""
        if source_free_rule(ability):
            raise Refusal("illegal", "illegal_action", "an intrinsically source-free triggered rule is not an activated ability of an object")
        engine_id = self._engine_id(source)
        rows = self.audit["objects"][str(engine_id)]["abilities"]
        if "rule" in ability:
            matches = [r for r in rows if r["origin"]["kind"] == "rule"]
            if len(matches) > 1 and adding:
                matches = [r for r in matches if all("{" + mana + "}" in r["text"] for mana in adding)] or matches
        elif "inherited_from" in ability:
            matches = [r for r in rows if r["origin"]["kind"] == "inherited"
                       and self._ability_ref(r, engine_id) == ability]
        elif "granted_by" in ability:
            matches = [r for r in rows if r["origin"]["kind"] == "given"
                       and self._ability_ref(r, engine_id) == ability]
        else:
            kinds = ("own",) if ability.get("of") == "card" else ("own", "copied")
            matches = [r for r in rows if r["origin"]["kind"] in kinds and r.get("paragraph") == ability["paragraph"]]
        if ability.get("sub") and len(matches) > 1:
            keyed = [r for r in matches if r.get("keyword")]
            if len(keyed) >= ability["sub"]:
                matches = [keyed[ability["sub"] - 1]]
        if len(matches) > 1:
            keyed = [r for r in matches if r.get("keyword")]
            if len(keyed) == 1:
                matches = keyed  # the activatable row of a keyword line, not its text row
        if len(matches) != 1:
            raise Refusal("illegal", "unknown_reference", f"ability {json.dumps(ability)} of {source} names {len(matches)} rows of the engine")
        return matches[0]["index"]

    def _mana_choice(self, source: str, row_index: int) -> bool:
        engine_id = self._engine_id(source)
        rows = self.audit["objects"][str(engine_id)]["abilities"]
        text = next((r["text"] for r in rows if r["index"] == row_index), "")
        effect = text.split(":", 1)[1] if ":" in text else text
        return bool(re.search(r"\bany\b|\bor\b|\bchoice\b", effect, re.I))

    def _payment(self, pay: dict, move: dict):
        plan = []
        for step in pay.get("abilities", []):
            nested = step.get("pay", {})
            row = self._row_index(step["source"], step["ability"], step.get("adding"))
            entry = {"source": self._engine_id(step["source"]), "ability": row,
                     "paying": [[self._engine_id(obj) for obj in group] for group in nested.get("objects", [])], "adding": []}
            self._payment(nested, entry)
            # Selections are bound at the actual locked boundary by _explicit_payment.
            entry.pop("mana", None)
            if "x" in step:
                entry["x"] = step["x"]
            if step.get("additional", {}).get("choices"):
                entry["chose"] = [choice - 1 for choice in step["additional"]["choices"]]
            if step.get("adding") and self._mana_choice(step["source"], row):
                entry["adding"] = [list(step["adding"])]
            plan.append(entry)
        if plan:
            move["plan"] = plan
        if pay.get("pool"):
            move["mana"] = {k: v for k, v in pay["pool"].items() if v}
        if pay.get("objects"):
            move["paying"] = [[self._engine_id(o) for o in group] for group in pay["objects"]]
        if pay.get("life"):
            move["life"] = list(pay["life"])

    def _move(self, do: dict) -> dict:
        kind = do["kind"]
        if kind not in ANSWER_KINDS:
            raise unsupported("unsupported_kind", f"{kind} has no engine move")
        if kind == "play_land":
            move = {"kind": "play", "card": self._engine_id(do["card"])}
            if do.get("face"):
                move["face"] = do["face"]
            return move
        if kind in ("cast", "cast_during_resolution"):
            if kind == "cast_during_resolution" and do.get("decline"):
                return {"kind": "choose", "answer": "no"}
            move = {"kind": "cast", "card": self._engine_id(do["card"]), "targets": self._groups(do.get("targets", []))}
            face = do.get("face", 0)
            if do.get("fused"):
                if face or do["card"] not in self.cards or not self.oracle.has_fuse(self.cards[do["card"]]["oracle"]):
                    raise unsupported("unsupported_fact", "fused casting needs both halves of a pinned fuse split card")
                move["cast_as"] = "fused"
            elif face or (do["card"] in self.cards and self.oracle.faces[self.cards[do["card"]]["oracle"]][0].raw.get("layout") == "split"):
                move["cast_as"] = self._cast_as(do["card"], face)
            self._announcement(do, move)
            cost = do.get("cost")
            if cost and cost.get("kind") == "alternative":
                if kind == "cast_during_resolution" and cost.get("effect") == "without_paying":
                    pass  # _send_move authenticates the selected permission before changing the game (118.9, 608.2g)
                else:
                    self._alternative_cost(do, cost, move)
            self._casting_payment(do, move)
            return move
        if kind == "activate":
            row = self._row_index(do["source"], do["ability"], do.get("adding"))
            move = {"kind": "activate", "source": self._engine_id(do["source"]), "ability": row,
                    "targets": self._groups(do.get("targets", []))}
            self._announcement(do, move)
            if do.get("adding") and self._mana_choice(do["source"], row):
                move["adding"] = [list(do["adding"])]
            return move
        if kind == "special_action":
            action = do.get("action")
            if action in ("plot", "foretell"):
                move = {"kind": action, "card": self._engine_id(do["object"])}
            elif action == "unlock":
                move = {"kind": "unlock", "room": self._engine_id(do["object"]), "half": do.get("half", 0)}
            elif action == "turn_face_up":
                move = {"kind": "turn_face_up", "permanent": self._engine_id(do["object"]), "way": do.get("way", "morph")}
            elif action == "companion":
                move = {"kind": "companion"}
            else:
                raise unsupported("unsupported_kind", f"special action {action} has no engine move")
            if do.get("pay"):
                self._payment(do["pay"], move)
            return move
        if kind == "declare_attackers":
            if do.get("pay"):
                raise unsupported("unsupported_kind", "costs to attack are not translated")
            return {"kind": "attack", "attacks": [[self._engine_id(a), self._target(d)] for a, d in do.get("attacks", [])]}
        if kind == "declare_blockers":
            if do.get("pay"):
                raise unsupported("unsupported_kind", "costs to block are not translated")
            return {"kind": "block", "blocks": [[self._engine_id(b), self._engine_id(a)] for b, a in do.get("blocks", [])]}
        if kind == "assign_combat_damage":
            return {"kind": "divide", "division": [[self._target(t), n] for t, n in do["assignments"]]}
        if kind == "choose_targets" or kind == "change_targets":
            asked_kind = (self.state.get("asked") or {}).get("kind")
            if asked_kind in ("change_targets", "retarget"):
                # a change of targets (115.7) or new targets for a copy (707.10c) is a choice of
                # the effect being applied, not the targeting of a triggered ability (603.3d)
                return {"kind": "choose", "answer": {"targets": self._groups(do.get("targets", []))}}
            move = {"kind": "targets", "targets": self._groups(do.get("targets", []))}
            if do.get("division"):
                move["division"] = deepcopy(do["division"])
            return move
        if kind == "choose_modes":
            return {"kind": "modes", "modes": [m - 1 for m in do.get("modes", [])]}
        if kind == "order_triggers":
            return {"kind": "order", "order": self._trigger_order(do["order"])}
        if kind == "optional":
            return {"kind": "choose", "answer": "yes" if do["yes"] else "no"}
        if kind == "command_zone":
            return {"kind": "choose", "answer": "yes" if do["yes"] else "no"}
        if kind == "choose_objects":
            asked = self.state.get("asked") or {}
            objects = [self._engine_id(o) for o in do.get("objects", [])]
            if asked.get("kind") == "optional_objects":
                return {"kind": "choose", "answer": {"accept": True, "objects": objects} if objects else {"accept": False}}
            return {"kind": "choose", "answer": {"objects": objects}}
        if kind == "opening_hand_action":
            # the engine asks for up to one card of the hand whose action is taken now (103.6)
            objects = [self._engine_id(do["card"])] if do.get("card") else []
            return {"kind": "choose", "answer": {"objects": objects}}
        if kind == "separate":
            # the engine separates into two piles: the cards chosen are the first pile named by
            # the text, the rest the other (700.3a)
            piles = do.get("piles", [])
            if len(piles) != 2:
                raise unsupported("unsupported_kind", "the engine separates cards into two piles only (700.3)")
            return {"kind": "choose", "answer": {"objects": [self._engine_id(o) for o in piles[0]]}}
        if kind == "concede":
            return {"kind": "concede"}
        if kind == "search":
            return {"kind": "choose", "answer": {"objects": [self._engine_id(o) for o in do.get("found", [])]}}
        if kind == "arrange":
            asked = self.state.get("asked") or {}
            if asked.get("kind") == "order_cards" and isinstance(asked.get("arrange"), dict):
                if do.get("away"):
                    raise Refusal("illegal", "illegal_action", "look-and-return arrangement puts every card back on top")
                return {"kind":"choose", "answer":{"objects":[self._engine_id(o) for o in do.get("top", [])]}}
            return {"kind": "choose", "answer": {"top": [self._engine_id(o) for o in do.get("top", [])],
                                                 "away": [self._engine_id(o) for o in do.get("away", [])]}}
        if kind == "library_destination":
            at = do["at"]
            position = {"position": at} if isinstance(at, str) else {"from_top": at["from_top"]}
            return {"kind": "choose", "answer": {"library_position": position}}
        if kind == "order_cards":
            asked = self.state.get("asked") or {}
            if asked.get("kind") != "order_cards":
                raise Refusal("illegal", "illegal_action", "no owner arrival order is being chosen")
            return {"kind": "choose", "answer": {"objects": [self._engine_id(card) for card in do["order"]]}}
        if kind == "choose_replacement":
            return {"kind": "choose", "answer": {"replacement": self._replacement(do["effect"])}}
        if kind == "spell_destination":
            asked = self.state.get("asked") or {}
            on = (asked.get("on") or {}).get("object")
            if on is None:
                raise Refusal("illegal", "illegal_action", "no spell destination is being chosen")
            offered = self._offered_effect(do["effect"], "spell_destination")
            return {"kind": "choose", "answer": {"spell_destination": {"on": on, "effect": offered["effect"]}}}
        if kind == "pay":
            if do.get("decline"):
                return {"kind": "choose", "answer": "no"}
            payment = {}
            self._payment(do.get("pay", {}), payment)
            return {"kind": "choose", "answer": {"pay": payment}}
        if kind == "choose_alternative":
            if do.get("decline"):
                return {"kind": "choose", "answer": "no"}
            return {"kind": "choose", "answer": {"alternative": do["alternative"] - 1}}
        if kind == "discard":
            return {"kind": "discard", "cards": [self._engine_id(c) for c in do["cards"]]}
        if kind == "bottom":
            return {"kind": "bottom", "cards": [self._engine_id(c) for c in do["cards"]]}
        if kind == "mulligan":
            return {"kind": "keep" if do["keep"] else "mulligan"}
        if kind == "choose_player":
            return {"kind": "choose", "answer": {"player": self._player(do["player"])}}
        if kind == "choose_number":
            return {"kind": "choose", "answer": {"value": {"number": do["number"]}}}
        if kind == "coin_call":
            return {"kind": "choose", "answer": {"call": do["call"]}}
        if kind == "choose_value":
            of = self._value_kind(self.state.get("asked") or {})
            keys = {"color": "color", "creature_type": "creature_type", "card_name": "card_name",
                    "basic_land_type": "basic_land_type", "card_type": "card_type", "land_type": "basic_land_type"}
            if of not in keys:
                raise unsupported("unsupported_kind", f"a chosen {of} has no engine answer")
            value = do["value"]
            domain = {"creature_type": "creature", "basic_land_type": "basic_land", "land_type": "land"}.get(of)
            if domain:
                try:
                    value = native_subtype(value, domain)
                except ValueError as error:
                    raise Refusal("illegal", "illegal_action", str(error)) from error
            return {"kind": "choose", "answer": {"value": {keys[of]: value}}}
        if kind == "choose_mana":
            # Mana symbols are a multiset (106.1); the native option vectors
            # have a presentation order which carries no rules meaning.
            choices = (self.state.get("asked") or {}).get("options", [])
            chosen = next((choice for choice in choices if Counter(choice) == Counter(do["mana"])), do["mana"])
            return {"kind": "choose", "answer": {"mana": list(chosen)}}
        if kind == "holders":
            return {"kind": "choose", "answer": {"holders": [self._target(h) for h in do["holders"]]}}
        if kind == "attach":
            return {"kind": "choose", "answer": {"attach": self._target(do["to"])}}
        if kind == "choose_defender":
            return {"kind": "choose", "answer": {"defender": self._target(do["defender"])}}
        if kind == "pile":
            return {"kind": "choose", "answer": {"pile": do["pile"] - 1}}
        if kind == "skip":
            return {"kind": "choose", "answer": {"skip": self._offered_effect(do["effect"], "skip")["id"]}}
        raise unsupported("unsupported_kind", f"{kind} has no engine move")

    def _value_kind(self, asked: dict) -> str | None:
        """What a `value` question chooses (105.1, 205.3m, 201.4, 205.2a, 305.6): as the engine
        says it, or else as its options spell it."""
        of = asked.get("of")
        if of is None:
            for option in asked.get("options", []):
                if isinstance(option, dict) and len(option) == 1:
                    of = next(iter(option))
                    break
        return VALUE_KINDS.get(of)

    def _offered_effect(self, effect: dict, question: str) -> dict:
        """The entry of the engine's offered effects (`among` of a `skip` or `spell_destination`
        question) that an MGN effect reference names: by rule, or by source and ability row."""
        asked = self.state.get("asked") or {}
        among = [e for e in asked.get("among", []) if isinstance(e, dict)]
        if not among:
            raise unsupported("unsupported_kind", f"the engine's {question} question does not say the source of each effect")
        if "rule" in effect:
            matches = [e for e in among if e.get("rule") == effect["rule"]]
        else:
            # the engine names the object that created the effect, which may since have changed
            # zone (a resolved spell, 400.7): the card is the same, so its physical identity is matched
            physical = self._physical_of(effect["source"])
            row = self._row_index(effect["source"], effect["ability"])
            matches = [e for e in among if self._physical_of_engine(e.get("source")) == physical and e.get("ability") == row]
        if len(matches) != 1:
            raise Refusal("illegal", "illegal_action", f"the effect {json.dumps(effect)} is offered {len(matches)} times")
        return matches[0]

    def _physical_of(self, record_id: str) -> int:
        if record_id not in self.physical:
            raise Refusal("illegal", "unknown_reference", f"{record_id} is not a card of this game")
        return self.physical[record_id]

    def _physical_of_engine(self, engine_id) -> int | None:
        """The physical identity of an engine object, seen or not yet seen."""
        if engine_id is None:
            return None
        if engine_id in self.seen_physical:
            return self.seen_physical[engine_id]
        item = self.audit.get("objects", {}).get(str(engine_id)) if self.audit else None
        return item.get("physical") if item else None

    def _cast_as(self, record_id: str, face: int):
        """How the engine names casting a card as a face other than its first: an Omen (720.3)
        or an Adventure (715.3) by that word, a modal double-faced card's back face by its index."""
        faces = self.oracle.faces[self.cards[record_id]["oracle"]]
        if face >= len(faces):
            raise Refusal("illegal", "unknown_reference", f"{record_id} has no face {face}")
        if faces[0].raw.get("layout") == "split":
            return face
        type_line = faces[face].raw.get("type_line", "")
        if "Omen" in type_line.split("—")[-1]:
            return "omen"
        if "Adventure" in type_line.split("—")[-1]:
            return "adventure"
        return {"face": face}

    def _announcement(self, do: dict, move: dict):
        if do.get("x") is not None:
            move["x"] = do["x"]
        if do.get("modes"):
            move["modes"] = [m - 1 for m in do["modes"]]
        if do.get("division"):
            move["division"] = deepcopy(do["division"])
        additional = do.get("additional") or {}
        if additional.get("kicker"):
            move["kicks"] = additional["kicker"]
        for key in ("buyback", "entwine"):
            if additional.get(key):
                move[key] = True
        if additional.get("replicate"):
            move["replicate"] = additional["replicate"]
        for paid in additional.get("paid", []):
            move["additional" if paid not in ("bargain", "offspring", "casualty", "teamwork") else paid] = True
        if additional.get("gift"):
            move["gift"] = f"p{self._player(additional['gift'])}"
        if additional.get("choices"):
            move["chose"] = [c - 1 for c in additional["choices"]]
        if do.get("pay"):
            self._payment(do["pay"], move)

    def _casting_payment(self, do: dict, move: dict):
        """Separately named substitutions follow ordinary cost groups (702.51/66).

        Harmonize's creature is a fixed announced tap, not a choosing payment
        group (702.180b). A zone permission does not select an alternative cost.
        """
        permission = do.get("permission")
        if permission:
            if "route" in move:
                raise unsupported("unsupported_kind", "a casting permission cannot be combined with a different casting route", "do.permission")
            source = self._engine_id(permission["source"])
            row = self._row_index(permission["source"], permission["ability"])
            rows = self.audit["objects"][str(source)].get("abilities", [])
            chosen = next((r for r in rows if r.get("index") == row), {})
            if source == move["card"] and (chosen.get("keyword") or keyword_name(chosen.get("text") or "") or "").lower() == "retrace":
                move["route"] = "retrace"
            else:
                move["route"] = {"source": source, "ability": row}
        # An omitted MGN cost means mana cost. Never let the native default route
        # silently choose flashback instead (601.2b, 118.9).
        move.setdefault("route", "normal")
        groups = deepcopy(move.get("paying", []))
        delve = do.get("delve", [])
        convoke = do.get("convoke", [])
        if delve:
            move["delve"] = len(delve)
            groups.append([self._engine_id(obj) for obj in delve])
        if convoke:
            if any(not isinstance(part, dict) or set(part) != {"object", "mana"}
                   or part["mana"] not in (None, "W", "U", "B", "R", "G") for part in convoke):
                raise unsupported("unsupported_kind", "convoke needs each creature and its colored or generic allocation", "do.convoke")
            move["convoke"] = [part["mana"] or "generic" for part in convoke]
            groups.extend([self._engine_id(part["object"])] for part in convoke)
        resources = [obj for part in convoke for obj in [self._engine_id(part["object"])]] + [self._engine_id(obj) for obj in delve]
        if len(resources) != len(set(resources)) or any(obj in resources for group in move.get("paying", []) for obj in group):
            raise Refusal("illegal", "illegal_cost", "a selected convoke or delve object cannot pay twice (118.10)")
        if groups:
            move["paying"] = groups

    def _alternative_cost(self, do: dict, cost: dict, move: dict):
        routes = {"flashback", "retrace", "escape", "foretell", "harmonize"}
        keyword = cost.get("keyword")
        if keyword == "retrace":
            raise unsupported("unsupported_kind", "retrace is a casting permission and additional discard, not an alternative cost (702.81a)", "do.cost")
        if keyword in routes:
            move["route"] = {"harmonize": self._engine_id(cost["tapped"])} if keyword == "harmonize" and cost.get("tapped") else keyword
            return
        if cost.get("effect") == "without_paying":
            raise unsupported("unsupported_kind", "casting without paying the mana cost is not translated")
        engine_id = self._engine_id(do["card"])
        alternatives = self.audit["objects"][str(engine_id)].get("casting_costs", {}).get("alternatives", [])
        if keyword:
            rows = [a for a in alternatives if (a.get("text") or "").lower().startswith(keyword)]
        else:
            rows = [a for a in alternatives
                    if self._record_id(a.get("source", engine_id)) == cost.get("source")
                    and self._ability_ref(a, a.get("source", engine_id)) == cost.get("ability")]
        if not rows and keyword and (self.state.get("asked") or {}).get("kind") == "cast":
            # CR 702.35a: the resolving madness trigger supplies the permission.
            # The pending card domain and retained definition identify its paid cost.
            texts = self.audit["objects"][str(engine_id)].get("definition_rows", [])
            if (engine_id in self.state["asked"].get("cards", []) and keyword == "madness"
                    and move.get("cast_as", "whole") in ("whole", {"face": 0})):
                rows = [{"paragraph": i + 1} for i, text in enumerate(texts)
                        if keyword_name(text) == keyword]
        if len(rows) != 1:
            raise unsupported("unsupported_kind", f"alternative cost {json.dumps(cost)} matches {len(rows)} of the engine's rows")
        move["cost"] = {"alternative": rows[0]["paragraph"] - 1}

    def _replacement(self, effect: dict) -> dict:
        asked = self.state.get("asked") or {}
        if asked.get("kind") != "replacement":
            raise Refusal("illegal", "illegal_action", "no replacement effect is being chosen")
        if "rule" in effect:
            raise unsupported("unsupported_kind", "a replacement effect of the rules is not offered by name")
        physical = self._physical_of(effect["source"])
        matches = []
        for offered in asked.get("among", []):
            if not isinstance(offered, dict):
                continue
            producer = offered.get("producer") or offered
            if self._physical_of_engine(producer.get("source")) != physical and self._physical_of_engine(offered.get("source")) != physical:
                continue
            paragraph = producer.get("paragraph")
            if paragraph is not None and paragraph != effect["ability"].get("paragraph"):
                continue
            matches.append(offered)
        if len(matches) != 1:
            raise Refusal("illegal", "illegal_action", f"the effect of {effect['source']} is offered {len(matches)} times")
        chosen = matches[0]
        producer = chosen.get("producer") or chosen
        key = {"source": producer.get("source", chosen.get("source")), "part": chosen.get("part", producer.get("part", 0))}
        ability = producer.get("ability")
        if ability is None:
            ability = self._row_index(effect["source"], effect["ability"])
        key["ability"] = ability
        return key

    def _trigger_order(self, order: list) -> list:
        prompt, labels = self._prompt_of(self.state["asked"]["player"])
        decision = prompt.get("decision") or {}
        listed = decision.get("abilities", [])
        pending = self.audit.get("pending_triggers", [])
        instances = [self._instance_from_prompt(item, labels, pending[i] if i < len(pending) else None) for i, item in enumerate(listed)]
        remaining = list(range(len(instances)))
        out = []
        for wanted in order:
            matches = [i for i in remaining if self._same_instance(instances[i], wanted)]
            if not matches:
                raise Refusal("illegal", "unknown_reference", f"no pending trigger matches {json.dumps(wanted)}")
            out.append(matches[0])
            remaining.remove(matches[0])
        if remaining:
            raise Refusal("illegal", "illegal_action", "the order names fewer triggers than are pending")
        return out

    @staticmethod
    def _same_instance(have: dict, wanted: dict) -> bool:
        if have.get("source") != wanted.get("source") or have.get("ability") != wanted.get("ability"):
            return False
        if "for" in wanted:
            return sorted(have.get("for", [])) == sorted(wanted["for"])
        return True

    def _prompt_of(self, player: int) -> tuple[dict, dict]:
        seat = f"p{player}"
        prompt = self.engine.must({"cmd": "prompt", "seat": seat})["prompt"]
        labels = self.engine.must({"cmd": "labels", "seat": seat})["labels"]
        return prompt, labels

    def _instance_from_prompt(self, item: dict, labels: dict, pending: dict | None) -> dict:
        source_label = item.get("source")
        if source_label is None:
            rule = ((pending or {}).get("ability") or {}).get("rule_ref")
            if not isinstance(rule, dict) or not source_free_rule(rule):
                raise unsupported("unsupported_fact", "a pending source-free rule lacks authenticated rule identity", "pending.about")
            instance = {"ability": deepcopy(rule)}
            subjects = [self._target_record(subject) for binding in ((pending or {}).get("bound") or {}).get("inputs", [])
                        if binding.get("kind") == "subject" for subject in binding.get("subjects", [])]
            if subjects:
                instance["for"] = subjects
            return instance
        if source_label not in labels:
            raise unsupported("unsupported_fact", "a pending trigger without a source cannot be named", "pending.about")
        engine_source = labels[source_label]
        record_source = self._record_id(engine_source)
        rows = self.audit["objects"].get(str(engine_source), {}).get("abilities", [])
        matches = [r for r in rows if r["text"] == item.get("text") and r["origin"]["kind"] != "rule"]
        if not matches:
            matches = [r for r in rows if r.get("index") == item.get("ability")]
        if not matches:
            raise unsupported("unsupported_fact", f"a pending trigger of {record_source} matches no ability row", "pending.about")
        instance = {"source": record_source, "ability": self._ability_ref(matches[0], engine_source)}
        subjects = []
        for binding in ((pending or {}).get("bound") or {}).get("inputs", []):
            if binding.get("kind") == "subject":
                for subject in binding.get("subjects", []):
                    subjects.append(self._target_record(subject))
        if subjects:
            instance["for"] = subjects
        return instance

    def _asked_kind(self, asked: dict) -> str:
        """The MGN kind of an engine question as the projection names it (`_pending`): the engine's
        `select` is the opening-hand action before the game began and the separation into piles
        when labelled so, its `replacement` of a spell's destination is `spell_destination`."""
        if asked["kind"] == "select":
            if not self.state.get("begun", True):
                return "opening_hand_action"
            if asked.get("separate"):
                return "separate"
        if asked["kind"] == "replacement" and asked.get("event") == "spell_destination":
            return "spell_destination"
        return KINDS.get(asked["kind"], asked["kind"])

    def _null_move(self, asked: dict) -> dict | None:
        kind = asked["kind"]
        if kind == "priority":
            return {"kind": "pass"}
        if kind == "attackers":
            if any(a.get("must") for a in asked.get("able", [])):
                return None  # an attack requirement (508.1d)
            return {"kind": "attack", "attacks": []}
        if kind == "blockers":
            return {"kind": "block", "blocks": []}
        if kind == "mulligan":
            return {"kind": "keep"}
        if kind == "may":
            return {"kind": "choose", "answer": "no"}
        if kind == "optional_objects":
            return {"kind": "choose", "answer": {"accept": False}}
        if kind == "select" and asked.get("separate"):
            return None  # a separation into piles has no null answer (700.3)
        if kind in ("objects", "select", "search"):
            if asked.get("min", 1) == 0:
                return {"kind": "choose", "answer": {"objects": []}}
            return None
        if kind == "targets":
            if all(slot.get("min", 1) == 0 for slot in asked.get("slots", [])):
                return {"kind": "targets", "targets": [[] for _ in asked.get("slots", [])]}
            return None
        if kind == "modes":
            if any(not option.get("modes") for option in asked.get("options", [])):
                return {"kind": "targets", "targets": [], "modes": []}
            return None
        if kind == "order_cards" and isinstance(asked.get("arrange"), dict):
            return {"kind": "choose", "answer": {"objects": list(asked["cards"])}}
        if kind == "arrange":
            return {"kind": "choose", "answer": {"top": list(asked.get("cards", [])), "away": []}}
        if kind == "pay":
            return {"kind": "choose", "answer": "no"}
        if kind == "alternative" and asked.get("optional"):
            return {"kind": "choose", "answer": "no"}
        if kind == "cast":
            return {"kind": "choose", "answer": "no"}
        if kind == "holders":
            return {"kind": "choose", "answer": {"holders": []}}
        if kind == "command_zone":
            return {"kind": "choose", "answer": "no"}
        if kind == "change_targets" and asked.get("policy") == "choose_new":
            return {"kind": "choose", "answer": "no"}  # the targets stay as they are (115.7d)
        if kind == "retarget":
            return {"kind": "choose", "answer": "no"}  # the copy keeps the original's targets (707.10c)
        return None

    # ----- restoration -----

    def _restore(self):
        """Replays the journal into a fresh engine: the state is back at the boundary."""
        journal = list(self.journal)
        setup = self.setup_request
        self.restoring = True
        try:
            self.setup(setup)
            for entry in journal[1:]:
                outcomes = list(entry.get("outcomes", []))

                def replay_ask(key, _outcomes=outcomes):
                    for index, outcome in enumerate(_outcomes):
                        if matches_request(outcome, key):
                            return _outcomes.pop(index)
                    return None
                if entry["op"] == "act":
                    self.act(entry["n"], entry["seat"], entry["do"], replay_ask)
                elif entry["op"] == "default":
                    self.default(entry["until"], replay_ask)
        finally:
            self.restoring = False

    # ----- observation -----

    def _observe(self, state: dict, initial: bool = False):
        """Reads the engine after a reply: the state given, the audit and the libraries."""
        session = state["session_id"]
        audit = self.engine.must({"cmd": "audit_game", "session_id": session})["state"]
        libraries = [self.engine.must({"cmd": "library_order", "session_id": session, "player": p})["state"]["order"]
                     for p in range(len(self.seats))]
        self._observe_snapshot(state, audit, libraries, initial=initial)

    def _observe_snapshot(self, state: dict, audit: dict, libraries: list, initial: bool = False):
        """Observe one complete frame: the engine's state, its audit and the library orders."""
        self.session_id = state["session_id"]
        self.state, self.audit, self.libraries = state, audit, libraries
        self.entered = []
        # the events since the last privileged look may span turns: each `began` belongs to the turn
        # that had begun by then, counted from the turn of the previous observation
        turn = self.turn_seen if self.turn_seen is not None else state["turn"]
        events = state.get("events", [])
        damage_total = sum(1 for e in events if e.get("event") == "began" and e.get("part") == "CombatDamage")
        damage_seen = 0
        for event in events:
            kind = event.get("event")
            if kind == "conceded":
                label = event.get("player")
                index = int(label[1:]) if isinstance(label, str) else label
                self.conceded.add(self.seats[index])
            if kind == "turn_began":
                turn += 1
            elif kind == "began" and not initial:
                if event["part"] == "CombatDamage":
                    damage_seen += 1
                self._began(event["part"], turn, damage_seen, damage_total)
            elif kind == "created" and event.get("token") is not None:
                self.token_root.setdefault(event["token"], event["token"])
            elif kind == "zone":
                was, now = event.get("was"), event.get("now")
                if was in self.token_root and now is not None:
                    self.token_root[now] = self.token_root[was]
                # Preparation's public copy is cast from exile (722.3c/d).
                # The record keeps its S identity while Karn creates a new
                # zone incarnation; ordinary resolved stack objects do not.
                prior = self.audit.get("objects", {}).get(str(was), {})
                if (now is not None and event.get("to") == "Stack"
                        and prior.get("prepare_source") is not None
                        and was in self.committed_stack):
                    self.committed_stack[now] = self.committed_stack[was]
                if event.get("to") == "Battlefield" and now is not None:
                    self.entered.append(now)
        begun = state.get("begun", True)
        if begun and turn != state["turn"] and not initial:
            self.turn_counts = {k: v for k, v in self.turn_counts.items() if k[0] != state["turn"]}  # the count is lost; defaults apply
        # before the first turn (103.5, 103.6) the engine already names turn 1; the record's turn is 0
        self.turn_seen = state["turn"] if begun else 0
        self._refresh_ids()

    def _began(self, part: str, turn: int, damage_seen: int, damage_total: int):
        step = PARTS[part]
        if part == "CombatDamage":
            combat = self.audit.get("combat", {})
            steps = combat.get("damage_steps") or 0
            first = combat.get("first_strikers")
            # the first combat damage step of a combat with first strikers is the first-strike one
            # (510.4); the audit describes the current combat, so of the combat damage steps begun
            # in this reply only the last `steps` belong to it and can be told apart; earlier ones
            # (an additional combat phase in the same stretch) cannot, and the turn's combat damage
            # labels are then unsupported
            in_current = damage_seen - (damage_total - steps)  # 1-based within the current combat
            if in_current < 1:
                self.uncertain_damage_turns.add(turn)
            elif first is not None and in_current == 1:
                step = "first_strike_damage"
        key = (turn, step)
        self.turn_counts[key] = self.turn_counts.get(key, 0) + 1
        # the position of the step in its turn: what orders two steps of a turn an effect added to
        self.steps_begun[turn] = self.steps_begun.get(turn, 0) + 1
        self.ordinals[(turn, step, self.turn_counts[key])] = self.steps_begun[turn]

    def _ordinal(self, turn: int, step: str, occurrence: int) -> int | None:
        """Where a step of a turn stands among the steps begun in it, if it has begun."""
        return self.ordinals.get((turn, step, occurrence))

    def _refresh_ids(self, *, keep_departed=False):
        """Provisional numbering of tokens, stack objects and emblems that exist now (spec 1.8), and
        the current engine id of every record id."""
        objects = self.audit["objects"]
        if not keep_departed:
            self.departed_tokens = deepcopy(self.committed_departed)
        for key, item in objects.items():
            if item["physical"] is not None:
                self.seen_physical[int(key)] = item["physical"]
        live = {int(k): v for k, v in objects.items() if v["zone"] != "Gone"}
        self.current = {}
        for engine_id, item in live.items():
            if item["physical"] is not None and item["physical"] not in self.by_physical:
                self._bind_newcomer(item["physical"], item)
            if item["physical"] is not None and item["physical"] in self.by_physical:
                self.current[self.by_physical[item["physical"]]] = engine_id
        self.tokens = dict(self.committed_tokens)
        numbered_roots = {self.token_root.get(e, e) for e in self.tokens}
        # tokens first shown at one boundary are numbered in the canonical order of their
        # definitions, then in creation order among equal definitions (spec 1.8)
        fresh = []
        for engine_id in sorted(set(live) | set(self.departed_tokens)):
            item = objects.get(str(engine_id), {})
            if engine_id in self.departed_tokens or (item["token"] and self.state["objects"].get(str(engine_id), {}).get("is_card") is False):
                root = self.token_root.setdefault(engine_id, engine_id)
                if root in numbered_roots:
                    if engine_id not in self.tokens:
                        for other, name in list(self.tokens.items()):
                            if self.token_root.get(other, other) == root:
                                self.tokens[engine_id] = name
                                break
                else:
                    numbered_roots.add(root)
                    definition = self.departed_tokens[engine_id]["token"] if engine_id in self.departed_tokens else self._token_definition(item)
                    fresh.append((canonical(definition), root, engine_id))
        for _, _, engine_id in sorted(fresh):
            self.tokens[engine_id] = f"T{len(set(self.tokens.values())) + 1}"
        self.stack_ids = dict(self.committed_stack)
        # A target/mode question exposes the trigger before Karn puts it on its
        # stack (603.3d). Bind that already reported identity to the actual object,
        # even when it will resolve before the next written decision (MGN 1.8).
        reservation = self.pending_stack
        if reservation is not None:
            matches = []
            for engine_id in self.state.get("stack", []):
                if engine_id in self.committed_stack:
                    continue
                item = live.get(engine_id, {})
                for occurrence in item.get("occurrences", []):
                    if (occurrence and item.get("properties", {}).get("controller") == reservation["controller"]
                            and all(occurrence.get(k) == reservation[k]
                                    for k in ("source", "paragraph", "origin"))):
                        matches.append(engine_id)
                        break
            if len(matches) > 1:
                raise unsupported("unsupported_fact", "the reported pending trigger matches several native stack objects", "objects")
            if matches:
                self.committed_stack[matches[0]] = reservation["id"]
                self.stack_ids[matches[0]] = reservation["id"]
                self.pending_stack = None
            elif (self.state.get("asked") or {}).get("kind") not in ("targets", "modes"):
                self.pending_stack = None  # no stack object resulted; the reported number still never recurs
        self.synthetic_stack = None
        for engine_id in self.state.get("stack", []):  # deeper on the stack means created earlier
            item = live.get(engine_id)
            if item is None or item["physical"] is not None or item["token"]:
                continue
            if engine_id not in self.stack_ids:
                self.stack_ids[engine_id] = self._next_stack_id()
        for engine_id in sorted(live):
            item = live[engine_id]
            if (item.get("prepare_source") is not None and item["zone"] == "Exile"
                    and engine_id not in self.stack_ids):
                self.stack_ids[engine_id] = self._next_stack_id()
        self.emblems = dict(self.committed_emblems)
        for engine_id in sorted(live):
            if self.state["objects"].get(str(engine_id), {}).get("emblem") and engine_id not in self.emblems:
                self.emblems[engine_id] = f"E{len(self.emblems) + 1}"
        for mapping in (self.tokens, self.stack_ids, self.emblems):
            for engine_id, record_id in mapping.items():
                if engine_id in live:
                    self.current[record_id] = engine_id

    def _commit_generated(self):
        """The last reported state was a boundary: its provisional numbers hold from now on."""
        if self.last_projection is not None:
            self.projection_history = self.last_projection
        self.committed_tokens = {e: name for e, name in self.tokens.items()}
        self.committed_departed = deepcopy(self.departed_tokens)
        self.committed_stack = dict(self.stack_ids)  # resolved abilities keep their numbers: engine ids never recur
        if self.synthetic_stack is not None:
            self.reserved_stack.add(self.synthetic_stack["id"])
            self.pending_stack = dict(self.synthetic_stack)
        self.committed_emblems = dict(self.emblems)

    def _next_stack_id(self):
        used = set(self.stack_ids.values()) | self.reserved_stack
        return f"S{max((int(name[1:]) for name in used), default=0) + 1}"

    def _record_id(self, engine_id: int) -> str:
        """The record's id of an engine object, a departed one included (a card keeps its physical
        identity across zone changes; a spell on the stack still names a target that left)."""
        if engine_id in self.causal_labels:
            return self.causal_labels[engine_id]
        item = self.audit["objects"].get(str(engine_id))
        physical = item["physical"] if item is not None else None
        if physical is None:
            physical = self.seen_physical.get(engine_id)
        if physical is not None and physical in self.by_physical:
            return self.by_physical[physical]
        for mapping in (self.tokens, self.stack_ids, self.emblems, self.committed_tokens, self.committed_stack, self.committed_emblems):
            if engine_id in mapping:
                return mapping[engine_id]
        root = self.token_root.get(engine_id)
        if root is not None:
            for mapping in (self.tokens, self.committed_tokens):
                for other, name in mapping.items():
                    if self.token_root.get(other, other) == root:
                        return name
        if item is not None and item.get("token") and item.get("physical") is None and item.get("zone") == "Gone":
            if item.get("copy_acquisition") or any(row.get("origin", {}).get("kind") == "copied" for row in item.get("abilities", [])):
                raise unsupported("unsupported_fact", "departed token has acquired copy provenance unavailable as an intrinsic token definition", "departed")
            if item.get("definition_rows") is not None and item["definition_rows"] != (item.get("base") or {}).get("rows"):
                raise unsupported("unsupported_fact", "departed token definition differs from its authenticated declaring rows", "departed")
            definition = self._token_definition(item)
            self.departed_tokens[engine_id] = {"token": definition, "owner": self.seats[item["owner"]]}
            used = {name for name in self.tokens.values()}
            record_id = f"T{max((int(name[1:]) for name in used), default=0) + 1}"
            self.tokens[engine_id] = record_id
            return record_id  # Canonical union numbering is recomputed before returning the projection.
        raise unsupported("unsupported_fact", f"engine object {engine_id} has no id in the record", "objects")

    def _target_record(self, value):
        if value is None:
            return None
        if "player" in value:
            return self.seats[value["player"]]
        return self._record_id(value["object"])

    # ----- projection -----

    def _turn(self) -> dict:
        state = self.state
        if not state.get("begun", True):
            return {"number": 0, "step": "pregame", "active": None}  # the mulligans and opening-hand actions (103.5, 103.6)
        part = state["part"]
        step = PARTS[part]
        if part == "CombatDamage":
            combat = self.audit.get("combat", {})
            if combat.get("first_strikers") is not None and combat.get("damage_steps") == 1:
                step = "first_strike_damage"
        turn = {"number": state["turn"], "step": step, "active": self.seats[state["active"]]}
        occurrence = self.turn_counts.get((state["turn"], step), 1)
        if occurrence > 1:
            turn["occurrence"] = occurrence
        if state["turn"] in self.uncertain_damage_turns and step in ("combat_damage", "first_strike_damage"):
            self.unsupported_paths.append("turn")  # an earlier combat of this turn passed unobserved
        return turn

    def _about_object(self, prefer_stack: bool = False) -> str | None:
        """The resolving object a resolution-time question is about: the permanent the engine says
        is entering (a choice made as it enters, 614.12), else the top of the stack, else the one
        permanent that entered in the last reply, else the source of a mana ability resolving
        without the stack (605.3b), which the seat's prompt names; the engine names nothing else,
        so there is no other answer."""
        asked = self.state.get("asked") or {}
        if asked.get("entering") is not None and not prefer_stack:
            try:
                return self._record_id(asked["entering"])
            except Refusal:
                pass  # a token being created has no id yet (spec 5.4): the resolving object is named instead
        if asked.get("kind") == "mana" and type(asked.get("source")) is int:
            return self._record_id(asked["source"])
        stack = self.state.get("stack", [])
        if stack:
            return self._record_id(stack[-1])
        if asked.get("kind") == "mana":
            prompt, labels = self._prompt_of(asked["player"])
            source = ((prompt.get("decision") or {}).get("for") or {}).get("source")
            if source in labels:
                return self._record_id(labels[source])
        entered = [e for e in self.entered if str(e) in self.audit["objects"] and self.audit["objects"][str(e)]["zone"] != "Gone"]
        if len(entered) == 1:
            return self._record_id(entered[0])
        # a question the engine asked within the response to a land play, before the land moved
        # (a replacement effect as it enters, 614.1c): the permanent entering is the land played
        if self.entering_card is not None and self.entering_card in self.current:
            if self.audit["objects"][str(self.current[self.entering_card])]["zone"] != "Battlefield":
                return self.entering_card
        return None

    def _pending(self, problems: list, synthetic: dict) -> dict | None:
        asked = self.state.get("asked")
        if asked is None:
            return None
        kind = KINDS.get(asked["kind"])
        if kind is None:
            raise unsupported("unsupported_kind", f"the engine asks {asked['kind']}, which has no MGN kind")
        pending = {"seat": self.seats[asked["player"]], "kind": kind}
        if kind in ("priority", "declare_attackers", "declare_blockers", "mulligan", "skip", "choose_defender"):
            if kind == "choose_defender":
                # the resolving object whose effect puts the creature onto the battlefield (spec
                # 5.4), whether a token or a card enters attacking
                pending["about"] = {"object": self._about_object(prefer_stack=True)}
            return pending
        if kind == "order_triggers":
            prompt, labels = self._prompt_of(asked["player"])
            listed = (prompt.get("decision") or {}).get("abilities", [])
            pendings = self.audit.get("pending_triggers", [])
            instances = [self._instance_from_prompt(item, labels, pendings[i] if i < len(pendings) else None) for i, item in enumerate(listed)]
            pending["about"] = {"abilities": distinguish_instances(instances)}  # `for` only where needed (spec 3.3)
            return pending
        if kind in ("choose_targets", "choose_modes") and asked["kind"] in ("targets", "modes"):
            # the engine asks before it puts the triggered ability on the stack; the record shows
            # the ability there with empty targets (spec 5.4), so it is synthesized at the top
            prompt, labels = self._prompt_of(asked["player"])
            about = (prompt.get("decision") or {}).get("for") or {}
            if about.get("kind") != "triggered" or about.get("source") not in labels:
                raise unsupported("unsupported_fact", "the engine does not say which ability the targets are for", "pending.about")
            engine_source = labels[about["source"]]
            rows = self.audit["objects"].get(str(engine_source), {}).get("abilities", [])
            matches = [r for r in rows if r["text"] == about.get("text") and r["origin"]["kind"] != "rule"]
            if len(matches) != 1:
                raise unsupported("unsupported_fact", "the targeted ability matches no unique row of its source", "pending.about")
            actor = asked.get("acting", asked["player"])
            descriptor = {"source": engine_source, "paragraph": matches[0].get("paragraph"),
                          "origin": matches[0].get("origin"), "controller": actor}
            same = self.pending_stack is not None and all(self.pending_stack[k] == v for k, v in descriptor.items())
            stack_id = self.pending_stack["id"] if same else self._next_stack_id()
            self.synthetic_stack = {"id": stack_id, **descriptor}
            synthetic[stack_id] = {"kind": "ability", "source": self._record_id(engine_source),
                                   "ability": self._ability_ref(matches[0], engine_source), "controller": self.seats[actor]}
            if asked.get("modes"):
                synthetic[stack_id]["modes"] = [mode + 1 for mode in asked["modes"]]
            pending["about"] = {"object": stack_id}
            return pending
        if kind == "assign_combat_damage":
            pending["about"] = {"creature": self._record_id(asked["attacker"])}
            return pending
        if kind in ("discard", "bottom"):
            pending["about"] = {"count": asked.get("count")}
            return pending
        if kind == "arrange":
            pending["about"] = {"object": self._about_object(), "cards": [self._record_id(c) for c in asked.get("cards", [])]}
            return pending
        if kind == "order_cards" and isinstance(asked.get("arrange"), dict):
            pending["kind"] = "arrange"
            pending["about"] = {"object": self._record_id(asked["arrange"]["object"]),
                                "cards": [self._record_id(c) for c in asked["cards"]]}
            return pending
        if kind == "order_cards":
            destination = asked["to"]
            zone = destination["zone"]
            cards = [self._record_id(c) for c in asked["cards"]]
            if any(self.cards[card]["owner"] != pending["seat"] for card in cards):
                # Some instructions explicitly let a different player arrange an owner's
                # library. Preserve that native choice; MGN order_cards is owner-only.
                raise unsupported("unsupported_kind", "a specified non-owner orders this library; MGN order_cards requires its owner")
            pending["about"] = {"cards": cards,
                                "zone": f"{pending['seat']}.{zone}"}
            if zone == "library":
                pending["about"]["placement"] = destination.get("position", {"from_top": destination.get("from_top")})
            elif zone != "graveyard":
                raise unsupported("unsupported_kind", f"owner ordering destination {zone} is not a library or graveyard")
            return pending
        if kind == "choose_objects" and asked["kind"] == "select":
            if not self.state.get("begun", True):
                pending["kind"] = "opening_hand_action"  # which card's opening-hand action is taken (103.6)
                return pending
            if asked.get("separate"):
                pending["kind"] = "separate"  # the cards are separated into piles (700.3)
                pending["about"] = {"object": self._about_object(), "cards": [self._record_id(c) for c in asked.get("among", [])]}
                return pending
        if kind == "choose_value":
            of = self._value_kind(asked)
            if of is None:
                raise unsupported("unsupported_fact", "the engine does not say what kind of value is chosen", "pending.about")
            pending["about"] = {"object": self._about_object(), "of": of}
            return pending
        if kind == "attach":
            pending["about"] = {"object": self._record_id(asked["aura"])}  # the Aura that is entering (303.4f)
            return pending
        if kind == "change_targets":
            if asked["kind"] == "retarget" and asked["copy"] not in self.state.get("stack", []):
                # the copy's targets are chosen before it is put onto the stack (707.10c): the
                # question is about the resolving object that copies, and the copy appears with
                # its targets in the answer's delta; which spell it copies is remembered for then
                self.copy_sources[asked["copy"]] = self._copied_spell(asked["player"])
                about = self._about_object()
                if about is None:
                    raise unsupported("unsupported_fact", "the object copying the spell cannot be told", "pending.about")
                pending["about"] = {"object": about}
                return pending
            pending["about"] = {"object": self._record_id(asked.get("object", asked.get("copy")))}
            return pending
        if kind == "command_zone":
            pending["about"] = {"object": self._record_id(asked["object"])}
            return pending
        if kind == "choose_replacement":
            if asked.get("optional"):
                raise unsupported("unsupported_kind", "an optional replacement choice has no MGN form (choose_replacement has no null answer)")
            if asked.get("event") == "spell_destination":
                pending["kind"] = "spell_destination"
                pending["about"] = {"object": self._target_record(asked.get("on"))}
                return pending
            on = asked.get("on")
            # the affected object or player; a token being created does not exist yet, so the
            # resolving object whose effect is replaced is named instead (spec 5.4)
            if asked.get("event") == "create" or on is None:
                pending["about"] = {"object": self._about_object()}
            elif "player" in on:
                pending["about"] = {"object": self.seats[on["player"]]}
            else:
                pending["about"] = {"object": self._record_id(on["object"])}
            return pending
        about = self._about_object()
        if about is None:
            raise unsupported("unsupported_fact", f"the object a {kind} question is about cannot be told", "pending.about")
        pending["about"] = {"object": about}
        return pending

    def _spell_named(self, name: str, copy: int) -> str:
        spells = [e for e in self.state.get("stack", []) if e != copy and self.state["objects"].get(str(e), {}).get("name") == name
                  and self.audit["objects"].get(str(e), {}).get("physical") is not None]
        if len(spells) != 1:
            raise unsupported("unsupported_fact", f"the engine does not say which spell the copy of {name} copies", "objects")
        return self._record_id(spells[0])

    def _copied_spell(self, player: int) -> str:
        """The spell a copy being created copies (707.10): the target of the copying effect, as
        the seat's prompt names the resolving object and its targets."""
        copying_id = (self.state.get("asked") or {}).get("copy")
        original = self.audit.get("objects", {}).get(str(copying_id), {}).get("stack_copy_of")
        if original is not None:
            return self._record_id(original)
        prompt, labels = self._prompt_of(player)
        decision = prompt.get("decision") or {}
        copying = (decision.get("for") or {}).get("object")
        if copying in labels:
            engine_id = labels[copying]
            targets = self.state["objects"].get(str(engine_id), {}).get("targets", [])
            spells = [t["object"] for group in targets for t in group if "object" in t and t["object"] in self.state.get("stack", [])]
            if len(spells) == 1:
                return self._record_id(spells[0])
        raise unsupported("unsupported_fact", "the engine does not say which spell the copy copies", "pending.about")

    def _result(self) -> dict:
        """The result; its cause only where the state shows it (704.5a life, 704.5c poison) or the
        engine's events say it (a concession, 104.3a), since the engine reports who lost but not
        why: otherwise `result` is unsupported."""
        state = self.state
        if not state.get("finished"):
            return {"kind": "unfinished"}
        winner = state.get("winner")
        if winner is None:
            self.unsupported_paths.append("result")
            return {"kind": "draw", "cause": "simultaneous"}
        winner_seat = self.seats[winner]
        cause = None
        for index, player in enumerate(state["players"]):
            if self.seats[index] == winner_seat:
                continue
            if self.seats[index] in self.conceded:
                cause = "concession"
            elif player["life"] <= 0:
                cause = "life"
            elif player.get("counters", {}).get("poison", 0) >= 10:
                cause = "poison"
        if cause is None:
            self.unsupported_paths.append("result")
            cause = "effect"
        return {"kind": "win", "winner": winner_seat, "cause": cause}

    def _ability_ref(self, row: dict, engine_source: int) -> dict:
        origin = row["origin"]["kind"]
        if origin == "inherited":
            try:
                return inherited_ability_ref(row["origin"]["family"], row["declaration_index"],
                                             object_ref=self._record_id, ability_ref=self._frozen_ability_ref)
            except (KeyError, ValueError) as error:
                raise unsupported("unsupported_fact", f"inherited ability lacks authenticated declaration provenance: {error}", "objects") from error
        if origin == "rule":
            return {"rule": "305.6"}
        if row.get("paragraph") is None:
            raise unsupported("unsupported_fact", f"an ability row of {engine_source} without a paragraph", "objects")
        ref = {"face": self._face_of(engine_source), "paragraph": row["paragraph"]}
        if origin == "given":
            by = row["origin"].get("by")
            if by is None:
                producer = row["origin"].get("producer") or {}
                if not producer or producer.get("face") != 0 or type(producer.get("paragraph")) is not int:
                    raise unsupported("unsupported_fact", "the engine did not retain this granted ability's source", "objects")
                source = self._record_id(producer["source"])
                card = self.cards.get(source)
                if (card is None or self.oracle.name(card["oracle"]) != producer.get("definition")
                        or not 1 <= producer["paragraph"] <= len(self.oracle.paragraphs(card["oracle"], 0))):
                    raise unsupported("unsupported_fact", "resolved grant producer does not authenticate a pinned paragraph", "objects")
                return {"granted_by": source, "face": 0, "paragraph": producer["paragraph"]}
            ref = {"granted_by": self._record_id(by), **ref}
        elif origin not in ("own", "copied"):
            raise unsupported("unsupported_fact", f"an ability of origin {origin} has no MGN reference", "objects")
        elif origin == "own":
            item = self.audit["objects"].get(str(engine_source), {})
            definition = row.get("definition_identity") or {}
            # Entry-copy rows can be Own too. Only an authenticated physical
            # printing identifies a retained printed exception (707.9b).
            if (item.get("copy_acquisition") and item.get("physical_definition") is not None
                    and definition.get("card") == item["physical_definition"]):
                ref["of"] = "card"
        if row.get("keyword"):
            text = self._paragraph_text(engine_source, row["paragraph"], origin)
            pieces = [p.strip() for p in text.split(",")] if text else []
            if len(pieces) > 1 and all(spelled_keyword(p) for p in pieces):
                for sub, piece in enumerate(pieces, start=1):
                    if piece.lower().startswith(row["keyword"].lower()):
                        ref["sub"] = sub
                        break
        return ref

    def _frozen_ability_ref(self, source, index, origin, definition):
        """Match an acquisition to exact native definition evidence, not text."""
        def same_origin(have):
            if origin == "own":
                return have.get("kind") == "own"
            if not isinstance(origin, dict):
                return False
            if "copied" in origin:
                return have.get("kind") == "copied" and have.get("acquisition") == origin["copied"]
            if "rule" in origin:
                return have.get("kind") == "rule"
            if "given" in origin:
                return (have.get("kind") == "given" and have.get("by") == origin.get("by")
                        and have.get("acquisition") == origin.get("stamp") and have.get("part") == origin.get("part"))
            return False
        matches = [row for row in self.audit.get("objects", {}).get(str(source), {}).get("abilities", [])
                   if row.get("declaration_index") == index and row.get("definition_identity") == definition
                   and same_origin(row.get("origin", {}))]
        if len(matches) != 1:
            raise ValueError("the acquired definition has no unique authenticated source row")
        return self._ability_ref(matches[0], source)

    def _face_of(self, engine_id: int) -> int:
        if engine_id in self.causal_labels:
            label = self.causal_labels[engine_id]
            return self.setup_request["start"]["causal"]["objects"][label]["definition"].get("face", 0)
        item = self.audit["objects"].get(str(engine_id)) or {}
        if item.get("physical") is None:
            return 0
        record_id = self._record_id(engine_id)
        return self._face_index(record_id, item.get("properties", {}).get("name"), self.state["objects"].get(str(engine_id), {}))

    def _face_index(self, record_id: str, name: str | None, seen: dict) -> int:
        """Which face is up (712): a transformed permanent's back, or the face whose name the object
        bears (a modal double-faced card played or cast as its back face)."""
        if seen.get("transformed"):
            return 1
        faces = self.oracle.faces[self.cards[record_id]["oracle"]]
        for index, face in enumerate(faces):
            if index and name == face.name and name != faces[0].name:
                return index
        return 0

    def _paragraph_text(self, engine_source: int, paragraph: int, origin: str) -> str:
        rows = self.audit["objects"].get(str(engine_source), {}).get("definition_rows") or []
        if 0 < paragraph <= len(rows):
            return rows[paragraph - 1]
        return ""

    def _keywords(self, engine_id: int, item: dict, record_id: str) -> list[str]:
        """The final keyword abilities: the engine's `held` list, which holds the keywords it tracks
        as characteristics after continuous effects and the keyword lines of the object's current
        rules text for the ones it does not track."""
        held = item.get("held")
        if held is None:
            self.unsupported_paths.append(f"objects.{record_id}.keywords")
            return []
        out = [normalize_keyword(k) for k in held]
        # a keyword the engine reads as the ability its rule gives it rather than as a keyword line
        # ("Solved — [ability text]", 702.169a) is still a keyword ability the object has: it is
        # taken from the object's current rows when the engine never recognized it in the base
        modeled_names = {keyword_name(k) for k in (item.get("base") or {}).get("keywords", []) + held}
        for row in item.get("abilities", []):
            origin = row["origin"]["kind"]
            if origin == "rule" or (item.get("abilities_removed") and origin in ("own", "copied")):
                continue
            out.extend(k for k in keyword_row(row["text"]) if keyword_name(k) not in modeled_names)
        return sorted(out)

    def _token_definition(self, item: dict) -> dict:
        base = item.get("base")
        if not base:
            raise unsupported("unsupported_fact", "the engine does not report the token's definition", "objects.*.token")
        definition = {"name": base["name"], "types": sorted(t.lower() for t in base.get("types", [])),
                      "colors": sorted(base.get("colors", []))}
        if base.get("supertypes"):
            definition["supertypes"] = sorted(s.lower() for s in base["supertypes"])
        if base.get("subtypes"):
            definition["subtypes"] = sorted(canonical_subtype(s) for s in base["subtypes"])
        for key in ("power", "toughness"):
            if base.get(key) is not None:
                definition[key] = base[key]
        keywords = Counter(normalize_keyword(k) for k in base.get("keywords", []))
        row_keywords = Counter()
        text_rows = []
        for row in base.get("rows", []):
            if not row:
                continue
            row_keywords.update(keyword_row(row))
            # Printed keyword paragraphs participate in ability references too
            # (113.2c); dropping them renumbers every subsequent paragraph.
            text_rows.append(row)
        keywords |= row_keywords
        if keywords:
            definition["keywords"] = sorted(keywords.elements())
        predefined = predefined_tokens().get(base["name"])
        if predefined is not None and "text" in predefined:
            definition["text"] = predefined["text"]  # the rules' own wording of a predefined token (111.10)
        elif text_rows:
            # the engine prints an object's own name as "~"; a token's text names it "this token"
            definition["text"] = "\n".join(row.replace("~", "this token") for row in text_rows)
        return definition

    def _spell_copy_definition(self, engine_id, item, facts):
        """Project only authenticated frozen definition evidence, never final donor values."""
        record_id = self._record_id(engine_id)
        previous = self.projection_history
        if previous is not None and record_id in previous.copy_bases:
            return deepcopy(previous.reference_facts(record_id).get("copy_definition"))
        donor_id = item.get("stack_copy_of")
        donor = self.audit["objects"].get(str(donor_id), {})
        donor_record = facts["copy_of"]
        if previous is not None and donor_record in previous.copy_bases:
            return None  # the recorded copy already freezes its intrinsic definition
        ordinary = (donor.get("zone") == "Stack" and donor.get("physical") is not None
                    and not donor.get("token") and not donor.get("copy_acquisition"))
        donor_seen = self.state["objects"].get(str(donor_id), {})
        current_face = self._face_index(donor_record, donor.get("properties", {}).get("name"), donor_seen) if ordinary else None
        unchanged = previous is None or (previous.zone_of(donor_record) == "stack"
                     and previous.fact(donor_record, "face") == current_face
                     and bool(previous.fact(donor_record, "fused")) == (donor.get("cast_as") == "fused")
                     and not previous.fact(donor_record, "face_down"))
        if ordinary and unchanged and not donor_seen.get("hidden"):
            return None
        proof = item.get("spell_copy_definition") or {}
        if proof.get("unavailable") or type(proof.get("card")) is not int:
            raise unsupported("unsupported_fact", "spell copy's full frozen definition is unavailable", f"objects.{record_id}.copy_definition")
        try:
            # Catalog definitions name the physical front, while MGN's card
            # name joins both faces. This is frozen catalog identity, not a
            # lookup using the donor's current visible name.
            matches = [oid for oid, faces in self.oracle.faces.items()
                       if faces[0].name == proof["name"] or faces[0].raw["name"] == proof["name"]]
            if len(matches) != 1:
                raise ValueError("ambiguous or unavailable pinned catalog identity")
            oid = matches[0]
            face = proof["face"]
            if face == "Fused":
                if not self.oracle.has_fuse(oid):
                    raise ValueError("not a pinned fuse spell")
                return {"oracle": oid, "fused": True}
            if face in ("Whole", "Face(0)"):
                index = 0
            elif face in ("Adventure", "Omen", "back", "Face(1)"):
                index = 1
            elif face.startswith("Half("):
                index = int(face[5:-1])
            else:
                raise ValueError("unknown frozen spell face")
            if index >= self.oracle.face_count(oid):
                raise ValueError("missing pinned spell face")
            return {"oracle": oid, "face": index}
        except (KeyError, ValueError, TypeError):
            raise unsupported("unsupported_fact", "frozen native definition has no pinned Oracle face", f"objects.{record_id}.copy_definition")

    def _object_facts(self, engine_id: int, zone: str, problems: list) -> dict:
        item = self.audit["objects"][str(engine_id)]
        seen = self.state["objects"].get(str(engine_id), {})
        props = item["properties"]
        record_id = self._record_id(engine_id)
        facts: dict = {}
        on_stack = zone == "stack"
        permanent = zone == "battlefield"
        is_card = item["physical"] is not None
        if (on_stack or item.get("prepare_source") is not None) and not is_card and not item["token"]:
            occurrences = [o for o in item.get("occurrences", []) if o and not o.get("unavailable")]
            if item.get("stack_copy_of") is not None:
                facts.update({"kind": "copy", "copy_of": self._record_id(item["stack_copy_of"]), "controller": self.seats[props["controller"]]})
            elif item.get("prepare_source") is not None:
                facts.update({"kind": "copy", "copy_of": self._record_id(item["prepare_source"]),
                              "copy_face": 1, "controller": self.seats[props["controller"]]})
            elif seen.get("ability") and occurrences:
                occurrence = occurrences[0]
                if occurrence.get("source") is None:
                    rule = occurrence.get("rule_ref")
                    if not isinstance(rule, dict) or not source_free_rule(rule):
                        raise unsupported("unsupported_fact", "source-free ability lacks authenticated rule identity", f"objects.{record_id}")
                    facts.update({"kind": "ability", "ability": deepcopy(rule), "controller": self.seats[props["controller"]]})
                else:
                    ability = self._ability_ref(occurrence, occurrence["source"])
                    source = self._record_id(occurrence["source"])
                    facts.update({"kind": "ability", "source": source,
                                  "ability": ability, "controller": self.seats[props["controller"]]})
            elif item.get("copy_acquisition"):
                facts.update({"kind": "copy", "copy_of": self._record_id(item["copy_acquisition"]["donor"]), "controller": self.seats[props["controller"]]})
            elif not seen.get("ability") and seen.get("name"):
                # An older audit may retain the exact donor at the copy's target
                # question. A matching name is never evidence of copy identity.
                copied = self.copy_sources.get(engine_id)
                if copied is None:
                    raise unsupported("unsupported_fact", "the engine does not retain this spell copy's exact donor", f"objects.{record_id}.copy_of")
                facts.update({"kind": "copy", "copy_of": copied, "controller": self.seats[props["controller"]]})
            else:
                raise unsupported("unsupported_fact", "a stack object that is neither a card, an ability nor a copy", f"objects.{record_id}")
            if (facts.get("kind") == "copy" and item.get("stack_copy_of") is not None
                    and item.get("spell_copy_definition") is not None):
                definition = self._spell_copy_definition(engine_id, item, facts)
                if definition is not None:
                    facts["copy_definition"] = definition
                    oid = definition.get("oracle")
                    if (definition.get("face") == 1 and oid in self.oracle
                            and self.oracle.faces[oid][0].raw.get("layout") in ("transform", "modal_dfc")):
                        facts["face"] = 1
            if on_stack:
                self._stack_facts(engine_id, item, seen, facts, record_id)
            return facts
        if item["token"]:
            facts["token"] = self._token_definition(item)
            facts["controller"] = self.seats[props["controller"]]
            facts["owner"] = self.seats[item["owner"]]
        if seen.get("emblem"):
            facts["emblem"] = {"text": "\n".join(seen.get("abilities", []))}
            facts["controller"] = self.seats[props["controller"]]
            facts["owner"] = self.seats[item["owner"]]
            return facts
        if (permanent or on_stack) and "controller" not in facts:
            facts["controller"] = self.seats[props["controller"]]
        if item.get("copy_acquisition") and (permanent or on_stack):
            facts["copy_of"] = self._record_id(item["copy_acquisition"]["donor"])
        if is_card and (permanent or on_stack):
            face = self._face_index(record_id, props.get("name"), seen)
            if face:
                facts["face"] = face
        if zone == "exile":
            designations = [d for d in ("plotted", "foretold") if seen.get(d)]
            if designations and self.tier == 2:
                facts["designations"] = designations
            if seen.get("hidden") and self.tier == 2:
                facts["face_down"] = True
        if on_stack:
            self._stack_facts(engine_id, item, seen, facts, record_id)
        if permanent:
            combat = self.audit.get("combat", {})
            for attack in combat.get("attackers", []):
                if attack["attacker"] == engine_id:
                    facts["attacking"] = self._target_record(attack["defender"])
            if engine_id in combat.get("blocked", []):
                facts["blocked"] = True
            blocking = [self._record_id(a) for b, a in combat.get("blockers", []) if b == engine_id]
            if blocking:
                facts["blocking"] = blocking
            if seen.get("choices"):
                chosen = {}
                for choice in seen["choices"]:
                    value = choice.get("value", {})
                    for key, mapped in (("color", "color"), ("creature_type", "creature_type"), ("card_name", "card_name"),
                                        ("number", "number"), ("basic_land_type", "basic_land_type"), ("card_type", "card_type")):
                        if key in value:
                            domain = {"creature_type": "creature", "basic_land_type": "basic_land"}.get(key)
                            chosen[mapped] = canonical_subtype(value[key], domain) if domain else value[key]
                            break
                    else:
                        self.unsupported_paths.append(f"objects.{record_id}.chosen")
                if chosen:
                    facts["chosen"] = chosen
        if self.tier == 2 and (permanent or on_stack):
            if seen.get("face_down"):
                facts["face_down"] = True
            if permanent:
                facts["tapped"] = bool(props.get("tapped"))
                facts["damage"] = props.get("damage", 0)
                attached = seen.get("attached_to")
                facts["attached_to"] = self._target_record(attached) if attached else None
                if seen.get("level") is not None:
                    facts["level"] = seen["level"]
                designations = [d for d in DESIGNATIONS if seen.get(d) or props.get(d)]
                unlocked = seen.get("unlocked")
                if unlocked:
                    if unlocked[0]:
                        designations.append("left half unlocked")
                    if len(unlocked) > 1 and unlocked[1]:
                        designations.append("right half unlocked")
                for extra in ("monstrous", "renowned"):
                    if seen.get(extra):
                        self.unsupported_paths.append(f"objects.{record_id}.designations")
                if designations:
                    facts["designations"] = sorted(designations)
            if props.get("counters"):
                facts["counters"] = dict(props["counters"])
            types = {t.lower() for t in props.get("types", [])}
            for key in ("power", "toughness"):
                if props.get(key) is None:
                    continue
                # a noncreature permanent has no power or toughness, whatever its face prints (208.3)
                facts[key] = None if permanent and "creature" not in types else props[key]
            facts["colors"] = sorted(props.get("colors", []))
            facts["types"] = sorted(t.lower() for t in props.get("types", []))
            facts["subtypes"] = sorted(canonical_subtype(s) for s in props.get("subtypes", []))
            if "supertypes" in props:
                facts["supertypes"] = sorted(s.lower() for s in props["supertypes"])
            else:
                self.unsupported_paths.append(f"objects.{record_id}.supertypes")
            facts["keywords"] = self._keywords(engine_id, item, record_id)
            granted = [r["text"] for r in item.get("abilities", []) if r["origin"]["kind"] == "given" and not r.get("keyword")
                       and not self._is_keyword_line(r["text"])]
            if granted:
                facts["granted"] = granted
        return facts

    @staticmethod
    def _is_keyword_line(text: str) -> bool:
        keywords = keyword_row(text)
        # A wrapper names a keyword but its following rules text still defines
        # what that particular ability does (e.g. solved, CR 702.169a).
        return bool(keywords) and not any(keyword_name(k) in TEXT_KEYWORDS for k in keywords)

    def _stack_facts(self, engine_id: int, item: dict, seen: dict, facts: dict, record_id: str):
        targets = seen.get("targets", [])
        if targets:
            facts["targets"] = [[self._target_record(t) for t in group] for group in targets]
        if item.get("cast_as") == "fused":
            facts["fused"] = True
        announced = item.get("announced") or {}
        if announced.get("modes"):
            facts["modes"] = [m + 1 for m in announced["modes"]]
        if announced.get("x") is not None:
            facts["x"] = announced["x"]
        if announced.get("division"):
            facts["division"] = deepcopy(announced["division"])
        if (facts.get("kind") is None and item.get("physical") is not None) or item.get("stack_copy_of") is not None:
            cast = self._cast_from_announced(item, record_id)
            if engine_id in self.free_casts:
                cast["cost"] = {"kind": "alternative", "effect": "without_paying"}
            if cast:
                facts["cast"] = cast

    def _cast_route_from_announced(self, item: dict, record_id: str) -> dict:
        """Map native accepted provenance, including completed substitution receipts."""
        retained = (item.get("announced") or {}).get("cast")
        if retained is None:
            return {}
        if not isinstance(retained, dict) or not isinstance(retained.get("route"), dict):
            raise unsupported("unsupported_fact", "accepted casting route is unavailable", f"objects.{record_id}.cast")
        route = retained["route"]
        kind = route.get("kind")
        if kind == "flashback" and "cost_index" in route:
            raise unsupported("unsupported_fact", "distinct native flashback cost choices have no MGN representation", f"objects.{record_id}.cast.cost")
        out: dict = {}
        if kind in ("flashback", "escape", "foretell", "harmonize"):
            out["cost"] = {"kind": "alternative", "keyword": kind}
            if kind == "harmonize" and route.get("tapped") is not None:
                out["cost"]["tapped"] = self._record_id(route["tapped"])
        elif kind == "permission" and item.get("stack_copy_of") is not None:
            pass  # the copied choices do not grant this copy a casting permission
        elif kind == "permission":
            owner = retained.get("permission") or {}
            if owner.get("unavailable") or owner.get("source") != route.get("source") or not owner.get("definition_identity") or not owner.get("origin"):
                raise unsupported("unsupported_fact", "casting permission exact frozen owner unavailable", f"objects.{record_id}.cast.permission")
            out["permission"] = {"source": self._record_id(owner["source"]), "ability": self._ability_ref(owner, owner["source"])}
        elif kind == "retrace" and item.get("stack_copy_of") is not None:
            pass
        elif kind == "retrace":
            if "unavailable" in (item.get("casting_costs") or {}):
                raise unsupported("unsupported_fact", "retrace declaring definition is unavailable", f"objects.{record_id}.cast.permission")
            rows = [row for row in item.get("abilities", []) if (row.get("keyword") or "").lower() == "retrace"]
            if not rows:
                rows = [row for row in item.get("abilities", []) if keyword_name(row.get("text") or "") == "retrace"]
            if len(rows) != 1 or rows[0].get("source") is None:
                raise unsupported("unsupported_fact", "retrace needs one authenticated declaring ability", f"objects.{record_id}.cast.permission")
            owner = rows[0]
            out["permission"] = {"source": record_id, "ability": self._ability_ref(owner, owner["source"])}
        elif kind != "normal":
            raise unsupported("unsupported_fact", "accepted casting route has no portable representation", f"objects.{record_id}.cast")
        if item.get("stack_copy_of") is not None:
            out.pop("permission", None)
            return out  # a copy did not cast, tap creatures, exile cards or use a permission
        payment = retained.get("payment")
        if retained.get("completed") is not True:
            if payment is not None:
                raise unsupported("unsupported_fact", "incomplete cast claims a completed substitution payment", f"objects.{record_id}.cast")
            return out
        if not isinstance(payment, dict) or set(payment) != {"convoke", "delve"} or not isinstance(payment["convoke"], list) or not isinstance(payment["delve"], list):
            raise unsupported("unsupported_fact", "completed casting payment receipt is unavailable", f"objects.{record_id}.cast")
        convoke, delve = payment["convoke"], payment["delve"]
        if any(not isinstance(part, dict) or set(part) != {"object", "color"} or type(part["object"]) is not int
               or part["color"] not in (None, "W", "U", "B", "R", "G") for part in convoke) or any(type(obj) is not int for obj in delve):
            raise unsupported("unsupported_fact", "casting substitution receipt is malformed", f"objects.{record_id}.cast")
        resources = [part["object"] for part in convoke] + delve
        if len(resources) != len(set(resources)):
            raise unsupported("unsupported_fact", "casting substitution receipt reuses a selected object", f"objects.{record_id}.cast")
        if convoke:
            out["convoke"] = [{"object": self._record_id(part["object"]), "mana": part["color"]} for part in convoke]
        if delve:
            out["delve"] = [self._record_id(obj) for obj in delve]
        return out

    def _cast_from_announced(self, item: dict, record_id: str) -> dict:
        """Casting choices and completed substitutions retained by their owning mechanisms."""
        costs = (item.get("announced") or {}).get("costs") or {}
        if "unavailable" in costs:
            self.unsupported_paths.append(f"objects.{record_id}.cast")
            return {}
        additional: dict = {}
        if costs.get("kicks"):
            additional["kicker"] = costs["kicks"]
        for key in ("buyback", "entwine"):
            if costs.get(key):
                additional[key] = True
        if costs.get("replicate"):
            additional["replicate"] = costs["replicate"]
        optional = costs.get("optional")
        if optional and optional.get("kind"):
            additional["paid"] = [optional["kind"]]
        if costs.get("gift") is not None:
            additional["gift"] = self.seats[costs["gift"]]
        branches = (item.get("announced") or {}).get("cost_branches") or []
        if isinstance(branches, list) and branches:
            choices = [b.get("branch", {}).get("arm") for b in branches]
            if all(isinstance(c, int) for c in choices):
                additional["choices"] = choices
        out: dict = {}
        alternative = costs.get("alternative")
        if alternative is not None:
            text = (alternative.get("text") or "") if isinstance(alternative, dict) else ""
            keyword = spelled_keyword(text) if text else None
            if keyword:
                out["cost"] = {"kind": "alternative", "keyword": keyword}
            elif isinstance(alternative, dict) and alternative.get("paragraph") is not None and alternative.get("source") is not None:
                source = alternative["source"]
                out["cost"] = {"kind": "alternative", "source": self._record_id(source),
                               "ability": self._ability_ref(alternative, source)}
            else:
                self.unsupported_paths.append(f"objects.{record_id}.cast")
        if additional:
            out["additional"] = additional
        try:
            route = self._cast_route_from_announced(item, record_id)
            if "cost" in out and "cost" in route and out["cost"] != route["cost"]:
                raise unsupported("unsupported_fact", "retained alternative cost and casting route disagree", f"objects.{record_id}.cast")
            out.update(route)
        except Refusal as error:
            self.unsupported_paths.append(error.path or f"objects.{record_id}.cast")
        return out

    def _restricted_lot(self, unit):
        """Use immutable production evidence, including choices made before source exit."""
        proof = unit.get("provenance")
        if not isinstance(proof, dict) or not isinstance(proof.get("restriction"), str):
            raise PaymentEvidenceError("restricted mana lacks authenticated production-time terms")
        row = proof.get("ability", {})
        definition = row.get("definition_identity") or {}
        # Printed whole/front and back faces have stable portable references.
        # Copied/granted text requires an acquisition reference, not current text.
        face = definition.get("face")
        if (row.get("origin", {}).get("kind") != "own" or face not in ("Whole", "back") or not row.get("paragraph")
                or type(proof.get("physical_definition")) is not int
                or definition.get("card") != proof["physical_definition"]):
            raise PaymentEvidenceError("restricted producer is not an authenticated printed whole/back-face ability")
        source = self._record_id(row["source"])
        face_index = 1 if face == "back" else 0
        paragraph = row["paragraph"]
        try:
            # The frozen physical definition authenticates the printing even if
            # this incarnation has departed or the current source changed face.
            rows = self.oracle.paragraphs(self.cards[source]["oracle"], face_index)
            if type(paragraph) is not int or not 0 < paragraph <= len(rows):
                raise KeyError("printed producing paragraph is absent")
            restriction = rows[paragraph - 1]
        except (KeyError, TypeError) as error:
            raise PaymentEvidenceError("restricted producer lacks its pinned Oracle paragraph") from error
        lot = {"type": unit["mana"], "restriction": restriction,
               "producer": {"source": source,
                            "ability": {"face": face_index, "paragraph": paragraph}}}
        chosen = deepcopy(proof.get("chosen", {}))
        for key, domain in (("creature_type", "creature"), ("basic_land_type", "basic_land")):
            if key in chosen:
                chosen[key] = canonical_subtype(chosen[key], domain)
        if chosen:
            lot["chosen"] = chosen
        return lot

    def _players(self) -> dict:
        out = {}
        for index, seat in enumerate(self.seats):
            player = self.state["players"][index]
            facts = {"life": player["life"]}
            if seat in self.conceded:
                facts["left"] = True  # a player who concedes leaves the game (104.3a)
            if self.tier == 2:
                if player.get("counters"):
                    facts["counters"] = dict(player["counters"])
                pool = player.get("pool", {})
                mana = {k: pool.get(k, 0) for k in MANA_TYPES if pool.get(k)}
                if mana:
                    facts["mana"] = mana
                if pool.get("restricted"):
                    try:
                        lots = {}
                        for unit in pool["restricted"]:
                            lot = self._restricted_lot(unit)
                            key = json.dumps(lot, sort_keys=True)
                            if key not in lots:
                                lots[key] = {**lot, "count": 0}
                            lots[key]["count"] += 1
                        facts["mana_restricted"] = list(lots.values())
                    except (PaymentEvidenceError, Refusal):
                        self.unsupported_paths.append(f"players.{seat}.mana_restricted")
                if player.get("speed") is not None:
                    facts["speed"] = player["speed"]
                if "maximum_hand_size" not in player:
                    self.unsupported_paths.append(f"players.{seat}.max_hand_size")
                elif player["maximum_hand_size"] != 7:
                    facts["max_hand_size"] = player["maximum_hand_size"]
            out[seat] = facts
        return out

    def _record_state(self) -> dict:
        """The engine's state in the record form of spec 3.4, with the record's ids."""
        self.unsupported_paths = []
        discovered = len(self.departed_tokens)
        problems: list = []
        synthetic: dict = {}
        pending = self._pending(problems, synthetic)
        state = {"turn": self._turn(), "pending": pending, "result": self._result(), "players": self._players(), "zones": {}, "objects": {}}
        if self.tier == 2:
            monarch = self.state.get("monarch")
            state["game"] = {"monarch": self.seats[monarch] if monarch is not None else None, "initiative": None, "day_night": None}
        listings = {}
        for index, seat in enumerate(self.seats):
            listings[f"{seat}.library"] = list(self.libraries[index])
            listings[f"{seat}.hand"] = list(self.state["players"][index]["hand"])
            listings[f"{seat}.graveyard"] = list(self.state["players"][index]["graveyard"])
        for zone in ("battlefield", "stack", "exile", "command"):
            listings[zone] = list(self.state[zone])
        for zone, members in listings.items():
            state["zones"][zone] = [self._record_id(e) for e in members]
            for engine_id in members:
                facts = self._object_facts(engine_id, zone.split(".")[-1], problems)
                if facts:
                    state["objects"][self._record_id(engine_id)] = facts
        for stack_id, facts in synthetic.items():
            state["zones"]["stack"].append(stack_id)
            state["objects"][stack_id] = facts
        if len(self.departed_tokens) != discovered:
            self._refresh_ids(keep_departed=True)
            return self._record_state()
        if self.departed_tokens:
            state["departed"] = {self.tokens[e]: deepcopy(facts) for e, facts in self.departed_tokens.items()}
        return state

    def _reply_state(self, with_unused: bool = False) -> dict:
        if self.pregame is not None:
            self.unsupported_paths = []
            record_state = self._pregame_state()
        else:
            record_state = self._record_state()
        problems: list = []
        # a sideboard card brought into the game (400.11b) is in a zone from then on; the record's
        # own start rules (spec 3.4) would refuse it there, so the model sees it as a main card
        cards = {card_id: ({**card, "from": "main"} if card.get("from") == "sideboard" and card_id in self.physical else card)
                 for card_id, card in self.cards.items()}
        record = {"tier": self.tier, "game": {"seats": self.seats, "life": self.life, "format": self.format}, "cards": cards,
                  "start": {"kind": "position", "state": record_state}}
        if self.setup_request is not None and self.setup_request["start"].get("causal"):
            record["start"]["causal"] = self.setup_request["start"]["causal"]
        state = State.from_observation(record, problems, self.projection_history)
        errors = [p for p in problems if p.severity == "error"]
        if errors:
            raise fault("engine_fault", "the projection is not a consistent MGN state: " + "; ".join(str(p) for p in errors[:4]))
        self.last_projection = state
        out = {"state": state.explicit(self.oracle)}
        if self.initial_causal is not None:
            from mgn.causal import readback
            out["causal"] = readback(self)
        # the cards that are new objects since the last reply (400.7): they changed zone, or left a
        # zone and came back within this request, which the states alone would not show
        renewed = sorted(rid for rid, eid in self.current.items() if rid in self.ids_at_reply and self.ids_at_reply[rid] != eid)
        self.ids_at_reply = dict(self.current)
        if renewed and not getattr(self, "restoring", False):
            out["renewed"] = renewed
        unsupported_paths = sorted(set(self.unsupported_paths))
        if unsupported_paths:
            out["unsupported"] = unsupported_paths
        if with_unused:
            unused = list(self.outcomes) + list(getattr(self, "unused_now", []))
            self.outcomes = []
            self.unused_now = []
            if unused:
                out["random_unused"] = unused
        return out


# ----- the protocol loop -----

def serve(adapter: KarnAdapter, reader, writer):
    """Reads requests line by line, writes replies; interim random requests go to the same writer."""

    def say(message: dict):
        writer.write(json.dumps(message, ensure_ascii=False) + "\n")
        writer.flush()

    for line in reader:
        line = line.strip()
        if not line:
            continue
        try:
            request = json.loads(line)
        except ValueError as error:
            say({"id": 0, "error": {"class": "fault", "code": "protocol", "message": f"not JSON: {error}"}})
            continue
        request_id = request.get("id")

        def ask(key: dict, _id=request_id):
            say({"id": _id, "random": key})
            while True:
                answer = reader.readline()
                if not answer:
                    raise fault("protocol", "the runner closed the stream while an outcome was asked for")
                answer = json.loads(answer)
                if answer.get("id") != _id or "outcome" not in answer:
                    raise fault("protocol", "an outcome was expected")
                return answer["outcome"]

        try:
            if "hello" in request:
                say({"id": request_id, "capabilities": adapter.capabilities()})
            elif "setup" in request:
                say({"id": request_id, "ok": adapter.setup(request["setup"])})
            elif "act" in request:
                step = request["act"]
                say({"id": request_id, "ok": adapter.act(step["n"], step["seat"], step["do"], ask)})
            elif "attempt" in request:
                step = request["attempt"]
                say({"id": request_id, "ok": adapter.attempt(step["n"], step["seat"], step["do"], ask)})
            elif "default" in request:
                say({"id": request_id, "ok": adapter.default(request["default"].get("until"), ask)})
            elif "project" in request:
                say({"id": request_id, "ok": adapter.project(request["project"]["tier"])})
            elif "end" in request:
                adapter.end()
                say({"id": request_id, "ok": {}})
            else:
                say({"id": request_id, "error": {"class": "fault", "code": "protocol", "message": "unknown request"}})
        except Refusal as refusal:
            key = "error" if "setup" in request else "refused"
            say({"id": request_id, key: refusal.json()})
        except Exception as error:  # noqa: BLE001 - every failure must reach the runner as a fault
            key = "error" if "setup" in request else "refused"
            say({"id": request_id, key: {"class": "fault", "code": "engine_fault", "message": f"{type(error).__name__}: {error}"}})


def main(argv=None) -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--engine", type=Path, default=ROOT / "target/fast/game-serve")
    parser.add_argument("--oracle", type=Path, default=CARDS)
    parser.add_argument("--timeout", type=float, default=30.0)
    args = parser.parse_args(argv)
    adapter = KarnAdapter(args.engine, Oracle(args.oracle), args.timeout)
    try:
        serve(adapter, sys.stdin, sys.stdout)
    finally:
        adapter.end()
    return 0


if __name__ == "__main__":
    sys.exit(main())
