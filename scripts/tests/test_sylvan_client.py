"""Sylvan's private drawn-card choices and separate life payments through real seats."""
import os
import unittest

import play
import test_player_prompt_client as fixtures


class SylvanClientTests(unittest.TestCase):
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

    def start(self, controller=0):
        players = [{'library': ['Forest', 'Island', 'Mountain', 'Plains'] * 5} for _ in range(2)]
        players[controller].update(battlefield=['Sylvan Library'], hand=['Memnite'])
        self.new(*players)
        self.draw_step(controller)
        self.pass_until_asked()
        owner = self.seats[controller]
        self.assertEqual(owner.last['decision']['kind'], 'may')
        return owner, self.seats[1-controller]

    def draw_step(self, active):
        for _ in range(300):
            seat = self.waiting()
            p = seat.last
            if p['part'] == 'Draw' and p['active'] == f'p{active}' and p['stack']:
                return
            kind = p['decision']['kind']
            if kind == 'priority':
                seat.answer({'kind': 'pass'})
            elif kind == 'attackers':
                seat.answer({'kind': 'attack', 'attacks': []})
            elif kind == 'blockers':
                seat.answer({'kind': 'block', 'blocks': []})
            else:
                self.fail(f'unexpected turn decision: {p["decision"]}')
        self.fail('no draw-step trigger')

    def clear_stack(self):
        for _ in range(30):
            seat = self.waiting()
            if not seat.last['stack']:
                return
            self.assertEqual(seat.last['decision']['kind'], 'priority')
            seat.answer({'kind': 'pass'})
        self.fail('stack did not clear')

    def chosen_pair(self, owner):
        p = owner.answer({'kind': 'choose', 'answer': 'yes'})
        decision = p['decision']
        self.assertEqual((decision['kind'], decision['count']), ('objects', 2))
        self.assertEqual(len(decision['among']), 3)
        old = self.named(p, 'Memnite', zone='hand')[0]
        self.assertNotIn(old, decision['among'])
        selected = decision['among'][:2]
        # The old hand card is a valid handle but is outside the drawn-card domain.
        self.assertTrue(self.refused(owner, {'cmd': 'answer', 'answer': {
            'kind': 'choose', 'answer': {'objects': [old, selected[0]]}}}))
        self.assertEqual(owner.prompt()['decision']['among'], decision['among'])
        p = owner.answer({'kind': 'choose', 'answer': {'objects': selected}})
        self.assertEqual((p['decision']['kind'], p['decision']['count']), ('objects', 1))
        self.assertEqual(set(p['decision']['among']), set(selected))
        return selected

    def test_private_selection_is_complete_before_individual_costs_and_payment_keeps_only_that_card(self):
        # Pinned Sylvan rulings 2022-12-08; CR 121.1,119.4,608.2c-d:
        # three actual draws are eligible, then each chosen card has its own cost.
        owner, opponent = self.start()
        pair = self.chosen_pair(owner)
        p = owner.answer({'kind': 'choose', 'answer': {'objects': [pair[1]]}})
        self.assertEqual((p['decision']['kind'], p['decision']['life']), ('pay', 4))
        self.assertIsNone(opponent.prompt()['decision'])
        for name in ['Forest', 'Island', 'Mountain', 'Plains']:
            self.assertEqual(self.named(opponent.last, name, controller='p0', zone='hand'), [])
        owner.prompt()
        p = owner.answer({'kind': 'choose', 'answer': {'pay': {'plan': [], 'mana': {}}}})
        self.assertEqual(p['players'][0]['life'], 16)
        self.assertEqual((p['decision']['kind'], p['decision']['life']), ('pay', 4))
        p = owner.answer({'kind': 'choose', 'answer': 'no'})
        self.assertEqual(p['decision']['kind'], 'priority')
        self.assertEqual(p['players'][0]['life'], 16)
        self.assertIn(pair[1], p['players'][0]['hand'])
        self.assertNotIn(pair[0], p['players'][0]['hand'])
        self.assertEqual(len(p['players'][0]['hand']), 3)

    def test_the_other_players_sylvan_asks_that_player_and_decline_has_no_card_or_payment_choice(self):
        # CR 109.5,504.1,118.12: "your" means the ability's controller;
        # declining the two additional draws leaves the normal draw in hand.
        owner, opponent = self.start(controller=1)
        self.assertEqual(owner.last['waiting_for'], 'p1')
        self.assertEqual(len(owner.last['players'][1]['hand']), 2)
        p = owner.answer({'kind': 'choose', 'answer': 'no'})
        self.assertEqual(p['decision']['kind'], 'priority')
        self.assertEqual((len(p['players'][1]['hand']), p['players'][1]['life']), (2, 20))

    def test_returning_both_cards_uses_the_chosen_per_card_order_without_losing_life(self):
        # CR 401.4,608.2c: return the selected first card, then the other;
        # no life is paid when both payment opportunities are declined.
        owner, _ = self.start()
        pair = self.chosen_pair(owner)
        owner.answer({'kind': 'choose', 'answer': {'objects': [pair[0]]}})
        owner.answer({'kind': 'choose', 'answer': 'no'})
        p = owner.answer({'kind': 'choose', 'answer': 'no'})
        self.assertEqual(p['decision']['kind'], 'priority')
        self.assertEqual((len(p['players'][0]['hand']), p['players'][0]['life']), (2, 20))
        self.assertFalse(any(card in p['players'][0]['hand'] for card in pair))

    def test_howling_mine_checks_untapped_again_and_the_active_player_draws_the_extra_card(self):
        # CR 603.4,504.1: Mine checks its intervening condition again on
        # resolution; Icy's actual activation can stop the extra draw in response.
        for tap in [False, True]:
            with self.subTest(tap=tap):
                me, them = self.new({'battlefield': ['Icy Manipulator', 'Island']},
                                    {'battlefield': ['Howling Mine']})
                self.draw_step(1)
                if tap:
                    them.answer({'kind': 'pass'})
                    p = me.prompt()
                    me.answer({'kind': 'activate', 'ability': f"{self.named(p, 'Icy Manipulator')[0]}.a0",
                               'targets': [[self.named(p, 'Howling Mine')[0]]],
                               'plan': [{'ability': f"{self.named(p, 'Island', controller='p0')[0]}.a0"}],
                               'mana': {'U': 1}})
                self.clear_stack()
                p = them.prompt()
                self.assertEqual(len(p['players'][1]['hand']), 1 if tap else 2)
                self.assertEqual(p['objects'][self.named(p, 'Howling Mine')[0]]['tapped'], tap)

    def test_anvil_draws_before_its_discard_and_can_discard_a_card_that_was_already_in_hand(self):
        # CR 608.2c-d,701.9a: the additional draw is a separate ordinary draw,
        # followed by the same player's mandatory discard from their whole hand.
        _, owner = self.new({'battlefield': ['Anvil of Bogardan'],
                          'library': ['Island'] * 10}, {'library': ['Forest'] * 10, 'hand': ['Memnite']})
        self.draw_step(1)
        self.pass_until_asked()
        p = owner.prompt()
        self.assertEqual((p['decision']['kind'], p['decision']['count']), ('objects', 1))
        old = self.named(p, 'Memnite', zone='hand')[0]
        self.assertEqual(len(p['players'][1]['hand']), 3)
        self.assertIn(old, p['decision']['among'])
        p = owner.answer({'kind': 'choose', 'answer': {'objects': [old]}})
        self.assertEqual(len(p['players'][1]['hand']), 2)
        self.assertEqual(len(self.named(p, 'Memnite', zone='graveyard')), 1)

    def test_well_of_ideas_draws_one_for_other_players_and_two_for_its_controller(self):
        # CR 109.5,504.1: each trigger keeps its own player and quantity;
        # neither additional-draw instruction repeats the turn-based draw.
        me, them = self.new({'battlefield': ['Well of Ideas'], 'library': ['Island'] * 12},
                            {'library': ['Forest'] * 12})
        before = len(me.prompt()['players'][0]['hand'])
        self.draw_step(1); self.clear_stack()
        self.assertEqual(len(them.prompt()['players'][1]['hand']), 2)
        self.draw_step(0); self.clear_stack()
        self.assertEqual(len(me.prompt()['players'][0]['hand']), before + 3)
