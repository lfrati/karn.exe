"""An inherited ability names its acquisition's exact definition (113.7, 607.5)."""
from copy import deepcopy
from pathlib import Path
import unittest
from unittest.mock import Mock

from mgn.adapters.karn import KarnAdapter, Refusal


class FrozenReferenceTests(unittest.TestCase):
    def test_resolved_grant_names_the_producer_paragraph_after_source_departure(self):
        # Urza's Saga chapters I and II each give an activated ability. Their
        # templates both have row 1; the granting paragraphs are 1 and 2
        # (714.2b, 611.2a). A departed grantor is still a valid historical label.
        oracle = Mock()
        oracle.name.return_value = "Urza's Saga"
        oracle.paragraphs.return_value = ['I — grant mana.', 'II — grant tokens.', 'III — search.']
        adapter = KarnAdapter(Path('/unused'), oracle=oracle)
        adapter.cards = {'saga': {'oracle': 'saga-oracle'}}
        adapter.current = {'recipient': 10}  # no current grantor object
        adapter._record_id = lambda native: {20: 'saga', 10: 'recipient'}[native]
        adapter._face_of = lambda native: 0
        rows = [{'index': paragraph + 2, 'paragraph': 1, 'origin': {
            'kind': 'given', 'by': None, 'producer': {
                'source': 20, 'definition': "Urza's Saga", 'face': 0, 'paragraph': paragraph}}}
            for paragraph in (1, 2)]
        adapter.audit = {'objects': {'10': {'abilities': rows}}}
        for paragraph, row in enumerate(rows, 1):
            ref = {'granted_by': 'saga', 'face': 0, 'paragraph': paragraph}
            self.assertEqual(adapter._ability_ref(row, 10), ref)
            self.assertEqual(adapter._row_index('recipient', ref), paragraph + 2)
        rows[1]['origin']['producer']['definition'] = 'A different printing'
        with self.assertRaisesRegex(Refusal, 'pinned paragraph'):
            adapter._ability_ref(rows[1], 10)

    def test_same_wording_does_not_authenticate_another_declaring_definition(self):
        adapter = KarnAdapter(Path('/unused'), oracle=object())
        adapter._record_id = lambda source: {10: 'mimic', 20: 'donor'}[source]
        adapter._face_of = lambda source: 0
        grant_def, donor_def = {'card': 1, 'face': 'Whole'}, {'card': 2, 'face': 'Whole'}
        grant = {'source': 10, 'index': 0, 'declaration_index': 0, 'definition_identity': grant_def,
                 'paragraph': 1, 'origin': {'kind': 'own'}, 'text': 'Acquires abilities.'}
        donor = {'source': 20, 'index': 4, 'declaration_index': 2, 'definition_identity': donor_def,
                 'paragraph': 2, 'origin': {'kind': 'own'}, 'text': 'Identical words.'}
        family = {'donor': 20, 'donor_origin': 'own', 'definition': donor_def,
                  'grant': {'by': 10, 'ability': 0, 'origin': 'own', 'definition': grant_def}}
        acquired = {'declaration_index': 2, 'origin': {'kind': 'inherited', 'family': family}}
        adapter.audit = {'objects': {'10': {'abilities': [grant, acquired]}, '20': {'abilities': [donor]}}}
        self.assertEqual(adapter._ability_ref(acquired, 10), {
            'inherited_from': {'source': 'donor', 'ability': {'face': 0, 'paragraph': 2}},
            'granted_by': {'source': 'mimic', 'ability': {'face': 0, 'paragraph': 1}}})
        changed = deepcopy(donor)
        changed['definition_identity']['card'] = 3
        adapter.audit['objects']['20']['abilities'] = [changed]
        with self.assertRaisesRegex(Refusal, 'acquired definition'):
            adapter._ability_ref(acquired, 10)

    def test_own_printed_copy_exception_and_inherited_activation_keep_their_namespace(self):
        # A copying card may keep/add an ability of its own printing (707.9b).
        # That paragraph is not a paragraph of the copied donor's text.
        adapter = KarnAdapter(Path('/unused'), oracle=object())
        adapter.current = {'copy': 10}
        adapter._face_of = lambda source: 0
        own = {'index': 0, 'paragraph': 2, 'origin': {'kind': 'own'},
               'definition_identity': {'card': 7, 'face': 'Whole'}}
        inherited = {'index': 7, 'origin': {'kind': 'inherited'}}
        adapter.audit = {'objects': {'10': {'physical_definition': 7,
                         'copy_acquisition': {'donor': 20}, 'abilities': [own, inherited]}}}
        self.assertEqual(adapter._ability_ref(own, 10), {'face': 0, 'paragraph': 2, 'of': 'card'})
        ref = {'inherited_from': {'source': 'donor', 'ability': {'face': 0, 'paragraph': 1}},
               'granted_by': {'source': 'copy', 'ability': {'face': 0, 'paragraph': 2, 'of': 'card'}}}
        adapter._ability_ref = lambda row, source: ref
        self.assertEqual(adapter._row_index('copy', ref), 7)

    def test_entry_copy_own_rows_reference_copied_text(self):
        # Metamorph copying Angel of Serenity acquires the Angel's paragraph 2
        # (707.2); Own is a native origin, not proof of Metamorph's printing.
        adapter = KarnAdapter(Path('/unused'), oracle=object())
        adapter._face_of = lambda source: 0
        adapter.audit = {'objects': {'10': {'physical_definition': 7,
                         'copy_acquisition': {'donor': 20}}}}
        row = {'paragraph': 2, 'origin': {'kind': 'own'},
               'definition_identity': {'definition': {'name': 'Angel of Serenity'}}}
        self.assertEqual(adapter._ability_ref(row, 10), {'face': 0, 'paragraph': 2})
        row['definition_identity'] = {'card': 8, 'face': 'Whole'}
        self.assertEqual(adapter._ability_ref(row, 10), {'face': 0, 'paragraph': 2})
