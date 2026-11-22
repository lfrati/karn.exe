"""The non-recipient mixed-target refusal changes no
from client_support import ClientTestCase
admitted program, so these are the pinned cards nearest the boundary. A named source, the
source's replacement "it", a token's own body and a clause possessor keep their referent through
the real audited client, with exact damage records and both viewers."""


class MixedReferenceRefusalReviewClientTests(ClientTestCase):
    def dealt(self):
        return sorted((e["source"], next(iter(e["to"].items())), e["amount"])
                      for e in self.engine.state["events"] if e["event"] == "damage")

    def life(self, player):
        return [self.view(v)["players"][player]["life"] for v in (0, 1)]

    def test_beacon_of_destruction_shuffles_itself_into_its_owners_library(self):
        # "Shuffle Beacon of Destruction into its owner's library": the source named again
        self.new([{"hand": ["Beacon of Destruction"], "battlefield": ["Mountain"] * 5}, {}])
        self.cast("Beacon of Destruction", [[{"player": 1}]], mana=self.ids('Mountain', zone="battlefield", controller=0)[:5], spend={'R': 5})
        spell = self.engine.state["stack"][-1]
        self.pass_round()
        self.assertEqual(self.dealt(), [(spell, ("player", 1), 5)])
        self.assertEqual(self.life(1), [15, 15])
        events = self.engine.state["events"]
        self.assertIn({"event": "moved", "object": spell, "to": "Library"}, events)
        self.assertIn({"event": "shuffled", "player": 0, "by": 0}, events)
        for viewer in (0, 1):
            graveyard = self.view(viewer)["players"][0]["graveyard"]
            self.assertFalse(any(self.view(viewer)["objects"].get(str(o), {}).get("name") == "Beacon of Destruction" for o in graveyard))

    def test_frost_bite_replacement_it_is_the_spell(self):
        # "If you control three or more snow permanents, it deals 3 damage instead"
        for lands, amount in [(["Snow-Covered Mountain"] * 3, 3), (["Mountain"] * 3, 2)]:
            with self.subTest(lands=lands[0]):
                self.new([{"hand": ["Frost Bite"], "battlefield": lands}, {"battlefield": ["Craw Wurm"]}])
                wurm, = self.ids("Craw Wurm")
                self.cast("Frost Bite", [[{"object": wurm}]], mana=self.ids(lands[0], zone="battlefield", controller=0)[:1], spend={"R": 1})
                spell = self.engine.state["stack"][-1]
                self.pass_round()
                self.assertEqual(self.dealt(), [(spell, ("object", wurm), amount)])
                self.assertEqual([self.view(v)["objects"][str(wurm)]["damage"] for v in (0, 1)], [amount] * 2)

    def test_make_mischief_token_gets_its_own_ability(self):
        # "Create a 1/1 red Devil creature token. It has …": the token, not the spell or target
        self.new([{"hand": ["Make Mischief"], "battlefield": ["Mountain"] * 3}, {}])
        self.cast("Make Mischief", [[{"player": 1}]], mana=self.ids('Mountain', zone="battlefield", controller=0)[:3], spend={'R': 3})
        spell = self.engine.state["stack"][-1]
        self.pass_round()
        self.assertEqual(self.dealt(), [(spell, ("player", 1), 1)])
        self.assertEqual(self.life(1), [19, 19])
        devil, = [i for i in self.ids("Devil Token") if i in self.engine.state["battlefield"]]
        for viewer in (0, 1):
            self.assertIn(devil, self.view(viewer)["battlefield"])
        self.assertTrue(self.object(devil)["token"])

    def test_pain_for_all_enchanted_creature_deals_its_power(self):
        # "enchanted creature deals damage equal to its power to any other target": the
        # amount belongs to the attached dealer, not the entering Aura. Both viewers see
        # its three damage.
        self.new([{"hand": ["Pain for All"], "battlefield": ["Hill Giant"] + ["Mountain"] * 3}, {}])
        giant, = self.ids("Hill Giant")
        self.cast("Pain for All", [[{"object": giant}]], mana=self.ids('Mountain', zone="battlefield", controller=0)[:3], spend={'R': 3})
        self.pass_round()
        asked = self.engine.state["asked"]
        self.assertEqual((asked["kind"], asked["player"]), ("targets", 0))
        self.engine.act({"kind": "targets", "targets": [[{"player": 1}]]})
        self.pass_round()
        self.assertEqual(self.dealt(), [(giant, ("player", 1), 3)])
        self.assertEqual(self.life(1), [17, 17])
