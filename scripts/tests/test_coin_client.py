"""705.2: the managed caller chooses before a trusted random result is consumed."""
import unittest

import play
from client_support import ClientTestCase


class CoinClientTests(ClientTestCase):
    def pending(self):
        self.engine.ask({'cmd': 'new', 'players': [
            {'battlefield': ['Mana Crypt'], 'library': ['Forest'] * 8},
            {'library': ['Forest'] * 8}, {'library': ['Forest'] * 8}]})
        for _ in range(200):
            if self.engine.state['asked']['kind'] == 'coin_call':
                return
            self.step()
        self.fail('never reached the upkeep coin call')

    def request(self, outcomes):
        state = self.engine.state
        return {'cmd': 'act', 'session_id': state['session_id'], 'request_id': state['request_id'],
                'player': 0, 'move': {'kind': 'choose', 'answer': {'call': 'heads'}},
                'random_outcomes': outcomes}

    def test_recorded_heads_and_tails_produce_different_crypt_results_for_every_seat(self):
        for face, life in [('heads', 20), ('tails', 17)]:
            with self.subTest(face=face):
                self.pending()
                self.assertEqual(self.engine.state['asked']['player'], 0)
                self.assertEqual(self.engine.state['players'][0]['life'], 20)
                state = self.engine.send(self.request([{'kind': 'coin', 'player': 0, 'result': face}]))
                flips = [e for e in state['events'] if e['event'] == 'coin_flipped']
                self.assertEqual(len(flips), 1)
                self.assertEqual((flips[0]['call'], flips[0]['result'], flips[0]['won']), ('heads', face, face == 'heads'))
                for viewer in [0, 1, 2]:
                    observed = self.view(viewer)
                    self.assertEqual([p['life'] for p in observed['players']], [life, 20, 20])
                    self.assertEqual(observed['stack'], [])
                    for other in set([0, 1, 2]) - {viewer}:
                        self.assertNotIn('hand', observed['players'][other])

    def test_bad_coin_receipts_are_atomic_and_do_not_poison_the_game(self):
        self.pending()
        before = self.view(0)
        for outcomes in [[], [{'kind': 'coin', 'player': 1, 'result': 'tails'}],
                         [{'kind': 'coin', 'player': 0, 'result': 'edge'}],
                         [{'kind': 'coin', 'player': 0, 'result': 'tails', 'call': 'tails'}]]:
            with self.assertRaises(play.Refused) as caught:
                self.engine.send(self.request(outcomes))
            self.assertEqual(caught.exception.answer['code'], 'replay_input')
            self.assertEqual(self.view(0), before)
        self.engine.send(self.request([{'kind': 'coin', 'player': 0, 'result': 'tails'}]))
        self.assertEqual(self.view(0)['players'][0]['life'], 17)


if __name__ == '__main__':
    unittest.main()
