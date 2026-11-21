"""The worked examples: loading the positive records and building the negative cases.

A negative case is a patch (RFC 6902 add, replace, remove, move) on a positive example with the
validator code it must produce, so that each file shows exactly one mistake.
"""
from __future__ import annotations

from copy import deepcopy
import json
from pathlib import Path

from .canonical import loads

EXAMPLES = Path(__file__).resolve().parents[2] / "docs/mgn/examples"


def positive_paths() -> list[Path]:
    return sorted(path for path in EXAMPLES.glob("*.json"))


def load(path: Path) -> dict:
    return loads(Path(path).read_text(encoding="utf-8"))


def _pointer(path: str) -> list:
    if path == "":
        return []
    if not path.startswith("/"):
        raise ValueError(f"not a JSON pointer: {path}")
    return [part.replace("~1", "/").replace("~0", "~") for part in path[1:].split("/")]


def _walk(document, parts: list):
    node = document
    for part in parts:
        node = node[int(part)] if isinstance(node, list) else node[part]
    return node


def _remove(document, parts: list):
    parent = _walk(document, parts[:-1])
    key = parts[-1]
    if isinstance(parent, list):
        return parent.pop(int(key))
    return parent.pop(key)


def _add(document, parts: list, value):
    parent = _walk(document, parts[:-1])
    key = parts[-1]
    if isinstance(parent, list):
        parent.insert(len(parent) if key == "-" else int(key), value)
    else:
        parent[key] = value


def apply_patch(document, operations: list) -> dict:
    document = deepcopy(document)
    for operation in operations:
        op = operation["op"]
        parts = _pointer(operation["path"])
        if op == "add":
            _add(document, parts, deepcopy(operation["value"]))
        elif op == "replace":
            _remove(document, parts)
            _add(document, parts, deepcopy(operation["value"]))
        elif op == "remove":
            _remove(document, parts)
        elif op == "move":
            value = _remove(document, _pointer(operation["from"]))
            _add(document, parts, value)
        else:
            raise ValueError(f"unsupported patch operation: {op}")
    return document


def negative_cases() -> list[dict]:
    """Each case with its patched record under `record`."""
    cases = json.loads((EXAMPLES / "negative/cases.json").read_text(encoding="utf-8"))
    out = []
    for case in cases:
        base = load(EXAMPLES / case["base"])
        out.append({**case, "record": apply_patch(base, case["patch"])})
    return out
