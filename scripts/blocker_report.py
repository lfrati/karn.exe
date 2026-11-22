"""Every known blocker of each refused card, ranked by the published decks it keeps from running.

    uv run scripts/blocker_report.py --out DIR [--engine PATH | --snapshot PATH] [--compare BASELINE]

Runs `game-gate --admission-snapshot DIR/admission.jsonl --all-blockers` (the fast build by
default), or reads such a snapshot, and projects it two ways: onto the main decks of the roster
`resources/decks/selected-decks.jsonl`, which orders the work ([roadmap](../docs/roadmap.md)),
and onto the Standard-legal cards of `resources/oracle/legal-standard.jsonl` by Oracle identity.
DIR is durable evidence, not Git:

  admission.jsonl          the full snapshot (when the engine is run)
  deck-labels.jsonl        one line per blocker label: the roster decks it keeps from running
                           and those it alone does, most completed decks first (see `deck_report`)
  standard-blockers.jsonl  one line per refused face of a legal card: its blockers in the order
                           admission meets them, the first its admission's refusal
  standard-labels.jsonl    one line per blocker label (the reason; of a text row the grammar
                           declines, with the token it stopped at), most cards first
  summary.json             totals and provenance
  admission-compare.json   with --compare: the admitted set against a baseline snapshot

Counts per label, over the refused legal cards: `faces` and `cards` with the label among their
blockers, `first_cards` whose first refused face has it first, `sole_cards` whose refused faces
have no other known blocker, and `sole_fully_analyzed_cards` of those, the ones nothing was left
unanalyzed of. Admission stops reading a row where the grammar stops, a lowered ability at its
first refusal, and the engine checks of a face the grammar does not read at all: a sole blocker
with something unanalyzed is no promise that fixing it admits the card. These are diagnostics,
not verification credit.
"""
from __future__ import annotations

import argparse
import collections
import hashlib
import json
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT / "scripts"))
import deck_archive  # noqa: E402
from progress_dashboard import MELD_RESULTS  # noqa: E402
from rust_tools import fast_binary  # noqa: E402

LEGAL = ROOT / "resources/oracle/legal-standard.jsonl"
ROSTER = ROOT / "resources/decks/selected-decks.jsonl"


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def read_jsonl(path: Path) -> list[dict]:
    return [json.loads(line) for line in path.read_text().splitlines() if line.strip()]


def standard_rows(snapshot: list[dict], legal: list[dict]) -> dict[str, list[dict]]:
    """The snapshot's faces of each legal card, by Oracle identity; a legal card the snapshot
    lacks is an error, never dropped from the pool."""
    faces = collections.defaultdict(list)
    for row in snapshot:
        faces[row["oracle_id"]].append(row)
    missing = [card["name"] for card in legal if card["oracle_id"] not in faces]
    if missing:
        raise ValueError(f"{len(missing)} legal cards are not in the snapshot: {', '.join(missing[:5])}")
    if any("blockers" not in row for row in snapshot):
        raise ValueError("the snapshot has no blockers: write it with game-gate --all-blockers")
    return {card["oracle_id"]: sorted(faces[card["oracle_id"]], key=lambda r: r["face"]) for card in legal}


def report(snapshot: list[dict], legal: list[dict]) -> tuple[list[dict], list[dict], dict]:
    """The refused faces of the legal cards, the counts per label, and the totals."""
    names = {card["oracle_id"]: card["name"] for card in legal}
    faces, labels = [], collections.defaultdict(lambda: {
        "stage": None, "faces": 0, "cards": set(), "first_cards": set(), "sole_cards": set(), "sole_fully_analyzed_cards": set()})
    refused_cards = {"parse": 0, "engine": 0}
    for oracle_id, rows in standard_rows(snapshot, legal).items():
        refused = [row for row in rows if row["stage"] != "admitted"]
        if not refused:
            continue
        refused_cards[refused[0]["stage"]] += 1
        card_labels, fully = set(), True
        for row in refused:
            if not row["blockers"]:
                raise ValueError(f"refused face without a blocker: {row['name']} ({oracle_id}, face {row['face']})")
            if row["blockers"][0]["reason"] != row["reason"]:
                raise ValueError(f"first blocker is not the refusal: {row['name']} ({oracle_id}, face {row['face']})")
            face_labels = list(dict.fromkeys(b["label"] for b in row["blockers"]))
            fully &= row["fully_analyzed"]
            card_labels.update(face_labels)
            for label in face_labels:
                entry = labels[label]
                entry["faces"] += 1
                entry["cards"].add(oracle_id)
                entry["stage"] = next(b["stage"] for b in row["blockers"] if b["label"] == label)
            faces.append({"oracle_id": oracle_id, "card": names[oracle_id], "face": row["face"], "name": row["name"],
                          "layout": row["layout"], "stage": row["stage"], "reason": row["reason"],
                          "labels": face_labels, "blockers": row["blockers"], "unanalyzed": row["unanalyzed"],
                          "fully_analyzed": row["fully_analyzed"]})
        labels[refused[0]["blockers"][0]["label"]]["first_cards"].add(oracle_id)
        if len(card_labels) == 1:
            (label,) = card_labels
            labels[label]["sole_cards"].add(oracle_id)
            if fully:
                labels[label]["sole_fully_analyzed_cards"].add(oracle_id)
    counted = [{"label": label, "stage": e["stage"], "cards": len(e["cards"]), "faces": e["faces"],
                "first_cards": len(e["first_cards"]), "sole_cards": len(e["sole_cards"]),
                "sole_fully_analyzed_cards": len(e["sole_fully_analyzed_cards"]),
                "examples": sorted(names[i] for i in e["cards"])[:5]} for label, e in labels.items()]
    counted.sort(key=lambda e: (-e["cards"], -e["faces"], e["label"]))
    per_face = collections.Counter(len(face["labels"]) for face in faces)
    totals = {
        "legal_cards": len(legal),
        "admitted_cards": len(legal) - sum(refused_cards.values()),
        "refused_cards": refused_cards,
        "refused_faces": len(faces),
        "fully_analyzed_refused_faces": sum(face["fully_analyzed"] for face in faces),
        "distinct_labels": len(counted),
        "refused_faces_by_label_count": {str(n): per_face[n] for n in sorted(per_face)},
    }
    return faces, counted, totals


def card_blockers(snapshot: list[dict], corpus: list[dict]) -> tuple[dict[str, str], dict[str, dict]]:
    """Whole physical cards as a deck names them: each corpus card's name ("Wear // Tear", not a
    face's) with its Oracle identity, and of each card the engine does not run, the blocker
    labels of all its faces, the stage it stops at and whether nothing of it was left unanalyzed.
    A meld component also needs the card it melds into, as `progress_dashboard.whole_cards`
    counts it."""
    faces = collections.defaultdict(list)
    for row in snapshot:
        faces[row["oracle_id"]].append(row)
    by_name = {card["name"]: card["oracle_id"] for card in corpus}
    absent = sorted(name for name, oracle_id in by_name.items() if oracle_id not in faces)
    if absent:
        raise ValueError(f"{len(absent)} corpus cards are not in the snapshot: {', '.join(absent[:5])}")
    needed = {oracle_id: list(rows) for oracle_id, rows in faces.items()}
    for result, components in MELD_RESULTS.items():
        for component in components:
            if result in by_name and component in by_name:
                needed[by_name[component]] += faces[by_name[result]]
    refused = {}
    for oracle_id, rows in needed.items():
        stopped = [row for row in rows if row["stage"] != "admitted"]
        if stopped:
            refused[oracle_id] = {
                "labels": {blocker["label"]: blocker["stage"] for row in stopped for blocker in row["blockers"]},
                "stage": "parse" if any(row["stage"] == "parse" for row in stopped) else "engine",
                "fully_analyzed": all(row["fully_analyzed"] for row in stopped)}
    return by_name, refused


def deck_report(snapshot: list[dict], roster: list[dict], corpus: list[dict]) -> tuple[list[dict], dict]:
    """The roster decks each blocker label keeps from running, and the totals.

    A deck runs when the engine runs every card of its main deck, its commanders and companion
    with it; a deck naming a card the corpus lacks never does. Per label: `cards`, the distinct
    main-deck cards with the label among their blockers; `decks` naming one of them; `sole_decks`,
    whose cards the engine does not run have no other known blocker, so that removing the label
    alone would run the deck; and `sole_fully_analyzed_decks`, those of them of whose cards
    nothing was left unanalyzed. `corpus_cards` and `sole_corpus_cards` count
    whole physical cards over the complete corpus, including cards in no roster deck.
    Most sole decks first, then the number of decks touched.
    """
    by_name, refused = card_blockers(snapshot, corpus)
    names = {oracle_id: name for name, oracle_id in by_name.items()}
    corpus_counts = collections.defaultdict(lambda: [0, 0])
    for oracle_id, card in refused.items():
        if names[oracle_id] in MELD_RESULTS:
            continue
        for label in card["labels"]:
            corpus_counts[label][0] += 1
            corpus_counts[label][1] += len(card["labels"]) == 1
    labels = collections.defaultdict(lambda: {
        "stage": None, "cards": set(), "decks": 0, "sole_decks": 0, "sole_fully_analyzed_decks": 0})
    totals = {"decks": len(roster), "unresolved_decks": 0, "main_decks_read": 0, "main_decks_that_run": 0,
              "decks_one_label_away": 0}
    missing_cards = {}
    for deck in roster:
        cards = deck_archive.deck_cards({**deck, "sideboard": {}})
        resolved = cards <= by_name.keys()
        missing = {by_name[name]: refused[by_name[name]] for name in cards
                   if name in by_name and by_name[name] in refused}
        missing_cards.update(missing)
        totals["unresolved_decks"] += not resolved
        totals["main_decks_read"] += resolved and all(card["stage"] != "parse" for card in missing.values())
        totals["main_decks_that_run"] += resolved and not missing
        deck_labels = {label: stage for card in missing.values() for label, stage in card["labels"].items()}
        for label, stage in deck_labels.items():
            entry = labels[label]
            entry["stage"] = stage
            entry["decks"] += 1
            entry["cards"].update(oracle_id for oracle_id, card in missing.items() if label in card["labels"])
        if resolved and len(deck_labels) == 1:
            (label,) = deck_labels
            totals["decks_one_label_away"] += 1
            labels[label]["sole_decks"] += 1
            labels[label]["sole_fully_analyzed_decks"] += all(card["fully_analyzed"] for card in missing.values())
    counted = [{"label": label, "stage": e["stage"], "cards": len(e["cards"]), "decks": e["decks"],
                "sole_decks": e["sole_decks"], "sole_fully_analyzed_decks": e["sole_fully_analyzed_decks"],
                "corpus_cards": corpus_counts[label][0], "sole_corpus_cards": corpus_counts[label][1],
                "examples": sorted(names[i] for i in e["cards"])[:5]} for label, e in labels.items()]
    counted.sort(key=lambda e: (-e["sole_decks"], -e["decks"], -e["sole_corpus_cards"], e["label"]))
    stages = collections.Counter(card["stage"] for card in missing_cards.values())
    totals.update(missing_cards=len(missing_cards), missing_cards_by_stage={"parse": stages["parse"], "engine": stages["engine"]},
                  distinct_labels=len(counted))
    return counted, totals


def compare(baseline: list[dict], current: list[dict]) -> dict:
    """The admitted faces, and each face's stage and first refusal, of two snapshots of one
    corpus."""
    key = lambda row: (row["oracle_id"], row["face"])
    before, after = {key(r): r for r in baseline}, {key(r): r for r in current}
    admitted = lambda rows: {k for k, r in rows.items() if r["stage"] == "admitted"}
    changed = sorted(k for k in before.keys() & after.keys()
                     if (before[k]["stage"], before[k]["reason"]) != (after[k]["stage"], after[k]["reason"]))
    return {
        "faces": [len(before), len(after)],
        "only_before": sorted(before.keys() - after.keys()), "only_after": sorted(after.keys() - before.keys()),
        "admitted": [len(admitted(before)), len(admitted(after))],
        "admitted_identical": admitted(before) == admitted(after) and before.keys() == after.keys(),
        "newly_admitted": sorted(admitted(after) - admitted(before)),
        "no_longer_admitted": sorted(admitted(before) - admitted(after)),
        "stage_or_reason_changed": [{"face": list(k), "before": [before[k]["stage"], before[k]["reason"]],
                                     "after": [after[k]["stage"], after[k]["reason"]]} for k in changed],
    }


def git_head() -> dict:
    run = lambda *args: subprocess.run(["git", *args], cwd=ROOT, capture_output=True, text=True).stdout.strip()
    return {"commit": run("rev-parse", "HEAD"), "dirty": bool(run("status", "--porcelain", "--untracked-files=no"))}


def main(argv=None) -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--out", type=Path, required=True, help="durable evidence directory")
    source = parser.add_mutually_exclusive_group()
    source.add_argument("--engine", type=Path, help="game-gate binary (default: the fast build)")
    source.add_argument("--snapshot", type=Path, help="an existing game-gate --all-blockers snapshot")
    parser.add_argument("--compare", type=Path, help="baseline admission snapshot: is the admitted set identical?")
    parser.add_argument("--roster", type=Path, default=ROSTER, help="deck corpus (default: tournament roster)")
    args = parser.parse_args(argv)
    roster = args.roster.resolve()
    args.out.mkdir(parents=True, exist_ok=True)
    provenance = {"repository": git_head(), "legal_pool": {"path": str(LEGAL.relative_to(ROOT)), "sha256": sha256(LEGAL)},
                  "roster": {"path": str(roster), "sha256": sha256(roster)},
                  "corpus_sha256": sha256(ROOT / "resources/oracle/cards.jsonl")}
    if args.snapshot:
        snapshot_path = args.snapshot
    else:
        engine = args.engine or fast_binary("game-gate")
        snapshot_path = args.out / "admission.jsonl"
        command = [str(engine), "--limit", "0", "--admission-snapshot", str(snapshot_path), "--all-blockers"]
        gate = subprocess.run(command, cwd=ROOT, capture_output=True, text=True)
        (args.out / "game-gate.txt").write_text(gate.stdout + gate.stderr)
        if gate.returncode != 0:
            print(f"game-gate failed ({gate.returncode}); see {args.out / 'game-gate.txt'}", file=sys.stderr)
            return 1
        provenance["engine"] = {"path": str(engine), "sha256": sha256(Path(engine)), "command": command}
    provenance["snapshot"] = {"path": str(snapshot_path), "sha256": sha256(snapshot_path)}
    snapshot = read_jsonl(snapshot_path)
    faces, labels, totals = report(snapshot, read_jsonl(LEGAL))
    with (args.out / "standard-blockers.jsonl").open("w") as out:
        out.writelines(json.dumps(face, sort_keys=True) + "\n" for face in faces)
    with (args.out / "standard-labels.jsonl").open("w") as out:
        out.writelines(json.dumps(label, sort_keys=True) + "\n" for label in labels)
    deck_labels, deck_totals = deck_report(snapshot, read_jsonl(roster), read_jsonl(ROOT / "resources/oracle/cards.jsonl"))
    with (args.out / "deck-labels.jsonl").open("w") as out:
        out.writelines(json.dumps(label, sort_keys=True) + "\n" for label in deck_labels)
    summary = {"decks": deck_totals, "totals": totals, "provenance": provenance}
    status = 0
    if args.compare:
        comparison = compare(read_jsonl(args.compare), snapshot)
        comparison["baseline"] = {"path": str(args.compare), "sha256": sha256(args.compare)}
        (args.out / "admission-compare.json").write_text(json.dumps(comparison, indent=1) + "\n")
        summary["admitted_identical_to_baseline"] = comparison["admitted_identical"]
        status = 0 if comparison["admitted_identical"] else 1
    (args.out / "summary.json").write_text(json.dumps(summary, indent=1) + "\n")
    print(json.dumps(deck_totals))
    for label in deck_labels[:15]:
        print(f"{label['sole_decks']:4} decks completed  {label['decks']:5} touched  {label['sole_corpus_cards']:5} corpus cards alone  {label['label']}")
    print(json.dumps(totals))
    for label in labels[:15]:
        print(f"{label['cards']:5} cards  {label['sole_cards']:4} sole  {label['first_cards']:4} first  {label['label']}")
    return status


if __name__ == "__main__":
    sys.exit(main())
