"""Buyback through the real audited client with
full pinned cards. "Buyback [cost]" is an optional additional cost (601.2b, 601.2f-h), and "if
the buyback cost was paid, put this spell into its owner's hand instead of into that player's
graveyard as it resolves" (702.27a). A countered spell, or one whose targets are all illegal,
doesn't resolve (608.2b), so it goes to the graveyard. A copy keeps the paid decision (707.10,
Reiterate ruling 7005f0d6) but ceases to exist outside the stack (704.5e). With Cast Through Time,
a paid spell cast from the hand has both, and its controller chooses (ruling d79a673e); Rebound
counts only if it is there as the spell finishes resolving (ruling ce120f88), and never on a copy
(fb734c65). A graveyard-only "exile it instead" rider likewise competes with buyback (616.1)."""
import unittest
import test_toxic_review_client as base
from client_support import ClientTestCase

HELPERS = {k: v for k, v in vars(base.ToxicReviewClientTests).items() if not k.startswith("test_") and not k.startswith("__")}


def effects(asked: dict) -> list[str]:
    """The effects a spell-destination question offers, by name: each entry also names the
    object whose effect it is (its `source` and ability row), or the rule that makes it."""
    return [entry["effect"] if isinstance(entry, dict) else entry for entry in asked["among"]]

class BuybackReviewClientTests(ClientTestCase):
    locals().update(HELPERS)

    # --- reading the state

    def state(self):
        return self.engine.state

    def where(self, name):
        """Each object of that name, by zone, seen by any player: a card that changes zones is a
        new object (400.7), and only its owner sees it in a hand."""
        found = set()
        for viewer in range(len(self.state()["players"])):
            v = self.view(viewer)
            for key, o in v["objects"].items():
                if o["name"] != name:
                    continue
                card = int(key)
                zones = [("stack", v["stack"]), ("battlefield", v["battlefield"]), ("exile", v["exile"])]
                zones += [(f"graveyard{i}", p["graveyard"]) for i, p in enumerate(v["players"])]
                zones += [(f"hand{i}", p["hand"]) for i, p in enumerate(v["players"]) if "hand" in p]
                found |= {(card, z) for z, ids in zones if card in ids}
        return sorted(z for _, z in found)

    def hand_names(self, player):
        v = self.view(player)
        return sorted(v["objects"][str(o)]["name"] for o in v["players"][player]["hand"])

    def untapped(self, player):
        return [o for o in self.state()["battlefield"] if self.obj(o)["controller"] == player and not self.obj(o).get("tapped")]

    def destination_asked(self):
        a = self.asked()
        return a if a and a["kind"] == "replacement" and a.get("event") == "spell_destination" else None

    # --- acting

    def cast(self, name, targets=None, *, mana, spend, **announced):
        actor = self.asked()['player']
        cards = self.asked().get('cards', []) + self.view(actor)['players'][actor]['hand']
        card = next(o for o in cards if self.obj(o)['name'] == name)
        cmd = {'kind': 'cast', 'card': card, 'targets': targets or [],
               'plan': [{'source': o, 'ability': 0} for o in mana], 'mana': spend, **announced}
        before = list(self.state()['stack'])
        self.engine.act(cmd)
        spell = [o for o in self.state()['stack'] if o not in before]
        self.assertEqual(len(spell), 1)
        return spell[0]

    def pass_until_asked(self):
        """Pass until something other than priority is asked, or the stack is empty."""
        for _ in range(40):
            a = self.asked()
            if a["kind"] != "priority" or not self.state()["stack"]:
                return a
            self.engine.act({"kind": "pass"})
        self.fail("the stack did not settle")

    def choose(self, effect):
        a = self.destination_asked()
        self.engine.act({"kind": "choose", "answer": {"spell_destination": {"on": a["on"]["object"], "effect": effect}}})

    def destinations_seen(self):
        return [e for e in self.seen if "spell_destination" in str(e)]

    # --- announcements and payment

    def test_paid_and_unpaid_total_costs_and_x_boundaries_are_executed(self):
        # 601.2f-h, 702.27a: these totals follow the printed cost plus buyback.
        cases = [('Capsize', False, None, 3), ('Capsize', True, None, 6),
                 ('Whispers of the Muse', False, None, 1), ('Whispers of the Muse', True, None, 6),
                 ('Unsummon', None, None, 1)]
        cases += [('Fanning the Flames', False, x, x + 2) for x in range(5)]
        cases += [('Fanning the Flames', True, x, x + 5) for x in range(2)]
        for name, paid, x, count in cases:
            with self.subTest(name=name, buyback=paid, x=x):
                self.new([{'hand': [name], 'battlefield': ['Island'] * 3 + ['Mountain'] * 3},
                          {'battlefield': ['Grizzly Bears']}])
                self.main()
                sources = ((self.ids('Mountain') + self.ids('Island')) if x is not None
                           else (self.ids('Island') + self.ids('Mountain')))
                targets = [[{'player': 1}]] if x is not None else ([] if name == 'Whispers of the Muse' else [[{'object': self.one('Grizzly Bears')}]])
                fields = {} if paid is None else {'buyback': paid}
                if x is not None:
                    fields['x'] = x
                self.cast(name, targets, mana=sources[:count], **fields, spend=({'R': min(count, 3), 'U': max(count - 3, 0)} if x is not None else {'U': min(count, 3), 'R': max(count - 3, 0)}))
                self.assertEqual(len(self.untapped(0)), 6 - count)
        for paid, x in ((False, 5), (True, 2)):
            self.new([{'hand': ['Fanning the Flames'], 'battlefield': ['Mountain'] * 6}, {}])
            self.main()
            self.reject(0, {'kind': 'cast', 'card': self.ids('Fanning the Flames')[0], 'buyback': paid,
                            'x': x, 'mana': {'R': 6}, 'targets': [[{'player': 1}]],
                            'plan': [{'source': o, 'ability': 0} for o in self.ids('Mountain')]})

    def test_paid_cost_one_mana_short_is_refused_atomically(self):
        self.new([{"hand": ["Capsize", "Whispers of the Muse", "Unsummon"], "battlefield": ["Island"] * 5},
                  {"battlefield": ["Grizzly Bears"]}])
        self.main()
        bears = self.one("Grizzly Bears")
        unpaid = {"kind": "cast", "card": self.ids("Capsize")[0], "buyback": False, "mana": {"U": 3},
                  "plan": [{"source": o, "ability": 0} for o in self.ids("Island")[:3]]}
        unpaid["targets"] = [[{"object": bears}]]
        before = (self.untapped(0), self.hand_names(0), self.state()["stack"])
        for wrong in [True, "true", 1, None, []]:
            with self.subTest(buyback=wrong):
                self.reject(0, {**unpaid, "buyback": wrong, "mana": {"U": 5},
                                "plan": [{"source": o, "ability": 0} for o in self.ids("Island")]})
        self.reject(0, {'kind': 'cast', 'card': self.ids('Whispers of the Muse')[0], 'buyback': True, 'mana': {'U': 5},
                        'plan': [{'source': o, 'ability': 0} for o in self.ids('Island')], 'targets': []})
        unsummon = {"kind": "cast", "card": self.ids("Unsummon")[0], "mana": {"U": 1},
                    "plan": [{"source": self.ids("Island")[0], "ability": 0}]}
        unsummon["targets"] = [[{"object": bears}]]
        for cmd in [{**unsummon, "buyback": False}, {**unsummon, "buyback": True}, {"kind": "pass", "buyback": False}]:
            with self.subTest(cmd=cmd):
                self.reject(0, cmd)
        island = self.untapped(0)[0]
        self.reject(0, {"kind": "activate", "source": island, "ability": 0, "targets": [], "plan": [], "paying": [], "buyback": False})
        self.assertEqual((self.untapped(0), self.hand_names(0), self.state()["stack"]), before)
        # omitted means unpaid
        del unpaid["buyback"]
        self.engine.act(unpaid)
        self.pass_until_asked()
        self.assertEqual(self.where("Capsize"), ["graveyard0"])
        self.assertEqual(len(self.untapped(0)), 2)

    def test_paid_capsize_returns_both_with_no_question_and_unpaid_goes_to_the_graveyard(self):
        for paid in (True, False):
            with self.subTest(paid=paid):
                self.new([{"hand": ["Capsize"], "battlefield": ["Island"] * 6}, {"battlefield": ["Grizzly Bears"]}])
                self.main()
                bears = self.one("Grizzly Bears")
                self.cast("Capsize", [[{"object": bears}]], buyback=paid, mana=self.ids("Island", controller=0)[:6 if paid else 3], spend={'U': 6 if paid else 3})
                self.assertEqual(len(self.untapped(0)), 0 if paid else 3, "the total cost was paid")
                self.engine.act({"kind": "pass"})
                self.engine.act({"kind": "pass"})
                self.assertIsNone(self.destination_asked(), "only one replacement applies: nothing to choose")
                self.assertEqual(self.state()["stack"], [])
                self.assertEqual(self.where("Grizzly Bears"), ["hand1"])
                self.assertEqual(self.where("Capsize"), ["hand0"] if paid else ["graveyard0"])
                self.assertEqual(self.hand_names(0), ["Capsize"] if paid else [])

    def test_convoke_pays_part_of_the_buyback_total(self):
        """Sprout Swarm {1}{G} + {3} from four Forests needs a creature tapped for convoke (702.51a)."""
        self.new([{"hand": ["Sprout Swarm"], "battlefield": ["Forest"] * 4 + ["Grizzly Bears"]}, {}])
        self.main()
        self.reject(0, {'kind': 'cast', 'card': self.ids('Sprout Swarm')[0], 'buyback': True,
                        'targets': [], 'mana': {'G': 4}, 'plan': [{'source': o, 'ability': 0} for o in self.ids('Forest')]})
        self.cast("Sprout Swarm", buyback=True, mana=self.ids('Forest'),
                  convoke=['generic'], paying=[[self.one('Grizzly Bears')]], spend={'G': 4})
        self.assertEqual(self.untapped(0), [])
        self.pass_until_asked()
        self.assertEqual((self.where("Sprout Swarm"), len(self.ids("Saproling Token"))), (["hand0"], 1))

    def test_paid_x_spells_return_and_keep_their_x(self):
        self.new([{"hand": ["Fanning the Flames"], "battlefield": ["Mountain"] * 6}, {}])
        self.main()
        self.cast("Fanning the Flames", [[{"player": 1}]], buyback=True, x=1, mana=self.ids("Mountain")[:6], spend={'R': 6})
        self.pass_until_asked()
        self.assertEqual((self.life(1), self.where("Fanning the Flames")), (19, ["hand0"]))

        self.new([{"hand": ["Lightning Bolt"], "battlefield": ["Mountain"]},
                  {"hand": ["Spell Burst"], "battlefield": ["Island"] * 5}])
        self.main()
        bolt = self.cast("Lightning Bolt", [[{"player": 1}]], mana=self.ids("Mountain")[:1], spend={'R': 1})
        self.engine.act({"kind": "pass"})
        for x in (0, 2):
            self.reject(1, {'kind': 'cast', 'card': self.ids('Spell Burst')[0], 'buyback': True,
                            'x': x, 'mana': {'R': 6}, 'targets': [[{'object': bolt}]],
                            'plan': [{'source': o, 'ability': 0} for o in self.ids('Island', controller=1)]})
        self.cast("Spell Burst", [[{"object": bolt}]], buyback=True, x=1, mana=self.ids("Island", controller=1), spend={'U': 5})
        self.pass_until_asked()
        self.assertEqual((self.life(1), self.where("Spell Burst"), self.where("Lightning Bolt")), (20, ["hand1"], ["graveyard0"]))

    def test_paid_whispers_draws_once_and_returns_once(self):
        self.new([{"hand": ["Whispers of the Muse"], "battlefield": ["Island"] * 6, "library": ["Plains"] * 8}, {}])
        self.main()
        self.cast("Whispers of the Muse", buyback=True, mana=self.ids("Island", controller=0)[:6], spend={'U': 6})
        self.pass_until_asked()
        self.assertEqual(self.hand_names(0), ["Plains", "Whispers of the Muse"])
        self.assertEqual(self.state()["players"][0]["library"], 7)

    def test_capsize_returns_a_stolen_permanent_to_its_owner(self):
        self.new([{"hand": ["Act of Treason", "Capsize"], "battlefield": ["Mountain"] * 3 + ["Island"] * 6},
                  {"battlefield": ["Hill Giant"]}])
        self.main()
        giant = self.one("Hill Giant")
        self.cast("Act of Treason", [[{"object": giant}]], mana=self.ids("Mountain")[:3], spend={'R': 3})
        self.pass_until_asked()
        self.assertEqual(self.obj(giant)["controller"], 0)
        self.cast("Capsize", [[{"object": giant}]], buyback=True, mana=self.ids("Island", controller=0)[:6], spend={'U': 6})
        self.pass_until_asked()
        self.assertEqual((self.where("Hill Giant"), self.where("Capsize")), (["hand1"], ["hand0"]))

    def test_countered_or_wholly_illegal_paid_capsize_goes_to_the_graveyard(self):
        for answer in ("Counterspell", "Unsummon"):
            with self.subTest(answer=answer):
                self.new([{"hand": ["Capsize"], "battlefield": ["Island"] * 6},
                          {"hand": [answer], "battlefield": ["Island"] * 2 + ["Grizzly Bears"]}])
                self.main()
                bears = self.one("Grizzly Bears")
                capsize = self.cast("Capsize", [[{"object": bears}]], buyback=True, mana=self.ids("Island", controller=0)[:6], spend={'U': 6})
                self.engine.act({"kind": "pass"})
                self.cast(answer, [[{"object": capsize}]] if answer == "Counterspell" else [[{"object": bears}]], mana=self.ids("Island", controller=1)[:2 if answer == "Counterspell" else 1], spend={'U': 2 if answer == 'Counterspell' else 1})
                self.pass_until_asked()
                self.assertIsNone(self.destination_asked())
                self.assertEqual(self.where("Capsize"), ["graveyard0"], "no refund and no return")
                self.assertEqual(self.where("Grizzly Bears"), ["battlefield"] if answer == "Counterspell" else ["hand1"])

    # --- copies

    def test_twincast_copy_keeps_the_paid_decision_and_ceases_to_exist(self):
        for paid in (True, False):
            with self.subTest(paid=paid):
                self.new([{"hand": ["Capsize"], "battlefield": ["Island"] * 6 + ["Hill Giant"]},
                          {"hand": ["Twincast"], "battlefield": ["Island"] * 2 + ["Grizzly Bears"]}])
                self.main()
                bears, giant = self.one("Grizzly Bears"), self.one("Hill Giant")
                capsize = self.cast("Capsize", [[{"object": bears}]], buyback=paid, mana=self.ids("Island", controller=0)[:6 if paid else 3], spend={'U': 6 if paid else 3})
                self.engine.act({"kind": "pass"})
                self.cast("Twincast", [[{"object": capsize}]], mana=self.ids("Island", controller=1)[:2], spend={'U': 2})
                self.engine.act({"kind": "pass"})
                self.engine.act({"kind": "pass"})
                retarget = self.asked()
                self.assertEqual((retarget["kind"], retarget["player"]), ("retarget", 1))
                self.engine.act({"kind": "choose", "answer": {"targets": [[{"object": giant}]]}})
                self.assertEqual(len(self.state()["stack"]), 2, "the copy above Capsize")
                self.pass_until_asked()
                self.assertIsNone(self.destination_asked())
                self.assertEqual(self.where("Hill Giant"), ["hand0"])
                self.assertEqual(self.where("Grizzly Bears"), ["hand1"])
                # the copy is in no zone, in nobody's hand, never a card (704.5e, 707.10a)
                self.assertEqual(self.where("Capsize"), ["hand0"] if paid else ["graveyard0"])
                self.assertEqual(self.hand_names(1), ["Grizzly Bears"])
                self.assertEqual(self.where("Twincast"), ["graveyard1"])

    def test_reiterate_paid_returns_and_its_copy_of_a_paid_capsize_keeps_buyback(self):
        self.new([{"hand": ["Lightning Bolt", "Reiterate"], "battlefield": ["Mountain"] * 7}, {}])
        self.main()
        bolt = self.cast("Lightning Bolt", [[{"player": 1}]], mana=self.ids("Mountain")[:1], spend={'R': 1})
        self.cast("Reiterate", [[{"object": bolt}]], buyback=True, mana=self.ids("Mountain")[1:7], spend={'R': (7) - (1)})
        self.engine.act({"kind": "pass"})
        self.engine.act({"kind": "pass"})
        self.assertEqual(self.asked()["kind"], "retarget")
        self.engine.act({"kind": "choose", "answer": "no"})
        self.pass_until_asked()
        self.assertEqual((self.life(1), self.where("Reiterate"), self.where("Lightning Bolt")), (14, ["hand0"], ["graveyard0"]))

        self.new([{"hand": ["Capsize", "Reiterate"], "battlefield": ["Island"] * 6 + ["Mountain"] * 3 + ["Hill Giant"]},
                  {"battlefield": ["Grizzly Bears"]}])
        self.main()
        bears, giant = self.one("Grizzly Bears"), self.one("Hill Giant")
        capsize = self.cast("Capsize", [[{"object": bears}]], buyback=True, mana=self.ids("Island", controller=0)[:6], spend={'U': 6})
        self.cast("Reiterate", [[{"object": capsize}]], buyback=False, mana=self.ids("Mountain")[:3], spend={'R': 3})
        self.engine.act({"kind": "pass"})
        self.engine.act({"kind": "pass"})
        self.engine.act({"kind": "choose", "answer": {"targets": [[{"object": giant}]]}})
        self.pass_until_asked()
        self.assertEqual((self.where("Hill Giant"), self.where("Grizzly Bears")), (["hand0"], ["hand1"]))
        self.assertEqual((self.where("Capsize"), self.where("Reiterate")), (["hand0"], ["graveyard0"]))
        self.assertEqual(self.hand_names(0), ["Capsize", "Hill Giant"], "no card for the copy")

    # --- buyback and rebound (Cast Through Time)

    def test_paid_whispers_with_cast_through_time_asks_its_controller(self):
        for effect in ("buyback", "rebound"):
            with self.subTest(effect=effect):
                self.new([{"hand": ["Whispers of the Muse"], "battlefield": ["Island"] * 6 + ["Cast Through Time"]}, {}])
                self.main()
                whispers = self.cast("Whispers of the Muse", buyback=True, mana=self.ids("Island", controller=0)[:6], spend={'U': 6})
                self.engine.act({"kind": "pass"})
                self.engine.act({"kind": "pass"})
                a = self.destination_asked()
                self.assertEqual((a["player"], effects(a), a["optional"]), (0, ["buyback", "rebound"], False), a)
                spell = a["on"]["object"]
                self.assertEqual(self.state()["stack"], [spell], "decided while it is still resolving")
                self.assertEqual(self.state()["players"][0]["library"], 7, "the body already drew")
                for player, answer in [(1, {"spell_destination": {"on": spell, "effect": effect}}),
                                       (0, {"spell_destination": {"on": spell, "effect": "cast_rider"}}),
                                       (0, {"spell_destination": {"on": spell + 1000, "effect": effect}}),
                                       (0, {"spell_destination": {"on": spell, "effect": "flashback"}}),
                                       (0, "no"), (0, "yes")]:
                    with self.subTest(player=player, answer=answer):
                        self.reject(player, {"kind": "choose", "answer": answer})
                self.reject(0, {"kind": "pass"})
                self.assertEqual(self.state()["players"][0]["library"], 7, "no second draw")
                self.choose(effect)
                self.assertEqual(self.state()["stack"], [])
                self.assertEqual(self.where("Whispers of the Muse"), ["hand0"] if effect == "buyback" else ["exile"])
                self.assertEqual(self.state()["players"][0]["library"], 7)
                self.assertEqual(self.rebound_offered(), effect == "rebound")

    def rebound_offered(self):
        """At P0's next upkeep, is a cast of Whispers offered (702.88a)?"""
        for _ in range(60):
            s = self.state()
            if s["turn"] == 3 and s["part"] not in ("Untap", "Upkeep"):
                return False
            a = s["asked"]
            if s["turn"] == 3 and a["kind"] == "cast":
                names = [self.obj(c)["name"] for c in a.get("cards", [])]
                self.assertEqual(names, ["Whispers of the Muse"], a)
                return True
            if s["turn"] == 3 and a["kind"] not in ("priority",):
                self.fail(f"unexpected question {a}")
            if a["kind"] == "discard":
                self.engine.act({"kind": "discard", "cards": a["among"][:a["count"]]})
                continue
            self.engine.act({"kind": "pass"})
        self.fail("did not reach the next upkeep")

    def test_the_free_rebound_cast_may_pay_buyback_and_then_returns_to_hand(self):
        """Rebound's cast is without paying the mana cost, but optional additional costs may be
        paid (ruling dc98f98b); cast from exile, rebound no longer applies (0afe3e20)."""
        self.new([{"hand": ["Whispers of the Muse"], "battlefield": ["Island"] * 6 + ["Cast Through Time"]}, {}])
        self.main()
        self.cast("Whispers of the Muse", buyback=False, mana=self.ids("Island", controller=0)[:1], spend={'U': 1})
        self.pass_until_asked()
        self.assertEqual(self.where("Whispers of the Muse"), ["exile"])
        self.assertTrue(self.rebound_offered())
        self.cast('Whispers of the Muse', buyback=True, mana=self.ids('Island', controller=0)[:5], spend={'U': 5})
        self.assertEqual(len([o for o in self.untapped(0) if self.obj(o)["name"] == "Island"]), 1)
        self.pass_until_asked()
        self.assertIsNone(self.destination_asked())
        self.assertEqual(self.where("Whispers of the Muse"), ["hand0"])

    def test_unpaid_whispers_with_cast_through_time_rebounds_without_a_question(self):
        self.new([{"hand": ["Whispers of the Muse"], "battlefield": ["Island"] * 6 + ["Cast Through Time"]}, {}])
        self.main()
        self.cast("Whispers of the Muse", buyback=False, mana=self.ids("Island", controller=0)[:1], spend={'U': 1})
        self.pass_until_asked()
        self.assertIsNone(self.destination_asked())
        self.assertEqual(self.where("Whispers of the Muse"), ["exile"])
        self.assertTrue(self.rebound_offered())

    def test_cast_through_time_flashed_in_after_the_announcement_still_competes(self):
        self.new([{"hand": ["Whispers of the Muse", "Cast Through Time"],
                   "battlefield": ["Island"] * 13 + ["Leyline of Anticipation"]}, {}])
        self.main()
        self.cast("Whispers of the Muse", buyback=True, mana=self.ids("Island", controller=0)[:6], spend={'U': 6})
        self.cast("Cast Through Time", mana=self.ids("Island", controller=0)[6:13], spend={'U': (13) - (6)})
        self.engine.act({"kind": "pass"})
        self.engine.act({"kind": "pass"})
        self.assertEqual(self.where("Cast Through Time"), ["battlefield"])
        self.engine.act({"kind": "pass"})
        self.engine.act({"kind": "pass"})
        self.assertEqual(effects(self.destination_asked()), ["buyback", "rebound"])
        self.choose("rebound")
        self.assertEqual(self.where("Whispers of the Muse"), ["exile"])
        self.assertTrue(self.rebound_offered())

    def test_capsize_returning_cast_through_time_leaves_only_buyback(self):
        self.new([{"hand": ["Capsize"], "battlefield": ["Island"] * 6 + ["Cast Through Time"]}, {}])
        self.main()
        ctt = self.one("Cast Through Time")
        self.cast("Capsize", [[{"object": ctt}]], buyback=True, mana=self.ids("Island", controller=0)[:6], spend={'U': 6})
        self.engine.act({"kind": "pass"})
        self.engine.act({"kind": "pass"})
        self.assertIsNone(self.destination_asked(), "rebound is gone as it finishes resolving (ce120f88)")
        self.assertEqual((self.where("Capsize"), self.where("Cast Through Time")), (["hand0"], ["hand0"]))
        self.assertEqual(self.state()["exile"], [])

    def test_cast_through_time_removed_in_response_leaves_only_buyback(self):
        self.new([{"hand": ["Whispers of the Muse"], "battlefield": ["Island"] * 6 + ["Cast Through Time"]},
                  {"hand": ["Capsize"], "battlefield": ["Island"] * 3}])
        self.main()
        ctt = self.one("Cast Through Time")
        self.cast("Whispers of the Muse", buyback=True, mana=self.ids("Island", controller=0)[:6], spend={'U': 6})
        self.engine.act({"kind": "pass"})
        self.cast("Capsize", [[{"object": ctt}]], buyback=False, mana=self.ids("Island", controller=1)[:3], spend={'U': 3})
        self.pass_until_asked()
        self.assertIsNone(self.destination_asked())
        self.assertEqual(self.where("Whispers of the Muse"), ["hand0"])
        self.assertEqual(self.where("Capsize"), ["graveyard1"], "P1's Capsize has no rebound: P1 controls no Cast Through Time")

    def test_a_copy_of_a_paid_whispers_never_rebounds_and_ceases(self):
        self.new([{"hand": ["Whispers of the Muse", "Twincast"], "battlefield": ["Island"] * 8 + ["Cast Through Time"]}, {}])
        self.main()
        whispers = self.cast("Whispers of the Muse", buyback=True, mana=self.ids("Island", controller=0)[:6], spend={'U': 6})
        self.cast("Twincast", [[{"object": whispers}]], mana=self.ids("Island", controller=0)[6:8], spend={'U': (8) - (6)})
        self.engine.act({"kind": "pass"})
        self.engine.act({"kind": "pass"})
        # Twincast itself is cast from the hand with rebound: one replacement, no question
        self.assertIsNone(self.destination_asked())
        self.assertEqual(self.where("Twincast"), ["exile"])
        self.engine.act({"kind": "pass"})
        self.engine.act({"kind": "pass"})
        self.assertIsNone(self.destination_asked(), "the copy: buyback only (fb734c65)")
        self.assertEqual(self.state()["players"][0]["library"], 7)
        self.engine.act({"kind": "pass"})
        self.engine.act({"kind": "pass"})
        self.assertEqual(effects(self.destination_asked()), ["buyback", "rebound"])
        self.choose("buyback")
        self.assertEqual(self.state()["players"][0]["library"], 6)
        self.assertEqual(self.hand_names(0).count("Whispers of the Muse"), 1, "the copy made no card")

    # --- buyback and a graveyard-only cast rider

    def test_emet_selch_reduces_the_total_including_buyback(self):
        self.new([{"hand": ["Shock"], "graveyard": ["Capsize"],
                   "battlefield": ["Mountain", "Island", "Island", "Island", "Island", "Emet-Selch of the Third Seat"]},
                  {"battlefield": ["Grizzly Bears"]}])
        self.main()
        bears = self.one("Grizzly Bears")
        self.cast("Shock", [[{"player": 1}]], mana=self.ids("Mountain")[:1], spend={'R': 1})
        a = self.pass_until_asked()
        if a["kind"] == "targets":
            self.engine.act({"kind": "targets", "targets": [[{"object": self.ids("Capsize")[0]}]]})
            a = self.pass_until_asked()
        self.assertEqual(a["kind"], "cast", a)
        self.cast('Capsize', [[{'object': bears}]], buyback=True, mana=self.ids('Island', controller=0)[:4], spend={'U': 4})
        self.assertEqual(len(self.untapped(0)), 1, "four Islands paid; Emet-Selch untapped")
        self.engine.act({"kind": "pass"})
        self.engine.act({"kind": "pass"})
        self.assertEqual(effects(self.destination_asked()), ["buyback", "cast_rider"])
        self.choose("buyback")
        self.assertEqual(self.where("Capsize"), ["hand0"])

    def bilbo(self, paid):
        """Bilbo attacks; its trigger lets P0 cast Capsize from the graveyard for {1} less (the
        total, buyback included), with the exile rider."""
        self.new([{"graveyard": ["Capsize"], "battlefield": ["Island"] * 5 + ["Bilbo, Thief in the Night"]},
                  {"battlefield": ["Grizzly Bears"]}])
        self.main()
        bears, bilbo = self.one("Grizzly Bears"), self.one("Bilbo, Thief in the Night")
        self.attack(bilbo, {"player": 1})
        a = self.pass_until_asked()
        self.assertEqual(a["kind"], "cast", a)
        self.cast('Capsize', [[{'object': bears}]], buyback=paid, mana=self.ids('Island', controller=0)[:5 if paid else 2], spend={'U': 5 if paid else 2})
        self.assertEqual(len([o for o in self.untapped(0) if self.obj(o)["name"] == "Island"]), 0 if paid else 3)
        self.pass_until_asked()

    def test_bilbo_rider_competes_with_paid_buyback(self):
        for effect, zone in (("buyback", ["hand0"]), ("cast_rider", ["exile"])):
            with self.subTest(effect=effect):
                self.bilbo(True)
                a = self.destination_asked()
                self.assertEqual((a["player"], effects(a)), (0, ["buyback", "cast_rider"]), a)
                self.choose(effect)
                self.assertEqual(self.where("Capsize"), zone)
                self.assertEqual(self.where("Grizzly Bears"), ["hand1"])

    def test_bilbo_rider_alone_exiles_unpaid_capsize_without_a_question(self):
        self.bilbo(False)
        self.assertIsNone(self.destination_asked())
        self.assertEqual(self.where("Capsize"), ["exile"])

    def test_countered_paid_capsize_cast_by_bilbo_is_exiled_by_its_rider(self):
        """Not resolving, buyback never applies (608.2n); the rider still replaces the graveyard."""
        self.new([{"graveyard": ["Capsize"], "battlefield": ["Island"] * 5 + ["Bilbo, Thief in the Night"]},
                  {"hand": ["Counterspell"], "battlefield": ["Island"] * 2 + ["Grizzly Bears"]}])
        self.main()
        bears, bilbo = self.one("Grizzly Bears"), self.one("Bilbo, Thief in the Night")
        self.attack(bilbo, {"player": 1})
        a = self.pass_until_asked()
        self.cast('Capsize', [[{'object': bears}]], buyback=True, mana=self.ids('Island', controller=0)[:5], spend={'U': 5})
        capsize = self.state()["stack"][-1]
        self.engine.act({"kind": "pass"})
        self.cast("Counterspell", [[{"object": capsize}]], mana=self.ids("Island", controller=1)[:2], spend={'U': 2})
        self.pass_until_asked()
        self.assertIsNone(self.destination_asked())
        self.assertEqual((self.where("Capsize"), self.where("Grizzly Bears")), (["exile"], ["battlefield"]))


if __name__ == "__main__":
    unittest.main()
