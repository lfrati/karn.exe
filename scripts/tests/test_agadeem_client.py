"""Agadeem's explicit X, graveyard targets and distinct-value constraint through real seats."""
import os
import unittest

import play
import test_player_prompt_client as fixtures


class AgadeemClientTests(unittest.TestCase):
    new = fixtures.PlayerPromptClientTests.new
    waiting = fixtures.PlayerPromptClientTests.waiting
    named = staticmethod(fixtures.PlayerPromptClientTests.named)
    close_engine = fixtures.PlayerPromptClientTests.close_engine
    refused = fixtures.PlayerPromptClientTests.refused
    pass_until_asked = fixtures.PlayerPromptClientTests.pass_until_asked

    @classmethod
    def setUpClass(cls):
        fixtures.PlayerPromptClientTests.setUpClass()

    def setUp(self):
        self.engine = play.Engine(executable=os.environ.get('GAME_SERVE_REVIEW'))
        self.addCleanup(self.close_engine)

    def clear_stack(self):
        for _ in range(30):
            seat = self.waiting()
            if not seat.last['stack']:
                return
            self.assertEqual(seat.last['decision']['kind'], 'priority')
            seat.answer({'kind': 'pass'})
        self.fail('stack did not clear')

    def setup_card(self, swamps=6, redirect=False):
        return self.new({'hand': ["Agadeem's Awakening"] + (['Redirect'] if redirect else []),
                         'battlefield': ['Swamp'] * swamps + (['Island'] * 2 if redirect else []),
                         'library': ['Forest'] * 8,
                         'graveyard': ['Memnite', 'Walking Ballista', 'Llanowar Elves', 'Grizzly Bears',
                                       'Dusk Legion Zealot', 'Centaur Courser']},
                        {'graveyard': ['Grizzly Bears']})

    def cast_action(self, prompt, x, chosen):
        return {'kind': 'cast', 'card': self.named(prompt, "Agadeem's Awakening", zone='hand')[0], 'x': x,
                'targets': [[self.named(prompt, name, controller='p0', zone='graveyard')[0] for name in chosen]],
                'plan': [{'ability': f'{o}.a0'} for o in self.named(prompt, 'Swamp', controller='p0')[:x+3]],
                'mana': {'B': x+3}}

    def test_x_three_accepts_values_zero_to_three_and_refuses_a_duplicate_value_without_tapping_mana(self):
        # Pinned Agadeem ruling 2020-09-25; CR 601.2c, 733.1: Any Number is
        # bounded by distinct mana values, so 0,1,2,3 are four legal targets.
        me, them = self.setup_card()
        p = me.prompt()
        bad = self.cast_action(p, 3, ['Grizzly Bears', 'Dusk Legion Zealot'])
        self.assertIn('different mana values', self.refused(me, {'cmd': 'answer', 'answer': bad}))
        p = me.prompt()
        self.assertTrue(all(not p['objects'][o]['tapped'] for o in self.named(p, 'Swamp', controller='p0')))
        self.assertEqual(len(self.named(p, "Agadeem's Awakening", zone='hand')), 1)
        me.answer(self.cast_action(p, 3, ['Memnite', 'Llanowar Elves', 'Dusk Legion Zealot', 'Centaur Courser']))
        self.clear_stack()
        p = me.prompt()
        for name in ['Memnite', 'Llanowar Elves', 'Dusk Legion Zealot', 'Centaur Courser']:
            self.assertEqual(len(self.named(p, name, controller='p0', zone='battlefield')), 1)
        self.assertEqual(len(self.named(p, 'Walking Ballista', zone='graveyard')), 1)
        self.assertEqual(len(self.named(p, 'Grizzly Bears', controller='p0', zone='graveyard')), 1)
        self.assertEqual(len(self.named(p, 'Forest', zone='hand')), 1)
        self.assertEqual(p['players'][0]['life'], 19)

    def test_x_zero_accepts_one_zero_value_card_and_walking_ballistas_x_does_not_add_value_in_the_graveyard(self):
        # CR 107.3g, 202.3e: Ballista's {X}{X} has mana value zero here, just
        # like Memnite's {0}; the two cannot both be chosen at X=0.
        me, them = self.setup_card(swamps=3)
        p = me.prompt()
        bad = self.cast_action(p, 0, ['Memnite', 'Walking Ballista'])
        self.assertIn('different mana values', self.refused(me, {'cmd': 'answer', 'answer': bad}))
        me.answer(self.cast_action(me.prompt(), 0, ['Memnite']))
        self.clear_stack()
        p = me.prompt()
        self.assertEqual(len(self.named(p, 'Memnite', zone='battlefield')), 1)
        self.assertEqual(len(self.named(p, 'Walking Ballista', zone='graveyard')), 1)
        self.assertEqual(p['players'][0]['life'], 20)

    def test_redirect_exposes_partitions_and_validates_the_supplied_final_group(self):
        # CR 115.7d-e: changing a target retains X and the two chosen target
        # positions, then checks the final set rather than fixing a supplied answer.
        me, them = self.setup_card(swamps=5, redirect=True)
        p = me.prompt()
        me.answer(self.cast_action(p, 2, ['Memnite', 'Grizzly Bears']))
        p = me.prompt()
        spell = p['stack'][0]
        me.answer({'kind': 'cast', 'card': self.named(p, 'Redirect', zone='hand')[0], 'targets': [[spell]],
                   'plan': [{'ability': f'{o}.a0'} for o in self.named(p, 'Island', controller='p0')], 'mana': {'U': 2}})
        self.pass_until_asked()
        p = me.prompt()
        self.assertEqual(p['decision']['kind'], 'change_targets')
        slot = p['decision']['slots'][0]
        self.assertEqual((slot['min'], slot['max']), (2,2))
        bear = self.named(p, 'Grizzly Bears', controller='p0', zone='graveyard')[0]
        zealot = self.named(p, 'Dusk Legion Zealot', zone='graveyard')[0]
        self.assertTrue(any(set(partition) == {bear,zealot} for partition in slot['different_mana_values']))
        self.assertIn('changed target must be legal', self.refused(me, {'cmd':'answer','answer':{
            'kind':'choose','answer':{'targets':[[bear,zealot]]}}}))
        p = me.prompt()
        memnite = self.named(p, 'Memnite', zone='graveyard')[0]
        me.answer({'kind':'choose','answer':{'targets':[[memnite,zealot]]}})
        self.clear_stack()
        p = me.prompt()
        self.assertEqual(len(self.named(p, 'Memnite', zone='battlefield')), 1)
        self.assertEqual(len(self.named(p, 'Dusk Legion Zealot', zone='battlefield')), 1)
        self.assertEqual(len(self.named(p, 'Grizzly Bears', controller='p0', zone='graveyard')), 1)
