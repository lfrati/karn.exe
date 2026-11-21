"""Rules regressions through the real Python/JSON client and game-serve."""
import os
import subprocess
import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import play


class CommanderTriggerClientTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        if os.environ.get("GAME_SERVE_REVIEW"):
            return
        # Build only when no binary was supplied through GAME_SERVE_REVIEW.
        subprocess.run(["cargo", "build", "--profile", "fast", "-p", "game", "--bin", "game-serve"],
                       cwd=play.ROOT, check=True, stdout=subprocess.DEVNULL)

    def setUp(self):
        self.engine = play.Engine(audit=True)
        self.addCleanup(self.close_engine)

    def close_engine(self):
        self.engine.proc.stdin.close()
        self.engine.proc.wait(timeout=10)
        self.engine.proc.stdout.close()

    def new(self, seats):
        self.engine.ask({"cmd": "new", "players": seats})
        for _ in range(20):
            if self.engine.state["part"] == "PrecombatMain":
                return
            self.engine.act({"kind": "pass"})
        self.fail("fixture did not reach its main phase")

    def view(self, player):
        return self.engine.send({"cmd": "state", "viewer": player})

    def pass_round(self):
        for _ in self.engine.state["players"]:
            self.engine.act({"kind": "pass"})

    def ids(self, name, zone):
        return [i for i in self.engine.state[zone] if self.engine.state["objects"][str(i)]["name"]==name]

    def cast(self, name, mana=(), *, spend=None):
        card=next(i for i in self.engine.state["players"][0]["hand"] if self.engine.state["objects"][str(i)]["name"]==name)
        self.engine.act({"kind":"cast","card":card,"targets":[],"plan":[{"source":i,"ability":0} for i in mana], **({"mana": spend} if spend is not None else {})})

    def reject(self, player, move):
        state = self.engine.state
        with self.assertRaises(SystemExit):
            self.engine.send({"cmd": "act", "player": player, "move": move, "viewer": "asked",
                              "session_id": state["session_id"], "request_id": state["request_id"]})
        self.assertEqual(self.view(state["viewer"])["request_id"], state["request_id"])

    def partner(self, source="Alisaie Leveilleur", partner="Alphinaud Leveilleur", target=1):
        lands = ["Plains"] * 3 if source.startswith("Alisaie") else ["Island"] * 4
        self.new([{"hand": [source], "battlefield": lands, "library": ["Island"] * 5},
                  {"library": [partner, "Forest"]}, {"library": ["Swamp"] * 5}])
        self.cast(source,mana=self.ids("Plains" if source.startswith("Alisaie") else "Island","battlefield"), spend={"W": 3} if source.startswith("Alisaie") else {"U": 4})
        self.pass_round()
        asked = self.engine.state["asked"]
        self.assertEqual((asked["kind"], asked["player"]), ("targets", 0))
        command = {"kind": "targets", "targets": [[{"player": target}]]}
        self.reject(1, command)
        self.engine.act(command)
        self.pass_round()
        self.assertEqual((self.engine.state["asked"]["kind"], self.engine.state["asked"]["player"]), ("may", target))

    def test_r702_124j_both_partners_route_search_and_reveal_to_the_target_player(self):
        for source, partner in [("Alisaie Leveilleur", "Alphinaud Leveilleur"),
                                ("Alphinaud Leveilleur", "Alisaie Leveilleur")]:
            with self.subTest(source=source):
                self.partner(source, partner)
                self.reject(0, {"kind": "choose", "answer": "yes"})
                self.engine.act({"kind": "choose", "answer": "yes"})
                asked = self.engine.state["asked"]
                self.assertEqual((asked["kind"], asked["player"], asked["min"], asked["max"]), ("search", 1, 0, 1))
                wanted, = asked["among"]
                self.assertEqual(self.engine.state["objects"][str(wanted)]["name"], partner)
                cards = set(asked["cards"])
                self.assertEqual(len(cards), 2)
                other, = cards - {wanted}
                for observer in [0, 2]:
                    view = self.view(observer)
                    self.assertNotIn("cards", view["asked"])
                    self.assertNotIn("among", view["asked"])
                    self.assertTrue(cards.isdisjoint(map(int, view["objects"])))
                    self.assertFalse(view["observations"])
                self.reject(0, {"kind": "choose", "answer": {"objects": [wanted]}})
                self.reject(1, {"kind": "choose", "answer": {"objects": [other]}})
                self.engine.act({"kind": "choose", "answer": {"objects": [wanted]}})
                # Viewer 0 received the act response; the other viewers receive their own
                # pending disclosure. Reading it again must not replay the reveal/shuffle.
                views = [self.engine.state, self.view(1), self.view(2)]
                for viewer, view in enumerate(views):
                    revealed = [row for row in view["observations"] if row["kind"] == "revealed"]
                    self.assertEqual(len(revealed), 1)
                    self.assertEqual(revealed[0]["by"], 1)
                    self.assertEqual([(card["object"], card["name"]) for card in revealed[0]["cards"]], [(wanted, partner)])
                    reveal_events = [event for event in view["events"] if event["event"] == "revealed"]
                    self.assertEqual(len(reveal_events), 1)
                    self.assertEqual((reveal_events[0]["player"], reveal_events[0]["cards"]), (1, [wanted]))
                    self.assertNotIn(str(other), view["objects"])
                    self.assertEqual(view["players"][1]["library"], 1)
                    self.assertEqual(view["players"][1]["hand_size"], 1)
                    shuffles = [event for event in view["events"] if event["event"] == "shuffled"]
                    self.assertEqual(shuffles, [{"event": "shuffled", "player": 1, "by": 1}])
                    if viewer != 1:
                        self.assertNotIn("hand", view["players"][1])
                    else:
                        hand, = view["players"][1]["hand"]
                        self.assertEqual(view["objects"][str(hand)]["name"], partner)
                    repeated = self.view(viewer)
                    self.assertFalse(repeated["observations"])
                    self.assertFalse(repeated["events"])

    def test_r702_124j_decline_skips_search_and_shuffle(self):
        self.partner()
        self.engine.act({"kind": "choose", "answer": "no"})
        self.assertEqual(self.engine.state["asked"]["kind"], "priority")
        self.assertEqual(self.engine.state["players"][1]["library"], 2)
        self.assertEqual(self.engine.state["players"][1]["hand_size"], 0)
        self.assertFalse(self.engine.state["observations"])
        self.assertFalse(any(event["event"] in ("shuffled", "revealed") for event in self.engine.state["events"]))

    def test_r701_23b_fail_to_find_is_allowed_with_or_without_a_matching_partner(self):
        for present in [True, False]:
            with self.subTest(present=present):
                self.partner(partner="Alphinaud Leveilleur" if present else "Mountain")
                self.engine.act({"kind": "choose", "answer": "yes"})
                asked = self.engine.state["asked"]
                self.assertEqual(len(asked["among"]), int(present))
                self.assertEqual(asked["min"], 0)
                self.engine.act({"kind": "choose", "answer": {"objects": []}})
                self.assertEqual(self.engine.state["players"][1]["library"], 2)
                self.assertEqual(self.engine.state["players"][1]["hand_size"], 0)
                self.assertFalse(self.engine.state["observations"])
                self.assertEqual([event for event in self.engine.state["events"] if event["event"] == "shuffled"],
                                 [{"event": "shuffled", "player": 1, "by": 1}])

    def test_r601_2f_ordinal_discount_and_failed_casts_do_not_count(self):
        for controller in [0, 1]:
            with self.subTest(controller=controller):
                seats = [{"hand": ["Bonesplitter"] * 3, "battlefield": ["Plains"], "library": ["Island"] * 5},
                         {"library": ["Island"] * 5}]
                seats[controller].setdefault("battlefield", []).append("Alisaie Leveilleur")
                self.new(seats)
                first=next(i for i in self.engine.state["players"][0]["hand"] if self.engine.state["objects"][str(i)]["name"]=="Bonesplitter")
                self.reject(0,{"kind":"cast","card":first,"targets":[],"plan":[]})
                self.cast("Bonesplitter",mana=self.ids("Plains","battlefield"), spend={"W": 1})
                self.pass_round()
                lands = [obj for obj in self.engine.state["objects"].values() if obj["name"] == "Plains"]
                self.assertEqual(len(lands), 1)
                self.assertTrue(lands[0]["tapped"])
                if controller == 0:
                    self.cast("Bonesplitter")
                    self.pass_round()
                    self.assertEqual(sum(obj["name"] == "Bonesplitter" for key, obj in self.engine.state["objects"].items()
                                         if int(key) in self.engine.state["battlefield"]), 2)
                third = next(card for card in self.engine.state["players"][0]["hand"]
                             if self.engine.state["objects"][str(card)]["name"] == "Bonesplitter")
                self.reject(0, {"kind": "cast", "card": third, "targets": [], "plan": []})
                self.assertEqual(sum(self.engine.state["players"][0]["pool"][color] for color in "WUBRGC"), 0)

    def test_r702_108_full_lyse_client_discount_prowess_and_double_strike(self):
        self.new([{"hand": ["Bonesplitter"] * 2, "battlefield": ["Lyse Hext"], "library": ["Island"] * 5},
                  {"library": ["Island"] * 5}])
        source = next(card for card in self.engine.state["battlefield"]
                      if self.engine.state["objects"][str(card)]["name"] == "Lyse Hext")
        for power in [3, 4]:
            self.cast("Bonesplitter")
            self.assertEqual(len(self.engine.state["stack"]), 2)
            self.pass_round()
            self.assertEqual(self.engine.state["objects"][str(source)]["power"], power)
            self.pass_round()
        for _ in range(40):
            asked = self.engine.state["asked"]
            if self.engine.state["part"] == "PostcombatMain":
                break
            if asked["kind"] == "attackers":
                self.engine.act({"kind": "attack", "attacks": [[source, {"player": 1}]]})
            elif asked["kind"] == "blockers":
                self.engine.act({"kind": "block", "blocks": []})
            else:
                self.engine.act({"kind": "pass"})
        self.assertEqual(self.engine.state["part"], "PostcombatMain")
        self.assertEqual(self.engine.state["players"][1]["life"], 12, "two damage steps at power 4")

    def test_r903_two_commander_setup_remains_refused_through_client(self):
        with self.assertRaisesRegex(SystemExit, "one commander card name per player"):
            self.engine.ask({"cmd": "start", "format": "commander", "decks": [["Island"] * 99] * 2,
                             "commanders": [["Alisaie Leveilleur", "Alphinaud Leveilleur"], "Yargle, Glutton of Urborg"]})

    def test_r701_20_controller_authored_reveal_keeps_the_controller_actor(self):
        self.new([{"hand": ["Open the Armory"], "battlefield": ["Plains"] * 2,
                   "library": ["Bonesplitter", "Forest", "Island"]}, {"library": ["Swamp"] * 5}])
        self.cast("Open the Armory",mana=self.ids("Plains","battlefield"), spend={"W": 2})
        self.pass_round()
        asked = self.engine.state["asked"]
        self.assertEqual((asked["kind"], asked["player"]), ("search", 0))
        wanted, = asked["among"]
        self.engine.act({"kind": "choose", "answer": {"objects": [wanted]}})
        for view in [self.engine.state, self.view(1)]:
            revealed, = [row for row in view["observations"] if row["kind"] == "revealed"]
            self.assertEqual(revealed["by"], 0)
            self.assertEqual([(card["object"], card["name"]) for card in revealed["cards"]], [(wanted, "Bonesplitter")])
