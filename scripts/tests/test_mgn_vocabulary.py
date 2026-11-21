"""The MGN vocabularies derived from the pinned rules and card text."""
import unittest

from mgn import vocabulary


class VocabularyTests(unittest.TestCase):
    def test_written_lists_match_the_rules_text(self):
        self.assertEqual(vocabulary.verify_against_rules(), [])

    def test_keyword_abilities_come_from_section_702(self):
        keywords = vocabulary.keyword_abilities()
        for name in ("flying", "first strike", "deathtouch", "indestructible", "ward", "daybound", "nightbound",
                     "start your engines!", "landwalk"):
            self.assertIn(name, keywords)
        self.assertNotIn("most abilities", " ".join(keywords))
        self.assertEqual(vocabulary.spelled_keyword("ward {2}"), "ward")
        self.assertEqual(vocabulary.spelled_keyword("Protection from red"), "protection")
        self.assertEqual(vocabulary.spelled_keyword("islandwalk"), "landwalk")
        self.assertIsNone(vocabulary.spelled_keyword("indestructable"))

    def test_subtypes_are_grouped_by_card_type(self):
        subtypes = vocabulary.subtypes()
        self.assertIn("Elf", subtypes["creature"])
        self.assertIn("Time Lord", subtypes["creature"])
        self.assertIn("Forest", subtypes["land"])
        self.assertIn("Aura", subtypes["enchantment"])
        self.assertIn("Equipment", subtypes["artifact"])
        self.assertIn("Clue", subtypes["artifact"])
        self.assertEqual(subtypes["battle"], ("Siege",))
        self.assertIn("Adventure", subtypes["instant"])
        self.assertNotIn("", vocabulary.all_subtypes())

    def test_counter_kinds_include_rules_and_card_text_kinds(self):
        kinds = vocabulary.counter_kinds()
        for kind in ("+1/+1", "-1/-1", "loyalty", "poison", "stun", "lore", "charge"):
            self.assertIn(kind, kinds)
        for kind in kinds:
            self.assertRegex(kind, vocabulary.COUNTER_KIND)

    def test_mana_symbols(self):
        for symbol in ("{W}", "{2}", "{X}", "{C}", "{S}", "{W/U}", "{2/W}", "{C/G}", "{G/P}", "{W/U/P}", "{15}"):
            self.assertRegex(symbol, vocabulary.MANA_SYMBOL)
        for symbol in ("W", "{w}", "{WW}", "{P}", "{2/C}"):
            self.assertNotRegex(symbol, vocabulary.MANA_SYMBOL)


if __name__ == "__main__":
    unittest.main()
