"""MGN 5.8 restricted mana lots: complete descriptors form a counted multiset."""
from copy import deepcopy

from .canonical import canonical


def normalize_lots(lots):
    """Aggregate identical descriptors and sort canonically; preserve invalid input.

    Validation owns malformed lots. In particular, do not turn invalid zero,
    negative or Boolean counts into a different, apparently valid assertion.
    """
    if not isinstance(lots, list) or any(not isinstance(lot, dict)
            or type(lot.get("count")) is not int or lot["count"] <= 0 for lot in lots):
        return deepcopy(lots)
    grouped = {}
    for lot in lots:
        descriptor = {key: value for key, value in lot.items() if key != "count"}
        key = canonical(descriptor)
        if key not in grouped:
            grouped[key] = {**deepcopy(descriptor), "count": 0}
        grouped[key]["count"] += lot["count"]
    return [grouped[key] for key in sorted(grouped, key=lambda key: key.encode("utf-16-be"))]


def normalize_state_lots(state):
    """Normalize only the lot arrays in a full state, without mutating its input."""
    if "players" not in state:
        return state
    return {**state, "players": {seat: {**facts, **(
        {"mana_restricted": normalize_lots(facts["mana_restricted"])}
        if "mana_restricted" in facts else {})} for seat, facts in state["players"].items()}}
