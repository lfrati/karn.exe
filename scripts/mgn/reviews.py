#!/usr/bin/env python3
"""Query append-only MGN review logs without discarding entries or their provenance."""
from __future__ import annotations

import argparse
from pathlib import Path
import re
import sys

ROOT = Path(__file__).resolve().parents[2]
if __name__ == "__main__":
    sys.path.insert(0, str(ROOT / "scripts"))

from mgn.canonical import canonical, loads
from mgn.identity import record_identity
from mgn.schema import validate as validate_schema

VERDICTS = ("vetted", "defect", "inconclusive", "replayed")


def query(paths, *, record=None, verdicts=None, start=None, end=None):
    """Return matching entries in file/line order, with source path and physical line.

    Range filtering selects intersecting scopes, inclusively; it never clips the
    entry's scope or findings. Scope 'all' intersects every requested range.
    """
    if record is not None and not re.fullmatch(r"sha256:[0-9a-f]{64}", record):
        raise ValueError("record identity must be sha256: followed by 64 lowercase hex digits")
    if start is not None and (type(start) is not int or start < 0):
        raise ValueError("range start must be a nonnegative integer")
    if end is not None and (type(end) is not int or end < 0):
        raise ValueError("range end must be a nonnegative integer")
    if start is not None and end is not None and start > end:
        raise ValueError("range start exceeds range end")
    if verdicts is not None and set(verdicts) - set(VERDICTS):
        raise ValueError("unknown review verdict")
    matches = []
    for path in paths:
        path = Path(path)
        lines = path.read_text(encoding="utf-8").split("\n")
        if lines and lines[-1] == "":
            lines.pop()
        for number, line in enumerate(lines, 1):
            try:
                entry = loads(line)
                canonical(entry)
                problems = validate_schema(entry, "review-log.schema.json")
                if problems:
                    raise ValueError(str(problems[0]))
                scope = entry["scope"]
                if scope != "all" and scope["transitions"][0] > scope["transitions"][1]:
                    raise ValueError("scope range is reversed")
            except (ValueError, TypeError) as error:
                raise ValueError(f"{path}:{number}: {error}") from error
            if record is not None and entry["record"] != record:
                continue
            if verdicts is not None and entry["verdict"] not in verdicts:
                continue
            if scope != "all":
                low, high = scope["transitions"]
                if (start is not None and high < start) or (end is not None and low > end):
                    continue
            matches.append({"source": str(path), "line": number, "entry": entry})
    return matches


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("logs", nargs="+", type=Path)
    identity = parser.add_mutually_exclusive_group()
    identity.add_argument("--record", help="exact sha256 record identity")
    identity.add_argument("--record-file", type=Path, help="derive exact identity from this record")
    parser.add_argument("--verdict", action="append", choices=VERDICTS)
    parser.add_argument("--from", dest="start", type=int)
    parser.add_argument("--to", dest="end", type=int)
    args = parser.parse_args(argv)
    try:
        record = record_identity(loads(args.record_file.read_text(encoding="utf-8"))) if args.record_file else args.record
        for match in query(args.logs, record=record, verdicts=args.verdict, start=args.start, end=args.end):
            print(canonical(match))
    except (ValueError, TypeError, OSError) as error:
        print(f"mgn reviews: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
