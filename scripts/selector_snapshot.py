#!/usr/bin/env python3
"""Capture and compare selector meanings, canonical text and round-trip verdicts.

  python3 scripts/selector_snapshot.py capture /tmp/before.json
  python3 scripts/selector_snapshot.py capture /tmp/after.json
  python3 scripts/selector_snapshot.py compare /tmp/before.json /tmp/after.json --output /tmp/diff.json

Use the SAME extracted phrase file for both revisions. Extraction is existing infrastructure:
`python3 scripts/corpus_structure.py --dump-nps`. This tool does not rewrite corpus reports.
Comparison fails on every unexplained change (including round-trip -> variant), new ambiguity,
or new mismatch. Reviewed changes may be approved with exact old/new fingerprints and a
rationale in a JSON list passed as --reviewed; stale approvals fail too.
"""
from __future__ import annotations
import argparse
import collections
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]
ACCEPTED = {"round_trip", "variant"}

def digest(value):
    return hashlib.sha256(json.dumps(value, sort_keys=True, separators=(",", ":")).encode()).hexdigest()

def file_hash(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()

def source_hash(root):
    paths = [root / "Cargo.toml", root / "Cargo.lock"]
    for crate in ("grammar", "ir", "oracle"):
        for folder in ("src", "data"):
            paths.extend(p for p in (root / "crates" / crate / folder).rglob("*") if p.is_file())
        paths.append(root / "crates" / crate / "Cargo.toml")
    return digest([(str(p.relative_to(root)), file_hash(p)) for p in sorted(paths)])

def counts(rows):
    result = collections.Counter()
    for row in rows:
        if "skip" in row:
            result["skipped_phrases"] += 1
        else:
            result["examined_phrases"] += 1
            if any(r["verdict"] in ACCEPTED for r in row["readings"].values()): result["accepted_phrases"] += 1
            for rule, reading in row["readings"].items():
                result[rule + ":" + reading["verdict"]] += 1
    return dict(sorted(result.items()))

def capture(args):
    root = args.repo.resolve()
    phrases = (args.phrases or root / ".cache/oracle/noun_phrases.jsonl").resolve()
    corpus = (args.corpus or root / "resources/oracle/cards.jsonl").resolve()
    fingerprint = source_hash(root)
    inputs = {"phrases_sha256": file_hash(phrases), "corpus_sha256": file_hash(corpus)}
    revision = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=root, text=True).strip()
    dirty = bool(subprocess.check_output(["git", "status", "--porcelain", "--", "Cargo.toml", "Cargo.lock", "crates/grammar", "crates/ir", "crates/oracle"], cwd=root, text=True))
    subprocess.run(["cargo", "build", "--profile", "fast", "-p", "grammar", "--bin", "selector-snapshot", "-j", "2"], cwd=root, check=True, stdout=sys.stderr)
    target = Path(os.environ.get("CARGO_TARGET_DIR", "target"))
    if not target.is_absolute(): target = root / target
    binary = target / "fast/selector-snapshot"
    output = subprocess.check_output([str(binary), str(phrases)], cwd=root, text=True)
    rows = [json.loads(line) for line in output.splitlines()]
    if source_hash(root) != fingerprint or inputs != {"phrases_sha256": file_hash(phrases), "corpus_sha256": file_hash(corpus)}:
        raise ValueError("sources or inputs changed during capture; discard and rerun")
    snapshot = {"schema": 1, "revision": revision, "dirty_sources": dirty,
                "source_sha256": fingerprint, "binary_sha256": file_hash(binary),
                **inputs, "counts": counts(rows), "phrases": rows}
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(snapshot, indent=2, sort_keys=True) + "\n")
    print(json.dumps({"snapshot": str(args.output), "revision": revision, "distinct_phrases": len(rows), "counts": snapshot["counts"]}, indent=2))
    return 0

def compare(before, after, reviewed=()):
    for key in ("schema", "phrases_sha256", "corpus_sha256"):
        if before[key] != after[key]: raise ValueError(f"different {key}: snapshots must use identical inputs")
    def index(snapshot):
        result = {r["phrase"]: r for r in snapshot["phrases"]}
        if len(result) != len(snapshot["phrases"]): raise ValueError("duplicate phrase in snapshot")
        return result
    old, new = index(before), index(after)
    if old.keys() != new.keys(): raise ValueError("phrase sets differ despite identical input hashes")
    approvals = {}
    for item in reviewed:
        key = (item["phrase"], item["rule"], item["before_sha256"], item["after_sha256"])
        if not item.get("rationale", "").strip(): raise ValueError("reviewed changes require a rationale")
        if key in approvals: raise ValueError("duplicate reviewed change")
        approvals[key] = item["rationale"]
    changes, used = [], set()
    for phrase in sorted(old):
        a, b = old[phrase], new[phrase]
        if a.get("skip") != b.get("skip"): raise ValueError(f"skip policy changed: {phrase}")
        if "skip" in a: continue
        if a["readings"].keys() != b["readings"].keys(): raise ValueError("snapshot parser rules differ")
        for rule in sorted(a["readings"]):
            x, y = a["readings"][rule], b["readings"][rule]
            if x == y: continue
            accepted_x, accepted_y = x["verdict"] in ACCEPTED, y["verdict"] in ACCEPTED
            kind = "gained" if accepted_y and not accepted_x else "lost" if accepted_x and not accepted_y else "changed"
            if accepted_x and accepted_y:
                kind = "meaning" if x["value"] != y["value"] else "printing"
            key = (phrase, rule, digest(x), digest(y))
            rationale = approvals.get(key)
            if rationale is not None: used.add(key)
            broken = y["verdict"] in {"ambiguous", "mismatch"}
            changes.append({"phrase": phrase, "rule": rule, "kind": kind,
                            "before_sha256": key[2], "after_sha256": key[3],
                            "before": x, "after": y, "rationale": rationale,
                            "unexplained": rationale is None or broken})
    stale = sorted(set(approvals) - used)
    return {"before_revision": before["revision"], "after_revision": after["revision"],
            "before_counts": counts(before["phrases"]), "after_counts": counts(after["phrases"]),
            "changes": changes, "stale_reviews": stale,
            "ok": not stale and not any(c["unexplained"] for c in changes)}

def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    commands = parser.add_subparsers(dest="command", required=True)
    c = commands.add_parser("capture")
    c.add_argument("output", type=Path)
    c.add_argument("--repo", type=Path, default=ROOT)
    c.add_argument("--phrases", type=Path)
    c.add_argument("--corpus", type=Path)
    d = commands.add_parser("compare")
    d.add_argument("before", type=Path)
    d.add_argument("after", type=Path)
    d.add_argument("--reviewed", type=Path)
    d.add_argument("--output", type=Path)
    args = parser.parse_args()
    try:
        if args.command == "capture": return capture(args)
        result = compare(json.loads(args.before.read_text()), json.loads(args.after.read_text()),
                         json.loads(args.reviewed.read_text()) if args.reviewed else [])
        if args.output:
            args.output.parent.mkdir(parents=True, exist_ok=True)
            args.output.write_text(json.dumps(result, indent=2, sort_keys=True) + "\n")
        summary = {"ok": result["ok"], "changes": dict(collections.Counter(c["kind"] for c in result["changes"])),
                   "unexplained": sum(c["unexplained"] for c in result["changes"]), "stale_reviews": len(result["stale_reviews"]),
                   "before_counts": result["before_counts"], "after_counts": result["after_counts"]}
        print(json.dumps(result if not args.output else summary, indent=2, sort_keys=True))
        return 0 if result["ok"] else 1
    except (ValueError, KeyError, OSError, subprocess.CalledProcessError) as error:
        print(f"selector snapshot: {error}", file=sys.stderr)
        return 2

if __name__ == "__main__":
    raise SystemExit(main())
