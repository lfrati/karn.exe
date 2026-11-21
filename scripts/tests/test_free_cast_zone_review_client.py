"""Free casts from the player's own graveyard,
through the real audited client with full pinned cards. "You may cast target ... card from your
graveyard without paying its mana cost" (Scholar of the Lost Trove and the cards like it)
casts the card from the graveyard during the resolution, ignoring timing (608.2g, 601.2).
Without paying the mana cost, X is 0 and no alternative cost can be paid, but optional
additional costs (kicker, buyback) may be (601.2b, 118.9; Cast Through Time ruling dc98f98b). Scholar's "if an instant or sorcery spell cast this way would be put into your
graveyard, exile it instead" applies wherever it would go there from, countered included, and
competes with buyback as a replacement its controller chooses between (616.1). Exile and copy
routes (Hidetsugu and Kairi, Shiko) are contrasts that must keep working."""
import unittest
import test_toxic_review_client as base
import test_multiplayer_outcome_client as explicit

HELPERS = {k: v for k, v in vars(base.ToxicReviewClientTests).items() if not k.startswith("test_") and not k.startswith("__")}


class FreeCastZoneReviewClientTests(unittest.TestCase):
    locals().update(HELPERS)
    validate_action = explicit.MultiplayerOutcomeClientTests.validate_action

    # --- reading the state

    def state(self):
        return self.engine.state

    def names(self, ids, viewer=0):
        objects = self.view(viewer)["objects"]
        return sorted(objects[str(o)]["name"] for o in ids)

    def graveyard(self, player=0):
        return self.names(self.state()["players"][player]["graveyard"])

    def exiled(self):
        return self.names(self.state()["exile"])

    def hand_names(self, player=0):
        v = self.view(player)
        return sorted(v["objects"][str(o)]["name"] for o in v["players"][player]["hand"])

    def untapped_lands(self, player=0):
        return [o for o in self.state()["battlefield"] if self.obj(o)["controller"] == player
                and "Land" in self.obj(o)["types"] and not self.obj(o).get("tapped")]

    def nonland(self, player=0):
        return sorted(self.obj(o)["name"] for o in self.state()["battlefield"]
                      if self.obj(o)["controller"] == player and "Land" not in self.obj(o)["types"])

    def offered(self):
        """The resolving effect's permitted card domain (CR 608.2g)."""
        a = self.asked()
        self.assertEqual((a["kind"], a["player"]), ("cast", 0), a)
        return a["cards"]

    # --- acting

    def until_asked(self):
        for _ in range(40):
            a = self.asked()
            if a["kind"] != "priority" or not self.state()["stack"]:
                return a
            self.engine.act({"kind": "pass"})
        self.fail("the stack did not settle")

    def scholar(self, graveyard, lands=7, opponent=None, extra_hand=()):
        """Scholar resolves and its enters trigger, targeting the named graveyard card, is
        about to resolve; returns that card."""
        self.new([{"hand": ["Scholar of the Lost Trove", *extra_hand], "graveyard": graveyard, "battlefield": ["Island"] * lands},
                  opponent or {"battlefield": ["Grizzly Bears"]}])
        self.main()
        self.cast("Scholar of the Lost Trove", mana=self.ids("Island", zone="battlefield", controller=0)[:7], spend={'U': 7})
        a = self.until_asked()
        self.assertEqual(a["kind"], "targets", a)
        card = next(o for o in self.state()["players"][0]["graveyard"] if self.obj(o)["name"] == graveyard[0])
        self.engine.act({"kind": "targets", "targets": [[{"object": card}]]})
        self.until_asked()
        return card

    def cast_offer(self, card, targets=(), *, plan=(), **announced):
        self.assertIn(card, self.offered())
        self.engine.act({"kind": "cast", "card": card, "route": "normal", "plan": list(plan),
                         "targets": list(targets), **announced})

    # --- Scholar of the Lost Trove: from the graveyard

    def test_scholar_casts_the_targeted_instant_free_and_exiles_it(self):
        card = self.scholar(["Unsummon"])
        offers = self.offered()
        self.assertEqual(set(offers), {card})
        bears = self.one("Grizzly Bears")
        self.cast_offer(card, [[{"object": bears}]])
        self.assertEqual(len(self.untapped_lands()), 0, "Scholar's {5}{U}{U} only")
        self.until_asked()
        self.assertEqual((self.nonland(1), self.hand_names(1)), ([], ["Grizzly Bears"]))
        self.assertEqual((self.graveyard(), self.exiled()), ([], ["Unsummon"]), "the rider")

    def test_scholar_casts_a_sorcery_at_instant_speed_during_its_resolution(self):
        card = self.scholar(["Divination"], lands=7)
        self.cast_offer(card)
        self.until_asked()
        self.assertEqual(len(self.hand_names()), 2)
        self.assertEqual(self.exiled(), ["Divination"])

    def test_scholar_casts_an_artifact_which_enters_and_is_not_exiled(self):
        card = self.scholar(["Mind Stone"])
        self.cast_offer(card)
        self.until_asked()
        self.assertEqual(self.nonland(), ["Mind Stone", "Scholar of the Lost Trove"])
        self.assertEqual(self.exiled(), [])

    def test_declining_leaves_the_card_in_the_graveyard(self):
        card = self.scholar(["Unsummon"])
        self.offered()
        self.reject(0, {"kind": "pass"})
        self.engine.act({"kind": "choose", "answer": "no"})
        self.until_asked()
        self.assertEqual((self.graveyard(), self.exiled()), (["Unsummon"], []))
        self.assertIn(card, self.state()["players"][0]["graveyard"])

    def test_a_countered_free_spell_is_exiled_by_the_rider(self):
        card = self.scholar(["Unsummon"], opponent={"hand": ["Counterspell"], "battlefield": ["Island", "Island", "Grizzly Bears"]})
        self.cast_offer(card, [[{"object": self.one("Grizzly Bears")}]])
        self.engine.act({"kind": "pass"})
        self.cast("Counterspell", [[{"object": self.state()["stack"][-1]}]], mana=self.ids("Island", zone="battlefield", controller=1)[:2], spend={'U': 2})
        self.until_asked()
        self.assertEqual(self.nonland(1), ["Grizzly Bears"])
        self.assertEqual(self.exiled(), ["Unsummon"])

    def test_kicker_may_be_paid_on_the_free_cast(self):
        card = self.scholar(["Burst Lightning"], lands=11)
        offers = self.offered()
        for kicks, sources in ((0, []), (1, self.ids("Island", zone="battlefield", controller=0)[7:11])):
            self.validate_action({"kind": "cast", "card": card, "route": "normal", "kicks": kicks, "mana": {"U": 4} if kicks else {},
                                  "targets": [[{"player": 1}]], "plan": [{"source": o, "ability": 0} for o in sources]})
        self.cast_offer(card, [[{"player": 1}]], kicks=1, mana={"U": 4},
                        plan=[{"source": o, "ability": 0} for o in self.ids("Island", zone="battlefield", controller=0)[7:11]])
        self.assertEqual(len(self.untapped_lands()), 0)
        self.until_asked()
        self.assertEqual((self.state()["players"][1]["life"], self.exiled()), (16, ["Burst Lightning"]))

    def test_x_is_zero_on_the_free_cast(self):
        card = self.scholar(["Fanning the Flames"], lands=9)
        self.reject(0, {"kind": "cast", "card": card, "route": "normal", "x": 1, "mana": {"U": 1}, "targets": [[{"player": 1}]],
                        "plan": [{"source": self.ids("Island", zone="battlefield", controller=0)[7], "ability": 0}]})
        self.cast_offer(card, [[{"player": 1}]], x=0)
        self.until_asked()
        self.assertEqual(self.state()["players"][1]["life"], 20)

    def scholar_capsize(self):
        card = self.scholar(["Capsize"], lands=10)
        target = {"object": self.one("Grizzly Bears")}
        for buyback, sources in ((False, []), (True, self.ids("Island", zone="battlefield", controller=0)[7:10])):
            self.validate_action({"kind": "cast", "card": card, "route": "normal", "buyback": buyback, "mana": {"U": 3} if buyback else {},
                                  "targets": [[target]], "plan": [{"source": o, "ability": 0} for o in sources]})
        self.cast_offer(card, [[target]], buyback=True, mana={"U": 3},
                        plan=[{"source": o, "ability": 0} for o in self.ids("Island", zone="battlefield", controller=0)[7:10]])
        a = self.until_asked()
        self.assertEqual((a["kind"], a.get("event"), a["player"]), ("replacement", "spell_destination", 0), a)
        self.assertEqual(sorted(e["effect"] if isinstance(e, dict) else e for e in a["among"]), ["buyback", "cast_rider"])  # entries name their source too
        return a

    def test_paid_buyback_on_the_free_cast_competes_with_the_rider(self):
        for effect, where in (("buyback", ["Capsize"]), ("cast_rider", [])):
            with self.subTest(effect=effect):
                a = self.scholar_capsize()
                self.engine.act({"kind": "choose", "answer": {"spell_destination": {"on": a["on"]["object"], "effect": effect}}})
                self.until_asked()
                self.assertEqual(self.hand_names(), where)
                self.assertEqual(self.exiled(), [] if where else ["Capsize"])
                self.assertEqual(self.hand_names(1), ["Grizzly Bears"])

    # --- contrasts: exile and copy routes

    def test_hidetsugu_and_kairi_still_casts_the_exiled_card(self):
        """Its dies trigger exiles the top card, then offers to cast it from exile."""
        self.new([{"hand": ["Murder"], "battlefield": ["Hidetsugu and Kairi"] + ["Swamp"] * 3,
                   "library": ["Lightning Bolt"] * 5}, {}])
        self.main()
        self.cast("Murder", [[{"object": self.one("Hidetsugu and Kairi")}]], mana=self.ids("Swamp", zone="battlefield", controller=0)[:3], spend={'B': 3})
        a = self.until_asked()
        self.assertEqual(a["kind"], "targets", a)
        self.engine.act({"kind": "targets", "targets": [[{"player": 1}]]})
        self.until_asked()
        offers = self.offered()
        self.assertEqual({self.view(0)["objects"][str(m)]["name"] for m in offers}, {"Lightning Bolt"})
        self.cast_offer(offers[0], [[{"player": 1}]])
        self.until_asked()
        self.assertEqual(self.state()["players"][1]["life"], 20 - 1 - 3, "its mana value, then the Bolt")
        self.assertEqual(self.graveyard(), ["Hidetsugu and Kairi", "Lightning Bolt", "Murder"])


    def test_shiko_still_casts_a_copy_of_the_exiled_card(self):
        """Shiko exiles the graveyard card, then casts a copy of it; the copy ceases to exist."""
        self.new([{"hand": ["Shiko, Paragon of the Way"], "graveyard": ["Divination"],
                   "battlefield": ["Island", "Island", "Mountain", "Plains", "Plains"]}, {}])
        self.main()
        self.cast("Shiko, Paragon of the Way", mana=self.ids("Island", zone="battlefield", controller=0)[:2]
                  + self.ids("Mountain", zone="battlefield", controller=0)[:1] + self.ids("Plains", zone="battlefield", controller=0)[:2], spend={'U': 2, 'R': 1, 'W': 2})
        a = self.until_asked()
        self.assertEqual(a["kind"], "targets", a)
        card = self.state()["players"][0]["graveyard"][0]
        self.engine.act({"kind": "targets", "targets": [[{"object": card}]]})
        self.until_asked()
        offers = self.offered()
        self.assertEqual(len(offers), 1, offers)
        self.cast_offer(offers[0])
        self.until_asked()
        self.assertEqual((len(self.hand_names()), self.exiled(), self.graveyard()), (2, ["Divination"], []))

    # --- newly admitted by the correction

    def enters_with_target(self, creature, graveyard, lands, land="Island", *, spend):
        """Cast the creature; its enters trigger is asked for its graveyard target."""
        self.new([{"hand": [creature], "graveyard": graveyard, "battlefield": [land] * lands}, {"battlefield": ["Grizzly Bears"]}])
        self.main()
        self.cast(creature, mana=self.ids(land, zone="battlefield", controller=0)[:lands], spend=spend)
        a = self.until_asked()
        self.assertEqual(a["kind"], "targets", a)
        return a

    def test_goblin_dark_dwellers_targets_only_mana_value_three_or_less(self):
        a = self.enters_with_target("Goblin Dark-Dwellers", ["Divination", "Tooth and Nail", "Mind Stone"], 5, "Mountain", spend={"R": 5})
        self.assertEqual(self.names([t["object"] for t in a["slots"][0]["among"]]), ["Divination"], "an instant or sorcery card, MV <= 3")
        self.engine.act({"kind": "targets", "targets": [[a["slots"][0]["among"][0]]]})
        self.until_asked()
        self.cast_offer(self.offered()[0])
        self.until_asked()
        self.assertEqual((len(self.hand_names()), self.exiled()), (2, ["Divination"]))

    def test_torrential_gearhulk_targets_only_an_instant(self):
        a = self.enters_with_target("Torrential Gearhulk", ["Divination", "Unsummon"], 6, spend={"U": 6})
        self.assertEqual(self.names([t["object"] for t in a["slots"][0]["among"]]), ["Unsummon"])

    def counterpoint(self, countered, graveyard):
        """P1 casts the spell; P0 counters it with Counterpoint and is offered its graveyard."""
        self.new([{"hand": ["Counterpoint"], "graveyard": graveyard, "battlefield": ["Island", "Island", "Island", "Swamp", "Swamp"]},
                  {"hand": [countered], "battlefield": ["Mountain"] * 6}])
        self.advance(lambda s: s["turn"] == 2 and s["part"] == "PrecombatMain" and s["asked"]["kind"] == "priority" and s["asked"]["player"] == 1)
        self.cast(countered, mana=self.ids("Mountain", zone="battlefield", controller=1)[:4], spend={'R': 4})
        spell = self.state()["stack"][-1]
        self.engine.act({"kind": "pass"})
        self.cast("Counterpoint", [[{"object": spell}]], mana=self.ids("Island", zone="battlefield", controller=0)[:3]
                  + self.ids("Swamp", zone="battlefield", controller=0)[:2], spend={'U': 3, 'B': 2})
        return self.until_asked()

    def test_counterpoint_offers_cards_up_to_the_countered_spells_mana_value_without_a_rider(self):
        a = self.counterpoint("Hill Giant", ["Grizzly Bears", "Divination", "Craw Wurm", "Mind Stone", "Lightning Bolt"])
        self.assertEqual((a["kind"], a["player"]), ("cast", 0), a)
        self.assertEqual(sorted({self.view(0)["objects"][str(m)]["name"] for m in a["cards"]}),
                         ["Divination", "Grizzly Bears", "Lightning Bolt"], "creature/instant/sorcery/planeswalker, MV <= 4")
        bears = next(m for m in a["cards"] if self.view(0)["objects"][str(m)]["name"] == "Grizzly Bears")
        self.cast_offer(bears)
        self.until_asked()
        self.assertEqual(self.nonland(), ["Grizzly Bears"], "a creature cast on P1's turn")
        self.assertEqual(self.graveyard(1), ["Hill Giant"])
        self.assertEqual(self.graveyard(), ["Counterpoint", "Craw Wurm", "Divination", "Lightning Bolt", "Mind Stone"])

    def test_counterpoint_cast_instant_goes_to_the_graveyard(self):
        a = self.counterpoint("Hill Giant", ["Lightning Bolt"])
        self.cast_offer(a["cards"][0], [[{"player": 1}]])
        self.until_asked()
        self.assertEqual(self.state()["players"][1]["life"], 17)
        self.assertEqual((self.exiled(), self.graveyard()), ([], ["Counterpoint", "Lightning Bolt"]), "no rider")

    def test_diviner_of_mist_may_cast_a_card_it_milled(self):
        self.new([{"battlefield": ["Diviner of Mist"], "library": ["Lightning Bolt"] * 8}, {}])
        self.main()
        self.attack(self.one("Diviner of Mist"), {"player": 1})
        a = self.until_asked()
        self.assertEqual((a["kind"],len(a["cards"])),("order_cards",4))
        first,second,third,fourth=a["cards"]
        self.engine.act({"kind":"choose","answer":{"objects":[fourth,third,second,first]}})
        a=self.until_asked()
        self.assertEqual((a["kind"], len(a["cards"]) >= 1), ("cast", True), a)
        self.assertEqual({self.view(0)["objects"][str(m)]["name"] for m in a["cards"]}, {"Lightning Bolt"})
        self.assertEqual(self.state()["players"][0]["library"], 4)
        self.cast_offer(a["cards"][0], [[{"player": 1}]])
        self.until_asked()
        self.assertEqual(self.state()["players"][1]["life"], 17)
        self.assertEqual((self.graveyard(), self.exiled()), (["Lightning Bolt"] * 3, ["Lightning Bolt"]))

if __name__ == "__main__":
    unittest.main()
