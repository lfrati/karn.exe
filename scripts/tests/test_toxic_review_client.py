"""Toxic through the real audited
client with full pinned cards. Combat damage dealt to a player by a creature with toxic also
gives that player poison counters equal to its total toxic value, the sum of all its toxic
instances (702.164b-c, 120.3g), whatever the amount of damage; the damage's other results
still happen. Damage to a creature or planeswalker, and noncombat damage, give none. One
controller's simultaneous poison to a player is one event (ruling 4fbc5a7311baf246), Infect's
included (120.3b). Ten poison counters lose the game (704.5c)."""
import unittest
# the module, not its class, so that its own tests are not collected here again
import test_infect_review_client as base
from client_support import ClientTestCase

HELPERS = {k: v for k, v in vars(base.InfectReviewClientTests).items() if not k.startswith("test_") and not k.startswith("__")}


class ToxicReviewClientTests(ClientTestCase):
    locals().update(HELPERS)

    def new(self, seats):
        """Each view lists what happened since that viewer was last asked, so player 0's
        views together list every event once."""
        HELPERS["new"](self, seats)
        self.seen = []
        ask = self.engine.ask
        def recording(request):
            state = ask(request)
            if state["asked"] and state["asked"]["player"] == 0:
                self.seen.extend(state["events"])
            return state
        self.engine.ask = recording

    def poison_events(self, player=1):
        events = self.seen if self.asked() and self.asked()["player"] == 0 else self.engine.state["events"]
        return [e["count"] for e in events
                if e.get("event") == "counters" and e.get("on") == {"player": player} and e.get("counter") == "poison"]

    def main(self, turn=1):
        self.advance(lambda s: s["turn"] == turn and s["part"] == "PrecombatMain" and s["asked"]["kind"] == "priority" and s["active"] == 0)

    def nurse(self, nurse, mana):
        self.activate(nurse, [], ability=1, mana=mana, spend={'G': 3})
        self.settle()

    def test_unblocked_toxic_is_life_loss_and_poison_in_both_views(self):
        self.new([{"battlefield": ["Bilious Skulldweller"]}, {}])
        self.attack(self.one("Bilious Skulldweller"), {"player": 1})
        self.after_combat()
        self.assertEqual((self.life(1), self.poison(1)), (19, 1))
        self.assertEqual((self.poison(1, viewer=0), self.poison(1, viewer=1)), (1, 1), "poison counters are public")
        self.assertEqual(self.poison_events(), [1])

    def test_blocked_toxic_deals_ordinary_damage_and_gives_no_poison(self):
        self.new([{"battlefield": ["Bilious Skulldweller"]}, {"battlefield": ["Grizzly Bears"]}])
        bilious, bears = self.one("Bilious Skulldweller"), self.one("Grizzly Bears")
        self.attack(bilious, {"player": 1})
        self.block([[bears, bilious]])
        self.after_combat()
        self.assertNotIn(bears, self.engine.state["battlefield"], "deathtouch")
        self.assertEqual((self.life(1), self.poison(1), self.counters({"player": 1})), (20, 0, {}))

    def test_double_strike_gives_toxic_in_each_damage_step(self):
        self.new([{"battlefield": ["Jawbone Duelist"]}, {}])
        self.attack(self.one("Jawbone Duelist"), {"player": 1})
        self.after_combat()
        self.assertEqual((self.life(1), self.poison(1)), (18, 2))
        self.assertEqual(self.poison_events(), [1, 1], "two combat damage steps, two events (510.4)")

    def test_first_strike_on_its_controllers_turn_gives_toxic_once(self):
        self.new([{"battlefield": ["Duelist of Deep Faith"]}, {}])
        self.attack(self.one("Duelist of Deep Faith"), {"player": 1})
        self.after_combat()
        self.assertEqual((self.life(1), self.poison(1), self.poison_events()), (18, 1, [1]))

    def test_toxic_is_independent_of_the_damage_amount(self):
        # Slaughter Singer gives the attacking Skulldweller +1/+1: 2 damage, still 1 poison.
        self.new([{"battlefield": ["Bilious Skulldweller", "Slaughter Singer"]}, {}])
        self.attack(self.one("Bilious Skulldweller"), {"player": 1})
        self.after_combat()
        self.assertEqual((self.life(1), self.poison(1)), (18, 1))

    def test_bloated_contaminator_unblocked_then_proliferates_the_completed_poison(self):
        self.new([{"battlefield": ["Bloated Contaminator"]}, {}])
        self.attack(self.one("Bloated Contaminator"), {"player": 1})
        self.advance(lambda s: s["asked"]["kind"] == "holders")
        self.assertEqual((self.life(1), self.poison(1)), (16, 1), "the trigger resolves after the damage's results")
        self.answer([{"player": 1}])
        self.after_combat()
        self.assertEqual(self.poison(1), 2)

    def test_bloated_contaminator_trampling_over_a_blocker(self):
        self.new([{"battlefield": ["Bloated Contaminator"]}, {"battlefield": ["Grizzly Bears"]}])
        bloated, bears = self.one("Bloated Contaminator"), self.one("Grizzly Bears")
        self.attack(bloated, {"player": 1})
        self.block([[bears, bloated]])
        for _ in range(60):
            kind = self.asked()["kind"]
            if kind == "holders":
                break
            if kind == "divide":
                self.assertEqual((self.asked()["amount"], self.asked()["lethal"]), (4, [2, None]))
                self.engine.act({"kind": "divide", "division": [[{"object": bears}, 2], [{"player": 1}, 2]]})
            else:
                self.step()
        self.assertNotIn(bears, self.engine.state["battlefield"])
        self.assertEqual((self.life(1), self.poison(1)), (18, 1), "toxic once, for the positive player hit")

    def test_plague_nurse_grant_adds_an_equal_toxic_instance_and_expires(self):
        self.new([{"battlefield": ["Plague Nurse", "Bilious Skulldweller", "Forest", "Forest", "Forest"]}, {}])
        self.main()
        self.nurse(self.one("Plague Nurse"), self.ids("Forest", zone="battlefield")[:3])
        self.attack(self.one("Bilious Skulldweller"), {"player": 1})
        self.after_combat()
        self.assertEqual((self.life(1), self.poison(1)), (19, 2), "printed toxic 1 plus granted toxic 1 (ruling 59a8529aca051c05)")
        self.advance(lambda s: s["turn"] == 3 and s["asked"]["kind"] == "attackers")
        self.engine.act({"kind": "attack", "attacks": [[self.one("Bilious Skulldweller"), {"player": 1}]]})
        self.after_combat()
        self.assertEqual((self.life(1), self.poison(1)), (18, 3), "the grant ended: toxic 1 again")

    def test_two_nurses_stack_and_every_toxic_attacker_is_one_poison_event(self):
        self.new([{"battlefield": ["Plague Nurse", "Plague Nurse", "Bilious Skulldweller"] + ["Forest"] * 6}, {}])
        self.main()
        for index, nurse in enumerate(self.mine("Plague Nurse", 0)):
            self.nurse(nurse, self.ids("Forest", zone="battlefield")[index*3:index*3+3])
        self.attack_all(self.mine("Plague Nurse", 0) + [self.one("Bilious Skulldweller")])
        self.after_combat()
        # each Nurse: 2 + 1 from the other; the Skulldweller: 1 + 1 + 1
        self.assertEqual(self.poison_events(), [9])
        self.assertEqual((self.life(1), self.poison(1)), (13, 9))

    def test_a_grant_after_blockers_counts(self):
        self.new([{"battlefield": ["Plague Nurse", "Bilious Skulldweller", "Forest", "Forest", "Forest"]}, {}])
        self.attack(self.one("Bilious Skulldweller"), {"player": 1})
        self.advance(lambda s: s["part"] == "DeclareBlockers" and s["asked"]["kind"] == "priority" and s["asked"]["player"] == 0)
        self.nurse(self.one("Plague Nurse"), self.ids("Forest", zone="battlefield")[:3])
        self.after_combat()
        self.assertEqual((self.life(1), self.poison(1)), (19, 2))

    def test_equipment_and_aura_grants_add_up(self):
        self.new([{"hand": ["Necrogen Communion"], "battlefield": ["Bilious Skulldweller", "Prosthetic Injector", "Swamp", "Swamp", "Swamp"]}, {}])
        bilious = self.one("Bilious Skulldweller")
        self.main()
        self.activate(self.one("Prosthetic Injector"), [[{"object": bilious}]], ability=2, mana=self.ids("Swamp")[:1], spend={'B': 1})
        self.settle()
        self.cast("Necrogen Communion", [[{"object": bilious}]], mana=self.ids("Swamp")[1:3], spend={'B': 2})
        self.settle()
        self.attack(bilious, {"player": 1})
        self.after_combat()
        self.assertEqual((self.life(1), self.poison(1)), (19, 4), "printed 1 + equipment 1 + aura 2")

    def test_infect_and_toxic_are_additive_and_one_event_with_another_infect_attacker(self):
        self.new([{"hand": ["Tainted Strike"], "battlefield": ["Bilious Skulldweller", "Glistener Elf", "Swamp"]}, {}])
        bilious = self.one("Bilious Skulldweller")
        self.main()
        self.cast("Tainted Strike", [[{"object": bilious}]], mana=[self.one("Swamp")], spend={'B': 1})
        self.settle()
        self.attack_all([bilious, self.one("Glistener Elf")])
        self.after_combat()
        # Skulldweller: 2 infect damage + toxic 1; Elf: 1 infect damage; no life loss
        self.assertEqual(self.poison_events(), [4])
        self.assertEqual((self.life(1), self.poison(1)), (20, 4))

    def test_ten_poison_from_one_combat_loses_after_completion(self):
        self.new([{"battlefield": ["Paladin of Predation", "Tyrranax Rex"]}, {}])
        self.attack_all([self.one("Paladin of Predation"), self.one("Tyrranax Rex")])
        for _ in range(40):
            if self.engine.state.get("finished") or self.engine.state["players"][1].get("lost"):
                break
            self.step()
        self.assertEqual(self.poison_events(), [10])
        self.assertEqual(self.life(1), 6, "the damage's life loss still happens")
        self.assertTrue(self.engine.state.get("finished") or self.engine.state["players"][1].get("lost"), "704.5c")

    def test_crawling_chorus_mite_token_deals_toxic_and_fling_is_not_toxic(self):
        self.new([{"hand": ["Fling"], "battlefield": ["Crawling Chorus", "Mountain", "Mountain"]}, {}])
        self.main()
        chorus = self.one("Crawling Chorus")
        self.cast("Fling", [[{"player": 1}]], mana=self.ids("Mountain")[:2], paying=[[chorus]], spend={'R': 2})
        self.settle()
        self.assertEqual((self.life(1), self.poison(1)), (19, 0), "Fling is the source, not the toxic creature")
        mites = self.mine("Phyrexian Mite Token", 0)
        self.assertEqual(len(mites), 1)
        self.advance(lambda s: s["turn"] == 3 and s["asked"]["kind"] == "attackers")
        self.engine.act({"kind": "attack", "attacks": [[mites[0], {"player": 1}]]})
        self.after_combat()
        self.assertEqual((self.life(1), self.poison(1)), (18, 1), "the token keeps toxic 1")

    def mite(self, extra_hand=(), extra_lands=()):
        """Fling sacrificing Crawling Chorus: its dies trigger makes P0 a Mite token."""
        self.new([{"hand": ["Fling", *extra_hand], "battlefield": ["Crawling Chorus", "Mountain", "Mountain", *extra_lands]},
                  {"battlefield": ["Grizzly Bears"]}])
        self.main()
        chorus = self.one("Crawling Chorus")
        self.cast("Fling", [[{"object": self.one("Grizzly Bears")}]], mana=self.ids("Mountain")[:2], paying=[[chorus]], spend={'R': 2})
        self.settle()
        return self.one("Phyrexian Mite Token")

    def test_a_clone_of_the_mite_token_copies_its_toxic(self):
        mite = self.mite(["Clone"], ["Island"] * 4)
        self.assertIn(self.one("Grizzly Bears"), self.engine.state["battlefield"], "1 damage from Fling")
        self.cast("Clone", mana=self.ids("Island")[:4], spend={'U': 4})
        for _ in range(20):
            asked = self.asked()
            if asked["kind"] == "priority" and not self.engine.state["stack"]:
                break
            if asked["kind"] == "may":
                self.engine.act({"kind": "choose", "answer": "yes"})
            elif asked["kind"] == "priority":
                self.engine.act({"kind": "pass"})
            else:
                self.assertEqual(asked["kind"], "select", asked)
                self.assertIn(mite, asked["among"])
                self.engine.act({"kind": "choose", "answer": {"objects": [mite]}})
        clones = [o for o in self.mine("Phyrexian Mite Token", 0) if o != mite]
        self.assertEqual(len(clones), 1, "Clone entered as a copy of the Mite (707.2)")
        self.advance(lambda s: s["turn"] == 3 and s["asked"]["kind"] == "attackers")
        self.engine.act({"kind": "attack", "attacks": [[mite, {"player": 1}], [clones[0], {"player": 1}]]})
        self.advance(lambda s: s["asked"]["kind"] == "blockers")
        self.engine.act({"kind": "block", "blocks": []})
        self.after_combat()
        self.assertEqual(self.poison_events(), [2], "two toxic 1 creatures, one event")
        self.assertEqual((self.life(1), self.poison(1)), (18, 2))

    def test_the_mite_token_cannot_block(self):
        mite = self.mite()
        self.advance(lambda s: s["turn"] == 2 and s["asked"]["kind"] == "attackers")
        self.engine.act({"kind": "attack", "attacks": [[self.one("Grizzly Bears"), {"player": 0}]]})
        bears = self.one("Grizzly Bears")
        self.advance(lambda s: s["asked"]["kind"] == "blockers" or s["part"] in ("CombatDamage", "EndOfCombat"))
        if self.asked()["kind"] == "blockers":
            self.reject(0, {"kind": "block", "blocks": [[mite, bears]]})
            self.engine.act({"kind": "block", "blocks": []})
        self.advance(lambda s: s["part"] == "PostcombatMain")
        self.assertEqual((self.life(0), self.poison(0)), (18, 0), "the Bears were not blocked")
        self.assertIn(mite, self.engine.state["battlefield"])


if __name__ == "__main__":
    unittest.main()
