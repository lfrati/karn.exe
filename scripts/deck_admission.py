"""Deck-list parsing and executable-admission reporting shared by progress and deck checks.

    uv run scripts/deck_admission.py --decks

The command saves local admission reports in .cache/deck-baseline.{json,md}. Admission
is executable acceptance, not independent evidence of correct card behavior.
"""

import argparse
import datetime
import hashlib
import json
import re
import subprocess
import sys
from pathlib import Path

from rust_tools import fast_binary

ROOT = Path(__file__).resolve().parent.parent


def run(*cmd: str) -> str:
    done = subprocess.run(cmd, cwd=ROOT, capture_output=True, text=True)
    if done.returncode != 0:
        sys.exit(f"{' '.join(cmd)} failed:\n{done.stdout}{done.stderr}")
    return done.stdout


def deck_lists(path: Path) -> dict[str, set[str]]:
    """Distinct names in each part, without losing a card present in both parts."""
    parts = {"main": set(), "sideboard": set()}
    for raw in path.read_text().splitlines():
        line = raw.strip()
        if not line or line.startswith(("#", "//")):
            continue
        match = re.fullmatch(r"(SB:\s*)?(\d+)\s+(?:\[[^]]+\]\s*)?(.+)", line)
        if not match:
            raise ValueError(f"{path}: unrecognized deck row {line!r}")
        if int(match[2]):
            parts["sideboard" if match[1] else "main"].add(match[3])
    return parts


def deck_report(extra_decks=None) -> dict:
    run("cargo", "build", "--profile", "fast", "-p", "game", "--bin", "game-serve")
    decks = {str(p.relative_to(ROOT)): deck_lists(p) for p in sorted((ROOT / "decks").rglob("*.dck"))}
    decks.update(extra_decks or {})
    names = sorted({name for parts in decks.values() for cards in parts.values() for name in cards})
    accepted = {}
    with subprocess.Popen([str(fast_binary("game-serve"))], cwd=ROOT, stdin=subprocess.PIPE,
                          stdout=subprocess.PIPE, text=True) as engine:
        try:
            if json.loads(engine.stdout.readline())["type"] != "ready":
                raise RuntimeError("engine did not become ready")
            for name in names:
                engine.stdin.write(json.dumps({"cmd": "new", "players": [{"hand": [name]}, {}]}) + "\n")
                engine.stdin.flush()
                answer = json.loads(engine.stdout.readline())
                accepted[name] = {"accepted": answer["ok"], "reason": answer.get("error")}
        finally:
            engine.stdin.close()
            engine.wait(timeout=10)
    corpus = ROOT / "resources/oracle/cards.jsonl"
    return {"date": datetime.datetime.now(datetime.timezone.utc).isoformat(),
            "commit": run("git", "rev-parse", "HEAD").strip(),
            "worktree": run("git", "status", "--porcelain", "--ignore-submodules"),
            "engine_sha256": hashlib.sha256((fast_binary("game-serve")).read_bytes()).hexdigest(),
            "corpus_sha256": hashlib.sha256(corpus.read_bytes()).hexdigest(),
            "meaning": "Executable acceptance only, not independently verified card correctness",
            "cards": accepted,
            "decks": {name: {part: {"total": len(cards), "accepted": sum(accepted[c]["accepted"] for c in cards),
                                     "missing": sorted(c for c in cards if not accepted[c]["accepted"])}
                             for part, cards in parts.items()} for name, parts in decks.items()}}


def deck_table(data: dict) -> str:
    rows = ["| Deck | Main accepted | Sideboard accepted |", "|---|---:|---:|"]
    for deck, parts in data["decks"].items():
        main, side = parts["main"], parts["sideboard"]
        rows.append(f"| {deck} | {main['accepted']} of {main['total']} | {side['accepted']} of {side['total']} |")
    return "\n".join(rows)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--decks", action="store_true", help="save main/sideboard admission reports in .cache")
    args = parser.parse_args()
    if not args.decks:
        parser.print_help()
        return
    data = deck_report()
    destination = ROOT / ".cache/deck-baseline.json"
    destination.parent.mkdir(parents=True, exist_ok=True)
    destination.write_text(json.dumps(data, indent=2, ensure_ascii=False) + "\n")
    destination.with_suffix(".md").write_text(deck_table(data) + "\n\n" + data["meaning"] + "\n")
    print(deck_table(data))
    print(data["meaning"])


if __name__ == "__main__":
    main()
