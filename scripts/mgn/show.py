#!/usr/bin/env python3
"""Print an MGN record for a reader: the start, then each transition with its decision, its random
outcomes, its delta and the state it leads to, card ids spelled with their names.

    uv run scripts/mgn/show.py tests/fixtures/mgn/vivi-decks/record.json --from 10 --to 19

A reviewing agent reads this instead of the raw deltas; nothing here runs an engine, so what it
prints is the record's own claim of each state, derived by applying its deltas (spec 4), not what
any engine did.
"""
from __future__ import annotations

import argparse
import json
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parents[2]
if __name__ == "__main__" and str(ROOT / "scripts") not in sys.path:
    sys.path.insert(0, str(ROOT / "scripts"))

from mgn.canonical import loads  # noqa: E402
from mgn.oracle import Oracle  # noqa: E402
from mgn.state import State  # noqa: E402

OBJECT_FACTS = ("tapped", "damage", "counters", "power", "toughness", "attacking", "blocking", "blocked", "attached_to",
                "face", "face_down", "chosen", "kind", "source", "ability", "targets", "modes", "x", "copy_of", "copy_definition", "cast", "token", "granted", "keywords")


def label(state: State, oracle: Oracle, object_id: str) -> str:
    card = state.cards.get(object_id)
    if card:
        return f"{object_id} ({oracle.name(card['oracle'])}, {card['owner']})"
    facts = state.objects.get(object_id, {})
    if facts.get("token"):
        return f"{object_id} (token {facts['token'].get('name', '')}, {facts.get('controller')})"
    if facts.get("kind") == "ability":
        return f"{object_id} (ability of {facts.get('source')} paragraph {facts.get('ability', {}).get('paragraph')}, {facts.get('controller')})"
    if facts.get("kind") == "copy":
        return f"{object_id} (copy of {facts.get('copy_of')}, {facts.get('controller')})"
    return object_id


def describe(state: State, oracle: Oracle, out) -> None:
    print(f"  turn {state.turn}  pending {state.pending}  result {state.result}", file=out)
    for seat in state.seats:
        print(f"  {seat}: {json.dumps(state.players.get(seat, {}), sort_keys=True)}", file=out)
    game_facts = {k: v for k, v in state.game.items() if v not in (None, {}, [])}
    if game_facts:
        print(f"  game: {json.dumps(game_facts, sort_keys=True)}", file=out)
    for zone in sorted(state.zones):
        members = state.listing(zone)
        if not members:
            continue
        if zone.endswith(".library") and len(members) > 6:
            print(f"  {zone} ({len(members)}, top first): {', '.join(label(state, oracle, o) for o in members[:3])}; bottom {label(state, oracle, members[-1])}", file=out)
            continue
        order = " (oldest first, top last)" if zone.endswith(".graveyard") else " (bottom first, top last)" if zone == "stack" else " (top first)" if zone.endswith(".library") else ""
        print(f"  {zone} ({len(members)}){order}:", file=out)
        for object_id in members:
            all_facts = state.objects.get(object_id, {})
            facts = {k: v for k, v in all_facts.items() if k in OBJECT_FACTS and v not in ({}, [], None, False)}
            card = state.cards.get(object_id)
            if card and all_facts.get("controller") not in (None, card["owner"]):
                facts["controller"] = all_facts["controller"]  # a control change (110.2) is worth seeing
            suffix = f"  {json.dumps(facts, sort_keys=True)}" if facts and zone in ("battlefield", "stack", "exile", "command") else ""
            print(f"    {label(state, oracle, object_id)}{suffix}", file=out)


def spell(value, state: State, oracle: Oracle):
    """The decision with each card id followed by its name."""
    if isinstance(value, dict):
        return {k: spell(v, state, oracle) for k, v in value.items()}
    if isinstance(value, list):
        return [spell(v, state, oracle) for v in value]
    if isinstance(value, str) and value in state.cards:
        return f"{value}={oracle.name(state.cards[value]['oracle'])}"
    return value


def main(argv=None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    parser.add_argument("record", type=Path)
    parser.add_argument("--from", dest="start", type=int, default=1, help="first transition to print (1-based)")
    parser.add_argument("--to", dest="end", type=int, default=None, help="last transition to print")
    parser.add_argument("--oracle", type=Path, default=None)
    args = parser.parse_args(argv)
    oracle = Oracle(args.oracle) if args.oracle else Oracle()
    record = loads(args.record.read_text(encoding="utf-8"))
    problems: list = []
    state = State.from_record(record, problems)
    out = sys.stdout
    print(f"record {args.record}: tier {record['tier']}, {len(record['transitions'])} transitions, seats {record['game']['seats']}, life {record['game']['life']}", file=out)
    if record.get("note"):
        print(f"note: {record['note']}", file=out)
    print("cards:", ", ".join(f"{cid}={oracle.name(card['oracle'])} ({card['owner']})" for cid, card in record["cards"].items()), file=out)
    print("start:", file=out)
    describe(state, oracle, out)
    end = args.end or len(record["transitions"])
    for index, transition in enumerate(record["transitions"]):
        n = index + 1
        action = transition.get("do", transition.get("attempt"))
        has_delta = "delta" in transition
        if n >= args.start and n <= end:
            print(f"\n== transition {n}: {transition['seat']} {'ATTEMPT (illegal)' if 'attempt' in transition else 'does'} {json.dumps(spell(action, state, oracle), sort_keys=True)}", file=out)
            if transition.get("random"):
                for outcome in transition["random"]:
                    short = {k: (v if k != "order" else f"{len(v)} cards, top {v[:3]}") for k, v in outcome.items()}
                    print(f"  random: {json.dumps(short, sort_keys=True)}", file=out)
            if transition.get("note"):
                print(f"  note: {transition['note']}", file=out)
            print(f"  delta: {json.dumps(spell(transition.get('delta'), state, oracle), sort_keys=True)}", file=out)
        if has_delta:
            state.apply(transition["delta"], transition.get("random", []), f"/transitions/{index}/delta", problems)
        if n >= args.start and n <= end and has_delta:
            print("  state after:", file=out)
            describe(state, oracle, out)
        if n >= end:
            break
    errors = [str(p) for p in problems if p.severity == "error"]
    if errors:
        print("\nproblems applying the deltas: " + "; ".join(errors[:5]), file=out)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
