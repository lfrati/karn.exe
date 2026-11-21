"""Record and game identities: renaming cards or upgrading the tier keeps the game identity."""
from copy import deepcopy
import json
import unittest

from mgn.examples import load, positive_paths
from mgn.canonical import canonical
from mgn.identity import _relabel, _scan, game_identity, game_inputs, record_identity, relabeling
from mgn.validate import validate


def renamed(record, mapping):
    # Physical references can share their spelling with syntax such as "top".
    # Use occurrence-aware renaming, not a replacement of every JSON string.
    return _relabel(record, mapping)


class IdentityTests(unittest.TestCase):
    def test_physical_references_do_not_rename_syntax_or_placement_literals(self):
        mapping = {"source": "c1", "top": "c2", "actual": "c3", "name": "c4"}
        action = {"kind": "activate", "source": "actual", "ability": {"face": 0, "paragraph": 1}}
        order = []
        _scan(action, mapping, order)
        self.assertEqual(order, ["actual"], "member names do not count as appearances")
        self.assertEqual(_relabel(action, mapping), {**action, "source": "c3"})
        outcome = {"kind": "shuffle", "pile": ["top", "source"], "order": ["source", "top"],
                   "scope": {"fixed": [["top", "top"], ["source", "bottom"]]}}
        normalized = _relabel(outcome, mapping)
        self.assertEqual(normalized["scope"]["fixed"], [["c2", "top"], ["c1", "bottom"]])
        self.assertEqual(normalized["order"], ["c1", "c2"])
        delta = {"zones": {"A.graveyard": {"top": ["top"]}},
                 "objects": {"top": {"source": "source", "name": "name"}}}
        self.assertEqual(_relabel(delta, mapping), {"zones": {"A.graveyard": {"top": ["c2"]}},
            "objects": {"c2": {"source": "c1", "name": "name"}}})
        _scan(outcome, mapping, order)
        self.assertEqual(order, ["actual", "source", "top"])

    def test_canonical_card_labels_are_not_relabeled_twice(self):
        record = next(load(p) for p in positive_paths() if p.name == "priority-and-steps.json")
        # The second physical card's authored id collides with the first canonical
        # id. The canonical card table must still contain each physical card once.
        physical = record["transitions"][1]["do"]["card"]
        other = _relabel(record, {physical: "c1"})
        inputs = game_inputs(other)
        self.assertEqual(len(inputs["cards"]), len(record["cards"]))
        self.assertEqual(set(inputs["cards"]), set(relabeling(other).values()))
        self.assertEqual(game_identity(record), game_identity(other))

    def test_member_order_and_canonical_round_trip_preserve_game_identity(self):
        def reverse_members(value):
            if isinstance(value, dict):
                return {key: reverse_members(item) for key, item in reversed(list(value.items()))}
            if isinstance(value, list):
                return [reverse_members(item) for item in value]
            return value
        for path in positive_paths():
            record = load(path)
            reordered = reverse_members(record)
            with self.subTest(example=path.name):
                self.assertEqual(game_identity(record), game_identity(reordered))
                self.assertEqual(game_identity(record), game_identity(json.loads(canonical(record))))
        # An activation's payment sources and ability source compete for first
        # appearance; JSON object insertion order must never choose the labels.
        record = next(load(p) for p in positive_paths() if p.name == "priority-and-steps.json")
        record["transitions"][0]["do"] = {"source": "lions", "kind": "activate",
            "ability": {"face": 0, "paragraph": 1},
            "pay": {"abilities": [{"source": "plains1", "ability": {"rule": "305.6"}, "adding": ["W"]}]}}
        self.assertEqual(game_identity(record), game_identity(json.loads(canonical(record))))

    def test_restricted_payment_multisets_normalize_after_recorded_relabeling(self):
        # Sources are introduced by ordered activations before either lot array.
        # This checks payment multiset spelling, not general graph isomorphism.
        record = next(load(p) for p in positive_paths() if p.name == "priority-and-steps.json")
        first = {"type": "W", "restriction": "Spend this mana only to cast a creature spell.",
                 "producer": {"source": "plains1", "ability": {"face": 0, "paragraph": 1}}}
        second = {**first, "producer": {"source": "plains2", "ability": {"face": 0, "paragraph": 1}}}
        record["transitions"][1]["do"]["pay"] = {
            "abilities": [{"source": "plains1", "ability": {"rule": "305.6"}, "adding": ["W"],
                           "pay": {"pool_restricted": [{**first, "count": 2}, {**second, "count": 1}]}}],
            "pool_restricted": [{**first, "count": 2}, {**second, "count": 1}]}
        other = deepcopy(record)
        split = [{**second, "count": 1}, {**first, "count": 1}, {**first, "count": 1}]
        other["transitions"][1]["do"]["pay"]["pool_restricted"] = deepcopy(split)
        other["transitions"][1]["do"]["pay"]["abilities"][0]["pay"]["pool_restricted"] = deepcopy(split)
        self.assertEqual(relabeling(record), relabeling(other))
        self.assertEqual(game_identity(record), game_identity(other))
        self.assertNotEqual(record_identity(record), record_identity(other))
        other["transitions"][1]["do"]["pay"]["pool_restricted"][0]["count"] = 2
        self.assertNotEqual(game_identity(record), game_identity(other))

    def test_renaming_physical_ids_keeps_the_game_identity(self):
        for path in positive_paths():
            record = load(path)
            mapping = {card_id: f"card{n}" for n, card_id in enumerate(record["cards"])}
            other = renamed(record, mapping)
            with self.subTest(example=path.name):
                # Renaming preserves validation evidence too. A folded trigger
                # stretch can remain temporally unproved under either spelling.
                problems = validate(record)
                self.assertEqual([p for p in problems if p.severity == "error"], [])
                self.assertEqual(validate(other), problems)
                self.assertEqual(game_identity(record), game_identity(other))
                self.assertNotEqual(record_identity(record), record_identity(other))

    def test_tier_upgrade_keeps_the_game_identity(self):
        record = next(load(p) for p in positive_paths() if p.name == "priority-and-steps.json")
        upgraded = deepcopy(record)
        upgraded["tier"] = 2
        upgraded["transitions"][1]["delta"]["objects"]["plains1"] = {"tapped": True}
        upgraded["lineage"] = {"upgrades": record_identity(record)}
        self.assertEqual(validate(upgraded), [])
        self.assertEqual(game_identity(record), game_identity(upgraded))
        self.assertNotEqual(record_identity(record), record_identity(upgraded))

    def test_notes_do_not_change_the_game_identity(self):
        record = load(positive_paths()[0])
        other = deepcopy(record)
        other["note"] = "another note"
        other["transitions"][0]["note"] = "changed"
        self.assertEqual(game_identity(record), game_identity(other))

    def test_a_different_action_changes_the_game_identity(self):
        record = next(load(p) for p in positive_paths() if p.name == "priority-and-steps.json")
        other = deepcopy(record)
        other["transitions"][1]["do"] = {"kind": "concede"}
        self.assertNotEqual(game_identity(record), game_identity(other))

    def test_relabeling_follows_first_appearance(self):
        record = next(load(p) for p in positive_paths() if p.name == "priority-and-steps.json")
        mapping = relabeling(record)
        self.assertEqual(mapping["plains2"], "c1")  # the first action plays it
        self.assertEqual(mapping["lions"], "c2")  # the second casts it
        self.assertEqual(len(set(mapping.values())), len(record["cards"]))
        inputs = game_inputs(record)
        self.assertEqual(set(inputs), {"cards", "start", "transitions"})
        self.assertNotIn("delta", inputs["transitions"][0])
        self.assertEqual(inputs["transitions"][0]["do"]["card"], "c1")


if __name__ == "__main__":
    unittest.main()
