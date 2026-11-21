"""The choice in "create a Food token or a Treasure token"
(Tireless Provisioner's landfall, Ant-Man's Army's enters trigger) through the real audited client
with full pinned cards. The controller chooses one of the two instructions as the ability resolves
(608.2d) and exactly that predefined token is created (701.7a, 111.10a-b), with its own ability.
Both are always possible; there is no decline. The trigger resolves after its source leaves
(603.10, 113.7a). When the token's entry has two applicable replacement effects, its controller
orders them (616.1) and the instruction is not asked again."""
import unittest
import test_toxic_review_client as base

HELPERS = {k: v for k, v in vars(base.ToxicReviewClientTests).items() if not k.startswith("test_") and not k.startswith("__")}

FOOD = {"index": 0, "action": "Create", "tokens": [{"name": "Food", "count": 1}]}
TREASURE = {"index": 1, "action": "Create", "tokens": [{"name": "Treasure", "count": 1}]}


class TokenAlternativeReviewClientTests(unittest.TestCase):
    locals().update(HELPERS)

    # --- reading the state

    def state(self):
        return self.engine.state

    def tokens(self, player=0):
        return sorted((self.obj(o)["name"], self.obj(o)["tapped"]) for o in self.state()["battlefield"]
                      if self.obj(o)["token"] and self.obj(o)["controller"] == player)

    def chosen(self, viewer):
        return [(c["player"], c["alternative"]) for c in self.view(viewer)["choices_made"] if "alternative" in c]

    def question(self, player=0):
        a = self.asked()
        self.assertEqual((a["kind"], a["player"], a["optional"], a["among"]), ("alternative", player, False, [FOOD, TREASURE]), a)
        return a

    # --- acting

    def until_asked(self):
        for _ in range(60):
            a = self.asked()
            if a["kind"] != "priority" or not self.state()["stack"]:
                return a
            self.engine.act({"kind": "pass"})
        self.fail("the stack did not settle")

    def play_land(self, name):
        card = next(o for o in self.state()["players"][self.asked()["player"]]["hand"] if self.obj(o)["name"] == name)
        self.engine.act({"kind": "play", "card": card})

    def choose(self, index):
        return self.engine.act({"kind": "choose", "answer": {"alternative": index}})

    def army(self, battlefield=(), theirs=(), hand=()):
        """P0 casts Ant-Man's Army; its enters trigger resolves and asks."""
        self.new([{"hand": ["Ant-Man's Army", *hand], "battlefield": ["Forest"] * 3 + list(battlefield), "library": ["Forest"] * 8},
                  {"battlefield": list(theirs), "library": ["Island"] * 8}])
        self.main()
        self.cast("Ant-Man's Army", mana=self.ids("Forest", zone="battlefield", controller=0)[:3], spend={'G': 3})
        return self.until_asked()

    def provisioner(self, hand=("Forest",), battlefield=(), theirs=()):
        self.new([{"hand": list(hand), "battlefield": ["Tireless Provisioner", *battlefield], "library": ["Forest"] * 8},
                  {"battlefield": list(theirs), "library": ["Island"] * 8}])
        self.main()

    # --- Ant-Man's Army

    def test_each_answer_creates_exactly_that_token(self):
        for index, name in ((0, "Food"), (1, "Treasure")):
            with self.subTest(name=name):
                self.army()
                self.question()
                self.assertEqual(self.tokens(), [])
                events = self.choose(index)["events"]
                self.assertEqual([e["event"] for e in events if e.get("event") == "created"], ["created"])
                self.assertEqual(self.tokens(), [(name + " Token", False)])
                token = self.one(name + " Token")
                self.assertEqual((self.obj(token)["types"], self.obj(token)["subtypes"]), (["Artifact"], [name]))
                self.assertEqual(self.asked()["kind"], "priority")

    def test_the_created_token_is_public_to_both_players(self):
        self.army()
        self.question()
        other = self.view(1)["asked"]
        self.assertEqual((other["kind"], other["player"]), ("alternative", 0), "the other player sees who is choosing")
        self.choose(1)
        token = self.one("Treasure Token")
        self.assertEqual(self.view(1)["objects"][str(token)]["name"], "Treasure Token")

    def test_invalid_answers_are_refused_and_change_nothing(self):
        self.army()
        self.question()
        for answer in ({"alternative": 2}, {"alternative": -1}, {"alternative": "0"}, {"alternative": None},
                       {"alternative": 1.0}, "no", "yes", {"objects": []}, {"value": {"number": 0}}):
            with self.subTest(answer=answer):
                self.reject(0, {"kind": "choose", "answer": answer})
                self.question()
        self.reject(0, {"kind": "pass"})
        self.reject(1, {"kind": "choose", "answer": {"alternative": 0}})
        self.question()
        self.assertEqual((self.tokens(), self.chosen(0)), ([], []))
        self.choose(0)
        self.assertEqual(self.tokens(), [("Food Token", False)])

    def test_food_gains_three_life_and_treasure_adds_mana(self):
        self.army(battlefield=["Forest"] * 2)
        self.choose(0)
        food = self.one("Food Token")
        self.engine.act({"kind": "activate", "source": food, "ability": 0, "mana": {"G": 2}, "targets": [],
                         "plan": [{"source": o, "ability": 0} for o in self.ids("Forest", zone="battlefield", controller=0)[3:5]]})
        self.until_asked()
        self.assertEqual((self.life(0), self.tokens()), (23, []), "{2}, {T}, sacrifice: gain 3 life")
        self.army(hand=["Lightning Bolt"])
        self.choose(1)
        treasure = self.one("Treasure Token")
        self.cast("Lightning Bolt", [[{"player": 1}]], spend={"R": 1}, plan=[{"source": treasure, "ability": 0, "adding": [["R"]]}])
        self.until_asked()
        self.assertEqual((self.life(1), self.tokens()), (17, []))

    def test_the_food_is_an_artifact_for_vision_of_love(self):
        """Contrast: an optional object alternative keeps its object wire, and offers the Food."""
        self.army(battlefield=["Mountain"] * 2, hand=["Vision of Love", "Forest"])
        self.choose(0)
        food = self.one("Food Token")
        self.cast("Vision of Love", mana=self.ids("Mountain", zone="battlefield", controller=0)[:2], spend={'R': 2})
        a = self.until_asked()
        self.assertEqual((a["kind"], a["optional"]), ("alternative", True), a)
        self.assertEqual(sorted(a["among"], key=lambda o: o["index"])[0].keys(), {"index", "action", "among", "count"})
        sacrifice = next(o for o in a["among"] if o["action"] == "Sacrifice")
        self.assertEqual((sacrifice["among"], sacrifice["count"]), ([food], 1))
        self.choose(sacrifice["index"])
        self.until_asked()
        self.assertEqual(self.tokens(), [])

    # --- Tireless Provisioner

    def test_each_landfall_asks_its_own_choice(self):
        self.provisioner(hand=["Forest", "Evolving Wilds"])
        self.play_land("Evolving Wilds")
        self.until_asked()
        self.question()
        self.choose(1)
        wilds = self.one("Evolving Wilds")
        self.engine.act({"kind": "activate", "source": wilds, "ability": 0, "targets": [], "plan": []})
        a = self.until_asked()
        if a["kind"] != "alternative":
            forest = next(o for o in a["among"] if self.obj(o)["name"] == "Forest")
            self.engine.act({"kind": "choose", "answer": {"objects": [forest]}})
            self.until_asked()
        self.question()
        self.choose(0)
        self.assertEqual(self.tokens(), [("Food Token", False), ("Treasure Token", False)])

    def test_the_trigger_resolves_after_the_provisioner_is_unsummoned(self):
        self.provisioner(hand=["Forest", "Unsummon"], battlefield=["Island"])
        self.play_land("Forest")
        self.assertEqual(self.asked()["player"], 0)
        self.cast("Unsummon", [[{"object": self.one("Tireless Provisioner")}]], mana=self.ids("Island", zone="battlefield", controller=0)[:1], spend={'U': 1})
        self.until_asked()
        self.question()
        self.assertNotIn("Tireless Provisioner", [self.obj(o)["name"] for o in self.state()["battlefield"]])
        self.choose(1)
        self.assertEqual(self.tokens(), [("Treasure Token", False)])

    def test_its_controller_chooses_on_an_opponents_turn(self):
        """Growth Spiral puts P0's land onto the battlefield in P1's turn: P0 chooses."""
        self.new([{"hand": ["Growth Spiral", "Forest"], "battlefield": ["Tireless Provisioner", "Forest", "Island"], "library": ["Forest"] * 8},
                  {"library": ["Island"] * 8}])
        self.advance(lambda s: s["active"] == 1 and s["part"] == "PrecombatMain" and s["asked"]["kind"] == "priority"
                     and s["asked"]["player"] == 1 and not s["stack"])
        self.engine.act({"kind": "pass"})
        self.cast("Growth Spiral", mana=self.ids("Forest", zone="battlefield", controller=0)[:1] + self.ids("Island", zone="battlefield", controller=0)[:1], spend={'G': 1, 'U': 1})
        a = self.until_asked()
        self.assertEqual((a["kind"], a["player"]), ("optional_objects", 0))
        land = next(o for o in a["among"] if self.obj(o)["name"] == "Forest")
        self.engine.act({"kind": "choose", "answer": {"accept": True, "objects": [land]}})
        self.until_asked()
        self.question(0)
        self.assertEqual(self.state()["active"], 1)
        self.choose(0)
        self.assertEqual(self.tokens(), [("Food Token", False)])

    # --- the token's entry

    def test_two_entry_effects_are_ordered_and_the_instruction_is_not_asked_again(self):
        for index, name in ((0, "Food"), (1, "Treasure")):
            with self.subTest(name=name):
                self.provisioner(battlefield=["Root Maze", "Orb of Dreams"])
                self.play_land("Forest")
                land = self.asked()
                self.assertEqual((land["kind"], land.get("event")), ("replacement", "entry"), "the land enters tapped too")
                self.engine.act({"kind": "choose", "answer": {"replacement": {k: v for k, v in land["among"][0].items() if k != "name"}}})
                self.until_asked()
                self.question()
                self.choose(index)
                a = self.asked()
                self.assertEqual((a["kind"], a.get("event"), a["player"]), ("replacement", "entry", 0), a)
                self.assertEqual(self.tokens(), [], "nothing has entered while the order is asked")
                self.assertEqual([self.chosen(v) for v in (0, 1)], [[(0, index)], [(0, index)]], "public at the child question")
                effect = {k: v for k, v in a["among"][0].items() if k != "name"}
                self.engine.act({"kind": "choose", "answer": {"replacement": effect}})
                self.assertEqual(self.asked()["kind"], "priority")
                self.assertEqual(self.tokens(), [(name + " Token", True)])

    def test_one_entry_effect_asks_nothing(self):
        self.provisioner(theirs=["Kismet"])
        self.play_land("Forest")
        self.until_asked()
        self.question()
        self.choose(0)
        self.assertEqual(self.asked()["kind"], "priority")
        self.assertEqual(self.tokens(), [("Food Token", True)])

    def test_strike_it_rich_still_creates_its_treasure_without_a_question(self):
        self.new([{"hand": ["Strike It Rich"], "battlefield": ["Mountain"], "library": ["Mountain"] * 8}, {"library": ["Island"] * 8}])
        self.main()
        self.cast("Strike It Rich", mana=self.ids("Mountain", zone="battlefield", controller=0)[:1], spend={'R': 1})
        self.assertEqual(self.until_asked()["kind"], "priority")
        self.assertEqual(self.tokens(), [("Treasure Token", False)])


if __name__ == "__main__":
    unittest.main()
