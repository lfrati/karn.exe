"""Real-seat checks of tokens entering attacking and battle cry."""
import os
import unittest

import play
import test_player_prompt_client as fixtures


class EnteringAttackingReviewClientTests(unittest.TestCase):
    new = fixtures.PlayerPromptClientTests.new
    waiting = fixtures.PlayerPromptClientTests.waiting
    named = staticmethod(fixtures.PlayerPromptClientTests.named)
    close_engine = fixtures.PlayerPromptClientTests.close_engine
    refused = fixtures.PlayerPromptClientTests.refused

    @classmethod
    def setUpClass(cls):
        if not os.environ.get('ENTERING_ATTACKING_REVIEW_ENGINE'):
            fixtures.PlayerPromptClientTests.setUpClass()

    def setUp(self):
        self.engine = play.Engine(executable=os.environ.get('ENTERING_ATTACKING_REVIEW_ENGINE'))
        self.addCleanup(self.close_engine)

    def combat(self, reversed_order, pick):
        """Attack with the Hero, order its triggers, answer each defender question with `pick`;
        the defender questions asked, at the postcombat main phase."""
        asked = []
        for _ in range(80):
            seat = self.waiting()
            d = seat.last['decision']
            if seat.last['part'] == 'PostcombatMain':
                return asked
            if d['kind'] == 'attackers':
                seat.answer({'kind': 'attack', 'attacks': [[self.named(seat.last, 'Hero of Bladehold')[0], 'p1']]})
            elif d['kind'] == 'order':
                order = list(range(len(d['ordered'])))
                seat.answer({'kind': 'order', 'order': order[::-1] if reversed_order else order})
            elif d['kind'] == 'defender':
                asked.append(d)
                self.assertIn('508.4', self.refused(seat, {'cmd': 'answer', 'answer': {'kind': 'choose', 'answer': {'defender': 'p0'}}}))
                seat.answer({'kind': 'choose', 'answer': {'defender': pick(d['among'])}})
            elif d['kind'] == 'blockers':
                seat.answer({'kind': 'block', 'blocks': []})
            else:
                self.assertEqual(d['kind'], 'priority', d)
                seat.answer({'kind': 'pass'})
        self.fail('combat did not end')

    def test_each_hero_token_is_asked_its_defender_and_battle_cry_order_decides_its_power(self):
        # Rules: 508.4, 506.3b, 702.91a, 603.3b
        losses = set()
        for reversed_order in [False, True]:
            with self.subTest(reversed_order=reversed_order):
                me, them = self.new({'battlefield': ['Hero of Bladehold']}, {'battlefield': ['Gideon, the Oathsworn']})
                gideon = self.named(me.prompt(), 'Gideon, the Oathsworn')[0]
                loyalty = me.last['objects'][gideon]['counters']['loyalty']
                asked = self.combat(reversed_order, lambda among: gideon)
                self.assertEqual(len(asked), 2, 'one question per token')
                for d in asked:
                    self.assertEqual(d['chooser'], 'p0')
                    self.assertEqual(sorted(d['among']), sorted(['p1', gideon]))
                    self.assertEqual(d['for']['source_name'], 'Hero of Bladehold')
                p = me.prompt()
                self.assertEqual(p['players'][1]['life'], 17, 'only the Hero hit the player')
                left = p['objects'][gideon]['counters'].get('loyalty', 0) if gideon in p['objects'] and p['objects'][gideon]['zone'] == 'battlefield' else 0
                losses.add(loyalty - left)
                self.assertEqual(len(self.named(p, 'Soldier Token', zone='battlefield')), 2)
        self.assertEqual(losses, {2, 4}, 'unpumped 1/1s, or pumped 2/1s, by trigger order')

    def test_with_one_defender_nothing_is_asked(self):
        # Rules: 508.4
        me, them = self.new({'battlefield': ['Hero of Bladehold']}, {})
        self.assertEqual(self.combat(False, lambda among: among[0]), [])
        self.assertIn(me.prompt()['players'][1]['life'], (20 - 3 - 2, 20 - 3 - 4))


if __name__ == '__main__':
    unittest.main()
