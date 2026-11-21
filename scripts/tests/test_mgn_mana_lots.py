"""MGN 5.8 counted lots preserve CR 106.6 restrictions, riders and chosen conditions."""
from copy import deepcopy
import unittest

from mgn.canonical import sha256
from mgn.lots import normalize_lots
from mgn.runner import Run, compare
from mgn.state import State, zone_names


CREATURE = {"type": "G", "restriction": "{T}: Add {G}. Spend this mana only to cast a creature spell."}
CHOSEN = {"type": "G", "restriction": "{T}: Add one mana of any color. Spend this mana only to cast a creature spell of the chosen type.",
          "producer": {"source": "land", "ability": {"face": 0, "paragraph": 2}}, "chosen": {"creature_type": "Elf"}}


def projection(lots):
    return State(seats=["A", "B"], tier=2, cards={},
                 turn={"number": 3, "step": "precombat_main", "active": "A"},
                 pending={"seat": "A", "kind": "priority"}, result={"kind": "unfinished"},
                 players={"A": {"life": 20, "mana_restricted": deepcopy(lots)}, "B": {"life": 20}},
                 zones={zone: [] for zone in zone_names(["A", "B"])})


class ManaLotTests(unittest.TestCase):
    def test_order_and_split_counts_have_one_explicit_state_and_hash(self):
        # Three identical creature-only units and one differently conditioned unit
        # have the same meaning regardless of the engine's pool enumeration.
        split = [{**CREATURE, "count": 1}, {**CHOSEN, "count": 1}, {**CREATURE, "count": 2}]
        joined = [{**CHOSEN, "count": 1}, {**CREATURE, "count": 3}]
        state = projection(split)
        before = deepcopy(state.players)
        self.assertEqual(state.explicit(), projection(joined).explicit())
        self.assertEqual(sha256(state.explicit()), sha256(projection(joined).explicit()))
        self.assertEqual(state.players, before)

    def test_compare_accepts_unsorted_split_adapter_lots(self):
        expected = projection([{**CREATURE, "count": 3}, {**CHOSEN, "count": 1}]).explicit()
        actual = deepcopy(expected)
        actual["players"]["A"]["mana_restricted"] = [
            {**CHOSEN, "count": 1}, {**CREATURE, "count": 2}, {**CREATURE, "count": 1}]
        frozen = deepcopy(actual)
        self.assertEqual(compare(expected, actual, [], 2), ([], []))
        self.assertEqual(actual, frozen)

    def test_changed_count_terms_producer_and_chosen_each_remain_differences(self):
        original = {**CHOSEN, "count": 2}
        expected = projection([original]).explicit()
        changes = [
            {"count": 1},
            {"restriction": CHOSEN["restriction"] + " That spell can't be countered."},
            {"producer": {"source": "other", "ability": {"face": 0, "paragraph": 2}}},
            {"producer": {"source": "land", "ability": {"face": 0, "paragraph": 3}}},
            {"chosen": {"creature_type": "Goblin"}},
        ]
        for change in changes:
            with self.subTest(change=change):
                actual = projection([{**original, **change}]).explicit()
                differences, inconclusive = compare(expected, actual, [], 2)
                self.assertEqual([(d["level"], d["path"]) for d in differences], [("L4", "players.A.mana_restricted")])
                self.assertEqual(inconclusive, [])
                self.assertNotEqual(sha256(expected), sha256(actual))
        self.assertEqual(len(normalize_lots([{**original, **change} for change in changes])), len(changes))

    def test_invalid_counts_are_not_repaired_by_aggregation(self):
        for count in (0, -1, True):
            lots = [{**CREATURE, "count": count}, {**CREATURE, "count": 2}]
            self.assertEqual(normalize_lots(lots), lots)

    def test_live_checkpoint_path_hashes_semantic_lots(self):
        # A protocol stub isolates the checkpoint calculation from native rules.
        # These two complete descriptors need no producer disambiguation.
        red = {"type": "R", "restriction": "{T}: Add {R}. Spend this mana only to cast a creature spell."}
        expected = projection([{**CREATURE, "count": 3}, {**red, "count": 1}]).explicit()
        actual = deepcopy(expected)
        actual["players"]["A"]["mana_restricted"] = [
            {**red, "count": 1}, {**CREATURE, "count": 1}, {**CREATURE, "count": 2}]
        record = {"tier": 2, "pins": {}, "game": {"seats": ["A", "B"]}, "cards": {},
                  "start": {"kind": "position", "state": expected},
                  "transitions": [{"seat": "A", "do": {"kind": "pass"}, "delta": {}}],
                  "checkpoints": {"1": sha256(expected)}}

        class Adapter:
            def request(self, body, on_random=None):
                if "hello" in body:
                    return {"capabilities": {"engine": {}, "adapter": {}, "tiers": [2]}}
                return {"ok": {"state": deepcopy(actual)}}

        run = Run(record, None, [])
        run._drive(Adapter(), {})
        self.assertEqual(run.replayed, 1)
        self.assertEqual(run.divergences, [])
        self.assertEqual(run.levels["L5"], "pass")
