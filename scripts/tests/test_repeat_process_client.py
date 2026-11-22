"""Countryside Crusher's "repeat this process" (608.2c), through the real audited client.
Each time through reveals the card then on top to every player, one reveal at a time; the
lands go to the graveyard in that order, the first nonland card stays on top (ruling
948c4fe5), and the counter triggers for those lands are asked about only once the upkeep
ability has finished resolving (603.3, ruling 78f6e66c)."""
import unittest
from client_support import ClientTestCase


class RepeatProcessClientTests(ClientTestCase):
    def names(self, state, objects):
        return [state["objects"][str(o)]["name"] for o in objects]

    def upkeep(self, library):
        """Player 0 controls a Crusher; its library, top first. The game begins at the
        untap step, so the first upkeep trigger is that turn's."""
        self.engine.ask({"cmd": "new", "players": [
            {"battlefield": ["Countryside Crusher"], "library": list(reversed(library))},
            {"library": ["Forest"] * 5}]})
        for _ in range(10):
            if self.engine.state["stack"]:
                return
            self.engine.act({"kind": "pass"})
        self.fail("no upkeep trigger")

    def resolve_upkeep(self):
        trigger, = self.engine.state["stack"]
        while trigger in self.engine.state["stack"]:
            self.assertEqual(self.engine.state["asked"]["kind"], "priority", "nothing is asked while it resolves")
            self.engine.act({"kind": "pass"})

    def settle(self):
        for _ in range(40):
            state = self.engine.state
            if not state["stack"] and state["asked"]["kind"] == "priority":
                return
            if state["asked"]["kind"] == "order":
                self.engine.act({"kind": "order", "order": list(range(len(state["asked"]["abilities"])))})
            else:
                self.engine.act({"kind": "pass"})
        self.fail("stack did not empty")

    def revealed_to(self, player):
        """What this player was shown revealed since they last looked: the response to the
        last act went to the player then asked."""
        view = self.engine.state if self.engine.state["viewer"] == player else self.view(player)
        return [[card["name"] for card in row["cards"]] for row in view["observations"] if row["kind"] == "revealed"]

    def test_each_land_is_revealed_and_moved_in_turn_and_the_nonland_stays_on_top(self):
        self.upkeep(["Forest", "Mountain", "Lightning Bolt", "Forest"])
        self.resolve_upkeep()
        for player in (0, 1):
            self.assertEqual(self.revealed_to(player), [["Forest"], ["Mountain"], ["Lightning Bolt"]], player)
        whole = self.engine.send({"cmd": "state"})
        me = whole["players"][0]
        self.assertEqual(self.names(whole, me["graveyard"]), ["Forest", "Mountain"])
        self.assertEqual(me["library"], 2, "the nonland card stays in the library (the client shows its size)")
        asked = self.engine.state["asked"]
        on_stack = len(asked["abilities"]) if asked["kind"] == "order" else len(self.engine.state["stack"])
        self.assertEqual(on_stack, 2, "one counter trigger for each land, after the process")
        self.settle()
        crusher, = self.ids("Countryside Crusher")
        self.assertEqual(self.engine.state["objects"][str(crusher)].get("counters"), {"+1/+1": 2})

    def test_an_empty_library_reveals_nothing_and_loses_no_game(self):
        self.upkeep([])
        self.resolve_upkeep()
        self.assertEqual(self.revealed_to(1), [])
        self.assertEqual(self.engine.state["stack"], [])
        self.assertFalse(any(p.get("lost") for p in self.engine.state["players"]))


if __name__ == "__main__":
    unittest.main()
