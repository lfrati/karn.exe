"""Foretell checks through the actual Python/JSON client: Demon Bolt is
from client_support import ClientTestCase
foretold during its player's turn with a spell on the stack (116.2h, 702.143a), only that player
is shown it (406.3), and on the next turn it is cast for its foretell cost {R} (702.143a)."""


class ForetellClientTests(ClientTestCase):
    def foretell(self):
        """Player 0 casts Lightning Bolt at player 1, then foretells Demon Bolt with it on the
        stack: the foretold card's id."""
        # two Mountains remain to test other-turn refusal with sufficient resources
        self.new([{"hand": ["Demon Bolt", "Demon Bolt", "Lightning Bolt"], "battlefield": ["Mountain"] * 5},
                  {"battlefield": ["Grizzly Bears"]}])
        self.cast("Lightning Bolt", [[{"player": 1}]], mana=self.ids("Mountain", zone="battlefield", controller=0)[:1], spend={'R': 1})
        self.assertEqual(len(self.engine.state["stack"]), 1)
        demon = next(o for o in self.hand(0) if self.engine.state["objects"][str(o)]["name"] == "Demon Bolt")
        self.engine.act({"kind": "foretell", "card": demon, "mana": {"R": 2},
                         "plan": [{"source": o, "ability": 0} for o in self.ids("Mountain", zone="battlefield", controller=0)[1:3]]})
        self.assertEqual(len(self.engine.state["stack"]), 1, "a special action, with any stack")
        [card] = [int(i) for i, o in self.view(0)["objects"].items() if "foretold" in o]
        self.pass_round()
        return card

    def test_foretold_card_is_shown_only_to_its_player_and_cast_next_turn_for_its_foretell_cost(self):
        card = self.foretell()
        turn = self.engine.state["turn"]
        mine, theirs = self.view(0), self.view(1)
        self.assertEqual(mine["objects"][str(card)]["hidden"], {"lookers": [0], "card": {"name": "Demon Bolt"}})
        self.assertEqual(theirs["objects"][str(card)]["hidden"], {"lookers": [0], "card": None})
        for seen in (mine, theirs):
            self.assertEqual(seen["objects"][str(card)]["foretold"], {"turn": turn})
        self.assertNotIn("Demon Bolt", str(theirs))
        bears = self.ids("Grizzly Bears")[0]
        self.reject(0, {"kind": "cast", "card": card, "route": "foretell", "mana": {"R": 1}, "targets": [[{"object": bears}]],
                        "plan": [{"source": self.ids("Mountain", zone="battlefield", controller=0)[3], "ability": 0}]})
        # to player 1's main phase, where player 1 passes priority to player 0
        for _ in range(40):
            state = self.engine.state
            if state["active"] == 1 and state["part"] == "PrecombatMain" and state["asked"]["player"] == 0:
                break
            self.engine.act({"kind": "attack", "attacks": []} if state["asked"]["kind"] == "attackers" else {"kind": "pass"})
        self.assertEqual((self.engine.state["active"], self.engine.state["asked"]["player"]), (1, 0))
        demon = next(o for o in self.hand(0) if self.engine.state["objects"][str(o)]["name"] == "Demon Bolt")
        self.reject(0, {"kind": "foretell", "card": demon, "mana": {"R": 2},
                        "plan": [{"source": o, "ability": 0} for o in self.ids("Mountain", zone="battlefield", controller=0)[3:5]]})
        self.engine.act({"kind": "cast", "card": card, "route": "foretell", "mana": {"R": 1}, "targets": [[{"object": bears}]],
                         "plan": [{"source": self.ids("Mountain", zone="battlefield", controller=0)[3], "ability": 0}]})
        self.pass_round()
        self.assertNotIn(bears, self.view(1)["battlefield"])
        self.assertIn("Demon Bolt", [self.view(1)["objects"][str(o)]["name"] for o in self.view(1)["players"][0]["graveyard"]])
