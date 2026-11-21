"""CR 601.2g–h/602.2b/605.3: authored nested windows and their exact receipts.

Expectations come from Grotto's {1},{T}: Add one mana of any color and
Vanguard's {W}, and Prismite's repeatable {2}: Add one mana of any color.
"""
from copy import deepcopy, copy
import json
from pathlib import Path
import unittest

from mgn.adapters.karn import KarnAdapter, Refusal
from mgn.native_actions import native_action
from mgn.payments import (PaymentEvidenceError, invert_payment_quote, restore_payment,
                          validate_bound_payment, validate_production)
from test_mgn_adapter import ENGINE, MAIN_A, ORACLE, PRIORITY_A, no_random, position_record, setup_of


@unittest.skipUnless(ENGINE.is_file(), "build the stamped engine")
class RecursivePaymentTests(unittest.TestCase):
    def setUp(self):
        self.adapter = KarnAdapter(ENGINE, ORACLE)
        self.addCleanup(self.adapter.end)

    def nested(self):
        draft = json.loads((Path(__file__).resolve().parents[2] / "tests/fixtures/mgn-drafts/nested-filter-payment.json").read_text())
        self.adapter.setup(setup_of(draft))
        return deepcopy(draft["transitions"][0]["do"])

    def bind(self, action):
        return self.adapter._explicit_payment(0, self.adapter._move(action), action)

    def test_frozen_nested_choices_execute_and_roundtrip_without_pool_inference(self):
        action = self.nested()
        move, proof = self.bind(action)
        self.assertEqual([r["path"] for r in proof["payments"]], [[0, 0], [0], []])
        self.assertEqual([r["path"] for r in proof["productions"]], [[0, 0], [0]])
        self.assertEqual(move["plan"][0]["plan"][0]["mana"], {})
        self.assertEqual(move["plan"][0]["mana"], {"G": 1})
        self.assertEqual(move["mana"], {"W": 1})
        before = deepcopy(self.adapter.state["players"][0]["pool"])
        reply = self.adapter.act(1, "A", action, no_random)
        self.assertEqual(reply["state"]["players"]["A"]["mana"], {})
        self.assertTrue(reply["state"]["objects"]["forest"]["tapped"])
        self.assertTrue(reply["state"]["objects"]["grotto"]["tapped"])
        self.assertEqual(reply["state"]["zones"]["stack"], ["vanguard"])
        following = self.adapter.state
        args = (move, before, following["players"][0]["pool"], following["events"], action["pay"], 0)
        self.assertEqual(restore_payment({"pay": action["pay"], "quote": proof}, *args, record_id=self.adapter._record_id), action["pay"])
        self.assertEqual(invert_payment_quote(proof, *args, record_id=self.adapter._record_id), action["pay"])

    def test_scopes_production_and_child_selectors_are_authenticated(self):
        action = self.nested()
        move, proof = self.bind(action)
        for mutate in (
            lambda q: q["payments"].pop(0),
            lambda q: q["payments"].append(deepcopy(q["payments"][0])),
            lambda q: q["payments"][0].update(path=[0]),
            lambda q: q["payments"][1].update(prior_pool={"G": 1}),
            lambda q: q["productions"][0].update(path=[0]),
            lambda q: q["productions"][0].update(triggered=True),
            lambda q: q["productions"][0].update(activation=99),
            lambda q: q["move"]["plan"][0].update(mana={"R": 1}),
        ):
            changed = deepcopy(proof)
            mutate(changed)
            with self.subTest(changed=changed), self.assertRaises(PaymentEvidenceError):
                production = validate_production(action["pay"], changed)
                validate_bound_payment(changed, changed["move"], 0, production)
        changed = deepcopy(proof)
        changed["productions"][0]["path"] = [0]
        response = self.adapter.engine.send({"cmd": "act", "session_id": self.adapter.session_id,
            "request_id": self.adapter.state["request_id"], "player": 0, "move": move, "payment_expect": changed})
        self.assertFalse(response["ok"], "changed transaction evidence refuses before live costs")
        self.adapter.act(1, "A", action, no_random)

    def test_repeated_producer_uses_distinct_receipts_and_inverse_scopes(self):
        record = position_record({"prism": ("Prismite", "A"), "lion": ("Silvercoat Lion", "A")},
                                 {"A.hand": ["lion"], "battlefield": ["prism"]}, MAIN_A, PRIORITY_A)
        record["start"]["state"]["players"]["A"]["mana"] = {"C": 4}
        self.adapter.setup(setup_of(record))
        step = {"source": "prism", "ability": {"face": 0, "paragraph": 1}, "adding": ["W"], "pay": {"pool": {"C": 2}}}
        action = {"kind": "cast", "card": "lion", "pay": {"abilities": [deepcopy(step), deepcopy(step)]}}
        move, proof = self.bind(action)
        self.assertEqual([r["path"] for r in proof["productions"]], [[0], [1]])
        self.assertEqual([r["mana"] for r in proof["productions"]], [["W"], ["W"]])
        before = deepcopy(self.adapter.state["players"][0]["pool"])
        reply = self.adapter.act(1, "A", action, no_random)
        self.assertEqual(reply["state"]["players"]["A"]["mana"], {})
        following = self.adapter.state
        self.assertEqual(invert_payment_quote(proof, move, before, following["players"][0]["pool"],
            following["events"], action["pay"], 0, record_id=self.adapter._record_id), action["pay"])

    def test_forced_triggered_production_remains_distinct_and_pays_during_casting(self):
        # CR 601.2g/h, 605.4a: Forest adds G; Bloom separately adds G.
        # Both pay Bears' 1G inside casting. No priority activation is inserted.
        record = position_record({"forest": ("Forest", "A"), "bloom": ("Vernal Bloom", "A"),
                                  "bears": ("Grizzly Bears", "A")},
                                 {"A.hand": ["bears"], "battlefield": ["forest", "bloom"]}, MAIN_A, PRIORITY_A)
        self.adapter.setup(setup_of(record))
        before = self.adapter._reply_state()
        action = {"kind": "cast", "card": "bears", "pay": {"abilities": [
            {"source": "forest", "ability": {"rule": "305.6"}, "adding": ["G"]}]}}
        move, proof = self.bind(action)
        self.assertEqual(self.adapter._reply_state(), before, "quoting cannot tap or spend live resources")
        self.assertEqual(move["mana"], {"G": 2})
        rows = proof["productions"]
        self.assertEqual([r["triggered"] for r in rows], [False, True])
        self.assertEqual([self.adapter._record_id(r["source"]) for r in rows], ["forest", "bloom"])
        self.assertEqual([r["mana"] for r in rows], [["G"], ["G"]])
        for change,reason in ((lambda q: q["productions"].pop(),"production receipts disagree with the captured mana events"),
                       (lambda q: q["productions"][1].update(triggered=False),"names another activation invocation"),
                       (lambda q: q["productions"][1]["invocation"].update(source=999),"lacks its actual producing invocation"),
                       (lambda q: q["productions"][1].update(path=[99]),"lacks its explicit execution scope or trigger classification")):
            forged = deepcopy(proof)
            change(forged)
            with self.assertRaisesRegex(PaymentEvidenceError,reason):
                validate_production(action["pay"], forged)
        inverse=copy(self.adapter)
        inverse.state,inverse.audit=deepcopy(self.adapter.state),deepcopy(self.adapter.audit)
        reply = self.adapter.act(1, "A", action, no_random)
        self.assertEqual(native_action(inverse,move,{"state":self.adapter.state,"audit":self.adapter.audit,
            "payment_selection":{"pay":action["pay"],"quote":proof}}),action)
        self.assertEqual(reply["state"]["players"]["A"]["mana"], {})
        self.assertEqual(reply["state"]["zones"]["stack"], ["bears"])
        self.assertTrue(reply["state"]["objects"]["forest"]["tapped"])
        self.assertEqual(invert_payment_quote(proof, move, {}, self.adapter.state["players"][0]["pool"],
            self.adapter.state["events"], action["pay"], 0, record_id=self.adapter._record_id), action["pay"])

    def test_one_unoffered_answer_does_not_remove_another_sources_required_choice(self):
        # Verge's conditional U is fixed production; Canal independently offers
        # U or R. CR 605.3b/608.2d: supplying the recorded U to Canal is required,
        # while Verge must not receive an answer its instruction does not offer.
        record = position_record({"verge": ("Riverpyre Verge", "A"), "canal": ("Spirebluff Canal", "A"),
                                  "island": ("Island", "A"), "merfolk": ("Coral Merfolk", "A")},
                                 {"A.hand": ["merfolk", "canal"], "battlefield": ["verge", "island"]}, MAIN_A, PRIORITY_A)
        self.adapter.setup(setup_of(record))
        self.adapter.act(1, "A", {"kind": "play_land", "card": "canal"}, no_random)
        action = {"kind": "cast", "card": "merfolk", "pay": {"abilities": [
            {"source": "verge", "ability": {"face": 0, "paragraph": 2}, "adding": ["U"]},
            {"source": "canal", "ability": {"face": 0, "paragraph": 2}, "adding": ["U"]}]}}
        native = self.adapter._move(action)
        native["plan"][1].pop("adding")
        move, proof = self.adapter._explicit_payment(0, native, action)
        self.assertNotIn("adding", move["plan"][0])
        self.assertEqual(move["plan"][1]["adding"], [["U"]])
        self.assertTrue(proof["complete"])
        self.assertEqual(proof["mana_answers"], [])
        reply = self.adapter.act(2, "A", action, no_random)
        self.assertEqual(reply["state"]["players"]["A"]["mana"], {})
        self.assertFalse(reply["state"]["objects"]["island"].get("tapped", False))

    def test_finite_self_recursion_refuses_even_when_its_recorded_pools_would_pay_every_cost(self):
        record = position_record({"prism": ("Prismite", "A"), "lion": ("Silvercoat Lion", "A")},
                                 {"A.hand": ["lion"], "battlefield": ["prism"]}, MAIN_A, PRIORITY_A)
        record["start"]["state"]["players"]["A"]["mana"] = {"C": 4}
        self.adapter.setup(setup_of(record))
        inner = {"source": "prism", "ability": {"face": 0, "paragraph": 1}, "adding": ["W"], "pay": {"pool": {"C": 2}}}
        outer = deepcopy(inner)
        outer["pay"]["abilities"] = [inner]
        before = self.adapter._reply_state()
        with self.assertRaisesRegex(Refusal, "605.3c"):
            self.adapter.act(1, "A", {"kind": "cast", "card": "lion", "pay": {"abilities": [outer]}}, no_random)
        self.assertEqual(self.adapter._reply_state(), before)

    def test_morph_uses_the_authenticated_face_down_cost_row_and_retains_that_announcement(self):
        # 702.37c/708.4: Morph is {3}, and the resulting spell is a colorless
        # nameless 2/2 with no abilities, before it is put on the stack.
        record = position_record({"angel": ("Exalted Angel", "A")}, {"A.hand": ["angel"]}, MAIN_A, PRIORITY_A)
        record["start"]["state"]["players"]["A"]["mana"] = {"C": 3}
        self.adapter.setup(setup_of(record))
        action = {"kind": "cast", "card": "angel", "cost": {"kind": "alternative", "keyword": "morph"},
                  "pay": {"pool": {"C": 3}}}
        native = self.adapter._move(action)
        self.assertEqual(native["cost"], {"alternative": 2})
        reply = self.adapter.act(1, "A", action, no_random)
        angel = reply["state"]["objects"]["angel"]
        self.assertTrue(angel["face_down"])
        self.assertEqual((angel["power"], angel["toughness"]), (2, 2))
        self.assertEqual(angel["cast"]["cost"], action["cost"])
        self.assertEqual(reply["state"]["players"]["A"]["mana"], {})
        self.assertNotIn("objects.angel.cast", reply.get("unsupported", []))

    def test_fixed_triggered_production_can_leave_one_generated_mana_unspent(self):
        # Llanowar Elves costs G, whereas Forest + Bloom produce GG under
        # CR 605.4a. CR 106.4 leaves the second G in the current pool.
        record=position_record({"forest":("Forest","A"),"bloom":("Vernal Bloom","A"),
            "elf":("Llanowar Elves","A")},{"A.hand":["elf"],"battlefield":["forest","bloom"]},MAIN_A,PRIORITY_A)
        self.adapter.setup(setup_of(record))
        action={"kind":"cast","card":"elf","pay":{"abilities":[
            {"source":"forest","ability":{"rule":"305.6"},"adding":["G"]}]}}
        move,proof=self.bind(action)
        self.assertEqual(move["mana"],{"G":1})
        self.assertEqual([r["mana"] for r in proof["productions"]],[["G"],["G"]])
        reply=self.adapter.act(1,"A",action,no_random)
        self.assertEqual(reply["state"]["players"]["A"]["mana"],{"G":1})
        self.assertEqual(reply["state"]["zones"]["stack"],["elf"])
