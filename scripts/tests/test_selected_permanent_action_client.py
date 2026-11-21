"""Non-targeted permanent selections through the actual JSON client."""
import unittest
import test_grant_client as helpers

class SelectedPermanentActionClientTests(unittest.TestCase):
    setUpClass=classmethod(helpers.GrantClientTests.setUpClass.__func__)
    setUp=helpers.GrantClientTests.setUp
    close_engine=helpers.GrantClientTests.close_engine
    new=helpers.GrantClientTests.new
    view=helpers.GrantClientTests.view
    hand=helpers.GrantClientTests.hand
    ids=helpers.GrantClientTests.ids
    object=helpers.GrantClientTests.object
    cast=helpers.GrantClientTests.cast
    activate=helpers.GrantClientTests.activate
    pass_round=helpers.GrantClientTests.pass_round
    choose=helpers.GrantClientTests.choose
    reject=helpers.GrantClientTests.reject
    advance=helpers.GrantClientTests.advance
    step=helpers.GrantClientTests.step

    def play_sanctuary(self):
        card=self.ids('Selesnya Sanctuary')[0]
        self.engine.act({'kind':'play','card':card});self.pass_round()
    def test_full_sanctuary_public_choice_owner_and_land_limit(self):
        self.new([{'hand':['Selesnya Sanctuary'],'battlefield':['Forest']},{'battlefield':['Island']},{}])
        self.play_sanctuary()
        asked=self.engine.state['asked'];self.assertEqual((asked['kind'],asked['player'],asked['count']),('objects',0,1))
        sanctuary=next(i for i in self.ids('Selesnya Sanctuary') if i in self.engine.state['battlefield'])
        forest=next(i for i in self.ids('Forest') if i in self.engine.state['battlefield'])
        island=self.ids('Island')[0]
        self.assertCountEqual(asked['among'],[sanctuary,forest])
        self.reject(1,{'kind':'choose','answer':{'objects':[forest]}})
        self.reject(0,{'kind':'choose','answer':{'objects':[island]}})
        self.choose({'objects':[forest]});self.assertIn(sanctuary,self.engine.state['battlefield'])
        self.assertTrue(self.object(sanctuary)['tapped'])
        for viewer in [0,1,2]:
            self.assertIn(sanctuary,self.view(viewer)['battlefield'])
        returned = next(o for o in self.hand(0) if self.object(o)['name'] == 'Forest')
        self.reject(0, {'kind': 'play', 'card': returned})
    def test_full_sanctuary_alone_returns_itself_and_cannot_be_replayed_this_turn(self):
        self.new([{'hand':['Selesnya Sanctuary']},{},{}]);self.play_sanctuary()
        self.assertFalse(self.engine.state['battlefield'])
        card=next(i for i in self.hand(0) if self.object(i)['name']=='Selesnya Sanctuary')
        self.reject(0,{'kind':'play','card':card})
    def test_full_scuttler_returns_stolen_shrouded_artifact_to_its_owner(self):
        self.new([{'battlefield':['Salvage Scuttler','Lightning Greaves']+['Mountain']*3,'hand':['Act of Treason']},
                  {'battlefield':['Darksteel Myr']},{}])
        myr=self.ids('Darksteel Myr')[0]
        self.cast('Act of Treason',[[{'object':myr}]], mana=self.ids('Mountain', zone="battlefield", controller=0)[:3], spend={'R': 3});self.pass_round()
        greaves=self.ids('Lightning Greaves')[0]
        self.activate(greaves,[[{'object':myr}]], ability=2);self.pass_round()
        self.assertEqual(self.object(greaves)['attached_to'],{'object':myr})
        self.reject(0,{'kind':'activate','source':greaves,'ability':2,
                       'targets':[[{'object':myr}]],'plan':[],'paying':[]})
        scuttler=self.ids('Salvage Scuttler')[0]
        self.advance(lambda s:s['asked']['kind']=='attackers')
        self.engine.act({'kind':'attack','attacks':[[scuttler,{'player':1}]]});self.pass_round()
        asked=self.engine.state['asked'];self.assertEqual((asked['kind'],asked['player']),('objects',0))
        self.assertIn(myr,asked['among']);self.reject(1,{'kind':'choose','answer':{'objects':[myr]}})
        self.choose({'objects':[myr]})
        returned=self.hand(1);self.assertTrue(any(self.view(1)['objects'][str(i)]['name']=='Darksteel Myr' for i in returned))
        self.assertIn(scuttler,self.engine.state['battlefield'])
    def test_full_scuttler_no_artifact_has_no_penalty_or_invented_choice(self):
        self.new([{'battlefield':['Salvage Scuttler']},{},{}]);scuttler=self.ids('Salvage Scuttler')[0]
        self.advance(lambda s:s['asked']['kind']=='attackers')
        self.engine.act({'kind':'attack','attacks':[[scuttler,{'player':1}]]});self.pass_round()
        self.assertEqual(self.engine.state['asked']['kind'],'priority');self.assertIn(scuttler,self.engine.state['battlefield'])
    def test_frantic_search_lists_only_possible_untaps_and_rejects_untapped_land(self):
        self.new([{'battlefield':['Island']*3+['Forest'],'hand':['Frantic Search']},{'battlefield':['Forest']},{}])
        self.cast('Frantic Search', mana=self.ids('Island', zone="battlefield", controller=0)[:3], spend={'U': 3});self.pass_round()
        asked=self.engine.state['asked'];self.assertEqual(asked['verb'],'Discard')
        forests=[o for o in self.hand(0) if self.object(o)['name']=='Forest']
        discarded=[forests[0],forests[1]]  # two fixed physical cards of the Forest-only draws
        self.choose({'objects':discarded})
        self.assertEqual(self.engine.state['asked']['kind'],'order_cards')
        self.choose({'objects':discarded})
        asked=self.engine.state['asked'];self.assertEqual((asked['kind'],asked['min'],asked['max']),('select',0,3))
        self.assertTrue(all(self.object(i)['tapped'] for i in asked['among']))
        untapped=next(i for i in self.ids('Forest') if i in self.engine.state['battlefield'] and not self.object(i)['tapped'])
        self.reject(0,{'kind':'choose','answer':{'objects':[untapped]}})
        self.choose({'objects':[]});self.assertEqual(self.engine.state['asked']['kind'],'priority')

if __name__=='__main__':unittest.main()
