"""The schema interpreter cross-checked against the required `jsonschema` package."""
import unittest

from mgn.examples import negative_cases, load, positive_paths
from mgn.schema import Validator, load_schema, validate

import jsonschema


class InterpreterTests(unittest.TestCase):
    def test_cast_substitutions_and_permission_are_distinct_choices(self):
        # CR 702.51: a multicolored creature may pay one colored or generic
        # component; 702.180's optional tap is specific to harmonize.
        schema = load_schema("mgn-1.schema.json")
        validator = Validator(schema)
        def errors(action):
            return validator.errors(action, schema["$defs"]["cast_fields"])
        good = {"kind": "cast", "card": "spell",
                "permission": {"source": "permit", "ability": {"face": 0, "paragraph": 1}},
                "convoke": [{"object": "bear", "mana": "G"}, {"object": "elf", "mana": None}]}
        self.assertEqual(errors(good), [])
        for convoke in (["bear"], [{"object": "bear"}], [{"object": "bear", "mana": "C"}]):
            self.assertTrue(errors({**good, "convoke": convoke}))
        for keyword, valid in (("harmonize", True), ("flashback", False)):
            action = {"kind": "cast", "card": "spell", "cost": {
                "kind": "alternative", "keyword": keyword, "tapped": "bear"}}
            self.assertEqual(not errors(action), valid)

    def test_improvise_lists_artifact_identities_and_prevents_resource_reuse(self):
        # 702.126a/118.10: MGN records the artifacts, not an unauthenticated count.
        schema = load_schema("mgn-1.schema.json")
        validator = Validator(schema)
        action = {"kind": "cast", "card": "spell", "improvise": ["artifact", "other"]}
        self.assertEqual(validator.errors(action, schema["$defs"]["cast_fields"]), [])
        for invalid in (2, ["artifact", "artifact"], [{"object": "artifact"}]):
            self.assertTrue(validator.errors({**action, "improvise": invalid}, schema["$defs"]["cast_fields"]))

    def test_cast_resource_references_are_live_and_not_duplicated(self):
        from mgn.validate import _check_references
        from types import SimpleNamespace
        state = SimpleNamespace(seats=["A", "B"], cards={},
                                exists=lambda obj: obj in {"spell", "bear"}, known=lambda obj: obj == "departed")
        action = {"kind": "cast", "card": "spell", "convoke": [
            {"object": "bear", "mana": "G"}, {"object": "bear", "mana": None},
            {"object": "departed", "mana": "G"}],
            "permission": {"source": "departed", "ability": {"face": 0, "paragraph": 1}}}
        errors = []
        _check_references(action, state, "/do", errors)
        self.assertEqual([(error.code, error.path) for error in errors], [
            ("coherence", "/do/convoke"), ("unknown_reference", "/do/convoke/2/object")])
        # A resolving effect can grant permission beyond its source's lifetime
        # (611.2a). An unknown source still supplies no such provenance.
        action["convoke"] = [{"object": "bear", "mana": "G"}]
        action["permission"]["source"] = "unknown"
        errors = []
        _check_references(action, state, "/do", errors)
        self.assertEqual([(error.code, error.path) for error in errors], [
            ("unknown_reference", "/do/permission/source")])

    def test_no_maximum_hand_size_is_distinct_from_zero(self):
        # CR 402.2: effects can remove a maximum rather than setting it to zero.
        schema = load_schema("mgn-1.schema.json")
        validator = Validator(schema)
        for maximum in (None, 0, 7):
            self.assertEqual(validator.errors({"max_hand_size": maximum}, schema["$defs"]["player_facts"]), [])
        self.assertTrue(validator.errors({"max_hand_size": -1}, schema["$defs"]["player_facts"]))

    def test_positive_examples_pass_the_schema(self):
        for path in positive_paths():
            with self.subTest(example=path.name):
                self.assertEqual([str(e) for e in validate(load(path))], [])

    def test_unknown_kind_names_the_alternatives(self):
        record = load(positive_paths()[0])
        record["transitions"][0]["do"] = {"kind": "ponder"}
        errors = validate(record)
        self.assertEqual(len(errors), 1)
        self.assertIn("unknown kind", errors[0].message)
        self.assertIn("play_land", errors[0].message)
        self.assertEqual(errors[0].path, "/transitions/0/do/kind")

    def test_discriminated_branch_errors_are_precise(self):
        record = load(positive_paths()[0])
        record["transitions"][0]["do"] = {"kind": "play_land", "cards": "plains2"}
        messages = [str(e) for e in validate(record)]
        self.assertTrue(any("missing required member 'card'" in m for m in messages), messages)
        self.assertTrue(any("unknown member 'cards'" in m for m in messages), messages)

    def test_keyword_semantics(self):
        v = Validator({"$defs": {"n": {"type": "integer", "minimum": 1}},
                       "type": "object",
                       "properties": {"a": {"$ref": "#/$defs/n"}, "b": {"enum": [1, "x"]}},
                       "patternProperties": {"^x-": True},
                       "additionalProperties": False,
                       "required": ["a"],
                       "if": {"required": ["b"], "properties": {"b": {"const": 1}}}, "then": {"required": ["c"]}})
        self.assertEqual(v.errors({"a": 2, "x-note": 5}), [])
        self.assertEqual([e.keyword for e in v.errors({"a": 0})], ["minimum"])
        self.assertEqual([e.keyword for e in v.errors({"a": 1.0})], [])  # 1.0 is an integer in JSON Schema
        self.assertEqual([e.keyword for e in v.errors({"a": True})], ["type"])
        self.assertEqual([e.keyword for e in v.errors({"a": 1, "zz": 1})], ["additionalProperties"])
        self.assertEqual([e.keyword for e in v.errors({"a": 1, "b": 1})], ["required"])
        self.assertEqual([e.keyword for e in v.errors({"a": 1, "b": 2})], ["enum"])
        self.assertEqual([e.keyword for e in v.errors({})], ["required"])
        items = Validator({"type": "array", "prefixItems": [{"type": "string"}], "items": {"type": "integer"},
                           "uniqueItems": True, "minItems": 2})
        self.assertEqual(items.errors(["a", 1, 2]), [])
        self.assertEqual([e.keyword for e in items.errors(["a", "b"])], ["type"])
        self.assertEqual([e.keyword for e in items.errors(["a"])], ["minItems"])
        self.assertEqual([e.keyword for e in items.errors(["a", 1, 1])], ["uniqueItems"])


class CrossCheckTests(unittest.TestCase):
    def test_schema_is_a_valid_2020_12_schema(self):
        schema = load_schema("mgn-1.schema.json")
        jsonschema.Draft202012Validator.check_schema(schema)

    def test_agreement_on_examples_and_negative_cases(self):
        schema = load_schema("mgn-1.schema.json")
        reference = jsonschema.Draft202012Validator(schema)
        documents = [(path.name, load(path)) for path in positive_paths()]
        documents += [(case["name"], case["record"]) for case in negative_cases()]
        for name, document in documents:
            with self.subTest(document=name):
                ours = not validate(document)
                theirs = reference.is_valid(document)
                self.assertEqual(ours, theirs)


class RecursivePaymentSchemaTests(unittest.TestCase):
    def setUp(self):
        self.schema = load_schema("mgn-1.schema.json")
        self.validator = Validator(self.schema)

    def payment_errors(self, payment):
        return self.validator.errors(payment, self.schema["$defs"]["payment"])

    def test_recursive_payment_reuses_activation_and_semantic_lot_vocabulary(self):
        lot = {"type": "G", "count": 1, "restriction": "Spend this mana only to cast a creature spell of the chosen type.",
               "producer": {"source": "grove", "ability": {"face": 0, "paragraph": 1}}, "chosen": {"creature_type": "Elf"}}
        nested = {"pool_restricted": [lot], "allocation": [{"symbol": 0, "mana": "G", "restricted": {k: v for k, v in lot.items() if k != "count"}}]}
        for index in range(4):
            nested = {"abilities": [{"source": f"filter{index}", "ability": {"face": 0, "paragraph": 1}, "adding": ["G"],
                                     "x": index, "additional": {"choices": [1]}, "pay": nested}]}
        self.assertEqual(self.payment_errors(nested), [])
        self.assertEqual(self.validator.errors({"mana_restricted": [lot]}, self.schema["$defs"]["player_facts"]), [])

    def test_recursive_payment_rejects_native_indices_and_unknown_nested_members(self):
        for payment in [
            {"pool_restricted": [{"type": "G", "count": 1, "restriction": "Creature spells only.", "index": 0}]},
            {"abilities": [{"source": "filter", "ability": {"engine_row": 3}, "adding": ["G"]}]},
            {"abilities": [{"source": "filter", "ability": {"face": 0, "paragraph": 1}, "adding": ["G"], "pay": {"engine_objects": [1]}}]},
            {"allocation": [{"symbol": 0, "mana": "G", "restricted": {"type": "G", "count": 1, "restriction": "Creature spells only."}}]},
        ]:
            with self.subTest(payment=payment):
                self.assertTrue(self.payment_errors(payment))


if __name__ == "__main__":
    unittest.main()
