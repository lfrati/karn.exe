"""Enumerated vocabularies of MGN, derived from the pinned Comprehensive Rules and Oracle data.

Everything an MGN record spells out of a closed set comes from here: steps, zones, colors,
mana symbols, card types, supertypes, subtypes, keyword abilities and counter kinds. The long
lists are parsed from the pinned rules text at import time, so that a rules refresh changes
the vocabulary with it; the short lists are written out and checked against the rules text by
`verify_against_rules()`, which the tests run.
"""
from __future__ import annotations

from functools import lru_cache
import json
from pathlib import Path
import re

import pinned

ROOT = Path(__file__).resolve().parents[2]
RULES = pinned.RULES
CARDS = ROOT / "resources/oracle/cards.jsonl"
LEGAL = ROOT / "resources/oracle/legal-standard.jsonl"

# 500.1, 501.1, 506.1, 510.4 (the additional combat damage step for first and double strike),
# 512.1: the steps of a turn as MGN spells them.
STEPS = ("untap", "upkeep", "draw", "precombat_main", "begin_combat", "declare_attackers",
         "declare_blockers", "first_strike_damage", "combat_damage", "end_combat",
         "postcombat_main", "end", "cleanup")
# 400.1: the seven zones. Libraries, hands and graveyards are one per seat and spelled
# "<seat>.library", "<seat>.hand", "<seat>.graveyard"; the others are shared.
SEAT_ZONES = ("library", "hand", "graveyard")
SHARED_ZONES = ("battlefield", "stack", "exile", "command")
ORDERED_ZONES = ("library", "graveyard", "stack")  # 401.2, 404.2, 405.2
# 105.1
COLORS = ("white", "blue", "black", "red", "green")
COLOR_LETTERS = {"W": "white", "U": "blue", "B": "black", "R": "red", "G": "green"}
# 106.1a plus colorless (107.4c): the mana a pool can hold, by letter.
MANA_TYPES = ("W", "U", "B", "R", "G", "C")
# 107.4: a mana symbol as printed, braces included: {W} {2} {X} {C} {S} {W/U} {2/W} {C/W} {W/P} {W/U/P}
MANA_SYMBOL = re.compile(r"^\{(?:[WUBRG]|C|S|X|Y|Z|\d{1,3}|[WUBRG]/[WUBRG]|2/[WUBRG]|C/[WUBRG]|[WUBRG]/P|[WUBRG]/[WUBRG]/P)\}$")
# 205.2a, 205.4a
CARD_TYPES = ("artifact", "battle", "conspiracy", "creature", "dungeon", "enchantment", "instant",
              "kindred", "land", "phenomenon", "plane", "planeswalker", "scheme", "sorcery", "vanguard")
SUPERTYPES = ("basic", "legendary", "ongoing", "snow", "world")
# A seat is one capital letter, in turn order: A, B, C ...
SEAT = re.compile(r"^[A-Z]$")
# A physical card id chosen by the author; a token id T<n>; a stack object id S<n>; an emblem E<n>.
CARD_ID = re.compile(r"^[A-Za-z][A-Za-z0-9_-]{0,31}$")
TOKEN_ID = re.compile(r"^T[1-9][0-9]*$")
STACK_ID = re.compile(r"^S[1-9][0-9]*$")
EMBLEM_ID = re.compile(r"^E[1-9][0-9]*$")
# Rules-defined designations of objects and of players (spec 3.2), lowercase as the rules name them.
DESIGNATIONS = ("solved", "saddled", "suspected", "harnessed", "prepared", "plotted", "foretold",
                "left half unlocked", "right half unlocked")  # 719.3b, 702.171b, 701.60b, 701.64b, 722.3a, 702.170a, 702.143a, 709.5c
PLAYER_DESIGNATIONS = ("enduring story",)  # 702.195b
# 122.1a: +X/+Y counters; otherwise a named kind in lowercase words, as the Oracle text names it.
COUNTER_KIND = re.compile(r"^(?:[+-]\d+/[+-]\d+|[a-z][a-z0-9'-]*(?: [a-z][a-z0-9'-]*)?)$")
SHA256 = re.compile(r"^[0-9a-f]{64}$")
DATE = re.compile(r"^\d{4}-\d{2}-\d{2}$")
UUID = re.compile(r"^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$")


@lru_cache(maxsize=None)
def rules_text() -> str:
    return RULES.read_text(encoding="utf-8", errors="replace")


def rule(number: str) -> str:
    """The text of one numbered rule, without its number."""
    match = re.search(r"^" + re.escape(number) + r"\.? (.*)$", rules_text(), re.M)
    if not match:
        raise KeyError(number)
    return match.group(1).strip()


def _listed(text: str) -> list[str]:
    """The items of an English list "A, B (see rule 1), C, and D." as written."""
    text = re.sub(r"\s*\([^)]*\)", "", text)
    text = text.rstrip(".").strip()
    parts = text.split(",")
    if len(parts) == 1 and " and " in text:
        parts = text.split(" and ")  # a two-item list: "end and cleanup"
    items = []
    for part in parts:
        part = part.strip()
        if part.startswith("and "):
            part = part[4:]
        if part:
            items.append(part)
    return items


def _after(text: str, marker: str) -> str:
    index = text.index(marker) + len(marker)
    rest = text[index:]
    end = rest.find(". ")
    return rest if end < 0 else rest[:end + 1]


@lru_cache(maxsize=None)
def keyword_abilities() -> tuple[str, ...]:
    """The keyword abilities of section 702, lowercased, in rule order (702.2 onward)."""
    names = re.findall(r"^702\.(\d+)\. ([A-Z][^\n]*)$", rules_text(), re.M)
    out = []
    for number, name in names:
        if int(number) < 2:
            continue
        name = name.strip().lower()
        # one heading defines two keywords ("daybound and nightbound", 702.145)
        out.extend(name.split(" and ") if " and " in name else [name])
    # 702.33c names this variant inside the kicker rule, without a separate
    # heading. It is a printed keyword, including its repeatable cost.
    if re.search(r'^702\.33c .*Multikicker', rules_text(), re.M):
        out.append('multikicker')
    return tuple(out)


@lru_cache(maxsize=None)
def keyword_actions() -> tuple[str, ...]:
    """The keyword actions of section 701, lowercased, in rule order (701.2 onward)."""
    names = re.findall(r"^701\.(\d+)\. ([A-Z][^\n]*)$", rules_text(), re.M)
    return tuple(name.strip().lower() for number, name in names if int(number) >= 2)


@lru_cache(maxsize=None)
def subtypes() -> dict[str, tuple[str, ...]]:
    """Subtypes by card type, as the rules list them (205.3g to 205.3q), spelled as printed."""
    out = {}
    out["artifact"] = tuple(_listed(_after(rule("205.3g"), "The artifact types are ")))
    out["enchantment"] = tuple(_listed(_after(rule("205.3h"), "The enchantment types are ")))
    out["land"] = tuple(_listed(_after(rule("205.3i"), "The land types are ")))
    out["planeswalker"] = tuple(_listed(_after(rule("205.3j"), "The planeswalker types are ")))
    spell = tuple(_listed(_after(rule("205.3k"), "The spell types are ")))
    out["instant"] = spell
    out["sorcery"] = spell
    creature = ["Time Lord"] + _listed(_after(rule("205.3m"), "All other creature types are one word long: "))
    out["creature"] = tuple(creature)
    out["kindred"] = tuple(creature)
    out["plane"] = tuple(_listed(_after(rule("205.3n"), "The planar types are ")))
    out["dungeon"] = ("Undercity",)  # 205.3p
    out["battle"] = ("Siege",)  # 205.3q
    return out


@lru_cache(maxsize=None)
def all_subtypes() -> frozenset[str]:
    return frozenset(name for names in subtypes().values() for name in names)


@lru_cache(maxsize=None)
def _subtype_aliases() -> dict[str, str]:
    # Oracle and the native IR fold CR typographic apostrophes to ASCII. Only
    # aliases of actual pinned CR 205.3 names are recognized; no fuzzy spelling.
    return {name.replace("’", "'"): name for name in all_subtypes() if "’" in name}


def canonical_subtype(name: str, kind: str | None = None) -> str:
    """An Oracle/native subtype as MGN 8.5 spells it; refuse unknown names.

    Optional kind is a card type from subtypes(), or ``basic_land`` (305.6).
    This belongs at a projection boundary, never in raw Oracle data or digests.
    """
    canonical = _subtype_aliases().get(name, name)
    if kind == "basic_land":
        allowed = ("Plains", "Island", "Swamp", "Mountain", "Forest")
    else:
        allowed = all_subtypes() if kind is None else subtypes()[kind]
    if canonical not in allowed:
        raise ValueError(f"unknown {kind + ' ' if kind else ''}subtype {name!r} (CR 205.3)")
    return canonical


def native_subtype(name: str, kind: str | None = None) -> str:
    """A validated subtype for the native IR's ASCII-apostrophe wire parser.

    ir::vocab::list folds exactly U+2019 to ASCII before Subtype::from_name.
    Apply to chosen subtype answers as well as characteristics on native input.
    """
    return canonical_subtype(name, kind).replace("’", "'")


def _standard_identities() -> set[str]:
    return {json.loads(line)["oracle_id"] for line in LEGAL.read_text(encoding="utf-8").splitlines() if line.strip()}


_COUNTER_DETERMINERS = {"a", "an", "another", "additional", "each", "no", "that", "the", "those", "these", "its",
                        "their", "two", "three", "four", "five", "six", "seven", "eight", "nine", "ten", "x",
                        "more", "fewer", "first", "second", "third", "all", "any", "with", "of"}


@lru_cache(maxsize=None)
def counter_kinds() -> tuple[str, ...]:
    """Counter kinds named by the pinned Standard card text, plus the ones the rules define.

    The Oracle text names a counter as "<kind> counter(s)"; the kind is +X/+Y (122.1a) or one
    lowercase word, or two for a keyword ability such as "double strike". The list is open:
    an unknown kind is a validator warning, not an error.
    """
    kinds = {"+1/+1", "-1/-1", "loyalty", "defense", "poison"}  # 122.1a, 306.5b, 310.4, 122.1 / 704.5c
    keywords = set(keyword_abilities())
    legal = _standard_identities()
    pattern = re.compile(r"(?<![\w/+-])([+-]\d+/[+-]\d+|[a-z]+(?: [a-z]+)?) counters?\b")
    for line in CARDS.read_text(encoding="utf-8").splitlines():
        if not line.strip():
            continue
        face = json.loads(line)
        if face["oracle_id"] not in legal:
            continue
        text = face.get("oracle_text", "")
        for match in pattern.finditer(text):
            candidate = match.group(1)
            if "/" in candidate:
                kinds.add(candidate)
                continue
            words = candidate.split()
            if len(words) == 2:
                if candidate in keywords:
                    kinds.add(candidate)
                    continue
                candidate = words[1]
            if candidate in _COUNTER_DETERMINERS or candidate in {"kind", "kinds", "number", "counter"}:
                continue
            before = text[:match.start()].split()
            if before and before[-1].lower().strip(",.;:") in _COUNTER_DETERMINERS | {"put", "remove", "removes", "has", "have", "had", "get", "gets"}:
                kinds.add(candidate)
            elif candidate in keywords:
                kinds.add(candidate)
    return tuple(sorted(kinds))


def verify_against_rules() -> list[str]:
    """Checks the written-out lists against the rules text; returns the discrepancies."""
    problems = []
    if _listed(_after(rule("205.2a"), "The card types are ")) != list(CARD_TYPES):
        problems.append("card types differ from 205.2a")
    if _listed(_after(rule("205.4a"), "The supertypes are ")) != list(SUPERTYPES):
        problems.append("supertypes differ from 205.4a")
    if _listed(_after(rule("105.1"), "There are five colors in the Magic game: ")) != list(COLORS):
        problems.append("colors differ from 105.1")
    if _listed(_after(rule("400.1"), "There are normally seven zones: ")) != ["library", "hand", "battlefield", "graveyard", "stack", "exile", "command"]:
        problems.append("zones differ from 400.1")
    phases = _listed(_after(rule("500.1"), "A turn consists of five phases, in this order: "))
    if phases != ["beginning", "precombat main", "combat", "postcombat main", "ending"]:
        problems.append("phases differ from 500.1")
    if _listed(_after(rule("501.1"), "three steps, in this order: ")) != ["untap", "upkeep", "draw"]:
        problems.append("beginning steps differ from 501.1")
    if _listed(_after(rule("506.1"), "five steps, which proceed in order: ")) != ["beginning of combat", "declare attackers", "declare blockers", "combat damage", "end of combat"]:
        problems.append("combat steps differ from 506.1")
    if _listed(_after(rule("512.1"), "two steps: ")) != ["end", "cleanup"]:
        problems.append("ending steps differ from 512.1")
    return problems


def spelled_keyword(text: str) -> str | None:
    """The keyword ability a characteristic string starts with ("ward {2}" -> "ward"), or None."""
    lowered = text.lower()
    for name in sorted(keyword_abilities(), key=len, reverse=True):
        if lowered == name or lowered.startswith(name + " "):
            return name
    # landwalk is a family of keywords named by the land type: islandwalk, swampwalk (702.14)
    if lowered.endswith("walk") and lowered[:-4].capitalize() in subtypes()["land"]:
        return "landwalk"
    return None


if __name__ == "__main__":
    print(json.dumps({"steps": STEPS, "colors": COLORS, "card_types": CARD_TYPES, "supertypes": SUPERTYPES,
                      "keyword_abilities": keyword_abilities(), "keyword_actions": keyword_actions(),
                      "subtypes": {k: len(v) for k, v in subtypes().items()}, "counter_kinds": counter_kinds(),
                      "rules_check": verify_against_rules()}, indent=1, ensure_ascii=False))
