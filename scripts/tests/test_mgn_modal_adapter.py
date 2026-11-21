"""MGN preserves the CR 603.3c–d mode/target announcement boundaries."""
import unittest

from mgn.adapters.karn import KarnAdapter
from test_mgn_adapter import ENGINE, ORACLE, MAIN_A, PRIORITY_A, position_record, setup_of, no_random, _mana


@unittest.skipUnless(ENGINE.is_file(), "build the stamped engine")
class ModalAdapterTests(unittest.TestCase):
    def setUp(self):
        self.adapter = KarnAdapter(ENGINE, ORACLE)
        cards = {"prince": ("Charming Prince", "A"), "bears": ("Grizzly Bears", "A"),
                 "p1": ("Plains", "A"), "p2": ("Plains", "A"),
                 "top1": ("Forest", "A"), "top2": ("Island", "A")}
        record = position_record(cards, {"A.hand": ["prince"], "battlefield": ["bears", "p1", "p2"],
                                        "A.library": ["top1", "top2"]}, MAIN_A, PRIORITY_A)
        self.adapter.setup(setup_of(record))
        self.adapter.act(1, "A", {"kind": "cast", "card": "prince", "pay": _mana("p1", "p2", mana="W")}, no_random)
        self.until = {"turn": 3, "step": "precombat_main", "active": "A", "seat": "A"}
        reply = self.adapter.default(self.until, no_random)
        while reply["state"]["pending"]["kind"] == "priority":
            reply = self.adapter.default(self.until, no_random)
        self.assertEqual(reply["state"]["pending"]["kind"], "choose_modes")
        self.trigger = reply["state"]["pending"]["about"]["object"]

    def tearDown(self):
        self.adapter.end()

    def test_targeted_mode_retains_identity_and_modes_before_targets(self):
        reply = self.adapter.act(2, "A", {"kind": "choose_modes", "modes": [3]}, no_random)
        self.assertEqual(reply["state"]["pending"], {"seat": "A", "kind": "choose_targets", "about": {"object": self.trigger}})
        ability = reply["state"]["objects"][self.trigger]
        self.assertEqual(ability["modes"], [3])
        self.assertEqual(ability["targets"], [])
        reply = self.adapter.act(3, "A", {"kind": "choose_targets", "targets": [["bears"]]}, no_random)
        self.assertEqual(reply["state"]["pending"], PRIORITY_A)
        self.assertEqual(reply["state"]["objects"][self.trigger]["targets"], [["bears"]])

    def test_untargeted_mode_resolves_to_scry_without_target_question(self):
        reply = self.adapter.act(2, "A", {"kind": "choose_modes", "modes": [1]}, no_random)
        self.assertEqual(reply["state"]["pending"], PRIORITY_A)
        reply = self.adapter.default(self.until, no_random)
        while reply["state"]["pending"]["kind"] == "priority":
            reply = self.adapter.default(self.until, no_random)
        self.assertEqual(reply["state"]["pending"], {"seat": "A", "kind": "arrange", "about": {"object": self.trigger, "cards": ["top1", "top2"]}})
