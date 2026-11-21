"""CR 113.7a/704.5d: a sacrificed token's ability outlives its source."""
from copy import deepcopy
import unittest

from mgn.state import State
from mgn.delta import diff_states
from mgn.validate import _check_abilities, _check_references


class HistoricalSourceTests(unittest.TestCase):
    def position(self):
        return {'tier': 2, 'game': {'seats': ['A', 'B']}, 'cards': {},
                'start': {'kind': 'position', 'state': {
                    'turn': {'number': 3, 'step': 'precombat_main', 'active': 'A'},
                    'pending': {'seat': 'A', 'kind': 'priority'}, 'result': {'kind': 'unfinished'},
                    'players': {'A': {'life': 20}, 'B': {'life': 20}},
                    'zones': {'A.library': [], 'A.hand': [], 'A.graveyard': [], 'B.library': [],
                              'B.hand': [], 'B.graveyard': [], 'battlefield': ['T1'], 'stack': [],
                              'exile': [], 'command': []},
                    'objects': {'T1': {'controller': 'A', 'owner': 'A', 'token': {
                        'name': 'Clue', 'types': ['artifact'], 'subtypes': ['Clue'], 'colors': [],
                        'text': '{2}, Sacrifice this token: Draw a card.'}}}}}}

    def test_departed_definition_supports_source_but_not_new_object_selection(self):
        # 602.2b, 601.2h: paying the Clue's cost sacrifices it. 704.5d makes
        # the token cease; its activated ability still exists (113.7a).
        problems = []
        state = State.from_record(self.position(), problems)
        delta = {'objects': {'T1': None, 'S1': {'zone': 'stack', 'kind': 'ability',
                  'source': 'T1', 'ability': {'face': 0, 'paragraph': 1}, 'controller': 'A'}}}
        state.apply(delta, [], '/delta', problems)
        _check_abilities(delta, state, object(), '/delta', problems)
        self.assertEqual(problems, [])
        self.assertFalse(state.exists('T1'))
        self.assertNotIn('T1', state.explicit()['objects'])
        _check_references({'kind': 'choose_objects', 'objects': ['T1']}, state, '/do', problems)
        self.assertTrue(any(p.code == 'unknown_reference' for p in problems))
        problems = []
        wrong = deepcopy(delta)
        wrong['objects']['S1']['ability']['paragraph'] = 2
        _check_abilities(wrong, state, object(), '/delta', problems)
        self.assertTrue(any(p.code == 'ability' for p in problems))
        problems = []
        _check_references({'kind': 'order_triggers', 'order': [
            {'source': 'T1', 'ability': {'face': 0, 'paragraph': 1}}]}, state, '/do', problems)
        self.assertEqual(problems, [])

    def test_full_observation_requires_actual_recorded_history(self):
        record = self.position()
        prior = State.from_record(record, [])
        later = deepcopy(record)
        observed = later['start']['state']
        observed['zones']['battlefield'] = []
        observed['zones']['stack'] = ['S1']
        observed['objects'] = {'S1': {'kind': 'ability', 'source': 'T1',
                                      'ability': {'face': 0, 'paragraph': 1}, 'controller': 'A'}}
        errors = []
        State.from_record(later, errors)
        self.assertTrue(any(p.code == 'unknown_reference' for p in errors))
        errors = []
        result = State.from_observation(later, errors, prior)
        self.assertEqual(errors, [])
        self.assertEqual(result.tokens_created, 1)
        self.assertTrue(result.known('T1'))
        observed['objects']['S1']['source'] = 'T2'
        errors = []
        State.from_observation(later, errors, prior)
        self.assertTrue(any(p.code == 'unknown_reference' for p in errors))

    def test_live_identity_never_recovers_stale_copy_facts_from_history(self):
        # A physical card is a new object after changing zone (400.7), even
        # though MGN keeps its card ID. Old copy characteristics do not return.
        record = self.position()
        record['cards'] = {'clone': {'owner': 'A'}}
        record['start']['state']['zones']['A.graveyard'] = ['clone']
        state = State.from_record(record, [])
        state.historical['clone'] = {'copy_of': 'T1'}
        self.assertEqual(state.reference_facts('clone'), {})

    def test_animation_ending_removes_unprinted_power_without_inventing_a_default(self):
        # 208.3: an artifact that is no longer a creature has no P/T. The
        # printed token definition never acquired the temporary animation.
        record = self.position()
        initial = record['start']['state']['objects']['T1']
        initial.update(types=['artifact', 'creature'], power=2, toughness=2)
        prior = State.from_record(record, [])
        after = deepcopy(record)
        for key in ('types', 'power', 'toughness'):
            del after['start']['state']['objects']['T1'][key]
        target = State.from_observation(after, [], prior)
        delta = diff_states(prior, target, None)
        self.assertIsNone(delta['objects']['T1']['power'])
        self.assertIsNone(delta['objects']['T1']['toughness'])
        errors = []
        prior.apply(delta, [], '/delta', errors)
        self.assertEqual(errors, [])
        self.assertEqual(prior.explicit(), target.explicit())

    def test_repeated_shuffles_validate_each_outcome_but_project_the_last_order(self):
        # A folded stretch can shuffle, put a graveyard card into the library,
        # then shuffle again (701.24). The first order cannot include the card
        # that was still in the graveyard at that moment.
        record = self.position()
        record['cards'] = {c: {'owner': 'A'} for c in ('a', 'b', 'c')}
        position = record['start']['state']
        position['zones']['battlefield'] = []
        position['objects'] = {}
        position['zones']['A.library'] = ['a', 'b']
        position['zones']['A.graveyard'] = ['c']
        random = [{'kind': 'shuffle', 'seat': 'A', 'order': ['b', 'a']},
                  {'kind': 'shuffle', 'seat': 'A', 'order': ['c', 'a', 'b']}]
        delta = {'objects': {'c': {'zone': 'A.library'}}}
        state, errors = State.from_record(record, []), []
        state.apply(delta, random, '/delta', errors)
        self.assertEqual(errors, [])
        self.assertEqual(state.listing('A.library'), ['c', 'a', 'b'])
        duplicate = deepcopy(random)
        duplicate[0]['order'] = ['a', 'a']
        errors = []
        State.from_record(record, []).apply(delta, duplicate, '/delta', errors)
        self.assertTrue(any(p.code == 'shuffle' for p in errors))
        errors = []
        State.from_record(record, []).apply(delta, random[:1], '/delta', errors)
        self.assertTrue(any(p.code == 'delta' for p in errors))
