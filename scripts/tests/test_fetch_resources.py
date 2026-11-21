"""Offline checks for lossless resource compaction and ruling associations."""
import unittest
import contextlib
import io
import json
import tempfile
from pathlib import Path
from unittest.mock import patch

from scripts import fetch_resources as resources
import pinned


class ResourcesTests(unittest.TestCase):
    def test_compaction_preserves_nondefault_faces_costs_and_indicators(self):
        front = {"name": "A // B", "face_name": "A", "oracle_id": "id", "face": 0,
                 "layout": "transform", "mana_cost": "{0}", "oracle_text": "",
                 "power": "0", "colors": [], "color_indicator": [], "keywords": [],
                 "toughness": None, "digital": False}
        compact = resources.compact_face(front)
        self.assertNotIn("face", compact)
        self.assertNotIn("toughness", compact)
        self.assertNotIn("oracle_text", compact)
        self.assertEqual(compact["layout"], "transform")
        self.assertEqual(compact["mana_cost"], "{0}")
        self.assertEqual(compact["power"], "0")
        self.assertEqual(compact["color_indicator"], [])
        back = resources.compact_face(front | {"face": 1, "mana_cost": "", "color_indicator": ["R"]})
        self.assertEqual(back["face"], 1)
        self.assertNotIn("mana_cost", back)
        self.assertEqual(back["color_indicator"], ["R"])

    def test_deduplication_keeps_date_source_and_card_links(self):
        row = {"oracle_id": "a", "published_at": "2026-01-01", "source": "wotc", "comment": "Shared rule."}
        rows = [row, row, row | {"oracle_id": "b"}, row | {"oracle_id": "excluded"},
                row | {"published_at": "2026-02-01"}, row | {"source": "scryfall"}]
        rulings, links = resources.deduplicate_rulings(rows, {"a", "b"})
        self.assertEqual(len(rulings), 3)
        by_id = {r["id"]: r for r in rulings}
        mapped = dict(links)
        self.assertEqual(set(mapped), {"a", "b"})
        self.assertEqual(len(mapped["a"]), 3)
        self.assertEqual(len(mapped["b"]), 1)
        self.assertEqual(by_id[mapped["b"][0]]["date"], "2026-01-01")
        self.assertEqual(by_id[mapped["b"][0]]["source"], "wotc")
        # Input ordering and unrelated additions must not renumber existing IDs.
        self.assertEqual(resources.deduplicate_rulings(list(reversed(rows)), {"a", "b"}), (rulings, links))
        more, _ = resources.deduplicate_rulings(rows + [row | {"comment": "Another rule."}], {"a", "b"})
        self.assertTrue(set(by_id) < {r["id"] for r in more})

    def test_face_extraction_keeps_scope_and_each_faces_characteristics(self):
        card = {"oracle_id": "a", "name": "Front // Back", "layout": "transform",
                "set_type": "expansion", "legalities": {"legacy": "legal"}, "colors": ["G"],
                "card_faces": [{"name": "Front", "type_line": "Creature", "mana_cost": "{G}", "power": "1"},
                               {"name": "Back", "type_line": "Creature", "color_indicator": ["R"], "power": "2"}]}
        rows = [card, card | {"set_type": "funny"}, card | {"layout": "token"},
                card | {"legalities": {"alchemy": "legal"}}]
        faces, skipped = resources.extract_faces(rows)
        self.assertEqual([f["face_name"] for f in faces], ["Front", "Back"])
        self.assertEqual([f["face"] for f in faces], [0, 1])
        self.assertEqual(faces[1]["color_indicator"], ["R"])
        self.assertEqual(sum(skipped.values()), 3)

    def test_offline_import_and_rejected_rules_leave_existing_snapshot_intact(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            cards, rulings, rules = root / "faces.jsonl", root / "rulings.jsonl", root / "rules.txt"
            cards.write_bytes(resources.jsonl([{"oracle_id": "a", "name": "A", "face_name": "A",
                                                "type_line": "Land", "face": 0, "layout": "normal"}]))
            rulings.write_bytes(resources.jsonl([{"oracle_id": "a", "published_at": "2026-01-01",
                                                  "source": "wotc", "comment": "A ruling."}]))
            rules.write_bytes(pinned.RULES.read_bytes())
            output = root / "resources"
            argv = ["fetch_resources", "--output", str(output), "--cards-source", str(cards),
                    "--rulings-source", str(rulings), "--rules-source", str(rules)]
            with patch("sys.argv", argv), patch.object(resources, "get", side_effect=AssertionError("network")):
                with contextlib.redirect_stdout(io.StringIO()):
                    resources.main()
                snapshot = {p.relative_to(output): p.read_bytes() for p in output.rglob("*") if p.is_file()}
                manifest = json.loads(snapshot[Path("manifest.json")])
                self.assertEqual(manifest["counts"]["ruling_links"], 1)
                # the edition is named once, and the rules file is the one it names
                self.assertEqual(manifest["sources"]["rules"]["edition"], pinned.RULES_EDITION)
                self.assertIn(f"rules/{pinned.RULES_NAME}", manifest["files"])
                for name, info in manifest["files"].items():
                    self.assertEqual(info["sha256"], resources.sha(snapshot[Path(name)]))
                # the pinned text is not another edition's, nor is an error page any edition's
                with patch("sys.argv", [*argv, "--rules-edition", "20270101"]):
                    with self.assertRaisesRegex(ValueError, "expected the pinned MagicCompRules-20270101.txt"):
                        resources.main()
                rules.write_text("<html>not a rulebook</html>")
                with self.assertRaisesRegex(ValueError, "expected the pinned"):
                    resources.main()
                self.assertEqual(snapshot, {p.relative_to(output): p.read_bytes() for p in output.rglob("*") if p.is_file()})

    def test_legal_pool_lists_legal_cards_once_without_banned_or_meld_results(self):
        legal = lambda status: {"standard": status}
        cards = [{"id": "1", "oracle_id": "a", "name": "Zed", "legalities": legal("legal")},
                 {"id": "2", "oracle_id": "a", "name": "Zed", "legalities": legal("legal")},  # a reprint
                 {"id": "3", "oracle_id": "b", "name": "Ban", "legalities": legal("banned")},
                 {"id": "4", "oracle_id": "c", "name": "Old", "legalities": legal("not_legal")},
                 {"id": "5", "oracle_id": "d", "name": "Abe", "legalities": legal("legal")},
                 {"id": "6", "oracle_id": "e", "name": "Melded", "legalities": legal("legal"),  # another printing's id in the parts
                  "all_parts": [{"id": "9", "component": "meld_result", "name": "Melded"}, {"id": "5", "component": "meld_part", "name": "Abe"}]}]
        self.assertEqual(resources.legal_pool(cards), [{"oracle_id": "d", "name": "Abe"}, {"oracle_id": "a", "name": "Zed"}])
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            output = root / "resources"
            (output / "oracle").mkdir(parents=True)
            (output / "oracle/cards.jsonl").write_bytes(resources.jsonl([{"oracle_id": "a", "name": "Zed"}, {"oracle_id": "d", "name": "Abe"}]))
            (output / "manifest.json").write_text(json.dumps({"schema": 1, "sources": {}, "counts": {"cards": 2}, "files": {}}))
            bulk = root / "bulk.json"
            bulk.write_text(json.dumps(cards))
            argv = ["fetch_resources", "--output", str(output), "--legal-only", str(bulk)]
            with patch("sys.argv", argv), patch.object(resources, "get", side_effect=AssertionError("network")):
                with contextlib.redirect_stdout(io.StringIO()):
                    resources.main()
            manifest = json.loads((output / "manifest.json").read_text())
            data = (output / "oracle/legal-standard.jsonl").read_bytes()
            self.assertEqual(manifest["counts"], {"cards": 2, "legal_standard": 2})
            self.assertEqual(manifest["files"]["oracle/legal-standard.jsonl"], {"bytes": len(data), "sha256": resources.sha(data)})
            self.assertEqual(manifest["sources"]["legal_standard"], {"imported_from": str(bulk), "sha256": resources.sha(bulk.read_bytes()),
                                                                          "filtering_policy": resources.LEGAL_POLICY})
            metadata = root / "bulk-metadata.json"
            metadata.write_text(json.dumps({"type": "oracle_cards", "id": "bulk-id",
                "uri": "https://api.scryfall.com/bulk-data/bulk-id", "download_uri": "https://data.scryfall.io/bulk.json",
                "updated_at": "2026-10-11T09:00:00Z", "retrieved_at": "2026-10-11T10:00:00Z",
                "sha256": resources.sha(bulk.read_bytes())}))
            dated_argv = argv + ["--legal-source-metadata", str(metadata)]
            with patch("sys.argv", dated_argv), patch.object(resources, "get", side_effect=AssertionError("network")):
                with contextlib.redirect_stdout(io.StringIO()):
                    resources.main()
            dated_manifest = json.loads((output / "manifest.json").read_text())
            self.assertEqual(dated_manifest["sources"]["legal_standard"]["updated_at"], "2026-10-11T09:00:00Z")
            self.assertEqual((output / "oracle/legal-standard.jsonl").read_bytes(), data)
            snapshot = {p.relative_to(output): p.read_bytes() for p in output.rglob("*") if p.is_file()}
            metadata.write_text(metadata.read_text().replace(resources.sha(bulk.read_bytes()), "incorrect"))
            with patch("sys.argv", dated_argv), self.assertRaisesRegex(ValueError, "SHA-256"):
                resources.main()
            self.assertEqual(snapshot, {p.relative_to(output): p.read_bytes() for p in output.rglob("*") if p.is_file()})
            (output / "oracle/cards.jsonl").write_bytes(resources.jsonl([{"oracle_id": "a", "name": "Zed"}]))
            with patch("sys.argv", argv), self.assertRaisesRegex(ValueError, "not in the corpus: Abe"):
                resources.main()

    def test_bulk_provenance_binds_bytes_and_requires_actual_zoned_dates(self):
        raw = b"[]"
        metadata = {"type": "oracle_cards", "id": "bulk-id", "uri": "https://api.scryfall.com/bulk-data/bulk-id",
                    "download_uri": "https://data.scryfall.io/cards.json", "updated_at": "2026-10-11T09:00:00Z",
                    "retrieved_at": "2026-10-11T10:00:00+00:00", "sha256": resources.sha(raw)}
        info = resources.bulk_provenance(raw, metadata)
        self.assertEqual(info["updated_at"], metadata["updated_at"])
        self.assertEqual(info["retrieved_at"], metadata["retrieved_at"])
        with self.assertRaisesRegex(ValueError, "SHA-256"):
            resources.bulk_provenance(b"different", metadata)
        with self.assertRaisesRegex(ValueError, "timezone"):
            resources.bulk_provenance(raw, metadata | {"updated_at": "2026-10-11"})
        with self.assertRaisesRegex(ValueError, "oracle_cards"):
            resources.bulk_provenance(raw, metadata | {"type": "rulings"})

    def test_dated_scope_reconciles_bans_and_rejects_digital_tabletop_cards(self):
        cards = [{"name": "A", "legalities": {"standard": "legal"}, "games": ["paper"]},
                 {"name": "B", "legalities": {"standard": "banned"}}]
        source = {"updated_at": "2026-10-11T09:00:00Z", "retrieved_at": "2026-10-11T10:00:00Z"}
        scope = {"effective_at": "2026-10-11T09:00:00Z", "banned_cards": ["B"],
                 "platform": "tabletop", "sources": [{"url": "https://magic.wizards.com/en/formats/standard"}],
                 "legal_set_codes": ["woe"]}
        self.assertEqual(resources.legal_provenance(source, cards, scope)["scope"], scope)
        with self.assertRaisesRegex(ValueError, "bans disagree"):
            resources.legal_provenance(source, cards, scope | {"banned_cards": ["C"]})
        with self.assertRaisesRegex(ValueError, "dated bulk"):
            resources.legal_provenance({}, cards, scope)
        with self.assertRaisesRegex(ValueError, "unavailable for tabletop"):
            resources.legal_provenance(source, [cards[0] | {"games": ["arena"]}], None)
        with self.assertRaisesRegex(ValueError, "unavailable for tabletop"):
            resources.legal_provenance(source, [cards[0] | {"digital": True}], None)


if __name__ == "__main__":
    unittest.main()
