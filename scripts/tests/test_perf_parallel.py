import unittest
import sys
import tempfile
from pathlib import Path
sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from perf_parallel import cpu_list, default_workers, placement, summarize, topology


def devices(root, cores, types=None):
    """A /sys/devices tree: `cores` lists each physical core's logical CPUs (package 0, the
    first two cores at 4.8 GHz, the rest at 3.6), `types` the CPU lists of a hybrid's core types."""
    for threads in cores:
        for cpu in threads:
            base = root / 'system/cpu' / f'cpu{cpu}'
            (base / 'topology').mkdir(parents=True)
            (base / 'cpufreq').mkdir()
            (base / 'topology/core_cpus_list').write_text(','.join(map(str, threads)) + '\n')
            (base / 'topology/physical_package_id').write_text('0\n')
            (base / 'cpufreq/cpuinfo_max_freq').write_text('4800000\n' if cores.index(threads) < 2 else '3600000\n')
    for kind, listed in (types or {}).items():
        (root / kind).mkdir()
        (root / kind / 'cpus').write_text(listed + '\n')
    return root


class ParallelReportTests(unittest.TestCase):
    def test_failures_remain_in_assigned_work(self):
        row = {'replay': {'results': [{'ok': True}, {'ok': False}]}}
        summary = summarize(row)
        self.assertEqual(summary['replay']['assigned'], 2)
        self.assertEqual(summary['replay']['failed'], 1)


class PlacementTests(unittest.TestCase):
    def setUp(self):
        temp = tempfile.TemporaryDirectory()
        self.addCleanup(temp.cleanup)
        self.root = Path(temp.name)

    def test_kernel_cpu_lists(self):
        self.assertEqual(cpu_list('0-3,8,10-11'), [0, 1, 2, 3, 8, 10, 11])

    def test_workers_go_to_distinct_physical_cores_never_to_smt_siblings(self):
        # four cores of two threads each, numbered as this machine's are: siblings adjacent
        topo = topology(range(8), devices(self.root, [[0, 1], [2, 3], [4, 5], [6, 7]]))
        self.assertTrue(topo['smt'])
        self.assertEqual([c['threads'] for c in topo['physical_cores']], [[0, 1], [2, 3], [4, 5], [6, 7]])
        self.assertEqual([c['max_khz'] for c in topo['physical_cores']], [4800000, 4800000, 3600000, 3600000])
        placed = placement(topo, 4)
        # the first four logical CPUs would be two cores' threads; one CPU of each core instead
        self.assertEqual(placed['cpus'], [0, 2, 4, 6])
        self.assertEqual(placed['physical_cores'], [[0, 1], [2, 3], [4, 5], [6, 7]])
        self.assertTrue(placed['distinct_physical_cores'])
        self.assertEqual(placed['warnings'], [])

    def test_only_the_cpus_this_process_may_run_on_are_placed(self):
        # affinity without CPU 0: core [0, 1] is reached through its other thread
        topo = topology([1, 3, 4, 5], devices(self.root, [[0, 1], [2, 3], [4, 5]]))
        self.assertEqual(placement(topo, 3)['cpus'], [1, 3, 4])

    def test_default_matrix_fits_one_two_and_twelve_physical_cores(self):
        for count, expected in [(1, [1]), (2, [1, 2]), (12, [1, 2, 4, 8, 12])]:
            with self.subTest(cores=count):
                topo = topology(range(count * 2), devices(self.root / str(count), [[i, i + count] for i in range(count)]))
                workers = default_workers(topo)
                self.assertEqual(workers, expected)
                for row in workers:
                    self.assertTrue(placement(topo, row)['distinct_physical_cores'])

    def test_default_matrix_uses_affinity_reachable_cores_without_counting_smt_twice(self):
        topo = topology([1, 3, 4, 5], devices(self.root, [[0, 1], [2, 3], [4, 5], [6, 7]]))
        self.assertEqual(default_workers(topo), [1, 2, 3])
        self.assertEqual(placement(topo, default_workers(topo)[-1])['cpus'], [1, 3, 4])

    def test_more_workers_than_physical_cores_is_refused_unless_sharing_is_asked_for(self):
        topo = topology(range(4), devices(self.root, [[0, 1], [2, 3]]))
        with self.assertRaisesRegex(SystemExit, '4 workers need 4 distinct physical cores; this process may run on 2'):
            placement(topo, 4)
        shared = placement(topo, 3, allow_shared=True)
        # each core's first thread, then a second thread: and it says so
        self.assertEqual(shared['cpus'], [0, 2, 1])
        self.assertFalse(shared['distinct_physical_cores'])
        self.assertIn('3 workers share 2 physical cores', shared['warnings'][0])
        with self.assertRaisesRegex(SystemExit, 'exceed the 4 logical CPUs'):
            placement(topo, 5, allow_shared=True)

    def test_a_hybrid_places_performance_cores_first_and_warns_when_it_mixes_types(self):
        # an efficient core listed first by number still comes after the performance cores
        topo = topology(range(6), devices(self.root, [[0], [1], [2, 3], [4, 5]],
                                          types={'cpu_core': '2-5', 'cpu_atom': '0-1'}))
        self.assertEqual([c['core_type'] for c in topo['physical_cores']], ['cpu_core', 'cpu_core', 'cpu_atom', 'cpu_atom'])
        self.assertEqual(placement(topo, 2)['cpus'], [2, 4])
        self.assertEqual(placement(topo, 2)['warnings'], [])
        mixed = placement(topo, 3)
        self.assertEqual((mixed['cpus'], mixed['core_types']), ([2, 4, 0], ['cpu_core', 'cpu_core', 'cpu_atom']))
        self.assertIn("mixes core types ['cpu_atom', 'cpu_core']", mixed['warnings'][0])

    def test_a_cpu_whose_core_the_kernel_does_not_say_is_refused(self):
        root = devices(self.root, [[0, 1]])
        (root / 'system/cpu/cpu1/topology/core_cpus_list').unlink()
        with self.assertRaisesRegex(SystemExit, 'which physical core CPU 1 is'):
            topology([0, 1], root)


if __name__ == '__main__':
    unittest.main()
