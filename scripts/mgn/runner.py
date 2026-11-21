#!/usr/bin/env python3
"""The MGN verification runner (docs/mgn/verification.md): replays a record on one adapter and
writes a divergence report (docs/mgn/schema/divergence-report.schema.json).

    uv run scripts/mgn/runner.py tests/fixtures/mgn/bant-ordered-arrivals/record.json --engine target/fast/game-serve
    uv run scripts/mgn/runner.py --active --out .cache/mgn-reports

The runner validates the record without an engine (L0), then drives the adapter transition by
transition: it sends each decision or attempt, answers the adapter's random requests from the
transition's outcomes by key, and moves through the stretch to the next boundary by having the
adapter give null answers until the engine's projection agrees with the expected state on every
tier-1 fact (the first such state is the boundary, spec 4.2), bounded by the order of turns and
steps. At the boundary it compares fact by fact at levels L2 to L4 and the checkpoint hash at L5.
It never aborts on a divergence; it stops only when the engine can no longer follow the record.
"""
from __future__ import annotations

import argparse
from copy import deepcopy
import json
import os
import re
from pathlib import Path
import subprocess
import sys
import time

ROOT = Path(__file__).resolve().parents[2]
if __name__ == "__main__" and str(ROOT / "scripts") not in sys.path:
    sys.path.insert(0, str(ROOT / "scripts"))

from mgn.canonical import loads, sha256  # noqa: E402
from mgn.lots import normalize_state_lots  # noqa: E402
from mgn.identity import game_identity, record_identity  # noqa: E402
from mgn.oracle import Oracle  # noqa: E402
from mgn.randomness import matches_request, outcome_key
from mgn.schema import validate as schema_validate  # noqa: E402
from mgn.state import GAME_DEFAULTS, OBJECT_DEFAULTS, PLAYER_DEFAULTS, State, TIER1_OBJECT, TIER1_PLAYER  # noqa: E402
from mgn.validate import validate  # noqa: E402
from mgn.vocabulary import STEPS  # noqa: E402

STEP_INDEX = {step: i for i, step in enumerate(STEPS)}
LEVELS = ("L0", "L1", "L2", "L3", "L4", "L5")
DEFAULT_ENGINE = ROOT / "target/fast/game-serve"
KARN_ADAPTER = ROOT / "scripts/mgn/adapters/karn.py"


class AdapterError(Exception):
    """The adapter process failed the protocol."""


class Adapter:
    """One adapter process spoken to in JSON lines; `random` interim messages are answered by a
    callback."""

    def __init__(self, command: list[str], timeout: float = 120.0):
        self.command = command
        self.timeout = timeout
        self.process = subprocess.Popen(command, stdin=subprocess.PIPE, stdout=subprocess.PIPE, text=True,
                                        bufsize=1, cwd=str(ROOT))
        self.next_id = 0

    def request(self, body: dict, on_random=None) -> dict:
        self.next_id += 1
        message = {"id": self.next_id, **body}
        self._write(message)
        while True:
            reply = self._read()
            if reply.get("id") != self.next_id:
                raise AdapterError(f"reply {reply.get('id')} to request {self.next_id}")
            if "random" in reply:
                outcome = on_random(reply["random"]) if on_random else None
                self._write({"id": self.next_id, "outcome": outcome})
                continue
            return reply

    def _write(self, message: dict):
        try:
            self.process.stdin.write(json.dumps(message, ensure_ascii=False) + "\n")
            self.process.stdin.flush()
        except (BrokenPipeError, OSError) as error:
            raise AdapterError(f"the adapter closed its input: {error}")

    def _read(self) -> dict:
        line = self.process.stdout.readline()
        if not line:
            raise AdapterError("the adapter exited")
        try:
            return json.loads(line)
        except ValueError as error:
            raise AdapterError(f"the adapter wrote something that is not JSON: {error}")

    def close(self):
        try:
            if self.process.stdin and not self.process.stdin.closed:
                self.process.stdin.close()
            self.process.wait(timeout=5)
        except (OSError, subprocess.TimeoutExpired):
            self.process.kill()
        finally:
            if self.process.stdout:
                self.process.stdout.close()


def karn_adapter_command(engine: Path) -> list[str]:
    return [sys.executable, str(KARN_ADAPTER), "--engine", str(engine)]


# ----- comparison -----

def _matches(pattern: str, path: str) -> bool:
    want, have = pattern.split("."), path.split(".")
    if len(want) != len(have):
        return False
    return all(w == "*" or w == h for w, h in zip(want, have))


def _is_default(path: str, value, holder: dict | None = None) -> bool:
    parts = path.split(".")
    if parts[0] == "players" and len(parts) == 3:
        return parts[2] in PLAYER_DEFAULTS and value == PLAYER_DEFAULTS[parts[2]]
    if parts[0] == "objects" and len(parts) == 3:
        if parts[2] == "in_combat":  # derived: attacking or blocking (506.4; spec 3.1)
            holder = holder or {}
            return value == (holder.get("attacking") is not None or bool(holder.get("blocking")))
        return parts[2] in OBJECT_DEFAULTS and value == OBJECT_DEFAULTS[parts[2]]
    if parts[0] == "objects" and len(parts) == 4 and parts[2] == "cast":
        return value is None
    if parts[0] == "game" and len(parts) == 2:
        return parts[1] in GAME_DEFAULTS and value == GAME_DEFAULTS[parts[1]]
    return False


def compare(expected: dict, actual: dict, unsupported: list[str], tier: int, *, oracle=None, cards=None) -> tuple[list[dict], list[dict]]:
    """Every difference between two explicit projections, as `{level, path, expected, actual}`, and
    the facts the adapter could not project, as `{level, path}` (inconclusive)."""
    expected, actual = normalize_state_lots(expected), normalize_state_lots(actual)
    if cards is not None:
        from mgn.copy_definition import comparison_projection
        expected = comparison_projection(expected, cards, oracle)
        actual = comparison_projection(actual, cards, oracle)
    diffs: list[dict] = []
    inconclusive: list[dict] = []

    def level_of(path: str) -> str:
        parts = path.split(".")
        if parts[0] in ("turn", "pending"):
            return "L2"
        if parts[0] == "result":
            return "L1"
        if parts[0] == "zones":
            return "L3"
        if parts[0] == "players":
            return "L3" if len(parts) == 3 and parts[2] in TIER1_PLAYER else "L4"
        if parts[0] == "objects":
            if len(parts) == 3:
                return "L3" if parts[2] in TIER1_OBJECT or parts[2] == "owner" else "L4"
            return "L3"
        if parts[0] == "game":
            return "L4"
        return "L3"

    def check(path: str, want, have, holder: dict | None = None):
        if any(_matches(pattern, path) for pattern in unsupported):
            if want is not None and not _is_default(path, want, holder):
                inconclusive.append({"level": level_of(path), "path": path})  # the record relies on the fact
            return
        if want != have:
            diffs.append({"level": level_of(path), "path": path, "expected": deepcopy(want), "actual": deepcopy(have)})

    def unordered(pending):
        # the abilities awaiting placement are listed in any order (spec 5.4: compared as a multiset)
        if isinstance(pending, dict) and pending.get("kind") == "order_triggers" and isinstance((pending.get("about") or {}).get("abilities"), list):
            pending = deepcopy(pending)
            pending["about"]["abilities"] = sorted(pending["about"]["abilities"], key=lambda i: json.dumps(i, sort_keys=True))
        return pending

    for key in ("turn", "pending", "result"):
        want, have = expected.get(key), actual.get(key)
        if key == "pending":
            want, have = unordered(want), unordered(have)
        check(key, want, have)
    for seat in sorted(set(expected.get("players", {})) | set(actual.get("players", {}))):
        want, have = expected.get("players", {}).get(seat, {}), actual.get("players", {}).get(seat, {})
        for fact in sorted(set(want) | set(have)):
            check(f"players.{seat}.{fact}", want.get(fact), have.get(fact))
    for zone in sorted(set(expected.get("zones", {})) | set(actual.get("zones", {}))):
        check(f"zones.{zone}", expected.get("zones", {}).get(zone), actual.get("zones", {}).get(zone))
    for object_id in sorted(set(expected.get("objects", {})) | set(actual.get("objects", {}))):
        want, have = expected.get("objects", {}).get(object_id), actual.get("objects", {}).get(object_id)
        if want is None or have is None:
            # Full states are sparse (spec 3.4): a physical card with only default
            # facts has no row. Its existence is independently stated by zones.
            # Generated identities always require their defining row (spec 3.1).
            present_in_both = all(any(object_id in ids for ids in state.get("zones", {}).values())
                                  for state in (expected, actual))
            if not present_in_both or re.fullmatch(r"[TSE][1-9][0-9]*", object_id):
                check(f"objects.{object_id}", want, have)
                continue
            want, have = want or {}, have or {}
        for fact in sorted(set(want) | set(have)):
            if fact == "cast":  # its members are facts of their own, so one of them can be unsupported
                want_cast, have_cast = want.get("cast") or {}, have.get("cast") or {}
                for member in sorted(set(want_cast) | set(have_cast)):
                    check(f"objects.{object_id}.cast.{member}", want_cast.get(member), have_cast.get(member))
                continue
            check(f"objects.{object_id}.{fact}", want.get(fact), have.get(fact), want)
    for object_id in sorted(set(expected.get("departed", {})) | set(actual.get("departed", {}))):
        check(f"departed.{object_id}", expected.get("departed", {}).get(object_id), actual.get("departed", {}).get(object_id))
    if tier == 2:
        for fact in sorted(set(expected.get("game", {})) | set(actual.get("game", {}))):
            check(f"game.{fact}", expected.get("game", {}).get(fact), actual.get("game", {}).get(fact))
    return diffs, inconclusive


def turn_key(state: dict) -> tuple:
    turn = state["turn"]
    return (turn["number"], STEP_INDEX.get(turn["step"], -1), turn.get("occurrence", 1))


def past_boundary(seen: list[tuple], actual: dict, target: dict) -> bool:
    """Is the engine's turn past the boundary's? A later turn; or, in the same turn, a step the
    runner saw begin after the boundary's step (`seen`: the (step, occurrence) pairs of this turn in
    the order observed); or, when neither step is one an effect added (both the first of their
    name), a later step in the order of 500.1. Steps an effect added (500.8, 500.9) have no fixed
    place in that order, so with them the turn number alone is the bound."""
    turn, want = actual["turn"], target["turn"]
    if turn["number"] != want["number"]:
        return turn["number"] > want["number"]
    pair = (turn["step"], turn.get("occurrence", 1))
    goal = (want["step"], want.get("occurrence", 1))
    if pair == goal:
        return False
    if goal in seen and pair in seen:
        return seen.index(pair) > seen.index(goal)
    if pair[1] == 1 and goal[1] == 1:
        return STEP_INDEX.get(pair[0], -1) > STEP_INDEX.get(goal[0], -1)
    return False


# ----- the run -----

class Run:
    def __init__(self, record: dict, oracle: Oracle, adapter_command: list[str], timeout: float = 120.0):
        self.record = record
        self.oracle = oracle
        self.adapter_command = adapter_command
        self.timeout = timeout
        self.divergences: list[dict] = []
        self.inconclusive: list[dict] = []
        self.refusals: list[dict] = []
        self.first_seen: set[str] = set()
        self.levels = {level: "pass" for level in LEVELS}
        self.unsupported_global: list[str] = []
        self.stopped_at: int | None = None
        self.replayed = 0
        self.queue: dict[tuple, list[dict]] = {}
        self.handed: list[tuple[tuple, dict]] = []
        self.notes: list[str] = []

    # ----- bookkeeping -----

    def _divergence(self, after: int, diff: dict):
        first = diff["level"] not in self.first_seen
        self.first_seen.add(diff["level"])
        self.divergences.append({"after": after, **diff, "first": first})
        self.levels[diff["level"]] = "fail"

    def _inconclusive(self, after: int, level: str, path: str, reason: str):
        self.inconclusive.append({"after": after, "level": level, "path": path, "reason": reason})
        if self.levels[level] == "pass":
            self.levels[level] = "inconclusive"

    def _refusal(self, after: int, failure: dict):
        self.refusals.append({"after": after, "class": failure["class"], "code": failure["code"], "message": failure.get("message", "")})
        if failure["class"] == "illegal":
            self.levels["L1"] = "fail"
        elif failure["class"] == "unsupported":
            for level in ("L1", "L2", "L3", "L4", "L5"):
                if self.levels[level] == "pass":
                    self.levels[level] = "inconclusive"
        else:
            for level in ("L1", "L2", "L3", "L4", "L5"):
                if self.levels[level] == "pass":
                    self.levels[level] = "inconclusive"

    # ----- random outcomes -----

    @staticmethod
    def _key(outcome: dict) -> tuple:
        return outcome_key(outcome)

    def _load_random(self, outcomes: list):
        self.queue = {}
        self.handed = []
        for outcome in outcomes:
            self.queue.setdefault(self._key(outcome), []).append(outcome)

    def _on_random(self, request: dict):
        for key, waiting in self.queue.items():
            if waiting and matches_request(waiting[0], request):
                outcome = waiting.pop(0)
                self.handed.append((key, outcome))
                return deepcopy(outcome)
        return None

    def _take_back(self, unused: list):
        for outcome in reversed(unused):
            for index in range(len(self.handed) - 1, -1, -1):
                key, handed = self.handed[index]
                if handed == outcome:
                    del self.handed[index]
                    self.queue.setdefault(key, []).insert(0, handed)
                    break

    def _check_random_consumed(self, after: int):
        for key, waiting in self.queue.items():
            for outcome in waiting:
                self._divergence(after, {"level": "L1", "path": "random", "expected": {"kind": outcome["kind"], "seat": outcome.get("seat"), "consumed": True},
                                         "actual": "never asked for"})

    # ----- the protocol calls -----

    def _call(self, adapter: Adapter, body: dict) -> dict:
        reply = adapter.request(body, self._on_random)
        ok = reply.get("ok")
        if isinstance(ok, dict) and ok.get("random_unused"):
            self._take_back(ok["random_unused"])
        return reply

    def run(self) -> dict:
        started = time.monotonic()
        record = self.record
        problems = validate(record, self.oracle)
        errors = [p for p in problems if p.severity == "error"]
        report = {"mgn_report": "1", "record": record_identity(record), "game": game_identity(record),
                  "engine": {"name": "unknown", "version": "unknown"}, "adapter": {"name": "unknown", "version": "unknown"},
                  "tier": record["tier"], "levels": self.levels, "transitions": len(record["transitions"]), "replayed": 0,
                  "stopped_at": None, "divergences": self.divergences, "inconclusive": self.inconclusive, "refusals": self.refusals}
        if errors:
            self.levels["L0"] = "fail"
            for level in LEVELS[1:]:
                self.levels[level] = "not_applicable"
            report["usage"] = {"seconds": round(time.monotonic() - started, 3), "l0": [str(p) for p in errors[:20]]}
            return report
        if record["tier"] == 1:
            self.levels["L4"] = "not_applicable"
        if not record.get("checkpoints"):
            self.levels["L5"] = "not_applicable"
        adapter = Adapter(self.adapter_command, self.timeout)
        try:
            self._drive(adapter, report)
        except AdapterError as error:
            self._refusal(self.replayed, {"class": "fault", "code": "engine_fault", "message": str(error)})
            self.stopped_at = self.replayed
        finally:
            try:
                adapter.request({"end": {}})
            except AdapterError:
                pass
            adapter.close()
        report["replayed"] = self.replayed
        report["stopped_at"] = self.stopped_at
        if self.unsupported_global:
            report["unsupported"] = self.unsupported_global
        report["usage"] = {"seconds": round(time.monotonic() - started, 3)}
        if self.notes:
            report["usage"]["notes"] = self.notes
        return report

    def _setup_causal(self, reply: dict) -> bool:
        """Check declared setup facts, not the adapter's native construction proof."""
        expected = self.record["start"].get("causal")
        if expected is None:
            return True
        errors = schema_validate(reply, "adapter-protocol.schema.json")
        if errors or not isinstance(reply.get("ok", {}).get("state"), dict):
            detail = "; ".join(str(error) for error in errors[:3]) or "missing object ok.state"
            self._refusal(0, {"class":"fault", "code":"protocol",
                "message":f"malformed causal setup reply: {detail}"})
            self.stopped_at = 0
            return False
        if "causal" not in reply["ok"]:
            self._refusal(0, {"class":"unsupported", "code":"unsupported_start",
                "message":"required start.causal has no independently read-back setup causal payload"})
            self.stopped_at = 0
            return False
        actual = reply["ok"]["causal"]
        changed = False
        def compare_fact(path, want, have):
            if isinstance(want, dict) and isinstance(have, dict):
                for key in sorted(set(want) | set(have)):
                    if key not in want or key not in have:
                        difference(f"{path}.{key}", want.get(key), have.get(key))
                    else:
                        compare_fact(f"{path}.{key}", want[key], have[key])
            elif isinstance(want, list) and isinstance(have, list) and len(want) == len(have):
                for index, (left, right) in enumerate(zip(want, have)):
                    compare_fact(f"{path}.{index}", left, right)
            elif want != have:
                difference(path, want, have)
        def difference(path, want, have):
            nonlocal changed
            changed = True
            self._divergence(0, {"level":"L1", "path":path, "expected":want, "actual":have})
        # Optional absent categories make no claim about that historical category.
        # Within each declared category, identity, multiplicity and order are exact.
        for family, wanted in expected.items():
            if family not in actual:
                difference(f"start.causal.{family}", wanted, None)
            else:
                compare_fact(f"start.causal.{family}", wanted, actual[family])
        if changed:
            self.stopped_at = 0
        return not changed

    def _drive(self, adapter: Adapter, report: dict):
        record = self.record
        hello = adapter.request({"hello": {"mgn": "1"}})
        capabilities = hello.get("capabilities")
        if not capabilities:
            raise AdapterError(f"no capabilities: {json.dumps(hello)[:200]}")
        report["engine"] = capabilities["engine"]
        report["adapter"] = capabilities["adapter"]
        self.unsupported_global = list(capabilities.get("unsupported_facts", []))
        if record["tier"] not in capabilities["tiers"]:
            self._refusal(0, {"class": "unsupported", "code": "unsupported_fact", "message": f"tier {record['tier']} is not projected"})
            self.stopped_at = 0
            return
        setup = {"tier": record["tier"], "pins": record["pins"], "game": record["game"], "cards": record["cards"], "start": record["start"]}
        if "match" in record:
            setup["match"] = record["match"]
        reply = adapter.request({"setup": setup})
        if "error" in reply:
            self._refusal(0, reply["error"])
            self.stopped_at = 0
            return
        if not self._setup_causal(reply):
            return
        problems: list = []
        expected = State.from_record(record, problems)
        actual = reply["ok"]["state"]
        self._compare_at(0, expected, actual, reply["ok"].get("unsupported", []))
        self._boundary(0, actual, reply["ok"].get("unsupported", []))
        transitions = record["transitions"]
        for index, transition in enumerate(transitions):
            n = index + 1
            action = transition.get("do", transition.get("attempt"))
            self._load_random(transition.get("random", []))
            step = {"n": n, "seat": transition["seat"], "do": action}
            if "attempt" in transition:
                reply = self._call(adapter, {"attempt": step})
                if "refused" in reply:
                    failure = reply["refused"]
                    if failure["class"] != "illegal":
                        self._refusal(n, failure)
                        self.stopped_at = n
                        return
                    self.replayed += 1
                else:
                    self.replayed += 1
                    self._divergence(n, {"level": "L1", "path": "attempt", "expected": "refused", "actual": "accepted"})
                    if not reply["ok"].get("restored"):
                        self.stopped_at = n
                        self.notes.append(f"transition {n}: the attempt was accepted and the adapter could not restore the boundary")
                        return
                if "delta" not in transition:
                    self._check_random_consumed(n)
                    continue
                actual = reply["ok"]["state"] if "ok" in reply else actual
                unsupported = reply["ok"].get("unsupported", []) if "ok" in reply else []
            else:
                reply = self._call(adapter, {"act": step})
                if "refused" in reply:
                    self._refusal(n, reply["refused"])
                    self.stopped_at = n
                    return
                self.replayed += 1
                actual = reply["ok"]["state"]
                unsupported = reply["ok"].get("unsupported", [])
            expected.apply(transition["delta"], transition.get("random", []), f"/transitions/{index}/delta", problems)
            if any(p.severity == "error" for p in problems):
                raise AdapterError("the record's deltas do not apply (L0 should have failed): " + "; ".join(str(p) for p in problems[:3]))
            found, actual, unsupported = self._reach(adapter, n, expected, actual, unsupported)
            self._check_random_consumed(n)
            if not found:
                return
            self._boundary(n, actual, unsupported)
            checkpoint = record.get("checkpoints", {}).get(str(n))
            if checkpoint is not None:
                digest = sha256(normalize_state_lots(actual))
                if digest != checkpoint:
                    self._divergence(n, {"level": "L5", "path": "checkpoint", "expected": checkpoint, "actual": digest})
        self.stopped_at = None

    def _boundary(self, after: int, actual: dict, unsupported: list[str]):
        """Called with the engine's explicit projection at every boundary the run reached (after
        the setup for 0): a hook for a producer that records what the engine did (record.py)."""

    def _compare_at(self, after: int, expected: State, actual: dict, unsupported: list[str]) -> tuple[bool, bool]:
        """Records every difference at `after`; returns (tier-1 agreement, L2 agreement)."""
        want = expected.explicit(self.oracle)
        diffs, inconclusive = compare(want, actual, list(self.unsupported_global) + list(unsupported), self.record["tier"], oracle=self.oracle, cards=self.record["cards"])
        for diff in diffs:
            self._divergence(after, diff)
        for item in inconclusive:
            self._inconclusive(after, item["level"], item["path"], "the adapter does not project this fact")
        tier1 = not any(d["level"] in ("L1", "L2", "L3") for d in diffs)
        l2 = not any(d["level"] == "L2" for d in diffs)
        return tier1, l2

    def _reach(self, adapter: Adapter, n: int, expected: State, actual: dict, unsupported: list[str]):
        """Moves through the stretch after transition `n` to the expected boundary (runner step 3):
        the first state agreeing on every tier-1 fact. Differences are recorded at the boundary, or
        at the first state agreeing on `turn` and `pending`, or at the state where the search
        stopped."""
        want = expected.explicit(self.oracle)
        until = {"turn": want["turn"]["number"], "step": want["turn"]["step"], "active": want["turn"]["active"],
                 "seat": want["pending"]["seat"] if want["pending"] else want["turn"]["active"]}
        if want["turn"].get("occurrence", 1) > 1:
            until["occurrence"] = want["turn"]["occurrence"]
        candidate = None  # the state agreeing at L2 with the fewest tier-1 differences, the first on a tie
        last = (actual, unsupported)
        asked = None
        seen: list[tuple] = []  # the steps of the current turn in the order the runner saw them begin
        while True:
            pair = (actual["turn"]["number"], actual["turn"]["step"], actual["turn"].get("occurrence", 1))
            if seen and seen[-1][0] != pair[0]:
                seen = []
            if pair not in seen:
                seen.append(pair)
            diffs, inconclusive = compare(want, actual, list(self.unsupported_global) + list(unsupported), self.record["tier"], oracle=self.oracle, cards=self.record["cards"])
            tier1 = [d for d in diffs if d["level"] in ("L1", "L2", "L3")]
            if not tier1:
                self._record_diffs(n, diffs, inconclusive)
                return True, actual, unsupported
            l2_ok = not any(d["level"] == "L2" for d in diffs)
            if l2_ok and (candidate is None or len(tier1) < candidate[4]):
                candidate = (actual, unsupported, diffs, inconclusive, len(tier1))
            pending = actual["pending"]
            if l2_ok and pending is not None and ((pending["kind"] == "priority" and not actual["zones"].get("stack"))
                                                  or pending["kind"] in ("declare_attackers", "declare_blockers")):
                # null answers alone cannot bring that seat this question again in this step: with
                # an empty stack a pass ends the window and nothing resolves (117.4, 500.2), and a
                # combat declaration is made once per step (508.1a, 509.1a); so this is the
                # boundary, with its tier-1 differences, and the game goes on from here rather than
                # past the boundary. A question asked while an object resolves may recur (two
                # "may" clauses of one ability, spec 5.5), so the search goes on there.
                break
            if asked is not None:
                break  # a question without a null answer that the record does not answer here
            if pending is None:
                break  # the game ended before the expected boundary
            if past_boundary([(s, o) for _, s, o in seen], actual, want):
                break  # past the boundary's turn and step: it was never reached
            reply = self._call(adapter, {"default": {"until": until}})
            if "refused" in reply:
                self._refusal(n, reply["refused"])
                self.stopped_at = n
                return False, actual, unsupported
            actual, unsupported = reply["ok"]["state"], reply["ok"].get("unsupported", [])
            asked = reply["ok"].get("asked")
            last = (actual, unsupported)
        # the boundary was not reached: report at the best state seen
        if candidate is not None:
            state, unsupported, diffs, inconclusive, _ = candidate
            self._record_diffs(n, diffs, inconclusive)
            if asked is not None:
                self.notes.append(f"transition {n}: the engine asks {asked} where the record has no answer")
            return True, state, unsupported
        actual, unsupported = last
        diffs, inconclusive = compare(want, actual, list(self.unsupported_global) + list(unsupported), self.record["tier"], oracle=self.oracle, cards=self.record["cards"])
        self._record_diffs(n, diffs, inconclusive)
        if asked is not None:
            self._divergence(n, {"level": "L1", "path": "pending", "expected": want["pending"], "actual": actual["pending"]})
            self.notes.append(f"transition {n}: the engine asks {asked}, a question without a null answer the record does not answer")
        self.stopped_at = n
        return False, actual, unsupported

    def _record_diffs(self, n: int, diffs: list[dict], inconclusive: list[dict]):
        for diff in diffs:
            self._divergence(n, diff)
        for item in inconclusive:
            self._inconclusive(n, item["level"], item["path"], "the adapter does not project this fact")


# ----- batch -----

def run_record(path: Path, oracle: Oracle, adapter_command: list[str], timeout: float) -> dict:
    record = loads(Path(path).read_text(encoding="utf-8"))
    return Run(record, oracle, adapter_command, timeout).run()


def summary_line(name: str, report: dict) -> str:
    levels = " ".join(f"{level}:{report['levels'][level]}" for level in LEVELS)
    stopped = "" if report["stopped_at"] is None else f" stopped_at={report['stopped_at']}"
    return f"{name}: {levels} replayed={report['replayed']}/{report['transitions']} divergences={len(report['divergences'])} inconclusive={len(report['inconclusive'])} refusals={len(report['refusals'])}{stopped} {report.get('usage', {}).get('seconds', 0)}s"


def inventory_records(inventory_path=None, *, active=False):
    """Select ordinary active fixtures or the explicit complete historical corpus.

    Archival status records provenance; it does not promise current replay success.
    Selection never infers semantic review from a successful recording or replay.
    """
    path = Path(inventory_path) if inventory_path else ROOT / 'tests/fixtures/mgn/inventory.json'
    inventory = json.loads(path.read_text(encoding='utf-8'))
    return [(item['record'].removesuffix('/record.json'), path.parent / item['record'])
            for item in inventory['records'] if 'record' in item
            and (not active or item.get('disposition') == 'active')]


def main(argv=None) -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("records", nargs="*", type=Path, help="record.json files")
    selection = parser.add_mutually_exclusive_group()
    selection.add_argument("--active", action="store_true", help="ordinary active fixtures from tests/fixtures/mgn/inventory.json")
    selection.add_argument("--all", action="store_true", help="complete fixture inventory, including archived diagnostic records")
    parser.add_argument("--inventory", type=Path, help="another inventory file for --active or --all")
    parser.add_argument("--engine", type=Path, default=DEFAULT_ENGINE, help="the game-serve binary for the Karn adapter")
    parser.add_argument("--adapter", help="an adapter command line to use instead of the Karn adapter")
    parser.add_argument("--oracle", type=Path, default=None)
    parser.add_argument("--out", type=Path, help="directory for one report per record")
    parser.add_argument("--timeout", type=float, default=120.0)
    parser.add_argument("--jobs", type=int, default=max(1, min(4, (os.cpu_count() or 2) // 2)))
    args = parser.parse_args(argv)
    if args.inventory and not (args.active or args.all):
        parser.error("--inventory needs --active or --all")
    oracle = Oracle(args.oracle) if args.oracle else Oracle()
    adapter_command = args.adapter.split() if args.adapter else karn_adapter_command(args.engine)
    records: list[tuple[str, Path]] = [(str(p), p) for p in args.records]
    if args.all or args.active:
        records.extend(inventory_records(args.inventory, active=args.active))
    if not records:
        parser.error("no records given")
    from concurrent.futures import ThreadPoolExecutor
    results = {}
    with ThreadPoolExecutor(max_workers=args.jobs) as pool:
        futures = {name: pool.submit(run_record, path, oracle, adapter_command, args.timeout) for name, path in records}
        for name, future in futures.items():
            results[name] = future.result()
            print(summary_line(name, results[name]), flush=True)
    if args.out:
        args.out.mkdir(parents=True, exist_ok=True)
        index = []
        for name, report in results.items():
            if args.all or args.active or Path(name).name == "record.json":
                target = args.out / (name if args.all or args.active else Path(name).parent.name) / "report.json"
            else:
                target = args.out / (Path(name).stem + "-report.json")
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_text(json.dumps(report, indent=1, ensure_ascii=False) + "\n", encoding="utf-8")
            index.append({"record": name, "report": str(target.relative_to(args.out)), "levels": report["levels"],
                          "replayed": report["replayed"], "transitions": report["transitions"], "stopped_at": report["stopped_at"],
                          "divergences": len(report["divergences"]), "inconclusive": len(report["inconclusive"]), "refusals": len(report["refusals"])})
        (args.out / "index.json").write_text(json.dumps({"mgn_reports": "1", "reports": index}, indent=1) + "\n", encoding="utf-8")
    failed = any(report["levels"][level] == "fail" for report in results.values() for level in LEVELS)
    if args.active:
        failed |= any(value not in ('pass', 'not_applicable')
                      for report in results.values() for value in report['levels'].values())
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
