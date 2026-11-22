"""MGN setup refusals, scoped shuffle receipts and look-and-return defaults."""
from copy import deepcopy
from pathlib import Path
from types import SimpleNamespace
import unittest

from mgn.adapters.karn import KarnAdapter, Refusal
from test_mgn_adapter import ENGINE, ORACLE, MAIN_A, PRIORITY_A, position_record, setup_of, _mana, no_random


class RepairBoundaryTests(unittest.TestCase):
    def test_initial_shuffle_cannot_claim_cards_kept_out(self):
        adapter = KarnAdapter(Path('/unused'), oracle=object())
        adapter.seats = ['A', 'B']; adapter.decks = {'A': ['a'], 'B': ['b']}
        adapter.pregame = {'first': 'A', 'companions': {}}
        adapter.engine = SimpleNamespace(send=lambda request: self.fail('invalid scope must be refused before setup'))
        for scope in ({'fixed': [['a', 'top']]}, {'excluding': ['a']}):
            with self.subTest(scope=scope), self.assertRaises(Refusal) as caught:
                adapter._start_engine(lambda key: {'kind': 'shuffle', 'seat': key['seat'],
                    'order': adapter.decks[key['seat']], 'scope': scope})
            self.assertEqual((caught.exception.class_, caught.exception.code), ('fault', 'random_mismatch'))

    def test_setup_uses_native_classes_even_when_the_message_mentions_a_companion(self):
        expected = {'illegal': ('illegal', 'illegal_action'), 'unsupported': ('unsupported', 'unsupported_kind'),
                    'unreferable': ('illegal', 'unknown_reference'), 'malformed': ('fault', 'protocol'),
                    'stale': ('fault', 'protocol'), None: ('fault', 'engine_fault')}
        for native, result in expected.items():
            adapter = KarnAdapter(Path('/unused'), oracle=object())
            adapter.seats = ['A', 'B']; adapter.life = 20; adapter.format = 'custom'
            adapter.decks = {'A': ['a'], 'B': ['b']}
            adapter.pregame = {'first': 'A', 'companions': {'A': 'companion'}}
            adapter._face_name = lambda card: 'Forest'
            message = 'controlled companion setup refusal'
            adapter.engine = SimpleNamespace(send=lambda request: {'ok': False, 'error': message, 'class': native})
            with self.subTest(native=native), self.assertRaises(Refusal) as caught:
                adapter._start_engine(lambda key: {'kind': 'shuffle', 'seat': key['seat'], 'order': adapter.decks[key['seat']]})
            self.assertEqual((caught.exception.class_, caught.exception.code), result)
            self.assertEqual(caught.exception.message, message)

    def test_null_look_return_preserves_current_native_order_and_piles_have_no_null(self):
        adapter = KarnAdapter(Path('/unused'), oracle=object())
        asked = {'kind': 'order_cards', 'cards': [17, 4, 11], 'arrange': {'object': 30}}
        self.assertEqual(adapter._null_move(asked), {'kind': 'choose', 'answer': {'objects': [17, 4, 11]}})
        self.assertIsNone(adapter._null_move({'kind': 'order_cards', 'cards': [17, 4, 11]}))
        self.assertIsNone(adapter._null_move({'kind': 'select', 'separate': True, 'min': 0}))

    def test_shuffle_scope_maps_every_physical_reference_without_losing_positions(self):
        adapter = KarnAdapter(Path('/unused'), oracle=object())
        adapter.seats = ['A', 'B']; adapter.current = {'a': 17, 'b': 4, 'c': 11, 'd': 22}
        adapter.outcomes = [{'kind': 'shuffle', 'seat': 'A', 'order': ['a'],
                             'scope': {'excluding': ['b'], 'fixed': [['c', 'top'], ['d', {'from_top': 3}]]}}]
        self.assertEqual(adapter._engine_random_outcomes(), [{'kind': 'shuffle', 'player': 0, 'order': [17],
            'scope': {'excluding': [4], 'fixed': [[11, 'top'], [22, {'from_top': 3}]]}}])
        adapter.outcomes[0]['scope']['fixed'][0][0] = 'unknown'
        with self.assertRaises(Refusal) as caught:
            adapter._engine_random_outcomes()
        self.assertEqual((caught.exception.class_, caught.exception.code), ('fault', 'random_mismatch'))


@unittest.skipUnless(ENGINE.is_file(), 'build game-serve to exercise the adapter')
class RepairIntegrationTests(unittest.TestCase):
    def setUp(self):
        self.adapter = KarnAdapter(ENGINE, ORACLE)
        self.addCleanup(self.adapter.end)

    def surveil(self):
        cards = {'truths': ('Cruel Truths', 'A'), 'colossus': ('Darksteel Colossus', 'A'),
                 'plains': ('Plains', 'A'), 'island': ('Island', 'A'), 'mountain': ('Mountain', 'A')}
        cards.update({f's{i}': ('Swamp', 'A') for i in range(4)})
        record = position_record(cards, {'A.hand': ['truths'], 'A.library': ['colossus', 'plains', 'island', 'mountain'],
            'battlefield': [f's{i}' for i in range(4)]}, MAIN_A, PRIORITY_A)
        self.adapter.setup(setup_of(record))
        self.adapter.act(1, 'A', {'kind': 'cast', 'card': 'truths', 'pay': _mana('s0', 's1', 's2', 's3', mana='B')}, no_random)
        self.adapter.default(None, no_random)
        reply = self.adapter.default(None, no_random)
        self.assertEqual(reply['state']['pending']['kind'], 'arrange')
        return {'kind': 'shuffle', 'seat': 'A', 'order': ['mountain', 'colossus', 'island'],
                'scope': {'fixed': [['plains', 'top']]}}

    def answer_surveil(self, outcomes):
        queue = list(outcomes)
        def random(key):
            if key.get('kind') == 'shuffle' and key.get('seat') == 'A' and queue:
                return queue.pop(0)
            return None
        return self.adapter.act(2, 'A', {'kind': 'arrange', 'top': ['plains'], 'away': ['colossus']}, random)

    def test_colossus_shuffle_keeps_the_other_surveilled_card_on_top_before_drawing(self):
        # Cruel Truths surveils 2, draws 2, loses 2 life. Colossus replaces its graveyard
        # move with a shuffle; Plains is simultaneously put on top (701.24g, 608.2c).
        outcome = self.surveil()
        reply = self.answer_surveil([outcome])
        self.assertEqual(set(reply['state']['zones']['A.hand']), {'plains', 'mountain'})
        self.assertEqual(reply['state']['zones']['A.library'], ['colossus', 'island'])
        self.assertEqual(reply['state']['zones']['A.graveyard'], ['truths'])
        self.assertEqual(reply['state']['players']['A']['life'], 18)
        self.assertEqual(self.adapter.used_outcomes, [outcome])

    def test_bad_fixed_positions_members_and_scope_provenance_are_random_faults(self):
        for mistake in ('position', 'foreign_fixed', 'duplicate', 'duplicate_fixed', 'missing', 'missing_fixed', 'randomized_fixed'):
            with self.subTest(mistake=mistake):
                outcome = self.surveil()
                if mistake == 'position': outcome['scope']['fixed'][0][1] = 'bottom'
                elif mistake == 'foreign_fixed': outcome['scope']['fixed'][0][0] = 'truths'
                elif mistake == 'duplicate': outcome['order'] = ['mountain', 'mountain', 'island']
                elif mistake == 'duplicate_fixed': outcome['scope']['fixed'].append(['plains', 'top'])
                elif mistake == 'missing': outcome['order'].remove('island')
                elif mistake == 'missing_fixed': outcome['scope']['fixed'] = []
                elif mistake == 'randomized_fixed': outcome['scope']['fixed'][0][0] = 'mountain'
                before = deepcopy(self.adapter.state)
                with self.assertRaises(Refusal) as caught:
                    self.answer_surveil([outcome])
                self.assertEqual((caught.exception.class_, caught.exception.code), ('fault', 'random_mismatch'))
                self.assertEqual(self.adapter.state, before)

    def test_unconsumed_fixed_scope_keeps_its_exact_receipt_identity(self):
        outcome = self.surveil()
        extra = deepcopy(outcome)
        extra['order'] = ['island', 'mountain', 'colossus']
        reply = self.answer_surveil([outcome, extra])
        self.assertEqual(self.adapter.used_outcomes, [outcome])
        self.assertEqual(reply['random_unused'], [extra])
        self.assertEqual(self.adapter.outcomes, [])  # handed back to the runner

    def test_default_index_returns_all_five_cards_in_their_current_order(self):
        # Index: look at the top five cards, then put them back in any order.
        # MGN 5.6 supplies the unchanged order as its null answer.
        cards = {'index': ('Index', 'A'), 'land': ('Island', 'A')}
        top = ['p', 'f', 'm', 's', 'i']
        cards.update({card: (name, 'A') for card, name in zip(top, ['Plains', 'Forest', 'Mountain', 'Swamp', 'Island'])})
        record = position_record(cards, {'A.hand': ['index'], 'A.library': top, 'battlefield': ['land']}, MAIN_A, PRIORITY_A)
        self.adapter.setup(setup_of(record))
        self.adapter.act(1, 'A', {'kind': 'cast', 'card': 'index', 'pay': _mana('land', mana='U')}, no_random)
        self.adapter.default(None, no_random)
        reply = self.adapter.default(None, no_random)
        self.assertEqual(reply['state']['pending']['kind'], 'arrange')
        reply = self.adapter.default(None, no_random)
        self.assertEqual(reply['state']['pending']['kind'], 'priority')
        self.assertEqual(reply['state']['zones']['A.library'], top)
        self.assertEqual(reply['state']['zones']['A.graveyard'], ['index'])
