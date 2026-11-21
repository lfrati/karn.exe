#!/usr/bin/env python3
"""Fixed resident-worker layer baseline. Supplied-action replay only.

Each matrix row runs its workers on as many distinct physical cores: the engine process
is confined to one logical CPU of each of that many cores, read from the kernel's topology
(/sys/devices/system/cpu), so no two workers share a core's SMT threads. Cores of one type
come first on a hybrid processor (performance cores, then efficient ones). The report records
the topology and each row's placement. A row needing more cores than this process may run on
is refused, unless --allow-shared-cores places it on SMT siblings with a warning.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import resource
import subprocess
import tempfile
import time

from perf_core import ROOT, TRACES


def summarize(row):
    """Count all assignments; failed/capped work remains in the denominator/report."""
    replay = row['replay']
    replay['assigned'] = len(replay['results'])
    replay['failed'] = sum(not r['ok'] for r in replay['results'])
    return row


def read(path):
    try:
        return Path(path).read_text().strip()
    except OSError:
        return None


def cpu_list(text):
    """The CPUs of a kernel CPU list such as `0-3,8,10-11`."""
    cpus = []
    for part in text.split(','):
        first, _, last = part.partition('-')
        cpus += range(int(first), int(last or first) + 1)
    return cpus


# Intel hybrid processors list their core types' CPUs here; other machines have neither
CORE_TYPES = ('cpu_core', 'cpu_atom')


def topology(available, devices=Path('/sys/devices')):
    """The physical cores (package, and the logical CPUs that are its threads) of the CPUs
    this process may run on, with each core's type and maximum frequency where the kernel
    says them. A CPU whose topology the kernel does not say is refused, never guessed."""
    types = {}
    for kind in CORE_TYPES:
        listed = read(devices / kind / 'cpus')
        types.update({cpu: kind for cpu in cpu_list(listed)} if listed else {})
    cores = {}
    for cpu in available:
        base = devices / 'system/cpu' / f'cpu{cpu}'
        threads = read(base / 'topology/core_cpus_list') or read(base / 'topology/thread_siblings_list')
        package = read(base / 'topology/physical_package_id')
        if threads is None or package is None:
            raise SystemExit(f'the kernel does not say which physical core CPU {cpu} is ({base / "topology"})')
        top = read(base / 'cpufreq/cpuinfo_max_freq')
        core = cores.setdefault((int(package), tuple(cpu_list(threads))), {
            'package': int(package), 'threads': cpu_list(threads), 'available': [],
            'core_type': types.get(cpu), 'max_khz': int(top) if top else None})
        core['available'].append(cpu)
    ordered = sorted(cores.values(), key=lambda c: (CORE_TYPES.index(c['core_type']) if c['core_type'] in CORE_TYPES
                                                     else len(CORE_TYPES), c['package'], c['threads']))
    return {'source': str(devices / 'system/cpu'), 'available_affinity': sorted(available),
            'logical_cpus': len(available), 'physical_cores': ordered,
            'smt': any(len(c['threads']) > 1 for c in ordered)}


def placement(topo, workers, allow_shared=False):
    """The logical CPUs a row of `workers` runs on: one of each of `workers` distinct physical
    cores, in topology order. With fewer cores than workers, refused; or, with `allow_shared`,
    SMT siblings are added (every core's first thread, then its second, ...) and it is said so."""
    cores = topo['physical_cores']
    warnings = []
    if workers <= len(cores):
        chosen = [(core, core['available'][0]) for core in cores[:workers]]
    elif not allow_shared:
        raise SystemExit(f'{workers} workers need {workers} distinct physical cores; this process may run on '
                         f'{len(cores)} ({topo["logical_cpus"]} logical CPUs {topo["available_affinity"]}): '
                         'run fewer workers, or pass --allow-shared-cores to measure SMT sharing')
    else:
        rounds = max(len(core['available']) for core in cores)
        chosen = [(core, core['available'][i]) for i in range(rounds) for core in cores if i < len(core['available'])]
        if workers > len(chosen):
            raise SystemExit(f'{workers} workers exceed the {len(chosen)} logical CPUs this process may run on')
        chosen = chosen[:workers]
        warnings.append(f'{workers} workers share {len(cores)} physical cores: SMT threads of one core compete, '
                        'so this row does not measure per-core scaling')
    kinds = sorted({str(core['core_type']) for core, _ in chosen})
    if len(kinds) > 1:
        warnings.append(f'the placement mixes core types {kinds}: workers do not run at one speed')
    return {'workers': workers, 'cpus': [cpu for _, cpu in chosen],
            'physical_cores': [core['threads'] for core, _ in chosen],
            'core_types': [core['core_type'] for core, _ in chosen],
            'distinct_physical_cores': len({tuple(core['threads']) for core, _ in chosen}) == len(chosen),
            'warnings': warnings}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--engine', type=Path, default=ROOT / 'target/release/game-serve')
    parser.add_argument('--workers', default='1,2,4,8,16')
    parser.add_argument('--allow-shared-cores', action='store_true',
                        help='run a row with more workers than physical cores on SMT siblings, with a warning')
    parser.add_argument('--ram-gib', type=int, default=8, help='Per-process address-space ceiling; also report actual RSS')
    parser.add_argument('--copies', type=int, default=1)
    parser.add_argument('--timeout', type=float, default=180)
    parser.add_argument('--trace', type=Path, action='append')
    parser.add_argument('--output', type=Path, default=ROOT / '.cache/performance/layer-parallel.json')
    parser.add_argument('--engine-source', help='Known engine build revision; omitted means unknown, not the current checkout')
    parser.add_argument('--build-profile', help='Build profile used for this binary, e.g. fast or release')
    args = parser.parse_args()
    workers = [int(n) for n in args.workers.split(',')]
    available = sorted(os.sched_getaffinity(0))
    if min(workers + [args.ram_gib, args.copies]) < 1 or args.timeout <= 0:
        parser.error('positive workers/budgets/limits required')
    topo = topology(available)
    # every row placed before any runs: a refused row refuses the matrix
    placements = {count: placement(topo, count, args.allow_shared_cores) for count in workers}
    for count, placed in placements.items():
        for warning in placed['warnings']:
            print(f'warning: workers={count}: {warning}', flush=True)
    engine = args.engine.resolve()
    paths = args.trace or [ROOT / 'tests/fixtures/performance' / n for n in TRACES]
    report = {
        'engine_source': args.engine_source,
        'engine_source_provenance': 'operator-supplied build revision' if args.engine_source else 'unknown',
        'build_profile': args.build_profile,
        'harness_revision': subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip(),
        'engine_sha256': hashlib.sha256(engine.read_bytes()).hexdigest(),
        'traces': [{'path': str(p.relative_to(ROOT) if p.is_relative_to(ROOT) else p), 'sha256': hashlib.sha256(p.read_bytes()).hexdigest()} for p in paths],
        'topology': topo,
        'budget': {'available_affinity': available, 'address_space_bytes': args.ram_gib * 1024**3,
                   'memory_total': read('/proc/meminfo'), 'cpu_hardware': read('/proc/cpuinfo'),
                   'cgroup_membership': read('/proc/self/cgroup'), 'cgroup_cpu_quota': read('/sys/fs/cgroup/user.slice/cpu.max'),
                   'cgroup_memory_limit': read('/sys/fs/cgroup/user.slice/memory.max')},
        'scope': 'One resident Rust process per matrix row; threads replay supplied actions and share immutable definitions. Obsolete discovery diagnostics skipped and counted. No generated policy games. Same mix per worker, address-space ceiling; each row confined to one logical CPU of each of as many distinct physical cores as it has workers (placement), the OS scheduling its threads within them. No competing builds/games.',
        'matrix': [],
    }
    args.output.parent.mkdir(parents=True, exist_ok=True)
    for count in workers:
        config = {'traces': [str(p.resolve()) for p in paths], 'workers': count, 'copies': args.copies, 'timeout': args.timeout}
        placed = placements[count]
        def budget():
            os.sched_setaffinity(0, placed['cpus'])
            resource.setrlimit(resource.RLIMIT_AS, (args.ram_gib * 1024**3, args.ram_gib * 1024**3))
        with tempfile.NamedTemporaryFile('w', suffix='.json') as f:
            json.dump(config, f)
            f.flush()
            began = time.perf_counter()
            proc = subprocess.run([str(engine), '--bench-parallel', f.name], cwd=ROOT, capture_output=True, text=True, preexec_fn=budget)
            if proc.returncode:
                row = {'workers': count, 'assigned_replays': count * args.copies * len(paths), 'process_failure': proc.stderr, 'returncode': proc.returncode, 'total_seconds': time.perf_counter() - began}
            else:
                row = summarize(json.loads(proc.stdout))
                row['total_process_seconds'] = time.perf_counter() - began
            row['placement'] = placed
            if 'replay' in row and report['matrix'] and 'replay' in report['matrix'][0]:
                base = report['matrix'][0]
                row['replay_scaling'] = row['replay']['decisions_per_second'] / base['replay']['decisions_per_second']
            report['matrix'].append(row)
            args.output.write_text(json.dumps(report, indent=2) + '\n')
            print(f"workers={count}: " + (f"{row['replay']['decisions_per_second']:.1f} replay decisions/s; peak {row['peak_rss_kib']/1024:.1f} MiB" if 'replay' in row else f"PROCESS FAILED: {row['process_failure']}"), flush=True)
    print(f'Report: {args.output}', flush=True)
    if any('process_failure' in r or r['replay']['failed'] for r in report['matrix']):
        raise SystemExit(1)


if __name__ == '__main__':
    main()
