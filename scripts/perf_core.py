#!/usr/bin/env python3
"""Measure prerecorded decisions through the Rust typed boundary, without players or pipes."""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parents[1]
TRACES = [
    "bant-ordered-mgn.jsonl",
    "vivi-mgn.jsonl",
    "torch-mgn.jsonl",
]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--engine", type=Path, default=ROOT / "target/release/game-serve")
    parser.add_argument("--runs", type=int, default=3)
    parser.add_argument("--trace", type=Path, action="append", help="default: three fixed MGN-derived supplied-action games")
    parser.add_argument("--output", type=Path, default=ROOT / ".cache/performance/core.json")
    parser.add_argument('--engine-source', help='Known engine build revision; omitted means unknown, not the current checkout')
    parser.add_argument('--build-profile', help='Build profile used for this binary, e.g. fast or release')
    args = parser.parse_args()
    if args.runs < 1:
        parser.error("--runs must be positive")
    engine = args.engine.resolve()
    traces = args.trace or [ROOT / "tests/fixtures/performance" / name for name in TRACES]
    report = {
        'engine_source': args.engine_source,
        'engine_source_provenance': 'operator-supplied build revision' if args.engine_source else 'unknown',
        'build_profile': args.build_profile,
        "harness_revision": subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip(),
        "engine_sha256": hashlib.sha256(engine.read_bytes()).hexdigest(),
        "workloads": [],
    }
    print("Single-threaded Rust engine; supplied actions and typed observations; obsolete discovery queries skipped.")
    print("Excludes player time, setup, JSON and transport. Run without competing builds/games.")
    print(f"{'WORKLOAD':44} {'DECISIONS':>9} {'SECONDS':>9} {'DECISIONS/S':>12} {'ACTION p99 ms':>15}", flush=True)
    for trace in traces:
        trace = trace.resolve()
        result = subprocess.run([str(engine), "--bench-replay", str(trace), "--runs", str(args.runs)],
                                cwd=ROOT, check=True, capture_output=True, text=True)
        sample = json.loads(result.stdout)
        sample["trace_sha256"] = hashlib.sha256(trace.read_bytes()).hexdigest()
        report["workloads"].append(sample)
        latency = sample["command_with_observation_latency"].get("act", {}).get("p99_ms", 0)
        print(f"{trace.stem:44} {sample['actions']:9} {sample['median_seconds']:9.3f} {sample['decisions_per_second']:12,.0f} {latency:15.3f}", flush=True)
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(report, indent=2) + "\n")
    print(f"Report: {args.output}")


if __name__ == "__main__":
    main()
