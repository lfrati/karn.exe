"""Tishana's counter identifies an ability's source across managed player prompts."""
import os
import unittest

import play
import test_player_prompt_client as fixtures


class CounterSourceClientTests(unittest.TestCase):
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
        for _ in range(20):
            seat = self.waiting()
            if not seat.last['stack']:
                return
            self.assertEqual(seat.last['decision']['kind'], 'priority')
            seat.answer({'kind': 'pass'})
        self.fail('stack did not clear')

    def staff_and_tide(self):
        me, them = self.new({'hand': ["Tishana's Tidebinder", 'Unsummon'], 'battlefield': ['Island'] * 4},
                            {'battlefield': ['Staff of Domination', 'Island', 'Island']})
        me.answer({'kind': 'pass'})
        p = them.prompt()
        staff = self.named(p, 'Staff of Domination')[0]
        them.answer({'kind': 'activate', 'ability': f'{staff}.a0',
                     'plan': [{'ability': f"{self.named(p, 'Island', controller='p1')[0]}.a0"}], 'mana': {'U': 1}})
        them.answer({'kind': 'pass'})
        p = me.prompt()
        me.answer({'kind': 'cast', 'card': self.named(p, "Tishana's Tidebinder")[0],
                   'plan': [{'ability': f'{island}.a0'} for island in self.named(p, 'Island', controller='p0')[:3]],
                   'mana': {'U': 3}})
        self.pass_until_asked()
        p = me.prompt()
        self.assertEqual(p['decision']['kind'], 'targets')
        ability = p['stack'][0]
        self.assertEqual(p['objects'][ability]['occurrence']['source_name'], 'Staff of Domination')
        me.answer({'kind': 'targets', 'targets': [[ability]]})
        return me, them

    def return_tide(self, me):
        p = me.prompt()
        tide = self.named(p, "Tishana's Tidebinder", zone='battlefield')[0]
        land = [o for o in self.named(p, 'Island', controller='p0') if not p['objects'][o]['tapped']][0]
        me.answer({'kind': 'cast', 'card': self.named(p, 'Unsummon')[0], 'targets': [[tide]],
                   'plan': [{'ability': f'{land}.a0'}], 'mana': {'U': 1}})

    def test_counter_blanks_only_its_source_and_departure_restores_activation_labels(self):
        # CR 701.6a, 611.2b, 400.7: no untap effect resolves; the artifact stays,
        # its ability labels disappear, and they return when this Tidebinder leaves.
        me, them = self.staff_and_tide()
        self.clear_stack()
        p = me.prompt()
        staff = self.named(p, 'Staff of Domination')[0]
        self.assertEqual(p['objects'][staff].get('abilities', []), [])
        self.assertEqual(len(self.named(p, 'Staff of Domination', zone='battlefield')), 1)
        self.return_tide(me)
        self.clear_stack()
        p = them.prompt()
        staff = self.named(p, 'Staff of Domination')[0]
        self.assertEqual(len(p['objects'][staff]['abilities']), 5)
        self.waiting().answer({'kind': 'pass'})
        p = them.prompt()
        land = [o for o in self.named(p, 'Island', controller='p1') if not p['objects'][o]['tapped']][0]
        them.answer({'kind': 'activate', 'ability': f'{staff}.a0',
                     'plan': [{'ability': f'{land}.a0'}], 'mana': {'U': 1}})
        self.clear_stack()
        self.assertEqual(len(them.prompt()['objects'][staff]['abilities']), 5)

    def test_departure_before_resolution_still_counters_but_never_blanks_the_source(self):
        # Pinned Tidebinder ruling 2023-11-10; CR 611.2b: a duration that never
        # begins applies no continuous change. The counter still succeeds.
        me, them = self.staff_and_tide()
        self.return_tide(me)
        self.clear_stack()
        p = them.prompt()
        staff = self.named(p, 'Staff of Domination')[0]
        self.assertEqual(len(p['objects'][staff]['abilities']), 5)
        self.assertEqual(p['stack'], [])
        self.assertEqual(p['players'][1]['life'], 20)

    def test_sower_source_presence_also_ends_control_on_its_first_departure(self):
        # CR 611.2b, 613.1b: the same source-present duration governs Sower's
        # control effect. Its departure restores the earlier controller immediately.
        me, them = self.new({'hand': ['Sower of Temptation', 'Unsummon'], 'battlefield': ['Island'] * 5},
                            {'battlefield': ['Grizzly Bears']})
        p = me.prompt()
        me.answer({'kind': 'cast', 'card': self.named(p, 'Sower of Temptation')[0],
                   'plan': [{'ability': f'{o}.a0'} for o in self.named(p, 'Island', controller='p0')[:4]], 'mana': {'U': 4}})
        self.pass_until_asked()
        p = me.prompt()
        bear = self.named(p, 'Grizzly Bears', zone='battlefield')[0]
        me.answer({'kind': 'targets', 'targets': [[bear]]})
        self.clear_stack()
        p = me.prompt()
        self.assertEqual(p['objects'][bear]['controller'], 'p0')
        source = self.named(p, 'Sower of Temptation', zone='battlefield')[0]
        land = [o for o in self.named(p, 'Island', controller='p0') if not p['objects'][o]['tapped']][0]
        me.answer({'kind': 'cast', 'card': self.named(p, 'Unsummon')[0], 'targets': [[source]],
                   'plan': [{'ability': f'{land}.a0'}], 'mana': {'U': 1}})
        self.clear_stack()
        self.assertEqual(me.prompt()['objects'][bear]['controller'], 'p1')
