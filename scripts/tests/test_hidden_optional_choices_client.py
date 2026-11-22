"""Hidden qualification must not change another seat's decision transcript (402.3).

The paired games differ in one concealed card. In both, the chooser declines the
optional action; only the chooser may learn whether a qualifying card existed.
"""
import unittest
import play
import client_support as helpers


class HiddenOptionalChoicesClientTests(unittest.TestCase):
    setUpClass = classmethod(helpers.ClientTestCase.setUpClass.__func__)
    new = helpers.ClientTestCase.new
    ids = helpers.ClientTestCase.ids
    cast = helpers.ClientTestCase.cast
    pass_round = helpers.ClientTestCase.pass_round

    def exercise(self, case, qualifies):
        self.engine = play.Engine(audit=True)
        try:
            if case == 'entry':
                self.new([{'hand': ["Theorist's Sanctum", 'Jace Beleren' if qualifies else 'Grizzly Bears']}, {}])
                land, = self.ids("Theorist's Sanctum")
                self.engine.act({'kind': 'play', 'card': land})
            elif case == 'looked_at':
                self.new([{'hand': ['Commune with Nature'], 'battlefield': ['Forest'],
                           'library': ['Island'] * 4 + ['Grizzly Bears' if qualifies else 'Forest']}, {}])
                self.cast('Commune with Nature', mana=self.ids('Forest', zone='battlefield'), spend={'G': 1})
                self.pass_round()
            elif case == 'discard':
                self.new([{'hand': ['Winternight Stories', 'Grizzly Bears' if qualifies else 'Plains'],
                           'battlefield': ['Island'] * 3}, {}])
                self.cast('Winternight Stories', mana=self.ids('Island', zone='battlefield'), spend={'U': 3})
                self.pass_round()
            elif case == 'behold':
                self.new([{'hand': ['Sarkhan, Dragon Ascendant', 'Shivan Dragon' if qualifies else 'Grizzly Bears'],
                           'battlefield': ['Mountain'] * 2}, {}])
                self.cast('Sarkhan, Dragon Ascendant', mana=self.ids('Mountain'), spend={'R': 2})
                self.pass_round()
                self.pass_round()
            elif case == 'put_land':
                self.new([{'hand': ['Lessons from Life', 'Forest' if qualifies else 'Grizzly Bears'],
                           'library': ['Grizzly Bears'] * 8, 'battlefield': ['Forest'] * 3 + ['Island']}, {}])
                self.cast('Lessons from Life', mana=self.ids('Forest', zone='battlefield') + self.ids('Island'),
                          spend={'G': 3, 'U': 1})
                self.pass_round()
            elif case == 'fetch_behold':
                self.new([{'hand': ['Llanowar Elves' if qualifies else 'Grizzly Bears'],
                           'library': ['Forest'], 'battlefield': ['Elven Passage']}, {}])
                passage, = self.ids('Elven Passage')
                self.engine.act({'kind': 'activate', 'source': passage, 'ability': 0,
                                 'targets': [], 'plan': [], 'paying': []})
                self.pass_round()
                asked = self.engine.state['asked']
                self.assertEqual(asked['kind'], 'search')
                self.engine.act({'kind': 'choose', 'answer': {'objects': asked['among'][:1]}})
            else:
                raise AssertionError(case)
            kind = self.engine.state['asked']['kind']
            before = self.engine.send({'cmd': 'prompt', 'seat': 'p1'})
            optional = kind in ('may', 'optional_objects', 'select')
            if optional:
                if not qualifies and kind in ('may', 'optional_objects'):
                    request = self.engine.state['request_id']
                    unchanged = self.engine.send({'cmd': 'state', 'viewer': 0})
                    answer = 'yes' if kind == 'may' else {'accept': True, 'objects': []}
                    with self.assertRaises(play.Refused) as refused:
                        self.engine.act({'kind': 'choose', 'answer': answer})
                    self.assertEqual(refused.exception.refusal_class, 'illegal')
                    state = self.engine.send({'cmd': 'state', 'viewer': 0})
                    self.assertEqual(state['request_id'], request)
                    self.assertEqual(state, unchanged)
                answer = {'may': 'no', 'optional_objects': {'accept': False}, 'select': {'objects': []}}[kind]
                self.engine.act({'kind': 'choose', 'answer': answer})
            after = self.engine.send({'cmd': 'prompt', 'seat': 'p1'})
            return kind, before, after
        finally:
            self.engine.proc.stdin.close()
            self.engine.proc.wait(timeout=10)
            self.engine.proc.stdout.close()

    def same_decline(self, case):
        empty_kind, empty_before, empty_after = self.exercise(case, False)
        held_kind, held_before, held_after = self.exercise(case, True)
        self.assertIn(held_kind, ('may', 'optional_objects', 'select'))
        self.assertEqual(empty_kind, held_kind, 'the optional boundary must exist in both games')
        self.assertEqual(empty_before, held_before, 'pending public facts must conceal qualification')
        self.assertEqual(empty_after, held_after, 'decline must leave the same public transcript and request IDs')

    def test_entry_does_not_disclose_whether_a_jace_is_held(self):
        self.same_decline('entry')

    def test_optional_reveal_does_not_disclose_looked_at_creature_qualification(self):
        self.same_decline('looked_at')

    def test_optional_discard_does_not_disclose_whether_a_creature_is_held(self):
        self.same_decline('discard')

    def test_optional_behold_does_not_disclose_whether_a_dragon_is_held(self):
        self.same_decline('behold')

    def test_optional_entry_does_not_disclose_whether_a_land_is_held(self):
        self.same_decline('put_land')

    def test_fetch_followup_does_not_disclose_whether_an_elf_is_held(self):
        self.same_decline('fetch_behold')


if __name__ == '__main__':
    unittest.main()
