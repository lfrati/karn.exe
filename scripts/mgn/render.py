#!/usr/bin/env python3
"""Lossless, line-oriented MGN text. JSON remains normative; see docs/mgn/rendering.md."""
from __future__ import annotations

import argparse
import json
from pathlib import Path
import re
import sys

ROOT = Path(__file__).resolve().parents[2]
if __name__ == "__main__":
    sys.path.insert(0, str(ROOT / "scripts"))

from mgn.canonical import canonical, loads

HEADER = "MGN-TEXT 1"
BARE = re.compile(r"[A-Za-z_][A-Za-z0-9_.-]*")
# Put the decision before its consequences; otherwise use canonical member order.
ORDER = ("mgn", "tier", "game", "pins", "author", "note", "cards", "start", "transitions",
         "seat", "do", "attempt", "illegal", "kind", "random", "delta")


def _keys(mapping):
    return sorted(mapping, key=lambda key: (ORDER.index(key) if key in ORDER else len(ORDER), key.encode("utf-16-be")))


def _record(record):
    if not isinstance(record, dict) or record.get("mgn") != "1" or not isinstance(record.get("transitions"), list):
        raise ValueError("expected an mgn/1 object with a transitions array")
    if any(not isinstance(entry, dict) for entry in record["transitions"]):
        raise ValueError("each transition must be an object")
    canonical(record)  # Reject non-JSON values and values without a canonical representation.



def _text_value(value) -> str:
    # Canonical JSON spells some doubles (e.g. 1e20) as integer tokens beyond
    # the integer input range of canonical(). Keep their floating-point syntax
    # in this derived format so loads() retains the original numeric domain.
    if isinstance(value, float):
        return repr(value)
    if isinstance(value, (list, tuple)):
        return "[" + ",".join(_text_value(item) for item in value) + "]"
    if isinstance(value, dict):
        keys = sorted(value, key=lambda key: key.encode("utf-16-be"))
        return "{" + ",".join(canonical(key) + ":" + _text_value(value[key]) for key in keys) + "}"
    return canonical(value)


def render(record: dict) -> str:
    """Render all record members, without defaults, truncation or semantic rewriting."""
    _record(record)
    lines = [HEADER]

    def mapping(value, depth, root=False):
        for key in _keys(value):
            label = key if BARE.fullmatch(key) else canonical(key)
            item = value[key]
            prefix = "  " * depth + label
            if root and key == "transitions" and item:
                lines.append(prefix + ":")
                for number, entry in enumerate(item, 1):
                    if entry:
                        lines.append("  " * (depth + 1) + f"{number}:")
                        mapping(entry, depth + 2)
                    else:
                        lines.append("  " * (depth + 1) + f"{number} = {{}}")
            elif isinstance(item, dict) and item:
                lines.append(prefix + ":")
                mapping(item, depth + 1)
            else:
                lines.append(prefix + " = " + _text_value(item))

    mapping(record, 0, root=True)
    return "\n".join(lines) + "\n"


def parse(text: str) -> dict:
    """Read text strictly; syntax errors include their physical line number.

    Standalone # comments are presentation only. Record notes are ordinary note members.
    """
    rows = []
    for number, line in enumerate(text.split("\n"), 1):
        if not line.strip() or line.lstrip(" ").startswith("#"):
            continue
        spaces = len(line) - len(line.lstrip(" "))
        if "\t" in line[:spaces + 1] or spaces % 2:
            raise ValueError(f"line {number}: indentation must use pairs of spaces")
        rows.append((number, spaces // 2, line[spaces:]))
    if not rows or rows[0][1:] != (0, HEADER):
        raise ValueError(f"line {rows[0][0] if rows else 1}: expected {HEADER}")
    cursor = 1

    def fail(number, message):
        raise ValueError(f"line {number}: {message}")

    def mapping(depth, sequence=False, root=False):
        nonlocal cursor
        result = [] if sequence else {}
        while cursor < len(rows):
            number, indent, content = rows[cursor]
            if indent < depth:
                break
            if indent != depth:
                fail(number, "unexpected indentation")
            if content.startswith('"'):
                # Decode a JSON string prefix while still rejecting duplicate keys in values.
                try:
                    key, end = json.JSONDecoder().raw_decode(content)
                except ValueError as error:
                    fail(number, f"invalid member name: {error}")
                tail = content[end:]
            else:
                match = re.match(r"([A-Za-z_][A-Za-z0-9_.-]*|[1-9][0-9]*)(.*)\Z", content)
                if not match:
                    fail(number, "expected member name")
                key, tail = match.groups()
            if sequence:
                if key != str(len(result) + 1):
                    fail(number, "transitions must be numbered consecutively from 1")
            elif key in result:
                fail(number, f"duplicate member name: {key}")
            elif not content.startswith('"') and key[0].isdigit():
                fail(number, "numeric member names must be JSON quoted")
            cursor += 1
            if tail == ":":
                if cursor >= len(rows) or rows[cursor][1] != depth + 1:
                    fail(number, "empty block: use = {} or = []")
                value = mapping(depth + 1, sequence=root and key == "transitions")
            elif tail.startswith(" = "):
                try:
                    value = loads(tail[3:])
                    canonical(value)
                except (ValueError, TypeError, OverflowError) as error:
                    fail(number, f"invalid JSON value: {error}")
            else:
                fail(number, "expected ':' or ' = JSON'")
            if sequence:
                if not isinstance(value, dict):
                    fail(number, "transition must be an object")
                result.append(value)
            else:
                result[key] = value
        return result

    result = mapping(0, root=True)
    try:
        _record(result)
    except (ValueError, TypeError) as error:
        fail(rows[0][0], str(error))
    return result


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("input", type=Path)
    parser.add_argument("--parse", action="store_true", help="read text and emit canonical JSON")
    parser.add_argument("--out", type=Path)
    args = parser.parse_args(argv)
    try:
        source = args.input.read_text(encoding="utf-8")
        output = canonical(parse(source)) + "\n" if args.parse else render(loads(source))
        if args.out:
            args.out.write_text(output, encoding="utf-8")
        else:
            sys.stdout.write(output)
    except (ValueError, TypeError, OSError) as error:
        print(f"mgn render: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
