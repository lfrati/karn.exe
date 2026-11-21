"""What the unread cards lack, by kind of phrase, weighed by the published decks they are in.

    uv run scripts/grammar_gaps.py GAPS.jsonl [--kind KIND] [--grep RE] [--limit N] [--examples N]

Reads what `target/fast/grammar-gaps OUT.jsonl` wrote ([grammar](../docs/grammar.md)): for
each row the grammar does not read, the shortest edits that let it read. Without --kind it
prints how many cards lack each kind of thing. With one, it prints that kind's gaps grouped by
their template, so that the phrases a rule must cover are read together and not card by
card; --grep keeps the gaps whose words in context match.

`decks` counts roster main decks (`resources/decks/selected-decks.jsonl`) holding a card with
the gap, each deck once per card: it orders work and is no promise that closing a gap makes a
deck run. An edit is evidence of what is missing, never a reading of the card.
"""
from __future__ import annotations

import argparse
import collections
import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT / "scripts"))
import deck_archive  # noqa: E402

ROSTER = ROOT / "resources/decks/selected-decks.jsonl"
CORPUS = ROOT / "resources/oracle/cards.jsonl"

# the sub-grammar that owns each kind of stand-in
OWNER = {
    "a thing": "noun phrase", "things": "noun phrase", "a player": "noun phrase", "whose": "noun phrase",
    "a noun": "noun phrase", "a subtype": "noun phrase", "an adjective": "noun phrase", "a zone": "noun phrase",
    "a counter": "noun phrase", "a keyword": "noun phrase",
    "an amount": "amount", "a condition": "condition", "an event": "event", "a duration": "duration",
    "a cost": "cost", "an instruction": "instruction", "a quoted ability": "instruction",
}
UNKNOWN_FRAME = "a sentence no edit explains"


def read_jsonl(path: Path) -> list[dict]:
    return [json.loads(line) for line in path.read_text().splitlines() if line.strip()]


def kind_of(gap: dict) -> str:
    """What a gap says the grammar lacks."""
    if gap["edit"] == "removed":
        return "a known phrase out of place" if gap["reads_as"] else "an extra phrase"
    if gap["edit"] == "a sentence":
        return "a whole sentence"
    if not gap["span"]:
        return "a missing word"
    return f"a new way to say: {OWNER[gap['edit']]}"


def main_decks(roster: list[dict], corpus: list[dict]) -> dict[str, int]:
    """The roster main decks each card is in, by Oracle identity."""
    by_name = {card["name"]: card["oracle_id"] for card in corpus}
    decks: collections.Counter = collections.Counter()
    for deck in roster:
        for name in deck_archive.deck_cards({**deck, "sideboard": {}}):
            if name in by_name:
                decks[by_name[name]] += 1
    return dict(decks)


def entries(rows: list[dict]):
    """Each gap and each unexplained stop, with its kind and its words in context."""
    for row in rows:
        for gap in row["gaps"]:
            yield row, kind_of(gap), gap["template"], f"{gap['before']} ⟦{gap['span']}⟧ {gap['after']}"
        stop = row["unexplained"]
        if stop:
            rest = stop["rest"][len(stop["at"]):]
            yield row, UNKNOWN_FRAME, stop["at"].lower(), f"…{stop['read'][-60:]} ⟦{stop['at']}⟧{rest[:80]}"


def kinds(rows: list[dict], decks: dict[str, int]) -> list[dict]:
    """Cards lacking each kind of thing: `only` of them lack nothing else."""
    cards = collections.defaultdict(set)
    of_card = collections.defaultdict(set)
    gaps: collections.Counter = collections.Counter()
    for row, kind, _, _ in entries(rows):
        cards[kind].add(row["oracle_id"])
        of_card[row["oracle_id"]].add(kind)
        gaps[kind] += 1
    only = collections.Counter(next(iter(found)) for found in of_card.values() if len(found) == 1)
    table = [{"kind": kind, "cards": len(ids), "only": only[kind], "decks": sum(decks.get(i, 0) for i in ids),
              "gaps": gaps[kind]} for kind, ids in cards.items()]
    return sorted(table, key=lambda line: (-line["cards"], line["kind"]))


def phrases(rows: list[dict], decks: dict[str, int], kind: str, grep: str | None = None) -> list[dict]:
    """The gaps of one kind grouped by template, most cards first, each with its examples
    most-played first."""
    groups = collections.defaultdict(lambda: {"cards": set(), "examples": []})
    for row, found, template, words in entries(rows):
        if found != kind or (grep and not re.search(grep, words, re.IGNORECASE)):
            continue
        group = groups[template]
        group["cards"].add(row["oracle_id"])
        group["examples"].append((decks.get(row["oracle_id"], 0), row["face"], words))
    table = [{"template": template, "cards": len(group["cards"]),
              "decks": sum(decks.get(i, 0) for i in group["cards"]),
              "examples": sorted(group["examples"], key=lambda e: (-e[0], e[1], e[2]))}
             for template, group in groups.items()]
    return sorted(table, key=lambda line: (-line["cards"], -line["decks"], line["template"]))


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("gaps", type=Path)
    parser.add_argument("--kind")
    parser.add_argument("--grep")
    parser.add_argument("--limit", type=int, default=60)
    parser.add_argument("--examples", type=int, default=2)
    args = parser.parse_args()
    rows = read_jsonl(args.gaps)
    decks = main_decks(read_jsonl(ROSTER), read_jsonl(CORPUS))
    if not args.kind:
        unread = {row["oracle_id"] for row in rows}
        whole = collections.defaultdict(list)
        for row in rows:
            whole[row["oracle_id"]].append(row)
        explained = [card for card in whole.values() if not any(row["unexplained"] for row in card)]
        one = sum(1 for card in explained if sum(len(row["gaps"]) for row in card) == 1)
        print(f"{len(unread)} cards with a row the grammar does not read; edits explain every such row of "
              f"{len(explained)}, and {one} are one gap from reading")
        print(f"{'cards':>6} {'only':>6} {'decks':>6} {'gaps':>6}  kind")
        for line in kinds(rows, decks):
            print(f"{line['cards']:6d} {line['only']:6d} {line['decks']:6d} {line['gaps']:6d}  {line['kind']}")
        return
    table = phrases(rows, decks, args.kind, args.grep)
    for line in table[:args.limit]:
        print(f"{line['cards']:5d} cards {line['decks']:5d} decks  {line['template']}")
        for played, face, words in line["examples"][:args.examples]:
            print(f"        {played:4d} {face[:24]:24s} {words}")
    print(f"{len(table)} templates, {sum(1 for line in table if line['cards'] == 1)} on one card")


if __name__ == "__main__":
    main()
