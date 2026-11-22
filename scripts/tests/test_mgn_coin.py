"""705.2 / MGN 5.4, 6: called flips consume physical results without inventing calls."""
from pathlib import Path
import os
import unittest

from mgn.adapters.karn import KarnAdapter, Refusal
from mgn.native_actions import native_action
from test_mgn_adapter import ORACLE, MAIN_A, PRIORITY_A, position_record, setup_of, no_random

ENGINE = Path(os.environ.get('KARN_CONFORMANCE_ENGINE', Path(__file__).resolve().parents[2] / 'target/fast/game-serve'))


@unittest.skipUnless(ENGINE.is_file(), 'build the native engine')
class CoinAdapterTests(unittest.TestCase):
    def setUp(self):
        self.a = KarnAdapter(ENGINE, ORACLE)
        self.addCleanup(self.a.end)

    def pending(self):
        cards = {'crypt': ('Mana Crypt', 'A'), **{f'a{i}': ('Forest', 'A') for i in range(4)},
                 **{f'b{i}': ('Forest', 'B') for i in range(4)}}
        record = position_record(cards, {'battlefield': ['crypt'], 'A.library': [f'a{i}' for i in range(4)],
                                        'B.library': [f'b{i}' for i in range(4)]}, MAIN_A, PRIORITY_A)
        self.a.setup(setup_of(record))
        for _ in range(200):
            reply = self.a.default(None, no_random)
            if reply['state']['pending']['kind'] == 'coin_call':
                break
        self.assertEqual(reply['state']['pending']['kind'], 'coin_call')
        self.assertEqual(reply['state']['pending']['seat'], 'A')

    def test_called_flip_replays_both_faces_and_native_call_has_an_inverse(self):
        for face, life, winner in [('heads', 20, 'A'), ('tails', 17, None)]:
            with self.subTest(face=face):
                self.pending()
                move = {'kind': 'choose', 'answer': {'call': 'heads'}}
                self.assertEqual(native_action(self.a, move, {}), {'kind': 'coin_call', 'call': 'heads'})
                outcomes = [{'kind': 'coin', 'caller': 'A', 'call': 'heads', 'result': face, 'winner': winner}]
                def ask(key):
                    return outcomes.pop(0) if key['kind'] == 'coin' and outcomes else None
                reply = self.a.act(1, 'A', {'kind': 'coin_call', 'call': 'heads'}, ask)
                self.assertEqual(reply['state']['players']['A']['life'], life)
                self.assertEqual(reply['state']['players']['B']['life'], 20)
                self.assertEqual(len(self.a.used_outcomes), 1)

    def test_a_receipts_call_and_winner_cannot_override_the_players_actual_call(self):
        self.pending()
        before = self.a.project(2)
        for receipt in [{'kind': 'coin', 'caller': 'A', 'call': 'tails', 'result': 'tails', 'winner': 'A'},
                        {'kind': 'coin', 'caller': 'A', 'call': 'heads', 'result': 'tails', 'winner': 'A'}]:
            outcomes = [receipt]
            def ask(key):
                return outcomes.pop(0) if key['kind'] == 'coin' and outcomes else None
            with self.assertRaises(Refusal) as caught:
                self.a.act(1, 'A', {'kind': 'coin_call', 'call': 'heads'}, ask)
            self.assertEqual(caught.exception.code, 'random_mismatch')
            self.assertEqual(self.a.project(2), before)


if __name__ == '__main__':
    unittest.main()
