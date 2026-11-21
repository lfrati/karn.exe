"""Infect through the real audited
client with full pinned cards. Infect damage to a player gives that many poison counters
instead of life loss (702.90b, 120.3b); to a creature it is -1/-1 counters instead of marked
damage (702.90c, 120.3d); to a planeswalker it removes loyalty as usual (120.3c). It is still
damage: lifelink gains and protection prevents it (120.3f, 702.16e). Ten poison counters lose
the game (704.5c). The source decides: a spell dealing damage equal to a sacrificed infect
creature's power is not itself a source with infect."""
import unittest
# the module, not its class, so that its own tests are not collected here again
import test_wither_review_client as base

HELPERS = {k: v for k, v in vars(base.WitherReviewClientTests).items() if not k.startswith("test_") and not k.startswith("__")}


class InfectReviewClientTests(unittest.TestCase):
    locals().update(HELPERS)

    def poison(self, player, viewer=None):
        state = self.engine.state if viewer is None else self.view(viewer)
        return state["players"][player].get("counters", {}).get("poison", 0)

    def life(self, player):
        return self.engine.state["players"][player]["life"]

    def attack_all(self, attackers, at=None):
        self.advance(lambda s: s["asked"]["kind"] == "attackers" and s["active"] == 0)
        self.engine.act({"kind": "attack", "attacks": [[a, at or {"player": 1}] for a in attackers]})

    def test_infect_damage_to_a_player_is_poison_and_no_life_loss_in_both_views(self):
        self.new([{"battlefield": ["Glistener Elf"]}, {}])
        self.attack(self.one("Glistener Elf"), {"player": 1})
        self.after_combat()
        self.assertEqual((self.life(1), self.poison(1)), (20, 1))
        self.assertEqual((self.poison(1, viewer=0), self.poison(1, viewer=1)), (1, 1), "poison counters are public")
        events = [e for e in self.engine.state["events"] if e.get("event") in ("life_lost", "counters", "damage")]
        self.assertFalse([e for e in events if e["event"] == "life_lost"], events)

    def test_flensermite_lifelink_still_gains_with_poison(self):
        self.new([{"battlefield": ["Flensermite"]}, {}])
        self.attack(self.one("Flensermite"), {"player": 1})
        self.after_combat()
        self.assertEqual((self.life(0), self.life(1), self.poison(1)), (21, 20, 1))

    def test_flensermite_blocked_puts_a_counter_instead_of_marking_and_gains(self):
        self.new([{"battlefield": ["Flensermite"]}, {"battlefield": ["Grizzly Bears"]}])
        mite, bears = self.one("Flensermite"), self.one("Grizzly Bears")
        self.attack(mite, {"player": 1})
        self.block([[bears, mite]])
        self.after_combat()
        self.assertEqual((self.pt(bears), self.counters({"object": bears}), self.obj(bears).get("damage", 0)), ((1, 1), {"-1/-1": 1}, 0))
        self.assertNotIn(mite, self.engine.state["battlefield"], "the Bears' 2 ordinary damage is marked")
        self.assertEqual(self.life(0), 21)

    def test_infect_and_ordinary_attackers_together(self):
        self.new([{"battlefield": ["Glistener Elf", "Grizzly Bears"]}, {}])
        self.attack_all([self.one("Glistener Elf"), self.one("Grizzly Bears")])
        self.after_combat()
        self.assertEqual((self.life(1), self.poison(1)), (18, 1))

    def test_ten_infect_attackers_are_one_placement_and_poison_loses_the_game(self):
        self.new([{"battlefield": ["Glistener Elf"] * 10}, {}])
        elves = self.ids("Glistener Elf")
        self.attack_all(elves)
        for _ in range(40):
            if self.engine.state.get("finished") or self.engine.state["players"][1].get("lost"):
                break
            self.step()
        placed = [e for e in self.engine.state["events"] if e.get("event") == "counters" and e.get("on") == {"player": 1}]
        self.assertEqual([(e["counter"], e["count"]) for e in placed], [("poison", 10)], "one controller's simultaneous poison is one event")
        self.assertEqual(self.life(1), 20)
        self.assertTrue(self.engine.state.get("finished") or self.engine.state["players"][1].get("lost"), "704.5c")

    def test_infect_damage_to_a_planeswalker_removes_loyalty_only(self):
        self.new([{"battlefield": ["Glistener Elf"]}, {"battlefield": ["Liliana Vess"]}])
        liliana = {"object": self.one("Liliana Vess")}
        self.attack(self.one("Glistener Elf"), liliana)
        self.after_combat()
        self.assertEqual((self.counters(liliana), self.poison(1), self.life(1)), ({"loyalty": 4}, 0, 20))

    def test_tainted_strike_grants_infect_until_end_of_turn(self):
        self.new([{"hand": ["Tainted Strike"], "battlefield": ["Grizzly Bears", "Swamp"]}, {}])
        bears = self.one("Grizzly Bears")
        self.attack(bears, {"player": 1})
        self.advance(lambda s: s["part"] == "DeclareBlockers" and s["asked"]["kind"] == "priority" and s["asked"].get("player", 0) == 0)
        self.cast("Tainted Strike", [[{"object": bears}]], mana=[self.one("Swamp")], spend={'B': 1})
        self.settle()
        self.after_combat()
        self.assertEqual((self.life(1), self.poison(1)), (20, 3), "3 power with granted infect")
        self.advance(lambda s: s["turn"] == 5 and s["asked"]["kind"] == "attackers")
        self.engine.act({"kind": "attack", "attacks": [[bears, {"player": 1}]]})
        self.after_combat()
        self.assertEqual((self.life(1), self.poison(1)), (18, 3), "the grant ended: ordinary damage")

    def test_protection_from_the_source_prevents_infect_counters(self):
        self.new([{"battlefield": ["Hand of the Praetors"]}, {"battlefield": ["Paladin en-Vec"]}])
        hand, paladin = self.one("Hand of the Praetors"), self.one("Paladin en-Vec")
        self.attack(hand, {"player": 1})
        self.block([[paladin, hand]])
        self.after_combat()
        self.assertEqual(self.counters({"object": paladin}), {}, "protection from black prevents the damage (702.16e)")
        self.assertNotIn(hand, self.engine.state["battlefield"], "the Paladin's first-strike 2 is ordinary")

    def test_fallen_ferromancer_ability_damage_is_infect_to_a_player_and_a_creature(self):
        self.new([{"battlefield": ["Fallen Ferromancer", "Mountain", "Mountain", "Mountain", "Mountain"]}, {"battlefield": ["Grizzly Bears"]}])
        ferro, bears = self.one("Fallen Ferromancer"), self.one("Grizzly Bears")
        self.activate(ferro, [[{"player": 1}]], ability=1, mana=self.ids("Mountain")[:2], spend={'R': 2})
        self.settle()
        self.assertEqual((self.life(1), self.poison(1)), (20, 1), "infect applies to any damage, not just combat")
        self.advance(lambda s: s["turn"] == 3 and s["part"] == "PrecombatMain" and s["asked"]["kind"] == "priority")
        self.activate(ferro, [[{"object": bears}]], ability=1, mana=self.ids("Mountain")[:2], spend={'R': 2})
        self.settle()
        self.assertEqual((self.pt(bears), self.counters({"object": bears}), self.obj(bears).get("damage", 0)), ((1, 1), {"-1/-1": 1}, 0))

    def test_inkmoth_nexus_animated_land_deals_poison(self):
        self.new([{"battlefield": ["Inkmoth Nexus", "Forest"]}, {}])
        nexus = self.one("Inkmoth Nexus")
        self.advance(lambda s: s["turn"] == 3 and s["part"] == "PrecombatMain" and s["asked"]["kind"] == "priority")
        # paid by the Forest, so that the land itself is untapped to attack
        self.engine.act({"kind": "activate", "source": nexus, "ability": 1, "targets": [],
                         "plan": [{"ability": 0, "adding": [], "paying": [], "source": self.one("Forest")}], "mana": {"G": 1}, "paying": []})
        self.settle()
        self.assertEqual((self.pt(nexus), self.obj(nexus).get("tapped")), ((1, 1), False))
        self.attack(nexus, {"player": 1})
        self.after_combat()
        self.assertEqual((self.life(1), self.poison(1)), (20, 1))

    def test_triumph_of_the_hordes_grants_infect_to_each_creature_as_one_placement(self):
        self.new([{"hand": ["Triumph of the Hordes"], "battlefield": ["Grizzly Bears", "Young Wolf", "Forest", "Forest", "Forest", "Forest"]}, {}])
        bears, elves = self.one("Grizzly Bears"), self.one("Young Wolf")
        self.advance(lambda s: s["turn"] == 3 and s["part"] == "PrecombatMain" and s["asked"]["kind"] == "priority")
        self.cast("Triumph of the Hordes", mana=self.ids("Forest", zone="battlefield")[:4], spend={'G': 4})
        self.settle()
        self.attack_all([bears, elves])
        self.after_combat()
        self.assertEqual((self.life(1), self.poison(1)), (20, 5), "3 + 2 with granted infect")

    def test_fling_is_the_source_so_a_flung_infect_creature_deals_ordinary_damage(self):
        self.new([{"hand": ["Fling"], "battlefield": ["Hand of the Praetors", "Mountain", "Mountain"]}, {}])
        self.cast("Fling", [[{"player": 1}]], mana=self.ids("Mountain")[:2],
                  paying=[[self.one("Hand of the Praetors")]], spend={'R': 2})
        self.settle()
        self.assertEqual((self.life(1), self.poison(1)), (17, 0), "Fling, not the sacrificed creature, deals the damage (no infect)")

    def test_blight_mamba_regenerates_from_marked_damage_but_not_from_wither_counters(self):
        self.new([{"battlefield": ["Craw Wurm"]}, {"battlefield": ["Blight Mamba", "Forest", "Forest"]}])
        wurm, mamba = self.one("Craw Wurm"), self.one("Blight Mamba")
        self.attack(wurm, {"player": 1})
        self.block([[mamba, wurm]])
        self.advance(lambda s: s["part"] == "DeclareBlockers" and s["asked"]["kind"] == "priority" and s["asked"]["player"] == 1)
        self.activate(mamba, [], ability=1, mana=self.ids("Forest", zone="battlefield", controller=1)[:2], spend={'G': 2})
        self.settle()
        self.after_combat()
        self.assertIn(mamba, self.engine.state["battlefield"], "regenerated from 6 marked damage (701.19)")
        self.assertEqual(self.counters({"object": wurm}), {"-1/-1": 1})

    def test_blight_mamba_with_toughness_zero_from_counters_is_not_saved_by_regeneration(self):
        self.new([{"battlefield": ["Boggart Ram-Gang"]}, {"battlefield": ["Blight Mamba", "Forest", "Forest"]}])
        ram, mamba = self.one("Boggart Ram-Gang"), self.one("Blight Mamba")
        self.attack(ram, {"player": 1})
        self.block([[mamba, ram]])
        self.advance(lambda s: s["part"] == "DeclareBlockers" and s["asked"]["kind"] == "priority" and s["asked"]["player"] == 1)
        self.activate(mamba, [], ability=1, mana=self.ids("Forest", zone="battlefield", controller=1)[:2], spend={'G': 2})
        self.settle()
        self.after_combat()
        self.assertEqual(self.mine("Blight Mamba", 1), [], "toughness 0 is not destruction (704.5f)")
        self.assertEqual(self.counters({"object": ram}), {"-1/-1": 1})

    def test_unpreventable_damage_overrides_protection_and_lifelink_cannot_gain(self):
        for leyline in [False, True]:
            self.new([{"battlefield": ["Razor Swine", "Flensermite"] + (["Leyline of Punishment"] if leyline else [])}, {"battlefield": ["Kor Firewalker"]}])
            goblin, mite, walker = self.one("Razor Swine"), self.one("Flensermite"), self.one("Kor Firewalker")
            self.attack_all([goblin, mite])
            self.block([[walker, goblin]])
            self.after_combat()
            if leyline:
                self.assertNotIn(walker, self.engine.state["battlefield"], "2 -1/-1 counters: damage can't be prevented")
            else:
                self.assertEqual(self.counters({"object": walker}), {}, "protection from red prevents the infect damage")
            self.assertEqual((self.poison(1), self.life(0)), (1, 20 if leyline else 21), "players can't gain life")

    def ask_kinds(self, turn, player):
        kinds = set()
        while self.engine.state["turn"] <= turn:
            asked = self.asked()
            if self.engine.state["turn"] == turn and asked["player"] == player:
                kinds.add(asked["kind"])
            self.step()
        return kinds

    def test_kulrath_knight_reads_infect_counters_on_an_opponents_creature(self):
        for knight in [False, True]:
            self.new([{"battlefield": ["Fallen Ferromancer", "Mountain", "Mountain"] + (["Kulrath Knight"] if knight else [])}, {"battlefield": ["Grizzly Bears"]}])
            bears = self.one("Grizzly Bears")
            self.activate(self.one("Fallen Ferromancer"), [[{"object": bears}]], ability=1, mana=self.ids("Mountain")[:2], spend={'R': 2})
            self.settle()
            self.assertEqual(self.counters({"object": bears}), {"-1/-1": 1})
            kinds = self.ask_kinds(2, 1)
            self.assertEqual("attackers" in kinds, not knight, kinds)


if __name__ == "__main__":
    unittest.main()
