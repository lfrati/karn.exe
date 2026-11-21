"""The pinned Comprehensive Rules edition, named once: `resources/manifest.json`
`sources.rules.edition` (YYYYMMDD, as Wizards dates the text). Everything else that names the
edition (the rules file, its effective date) is derived here; `scripts/fetch_resources.py`
writes the manifest, and `oracle::corpus::rules_path` reads the same field for Rust tests."""
from __future__ import annotations

from datetime import date
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
MANIFEST = ROOT / "resources/manifest.json"


def rules_name(edition: str) -> str:
    """The file Wizards publishes the edition as, under resources/rules/."""
    return f"MagicCompRules-{edition}.txt"


def effective(edition: str) -> date:
    """The date the edition says its rules are effective as of."""
    return date(int(edition[:4]), int(edition[4:6]), int(edition[6:8]))


RULES_EDITION = json.loads(MANIFEST.read_text())["sources"]["rules"]["edition"]
RULES_NAME = rules_name(RULES_EDITION)
RULES = ROOT / "resources/rules" / RULES_NAME
RULES_EFFECTIVE = effective(RULES_EDITION).isoformat()
