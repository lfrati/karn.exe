"""Ordinary scenario authoring: CR 400.3, 401.2, 404.2, 302.6; no engine."""
from unittest.mock import patch
import unittest

from mgn.oracle import Oracle
from mgn.render import parse, render
from mgn.scenario import PRIORITY_STEPS, Scenario
from mgn.validate import validate


class ScenarioTests(unittest.TestCase):
    def test_ordered_zones_and_battlefield_facts_survive_normal_mgn_roundtrip(self):
        scenario = Scenario(turn=4, active='B', step='draw', priority='A')
        scenario.player('A', life=17, mana={'G': 1})
        for label, name, zone in [('top', 'Forest', 'library'), ('bottom', 'Island', 'library'),
                                  ('old', 'Swamp', 'graveyard'), ('new', 'Plains', 'graveyard'),
                                  ('spell', 'Giant Growth', 'hand'), ('away', 'Mountain', 'exile')]:
            scenario.card(label, name, owner='A', zone=zone)
        counters = {'+1/+1': 1}
        expected = {'power': 4, 'toughness': 4}  # CR 122.1a; no helper inference.
        scenario.card('borrowed', 'Hill Giant', owner='B', zone='battlefield', controller='A',
                      tapped=True, counters=counters, damage=1, summoning_sick=True, expected=expected)
        counters['+1/+1'] = 99
        expected['power'] = 99
        result = scenario.build()
        self.assertEqual(result['start']['state']['zones']['A.library'], ['top', 'bottom'])
        self.assertEqual(result['start']['state']['zones']['A.graveyard'], ['old', 'new'])
        self.assertEqual(result['start']['state']['objects']['borrowed'],
                         {'controller': 'A', 'tapped': True, 'counters': {'+1/+1': 1}, 'damage': 1,
                          'power': 4, 'toughness': 4})
        self.assertEqual(result['start']['assumptions'], {'summoning_sick': ['borrowed']})
        self.assertEqual(result['cards']['borrowed']['owner'], 'B')
        self.assertEqual(parse(render(result)), result)
        self.assertEqual([p for p in validate(result, Oracle()) if p.severity == 'error'], [])

    def test_supported_priority_steps_emit_the_requested_boundary(self):
        for step in PRIORITY_STEPS:
            result = Scenario(step=step).build()
            self.assertEqual(result['start']['state']['turn']['step'], step)
            self.assertEqual(result['start']['state']['pending'], {'kind': 'priority', 'seat': 'A'})
            self.assertEqual(result['start']['state']['zones']['stack'], [])
            self.assertNotIn('causal', result['start'])

    def test_no_ambiguous_name_duplicate_label_or_wrong_owner_zone(self):
        scenario = Scenario()
        with self.assertRaisesRegex(ValueError, 'exactly one pinned'):
            scenario.card('missing', 'Not a pinned card', owner='A', zone='hand')
        with patch.object(Oracle, 'by_name', side_effect=KeyError('ambiguous')):
            with self.assertRaisesRegex(ValueError, 'exactly one pinned'):
                scenario.card('ambiguous', 'Forest', owner='A', zone='hand')
        scenario.card('land', 'Forest', owner='A', zone='hand')
        with self.assertRaisesRegex(ValueError, 'duplicate'):
            scenario.card('land', 'Forest', owner='A', zone='library')
        with self.assertRaisesRegex(ValueError, 'owner'):
            scenario.card('wrong', 'Forest', owner='A', zone='B.hand')
        self.assertEqual(set(scenario.build()['cards']), {'land'})

    def test_limits_and_invalid_facts_are_refused_without_a_runtime(self):
        for kwargs in ({'step': 'declare_attackers'}, {'step': 'untap'}, {'turn': 3, 'active': 'B'}, {'turn': 1, 'step': 'draw'}):
            with self.assertRaises(ValueError):
                Scenario(**kwargs)
        scenario = Scenario()
        with self.assertRaises(ValueError):
            scenario.card('stack_spell', 'Giant Growth', owner='A', zone='stack')
        with self.assertRaises(ValueError):
            scenario.card('grave', 'Forest', owner='A', zone='graveyard', tapped=True)
        scenario.card('bear', 'Grizzly Bears', owner='A', zone='battlefield', damage=-1)
        with self.assertRaisesRegex(ValueError, 'invalid MGN'):
            scenario.build()

    def test_build_does_not_alias_mutable_inputs_or_previous_output(self):
        scenario = Scenario().card('land', 'Forest', owner='A', zone='battlefield')
        mana = {'G': 1}
        scenario.player('A', mana=mana)
        mana['G'] = 9
        first = scenario.build()
        first['cards']['land']['name'] = 'changed'
        first['start']['state']['zones']['battlefield'].clear()
        second = scenario.build()
        self.assertEqual(second['cards']['land']['name'], 'Forest')
        self.assertEqual(second['start']['state']['zones']['battlefield'], ['land'])
        self.assertEqual(second['start']['state']['players']['A']['mana'], {'G': 1})
