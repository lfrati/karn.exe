"""Timing checks through the actual Python/JSON client."""
import unittest
from test_multiplayer_outcome_client import MultiplayerOutcomeClientTests as Helpers


class TimingClientTests(unittest.TestCase):
    setUpClass = classmethod(Helpers.setUpClass.__func__)
    setUp = Helpers.setUp
    close_engine = Helpers.close_engine
    new = Helpers.new
    view = Helpers.view
    hand = Helpers.hand
    ids = Helpers.ids
    pass_round = Helpers.pass_round
    choose = Helpers.choose
    reject = Helpers.reject
    cast = Helpers.cast
    validate_action = Helpers.validate_action

    def validate_cast(self, name, mana, *, spend, accepted=True):
        state = self.engine.state
        actor = state["asked"]["player"]
        cards = state["players"][actor]["hand"] + state["players"][actor]["graveyard"]
        card = next(o for o in cards if state["objects"][str(o)]["name"] == name)
        return self.validate_action({"kind": "cast", "card": card, "targets": [], "route": "normal", "mana": spend,
                                     "plan": [{"source": o, "ability": 0} for o in mana]}, accepted)

    def activate(self, ability, targets=None):
        source = next(i for i in self.ids("Teferi, Time Raveler")
                      if i in self.engine.state["battlefield"] and self.engine.state["objects"][str(i)]["controller"] == 0)
        self.engine.act({"kind": "activate", "source": source, "ability": ability, "targets": targets or [], "plan": []})
        return source

    def next_main(self, player):
        turn = self.engine.state["turn"]
        for _ in range(200):
            state = self.engine.state
            if state["turn"] > turn and state["active"] == player and state["part"] == "PrecombatMain":
                return
            self.engine.act({"kind": "pass"})
        self.fail("did not reach next main")

    def test_restriction_filters_instant_flash_and_explicit_commands_on_stack_and_other_turn(self):
        self.new([{"hand": ["Ornithopter", "Opt", "Shimmer Myr"], "battlefield": ["Island", "Forest", "Forest"]},
                  {"battlefield": ["Teferi, Time Raveler"]}, {}])
        self.validate_cast('Opt', self.ids('Island', zone="battlefield", controller=0)[:1], accepted=True, spend={'U': 1})
        self.validate_cast('Shimmer Myr', self.ids('Island', zone="battlefield", controller=0)[:1] + self.ids('Forest', zone="battlefield", controller=0)[:2], accepted=True, spend={'U': 1, 'G': 2})
        self.cast("Ornithopter")
        self.validate_cast('Opt', self.ids('Island', zone="battlefield", controller=0)[:1], accepted=False, spend={'U': 1})
        self.validate_cast('Shimmer Myr', self.ids('Island', zone="battlefield", controller=0)[:1] + self.ids('Forest', zone="battlefield", controller=0)[:2], accepted=False, spend={'U': 1, 'G': 2})
        self.reject(0, {"kind": "cast", "card": self.ids("Opt")[0], "targets": []})
        self.reject(0, {"kind": "cast", "card": self.ids("Shimmer Myr")[0], "targets": []})
        self.pass_round()
        self.validate_cast('Opt', self.ids('Island', zone="battlefield", controller=0)[:1], accepted=True, spend={'U': 1})
        self.next_main(1)
        self.engine.act({"kind": "pass"})
        self.engine.act({"kind": "pass"})
        self.assertEqual(self.engine.state["asked"]["player"], 0)
        self.validate_cast('Opt', self.ids('Island', zone="battlefield", controller=0)[:1], accepted=False, spend={'U': 1})
        self.reject(0, {"kind": "cast", "card": self.ids("Opt")[0], "targets": []})

    def test_controller_response_and_zero_target_minus_three_draw(self):
        self.new([{"battlefield": ["Teferi, Time Raveler", "Island"], "hand": ["Opt"]}, {}, {}])
        before = len(self.hand(0))
        source = self.activate(2, [[]])
        self.validate_cast('Opt', self.ids('Island', zone="battlefield", controller=0)[:1], accepted=True, spend={'U': 1})
        self.pass_round()
        self.assertEqual(len(self.hand(0)), before + 1)
        self.assertEqual(self.engine.state["objects"][str(source)]["counters"]["loyalty"], 1)
        self.assertEqual(self.engine.state["asked"]["kind"], "priority")

    def test_resolved_permission_survives_bounced_source_and_multiplayer_cleanup_then_expires(self):
        self.new([{"battlefield": ["Teferi, Time Raveler", "Island", "Forest", "Forest"],
                   "hand": ["Divination", "Grizzly Bears"]},
                  {"battlefield": ["Island", "Forest"], "hand": ["Into the Roil"]}, {}])
        source = self.activate(1)
        self.pass_round()
        self.assertEqual(self.engine.state["asked"]["kind"], "priority", "permission installs without a May question")
        self.next_main(1)
        self.cast("Into the Roil", [[{"object": source}]], mana=self.ids("Island", zone="battlefield", controller=1)[:1] + self.ids("Forest", zone="battlefield", controller=1)[:1], spend={'U': 1, 'G': 1})
        self.pass_round()
        self.assertTrue(any(self.view(0)["objects"][str(i)]["name"] == "Teferi, Time Raveler" for i in self.hand(0)))
        self.engine.act({"kind": "pass"})
        self.engine.act({"kind": "pass"})
        self.validate_cast('Divination', self.ids('Island', zone="battlefield", controller=0)[:1] + self.ids('Forest', zone="battlefield", controller=0)[:2], accepted=True, spend={'U': 1, 'G': 2})
        self.validate_cast('Grizzly Bears', self.ids('Island', zone="battlefield", controller=0)[:1] + self.ids('Forest', zone="battlefield", controller=0)[:1], accepted=False, spend={'U': 1, 'G': 1})
        self.next_main(2)
        self.engine.act({"kind": "pass"})
        self.validate_cast('Divination', self.ids('Island', zone="battlefield", controller=0)[:1] + self.ids('Forest', zone="battlefield", controller=0)[:2], accepted=True, spend={'U': 1, 'G': 2})
        for _ in range(100):
            if self.engine.state["active"] == 0:
                break
            self.engine.act({"kind": "pass"})
        self.assertEqual(self.engine.state["active"], 0)
        self.assertEqual(self.engine.state["part"], "Upkeep")
        self.validate_cast('Divination', self.ids('Island', zone="battlefield", controller=0)[:1] + self.ids('Forest', zone="battlefield", controller=0)[:2], accepted=False, spend={'U': 1, 'G': 2})
        self.reject(0, {"kind": "cast", "card": self.ids("Divination")[0], "targets": []})

    def test_restriction_overrides_controller_flash_permission(self):
        self.new([{"battlefield": ["Teferi, Time Raveler", "Island", "Forest", "Forest"],
                   "hand": ["Divination", "Ornithopter"]}, {"battlefield": ["Teferi, Time Raveler"]}, {}])
        self.activate(1)
        self.pass_round()
        self.validate_cast('Divination', self.ids('Island', zone="battlefield", controller=0)[:1] + self.ids('Forest', zone="battlefield", controller=0)[:2], accepted=True, spend={'U': 1, 'G': 2})
        self.cast("Ornithopter")
        self.validate_cast('Divination', self.ids('Island', zone="battlefield", controller=0)[:1] + self.ids('Forest', zone="battlefield", controller=0)[:2], accepted=False, spend={'U': 1, 'G': 2})
        self.reject(0, {"kind": "cast", "card": self.ids("Divination")[0], "targets": []})
        self.pass_round()
        self.validate_cast('Divination', self.ids('Island', zone="battlefield", controller=0)[:1] + self.ids('Forest', zone="battlefield", controller=0)[:2], accepted=True, spend={'U': 1, 'G': 2})
        self.cast("Divination", mana=self.ids("Island", zone="battlefield", controller=0)[:1] + self.ids("Forest", zone="battlefield", controller=0)[:2], spend={'U': 1, 'G': 2})
        self.pass_round()

    def test_permission_accepts_sorcery_response_on_opponent_turn(self):
        self.new([{"battlefield": ["Teferi, Time Raveler", "Island", "Forest", "Forest"],
                   "hand": ["Divination"]}, {"hand": ["Ornithopter"]}, {}])
        self.activate(1)
        self.pass_round()
        self.next_main(1)
        self.cast("Ornithopter")
        self.engine.act({"kind": "pass"})
        self.engine.act({"kind": "pass"})
        before = len(self.hand(0))
        self.cast("Divination", mana=self.ids("Island", zone="battlefield", controller=0)[:1] + self.ids("Forest", zone="battlefield", controller=0)[:2], spend={'U': 1, 'G': 2})
        self.pass_round()
        self.assertEqual(len(self.hand(0)), before + 1, "one cast card, two drawn")
        self.assertEqual(len(self.engine.state["stack"]), 1)

    def test_minus_three_returns_artifact_to_owner_and_draws(self):
        self.new([{"battlefield": ["Teferi, Time Raveler"]}, {"battlefield": ["Ornithopter"]}, {}])
        target = self.ids("Ornithopter")[0]
        before = len(self.hand(0))
        self.activate(2, [[{"object": target}]])
        self.pass_round()
        self.assertEqual(len(self.hand(0)), before + 1)
        self.assertTrue(any(self.view(1)["objects"][str(i)]["name"] == "Ornithopter" for i in self.hand(1)))

    def test_minus_three_illegal_chosen_target_does_not_draw(self):
        self.new([{"battlefield": ["Teferi, Time Raveler", "Island"], "hand": ["Unsummon"]},
                  {"battlefield": ["Ornithopter"]}, {}])
        target = self.ids("Ornithopter")[0]
        self.activate(2, [[{"object": target}]])
        self.cast("Unsummon", [[{"object": target}]], mana=self.ids("Island", zone="battlefield", controller=0)[:1], spend={'U': 1})
        before = len(self.hand(0))
        self.pass_round()
        self.pass_round()
        self.assertEqual(len(self.hand(0)), before)

    def test_emet_resolution_offer_has_no_legal_cast_and_rejection_preserves_choice(self):
        self.new([{"battlefield": ["Emet-Selch of the Third Seat", "Swamp", "Swamp", "Swamp", "Island"],
                   "hand": ["Exsanguinate"], "graveyard": ["Divination"]},
                  {"battlefield": ["Teferi, Time Raveler"]}, {}])
        self.cast("Exsanguinate", x=1, mana=self.ids("Swamp", zone="battlefield", controller=0)[:3], spend={'B': 3})
        self.pass_round()
        card = next(i for i in self.ids("Divination") if i in self.engine.state["players"][0]["graveyard"])
        self.engine.act({"kind": "targets", "targets": [[{"object": card}]]})
        self.pass_round()
        self.assertEqual(self.engine.state["asked"]["kind"], "cast")
        self.validate_cast('Divination', [], accepted=False, spend={})
        before = self.engine.state["request_id"]
        self.reject(0, {"kind": "cast", "card": card, "targets": []})
        self.assertEqual(self.engine.state["request_id"], before)
        self.assertEqual(self.engine.state["asked"]["kind"], "cast")
        self.choose("no")
        self.assertIn(card, self.engine.state["players"][0]["graveyard"])
        self.assertEqual(self.engine.state["asked"]["kind"], "priority")


del Helpers
