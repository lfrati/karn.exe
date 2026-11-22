"""The MGN recorder (scripts/mgn/record.py). Without an engine: the exact executable and inputs
of a recording are captured and any drift is refused; a renewed card in the zone it held is a
re-entry (400.7), in a library a reorder. On Karn: a decks start whose one pregame entry reaches
the opening-hand actions (103.6) replays through them into the first turn, and a position the
pregame cannot reach diverges; a drafted scenario is recorded with the deltas the rules give, the
tier-2 facts a tier-1 draft never stated supplied by the engine; a draft the engine disagrees
with on a tier-1 fact and a directory that already holds a record are refused."""
from copy import deepcopy
import json
import hashlib
from unittest.mock import patch
import os
from pathlib import Path
import tempfile
import unittest

from mgn.canonical import loads
from mgn.delta import diff_states
from mgn.oracle import Oracle
from mgn.record import Recorder, RecordingError, _finish, _provenance, _unchanged, from_driver
from mgn.runner import Run, karn_adapter_command
from mgn.state import State
from mgn.validate import validate

ROOT = Path(__file__).resolve().parents[2]
ENGINE = Path(os.environ.get("KARN_CONFORMANCE_ENGINE", ROOT / "target/fast/game-serve"))
FIXTURES = ROOT / "tests/fixtures/mgn"
VIVI = FIXTURES / "vivi-decks/record.json"
LINKED = FIXTURES / "scenario-linked-first-v2/draft.json"
PREVENTION = FIXTURES / "scenario-prevention-resources-v2/draft.json"
ORACLE = Oracle()


def record(path: Path) -> dict:
    return loads(path.read_text(encoding="utf-8"))


def write_draft(directory: Path, draft: dict) -> Path:
    path = directory / "draft.json"
    path.write_text(json.dumps(draft), encoding="utf-8")
    return path


def opening_position(owner="B", copies=2):
    """Rules/card-derived position: both keep seven; Leylines await CR 103.6.

    Leyline of Sanctity's pinned full Oracle text permits starting it on the
    battlefield; its other clause grants its controller hexproof. No triggered
    ability or intervening choice changes the modeled hand/library boundary.
    Pinned ruling 95db2230a58af131 (2021-03-19) confirms post-mulligan hand
    identity, starting-player-first ordering and the turn beginning afterward.
    Returns the card table and the position.
    """
    cards = {}
    zones = {zone: [] for zone in ("battlefield", "stack", "exile", "command")}
    for seat in "AB":
        names = (["Leyline of Sanctity"] * copies + ["Plains"] * (8 - copies)
                 if seat == owner else ["Forest"] * 8)
        for i, name in enumerate(names):
            cards[f"{seat}{i}"] = {"owner": seat, "oracle": ORACLE.by_name(name), "name": name}
        zones[f"{seat}.hand"] = [f"{seat}{i}" for i in range(7)]
        zones[f"{seat}.library"] = [f"{seat}7"]
        zones[f"{seat}.graveyard"] = []
    return cards, {"turn": {"number": 0, "step": "pregame", "active": None},
                   "pending": {"seat": owner, "kind": "opening_hand_action"},
                   "result": {"kind": "unfinished"}, "players": {s: {"life": 20} for s in "AB"},
                   "zones": zones, "objects": {}}


def at(rec: dict, position: dict) -> State:
    """The state of a stated position of the record's game."""
    return State.from_record({**rec, "start": {"kind": "position", "state": position}}, [])


def from_the_decks(cards: dict, opening: dict, first: str) -> dict:
    """A decks start whose one pregame entry reaches `opening`: `first` is fixed as the starting
    player and chooses to play first (103.1); each shuffle (103.3) lists the seven cards drawn as
    the hand first (103.5; a hand is unordered, 402.3), then the library as the position lists it;
    every seat keeps, the null answer the entry's stretch implies (spec 5.6)."""
    example = record(ROOT / "docs/mgn/examples/pregame.json")
    rec = {**{key: example[key] for key in ("mgn", "tier", "pins", "game")},
           "author": {"kind": "tool", "name": "rules-test", "version": "1"}, "cards": cards,
           "start": {"kind": "decks", "random": [{"kind": "starting_player", "method": "other", "result": first}]},
           "transitions": []}
    shuffles = [{"kind": "shuffle", "seat": seat,
                 "order": opening["zones"][f"{seat}.hand"] + opening["zones"][f"{seat}.library"]}
                for seat in rec["game"]["seats"]]
    delta = diff_states(State.from_record(rec, []), at(rec, opening), ORACLE, random_outcomes=shuffles)
    rec["transitions"].append({"seat": first, "do": {"kind": "choose_first", "first": first},
                               "random": shuffles, "delta": delta})
    return rec


class ProvenanceTests(unittest.TestCase):
    def test_exact_executable_and_inputs_are_captured_and_drift_refused(self):
        with tempfile.TemporaryDirectory() as tmp:
            engine = Path(tmp) / "engine"
            driver = Path(tmp) / "driver.json"
            engine.write_bytes(b"preserved engine")
            driver.write_bytes(b'{"tier": 1}\n')
            provenance = _provenance(engine, ORACLE, [driver])
            self.assertEqual(provenance["engine_executable"],
                             {"path": str(engine.resolve()), "sha256": hashlib.sha256(engine.read_bytes()).hexdigest()})
            self.assertEqual(provenance["files"][str(driver)], hashlib.sha256(driver.read_bytes()).hexdigest())
            self.assertIn(str(ROOT / "scripts/mgn/runner.py"), provenance["files"])
            self.assertIn(str(ROOT / "docs/mgn/schema/mgn-1.schema.json"), provenance["files"])
            self.assertIn("version", provenance["python"])
            _unchanged(provenance)
            for path in (engine, driver):
                original = path.read_bytes()
                path.write_bytes(original + b"changed")
                with self.assertRaisesRegex(RecordingError, "provenance drift"):
                    _unchanged(provenance)
                path.write_bytes(original)

    def test_drift_during_replay_prevents_saving_the_record(self):
        with tempfile.TemporaryDirectory() as tmp:
            engine = Path(tmp) / "engine"
            engine.write_bytes(b"before")
            provenance = _provenance(engine, ORACLE, [VIVI])
            report = {"engine": {"name": "test", "version": "same-version"}, "adapter": {}, "replayed": 0}
            replay = {"stopped_at": None, "inconclusive": [], "divergences": [], "replayed": 0,
                      "levels": {level: "pass" for level in ("L0", "L1", "L2", "L3", "L4", "L5")},
                      "engine": report["engine"], "usage": {"seconds": 0}}
            def replay_with_changed_binary():
                engine.write_bytes(b"after")  # version strings alone cannot detect this
                return replay
            out = Path(tmp) / "recording"
            with patch("mgn.record.Run") as runner:
                runner.return_value.run.side_effect = replay_with_changed_binary
                with self.assertRaisesRegex(RecordingError, "provenance drift"):
                    _finish(record(VIVI), report, {"not_projected": [], "supplied": [], "accepted": [], "renewed": {}},
                            ORACLE, engine, 120, out, {}, "test", provenance)
            self.assertFalse(out.exists())

    def test_source_changed_since_import_is_refused_before_driving(self):
        from mgn import record as recorder_module
        changed = dict(recorder_module._IMPORT_FILES)
        changed[str(ROOT / "scripts/mgn/runner.py")] = "wrong hash"
        with patch.object(recorder_module, "_IMPORT_FILES", changed):
            with self.assertRaisesRegex(RecordingError, "provenance drift"):
                _provenance(Path(__file__), ORACLE, [])


class BoundaryTests(unittest.TestCase):
    def test_a_renewed_card_is_a_reentry_in_its_zone_but_a_reorder_in_a_library(self):
        # 400.7: an object that left a zone and came back is new; within a library the engine renews a
        # card it moved (to the bottom), which 400.7 does not cover, so the delta reorders instead
        recorder = Recorder({"cards": {"a": {}, "b": {}, "c": {}}, "tier": 2}, ORACLE, ["unused"])
        recorder._boundary(0, {"zones": {"battlefield": ["a"], "A.library": ["b", "c"]}}, [])
        recorder._renewed_since.update(["a", "b", "c"])
        recorder._boundary(1, {"zones": {"battlefield": ["a", "c"], "A.library": ["b"]}}, [])
        self.assertEqual(recorder.reentered[1], ["a"])  # a: left and came back; c: moved zones; b: renewed within its library
        self.assertEqual(recorder.renewed_at[1], ["a", "b", "c"])



@unittest.skipUnless(ENGINE.is_file(), "build target/fast/game-serve to replay MGN games on Karn")
class OpeningHandReplayTests(unittest.TestCase):
    def test_r103_6_opening_actions_replay_from_the_decks_and_continue_into_turn_one(self):
        # CR 103.5, 103.6/103.6a: seven cards each after keeps; eligible
        # player may take multiple actions in any order. CR 103.8a: no draw
        # in the starting player's first turn; we join its first main phase.
        for first, owner in (("A", "B"), ("B", "B"), ("B", "A")):
            with self.subTest(first=first, owner=owner):
                cards, opening = opening_position(owner)
                rec = from_the_decks(cards, opening, first)
                current = at(rec, opening)
                for i in (1, 0):  # choose the second eligible Leyline first
                    position = deepcopy(opening)
                    chosen = [f"{owner}1"] + ([f"{owner}0"] if i == 0 else [])
                    position["zones"][f"{owner}.hand"] = [c for c in position["zones"][f"{owner}.hand"] if c not in chosen]
                    position["zones"]["battlefield"] = chosen
                    if i == 0:
                        position["turn"] = {"number": 1, "step": "precombat_main", "active": first}
                        position["pending"] = {"seat": first, "kind": "priority"}
                    target = at(rec, position)
                    rec["transitions"].append({"seat": owner, "do": {"kind": "opening_hand_action", "card": f"{owner}{i}"},
                                               "delta": diff_states(current, target, ORACLE)})
                    current = target
                self.assertEqual([str(p) for p in validate(rec, ORACLE) if p.severity == "error"], [])
                report = Run(rec, ORACLE, karn_adapter_command(ENGINE)).run()
                self.assertEqual(report["divergences"], [])
                self.assertEqual(report["refusals"], [])
                self.assertEqual(report["inconclusive"], [])
                self.assertEqual(report["replayed"], 3)
                self.assertIsNone(report["stopped_at"])

    def test_r103_6_declining_remaining_opening_action_keeps_that_card_and_begins_turn_one(self):
        cards, opening = opening_position()
        rec = from_the_decks(cards, opening, "A")
        position = deepcopy(opening)
        position["turn"] = {"number": 1, "step": "precombat_main", "active": "A"}
        position["pending"] = {"seat": "A", "kind": "priority"}
        # The null decline of B0 is folded into the boundary after taking B1.
        position["zones"]["B.hand"].remove("B1")
        position["zones"]["battlefield"] = ["B1"]
        rec["transitions"].append({"seat": "B", "do": {"kind": "opening_hand_action", "card": "B1"},
                                   "delta": diff_states(at(rec, opening), at(rec, position), ORACLE)})
        self.assertEqual([str(p) for p in validate(rec, ORACLE) if p.severity == "error"], [])
        report = Run(rec, ORACLE, karn_adapter_command(ENGINE)).run()
        self.assertEqual(report["divergences"], [])
        self.assertEqual(report["refusals"], [])
        self.assertEqual(report["inconclusive"], [])
        self.assertEqual(report["replayed"], 2)

    def test_r103_6_an_opening_position_the_pregame_cannot_reach_does_not_replay(self):
        # 103.6: only a player whose opening hand holds a card with such an action is asked to
        # take one, so the seat without a Leyline, and either seat when no hand holds one, is
        # never asked; 103.4: each player begins at the starting life total, which no keep changes.
        for mutation in ("wrong_seat", "no_eligible_card", "life"):
            with self.subTest(mutation=mutation):
                cards, opening = opening_position(copies=0 if mutation == "no_eligible_card" else 2)
                if mutation == "wrong_seat":
                    opening["pending"]["seat"] = "A"
                elif mutation == "life":
                    opening["players"]["A"]["life"] = 19
                rec = from_the_decks(cards, opening, "A")
                self.assertEqual([str(p) for p in validate(rec, ORACLE) if p.severity == "error"], [])
                report = Run(rec, ORACLE, karn_adapter_command(ENGINE)).run()
                if mutation == "life":
                    # the question is reached, so the difference is reported there (verification.md, L3)
                    self.assertEqual(report["divergences"],
                                     [{"after": 1, "level": "L3", "path": "players.A.life", "expected": 19, "actual": 20, "first": True}])
                else:
                    # no state asks that seat for an opening-hand action: the boundary is never reached (L2)
                    self.assertEqual(report["levels"]["L2"], "fail")
                    self.assertIn("pending", {d["path"] for d in report["divergences"] if d["level"] == "L2"})
                    self.assertEqual({d["after"] for d in report["divergences"]}, {1})


@unittest.skipUnless(ENGINE.is_file(), "build target/fast/game-serve to record MGN games on Karn")
class RecorderTests(unittest.TestCase):
    def test_echo_history_is_established_by_the_recorded_cast_before_paying_its_cost(self):
        # 702.30a/603.4/118.12: the start asserts no unknown battlefield Echo history.
        driver = ROOT / "tests/fixtures/mgn-drafts/echo.json"
        with tempfile.TemporaryDirectory() as tmp:
            out = Path(tmp) / "echo"
            from_driver(driver, out, ENGINE, ORACLE, tier=2)
            self.assertEqual((out / "draft.json").read_bytes(), driver.read_bytes())
            saved = record(out / "record.json")
            problems = []
            state = State.from_record(saved, problems)
            for index, transition in enumerate(saved["transitions"]):
                state.apply(transition["delta"], transition.get("random", []), f"transitions.{index}.delta", problems)
            self.assertEqual(problems, [])
            self.assertEqual(state.turn, {"number": 3, "step": "upkeep", "active": "A"})
            self.assertEqual(state.zone_of("jaguar"), "battlefield")
            self.assertEqual(state.players["A"].get("mana", {}), {})
            self.assertEqual(state.zones["B.hand"], ["b1"])

    def test_cascade_decline_records_the_supplied_random_return_order(self):
        # 702.85a: the authored driver supplies the order, rather than an engine seed.
        driver = ROOT / "tests/fixtures/mgn-drafts/cascade.json"
        with tempfile.TemporaryDirectory() as tmp:
            out = Path(tmp) / "cascade"
            from_driver(driver, out, ENGINE, ORACLE, tier=2)
            self.assertEqual((out / "draft.json").read_bytes(), driver.read_bytes())
            transition = record(out / "record.json")["transitions"][0]
            self.assertEqual(transition["random"], record(driver)["transitions"][0]["random"])
            self.assertEqual(transition["delta"]["objects"]["agent"]["zone"], "battlefield")
            saved = record(out / "record.json")
            problems = []
            state = State.from_record(saved, problems)
            state.apply(transition["delta"], transition["random"], "transitions.0.delta", problems)
            self.assertEqual(problems, [])
            self.assertEqual(state.zones["A.library"], ["untouched", "bear", "land", "giant"])

    def test_suspend_special_action_records_its_public_time_counter(self):
        # 116.2f/702.62a/b: this authored driver states the complete special action's effects.
        driver = ROOT / "tests/fixtures/mgn-drafts/suspend.json"
        with tempfile.TemporaryDirectory() as tmp:
            out = Path(tmp) / "suspend"
            summary = from_driver(driver, out, ENGINE, ORACLE, tier=2)
            self.assertEqual(summary["supplied"], [])
            self.assertEqual((out / "draft.json").read_bytes(), driver.read_bytes())
            transition = record(out / "record.json")["transitions"][0]
            self.assertEqual(transition["do"]["action"], "suspend")
            self.assertEqual(transition["delta"]["objects"]["bolt"],
                             {"zone": "exile", "counters": {"time": 1}})

    def test_foretell_tier_one_draft_records_new_sparse_tier_two_facts(self):
        # 702.143a: the special action exiles the card face down, foretold;
        # 305.6: both Forest mana abilities tap their sources to pay {2}.
        driver = ROOT / "tests/fixtures/mgn-drafts/special-action.json"
        with tempfile.TemporaryDirectory() as tmp:
            out = Path(tmp) / "foretell"
            summary = from_driver(driver, out, ENGINE, ORACLE, tier=2)
            self.assertEqual({x["path"]: x["engine"] for x in summary["supplied"]},
                             {"objects.doom.designations": ["foretold"], "objects.doom.face_down": True,
                              "objects.f0.tapped": True, "objects.f1.tapped": True})
            self.assertEqual((out / "draft.json").read_bytes(), driver.read_bytes())
            self.assertEqual(record(out / "draft.json")["tier"], 1)
            self.assertEqual(summary["provenance"]["driver_input"]["sha256"], hashlib.sha256((out / "draft.json").read_bytes()).hexdigest())
            self.assertEqual(summary["provenance"]["engine_executable"]["sha256"], hashlib.sha256(ENGINE.read_bytes()).hexdigest())
            self.assertEqual(record(out / "record.json")["transitions"][0]["delta"]["objects"]["doom"],
                             {"zone": "exile", "face_down": True, "designations": ["foretold"]})

    def test_a_whole_game_is_recorded_with_a_card_moved_within_its_library_as_a_reorder(self):
        old = record(VIVI)
        with tempfile.TemporaryDirectory() as out:
            summary = from_driver(VIVI, Path(out) / "rec", ENGINE, ORACLE)
            new = record(Path(out) / "rec" / "record.json")
            review = [json.loads(line) for line in (Path(out) / "rec" / "review.jsonl").read_text(encoding="utf-8").splitlines()]
        self.assertEqual(summary["replay"]["levels"], {"L0": "pass", "L1": "pass", "L2": "pass", "L3": "pass", "L4": "pass", "L5": "not_applicable"})
        self.assertEqual([str(p) for p in validate(new, ORACLE) if p.severity == "error"], [])
        self.assertEqual(summary["supplied"], [])  # a tier-2 driver the engine agreed with throughout
        self.assertEqual(summary["accepted"], [])
        self.assertEqual([t.get("do", t.get("attempt")) for t in new["transitions"]], [t.get("do", t.get("attempt")) for t in old["transitions"]])
        self.assertEqual(new["author"]["kind"], "tool")
        self.assertIn("objects.*.in_combat", new["x-not-projected"])  # the adapter's declared unsupported facts, at their defaults
        self.assertEqual([(entry["verdict"], entry["scope"]) for entry in review], [("replayed", {"transitions": [1, 28]})])
        # Sleight of Hand: "put the other on the bottom of your library": the card stays in its zone, so 400.7
        # does not apply and the delta is a reorder, not a re-entry
        self.assertEqual(new["transitions"][11]["do"], {"kind": "choose_objects", "objects": ["C048"]})
        self.assertEqual(new["transitions"][11]["delta"]["zones"], {"A.library": {"bottom": ["C049"]}})
        self.assertNotIn("C049", new["transitions"][11]["delta"]["objects"])

    def test_a_drafted_scenario_is_recorded_as_the_rules_say(self):
        with tempfile.TemporaryDirectory() as out:
            summary = from_driver(LINKED, Path(out), ENGINE, ORACLE, tier=2)
            new = record(Path(out) / "record.json")
        self.assertEqual(summary["replay"]["levels"]["L4"], "pass")
        self.assertEqual(new["tier"], 2)
        deltas = [t["delta"] for t in new["transitions"]]
        plains = {f"plains{i}": {"tapped": True} for i in range(3)}
        swamps = {f"swamp{i}": {"tapped": True} for i in range(3)}
        # T1: {1}{W}{W} paid with three Plains tapped (601.2g, 605.3a, 107.5); the Hunter resolves and enters (608.3);
        # its enters trigger is put on the stack with empty targets and A is asked for them (603.3d, spec 4.1)
        self.assertEqual(deltas[0], {"pending": {"seat": "A", "kind": "choose_targets", "about": {"object": "S1"}},
                                     "objects": {"first": {"zone": "battlefield"}, **plains,
                                                 "S1": {"zone": "stack", "kind": "ability", "source": "first", "ability": {"face": 0, "paragraph": 1}, "controller": "A"}}})
        # T2: the target chosen; the "may" is decided as the ability resolves, with it still on the stack (608.2d, spec 4.2)
        self.assertEqual(deltas[1], {"pending": {"seat": "A", "kind": "optional", "about": {"object": "S1"}}, "objects": {"S1": {"targets": [["turtle"]]}}})
        # T3: the Turtle exiled, the ability gone (608.2n), the active player with priority (117.3b)
        self.assertEqual(deltas[2], {"pending": {"seat": "A", "kind": "priority"}, "objects": {"S1": None, "turtle": {"zone": "exile"}}})
        # T4: Murder destroys the Hunter (701.8a, 404.1) and goes to the graveyard last (608.2n); the leaves trigger
        # returns the Turtle under its owner's control (607.2a), a new object (400.7); the Swamps paid (601.2g)
        self.assertEqual(deltas[3], {"objects": {"kill": {"zone": "A.graveyard"}, "first": {"zone": "A.graveyard"}, "turtle": {"zone": "battlefield"}, **swamps},
                                     "zones": {"A.graveyard": {"top": ["kill", "first"]}}})
        # the tier-1 draft stated no tapped status: the engine supplied exactly the lands tapped for mana (107.5)
        self.assertEqual(sorted((s["after"], s["path"]) for s in summary["supplied"]),
                         sorted([(n, f"objects.plains{i}.tapped") for n in (1, 2, 3, 4) for i in range(3)] + [(4, f"objects.swamp{i}.tapped") for i in range(3)]))
        self.assertEqual(summary["accepted"], [])
        self.assertEqual(new["x-source"]["driver_tier"], 1)

    def test_the_prevention_scenario_consumes_the_shield_as_the_rules_say(self):
        with tempfile.TemporaryDirectory() as out:
            summary = from_driver(PREVENTION, Path(out), ENGINE, ORACLE)
            new = record(Path(out) / "record.json")
        self.assertEqual(summary["supplied"], [])  # a tier-2 draft that the engine agreed with throughout
        deltas = [t.get("delta") for t in new["transitions"]]
        self.assertEqual(deltas[0], {"objects": {"healer": {"tapped": True}}})  # 107.5: tapped for its cost; the shield is not a projected fact
        # 615.6, 615.7: one damage to each player, B's prevented; 120.3a
        self.assertEqual(deltas[1], {"objects": {"spewer": {"tapped": True}}, "players": {"A": {"life": 19}}})
        # the shield's last point prevents one of Shock's two (615.7); 601.2h: one R from the pool
        self.assertEqual(deltas[2], {"objects": {"shock0": {"zone": "A.graveyard"}}, "players": {"A": {"mana": {"R": 1}}, "B": {"life": 19}}})
        # 122.1a, 704.5g: 1/6 with 4 + 2 damage is destroyed as a state-based action
        self.assertEqual(deltas[3], {"objects": {"shock1": {"zone": "A.graveyard"}, "turtle": {"zone": "B.graveyard"}}, "players": {"A": {"mana": {}}}})
        self.assertEqual(deltas[4], {"objects": {"healer1": {"tapped": True}}})  # 615.7: a second, distinct shield
        self.assertIsNone(deltas[5])  # 733.1: the refused re-activation of the tapped Healer changes nothing
        self.assertTrue(new["transitions"][5].get("illegal"))

    def test_a_draft_the_engine_disagrees_with_on_a_tier_one_fact_is_refused(self):
        draft = record(LINKED)
        draft["transitions"][2]["delta"]["objects"]["turtle"] = {"zone": "B.graveyard"}  # 608.2d: the Turtle is exiled, not destroyed
        with tempfile.TemporaryDirectory() as out:
            with self.assertRaises(RecordingError) as refused:
                from_driver(write_draft(Path(out), draft), Path(out) / "rec", ENGINE, ORACLE, tier=2)
            self.assertIn("tier-1", str(refused.exception))
            self.assertFalse((Path(out) / "rec" / "record.json").exists())

    def test_a_directory_that_holds_a_record_is_not_overwritten(self):
        # records are immutable (spec 9): a successor goes beside them, never over them
        with tempfile.TemporaryDirectory() as out:
            (Path(out) / "record.json").write_text("{}", encoding="utf-8")
            (Path(out) / "review.jsonl").write_text('{"kept": true}\n', encoding="utf-8")
            with self.assertRaises(RecordingError):
                from_driver(PREVENTION, Path(out), ENGINE, ORACLE)
            self.assertEqual((Path(out) / "review.jsonl").read_text(encoding="utf-8"), '{"kept": true}\n')


if __name__ == "__main__":
    unittest.main()
