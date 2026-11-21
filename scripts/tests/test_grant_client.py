"""Full-card grant regressions through the real Python/JSON client."""
import unittest
from test_multiplayer_outcome_client import MultiplayerOutcomeClientTests as Helpers


class GrantClientTests(unittest.TestCase):
    setUpClass = classmethod(Helpers.setUpClass.__func__)
    setUp = Helpers.setUp
    close_engine = Helpers.close_engine
    new = Helpers.new
    view = Helpers.view
    hand = Helpers.hand
    ids = Helpers.ids
    pass_round = Helpers.pass_round
    choose = Helpers.choose
    reject = Helpers.reject
    cast = Helpers.cast

    def object(self, obj):
        return self.engine.state["objects"][str(obj)]

    def activate(self, source, targets, *, ability, mana=(), paying=(), spend=None, **choices):
        self.engine.act({"kind": "activate", "source": source, "ability": ability,
                         "targets": targets, "plan": [{"source": o, "ability": 0} for o in mana],
                         "paying": list(paying), **({"mana": spend} if spend is not None else {}), **choices})

    def step(self):
        state = self.engine.state
        asked = state["asked"]
        kind = asked["kind"]
        if kind == "mulligan":
            self.engine.act({"kind": "keep"})
        elif kind == "attackers":
            self.engine.act({"kind": "attack", "attacks": []})
        elif kind == "blockers":
            self.engine.act({"kind": "block", "blocks": []})
        elif kind == "discard":
            self.engine.act({"kind": "discard", "cards": self.hand(asked["player"])[:asked["count"]]})
        else:
            self.assertEqual(kind, "priority", state)
            self.engine.act({"kind": "pass"})

    def advance(self, predicate):
        for _ in range(600):
            if predicate(self.engine.state):
                return
            self.step()
        self.fail("did not reach checkpoint")

    def helm_ping(self, blue_only=False):
        self.new([{"hand": ["Helm of the Ghastlord", "Disenchant"], "battlefield": ["Island"] + ["Forest"] * 4 + ["Plains"]},
                  {"hand": ["Cerulean Wisps"], "battlefield": ["Baleful Strix", "Viridian Longbow", "Forest", "Forest", "Forest", "Island"]},
                  {"hand": ["Forest", "Forest"]}])
        creature = self.ids("Baleful Strix")[0]
        self.cast("Helm of the Ghastlord", [[{"object": creature}]], mana=self.ids("Island")[:1] + self.ids("Forest")[:3], spend={'U': 1, 'G': 3})
        self.pass_round()
        self.assertEqual(self.object(creature)["power"], 3)
        self.advance(lambda s: s["active"] == 1 and s["part"] == "PrecombatMain")
        if blue_only:
            self.cast("Cerulean Wisps", [[{"object": creature}]], mana=[self.ids("Island")[-1]], spend={'U': 1})
            self.pass_round()
            self.assertEqual(self.object(creature)["power"], 2)
        self.activate(self.ids("Viridian Longbow")[0], [[{"object": creature}]], ability=2, mana=self.ids("Forest", zone="battlefield", controller=1)[:3], spend={'G': 3})
        self.pass_round()
        self.activate(creature, [[{"player": 2}]], ability=3 if blue_only else 4)
        self.pass_round()
        asked = self.engine.state["asked"]
        if blue_only:
            self.assertEqual(asked["kind"], "priority")
            self.assertEqual(len(self.engine.state["stack"]), 1)
            return creature
        self.assertEqual((asked["kind"], asked["player"]), ("order", 1))
        self.assertEqual(len(asked["abilities"]), 2)
        self.assertTrue(all(a["source"] == creature for a in asked["abilities"]))
        self.reject(0, {"kind": "order", "order": [0, 1]})
        return creature

    def resolve_helm_triggers(self):
        self.engine.act({"kind": "order", "order": [0, 1]})
        before = len(self.hand(1))
        for _ in range(2):
            self.pass_round()
            if self.engine.state["asked"]["kind"] == "objects":
                self.assertEqual(self.engine.state["asked"]["player"], 2)
                self.reject(1, {"kind": "choose", "answer": {"objects": [self.hand(2)[0]]}})
                self.choose({"objects": [self.hand(2)[-1]]})
        self.assertEqual(len(self.hand(1)), before + 1)
        self.assertEqual(len(self.hand(2)), 1)

    def test_full_helm_noncombat_damage_controller_recipient_and_private_discard(self):
        self.helm_ping()
        before = len(self.hand(0))
        self.resolve_helm_triggers()
        self.assertEqual(len(self.hand(0)), before)
        graveyard = self.engine.state["players"][2]["graveyard"]
        self.assertEqual(len(graveyard), 1)
        for viewer in [0, 1, 2]:
            self.assertEqual(self.view(viewer)["objects"][str(graveyard[0])]["name"], "Forest")
        self.assertNotIn("hand", self.view(0)["players"][2])

    def test_helm_color_change_removes_black_trigger_and_restores_at_cleanup(self):
        creature = self.helm_ping(blue_only=True)
        before = len(self.hand(1))
        self.pass_round()
        self.assertEqual(len(self.hand(1)), before + 1)
        self.assertEqual(len(self.hand(2)), 2, "the black discard trigger wasn't present")
        self.advance(lambda s: s["active"] == 2 and s["part"] == "PrecombatMain")
        self.assertEqual(self.object(creature)["power"], 3, "both conditional bonuses return after paint expires")

    def test_helm_source_departure_keeps_captured_damage_triggers(self):
        self.helm_ping()
        self.engine.act({"kind": "order", "order": [0, 1]})
        self.advance(lambda s: s["asked"]["player"] == 0)
        helm = next(i for i in self.ids("Helm of the Ghastlord") if i in self.engine.state["battlefield"])
        self.cast("Disenchant", [[{"object": helm}]], mana=self.ids("Plains", zone="battlefield", controller=0)[:1] + self.ids("Forest")[3:4], spend={'W': 1, 'G': 1})
        self.pass_round()
        creature = self.ids("Baleful Strix")[0]
        self.assertEqual(self.object(creature)["power"], 1)
        self.assertEqual(len(self.engine.state["stack"]), 2)
        before = len(self.hand(1))
        for _ in range(2):
            self.pass_round()
            if self.engine.state["asked"]["kind"] == "objects":
                self.choose({"objects": [self.hand(2)[0]]})
        self.assertEqual(len(self.hand(1)), before + 1)
        self.assertEqual(len(self.hand(2)), 1)

    def test_chakrams_job_select_suspends_entry_and_resumes_once(self):
        self.new([{"hand": ["Dancer's Chakrams"], "battlefield": ["Plains"] * 4 +
                   ["Grumgully, the Generous", "Hardened Scales", "Branching Evolution"]}, {}, {}])
        self.cast("Dancer's Chakrams", mana=self.ids("Plains", zone="battlefield", controller=0)[:4], spend={'W': 4})
        self.pass_round()
        self.pass_round()
        asked = self.engine.state["asked"]
        self.assertEqual((asked["kind"], asked["player"]), ("replacement", 0))
        effect = next(e for e in asked["among"] if self.object(e["source"])["name"] == "Hardened Scales")
        answer = {"replacement": {key: effect[key] for key in ["source", "ability", "part"]}}
        self.reject(1, {"kind": "choose", "answer": answer})
        self.choose(answer)
        heroes = [i for i in self.engine.state["battlefield"] if self.object(i)["token"]]
        self.assertEqual(len(heroes), 1)
        hero = self.object(heroes[0])
        self.assertEqual(hero["power"], 7)
        self.assertEqual(hero["counters"]["+1/+1"], 4)
        self.assertIn("Hero", hero["subtypes"])
        self.assertIn("Performer", hero["subtypes"])
        equipment = next(i for i in self.ids("Dancer's Chakrams") if i in self.engine.state["battlefield"])
        self.assertEqual(self.object(equipment)["attached_to"], {"object": heroes[0]})
        self.assertEqual(self.engine.state["asked"]["kind"], "priority")

    def test_full_chakrams_equip_moves_performer_bonus_and_source_loss_restores_base(self):
        self.new([{"hand": ["Dancer's Chakrams", "Disenchant"], "battlefield": ["Plains"] * 9 + ["Grizzly Bears"]}, {}, {}])
        self.cast("Dancer's Chakrams", mana=self.ids("Plains", zone="battlefield", controller=0)[:4], spend={'W': 4})
        self.pass_round()
        self.pass_round()
        equipment = next(i for i in self.ids("Dancer's Chakrams") if i in self.engine.state["battlefield"])
        hero = next(i for i in self.engine.state["battlefield"] if self.object(i)["token"])
        bear = self.ids("Grizzly Bears")[0]
        self.activate(equipment, [[{"object": bear}]], ability=4, mana=self.ids("Plains", zone="battlefield", controller=0)[4:7], spend={'W': 3})
        self.pass_round()
        self.assertEqual(self.object(hero)["power"], 1)
        self.assertNotIn("Performer", self.object(hero)["subtypes"])
        self.assertEqual(self.object(bear)["power"], 4)
        self.assertIn("Performer", self.object(bear)["subtypes"])
        self.cast("Disenchant", [[{"object": equipment}]], mana=self.ids("Plains", zone="battlefield", controller=0)[7:9], spend={'W': 2})
        self.pass_round()
        self.assertEqual(self.object(bear)["power"], 2)
        self.assertNotIn("Performer", self.object(bear)["subtypes"])

    def test_managed_commander_bonus_lifelink_and_self_exclusion_through_actual_combat(self):
        # Legal singleton Commander deck; fixed seed puts Chakrams in the opening hand.
        self.engine.ask({"cmd": "start", "format": "commander", "seed": 9,
                         "decks": [["Dancer's Chakrams"] + ["Plains"] * 98, ["Plains"] * 99, ["Plains"] * 99],
                         "commanders": ["Isamaru, Hound of Konda"] * 3})
        self.advance(lambda s: s["active"] == 0 and s["part"] == "PrecombatMain" and s["asked"]["kind"] == "priority")
        for lands in range(1, 5):
            state = self.engine.state
            land = next(o for o in state["players"][0]["hand"] if state["objects"][str(o)]["name"] == "Plains")
            self.engine.act({"kind": "play", "card": land})
            if lands == 1:
                self.cast("Isamaru, Hound of Konda", mana=self.ids("Plains", zone="battlefield", controller=0)[:1], spend={'W': 1})
                self.pass_round()
            if lands != 4:
                turn = self.engine.state["turn"]
                self.advance(lambda s: s["turn"] > turn and s["active"] == 0 and s["part"] == "PrecombatMain")
        self.cast("Dancer's Chakrams", mana=self.ids("Plains", zone="battlefield", controller=0)[:4], spend={'W': 4})
        self.pass_round()
        self.pass_round()
        commander = next(i for i in self.engine.state["battlefield"] if self.object(i).get("commander"))
        hero = next(i for i in self.engine.state["battlefield"] if self.object(i)["token"])
        self.assertEqual(self.object(commander)["power"], 4)
        before = self.engine.state["players"][0]["life"]
        self.advance(lambda s: s["asked"]["kind"] == "attackers")
        self.engine.act({"kind": "attack", "attacks": [[commander, {"player": 1}]]})
        self.advance(lambda s: s["part"] == "PostcombatMain")
        self.assertEqual(self.engine.state["players"][0]["life"], before + 4, "nested static grants commander lifelink")
        turn = self.engine.state["turn"]
        self.advance(lambda s: s["turn"] > turn and s["active"] == 0 and s["part"] == "PrecombatMain")
        equipment = self.ids("Dancer's Chakrams")[0]
        self.activate(equipment, [[{"object": commander}]], ability=4, mana=self.ids("Plains", zone="battlefield", controller=0)[:3], spend={'W': 3})
        self.pass_round()
        self.assertEqual(self.object(commander)["power"], 4, "direct +2 excludes self from other-commanders bonus")
        self.assertEqual(self.object(hero)["power"], 1)


del Helpers
