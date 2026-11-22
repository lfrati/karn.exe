"""The Karn adapter in process (docs/mgn/adapter-protocol.md): its capabilities validate against the
protocol schema; a position start projects exactly; `default` with `until` is served by the
explicit null actions and stops where the protocol says; a question without a null answer is named;
steps are counted across turns; the combat damage steps are told apart (510.4); the projection
carries the keyword abilities the engine does not model, a spell's printed power,
toughness and keywords on the stack (208.3, 702.34a), and a token's definition (111.4)."""
import os
from pathlib import Path
import unittest
from unittest.mock import patch

from mgn.adapters.karn import KarnAdapter, Refusal
from mgn.canonical import loads
from mgn.oracle import Oracle
from mgn.schema import validate as schema_validate
from mgn.state import State
from mgn.validate import validate
from pinned import RULES_EFFECTIVE

ROOT = Path(__file__).resolve().parents[2]
ENGINE = Path(os.environ.get("KARN_CONFORMANCE_ENGINE", ROOT / "target/fast/game-serve"))
FIXTURES = ROOT / "tests/fixtures/mgn"
ORACLE = Oracle()
RULES_SHA = "8d860e451f20f38865b725b42d82feb714c725373dd8f3b32b8652b3eeb070ca"
ORACLE_SHA = "afda2e8de024387848267601d749d170c3f36fe98a982291509091b7d5670bc5"


def no_random(key):
    return None


def position_record(cards: dict, zones: dict, turn: dict, pending: dict, objects: dict | None = None, assumptions: dict | None = None) -> dict:
    """A tier-2 record at a position: `cards` maps ids to (name, owner)."""
    table = {}
    for card_id, (name, owner) in cards.items():
        oracle_id = ORACLE.by_name(name)
        table[card_id] = {"owner": owner, "oracle": oracle_id, "name": ORACLE.name(oracle_id), "faces": ORACLE.digests(oracle_id)}
    full = {zone: [] for zone in ("A.library", "A.hand", "A.graveyard", "B.library", "B.hand", "B.graveyard", "battlefield", "stack", "exile", "command")}
    full.update(zones)
    state = {"turn": turn, "pending": pending, "result": {"kind": "unfinished"}, "players": {"A": {"life": 20}, "B": {"life": 20}},
             "zones": full, "objects": objects or {}}
    start = {"kind": "position", "state": state}
    if assumptions:
        start["assumptions"] = assumptions
    record = {"mgn": "1", "tier": 2,
              "pins": {"rules": {"effective": RULES_EFFECTIVE, "sha256": RULES_SHA},
                       "oracle": {"source": "scryfall-oracle-cards", "updated_at": "2026-10-11T09:01:56.463+00:00", "sha256": ORACLE_SHA}},
              "game": {"format": "custom", "life": 20, "hand_size": 7, "seats": ["A", "B"]},
              "author": {"kind": "tool", "name": "test", "version": "1"}, "cards": table, "start": start, "transitions": []}
    errors = [str(p) for p in validate(record, ORACLE) if p.severity == "error"]
    assert not errors, errors
    return record


def setup_of(record: dict) -> dict:
    return {key: record[key] for key in ("tier", "pins", "game", "cards", "start")}


MAIN_A = {"number": 3, "step": "precombat_main", "active": "A"}
PRIORITY_A = {"seat": "A", "kind": "priority"}


@unittest.skipUnless(ENGINE.is_file(), "build target/fast/game-serve to exercise the Karn adapter")
class AdapterTests(unittest.TestCase):
    def setUp(self):
        self.adapter = KarnAdapter(ENGINE, ORACLE)

    def tearDown(self):
        self.adapter.end()

    def test_capabilities_validate_against_the_protocol_schema(self):
        capabilities = self.adapter.capabilities()
        self.assertEqual([str(e) for e in schema_validate({"id": 1, "capabilities": capabilities}, "adapter-protocol.schema.json")], [])
        self.assertEqual(capabilities["starts"], ["decks", "position"])
        self.assertEqual(capabilities["random"], ["shuffle", "random_order", "coin", "starting_player"])
        for kind in ("declare_attackers", "choose_first", "companion", "mulligan", "bottom", "opening_hand_action", "concede",
                     "assign_combat_damage", "discard", "separate", "skip", "spell_destination", "command_zone"):
            self.assertIn(kind, capabilities["kinds"])

    def test_a_position_start_projects_exactly(self):
        record = loads((FIXTURES / "scenario-announcements" / "record.json").read_text(encoding="utf-8"))
        reply = self.adapter.setup(setup_of(record))
        self.assertEqual([str(e) for e in schema_validate({"id": 2, "ok": reply}, "adapter-protocol.schema.json")], [])
        expected = State.from_record(record, []).explicit(ORACLE)
        self.assertEqual(reply["state"], expected)

    def test_priority_scenarios_install_zones_and_facts_without_playing_setup_actions(self):
        # CR 117.3a, 503.1, 504.1: the supplied state is already at priority.
        # Setup does not repeat untapping/drawing, empty mana or enter permanents.
        cards = {"bear": ("Grizzly Bears", "B"), "forest": ("Forest", "A"),
                 "top": ("Island", "A"), "bottom": ("Mountain", "A"), "libB": ("Forest", "B"),
                 "hand": ("Giant Growth", "A"), "dead": ("Hill Giant", "B"),
                 "exiled": ("Grizzly Bears", "B")}
        zones = {"battlefield": ["bear", "forest"], "A.library": ["top", "bottom"],
                 "B.library": ["libB"], "A.hand": ["hand"], "B.graveyard": ["dead"], "exile": ["exiled"]}
        for step in ("upkeep", "draw", "precombat_main", "begin_combat", "end_combat", "postcombat_main", "end"):
            with self.subTest(step=step):
                sample = position_record(cards, zones, {"number": 3, "step": step, "active": "A"}, PRIORITY_A,
                    # CR 122.1a: the counter makes printed 2/2 Bears 3/3.
                    objects={"bear": {"controller": "A", "tapped": True, "counters": {"+1/+1": 1}, "damage": 1,
                                      "power": 3, "toughness": 3}},
                    assumptions={"summoning_sick": ["bear"]})
                sample["start"]["state"]["players"]["A"].update(life=17, mana={"G": 1})
                with patch.object(self.adapter, "_send_move", side_effect=AssertionError("setup played an action")):
                    reply = self.adapter.setup(setup_of(sample))
                self.assertEqual(reply["state"], State.from_record(sample, []).explicit(ORACLE))

    def test_first_main_phase_also_uses_direct_setup(self):
        sample = position_record({"card": ("Forest", "A")}, {"A.hand": ["card"]},
                                 {"number": 1, "step": "precombat_main", "active": "A"}, PRIORITY_A)
        with patch.object(self.adapter, "_send_move", side_effect=AssertionError("setup played an action")):
            reply = self.adapter.setup(setup_of(sample))
        self.assertEqual(reply["state"], State.from_record(sample, []).explicit(ORACLE))

    def test_default_with_until_sends_explicit_null_actions_and_stops_at_the_target_or_past_it(self):
        cards = {"fA": ("Forest", "A"), "fB": ("Forest", "B"), "libA": ("Island", "A"), "libB": ("Island", "B")}
        record = position_record(cards, {"battlefield": ["fA", "fB"], "A.library": ["libA"], "B.library": ["libB"]}, MAIN_A, PRIORITY_A,
                                 objects={"fB": {"controller": "B"}})
        self.adapter.setup(setup_of(record))
        until = {"turn": 3, "step": "end", "active": "A", "seat": "A"}
        reply = self.adapter.default(until, no_random)
        # explicit null actions pass both seats' priority through combat (508.1a asks nobody: no creature can
        # attack) and stops at the active seat's first priority in the end step (513.1)
        self.assertEqual(reply["state"]["turn"], {"number": 3, "step": "end", "active": "A", "occurrence": 1})
        self.assertEqual(reply["state"]["pending"], PRIORITY_A)
        self.assertNotIn("asked", reply)
        reply = self.adapter.default(until, no_random)
        # at the target, each null answer is one pass: past the end step the stretch stops in the
        # next turn, at the first priority there (502.4: none in the untap step; 503.1: the upkeep)
        self.assertEqual(reply["state"]["turn"], {"number": 4, "step": "upkeep", "active": "B", "occurrence": 1})
        self.assertEqual(reply["state"]["pending"], {"seat": "B", "kind": "priority"})
        self.assertEqual(self.adapter.turn_counts[(4, "upkeep")], 1)
        self.assertFalse(any(key[0] == 4 and key[1] in ("end", "precombat_main") for key in self.adapter.turn_counts))

    def test_a_question_without_a_null_answer_is_named_and_left_pending(self):
        cards = {f"f{i}": ("Forest", "A") for i in range(9)}
        cards["lib"] = ("Island", "A")
        cards["libB"] = ("Island", "B")
        record = position_record(cards, {"A.hand": [f"f{i}" for i in range(9)], "A.library": ["lib"], "B.library": ["libB"]}, MAIN_A, PRIORITY_A)
        self.adapter.setup(setup_of(record))
        reply = self.adapter.default({"turn": 4, "step": "upkeep", "active": "B", "seat": "B"}, no_random)
        # nine cards in hand at cleanup: two must be discarded (514.1), a question with no null answer
        self.assertEqual(reply["asked"], "discard")
        self.assertEqual(reply["state"]["pending"], {"seat": "A", "kind": "discard", "about": {"count": 2}})
        self.assertEqual(reply["state"]["turn"]["step"], "cleanup")

    def _seance_thragtusk(self):
        # Séance's next-end-step exile (603.7) makes Thragtusk leave, so its
        # second ability creates a Beast even though the Spirit token ceases
        # to exist (113.7a, 603.6c, 704.5d). No choice is made by that ability.
        cards = {"seance": ("Séance", "A"), "thr": ("Thragtusk", "A"),
                 "growth": ("Giant Growth", "A"), "forest": ("Forest", "A"),
                 "libA": ("Plains", "A"), "libB": ("Forest", "B")}
        record = position_record(cards, {"battlefield": ["seance", "forest"], "A.hand": ["growth"],
                                        "A.graveyard": ["thr"], "A.library": ["libA"], "B.library": ["libB"]},
                                 MAIN_A, PRIORITY_A)
        self.adapter.setup(setup_of(record))
        upkeep = {"turn": 4, "step": "upkeep", "active": "B", "seat": "B"}
        reply = self.adapter.default(upkeep, no_random)
        self.assertEqual(reply["state"]["pending"]["kind"], "choose_targets")
        self.adapter.act(1, "A", {"kind": "choose_targets", "targets": [["thr"]]}, no_random)
        reply = self.adapter.default(upkeep, no_random)
        self.assertEqual(reply["state"]["pending"]["kind"], "optional")
        self.adapter.act(2, "A", {"kind": "optional", "yes": True}, no_random)
        until = {"turn": 4, "step": "end", "active": "B", "seat": "A"}
        self.adapter.default(until, no_random)  # Séance's delayed exile; B has priority.
        reply = self.adapter.default(until, no_random)  # B passes to A.
        self.assertEqual(reply["state"]["pending"], PRIORITY_A)
        self.assertEqual(reply["state"]["objects"]["T1"]["token"]["name"], "Thragtusk")
        return until

    def test_uncommitted_departed_provenance_drops_when_the_whole_ability_is_implied(self):
        until = self._seance_thragtusk()
        reply = self.adapter.default(until, no_random)
        state = reply["state"]
        self.assertEqual(state["departed"]["T1"]["token"]["name"], "Thragtusk")
        self.assertEqual(state["objects"][state["zones"]["stack"][-1]]["source"], "T1")
        self.assertNotIn("T1", state["zones"]["battlefield"])
        self.assertEqual(self.adapter.committed_departed, {})
        for _ in range(5):
            state = self.adapter.default(until, no_random)["state"]
            if not state["zones"]["stack"]: break
        # No recorded decision used the intermediate source. It consumes no ID.
        self.assertNotIn("departed", state)
        self.assertEqual(state["objects"]["T1"]["token"]["name"], "Beast Token")
        self.assertEqual(state["players"]["A"]["life"], 25)
        self.assertNotIn("T2", state["objects"])
        self.assertEqual(self.adapter.committed_tokens, {})

    def test_a_recorded_departed_token_keeps_its_number_and_ability_source(self):
        until = self._seance_thragtusk()
        # This actual decision makes the living Thragtusk a written boundary.
        self.adapter.act(3, "A", {"kind": "cast", "card": "growth", "targets": [["T1"]],
                                 "pay": {"abilities": [{"source": "forest", "ability": {"rule": "305.6"},
                                                         "adding": ["G"]}]}}, no_random)
        departed_trigger = None
        for _ in range(8):
            reply = self.adapter.default(until, no_random)
            state = reply["state"]
            if "T1" not in state["objects"] and state["zones"]["stack"]:
                departed_trigger = state["objects"][state["zones"]["stack"][-1]]
            if not state["zones"]["stack"] and state["pending"] == PRIORITY_A:
                break
        self.assertIsNotNone(departed_trigger)
        self.assertEqual(departed_trigger["source"], "T1")
        self.assertEqual(departed_trigger["ability"], {"face": 0, "paragraph": 2})
        self.assertNotIn("T1", state["objects"])
        self.assertEqual(state["objects"]["T2"]["token"]["name"], "Beast Token")
        self.assertEqual(state["players"]["A"]["life"], 25)

    def test_single_default_and_projection_expose_exact_departed_definition_without_skipping(self):
        self._seance_thragtusk()
        state = self.adapter.default(None, no_random)["state"]
        self.assertTrue(state["zones"]["stack"])
        self.assertEqual(state["departed"]["T1"]["owner"], "A")
        self.assertEqual(state["departed"]["T1"]["token"]["subtypes"], ["Beast", "Spirit"])
        request = self.adapter.state["request_id"]
        self.assertEqual(self.adapter.project(2)["state"], state)
        self.assertEqual(self.adapter.state["request_id"], request)
        self.assertEqual(self.adapter.committed_tokens, {})
        with self.assertRaisesRegex(Refusal, "not an object the engine has now"):
            self.adapter._target("T1")

    def test_transient_source_does_not_conceal_another_objects_projection_failure(self):
        for hard_error in (True, False):
            with self.subTest(hard_error=hard_error):
                until = self._seance_thragtusk()
                original = self.adapter._object_facts
                def fail_exile(engine_id, zone, problems):
                    facts = original(engine_id, zone, problems)
                    if zone == "exile":
                        if hard_error:
                            raise Refusal("unsupported", "unsupported_fact", "other object's facts unavailable")
                        self.adapter.unsupported_paths.append("objects.thr.test_fact")
                    return facts
                # Exile is projected after the stack: an earlier missing token
                # source must not suppress this distinct failure.
                with patch.object(self.adapter, "_object_facts", side_effect=fail_exile):
                    if hard_error:
                        with self.assertRaises(Refusal) as raised:
                            self.adapter.default(until, no_random)
                        self.assertEqual(raised.exception.code, "unsupported_fact")
                        self.assertIn("other object's facts", raised.exception.message)
                    else:
                        reply = self.adapter.default(until, no_random)
                        self.assertIn("objects.thr.test_fact", reply["unsupported"])
                        self.assertTrue(reply["state"]["zones"]["stack"])
                        self.assertIn("departed", reply["state"])

    def test_the_combat_damage_steps_are_told_apart(self):
        cards = {"knight": ("Youthful Knight", "A"), "bears": ("Grizzly Bears", "B"), "libA": ("Plains", "A"), "libB": ("Forest", "B")}
        record = position_record(cards, {"battlefield": ["knight", "bears"], "A.library": ["libA"], "B.library": ["libB"]}, MAIN_A, PRIORITY_A,
                                 objects={"bears": {"controller": "B"}})
        self.adapter.setup(setup_of(record))
        reply = self.adapter.default({"turn": 3, "step": "declare_attackers", "active": "A", "seat": "A"}, no_random)
        self.assertEqual(reply["state"]["pending"], {"seat": "A", "kind": "declare_attackers"})
        self.adapter.act(1, "A", {"kind": "declare_attackers", "attacks": [["knight", "B"]]}, no_random)
        reply = self.adapter.default({"turn": 3, "step": "first_strike_damage", "active": "A", "seat": "A"}, no_random)
        # the stretch stops at B's block declaration (509.1a), a question of another kind
        self.assertEqual(reply["state"]["pending"], {"seat": "B", "kind": "declare_blockers"})
        reply = self.adapter.default({"turn": 3, "step": "first_strike_damage", "active": "A", "seat": "A"}, no_random)
        # no blocks; the knight has first strike, so the first combat damage step is its own (510.4):
        # B loses 2 life in it and the active seat gets priority there
        self.assertEqual(reply["state"]["turn"], {"number": 3, "step": "first_strike_damage", "active": "A", "occurrence": 1})
        self.assertEqual(reply["state"]["pending"], PRIORITY_A)
        self.assertEqual(reply["state"]["players"]["B"]["life"], 18)
        reply = self.adapter.default({"turn": 3, "step": "combat_damage", "active": "A", "seat": "A"}, no_random)
        # the second combat damage step: the bears are not in combat, nothing more is dealt
        self.assertEqual(reply["state"]["turn"], {"number": 3, "step": "combat_damage", "active": "A", "occurrence": 1})
        self.assertEqual(reply["state"]["players"]["B"]["life"], 18)

    def test_keywords_the_engine_does_not_model_are_projected_from_the_text(self):
        cards = {"showoff": ("Slickshot Show-Off", "A"), "libA": ("Mountain", "A"), "libB": ("Forest", "B")}
        record = position_record(cards, {"battlefield": ["showoff"], "A.library": ["libA"], "B.library": ["libB"]}, MAIN_A, PRIORITY_A)
        reply = self.adapter.setup(setup_of(record))
        # plot is printed on the card (702.170) though the engine tracks only flying and haste
        self.assertEqual(reply["state"]["objects"]["showoff"]["keywords"], ["flying", "haste", "plot {1}{R}"])
        self.assertEqual(reply["state"]["objects"]["showoff"]["power"], 1)

    def test_a_spell_on_the_stack_keeps_its_printed_power_toughness_and_keywords(self):
        cards = {"showoff": ("Slickshot Show-Off", "A"), "m0": ("Mountain", "A"), "m1": ("Mountain", "A"), "libA": ("Mountain", "A"), "libB": ("Forest", "B")}
        record = position_record(cards, {"A.hand": ["showoff"], "battlefield": ["m0", "m1"], "A.library": ["libA"], "B.library": ["libB"]}, MAIN_A, PRIORITY_A)
        self.adapter.setup(setup_of(record))
        pay = {"abilities": [{"source": "m0", "ability": {"rule": "305.6"}, "adding": ["R"]}, {"source": "m1", "ability": {"rule": "305.6"}, "adding": ["R"]}]}
        reply = self.adapter.act(1, "A", {"kind": "cast", "card": "showoff", "pay": pay}, no_random)
        self.assertEqual(reply["state"]["zones"]["stack"], ["showoff"])
        spell = reply["state"]["objects"]["showoff"]
        self.assertEqual((spell["power"], spell["toughness"]), (1, 2))  # 208.3: printed values on the stack
        self.assertEqual(spell["keywords"], ["flying", "haste", "plot {1}{R}"])  # printed ability, including its cost (702.170)
        self.assertEqual(reply["state"]["pending"], PRIORITY_A)

    def test_ward_cost_is_part_of_the_projected_ability(self):
        # Cactusfolk Sureshot's Oracle ward cost is {2} (702.21a), not a bare name.
        cards = {"cactus": ("Cactusfolk Sureshot", "A"), "libA": ("Island", "A"), "libB": ("Forest", "B")}
        record = position_record(cards, {"battlefield": ["cactus"], "A.library": ["libA"], "B.library": ["libB"]}, MAIN_A, PRIORITY_A)
        reply = self.adapter.setup(setup_of(record))
        self.assertEqual(reply["state"]["objects"]["cactus"]["keywords"], ["reach", "ward {2}"])

    def test_a_token_is_defined_as_the_creating_effect_defines_it(self):
        cards = {"splicer": ("Blade Splicer", "A"), "p0": ("Plains", "A"), "p1": ("Plains", "A"), "p2": ("Plains", "A"),
                 "libA": ("Plains", "A"), "libB": ("Forest", "B")}
        record = position_record(cards, {"A.hand": ["splicer"], "battlefield": ["p0", "p1", "p2"], "A.library": ["libA"], "B.library": ["libB"]}, MAIN_A, PRIORITY_A)
        self.adapter.setup(setup_of(record))
        pay = {"abilities": [{"source": p, "ability": {"rule": "305.6"}, "adding": ["W"]} for p in ("p0", "p1", "p2")]}
        self.adapter.act(1, "A", {"kind": "cast", "card": "splicer", "pay": pay}, no_random)
        reply = self.adapter.default({"turn": 3, "step": "precombat_main", "active": "A", "seat": "A"}, no_random)
        # the spell resolved, its trigger went on the stack as S1 (603.3) and A has priority
        self.assertEqual(reply["state"]["zones"]["stack"], ["S1"])
        self.assertEqual(reply["state"]["objects"]["S1"]["source"], "splicer")
        reply = self.adapter.default({"turn": 3, "step": "precombat_main", "active": "A", "seat": "A"}, no_random)
        token = reply["state"]["objects"]["T1"]
        self.assertEqual(token["token"], {"name": "Phyrexian Golem Token", "types": ["artifact", "creature"], "colors": [],
                                          "subtypes": ["Golem", "Phyrexian"], "power": 3, "toughness": 3})
        self.assertEqual(token["keywords"], ["first strike"])  # granted by Blade Splicer, not part of the definition
        self.assertEqual(token["controller"], "A")
        self.assertIn("T1", reply["state"]["zones"]["battlefield"])


class DefaultBoundaryTests(unittest.TestCase):
    def test_normal_default_stops_at_new_stack_or_other_question_without_skipping(self):
        until = {"turn": 4, "step": "end", "active": "B", "seat": "A"}
        for kind in ("priority", "mana"):
            adapter = KarnAdapter(ENGINE, ORACLE)
            adapter.seats = ["A", "B"]
            adapter.state = {"asked": {"kind": "priority", "player": 0}, "stack": []}
            def advance(*args, **kwargs):
                adapter.state = {"asked": {"kind": kind, "player": 0}, "stack": [1]}
            with patch.object(adapter, "_require_game"), \
                 patch.object(adapter, "_at_or_past", return_value=True), \
                 patch.object(adapter, "_send_move", side_effect=advance) as send, \
                 patch.object(adapter, "_reply_state", return_value={"state": {"sentinel": True}}):
                self.assertEqual(adapter.default(until, no_random)["state"], {"sentinel": True})
                self.assertEqual(send.call_count, 1)


class KeywordProjectionTests(unittest.TestCase):
    """Characteristic boundaries of the audit interface (113.2c, 613.1f, 702.21)."""

    def setUp(self):
        self.adapter = KarnAdapter(ENGINE, ORACLE)

    def test_current_instances_survive_and_removed_printed_instances_stay_removed(self):
        item = {"held": ["ward {2}", "ward {2}", "ward {3}"],
                "base": {"keywords": ["ward {2}"]}, "abilities": []}
        self.assertEqual(self.adapter._keywords(1, item, "one"), ["ward {2}", "ward {2}", "ward {3}"])
        item.update(held=[], abilities_removed=True)
        self.assertEqual(self.adapter._keywords(1, item, "one"), [])
        # Layer 6 removal does not erase an ability granted afterward.
        item["abilities"] = [{"origin": {"kind": "given"}, "text": "Exhaust — {R}: Draw a card."}]
        self.assertEqual(self.adapter._keywords(1, item, "one"), ["exhaust"])

    def test_token_cost_and_wrapper_body_are_both_retained(self):
        # 111.3, 702.21a, 702.169a: a token's definition retains the ward cost
        # and the actual ability Solved introduces, not merely its keyword name.
        item = {"base": {"name": "Example", "types": ["Enchantment"], "keywords": [],
                         "rows": ["Ward—Pay 3 life.", "Solved — Creatures you control get +1/+0."]}}
        definition = self.adapter._token_definition(item)
        self.assertEqual(definition["keywords"], ["solved", "ward—Pay 3 life."])
        # 113.2c: keyword paragraphs still count when another ability is
        # referenced by paragraph; do not renumber Solved as paragraph one.
        self.assertEqual(definition["text"], "Ward—Pay 3 life.\nSolved — Creatures you control get +1/+0.")


if __name__ == "__main__":
    unittest.main()


def _mana(*lands: str, mana: str = "G") -> dict:
    """A payment activating the intrinsic mana ability of each basic land (305.6)."""
    return {"abilities": [{"source": land, "ability": {"rule": "305.6"}, "adding": [mana]} for land in lands]}


MAIN_A_UNTIL = {"turn": 3, "step": "precombat_main", "active": "A", "seat": "A"}


@unittest.skipUnless(ENGINE.is_file(), "build target/fast/game-serve to exercise the Karn adapter")
class ResolutionKindTests(unittest.TestCase):
    """Every resolution-time question kind of spec 5.4 the engine asks maps both ways, with the
    answer's consequences as the rules state them."""

    def setUp(self):
        self.adapter = KarnAdapter(ENGINE, ORACLE)

    def tearDown(self):
        self.adapter.end()

    def start(self, cards, zones, objects=None, assumptions=None, turn=MAIN_A, pending=PRIORITY_A):
        record = position_record(cards, zones, turn, pending, objects=objects, assumptions=assumptions)
        return self.adapter.setup(setup_of(record))

    def act(self, n, seat, do):
        return self.adapter.act(n, seat, do, no_random)

    def default(self, until=MAIN_A_UNTIL):
        return self.adapter.default(until, no_random)

    def test_pay_is_asked_while_the_spell_resolves_and_a_payment_keeps_the_spell(self):
        # Dazzling Denial: "Counter target spell unless its controller pays {2}" (118.12)
        cards = {"bears": ("Grizzly Bears", "A"), "f1": ("Forest", "A"), "f2": ("Forest", "A"), "f3": ("Forest", "A"), "f4": ("Forest", "A"),
                 "denial": ("Dazzling Denial", "B"), "i1": ("Island", "B"), "i2": ("Island", "B"), "la": ("Forest", "A"), "lb": ("Island", "B")}
        self.start(cards, {"A.hand": ["bears"], "B.hand": ["denial"], "battlefield": ["f1", "f2", "f3", "f4", "i1", "i2"], "A.library": ["la"], "B.library": ["lb"]},
                   objects={"i1": {"controller": "B"}, "i2": {"controller": "B"}})
        self.act(1, "A", {"kind": "cast", "card": "bears", "pay": _mana("f1", "f2")})
        self.default({**MAIN_A_UNTIL, "seat": "B"})  # A passes: B receives priority (117.3b)
        self.act(2, "B", {"kind": "cast", "card": "denial", "targets": [["bears"]], "pay": _mana("i1", "i2", mana="U")})
        reply = self.default()  # B passes, A passes: Dazzling Denial resolves (117.4) and asks A to pay
        reply = self.default() if reply["state"]["pending"]["kind"] == "priority" else reply
        self.assertEqual(reply["state"]["pending"], {"seat": "A", "kind": "pay", "about": {"object": "denial"}})
        self.assertEqual(reply["state"]["zones"]["stack"], ["bears", "denial"])  # the resolving spell is still on the stack (spec 4.2)
        reply = self.act(3, "A", {"kind": "pay", "pay": _mana("f3", "f4")})
        # paid: the Bears spell is not countered; Denial finished resolving (608.2n)
        self.assertEqual(reply["state"]["zones"]["stack"], ["bears"])
        self.assertEqual(reply["state"]["zones"]["B.graveyard"], ["denial"])
        reply = self.default()
        self.assertIn("bears", reply["state"]["zones"]["battlefield"])

    def test_values_chosen_as_a_permanent_enters_are_asked_about_it_and_retained(self):
        # Diamond Mare: "As this creature enters, choose a color." (614.1c; spec 3.1 `chosen`)
        # Multiversal Passage: "As this land enters, choose a basic land type. Then you may pay 2 life. If you don't, it enters tapped."
        cards = {"mare": ("Diamond Mare", "A"), "passage": ("Multiversal Passage", "A"), "i1": ("Island", "A"), "i2": ("Island", "A"), "la": ("Forest", "A"), "lb": ("Island", "B")}
        self.start(cards, {"A.hand": ["mare", "passage"], "battlefield": ["i1", "i2"], "A.library": ["la"], "B.library": ["lb"]})
        self.act(1, "A", {"kind": "cast", "card": "mare", "pay": _mana("i1", "i2", mana="U")})
        reply = self.default()
        self.assertEqual(reply["state"]["pending"], {"seat": "A", "kind": "choose_value", "about": {"object": "mare", "of": "color"}})
        self.assertEqual(reply["asked"], "choose_value")  # no null answer (spec 5.6)
        reply = self.act(2, "A", {"kind": "choose_value", "value": "red"})
        self.assertIn("mare", reply["state"]["zones"]["battlefield"])
        self.assertEqual(reply["state"]["objects"]["mare"]["chosen"], {"color": "red"})
        reply = self.act(3, "A", {"kind": "play_land", "card": "passage"})
        self.assertEqual(reply["state"]["pending"], {"seat": "A", "kind": "choose_value", "about": {"object": "passage", "of": "basic_land_type"}})
        reply = self.act(4, "A", {"kind": "choose_value", "value": "Mountain"})
        self.assertEqual(reply["state"]["pending"], {"seat": "A", "kind": "optional", "about": {"object": "passage"}})
        reply = self.act(5, "A", {"kind": "optional", "yes": True})
        self.assertEqual(reply["state"]["players"]["A"]["life"], 18)  # 2 life paid (119.4)
        passage = reply["state"]["objects"]["passage"]
        self.assertEqual((passage["chosen"], passage["tapped"]), ({"basic_land_type": "Mountain"}, False))
        self.assertEqual(passage["subtypes"], ["Mountain"])  # "This land is the chosen type" (305.7)

    def test_creature_type_choice_uses_rules_spelling_on_both_sides_of_native_io(self):
        # Cavern's entry choice (614.1c); C’tan is a creature type in CR 205.3m.
        cards = {"cavern": ("Cavern of Souls", "A"), "la": ("Forest", "A"), "lb": ("Island", "B")}
        self.start(cards, {"A.hand": ["cavern"], "A.library": ["la"], "B.library": ["lb"]})
        reply = self.act(1, "A", {"kind": "play_land", "card": "cavern"})
        self.assertEqual(reply["state"]["pending"]["about"]["of"], "creature_type")
        move = self.adapter._move({"kind": "choose_value", "value": "C’tan"})
        self.assertEqual(move["answer"]["value"], {"creature_type": "C'tan"})
        reply = self.act(2, "A", {"kind": "choose_value", "value": "C’tan"})
        self.assertEqual(reply["state"]["objects"]["cavern"]["chosen"], {"creature_type": "C’tan"})

    def test_a_triggered_mana_ability_asks_its_mana_choice_about_its_source(self):
        # Fertile Ground: "Whenever enchanted land is tapped for mana, its controller adds an additional one mana of any color."
        # A triggered mana ability resolves immediately without the stack (605.1b, 605.3b)
        cards = {"fertile": ("Fertile Ground", "A"), "f1": ("Forest", "A"), "f2": ("Forest", "A"), "f3": ("Forest", "A"), "la": ("Forest", "A"), "lb": ("Island", "B")}
        self.start(cards, {"A.hand": ["fertile"], "battlefield": ["f1", "f2", "f3"], "A.library": ["la"], "B.library": ["lb"]})
        self.act(1, "A", {"kind": "cast", "card": "fertile", "targets": [["f1"]], "pay": _mana("f2", "f3")})
        reply = self.default()
        self.assertEqual(reply["state"]["objects"]["fertile"]["attached_to"], "f1")
        reply = self.act(2, "A", {"kind": "activate", "source": "f1", "ability": {"rule": "305.6"}, "adding": ["G"]})
        self.assertEqual(reply["state"]["pending"], {"seat": "A", "kind": "choose_mana", "about": {"object": "fertile"}})
        self.assertEqual(reply["state"]["zones"]["stack"], [])
        reply = self.act(3, "A", {"kind": "choose_mana", "mana": ["R"]})
        self.assertEqual(reply["state"]["players"]["A"]["mana"], {"G": 1, "R": 1})
        self.assertEqual(reply["state"]["pending"], PRIORITY_A)

    def test_proliferate_asks_which_holders_and_adds_a_counter_to_each(self):
        # Steady Progress: "Proliferate. Draw a card." (701.34a: choose any number of permanents and players with counters)
        cards = {"sp": ("Steady Progress", "A"), "i1": ("Island", "A"), "i2": ("Island", "A"), "i3": ("Island", "A"), "ab": ("Grizzly Bears", "A"),
                 "la": ("Forest", "A"), "lb": ("Island", "B"), "bb": ("Grizzly Bears", "B")}
        self.start(cards, {"A.hand": ["sp"], "battlefield": ["i1", "i2", "i3", "ab", "bb"], "A.library": ["la"], "B.library": ["lb"]},
                   objects={"ab": {"counters": {"+1/+1": 1}}, "bb": {"controller": "B"}})
        self.act(1, "A", {"kind": "cast", "card": "sp", "pay": _mana("i1", "i2", "i3", mana="U")})
        reply = self.default()
        self.assertEqual(reply["state"]["pending"], {"seat": "A", "kind": "holders", "about": {"object": "sp"}})
        reply = self.act(2, "A", {"kind": "holders", "holders": ["ab"]})
        self.assertEqual(reply["state"]["objects"]["ab"]["counters"], {"+1/+1": 2})
        self.assertEqual(reply["state"]["objects"]["ab"]["power"], 4)
        self.assertEqual(reply["state"]["zones"]["A.hand"], ["la"])  # then the draw

    def test_an_aura_entering_from_an_effect_asks_what_it_enchants(self):
        # Rescue Skiff: "When this Spacecraft enters, return target creature or enchantment card from your graveyard to the battlefield."
        # An Aura put onto the battlefield without being cast: its controller chooses what it enchants as it enters (303.4f)
        cards = {"skiff": ("Rescue Skiff", "A"), "rancor": ("Rancor", "A"), "bears": ("Grizzly Bears", "A"), "bb": ("Grizzly Bears", "B"), "la": ("Forest", "A"), "lb": ("Island", "B")}
        cards.update({f"p{i}": ("Plains", "A") for i in range(6)})
        self.start(cards, {"A.hand": ["skiff"], "A.graveyard": ["rancor"], "battlefield": ["bears", "bb"] + [f"p{i}" for i in range(6)], "A.library": ["la"], "B.library": ["lb"]},
                   objects={"bb": {"controller": "B"}})
        self.act(1, "A", {"kind": "cast", "card": "skiff", "pay": _mana(*[f"p{i}" for i in range(6)], mana="W")})
        reply = self.default()
        self.assertEqual(reply["state"]["pending"], {"seat": "A", "kind": "choose_targets", "about": {"object": "S1"}})
        self.act(2, "A", {"kind": "choose_targets", "targets": [["rancor"]]})
        reply = self.default()
        self.assertEqual(reply["state"]["pending"], {"seat": "A", "kind": "attach", "about": {"object": "rancor"}})
        self.assertEqual(reply["asked"], "attach")
        reply = self.act(3, "A", {"kind": "attach", "to": "bb"})
        self.assertEqual(reply["state"]["objects"]["rancor"]["attached_to"], "bb")
        self.assertEqual(reply["state"]["objects"]["bb"]["power"], 4)  # Rancor: enchanted creature gets +2/+0 and has trample
        self.assertIn("trample", reply["state"]["objects"]["bb"]["keywords"])

    def test_a_copy_of_a_spell_asks_new_targets_as_it_is_created(self):
        # Twincast: "Copy target instant or sorcery spell. You may choose new targets for the copy." (707.10, 707.10c)
        cards = {"twin": ("Twincast", "A"), "bolt": ("Lightning Bolt", "A"), "m1": ("Mountain", "A"), "i1": ("Island", "A"), "i2": ("Island", "A"),
                 "la": ("Forest", "A"), "lb": ("Island", "B"), "bb": ("Grizzly Bears", "B"), "ab": ("Grizzly Bears", "A")}
        self.start(cards, {"A.hand": ["twin", "bolt"], "battlefield": ["m1", "i1", "i2", "bb", "ab"], "A.library": ["la"], "B.library": ["lb"]},
                   objects={"bb": {"controller": "B"}})
        self.act(1, "A", {"kind": "cast", "card": "bolt", "targets": [["bb"]], "pay": _mana("m1", mana="R")})
        self.act(2, "A", {"kind": "cast", "card": "twin", "targets": [["bolt"]], "pay": _mana("i1", "i2", mana="U")})
        reply = self.default()  # both pass: Twincast resolves and the copy's targets may be changed (707.10c)
        # the copy is put onto the stack only once its targets are chosen (707.10c): the question is about Twincast
        self.assertEqual(reply["state"]["pending"], {"seat": "A", "kind": "change_targets", "about": {"object": "twin"}})
        self.assertEqual(reply["state"]["zones"]["stack"], ["bolt", "twin"])
        reply = self.act(3, "A", {"kind": "change_targets", "targets": [["B"]]})
        copy = reply["state"]["objects"]["S1"]
        self.assertEqual((copy["kind"], copy["copy_of"], copy["controller"], copy["targets"]), ("copy", "bolt", "A", [["B"]]))
        self.assertEqual(reply["state"]["zones"]["stack"], ["bolt", "S1"])
        self.assertEqual(reply["state"]["zones"]["A.graveyard"], ["twin"])
        reply = self.default()  # the copy resolves: 3 damage to B (608.2n: a copy ceases to exist)
        self.assertEqual(reply["state"]["players"]["B"]["life"], 17)
        self.assertNotIn("S1", reply["state"]["objects"])

    def test_an_effect_that_changes_a_target_asks_the_new_one(self):
        # Bolt Bend: "Change the target of target spell or ability with a single target." (115.7)
        cards = {"bolt": ("Lightning Bolt", "A"), "bend": ("Bolt Bend", "B"), "m1": ("Mountain", "A"), "la": ("Forest", "A"), "lb": ("Island", "B"),
                 "bb": ("Grizzly Bears", "B"), "ab": ("Grizzly Bears", "A")}
        cards.update({f"bm{i}": ("Mountain", "B") for i in range(4)})
        self.start(cards, {"A.hand": ["bolt"], "B.hand": ["bend"], "battlefield": ["m1", "bb", "ab"] + [f"bm{i}" for i in range(4)], "A.library": ["la"], "B.library": ["lb"]},
                   objects={**{f"bm{i}": {"controller": "B"} for i in range(4)}, "bb": {"controller": "B"}})
        self.act(1, "A", {"kind": "cast", "card": "bolt", "targets": [["bb"]], "pay": _mana("m1", mana="R")})
        self.default({**MAIN_A_UNTIL, "seat": "B"})
        self.act(2, "B", {"kind": "cast", "card": "bend", "targets": [["bolt"]], "pay": _mana(*[f"bm{i}" for i in range(4)], mana="R")})
        reply = self.default()
        reply = self.default() if reply["state"]["pending"]["kind"] == "priority" else reply
        self.assertEqual(reply["state"]["pending"], {"seat": "B", "kind": "change_targets", "about": {"object": "bolt"}})
        self.assertEqual(reply["asked"], "change_targets")  # a mandatory change has no null answer (115.7a; spec 5.6)
        reply = self.act(3, "B", {"kind": "change_targets", "targets": [["ab"]]})
        self.assertEqual(reply["state"]["objects"]["bolt"]["targets"], [["ab"]])
        reply = self.default()  # the Bolt resolves on its new target
        self.assertEqual(reply["state"]["zones"]["A.graveyard"], ["bolt", "ab"])

    def test_a_cast_offered_while_resolving_is_asked_and_the_cost_waived(self):
        # Planetarium of Wan Shi Tong: "{1}, {T}: Scry 2. Whenever you scry or surveil, look at the top card of your
        # library. You may cast that card without paying its mana cost." (608.2g, 118.9)
        cards = {"plan": ("Planetarium of Wan Shi Tong", "A"), "m1": ("Mountain", "A"), "bolt": ("Lightning Bolt", "A"), "c2": ("Forest", "A"), "c3": ("Forest", "A"), "lb": ("Island", "B")}
        self.start(cards, {"battlefield": ["plan", "m1"], "A.library": ["bolt", "c2", "c3"], "B.library": ["lb"]})
        self.act(1, "A", {"kind": "activate", "source": "plan", "ability": {"face": 0, "paragraph": 1}, "pay": _mana("m1", mana="R")})
        reply = self.default()
        self.assertEqual(reply["state"]["pending"], {"seat": "A", "kind": "arrange", "about": {"object": "S1", "cards": ["bolt", "c2"]}})
        reply = self.act(2, "A", {"kind": "arrange", "top": ["bolt", "c2"], "away": []})  # scry: the Bolt stays on top (701.22a)
        # the scry ability finished resolving (608.2n) and the trigger went on the stack (603.3): S2, S1 never recurring
        self.assertEqual(reply["state"]["zones"]["stack"], ["S2"])
        reply = self.default()
        self.assertEqual(reply["state"]["pending"], {"seat": "A", "kind": "cast_during_resolution", "about": {"object": "S2"}})
        self.assertEqual(reply["state"]["zones"]["A.library"], ["bolt", "c2", "c3"])
        # The record cannot establish its own waiver: another card has no offer.
        with self.assertRaises(Refusal) as refused:
            self.act(3, "A", {"kind": "cast_during_resolution", "card": "c2",
                              "cost": {"kind": "alternative", "effect": "without_paying"}})
        self.assertEqual(refused.exception.code, "illegal_cost")
        self.assertEqual(self.adapter._reply_state()["state"], reply["state"])
        reply = self.act(3, "A", {"kind": "cast_during_resolution", "card": "bolt", "targets": [["B"]],
                                  "cost": {"kind": "alternative", "effect": "without_paying"}})
        # cast while the trigger resolved, with no priority in between (608.2g); the trigger then finished resolving
        # and left the stack (608.2n) before the next decision, so the Bolt is alone there
        self.assertEqual(reply["state"]["zones"]["stack"], ["bolt"])
        self.assertEqual(reply["state"]["objects"]["bolt"]["targets"], [["B"]])
        self.assertEqual(reply["state"]["objects"]["bolt"]["cast"]["cost"],
                         {"kind": "alternative", "effect": "without_paying"})
        self.assertEqual(reply["state"]["players"]["A"]["mana"], {})  # nothing paid (118.9)
        reply = self.default()
        self.assertEqual(reply["state"]["players"]["B"]["life"], 17)

    def test_combat_damage_is_assigned_among_blockers_and_with_trample_to_the_player(self):
        # 510.1c, 510.1d: a blocked creature with two blockers divides its damage; 702.19b: trample's excess to the player
        cards = {"ball": ("Ball Lightning", "A"), "giant": ("Hill Giant", "A"), "la": ("Forest", "A"), "lb": ("Island", "B"),
                 "b1": ("Grizzly Bears", "B"), "b2": ("Grizzly Bears", "B"), "b3": ("Grizzly Bears", "B")}
        self.start(cards, {"battlefield": ["ball", "giant", "b1", "b2", "b3"], "A.library": ["la"], "B.library": ["lb"]},
                   objects={k: {"controller": "B"} for k in ("b1", "b2", "b3")})
        self.default({"turn": 3, "step": "declare_attackers", "active": "A", "seat": "A"})
        self.act(1, "A", {"kind": "declare_attackers", "attacks": [["ball", "B"], ["giant", "B"]]})
        self.default({"turn": 3, "step": "declare_blockers", "active": "A", "seat": "B"})
        self.act(2, "B", {"kind": "declare_blockers", "blocks": [["b1", "ball"], ["b2", "giant"], ["b3", "giant"]]})
        reply = self.default({"turn": 3, "step": "combat_damage", "active": "A", "seat": "A"})
        self.assertEqual(reply["state"]["pending"], {"seat": "A", "kind": "assign_combat_damage", "about": {"creature": "ball"}})
        self.assertEqual(reply["asked"], "assign_combat_damage")
        reply = self.act(3, "A", {"kind": "assign_combat_damage", "assignments": [["b1", 2], ["B", 4]]})
        self.assertEqual(reply["state"]["pending"], {"seat": "A", "kind": "assign_combat_damage", "about": {"creature": "giant"}})
        reply = self.act(4, "A", {"kind": "assign_combat_damage", "assignments": [["b2", 2], ["b3", 1]]})
        # 404.3 and 101.4: owners choose each incoming graveyard block in
        # APNAP order before the simultaneous deaths commit.
        self.assertEqual(reply["state"]["pending"]["kind"], "order_cards")
        reply = self.act(5, "A", {"kind": "order_cards", "order": ["ball", "giant"]})
        self.assertEqual(reply["state"]["zones"]["A.graveyard"], [])
        reply = self.act(6, "B", {"kind": "order_cards", "order": ["b1", "b2"]})
        state = reply["state"]
        self.assertEqual(state["players"]["B"]["life"], 16)
        # Full MGN graveyard listings are oldest first (spec 3.3); authored
        # order_cards answers are top first. Both lethal groups die (704.5g).
        self.assertEqual(state["zones"]["A.graveyard"], ["giant", "ball"])
        self.assertEqual(state["zones"]["B.graveyard"], ["b2", "b1"])
        self.assertEqual(state["objects"]["b3"]["damage"], 1)
        self.assertEqual(state["pending"], PRIORITY_A)  # 510.3: the active player receives priority

    def test_a_cleanup_discard_is_answered_and_the_next_turn_begins(self):
        # 514.1: discard down to the maximum hand size; 701.9a
        cards = {f"f{i}": ("Forest", "A") for i in range(8)}
        cards.update({"lions": ("Savannah Lions", "A"), "lib": ("Island", "A"), "libB": ("Island", "B")})
        self.start(cards, {"A.hand": [f"f{i}" for i in range(8)] + ["lions"], "A.library": ["lib"], "B.library": ["libB"]})
        self.act(1, "A", {"kind": "play_land", "card": "f0"})
        reply = self.default({"turn": 4, "step": "upkeep", "active": "B", "seat": "B"})
        self.assertEqual(reply["state"]["pending"], {"seat": "A", "kind": "discard", "about": {"count": 1}})
        reply = self.act(2, "A", {"kind": "discard", "cards": ["lions"]})
        self.assertEqual(reply["state"]["zones"]["A.graveyard"], ["lions"])
        # nothing else happens in the cleanup step (514.2, 514.3a): turn 4 begins and B has priority in the upkeep
        # (502.4, 503.1), before the draw step (504.1)
        self.assertEqual(reply["state"]["turn"], {"number": 4, "step": "upkeep", "active": "B", "occurrence": 1})
        self.assertEqual(reply["state"]["pending"], {"seat": "B", "kind": "priority"})
        self.assertEqual(reply["state"]["zones"]["B.library"], ["libB"])

    def test_kinds_the_engine_never_asks_in_a_two_seat_game_still_translate(self):
        # "choose a player" and "choose a number" (spec 5.4): the engine admits no card asking them, so the
        # answers are translated and checked here without a game
        adapter = KarnAdapter(ENGINE, ORACLE)
        adapter.seats = ["A", "B"]
        adapter.state = {"asked": {"kind": "player", "among": [0, 1]}}
        self.assertEqual(adapter._move({"kind": "choose_player", "player": "B"}), {"kind": "choose", "answer": {"player": 1}})
        adapter.state = {"asked": {"kind": "number", "min": 0, "max": 10}}
        self.assertEqual(adapter._move({"kind": "choose_number", "number": 3}), {"kind": "choose", "answer": {"value": {"number": 3}}})
        self.assertEqual(adapter._move({"kind": "concede"}), {"kind": "concede"})


@unittest.skipUnless(ENGINE.is_file(), "build target/fast/game-serve to exercise the Karn adapter")
class LaterStepStartTests(unittest.TestCase):
    """A position later in the turn than the precombat main phase is constructed by playing the
    turn forward from that main phase with null answers and the declarations the position shows
    (spec 2.4: nothing happened earlier in the turn beyond what the assumptions state)."""

    def setUp(self):
        self.adapter = KarnAdapter(ENGINE, ORACLE)

    def tearDown(self):
        self.adapter.end()

    def test_a_block_declaration_boundary_is_constructed_with_its_attackers_declared(self):
        cards = {"giant": ("Hill Giant", "A"), "b1": ("Grizzly Bears", "B"), "b2": ("Grizzly Bears", "B"), "la": ("Forest", "A"), "lb": ("Island", "B")}
        record = position_record(cards, {"battlefield": ["giant", "b1", "b2"], "A.library": ["la"], "B.library": ["lb"]},
                                 {"number": 3, "step": "declare_blockers", "active": "A"}, {"seat": "B", "kind": "declare_blockers"},
                                 objects={"giant": {"attacking": "B", "tapped": True}, "b1": {"controller": "B"}, "b2": {"controller": "B"}})  # 508.1f
        reply = self.adapter.setup(setup_of(record))
        # 508.1: the Giant was declared attacking B on the way; 509.1a: B, controlling creatures, declares blockers
        self.assertEqual(reply["state"], State.from_record(record, []).explicit(ORACLE))
        reply = self.adapter.act(1, "B", {"kind": "declare_blockers", "blocks": [["b1", "giant"], ["b2", "giant"]]}, no_random)
        self.assertEqual(reply["state"]["objects"]["giant"]["blocked"], True)

    def test_a_cleanup_discard_boundary_is_constructed(self):
        record = loads((ROOT / "docs/mgn/examples/cleanup-discard.json").read_text(encoding="utf-8"))
        reply = self.adapter.setup(setup_of(record))
        self.assertEqual(reply["state"]["turn"], {"number": 3, "step": "cleanup", "active": "A", "occurrence": 1})
        self.assertEqual(reply["state"]["pending"], {"seat": "A", "kind": "discard", "about": {"count": 2}})  # 514.1

    def test_a_position_after_blocks_is_refused(self):
        cards = {"giant": ("Hill Giant", "A"), "b1": ("Grizzly Bears", "B"), "la": ("Forest", "A"), "lb": ("Island", "B")}
        record = position_record(cards, {"battlefield": ["giant", "b1"], "A.library": ["la"], "B.library": ["lb"]},
                                 {"number": 3, "step": "end_combat", "active": "A"}, PRIORITY_A,
                                 objects={"giant": {"attacking": "B", "blocked": True}, "b1": {"controller": "B", "blocking": ["giant"]}})
        with self.assertRaises(Exception) as refused:
            self.adapter.setup(setup_of(record))
        self.assertEqual(getattr(refused.exception, "code", None), "unsupported_start")


@unittest.skipUnless(ENGINE.is_file(), "build target/fast/game-serve to exercise the Karn adapter")
class PregameAndEndingTests(unittest.TestCase):
    """The decks start (spec 2.4, 5.1) and the questions the engine asks with the step-4 changes: the
    pregame from the recorded shuffle orders, the concession (104.3a), the forced defender choice
    (508.4), the pile separation (700.3), competing skips (616.1) and spell destinations."""

    def setUp(self):
        self.adapter = KarnAdapter(ENGINE, ORACLE)

    def tearDown(self):
        self.adapter.end()

    def _defaults(self, until: dict) -> dict:
        """Null answers until the target as the runner gives them (verification.md, L2): a stretch
        stops at a combat declaration or at a question without a null answer, and the next call
        goes on from there."""
        for _ in range(16):
            reply = self.adapter.default(until, no_random)
            turn, pending = reply["state"]["turn"], reply["state"]["pending"]
            if "asked" in reply or pending is None:
                return reply
            if (turn["number"], turn["step"], turn["active"], pending["seat"]) == (until["turn"], until["step"], until["active"], until["seat"]):
                return reply
        raise AssertionError(f"{until} was not reached by null answers")

    def test_a_decks_start_plays_the_pregame_from_the_recorded_orders(self):
        record = loads((ROOT / "docs/mgn/examples/pregame.json").read_text(encoding="utf-8"))
        reply = self.adapter.setup(setup_of(record))
        expected = State.from_record(record, [])
        self.assertEqual(reply["state"], expected.explicit(ORACLE))  # turn 0, pregame, choose_first pending (spec 2.4)
        outcomes = list(record["transitions"][0]["random"])

        def ask(key):
            for i, outcome in enumerate(outcomes):
                if outcome["kind"] == key["kind"] and outcome.get("seat") == key.get("seat"):
                    return outcomes.pop(i)
            return None
        reply = self.adapter.act(1, "A", {"kind": "choose_first", "first": "A"}, ask)
        self.assertEqual(outcomes, [])  # both start shuffles consumed (103.3)
        state = reply["state"]
        self.assertEqual(state["turn"], {"number": 0, "step": "pregame", "active": None, "occurrence": 1})
        self.assertEqual(state["pending"], {"seat": "A", "kind": "mulligan"})  # the starting player declares first (103.5)
        self.assertEqual(state["zones"]["A.hand"], sorted(record["transitions"][0]["random"][0]["order"][:7]))
        self.assertEqual(state["zones"]["A.library"], record["transitions"][0]["random"][0]["order"][7:])
        reply = self.adapter.default({"turn": 0, "step": "pregame", "active": None, "seat": "B"}, no_random)  # A keeps (implied)
        self.assertEqual(reply["state"]["pending"], {"seat": "B", "kind": "mulligan"})
        outcomes = list(record["transitions"][1]["random"])
        reply = self.adapter.act(2, "B", {"kind": "mulligan", "keep": False}, ask)
        self.assertEqual(reply["state"]["pending"], {"seat": "B", "kind": "bottom", "about": {"count": 1}})  # one card per mulligan taken (103.5)
        self.assertEqual(len(reply["state"]["zones"]["B.hand"]), 7)
        reply = self.adapter.act(3, "B", {"kind": "bottom", "cards": ["elvesB2"]}, no_random)
        self.assertEqual(reply["state"]["zones"]["B.library"][-1], "elvesB2")
        # B keeps (implied); the first turn begins: no draw for the starting player (103.8a), priority in the upkeep (503.1)
        reply = self.adapter.default({"turn": 1, "step": "upkeep", "active": "A", "seat": "A"}, no_random)
        self.assertEqual(reply["state"]["turn"], {"number": 1, "step": "upkeep", "active": "A", "occurrence": 1})
        self.assertEqual(reply["state"]["pending"], PRIORITY_A)
        self.assertEqual(len(reply["state"]["zones"]["A.hand"]), 7)

    def test_a_concession_ends_a_two_player_game_from_any_seat(self):
        cards = {"bears": ("Grizzly Bears", "A"), "f1": ("Forest", "A"), "f2": ("Forest", "A"), "la": ("Forest", "A"), "lb": ("Island", "B")}
        record = position_record(cards, {"A.hand": ["bears"], "battlefield": ["f1", "f2"], "A.library": ["la"], "B.library": ["lb"]}, MAIN_A, PRIORITY_A)
        self.adapter.setup(setup_of(record))
        self.adapter.act(1, "A", {"kind": "cast", "card": "bears", "pay": _mana("f1", "f2")}, no_random)
        reply = self.adapter.act(2, "B", {"kind": "concede"}, no_random)  # B concedes while A is to answer (104.3a)
        state = reply["state"]
        self.assertEqual(state["result"], {"kind": "win", "winner": "A", "cause": "concession"})
        self.assertIsNone(state["pending"])
        self.assertEqual(state["players"]["B"]["left"], True)
        self.assertEqual(state["zones"]["stack"], ["bears"])  # the game ends with everything where it is (104.2a)

    def test_a_creature_entering_attacking_asks_its_defender_even_when_forced(self):
        cards = {"brig": ("Nightblade Brigade", "A"), "la": ("Forest", "A"), "lb": ("Island", "B")}
        record = position_record(cards, {"battlefield": ["brig"], "A.library": ["la"], "B.library": ["lb"]},
                                 {"number": 3, "step": "declare_attackers", "active": "A"}, {"seat": "A", "kind": "declare_attackers"})
        self.adapter.setup(setup_of(record))
        reply = self.adapter.act(1, "A", {"kind": "declare_attackers", "attacks": [["brig", "B"]]}, no_random)
        reply = self.adapter.default({"turn": 3, "step": "declare_attackers", "active": "A", "seat": "A"}, no_random)
        # mobilize's trigger resolves: the token enters attacking and A chooses what it attacks (508.4), B the only choice
        self.assertEqual(reply["state"]["pending"], {"seat": "A", "kind": "choose_defender", "about": {"object": "S1"}})
        self.assertEqual(reply["asked"], "choose_defender")  # no null answer (spec 5.6)
        self.assertNotIn("T1", reply["state"]["objects"])  # the token does not exist yet (spec 4.2)
        reply = self.adapter.act(2, "A", {"kind": "choose_defender", "defender": "B"}, no_random)
        token = reply["state"]["objects"]["T1"]
        self.assertEqual((token["attacking"], token["tapped"], token["controller"]), ("B", True, "A"))
        self.assertEqual(token["token"]["name"], "Warrior Token")  # 111.4

    def test_a_pile_separation_is_asked_of_the_opponent_and_the_pile_of_the_caster(self):
        cards = {"fof": ("Fact or Fiction", "A"), "c1": ("Forest", "A"), "c2": ("Grizzly Bears", "A"), "c3": ("Forest", "A"), "c4": ("Lightning Bolt", "A"), "c5": ("Forest", "A"), "c6": ("Forest", "A"), "lb": ("Island", "B")}
        cards.update({f"i{n}": ("Island", "A") for n in range(4)})
        record = position_record(cards, {"A.hand": ["fof"], "battlefield": [f"i{n}" for n in range(4)], "A.library": ["c1", "c2", "c3", "c4", "c5", "c6"], "B.library": ["lb"]}, MAIN_A, PRIORITY_A)
        self.adapter.setup(setup_of(record))
        self.adapter.act(1, "A", {"kind": "cast", "card": "fof", "pay": _mana(*[f"i{n}" for n in range(4)], mana="U")}, no_random)
        reply = self.adapter.default(MAIN_A_UNTIL, no_random)
        self.assertEqual(reply["state"]["pending"], {"seat": "B", "kind": "separate", "about": {"object": "fof", "cards": ["c1", "c2", "c3", "c4", "c5"]}})
        self.assertEqual(reply["asked"], "separate")  # no null answer (700.3)
        reply = self.adapter.act(2, "B", {"kind": "separate", "piles": [["c1", "c2"], ["c3", "c4", "c5"]]}, no_random)
        self.assertEqual(reply["state"]["pending"], {"seat": "A", "kind": "pile", "about": {"object": "fof"}})
        reply = self.adapter.act(3, "A", {"kind": "pile", "pile": 2}, no_random)
        # 608.2f: both piles move simultaneously, after the owner chooses
        # the graveyard order (404.3); choosing a pile alone moves neither.
        self.assertEqual(reply["state"]["pending"]["kind"], "order_cards")
        self.assertEqual(reply["state"]["zones"]["A.hand"], [])
        self.assertEqual(reply["state"]["zones"]["A.library"], ["c1", "c2", "c3", "c4", "c5", "c6"])
        reply = self.adapter.act(4, "A", {"kind": "order_cards", "order": ["c2", "c1"]}, no_random)
        self.assertEqual(sorted(reply["state"]["zones"]["A.hand"]), ["c3", "c4", "c5"])
        # Oldest first: c2 is above c1, and the spell finishes last (608.2n).
        self.assertEqual(reply["state"]["zones"]["A.graveyard"], ["c1", "c2", "fof"])

    def _two_skips(self) -> dict:
        """Stonehorn Dignitary's effect (a permanent's ability) and Moment of Silence (a resolved
        spell) would both skip B's next combat phase: the reply at B's choice between them."""
        cards = {"stone": ("Stonehorn Dignitary", "A"), "moment": ("Moment of Silence", "A"), "la": ("Forest", "A"), "lb": ("Island", "B"), "lb2": ("Island", "B"), "bb": ("Grizzly Bears", "B")}
        cards.update({f"p{n}": ("Plains", "A") for n in range(5)})
        record = position_record(cards, {"A.hand": ["stone", "moment"], "battlefield": [f"p{n}" for n in range(5)] + ["bb"], "A.library": ["la"], "B.library": ["lb", "lb2"]},
                                 MAIN_A, PRIORITY_A, objects={"bb": {"controller": "B"}})
        self.adapter.setup(setup_of(record))
        self.adapter.act(1, "A", {"kind": "cast", "card": "stone", "pay": _mana(*[f"p{n}" for n in range(4)], mana="W")}, no_random)
        self.adapter.default(MAIN_A_UNTIL, no_random)
        self.adapter.act(2, "A", {"kind": "choose_targets", "targets": [["B"]]}, no_random)
        self.adapter.default({"turn": 4, "step": "precombat_main", "active": "B", "seat": "A"}, no_random)
        self.adapter.act(3, "A", {"kind": "cast", "card": "moment", "targets": [["B"]], "pay": _mana("p4", mana="W")}, no_random)
        reply = self.adapter.default({"turn": 4, "step": "begin_combat", "active": "B", "seat": "B"}, no_random)
        # two skip effects would replace B's combat phase: B chooses which (616.1), before it begins (614.10)
        self.assertEqual(reply["state"]["pending"], {"seat": "B", "kind": "skip"})
        self.assertEqual(reply["state"]["turn"]["step"], "precombat_main")
        return reply

    def test_competing_skip_effects_are_named_by_their_source(self):
        self._two_skips()
        reply = self.adapter.act(4, "B", {"kind": "skip", "effect": {"source": "stone", "ability": {"face": 0, "paragraph": 1}}}, no_random)
        self.assertEqual(reply["state"]["turn"], {"number": 4, "step": "postcombat_main", "active": "B", "occurrence": 1})

    def test_a_skip_effect_of_a_resolved_spell_is_found_by_its_card(self):
        reply = self._two_skips()
        self.assertEqual(reply["state"]["zones"]["A.graveyard"], ["moment"])  # the spell resolved (608.2n): its effect lives on
        reply = self.adapter.act(4, "B", {"kind": "skip", "effect": {"source": "moment", "ability": {"face": 0, "paragraph": 1}}}, no_random)
        # the Moment's effect is used up and the Dignitary's waits for a later combat phase (614.10a)
        self.assertEqual(reply["state"]["turn"], {"number": 4, "step": "postcombat_main", "active": "B", "occurrence": 1})

    def _omen_destination(self) -> dict:
        """Skimming Strike, the Omen of Dirgur Island Dragon, resolves under B's Leyline of the Void:
        the reply at A's choice between the Omen's shuffle (720.3d) and the Leyline's exile."""
        cards = {"dragon": ("Dirgur Island Dragon // Skimming Strike", "A"), "i1": ("Island", "A"), "i2": ("Island", "A"), "ley": ("Leyline of the Void", "B"),
                 "la": ("Forest", "A"), "la2": ("Forest", "A"), "lb": ("Island", "B"), "bb": ("Grizzly Bears", "B")}
        cards.update({f"s{n}": ("Swamp", "B") for n in range(4)})
        record = position_record(cards, {"A.hand": ["dragon"], "B.hand": ["ley"], "battlefield": ["i1", "i2", "bb"] + [f"s{n}" for n in range(4)], "A.library": ["la", "la2"], "B.library": ["lb"]},
                                 {"number": 4, "step": "precombat_main", "active": "B"}, {"seat": "B", "kind": "priority"},
                                 objects={**{f"s{n}": {"controller": "B"} for n in range(4)}, "bb": {"controller": "B"}})
        self.adapter.setup(setup_of(record))
        self.adapter.act(1, "B", {"kind": "cast", "card": "ley", "pay": _mana(*[f"s{n}" for n in range(4)], mana="B")}, no_random)
        self._defaults({"turn": 5, "step": "precombat_main", "active": "A", "seat": "A"})
        reply = self.adapter.act(2, "A", {"kind": "cast", "card": "dragon", "face": 1, "targets": [["bb"]], "pay": _mana("i1", "i2", mana="U")}, no_random)
        self.assertEqual(reply["state"]["objects"]["dragon"]["face"], 1)  # cast as an Omen (720.3)
        reply = self.adapter.default({"turn": 5, "step": "precombat_main", "active": "A", "seat": "A"}, no_random)
        # the Omen's shuffle (720.3d) and the Leyline's exile would both replace the move to the graveyard (616.1)
        self.assertEqual(reply["state"]["pending"], {"seat": "A", "kind": "spell_destination", "about": {"object": "dragon"}})
        return reply

    def test_a_spell_destination_is_named_by_its_source(self):
        self._omen_destination()
        reply = self.adapter.act(3, "A", {"kind": "spell_destination", "effect": {"source": "ley", "ability": {"face": 0, "paragraph": 2}}}, no_random)
        self.assertEqual(reply["state"]["zones"]["exile"], ["dragon"])

    def test_a_spell_destination_named_by_rule_shuffles_the_omen_into_its_library(self):
        self._omen_destination()

        # the Omen's "draw a card" took A's last card, so the library holds the Omen alone after its shuffle
        self.assertEqual(self.adapter.project(1)["state"]["zones"]["A.library"], [])

        outcomes = [{"kind": "shuffle", "seat": "A", "order": ["dragon"]}]

        def shuffled(key):
            return outcomes.pop(0) if outcomes and key == {"kind": "shuffle", "seat": "A"} else None

        reply = self.adapter.act(3, "A", {"kind": "spell_destination", "effect": {"rule": "720.3d"}}, shuffled)
        # 720.3d: the card is shuffled into its owner's library instead; the recorded order is the library's
        self.assertEqual(reply["state"]["zones"]["A.library"], ["dragon"])
        self.assertEqual(reply["state"]["zones"]["exile"], [])
        self.assertEqual(reply["state"]["zones"]["A.graveyard"], [])
