"""The closed set of card names (CR 201.1), baked into the `oracle` crate.

    uv run python scripts/card_names.py          # write crates/oracle/data/card_names.json
    uv run python scripts/card_names.py --check  # exit 1 when the baked file is not what the corpus gives

Reads resources/oracle/cards.jsonl (scripts/fetch_resources.py). The names are what a face is called
(`face_name`) and what a single-faced card is called (`name`); a split card's "A // B" is not
a name text refers to. Each name keeps the types of the face it names, the part of its type
line before the dash: a choice of "a nonland card name" reads them (201.4a-f). The file records the sha256 of the corpus it came from, so a stale bake
is visible; `crates/oracle/src/names.rs` also checks the set against the corpus when it is
present.
"""

from __future__ import annotations

import hashlib
import json
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent



def find_cards() -> Path:
    """`resources/oracle/cards.jsonl` under the repository, or under a parent of a worktree."""
    for d in [ROOT, *ROOT.parents]:
        if (d / "resources/oracle/cards.jsonl").exists():
            return d / "resources/oracle/cards.jsonl"
    return ROOT / "resources/oracle/cards.jsonl"


CARDS = find_cards()
OUT = ROOT / "crates/oracle/data/card_names.json"


def card_names(path: Path = CARDS) -> dict:
    names: dict[str, set[str]] = {}
    faces = 0
    for line in path.read_text(encoding="utf-8").splitlines():
        if not line.strip():
            continue
        face = json.loads(line)
        faces += 1
        types = face.get("type_line", "").split(" — ")[0].strip()
        names.setdefault(face["face_name"], set()).add(types)
        if " // " not in face["name"]:
            names.setdefault(face["name"], set()).add(types)
    order = sorted(names)
    return {
        "corpus": {"file": "resources/oracle/cards.jsonl", "sha256": hashlib.sha256(path.read_bytes()).hexdigest(), "faces": faces},
        "names": order,
        # of each name, the types of the faces it names, "|" between faces that differ
        "types": ["|".join(sorted(names[n])) for n in order],
    }


def render(data: dict) -> str:
    return json.dumps(data, indent=0, ensure_ascii=False) + "\n"


def main(argv: list[str]) -> int:
    data = card_names()
    if "--check" in argv:
        if OUT.read_text(encoding="utf-8") != render(data):
            print(f"{OUT} is stale: run scripts/card_names.py", file=sys.stderr)
            return 1
        print("card names are current")
        return 0
    OUT.parent.mkdir(parents=True, exist_ok=True)
    OUT.write_text(render(data), encoding="utf-8")
    print(f"{len(data['names'])} names from {data['corpus']['faces']} faces")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
