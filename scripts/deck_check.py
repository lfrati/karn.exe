"""Parse ordered deck files for explicit recording and provenance."""
from collections import Counter
from pathlib import Path
import re

def read_deck(path: Path) -> tuple[Counter, Counter]:
    main, side = Counter(), Counter()
    for line in path.read_text().splitlines():
        line = line.strip()
        if not line or line.startswith(("//", "#")):
            continue
        m = re.fullmatch(r"(SB:\s*)?(\d+)\s+(?:\[[^]]+\]\s*)?(.+)", line)
        if not m or int(m[2]) <= 0:
            raise ValueError(f"{path}: invalid deck row: {line}")
        (side if m[1] else main)[m[3]] += int(m[2])
    return main, side


def ordered_main(path: Path) -> list[str]:
    """Inline a pipe setup without changing row order, even for repeated names."""
    read_deck(path)  # retain the existing validation
    cards = []
    for raw in path.read_text().splitlines():
        row = re.fullmatch(r"(\d+)\s+(?:\[[^]]+\]\s*)?(.+)", raw.strip())
        if row:
            cards.extend([row[2]] * int(row[1]))
    return cards
