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

    def test_improvise_taps_artifacts_and_keeps_the_colored_mana_payment(self):
        # 702.126a-b: six artifacts pay {6}; the Swamp still pays Fen Hauler's {B}.
        self.new([{"hand": ["Fen Hauler"], "battlefield": ["Darksteel Relic"] * 6 + ["Swamp"]}, {}])
        artifacts = self.ids("Darksteel Relic", controller=0)
        self.cast("Fen Hauler", mana=self.ids("Swamp", controller=0), spend={"B": 1}, improvise=6, paying=[artifacts])
        self.assertTrue(all(self.object(o)["tapped"] for o in artifacts))
        self.pass_round()
        self.assertEqual(len(self.ids("Fen Hauler", zone="battlefield")), 1)

    def test_improvise_combines_with_jegantha_mana_for_the_remaining_colored_cost(self):
        # 702.126a: artifacts pay {6}; Jegantha's restricted black unit pays {B}, not generic.
        self.new([{"hand": ["Fen Hauler"], "battlefield": ["Darksteel Relic"] * 6 + ["Jegantha, the Wellspring"]}, {}])
        artifacts = self.ids("Darksteel Relic", controller=0)
        source, = self.ids("Jegantha, the Wellspring", controller=0)
        card, = [o for o in self.hand(0) if self.object(o)["name"] == "Fen Hauler"]
        before = self.view(0)
        with self.assertRaisesRegex(play.Refused, "does not pay that cost"):
            self.engine.act({"kind": "cast", "card": card, "targets": [], "improvise": 5, "paying": [artifacts[:5]],
                             "plan": [{"source": source, "ability": 1}], "mana": {"restricted_after": [0, 2]}})
        self.assertEqual(self.view(0), before)
        self.cast("Fen Hauler", plan=[{"source": source, "ability": 1}], spend={"restricted_after": [2]}, improvise=6, paying=[artifacts])
        self.assertTrue(self.object(source)["tapped"])
        self.assertTrue(all(self.object(o)["tapped"] for o in artifacts))
        self.pass_round()
        self.assertEqual(len(self.ids("Fen Hauler", zone="battlefield")), 1)

    def test_improvise_cannot_reuse_an_artifact_activated_for_mana(self):
        # 702.126a: an artifact tapped during mana production is no longer untapped.
        self.new([{"hand": ["Fen Hauler"], "battlefield": ["Darksteel Relic"] * 4 + ["Sol Ring", "Swamp"]}, {}])
        artifacts = self.ids("Darksteel Relic", controller=0)
        ring, = self.ids("Sol Ring", controller=0)
        swamp, = self.ids("Swamp", controller=0)
        card, = [o for o in self.hand(0) if self.object(o)["name"] == "Fen Hauler"]
        move = {"kind": "cast", "card": card, "targets": [], "improvise": 5, "paying": [artifacts + [ring]],
                "plan": [{"source": ring, "ability": 0}, {"source": swamp, "ability": 0}], "mana": {"C": 1, "B": 1}}
        self.reject(0, move)
        self.assertFalse(self.object(ring)["tapped"])
        move.update(improvise=4, paying=[artifacts], mana={"C": 2, "B": 1})
        self.engine.act(move)
        self.assertTrue(self.object(ring)["tapped"])
        self.assertTrue(all(self.object(o)["tapped"] for o in artifacts))

    def test_evolve_trigger_uses_current_creature_sizes(self):
        # 702.100a: matching power still evolves when the entering toughness is greater.
        self.new([{"hand": ["Grizzly Bears"], "battlefield": ["Shambleshark", "Forest", "Forest"]}, {}])
        source, = self.ids("Shambleshark", zone="battlefield")
        self.cast("Grizzly Bears", mana=self.ids("Forest", controller=0), spend={"G": 2})
        self.pass_round()
        self.assertEqual(len(self.engine.state["stack"]), 1)
        self.pass_round()
        self.assertEqual(self.object(source)["counters"], {"+1/+1": 1})

    def test_mentor_uses_the_existing_explicit_trigger_target_choice(self):
        # 702.134a: attacking creatures of equal power cannot be chosen for mentor.
        self.new([{"battlefield": ["Sunhome Stalwart", "Memnite"]}, {}])
        source, = self.ids("Sunhome Stalwart", zone="battlefield")
        small, = self.ids("Memnite", zone="battlefield")
        self.advance(lambda state: state["asked"]["kind"] == "attackers")
        self.engine.act({"kind": "attack", "attacks": [[source, {"player": 1}], [small, {"player": 1}]]})
        self.assertEqual(self.engine.state["asked"]["kind"], "targets")
        self.reject(0, {"kind": "targets", "targets": [[{"object": source}]]})
        self.engine.act({"kind": "targets", "targets": [[{"object": small}]]})
        self.pass_round()
        self.assertEqual(self.object(small)["counters"], {"+1/+1": 1})

    def test_soulshift_chooses_its_target_before_the_optional_return(self):
        # 702.46a: a Kindred instant with changeling is a Spirit card, not a creature.
        self.new([{"hand": ["Murder"], "battlefield": ["Venerable Kumo"] + ["Swamp"] * 3,
                   "graveyard": ["Crib Swap"]}, {}])
        kumo, = self.ids("Venerable Kumo", zone="battlefield")
        spirit, = self.engine.state["players"][0]["graveyard"]
        self.cast("Murder", [[{"object": kumo}]], mana=self.ids("Swamp", controller=0), spend={"B": 3})
        self.pass_round()
        self.assertEqual((self.engine.state["asked"]["kind"], self.engine.state["asked"]["player"]), ("targets", 0))
        self.reject(1, {"kind": "targets", "targets": [[{"object": spirit}]]})
        self.engine.act({"kind": "targets", "targets": [[{"object": spirit}]]})
        self.pass_round()
        self.assertEqual(self.engine.state["asked"]["kind"], "may")
        self.choose("yes")
        self.assertTrue(any(self.object(o)["name"] == "Crib Swap" for o in self.hand(0)))

    def test_transmute_discards_then_searches_through_the_native_boundary(self):
        # 702.53a: generated hand activation uses the existing explicit mana and search choices.
        self.new([{"hand": ["Muddle the Mixture"], "battlefield": ["Island"] * 3,
                   "library": ["Grizzly Bears"] + ["Forest"] * 8}, {}])
        card, = [o for o in self.hand(0) if self.object(o)["name"] == "Muddle the Mixture"]
        self.engine.act({"kind": "activate", "source": card, "ability": 2, "targets": [],
                         "plan": [{"source": o, "ability": 0} for o in self.ids("Island", controller=0)], "mana": {"U": 3}})
        self.assertTrue(any(self.object(o)["name"] == "Muddle the Mixture" for o in self.engine.state["players"][0]["graveyard"]))
        self.pass_round()
        self.assertEqual(self.engine.state["asked"]["kind"], "search")
        bear, = self.engine.state["asked"]["among"]
        self.assertEqual(self.object(bear)["name"], "Grizzly Bears")
        self.reject(1, {"kind": "choose", "answer": {"objects": [bear]}})
        self.choose({"objects": [bear]})
        self.assertTrue(any(self.object(o)["name"] == "Grizzly Bears" for o in self.hand(0)))

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
        self.new([{"hand": ["Mystic Remora"], "battlefield": ["Island"] * 3}, {}])
        self.cast("Mystic Remora", mana=self.ids("Island", controller=0)[:1], spend={"U": 1})
        self.pass_round()
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
        self.new([{"hand": ["Mardu Scout"], "battlefield": ["Mountain", "Mountain"]}, {}])
        self.cast("Mardu Scout", mana=self.ids("Mountain", controller=0), spend={"R": 2}, cost={"alternative": 0})
        self.pass_round()
        self.assertFalse(self.engine.state["stack"], "dash schedules a delayed ability rather than triggering on entry")
        scout, = self.ids("Mardu Scout", zone="battlefield")
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
        trigger, = [prompt["objects"][o]["occurrence"] for o in prompt["stack"]
                    if prompt["objects"][o]["occurrence"]["kind"] == "triggered"]
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
