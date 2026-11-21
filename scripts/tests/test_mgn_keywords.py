"""Rule-derived keyword characteristics (CR 113.2c, 702; MGN 8.5)."""
import json
import re
import unittest

from mgn.tokens import _split_abilities
from mgn.keywords import is_keyword_characteristic, keyword_row, merge_observed_keywords, normalize_keyword, printed_keywords
from mgn.oracle import Face, Oracle, ROOT, keyword_count
from mgn.state import State


class KeywordTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.oracle = Oracle()

    def face(self, name):
        return self.oracle.faces[self.oracle.by_name(name)][0]

    def test_actual_oracle_costs_not_bare_search_index(self):
        # CR 702.21a ward's cost; 702.6a equip's cost; 702.33a kicker costs.
        self.assertEqual(self.face("Cactusfolk Sureshot").printed()["keywords"], ["reach", "ward {2}"])
        self.assertEqual(self.face("Maul of the Skyclaves").printed()["keywords"], ["equip {2}{W}{W}"])
        self.assertEqual(self.face("Archangel of Wrath").printed()["keywords"],
                         ["flying", "kicker {B} and/or {R}", "lifelink"])
        self.assertEqual(self.face("Team Pennant").printed()["keywords"], ["equip creature token {1}", "equip {3}"])

    def test_dash_cost_and_ability_text(self):
        # CR 702.21a: discarding is the ward cost, including sentence punctuation.
        self.assertEqual(self.face("Graveyard Trespasser // Graveyard Glutton").printed()["keywords"],
                         ["daybound", "ward—Discard a card."])
        # CR 702.169a: Solved introduces an ability; its body is not a keyword parameter.
        self.assertEqual(self.face("Case of the Uneaten Feast").printed()["keywords"], ["solved"])
        self.assertEqual(keyword_row("Kicker {X}. X can't be 0."), ["kicker {X}"])

    def test_text_introducing_keywords_keep_the_wrapper_not_its_body(self):
        # CR 702.142a, 702.177a, 702.178a, 702.193a: these names introduce
        # activated/static abilities, not keyword parameters or unconditional grants.
        examples = {"Pacesetter Paragon": "exhaust", "Thanos, the Mad Titan": "power-up",
                    "Lightwheel Enhancements": "max speed", "Baron Helmut Zemo": "boast"}
        for card, name in examples.items():
            with self.subTest(card=card):
                self.assertIn(name, self.face(card).printed()["keywords"])
        self.assertEqual(keyword_row("Max speed — Flying"), ["max speed"])
        self.assertEqual(keyword_row("Exhaust — {2}{R}: Draw a card. Activate only as a sorcery."), ["exhaust"])
        self.assertEqual(printed_keywords("Exhaust — {1}: Draw a card.\nExhaust — {2}: Gain 2 life."),
                         ["exhaust", "exhaust"])

    def test_pinned_standard_text_keyword_rows(self):
        # Every pinned Standard row of the four families: Exhaust, Power-up, Max speed, Boast.
        legal = {json.loads(line)["oracle_id"] for line in
                 (ROOT / "resources/oracle/legal-standard.jsonl").read_text().splitlines()}
        exercised = set()
        for oracle_id in legal:
            for face in self.oracle.faces[oracle_id]:
                for row in face.raw.get("oracle_text", "").splitlines():
                    match = re.match(r"^(Exhaust|Power-up|Max speed|Boast) — ", row)
                    if match:
                        name = match[1].lower()
                        with self.subTest(card=face.name, row=row):
                            self.assertEqual(keyword_row(row), [name])
                        exercised.add(name)
        self.assertEqual(exercised, {"exhaust", "power-up", "max speed", "boast"})

    def test_quality_qualified_devour_retains_quality_and_number(self):
        # CR 702.82c: Devour land 3 may sacrifice lands, with three counters per land.
        self.assertIn("devour land 3", self.face("Famished Worldsire").printed()["keywords"])
        self.assertEqual(keyword_row("Devour land 3"), ["devour land 3"])
        self.assertEqual(keyword_row("Devour 3"), ["devour 3"])
        self.assertEqual(keyword_row("Devour abilities are interesting."), [])

    def test_reminder_and_granted_keywords_are_not_printed_abilities(self):
        # CR 207.2a/c, 113.1a: reminder text and ability words give no abilities.
        face = Face({"name": "Example", "keywords": ["Ward", "Flying", "Landfall"], "oracle_text":
                     "Disguise {2}{R} (You may cast this face down with ward {2}.)\n"
                     "Landfall — Whenever a land enters, this creature gains flying until end of turn."})
        self.assertEqual(face.printed()["keywords"], ["disguise {2}{R}"])
        self.assertEqual(printed_keywords('When this creature enters, create a token with flying.'), [])

    def test_parameters_and_multiplicity_survive(self):
        # CR 113.2c / 702.21a: each ward instance triggers; 702.5c: every enchant applies.
        self.assertEqual(printed_keywords("Ward {2}\nWard {2}\nWard {3}\nEnchant creature\nEnchant land"),
                         ["enchant creature", "enchant land", "ward {2}", "ward {2}", "ward {3}"])
        self.assertEqual(keyword_row("Flying, ward {2}, ward {3}"), ["flying", "ward {2}", "ward {3}"])
        self.assertEqual(keyword_count("Flying, ward—Pay 2 life, sacrifice a creature."), 2)
        self.assertEqual(keyword_row("Ward—Pay 2 life, sacrifice a creature."),
                         ["ward—Pay 2 life, sacrifice a creature."])
        self.assertEqual(normalize_keyword("WARD {w/u}"), "ward {W/U}")
        self.assertEqual(normalize_keyword("Protection from Elves"), "protection from Elves")
        self.assertEqual(keyword_row("Craft with artifact {3}{W}"), ["craft with artifact {3}{W}"])
        self.assertEqual(keyword_row("Plainscycling {2}"), ["plainscycling {2}"])
        self.assertEqual(keyword_row("Artifact landcycling {2}"), ["artifact landcycling {2}"])
        self.assertEqual(keyword_row("Goblin offering"), ["goblin offering"])
        # Ability words are labels, not abilities; the keyword following one remains.
        self.assertIn("equip {4}", self.face("Paladin's Arms").printed()["keywords"])

    def test_keyword_like_prose_is_not_a_keyword_row(self):
        for text in ("Equip abilities you activate cost {1} less to activate.",
                     "Cycling abilities you activate cost {2} less to activate.",
                     "Storm Seeker deals damage to target player.",
                     "Champion of Freedom — Whenever this creature attacks, draw a card."):
            self.assertEqual(keyword_row(text), [], text)

    def test_characteristic_spelling_is_one_complete_canonical_value(self):
        # CR 702.5a/6a/16a/21a define parameters; 702.177a introduces a
        # following ability body which the characteristic spells only as exhaust.
        valid = ["flying", "ward {2}", "ward {W/U}", "ward—Pay 3 life.", "equip {3}",
                 "equip creature token {1}", "enchant creature", "protection from Elves",
                 "kicker {B} and/or {R}", "devour land 3", "plot {2}{R}", "exhaust",
                 "max speed", "hexproof", "partner", "plainscycling {2}", "goblin offering"]
        for value in valid:
            with self.subTest(valid=value):
                self.assertTrue(is_keyword_characteristic(value))
        invalid = ["ward bananas", "flying penguins", "exhaust — {R}: Draw a card.",
                   "ward", "equip", "enchant", "plot", "gift", "affinity", "craft", "devour",
                   "toxic", "impending", "Flying", "ward {w}", "flying, haste", " flying",
                   "flying (This creature can't be blocked except by creatures with flying.)"]
        for value in invalid:
            with self.subTest(invalid=value):
                self.assertFalse(is_keyword_characteristic(value))
        for card in ("Cactusfolk Sureshot", "Maul of the Skyclaves", "Archangel of Wrath",
                     "Team Pennant", "Graveyard Trespasser // Graveyard Glutton", "Famished Worldsire"):
            for value in self.face(card).printed()["keywords"]:
                with self.subTest(card=card, value=value):
                    self.assertTrue(is_keyword_characteristic(value))
        # Checking values must not deduplicate 113.2c's independent instances.
        repeated = printed_keywords("Ward {2}\nWard {2}\nWard {3}")
        self.assertEqual([k for k in repeated if is_keyword_characteristic(k)],
                         ["ward {2}", "ward {2}", "ward {3}"])

    def test_old_observation_uses_printed_evidence_without_duplicate_instances(self):
        self.assertEqual(merge_observed_keywords(["ward {2}", "ward {3}"], ["ward"]), ["ward {2}", "ward {3}"])
        self.assertEqual(merge_observed_keywords(["ward {2}"], ["ward {2}", "ward {2}"]), ["ward {2}", "ward {2}"])
        self.assertEqual(merge_observed_keywords([], ["flying"]), ["flying"])
        for name in ("ward", "plot", "gift", "affinity", "craft", "devour", "toxic", "impending"):
            with self.subTest(name=name), self.assertRaisesRegex(ValueError, "no observed or printed parameter"):
                merge_observed_keywords([], [name])

    def test_token_definition_parameters_and_repeated_instances(self):
        self.assertEqual(_split_abilities('ward {W} and flying'), (["ward {W}", "flying"], None))
        self.assertEqual(_split_abilities('enchant creature and "Enchanted creature gets +1/+1."'),
                         (["enchant creature"], "Enchanted creature gets +1/+1."))
        self.assertEqual(_split_abilities('protection from red and from black and flying'),
                         (["protection from red and from black", "flying"], None))
        state = State(tier=2, seats=["A", "B"], cards={}, turn={}, pending=None, result=None)
        state.zones["battlefield"] = ["T1"]
        state.objects["T1"] = {"token": {"name": "Example", "keywords": ["ward {W}", "ward {W}", "enchant creature"]}}
        self.assertEqual(state._printed("T1", None)["keywords"], ["enchant creature", "ward {W}", "ward {W}"])




if __name__ == "__main__":
    unittest.main()
