"""CR 106.4/106.6, 113.7a: retained provenance is not a live object choice."""
from copy import deepcopy
import unittest

from mgn.examples import EXAMPLES, load
from mgn.oracle import Oracle
from mgn.state import State
from mgn.validate import _check_references, validate


def inherited(donor, grant):
    return {"inherited_from": {"source": donor, "ability": {"face": 0, "paragraph": 1}},
            "granted_by": {"source": grant, "ability": {"face": 0, "paragraph": 1}}}


def lot(source="elves", ability=None, player="A"):
    return {"type": "G", "count": 1, "restriction": "Spend this mana only to cast a creature spell.",
            "producer": {"source": source, "ability": ability or {"face": 0, "paragraph": 1}},
            "chosen": {"player": player}}


class ProvenanceReferenceTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.oracle = Oracle()

    def record(self):
        record = load(EXAMPLES / "tokens-counters-combat.json")
        record["transitions"] = []
        return record

    def test_review_counterexample_player_lot_rejected_by_full_validator(self):
        record = self.record()
        record["start"]["state"]["players"]["A"]["mana_restricted"] = [
            lot("unknown_producer", {"face": 0, "paragraph": 99}, "C")]
        problems = validate(record, self.oracle)
        unknown = [p for p in problems if p.code == "unknown_reference"]
        self.assertEqual(len(unknown), 2)
        self.assertTrue(any("producer" in p.path for p in unknown))
        self.assertTrue(any(p.path.endswith("chosen/player") for p in unknown))

    def test_delta_player_lot_checks_nested_provenance_and_chosen_seat(self):
        state = State.from_record(self.record(), [])
        delta = {"players": {"A": {"mana_restricted": [
            lot("elves", inherited("missing_donor", "missing_grant"), "C")]}}}
        problems = []
        state.apply(delta, [], "/delta", problems)
        unknown = [p for p in problems if p.code == "unknown_reference"]
        self.assertEqual(len(unknown), 3)
        self.assertTrue(all(p.path.startswith("/delta/players/A/mana_restricted/0/") for p in unknown))

    def test_full_and_delta_load_producers_before_checking_player_lots(self):
        record = self.record()
        given = record["start"]["state"]
        token = {"owner": "A", "controller": "A", "token": {
            "name": "Mana Elf", "types": ["creature"], "subtypes": ["Elf"], "colors": ["green"],
            "power": 1, "toughness": 1, "text": "{T}: Add {G}."}}
        given["zones"]["battlefield"].append("T1")
        given.setdefault("objects", {})["T1"] = token
        given["players"]["A"]["mana_restricted"] = [lot("T1")]
        problems = []
        State.from_record(record, problems)
        self.assertEqual(problems, [])
        state = State.from_record(self.record(), [])
        problems = []
        state.apply({"players": {"A": {"mana_restricted": [lot("T1")]}},
                     "objects": {"T1": {"zone": "battlefield", **token}}}, [], "/delta", problems)
        self.assertEqual(problems, [])
        # The pool's provenance survives the producing token (106.4, 106.6).
        state.apply({"objects": {"T1": None}}, [], "/later", problems)
        self.assertEqual(problems, [])
        record["start"]["state"]["zones"]["battlefield"].remove("T1")
        del record["start"]["state"]["objects"]["T1"]
        problems = []
        State.from_observation(record, problems, state)
        self.assertEqual(problems, [])

    def retained_state(self):
        state = State.from_record(self.record(), [])
        state.historical["T1"] = {"token": {"text": "{T}: Add {G}."}}
        return state

    def test_review_counterexample_retained_payment_producer_is_valid(self):
        state = self.retained_state()
        action = {"kind": "activate", "source": "elves", "ability": {"face": 0, "paragraph": 1},
                  "pay": {"pool_restricted": [lot("T1", inherited("T1", "T1"))]}}
        problems = []
        _check_references(action, state, "/do", problems)
        self.assertEqual(problems, [])

    def test_same_id_provenance_does_not_legalize_live_targets_sources_or_costs(self):
        for choice in ("targets", "source", "objects", "payment_source"):
            action = {"kind": "activate", "source": "elves", "ability": inherited("T1", "T1"),
                      "pay": {"pool_restricted": [lot("T1")]}}
            if choice == "objects":
                action["pay"]["objects"] = [["T1"]]
                expected_path = "/do/pay/objects/0/0"
            elif choice == "payment_source":
                action["pay"]["abilities"] = [{"source": "T1", "ability": inherited("T1", "T1"), "adding": ["G"]}]
                expected_path = "/do/pay/abilities/0/source"
            elif choice == "source":
                action["source"] = "T1"
                expected_path = "/do/source"
            else:
                action["targets"] = [["T1"]]
                expected_path = "/do/targets/0/0"
            problems = []
            _check_references(action, self.retained_state(), "/do", problems)
            self.assertEqual([(p.code, p.path) for p in problems], [("unknown_reference", expected_path)])

    def test_chosen_player_is_a_seat_even_if_the_identity_is_a_live_object(self):
        action = {"kind": "activate", "source": "elves", "ability": {"face": 0, "paragraph": 1},
                  "pay": {"pool_restricted": [lot(player="elves")]}}
        problems = []
        _check_references(action, self.retained_state(), "/do", problems)
        self.assertEqual(len(problems), 1)
        self.assertTrue(problems[0].path.endswith("chosen/player"))

    def test_review_counterexample_pending_nested_provenance_rejected(self):
        record = self.record()
        pending = {"seat": "A", "kind": "order_triggers", "about": {"abilities": [
            {"source": "elves", "ability": inherited("missing_donor", "missing_grant")},
            {"source": "elves", "ability": {"face": 0, "paragraph": 1}}]}}
        record["start"]["state"]["pending"] = pending
        problems = validate(record, self.oracle)
        self.assertEqual(len([p for p in problems if p.code == "unknown_reference"]), 2)
        state = State.from_record(self.record(), [])
        problems = []
        state.apply({"pending": pending}, [], "/delta", problems)
        self.assertEqual(len(problems), 2)
        pending = deepcopy(pending)
        pending["about"]["abilities"][0] = {"source": "T1", "ability": inherited("T1", "T1")}
        problems = []
        state = self.retained_state()
        state._check_pending(pending, "/pending", problems)
        _check_references({"kind": "order_triggers", "order": pending["about"]["abilities"]},
                          state, "/do", problems)
        self.assertEqual(problems, [])
