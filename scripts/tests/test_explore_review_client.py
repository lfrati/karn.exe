"""Explore composition through labelled player prompts and answers."""
import os
import unittest
import play
import test_player_prompt_client as fixtures


class ExploreReviewClientTests(unittest.TestCase):
    new = fixtures.PlayerPromptClientTests.new
    waiting = fixtures.PlayerPromptClientTests.waiting
    named = staticmethod(fixtures.PlayerPromptClientTests.named)
    close_engine = fixtures.PlayerPromptClientTests.close_engine

    @classmethod
    def setUpClass(cls):
        if not os.environ.get('EXPLORE_REVIEW_ENGINE'):
            fixtures.PlayerPromptClientTests.setUpClass()

    def setUp(self):
        self.engine = play.Engine(executable=os.environ.get('EXPLORE_REVIEW_ENGINE'))
        self.addCleanup(self.close_engine)
        self.seen_reveals = []

    def cast(self, seat, name, mana, *, spend, **fields):
        p = seat.prompt()
        lands = []
        for land, count in mana:
            lands.extend([o for o in self.named(p, land, controller=seat.label, zone='battlefield')
                          if not p['objects'][o]['tapped']][:count])
        return seat.answer({'kind': 'cast', 'card': self.named(p, name, zone='hand')[0],
                            'plan': [{'ability': f'{o}.a0'} for o in lands], 'mana': spend, **fields})

    def pause(self):
        for _ in range(30):
            seat = self.waiting()
            for other in self.seats:
                self.seen_reveals.extend(o['name'] for obs in other.last.get('observations', [])
                                         if obs['kind'] == 'revealed' for o in obs['cards'])
            if seat.last['decision']['kind'] != 'priority' or not seat.last['stack']:
                return seat
            seat.answer({'kind': 'pass'})
        self.fail('stack did not settle')

    def test_a_scout_reveals_a_land_publicly_and_triggers_the_walker_without_a_scout_counter(self):
        # Rules: 701.44a-b, 701.20a, 402.3
        me, them = self.new({'hand': ['Cenote Scout'], 'battlefield': ['Forest', 'Wildgrowth Walker'],
                             'library': ['Lightning Bolt', 'Forest']}, {})
        self.cast(me, 'Cenote Scout', [('Forest', 1)], spend={'G': 1})
        self.pause()
        p = me.prompt()
        scout = self.named(p, 'Cenote Scout', zone='battlefield')[0]
        walker = self.named(p, 'Wildgrowth Walker')[0]
        self.assertEqual(p['objects'][scout]['counters'].get('+1/+1', 0), 0)
        self.assertEqual(p['objects'][walker]['counters']['+1/+1'], 1)
        self.assertEqual(p['players'][0]['life'], 23)
        self.assertEqual(len(self.named(p, 'Forest', zone='hand')), 1)
        q = them.prompt()
        self.assertIn('Forest', self.seen_reveals)
        self.assertFalse(self.named(q, 'Lightning Bolt'), 'the unrevealed next library card stays private')

    def test_repeated_activations_can_keep_the_same_public_nonland_with_counter_replacement(self):
        # Rules: 701.44a-b, 614.1a, 701.20a
        me, them = self.new({'battlefield': ['Seeker of Sunlight', 'Wildgrowth Walker',
                                            'Hardened Scales'] + ['Forest'] * 6,
                             'library': ['Island', 'Lightning Bolt']}, {})
        for _ in range(2):
            p = me.prompt()
            seeker = self.named(p, 'Seeker of Sunlight')[0]
            lands = [o for o in self.named(p, 'Forest') if not p['objects'][o]['tapped']][:3]
            me.answer({'kind': 'activate', 'ability': f'{seeker}.a0', 'mana': {'G': 3},
                       'plan': [{'ability': f'{o}.a0'} for o in lands]})
            chooser = self.pause()
            self.assertIs(chooser, me)
            self.assertEqual(chooser.last['decision']['kind'], 'may')
            revealed = them.prompt()
            self.assertIsNone(revealed['decision'])
            self.assertIn('Lightning Bolt', self.seen_reveals)
            self.assertFalse(self.named(revealed, 'Island', zone='library'))
            me.answer({'kind': 'choose', 'answer': 'no'})
            self.pause()
        p = me.prompt()
        for name in ['Seeker of Sunlight', 'Wildgrowth Walker']:
            self.assertEqual(p['objects'][self.named(p, name)[0]]['counters']['+1/+1'], 4)
        self.assertEqual(p['players'][0]['life'], 26)
        self.assertEqual(p['players'][0]['library'], 2)

    def test_a_bounced_scout_still_explores_its_last_controllers_library(self):
        # Rules: 701.44c, 608.2h, 400.7
        me, them = self.new({'hand': ['Cenote Scout'], 'battlefield': ['Forest', 'Wildgrowth Walker'],
                             'library': ['Island', 'Lightning Bolt']},
                            {'hand': ['Unsummon'], 'battlefield': ['Island'], 'library': ['Forest'] * 5})
        self.cast(me, 'Cenote Scout', [('Forest', 1)], spend={'G': 1})
        me.answer({'kind': 'pass'})
        them.prompt()
        them.answer({'kind': 'pass'})
        p = me.prompt()
        self.assertTrue(p['stack'])
        me.answer({'kind': 'pass'})
        p = them.prompt()
        scout = self.named(p, 'Cenote Scout', zone='battlefield')[0]
        self.cast(them, 'Unsummon', [('Island', 1)], targets=[[scout]], spend={'U': 1})
        chooser = self.pause()
        self.assertIs(chooser, me)
        self.assertEqual(chooser.last['decision']['kind'], 'may')
        self.assertFalse(self.named(chooser.last, 'Cenote Scout', zone='battlefield'))
        me.answer({'kind': 'choose', 'answer': 'yes'})
        self.pause()
        p = me.prompt()
        self.assertEqual(len(self.named(p, 'Lightning Bolt', zone='graveyard')), 1)
        self.assertEqual(len(self.named(p, 'Cenote Scout', zone='hand')), 1)
        self.assertEqual(p['players'][0]['life'], 23)
        self.assertEqual(p['players'][1]['library'], 5)

    def test_an_empty_library_still_explores_and_the_path_triggers_for_a_new_creature(self):
        # Rules: 701.44a-b, 603.2
        me, them = self.new({'hand': ['Grizzly Bears'], 'battlefield': ['Forest'] * 2 +
                             ['Path of Discovery', 'Wildgrowth Walker'], 'library': []}, {})
        self.cast(me, 'Grizzly Bears', [('Forest', 2)], spend={'G': 2})
        self.pause()
        p = me.prompt()
        bear = self.named(p, 'Grizzly Bears', zone='battlefield')[0]
        self.assertEqual(p['objects'][bear]['counters']['+1/+1'], 1)
        self.assertEqual(p['players'][0]['life'], 23)
        self.assertFalse(self.named(p, 'Grizzly Bears', zone='graveyard'))

    def test_the_revealed_nonland_stays_visible_while_its_graveyard_choice_is_pending(self):
        # Rules: 701.44a, 701.20a
        me, them = self.new({'battlefield': ['Seeker of Sunlight'] + ['Forest'] * 3,
                             'library': ['Island', 'Lightning Bolt']}, {})
        p = me.prompt()
        seeker = self.named(p, 'Seeker of Sunlight')[0]
        me.answer({'kind': 'activate', 'ability': f'{seeker}.a0', 'mana': {'G': 3},
                   'plan': [{'ability': f'{o}.a0'} for o in self.named(p, 'Forest')]})
        chooser = self.pause()
        self.assertEqual(chooser.last['decision']['kind'], 'may')
        self.assertIn('Lightning Bolt', self.seen_reveals)
        for seat in [me, them]:
            with self.subTest(seat=seat.label):
                p = seat.prompt()
                self.assertEqual(len(self.named(p, 'Lightning Bolt', zone='library')), 1,
                                 '701.20a: the card is relevant until the graveyard choice completes')
                self.assertFalse(self.named(p, 'Island', zone='library'))
        me.answer({'kind': 'choose', 'answer': 'no'})
        self.pause()
        for seat in [me, them]:
            self.assertFalse(self.named(seat.prompt(), 'Lightning Bolt', zone='library'),
                             'after completion the library card stops being revealed')
