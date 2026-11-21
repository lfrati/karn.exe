"""Static prevention through complete cards and labelled seat decisions."""
import os
import unittest
import play
import test_player_prompt_client as fixtures


class StaticPreventionReviewClientTests(unittest.TestCase):
    new = fixtures.PlayerPromptClientTests.new
    waiting = fixtures.PlayerPromptClientTests.waiting
    named = staticmethod(fixtures.PlayerPromptClientTests.named)
    close_engine = fixtures.PlayerPromptClientTests.close_engine

    @classmethod
    def setUpClass(cls):
        if not os.environ.get('STATIC_PREVENTION_REVIEW_ENGINE'):
            fixtures.PlayerPromptClientTests.setUpClass()

    def setUp(self):
        self.engine = play.Engine(executable=os.environ.get('STATIC_PREVENTION_REVIEW_ENGINE'))
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

    def until(self, predicate):
        for _ in range(50):
            seat = self.waiting()
            if predicate(seat.last):
                return seat
            self.assertEqual(seat.last['decision']['kind'], 'priority', seat.last['decision'])
            seat.answer({'kind': 'pass'})
        self.fail('requested step was not reached')

    def test_two_static_sources_offer_distinct_handles_and_leave_a_shield_unspent(self):
        # Rules: 615.1, 615.7, 616.1
        me, them = self.new({'hand': ['Shock'] * 3 + ['Naturalize'] * 2,
                             'battlefield': ['Mountain'] * 5 + ['Forest'] * 2},
                            {'hand': ['Healing Salve'],
                             'battlefield': ['Plains', 'Blessed Sanctuary', 'Blessed Sanctuary']})
        self.cast(me, 'Shock', [('Mountain', 1)], targets=[['p1']], spend={'R': 1})
        me.answer({'kind': 'pass'})
        self.cast(them, 'Healing Salve', [('Plains', 1)], modes=[1], targets=[['p1']], spend={'W': 1})
        chooser = self.pause()
        self.assertIs(chooser, them)
        d = chooser.last['decision']
        self.assertEqual((d['kind'], d['event'], d['on']), ('replacement', 'damage_prevention', 'p1'))
        static = [o for o in d['among'] if o['kind'] == 'prevent_all']
        self.assertEqual(len(static), 2)
        self.assertEqual(len({o['source'] for o in static}), 2)
        self.assertEqual({o['name'] for o in static}, {'Blessed Sanctuary'})
        self.assertEqual(len([o for o in d['among'] if o['kind'] == 'shield']), 1)
        choice = {k: v for k, v in static[1].items() if k not in ('name', 'remaining', 'damage_source')}
        them.answer({'kind': 'choose', 'answer': {'damage_order': choice}})
        self.pause()
        self.assertEqual(them.prompt()['players'][1]['life'], 20)
        for _ in range(2):
            p = me.prompt()
            sanctuary = self.named(p, 'Blessed Sanctuary', zone='battlefield')[0]
            self.cast(me, 'Naturalize', [('Forest', 1), ('Mountain', 1)], targets=[[sanctuary]], spend={'G': 1, 'R': 1})
            self.pause()
        self.cast(me, 'Shock', [('Mountain', 1)], targets=[['p1']], spend={'R': 1})
        self.pause()
        self.assertEqual(them.prompt()['players'][1]['life'], 20, 'Salve still has all three points before this hit')
        self.cast(me, 'Shock', [('Mountain', 1)], targets=[['p1']], spend={'R': 1})
        self.pause()
        self.assertEqual(them.prompt()['players'][1]['life'], 19)

    def test_an_aura_protects_its_enchanted_creature_until_the_aura_leaves(self):
        # Rules: 615.1, 611.3a
        me, them = self.new({'hand': ['Inviolability', 'Shock', 'Shock', 'Naturalize'],
                             'battlefield': ['Plains'] * 2 + ['Forest'] + ['Mountain'] * 3},
                            {'battlefield': ['Grizzly Bears']})
        bear = self.named(me.prompt(), 'Grizzly Bears')[0]
        self.cast(me, 'Inviolability', [('Plains', 2)], targets=[[bear]], spend={'W': 2})
        self.pause()
        self.cast(me, 'Shock', [('Mountain', 1)], targets=[[bear]], spend={'R': 1})
        self.pause()
        self.assertEqual(me.prompt()['objects'][bear]['damage'], 0)
        aura = self.named(me.last, 'Inviolability', zone='battlefield')[0]
        self.cast(me, 'Naturalize', [('Forest', 1), ('Mountain', 1)], targets=[[aura]], spend={'G': 1, 'R': 1})
        self.pause()
        self.cast(me, 'Shock', [('Mountain', 1)], targets=[[bear]], spend={'R': 1})
        self.pause()
        self.assertFalse(self.named(me.prompt(), 'Grizzly Bears', zone='battlefield'))

    def test_skullcrack_bypasses_dawns_prevention(self):
        # Rules: 615.1, 615.12
        me, them = self.new({'hand': ['Shock', 'Shock', 'Skullcrack'],
                             'battlefield': ['Mountain'] * 4},
                            {'battlefield': ['Dawn Elemental']})
        dawn = self.named(me.prompt(), 'Dawn Elemental')[0]
        self.cast(me, 'Shock', [('Mountain', 1)], targets=[[dawn]], spend={'R': 1})
        self.pause()
        self.assertEqual(me.prompt()['objects'][dawn]['damage'], 0)
        self.cast(me, 'Skullcrack', [('Mountain', 2)], targets=[['p1']], spend={'R': 2})
        self.pause()
        self.cast(me, 'Shock', [('Mountain', 1)], targets=[[dawn]], spend={'R': 1})
        self.pause()
        self.assertEqual(me.prompt()['objects'][dawn]['damage'], 2)

    def test_fog_bank_prevents_combat_but_is_killed_by_a_later_shock(self):
        # Rules: 615.1, 120.2a-b
        me, them = self.new({'hand': ['Shock'], 'battlefield': ['Hill Giant', 'Mountain']},
                            {'battlefield': ['Fog Bank']})
        self.until(lambda p: p['decision']['kind'] == 'attackers')
        giant = self.named(me.last, 'Hill Giant')[0]
        me.answer({'kind': 'attack', 'attacks': [[giant, 'p1']]})
        chooser = self.until(lambda p: p['decision']['kind'] == 'blockers')
        p = chooser.last
        bank = self.named(p, 'Fog Bank')[0]
        giant = self.named(p, 'Hill Giant')[0]
        them.answer({'kind': 'block', 'blocks': [[bank, giant]]})
        self.until(lambda p: p['part'] == 'PostcombatMain')
        p = me.prompt()
        bank = self.named(p, 'Fog Bank')[0]
        self.assertEqual(p['objects'][bank]['damage'], 0)
        self.cast(me, 'Shock', [('Mountain', 1)], targets=[[bank]], spend={'R': 1})
        self.pause()
        self.assertFalse(self.named(me.prompt(), 'Fog Bank', zone='battlefield'))

    def test_caduceus_granted_prevention_ends_when_the_controller_drops_below_thirty(self):
        # Rules: 615.1, 611.3a, 613.1f
        self.engine.send({'cmd': 'new', 'life': 30, 'players': [
            {'library': ['Island'] * 5, 'hand': ['Shock'] * 3, 'battlefield': ['Mountain'] * 3},
            {'library': ['Island'] * 5, 'battlefield': ['Plains'] * 2 +
             ['Grizzly Bears', 'Caduceus, Staff of Hermes']}]})
        self.seats = [play.Seat(self.engine, i) for i in range(2)]
        me, them = self.seats
        for _ in range(60):
            seat = self.waiting()
            if seat.last['part'] == 'PrecombatMain' and seat.last['active'] == 'p1':
                break
            kind = seat.last['decision']['kind']
            if kind == 'attackers':
                seat.answer({'kind': 'attack', 'attacks': []})
            elif kind == 'blockers':
                seat.answer({'kind': 'block', 'blocks': []})
            else:
                self.assertEqual(kind, 'priority')
                seat.answer({'kind': 'pass'})
        else:
            self.fail('player1 main phase not reached')
        p = them.prompt()
        bear = self.named(p, 'Grizzly Bears')[0]
        staff = self.named(p, 'Caduceus, Staff of Hermes')[0]
        equip = next(a['label'] for a in p['objects'][staff]['abilities'] if 'Attach' in a['text'])
        them.answer({'kind': 'activate', 'ability': equip, 'targets': [[bear]], 'mana': {'W': 2},
                     'plan': [{'ability': f'{o}.a0'} for o in self.named(p, 'Plains')]})
        self.pause()
        them.prompt()
        them.answer({'kind': 'pass'})
        p = me.prompt()
        bear = self.named(p, 'Grizzly Bears')[0]
        self.cast(me, 'Shock', [('Mountain', 1)], targets=[[bear]], spend={'R': 1})
        self.pause()
        self.assertEqual(me.prompt()['objects'][bear]['damage'], 0)
        them.answer({'kind': 'pass'})
        # Once Shock lowers its controller's life, the conditional grant disappears.
        self.cast(me, 'Shock', [('Mountain', 1)], targets=[['p1']], spend={'R': 1})
        self.pause()
        self.assertEqual(me.prompt()['players'][1]['life'], 28)
        them.answer({'kind': 'pass'})
        self.cast(me, 'Shock', [('Mountain', 1)], targets=[[bear]], spend={'R': 1})
        self.pause()
        self.assertFalse(self.named(me.prompt(), 'Grizzly Bears', zone='battlefield'))
