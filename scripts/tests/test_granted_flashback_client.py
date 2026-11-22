"""702.34/601.2b: full grants and explicit cost choices through native seat views."""
import unittest

import play
from client_support import ClientTestCase


class GrantedFlashbackClientTests(ClientTestCase):
    def grant(self, recipient):
        self.new([{'hand': ['Snapcaster Mage'], 'graveyard': [recipient],
                   'battlefield': ['Island'] * 5}, {}, {}])
        self.cast('Snapcaster Mage', mana=self.ids('Island', zone='battlefield')[:2], spend={'U': 2})
        self.pass_round()
        self.assertEqual(self.engine.state['asked']['kind'], 'targets')
        card, = self.engine.state['players'][0]['graveyard']
        move = {'kind': 'targets', 'targets': [[{'object': card}]]}
        self.reject(1, move)
        self.engine.act(move)
        self.pass_round()
        return card

    def test_distinct_costs_are_supplied_and_retained_without_exposing_other_hands(self):
        for index, price in [(0, 3), (1, 2)]:
            with self.subTest(index=index):
                card = self.grant('Think Twice')
                hand_size = len(self.hand(0))
                before = self.view(0)
                move = {'kind': 'cast', 'card': card, 'route': 'flashback', 'targets': [],
                        'mana': {'U': price},
                        'plan': [{'source': land, 'ability': 0} for land in self.ids('Island', zone='battlefield')[2:2 + price]]}
                for cost, reason in [(None, 'multiple flashback costs'), ({'flashback': 2}, 'that flashback cost is unavailable'),
                                     ({'flashback': -1}, 'a flashback cost is a nonnegative cost index'),
                                     ({'flashback': 0, 'alternative': 0}, 'a cost is')]:
                    with self.assertRaisesRegex(play.Refused, reason):
                        self.engine.act({**move, **({'cost': cost} if cost is not None else {})})
                    self.assertEqual(self.view(0), before)
                self.engine.act({**move, 'cost': {'flashback': index}})
                spell, = self.engine.state['stack']
                audit = self.engine.send({'cmd': 'audit_game', 'session_id': self.engine.state['session_id']})
                retained = audit['objects'][str(spell)]['announced']['cast']
                self.assertEqual(retained['route'], {'kind': 'flashback', 'cost_index': index})
                self.pass_round()
                for viewer in [0, 1, 2]:
                    state = self.view(viewer)
                    self.assertEqual(state['stack'], [])
                    self.assertEqual(state['players'][0]['hand_size'], hand_size + 1)
                    self.assertEqual(state['objects'][str(state['exile'][0])]['name'], 'Think Twice')
                    for other in set([0, 1, 2]) - {viewer}:
                        self.assertNotIn('hand', state['players'][other])


if __name__ == '__main__':
    unittest.main()
