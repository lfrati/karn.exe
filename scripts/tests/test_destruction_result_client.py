"""Full-card checks at the actual Python/JSON boundary (701.7/700.4/608.2h)."""
import subprocess
import os
import sys
import unittest
from pathlib import Path
sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import play

class DestructionResultClientTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        if os.environ.get('GAME_SERVE_REVIEW'):return
        subprocess.run(['cargo','build','--profile','fast','-p','game','--bin','game-serve'], cwd=play.ROOT, check=True, stdout=subprocess.DEVNULL)
    def setUp(self):
        self.engine=play.Engine(audit=True)
        self.addCleanup(self.close)
    def close(self):
        self.engine.proc.stdin.close();self.engine.proc.wait(timeout=10);self.engine.proc.stdout.close()
    def new(self, seats):
        self.engine.ask({'cmd':'new','players':[{'library':['Forest']*10,**seat} for seat in seats]})
        for _ in range(30):
            if self.engine.state['part']=='PrecombatMain':return
            self.engine.act({'kind':'pass'})
        self.fail('did not reach main phase')
    def ids(self,name):
        return [int(k) for k,v in self.engine.state['objects'].items() if v['name']==name]
    def cast(self, name, targets=None, *, mana, spend):
        card = next(o for o in self.engine.state['players'][0]['hand'] if self.engine.state['objects'][str(o)]['name'] == name)
        self.engine.act({'kind': 'cast', 'card': card, 'plan': [{'source': o, 'ability': 0} for o in mana], 'targets': targets or [], 'mana': spend})
        self.engine.act({'kind': 'pass'}); self.engine.act({'kind': 'pass'})
    def life(self):return [p['life'] for p in self.engine.state['players']]
    def test_full_hellfire_zero_result_still_deals_three_and_multiple_deaths_add_to_it(self):
        for creatures,expected in [([],17),(['Silvercoat Lion','Grizzly Bears'],15)]:
            self.new([{'hand':['Hellfire'],'battlefield':['Swamp']*5}, {'battlefield':creatures+['Walking Corpse','Darksteel Myr']}])
            dying=self.ids('Silvercoat Lion')+self.ids('Grizzly Bears')
            self.cast('Hellfire', mana=self.ids('Swamp')[:5], spend={'B': 5})
            if creatures:
                self.engine.act({'kind':'choose','answer':{'objects':dying}})
            self.assertEqual(self.life(),[expected,20])
            names=[self.engine.state['objects'][str(o)]['name'] for o in self.engine.state['battlefield']]
            self.assertIn('Walking Corpse',names);self.assertIn('Darksteel Myr',names)
    def test_full_cinder_uses_effective_power_and_last_controller_of_multicolor_white_target(self):
        self.new([{'hand':['Act of Treason','Giant Growth','Cinder Cloud'], 'battlefield':['Mountain']*8+['Forest']},
                  {'battlefield':['Watchwolf']}])
        target=self.ids('Watchwolf')[0]
        self.cast('Act of Treason',[[{'object':target}]], mana=self.ids('Mountain')[:3], spend={'R': 3})
        self.cast('Giant Growth',[[{'object':target}]], mana=self.ids('Forest')[:1], spend={'G': 1})
        self.cast('Cinder Cloud',[[{'object':target}]], mana=self.ids('Mountain')[3:8], spend={'R': 5})
        self.assertEqual(self.life(),[14,20])
        # The stolen creature's owner receives it, but its former controller receives damage.
        whole=self.engine.send({'cmd':'state'})
        self.assertEqual([whole['objects'][str(o)]['name'] for o in whole['players'][1]['graveyard']],['Watchwolf'])
    def test_full_cinder_replaced_or_indestructible_targets_do_not_damage(self):
        for creature in ["Nissa's Chosen",'Darksteel Myr','Avacyn, Angel of Hope']:
            self.new([{'hand':['Cinder Cloud'],'battlefield':['Mountain']*5}, {'battlefield':[creature]}])
            self.cast('Cinder Cloud',[[{'object':self.ids(creature)[0]}]], mana=self.ids('Mountain')[:5], spend={'R': 5})
            self.assertEqual(self.life(),[20,20])
    def test_full_hellfire_retains_result_until_private_library_order_completes(self):
        self.new([{'hand':['Hellfire'],'battlefield':['Swamp']*5},
                  {'battlefield':["Nissa's Chosen","Nissa's Chosen",'Silvercoat Lion']}])
        chosen=self.ids("Nissa's Chosen")
        self.cast('Hellfire', mana=self.ids('Swamp')[:5], spend={'B': 5})
        self.assertEqual((self.engine.state['asked']['kind'],self.engine.state['asked']['player']),('order_cards',1))
        self.assertEqual(self.life(),[20,20])
        self.engine.act({'kind':'choose','answer':{'objects':list(reversed(chosen))}})
        self.assertEqual(self.life(),[16,20]);self.assertEqual(self.engine.state['asked']['kind'],'priority')

if __name__=='__main__':unittest.main()
