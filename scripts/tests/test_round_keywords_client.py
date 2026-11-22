"""New keyword mechanisms through the existing explicit Python/JSON player boundary."""
import unittest
import play
from test_grant_client import GrantClientTests as Helpers


class RoundKeywordsClientTests(unittest.TestCase):
    setUpClass = classmethod(Helpers.setUpClass.__func__)
    setUp = Helpers.setUp
    close_engine = Helpers.close_engine
    new = Helpers.new
    view = Helpers.view
    hand = Helpers.hand
    ids = Helpers.ids
    object = Helpers.object
    pass_round = Helpers.pass_round
    choose = Helpers.choose
    reject = Helpers.reject
    cast = Helpers.cast
    step = Helpers.step
    advance = Helpers.advance

    def test_bloodthirst_enters_with_counters_after_real_player_damage(self):
        # 702.54a: damage to an opponent enables the entry replacement.
        self.new([{"hand": ["Lightning Bolt", "Bloodscale Prowler"], "battlefield": ["Mountain"] * 4}, {}])
        mountains = self.ids("Mountain", controller=0)
        self.cast("Lightning Bolt", [[{"player": 1}]], mana=mountains[:1], spend={"R": 1})
        self.pass_round()
        self.cast("Bloodscale Prowler", mana=mountains[1:], spend={"R": 3})
        self.pass_round()
        prowler, = self.ids("Bloodscale Prowler", zone="battlefield")
        self.assertEqual(self.object(prowler)["counters"], {"+1/+1": 1})
        self.assertEqual((self.object(prowler)["power"], self.object(prowler)["toughness"]), (4, 2))

    def test_affinity_accepts_the_authored_discounted_mana_payment(self):
        # 702.41a: five controlled artifacts reduce {7} to {2}.
        self.new([{"hand": ["Myr Enforcer"], "battlefield": ["Memnite"] * 5 + ["Island"] * 2}, {}])
        self.cast("Myr Enforcer", mana=self.ids("Island", controller=0), spend={"U": 2})
        self.pass_round()
        enforcer, = self.ids("Myr Enforcer", zone="battlefield")
        self.assertEqual(self.object(enforcer)["power"], 4)

    def test_cumulative_upkeep_is_one_correlated_explicit_payment(self):
        # 702.24a: add an age counter, then offer the full payment to its controller.
        self.new([{"battlefield": ["Mystic Remora", "Island", "Island"]}, {}])
        remora, = self.ids("Mystic Remora", zone="battlefield")
        turn = self.engine.state["turn"]
        self.advance(lambda s: s["turn"] > turn and s["active"] == 0 and s["part"] == "Upkeep")
        self.pass_round()
        self.assertEqual((self.engine.state["asked"]["kind"], self.engine.state["asked"]["player"]), ("pay", 0))
        self.assertEqual(self.object(remora)["counters"], {"age": 1})
        self.reject(1, {"kind": "choose", "answer": "no"})
        island = self.ids("Island", controller=0)[0]
        self.choose({"pay": {"plan": [{"source": island, "ability": 0}], "mana": {"U": 1}}})
        self.assertIn(remora, self.engine.state["battlefield"])
        turn = self.engine.state["turn"]
        self.advance(lambda s: s["turn"] > turn and s["active"] == 0 and s["part"] == "Upkeep")
        self.pass_round()
        self.assertEqual(self.object(remora)["counters"], {"age": 2})
        self.choose("no")
        self.assertFalse(self.ids("Mystic Remora", zone="battlefield"))

    def test_dash_existing_alternative_choice_grants_haste_and_delayed_return(self):
        # 702.109a: the spell creates the return independently of enters triggers.
        self.new([{"hand": ["Mardu Scout"], "battlefield": ["Mountain", "Mountain", "Torpor Orb"]}, {}])
        self.cast("Mardu Scout", mana=self.ids("Mountain", controller=0), spend={"R": 2}, cost={"alternative": 0})
        self.pass_round()
        scout, = self.ids("Mardu Scout", zone="battlefield")
        self.assertIn("haste", self.object(scout)["keywords"])
        self.advance(lambda s: s["asked"]["kind"] == "attackers")
        self.engine.act({"kind": "attack", "attacks": [[scout, {"player": 1}]]})
        self.advance(lambda s: s["part"] == "End")
        prompt = play.Seat(self.engine, 0).prompt()
        ability, = prompt["stack"]
        occurrence = prompt["objects"][ability]["occurrence"]
        self.assertIn("return", occurrence["text"])
        self.assertIn("Dash", occurrence["created_by"])
        self.pass_round()
        self.assertTrue(any(self.object(o)["name"] == "Mardu Scout" for o in self.hand(0)))

    def test_storm_keeps_earlier_cast_count_and_presents_its_trigger(self):
        # 702.40a: the later response doesn't increase the number of copies.
        self.new([{"hand": ["Nourish", "Weather the Storm"], "battlefield": ["Forest"] * 4},
                  {"hand": ["Nourish", "Nourish"], "battlefield": ["Forest"] * 4}])
        own = self.ids("Forest", controller=0)
        other = self.ids("Forest", controller=1)
        self.cast("Nourish", mana=own[:2], spend={"G": 2})
        self.pass_round()
        self.engine.act({"kind": "pass"})
        self.cast("Nourish", mana=other[:2], spend={"G": 2})
        self.pass_round()
        self.cast("Weather the Storm", mana=own[2:], spend={"G": 2})
        prompt = play.Seat(self.engine, 0).prompt()
        trigger = prompt["objects"][prompt["stack"][-1]]["occurrence"]
        self.assertIn("copy", trigger["text"].lower())
        self.engine.act({"kind": "pass"})
        self.cast("Nourish", mana=other[2:], spend={"G": 2})
        self.pass_round()
        self.pass_round()
        self.assertEqual(len(self.engine.state["stack"]), 3)
        for _ in range(3):
            self.pass_round()
        self.assertEqual([p["life"] for p in self.engine.state["players"]], [35, 32])


del Helpers
