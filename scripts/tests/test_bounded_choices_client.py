"""Full pinned cards through the audited client, including private bounded choices."""
import unittest
import test_grant_client as helpers


class BoundedChoicesClientTests(unittest.TestCase):
    setUpClass = classmethod(helpers.GrantClientTests.setUpClass.__func__)
    setUp = helpers.GrantClientTests.setUp
    close_engine = helpers.GrantClientTests.close_engine
    new = helpers.GrantClientTests.new
    view = helpers.GrantClientTests.view
    hand = helpers.GrantClientTests.hand
    ids = helpers.GrantClientTests.ids
    pass_round = helpers.GrantClientTests.pass_round
    choose = helpers.GrantClientTests.choose
    reject = helpers.GrantClientTests.reject
    cast = helpers.GrantClientTests.cast
    object = helpers.GrantClientTests.object
    activate = helpers.GrantClientTests.activate
    step = helpers.GrantClientTests.step
    advance = helpers.GrantClientTests.advance

    def test_ballista_uses_its_cost_source_after_its_last_counter_is_removed(self):
        self.new([{'hand': ['Walking Ballista'], 'battlefield': ['Forest', 'Forest']}, {}, {}])
        self.cast('Walking Ballista', mana=self.ids('Forest', zone='battlefield')[:2], spend={'G': 2}, x=1)
        self.pass_round()
        source = self.ids('Walking Ballista', zone='battlefield')[0]
        self.activate(source, [[{'player': 2}]], ability=2)
        self.assertFalse(self.ids('Walking Ballista', zone='battlefield'))
        self.pass_round()
        self.assertEqual(self.engine.state['players'][2]['life'], 19)

    def test_fable_bounds_are_private_and_invalid_answers_preserve_the_request(self):
        self.new([{'hand': ['Fable of the Mirror-Breaker', 'Island', 'Grizzly Bears'],
                   'battlefield': ['Mountain'] * 3}, {}, {}])
        self.cast('Fable of the Mirror-Breaker', mana=self.ids('Mountain', zone='battlefield')[:3], spend={'R': 3})
        self.pass_round()
        self.pass_round()
        turn = self.engine.state['turn']
        self.advance(lambda s: s['turn'] > turn and s['active'] == 0 and s['part'] == 'PrecombatMain')
        self.assertEqual(len(self.engine.state['stack']), 1)
        self.pass_round()
        self.assertEqual(self.engine.state['asked']['kind'], 'may')
        self.choose('yes')
        asked = self.engine.state['asked']
        self.assertEqual((asked['kind'], asked['verb'], asked['min'], asked['max']), ('objects', 'Discard', 0, 2))
        cards = asked['among']
        self.assertGreaterEqual(len(cards), 3)
        for observer in [1, 2]:
            view = self.view(observer)
            self.assertNotIn('among', view['asked'])
            self.assertTrue(all(str(card) not in view['objects'] for card in cards))
        self.reject(1, {'kind': 'choose', 'answer': {'objects': cards[:1]}})
        self.reject(0, {'kind': 'choose', 'answer': {'objects': cards[:3]}})
        self.reject(0, {'kind': 'choose', 'answer': {'objects': [cards[0], cards[0]]}})
        before = len(self.hand(0))
        self.choose({'objects': cards[:1]})
        self.assertEqual(len(self.hand(0)), before)
        self.assertEqual(self.engine.state['asked']['kind'], 'priority')


if __name__ == '__main__':
    unittest.main()
