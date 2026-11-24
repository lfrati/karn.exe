"""MGN 6 event keys, native consumption receipts and actionable fault diagnostics."""
from copy import deepcopy
from pathlib import Path
from tempfile import TemporaryDirectory
from types import SimpleNamespace
import unittest

from mgn.adapters.karn import Engine, KarnAdapter, Refusal
from mgn.runner import Run


class RandomBoundaryTests(unittest.TestCase):
    def queue(self, outcomes):
        run = Run.__new__(Run)
        run._load_random(deepcopy(outcomes))
        return run

    def test_event_keys_ignore_set_order_and_keep_each_keys_occurrence_order(self):
        first = {'kind': 'random_order', 'objects': ['a', 'b'], 'order': ['b', 'a']}
        second = {'kind': 'random_order', 'objects': ['b', 'a'], 'order': ['a', 'b']}
        other = {'kind': 'random_order', 'objects': ['c', 'd'], 'order': ['d', 'c']}
        run = self.queue([first, other, second])
        self.assertEqual(run._on_random({'kind': 'random_order', 'objects': ['d', 'c']}), other)
        self.assertEqual(run._on_random({'kind': 'random_order', 'objects': ['b', 'a']}), first)
        run._take_back([other, first])
        self.assertEqual(run._on_random({'kind': 'random_order', 'objects': ['a', 'b']}), first)
        self.assertEqual(run._on_random({'kind': 'random_order', 'objects': ['a', 'b']}), second)
        self.assertIsNone(run._on_random({'kind': 'random_order', 'objects': ['a', 'c']}))

    def test_one_command_can_consume_multiple_shuffles_and_a_nonprefix_subset(self):
        # CR 701.24a/720.3d: a search shuffle and the resolving Omen's own
        # shuffle are distinct events, even inside one native response.
        a = {'kind': 'shuffle', 'seat': 'A', 'order': ['a']}
        b1 = {'kind': 'shuffle', 'seat': 'B', 'order': ['b']}
        b2 = {'kind': 'shuffle', 'seat': 'B', 'order': ['b', 'spell']}
        order = {'kind': 'random_order', 'objects': ['x', 'y'], 'order': ['y', 'x']}
        run = self.queue([b1, a, b2, order])
        adapter = KarnAdapter(Path('/unused'), oracle=object())
        adapter.seats = ['A', 'B']
        adapter.session_id = 1
        adapter.state = {'request_id': 1}
        adapter._engine_id = {'a': 10, 'b': 20, 'spell': 21, 'x': 30, 'y': 31}.__getitem__
        adapter._observe = lambda state: None
        sent = []

        def send(request):
            sent.append(request)
            return {'ok': True, 'state': {'random_outcomes_unused_indices': [0, 3]}}

        adapter.engine = SimpleNamespace(send=send)
        adapter._send_move(1, {'kind': 'pass'}, run._on_random)
        self.assertEqual(adapter.used_outcomes, [b1, b2])
        self.assertEqual(adapter.outcomes, [a, order])
        self.assertEqual(sent[0]['random_outcomes'], [
            {'kind': 'shuffle', 'player': 0, 'order': [10]},
            {'kind': 'shuffle', 'player': 1, 'order': [20]},
            {'kind': 'shuffle', 'player': 1, 'order': [20, 21]},
            {'kind': 'random_order', 'order': [31, 30]}])
        run._take_back(adapter.outcomes)
        self.assertEqual(run._on_random({'kind': 'shuffle', 'seat': 'A'}), a)
        self.assertIsNone(run._on_random({'kind': 'shuffle', 'seat': 'B'}))

    def test_a_random_choice_is_sent_as_a_random_order_with_the_chosen_cards_first(self):
        # CR 701.9b: Hymn to Tourach's two cards discarded at random. The engine picks the
        # first cards of one random order of the hand, and checks that set as it picks.
        choice = {'kind': 'random_choice', 'from': ['x', 'y', 'z'], 'chosen': ['z', 'x']}
        run = self.queue([choice])
        adapter = KarnAdapter(Path('/unused'), oracle=object())
        adapter.seats = ['A', 'B']
        adapter.session_id = 1
        adapter.state = {'request_id': 1}
        adapter._engine_id = {'x': 30, 'y': 31, 'z': 32}.__getitem__
        adapter._observe = lambda state: None
        sent = []

        def send(request):
            sent.append(request)
            return {'ok': True, 'state': {'random_outcomes_unused_indices': []}}

        adapter.engine = SimpleNamespace(send=send)
        adapter._send_move(1, {'kind': 'pass'}, run._on_random)
        self.assertEqual(sent[0]['random_outcomes'], [{'kind': 'random_order', 'order': [32, 30, 31]}])
        self.assertEqual(adapter.used_outcomes, [choice])
        for bad in ({'kind': 'random_choice', 'from': ['x', 'y'], 'chosen': ['z']},
                    {'kind': 'random_choice', 'from': ['x', 'x'], 'chosen': ['x']},
                    {'kind': 'random_choice', 'from': ['x', 'y'], 'chosen': ['x', 'x']}):
            adapter.outcomes = [bad]
            with self.assertRaises(Refusal) as caught:
                adapter._engine_random_outcomes()
            self.assertEqual((caught.exception.class_, caught.exception.code), ('fault', 'random_mismatch'))

    def test_bad_replay_input_cannot_satisfy_an_illegal_action_expectation(self):
        adapter = KarnAdapter(Path('/unused'), oracle=object())
        adapter.seats = ['A', 'B']
        adapter.session_id = 1
        adapter.state = {'request_id': 1}
        adapter.engine = SimpleNamespace(send=lambda request: {
            'ok': False, 'code': 'replay_input', 'error': 'replay input: missing random order'})
        with self.assertRaises(Refusal) as caught:
            adapter._send_move(0, {'kind': 'pass'}, lambda request: None)
        self.assertEqual((caught.exception.class_, caught.exception.code), ('fault', 'random_mismatch'))

    def test_only_an_engine_illegal_refusal_is_an_illegal_action(self):
        # an attempt the record marks illegal is not satisfied by a missing capability,
        # an unreadable request or a stale one (adapter-protocol.md, the Karn adapter)
        adapter = KarnAdapter(Path('/unused'), oracle=object())
        adapter.seats = ['A', 'B']
        adapter.session_id = 1
        adapter.state = {'request_id': 1}
        expected = {'illegal': ('illegal', 'illegal_action'), 'unreferable': ('illegal', 'unknown_reference'),
                    'unsupported': ('unsupported', 'unsupported_kind'), 'malformed': ('fault', 'protocol'),
                    'stale': ('fault', 'protocol'), None: ('fault', 'engine_fault')}
        for engine_class, (class_, code) in expected.items():
            reply = {'ok': False, 'error': 'refused', **({'class': engine_class} if engine_class else {})}
            adapter.engine = SimpleNamespace(send=lambda request, reply=reply: reply, diagnostic=lambda message: message)
            with self.assertRaises(Refusal) as caught:
                adapter._send_move(0, {'kind': 'pass'}, lambda request: None)
            self.assertEqual((caught.exception.class_, caught.exception.code), (class_, code), engine_class)
        adapter.engine = SimpleNamespace(send=lambda request: {'ok': False, 'error': 'simulation fault', 'class': 'illegal',
                                                               'fault': {'stage': 'Response'}}, diagnostic=lambda m: m)
        with self.assertRaises(Refusal) as caught:
            adapter._send_move(0, {'kind': 'pass'}, lambda request: None)
        self.assertEqual(caught.exception.code, 'engine_fault')

    def test_a_quoted_moves_refusal_is_an_illegal_action_only_when_the_engine_says_so(self):
        # a payment quote is answered; its proof says the quoted move's refusal and its class
        # (`error_class`). A quoted move the engine does not run does not satisfy an attempt
        # the record marks illegal (adapter-protocol.md, the Karn adapter)
        adapter = KarnAdapter(Path('/unused'), oracle=object())
        adapter.session_id = 1
        adapter.state = {'request_id': 1, 'asked': {'player': 0}}
        expected = {'illegal': ('illegal', 'illegal_action'), 'unreferable': ('illegal', 'unknown_reference'),
                    'unsupported': ('unsupported', 'unsupported_kind'), None: ('fault', 'engine_fault')}
        for engine_class, (class_, code) in expected.items():
            proof = {'complete': False, 'error': 'refused', 'payments': [], 'mana_answers': [], 'events': [],
                     **({'error_class': engine_class} if engine_class else {})}
            adapter.engine = SimpleNamespace(send=lambda request, proof=proof: {'ok': True, 'state': deepcopy(proof)})
            with self.assertRaises(Refusal) as caught:
                adapter._explicit_payment(0, {'kind': 'cast', 'card': 3}, {'kind': 'cast', 'pay': {}})
            self.assertEqual((caught.exception.class_, caught.exception.code), (class_, code), engine_class)

    def test_journal_restoration_matches_keys_instead_of_global_fetch_order(self):
        adapter = KarnAdapter(Path('/unused'), oracle=object())
        a = {'kind': 'shuffle', 'seat': 'A', 'order': ['a']}
        b = {'kind': 'shuffle', 'seat': 'B', 'order': ['b']}
        adapter.journal = [{'op': 'setup'}, {'op': 'act', 'n': 1, 'seat': 'B', 'do': {}, 'outcomes': [b, a]}]
        adapter.setup = lambda setup: None

        def act(n, seat, do, ask):
            self.assertEqual(ask({'kind': 'shuffle', 'seat': 'A'}), a)
            self.assertIsNone(ask({'kind': 'shuffle', 'seat': 'A'}))
            self.assertEqual(ask({'kind': 'shuffle', 'seat': 'B'}), b)

        adapter.act = act
        adapter._restore()

    def test_engine_drains_large_stderr_without_blocking_and_keeps_bounded_diagnostic(self):
        with TemporaryDirectory() as directory:
            executable = Path(directory) / 'diagnostic-engine'
            executable.write_text('#!/usr/bin/env python3\nimport sys,json\n'
                                  'print(json.dumps({"version":{}}),flush=True)\n'
                                  'sys.stdin.readline()\n'
                                  'sys.stderr.write("x"*131072+"meaningful engine failure\\n")\n'
                                  'sys.stderr.flush()\n'
                                  'print(json.dumps({"ok":False,"error":"fault"}),flush=True)\n')
            executable.chmod(0o755)
            engine = Engine(executable, timeout=5)
            try:
                with self.assertRaisesRegex(Refusal, 'meaningful engine failure'):
                    engine.must({'cmd': 'act'})
                self.assertLessEqual(len(engine.stderr_tail), 8192)
            finally:
                engine.close()


if __name__ == '__main__':
    unittest.main()
