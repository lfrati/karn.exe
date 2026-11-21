"""Internal timing summaries preserve scope overlap rather than adding it as elapsed time."""
import json
from pathlib import Path
import sys
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from perf_replay import checkpoint, summarize_internal


class InternalProfileTests(unittest.TestCase):
    def test_groups_commands_without_flattening_nested_times(self):
        rows = [
            {'counts': {'worlds': 3}, 'timing': {'options': {'calls': 1, 'inclusive_ns': 100, 'exclusive_ns': 20},
                                             'seen': {'calls': 3, 'inclusive_ns': 80, 'exclusive_ns': 80}}},
            {'counts': {'worlds': 1}, 'timing': {'options': {'calls': 0, 'inclusive_ns': 0, 'exclusive_ns': 0},
                                             'seen': {'calls': 1, 'inclusive_ns': 10, 'exclusive_ns': 10}}},
        ]
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / 'profile.jsonl'
            path.write_text(''.join(json.dumps(row) + '\n' for row in rows))
            report = summarize_internal(path, [{'cmd': 'options'}, {'cmd': 'act'}])
            self.assertEqual(report['all']['counts']['worlds'], 4)
            self.assertEqual(report['by_command']['act']['timing']['seen']['inclusive_ns'], 10)
            self.assertEqual(report['all']['timing']['options']['inclusive_ns'], 100)
            self.assertEqual(report['slowest_options'][0]['command'], 1)
            with self.assertRaisesRegex(RuntimeError, '2 rows for 1 commands'):
                summarize_internal(path, [{'cmd': 'act'}])


class PlayerCounterCheckpointTests(unittest.TestCase):
    def test_empty_and_nonempty_player_counters_both_change_exact_digest(self):
        old = {"state": {"players": [{"life": 20}, {"life": 20}]}}
        empty = {"state": {"players": [{"life": 20, "counters": {}}, {"life": 20, "counters": {}}]}}
        counted = {"state": {"players": [{"life": 20, "counters": {"poison": 1}}, {"life": 20, "counters": {}}]}}
        self.assertNotEqual(checkpoint(old), checkpoint(empty))
        self.assertNotEqual(checkpoint(old), checkpoint(counted))
        self.assertNotEqual(checkpoint(empty), checkpoint(counted))
        self.assertEqual(empty["state"]["players"][0]["counters"], {})

    def test_falsey_nonmap_counters_remain_visible_in_digest(self):
        old = {"state": {"players": [{"life": 20}]}}
        digests = {checkpoint(old)["sha256"]}
        for value in (None, [], 0, False):
            with self.subTest(counters=value):
                answer = {"state": {"players": [{"life": 20, "counters": value}]}}
                digest = checkpoint(answer)["sha256"]
                self.assertNotIn(digest, digests)
                digests.add(digest)
                self.assertIs(answer["state"]["players"][0]["counters"], value)

class ExecutionScopeTests(unittest.TestCase):
    def test_discovery_is_accounted_without_changing_supplied_actions(self):
        from copy import deepcopy
        from perf_replay import execution_requests
        actions = [{'cmd': 'start', 'decks': [[], []]}, {'cmd': 'options'},
                   {'cmd': 'act', 'move': {'kind': 'cast', 'card': 3, 'plan': []}},
                   {'cmd': 'state'}, {'cmd': 'moves'}]
        frozen = deepcopy(actions)
        kept, skipped = execution_requests(actions)
        self.assertEqual(kept, [actions[0], actions[2], actions[3]])
        self.assertEqual(skipped, {'options': 1, 'moves': 1})
        self.assertEqual(actions, frozen)
        kept, skipped = execution_requests(actions, actions_only=True)
        self.assertEqual(kept, [actions[0], actions[2]])
        self.assertEqual(skipped, {'options': 1, 'state': 1, 'moves': 1})

    def test_profile_does_not_require_retired_search_instrumentation(self):
        rows = [{'counts': {}, 'timing': {'respond': {'calls': 1, 'inclusive_ns': 100}}}]
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / 'profile.jsonl'
            path.write_text(json.dumps(rows[0]) + '\n')
            self.assertEqual(summarize_internal(path, [{'cmd': 'act'}])['slowest_options'], [])
