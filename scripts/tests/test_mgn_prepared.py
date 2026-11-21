"""CR 722.3c/d, 707.12: preparation creates an alternative-characteristic copy in exile."""
from copy import deepcopy
import unittest

from mgn.delta import diff_states
from mgn.examples import EXAMPLES, load
from mgn.oracle import Oracle
from mgn.state import State, zone_names
from mgn.validate import validate, _check_prepared_copies

ORACLE = Oracle()
GLASSWRIGHT = '23c3de07-8aa2-48e9-848c-50b5a40bd133'


class PreparedCopyTests(unittest.TestCase):
    def record(self, tier=2):
        record = load(EXAMPLES / 'priority-and-steps.json')
        record['tier'] = tier
        record['cards'] = {'goblin': {'owner': 'A', 'oracle': GLASSWRIGHT,
                                     'name': ORACLE.name(GLASSWRIGHT)}}
        record['transitions'] = []
        record.pop('checkpoints', None)
        zones = {zone: [] for zone in zone_names(['A', 'B'])}
        zones.update(battlefield=['goblin'], exile=['S1'])
        record['start'] = {'kind': 'position', 'state': {
            'turn': {'number': 3, 'step': 'precombat_main', 'active': 'A'},
            'pending': {'seat': 'A', 'kind': 'priority'}, 'result': {'kind': 'unfinished'},
            'players': {'A': {'life': 20}, 'B': {'life': 20}}, 'zones': zones,
            'objects': {'S1': {'kind': 'copy', 'copy_of': 'goblin', 'copy_face': 1, 'controller': 'A'}}}}
        if tier == 2:
            record['start']['state']['objects']['goblin'] = {'designations': ['prepared']}
        return record

    def test_prepared_copy_uses_sorcery_face_and_survives_cast_roundtrip(self):
        # Glasswright enters prepared; its copy has only Craft with Pride's
        # characteristics, which are now normal characteristics (722.3c).
        for tier in (1, 2):
            record = self.record(tier)
            self.assertEqual(validate(record, ORACLE), [])
            errors = []
            state = State.from_record(record, errors)
            self.assertEqual(state._printed('goblin', ORACLE)['types'], ['creature'])
            self.assertEqual(state._printed('S1', ORACLE)['types'], ['sorcery'])
            explicit = state.explicit(ORACLE)
            self.assertEqual(explicit['objects']['S1']['copy_face'], 1)
            self.assertEqual(explicit['objects']['S1']['copy_of'], 'goblin')
            observed = deepcopy(record)
            observed['start']['state'] = explicit
            restored = State.from_record(observed, errors)
            self.assertEqual(restored.explicit(ORACLE), explicit)
            before = deepcopy(state)
            state.apply({'objects': {'S1': {'zone': 'stack'}, **({'goblin': {'designations': []}} if tier == 2 else {})}}, [], '/cast', errors)
            self.assertEqual(state._printed('S1', ORACLE)['types'], ['sorcery'])
            delta = diff_states(before, state, ORACLE)
            before.apply(delta, [], '/roundtrip', errors)
            self.assertEqual(before.explicit(ORACLE), state.explicit(ORACLE))
            # A copied prepare spell remains one (722.3d), without another face override.
            state.apply({'objects': {'S2': {'zone': 'stack', 'kind': 'copy',
                         'copy_of': 'S1', 'controller': 'A'}}}, [], '/copy', errors)
            self.assertEqual(state._printed('S2', ORACLE)['types'], ['sorcery'])
            before_ceased = deepcopy(state)
            state.apply({'objects': {'S1': None}}, [], '/resolved', errors)
            self.assertEqual(state._printed('S2', ORACLE)['types'], ['sorcery'])
            self.assertNotIn('S1', state.explicit(ORACLE)['objects'])
            observed['start']['state'] = state.explicit(ORACLE)
            restored = State.from_observation(observed, errors, before_ceased)
            self.assertEqual(restored.explicit(ORACLE), state.explicit(ORACLE))
            self.assertEqual(restored._printed('S2', ORACLE)['types'], ['sorcery'])
            self.assertEqual(errors, [])

    def test_direct_printed_source_entering_prepared_creates_copy_at_same_boundary(self):
        # CR 722.3c: Glasswright enters prepared, so its controller immediately
        # creates Craft with Pride in exile. This is not an ambiguous ordinary
        # spell copy of the earlier creature spell or hand incarnation.
        record = self.record()
        zones = record['start']['state']['zones']
        zones['battlefield'] = []
        zones['exile'] = []
        zones['A.hand'] = ['goblin']
        record['start']['state']['objects'] = {}
        errors = []
        before = State.from_record(record, errors)
        delta = {'objects': {'goblin': {'zone': 'battlefield', 'designations': ['prepared']},
                            'S1': {'zone': 'exile', 'kind': 'copy', 'copy_of': 'goblin',
                                   'copy_face': 1, 'controller': 'A'}}}
        state = deepcopy(before)
        state.apply(delta, [], '/enters-prepared', errors)
        _check_prepared_copies(state, ORACLE, '/enters-prepared', errors)
        self.assertEqual(state._printed('S1', ORACLE)['types'], ['sorcery'])
        self.assertEqual(state.copy_bases['S1'], (GLASSWRIGHT, 1))
        observed = deepcopy(record)
        observed['start']['state'] = state.explicit(ORACLE)
        restored = State.from_observation(observed, errors, before)
        _check_prepared_copies(restored, ORACLE, '/observed', errors)
        self.assertEqual(restored._printed('S1', ORACLE)['types'], ['sorcery'])
        self.assertEqual(errors, [])

    def test_nonprepared_copy_and_ability_cannot_remain_in_exile(self):
        record = self.record()
        del record['start']['state']['objects']['S1']['copy_face']
        self.assertTrue(any(p.code == 'delta' for p in validate(record, ORACLE)))
        state = State.from_record(self.record(), [])
        errors = []
        state.apply({'objects': {'S2': {'zone': 'exile', 'kind': 'copy',
                     'copy_of': 'goblin', 'controller': 'A'}}}, [], '/ordinary', errors)
        self.assertTrue(any(p.code == 'delta' for p in errors))
        record['start']['state']['objects']['S1'] = {
            'kind': 'ability', 'source': 'goblin', 'ability': {'face': 0, 'paragraph': 1}, 'controller': 'A'}
        self.assertTrue(any(p.code == 'delta' for p in validate(record, ORACLE)))

    def test_copy_face_bounds_and_layout_are_not_clamped(self):
        for face in (0, 2):
            record = self.record()
            record['start']['state']['objects']['S1']['copy_face'] = face
            self.assertTrue(any(p.code == 'schema' for p in validate(record, ORACLE)))
        record = self.record()
        record['start']['state']['objects']['S1']['face'] = 1
        self.assertTrue(any(p.code == 'delta' for p in validate(record, ORACLE)))
        record = self.record()
        plains = ORACLE.by_name('Plains')
        record['cards']['goblin'].update(oracle=plains, name='Plains')
        self.assertTrue(any(p.code == 'oracle' for p in validate(record, ORACLE)))
        state = State.from_record(record, [])
        self.assertIsNone(state._printed('S1', ORACLE))
        other = next(oid for oid, faces in ORACLE.faces.items()
                     if len(faces) > 1 and faces[0].raw.get('layout') != 'prepare')
        record['cards']['goblin'].update(oracle=other, name=ORACLE.name(other))
        self.assertTrue(any(p.code == 'oracle' for p in validate(record, ORACLE)))

    def test_source_copy_lineage_takes_precedence_over_own_printing(self):
        record = self.record()
        plains = ORACLE.by_name('Plains')
        record['cards']['clone'] = {'owner': 'A', 'oracle': plains, 'name': 'Plains'}
        record['start']['state']['zones']['battlefield'].append('clone')
        objects = record['start']['state']['objects']
        objects['clone'] = {'copy_of': 'goblin'}
        objects['S1']['copy_of'] = 'clone'
        self.assertEqual(validate(record, ORACLE), [])
        state = State.from_record(record, [])
        self.assertEqual(state._printed('S1', ORACLE)['types'], ['sorcery'])

    def clone_record(self):
        record = self.record()
        plains = ORACLE.by_name('Plains')
        record['cards']['clone'] = {'owner': 'A', 'oracle': plains, 'name': 'Plains'}
        record['start']['state']['zones']['battlefield'].append('clone')
        record['start']['state']['objects']['clone'] = {'copy_of': 'goblin'}
        record['start']['state']['objects']['S1']['copy_of'] = 'clone'
        return record

    def test_basis_is_frozen_at_creation_before_any_printed_lookup(self):
        # 707.2b: changing the original cannot change a copy already made.
        # 400.7: the physical Plains leaving the battlefield loses its copy effect.
        for donor_delta in ({'zone': 'A.graveyard'}, {'copy_of': None}):
            record = self.clone_record()
            del record['start']['state']['objects']['S1']
            record['start']['state']['zones']['exile'] = []
            errors = []
            state = State.from_record(record, errors)
            state.apply({'objects': {'S1': {'zone': 'exile', 'kind': 'copy',
                         'copy_of': 'clone', 'copy_face': 1, 'controller': 'A'}}},
                        [], '/prepare', errors)
            state.apply({'objects': {'S1': {'zone': 'stack'}, 'clone': donor_delta}},
                        [], '/cast-and-change-donor', errors)
            _check_prepared_copies(state, ORACLE, '/cast', errors)
            self.assertEqual(state._printed('S1', ORACLE), ORACLE.printed(GLASSWRIGHT, 1))
            self.assertEqual(state.explicit(ORACLE)['objects']['S1']['types'], ['sorcery'])
            self.assertEqual(errors, [])

    def test_observation_preserves_basis_across_donor_change(self):
        record = self.clone_record()
        errors = []
        previous = State.from_record(record, errors)
        # An independently supplied later observation changes the donor without
        # first applying that change to previous (the observation-reader path).
        observed = deepcopy(record)
        given = observed['start']['state']
        given['zones']['battlefield'].remove('clone')
        given['zones']['A.graveyard'] = ['clone']
        given['objects'].pop('clone')
        given['zones']['exile'] = []
        given['zones']['stack'] = ['S1']
        restored = State.from_observation(observed, errors, previous)
        _check_prepared_copies(restored, ORACLE, '/observed', errors)
        self.assertEqual(restored._printed('S1', ORACLE), ORACLE.printed(GLASSWRIGHT, 1))
        self.assertEqual(errors, [])
        # Without previous context the initial premise has no complete prepare
        # donor. Section 2.4 cannot reconstruct the missing historical basis.
        self.assertTrue(any(p.code == 'oracle' for p in validate(observed, ORACLE)))

    def test_spell_copy_keeps_basis_after_donor_change_and_original_ceases(self):
        # 722.3d and 707.2b: copying the prepare spell copies its now-normal
        # characteristics, independent of both its original and earlier donor.
        record = self.clone_record()
        errors = []
        state = State.from_record(record, errors)
        state.apply({'objects': {'S1': {'zone': 'stack'}}}, [], '/cast', errors)
        state.apply({'objects': {'clone': {'zone': 'A.graveyard'}}}, [], '/donor-left', errors)
        state.apply({'objects': {'S2': {'zone': 'stack', 'kind': 'copy',
                     'copy_of': 'S1', 'controller': 'A'}}}, [], '/copy', errors)
        previous = deepcopy(state)
        state.apply({'objects': {'S1': None}}, [], '/ceased', errors)
        self.assertEqual(state._printed('S2', ORACLE), ORACLE.printed(GLASSWRIGHT, 1))
        observed = deepcopy(record)
        observed['start']['state'] = state.explicit(ORACLE)
        restored = State.from_observation(observed, errors, previous)
        self.assertEqual(restored._printed('S2', ORACLE), ORACLE.printed(GLASSWRIGHT, 1))
        self.assertEqual(restored.explicit(ORACLE), state.explicit(ORACLE))
        self.assertEqual(errors, [])

    def test_arbitrary_token_without_alternative_definition_is_refused(self):
        record = self.record()
        record['cards'] = {}
        record['start']['state']['zones']['battlefield'] = ['T1']
        objects = record['start']['state']['objects']
        del objects['goblin']
        objects['T1'] = {'controller': 'A', 'token': {
            'name': 'Prepared impostor', 'types': ['creature'], 'colors': [], 'power': 2, 'toughness': 2}}
        objects['S1']['copy_of'] = 'T1'
        self.assertTrue(any(p.code == 'oracle' for p in validate(record, ORACLE)))
        # A token actually copying the complete preparation card has the face definition.
        record['cards'] = {'goblin': self.record()['cards']['goblin']}
        record['start']['state']['zones']['A.hand'] = ['goblin']
        objects['T1']['copy_of'] = 'goblin'
        self.assertEqual(validate(record, ORACLE), [])

    def test_new_exile_copy_consumes_the_shared_s_sequence(self):
        record = self.record()
        state = State.from_record(record, [])
        errors = []
        state.apply({'objects': {'S2': {'zone': 'exile', 'kind': 'copy', 'copy_of': 'goblin',
                     'copy_face': 1, 'controller': 'A'}}}, [], '/prepared', errors)
        _check_prepared_copies(state, ORACLE, '/prepared', errors)
        self.assertEqual(errors, [])
        self.assertEqual(state.stack_created, 2)


if __name__ == '__main__':
    unittest.main()
