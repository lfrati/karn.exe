"""Entwine through the real audited client with
full pinned cards. "Entwine [cost]" means "you may choose all modes of this spell instead of just
the number specified; if you do, you pay an additional [cost]" (702.42a), following the rules for
choosing modes and paying additional costs (601.2b, 601.2f-h); each chosen mode needs its targets
(601.2c), and the same object may be chosen once for each word "target" (115.3). If the entwine
cost was paid, the modes are followed in the order written (702.42b; Kaya's Guile ruling
c3ff939470b76723), whatever order a command lists them in. Cost increases and reductions apply to
the total (601.2f), and a cast without paying the mana cost may still pay entwine (Cast Through
Time ruling dc98f98b). A copy copies the chosen modes and the paid decision (707.10) and pays
nothing again."""
import unittest
import test_toxic_review_client as base
from client_support import ClientTestCase

HELPERS = {k: v for k, v in vars(base.ToxicReviewClientTests).items() if not k.startswith("test_") and not k.startswith("__")}


class EntwineReviewClientTests(ClientTestCase):
    locals().update(HELPERS)

    # --- reading the state

    def state(self):
        return self.engine.state

    def untapped(self, player=0):
        return [o for o in self.state()["battlefield"] if self.obj(o)["controller"] == player and not self.obj(o).get("tapped")]

    def untapped_named(self, name, player=0):
        return [o for o in self.untapped(player) if self.obj(o)["name"] == name]

    def mine(self, name, player=0):
        return [o for o in self.state()["battlefield"] if self.obj(o)["name"] == name and self.obj(o)["controller"] == player]

    def hand_names(self, player=0):
        v = self.view(player)
        return sorted(v["objects"][str(o)]["name"] for o in v["players"][player]["hand"])

    def names(self, ids, viewer=0):
        objects = self.view(viewer)["objects"]
        return sorted(objects[str(o)]["name"] for o in ids)

    def graveyard(self, player):
        return self.names(self.state()["players"][player]["graveyard"])

    def nonland(self, player=0):
        return sorted(self.obj(o)["name"] for o in self.state()["battlefield"]
                      if self.obj(o)["controller"] == player and "Land" not in self.obj(o)["types"])

    def events(self, kind):
        return [e for e in self.seen if e.get("event") == kind]

    # --- acting

    def spell_command(self, name, targets=(), modes=(), mana=(), spend=None, **announced):
        actor=self.asked().get("acting",self.asked()["player"])
        cards=self.asked()["cards"] if self.asked()["kind"]=="cast" else self.view(actor)["players"][actor]["hand"]
        card=next(o for o in cards if self.obj(o)["name"]==name)
        return {"kind":"cast","card":card,"targets":list(targets),"modes":list(modes),
                "plan":[{"source":o,"ability":0} for o in mana],**({"mana": spend} if spend is not None else {}),**announced}

    def cast_spell(self, name, targets=(), modes=(), mana=(), spend=None, **announced):
        before=list(self.state()["stack"])
        self.engine.act(self.spell_command(name,targets,modes,mana,spend=spend,**announced))
        spell=[o for o in self.state()["stack"] if o not in before]
        self.assertEqual(len(spell),1)
        return spell[0]

    def resolve(self, picks=()):
        """Pass until the stack is empty, answering each object choice with the named objects;
        returns (asked player, verb or kind, names offered) of each question."""
        picks, asked = list(picks), []
        for _ in range(12):
            a = self.settle()
            if a["kind"] == "priority":
                return asked
            names = {o: self.view(a["player"])["objects"][str(o)]["name"] for o in a["among"]}
            asked.append((a["player"], a.get("verb", a["kind"]), sorted(names.values())))
            want = list(picks.pop(0))
            chosen = []
            for o in a["among"]:
                if names[o] in want:
                    want.remove(names[o])
                    chosen.append(o)
            self.engine.act({"kind": "choose", "answer": {"objects": chosen}})
        self.fail("too many questions")

    # --- authored announcements and refusals

    def test_entwined_total_is_paid_in_full(self):
        for name, land, enough, targets in [("Tooth and Nail","Forest",9,False),("Grab the Reins","Mountain",7,True)]:
            for n in [enough-1,enough]:
                self.new([{"hand":[name],"battlefield":[land]*n},{"battlefield":["Hill Giant"]}])
                self.main()
                aim=[[{"object":self.one("Hill Giant")}],[{"player":1}]] if targets else []
                cmd=self.spell_command(name,aim,[0,1],self.mine(land),entwine=True, spend=({"G": n} if land == "Forest" else {"R": n}))
                if n<enough:
                    self.reject(0,cmd)
                    self.assertEqual(len(self.untapped()),n)
                else:
                    self.engine.act(cmd)
                    self.assertEqual(self.untapped(),[])

    def test_choose_two_entwine_selects_all_four(self):
        self.new([{"hand":["Kaya's Guile"],"battlefield":["Plains","Swamp"]+["Plains"]*4},{"battlefield":["Grizzly Bears"]}])
        self.main()
        for modes in [[0,1],[0,2],[0,3],[1,2],[1,3],[2,3],[0,1,2,3]]:
            paid=len(modes)==4
            cmd=self.spell_command("Kaya's Guile",modes=modes,mana=self.mine("Swamp")+self.mine("Plains")[:5 if paid else 2],entwine=paid, spend={"B": 1, "W": 5 if paid else 2})
            state=self.state()
            quote=self.engine.send({"cmd":"quote_payment","session_id":state["session_id"],"request_id":state["request_id"],"player":0,"move":cmd})
            self.assertIsNone(quote["error"],quote)

    def test_mismatched_or_unaffordable_announcements_are_refused_atomically(self):
        self.new([{"hand": ["Tooth and Nail", "Lightning Bolt", "Kaya's Guile"],
                   "battlefield": ["Forest"] * 9 + ["Mountain", "Plains", "Swamp"]}, {}])
        self.main()
        tooth = self.spell_command("Tooth and Nail",modes=[0],mana=self.mine("Forest")[:7], spend={'G': 7})
        kaya = self.spell_command("Kaya's Guile",modes=[0,1],mana=self.mine("Plains")+self.mine("Swamp")+self.mine("Mountain"), spend={"W": 1, "B": 1, "R": 1})
        bolt = self.spell_command("Lightning Bolt",[[{"player":1}]],mana=self.mine("Mountain"), spend={'R': 1})
        before = (self.untapped(), self.hand_names(), self.state()["stack"], self.life(1))
        bad = [
            {**tooth, "entwine": True},                      # paid, one mode
            {**tooth, "modes": [0, 1], "entwine": False},    # both modes, unpaid
            {**tooth, "modes": [0, 1]},                      # both modes, absent means unpaid
            {**tooth, "modes": [0, 0], "entwine": True},     # a mode twice
            {**tooth, "modes": [0, 1, 1], "entwine": True},
            {**tooth, "modes": [0, 2], "entwine": True},     # no such mode
            {**tooth, "modes": [], "entwine": True},
            {**tooth, "entwine": "true"}, {**tooth, "entwine": 1}, {**tooth, "entwine": None},
            {**kaya, "modes": [0, 1, 2], "entwine": True},   # paid, three of four
            {**kaya, "modes": [0, 1, 2, 3], "entwine": False},
            {**kaya, "modes": [0, 1, 2]},
            {**bolt, "entwine": True}, {**bolt, "entwine": False},
            {"kind": "pass", "entwine": False},
        ]
        for cmd in bad:
            with self.subTest(cmd={k: cmd.get(k) for k in ("kind", "card", "modes", "entwine")}):
                self.reject(0, cmd)
        self.assertEqual((self.untapped(), self.hand_names(), self.state()["stack"], self.life(1)), before)

    def test_a_paid_command_one_mana_short_is_refused(self):
        self.new([{"hand": ["Tooth and Nail"], "battlefield": ["Forest"] * 8}, {}])
        self.main()
        unpaid = self.spell_command("Tooth and Nail",modes=[0],mana=self.mine("Forest"), spend={'G': 8})
        self.reject(0, {**unpaid, "modes": [0, 1], "entwine": True})
        self.assertEqual(len(self.untapped()), 8)

    # --- printed order

    def test_entwined_tooth_and_nail_searches_then_puts_what_it_found(self):
        for modes in ([0, 1], [1, 0]):
            with self.subTest(modes=modes):
                self.new([{"hand": ["Tooth and Nail", "Grizzly Bears"], "battlefield": ["Forest"] * 9,
                           "library": ["Craw Wurm", "Forest", "Hill Giant", "Forest"]}, {}])
                self.main()
                self.cast_spell("Tooth and Nail", modes=modes, entwine=True,mana=self.mine("Forest"), spend={'G': 9})
                self.assertEqual(self.untapped(), [])
                asked = self.resolve([["Craw Wurm", "Hill Giant"], ["Craw Wurm", "Grizzly Bears"]])
                self.assertEqual([(p, names) for p, _, names in asked],
                                 [(0, ["Craw Wurm", "Hill Giant"]), (0, ["Craw Wurm", "Grizzly Bears", "Hill Giant"])],
                                 "the search first, then the hand with the found cards")
                self.assertEqual(self.nonland(), ["Craw Wurm", "Grizzly Bears"])
                self.assertEqual(self.hand_names(), ["Hill Giant"])

    def test_unpaid_tooth_and_nail_does_only_its_mode(self):
        self.new([{"hand": ["Tooth and Nail", "Grizzly Bears"], "battlefield": ["Forest"] * 9,
                   "library": ["Craw Wurm", "Forest"]}, {}])
        self.main()
        self.cast_spell("Tooth and Nail", entwine=False, modes=[1],mana=self.mine("Forest")[:7], spend={'G': 7})
        self.assertEqual(len(self.untapped()), 2)
        self.assertEqual([names for _, _, names in self.resolve([["Grizzly Bears"]])], [["Grizzly Bears"]], "no search")
        self.assertEqual(self.nonland(), ["Grizzly Bears"])

    def test_entwined_rude_awakening_untaps_and_animates_until_end_of_turn(self):
        self.new([{"hand": ["Rude Awakening"], "battlefield": ["Forest"] * 8}, {}])
        self.main()
        self.cast_spell("Rude Awakening",modes=[0,1],entwine=True,mana=self.mine("Forest"), spend={'G': 8})
        self.assertEqual(self.untapped(), [], "{4}{G} + {2}{G}")
        self.settle()
        forests = self.mine("Forest")
        self.assertEqual(len(self.untapped()), 8)
        for f in forests:
            self.assertTrue({"Creature", "Land"} <= set(self.obj(f)["types"]), self.obj(f))
            self.assertEqual(self.pt(f), (2, 2))
        self.engine.act({"kind":"activate","source":forests[0],"ability":0,"targets":[]})
        self.assertEqual(self.state()["players"][0]["pool"]["G"],1)
        self.advance(lambda s: s["turn"] == 2 and s["asked"]["kind"] == "priority")
        self.assertNotIn("Creature", self.obj(forests[0])["types"], "until end of turn")

    def test_unpaid_rude_awakening_animates_without_untapping(self):
        self.new([{"hand": ["Rude Awakening"], "battlefield": ["Forest"] * 8}, {}])
        self.main()
        self.cast_spell("Rude Awakening",entwine=False,modes=[1],mana=self.mine("Forest")[:5], spend={'G': 5})
        self.settle()
        self.assertEqual(len(self.untapped()), 3)
        self.assertTrue(all("Creature" in self.obj(f)["types"] for f in self.mine("Forest")))

    def test_entwined_goblin_war_party_pumps_and_hastes_its_own_tokens(self):
        self.new([{"hand": ["Goblin War Party"], "battlefield": ["Mountain"] * 7}, {}])
        self.main()
        self.cast_spell("Goblin War Party",modes=[1,0],entwine=True,mana=self.mine("Mountain"), spend={'R': 7})
        self.assertEqual(self.untapped(), [])
        self.settle()
        goblins = self.mine("Goblin Token")
        self.assertEqual(len(goblins), 3)
        self.assertEqual({self.pt(g) for g in goblins}, {(2, 2)}, "the tokens first, then +1/+1 and haste")
        self.attack(goblins[0], {"player": 1})
        self.after_combat()
        self.assertEqual(self.life(1), 18)

    def test_unpaid_goblin_war_party_tokens_are_not_pumped(self):
        self.new([{"hand": ["Goblin War Party"], "battlefield": ["Mountain"] * 7}, {}])
        self.main()
        self.cast_spell("Goblin War Party",entwine=False,modes=[0],mana=self.mine("Mountain")[:4], spend={'R': 4})
        self.settle()
        self.assertEqual(len(self.untapped_named("Mountain")), 3)
        self.assertEqual(sorted(self.pt(g) for g in self.mine("Goblin Token")), [(1, 1)] * 3)

    def test_kayas_guile_entwined_exiles_the_creature_it_made_them_sacrifice(self):
        """Ruling 112a1544: the sacrificed creature is exiled with the graveyards."""
        for modes in ([0, 1, 2, 3], [3, 2, 1, 0]):
            with self.subTest(modes=modes):
                self.new([{"hand": ["Kaya's Guile"], "battlefield": ["Plains", "Swamp"] + ["Plains"] * 4, "graveyard": ["Plains"]},
                          {"battlefield": ["Grizzly Bears", "Hill Giant"], "graveyard": ["Forest"]}])
                self.main()
                self.cast_spell("Kaya's Guile",modes=modes,entwine=True,mana=self.mine("Swamp")+self.mine("Plains"), spend={"B": 1, "W": 5})
                self.assertEqual(self.untapped(), [], "{1}{W}{B} + {3}")
                asked = self.resolve([["Grizzly Bears"]])
                self.assertEqual(asked, [(1, "Sacrifice", ["Grizzly Bears", "Hill Giant"])], "its opponent chooses")
                self.assertEqual(self.graveyard(1), [])
                self.assertEqual(self.names(self.state()["exile"]), ["Forest", "Grizzly Bears"])
                self.assertEqual(self.graveyard(0), ["Kaya's Guile", "Plains"], "only opponents' graveyards")
                self.assertEqual((self.nonland(), self.nonland(1)), (["Spirit Token"], ["Hill Giant"]))
                self.assertEqual(self.life(0), 24)

    def test_kayas_guile_unpaid_does_its_two_modes(self):
        self.new([{"hand": ["Kaya's Guile"], "battlefield": ["Plains", "Swamp"] + ["Plains"] * 4},
                  {"battlefield": ["Grizzly Bears"], "graveyard": ["Forest"]}])
        self.main()
        self.cast_spell("Kaya's Guile",entwine=False,modes=[2,3],mana=self.mine("Swamp")+self.mine("Plains")[:2], spend={"B": 1, "W": 2})
        self.assertEqual(len(self.untapped()), 3)
        self.assertEqual(self.resolve(), [])
        self.assertEqual((self.nonland(), self.life(0), self.nonland(1)), (["Spirit Token"], 24, ["Grizzly Bears"]))
        self.assertEqual(self.graveyard(1), ["Forest"])

    def test_entwined_grab_the_reins_may_sacrifice_the_creature_it_stole(self):
        """Rulings a8cd5c54 and 702deb4d: the damage is targeted on announcement, the sacrifice
        chosen on resolution, after the steal."""
        self.new([{"hand": ["Grab the Reins"], "battlefield": ["Mountain"] * 7 + ["Grizzly Bears"]}, {"battlefield": ["Hill Giant"]}])
        self.main()
        giant = self.one("Hill Giant")
        self.cast_spell("Grab the Reins", [[{"object": giant}], [{"player": 1}]], modes=[1,0],entwine=True,mana=self.mine("Mountain"), spend={'R': 7})
        asked = self.resolve([["Hill Giant"]])
        self.assertEqual(asked, [(0, "Sacrifice", ["Grizzly Bears", "Hill Giant"])])
        self.assertEqual(self.life(1), 17, "the Giant's power")
        self.assertEqual(self.graveyard(1), ["Hill Giant"], "its owner's graveyard")
        self.assertEqual(self.nonland(), ["Grizzly Bears"])

    def test_entwined_dreams_grip_taps_then_untaps_the_same_permanent(self):
        for modes in ([0, 1], [1, 0]):
            with self.subTest(modes=modes):
                self.new([{"hand": ["Dream's Grip"], "battlefield": ["Island"] * 2}, {"battlefield": ["Hill Giant"]}])
                self.main()
                giant = self.one("Hill Giant")
                self.cast_spell("Dream's Grip", [[{"object": giant}], [{"object": giant}]], modes=modes,entwine=True,mana=self.mine("Island"), spend={'U': 2})
                self.assertEqual(self.untapped(), [], "{U} + {1}")
                self.settle()
                self.assertFalse(self.obj(giant).get("tapped"), "tapped, then untapped (702.42b)")
                self.assertEqual(self.graveyard(0), ["Dream's Grip"])

    def test_entwined_mirage_mockery_copies_the_artifact_creature_first(self):
        """Ruling f3566b51: the artifact creature's copy enters first, then the other."""
        self.new([{"hand": ["Mirage Mockery"], "battlefield": ["Island"] * 6 + ["Grizzly Bears", "Ornithopter"]}, {}])
        self.main()
        bears, bird = self.one("Grizzly Bears"), self.one("Ornithopter")
        self.cast_spell("Mirage Mockery", [[{"object":bird}],[{"object":bears}]],modes=[1,0],entwine=True,mana=self.mine("Island"), spend={'U': 6})
        self.settle()
        made = [self.obj(e["token"])["name"] for e in self.events("created")]
        self.assertEqual(made, ["Ornithopter", "Grizzly Bears"])
        self.assertEqual(self.nonland(), ["Grizzly Bears", "Grizzly Bears", "Ornithopter", "Ornithopter"])

    # --- targets

    def test_entwine_requires_every_modes_targets(self):
        self.new([{"hand":["Barbed Lightning","Gravedig"],"battlefield":["Mountain"]*5+["Swamp"]*4},{}])
        self.main()
        self.reject(0,self.spell_command("Barbed Lightning",[[],[{"player":1}]],[0,1],self.mine("Mountain"),entwine=True, spend={'R': 5}))
        self.reject(0,self.spell_command("Gravedig",[[{"player":0}],[]],[0,1],self.mine("Swamp"),entwine=True, spend={'B': 4}))
        self.cast_spell("Barbed Lightning",[[{"player":1}]],[1],self.mine("Mountain")[:3],entwine=False, spend={'R': 3})
        self.settle()
        self.assertEqual(self.life(1),17)
        self.cast_spell("Gravedig",[[{"player":0}]],modes=[0],mana=self.mine("Swamp")[:2],entwine=False, spend={'B': 2})
        self.settle()
        self.assertEqual([self.pt(o) for o in self.mine("Zombie Token")],[(2,2)])

    def test_entwined_barbed_lightning_kills_and_burns(self):
        self.new([{"hand": ["Barbed Lightning"], "battlefield": ["Mountain"] * 5}, {"battlefield": ["Hill Giant"]}])
        self.main()
        giant = self.one("Hill Giant")
        self.cast_spell("Barbed Lightning", [[{"object":giant}],[{"player":1}]],modes=[0,1],entwine=True,mana=self.mine("Mountain"), spend={'R': 5})
        self.assertEqual(self.untapped(), [], "{2}{R} + {2}")
        self.settle()
        self.assertEqual((self.mine("Hill Giant", 1), self.life(1)), ([], 17))

    def test_entwined_barbed_lightning_resolves_its_legal_part(self):
        self.new([{"hand": ["Barbed Lightning"], "battlefield": ["Mountain"] * 5},
                  {"hand": ["Unsummon"], "battlefield": ["Island", "Hill Giant"]}])
        self.main()
        giant = self.one("Hill Giant")
        self.cast_spell("Barbed Lightning", [[{"object":giant}],[{"player":1}]],modes=[0,1],entwine=True,mana=self.mine("Mountain"), spend={'R': 5})
        self.engine.act({"kind": "pass"})
        self.cast_spell("Unsummon",[[{"object":giant}]],mana=self.mine("Island",1), spend={'U': 1})
        self.settle()
        self.assertEqual(self.life(1), 17, "the player part still resolves (608.2b)")
        self.assertEqual(self.graveyard(0), ["Barbed Lightning"])

    def test_entwined_dreams_grip_with_every_target_gone_does_not_resolve(self):
        self.new([{"hand": ["Dream's Grip"], "battlefield": ["Island"] * 2},
                  {"hand": ["Unsummon"], "battlefield": ["Island", "Hill Giant"]}])
        self.main()
        giant = self.one("Hill Giant")
        self.cast_spell("Dream's Grip",[[{"object":giant}],[{"object":giant}]],modes=[0,1],entwine=True,mana=self.mine("Island"), spend={'U': 2})
        self.engine.act({"kind": "pass"})
        self.cast_spell("Unsummon",[[{"object":giant}]],mana=self.mine("Island",1), spend={'U': 1})
        self.settle()
        self.assertEqual(self.graveyard(0), ["Dream's Grip"])
        self.assertEqual(self.hand_names(1), ["Hill Giant"])
        self.assertNotIn(self.state()["players"][0]["graveyard"][0], [e.get("spell") for e in self.events("resolved")])

    def test_twincast_copies_both_modes_without_paying_entwine(self):
        self.new([{"hand": ["Barbed Lightning"], "battlefield": ["Mountain"] * 5 + ["Grizzly Bears"]},
                  {"hand": ["Twincast"], "battlefield": ["Island", "Island", "Hill Giant"]}])
        self.main()
        giant, bears = self.one("Hill Giant"), self.one("Grizzly Bears")
        spell = self.cast_spell("Barbed Lightning", [[{"object":giant}],[{"player":1}]],modes=[0,1],entwine=True,mana=self.mine("Mountain"), spend={'R': 5})
        self.engine.act({"kind": "pass"})
        self.cast_spell("Twincast",[[{"object":spell}]],mana=self.mine("Island",1), spend={'U': 2})
        self.assertEqual(self.untapped(1), [giant], "Twincast's {U}{U} only")
        self.engine.act({"kind": "pass"})
        self.engine.act({"kind": "pass"})
        a = self.asked()
        self.assertEqual((a["kind"], a["player"], len(a["slots"])), ("retarget", 1, 2), a)
        self.engine.act({"kind": "choose", "answer": {"targets": [[{"object": bears}], [{"player": 0}]]}})
        self.assertEqual(self.untapped(1), [giant], "nothing paid for the copy")
        self.settle()
        self.assertEqual((self.mine("Grizzly Bears"), self.life(0)), ([], 17), "the copy did both")
        self.assertEqual((self.mine("Hill Giant", 1), self.life(1)), ([], 17), "the original did both")

    def test_twincast_copy_of_an_unpaid_spell_keeps_its_one_mode(self):
        self.new([{"hand": ["Barbed Lightning"], "battlefield": ["Mountain"] * 5 + ["Grizzly Bears"]},
                  {"hand": ["Twincast"], "battlefield": ["Island", "Island", "Hill Giant"]}])
        self.main()
        spell = self.cast_spell("Barbed Lightning",[[{"player":1}]],entwine=False,modes=[1],mana=self.mine("Mountain")[:3], spend={'R': 3})
        self.engine.act({"kind": "pass"})
        self.cast_spell("Twincast",[[{"object":spell}]],mana=self.mine("Island",1), spend={'U': 2})
        self.engine.act({"kind": "pass"})
        self.engine.act({"kind": "pass"})
        a = self.asked()
        self.assertEqual((a["kind"], len(a["slots"])), ("retarget", 1), a)
        self.engine.act({"kind": "choose", "answer": {"targets": [[{"player": 0}]]}})
        self.settle()
        self.assertEqual((self.life(0), self.life(1)), (17, 17))
        self.assertEqual(len(self.mine("Grizzly Bears")), 1)

    # --- composition with cost changes and a free cast

    def test_cost_changes_apply_to_the_entwined_total(self):
        """Barbed Lightning entwined is {4}{R}: Goblin Electromancer takes {1} off, Thalia adds {1}."""
        for card, mine, lands, paid in (("Goblin Electromancer", True, 3, False), ("Goblin Electromancer", True, 4, True),
                                        ("Thalia, Guardian of Thraben", False, 5, False), ("Thalia, Guardian of Thraben", False, 6, True)):
            with self.subTest(card=card, lands=lands):
                self.new([{"hand": ["Barbed Lightning"], "battlefield": ["Mountain"] * lands + ([card] if mine else [])},
                          {"battlefield": ["Hill Giant"] + ([] if mine else [card])}])
                self.main()
                if not paid:
                    self.reject(0,self.spell_command("Barbed Lightning",[[{"object":self.one("Hill Giant")}],[{"player":1}]],[0,1],self.mine("Mountain"),entwine=True, spend={'R': lands}))
                if paid:
                    self.cast_spell("Barbed Lightning",[[{"object":self.one("Hill Giant")}],[{"player":1}]],modes=[0,1],entwine=True,mana=self.mine("Mountain"), spend={'R': lands})
                    self.assertEqual(self.untapped_named("Mountain"), [])
                    self.settle()
                    self.assertEqual(self.life(1), 17)

    def test_the_free_rebound_cast_may_pay_entwine(self):
        """Cast Through Time: from exile without paying its mana cost, entwine for {2} alone."""
        for paid in (True, False):
            with self.subTest(paid=paid):
                self.new([{"hand": ["Barbed Lightning"], "battlefield": ["Mountain"] * 5 + ["Cast Through Time"]},
                          {"battlefield": ["Hill Giant", "Craw Wurm"]}])
                self.main()
                self.cast_spell("Barbed Lightning",[[{"object":self.one("Hill Giant")}],[{"player":1}]],modes=[0,1],entwine=True,mana=self.mine("Mountain"), spend={'R': 5})
                self.settle()
                self.assertEqual((self.life(1), self.names(self.state()["exile"])), (17, ["Barbed Lightning"]))
                for _ in range(120):
                    s = self.state()
                    if s["turn"] == 3 and s["asked"]["kind"] == "cast":
                        break
                    self.assertLess(s["turn"], 4)
                    if s["asked"]["kind"] == "priority":
                        self.engine.act({"kind": "pass"})
                    else:
                        self.step()
                self.assertEqual(self.asked()["cards"],self.state()["exile"])
                wurm = self.one("Craw Wurm")
                if paid:
                    self.cast_spell("Barbed Lightning",[[{"object":wurm}],[{"player":1}]],modes=[0,1],entwine=True,mana=self.mine("Mountain")[:2], spend={'R': 2})
                    self.assertEqual(len(self.untapped_named("Mountain")), 3)
                else:
                    self.cast_spell("Barbed Lightning", [[{"player": 1}]], entwine=False, modes=[1], spend={})
                    self.assertEqual(len(self.untapped_named("Mountain")), 5)
                self.settle()
                self.assertEqual(self.life(1), 14)
                self.assertEqual(self.obj(wurm).get("damage", 0), 3 if paid else 0)
                self.assertEqual(self.graveyard(0), ["Barbed Lightning"], "cast from exile, so no second rebound")


if __name__ == "__main__":
    unittest.main()
