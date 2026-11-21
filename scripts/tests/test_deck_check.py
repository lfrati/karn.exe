"""Deck parsing preserves author-supplied order, without a gameplay policy."""
from pathlib import Path
import tempfile
import unittest
from deck_check import read_deck, ordered_main

class DeckCheckTests(unittest.TestCase):
    def test_order_and_sideboard_identity(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / 'cards.dck'
            path.write_text('2 [M21:274] Forest\n1 Mountain\n1 Forest\nSB: 1 Island\n')
            self.assertEqual(ordered_main(path), ['Forest', 'Forest', 'Mountain', 'Forest'])
            main, side = read_deck(path)
            self.assertEqual(main, {'Forest': 3, 'Mountain': 1})
            self.assertEqual(side, {'Island': 1})

    def test_bad_quantity_refused(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / 'cards.dck'
            path.write_text('0 Forest\n')
            with self.assertRaises(ValueError):
                read_deck(path)
