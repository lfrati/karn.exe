"""Checks through player/1 of effect-made death replacements."""
import os
import unittest

import play
import test_player_prompt_client as fixtures


class DeathExileReviewClientTests(unittest.TestCase):
    new = fixtures.PlayerPromptClientTests.new
    waiting = fixtures.PlayerPromptClientTests.waiting
    named = staticmethod(fixtures.PlayerPromptClientTests.named)
    close_engine = fixtures.PlayerPromptClientTests.close_engine

    @classmethod
    def setUpClass(cls):
        if not os.environ.get('DEATH_EXILE_REVIEW_ENGINE'):
            fixtures.PlayerPromptClientTests.setUpClass()

    def setUp(self):
        self.engine = play.Engine(executable=os.environ.get('DEATH_EXILE_REVIEW_ENGINE'))
        self.addCleanup(self.close_engine)

    def pause(self):
        for _ in range(50):
            seat = self.waiting()
            d = seat.last['decision']
            if d['kind'] != 'priority' or not seat.last['stack']:
                return seat
            seat.answer({'kind': 'pass'})
        self.fail('resolution did not settle')

    def cast(self, seat, name, lands=(), *, spend, **fields):
        p = seat.prompt()
        mana = []
        for land, count in lands:
            mana.extend([o for o in self.named(p, land, controller=seat.label, zone='battlefield')
                         if not p['objects'][o]['tapped']][:count])
        return seat.answer({'kind': 'cast', 'card': self.named(p, name, zone='hand')[0],
                            'plan': [{'ability': f'{o}.a0'} for o in mana], 'mana': spend, **fields})

    def coil_question(self):
        me, them = self.new({'battlefield': ['Mountain', 'Mountain'], 'hand': ['Lava Coil']},
                            {'battlefield': ["Nissa's Chosen"]})
        victim = self.named(me.prompt(), "Nissa's Chosen", zone='battlefield')[0]
        self.cast(me, 'Lava Coil', [('Mountain', 2)], targets=[[victim]], spend={'R': 2})
        self.assertIs(self.pause(), them)
        return me, them, them.last['decision']

    def test_a_death_replacement_is_not_reported_as_putting_death_counters(self):
        # Rules: 614.1a, 616.1; player/1 decision context.
        _, _, d = self.coil_question()
        self.assertEqual((d['event'], d['counter']), ('death', None), d)

    def test_lava_coil_asks_its_victims_controller_and_accepts_both_destinations(self):
        # Rules: 614.1a, 616.1, 400.7
        for exile in (False, True):
            with self.subTest(exile=exile):
                me, them, d = self.coil_question()
                self.assertEqual((d['kind'], d['chooser']), ('replacement', 'p1'))
                own_victim = self.named(them.last, "Nissa's Chosen", zone='battlefield')[0]
                self.assertEqual(d['on'], own_victim)
                self.assertEqual({r['name'] for r in d['among']}, {"Nissa's Chosen", 'Lava Coil'})
                self.assertIsNone(me.prompt()['decision'])
                effect = next(r for r in d['among'] if (r['name'] == 'Lava Coil') == exile)
                with self.assertRaises(SystemExit):
                    me.answer({'kind': 'choose', 'answer': {'replacement':
                               {k: effect[k] for k in ('source', 'ability', 'part')}}})
                them.answer({'kind': 'choose', 'answer': {'replacement':
                             {k: effect[k] for k in ('source', 'ability', 'part')}}})
                self.pause()
                p = them.prompt()
                self.assertFalse(self.named(p, "Nissa's Chosen", zone='battlefield'))
                self.assertFalse(self.named(p, "Nissa's Chosen", zone='graveyard'))
                if exile:
                    self.assertEqual(len(self.named(p, "Nissa's Chosen", zone='exile')), 1)
                else:
                    self.assertEqual(p['players'][1]['library'], 6)
                    self.assertFalse(self.named(p, "Nissa's Chosen", zone='exile'))

    def test_bleed_dry_exiles_an_indestructible_creature_at_zero_toughness(self):
        # Rules: 704.5f, 702.12b, 614.1a, 700.4
        me, them = self.new({'battlefield': ['Swamp'] * 4, 'hand': ['Bleed Dry']},
                            {'battlefield': ['Darksteel Myr', 'Morbid Opportunist']})
        victim = self.named(me.prompt(), 'Darksteel Myr', zone='battlefield')[0]
        before = them.prompt()['players'][1]['hand_size']
        self.cast(me, 'Bleed Dry', [('Swamp', 4)], targets=[[victim]], spend={'B': 4})
        self.pause()
        p = them.prompt()
        self.assertEqual(len(self.named(p, 'Darksteel Myr', zone='exile')), 1)
        self.assertFalse(self.named(p, 'Darksteel Myr', zone='graveyard'))
        self.assertEqual(p['players'][1]['hand_size'], before, 'replacement means it did not die')

    def test_magma_sprays_replacement_remains_when_all_damage_is_prevented(self):
        # Rules: 615.6, 614.1a, 614.4
        me, them = self.new({'battlefield': ['Mountain', 'Mountain', 'Swamp'],
                            'hand': ['Magma Spray', 'Terminate']},
                            {'battlefield': ['Hill Giant', 'Plains'], 'hand': ['Healing Salve']})
        giant = self.named(me.prompt(), 'Hill Giant', zone='battlefield')[0]
        self.cast(me, 'Magma Spray', [('Mountain', 1)], targets=[[giant]], spend={'R': 1})
        me.answer({'kind': 'pass'})
        self.assertIs(self.waiting(), them)
        their_giant = self.named(them.last, 'Hill Giant', zone='battlefield')[0]
        self.cast(them, 'Healing Salve', [('Plains', 1)], modes=[1], targets=[[their_giant]], spend={'W': 1})
        self.pause()
        p = me.prompt()
        self.assertIn(giant, p['objects'])
        self.assertEqual(p['objects'][giant]['damage'], 0)
        self.cast(me, 'Terminate', [('Mountain', 1), ('Swamp', 1)], targets=[[giant]], spend={'R': 1, 'B': 1})
        self.pause()
        p = me.prompt()
        self.assertEqual(len(self.named(p, 'Hill Giant', zone='exile')), 1)
        self.assertFalse(self.named(p, 'Hill Giant', zone='graveyard'))


if __name__ == '__main__':
    unittest.main()
