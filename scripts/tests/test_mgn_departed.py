"""CR 113.7a/603.6c/704.5d: explicit token definition history has no live rights."""
from copy import deepcopy
from pathlib import Path
import unittest

from mgn.canonical import canonical_bytes, loads
from mgn.delta import diff_states
from mgn.identity import record_identity
from mgn.render import parse, render
from mgn.runner import compare
from mgn.schema import Validator, load_schema, validate as schema_validate
from mgn.state import State
from mgn.validate import _check_references, _check_abilities
import test_mgn_history


class DepartedDefinitionTests(unittest.TestCase):
    def start(self):
        record = test_mgn_history.HistoricalSourceTests().position()
        record['start']['state']['zones']['battlefield'] = []
        record['start']['state']['objects'] = {}
        return record

    def delta(self, token='T1'):
        definition = test_mgn_history.HistoricalSourceTests().position()['start']['state']['objects']['T1']['token']
        return {'departed': {token: {'owner': 'A', 'token': definition}},
                'objects': {'S1': {'zone': 'stack', 'kind': 'ability', 'source': token,
                                  'ability': {'face': 0, 'paragraph': 1}, 'controller': 'A'}}}

    def state(self):
        record = self.start()
        state = State.from_record(record, [])
        problems = []
        state.apply(self.delta(), [], '/delta', problems)
        self.assertEqual(problems, [])
        return record, state

    def test_definition_source_is_known_but_never_live_or_actionable(self):
        _, state = self.state()
        self.assertTrue(state.known('T1'))
        self.assertFalse(state.exists('T1'))
        self.assertIsNone(state.zone_of('T1'))
        self.assertEqual(state.reference_facts('T1')['token']['name'], 'Clue')
        for action in ({'kind': 'activate', 'source': 'T1', 'ability': {'face': 0, 'paragraph': 1}},
                       {'kind': 'choose_objects', 'objects': ['T1']},
                       {'kind': 'choose_targets', 'targets': [['T1']]}):
            problems = []
            _check_references(action, state, '/do', problems)
            self.assertTrue(any(p.code == 'unknown_reference' for p in problems))
        problems = []
        state.apply({'objects': {'T1': {'zone': 'battlefield', 'controller': 'A'}}}, [], '/delta', problems)
        self.assertTrue(any('cannot become live' in p.message for p in problems))

    def test_full_observation_delta_render_and_identity_roundtrip(self):
        record, state = self.state()
        before = State.from_record(record, [])
        delta = diff_states(before, state, None)
        self.assertEqual(delta['departed'], self.delta()['departed'])
        restored = deepcopy(before)
        problems = []
        restored.apply(delta, [], '/delta', problems)
        self.assertEqual(problems, [])
        self.assertEqual(restored.explicit(), state.explicit())
        full = {**record, 'mgn': '1', 'transitions': [], 'start': {'kind': 'position', 'state': state.explicit()}}
        problems = []
        observed = State.from_observation(full, problems, before)
        self.assertEqual(problems, [])
        self.assertEqual(observed.explicit(), state.explicit())
        self.assertEqual(canonical_bytes(parse(render(full))), canonical_bytes(full))
        self.assertEqual(record_identity(parse(render(full))), record_identity(full))
        self.assertNotIn('departed', before.explicit())
        self.assertNotIn('departed', diff_states(before, before, None))

    def test_new_live_and_departed_tokens_share_one_number_sequence(self):
        state = State.from_record(self.start(), [])
        delta = self.delta('T2')
        delta['objects']['T1'] = {'zone': 'battlefield', 'controller': 'A',
                                  'token': {'name': 'Bear Token', 'types': ['creature'], 'colors': ['green'], 'power': 2, 'toughness': 2}}
        problems = []
        state.apply(delta, [], '/delta', problems)
        self.assertEqual(problems, [])
        self.assertEqual(state.tokens_created, 2)
        self.assertTrue(state.exists('T1'))
        self.assertFalse(state.exists('T2'))

    def test_duplicate_unused_unknown_and_redefined_introductions_refuse(self):
        record, state = self.state()
        cases = [self.delta(), {'departed': {'T2': deepcopy(state.departed['T1'])}},
                 {'departed': {'T1': {**state.departed['T1'], 'owner': 'B'}}}]
        for delta in cases:
            problems = []
            deepcopy(state).apply(delta, [], '/delta', problems)
            self.assertTrue(any(p.severity == 'error' for p in problems))
        problems = []
        fresh = State.from_record(record, [])
        fresh.apply({'objects': self.delta()['objects']}, [], '/delta', problems)
        self.assertTrue(any(p.code == 'unknown_reference' for p in problems))
        schema = load_schema('mgn-1.schema.json')
        for row in ({'owner': 'A'}, {**state.departed['T1'], 'zone': 'battlefield'},
                    {**state.departed['T1'], 'controller': 'A'}):
            self.assertTrue(Validator(schema).errors(row, schema['$defs']['departed_definition']))
        with self.assertRaises(ValueError):
            loads('{"departed":{"T1":{},"T1":{}}}')

    def test_declared_ability_and_full_observation_cannot_invent_a_living_token(self):
        record, state = self.state()
        wrong = self.delta()
        wrong['objects']['S1']['ability']['paragraph'] = 2
        problems = []
        _check_abilities(wrong, state, object(), '/delta', problems)
        self.assertTrue(any(p.code == 'ability' for p in problems))
        after = deepcopy(record)
        after['start']['state'] = state.explicit()
        after['start']['state']['zones']['battlefield'] = ['T1']
        after['start']['state']['objects']['T1'] = {**state.departed['T1'], 'controller': 'A'}
        problems = []
        State.from_observation(after, problems, state)
        self.assertTrue(any('cannot become live' in p.message for p in problems))

    def test_full_observation_rejects_a_skipped_departed_number(self):
        record, state = self.state()
        before = State.from_record(record, [])
        observed = state.explicit()
        observed['departed']['T8'] = observed['departed'].pop('T1')
        observed['objects']['S1']['source'] = 'T8'
        record['start']['state'] = observed
        problems = []
        State.from_observation(record, problems, before)
        self.assertTrue(any('consecutive' in p.message for p in problems))

    def test_runner_compares_departed_definition_as_l3(self):
        _, state = self.state()
        expected = state.explicit()
        for change in ('missing', 'definition', 'owner'):
            actual = deepcopy(expected)
            if change == 'missing': del actual['departed']
            elif change == 'owner': actual['departed']['T1']['owner'] = 'B'
            else: actual['departed']['T1']['token']['text'] = 'Draw two cards.'
            diffs, unsupported = compare(expected, actual, [], 2)
            self.assertEqual(unsupported, [])
            self.assertEqual([d['level'] for d in diffs], ['L3'])
            self.assertEqual(diffs[0]['path'], 'departed.T1')


class DepartedNativeEvidenceTests(unittest.TestCase):
    def adapter(self):
        from mgn.adapters.karn import KarnAdapter
        adapter = KarnAdapter(Path("unused"))
        adapter.seats = ["A", "B"]
        adapter.audit = {"objects": {"10": {"physical": None, "token": True, "zone": "Gone", "owner": 0,
            "base": {"name": "Thragtusk", "types": ["Creature"], "subtypes": ["Beast", "Spirit"],
                     "colors": ["green"], "power": 5, "toughness": 3,
                     "rows": ["When ~ leaves the battlefield, create a 3/3 green Beast creature token."]},
            "definition_rows": ["When ~ leaves the battlefield, create a 3/3 green Beast creature token."],
            "abilities": []}}}
        return adapter

    def test_retained_intrinsic_definition_is_exact_and_never_a_live_target(self):
        adapter = self.adapter()
        self.assertEqual(adapter._record_id(10), "T1")
        self.assertEqual(adapter.departed_tokens[10]["owner"], "A")
        self.assertEqual(adapter.departed_tokens[10]["token"]["subtypes"], ["Beast", "Spirit"])
        self.assertEqual(adapter.committed_departed, {})
        self.assertEqual(adapter.current, {})
        adapter.committed_tokens = {10: "T4"}
        adapter.tokens = {}
        adapter.departed_tokens = {}
        self.assertEqual(adapter._record_id(10), "T4")
        self.assertEqual(adapter.departed_tokens, {})  # recorded history already supplies it

    def test_unknown_mismatched_and_acquired_copy_sources_refuse(self):
        from mgn.adapters.karn import Refusal
        adapter = self.adapter()
        with self.assertRaisesRegex(Refusal, "has no id"):
            adapter._record_id(99)
        for patch, message in (({"definition_rows": ["Draw a card."]}, "authenticated declaring rows"),
                               ({"copy_acquisition": {"donor": 999}}, "acquired copy provenance"),
                               ({"abilities": [{"origin": {"kind": "copied"}}]}, "acquired copy provenance"),
                               ({"base": None}, "authenticated declaring rows")):
            adapter = self.adapter()
            adapter.audit["objects"]["10"].update(patch)
            with self.assertRaisesRegex(Refusal, message):
                adapter._record_id(10)
            self.assertEqual(adapter.departed_tokens, {})

    def test_protocol_uses_the_full_record_state_schema(self):
        _, state = DepartedDefinitionTests().state()
        self.assertEqual(schema_validate({"id": 1, "ok": {"state": state.explicit()}}, "adapter-protocol.schema.json"), [])
        wrong = state.explicit()
        wrong["departed"]["T1"]["controller"] = "A"
        self.assertTrue(schema_validate({"id": 1, "ok": {"state": wrong}}, "adapter-protocol.schema.json"))

    def test_setup_cannot_drop_a_departed_premise(self):
        from mgn.adapters.karn import Refusal
        _, state = DepartedDefinitionTests().state()
        adapter = self.adapter()
        with self.assertRaisesRegex(Refusal, "departed token premises cannot be constructed"):
            adapter._native_position({"state": state.explicit()}, {"seats": ["A", "B"]})

    def test_live_and_departed_newcomers_share_canonical_definition_order(self):
        adapter = self.adapter()
        adapter.state = {"objects": {"20": {"is_card": False}}}
        live = deepcopy(adapter.audit["objects"]["10"])
        live.update(zone="Battlefield", base={"name": "Aardvark Token", "types": ["Creature"],
                                               "colors": ["green"], "power": 1, "toughness": 1})
        adapter.audit["objects"]["20"] = live
        adapter.state.update(stack=[], command=[])
        self.assertEqual(adapter._record_id(10), "T1")  # provisional discovery, before normalization
        adapter._refresh_ids(keep_departed=True)
        self.assertEqual(adapter.tokens, {20: "T1", 10: "T2"})
        self.assertEqual(adapter.current, {"T1": 20})
        adapter._refresh_ids()  # no decision committed this projection
        self.assertEqual(adapter.tokens, {20: "T1"})
        self.assertEqual(adapter.departed_tokens, {})


class DepartedDecisionWitnessTests(unittest.TestCase):
    def test_authored_shock_response_commits_only_the_departed_definition(self):
        from test_mgn_adapter import ENGINE, ORACLE
        from mgn.runner import Run, karn_adapter_command
        if not ENGINE.is_file(): self.skipTest("requires the preserved conformance engine")
        path = Path(__file__).resolve().parents[2] / 'tests/fixtures/mgn-drafts/departed-token-shock.json'
        record = loads(path.read_text())
        report = Run(record, ORACLE, karn_adapter_command(ENGINE), 30).run()
        self.assertEqual(report['divergences'], [])
        self.assertEqual(report['refusals'], [])
        self.assertEqual(report['inconclusive'], [])
        self.assertEqual(report['replayed'], 4)
        self.assertEqual({k: report['levels'][k] for k in ('L0', 'L1', 'L2', 'L3')},
                         {k: 'pass' for k in ('L0', 'L1', 'L2', 'L3')})
