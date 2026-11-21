"""CR 401.4/404.3 ordering is input, even if later effects consume the order.

These are synthetic validator boundary frames, not executable starting premises.
"""
from copy import deepcopy
from pathlib import Path
import unittest

from mgn.adapters.karn import KarnAdapter, Refusal
from mgn.delta import null_answer
from mgn.oracle import Oracle
from mgn.state import State, zone_names
from mgn.examples import EXAMPLES, load
from mgn.validate import validate

O = Oracle()


class OrderCardsTests(unittest.TestCase):
    def record(self, library=False):
        r = load(EXAMPLES / "priority-and-steps.json")
        r.pop("checkpoints", None)
        r["tier"] = 1
        r["cards"] = {name: {"owner": "A", "oracle": O.by_name(oracle), "name": oracle}
                      for name, oracle in [("first", "Island"), ("second", "Forest")]}
        zones = {z: [] for z in zone_names(["A", "B"])}
        zones["A.hand"] = ["first", "second"]
        about = {"cards": ["first", "second"], "zone": "A.library" if library else "A.graveyard"}
        if library:
            about["placement"] = "top"
        r["start"] = {"kind": "position", "state": {"turn": {"number": 3, "step": "precombat_main", "active": "A"},
            "pending": {"seat": "A", "kind": "order_cards", "about": about}, "result": {"kind": "unfinished"},
            "players": {"A": {"life": 20}, "B": {"life": 20}}, "zones": zones, "objects": {}}}
        zone = about["zone"]
        r["transitions"] = [{"seat": "A", "do": {"kind": "order_cards", "order": ["second", "first"]},
            "delta": {"objects": {name: {"zone": zone} for name in ["first", "second"]},
                      "zones": {zone: {"top": ["second", "first"]}}, "pending": {"seat": "A", "kind": "priority"}}}]
        return r

    def test_graveyard_order_top_first_and_library_placement(self):
        for library in (False, True):
            r = self.record(library)
            self.assertEqual(validate(r, O), [])
            errors = []; s = State.from_record(r, errors)
            s.apply(r["transitions"][0]["delta"], [], "/order", errors)
            self.assertEqual(s.zones[r["start"]["state"]["pending"]["about"]["zone"]], ["second", "first"])
            if not library:
                self.assertEqual(s.listing("A.graveyard"), ["first", "second"])
            self.assertFalse(null_answer(r["transitions"][0]["do"], State.from_record(r, [])))

    def test_permutation_owner_zone_and_placement_boundaries(self):
        for mutate in (lambda r: r["transitions"][0]["do"].update(order=["first"]),
                       lambda r: r["transitions"][0]["do"].update(order=["first", "first"]),
                       lambda r: r["start"]["state"]["pending"]["about"].update(zone="B.graveyard"),
                       lambda r: r["cards"]["second"].update(owner="B"),
                       lambda r: r["start"]["state"]["pending"]["about"].update(placement="top")):
            r = self.record(); mutate(r)
            self.assertTrue(validate(r, O))
        r = self.record(True)
        del r["start"]["state"]["pending"]["about"]["placement"]
        self.assertTrue(validate(r, O))

    def test_only_unique_zero_or_one_card_order_is_implied(self):
        for cards in ([], ["first"]):
            r = self.record(); r["start"]["state"]["pending"]["about"]["cards"] = cards
            self.assertTrue(null_answer({"kind": "order_cards", "order": cards}, State.from_record(r, [])))
        # Two physically distinct Islands still need the owner's actual choice.
        r = self.record(); r["cards"]["second"].update(oracle=O.by_name("Island"), name="Island")
        self.assertFalse(null_answer({"kind": "order_cards", "order": ["first", "second"]}, State.from_record(r, [])))

    def test_order_can_be_consumed_before_next_boundary_without_losing_input(self):
        r = self.record(True)
        # A following draw consumes the chosen top card before the next recorded
        # decision. Its identity remains explicit in order_cards.order.
        r["transitions"][0]["delta"] = {"objects": {"first": {"zone": "A.library", "at": "top"}, "second": {"zone": "A.hand"}},
            "pending": {"seat": "A", "kind": "priority"}}
        self.assertEqual(validate(r, O), [])
        self.assertEqual(r["transitions"][0]["do"]["order"], ["second", "first"])
        a = KarnAdapter(Path("/unused"), O)
        a.state = {"asked": {"kind": "priority"}}
        with self.assertRaises(Refusal) as cm:
            a._move(r["transitions"][0]["do"])
        self.assertEqual(cm.exception.class_, "illegal")


class OwnerOrderTests(unittest.TestCase):
    def adapter(self, destination):
        adapter = KarnAdapter.__new__(KarnAdapter)
        adapter.seats = ["A", "B"]
        adapter.cards = {"bear": {"owner": "B"}, "wolf": {"owner": "B"}}
        adapter.state = {"asked": {"kind": "order_cards", "player": 1, "to": destination, "cards": [70, 12]}}
        adapter._record_id = lambda native: {70: "bear", 12: "wolf"}[native]
        adapter._engine_id = lambda record: {"bear": 70, "wolf": 12}[record]
        return adapter

    def test_graveyard_order_is_top_first_and_authored_identifiers_keep_their_order(self):
        adapter = self.adapter({"zone": "graveyard"})
        self.assertEqual(adapter._pending([], {}), {"seat": "B", "kind": "order_cards",
            "about": {"cards": ["bear", "wolf"], "zone": "B.graveyard"}})
        self.assertEqual(adapter._move({"kind": "order_cards", "order": ["wolf", "bear"]}),
            {"kind": "choose", "answer": {"objects": [12, 70]}})

    def test_library_position_is_projected_from_the_native_prompt(self):
        for placement in [{"position": "top"}, {"position": "bottom"}, {"from_top": 3}]:
            adapter = self.adapter({"zone": "library", **placement})
            expected = placement.get("position", {"from_top": placement.get("from_top")})
            self.assertEqual(adapter._pending([], {})["about"], {"cards": ["bear", "wolf"],
                "zone": "B.library", "placement": expected})

    def test_generic_object_selection_is_not_authentication_for_owner_order(self):
        adapter = self.adapter({"zone": "graveyard"})
        adapter.state["asked"] = {"kind": "objects", "player": 1, "verb": "Move",
                                   "among": [70, 12], "count": 2, "to": {"zone": "graveyard"}}
        with self.assertRaisesRegex(Refusal, "no owner arrival order"):
            adapter._move({"kind": "order_cards", "order": ["wolf", "bear"]})

    def test_explicit_nonowner_order_is_refused_instead_of_naming_the_wrong_library(self):
        adapter = self.adapter({"zone": "library", "position": "top"})
        adapter.cards = {"bear": {"owner": "A"}, "wolf": {"owner": "A"}}
        with self.assertRaisesRegex(Refusal, "specified non-owner") as caught:
            adapter._pending([], {})
        self.assertEqual(caught.exception.class_, "unsupported")
