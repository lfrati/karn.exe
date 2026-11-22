"""Indentured Djinn's "When this creature
enters, each other player may draw up to three cards." through the real audited client with full
pinned cards. Each affected player chooses a number from 0 to 3 in APNAP order, knowing the
earlier choices (101.4, 101.4b, 608.2e); then the draws happen, the active player first and then
the others in turn order (121.2c). An empty library never limits the choice: a player may choose
to draw more cards than they have, and loses at the next state-based check (121.3, 608.2d,
704.5b). A replacement of one of those draws (dredge, 702.52a) applies to that draw alone, and
the numbers are not asked again. The trigger resolves after the Djinn leaves (603.10, 113.7a)."""
import unittest
import test_toxic_review_client as base
from client_support import ClientTestCase

HELPERS = {k: v for k, v in vars(base.ToxicReviewClientTests).items() if not k.startswith("test_") and not k.startswith("__")}

DJINN = "Indentured Djinn"


class DrawUpToReviewClientTests(ClientTestCase):
    locals().update(HELPERS)

    # --- reading the state

    def state(self):
        return self.engine.state

    def library(self, player):
        return self.state()["players"][player]["library"]

    def hand_size(self, player):
        return self.state()["players"][player]["hand_size"]

    def numbers(self, viewer):
        """The numbers chosen so far for this resolution, as seen by this viewer."""
        return [(c["player"], c["value"]["number"]) for c in self.view(viewer)["choices_made"]
                if "number" in c.get("value", {})]

    def question(self, player, maximum=3):
        a = self.asked()
        self.assertEqual((a["kind"], a["player"], a.get("min"), a.get("max")), ("number", player, 0, maximum), a)
        return a

    def draws(self, events):
        return [e["player"] for e in events if e.get("event") == "drew"]

    # --- acting

    def until_asked(self):
        for _ in range(60):
            a = self.asked()
            if a["kind"] != "priority" or not self.state()["stack"]:
                return a
            self.engine.act({"kind": "pass"})
        self.fail("the stack did not settle")

    def cast(self, name, targets=(), *, mana=(), spend):
        actor = self.asked()["player"]
        card = next(o for o in self.state()["players"][actor]["hand"] if self.obj(o)["name"] == name)
        self.engine.act({"kind": "cast", "card": card, "targets": list(targets),
                         "plan": [{"source": o, "ability": 0} for o in mana], "mana": spend})

    def number(self, n):
        return {"kind": "choose", "answer": {"value": {"number": n}}}

    def collect(self, answers):
        """Answer each number question in turn; returns every event the engine reported."""
        events = []
        for n in answers:
            a = self.asked()
            self.assertEqual(a["kind"], "number", a)
            events.extend(self.engine.act(self.number(n))["events"])
        return events

    def cast_djinn(self, seats, respond=None):
        """P0 casts the Djinn and it resolves; the enters trigger is on the stack (P0 has
        priority). `respond` runs before it resolves."""
        self.new(seats)
        self.main()
        self.cast(DJINN, mana=self.ids("Island")[:3], spend={'U': 3})
        for _ in range(10):
            on_battlefield = any(self.obj(o)["name"] == DJINN for o in self.state()["battlefield"])
            if on_battlefield and self.state()["stack"] and self.asked()["kind"] == "priority":
                break
            self.engine.act({"kind": "pass"})
        self.assertEqual(self.asked()["player"], 0, "the trigger waits on the stack")
        if respond:
            respond()
        return self.until_asked()

    def djinn_seat(self, **extra):
        return {"hand": [DJINN], "battlefield": ["Island"] * 3, **extra}

    # --- two players

    def test_only_the_other_player_chooses_and_draws_after_choosing(self):
        self.cast_djinn([self.djinn_seat(), {"library": ["Mountain"] * 8}])
        self.question(1)
        self.assertEqual((self.library(1), self.hand_size(1), self.numbers(0)), (8, 0, []))
        events = self.collect([2])
        self.assertEqual(self.draws(events), [1, 1])
        self.assertEqual((self.library(0), self.library(1), self.hand_size(1)), (8, 6, 2), "the controller draws nothing")
        self.assertEqual(self.asked()["kind"], "priority")

    def test_zero_draws_nothing_and_three_draws_three(self):
        for n in (0, 3):
            with self.subTest(n=n):
                self.cast_djinn([self.djinn_seat(), {"library": ["Mountain"] * 8}])
                self.question(1)
                self.assertEqual(self.draws(self.collect([n])), [1] * n)
                self.assertEqual(self.hand_size(1), n)

    def test_choosing_more_than_the_library_holds_draws_what_there_is_and_loses(self):
        for library, n, lost in ((1, 3, True), (0, 1, True), (0, 0, False), (3, 3, False)):
            with self.subTest(library=library, n=n):
                self.cast_djinn([self.djinn_seat(), {"library": ["Mountain"] * library}])
                self.question(1)  # the bound is 3 whatever the library holds (121.3)
                events = self.collect([n])
                self.assertEqual(self.draws(events), [1] * min(library, n), "the cards there are are drawn")
                self.assertEqual([e["player"] for e in events if e.get("event") == "lost"], [1] if lost else [])
                self.assertEqual((self.library(1), self.state()["players"][1]["lost"]), (max(library - n, 0), lost))
                self.assertEqual(self.state()["winner"], 0 if lost else None)

    def test_the_trigger_resolves_after_the_djinn_is_unsummoned(self):
        def bounce():
            self.cast("Unsummon", [[{"object": self.one(DJINN)}]], mana=self.ids("Island")[3:4], spend={'U': 1})
        self.cast_djinn([self.djinn_seat(hand=[DJINN, "Unsummon"], battlefield=["Island"] * 4),
                         {"library": ["Mountain"] * 8}], respond=bounce)
        self.question(1)
        self.assertNotIn(DJINN, [self.obj(o)["name"] for o in self.state()["battlefield"]])
        self.collect([2])
        self.assertEqual(self.hand_size(1), 2)

    # --- answers

    def test_invalid_answers_are_refused_and_change_nothing(self):
        self.cast_djinn([self.djinn_seat(), {"library": ["Mountain"] * 8}])
        self.question(1)
        bad = [{"value": {"number": -1}}, {"value": {"number": 1.5}}, {"value": {"number": "2"}},
               {"value": {"number": 4}}, {"value": {"number": 4294967296}}, {"value": {"number": None}},
               {"value": {"number": 1, "color": "blue"}}, {"value": {"color": "blue"}},
               {"yes": True}, "no", "yes", {"objects": []}, {"number": 2}]
        for answer in bad:
            with self.subTest(answer=answer):
                self.reject(1, {"kind": "choose", "answer": answer})
                self.question(1)
        self.reject(1, {"kind": "pass"})
        self.reject(0, self.number(1))  # the controller does not choose this number
        self.question(1)
        self.assertEqual((self.library(1), self.numbers(0)), (8, []))
        self.collect([1])
        self.assertEqual(self.hand_size(1), 1)

    # --- several players

    def flicker_seats(self, players):
        """P2 controls the Djinn and P1 is active: P2 flickers it with Ephemerate on P1's turn.
        Every library has its own name, so each player's draws are visible."""
        seats = [{"library": ["Forest"] * 8}, {"library": ["Mountain"] * 8},
                 {"battlefield": [DJINN, "Plains"], "hand": ["Ephemerate"], "library": ["Swamp"] * 8},
                 {"library": ["Island"] * 8}]
        return seats[:players]

    def flicker(self, players):
        self.new(self.flicker_seats(players))
        self.advance(lambda s: s["active"] == 1 and s["part"] == "PrecombatMain" and s["asked"]["kind"] == "priority"
                     and s["asked"]["player"] == 1 and not s["stack"])
        self.engine.act({"kind": "pass"})
        self.assertEqual(self.asked()["player"], 2, "P2 gets priority in P1's main phase")
        self.cast("Ephemerate", [[{"object": self.one(DJINN)}]], mana=[self.one("Plains")], spend={'W': 1})
        return self.until_asked()

    def test_four_players_choose_in_apnap_order_from_the_active_player(self):
        self.flicker(4)
        start = [self.library(p) for p in range(4)]
        events, order = [], []
        for n in (2, 0, 3):
            a = self.asked()
            self.assertEqual((a["kind"], a["min"], a["max"]), ("number", 0, 3), a)
            order.append(a["player"])
            self.assertEqual([self.library(p) for p in range(4)], start, "nobody draws before every number is chosen")
            events.extend(self.engine.act(self.number(n))["events"])
        self.assertEqual(order, [1, 3, 0], "APNAP from the active P1; the controller P2 is not asked")
        self.assertEqual(self.draws(events), [1, 1, 0, 0, 0], "the active player draws first (121.2c)")
        self.assertEqual([start[p] - self.library(p) for p in range(4)], [3, 2, 0, 0])

    def test_each_number_is_public_to_every_viewer_before_the_next_chooser_is_asked(self):
        self.flicker(4)
        self.question(1)
        self.assertEqual([self.numbers(v) for v in range(4)], [[]] * 4)
        self.engine.act(self.number(2))
        self.question(3)
        self.assertEqual([self.numbers(v) for v in range(4)], [[(1, 2)]] * 4, "101.4b")
        self.engine.act(self.number(0))
        self.question(0)
        self.assertEqual([self.numbers(v) for v in range(4)], [[(1, 2), (3, 0)]] * 4, "a zero is a choice too")
        self.engine.act(self.number(1))
        self.assertEqual(self.asked()["kind"], "priority")

    # --- replacement of one draw

    def dredge_question(self, among):
        a = self.asked()
        self.assertEqual((a["kind"], a.get("event"), a["player"], a["optional"], a["on"]), ("replacement", "draw", 1, True, {"player": 1}), a)
        self.assertEqual(sorted(e["name"] for e in a["among"]), sorted(among))
        return a

    def dredge(self, a, name):
        effect = next(e for e in a["among"] if e["name"] == name)
        self.engine.act({"kind": "choose", "answer": {"replacement": {k: v for k, v in effect.items() if k != "name"}}})

    def test_dredge_replaces_single_draws_and_the_number_is_not_asked_again(self):
        """Each of the three draws is its own event (121.2): Stinkweed Imp dredges the first (5),
        the second is declined and drawn, Golgari Thug dredges the third (4)."""
        self.cast_djinn([self.djinn_seat(), {"library": ["Mountain"] * 12, "graveyard": ["Stinkweed Imp", "Golgari Thug"]}])
        self.question(1)
        self.engine.act(self.number(3))
        self.dredge(self.dredge_question(["Stinkweed Imp", "Golgari Thug"]), "Stinkweed Imp")
        order=self.asked()
        self.assertEqual((order["kind"],len(order["cards"])),("order_cards",5))
        a,b,c,d,e=order["cards"]
        self.engine.act({"kind":"choose","answer":{"objects":[e,d,c,b,a]}})
        self.assertEqual(self.library(1), 7, "milled five")
        self.dredge_question(["Golgari Thug"])
        self.engine.act({"kind": "choose", "answer": "no"})
        self.dredge(self.dredge_question(["Golgari Thug"]), "Golgari Thug")
        order=self.asked()
        self.assertEqual((order["kind"],len(order["cards"])),("order_cards",4))
        a,b,c,d=order["cards"]
        self.engine.act({"kind":"choose","answer":{"objects":[d,c,b,a]}})
        self.assertEqual(self.asked()["kind"], "priority", "no number question again")
        mine = self.view(1)
        hand = sorted(mine["objects"][str(o)]["name"] for o in mine["players"][1]["hand"])
        self.assertEqual(hand, ["Golgari Thug", "Mountain", "Stinkweed Imp"])
        self.assertEqual((self.library(1), len(self.state()["players"][1]["graveyard"])), (2, 9))

    # --- contrasts

    def test_consecrated_sphinx_still_asks_yes_or_no_once_per_card_drawn(self):
        self.cast_djinn([self.djinn_seat(battlefield=["Island"] * 3 + ["Consecrated Sphinx"]), {"library": ["Mountain"] * 8}])
        self.question(1)
        self.engine.act(self.number(2))
        # Two completed draws create two Sphinx occurrences. CR 603.3b leaves
        # their controller an explicit ordering choice before either may answer.
        order = self.until_asked()
        self.assertEqual((order['kind'], order['player']), ('order', 0), order)
        self.assertEqual(len(order['abilities']), 2)
        sphinx = self.one('Consecrated Sphinx')
        self.assertTrue(all(ability['source'] == sphinx for ability in order['abilities']))
        self.engine.act({'kind': 'order', 'order': [0, 1]})
        mays = 0
        for _ in range(20):
            a = self.until_asked()
            if a["kind"] == "priority":
                break
            self.assertEqual((a["kind"], a["player"]), ("may", 0), a)
            mays += 1
            self.engine.act({"kind": "choose", "answer": "yes" if mays == 1 else "no"})
        self.assertEqual(mays, 2, "one trigger for each card P1 drew")
        self.assertEqual((self.hand_size(0), self.hand_size(1)), (2, 2))

    def test_vision_skeins_draws_without_a_question(self):
        self.new([{"hand": ["Vision Skeins"], "battlefield": ["Island"] * 2, "library": ["Island"] * 8},
                  {"library": ["Mountain"] * 8}])
        self.main()
        self.cast("Vision Skeins", mana=self.ids("Island"), spend={'U': 2})
        self.assertEqual(self.until_asked()["kind"], "priority")
        self.assertEqual((self.hand_size(0), self.hand_size(1)), (2, 2))


if __name__ == "__main__":
    unittest.main()
