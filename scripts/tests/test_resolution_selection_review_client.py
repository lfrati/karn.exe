"""Real-seat resolution choices: 608.2d, 608.2c, 609.3 and linked cards (607.2a).
"""
import unittest
import test_player_prompt_client as fixtures
import play


class ResolutionSelectionReviewClientTests(unittest.TestCase):
    setUpClass = classmethod(fixtures.PlayerPromptClientTests.setUpClass.__func__)
    new = fixtures.PlayerPromptClientTests.new
    waiting = fixtures.PlayerPromptClientTests.waiting
    named = staticmethod(fixtures.PlayerPromptClientTests.named)
    close_engine = fixtures.PlayerPromptClientTests.close_engine
    refused = fixtures.PlayerPromptClientTests.refused

    def setUp(self):
        self.engine = play.Engine()
        self.addCleanup(self.close_engine)

    def until_choice(self):
        for _ in range(16):
            seat = self.waiting()
            if seat.last['decision']['kind'] != 'priority':
                return seat
            self.assertTrue(seat.last['stack'], 'expected a resolving spell or ability')
            seat.answer({'kind': 'pass'})
        self.fail('no resolving choice')

    def clear_stack(self):
        for _ in range(16):
            seat = self.waiting()
            if not seat.last['stack']:
                return
            self.assertEqual(seat.last['decision']['kind'], 'priority')
            seat.answer({'kind': 'pass'})
        self.fail('stack did not clear')

    def test_cruel_ultimatum_chooses_its_return_after_the_opponents_actions(self):
        me, them = self.new({'hand': ['Cruel Ultimatum'],
                            'battlefield': ['Island'] * 2 + ['Swamp'] * 3 + ['Mountain'] * 2,
                            'graveyard': ['Grizzly Bears', 'Llanowar Elves', 'Shock']},
                           {'hand': ['Mountain', 'Island', 'Swamp', 'Plains'],
                            'battlefield': ['Gladecover Scout', 'Grizzly Bears']})
        p = me.prompt()
        mana = self.named(p, 'Island') + self.named(p, 'Swamp') + self.named(p, 'Mountain', controller='p0')
        me.answer({'mana': {'U': 2, 'B': 3, 'R': 2}, 'kind': 'cast', 'card': self.named(p, 'Cruel Ultimatum')[0], 'targets': [['p1']],
                   'plan': [{'ability': f'{o}.a0'} for o in mana]})
        self.assertIs(self.until_choice(), them)
        d = them.last['decision']
        self.assertEqual((d['kind'], d['verb'], d['count']), ('objects', 'Sacrifice', 1))
        scout = self.named(them.last, 'Gladecover Scout')[0]
        self.assertIn(scout, d['among'], 'hexproof does not prevent a non-targeted sacrifice choice')
        them.answer({'kind': 'choose', 'answer': {'objects': [scout]}})
        self.assertIs(self.until_choice(), them)
        d = them.last['decision']
        self.assertEqual((d['kind'], d['verb'], d['count']), ('objects', 'Discard', 3))
        self.assertIsNone(me.prompt()['decision'], 'only the opponent sees its discard choice')
        discarded=[self.named(them.last,name,zone='hand')[0] for name in ['Mountain','Island','Swamp']]
        them.answer({'kind': 'choose', 'answer': {'objects': discarded}})
        self.assertEqual(them.last['decision']['kind'],'order_cards')
        them.answer({'kind':'choose','answer':{'objects':discarded[::-1]}})
        self.assertIs(self.until_choice(), me)
        d = me.last['decision']
        self.assertEqual((d['kind'], d['count'], d['chooser'], d['reason']), ('objects', 1, 'p0', 'resolution'))
        self.assertEqual(d['for']['source_name'], 'Cruel Ultimatum')
        self.assertEqual(me.last['players'][1]['life'], 15, 'the preceding life loss already happened')
        self.assertEqual({me.last['objects'][o]['name'] for o in d['among']}, {'Grizzly Bears', 'Llanowar Elves'})
        elf = self.named(me.last, 'Llanowar Elves', zone='graveyard')[0]
        me.answer({'kind': 'choose', 'answer': {'objects': [elf]}})
        p = me.prompt()
        self.assertEqual([v['life'] for v in p['players']], [25, 15])
        self.assertEqual(len(p['players'][0]['hand']), 4, 'return one card, then draw three')
        self.assertEqual(len(self.named(p, 'Llanowar Elves', zone='hand')), 1)
        self.assertEqual(len(self.named(p, 'Grizzly Bears', zone='graveyard', controller='p0')), 1)

    def test_witherbloom_choice_sees_milled_lands_before_the_next_mode(self):
        me, them = self.new({'hand': ['Witherbloom Command'], 'battlefield': ['Swamp', 'Forest'],
                            'library': ['Shock', 'Shock', 'Plains', 'Mountain', 'Island'], 'graveyard': ['Forest']},
                           {'battlefield': ['Sol Ring']})
        p = me.prompt()
        ring = self.named(p, 'Sol Ring')[0]
        me.answer({'mana': {'B': 1, 'G': 1}, 'kind': 'cast', 'card': self.named(p, 'Witherbloom Command')[0], 'modes': [0, 1],
                   'targets': [['p0'], [ring]],
                   'plan': [{'ability': f'{o}.a0'} for o in self.named(p, 'Swamp') + self.named(p, 'Forest', zone='battlefield')]})
        self.assertIs(self.until_choice(), me)
        self.assertEqual(me.last['decision']['kind'],'order_cards')
        milled=[self.named(me.last,name,zone='library')[0] for name in ['Plains','Mountain','Island']]
        me.answer({'kind':'choose','answer':{'objects':milled}})
        d = me.last['decision']
        self.assertEqual((d['kind'], d['count'], d['reason']), ('objects', 1, 'resolution'))
        self.assertEqual({me.last['objects'][o]['name'] for o in d['among']}, {'Forest', 'Island', 'Mountain', 'Plains'})
        self.assertIn(ring, me.last['battlefield'], 'mode two has not executed while mode one awaits its return')
        island = self.named(me.last, 'Island', zone='graveyard')[0]
        me.answer({'kind': 'choose', 'answer': {'objects': [island]}})
        p = me.prompt()
        self.assertNotIn(ring, p['battlefield'])
        self.assertEqual(len(self.named(p, 'Sol Ring', zone='graveyard')), 1)
        self.assertEqual(len(self.named(p, 'Island', zone='hand')), 1)

    def test_egon_requires_the_whole_cost_and_offers_no_partial_payment(self):
        self.engine.send({'cmd': 'new', 'players': [
            {'library': ['Island'] * 5, 'battlefield': ['Egon, God of Death'],
             'graveyard': ['Forest', 'Swamp', 'Mountain']},
            {'library': ['Island'] * 5}]})
        self.seats = [play.Seat(self.engine, i) for i in range(2)]
        me, them = self.seats
        for _ in range(16):
            seat = self.waiting()
            if seat.last['decision']['kind'] != 'priority':
                break
            seat.answer({'kind': 'pass'})
        else:
            self.fail('no upkeep cost choice')
        self.assertIs(seat, me)
        d = me.last['decision']
        self.assertEqual((d['kind'], d['count'], d['reason']), ('objects', 2, 'resolution'))
        self.assertEqual({me.last['objects'][o]['name'] for o in d['among']}, {'Forest', 'Swamp', 'Mountain'})
        self.assertIsNone(them.prompt()['decision'])
        request = me.last['request_id']
        error = self.refused(me, {'cmd': 'answer', 'answer': {'kind': 'choose', 'answer': {'objects': d['among'][:1]}}})
        self.assertTrue(error)
        self.assertEqual(me.prompt()['request_id'], request)
        self.assertEqual(me.last['decision']['among'], d['among'])
        self.assertEqual(me.last['exile'], [])
        me.answer({'kind': 'choose', 'answer': {'objects': d['among'][:2]}})
        p = me.prompt()
        self.assertEqual(len(self.named(p, 'Egon, God of Death', zone='battlefield')), 1)
        self.assertEqual(len(p['exile']), 2)
        self.assertEqual(len(p['players'][0]['graveyard']), 1)
        # With only one card the cost is impossible: keep it, sacrifice Egon and draw.
        me, them = self.new({'battlefield': ['Egon, God of Death'], 'graveyard': ['Forest']}, {})
        p = me.prompt()
        self.assertEqual(self.named(p, 'Egon, God of Death', zone='battlefield'), [])
        self.assertEqual(len(self.named(p, 'Egon, God of Death', zone='graveyard')), 1)
        self.assertEqual(len(self.named(p, 'Forest', zone='graveyard')), 1)
        self.assertEqual(p['exile'], [])
        self.assertEqual(len(p['players'][0]['hand']), 1)

    def test_wormfang_newt_returns_the_land_chosen_by_its_entry_ability(self):
        me, them = self.new({'hand': ['Wormfang Newt'], 'battlefield': ['Forest', 'Island']},
                           {'hand': ['Lightning Bolt'], 'battlefield': ['Mountain']})
        p = me.prompt()
        lands = self.named(p, 'Forest') + self.named(p, 'Island')
        me.answer({'mana': {'G': 1, 'U': 1}, 'kind': 'cast', 'card': self.named(p, 'Wormfang Newt')[0],
                   'plan': [{'ability': f'{o}.a0'} for o in lands]})
        self.assertIs(self.until_choice(), me)
        self.assertEqual(set(me.last['decision']['among']), set(lands))
        forest = self.named(me.last, 'Forest', zone='battlefield')[0]
        me.answer({'kind': 'choose', 'answer': {'objects': [forest]}})
        p = me.prompt()
        self.assertNotIn(forest, p['battlefield'])
        self.assertEqual(len(self.named(p, 'Forest', zone='exile')), 1)
        me.answer({'kind': 'pass'})
        p = them.prompt()
        them.answer({'mana': {'R': 1}, 'kind': 'cast', 'card': self.named(p, 'Lightning Bolt')[0],
                     'targets': [[self.named(p, 'Wormfang Newt')[0]]],
                     'plan': [{'ability': f"{self.named(p, 'Mountain')[0]}.a0"}]})
        self.clear_stack()
        p = me.prompt()
        returned = self.named(p, 'Forest', zone='battlefield')[0]
        self.assertNotEqual(returned, forest, '400.7: the returned card is a new permanent')
        self.assertEqual(p['objects'][returned]['controller'], 'p0')
        self.assertEqual(self.named(p, 'Forest', zone='exile'), [])


if __name__ == '__main__':
    unittest.main()
