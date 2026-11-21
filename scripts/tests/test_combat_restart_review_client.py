"""Real-seat checks of whole combat assignment restart (510.1e/702.19b)."""
import unittest
import test_player_prompt_client as fixtures
import play


class CombatRestartReviewClientTests(unittest.TestCase):
    setUpClass = classmethod(fixtures.PlayerPromptClientTests.setUpClass.__func__)
    new = fixtures.PlayerPromptClientTests.new
    waiting = fixtures.PlayerPromptClientTests.waiting
    named = staticmethod(fixtures.PlayerPromptClientTests.named)
    close_engine = fixtures.PlayerPromptClientTests.close_engine
    refused = fixtures.PlayerPromptClientTests.refused

    def setUp(self):
        self.engine = play.Engine()
        self.addCleanup(self.close_engine)

    def until(self, kind):
        for _ in range(32):
            seat = self.waiting()
            if seat.last['decision']['kind'] == kind:
                return seat
            self.assertEqual(seat.last['decision']['kind'], 'priority', seat.last['decision'])
            seat.answer({'kind': 'pass'})
        self.fail(f'never asked {kind}')

    def test_a_whole_trample_restart_refreshes_the_request_and_accepts_its_new_first_division(self):
        # 510.1e validates the whole assignment after both attackers' divisions; 702.19b
        # requires lethal to a blocker before assigning excess to the defending player.
        me, them = self.new({'battlefield': ['Charging Monstrosaur', 'Ball Lightning']},
                           {'battlefield': ['Grizzly Bears', 'Wandering Ones']})
        p = me.prompt()
        mon = self.named(p, 'Charging Monstrosaur')[0]
        ball = self.named(p, 'Ball Lightning')[0]
        bear = self.named(p, 'Grizzly Bears')[0]
        one = self.named(p, 'Wandering Ones')[0]
        self.assertIs(self.until('attackers'), me)
        me.answer({'kind': 'attack', 'attacks': [[mon, 'p1'], [ball, 'p1']]})
        self.assertIs(self.until('blockers'), them)
        theirs = them.last
        them.answer({'kind': 'block', 'blocks': [[self.named(theirs, 'Grizzly Bears')[0], self.named(theirs, 'Charging Monstrosaur')[0]],
                                               [self.named(theirs, 'Wandering Ones')[0], self.named(theirs, 'Ball Lightning')[0]]]})
        self.assertIs(self.until('divide'), me)
        first = me.last['decision']
        self.assertEqual((first['attacker'], first['amount'], first['among'], first['lethal']),
                         (mon, 5, [bear, 'p1'], [2, None]))
        me.answer({'kind': 'divide', 'division': [[bear, 1], ['p1', 4]]})
        self.assertEqual((me.last['decision']['attacker'], me.last['decision']['amount']), (ball, 6))
        old_request = me.last['request_id']
        with self.assertRaises(SystemExit) as refused:
            me.answer({'kind': 'divide', 'division': [[one, 1], ['p1', 5]]})
        self.assertIn('702.19b', str(refused.exception))
        self.assertIn('510.1e', str(refused.exception))
        # A refused Seat.answer keeps its last prompt; a fresh prompt asks the restart.
        self.assertEqual(me.last['request_id'], old_request)
        p = me.prompt()
        restarted = p['request_id']
        self.assertGreater(restarted, old_request)
        self.assertEqual(p['decision'], first | {'request_id': restarted})
        self.assertEqual([v['life'] for v in p['players']], [20, 20])
        self.assertEqual(them.prompt()['request_id'], restarted)
        self.assertIsNone(them.last['decision'])
        legal_first = {'kind': 'divide', 'division': [[bear, 2], ['p1', 3]]}
        error = self.refused(me, {'cmd': 'answer', 'request_id': old_request, 'answer': legal_first})
        self.assertIn(f'stale request {old_request}; expected {restarted}', error)
        self.assertEqual(me.prompt()['request_id'], restarted)
        # Local shape/sum failures change no assignment and are never capped mandatory answers.
        for _ in range(4):
            error = self.refused(me, {'cmd': 'answer', 'answer': {'kind': 'divide', 'division': [[bear, 1], ['p1', 3]]}})
            self.assertIn('510.1a', error)
            self.assertEqual(me.prompt()['request_id'], restarted)
            self.assertEqual(me.last['decision'], first | {'request_id': restarted})
        me.answer(legal_first)
        self.assertEqual(me.last['decision']['attacker'], ball)
        self.assertEqual([v['life'] for v in me.last['players']], [20, 20], 'damage waits for the whole assignment')
        me.answer({'kind': 'divide', 'division': [[one, 1], ['p1', 5]]})
        p = them.prompt()
        self.assertEqual(p['decision']['kind'], 'order_cards')
        them.answer({'kind': 'choose', 'answer': {'objects': [bear, one]}})
        p = me.prompt()
        self.assertEqual([v['life'] for v in p['players']], [20, 12])
        self.assertEqual(p['decision']['kind'], 'priority')
        self.assertEqual(len(self.named(p, 'Charging Monstrosaur', zone='battlefield')), 1)
        for name in ['Grizzly Bears', 'Wandering Ones', 'Ball Lightning']:
            self.assertEqual(len(self.named(p, name, zone='graveyard')), 1, name)

    def test_a_refused_payment_correction_keeps_the_request_and_reversal_moves_on(self):
        # 601.2g-h/733.1: the mana ability completes but cannot finish the outer cost.
        me, them = self.new({'hand': ['Grizzly Bears'], 'battlefield': ['Empowered Autogenerator']}, {})
        p = me.prompt()
        bear = self.named(p, 'Grizzly Bears')[0]
        auto = self.named(p, 'Empowered Autogenerator')[0]
        step = {'ability': f'{auto}.a1'}
        me.answer({'kind': 'cast', 'card': bear, 'plan': [step, step]})
        self.assertEqual(me.last['decision']['kind'], 'mana')
        me.answer({'kind': 'choose', 'answer': {'mana': ['G']}})
        p = me.prompt()
        self.assertEqual(p['decision']['kind'], 'payment')
        self.assertIsNone(them.prompt()['decision'])
        request, decision = p['request_id'], p['decision']
        for _ in range(4):
            error = self.refused(me, {'cmd': 'answer', 'answer': {'kind': 'payment', 'plan': [], 'paying': []}})
            self.assertTrue(error)
            after = me.prompt()
            self.assertEqual(after['request_id'], request)
            self.assertEqual(after['decision'], decision)
            self.assertEqual(after['players'], p['players'])
            self.assertEqual(after['stack'], p['stack'])
            self.assertEqual(after['objects'], p['objects'])
        p = me.answer({'kind': 'reverse_payment'})
        self.assertGreater(p['request_id'], request)
        self.assertEqual(p['decision']['kind'], 'priority')
        self.assertEqual(p['stack'], [])
        self.assertEqual(len(self.named(p, 'Grizzly Bears', zone='hand')), 1)
        self.assertEqual(p['players'][0]['pool']['G'], 1, 'legal mana abilities are retained on outer reversal')
        self.assertTrue(p['objects'][auto]['tapped'])
        self.assertEqual(p['objects'][auto]['counters']['charge'], 1)


if __name__ == '__main__':
    unittest.main()
