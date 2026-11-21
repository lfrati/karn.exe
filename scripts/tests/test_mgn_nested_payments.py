"""CR 602.2b/605.3: a mana activation pays its own cost before producing mana.

Shimmering Grotto's {1}, {T} activation spends an already available unit,
then adds its chosen color. That unit is not also spent by the outer spell.
"""
from copy import deepcopy
import unittest

from mgn.payments import InvalidPayment, PaymentEvidenceError, invert_payment_quote, restore_payment, select_payment, validate_bound_payment, validate_production


def filtering_quote(prior=None, final_symbols=None):
    prior = deepcopy(prior or {})
    after_forest = {**prior, "G": prior.get("G", 0) + 1}
    after_filter = {**prior, "W": 1}
    move = {"kind": "cast", "card": 10, "plan": [{"source": 1, "ability": 0}, {"source": 2, "ability": 1}]}
    events = [{"event": "activated", "source": 1, "ability": None},
              {"event": "mana_added", "source": 1, "player": 0, "mana": ["G"]},
              {"event": "activated", "source": 2, "ability": None},
              {"event": "mana_added", "source": 2, "player": 0, "mana": ["W"]},
              {"event": "cast", "spell": 20}]
    quote = {"complete": True, "move": move, "events": events,
             "prior_pools": [prior, {}], "pools": [prior, {}], "symbol_indices": [0],
             "payments": [
                 {"source": 1, "kind": "ability", "player": 0, "pool": prior, "left": prior, "requirement": {"symbols": []}},
                 {"source": 2, "kind": "ability", "player": 0, "pool": after_forest, "left": prior, "requirement": {"symbols": ["1"]}},
                 {"source": 20, "kind": "spell", "player": 0, "pool": after_filter, "left": prior, "requirement": {"symbols": final_symbols or ["W"]}}]}
    pay = {"abilities": [{"source": "forest", "ability": {"rule": "305.6"}, "adding": ["G"]},
                         {"source": "grotto", "ability": {"face": 0, "paragraph": 2}, "adding": ["W"], "pay": {"pool": {"G": 1}}}]}
    return quote, pay


def scoped_filtering_quote(prior=None, final_symbols=None):
    quote, pay = filtering_quote(prior, final_symbols)
    receipts = quote["payments"]
    for i, receipt in enumerate(receipts):
        receipt["path"] = [i] if i < 2 else []
        receipt["prior_pool"] = deepcopy(receipt["pool"] if i < 2 else quote["prior_pools"][0])
        receipt["symbol_indices"] = list(range(len(receipt["requirement"]["symbols"])))
    quote["productions"] = []
    for i in range(2):
        quote["productions"].append({"path": [i], "triggered": False, "sequence": i * 2 + 1,
            "source": i + 1, "player": 0, "activation": i, "invocation": {"source": i + 1, "declaration_index": i}, "mana": ["G" if i == 0 else "W"],
            "before": deepcopy(receipts[i]["left"]), "after": deepcopy(receipts[i + 1]["pool"])})
    return quote, pay


class NestedPaymentTests(unittest.TestCase):
    def test_earlier_outer_production_is_prior_to_the_filter_activation(self):
        quote, pay = filtering_quote()
        self.assertEqual(select_payment(pay, {}, quote), {"W": 1})
        self.assertNotIn("pool", pay)
        self.assertEqual(pay["abilities"][1]["pay"], {"pool": {"G": 1}})

    def test_child_payment_cannot_be_omitted_or_changed(self):
        quote, pay = filtering_quote()
        del pay["abilities"][1]["pay"]
        with self.assertRaises(InvalidPayment):
            select_payment(pay, {}, quote)
        pay["abilities"][1]["pay"] = {"pool": {"R": 1}}
        with self.assertRaises(PaymentEvidenceError):
            select_payment(pay, {}, quote)

    def test_plain_same_type_old_new_survival_is_feasible_without_age_tags(self):
        quote, pay = filtering_quote({"G": 1}, ["1", "W"])
        quote["payments"][-1]["left"] = {}
        quote["pools"][0] = {}
        quote["symbol_indices"] = [0, 1]
        # The filter spends one of two indistinguishable G; the remaining G
        # can satisfy either explicit old/new attribution of the main payment.
        self.assertEqual(select_payment(pay, {"G": 1}, quote), {"W": 1, "G": 1})
        pay["pool"] = {"G": 1}
        self.assertEqual(select_payment(pay, {"G": 1}, quote), {"W": 1, "G": 1})
        quote["payments"][-1]["requirement"]["symbols"] = ["2", "W"]
        with self.assertRaises(InvalidPayment):
            select_payment(pay, {"G": 1}, quote)

    def test_child_cannot_spend_a_prior_unit_twice(self):
        quote, pay = filtering_quote({"G": 1})
        # No Forest activation: Grotto consumes the only original G.
        quote["payments"] = quote["payments"][1:]
        quote["payments"][0].update(pool={"G": 1}, left={})
        quote["payments"][-1].update(pool={"W": 1}, left={})
        quote["move"]["plan"] = quote["move"]["plan"][1:]
        pay["abilities"] = pay["abilities"][1:]
        pay["pool"] = {"G": 1}
        with self.assertRaisesRegex(InvalidPayment, "consumed by a child"):
            select_payment(pay, {"G": 1}, quote)

    def test_inverse_records_and_proves_child_payment_from_its_receipt(self):
        quote, expected = filtering_quote()
        reconstructed = deepcopy(expected)
        del reconstructed["abilities"][1]["pay"]
        got = invert_payment_quote(quote, quote["move"], {}, {}, quote["events"], reconstructed, 0)
        self.assertEqual(got, expected)
        self.assertEqual(select_payment(got, {}, quote), {"W": 1})

    def test_bound_quote_checks_every_boundary_and_exact_child_selection(self):
        quote, pay = filtering_quote()
        quote["move"]["plan"][0]["mana"] = {}
        quote["move"]["plan"][1]["mana"] = {"G": 1}
        quote["move"]["mana"] = {"W": 1}
        production = validate_production(pay, quote)
        validate_bound_payment(quote, quote["move"], 0, production)
        for mutate in (
            lambda q: q["payments"][1].update(pool={"G": 2}),
            lambda q: q["payments"][1].update(player=1),
            lambda q: q["move"]["plan"][1].update(mana={"R": 1}),
            lambda q: q["move"]["plan"][1].pop("mana"),
        ):
            changed = deepcopy(quote)
            mutate(changed)
            with self.subTest(changed=changed), self.assertRaises(PaymentEvidenceError):
                validate_bound_payment(changed, changed["move"], 0, production)

    def test_adapter_binds_unpaid_child_before_production_then_outer_pool(self):
        from mgn.adapters.karn import KarnAdapter
        complete, pay = scoped_filtering_quote()
        first = deepcopy(complete)
        first.update(complete=False, error="explicit pool is required")
        first["payments"] = first["payments"][:2]
        first["payments"][-1]["left"] = None
        first["events"] = []
        second = deepcopy(complete)
        second.update(complete=False, error="explicit pool is required")
        second["payments"][-1]["left"] = None
        second["events"] = []
        calls = []
        replies = iter([first, second, complete])
        class Engine:
            def send(self, request):
                calls.append(deepcopy(request["move"]))
                reply = deepcopy(next(replies))
                reply["move"] = deepcopy(request["move"])
                return {"ok": True, "state": reply}
        class Adapter:
            engine = Engine()
            session_id = 1
            state = {"request_id": 1, "asked": {"player": 0}}
            _restricted_lot = None
            def _record_id(self, source):
                return str(source)
        move, proof = KarnAdapter._explicit_payment(Adapter(), 0, complete["move"], {"kind": "cast", "pay": pay})
        self.assertEqual(len(calls), 3)
        self.assertNotIn("mana", calls[0])
        self.assertNotIn("mana", calls[0]["plan"][1])
        self.assertEqual(calls[1]["plan"][1]["mana"], {"G": 1})
        self.assertNotIn("mana", calls[1])
        self.assertEqual(calls[2]["mana"], {"W": 1})
        self.assertEqual(move["plan"][1]["mana"], {"G": 1})
        self.assertTrue(proof["complete"])
        self.assertNotIn("mana", complete["move"]["plan"][1], "frozen input remains unchanged")

    def test_unpaid_child_without_recorded_pool_refuses_without_trying_candidates(self):
        from mgn.adapters.karn import KarnAdapter, Refusal
        complete, pay = scoped_filtering_quote()
        del pay["abilities"][1]["pay"]
        partial = deepcopy(complete)
        partial["payments"] = partial["payments"][:2]
        partial["payments"][-1]["left"] = None
        calls = []
        class Engine:
            def send(self, request):
                calls.append(request)
                return {"ok": True, "state": deepcopy(partial)}
        class Adapter:
            engine = Engine()
            session_id = 1
            state = {"request_id": 1, "asked": {"player": 0}}
            _restricted_lot = None
            def _record_id(self, source):
                return str(source)
        with self.assertRaises(Refusal) as caught:
            KarnAdapter._explicit_payment(Adapter(), 0, complete["move"], {"kind": "cast", "pay": pay})
        self.assertEqual(caught.exception.code, "illegal_cost")
        self.assertEqual(len(calls), 1)

    def test_final_transaction_quote_rechecks_the_child_spend(self):
        from mgn.adapters.karn import KarnAdapter, Refusal
        initial, pay = scoped_filtering_quote({"R": 1})
        final = deepcopy(initial)
        # A second quote spends R on Grotto instead of the authored G, while
        # producing the same W and paying the same main cost. This must refuse.
        final["payments"][1]["left"] = {"G": 1}
        final["payments"][-1].update(pool={"G": 1, "W": 1}, left={"G": 1})
        final["pools"][0] = {"G": 1}
        replies = iter([initial, final])
        class Engine:
            def send(self, request):
                return {"ok": True, "state": deepcopy(next(replies))}
        class Adapter:
            engine = Engine()
            session_id = 1
            state = {"request_id": 1, "asked": {"player": 0}}
            _restricted_lot = None
            def _record_id(self, source):
                return str(source)
        with self.assertRaisesRegex(Refusal, "quoted native child spending"):
            KarnAdapter._explicit_payment(Adapter(), 0, initial["move"], {"kind": "cast", "pay": pay})

    def test_authored_child_selection_is_restored_only_after_exact_receipt_check(self):
        quote, expected = filtering_quote()
        quote["move"]["mana"] = {"W": 1}
        quote["payments"][-1]["selected"] = {"W": 1}
        reconstructed = deepcopy(expected)
        del reconstructed["abilities"][1]["pay"]
        selection = {"pay": expected, "quote": quote}
        self.assertEqual(restore_payment(selection, quote["move"], {}, {}, quote["events"], reconstructed, 0), expected)
        selection["pay"]["abilities"][1]["pay"]["pool"] = {}
        with self.assertRaises(InvalidPayment):
            restore_payment(selection, quote["move"], {}, {}, quote["events"], reconstructed, 0)

    def test_restricted_child_origin_is_exact_or_refused_when_old_new_ambiguous(self):
        unit = {"mana": "G", "source": 1}
        lot = {"type": "G", "restriction": "Spend this mana only to activate abilities."}
        resolve = lambda value: lot
        quote, pay = filtering_quote()
        quote["prior_pools"][0] = {"restricted": [unit]}
        quote["payments"] = quote["payments"][1:]
        quote["payments"][0].update(pool={"restricted": [unit]}, left={})
        quote["move"]["plan"] = quote["move"]["plan"][1:]
        pay["abilities"] = pay["abilities"][1:]
        pay["abilities"][0]["pay"] = {"pool_restricted": [{**lot, "count": 1}]}
        self.assertEqual(select_payment(pay, {"restricted": [unit]}, quote, restricted_lot=resolve), {"W": 1})
        # An earlier activation produces an indistinguishable second restricted
        # unit. Spending just one no longer proves which outer prior unit survives.
        quote["payments"].insert(0, {"source": 1, "pool": {"restricted": [unit]}, "left": {"restricted": [unit]}, "requirement": {"symbols": []}})
        quote["payments"][1].update(pool={"restricted": [unit, unit]}, left={"restricted": [unit]})
        quote["payments"][-1]["pool"] = {"W": 1, "restricted": [unit]}
        quote["move"]["plan"].insert(0, {"source": 1, "ability": 0})
        pay["abilities"].insert(0, {"source": "producer", "ability": {"face": 0, "paragraph": 1}, "adding": ["G"]})
        with self.assertRaisesRegex(PaymentEvidenceError, "ambiguous prior/generated origin"):
            select_payment(pay, {"restricted": [unit]}, quote, restricted_lot=resolve)

    def test_authored_alternative_child_spend_must_match_exact_quote(self):
        quote, pay = filtering_quote({"R": 1})
        # Both G and R can pay {1}, but this native receipt specifically spent G.
        pay["abilities"][1]["pay"]["pool"] = {"R": 1}
        with self.assertRaisesRegex(PaymentEvidenceError, "quoted native child spending"):
            select_payment(pay, {"R": 1}, quote)

    def test_boolean_native_mana_is_classification_but_other_types_are_invalid(self):
        for classification in (True, False):
            quote, expected = filtering_quote()
            quote["move"].update(kind="activate", source=3, ability=0, mana=classification)
            quote["events"][-1] = {"event": "activated", "source": 3, "ability": 1}
            quote["payments"][-1].update(kind="ability", source=3)
            self.assertEqual(invert_payment_quote(quote, quote["move"], {}, {}, quote["events"],
                                                  {"abilities": expected["abilities"]}, 0), expected)
        for malformed in ([], "false", 1):
            quote, expected = filtering_quote()
            quote["move"]["mana"] = malformed
            with self.assertRaisesRegex(PaymentEvidenceError, "malformed explicit native mana selector"):
                invert_payment_quote(quote, quote["move"], {}, {}, quote["events"], expected, 0)


class NativeNestedPaymentTests(unittest.TestCase):
    def test_grotto_spends_authored_green_before_producing_white_for_lions(self):
        # CR 602.2b/605.3: {1}, {T} is paid before Grotto adds W. Savannah
        # Lions costs W; the same G unit cannot pay both activations and spell.
        import play
        from mgn.adapters.karn import KarnAdapter
        engine = play.Engine()
        self.addCleanup(lambda: (engine.proc.stdin.close(), engine.proc.wait(timeout=10), engine.proc.stdout.close()))
        engine.ask({"cmd": "new", "players": [
            {"library": ["Island"] * 8, "battlefield": ["Forest", "Shimmering Grotto"], "hand": ["Savannah Lions"]},
            {"library": ["Island"] * 8}]})
        while engine.state["part"] != "PrecombatMain":
            engine.act({"kind": "pass"})
        def one(name):
            return next(int(o) for o, facts in engine.state["objects"].items() if facts["name"] == name)
        forest, grotto, lions = one("Forest"), one("Shimmering Grotto"), one("Savannah Lions")
        class Bridge:
            def send(self, request):
                return {"ok": True, "state": engine.send(request)}
        class Adapter:
            session_id = engine.state["session_id"]
            state = engine.state
            _restricted_lot = None
            def _record_id(self, source):
                return str(source)
        adapter = Adapter()
        adapter.engine = Bridge()
        native = {"kind": "cast", "card": lions, "targets": [], "plan": [
            {"source": forest, "ability": 0}, {"source": grotto, "ability": 1, "adding": [["W"]]}]}
        do = {"kind": "cast", "pay": {"abilities": [
            {"source": str(forest), "ability": {"rule": "305.6"}, "adding": ["G"]},
            {"source": str(grotto), "ability": {"face": 0, "paragraph": 2}, "adding": ["W"], "pay": {"pool": {"G": 1}}}]}}
        move, proof = KarnAdapter._explicit_payment(adapter, 0, native, do)
        self.assertEqual(move["plan"][1]["mana"], {"G": 1})
        self.assertEqual(move["mana"], {"W": 1})
        self.assertEqual(proof["payments"][1]["selected"]["G"], 1)
        self.assertEqual(proof["payments"][-1]["selected"]["W"], 1)
        self.assertEqual(engine.state["stack"], [], "quotes did not execute the live action")
        engine.ask({"cmd": "act", "player": 0, "move": move, "payment_expect": proof})
        self.assertEqual(len(engine.state["stack"]), 1)
        self.assertTrue(all(engine.state["objects"][str(o)]["tapped"] for o in (forest, grotto)))
        self.assertTrue(all(not n for c, n in engine.state["players"][0]["pool"].items() if c != "restricted"))


class RecursiveScopedBoundaryTests(unittest.TestCase):
    def test_child_allocation_authenticates_new_restricted_units_and_printed_indices(self):
        # 106.6, 601.2g/h: the inner producer adds C restricted to ability
        # costs; that C pays the filter's {1}, then its W pays the outer {W}.
        unit = {"mana": "C", "source": 1}
        lot = {"type": "C", "restriction": "Spend this mana only to activate abilities."}
        move = {"kind": "cast", "card": 10, "mana": {"W": 1}, "plan": [{"source": 2, "ability": 0,
            "mana": {"restricted_after": [0]}, "plan": [{"source": 1, "ability": 0, "mana": {}}]}]}
        empty = {"restricted": []}
        pooled = {"restricted": [unit]}
        def receipt(path, source, kind, pool, left, symbols, selected):
            return {"path": path, "source": source, "kind": kind, "player": 0, "prior_pool": empty,
                "pool": pool, "left": left, "requirement": {"symbols": symbols}, "selected": selected,
                "symbol_indices": list(range(len(symbols)))}
        receipts = [receipt([0,0],1,"ability",empty,empty,[],empty),
                    receipt([0],2,"ability",pooled,empty,["1"],pooled),
                    receipt([],20,"spell",{"W":1},empty,["W"],{"W":1})]
        events = [{"event":"activated","source":1,"ability":None},
            {"event":"mana_added","source":1,"player":0,"mana":["C"]},
            {"event":"activated","source":2,"ability":None},
            {"event":"mana_added","source":2,"player":0,"mana":["W"]}, {"event":"cast","spell":20}]
        quote = {"move": move, "complete": True, "events": events, "prior_pools": [empty,empty],
            "pools": [empty,empty], "payments": receipts, "symbol_indices":[0], "productions":[
                {"path":[0,0],"source":1,"player":0,"activation":0,"triggered":False,"sequence":1,
                 "invocation":{"source":1,"declaration_index":0},"mana":["C"],"before":empty,"after":pooled},
                {"path":[0],"source":2,"player":0,"activation":0,"triggered":False,"sequence":3,
                 "invocation":{"source":2,"declaration_index":0},"mana":["W"],"before":empty,"after":{"W":1}}]}
        pay = {"abilities":[{"source":"filter","ability":{"face":0,"paragraph":1},"adding":["W"],"pay":{
            "abilities":[{"source":"producer","ability":{"face":0,"paragraph":1},"adding":["C"]}],
            "allocation":[{"symbol":0,"mana":"C","restricted":lot}]}}]}
        self.assertEqual(select_payment(pay, empty, quote, restricted_lot=lambda unit:lot), {"W":1})
        production = validate_production(pay, quote)
        validate_bound_payment(quote,move,0,production)
        changed = deepcopy(quote)
        changed["payments"][1]["symbol_indices"] = None
        with self.assertRaisesRegex(PaymentEvidenceError,"printed-symbol"):
            select_payment(pay,empty,changed,restricted_lot=lambda unit:lot)
