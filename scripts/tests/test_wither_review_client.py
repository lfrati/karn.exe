"""Wither through the real audited
client with full pinned cards. Damage from a source with wither to a creature puts that many
-1/-1 counters on it instead of marking damage (702.80a, 120.3d); to a player or planeswalker
it is ordinary damage (120.3a, c). The counters stay after the turn, unlike damage (514.2); a
creature that dies with them has them as last known information (704.8), which persist and
undying read; +1/+1 and -1/-1 counters on a survivor cancel (704.5q)."""
import unittest
# the module, not its class, so that its own tests are not collected here again
import test_proliferate_review_client as base
from client_support import ClientTestCase

HELPERS = {k: v for k, v in vars(base.ProliferateReviewClientTests).items() if not k.startswith("test_") and not k.startswith("__")}


class WitherReviewClientTests(ClientTestCase):
    locals().update(HELPERS)

    def obj(self, o):
        return self.engine.state["objects"][str(o)]

    def mine(self, name, player):
        return [o for o in self.ids(name) if o in self.engine.state["battlefield"] and self.obj(o)["controller"] == player]

    def step(self):
        asked = self.asked()
        kind = asked["kind"]
        if kind == "attackers":
            self.engine.act({"kind": "attack", "attacks": []})
        elif kind == "blockers":
            self.engine.act({"kind": "block", "blocks": []})
        elif kind == "discard":
            self.engine.act({"kind": "discard", "cards": asked.get("among", asked.get("cards", []))[:asked.get("count", 0)]})
        elif kind == "holders":
            self.answer([])
        elif kind == "may":
            self.engine.act({"kind": "choose", "answer": "yes"})
        elif kind == "order":
            self.engine.act({"kind": "order", "order": list(range(len(asked["abilities"])))})
        else:
            self.engine.act({"kind": "pass"})

    def advance(self, predicate):
        for _ in range(600):
            if predicate(self.engine.state):
                return
            self.step()
        self.fail("did not reach checkpoint")

    def attack(self, attacker, at):
        self.advance(lambda s: s["asked"]["kind"] == "attackers" and s["active"] == 0)
        self.engine.act({"kind": "attack", "attacks": [[attacker, at]]})

    def block(self, blocks):
        self.advance(lambda s: s["asked"]["kind"] == "blockers")
        self.engine.act({"kind": "block", "blocks": blocks})

    def activate(self, source, targets, *, ability, mana=(), spend=None, **choices):
        self.engine.act({"kind": "activate", "source": source, "ability": ability, "targets": targets,
                         "plan": [{"source": o, "ability": 0} for o in mana],
                         **({"mana": spend} if spend is not None else {}), **choices})

    def pt(self, o):
        return (self.obj(o)["power"], self.obj(o)["toughness"])

    def after_combat(self):
        self.advance(lambda s: s["part"] == "PostcombatMain" and s["asked"]["kind"] == "priority" and not s["stack"])

    def ram_gang_blocked_by(self, blocker, extra=()):
        self.new([{"battlefield": ["Boggart Ram-Gang"]}, {"battlefield": [blocker, *extra]}])
        ram = self.one("Boggart Ram-Gang")
        guard = self.one(blocker)
        self.attack(ram, {"player": 1})
        self.block([[guard, ram]])
        return ram, guard

    def test_a_surviving_blocker_keeps_minus_counters_past_the_turn(self):
        ram, wurm = self.ram_gang_blocked_by("Craw Wurm")
        self.after_combat()
        self.assertEqual((self.counters({"object": wurm}), self.obj(wurm).get("damage", 0)), ({"-1/-1": 3}, 0))
        self.assertEqual((self.obj(wurm)["power"], self.obj(wurm)["toughness"]), (3, 1))
        self.assertNotIn(ram, self.engine.state["battlefield"], "the Wurm's 6 damage is ordinary and marked")
        self.advance(lambda s: s["turn"] == 2 and s["asked"]["kind"] == "priority")
        self.assertEqual((self.obj(wurm)["power"], self.obj(wurm)["toughness"]), (3, 1), "counters are not removed in cleanup")

    def test_kitchen_finks_killed_by_wither_does_not_persist(self):
        ram, finks = self.ram_gang_blocked_by("Kitchen Finks")
        self.after_combat()
        self.assertNotIn(finks, self.engine.state["battlefield"])
        self.assertEqual(self.mine("Kitchen Finks", 1), [], "it had -1/-1 counters, so persist does not return it")
        self.assertNotIn(ram, self.engine.state["battlefield"], "Finks' 3 ordinary damage kills the 3/3")

    def test_young_wolf_killed_by_wither_undies_because_it_had_no_plus_counter(self):
        _, wolf = self.ram_gang_blocked_by("Young Wolf")
        self.after_combat()
        self.assertNotIn(wolf, self.engine.state["battlefield"])
        back = self.mine("Young Wolf", 1)
        self.assertEqual(len(back), 1)
        self.assertEqual(self.counters({"object": back[0]}), {"+1/+1": 1})

    def test_unblocked_wither_damage_to_a_player_is_life_loss(self):
        self.new([{"battlefield": ["Boggart Ram-Gang"]}, {}])
        self.attack(self.one("Boggart Ram-Gang"), {"player": 1})
        self.after_combat()
        self.assertEqual((self.engine.state["players"][1]["life"], self.counters({"player": 1})), (17, {}))

    def test_wither_damage_to_a_planeswalker_removes_loyalty_only(self):
        self.new([{"battlefield": ["Boggart Ram-Gang"]}, {"battlefield": ["Liliana Vess"]}])
        liliana = {"object": self.one("Liliana Vess")}
        self.assertEqual(self.counters(liliana), {"loyalty": 5})
        self.attack(self.one("Boggart Ram-Gang"), liliana)
        self.after_combat()
        self.assertEqual(self.counters(liliana), {"loyalty": 2}, "3 loyalty removed and no -1/-1 counters (120.3c)")
    def test_first_strike_wither_shrinks_the_blocker_before_it_deals_damage(self):
        self.new([{"battlefield": ["Rustrazor Butcher"]}, {"battlefield": ["Grizzly Bears"]}])
        butcher, bears = self.one("Rustrazor Butcher"), self.one("Grizzly Bears")
        self.advance(lambda s: s["turn"] == 3 and s["asked"]["kind"] == "attackers")
        self.engine.act({"kind": "attack", "attacks": [[butcher, {"player": 1}]]})
        self.block([[bears, butcher]])
        self.after_combat()
        self.assertEqual((self.pt(bears), self.counters({"object": bears})), ((1, 1), {"-1/-1": 1}))
        self.assertIn(butcher, self.engine.state["battlefield"], "the 1/1 Bears dealt 1, not 2 (510.4)")
        self.assertEqual(self.obj(butcher).get("damage"), 1)

    def test_necroskitter_returns_a_creature_that_died_with_wither_counters(self):
        self.new([{"battlefield": ["Boggart Ram-Gang", "Necroskitter"]}, {"battlefield": ["Kitchen Finks"]}])
        ram, finks = self.one("Boggart Ram-Gang"), self.one("Kitchen Finks")
        self.attack(ram, {"player": 1})
        self.block([[finks, ram]])
        self.after_combat()
        self.assertEqual(self.mine("Kitchen Finks", 1), [], "no persist: it had -1/-1 counters")
        stolen = self.mine("Kitchen Finks", 0)
        self.assertEqual(len(stolen), 1, "Necroskitter read the counters it died with")
        self.assertEqual(self.counters({"object": stolen[0]}), {})

    def test_village_pillagers_enter_trigger_is_wither_damage_and_counters_are_read_when_dying(self):
        self.new([{"hand": ["Village Pillagers"], "battlefield": ["Mountain"] * 5}, {"battlefield": ["Grizzly Bears", "Llanowar Elves"]}])
        bears, elves = self.one("Grizzly Bears"), self.one("Llanowar Elves")
        self.cast("Village Pillagers", mana=self.ids("Mountain")[:5], spend={'R': 5})
        self.settle()
        self.assertEqual((self.pt(bears), self.counters({"object": bears}), self.obj(bears).get("damage", 0)), ((1, 1), {"-1/-1": 1}, 0))
        self.assertNotIn(elves, self.engine.state["battlefield"])
        treasures = self.mine("Treasure Token", 0)
        self.assertEqual(len(treasures), 1, "the Elves died with a counter on it")
        self.assertTrue(self.obj(treasures[0]).get("tapped"))

    def test_granted_wither_from_blight_sickle(self):
        self.new([{"battlefield": ["Grizzly Bears", "Blight Sickle", "Swamp", "Swamp"]}, {"battlefield": ["Craw Wurm"]}])
        bears, wurm = self.one("Grizzly Bears"), self.one("Craw Wurm")
        self.advance(lambda s: s["turn"] == 3 and s["part"] == "PrecombatMain" and s["asked"]["kind"] == "priority")
        self.activate(self.one("Blight Sickle"), [[{"object": bears}]], ability=2, mana=self.ids("Swamp")[:2], spend={'B': 2})
        self.settle()
        self.assertEqual(self.pt(bears), (3, 2))
        self.attack(bears, {"player": 1})
        self.block([[wurm, bears]])
        self.after_combat()
        self.assertEqual((self.pt(wurm), self.counters({"object": wurm}), self.obj(wurm).get("damage", 0)), ((3, 1), {"-1/-1": 3}, 0))

    def test_a_fight_spell_with_a_wither_creature(self):
        self.new([{"hand": ["Prey Upon"], "battlefield": ["Boggart Ram-Gang", "Forest"]}, {"battlefield": ["Craw Wurm"]}])
        ram, wurm = self.one("Boggart Ram-Gang"), self.one("Craw Wurm")
        self.cast("Prey Upon", [[{"object": ram}], [{"object": wurm}]], mana=[self.one("Forest")], spend={'G': 1})
        self.settle()
        self.assertEqual((self.pt(wurm), self.counters({"object": wurm})), ((3, 1), {"-1/-1": 3}))
        self.assertNotIn(ram, self.engine.state["battlefield"], "the Wurm's damage is marked")

    def test_protection_prevents_wither_damage_so_no_counters(self):
        ram, paladin = self.ram_gang_blocked_by("Paladin en-Vec")
        self.after_combat()
        self.assertEqual((self.counters({"object": paladin}), self.pt(paladin)), ({}, (2, 2)), "protection from red (702.16e)")
        self.assertEqual(self.obj(ram).get("damage"), 2, "first strike from the Paladin is marked")

    def test_a_deathtouch_lifelink_blocker_is_shrunk_while_its_own_damage_still_kills_and_gains(self):
        ram, hawk = self.ram_gang_blocked_by("Vampire Nighthawk")
        self.after_combat()
        self.assertNotIn(hawk, self.engine.state["battlefield"], "3 -1/-1 counters on a 2/3")
        self.assertNotIn(ram, self.engine.state["battlefield"], "deathtouch")
        self.assertEqual(self.engine.state["players"][1]["life"], 22, "lifelink from the frozen damage")


if __name__ == "__main__":
    unittest.main()
