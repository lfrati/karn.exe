"""Modal face regressions through the actual Python/JSON client."""
from client_support import ClientTestCase


class ModalFaceClientTests(ClientTestCase):
    def land(self):
        card = next(o for o in self.hand(0) if self.object(o)["name"] == "Sink into Stupor")
        return {"kind": "play", "card": card, "face": 1}

    def front(self, target):
        card = next(o for o in self.hand(0) if self.object(o)["name"] == "Sink into Stupor")
        return {"kind": "cast", "card": card, "cast_as": {"face": 0}, "mana": {"U": 3}, "targets": [[target]],
                "plan": [{"source": o, "ability": 0} for o in self.ids("Island", zone="battlefield", controller=0)[:3]]}

    def test_explicit_front_cast_preserves_face_and_target_legality(self):
        self.new([{"hand": ["Sink into Stupor"], "battlefield": ["Island"] * 3 + ["Grizzly Bears"]},
                  {"battlefield": ["Grizzly Bears", "Island"]}, {}])
        self.assertEqual(self.land()["face"], 1)
        own = next(i for i in self.ids("Grizzly Bears") if self.object(i)["controller"] == 0)
        enemy = next(i for i in self.ids("Grizzly Bears") if self.object(i)["controller"] == 1)
        island = next(i for i in self.ids("Island") if self.object(i)["controller"] == 1)
        command = self.front({"object": enemy})
        self.assertEqual(command["cast_as"], {"face": 0})
        for target in (own, island):
            self.reject(0, {**command, "targets": [[{"object": target}]]})
        self.reject(0, {**command, "cast_as": {"face": 1}})
        self.reject(0, {**command, "cast_as": {"face": 2}})
        self.engine.act(command)
        self.assertEqual(self.object(self.engine.state["stack"][0])["name"], "Sink into Stupor")
        self.pass_round()
        self.assertNotIn(enemy, self.engine.state["battlefield"])
        self.assertEqual(self.view(1)["objects"][str(self.hand(1)[0])]["name"], "Grizzly Bears")
        grave = self.engine.state["players"][0]["graveyard"][0]
        self.assertEqual(self.object(grave)["name"], "Sink into Stupor")

    def test_back_land_paid_declined_entry_preserves_explicit_face(self):
        for pay in (False, True):
            with self.subTest(pay=pay):
                self.new([{"hand": ["Sink into Stupor", "Sink into Stupor"]}, {}, {}])
                command = self.land()
                self.assertEqual(command["face"], 1)
                for face in (0, 2, -1, "back"):
                    self.reject(0, {**command, "face": face})
                self.reject(0, {"kind": "play", "card": command["card"]})
                self.engine.act(command)
                self.assertEqual((self.engine.state["asked"]["kind"], self.engine.state["asked"]["player"]), ("may", 0))
                self.assertIn(command["card"], self.hand(0))
                self.reject(1, {"kind": "choose", "answer": "yes"})
                self.reject(0, {"kind": "choose", "answer": {"objects": []}})
                self.choose("yes" if pay else "no")
                permanent, = self.ids("Soporific Springs")
                self.assertEqual(self.object(permanent)["tapped"], not pay)
                self.assertEqual(self.engine.state["players"][0]["life"], 17 if pay else 20)
                self.reject(0, {"kind": "play", "card": self.hand(0)[0], "face": 1})
                if pay:
                    self.engine.act({"kind": "activate", "source": permanent, "ability": 1, "targets": [], "plan": []})
                    self.assertEqual(self.engine.state["players"][0]["pool"]["U"], 1)
                else:
                    self.reject(0, {"kind": "activate", "source": permanent, "ability": 1, "targets": [], "plan": []})

    def test_land_destruction_restores_physical_front_in_graveyard(self):
        self.new([{"hand": ["Sink into Stupor"], "battlefield": ["Strip Mine"]}, {}, {}])
        self.engine.act(self.land())
        self.choose("no")
        land, = self.ids("Soporific Springs")
        mine, = self.ids("Strip Mine")
        self.engine.act({"kind": "activate", "source": mine, "ability": 1, "targets": [[{"object": land}]], "plan": []})
        self.pass_round()
        self.assertNotIn(land, self.engine.state["battlefield"])
        for viewer in (0, 1, 2):
            state = self.view(viewer)
            names = [state["objects"][str(i)]["name"] for i in state["players"][0]["graveyard"]]
            self.assertIn("Sink into Stupor", names)
            self.assertNotIn("Soporific Springs", names)

    def test_insufficient_life_enters_tapped_without_offering_payment(self):
        self.engine.ask({"cmd": "new", "life": 2, "players": [
            {"hand": ["Sink into Stupor"], "library": ["Island"] * 8},
            {"library": ["Island"] * 8}, {"library": ["Island"] * 8}]})
        self.advance(lambda s: s["part"] == "PrecombatMain" and s["asked"]["kind"] == "priority")
        self.engine.act(self.land())
        self.assertEqual(self.engine.state["asked"]["kind"], "priority")
        permanent, = self.ids("Soporific Springs")
        self.assertTrue(self.object(permanent)["tapped"])
        self.assertEqual(self.engine.state["players"][0]["life"], 2)
        self.reject(0, {"kind": "choose", "answer": "yes"})

    def test_front_can_bounce_opponent_spell_while_land_play_is_unavailable(self):
        self.new([{"hand": ["Sink into Stupor"], "battlefield": ["Island"] * 3},
                  {"hand": ["Shock"], "battlefield": ["Mountain"]}, {}])
        self.engine.act({"kind": "pass"})
        shock = next(o for o in self.hand(1) if self.object(o)["name"] == "Shock")
        self.engine.act({"kind": "cast", "card": shock, "mana": {"R": 1}, "targets": [[{"player": 2}]],
                         "plan": [{"source": self.ids("Mountain", zone="battlefield", controller=1)[0], "ability": 0}]})
        self.engine.act({"kind": "pass"})
        self.engine.act({"kind": "pass"})
        self.reject(0, self.land())
        self.engine.act(self.front({"object": self.engine.state["stack"][0]}))
        self.pass_round()
        self.assertFalse(self.engine.state["stack"])
        self.assertEqual(self.view(1)["objects"][str(self.hand(1)[0])]["name"], "Shock")
        self.assertEqual(self.engine.state["players"][2]["life"], 20)

    def test_managed_whole_card_alias_and_color_identity_setup(self):
        for name in ("Sink into Stupor", "Sink into Stupor // Soporific Springs"):
            self.engine.ask({"cmd": "start", "format": "commander", "seed": 16,
                             "decks": [[name] + ["Island"] * 98, ["Island"] * 99, ["Island"] * 99],
                             "commanders": ["Talrand, Sky Summoner"] * 3})
            self.advance(lambda s: s["part"] == "PrecombatMain" and s["asked"]["kind"] == "priority")
            # Same physical card; the caller supplies its back land face.
            self.engine.act(self.land())
            self.choose("no")
            self.assertEqual(self.engine.state["players"][0]["life"], 40)
            self.assertEqual(len(self.ids("Soporific Springs")), 1)
        for decks, commanders in [
            ([["Soporific Springs"] + ["Island"] * 98, ["Island"] * 99], ["Talrand, Sky Summoner"] * 2),
            ([["Sink into Stupor"] + ["Plains"] * 98, ["Plains"] * 99], ["Isamaru, Hound of Konda"] * 2),
            ([["Sink into Stupor", "Sink into Stupor // Soporific Springs"] + ["Island"] * 97, ["Island"] * 99], ["Talrand, Sky Summoner"] * 2),
        ]:
            with self.assertRaises(SystemExit):
                self.engine.ask({"cmd": "start", "format": "commander", "decks": decks, "commanders": commanders})
