"""Real-seat checks of declaration counts and captured attacker groups."""
import os
import unittest

import play
import test_player_prompt_client as fixtures


class AttackCountReviewClientTests(unittest.TestCase):
    new = fixtures.PlayerPromptClientTests.new
    waiting = fixtures.PlayerPromptClientTests.waiting
    named = staticmethod(fixtures.PlayerPromptClientTests.named)
    close_engine = fixtures.PlayerPromptClientTests.close_engine

    @classmethod
    def setUpClass(cls):
        if not os.environ.get('ATTACK_COUNT_REVIEW_ENGINE'):
            fixtures.PlayerPromptClientTests.setUpClass()

    def setUp(self):
        self.engine = play.Engine(executable=os.environ.get('ATTACK_COUNT_REVIEW_ENGINE'))
        self.addCleanup(self.close_engine)

    def until(self, kind):
        for _ in range(40):
            seat = self.waiting()
            d = seat.last['decision']
            if d['kind'] == kind:
                return seat
            if d['kind'] == 'order':
                seat.answer({'kind': 'order', 'order': list(range(len(d['ordered'])))})
            else:
                self.assertEqual(d['kind'], 'priority', d)
                seat.answer({'kind': 'pass'})
        self.fail(f'never asked {kind}')

    def clear_stack(self):
        for _ in range(40):
            seat = self.waiting()
            d = seat.last['decision']
            if d['kind'] == 'order':
                seat.answer({'kind': 'order', 'order': list(range(len(d['ordered'])))})
            elif not seat.last['stack']:
                return
            else:
                self.assertEqual(d['kind'], 'priority', d)
                seat.answer({'kind': 'pass'})
        self.fail('stack did not clear')

    def cast(self, seat, name, land, target, spend):
        p = seat.prompt()
        source = next(o for o in self.named(p, land, controller=seat.label, zone='battlefield')
                      if not p['objects'][o]['tapped'])
        return seat.answer({'kind': 'cast', 'card': self.named(p, name, zone='hand')[0],
                            'targets': [[target]], 'mana': spend, 'plan': [{'ability': f'{source}.a0'}]})

    def test_military_intelligence_triggers_once_and_keeps_the_declared_threshold(self):
        # Rules: 508.3d, 603.2c, 603.4
        me, them = self.new({'battlefield': ['Military Intelligence', 'Grizzly Bears',
                                           'Ornithopter', 'Llanowar Elves', 'Island']},
                            {'hand': ['Unsummon'], 'battlefield': ['Island']})
        self.assertIs(self.until('attackers'), me)
        p = me.last
        attackers = [self.named(p, n)[0] for n in ['Grizzly Bears', 'Ornithopter', 'Llanowar Elves']]
        me.answer({'kind': 'attack', 'attacks': [[o, 'p1'] for o in attackers]})
        p = me.prompt()
        self.assertEqual(len(p['stack']), 1, 'one trigger for the declaration, not three')
        self.assertEqual(p['objects'][p['stack'][0]]['occurrence']['source_name'], 'Military Intelligence')
        me.answer({'kind': 'pass'})
        their_bear = self.named(them.prompt(), 'Grizzly Bears')[0]
        self.cast(them, 'Unsummon', 'Island', their_bear, {'U': 1})
        self.clear_stack()
        p = me.prompt()
        self.assertEqual(len(self.named(p, 'Grizzly Bears', zone='hand')), 1)
        self.assertEqual(len(p['players'][0]['hand']), 2, 'bounced Bear plus exactly one drawn card')

    def test_firemane_battalion_is_captured_before_another_attacker_dies(self):
        # Rules: 508.3a, 603.3d, 603.4, 608.2b
        me, them = self.new({'battlefield': ['Firemane Avenger', 'Grizzly Bears', 'Ornithopter']},
                            {'hand': ['Lightning Bolt'], 'battlefield': ['Mountain']})
        self.assertIs(self.until('attackers'), me)
        p = me.last
        attackers = [self.named(p, n)[0] for n in ['Firemane Avenger', 'Grizzly Bears', 'Ornithopter']]
        me.answer({'kind': 'attack', 'attacks': [[o, 'p1'] for o in attackers]})
        p = me.prompt()
        self.assertEqual((p['decision']['kind'], p['decision']['reason']), ('targets', 'trigger'))
        self.assertEqual(p['decision']['for']['source_name'], 'Firemane Avenger')
        self.assertIsNone(them.prompt()['decision'])
        me.answer({'kind': 'targets', 'targets': [['p1']]})
        me.answer({'kind': 'pass'})
        their_bear = self.named(them.prompt(), 'Grizzly Bears')[0]
        self.cast(them, 'Lightning Bolt', 'Mountain', their_bear, {'R': 1})
        self.clear_stack()
        p = me.prompt()
        self.assertEqual([x['life'] for x in p['players']], [23, 17])
        self.assertEqual(len(self.named(p, 'Grizzly Bears', zone='graveyard')), 1)

    def test_sigil_excludes_its_equipped_attacker_and_needs_a_single_declaration(self):
        # Rules: 506.5, 508.3a, 608.2k
        for alone in [True, False]:
            with self.subTest(alone=alone):
                me, them = self.new({'battlefield': ['Sigil of Valor', 'Rogue Kavu',
                                                   'Llanowar Elves', 'Llanowar Elves', 'Forest']}, {})
                p = me.prompt()
                sigil = self.named(p, 'Sigil of Valor')[0]
                kavu = self.named(p, 'Rogue Kavu')[0]
                forest = self.named(p, 'Forest')[0]
                ability = p['objects'][sigil]['abilities'][0]['label']
                me.answer({'kind': 'activate', 'ability': ability, 'targets': [[kavu]], 'mana': {'G': 1},
                           'plan': [{'ability': f'{forest}.a0'}]})
                self.clear_stack()
                self.assertIs(self.until('attackers'), me)
                p = me.last
                attackers = [kavu]
                if not alone:
                    attackers.append(self.named(p, 'Llanowar Elves')[0])
                me.answer({'kind': 'attack', 'attacks': [[o, 'p1'] for o in attackers]})
                self.clear_stack()
                p = me.prompt()
                self.assertEqual((p['objects'][kavu]['power'], p['objects'][kavu]['toughness']),
                                 (5, 3) if alone else (1, 1))

    def test_gideon_binds_the_non_gideon_attacking_group_only(self):
        # Rules: 508.3d, 608.2k
        me, them = self.new({'battlefield': ['Gideon, the Oathsworn', 'Grizzly Bears',
                                           'Llanowar Elves', 'Ornithopter', 'Grizzly Bears']}, {})
        p = me.prompt()
        gideon = self.named(p, 'Gideon, the Oathsworn')[0]
        ability = next(a['label'] for a in p['objects'][gideon]['abilities'] if 'becomes a 5/5' in a['text'])
        me.answer({'kind': 'activate', 'ability': ability})
        self.clear_stack()
        self.assertIs(self.until('attackers'), me)
        p = me.last
        bear, home = self.named(p, 'Grizzly Bears')
        elf = self.named(p, 'Llanowar Elves')[0]
        thopter = self.named(p, 'Ornithopter')[0]
        me.answer({'kind': 'attack', 'attacks': [[o, 'p1'] for o in [gideon, bear, elf, thopter]]})
        self.clear_stack()
        p = me.prompt()
        for o in [bear, elf, thopter]:
            self.assertEqual(p['objects'][o]['counters'].get('+1/+1'), 1)
        for o in [gideon, home]:
            self.assertNotIn('+1/+1', p['objects'][o]['counters'])

    def test_ninjutsu_entry_does_not_add_another_attack_count_trigger(self):
        # Rules: 508.3a/d, 508.4, 702.49a
        me, them = self.new({'hand': ['Kaito, Bane of Nightmares'],
                             'battlefield': ['Military Intelligence', 'Rogue Kavu',
                                             'Ornithopter', 'Island', 'Swamp', 'Forest']}, {})
        self.assertIs(self.until('attackers'), me)
        p = me.last
        kavu = self.named(p, 'Rogue Kavu')[0]
        thopter = self.named(p, 'Ornithopter')[0]
        me.answer({'kind': 'attack', 'attacks': [[kavu, 'p1'], [thopter, 'p1']]})
        self.clear_stack()
        self.assertEqual(len(me.prompt()['players'][0]['hand']), 2, 'Kaito and one draw')
        for _ in range(20):
            seat = self.waiting()
            if seat.last['part'] == 'DeclareBlockers' and seat.last['decision']['kind'] == 'priority':
                break
            self.assertEqual(seat.last['decision']['kind'], 'priority')
            seat.answer({'kind': 'pass'})
        self.assertIs(seat, me)
        p = me.prompt()
        kaito = self.named(p, 'Kaito, Bane of Nightmares', zone='hand')[0]
        ability = f'{kaito}.a5'  # Kaito's printed ninjutsu ability (702.49a).
        plan = [{'ability': f'{self.named(p, n)[0]}.a0'} for n in ['Island', 'Swamp', 'Forest']]
        me.answer({'kind': 'activate', 'ability': ability, 'paying': [[kavu]], 'plan': plan, 'mana': {'U': 1, 'B': 1, 'G': 1}})
        self.clear_stack()
        p = me.prompt()
        arrived = self.named(p, 'Kaito, Bane of Nightmares', zone='battlefield')[0]
        self.assertNotEqual(arrived, kaito)
        self.assertTrue(p['objects'][arrived]['tapped'])
        self.assertEqual(set(p['attacking']), {arrived, thopter})
        self.assertEqual(len(p['players'][0]['hand']), 2, 'returned Kavu and one draw; no new attack trigger')

    def test_an_extra_combat_uses_its_own_declaration_count(self):
        # Rules: 500.8, 508.3d
        me, them = self.new({'hand': ['Seize the Day'],
                             'battlefield': ['Military Intelligence', 'Serra Angel', 'Serra Angel']
                                            + ['Mountain'] * 4}, {})
        p = me.prompt()
        me.answer({'kind': 'cast', 'card': self.named(p, 'Seize the Day')[0],
                   'targets': [[self.named(p, 'Serra Angel')[0]]], 'mana': {'R': 4},
                   'plan': [{'ability': f'{o}.a0'} for o in self.named(p, 'Mountain')]})
        self.clear_stack()
        self.assertIs(self.until('attackers'), me)
        angels = self.named(me.last, 'Serra Angel')
        me.answer({'kind': 'attack', 'attacks': [[o, 'p1'] for o in angels]})
        self.clear_stack()
        self.assertEqual(len(me.prompt()['players'][0]['hand']), 1)
        self.assertIs(self.until('attackers'), me)
        self.assertEqual(me.last['turn'], 1, 'a second combat in the same turn')
        me.answer({'kind': 'attack', 'attacks': [[angels[0], 'p1']]})
        p = me.prompt()
        self.assertEqual(p['stack'], [], 'the previous two-attacker declaration cannot meet this threshold')
        self.assertEqual(len(p['players'][0]['hand']), 1)


if __name__ == '__main__':
    unittest.main()
