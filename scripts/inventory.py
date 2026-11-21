"""Inventory of everything the Oracle parser has to cover, from the Comprehensive Rules (closed lists)
and the card corpus (what actually occurs, and how often).

    uv run python scripts/inventory.py

Reads the pinned rules (scripts/pinned.py) and resources/oracle/cards.jsonl (scripts/fetch_resources.py); writes
.cache/research/oracle-inventory.md and .cache/oracle/inventory.json.
"""

from __future__ import annotations

import collections
import json
import re
from pathlib import Path

import pinned

ROOT = Path(__file__).resolve().parent.parent
CR_FILE = pinned.RULES
CARDS = ROOT / "resources/oracle/cards.jsonl"


# ---------------------------------------------------------------- rules

def cr_lines() -> list[str]:
    return CR_FILE.read_text(encoding="utf-8").splitlines()


def cr_rule(lines: list[str], number: str) -> str:
    for line in lines:
        if line.startswith(number + " "):
            return line[len(number) + 1:]
    raise KeyError(number)


def listed(text: str, after: str) -> list[str]:
    """The comma-separated list that follows `after` in a rule, up to the end of its sentence."""
    tail = text.split(after, 1)[1]
    tail = re.split(r"\.(?:\s|$)", tail, maxsplit=1)[0]
    tail = re.sub(r"\s*\(see rule[^)]*\)", "", tail)
    items = re.split(r",\s*(?:and\s+)?|\s+and\s+", tail)
    return [i.strip().strip("“”\"") for i in items if i.strip()]


def numbered_headings(lines: list[str], section: int) -> list[str]:
    """Entries like '702.9. Flying' in a section (the first entry is the section's general rule)."""
    out = []
    for line in lines:
        m = re.match(rf"^{section}\.(\d+)\. (.+)$", line)
        if m and int(m.group(1)) > 1:
            # "702.145. Daybound and Nightbound" heads two keywords; "701.26. Tap and Untap" two actions
            out.extend(h.strip() for h in m.group(2).split(" and "))
    return list(dict.fromkeys(out))


def keyword_variants(lines: list[str]) -> list[str]:
    """Named variants a 702 section defines ("702.33c Multikicker is a variant of the kicker
    ability"); variants with a parameter in their name ("Hexproof from [quality]", "[Type]cycling")
    are read as their base keyword with that parameter."""
    out = []
    for line in lines:
        m = re.match(r"^702\.\d+[a-z] (.+?) is a variant of ", line)
        if m and not m.group(1).startswith("“") and m.group(1) != "Typecycling" and "[" not in m.group(1):
            out.append(m.group(1))
    return list(dict.fromkeys(out))


def keywords_with_a_cost(lines: list[str]) -> list[str]:
    """The keywords (and named variants) whose own 702 section gives them a cost: written with
    one ("Equip [cost]", "Suspend N—[cost]", "Splice onto [quality] [cost]") or naming it
    ("the crew cost of a Vehicle", "a foretell cost"). What "its echo cost" can refer to."""
    variants = keyword_variants(lines)
    out = []
    for line in lines:
        m = re.match(r"^702\.(\d+)\. (.+)$", line)
        if not m or int(m.group(1)) <= 1:
            continue
        n = m.group(1)
        text = " ".join(l for l in lines if re.match(rf"^702\.{n}[a-z] ", l))
        named = [h.strip() for h in m.group(2).split(" and ")]
        named += [v for v in variants if re.search(rf"^702\.{n}[a-z] {re.escape(v)} is a variant of ", "\n".join(lines), re.M)]
        for k in named:
            if re.search(re.escape(k) + r"(?: [^.”]*?)?[\s—]\[cost\]|" + re.escape(k) + r" costs?\b", text, re.I):
                out.append(k)
    return list(dict.fromkeys(out))


def rules_inventory() -> dict:
    lines = cr_lines()
    inv = {
        "source": CR_FILE.name,
        "card types": listed(cr_rule(lines, "205.2a"), "The card types are "),
        "supertypes": listed(cr_rule(lines, "205.4a"), "The supertypes are "),
        "artifact types": listed(cr_rule(lines, "205.3g"), "The artifact types are "),
        "enchantment types": listed(cr_rule(lines, "205.3h"), "The enchantment types are "),
        "land types": listed(cr_rule(lines, "205.3i"), "The land types are "),
        "planeswalker types": listed(cr_rule(lines, "205.3j"), "The planeswalker types are "),
        "spell types": listed(cr_rule(lines, "205.3k"), "The spell types are "),
        "creature types": ["Time Lord"] + listed(cr_rule(lines, "205.3m"), "All other creature types are one word long: "),
        "battle types": ["Siege"],
        "ability words": listed(cr_rule(lines, "207.2c"), "The ability words are "),
        "keyword abilities (702)": numbered_headings(lines, 702) + keyword_variants(lines),
        "keyword abilities with a cost (702)": keywords_with_a_cost(lines),
        "keyword actions (701)": numbered_headings(lines, 701),
        "turn structure (500-514)": [l.split(". ", 1)[1] for l in lines if re.match(r"^5(0[1-9]|1[0-4])\. ", l)],
        "special card frames and mechanics (703-729)": [l.split(". ", 1)[1] for l in lines if re.match(r"^7(0[3-9]|[12]\d)\. ", l)],
        "zones (4xx)": [l.split(". ", 1)[1] for l in lines if re.match(r"^40[1-8]\. ", l)],
    }
    # the rules index lines appear twice (table of contents and body): keep unique, in order
    for k, v in inv.items():
        if isinstance(v, list):
            inv[k] = list(dict.fromkeys(v))
    return inv


# ---------------------------------------------------------------- corpus

REMINDER = re.compile(r"\s*\([^()]*\)")
MANA = re.compile(r"\{[^}]+\}")


def faces() -> list[dict]:
    defaults = {"face": 0, "layout": "normal", "oracle_text": "", "mana_cost": "", "keywords": []}
    return [defaults | json.loads(line) for line in CARDS.read_text(encoding="utf-8").splitlines() if line.strip()]


def normalize(text: str, name: str) -> str:
    """Card-independent template: own name -> CARDNAME, numbers -> N."""
    for n in {name, name.split(",")[0]} if name else ():
        if n:
            text = text.replace(n, "CARDNAME")
    text = re.sub(r"(?<![{\w])\d+(?![}\w])", "N", text)
    return text


def classify(line: str, keywords: set[str]) -> str:
    l = line.strip()
    if re.match(r"^[•▪]", l):
        return "mode (bullet)"
    if re.match(r"^[+−-]?(\d+|X):", l):
        return "loyalty ability"
    if re.match(r"^(I|II|III|IV|V|VI)(, (I|II|III|IV|V|VI))* —", l):
        return "saga chapter"
    if re.match(r"^LEVEL \d", l) or re.match(r"^Level up", l):
        return "leveler"
    if re.match(r"^\{[^}]+\}(\{[^}]+\})*: Level \d", l):
        return "class level"
    m = re.match(r"^([A-Z][\w' -]+) — ", l)
    prefix_ability_word = m and m.group(1).lower() in ABILITY_WORDS
    body = l.split(" — ", 1)[1] if prefix_ability_word else l
    if re.match(r"^Choose (one|two|three|four|five|one or more|one or both|any number|up to)\b", body):
        kind = "modal (Choose ...)"
    elif re.match(r"^(When|Whenever|At)\b", body):
        kind = "triggered"
    elif re.search(r"^[^\"]*?:\s", body) and not body.startswith(("As ", "If ")):
        kind = "activated"
    elif all(part.strip().split(" ")[0].lower() in keywords or part.strip().lower() in keywords
             for part in re.split(r",|;", body) if part.strip()):
        kind = "keyword line"
    else:
        kind = "static or spell text"
    return f"{kind} (ability word)" if prefix_ability_word else kind


ABILITY_WORDS: set[str] = set()


def corpus_inventory(rules: dict) -> dict:
    global ABILITY_WORDS
    ABILITY_WORDS = {w.lower() for w in rules["ability words"]}
    fs = faces()
    keywords = {k.lower() for k in rules["keyword abilities (702)"]}
    line_kinds = collections.Counter()
    costs = collections.Counter()
    triggers = collections.Counter()
    trigger_words = collections.Counter()
    counters = collections.Counter()
    type_words = collections.Counter()
    sf_keywords = collections.Counter()
    layouts = collections.Counter()
    examples: dict[str, str] = {}
    n_lines = 0
    for f in fs:
        layouts[f["layout"]] += 1
        for k in set(f["keywords"]):
            sf_keywords[k] += 1
        for word in re.split(r"[\s—]+", f["type_line"]):
            if word:
                type_words[word] += 1
        text = REMINDER.sub("", f["oracle_text"])
        for m in re.finditer(r"\b([\w+\-/]+(?: [\w+\-/]+)?) counters?\b", text):
            w = m.group(1)
            w = w.split(" ")[-1] if not w.startswith(("+", "-", "−")) else w
            if w.lower() not in {"a", "an", "the", "of", "those", "that", "with", "no", "more", "any", "N", "X", "two", "three", "four", "five", "six"}:
                counters[w] += 1
        for line in text.split("\n"):
            line = line.strip()
            if not line:
                continue
            n_lines += 1
            kind = classify(line, keywords)
            line_kinds[kind] += 1
            examples.setdefault(kind, f'{f["face_name"]}: {line}')
            body = line.split(" — ", 1)[1] if re.match(r"^[A-Z][\w' -]+ — ", line) else line
            if kind.startswith("activated") or kind == "loyalty ability":
                cost = body.split(":", 1)[0]
                for part in re.split(r",\s*", cost):
                    costs[normalize(part.strip(), f["face_name"])] += 1
            if kind.startswith("triggered"):
                m = re.match(r"^(When|Whenever|At)\s+(.*?)(,|$)", body)
                if m:
                    trigger_words[m.group(1)] += 1
                    triggers[normalize(m.group(2), f["face_name"])] += 1

    known_types = {t.lower() for k in ("card types", "supertypes", "artifact types", "enchantment types", "land types",
                                       "planeswalker types", "spell types", "creature types", "battle types")
                   for t in rules[k]} | {"time", "lord", "—", "//"}
    unknown_types = {w: n for w, n in type_words.items() if w.lower().strip("’'") not in known_types}
    unknown_keywords = {k: n for k, n in sf_keywords.items()
                        if k.lower() not in keywords and k.lower() not in {a.lower() for a in rules["keyword actions (701)"]}
                        and k.lower() not in ABILITY_WORDS}
    return {
        "faces": len(fs),
        "cards": len({f["oracle_id"] for f in fs}),
        "ability lines": n_lines,
        "layouts": dict(layouts.most_common()),
        "line kinds": dict(line_kinds.most_common()),
        "line kind examples": examples,
        "cost components": dict(costs.most_common()),
        "trigger words": dict(trigger_words.most_common()),
        "trigger conditions": dict(triggers.most_common()),
        "counter kinds": dict(counters.most_common()),
        "scryfall keywords": dict(sf_keywords.most_common()),
        "type-line words not in the rules lists": dict(sorted(unknown_types.items(), key=lambda kv: -kv[1])),
        "scryfall keywords not in 701/702/207.2c": dict(sorted(unknown_keywords.items(), key=lambda kv: -kv[1])),
    }


# ---------------------------------------------------------------- report

def top(d: dict, n: int) -> str:
    return "\n".join(f"| {k} | {v} |" for k, v in list(d.items())[:n])


def main() -> None:
    rules = rules_inventory()
    corpus = corpus_inventory(rules)
    (ROOT / ".cache/oracle").mkdir(parents=True, exist_ok=True)
    (ROOT / ".cache/research").mkdir(parents=True, exist_ok=True)
    (ROOT / ".cache/oracle/inventory.json").write_text(json.dumps({"rules": rules, "corpus": corpus}, indent=1, ensure_ascii=False))
    # the closed lists the Rust `ir` crate embeds (committed: derived from the CR, not from the corpus)
    lists = {k: v for k, v in rules.items() if k != "source"}
    (ROOT / "crates/ir/data/cr_lists.json").write_text(json.dumps(lists, indent=1, ensure_ascii=False) + "\n")
    out = [f"# Oracle parser inventory\n",
           f"Generated by `scripts/inventory.py` from `{rules['source']}` and Scryfall's Oracle Cards "
           f"({corpus['cards']} cards, {corpus['faces']} faces, {corpus['ability lines']} ability lines after "
           f"removing reminder text). Full counts in `.cache/oracle/inventory.json`.\n",
           "## 1. Closed lists from the Comprehensive Rules\n",
           "Everything in these lists must be recognized; new sets extend them (rerun after a rules update).\n"]
    for k, v in rules.items():
        if isinstance(v, list):
            out.append(f"**{k}** ({len(v)}): " + ", ".join(v) + "\n")
    out += ["## 2. What the cards actually say\n",
            "### Ability lines by kind\n", "| kind | lines |", "|---|---|", top(corpus["line kinds"], 50), "",
            "Examples:\n"] + [f"- *{k}*: {v}" for k, v in corpus["line kind examples"].items()] + ["",
            "### Layouts (card frames)\n", "| layout | faces |", "|---|---|", top(corpus["layouts"], 50), "",
            f"### Activated-ability cost components ({len(corpus['cost components'])} distinct templates; top 60)\n",
            "| component | count |", "|---|---|", top(corpus["cost components"], 60), "",
            f"### Trigger conditions ({len(corpus['trigger conditions'])} distinct templates; top 60)\n",
            "Words: " + ", ".join(f"{k} {v}" for k, v in corpus["trigger words"].items()) + "\n",
            "| condition | count |", "|---|---|", top(corpus["trigger conditions"], 60), "",
            f"### Counter kinds ({len(corpus['counter kinds'])})\n",
            ", ".join(f"{k} {v}" for k, v in corpus["counter kinds"].items()) + "\n",
            f"### Keywords as tagged by Scryfall ({len(corpus['scryfall keywords'])})\n",
            ", ".join(f"{k} {v}" for k, v in corpus["scryfall keywords"].items()) + "\n",
            "### Cross-checks\n",
            "Type-line words not in the rules' type lists (should be empty apart from punctuation and two-word names): "
            + (", ".join(f"{k} {v}" for k, v in corpus["type-line words not in the rules lists"].items()) or "none") + "\n",
            "Scryfall keywords that are not keyword abilities, keyword actions or ability words: "
            + (", ".join(f"{k} {v}" for k, v in corpus["scryfall keywords not in 701/702/207.2c"].items()) or "none") + "\n"]
    (ROOT / ".cache/research/oracle-inventory.md").write_text("\n".join(out), encoding="utf-8")
    print(f"{corpus['cards']} cards, {corpus['ability lines']} ability lines")
    for k in ("line kinds",):
        print(k, corpus[k])
    print({k: len(v) for k, v in rules.items() if isinstance(v, list)})
    print("distinct cost templates", len(corpus["cost components"]), "| trigger templates", len(corpus["trigger conditions"]),
          "| counter kinds", len(corpus["counter kinds"]))
    print("unknown type words:", list(corpus["type-line words not in the rules lists"].items())[:20])
    print("unknown keywords:", list(corpus["scryfall keywords not in 701/702/207.2c"].items())[:20])


if __name__ == "__main__":
    main()
