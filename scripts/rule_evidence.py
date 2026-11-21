"""Rule evidence attributed to individual expectations of the rules test target.

    uv run scripts/rules.py --evidence            discovered expectation sites, and the last run
    uv run scripts/rules.py --evidence --run      run the rules target with receipts, then report
    uv run scripts/rules.py --evidence --check    reject invalid attribution (no run)
    uv run scripts/rules.py 601.2d                a rule's attributed expectations and legacy refs

Sites are discovered from source: `cite!([ids]; …)` and the steps of an `expect()` builder after
`.rules([ids])` (see crates/game/tests/support/cite.rs). A run of the `rules` target writes a
receipt for each expectation that completed or failed, naming the libtest thread (the qualified
test). A receipt is evidence only for its own run, while the source is unchanged since that run
(suite hash), and passing evidence only when the enclosing test also passed. Each expectation's
rules are joint evidence of their interaction, never independent proof of each rule. Test-level
name prefixes and `// Rules:` comments stay legacy discovery, reported separately.
"""
from __future__ import annotations

import json
import os
import re
import subprocess
import uuid
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
TESTS = ROOT / "crates/game/tests"
CACHE = ROOT / ".cache"
RUN = CACHE / "rule-evidence-run.json"
# what the measure counts; a change makes earlier counts incomparable
SCOPE = "game/rules cite! and expect().rules expectations, tooling/ excluded, v1"
# tooling fixtures exercise the attribution itself: never coverage evidence or obligations
FIXTURES = TESTS / "tooling"
RULE_ID = re.compile(r"[1-9]\d{2}\.\d+[a-z]{0,2}")
FAILED_RECEIPTS = "RULE RECEIPTS NOT WRITTEN"  # crates/game/tests/support/cite.rs
ASSERTS = re.compile(r"\b(?:assert|assert_eq|assert_ne|debug_assert)!\s*\(")


def mask(src: str) -> str:
    """The source with comment and literal contents blanked (same length and lines), so that
    brackets and commas inside them do not count as structure. Quotes themselves remain."""
    out, i, n = list(src), 0, len(src)

    def blank(a, b):
        for k in range(a, b):
            if out[k] != "\n":
                out[k] = " "
    while i < n:
        c = src[i]
        if src.startswith("//", i):
            j = src.find("\n", i)
            j = n if j < 0 else j
            blank(i, j)
            i = j
        elif src.startswith("/*", i):
            depth, j = 1, i + 2
            while j < n and depth:
                if src.startswith("/*", j):
                    depth, j = depth + 1, j + 2
                elif src.startswith("*/", j):
                    depth, j = depth - 1, j + 2
                else:
                    j += 1
            blank(i, j)
            i = j
        elif m := re.match(r'(?:b?r)(#*)"', src[i:i + 260]) if c in "rb" and (i == 0 or not (src[i - 1].isalnum() or src[i - 1] == "_")) else None:
            close = '"' + m[1]
            j = src.find(close, i + len(m[0]))
            j = n if j < 0 else j
            blank(i + len(m[0]), j)
            i = j + len(close)
        elif c == '"':
            j = i + 1
            while j < n and src[j] != '"':
                j += 2 if src[j] == "\\" else 1
            blank(i + 1, j)
            i = j + 1
        elif c == "'" and (m := re.match(r"'(?:\\.|\\u\{[0-9a-fA-F]+\}|[^\\'])'", src[i:i + 12])):
            blank(i + 1, i + len(m[0]) - 1)
            i += len(m[0])
        else:
            i += 1
    return "".join(out)


def closing(masked: str, at: int) -> int:
    """The index just past the bracket that closes the one at `at`."""
    pairs = {"(": ")", "[": "]", "{": "}"}
    stack = []
    for i in range(at, len(masked)):
        c = masked[i]
        if c in pairs:
            stack.append(pairs[c])
        elif c in ")]}":
            if not stack or stack.pop() != c:
                raise ValueError(f"unbalanced {c!r} at {i}")
            if not stack:
                return i + 1
    raise ValueError(f"unclosed bracket at {at}")


def split_top(masked: str, a: int, b: int, *, turbofish: bool = False) -> list[tuple[int, int]]:
    """Comma-separated items of masked[a:b] at depth 0, as (start, end) ranges."""
    items, depth, generic, start = [], 0, 0, a
    for i in range(a, b):
        c = masked[i]
        if c in "([{":
            depth += 1
        elif c in ")]}":
            depth -= 1
        elif turbofish and depth == 0 and c == "<" and (generic or re.search(r"::\s*$", masked[a:i])):
            generic += 1
        elif turbofish and depth == 0 and c == ">" and generic and masked[i - 1] != "-":
            generic -= 1
        elif c == "," and depth == 0 and generic == 0:
            items.append((start, i))
            start = i + 1
    items.append((start, b))
    return [(s, e) for s, e in items if masked[s:e].strip()]


def module_path(path: Path) -> str:
    """The module of a file under the rules target: tests/scenarios/legal.rs is scenarios::legal."""
    parts = list(path.relative_to(TESTS).with_suffix("").parts)
    if parts and parts[-1] == "mod":
        parts.pop()
    return "::".join(parts)


class Source:
    def __init__(self, path: Path):
        self.path, self.rel = path, str(path.relative_to(ROOT))
        self.src = path.read_text()
        self.masked = mask(self.src)
        self.starts = [0] + [m.end() for m in re.finditer("\n", self.src)]
        self.module = module_path(path)
        self.functions = self._functions()

    def position(self, at: int) -> tuple[int, int]:
        line = max(i for i, s in enumerate(self.starts) if s <= at)
        return line + 1, at - self.starts[line] + 1

    def _functions(self) -> list[dict]:
        found = []
        for m in re.finditer(r"\bfn\s+(\w+)", self.masked):
            brace = self.masked.find("{", m.end())
            semi = self.masked.find(";", m.end())
            if brace < 0 or 0 <= semi < brace:
                continue
            end = closing(self.masked, brace)
            line = self.position(m.start())[0]
            found.append({"name": m[1], "start": m.start(), "body": (brace, end), "line": line,
                          **self._attributes(m.start())})
        return found

    def _attributes(self, at: int) -> dict:
        """The outer attributes of the item whose `fn` is at `at`, read back token by token
        (comments are blank in the masked source), wherever their lines break."""
        attributes, k = [], at
        while True:
            head = re.search(r"(?:\b(?:pub(?:\s*\([^()]*\))?|async|unsafe|const|extern(?:\s*\"[^\"]*\")?)\s*)*$", self.masked[:k])
            k = head.start() if head else k
            while k > 0 and self.masked[k - 1].isspace():
                k -= 1
            if k == 0 or self.masked[k - 1] != "]":
                break
            depth, j = 0, k - 1
            while j >= 0:
                depth += {"]": 1, "[": -1}.get(self.masked[j], 0)
                if depth == 0:
                    break
                j -= 1
            if j < 1 or self.masked[j - 1] != "#":
                break
            attributes.append(re.sub(r"\s+", "", self.masked[j + 1:k - 1]))
            k = j - 1
        return {"test": "test" in attributes, "should_panic": any(a.startswith("should_panic") for a in attributes)}

    def enclosing(self, at: int) -> dict | None:
        inside = [f for f in self.functions if f["body"][0] < at < f["body"][1]]
        return min(inside, key=lambda f: f["body"][1] - f["body"][0]) if inside else None

    def ids(self, a: int, b: int) -> tuple[list[str], list[str]]:
        """The literal rule IDs of a `[...]` list, and the items given by expressions."""
        literal, dynamic = [], []
        for s, e in split_top(self.masked, a, b):
            item = self.src[s:e].strip()
            if m := re.fullmatch(r'"([^"\\]*)"', item):
                literal.append(m[1])
            else:
                dynamic.append(item)
        return literal, dynamic


def site(source: Source, at: int, kind: str, literal: list[str], dynamic: list[str], **extra) -> dict:
    line, column = source.position(at)
    owner = source.enclosing(at)
    qualified = f"{source.module}::{owner['name']}" if owner and source.module else (owner or {}).get("name")
    return {"file": source.rel, "line": line, "column": column, "kind": kind, "rules": literal,
            "row_supplied": dynamic, "function": qualified,
            "in_test": bool(owner and owner["test"]), "should_panic": bool(owner and owner["should_panic"]), **extra}


def cite_sites(source: Source) -> list[dict]:
    out = []
    macros = [(m.start(), closing(source.masked, m.end() - 1))
              for m in re.finditer(r"\bmacro_rules!\s*\w+\s*[{(\[]", source.masked)]
    for m in re.finditer(r"\bcite!\s*\(", source.masked):
        if any(a < m.start() < b for a, b in macros):
            # its receipt would be located at the macro's use, never at this site
            out.append(site(source, m.start(), "unsupported", [], [], problem="cite! inside a macro_rules! body"))
            continue
        open_paren = m.end() - 1
        end = closing(source.masked, open_paren)
        bracket = source.masked.find("[", open_paren)
        if bracket < 0 or source.masked[open_paren + 1:bracket].strip():
            out.append(site(source, m.start(), "unsupported", [], [], problem="cite! without a [rules] list"))
            continue
        close = closing(source.masked, bracket)
        literal, dynamic = source.ids(bracket + 1, close - 1)
        rest = source.masked[close:end]
        case = bool(re.match(r"\s*;\s*case\s*:", rest))
        body = rest.split(";", 2)[-1] if case else rest.split(";", 1)[-1]
        kind = "ok" if re.match(r"\s*ok\s*:", body) else "refused" if re.match(r"\s*refused\s*\(", body) else "assert"
        extra = {"case": case}
        if kind == "refused":
            reason = source.src[end - len(body):end]
            if not re.match(r'\s*refused\s*\(\s*\S', body) or re.match(r'\s*refused\s*\(\s*"\s*"\s*\)', reason):
                extra["problem"] = "refused() without a reason; an empty one is in every error"
        if kind == "assert" and re.match(r"\s*debug_assert", body):
            extra["problem"] = "cite! of a debug assertion, which checks nothing where debug assertions are off (the fast profile)"
        elif kind == "assert" and not re.match(r"\s*\w*assert\w*!", body):
            # the macro also requires `()`; this names the mistake before a build does
            extra["problem"] = "cite! of neither an assertion, ok: nor refused(…)"
        out.append(site(source, m.start(), kind, literal, dynamic, **extra))
    return out


def builder_sites(source: Source) -> list[dict]:
    """The steps of each `….expect()…check()` chain, cited by the `.rules([…])` before them."""
    out = []
    for m in re.finditer(r"\.\s*expect\s*\(\s*\)", source.masked):
        at, rules, dynamic, closed = m.end(), [], [], False
        while True:
            step = re.match(r"\s*\.\s*(\w+)\s*\(", source.masked[at:])
            if not step:
                break
            name_at = at + step.start(1)
            paren = at + step.end() - 1
            end = closing(source.masked, paren)
            name = step[1]
            if name == "rules":
                bracket = source.masked.find("[", paren)
                if 0 <= bracket < end and not source.masked[paren + 1:bracket].strip():
                    rules, dynamic = source.ids(bracket + 1, closing(source.masked, bracket) - 1)
                else:
                    rules, dynamic = [], [source.src[paren + 1:end - 1].strip()]
            elif name == "check":
                closed = True
                break
            else:
                kind = "step" if rules or dynamic else "uncited-step"
                out.append(site(source, name_at, kind, list(rules), list(dynamic), step=name))
            at = end
        if not closed and any(s.get("step") for s in out):
            line, _ = source.position(m.start())
            out.append(site(source, m.start(), "unsupported", [], [], problem=f"expect() builder at line {line} not followed to .check()"))
    return out


def uncited_asserts(source: Source, cited: list[tuple[int, int]]) -> list[dict]:
    out = []
    for m in ASSERTS.finditer(source.masked):
        if any(a <= m.start() < b for a, b in cited):
            continue
        owner = source.enclosing(m.start())
        if owner and owner["test"]:
            line, column = source.position(m.start())
            out.append({"file": source.rel, "line": line, "column": column, "function": f"{source.module}::{owner['name']}"})
    return out


def refusal_assertion_problems(paths: list[Path] | None = None) -> list[str]:
    """Reject error-presence assertions, including uncited tests and inline unit tests.

    This is a source guard, not type/flow analysis or semantic review. Production
    error handling, comments/literals and assertion diagnostic arguments are untouched.
    """
    problems = []
    assertions = re.compile(r"\b(?:debug_)?assert(?:_(?:eq|ne))?\s*!\s*[(\[{]")
    for path in sorted(paths if paths is not None else (ROOT / "crates").rglob("*.rs")):
        masked = mask(path.read_text())
        for match in assertions.finditer(masked):
            end = closing(masked, match.end() - 1)
            operands = split_top(masked, match.end(), end - 1, turbofish=True)
            count = 2 if match[0].split("!")[0].strip().endswith(("_eq", "_ne")) else 1
            if any(re.search(r"\.\s*is_err\s*\(\s*\)", masked[a:b]) for a, b in operands[:count]):
                line = masked.count("\n", 0, match.start()) + 1
                problems.append(f"{path.relative_to(ROOT)}:{line}: assertion uses is_err() without "
                                "authenticating the error; use refused(reason) or assert the expected error")
    return problems


def discover(paths: list[Path] | None = None, known: set[str] | None = None) -> dict:
    """Every attributed site of the rules target, the uncited assertions of files that cite,
    attribution problems, and reason-free Rust assertion problems."""
    if known is None:
        import rules as rulebook
        known = set(rulebook.inventory()["rules"])
    sites, uncited, problems = [], [], refusal_assertion_problems(paths)
    for path in sorted(paths if paths is not None else (p for p in TESTS.rglob("*.rs") if FIXTURES not in p.parents)):
        source = Source(path)
        found = cite_sites(source) + builder_sites(source)
        sites += found
        cited_ranges = []
        for m in re.finditer(r"\bcite!\s*\(", source.masked):
            cited_ranges.append((m.start(), closing(source.masked, m.end() - 1)))
        if any(s["kind"] != "uncited-step" for s in found):
            uncited += uncited_asserts(source, cited_ranges)
    for s in sites:
        where = f"{s['file']}:{s['line']}"
        if s.get("problem"):
            problems.append(f"{where}: {s['problem']}")
        if s["should_panic"] and (s["rules"] or s["row_supplied"]):
            problems.append(f"{where}: attributed expectation in a #[should_panic] test; its success would not show the expectation passed")
        for rid in s["rules"]:
            if not RULE_ID.fullmatch(rid):
                problems.append(f"{where}: {rid!r} is not a complete rule number")
            elif rid not in known:
                problems.append(f"{where}: {rid} is not in the pinned rulebook")
    return {"sites": sites, "uncited": uncited, "problems": problems, "known": known}


def receipts_env() -> tuple[str, Path, dict]:
    """A new run's id, its receipts file and the environment that makes `cite!` write them."""
    run_id = uuid.uuid4().hex
    receipts = CACHE / "rule-receipts" / f"{run_id}.jsonl"
    receipts.parent.mkdir(parents=True, exist_ok=True)
    return run_id, receipts, {**os.environ, "RULE_RECEIPTS": str(receipts), "RULE_RUN": run_id}


def record(run_id: str, receipts: Path, output: str | None, exit_code: int, before: str, after: str,
           revision: dict, command: str) -> dict:
    """Record the rules target's run from its own output (None: it did not run, as when it
    failed to build) and the exit code of the command that ran it."""
    output = output or ""
    outcomes = dict(re.findall(r"^test (\S+)(?: - should panic)? \.\.\. (ok|FAILED|ignored)\b", output, re.M))
    # Complete means the recorded (unfiltered) command finished and no receipt failed to be
    # written (said in a failed test's captured output, or on stderr); not that discovery or
    # coverage is complete.
    complete = bool(re.search(r"^test result: ", output, re.M)) and FAILED_RECEIPTS not in output
    data = {"run": run_id, "target": "game/rules", "revision": revision, "suite_sha256": before, "unchanged": before == after,
            "tooling_sha256": tooling_hash(),
            "exit_code": exit_code, "complete": complete, "tests": outcomes,
            "receipts": str(receipts.relative_to(ROOT)), "command": command}
    (CACHE / "rule-evidence-run.log").write_text(output)
    RUN.write_text(json.dumps(data, indent=2) + "\n")
    return data


def run(suite_hash, revision) -> dict:
    """Run the rules target with receipts; record each qualified test's outcome."""
    run_id, receipts, env = receipts_env()
    before = suite_hash()
    command = ["cargo", "test", "-p", "game", "--test", "rules", "--", "--color", "never"]
    result = subprocess.run(command, cwd=ROOT, env=env, capture_output=True, text=True)
    return record(run_id, receipts, result.stdout + result.stderr, result.returncode, before, suite_hash(),
                  revision(), " ".join(command))


def tooling_hash() -> str:
    """The scanner and reporter decide what a receipt is evidence of: a run is joined only by
    the same code (the Rust side, support/cite.rs, is in the suite hash)."""
    import hashlib
    digest = hashlib.sha256()
    for path in (Path(__file__), Path(__file__).with_name("rules.py")):
        digest.update(path.name.encode() + b"\0" + path.read_bytes() + b"\0")
    return digest.hexdigest()


def load_run(path: Path = RUN) -> dict | None:
    return json.loads(path.read_text()) if path.exists() else None


def receipts_of(data: dict) -> list[dict]:
    path = ROOT / data["receipts"]
    rows = [json.loads(line) for line in path.read_text().splitlines() if line.strip()] if path.exists() else []
    return [r for r in rows if r.get("run") == data["run"]]


def join(found: dict, data: dict | None, current_hash: str) -> dict:
    """Each rule's attributed expectations and their outcomes in the last run, if it is current.

    An expectation's outcome is `passing` (it completed and its test passed), `diagnostic`
    (it completed, its test failed later), `failing` (it failed), `not run` (no receipt: an
    untaken branch, a failure before it, a test that did not run) or `unattributed` (a receipt
    from a thread that is no test of the run)."""
    fresh = bool(data and data["complete"] and data["unchanged"] and data["suite_sha256"] == current_hash
                 and data.get("tooling_sha256") == tooling_hash())
    receipts = receipts_of(data) if fresh else []
    tests = data["tests"] if fresh else {}
    at = {(s["file"], s["line"], s["column"]): s for s in found["sites"]}
    rules: dict[str, list[dict]] = {}
    seen = set()
    unmatched, unattributed, problems = [], [], []
    fixtures = str(FIXTURES.relative_to(ROOT)) + "/"
    for r in receipts:
        if r["file"].startswith(fixtures):
            continue
        s = at.get((r["file"], r["line"], r["column"]))
        if s is None:
            unmatched.append(r)
            continue
        test = r.get("test")
        if test not in tests:
            unattributed.append(r)
            continue
        seen.add((r["file"], r["line"], r["column"], test))
        outcome = "failing" if r["outcome"] == "failed" else "passing" if tests[test] == "ok" else "diagnostic"
        # a rule number outside the pinned rulebook (a table row's, say) is never evidence
        invalid = [rid for rid in r["rules"] if rid not in found["known"]]
        for rid in invalid:
            problems.append(f"{r['file']}:{r['line']}: receipt for {rid!r}, not in the pinned rulebook ({test})")
        if invalid:
            continue
        for rid in r["rules"]:
            rules.setdefault(rid, []).append({"file": r["file"], "line": r["line"], "test": test, "case": r.get("case"),
                                              "rules": r["rules"], "outcome": outcome, "kind": s["kind"]})
    for s in found["sites"]:
        if s["kind"] in ("uncited-step", "unsupported"):
            continue
        ran = any(key[:3] == (s["file"], s["line"], s["column"]) for key in seen)
        if ran:
            continue
        # a site in a test fn did not run in that test; a helper's callers are unknown statically
        for rid in (r for r in s["rules"] if r in found["known"]):
            rules.setdefault(rid, []).append({"file": s["file"], "line": s["line"], "test": s["function"] if s["in_test"] else None,
                                              "case": None, "rules": s["rules"], "outcome": "not run", "kind": s["kind"]})
    status = {}
    for rid, rows in rules.items():
        outcomes = {r["outcome"] for r in rows}
        status[rid] = next(o for o in ("failing", "passing", "diagnostic", "not run") if o in outcomes)
    return {"fresh": fresh, "run": data and {k: data[k] for k in ("run", "target", "revision", "suite_sha256", "unchanged", "complete", "exit_code")},
            "rules": rules, "status": status, "unmatched": unmatched, "unattributed": unattributed, "problems": problems}


def summary(found: dict, joined: dict, all_rules: dict) -> str:
    sites = [s for s in found["sites"] if s["kind"] not in ("uncited-step", "unsupported")]
    files = sorted({s["file"] for s in sites})
    counts = {k: sum(v == k for v in joined["status"].values()) for k in ("passing", "failing", "diagnostic", "not run")}
    rows = [f"Attributed expectations: {len(sites)} sites in {len(files)} files, citing {len({r for s in sites for r in s['rules']})} rules"
            + (f" (+{sum(bool(s['row_supplied']) for s in sites)} with row-supplied rules)" if any(s['row_supplied'] for s in sites) else ""),
            f"Uncited assertions in those files: {len(found['uncited'])}; uncited builder steps: {sum(s['kind'] == 'uncited-step' for s in found['sites'])}"]
    if joined["fresh"]:
        run = joined["run"]
        rows.append(f"Run {run['run'][:12]} at {run['revision']['commit'][:10]}{' (dirty)' if run['revision']['worktree'] else ''}: "
                    f"rules passing {counts['passing']}, failing {counts['failing']}, diagnostic only {counts['diagnostic']}, not run {counts['not run']}; "
                    f"of {len(all_rules)} pinned rules, {len(all_rules) - len(joined['status'])} have no attributed evidence")
    else:
        rows.append("No current run: receipts are historical or absent; no attributed expectation counts as run")
    if joined["unattributed"]:
        rows.append(f"Unattributed receipts (no test of the run, e.g. a spawned thread): {len(joined['unattributed'])}")
    if joined["unmatched"]:
        rows.append(f"Receipts at no discovered site: {len(joined['unmatched'])}")
    for p in found["problems"] + joined["problems"]:
        rows.append("PROBLEM " + p)
    rows.append("An expectation's rules are joint evidence of their interaction; breadth is not implementation.")
    return "\n".join(rows)


def drill(rid: str, joined: dict) -> list[str]:
    out = []
    for r in sorted(joined["rules"].get(rid, []), key=lambda r: (r["file"], r["line"], r["test"] or "")):
        joint = "" if r["rules"] == [rid] else f" with {', '.join(x for x in r['rules'] if x != rid)}"
        case = f" [{r['case']}]" if r.get("case") else ""
        out.append(f"    {r['outcome']:10} {r['file']}:{r['line']} {r['kind']}{joint} in {r['test'] or 'a helper'}{case}")
    return out


def measure(found: dict, data: dict | None, current_hash: str, total: int) -> dict:
    """The progress report's expectation-level rule measure. Static counts come from source and
    are always current; execution counts only from a complete run whose inputs are unchanged,
    else they are unavailable (None) and the last run is named with why it does not count. No
    attributed expectation is not proof that no legacy test exercises the rule."""
    sites = [s for s in found["sites"] if s["kind"] not in ("uncited-step", "unsupported")]
    cited = {r for s in sites for r in s["rules"]}
    joined = join(found, data, current_hash)
    if data is None:
        state = "MISSING"
    elif not data["complete"]:
        state = "INCOMPLETE"
    elif not data["unchanged"] or data["suite_sha256"] != current_hash or data.get("tooling_sha256") != tooling_hash():
        state = "STALE"
    else:
        state = "CURRENT"
    execution = None
    if joined["fresh"]:
        statuses = list(joined["status"].values())
        execution = {k.replace(" ", "_"): statuses.count(k) for k in ("passing", "failing", "diagnostic", "not run")}
        execution["unattributed_receipts"] = len(joined["unattributed"])
    return {"scope": SCOPE, "total": total, "rules_with_sites": len(cited), "rules_without_sites": total - len(cited),
            "sites": len(sites), "row_supplied_sites": sum(bool(s["row_supplied"]) for s in sites),
            "problems": found["problems"] + joined["problems"], "run_state": state, "execution": execution,
            "run": data and {k: data.get(k) for k in ("run", "target", "revision", "exit_code", "complete", "unchanged")},
            "meaning": "Rules of attributed expectations (cite!, expect().rules); each expectation's rules are joint "
                       "evidence. None attributed is not proof that no legacy test exercises a rule."}


def render(m: dict) -> list[str]:
    lines = [f"  Rules with attributed expectations  {m['rules_with_sites']:,}/{m['total']:,} "
             f"({m['sites']:,} expectations; {m['rules_without_sites']:,} rules have none attributed)"]
    if m["row_supplied_sites"]:
        lines.append(f"    expectations taking their rules from table rows (resolved only by a run): {m['row_supplied_sites']}")
    run = m["run"]
    where = run and f"run {run['run'][:12]} at {run['revision']['commit'][:12]}{' (dirty)' if run['revision']['worktree'] else ''}"
    if m["execution"] is not None:
        e = m["execution"]
        lines.append(f"    Executed ({where}): passing {e['passing']:,}, failing {e['failing']:,}, "
                     f"diagnostic only {e['diagnostic']:,}, not run {e['not_run']:,}")
        if e["unattributed_receipts"]:
            lines.append(f"    {e['unattributed_receipts']} receipts name no test of the run")
    elif run:
        why = {"STALE": "inputs or evidence tooling changed since", "INCOMPLETE": "not the whole target, or receipts incomplete"}[m["run_state"]]
        lines.append(f"    Executed: UNAVAILABLE, last {where} is {m['run_state']} ({why}); not counted")
    else:
        lines.append("    Executed: UNAVAILABLE, no attributed run (make progress)")
    for problem in m["problems"]:
        lines.append(f"    PROBLEM {problem}")
    return lines
