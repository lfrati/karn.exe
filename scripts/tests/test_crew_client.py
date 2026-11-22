"""Crew checks through the actual Python/JSON client: Smuggler's Copter's
crew 1 is offered as a choice of untapped other creatures with each one's power (702.122a), a
selection short of the total power is refused, and the crewed Copter is a 3/3 artifact
creature (301.7b) that both seats see crewed (702.122b, e)."""
from client_support import ClientTestCase


class CrewClientTests(ClientTestCase):
    def ids(self, name):
        return sorted(int(i) for i, o in self.view(0)["objects"].items() if o["name"] == name)

    def reject(self, player, move):
        state = self.engine.state
        with self.assertRaises(SystemExit):
            self.engine.send({"cmd": "act", "player": player, "move": move, "viewer": "asked",
                              "session_id": state["session_id"], "request_id": state["request_id"]})
        self.assertEqual(self.view(state["viewer"])["request_id"], state["request_id"])

    def test_crew_is_a_total_power_choice_and_both_seats_see_the_copter_crewed(self):
        self.new([{"battlefield": ["Smuggler's Copter", "Ornithopter", "Grizzly Bears"]}, {}])
        [copter], [thopter], [bears] = self.ids("Smuggler's Copter"), self.ids("Ornithopter"), self.ids("Grizzly Bears")
        self.assertNotIn("power", self.view(0)["objects"][str(copter)], "a noncreature Vehicle has no power (301.7a)")
        self.assertEqual(self.view(0)["objects"][str(thopter)]["power"], 0)
        self.assertEqual(self.view(0)["objects"][str(bears)]["power"], 2)
        move = {"kind": "activate", "source": copter, "ability": 3, "modes": [], "targets": [], "plan": []}
        self.reject(0, {**move, "paying": [[thopter]]})
        paying = [[bears]]  # CR 702.122a: power two satisfies crew one.
        # each response tells its viewer the events since that viewer was last told
        told = {0: [], 1: []}
        for act in [{**move, "paying": paying}, {"kind": "pass"}, {"kind": "pass"}]:
            state = self.engine.act(act)
            told[state["viewer"]] += state["events"]
        for seat in (0, 1):
            seen = self.view(seat)
            told[seat] += seen["events"]
            copter_now = seen["objects"][str(copter)]
            self.assertIn("Creature", copter_now["types"])
            self.assertEqual((copter_now["power"], copter_now["toughness"]), (3, 3))
            events = [e["event"] for e in told[seat]]
            self.assertIn("crews", events)
            self.assertIn("became_crewed", events)
