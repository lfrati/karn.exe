import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import deck_admission


class DeckBaselineTests(unittest.TestCase):
    def test_main_and_sideboard_are_distinct_name_counts_not_copy_counts(self):
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "deck.dck"
            path.write_text("4 [SET:1] Shared\n2 Shared\n1 Main\nSB: 3 Shared\nSB: 1 Side\n# comment\n")
            self.assertEqual(deck_admission.deck_lists(path), {"main": {"Shared", "Main"}, "sideboard": {"Shared", "Side"}})

    def test_unread_deck_rows_are_errors_not_silent_omissions(self):
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "deck.dck"
            path.write_text("SB: Unknown format\n")
            with self.assertRaisesRegex(ValueError, "unrecognized deck row"):
                deck_admission.deck_lists(path)


if __name__ == "__main__":
    unittest.main()
