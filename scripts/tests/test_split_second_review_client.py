"""Split second through the real audited client,
with full pinned Sudden Shock and Krosan Grip. While a spell with split second is on the stack,
players can't cast other spells or activate abilities that aren't mana abilities (702.61a); mana
abilities and triggers still work (702.61b, 605.1a); instances are redundant (702.61c). Spells and
abilities already on the stack are unaffected, and once the spell leaves the stack players may
act again before the next object resolves (Sudden Shock rulings 1b81ebb4, f767e8d9). Split second
doesn't stop triggers (985d45d8), so a countering trigger can still counter the spell."""
import unittest
import test_toxic_review_client as base
import test_multiplayer_outcome_client as explicit

HELPERS = {k: v for k, v in vars(base.ToxicReviewClientTests).items() if not k.startswith("test_") and not k.startswith("__")}


def command(move):
    """Copy an author-supplied action; no targets or payment choices are filled."""
    return dict(move)


class SplitSecondReviewClientTests(unittest.TestCase):
    locals().update(HELPERS)
    validate_action = explicit.MultiplayerOutcomeClientTests.validate_action

    def new(self, seats):
        HELPERS["new"](self, seats)
        self.responses = {p: [] for p in range(len(seats))}

    def spell(self, player, name, mana, targets, *, spend):
        view = self.view(player)
        card = next(o for o in view["players"][player]["hand"] if view["objects"][str(o)]["name"] == name)
        return {"kind": "cast", "card": card, "targets": targets,
                "plan": [{"source": o, "ability": 0} for o in mana], "mana": spend}

    def assert_responses(self, allowed):
        responses = self.responses[self.asked()["player"]]
        self.assertTrue(set(allowed) <= {name for name, _ in responses})
        for name, action in responses:
            with self.subTest(response=name):
                self.validate_action(action, name in allowed)

    def act(self, name, targets=None, *, mana, spend):
        self.cast(name, targets or [], mana=mana, spend=spend)

    def assert_locked(self, player, legal_commands):
        """Every independently supplied earlier-legal response is refused while locked."""
        asked = self.asked()
        self.assertEqual((asked["kind"], asked["player"]), ("priority", player))
        before = (self.engine.state["stack"], [p["life"] for p in self.engine.state["players"]])
        for name, cmd in legal_commands:
            with self.subTest(player=player, refused=name):
                self.reject(player, cmd)
        self.assertEqual((self.engine.state["stack"], [p["life"] for p in self.engine.state["players"]]), before)

    def bolt_then_sudden_shock(self):
        self.new([{"hand": ["Lightning Bolt", "Giant Growth"], "graveyard": ["Think Twice"],
                   "battlefield": ["Mountain", "Forest", "Island", "Island", "Island", "Elves of Deep Shadow", "Prodigal Pyromancer"]},
                  {"hand": ["Sudden Shock", "Lull"], "graveyard": ["Reassembling Skeleton"],
                   "battlefield": ["Mountain", "Mountain", "Swamp", "Swamp", "Plains", "Plains", "Mind Stone"]}])
        self.main()
        self.act("Lightning Bolt", [[{"player": 1}]], mana=self.mine("Mountain", 0)[:1], spend={'R': 1})
        pyro = self.one("Prodigal Pyromancer")
        p0 = [("Giant Growth", self.spell(0, "Giant Growth", self.mine("Forest", 0)[:1], [[{"object": self.one("Elves of Deep Shadow")}]], spend={'G': 1})),
              ("Prodigal Pyromancer", {"kind": "activate", "source": pyro, "ability": 0, "targets": [[{"player": 1}]], "plan": []}),
              ("Think Twice", {"kind": "cast", "card": self.ids("Think Twice")[0], "route": "flashback", "targets": [], "mana": {"U": 3},
                               "plan": [{"source": o, "ability": 0} for o in self.mine("Island", 0)[:3]]})]
        self.responses[0] = p0
        self.assert_responses([name for name, _ in p0])
        self.engine.act({"kind": "pass"})
        p1 = [("Lull", {"kind": "activate", "source": self.ids("Lull")[0], "ability": 2, "mana": {"W": 2}, "targets": [],
                         "plan": [{"source": o, "ability": 0} for o in self.mine("Plains", 1)[:2]]}),
              ("Mind Stone", {"kind": "activate", "source": self.one("Mind Stone"), "ability": 1, "mana": {"W": 1}, "targets": [],
                               "plan": [{"source": self.mine("Plains", 1)[0], "ability": 0}]}),
              ("Reassembling Skeleton", {"kind": "activate", "source": self.ids("Reassembling Skeleton")[0], "ability": 0, "mana": {"B": 2}, "targets": [],
                                         "plan": [{"source": o, "ability": 0} for o in self.mine("Swamp", 1)[:2]]})]
        self.responses[1] = p1
        self.assert_responses([name for name, _ in p1])
        self.act("Sudden Shock", [[{"object": pyro}]], mana=self.mine("Mountain", 1)[:2], spend={'R': 2})
        return p0, p1, pyro

    def test_sudden_shock_locks_both_players_and_lifts_before_the_bolt_resolves(self):
        p0, p1, pyromancer = self.bolt_then_sudden_shock()
        self.assertEqual(len(self.engine.state["stack"]), 2)
        # the caster holds priority first: P1 is locked too, including hand cycling and the graveyard
        self.assert_locked(1, p1)
        self.engine.act({"kind": "pass"})
        self.assert_locked(0, p0)
        # mana abilities stay: Elves of Deep Shadow adds {B} and deals 1 to P0
        elves = self.one("Elves of Deep Shadow")
        self.engine.act({"kind": "activate", "source": elves, "ability": 0, "targets": [], "plan": []})
        self.assertEqual((self.life(0), self.engine.state["players"][0]["pool"].get("B", 0)), (19, 1))
        self.assertEqual(len(self.engine.state["stack"]), 2)
        # an action between passes: both players pass again in succession (117.4)
        self.engine.act({"kind": "pass"})
        self.assert_locked(1, p1)
        self.engine.act({"kind": "pass"})
        # Sudden Shock resolved; the Bolt is still there and both players may act again
        self.assertNotIn(pyromancer, self.engine.state["battlefield"])
        self.assertEqual(len(self.engine.state["stack"]), 1)
        self.assertEqual(self.asked()["player"], 0)
        self.assert_responses(["Giant Growth", "Think Twice"])
        self.engine.act({"kind": "pass"})
        self.assertEqual(self.asked()["player"], 1)
        self.assert_responses(["Lull", "Mind Stone", "Reassembling Skeleton"])
        self.engine.act({"kind": "pass"})
        self.assertEqual((self.engine.state["stack"], self.life(1)), ([], 17), "the Bolt was unaffected")

    def test_krosan_grip_keeps_the_mana_ability_of_its_target(self):
        self.new([{"hand": ["Krosan Grip"], "battlefield": ["Forest"] * 3},
                  {"hand": ["Shock"], "battlefield": ["Mind Stone", "Mountain"]}])
        stone = self.one("Mind Stone")
        self.responses[1] = [("Shock", self.spell(1, "Shock", self.mine("Mountain", 1)[:1], [[{"player": 0}]], spend={'R': 1})),
                             ("Mind Stone", {"kind": "activate", "source": stone, "ability": 1, "mana": {"R": 1}, "targets": [],
                                             "plan": [{"source": self.mine("Mountain", 1)[0], "ability": 0}]})]
        self.main()
        self.act("Krosan Grip", [[{"object": stone}]], mana=self.mine('Forest', self.asked()["player"])[:3], spend={'G': 3})
        self.engine.act({"kind": "pass"})
        self.assertEqual(self.asked()["player"], 1)
        self.assert_responses([])
        self.reject(1, {"kind": "activate", "source": stone, "ability": 1, "mana": {"R": 1}, "targets": [],
                        "plan": [{"source": self.mine("Mountain", 1)[0], "ability": 0}]})
        self.engine.act({"kind": "activate", "source": stone, "ability": 0, "targets": [], "plan": []})
        self.assertEqual(self.engine.state["players"][1]["pool"].get("C", 0), 1)
        self.engine.act({"kind": "pass"})
        self.assertEqual(self.asked()["player"], 0)
        self.assert_responses([])
        self.engine.act({"kind": "pass"})
        self.assertNotIn(stone, self.engine.state["battlefield"])

    def test_a_cast_trigger_stacks_and_resolves_above_split_second(self):
        self.new([{"hand": ["Sudden Shock", "Lightning Bolt"], "battlefield": ["Guttersnipe", "Mountain", "Mountain", "Mountain"]}, {}])
        self.responses[0] = [("Lightning Bolt", self.spell(0, "Lightning Bolt", self.mine("Mountain", 0)[2:3], [[{"player": 1}]], spend={'R': 1}))]
        self.main()
        self.act("Sudden Shock", [[{"player": 1}]], mana=self.mine('Mountain', self.asked()["player"])[:2], spend={'R': 2})
        self.assertEqual(len(self.engine.state["stack"]), 2, "Guttersnipe's trigger is above the spell")
        self.assert_responses([])
        self.engine.act({"kind": "pass"})
        self.engine.act({"kind": "pass"})
        self.assertEqual((self.life(1), len(self.engine.state["stack"])), (18, 1))
        self.assertEqual(self.asked()["player"], 0)
        self.assert_responses([])
        self.settle()
        self.assertEqual(self.life(1), 16)
        self.assert_responses(["Lightning Bolt"])

    def test_hesitation_counters_split_second_and_unlocks(self):
        self.new([{"hand": ["Sudden Shock", "Lightning Bolt"], "battlefield": ["Mountain"] * 3},
                  {"battlefield": ["Hesitation"]}])
        self.responses[0] = [("Lightning Bolt", self.spell(0, "Lightning Bolt", self.mine("Mountain", 0)[2:3], [[{"player": 1}]], spend={'R': 1}))]
        self.main()
        self.act("Sudden Shock", [[{"player": 1}]], mana=self.mine('Mountain', self.asked()["player"])[:2], spend={'R': 2})
        self.assert_responses([])
        self.settle()
        self.assertEqual((self.life(1), self.engine.state["stack"]), (20, []), "countered by the trigger")
        self.assertEqual(self.mine("Hesitation", 1), [], "sacrificed")
        self.assert_responses(["Lightning Bolt"])

    def test_double_vision_copy_also_locks_and_the_original_keeps_locking(self):
        self.new([{"hand": ["Sudden Shock", "Lightning Bolt"], "battlefield": ["Double Vision", "Mountain", "Mountain", "Mountain"]},
                  {"hand": ["Shock"], "battlefield": ["Mountain"]}])
        self.responses[0] = [("Lightning Bolt", self.spell(0, "Lightning Bolt", self.mine("Mountain", 0)[2:3], [[{"player": 1}]], spend={'R': 1}))]
        self.responses[1] = [("Shock", self.spell(1, "Shock", self.mine("Mountain", 1)[:1], [[{"player": 0}]], spend={'R': 1}))]
        self.main()
        self.act("Sudden Shock", [[{"player": 1}]], mana=self.mine('Mountain', self.asked()["player"])[:2], spend={'R': 2})
        self.assertEqual([self.obj(o)["name"] for o in self.engine.state["stack"]], ["Sudden Shock", "Double Vision"])
        self.assert_responses([])
        self.engine.act({"kind": "pass"})
        self.engine.act({"kind": "pass"})
        # the copy waits on its new-target choice (707.10c): a pending decision
        asked = self.asked()
        self.assertEqual((asked["kind"], asked["player"], asked["original"]), ("retarget", 0, [[{"player": 1}]]))
        self.reject(1, {"kind": "pass"})
        self.engine.act({"kind": "choose", "answer": {"targets": [[{"player": 0}]]}})
        self.assertEqual([self.obj(o)["name"] for o in self.engine.state["stack"]], ["Sudden Shock", "Sudden Shock"])
        self.assertEqual(self.asked()["kind"], "priority")
        self.assert_responses([])
        self.engine.act({"kind": "pass"})
        self.assert_responses([])
        self.engine.act({"kind": "pass"})
        self.assertEqual((self.life(0), self.life(1), len(self.engine.state["stack"])), (18, 20, 1), "the retargeted copy resolved")
        self.assert_responses([])
        self.engine.act({"kind": "pass"})
        self.assert_responses([])
        self.settle()
        self.assertEqual((self.life(0), self.life(1)), (18, 18))
        self.assertEqual(self.asked()["player"], 0)
        self.assert_responses(["Lightning Bolt"])

    def test_mirari_payment_uses_mana_abilities_under_split_second(self):
        self.new([{"hand": ["Sudden Shock", "Lightning Bolt"], "battlefield": ["Mirari"] + ["Mountain"] * 6}, {}])
        self.responses[0] = [("Lightning Bolt", self.spell(0, "Lightning Bolt", self.mine("Mountain", 0)[5:6], [[{"player": 1}]], spend={'R': 1}))]
        self.main()
        self.act("Sudden Shock", [[{"player": 1}]], mana=self.mine('Mountain', self.asked()["player"])[:2], spend={'R': 2})
        self.engine.act({"kind": "pass"})
        self.engine.act({"kind": "pass"})
        asked = self.asked()
        self.assertEqual((asked["kind"], asked["player"], asked["cost"]), ("pay", 0, ["3"]))
        self.engine.act({"kind": "choose", "answer": {"pay": {"mana": {"R": 3}, "plan": [{"source": o, "ability": 0} for o in self.mine("Mountain", 0)[2:5]]}}})
        self.assertEqual(self.asked()["kind"], "retarget")
        self.engine.act({"kind": "choose", "answer": "no"})
        self.assertEqual([self.obj(o)["name"] for o in self.engine.state["stack"]], ["Sudden Shock", "Sudden Shock"])
        self.assertEqual(sum(self.obj(m)["tapped"] for m in self.mine("Mountain", 0)), 5, "2 for the spell and 3 for Mirari")
        self.assert_responses([])
        self.engine.act({"kind": "pass"})
        self.engine.act({"kind": "pass"})
        self.assertEqual(self.life(1), 18)
        self.assert_responses([])
        self.settle()
        self.assertEqual(self.life(1), 16)
        self.assert_responses(["Lightning Bolt"])
    def test_samut_grants_split_second_only_to_its_controllers_spells(self):
        self.new([{"hand": ["Lightning Bolt"], "battlefield": ["Samut, Tyrant of Naktamun", "Mountain"]},
                  {"hand": ["Shock", "Shock"], "battlefield": ["Mountain", "Mountain"]}])
        self.responses[0] = [("Lightning Bolt", self.spell(0, "Lightning Bolt", self.mine("Mountain", 0)[:1], [[{"player": 1}]], spend={'R': 1}))]
        self.main()
        self.engine.act({"kind": "pass"})
        self.act("Shock", [[{"player": 0}]], mana=self.mine('Mountain', self.asked()["player"])[:1], spend={'R': 1})
        self.responses[1] = [("Shock", self.spell(1, "Shock", self.mine("Mountain", 1)[1:2], [[{"player": 0}]], spend={'R': 1}))]
        self.engine.act({"kind": "pass"})
        self.assert_responses(["Lightning Bolt"])
        self.act("Lightning Bolt", [[{"player": 1}]], mana=self.mine('Mountain', self.asked()["player"])[:1], spend={'R': 1})
        self.assert_responses([])
        self.engine.act({"kind": "pass"})
        self.assertEqual(self.asked()["player"], 1)
        self.assert_responses([])
        self.engine.act({"kind": "pass"})
        self.assertEqual((self.life(1), len(self.engine.state["stack"])), (17, 1))
        self.assertEqual(self.asked()["player"], 0)
        self.engine.act({"kind": "pass"})
        self.assert_responses(["Shock"])

    def test_samut_leaving_ends_the_grant_while_its_spell_is_on_the_stack(self):
        # 611.3a: the granted ability exists only while Samut is on the battlefield
        self.new([{"hand": ["Shock"], "battlefield": ["Samut, Tyrant of Naktamun", "Double Vision", "Mountain", "Mountain"]},
                  {"hand": ["Lightning Bolt"], "battlefield": ["Mountain"]}])
        samut = self.one("Samut, Tyrant of Naktamun")
        self.responses[1] = [("Lightning Bolt", self.spell(1, "Lightning Bolt", self.mine("Mountain", 1)[:1], [[{"player": 0}]], spend={'R': 1}))]
        self.main()
        self.act("Shock", [[{"object": samut}]], mana=self.mine('Mountain', self.asked()["player"])[:1], spend={'R': 1})
        self.assert_responses([])
        self.engine.act({"kind": "pass"})
        self.assertEqual(self.asked()["player"], 1)
        self.assert_responses([])
        self.engine.act({"kind": "pass"})
        self.assertEqual(self.asked()["kind"], "retarget")
        self.engine.act({"kind": "choose", "answer": "no"})
        self.assertEqual([self.obj(o)["name"] for o in self.engine.state["stack"]], ["Shock", "Shock"])
        self.engine.act({"kind": "pass"})
        self.assert_responses([])
        self.engine.act({"kind": "pass"})
        # the copy killed Samut; the original Shock no longer has split second
        self.assertNotIn(samut, self.engine.state["battlefield"])
        self.assertEqual(len(self.engine.state["stack"]), 1)
        self.engine.act({"kind": "pass"}) if self.asked()["player"] == 0 else None
        self.assertEqual(self.asked()["player"], 1)
        self.assert_responses(["Lightning Bolt"])

    def test_celestial_crusader_locks_only_while_it_is_a_spell(self):
        self.new([{"hand": ["Lightning Bolt"], "battlefield": ["Mountain", "Mountain"]},
                  {"hand": ["Celestial Crusader"], "battlefield": ["Plains"] * 4 + ["Savannah Lions"]}])
        lions = self.one("Savannah Lions")
        self.responses[0] = [("Lightning Bolt", self.spell(0, "Lightning Bolt", self.mine("Mountain", 0)[:1], [[{"player": 1}]], spend={'R': 1}))]
        self.main()
        self.engine.act({"kind": "pass"})
        self.act("Celestial Crusader", mana=self.mine('Plains', self.asked()["player"])[:4], spend={'W': 4})
        self.assert_responses([])
        self.engine.act({"kind": "pass"})
        self.assertEqual(self.asked()["player"], 0)
        self.assert_responses([])
        self.engine.act({"kind": "pass"})
        crusader = self.one("Celestial Crusader")
        self.assertEqual((self.obj(lions)["power"], self.obj(lions)["toughness"]), (3, 2))
        self.assertEqual(self.asked()["player"], 0)
        self.assert_responses(["Lightning Bolt"])
        self.act("Lightning Bolt", [[{"object": crusader}]], mana=self.mine('Mountain', self.asked()["player"])[:1], spend={'R': 1})
        self.settle()
        self.assertNotIn(crusader, self.engine.state["battlefield"])


if __name__ == "__main__":
    unittest.main()
