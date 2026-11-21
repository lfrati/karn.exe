"""Lethal Scheme payment and retained connive through Python/JSON."""
import unittest
from test_grant_client import GrantClientTests as Helpers


class ConvokeConniveClientTests(unittest.TestCase):
    setUpClass = classmethod(Helpers.setUpClass.__func__)
    setUp = Helpers.setUp
    close_engine = Helpers.close_engine
    new = Helpers.new
    view = Helpers.view
    hand = Helpers.hand
    ids = Helpers.ids
    object = Helpers.object
    cast = Helpers.cast
    pass_round = Helpers.pass_round
    choose = Helpers.choose
    reject = Helpers.reject

    def setup_scheme(self, counters=False):
        self.new([{"hand": ["Lethal Scheme", "Sol Ring", "Island"],
                   "battlefield": ["Baleful Strix"] * 2 + ["Universal Automaton"] * 2 +
                   (["Hardened Scales", "Branching Evolution"] if counters else [])},
                  {"battlefield": ["Grizzly Bears"]}, {}])
        self.convokers = list(self.engine.state["battlefield"][:4])
        self.target = self.ids("Grizzly Bears")[0]
        black=self.ids("Baleful Strix")
        generic=self.ids("Universal Automaton")
        command={"kind":"cast","card":self.ids("Lethal Scheme")[0],"targets":[[{"object":self.target}]],"plan":[],
                 "convoke":["B","B","generic","generic"],"paying":[[i] for i in black+generic]}
        return command

    def finish_connives(self):
        while self.engine.state["asked"]["kind"] == "objects":
            asked = self.engine.state["asked"]
            self.assertEqual(asked["player"], 0)
            if asked["verb"] == "Connive":
                self.choose({"objects": [asked["among"][0]]})
            else:
                self.assertEqual(asked["verb"], "Discard")
                land = next((i for i in asked["among"] if self.object(i)["name"] in ("Forest", "Island")), asked["among"][0])
                self.choose({"objects": [land]})
        self.assertEqual(self.engine.state["asked"]["kind"], "priority")

    def test_authored_colored_generic_payment_and_private_discard(self):
        command = self.setup_scheme()
        for player in (1, 2):
            self.assertNotIn("hand", self.view(player)["players"][0])
        self.engine.act(command)
        self.assertTrue(all(self.object(i)["tapped"] for i in self.convokers))
        self.assertEqual(self.engine.state["players"][0]["pool"]["B"], 0)
        self.pass_round()
        self.assertNotIn(self.target, self.engine.state["battlefield"])
        asked = self.engine.state["asked"]
        self.assertEqual((asked["kind"], asked["verb"]), ("objects", "Connive"))
        self.reject(1, {"kind": "choose", "answer": {"objects": [self.convokers[0]]}})
        self.reject(0, {"kind": "choose", "answer": {"objects": self.convokers[:2]}})
        first = self.convokers[0]
        self.choose({"objects": [first]})
        self.assertEqual(self.engine.state["asked"]["verb"], "Discard")
        hand = self.hand(0)
        for player in (1, 2):
            view = self.view(player)
            self.assertNotIn("among", view["asked"])
            self.assertTrue(all(str(i) not in view["objects"] for i in hand))
        card = next(i for i in hand if self.object(i)["name"] == "Sol Ring")
        self.reject(1, {"kind": "choose", "answer": {"objects": [card]}})
        self.reject(0, {"kind": "choose", "answer": {"objects": [self.target]}})
        self.choose({"objects": [card]})
        self.assertEqual(self.object(first)["counters"]["+1/+1"], 1)
        grave = next(i for i in self.engine.state["players"][0]["graveyard"] if self.object(i)["name"] == "Sol Ring")
        for player in (1, 2):
            self.assertEqual(self.view(player)["objects"][str(grave)]["name"], "Sol Ring")
        self.finish_connives()
        self.assertEqual(sum(self.object(i)["counters"].get("+1/+1", 0) for i in self.convokers), 1)

    def test_explicit_wrong_color_duplicate_and_excess_payments_do_not_commit(self):
        command = self.setup_scheme()
        black = self.ids("Baleful Strix")
        generic = self.ids("Universal Automaton")
        bad = [
            {**command, "paying": [[generic[0]], [black[0]], [black[1]], [generic[1]]]},
            {**command, "paying": [[black[0]], [black[0]], [generic[0]], [generic[1]]]},
            {**command, "convoke": ["B", "B", "generic", "generic", "generic"]},
            {**command, "convoke": ["C", "B", "generic", "generic"]},
        ]
        for move in bad:
            self.reject(0, move)
            self.assertIn(command["card"], self.hand(0))
            self.assertFalse(any(self.object(i)["tapped"] for i in self.convokers))
        self.engine.act(command)
        self.pass_round()
        self.finish_connives()

    def test_destroyed_convoker_still_draws_discards_without_a_counter_recipient(self):
        command = self.setup_scheme()
        gone = self.convokers[0]
        command["targets"] = [[{"object": gone}]]
        self.engine.act(command)
        self.pass_round()
        self.assertNotIn(gone, self.engine.state["battlefield"])
        # The departed original is still among the controller's ordering choices.
        self.assertIn(gone, self.engine.state["asked"]["among"])
        self.choose({"objects": [gone]})
        self.assertEqual(self.engine.state["asked"]["verb"], "Discard")
        card = next(i for i in self.hand(0) if self.object(i)["name"] == "Sol Ring")
        self.choose({"objects": [card]})
        self.assertFalse(any(self.object(i)["counters"] for i in self.convokers if i != gone))
        self.finish_connives()

    def test_counter_replacement_question_resumes_once(self):
        command = self.setup_scheme(counters=True)
        first = self.convokers[0]
        self.engine.act(command)
        self.pass_round()
        self.choose({"objects": [first]})
        card = next(i for i in self.hand(0) if self.object(i)["name"] == "Sol Ring")
        before = self.engine.state["players"][0]["library"]
        self.choose({"objects": [card]})
        asked = self.engine.state["asked"]
        self.assertEqual((asked["kind"], asked["player"]), ("replacement", 0))
        effect = next(e for e in asked["among"] if self.object(e["source"])["name"] == "Branching Evolution")
        answer = {"replacement": {key: effect[key] for key in ("source", "ability", "part")}}
        self.reject(1, {"kind": "choose", "answer": answer})
        self.choose(answer)
        self.assertEqual(self.object(first)["counters"]["+1/+1"], 3)
        self.assertEqual(self.engine.state["players"][0]["library"], before)
        self.assertEqual(self.engine.state["asked"]["verb"], "Connive")
        self.finish_connives()

    def test_changed_controller_owns_connive_draw_and_private_discard(self):
        self.new([{"hand": ["Lethal Scheme", "Sol Ring", "Island"],
                   "battlefield": ["Baleful Strix"] * 2 + ["Universal Automaton"] * 2},
                  {"hand": ["Act of Aggression", "Sol Ring", "Island"],
                   "battlefield": ["Mountain"] * 5 + ["Grizzly Bears"]}, {}])
        original = next(i for i in self.ids("Baleful Strix") if self.object(i)["controller"] == 0)
        target = self.ids("Grizzly Bears")[0]
        black=self.ids("Baleful Strix"); generic=self.ids("Universal Automaton")
        command={"kind":"cast","card":self.ids("Lethal Scheme")[0],"targets":[[{"object":target}]],"plan":[],
                 "convoke":["B","B","generic","generic"],"paying":[[i] for i in black+generic]}
        self.engine.act(command)
        self.engine.act({"kind": "pass"})
        self.cast("Act of Aggression", [[{"object": original}]], mana=self.ids("Mountain"), spend={'R': 5})
        self.pass_round()
        self.assertEqual(self.object(original)["controller"], 1)
        self.pass_round()
        while self.engine.state["asked"]["player"] == 0:
            asked = self.engine.state["asked"]
            self.assertEqual(asked["kind"], "objects")
            pick = asked["among"][0] if asked["verb"] == "Connive" else next(i for i in asked["among"] if self.object(i)["name"] in ("Island", "Forest"))
            self.choose({"objects": [pick]})
        asked = self.engine.state["asked"]
        self.assertEqual((asked["kind"], asked["verb"], asked["player"]), ("objects", "Discard", 1))
        for observer in (0, 2):
            self.assertNotIn("among", self.view(observer)["asked"])
            self.assertNotIn("hand", self.view(observer)["players"][1])
        card = next(i for i in self.hand(1) if self.object(i)["name"] == "Sol Ring")
        self.reject(0, {"kind": "choose", "answer": {"objects": [card]}})
        self.choose({"objects": [card]})
        self.assertEqual(self.object(original)["counters"]["+1/+1"], 1)
        self.assertEqual(self.engine.state["asked"]["kind"], "priority")

    def test_suspended_failed_payment_reversal_preserves_mana_but_not_convoke_taps(self):
        self.new([{"hand": ["Lethal Scheme"], "battlefield": ["Baleful Strix"] * 2 + ["Empowered Autogenerator"]},
                  {"battlefield": ["Grizzly Bears"]}, {}])
        card = self.ids("Lethal Scheme")[0]
        source = self.ids("Empowered Autogenerator")[0]
        convokers = self.ids("Baleful Strix")
        target = self.ids("Grizzly Bears")[0]
        self.engine.act({"kind": "cast", "card": card, "targets": [[{"object": target}]],
                         "convoke": ["B", "B"], "paying": [[i] for i in convokers],
                         "plan": [{"source": source, "ability": 1, "paying": [], "adding": []}]})
        self.assertEqual(self.engine.state["asked"]["kind"], "mana")
        self.assertFalse(any(self.object(i)["tapped"] for i in convokers))
        self.choose({"mana": ["B"]})
        self.assertEqual(self.engine.state["asked"]["kind"], "payment")
        self.reject(1, {"kind": "reverse_payment"})
        self.engine.act({"kind": "reverse_payment"})
        self.assertIn(card, self.hand(0))
        self.assertFalse(self.engine.state["stack"])
        self.assertFalse(any(self.object(i)["tapped"] for i in convokers))
        self.assertTrue(self.object(source)["tapped"])
        self.assertEqual(self.object(source)["counters"]["charge"], 1)
        self.assertEqual(self.engine.state["players"][0]["pool"]["B"], 1)
        self.assertIn(target, self.engine.state["battlefield"])

    def test_mana_paid_scheme_has_no_invented_convokers(self):
        self.new([{"hand": ["Lethal Scheme"], "battlefield": ["Swamp"] * 4},
                  {"battlefield": ["Grizzly Bears"]}, {}])
        target = self.ids("Grizzly Bears")[0]
        self.cast("Lethal Scheme",[[{"object":target}]],mana=self.ids("Swamp"),convoke=[], spend={'B': 4})
        before = len(self.hand(0))
        self.pass_round()
        self.assertNotIn(target, self.engine.state["battlefield"])
        self.assertEqual(len(self.hand(0)), before)
        self.assertEqual(self.engine.state["asked"]["kind"], "priority")


del Helpers
