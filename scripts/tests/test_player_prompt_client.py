"""The player-facing pipe (API player/1) through the real client: each seat is prompted with
the game as it may see it, labelled, with readable text and the question's context, and
answers by labels. Expectations come from the cards' Oracle text and the rules cited."""
import subprocess
import os
import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import play


class PlayerPromptClientTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        if os.environ.get("GAME_SERVE_REVIEW"):
            return
        subprocess.run(["cargo", "build", "--profile", "fast", "-p", "game", "--bin", "game-serve"],
                       cwd=play.ROOT, check=True, stdout=subprocess.DEVNULL)

    def setUp(self):
        self.engine = play.Engine(executable=os.environ.get("GAME_SERVE_REVIEW"))
        self.addCleanup(self.close_engine)

    def close_engine(self):
        self.engine.proc.stdin.close()
        self.engine.proc.wait(timeout=10)
        self.engine.proc.stdout.close()

    def new(self, *players):
        self.engine.send({"cmd": "new", "players": [{"library": ["Island"] * 5, **p} for p in players]})
        self.seats = [play.Seat(self.engine, i) for i in range(len(players))]
        # to the first main phase, the seat waited for answering each time
        while not (self.waiting().last["part"] == "PrecombatMain" and self.seats[0].last["waiting_for"] == "p0"):
            self.waiting().answer({"kind": "pass"})
        return self.seats

    def waiting(self):
        """The seat the game waits for, prompted once."""
        label = self.seats[0].prompt()["waiting_for"]
        seat = self.seats[int(label[1:])]
        if seat is not self.seats[0]:
            seat.prompt()
        return seat

    def pass_until_asked(self):
        """Pass priority until a question other than priority is asked; that seat, prompted."""
        while True:
            seat = self.waiting()
            if seat.last["decision"]["kind"] != "priority" or not seat.last["stack"]:
                return seat
            seat.answer({"kind": "pass"})

    @staticmethod
    def named(prompt, name, controller=None, zone=None):
        return [label for label, o in prompt["objects"].items() if o["name"] == name
                and (controller is None or o["controller"] == controller) and (zone is None or o["zone"] == zone)]

    def refused(self, seat, request):
        fields = {"seat": seat.label, "session_id": seat.last["session_id"]}
        if request["cmd"] == "answer":
            fields["request_id"] = seat.last["request_id"]
        with self.assertRaises(SystemExit) as refusal:
            self.engine.send(fields | request)
        return str(refusal.exception)

    def test_a_target_is_chosen_by_label_among_duplicates_and_paid_by_an_ability_label(self):
        me, them = self.new({"hand": ["Lightning Bolt"], "battlefield": ["Mountain"]},
                            {"battlefield": ["Grizzly Bears", "Grizzly Bears"], "hand": ["Shock"]})
        prompt = me.prompt()
        self.assertEqual((prompt["api"], prompt["seat"], prompt["decision"]["chooser"]), ("player/1", "p0", "p0"))
        bears = self.named(prompt, "Grizzly Bears")
        self.assertEqual(len(bears), 2, "two cards with one name are two labelled objects")
        self.assertNotIn("hand", prompt["players"][1], "402.3: the opponent's hand is only counted")
        self.assertEqual(prompt["players"][1]["hand_size"], 1)
        [bolt] = self.named(prompt, "Lightning Bolt")
        self.assertEqual(prompt["objects"][bolt]["text"], ["Lightning Bolt deals 3 damage to any target."])
        self.assertEqual(prompt["objects"][bolt]["mana_cost"], "{R}")
        [mountain] = self.named(prompt, "Mountain")
        self.assertEqual(prompt["objects"][mountain]["abilities"], [{"label": f"{mountain}.a0", "text": "{T}: Add {R}."}])
        self.assertNotIn("moves", prompt["decision"], "a prompt lists no moves")
        after = me.answer({"kind": "cast", "card": bolt, "mana": {"R": 1}, "targets": [[bears[1]]], "plan": [{"ability": f"{mountain}.a0"}]})
        [spell] = after["stack"]
        self.assertEqual(after["objects"][spell]["occurrence"], {"kind": "spell", "object": spell, "controller": "p0", "source_name": "Lightning Bolt"})
        self.assertEqual(after["objects"][spell]["text"], ["Lightning Bolt deals 3 damage to any target."])
        self.assertIn({"event": "became_target", "object": bears[1], "of": spell}, after["events"])
        # 117.3c: the caster keeps priority; the other seat learns it is waited for
        theirs = them.prompt()
        self.assertEqual((after["decision"]["kind"], theirs["decision"], theirs["waiting_for"]), ("priority", None, "p0"))
        # each seat names objects by its own labels, and is told what happened once
        self.assertIn({"event": "cast", "player": "p0", "spell": theirs["stack"][0]}, theirs["events"])
        self.assertEqual(them.prompt()["events"], [])
        me.answer({"kind": "pass"})
        self.assertEqual(them.prompt()["decision"]["kind"], "priority")
        them.answer({"kind": "pass"})
        done = me.prompt()
        self.assertEqual(self.named(done, "Grizzly Bears", zone="battlefield"), [bears[0]])
        moved = [e for e in done["events"] if e["event"] == "zone" and e["was"] == bears[1]]
        self.assertEqual(len(moved), 1)
        self.assertNotIn(moved[0]["now"], bears, "a card that changes zones is a new object with a new label (400.7)")
        self.assertEqual(done["objects"][moved[0]["now"]]["zone"], "graveyard")

    def test_a_payment_is_asked_of_the_spells_controller_for_the_counterspell_then_a_private_look(self):
        me, them = self.new({"hand": ["Ponder"], "battlefield": ["Island"] + ["Mountain"] * 3,
                             "library": ["Forest", "Shock", "Grizzly Bears", "Lightning Bolt"]},
                            {"hand": ["Mana Leak"], "battlefield": ["Island", "Island"]})
        prompt = me.prompt()
        [ponder] = self.named(prompt, "Ponder")
        [island] = self.named(prompt, "Island", controller="p0")
        spell_of_mine = me.answer({"kind": "cast", "card": ponder, "mana": {"U": 1}, "plan": [{"ability": f"{island}.a0"}]})["stack"][0]
        me.answer({"kind": "pass"})
        prompt = them.prompt()
        [spell] = prompt["stack"]
        [leak] = self.named(prompt, "Mana Leak")
        self.assertEqual(prompt["objects"][leak]["text"], ["Counter target spell unless its controller pays {3}."])
        islands = self.named(prompt, "Island", controller="p1")
        them.answer({"kind": "cast", "card": leak, "mana": {"U": 2}, "targets": [[spell]], "plan": [{"ability": f"{i}.a0"} for i in islands]})
        self.assertIs(self.pass_until_asked(), me)
        asked = me.last["decision"]
        # 608.2: Mana Leak's controller does what it says; the choice to pay is the spell's controller's
        self.assertEqual((asked["kind"], asked["chooser"], asked["reason"]), ("pay", "p0", "resolution"))
        self.assertEqual((asked["for"]["kind"], asked["for"]["source_name"], asked["for"]["controller"]), ("spell", "Mana Leak", "p1"))
        self.assertEqual(asked["cost"], ["3"])
        self.assertIsNone(them.prompt()["decision"], "the other seat is told only whom the game waits for")
        self.assertEqual(them.last["waiting_for"], "p0")
        me.prompt()
        me.answer({"kind": "choose", "answer": {"pay": {"mana": {"R": 3}, "plan": [{"ability": f"{m}.a0"} for m in self.named(me.last, "Mountain")]}}})
        self.assertIs(self.pass_until_asked(), me)
        asked = me.last["decision"]
        # Ponder: "Look at the top three cards of your library, then put them back in any order."
        self.assertEqual((asked["kind"], len(asked["cards"]), asked["reason"]), ("order_cards", 3, "resolution"))
        self.assertEqual((asked["for"]["source_name"], asked["for"]["object"]), ("Ponder", spell_of_mine))
        looked = asked["cards"]
        names = {c: me.last["objects"][c]["name"] for c in looked}
        self.assertEqual(sorted(names.values()), ["Grizzly Bears", "Lightning Bolt", "Shock"])
        self.assertEqual({me.last["objects"][c]["zone"] for c in looked}, {"library"})
        self.assertEqual([(o["kind"], len(o["cards"])) for o in me.last["observations"]], [("looked", 3)])
        theirs = them.prompt()
        self.assertFalse([o for o in theirs["objects"].values() if o["zone"] == "library"], "a private look (400.2)")
        self.assertEqual(theirs["observations"], [])
        # the other seat was given no label for them, and may not answer this question at all
        self.assertIn("names nothing you can refer to now", self.refused(them, {"cmd": "answer", "answer": {"kind": "choose", "answer": {"objects": looked}}}))
        self.assertIn("request belongs to player 0", self.refused(them, {"cmd": "answer", "answer": {"kind": "pass"}}))
        order = [next(c for c in looked if names[c]==name) for name in ["Shock","Lightning Bolt","Grizzly Bears"]]
        asked = me.answer({"kind": "choose", "answer": {"objects": order}})["decision"]
        self.assertEqual((asked["kind"], asked["for"]["source_name"]), ("may", "Ponder"))
        done = me.answer({"kind": "choose", "answer": "no"})
        [drawn] = done["players"][0]["hand"]
        self.assertEqual(done["objects"][drawn]["name"], names[order[0]], "the card put on top is drawn")
        self.assertNotIn(drawn, looked, "the drawn card is a new object with a new label (400.7)")
        # the cards looked at are hidden again: their labels name nothing the seat may refer to
        refusal = self.refused(me, {"cmd": "answer", "answer": {"kind": "cast", "card": order[1]}})
        self.assertIn(f"{order[1]} names nothing you can refer to now", refusal)
        self.assertIn("o999 names nothing you can refer to now", self.refused(me, {"cmd": "answer", "answer": {"kind": "cast", "card": "o999"}}))

    def test_simultaneous_triggers_are_ordered_by_their_sources_and_texts(self):
        me, them = self.new({"hand": ["Grizzly Bears"], "battlefield": ["Soul Warden", "Essence Warden", "Forest", "Forest"]}, {})
        prompt = me.prompt()
        [bears] = self.named(prompt, "Grizzly Bears")
        forests = self.named(prompt, "Forest")
        me.answer({"kind": "cast", "card": bears, "mana": {"G": 2}, "plan": [{"ability": f"{f}.a0"} for f in forests]})
        self.assertIs(self.pass_until_asked(), me)
        asked = me.last["decision"]
        self.assertEqual((asked["kind"], asked["reason"], asked["chooser"]), ("order", "trigger_order", "p0"))
        wardens = {self.named(me.last, n)[0]: n for n in ("Soul Warden", "Essence Warden")}
        told = sorted((o["source_name"], o["text"], o["kind"], wardens[o["source"]]) for o in asked["ordered"])
        self.assertEqual(told, [
            ("Essence Warden", "Whenever another creature enters, you gain 1 life.", "triggered", "Essence Warden"),
            ("Soul Warden", "Whenever another creature enters, you gain 1 life.", "triggered", "Soul Warden"),
        ])
        self.assertIsNone(them.prompt()["decision"])
        after = me.answer({"kind": "order", "order": [1, 0]})
        self.assertEqual(len(after["stack"]), 2)
        self.assertEqual({after["objects"][o]["occurrence"]["source_name"] for o in after["stack"]}, {"Soul Warden", "Essence Warden"})

    def test_a_dies_trigger_whose_source_has_left_keeps_its_name_and_text(self):
        me, them = self.new({"hand": ["Lightning Bolt"], "battlefield": ["Mountain"]},
                            {"battlefield": ["Perilous Myr"]})
        # each seat has its own labels: the Myr as its controller was shown it
        [myr] = self.named(them.prompt(), "Perilous Myr")
        prompt = me.prompt()
        me.answer({"kind": "cast", "card": self.named(prompt, "Lightning Bolt")[0], "mana": {"R": 1}, "targets": [self.named(prompt, "Perilous Myr")],
                   "plan": [{"ability": f"{self.named(prompt, 'Mountain')[0]}.a0"}]})
        seat = self.pass_until_asked()
        self.assertIs(seat, them, "the Myr's controller puts its trigger on the stack and chooses its target")
        asked = them.last["decision"]
        self.assertEqual((asked["kind"], asked["reason"]), ("targets", "trigger"))
        theirs = them.last
        [old] = self.named(theirs, "Perilous Myr", zone="graveyard")
        trigger = asked["for"]
        self.assertEqual((trigger["kind"], trigger["source_name"], trigger["source_left"], trigger["controller"]),
                         ("triggered", "Perilous Myr", True, "p1"))
        self.assertEqual(trigger["text"], "When Perilous Myr dies, it deals 2 damage to any target.")
        # the source is named as the seat saw it, the Myr on the battlefield; the card in the
        # graveyard is another object (400.7)
        self.assertEqual(trigger["source"], myr)
        self.assertNotEqual(old, myr)
        self.assertNotIn(myr, theirs["objects"])
        self.assertEqual(theirs["objects"][old]["zone"], "graveyard")
        after = them.answer({"kind": "targets", "targets": [["p0"]]})
        [on_stack] = after["stack"]
        self.assertEqual(after["objects"][on_stack]["occurrence"]["text"], trigger["text"])
        self.assertTrue(after["objects"][on_stack]["occurrence"]["source_left"])
        seat = self.pass_until_asked()
        self.assertEqual(seat.last["players"][0]["life"], 18)

    def test_a_granted_ability_has_its_own_label_on_the_permanent_and_says_what_grants_it(self):
        me, _ = self.new({"battlefield": ["Cryptolith Rite", "Llanowar Elves"]}, {})
        prompt = me.prompt()
        [rite] = self.named(prompt, "Cryptolith Rite")
        [elves] = self.named(prompt, "Llanowar Elves")
        self.assertEqual(prompt["objects"][rite]["text"], ['Creatures you control have "{T}: Add one mana of any color."'])
        abilities = prompt["objects"][elves]["abilities"]
        self.assertEqual(abilities, [{"label": f"{elves}.a0", "text": "{T}: Add {G}."},
                                     {"label": f"{elves}.a1", "text": "{T}: Add one mana of any color.", "granted_by": rite}])
        after = me.answer({"kind": "activate", "ability": f"{elves}.a1", "adding": [["U"]]})
        self.assertEqual(after["players"][0]["pool"]["U"], 1)
        self.assertTrue(after["objects"][elves]["tapped"])
        # a mana ability does not use the stack (605.3a): the event names no ability on it
        self.assertIn({"ability": None, "event": "activated", "keyword": None, "player": "p0", "source": elves}, after["events"])

    def test_a_tutor_offers_library_cards_only_its_chooser_is_given_labels_for(self):
        me, them = self.new({"hand": ["Demonic Tutor"], "battlefield": ["Swamp", "Swamp"],
                             "library": ["Lightning Bolt", "Grizzly Bears", "Shock"]}, {})
        prompt = me.prompt()
        me.answer({"kind": "cast", "card": self.named(prompt, "Demonic Tutor")[0], "mana": {"B": 2},
                   "plan": [{"ability": f"{s}.a0"} for s in self.named(prompt, "Swamp")]})
        self.assertIs(self.pass_until_asked(), me)
        asked = me.last["decision"]
        self.assertEqual((asked["kind"], asked["reason"], asked["for"]["source_name"]), ("search", "resolution", "Demonic Tutor"))
        [bolt] = [c for c in asked["cards"] if me.last["objects"][c]["name"] == "Lightning Bolt"]
        self.assertEqual(me.last["objects"][bolt]["text"], ["Lightning Bolt deals 3 damage to any target."])
        theirs = them.prompt()
        self.assertFalse([o for o in theirs["objects"].values() if o["zone"] == "library"], "701.23a: the searcher alone")
        done = me.answer({"kind": "choose", "answer": {"objects": [bolt]}})
        [found] = done["players"][0]["hand"]
        self.assertEqual(done["objects"][found]["name"], "Lightning Bolt")

    def test_requests_for_another_session_seat_or_request_are_refused_and_change_nothing(self):
        me, them = self.new({"hand": ["Lightning Bolt"], "battlefield": ["Mountain"]}, {"hand": ["Shock"]})
        before = me.prompt()
        self.assertIn("stale session_id", self.refused(me, {"cmd": "answer", "session_id": 99, "answer": {"kind": "pass"}}))
        self.assertIn("stale", self.refused(me, {"cmd": "answer", "request_id": before["request_id"] - 1, "answer": {"kind": "pass"}}))
        self.assertIn("request belongs to player 0", self.refused(them, {"cmd": "answer", "request_id": before["request_id"], "answer": {"kind": "pass"}}))
        self.assertIn("no player 7", self.refused(me, {"cmd": "prompt", "seat": "p7"}))
        self.assertIn("player label", self.refused(me, {"cmd": "prompt", "seat": "o1"}))
        self.assertIn("names nothing", self.refused(me, {"cmd": "answer", "answer": {"kind": "cast", "card": "o42"}}))
        self.assertIn("ability label", self.refused(me, {"cmd": "answer", "answer": {"kind": "activate", "ability": before["players"][0]["hand"][0]}}))
        after = me.prompt()
        self.assertEqual((after["request_id"], after["objects"], after["events"]), (before["request_id"], before["objects"], []))


    def test_a_modal_spell_reads_with_its_modes_and_announces_the_one_chosen(self):
        me, them = self.new({"hand": ["Abrade"], "battlefield": ["Mountain", "Mountain"]}, {"battlefield": ["Myr Retriever"]})
        prompt = me.prompt()
        [abrade] = self.named(prompt, "Abrade")
        text = ["Choose one —\n• Abrade deals 3 damage to target creature.\n• Destroy target artifact."]
        self.assertEqual(prompt["objects"][abrade]["text"], text)
        [retriever] = self.named(prompt, "Myr Retriever")
        me.answer({"kind": "cast", "card": abrade, "mana": {"R": 2}, "modes": [1], "targets": [[retriever]],
                   "plan": [{"ability": f"{m}.a0"} for m in self.named(prompt, "Mountain")]})
        # 601.2b, 700.2a: the mode is announced; the other seat reads it, with the target
        theirs = them.prompt()
        [spell] = theirs["stack"]
        self.assertEqual(theirs["objects"][spell]["text"], text)
        self.assertEqual(theirs["objects"][spell]["occurrence"]["modes"], [{"index": 1, "text": "• Destroy target artifact."}])
        self.assertEqual(theirs["objects"][spell]["targets"], [self.named(theirs, "Myr Retriever")])

    def test_r702_174a_a_gift_is_promised_by_player_label_and_both_seats_see_the_promise(self):
        me, them = self.new({"hand": ["Into the Flood Maw"], "battlefield": ["Island"]}, {"battlefield": ["Myr Retriever"]})
        prompt = me.prompt()
        [maw] = self.named(prompt, "Into the Flood Maw")
        [retriever] = self.named(prompt, "Myr Retriever")
        answer = {"kind": "cast", "card": maw, "mana": {"U": 1}, "gift": "p1", "targets": [[], [retriever]],
                  "plan": [{"ability": f"{self.named(prompt, 'Island')[0]}.a0"}]}
        self.assertIn('player label ("p1")', self.refused(me, {"cmd": "answer", "answer": answer | {"gift": "o1"}}))
        me.answer(answer)
        # 702.174a: the promise is made as the spell is cast; every seat reads it on the stack
        for seat in (me, them):
            view = seat.prompt()
            [spell] = view["stack"]
            self.assertEqual(view["objects"][spell]["gift"], "p1", seat.label)

    def test_r601_2b_the_alternative_announced_and_the_permanent_beheld_are_public(self):
        # Kinsbaile Aspirant: "As an additional cost to cast this spell, behold a Kithkin or pay {2}."
        me, them = self.new({"hand": ["Kinsbaile Aspirant"], "battlefield": ["Plains", "Kinsbaile Aspirant"]}, {})
        prompt = me.prompt()
        them.prompt()
        [card] = self.named(prompt, "Kinsbaile Aspirant", zone="hand")
        [kithkin] = self.named(prompt, "Kinsbaile Aspirant", zone="battlefield")
        after = me.answer({"kind": "cast", "card": card, "mana": {"W": 1}, "chose": [0], "paying": [[kithkin]],
                           "plan": [{"ability": f"{self.named(prompt, 'Plains')[0]}.a0"}]})
        theirs = them.prompt()
        # every seat reads the alternative announced on the stack, and the permanent beheld (701.4a)
        for view in (after, theirs):
            [spell] = view["stack"]
            self.assertEqual(view["objects"][spell]["announced"], ["behold a Kithkin"])
            self.assertIn({"event": "beheld", "player": "p0", "permanents": [kithkin]}, view["events"])

    def test_r605_3a_a_forage_mana_ability_is_a_step_of_a_plan(self):
        # Thornvault Forager: "{T}, Forage: Add two mana in any combination of colors." With
        # three cards in the graveyard and no Food, it pays for Grizzly Bears by exiling them.
        me, them = self.new({"hand": ["Grizzly Bears"], "battlefield": ["Thornvault Forager"],
                             "graveyard": ["Island", "Island", "Island"]}, {})
        prompt = me.prompt()
        [bears] = self.named(prompt, "Grizzly Bears")
        [forager] = self.named(prompt, "Thornvault Forager")
        step = {"ability": f"{forager}.a1", "chose": [0],
                "paying": [list(prompt["players"][0]["graveyard"])], "adding": [["G", "G"]]}
        after = me.answer({"kind": "cast", "card": bears, "mana": {"G": 2}, "plan": [step]})
        [spell] = after["stack"]
        self.assertEqual(after["objects"][spell]["name"], "Grizzly Bears")
        self.assertEqual(after["players"][0]["graveyard"], [], "701.61a: the three cards are exiled")

    def test_r601_2b_a_forage_step_paid_by_the_alternative_the_player_says(self):
        # Thornvault Forager with a Food and three graveyard cards: either alternative pays
        # (701.61a); a plan step saying "chose": [1] and the Food sacrifices it, the graveyard
        # kept, which the other seat sees.
        me, them = self.new({"hand": ["Grizzly Bears"], "battlefield": ["Thornvault Forager", "Nutrient Block"],
                             "graveyard": ["Island", "Island", "Island"]}, {})
        prompt = me.prompt()
        [bears] = self.named(prompt, "Grizzly Bears")
        [forager] = self.named(prompt, "Thornvault Forager")
        [food] = self.named(prompt, "Nutrient Block")
        said = {"ability": f"{forager}.a1", "chose": [1], "paying": [[food]], "adding": [["G", "G"]]}
        after = me.answer({"kind": "cast", "card": bears, "mana": {"G": 2}, "plan": [said]})
        theirs = them.prompt()
        kept = [after["objects"][o]["name"] for o in after["players"][0]["graveyard"]]
        self.assertEqual(sorted(kept), ["Island"] * 3 + ["Nutrient Block"], "the graveyard kept, the Food put there")
        for view in (after, theirs):
            self.assertNotIn(food, [o for o, seen in view["objects"].items() if seen["zone"] == "battlefield"], "701.61a: the Food is sacrificed")
            self.assertTrue(any(view["objects"][s]["name"] == "Grizzly Bears" for s in view["stack"]))

    def test_r605_3_nested_labeled_plan_spends_green_before_grotto_produces_white(self):
        # CR 602.2b/605.3: Grotto's {1} is paid before adding W; Lions costs W.
        me, _ = self.new({"hand": ["Savannah Lions"],
                          "battlefield": ["Forest", "Shimmering Grotto"]}, {})
        prompt = me.prompt()
        forest, grotto, lions = (self.named(prompt, name)[0]
                                for name in ("Forest", "Shimmering Grotto", "Savannah Lions"))
        after = me.answer({"kind": "cast", "card": lions, "targets": [],
                           "plan": [{"ability": forest + ".a0", "mana": {}},
                                    {"ability": grotto + ".a1", "mana": {"G": 1}, "adding": [["W"]]}],
                           "mana": {"W": 1}})
        self.assertEqual([after["objects"][o]["name"] for o in after["stack"]], ["Savannah Lions"])
        self.assertTrue(all(after["objects"][o]["tapped"] for o in (forest, grotto)))
        self.assertEqual((after["players"][0]["pool"]["G"], after["players"][0]["pool"]["W"]), (0, 0))

    def test_r106_labeled_plan_selects_post_plan_path_lot_and_keeps_its_scry(self):
        # Path's marked mana retains its spending rider (106.6); changeling shares
        # Isamaru's creature type, so spending the selected lot triggers the scry.
        self.engine.send({"cmd": "start", "format": "commander", "seed": 16,
                          "decks": [["Path of Ancestry", "Universal Automaton"] + ["Plains"] * 97,
                                    ["Plains"] * 99, ["Plains"] * 99],
                          "commanders": ["Isamaru, Hound of Konda"] * 3})
        self.seats = [play.Seat(self.engine, i) for i in range(3)]
        def main_after(turn):
            for _ in range(300):
                seat = self.waiting()
                view = seat.last
                if (view["turn"] > turn and view["active"] == "p0" and view["part"] == "PrecombatMain"
                        and view["waiting_for"] == "p0" and view["decision"]["kind"] == "priority"):
                    return self.seats[0].prompt()
                kind = view["decision"]["kind"]
                self.assertIn(kind, ("mulligan", "priority", "discard"))
                if kind == "discard":
                    # These decks deliberately have only Plains beyond the two named cards.
                    plains = self.named(view, "Plains", controller=seat.label, zone="hand")
                    count = view["decision"]["count"]
                    self.assertGreaterEqual(len(plains), count)
                    seat.answer({"kind": "discard", "cards": plains[:count]})
                else:
                    seat.answer({"kind": "keep"} if kind == "mulligan" else {"kind": "pass"})
            self.fail("did not reach the next authored main phase")
        prompt = main_after(-1)
        me = self.seats[0]
        path = self.named(prompt, "Path of Ancestry", zone="hand")[0]
        after = me.answer({"kind": "play", "card": path})
        self.assertTrue(after["objects"][self.named(after, "Path of Ancestry", zone="battlefield")[0]]["tapped"])
        prompt = main_after(after["turn"])
        path = self.named(prompt, "Path of Ancestry", zone="battlefield")[0]
        automaton = self.named(prompt, "Universal Automaton", zone="hand")[0]
        self.assertFalse(prompt["objects"][path]["tapped"], "Path untapped on its controller's next turn")
        after = me.answer({"kind": "cast", "card": automaton,
                           "plan": [{"ability": path + ".a1", "mana": {}}],
                           "mana": {"restricted_after": [0]}})
        self.assertTrue(after["objects"][path]["tapped"])
        self.assertEqual(after["players"][0]["pool"]["restricted"], [])
        self.assertEqual(len(after["stack"]), 2)
        seat = self.pass_until_asked()
        self.assertIs(seat, me)
        self.assertEqual((seat.last["decision"]["kind"], seat.last["decision"]["verb"]), ("arrange", "Scry"))

if __name__ == "__main__":
    unittest.main()
