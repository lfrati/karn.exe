"""Completed discard counts through the actual client."""
import unittest
import test_grant_client as helpers

class FilteredDiscardResultClientTests(unittest.TestCase):
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

    def tokens(self):
        return [o for o in self.engine.state['battlefield'] if self.object(o)['token']]
    def play_opening_draw(self):
        forest=next(i for i in self.hand(0) if self.object(i)["name"]=="Forest")
        self.engine.act({"kind":"play","card":forest})
    def resolve(self):
        self.cast('Seasoned Pyromancer', mana=self.ids('Mountain', zone="battlefield", controller=self.engine.state["asked"]["player"])[0:3], spend={'R': 3}); self.pass_round(); self.pass_round()
    def test_full_private_mixed_discard_draw_and_public_single_token(self):
        self.new([{'battlefield':['Mountain']*3, 'hand':['Seasoned Pyromancer','Grizzly Bears','Island','Hill Giant']}, {}, {}])
        self.play_opening_draw(); self.resolve()
        asked=self.engine.state['asked']
        self.assertEqual((asked['kind'],asked['verb'],asked['player'],asked['count']),('objects','Discard',0,2))
        chosen=[i for i in asked['among'] if self.object(i)['name'] in ['Grizzly Bears','Island']]
        self.assertEqual(len(chosen),2)
        for observer in [1,2]:
            view=self.view(observer)
            self.assertNotIn('among',view['asked']);self.assertNotIn('hand',view['players'][0])
            self.assertTrue(all(str(i) not in view['objects'] for i in chosen))
        self.reject(1,{'kind':'choose','answer':{'objects':chosen}})
        self.reject(0,{'kind':'choose','answer':{'objects':[chosen[0],chosen[0]]}})
        self.choose({'objects':chosen})
        self.assertEqual(self.engine.state['asked']['kind'],'order_cards')
        self.choose({'objects':chosen})  # same authored Bears/Island order, top first (404.3)
        self.assertEqual(len(self.hand(0)),3); self.assertEqual(len(self.tokens()),1)
        for observer in [0,1,2]:
            view=self.view(observer)
            names=[view['objects'][str(i)]['name'] for i in view['players'][0]['graveyard']]
            self.assertCountEqual(names,['Grizzly Bears','Island'])
            token=view['objects'][str(self.tokens()[0])]
            self.assertEqual((token['power'],token['toughness']),(1,1))
    def test_empty_short_and_all_nonland_hands_count_only_pre_draw_discards(self):
        for cards,expected in [([],0),(['Island'],0),(['Grizzly Bears'],1),(['Grizzly Bears','Hill Giant'],2)]:
            self.new([{'battlefield':['Mountain']*3,'hand':['Seasoned Pyromancer']+cards}, {}, {}])
            self.play_opening_draw()
            discarded=[next(i for i in self.hand(0) if self.object(i)["name"]==name) for name in cards]
            self.resolve()
            if len(cards)==2:
                self.choose({"objects":discarded})
            self.assertEqual(len(self.hand(0)),2);self.assertEqual(len(self.tokens()),expected)
    def test_graveyard_activation_is_offered_pays_exile_and_creates_two(self):
        self.new([{'battlefield':['Mountain']*5,'graveyard':['Seasoned Pyromancer']}, {}, {}])
        card=self.ids('Seasoned Pyromancer')[0]
        self.activate(card, [], ability=1, mana=self.ids("Mountain", zone="battlefield", controller=0)[:5], spend={'R': 5})
        self.assertFalse(self.engine.state['players'][0]['graveyard'])
        self.assertTrue(any(self.object(o)['name']=='Seasoned Pyromancer' for o in self.engine.state['exile']))
        self.pass_round();self.assertEqual(len(self.tokens()),2)

if __name__=='__main__':unittest.main()
