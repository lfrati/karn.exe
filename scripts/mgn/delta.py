"""Differences between two MGN states, as the delta of spec section 4.1, and the null answers a
record never writes (spec section 5.6).

`diff_states` is the one place that turns two states into a delta: the recorder uses it between
the engine's projections at consecutive recorded decisions. `null_answer` tells the validator
which answers are implied.
"""
from __future__ import annotations

from copy import deepcopy
import re

from .state import GAME_DEFAULTS, GAME_FACTS, OBJECT_DEFAULTS, PLAYER_DEFAULTS, State, TIER2_PLAYER

CHARACTERISTICS = ("power", "toughness", "colors", "types", "supertypes", "subtypes", "keywords")
ORDERED = ("A.library", "B.library", "A.graveyard", "B.graveyard", "stack")


def _position(order: list, object_id: str):
    index = order.index(object_id)
    if index == 0:
        return "top"
    if index == len(order) - 1:
        return "bottom"
    return {"from_top": index + 1}


def _object_delta(prev: State, nxt: State, object_id: str, oracle, moved: bool) -> dict:
    """The facts of `object_id` that changed (or, for a new object, differ from their defaults)."""
    before = prev.objects.get(object_id, {}) if not moved else {}
    after = nxt.objects.get(object_id, {})
    base = nxt._printed(object_id, oracle) or {}
    changes = {}
    missing = object()
    generated = object_id not in nxt.cards
    for fact in set(before) | set(after):
        if fact == "owner" and (object_id in nxt.cards or not moved):
            continue
        if fact in OBJECT_DEFAULTS:
            default = OBJECT_DEFAULTS[fact]
        elif fact == "controller":
            # a new token, ability or emblem states its controller (111.2, 603.3a, 114.2); a card's defaults to its owner
            default = missing if moved and generated else nxt.owner_of(object_id)
        elif fact in CHARACTERISTICS:
            if fact in ('power', 'toughness'):
                types = after.get('types', base.get('types', []))
                default = None if nxt.zone_of(object_id) == 'battlefield' and 'creature' not in types else base.get(fact, None)
            else:
                default = base.get(fact, missing)
        else:
            default = missing
        if before.get(fact, default) != after.get(fact, default):
            value = after.get(fact, default)
            if value is missing:
                raise ValueError(f"{object_id}.{fact} returns to a default that is not known")
            changes[fact] = deepcopy(value)
    return changes


def _reorder_for(current: list, target: list) -> dict:
    """The smallest reorder of spec 4.1 that turns `current` (top first) into `target`."""
    if [c for c in target if c not in current] or [c for c in current if c not in target]:
        return {"order": list(target)}
    for n in range(1, len(target)):
        if target[:n] + [c for c in current if c not in target[:n]] == target:
            return {"top": target[:n]}
        if [c for c in current if c not in target[-n:]] + target[-n:] == target:
            return {"bottom": target[-n:]}
    return {"order": list(target)}


def diff_states(prev: State, nxt: State, oracle, random_outcomes=(), reentered=()) -> dict:
    """The MGN delta that takes `prev` to `nxt`. `random_outcomes` are the outcomes consumed in
    between (a shuffled library's order comes from them); `reentered` names cards that left and
    came back to the same zone in between (new objects, 400.7)."""
    delta: dict = {}
    departed = {key: deepcopy(value) for key, value in nxt.departed.items() if key not in prev.departed}
    if any(nxt.departed.get(key) != value for key, value in prev.departed.items()):
        raise ValueError("departed definitions cannot be removed or changed")
    if departed:
        delta["departed"] = departed
    for key in ("turn", "pending", "result"):
        if getattr(prev, key) != getattr(nxt, key):
            delta[key] = deepcopy(getattr(nxt, key))
    players = {}
    for seat in nxt.seats:
        before, after = prev.players.get(seat, {}), nxt.players.get(seat, {})
        changes = {f: deepcopy(after.get(f, PLAYER_DEFAULTS.get(f))) for f in set(before) | set(after)
                   if before.get(f, PLAYER_DEFAULTS.get(f)) != after.get(f, PLAYER_DEFAULTS.get(f))}
        if changes:
            players[seat] = changes
    if players:
        delta["players"] = players
    game = {f: nxt.game.get(f, GAME_DEFAULTS[f]) for f in GAME_FACTS if prev.game.get(f, GAME_DEFAULTS[f]) != nxt.game.get(f, GAME_DEFAULTS[f])}
    if game:
        delta["game"] = game
    prev_zone = {o: z for z, members in prev.zones.items() for o in members}
    next_zone = {o: z for z, members in nxt.zones.items() for o in members}
    shuffled = {o["seat"]: set(o["order"]) for o in random_outcomes if o["kind"] == "shuffle" and "seat" in o}
    objects = {}
    for object_id in sorted(set(prev_zone) | set(next_zone)):
        if object_id not in next_zone:
            if object_id in nxt.cards:
                raise ValueError(f"card {object_id} is in no zone of the next state")
            objects[object_id] = None
            continue
        zone = next_zone[object_id]
        moved = prev_zone.get(object_id) != zone or object_id in reentered
        changes = _object_delta(prev, nxt, object_id, oracle, moved)
        if moved:
            facts = {"zone": zone}
            if zone.endswith(".library"):
                seat = zone.split(".")[0]
                if not (seat in shuffled and object_id in shuffled[seat]):
                    facts["at"] = _position(nxt.zones[zone], object_id)
            facts.update(changes)
            objects[object_id] = facts
        elif changes:
            objects[object_id] = changes
    if objects:
        delta["objects"] = objects
    # two or more objects entering one ordered zone at once state their order (spec 4.1), except
    # new stack abilities, which follow their numbers
    entered: dict[str, list] = {}
    for object_id, facts in objects.items():
        if facts and "zone" in facts and facts["zone"] in ORDERED:
            entered.setdefault(facts["zone"], []).append(object_id)
    for zone, ids in entered.items():
        if len(ids) < 2 or (zone == "stack" and all(re.match(r"^S\d+$", i) for i in ids)):
            continue
        seat = zone.split(".")[0] if "." in zone else None
        if seat and seat in shuffled and all(i in shuffled[seat] for i in ids):
            continue
        if zone.endswith(".library"):
            delta.setdefault("zones", {})[zone] = {"order": list(nxt.zones[zone])}
        else:
            delta.setdefault("zones", {})[zone] = {"top": [i for i in nxt.zones[zone] if i in ids]}
    # whatever order the rules of 4.1 do not reach is written out
    trial = deepcopy(prev)
    problems: list = []
    trial.apply(delta, list(random_outcomes), "/delta", problems)
    for zone in ORDERED:
        if zone in nxt.zones and trial.zones.get(zone) != nxt.zones[zone]:
            delta.setdefault("zones", {})[zone] = _reorder_for(trial.zones.get(zone, []), nxt.zones[zone])
    return delta


def null_answer(do: dict, state: State | None = None) -> bool:
    """Whether `do` is its question's null answer (spec 5.6), which a record never writes."""
    kind = do.get("kind")
    about = ((state.pending or {}).get("about") or {}) if state is not None else {}
    if kind == "pass":
        return True
    if kind == "declare_attackers":
        return do.get("attacks") == [] and "pay" not in do
    if kind == "declare_blockers":
        return do.get("blocks") == [] and "pay" not in do
    if kind == "mulligan":
        return do.get("keep") is True
    if kind == "optional":
        return do.get("yes") is False
    if kind == "choose_objects":
        return do.get("objects") == []
    if kind == "choose_targets":
        return all(group == [] for group in do.get("targets", [])) and "division" not in do
    if kind == "choose_modes":
        return do.get("modes") == []  # "choose up to one": no mode, and the ability leaves the stack (603.3c)
    if kind == "search":
        return do.get("found") == []
    if kind == "order_cards":
        return "cards" in about and len(about["cards"]) <= 1 and do.get("order") == about["cards"]
    if kind == "arrange":
        return do.get("away") == [] and ("cards" in about and list(about["cards"]) == list(do.get("top", [])))
    if kind == "change_targets":
        if state is None or "object" not in about or not state.exists(about["object"]):
            return False
        return state.fact(about["object"], "targets") == do.get("targets")
    if kind in ("pay", "choose_alternative", "cast_during_resolution"):
        return do.get("decline") is True
    if kind == "holders":
        return do.get("holders") == []
    if kind == "command_zone":
        return do.get("yes") is False
    if kind == "companion":
        return do.get("card") is None
    if kind == "opening_hand_action":
        return do.get("done") is True
    return False
