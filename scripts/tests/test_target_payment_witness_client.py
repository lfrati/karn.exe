"""Authored target/payment actions exercise collect evidence through the JSON client.

Focused fixture positions isolate 601.2c/f/h and 701.59; these are not games from an ordinary start.
"""
import os
import unittest
from pathlib import Path
import sys
sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import play
from rust_tools import fast_binary

class TargetPaymentWitnessClientTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.binary=Path(os.environ.get('GAME_SERVE_REVIEW',fast_binary('game-serve')))
        if not cls.binary.exists():raise unittest.SkipTest('build game-serve first')
    def setUp(self):
        self.engine=play.Engine(audit=True,executable=str(self.binary))
        self.addCleanup(self.close)
    def close(self):
        self.engine.proc.stdin.close();self.engine.proc.wait(timeout=10);self.engine.proc.stdout.close()
    def setup(self,graveyard):
        self.engine.ask({'cmd':'new','players':[
            {'library':['Forest']*8,'hand':['Urgent Necropsy'],'battlefield':['Swamp','Swamp','Forest','Forest'],'graveyard':graveyard},
            {'library':['Forest']*8,'battlefield':['Llanowar Elves']}]})
        for _ in range(30):
            if self.engine.state['part']=='PrecombatMain':return
            self.engine.act({'kind':'pass'})
        self.fail('main phase not reached')
    def test_authored_evidence_payment_destroys_the_selected_creature(self):
        self.setup(['Trumpeting Carnosaur'])
        objects=self.engine.state['objects']
        elf=next(int(i) for i,o in objects.items() if o['name']=='Llanowar Elves')
        card=next(i for i in self.engine.state['players'][0]['hand'] if objects[str(i)]['name']=='Urgent Necropsy')
        evidence=self.engine.state['players'][0]['graveyard']
        plan=[{'source':i,'ability':0} for i in self.engine.state['battlefield'] if objects[str(i)]['name'] in ('Swamp','Forest')]
        self.engine.act({'kind':'cast','card':card,'targets':[[],[{'object':elf}],[],[]],'paying':[evidence],'plan':plan, 'mana': {'B': 2, 'G': 2}})
        self.assertEqual(len(self.engine.state['exile']),1)
        for _ in range(2):self.engine.act({'kind':'pass'})
        self.assertNotIn(elf,self.engine.state['battlefield'])
        self.assertEqual(self.engine.state['players'][0]['life'],20)
    def test_empty_graveyard_refuses_positive_value_targets_and_allows_no_targets(self):
        self.setup([])
        objects=self.engine.state['objects']
        elf=next(int(i) for i,o in objects.items() if o['name']=='Llanowar Elves')
        card=next(i for i in self.engine.state['players'][0]['hand'] if objects[str(i)]['name']=='Urgent Necropsy')
        plan=[{'source':i,'ability':0} for i in self.engine.state['battlefield'] if objects[str(i)]['name'] in ('Swamp','Forest')]
        before=self.engine.state
        with self.assertRaises(SystemExit):
            self.engine.act({'kind':'cast','card':card,'targets':[[],[{'object':elf}],[],[]],'paying':[[]],'plan':plan, 'mana': {'B': 2, 'G': 2}})
        after=self.engine.send({"cmd":"state","viewer":"asked"})
        self.assertEqual(after["request_id"],before["request_id"])
        self.assertEqual(after["players"][0]["pool"],before["players"][0]["pool"])
        self.assertTrue(all(not after["objects"][str(step["source"])]["tapped"] for step in plan))
        self.assertIn(card,after["players"][0]["hand"])
        self.engine.act({'kind':'cast','card':card,'targets':[[],[],[],[]],'paying':[[]],'plan':plan, 'mana': {'B': 2, 'G': 2}})
        self.assertEqual(self.engine.state['exile'],[])
