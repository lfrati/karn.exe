"""Ability provenance from CR 113.2c, 113.7, 602.5c and 607.5, spec 3.3.

Marvin's first paragraph acquires creatures' activated abilities; Thranduil's
first paragraph acquires graveyard Elf cards' activated abilities. Neither is
the declaring paragraph of Llanowar Elves' mana ability (605.1a).
"""
from copy import deepcopy
import unittest

from mgn.abilities import ability_object_refs, inherited_ability_ref
from mgn.oracle import Oracle
from mgn.schema import Validator, load_schema
from mgn.state import State
from mgn.validate import _ability_refs, _ability_text, _check_abilities, _check_mana_ability, _check_references


MARVIN = "22f0dbbb-9d8d-42bc-8903-fc0b65e00952"
THRANDUIL = "4a5f76e7-40be-4b06-9935-4a3b2672e1c2"


def acquired(donor="elves1", grant="marvin1", ability=None):
    return {"inherited_from": {"source": donor, "ability": ability or {"face": 0, "paragraph": 1}},
            "granted_by": {"source": grant, "ability": {"face": 0, "paragraph": 1}}}


def family(donor=22, by=11):
    return {"donor": donor, "donor_origin": "own", "definition": {"card": 2, "face": "Whole"},
            "grant": {"by": by, "ability": 0, "origin": "own", "stamp": 100,
                      "part": 0, "part_in_ability": [0, 0],
                      "definition": {"card": 1, "face": "Whole"}}}


class InheritedReferenceTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.oracle = Oracle()
        # Use the pinned full Oracle identity rather than deriving expected text from the engine.
        cls.elves = next(key for key, faces in cls.oracle.faces.items()
                         if faces[0].name == "Llanowar Elves")
        schema = load_schema("mgn-1.schema.json")
        cls.validator = Validator({"$defs": schema["$defs"], "$ref": "#/$defs/ability_ref"})

    def state(self):
        return State(seats=["A", "B"], tier=1,
                     cards={"elves1": {"oracle": self.elves}, "marvin1": {"oracle": MARVIN},
                            "thranduil1": {"oracle": THRANDUIL}},
                     turn={}, pending=None, result={},
                     zones={"battlefield": ["elves1", "marvin1", "thranduil1"]})

    def test_recursive_schema_and_native_identifiers_refused(self):
        ref = acquired("marvin1", "thranduil1", acquired())
        self.assertEqual(self.validator.errors(ref), [])
        for mutate in (lambda r: r.update(index=3),
                       lambda r: r["inherited_from"].update(stamp=100),
                       lambda r: r["granted_by"].update(source="unknown native id"),
                       lambda r: r.update(inherited_from={"rule": "305.6"}),
                       lambda r: r.pop("granted_by")):
            bad = deepcopy(ref)
            mutate(bad)
            self.assertTrue(self.validator.errors(bad))

    def test_helper_retains_both_frozen_definitions_without_leaking_native_identity(self):
        calls = []
        def terminal(source, index, origin, definition):
            calls.append((source, index, origin, definition))
            return {"face": 0, "paragraph": index + 1}
        native = family()
        result = inherited_ability_ref(native, 1, object_ref=lambda n: {22: "elves1", 11: "marvin1"}[n],
                                       ability_ref=terminal)
        self.assertEqual(result, acquired(ability={"face": 0, "paragraph": 2}))
        self.assertEqual(calls, [(22, 1, "own", native["definition"]),
                                 (11, 0, "own", native["grant"]["definition"])])
        calls[0][3]["card"] = 99
        self.assertEqual(native["definition"]["card"], 2)

    def test_helper_recurses_donor_and_grant_origins(self):
        native = family(33, 44)
        native["donor_origin"] = family(22, 11)
        native["grant"]["origin"] = family(66, 55)
        ref = inherited_ability_ref(native, 0, object_ref=lambda n: f"card{n}",
                                    ability_ref=lambda *_: {"face": 0, "paragraph": 1})
        self.assertEqual(ability_object_refs(ref), ["card33", "card22", "card11", "card44", "card66", "card55"])
        self.assertEqual(len(_ability_refs({"source": "card77", "ability": ref})), 4)

    def test_helper_propagates_frozen_definition_refusal(self):
        def refuse(*_):
            raise ValueError("frozen donor differs from authenticated text")
        with self.assertRaisesRegex(ValueError, "frozen donor"):
            inherited_ability_ref(family(), 0, object_ref=str, ability_ref=refuse)
        native = family()
        native["grant"]["by"] = None
        with self.assertRaisesRegex(ValueError, "record identities"):
            inherited_ability_ref(native, 0, object_ref=str, ability_ref=refuse)
        native = family()
        native["donor_origin"] = native
        with self.assertRaisesRegex(ValueError, "cyclic"):
            inherited_ability_ref(native, 0, object_ref=str, ability_ref=refuse)

    def test_donor_text_classifies_mana_ability_for_both_acquirers(self):
        for source in ("marvin1", "thranduil1"):
            ref = acquired(grant=source)
            self.assertEqual(_ability_text(source, ref, self.state(), self.oracle), "{T}: Add {G}.")
            problems = []
            action = {"kind": "activate", "source": source, "ability": ref}
            _check_mana_ability(action, self.state(), self.oracle, "/do", problems)
            self.assertEqual([p.code for p in problems], ["ability"])
            action["adding"] = {"G": 1}
            problems = []
            _check_mana_ability(action, self.state(), self.oracle, "/do", problems)
            self.assertEqual(problems, [])

    def test_every_nested_paragraph_is_validated(self):
        for branch in ("inherited_from", "granted_by"):
            ref = acquired("marvin1", "thranduil1", acquired())
            ref["inherited_from"]["ability"][branch]["ability"]["paragraph"] = 99
            problems = []
            _check_abilities({"source": "thranduil1", "ability": ref}, self.state(), self.oracle, "/do", problems)
            self.assertEqual(len(problems), 1)
            self.assertIn("99", problems[0].message)

    def test_unknown_donor_and_grant_checked_in_actions_and_stack_facts(self):
        ref = acquired("missing1", "missing2")
        self.assertEqual(State._object_refs("ability", ref), ["missing1", "missing2"])
        problems = []
        _check_references({"kind": "activate", "source": "marvin1", "ability": ref}, self.state(), "/do", problems)
        self.assertEqual([p.code for p in problems], ["unknown_reference", "unknown_reference"])
