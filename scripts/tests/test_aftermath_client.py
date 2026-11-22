"""Real Aftermath halves through existing native cast choices and permissions."""
import play
from client_support import ClientTestCase

class AftermathClientTests(ClientTestCase):
    def split(self, zone, part):
        return next(o for o in self.engine.state["players"][0][zone] if part in self.object(o)["name"])

    def test_claim_then_fame_preserves_the_physical_card_and_selected_half(self):
        self.new([{"hand": ["Claim // Fame"], "graveyard": ["Grizzly Bears"],
                   "battlefield": ["Grizzly Bears", "Swamp", "Mountain", "Mountain"]}, {}])
        card = self.split("hand", "Claim")
        old, = self.ids("Grizzly Bears", zone="battlefield")
        grave, = self.engine.state["players"][0]["graveyard"]
        with self.assertRaisesRegex(play.Refused, "can't be cast from its current zone"):
            self.engine.act({"kind": "cast", "card": card, "cast_as": 1, "targets": [[{"object": old}]]})
        self.cast("Claim // Fame", [[{"object": grave}]], card=card, cast_as=0,
                  mana=self.ids("Swamp"), spend={"B": 1})
        self.pass_round()
        card = self.split("graveyard", "Claim")
        self.cast("Claim // Fame", [[{"object": old}]], card=card, cast_as=1,
                  mana=self.ids("Mountain"), spend={"R": 2})
        self.pass_round()
        self.assertEqual(self.object(old)["power"], 4)
        self.assertTrue(any("Claim" in self.object(o)["name"] for o in self.engine.state["exile"]))
        self.assertFalse(self.engine.state["players"][0]["graveyard"])

    def test_mind_is_an_instant_on_the_opponents_turn(self):
        self.new([{"graveyard": ["Spring // Mind"], "battlefield": ["Island"] * 6}, {}])
        self.advance(lambda state: state["active"] == 1 and state["part"] == "PrecombatMain"
                     and state["asked"]["kind"] == "priority" and state["asked"]["player"] == 0)
        card = self.split("graveyard", "Spring")
        before = len(self.hand(0))
        self.cast("Spring // Mind", card=card, cast_as=1, mana=self.ids("Island"), spend={"U": 6})
        self.pass_round()
        self.assertEqual(len(self.hand(0)), before + 2)
        self.assertTrue(any("Spring" in self.object(o)["name"] for o in self.engine.state["exile"]))

    def test_citadel_cannot_bypass_aftermath_but_can_cast_the_front_half(self):
        self.new([{"library": ["Forest"] * 8 + ["Claim // Fame"], "graveyard": ["Grizzly Bears"],
                   "battlefield": ["Bolas's Citadel", "Grizzly Bears"]}, {}])
        card = next(int(o) for o, obj in self.engine.state["objects"].items() if "Claim" in obj["name"])
        permit, = self.ids("Bolas's Citadel")
        old, = self.ids("Grizzly Bears", zone="battlefield")
        grave, = self.engine.state["players"][0]["graveyard"]
        route = {"source": permit, "ability": 1}
        with self.assertRaisesRegex(play.Refused, "can't be cast from its current zone"):
            self.engine.act({"kind": "cast", "card": card, "cast_as": 1, "route": route,
                             "targets": [[{"object": old}]]})
        self.cast("Claim // Fame", [[{"object": grave}]], card=card, cast_as=0, route=route)
        self.assertEqual(self.engine.state["players"][0]["life"], 19)
        self.pass_round()
        self.assertTrue(self.split("graveyard", "Claim"))
        self.assertFalse(self.engine.state["exile"])
