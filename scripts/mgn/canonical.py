"""RFC 8785 JSON Canonicalization Scheme, and the SHA-256 identities MGN defines over it.

Canonical form: no whitespace; object members sorted by their names as arrays of UTF-16 code
units; strings escaped only where RFC 8785 section 3.2.2.2 requires (control characters, the
quote and the backslash), everything else kept as is; numbers serialized as ECMAScript's
Number::toString (section 3.2.2.3); NaN and infinities refused.
"""
from __future__ import annotations

import hashlib
import json
import math

_SHORTHAND = {"\b": "\\b", "\t": "\\t", "\n": "\\n", "\f": "\\f", "\r": "\\r", '"': '\\"', "\\": "\\\\"}


def _string(value: str) -> str:
    out = ['"']
    for char in value:
        if char in _SHORTHAND:
            out.append(_SHORTHAND[char])
        elif ord(char) < 0x20:
            out.append(f"\\u{ord(char):04x}")
        elif 0xD800 <= ord(char) <= 0xDFFF:
            raise ValueError("lone surrogate in string")
        else:
            out.append(char)
    out.append('"')
    return "".join(out)


def es_number(value: float) -> str:
    """ECMAScript Number::toString of a finite double (ECMA-262 7.1.12.1, as RFC 8785 requires)."""
    if math.isnan(value) or math.isinf(value):
        raise ValueError("NaN and infinities have no canonical JSON form")
    if value == 0:
        return "0"
    sign = "-" if value < 0 else ""
    text = repr(abs(value))  # the shortest digit string that round-trips, as Number::toString
    mantissa, _, exponent = text.partition("e")
    exponent = int(exponent) if exponent else 0
    integer, _, fraction = mantissa.partition(".")
    digits = (integer + fraction).lstrip("0")
    n = exponent - len(fraction) + len(digits)  # value = digits * 10 ** (n - k)
    digits = digits.rstrip("0")
    k = len(digits)
    if k <= n <= 21:
        return sign + digits + "0" * (n - k)
    if 0 < n <= 21:
        return sign + digits[:n] + "." + digits[n:]
    if -6 < n <= 0:
        return sign + "0." + "0" * (-n) + digits
    e = n - 1
    exp = ("+" if e >= 0 else "-") + str(abs(e))
    if k == 1:
        return sign + digits + "e" + exp
    return sign + digits[0] + "." + digits[1:] + "e" + exp


def _number(value) -> str:
    if isinstance(value, bool):
        raise TypeError("bool is not a number")
    if isinstance(value, int):
        if abs(value) > 2 ** 53:
            raise ValueError("integer beyond 2**53 has no exact double; store it as a string")
        return str(value)
    return es_number(float(value))


def _sort_key(name: str) -> bytes:
    # UTF-16 big-endian bytes compare as the sequence of UTF-16 code units, unsigned
    return name.encode("utf-16-be", "surrogatepass")


def canonical(value) -> str:
    """The RFC 8785 canonical JSON text of a JSON value."""
    if value is None:
        return "null"
    if value is True:
        return "true"
    if value is False:
        return "false"
    if isinstance(value, str):
        return _string(value)
    if isinstance(value, (int, float)):
        return _number(value)
    if isinstance(value, (list, tuple)):
        return "[" + ",".join(canonical(item) for item in value) + "]"
    if isinstance(value, dict):
        for key in value:
            if not isinstance(key, str):
                raise TypeError("object member names must be strings")
        members = sorted(value.items(), key=lambda item: _sort_key(item[0]))
        return "{" + ",".join(_string(key) + ":" + canonical(item) for key, item in members) + "}"
    raise TypeError(f"not a JSON value: {type(value).__name__}")


def canonical_bytes(value) -> bytes:
    return canonical(value).encode("utf-8")


def sha256(value) -> str:
    """The lowercase hex SHA-256 of the canonical form."""
    return hashlib.sha256(canonical_bytes(value)).hexdigest()


def loads(text: str):
    """Read JSON without duplicate names or silent rounding of large integer inputs.

    Large integer tokens are doubles only when exactly representable, or when
    they already spell that double's canonical text (RFC 8785 appendix B includes
    integer spellings that are not the double's exact mathematical integer).
    Direct canonical(int) callers retain the explicit safe-integer contract.
    """
    def integer(text):
        value = int(text)
        if abs(value) <= 2 ** 53:
            return value
        number = float(text)
        if math.isfinite(number) and (int(number) == value or es_number(number) == text):
            return number
        raise ValueError("integer token cannot be represented without precision loss; store it as a string")

    def no_duplicates(pairs):
        out = {}
        for key, item in pairs:
            if key in out:
                raise ValueError(f"duplicate member name: {key}")
            out[key] = item
        return out
    return json.loads(text, object_pairs_hook=no_duplicates, parse_int=integer)


def same(left, right):
    """JSON equality without Python's false == 0 and true == 1 coercions."""
    if type(left) is not type(right):
        return False
    if isinstance(left, dict):
        return left.keys() == right.keys() and all(same(left[key], right[key]) for key in left)
    if isinstance(left, list):
        return len(left) == len(right) and all(same(a, b) for a, b in zip(left, right))
    return left == right
