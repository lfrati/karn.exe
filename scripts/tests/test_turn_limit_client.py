"""Turn-limit checks through the actual Python/JSON client."""
from client_support import ClientTestCase


class TurnLimitClientTests(ClientTestCase):
    def tokens(self, name):
        return [o for key, o in self.engine.state["objects"].items()
                if int(key) in self.engine.state["battlefield"] and name in o["name"]]

    def target_card(self, name):
        self.assertEqual(self.engine.state["asked"]["kind"], "targets")
        card = next(i for i in self.ids(name) if i in self.engine.state["players"][0]["graveyard"])
        self.engine.act({"kind": "targets", "targets": [[{"object": card}]]})
        return card

    def test_tataru_entry_target_draw_owner_and_treasure_trigger_cap(self):
        self.new([{"hand": ["Tataru Taru"], "battlefield": ["Plains"] * 2}, {}, {}])
        self.cast("Tataru Taru",mana=self.ids("Plains"), spend={"W": 2})
        self.pass_round()
        self.assertEqual((self.engine.state["asked"]["kind"], self.engine.state["asked"]["player"]), ("targets", 0))
        self.reject(0, {"kind": "targets", "targets": [[{"player": 0}]]})
        self.engine.act({"kind": "targets", "targets": [[{"player": 1}]]})
        before = len(self.hand(0))
        self.pass_round()
        self.assertEqual(len(self.hand(0)), before + 1)
        self.assertEqual((self.engine.state["asked"]["kind"], self.engine.state["asked"]["player"]), ("may", 1))
        self.reject(0, {"kind": "choose", "answer": "yes"})
        self.choose("yes")
        self.pass_round()
        self.assertEqual(len(self.tokens("Treasure")), 1)
        self.assertTrue(self.tokens("Treasure")[0]["tapped"])

    def test_graha_decline_then_pay_keeps_x_and_suppresses_later_noncreature_trigger(self):
        self.new([{"battlefield": ["G'raha Tia, Scion Reborn", "Island", "Mountain"],
                   "hand": ["Opt", "Shock", "Ornithopter", "Everflowing Chalice"]}, {}, {}])
        self.cast("Opt",mana=self.ids("Island")[:1], spend={'U': 1})
        self.pass_round()
        self.assertEqual((self.engine.state["asked"]["kind"], self.engine.state["asked"]["life"]), ("pay", 1))
        self.reject(1, {"kind": "choose", "answer": "yes"})
        self.choose("no")
        # Another spell can trigger before the first spell resolves.
        self.cast("Shock", [[{"player": 2}]],mana=self.ids("Mountain")[:1], spend={'R': 1})
        self.pass_round()
        self.choose({"pay":{"plan":[]}})
        self.assertEqual(self.engine.state["players"][0]["life"], 19)
        self.assertEqual(self.tokens("Hero")[0]["counters"]["+1/+1"], 1)
        # Creature casts don't trigger; the paid source also can't trigger for a later noncreature.
        while self.engine.state["stack"]:
            if self.engine.state["asked"]["kind"] == "arrange":
                self.choose({"top": self.engine.state["asked"]["cards"], "away": []})
            else:
                self.engine.act({"kind": "pass"})
        self.cast("Everflowing Chalice")
        self.assertEqual(len(self.engine.state["stack"]), 1)

    def test_graha_phyrexian_cost_and_multiple_pending_instances(self):
        self.new([{"battlefield": ["G'raha Tia, Scion Reborn"],
                   "hand": ["Gut Shot", "Gut Shot"]}, {}, {}])
        # Paying the Phyrexian alternative doesn't change mana value.
        def shot():
            self.cast("Gut Shot",[[{"player":2}]],life=[True])
        shot()
        shot()
        self.assertEqual(len(self.engine.state["stack"]), 4)
        self.pass_round()
        self.choose({"pay":{"plan":[]}})
        self.assertEqual(self.tokens("Hero")[0]["counters"]["+1/+1"], 1)
        self.pass_round()
        self.pass_round()
        self.assertEqual(self.engine.state["asked"]["kind"], "priority")
        self.assertEqual(len(self.tokens("Hero")), 1)

    def test_emet_grouped_losses_offer_targeted_discounted_cast_and_public_exile(self):
        self.new([{"battlefield": ["Emet-Selch of the Third Seat", "Swamp", "Swamp", "Swamp", "Island"],
                   "hand": ["Exsanguinate"], "graveyard": ["Divination"]}, {}, {}, {}])
        self.cast("Exsanguinate",x=1,mana=self.ids("Swamp")[:3], spend={'B': 3})
        self.pass_round()
        self.target_card("Divination")
        self.assertEqual(len(self.engine.state["stack"]), 1, "three simultaneous losses make one trigger")
        self.pass_round()
        self.assertEqual(self.engine.state["asked"]["kind"], "cast")
        self.reject(1, {"kind": "choose", "answer": "no"})
        self.cast("Divination",card=next(i for i in self.ids("Divination") if i in self.engine.state["players"][0]["graveyard"]),mana=self.ids("Island")[:1], spend={'U': 1})
        self.assertEqual(len(self.engine.state["stack"]), 1)
        self.pass_round()
        exiled = [i for i in self.ids("Divination") if i in self.engine.state["exile"]]
        self.assertEqual(len(exiled), 1)
        for viewer in [1, 2, 3]:
            self.assertEqual(self.view(viewer)["objects"][str(exiled[0])]["name"], "Divination")

    def test_graha_zero_payment_is_taken_and_allowance_resets_on_an_opponents_turn(self):
        self.new([{"battlefield": ["G'raha Tia, Scion Reborn"],
                   "hand": ["Everflowing Chalice", "Gut Shot"]}, {}, {}])
        self.cast("Everflowing Chalice")
        self.pass_round()
        self.assertEqual(self.engine.state["asked"]["kind"], "pay")
        self.assertEqual(self.engine.state["asked"].get("life", 0), 0)
        self.choose({"pay":{"plan":[]}})
        self.assertEqual(self.tokens("Hero")[0]["counters"], {})
        for _ in range(100):
            if self.engine.state["active"] == 1 and self.engine.state["part"] == "PrecombatMain":
                break
            self.engine.act({"kind": "attack", "attacks": []} if self.engine.state["asked"]["kind"] == "attackers" else {"kind": "pass"})
        self.assertEqual(self.engine.state["active"], 1)
        self.engine.act({"kind": "pass"})
        self.engine.act({"kind": "pass"})
        self.cast("Gut Shot",[[{"player":2}]],life=[True])
        self.pass_round()
        self.assertEqual(self.engine.state["asked"]["kind"], "pay")
        self.choose({"pay":{"plan":[]}})
        self.assertEqual(len(self.tokens("Hero")), 2)

    def test_emet_nested_cast_reversal_keeps_offer_and_action_available(self):
        self.new([{"battlefield": ["Emet-Selch of the Third Seat", "Swamp", "Swamp", "Swamp", "Island", "Empowered Autogenerator"],
                   "hand": ["Exsanguinate"], "graveyard": ["Divination"]}, {}, {}])
        self.cast("Exsanguinate",x=1,mana=self.ids("Swamp")[:3], spend={'B': 3})
        self.pass_round()
        self.target_card("Divination")
        self.pass_round()
        source, = self.ids("Empowered Autogenerator")
        self.cast("Divination",card=next(i for i in self.ids("Divination") if i in self.engine.state["players"][0]["graveyard"]), plan=[{"source": source, "ability": 1, "paying": [], "adding": []}])
        asked = self.engine.state["asked"]
        self.assertEqual(asked["kind"], "mana")
        red = next(option for option in asked["options"] if option == ["R"])
        self.choose({"mana": red})
        self.assertEqual(self.engine.state["asked"]["kind"], "payment")
        self.reject(1, {"kind": "reverse_payment"})
        self.engine.act({"kind": "reverse_payment"})
        self.assertEqual(self.engine.state["asked"]["kind"], "cast")
        self.assertEqual(self.engine.state["objects"][str(source)]["counters"]["charge"], 1)
        self.cast("Divination",card=next(i for i in self.ids("Divination") if i in self.engine.state["players"][0]["graveyard"]),mana=self.ids("Island")[:1], spend={'U': 1})
        self.assertEqual(self.engine.state["asked"]["kind"], "priority")
        self.assertEqual(self.engine.state["objects"][str(source)]["counters"]["charge"], 1)
        self.pass_round()
        self.assertTrue(any(self.engine.state["objects"][str(i)]["name"] == "Divination" for i in self.engine.state["exile"]))

    def test_emet_decline_allows_a_later_targeted_offer(self):
        self.new([{"battlefield": ["Emet-Selch of the Third Seat", "Mountain", "Mountain", "Island"],
                   "hand": ["Shock", "Shock"], "graveyard": ["Divination"]}, {}, {}])
        self.cast("Shock", [[{"player": 1}]],mana=self.ids("Mountain")[:1], spend={'R': 1})
        self.pass_round()
        self.target_card("Divination")
        self.pass_round()
        self.choose("no")
        self.cast("Shock", [[{"player": 2}]],mana=self.ids("Mountain")[1:2], spend={'R': 1})
        self.pass_round()
        self.target_card("Divination")
        self.pass_round()
        self.cast("Divination",card=next(i for i in self.ids("Divination") if i in self.engine.state["players"][0]["graveyard"]),mana=self.ids("Island")[:1], spend={'U': 1})
        self.assertEqual(self.engine.state["asked"]["kind"], "priority")

# Only this class supplies tests; imported methods are reused as fixture helpers.
