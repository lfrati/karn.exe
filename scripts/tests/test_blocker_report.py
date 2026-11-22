import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import blocker_report


def row(oracle_id, face, stage, blockers=(), unanalyzed=(), name=None):
    blockers = [{"stage": s, "reason": r, "label": label or r, **({"unanalyzed": list(u)} if u else {})}
                for s, r, label, u in blockers]
    fully = not unanalyzed and all("unanalyzed" not in b for b in blockers)
    return {"oracle_id": oracle_id, "face": face, "name": name or f"{oracle_id}/{face}", "layout": "normal",
            "stage": stage, "reason": blockers[0]["reason"] if blockers else None, "compiled_abilities": None,
            "blockers": blockers, "unanalyzed": list(unanalyzed), "fully_analyzed": fully}


LEGAL = [{"oracle_id": i, "name": i.upper()} for i in ("a", "b", "c", "d")]
SNAPSHOT = [
    row("a", 0, "admitted"),
    # two independent engine blockers: the first leaves an instruction after it unanalyzed,
    # the second hides nothing
    row("b", 0, "engine", [("engine", "program: action not lowered: TapOrUntap", None, ["b, ability 1, 1 instruction its lowering did not reach: it stops at its first refusal"]),
                           ("engine", "defense", None, ())]),
    # one blocker, nothing unanalyzed: a sole and fully analyzed one
    row("c", 0, "engine", [("engine", "defense", None, ())]),
    # a double-faced card: the front reads, the back is not read at two rows, one label
    row("d", 0, "admitted"),
    row("d", 1, "parse", [("parse", "text the grammar does not read", 'text the grammar does not read: at "of"', ["the rest of row 0"]),
                          ("parse", "text the grammar does not read", 'text the grammar does not read: at "of"', ["the rest of row 2"])],
        unanalyzed=["engine admission: not analyzed"]),
    # not a legal card: not counted
    row("z", 0, "engine", [("engine", "defense", None, ())]),
]


class ReportTests(unittest.TestCase):
    def test_counts_each_label_once_per_face_and_card(self):
        faces, labels, totals = blocker_report.report(SNAPSHOT, LEGAL)
        by = {label["label"]: label for label in labels}
        self.assertEqual([f["oracle_id"] for f in faces], ["b", "c", "d"])
        self.assertEqual(by["defense"]["cards"], 2)
        self.assertEqual(by["defense"]["first_cards"], 1)
        self.assertEqual(by["defense"]["sole_cards"], 1)
        self.assertEqual(by["defense"]["sole_fully_analyzed_cards"], 1)
        self.assertEqual(by["program: action not lowered: TapOrUntap"]["sole_cards"], 0)
        of = by['text the grammar does not read: at "of"']
        self.assertEqual((of["faces"], of["cards"], of["sole_cards"], of["sole_fully_analyzed_cards"]), (1, 1, 1, 0))
        self.assertEqual(faces[2]["labels"], ['text the grammar does not read: at "of"'])
        self.assertEqual(totals["admitted_cards"], 1)
        self.assertEqual(totals["refused_cards"], {"parse": 1, "engine": 2})
        self.assertEqual(totals["fully_analyzed_refused_faces"], 1)
        self.assertEqual(labels[0]["label"], "defense", "most cards first")

    def test_a_refused_face_needs_a_reason_that_is_its_first_blocker(self):
        bare = [r for r in SNAPSHOT if r["oracle_id"] != "c"] + [{**row("c", 0, "engine"), "reason": "defense"}]
        with self.assertRaisesRegex(ValueError, "without a blocker"):
            blocker_report.report(bare, LEGAL)
        other = [r for r in SNAPSHOT if r["oracle_id"] != "c"] + [{**row("c", 0, "engine", [("engine", "defense", None, ())]), "reason": "x"}]
        with self.assertRaisesRegex(ValueError, "not the refusal"):
            blocker_report.report(other, LEGAL)

    def test_a_legal_card_missing_from_the_snapshot_is_an_error(self):
        with self.assertRaisesRegex(ValueError, "not in the snapshot"):
            blocker_report.report([r for r in SNAPSHOT if r["oracle_id"] != "c"], LEGAL)

    def test_compare_reports_the_admitted_set_and_changed_refusals(self):
        same = blocker_report.compare(SNAPSHOT, SNAPSHOT)
        self.assertTrue(same["admitted_identical"])
        self.assertEqual(same["stage_or_reason_changed"], [])
        changed = [row("c", 0, "admitted") if r["oracle_id"] == "c" else r for r in SNAPSHOT]
        diff = blocker_report.compare(SNAPSHOT, changed)
        self.assertFalse(diff["admitted_identical"])
        self.assertEqual(diff["newly_admitted"], [("c", 0)])
        self.assertEqual(diff["stage_or_reason_changed"][0]["before"], ["engine", "defense"])


def deck(name, main, **parts):
    return {"id": name, "format": "modern", "main": main, **parts}


# A deck names a whole card as the corpus does, by one name for all its faces.
CORPUS = [{"oracle_id": i, "name": i.upper()} for i in ("a", "b", "c", "d", "d", "z")]


class DeckReportTests(unittest.TestCase):

    def test_a_label_counts_the_decks_it_keeps_from_running_and_those_it_alone_does(self):
        roster = [
            deck("runs", {"A": 4}),
            # one card the engine does not run, with one blocker and nothing unanalyzed
            deck("defense alone", {"A": 4, "C": 2}),
            # a companion is played with the main deck
            deck("companion", {"A": 4}, companion="C"),
            # one card with two blockers: no single label runs the deck
            deck("two labels", {"A": 4, "B": 1}),
            # the back face of a double-faced card is not read; the sideboard is not counted
            deck("unread", {"D": 1}, sideboard={"B": 1}),
            # a name the corpus lacks: the deck never runs, whatever is removed
            deck("unknown", {"C": 1, "Nope": 1}),
        ]
        labels, totals = blocker_report.deck_report(SNAPSHOT, roster, CORPUS)
        by = {label["label"]: label for label in labels}
        of = 'text the grammar does not read: at "of"'
        self.assertEqual([label["label"] for label in labels],
                         ["defense", of, "program: action not lowered: TapOrUntap"], "most sole decks first, then decks touched")
        self.assertEqual({k: by["defense"][k] for k in ("stage", "cards", "decks", "sole_decks", "sole_fully_analyzed_decks")},
                         {"stage": "engine", "cards": 2, "decks": 4, "sole_decks": 2, "sole_fully_analyzed_decks": 2})
        self.assertEqual(by["defense"]["examples"], ["B", "C"])
        self.assertEqual({k: by[of][k] for k in ("stage", "cards", "decks", "sole_decks", "sole_fully_analyzed_decks")},
                         {"stage": "parse", "cards": 1, "decks": 1, "sole_decks": 1, "sole_fully_analyzed_decks": 0})
        self.assertEqual((by["program: action not lowered: TapOrUntap"]["decks"],
                          by["program: action not lowered: TapOrUntap"]["sole_decks"]), (1, 0))
        self.assertEqual(totals, {
            "decks": 6, "unresolved_decks": 1, "main_decks_read": 4, "main_decks_that_run": 1,
            "decks_one_label_away": 3, "missing_cards": 3, "missing_cards_by_stage": {"parse": 1, "engine": 2},
            "distinct_labels": 3})

    def test_rank_by_decks_completed_and_count_whole_corpus_cards(self):
        roster = [deck(f"touches {i}", {"B": 1}) for i in range(4)]
        roster += [deck("readable", {"D": 1}), deck("defense alone", {"C": 1})]
        labels, _ = blocker_report.deck_report(SNAPSHOT, roster, CORPUS)
        by = {label["label"]: label for label in labels}
        self.assertEqual([label["sole_decks"] for label in labels], [1, 1, 0])
        self.assertEqual(labels[0]["label"], "defense")
        # Z occurs in no deck; B has two labels, while C and Z have defense alone.
        self.assertEqual((by["defense"]["cards"], by["defense"]["corpus_cards"],
                          by["defense"]["sole_corpus_cards"]), (2, 3, 2))
        self.assertEqual(by["program: action not lowered: TapOrUntap"]["sole_corpus_cards"], 0)

    def test_a_meld_component_needs_the_card_it_melds_into(self):
        snapshot = [row("rats", 0, "admitted", name="Graf Rats"), row("scavengers", 0, "admitted", name="Midnight Scavengers"),
                    row("host", 0, "engine", [("engine", "meld", None, ())], name="Chittering Host")]
        corpus = [{"oracle_id": r["oracle_id"], "name": r["name"]} for r in snapshot]
        labels, totals = blocker_report.deck_report(snapshot, [deck("melds", {"Graf Rats": 4})], corpus)
        self.assertEqual([(label["label"], label["cards"], label["sole_decks"]) for label in labels], [("meld", 1, 1)])
        self.assertEqual(totals["main_decks_that_run"], 0)

    def test_a_corpus_card_missing_from_the_snapshot_is_an_error(self):
        with self.assertRaisesRegex(ValueError, "not in the snapshot"):
            blocker_report.deck_report([r for r in SNAPSHOT if r["oracle_id"] != "c"], [], CORPUS)


if __name__ == "__main__":
    unittest.main()
