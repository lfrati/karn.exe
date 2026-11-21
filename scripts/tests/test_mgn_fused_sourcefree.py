"""CR 702.102/709.4d and 725.2: combined split spells and source-free rules."""
from copy import deepcopy
from pathlib import Path
import unittest

from mgn.examples import EXAMPLES, load
from mgn.oracle import Oracle
from mgn.state import State, zone_names
from mgn.validate import validate
from mgn.schema import Validator, load_schema
from mgn.adapters.karn import KarnAdapter, Refusal

O = Oracle()
FAR = O.by_name("Far // Away")
BEAR = O.by_name("Grizzly Bears")


class FusedTests(unittest.TestCase):
    def record(self):
        r = load(EXAMPLES / "priority-and-steps.json")
        r["tier"] = 2
        r.pop("checkpoints", None)
        r["cards"] = {"spell": {"owner": "A", "oracle": FAR, "name": O.name(FAR)},
                      "bear": {"owner": "B", "oracle": BEAR, "name": O.name(BEAR)}}
        zones = {z: [] for z in zone_names(["A", "B"])}
        zones["A.hand"], zones["battlefield"] = ["spell"], ["bear"]
        r["start"] = {"kind": "position", "state": {
            "turn": {"number": 3, "step": "precombat_main", "active": "A"},
            "pending": {"seat": "A", "kind": "priority"}, "result": {"kind": "unfinished"},
            "players": {"A": {"life": 20, "mana": {"U": 1, "B": 1, "C": 3}}, "B": {"life": 20}},
            "zones": zones, "objects": {}}}
        r["transitions"] = [{"seat": "A", "do": {"kind": "cast", "card": "spell", "fused": True,
            "targets": [["bear"], ["B"]], "pay": {"pool": {"U": 1, "B": 1, "C": 3}}},
            "delta": {"objects": {"spell": {"zone": "stack", "controller": "A", "fused": True,
                                               "targets": [["bear"], ["B"]]}},
                      "players": {"A": {"mana": {}}}, "pending": {"seat": "B", "kind": "priority"}}}]
        return r

    def test_both_halves_one_spell_and_one_payment(self):
        # Far returns target creature; Away makes target player sacrifice. Their
        # left-to-right cost is {1}{U}{2}{B}, one blue/black instant on stack.
        r = self.record()
        self.assertEqual(validate(r, O), [])
        problems = []
        s = State.from_record(r, problems)
        s.apply(r["transitions"][0]["delta"], [], "/cast", problems)
        self.assertEqual(s._printed("spell", O)["colors"], ["black", "blue"])
        self.assertEqual(s._printed("spell", O)["types"], ["instant"])
        self.assertTrue(s.explicit(O)["objects"]["spell"]["fused"])
        s.apply({"objects": {"S1": {"zone": "stack", "kind": "copy", "copy_of": "spell", "controller": "B", "fused": True}}}, [], "/copy", problems)
        s.apply({"objects": {"spell": {"zone": "A.graveyard"}}}, [], "/resolve", problems)
        self.assertEqual(s.copy_bases["S1"], (FAR, "fused"))
        self.assertEqual(s._printed("S1", O)["colors"], ["black", "blue"])
        self.assertEqual(problems, [])

    def test_invalid_card_zone_face_or_lost_selection_refuses(self):
        for mutate in (lambda r: r["transitions"][0]["do"].update(face=1),
                       lambda r: r["cards"]["spell"].update(oracle=BEAR, name=O.name(BEAR)),
                       lambda r: r["start"]["state"]["zones"].update({"A.hand": [], "A.graveyard": ["spell"]}),
                       lambda r: r["transitions"][0]["delta"]["objects"]["spell"].pop("fused")):
            r = self.record(); mutate(r)
            self.assertTrue(validate(r, O))

    def test_copies_cannot_add_fused_selection_to_single_half(self):
        r = self.record()
        del r["transitions"][0]["do"]["fused"]
        del r["transitions"][0]["delta"]["objects"]["spell"]["fused"]
        r["transitions"].append({"seat": "B", "do": {"kind": "concede"}, "delta": {
            "objects": {"S1": {"zone": "stack", "kind": "copy", "copy_of": "spell", "controller": "A", "fused": True}},
            "pending": None, "result": {"kind": "win", "winner": "A", "cause": "concession"}}})
        self.assertTrue(any("cannot add fuse" in p.message for p in validate(r, O)))
        r = self.record(); r["transitions"][0]["do"]["fused"] = False
        self.assertTrue(any(p.code == "schema" for p in validate(r, O)))

    def test_adapter_emits_fused_native_choice(self):
        a = KarnAdapter(Path("/unused"), O)
        a.cards = self.record()["cards"]
        a._engine_id = lambda _: 12
        a._groups = lambda g: g
        a._announcement = lambda *args: None
        a._casting_payment = lambda *args: None
        self.assertEqual(a._move({"kind": "cast", "card": "spell", "fused": True})["cast_as"], "fused")


class SourceFreeTests(unittest.TestCase):
    def test_source_free_stack_ability_in_full_initial_state_and_delta(self):
        # CR 725.2-3: monarch triggers have a controller but no source. Test
        # the whole record schema, not merely the pending ability-instance form.
        r = load(EXAMPLES.parents[2] / "tests/fixtures/mgn-drafts/monarch-controller-timing.json")
        self.assertEqual(validate(r, O), [])
        with_source = deepcopy(r)
        with_source["start"]["state"]["objects"]["S1"]["source"] = "thorn"
        self.assertTrue(validate(with_source, O))
        ordinary = deepcopy(r)
        ordinary["start"]["state"]["objects"]["S1"]["ability"] = {"face": 0, "paragraph": 2}
        self.assertTrue(validate(ordinary, O))

        # The same requirement applies when the source-free trigger is first
        # introduced in a transition, and ordinary sourced abilities still
        # cannot omit their source. The premise itself is independently valid.
        r = FusedTests().record()
        trigger = {"zone": "stack", "kind": "ability",
                   "ability": {"rule": "725.2", "sub": 1}, "controller": "A"}
        r["transitions"][0]["delta"]["objects"]["S1"] = trigger
        r["transitions"][0]["delta"]["zones"] = {"stack": {"top": ["S1", "spell"]}}
        self.assertEqual(validate(r, O), [])
        trigger["source"] = "bear"
        self.assertTrue(validate(r, O))
        trigger.pop("source")
        trigger["ability"] = {"face": 0, "paragraph": 1}
        self.assertTrue(validate(r, O))

    def test_only_explicit_rules_omit_source_and_never_invent_one(self):
        v = Validator(load_schema("mgn-1.schema.json"))
        schema = {"$ref": "#/$defs/ability_instance"}
        for ability in ({"rule": "725.2", "sub": 1}, {"rule": "725.2", "sub": 2},
                        {"rule": "726.2", "sub": 3}, {"rule": "702.179d"}):
            self.assertEqual(v.errors({"ability": ability}, schema), [])
            self.assertTrue(v.errors({"source": "bear", "ability": ability}, schema))
        for ability in ({"rule": "725.2"}, {"rule": "725.2", "sub": 3}, {"rule": "305.6"}):
            self.assertTrue(v.errors({"ability": ability}, schema))

    def test_two_monarch_damage_instances_keep_subjects_and_event_controller(self):
        a = KarnAdapter(Path("/unused"), O)
        a._target_record = lambda t: "left" if t["object"] == 11 else "right"
        pending = lambda n: {"ability": {"rule_ref": {"rule": "725.2", "sub": 2}},
                             "controller": 0, "bound": {"inputs": [{"kind": "subject", "subjects": [{"object": n}]}]}}
        left = a._instance_from_prompt({"source": None}, {}, pending(11))
        right = a._instance_from_prompt({"source": None}, {}, pending(12))
        self.assertEqual(left, {"ability": {"rule": "725.2", "sub": 2}, "for": ["left"]})
        self.assertFalse(a._same_instance(left, right))
        with self.assertRaises(Refusal):
            a._instance_from_prompt({"source": None, "text": "Whenever a creature deals combat damage to you, its controller becomes the monarch."}, {}, None)
        r = FusedTests().record(); r["transitions"] = []
        s = State.from_record(r, [])
        errors = []
        s.apply({"objects": {"S1": {"zone": "stack", "kind": "ability", "ability": {"rule": "725.2", "sub": 2}, "controller": "A"}}}, [], "/rule", errors)
        self.assertEqual(errors, [])
        self.assertNotIn("owner", s.explicit(O)["objects"]["S1"])
        self.assertNotIn("source", s.explicit(O)["objects"]["S1"])
        errors = []
        s.apply({"objects": {"S1": {"source": "bear"}}}, [], "/fabricated-source", errors)
        self.assertTrue(errors)
        with self.assertRaises(Refusal):
            a._row_index("bear", {"rule": "725.2", "sub": 2})
