#!/usr/bin/env python3
"""Run native scaling probes, preserving failures and build/workload provenance."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[1]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--engine', type=Path, default=ROOT / 'target/release/game-scale')
    parser.add_argument('--objects', default='100,400,1000')
    parser.add_argument('--players', type=int, default=4)
    parser.add_argument('--runs', type=int, default=9)
    parser.add_argument('--iterations', type=int, default=200)
    parser.add_argument('--engine-source')
    parser.add_argument('--build-profile')
    parser.add_argument('--output', type=Path, default=ROOT / '.cache/performance/scale.json')
    args = parser.parse_args()
    try:
        sizes = [int(n) for n in args.objects.split(',')]
    except ValueError:
        parser.error('--objects must be comma-separated positive integers')
    if not sizes or any(n < 1 or n > 100000 for n in sizes) or not 2 <= args.players <= 16 or min(args.runs, args.iterations) < 1:
        parser.error('invalid object counts, players, runs or iterations')
    config = dict(objects=sizes, players=args.players, runs=args.runs, iterations=args.iterations)
    engine = args.engine.resolve()
    report = dict(engine_source=args.engine_source, build_profile=args.build_profile,
                  engine_source_provenance='operator-supplied build revision' if args.engine_source else 'unknown',
                  engine_sha256=hashlib.sha256(engine.read_bytes()).hexdigest(),
                  harness_revision=subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip(),
                  environment=dict(platform=platform.platform(), cpu_affinity=sorted(os.sched_getaffinity(0)),
                                   rustc=subprocess.check_output(['rustc', '-V'], text=True).strip()))
    with tempfile.TemporaryDirectory(prefix='karn-scale-') as folder:
        path = Path(folder) / 'config.json'
        path.write_text(json.dumps(config))
        result = subprocess.run([str(engine), str(path)], cwd=ROOT, capture_output=True, text=True)
    try:
        measured = json.loads(result.stdout)
        report['workload_source_sha256'] = hashlib.sha256(measured.pop('workload_source').encode()).hexdigest()
        failed = result.returncode != 0 or len(measured['workloads']) != 7 * len(sizes) or any(row['status'] != 'passed' for row in measured['workloads'])
        report.update(measured)
    except (ValueError, KeyError):
        report.update(config=config, workloads=[], error=result.stderr or result.stdout)
        failed = True
    report['status'] = 'failed' if failed else 'passed'
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(report, indent=2) + '\n')
    for row in report['workloads']:
        if row['status'] == 'passed':
            print(f"{row['name']:42} n={row['objects']:5} {row['ns_per_operation']:12,.0f} ns/op")
        else:
            print(f"FAILED n={row['objects']}: {row['error']}")
    print(f'Report: {args.output}')
    if failed:
        raise SystemExit(report.get('error', 'one or more scaling workloads failed'))


if __name__ == '__main__':
    main()
