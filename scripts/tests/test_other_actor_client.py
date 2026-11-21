"""After a named actor, "each other creature" is other than that actor, through the real client."""
import unittest
from test_grant_client import GrantClientTests as Helpers


class OtherActorClientTests(unittest.TestCase):
    setUpClass = classmethod(Helpers.setUpClass.__func__)
    setUp = Helpers.setUp
    close_engine = Helpers.close_engine
    new = Helpers.new
    view = Helpers.view
    hand = Helpers.hand
    ids = Helpers.ids
    object = Helpers.object
    pass_round = Helpers.pass_round
    choose = Helpers.choose
    reject = Helpers.reject
    cast = Helpers.cast
    activate = Helpers.activate

    def dealt(self):
        return {(e["source"], e["to"].get("object", ("player", e["to"].get("player"))), e["amount"])
                for e in self.engine.state["events"] if e["event"] == "damage"}

    def test_chandras_ignition_target_actor_is_not_another_creature(self):
        # the spell is the effect source; the target creature deals the damage (608.2h)
        self.new([{"hand": ["Chandra's Ignition"], "battlefield": ["Craw Wurm", "Llanowar Elves"] + ["Mountain"] * 5},
                  {"battlefield": ["Grizzly Bears"]}])
        wurm, = self.ids("Craw Wurm")
        elves, = self.ids("Llanowar Elves")
        bears, = self.ids("Grizzly Bears")
        self.cast("Chandra's Ignition", [[{"object": wurm}]], mana=self.ids('Mountain', zone="battlefield", controller=self.engine.state["asked"]["player"])[0:5], spend={'R': 5})
        self.pass_round()
        self.assertEqual(self.dealt(), {(wurm, elves, 6), (wurm, bears, 6), (wurm, ("player", 1), 6)})
        self.assertEqual(self.engine.state["players"][1]["life"], 14)
        self.assertEqual(self.engine.state["players"][0]["life"], 20)
        for viewer in (0, 1):
            view = self.view(viewer)
            self.assertIn(wurm, view["battlefield"])
            self.assertEqual(view["objects"][str(wurm)]["damage"], 0)
            self.assertNotIn(elves, view["battlefield"])
            self.assertNotIn(bears, view["battlefield"])

    def test_fire_ants_self_naming_source_still_excludes_itself(self):
        # "This creature deals 1 damage to each other creature without flying": source-relative
        self.new([{"battlefield": ["Fire Ants", "Llanowar Elves"]}, {"battlefield": ["Grizzly Bears", "Suntail Hawk"]}])
        ants, = self.ids("Fire Ants")
        elves, = self.ids("Llanowar Elves")
        bears, = self.ids("Grizzly Bears")
        self.activate(ants, [], ability=0)
        self.pass_round()
        self.assertEqual(self.dealt(), {(ants, elves, 1), (ants, bears, 1)})
        self.assertEqual(self.object(ants)["damage"], 0)
        self.assertEqual(self.object(bears)["damage"], 1)


del Helpers
