"""Rules: 611.2a/c, 603.7a/c/d, 400.7j, 514.2. Independent initial causes."""
from copy import deepcopy
import os
from pathlib import Path
import unittest

from mgn.adapters.karn import KarnAdapter, Refusal
from mgn.examples import EXAMPLES, load
from mgn.oracle import Oracle
from mgn.state import State, zone_names
from mgn.validate import validate

ORACLE = Oracle()
ROOT = Path(__file__).resolve().parents[2]
ENGINE = Path(os.environ.get("KARN_CONFORMANCE_ENGINE", ROOT / "target/debug/game-serve"))


def record(delayed=False, step="postcombat_main"):
    result = load(EXAMPLES / "priority-and-steps.json")
    result["tier"] = 2
    names = {"spell": ("Long Road Home" if delayed else "Giant Growth", "A"), "bear": ("Grizzly Bears", "B"), "draw": ("Forest", "B")}
    result["cards"] = {key: {"owner": owner, "oracle": ORACLE.by_name(name), "name": name} for key, (name, owner) in names.items()}
    zones = {zone: [] for zone in zone_names(["A", "B"])}
    zones["A.graveyard"] = ["spell"]
    zones["B.library"] = ["draw"]
    zones["exile" if delayed else "battlefield"] = ["bear"]
    oid = result["cards"]["spell"]["oracle"]
    cause = {"objects": {"old_spell": {"card": "spell", "definition": {"oracle": oid, "face": 0}, "zone": "stack"}},
        "producers": [{"id": "resolution", "source": "old_spell", "ability": {"face": 0, "paragraph": 1}, "controller": "A"}],
        "continuous_effects": [], "delayed_triggers": [], "casts_this_turn":[{"source":"old_spell","player":"A"}]}
    if delayed:
        cause["delayed_triggers"] = [{"id": "return_card", "producer": "resolution", "clause": 1,
            "retained": [{"kind": "exiled", "values": [{"object": "bear"}]}],
            "when": {"kind": "step", "step": "end", "whose": "any", "next": True}, "once": True, "duration": {"kind": "indefinite"}}]
    else:
        cause["continuous_effects"] = [{"id": "growth", "producer": "resolution", "subjects": [{"object": "bear"}],
            "changes": [{"kind": "add_pt", "power": 3, "toughness": 3}], "duration": {"kind": "end_of_turn", "turn": 3}}]
    result["start"] = {"kind": "position", "state": {
        "turn": {"number": 3, "step": step, "active": "A"}, "pending": {"seat": "A", "kind": "priority"},
        "result": {"kind": "unfinished"}, "players": {"A": {"life": 20}, "B": {"life": 20}}, "zones": zones,
        "objects": {} if delayed else {"bear": {"controller": "B", "power": 5, "toughness": 5}}}, "causal": cause,
        "assumptions":{"spells_cast":{"A":1}}}
    result["transitions"] = []
    result.pop("checkpoints", None)
    return result


def setup(record):
    return {key: record[key] for key in ("tier", "pins", "game", "cards", "start")}


class CausalValidationTests(unittest.TestCase):
    def test_both_witnesses_are_portable_and_reference_checked(self):
        for delayed in (False, True):
            sample = record(delayed)
            self.assertEqual([str(p) for p in validate(sample, ORACLE) if p.severity == "error"], [])
            bad = deepcopy(sample)
            bad["start"]["causal"]["producers"][0]["source"] = "unknown"
            self.assertTrue(any(p.code == "unknown_reference" for p in validate(bad, ORACLE)))

    def test_wrong_duration_and_wrong_retained_role_are_errors(self):
        sample = record()
        sample["start"]["causal"]["continuous_effects"][0]["duration"]["turn"] = 2
        self.assertTrue(any(p.code == "coherence" for p in validate(sample, ORACLE)))
        sample = record(True)
        sample["start"]["causal"]["delayed_triggers"][0]["retained"][0]["values"] = [{"number": 3}]
        self.assertTrue(any(p.code == "coherence" for p in validate(sample, ORACLE)))

    def test_nested_ability_references_and_scope_quotations_are_checked(self):
        sample=record()
        sample["start"]["causal"]["producers"][0]["ability"]={
            "inherited_from":{"source":"missing","ability":{"face":0,"paragraph":1}},
            "granted_by":{"source":"bear","ability":{"face":0,"paragraph":1}}}
        self.assertTrue(any(p.code=="unknown_reference" for p in validate(sample,ORACLE)))
        sample=record()
        sample["start"]["causal"]["continuous_effects"][0]["subjects"]=[{"scope":"invented scope"}]
        self.assertTrue(any(p.code=="coherence" for p in validate(sample,ORACLE)))

    def test_cast_inventory_must_agree_with_explicit_count(self):
        sample=record()
        sample["start"]["assumptions"]["spells_cast"]["A"]=0
        self.assertTrue(any(p.code=="coherence" for p in validate(sample,ORACLE)))

    def test_noncard_current_alias_and_already_departed_token_are_portable(self):
        sample=record()
        token={"name":"Goblin","types":["creature"],"colors":["red"],"power":1,"toughness":1}
        sample["start"]["state"]["zones"]["battlefield"].append("T1")
        sample["start"]["state"]["objects"]["T1"]={"owner":"A","controller":"A","token":token}
        full={**token,"power":"1","toughness":"1","mana_cost":None,"color_indicator":[],"supertypes":[],"subtypes":[],"text":""}
        sample["start"]["causal"]["objects"]["saved_token"]={"owner":"A","definition":full,"zone":"battlefield","current":"T1"}
        self.assertEqual([str(p) for p in validate(sample,ORACLE) if p.severity=="error"],[])
        sample=record()
        token["text"]="{0}: Target creature gets +3/+3 until end of turn."
        sample["start"]["state"]["departed"]={"T1":{"owner":"A","token":token}}
        sample["start"]["causal"]["producers"][0]["source"]="T1"
        self.assertEqual([str(p) for p in validate(sample,ORACLE) if p.severity=="error"],[])

    def test_current_alias_preserves_identity_with_distinct_frozen_producing_definitions(self):
        from mgn.causal import check_causal
        for explicit in (False,True):
            sample=record()
            state=State.from_record(sample,[])
            oid=sample["cards"]["spell"]["oracle"]
            state.zones["stack"].append("S1")
            state.objects["S1"]={"kind":"copy","owner":"A","controller":"A","copy_of":"spell"}
            state.copy_bases["S1"]=(oid,0)
            if explicit:
                state.objects["S1"]["copy_definition"]={"oracle":oid,"face":0}
            sample["start"]["causal"]["objects"]["saved_copy"]={"owner":"A","definition":{"oracle":oid,"face":0},"zone":"stack","current":"S1"}
            problems=[]
            check_causal(sample,state,ORACLE,problems)
            self.assertEqual(problems,[])
            # A source can copy/transform without becoming a new object (400.7).
            # A second occurrence freezes its own producing definition.
            other=ORACLE.by_name("Long Road Home")
            sample["start"]["causal"]["objects"]["later_copy"]={"owner":"A","definition":{"oracle":other,"face":0},"zone":"stack","current":"S1"}
            problems=[]
            check_causal(sample,state,ORACLE,problems)
            self.assertEqual(problems,[])
            sample["start"]["causal"]["objects"]["later_copy"]["owner"]="B"
            problems=[]
            check_causal(sample,state,ORACLE,problems)
            self.assertTrue(any(p.code=="coherence" for p in problems))


@unittest.skipUnless(ENGINE.is_file(), "build worker game-serve")
class CausalAdapterTests(unittest.TestCase):
    def setUp(self):
        self.adapter = KarnAdapter(ENGINE, ORACLE)

    def tearDown(self):
        self.adapter.end()

    def test_growth_readback_and_cleanup(self):
        sample = record(step="end")
        reply = self.adapter.setup(setup(sample))
        self.assertEqual(reply["causal"], sample["start"]["causal"])
        self.assertEqual(reply["state"]["objects"]["bear"]["power"], 5)
        reply = self.adapter.default(None, lambda _: None)
        reply = self.adapter.default(None, lambda _: None)
        self.assertEqual(reply["state"]["objects"]["bear"]["power"], 2)
        self.assertEqual(reply["causal"]["continuous_effects"], [])

    def test_delay_return_uses_historical_source_and_does_not_repeat_current_end(self):
        sample = record(True, step="end")
        reply = self.adapter.setup(setup(sample))
        self.assertEqual(reply["causal"], sample["start"]["causal"])
        self.assertEqual(reply["state"]["zones"]["stack"], [])
        reply = self.adapter.default({"turn": 4, "step": "end", "active": "B", "seat": "B"}, lambda _: None)
        self.assertEqual(reply["state"]["turn"]["number"], 4)
        self.assertEqual(len(reply["state"]["zones"]["stack"]), 1)
        stack = reply["state"]["zones"]["stack"][0]
        self.assertEqual(reply["state"]["objects"][stack]["source"], "old_spell")
        reply = self.adapter.default(None, lambda _: None)
        reply = self.adapter.default(None, lambda _: None)
        self.assertEqual(reply["state"]["zones"]["battlefield"], ["bear"])
        self.assertEqual(reply["state"]["objects"]["bear"]["counters"], {"+1/+1": 1})
        self.assertEqual(reply["causal"]["delayed_triggers"], [])

    def test_new_resolved_effect_after_import_is_a_valid_continuation(self):
        sample=record()
        sample["cards"]["another"] = deepcopy(sample["cards"]["spell"])
        sample["start"]["state"]["zones"]["A.hand"] = ["another"]
        sample["start"]["state"]["players"]["A"]["mana"] = {"G":1}
        self.adapter.setup(setup(sample))
        self.adapter.act(1,"A",{"kind":"cast","card":"another","targets":[["bear"]],"pay":{"pool":{"G":1}}},lambda _:None)
        self.adapter.default(None,lambda _:None)
        reply=self.adapter.default(None,lambda _:None)
        self.assertEqual(reply["state"]["objects"]["bear"]["power"],8)
        self.assertIn("causal.new_occurrences",reply.get("unsupported",[]))

    def test_current_alias_binds_one_retained_identity_and_timestamp_aliases_compose(self):
        for delayed in (False,True):
            sample=record(delayed,step="end")
            cause=sample["start"]["causal"]
            cause["objects"]["saved_bear"]={"card":"bear","definition":{"oracle":sample["cards"]["bear"]["oracle"],"face":0},
                "zone":"exile" if delayed else "battlefield","current":"bear"}
            if delayed:
                cause["delayed_triggers"][0]["retained"][0]["values"]=[{"object":"saved_bear"}]
            else:
                cause["timestamp_order"]=[{"object":"bear"},{"effect":"growth"}]
            reply=self.adapter.setup(setup(sample))
            self.assertEqual(reply["causal"],cause)
