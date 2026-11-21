"""Token definitions and occurrence distinctions used by the current MGN adapter."""
import json
import re
from . import vocabulary
from .keywords import keyword_row

COLOR_WORDS = ("white", "blue", "black", "red", "green", "colorless")
TYPE_WORDS = ("artifact", "enchantment", "creature", "land", "planeswalker", "battle", "legendary", "basic", "snow")
_PREDEFINED = None

def _split_abilities(text: str) -> tuple[list[str], str | None]:
    """A "with ..." clause of a token: keyword abilities and the quoted rules text."""
    keywords, quoted = [], []
    for match in re.finditer(r"[“\"]([^”\"]+)[”\"]", text):
        quoted.append(match.group(1).strip())
    rest = re.sub(r"[“\"][^”\"]*[”\"]", "", text)
    rest = re.sub(r"^and\s+|\s+and$", "", rest.strip().strip(",").strip())
    # Split only where another keyword starts, preserving selector/cost conjunctions
    # such as "protection from red and from black" (CR 702.16).
    pieces, start = [], 0
    for separator in re.finditer(r",\s*|\s+and\s+", rest):
        if keyword_row(rest[separator.end():]):
            pieces.append(rest[start:separator.start()])
            start = separator.end()
    pieces.append(rest[start:])
    for piece in pieces:
        piece = piece.strip().strip(",").strip()
        if piece:
            keywords.extend(keyword_row(piece))
    return keywords, ("\n".join(quoted) or None)

def _parse_token_description(desc: str) -> dict | None:
    """'3/3 colorless Phyrexian Golem artifact creature' -> definition parts, or None."""
    words = desc.replace(",", " ").split()
    out: dict = {"colors": [], "types": [], "supertypes": [], "subtypes": []}
    i = 0
    if words and re.fullmatch(r"(\d+|X|\*)/(\d+|X|\*)", words[0]):
        power, toughness = words[0].split("/")
        if power.isdigit() and toughness.isdigit():
            out["power"], out["toughness"] = int(power), int(toughness)
        i = 1
    while i < len(words) and words[i] in COLOR_WORDS + ("and",):
        if words[i] not in ("and", "colorless"):
            out["colors"].append(words[i])
        i += 1
    rest = words[i:]
    if not rest:
        return None
    # CR 205.3m has one two-word creature type.
    rest = re.findall(r"Time Lord|\S+", " ".join(rest))
    for word in rest:
        lowered = word.lower()
        if lowered in TYPE_WORDS:
            (out["supertypes"] if lowered in ("legendary", "basic", "snow") else out["types"]).append(lowered)
        elif word[:1].isupper():
            try:
                out["subtypes"].append(vocabulary.canonical_subtype(word))
            except ValueError:
                return None
        else:
            return None
    if not out["types"]:
        return None
    for key in ("colors", "types", "supertypes", "subtypes"):
        out[key] = sorted(out[key])
    return out

def distinguish_instances(instances: list[dict]) -> list[dict]:
    """Trigger instances carry `for` only where two from the same source and ability would
    otherwise be indistinguishable (spec 3.3)."""
    keys = [json.dumps({"source": i.get("source"), "ability": i.get("ability")}, sort_keys=True) for i in instances]
    return [{k: v for k, v in i.items() if k != "for"} if keys.count(key) == 1 else i for key, i in zip(keys, instances)]

def predefined_tokens() -> dict[str, dict]:
    """Definitions of the predefined tokens from rule 111.10 of the pinned rules, by token name
    (the adapter's source for a predefined token's text)."""
    global _PREDEFINED
    if _PREDEFINED is not None:
        return _PREDEFINED
    out = {}
    pattern = re.compile(r"^111\.10[a-z] An? (?P<name>[\w ]+?) token is an? (?P<desc>.*?) tokens?(?: named (?P<named>[^.,“]+?))?(?:,? with (?P<with>.*))?\.?$", re.M)
    for match in pattern.finditer(vocabulary.rules_text()):
        parsed = _parse_token_description(match.group("desc"))
        if parsed is None:
            continue
        name = (match.group("named") or f"{match.group('name')} Token").strip()
        definition = {"name": name, "types": parsed["types"], "colors": parsed["colors"]}
        if parsed["supertypes"]:
            definition["supertypes"] = parsed["supertypes"]
        if parsed["subtypes"]:
            definition["subtypes"] = parsed["subtypes"]
        for key in ("power", "toughness"):
            if key in parsed:
                definition[key] = parsed[key]
        if match.group("with"):
            keywords, text = _split_abilities(re.split(r"\.? See rule", match.group("with"))[0])
            if keywords:
                definition["keywords"] = sorted(keywords)
            if text:
                definition["text"] = text
        out[name] = definition
    _PREDEFINED = out
    return out
