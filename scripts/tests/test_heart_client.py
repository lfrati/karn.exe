"""Heart's explicit normal/loyalty Crew cost choices through real player seats."""
import os
import unittest

import play
import test_player_prompt_client as fixtures


class HeartClientTests(unittest.TestCase):
    new = fixtures.PlayerPromptClientTests.new
    waiting = fixtures.PlayerPromptClientTests.waiting
    named = staticmethod(fixtures.PlayerPromptClientTests.named)
    close_engine = fixtures.PlayerPromptClientTests.close_engine
    refused = fixtures.PlayerPromptClientTests.refused

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

    def action(self, p, holders, alternative):
        heart = self.named(p, 'Heart of Kiran', controller='p0')[0]
        return {'kind': 'activate', 'ability': f'{heart}.a3', 'targets': [],
                'chose': [int(alternative)], 'paying': [holders]}

    def test_one_of_two_controlled_planeswalkers_is_chosen_explicitly_and_the_normal_crew_cost_remains_available(self):
        # Pinned Heart ruling2017-02-09; CR602.2b,606.2,702.122:
        # loyalty removal is a resource cost and does not activate that planeswalker.
        me, them = self.new({'battlefield': ['Heart of Kiran', 'Jace Beleren', 'Liliana of the Veil',
                                           'Grizzly Bears', 'Memnite']}, {'battlefield': ['Jace Beleren']})
        p = me.prompt()
        jace = self.named(p, 'Jace Beleren', controller='p0')[0]
        liliana = self.named(p, 'Liliana of the Veil')[0]
        their_jace = self.named(p, 'Jace Beleren', controller='p1')[0]
        bad = self.action(p, [their_jace], True)
        self.assertTrue(self.refused(me, {'cmd': 'answer', 'answer': bad}))
        p = me.prompt()
        self.assertEqual(p['objects'][jace]['counters']['loyalty'], 3)
        p = me.answer(self.action(p, [liliana], True))
        self.assertEqual(p['objects'][liliana]['counters']['loyalty'], 2)
        self.assertEqual(p['objects'][jace]['counters']['loyalty'], 3)
        self.assertFalse(p['objects'][liliana]['tapped'])
        self.clear_stack()
        p = me.prompt()
        heart = self.named(p, 'Heart of Kiran')[0]
        self.assertEqual((p['objects'][heart]['power'], p['objects'][heart]['toughness']), (4,4))
        bears = self.named(p, 'Grizzly Bears')[0]
        memnite = self.named(p, 'Memnite')[0]
        p = me.answer(self.action(p, [bears, memnite], False))
        self.assertTrue(p['objects'][bears]['tapped'] and p['objects'][memnite]['tapped'])
        self.assertEqual(p['objects'][liliana]['counters']['loyalty'], 2)
        self.clear_stack()

    def test_counter_payment_repeats_after_jaces_loyalty_ability_was_already_activated(self):
        # CR606.2/3: the same turn's Jace use neither enables nor limits Heart's
        # separate activated ability, and a second Jace use is still forbidden.
        me, _ = self.new({'battlefield': ['Heart of Kiran', 'Jace Beleren'], 'library': ['Forest'] * 10},
                         {'library': ['Island'] * 10})
        p = me.prompt()
        jace = self.named(p, 'Jace Beleren')[0]
        me.answer({'kind': 'activate', 'ability': f'{jace}.a0', 'targets': []})
        self.clear_stack()
        for remaining in [4, 3]:
            p = me.prompt()
            p = me.answer(self.action(p, [jace], True))
            self.assertEqual(p['objects'][jace]['counters']['loyalty'], remaining)
        self.assertEqual(len(p['stack']), 2)
        self.clear_stack()
        p = me.prompt()
        self.assertIn('already been activated', self.refused(me, {'cmd': 'answer', 'answer': {
            'kind': 'activate', 'ability': f'{jace}.a0', 'targets': []}}))

    def test_a_declared_resource_and_branch_are_required_and_rejected_inputs_leave_counters_untouched(self):
        # CR601.2b/h,733.1: neither the branch nor the planeswalker is chosen
        # by the engine; empty/excess resources and ordinary creatures cannot pay it.
        me, _ = self.new({'battlefield': ['Heart of Kiran', 'Jace Beleren', 'Grizzly Bears']}, {})
        p = me.prompt()
        jace = self.named(p, 'Jace Beleren')[0]
        bears = self.named(p, 'Grizzly Bears')[0]
        for holders in [[], [jace, bears], [bears]]:
            self.assertTrue(self.refused(me, {'cmd': 'answer', 'answer': self.action(p, holders, True)}))
            p = me.prompt()
            self.assertEqual(p['objects'][jace]['counters']['loyalty'], 3)
            self.assertFalse(p['objects'][bears]['tapped'])
        missing_branch = self.action(p, [jace], True)
        del missing_branch['chose']
        self.assertIn('alternative', self.refused(me, {'cmd': 'answer', 'answer': missing_branch}))
        p = me.prompt()
        me.answer(self.action(p, [jace], True))
        self.clear_stack()

    def test_quillspike_uses_the_same_selected_counter_resource_with_its_ordinary_mana_cost(self):
        # CR118.3,122.1,702.79a: persist supplies an actual -1/-1 counter;
        # removing it pays Quillspike's cost before the pump ability resolves.
        me, _ = self.new({'battlefield': ['Quillspike', 'Kitchen Finks', 'Forest', 'Mountain'],
                          'hand': ['Lightning Bolt'], 'library': ['Island'] * 8},
                         {'library': ['Island'] * 8})
        p = me.prompt()
        finks = self.named(p, 'Kitchen Finks')[0]
        bolt = self.named(p, 'Lightning Bolt', zone='hand')[0]
        mountain = self.named(p, 'Mountain')[0]
        me.answer({'kind': 'cast', 'card': bolt, 'targets': [[finks]],
                   'plan': [{'ability': f'{mountain}.a0'}], 'mana': {'R': 1}})
        self.clear_stack()
        p = me.prompt()
        returned = self.named(p, 'Kitchen Finks')[0]
        self.assertNotEqual(returned, finks)
        self.assertEqual(p['objects'][returned]['counters']['-1/-1'], 1)
        quill = self.named(p, 'Quillspike')[0]
        forest = self.named(p, 'Forest')[0]
        p = me.answer({'kind': 'activate', 'ability': f'{quill}.a0', 'targets': [], 'paying': [[returned]],
                       'plan': [{'ability': f'{forest}.a0'}], 'mana': {'G': 1}})
        self.assertNotIn('-1/-1', p['objects'][returned].get('counters', {}))
        self.assertEqual(p['objects'][quill]['power'], 1)
        self.clear_stack()
        p = me.prompt()
        self.assertEqual((p['objects'][quill]['power'], p['objects'][quill]['toughness']), (4,4))
