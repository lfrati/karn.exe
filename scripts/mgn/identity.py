"""Record identity and game identity (spec section 7.2)."""
from __future__ import annotations

from copy import deepcopy

from .canonical import sha256
from .lots import normalize_lots
from .state import TIER2_OBJECT, TIER2_PLAYER
from .vocabulary import CARD_ID, STACK_ID, TOKEN_ID


def record_identity(record: dict) -> str:
    return "sha256:" + sha256(record)


# Only these fields contain physical references. Member names, free text and
# structural string values are never ids merely because an author used that word.
_REFERENCE_VALUES = {"card", "source", "object", "current", "copy_of", "attached_to",
                     "attacking", "granted_by", "creature", "keep", "defender", "to", "on", "tapped"}
_REFERENCE_LISTS = {"cards", "objects", "blocking", "targets", "attacks", "blocks",
                    "assignments", "amounts", "pile", "piles", "order", "from", "chosen",
                    "excluding", "delve", "summoning_sick", "top", "bottom", "away",
                    "found", "holders", "for", "in", "out"}
_REFERENCE_MAPS = {"cards", "objects", "base_controllers"}


def _map_references(value, reference, field="", *, canonical=False):
    """Visit reference occurrences, keeping the surrounding MGN syntax intact."""
    if isinstance(value, str):
        return reference(value) if field in _REFERENCE_VALUES or field == "_ref" else value
    if isinstance(value, list):
        if field == "fixed":
            # Random shuffle constraints pair a card with a library position;
            # the latter's literal "top"/"bottom" is not a second card.
            return [[reference(card), deepcopy(position)] for card, position in value]
        child = "_ref" if field in _REFERENCE_LISTS or field == "_ref" else ""
        return [_map_references(item, reference, child, canonical=canonical) for item in value]
    if isinstance(value, dict):
        items = sorted(value.items(), key=lambda item: item[0].encode("utf-16-be")) if canonical else value.items()
        if field in _REFERENCE_MAPS:
            return {reference(key): _map_references(item, reference, canonical=canonical) for key, item in items}
        if field == "zones":
            return {key: _map_references(item, reference, "_ref" if isinstance(item, list) else "", canonical=canonical)
                    for key, item in items}
        return {key: deepcopy(item) if key in {"pins", "author", "note", "lineage", "integrity"}
                else _map_references(item, reference, key, canonical=canonical) for key, item in items}
    return value


def _scan(value, cards, order: list[str]):
    """Append referenced physical ids in canonical member/list order."""
    def reference(value):
        if value in cards and value not in order:
            order.append(value)
        return value
    _map_references(value, reference, canonical=True)


def relabeling(record: dict) -> dict[str, str]:
    """Physical id -> c<n>, in the order spec 7.2 defines."""
    cards = record["cards"]
    order: list[str] = []
    start = record["start"]
    _scan(start.get("random", []), cards, order)
    for transition in record["transitions"]:
        _scan(transition.get("do", transition.get("attempt")), cards, order)
        _scan(transition.get("random", []), cards, order)
    if start["kind"] == "position":
        zones = start["state"]["zones"]
        seats = record["game"]["seats"]
        names = [f"{seat}.{zone}" for seat in seats for zone in ("library", "hand", "graveyard")] + ["battlefield", "stack", "exile", "command"]
        for name in names:
            members = list(zones.get(name, []))
            if name.endswith(".hand") or name in ("battlefield", "exile", "command"):
                members.sort(key=lambda i: (cards[i]["owner"], cards[i]["oracle"], i) if i in cards else ("~", i, i))
            for member in members:
                if member in cards and member not in order:
                    order.append(member)
    for card_id in sorted(cards, key=lambda i: (cards[i]["owner"], cards[i]["oracle"], i)):
        if card_id not in order:
            order.append(card_id)
    return {card_id: f"c{n}" for n, card_id in enumerate(order, start=1)}


def _relabel(value, mapping):
    return _map_references(value, lambda reference: mapping.get(reference, reference))


def _normalize_payment_lots(value):
    """Only payment lot arrays are multisets; activation order stays significant."""
    if isinstance(value, list):
        return [_normalize_payment_lots(item) for item in value]
    if isinstance(value, dict):
        return {key: normalize_lots(item) if key == "pool_restricted" else _normalize_payment_lots(item)
                for key, item in value.items()}
    return value


def game_inputs(record: dict) -> dict:
    """The relabeled inputs the game identity hashes."""
    mapping = relabeling(record)
    cards = {mapping[i]: {"owner": c["owner"], "oracle": c["oracle"]} for i, c in record["cards"].items()}
    start = deepcopy(record["start"])
    if start["kind"] == "position":
        state = start["state"]
        for seat_facts in state.get("players", {}).values():
            for fact in list(seat_facts):
                if fact in TIER2_PLAYER:
                    del seat_facts[fact]
        state.pop("game", None)
        for facts in state.get("objects", {}).values():
            for fact in list(facts):
                if fact in TIER2_OBJECT:
                    del facts[fact]
        state["objects"] = {k: v for k, v in state.get("objects", {}).items() if v}
    transitions = []
    for transition in record["transitions"]:
        entry = {"seat": transition["seat"]}
        if "do" in transition:
            entry["do"] = transition["do"]
        else:
            entry["attempt"] = transition["attempt"]
            entry["illegal"] = True
        if transition.get("random"):
            entry["random"] = transition["random"]
        transitions.append(entry)
    # The card map is already canonical: an authored id such as c1 must not
    # relabel a newly generated canonical key a second time.
    inputs = _relabel({"start": start, "transitions": transitions}, mapping)
    return _normalize_payment_lots({"cards": cards, **inputs})


def game_identity(record: dict) -> str:
    return "sha256:" + sha256(game_inputs(record))
