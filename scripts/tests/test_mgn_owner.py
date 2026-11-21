"""CR 111.2/112.2/114.2: control changes do not change generated ownership."""
from copy import deepcopy
import unittest

from mgn.delta import diff_states
from mgn.examples import EXAMPLES, load
from mgn.schema import Validator, load_schema, validate as schema_validate
from mgn.state import State
from tests.test_mgn_history import HistoricalSourceTests


class GeneratedOwnerTests(unittest.TestCase):
    def record(self):
        record = load(EXAMPLES / 'priority-and-steps.json')
        position = HistoricalSourceTests().position()
        record.update({key: position[key] for key in ('tier', 'cards', 'start')})
        record['transitions'] = []
        record.pop('checkpoints', None)
        return record

    def test_explicit_position_owner_passes_schema_and_roundtrip(self):
        record = self.record()
        record['start']['state']['objects']['T1']['controller'] = 'B'
        errors = []
        state = State.from_record(record, errors)
        record['start']['state'] = state.explicit()
        # Explicit projection currently includes the separate implicit occurrence default.
        record['start']['state']['turn'].pop('occurrence', None)
        self.assertEqual(schema_validate(record), [])
        restored = State.from_record(record, errors)
        self.assertEqual(restored.owner_of('T1'), 'A')
        self.assertEqual(restored.fact('T1', 'controller'), 'B')
        self.assertEqual(errors, [])

    def test_control_change_and_observation_keep_initial_owner(self):
        record = self.record()
        del record['start']['state']['objects']['T1']['owner']
        state = State.from_record(record, [])
        errors = []
        state.apply({'objects': {'T1': {'controller': 'B'}}}, [], '/change', errors)
        self.assertEqual(state.owner_of('T1'), 'A')
        observed = deepcopy(record)
        observed['start']['state']['objects']['T1']['controller'] = 'B'
        restored = State.from_observation(observed, errors, state)
        self.assertEqual(restored.owner_of('T1'), 'A')
        self.assertEqual(errors, [])
        restored.apply({'objects': {'T1': {'owner': 'B'}}}, [], '/wrong', errors)
        self.assertTrue(any('cannot change' in p.message for p in errors))
        self.assertEqual(restored.owner_of('T1'), 'A')
        observed['start']['state']['objects']['T1']['owner'] = 'B'
        errors = []
        State.from_observation(observed, errors, state)
        self.assertTrue(any('cannot change' in p.message for p in errors))

    def test_owner_is_a_game_seat_and_cards_use_table(self):
        record = self.record()
        record['start']['state']['objects']['T1']['owner'] = 'C'
        self.assertTrue(any(p.code == 'unknown_reference' for p in self.load_errors(record)))
        record = load(EXAMPLES / 'priority-and-steps.json')
        state = State.from_record(record, [])
        card = next(iter(state.cards))
        errors = []
        state._set_object(card, 'owner', state.cards[card]['owner'], '/owner', errors)
        self.assertTrue(any('card table' in p.message for p in errors))

    def load_errors(self, record):
        errors = []
        State.from_record(record, errors)
        return errors

    def test_new_generated_delta_preserves_captured_owner(self):
        before_record = self.record()
        before_record['start']['state']['zones']['battlefield'] = []
        before_record['start']['state']['objects'] = {}
        before = State.from_record(before_record, [])
        after_record = self.record()
        after_record['start']['state']['objects']['T1']['controller'] = 'B'
        after = State.from_record(after_record, [])
        delta = diff_states(before, after, None)
        self.assertEqual(delta['objects']['T1']['owner'], 'A')
        schema = load_schema('mgn-1.schema.json')
        self.assertEqual(Validator(schema).errors(delta['objects']['T1'],
                         schema['$defs']['object_facts_with_zone']), [])
        errors = []
        before.apply(delta, [], '/new', errors)
        self.assertEqual(before.explicit(), after.explicit())
        self.assertEqual(errors, [])

    def test_stack_ability_has_controller_without_owner(self):
        record = self.record()
        record['start']['state']['zones']['stack'] = ['S1']
        ability = {'kind': 'ability', 'source': 'T1',
                   'ability': {'face': 0, 'paragraph': 1}, 'controller': 'A'}
        record['start']['state']['objects']['S1'] = ability
        errors = []
        state = State.from_record(record, errors)
        self.assertEqual(errors, [])
        self.assertNotIn('owner', state.explicit()['objects']['S1'])
        restored = State.from_observation(record, errors, state)
        self.assertNotIn('owner', restored.explicit()['objects']['S1'])
        self.assertEqual(errors, [])
        # Reject owner regardless of input object member order.
        for facts in ({'owner': 'A', **ability}, {**ability, 'owner': 'A'}):
            record['start']['state']['objects']['S1'] = facts
            self.assertTrue(any('not an owner' in p.message for p in self.load_errors(record)))
