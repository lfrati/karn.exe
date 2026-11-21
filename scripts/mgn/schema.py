"""A JSON Schema (draft 2020-12) interpreter for the keyword subset the MGN schemas use.

It exists so that the schema file is the one structural definition and the validator needs
no third-party package. Supported: type, enum, const, properties, patternProperties,
additionalProperties, required, minProperties, maxProperties, items, prefixItems,
minItems, maxItems, uniqueItems, minimum, maximum, exclusiveMinimum, exclusiveMaximum,
minLength, maxLength, pattern, allOf, anyOf, oneOf, not, if/then/else, $ref to "#/$defs/..."
and boolean schemas. References may also name a sibling schema file in SCHEMA_DIR. The test suite cross-checks it against the `jsonschema` package when
that is installed. Errors carry the instance path as a JSON pointer.
"""
from __future__ import annotations

from dataclasses import dataclass
import json
from pathlib import Path
import re

SCHEMA_DIR = Path(__file__).resolve().parents[2] / "docs/mgn/schema"


@dataclass(frozen=True)
class SchemaError:
    path: str  # JSON pointer into the instance
    keyword: str
    message: str

    def __str__(self):
        return f"{self.path or '/'}: {self.message} [{self.keyword}]"


def load_schema(name: str) -> dict:
    return json.loads((SCHEMA_DIR / name).read_text(encoding="utf-8"))


def _type_of(value) -> set[str]:
    if value is None:
        return {"null"}
    if isinstance(value, bool):
        return {"boolean"}
    if isinstance(value, int):
        return {"integer", "number"}
    if isinstance(value, float):
        return {"number", "integer"} if value == int(value) else {"number"}
    if isinstance(value, str):
        return {"string"}
    if isinstance(value, list):
        return {"array"}
    if isinstance(value, dict):
        return {"object"}
    raise TypeError(type(value).__name__)


def _equal(a, b) -> bool:
    """JSON equality: 1 == 1.0, true != 1, objects by members, arrays item by item."""
    if isinstance(a, bool) or isinstance(b, bool):
        return isinstance(a, bool) and isinstance(b, bool) and a == b
    if isinstance(a, (int, float)) and isinstance(b, (int, float)):
        return a == b
    if isinstance(a, dict) and isinstance(b, dict):
        return a.keys() == b.keys() and all(_equal(a[k], b[k]) for k in a)
    if isinstance(a, list) and isinstance(b, list):
        return len(a) == len(b) and all(_equal(x, y) for x, y in zip(a, b))
    return type(a) is type(b) and a == b


def _short(value) -> str:
    text = json.dumps(value, ensure_ascii=False)
    return text if len(text) <= 60 else text[:57] + "..."


class Validator:
    def __init__(self, schema: dict):
        self.root = schema

    def resolve(self, ref: str) -> dict | bool:
        if not ref.startswith("#/"):
            raise ValueError(f"only local references are supported: {ref}")
        node = self.root
        for part in ref[2:].split("/"):
            part = part.replace("~1", "/").replace("~0", "~")
            node = node[part]
        return node

    def errors(self, instance, schema=None, path: str = "") -> list[SchemaError]:
        if schema is None:
            schema = self.root
        if schema is True:
            return []
        if schema is False:
            return [SchemaError(path, "false", "no value is allowed here")]
        errors: list[SchemaError] = []
        if "$ref" in schema:
            ref = schema["$ref"]
            if ref.startswith("#/"):
                errors.extend(self.errors(instance, self.resolve(ref), path))
            else:
                # Shared protocol projections use the record's structural
                # definition directly; schema references never access a network.
                filename, fragment = ref.split("#", 1)
                if Path(filename).name != filename or not filename.endswith(".schema.json"):
                    raise ValueError(f"only sibling schema files are supported: {ref}")
                other = Validator(load_schema(filename))
                errors.extend(other.errors(instance, other.resolve("#" + fragment), path))
        if "type" in schema:
            wanted = schema["type"] if isinstance(schema["type"], list) else [schema["type"]]
            if not _type_of(instance) & set(wanted):
                errors.append(SchemaError(path, "type", f"expected {' or '.join(wanted)}, got {_short(instance)}"))
                return errors  # the other keywords assume the type
        if "enum" in schema and not any(_equal(instance, item) for item in schema["enum"]):
            errors.append(SchemaError(path, "enum", f"{_short(instance)} is not one of {_short(schema['enum'])}"))
        if "const" in schema and not _equal(instance, schema["const"]):
            errors.append(SchemaError(path, "const", f"expected {_short(schema['const'])}, got {_short(instance)}"))
        if isinstance(instance, (int, float)) and not isinstance(instance, bool):
            if "minimum" in schema and instance < schema["minimum"]:
                errors.append(SchemaError(path, "minimum", f"{instance} is below the minimum {schema['minimum']}"))
            if "maximum" in schema and instance > schema["maximum"]:
                errors.append(SchemaError(path, "maximum", f"{instance} is above the maximum {schema['maximum']}"))
            if "exclusiveMinimum" in schema and instance <= schema["exclusiveMinimum"]:
                errors.append(SchemaError(path, "exclusiveMinimum", f"{instance} must be greater than {schema['exclusiveMinimum']}"))
            if "exclusiveMaximum" in schema and instance >= schema["exclusiveMaximum"]:
                errors.append(SchemaError(path, "exclusiveMaximum", f"{instance} must be less than {schema['exclusiveMaximum']}"))
        if isinstance(instance, str):
            if "minLength" in schema and len(instance) < schema["minLength"]:
                errors.append(SchemaError(path, "minLength", f"shorter than {schema['minLength']} characters"))
            if "maxLength" in schema and len(instance) > schema["maxLength"]:
                errors.append(SchemaError(path, "maxLength", f"longer than {schema['maxLength']} characters"))
            if "pattern" in schema and not re.search(schema["pattern"], instance):
                errors.append(SchemaError(path, "pattern", f"{_short(instance)} does not match {schema['pattern']}"))
        if isinstance(instance, list):
            if "minItems" in schema and len(instance) < schema["minItems"]:
                errors.append(SchemaError(path, "minItems", f"fewer than {schema['minItems']} items"))
            if "maxItems" in schema and len(instance) > schema["maxItems"]:
                errors.append(SchemaError(path, "maxItems", f"more than {schema['maxItems']} items"))
            if schema.get("uniqueItems"):
                for i, item in enumerate(instance):
                    if any(_equal(item, other) for other in instance[:i]):
                        errors.append(SchemaError(f"{path}/{i}", "uniqueItems", f"duplicate item {_short(item)}"))
            prefix = schema.get("prefixItems", [])
            for i, (item, sub) in enumerate(zip(instance, prefix)):
                errors.extend(self.errors(item, sub, f"{path}/{i}"))
            if "items" in schema:
                for i in range(len(prefix), len(instance)):
                    errors.extend(self.errors(instance[i], schema["items"], f"{path}/{i}"))
        if isinstance(instance, dict):
            for name in schema.get("required", []):
                if name not in instance:
                    errors.append(SchemaError(path, "required", f"missing required member {name!r}"))
            if "minProperties" in schema and len(instance) < schema["minProperties"]:
                errors.append(SchemaError(path, "minProperties", f"fewer than {schema['minProperties']} members"))
            if "maxProperties" in schema and len(instance) > schema["maxProperties"]:
                errors.append(SchemaError(path, "maxProperties", f"more than {schema['maxProperties']} members"))
            properties = schema.get("properties", {})
            patterns = schema.get("patternProperties", {})
            for name, value in instance.items():
                child = f"{path}/{name.replace('~', '~0').replace('/', '~1')}"
                matched = False
                if name in properties:
                    matched = True
                    errors.extend(self.errors(value, properties[name], child))
                for pattern, sub in patterns.items():
                    if re.search(pattern, name):
                        matched = True
                        errors.extend(self.errors(value, sub, child))
                if not matched and "additionalProperties" in schema:
                    extra = schema["additionalProperties"]
                    if extra is False:
                        allowed = sorted(properties)
                        hint = f"; allowed: {', '.join(allowed)}" if allowed else ""
                        errors.append(SchemaError(child, "additionalProperties", f"unknown member {name!r}{hint}"))
                    else:
                        errors.extend(self.errors(value, extra, child))
        for sub in schema.get("allOf", []):
            errors.extend(self.errors(instance, sub, path))
        if "anyOf" in schema:
            branches = [self.errors(instance, sub, path) for sub in schema["anyOf"]]
            if all(branches):
                errors.extend(self._combine(instance, schema["anyOf"], branches, path, "anyOf"))
        if "oneOf" in schema:
            branches = [self.errors(instance, sub, path) for sub in schema["oneOf"]]
            passing = [i for i, b in enumerate(branches) if not b]
            if len(passing) != 1:
                if not passing:
                    errors.extend(self._combine(instance, schema["oneOf"], branches, path, "oneOf"))
                else:
                    errors.append(SchemaError(path, "oneOf", f"matches {len(passing)} alternatives, expected exactly one"))
        if "not" in schema and not self.errors(instance, schema["not"], path):
            errors.append(SchemaError(path, "not", "matches a schema it must not match"))
        if "if" in schema:
            if not self.errors(instance, schema["if"], path):
                if "then" in schema:
                    errors.extend(self.errors(instance, schema["then"], path))
            elif "else" in schema:
                errors.extend(self.errors(instance, schema["else"], path))
        return errors

    def _combine(self, instance, subschemas, branches, path, keyword) -> list[SchemaError]:
        """When every alternative fails: the alternative selected by a `kind` discriminator, else the closest."""
        kinds = {}
        for i, sub in enumerate(subschemas):
            resolved = self._deref(sub)
            kind = self._discriminator(resolved)
            if kind is not None:
                kinds.setdefault(kind, i)
        if kinds and isinstance(instance, dict):
            kind = instance.get("kind")
            if kind in kinds:
                return branches[kinds[kind]]
            names = ", ".join(sorted(kinds))
            return [SchemaError(f"{path}/kind", keyword, f"unknown kind {_short(kind)}; one of: {names}")]
        best = min(branches, key=len)
        return best

    def _deref(self, sub):
        if isinstance(sub, dict) and "$ref" in sub and len(sub) == 1:
            return self.resolve(sub["$ref"])
        if isinstance(sub, dict) and "allOf" in sub:
            merged = {}
            for part in sub["allOf"]:
                part = self._deref(part)
                if isinstance(part, dict):
                    for key, value in part.items():
                        if key == "properties":
                            merged.setdefault("properties", {}).update(value)
                        else:
                            merged.setdefault(key, value)
            return merged
        return sub

    def _discriminator(self, schema):
        if not isinstance(schema, dict):
            return None
        kind = schema.get("properties", {}).get("kind")
        if isinstance(kind, dict) and "const" in kind:
            return kind["const"]
        return None


def validate(instance, schema_name: str = "mgn-1.schema.json") -> list[SchemaError]:
    return Validator(load_schema(schema_name)).errors(instance)
