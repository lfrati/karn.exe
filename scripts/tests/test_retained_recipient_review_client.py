"""The damage-recipient pronoun path through the real
audited client. Binding a bare "it" recipient to a mixed target changes no pinned card,
so these are the pinned cards that use a bare "it" recipient, or a named source and subject
pronoun beside a target, with exact damage records and both viewers."""
import unittest
from test_grant_client import GrantClientTests as Helpers


class RetainedRecipientReviewClientTests(unittest.TestCase):
    setUpClass = classmethod(Helpers.setUpClass.__func__)
    setUp = Helpers.setUp
    close_engine = Helpers.close_engine
    new = Helpers.new
    view = Helpers.view
    hand = Helpers.hand
    ids = Helpers.ids
    object = Helpers.object
    pass_round = Helpers.pass_round
    cast = Helpers.cast
    activate = Helpers.activate
    step = Helpers.step
    advance = Helpers.advance

    def dealt(self):
        return sorted((e["source"], next(iter(e["to"].items())), e["amount"])
                      for e in self.engine.state["events"] if e["event"] == "damage")

    def seen(self, obj, field):
        return [self.view(v)["objects"][str(obj)][field] for v in (0, 1)]

    def life(self, player):
        return [self.view(v)["players"][player]["life"] for v in (0, 1)]

    def test_caltrops_damages_only_the_attacking_creature(self):
        # "Whenever a creature attacks, this artifact deals 1 damage to it": the trigger subject
        self.new([{"battlefield": ["Caltrops", "Grizzly Bears", "Hill Giant"]}, {}])
        caltrops, = self.ids("Caltrops")
        bears, = self.ids("Grizzly Bears")
        giant, = self.ids("Hill Giant")
        self.advance(lambda s: s["asked"]["kind"] == "attackers")
        self.engine.act({"kind": "attack", "attacks": [[bears, {"player": 1}]]})
        self.advance(lambda s: any(e["event"] == "damage" for e in s["events"]))
        self.assertEqual(self.dealt(), [(caltrops, ("object", bears), 1)])
        self.assertEqual(self.seen(bears, "damage"), [1, 1])
        self.assertEqual(self.seen(giant, "damage"), [0, 0])

    def test_aether_flash_damages_the_entering_creature(self):
        # the opponent's Aether Flash: "Whenever a creature enters, … deals 2 damage to it"
        self.new([{"hand": ["Hill Giant"], "battlefield": ["Mountain"] * 4}, {"battlefield": ["Aether Flash"]}])
        flash, = self.ids("Aether Flash")
        self.cast("Hill Giant", mana=self.ids('Mountain', zone="battlefield", controller=self.engine.state["asked"]["player"])[0:4], spend={'R': 4})
        self.pass_round()
        giant, = self.ids("Hill Giant")
        self.advance(lambda s: any(e["event"] == "damage" for e in s["events"]))
        self.assertEqual(self.dealt(), [(flash, ("object", giant), 2)])
        self.assertEqual(self.seen(giant, "damage"), [2, 2])

    def test_garruk_target_deals_its_power_back_to_him(self):
        # "Garruk deals 3 damage to target creature. That creature deals damage equal to its power
        # to him": the dealer is the target and the recipient is the named source
        self.new([{"hand": ["Garruk Relentless"], "battlefield": ["Forest"] * 4}, {"battlefield": ["Grizzly Bears"]}])
        self.cast("Garruk Relentless", mana=self.ids('Forest', zone="battlefield", controller=self.engine.state["asked"]["player"])[0:4], spend={'G': 4})
        self.pass_round()
        garruk, = self.ids("Garruk Relentless")
        bears, = self.ids("Grizzly Bears")
        self.activate(garruk, [[{"object": bears}]], ability=1)
        self.pass_round()
        self.assertEqual(self.dealt(), sorted([(garruk, ("object", bears), 3), (bears, ("object", garruk), 2)]))
        self.assertEqual(self.seen(garruk, "counters"), [{"loyalty": 1}] * 2)
        for viewer in (0, 1):
            self.assertNotIn(bears, self.view(viewer)["battlefield"])

    def test_chandras_ignition_target_deals_its_power_except_to_itself(self):
        # a singular target dealer with a member amount; its recipients aren't a pronoun
        self.new([{"hand": ["Chandra's Ignition"], "battlefield": ["Hill Giant", "Grizzly Bears"] + ["Mountain"] * 5},
                  {"battlefield": ["Craw Wurm"]}])
        giant, = self.ids("Hill Giant")
        bears, = self.ids("Grizzly Bears")
        wurm, = self.ids("Craw Wurm")
        self.cast("Chandra's Ignition", [[{"object": giant}]], mana=self.ids('Mountain', zone="battlefield", controller=self.engine.state["asked"]["player"])[0:5], spend={'R': 5})
        self.pass_round()
        self.assertEqual(self.dealt(), sorted([(giant, ("object", bears), 3), (giant, ("object", wurm), 3), (giant, ("player", 1), 3)]))
        self.assertEqual(self.life(1), [17, 17])
        self.assertEqual(self.seen(giant, "damage"), [0, 0])
        self.assertEqual(self.seen(wurm, "damage"), [3, 3])

    def test_subject_it_after_any_target_stays_the_spell(self):
        # Burst Lightning / Roil Eruption: "… deals N damage to any target. If this spell was
        # kicked, it deals M damage instead": unkicked, the spell deals N to the chosen player
        for card, lands, amount, spend in [("Burst Lightning", 1, 2, {"R": 1}), ("Roil Eruption", 2, 3, {"R": 2})]:
            with self.subTest(card=card):
                self.new([{"hand": [card], "battlefield": ["Mountain"] * lands}, {"battlefield": ["Craw Wurm"]}])
                spell, = self.ids(card)
                self.cast(card, [[{"player": 1}]], mana=self.ids("Mountain", zone="battlefield", controller=0)[:lands], spend=spend)
                source = self.engine.state["stack"][-1]
                self.pass_round()
                self.assertEqual(self.dealt(), [(source, ("player", 1), amount)])
                self.assertEqual(self.life(1), [20 - amount] * 2)
                self.assertEqual(self.seen(self.ids("Craw Wurm")[0], "damage"), [0, 0])


del Helpers
