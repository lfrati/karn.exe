"""Player-counter witnesses through the actual audited JSON client."""
from client_support import ClientTestCase


class PlayerCounterClientTests(ClientTestCase):
    def assert_public(self, expected):
        for viewer in range(len(expected)):
            self.assertEqual([p["counters"] for p in self.view(viewer)["players"]], expected)

    def test_live_fast_draws_loses_life_and_places_energy_on_its_controller(self):
        self.new([{"hand": ["Live Fast"], "battlefield": ["Swamp"] * 3}, {}])
        before = len(self.hand(0))
        self.cast("Live Fast", mana=self.ids("Swamp", zone="battlefield", controller=0)[:3], spend={'B': 3})
        self.pass_round()
        self.assertEqual(len(self.hand(0)), before + 1)
        self.assertEqual(self.engine.state["players"][0]["life"], 18)
        self.assert_public([{"energy": 2}, {}])
        events = [e for e in self.engine.state["events"] if e["event"] == "counters"]
        self.assertEqual([(e["on"], e["counter"], e["count"]) for e in events], [({"player": 0}, "energy", 2)])

    def test_glimmer_scry_waits_before_draw_and_energy_then_resolves_once(self):
        self.new([{"hand": ["Glimmer of Genius"], "battlefield": ["Island"] * 4}, {}])
        before = len(self.hand(0))
        self.cast("Glimmer of Genius", mana=self.ids("Island", zone="battlefield", controller=0)[:4], spend={'U': 4})
        self.pass_round()
        asked = self.engine.state["asked"]
        self.assertEqual((asked["kind"], asked["verb"]), ("arrange", "Scry"))
        self.assertEqual(len(self.hand(0)), before - 1)
        self.assert_public([{}, {}])
        self.choose({"top": asked["cards"], "away": []})
        self.assertEqual(len(self.hand(0)), before + 1)
        self.assert_public([{"energy": 2}, {}])
        self.assertFalse(self.engine.state["stack"])

    def test_caress_target_draw_life_and_poison_share_the_named_player(self):
        self.new([{"hand": ["Caress of Phyrexia"], "battlefield": ["Swamp"] * 5}, {}, {}])
        before = len(self.hand(2))
        self.cast("Caress of Phyrexia", [[{"player": 2}]], mana=self.ids("Swamp", zone="battlefield", controller=0)[:5], spend={'B': 5})
        self.pass_round()
        self.assertEqual(len(self.hand(2)), before + 3)
        self.assertEqual(self.engine.state["players"][2]["life"], 17)
        self.assert_public([{}, {}, {"poison": 3}])

    def test_prologue_mixed_player_batch_places_poison_and_then_draws(self):
        self.new([{"hand": ["Prologue to Phyresis"], "battlefield": ["Island"] * 2}, {}, {}])
        before = len(self.hand(0))
        self.cast("Prologue to Phyresis", mana=self.ids("Island", zone="battlefield", controller=0)[:2], spend={'U': 2})
        self.pass_round()
        self.assertEqual(len(self.hand(0)), before)
        self.assert_public([{}, {"poison": 1}, {"poison": 1}])

    def test_kelsien_full_delayed_death_trigger_gets_experience_and_changes_power(self):
        self.new([{"battlefield": ["Kelsien, the Plague"]}, {"battlefield": ["Llanowar Elves"]}])
        source, = self.ids("Kelsien, the Plague")
        target, = self.ids("Llanowar Elves")
        self.engine.act({"kind": "activate", "source": source, "ability": 2,
                         "targets": [[{"object": target}]], "plan": []})
        self.pass_round()
        self.assertNotIn(target, self.engine.state["battlefield"])
        self.assert_public([{}, {}])
        self.assertEqual(len(self.engine.state["stack"]), 1, "the delayed death trigger still needs to resolve")
        self.pass_round()
        self.assert_public([{"experience": 1}, {}])
        self.assertEqual(self.engine.state["objects"][str(source)]["power"], 3)
        self.assertEqual(self.engine.state["objects"][str(source)]["toughness"], 3)
