"""Keyword characteristic spellings from actual rules text (MGN 8.5, CR 702).

The Oracle `keywords` index loses costs and repeated instances and includes ability words
and actions. It is never a source of characteristic values. Only top-level keyword rows
are read here; occurrences inside an effect or reminder text do not give its source that
ability (CR 113.1a, 207.2c).
"""
from __future__ import annotations

import re
from collections import Counter

from .vocabulary import spelled_keyword

# These keywords introduce an ability body, rather than taking that body as an
# 8.5 parameter: CR 702.142a, 702.169a, 702.177a, 702.178a, 702.193a.
TEXT_KEYWORDS = frozenset({"boast", "solved", "exhaust", "max speed", "power-up"})

# Explicitly recognized required parameters, not a cost/selector grammar. The
# pinned rules define these forms with a cost, number, or quality: e.g. enchant
# (702.5a), equip (702.6a), protection (702.16a), ward (702.21a), affinity
# (702.41a), devour (702.82a/c), toxic (702.164a), craft (702.167a), plot
# (702.170a), gift (702.174a) and impending (702.176a). Optional-parameter
# families such as hexproof, trample and partner deliberately are absent.
REQUIRED_PARAMETERS = frozenset({
    "ward", "equip", "enchant", "protection", "landwalk", "offering", "splice",
    "kicker", "multikicker", "crew", "saddle", "cycling", "flashback", "escape",
    "offspring", "disguise", "morph", "plot", "gift", "affinity", "craft", "devour",
    "toxic", "impending", "freerunning", "backup", "firebending", "mobilize", "harmonize", "warp",
})


def normalize_keyword(text: str) -> str:
    """Lowercase the keyword name, retaining its printed parameter (MGN 8.5).

    Mana symbols keep canonical capitals; dash costs keep sentence punctuation and
    parameter case ("Ward—Pay 3 life." becomes "ward—Pay 3 life.").
    """
    value = text.strip()
    name = keyword_name(value)
    if name is not None:
        if name in {"landwalk", "cycling", "offering"} and not value.lower().startswith(name):
            match = re.match(r".*?(?:walk|cycling|offering)", value, re.I)
            prefix = match.end() if match else len(name)
        else:
            prefix = len(name)
        value = value[:prefix].lower() + value[prefix:]
    return re.sub(r"\{[^}]+\}", lambda m: m[0].upper(), value)


def without_reminder(text: str) -> str:
    out, depth = [], 0
    for char in text:
        if char == "(":
            depth += 1
        elif char == ")" and depth:
            depth -= 1
        elif not depth:
            out.append(char)
    return "".join(out).strip()


def keyword_name(text: str) -> str | None:
    """Recognize the name also when a printed cost immediately follows an em dash."""
    head = re.sub(r"[—–]", " ", text).split(",", 1)[0].strip()
    name = spelled_keyword(head)
    if name is None and re.match(r"(?:[A-Za-z]+ )*[A-Za-z]+cycling(?=\s*\{)", head, re.I):
        return "cycling"
    if name is None and re.fullmatch(r"[A-Za-z]+ offering", head):
        return "offering"
    return name


def keyword_row(text: str) -> list[str]:
    """Read one top-level keyword row, keeping parameters and repeated instances.

    Commas inside a cost or an X definition stay with that ability; only a comma
    followed by another keyword starts the next instance (CR 702.1).
    A text-introducing keyword's following ability body is not its parameter.
    """
    text = without_reminder(text)
    if not text:
        return []
    # Recognize rule-defined wrappers before flavor labels: "Max speed — Flying"
    # gives a max speed ability, not unconditional flying (CR 702.178a).
    wrapper = re.match(r"^(.+?)\s*[—–]", text)
    if wrapper and wrapper[1].strip().lower() in TEXT_KEYWORDS:
        return [wrapper[1].strip().lower()]
    # Ability-word labels do not change the following keyword ability (207.2c).
    head, dash, body = text.partition(" — ")
    if dash and keyword_name(body) is not None:
        text = body
    if keyword_name(text) is None:
        return []
    pieces = re.split(r",\s*", text)
    instances = [pieces[0]]
    for piece in pieces[1:]:
        if keyword_name(piece) is not None:
            instances.append(piece)
        else:
            instances[-1] += ", " + piece
    out = []
    for piece in instances:
        # A following sentence such as "X can't be 0" is separate rules text.
        sentence, dot, _ = piece.partition(". ")
        piece = sentence + ("." if dot else "")
        if "—" not in piece and "–" not in piece:
            piece = piece.rstrip(".")
        value = normalize_keyword(piece)
        name = keyword_name(value)
        # Prefix recognition alone would mistake "Equip abilities you activate ..."
        # or a card named "Storm Seeker" for a keyword row. Numeric/mana/cost
        # parameters have a syntactic boundary; selector parameters have a small
        # set of rule-defined introductions.
        tail = value[len(name):] if value.startswith(name) else ""
        textual = {"enchant", "protection", "affinity", "splice", "partner", "friends forever",
                   "choose a background", "hexproof", "trample", "gift", "landwalk", "craft", "offering"}
        if name == "champion" and re.match(r"champion (?:a|an) ", value):
            textual.add("champion")
        # Devour [quality] N restricts the permanents that may be sacrificed
        # (CR 702.82c); the quality and multiplier are both printed parameters.
        quality_devour = name == "devour" and bool(re.fullmatch(r"\s+.+\s+(?:[0-9]+|X)", tail))
        restricted_equip = name == "equip" and bool(re.search(r"\{[^}]+\}$", tail))
        if (value != name and name not in textual and not restricted_equip and not quality_devour
                and not re.match(r"(?:\s+(?:\{|[0-9X])|[—–])", tail)
                and not (name == "cycling" and not value.startswith("cycling"))):
            return []
        out.append(value)
    return out


def printed_keywords(text: str) -> list[str]:
    """Printed keyword instances in Oracle text, sorted but never deduplicated."""
    return sorted(k for row in (text or "").splitlines() for k in keyword_row(row))


def is_keyword_characteristic(text: str) -> bool:
    """Whether one value has a complete canonical characteristic spelling (MGN 8.5).

    A row may introduce an ability body or contain several keyword instances; a
    characteristic value must instead be exactly one already-normalized result.
    Reject missing parameters for the explicit families above. This checks the
    representation, not whether an arbitrary cost or quality is legal under 702;
    the permissive row reader is not a replacement for the engine's grammar.
    """
    if not isinstance(text, str) or "\n" in text or text != normalize_keyword(text):
        return False
    name = keyword_name(text)
    if name is None or (name in REQUIRED_PARAMETERS and text == name):
        return False
    return keyword_row(text) == [text]


def merge_observed_keywords(printed: list[str], observed: list[str]) -> list[str]:
    """Recover the old partial observation using printed parameters, without guessing.

    A bare observed name is explained by printed instances of that name when present.
    Without that evidence a parametric bare name is refused instead of inventing a cost.
    Exact observations are merged by maximum multiplicity: the same printed instance
    in both sources is not an additional ability (CR 113.2c).
    """
    base = Counter(normalize_keyword(k) for k in printed)
    seen = Counter()
    for word in observed:
        value = normalize_keyword(word)
        name = keyword_name(value)
        matches = [k for k in base if keyword_name(k) == name]
        if name == value and any(k != value for k in matches):
            continue
        if name == value and name in REQUIRED_PARAMETERS:
            raise ValueError(f"keyword {value!r} has no observed or printed parameter")
        seen[value] += 1
    return sorted((base | seen).elements())
