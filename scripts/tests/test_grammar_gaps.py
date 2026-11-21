import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import grammar_gaps


def gap(edit, span, template=None, reads_as=(), before="Destroy target", after="creature."):
    return {"edit": edit, "stand_in": None, "span": span, "template": template or span.lower(),
            "reads_as": list(reads_as), "before": before, "after": after, "quoted": False}


def row(oracle_id, gaps=(), unexplained=None):
    return {"oracle_id": oracle_id, "card": oracle_id.upper(), "face": oracle_id.upper(), "row": "…", "mode": False,
            "gaps": list(gaps), "unexplained": unexplained}


STOP = {"read": "Companion — Your", "at": "starting", "rest": "starting deck contains only cards.", "expected": []}
ROWS = [
    row("a", [gap("removed", "1/1", "⟨n⟩ / ⟨n⟩")]),
    row("b", [gap("removed", "1/1", "⟨n⟩ / ⟨n⟩"), gap("an amount", "the greatest number")]),
    row("c", [gap("removed", "until end of turn", reads_as=["a modifier"])]),
    row("d", [gap("a sentence", "End the turn"), gap("a noun", "")]),
    row("e", unexplained=STOP),
]
DECKS = {"a": 30, "b": 2, "e": 7}


class GrammarGapsTests(unittest.TestCase):
    def test_a_gap_is_named_by_what_the_grammar_lacks(self):
        self.assertEqual([grammar_gaps.kind_of(g) for r in ROWS for g in r["gaps"]], [
            "an extra phrase", "an extra phrase", "a new way to say: amount", "a known phrase out of place",
            "a whole sentence", "a missing word"])

    def test_kinds_count_cards_those_lacking_nothing_else_and_their_decks(self):
        table = {line["kind"]: line for line in grammar_gaps.kinds(ROWS, DECKS)}
        self.assertEqual(table["an extra phrase"], {"kind": "an extra phrase", "cards": 2, "only": 1, "decks": 32, "gaps": 2})
        self.assertEqual(table["a new way to say: amount"]["only"], 0)
        self.assertEqual(table[grammar_gaps.UNKNOWN_FRAME], {"kind": grammar_gaps.UNKNOWN_FRAME, "cards": 1, "only": 1, "decks": 7, "gaps": 1})
        # most cards first
        self.assertEqual(grammar_gaps.kinds(ROWS, DECKS)[0]["kind"], "an extra phrase")

    def test_phrases_group_one_kind_by_template_with_the_most_played_example_first(self):
        [line] = grammar_gaps.phrases(ROWS, DECKS, "an extra phrase")
        self.assertEqual((line["template"], line["cards"], line["decks"]), ("⟨n⟩ / ⟨n⟩", 2, 32))
        self.assertEqual([e[:2] for e in line["examples"]], [(30, "A"), (2, "B")])
        self.assertEqual(line["examples"][0][2], "Destroy target ⟦1/1⟧ creature.")

    def test_grep_keeps_the_gaps_whose_words_in_context_match(self):
        self.assertEqual(grammar_gaps.phrases(ROWS, DECKS, "an extra phrase", grep="zorblax"), [])
        [line] = grammar_gaps.phrases(ROWS, DECKS, grammar_gaps.UNKNOWN_FRAME, grep="starting")
        self.assertEqual((line["template"], line["cards"]), ("starting", 1))

    def test_main_decks_count_each_main_deck_once_per_card_and_no_sideboard(self):
        corpus = [{"name": "Alpha", "oracle_id": "a"}, {"name": "Beta // Gamma", "oracle_id": "b"}]
        roster = [{"main": {"Alpha": 4, "Beta // Gamma": 1}, "sideboard": {"Alpha": 2}},
                  {"main": {"Alpha": 1, "Unknown": 3}, "sideboard": {"Beta // Gamma": 2}}]
        self.assertEqual(grammar_gaps.main_decks(roster, corpus), {"a": 2, "b": 1})


if __name__ == "__main__":
    unittest.main()
