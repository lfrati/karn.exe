"""Reflexive placement/payment regressions through the audited JSON client."""
from client_support import ClientTestCase


class ReflexiveTriggerClientTests(ClientTestCase):
    def attack(self):
        source=self.ids('Labyrinth Adversary')[0]
        self.advance(lambda s:s['asked']['kind']=='attackers')
        self.engine.act({'kind':'attack','attacks':[[source,{'player':1}]]})
        self.assertEqual(self.engine.state['asked']['kind'],'priority')
        self.assertEqual(len(self.engine.state['stack']),1)
        self.pass_round()
        return source
    def test_full_adversary_pay_then_fresh_target_then_blocking_restriction(self):
        self.new([{'battlefield':['Labyrinth Adversary','Mountain','Mountain']},
                  {'battlefield':['Grizzly Bears','Darksteel Myr']},{}])
        source=self.attack();target=self.ids('Grizzly Bears')[0]
        self.assertEqual(self.engine.state['asked']['kind'],'pay')
        self.reject(1,{'kind':'choose','answer':'yes'})
        self.choose({'pay':{'plan':[{'source':o,'ability':0} for o in self.ids('Mountain')], 'mana': {'R': 2}}});self.assertEqual(self.engine.state['asked']['kind'],'targets')
        self.reject(1,{'kind':'targets','targets':[[{'object':target}]]})
        self.reject(0,{'kind':'targets','targets':[[{'player':1}]]})
        self.engine.act({'kind':'targets','targets':[[{'object':target}]]})
        self.assertEqual(self.engine.state['asked']['kind'],'priority')
        self.assertEqual(len(self.engine.state['stack']),1)
        self.pass_round();self.advance(lambda s:s['asked']['kind']=='blockers')
        source=self.ids('Labyrinth Adversary')[0];target=self.ids('Grizzly Bears')[0]
        self.reject(1,{'kind':'block','blocks':[[target,source]]})
        self.engine.act({'kind':'block','blocks':[]})
    def test_adversary_decline_and_unpayable_do_not_ask_targets(self):
        for mana in [False,True]:
            with self.subTest(mana=mana):
                self.new([{'battlefield':['Labyrinth Adversary']+(['Mountain']*2 if mana else [])},{},{}])
                self.attack()
                self.assertEqual(self.engine.state['asked']['kind'],'pay')
                if not mana:
                    self.reject(0,{'kind':'choose','answer':{'pay':{'plan':[]}}})
                    self.assertEqual(self.engine.state['asked']['kind'],'pay')
                self.choose('no')
                self.assertEqual(self.engine.state['asked']['kind'],'priority')
                self.assertFalse(self.engine.state['stack'])
    def glorifier(self,others):
        self.new([{'hand':['Glorifier of Suffering'],'battlefield':['Plains']*3+others},
                  {'hand':['Shock'],'battlefield':['Mountain']},{}])
        self.cast('Glorifier of Suffering',mana=self.ids('Plains'), spend={'W': 3});self.pass_round()
        self.assertEqual(self.engine.state['asked']['kind'],'priority')
        self.assertEqual(len(self.engine.state['stack']),1)
        self.pass_round()
    def test_glorifier_sacrifice_then_new_targets_outlive_source_with_response_window(self):
        self.glorifier(['Ornithopter','Darksteel Myr'])
        source=next(i for i in self.ids('Glorifier of Suffering') if i in self.engine.state['battlefield']);sacrifice=self.ids('Ornithopter')[0];target=self.ids('Darksteel Myr')[0]
        self.assertEqual(self.engine.state['asked']['kind'],'may');self.choose('yes')
        self.assertEqual(self.engine.state['asked']['kind'],'objects')
        self.reject(0,{'kind':'choose','answer':{'objects':[source]}})
        self.choose({'objects':[sacrifice]});self.assertEqual(self.engine.state['asked']['kind'],'targets')
        self.assertNotIn(sacrifice,self.engine.state['battlefield'])
        self.engine.act({'kind':'targets','targets':[[{'object':target}]]})
        self.assertEqual(self.object(target)['toughness'],1)
        self.engine.act({'kind':'pass'});source=next(i for i in self.ids('Glorifier of Suffering') if i in self.engine.state['battlefield'])
        self.cast('Shock',[[{'object':source}]],mana=self.ids('Mountain'), spend={'R': 1});self.pass_round()
        self.assertNotIn(source,self.engine.state['battlefield'])
        self.assertEqual(len(self.engine.state['stack']),1)
        self.pass_round();target=self.ids('Darksteel Myr')[0]
        self.assertEqual(self.object(target)['toughness'],2)
        for viewer in [0,1,2]:
            view=self.view(viewer);myr=next(o for o in view['objects'].values() if o['name']=='Darksteel Myr')
            self.assertEqual(myr['toughness'],2)
    def test_glorifier_empty_declined_and_zero_target_occurrences(self):
        for mode in ['empty','decline','zero']:
            with self.subTest(mode=mode):
                self.glorifier([] if mode=='empty' else ['Ornithopter'])
                if mode=='decline':self.choose('no')
                elif mode=='zero':
                    self.choose('yes');self.assertEqual(self.engine.state['asked']['kind'],'targets')
                    self.engine.act({'kind':'targets','targets':[[]]})
                    self.assertEqual(len(self.engine.state['stack']),1);self.pass_round()
                self.assertEqual(self.engine.state['asked']['kind'],'priority');self.assertFalse(self.engine.state['stack'])

    def test_terror_decline_and_accepted_return_include_the_phyrexian_rider(self):
        for accept in [False, True]:
            with self.subTest(accept=accept):
                self.new([{'battlefield':['Terror of Towashi']+['Swamp']*4,
                           'graveyard':['Grizzly Bears']}, {}, {}])
                self.advance(lambda s:s['asked']['kind']=='attackers')
                source=next(i for i in self.ids('Terror of Towashi') if i in self.engine.state['battlefield'])
                self.engine.act({'kind':'attack','attacks':[[source,{'player':1}]]})
                self.assertEqual(self.engine.state['asked']['kind'],'priority')
                self.assertEqual(len(self.engine.state['stack']),1)
                self.pass_round();self.assertEqual(self.engine.state['asked']['kind'],'pay')
                self.reject(1,{'kind':'choose','answer':'yes'})
                self.choose({'pay':{'plan':[{'source':o,'ability':0} for o in self.ids('Swamp')], 'mana': {'B': 4}}} if accept else 'no')
                self.assertEqual(len(self.engine.state['players'][0]['graveyard']),1)
                if not accept:
                    self.assertFalse(self.engine.state['stack'])
                    self.assertEqual(self.engine.state['asked']['kind'],'priority')
                    continue
                self.assertEqual(self.engine.state['asked']['kind'],'targets')
                dead=self.engine.state['players'][0]['graveyard'][0]
                self.engine.act({'kind':'targets','targets':[[{'object':dead}]]})
                self.assertEqual(self.engine.state['asked']['kind'],'priority')
                self.assertEqual(len(self.engine.state['stack']),1)
                self.assertEqual(len(self.engine.state['players'][0]['graveyard']),1)
                self.pass_round()
                self.assertFalse(self.engine.state['players'][0]['graveyard'])
                for viewer in [0,1,2]:
                    view=self.view(viewer)
                    bear=next(view['objects'][str(i)] for i in view['battlefield']
                              if view['objects'][str(i)]['name']=='Grizzly Bears')
                    self.assertIn('Bear',bear['subtypes']);self.assertIn('Phyrexian',bear['subtypes'])
