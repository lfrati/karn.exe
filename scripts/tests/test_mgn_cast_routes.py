"""Casting evidence follows CR 601.2b/h/i, 702.51/66/81/180 and 707.10.

Frames deliberately name independently selected resources and allocations; output
must preserve those facts and refuse changed evidence without touching a frame.
"""
from copy import deepcopy
from pathlib import Path
import unittest

from mgn.adapters.karn import KarnAdapter, Refusal
from mgn.native_actions import native_action


class CastRouteTests(unittest.TestCase):
    def setUp(self):
        self.a = KarnAdapter(Path('/unused'), oracle=object())
        self.a.seats = ['A', 'B']
        self.a._record_id = lambda obj: f'c{obj}'
        self.a._engine_id = lambda obj: int(obj[1:])
        self.a._ability_ref = lambda row, source: {'face': 0, 'paragraph': row['paragraph']}
        self.a.state = {'begun': True, 'asked': {'kind': 'priority', 'player': 0},
                        'players': [{'pool': {}}, {'pool': {}}], 'objects': {}}
        self.a.audit = {'objects': {'3': {'physical': 103, 'abilities': [],
                                          'casting_costs': {'alternatives': []}}}}

    def frame(self, route, convoke=(), delve=(), permission=None):
        announced = {'cast_by': 0, 'costs': {'kicks': 0, 'optional': None, 'alternative': None},
                     'cast': {'route': route, 'permission': permission, 'completed': True,
                              'payment': {'convoke': list(convoke), 'delve': list(delve)}}}
        return {'audit': {'objects': {'4': {'physical': 103, 'announced': announced}}},
                'state': {'players': [{'pool': {}}, {'pool': {}}],
                          'events': [{'event': 'cast', 'spell': 4, 'player': 'p0'}]}}

    def inverse(self, move, following):
        return native_action(self.a, move, following)

    def refuses_atomically(self, move, following, text):
        frozen = deepcopy((move, following, self.a.state, self.a.audit))
        with self.assertRaisesRegex(Refusal, text):
            self.inverse(move, following)
        self.assertEqual((move, following, self.a.state, self.a.audit), frozen)

    def test_default_flashback_is_taken_from_accepted_announcement(self):
        # CR 601.2b/702.34a: a missing input wrapper does not make flashback normal.
        following = self.frame({'kind': 'flashback'})
        do = self.inverse({'kind': 'cast', 'card': 3}, following)
        self.assertEqual(do, {'kind': 'cast', 'card': 'c3', 'cost': {'kind': 'alternative', 'keyword': 'flashback'}})
        self.assertEqual(self.a._move(do)['route'], 'flashback')
        self.assertEqual(self.a._move({'kind': 'cast', 'card': 'c3'})['route'], 'normal')
        self.assertEqual(self.a._cast_from_announced(following['audit']['objects']['4'], 'c3'), {'cost': do['cost']})

    def test_convoke_pairs_each_actual_creature_with_its_colored_or_generic_payment(self):
        # CR 702.51a/b: same-color creatures may be assigned to different requirements.
        following = self.frame({'kind': 'normal'}, [{'object': 1, 'color': 'B'}, {'object': 2, 'color': None}])
        move = {'kind': 'cast', 'card': 3, 'route': 'normal', 'convoke': ['B', 'generic'], 'paying': [[5], [1], [2]]}
        do = self.inverse(move, following)
        self.assertEqual(do, {'kind': 'cast', 'card': 'c3', 'convoke': [{'object': 'c1', 'mana': 'B'}, {'object': 'c2', 'mana': None}],
                              'pay': {'objects': [['c5']]}})
        self.assertEqual(self.a._move(do), {'kind': 'cast', 'card': 3, 'targets': [], 'route': 'normal',
                                           'convoke': ['B', 'generic'], 'paying': [[5], [1], [2]]})
        self.assertEqual(self.a._cast_from_announced(following['audit']['objects']['4'], 'c3'), {'convoke': do['convoke']})

    def test_delve_selection_is_separate_from_the_additional_discard_group(self):
        # CR 702.66a/b: the discarded card cannot be inferred to have paid delve.
        following = self.frame({'kind': 'normal'}, delve=[2, 1])
        move = {'kind': 'cast', 'card': 3, 'delve': 2, 'paying': [[5], [2, 1]]}
        do = self.inverse(move, following)
        self.assertEqual(do, {'kind': 'cast', 'card': 'c3', 'delve': ['c2', 'c1'], 'pay': {'objects': [['c5']]}})
        forward = self.a._move(do)
        self.assertEqual(forward['paying'], [[5], [2, 1]])
        self.assertEqual(forward['delve'], 2)

    def test_harmonize_optional_creature_is_announced_without_a_choosing_group(self):
        # CR 702.180b: choose this creature at announcement; tap it at payment.
        for tapped in (None, 1):
            with self.subTest(tapped=tapped):
                route = {'kind': 'harmonize', 'tapped': tapped}
                following = self.frame(route)
                native = 'harmonize' if tapped is None else {'harmonize': tapped}
                do = self.inverse({'kind': 'cast', 'card': 3, 'route': native}, following)
                expected = {'kind': 'alternative', 'keyword': 'harmonize'}
                if tapped is not None:
                    expected['tapped'] = 'c1'
                self.assertEqual(do['cost'], expected)
                forward = self.a._move(do)
                self.assertEqual(forward['route'], native)
                self.assertNotIn('paying', forward)

    def test_static_permission_uses_frozen_owner_and_keeps_mana_cost(self):
        # CR 601.3: permission to cast from a zone does not replace the mana cost.
        owner = {'source': 10, 'origin': {'kind': 'own'}, 'paragraph': 2,
                 'declaration_index': 1, 'definition_identity': {'card': 7, 'face': 'Whole'},
                 'text': 'Once during each of your turns, you may cast a permanent spell with mana value 2 or less from your graveyard.'}
        self.a.audit['objects']['10'] = {'abilities': [{**owner, 'index': 6}]}
        following = self.frame({'kind': 'permission', 'source': 10, 'ability': 6}, permission=owner)
        move = {'kind': 'cast', 'card': 3, 'route': {'source': 10, 'ability': 6}}
        do = self.inverse(move, following)
        self.assertEqual(do, {'kind': 'cast', 'card': 'c3', 'permission': {'source': 'c10', 'ability': {'face': 0, 'paragraph': 2}}})
        self.assertEqual(self.a._move(do)['route'], move['route'])
        self.assertNotIn('cost', do)

    def test_retrace_is_a_permission_with_additional_discard_not_alternative_cost(self):
        # CR 702.81a: retain the card's exact Retrace ability and ordinary discard group.
        row = {'source': 4, 'origin': {'kind': 'own'}, 'index': 7, 'paragraph': 2, 'keyword': 'Retrace', 'text': 'Retrace'}
        self.a.audit['objects']['3']['abilities'] = [{**row, 'source': 3}]
        following = self.frame({'kind': 'retrace'})
        following['audit']['objects']['4']['abilities'] = [row]
        do = self.inverse({'kind': 'cast', 'card': 3, 'route': 'retrace', 'paying': [[5]]}, following)
        self.assertEqual(do, {'kind': 'cast', 'card': 'c3', 'permission': {'source': 'c3', 'ability': {'face': 0, 'paragraph': 2}},
                              'pay': {'objects': [['c5']]}})
        self.assertEqual(self.a._move(do)['route'], 'retrace')
        with self.assertRaisesRegex(Refusal, 'not an alternative cost'):
            self.a._move({'kind': 'cast', 'card': 'c3', 'cost': {'kind': 'alternative', 'keyword': 'retrace'}})

    def test_changed_substitution_evidence_refuses_without_mutating_frames(self):
        move = {'kind': 'cast', 'card': 3, 'convoke': ['B'], 'paying': [[1]]}
        for label, mutate in [
            ('allocation', lambda f: f['audit']['objects']['4']['announced']['cast']['payment']['convoke'][0].update(color=None)),
            ('chosen objects', lambda f: f['audit']['objects']['4']['announced']['cast']['payment']['convoke'][0].update(object=2)),
            ('incomplete', lambda f: f['audit']['objects']['4']['announced']['cast'].update(completed=False)),
            ('copied', lambda f: f['audit']['objects']['4'].update(stack_copy_of=3)),
            ('another caster', lambda f: f['audit']['objects']['4']['announced'].update(cast_by=1)),
            ('retained cast announcement', lambda f: f['audit']['objects']['4'].update(physical=999)),
        ]:
            with self.subTest(label=label):
                following = self.frame({'kind': 'normal'}, [{'object': 1, 'color': 'B'}])
                mutate(following)
                self.refuses_atomically(move, following, label)

    def test_missing_or_conflicting_route_and_permission_are_not_inferred(self):
        self.refuses_atomically({'kind': 'cast', 'card': 3, 'route': 'flashback'}, self.frame({'kind': 'normal'}), 'route disagrees')
        self.refuses_atomically({'kind': 'cast', 'card': 3}, self.frame({'kind': 'permission', 'source': 10, 'ability': 2},
                               permission={'unavailable': 'not retained'}), 'exact frozen owner')
        self.refuses_atomically({'kind': 'cast', 'card': 3, 'delve': 1, 'paying': [[1]]},
                               {'state': {'events': []}}, 'chosen objects')

    def test_unsupported_quote_vocabulary_remains_unsupported_at_the_adapter_boundary(self):
        # Missing snow provenance cannot prove an illegal payment; even an
        # earlier unpaid R must not hide that unsupported later requirement.
        class QuoteEngine:
            def send(self, command):
                return {'ok': True, 'state': {'prior_pools': [{}, {}], 'events': [],
                    'payments': [{'requirement': {'symbols': ['R', 'S']}, 'pool': {}, 'left': {}}]}}
        self.a.engine = QuoteEngine()
        self.a.session_id = 'fixture'
        self.a.state['request_id'] = 1
        with self.assertRaises(Refusal) as cm:
            self.a._explicit_payment(0, {'kind': 'cast', 'card': 3}, {'kind': 'cast', 'pay': {}})
        self.assertEqual((cm.exception.class_, cm.exception.code), ('unsupported', 'unsupported_kind'))

    def prepared_frame(self):
        # CR 722: a prepared card copy is cast; its exile incarnation becomes
        # the stack incarnation. CR 707.10's uncast stack copies are distinct.
        self.a.audit['objects']['3'].update(physical=None, prepare_source=10, zone='Exile', stack_copy_of=None)
        following = self.frame({'kind': 'normal'})
        following['audit']['objects']['4'].update(physical=None, prepare_source=10, zone='Stack', stack_copy_of=None)
        following['state']['events'].insert(0, {'event': 'zone', 'from': 'Exile', 'was': 3, 'to': 'Stack', 'now': 4})
        return following

    def test_prepared_card_copy_is_bound_by_exact_exile_to_stack_incarnation_and_donor(self):
        following = self.prepared_frame()
        move = {'kind': 'cast', 'card': 3}
        frozen = deepcopy((move, following, self.a.state, self.a.audit))
        self.assertEqual(self.inverse(move, following), {'kind': 'cast', 'card': 'c3'})
        self.assertEqual((move, following, self.a.state, self.a.audit), frozen)

    def test_prepared_copy_wrong_source_or_donor_refuses_atomically(self):
        for label, mutate in [
            ('wrong was', lambda f: f['state']['events'][0].update(was=99)),
            ('wrong endpoint', lambda f: f['state']['events'][0].update(now=99)),
            ('wrong donor', lambda f: f['audit']['objects']['4'].update(prepare_source=99)),
            ('uncast stack copy', lambda f: f['audit']['objects']['4'].update(stack_copy_of=3)),
            ('incomplete', lambda f: f['audit']['objects']['4']['announced']['cast'].update(completed=False)),
        ]:
            with self.subTest(label=label):
                following = self.prepared_frame()
                mutate(following)
                self.refuses_atomically({'kind': 'cast', 'card': 3}, following,
                                       'incomplete' if label == 'incomplete' else 'exact input-to-cast transition')

    def test_forward_duplicate_substitution_resources_refuse_atomically(self):
        for do in [
            {'kind': 'cast', 'card': 'c3', 'convoke': [{'object': 'c1', 'mana': 'B'}, {'object': 'c1', 'mana': None}]},
            {'kind': 'cast', 'card': 'c3', 'delve': ['c1', 'c1']},
            {'kind': 'cast', 'card': 'c3', 'convoke': [{'object': 'c1', 'mana': 'B'}], 'pay': {'objects': [['c1']]}},
        ]:
            before = deepcopy((do, self.a.state, self.a.audit))
            with self.assertRaisesRegex(Refusal, 'cannot pay twice'):
                self.a._move(do)
            self.assertEqual((do, self.a.state, self.a.audit), before)

    def test_spell_copy_projects_kicker_choices_without_permission_or_payment(self):
        # CR 707.10: copied choices survive, while this copy was never cast or paid.
        following = self.frame({'kind': 'permission', 'source': 10, 'ability': 2})
        item = following['audit']['objects']['4']
        item.update(physical=None, token=False, stack_copy_of=3, properties={'controller': 0})
        item['announced']['costs']['kicks'] = 2
        item['announced']['cast'].update(completed=False, payment=None)
        self.a.audit['objects']['4'] = item
        self.a.state['objects']['4'] = {}
        facts = self.a._object_facts(4, 'stack', [])
        self.assertEqual(facts, {'kind': 'copy', 'copy_of': 'c3', 'controller': 'A', 'cast': {'additional': {'kicker': 2}}})
        self.assertEqual(self.a.unsupported_paths, [])

# Real processes validate forward quotes/payment and inverse projection together.
# No source expectations come from these observations: each selected cost and
# resource is fixed below from the cited Oracle text/rules before execution.
import os
from test_mgn_adapter import ORACLE, MAIN_A, PRIORITY_A, position_record, setup_of, no_random
ENGINE = Path(os.environ.get('KARN_CONFORMANCE_ENGINE', Path(__file__).resolve().parents[2] / 'target/fast/game-serve'))


@unittest.skipUnless(ENGINE.is_file(), 'build the native engine')
class NativeCastRouteTests(unittest.TestCase):
    def setUp(self):
        self.a = KarnAdapter(ENGINE, ORACLE)

    def tearDown(self):
        self.a.end()

    def exercise(self, cards, zones, do, *, pool=None, expected_cast, prelude=()):
        record = position_record(cards, zones, MAIN_A, PRIORITY_A)
        if pool:
            record['start']['state']['players']['A']['mana'] = pool
        self.a.setup(setup_of(record))
        for n, (seat, action) in enumerate(prelude, 1):
            if action['kind'] == 'pass':
                self.a._send_move(self.a._player(seat), action, no_random)  # MGN's null priority answer
            else:
                self.a.act(n, seat, action, no_random)
        move = self.a._move(do)
        before_state, before_audit = deepcopy(self.a.state), deepcopy(self.a.audit)
        quote = self.a.engine.must({'cmd': 'quote_payment', 'session_id': self.a.session_id,
                                   'request_id': self.a.state['request_id'], 'player': 0, 'move': move})['state']
        self.assertTrue(quote['complete'])
        reply = self.a.act(len(prelude) + 1, 'A', do, no_random)
        self.assertEqual(reply['state']['objects'][do['card']]['cast'], expected_cast)
        following = {'state': deepcopy(self.a.state), 'audit': deepcopy(self.a.audit), 'payment_quote': quote}
        after_state, after_audit = self.a.state, self.a.audit
        try:
            self.a.state, self.a.audit = before_state, before_audit
            inverse = native_action(self.a, move, following)
        finally:
            self.a.state, self.a.audit = after_state, after_audit
        for key, value in expected_cast.items():
            self.assertEqual(inverse[key], value)
        self.assertEqual(inverse.get('pay', {}), do.get('pay', {}))
        return reply

    def test_real_flashback_quotes_and_projects_the_paid_route(self):
        # Think Twice: Flashback {2}{U}; CR 702.34a/601.2b.
        cost = {'kind': 'alternative', 'keyword': 'flashback'}
        self.exercise({'spell': ('Think Twice', 'A')}, {'A.graveyard': ['spell']},
                      {'kind': 'cast', 'card': 'spell', 'cost': cost, 'pay': {'pool': {'C': 2, 'U': 1}}},
                      pool={'C': 2, 'U': 1}, expected_cast={'cost': cost})

    def test_real_convoke_selects_colored_and_generic_creatures_without_object_cost_echo(self):
        # Stoke the Flames {2}{R}{R}; CR 702.51a/b, no mana production.
        cards = {'spell': ('Stoke the Flames', 'A'), **{f'g{i}': ('Goblin Piker', 'A') for i in range(4)}}
        convoke = [{'object': f'g{i}', 'mana': 'R' if i < 2 else None} for i in range(4)]
        reply = self.exercise(cards, {'A.hand': ['spell'], 'battlefield': [f'g{i}' for i in range(4)]},
                              {'kind': 'cast', 'card': 'spell', 'targets': [['B']], 'convoke': convoke},
                              expected_cast={'convoke': convoke})
        for i in range(4):
            self.assertTrue(reply['state']['objects'][f'g{i}']['tapped'])

    def test_real_delve_keeps_the_selected_preexile_physical_cards(self):
        # Treasure Cruise {7}{U}; CR 702.66a/b. The U remains a mana payment.
        cards = {'spell': ('Treasure Cruise', 'A'), **{f'f{i}': ('Forest', 'A') for i in range(7)}}
        chosen = [f'f{i}' for i in range(7)]
        reply = self.exercise(cards, {'A.hand': ['spell'], 'A.graveyard': chosen},
                              {'kind': 'cast', 'card': 'spell', 'delve': chosen, 'pay': {'pool': {'U': 1}}},
                              pool={'U': 1}, expected_cast={'delve': chosen})
        self.assertEqual(set(reply['state']['zones']['exile']), set(chosen))

    def test_real_harmonize_announced_creature_reduces_generic_cost(self):
        # Winternight Stories harmonize {4}{U}; Grizzly Bears power 2 => {2}{U}.
        # CR 702.180a/b: the tap is announced and fixed, not pay.objects.
        cost = {'kind': 'alternative', 'keyword': 'harmonize', 'tapped': 'bear'}
        reply = self.exercise({'spell': ('Winternight Stories', 'A'), 'bear': ('Grizzly Bears', 'A')},
                              {'A.graveyard': ['spell'], 'battlefield': ['bear']},
                              {'kind': 'cast', 'card': 'spell', 'cost': cost, 'pay': {'pool': {'C': 2, 'U': 1}}},
                              pool={'C': 2, 'U': 1}, expected_cast={'cost': cost})
        self.assertTrue(reply['state']['objects']['bear']['tapped'])

    def test_real_retrace_is_permission_plus_discard_and_mana(self):
        # Raven's Crime: retrace, additional land discard, ordinary {B}; 702.81a.
        permission = {'source': 'spell', 'ability': {'face': 0, 'paragraph': 2}}
        self.exercise({'spell': ("Raven's Crime", 'A'), 'land': ('Forest', 'A')},
                      {'A.graveyard': ['spell'], 'A.hand': ['land']},
                      {'kind': 'cast', 'card': 'spell', 'targets': [['B']], 'permission': permission,
                       'pay': {'objects': [['land']], 'pool': {'B': 1}}},
                      pool={'B': 1}, expected_cast={'permission': permission})

    def test_real_static_zone_permission_keeps_the_ordinary_mana_cost(self):
        # Lurrus: one permanent spell with mana value 2 or less from your graveyard
        # during each of your turns. This is permission, not an alternative cost.
        permission = {'source': 'lurrus', 'ability': {'face': 0, 'paragraph': 3}}
        self.exercise({'spell': ('Grizzly Bears', 'A'), 'lurrus': ('Lurrus of the Dream-Den', 'A')},
                      {'A.graveyard': ['spell'], 'A.hand': ['lurrus']},
                      {'kind': 'cast', 'card': 'spell', 'permission': permission, 'pay': {'pool': {'C': 1, 'G': 1}}},
                      pool={'C': 2, 'B': 2, 'G': 1}, expected_cast={'permission': permission},
                      # Play Lurrus first: constructing its limited-use permission on
                      # a starting permanent needs history the position API refuses.
                      prelude=[('A', {'kind': 'cast', 'card': 'lurrus', 'pay': {'pool': {'C': 1, 'B': 2}}}),
                               ('A', {'kind': 'pass'}), ('B', {'kind': 'pass'})])

    def test_real_wrong_convoke_color_refuses_without_publishing_an_announcement(self):
        record = position_record({'spell': ('Stoke the Flames', 'A'), 'goblin': ('Goblin Piker', 'A')},
                                 {'A.hand': ['spell'], 'battlefield': ['goblin']}, MAIN_A, PRIORITY_A)
        record['start']['state']['players']['A']['mana'] = {'C': 2, 'R': 1}
        self.a.setup(setup_of(record))
        before = self.a._reply_state()
        with self.assertRaises(Refusal):
            self.a.act(1, 'A', {'kind': 'cast', 'card': 'spell', 'targets': [['B']],
                'convoke': [{'object': 'goblin', 'mana': 'G'}], 'pay': {'pool': {'C': 2, 'R': 1}}}, no_random)
        self.assertEqual(self.a._reply_state(), before)

    def test_real_omitted_alternative_cost_never_selects_native_default_flashback(self):
        record = position_record({'spell': ('Think Twice', 'A')}, {'A.graveyard': ['spell']}, MAIN_A, PRIORITY_A)
        record['start']['state']['players']['A']['mana'] = {'C': 2, 'U': 1}
        self.a.setup(setup_of(record))
        before = self.a._reply_state()
        with self.assertRaises(Refusal):
            self.a.act(1, 'A', {'kind': 'cast', 'card': 'spell', 'pay': {'pool': {'C': 2, 'U': 1}}}, no_random)
        self.assertEqual(self.a._reply_state(), before)

    def test_authored_convoke_illegal_then_valid_attempt_keeps_live_state_unchanged(self):
        # external3/convoke-color-and-generic: Aerial Boost {1}{W}. Two G/W
        # creatures are valid convokers, but G+generic leaves W unpaid (702.51a).
        record = position_record({'boost': ('Aerial Boost', 'A'),
                                  'guild1': ('Selesnya Guildmage', 'A'), 'guild2': ('Selesnya Guildmage', 'A')},
                                 {'A.hand': ['boost'], 'battlefield': ['guild1', 'guild2']}, MAIN_A, PRIORITY_A)
        self.a.setup(setup_of(record))
        attempt = {'kind': 'cast', 'card': 'boost', 'targets': [['guild1']],
                   'convoke': [{'object': 'guild1', 'mana': 'G'}, {'object': 'guild2', 'mana': None}], 'pay': {}}
        before = self.a._reply_state()
        with self.assertRaises(Refusal) as cm:
            self.a.attempt(1, 'A', attempt, no_random)
        self.assertEqual((cm.exception.class_, cm.exception.code), ('illegal', 'illegal_cost'))
        self.assertEqual(self.a._reply_state(), before)
        attempt['convoke'][0]['mana'] = 'W'
        reply = self.a.act(2, 'A', attempt, no_random)
        self.assertTrue(reply['state']['objects']['guild1']['tapped'])
        self.assertTrue(reply['state']['objects']['guild2']['tapped'])
        self.assertEqual(reply['state']['zones']['stack'], ['boost'])


if __name__ == '__main__':
    unittest.main()
