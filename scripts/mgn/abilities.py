"""Portable ability provenance shared by the state model and live adapters (spec 3.3)."""
from __future__ import annotations

from copy import deepcopy
from collections.abc import Callable


def inherited_ability_ref(family: dict, index: int, *, object_ref: Callable,
                          ability_ref: Callable) -> dict:
    """Translate an inherited native family without consulting mutable game state.

    ``object_ref(native_id)`` returns the acquisition's record identity.
    ``ability_ref(source, index, origin, frozen_definition)`` authenticates and maps a
    terminal donor/grant reference. It must refuse if that identity cannot faithfully
    name the supplied frozen definition; a current row is insufficient evidence.
    Native indices, stamps and definitions are callback inputs only, never output.
    Nested inherited origins are recursively translated here, not by the callback.
    """
    active = set()

    def member(source, row, origin, definition):
        if isinstance(origin, dict) and "donor" in origin and "grant" in origin:
            return inherited(origin, row)
        return deepcopy(ability_ref(source, row, deepcopy(origin), deepcopy(definition)))

    def inherited(node, row):
        if not isinstance(node, dict) or not isinstance(row, int) or isinstance(row, bool) or row < 0:
            raise ValueError("inherited ability needs a family and a nonnegative donor index")
        if id(node) in active:
            raise ValueError("cyclic inherited ability provenance")
        active.add(id(node))
        try:
            grant = node["grant"]
            donor = node["donor"]
            by = grant["by"]
            if donor is None or by is None:
                raise ValueError("inherited donor and granting source must have record identities")
            return {
                "inherited_from": {"source": object_ref(donor),
                    "ability": member(donor, row, node["donor_origin"], node["definition"])},
                "granted_by": {"source": object_ref(by),
                    "ability": member(by, grant["ability"], grant["origin"], grant["definition"])},
            }
        except (KeyError, TypeError) as exc:
            raise ValueError("incomplete inherited ability family") from exc
        finally:
            active.remove(id(node))

    return inherited(family, index)


def ability_object_refs(ability: dict) -> list[str]:
    """Every declaring/granting object named by an ability, including nested grants."""
    if "inherited_from" in ability:
        return [ref for key in ("inherited_from", "granted_by")
                for ref in effect_object_refs(ability[key])]
    by = ability.get("granted_by")
    return [by] if isinstance(by, str) else []


def effect_object_refs(effect: dict) -> list[str]:
    if "source" not in effect:
        return []
    return [effect["source"], *ability_object_refs(effect["ability"])]


def source_free_rule(ability: dict) -> bool:
    """Only intrinsically sourceless subscriptions in the pinned CR."""
    rule = ability.get("rule")
    if rule in ("725.2", "726.2"):
        return type(ability.get("sub")) is int and 1 <= ability["sub"] <= (2 if rule == "725.2" else 3)
    return ability == {"rule": "702.179d"}
