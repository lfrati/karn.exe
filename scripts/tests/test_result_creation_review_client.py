"""Full pinned Terastodon and Mogg Infestation through
from client_support import ClientTestCase
the real audited client. Tokens go to each destroyed permanent's former controller (608.2h),
only for actual battlefield-to-graveyard results (Terastodon ruling 6aeb7aa39fd324bf)."""


class ResultCreationReviewClientTests(ClientTestCase):
    def tokens(self):
        """Token names per controller, as both players see them."""
        seen = []
        for viewer in (0, 1):
            v = self.view(viewer)
            seen.append(sorted((v["objects"][str(o)]["controller"], v["objects"][str(o)]["name"])
                               for o in v["battlefield"] if v["objects"][str(o)].get("token")))
        self.assertEqual(seen[0], seen[1])
        return seen[0]

    def names(self, zone, player):
        whole = self.engine.send({"cmd": "state"})
        return sorted(whole["objects"][str(o)]["name"] for o in whole["players"][player][zone])

    def terastodon(self, consent):
        self.new([{"hand": ["Terastodon"], "battlefield": ["Forest"] * 8 + ["Spellbook"]},
                  {"battlefield": ["Spellbook", "Darksteel Citadel"]}])
        mine, theirs = sorted(self.ids("Spellbook"), key=lambda o: self.engine.state["objects"][str(o)]["controller"])
        citadel, = self.ids("Darksteel Citadel")
        self.cast("Terastodon", mana=self.ids('Forest', zone="battlefield", controller=self.engine.state["asked"]["player"])[0:8], spend={'G': 8})
        self.pass_round()
        asked = self.engine.state["asked"]
        self.assertEqual((asked["kind"], asked["player"]), ("targets", 0))
        self.engine.act({"kind": "targets", "targets": [[{"object": mine}, {"object": theirs}, {"object": citadel}]]})
        self.pass_round()
        asked = self.engine.state["asked"]
        self.assertEqual(asked["player"], 0, asked)
        self.engine.act({"kind": "choose", "answer": "yes" if consent else "no"})
        return mine, theirs, citadel

    def test_terastodon_gives_each_former_controller_one_elephant_per_actual_destruction(self):
        mine, theirs, citadel = self.terastodon(True)
        self.assertEqual(self.tokens(), [(0, "Elephant Token"), (1, "Elephant Token")])
        self.assertIn(citadel, self.engine.state["battlefield"])
        self.assertEqual(self.names("graveyard", 0), ["Spellbook"])
        self.assertEqual(self.names("graveyard", 1), ["Spellbook"])

    def test_declined_terastodon_destroys_and_creates_nothing(self):
        mine, theirs, citadel = self.terastodon(False)
        self.assertEqual(self.tokens(), [])
        for o in (mine, theirs, citadel):
            self.assertIn(o, self.engine.state["battlefield"])

    def test_mogg_on_yourself_pays_the_former_controller_not_the_owner_of_a_stolen_creature(self):
        self.new([{"hand": ["Act of Treason", "Mogg Infestation"], "battlefield": ["Mountain"] * 8 + ["Grizzly Bears", "Darksteel Myr"]},
                  {"battlefield": ["Grizzly Bears"]}])
        stolen = next(o for o in self.ids("Grizzly Bears") if self.engine.state["objects"][str(o)]["controller"] == 1)
        myr, = self.ids("Darksteel Myr")
        self.cast("Act of Treason", [[{"object": stolen}]], mana=self.ids('Mountain', zone="battlefield", controller=self.engine.state["asked"]["player"])[0:3], spend={'R': 3})
        self.pass_round()
        self.cast("Mogg Infestation", [[{"player": 0}]], mana=self.ids('Mountain', zone="battlefield", controller=self.engine.state["asked"]["player"])[3:8], spend={'R': 5})
        self.pass_round()
        # Two creatures died under player 0's control, so 2 x 2 Goblins; the Myr survives.
        self.assertEqual(self.tokens(), [(0, "Goblin Token")] * 4)
        self.assertIn(myr, self.engine.state["battlefield"])
        self.assertEqual(self.names("graveyard", 0), ["Act of Treason", "Grizzly Bears", "Mogg Infestation"])
        self.assertEqual(self.names("graveyard", 1), ["Grizzly Bears"])

    def test_mogg_on_the_opponent_counts_only_their_actual_deaths(self):
        self.new([{"hand": ["Mogg Infestation"], "battlefield": ["Mountain"] * 5 + ["Grizzly Bears"]},
                  {"battlefield": ["Grizzly Bears", "Grizzly Bears", "Darksteel Myr"]}])
        dying=self.ids("Grizzly Bears",zone="battlefield",controller=1)
        self.cast("Mogg Infestation", [[{"player": 1}]], mana=self.ids("Mountain", zone="battlefield", controller=0)[:5], spend={'R': 5})
        self.pass_round()
        self.engine.act({"kind":"choose","answer":{"objects":dying}})
        self.assertEqual(self.tokens(), [(1, "Goblin Token")] * 4)
        self.assertEqual(self.names("graveyard", 1), ["Grizzly Bears", "Grizzly Bears"])
        self.assertEqual(len([o for o in self.ids("Grizzly Bears") if o in self.engine.state["battlefield"]]), 1)

    def test_reclamation_sage_optional_destroy_still_asks_and_honours_both_answers(self):
        # Its program changed (optional Destroy is now an attempt); the play must not.
        for consent in (True, False):
            self.new([{"hand": ["Reclamation Sage"], "battlefield": ["Forest"] * 3},
                      {"battlefield": ["Spellbook"]}])
            book, = self.ids("Spellbook")
            self.cast("Reclamation Sage", mana=self.ids('Forest', zone="battlefield", controller=self.engine.state["asked"]["player"])[0:3], spend={'G': 3})
            self.pass_round()
            self.engine.act({"kind": "targets", "targets": [[{"object": book}]]})
            self.pass_round()
            self.assertEqual(self.engine.state["asked"]["player"], 0)
            self.engine.act({"kind": "choose", "answer": "yes" if consent else "no"})
            self.assertEqual(book in self.engine.state["battlefield"], not consent)
            self.assertEqual(self.names("graveyard", 1), ["Spellbook"] if consent else [])

    def test_gulf_squid_taps_only_the_target_players_lands(self):
        self.new([{"hand": ["Gulf Squid"], "battlefield": ["Island"] * 5},
                  {"battlefield": ["Forest", "Forest"]}])
        self.cast("Gulf Squid", mana=self.ids('Island', zone="battlefield", controller=self.engine.state["asked"]["player"])[0:4], spend={'U': 4})
        self.pass_round()
        asked = self.engine.state["asked"]
        self.assertEqual((asked["kind"], asked["player"]), ("targets", 0))
        self.engine.act({"kind": "targets", "targets": [[{"player": 1}]]})
        self.pass_round()
        whole = self.engine.send({"cmd": "state"})
        theirs = [o for o in self.ids("Forest") if whole["objects"][str(o)]["controller"] == 1 and o in whole["battlefield"]]
        self.assertEqual([whole["objects"][str(o)]["tapped"] for o in theirs], [True, True])
