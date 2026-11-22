"""Changing the targets of an existing spell or
ability through the real audited client with full pinned cards.

Required single changes (Shunt, Ricochet Trap, Imp's Mischief, Untimely Malfunction, Reroute):
the new target is chosen as the changer resolves, must differ and be legal for the ORIGINAL
spell or ability, and the target is unchanged with no question when nothing else is legal
(115.7a; rulings 4d902426, 6d2f95c4). The spell itself is never offered (115.5, e1a33b99); the
resolving changer may be (d5bef373). "A single target" counts occurrences (115.9a; f66f074c).
Optional new targets (Redirect, Wild Ricochet) keep any target, even an illegal one, change
any subset, and check the final set (115.7d-e; 03d10e5a). The deflected spell's controller is
still the one targeting (6a1fa067, 702.21a)."""
import unittest
import test_toxic_review_client as base
import client_support as explicit
from client_support import ClientTestCase

HELPERS = {k: v for k, v in vars(base.ToxicReviewClientTests).items() if not k.startswith("test_") and not k.startswith("__")}


def P(n):
    return {"player": n}


def O(i):
    return {"object": i}


class TargetChangeReviewClientTests(ClientTestCase):
    locals().update(HELPERS)

    # --- setup and acting

    def state(self):
        return self.engine.state

    def their_turn(self, mine, theirs):
        """P1's precombat main, P1 to act, empty stack."""
        self.new([{"library": ["Island"] * 10, **mine}, {"library": ["Island"] * 10, **theirs}])
        self.advance(lambda s: s["active"] == 1 and s["part"] == "PrecombatMain" and s["asked"]["kind"] == "priority"
                     and s["asked"]["player"] == 1 and not s["stack"])

    def named(self, name, controller=None):
        return [o for o in self.state()["battlefield"] if self.obj(o)["name"] == name and (controller is None or self.obj(o)["controller"] == controller)]

    def the(self, name, controller=None):
        found = self.named(name, controller)
        self.assertEqual(len(found), 1, (name, found))
        return found[0]

    validate_action = explicit.ClientTestCase.validate_action

    def fund(self, name, count):
        """The fixture authors a named set of untapped basic lands and a fixed count."""
        actor = self.asked().get("acting", self.asked()["player"])
        sources = [o for o in self.named(name, actor) if not self.obj(o)["tapped"]][:count]
        self.assertEqual(len(sources), count)
        return sources

    def cast_action(self, name, targets=(), *, mana, spend=None, modes=None, alternative=None):
        actor = self.asked().get("acting", self.asked()["player"])
        cards = [o for o in self.hand(actor) if self.obj(o)["name"] == name]
        self.assertEqual(len(cards), 1, (name, cards))
        move = {"kind": "cast", "card": cards[0], "targets": [list(g) for g in targets],
                "plan": [{"source": o, "ability": 0} for o in mana],
                **({"mana": spend} if spend is not None else {})}
        if modes is not None:
            move["modes"] = modes
        if alternative:
            move["cost"] = {"alternative": 0}
        return move

    def cast(self, name, targets=(), *, mana, spend=None, modes=None, alternative=None):
        self.engine.act(self.cast_action(name, targets, mana=mana, spend=spend, modes=modes, alternative=alternative))
        return next(o for o in self.state()["stack"] if self.obj(o)["name"] == name)

    def activate(self, source, targets=()):
        # Prodigal Pyromancer's sole printed activated ability is its tap-to-ping.
        self.engine.act({"kind": "activate", "source": source, "ability": 0, "plan": [],
                         "targets": [list(g) for g in targets]})
        return self.state()["stack"][-1]

    def pass_(self):
        self.engine.act({"kind": "pass"})

    def resolve_top(self):
        """Both players pass once: the top of the stack resolves."""
        self.pass_()
        self.pass_()

    def drain(self):
        for _ in range(40):
            a = self.asked()
            if a["kind"] != "priority" or not self.state()["stack"]:
                return a
            self.pass_()
        self.fail("the stack did not settle")

    def hand(self, player):
        return self.view(player)["players"][player]["hand"]

    def targets_of(self, obj):
        return self.obj(obj)["targets"]

    def question(self, obj, original, policy, player=0):
        a = self.asked()
        self.assertEqual((a["kind"], a["player"], a["object"], a["original"], a["policy"]),
                         ("change_targets", player, obj, original, policy), a)
        self.assertEqual([(s["min"], s["max"]) for s in a["slots"]], [(len(g), len(g)) for g in original])
        return a

    def answer(self, targets):
        self.engine.act({"kind": "choose", "answer": {"targets": [list(g) for g in targets]}})

    def bolt_at_me(self, mine, theirs=(), their_lands=1):
        """P1 Bolts P0; P0 holds the response mana and cards in `mine`."""
        self.their_turn(mine, {"battlefield": ["Mountain"] * their_lands + list(theirs), "hand": ["Lightning Bolt"]})
        bolt = self.cast("Lightning Bolt", [[P(0)]], mana=self.fund("Mountain", 1), spend={'R': 1})
        self.pass_()
        return bolt

    # --- Shunt: the required question

    def test_shunt_asks_once_for_another_legal_target_of_the_original_bolt(self):
        bolt = self.bolt_at_me({"hand": ["Shunt"], "battlefield": ["Mountain"] * 3 + ["Grizzly Bears"]}, theirs=["Grizzly Bears"])
        mine, theirs = self.the("Grizzly Bears", 0), self.the("Grizzly Bears", 1)
        shunt = self.cast("Shunt", [[O(bolt)]], mana=self.fund("Mountain", 3), spend={'R': 3})
        self.resolve_top()
        a = self.question(bolt, [[P(0)]], "required_single")
        among = a["slots"][0]["among"]
        self.assertEqual(sorted(map(str, among)), sorted(map(str, [P(1), O(mine), O(theirs)])), "not the current target, not Bolt")
        self.assertEqual(self.targets_of(bolt), [[P(0)]], "nothing changes before the answer")
        other = self.view(1)["asked"]
        self.assertEqual((other["kind"], other["player"]), ("change_targets", 0))
        for answer in ([[P(0)]], [[O(bolt)]], [[O(shunt)]], [[]], [[P(1), O(theirs)]], [[P(1)], [P(1)]], [], [[O(999)]]):
            with self.subTest(answer=answer):
                self.reject(0, {"kind": "choose", "answer": {"targets": answer}})
        for answer in ("no", "yes", {"objects": [theirs]}, {"alternative": 0}):
            with self.subTest(answer=answer):
                self.reject(0, {"kind": "choose", "answer": answer})
        self.reject(0, {"kind": "pass"})
        self.reject(1, {"kind": "choose", "answer": {"targets": [[P(1)]]}})
        self.question(bolt, [[P(0)]], "required_single")
        self.answer([[O(theirs)]])
        self.assertEqual(self.targets_of(bolt), [[O(theirs)]])
        self.assertNotIn(shunt, self.state()["stack"])
        self.drain()
        self.assertEqual((self.life(0), self.named("Grizzly Bears", 1), self.named("Grizzly Bears", 0)), (20, [], [mine]))

    def test_a_bolt_whose_target_died_is_still_single_and_must_change(self):
        """192d65ca: Shunt does not check the current target's legality, only that there is one."""
        self.their_turn({"hand": ["Shunt", "Shock"], "battlefield": ["Mountain"] * 4 + ["Grizzly Bears"]},
                        {"battlefield": ["Mountain"], "hand": ["Lightning Bolt"]})
        bears = self.the("Grizzly Bears")
        bolt = self.cast("Lightning Bolt", [[O(bears)]], mana=self.fund("Mountain", 1), spend={'R': 1})
        self.pass_()
        self.cast("Shock", [[O(bears)]], mana=self.fund("Mountain", 1), spend={'R': 1})
        self.resolve_top()
        self.assertEqual(self.named("Grizzly Bears"), [])
        self.assertEqual(self.asked()["player"], 1, "the active player receives priority")
        self.pass_()
        self.cast("Shunt", [[O(bolt)]], mana=self.fund("Mountain", 3), spend={'R': 3})
        self.resolve_top()
        a = self.question(bolt, [[O(bears)]], "required_single")
        self.assertEqual(sorted(map(str, a["slots"][0]["among"])), sorted(map(str, [P(0), P(1)])))
        self.answer([[P(1)]])
        self.drain()
        self.assertEqual((self.life(0), self.life(1)), (20, 17))

    def test_no_other_legal_target_leaves_the_aura_unchanged_without_a_question(self):
        """Pacifism is a single-target Aura spell (enchant creature); with one creature there is
        nothing else to choose."""
        self.their_turn({"hand": ["Shunt"], "battlefield": ["Mountain"] * 3 + ["Grizzly Bears"]},
                        {"battlefield": ["Plains"] * 2, "hand": ["Pacifism"]})
        bears = self.the("Grizzly Bears")
        pacifism = self.cast("Pacifism", [[O(bears)]], mana=self.fund("Plains", 2), spend={'W': 2})
        self.pass_()
        self.cast("Shunt", [[O(pacifism)]], mana=self.fund("Mountain", 3), spend={'R': 3})
        self.resolve_top()
        a = self.asked()
        self.assertEqual((a["kind"], self.targets_of(pacifism)), ("priority", [[O(bears)]]))
        self.drain()
        self.assertEqual(self.obj(self.the("Pacifism"))["attached_to"], O(bears))

    def test_the_aura_moves_when_another_creature_can_be_enchanted(self):
        self.their_turn({"hand": ["Shunt"], "battlefield": ["Mountain"] * 3 + ["Grizzly Bears"]},
                        {"battlefield": ["Plains"] * 2 + ["Grizzly Bears"], "hand": ["Pacifism"]})
        mine, theirs = self.the("Grizzly Bears", 0), self.the("Grizzly Bears", 1)
        pacifism = self.cast("Pacifism", [[O(mine)]], mana=self.fund("Plains", 2), spend={'W': 2})
        self.pass_()
        self.cast("Shunt", [[O(pacifism)]], mana=self.fund("Mountain", 3), spend={'R': 3})
        self.resolve_top()
        a = self.question(pacifism, [[O(mine)]], "required_single")
        self.assertEqual(a["slots"][0]["among"], [O(theirs)])
        self.answer([[O(theirs)]])
        self.drain()
        self.assertEqual(self.obj(self.the("Pacifism"))["attached_to"], O(theirs))

    def test_a_spell_with_several_target_words_is_not_single(self):
        """Seeds of Strength naming the same creature three times has three targets (f66f074c)."""
        self.their_turn({"hand": ["Shunt", "Redirect"], "battlefield": ["Mountain"] * 3 + ["Island"] * 2},
                        {"battlefield": ["Forest", "Plains", "Grizzly Bears"], "hand": ["Seeds of Strength"]})
        bears = self.the("Grizzly Bears")
        seeds = self.cast("Seeds of Strength", [[O(bears)]] * 3, mana=self.fund("Forest", 1) + self.fund("Plains", 1), spend={'G': 1, 'W': 1})
        self.pass_()
        self.validate_action(self.cast_action("Shunt", [[O(seeds)]], mana=self.fund("Mountain", 3), spend={'R': 3}), accepted=False)
        self.validate_action(self.cast_action("Redirect", [[O(seeds)]], mana=self.fund("Island", 2), spend={'U': 2}))

    # --- the spell itself, the changer, and a counterspell (rulings 1a5ba918, d5bef373)

    def test_a_counterspell_redirected_to_the_resolving_trap_fizzles(self):
        self.their_turn({"hand": ["Lightning Bolt", "Ricochet Trap"], "battlefield": ["Mountain"] * 2},
                        {"battlefield": ["Mountain"] + ["Island"] * 2, "hand": ["Shock", "Counterspell"]})
        shock = self.cast("Shock", [[P(0)]], mana=self.fund("Mountain", 1), spend={'R': 1})
        self.pass_()
        self.validate_action(self.cast_action("Ricochet Trap", [[O(shock)]], mana=self.fund("Mountain", 1), alternative=True, spend={'R': 1}), accepted=False)
        bolt = self.cast("Lightning Bolt", [[P(1)]], mana=self.fund("Mountain", 1), spend={'R': 1})
        self.pass_()
        counter = self.cast("Counterspell", [[O(bolt)]], mana=self.fund("Island", 2), spend={'U': 2})
        self.pass_()
        trap = self.cast("Ricochet Trap", [[O(counter)]], alternative=True, mana=self.fund("Mountain", 1), spend={'R': 1})
        self.resolve_top()
        a = self.question(counter, [[O(bolt)]], "required_single")
        self.assertEqual(sorted(map(str, a["slots"][0]["among"])), sorted(map(str, [O(shock), O(trap)])),
                         "neither Bolt (the current target) nor Counterspell itself")
        self.reject(0, {"kind": "choose", "answer": {"targets": [[O(counter)]]}})
        self.answer([[O(trap)]])
        self.drain()
        self.assertEqual((self.life(0), self.life(1)), (18, 17), "Counterspell fizzled; Bolt and Shock resolved")
        self.assertIn("Counterspell", [self.obj(o)["name"] for o in self.state()["players"][1]["graveyard"]])

    def test_the_trap_alternative_cost_needs_an_opponents_blue_spell_this_turn(self):
        """P0's own blue spell does not enable {R}; P1's Unsummon does, for any target spell
        (4a6d7bb2), and only in that turn."""
        self.their_turn({"hand": ["Ricochet Trap", "Unsummon"], "battlefield": ["Mountain", "Island"]},
                        {"battlefield": ["Mountain", "Island", "Grizzly Bears"], "hand": ["Shock", "Unsummon"]})
        bears = self.the("Grizzly Bears")
        shock = self.cast("Shock", [[P(0)]], mana=self.fund("Mountain", 1), spend={'R': 1})
        self.pass_()
        self.validate_action(self.cast_action("Ricochet Trap", [[O(shock)]], mana=self.fund("Mountain", 1), alternative=True, spend={'R': 1}), accepted=False)
        self.cast("Unsummon", [[O(bears)]], mana=self.fund("Island", 1), spend={'U': 1})
        self.validate_action(self.cast_action("Ricochet Trap", [[O(shock)]], mana=self.fund("Mountain", 1), alternative=True, spend={'R': 1}), accepted=False)
        self.pass_()
        self.cast("Unsummon", [[O(bears)]], mana=self.fund("Island", 1), spend={'U': 1})
        self.pass_()
        trap = self.cast("Ricochet Trap", [[O(shock)]], alternative=True, mana=self.fund("Mountain", 1), spend={'R': 1})
        self.resolve_top()
        a = self.question(shock, [[P(0)]], "required_single")
        self.assertIn(P(1), a["slots"][0]["among"])
        self.answer([[P(1)]])
        self.drain()
        self.assertEqual((self.life(0), self.life(1)), (20, 18))
        self.assertNotIn(trap, self.state()["stack"])

    # --- the original controller targets: ward and becomes-the-target triggers

    def test_ward_belongs_to_the_bolts_controller_not_the_changer(self):
        for side, warded in ((1, False), (0, True)):
            with self.subTest(onto=side):
                self.bolt_at_me({"hand": ["Shunt"], "battlefield": ["Mountain"] * 3 + ["Tolarian Terror"]}, theirs=["Tolarian Terror"])
                terror = self.the("Tolarian Terror", side)
                bolt = next(o for o in self.state()["stack"])
                self.cast("Shunt", [[O(bolt)]], mana=self.fund("Mountain", 3), spend={'R': 3})
                self.resolve_top()
                self.question(bolt, [[P(0)]], "required_single")
                self.answer([[O(terror)]])
                names = [self.obj(o)["name"] for o in self.state()["stack"]]
                self.assertEqual(names, ["Lightning Bolt", "Tolarian Terror"] if warded else ["Lightning Bolt"], names)
                if warded:
                    self.assertEqual(self.obj(self.state()["stack"][-1])["controller"], 0)
                self.drain()
                self.assertEqual(self.obj(terror)["damage"], 0 if warded else 3, "P1 cannot pay ward {2}")
                self.assertEqual(self.life(0), 20)

    def test_cactarantula_triggers_for_its_controller_when_an_opponents_spell_is_moved_onto_it(self):
        for side, triggers in ((0, True), (1, False)):
            with self.subTest(onto=side):
                bolt = self.bolt_at_me({"hand": ["Shunt"], "battlefield": ["Mountain"] * 3 + ["Cactarantula"]}, theirs=["Cactarantula"])
                spider = self.the("Cactarantula", side)
                self.cast("Shunt", [[O(bolt)]], mana=self.fund("Mountain", 3), spend={'R': 3})
                self.resolve_top()
                self.answer([[O(spider)]])
                names = [self.obj(o)["name"] for o in self.state()["stack"]]
                self.assertEqual(names, ["Lightning Bolt", "Cactarantula"] if triggers else ["Lightning Bolt"])
                hand = len(self.hand(0))
                a = self.drain()
                if triggers:
                    self.assertEqual((a["kind"], a["player"]), ("may", 0), a)
                    self.engine.act({"kind": "choose", "answer": "yes"})
                    self.drain()
                    self.assertEqual(len(self.hand(0)), hand + 1)

    def test_hexproof_follows_the_original_ability_controller_and_the_departed_source_still_deals_damage(self):
        """P1's Prodigal Pyromancer pings P0; P0 kills it, then Untimely Malfunction (mode 2)
        changes the ability's target. P0's hexproof Scout is not a legal target of P1's ability;
        P1's own Scout is. The ability resolves with its source's last known information."""
        self.their_turn({"hand": ["Untimely Malfunction", "Shock"], "battlefield": ["Mountain"] * 3 + ["Gladecover Scout"]},
                        {"battlefield": ["Prodigal Pyromancer", "Gladecover Scout"]})
        pyro = self.the("Prodigal Pyromancer")
        mine, theirs = self.the("Gladecover Scout", 0), self.the("Gladecover Scout", 1)
        ping = self.activate(pyro, [[P(0)]])
        self.pass_()
        self.cast("Untimely Malfunction", [[O(ping)]], modes=[1], mana=self.fund("Mountain", 2), spend={'R': 2})
        self.cast("Shock", [[O(pyro)]], mana=self.fund("Mountain", 1), spend={'R': 1})
        self.resolve_top()
        self.assertEqual(self.named("Prodigal Pyromancer"), [], "the source has died")
        self.resolve_top()
        a = self.question(ping, [[P(0)]], "required_single")
        among = a["slots"][0]["among"]
        self.assertIn(O(theirs), among)
        self.assertNotIn(O(mine), among, "hexproof against P1, the ability's controller")
        self.assertIn(P(1), among)
        self.answer([[O(theirs)]])
        self.drain()
        self.assertEqual(self.named("Gladecover Scout", 1), [])
        self.assertEqual(self.named("Gladecover Scout", 0), [mine])

    def test_reroute_changes_only_activated_abilities_and_draws(self):
        self.their_turn({"hand": ["Reroute", "Shunt"], "battlefield": ["Mountain"] * 3},
                        {"battlefield": ["Prodigal Pyromancer", "Mountain"], "hand": ["Shock"]})
        ping = self.activate(self.the("Prodigal Pyromancer"), [[P(0)]])
        self.pass_()
        self.validate_action(self.cast_action("Reroute", [[O(ping)]], mana=self.fund("Mountain", 2), spend={'R': 2}))
        self.validate_action(self.cast_action("Shunt", [[O(ping)]], mana=self.fund("Mountain", 3), spend={'R': 3}), accepted=False)
        hand = len(self.hand(0))
        self.cast("Reroute", [[O(ping)]], mana=self.fund("Mountain", 2), spend={'R': 2})
        self.resolve_top()
        self.question(ping, [[P(0)]], "required_single")
        self.answer([[P(1)]])
        self.assertEqual(len(self.hand(0)), hand, "Reroute left the hand and drew one")
        self.drain()
        self.assertEqual((self.life(0), self.life(1)), (20, 19))

    def test_imps_mischief_loses_life_after_the_change_and_when_nothing_changes(self):
        bolt = self.bolt_at_me({"hand": ["Imp's Mischief"], "battlefield": ["Swamp"] * 2})
        self.cast("Imp's Mischief", [[O(bolt)]], mana=self.fund("Swamp", 2), spend={'B': 2})
        self.resolve_top()
        self.answer([[P(1)]])
        self.assertEqual(self.life(0), 19, "Bolt's mana value")
        self.drain()
        self.assertEqual((self.life(0), self.life(1)), (19, 17))
        self.their_turn({"hand": ["Imp's Mischief"], "battlefield": ["Swamp"] * 2 + ["Grizzly Bears"]},
                        {"battlefield": ["Plains"] * 2, "hand": ["Pacifism"]})
        pacifism = self.cast("Pacifism", [[O(self.the("Grizzly Bears"))]], mana=self.fund("Plains", 2), spend={'W': 2})
        self.pass_()
        self.cast("Imp's Mischief", [[O(pacifism)]], mana=self.fund("Swamp", 2), spend={'B': 2})
        self.resolve_top()
        self.assertEqual((self.asked()["kind"], self.life(0)), ("priority", 18), "no question; still loses 2")

    # --- optional new targets

    def arc_trail(self, extra_mine=()):
        """P1's Arc Trail: 2 to P0's Bears, 1 to P0. P0 answers with Redirect."""
        self.their_turn({"hand": ["Redirect", *extra_mine], "battlefield": ["Island"] * 3 + ["Grizzly Bears"]},
                        {"battlefield": ["Mountain"] * 2 + ["Grizzly Bears"], "hand": ["Arc Trail"]})
        self.mine, self.theirs = self.the("Grizzly Bears", 0), self.the("Grizzly Bears", 1)
        self.trail = self.cast("Arc Trail", [[O(self.mine)], [P(0)]], mana=self.fund("Mountain", 2), spend={'R': 2})
        self.pass_()
        self.cast("Redirect", [[O(self.trail)]], mana=self.fund("Island", 2), spend={'U': 2})

    def test_redirect_swaps_arc_trails_targets_and_keeps_its_two_and_one(self):
        self.arc_trail()
        self.resolve_top()
        a = self.question(self.trail, [[O(self.mine)], [P(0)]], "choose_new")
        self.assertEqual(a["slots"][1]["distinct_from"], [0])
        for answer in ([[O(self.mine)], [O(self.mine)]], [[O(self.mine)]], [[O(self.trail)], [P(0)]], [[O(self.mine), P(1)], [P(0)]]):
            with self.subTest(answer=answer):
                self.reject(0, {"kind": "choose", "answer": {"targets": answer}})
        self.answer([[P(0)], [O(self.mine)]])
        self.drain()
        self.assertEqual((self.life(0), self.obj(self.mine)["damage"]), (18, 1), "the 2 follows the first position")

    def test_redirect_may_decline_keep_everything_or_change_part(self):
        self.arc_trail()
        self.resolve_top()
        self.engine.act({"kind": "choose", "answer": "no"})
        self.drain()
        self.assertEqual((self.life(0), self.named("Grizzly Bears", 0)), (19, []))
        self.arc_trail()
        self.resolve_top()
        self.answer([[O(self.theirs)], [P(0)]])
        self.drain()
        self.assertEqual((self.life(0), self.named("Grizzly Bears", 0), self.named("Grizzly Bears", 1)), (19, [self.mine], []))

    def test_redirect_keeps_a_target_that_became_illegal_and_changes_the_other(self):
        """03d10e5a: a target may stay unchanged even when it is illegal."""
        self.arc_trail(extra_mine=["Unsummon"])
        self.cast("Unsummon", [[O(self.mine)]], mana=self.fund("Island", 1), spend={'U': 1})
        self.resolve_top()
        self.assertEqual(self.named("Grizzly Bears", 0), [])
        self.resolve_top()
        a = self.question(self.trail, [[O(self.mine)], [P(0)]], "choose_new")
        self.assertIn(O(self.mine), a["slots"][0]["among"], "the kept illegal original is offered")
        self.reject(0, {"kind": "choose", "answer": {"targets": [[O(self.mine)], [O(self.mine)]]}})
        self.answer([[O(self.mine)], [P(1)]])
        self.drain()
        self.assertEqual((self.life(0), self.life(1)), (20, 19), "only the legal target is dealt damage")

    def test_wild_ricochet_changes_the_original_then_copies_the_changed_spell(self):
        bolt = self.bolt_at_me({"hand": ["Wild Ricochet"], "battlefield": ["Mountain"] * 4 + ["Grizzly Bears"]}, theirs=["Grizzly Bears"])
        theirs = self.the("Grizzly Bears", 1)
        self.cast("Wild Ricochet", [[O(bolt)]], mana=self.fund("Mountain", 4), spend={'R': 4})
        self.resolve_top()
        self.question(bolt, [[P(0)]], "choose_new")
        self.answer([[P(1)]])
        a = self.asked()
        self.assertEqual((a["kind"], a["original"]), ("retarget", [[P(1)]]), "the copy copies the changed spell")
        self.assertNotIn("policy", a)
        self.engine.act({"kind": "choose", "answer": {"targets": [[O(theirs)]]}})
        self.drain()
        self.assertEqual((self.life(0), self.life(1), self.named("Grizzly Bears", 1)), (20, 17, []))

    def my_counterspell_on_their_shock(self, response):
        """P1 Shocks P0; P0 Counterspells the Shock and, holding priority, casts `response`
        targeting that Counterspell."""
        self.their_turn({"hand": ["Counterspell", response], "battlefield": ["Island"] * 4},
                        {"battlefield": ["Mountain"], "hand": ["Shock"]})
        shock = self.cast("Shock", [[P(0)]], mana=self.fund("Mountain", 1), spend={'R': 1})
        self.pass_()
        counter = self.cast("Counterspell", [[O(shock)]], mana=self.fund("Island", 2), spend={'U': 2})
        changer = self.cast(response, [[O(counter)]], mana=self.fund("Island", 2), spend={'U': 2})
        self.resolve_top()
        return shock, counter, changer

    def test_redirect_never_offers_the_spell_itself_as_its_new_target(self):
        """115.5 (e1a33b99, 7cc39730): a spell on the stack is not a legal target for itself. The
        offer must agree with the final-set validator, which refuses it: the audited play.py run
        (seed 119, Wild Ricochet on Deflection) was refused after choosing an offered target."""
        shock, counter, redirect = self.my_counterspell_on_their_shock("Redirect")
        a = self.question(counter, [[O(shock)]], "choose_new")
        self.assertIn(O(redirect), a["slots"][0]["among"], "the resolving Redirect may be chosen")
        self.assertNotIn(O(counter), a["slots"][0]["among"])
        self.reject(0, {"kind": "choose", "answer": {"targets": [[O(counter)]]}})

    def test_a_copy_is_never_offered_as_its_own_new_target(self):
        """Contrast: the copy retarget question shares the validator, which refuses a copy
        targeting itself (115.5), and its offer, built for the copy's own stack object, already
        leaves the copy out."""
        shock, counter, twincast = self.my_counterspell_on_their_shock("Twincast")
        a = self.asked()
        self.assertEqual((a["kind"], a["original"]), ("retarget", [[O(shock)]]), a)
        self.assertNotIn(O(a["copy"]), a["slots"][0]["among"])
        self.reject(0, {"kind": "choose", "answer": {"targets": [[O(a["copy"])]]}})

    def test_twincast_still_asks_the_copy_retarget_question(self):
        bolt = self.bolt_at_me({"hand": ["Twincast"], "battlefield": ["Island"] * 2})
        self.cast("Twincast", [[O(bolt)]], mana=self.fund("Island", 2), spend={'U': 2})
        self.resolve_top()
        a = self.asked()
        self.assertEqual(set(a) - {"player", "request_id"}, {"kind", "copy", "original", "slots"}, a)
        self.assertEqual(a["kind"], "retarget")


if __name__ == "__main__":
    unittest.main()
