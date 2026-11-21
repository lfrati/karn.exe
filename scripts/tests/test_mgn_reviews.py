"""Review filtering retains scope, findings and append-only entry provenance."""
from copy import deepcopy
from pathlib import Path
import tempfile
import unittest

from mgn.canonical import canonical
from mgn.reviews import query


class ReviewQueryTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.path = Path(self.temp.name) / "review.jsonl"
        self.entry = {"record": "sha256:" + "a" * 64, "reviewer": {"kind": "agent", "name": "雪", "provider": "test", "model": "test"},
                      "date": "2026-10-19", "scope": {"transitions": [1, 4]}, "tiers": [1, 2], "verdict": "vetted",
                      "findings": [{"transition": 3, "note": "Rule review\u2028kept intact", "rules": ["117.4"]}],
                      "receipt": "evidence/review", "supersedes": "sha256:" + "b" * 64}

    def write(self, entries):
        self.path.write_text("\n".join(canonical(entry) for entry in entries) + "\n", encoding="utf-8")

    def test_identity_verdict_range_and_full_provenance(self):
        other = deepcopy(self.entry)
        other.update(record="sha256:" + "c" * 64, verdict="defect")
        all_scope = deepcopy(self.entry)
        all_scope.update(scope="all", verdict="replayed")
        self.write([self.entry, other, all_scope, self.entry])
        matches = query([self.path], record=self.entry["record"], verdicts=["vetted"], start=4, end=9)
        self.assertEqual(matches, [{"source": str(self.path), "line": line, "entry": self.entry} for line in (1, 4)])
        self.assertEqual(len(query([self.path], start=5, end=5)), 1)
        self.assertEqual(len(query([self.path], start=0, end=1)), 4)
        self.assertEqual(query([self.path], record="sha256:" + "d" * 64), [])
        self.assertEqual(len(query([self.path, self.path])), 8)  # no dedup or latest-wins semantics

    def test_append_preserves_prior_matches_and_line_numbers(self):
        self.write([self.entry])
        before = query([self.path])
        with self.path.open("a", encoding="utf-8") as stream:
            stream.write(canonical(self.entry) + "\n")
        self.assertEqual(query([self.path])[:1], before)
        self.assertEqual(query([self.path])[1]["line"], 2)

    def test_bad_logs_fail_even_when_filter_would_exclude_entry(self):
        bad_scope = deepcopy(self.entry)
        bad_scope["scope"] = {"transitions": [4, 1]}
        for text in ('{}\n', '\n', canonical(self.entry) + '\n\n', '{"record":1,"record":2}\n', canonical(bad_scope)):
            self.path.write_text(text, encoding="utf-8")
            with self.subTest(text=text), self.assertRaisesRegex(ValueError, r"review.jsonl:[0-9]+:"):
                query([self.path], record="sha256:" + "d" * 64)

    def test_invalid_queries(self):
        for kwargs in ({"start": -1}, {"end": -1}, {"start": 3, "end": 2}, {"verdicts": ["approved"]}, {"record": "not-an-identity"}):
            with self.subTest(kwargs=kwargs), self.assertRaises(ValueError):
                query([], **kwargs)


if __name__ == "__main__":
    unittest.main()
