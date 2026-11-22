"""Damage prevention through the
real audited client, with full pinned Mending Hands, Healing Salve, Fog, Holy Day, Kami of False
Hope and Sunspine Lynx. A "next N" shield prevents damage to the shielded player or permanent;
when two or more sources would deal damage to it at once, the player or controller chooses
which damage the shield prevents (615.7). The affected player orders prevention and
replacement effects (616.1). Damage that can't be prevented doesn't reduce shields (615.12),
and protection can't prevent it (Sunspine Lynx ruling 2972b803). Effects that last "this turn"
end at cleanup (514.2), and a permanent that left and returned is a new object (400.7)."""
import unittest
import test_toxic_review_client as base
from client_support import ClientTestCase

HELPERS = {k: v for k, v in vars(base.ToxicReviewClientTests).items() if not k.startswith("test_") and not k.startswith("__")}


class PreventionReviewClientTests(ClientTestCase):
    locals().update(HELPERS)

    def new(self, seats):
        """Unwrap an earlier recording first, so a loop of games records each event once."""
        if hasattr(self, "_ask"):
            self.engine.ask = self._ask
        else:
            self._ask = self.engine.ask
        HELPERS["new"](self, seats)

    def cast_spell(self, name, targets=(), mode=None, *, mana=(), spend=None):
        self.cast(name, [list(targets)] if targets else [], mana=mana, spend=spend,
                  **({"modes": [mode]} if mode is not None else {}))
        self.settle()

    def p1_main_spell(self, name, targets=(), mode=None, *, mana=(), spend=None):
        """P0 passes in its first main phase; P1 casts at instant speed, and it resolves."""
        self.main()
        self.engine.act({"kind": "pass"})
        self.assertEqual(self.asked()["player"], 1)
        self.cast_spell(name, targets, mode, mana=mana, spend=spend)
        self.assertEqual((self.engine.state["part"], self.asked()["player"], self.engine.state["stack"]), ("PrecombatMain", 0, []))

    def damage_taken(self, holder):
        return [(e["source"], e["amount"]) for e in self.seen if e.get("event") == "damage" and e.get("to") == holder]

    def until_question(self):
        self.advance(lambda s: s["asked"]["kind"] in ("prevention", "replacement")
                     or (s["part"] == "PostcombatMain" and s["asked"]["kind"] == "priority" and not s["stack"]))
        return self.asked()

    def order(self, kind, name=None, hit=None):
        asked = self.asked()
        self.assertEqual((asked["kind"], asked.get("event")), ("replacement", "damage_prevention"), asked)
        offer = next(o for o in asked["among"] if o["kind"] == kind and (name is None or o.get("name") == name)
                     and (hit is None or o.get("hit") == hit))
        self.engine.act({"kind": "choose", "answer": {"damage_order": {k: v for k, v in offer.items() if k not in ("name", "remaining", "damage_source")}}})

    def allocate(self, pairs):
        self.engine.act({"kind": "choose", "answer": {"prevention": [list(p) for p in pairs]}})

    def test_mending_hands_shields_a_player_and_the_rest_expires_at_cleanup(self):
        self.new([{"hand": ["Shock", "Shock"], "battlefield": ["Hill Giant", "Mountain", "Mountain"]},
                  {"hand": ["Mending Hands"], "battlefield": ["Plains"]}])
        self.p1_main_spell("Mending Hands", [{"player": 1}], mana=[o for o in self.ids('Plains', zone="battlefield", controller=1) if not self.obj(o).get("tapped")][:1], spend={'W': 1})
        self.attack(self.one("Hill Giant"), {"player": 1})
        self.after_combat()
        self.assertEqual(self.life(1), 20, "3 of 4 used")
        self.cast_spell("Shock", [{"player": 1}], mana=[o for o in self.ids('Mountain', zone="battlefield", controller=self.asked()["player"]) if not self.obj(o).get("tapped")][:1], spend={'R': 1})
        self.assertEqual(self.life(1), 19, "the last 1 prevents 1 of Shock's 2")
        # P0 Shocks in P1's turn: the shield ended at cleanup of turn 1
        self.advance(lambda s: s["turn"] == 2 and s["asked"]["kind"] == "priority" and s["asked"]["player"] == 0)
        self.cast_spell("Shock", [{"player": 1}], mana=[o for o in self.ids('Mountain', zone="battlefield", controller=self.asked()["player"]) if not self.obj(o).get("tapped")][:1], spend={'R': 1})
        self.assertEqual(self.life(1), 17)

    def salve_nighthawk_bears(self):
        self.new([{"battlefield": ["Vampire Nighthawk", "Grizzly Bears"]}, {"hand": ["Healing Salve"], "battlefield": ["Plains"]}])
        self.p1_main_spell("Healing Salve", [{"player": 1}], mode=1, mana=[o for o in self.ids('Plains', zone="battlefield", controller=1) if not self.obj(o).get("tapped")][:1], spend={'W': 1})
        hawk, bears = self.one("Vampire Nighthawk"), self.one("Grizzly Bears")
        self.attack_all([hawk, bears])
        asked = self.until_question()
        self.assertEqual((asked["kind"], asked["player"], asked["on"], asked["total"]), ("prevention", 1, {"player": 1}, 3))
        hits = {h["source"]: h["hit"] for h in asked["hits"]}
        self.assertEqual(sorted((h["source"], h["amount"]) for h in asked["hits"]), sorted([(hawk, 2), (bears, 2)]))
        self.assertEqual((self.life(0), self.life(1)), (20, 20), "nothing is dealt while asked")
        return hawk, bears, hits

    def test_salve_allocation_on_the_lifelinker_denies_the_life_gain(self):
        hawk, bears, hits = self.salve_nighthawk_bears()
        self.allocate([(hits[hawk], 2), (hits[bears], 1)])
        self.after_combat()
        self.assertEqual((self.life(1), self.life(0)), (19, 20))

    def test_salve_allocation_on_the_bears_lets_one_lifelink_through(self):
        hawk, bears, hits = self.salve_nighthawk_bears()
        self.allocate([(hits[hawk], 1), (hits[bears], 2)])
        self.after_combat()
        self.assertEqual((self.life(1), self.life(0)), (19, 21))

    def test_invalid_allocations_keep_the_question(self):
        hawk, bears, hits = self.salve_nighthawk_bears()
        h, b = hits[hawk], hits[bears]
        for bad in ([[h, 2], [b, 2]], [[h, 1], [b, 1]], [[h, 2], [h, 1]], [[h, 3], [b, 0]], [[h, 2]], [[h, 2], [b, 1], [99, 0]],
                    [[h, -1], [b, 4]], [[h, 1.5], [b, 1.5]]):
            self.reject(1, {"kind": "choose", "answer": {"prevention": bad}})
        self.reject(0, {"kind": "choose", "answer": {"prevention": [[h, 2], [b, 1]]}})
        self.reject(1, {"kind": "pass"})
        self.allocate([(h, 2), (b, 1)])
        self.after_combat()
        self.assertEqual((self.life(1), self.life(0)), (19, 20))

    def test_salve_with_infect_chooses_poison_or_life(self):
        # Elf 1 (infect) + Hill Giant 3 against shield 3: one damage gets through
        for prevent_elf, expected in ((1, (19, 0)), (0, (20, 1))):
            self.new([{"battlefield": ["Glistener Elf", "Hill Giant"]}, {"hand": ["Healing Salve"], "battlefield": ["Plains"]}])
            self.p1_main_spell("Healing Salve", [{"player": 1}], mode=1, mana=[o for o in self.ids('Plains', zone="battlefield", controller=1) if not self.obj(o).get("tapped")][:1], spend={'W': 1})
            elf, bears = self.one("Glistener Elf"), self.one("Hill Giant")
            self.attack_all([elf, bears])
            asked = self.until_question()
            hits = {h["source"]: h["hit"] for h in asked["hits"]}
            self.assertEqual(asked["total"], 3)
            self.allocate([(hits[elf], prevent_elf), (hits[bears], 3 - prevent_elf)])
            self.after_combat()
            self.assertEqual((self.life(1), self.poison(1)), expected)

    def torbran_ab(self):
        # A: green 1/1, B: red 1/1. Torbran adds 2 only to B. Shield 3 on P1.
        self.new([{"battlefield": ["Llanowar Elves", "Raging Goblin", "Torbran, Thane of Red Fell"]},
                  {"hand": ["Healing Salve"], "battlefield": ["Plains"]}])
        self.p1_main_spell("Healing Salve", [{"player": 1}], mode=1, mana=[o for o in self.ids('Plains', zone="battlefield", controller=1) if not self.obj(o).get("tapped")][:1], spend={'W': 1})
        a, b = self.one("Llanowar Elves"), self.one("Raging Goblin")
        self.attack_all([a, b])
        asked = self.until_question()
        self.assertEqual((asked["kind"], asked.get("event"), asked["player"]), ("replacement", "damage_prevention", 1))
        self.assertEqual(sorted(o["kind"] for o in asked["among"]), ["modifier", "shield"], "Torbran on B competes with the shared shield")
        modifier = next(o for o in asked["among"] if o["kind"] == "modifier")
        self.assertEqual(modifier["damage_source"], b)
        return a, b

    def test_shield_first_prevents_both_before_torbran(self):
        a, b = self.torbran_ab()
        self.order("shield")
        self.after_combat()
        self.assertEqual(self.life(1), 20)
        self.assertEqual(self.damage_taken({"player": 1}), [])

    def test_torbran_first_then_the_shield_is_divided_over_one_and_three(self):
        for prevent_a, dealt in ((1, [("b", 1)]), (0, [("a", 1)])):
            a, b = self.torbran_ab()
            self.order("modifier")
            asked = self.asked()
            self.assertEqual(asked["kind"], "prevention")
            amounts = {h["source"]: h["amount"] for h in asked["hits"]}
            self.assertEqual((amounts[a], amounts[b], asked["total"]), (1, 3, 3))
            hits = {h["source"]: h["hit"] for h in asked["hits"]}
            self.allocate([(hits[a], prevent_a), (hits[b], 3 - prevent_a)])
            self.after_combat()
            self.assertEqual(self.life(1), 19)
            self.assertEqual(self.damage_taken({"player": 1}), [({"a": a, "b": b}[k], n) for k, n in dealt])

    def paladin(self, first):
        self.new([{"hand": ["Rabid Bite"], "battlefield": ["Hill Giant", "Grizzly Bears", "Forest", "Forest"]},
                  {"hand": ["Mending Hands"], "battlefield": ["Paladin en-Vec", "Plains"]}])
        paladin = self.one("Paladin en-Vec")
        self.p1_main_spell("Mending Hands", [{"object": paladin}], mana=[o for o in self.ids('Plains', zone="battlefield", controller=1) if not self.obj(o).get("tapped")][:1], spend={'W': 1})
        giant, bears = self.one("Hill Giant"), self.one("Grizzly Bears")
        self.attack(giant, {"player": 1})
        self.block([[paladin, giant]])
        asked = self.until_question()
        self.assertEqual(sorted(o["kind"] for o in asked["among"]), ["protection", "shield"])
        self.assertEqual(asked["player"], 1)
        self.order(first)
        self.after_combat()
        self.assertEqual(self.obj(paladin).get("damage", 0), 0)
        # Grizzly Bears (green) bites the Paladin for 2: protection from red doesn't stop it
        self.cast("Rabid Bite", [[{"object": bears}], [{"object": paladin}]],
                  mana=self.ids("Forest", zone="battlefield", controller=0)[:2], spend={'G': 2})
        self.settle()
        return paladin

    def test_protection_first_keeps_the_whole_shield(self):
        paladin = self.paladin("protection")
        self.assertEqual(self.obj(paladin).get("damage", 0), 0, "4 left prevents the Bite's 2")

    def test_shield_first_spends_three_of_it(self):
        paladin = self.paladin("shield")
        self.assertEqual(self.obj(paladin).get("damage", 0), 1, "1 left prevents 1 of 2")

    def test_fog_first_keeps_the_shield_shield_first_spends_it(self):
        for first, life in (("prevent_all", 20), ("shield", 19)):
            self.new([{"hand": ["Shock"], "battlefield": ["Hill Giant", "Mountain"]},
                      {"hand": ["Fog", "Mending Hands"], "battlefield": ["Plains", "Forest"]}])
            self.main()
            self.engine.act({"kind": "pass"})
            self.cast_spell("Mending Hands", [{"player": 1}], mana=[o for o in self.ids('Plains', zone="battlefield", controller=self.asked()["player"]) if not self.obj(o).get("tapped")][:1], spend={'W': 1})
            self.assertEqual(self.asked()["player"], 0)
            self.engine.act({"kind": "pass"})
            self.cast_spell("Fog", mana=[o for o in self.ids('Forest', zone="battlefield", controller=self.asked()["player"]) if not self.obj(o).get("tapped")][:1], spend={'G': 1})
            self.attack(self.one("Hill Giant"), {"player": 1})
            self.until_question()
            self.order(first)
            self.after_combat()
            self.assertEqual(self.life(1), 20)
            self.cast_spell("Shock", [{"player": 1}], mana=[o for o in self.ids('Mountain', zone="battlefield", controller=self.asked()["player"]) if not self.obj(o).get("tapped")][:1], spend={'R': 1})
            self.assertEqual(self.life(1), life, "4 or 1 shield left against Shock's 2")

    def test_fog_holy_day_and_kami_prevent_all_combat_damage_but_not_shock(self):
        for name, extra in (("Fog", ["Forest"]), ("Holy Day", ["Plains"]), ("Kami of False Hope", [])):
            hand = [] if name.startswith("Kami") else [name]
            field = (["Kami of False Hope"] if name.startswith("Kami") else []) + extra + ["Grizzly Bears"]
            self.new([{"hand": ["Shock"], "battlefield": ["Craw Wurm", "Mountain"]}, {"hand": hand, "battlefield": field}])
            self.main()
            self.engine.act({"kind": "pass"})
            if hand:
                self.cast_spell(name, mana=self.ids("Forest", zone="battlefield", controller=1)[:1] if name == "Fog" else self.ids("Plains", zone="battlefield", controller=1)[:1], spend={"G": 1} if name == "Fog" else {"W": 1})
            else:
                kami = self.one("Kami of False Hope")
                self.activate(kami, [], ability=0)
                self.settle()
                self.assertNotIn(kami, self.engine.state["battlefield"], "sacrificed as a cost")
            wurm, bears = self.one("Craw Wurm"), self.one("Grizzly Bears")
            self.attack(wurm, {"player": 1})
            self.block([[bears, wurm]])
            self.after_combat()
            self.assertEqual((self.obj(bears).get("damage", 0), self.obj(wurm).get("damage", 0)), (0, 0), name)
            self.cast_spell("Shock", [{"object": bears}], mana=[o for o in self.ids('Mountain', zone="battlefield", controller=self.asked()["player"]) if not self.obj(o).get("tapped")][:1], spend={'R': 1})
            self.assertNotIn(bears, self.engine.state["battlefield"], f"{name}: noncombat damage is dealt")

    def test_sunspine_lynx_makes_damage_unpreventable_and_keeps_shields(self):
        self.new([{"battlefield": ["Hill Giant", "Sunspine Lynx"]},
                  {"hand": ["Mending Hands"], "battlefield": ["Paladin en-Vec", "Plains"]}])
        paladin = self.one("Paladin en-Vec")
        self.p1_main_spell("Mending Hands", [{"object": paladin}], mana=[o for o in self.ids('Plains', zone="battlefield", controller=1) if not self.obj(o).get("tapped")][:1], spend={'W': 1})
        giant = self.one("Hill Giant")
        self.attack(giant, {"player": 1})
        self.block([[paladin, giant]])
        self.after_combat()
        self.assertNotIn(paladin, self.engine.state["battlefield"], "neither protection nor the shield prevents it")
        self.assertEqual([a for a in self.seen if a.get("event") == "damage" and a.get("to") == {"object": paladin}][0]["amount"], 3)

    def test_blinked_creature_loses_its_shield(self):
        self.new([{"hand": ["Shock"], "battlefield": ["Mountain"]},
                  {"hand": ["Mending Hands", "Cloudshift"], "battlefield": ["Grizzly Bears", "Plains", "Plains"]}])
        bears = self.one("Grizzly Bears")
        self.main()
        self.engine.act({"kind": "pass"})
        self.cast_spell("Mending Hands", [{"object": bears}], mana=[o for o in self.ids('Plains', zone="battlefield", controller=self.asked()["player"]) if not self.obj(o).get("tapped")][:1], spend={'W': 1})
        self.engine.act({"kind": "pass"})
        self.cast_spell("Cloudshift", [{"object": bears}], mana=[o for o in self.ids('Plains', zone="battlefield", controller=self.asked()["player"]) if not self.obj(o).get("tapped")][:1], spend={'W': 1})
        new = self.one("Grizzly Bears")
        self.assertNotEqual(new, bears)
        self.cast_spell("Shock", [{"object": new}], mana=[o for o in self.ids('Mountain', zone="battlefield", controller=self.asked()["player"]) if not self.obj(o).get("tapped")][:1], spend={'R': 1})
        self.assertNotIn(new, self.engine.state["battlefield"], "400.7: the new object has no shield")

    def test_shield_alone_on_one_hit_asks_nothing(self):
        self.new([{"battlefield": ["Hill Giant", "Grizzly Bears"]}, {"hand": ["Mending Hands"], "battlefield": ["Plains", "Craw Wurm"]}])
        wurm = self.one("Craw Wurm")
        self.p1_main_spell("Mending Hands", [{"object": wurm}], mana=[o for o in self.ids('Plains', zone="battlefield", controller=1) if not self.obj(o).get("tapped")][:1], spend={'W': 1})
        self.attack_all([self.one("Hill Giant"), self.one("Grizzly Bears")])
        self.block([[wurm, self.one("Hill Giant")]])
        self.after_combat()
        self.assertEqual(self.obj(wurm).get("damage", 0), 0)
        self.assertEqual(self.life(1), 18)

    def test_shield_before_or_after_furnace(self):
        for first, life in (("shield", 20), ("modifier", 18)):
            self.new([{"battlefield": ["Hill Giant", "Furnace of Rath"]}, {"hand": ["Mending Hands"], "battlefield": ["Plains"]}])
            self.p1_main_spell("Mending Hands", [{"player": 1}], mana=[o for o in self.ids('Plains', zone="battlefield", controller=1) if not self.obj(o).get("tapped")][:1], spend={'W': 1})
            self.attack(self.one("Hill Giant"), {"player": 1})
            asked = self.until_question()
            self.assertEqual(sorted(o["kind"] for o in asked["among"]), ["modifier", "shield"])
            shield = next(o for o in asked["among"] if o["kind"] == "shield")
            self.assertEqual((shield["name"], shield["remaining"]), ("Mending Hands", 4))
            self.order(first)
            self.after_combat()
            self.assertEqual(self.life(1), life, "3 prevented then doubled 0, or doubled 6 less 4")

    def test_one_shield_spans_both_double_strike_damage_steps(self):
        self.new([{"hand": ["Shock"], "battlefield": ["Fencing Ace", "Mountain"]}, {"hand": ["Mending Hands"], "battlefield": ["Plains"]}])
        self.p1_main_spell("Mending Hands", [{"player": 1}], mana=[o for o in self.ids('Plains', zone="battlefield", controller=1) if not self.obj(o).get("tapped")][:1], spend={'W': 1})
        self.attack(self.one("Fencing Ace"), {"player": 1})
        self.after_combat()
        self.assertEqual(self.life(1), 20)
        self.assertEqual(self.damage_taken({"player": 1}), [])
        self.cast_spell("Shock", [{"player": 1}], mana=[o for o in self.ids('Mountain', zone="battlefield", controller=self.asked()["player"]) if not self.obj(o).get("tapped")][:1], spend={'R': 1})
        self.assertEqual(self.life(1), 20, "2 of 4 left after two steps of 1")

    def test_invalid_order_answers_keep_the_question(self):
        a, b = self.torbran_ab()
        asked = self.asked()
        shield = next({k: v for k, v in o.items() if k not in ("name", "remaining", "damage_source")} for o in asked["among"] if o["kind"] == "shield")
        modifier = next({k: v for k, v in o.items() if k not in ("name", "remaining", "damage_source")} for o in asked["among"] if o["kind"] == "modifier")
        self.reject(0, {"kind": "choose", "answer": {"damage_order": shield}})
        self.reject(1, {"kind": "choose", "answer": {"damage_order": {**modifier, "hit": modifier["hit"] + 99}}})
        self.reject(1, {"kind": "choose", "answer": {"damage_order": {**shield, "kind": "prevent_all", "hit": 0}}})
        self.reject(1, {"kind": "choose", "answer": {"replacement": {k: modifier[k] for k in ("source", "ability", "part", "hit")}}})
        self.reject(1, {"kind": "choose", "answer": {"prevention": [[0, 1], [1, 1]]}})
        self.reject(1, {"kind": "pass"})
        self.order("shield")
        self.after_combat()
        self.assertEqual(self.life(1), 20)

    def test_two_shields_on_one_hit_are_ordered_and_both_can_be_spent(self):
        self.new([{"hand": ["Shock"], "battlefield": ["Craw Wurm", "Mountain"]},
                  {"hand": ["Mending Hands", "Healing Salve"], "battlefield": ["Plains", "Plains"]}])
        self.main()
        self.engine.act({"kind": "pass"})
        self.cast_spell("Mending Hands", [{"player": 1}], mana=[o for o in self.ids('Plains', zone="battlefield", controller=self.asked()["player"]) if not self.obj(o).get("tapped")][:1], spend={'W': 1})
        self.engine.act({"kind": "pass"})
        self.cast_spell("Healing Salve", [{"player": 1}], mode=1, mana=[o for o in self.ids('Plains', zone="battlefield", controller=self.asked()["player"]) if not self.obj(o).get("tapped")][:1], spend={'W': 1})
        self.attack(self.one("Craw Wurm"), {"player": 1})
        asked = self.until_question()
        self.assertEqual(sorted(o["name"] for o in asked["among"]), ["Healing Salve", "Mending Hands"])
        self.order("shield", "Healing Salve")
        self.after_combat()
        self.assertEqual(self.life(1), 20, "3 then 3 of Mending's 4")
        self.cast_spell("Shock", [{"player": 1}], mana=[o for o in self.ids('Mountain', zone="battlefield", controller=self.asked()["player"]) if not self.obj(o).get("tapped")][:1], spend={'R': 1})
        self.assertEqual(self.life(1), 19, "Mending has 1 left")

    def p1_priority_in_combat(self):
        self.advance(lambda s: s["asked"]["kind"] == "priority" and s["asked"]["player"] == 1
                     and s["part"] not in ("PrecombatMain", "Beginning", "Upkeep", "Draw"))

    def test_maze_of_ith_prevents_damage_dealt_by_an_unblocked_attacker(self):
        # "dealt to and dealt by that creature": either direction (Maze of Ith rulings 370602e0, 731b676e)
        self.new([{"battlefield": ["Hill Giant"]}, {"battlefield": ["Maze of Ith"]}])
        giant = self.one("Hill Giant")
        self.attack(giant, {"player": 1})
        self.p1_priority_in_combat()
        self.activate(self.one("Maze of Ith"), [[{"object": giant}]], ability=0)
        self.settle()
        self.assertFalse(self.obj(giant)["tapped"], "untapped")
        self.after_combat()
        self.assertEqual(self.life(1), 20, "the Giant's combat damage to P1 is prevented")

    def test_maze_of_ith_prevents_both_directions_when_blocked(self):
        self.new([{"battlefield": ["Hill Giant"]}, {"battlefield": ["Maze of Ith", "Craw Wurm"]}])
        giant, wurm = self.one("Hill Giant"), self.one("Craw Wurm")
        self.attack(giant, {"player": 1})
        self.block([[wurm, giant]])
        self.p1_priority_in_combat()
        self.activate(self.one("Maze of Ith"), [[{"object": giant}]], ability=0)
        self.settle()
        self.after_combat()
        self.assertIn(giant, self.engine.state["battlefield"], "the Wurm's 6 to the Giant is prevented")
        self.assertEqual(self.obj(wurm).get("damage", 0), 0, "the Giant's 3 to the Wurm is prevented")

    def test_moonlight_geist_prevents_its_own_damage_to_a_player(self):
        self.new([{"battlefield": ["Moonlight Geist"] + ["Plains"] * 4}, {}])
        geist = self.one("Moonlight Geist")
        self.main()
        self.activate(geist, [], ability=1, mana=self.ids("Plains", zone="battlefield", controller=0)[:4], spend={'W': 4})
        self.settle()
        self.attack(geist, {"player": 1})
        self.after_combat()
        self.assertEqual(self.life(1), 20)

    def test_stonewise_fortifier_needs_both_its_recipient_and_the_target_source(self):
        # control: "dealt to this creature by target creature" is a conjunction
        self.new([{"hand": ["Shock"], "battlefield": ["Hill Giant", "Mountain"]},
                  {"battlefield": ["Stonewise Fortifier"] + ["Plains"] * 5}])
        giant, fortifier = self.one("Hill Giant"), self.one("Stonewise Fortifier")
        self.attack(giant, {"player": 1})
        self.block([[fortifier, giant]])
        self.p1_priority_in_combat()
        self.activate(fortifier, [[{"object": giant}]], ability=0, mana=self.ids("Plains", zone="battlefield", controller=1)[:5], spend={'W': 5})
        self.settle()
        self.after_combat()
        self.assertIn(fortifier, self.engine.state["battlefield"])
        self.assertEqual(self.obj(giant).get("damage", 0), 2, "the Fortifier's own damage is not prevented")
        self.cast_spell("Shock", [{"object": fortifier}], mana=[o for o in self.ids('Mountain', zone="battlefield", controller=self.asked()["player"]) if not self.obj(o).get("tapped")][:1], spend={'R': 1})
        self.assertNotIn(fortifier, self.engine.state["battlefield"], "another source's damage is not prevented")

    def test_vine_snare_checks_power_as_damage_is_dealt(self):
        # ruling 8cba72a2: the power at resolution doesn't matter
        self.new([{"hand": ["Giant Growth"], "battlefield": ["Grizzly Bears", "Hill Giant", "Craw Wurm", "Forest"]},
                  {"hand": ["Vine Snare"], "battlefield": ["Forest"] * 3}])
        self.p1_main_spell("Vine Snare", mana=[o for o in self.ids('Forest', zone="battlefield", controller=1) if not self.obj(o).get("tapped")][:3], spend={'G': 3})
        bears = self.one("Grizzly Bears")
        self.cast_spell("Giant Growth", [{"object": bears}], mana=[o for o in self.ids('Forest', zone="battlefield", controller=self.asked()["player"]) if not self.obj(o).get("tapped")][:1], spend={'G': 1})
        self.attack_all([bears, self.one("Hill Giant"), self.one("Craw Wurm")])
        self.after_combat()
        self.assertEqual(self.life(1), 20 - 5 - 6, "the pumped 5/5 and the 6/6 deal damage; the 3/3 doesn't")

    def test_winds_ferocious_prevents_only_opponents_creatures(self):
        for field, giant_dies in (([], False), (["Craw Wurm"], True)):
            self.new([{"battlefield": ["Hill Giant"]}, {"hand": ["Winds of Qal Sisma"], "battlefield": ["Forest", "Forest", "Grizzly Bears"] + field}])
            self.p1_main_spell("Winds of Qal Sisma", mana=[o for o in self.ids('Forest', zone="battlefield", controller=1) if not self.obj(o).get("tapped")][:2], spend={'G': 2})
            giant = self.one("Hill Giant")
            blocker = self.one(field[0] if field else "Grizzly Bears")
            self.attack(giant, {"player": 1})
            self.block([[blocker, giant]])
            self.after_combat()
            self.assertEqual(self.obj(blocker).get("damage", 0), 0)
            self.assertEqual(giant not in self.engine.state["battlefield"], giant_dies, f"ferocious={bool(field)}")

    def test_haze_frog_prevents_other_creatures_damage_not_its_own(self):
        # ruling 5a546410
        self.new([{"battlefield": ["Hill Giant"]}, {"hand": ["Haze Frog"], "battlefield": ["Forest"] * 5}])
        self.p1_main_spell("Haze Frog", mana=[o for o in self.ids('Forest', zone="battlefield", controller=1) if not self.obj(o).get("tapped")][:5], spend={'G': 5})
        frog, giant = self.one("Haze Frog"), self.one("Hill Giant")
        self.attack(giant, {"player": 1})
        self.block([[frog, giant]])
        self.after_combat()
        self.assertIn(frog, self.engine.state["battlefield"])
        self.assertEqual(self.obj(giant).get("damage", 0), 2)


if __name__ == "__main__":
    unittest.main()
