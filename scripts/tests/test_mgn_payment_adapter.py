"""Forward payment choices retain resource selection (106.6, 601.2f–h)."""
import unittest
from copy import copy, deepcopy

from mgn.adapters.karn import KarnAdapter, Refusal
from mgn.native_actions import native_action
from mgn.payments import PaymentEvidenceError, validate_bound_payment, validate_production
from test_mgn_adapter import ENGINE, ORACLE, MAIN_A, PRIORITY_A, position_record, setup_of, no_random


@unittest.skipUnless(ENGINE.is_file(), "build the stamped engine")
class PaymentAdapterTests(unittest.TestCase):
    def setUp(self):
        self.adapter = KarnAdapter(ENGINE, ORACLE)

    def tearDown(self):
        self.adapter.end()

    def setup_bears(self):
        record = position_record(
            {"bears": ("Grizzly Bears", "A"), "forest": ("Forest", "A")},
            {"A.hand": ["bears"], "battlefield": ["forest"]}, MAIN_A, PRIORITY_A)
        record["start"]["state"]["players"]["A"]["mana"] = {"R": 1, "G": 1}
        self.adapter.setup(setup_of(record))

    def test_mixed_payment_spends_stated_prior_mana_and_leaves_other_prior_mana(self):
        self.setup_bears()
        reply = self.adapter.act(1, "A", {"kind": "cast", "card": "bears", "pay": {
            "pool": {"R": 1}, "abilities": [{"source": "forest", "ability": {"rule": "305.6"}, "adding": ["G"]}],
            "allocation": [{"symbol": 0, "mana": "R"}, {"symbol": 1, "mana": "G"}]}}, no_random)
        self.assertEqual(reply["state"]["players"]["A"]["mana"], {"G": 1})
        self.assertEqual(reply["state"]["zones"]["stack"], ["bears"])
        self.assertTrue(reply["state"]["objects"]["forest"]["tapped"])

    def test_omitted_prior_spending_does_not_authorize_automatic_payment(self):
        self.setup_bears()
        before = self.adapter._reply_state()
        with self.assertRaisesRegex(Refusal, "do not pay"):
            self.adapter.act(1, "A", {"kind": "cast", "card": "bears", "pay": {
                "abilities": [{"source": "forest", "ability": {"rule": "305.6"}, "adding": ["G"]}]}}, no_random)
        self.assertEqual(self.adapter._reply_state(), before)

    def test_mana_choice_uses_types_not_native_vector_order(self):
        self.adapter.state = {"asked": {"kind": "mana", "options": [["U", "U", "R", "R"]]}}
        self.assertEqual(self.adapter._move({"kind": "choose_mana", "mana": ["R", "U", "U", "R"]}),
                         {"kind": "choose", "answer": {"mana": ["U", "U", "R", "R"]}})

    def test_conditional_fixed_output_is_verified_without_an_unoffered_choice(self):
        # Gemstone Caverns has no luck counter, so its {T} ability adds only C.
        # CR 605.3b: the inactive any-color clause offers no mana choice.
        record = position_record({"caverns": ("Gemstone Caverns", "A")},
                                 {"A.hand": ["caverns"]}, MAIN_A, PRIORITY_A)
        self.adapter.setup(setup_of(record))
        self.adapter.act(1, "A", {"kind": "play_land", "card": "caverns"}, no_random)
        action = {"kind": "activate", "source": "caverns", "ability": {"face": 0, "paragraph": 2}, "adding": ["R"]}
        before = self.adapter._reply_state()
        with self.assertRaisesRegex(Refusal, "output differs"):
            self.adapter.act(1, "A", action, no_random)
        self.assertEqual(self.adapter._reply_state(), before)
        action["adding"] = ["C"]
        reply = self.adapter.act(3, "A", action, no_random)
        self.assertEqual(reply["state"]["players"]["A"]["mana"], {"C": 1})
        self.assertTrue(reply["state"]["objects"]["caverns"]["tapped"])

    def test_absent_pay_also_forbids_implicit_pool_spending(self):
        self.setup_bears()
        before = self.adapter._reply_state()
        with self.assertRaisesRegex(Refusal, "do not pay"):
            self.adapter.act(1, "A", {"kind": "cast", "card": "bears"}, no_random)
        self.assertEqual(self.adapter._reply_state(), before)

    def test_printed_nonkeyword_alternative_cost_names_its_actual_source(self):
        # Fireblast's printed alternative replaces {4}{R}{R} with sacrificing
        # two Mountains (118.9, 601.2b/h). It is not a keyword ability.
        record = position_record({"blast": ("Fireblast", "A"), "m1": ("Mountain", "A"), "m2": ("Mountain", "A")},
                                 {"A.hand": ["blast"], "battlefield": ["m1", "m2"]}, MAIN_A, PRIORITY_A)
        self.adapter.setup(setup_of(record))
        action = {"kind": "cast", "card": "blast", "targets": [["B"]],
                  "cost": {"kind": "alternative", "source": "m1", "ability": {"face": 0, "paragraph": 1}},
                  "pay": {"objects": [["m1", "m2"]]}}
        before = self.adapter._reply_state()
        with self.assertRaisesRegex(Refusal, "matches 0"):
            self.adapter.act(1, "A", action, no_random)
        self.assertEqual(self.adapter._reply_state(), before)
        action["cost"]["source"] = "blast"
        # Mana is fully selected, but the simultaneous sacrifice still waits for
        # the owner's explicit graveyard order (404.3). Quotes mutate no live state.
        move, proof = self.adapter._explicit_payment(0, self.adapter._move(action), action)
        self.assertEqual(self.adapter._reply_state(), before)
        self.assertFalse(proof["complete"])
        self.assertEqual(proof["suspended"]["asked"]["kind"], "order_cards")
        self.assertFalse(proof["suspended"]["announced"]["cast"]["completed"])
        for mutate in (lambda q: q["suspended"].update(source=999),
                       lambda q: q["suspended"]["asked"].update(kind="objects"),
                       lambda q: q["payments"].clear(),
                       lambda q: q["suspended"].pop("announced")):
            forged = deepcopy(proof)
            mutate(forged)
            with self.assertRaises(PaymentEvidenceError):
                validate_bound_payment(forged, move, 0, validate_production(action["pay"], forged))
            denied = self.adapter.engine.send({"cmd": "act", "session_id": self.adapter.session_id,
                "request_id": self.adapter.state["request_id"], "player": 0,
                "move": move, "payment_expect": forged})
            self.assertFalse(denied["ok"])
            self.assertEqual(denied["error"], "payment quote changed or is incomplete; action was not applied")
            self.assertEqual(self.adapter._reply_state(), before)
        inverse = copy(self.adapter)
        inverse.state, inverse.audit = deepcopy(self.adapter.state), deepcopy(self.adapter.audit)
        reply = self.adapter.act(1, "A", action, no_random)
        captured = {"state": deepcopy(self.adapter.state), "audit": deepcopy(self.adapter.audit),
                    "payment_selection": {"pay": action["pay"], "quote": proof}}
        self.assertEqual(native_action(inverse, move, captured), action)
        forged_capture = deepcopy(captured)
        forged_capture["state"]["asked"]["cards"].reverse()
        with self.assertRaisesRegex(Refusal, "pending owner-order|pending order"):
            native_action(inverse, move, forged_capture)
        for mutate in (lambda state: state.update(events=[event for event in state["events"] if event.get("event") != "zone"]),
                       lambda state: next(event for event in state["events"] if event.get("event") == "zone").update(was=999),
                       lambda state: next(event for event in state["events"] if event.get("event") == "zone").update(now=999),
                       lambda state: next(event for event in state["events"] if event.get("event") == "zone").update(**{"from": "Library"}),
                       lambda state: state["events"].append(deepcopy(next(event for event in state["events"] if event.get("event") == "zone")))):
            forged_capture = deepcopy(captured)
            mutate(forged_capture["state"])
            with self.subTest(events=forged_capture["state"]["events"]), self.assertRaisesRegex(Refusal, "captured input-to-stack transition"):
                native_action(inverse, move, forged_capture)
        self.assertEqual(reply["state"]["objects"]["blast"]["cast"]["cost"], action["cost"])
        self.assertEqual(reply["state"]["pending"]["kind"], "order_cards")
        self.assertEqual(reply["state"]["zones"]["A.graveyard"], [])
        self.assertCountEqual(reply["state"]["zones"]["battlefield"], ["m1", "m2"])
        suspended = self.adapter._reply_state()
        for seat, order, reason in [("B", ["m2", "m1"], "request belongs to player 0, not 1"),
                ("A", ["m1", "m1"], "order every offered card exactly once (401.4, 404.3)"),
                ("A", ["m1"], "order every offered card exactly once (401.4, 404.3)")]:
            with self.subTest(seat=seat, order=order), self.assertRaises(Refusal) as denied:
                self.adapter.act(2, seat, {"kind": "order_cards", "order": order}, no_random)
            self.assertEqual(str(denied.exception), reason)
            self.assertEqual(self.adapter._reply_state(), suspended)
        reply = self.adapter.act(3, "A", {"kind": "order_cards", "order": ["m2", "m1"]}, no_random)
        # State lists the graveyard oldest first (404.2); authored order is top first.
        self.assertEqual(reply["state"]["zones"]["A.graveyard"], ["m1", "m2"])
        self.assertEqual(reply["state"]["zones"]["stack"], ["blast"])
        spell = self.adapter._engine_id("blast")
        completed = self.adapter.audit["objects"][str(spell)]["announced"]
        self.assertTrue(completed["cast"]["completed"])
        self.assertEqual(completed["costs"], proof["suspended"]["announced"]["costs"])
        self.assertEqual(completed["cast"]["payment"], proof["suspended"]["substitutions"])
        self.assertEqual(reply["state"]["players"]["A"]["mana"], {})
