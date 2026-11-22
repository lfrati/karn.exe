"""Original-card amounts versus returned-object identity through audited JSON."""
from client_support import ClientTestCase


class ReturnReferenceClientTests(ClientTestCase):
    def test_reanimate_clone_reads_graveyard_card_value_after_entry_copy_choice(self):
        self.new([{'hand':['Reanimate'],'battlefield':['Swamp'],'graveyard':['Clone']},
                  {'battlefield':['Colossal Dreadmaw']}, {}])
        dead=self.engine.state['players'][0]['graveyard'][0]
        self.cast('Reanimate',[[{'object':dead}]], mana=self.ids('Swamp', controller=0), spend={'B': 1});self.pass_round()
        self.assertEqual(self.engine.state['asked']['kind'],'select')
        self.assertEqual(self.engine.state['players'][0]['life'],20)
        big=next(i for i in self.ids('Colossal Dreadmaw') if i in self.engine.state['battlefield'])
        self.reject(1,{'kind':'choose','answer':{'objects':[big]}})
        self.choose({'objects':[big]})
        self.assertEqual(self.engine.state['players'][0]['life'],16)
        copies=[self.object(i) for i in self.engine.state['battlefield'] if self.object(i)['name']=='Colossal Dreadmaw']
        self.assertEqual(len(copies),2);self.assertTrue(all(c['power']==6 for c in copies))
        for viewer in [0,1,2]:self.assertEqual(self.view(viewer)['players'][0]['life'],16)

    def test_cage_prevents_entry_but_reanimate_still_loses_card_mana_value(self):
        self.new([{'hand':['Reanimate'],'battlefield':['Swamp'],'graveyard':['Centaur Courser']},
                  {'battlefield':["Grafdigger's Cage"]}, {}])
        dead=self.engine.state['players'][0]['graveyard'][0]
        self.cast('Reanimate',[[{'object':dead}]], mana=self.ids('Swamp', controller=0), spend={'B': 1});self.pass_round()
        self.assertEqual(self.engine.state['asked']['kind'],'priority')
        self.assertEqual(self.engine.state['players'][0]['life'],17)
        self.assertTrue(any(self.object(i)['name']=='Centaur Courser' for i in self.engine.state['players'][0]['graveyard']))
        self.assertFalse(any(self.object(i)['name']=='Centaur Courser' for i in self.engine.state['battlefield']))
        for viewer in [0,1,2]:self.assertEqual(self.view(viewer)['players'][0]['life'],17)

    def test_fearsome_awakening_checks_the_returned_copys_dragon_type(self):
        self.new([{'hand':['Fearsome Awakening'],'battlefield':['Swamp']*5,'graveyard':['Clone']},
                  {'battlefield':['Shivan Dragon']}, {}])
        dead=self.engine.state['players'][0]['graveyard'][0]
        self.cast('Fearsome Awakening',[[{'object':dead}]], mana=self.ids('Swamp', controller=0), spend={'B': 5});self.pass_round()
        self.assertEqual(self.engine.state['asked']['kind'],'select')
        dragon=next(i for i in self.ids('Shivan Dragon') if i in self.engine.state['battlefield'])
        self.choose({'objects':[dragon]})
        returned=next(self.object(i) for i in self.engine.state['battlefield']
                      if self.object(i)['name']=='Shivan Dragon' and self.object(i)['controller']==0)
        self.assertEqual(returned['power'],7);self.assertEqual(returned['toughness'],7)
        self.assertIn('Dragon',returned['subtypes'])

    def test_too_greedily_returned_clone_excludes_itself_from_other_creatures_damage(self):
        self.new([{'hand': ['Too Greedily, Too Deep'],
                   'battlefield': ['Swamp'] * 6 + ['Mountain'], 'graveyard': ['Clone']},
                  {'battlefield': ['Colossal Dreadmaw', 'Wall of Denial']}, {}])
        dead = self.engine.state['players'][0]['graveyard'][0]
        big = next(i for i in self.ids('Colossal Dreadmaw') if i in self.engine.state['battlefield'])
        wall = self.ids('Wall of Denial')[0]
        self.cast('Too Greedily, Too Deep', [[{'object': dead}]],
                  mana=self.ids('Swamp', controller=0) + self.ids('Mountain', controller=0), spend={'B': 6, 'R': 1})
        self.pass_round()
        self.assertEqual((self.engine.state['asked']['kind'], self.engine.state['asked']['player']), ('select', 0))
        self.reject(1, {'kind': 'choose', 'answer': {'objects': [big]}})
        self.choose({'objects': [big]})
        actor, = [i for i in self.engine.state['battlefield']
                  if self.object(i)['name'] == 'Colossal Dreadmaw' and self.object(i)['controller'] == 0]
        self.assertEqual((self.object(actor)['power'], self.object(actor)['damage']), (6, 0))
        self.assertEqual(self.object(wall)['damage'], 6)
        self.assertNotIn(big, self.engine.state['battlefield'])
        dealt = [e for e in self.engine.state['events'] if e['event'] == 'damage']
        self.assertEqual({(e['source'], e['to']['object'], e['amount']) for e in dealt},
                         {(actor, big, 6), (actor, wall, 6)})
        for viewer in [0, 1, 2]:
            view = self.view(viewer)
            self.assertIn(actor, view['battlefield'])
            self.assertEqual(view['objects'][str(actor)]['damage'], 0)
