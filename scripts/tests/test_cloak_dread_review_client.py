"""Actual-client checks of cloak, manifest dread and reveal lifetime."""
import os
import sys
import unittest
from pathlib import Path
sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import play
import test_multiplayer_outcome_client as helpers

def shared(name):
    return getattr(helpers.MultiplayerOutcomeClientTests, name)

class CloakDreadReviewClientTests(unittest.TestCase):
    new = shared("new")
    view = shared("view")
    ids = shared("ids")
    hand = shared("hand")
    cast = shared("cast")
    validate_action = shared("validate_action")
    choose = shared("choose")
    reject = shared("reject")
    pass_round = shared("pass_round")
    close_engine = shared("close_engine")

    def setUp(self):
        self.engine = play.Engine(audit=True, executable=os.environ.get("MAGIC_REVIEW_ENGINE"))
        self.addCleanup(self.close_engine)

    def activate(self, name, ability, targets=None, *, mana=(), spend=None):
        source, = [o for o in self.ids(name) if o in self.engine.state["battlefield"]]
        self.engine.act({"kind": "activate", "source": source, "ability": ability, "targets": targets or [],
                         "plan": [{"source": o, "ability": 0} for o in mana],
                         **({"mana": spend} if spend is not None else {})})

    def until(self, kind):
        for _ in range(100):
            asked = self.engine.state["asked"]
            if asked["kind"] == kind:
                return asked
            self.assertEqual(asked["kind"], "priority", asked)
            self.engine.act({"kind": "pass"})
        self.fail(f"no {kind}")

    def down(self):
        return [o for o in self.engine.state["battlefield"] if "face_down" in self.engine.state["objects"][str(o)]]

    def setup_shrieker(self, card):
        self.new([{"battlefield": ["Hauntwoods Shrieker", "Forest", "Forest"]},
                  {"battlefield": ["Scroll of Fate"], "hand": [card, "Mountain"]}, {}])
        self.engine.act({"kind": "pass"})
        self.activate("Scroll of Fate", 0)
        self.pass_round()
        wanted = next(o for o in self.engine.state["asked"]["among"] if self.engine.state["objects"][str(o)]["name"] == card)
        self.choose({"objects": [wanted]})
        permanent, = self.down()
        self.assertIsNone(self.engine.state["objects"][str(permanent)]["face_down"]["card"])
        self.activate("Hauntwoods Shrieker", 1, [[{"object": permanent}]], mana=self.ids("Forest", zone="battlefield", controller=0)[:2], spend={'G': 2})
        self.pass_round()
        return permanent

    def test_shrieker_reveal_then_decline_is_audited_and_hides_again(self):
        permanent = self.setup_shrieker("Grizzly Bears")
        self.assertEqual((self.engine.state["asked"]["kind"], self.engine.state["asked"]["player"]), ("may", 0))
        self.assertEqual(self.engine.state["objects"][str(permanent)]["face_down"]["card"]["name"], "Grizzly Bears")
        observer = self.view(2)
        self.assertEqual(observer["objects"][str(permanent)]["face_down"]["card"]["name"], "Grizzly Bears")
        self.reject(1, {"kind": "choose", "answer": "yes"})
        self.choose("no")
        self.assertIsNone(self.engine.state["objects"][str(permanent)]["face_down"]["card"])
        self.assertIsNone(self.view(2)["objects"][str(permanent)]["face_down"]["card"])

    def test_shrieker_reveal_then_accept_turns_the_opponents_creature_up(self):
        permanent = self.setup_shrieker("Grizzly Bears")
        self.assertEqual(self.engine.state["asked"]["kind"], "may")
        self.choose("yes")
        self.assertNotIn("face_down", self.engine.state["objects"][str(permanent)])
        self.assertEqual(self.engine.state["objects"][str(permanent)]["name"], "Grizzly Bears")
        self.assertEqual(self.engine.state["objects"][str(permanent)]["controller"], 1)

    def test_shrieker_revealed_land_is_not_offered_a_turn_up_choice(self):
        permanent = self.setup_shrieker("Island")
        self.assertEqual(self.engine.state["asked"]["kind"], "priority")
        self.assertIn("face_down", self.engine.state["objects"][str(permanent)])
        self.assertTrue(any(c["name"] == "Island" for row in self.engine.state["observations"] if row["kind"] == "revealed" for c in row["cards"]))

    def test_oculus_opponents_upkeep_looks_only_at_its_controllers_library(self):
        self.engine.ask({"cmd": "new", "players": [
            {"library": ["Forest"] * 8},
            {"battlefield": ["Abhorrent Oculus"], "library": ["Mountain"] * 8 + ["Island", "Grizzly Bears"]},
            {"library": ["Swamp"] * 8}]})
        asked = self.until("objects")
        self.assertEqual(asked["player"], 1)
        among = asked["among"]
        self.assertEqual({self.engine.state["objects"][str(o)]["name"] for o in among}, {"Island", "Grizzly Bears"})
        for observer in [0, 2]:
            view = self.view(observer)
            self.assertNotIn("among", view["asked"])
            self.assertFalse(set(map(str, among)) & set(view["objects"]))
            self.assertFalse(view["observations"])
        self.reject(0, {"kind": "choose", "answer": {"objects": [among[0]]}})
        bear = next(o for o in among if self.engine.state["objects"][str(o)]["name"] == "Grizzly Bears")
        self.choose({"objects": [bear]})
        permanent, = self.down()
        self.assertEqual(self.engine.state["objects"][str(permanent)]["controller"], 1)
        for observer in [0, 2]:
            view = self.view(observer)
            self.assertIsNone(view["objects"][str(permanent)]["face_down"]["card"])
            self.assertEqual(view["players"][1]["library"], 8)
            self.assertEqual([view["objects"][str(o)]["name"] for o in view["players"][1]["graveyard"]], ["Island"])

    def test_fear_of_impostors_uses_countered_spells_last_controller(self):
        self.new([{"battlefield": ["Island"] * 3, "hand": ["Fear of Impostors"]},
                  {"battlefield": ["Mountain"], "hand": ["Lightning Bolt"], "library": ["Forest"] * 8 + ["Grizzly Bears", "Island"]}, {}])
        self.engine.act({"kind": "pass"})
        self.cast("Lightning Bolt", [[{"player": 2}]], mana=self.ids("Mountain", zone="battlefield", controller=1)[:1], spend={'R': 1})
        self.engine.act({"kind": "pass"})
        self.engine.act({"kind": "pass"})
        self.cast("Fear of Impostors", mana=self.ids("Island", zone="battlefield", controller=0)[:3], spend={'U': 3})
        self.pass_round()
        asked = self.engine.state["asked"]
        self.assertEqual((asked["kind"], asked["player"]), ("targets", 0))
        bolt = next(o for o in self.engine.state["stack"] if self.engine.state["objects"][str(o)]["name"] == "Lightning Bolt")
        self.engine.act({"kind": "targets", "targets": [[{"object": bolt}]]})
        self.pass_round()
        asked = self.engine.state["asked"]
        self.assertEqual((asked["kind"], asked["player"]), ("objects", 1))
        self.assertEqual(self.engine.state["players"][2]["life"], 20)
        self.assertNotIn(bolt, self.engine.state["stack"], "the Bolt is already countered; the trigger is still resolving")
        self.assertNotIn("among", self.view(0)["asked"])
        chosen = asked["among"][0]
        self.choose({"objects": [chosen]})
        permanent, = self.down()
        self.assertEqual(self.engine.state["objects"][str(permanent)]["controller"], 1)
        self.assertEqual(self.engine.state["players"][2]["life"], 20)

    def test_cloaked_way_is_offered_and_wrong_manifested_way_is_rejected(self):
        self.new([{"battlefield": ["Forest"] * 4 + ["Island"] * 2, "hand": ["Hide in Plain Sight"],
                   "library": ["Forest"] * 12 + ["Exit Specialist", "Island"]},
                  {"battlefield": ["Grizzly Bears"]}])
        self.cast("Hide in Plain Sight", mana=self.ids("Forest", zone="battlefield", controller=0)[:4], spend={'G': 4})
        self.pass_round()
        asked = self.engine.state["asked"]
        specialist = next(o for o in asked["among"] if self.engine.state["objects"][str(o)]["name"] == "Exit Specialist")
        self.choose({"objects": [specialist]})
        self.choose({"objects": [self.engine.state["asked"]["among"][0]]})
        permanent, = [o for o in self.down() if self.engine.state["objects"][str(o)]["face_down"]["card"]["name"] == "Exit Specialist"]
        self.assertEqual(self.engine.state["objects"][str(permanent)]["face_down"]["by"], "cloaked")
        plan = [{"source": o, "ability": 0} for o in self.ids("Island", zone="battlefield", controller=0)[:2]]
        for way in ("cloaked", "disguise"):
            self.validate_action({"kind": "turn_face_up", "permanent": permanent, "way": way, "plan": plan, "mana": {"U": 2}})
        self.reject(0, {"kind": "turn_face_up", "permanent": permanent, "way": "manifested", "plan": plan, "mana": {"U": 2}})
        self.engine.act({"kind": "turn_face_up", "permanent": permanent, "way": "cloaked", "plan": plan, "mana": {"U": 2}})
        self.assertNotIn("face_down", self.engine.state["objects"][str(permanent)])
        self.assertEqual(self.engine.state["objects"][str(permanent)]["name"], "Exit Specialist")

    def test_two_cloak_choices_remain_distinct_when_the_first_entry_is_blocked(self):
        self.new([{"battlefield": ["Forest"] * 4, "hand": ["Hide in Plain Sight"],
                   "library": ["Island"] * 8 + ["Grizzly Bears", "Mountain"]},
                  {"battlefield": ["Grafdigger's Cage"]}])
        library_before = self.engine.state["players"][0]["library"]
        self.cast("Hide in Plain Sight", mana=self.ids("Forest", zone="battlefield", controller=0)[:4], spend={'G': 4})
        self.pass_round()
        asked = self.engine.state["asked"]
        first = next(o for o in asked["among"] if self.engine.state["objects"][str(o)]["name"] == "Grizzly Bears")
        self.choose({"objects": [first]})
        self.assertNotIn(first, self.engine.state["asked"]["among"], "two of the cards looked at means two distinct cards, even if the first cannot enter")
        self.reject(0, {"kind": "choose", "answer": {"objects": [first]}})
        self.choose({"objects": [self.engine.state["asked"]["among"][0]]})
        self.assertEqual(self.down(), [], "Cage prevents both entries")
        self.assertEqual(self.engine.state["asked"]["kind"], "priority")
        self.assertEqual(self.engine.state["players"][0]["library"], library_before)

if __name__ == "__main__":
    unittest.main()
