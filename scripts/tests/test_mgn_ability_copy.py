"""CR 405.4, 707.10b: copying an ability does not create a spell or its characteristics."""
from copy import deepcopy
import unittest

from mgn.examples import EXAMPLES, load
from mgn.oracle import Oracle
from mgn.state import State, zone_names


class AbilityCopyTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.oracle = Oracle()

    def premise(self):
        record = load(EXAMPLES / 'priority-and-steps.json')
        oid = self.oracle.by_name('Prodigal Pyromancer')
        record['tier'] = 2
        record['cards'] = {'mage': {'owner': 'A', 'oracle': oid,
                                   'name': self.oracle.name(oid)}}
        record['transitions'] = []
        record.pop('checkpoints', None)
        zones = {zone: [] for zone in zone_names(['A', 'B'])}
        zones.update(battlefield=['mage'], stack=['S1'])
        record['start'] = {'kind': 'position', 'state': {
            'turn': {'number': 3, 'step': 'precombat_main', 'active': 'A'},
            'pending': {'seat': 'A', 'kind': 'priority'}, 'result': {'kind': 'unfinished'},
            'players': {'A': {'life': 20}, 'B': {'life': 20}}, 'zones': zones,
            'objects': {'S1': {'kind': 'ability', 'source': 'mage',
                'ability': {'face': 0, 'paragraph': 1}, 'controller': 'A', 'targets': [['B']]}}}}
        return record

    def test_ability_copy_and_copy_of_copy_survive_original_and_source_departure(self):
        record = self.premise()
        errors = []
        state = State.from_record(record, errors)
        for number in (2, 3):
            state.apply({'objects': {f'S{number}': {'zone': 'stack', 'kind': 'copy',
                'copy_of': f'S{number-1}', 'controller': 'B', 'targets': [['A']]}}}, [], '/copy', errors)
        previous = deepcopy(state)
        state.apply({'objects': {'S1': None, 'S2': None, 'mage': {'zone': 'A.graveyard'}}}, [], '/leave', errors)
        observed = deepcopy(record)
        observed['start']['state'] = state.explicit(self.oracle)
        restored = State.from_observation(observed, errors, previous)
        self.assertEqual(errors, [])
        self.assertEqual(restored.explicit(self.oracle), state.explicit(self.oracle))
        self.assertEqual(state.fact('S3', 'targets'), [['A']])
        self.assertIsNone(state.owner_of('S3'))
        self.assertIsNone(state._printed('S3', self.oracle))
        facts = state.explicit(self.oracle)['objects']['S3']
        self.assertEqual(facts['controller'], 'B')
        for key in ('owner', 'types', 'colors', 'power', 'toughness', 'counters', 'granted'):
            self.assertNotIn(key, facts)

    def test_ability_copy_rejects_card_characteristics_and_ownership(self):
        for extra in ({'owner': 'A'}, {'types': ['creature']}, {'power': 1}):
            state = State.from_record(self.premise(), [])
            errors = []
            state.apply({'objects': {'S2': {'zone': 'stack', 'kind': 'copy', 'copy_of': 'S1',
                         'controller': 'A', **extra}}}, [], '/copy', errors)
            self.assertTrue(any(p.code == 'delta' for p in errors), errors)

    def test_unseen_original_does_not_become_an_ability_by_inference(self):
        state = State.from_record(self.premise(), [])
        errors = []
        state.apply({'objects': {'S2': {'zone': 'stack', 'kind': 'copy',
                     'copy_of': 'mage', 'controller': 'A'}}}, [], '/copy', errors)
        self.assertTrue(any(p.code == 'unsupported' for p in errors))
