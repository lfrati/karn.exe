"""Unified progress measurements; use make progress.

Every report runs the Rust workspace tests (the rules target writing rule receipts in that same
run), the Python tooling tests and the grammar gates, and prints their counts with the rule and
card measurements; --details adds legacy metrics, deck tables and diagnostics.
Admission is not correctness. Test references are not adequacy. Missing/stale execution
must never become passing evidence. Snapshots and raw logs live in .cache/progress-runs/.
"""
from __future__ import annotations

import argparse
import collections
import datetime as dt
import fcntl
import hashlib
import json
import math
import os
import re
import subprocess
import sys
import textwrap
import time
from pathlib import Path

from rust_tools import claim_target, fast_binary

import deck_admission
import rules
import rule_evidence
import progress_dashboard
import snapshot
import tooling_tests

ROOT = Path(__file__).resolve().parent.parent
# a snapshot's `.cache` is a link to the checkout's: the reports name the real place
CACHE = (ROOT / ".cache").resolve()
# the pinned format-legal pool measured beside the whole corpus (scripts/fetch_resources.py --legal-only)
LEGAL_POOL = ROOT / "resources/oracle/legal-standard.jsonl"
# the checkout a snapshot run measures (scripts/snapshot.py), set by the process that made it
SNAPSHOT_OF = "KARN_SNAPSHOT_OF"
SNAPSHOT_MANIFEST = "KARN_SNAPSHOT_MANIFEST"
GATES = ("selectors", "amounts", "triggers", "conditions", "costs", "effects", "cards")


def read_json(path: Path, default=None):
    return json.loads(path.read_text()) if path.exists() else default


def write_json(path: Path, data):
    path.parent.mkdir(parents=True, exist_ok=True)
    tmp = path.with_suffix(path.suffix + ".tmp")
    tmp.write_text(json.dumps(data, indent=2, ensure_ascii=False) + "\n")
    tmp.replace(path)


def fingerprint(paths) -> str:
    h = hashlib.sha256()
    for path in sorted(set(paths)):
        h.update(str(path).encode() + b"\0")
        if not path.is_file():
            h.update(b"MISSING\0")
            continue
        with path.open("rb") as stream:
            for block in iter(lambda: stream.read(1024 * 1024), b""):
                h.update(block)
        h.update(b"\0")
    return h.hexdigest()


def grammar_hash() -> str:
    """The grammar gate and its local dependencies; game-only changes cannot affect it."""
    paths = [ROOT / "Cargo.toml", ROOT / "Cargo.lock", ROOT / "scripts/gates.sh",
             ROOT / "resources/oracle/cards.jsonl"]
    for crate in ("grammar", "oracle", "ir"):
        paths.extend(p for p in (ROOT / "crates" / crate).rglob("*") if p.is_file())
    return fingerprint(paths)


def census_hash() -> str:
    """Inputs which can change the corpus verdicts; reporting/test edits need no rescan."""
    paths = [ROOT / 'Cargo.toml', ROOT / 'Cargo.lock', ROOT / 'resources/oracle/cards.jsonl']
    paths.extend(p for p in (ROOT / 'crates').rglob('*') if p.is_file()
                 and 'tests' not in p.relative_to(ROOT / 'crates').parts)
    return fingerprint(paths)




def input_hashes() -> dict:
    engine = rules.suite_hash()
    tooling = fingerprint([ROOT / "pyproject.toml", ROOT / "uv.lock",
                           *sorted((ROOT / "scripts").rglob("*.py"))])
    return {"engine": engine, "tooling": tooling,
            "grammar": grammar_hash(),
            "decks": fingerprint([*list((ROOT / "decks").rglob("*.dck")), *sorted((ROOT / "resources/decks").glob("*.json*")), LEGAL_POOL, ROOT / "resources/manifest.json"]),
            "assessments": fingerprint([ROOT / "docs/rule-assessments.json"]),
            "census": census_hash(),
            "corpus": fingerprint([ROOT / "resources/oracle/cards.jsonl"]),
            "rules": fingerprint([rules.RULES]),
            "engine_binary": hashlib.sha256((fast_binary("game-serve")).read_bytes()).hexdigest()
            if (fast_binary("game-serve")).exists() else None}


def announce(message):
    print(f'[progress] {message}', file=sys.stderr, flush=True)


# a finished test in cargo's or unittest's verbose output
FINISHED = re.compile(rb"^test .* \.\.\. (?:ok|FAILED|ignored)|^\w+ \([\w.]+\) \.\.\. (?:ok|FAIL|ERROR|skipped|expected failure|unexpected success)", re.M)


class Job:
    """One command with stdout and stderr merged into `log`, watched for finished tests."""

    def __init__(self, args: list[str], log: Path, env=None, label=None):
        self.args, self.log, self.env, self.label = args, log, env, label or Path(args[0]).name
        self.process = self.stream = None
        self.read = self.finished = 0
        self.code = None
        self.text = ''

    def start(self):
        self.stream = self.log.open('w')
        self.process = subprocess.Popen(self.args, cwd=ROOT, stdout=self.stream, stderr=subprocess.STDOUT, env=self.env)

    def poll(self) -> bool:
        """Count the tests the log reports finished so far; whether the command has ended."""
        with self.log.open('rb') as tail:
            tail.seek(self.read)
            chunk = tail.read()
        # count whole lines only; a partial last line is read again next time
        chunk = chunk[:chunk.rfind(b'\n') + 1]
        self.read += len(chunk)
        self.finished += len(FINISHED.findall(chunk))
        if self.code is None:
            self.code = self.process.poll()
        return self.code is not None

    def stop(self):
        if self.process is not None and self.process.poll() is None:  # interrupted: do not leave it running
            self.process.kill()
            self.process.wait()
        if self.stream is not None:
            self.stream.close()
        if self.code is not None:
            self.text = self.log.read_text()

    def status(self, elapsed: float) -> str:
        tests = f', {self.finished:,} tests' if self.finished else ''
        return f'{self.label}: done{tests}' if self.code is not None else f'{self.label} {elapsed:.0f}s{tests}'


def commands(jobs: list[Job], verbose=False) -> list[tuple[int, str]]:
    """Run the jobs at once, showing elapsed time and finished tests, until every one ends."""
    started = time.monotonic()
    interactive = sys.stderr.isatty()
    frame, reported = 0, started
    try:
        for job in jobs:
            if verbose:
                announce('Running: ' + ' '.join(job.args))
            job.start()
        while True:
            done = all([job.poll() for job in jobs])
            elapsed = time.monotonic() - started
            status = ' | '.join(job.status(elapsed) for job in jobs)
            if interactive:
                position = frame % 36
                position = position if position <= 18 else 36 - position
                bar = ' ' * position + '====' + ' ' * (18 - position)
                print(f'\r\033[2K[progress] [{bar}] {status}', file=sys.stderr, end='', flush=True)
                frame += 1
            elif time.monotonic() - reported >= 60:
                announce(f'still running after {elapsed:.0f}s: {status}')
                reported = time.monotonic()
            if done:
                break
            time.sleep(.15 if interactive else 10)
    finally:
        for job in jobs:
            job.stop()
        if interactive:
            print('\r\033[2K', file=sys.stderr, end='', flush=True)
    return [(job.code, job.text) for job in jobs]


def command(args: list[str], log: Path, verbose=False, env=None, label=None) -> tuple[int, str]:
    """Run one command with stdout and stderr merged into `log` (see `commands`)."""
    return commands([Job(args, log, env, label)], verbose)[0]


def reusable_census(previous, current, refresh=False):
    if refresh or not previous or not previous.get('measurement_unchanged'):
        return None
    if not current.get('census') or previous.get('inputs', {}).get('census') != current['census']:
        return None
    path = previous.get('routing', {}).get('snapshot')
    return Path(path) if path and Path(path).is_file() else None


def corpus_counts(output: str) -> dict:
    match = re.search(r"^SUMMARY (.*)$", output, re.M)
    if not match:
        raise ValueError("game-gate produced no SUMMARY; no admission measurement")
    values = dict(part.split("=", 1) for part in match[1].split())
    total, parsed, admitted = (int(values[k]) for k in ("faces", "read", "run"))
    if not 0 <= admitted <= parsed <= total or total == 0:
        raise ValueError("inconsistent corpus counts")
    gaps = [{"faces": int(n), "reason": why, "examples": examples}
            for n, why, examples in re.findall(r"^\s+(\d+)\s+(.+?)\s+e\.g\. (.*)$", output, re.M)]
    return {"faces": total, "parsed": parsed, "admitted": admitted,
            "parsed_not_admitted": parsed - admitted, "not_parsed": total - parsed,
            "first_unsupported_concepts": gaps}


def route_inventory(path: Path) -> dict:
    """Read core-produced cache identities; never infer an execution route from Oracle text."""
    rows = [json.loads(line) for line in path.read_text().splitlines() if line.strip()]
    inventory = {}
    for row in rows:
        key = (row["oracle_id"], row["face"])
        if key in inventory:
            raise ValueError(f"duplicate admission face: {key}")
        inventory[key] = row
    return inventory


def route_counts(inventory: dict) -> dict:
    admitted = [row for row in inventory.values() if row["stage"] == "admitted"]
    if any(not isinstance(row.get("compiled_abilities"), dict) for row in admitted):
        raise ValueError("admission snapshot lacks compiled-route evidence")
    return {"admitted_faces": len(admitted),
            "faces_with_compiled_program": sum(any(row["compiled_abilities"].values()) for row in admitted),
            "scope": "Successful cached nonmodal printed ability programs, including split definitions and halves. "
                     "Modal and expanded keyword execution is excluded. Static registrations and remaining "
                     "abilities are not classified; this is a partial program inventory, not correctness coverage."}


def route_changes(previous: dict, current: dict) -> dict:
    if previous.keys() != current.keys():
        return {"status": "inventory changed", "faces": []}
    if any("compiled_abilities" not in row for row in previous.values()):
        return {"status": "baseline lacks route evidence", "faces": []}
    changes = []
    for key, row in current.items():
        old = previous[key]
        if (old["stage"], old.get("compiled_abilities")) != (row["stage"], row.get("compiled_abilities")):
            changes.append({"oracle_id": key[0], "face": key[1], "name": row["name"],
                            "before": {k: old.get(k) for k in ("stage", "compiled_abilities")},
                            "after": {k: row.get(k) for k in ("stage", "compiled_abilities")}})
    return {"status": "compared", "faces": changes}


def grammar_counts(text: str) -> dict:
    rows = {}
    for gate in GATES:
        line = re.search(rf"^{gate}\s+(.+)$", text, re.M)
        if not line:
            continue
        count = re.search(r"(?:RoundTrip:|all rows round-trip:) (\d+) \(([\d.]+)%\)", line[1])
        if count:
            rows[gate] = {"count": int(count[1]), "percent": float(count[2]),
                          "passed": "GATE no mismatch, no ambiguity: ok" in line[1]}
    return rows


def render_grammar(check: dict) -> list[str]:
    rows = check.get("rows", {})
    lines = ["", "  Grammar round trips", ""]
    for gate in GATES:
        name = "total" if gate == "cards" else gate
        if gate == "cards":
            lines.append("  " + "-" * 40)
        row = rows.get(gate)
        if row is None:
            lines.append(f"  {name:12} UNKNOWN  check: missing")
            continue
        status = ("stale" if check["status"] == "STALE" else
                  "missing" if check["status"] == "MISSING" else
                  "ok" if row["passed"] else "FAILED")
        suffix = f" check: {status}" if gate != "cards" or status != "ok" else ""
        lines.append(f"  {name:12} {row['percent']:5.1f} % ( {row['count']:5} ){suffix}")
    lines.append("  Total = faces with text whose every row round-trips; not executable faces.")
    if check["status"] != "PASS":
        lines.append(f"  Grammar run: {check['status']}. Rerun make progress.")
    return lines


def last(pattern: str, text: str) -> re.Match | None:
    """The last line-anchored match of `pattern` in `text`, if any."""
    return next(reversed(list(re.finditer(pattern, text, re.M))), None)


def check_result(name: str, code: int, text: str) -> dict:
    if name == "grammar":
        passed = [g for g in GATES if re.search(rf"^{g}\s+.*GATE no mismatch, no ambiguity: ok", text, re.M)]
        counts = {"passed": len(passed), "expected": len(GATES)}
        ok = code == 0 and len(passed) == len(GATES)
        # each failing gate with what it printed, e.g. a missing input
        failures = [f"{g}: {line[1].strip()[:160]}" if (line := re.search(rf"^{g}\s+(.*)$", text, re.M)) and line[1].strip() else g
                    for g in GATES if g not in passed]
    else:
        # the last footer: a test's own output may print one of a nested run
        count = last(r"Ran (\d+) tests? in", text)
        summary = last(r"^(?:OK|FAILED)(?: \((.*)\))?$", text)
        parts = dict(part.split("=") for part in (summary[1] or "").split(", ") if "=" in part) if summary else {}
        # an unexpected success fails the run, so it is counted as failed
        failed = sum(int(parts.get(k, 0)) for k in ("failures", "errors", "unexpected successes"))
        skipped, expected = int(parts.get("skipped", 0)), int(parts.get("expected failures", 0))
        executed = int(count[1]) if count else None
        counts = {"executed": executed, "failed": failed if summary else None, "skipped": skipped if summary else None,
                  "expected_failures": expected if summary else None,
                  "passed": executed - failed - skipped - expected if count and summary else None}
        ok = code == 0 and count is not None and int(count[1]) > 0
        failures = [f"{test} ({where})" for test, where in re.findall(r"^(?:FAIL|ERROR): (\S+) \((\S+)\)", text, re.M)]
        failures += [f"{test} ({where}): unexpected success" for test, where in
                     re.findall(r"^(\S+) \((\S+)\)(?:\n.*)? \.\.\. unexpected success$", text, re.M)]
    return {"passed": ok, "exit_code": code, "counts": counts, "failures": failures,
            **({"rows": grammar_counts(text)} if name == "grammar" else {})}


def evidence_status(record: dict | None, current: dict, keys: tuple[str, ...]) -> str:
    if record is None:
        return "MISSING"
    if not record.get("unchanged", False) or any(record.get("inputs", {}).get(k) != current.get(k) for k in keys):
        return "STALE"
    return "PASS" if record["passed"] else "FAIL"


def deck_counts(data: dict) -> dict:
    groups = {"decks": {}, "starters": {}, "fixtures": {}}
    for path, parts in data["decks"].items():
        group = "starters" if path.startswith("decks/starter/") else "fixtures" if path == "decks/lands.dck" else "decks"
        groups[group][path] = parts
    cards = data["cards"]
    main = lambda p: p["main"]["accepted"] == p["main"]["total"]
    complete = lambda p: main(p) and p["sideboard"]["accepted"] == p["sideboard"]["total"]
    return {"groups": groups, "distinct_names": len(cards), "admitted_names": sum(c["accepted"] for c in cards.values()),
            "main_decks_admitted": sum(main(p) for p in groups["decks"].values()),
            "whole_decks_admitted": sum(complete(p) for p in groups["decks"].values()),
            "main_decks_total": len(groups["decks"]), "cards": cards}


def rule_counts(data: dict) -> dict:
    m = dict(data["metrics"])
    # Stale evidence is unknown, not a collapse in implementation progress to zero.
    m["verified"] = m["verified"] if data["execution_current"] else None
    execution = data.get("execution")
    passed_names = {t for t, outcomes in (execution or {}).get("tests", {}).items() if outcomes == ["ok"]}
    m["with_passing_named_test"] = (sum(any(t["test"] in passed_names for t in e["tests"])
                                        for e in data["rules"].values()) if data["execution_current"] else None)
    m["sections"] = {title: {"total": len(entries), "referenced": sum(bool(e["tests"]) for e in entries),
                              "reviewed": sum(e["status"] != "unreviewed" for e in entries),
                              "verified": sum(e["verified"] for e in entries) if data["execution_current"] else None}
                     for digit, title in rules.SECTIONS.items()
                     for entries in [[v for k, v in data["rules"].items() if k.startswith(digit)]]}
    m["source"] = data["source"]
    m["limitations"] = data["limits"]
    return m


def delta(previous: dict | None, current: dict) -> list[str]:
    if not previous or previous.get("schema") != current["schema"]:
        return ["No comparable previous snapshot."]
    out = []
    for key, section, fields in [
        ("corpus", "corpus", ("parsed", "admitted")),
        ("rules", "rules", ("reviewed", "referenced_by_tests", "verified", "attributed_rules", "attributed_passing")),
        ("decks", "decks", ("admitted_names", "main_decks_admitted", "whole_decks_admitted")),
    ]:
        if previous["inputs"][key] != current["inputs"][key]:
            out.append(f"{section}: inventory changed; numerical delta omitted")
            continue
        for field in fields:
            if field.startswith("attributed_") and previous[section].get("attributed_scope") != current[section].get("attributed_scope"):
                continue
            a, b = previous[section].get(field), current[section].get(field)
            if a is not None and b is not None:
                out.append(f"{section}.{field}: {b - a:+d}")
    return out


def fraction(n, total) -> str:
    return "UNKNOWN (missing/stale execution)" if n is None else f"{n:,}/{total:,} ({n / total:.2%})" if total else "n/a (empty inventory)"


def percentage(n, total) -> str:
    if n is None:
        return "UNKNOWN (missing/stale evidence)"
    if not total:
        return "n/a (empty inventory)"
    return f"{100 * n / total:6.2f} % ( {n:6,}/{total:,} )"


def deck_color(parts: dict) -> str:
    main, side = (parts[k]["accepted"] == parts[k]["total"] for k in ("main", "sideboard"))
    return "32" if main and side else "33" if main else "38;5;208" if side else "31"


def truncated(n, total) -> tuple[str, str]:
    """A percentage truncated to two decimals, so incomplete never shows as 100%, and its count."""
    if n is None or total is None:
        return 'UNKNOWN', ''
    return (f"{math.floor(10000 * n / total) / 100:.2f}%", f"({n:,}/{total:,})") if total else ('n/a', '')


def row(label: str, value: tuple[str, str], rest: str = '') -> str:
    """One aligned summary row: label, percentage, count, then anything else."""
    return f"  {label:28}{value[0]:>8}  {value[1]:>15}{rest}".rstrip()


def suite_rows(checks: dict) -> list[tuple[str, int | None, int | None, int | None, int | None, str]]:
    """(suite, passed, total, failed, skipped, status) of each suite this report ran."""
    rust, tooling, grammar = checks["workspace"], checks["tooling"], checks["grammar"]
    t = rust.get("totals") or {}
    rows = [("Rust workspace", t.get("passed"), sum(t.get(k, 0) for k in ("passed", "failed", "ignored")) if t else None,
             t.get("failed"), t.get("ignored"), rust["status"])]
    c = tooling.get("counts", {})
    # expected failures (known limits) are not passes; they join the skipped column, labelled
    skipped = None if c.get("skipped") is None else c["skipped"] + (c.get("expected_failures") or 0)
    rows.append(("Python tooling", c.get("passed"), c.get("executed"), c.get("failed"), skipped, tooling["status"]))
    g = grammar.get("counts", {})
    rows.append(("Grammar gates", g.get("passed"), g.get("expected"),
                 g["expected"] - g["passed"] if "passed" in g else None, 0 if "passed" in g else None, grammar["status"]))
    return rows


def summary(data: dict) -> str:
    """The default report: what this run measured, as counts and percentages."""
    checks, revision = data["checks"], data.get("revision") or {}
    count = lambda n: '?' if n is None else f'{n:,}'
    lines = [f"Magic engine progress at {revision.get('commit', 'unrecorded')[:12]}"
             f"{' (uncommitted changes)' if revision.get('worktree') else ''}"]
    if taken := data.get("measured"):  # `snapshot` is this report's own file (see generate)
        captured = dt.datetime.fromisoformat(taken['captured_at']).strftime('%Y-%m-%d %H:%M:%S UTC')
        lines.append(f"  a snapshot of {taken['root']} captured {captured}")
    lines += ["", row('Tests run now', ('passed', 'passed/total'), f"  {'failed':>7}  {'skipped':>7}")]
    for name, passed, total, failed, skipped, status in suite_rows(checks):
        flag = '' if status == 'PASS' else f"  {status}"
        if name == "Python tooling" and (expected := checks["tooling"].get("counts", {}).get("expected_failures")):
            flag += f"  (skipped includes {expected} expected failures)"
        lines.append(row(name, truncated(passed, total), f"  {count(failed):>7}  {count(skipped):>7}{flag}"))
    attributed = data["rules"]["attributed"]
    total, e = attributed["total"], attributed["execution"]
    lines += ["", f"  Rules ({total:,} numbered rules in the pinned rulebook)"]
    if e is None:
        lines.append(row('with a passing expectation', ('UNAVAILABLE', ''), f"  run {attributed['run_state']}"))
    else:
        lines.append(row('with a passing expectation', truncated(e['passing'], total)))
    lines.append(row('cited by expectations', truncated(attributed['rules_with_sites'], total),
                     f"  {attributed['sites']:,} expectations (single checks)"))
    if e is not None:
        lines.append(row('', ('', ''), f"  of those rules: failing {e['failing']:,}, "
                         f"diagnostic only {e['diagnostic']:,}, not run {e['not_run']:,}"))
    cards, decks = data['dashboard']['cards'], data['dashboard']['decks']['overall']
    deck_average = 'UNKNOWN' if decks['percent'] is None else f"{math.floor(decks['percent'] * 100) / 100:.2f}%"
    lines += ["", "  Cards and decks (engine admission, not game behavior)",
              row('whole cards parsed', truncated(cards['parsed'], cards['total'])),
              row('whole cards supported', truncated(cards['supported'], cards['total']))]
    if pool := data['dashboard'].get('legal'):
        refused = pool['refused']
        lines.append(row(f"{pool['format'].capitalize()}-legal supported", truncated(pool['supported'], pool['total']),
                         ('' if refused is None else f"  refused: {refused['parse']:,} not read, {refused['engine']:,} by the engine, "
                          f"{pool['distinct_blockers']:,} first blockers") + f"; scope {pool.get('effective_at') or 'undated'}"))
    lines += [row('archive deck average', (deck_average, f"({decks['decks']:,} lists)"),
                  f"  {count(decks['main_fully_admitted'])} main, {count(decks['fully_admitted'])} whole lists admitted")]
    failures = failure_lines(data)
    if failures:
        lines += ["", "  Failures"] + failures
    if not data["measurement_unchanged"]:
        lines += ["", "  Inputs changed during the run: these results describe no single checkout. Rerun."]
    lines += ["", "Passing tests and citations are evidence, not complete-rule correctness.",
              "Details: make progress PROGRESS_ARGS=--details   JSON: .cache/progress.json"]
    return "\n".join(lines)


def failure_lines(data: dict, limit=10) -> list[str]:
    """Where each suite failed or stopped, with its log."""
    checks, out = data["checks"], []
    def listed(prefix, items, log):
        shown = [f"    {prefix}: {item}" for item in items[:limit]]
        if len(items) > limit:
            shown.append(f"    {prefix}: … {len(items) - limit} more")
        return shown + ([f"    {prefix} log: {log}"] if log else [])
    rust = checks["workspace"]
    if rust["status"] != "PASS":
        items = rust.get("failures", []) + [f"{name}: no result line" for name in rust.get("unfinished", [])] + rust.get("errors", [])
        items = items or ["no failing test named: " + ("not every test binary ran to its result" if rust["status"] == "INCOMPLETE" else rust["status"].lower())]
        out += listed("Rust", items, rust.get("log"))
    for name, label in (("tooling", "Python"), ("grammar", "grammar gate")):
        check = checks[name]
        if check["status"] != "PASS":
            out += listed(label, check.get("failures") or [f"{check['status'].lower()}, exit code {check.get('exit_code')}"], check.get("log"))
    attributed = data["rules"]["attributed"]
    if attributed["run_state"] != "CURRENT":
        out.append(f"    rule receipts: {attributed['run_state']}; see .cache/rule-evidence-run.log")
    out += [f"    rule attribution: {problem}" for problem in attributed["problems"][:limit]]
    return out


def render(data: dict, *, color=False, details=False) -> str:
    if not details:
        return summary(data)
    text = progress_dashboard.render(data)
    cards = data['dashboard']['cards']
    text += "\n\nWhole-card engine implementation checks: " + fraction(cards['supported'], cards['total'])
    text += "\n\n" + "\n".join(render_details(data))
    return text


def render_details(data: dict) -> list[str]:
    c, checks = data["corpus"], data["checks"]
    lines = ["", "FACE INVENTORY — diagnostic, distinct from headline whole cards",
             f"  {c['faces']:,} faces; {c['parsed']:,} parsed, {c['admitted']:,} admitted; "
             f"{c['parsed_not_admitted']:,} parsed but not admitted.",
             "  First blockers are censored diagnostics; fixing one may reveal another."]
    for gap in c.get('first_unsupported_concepts', [])[:5]:
        lines.append(f"  {gap['faces']:5,}  {gap['reason']}; e.g. {gap['examples']}")
    if pool := data['dashboard'].get('legal'):
        lines += ["", f"{pool['format'].upper()}-LEGAL POOL — {pool['total']:,} cards pinned in {pool['source']}"]
        lines.append(f"  Scope: {pool.get('effective_at') or 'undated'}; pool SHA-256 {pool.get('pool_sha256', 'unrecorded')}")
        if pool['supported'] is None:
            lines.append("  UNKNOWN: inputs changed during the measurement")
        else:
            lines.append(f"  {pool['supported']:,} supported, {pool['parsed']:,} parsed; "
                         f"{pool['distinct_blockers']:,} distinct first blockers over the refused cards (censored diagnostics):")
            for gap in pool['first_blockers']:
                lines.append(f"  {gap['cards']:5,}  {gap['reason']}; e.g. {gap['examples']}")
    lines += ["", "VALIDATION — stale results are historical only"]
    for name in ("workspace", "tooling", "grammar"):
        check = checks[name]
        lines.append(f"  {name:12} {check['status']:7} {check.get('counts', {})}")
        if check.get("log"):
            lines.append(f"    log: {check['log']}")
    lines += ["", "CHANGE SINCE BASELINE " + (data.get("baseline") or "(none)")]
    lines += ["  " + item for item in data["delta"]]
    if routing := data.get("routing"):
        lines += [f"  Route inventory: {routing['snapshot']}", f"  Route comparison: {routing['changes']['status']}"]
        for face in routing["changes"]["faces"]:
            def label(state):
                groups = state["compiled_abilities"] or {}
                programs = ", ".join(f"{name}:{indices}" for name, indices in groups.items() if indices)
                return f"{state['stage']} ({programs or 'no cached program'})"
            lines.append(f"    {face['name']}: {label(face['before'])} -> {label(face['after'])}")
    lines += ["", f"Snapshot: {data['snapshot']}", "Rerun: make progress"]
    return lines


# what the suites run: the workspace test binaries (their own profile), then the tools the
# Python tests and the gates run (built one at a time, each using every core)
TEST_BUILDS = [["cargo", "test", "--workspace", "--no-run"],
               ["cargo", "build", "--profile", "fast", "-p", "game", "--bin", "game-serve", "-p", "grammar", "--bin", "grammar-gate"]]


def run_suites(directory: Path, verbose: bool, record, workers: int | None = None) -> tuple[dict, int]:
    """Build once, then run at the same time the Rust workspace (the rules target writing
    receipts in that same run), the Python tooling tests and the grammar gates; `record(name,
    result, keys)` keeps the last two. Returns the workspace report and 1 when a suite failed,
    stopped or saw its inputs change."""
    exit_status = 0
    # Every report runs the suites: its test counts are this checkout's, never a cached run's.
    announce('Building the test binaries and tools...')
    for args in TEST_BUILDS:
        # a build error is reported by the suite that needs it, from its own cargo output
        command(args, directory / "build-tests.log", verbose, label='test build')
    announce('Running the Rust workspace tests (with rule receipts from the rules target), '
             'the Python tooling tests and the grammar gates...')
    # One workspace run: the rules target's section of its log is that target's receipts run.
    run_id, receipts, env = rule_evidence.receipts_env()
    workspace_log = directory / "workspace.log"
    suites = [("tooling", "Python tooling tests",
               [sys.executable, "scripts/tooling_tests.py", "--durations", "25",
                *(["--workers", str(workers)] if workers else [])], ("tooling", "engine")),
              ("grammar", "grammar gates", ["bash", "scripts/gates.sh"], ("grammar",))]
    jobs = [Job(rules.WORKSPACE_TESTS, workspace_log, env, 'Rust workspace')]
    # the gates regenerate missing corpus dumps with this interpreter
    tools_env = {**os.environ, "PYTHON": sys.executable}
    jobs += [Job(cmd, directory / f"{name}.log", tools_env, label) for name, label, cmd, _ in suites]
    before = rules.suite_hash()  # what the run started from, compared to the tree after it
    results = commands(jobs, verbose)
    run = rules.run_tests(CACHE / "conformance-tests.json", run=lambda cmd: results[0], before=before)
    write_json(directory / "workspace.json", run)
    exit_status |= int(run["exit_code"] != 0 or not run["complete"] or not run["unchanged"])
    targets = [text for name, source, text in rules.sections(workspace_log.read_text()) if name == "rules" and source == "tests/rules.rs"]
    if len(targets) > 1:
        raise RuntimeError(f'more than one rules target in {workspace_log}')
    section = targets[0] if targets else None
    # the rules binary's own result decides its run, not another binary's failure
    rules_code = 0 if section and re.search(r"^test result: ok\.", section, re.M) else 1
    attributed_run = rule_evidence.record(run_id, receipts, section, rules_code, run["suite_sha256"],
                                          rules.suite_hash(), run["revision"], " ".join(rules.WORKSPACE_TESTS))
    exit_status |= int(attributed_run["exit_code"] != 0 or not attributed_run["complete"] or not attributed_run["unchanged"])
    for (name, _, _, keys), (code, text) in zip(suites, results[1:]):
        record(name, {**check_result(name, code, text), "log": str(directory / f"{name}.log")}, keys)
    return run, exit_status


def snapshot_identity() -> dict | None:
    """The checkout this run measures a snapshot of, from the manifest of its capture."""
    if not os.environ.get(SNAPSHOT_OF):
        return None
    manifest = read_json(Path(os.environ[SNAPSHOT_MANIFEST]))
    return {key: manifest[key] for key in ("root", "snapshot", "revision", "status", "captured_at", "digest")}


def generate(args) -> tuple[dict, int]:
    now = dt.datetime.now(dt.timezone.utc)
    directory = CACHE / "progress-runs" / now.strftime("%Y%m%dT%H%M%S%fZ")
    directory.mkdir(parents=True)
    claim_target()
    announce('Checking source changes and cached measurements...')
    measure_inputs = input_hashes
    before = measure_inputs()
    previous_path = args.baseline or CACHE / "progress.json"
    previous = read_json(previous_path)
    checks_path = CACHE / "progress-checks.json"
    checks = {name: check for name, check in read_json(checks_path, {}).items()
              if name in {"workspace", "tooling", "grammar"}}
    exit_status = 0
    census_cache = read_json(CACHE / 'card-census.json', previous)
    admission_path = reusable_census(census_cache, before, args.refresh)
    if admission_path:
        announce('Using unchanged engine/card measurement (no build or corpus rescan).')
        corpus = census_cache['corpus']
    else:
        announce('Building the engine...')
        code, _ = command(['cargo', 'build', '--profile', 'fast', '-p', 'game', '--bin', 'game-gate', '--bin', 'game-serve'],
                          directory / 'build.log', args.verbose)
        if code:
            raise RuntimeError(f'build failed; see {directory / "build.log"}')
        admission_path = directory / 'admission.jsonl'
        announce('Parsing and checking the complete card corpus...')
        code, output = command([str(fast_binary('game-gate')), '--limit', '3', '--admission-snapshot', str(admission_path)],
                               directory / 'corpus.log', args.verbose)
        if code:
            raise RuntimeError(f'corpus measurement failed; see {directory / "corpus.log"}')
        corpus = corpus_counts(output)
    announce('Calculating whole-card and combined-deck coverage...')
    inventory = route_inventory(admission_path)
    routing = {**route_counts(inventory), "snapshot": str(admission_path),
               "changes": {"status": "no baseline route inventory", "faces": []}}
    if len(inventory) != corpus["faces"] or routing["admitted_faces"] != corpus["admitted"]:
        raise ValueError("route inventory disagrees with corpus admission counts")
    baseline_routes = previous.get("routing", {}).get("snapshot") if previous else None
    if baseline_routes and Path(baseline_routes).is_file():
        routing["changes"] = route_changes(route_inventory(Path(baseline_routes)), inventory)
    # Reuse this measurement's whole-card verdicts. Repeated engine setup probes were
    # expensive and gave the old report a different unit for deck versus corpus counts.
    physical_cards = progress_dashboard.whole_cards(inventory, ROOT / 'resources/oracle/cards.jsonl')
    catalog_names = {row['name']: row['supported'] for row in physical_cards.values()}
    aliases = progress_dashboard.deck_archive.Names()
    paths = {str(p.relative_to(ROOT)): deck_admission.deck_lists(p) for p in sorted((ROOT / 'decks').rglob('*.dck'))}
    accepted = {}
    for name in {n for parts in paths.values() for names in parts.values() for n in names}:
        canonical, _, _ = aliases.normalize({'main': {name: 1}})
        canonical_name = next(iter(canonical['main']))
        accepted[name] = {'accepted': catalog_names.get(canonical_name, False)}
    deck = {'cards': accepted, 'decks': {
        path: {part: {'total': len(names), 'accepted': sum(accepted[n]['accepted'] for n in names)}
               for part, names in parts.items()} for path, parts in paths.items()},
        'meaning': 'Whole-card compiler admission from the current corpus snapshot; not behavior verification.'}
    write_json(directory / 'decks.json', deck)

    def record(name, result, keys):
        nonlocal exit_status
        after = measure_inputs()
        checks[name] = {**result, "created_at": now.isoformat(), "inputs": before,
                        "unchanged": all(before[k] == after[k] for k in keys)}
        write_json(checks_path, checks)
        if not result["passed"] or not checks[name]["unchanged"]:
            exit_status = 1

    run, failed = run_suites(directory, args.verbose, record, args.workers)
    exit_status |= failed
    announce('Reading behavioral rule tests and their latest recorded results...')
    ledger = rules.ledger(ROOT / "docs/rule-assessments.json", CACHE / "conformance-tests.json")
    write_json(CACHE / "rules-ledger.json", ledger)
    current = measure_inputs()
    if before['census'] == current['census']:
        write_json(CACHE / 'card-census.json', {
            'inputs': {'census': current['census']}, 'measurement_unchanged': True,
            'corpus': corpus, 'routing': {'snapshot': str(admission_path)}})
    evidence = {}
    for name, keys in [("tooling", ("tooling", "engine")), ("grammar", ("grammar",))]:
        evidence[name] = {**checks.get(name, {}), "status": evidence_status(checks.get(name), current, keys)}
    # Older check manifests retain raw logs but predate structured grammar rows.
    grammar = evidence["grammar"]
    if "rows" not in grammar:
        log = Path(grammar["log"]) if grammar.get("log") else None
        grammar["rows"] = grammar_counts(log.read_text()) if log and log.is_file() else {}
    known = rules.inventory()["rules"]
    attributed = rule_evidence.measure(rule_evidence.discover(known=set(known)), rule_evidence.load_run(),
                                       current["engine"], len(known))
    if attributed["problems"]:
        exit_status = 1
    evidence["workspace"] = {"status": "STALE" if not ledger["execution_current"] else
                                       "INCOMPLETE" if not run["complete"] else
                                       "PASS" if run["exit_code"] == 0 and run["totals"]["passed"] > 0 else "FAIL",
                             "counts": run["counts"], "totals": run["totals"], "failures": run["failures"],
                             "errors": run["errors"], "unfinished": run.get("unfinished", []),
                             "log": str(directory / "workspace.log")}
    data = {"schema": 1, "created_at": now.isoformat(), "measured": snapshot_identity(),
            "meanings": {"admitted": "All text is accepted by the engine; behavior may still contain bugs.",
                         "referenced_by_tests": "Legacy: numbered rules named by tests (name or // Rules:), not thorough coverage.",
                         "attributed_rules": "Rules of attributed expectations in source; not run evidence.",
                         "attributed_passing": "Rules with an attributed expectation that passed in a current complete run; null when unavailable.",
                         "verified": "Manual complete assessment with current passing explicit evidence and verified subrules; not exhaustive proof.",
                         "implemented_game_percentage": None, "thoroughly_tested_game_percentage": None,
                         "unknown_reason": "No exhaustive or complexity-weighted coverage measurement exists."}, "revision": rules.revision(), "inputs": current,
            "measurement_unchanged": all(before[k] == current[k] for k in ("engine", "decks", "tooling", "corpus", "rules", "assessments", "census")),
            "corpus": corpus, "routing": routing, "decks": deck_counts(deck), "rules": {**rule_counts(ledger), "attributed": attributed,
                      "attributed_scope": attributed["scope"], "attributed_rules": attributed["rules_with_sites"],
                      "attributed_passing": (attributed["execution"] or {}).get("passing")}, "checks": evidence,
            "snapshot": str(directory / "report.json"), "baseline": str(args.baseline or previous.get("snapshot", previous_path)) if previous else None}
    data['decks'].update(population=str(ROOT / 'decks'),
                         meaning='Local .dck lists, excluding starter and lands fixtures. Main and whole-list admission counts; separate from selected archive averages.')
    data['dashboard'] = progress_dashboard.dashboard(
        inventory, ROOT / 'resources/oracle/cards.jsonl', ROOT / 'resources/decks/selected-decks.jsonl',
        ledger, data['measurement_unchanged'], legal_path=LEGAL_POOL)
    data['dashboard']['rules']['attributed'] = attributed
    data["delta"] = delta(previous, data)
    if not data["measurement_unchanged"]:
        exit_status = 1
    announce('Saving the report...')
    write_json(directory / "report.json", data)
    (directory / "report.txt").write_text(render(data) + "\n")
    (directory / "details.txt").write_text(render(data, details=True) + "\n")
    write_json(CACHE / "progress.json", data)
    (CACHE / "progress.txt").write_text(render(data) + "\n")
    return data, exit_status


def run_in_snapshot(argv: list[str]) -> int:
    """Capture this checkout (scripts/snapshot.py) and run the report there, with the
    snapshot's own target directory; then say whether the checkout still matches it."""
    base = CACHE / "progress-snapshot"
    tree, manifest_path = base / "tree", base / "manifest.json"
    announce(f'Capturing the checkout in {tree}...')
    try:
        manifest = snapshot.capture(ROOT, tree, manifest_path, cache=CACHE)
    except snapshot.CaptureConflict as error:
        print(f"Progress report not run: {error}", file=sys.stderr)
        return 2
    env = {**os.environ, SNAPSHOT_OF: str(ROOT), SNAPSHOT_MANIFEST: str(manifest_path),
           "CARGO_TARGET_DIR": str(tree / "target")}
    code = subprocess.call([sys.executable, str(tree / "scripts" / "progress.py"), *argv], cwd=tree, env=env)
    if not snapshot.live_matches(manifest):
        print(f"\n{ROOT} changed after the snapshot was captured: this report describes the snapshot "
              f"({manifest['digest'][:12]}), not the checkout as it is now.")
    return code


def main() -> int:
    global CACHE
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--refresh", action="store_true", help="force a rebuild and corpus rescan; unchanged inputs reuse their measurement by default")
    # every report runs the checks; --check (make progress-check) remains accepted, running nothing more
    parser.add_argument("--check", action="store_true", help=argparse.SUPPRESS)
    parser.add_argument("--baseline", type=Path, help="compare with this saved report instead of the previous report")
    parser.add_argument("--json", action="store_true", help="print JSON instead of the human-readable report")
    parser.add_argument("--details", action="store_true", help="the full dashboard: legacy rule metrics, deck tables, validationlogs and snapshot deltas")
    parser.add_argument("--verbose", action="store_true", help="print build and measurement progress to stderr")
    parser.add_argument("--live", action="store_true", help="run in this checkout as it is, rather than in a snapshot of it (edits during the run then change what is measured)")
    parser.add_argument("--workers", type=int, help=f"Python test modules run at a time (default {tooling_tests.workers_default()} here)")
    parser.add_argument("--color", choices=("auto", "always", "never"), default="auto", help="deck colors; auto respects NO_COLOR and redirected output")
    parser.add_argument("--cache", type=Path, help="keep the report, its runs, the lock and the snapshot in this directory rather than this checkout's .cache (the snapshot's .cache links there)")
    args = parser.parse_args()
    if args.baseline and not args.baseline.is_file():
        parser.error("baseline file does not exist")
    if args.workers is not None and args.workers < 1:
        parser.error("--workers must be positive")
    if args.cache:
        if args.live and not os.environ.get(SNAPSHOT_OF):
            parser.error("--cache needs a snapshot run, not --live: the suites write the checkout's .cache")
        CACHE = args.cache.resolve()
    CACHE.mkdir(parents=True, exist_ok=True)
    with (CACHE / "progress.lock").open("w") as lock:
        # the process that captured the snapshot holds the lock while the run inside it lasts
        if not os.environ.get(SNAPSHOT_OF):
            try:
                fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
            except BlockingIOError:
                print("Another progress report is running; wait for it to finish.", file=sys.stderr)
                return 2
            if not args.live:
                return run_in_snapshot(sys.argv[1:])
        try:
            data, code = generate(args)
        except (OSError, ValueError, KeyError, RuntimeError) as error:
            print(f"Progress report failed: {error}", file=sys.stderr)
            return 2
        print(json.dumps(data, indent=2) if args.json else render(data, details=args.details,
              color=args.color == "always" or (args.color == "auto" and sys.stdout.isatty() and "NO_COLOR" not in os.environ)))
        return code


if __name__ == "__main__":
    sys.exit(main())
