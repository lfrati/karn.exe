"""Engine-independent setup readback, MGN causal-start contract and L1 construction.

CR 611.2/613: two resolved +3/+3 effects have separate enduring occurrences
and lifetimes; the same visible 8/8 is no proof those causes were constructed.
The fake adapter deliberately supplies the correct visible state in every case.
"""
from copy import deepcopy
import unittest
from unittest.mock import patch

import mgn.runner as runner
from mgn.schema import validate as schema_validate
from mgn.state import State
from mgn.validate import validate
from test_mgn_causal import ORACLE, record


class CausalSetupRunnerTests(unittest.TestCase):
    def sample(self):
        sample=record()
        cause=sample['start']['causal']
        # Each Giant Growth resolves once and creates its own single +3/+3.
        sample['cards']['spell2']=deepcopy(sample['cards']['spell'])
        sample['start']['state']['zones']['A.graveyard'].append('spell2')
        cause['objects']['old_spell2']=deepcopy(cause['objects']['old_spell'])
        cause['objects']['old_spell2']['card']='spell2'
        producer=deepcopy(cause['producers'][0])
        producer.update(id='resolution2',source='old_spell2')
        cause['producers'].append(producer)
        cause['casts_this_turn'].append({'source':'old_spell2','player':'A'})
        sample['start']['assumptions']['spells_cast']['A']=2
        second=deepcopy(cause['continuous_effects'][0])
        second.update(id='growth2',producer='resolution2')
        cause['continuous_effects'].append(second)
        cause['timestamp_order']=[{'object':'bear'},{'effect':'growth'},{'effect':'growth2'}]
        sample['start']['state']['objects']['bear'].update(power=8,toughness=8)
        sample['cards']['land']=deepcopy(sample['cards']['draw']);sample['cards']['land']['owner']='A'
        sample['start']['state']['zones']['A.hand']=['land']
        sample['transitions']=[{'seat':'A','do':{'kind':'play_land','card':'land'},'delta':{'objects':{'land':{'zone':'battlefield'}}}}]
        self.assertEqual([str(p) for p in validate(sample,ORACLE) if p.severity=='error'],[])
        return sample

    def run_fake(self, sample, mutate=lambda ok:None):
        initial=State.from_record(sample,[])
        state=initial.explicit(ORACLE)
        after=deepcopy(initial)
        after.apply(sample["transitions"][0]["delta"],[],"/delta",[])
        following=after.explicit(ORACLE)
        ok={'state':state}
        if 'causal' in sample['start']:
            ok['causal']=deepcopy(sample['start']['causal'])
        mutate(ok)
        calls=[]
        class FakeAdapter:
            def __init__(self,*args):pass
            def request(self,body,*args):
                calls.append(deepcopy(body))
                if 'hello' in body:
                    return {'id':1,'capabilities':{'engine':{'name':'foreign','version':'test'},
                        'adapter':{'name':'fake','version':'test'},'tiers':[2]}}
                if 'setup' in body:return {'id':2,'ok':deepcopy(ok)}
                if 'act' in body:return {'id':3,'ok':{'state':deepcopy(following)}}
                if 'end' in body:return {'id':4,'ok':{}}
                raise AssertionError(body)
            def close(self):pass
        with patch.object(runner,'Adapter',FakeAdapter):
            report=runner.Run(sample,ORACLE,['unused-fake-adapter']).run()
        self.assertEqual([str(e) for e in schema_validate(report,'divergence-report.schema.json')],[])
        return report,calls

    def test_exact_readback_reaches_the_real_run_action_api(self):
        sample=self.sample()
        report,calls=self.run_fake(sample)
        self.assertEqual({k:report['levels'][k] for k in ('L0','L1','L2','L3','L4')},
            dict.fromkeys(('L0','L1','L2','L3','L4'),'pass'))
        self.assertEqual(report['replayed'],1)
        self.assertIsNone(report['stopped_at'])
        self.assertEqual(len([c for c in calls if 'act' in c]),1)

    def test_missing_required_readback_stops_as_unsupported_instead_of_visible_state_pass(self):
        report,calls=self.run_fake(self.sample(),lambda ok:ok.pop('causal'))
        self.assertEqual(report['stopped_at'],0)
        self.assertEqual(report['replayed'],0)
        self.assertEqual(report['refusals'],[{'after':0,'class':'unsupported','code':'unsupported_start',
            'message':'required start.causal has no independently read-back setup causal payload'}])
        self.assertEqual(report['levels']['L1'],'inconclusive')
        self.assertFalse(any('act' in c for c in calls))

    def test_malformed_readback_is_a_protocol_fault(self):
        for payload in (None,[],{}, {'objects':{},'producers':[],'continuous_effects':[],'delayed_triggers':'bad'}):
            with self.subTest(payload=payload):
                report,calls=self.run_fake(self.sample(),lambda ok:ok.update(causal=payload))
                self.assertEqual(report['stopped_at'],0)
                self.assertEqual([(r['class'],r['code']) for r in report['refusals']],[('fault','protocol')])
                self.assertIn('malformed causal setup reply',report['refusals'][0]['message'])
                self.assertEqual(report['divergences'],[])
                self.assertFalse(any('act' in c for c in calls))

    def test_changed_causes_diverge_at_setup_despite_identical_visible_characteristics(self):
        changes=[
            (lambda c:c['producers'][0].update(source='bear'),'start.causal.producers.0.source'),
            (lambda c:c['continuous_effects'][0].update(subjects=[{'object':'draw'}]),'start.causal.continuous_effects.0.subjects.0.object'),
            (lambda c:c['continuous_effects'][0]['duration'].update(turn=4),'start.causal.continuous_effects.0.duration.turn'),
            (lambda c:c['continuous_effects'].append(deepcopy(c['continuous_effects'][0])),'start.causal.continuous_effects'),
            (lambda c:c['continuous_effects'].pop(),'start.causal.continuous_effects'),
            (lambda c:c['continuous_effects'].reverse(),'start.causal.continuous_effects.0.id'),
            (lambda c:c['timestamp_order'].reverse(),'start.causal.timestamp_order.0.object'),
        ]
        for mutate,path in changes:
            with self.subTest(path=path):
                report,calls=self.run_fake(self.sample(),lambda ok:mutate(ok['causal']))
                self.assertEqual(report['stopped_at'],0)
                self.assertEqual(report['levels']['L1'],'fail')
                self.assertEqual(report['refusals'],[])
                self.assertTrue(any(d['after']==0 and d['level']=='L1' and d['path']==path for d in report['divergences']))
                self.assertFalse(any('act' in c for c in calls))

    def test_undeclared_optional_history_category_is_not_an_empty_history_claim(self):
        sample=self.sample();sample['start']['causal'].pop('casts_this_turn')
        sample['start'].pop('assumptions')
        report,_=self.run_fake(sample,lambda ok:ok['causal'].update(casts_this_turn=[]))
        self.assertEqual(report['levels']['L1'],'pass')
        self.assertEqual(report['replayed'],1)

    def test_ordinary_start_does_not_require_causal_export(self):
        sample=self.sample();sample['start'].pop('causal');sample['start'].pop('assumptions')
        sample['start']['state']['turn']['step']='precombat_main'
        sample['start']['state']['objects']['bear'].update(power=2,toughness=2)
        report,_=self.run_fake(sample)
        self.assertEqual(report['levels']['L1'],'pass')
        self.assertEqual(report['replayed'],1)
