"""MGN 8.5 subtype spellings from pinned CR 205.3i/m, independent of native output."""
from copy import deepcopy
import unittest

from mgn.canonical import sha256
from mgn.tokens import _parse_token_description
from mgn.oracle import FACE_FIELDS, Face, Oracle
from mgn.vocabulary import canonical_subtype, native_subtype, subtypes


class SubtypeSpellingTests(unittest.TestCase):
    def test_rule_spellings_and_exact_native_aliases(self):
        # CR 205.3m lists these spellings, including U+2019; MGN 8.5 uses them.
        for native, printed in (("C'tan", "C’tan"), ("Shi'ar", "Shi’ar")):
            self.assertIn(printed, subtypes()["creature"])
            self.assertNotIn(native, subtypes()["creature"])
            self.assertEqual(canonical_subtype(native, "creature"), printed)
            self.assertEqual(canonical_subtype(printed, "creature"), printed)
            self.assertEqual(native_subtype(printed, "creature"), native)
            self.assertEqual(canonical_subtype(native_subtype(printed)), printed)
        # CR 205.3i also uses a typographic apostrophe for this land type.
        self.assertEqual(canonical_subtype("Urza's", "land"), "Urza’s")
        self.assertEqual(native_subtype("Urza’s", "land"), "Urza's")

    def test_exact_vocabulary_and_choice_domains_remain_closed(self):
        for bad in ("C‘tan", "C’tann", "shi'ar", "Shi`ar", "Time_Lord", "Invented's", "C’tan "):
            with self.subTest(bad=bad), self.assertRaises(ValueError):
                canonical_subtype(bad)
        with self.assertRaises(ValueError):
            canonical_subtype("Forest", "creature")
        with self.assertRaises(ValueError):
            native_subtype("Urza’s", "basic_land")
        # CR 305.6, independent of the broader land-type list in 205.3i.
        for basic in ("Plains", "Island", "Swamp", "Mountain", "Forest"):
            self.assertEqual(native_subtype(basic, "basic_land"), basic)
        self.assertEqual(canonical_subtype("Time Lord", "creature"), "Time Lord")

    def test_oracle_defaults_canonicalize_without_changing_provenance(self):
        for ascii_name, rule_name in (("C'tan", "C’tan"), ("Shi'ar", "Shi’ar"), ("Urza's", "Urza’s")):
            raw = {"name": "Evidence", "type_line": ("Land — " if ascii_name == "Urza's" else "Creature — ") + ascii_name, "colors": [], "oracle_text": ""}
            before = deepcopy(raw)
            digest = sha256({field: raw[field] for field in FACE_FIELDS if field in raw})
            face = Face(raw)
            self.assertEqual(face.printed()["subtypes"], [rule_name])
            self.assertEqual(face.digest(), digest)
            self.assertEqual(raw, before)
        with self.assertRaises(ValueError):
            Face({"name": "Invalid", "type_line": "Creature — Unlisted"}).printed()

    def test_token_descriptions_and_observed_subtypes_use_rule_spellings(self):
        for native, rule_name in (("C'tan", "C’tan"), ("Shi'ar", "Shi’ar"), ("Time Lord", "Time Lord")):
            self.assertEqual(_parse_token_description("1/1 colorless " + native + " creature")["subtypes"], [rule_name])
        self.assertIsNone(_parse_token_description("1/1 colorless Unknown creature"))

    def test_actual_pinned_oracle_faces_keep_raw_digests(self):
        oracle = Oracle()
        count = 0
        for faces in oracle.faces.values():
            for face in faces:
                if any(name in face.raw.get("type_line", "") for name in ("C'tan", "Shi'ar", "Urza's")):
                    raw = deepcopy(face.raw)
                    digest = face.digest()
                    expected = raw["type_line"].partition("—")[2].strip().split()
                    spellings = {"C'tan": "C’tan", "Shi'ar": "Shi’ar", "Urza's": "Urza’s"}
                    self.assertEqual(face.printed()["subtypes"], sorted(spellings.get(name, name) for name in expected))
                    self.assertEqual(face.digest(), digest)
                    self.assertEqual(face.raw, raw)
                    count += 1
        self.assertGreater(count, 0)


if __name__ == "__main__":
    unittest.main()
