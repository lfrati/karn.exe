"""107.3m through the client: entry X belongs to the spell the permanent was.
Rules: 107.3m, 107.3g, 113.7a, 400.7, 608.2h, 708.2a, 708.4, 708.6.
"""
import unittest
import test_player_prompt_client as fixtures
import play


class SpellXEntryReviewClientTests(unittest.TestCase):
    setUpClass = classmethod(fixtures.PlayerPromptClientTests.setUpClass.__func__)
    new = fixtures.PlayerPromptClientTests.new
    waiting = fixtures.PlayerPromptClientTests.waiting
    named = staticmethod(fixtures.PlayerPromptClientTests.named)
    close_engine = fixtures.PlayerPromptClientTests.close_engine

    def setUp(self):
        self.engine = play.Engine()
        self.addCleanup(self.close_engine)

    def resolve_top(self):
        stack = self.seats[0].prompt()['stack']
        self.assertTrue(stack)
        before = stack[-1]
        for _ in range(10):
            seat = self.waiting()
            self.assertEqual(seat.last['decision']['kind'], 'priority')
            seat.answer({'kind': 'pass'})
            if before not in self.seats[0].prompt()['stack']:
                return
        self.fail('top stack object did not resolve')

    def test_meathook_keeps_x_after_disenchant_removes_its_source(self):
        me, them = self.new({'hand': ['The Meathook Massacre'], 'battlefield': ['Swamp'] * 4},
                            {'hand': ['Disenchant'], 'battlefield': ['Plains'] * 2 + ['Grizzly Bears'] * 2})
        p = me.prompt()
        me.answer({'mana': {'B': 4}, 'kind': 'cast', 'card': self.named(p, 'The Meathook Massacre')[0], 'x': 2,
                   'plan': [{'ability': f'{o}.a0'} for o in self.named(p, 'Swamp')]})
        # The spell becomes an enchantment, then its enters ability waits on the stack.
        me.answer({'kind': 'pass'})
        them.prompt()
        them.answer({'kind': 'pass'})
        p = me.prompt()
        self.assertEqual(len(p['stack']), 1)
        self.assertEqual(p['objects'][p['stack'][0]]['occurrence']['kind'], 'triggered')
        me.answer({'kind': 'pass'})
        p = them.prompt()
        massacre = self.named(p, 'The Meathook Massacre', zone='battlefield')[0]
        them.answer({'mana': {'W': 2}, 'kind': 'cast', 'card': self.named(p, 'Disenchant')[0], 'targets': [[massacre]],
                     'plan': [{'ability': f'{o}.a0'} for o in self.named(p, 'Plains')]})
        self.resolve_top()
        p = me.prompt()
        self.assertEqual(self.named(p, 'The Meathook Massacre', zone='battlefield'), [])
        self.assertEqual(len(p['stack']), 1, '113.7a: removing the source does not remove its ability')
        self.resolve_top()
        p = them.prompt()
        self.assertEqual(p['decision']['kind'],'order_cards')
        bears=self.named(p,'Grizzly Bears',zone='battlefield')
        them.answer({'kind':'choose','answer':{'objects':bears}})
        p = me.prompt()
        self.assertEqual(self.named(p, 'Grizzly Bears', zone='battlefield'), [])
        self.assertEqual(len(self.named(p, 'Grizzly Bears', zone='graveyard')), 2,
                         '107.3m/608.2h: the departed source still supplies X=2')
        self.assertEqual([v['life'] for v in p['players']], [20, 20],
                         'the Massacre was absent when the Bears died, so its death triggers do not trigger')

    def test_marketback_xx_cost_has_one_entry_x_and_death_draws_by_counters(self):
        me, them = self.new({'hand': ['Marketback Walker'], 'battlefield': ['Forest'] * 4},
                            {'hand': ['Disenchant'], 'battlefield': ['Plains'] * 2})
        p = me.prompt()
        me.answer({'mana': {'G': 4}, 'kind': 'cast', 'card': self.named(p, 'Marketback Walker')[0], 'x': 2,
                   'plan': [{'ability': f'{o}.a0'} for o in self.named(p, 'Forest')]})
        self.resolve_top()
        p = me.prompt()
        walker = self.named(p, 'Marketback Walker', zone='battlefield')[0]
        self.assertEqual(p['objects'][walker]['counters'], {'+1/+1': 2})
        self.assertEqual(p['objects'][walker]['mana_cost'], '{X}{X}')
        me.answer({'kind': 'pass'})
        p = them.prompt()
        them.answer({'mana': {'W': 2}, 'kind': 'cast', 'card': self.named(p, 'Disenchant')[0],
                     'targets': [[self.named(p, 'Marketback Walker', zone='battlefield')[0]]],
                     'plan': [{'ability': f'{o}.a0'} for o in self.named(p, 'Plains')]})
        self.resolve_top()
        self.resolve_top()
        p = me.prompt()
        self.assertEqual(len(p['players'][0]['hand']), 2, 'the last known two counters cause two draws')

    def test_hooded_hydra_cast_face_down_has_no_printed_entry_replacement(self):
        me, them = self.new({'hand': ['Hooded Hydra'], 'battlefield': ['Forest'] * 8}, {})
        p = me.prompt()
        hydra = self.named(p, 'Hooded Hydra')[0]
        forests = self.named(p, 'Forest')
        morph_row = next(i for i, text in enumerate(p['objects'][hydra]['text']) if text.startswith('Morph'))
        me.answer({'mana': {'G': 3}, 'kind': 'cast', 'card': hydra, 'cost': {'alternative': morph_row},
                   'plan': [{'ability': f'{o}.a0'} for o in forests[:3]]})
        self.resolve_top()
        p = me.prompt()
        down = [o for o in p['battlefield'] if 'face_down' in p['objects'][o]]
        self.assertEqual(len(down), 1)
        self.assertEqual(p['objects'][down[0]]['counters'], {}, '708.2a: no printed enters-with-counters ability')
        self.assertEqual(p['stack'], [], '708.4: turning face up is not entering')
        me.answer({'mana': {'G': 5}, 'kind': 'turn_face_up', 'permanent': down[0], 'way': 'morph',
                   'plan': [{'ability': f'{o}.a0'} for o in forests[3:]]})
        p = me.prompt()
        face = self.named(p, 'Hooded Hydra', zone='battlefield')[0]
        self.assertEqual(p['objects'][face]['counters'], {'+1/+1': 5})
        self.assertEqual(p['stack'], [])


if __name__ == '__main__':
    unittest.main()
