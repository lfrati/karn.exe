import json
import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import card_names


class BakedNamesTests(unittest.TestCase):
    def test_baked_file_is_what_the_corpus_gives(self):
        if not card_names.CARDS.exists():
            self.skipTest("corpus not fetched")
        baked = json.loads(card_names.OUT.read_text(encoding="utf-8"))
        self.assertEqual(baked, card_names.card_names(), "run scripts/card_names.py")


if __name__ == "__main__":
    unittest.main()
