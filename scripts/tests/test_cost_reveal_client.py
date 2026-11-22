"""701.20a: cost disclosures remain public for the announcing stack object's lifetime."""
import unittest
import test_grant_client as helpers


KINDLE = 'Kindle the Inner Flame'


class CostRevealClientTests(unittest.TestCase):
    setUpClass = classmethod(helpers.GrantClientTests.setUpClass.__func__)
    setUp = helpers.GrantClientTests.setUp
    close_engine = helpers.GrantClientTests.close_engine
    new = helpers.GrantClientTests.new
    ids = helpers.GrantClientTests.ids
    cast = helpers.GrantClientTests.cast
    pass_round = helpers.GrantClientTests.pass_round
    view = helpers.GrantClientTests.view
    hand = helpers.GrantClientTests.hand

    def revealed(self, *cards):
        other = self.view(1)
        self.assertTrue(set(cards).issubset(other['publicly_revealed']))
        for card in cards:
            self.assertTrue(other['objects'][str(card)]['name'])

    def hidden(self, *cards):
        other = self.view(1)
        self.assertTrue(set(cards).isdisjoint(other.get('publicly_revealed', [])))
        for card in cards:
            self.assertNotIn(str(card), other['objects'])

    def test_kindle_cost_cards_remain_visible_until_resolution_finishes(self):
        self.new([{'graveyard': [KINDLE], 'hand': [KINDLE, 'Fire Elemental'],
                   'battlefield': ['Air Elemental', 'Mountain', 'Mountain']}, {}])
        kindred, = [card for card in self.ids(KINDLE) if card in self.hand(0)]
        spell, = self.engine.state['players'][0]['graveyard']
        fire, = self.ids('Fire Elemental')
        air, = self.ids('Air Elemental')
        self.cast(KINDLE, [[{'object': air}]], card=spell, route='flashback', paying=[[kindred, fire, air]],
                  mana=self.ids('Mountain', zone='battlefield'), spend={'R': 2})
        self.revealed(kindred, fire)
        self.pass_round()
        self.assertEqual(self.engine.state['stack'], [])
        self.hidden(kindred, fire)

    def test_countering_one_of_two_revealing_spells_preserves_the_other_grant(self):
        self.new([{'hand': ['Caustic Exhale', 'Caustic Exhale', 'Shivan Dragon'], 'battlefield': ['Swamp'] * 2},
                  {'hand': ['Counterspell'], 'battlefield': ['Island'] * 2 + ['Grizzly Bears']}])
        dragon, = self.ids('Shivan Dragon')
        bear, = self.ids('Grizzly Bears')
        for swamp in self.ids('Swamp', zone='battlefield'):
            self.cast('Caustic Exhale', [[{'object': bear}]], paying=[[dragon]], chose=[0],
                      mana=[swamp], spend={'B': 1})
        top = self.engine.state['stack'][-1]
        self.revealed(dragon)
        self.engine.act({'kind': 'pass'})
        self.cast('Counterspell', [[{'object': top}]], mana=self.ids('Island', zone='battlefield'), spend={'U': 2})
        self.pass_round()
        self.assertEqual(len(self.engine.state['stack']), 1)
        self.revealed(dragon)
        self.pass_round()
        self.assertEqual(self.engine.state['stack'], [])
        self.hidden(dragon)


if __name__ == '__main__':
    unittest.main()
