#!/usr/bin/env python3
"""The MGN recorder: plays a game through the adapter protocol and writes what the engine did.

    uv run scripts/mgn/record.py --driver tests/fixtures/mgn/scenario-linked-first-v2/draft.json --tier 2 --out .cache/mgn-linked-first

Records are written from the rules by hand or produced by this recorder, which drives an engine
through the adapter protocol and writes the engine's projection at every boundary
as deltas (`delta.diff_states`). It is driven by a record an author drafted (`--driver RECORD`):
the runner replays the draft's decisions, the recorder keeps the engine's state at every boundary
the runner reaches, and the result has the draft's decisions with every delta written anew from
the engine. What is written is execution evidence, not vetting: the record's `review.jsonl`
starts with a `replayed` entry. The draft is kept beside the record as `draft.json`.

The draft's deltas must be right at the draft's own tier; they locate each boundary. Recorded at
tier 2 from a tier-1 draft (`--tier 2`), the tier-2 facts the draft never stated are supplied by
the engine and listed in `recording.json` as such; a tier-2 draft that states a fact differently
from the engine is a disagreement the rules must settle, so the recording is refused unless the
author accepts the engine's value for that path with `--accept PATH`, which the summary records.

Fail closed: a refused action, a boundary never reached, an accepted illegal attempt, a difference
at the start, a fact the engine could not project at some boundary, or a fact the record states
that the engine cannot confirm (`inconclusive`) each stop the recording with the reason. The facts
the adapter declares it never projects are left at their defaults, listed in the record's
`x-not-projected` member and named by the `replayed` review entry, for a reviewer to confirm. An
existing `record.json` is never overwritten: records are immutable and a successor supersedes.
"""
from __future__ import annotations

import argparse
from copy import deepcopy
import datetime as _dt
import hashlib
import json
from pathlib import Path
import sys
import subprocess

ROOT = Path(__file__).resolve().parents[2]
if __name__ == "__main__" and str(ROOT / "scripts") not in sys.path:
    sys.path.insert(0, str(ROOT / "scripts"))

# Freeze source before importing recorder dependencies: a long-lived process must
# not describe changed files as the sources of code it has already imported.
def _tooling_files() -> list[Path]:
    manifest = json.loads((ROOT / "resources/manifest.json").read_text(encoding="utf-8"))
    rules = [ROOT / "resources" / name for name in manifest["files"] if name.startswith("rules/")]
    return sorted([*(ROOT / "scripts/mgn").rglob("*.py"),
                   *(ROOT / "docs/mgn/schema").glob("*.json"),
                   ROOT / "resources/manifest.json",
                   ROOT / "resources/oracle/cards.jsonl",
                   ROOT / "resources/oracle/legal-standard.jsonl",
                   *rules])


_IMPORT_FILES = {str(p.resolve()): hashlib.sha256(p.read_bytes()).hexdigest() for p in _tooling_files()}

from mgn.canonical import canonical, loads  # noqa: E402
from mgn.delta import diff_states  # noqa: E402
from mgn.identity import game_identity, record_identity  # noqa: E402
from mgn.oracle import Oracle  # noqa: E402
from mgn.runner import LEVELS, Run, karn_adapter_command, summary_line  # noqa: E402
from mgn.state import State  # noqa: E402
from mgn.validate import validate  # noqa: E402

PRODUCER = {"name": "mgn-record", "version": "2"}
DEFAULT_ENGINE = ROOT / "target/fast/game-serve"
TIER1_LEVELS = ("L1", "L2", "L3")


class RecordingError(Exception):
    """The game could not be recorded as asked; the message says why."""


def _relative(path: Path) -> str:
    resolved = path.resolve()
    return str(resolved.relative_to(ROOT)) if resolved.is_relative_to(ROOT) else str(path)


def _digest(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def _provenance(engine: Path, oracle: Oracle, inputs: list[Path]) -> dict:
    files = dict(_IMPORT_FILES)
    for path in [engine, Path(sys.executable), oracle.path, *inputs]:
        files.setdefault(str(path.resolve()), _digest(path))
    try:
        commit = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True,
                                         stderr=subprocess.DEVNULL).strip()
    except (OSError, subprocess.CalledProcessError):
        commit = None
    provenance = {"engine_executable": {"path": str(engine.resolve()), "sha256": files[str(engine.resolve())]},
                  "python": {"path": str(Path(sys.executable).resolve()), "version": sys.version},
                  "tooling_checkout": {"path": str(ROOT), "commit": commit},
                  "files": files}
    _unchanged(provenance)
    # Oracle may have been constructed by a caller before the snapshot.
    fresh = Oracle(oracle.path)
    if {k: [f.raw for f in v] for k, v in oracle.faces.items()} != {k: [f.raw for f in v] for k, v in fresh.faces.items()}:
        raise RecordingError("recording provenance drift: loaded Oracle differs from its file")
    return provenance


def _unchanged(provenance: dict):
    current_tooling = {str(p.resolve()) for p in _tooling_files()}
    if current_tooling != set(_IMPORT_FILES):
        raise RecordingError("recording provenance drift: tooling input set changed")
    for name, digest in provenance["files"].items():
        path = Path(name)
        if not path.is_file() or _digest(path) != digest:
            raise RecordingError(f"recording provenance drift: {name}")


# ----- the recorder -----

def _zone_map(state: dict, cards: dict) -> dict[str, str]:
    return {o: zone for zone, members in state.get("zones", {}).items() for o in members if o in cards}


class Recorder(Run):
    """A run that keeps the engine's explicit projection at every boundary it reaches and the cards
    the adapter reported as new objects (400.7) along the way, so that a card that left a zone and
    came back to it within a stretch is written as moved."""

    def __init__(self, *args, **kwargs):
        super().__init__(*args, **kwargs)
        self.states: dict[int, dict] = {}
        self.unsupported_at: dict[int, list[str]] = {}
        self.renewed_at: dict[int, list[str]] = {}
        self.reentered: dict[int, list[str]] = {}
        self._renewed_since: set[str] = set()
        self._boundary_zones: dict[str, str] | None = None

    def _call(self, adapter, body):
        reply = super()._call(adapter, body)
        ok = reply.get("ok")
        if isinstance(ok, dict):
            self._renewed_since.update(ok.get("renewed", []))
        return reply

    def _boundary(self, after: int, actual: dict, unsupported: list[str]):
        zones = _zone_map(actual, self.record["cards"])
        if self._boundary_zones is not None:
            # a renewed card in the zone it held at the last boundary left and came back (400.7);
            # not for a library, where the engine renews a card moved within it (to the bottom, after
            # a look), which 400.7 does not cover and the delta writes as a reorder (spec 4.1): a card
            # that left its library and came back within one stretch is not told apart there
            self.reentered[after] = sorted(c for c in self._renewed_since
                                           if c in zones and self._boundary_zones.get(c) == zones[c] and not zones[c].endswith(".library"))
        self.renewed_at[after] = sorted(self._renewed_since)
        self.states[after] = deepcopy(actual)
        self.unsupported_at[after] = list(unsupported)
        self._boundary_zones = zones
        self._renewed_since = set()


def _without(explicit: dict, patterns: list[str]) -> dict:
    """The explicit projection without the facts of the given paths (`*` standing for one segment):
    the facts the adapter never projects are not diffed, so they stay at their defaults."""
    out = deepcopy(explicit)
    for pattern in patterns:
        _remove(out, pattern.split("."))
    return out


def _remove(value, parts: list[str]):
    if not parts or not isinstance(value, dict):
        return
    head, rest = parts[0], parts[1:]
    keys = list(value.keys()) if head == "*" else ([head] if head in value else [])
    for key in keys:
        if rest:
            _remove(value[key], rest)
        else:
            del value[key]


def _state_of(explicit: dict, driver: dict, previous: State | None = None) -> State:
    pseudo = {"tier": driver["tier"], "game": driver["game"], "cards": driver["cards"],
              "start": {"kind": "position", "state": explicit}}
    problems: list = []
    state = State.from_observation(pseudo, problems, previous)
    errors = [p for p in problems if p.severity == "error"]
    if errors:
        raise RecordingError("the engine's projection does not load as a state: " + "; ".join(str(p) for p in errors[:3]))
    return state


def _brief(items: list, limit: int = 4) -> str:
    return "; ".join(json.dumps(item)[:200] for item in items[:limit]) + (f"; and {len(items) - limit} more" if len(items) > limit else "")


def record_game(driver: dict, oracle: Oracle, engine: Path, timeout: float, *, drafted_tier: int,
                accept: tuple[str, ...] = ()) -> tuple[dict, dict, Recorder, dict]:
    """Replays the drafted driver on the engine and writes the engine's deltas.

    The driver's tier-1 facts must agree with the engine, and its tier-2 facts: a fact the draft
    never stated (its tier was 1) is supplied by the engine; a fact it stated differently is a
    disagreement the rules must settle, refused unless its path is in `accept`. Returns the record,
    the driver's replay report, the recorder and the summary of what the engine supplied or was
    accepted on."""
    errors = [str(p) for p in validate(driver, oracle) if p.severity == "error"]
    if errors:
        raise RecordingError("the driver record does not validate: " + "; ".join(errors[:5]))
    recorder = Recorder(driver, oracle, karn_adapter_command(engine), timeout)
    report = recorder.run()
    # everything that disqualifies the run, reported from the earliest boundary on: a later refusal
    # or divergence is usually a consequence of the first
    problems: list[tuple[int, str]] = []
    for d in report["divergences"]:
        tolerated = (d["after"] > 0 and d["level"] == "L4"
                     and (drafted_tier == 1 or d["path"] in accept))
        if not tolerated:
            what = ("the engine's start differs from the driver's" if d["after"] == 0
                    else "the engine disagrees with the draft on a tier-1 fact" if d["level"] in TIER1_LEVELS
                    else "the engine disagrees with the draft on a tier-2 fact, which the rules must settle (correct the draft, or --accept the path with your reason)")
            problems.append((d["after"], f"{what}: {json.dumps(d)[:240]}"))
    for item in report["inconclusive"]:
        problems.append((item["after"], f"the engine could not confirm a fact the driver states: {json.dumps(item)[:200]}"))
    for n, paths in recorder.unsupported_at.items():
        if paths:
            problems.append((n, f"the engine could not project a fact at the boundary after transition {n}: {', '.join(paths)}"))
    for refusal in report["refusals"]:
        problems.append((refusal["after"], f"the engine refused or failed: {json.dumps(refusal)[:240]}"))
    if problems:
        problems.sort(key=lambda p: p[0])
        raise RecordingError(problems[0][1] + (f"; and {len(problems) - 1} more problem(s) after it" if len(problems) > 1 else ""))
    if report["stopped_at"] is not None:
        raise RecordingError(f"the run stopped after transition {report['stopped_at']}: " + "; ".join(report.get("usage", {}).get("notes", [])))
    tier2 = [d for d in report["divergences"] if d["level"] == "L4"]
    supplied = [{"after": d["after"], "path": d["path"], "engine": d.get("actual")} for d in tier2] if drafted_tier == 1 else []
    accepted = [{"after": d["after"], "path": d["path"], "draft": d.get("expected"), "engine": d.get("actual")} for d in tier2] if drafted_tier == 2 else []
    unused = [p for p in accept if not any(a["path"] == p for a in accepted)]
    if unused:
        raise RecordingError(f"--accept names paths the engine did not dispute: {', '.join(unused)}")
    not_projected = list(report.get("unsupported", []))
    out = {key: deepcopy(driver[key]) for key in ("mgn", "tier", "pins", "game", "match", "cards", "start") if key in driver}
    out["author"] = {"kind": "tool", "name": "karn-recorder", "version": str(report["engine"]["version"])}
    out["producer"] = dict(PRODUCER)
    if not_projected:
        out["x-not-projected"] = not_projected
    running = State.from_record(driver, [])
    transitions = []
    for index, transition in enumerate(driver["transitions"]):
        n = index + 1
        entry = {key: deepcopy(value) for key, value in transition.items() if key != "delta"}
        if n in recorder.states:
            nxt = _state_of(_without(recorder.states[n], not_projected), driver, running)
            delta = diff_states(running, nxt, oracle, random_outcomes=transition.get("random", []), reentered=recorder.reentered.get(n, []))
            problems: list = []
            running.apply(delta, list(transition.get("random", [])), f"/transitions/{index}/delta", problems)
            errors = [p for p in problems if p.severity == "error"]
            if errors:
                raise RecordingError(f"transition {n}: the written delta does not apply: " + "; ".join(str(p) for p in errors[:3]))
            if running.explicit(oracle) != nxt.explicit(oracle):
                raise RecordingError(f"transition {n}: the written delta does not reproduce the engine's state")
            if delta or "do" in entry:
                entry["delta"] = delta
        elif "do" in entry:
            raise RecordingError(f"transition {n}: the run reached no boundary after this decision")
        transitions.append(entry)
    out["transitions"] = transitions
    extras = {"supplied": supplied, "accepted": accepted, "not_projected": not_projected,
              "renewed": {str(n): ids for n, ids in recorder.renewed_at.items() if ids}}
    return out, report, recorder, extras


def _clean(report: dict) -> bool:
    return (report["stopped_at"] is None and not report["inconclusive"]
            and all(report["levels"][level] in ("pass", "not_applicable") for level in LEVELS))


def _finish(record: dict, report: dict, extras: dict, oracle: Oracle, engine: Path, timeout: float, out_dir: Path, source: dict, receipt: str, provenance: dict) -> dict:
    """Validates the record, replays it, and writes it with its review log and recording summary
    into a directory that holds no record yet."""
    if (out_dir / "record.json").exists():
        raise RecordingError(f"{out_dir} already holds a record; records are immutable, so a successor goes into a directory of its own")
    errors = [str(p) for p in validate(record, oracle) if p.severity == "error"]
    if errors:
        raise RecordingError("the recorded record does not validate: " + "; ".join(errors[:5]))
    replay = Run(record, oracle, karn_adapter_command(engine), timeout).run()
    if not _clean(replay):
        raise RecordingError("the recorded record does not replay clean: " + summary_line(out_dir.name, replay))
    identity = record_identity(record)
    today = _dt.date.today().isoformat()
    findings = []
    if extras["not_projected"]:
        findings.append({"transition": 0, "note": "Not projected by the engine and left at their defaults in every delta, for a reviewer to confirm: " + ", ".join(extras["not_projected"])})
    if extras["supplied"]:
        findings.append({"transition": 0, "note": f"{len(extras['supplied'])} tier-2 facts the draft did not state are the engine's, unvetted (recording.json: supplied)"})
    for item in extras["accepted"]:
        findings.append({"transition": item["after"], "note": f"{item['path']}: the draft said {json.dumps(item['draft'])}, the engine {json.dumps(item['engine'])}; the author accepted the engine's value"})
    review = [{"record": identity, "reviewer": {"kind": "tool", "name": PRODUCER["name"], "version": PRODUCER["version"]},
               "date": today, "scope": {"transitions": [1, len(record["transitions"])]}, "tiers": [1, 2] if record["tier"] == 2 else [1],
               "verdict": "replayed", "findings": findings,
               "receipt": receipt + f"; the engine ({replay['engine']['version']}) replayed this record with every level passing; no semantic review claimed"}]
    summary = {"format": "mgn-recording/2", "record": identity, "game": game_identity(record), **source,
               "engine": report["engine"], "adapter": report["adapter"], "date": today,
               "transitions": len(record["transitions"]), "decisions_replayed": report["replayed"],
               "not_projected": extras["not_projected"], "supplied": extras["supplied"], "accepted": extras["accepted"],
               "renewed": extras["renewed"], "canonical_bytes": len(canonical(record).encode("utf-8")),
               "replay": {"levels": replay["levels"], "replayed": replay["replayed"], "divergences": len(replay["divergences"]),
                          "inconclusive": len(replay["inconclusive"]), "seconds": replay["usage"]["seconds"]}}
    _unchanged(provenance)
    summary["provenance"] = provenance
    out_dir.mkdir(parents=True, exist_ok=True)
    (out_dir / "record.json").write_text(canonical(record) + "\n", encoding="utf-8")
    with (out_dir / "review.jsonl").open("a", encoding="utf-8") as log:  # append-only (spec 9)
        for entry in review:
            log.write(canonical(entry) + "\n")
    (out_dir / "recording.json").write_text(json.dumps(summary, indent=1, ensure_ascii=False) + "\n", encoding="utf-8")
    return summary


def from_driver(driver_path: Path, out_dir: Path, engine: Path, oracle: Oracle, tier: int | None = None, timeout: float = 120.0,
                accept: tuple[str, ...] = ()) -> dict:
    """Records the game an author drafted in `driver_path`, at `tier` (the draft's unless given),
    and writes it under `out_dir`; the summary lists every tier-2 fact the engine supplied beyond
    the draft and every disagreement the author accepted."""
    provenance = _provenance(engine, oracle, [driver_path])
    driver_bytes = driver_path.read_bytes()
    provenance["driver_input"] = {"path": str(driver_path.resolve()), "sha256": hashlib.sha256(driver_bytes).hexdigest()}
    if provenance["driver_input"]["sha256"] != provenance["files"][str(driver_path.resolve())]:
        raise RecordingError("recording provenance drift: driver changed before loading")
    driver = loads(driver_bytes.decode("utf-8"))
    drafted_tier = driver["tier"]
    errors = [str(p) for p in validate(driver, oracle) if p.severity == "error"]
    if errors:
        raise RecordingError("the draft does not validate at its own tier: " + "; ".join(errors[:5]))
    if tier is not None:
        if tier < drafted_tier:
            raise RecordingError(f"a tier-{drafted_tier} draft is not recorded at tier {tier}")
        driver["tier"] = tier
    provenance["executed_driver_sha256"] = hashlib.sha256(canonical(driver).encode("utf-8")).hexdigest()
    record, report, _, extras = record_game(driver, oracle, engine, timeout, drafted_tier=drafted_tier, accept=accept)
    rel = _relative(driver_path)
    if "lineage" in driver:
        record["lineage"] = deepcopy(driver["lineage"])
    record["note"] = (str(driver.get("note", "")).rstrip() + f" Recorded through the Karn adapter from the draft {rel} (tier {drafted_tier}): "
                      f"its decisions, every delta the engine's projection ({report['engine']['version']}); the tier-2 facts the draft "
                      f"did not state are the engine's and unvetted until a review entry says otherwise; the facts the engine does not "
                      f"project are left at their defaults and listed in x-not-projected.").strip()
    record["x-source"] = {**driver.get("x-source", {}), "driver": rel, "driver_identity": record_identity(driver), "driver_tier": drafted_tier,
                         "driver_author": deepcopy(driver.get("author"))}
    source = {"driven_by": {"driver": rel, "identity": record_identity(driver), "tier": drafted_tier}}
    receipt = f"recorded through the Karn adapter from the draft {rel} (tier {drafted_tier})"
    summary = _finish(record, report, extras, oracle, engine, timeout, out_dir, source, receipt, provenance)
    (out_dir / "draft.json").write_bytes(driver_bytes)
    return summary


def main(argv=None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    parser.add_argument("--driver", type=Path, required=True, help="a drafted record to record the engine's deltas for")
    parser.add_argument("--tier", type=int, choices=(1, 2), help="the tier to record at (the draft's unless given)")
    parser.add_argument("--accept", action="append", default=[], metavar="PATH", help="a tier-2 path the engine disputes, recorded with the engine's value")
    parser.add_argument("--out", type=Path, required=True, help="a new directory for record.json, review.jsonl, recording.json and draft.json")
    parser.add_argument("--engine", type=Path, default=DEFAULT_ENGINE)
    parser.add_argument("--oracle", type=Path, default=None)
    parser.add_argument("--timeout", type=float, default=120.0)
    args = parser.parse_args(argv)
    oracle = Oracle(args.oracle) if args.oracle else Oracle()
    try:
        summary = from_driver(args.driver, args.out, args.engine, oracle, args.tier, args.timeout, tuple(args.accept))
    except RecordingError as error:
        print(f"not recorded: {error}", file=sys.stderr)
        return 1
    print(f"{args.out}: {summary['transitions']} transitions, {summary['canonical_bytes']} B canonical, replay "
          f"{summary['replay']['levels']}, {summary['replay']['seconds']}s; {len(summary['supplied'])} tier-2 facts supplied "
          f"by the engine, {len(summary['accepted'])} accepted")
    return 0


if __name__ == "__main__":
    sys.exit(main())
