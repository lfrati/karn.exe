"""Portable copiable spell characteristics (707.2), separate from layer results.

An Oracle face is a compact intrinsic definition. Other definitions state every
intrinsic characteristic, including absent mana cost and name, explicitly.
"""
from copy import deepcopy

from .keywords import printed_keywords
from .oracle import paragraphs


def basis(definition):
    if "oracle" in definition:
        return definition["oracle"], "fused" if definition.get("fused") else definition["face"]
    return deepcopy(definition)


def characteristics(value, oracle):
    if isinstance(value, dict):
        return deepcopy(value["faces"][value["face"]] if "faces" in value else value)
    if not isinstance(value, tuple) or oracle is None or value[0] not in oracle:
        return None
    oracle_id, face = value
    if face == "fused":
        if not oracle.has_fuse(oracle_id):
            return None
        parts = [characteristics((oracle_id, i), oracle) for i in range(2)]
        result = {key: sorted(set(parts[0][key]) | set(parts[1][key]))
                  for key in ("colors", "color_indicator", "types", "supertypes", "subtypes")}
        result.update(name=" // ".join(p["name"] for p in parts),
                      mana_cost="".join(p["mana_cost"] or "" for p in parts) or None,
                      text="\n".join(p["text"] for p in parts if p["text"]))
        return result
    if not 0 <= face < oracle.face_count(oracle_id):
        return None
    raw = oracle.faces[oracle_id][face].raw
    printed = oracle.printed(oracle_id, face)
    # A zero cost is "{0}"; no mana cost is null (202.1b).
    result = {"name": oracle.faces[oracle_id][face].name,
              "mana_cost": raw.get("mana_cost") or None,
              "text": raw.get("oracle_text", ""),
              "color_indicator": sorted(raw.get("color_indicator", []))}
    from .vocabulary import COLOR_LETTERS
    result["color_indicator"] = sorted(COLOR_LETTERS[c] for c in result["color_indicator"])
    result.update({key: printed[key] for key in ("colors", "types", "supertypes", "subtypes")})
    for key in ("power", "toughness", "loyalty", "defense"):
        if key in raw:
            result[key] = str(raw[key])
    return result


def whole(value, oracle):
    """Both faces are copiable on a double-faced permanent spell (707.10g)."""
    if isinstance(value, dict):
        return deepcopy(value)
    selected = characteristics(value, oracle)
    if selected is None or not isinstance(value, tuple):
        return selected
    oid, face = value
    if (isinstance(face, int) and oracle.faces[oid][0].raw.get("layout") in ("transform", "modal_dfc")
            and any(t in selected["types"] for t in ("artifact", "battle", "creature", "enchantment", "land", "planeswalker"))):
        return {"faces": [characteristics((oid, i), oracle) for i in range(2)], "face": face}
    return selected


def _comparable(full):
    if "faces" in full:
        return {"faces": [_comparable(face) for face in full["faces"]], "face": full["face"]}
    result = deepcopy(full)
    result["text"] = paragraphs(result["text"])
    for key in ("colors", "color_indicator", "types", "supertypes", "subtypes"):
        result[key] = sorted(result[key])
    return result


def equivalent(left, right, oracle):
    a, b = whole(left, oracle), whole(right, oracle)
    if a is None or b is None:
        return None
    # Reminder text is not an ability; the compact and explicit forms may omit it.
    return _comparable(a) == _comparable(b)


def printed(value, oracle):
    full = characteristics(value, oracle)
    if full is None:
        return None
    result = {key: full[key] for key in ("colors", "types", "supertypes", "subtypes")}
    result["keywords"] = printed_keywords(full["text"])
    for key in ("power", "toughness"):
        if key in full:
            text = str(full[key])
            result[key] = int(text) if text.lstrip("+-").isdigit() else text
    return result


def comparison_projection(projection, cards, oracle):
    """Compare equivalent definition spellings without changing records or hashes.

    An omitted definition can only be supplied by a currently visible spell
    donor. A moved, face-down or token-defined donor needs the explicit fact.
    """
    copies = [object_id for object_id, facts in projection.get("objects", {}).items()
              if facts.get("kind") == "copy"]
    if not copies:
        return projection
    result = deepcopy(projection)
    objects = result.get("objects", {})
    stack = result.get("zones", {}).get("stack", [])

    def intrinsic(object_id, seen):
        if object_id in seen:
            return None
        facts = objects.get(object_id, {})
        if "copy_definition" in facts:
            return whole(basis(facts["copy_definition"]), oracle)
        if object_id not in stack or facts.get("face_down") or "token" in facts:
            return None
        if facts.get("kind") == "copy":
            return intrinsic(facts.get("copy_of"), seen | {object_id})
        if object_id in cards:
            return whole((cards[object_id]["oracle"],
                                    "fused" if facts.get("fused") else facts.get("face", 0)), oracle)
        return None

    definitions = {object_id: intrinsic(object_id, set()) for object_id in copies}
    for object_id, full in definitions.items():
        facts = objects[object_id]
        if full is not None:
            facts["copy_definition"] = _comparable(full)
    return result
