"""Validate an MGN record without an engine (verification level L0).

    uv run scripts/mgn/validate.py RECORD.json [--oracle resources/oracle/cards.jsonl] [--json] [--identity]

Prints one problem per line as `<severity> <code> <path>: <message>`; exit status 1 when any
error is present. The codes are listed in docs/mgn/spec.md section 10.
"""
from __future__ import annotations

import argparse
from copy import deepcopy
import json
from pathlib import Path
import re
import sys

# One absolute import block serves both the script and the package module, so an
# import added for one mode cannot break the other.
SCRIPTS = Path(__file__).resolve().parents[1]
if __name__ == "__main__" and str(SCRIPTS) not in sys.path:
    sys.path.insert(0, str(SCRIPTS))

from mgn.canonical import loads, sha256  # noqa: E402
from mgn import copy_definition  # noqa: E402
from mgn.causal import check_causal  # noqa: E402
from mgn.delta import null_answer  # noqa: E402
from mgn.identity import game_identity, record_identity  # noqa: E402
from mgn.keywords import is_keyword_characteristic  # noqa: E402
from mgn.oracle import Oracle, keyword_count, paragraphs  # noqa: E402
from mgn.schema import validate as schema_validate  # noqa: E402
from mgn.state import Problem, State  # noqa: E402
from mgn import vocabulary  # noqa: E402

PRIORITY_ANSWERS = {"play_land", "cast", "activate", "special_action", "concede"}  # passing is implied (spec 5.6)
# which fields of each answer hold object ids or seats
REFERENCE_FIELDS = {
    "choose_first": ["first"], "companion": ["card"], "bottom": ["cards"], "opening_hand_action": ["card"],
    "play_land": ["card"], "cast": ["card", "targets", "convoke", "delve", "improvise", "pay", "cost", "permission"],
    "cast_during_resolution": ["card", "targets", "convoke", "delve", "improvise", "pay", "cost", "permission"],
    "activate": ["source", "targets", "pay", "ability"], "special_action": ["object", "pay"],
    "declare_attackers": ["attacks", "pay"], "declare_blockers": ["blocks", "pay"], "assign_combat_damage": ["assignments"],
    "order_triggers": ["order"], "order_cards": ["order"], "legend_rule": ["keep"], "separate": ["piles"],
    "choose_replacement": ["effect"], "skip": ["effect"], "spell_destination": ["effect"],
    "choose_targets": ["targets"], "change_targets": ["targets"], "choose_objects": ["objects"],
    "choose_player": ["player"], "divide": ["amounts"], "attach": ["to"], "choose_defender": ["defender"],
    "discard": ["cards"], "arrange": ["top", "away"], "search": ["found"], "pay": ["pay"], "holders": ["holders"],
}
SKIP_KEYS = {"adding", "mana", "kind", "keyword", "rule", "face", "paragraph", "sub", "symbol", "life", "pool", "action", "value", "restriction", "type"}


def _references(value, path="", *, historical=False, trigger_instance=False):
    """Yield (id, exact path, historical provenance, seat-only) occurrences.

    Classify each occurrence, never each identity: the same departed token may be
    valid producer provenance and an invalid sacrifice/target in the same action.
    """
    if isinstance(value, str):
        yield value, path, historical, False
    elif isinstance(value, list):
        for index, item in enumerate(value):
            item_path = f"{path}/{index}"
            if isinstance(item, list) and len(item) == 2 and isinstance(item[1], int):
                yield from _references(item[0], f"{item_path}/0", historical=historical)
            else:
                yield from _references(item, item_path, historical=historical,
                                       trigger_instance=trigger_instance)
    elif isinstance(value, dict):
        for key, item in value.items():
            if key in SKIP_KEYS or key == "of" or (key == "effect" and isinstance(item, str)):
                continue
            item_path = f"{path}/{key}"
            if key == "chosen" and isinstance(item, dict):
                if "player" in item:
                    yield item["player"], f"{item_path}/player", False, True
                continue
            retained = historical or key in {"producer", "inherited_from", "granted_by"}
            # A trigger being ordered is an existing instance; these are event
            # participants, not a new activation's source or chosen targets.
            retained |= trigger_instance and key in {"source", "for"}
            yield from _references(item, item_path, historical=retained)


def _check_references(action, state: State, path: str, problems: list[Problem]):
    kind = action["kind"]
    convoked = [entry["object"] for entry in action.get("convoke", [])]
    delved = action.get("delve", [])
    for field, ids in (("convoke", convoked), ("delve", delved), ("improvise", action.get("improvise", []))):
        if len(ids) != len(set(ids)):
            problems.append(Problem("error", "coherence", f"{path}/{field}", "an object cannot pay this substitution twice (118.10)"))
    if set(action.get("improvise", [])) & set(convoked + delved):
        problems.append(Problem("error", "coherence", f"{path}/improvise", "an object cannot pay multiple substitutions (118.10)"))
    for field_name in REFERENCE_FIELDS.get(kind, []):
        if field_name not in action:
            continue
        entries = _references(action[field_name], f"{path}/{field_name}",
                              historical=field_name == "permission",
                              trigger_instance=kind == "order_triggers" and field_name == "order")
        for ref, ref_path, historical, seat_only in entries:
            if ref in state.seats:
                continue
            if seat_only:
                problems.append(Problem("error", "unknown_reference", ref_path, "not a seat of this game"))
                continue
            if state.exists(ref) or (historical and state.known(ref)):
                continue
            if ref in state.cards:
                if state.cards[ref].get("from") == "sideboard" and kind in ("companion", "special_action"):
                    continue
                problems.append(Problem("error", "unknown_reference", ref_path, f"{ref} is a card of the record but in no zone at this point"))
                continue
            problems.append(Problem("error", "unknown_reference", ref_path, f"{ref!r} is not an object or seat that exists at this point"))


def _check_answer(action, state: State, path: str, problems: list[Problem]):
    pending = state.pending
    kind = action["kind"]
    if pending is None:
        problems.append(Problem("error", "answer_kind", path, "the game has ended; nothing is pending"))
        return
    if kind == "concede":
        return  # any seat, at any boundary (104.3a)
    expected = PRIORITY_ANSWERS if pending["kind"] == "priority" else {pending["kind"]}
    if kind not in expected:
        problems.append(Problem("error", "answer_kind", f"{path}/kind", f"{kind} does not answer the pending {pending['kind']}"))
        return
    about = pending.get("about", {})
    if kind in ("bottom", "discard") and "count" in about and len(action["cards"]) != about["count"]:
        problems.append(Problem("error", "answer_kind", f"{path}/cards", f"{len(action['cards'])} cards, the question asks for {about['count']}"))
    if kind == "arrange" and "cards" in about:
        if sorted(action["top"] + action["away"]) != sorted(about["cards"]) or set(action["top"]) & set(action["away"]):
            problems.append(Problem("error", "answer_kind", path, "top and away must partition the cards looked at"))
    if kind == "order_cards" and "cards" in about:
        if sorted(action["order"]) != sorted(about["cards"]):
            problems.append(Problem("error", "answer_kind", f"{path}/order", "order must be a permutation of the simultaneous cards"))
    if kind == "order_triggers" and "abilities" in about:
        asked = sorted(sha256(a) for a in about["abilities"])
        given = sorted(sha256(a) for a in action["order"])
        if asked != given:
            problems.append(Problem("error", "answer_kind", f"{path}/order", "order must be a permutation of the abilities awaiting placement"))
    if kind == "legend_rule" and "objects" in about and action["keep"] not in about["objects"]:
        problems.append(Problem("error", "answer_kind", f"{path}/keep", "the kept permanent must be one of those the legend rule applies to"))
    if kind == "separate" and "cards" in about:
        flat = [card for pile in action["piles"] for card in pile]
        if sorted(flat) != sorted(about["cards"]) or len(flat) != len(set(flat)):
            problems.append(Problem("error", "answer_kind", f"{path}/piles", "the piles must partition the cards being separated"))
    if kind == "companion" and action.get("card") is not None:
        card = state.cards.get(action["card"], {})
        if card.get("owner") != pending["seat"] or card.get("from") != "sideboard":
            problems.append(Problem("error", "answer_kind", f"{path}/card", "a companion is revealed from outside the game: a sideboard card of that seat (103.2b, 702.139a)"))


def companion_candidates(state: State, seat: str, oracle: Oracle | None) -> list[str]:
    """The seat's sideboard cards with companion (702.139a), which 103.2b lets it reveal; without
    Oracle data every sideboard card of the seat."""
    out = []
    for card_id, card in state.cards.items():
        if card["owner"] != seat or card.get("from") != "sideboard":
            continue
        if oracle is not None and card["oracle"] in oracle:
            keywords = oracle.faces[card["oracle"]][0].raw.get("keywords", [])
            if not any(keyword.lower() == "companion" for keyword in keywords):
                continue
        out.append(card_id)
    return out


def _same_ability(facts: dict, instance: dict) -> bool:
    return facts.get("source") == instance.get("source") and facts.get("ability") == instance.get("ability")


COMBAT_STEPS = ("declare_attackers", "declare_blockers", "first_strike_damage", "combat_damage")


def _moved(delta: dict, object_id: str) -> bool:
    """Whether the delta moves the object (a move back into the same zone is a new object, 400.7)."""
    facts = (delta.get("objects") or {}).get(object_id)
    return isinstance(facts, dict) and "zone" in facts


def _check_coherence(action, before: State, created_before: int, state: State, delta: dict, path: str, problems: list[Problem]):
    """The delta agrees with the action it follows (spec section 4.1). The delta runs to the next
    recorded decision, so a fact is checked only where the state still shows the object concerned."""
    kind = action["kind"]
    about = (before.pending or {}).get("about", {})
    new_stack = [f"S{n}" for n in range(created_before + 1, state.stack_created + 1) if state.exists(f"S{n}")]
    if kind in ("cast", "cast_during_resolution") and "card" in action:
        card = action["card"]
        if state.zone_of(card) == "stack":
            for fact, default in (("targets", []), ("modes", []), ("division", []), ("fused", False)):
                if state.fact(card, fact) != action.get(fact, default):
                    problems.append(Problem("error", "coherence", f"{path}/{fact}", f"the spell's {fact} differ from the announcement"))
            if action.get("x") is not None and state.objects.get(card, {}).get("x") != action["x"]:
                problems.append(Problem("error", "coherence", f"{path}/x", "the spell's x differs from the announcement"))
            announced = {k: action[k] for k in ("cost", "permission", "additional", "convoke", "delve", "improvise") if k in action}
            if state.objects.get(card, {}).get("cast", {}) != announced:
                problems.append(Problem("error", "coherence", path, "the spell's cast fact must repeat the cost, permission, additional, convoke and delve choices of the announcement, or be absent when there are none"))
        elif state.zone_of(card) == before.zone_of(card) and not _moved(delta, card):
            problems.append(Problem("error", "coherence", path, f"{card} was cast but is still where it was, and not on the stack"))
    elif kind == "play_land":
        pending_about = ((state.pending or {}).get("about") or {}).get("object")
        card = action["card"]
        if state.zone_of(card) == before.zone_of(card) and not _moved(delta, card) and pending_about != card:
            problems.append(Problem("error", "coherence", path, f"{card} was played but is still where it was, and no question about it is pending"))
    elif kind == "activate" and "adding" not in action:
        matches = [s for s in new_stack if state.objects[s].get("kind") == "ability" and _same_ability(state.objects[s], action)]
        if matches and matches[0] in new_stack and state.objects[matches[0]].get("targets", []) != action.get("targets", []):
            problems.append(Problem("error", "coherence", f"{path}/targets", "the ability's targets differ from the announcement"))
    elif kind == "declare_attackers" and state.turn["step"] in COMBAT_STEPS and state.turn["number"] == before.turn["number"]:
        for creature, defender in action["attacks"]:
            if state.zone_of(creature) == "battlefield" and state.fact(creature, "attacking") != defender:
                problems.append(Problem("error", "coherence", f"{path}/attacks", f"{creature} was declared attacking {defender} but the delta does not say so"))
    elif kind == "declare_blockers" and state.turn["step"] in COMBAT_STEPS[1:] and state.turn["number"] == before.turn["number"]:
        for blocker, attacker in action["blocks"]:
            if state.zone_of(blocker) == "battlefield" and state.zone_of(attacker) == "battlefield" and attacker not in state.fact(blocker, "blocking"):
                problems.append(Problem("error", "coherence", f"{path}/blocks", f"{blocker} was declared blocking {attacker} but the delta does not say so"))
    elif kind in ("discard", "bottom"):
        for card in action["cards"]:
            if state.zone_of(card) == before.zone_of(card) and not _moved(delta, card):
                problems.append(Problem("error", "coherence", f"{path}/cards", f"{card} is still where it was"))
    elif kind in ("choose_targets", "change_targets", "choose_modes") and "object" in about:
        target = about["object"]
        # the targets of a copy are chosen before it is put onto the stack, so the question is
        # about the copying object and the answer's targets are the copy's (707.10c; spec 4.1)
        copies = [o for o, facts in (delta.get("objects") or {}).items() if isinstance(facts, dict) and facts.get("kind") == "copy"]
        if kind == "change_targets" and any(state.fact(o, "targets") == action.get("targets") for o in copies):
            pass
        elif state.zone_of(target) == "stack":  # a spell or ability that resolved within the stretch shows its effects instead
            fact = "modes" if kind == "choose_modes" else "targets"
            if state.fact(target, fact) != action[fact]:
                problems.append(Problem("error", "coherence", f"{path}/{fact}", f"{target}'s {fact} differ from the answer"))
            if kind == "choose_targets" and "division" in action and state.fact(target, "division") != action["division"]:
                problems.append(Problem("error", "coherence", f"{path}/division", f"{target}'s division differs from the answer"))
    elif kind == "order_triggers":
        # MGN 1.8 numbers new stack objects bottom first at their first
        # recorded boundary. This is a static numbering invariant even when
        # these are later triggers, unrelated to the answered order.
        stack = state.zones["stack"]  # top first
        depths = [stack.index(s) for s in new_stack if s in stack]
        if depths != sorted(depths, reverse=True):
            problems.append(Problem("error", "coherence", f"{path}/order", "new stack object numbers are not bottom first"))
        # 500.2 / 117.4: changing steps requires the stack to empty. The
        # ordered triggers therefore resolved inside this implied stretch.
        if state.turn != before.turn:
            return
        # Even a unique source/ability match can be a later occurrence of
        # that same ability after the originals resolved in this step. Stack
        # facts carry no event identity (nor pending instances' `for` subjects),
        # so matching declarations cannot authenticate occurrence survival.
        if any(state.objects[s].get("kind") == "ability"
               and _same_ability(state.objects[s], instance)
               for s in new_stack for instance in action["order"]):
            problems.append(Problem("warning", "coherence", f"{path}/order",
                "matching surviving source/ability declarations do not identify the answered occurrences; temporal order coherence is unproved"))
    elif kind == "legend_rule" and "objects" in about:
        for other in about["objects"]:
            if other != action["keep"] and state.zone_of(other) == "battlefield":
                problems.append(Problem("error", "coherence", f"{path}/keep", f"{other} should have gone to its owner's graveyard (704.5j)"))


def _check_null_answer(action, before: State, path: str, problems: list[Problem]):
    """A null answer is implied, never written (spec 5.6)."""
    if null_answer(action, before):
        problems.append(Problem("error", "null_answer", path, f"{action['kind']} with its null answer is implied by the next entry and is not recorded"))


def _ability_refs(value, carried_source=None):
    """(source, ability ref) pairs anywhere in a JSON value: an `ability` member next to a `source`."""
    out = []
    if isinstance(value, dict):
        source = value.get("source", carried_source)
        if "ability" in value and isinstance(value["ability"], dict):
            ability = value["ability"]
            if "paragraph" in ability:
                out.append((source, ability))
            elif "inherited_from" in ability:
                out.extend(_ability_refs(ability))
        for key, item in value.items():
            if key != "ability":
                out.extend(_ability_refs(item, source))
    elif isinstance(value, list):
        for item in value:
            out.extend(_ability_refs(item, carried_source))
    return out


def _check_abilities(value, state: State, oracle: Oracle, path: str, problems: list[Problem]):
    """With Oracle data: every ability reference names a paragraph the source has (spec 3.3)."""
    for source, ability in _ability_refs(value):
        if source is None:
            continue
        owner = ability.get("granted_by", source)
        # a copy's abilities are those of the copied object's text (707.2), unless the reference says
        # `of: card`: an ability the copying card keeps or adds as an exception (707.9b)
        seen = set()
        while (ability.get("of") != "card" and owner not in state.copy_bases
               and owner not in seen and state.reference_facts(owner).get("copy_of")):
            seen.add(owner)
            owner = state.reference_facts(owner)["copy_of"]
        retained = state.reference_facts(owner).get("copy_definition")
        if (owner in state.copy_bases or retained is not None) and ability.get("of") != "card":
            basis = state.copy_bases.get(owner) if owner in state.copy_bases else copy_definition.basis(retained)
            full = copy_definition.characteristics(basis, oracle)
            if full is None:
                continue
            lines = paragraphs(full["text"])
        elif owner in state.cards:
            card = state.cards[owner]
            if card["oracle"] not in oracle:
                continue
            face = ability.get("face", 0)
            if face >= oracle.face_count(card["oracle"]):
                problems.append(Problem("error", "ability", path, f"{owner} has no face {face}"))
                continue
            lines = oracle.paragraphs(card["oracle"], face)
        else:
            facts = state.reference_facts(owner)
            if "token" not in facts:
                continue
            lines = paragraphs(facts["token"].get("text", ""))
        paragraph = ability["paragraph"]
        if paragraph > len(lines):
            problems.append(Problem("error", "ability", path, f"{owner} has {len(lines)} numbered paragraph(s), not {paragraph}"))
            continue
        if "sub" in ability:
            count = keyword_count(lines[paragraph - 1])
            if count < 2 or ability["sub"] > count:
                problems.append(Problem("error", "ability", path, f"paragraph {paragraph} of {owner} is not a run of {ability['sub']} or more keyword abilities"))


def _ability_text(source, ability: dict, state: State, oracle: Oracle) -> str | None:
    """The paragraph an ability reference names, or None when the text is not known."""
    if "inherited_from" in ability:
        donor = ability["inherited_from"]
        return _ability_text(donor["source"], donor["ability"], state, oracle)
    if "rule" in ability:
        return "{T}: Add one mana of the land's type." if ability["rule"] == "305.6" else None
    owner = ability.get("granted_by", source)
    seen = set()
    while (ability.get("of") != "card" and owner not in state.copy_bases
           and owner not in seen and state.reference_facts(owner).get("copy_of")):
        seen.add(owner)
        owner = state.reference_facts(owner)["copy_of"]
    retained = state.reference_facts(owner).get("copy_definition")
    if (owner in state.copy_bases or retained is not None) and ability.get("of") != "card":
        basis = state.copy_bases.get(owner) if owner in state.copy_bases else copy_definition.basis(retained)
        full = copy_definition.characteristics(basis, oracle)
        if full is None:
            return None
        lines = paragraphs(full["text"])
    elif owner in state.cards:
        card = state.cards[owner]
        if card["oracle"] not in oracle:
            return None
        face = ability.get("face", 0)
        if face >= oracle.face_count(card["oracle"]):
            return None
        lines = oracle.paragraphs(card["oracle"], face)
    else:
        facts = state.reference_facts(owner)
        if "token" not in facts:
            return None
        lines = paragraphs(facts["token"].get("text", ""))
    paragraph = ability.get("paragraph", 0)
    if not 1 <= paragraph <= len(lines):
        return None
    return lines[paragraph - 1]


def _check_mana_ability(action, state: State, oracle: Oracle, path: str, problems: list[Problem]):
    """`adding` is present exactly when the activated ability is a mana ability (spec 5.2): an
    activated ability that could add mana, has no target and is not a loyalty ability (605.1a)."""
    text = _ability_text(action["source"], action["ability"], state, oracle)
    if text is None:
        return
    loyalty = re.match(r"^[+−-]?\d+:", text) is not None
    mana = re.search(r"\bAdd\b", text) is not None and "target" not in text.lower() and not loyalty
    if mana and "adding" not in action:
        problems.append(Problem("error", "ability", f"{path}/adding", "a mana ability adds mana and uses no stack: record what it added (605.1a, 605.3)"))
    elif not mana and "adding" in action:
        problems.append(Problem("error", "ability", f"{path}/adding", "adding belongs to a mana ability only; this ability adds no mana (605.1a)"))


def _check_nested_mana_abilities(payment, state, oracle, path, problems):
    for index, ability in enumerate(payment.get("abilities", [])):
        where = f"{path}/abilities/{index}"
        _check_mana_ability(ability, state, oracle, where, problems)
        _check_nested_mana_abilities(ability.get("pay", {}), state, oracle, f"{where}/pay", problems)


def _check_spellings(record, problems: list[Problem]):
    """Values outside their closed sets, and counter kinds the pinned text does not know."""
    known_counters = set(vocabulary.counter_kinds())
    subtypes = vocabulary.all_subtypes()

    def walk(value, path):
        if isinstance(value, dict):
            for key, item in value.items():
                child = f"{path}/{key}"
                if key == "counters" and isinstance(item, dict):
                    for kind in item:
                        if kind not in known_counters:
                            problems.append(Problem("warning", "counter_kind", f"{child}/{kind}", "counter kind not found in the pinned card text or rules"))
                elif key == "keywords" and isinstance(item, list):
                    for word in item:
                        if isinstance(word, str) and not is_keyword_characteristic(word):
                            problems.append(Problem("error", "spelling", child, f"{word!r} must name a pinned keyword in lowercase, retaining its printed parameter"))
                elif key == "subtypes" and isinstance(item, list):
                    for word in item:
                        if isinstance(word, str) and word not in subtypes:
                            problems.append(Problem("error", "spelling", child, f"{word!r} is not a subtype the pinned rules list"))
                walk(item, child)
        elif isinstance(value, list):
            for i, item in enumerate(value):
                walk(item, f"{path}/{i}")

    walk(record.get("start"), "/start")
    for i, transition in enumerate(record["transitions"]):
        walk(transition.get("delta", {}), f"/transitions/{i}/delta")


def _check_oracle(record, oracle: Oracle, problems: list[Problem]):
    for card_id, card in record["cards"].items():
        path = f"/cards/{card_id}"
        if card["oracle"] not in oracle:
            problems.append(Problem("error", "oracle", f"{path}/oracle", "unknown Oracle id in the pinned data"))
            continue
        if oracle.name(card["oracle"]) != card["name"]:
            problems.append(Problem("error", "oracle", f"{path}/name", f"the pinned name is {oracle.name(card['oracle'])!r}"))
        if "faces" in card:
            pinned = oracle.digests(card["oracle"])
            if len(card["faces"]) != len(pinned):
                problems.append(Problem("error", "oracle", f"{path}/faces", f"the card has {len(pinned)} face(s)"))
            else:
                for index, (given, expected) in enumerate(zip(card["faces"], pinned)):
                    if given != expected:
                        problems.append(Problem("error", "oracle", f"{path}/faces/{index}", f"the pinned face digest is {expected}"))


def _check_prepared_copies(state: State, oracle: Oracle | None, path: str, problems: list[Problem]):
    """722.3c copies the actual copiable prepare face, not the source's current face."""
    for object_id, facts in state.objects.items():
        if "copy_face" not in facts:
            continue
        where = f"{path}/objects/{object_id}/copy_face"
        if not vocabulary.STACK_ID.match(object_id) or facts.get("kind") != "copy" or facts.get("copy_face") != 1 or facts.get("face", 0) != 0:
            problems.append(Problem("error", "delta", where, "copy_face is the prepared-copy exception for an S object (722.3c)"))
            continue
        basis = state.copy_bases.get(object_id)
        if basis is None:
            problems.append(Problem("error", "oracle", where, "prepare copy needs complete alternative characteristics from a pinned card through its copy lineage; an arbitrary token definition is insufficient (722.3c)"))
        elif not isinstance(basis, tuple) or oracle is not None and (basis[0] not in oracle or
                oracle.face_count(basis[0]) <= 1 or
                oracle.faces[basis[0]][0].raw.get("layout") != "prepare"):
            problems.append(Problem("error", "oracle", where, "copy_face needs the alternative face of a pinned preparation card (722.3c)"))


def _check_copy_definitions(state: State, oracle: Oracle | None, path: str, problems: list[Problem]):
    """Intrinsic definitions are assertions, not permission to override known copy history."""
    for object_id, facts in state.objects.items():
        definition = facts.get("copy_definition")
        if definition is None:
            continue
        where = f"{path}/objects/{object_id}/copy_definition"
        value = copy_definition.basis(definition)
        if isinstance(value, tuple) and oracle is not None:
            oid, face = value
            if oid not in oracle or (not oracle.has_fuse(oid) if face == "fused"
                                     else face >= oracle.face_count(oid)):
                problems.append(Problem("error", "oracle", where, "copy definition needs an existing pinned Oracle spell face"))
        full = copy_definition.whole(value, oracle)
        if full is not None and "faces" in full and state.fact(object_id, "face") != full["face"]:
            problems.append(Problem("error", "coherence", where, "a double-faced spell copy must retain the copied face up (707.10g)"))
        authenticated = state.copy_authentications.get(object_id)
        if authenticated is not None and copy_definition.equivalent(value, authenticated, oracle) is False:
            problems.append(Problem("error", "coherence", where, "copy definition contradicts its unchanged donor's copiable characteristics (707.2)"))


def _check_power_toughness(state: State, oracle: Oracle, path: str, problems: list[Problem]):
    """208.3: a noncreature permanent has no power or toughness; a card elsewhere keeps its printed ones."""
    for object_id, facts in state.objects.items():
        if "power" not in facts and "toughness" not in facts:
            continue
        zone = state.zone_of(object_id)
        if zone is None or object_id not in state.cards:
            continue
        card = state.cards[object_id]
        if card["oracle"] not in oracle:
            continue
        printed = oracle.printed(card["oracle"], facts.get("face", 0))
        null = [k for k in ("power", "toughness") if k in facts and facts[k] is None]
        if not null:
            continue
        if zone == "stack":
            # a station card has no power or toughness off the battlefield (721.2c)
            if any(isinstance(printed.get(k), int) for k in null) and "station" not in printed.get("keywords", []):
                problems.append(Problem("error", "delta", f"{path}/objects/{object_id}", "a creature spell on the stack keeps its printed power and toughness (208.3)"))
        elif zone == "battlefield":
            types = facts.get("types", printed.get("types", []))
            if "creature" in types:
                problems.append(Problem("error", "delta", f"{path}/objects/{object_id}", "a creature has power and toughness; null is for a noncreature permanent (208.3)"))


def _check_payment_lots(action, path: str, problems: list[Problem]):
    """Typed lot selectors remain consistent at every recursive payment depth."""
    def walk(value, where):
        if isinstance(value, dict):
            if "symbol" in value and "mana" in value and "restricted" in value:
                if value["restricted"]["type"] != value["mana"]:
                    problems.append(Problem("error", "coherence", f"{where}/restricted/type", "restricted lot type must equal the allocated mana type"))
            for key, item in value.items():
                walk(item, f"{where}/{key}")
        elif isinstance(value, list):
            for index, item in enumerate(value):
                walk(item, f"{where}/{index}")
    walk(action, path)


def _check_fused(state: State, oracle: Oracle | None, path: str, problems: list[Problem], action=None):
    """Fuse combines both halves of a spell, never a permanent (702.102)."""
    if action is not None:
        if not action.get("fused"):
            return
        card = action.get("card")
        if action.get("face", 0) != 0 or state.zone_of(card) != f"{state.cards.get(card, {}).get('owner')}.hand":
            problems.append(Problem("error", "coherence", path, "fuse casts both halves from hand, without a separate face choice (702.102a)"))
        candidates = [(card, {"fused": True})]
    else:
        candidates = [(oid, facts) for oid, facts in state.objects.items() if facts.get("fused")]
    for oid, facts in candidates:
        if action is None and (state.zone_of(oid) != "stack" or facts.get("face", 0) != 0 or state.is_ability(oid)):
            problems.append(Problem("error", "coherence", f"{path}/objects/{oid}/fused", "fused describes one combined split spell on the stack, with face 0 (702.102b)"))
        card = state.cards.get(oid)
        basis = state.copy_bases.get(oid)
        oracle_id = card.get("oracle") if card else basis[0] if isinstance(basis, tuple) else None
        if isinstance(basis, tuple) and basis[1] != "fused":
            problems.append(Problem("error", "coherence", path, "a copy cannot add fuse to a donor that was cast as one half (707.10)"))
        if oracle is not None and (oracle_id not in oracle or not oracle.has_fuse(oracle_id)):
            problems.append(Problem("error", "oracle", path, "fused requires a pinned two-half split card with fuse (702.102a)"))


def _check_cast_payment(action, state: State, oracle: Oracle, path: str, problems: list[Problem]):
    """A spell cast for its mana cost records how the mana was paid (spec 5.8)."""
    card = action.get("card")
    if card not in state.cards or state.cards[card]["oracle"] not in oracle:
        return
    face = oracle.faces[state.cards[card]["oracle"]][min(action.get("face", 0), oracle.face_count(state.cards[card]["oracle"]) - 1)]
    mana_cost = ("".join(f.raw.get("mana_cost") or "" for f in oracle.faces[state.cards[card]["oracle"]])
                 if action.get("fused") else face.raw.get("mana_cost") or "")
    cost = action.get("cost", {"kind": "mana"})
    if mana_cost and cost.get("kind") == "mana" and "pay" not in action:
        problems.append(Problem("error", "coherence", path, f"{card} costs {mana_cost}; the payment must be recorded (pay)"))


def _check_assumptions(record, state: State, problems: list[Problem]):
    assumptions = record["start"].get("assumptions", {})
    for key in ("land_played", "spells_cast"):
        for seat in assumptions.get(key, {}):
            if seat not in state.seats:
                problems.append(Problem("error", "unknown_reference", f"/start/assumptions/{key}/{seat}", "not a seat of this game"))
    for object_id in assumptions.get("summoning_sick", []):
        if state.zone_of(object_id) != "battlefield":
            problems.append(Problem("error", "unknown_reference", "/start/assumptions/summoning_sick", f"{object_id} is not a permanent of the start position"))
    if assumptions.get("consecutive_passes", 0) >= len(state.seats):
        problems.append(Problem("error", "start", "/start/assumptions/consecutive_passes", f"at most {len(state.seats) - 1} seats can have passed in succession without the round ending (117.4)"))


def validate(record: dict, oracle: Oracle | None = None) -> list[Problem]:
    problems: list[Problem] = []
    for error in schema_validate(record):
        problems.append(Problem("error", "schema", error.path, f"{error.message} [{error.keyword}]"))
    if problems:
        return problems
    for card_id in record["cards"]:
        if any(p.match(card_id) for p in (vocabulary.TOKEN_ID, vocabulary.STACK_ID, vocabulary.EMBLEM_ID, vocabulary.SEAT)):
            problems.append(Problem("error", "duplicate_id", f"/cards/{card_id}", "a physical id must not look like a seat, a token (T<n>), a stack object (S<n>) or an emblem (E<n>)"))
    for card_id, card in record["cards"].items():
        if card["owner"] not in record["game"]["seats"]:
            problems.append(Problem("error", "unknown_reference", f"/cards/{card_id}/owner", "not a seat of this game"))
    if oracle is not None:
        _check_oracle(record, oracle, problems)
    state = State.from_record(record, problems)
    _check_fused(state, oracle, "/start/state", problems)
    _check_assumptions(record, state, problems)
    check_causal(record, state, oracle, problems)
    _check_prepared_copies(state, oracle, "/start/state", problems)
    _check_copy_definitions(state, oracle, "/start/state", problems)
    if oracle is not None and record["start"]["kind"] == "position":
        _check_abilities(record["start"]["state"], state, oracle, "/start/state", problems)
    checkpoints = record.get("checkpoints", {})
    for index in checkpoints:
        if int(index) > len(record["transitions"]):
            problems.append(Problem("error", "checkpoint", f"/checkpoints/{index}", "names a transition the record does not have"))
    companions: dict = {}  # seat -> the companion it revealed (103.2b), which the special action puts into its hand (116.2g)
    if state.pending is not None and state.pending["kind"] == "companion" and not companion_candidates(state, state.pending["seat"], oracle):
        problems.append(Problem("error", "pending", "/start", f"{state.pending['seat']} owns no sideboard card with companion; no companion question is asked of it (103.2b)"))
    for i, transition in enumerate(record["transitions"]):
        path = f"/transitions/{i}"
        action = transition.get("do", transition.get("attempt"))
        if state.pending is not None and transition["seat"] != state.pending["seat"] and action["kind"] != "concede":
            problems.append(Problem("error", "pending_seat", f"{path}/seat", f"{transition['seat']} acts but {state.pending['seat']} must answer"))
        action_path = f"{path}/do" if "do" in transition else f"{path}/attempt"
        _check_answer(action, state, action_path, problems)
        _check_references(action, state, action_path, problems)
        _check_payment_lots(action, action_path, problems)
        if "do" in transition and action["kind"] == "companion":
            if action.get("card") is not None and action["card"] not in companion_candidates(state, transition["seat"], oracle):
                problems.append(Problem("error", "answer_kind", f"{action_path}/card", f"{action['card']} is not a sideboard card of {transition['seat']} with companion (702.139a)"))
            companions[transition["seat"]] = action.get("card")
        if "do" in transition and action["kind"] == "special_action" and action.get("action") == "companion":
            if action.get("object") != companions.get(transition["seat"]):
                problems.append(Problem("error", "answer_kind", f"{action_path}/object", "the companion special action puts the companion that seat revealed into its hand (116.2g, 702.139a)"))
        if action["kind"] in ("cast", "cast_during_resolution") and "do" in transition:
            _check_fused(state, oracle, action_path, problems, action)
        if oracle is not None:
            _check_abilities(action, state, oracle, action_path, problems)
            _check_nested_mana_abilities(action.get("pay", {}), state, oracle, f"{action_path}/pay", problems)
            if action["kind"] in ("cast", "cast_during_resolution") and "do" in transition:
                _check_cast_payment(action, state, oracle, action_path, problems)
            if action["kind"] == "activate":
                _check_mana_ability(action, state, oracle, action_path, problems)
        if "attempt" in transition:
            if "delta" not in transition:
                continue  # the next entry answers the question in the same state
            if not transition["delta"] and not transition.get("random"):
                problems.append(Problem("error", "delta", f"{path}/delta", "an attempt carries a delta only when the implied answers that followed it changed the state; omit an empty one"))
            state.apply(transition["delta"], transition.get("random", []), f"{path}/delta", problems)
        else:
            _check_null_answer(action, state, action_path, problems)
            before = deepcopy(state)
            created_before = state.stack_created
            state.apply(transition["delta"], transition.get("random", []), f"{path}/delta", problems)
            _check_coherence(action, before, created_before, state, transition["delta"], action_path, problems)
        _check_fused(state, oracle, f"{path}/delta", problems)
        _check_prepared_copies(state, oracle, f"{path}/delta", problems)
        _check_copy_definitions(state, oracle, f"{path}/delta", problems)
        if oracle is not None:
            _check_abilities(transition["delta"], state, oracle, f"{path}/delta", problems)
            _check_power_toughness(state, oracle, f"{path}/delta", problems)
        if state.pending is not None and state.pending["kind"] == "companion" and not companion_candidates(state, state.pending["seat"], oracle):
            problems.append(Problem("error", "pending", f"{path}/delta/pending", f"{state.pending['seat']} owns no sideboard card with companion; no companion question is asked of it (103.2b)"))
        if str(i + 1) in checkpoints:
            if oracle is None and state.tier == 2:
                problems.append(Problem("warning", "checkpoint", f"/checkpoints/{i + 1}", "not verified: printed values need --oracle"))
            else:
                digest = sha256(state.explicit(oracle))
                if digest != checkpoints[str(i + 1)]:
                    problems.append(Problem("error", "checkpoint", f"/checkpoints/{i + 1}", f"the explicit projection hashes to {digest}"))
    _check_spellings(record, problems)
    return problems


def main(argv=None) -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("record", type=Path)
    parser.add_argument("--oracle", type=Path, help="pinned Oracle faces (resources/oracle/cards.jsonl) to check names, faces, abilities and printed values")
    parser.add_argument("--json", action="store_true", help="print the problems as JSON")
    parser.add_argument("--identity", action="store_true", help="also print the record and game identities")
    args = parser.parse_args(argv)
    try:
        record = loads(args.record.read_text(encoding="utf-8"))
    except (OSError, ValueError) as error:
        print(f"error parse /: {error}")
        return 1
    oracle = Oracle(args.oracle) if args.oracle else None
    problems = validate(record, oracle)
    if args.json:
        print(json.dumps([problem.__dict__ for problem in problems], indent=1))
    else:
        for problem in problems:
            print(problem)
    errors = sum(1 for problem in problems if problem.severity == "error")
    if args.identity and not errors:
        print(f"record {record_identity(record)}")
        print(f"game {game_identity(record)}")
    if not problems:
        print("ok")
    return 1 if errors else 0


if __name__ == "__main__":
    sys.exit(main())
