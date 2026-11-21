"""707.2/2b/9/10, 708.2a, 202.1b: intrinsic spell-copy definitions.

These state-unit boundaries are not claims that their abbreviated premises are
complete legal games. The separate authored Narset record has reviewed actions.
"""
from copy import deepcopy
from pathlib import Path
import unittest

from mgn import copy_definition
from mgn.adapters.karn import KarnAdapter, Refusal
from mgn.canonical import sha256
from mgn.delta import diff_states
from mgn.examples import EXAMPLES, load
from mgn.identity import game_identity, record_identity
from mgn.oracle import Oracle
from mgn.render import parse, render
from mgn.runner import compare
from mgn.state import State
from mgn.validate import validate, _check_copy_definitions, _check_abilities
from scripts.tests import test_mgn_spell_copy as spell_tests
from scripts.tests import test_mgn_ability_copy as ability_tests

BONECRUSHER = spell_tests.BONECRUSHER

O = Oracle()
DRAFT = EXAMPLES.parents[2] / 'tests/fixtures/mgn-drafts/narset-reversal-folded-copy.json'


class CopyDefinitionTests(unittest.TestCase):
    def initial(self):
        return State.from_record(spell_tests.SpellCopyBasisTests().record(), [])

    def introduce(self, state, definition, **other):
        errors = []
        state.apply({'objects': {'giant': {'zone': 'exile'},
            'S1': {'zone': 'stack', 'kind': 'copy', 'copy_of': 'giant',
                   'copy_definition': definition, 'controller': 'A', **other}}}, [], '/copy', errors)
        return errors

    def definition(self):
        return {'oracle': BONECRUSHER, 'face': 1}

    def test_folded_departure_retains_full_spell_face_choices_and_roundtrips(self):
        state = self.initial()
        before = deepcopy(state)
        # 707.10: targets/modes/X/additional-cost choices are separate from 707.2 values.
        choices = {'targets': [['B']], 'modes': [1], 'x': 4,
                   'cast': {'additional': {'kicker': 1}}}
        self.assertEqual(self.introduce(state, self.definition(), **choices), [])
        self.assertEqual(state._printed('S1', O)['types'], ['instant'])
        for key, value in choices.items():
            self.assertEqual(state.objects['S1'][key], value)
        delta = diff_states(before, state, O)
        errors = []
        before.apply(delta, [], '/inverse', errors)
        self.assertEqual(errors, [])
        self.assertEqual(before.explicit(O), state.explicit(O))
        observed = spell_tests.SpellCopyBasisTests().record()
        observed['start']['state'] = state.explicit(O)
        restored = State.from_record(observed, errors)
        self.assertEqual(errors, [])
        self.assertEqual(restored.explicit(O), state.explicit(O))
        self.assertEqual(restored.copy_bases['S1'], (BONECRUSHER, 1))

    def test_explicit_definition_is_immutable_and_unavailable_basis_cannot_be_repaired(self):
        state = self.initial()
        self.assertEqual(self.introduce(state, self.definition()), [])
        errors = []
        state.apply({'objects': {'S1': {'copy_definition': {'oracle': BONECRUSHER, 'face': 0}}}}, [], '/change', errors)
        self.assertTrue(any(p.code == 'delta' and 'immutable' in p.message for p in errors))
        self.assertEqual(state.copy_bases['S1'], (BONECRUSHER, 1))
        state = self.initial()
        errors = []
        state.apply({'objects': {'giant': {'zone': 'exile'}, 'S1': {'zone': 'stack',
            'kind': 'copy', 'copy_of': 'giant', 'controller': 'A'}}}, [], '/missing', errors)
        self.assertTrue(any(p.code == 'unsupported' for p in errors))
        errors = []
        state.apply({'objects': {'S1': {'copy_definition': self.definition()}}}, [], '/too-late', errors)
        self.assertTrue(any('immutable' in p.message for p in errors))
        self.assertIsNone(state.copy_bases['S1'])

    def test_contradictory_printed_or_full_definition_cannot_override_unchanged_donor(self):
        for definition in ({'oracle': BONECRUSHER, 'face': 0},
                           copy_definition.characteristics((BONECRUSHER, 0), O)):
            state = self.initial()
            errors = []
            state.apply({'objects': {'S1': {'zone': 'stack', 'kind': 'copy', 'copy_of': 'giant',
                'controller': 'A', 'copy_definition': definition}}}, [], '/copy', errors)
            _check_copy_definitions(state, O, '/copy', errors)
            self.assertTrue(any(p.code == 'coherence' for p in errors), errors)

    def test_nonprinted_nameless_costless_copy_is_not_observable_donor_power(self):
        # CR 708.2a: copying a face-down creature spell gives its face-down
        # copiable values, not its printed face or a later Giant Growth's 5/5.
        definition = {'name': None, 'mana_cost': None, 'colors': [], 'color_indicator': [],
            'types': ['creature'], 'supertypes': [], 'subtypes': [], 'text': '',
            'power': '2', 'toughness': '2'}
        state = self.initial()
        state.objects['giant'].update(face_down=True, power=5, toughness=5)
        errors = self.introduce(state, definition)
        self.assertEqual(errors, [])
        self.assertEqual(state._printed('S1', O)['power'], 2)
        self.assertEqual(state._printed('S1', O)['types'], ['creature'])
        self.assertEqual(state._printed('S1', O)['keywords'], [])
        _check_abilities({'source': 'S1', 'ability': {'face': 0, 'paragraph': 1}}, state, O, '/ability', errors)
        self.assertTrue(any(p.code == 'ability' for p in errors))
        zero = deepcopy(definition); zero['mana_cost'] = '{0}'
        self.assertFalse(copy_definition.equivalent(zero, definition, O))
        self.assertFalse(copy_definition.equivalent({**definition, 'power': '*'}, definition, O))

    def test_nonprinted_definition_and_copy_of_copy_keep_text_after_donor_ceases(self):
        definition = {'name': 'Copied Beast', 'mana_cost': None, 'colors': ['green'],
            'color_indicator': [], 'types': ['creature'], 'supertypes': [], 'subtypes': ['beast'],
            'text': 'Flying\nWhen this creature enters, draw a card.', 'power': '*', 'toughness': '3'}
        state = self.initial()
        self.assertEqual(self.introduce(state, definition), [])
        errors = []
        state.apply({'objects': {'S2': {'zone': 'stack', 'kind': 'copy', 'copy_of': 'S1',
            'controller': 'B'}, 'S1': None}}, [], '/copy-again', errors)
        self.assertEqual(errors, [])
        self.assertEqual(state._printed('S2', O)['power'], '*')
        self.assertEqual(state._printed('S2', O)['keywords'], ['flying'])
        _check_abilities({'source': 'S2', 'ability': {'face': 0, 'paragraph': 2}}, state, O, '/ability', errors)
        self.assertEqual(errors, [])

    def test_explicit_definition_cannot_turn_copied_ability_into_spell(self):
        fixture = ability_tests.AbilityCopyTests()
        fixture.oracle = O
        state = State.from_record(fixture.premise(), [])
        errors = []
        state.apply({'objects': {'S2': {'zone': 'stack', 'kind': 'copy', 'copy_of': 'S1',
            'controller': 'B', 'copy_definition': self.definition()}}}, [], '/ability-copy', errors)
        self.assertTrue(any('no spell definition' in p.message for p in errors))
        self.assertTrue(state.is_ability('S2'))
        self.assertIsNone(state._printed('S2', O))

    def test_comparison_accepts_equivalent_definition_forms_without_changing_hashes(self):
        record = spell_tests.SpellCopyBasisTests().record(copy_present=True)
        state = State.from_record(record, [])
        plain = state.explicit(O)
        explicit = deepcopy(plain)
        explicit['objects']['S1']['copy_definition'] = self.definition()
        before = sha256(plain)
        self.assertEqual(compare(plain, explicit, [], 2, oracle=O, cards=state.cards), ([], []))
        self.assertEqual(sha256(plain), before)
        explicit['objects']['S1']['copy_definition'] = copy_definition.characteristics((BONECRUSHER, 1), O)
        self.assertEqual(compare(plain, explicit, [], 2, oracle=O, cards=state.cards), ([], []))
        explicit['objects']['S1']['copy_definition']['power'] = '9'
        self.assertTrue(compare(plain, explicit, [], 2, oracle=O, cards=state.cards)[0])

    def test_authored_narset_record_validation_render_identity_and_expected_outcomes(self):
        record = load(DRAFT)
        self.assertEqual(validate(record, O), [])
        self.assertEqual(parse(render(record)), record)
        errors = []
        state = State.from_record(record, errors)
        for transition in record['transitions'][:2]:
            state.apply(transition['delta'], [], '/transition', errors)
        self.assertEqual(state.listing('stack'), ['S1'])
        self.assertEqual(state.zone_of('divination'), 'A.hand')
        self.assertEqual(state._printed('S1', O)['types'], ['sorcery'])
        state.apply(record['transitions'][2]['delta'], [], '/shock-and-draw', errors)
        self.assertEqual(errors, [])
        self.assertEqual(state.players['A']['life'], 18)
        self.assertEqual(state.listing('A.library'), ['draw3'])
        self.assertEqual(set(state.listing('A.hand')), {'divination', 'draw1', 'draw2'})
        changed = deepcopy(record)
        changed['transitions'][1]['delta']['objects']['S1']['copy_definition']['face'] = 1
        self.assertTrue(validate(changed, O))
        self.assertNotEqual(record_identity(record), record_identity(changed))
        # Deltas are expectations; game identity deliberately excludes them.
        self.assertEqual(game_identity(record), game_identity(changed))

    def test_double_faced_spell_copy_retains_both_modified_faces(self):
        # 707.10g retains BOTH copiable faces, including when the back is up.
        oid = O.by_name('Etali, Primal Conqueror // Etali, Primal Sickness')
        pair = copy_definition.whole((oid, 1), O)
        self.assertEqual(len(pair['faces']), 2)
        self.assertEqual(pair['face'], 1)
        self.assertTrue(copy_definition.equivalent(pair, (oid, 1), O))
        modified = deepcopy(pair)
        modified['faces'][0]['name'] = 'Modified front'
        modified['faces'][1]['power'] = '9'
        self.assertFalse(copy_definition.equivalent(modified, pair, O))
        state = self.initial()
        self.assertEqual(self.introduce(state, modified, face=1), [])
        self.assertEqual(state._printed('S1', O)['power'], 9)
        record = spell_tests.SpellCopyBasisTests().record()
        record['start']['state'] = state.explicit(O)
        # Records omit the default occurrence; the explicit checkpoint form
        # writes it out as 1, while record spelling admits only nondefault 2+.
        record['start']['state']['turn'].pop('occurrence', None)
        self.assertEqual(validate(record, O), [])
        self.assertEqual(parse(render(record)), record)
        restored = State.from_record(record, [])
        self.assertEqual(restored.copy_bases['S1']['faces'][0]['name'], 'Modified front')
        self.assertEqual(restored.copy_bases['S1']['faces'][1]['power'], '9')
        for malformed in ({'faces': [pair['faces'][0]], 'face': 0},
                          {'faces': pair['faces'], 'face': 2}):
            record['start']['state']['objects']['S1']['copy_definition'] = malformed
            self.assertTrue(any(p.code == 'schema' for p in validate(record, O)))

    def test_copy_definition_is_part_of_initial_inputs_and_record_identity(self):
        record = spell_tests.SpellCopyBasisTests().record(copy_present=True)
        record['start']['state']['objects']['S1']['copy_definition'] = self.definition()
        changed = deepcopy(record)
        changed['start']['state']['objects']['S1']['copy_definition']['face'] = 0
        self.assertNotEqual(record_identity(record), record_identity(changed))
        self.assertNotEqual(game_identity(record), game_identity(changed))

    def test_observation_cannot_replace_a_frozen_definition(self):
        state = self.initial()
        self.assertEqual(self.introduce(state, self.definition()), [])
        observed = spell_tests.SpellCopyBasisTests().record()
        observed['start']['state'] = state.explicit(O)
        observed['start']['state']['objects']['S1']['copy_definition']['face'] = 0
        errors = []
        State.from_observation(observed, errors, state)
        self.assertTrue(any('immutable' in p.message for p in errors))

    def test_adapter_frozen_face_proof_survives_departed_donor_and_missing_evidence_refuses(self):
        adapter = KarnAdapter(Path('/unused'), O)
        adapter.projection_history = None
        adapter._record_id = lambda i: {10: 'S1', 4: 'giant'}[i]
        adapter.audit = {'objects': {'4': {'zone': 'Hand'}}}
        adapter.state = {'objects': {}}
        facts = {'kind': 'copy', 'copy_of': 'giant'}
        item = {'stack_copy_of': 4, 'spell_copy_definition': {'card': 12,
            'name': O.faces[BONECRUSHER][0].name, 'face': 'Adventure'}}
        self.assertEqual(adapter._spell_copy_definition(10, item, facts), self.definition())
        item['spell_copy_definition'] = {'unavailable': 'derived'}
        with self.assertRaises(Refusal) as refused:
            adapter._spell_copy_definition(10, item, facts)
        self.assertEqual(refused.exception.class_, 'unsupported')
        self.assertEqual(refused.exception.code, 'unsupported_fact')
        self.assertEqual(refused.exception.path, 'objects.S1.copy_definition')
        self.assertEqual(refused.exception.message, "spell copy's full frozen definition is unavailable")


if __name__ == '__main__':
    unittest.main()
