"""A sentence after "When you do," belongs to the reflexive ability (603.12),
through the audited JSON client with Terror of Towashi (ruling 64f15a4f73fb18e1)."""
import unittest
import test_grant_client as helpers

class ReflexiveTrailingSentenceClientTests(unittest.TestCase):
    setUpClass=classmethod(helpers.GrantClientTests.setUpClass.__func__)
    setUp=helpers.GrantClientTests.setUp
    close_engine=helpers.GrantClientTests.close_engine
    new=helpers.GrantClientTests.new
    view=helpers.GrantClientTests.view
    ids=helpers.GrantClientTests.ids
    object=helpers.GrantClientTests.object
    pass_round=helpers.GrantClientTests.pass_round
    choose=helpers.GrantClientTests.choose
    reject=helpers.GrantClientTests.reject
    advance=helpers.GrantClientTests.advance
    step=helpers.GrantClientTests.step

    def attack(self):
        self.new([{'battlefield':['Terror of Towashi']+['Swamp']*4,'graveyard':['Grizzly Bears']},{},{}])
        source=self.ids('Terror of Towashi')[0]
        self.advance(lambda s:s['asked']['kind']=='attackers')
        self.engine.act({'kind':'attack','attacks':[[source,{'player':1}]]})
        self.assertEqual(len(self.engine.state['stack']),1)
        self.pass_round()
        self.assertEqual(self.engine.state['asked']['kind'],'pay')
    def bears(self):
        return [o for o in self.engine.state['objects'].values() if o['name']=='Grizzly Bears']
    def test_decline_does_nothing_and_accept_types_the_returned_creature(self):
        for accept in [False,True]:
            with self.subTest(accept=accept):
                self.attack()
                grave=self.engine.state['players'][0]['graveyard']
                self.assertEqual(len(grave),1)
                self.choose({'pay':{'plan':[{'source':o,'ability':0} for o in self.ids('Swamp')], 'mana': {'B': 4}}} if accept else 'no')
                # the parent has finished: no trailing sentence has done anything yet
                self.assertTrue(all('Phyrexian' not in b['subtypes'] for b in self.bears()))
                if not accept:
                    self.assertEqual(self.engine.state['asked']['kind'],'priority')
                    self.assertFalse(self.engine.state['stack'])
                    self.assertEqual(self.engine.state['players'][0]['graveyard'],grave)
                    continue
                self.assertEqual(self.engine.state['asked']['kind'],'targets')
                self.engine.act({'kind':'targets','targets':[[{'object':grave[0]}]]})
                self.assertEqual(len(self.engine.state['stack']),1)
                self.pass_round()
                back=next(i for i in self.engine.state['battlefield'] if self.object(i)['name']=='Grizzly Bears')
                self.assertIn('Phyrexian',self.object(back)['subtypes'])
                self.assertIn('Bear',self.object(back)['subtypes'])
                for viewer in [0,1,2]:
                    view=self.view(viewer)
                    self.assertIn('Phyrexian',view['objects'][str(back)]['subtypes'])

if __name__=='__main__':
    unittest.main()
