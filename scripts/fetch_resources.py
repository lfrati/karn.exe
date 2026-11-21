"""Refresh committed MTG inputs; generated inventories and reports stay in .cache.

python scripts/fetch_resources.py
python scripts/fetch_resources.py --cards-source .cache/oracle/cards.jsonl \
    --rules-source .cache/MagicCompRules-YYYYMMDD.txt --rules-edition YYYYMMDD

The first form refreshes Scryfall cards/rulings and retains the committed CR pin.
Use --rules-source PATH or --rules-url URL to explicitly replace that pin's text, and
--rules-edition to pin another edition: the manifest's `sources.rules.edition` is the one
place the edition is named (scripts/pinned.py derives the rest).
All downloads are consumed in memory; no raw bulk archive is retained.

python scripts/fetch_resources.py --legal-only .cache/oracle/oracle-cards.jsonl.gz

The third form rewrites only oracle/legal-standard.jsonl (the Standard-legal pool) from a
raw Scryfall bulk file, keeping the other inputs, and records it in the manifest.
"""
from __future__ import annotations

import argparse
import collections
from datetime import datetime, timezone
import gzip
import hashlib
import json
import urllib.request
from pathlib import Path

import pinned

ROOT = Path(__file__).resolve().parent.parent
HEADERS = {"User-Agent": "karn.exe/0.1 (research)", "Accept": "application/json"}


def encoded(value) -> bytes:
    return json.dumps(value, ensure_ascii=False, separators=(",", ":")).encode("utf-8")


def sha(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def get(url: str) -> bytes:
    request = urllib.request.Request(url, headers=HEADERS)
    with urllib.request.urlopen(request, timeout=60) as response:
        return response.read()


def records(data: bytes) -> list[dict]:
    if data.startswith(b"\x1f\x8b"):
        data = gzip.decompress(data)
    if data.lstrip().startswith(b"["):
        return json.loads(data)
    return [json.loads(line) for line in data.splitlines() if line.strip()]


LEGAL_FORMAT = "standard"
LEGAL_FILE = f"oracle/legal-{LEGAL_FORMAT}.jsonl"


def legal_pool(cards: list[dict]) -> list[dict]:
    """The cards Scryfall lists as legal in the format: one row per card (oracle id), by name.

    Taken from the raw bulk rows, which carry `legalities`; the committed faces do not. A card
    is legal, not merely not banned: `banned`, `restricted` and `not_legal` are all excluded.
    """
    rows = {}
    for card in cards:
        if card.get("legalities", {}).get(LEGAL_FORMAT) != "legal" or meld_result(card):
            continue
        rows.setdefault(card["oracle_id"], {"oracle_id": card["oracle_id"], "name": card["name"]})
    return sorted(rows.values(), key=lambda row: (row["name"], row["oracle_id"]))


LEGAL_POLICY = {"format": "standard", "platform": "tabletop",
                "legality": "legal", "exclude": ["meld_result"],
                "identity": "oracle_id", "order": ["name", "oracle_id"]}


def timestamp(value: str) -> str:
    parsed = datetime.fromisoformat(value.replace("Z", "+00:00"))
    if parsed.tzinfo is None:
        raise ValueError("source timestamps must include a timezone")
    return value


def bulk_provenance(raw: bytes, metadata: dict) -> dict:
    """Bind supplied retrieval metadata to the exact imported bytes; never infer a date."""
    if metadata["sha256"] != sha(raw):
        raise ValueError("bulk metadata SHA-256 does not match imported bytes")
    if metadata["type"] != "oracle_cards":
        raise ValueError("legality requires an oracle_cards bulk")
    return {"url": metadata.get("jsonl_download_uri") or metadata["download_uri"],
            "bulk_id": metadata["id"], "metadata_url": metadata["uri"],
            "updated_at": timestamp(metadata["updated_at"]),
            "retrieved_at": timestamp(metadata["retrieved_at"]), "sha256": sha(raw)}


def legal_provenance(source: dict, cards: list[dict], scope: dict | None) -> dict:
    legal_cards = [c for c in cards if c.get("legalities", {}).get("standard") == "legal" and not meld_result(c)]
    if any(c.get("digital") or ("games" in c and "paper" not in c["games"]) for c in legal_cards):
        raise ValueError("Scryfall legal pool includes cards unavailable for tabletop")
    result = {**source, "filtering_policy": LEGAL_POLICY}
    if scope is not None:
        timestamp(scope["effective_at"])
        if scope["platform"] != "tabletop" or not scope["sources"] or not scope["legal_set_codes"]:
            raise ValueError("dated Standard scope requires tabletop sources and legal sets")
        if "updated_at" not in source or "retrieved_at" not in source:
            raise ValueError("dated Standard scope requires dated bulk provenance")
        # Compare all banned names, including exclusions, rather than merely checking that
        # banned cards were filtered out. Wizards' reviewed spelling correction is explicit.
        banned = {c["name"] for c in cards if c.get("legalities", {}).get("standard") == "banned"}
        if banned != set(scope["banned_cards"]):
            raise ValueError("Scryfall Standard bans disagree with reviewed tabletop scope")
        result["scope"] = scope
    return result


def meld_result(card: dict) -> bool:
    """A melded face is no card a deck can contain (712.4b): Scryfall lists it as a card of its own.

    Matched by name: the oracle-card bulk's row for the result may carry another printing's id
    than the `meld_result` part names.
    """
    return any(part.get("component") == "meld_result" and part.get("name") == card.get("name")
               for part in card.get("all_parts") or [])


def compact_face(face: dict) -> dict:
    # Only drop values with defined reader defaults. In particular, preserve an
    # explicit empty color indicator: the engine must reject that invalid input.
    defaults = {"face": 0, "layout": "normal", "mana_cost": "",
                "oracle_text": "", "keywords": [], "digital": False}
    return {key: value for key, value in face.items()
            if value is not None and (key not in defaults or value != defaults[key])}


def deduplicate_rulings(rows: list[dict], oracle_ids: set[str]):
    unique, links = {}, collections.defaultdict(set)
    for row in rows:
        if row["oracle_id"] not in oracle_ids:
            continue
        entry = {"date": row["published_at"], "source": row["source"], "text": row["comment"]}
        key = sha(encoded(entry))[:16]
        if key in unique and unique[key] != entry:
            raise ValueError("ruling ID collision")
        unique[key] = entry
        links[row["oracle_id"]].add(key)
    return ([{"id": key, **unique[key]} for key in sorted(unique)],
            [[card, sorted(ids)] for card, ids in sorted(links.items())])


def jsonl(rows) -> bytes:
    return b"".join(encoded(row) + b"\n" for row in rows)


# layouts that are not cards a deck can contain
NON_CARD_LAYOUTS = {"token", "double_faced_token", "emblem", "art_series", "vanguard", "planar", "scheme", "reversible_card"}
# paper formats: being legal (or banned/restricted) in any one makes a card real Magic
PAPER_FORMATS = ("vintage", "legacy", "commander", "modern", "pioneer", "standard", "pauper", "premodern", "oldschool", "penny", "predh", "duel")


def extract_faces(cards):
    skipped = collections.Counter()
    faces_out = []
    for card in cards:
        if card["layout"] in NON_CARD_LAYOUTS:
            skipped[f"layout {card['layout']}"] += 1
            continue
        if card.get("set_type") == "funny":
            skipped["funny set (Un-sets, playtest cards...)"] += 1
            continue
        if not any(card["legalities"].get(f) in ("legal", "restricted", "banned") for f in PAPER_FORMATS):
            skipped["not legal in any paper format (digital-only, oversized...)"] += 1
            continue
        faces = card.get("card_faces") or [card]
        for i, face in enumerate(faces):
            faces_out.append({
                "oracle_id": card["oracle_id"],
                "name": card["name"],
                "face": i,
                "face_name": face.get("name", card["name"]),
                "layout": card["layout"],
                "mana_cost": face.get("mana_cost", card.get("mana_cost", "")),
                "type_line": face.get("type_line", card.get("type_line", "")),
                "oracle_text": face.get("oracle_text", ""),
                "power": face.get("power"),
                "toughness": face.get("toughness"),
                "loyalty": face.get("loyalty"),
                "defense": face.get("defense"),
                "colors": face.get("colors", card.get("colors")),
                "color_indicator": face.get("color_indicator"),
                "keywords": card.get("keywords", []),
                "digital": card.get("digital", False),
            })
    return faces_out, skipped


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, default=ROOT / "resources")
    parser.add_argument("--cards-source", type=Path, help="existing filtered face JSONL; preserve its corpus")
    parser.add_argument("--rulings-source", type=Path, help="local raw Scryfall rulings JSON/JSONL, optionally gzip")
    rules = parser.add_mutually_exclusive_group()
    rules.add_argument("--rules-source", type=Path, help="local pinned CR text")
    rules.add_argument("--rules-url", help="explicit CR text URL (never silently follow latest)")
    parser.add_argument("--rules-edition", metavar="YYYYMMDD",
                        help="the edition --rules-source/--rules-url is (default: the pinned edition)")
    parser.add_argument("--legal-only", type=Path, metavar="BULK",
                        help=f"rewrite only {LEGAL_FILE} from this raw Scryfall card bulk (JSON/JSONL, optionally gzip)")
    parser.add_argument("--legal-source-metadata", type=Path,
                        help="Scryfall bulk metadata JSON with retrieved_at and exact raw sha256 for --legal-only")
    parser.add_argument("--standard-scope", type=Path,
                        help="reviewed tabletop scope JSON with effective_at, sources, legal sets and banned_cards")
    args = parser.parse_args()
    scope = json.loads(args.standard_scope.read_text()) if args.standard_scope else None
    if args.legal_source_metadata and not args.legal_only:
        parser.error("--legal-source-metadata requires --legal-only")
    if args.rules_edition and not (args.rules_source or args.rules_url):
        parser.error("--rules-edition names the edition of --rules-source or --rules-url")
    manifest_path = args.output / "manifest.json"
    previous = json.loads(manifest_path.read_text()) if manifest_path.exists() else {}
    if args.legal_only:
        write_legal_pool(args.output, manifest_path, previous, args.legal_only, args.legal_source_metadata, scope)
        return
    sources = {}
    bulk = None

    def source(kind: str, local: Path | None):
        nonlocal bulk
        if local:
            raw = local.read_bytes()
            sources[kind] = {"imported_from": str(local), "sha256": sha(raw)}
        else:
            if bulk is None:
                bulk = json.loads(get("https://api.scryfall.com/bulk-data"))["data"]
            entry = next(item for item in bulk if item["type"] == kind)
            url = entry.get("jsonl_download_uri") or entry["download_uri"]
            raw = get(url)
            sources[kind] = {"url": url, "bulk_id": entry["id"], "metadata_url": entry["uri"],
                             "updated_at": entry["updated_at"],
                             "retrieved_at": datetime.now(timezone.utc).isoformat(), "sha256": sha(raw)}
        return records(raw)

    cards = source("oracle_cards", args.cards_source)
    legal = None if args.cards_source else legal_pool(cards)
    legal_source = None if legal is None else legal_provenance(sources["oracle_cards"], cards, scope)
    if scope and legal is None:
        raise ValueError("filtered faces cannot establish a dated Standard scope")
    if not args.cards_source:
        cards, skipped = extract_faces(cards)
        for reason, count in skipped.most_common():
            print(f"skipped {count}: {reason}")
    identities = [(row["oracle_id"], row.get("face", 0)) for row in cards]
    if not cards or len(set(identities)) != len(cards):
        raise ValueError("empty corpus or duplicate face identity")
    rulings, links = deduplicate_rulings(source("rulings", args.rulings_source), {key[0] for key in identities})
    edition = (args.rules_edition or previous.get("sources", {}).get("rules", {}).get("edition")
               or pinned.RULES_EDITION)
    rules_name = pinned.rules_name(edition)
    rules_path = args.output / "rules" / rules_name
    if args.rules_url:
        rule_bytes = get(args.rules_url)
        sources["rules"] = {"url": args.rules_url, "sha256": sha(rule_bytes)}
    elif args.rules_source:
        rule_bytes = args.rules_source.read_bytes()
        sources["rules"] = {"imported_from": str(args.rules_source), "sha256": sha(rule_bytes)}
    else:
        rule_bytes = rules_path.read_bytes()
        sources["rules"] = previous.get("sources", {}).get("rules", {"sha256": sha(rule_bytes)})
    sources["rules"] = {"edition": edition, **sources["rules"]}
    # Reject HTML/error responses or a new edition accidentally stored under the old pin.
    rule_text = rule_bytes.decode("utf-8-sig")
    day = pinned.effective(edition)
    if ("Magic: The Gathering" not in rule_text or "202.3b" not in rule_text
            or f"effective as of {day:%B} {day.day}, {day.year}" not in rule_text):
        raise ValueError(f"expected the pinned {rules_name} Comprehensive Rules text")
    files = {f"rules/{rules_name}": rule_bytes,
             "oracle/cards.jsonl": jsonl([compact_face(row) for row in cards]),
             "oracle/rulings.jsonl": jsonl(rulings), "oracle/card_rulings.jsonl": jsonl(links)}
    counts = {"cards": len({key[0] for key in identities}), "faces": len(cards),
              "rulings": len(rulings), "cards_with_rulings": len(links),
              "ruling_links": sum(len(ids) for _, ids in links)}
    if legal is not None:
        check_legal_pool(legal, {key[0] for key in identities})
        files[LEGAL_FILE] = jsonl(legal)
        counts[f"legal_{LEGAL_FORMAT}"] = len(legal)
        sources[f"legal_{LEGAL_FORMAT}"] = legal_source
    elif (kept := (args.output / LEGAL_FILE)).exists():
        # a corpus imported from filtered faces carries no legalities: the pool stays as pinned
        files[LEGAL_FILE] = kept.read_bytes()
        counts[f"legal_{LEGAL_FORMAT}"] = previous.get("counts", {}).get(f"legal_{LEGAL_FORMAT}")
        if f"legal_{LEGAL_FORMAT}" in previous.get("sources", {}):
            sources[f"legal_{LEGAL_FORMAT}"] = previous["sources"][f"legal_{LEGAL_FORMAT}"]
    manifest = {"schema": 1, "sources": sources, "counts": counts,
                "files": {name: {"bytes": len(data), "sha256": sha(data)} for name, data in files.items()}}
    # Prepare/validate everything before replacing any committed input.
    for name, data in files.items():
        path = args.output / name
        path.parent.mkdir(parents=True, exist_ok=True)
        temp = path.with_suffix(path.suffix + ".tmp")
        temp.write_bytes(data)
        temp.replace(path)
        print(f"{name}: {len(data):,} bytes")
    manifest_path.write_text(json.dumps(manifest, indent=2) + "\n")
    print(json.dumps(manifest["counts"]))


def check_legal_pool(legal: list[dict], oracle_ids: set[str]) -> None:
    """Every legal card is a card of the corpus: the pool measures a subset of it."""
    missing = [row["name"] for row in legal if row["oracle_id"] not in oracle_ids]
    if missing:
        raise ValueError(f"{len(missing)} {LEGAL_FORMAT}-legal cards are not in the corpus: {', '.join(missing[:5])}")
    if not legal:
        raise ValueError(f"no {LEGAL_FORMAT}-legal cards: not a Scryfall card bulk?")


def write_legal_pool(output: Path, manifest_path: Path, previous: dict, bulk: Path, metadata_path: Path | None = None,
                     scope: dict | None = None) -> None:
    raw = bulk.read_bytes()
    cards = records(raw)
    legal = legal_pool(cards)
    source = {"imported_from": str(bulk), "sha256": sha(raw)}
    if metadata_path:
        source.update(bulk_provenance(raw, json.loads(metadata_path.read_text())))
    source = legal_provenance(source, cards, scope)
    corpus = output / "oracle/cards.jsonl"
    check_legal_pool(legal, {json.loads(line)["oracle_id"] for line in corpus.read_text().splitlines() if line.strip()})
    data = jsonl(legal)
    path = output / LEGAL_FILE
    temp = path.with_suffix(path.suffix + ".tmp")
    temp.write_bytes(data)
    temp.replace(path)
    manifest = dict(previous)
    manifest["sources"] = {**manifest.get("sources", {}),
                           f"legal_{LEGAL_FORMAT}": source}
    manifest["counts"] = {**manifest.get("counts", {}), f"legal_{LEGAL_FORMAT}": len(legal)}
    manifest["files"] = {**manifest.get("files", {}), LEGAL_FILE: {"bytes": len(data), "sha256": sha(data)}}
    manifest_path.write_text(json.dumps(manifest, indent=2) + "\n")
    print(f"{LEGAL_FILE}: {len(legal):,} cards, {len(data):,} bytes")


if __name__ == "__main__":
    main()
