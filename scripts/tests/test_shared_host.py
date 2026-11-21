"""Actual shared-process client checks; no synthetic game rules."""
from concurrent.futures import ThreadPoolExecutor
import os
import json
import subprocess
from pathlib import Path
import sys
import unittest
sys.path.insert(0,str(Path(__file__).resolve().parents[1]))
from shared_host import SharedHost
from rust_tools import fast_binary

class SharedHostTests(unittest.TestCase):
    def test_concurrent_identity_ordering_close_and_private_views(self):
        with SharedHost(os.environ.get('MAGIC_ENGINE'),workers=4) as host:
            clients=[host.session() for _ in range(8)]
            def run(client):
                client.ask({'cmd':'new','players':[{'hand':['Forest'],'library':['Forest']*30},{'hand':['Island'],'library':['Island']*30}]})
                first=client.state
                # A viewer receives only its hand; whole-state delivery is separate.
                seen=client.send({'cmd':'state','viewer':0})
                self.assertNotIn('hand',seen['players'][1])
                self.assertEqual(seen['players'][0]['hand_size'],1)
                for _ in range(4):
                    player=client.state['asked']['player']
                    client.ask({'cmd':'act','player':player,'move':{'kind':'pass'}})
                return first['session_id']
            with ThreadPoolExecutor(max_workers=8) as pool: identities=list(pool.map(run,clients))
            self.assertEqual(len(set(identities)),8)
            wrong={'cmd':'act','session_id':identities[0], 'request_id':clients[1].state['request_id'],
                   'player':clients[1].state['asked']['player'],'move':{'kind':'pass'}}
            self.assertEqual(host.request(clients[1].game,wrong)['error'],'stale session_id')
            clients[1].send({'cmd':'state'})
            other=clients[1].send({'cmd':'state'})
            clients[0].close()
            self.assertEqual(host.request(clients[0].game,{'cmd':'state'})['error'],'no game')
            self.assertEqual(clients[1].send({'cmd':'state'}),other)
            with self.assertRaisesRegex(RuntimeError,'game closed'): clients[0].send({'cmd':'state'})

    def test_failed_setup_keeps_previous_session_and_other_games(self):
        with SharedHost(os.environ.get('MAGIC_ENGINE'),workers=2) as host:
            a,b=host.session(),host.session()
            setup={'cmd':'new','players':[{},{}]}
            a.ask(setup);b.ask(setup)
            a.send({'cmd':'state'});b.send({'cmd':'state'})
            before=a.send({'cmd':'state'});other=b.send({'cmd':'state'})
            for request,reason in [
                ({'cmd':'new','players':[{'hand':['No such card']},{}]}, 'No such card'),
                ({'cmd':'new','players':[{}, {}, {}],'viewer':999}, 'a viewer is a player'),
                ({'cmd':'start','decks':[[], [], []],'viewer':999}, 'a viewer is a player'),
            ]:
                with self.subTest(request=request):
                    rejected=host.request(a.game,request)
                    self.assertFalse(rejected['ok'])
                    self.assertIn(reason,rejected['error'])
                    self.assertEqual(a.send({'cmd':'state'}),before)
                    self.assertEqual(b.send({'cmd':'state'}),other)

    def test_pipelined_same_game_input_order_is_preserved(self):
        proc=subprocess.Popen([os.environ.get('MAGIC_ENGINE') or str(fast_binary('game-serve')),'--host','2'],
            stdin=subprocess.PIPE,stdout=subprocess.PIPE,text=True)
        try:
            self.assertTrue(json.loads(proc.stdout.readline())['host'])
            requests=[{'id':0,'game':4,'request':{'cmd':'new','players':[{},{}]}},
                      {'id':1,'game':4,'request':{'cmd':'state','viewer':0}},
                      {'id':2,'game':4,'request':{'cmd':'state','viewer':1}}]
            proc.stdin.write(''.join(json.dumps(r)+'\n' for r in requests));proc.stdin.flush()
            replies=[json.loads(proc.stdout.readline()) for _ in requests]
            self.assertEqual([r['id'] for r in replies],[0,1,2])
            self.assertEqual([r['state']['viewer'] for r in replies],[None,0,1])
            self.assertEqual(len({r['state']['session_id'] for r in replies}),1)
        finally:
            proc.stdin.close();proc.wait(timeout=10);proc.stdout.close()

    def test_private_look_delivery_is_separate_between_games_and_viewers(self):
        with SharedHost(os.environ.get('MAGIC_ENGINE'),workers=2) as host:
            a,b=host.session(),host.session()
            a.ask({'cmd':'new','players':[{'hand':['Opt'],'battlefield':['Island'],'library':['Forest']*30},{}]})
            b.ask({'cmd':'new','players':[{'library':['Plains']*30},{}]})
            objects = a.state['objects']
            opt = next(int(o) for o, item in objects.items() if item['name'] == 'Opt')
            island = next(int(o) for o, item in objects.items() if item['name'] == 'Island')
            a.act({'kind': 'cast', 'card': opt, 'targets': [], 'mana': {'U': 1},
                   'plan': [{'source': island, 'ability': 0}]})
            for _ in range(2):a.act({'kind':'pass'})
            self.assertEqual(a.state['asked']['kind'],'arrange')
            cards=a.state['asked']['cards']
            self.assertTrue(any(d['kind']=='looked' for d in a.state['observations']))
            other=a.send({'cmd':'state','viewer':1})
            self.assertFalse(other['observations'])
            self.assertFalse(any(str(c) in other['objects'] for c in cards))
            self.assertFalse(b.send({'cmd':'state','viewer':0})['observations'])
            self.assertFalse(a.send({'cmd':'state','viewer':0})['observations'])
