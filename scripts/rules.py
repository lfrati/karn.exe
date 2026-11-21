"""Discover test evidence for numbered Comprehensive Rules passages and interactions.

    uv run scripts/rules.py              per section: rules with discoverable test references
    uv run scripts/rules.py 603.4        rule text, source locations and commands for its tests
    uv run scripts/rules.py --untested   source-cited rules with no discovered tests
    uv run scripts/rules.py --run-tests --ledger .cache/rules-ledger.json --report
    uv run scripts/rules.py --evidence [--run | --check]   expectation-level evidence

Rule-named functions and adjacent `// Rules: 603.4, 608.2a` test comments are legacy discovery
metadata. Expectations attributed with `cite!` or `expect().rules(…)` are reported separately
(scripts/rule_evidence.py). Neither a citation nor a passing test proves a rule is implemented.
"""

import re
import shlex
import tomllib
import sys
import argparse
import hashlib
import json
import subprocess
from collections import Counter
from pathlib import Path

import pinned

ROOT = Path(__file__).resolve().parent.parent
RULES = pinned.RULES
SECTIONS = {
    "1": "Game Concepts",
    "2": "Parts of a Card",
    "3": "Card Types",
    "4": "Zones",
    "5": "Turn Structure",
    "6": "Spells, Abilities, and Effects",
    "7": "Additional Rules",
    "8": "Multiplayer Rules",
    "9": "Casual Variants",
}


def inventory(path: Path = RULES) -> dict:
    """Extract the rules body, preserving continuations and examples, with source lines.

    Contents and glossary are deliberately outside the numbered-rule denominator. References
    remain text; this does not claim to resolve them or structure glossary definitions.
    """
    raw = path.read_bytes()
    lines = raw.decode("utf-8-sig").splitlines()
    starts = [i for i, line in enumerate(lines) if line.strip() == "1. Game Concepts"]
    if len(starts) != 2:
        raise ValueError("expected contents and body headings for 1. Game Concepts")
    start = starts[1]
    end = next(i for i in range(start + 1, len(lines)) if lines[i].strip() == "Glossary")
    version = next(line.strip() for line in lines if line.startswith("These rules are effective"))
    entries, titles = {}, {}
    current = None
    for i in range(start, end):
        line = lines[i].strip()
        if not line:
            continue
        if m := re.match(r"^([1-9]\d{2}\.\d+[a-z]{0,2})\.?\s+(.*)$", line):
            rid, text = m.groups()
            if rid in entries:
                raise ValueError(f"duplicate rule {rid} at line {i + 1}")
            current = {"section": rid[:3], "text": text, "examples": [], "line": i + 1, "end_line": i + 1}
            entries[rid] = current
        elif m := re.match(r"^([1-9]\d{2})\.\s+(.+)$", line):
            titles[m[1]] = m[2]
            current = None
        elif re.match(r"^[1-9]\. ", line):
            current = None
        elif current is not None:
            current["end_line"] = i + 1
            if line.startswith("Example:"):
                current["examples"].append(line.removeprefix("Example:").strip())
            elif current["examples"]:
                current["examples"][-1] += "\n" + line
            else:
                current["text"] += "\n" + line
        else:
            raise ValueError(f"unassigned rulebook text at line {i + 1}: {line}")
    return {"source": {"path": str(path), "version": version, "sha256": hashlib.sha256(raw).hexdigest()},
            "limits": ["Cross-references preserved as text; not resolved", "Glossary not structured"],
            "titles": titles, "rules": entries}


def rules() -> dict[str, str]:
    return {rid: entry["text"] for rid, entry in inventory()["rules"].items()}


def test_references(path: Path) -> list[dict]:
    """Read the primary name and explicit adjacent interaction citations, without running Rust.

    This is source discovery, not a Rust parser or proof that cfg-gated tests execute. The
    existing execution evidence remains responsible for actual test outcomes.
    """
    found, interactions, test_attribute = [], set(), False
    for line_no, line in enumerate(path.read_text().splitlines(), 1):
        stripped = line.strip()
        if stripped.startswith("//"):
            if stripped.startswith("//!"):
                interactions, test_attribute = set(), False
                continue
            if match := re.match(r"//[/!]?\s*Rules:\s*(.*)", stripped):
                interactions.update(re.findall(r"\b([1-9]\d{2}\.\d+[a-z]{0,2})\b", match[1]))
            continue
        if not stripped:
            continue
        if stripped.startswith("#["):
            test_attribute |= bool(re.match(r"#\[test\]", stripped))
            # Strip complete attributes without treating a bracket inside a quoted message
            # as their end (e.g. should_panic(expected = "index [0]")).
            while stripped.startswith("#["):
                depth, quoted, escaped, end = 0, False, False, None
                for index, char in enumerate(stripped[1:], 1):
                    if quoted:
                        if escaped:
                            escaped = False
                        elif char == "\\":
                            escaped = True
                        elif char == '"':
                            quoted = False
                    elif char == '"':
                        quoted = True
                    elif char == "[":
                        depth += 1
                    elif char == "]":
                        depth -= 1
                        if depth == 0:
                            end = index + 1
                            break
                if end is None:
                    break
                stripped = stripped[end:].lstrip()
            if not stripped:
                continue
        if match := re.match(r"(?:pub(?:\([^)]*\))?\s+)?fn\s+(\w+)\s*\(", stripped):
            name = match[1]
            primary = re.match(r"r(\d{3})_(\d+[a-z]{0,2})_", name)
            refs = set(interactions) if test_attribute else set()
            if primary and test_attribute:
                refs.add(f"{primary[1]}.{primary[2]}")
            if refs:
                found.append({"line": line_no, "test": name, "rules": sorted(refs)})
        interactions, test_attribute = set(), False
    return found


def evidence() -> tuple[dict, dict]:
    """Discover primary and interaction test references and source citations; neither is credit."""
    named, citations = {}, {}
    for path in sorted((ROOT / "crates").rglob("*.rs")):
        rel = str(path.relative_to(ROOT))
        for reference in test_references(path):
            for rid in reference["rules"]:
                named.setdefault(rid, []).append({"path": rel, "line": reference["line"], "test": reference["test"]})
        for line_no, line in enumerate(path.read_text().splitlines(), 1):
            for rid in sorted(set(re.findall(r"\b([1-9]\d{2}\.\d+[a-z]{0,2})\b", line))):
                citations.setdefault(rid, []).append({"path": rel, "line": line_no})
    return named, citations


def suite_hash() -> str:
    """Invalidate evidence when workspace inputs, the specification, or corpus changes."""
    digest = hashlib.sha256()
    paths = [ROOT / "Cargo.toml", ROOT / "Cargo.lock"]
    paths += [p for p in (RULES, ROOT / "resources/oracle/cards.jsonl") if p.exists()]
    paths += sorted(p for p in (ROOT / "crates").rglob("*") if p.is_file())
    for path in paths:
        digest.update(str(path.relative_to(ROOT)).encode() + b"\0" + path.read_bytes() + b"\0")
    return digest.hexdigest()


def revision() -> dict:
    def git(*args):
        return subprocess.check_output(["git", *args], cwd=ROOT, text=True).strip()
    return {"commit": git("rev-parse", "HEAD"),
            "worktree": git("status", "--porcelain", "--ignore-submodules")}


# every binary runs even after one fails, so a failure never hides the counts of the rest
WORKSPACE_TESTS = ["cargo", "test", "--workspace", "--no-fail-fast", "--", "--color", "never"]
# cargo's header before each test binary's output (on stderr; a run read for its sections
# merges stderr into stdout so headers and results keep their order)
TARGET = re.compile(r"^\s+(?:Running (?:unittests )?(\S+) \((\S+)\)|Doc-tests (\S+))$", re.M)
RESULT = re.compile(r"^test result: \w+\. (\d+) passed; (\d+) failed; (\d+) ignored;", re.M)


def sections(log: str) -> list[tuple[str, str, str]]:
    """(name, source, text) of each test binary in a cargo test log: `rules`, `game`, `doc-tests game`."""
    heads = list(TARGET.finditer(log))
    out = []
    for i, head in enumerate(heads):
        end = heads[i + 1].start() if i + 1 < len(heads) else len(log)
        if head[3]:
            out.append((f"doc-tests {head[3]}", "", log[head.end():end]))
        else:
            out.append((Path(head[2]).name.rsplit("-", 1)[0], head[1], log[head.end():end]))
    return out


def test_report(log: str, exit_code: int, before: str, after: str) -> dict:
    """Results of a `cargo test --workspace` log: per short test name (the ledger's join) and,
    from each binary's own result line, the run's totals and failing tests by binary."""
    outcomes = {}
    for name, status in re.findall(r"^test (\S+) \.\.\. (ok|FAILED|ignored)\b", log, re.M):
        outcomes.setdefault(name.split("::")[-1], []).append(status)
    totals = {"passed": 0, "failed": 0, "ignored": 0, "binaries": 0}
    failures, unfinished = [], []
    for name, _, text in sections(log):
        if not RESULT.search(text):
            unfinished.append(name)
        for row in RESULT.findall(text):
            totals["binaries"] += 1
            for key, n in zip(("passed", "failed", "ignored"), row):
                totals[key] += int(n)
        failures += [f"{name} {test}" for test in re.findall(r"^test (\S+)(?: - should panic)? \.\.\. FAILED\b", text, re.M)]
    # build errors with their location; not cargo's own summary of failing binaries
    errors = [f"{e} at {at}" if at else e for e, at in re.findall(r"^(error(?:\[\w+\])?: .*)$(?:\n\s*--> (\S+))?", log, re.M)
              if not re.match(r"error: (?:test failed|\d+ targets? failed)", e)]
    return {"revision": revision(), "suite_sha256": before, "unchanged": before == after,
            "exit_code": exit_code, "counts": dict(Counter(s for v in outcomes.values() for s in v)),
            "totals": totals, "failures": failures, "errors": errors[:5], "unfinished": unfinished,
            # complete: cargo built and ran every binary to its result line
            "complete": totals["binaries"] > 0 and totals["binaries"] == len(sections(log)),
            "tests": outcomes, "command": " ".join(WORKSPACE_TESTS)}


def run_tests(path: Path, run=None, before: str | None = None) -> dict:
    """Run the workspace tests and record them at `path` (log beside it). `run(args)` returns
    (exit code, stdout and stderr merged); by default a plain subprocess. `before` is the
    suite hash taken before a run that already happened."""
    if before is None:
        before = suite_hash()
    if run is None:
        def run(args):
            result = subprocess.run(args, cwd=ROOT, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True)
            return result.returncode, result.stdout
    code, log = run(WORKSPACE_TESTS)
    report = test_report(log, code, before, suite_hash())
    path.parent.mkdir(parents=True, exist_ok=True)
    path.with_suffix(".log").write_text(log)
    path.write_text(json.dumps(report, indent=2) + "\n")
    return report


STATUSES = {"unreviewed", "missing", "partial", "implemented", "defective", "deferred", "blocked"}


def ledger(overrides: Path, results: Path | None = None) -> dict:
    data = inventory()
    named, citations = evidence()
    manual = json.loads(overrides.read_text()) if overrides.exists() else {"rules": {}}
    if manual.get("rules") and manual.get("rules_sha256") != data["source"]["sha256"]:
        raise ValueError("assessment rulebook hash differs; review overrides against the new specification")
    unknown = set(manual.get("rules", {})) - data["rules"].keys()
    if unknown:
        raise ValueError(f"unknown assessed rules: {sorted(unknown)}")
    run = json.loads(results.read_text()) if results and results.exists() else None
    fresh = bool(run and run["unchanged"] and run["suite_sha256"] == suite_hash())
    for rid, entry in data["rules"].items():
        assessment = manual.get("rules", {}).get(rid, {})
        status = assessment.get("status", "unreviewed")
        if status not in STATUSES:
            raise ValueError(f"{rid}: unknown status {status}")
        if status != "unreviewed" and not all(assessment.get(k) for k in ("rationale", "revision")):
            raise ValueError(f"{rid}: reviewed status needs rationale and assessment revision")
        entry.update(status=status, assessment=assessment, tests=named.get(rid, []), citations=citations.get(rid, []))
        required = assessment.get("evidence_tests", [])
        # Requiring explicit evidence prevents a source citation or test-name match from
        # automatically establishing implementation. The adequacy assessment stays manual.
        entry["verified"] = bool(status == "implemented" and required and fresh and run["exit_code"] == 0
                                 and all(run["tests"].get(t) == ["ok"] for t in required))
    # A numbered parent receives no credit while a dependent subrule is incomplete.
    for rid, entry in data["rules"].items():
        if rid[-1].isdigit():
            children = [v for k, v in data["rules"].items() if re.fullmatch(re.escape(rid) + "[a-z]{1,2}", k)]
            entry["verified"] &= all(c["verified"] for c in children)
    entries = list(data["rules"].values())
    data.update(revision=revision(), execution=run, execution_current=fresh,
                unknown_test_rules=sorted(set(named) - data["rules"].keys()),
                metrics={"total": len(entries), "reviewed": sum(e["status"] != "unreviewed" for e in entries),
                         "referenced_by_tests": sum(bool(e["tests"]) for e in entries),
                         "verified": sum(e["verified"] for e in entries),
                         "statuses": dict(Counter(e["status"] for e in entries))})
    return data


def report(data: dict) -> str:
    m = data["metrics"]
    rows = [f"Rules: {data['source']['version']}", f"SHA256 {data['source']['sha256']}",
            f"Reviewed {m['reviewed']}/{m['total']} ({m['reviewed'] / m['total']:.2%}); "
            f"referenced by tests {m['referenced_by_tests']}/{m['total']}; "
            f"verified {m['verified']}/{m['total']} ({m['verified'] / m['total']:.2%})",
            "Test references are evidence discovery, not implementation credit.",
            f"Execution evidence current: {data['execution_current']}"]
    for digit, title in SECTIONS.items():
        entries = [v for k, v in data["rules"].items() if k.startswith(digit)]
        rows.append(f"{digit} {title}: {len(entries)} rules, "
                    f"{sum(e['status'] != 'unreviewed' for e in entries)} reviewed, "
                    f"{sum(e['verified'] for e in entries)} verified")
    rows.append("Statuses: " + json.dumps(m["statuses"], sort_keys=True))
    rows.extend("Limit: " + limit for limit in data["limits"])
    if data["unknown_test_rules"]:
        rows.append("Unknown test rule references: " + ", ".join(data["unknown_test_rules"]))
    return "\n".join(rows)


def tests() -> dict[str, list[dict]]:
    """The same source-discovered test references used by the coverage ledger."""
    return evidence()[0]


def test_command(reference: dict) -> str:
    """Select the workspace package and Cargo target for a discovered source test.

    Numbered folders under the game's sole integration target are modules, not targets.
    Unknown/shared layouts use all targets rather than printing a command that misses tests.
    """
    path = ROOT / reference["path"]
    crate = next(parent for parent in path.parents if (parent / "Cargo.toml").is_file())
    manifest = tomllib.loads((crate / "Cargo.toml").read_text())
    relative = path.relative_to(crate)
    target = ["--all-targets"]
    if relative.parts[:2] == ("src", "bin"):
        name = Path(relative.parts[2]).stem
        target = ["--bin", name]
    elif relative.parts[0] == "src":
        target = ["--lib"]
    elif relative.parts[0] == "tests":
        declared = manifest.get("test", [])
        if manifest["package"].get("autotests", True) is False and len(declared) == 1:
            target = ["--test", declared[0]["name"]]
        elif len(relative.parts) == 2:
            target = ["--test", relative.stem]
    return shlex.join(["cargo", "test", "-p", manifest["package"]["name"],
                       *target, reference["test"]])


def cited() -> dict[str, list[str]]:
    """The rules the engine's source cites, and the files that cite each: the rules it claims
    to implement."""
    out: dict[str, list[str]] = {}
    for path in sorted((ROOT / "crates/game/src").glob("*.rs")):
        for rule in set(re.findall(r"\b([1-9]\d{2}\.\d+[a-z]{0,2})\b", path.read_text())):
            out.setdefault(rule, []).append(path.name)
    return out


def evidence_main(args) -> int:
    import rule_evidence
    known = inventory()["rules"]
    found = rule_evidence.discover(known=set(known))
    if args.check:
        for problem in found["problems"]:
            print("PROBLEM " + problem)
        print(f"{len(found['sites'])} sites; {len(found['problems'])} problems")
        return 1 if found["problems"] else 0
    data = rule_evidence.run(suite_hash, revision) if args.run else rule_evidence.load_run()
    joined = rule_evidence.join(found, data, suite_hash())
    print(rule_evidence.summary(found, joined, known))
    for rid in sorted(joined["status"], key=lambda r: [int(r.split(".")[0]), r.split(".")[1]]):
        print(f"{rid:8} {joined['status'][rid]}")
        print("\n".join(rule_evidence.drill(rid, joined)))
    if found["problems"] or joined["problems"] or (args.run and (data["exit_code"] or not data["complete"])):
        return 1
    return 0


def main(argv=None) -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("prefix", nargs="?", help="rule identifier prefix to inspect")
    ap.add_argument("--evidence", action="store_true", help="inspect expectation-level rule evidence")
    ap.add_argument("--check", action="store_true", help="validate expectation-level rule evidence without running tests")
    ap.add_argument("--run", action="store_true", help="run expectation-level evidence (requires --evidence)")
    ap.add_argument("--ledger", type=Path, help="write generated JSON ledger")
    ap.add_argument("--report", action="store_true")
    ap.add_argument("--overrides", type=Path)
    ap.add_argument("--results", type=Path)
    ap.add_argument("--run-tests", action="store_true", help="run workspace tests and record actual results")
    ap.add_argument("--untested", action="store_true")
    args = ap.parse_args(argv)
    ledger_mode = bool(args.ledger or args.report or args.run_tests)
    modes = sum((bool(args.evidence or args.check), ledger_mode, args.untested, bool(args.prefix)))
    if modes > 1:
        ap.error("choose one of rule prefix, --evidence/--check, ledger/report, or --untested")
    if args.run and not args.evidence:
        ap.error("--run requires --evidence")
    if args.run and args.check:
        ap.error("--check cannot run tests")
    if (args.overrides or args.results) and not ledger_mode:
        ap.error("--overrides and --results require --ledger, --report or --run-tests")
    if args.evidence or args.check:
        return evidence_main(args)
    if ledger_mode:
        args.overrides = args.overrides or ROOT / "docs/rule-assessments.json"
        args.results = args.results or ROOT / ".cache/conformance-tests.json"
        if args.run_tests:
            result = run_tests(args.results)
        data = ledger(args.overrides, args.results)
        if args.ledger:
            args.ledger.parent.mkdir(parents=True, exist_ok=True)
            args.ledger.write_text(json.dumps(data, indent=2, ensure_ascii=False) + "\n")
        print(report(data))
        if args.run_tests and result["exit_code"]:
            return result["exit_code"]
        return 0
    rulebook, named = inventory(), tests()
    all_rules = {rid: entry["text"] for rid, entry in rulebook["rules"].items()}
    if args.untested:
        # implemented by its own account, and not asked by any test of its own
        missing = {r: files for r, files in cited().items() if r in all_rules and r not in named}
        for rule in sorted(missing, key=lambda r: [int(r.split(".")[0]), r.split(".")[1]]):
            print(f"{rule:9} {', '.join(missing[rule]):28} {all_rules[rule][:90]}")
        print(f"UNTESTED {len(missing)} of {len([r for r in cited() if r in all_rules])} rules the engine cites")
        return 0
    if args.prefix:
        import rule_evidence
        prefix = args.prefix
        joined = rule_evidence.join(rule_evidence.discover(), rule_evidence.load_run(), suite_hash())
        for rule, text in all_rules.items():
            if rule.startswith(prefix):
                mark = "x" if rule in named else " "
                print(f"[{mark}] {rule} {text}")
                source = rulebook["rules"][rule]
                print(f"    Source: {RULES.relative_to(ROOT)}:{source['line']}")
                for example in source["examples"]:
                    print(f"    Example: {example}")
                attributed = rule_evidence.drill(rule, joined)
                if attributed:
                    print("    Attributed expectations" + ("" if joined["fresh"] else " (no current run: not run)") + ":")
                    print("\n".join(attributed))
                if named.get(rule):
                    print("    Legacy test references (test name or // Rules:, not expectation evidence):")
                for t in named.get(rule, []):
                    print(f"    {t['path']}:{t['line']}: {t['test']}")
                    print(f"        {test_command(t)}")
        return 0
    unknown = sorted(r for r in named if r not in all_rules)
    print(f"{'section':40} rules  referenced")
    for digit, title in SECTIONS.items():
        of_section = [r for r in all_rules if r[0] == digit]
        tested = [r for r in of_section if r in named]
        print(f"{digit} {title:38} {len(of_section):5}  {len(tested):6}")
    print(f"{'all':40} {len(all_rules):5}  {len([r for r in all_rules if r in named]):6}")
    if unknown:
        print(f"tests that name a rule the rules do not have: {', '.join(unknown)}")
    print(f"TESTED {len([r for r in all_rules if r in named])}")
    print("References locate evidence; they do not establish coverage or passing execution.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
