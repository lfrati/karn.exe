"""Path spending/trigger checks through the actual Python/JSON client."""
import unittest
from test_grant_client import GrantClientTests as Helpers


class ManaTriggerClientTests(unittest.TestCase):
    setUpClass = classmethod(Helpers.setUpClass.__func__)
    setUp = Helpers.setUp
    close_engine = Helpers.close_engine
    view = Helpers.view
    hand = Helpers.hand
    ids = Helpers.ids
    object = Helpers.object
    cast = Helpers.cast
    pass_round = Helpers.pass_round
    reject = Helpers.reject
    step = Helpers.step
    advance = Helpers.advance
    choose = Helpers.choose

    def setup_path(self, plains=False, card="Universal Automaton", strip=False):
        self.engine.ask({"cmd": "start", "format": "commander", "seed": 8363 if strip else 16,
                         "decks": [["Path of Ancestry", card] + (["Strip Mine"] if strip else []) + ["Plains"] * (96 if strip else 97),
                                   ["Plains"] * 99, ["Plains"] * 99],
                         "commanders": ["Isamaru, Hound of Konda"] * 3})
        self.advance(lambda s: s["part"] == "PrecombatMain" and s["asked"]["kind"] == "priority")
        path, = self.ids("Path of Ancestry")
        self.engine.act({"kind": "play", "card": path})
        path = next(i for i in self.ids("Path of Ancestry") if i in self.engine.state["battlefield"])
        self.assertTrue(self.object(path)["tapped"])
        self.reject(0, {"kind": "activate", "source": path, "ability": 1, "targets": [], "plan": []})
        turn = self.engine.state["turn"]
        self.advance(lambda s: s["turn"] > turn and s["active"] == 0 and s["part"] == "PrecombatMain")
        if plains:
            card = next(i for i in self.ids("Plains") if i in self.hand(0))
            self.engine.act({"kind": "play", "card": card})
        return path

    def activate(self, source, ability):
        self.engine.act({"kind": "activate", "source": source, "ability": ability, "targets": [], "plan": []})

    def scry(self):
        self.assertEqual(len(self.engine.state["stack"]), 2)
        self.pass_round()
        asked = self.engine.state["asked"]
        self.assertEqual((asked["kind"], asked["verb"], asked["player"]), ("arrange", "Scry", 0))
        top, = asked["cards"]
        for observer in (1, 2):
            view = self.view(observer)
            self.assertNotIn("cards", view["asked"])
            self.assertNotIn(str(top), view["objects"])
        self.reject(1, {"kind": "choose", "answer": {"top": [top], "away": []}})
        self.reject(0, {"kind": "choose", "answer": {"top": [top, top], "away": []}})
        self.choose({"top": [], "away": [top]})
        self.assertEqual(len(self.engine.state["stack"]), 1)
        for observer in (1, 2):
            self.assertNotIn(str(top), self.view(observer)["objects"])
        self.pass_round()
        self.assertTrue(any(self.object(i)["name"] == "Universal Automaton" for i in self.engine.state["battlefield"]))

    def test_authored_plan_pays_with_path_and_scry_is_private_above_changeling(self):
        path = self.setup_path()
        self.cast("Universal Automaton", spend={"restricted_after": [0]}, plan=[{"source":path,"ability":1}])
        self.assertTrue(self.object(path)["tapped"])
        self.scry()

    def test_pooled_marked_mana_is_distinct_from_identical_plain_white_mana(self):
        for marked in (False, True):
            with self.subTest(marked=marked):
                path = self.setup_path(plains=True)
                plain = next(i for i in self.ids("Plains") if i in self.engine.state["battlefield"])
                self.activate(plain, 0)
                self.engine.act({"kind":"activate","source":path,"ability":1,"targets":[]})
                pool = self.engine.state["players"][0]["pool"]
                self.assertEqual((pool["W"], len(pool["restricted"])), (1, 1))
                self.assertEqual(pool["restricted"][0]["mana"], "W")
                card=next(i for i in self.ids("Universal Automaton") if i in self.hand(0))
                self.reject(0, {"kind": "cast", "card": card, "targets": [], "plan": [], "mana": {"restricted": [0, 0]}})
                self.assertEqual(self.engine.state["players"][0]["pool"], pool)
                self.engine.act({"kind": "cast", "card": card, "targets": [], "plan": [],
                                 "mana": {"restricted": [0]} if marked else {"W": 1}})
                if marked:
                    self.assertEqual(self.engine.state["players"][0]["pool"]["W"], 1)
                    self.scry()
                else:
                    self.assertEqual(len(self.engine.state["stack"]), 1)
                    self.assertEqual(len(self.engine.state["players"][0]["pool"]["restricted"]), 1)
                    self.pass_round()

    def test_nonmatching_creature_and_noncreature_consume_path_mana_without_scry(self):
        for card in ("Perilous Myr", "Sol Ring"):
            with self.subTest(card=card):
                path = self.setup_path(plains=True, card=card)
                self.engine.act({"kind":"activate","source":path,"ability":1,"targets":[]})
                if card == "Perilous Myr":
                    plain = next(i for i in self.ids("Plains") if i in self.engine.state["battlefield"])
                    self.activate(plain, 0)
                spell=next(i for i in self.ids(card) if i in self.hand(0))
                self.engine.act({"kind": "cast", "card": spell, "targets": [], "plan": [],
                                 "mana": {"restricted": [0], "W": int(card == "Perilous Myr")}})
                self.assertEqual(len(self.engine.state["stack"]), 1)
                self.assertFalse(self.engine.state["players"][0]["pool"]["restricted"])
                self.pass_round()


    def test_pooled_path_mana_keeps_delayed_program_after_strip_mine_removes_source(self):
        path = self.setup_path(strip=True)
        mine = next(i for i in self.ids("Strip Mine") if i in self.hand(0))
        self.engine.act({"kind": "play", "card": mine})
        mine = next(i for i in self.ids("Strip Mine") if i in self.engine.state["battlefield"])
        self.engine.act({"kind":"activate","source":path,"ability":1,"targets":[]})
        self.engine.act({"kind": "activate", "source": mine, "ability": 1,
                         "targets": [[{"object": path}]], "plan": []})
        self.pass_round()
        self.assertNotIn(path, self.engine.state["battlefield"])
        self.assertEqual(len(self.engine.state["players"][0]["pool"]["restricted"]), 1)
        self.cast("Universal Automaton", spend={"restricted": [0]})
        self.scry()



del Helpers
