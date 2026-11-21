import copy
import sys
import unittest
from pathlib import Path
sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import selector_snapshot as snapshot


def reading(value="creature", verdict="round_trip", printed="a creature"):
    return {"value": value, "verdict": verdict, "printed": printed}


def sample(value=None):
    return {"schema": 1, "revision": "test", "phrases_sha256": "phrases", "corpus_sha256": "cards",
            "phrases": [{"phrase": "a creature", "readings": {"selector": value or reading()}}]}


class SemanticSnapshotTests(unittest.TestCase):
    def test_same_acceptance_and_printing_does_not_hide_a_changed_meaning(self):
        before, after = sample(), sample(reading(value="artifact"))
        result = snapshot.compare(before, after)
        self.assertFalse(result["ok"])
        self.assertEqual(result["changes"][0]["kind"], "meaning")

    def test_round_trip_to_variant_is_a_visible_failure(self):
        result = snapshot.compare(sample(), sample(reading(verdict="variant", printed="one creature")))
        self.assertFalse(result["ok"])
        self.assertEqual(result["changes"][0]["kind"], "printing")
        self.assertEqual(result["before_counts"]["selector:round_trip"], 1)
        self.assertEqual(result["after_counts"]["selector:variant"], 1)

    def test_review_is_exact_requires_rationale_and_expires_when_the_change_changes(self):
        before, after = sample(), sample(reading(value="artifact"))
        change = snapshot.compare(before, after)["changes"][0]
        approval = {k: change[k] for k in ("phrase", "rule", "before_sha256", "after_sha256")}
        with self.assertRaises(ValueError): snapshot.compare(before, after, [approval])
        approval["rationale"] = "Rule-derived correction checked with selector truth-table test."
        self.assertTrue(snapshot.compare(before, after, [approval])["ok"])
        self.assertFalse(snapshot.compare(before, sample(reading(value="land")), [approval])["ok"])
        stale = snapshot.compare(before, before, [approval])
        self.assertFalse(stale["ok"])
        self.assertEqual(len(stale["stale_reviews"]), 1)

    def test_approval_cannot_hide_new_ambiguity_or_mismatch(self):
        for verdict in ("ambiguous", "mismatch"):
            before, after = sample(), sample({"verdict": verdict})
            change = snapshot.compare(before, after)["changes"][0]
            change["rationale"] = "Not a legitimate exemption."
            self.assertFalse(snapshot.compare(before, after, [change])["ok"])

    def test_input_mismatch_and_duplicate_phrases_are_errors(self):
        before = sample()
        for key in ("schema", "phrases_sha256", "corpus_sha256"):
            after = copy.deepcopy(before)
            after[key] = "different"
            with self.assertRaises(ValueError): snapshot.compare(before, after)
        after = copy.deepcopy(before)
        after["phrases"].append(after["phrases"][0])
        with self.assertRaises(ValueError): snapshot.compare(before, after)

    def test_skips_and_gains_are_counted_without_inventing_parsed_evidence(self):
        before = sample({"verdict": "declined"})
        after = sample()
        for s in (before, after): s["phrases"].append({"phrase": "QUOTED", "skip": "quoted-placeholder"})
        result = snapshot.compare(before, after)
        self.assertEqual(result["changes"][0]["kind"], "gained")
        self.assertEqual(result["after_counts"]["skipped_phrases"], 1)
        self.assertEqual(result["after_counts"]["examined_phrases"], 1)
        self.assertTrue(snapshot.compare(after, after)["ok"])
