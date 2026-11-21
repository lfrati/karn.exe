"""Run the Python tooling tests (scripts/tests) module by module in parallel, printing one
unittest-style log: each module's verbose output as it finishes, then the slowest tests, the
total `Ran N tests in Ts` and one `OK` or `FAILED (...)` line, as `unittest discover -v` would
print for the whole run, so that the counts and failures read the same way.

Most modules play games through their own `game-serve`, independent of one another; they
run `--workers` at a time. Modules which edit this checkout's tooling sources or build in a
profile of their own (SERIAL) run after the rest, one at a time. A module that produces no
result (an import error, a crash) is reported as an error of that module.

Usage: tooling_tests.py [--workers N] [--durations N] [MODULE ...]
"""
from __future__ import annotations

import argparse
from concurrent.futures import ThreadPoolExecutor
import os
from pathlib import Path
import re
import subprocess
import sys
import time

ROOT = Path(__file__).resolve().parent.parent
TESTS = ROOT / "scripts" / "tests"
# test_rule_evidence_fixtures appends to scripts/rule_evidence.py and rules.py while it checks
# staleness, and builds the rules target in a check profile of its own
SERIAL = ("test_rule_evidence_fixtures",)
RAN = re.compile(r"^Ran (\d+) tests? in ([\d.]+)s$", re.M)
SUMMARY = re.compile(r"^(OK|FAILED)(?: \((.*)\))?$", re.M)
DURATION = re.compile(r"^(\d+\.\d+)s\s+(.*)$", re.M)
COUNTS = ("failures", "errors", "skipped", "expected failures", "unexpected successes")


def workers_default() -> int:
    # bounded: the Rust workspace tests may run alongside, and each module is one Python
    # process driving one engine process
    return max(2, ((os.cpu_count() or 4) - 2) // 3)


def modules() -> list[str]:
    return sorted(p.stem for p in TESTS.glob("test_*.py"))


def last(pattern: re.Pattern, text: str) -> re.Match | None:
    """The last match of `pattern` in `text`, if any."""
    return next(reversed(list(pattern.finditer(text))), None)


def run_module(module: str, durations: int) -> dict:
    """One module's run: its output and the counts it reports."""
    started = time.monotonic()
    # the repository and scripts/ importable, as under `unittest discover -s scripts/tests`
    # where any module's sys.path insertion served every later module of the same process
    path = [str(ROOT), str(ROOT / "scripts"), os.environ.get("PYTHONPATH")]
    env = {**os.environ, "PYTHONPATH": os.pathsep.join(filter(None, path))}
    process = subprocess.run([sys.executable, "-m", "unittest", "-v", "--durations", str(durations), module],
                             cwd=TESTS, env=env, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
    elapsed = time.monotonic() - started
    # unittest reports on stderr; what a test prints goes to stdout, buffered apart (a footer
    # printed there would land after the real one), so the report is read on its own
    output = process.stderr
    # the last footer: a test may run a nested unittest, which reports before the module does
    ran, summary = last(RAN, output), last(SUMMARY, output)
    counts = {key: 0 for key in COUNTS}
    if summary and summary[2]:
        for part in summary[2].split(", "):
            key, _, value = part.partition("=")
            if key in counts:
                counts[key] = int(value)
    slowest = []
    block = output.rfind("Slowest test durations")
    if block >= 0:
        slowest = [(float(seconds), name) for seconds, name in DURATION.findall(output[block:ran.start() if ran else len(output)])]
    # the module's own footer (durations, Ran, OK/FAILED) is replaced by the run's
    body = output[:block] if block >= 0 else output
    if ran and block < 0:
        body = output[:ran.start()]
    if process.stdout.strip():
        body = f"{body.rstrip(chr(10))}\n-- printed by {module}:\n{process.stdout}"
    return {"module": module, "exit_code": process.returncode, "elapsed": elapsed, "body": body.rstrip("\n"),
            "tests": int(ran[1]) if ran else None, "ok": bool(summary and summary[1] == "OK") if ran else None,
            "counts": counts, "slowest": slowest}


def footer(results: list[dict], elapsed: float, durations: int) -> str:
    totals = {key: sum(r["counts"][key] for r in results) for key in COUNTS}
    crashed = [r for r in results if r["tests"] is None]
    totals["errors"] += len(crashed)
    tests = sum(r["tests"] or 0 for r in results) + len(crashed)
    lines = []
    slowest = sorted((d for r in results for d in r["slowest"]), reverse=True)[:durations]
    if durations and slowest:
        lines += ["", "Slowest test durations", *(f"{seconds:.3f}s     {name}" for seconds, name in slowest)]
    lines += ["", "-" * 70, f"Ran {tests} tests in {elapsed:.3f}s", ""]
    failed = any(r["exit_code"] != 0 or not r["ok"] for r in results)
    shown = [f"{key}={value}" for key, value in totals.items() if value]
    lines.append(("FAILED" if failed else "OK") + (f" ({', '.join(shown)})" if shown else ""))
    return "\n".join(lines) + "\n"


def emit(result: dict) -> None:
    status = ("no result" if result["tests"] is None else "ok" if result["ok"] else "FAILED")
    print(result["body"])
    if result["tests"] is None:
        # listed like a test's error, so a report names the module that produced nothing
        print(f"ERROR: {result['module']} ({result['module']}.import)\n"
              f"{'-' * 70}\nno test result (exit code {result['exit_code']}); its output is above\n")
    print(f"== {result['module']}: {result['tests'] if result['tests'] is not None else '?'} tests, "
          f"{status}, {result['elapsed']:.1f}s\n", flush=True)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("modules", nargs="*", help="test modules (default: every scripts/tests/test_*.py)")
    parser.add_argument("--workers", type=int, default=workers_default(), help="modules run at a time")
    parser.add_argument("--durations", type=int, default=25, help="slowest tests to list (0: none)")
    args = parser.parse_args()
    if args.workers < 1:
        parser.error("--workers must be positive")
    names = args.modules or modules()
    missing = [m for m in names if not (TESTS / f"{m}.py").is_file()]
    if missing:
        parser.error(f"no such test module: {', '.join(missing)}")
    parallel = [m for m in names if m not in SERIAL]
    serial = [m for m in names if m in SERIAL]
    started = time.monotonic()
    results = []
    with ThreadPoolExecutor(max_workers=args.workers) as pool:
        for result in pool.map(lambda m: run_module(m, args.durations), parallel):
            results.append(result)
            emit(result)
    for module in serial:
        results.append(run_module(module, args.durations))
        emit(results[-1])
    print(footer(results, time.monotonic() - started, args.durations), end="", flush=True)
    return 0 if all(r["exit_code"] == 0 and r["ok"] for r in results) else 1


if __name__ == "__main__":
    sys.exit(main())
