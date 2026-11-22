"""Inverse decision expectations derived from MGN 3.3, 5.2–5.8, not engine output.

These compact frames are deliberate evidence: changing an engine's auto-payer
must not change the meaning of captured payment or manufacture a permission.
"""
from copy import deepcopy
import unittest

from mgn.adapters.karn import Refusal, KarnAdapter
from mgn.native_actions import native_action


class Adapter:
    _asked_kind = KarnAdapter._asked_kind
    seats = ["A", "B"]

    def __init__(self, kind="priority"):
        self.cards = {}
        self.state = {"begun": True, "asked": {"kind": kind, "player": 0},
                      "players": [{"pool": {}}, {"pool": {}}], "objects": {}}
        self.audit = {"objects": {"1": {"abilities": [
            {"index": 0, "text": "{T}: Add {G}.", "origin": {"kind": "rule"}}]},
            "2": {"abilities": [{"index": 2, "paragraph": 3, "text": "{1}: Draw a card.", "origin": {"kind": "own"}}]},
            "3": {"casting_costs": {"alternatives": [{"paragraph": 2, "text": "Evoke {1}{G}"}]}}}}

    def _record_id(self, n):
        return f"c{n}"

    def _target_record(self, t):
        return self.seats[t["player"]] if "player" in t else self._record_id(t["object"])

    def _ability_ref(self, row, source):
        return {"rule": "305.6"} if row["origin"]["kind"] == "rule" else {"face": 0, "paragraph": row["paragraph"]}


def after(events=(), pool=None):
    return {"state": {"players": [{"pool": pool or {}}, {"pool": {}}], "events": list(events)}}


class NativeActionTests(unittest.TestCase):
    def setUp(self):
        self.a = Adapter()

    def call(self, move, following=None, pending=None):
        return native_action(self.a, move, following or after(), pending)

    def refuses(self, move, text, following=None, pending=None):
        with self.assertRaises(Refusal) as cm:
            self.call(move, following, pending)
        self.assertEqual(cm.exception.class_, "unsupported")
        self.assertIn(text, str(cm.exception))

    def test_priority_land_and_null_pass(self):
        self.assertEqual(self.call({"kind": "pass"}), {"kind": "pass"})
        self.assertEqual(self.call({"kind": "play", "card": 3, "face": 1}), {"kind": "play_land", "card": "c3", "face": 1})

    def test_owner_arrival_order_uses_the_captured_permutation_before_consumption(self):
        self.a.cards = {"c20": {"owner": "A"}, "c10": {"owner": "A"}}
        for zone in ("graveyard", "library"):
            self.a.state["asked"] = {"kind": "order_cards", "player": 0, "cards": [20, 10], "to": {"zone": zone}}
            self.assertEqual(self.call({"kind": "choose", "answer": {"objects": [10, 20]}}),
                {"kind": "order_cards", "order": ["c10", "c20"]})
            for invalid in ([20, 20], [10], [10, 30]):
                self.refuses({"kind": "choose", "answer": {"objects": invalid}}, "not a permutation")
        self.a.cards["c10"]["owner"] = "B"
        self.refuses({"kind": "choose", "answer": {"objects": [10, 20]}}, "ordering owner")

    def test_generic_selection_or_mismatched_pending_cannot_supply_owner_order(self):
        self.a.state["asked"] = {"kind": "objects", "player": 0, "among": [20, 10], "count": 2}
        self.refuses({"kind": "choose", "answer": {"objects": [10, 20]}}, "authenticated native",
                     pending={"kind": "order_cards"})

    def test_cast_announcements_are_explicit_and_one_based(self):
        self.a.state["players"][0]["pool"] = {"R": 1}
        move = {"kind": "cast", "card": 3, "x": 0, "modes": [0, 2], "targets": [[{"player": 1}], [{"object": 2}]],
                "division": [[2, 1]], "kicks": 2, "bargain": True, "gift": "p1", "chose": [0, 1],
                "paying": [[2]], "life": [True, False], "mana": {"R": 1, "G": 1, "B": 0}, "plan": [{"source": 1, "ability": 0}]}
        self.assertEqual(self.call(move), {"kind": "cast", "card": "c3", "x": 0, "modes": [1, 3],
            "targets": [["B"], ["c2"]], "division": [[2, 1]],
            "additional": {"kicker": 2, "paid": ["bargain"], "gift": "B", "choices": [1, 2]},
            "pay": {"abilities": [{"source": "c1", "ability": {"rule": "305.6"}, "adding": ["G"]}],
                    "pool": {"R": 1}, "objects": [["c2"]], "life": [True, False]}})

    def test_pool_spending_is_before_pool_only(self):
        self.a.state["players"][0]["pool"] = {"R": 2, "G": 1}
        self.assertEqual(self.call({"kind": "cast", "card": 3}, after(pool={"R": 1}))["pay"], {"pool": {"R": 1, "G": 1}})

    def test_ambiguous_overlap_leftover_and_restricted_mana_refuse(self):
        self.a.state["players"][0]["pool"] = {"G": 1}
        self.refuses({"kind": "cast", "card": 3, "plan": [{"source": 1, "ability": 0}]}, "ambiguous", after(pool={"G": 1}))
        self.refuses({"kind": "cast", "card": 3, "mana": {"restricted": [0]}}, "allocation")

    def test_explicit_native_mana_is_total_spending_after_the_plan(self):
        move = {"kind": "cast", "card": 3, "plan": [{"source": 1, "ability": 0}], "mana": {"G": 1}}
        self.assertEqual(self.call(move)["pay"], {"abilities": [{"source": "c1", "ability": {"rule": "305.6"}, "adding": ["G"]}]})
        self.a.state["players"][0]["pool"] = {"R": 1}
        result = self.call({**move, "mana": {"G": 1, "R": 1}})
        self.assertEqual(result["pay"]["pool"], {"R": 1})
        self.a.state["players"][0]["pool"] = {"G": 1}
        self.refuses(move, "ambiguous", after(pool={"G": 1}))
        self.assertEqual(self.call({**move, "mana": {"G": 2}})["pay"]["pool"], {"G": 1})
        self.refuses({**move, "mana": {"G": 3}}, "disagrees")

    def rich_payment(self):
        self.a.state["players"][0]["pool"] = {"G": 1}
        move = {"kind": "cast", "card": 3, "plan": [{"source": 1, "ability": 0}], "mana": {"G": 1}}
        events = [{"event": "activated", "source": 1, "ability": None},
                  {"event": "mana_added", "source": 1, "player": 0, "mana": ["G"]},
                  {"event": "cast", "spell": 4, "player": 0}]
        following = after(events, pool={"G": 1})
        pay = {"abilities": [{"source": "c1", "ability": {"rule": "305.6"}, "adding": ["G"]}]}
        following["payment_selection"] = {"pay": pay, "quote": {
            "move": deepcopy(move), "complete": True, "events": deepcopy(events), "symbol_indices": [0],
            "prior_pools": [{"G": 1}, {}], "pools": [{"G": 1}, {}],
            "payments": [{"player": 0, "kind": "ability", "source": 1, "pool": {"G": 1}, "left": {"G": 1}},
                         {"player": 0, "kind": "spell", "source": 4, "pool": {"G": 2}, "left": {"G": 1}, "selected": {"G": 1},
                          "requirement": {"symbols": ["G"]}}]}}
        return move, following

    def native_quote(self, explicit=False):
        move, following = self.rich_payment()
        following["payment_quote"] = following.pop("payment_selection")["quote"]
        if not explicit:
            del move["mana"]
            following["payment_quote"]["move"] = deepcopy(move)
            following["payment_quote"]["payments"][-1]["selected"] = None
        return move, following

    def test_native_quote_canonicalizes_fungible_prior_units_with_leftovers(self):
        # CR 106.4: identical unrestricted G has no persistent producer/age tag.
        # One G spent from two G leaves one G regardless of the canonical tag.
        for explicit in (False, True):
            move, following = self.native_quote(explicit)
            frozen = deepcopy((move, following, self.a.state))
            self.assertEqual(self.call(move, following)["pay"]["pool"], {"G": 1})
            self.assertEqual((move, following, self.a.state), frozen)

    def test_native_quote_uses_acting_player_when_controlling_another_turn(self):
        move, following = self.native_quote()
        self.a.state["asked"].update(player=1, acting=0)
        self.a.state["players"][1]["pool"] = {"R": 3}
        following["state"]["players"][1]["pool"] = {"R": 3}
        following["payment_quote"]["prior_pools"][1] = {"R": 3}
        following["payment_quote"]["pools"][1] = {"R": 3}
        self.assertEqual(self.call(move, following)["pay"]["pool"], {"G": 1})

    def test_native_quote_checks_action_actor_frames_events_and_selected_spend(self):
        mutations = [
            (lambda q: q.update(complete=False), "complete"),
            (lambda q: q["move"].update(card=2), "exact native move"),
            (lambda q: q["payments"][-1].update(player=1), "another payer"),
            (lambda q: q["pools"].__setitem__(0, {}), "before/after pools"),
            (lambda q: q["events"][0].update(source=2), "payment events"),
            (lambda q: q["payments"][-1].update(selected={"R": 1}), "spending conservation"),
            (lambda q: q["payments"][0].update(selected={"G": 1}), "selected resources"),
            (lambda q: q["payments"][-1].update(requirement={"symbols": ["R"]}), "locked cost"),
            (lambda q: q["payments"][-1].update(left={"G": 2}), "consumed mana outside"),
        ]
        for mutation, reason in mutations:
            with self.subTest(reason=reason):
                move, following = self.native_quote()
                mutation(following["payment_quote"])
                self.refuses(move, reason, following)

    def test_native_quote_refuses_restricted_lot_without_portable_origin(self):
        move, following = self.native_quote()
        unit = {"mana": "G", "source": 1}
        self.a.state["players"][0]["pool"] = {"restricted": [unit]}
        following["state"]["players"][0]["pool"] = {"restricted": [unit]}
        q = following["payment_quote"]
        q["prior_pools"][0] = {"restricted": [unit]}
        q["pools"][0] = {"restricted": [unit]}
        q["payments"][0].update(pool={"restricted": [unit]}, left={"restricted": [unit]})
        q["payments"][-1].update(pool={"G": 1, "restricted": [unit]}, left={"restricted": [unit]})
        self.refuses(move, "authenticated Oracle", following)

    def test_native_quote_spends_generated_restricted_unit_with_plain_same_type_leftover(self):
        # CR 106.6: a creature-only G spent on the creature cannot be replaced by
        # an unspent unrestricted G in the authored payment's semantic identity.
        self.a._restricted_lot = lambda unit: {"type": unit["mana"], "restriction": "Spend this mana only to cast a creature spell.",
            "producer": {"source": "c1", "ability": {"face": 0, "paragraph": 1}}}
        move, following = self.native_quote()
        unit = {"mana": "G", "source": 1}
        q = following["payment_quote"]
        q["payments"][-1].update(pool={"G": 1, "restricted": [unit]}, left={"G": 1})
        pay = self.call(move, following)["pay"]
        self.assertNotIn("pool", pay)
        self.assertEqual(pay["allocation"][0]["restricted"]["producer"]["source"], "c1")
        self.assertEqual(pay["allocation"][0]["symbol"], 0)

    def test_native_quote_spends_prior_restricted_unit_and_plain_unit_separately(self):
        self.a._restricted_lot = lambda unit: {"type": unit["mana"], "restriction": "Spend this mana only to cast a creature spell."}
        move, following = self.native_quote()
        unit = {"mana": "G", "source": 1}
        self.a.state["players"][0]["pool"] = {"restricted": [unit]}
        q = following["payment_quote"]
        q["prior_pools"][0] = {"restricted": [unit]}
        q["pools"][0] = {}
        q["payments"][0].update(pool={"restricted": [unit]}, left={"restricted": [unit]})
        q["payments"][-1].update(pool={"G": 1, "restricted": [unit]}, left={}, requirement={"symbols": ["1", "G"]})
        q["symbol_indices"] = [0, 1]
        following["state"]["players"][0]["pool"] = {}
        pay = self.call(move, following)["pay"]
        self.assertEqual(pay["pool_restricted"][0]["count"], 1)
        self.assertEqual(sum("restricted" in item for item in pay["allocation"]), 1)

    def test_native_quote_keeps_main_mana_output_after_cost_payment(self):
        # CR 605.3: spend the prior G before this ability produces GG.
        self.a.state["players"][0]["pool"] = {"G": 1}
        move = {"kind": "activate", "source": 1, "ability": 0}
        events = [{"event": "activated", "source": 1, "ability": None},
                  {"event": "mana_added", "source": 1, "player": 0, "mana": ["G", "G"]}]
        following = after(events, {"G": 2})
        following["payment_quote"] = {"complete": True, "move": move, "events": events,
            "prior_pools": [{"G": 1}, {}], "pools": [{"G": 2}, {}], "symbol_indices": [0],
            "payments": [{"kind": "ability", "source": 1, "player": 0, "pool": {"G": 1},
                          "left": {}, "selected": None, "requirement": {"symbols": ["G"]}}]}
        self.assertEqual(self.call(move, following)["pay"], {"pool": {"G": 1}})

    def test_native_quote_records_generic_generated_color_allocation(self):
        # CR 107.4b: generic {1} can spend either type; retain the captured R.
        self.a.state["players"][0]["pool"] = {}
        move = {"kind": "cast", "card": 3, "plan": [{"source": 1, "ability": 0}]}
        events = [{"event": "activated", "source": 1, "ability": None},
                  {"event": "mana_added", "source": 1, "player": 0, "mana": ["R", "G"]},
                  {"event": "cast", "spell": 4, "player": 0}]
        following = after(events, {"G": 1})
        following["payment_quote"] = {"complete": True, "move": move, "events": events,
            "prior_pools": [{}, {}], "pools": [{"G": 1}, {}], "symbol_indices": [0],
            "payments": [{"kind": "ability", "source": 1, "player": 0, "pool": {}, "left": {}},
                {"kind": "spell", "source": 4, "player": 0, "pool": {"G": 1, "R": 1}, "left": {"G": 1},
                 "selected": None, "requirement": {"symbols": ["1"]}}]}
        self.assertEqual(self.call(move, following)["pay"]["allocation"], [{"symbol": 0, "mana": "R"}])
        following["payment_quote"]["symbol_indices"] = None
        self.refuses(move, "printed-symbol allocation provenance", following)

    def test_native_quote_child_consumption_must_match_following_production(self):
        move, following = self.native_quote()
        following["payment_quote"]["payments"][0]["left"] = {}
        self.refuses(move, "production conservation", following)

    def test_authenticated_selection_preserves_same_type_prior_new_choice(self):
        move, following = self.rich_payment()
        self.assertNotIn("pool", self.call(move, following)["pay"])
        following["payment_selection"]["pay"]["pool"] = {"G": 1}
        self.assertEqual(self.call(move, following)["pay"]["pool"], {"G": 1})
        del following["payment_selection"]
        self.refuses(move, "ambiguous", following)

    def test_authenticated_generated_restricted_selection_restores_allocation(self):
        move, following = self.rich_payment()
        self.a.state["players"][0]["pool"] = {}
        following["state"]["players"][0]["pool"] = {}
        move["mana"] = {"restricted_after": [0]}
        proof = following["payment_selection"]["quote"]
        proof["move"] = deepcopy(move)
        proof["prior_pools"][0] = {}
        proof["pools"][0] = {}
        restricted = {"restricted": [{"mana": "G", "source": 1, "uncounterable": False}]}
        proof["payments"][0] = {"player": 0, "kind": "ability", "source": 1, "pool": {}, "left": {}}
        proof["payments"][-1].update(pool=restricted, selected=restricted, left={})
        lot = {"type": "G", "restriction": "Spend this mana only to cast a creature spell."}
        self.a._restricted_lot = lambda unit: dict(lot)
        following["payment_selection"]["pay"]["allocation"] = [{"symbol": 0, "mana": "G", "restricted": lot}]
        self.assertEqual(self.call(move, following)["pay"]["allocation"], [{"symbol": 0, "mana": "G", "restricted": lot}])

    def test_controlled_turn_uses_actor_pool_in_legacy_and_rich_payments(self):
        self.a.state["asked"].update(player=1, acting=0)
        self.a.state["players"][0]["pool"] = {"G": 1}
        self.a.state["players"][1]["pool"] = {"R": 3}
        following = after()
        following["state"]["players"][1]["pool"] = {"R": 3}
        self.assertEqual(self.call({"kind": "cast", "card": 3, "mana": {"G": 1}}, following)["pay"], {"pool": {"G": 1}})
        move, following = self.rich_payment()
        self.a.state["asked"].update(player=1, acting=0)
        self.a.state["players"][1]["pool"] = {"R": 3}
        following["state"]["players"][1]["pool"] = {"R": 3}
        proof = following["payment_selection"]["quote"]
        proof["prior_pools"][1] = {"R": 3}
        proof["pools"][1] = {"R": 3}
        following["payment_selection"]["pay"]["pool"] = {"G": 1}
        self.assertEqual(self.call(move, following)["pay"]["pool"], {"G": 1})

    def test_rich_receipts_must_name_the_actor_and_actual_selected_resources(self):
        move, following = self.rich_payment()
        following["payment_selection"]["quote"]["payments"][-1]["player"] = 1
        self.refuses(move, "another payer", following)
        move, following = self.rich_payment()
        # Conserve the forged receipt internally while contradicting the native G selector.
        proof = following["payment_selection"]["quote"]
        proof["payments"][-1].update(pool={"G": 1, "R": 1}, selected={"R": 1}, left={"G": 1})
        self.refuses(move, "production conservation", following)
        move, following = self.rich_payment()
        proof = following["payment_selection"]["quote"]
        proof["payments"][-1]["selected"] = {"R": 1}
        self.refuses(move, "explicit native selector", following)

    def test_rich_receipt_boundaries_must_chain_from_outer_pools(self):
        move, following = self.rich_payment()
        proof = following["payment_selection"]["quote"]
        proof["payments"][0].update(pool={"R": 1}, left={"R": 1})
        self.refuses(move, "preceding boundary", following)
        move, following = self.rich_payment()
        proof = following["payment_selection"]["quote"]
        proof["payments"][-1].update(pool={"G": 3}, left={"G": 2}, selected={"G": 1})
        self.refuses(move, "production conservation", following)
        move, following = self.rich_payment()
        proof = following["payment_selection"]["quote"]
        proof["payments"][-1].update(pool={"G": 2}, left={}, selected={"G": 2})
        self.refuses(move, "production conservation", following)

    def test_rich_receipt_rejects_undeclared_trailing_or_unattributed_production(self):
        move, following = self.rich_payment()
        event = {"event": "mana_added", "source": 2, "player": 0, "mana": ["R"]}
        following["state"]["events"].append(event)
        following["payment_selection"]["quote"]["events"].append(deepcopy(event))
        self.refuses(move, "declared activation provenance", following)

    def test_main_mana_activation_output_is_separate_from_its_payment(self):
        move = {"kind": "activate", "source": 1, "ability": 0, "mana": {"G": 1}}
        self.a.audit["objects"]["1"]["abilities"][0]["text"] = "{G}: Add {G}{G}."
        self.a.state["players"][0]["pool"] = {"G": 1}
        events = [{"event": "activated", "source": 1, "ability": None},
                  {"event": "mana_added", "source": 1, "player": 0, "mana": ["G", "G"]}]
        following = after(events, pool={"G": 2})
        following["payment_selection"] = {"pay": {"pool": {"G": 1}}, "quote": {
            "move": deepcopy(move), "complete": True, "events": deepcopy(events), "symbol_indices": [0],
            "prior_pools": [{"G": 1}, {}], "pools": [{"G": 2}, {}],
            "payments": [{"player": 0, "kind": "ability", "source": 1, "pool": {"G": 1},
                          "selected": {"G": 1}, "left": {}, "requirement": {"symbols": ["G"]}}]}}
        self.assertEqual(self.call(move, following)["pay"], {"pool": {"G": 1}})
        following["payment_selection"]["quote"]["payments"][0]["left"] = {"G": 1}
        self.refuses(move, "production conservation", following)

    def test_authenticated_selection_refuses_naked_override_or_changed_move(self):
        move, following = self.rich_payment()
        following["payment_selection"] = {"pool": {"G": 1}}
        self.refuses(move, "original MGN pay", following)
        move, following = self.rich_payment()
        following["payment_selection"]["quote"]["move"]["mana"] = {"G": 2}
        self.refuses(move, "exact native move", following)

    def test_authenticated_selection_checks_captured_pools_and_events(self):
        move, following = self.rich_payment()
        following["payment_selection"]["quote"]["pools"][0] = {}
        self.refuses(move, "before/after pools", following)
        move, following = self.rich_payment()
        following["payment_selection"]["quote"]["events"][0]["source"] = 2
        self.refuses(move, "payment events", following)

    def test_authenticated_selection_checks_totals_and_conservation(self):
        move, following = self.rich_payment()
        following["payment_selection"]["quote"]["payments"][-1]["left"] = {}
        self.refuses(move, "conservation", following)
        move, following = self.rich_payment()
        following["payment_selection"]["pay"]["pool"] = {"G": 2}
        self.refuses(move, "exceeds", following)

    def test_mana_activation_includes_actual_production(self):
        events = [{"event": "activated", "source": 1, "ability": None}, {"event": "mana_added", "source": 1, "mana": ["G", "G"]}]
        self.assertEqual(self.call({"kind": "activate", "source": 1, "ability": 0}, after(events, pool={"G": 2})),
                         {"kind": "activate", "source": "c1", "ability": {"rule": "305.6"}, "adding": ["G", "G"]})

    def test_same_source_mana_payment_does_not_make_main_ability_a_mana_ability(self):
        self.a.audit["objects"]["1"]["abilities"].append({"index": 1, "paragraph": 2, "text": "{1}: This land becomes a creature.", "origin": {"kind": "own"}})
        following = after([{"event": "activated", "source": 1, "ability": None},
                           {"event": "mana_added", "source": 1, "mana": ["C"]},
                           {"event": "activated", "source": 1, "ability": 5}])
        result = self.call({"kind": "activate", "source": 1, "ability": 1,
                            "plan": [{"source": 1, "ability": 0}]}, following)
        self.assertNotIn("adding", result)
        self.assertEqual(result["pay"]["abilities"][0]["adding"], ["C"])

    def test_nonmana_activation_and_object_payment(self):
        self.assertEqual(self.call({"kind": "activate", "source": 2, "ability": 2, "paying": [[3]]}),
            {"kind": "activate", "source": "c2", "ability": {"face": 0, "paragraph": 3}, "pay": {"objects": [["c3"]]}})

    def test_variable_mana_requires_exact_event(self):
        self.a.audit["objects"]["1"]["abilities"][0]["text"] = "{T}: Add one mana of any color."
        move = {"kind": "cast", "card": 3, "plan": [{"source": 1, "ability": 0, "adding": [["R"]]}]}
        self.refuses(move, "mana_added")
        result = self.call(move, after([{"event": "mana_added", "source": 1, "mana": ["R"]}]))
        self.assertEqual(result["pay"]["abilities"][0]["adding"], ["R"])

    def test_nested_object_payment_and_positional_cost_choices_are_recorded(self):
        move = {"kind": "cast", "card": 3, "plan": [{"source": 1, "ability": 0, "paying": [[2]], "chose": [1]}]}
        ability = self.call(move)["pay"]["abilities"][0]
        self.assertEqual(ability["pay"], {"objects": [["c2"]]})
        self.assertEqual(ability["additional"], {"choices": [2]})
        self.refuses({"kind": "cast", "card": 3, "plan": [{"source": 1, "ability": 0, "x": 2}]}, "retained nested X")

    def test_alternative_cost_and_routes(self):
        self.assertEqual(self.call({"kind": "cast", "card": 3, "cost": {"alternative": 1}})["cost"], {"kind": "alternative", "keyword": "evoke"})
        self.assertNotIn("cost", self.call({"kind": "cast", "card": 3, "route": "normal", "cost": "mana"}))
        # A route wrapper alone is not evidence that the accepted cast used it.
        self.refuses({"kind": "cast", "card": 3, "route": "flashback"}, "retained cast announcement")

    def test_exact_printed_nonkeyword_alternative_cost_keeps_its_ability_reference(self):
        self.a.audit["objects"]["3"]["casting_costs"]["alternatives"] = [{
            "paragraph": 2, "origin": {"kind": "own"}, "text": "You may sacrifice two Mountains rather than pay this spell’s mana cost."}]
        self.assertEqual(self.call({"kind": "cast", "card": 3, "cost": {"alternative": 1}})["cost"],
            {"kind": "alternative", "source": "c3", "ability": {"face": 0, "paragraph": 2}})

    def test_split_cast_selects_either_room_half_and_omits_default_face_zero(self):
        from types import SimpleNamespace
        self.a.cards = {"c3": {"oracle": "room"}}
        self.a.oracle = SimpleNamespace(faces={"room": [SimpleNamespace(raw={"mana_cost": "{2}{U}"}),
                                                       SimpleNamespace(raw={"mana_cost": "{3}{U}"})]})
        self.a._cast_as = lambda card, face: face
        self.assertEqual(self.call({"kind": "cast", "card": 3, "cast_as": 0}), {"kind": "cast", "card": "c3"})
        self.assertEqual(self.call({"kind": "cast", "card": 3, "cast_as": 1}), {"kind": "cast", "card": "c3", "face": 1})
        self.refuses({"kind": "cast", "card": 3, "cast_as": "fused"}, "fused casting")

    def test_printed_zero_cost_has_explicit_empty_payment(self):
        # CR 118.5: paying {0} is a payment, unlike an unpayable absent cost.
        from types import SimpleNamespace
        self.a.cards = {"c3": {"oracle": "memnite"}}
        self.a.oracle = SimpleNamespace(faces={"memnite": [SimpleNamespace(raw={"mana_cost": "{0}"})]})
        self.assertEqual(self.call({"kind": "cast", "card": 3}), {"kind": "cast", "card": "c3", "pay": {}})

    def test_free_cast_requires_matching_permission(self):
        self.a.state["asked"]["kind"] = "cast"
        move = {"kind": "cast", "card": 3}
        self.refuses(move, "offered_casts")
        self.a.audit["objects"]["3"]["offered_casts"] = [{"player": 0, "cast_as": "whole", "without_paying": True}]
        self.assertEqual(self.call(move), {"kind": "cast_during_resolution", "card": "c3", "cost": {"kind": "alternative", "effect": "without_paying"}})
        self.a.audit["objects"]["3"]["offered_casts"][0]["player"] = 1
        self.refuses(move, "permission")

    def test_paid_resolution_cast_permission_matches_exact_card_face_and_cost(self):
        self.a.state["asked"]["kind"] = "cast"
        self.a.state["asked"]["cards"] = [3]
        self.a.audit["objects"]["3"] = {"offered_casts": [], "casting_costs": {"alternatives": []},
                                         "definition_rows": ["Deal 3 damage to any target.", "Madness {R}"]}
        move = {"kind": "cast", "card": 3, "cost": {"alternative": 1}}
        self.assertEqual(self.call(move), {"kind": "cast_during_resolution", "card": "c3", "cost": {"kind": "alternative", "keyword": "madness"}})
        self.refuses({"kind": "cast", "card": 3}, "permission")
        self.refuses({**move, "cost": {"alternative": 0}}, "permission")
        self.refuses({**move, "cast_as": {"face": 1}}, "permission")
        self.a.state["asked"]["cards"] = [2]
        self.refuses(move, "permission")

    def test_typed_printed_or_cost_choices_follow_retained_text_order(self):
        first = {"ability": {}, "branch": {"kind": "printed_or", "arm": 2}}
        second = {"ability": {}, "branch": {"kind": "printed_or", "arm": 1}}
        following = after([{"event": "cast", "spell": 4}])
        following["audit"] = {"objects": {"4": {"announced": {"cost_branches": [second, first]}}}}
        move = {"kind": "cast", "card": 3, "cost_branches": [first, second]}
        self.assertEqual(self.call(move, following)["additional"], {"choices": [1, 2]})
        self.refuses(move, "retained", after())
        self.refuses({**move, "chose": [1, 0]}, "disagree", following)

    def test_mana_ability_payment_excludes_its_subsequent_production(self):
        self.a.audit["objects"]["1"]["abilities"][0]["text"] = "{G}: Add {G}{G}."
        self.a.state["players"][0]["pool"] = {"G": 1}
        following = after([{"event": "activated", "source": 1, "ability": None},
                           {"event": "mana_added", "source": 1, "mana": ["G", "G"]}], pool={"G": 2})
        result = self.call({"kind": "activate", "source": 1, "ability": 0}, following)
        self.assertEqual(result["adding"], ["G", "G"])
        self.assertEqual(result["pay"], {"pool": {"G": 1}})

    def test_combat_targets_and_damage_are_typed(self):
        self.assertEqual(self.call({"kind": "attack", "attacks": [[2, {"player": 1}], [3, {"object": 4}]]}),
                         {"kind": "declare_attackers", "attacks": [["c2", "B"], ["c3", "c4"]]})
        self.assertEqual(self.call({"kind": "block", "blocks": []}), {"kind": "declare_blockers", "blocks": []})
        self.a.state["asked"]["kind"] = "damage"
        self.assertEqual(self.call({"kind": "divide", "division": [[{"player": 1}, 3]]}, pending={"kind": "assign_combat_damage"}),
                         {"kind": "assign_combat_damage", "assignments": [["B", 3]]})

    def test_trigger_instances_keep_distinguishing_subjects(self):
        pending = {"kind": "order_triggers", "about": {"abilities": [{"source": "c2", "ability": {"face": 0, "paragraph": 1}, "for": ["c3"]}, {"source": "c2", "ability": {"face": 0, "paragraph": 1}, "for": ["c4"]}]}}
        self.assertEqual(self.call({"kind": "order", "order": [1, 0]}, pending=pending)["order"], list(reversed(pending["about"]["abilities"])))
        self.refuses({"kind": "order", "order": [0, 0]}, "permutation", pending=pending)

    def test_legacy_mixed_targets_normalize_only_target_positions(self):
        self.assertEqual(self.call({"kind": "attack", "attacks": [[2, "p1"], [3, 4]]}),
                         {"kind": "declare_attackers", "attacks": [["c2", "B"], ["c3", "c4"]]})
        self.assertEqual(self.call({"kind": "cast", "card": 3, "targets": [["p1", 2, {"player": "p0"}]]})["targets"],
                         [["B", "c2", "A"]])
        self.assertEqual(self.call({"kind": "choose", "answer": {"attach": 2}}, pending={"kind": "attach"}),
                         {"kind": "attach", "to": "c2"})

    def test_chosen_subtypes_use_canonical_spellings(self):
        for domain, native, expected in [("creature_type", "C'tan", "C’tan"), ("basic_land_type", "Forest", "Forest"), ("creature_type", "Time Lord", "Time Lord")]:
            self.assertEqual(self.call({"kind": "choose", "answer": {"value": {domain: native}}}, pending={"kind": "choose_value"}),
                             {"kind": "choose_value", "value": expected})

    def test_pay_yes_retains_listed_plan_and_never_searches_for_one(self):
        # CR 118.3, 601.2g-h: record the actual cost payment, including its
        # listed mana ability; the yes spelling itself supplies no new choice.
        self.a.state["asked"].update(kind="pay", cost=["G"], plan=[{"source": 1, "ability": 0}])
        move = {"kind": "choose", "answer": "yes"}
        result = self.call(move, after([{"event": "mana_added", "source": 1, "mana": ["G"]}]))
        self.assertEqual(result, {"kind": "pay", "pay": {"abilities": [
            {"source": "c1", "ability": {"rule": "305.6"}, "adding": ["G"]}]}})
        self.a.state["asked"]["plan"] = None
        self.refuses(move, "captured payment plan")
        self.a.state["asked"].update(plan=[], paying=[{"waterbend": 1}], tapped=[2])
        self.refuses(move, "cost-position mapping")

    def test_listing_life_total_is_not_a_phyrexian_announcement(self):
        # CR 118.9, 601.2b,h: the chosen alternative cost supplies fixed life;
        # CR 107.4f: pay.life instead records Phyrexian symbol choices.
        move = {"kind": "cast", "card": 3, "pay_life": 1}
        self.assertNotIn("pay", self.call(move))
        self.assertEqual(self.call({**move, "life": [True]})["pay"], {"life": [True]})
        for malformed in (-1, True, "1", None):
            self.refuses({**move, "pay_life": malformed}, "nonnegative integer")

    def test_unchanged_pending_copy_uses_authenticated_frozen_targets(self):
        # CR 707.10,c: copied target positions include empty groups; declining
        # new targets preserves every original target, even now-illegal ones.
        self.a.state["asked"].update(kind="retarget", copy=4, original=[[], [{"player": 0}], [{"object": 2}]])
        self.a.audit["objects"]["4"] = {"stack_copy_of": 3}
        expected = {"kind": "change_targets", "targets": [[], ["A"], ["c2"]]}
        frozen = deepcopy((self.a.state, self.a.audit))
        self.assertEqual(self.call({"kind": "choose", "answer": "no"}), expected)
        self.assertEqual((self.a.state, self.a.audit), frozen)
        self.a.state["objects"]["3"] = {"targets": [[{"player": 1}]]}
        self.refuses({"kind": "choose", "answer": "no"}, "disagree")

    def test_pending_copy_does_not_guess_targets_or_lineage(self):
        self.a.state["asked"].update(kind="retarget", copy=4, original=[[{"player": 0}]])
        move = {"kind": "choose", "answer": "no"}
        for copy in ({}, {"stack_copy_of": "3"}, {"stack_copy_of": 99}):
            self.a.audit["objects"]["4"] = copy
            self.refuses(move, "authenticated before evidence")
        self.a.audit["objects"]["4"] = {"stack_copy_of": 3}
        del self.a.state["asked"]["original"]
        self.refuses(move, "authenticated before evidence")

    def test_combined_modal_targeting_refuses_without_dropping_choices(self):
        move = {"kind": "targets", "modes": [0], "targets": [[{"object": 2}]], "division": [[1]]}
        original = deepcopy(move)
        self.refuses(move, "combined modal targeting needs explicit boundary decomposition", pending={"kind": "choose_modes"})
        self.assertEqual(move, original)
        self.refuses({"kind": "targets", "modes": [0], "targets": [[]]}, "boundary decomposition", pending={"kind": "choose_modes"})
        self.refuses({"kind": "targets", "modes": [0], "division": [[1]]}, "boundary decomposition", pending={"kind": "choose_modes"})

    def test_targets_and_modes(self):
        self.assertEqual(self.call({"kind": "targets", "modes": [0, 2]}, pending={"kind": "choose_modes"}), {"kind": "choose_modes", "modes": [1, 3]})
        self.assertEqual(self.call({"kind": "modes", "modes": [0, 2]}, pending={"kind": "choose_modes"}), {"kind": "choose_modes", "modes": [1, 3]})
        self.refuses({"kind": "modes", "modes": [0]}, "choose_modes boundary", pending={"kind": "choose_targets"})
        self.assertEqual(self.call({"kind": "targets", "targets": [[]]}, pending={"kind": "choose_targets"}), {"kind": "choose_targets", "targets": [[]]})

    def test_optional_objects_search_and_opening_hand(self):
        for question, answer, expected in [
            ("optional", "no", {"yes": False}),
            ("choose_objects", {"accept": False}, {"objects": []}),
            ("search", {"objects": [3]}, {"found": ["c3"]}),
            ("opening_hand_action", {"objects": []}, {"done": True}),
            ("opening_hand_action", {"objects": [3]}, {"card": "c3"})]:
            with self.subTest(question=question, answer=answer):
                self.assertEqual(self.call({"kind": "choose", "answer": answer}, pending={"kind": question}), {"kind": question, **expected})

    def test_typed_resolution_choices(self):
        for question, answer, expected in [
            ("choose_number", {"value": {"number": 4}}, {"number": 4}),
            ("choose_value", {"value": {"color": "red"}}, {"value": "red"}),
            ("choose_player", {"player": 1}, {"player": "B"}),
            ("choose_mana", {"mana": ["R"]}, {"mana": ["R"]}),
            ("choose_alternative", {"alternative": 0}, {"alternative": 1}),
            ("pile", {"pile": 1}, {"pile": 2}),
            ("holders", {"holders": [{"object": 2}, {"player": 1}]}, {"holders": ["c2", "B"]}),
            ("attach", {"attach": {"object": 2}}, {"to": "c2"}),
            ("library_destination", {"library_position": {"from_top": 2}}, {"at": {"from_top": 2}})]:
            with self.subTest(question=question):
                self.assertEqual(self.call({"kind": "choose", "answer": answer}, pending={"kind": question}), {"kind": question, **expected})

    def test_arrange_separate_and_pay_decline(self):
        self.assertEqual(self.call({"kind": "choose", "answer": {"top": [3, 2], "away": [4]}}, pending={"kind": "arrange"}),
                         {"kind": "arrange", "top": ["c3", "c2"], "away": ["c4"]})
        self.a.state["asked"]["among"] = [2, 3, 4]
        self.assertEqual(self.call({"kind": "choose", "answer": {"objects": [3]}}, pending={"kind": "separate"}),
                         {"kind": "separate", "piles": [["c3"], ["c2", "c4"]]})
        self.assertEqual(self.call({"kind": "choose", "answer": "no"}, pending={"kind": "pay"}), {"kind": "pay", "decline": True})

    def test_special_action_payment(self):
        self.a.state["players"][0]["pool"] = {"R": 2}
        self.assertEqual(self.call({"kind": "unlock", "room": 3, "half": 1, "mana": {"R": 2}}),
                         {"kind": "special_action", "action": "unlock", "object": "c3", "half": 1, "pay": {"pool": {"R": 2}}})

    def test_suspend_special_action_keeps_its_explicit_payment(self):
        self.a.state["players"][0]["pool"] = {"R": 1}
        self.assertEqual(self.call({"kind": "suspend", "card": 3, "mana": {"R": 1}}),
                         {"kind": "special_action", "action": "suspend", "object": "c3", "pay": {"pool": {"R": 1}}})

    def test_offered_effect_uses_captured_producer(self):
        self.a.state["asked"]["among"] = [{"id": 8, "source": 2, "ability": 2}]
        self.assertEqual(self.call({"kind": "choose", "answer": {"skip": 8}}, pending={"kind": "skip"}),
                         {"kind": "skip", "effect": {"source": "c2", "ability": {"face": 0, "paragraph": 3}}})

    def test_exclusions_refuse_instead_of_approximating(self):
        self.refuses({"kind": "cast", "card": 3, "delve": 2}, "chosen objects")
        self.refuses({"kind": "cast", "card": 3, "convoke": ["G"]}, "chosen objects")
        self.refuses({"kind": "cast", "card": 3, "face_down": "morph"}, "mapping")

    def test_missing_mana_activation_event_and_unknown_fields_refuse(self):
        self.refuses({"kind": "activate", "source": 1, "ability": 0}, "distinguishing")
        self.refuses({"kind": "pass", "invented": True}, "mapping")
        self.assertEqual(self.call({"kind": "activate", "source": 2, "ability": 2, "chose": [0]})["additional"], {"choices": [1]})

    def test_translation_does_not_mutate_frames(self):
        move = {"kind": "cast", "card": 3, "modes": [0], "plan": [{"source": 1, "ability": 0}]}
        following = after()
        originals = deepcopy((self.a.state, self.a.audit, move, following))
        self.call(move, following)
        self.assertEqual((self.a.state, self.a.audit, move, following), originals)


if __name__ == "__main__":
    unittest.main()
