"""Multiplayer format witnesses beyond seeded full-deck games."""
from client_support import ClientTestCase


class CommanderCompletionClientTests(ClientTestCase):
    def test_eliminated_seat_cannot_act_or_be_targeted_and_future_turn_is_skipped(self):
        self.engine.ask({"cmd": "new", "life": 3, "players": [
            {"library": ["Forest"] * 8, "battlefield": ["Mountain", "Mountain"],
             "hand": ["Lightning Bolt", "Shock"]},
            *[{"library": ["Forest"] * 8} for _ in range(3)]]})
        self.advance(lambda s: s["part"] == "PrecombatMain" and s["asked"]["kind"] == "priority")
        mountains = self.ids("Mountain")
        self.cast("Lightning Bolt", [[{"player": 1}]], mana=mountains[:1], spend={'R': 1})
        self.pass_round()
        self.assertTrue(self.engine.state["players"][1]["lost"])
        self.reject(1, {"kind": "pass"})
        self.reject(0, {"kind": "cast", "card": self.ids("Shock")[0], "mana": {"R": 1},
                        "plan": [{"source": mountains[1], "ability": 0}],
                        "targets": [[{"player": 1}]], "paying": []})
        self.engine.act({"kind": "pass"})
        self.assertEqual(self.engine.state["asked"]["player"], 2)
        self.advance(lambda s: s["turn"] == 2)
        self.assertEqual(self.engine.state["active"], 2)

    def test_borrowed_creature_sacrificed_as_cost_goes_to_its_owner(self):
        self.new([{"battlefield": ["Mountain"] * 3 + ["Swamp"],
                   "hand": ["Act of Treason", "Village Rites"]},
                  {"battlefield": ["Grizzly Bears"]}, {}, {}])
        bear = self.ids("Grizzly Bears")[0]
        self.cast("Act of Treason", [[{"object": bear}]], mana=self.ids("Mountain"), spend={'R': 3})
        self.pass_round()
        self.assertEqual(self.object(bear)["controller"], 0)
        self.cast("Village Rites", mana=self.ids("Swamp"), paying=[[bear]], spend={'B': 1})
        self.assertNotIn(bear, self.engine.state["battlefield"])
        for viewer in range(4):
            state = self.view(viewer)
            self.assertEqual(len(state["players"][1]["graveyard"]), 1)
            card = state["players"][1]["graveyard"][0]
            self.assertEqual(state["objects"][str(card)]["name"], "Grizzly Bears")
            self.assertEqual(state["objects"][str(card)]["owner"], 1)
        self.pass_round()

    def play_plains(self):
        actor = self.engine.state["asked"]["player"]
        card = next(o for o in self.hand(actor) if self.object(o)["name"] == "Plains")
        self.engine.act({"kind": "play", "card": card})

    def test_managed_commander_owner_return_and_recast_tax(self):
        self.engine.ask({"cmd": "start", "format": "commander", "seed": 23, "first": 0,
                         "decks": [["Plains"] * 98 + ["Swords to Plowshares"], ["Plains"] * 99, ["Plains"] * 99],
                         "commanders": ["Isamaru, Hound of Konda"] * 3})
        self.assertEqual([p["life"] for p in self.engine.state["players"]], [40] * 3)
        self.advance(lambda s: s["part"] == "PrecombatMain" and s["asked"]["kind"] == "priority")
        self.play_plains()
        self.advance(lambda s: s["active"] == 1 and s["part"] == "PrecombatMain" and s["asked"]["kind"] == "priority")
        self.play_plains()
        leader = next(o for o in self.ids("Isamaru, Hound of Konda") if self.object(o)["owner"] == 1)
        self.cast("Isamaru, Hound of Konda", card=leader, mana=self.ids("Plains", "battlefield", 1), spend={'W': 1})
        self.pass_round()
        leader = next(i for i in self.ids("Isamaru, Hound of Konda") if i in self.engine.state["battlefield"])
        self.advance(lambda s: s["active"] == 0 and s["turn"] > 1 and s["part"] == "PrecombatMain" and s["asked"]["kind"] == "priority")
        self.cast("Swords to Plowshares", [[{"object": leader}]], mana=self.ids("Plains", "battlefield", 0), spend={'W': 1})
        self.pass_round()
        self.assertEqual((self.engine.state["asked"]["kind"], self.engine.state["asked"]["player"]), ("command_zone", 1))
        self.reject(0, {"kind": "choose", "answer": "yes"})
        self.choose("yes")
        returned = next(i for i in self.ids("Isamaru, Hound of Konda") if self.object(i)["owner"] == 1)
        self.assertTrue(self.object(returned)["commander"])
        self.assertNotIn(returned, self.engine.state["battlefield"])
        self.assertEqual(self.engine.state["players"][1]["life"], 42)
        self.advance(lambda s: s["active"] == 1 and s["turn"] > 2 and s["part"] == "PrecombatMain" and s["asked"]["kind"] == "priority")
        self.play_plains()
        self.reject(1, {"kind": "cast", "card": returned, "mana": {"W": 2},
                        "plan": [{"source": o, "ability": 0} for o in self.ids("Plains", "battlefield", 1)],
                        "targets": [], "paying": []})
        turn = self.engine.state["turn"]
        self.advance(lambda s: s["active"] == 1 and s["turn"] > turn and s["part"] == "PrecombatMain" and s["asked"]["kind"] == "priority")
        self.play_plains()
        self.cast("Isamaru, Hound of Konda", card=returned, mana=self.ids("Plains", "battlefield", 1), spend={'W': 3})
        self.pass_round()
        self.assertEqual(self.engine.state["players"][1]["commanders"][0]["casts"], [0, 2, 0])
