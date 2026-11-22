"""Full pinned counter-removal consumers through
the real audited client. Effect removal takes what is there (609.3). Coalition Relic's
"for each charge counter removed this way" may give a different color for each one (ruling
34810d223491d650). The trigger isn't a mana ability (605.1b), so it adds the mana on the
stack. Axebane Guardian's mana ability asks the same per-color choices above three mana, as
one activation."""
import unittest
from client_support import ClientTestCase


class CounterRemovalReviewClientTests(ClientTestCase):
    def on_battlefield(self, name):
        return [o for o in self.ids(name) if o in self.engine.state["battlefield"]]

    def activate(self, source, ability, targets=None, mana=(), spend=None):
        self.engine.act({"kind":"activate","source":source,"ability":ability,"targets":targets or [],
                         "plan":[{"source":o,"ability":0} for o in mana],"paying":[], **({"mana": spend} if spend is not None else {})})

    def counters(self, obj):
        return self.engine.state["objects"][str(obj)].get("counters", {})

    def step(self):
        """Pass priority, skip combat, order triggers as listed and discard what is asked;
        False for anything else."""
        asked = self.engine.state["asked"]
        if asked["kind"] == "attackers":
            self.engine.act({"kind": "attack", "attacks": []})
        elif asked["kind"] == "blockers":
            self.engine.act({"kind": "block", "blocks": []})
        elif asked["kind"] == "discard":
            self.engine.act({"kind": "discard", "cards": self.view(asked["player"])["players"][asked["player"]]["hand"][:asked["count"]]})
        elif asked["kind"] == "objects" and asked.get("verb") == "Discard":
            self.engine.act({"kind": "choose", "answer": {"objects": asked["among"][:asked["count"]]}})
        elif asked["kind"] == "order":
            self.engine.act({"kind": "order", "order": list(range(len(asked["abilities"])))})
        elif asked["kind"] == "priority":
            self.engine.act({"kind": "pass"})
        else:
            return False
        return True

    def to_next_choice(self):
        for _ in range(60):
            if not self.step():
                return self.engine.state["asked"]
        self.fail("no choice")

    def settle(self):
        """Resolve the whole stack."""
        for _ in range(60):
            if not self.engine.state["stack"]:
                return
            self.assertTrue(self.step(), self.engine.state["asked"])
        self.fail("stack did not empty")

    def next_own_main(self):
        """Pass to player 0's next first main phase, where its creatures are no longer new."""
        turn = self.engine.state["turn"]
        for _ in range(80):
            state = self.engine.state
            if state["turn"] != turn and state["active"] == 0 and state["part"] == "PrecombatMain" and state["asked"]["kind"] == "priority":
                return
            self.assertTrue(self.step(), state["asked"])
        self.fail("no next turn")

    def relic_with(self, coretappers):
        """Coalition Relic, whose first-main-phase trigger is already waiting when the game
        reaches that phase. Each sacrificed Coretapper puts two charge counters on it in
        response, and the trigger removes what is there when it resolves (608.2h)."""
        self.new([{"battlefield": ["Coalition Relic"] + ["Coretapper"] * coretappers}, {}])
        relic, = self.on_battlefield("Coalition Relic")
        for tapper in self.on_battlefield("Coretapper"):
            self.activate(tapper, 1, [[{"object": relic}]])
            self.pass_round()
        self.assertEqual(self.counters(relic), {"charge": 2 * coretappers})
        return relic

    def relic_mana(self, relic):
        return [e["mana"] for e in self.engine.state["events"] if e.get("event") == "mana_added" and e.get("source") == relic]

    def test_coalition_with_four_counters_asks_each_color_and_adds_them_at_once(self):
        relic = self.relic_with(2)
        for color in ["W", "U", "B", "R"]:
            asked = self.to_next_choice()
            self.assertEqual((asked["kind"], asked["player"]), ("mana", 0))
            self.assertEqual(asked["options"], [["W"], ["U"], ["B"], ["R"], ["G"]])
            if color == "W":
                self.reject(0, {"kind": "choose", "answer": {"mana": ["C"]}})
                self.reject(0, {"kind": "choose", "answer": {"mana": ["W", "U"]}})
            self.assertEqual(self.relic_mana(relic), [], "mana added before every color was chosen")
            self.engine.act({"kind": "choose", "answer": {"mana": [color]}})
        self.assertEqual(self.engine.state["asked"]["kind"], "priority")
        self.assertEqual(self.counters(relic), {})
        pool = self.engine.state["players"][0]["pool"]
        self.assertEqual({k: pool[k] for k in "WUBRGC"}, {"W": 1, "U": 1, "B": 1, "R": 1, "G": 0, "C": 0})
        self.assertEqual(self.relic_mana(relic), [["W", "U", "B", "R"]])

    def test_coalition_with_two_counters_keeps_the_combined_choice(self):
        relic = self.relic_with(1)
        asked = self.to_next_choice()
        self.assertEqual(asked["kind"], "mana")
        self.assertEqual(len(asked["options"]), 15)
        self.assertIn(["U", "R"], asked["options"])
        self.engine.act({"kind": "choose", "answer": {"mana": ["U", "R"]}})
        self.assertEqual(self.counters(relic), {})
        self.assertEqual(self.relic_mana(relic), [["U", "R"]])

    def axebane(self, walls):
        self.new([{"battlefield": ["Axebane Guardian"] + ["Wall of Wood"] * walls}, {}])
        self.next_own_main()
        guardian, = self.on_battlefield("Axebane Guardian")
        return guardian

    def test_axebane_mana_ability_asks_each_color_above_three(self):
        # Axebane itself has defender: two Walls make three defenders, three Walls four.
        guardian=self.axebane(2)
        self.engine.act({"kind":"activate","source":guardian,"ability":1,"targets":[],"adding":[["W","U","G"]]})
        self.assertEqual(self.engine.state["players"][0]["pool"]["G"],1)
        guardian=self.axebane(3)
        move={"kind":"activate","source":guardian,"ability":1,"targets":[],"plan":[],"paying":[]}
        self.reject(0, {**move, "adding": [["W"], ["U"], ["B"]]})
        self.reject(0, {**move, "adding": [["W"], ["U"], ["B"], ["W", "U"]]})
        self.assertFalse(self.engine.state["objects"][str(guardian)].get("tapped"))
        self.engine.act({**move, "adding": [["W"], ["G"], ["G"], ["R"]]})
        pool = self.engine.state["players"][0]["pool"]
        self.assertEqual({k: pool[k] for k in "WUBRGC"}, {"W": 1, "U": 0, "B": 0, "R": 1, "G": 2, "C": 0})
        self.assertEqual([e["mana"] for e in self.engine.state["events"]
                          if e.get("event") == "mana_added" and e.get("source") == guardian], [["W", "G", "G", "R"]])
        self.assertEqual(self.engine.state["asked"]["kind"], "priority", "a mana ability uses no stack")

    def test_shrewd_hatchling_loses_one_counter_per_color_trigger(self):
        self.new([{"hand": ["Shrewd Hatchling", "Divination", "Thoughtflare"], "battlefield": ["Island"] * 10 + ["Mountain"] * 5}, {}])
        self.cast("Shrewd Hatchling",mana=self.on_battlefield("Island")[:4], spend={'U': 4})
        self.settle()
        hatchling, = self.on_battlefield("Shrewd Hatchling")
        self.assertEqual(self.counters(hatchling), {"-1/-1": 4})
        self.cast("Divination",mana=self.on_battlefield("Island")[4:7], spend={'U': 3})
        self.settle()
        self.assertEqual(self.counters(hatchling), {"-1/-1": 3})
        self.cast("Thoughtflare",mana=self.on_battlefield("Island")[7:10]+self.on_battlefield("Mountain")[:2], spend={'U': 3, 'R': 2})
        while self.engine.state["asked"]["kind"] != "objects":
            self.assertTrue(self.step())
        hand=self.view(0)["players"][0]["hand"]
        forests=[o for o in hand if self.view(0)["objects"][str(o)]["name"]=="Forest"]
        discarded=forests[:2]
        self.assertEqual(len(discarded),2)
        self.engine.act({"kind":"choose","answer":{"objects":discarded}})
        self.engine.act({"kind":"choose","answer":{"objects":discarded[::-1]}})
        self.settle()
        self.assertEqual(self.counters(hatchling), {"-1/-1": 1}, "a blue and red spell triggers both")

    def test_chainbreaker_removes_one_and_does_nothing_without_counters(self):
        self.new([{"hand": ["Shrewd Hatchling"], "battlefield": ["Chainbreaker", "Grizzly Bears"] + ["Island"] * 4 + ["Mountain"] * 6}, {}])
        self.cast("Shrewd Hatchling",mana=self.on_battlefield("Island")[:4], spend={'U': 4})
        self.settle()
        self.next_own_main()
        hatchling, = self.on_battlefield("Shrewd Hatchling")
        breaker, = self.on_battlefield("Chainbreaker")
        bears, = self.on_battlefield("Grizzly Bears")
        self.activate(breaker, 1, [[{"object": hatchling}]],mana=self.on_battlefield("Mountain")[:3], spend={"R": 3})
        self.settle()
        self.assertEqual(self.counters(hatchling), {"-1/-1": 3})
        self.next_own_main()
        self.activate(breaker, 1, [[{"object": bears}]],mana=self.on_battlefield("Mountain")[:3], spend={"R": 3})
        self.settle()
        self.assertEqual(self.counters(bears), {})
        self.assertIn(bears, self.engine.state["battlefield"])
        self.assertTrue(self.engine.state["objects"][str(breaker)].get("tapped"))

    def droplet_upkeep(self):
        """Pass to the next upkeep trigger of Sun Droplet and say yes to it, or no if the yes
        is refused; whether it was asked at all before the next turn's draw step."""
        turn = self.engine.state["turn"]
        for _ in range(80):
            asked = self.engine.state["asked"]
            if asked["kind"] == "may":
                try:
                    self.engine.act({"kind": "choose", "answer": "yes"})
                except SystemExit:
                    self.engine.act({"kind": "choose", "answer": "no"})
                self.settle()
                return True
            state = self.engine.state
            if state["turn"] != turn and state["part"] == "Draw" and not state["stack"]:
                return False
            self.assertTrue(self.step(), asked)
        self.fail("no upkeep")

    def test_sun_droplet_gains_life_only_for_a_removed_counter(self):
        self.new([{"hand": ["Lightning Bolt"], "battlefield": ["Sun Droplet", "Mountain"]}, {}])
        droplet, = self.on_battlefield("Sun Droplet")
        self.cast("Lightning Bolt", [[{"player": 0}]],mana=self.on_battlefield("Mountain"), spend={"R": 1})
        self.settle()
        self.assertEqual(self.counters(droplet), {"charge": 3})
        self.assertTrue(self.droplet_upkeep())
        self.assertEqual((self.engine.state["players"][0]["life"], self.counters(droplet)), (18, {"charge": 2}))
        for _ in range(2):
            self.droplet_upkeep()
        self.assertEqual(self.counters(droplet), {})
        life = self.engine.state["players"][0]["life"]
        self.droplet_upkeep()
        self.assertEqual(self.engine.state["players"][0]["life"], life, "life for an impossible removal")


if __name__ == "__main__":
    unittest.main()
