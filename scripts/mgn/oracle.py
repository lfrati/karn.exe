"""Printed card data for the validator: names, face digests and printed characteristics.

Reads the pinned Oracle face file (resources/oracle/cards.jsonl, one row per face). Nothing
here depends on an engine.
"""
from __future__ import annotations

from dataclasses import dataclass
import json
from pathlib import Path
import re

from .canonical import sha256
from .keywords import keyword_row, printed_keywords
from .vocabulary import COLOR_LETTERS, CARD_TYPES, SUPERTYPES, canonical_subtype


def paragraphs(text: str) -> list[str]:
    """The numbered paragraphs of a rules text (spec 3.3): reminder-only lines dropped, reminder
    text removed, modal bullet lines grouped with the line that introduces them."""
    out: list[str] = []
    for line in (text or "").split("\n"):
        stripped = re.sub(r"\s*\([^()]*\)", "", line).strip()
        if not stripped:
            continue
        if stripped.startswith("•") and out:
            out[-1] += "\n" + stripped
            continue
        out.append(stripped)
    return out


def keyword_count(paragraph: str) -> int:
    """How many keyword abilities a paragraph strings together ("Flying, first strike" -> 2); 0 when
    the line is not made of keywords only."""
    return len(keyword_row(paragraph))

ROOT = Path(__file__).resolve().parents[2]
DEFAULT_CARDS = ROOT / "resources/oracle/cards.jsonl"
FACE_FIELDS = ("name", "mana_cost", "type_line", "oracle_text", "colors", "color_indicator", "power", "toughness", "loyalty", "defense", "keywords")


@dataclass
class Face:
    raw: dict

    @property
    def name(self) -> str:
        return self.raw.get("face_name") or self.raw["name"]

    def digest(self) -> str:
        """SHA-256 of the canonical pinned characteristics (spec section 2.3)."""
        chars = {}
        for field in FACE_FIELDS:
            if field in self.raw:
                value = self.raw[field]
                if field == "name":
                    value = self.name
                if field in ("colors", "color_indicator", "keywords"):
                    value = sorted(value)
                chars[field] = value
        return sha256(chars)

    def paragraphs(self) -> list[str]:
        return paragraphs(self.raw.get("oracle_text", ""))

    def printed(self) -> dict:
        """The printed characteristics as MGN spells them: the tier-2 defaults of this face."""
        type_line = self.raw.get("type_line", "")
        left, _, right = type_line.partition("—")
        words = left.split()
        supertypes = sorted(w.lower() for w in words if w.lower() in SUPERTYPES)
        types = sorted(w.lower() for w in words if w.lower() in CARD_TYPES)
        subtypes = sorted(s for s in re.split(r"\s+", right.strip()) if s) if right.strip() else []
        # two-word creature type
        if "Time" in subtypes and "Lord" in subtypes:
            subtypes = sorted([s for s in subtypes if s not in ("Time", "Lord")] + ["Time Lord"])
        out = {
            "colors": sorted(COLOR_LETTERS[c] for c in self.raw.get("colors", []) if c in COLOR_LETTERS),
            "types": types, "supertypes": supertypes, "subtypes": sorted(canonical_subtype(s) for s in subtypes),
            # Read actual printed rows: the search index drops parameters and
            # includes ability words/actions and other faces' abilities (207.2c).
            "keywords": printed_keywords(self.raw.get("oracle_text", "")),
        }
        for key in ("power", "toughness"):
            value = self.raw.get(key)
            if value is not None:
                out[key] = int(value) if re.fullmatch(r"[+-]?\d+", value) else value  # "*" stays a string: no default
        return out


class Oracle:
    def __init__(self, path: Path = DEFAULT_CARDS):
        self.path = Path(path)
        self.faces: dict[str, list[Face]] = {}
        for line in self.path.read_text(encoding="utf-8").splitlines():
            if not line.strip():
                continue
            raw = json.loads(line)
            self.faces.setdefault(raw["oracle_id"], []).append(Face(raw))
        for faces in self.faces.values():
            faces.sort(key=lambda face: face.raw.get("face", 0))

    def __contains__(self, oracle_id: str) -> bool:
        return oracle_id in self.faces

    def name(self, oracle_id: str) -> str:
        return self.faces[oracle_id][0].raw["name"]

    def digests(self, oracle_id: str) -> list[str]:
        return [face.digest() for face in self.faces[oracle_id]]

    def printed(self, oracle_id: str, face: int = 0) -> dict:
        faces = self.faces[oracle_id]
        return faces[min(face, len(faces) - 1)].printed()

    def has_fuse(self, oracle_id: str) -> bool:
        faces = self.faces[oracle_id]
        return (len(faces) == 2 and all(f.raw.get("layout") == "split" for f in faces)
                and any("fuse" in f.printed()["keywords"] for f in faces))

    def printed_fused(self, oracle_id: str) -> dict:
        """CR 702.102b/709.4d: both halves supply one spell's characteristics."""
        if not self.has_fuse(oracle_id):
            raise ValueError("fused needs a two-half split card with fuse")
        parts = [face.printed() for face in self.faces[oracle_id]]
        return {key: sorted({value for part in parts for value in part.get(key, [])})
                for key in ("colors", "types", "supertypes", "subtypes", "keywords")}

    def face_count(self, oracle_id: str) -> int:
        return len(self.faces[oracle_id])

    def paragraphs(self, oracle_id: str, face: int = 0) -> list[str]:
        faces = self.faces[oracle_id]
        if face >= len(faces):
            return []
        return faces[face].paragraphs()

    def by_name(self, name: str) -> str:
        matches = [oid for oid, faces in self.faces.items() if faces[0].raw["name"] == name]
        if len(matches) != 1:
            raise KeyError(name)
        return matches[0]
