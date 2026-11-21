"""The MGN validator on the worked examples: every positive example is clean, every negative case
fails with its stated code, and checkpoints verify against the explicit projection."""
from copy import deepcopy
import json
from pathlib import Path
import subprocess
import sys
from tempfile import TemporaryDirectory
import unittest

from mgn.canonical import sha256
from mgn.examples import EXAMPLES, negative_cases, load, positive_paths
from mgn.identity import game_identity, record_identity
from mgn.oracle import Oracle
from mgn.state import State
from mgn.validate import _check_coherence, validate

ORACLE = Oracle()
ROOT = Path(__file__).resolve().parents[2]
VALIDATOR = ROOT / "scripts/mgn/validate.py"

# The order-to-target-choice example is structurally valid, but stack facts
# alone do not prove temporal linkage to the previously ordered occurrences.
ORDER_UNPROVED = ("warning coherence /transitions/1/do/order: matching surviving source/ability "
                  "declarations do not identify the answered occurrences; temporal order coherence is unproved")


def expected_example_problems(path):
    return [ORDER_UNPROVED] if path.name == "dogfood-trigger-order.json" else []


class PositiveExampleTests(unittest.TestCase):
    def test_examples_exist(self):
        names = {path.name for path in positive_paths()}
        self.assertTrue({"priority-and-steps.json", "damage-then-shrink.json", "tokens-counters-combat.json",
                         "search-and-shuffle.json", "pregame.json", "cleanup-discard.json"} <= names)

    def test_every_example_validates_with_and_without_oracle(self):
        for path in positive_paths():
            record = load(path)
            with self.subTest(example=path.name, oracle=False):
                self.assertEqual([str(p) for p in validate(record)], expected_example_problems(path))
            with self.subTest(example=path.name, oracle=True):
                self.assertEqual([str(p) for p in validate(record, ORACLE)], expected_example_problems(path))

    def test_examples_cover_both_tiers_and_both_starts(self):
        records = [load(path) for path in positive_paths()]
        self.assertEqual({r["tier"] for r in records}, {1, 2})
        self.assertEqual({r["start"]["kind"] for r in records}, {"decks", "position"})
        kinds = {t.get("do", t.get("attempt"))["kind"] for r in records for t in r["transitions"]}
        self.assertTrue({"play_land", "cast", "declare_attackers", "declare_blockers", "search", "discard",
                         "choose_first", "mulligan", "bottom"} <= kinds)
        self.assertNotIn("pass", kinds)  # passing is implied (decision 16)
        self.assertTrue(any("attempt" in t and "delta" not in t for r in records for t in r["transitions"]))
        self.assertTrue(any("attempt" in t and "delta" in t for r in records for t in r["transitions"]))  # the null answer followed


class TriggerOrderCoherenceTests(unittest.TestCase):
    def check(self, *, sources=("first", "second"), step="precombat_main", stack=("S2", "S1"), repeated=False):
        # CR 603.3b: the answer orders abilities bottom first. CR 117.4:
        # later resolutions may produce unrelated triggers before the next
        # recorded boundary; crossing a step implies this group resolved.
        from types import SimpleNamespace
        ability = {"face": 0, "paragraph": 1}
        order = [{"source": source, "ability": ability} for source in ("first", "second")]
        if repeated:
            order = [{"source": "first", "ability": ability, "for": [subject]} for subject in ("one", "two")]
        turn = {"number": 1, "step": "precombat_main", "active": "A", "occurrence": 1}
        before = SimpleNamespace(turn=turn, pending={"kind": "order_triggers"})
        objects = {f"S{i}": {"kind": "ability", "source": source, "ability": ability}
                   for i, source in enumerate(sources, 1)}
        state = SimpleNamespace(turn={**turn, "step": step}, objects=objects,
                                stack_created=len(objects), zones={"stack": list(stack)}, exists=lambda key: key in objects)
        problems = []
        _check_coherence({"kind": "order_triggers", "order": order}, before, 0, state, {}, "/do", problems)
        return problems

    def test_static_new_stack_numbering_remains_checked(self):
        self.assertFalse(any(p.severity == "error" for p in self.check()))
        self.assertTrue(any(p.severity == "error" for p in self.check(stack=("S1", "S2"))))
        # This invariant concerns the numbers, even for unrelated later
        # objects or objects first recorded in a later step.
        self.assertTrue(any(p.severity == "error" for p in
                            self.check(sources=("later1", "later2"), stack=("S1", "S2"), step="combat_damage")))

    def test_unrelated_later_trigger_is_not_relabelled_as_an_ordered_instance(self):
        self.assertEqual(self.check(sources=("later",), stack=("S1",)), [])
        self.assertFalse(any(p.severity == "error" for p in self.check(sources=("first", "later"))))
        self.assertEqual(self.check(sources=("second", "first"), step="combat_damage"), [])

    def test_same_step_later_occurrences_can_match_unique_declarations_in_another_order(self):
        # Both original triggers can resolve, then later events can trigger
        # these same two declarations again in the reverse order, all during
        # this step (603.2, 117.4). The new S1/S2 do not prove which events
        # created them. Source/paragraph equality must not reject this stretch.
        problems = self.check(sources=("second", "first"))
        self.assertEqual([(p.severity, p.code) for p in problems], [("warning", "coherence")])
        self.assertIn("unproved", problems[0].message)

    def test_repeated_source_without_retained_subject_identity_is_explicitly_unproved(self):
        problems = self.check(sources=("first", "first"), repeated=True)
        self.assertEqual([(p.severity, p.code) for p in problems], [("warning", "coherence")])
        self.assertIn("unproved", problems[0].message)

    def test_answer_must_still_permute_the_exact_pending_instances(self):
        record = load(EXAMPLES / "dogfood-trigger-order.json")
        order = record["transitions"][1]["do"]["order"]
        order[1] = deepcopy(order[0])
        problems = validate(record, ORACLE)
        self.assertTrue(any(p.severity == "error" and "/transitions/1/do/order" in p.path for p in problems))


class NegativeCaseTests(unittest.TestCase):
    def test_every_negative_case_fails_as_stated(self):
        for case in negative_cases():
            with self.subTest(case=case["name"]):
                problems = validate(case["record"], ORACLE)
                severity = case.get("severity", "error")
                self.assertIn((severity, case["expect"]), [(p.severity, p.code) for p in problems],
                              "\n".join(str(p) for p in problems))
                if severity == "warning":
                    self.assertFalse(any(p.severity == "error" for p in problems))

    def test_duplicate_member_names_are_refused(self):
        from mgn.canonical import loads
        with self.assertRaises(ValueError):
            loads('{"mgn": "1", "mgn": "1"}')


class EntrypointTests(unittest.TestCase):
    """The documented command (docs/mgn/README.md, spec section 10) runs the file as a script,
    not as a package module; both must load every check, including the causal premises."""

    def run_validator(self, path):
        return subprocess.run(
            [sys.executable, str(VALIDATOR), str(path), "--oracle", str(ROOT / "resources/oracle/cards.jsonl"), "--identity"],
            cwd=ROOT, capture_output=True, text=True, timeout=60)

    def test_script_validates_a_causal_example_and_prints_its_identities(self):
        path = EXAMPLES / "causal-growth-position.json"
        record = load(path)
        self.assertIn("causal", record["start"])
        result = self.run_validator(path)
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertEqual(result.stdout.splitlines(), [
            f"record {record_identity(record)}", f"game {game_identity(record)}", "ok"])

    def test_script_reports_an_error_code_and_exits_nonzero(self):
        case = next(case for case in negative_cases() if case["name"] == "wrong-seat-acts")
        with TemporaryDirectory() as directory:
            path = Path(directory) / "record.json"
            path.write_text(json.dumps(case["record"]), encoding="utf-8")
            result = self.run_validator(path)
        self.assertEqual(result.returncode, 1, result.stdout + result.stderr)
        self.assertEqual(result.stderr, "")
        self.assertIn(" pending_seat ", result.stdout)
        self.assertNotIn("record sha256:", result.stdout)


class CheckpointTests(unittest.TestCase):
    def final_state(self, record):
        problems = []
        state = State.from_record(record, problems)
        for i, transition in enumerate(record["transitions"]):
            if "delta" in transition:
                state.apply(transition["delta"], transition.get("random", []), f"/transitions/{i}/delta", problems)
        self.assertEqual(problems, [])
        return state

    def test_checkpoint_matches_the_explicit_projection(self):
        for path in positive_paths():
            record = load(path)
            state = self.final_state(record)
            explicit = state.explicit(ORACLE)
            record = deepcopy(record)
            record["checkpoints"] = {str(len(record["transitions"])): sha256(explicit)}
            with self.subTest(example=path.name):
                self.assertEqual([str(p) for p in validate(record, ORACLE)], expected_example_problems(path))
                # the explicit form lists every zone and writes defaults out
                self.assertEqual(set(explicit["zones"]), {f"{s}.{z}" for s in "AB" for z in ("library", "hand", "graveyard")} | {"battlefield", "stack", "exile", "command"})
                for object_id in explicit["zones"]["battlefield"]:
                    self.assertIn("controller", explicit["objects"][object_id])
                    if record["tier"] == 2:
                        self.assertIn("tapped", explicit["objects"][object_id])

    def test_wrong_checkpoint_is_reported(self):
        record = load(positive_paths()[0])
        record["checkpoints"] = {"1": "0" * 64}
        codes = [p.code for p in validate(record, ORACLE)]
        self.assertIn("checkpoint", codes)

    def test_tier2_checkpoint_without_oracle_is_a_warning(self):
        record = next(load(p) for p in positive_paths() if load(p)["tier"] == 2)
        record["checkpoints"] = {"1": "0" * 64}
        problems = validate(record)
        self.assertEqual([(p.severity, p.code) for p in problems], [("warning", "checkpoint")])

    def test_printed_values_are_the_tier2_defaults(self):
        record = next(load(p) for p in positive_paths() if p.name == "tokens-counters-combat.json")
        state = self.final_state(record)
        explicit = state.explicit(ORACLE)
        elves = explicit["objects"]["elves"]
        self.assertEqual((elves["power"], elves["toughness"]), (2, 2))
        self.assertEqual(elves["counters"], {"+1/+1": 1})
        self.assertEqual(elves["keywords"], [])
        self.assertEqual(elves["types"], ["creature"])
        self.assertEqual(sorted(elves["subtypes"]), ["Druid", "Elf"])
        self.assertEqual(elves["colors"], ["green"])
        token = explicit["objects"]["T1"]
        self.assertEqual((token["power"], token["toughness"], token["colors"]), (1, 1, ["red"]))


class PortablePaymentValidationTests(unittest.TestCase):
    def test_nested_references_remain_checked_without_treating_oracle_words_as_ids(self):
        from mgn.validate import _check_references
        class References:
            seats = ["A", "B"]
            cards = {}
            def known(self, reference):
                return self.exists(reference)
            def exists(self, reference):
                return reference in {"spell", "filter", "food", "grove"}
        lot = {"type": "G", "count": 1, "restriction": "Spend this mana only to cast a creature spell of the chosen type.",
               "chosen": {"creature_type": "Elf"}, "producer": {"source": "grove", "ability": {"face": 0, "paragraph": 1}}}
        action = {"kind": "cast", "card": "spell", "pay": {"abilities": [
            {"source": "filter", "ability": {"face": 0, "paragraph": 1}, "adding": ["G"],
             "pay": {"objects": [["food"]], "pool_restricted": [lot]}}]}}
        problems = []
        _check_references(action, References(), "/do", problems)
        self.assertEqual(problems, [])
        action["pay"]["abilities"][0]["pay"]["objects"] = [["undeclared_object"]]
        action["pay"]["abilities"][0]["source"] = "undeclared_ability_source"
        lot["producer"]["source"] = "undeclared_producer"
        _check_references(action, References(), "/do", problems)
        messages = " ".join(problem.message for problem in problems)
        for reference in ("undeclared_object", "undeclared_ability_source", "undeclared_producer"):
            self.assertIn(reference, messages)
        self.assertEqual(len(problems), 3)

    def test_nested_nonmana_ability_cannot_be_declared_as_mana_production(self):
        from mgn.validate import _check_nested_mana_abilities
        class TokenState:
            copy_bases = {}
            cards = {}
            objects = {"filter": {"token": {"text": "{T}: Draw a card."}}}
            def reference_facts(self, reference):
                return self.objects.get(reference, {})
        payment = {"abilities": [{"source": "filter", "ability": {"face": 0, "paragraph": 1}, "adding": ["G"]}]}
        problems = []
        _check_nested_mana_abilities(payment, TokenState(), object(), "/do/pay", problems)
        self.assertEqual(len(problems), 1)
        self.assertEqual(problems[0].code, "ability")
        self.assertEqual(problems[0].path, "/do/pay/abilities/0/adding")

    def test_recursive_allocation_restricted_type_must_match_mana(self):
        from mgn.validate import _check_payment_lots
        action = {"kind": "activate", "pay": {"abilities": [{"pay": {"allocation": [
            {"symbol": 0, "mana": "R", "restricted": {"type": "G", "restriction": "Creature spells only."}}]}}]}}
        problems = []
        _check_payment_lots(action, "/do", problems)
        self.assertEqual(len(problems), 1)
        self.assertEqual(problems[0].code, "coherence")
        self.assertEqual(problems[0].path, "/do/pay/abilities/0/pay/allocation/0/restricted/type")


if __name__ == "__main__":
    unittest.main()
