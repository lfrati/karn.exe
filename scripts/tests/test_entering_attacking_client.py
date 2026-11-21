"""Real seats see what a creature put onto the battlefield attacking attacks (508.4, 506.4)."""
import unittest

import play
import test_player_prompt_client as fixtures


class EnteringAttackingClientTests(unittest.TestCase):
    new = fixtures.PlayerPromptClientTests.new
    waiting = fixtures.PlayerPromptClientTests.waiting
    named = staticmethod(fixtures.PlayerPromptClientTests.named)
    close_engine = fixtures.PlayerPromptClientTests.close_engine

    @classmethod
    def setUpClass(cls):
        fixtures.PlayerPromptClientTests.setUpClass()

    def setUp(self):
        self.engine = play.Engine()
        self.addCleanup(self.close_engine)

    def test_each_seat_sees_whom_each_token_attacks(self):
        # Rules: 508.4, 506.4
        me, them = self.new({'battlefield': ['Hero of Bladehold']}, {'battlefield': ['Gideon, the Oathsworn']})
        hero = self.named(me.prompt(), 'Hero of Bladehold')[0]
        gideon = self.named(me.last, 'Gideon, the Oathsworn')[0]
        asked = []
        for _ in range(80):
            seat = self.waiting()
            d = seat.last['decision']
            if d['kind'] in ('blockers', 'priority') and len(asked) == 2:
                break
            if d['kind'] == 'attackers':
                seat.answer({'kind': 'attack', 'attacks': [[hero, 'p1']]})
            elif d['kind'] == 'order':
                seat.answer({'kind': 'order', 'order': list(range(len(d['ordered'])))})
            elif d['kind'] == 'defender':
                asked.append((d['nth'], d['of']))
                # the first soldier at Gideon, the second at the player
                seat.answer({'kind': 'choose', 'answer': {'defender': gideon if d['nth'] == 1 else 'p1'}})
            else:
                self.assertEqual(d['kind'], 'priority', d)
                seat.answer({'kind': 'pass'})
        else:
            self.fail('the tokens never entered')
        self.assertEqual(asked, [(1, 2), (2, 2)])
        for view in (me.prompt(), them.prompt()):
            soldiers = self.named(view, 'Soldier Token', zone='battlefield')
            attacks = {a['attacker']: a['target'] for a in view['attacks']}
            self.assertEqual(attacks[hero], 'p1')
            self.assertEqual(sorted(attacks[s] for s in soldiers), sorted([gideon, 'p1']))
            self.assertEqual(sorted(view['attacking']), sorted(attacks))


if __name__ == '__main__':
    unittest.main()
