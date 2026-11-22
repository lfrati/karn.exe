"""Turn schedule checks through the actual Python/JSON client: a skipped draw
from client_support import ClientTestCase
step (614.10), an additional combat and main phase (500.8), and the active player's choice
between skip effects that last differently (616.1)."""


class TurnScheduleClientTests(ClientTestCase):
    # Events are delivered to each seat once, with the next answer that seat sees: these
    # checks read seat 0's, from every answer it sees and its own state request at the end.
    def act(self, move):
        state = self.engine.act(move)
        if state["viewer"] == 0:
            self.log.extend(state["events"])

    def watch(self):
        """Seat 0's events from now on."""
        self.view(0)
        self.log = []

    def seen(self):
        self.log.extend(self.view(0)["events"])
        return self.log

    def advance(self, predicate):
        """The least each player can do until `predicate`, keeping the events seen on the way."""
        for _ in range(600):
            state = self.engine.state
            if predicate(state):
                return
            asked = state["asked"]
            if asked["kind"] == "attackers":
                self.act({"kind": "attack", "attacks": []})
            elif asked["kind"] == "blockers":
                self.act({"kind": "block", "blocks": []})
            else:
                self.assertEqual(asked["kind"], "priority", state)
                self.act({"kind": "pass"})
        self.fail("did not reach checkpoint")

    def resolve(self):
        for _ in self.engine.state["players"]:
            self.act({"kind": "pass"})

    def main(self, active, turn=None):
        return lambda s: (s["active"] == active and s["part"] == "PrecombatMain" and not s["stack"]
                          and s["asked"]["kind"] == "priority" and (turn is None or s["turn"] == turn))

    def test_fatigue_skips_only_the_opponents_next_draw_step(self):
        self.log = []
        self.new([{"hand": ["Fatigue"], "battlefield": ["Island", "Island"]}, {}])
        self.cast("Fatigue", [[{"player": 1}]], mana=self.ids("Island", zone="battlefield", controller=0)[:2], spend={'U': 2})
        self.resolve()
        skips = self.engine.state["skips"]
        self.assertEqual([(s["player"], s["what"], s["lasts"]) for s in skips], [(1, {"step": "Draw"}, "next")])
        self.assertEqual(self.view(1)["skips"], skips, "a resolved skip effect is public")
        hand = len(self.hand(1))
        self.watch()
        self.advance(self.main(1))
        turn = self.engine.state["turn"]
        self.assertIn({"event": "skipped", "player": 1, "what": {"step": "Draw"}, "skip": skips[0]["id"]},
                      [{k: e[k] for k in ("event", "player", "what", "skip") if k in e} for e in self.seen()])
        began = [e["part"] for e in self.seen() if e["event"] == "began"]
        self.assertNotIn("Draw", began, "the skipped draw step never began")
        self.assertIn("Upkeep", began)
        self.assertEqual(len(self.hand(1)), hand)
        self.assertNotIn("skips", self.engine.state)
        self.assertNotIn("upcoming", self.engine.state, "a skipped step changes no phase: an ordinary turn reads as before")
        self.watch()
        self.advance(lambda s: self.main(1)(s) and s["turn"] > turn)
        self.assertIn("Draw", [e["part"] for e in self.seen() if e["event"] == "began"])
        self.assertFalse(any(e["event"] == "skipped" for e in self.seen()))
        self.assertEqual(len(self.hand(1)), hand + 1)

    def test_seize_the_day_gives_an_additional_combat_and_main_phase(self):
        self.log = []
        self.new([{"hand": ["Seize the Day"], "battlefield": ["Grizzly Bears"] + ["Mountain"] * 4}, {}])
        bears, = self.ids("Grizzly Bears")
        self.assertNotIn("upcoming", self.engine.state)
        self.advance(lambda s: s["asked"]["kind"] == "attackers")
        self.act({"kind": "attack", "attacks": [[bears, {"player": 1}]]})
        self.advance(lambda s: s["part"] == "PostcombatMain" and s["asked"]["kind"] == "priority" and not s["stack"])
        self.assertEqual(self.engine.state["players"][1]["life"], 18)
        self.assertTrue(self.object(bears)["tapped"])
        self.cast("Seize the Day", [[{"object": bears}]], mana=self.ids("Mountain", zone="battlefield", controller=0)[:4], spend={'R': 4})
        self.resolve()
        state = self.engine.state
        self.assertFalse(self.object(bears)["tapped"])
        self.assertEqual([(p["phase"], p["added"]) for p in state["upcoming"]],
                         [("Combat", True), ("PostcombatMain", True), ("Ending", False)])
        self.assertEqual(state["phase"]["added"], False)
        combat_id = state["upcoming"][0]["id"]
        self.assertEqual(self.view(1)["upcoming"], state["upcoming"], "public to every player")
        self.watch()
        self.advance(lambda s: s["asked"]["kind"] == "attackers")
        self.assertEqual(self.engine.state["phase"], {"id": combat_id, "added": True})
        self.act({"kind": "attack", "attacks": [[bears, {"player": 1}]]})
        self.advance(lambda s: s["part"] == "PostcombatMain" and s["asked"]["kind"] == "priority" and not s["stack"])
        self.assertEqual(self.engine.state["players"][1]["life"], 16)
        self.assertTrue(self.engine.state["phase"]["added"])
        self.advance(lambda s: s["active"] == 1)
        began = [e["part"] for e in self.seen() if e["event"] == "began"]
        self.assertEqual(began[:began.index("End")],
                         ["BeginningOfCombat", "DeclareAttackers", "DeclareBlockers", "CombatDamage", "EndOfCombat", "PostcombatMain"])
        self.assertEqual(began.count("Cleanup"), 1)

    def competing(self):
        """Player 0 at turn 3 with an older skip of their next combat phase (Stonehorn Dignitary's)
        and a newer one of their next combat phase this turn (Moment of Silence): asked which."""
        self.log = []
        self.new([{"hand": ["Moment of Silence"], "battlefield": ["Plains"]},
                  {"hand": ["Stonehorn Dignitary"], "battlefield": ["Plains"] * 4}])
        self.advance(self.main(1))
        self.cast("Stonehorn Dignitary", mana=self.ids("Plains", zone="battlefield", controller=1)[:4], spend={'W': 4})
        self.resolve()
        if self.engine.state["asked"]["kind"] == "targets":
            self.act({"kind": "targets", "targets": [[{"player": 0}]]})
        self.resolve()
        self.assertEqual([(s["player"], s["what"], s["lasts"]) for s in self.engine.state["skips"]], [(0, {"phase": "Combat"}, "next")])
        self.advance(self.main(0, turn=3))
        self.cast("Moment of Silence", [[{"player": 0}]], mana=self.ids("Plains", zone="battlefield", controller=0)[:1], spend={'W': 1})
        self.resolve()
        self.watch()
        self.resolve()
        state = self.engine.state
        asked = state["asked"]
        self.assertEqual((asked["kind"], asked["player"], asked["what"]), ("skip", 0, {"phase": "Combat"}))
        self.assertEqual([s["lasts"] for s in asked["among"]], ["next", {"this_turn": 3}])
        self.assertEqual(state["skipping"], {"phase": "Combat"})
        self.assertEqual(state["part"], "PrecombatMain", "nothing of the combat phase began")
        return asked

    def test_competing_skips_are_the_active_players_choice_answered_either_way(self):
        asked = self.competing()
        older, moment = asked["among"]
        self.reject(1, {"kind": "choose", "answer": {"skip": moment["id"]}})
        self.reject(0, {"kind": "pass"})
        self.reject(0, {"kind": "choose", "answer": {"skip": 999}})
        self.act({"kind": "choose", "answer": {"skip": moment["id"]}})
        self.assertEqual(self.engine.state["part"], "PostcombatMain")
        self.assertIn({"event": "skipped", "player": 0, "what": {"phase": "Combat"}, "skip": moment["id"]},
                      [{k: e[k] for k in ("event", "player", "what", "skip") if k in e} for e in self.seen()])
        self.assertEqual([e["part"] for e in self.seen() if e["event"] == "began"], ["PostcombatMain"],
                         "no step of the skipped combat phase began")
        self.assertEqual([s["id"] for s in self.engine.state["skips"]], [older["id"]], "the older skip waits")
        self.assertNotIn("skipping", self.engine.state)
        self.watch()
        self.advance(self.main(0, turn=5))
        self.advance(lambda s: s["part"] == "PostcombatMain")
        self.assertIn("skipped", [e["event"] for e in self.seen()], "turn 5's combat is skipped")

    def test_answered_with_the_older_skip_this_turns_expires(self):
        asked = self.competing()
        older, moment = asked["among"]
        self.act({"kind": "choose", "answer": {"skip": older["id"]}})
        self.assertEqual([s["id"] for s in self.engine.state["skips"]], [moment["id"]])
        self.advance(lambda s: s["turn"] == 4)
        self.assertNotIn("skips", self.engine.state, "this turn's skip ended with the turn")
        self.watch()
        self.advance(lambda s: s["turn"] == 5 and s["part"] == "PostcombatMain")
        self.assertIn("BeginningOfCombat", [e["part"] for e in self.seen() if e["event"] == "began"])
