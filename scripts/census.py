"""How many cards each IR constructor serves: grammar or phrasebook?

    cargo run --profile fast -p grammar --bin grammar-census      # writes target/census.jsonl
    uv run python scripts/census.py [--list N] [--enum NAME]

The round trip passes a rule that memorizes one sentence as readily as a rule that captures a
structure. A memorized sentence needs a constructor of its own to print from, so the constructors
that serve only a few cards are the phrasebook. Reports, over the faces the grammar reads whole:

- constructors declared in crates/ir, used, and never used;
- how many cards each one serves (histogram), per enum;
- the pass rate the grammar would have without the constructors serving fewer than k cards
  (how much of the coverage rests on structure shared by many cards);
- with --list N, every constructor serving at most N cards; with --enum NAME, that enum's variants.

Vocabulary enums (card types, colors, zones, mana symbols, predefined tokens: closed word lists of
the Comprehensive Rules) are counted apart: a word list is not a rule.
"""

from __future__ import annotations

import argparse
import collections
import json
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
VOCAB = {"CardType", "Supertype", "Color", "Zone", "Predefined", "ManaSymbol", "ManaColor", "SubtypeKind"}
BINS = [(1, 1), (2, 3), (4, 10), (11, 30), (31, 100), (101, 1000), (1001, 10**9)]


def declared() -> dict[str, list[str]]:
    """Enum → variants, read from the IR source (brace depth 1 inside `pub enum`)."""
    out: dict[str, list[str]] = {}
    for path in sorted((ROOT / "crates/ir/src").glob("*.rs")):
        src = re.sub(r"//[^\n]*", "", path.read_text())
        for m in re.finditer(r"pub enum (\w+)\s*\{", src):
            depth, i, body = 1, m.end(), []
            while depth and i < len(src):
                c = src[i]
                depth += (c in "{(") - (c in "})")
                if depth >= 1:
                    body.append(c if depth == 1 else " ")
                i += 1
            names = [v for v in re.findall(r"(?:^|,)\s*(?:#\[[^\]]*\]\s*)*([A-Z]\w*)", "".join(body))]
            if names and "$" not in m.group(0):
                out[m.group(1)] = names
    # the lexicon's verbs are declared by its table, one entry a line (`Draw: "draw", …`)
    lexicon = (ROOT / "crates/ir/src/lexicon.rs").read_text()
    table = lexicon[lexicon.index("\nlexicon! {") :]
    out["Verb"] = re.findall(r'^    (\w+): "', table[: table.index("\n}\n")], flags=re.M)
    return out


def label(lo: int, hi: int) -> str:
    return f"{lo}" if lo == hi else (f"{lo}+" if hi >= 10**9 else f"{lo}-{hi}")


def slots(faces: list[dict], prefix: str) -> None:
    """Per slot, the domains real cards put there, by cards: the slot's type, and its outliers.

    `Action::Destroy` is what the action is done to, `Action::Attach.to` a named role, and
    `Action::Draw subject` who does it (`none`: an imperative, done by the controller).
    """
    filled: dict[str, dict[str, set[str]]] = collections.defaultdict(lambda: collections.defaultdict(set))
    example: dict[tuple[str, str], str] = {}
    for f in faces:
        for s in f.get("slots", []):
            sep = "=" if "=" in s else "<"
            name, domain = s.split(sep)
            name += "" if sep == "=" else " subject"
            filled[name][domain].add(f["oracle_id"])
            example.setdefault((name, domain), f["face"])
    for name in sorted(filled):
        if not name.startswith(prefix):
            continue
        by_cards = sorted(filled[name].items(), key=lambda x: -len(x[1]))
        total = sum(len(v) for _, v in by_cards)
        print(f"{name:40} " + "  ".join(f"{d}:{len(v)}" for d, v in by_cards))
        for d, v in by_cards:
            if len(v) <= 3 and len(v) * 50 < total:
                print(f"{'':40}   rare {d}: {example[(name, d)]}")


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--census", default=str(ROOT / "target/census.jsonl"))
    ap.add_argument("--list", type=int, default=0, help="list constructors serving at most N cards")
    ap.add_argument("--enum", default="", help="list this enum's variants with their card counts")
    ap.add_argument("--slots", default="", help="what fills each slot whose name starts with this (e.g. Action::)")
    args = ap.parse_args()

    faces = [json.loads(line) for line in open(args.census)]
    if args.slots:
        slots(faces, args.slots)
        return
    ok = [f for f in faces if f["ok"]]
    cards: dict[str, set[str]] = collections.defaultdict(set)
    for f in ok:
        for c in f["ctors"]:
            cards[c].add(f["oracle_id"])
    serves = {c: len(ids) for c, ids in cards.items()}

    decl = declared()
    is_vocab = lambda c: c.split("::")[0].split(".")[0] in VOCAB
    variants = {f"{e}::{v}" for e, vs in decl.items() for v in vs if e not in VOCAB}
    used = {c for c in serves if not is_vocab(c)}
    used_variants = {c for c in used if "::" in c and "." not in c}
    flags = {c for c in used if "." in c}
    unknown = used_variants - variants
    never = variants - used_variants

    print(f"faces with text {len(faces)}, read whole {len(ok)} ({100 * len(ok) / len(faces):.1f}%), cards {len({f['oracle_id'] for f in ok})}")
    print(f"enums {len([e for e in decl if e not in VOCAB])} (+{len(VOCAB & set(decl))} vocabulary)")
    print(f"variants declared {len(variants)}, used on a whole face {len(used_variants)}, never {len(never)}; boolean flags set {len(flags)}")
    if unknown:
        print(f"  (used but not found in the source scan: {sorted(unknown)[:8]})")

    print("\ncards served per constructor (variants + flags):")
    for lo, hi in BINS:
        cs = [c for c in used if lo <= serves[c] <= hi]
        print(f"  {label(lo, hi):>9} cards: {len(cs):4} constructors")
    print(f"  {'never':>9}      : {len(never):4} variants (declared, no whole face uses them)")

    print("\npass rate without the constructors serving fewer than k cards:")
    for k in (1, 2, 3, 5, 10, 20, 50, 100):
        keep = sum(all(is_vocab(c) or serves[c] >= k for c in f["ctors"]) for f in ok)
        n = sum(1 for c in used if serves[c] >= k)
        print(f"  k={k:<4} {n:4} constructors  {keep:6} faces  {100 * keep / len(faces):5.1f}%")

    print("\nby enum (variants used/declared, serving ≤3 cards, median cards):")
    rows = []
    for e, vs in decl.items():
        if e in VOCAB:
            continue
        counts = sorted(serves.get(f"{e}::{v}", 0) for v in vs)
        few = sum(1 for n in counts if 0 < n <= 3)
        rows.append((len(vs), e, sum(1 for n in counts if n), few, sum(1 for n in counts if n == 0), counts[len(counts) // 2]))
    for n, e, u, few, zero, med in sorted(rows, reverse=True)[:28]:
        print(f"  {e:18} {u:3}/{n:<3}  ≤3 cards: {few:3}  never: {zero:3}  median {med}")

    if args.enum:
        print(f"\n{args.enum}:")
        for v in sorted(decl.get(args.enum, []), key=lambda v: -serves.get(f"{args.enum}::{v}", 0)):
            print(f"  {serves.get(f'{args.enum}::{v}', 0):6}  {v}")
        for c in sorted(flags):
            if c.startswith(args.enum + "::") or c.startswith(args.enum + "."):
                print(f"  {serves[c]:6}  {c} (flag)")
    if args.list:
        print(f"\nconstructors serving at most {args.list} cards:")
        for c in sorted(used, key=lambda c: (serves[c], c)):
            if serves[c] <= args.list:
                print(f"  {serves[c]:3}  {c}")
        print("never used on a whole face:")
        print("  " + ", ".join(sorted(never)))


if __name__ == "__main__":
    main()
