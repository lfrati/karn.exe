"""Black Market/changeling coverage through the actual Python/JSON client."""
from client_support import ClientTestCase


class ChangelingClientTests(ClientTestCase):
    def market(self, seat=None, life=20):
        self.engine.ask({"cmd": "new", "life": life, "players": [
            {"library": ["Forest"] * 16, "battlefield": ["Black Market Connections"] + (seat or {}).get("battlefield", []),
             "hand": (seat or {}).get("hand", [])},
            {"library": ["Forest"] * 16}, {"library": ["Forest"] * 16}]})
        self.advance(lambda s: s["asked"]["kind"] == "modes")
        self.assertEqual(self.engine.state["asked"]["player"], 0)
        return {tuple(option["modes"]) for option in self.engine.state["asked"]["options"]}

    def choose_modes(self, modes):
        self.engine.act({"kind": "targets", "modes": modes, "targets": []})

    def tokens(self, name):
        return [o for o in self.engine.state["battlefield"] if name in self.object(o)["name"]]

    def test_every_nonempty_subset_is_offered_and_executes_its_life_draw_and_untapped_tokens(self):
        subsets = {(0,), (1,), (2,), (0, 1), (0, 2), (1, 2), (0, 1, 2)}
        for modes in sorted(subsets):
            with self.subTest(modes=modes):
                self.assertEqual(self.market(), subsets)
                before = len(self.hand(0))
                self.choose_modes(list(reversed(modes)))
                self.pass_round()
                self.assertEqual(self.engine.state["players"][0]["life"], 20 - sum(m + 1 for m in modes))
                self.assertEqual(len(self.hand(0)), before + (1 in modes))
                treasures, shapes = self.tokens("Treasure"), self.tokens("Shapeshifter")
                self.assertEqual(len(treasures), int(0 in modes))
                self.assertEqual(len(shapes), int(2 in modes))
                for token in treasures + shapes:
                    self.assertFalse(self.object(token)["tapped"])
                if shapes:
                    shape = self.object(shapes[0])
                    self.assertEqual((shape["power"], shape["toughness"]), (3, 2))
                    self.assertTrue({"Elf", "Goblin", "Time Lord", "Performer", "Shapeshifter"} <= set(shape["subtypes"]))
                    self.assertFalse({"Treasure", "Forest", "Aura"} & set(shape["subtypes"]))

    def test_rejected_answers_preserve_pending_modes_and_source_departure_keeps_selected_program(self):
        self.market({"battlefield": ["Plains", "Forest"], "hand": ["Disenchant"]})
        for player, modes in [(1, [0]), (0, []), (0, [0, 0]), (0, [3])]:
            self.reject(player, {"kind": "targets", "modes": modes, "targets": []})
        before = len(self.hand(0))
        self.choose_modes([2, 0, 1])
        source = self.ids("Black Market Connections")[0]
        self.cast("Disenchant", [[{"object": source}]], mana=self.ids('Plains', zone="battlefield", controller=self.engine.state["asked"]["player"])[0:1] + self.ids('Forest', zone="battlefield", controller=self.engine.state["asked"]["player"])[0:1], spend={'W': 1, 'G': 1})
        self.pass_round()
        self.assertNotIn(source, self.engine.state["battlefield"])
        self.pass_round()
        self.assertEqual(self.engine.state["players"][0]["life"], 14)
        self.assertEqual(len(self.hand(0)), before)  # Disenchant spent, one drawn
        self.assertEqual(len(self.tokens("Treasure")), 1)
        self.assertEqual(len(self.tokens("Shapeshifter")), 1)

    def test_creature_type_queries_and_copy_use_definition_after_frogify_override(self):
        self.market({"battlefield": ["Elvish Archdruid", "Goblin King"] + ["Island"] * 5,
                     "hand": ["Frogify", "Cackling Counterpart"]})
        self.choose_modes([2])
        self.pass_round()
        shape, = self.tokens("Shapeshifter")
        self.assertEqual((self.object(shape)["power"], self.object(shape)["toughness"]), (5, 4), "both Elf and Goblin lords apply")
        self.cast("Frogify", [[{"object": shape}]], mana=self.ids('Island', zone="battlefield", controller=self.engine.state["asked"]["player"])[0:2], spend={'U': 2})
        self.pass_round()
        self.assertEqual(self.object(shape)["subtypes"], ["Frog"])
        self.assertEqual(self.object(shape)["power"], 1)
        self.cast("Cackling Counterpart", [[{"object": shape}]], mana=self.ids('Island', zone="battlefield", controller=self.engine.state["asked"]["player"])[2:5], spend={'U': 3})
        self.pass_round()
        copy, = [o for o in self.tokens("Shapeshifter") if o != shape]
        self.assertEqual((self.object(copy)["power"], self.object(copy)["toughness"]), (5, 4))
        self.assertTrue({"Elf", "Goblin", "Time Lord"} <= set(self.object(copy)["subtypes"]))

    def test_later_ability_removal_preserves_types_without_remaining_lord_bonuses(self):
        self.market({"battlefield": ["Elvish Archdruid", "Island", "Forest"], "hand": ["Dress Down"]})
        self.choose_modes([2])
        self.pass_round()
        shape, = self.tokens("Shapeshifter")
        self.assertEqual(self.object(shape)["power"], 4)
        self.cast("Dress Down", mana=self.ids('Island', zone="battlefield", controller=self.engine.state["asked"]["player"])[0:1] + self.ids('Forest', zone="battlefield", controller=self.engine.state["asked"]["player"])[0:1], spend={'U': 1, 'G': 1})
        self.pass_round()  # Dress Down entry draw is now stacked
        self.pass_round()
        self.assertEqual(self.object(shape)["power"], 3)
        self.assertTrue({"Elf", "Goblin", "Time Lord"} <= set(self.object(shape)["subtypes"]))

    def test_printed_changeling_type_queries_in_hand_stack_and_graveyard(self):
        self.market({"battlefield": ["Forest", "Mountain"], "hand": ["Universal Automaton", "Shock"]})
        types = {"Elf", "Goblin", "Time Lord", "Performer", "Construct", "Shapeshifter"}
        hand_card = self.ids("Universal Automaton")[0]
        self.assertTrue(types <= set(self.object(hand_card)["subtypes"]))
        self.choose_modes([0])
        self.pass_round()
        self.cast("Universal Automaton", mana=self.ids('Forest', zone="battlefield", controller=self.engine.state["asked"]["player"])[0:1], spend={'G': 1})
        spell = self.engine.state["stack"][-1]
        self.assertTrue(types <= set(self.object(spell)["subtypes"]))
        self.pass_round()
        creature = next(o for o in self.ids("Universal Automaton") if o in self.engine.state["battlefield"])
        self.cast("Shock", [[{"object": creature}]], mana=self.ids('Mountain', zone="battlefield", controller=self.engine.state["asked"]["player"])[0:1], spend={'R': 1})
        self.pass_round()
        grave = next(o for o in self.ids("Universal Automaton") if o in self.engine.state["players"][0]["graveyard"])
        self.assertTrue(types <= set(self.object(grave)["subtypes"]))

    def test_selected_face_cast_permission_lists_and_accepts_changeling_with_existing_limit(self):
        self.engine.ask({"cmd": "new", "players": [
            {"library": ["Forest"] * 16, "battlefield": ["Lurrus of the Dream-Den", "Forest", "Forest"],
             "graveyard": ["Universal Automaton", "Universal Automaton", "Forest"]},
            {"library": ["Forest"] * 16}, {"library": ["Forest"] * 16}]})
        self.advance(lambda s: s["part"] == "PrecombatMain" and s["asked"]["kind"] == "priority")
        shapes = self.ids("Universal Automaton")
        forest = next(i for i in self.ids("Forest") if i in self.engine.state["players"][0]["graveyard"])
        self.reject(0, {"kind": "cast", "card": forest, "targets": [], "plan": []})
        self.cast("Universal Automaton", card=shapes[0], mana=self.ids("Forest", zone="battlefield", controller=0)[:1],
                  route={"source": self.ids("Lurrus of the Dream-Den")[0], "ability": 2}, spend={'G': 1})
        stack = self.engine.state["stack"][-1]
        self.assertTrue({"Elf", "Goblin", "Time Lord"} <= set(self.object(stack)["subtypes"]))
        self.pass_round()
        remaining = next(i for i in shapes if i in self.engine.state["players"][0]["graveyard"])
        self.reject(0, {"kind": "cast", "card": remaining, "route": {"source": self.ids("Lurrus of the Dream-Den")[0], "ability": 2}, "targets": [],
                        "plan": [{"source": self.ids("Forest", zone="battlefield", controller=0)[1], "ability": 0}]})

    def test_insufficient_life_does_not_hide_modes_and_all_instructions_finish_before_elimination(self):
        options = self.market(life=1)
        self.assertIn((0, 1, 2), options)
        self.choose_modes([0, 1, 2])
        self.pass_round()
        self.assertEqual(self.engine.state["players"][0]["life"], -5)
        self.assertTrue(self.engine.state["players"][0]["lost"])
