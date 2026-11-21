"""Replay supplied actions from immutable complete-deck traces.

Build game-serve separately, then choose a trace in tests/fixtures/performance.
Timings are protocol round trips, including projection and transport, not pure rules-core
execution. Execution replay skips obsolete move-discovery queries. --actions-only also
omits state queries. Original discovery checkpoints cannot validate this execution boundary;
response digests compare equivalent runs of the same mode.
"""
import argparse
import hashlib
import json
import math
import os
from pathlib import Path
import statistics
import subprocess
import threading
import time

from rust_tools import ROOT, fast_binary


def checkpoint(answer):
    """Exact projected answer, including choices, events and disclosure delivery."""
    state = answer.get("state", {})
    if "prompt" in answer:
        # a seat's prompt (player/1): the question is its decision, its chooser a label
        prompt = answer["prompt"]
        decision = prompt.get("decision") or {}
        return {"sha256": hashlib.sha256(json.dumps(answer, sort_keys=True).encode()).hexdigest(),
                "turn": prompt.get("turn"), "part": prompt.get("part"), "seat": prompt.get("seat"),
                "asked": decision.get("kind"), "player": decision.get("chooser"),
                "finished": prompt.get("finished"), "winner": prompt.get("winner")}
    asked = state.get("asked") or {}
    return {"sha256": hashlib.sha256(json.dumps(answer, sort_keys=True).encode()).hexdigest(),
            "turn": state.get("turn"), "part": state.get("part"),
            "asked": asked.get("kind"), "player": asked.get("player"),
            "finished": state.get("finished"), "winner": state.get("winner")}


def summarize(samples):
    ordered = sorted(samples)
    return {"count": len(ordered), "seconds": sum(ordered),
            "median_seconds": statistics.median(ordered),
            "p95_seconds": ordered[math.ceil(len(ordered) * .95) - 1],
            "max_seconds": ordered[-1]}


def summarize_internal(path, requests):
    rows = [json.loads(line) for line in path.read_text().splitlines()]
    if len(rows) != len(requests):
        raise RuntimeError(f"profile has {len(rows)} rows for {len(requests)} commands")
    def merge(selected):
        counters, timing = {}, {}
        for row in selected:
            for key, value in row["counts"].items():
                counters[key] = counters.get(key, 0) + value
            for key, fields in row["timing"].items():
                dest = timing.setdefault(key, {field: 0 for field in fields})
                for field, value in fields.items():
                    dest[field] += value
        return {"counts": counters, "timing": timing}
    top = sorted(enumerate(rows), key=lambda pair: pair[1]["timing"].get("options", {}).get("inclusive_ns", 0), reverse=True)[:5]
    return {"raw": str(path), "meaning": "Nanoseconds inside Rust. Inclusive scopes overlap (including recursive calls); do not sum them. Instrumentation adds overhead.",
            "all": merge(rows),
            "by_command": {cmd: merge(row for row, req in zip(rows, requests) if req["cmd"] == cmd)
                           for cmd in sorted({req["cmd"] for req in requests})},
            "slowest_options": [{"command": i + 1, "profile": row} for i, row in top
                                if row["timing"].get("options", {}).get("calls", 0)]}


class ReplayDivergence(RuntimeError):
    """The first exact-checkpoint mismatch; retain both contexts for a failure report."""
    def __init__(self, number, request, expected, actual):
        self.details = {"command": number, "request": request, "expected": expected, "actual": actual}
        super().__init__(f"first divergence at command {number}: {request}; "
                         f"expected {expected}, actual {actual}")


class SuiteFailure(RuntimeError):
    """A failed corpus still has an explicit report, never a passing timing result."""
    def __init__(self, report):
        self.report = report
        failures = [row for row in report["games"] if row["status"] == "failed"]
        first = failures[0]
        super().__init__(f"{first['id']}: {first['error']} ({len(failures)} games failed)")


def replay(engine, requests, timeout, profile_path=None, expected=None, audit=False, on_answer=None):
    if expected is not None and len(expected) != len(requests):
        raise RuntimeError("checkpoint count differs from command count")
    started = time.perf_counter()
    env = os.environ.copy()
    if profile_path is not None:
        profile_path.parent.mkdir(parents=True, exist_ok=True)
        profile_path.unlink(missing_ok=True)
        env["MAGIC_PROFILE"] = str(profile_path.resolve())
    process = subprocess.Popen([str(engine)], cwd=ROOT, stdin=subprocess.PIPE,
                               stdout=subprocess.PIPE, text=True, bufsize=1, env=env)
    watchdog = threading.Timer(timeout, process.kill)
    watchdog.start()
    digest = hashlib.sha256()
    timings = {}
    auditor = None
    pending_views = []
    audited_views = 0
    if audit:
        from play import Engine
        auditor = object.__new__(Engine)
        auditor.disclosed = {}
    try:
        ready = json.loads(process.stdout.readline())
        if ready.get("type") != "ready":
            raise RuntimeError(f"engine did not become ready: {ready}")
        startup = time.perf_counter() - started
        for number, request in enumerate(requests, 1):
            began = time.perf_counter()
            process.stdin.write(json.dumps(request) + "\n")
            process.stdin.flush()
            line = process.stdout.readline()
            if not line:
                reason = "timeout" if time.perf_counter() - started >= timeout else "engine stopped"
                raise RuntimeError(f"{reason} at recorded command {number}: {request}")
            answer = json.loads(line)
            timings.setdefault(request["cmd"], []).append(time.perf_counter() - began)
            if not answer.get("ok"):
                raise RuntimeError(f"recorded command {number} was rejected: {answer}; command: {request}")
            if expected is not None and checkpoint(answer) != expected[number - 1]:
                raise ReplayDivergence(number, request, expected[number - 1], checkpoint(answer))
            if on_answer is not None:
                on_answer(number, request, answer)
            digest.update(json.dumps(answer, sort_keys=True).encode())
            if auditor is not None:
                # Reuse the recorded audit query. Additional state requests drain host
                # delivery bookkeeping and would change the exact replay being tested.
                if request["cmd"] == "act":
                    pending_views.clear()
                state = answer["state"]
                if state.get("viewer") is not None and state.get("asked"):
                    pending_views.append(state)
                elif request["cmd"] == "state" and state.get("viewer") is None:
                    auditor.send = lambda request: state
                    for view in pending_views:
                        auditor.state = view
                        try:
                            auditor.check()
                        except AssertionError as error:
                            raise RuntimeError(f"privacy audit at command {number}: {request}: {error}") from error
                        audited_views += 1
                    pending_views.clear()
        final = answer["state"]
        result = {"startup_seconds": startup, "total_seconds": time.perf_counter() - started,
                "commands": len(requests), "response_sha256": digest.hexdigest(), "audited_views": audited_views,
                "round_trips": {kind: summarize(values) for kind, values in timings.items()},
                "final": {key: final[key] for key in ("turn", "winner", "finished", "request_id")}}
    finally:
        watchdog.cancel()
        process.stdin.close()
        try:
            process.wait(timeout=5)
        except subprocess.TimeoutExpired:
            process.kill()
            process.wait()
        process.stdout.close()
    if profile_path is not None:
        if not profile_path.exists():
            raise RuntimeError("No internal profile: build game-serve with --features profile")
        result["internal"] = summarize_internal(profile_path, requests)
    return result


def execution_requests(requests, actions_only=False):
    """Retain supplied actions; account for omitted historic discovery diagnostics."""
    skipped = {}
    kept = []
    for request in requests:
        command = request["cmd"]
        if command in ("options", "moves") or (actions_only and command == "state"):
            skipped[command] = skipped.get(command, 0) + 1
        else:
            kept.append(request)
    return kept, skipped


def load_game(manifest, game):
    """A manifest game's commands and recorded checkpoints, after verifying its fixtures."""
    trace = manifest.parent / game["trace"]
    checks = manifest.parent / game["checkpoints"]
    for path, field in [(trace, "trace_sha256"), (checks, "checkpoints_sha256")]:
        if hashlib.sha256(path.read_bytes()).hexdigest() != game[field]:
            raise RuntimeError(f"{game['id']}: fixture hash changed: {path}")
    requests = [json.loads(line) for line in trace.read_text().splitlines()]
    expected = [json.loads(line) for line in checks.read_text().splitlines()]
    if requests[0].get("cmd") != "start" or not all(isinstance(d, list) for d in requests[0]["decks"]):
        raise RuntimeError(f"{game['id']}: requires normal start with durable inline decks")
    if any(row["cmd"] not in ("act", "options", "state") for row in requests[1:]):
        raise RuntimeError(f"{game['id']}: only one initial setup is allowed")
    if sum(row["cmd"] == "act" for row in requests) != game["actions"]:
        raise RuntimeError(f"{game['id']}: recorded action count differs from manifest")
    if not expected[-1].get("finished") or expected[-1].get("winner") != game["winner"] or game["winner"] is None:
        raise RuntimeError(f"{game['id']}: requires a recorded terminal winner")
    return trace, checks, requests, expected


def suite(manifest, engine, timeout, performance=False, runs=1, execution_only=False):
    """One explicit complete-game corpus; correctness has no timing threshold."""
    data = json.loads(manifest.read_text())
    reports = []
    began = time.perf_counter()
    for game in data["games"]:
        row = {"id": game["id"], "trace_sha256": game["trace_sha256"], "actions": game["actions"]}
        try:
            trace, _, requests, expected = load_game(manifest, game)
            if execution_only:
                requests, skipped = execution_requests(requests)
                row["skipped_diagnostics"] = skipped
                checked = replay(engine, requests, timeout, audit=True)
                row["historical_checkpoints"] = "preserved; not compared across the changed execution interface"
            else:
                checked = replay(engine, requests, timeout, expected=expected, audit=True)
            row.update(status="passed", validation=checked)
            if performance:
                result = subprocess.run([str(engine), "--bench-replay", str(trace.resolve()), "--runs", str(runs)],
                                        cwd=ROOT, check=True, capture_output=True, text=True, timeout=timeout)
                row["typed"] = json.loads(result.stdout)
        except (RuntimeError, OSError, json.JSONDecodeError, subprocess.CalledProcessError,
                subprocess.TimeoutExpired) as error:
            row.update(status="failed", error=str(error))
            if isinstance(error, ReplayDivergence):
                row["divergence"] = error.details
            print(f"{game['id']}: FAILED: {error}", flush=True)
        else:
            seconds = row.get("typed", {}).get("median_seconds", checked["total_seconds"])
            print(f"{game['id']}: winner {game['winner']}, {game['actions']} decisions, {seconds:.3f}s", flush=True)
        reports.append(row)
    report = {"manifest_sha256": hashlib.sha256(manifest.read_bytes()).hexdigest(),
              "engine_sha256": hashlib.sha256(engine.read_bytes()).hexdigest(),
              "status": "failed" if any(row["status"] == "failed" for row in reports) else "passed",
              "mode": ("typed_execution_performance" if performance else "supplied_action_execution_with_privacy_audit")
                      if execution_only else "historical_exact_checkpoints",
              "elapsed_seconds": time.perf_counter() - began, "games": reports,
              "limits": data.get("coverage_limits", [])}
    if report["status"] == "failed":
        raise SuiteFailure(report)
    return report


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("trace", type=Path, nargs="?")
    parser.add_argument("--manifest", type=Path, help="explicit complete-deck execution replay suite")
    parser.add_argument("--performance", action="store_true", help="with --manifest, measure the same games through existing typed replay")
    parser.add_argument("--engine", type=Path, default=fast_binary("game-serve"))
    parser.add_argument("--actions-only", action="store_true")
    parser.add_argument("--execution-only", action="store_true", help="with --manifest, skip obsolete discovery diagnostics; do not compare historical checkpoints")
    parser.add_argument("--timeout", type=float, default=180)
    parser.add_argument("--runs", type=int, default=1)
    parser.add_argument("--expect-sha", help="require this response digest for every run")
    parser.add_argument("--output", type=Path)
    parser.add_argument("--profile-dir", type=Path, help="write per-command internal profiles (requires --features profile)")
    args = parser.parse_args()
    if args.runs < 1 or args.timeout <= 0:
        parser.error("runs and timeout must be positive")
    if args.manifest:
        if args.trace or args.actions_only or args.profile_dir or args.expect_sha:
            parser.error("--manifest is separate from individual trace/profile modes")
        failure = None
        try:
            report = suite(args.manifest, args.engine.resolve(), args.timeout, args.performance, args.runs,
                           execution_only=args.execution_only)
        except SuiteFailure as error:
            report, failure = error.report, error
        if args.output:
            args.output.parent.mkdir(parents=True, exist_ok=True)
            args.output.write_text(json.dumps(report, indent=2) + "\n")
        else:
            print(json.dumps(report, indent=2))
        if failure is not None:
            raise SystemExit(str(failure))
        return
    if args.trace is None or args.performance or args.execution_only:
        parser.error("choose a trace or --manifest; --performance requires --manifest")
    raw = args.trace.read_bytes()
    requests = [json.loads(line) for line in raw.splitlines() if line.strip()]
    requests, skipped = execution_requests(requests, args.actions_only)
    if not requests or requests[0]["cmd"] != "start":
        parser.error("trace must begin with start")
    results = [replay(args.engine.resolve(), requests, args.timeout,
                      args.profile_dir / f"run-{i + 1}.jsonl" if args.profile_dir else None)
               for i in range(args.runs)]
    if args.expect_sha and any(r["response_sha256"] != args.expect_sha for r in results):
        raise SystemExit("response digest differs from --expect-sha")
    report = {"engine": str(args.engine.resolve()), "trace": str(args.trace),
              "engine_sha256": hashlib.sha256(args.engine.read_bytes()).hexdigest(),
              "trace_sha256": hashlib.sha256(raw).hexdigest(),
              "mode": "supplied_actions" if args.actions_only else "execution_and_observation",
              "skipped_diagnostics": skipped,
              "meaning": "Protocol timings include engine projection and transport; this is not pure core throughput.",
              "runs": results}
    encoded = json.dumps(report, indent=2) + "\n"
    if args.output:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(encoded)
        if args.profile_dir:
            for i, result in enumerate(results, 1):
                internal = result["internal"]["all"]
                timing, counts = internal["timing"], internal["counts"]
                secs = lambda key: timing.get(key, {}).get("inclusive_ns", 0) / 1e9
                print(f"Run {i} (instrumented): actions {secs('respond'):.3f}s; projection {secs('projection'):.3f}s")
                print(f"  Characteristic queries: {timing['seen']['calls']:,}; "
                      f"world records initialized: {counts['world_objects']:,}; "
                      f"dependency graphs: {counts['dependency_graphs']:,}")
            print(f"Report: {args.output}")
    else:
        print(encoded, end="")


if __name__ == "__main__":
    main()
