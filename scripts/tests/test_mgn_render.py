"""Derived notation preserves canonical inputs; it does not adjudicate Magic."""
from copy import deepcopy
from pathlib import Path
import math
import unittest

from mgn.canonical import canonical_bytes, loads
from mgn.examples import load, negative_cases, positive_paths
from mgn.identity import record_identity
from mgn.render import parse, render

ROOT = Path(__file__).resolve().parents[2]


class RenderTests(unittest.TestCase):
    def test_corpus_canonical_byte_roundtrip(self):
        paths = positive_paths() + sorted((ROOT / "tests/fixtures/mgn").rglob("record.json"))
        paths += sorted((ROOT / "tests/fixtures/mgn").rglob("draft.json"))
        self.assertTrue(paths)
        for path in paths:
            with self.subTest(path=str(path)):
                record = load(path)
                rendered = render(record)
                parsed = parse(rendered)
                self.assertEqual(canonical_bytes(parsed), canonical_bytes(record))
                self.assertEqual(record_identity(parsed), record_identity(record))
                self.assertEqual(render(parsed), rendered)

    def test_negative_examples_are_not_silently_repaired(self):
        for case in negative_cases():
            with self.subTest(case=case.get("name")):
                record = case["record"]
                self.assertEqual(canonical_bytes(parse(render(record))), canonical_bytes(record))

    def test_unicode_comments_notes_extensions_full_pins_and_empty_values(self):
        record = deepcopy(load(positive_paths()[0]))
        record["note"] = 'quoted " # note\nline\t雪\u2028🙂'
        record["x-unknown"] = {"": {}, "1": [], "# tricky:= ": {"😀": "é", "\ue000": None}, "array": [{"x": 1e-7}]}
        record["transitions"][0]["x-extra"] = {"random": [], "transitions": {"odd key": "value"}}
        record["cards"][next(iter(record["cards"]))]["x-note"] = "do not discard"
        text = render(record)
        parsed = parse("# comment\n" + text + "  # another comment\n")
        self.assertEqual(canonical_bytes(parsed), canonical_bytes(record))
        for pin in record["pins"].values():
            self.assertIn(pin["sha256"], text)
        self.assertIn('note = "quoted', text)
        self.assertIn("transitions:\n  1:\n", text)
        self.assertIn("    do:\n", text)

    def test_finite_float_boundaries_preserve_canonical_bytes(self):
        numbers = [0.0, -0.0, 5e-324, -5e-324, 1.7976931348623157e308]
        for boundary in (float(2**53), 1e-6, 1e-7, 1e20, 1e21):
            numbers.extend([math.nextafter(boundary, -math.inf), boundary,
                            math.nextafter(boundary, math.inf), -boundary])
        for number in numbers:
            # Exercise scalar leaves, arrays and objects nested inside arrays.
            record = {"mgn": "1", "transitions": [], "x-number": number,
                      "x-nested": [number, {"nested": [number]}]}
            with self.subTest(number=number):
                text = render(record)
                self.assertEqual(canonical_bytes(parse(text)), canonical_bytes(record))
                self.assertEqual(render(parse(text)), text)
                self.assertEqual(canonical_bytes(loads(canonical_bytes(parse(text)).decode())), canonical_bytes(record))
        record = loads('{"mgn":"1","transitions":[],"x-number":1e20}')
        self.assertIn("x-number = 1e+20", render(record))
        self.assertEqual(canonical_bytes(parse(render(record))), canonical_bytes(record))

    def test_numeric_fix_keeps_canonical_validation(self):
        for number in (2**53 + 1, float("inf"), float("nan")):
            with self.subTest(number=number), self.assertRaises(ValueError):
                render({"mgn": "1", "transitions": [], "x-number": number})
        with self.assertRaisesRegex(ValueError, "line 4:"):
            parse('MGN-TEXT 1\nmgn = "1"\ntransitions = []\nx-number = 9007199254740993\n')

    def test_empty_transitions_and_empty_transition_object(self):
        for transitions in ([], [{}]):
            record = {"mgn": "1", "transitions": transitions}
            self.assertEqual(parse(render(record)), record)

    def test_strict_parse_errors(self):
        base = 'MGN-TEXT 1\nmgn = "1"\ntransitions = []\n'
        invalid = [
            "MGN-TEXT 2\n", base + 'mgn = "1"\n', base + '  wrong = 1\n',
            base + 'x-empty:\n', base + 'x-bad = NaN\n', base + 'x-bad = Infinity\n',
            base + 'x-bad = {"x":1,"x":2}\n', base + 'x-bad = "\\ud800"\n',
            base + 'x-bad = 1 # comment\n', base + '\tx-bad = 1\n', base + ' x-bad = 1\n',
            base + 'x-bad: 1\n', base + '123 = 1\n', base + 'x-bad = "unterminated\n',
            'MGN-TEXT 1\nmgn = "1"\ntransitions:\n  2:\n    seat = "A"\n',
            'MGN-TEXT 1\nmgn = "1"\ntransitions:\n  1 = false\n',
        ]
        for text in invalid:
            with self.subTest(text=text), self.assertRaisesRegex(ValueError, "line [0-9]+:"):
                parse(text)


if __name__ == "__main__":
    unittest.main()
