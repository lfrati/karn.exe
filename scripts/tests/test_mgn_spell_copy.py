"""CR 707.2b/707.10, 715.3b/c: copied Adventure spells retain their spell face."""
from copy import deepcopy
import unittest

from mgn.examples import EXAMPLES, load
from mgn.oracle import Oracle
from mgn.state import State, zone_names

ORACLE = Oracle()
BONECRUSHER = ORACLE.by_name('Bonecrusher Giant // Stomp')


class SpellCopyBasisTests(unittest.TestCase):
    def record(self, copy_present=False):
        record = load(EXAMPLES / 'priority-and-steps.json')
        record['tier'] = 2
        record['cards'] = {'giant': {'owner': 'A', 'oracle': BONECRUSHER,
                                    'name': ORACLE.name(BONECRUSHER)}}
        zones = {zone: [] for zone in zone_names(['A', 'B'])}
        zones['stack'] = ['giant']
        objects = {'giant': {'face': 1, 'controller': 'A'}}
        if copy_present:
            zones['stack'].append('S1')
            objects['S1'] = {'kind': 'copy', 'copy_of': 'giant', 'controller': 'A'}
        record['start'] = {'kind': 'position', 'state': {
            'turn': {'number': 3, 'step': 'precombat_main', 'active': 'A'},
            'pending': {'seat': 'A', 'kind': 'priority'}, 'result': {'kind': 'unfinished'},
            'players': {'A': {'life': 20}, 'B': {'life': 20}},
            'zones': zones, 'objects': objects}}
        record['transitions'] = []
        record.pop('checkpoints', None)
        return record

    def copy(self, state, errors, object_id='S1', source='giant'):
        state.apply({'objects': {object_id: {'zone': 'stack', 'kind': 'copy',
                     'copy_of': source, 'controller': 'A'}}}, [], '/copy', errors)

    def assert_stomp(self, state, object_id):
        # Pinned Stomp is a red instant, never the creature Giant face (715.3b).
        self.assertEqual(state._printed(object_id, ORACLE)['types'], ['instant'])
        self.assertEqual(state._printed(object_id, ORACLE)['colors'], ['red'])
        self.assertEqual(state.copy_bases[object_id], (BONECRUSHER, 1))
        self.assertNotIn('power', state._printed(object_id, ORACLE))

    def test_adventure_copy_survives_source_resolution_and_recast(self):
        for destination in ('exile', 'A.hand'):
            errors = []
            state = State.from_record(self.record(), errors)
            self.copy(state, errors)
            state.apply({'objects': {'giant': {'zone': destination}}}, [], '/resolve', errors)
            self.assertEqual(state._printed('giant', ORACLE)['types'], ['creature'])
            self.assert_stomp(state, 'S1')
            state.apply({'objects': {'giant': {'zone': 'stack', 'controller': 'A'}}}, [], '/recast', errors)
            self.assert_stomp(state, 'S1')
            self.assertEqual(errors, [])

    def test_copy_of_copy_and_observation_keep_original_incarnation(self):
        record = self.record(copy_present=True)
        errors = []
        previous = State.from_record(record, errors)
        observed = deepcopy(record)
        observed['start']['state']['zones']['stack'] = ['S1']
        observed['start']['state']['zones']['exile'] = ['giant']
        observed['start']['state']['objects'].pop('giant')
        state = State.from_observation(observed, errors, previous)
        self.copy(state, errors, 'S2', 'S1')
        previous = deepcopy(state)
        state.apply({'objects': {'S1': None}}, [], '/ceased', errors)
        self.assert_stomp(state, 'S2')
        observed['start']['state'] = state.explicit(ORACLE)
        restored = State.from_observation(observed, errors, previous)
        self.assert_stomp(restored, 'S2')
        self.assertEqual(errors, [])
        errors = []
        State.from_record(observed, errors)
        self.assertTrue(any(p.code == 'unsupported' for p in errors))

    def test_changed_source_without_history_is_refused(self):
        record = self.record(copy_present=True)
        record['start']['state']['zones']['stack'] = ['S1']
        record['start']['state']['zones']['exile'] = ['giant']
        record['start']['state']['objects'].pop('giant')
        errors = []
        state = State.from_record(record, errors)
        self.assertIsNone(state._printed('S1', ORACLE))
        self.assertTrue(any(p.code == 'unsupported' for p in errors))

    def test_new_copy_with_folded_donor_change_is_refused(self):
        errors = []
        state = State.from_record(self.record(), errors)
        state.apply({'objects': {'giant': {'zone': 'exile'},
                     'S1': {'zone': 'stack', 'kind': 'copy', 'copy_of': 'giant',
                            'controller': 'A'}}}, [], '/folded', errors)
        self.assertIsNone(state._printed('S1', ORACLE))
        self.assertTrue(any(p.code == 'unsupported' for p in errors))
        # Also refuse when the donor ends on the stack again with its normal face:
        # the final boundary cannot establish which incarnation was copied.
        record = self.record()
        previous = State.from_record(record, [])
        observed = self.record(copy_present=True)
        observed['start']['state']['objects']['giant']['face'] = 0
        errors = []
        state = State.from_observation(observed, errors, previous)
        self.assertTrue(any(p.code == 'unsupported' for p in errors))
        self.assertIsNone(state._printed('S1', ORACLE))


if __name__ == '__main__':
    unittest.main()
