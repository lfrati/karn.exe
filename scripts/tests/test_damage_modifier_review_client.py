"""Replacement effects that change
how much damage is dealt (614.1a), with full pinned Furnace of Rath, Torbran, Thane of Red Fell
and Embermaw Hellion, through the real audited client. The damage is still dealt by the
original source (rulings 316a6518, 44e2694b, ce034255). The player dealt damage, or the
controller of the permanent dealt damage, orders the applicable effects (616.1, ruling
f6df8005); with only one applicable effect there is nothing to choose. Damage is divided
before it is modified (rulings 5f5824df, 5fbf4f2b). Protection prevents all of it (702.16e),
unless damage can't be prevented."""
import unittest
# the module, not its class, so that its own tests are not collected here again
import test_toxic_review_client as base

HELPERS = {k: v for k, v in vars(base.ToxicReviewClientTests).items() if not k.startswith("test_") and not k.startswith("__")}


class DamageModifierReviewClientTests(unittest.TestCase):
    locals().update(HELPERS)

    def new(self, seats):
        """Also record every damage replacement question the engine asks."""
        HELPERS["new"](self, seats)
        self.questions = []
        ask = self.engine.ask
        def recording(request):
            state = ask(request)
            asked = state["asked"]
            if asked and asked["kind"] == "replacement" and asked.get("event") == "damage" and request.get("cmd") == "act":
                self.questions.append(asked)
            return state
        self.engine.ask = recording

    def choose_damage(self, provider_name, hit=None):
        asked = self.asked()
        self.assertEqual((asked["kind"], asked.get("event")), ("replacement", "damage"), asked)
        offer = next(o for o in asked["among"] if o["name"] == provider_name and (hit is None or o["hit"] == hit))
        self.engine.act({"kind": "choose", "answer": {"replacement": {k: offer[k] for k in ("source", "ability", "part", "hit")}}})

    def until_damage_question_or_after_combat(self):
        self.advance(lambda s: (s["asked"]["kind"] == "replacement" and s["asked"].get("event") == "damage")
                     or (s["part"] == "PostcombatMain" and s["asked"]["kind"] == "priority" and not s["stack"]))

    def test_furnace_doubles_one_attacker_without_a_question(self):
        self.new([{"battlefield": ["Grizzly Bears", "Furnace of Rath"]}, {}])
        self.attack(self.one("Grizzly Bears"), {"player": 1})
        self.after_combat()
        self.assertEqual(self.life(1), 16)
        self.assertEqual(self.questions, [])

    def test_furnace_doubles_two_attackers_and_asks_nothing(self):
        # Each hit has exactly one applicable effect: nothing to choose (616.1).
        self.new([{"battlefield": ["Grizzly Bears", "Grizzly Bears", "Furnace of Rath"]}, {}])
        self.attack_all(self.mine("Grizzly Bears", 0))
        self.after_combat()
        self.assertEqual(self.life(1), 12)
        self.assertEqual(self.questions, [], "one effect per hit is not a choice")

    def furnace_torbran(self):
        self.new([{"battlefield": ["Hill Giant", "Furnace of Rath", "Torbran, Thane of Red Fell"]}, {}])
        self.attack(self.one("Hill Giant"), {"player": 1})
        self.until_damage_question_or_after_combat()
        asked = self.asked()
        self.assertEqual(asked["player"], 1, "the affected player chooses, though P0 controls both effects")
        self.assertEqual(sorted(o["name"] for o in asked["among"]), ["Furnace of Rath", "Torbran, Thane of Red Fell"])
        self.assertEqual({o["damage_source"] for o in asked["among"]}, {self.one("Hill Giant")})
        return asked

    def test_torbran_then_furnace_is_ten(self):
        self.furnace_torbran()
        self.choose_damage("Torbran, Thane of Red Fell")
        self.after_combat()
        self.assertEqual(self.life(1), 10, "(3 + 2) x 2")

    def test_furnace_then_torbran_is_eight(self):
        self.furnace_torbran()
        self.choose_damage("Furnace of Rath")
        self.after_combat()
        self.assertEqual(self.life(1), 12, "3 x 2 + 2")

    def test_torbran_modifies_only_red_sources_you_control_to_opponents_and_their_permanents(self):
        self.new([{"hand": ["Shock", "Shock", "Shock"], "battlefield": ["Torbran, Thane of Red Fell", "Hill Giant"] + ["Mountain"] * 6},
                  {"battlefield": ["Craw Wurm"]}])
        self.main()
        giant, wurm = self.one("Hill Giant"), self.one("Craw Wurm")
        self.cast("Shock", [[{"player": 1}]], mana=self.ids("Mountain")[:1], spend={'R': 1})
        self.settle()
        self.assertEqual(self.life(1), 16, "2 + 2 to an opponent")
        self.cast("Shock", [[{"object": giant}]], mana=self.ids("Mountain")[1:2], spend={'R': 1})
        self.settle()
        self.assertEqual(self.obj(giant).get("damage"), 2, "not P0's own permanent")
        self.cast("Shock", [[{"object": wurm}]], mana=self.ids("Mountain")[2:3], spend={'R': 1})
        self.settle()
        self.assertNotIn(wurm, self.engine.state["battlefield"], "2 + 2 kills a 6/4 an opponent controls")

    def test_torbran_adds_to_a_red_attacker_but_not_to_the_opponents_blocker(self):
        self.new([{"battlefield": ["Torbran, Thane of Red Fell", "Hill Giant"]}, {"battlefield": ["Craw Wurm"]}])
        giant, wurm = self.one("Hill Giant"), self.one("Craw Wurm")
        self.attack(giant, {"player": 1})
        self.block([[wurm, giant]])
        self.after_combat()
        self.assertNotIn(wurm, self.engine.state["battlefield"], "3 + 2 kills the 6/4; 3 alone would not")
        self.assertNotIn(giant, self.engine.state["battlefield"])
        self.assertEqual(self.questions, [])

    def test_embermaw_hellion_adds_to_another_red_source_not_itself(self):
        self.new([{"hand": ["Shock"], "battlefield": ["Embermaw Hellion", "Mountain"]}, {}])
        self.main()
        self.cast("Shock", [[{"player": 1}]], mana=self.ids("Mountain"), spend={"R": 1})
        self.settle()
        self.assertEqual(self.life(1), 17)
        self.attack(self.one("Embermaw Hellion"), {"player": 1})
        self.after_combat()
        self.assertEqual(self.life(1), 13, "its own 4 is not increased")

    def test_two_embermaws_add_to_each_other(self):
        self.new([{"battlefield": ["Embermaw Hellion", "Embermaw Hellion"]}, {}])
        self.attack_all(self.mine("Embermaw Hellion", 0))
        self.after_combat()
        self.assertEqual(self.life(1), 10, "each 4 + 1 from the other (ruling 0abc6a04)")
        self.assertEqual(self.questions, [])

    def test_trample_divides_before_doubling(self):
        self.new([{"battlefield": ["Bloated Contaminator", "Furnace of Rath"]}, {"battlefield": ["Grizzly Bears"]}])
        bloated, bears = self.one("Bloated Contaminator"), self.one("Grizzly Bears")
        self.attack(bloated, {"player": 1})
        self.block([[bears, bloated]])
        for _ in range(60):
            asked = self.asked()
            if asked["kind"] == "holders":
                self.answer([])
                continue
            if asked["kind"] == "divide":
                self.assertEqual((asked["amount"], asked["lethal"]), (4, [2, None]), "lethal is from the undoubled amount")
                self.engine.act({"kind": "divide", "division": [[{"object": bears}, 2], [{"player": 1}, 2]]})
                continue
            if self.engine.state["part"] == "PostcombatMain" and asked["kind"] == "priority" and not self.engine.state["stack"]:
                break
            self.step()
        self.assertNotIn(bears, self.engine.state["battlefield"])
        self.assertEqual((self.life(1), self.poison(1)), (16, 1), "2 doubled to 4; toxic stays 1")

    def test_infect_poison_and_lifelink_see_the_doubled_damage(self):
        self.new([{"battlefield": ["Glistener Elf", "Vampire Nighthawk", "Furnace of Rath"]}, {}])
        self.attack_all([self.one("Glistener Elf"), self.one("Vampire Nighthawk")])
        self.after_combat()
        self.assertEqual((self.poison(1), self.life(1), self.life(0)), (2, 16, 24))

    def test_protection_prevents_all_and_asks_nothing(self):
        # Earth Elemental (4/5 red) survives Paladin's doubled first strike 2 -> 4.
        self.new([{"battlefield": ["Earth Elemental", "Furnace of Rath", "Torbran, Thane of Red Fell"]}, {"battlefield": ["Paladin en-Vec"]}])
        elemental, paladin = self.one("Earth Elemental"), self.one("Paladin en-Vec")
        self.attack(elemental, {"player": 1})
        self.block([[paladin, elemental]])
        self.after_combat()
        self.assertIn(paladin, self.engine.state["battlefield"])
        self.assertEqual(self.obj(paladin).get("damage", 0), 0, "protection from red (702.16e)")
        self.assertEqual(self.obj(elemental).get("damage"), 4, "Paladin's 2 doubled; Torbran is not for P1's sources")
        self.assertEqual(self.questions, [], "no order is asked for a hit protection prevents")

    def test_unpreventable_damage_to_a_protected_blocker_is_still_modified_by_its_controllers_choice(self):
        self.new([{"battlefield": ["Earth Elemental", "Furnace of Rath", "Torbran, Thane of Red Fell", "Leyline of Punishment"]},
                  {"battlefield": ["Paladin en-Vec"]}])
        elemental, paladin = self.one("Earth Elemental"), self.one("Paladin en-Vec")
        self.attack(elemental, {"player": 1})
        self.block([[paladin, elemental]])
        self.until_damage_question_or_after_combat()
        asked = self.asked()
        self.assertEqual((asked["kind"], asked.get("event"), asked["player"], asked["on"]), ("replacement", "damage", 1, {"object": paladin}))
        self.assertEqual([(h["source"], h["amount"]) for h in asked["hits"]], [(elemental, 4)])
        self.choose_damage("Furnace of Rath")
        self.after_combat()
        self.assertNotIn(paladin, self.engine.state["battlefield"], "4 x 2 + 2 can't be prevented")
        self.assertEqual(len(self.questions), 1)

    def test_furnace_doubles_a_spell_and_damage_to_its_own_controller(self):
        self.new([{"hand": ["Shock", "Shock"], "battlefield": ["Furnace of Rath", "Mountain", "Mountain"]}, {}])
        self.main()
        self.cast("Shock", [[{"player": 1}]], mana=self.ids("Mountain")[:1], spend={'R': 1})
        self.settle()
        self.cast("Shock", [[{"player": 0}]], mana=self.ids("Mountain")[1:2], spend={'R': 1})
        self.settle()
        self.assertEqual((self.life(1), self.life(0)), (16, 16))

    def test_same_source_to_one_player_uses_one_replacement_order(self):
        # Char's 4 and 2 damage to its caster happen simultaneously (608.2f).
        # Both replacements see one source dealing 6 to one player (700.1), and
        # each applies once (614.5). P0 chooses the order (616.1e-f): 6*2+1 or
        # (6+1)*2. The multiple-recipient Furnace ruling does not split this event.
        for first, damage in (("Furnace of Rath", 13), ("Embermaw Hellion", 14)):
            with self.subTest(first=first):
                self.new([{"hand": ["Char"], "battlefield": ["Furnace of Rath", "Embermaw Hellion"] + ["Mountain"] * 3}, {}])
                self.main()
                providers = {self.one(name) for name in ("Furnace of Rath", "Embermaw Hellion")}
                self.cast("Char", [[{"player": 0}]], mana=self.ids("Mountain"), spend={"R": 3})
                char, = self.engine.state["stack"]
                self.assertEqual(self.obj(char)["name"], "Char")
                self.settle()
                asked = self.asked()
                self.assertEqual((asked["kind"], asked.get("event"), asked["player"], asked["on"]), ("replacement", "damage", 0, {"player": 0}))
                self.assertEqual([(h["source"], h["amount"]) for h in asked["hits"]], [(char, 6)])
                hit, = asked["hits"]
                self.assertEqual(len(asked["among"]), 2)
                self.assertEqual({o["source"] for o in asked["among"]}, providers)
                self.assertEqual({o["name"] for o in asked["among"]}, {"Furnace of Rath", "Embermaw Hellion"})
                self.assertEqual({(o["damage_source"], o["hit"]) for o in asked["among"]}, {(char, hit["hit"])})
                self.choose_damage(first, hit["hit"])
                self.settle()
                self.assertEqual(self.asked()["kind"], "priority", "the sole remaining replacement needs no choice")
                self.assertEqual(self.engine.state["stack"], [])
                self.assertEqual((self.life(0), self.life(1)), (20 - damage, 20))
                dealt = [e for e in self.engine.state["events"] if e.get("event") == "damage"]
                self.assertEqual([(e["source"], e["to"], e["amount"]) for e in dealt], [(char, {"player": 0}, damage)])

    def test_both_views_and_refused_answers_keep_the_request(self):
        asked = self.furnace_torbran()
        offer = asked["among"][0]
        identity = {k: offer[k] for k in ("source", "ability", "part", "hit")}
        p0 = self.view(0)["asked"]
        self.assertEqual(p0["player"], 1, "P0 sees that P1 is choosing")
        p1 = self.view(1)["asked"]
        self.assertEqual((p1["event"], p1["on"]), ("damage", {"player": 1}))
        self.assertEqual([(h["source"], h["amount"]) for h in p1["hits"]], [(self.one("Hill Giant"), 3)])
        self.reject(0, {"kind": "choose", "answer": {"replacement": identity}})
        self.reject(1, {"kind": "choose", "answer": {"replacement": {k: offer[k] for k in ("source", "ability", "part")}}})
        self.reject(1, {"kind": "choose", "answer": {"replacement": {**identity, "hit": identity["hit"] + 999}}})
        self.reject(1, {"kind": "pass"})
        self.choose_damage(offer["name"], offer["hit"])
        self.after_combat()
        self.assertEqual(self.life(1), 10 if offer["name"].startswith("Torbran") else 12)


if __name__ == "__main__":
    unittest.main()
