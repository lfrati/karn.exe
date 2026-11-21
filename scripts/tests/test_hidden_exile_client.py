"""Face-down exile checks through the actual Python/JSON client: Watcher for
Tomorrow's hideaway hides a card only its player may look at (406.3, 702.75a), with no name or
types for anyone (406.3a), and returns it to its owner's hand when Watcher leaves."""
import unittest
from test_grant_client import GrantClientTests as Helpers


class HiddenExileClientTests(unittest.TestCase):
    setUpClass = classmethod(Helpers.setUpClass.__func__)
    setUp = Helpers.setUp
    close_engine = Helpers.close_engine
    new = Helpers.new
    view = Helpers.view
    ids = Helpers.ids
    hand = Helpers.hand
    pass_round = Helpers.pass_round
    choose = Helpers.choose
    reject = Helpers.reject
    cast = Helpers.cast

    def hide(self):
        """Player 0 casts Watcher for Tomorrow over Grizzly Bears, Forest, Llanowar Elves and
        Island, and hides the Bears: the hidden card's id."""
        # of a library the last name is on top
        self.new([{"hand": ["Watcher for Tomorrow"], "battlefield": ["Island", "Island"],
                   "library": ["Forest"] * 4 + ["Island", "Llanowar Elves", "Forest", "Grizzly Bears"]},
                  {"hand": ["Unsummon"], "battlefield": ["Island"]}])
        self.cast("Watcher for Tomorrow", mana=self.ids("Island", zone="battlefield", controller=0)[:2], spend={'U': 2})
        self.pass_round()
        self.pass_round()
        asked = self.engine.state["asked"]
        self.assertEqual((asked["kind"], asked["player"], len(asked["among"])), ("objects", 0, 4))
        mine = self.view(0)
        bears = next(o for o in asked["among"] if mine["objects"][str(o)]["name"] == "Grizzly Bears")
        self.assertIsNone(self.view(1)["asked"].get("among"), "the other seat is not shown the cards looked at")
        self.choose({"objects": [bears]})
        self.pass_round()
        hidden = [int(i) for i, o in self.view(0)["objects"].items() if "hidden" in o]
        self.assertEqual(len(hidden), 1)
        return hidden[0]

    def test_hideaway_card_is_shown_to_its_player_only_with_no_decision(self):
        card = self.hide()
        watcher = next(int(i) for i, o in self.view(0)["objects"].items() if o["name"] == "Watcher for Tomorrow")
        mine, theirs = self.view(0), self.view(1)
        self.assertEqual(mine["objects"][str(card)]["hidden"], {"lookers": [0], "card": {"name": "Grizzly Bears"}, "watcher": watcher})
        self.assertEqual(theirs["objects"][str(card)]["hidden"], {"lookers": [0], "card": None, "watcher": watcher})
        for seen in (mine, theirs):
            self.assertEqual((seen["objects"][str(card)]["name"], seen["objects"][str(card)]["types"]), ("", []))
            self.assertIn(card, seen["exile"])
        self.assertNotIn("Grizzly Bears", str(theirs))
        self.assertEqual(self.engine.state["asked"]["kind"], "priority", "nobody is asked whether to look")

    def test_bounced_watcher_returns_the_hidden_card_to_its_owners_hand(self):
        card = self.hide()
        watcher = next(int(i) for i, o in self.view(0)["objects"].items() if o["name"] == "Watcher for Tomorrow")
        self.engine.act({"kind": "pass"})
        self.cast("Unsummon", [[{"object": watcher}]], mana=self.ids("Island", zone="battlefield", controller=1)[:1], spend={'U': 1})
        self.pass_round()
        self.pass_round()
        names = [self.view(0)["objects"][str(o)]["name"] for o in self.hand(0)]
        self.assertIn("Grizzly Bears", names)
        self.assertIn("Watcher for Tomorrow", names)
        theirs = self.view(1)
        self.assertNotIn(str(card), [str(o) for o in theirs["exile"]])
        self.assertNotIn("Grizzly Bears", str(theirs), "it left exile for a hand, unrevealed")


del Helpers
