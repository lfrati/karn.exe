"""Populate decisions and complete consumers through play.Seat."""
import os
import unittest
import play
import test_player_prompt_client as fixtures


class PopulateReviewClientTests(unittest.TestCase):
    new = fixtures.PlayerPromptClientTests.new
    waiting = fixtures.PlayerPromptClientTests.waiting
    named = staticmethod(fixtures.PlayerPromptClientTests.named)
    close_engine = fixtures.PlayerPromptClientTests.close_engine

    @classmethod
    def setUpClass(cls):
        if not os.environ.get('POPULATE_REVIEW_ENGINE'):
            fixtures.PlayerPromptClientTests.setUpClass()

    def setUp(self):
        self.engine = play.Engine(executable=os.environ.get('POPULATE_REVIEW_ENGINE'))
        self.addCleanup(self.close_engine)

    def cast(self, seat, name, mana, *, spend, **fields):
        p = seat.prompt()
        lands = []
        for land, count in mana:
            lands.extend([o for o in self.named(p, land, controller=seat.label, zone='battlefield')
                          if not p['objects'][o]['tapped']][:count])
        return seat.answer({'kind': 'cast', 'card': self.named(p, name, zone='hand')[0],
                            'plan': [{'ability': f'{o}.a0'} for o in lands], 'mana': spend, **fields})

    def pause(self):
        for _ in range(40):
            seat = self.waiting()
            if seat.last['decision']['kind'] != 'priority' or not seat.last['stack']:
                return seat
            seat.answer({'kind': 'pass'})
        self.fail('stack did not settle')

    @staticmethod
    def tokens(p, player='p0'):
        return {o: v for o, v in p['objects'].items()
                if v.get('token') and v.get('controller') == player and v.get('zone') == 'battlefield'}

    def test_eyes_can_populate_the_bird_it_just_created_and_rootborn_covers_the_new_copy(self):
        # Rules: 701.36a, 608.2d, 707.2, 611.2c
        me, them = self.new({'hand': ['Raise the Alarm', 'Eyes in the Skies', 'Rootborn Defenses'],
                             'battlefield': ['Plains'] * 9 + ['Grizzly Bears', 'Intangible Virtue']},
                            {'hand': ['Volcanic Fallout'], 'battlefield': ['Mountain'] * 3 + ['Grizzly Bears']})
        self.cast(me, 'Raise the Alarm', [('Plains', 2)], spend={'W': 2})
        self.pause()
        self.cast(me, 'Eyes in the Skies', [('Plains', 4)], spend={'W': 4})
        chooser = self.pause()
        self.assertIs(chooser, me)
        d = chooser.last['decision']
        self.assertEqual((d['kind'], d['verb'], d['count'], d['reason']),
                         ('objects', 'CreateCopy', 1, 'resolution'))
        self.assertEqual(d['for']['source_name'], 'Eyes in the Skies')
        self.assertEqual(len(d['among']), 3)
        bird = next(o for o in d['among'] if 'Bird' in chooser.last['objects'][o]['subtypes'])
        bear = self.named(chooser.last, 'Grizzly Bears')[0]
        with self.assertRaises(SystemExit):
            me.answer({'kind': 'choose', 'answer': {'objects': [bear]}})
        me.prompt()
        with self.assertRaises(SystemExit):
            them.prompt()
            them.answer({'kind': 'choose', 'answer': {'objects': [bird]}})
        self.assertEqual(me.prompt()['decision']['among'], d['among'])
        me.answer({'kind': 'choose', 'answer': {'objects': [bird]}})
        self.pause()
        p = me.prompt()
        self.assertEqual(len(self.tokens(p)), 4)
        birds = [v for v in self.tokens(p).values() if 'Bird' in v['subtypes']]
        self.assertEqual(len(birds), 2)
        self.assertTrue(all(v['power'] == 2 and v['toughness'] == 2 for v in birds))
        self.cast(me, 'Rootborn Defenses', [('Plains', 3)], spend={'W': 3})
        chooser = self.pause()
        self.assertEqual(len(chooser.last['decision']['among']), 4)
        soldier = next(o for o in chooser.last['decision']['among']
                       if 'Soldier' in chooser.last['objects'][o]['subtypes'])
        me.answer({'kind': 'choose', 'answer': {'objects': [soldier]}})
        self.pause()
        p = me.prompt()
        self.assertEqual(len(self.tokens(p)), 5)
        me.answer({'kind': 'pass'})
        self.cast(them, 'Volcanic Fallout', [('Mountain', 3)], spend={'R': 3})
        self.pause()
        p = me.prompt()
        self.assertEqual(len(self.tokens(p)), 5, 'the new populated token is protected too')
        self.assertEqual(len(self.named(p, 'Grizzly Bears', controller='p0', zone='battlefield')), 1)
        self.assertFalse(self.named(p, 'Grizzly Bears', controller='p1', zone='battlefield'))

    def test_rootborn_still_grants_indestructible_with_no_creature_token(self):
        # Rules: 701.36b, 608.2c
        me, them = self.new({'hand': ['Rootborn Defenses'],
                             'battlefield': ['Plains'] * 3 + ['Grizzly Bears']},
                            {'hand': ['Volcanic Fallout'], 'battlefield': ['Mountain'] * 3 + ['Grizzly Bears']})
        self.cast(me, 'Rootborn Defenses', [('Plains', 3)], spend={'W': 3})
        seat = self.pause()
        self.assertEqual(seat.last['decision']['kind'], 'priority')
        p = me.prompt()
        self.assertFalse(self.tokens(p))
        me.answer({'kind': 'pass'})
        self.cast(them, 'Volcanic Fallout', [('Mountain', 3)], spend={'R': 3})
        self.pause()
        p = me.prompt()
        self.assertEqual(len(self.named(p, 'Grizzly Bears', controller='p0', zone='battlefield')), 1)
        self.assertFalse(self.named(p, 'Grizzly Bears', controller='p1', zone='battlefield'))

    def test_trostanis_judgment_can_remove_your_last_token_before_populating(self):
        # Rules: 701.36b, 608.2c
        me, them = self.new({'hand': ['Selesnya Charm', "Trostani's Judgment"],
                             'battlefield': ['Forest'] + ['Plains'] * 7 + ['Grizzly Bears']},
                            {'battlefield': ['Grizzly Bears']})
        self.cast(me, 'Selesnya Charm', [('Forest', 1), ('Plains', 1)], modes=[2], spend={'G': 1, 'W': 1})
        self.pause()
        p = me.prompt()
        [knight] = self.tokens(p)
        self.cast(me, "Trostani's Judgment", [('Plains', 6)], targets=[[knight]], spend={'W': 6})
        seat = self.pause()
        self.assertEqual(seat.last['decision']['kind'], 'priority')
        p = me.prompt()
        self.assertFalse(self.tokens(p), 'the only token left before the populate instruction')
        self.assertEqual(len(self.named(p, 'Grizzly Bears', zone='battlefield')), 2)
