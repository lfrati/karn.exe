"""A root "other" without a comparison or controller
from client_support import ClientTestCase
reference excludes the earlier target (608.2c), while "you control another …" stays relative to
the source. Full pinned cards through the real audited JSON client, both viewers."""


class LexicalTargetOtherReviewClientTests(ClientTestCase):
    def seen(self, obj):
        return [(self.view(v)["objects"][str(obj)]["power"], self.view(v)["objects"][str(obj)]["toughness"]) for v in (0, 1)]

    def pump(self, spell, lands, spend):
        self.new([{"hand": [spell], "battlefield": ["Grizzly Bears", "Hill Giant"] + lands},
                  {"battlefield": ["Craw Wurm"]}])
        bears, = self.ids("Grizzly Bears")
        giant, = self.ids("Hill Giant")
        wurm, = self.ids("Craw Wurm")
        self.cast(spell, [[{"object": bears}]], mana=[o for name in dict.fromkeys(lands) for o in self.ids(name, zone="battlefield", controller=0)[:lands.count(name)]], spend=spend)
        self.pass_round()
        return bears, giant, wurm

    def test_phalanx_tactics_target_gets_only_its_own_bonus(self):
        bears, giant, wurm = self.pump("Phalanx Tactics", ["Plains"] * 2, {"W": 2})
        self.assertEqual(self.seen(bears), [(4, 3)] * 2)
        self.assertEqual(self.seen(giant), [(4, 4)] * 2)
        self.assertEqual(self.seen(wurm), [(6, 4)] * 2)

    def test_exhilarating_elocution_counters_and_others_anthem(self):
        bears, giant, wurm = self.pump("Exhilarating Elocution", ["Plains", "Plains", "Swamp", "Swamp"], {"W": 2, "B": 2})
        self.assertEqual(self.object(bears)["counters"].get("+1/+1"), 2)
        self.assertEqual(self.seen(bears), [(4, 4)] * 2)
        self.assertEqual(self.seen(giant), [(4, 4)] * 2)
        self.assertEqual(self.seen(wurm), [(6, 4)] * 2)

    def test_sigil_blessing_target_three_others_one(self):
        bears, giant, wurm = self.pump("Sigil Blessing", ["Plains", "Forest"], {"W": 1, "G": 1})
        self.assertEqual(self.seen(bears), [(5, 5)] * 2)
        self.assertEqual(self.seen(giant), [(4, 4)] * 2)
        self.assertEqual(self.seen(wurm), [(6, 4)] * 2)

    def enters(self, card, lands, mine, theirs, spend):
        self.new([{"hand": [card], "battlefield": lands + mine}, {"battlefield": ["Hill Giant"] + theirs}])
        target, = self.ids("Hill Giant")
        self.cast(card, mana=[o for name in dict.fromkeys(lands) for o in self.ids(name, zone="battlefield", controller=0)[:lands.count(name)]], spend=spend)
        self.pass_round()
        asked = self.engine.state["asked"]
        self.assertEqual((asked["kind"], asked["player"]), ("targets", 0))
        self.engine.act({"kind": "targets", "targets": [[{"object": target}]]})
        self.pass_round()
        return target

    def test_skulking_killer_counts_that_opponents_other_creatures(self):
        # The condition is about the target's controller, not you or the Killer.
        for mine, theirs, shrunk in [([], [], True), ([], ["Craw Wurm"], False),
                                     (["Grizzly Bears"], [], True), (["Grizzly Bears"], ["Craw Wurm"], False)]:
            with self.subTest(mine=mine, theirs=theirs):
                target = self.enters("Skulking Killer", ["Swamp"] * 4, mine, theirs, {"B": 4})
                self.assertEqual(self.seen(target), [(1, 1) if shrunk else (3, 3)] * 2)

    def test_protocol_knight_another_knight_is_other_than_itself(self):
        for mine, stunned in [([], 0), (["Youthful Knight"], 1)]:
            with self.subTest(mine=mine):
                target = self.enters("Protocol Knight", ["Island"] * 4, mine, [], {"U": 4})
                for viewer in (0, 1):
                    seen = self.view(viewer)["objects"][str(target)]
                    self.assertTrue(seen["tapped"])
                    self.assertEqual(seen["counters"].get("stun", 0), stunned)

    def test_intimidation_bolt_surviving_target_can_still_attack(self):
        self.new([{"hand": ["Intimidation Bolt"], "battlefield": ["Craw Wurm", "Grizzly Bears", "Mountain", "Plains", "Plains"]},
                  {"battlefield": ["Hill Giant"]}])
        wurm, = self.ids("Craw Wurm")
        bears, = self.ids("Grizzly Bears")
        giant, = self.ids("Hill Giant")
        self.cast("Intimidation Bolt", [[{"object": wurm}]], mana=self.ids('Mountain', zone="battlefield", controller=0)[:1] + self.ids('Plains', zone="battlefield", controller=0)[:2], spend={'R': 1, 'W': 2})
        self.pass_round()
        self.assertEqual(self.object(wurm)["damage"], 3)
        self.advance(lambda s: s["asked"]["kind"] == "attackers")
        self.reject(0, {"kind": "attack", "attacks": [[bears, {"player": 1}]]})
        self.reject(0, {"kind": "attack", "attacks": [[wurm, {"player": 1}], [bears, {"player": 1}]]})
        self.engine.act({"kind": "attack", "attacks": [[wurm, {"player": 1}]]})
        self.assertTrue(self.object(wurm)["tapped"])
        self.assertFalse(self.object(bears)["tapped"])
        self.assertIn(giant, self.view(1)["battlefield"])
