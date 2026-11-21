"""MGN 5.8 and CR 601.2f–h, 605.3, 106.6 boundary expectations.

Pools/costs below are independent examples of resource conservation and legal
symbol payments, not observed engine choices promoted to expectations.
"""
import unittest
from copy import deepcopy
from mgn.payments import InvalidPayment, PaymentEvidenceError, prior_spending, select_payment, validate_production


def quote(symbols, pool, **changes):
    return {"symbol_indices": list(range(len(symbols))), "payments": [{"requirement": {"symbols": symbols, **changes}, "pool": pool, "left": {}}]}


class PaymentsTests(unittest.TestCase):
    def test_adapter_projects_only_frozen_production_terms_and_choices(self):
        from mgn.adapters.karn import KarnAdapter
        from mgn.oracle import Oracle
        pinned = Oracle()
        class Adapter:
            oracle = pinned
            cards = {"c7": {"oracle": pinned.by_name("Cavern of Souls")}}
            def _record_id(self, source):
                return f"c{source}"
        unit = {"mana": "R", "source": 7, "provenance": {"physical_definition": 0,
            "ability": {"source": 7, "origin": {"kind": "own"}, "paragraph": 3,
                        "definition_identity": {"card": 0, "face": "Whole"}},
            "restriction": "Spend this mana only to cast a creature spell of the chosen type, and that spell can't be countered.",
            "chosen": {"creature_type": "Goblin"}}}
        # Only pinned Oracle remains; no current object state exists. Native
        # rendering is deliberately different from the canonical Oracle paragraph.
        unit["provenance"]["restriction"] = "Native paraphrase of chosen-type condition and countering rider."
        lot = KarnAdapter._restricted_lot(Adapter(), unit)
        self.assertEqual(lot["chosen"], {"creature_type": "Goblin"})
        self.assertEqual(lot["producer"], {"source": "c7", "ability": {"face": 0, "paragraph": 3}})
        self.assertEqual(lot["restriction"], pinned.paragraphs(pinned.by_name("Cavern of Souls"))[2])
        for origin in ("copied", "given"):
            unit["provenance"]["ability"]["origin"]["kind"] = origin
            with self.assertRaisesRegex(PaymentEvidenceError, "authenticated printed"):
                KarnAdapter._restricted_lot(Adapter(), unit)
        unit["provenance"]["ability"]["origin"]["kind"] = "own"
        unit["provenance"]["physical_definition"] = 99
        with self.assertRaisesRegex(PaymentEvidenceError, "authenticated printed"):
            KarnAdapter._restricted_lot(Adapter(), unit)
        with self.assertRaisesRegex(PaymentEvidenceError, "production-time"):
            KarnAdapter._restricted_lot(Adapter(), {"mana": "R"})

    def test_printed_lot_uses_pinned_reminder_stripped_paragraph_or_refuses(self):
        from mgn.adapters.karn import KarnAdapter
        from mgn.oracle import Face, Oracle
        pinned = object.__new__(Oracle)
        pinned.faces = {"fixture": [Face({"oracle_text":
            "{T}: Add {G}. Spend this mana only to cast a creature spell. (Reminder text.)"})]}
        class Adapter:
            oracle = pinned
            cards = {"land": {"oracle": "fixture"}}
            def _record_id(self, source):
                return "land"
        unit = {"mana": "G", "source": 7, "provenance": {"physical_definition": 0,
            "ability": {"source": 7, "origin": {"kind": "own"}, "paragraph": 1,
                        "definition_identity": {"card": 0, "face": "Whole"}},
            "restriction": "Native rendering is not the portable spelling.", "chosen": {}}}
        self.assertEqual(KarnAdapter._restricted_lot(Adapter(), unit)["restriction"],
                         "{T}: Add {G}. Spend this mana only to cast a creature spell.")
        unit["provenance"]["ability"]["paragraph"] = 2
        with self.assertRaisesRegex(PaymentEvidenceError, "pinned Oracle paragraph"):
            KarnAdapter._restricted_lot(Adapter(), unit)

    def test_proven_unpaid_cost_is_distinct_from_unsupported_or_ambiguous_evidence(self):
        # CR 601.2h/118.3a: an otherwise supported G requirement cannot be paid
        # with no resources. Unknown symbols or ambiguous spending prove less.
        with self.assertRaises(InvalidPayment):
            select_payment({}, {}, quote(["G"], {}))
        for q in (quote(["R", "S"], {}), quote(["G"], {}, convoke=[True, 0, 0, 0, 0]),
                  quote(["1"], {"R": 1, "G": 1})):
            with self.subTest(quote=q), self.assertRaises(PaymentEvidenceError) as cm:
                select_payment({}, {}, q)
            self.assertNotIsInstance(cm.exception, InvalidPayment)

    def test_empty_prior_green_production(self):
        self.assertEqual(select_payment({"abilities": [{"adding": ["G"]}]}, {}, quote(["G"], {"G": 1})), {"G": 1})

    def test_prior_red_and_generated_green_pay_one_green(self):
        self.assertEqual(select_payment({"pool": {"R": 1}}, {"R": 1}, quote(["1", "G"], {"R": 1, "G": 1})), {"R": 1, "G": 1})

    def test_generated_mana_can_remain(self):
        self.assertEqual(select_payment({}, {}, quote(["G"], {"G": 2})), {"G": 1})

    def test_explicit_prior_selection_disambiguates_same_type_leftover(self):
        q = quote(["G"], {"G": 2})
        self.assertEqual(select_payment({"pool": {"G": 1}}, {"G": 1}, q), {"G": 1})
        self.assertEqual(select_payment({}, {"G": 1}, q), {"G": 1})
        with self.assertRaisesRegex(PaymentEvidenceError, "ambiguous"):
            prior_spending({"G": 1}, {"G": 1}, {"G": 1}, {"G": 1})

    def test_hybrid_allocation_selects_one_of_two_generated_colors(self):
        q = quote(["R/G"], {"R": 1, "G": 1})
        with self.assertRaisesRegex(PaymentEvidenceError, "ambiguous"):
            select_payment({}, {}, q)
        self.assertEqual(select_payment({"allocation": [{"symbol": 0, "mana": "G"}]}, {}, q), {"G": 1})

    def test_same_aggregate_hybrid_assignments_are_validated_independently(self):
        q = quote(["R/G", "R/G"], {"R": 1, "G": 1})
        pay = {"pool": {"R": 1, "G": 1}, "allocation": [{"symbol": 0, "mana": "G"}, {"symbol": 1, "mana": "R"}]}
        self.assertEqual(select_payment(pay, {"R": 1, "G": 1}, q), {"R": 1, "G": 1})
        pay["allocation"][1]["mana"] = "G"
        with self.assertRaisesRegex(PaymentEvidenceError, "do not pay"):
            select_payment(pay, {"R": 1, "G": 1}, q)

    def test_generic_allocation_names_each_unit(self):
        q = quote(["2"], {"R": 1, "G": 2})
        self.assertEqual(select_payment({"allocation": [{"symbol": 0, "mana": "G"}, {"symbol": 0, "mana": "G"}]}, {}, q), {"G": 2})

    def test_two_generic_hybrid_choice_preserves_two_mana_payment(self):
        q = quote(["2/G"], {"R": 2, "G": 1})
        self.assertEqual(select_payment({"pool": {"R": 2}, "allocation": [{"symbol": 0, "mana": "R"}, {"symbol": 0, "mana": "R"}]}, {"R": 2}, q), {"R": 2})

    def test_reductions_and_minimum_are_locked_requirement_facts(self):
        self.assertEqual(select_payment({"pool": {"R": 1}}, {"R": 1}, quote(["3", "G"], {"R": 1, "G": 1}, reduction=2)), {"R": 1, "G": 1})
        self.assertEqual(select_payment({"pool": {"R": 2}}, {"R": 2}, quote(["G"], {"R": 2, "G": 1}, minimum=3)), {"R": 2, "G": 1})

    def test_phyrexian_life_choice_needs_no_mana_for_life_symbol(self):
        self.assertEqual(select_payment({"life": [True]}, {}, quote([], {})), {})
        q = quote(["G"], {"G": 1})
        q["symbol_indices"] = [1]
        self.assertEqual(select_payment({"life": [True], "allocation": [{"symbol": 1, "mana": "G"}]}, {}, q), {"G": 1})
        with self.assertRaisesRegex(PaymentEvidenceError, "paid with life"):
            select_payment({"life": [True], "allocation": [{"symbol": 0, "mana": "G"}]}, {}, q)

    def test_prior_plain_and_restricted_same_color_are_selected_separately(self):
        unit = {"mana": "G", "restriction": "Spend this mana only to cast a creature spell.", "source": 2}
        lot = {"type": "G", "restriction": unit["restriction"], "producer": {"source": "c2", "ability": {"face": 0, "paragraph": 1}}}
        resolver = lambda u: {"type": u["mana"], "restriction": u["restriction"], "producer": {"source": f"c{u['source']}", "ability": {"face": 0, "paragraph": 1}}}
        pool = {"G": 1, "restricted": [unit]}
        self.assertEqual(select_payment({"pool": {"G": 1}}, pool, quote(["G"], pool)), {"G": 1})
        pay = {"pool_restricted": [{**lot, "count": 1}], "allocation": [{"symbol": 0, "mana": "G", "restricted": lot}]}
        self.assertEqual(select_payment(pay, pool, quote(["G"], pool), restricted_lot=resolver), {"restricted": [0]})
        with self.assertRaisesRegex(PaymentEvidenceError, "authenticated Oracle"):
            select_payment(pay, pool, quote(["G"], pool))
        pay["allocation"][0].pop("restricted")
        with self.assertRaisesRegex(PaymentEvidenceError, "do not pay"):
            select_payment(pay, pool, quote(["G"], pool), restricted_lot=resolver)

    def test_generated_restricted_selection_uses_a_portable_lot_identity(self):
        unit = {"mana": "G", "restriction": "Spend this mana only to cast a creature spell.", "source": 2}
        lot = {"type": "G", "restriction": unit["restriction"]}
        pay = {"allocation": [{"symbol": 0, "mana": "G", "restricted": lot}]}
        resolver = lambda u: {"type": u["mana"], "restriction": u["restriction"]}
        q = quote(["G"], {"restricted": [unit]})
        self.assertEqual(select_payment(pay, {}, q, restricted_lot=resolver), {"restricted_after": [0]})
        with self.assertRaisesRegex(PaymentEvidenceError, "do not pay"):
            select_payment({}, {}, q, restricted_lot=resolver)
        pay["allocation"][0]["mana"] = "R"
        with self.assertRaisesRegex(PaymentEvidenceError, "type must match"):
            select_payment(pay, {}, q, restricted_lot=resolver)

    def test_restricted_lots_require_frozen_choices_when_wording_and_producer_match(self):
        restriction = "Spend this mana only to cast a creature spell of the chosen type."
        units = [{"mana": "G", "source": 2, "restriction": restriction, "chosen": {"creature_type": t}} for t in ("Elf", "Goblin")]
        resolver = lambda u: {"type": u["mana"], "restriction": u["restriction"], "chosen": u["chosen"]}
        q = quote(["G"], {"restricted": units})
        lot = {"type": "G", "restriction": restriction, "count": 1}
        with self.assertRaisesRegex(PaymentEvidenceError, "frozen chosen"):
            select_payment({"pool_restricted": [lot]}, q["payments"][0]["pool"], q, restricted_lot=resolver)
        lot["chosen"] = {"creature_type": "Goblin"}
        self.assertEqual(select_payment({"pool_restricted": [lot]}, q["payments"][0]["pool"], q, restricted_lot=resolver), {"restricted": [1]})

    def test_same_color_restriction_wording_and_producer_select_the_intended_unit(self):
        units = [{"mana": "G", "source": source, "restriction": "Spend this mana only to cast a creature spell."} for source in (2, 3)]
        resolver = lambda u: {"type": u["mana"], "restriction": u["restriction"], "producer": {"source": f"c{u['source']}", "ability": {"face": 0, "paragraph": 1}}}
        pool = {"restricted": units}
        lot = {"type": "G", "count": 1, "restriction": units[0]["restriction"]}
        with self.assertRaisesRegex(PaymentEvidenceError, "producer or frozen"):
            select_payment({"pool_restricted": [lot]}, pool, quote(["G"], pool), restricted_lot=resolver)
        lot["producer"] = {"source": "c3", "ability": {"face": 0, "paragraph": 1}}
        self.assertEqual(select_payment({"pool_restricted": [lot]}, pool, quote(["G"], pool), restricted_lot=resolver), {"restricted": [1]})

    def test_convoke_colored_and_generic_substitutions_follow_total_cost_determination(self):
        # CR 702.51a/b: two red and two generic convokers pay {2}{R}{R}.
        self.assertEqual(select_payment({}, {}, quote(["2", "R", "R"], {}, convoke=[0, 0, 0, 2, 0], substitution=2)), {})
        # A floor of three is applied before the green creature substitutes.
        self.assertEqual(select_payment({"pool": {"C": 1}}, {"C": 1},
            quote(["G"], {"C": 1}, minimum=3, convoke=[0, 0, 0, 0, 1], substitution=1)), {"C": 1})

    def test_convoke_cannot_pay_colorless_wrong_colors_or_double_pay_a_symbol(self):
        # CR 702.51a: a colored convoker must match a surviving colored requirement.
        for symbols, convoke, extra in [(["C"], [0, 0, 0, 0, 1], {}),
                (["R"], [0, 0, 0, 0, 1], {}),
                (["G"], [0, 0, 0, 0, 1], {"typed_reduction": [0, 0, 0, 0, 1, 0]})]:
            with self.subTest(symbols=symbols, extra=extra), self.assertRaisesRegex(PaymentEvidenceError, "do not pay"):
                select_payment({}, {}, quote(symbols, {}, convoke=convoke, **extra))
        with self.assertRaisesRegex(PaymentEvidenceError, "do not pay"):
            select_payment({"allocation": [{"symbol": 0, "mana": "G"}]}, {},
                quote(["G"], {"G": 1}, convoke=[0, 0, 0, 0, 1]))

    def test_convoke_respects_hybrid_equivalence_and_leaves_other_mana_explicit(self):
        # CR 601.2b/702.51a: the U requirement of chosen {W/U} can be convoked.
        q = quote(["1", "W/U"], {"R": 1}, convoke=[0, 1, 0, 0, 0])
        self.assertEqual(select_payment({"pool": {"R": 1}}, {"R": 1}, q), {"R": 1})
        # With two U symbols, one convoker replaces one; the other needs actual mana.
        self.assertEqual(select_payment({"pool": {"U": 1}}, {"U": 1},
            quote(["U", "U"], {"U": 1}, convoke=[0, 1, 0, 0, 0])), {"U": 1})

    def test_convoke_can_pay_a_later_symbol_after_an_explicit_earlier_mana_payment(self):
        # CR 601.2b/702.51a: one G convoker and one G mana pay {G}{G};
        # either printed symbol can be the explicit mana payment.
        for symbol in (0, 1):
            with self.subTest(symbol=symbol):
                pay = {"pool": {"G": 1}, "allocation": [{"symbol": symbol, "mana": "G"}]}
                self.assertEqual(select_payment(pay, {"G": 1},
                    quote(["G", "G"], {"G": 1}, convoke=[0, 0, 0, 0, 1])), {"G": 1})
        # Resolve the first hybrid as U and pay U; the G creature pays the later G.
        pay = {"pool": {"U": 1}, "allocation": [{"symbol": 0, "mana": "U"}]}
        self.assertEqual(select_payment(pay, {"U": 1},
            quote(["G/U", "G"], {"U": 1}, convoke=[0, 0, 0, 0, 1])), {"U": 1})

    def test_nested_mana_plan_inputs_do_not_drop_unsupported_announcements(self):
        q = quote(["G"], {"G": 1})
        q["move"] = {"kind": "cast", "plan": [{"source": 2, "chose": [1], "paying": [[8]]}]}
        q["payments"].insert(0, {"pool": {}, "left": {}})
        ability = {"adding": ["G"], "additional": {"choices": [2]}, "pay": {"objects": [["food"]]}}
        with self.assertRaisesRegex(PaymentEvidenceError, "native-to-record"):
            select_payment({"abilities": [ability]}, {}, q)
        self.assertEqual(select_payment({"abilities": [ability]}, {}, q, record_id=lambda n: "food"), {"G": 1})
        ability["x"] = 1
        with self.assertRaisesRegex(PaymentEvidenceError, "own retained X"):
            select_payment({"abilities": [ability]}, {}, q)
        ability.pop("x")
        ability["pay"]["life"] = [True]
        with self.assertRaisesRegex(PaymentEvidenceError, "nested life"):
            select_payment({"abilities": [ability]}, {}, q, record_id=lambda n: "food")

    def test_missing_locked_requirement_and_nested_costs_refuse(self):
        with self.assertRaisesRegex(PaymentEvidenceError, "locked"):
            select_payment({}, {}, {})
        q = quote(["G"], {"G": 1})
        q["payments"].insert(0, {"pool": {"R": 1}, "left": {}})
        with self.assertRaisesRegex(PaymentEvidenceError, "nested"):
            select_payment({}, {"R": 1}, q)

    def test_mana_production_is_captured_not_assumed_from_adding(self):
        pay = {"abilities": [{"adding": ["G"]}]}
        with self.assertRaisesRegex(PaymentEvidenceError, "production disagrees"):
            validate_production(pay, {"events": [{"event": "mana_added", "mana": ["G", "G"]}]})

    def test_empty_payment_does_not_admit_unattributed_mana(self):
        with self.assertRaisesRegex(PaymentEvidenceError, "unattributed"):
            validate_production({}, {"events": [{"event": "mana_added", "source": 5, "mana": ["R"]}]})

    def test_production_validation_includes_every_declared_instruction(self):
        q = {"move": {"kind": "cast", "plan": [{"source": 1}]}, "events": [
            {"event": "activated", "source": 1, "ability": None},
            {"event": "mana_added", "source": 1, "mana": ["R"]},
            {"event": "mana_added", "source": 1, "mana": ["G"]}]}
        with self.assertRaisesRegex(PaymentEvidenceError, "invocation provenance"):
            validate_production({"abilities": [{"adding": ["R", "G"]}]}, q)
        q["events"] = q["events"][:1] + [{"event": "mana_added", "source": 1, "mana": ["R", "G"]}]
        self.assertEqual(validate_production({"abilities": [{"adding": ["G", "R"]}]}, q)["plan"], [["R", "G"]])
        with self.assertRaisesRegex(PaymentEvidenceError, "production disagrees"):
            validate_production({"abilities": [{"adding": ["R"]}]}, q)

    def test_inputs_unchanged(self):
        args = ({"pool": {"R": 1}}, {"R": 1}, quote(["1", "G"], {"R": 1, "G": 1}))
        frozen = deepcopy(args)
        select_payment(*args)
        self.assertEqual(args, frozen)


if __name__ == "__main__":
    unittest.main()
