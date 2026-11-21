"""Sub-line structure of Oracle text: clauses, verbs, selectors, amounts, conditions, durations, statics,
replacements, choices and rare constructs. Measures what the IR and the grammar must express.

    uv run python scripts/corpus_structure.py            # writes .cache/research/corpus-structure.json
    uv run python scripts/corpus_structure.py --dump-nps # also writes .cache/oracle/noun_phrases.jsonl
    uv run python scripts/corpus_structure.py --dump-amounts # also writes .cache/oracle/amounts.jsonl
    uv run python scripts/corpus_structure.py --dump-conditions # also writes .cache/oracle/conditions.jsonl
    uv run python scripts/corpus_structure.py --dump-triggers # also writes .cache/oracle/triggers.jsonl
    uv run python scripts/corpus_structure.py --no-report --dump-nps ... # the dumps only, the report untouched
    uv run python scripts/corpus_structure.py --tables   # also prints markdown tables

Everything is regex/lexicon based (no POS tagger): Oracle text is templated enough that closed lexicons
plus a small noun-phrase chunker explain almost all of it, and what they do not explain is itself the
measurement (the "unexplained" counts below are the parser worklist).

Builds on scripts/inventory.py (line classification, reminder-text removal, CR closed lists).
"""

from __future__ import annotations

import collections
import json
import re
import sys
from dataclasses import dataclass, field
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import inventory  # noqa: E402

ROOT = Path(__file__).resolve().parent.parent
OUT_JSON = ROOT / ".cache/research/corpus-structure.json"

# ============================================================== preprocessing

RULES = inventory.rules_inventory()
inventory.ABILITY_WORDS = {w.lower() for w in RULES["ability words"]}
KEYWORDS = {k.lower() for k in RULES["keyword abilities (702)"]}

SELF_NOUNS = ("creature", "artifact", "enchantment", "land", "spell", "card", "permanent", "planeswalker",
              "Aura", "Equipment", "Vehicle", "Saga", "Siege", "battle", "Class", "Case", "Room", "token",
              "Fortification", "Spacecraft", "Background", "Shrine", "Curse", "Role", "Attraction", "Contraption",
              "Food", "Clue", "Treasure", "Plan", "Planet", "Desert", "Cave", "Gate", "Town", "Lair", "Sphere",
              "Locus", "Mine", "Tower", "Power-Plant", "Stone", "Book", "Map", "Mount", "Rune", "Cartouche", "Shard")
SELF_RE = re.compile(r"\bthis (?:" + "|".join(re.escape(n) for n in SELF_NOUNS) + r")\b")
QUOTE_RE = re.compile(r"[\"“]([^\"”]*)[\"”]")
NUMWORDS = ("one|two|three|four|five|six|seven|eight|nine|ten|eleven|twelve|thirteen|fourteen|fifteen|"
            "twenty|ninety-nine|a hundred")


@dataclass
class Ability:
    card: str            # oracle_id
    name: str            # face name
    kind: str            # inventory.classify kind, plus 'quoted' for abilities granted in quotes
    raw: str             # original line (reminder text removed)
    trigger: str = ""    # trigger condition (triggered abilities)
    cost: str = ""       # cost (activated / loyalty)
    effect: str = ""     # the effect text, self-reference -> SELF, quotes -> QUOTED
    is_spell: bool = False  # line belongs to an instant/sorcery face (spell text, not a static ability)
    quotes: list[str] = field(default_factory=list)


def self_normalize(text: str, name: str) -> str:
    names = {name, name.split(",")[0], name.split(" of ")[0] if "," not in name else ""}
    for n in sorted((n for n in names if n and len(n) > 2), key=len, reverse=True):
        text = re.sub(r"(?<![\w])" + re.escape(n) + r"(?![\w])", "SELF", text)
    text = SELF_RE.sub("SELF", text)
    return text


def split_body(line: str, kind: str) -> tuple[str, str, str]:
    """(trigger, cost, effect) of one ability line."""
    body = line
    m = re.match(r"^([A-Z][\w' -]+) — ", body)
    if m and m.group(1).lower() in inventory.ABILITY_WORDS:
        body = body.split(" — ", 1)[1]
    if kind.startswith("mode") or kind == "spree/pawprint mode":
        body = re.sub(r"^([•▪]|\+ ?(\{[^}]+\})+ —|(\{P\})+ —)\s*", "", body)
        return "", "", body
    if kind.startswith("saga"):
        return "", "", body.split(" — ", 1)[1]
    if kind.startswith("triggered"):
        m = re.match(r"^((?:When|Whenever|At)\b.*?),\s+(.*)$", body)
        if m:
            return m.group(1), "", m.group(2)
        return body, "", ""
    if kind.startswith("activated") or kind == "loyalty ability":
        # the cost ends at the first colon outside quotes
        depth = 0
        for i, ch in enumerate(body):
            if ch in "\"“":
                depth ^= 1
            elif ch == ":" and not depth:
                return "", body[:i], body[i + 1:].strip()
    return "", "", body


def load_abilities() -> tuple[list[dict], list[Ability]]:
    faces = inventory.faces()
    out: list[Ability] = []
    for f in faces:
        text = inventory.REMINDER.sub("", f["oracle_text"])
        spell_face = bool(re.search(r"\b(Instant|Sorcery)\b", f["type_line"]))
        for line in text.split("\n"):
            line = line.strip()
            if not line:
                continue
            kind = inventory.classify(line, KEYWORDS)
            if re.match(r"^(\+ ?(\{[^}]+\})+|(\{P\})+) —", line):
                kind = "spree/pawprint mode"
            if kind.startswith("keyword line"):
                out.append(Ability(f["oracle_id"], f["face_name"], kind, line, effect=""))
                continue
            trig, cost, eff = split_body(line, kind)
            eff_n = self_normalize(eff, f["face_name"])
            quotes = QUOTE_RE.findall(eff_n)
            eff_q = QUOTE_RE.sub("QUOTED", eff_n)
            is_spell = spell_face and kind in ("static or spell text", "static or spell text (ability word)",
                                               "mode (bullet)", "spree/pawprint mode")
            a = Ability(f["oracle_id"], f["face_name"], kind, line, self_normalize(trig, f["face_name"]),
                        self_normalize(cost, f["face_name"]), eff_q, is_spell, quotes)
            out.append(a)
            for q in quotes:  # granted abilities are abilities too
                qk = inventory.classify(q, KEYWORDS)
                qt, qc, qe = split_body(q, qk)
                out.append(Ability(f["oracle_id"], f["face_name"], "quoted " + qk, q, qt, qc,
                                   QUOTE_RE.sub("QUOTED", qe), False, []))
    return faces, out


# ============================================================== sentences and clauses

def sentences(effect: str) -> list[str]:
    s = re.split(r"(?<=[.!?])\s+(?=[A-Z•])", effect.strip())
    return [x.strip() for x in s if x.strip()]


NUM = r"(?:\d+|X|" + NUMWORDS + r")"


# ============================================================== verbs (section 2)

# (label, regex over effect text, primitive family). Regexes match predicate uses (base / 3rd person forms);
# participles ("exiled", "destroyed") are back-references, counted in section 3, not here.
NOT_SEL = r"(?<!spells you )(?<!spell you )(?<!that you )(?<!cards you )(?<!card you )"
VERBS: list[tuple[str, str, str]] = [
    # zone moves
    ("destroy", r"\bdestroys?\b", "ZoneMove"),
    ("exile", r"\bexiles?\b(?! with)(?!d)", "ZoneMove"),
    ("sacrifice", r"\bsacrifices?\b", "ZoneMove"),
    ("discard", r"\bdiscards?\b", "ZoneMove"),
    ("mill", r"\bmills?\b", "ZoneMove"),
    ("return ... to hand", r"\breturns? [^.]*?\bto (?:its|their|your|that player's|his|her)(?: owners?'s?)? hands?", "ZoneMove"),
    ("return ... to battlefield", r"\breturns? [^.]*?\bto the battlefield", "ZoneMove"),
    ("return (other)", r"\breturns?\b(?![^.]*?\bto (?:its|their|your|that player's|his|her)(?: owners?'s?)? hands?)(?![^.]*?\bto the battlefield)", "ZoneMove"),
    ("put ... onto battlefield", r"\bputs? [^.]*?\bonto the battlefield", "ZoneMove"),
    ("put ... into hand", r"\bputs? [^.]*?\binto (?:your|their|its owner's|that player's|his|her|a player's) hands?", "ZoneMove"),
    ("put ... into graveyard", r"\bputs? [^.]*?\binto (?:your|their|its owner's|that player's|a player's|his|her) graveyards?", "ZoneMove"),
    ("put ... on top/bottom of library", r"\bputs? [^.]*?\b(?:on (?:the )?(?:top|bottom) of|into) (?:your|their|its owner's|that player's|a player's|his|her) library", "ZoneMove"),
    ("shuffle ... into library", r"\bshuffles? [^.]*?\binto (?:your|their|its owner's|that player's|his|her) librar", "ZoneMove"),
    ("draw", r"\bdraws?\b(?! step)", "ZoneMove"),
    ("counter (spell/ability)", r"\bcounters? (?:target|it|that|the|all|each|up to|any number|SELF|those|two)\b(?! counters?\b)", "ZoneMove"),
    ("cast / play (effect)", NOT_SEL + r"(?<!you )\b(?:cast|play)s? (?:it|them|that|those|the|a|an|one|SELF|target|up to|any number|cards?|spells?|copies|the copy|instant|sorcery|lands?)\b|\bmay (?:cast|play)\b", "ZoneMove"),
    # objects
    ("create token", r"\bcreates?\b", "Create"),
    ("copy", r"\bcop(?:y|ies) (?:target|it|that|SELF|the|each|this|those|a)\b", "Create"),
    ("investigate/amass/populate/incubate…", r"\b(?:investigates?|amass(?:es)?|populates?|incubates?|forages?|manifests?(?: dread)?|cloaks?|mobilize)\b", "Create"),
    # damage / life
    ("deal damage", r"\bdeals? (?:\d+|X|that much|twice|damage|excess|combat damage|noncombat damage|an amount|half|three times|one)\b", "Damage"),
    ("fight", r"\bfights?\b", "Damage"),
    ("gain life", r"\bgains? (?:\d+|X|that much|life|an amount|twice|half)\b(?:[^.]*?\blife\b)?", "Life"),
    ("lose life", r"\bloses? (?:\d+|X|that much|life|half|an amount|twice)\b", "Life"),
    ("pay life / mana (effect)", r"\bpays? (?:\d+|X|that much|any amount|half|\{)", "Payment"),
    ("life total becomes / exchange", r"life totals? becomes?|exchange life totals|life total is", "Life"),
    # counters
    ("put counters", r"\bputs? (?:a|an|one|two|three|four|five|six|X|\d+|that many|twice|a number of|an additional|any number of)? ?(?:\S+ )?counters? on\b|\bputs? [^.]{0,40}? counters? on\b", "Counters"),
    ("remove counters", r"\bremoves? [^.]*?counters?\b", "Counters"),
    ("move counters", r"\bmoves? [^.]*?counters?\b", "Counters"),
    ("proliferate", r"\bproliferates?\b", "Counters"),
    ("distribute counters", r"\bdistributes?\b", "Counters"),
    ("bolster/adapt/support/monstrosity/endure…", r"\b(?:bolsters?|adapts?|supports?|monstrosity|endures?|evolves?|level up|training|trains?|time travel)\b", "Counters"),
    # status
    ("tap", r"(?<!un)\btaps? (?:target|it|that|all|each|up to|SELF|enchanted|equipped|any number|another|a |an |two|X|those|them|this|the)\b", "Status"),
    ("untap", r"\buntaps? (?:target|it|that|all|each|up to|SELF|enchanted|equipped|any number|another|a |an |two|X|those|them|this|the|those)\b", "Status"),
    ("transform / convert", r"\b(?:transforms?|converts?)\b", "Status"),
    ("turn face up/down", r"\bturns? [^.]*?face (?:up|down)\b", "Status"),
    ("phase out", r"\bphases? (?:out|in)\b", "Status"),
    ("goad / detain / suspect / exert", r"\b(?:goads?|detains?|suspects?|exerts?)\b", "Status"),
    ("regenerate", r"\bregenerates?\b", "Shield"),
    ("attach / unattach", r"\b(?:attach|attaches|unattach)\b", "Status"),
    ("become monarch / initiative / ring / dungeon", r"become the monarch|take the initiative|the Ring tempts you|venture into the dungeon|ventures into", "Designation"),
    # continuous effects created by one-shots
    ("gets +N/+N (pump)", r"\bgets? [+\-−][\dX]+/[+\-−][\dX]+|\bget an additional [+\-]", "Continuous"),
    ("gains/has ability", r"\b(?:gains?|has|have) (?:flying|first strike|double strike|trample|haste|vigilance|lifelink|deathtouch|menace|reach|hexproof|indestructible|shroud|protection|defender|ward|infect|wither|flash|fear|intimidate|islandwalk|swampwalk|forestwalk|mountainwalk|plainswalk|QUOTED|all|your choice|\w+ until end of turn|the chosen|those abilities|banding|shadow|horsemanship|skulk|prowess)", "Continuous"),
    ("loses ability", r"\bloses? (?:all|flying|first strike|trample|hexproof|indestructible|defender|\w+ and \w+|your choice|that ability|all other)\b", "Continuous"),
    ("becomes (type/P-T/copy)", r"\bbecomes? (?:a|an|the|\d|X|colorless|white|blue|black|red|green|artifact|legendary|basic|tapped|untapped|copies|that|your|blocked|attached|the target)\b|\bbecome (?:\d|a |an |artifact|the color|copies|tapped|creature)", "Continuous"),
    ("gain control", r"\b(?:gains?|gain) control\b|\bexchange control\b|\bcontrol of\b", "Control"),
    ("switch power/toughness", r"\bswitch\b", "Continuous"),
    ("double (P/T, counters, life, damage)", r"\bdoubles?\b", "Arithmetic"),
    ("can't / doesn't (restriction as effect)", r"\bcan't\b|\bdoesn't untap\b|\bdon't untap\b", "Continuous"),
    # information
    ("look at", r"\blooks? at\b", "Info"),
    ("reveal", r"\breveals?\b", "Info"),
    ("search", r"\bsearch(?:es)?\b", "Info"),
    ("scry", r"\bscry\b|\bscries\b", "Info"),
    ("surveil", r"\bsurveils?\b", "Info"),
    ("shuffle (alone)", r"\bshuffles?(?:\.|,| your library\.| their library\.)", "Info"),
    ("explore / connive / discover / learn", r"\b(?:explores?|connives?|discovers?|learns?)\b", "Compound"),
    ("cascade-like: exile until", r"\bexiles? cards? from the top of [^.]* until\b|\breveals? cards from the top of [^.]* until\b", "Info"),
    # choice / random
    ("choose (non-modal)", r"\bchooses? (?!one\b|two\b|three\b|one or more\b|one or both\b|any number —|up to (?:one|two|three|four|five) —)", "Choice"),
    ("vote", r"\bvotes?\b", "Choice"),
    ("flip a coin", r"\bflips? (?:a|two|\w+) coins?\b", "Random"),
    ("roll a die", r"\brolls? (?:a|two|\w+) (?:d\d+|(?:six|four|twenty)-sided di(?:e|ce)|dice|die)\b", "Random"),
    ("clash", r"\bclash\b", "Random"),
    ("separate into piles", r"\bseparates? [^.]* piles?\b", "Choice"),
    # mana
    ("add mana", r"(?<!or )\badds? (?:\{|one|two|three|X|an amount|that much|mana|an additional|\w+ mana|that|the|any|\d|B|R|G|W|U)", "Mana"),
    # turn / game
    ("extra turn / combat", r"extra turn|additional combat phase|additional beginning phase|additional upkeep|additional main phase", "Turn"),
    ("skip", r"\bskips?\b", "Turn"),
    ("end the turn", r"\bend the turn\b", "Turn"),
    ("win / lose the game", r"\b(?:wins?|loses?) the game\b", "Game"),
    ("prevent damage", r"\bprevent\b", "Prevention"),
    ("additional land / cast permission", r"play an additional land|play two additional lands|may play lands? from|may cast [^.]* from (?:your graveyard|the top|exile|among)", "Permission"),
    ("assign damage / remove from combat", r"\bassigns?\b|remove [^.]* from combat", "Combat"),
    ("attack / block (forced in effect)", r"\battacks? (?:this turn|each combat|that player|target|you)\b(?:[^.]*if able)|\bblocks? (?:this turn|each combat)\b(?:[^.]*if able)", "Combat"),
    ("exchange", r"\bexchanges?\b(?! control)", "Status"),
    ("note / name / guess / bid", r"\bnotes?\b|\bname a card\b|\bguess(?:es)?\b|\bbids?\b", "Choice"),
    ("activate (effect)", r"\bactivates?\b(?! only)", "Permission"),
    ("repeat this process", r"repeat this process", "Control flow"),
    ("player counters (get {E}, get poison/rad/experience)", r"\bgets? (?:\{E\}|(?:a|an|one|two|three|four|\w+|X|that many) (?:poison|rad|experience|energy|ticket) counters?)", "Counters"),
]
_COVERED_701 = {"activate", "attach", "cast", "counter", "create", "destroy", "discard", "double", "exchange", "exile",
                "fight", "mill", "play", "regenerate", "reveal", "sacrifice", "scry", "search", "shuffle", "surveil",
                "tap and untap", "transform", "convert", "proliferate", "vote", "clash", "goad", "detain", "investigate",
                "populate", "manifest", "bolster", "support", "monstrosity", "adapt", "amass", "learn", "explore",
                "connive", "incubate", "discover", "cloak", "suspect", "forage", "manifest dread", "endure", "exert",
                "venture into the dungeon", "the ring tempts you", "time travel"}
VERBS.append(("other keyword action (701: earthbend, blight, collect evidence…)",
              r"\b(?:" + "|".join(re.escape(k.lower()) + "(?:s|es)?" for k in RULES["keyword actions (701)"]
                                if k.lower() not in _COVERED_701) + r")\b", "Keyword action"))
VERB_RES = [(lbl, re.compile(rx, re.I), fam) for lbl, rx, fam in VERBS]
# verbs that can start a second predicate after "and"/"then": used for clause segmentation
PRED_VERBS = (r"(?:destroy|exile|sacrifice|discard|mill|return|put|shuffle|draw|counter|cast|play|create|cop(?:y|ie)|"
              r"deal|fight|gain|lose|pay|remove|move|proliferate|distribute|tap|untap|transform|turn|phase|goad|"
              r"regenerate|attach|get|ha(?:ve|s)|become|look|reveal|search|scry|surveil|explore|connive|choose|vote|"
              r"flip|roll|add|skip|prevent|investigate|amass|populate|attack|block|can't|doesn't|exchange|"
              r"double|switch|separate|learn|discover|manifest|venture|incubate|bolster|adapt|support|win)(?:s|es)?")


# ============================================================== clause structure (section 1)

MARKERS: dict[str, re.Pattern] = {
    "Then / , then": re.compile(r"(?:^Then\b|, then\b|\bthen\b)"),
    "and + 2nd predicate": re.compile(r"\band (?:then )?(?:(?:you|it|they|that player|SELF|each \w+|target \w+|its controller|that creature) )?(?:may )?" + PRED_VERBS + r"\b"),
    "If ..., (prefix)": re.compile(r"^(?:Then )?[Ii]f\b(?! you do\b| you don't\b| they do\b| that player does\b| no one does\b| a player does\b)"),
    ", if ... (postfix)": re.compile(r"(?<!^)(?<!as though )(?<!only )(?<!even )\bif\b(?! able)(?! you do\b)(?! you don't\b)(?! they do\b)(?! it would)"),
    "If you do / don't (result)": re.compile(r"\b[Ii]f (?:you|they|that player|he|she|no one|a player|any player|an opponent) (?:do|does|don't|doesn't|did|didn't)\b"),
    "When you do (reflexive 603.12)": re.compile(r"\b[Ww]hen(?:ever)? (?:you|they|that player|a player) (?:do|does)\b|\bwhen [^,]* this way\b"),
    "Otherwise": re.compile(r"\b[Oo]therwise\b"),
    "instead": re.compile(r"\binstead\b"),
    "unless": re.compile(r"\bunless\b"),
    "For each ..., (loop)": re.compile(r"^(?:Then )?For each\b|, for each\b[^,.]*,|^(?:Then )?Starting with you\b|^(?:Then )?In turn order\b"),
    "for each (multiplier)": re.compile(r"(?<!^)(?<!, )\bfor each\b"),
    "Each player/opponent <verb> (distributive)": re.compile(r"(?:^|, |then )(?:each|Each) (?:player|opponent|other player|of your opponents|creature|of those|of them)\b"),
    "where X is": re.compile(r"\bwhere X is\b"),
    "you may / may (optional)": re.compile(r"\bmay\b(?! be\b)(?! have\b)(?! only)"),
    "this way (result ref)": re.compile(r"\bthis way\b"),
    "Until/During ... , (duration prefix)": re.compile(r"^(?:Until|During|This turn|For as long as)\b"),
    "repeat this process": re.compile(r"repeat this process"),
}
NESTING = ("If ..., (prefix)", ", if ... (postfix)", "If you do / don't (result)", "When you do (reflexive 603.12)",
           "unless", "For each ..., (loop)", "Each player/opponent <verb> (distributive)", "you may / may (optional)",
           "Otherwise", "instead")


def verb_hits(text: str) -> list[str]:
    hits = []
    for lbl, rx, _ in VERB_RES:
        n = len(rx.findall(text))
        hits += [lbl] * n
    return hits


def clause_stats(abls: list[Ability]) -> dict:
    marker_eff = collections.Counter()
    marker_sent = collections.Counter()
    n_sent_hist = collections.Counter()
    n_verb_hist = collections.Counter()
    depth_hist = collections.Counter()
    shape = collections.Counter()
    examples: dict[str, list[str]] = collections.defaultdict(list)
    depth_ex: dict[int, list[str]] = collections.defaultdict(list)
    n_eff = n_sent = 0
    quote_depth = collections.Counter()
    for a in abls:
        if not a.effect or a.kind.startswith(("modal", "keyword", "leveler", "class level")):
            continue
        n_eff += 1
        ss = sentences(a.effect)
        n_sent += len(ss)
        n_sent_hist[min(len(ss), 6)] += 1
        verbs = verb_hits(a.effect)
        n_verb_hist[min(len(verbs), 8)] += 1
        seen = set()
        depth = 0
        for s in ss:
            d = 1
            for k, rx in MARKERS.items():
                if rx.search(s):
                    marker_sent[k] += 1
                    seen.add(k)
                    if k in NESTING:
                        d += len(rx.findall(s)) if k in ("If ..., (prefix)", ", if ... (postfix)", "unless") else 1
            depth = max(depth, d)
        depth += 1 if len(ss) > 1 else 0
        if a.quotes:
            depth += 1
        depth_hist[min(depth, 7)] += 1
        if len(depth_ex[depth]) < 3 and len(a.raw) < 260:
            depth_ex[depth].append(f"{a.name}: {a.raw}")
        for k in seen:
            marker_eff[k] += 1
            if len(examples[k]) < 2 and len(a.raw) < 200:
                examples[k].append(f"{a.name}: {a.raw}")
        ctl = seen - {"you may / may (optional)", "this way (result ref)"}
        if len(ss) == 1 and len(verbs) <= 1 and not ctl:
            shape["single action" + (" (optional: may)" if "you may / may (optional)" in seen else "")] += 1
        elif len(ss) == 1 and not ctl - {"and + 2nd predicate"}:
            shape["one sentence, 2+ actions joined by and/commas"] += 1
        elif not ctl - {"and + 2nd predicate", "Then / , then"}:
            shape["plain sequence (several sentences / then)"] += 1
        else:
            shape["has control structure (if/unless/for each/instead/...)"] += 1
    return {"effects": n_eff, "sentences": n_sent, "markers by effect": dict(marker_eff.most_common()),
            "markers by sentence": dict(marker_sent.most_common()), "sentences per effect": dict(sorted(n_sent_hist.items())),
            "verbs per effect": dict(sorted(n_verb_hist.items())), "depth": dict(sorted(depth_hist.items())),
            "shape": dict(shape.most_common()), "examples": examples, "depth examples": depth_ex}


# ============================================================== selectors (section 3)

def plural_forms(w: str) -> set[str]:
    special = {"Elf": "Elves", "Dwarf": "Dwarves", "Wolf": "Wolves", "Mouse": "Mice", "Ox": "Oxen", "Child": "Children",
               "Fish": "Fish", "Sheep": "Sheep", "Cyclops": "Cyclopes", "Djinn": "Djinn", "Samurai": "Samurai",
               "Moonfolk": "Moonfolk", "Kithkin": "Kithkin", "Merfolk": "Merfolk", "Kor": "Kor", "sorcery": "sorceries",
               "Sphinx": "Sphinxes", "Phoenix": "Phoenixes", "Lamia": "Lamia", "Octopus": "Octopuses", "Fungus": "Fungi",
               "Nautilus": "Nautiluses", "Homunculus": "Homunculi", "Locus": "Loci", "Thalakos": "Thalakos",
               "Atog": "Atogs", "Goose": "Geese", "Leech": "Leeches", "Witch": "Witches", "Fox": "Foxes"}
    out = {w}
    if w in special:
        out.add(special[w])
    elif w.endswith(("s", "x", "sh", "ch")):
        out.add(w + "es")
    elif w.endswith("y") and w[-2:-1] not in "aeiou":
        out.add(w[:-1] + "ies")
    else:
        out.add(w + "s")
    return out


CARD_TYPES = [t for t in RULES["card types"] if t not in ("conspiracy", "dungeon", "phenomenon", "plane", "scheme", "vanguard")]
SUPERTYPES = ["basic", "legendary", "snow", "world"]
SUBTYPES = sorted({t for k in ("artifact types", "enchantment types", "land types", "planeswalker types", "spell types",
                               "creature types", "battle types") for t in RULES[k]}, key=len, reverse=True)
COLORS = ["white", "blue", "black", "red", "green", "colorless", "multicolored", "monocolored"]
TYPE_WORDS = sorted({f for t in CARD_TYPES + [c.capitalize() for c in CARD_TYPES] + SUBTYPES for f in plural_forms(t)},
                    key=len, reverse=True)
TYPE_RX = "(?:" + "|".join(re.escape(t) for t in TYPE_WORDS) + ")"
STATUS = ["tapped", "untapped", "attacking", "blocking", "blocked", "unblocked", "face-down", "face-up", "token",
          "nontoken", "modified", "enchanted", "equipped", "historic", "outlaw", "commander", "suspected", "saddled",
          "goaded", "double-faced", "transformed", "creature", "permanent", "nonbasic", "nonlegendary", "nonsnow",
          "legendary", "basic", "snow", "world", "chosen", "defending", "attacking or blocking", "other", "single",
          "noncommander", "attached", "exiled", "revealed", "sacrificed", "discarded", "copied", "unattached",
          "untapped", "summoning-sick", "Mount", "mana", "colored", "unenchanted", "unequipped", "nonattacking",
          "non-Aura", "non-Equipment", "nonartifact", "nonblack", "nonland", "noncreature", "nonenchantment",
          "monocolored", "multicolored", "colorless", "white", "blue", "black", "red", "green", "nonwhite", "nonblue",
          "nonred", "nongreen", "permanent", "instant", "sorcery", "random", "new", "second", "first", "additional"]
PREMOD_RX = ("(?:[Nn]on-?" + TYPE_RX + "|" + TYPE_RX + "|(?i:" + "|".join(re.escape(s) for s in sorted(set(STATUS), key=len, reverse=True))
             + r")|\d+/\d+|X/X|[Nn]on-?\w+)")
HEADS = ["permanents?", "cards?", "spells?", "tokens?", "players?", "opponents?", "sources?", "abilit(?:y|ies)",
         "objects?", "targets?", "emblems?", "commanders?", "foes?", "teammates?", "planeswalkers?"]
HEAD_RX = "(?:" + "|".join(HEADS) + "|" + TYPE_RX + ")"
QUANT_RX = (r"(?:up to (?:" + NUM + r"|X) (?:other )?(?:target )?|any number of (?:other )?(?:target )?|"
            r"(?:" + NUM + r") (?:other )?target |another target |target |each other |each |all other |all |another |"
            r"an? (?:other |single )?|one or more (?:other )?|two or more |the top (?:" + NUM + r") |the next (?:" + NUM + r" )?|"
            r"the |that |those |these |enchanted |equipped |your |their |its |no |any |(?:" + NUM + r") (?:other )?)")
NP_RE = re.compile(r"(?<![\w'])(?P<q>(?i:" + QUANT_RX + r"))?(?P<pre>(?:" + PREMOD_RX + r"(?:,? (?:or|and|and/or) |, | ))*)"
                   r"(?P<head>" + HEAD_RX + r")(?![\w'’])")
POSTMODS: list[tuple[str, re.Pattern]] = [
    ("controller", re.compile(r"^,? (?:you|an opponent|your opponents|opponents|target player|target opponent|that player|defending player|each player|each opponent|they|its controller|that creature's controller|its owner|a player|the chosen player|chosen player|your team|another player|no one|SELF's controller|the same player|he or she|enchanted player) (?:both own and control|don't control|doesn't control|don't own|doesn't own|controls?|owns?|controlled|control and own)\b")),
    ("zone", re.compile(r"^,? (?:in|from|on|onto|into|among|on top of|on the bottom of) (?:your|a|an|an opponent's|target player's|target opponent's|that player's|each player's|their|its owner's|its controller's|any|all|the|each opponent's|defending player's|a single|any one|your opponents'|all players'|any player's|his or her|exile|opponents') ?(?:graveyards?|hands?|librar(?:y|ies)|exile|the battlefield|battlefield|the stack|stack|command zone|outside the game|zones?)\b")),
    ("zone", re.compile(r"^,? of (?:your|their|its owner's|target player's|that player's|each player's|target opponent's|an opponent's|defending player's|his or her|a player's|each opponent's|that card's owner's|its controller's) (?:library|graveyard|hand)\b")),
    ("reduced relative (spells you cast, a card you drew)", re.compile(r"^,? (?:you|an opponent|a player|target player|that player|they|your opponents|each player|its controller|another player|opponents) (?:cast|casts|'ve cast|have cast|drew|discarded|discard|sacrificed|sacrifice|played|chose|'ve chosen|attacked with|noted|named|exiled|returned|own but don't control|didn't cast)\b(?: (?:this turn|this way|each turn|this game|from (?:your hand|exile|your graveyard|among them)))?")),
    ("zone", re.compile(r"^,? (?:in exile|on the battlefield|on the stack|exiled with SELF|exiled with it|from exile|from anywhere|from among them|among them|outside the game|from the battlefield|from the top of your library|from your hand)\b")),
    ("with: P/T/MV comparison", re.compile(r"^,? (?:with|without) (?:total |base )?(?:power|toughness|mana value|converted mana cost|loyalty|defense|power and toughness|power or toughness|power plus toughness)\b[^,.;]*?(?=,|\.|;| and | that | you | an opponent | (?:from|to|onto|into|until|can't|enters?|gets?|get|this combat|on the battlefield|in your|in a|among|dies|deals?|attacks?|have|has|gains?)\b|$)")),
    ("with: counters", re.compile(r"^,? (?:with|without) (?:a|an|one or more|no|two or more|three or more|N|X|\d+|exactly \w+|the most|fewer|more)? ?[^,.;]{0,25}?counters? on (?:it|them)\b")),
    ("with: keyword/ability", re.compile(r"^,? (?:with|without) (?:flying|first strike|double strike|trample|haste|vigilance|lifelink|deathtouch|menace|reach|hexproof|indestructible|shroud|defender|infect|flash|islandwalk|swampwalk|forestwalk|mountainwalk|plainswalk|horsemanship|shadow|fear|intimidate|banding|changeling|protection[^,.]*|an? (?:activated|triggered|mana|static) abilit(?:y|ies)[^,.]*|abilities|no abilities|a mana ability|ward|toxic|morph|kicker|flashback|cycling|convoke|an Adventure|deathtouch|the chosen \w+|that name|the same name|a different name|the same mana value|different names|an? \w+ ability)\b")),
    ("with: superlative", re.compile(r"^,? with the (?:greatest|least|highest|lowest|most|fewest) [^,.;]*")),
    ("with: other", re.compile(r"^,? (?:with|without)(?! paying) [^,.;]*?(?=,|\.|;| and | you | an opponent |$)")),
    ("named", re.compile(r"^,? (?:not )?named (?:SELF|[A-Z][\w'’\-]*(?:,? (?:of|the|[A-Z][\w'’\-]*))*)")),
    ("of the chosen / of that type", re.compile(r"^,? of the (?:chosen|creature|named|most common|same|appropriate) (?:type|color|name|card type|creature type)s?\b|^,? of (?:that|each|the chosen|your choice|their choice|the creature type of your choice|any|a single|a different) ?(?:type|color|name|card type|creature type|kind)s?\b|^,? of (?:their|your|its controller's|that player's|his or her) choice\b|^,? of the creature type of your choice\b")),
    ("history (this turn)", re.compile(r"^,? (?:that|which)? ?(?:was|were|has been|had been|have been)? ?(?:dealt damage|put into (?:a|your) graveyard|cast|entered|attacked|blocked|died|left the battlefield|exiled|discarded|milled|sacrificed|drawn|returned|destroyed|countered|created|tapped)\b[^,.;]*? this turn\b")),
    ("combat relation", re.compile(r"^,? (?:attacking (?:you|it|that player|a planeswalker you control|alone)|blocking (?:it|SELF|that creature|or blocked by SELF)|blocked by (?:it|SELF|that creature)|blocking or blocked by (?:it|SELF|this creature|that creature)|it's blocking|that's attacking|it blocked|that blocked it|that's blocking)\b")),
    ("attached relation", re.compile(r"^,? (?:attached to (?:it|SELF|that creature|a creature|target creature|enchanted creature)|it's attached to|enchanting \w+|SELF is attached to)\b")),
    ("exception", re.compile(r"^,? (?:other than (?:SELF|it|that creature|the [\w ]+?|those [\w ]+?|enchanted creature|a \w+)|except (?:by|for) [^,.;]+|not named [^,.;]+)\b")),
    ("that-clause", re.compile(r"^,? (?:that|which|who)(?:'s|'re| (?:is|are|was|were|has|had|have|targets?|shares?|entered|attacked|blocked|died|dealt|deals?|can|can't|isn't|aren't|wasn't|weren't|doesn't|didn't|don't|you|an opponent|a player|it|they|its|their|player|opponent|attacks?|blocks?|became|becomes?|would|could|cast|crewed|saddled|was|left|entered|share|searched|does|did|lost|gained|controls?|owns?|wasn't|isn't|remains?|haven't|hasn't|tapped|untapped)\b)[^,.;]*?(?=,|\.|;|$| and (?:you|it|put|return|exile|then)\b)")),
    ("dealt damage / power filter", re.compile(r"^,? dealt damage [^,.;]*")),
    ("among / from among", re.compile(r"^,? (?:from )?among (?:them|those cards|the [\w ]+|cards? [\w ]+)\b")),
]


@dataclass
class NP:
    text: str
    quant: str
    pre: list[str]
    head: str
    post: list[tuple[str, str]]
    segment: str
    conj: list[str] = field(default_factory=list)


def norm_q(q: str) -> str:
    q = q.strip()
    q = re.sub(r"\b(?:" + NUMWORDS + r"|\d+)\b", "N", q)
    return q or "(bare)"


PLAYER_HEADS = {"player", "players", "opponent", "opponents", "teammate", "teammates", "foe", "foes"}


def find_nps(text: str, segment: str) -> list[NP]:
    out = []
    for m in NP_RE.finditer(text):
        head = m.group("head")
        q = m.group("q") or ""
        pre = m.group("pre").strip()
        # bare plural/singular type words without quantifier inside larger prose: keep (e.g. "Creatures you control")
        pos = m.end()
        post = []
        while True:
            for lbl, rx in POSTMODS:
                pm = rx.match(text[pos:])
                if pm and pm.group(0).strip(" ,"):
                    post.append((lbl, pm.group(0).strip(" ,")))
                    pos += pm.end()
                    break
            else:
                break
        pres = [p for p in re.split(r"[ ,]+", pre) if p and p not in ("or", "and", "and/or")] if pre else []
        conj = re.findall(r"\b(and/or|or|and)\b", pre)
        out.append(NP(text[m.start():pos], q.strip(), pres, head, post, segment, conj))
    return out


def classify_quant(q: str) -> str:
    q = q.lower()
    if "target" in q:
        if q.startswith("up to"):
            return "up to N target"
        if q.startswith("any number"):
            return "any number of target"
        if q.startswith("another"):
            return "another target"
        if re.match(r"^(?:" + NUM + r") ", q):
            return "N target"
        return "target"
    for k, rx in [("each/all (universal)", r"^(?:each|all)\b"), ("a/an/another (one, chosen on resolution)", r"^(?:an?|another)\b"),
                  ("up to N (untargeted)", r"^up to"), ("any number of (untargeted)", r"^any number"),
                  ("one or more", r"^(?:one|two) or more"), ("N (untargeted count)", r"^(?:" + NUM + r")\b"),
                  ("the/that/those (reference)", r"^(?:the|that|those|these)\b"), ("enchanted/equipped (attachment)", r"^(?:enchanted|equipped)"),
                  ("your/their/its (possessive)", r"^(?:your|their|its)"), ("no (negated)", r"^no\b"), ("any", r"^any\b")]:
        if re.match(rx, q):
            return k
    return "(bare: plural generic or subject)"


def classify_head(head: str, pre: list[str]) -> str:
    h = head.lower().rstrip("s")
    if head.lower() in PLAYER_HEADS:
        return "player"
    if h in ("card",):
        return "card (non-battlefield zone)"
    if h in ("spell",):
        return "spell (stack)"
    if h in ("abilit", "ability", "abilitie"):
        return "ability (stack)"
    if h == "token":
        return "token"
    if h in ("source", "object", "cop", "copy", "copie", "target", "emblem"):
        return h if h not in ("cop", "copie") else "copy"
    if h == "permanent":
        return "permanent"
    return "typed permanent/object (type word as head)"


ONE_SHOT_KINDS = ("triggered", "activated", "loyalty", "saga", "mode", "spree", "quoted triggered", "quoted activated")


def is_one_shot(a: Ability) -> bool:
    return a.is_spell or a.kind.startswith(ONE_SHOT_KINDS)


def is_static(a: Ability) -> bool:
    return (not a.is_spell) and a.kind.startswith(("static or spell text", "quoted static"))


SUPER_SET = {"basic", "legendary", "snow", "world"}
COLOR_SET = {"white", "blue", "black", "red", "green", "colorless", "multicolored", "monocolored", "colored"}
CARDTYPE_SET = {f for t in CARD_TYPES for f in plural_forms(t)} | {f.capitalize() for t in CARD_TYPES for f in plural_forms(t)}
SUBTYPE_SET = {f for t in SUBTYPES for f in plural_forms(t)}


def premod_class(p: str) -> str:
    pl = p.lower()
    if pl in ("other", "another"):
        return "other (excludes SELF/antecedent)"
    if p in CARDTYPE_SET:
        return "card type"
    if p in SUBTYPE_SET:
        return "subtype"
    if pl in SUPER_SET:
        return "supertype"
    if pl in COLOR_SET:
        return "color"
    if pl.startswith("non"):
        return "negated (non-X)"
    if pl in ("tapped", "untapped", "attacking", "blocking", "blocked", "unblocked", "face-down", "face-up",
              "token", "modified", "enchanted", "equipped", "suspected", "saddled", "goaded", "transformed", "attached",
              "summoning-sick", "unenchanted", "unequipped", "nonattacking", "double-faced", "commander", "historic", "outlaw"):
        return "status/property"
    if re.match(r"^(\d+|X)/(\d+|X)$", p):
        return "P/T (token spec)"
    if pl in ("chosen", "exiled", "revealed", "sacrificed", "discarded", "copied", "defending"):
        return "reference participle"
    return "other word"


NP_DUMP: list[dict] = []  # every noun phrase, written by --dump-nps (the selector grammar's sub-corpus)


def selector_stats(abls: list[Ability]) -> dict:
    quant = collections.Counter()
    quant_raw = collections.Counter()
    head = collections.Counter()
    pre_cls = collections.Counter()
    post_cls = collections.Counter()
    post_vals: dict[str, collections.Counter] = collections.defaultdict(collections.Counter)
    n_types = collections.Counter()
    conj = collections.Counter()
    ncomp = collections.Counter()
    literal = collections.Counter()
    factor_tuple = collections.Counter()
    segs = collections.Counter()
    examples: dict[str, str] = {}
    unconsumed = collections.Counter()
    n = 0
    for a in abls:
        for seg, text in (("effect", a.effect), ("trigger", a.trigger), ("cost", a.cost)):
            if not text:
                continue
            segment = ("static" if is_static(a) else "effect") if seg == "effect" else seg
            for np_ in find_nps(text, segment):
                if not np_.quant and not np_.pre and not np_.post and np_.head.lower() in ("target", "targets", "cards", "card", "spell", "spells"):
                    continue
                n += 1
                segs[segment] += 1
                qc = classify_quant(np_.quant)
                NP_DUMP.append({"oracle_id": a.card, "face": a.name, "segment": segment, "text": np_.text,
                                "quant": qc, "head": classify_head(np_.head, np_.pre),
                                "pre": sorted({premod_class(p) for p in np_.pre}), "post": [l for l, _ in np_.post]})
                quant[qc] += 1
                quant_raw[norm_q(np_.quant).lower()] += 1
                hc = classify_head(np_.head, np_.pre)
                head[hc] += 1
                types = [p for p in np_.pre if premod_class(p) in ("card type", "subtype")]
                if classify_head(np_.head, []) == "typed permanent/object (type word as head)":
                    types.append(np_.head)
                n_types[min(len(types), 5)] += 1
                for c in np_.conj:
                    conj[c] += 1
                for p in np_.pre:
                    pc = premod_class(p)
                    pre_cls[pc] += 1
                    post_vals["pre:" + pc][p.lower()] += 1
                for lbl, val in np_.post:
                    post_cls[lbl] += 1
                    post_vals[lbl][re.sub(r"\b\d+\b", "N", val)] += 1
                    examples.setdefault(lbl, np_.text)
                comps = (1 if np_.quant else 0) + len({premod_class(p) for p in np_.pre}) + 1 + len(np_.post)
                ncomp[min(comps, 7)] += 1
                lit = re.sub(r"\b(?:\d+|" + NUMWORDS + r")\b", "N", np_.text)
                literal[lit] += 1
                factor_tuple[(qc, hc, tuple(sorted({premod_class(p) for p in np_.pre})), tuple(l for l, _ in np_.post))] += 1
                # what follows the NP: does it look like an unconsumed post-modifier?
                tail = text[text.find(np_.text) + len(np_.text):][:30]
                m = re.match(r"^,? (with|without|that|named|of|other than|except|attached|among|dealt|blocking|attacking|in|from|on|you|an opponent)\b", tail)
                if m and not re.match(r"^,? (?:from|in|on) (?:the top|the bottom|among|a graveyard|anywhere)", tail):
                    unconsumed[m.group(1)] += 1
    return {"noun phrases": n, "by segment": dict(segs), "quantifier class": dict(quant.most_common()),
            "quantifier surface": dict(quant_raw.most_common(60)), "head class": dict(head.most_common()),
            "premodifier class": dict(pre_cls.most_common()), "postmodifier class": dict(post_cls.most_common()),
            "values": {k: dict(v.most_common(40)) for k, v in post_vals.items()},
            "distinct values": {k: len(v) for k, v in post_vals.items()},
            "types per NP": dict(sorted(n_types.items())), "type-list connectors": dict(conj),
            "components per NP": dict(sorted(ncomp.items())), "distinct literal NPs": len(literal),
            "literal top": dict(literal.most_common(40)), "literal curve": curve(literal),
            "distinct factor tuples": len(factor_tuple), "factor tuple curve": curve(factor_tuple),
            "factor tuple top": {" × ".join([q, h, "+".join(p) or "-", "+".join(pp) or "-"]): c for (q, h, p, pp), c in factor_tuple.most_common(25)},
            "post examples": examples, "unconsumed after NP": dict(unconsumed.most_common())}


def curve(counter: collections.Counter, points=(1, 5, 10, 20, 50, 100, 200, 500, 1000, 2000, 5000)) -> dict:
    tot = sum(counter.values()) or 1
    vals = [c for _, c in counter.most_common()]
    out = {}
    acc = 0
    for i, v in enumerate(vals, 1):
        acc += v
        if i in points:
            out[i] = round(100 * acc / tot, 1)
    out["all"] = len(vals)
    singletons = sum(1 for v in vals if v == 1)
    out["singletons"] = singletons
    return out


# ---------------------------------------------------------------- back-references

BACKREFS: list[tuple[str, str, str]] = [
    ("pronoun: it / its", "pronoun", r"\b(?:it|its|it's)\b"),
    ("pronoun: them / they / their (objects or players)", "pronoun", r"\b(?:them|they|their|they're)\b"),
    ("that player / that opponent", "demonstrative", r"\bthat (?:player|opponent)(?:'s)?\b"),
    ("that creature / permanent / card / spell / token ...", "demonstrative", r"\bthat (?:creature|permanent|card|spell|token|land|artifact|enchantment|planeswalker|ability|source|Aura|Equipment|Vehicle|object|copy|battle|[A-Z][a-z]+)(?:'s)?\b"),
    ("those cards / creatures / tokens ...", "demonstrative", r"\bthose (?:cards?|creatures?|permanents?|tokens?|players?|spells?|lands?|artifacts?|opponents?|abilities|counters|[A-Z][a-z]+s?)\b"),
    ("that much / that many (amount ref)", "amount", r"\bthat (?:much|many)\b"),
    ("the exiled / revealed / chosen / sacrificed ... (participle)", "participle", r"\bthe (?:exiled|revealed|chosen|sacrificed|discarded|milled|copied|returned|destroyed|countered|named|noted|drawn|created|tapped|untapped|stolen|enchanted|cast|targeted|attacking|blocking|removed|imprinted|haunted|foretold|plotted|marked|bid|secretly chosen|selected|kicked) [\w ]{0,20}?(?:cards?|creatures?|permanents?|tokens?|spells?|players?|types?|colors?|names?|numbers?|modes?|objects?|lands?|artifacts?|card type|counters?|creature type|piles?|opponents?)\b"),
    ("the token / the copy / the card (definite after creation)", "definite", r"\bthe (?:token|tokens|copy|copies|card|cards|creature|creatures|permanent|permanents|spell)\b(?! (?:named|that|with|you|from|in|of|an opponent))"),
    ("... this way (result of previous step)", "result", r"\bthis way\b"),
    ("the rest / the others / the remaining", "remainder", r"\bthe (?:rest|others?|remaining|remainder)\b"),
    ("each of them / one of them / among them / any of them", "set member", r"\b(?:each|one|any|all|two|none|either|both) of (?:them|those)\b|\bamong them\b"),
    ("exiled with SELF / cards exiled with (linked 607)", "linked", r"\bexiled with (?:SELF|it|this)\b|\bexiled with (?:a|an) [\w ]+ named\b"),
    ("its controller / its owner / that card's owner", "relational", r"\b(?:its|their|that \w+'s) (?:controller|owner|controllers|owners)\b"),
    ("the same name / type / color as", "relational", r"\bthe same (?:name|type|color|mana value|card type|creature type|power|controller)\b|\bshares? (?:a|an) [\w ]+ with\b"),
    ("enchanted / equipped / fortified (attachment)", "attachment", r"\b(?:enchanted|equipped|fortified|haunted|paired) (?:creature|permanent|land|artifact|player|enchantment|planeswalker)\b"),
    ("SELF (self-reference)", "self", r"\bSELF\b"),
    ("he / she / him / her (legendary characters)", "pronoun", r"\b(?:he|she|him|her|his|hers)\b"),
    ("X (bound by where/as cast)", "variable", r"\bX\b"),
    ("defending player / the attacking player", "contextual", r"\b(?:defending player|the attacking player|the active player|the player to your (?:left|right)|the monarch|the chosen player|the next player)\b"),
]
BACKREF_RES = [(l, k, re.compile(r)) for l, k, r in BACKREFS]


def backref_stats(abls: list[Ability]) -> dict:
    by = collections.Counter()
    by_kind = collections.Counter()
    by_ability = collections.Counter()
    cards = collections.defaultdict(set)
    ex = {}
    trig_vs = collections.Counter()
    n_ab = n_with = 0
    for a in abls:
        if not a.effect or not is_one_shot(a):
            continue
        n_ab += 1
        any_ref = False
        for l, k, rx in BACKREF_RES:
            if k in ("self", "variable", "attachment"):
                hits = len(rx.findall(a.effect))
            else:
                hits = len(rx.findall(a.effect))
            if hits:
                by[l] += hits
                by_kind[k] += hits
                cards[l].add(a.card)
                if k not in ("self", "variable", "attachment", "relational", "contextual"):
                    any_ref = True
                    if a.kind.startswith(("triggered", "quoted triggered")) and k in ("demonstrative", "pronoun"):
                        trig_vs[l] += hits
                if len(a.raw) < 170:
                    ex.setdefault(l, f"{a.name}: {a.raw}")
        n_with += any_ref
    return {"one-shot abilities": n_ab, "with an anaphoric reference": n_with, "by phrase": dict(by.most_common()),
            "by kind": dict(by_kind.most_common()), "cards": {k: len(v) for k, v in cards.items()},
            "in triggered abilities (event refs likely)": dict(trig_vs), "examples": ex}


# ============================================================== amounts (section 4)

def gen_norm(s: str) -> str:
    """Template a phrase: numbers -> N, subtypes -> TYPE, colors -> COLOR, card types kept."""
    s = re.sub(r"\b(?:\d+|" + NUMWORDS + r")\b", "N", s)
    s = re.sub(r"\{[WUBRGC]\}", "{M}", s)
    s = re.sub(r"(?<![\w'])(?:" + "|".join(re.escape(t) for t in sorted(SUBTYPE_SET, key=len, reverse=True)) + r")(?![\w'])", "TYPE", s)
    s = re.sub(r"\b(?:white|blue|black|red|green)\b", "COLOR", s)
    s = re.sub(r"\bTYPE(?:(?:, | or | and | and/or |, or |, and )TYPE)+", "TYPE", s)
    return s.strip()


AMOUNT_CLASSES: list[tuple[str, str]] = [
    ("reference to an earlier amount (that much / that many)", r"^(?:number of )?that (?:much|many)\b"),
    ("arithmetic (twice / half / plus / minus / rounded)", r"\btwice\b|\bhalf\b|\bplus\b|\bminus\b|rounded|\bthree times\b|\bdifference\b"),
    ("distinct count (colors / types / names among)", r"(?:different|differently named|distinct|colors? (?:of|among)|basic land types? among|card types? among|creature types? among|SEL types among|types among)"),
    ("aggregate (greatest / total / highest among)", r"(?:greatest|highest|least|lowest|total|sum of|combined)\b"),
    ("counters on an object / player counters", r"counters? (?:on|you have|they have)|counters?$|experience|energy|poison"),
    ("event amount (damage dealt / life gained / cards drawn this way / this turn)", r"(?:damage|life|card|mana|spell|creature)[^.]*?(?:this way|this turn|dealt to|you've gained|you've lost|lost this turn|gained this turn)|(?:damage|life) (?:dealt|gained|lost)|excess damage|amount of (?:damage|life)"),
    ("mana spent / paid (incl. X)", r"(?:mana (?:spent|paid)|amount of mana|amount paid|life paid|mana from|spent to cast)"),
    ("player stat (life total / devotion / hand size / party / opponents)", r"life total|devotion|speed|party|opponent you have|your (?:life|hand)|starting life|card in (?:your|their|that player's|target player's|an opponent's) hand"),
    ("power / toughness / mana value of an object", r"(?:'s|its|their|his|her) (?:power|toughness|mana value|converted mana cost|loyalty|base power|mana cost)\b|^(?:power|toughness|mana value)\b"),
    ("count of objects in a zone / on battlefield (number of <selector>)", r"^number of |\byou control\b|\bcontrols?\b|in (?:your|their|all|a|an opponent's|target player's) (?:graveyards?|library|exile)|on the battlefield|\bTYPE\b|attacking|blocking"),
]
AMOUNT_RES = [(l, re.compile(r, re.I)) for l, r in AMOUNT_CLASSES]


SINGULAR = {"cards": "card", "creatures": "creature", "lands": "land", "artifacts": "artifact", "permanents": "permanent",
            "enchantments": "enchantment", "spells": "spell", "tokens": "token", "opponents": "opponent", "players": "player",
            "counters": "counter", "planeswalkers": "planeswalker", "instants": "instant", "sorceries": "sorcery"}


def canon_amount(e: str) -> str:
    e = gen_norm(e)
    e = re.sub(r"^(?:the )?(?:total )?number of ", "", e)
    e = re.sub(r"^the ", "", e)
    e = re.sub(r"\b(\w+)\b", lambda m: SINGULAR.get(m.group(1), m.group(1)), e)
    return re.sub(r"\s+", " ", e).strip()


def classify_amount(expr: str) -> str:
    for l, rx in AMOUNT_RES:
        if rx.search(expr):
            return l
    return "other"


AMOUNT_DUMP: list[dict] = []  # every amount expression with its introducer, written by --dump-amounts


def amount_stats(abls: list[Ability]) -> dict:
    lits = collections.Counter()
    exprs = collections.Counter()
    expr_cls = collections.Counter()
    expr_ex: dict[str, str] = {}
    intro = collections.Counter()
    shapes = collections.Counter()
    for a in abls:
        t = a.effect
        if not t:
            continue
        for m in re.finditer(r"(?<![{\w/+\-−])(\d+|" + NUMWORDS + r")(?![}\w/])", t):
            lits["digit literal" if m.group(1).isdigit() else "number word"] += 1
        lits["X"] += len(re.findall(r"\bX\b", t))
        lits["P/T modifier +N/+N"] += len(re.findall(r"[+\-−][\dX]+/[+\-−][\dX]+", t))
        lits["mana symbol amounts {N}"] += len(re.findall(r"\{\d+\}", t))
        for key, rx in (("equal to ...", r"\bequal to (.+?)(?=[.,;]| plus | minus | and (?:you|it|then|each|put|draw|create|gain)| to (?:any|target|each|that|it|you|him|her|them|SELF|up to|another|itself|the)\b| (?:until|this turn|instead|rounded)\b|$)"),
                        ("where X is ...", r"\bwhere X is (.+?)(?=[.;]|, (?:and|then|rounded)|$)"),
                        ("for each ... (multiplier)", r"(?<!^)(?<!, )\bfor each (.+?)(?=[.,;]| (?:you control|this way)\b(?=[.,])|$)"),
                        ("the number of ...", r"\bthe number of (.+?)(?=[.,;]|$)"),
                        ("that much / that many", r"\b(that (?:much|many))\b"),
                        ("twice / half / N times", r"\b(twice (?:that many|that much|the|X|its|the number)|half (?:that many|that much|your|their|the|X|its|his|her)|three times (?:that|the|its))\b")):
            for m in re.finditer(rx, t):
                intro[key] += 1
                AMOUNT_DUMP.append({"oracle_id": a.card, "face": a.name, "intro": key, "text": m.group(0)})
                e = canon_amount(m.group(1))
                exprs[e] += 1
                sel_shape = re.sub(r"\s+", " ", NP_RE.sub("SEL", e))
                shapes[sel_shape] += 1
                cls = classify_amount(("number of " if key in ("the number of ...", "for each ... (multiplier)") or
                                       re.match(r"^(?:the )?number of", m.group(1)) else "") + e)
                if key == "for each ... (multiplier)" and cls == "other":
                    cls = "count of objects (the number of <selector>)" if not re.search(r"counter|mana|damage|life|time", e) else cls
                expr_cls[cls] += 1
                if cls not in expr_ex and len(a.raw) < 180:
                    expr_ex[cls] = f"{a.name}: {a.raw}"
    return {"literals": dict(lits.most_common()), "introducers": dict(intro.most_common()),
            "distinct expressions": len(exprs), "expression curve": curve(exprs),
            "top expressions": dict(exprs.most_common(45)), "distinct shapes (selectors abstracted)": len(shapes),
            "shape curve": curve(shapes), "top shapes": dict(shapes.most_common(40)), "classes": dict(expr_cls.most_common()), "class examples": expr_ex}


# ============================================================== conditions (section 5)

COND_INTRO = [
    ("intervening if (603.4)", r"^(?:When|Whenever|At)\b[^,]*, if (.+?)(?:,|$)"),
    ("If ..., (prefix)", r"(?:^|\. |\bThen )If (?!you do\b|you don't\b|they do\b|that player does\b|no one does\b|it would\b|.*\bwould\b)(.+?),"),
    (", if ... (postfix)", r"(?<=[\w}]) if (?!able\b|you do\b|you don't\b|it would|.*\bwould\b)(.+?)(?:[.;]|$)"),
    ("as long as ...", r"\b[Aa]s long as (.+?)(?:,|\.|$)"),
    ("unless ...", r"\bunless (.+?)(?:[.,;]|$)"),
    ("only if ...", r"\bonly if (.+?)(?:[.,;]|$)"),
    ("only during / only as a sorcery / only once", r"\b(only (?:during [^.,]+|as a sorcery|once each turn|any time you could cast a sorcery|before (?:blockers|attackers) [^.,]+|from your graveyard|if [^.,]*|on your turn|on an opponent's turn|while [^.,]+|twice each turn|three times each turn))"),
    ("If you do / don't (result of prior step)", r"\b[Ii]f (you|they|that player|no one|a player|an opponent|any player|he|she) (do|does|don't|doesn't|did|didn't)\b"),
]
COND_SHAPES: list[tuple[str, str]] = [
    ("history: cast/played this turn", r"(?:cast|played|play) [^.]*?this turn|cast (?:your|their|a|an|another) [^.]*?(?:this turn|last turn)|spells? [^.]*?this turn"),
    ("history: life gained/lost this turn", r"(?:gained|lost|gain|lose) [^.]*?life this turn|life this turn"),
    ("history: died / left / put into graveyard this turn", r"(?:died|dies|left the battlefield|put into (?:a|your) graveyard|descended) [^.]*?this turn|this turn[^.]*died"),
    ("history: attacked / blocked / dealt damage this turn", r"(?:attacked|blocked|attacking|dealt (?:combat )?damage|was dealt damage|been dealt damage) [^.]*?this turn|attacked with"),
    ("history: entered / drawn / sacrificed / discarded / other this turn", r"this turn|last turn|since"),
    ("history: how the object was cast / paid (kicked, from zone, mana spent)", r"\b(?:was|were) (?:kicked|bargained|cast|paid|foretold|blitzed|bestowed|evoked|spent|turned face up|cast from|plotted|offspring)|\bwas spent\b|mana from|{[WUBRG]} was spent|it was cast|kicker|gift was promised|the gift was|you cast it from|was \w+ed this game|SELF was \w+ed|wasn't cast"),
    ("history: this game / ever", r"this game|\bever\b|\bfirst time\b"),
    ("designation / game state (monarch, initiative, day/night, speed, city's blessing)", r"monarch|initiative|city's blessing|\bday\b|\bnight\b|max speed|speed is|\bRing\b|\bdungeon\b|\bsolved\b|\bunlocked\b|\blevel \d|\bprepared\b|\bpaired\b|\bRing-bearer\b"),
    ("your turn / not your turn / phase", r"it's (?:not )?(?:your|their|an opponent's) turn|it is (?:not )?your turn|during (?:your|an opponent's|their|each) turn|your (?:first|second|precombat|postcombat) main phase|(?:combat|upkeep|end step)\b"),
    ("you control (a/N or more/no) <selector>", r"\byou (?:control|don't control) (?:a|an|another|N or more|no|two or more|three or more|four or more|five or more|at least|exactly|fewer|more|the|each|SELF|that|it|all|enchanted|equipped|N|X)\b|you control (?:N|a|an) [\w ]+? (?:or more|or fewer)?"),
    ("opponent / player controls <selector>", r"\b(?:an opponent|that player|defending player|each opponent|no opponent|target player|a player|its controller|no player|each player) (?:controls?|doesn't control|has|have|had)\b|\bopponents? controls?\b"),
    ("count threshold (N or more ... in graveyard / hand / library)", r"N or more (?:cards?|card types?|[\w ]*cards?|creature cards?|instant and/or sorcery|lands?|artifacts?|permanents?|counters|creatures?|[\w ]+) (?:in|among|on|you control)|N or (?:fewer|more) cards in (?:your|their) hand|no cards in (?:your|their) hand|(?:hand|graveyard|library) (?:is empty|has)|cards in hand|seven or more cards|card types among cards"),
    ("life threshold / comparison", r"life total|N or (?:more|less) life|more life than|less life than|N life|life total is"),
    ("object property (it's a creature / has flying / power N or greater / tapped)", r"\b(?:it|that \w+|SELF|the \w+|enchanted creature|equipped creature|that card|the revealed card|the exiled card|it's|its|SELF's|target \w+)\b[^.]*?\b(?:is|are|isn't|was|has|had|have|power|toughness|mana value|tapped|untapped|attacking|blocking|a creature|a land|legendary|card|token|colorless|COLOR|shares?)\b"),
    ("numeric comparison (X / the number of ... is N or greater)", r"\bX is\b|\bN or (?:greater|more|less|fewer)\b|greater than|less than|at least N|exactly N|equal to"),
    ("counters on (it/a player)", r"counters? on|has (?:N|a|an|no) \w+ counters?|poison counters|energy"),
    ("payment / choice by a player (unless pays)", r"\bpays?\b|sacrifices?|discards?|exiles?|reveals?|returns?|removes?|taps?|chooses?|puts?"),
    ("choice / mode reference (you chose, the chosen)", r"\bchose\b|\bchosen\b|the vote|votes?|guessed|the coin|won the flip|you win|lost the flip|the result"),
    ("comparison between players (more cards / most / fewer)", r"\b(?:more|fewer|most|greatest|least|highest|lowest) [\w ]+ than\b|\bthe most\b|\bmore\b|\bfewer\b"),
]
COND_SHAPE_RES = [(l, re.compile(r)) for l, r in COND_SHAPES]
HISTORY_KEYS = {"history: cast/played this turn", "history: life gained/lost this turn", "history: died / left / put into graveyard this turn",
                "history: attacked / blocked / dealt damage this turn", "history: entered / drawn / sacrificed / discarded / other this turn",
                "history: how the object was cast / paid (kicked, from zone, mana spent)", "history: this game / ever"}
CONDITION_ABILITY_WORDS = ["threshold", "delirium", "metalcraft", "morbid", "hellbent", "ferocious", "spell mastery", "revolt",
                           "raid", "coven", "domain", "formidable", "fateful hour", "corrupted", "descend 4", "descend 8",
                           "fathomless descent", "max speed", "eerie", "survival", "void", "valiant", "celebration",
                           "adamant", "addendum", "kinship", "chroma", "converge", "radiance", "lieutenant", "battalion", "pack tactics"]


COND_DUMP: list[dict] = []  # every condition with its introducer, written by --dump-conditions


def condition_stats(abls: list[Ability]) -> dict:
    intro = collections.Counter()
    shapes = collections.Counter()
    shape_ex: dict[str, str] = {}
    norm = collections.Counter()
    cshape = collections.Counter()
    hist_cards = set()
    hist_facts = collections.Counter()
    aw = collections.Counter()
    for a in abls:
        full = (a.trigger + ", " + a.effect) if a.trigger else a.effect
        if a.cost:
            full = a.cost + ": " + full
        if not full.strip():
            continue
        m = re.match(r"^([A-Z][\w' ]+?\d?) — ", a.raw)
        if m and m.group(1).lower() in CONDITION_ABILITY_WORDS:
            aw[m.group(1).lower()] += 1
        for label, rx in COND_INTRO:
            for mm in re.finditer(rx, full):
                c = mm.group(1)
                intro[label] += 1
                if label.startswith("If you do"):
                    continue
                COND_DUMP.append({"oracle_id": a.card, "face": a.name, "intro": label, "text": c})
                cn = gen_norm(c)
                norm[(label.split(" ")[0], cn)] += 1
                cshape[re.sub(r"\s+", " ", NP_RE.sub("SEL", cn))] += 1
                cls = "other"
                if label.startswith("only during") or label.startswith("only if"):
                    cls = "timing / activation restriction" if label.startswith("only during") else None
                if cls != "timing / activation restriction":
                    cls = "other"
                    for l, crx in COND_SHAPE_RES:
                        if crx.search(cn):
                            cls = l
                            break
                shapes[cls] += 1
                if cls in HISTORY_KEYS:
                    hist_cards.add(a.card)
                if cls not in shape_ex and len(a.raw) < 190:
                    shape_ex[cls] = f"{a.name}: {a.raw}"
        # history facts anywhere in the text (conditions, amounts, triggers)
        for fact, rx in HISTORY_FACTS:
            if re.search(rx, full):
                hist_facts[fact] += 1
    return {"introducers": dict(intro.most_common()), "shapes": dict(shapes.most_common()), "shape examples": shape_ex,
            "distinct normalized conditions": len(norm), "condition curve": curve(collections.Counter({k: v for k, v in norm.items()})),
            "top conditions": {f"[{k[0]}] {k[1]}": v for k, v in norm.most_common(40)},
            "distinct shapes (selectors abstracted)": len(cshape), "shape curve": curve(cshape),
            "top shapes": dict(cshape.most_common(30)),
            "cards with history-dependent conditions": len(hist_cards), "history facts (any text)": dict(hist_facts.most_common()),
            "condition ability words": dict(aw.most_common())}


HISTORY_FACTS: list[tuple[str, str]] = [
    ("spells cast this turn (by player, count, type, order: 'second spell')", r"(?:cast|casts) [^.]*?this turn|(?:second|third|first) spell|spells? (?:you've|they've|that player has|you|an opponent has) cast (?:this turn|before)|cast this turn|storm"),
    ("life gained / lost this turn (amount)", r"(?:gained|lost) [^.]*?life this turn|life this turn"),
    ("creatures / permanents died this turn", r"died this turn|put into (?:a|your) graveyard from the battlefield this turn|creatures? (?:that )?died|morbid"),
    ("attacked / attacked with this turn", r"attacked (?:this turn|with)|attacked during|creatures? that attacked|raid"),
    ("dealt damage this turn (by/to whom)", r"dealt (?:combat )?damage (?:to \w+ )?this turn|been dealt damage this turn|dealt damage by [^.]* this turn|was dealt damage this turn"),
    ("permanents/cards entered this turn (landfall count, tokens)", r"entered (?:the battlefield )?(?:under your control )?this turn|lands? (?:you played|played) this turn|played a land this turn"),
    ("cards drawn this turn (count, 'second card')", r"(?:drawn|drew) [^.]*?this turn|second card each turn|cards? (?:you've )?drawn this turn"),
    ("left the battlefield / exiled / sacrificed / discarded this turn", r"(?:left the battlefield|sacrificed|discarded|exiled|milled) [^.]*?this turn|revolt|descended"),
    ("counters put this turn", r"counters? (?:were )?put on [^.]* this turn|put (?:one or more )?counters? [^.]* this turn"),
    ("how the object was cast (kicked, from where, mana spent)", r"(?:was|were) (?:kicked|cast from|bargained|foretold|evoked|blitzed|bestowed)|mana from|\{[WUBRGC]\} was spent|mana spent to cast|was spent to cast|if it was cast|cast it from|you cast it|wasn't cast"),
    ("this game / ever / first time", r"this game|\bfirst time\b|\bever\b"),
    ("last turn / since your last turn", r"last turn|since (?:your|the) last|since the beginning of|until your next turn"),
    ("crime committed / expended / descended / other designations", r"committed a crime|expend|descended|you sacrificed|you've completed|completed a dungeon|become the monarch|initiative"),
    ("choices recorded on the object (chosen color/type/name/player/mode)", r"the chosen (?:color|type|creature type|name|player|card type|number|mode|opponent)|of the chosen|named card|noted"),
    ("linked objects (exiled with / imprinted / haunted)", r"exiled with (?:SELF|it)|imprinted|haunted by|cards? exiled with|haunting"),
]


# ============================================================== durations (section 6)

DURATIONS: list[tuple[str, str]] = [
    ("until end of turn", r"\buntil end of turn\b"),
    ("this turn", r"\bthis turn\b"),
    ("until your next turn", r"\buntil your next turn\b"),
    ("until the end of your next turn", r"\buntil the end of (?:your|their|that player's) next turn\b"),
    ("until end of combat / this combat", r"\buntil end of combat\b|\bthis combat\b"),
    ("for as long as <condition>", r"\bfor as long as\b"),
    ("as long as <condition> (static)", r"(?<!for )\bas long as\b"),
    ("until SELF / it leaves the battlefield", r"\buntil [^.,]* leaves the battlefield\b"),
    ("during your/its controller's next untap step / next turn", r"\bduring (?:your|its controller's|their|that player's) next (?:untap step|turn)|\bnext untap step\b"),
    ("until <player>'s next end step / upkeep / beginning of", r"\buntil (?:the beginning of )?(?:your|their|that player's|the) next (?:end step|upkeep)|\buntil the next end step\b"),
    ("this game / for the rest of the game", r"\bfor the rest of the game\b|\bthis game\b"),
    ("until <event> (event-ended duration, not a reveal-until loop)", r"(?<!library )(?<!libraries )\buntil (?:a|that|an|target|any) (?:player|opponent) [^.,]*\b|(?<!library )\buntil you (?!reveal|exile|mill)[^.,]*\b"),
    ("delayed: at the beginning of the next end step / next upkeep", r"\bat the beginning of (?:the|your|that player's|their) next (?:end step|upkeep|turn|cleanup|draw step|main phase|combat)|\bat end of combat\b|\bat the beginning of the end step\b"),
    ("the next time ... (shield / one-shot replacement)", r"\bthe next time\b"),
    ("each turn / once each turn (per-turn reset)", r"\b(?:once|twice) each turn\b|\beach turn\b"),
]


def duration_stats(abls: list[Ability]) -> dict:
    c = collections.Counter()
    cards = collections.defaultdict(set)
    ex = {}
    for a in abls:
        t = a.effect
        if not t:
            continue
        for l, rx in DURATIONS:
            k = len(re.findall(rx, t, re.I))
            if k:
                c[l] += k
                cards[l].add(a.card)
                if len(a.raw) < 170:
                    ex.setdefault(l, f"{a.name}: {a.raw}")
    return {"counts": dict(c.most_common()), "cards": {k: len(v) for k, v in cards.items()}, "examples": ex}


# ============================================================== static abilities (section 7)

KW_LIST = (r"(?:flying|first strike|double strike|trample|haste|vigilance|lifelink|deathtouch|menace|reach|hexproof|"
           r"indestructible|shroud|protection|defender|ward|infect|wither|flash|fear|intimidate|\w+walk|banding|shadow|"
           r"horsemanship|skulk|prowess|changeling|convoke|affinity|cascade|annihilator|toxic|exalted|undying|persist|"
           r"devoid|split second|storm|flanking|rampage|bushido|partner|crew|equip|cycling|flashback|unearth|escape|"
           r"QUOTED|all|an? (?:additional )?(?:activated|triggered|mana) abilit)")
STATIC_CLASSES: list[tuple[str, str, str]] = [
    # label, layer / rule, regex
    ("characteristic-defining P/T (SELF's power is equal to ...)", "7a (604.3)", r"^SELF's (?:power|toughness|power and toughness) (?:is|are)(?: each)? equal to"),
    ("characteristic-defining color/type (SELF is all colors / every creature type)", "4/5 CDA (604.3)", r"^SELF is (?:all colors|every creature type|colorless|white|blue|black|red|green|the chosen|also)|^SELF is (?:every|all)"),
    ("copy effects (as a copy of / is a copy)", "1 (613.2)", r"\bcopy of\b|\bcopies of\b"),
    ("control change (you control enchanted ...)", "2", r"\byou control enchanted\b|\bgain control\b|\bcontrols? enchanted\b"),
    ("text change", "3 (612)", r"\btext of\b|\breplacing all instances\b"),
    ("type change (is a/an ... in addition / becomes / every type)", "4", r"in addition to its other (?:types|creature types)|\bis (?:a|an|every) (?:\w+ )?(?:creature|land|artifact|enchantment|Island|Forest|Swamp|Mountain|Plains|[A-Z]\w+)\b|\bare (?:\w+ )?(?:creatures|lands|artifacts|Forests|Islands|Swamps|Mountains|Plains)\b|\blos(?:es|e) all (?:creature|land|other card) types|\bisn't a creature\b|\bare (?:basic )?[A-Z]\w+ lands\b|\bis legendary\b|\bis (?:still )?a [\w ]*?creature\b"),
    ("color change", "5", r"\bis (?:white|blue|black|red|green|colorless|all colors)\b|\bare (?:white|blue|black|red|green|colorless)\b|\bthe colors? of (?:your|their) choice\b|\bis the chosen color\b"),
    ("ability grant (has / have / gains)", "6", r"\b(?:has|have|gains?|gain) " + KW_LIST + r"|\bhas QUOTED|\bhave QUOTED|\bhas all activated|\bhas (?:the|that|those|each) abilit|\bhave (?:each|the) abilit"),
    ("ability removal (loses / can't have)", "6", r"\blos(?:es|e) (?:all abilities|" + KW_LIST + r")|\bcan't have or gain\b|\blose all abilities\b"),
    ("P/T setting (base P/T N/N, becomes N/N)", "7b", r"base power and toughness|\bbase power\b|\bhas base\b|\bare \d+/\d+\b|\bis (?:a )?\d+/\d+\b|power and toughness are each \d"),
    ("P/T modification (gets / get +N/+N)", "7c", r"\bgets? [+\-−][\dX]+/[+\-−][\dX]+|\bgets? \+\d+/\+\d+ for each|\bget an additional"),
    ("P/T switch", "7d", r"\bswitch\b"),
    ("cost change (costs {N} more/less, without paying, rather than)", "601.2f", r"\bcosts? (?:\{[^}]+\})+ (?:more|less)|\bcosts? [\w ]+ (?:more|less) to (?:cast|activate)|\bless to cast\b|\bmore to cast\b|rather than pay|without paying (?:its|their) mana cost|\bcost \{[^}]+\} more\b|\bmay pay [^.]* rather than|\bcost(?:s)? (?:X|N) (?:more|less)|reduce|\bcost(?:s)? an additional\b"),
    ("restriction (can't attack / block / be blocked / cast / activate / be countered ...)", "rules (508/509/601)", r"\bcan't\b|\bcannot\b|doesn't untap|don't untap|can block only|can attack only|only (?:one|two) [\w ]+ can|no more than"),
    ("requirement (attacks / blocks each combat if able, must be blocked)", "508.1d/509.1c", r"\bif able\b|\bmust be blocked\b|\bmust attack\b|\bmust block\b"),
    ("as-though permission (cast from graveyard / flash / play from top / spend mana as though)", "601.3 / 609.4b", r"\bas though\b|\bmay (?:cast|play) [^.]*? from (?:your graveyard|exile|the top|among|your hand|outside)|\bmay play lands from\b|\bmay look at the top card\b|\bmay cast [^.]*(?:any time you could cast an instant|as though)|\bany time you could cast an instant\b|\bplay with the top card\b"),
    ("rule change (hand size, extra land, untap, life, mana pool, win/lose)", "613.11 / misc", r"\bmaximum hand size\b|\badditional land\b|\buntap (?:all|each|those)[^.]* during each other player's untap step|\bdon't lose (?:this|unspent) mana|\bmana pools?\b|\bcan't (?:win|lose) the game|\bdamage doesn't\b|\blife total can't change|\bspells and abilities can't\b|\bplayers can't\b|\beach player can't\b|\bno more than\b|\bskip (?:your|their)\b|\buntap step\b|\bmay play an additional\b|\bthe top card of your library\b"),
    ("replacement (If ... would ..., ... instead / prevent)", "614 / 615", r"\bwould\b[^.]*\binstead\b|\binstead\b[^.]*\bwould\b|\bprevent\b|\bwould\b"),
    ("enters tapped / with counters / as (614.1c-e, 614.12)", "614.1c-e", r"\benters? (?:tapped|with|as|attacking|transformed|face down|under)\b|^As SELF enters\b|\bAs [\w ]+ enters\b|\benter tapped\b|\benter with\b"),
    ("mana-related (spend only / tap for additional / mana of any type)", "106 / 605", r"\bspend (?:this|only|mana)\b|\bmana (?:of any (?:type|color)|abilities)\b|\btapped for mana\b|\bproduces?\b"),
    ("player effects (hexproof / protection / can't be targeted for players)", "613.10", r"\byou have\b|\bplayers have\b|\beach (?:player|opponent) has\b|\byour opponents have\b|\byou (?:can't|don't) lose\b"),
    ("activation/casting restriction text on SELF (Activate only ...)", "602.5 / 601.2", r"^(?:Activate|Cast) (?:only|this|SELF)\b|^Any player may activate\b|^You may activate\b|^SELF can be your commander"),
    ("deck-construction / outside-game (any number, commander, companion)", "100 / 903", r"\bdeck\b|\bcommander\b|\boutside the game\b|\bopening hand\b|\bstart(?:ing)? the game\b|\bbegin the game\b"),
]
STATIC_RES = [(l, lay, re.compile(r)) for l, lay, r in STATIC_CLASSES]


def static_stats(abls: list[Ability]) -> dict:
    c = collections.Counter()
    n = 0
    n_unclassified = 0
    unclassified = collections.Counter()
    ex: dict[str, str] = {}
    multi = collections.Counter()
    oneshot_layer = collections.Counter()
    for a in abls:
        t = a.effect
        if not t:
            continue
        if is_static(a):
            n += 1
            hit = [l for l, _, rx in STATIC_RES if rx.search(t)]
            multi[min(len(hit), 4)] += 1
            if not hit:
                n_unclassified += 1
                unclassified[gen_norm(t)[:90]] += 1
            for l in hit:
                c[l] += 1
                if len(a.raw) < 150:
                    ex.setdefault(l, f"{a.name}: {a.raw}")
        elif is_one_shot(a):
            for l, lay, rx in STATIC_RES[:12] + STATIC_RES[13:15]:
                if rx.search(t):
                    oneshot_layer[l] += 1
    return {"static abilities": n, "classes": dict(c.most_common()), "classes per ability": dict(sorted(multi.items())),
            "unclassified": n_unclassified, "unclassified top": dict(unclassified.most_common(25)),
            "examples": ex, "layer map": {l: lay for l, lay, _ in STATIC_CLASSES},
            "continuous effects created by one-shot abilities": dict(oneshot_layer.most_common())}


# ============================================================== replacement and prevention (section 8)

def replacement_stats(abls: list[Ability]) -> dict:
    would = collections.Counter()
    would_ex: dict[str, str] = {}
    prevent = collections.Counter()
    skip = collections.Counter()
    etb = collections.Counter()
    instead_kind = collections.Counter()
    alt = collections.Counter()
    ex: dict[str, str] = {}
    for a in abls:
        t = a.effect
        if not t:
            continue
        for m in re.finditer(r"\bwould (?:be )?(\w+)(?: (\w+))?", t):
            v = m.group(1)
            nxt = m.group(2) or ""
            key = {"put": "be put into a graveyard / zone" if "graveyard" in t[m.start():m.start() + 60] else "put counters / put onto",
                   "deal": "deal damage", "dealt": "be dealt damage", "die": "die", "draw": "draw a card", "gain": "gain life",
                   "lose": "lose life / lose the game", "enter": "enter (the battlefield)", "destroyed": "be destroyed",
                   "create": "create tokens", "exiled": "be exiled", "countered": "be countered", "leave": "leave the battlefield",
                   "discard": "discard", "mill": "mill", "cast": "cast", "win": "win the game", "untap": "untap",
                   "produce": "produce mana", "add": "add mana", "begin": "begin (turn/step)", "sacrificed": "be sacrificed",
                   "returned": "be returned", "search": "search", "attack": "attack", "become": "become (tapped/target)",
                   "turned": "be turned face up", "created": "create tokens", "reveal": "reveal", "pay": "pay", "remove": "remove counters"}.get(v, v + " " + nxt)
            would[key] += 1
            if len(a.raw) < 170:
                would_ex.setdefault(key, f"{a.name}: {a.raw}")
        for m in re.finditer(r"\b[Pp]revent (all|the next \S+|that|any|X of that|half|up to \S+)\b[^.]*", t):
            k = "prevent " + gen_norm(m.group(1))
            if "combat damage" in m.group(0):
                k += " (combat)"
            prevent[k] += 1
            ex.setdefault(k, f"{a.name}: {a.raw}")
        if re.search(r"damage can't be prevented|can't be prevented", t):
            prevent["damage can't be prevented"] += 1
        for m in re.finditer(r"(?i)\bskips? (?:your |their |his or her |that player's |that |the )?(next |each )?(\w+(?: \w+)?)", t):
            skip["skip " + (m.group(1) or "") + m.group(2)] += 1
        for k, rx in (("enters tapped", r"\benters? tapped\b(?! and attacking)(?! unless)"), ("enters tapped unless <condition>", r"enters? tapped unless"),
                      ("enters with N counters", r"\benters? with (?:a|an|one|two|three|four|five|six|seven|eight|nine|ten|\d+) [^.]*?counters?\b"),
                      ("enters with X / variable counters", r"\benters? with (?:X|that many|a number of|twice|an additional|additional) [^.]*?counters?\b"),
                      ("As SELF enters, choose ...", r"^As (?:SELF|[\w ]+) enters,? [^.]*\b(?:choose|chooses|name|note|secretly|you may|reveal|pay|look)"),
                      ("As ... enters (other)", r"^As (?:SELF|[\w ]+) enters(?!,? [^.]*\b(?:choose|chooses|name|note|secretly|you may|reveal|pay|look))"),
                      ("enters as a copy", r"\benters? as a copy\b|enter the battlefield as a copy|have SELF enter as a copy"),
                      ("enters attacking / tapped and attacking", r"\benters? (?:tapped and )?attacking\b"),
                      ("enters transformed / face down", r"\benters? (?:transformed|face down|face-down)\b"),
                      ("enters under <player>'s control", r"\benters? under\b"),
                      ("ETB with additional counter (on others)", r"\benters? with an additional\b|\benter with an additional\b")):
            if re.search(rx, t):
                etb[k] += 1
                ex.setdefault(k, f"{a.name}: {a.raw}")
        if "instead" in t:
            if re.search(r"\bwould\b", t):
                instead_kind["replacement: If ... would ..., ... instead (614.1a)"] += 1
            elif re.search(r"\bIf\b[^.]*,[^.]*\binstead\b", t):
                instead_kind["conditional override: X. If <cond>, Y instead"] += 1
                ex.setdefault("override", f"{a.name}: {a.raw}")
            elif re.search(r"[Cc]hoose [^.]* instead", t):
                instead_kind["modal upgrade: choose one ... instead"] += 1
            else:
                instead_kind["other instead"] += 1
                ex.setdefault("other instead", f"{a.name}: {a.raw}")
        for k, rx in (("rather than pay (alternative cost)", r"rather than pay"), ("without paying its mana cost", r"without paying"),
                      ("as an additional cost", r"(?i)as an additional cost"), ("If ... would ... the next time (one-shot shield)", r"the next time [^.]* would")):
            if re.search(rx, t):
                alt[k] += 1
    return {"would verbs": dict(would.most_common()), "would examples": would_ex, "prevent": dict(prevent.most_common(25)),
            "skip": dict(skip.most_common(20)), "enters": dict(etb.most_common()), "instead kinds": dict(instead_kind.most_common()),
            "alternative/extra cost and shields": dict(alt.most_common()), "examples": ex}


# ============================================================== choices (section 9)

CHOICES: list[tuple[str, str, str]] = [
    ("you may (optional action)", "YesNo", r"\b(?:you|that player|they|its controller|target player|each player|an opponent) may\b(?! (?:cast|play)[^.]*as though)"),
    ("target (declared on cast / activation)", "Targets", r"\btarget\b"),
    ("a / an <object> (chosen on resolution: sacrifice a creature)", "ChooseObjects", r"\b(?:sacrifices?|discards?|exiles?|returns?|destroys?|taps?|untaps?|reveals?) (?:a|an|another|one|two|three|N) (?!target)"),
    ("choose <object> (choose a creature / player / card)", "ChooseObjects", r"\bchooses? (?:a|an|another|up to|any number of|one or more|two|three|N|target) (?!color|creature type|card type|number|nonland card name|card name|land type|basic land type|mode|player|opponent)"),
    ("choose a player / opponent", "ChoosePlayer", r"\bchooses? (?:a|an|target|another) (?:player|opponent)\b"),
    ("choose a color", "ChooseFromSet(color)", r"\bchooses? a color\b|\bcolor of (?:your|their) choice\b|\bchosen color\b"),
    ("choose a creature type / card type / land type", "ChooseFromSet(type)", r"\bchooses? (?:a|an) (?:creature|card|land|basic land|artifact|nonbasic land) type\b|\btype of your choice\b|\bchosen (?:creature )?type\b"),
    ("choose / name a card name", "ChooseFromSet(name)", r"\bchooses? (?:a|an) (?:\w+ )?card name\b|\bname a card\b|\bnames? a\b"),
    ("choose a number / pay any amount / X", "ChooseNumber", r"\bchooses? a number\b|\bany amount of\b|\bchoose any number\b(?! —)|\bsecretly choose\b|\bX\b"),
    ("any number of <objects>", "ChooseObjects(any number)", r"\bany number of\b"),
    ("up to N <objects>", "ChooseObjects(up to)", r"\bup to (?:one|two|three|four|five|six|seven|X|\d+)\b"),
    ("in any order", "Order", r"\bin any order\b"),
    ("in a random order / at random", "Random", r"\bin a random order\b|\bat random\b"),
    ("divided / distributed as you choose", "Divide", r"\bdivided (?:as you choose|evenly)\b|\bdistribute\b"),
    ("an opponent / target opponent / that player chooses", "OtherPlayerChooses", r"\b(?:an opponent|target opponent|that player|target player|each opponent|each player|defending player|its controller|the chosen player|its owner) chooses\b|\bof (?:their|his or her|that player's|the opponent's) choice\b"),
    ("for each player / each player chooses (simultaneous / APNAP)", "PerPlayer", r"\beach player chooses\b|\bstarting with you\b|\bin turn order\b|\bfor each player\b"),
    ("vote", "Vote", r"\bvotes?\b"),
    ("separate into piles", "Piles", r"\bpiles?\b"),
    ("flip a coin / roll a die (random, no decision)", "Random", r"\bflips? (?:a|two|\w+) coins?\b|\brolls? (?:a|two|\w+) (?:d\d+|(?:six|four|twenty)-sided di(?:e|ce)|dice|die)\b"),
    ("choose new targets / change the target", "Retarget", r"\bnew targets?\b|\bchange the targets?\b|\bchange the target\b"),
    ("guess / bid / secretly", "Hidden choice", r"\bguess|\bbid\b|\bsecretly\b"),
    ("pay {cost} or ... (unless pays / pays any amount)", "Payment decision", r"\bunless (?:that player|its controller|they|he or she|you|any player|an opponent|target player|its owner|that spell's controller) pays?\b|\bmay pay\b|\bpays? any amount\b"),
    ("your choice of / of your choice", "ChooseFromList", r"\byour choice of\b|\bof your choice\b"),
    ("order blockers / assign damage", "Assign", r"\bassigns? (?:its )?combat damage\b|\bdamage assignment order\b"),
]


def choice_stats(abls: list[Ability], faces: list[dict]) -> dict:
    c = collections.Counter()
    cards = collections.defaultdict(set)
    ex = {}
    for a in abls:
        t = (a.effect or "") + " " + (a.cost or "")
        if not t.strip():
            continue
        for l, _, rx in CHOICES:
            k = len(re.findall(rx, t, re.I))
            if k:
                c[l] += k
                cards[l].add(a.card)
                if len(a.raw) < 160:
                    ex.setdefault(l, f"{a.name}: {a.raw}")
    modal = collections.Counter()
    modal_ex = {}
    for f in faces:
        text = inventory.REMINDER.sub("", f["oracle_text"])
        text = self_normalize(text, f["face_name"])
        for m in re.finditer(r"(?:^|[,.] |— )((?:[Cc]hoose|[Aa]n opponent chooses|[Tt]arget opponent chooses) (?:one|two|three|four|five|one or more|one or both|any number|up to \w+|\w+ —)[^—\n]*?)(?: —|\n|$)", text, re.M):
            h = gen_norm(m.group(1))
            h = re.sub(r"\s+", " ", h).strip(" .")
            modal[h] += 1
            modal_ex.setdefault(h, f["face_name"])
        if re.search(r"^\+ ?(\{[^}]+\})+ —", text, re.M):
            modal["Spree: + {cost} — modes (pay per chosen mode)"] += 1
            modal_ex.setdefault("Spree: + {cost} — modes (pay per chosen mode)", f["face_name"])
        if re.search(r"^Escalate\b", text, re.M):
            modal["Escalate (pay per extra mode)"] += 1
            modal_ex.setdefault("Escalate (pay per extra mode)", f["face_name"])
        if re.search(r"^Entwine\b", text, re.M):
            modal["Entwine (pay to choose all modes)"] += 1
            modal_ex.setdefault("Entwine (pay to choose all modes)", f["face_name"])
        if re.search(r"Tiered\b", text):
            modal["Tiered (pay per chosen tier)"] += 1
        if f["layout"] in ("split", "adventure", "modal_dfc", "prepare", "flip", "transform") and f["face"] == 0:
            modal[f"(frame-level choice of face: {f['layout']})"] += 1
    return {"phrasings": dict(c.most_common()), "cards": {k: len(v) for k, v in cards.items()}, "examples": ex,
            "decision type": {l: d for l, d, _ in CHOICES}, "modal heads": dict(modal.most_common()), "modal examples": modal_ex}


# ============================================================== constructs that resist composition (section 10)

ODDITIES: list[tuple[str, str, str]] = [
    # label, what the engine needs, regex over the face's oracle text (reminder removed)
    ("subgame (Shahrazad, 729)", "nested game state", r"\bsubgame\b"),
    ("ante (407)", "ante zone, ownership change", r"\bante\b"),
    ("restart the game (Karn Liberated, 727)", "game restart keeping some cards", r"\brestart the game\b"),
    ("control another player's turn (Mindslaver, 723)", "one player makes another's decisions", r"\bcontrol (?:target|that|each) (?:player|opponent) during\b|\bcontrols? (?:target|that) player\b|\byou control (?:that|target) player\b|\bcontrol that player until\b|\bYou control that player\b"),
    ("extra turns", "turn order queue", r"\bextra turn\b"),
    ("additional phases / steps (extra combat, extra upkeep)", "dynamic turn structure", r"\badditional (?:combat|beginning|main|upkeep|end) (?:phase|step)\b|\bafter this (?:phase|main phase)\b"),
    ("skip turns / phases / steps", "turn structure edits", r"\bskips? (?:your|their|his or her|that player's|the|each|that)? ?(?:next )?(?:turn|untap step|upkeep|draw step|combat|combat phase|main phase|end step)"),
    ("end the turn (Time Stop, 724)", "exile stack, jump to cleanup", r"\bend the turn\b"),
    ("win / lose the game (alternate win/loss)", "game result effects", r"\b(?:win|wins|lose|loses) the game\b"),
    ("can't win / can't lose the game", "game-result prohibition", r"\bcan't (?:win|lose) the game\b|don't lose the game|doesn't lose the game"),
    ("exchange control", "simultaneous control swap", r"\bexchange control\b"),
    ("exchange life totals / life total becomes", "life setting", r"\bexchange life totals?\b|\blife total becomes\b|\blife totals? become\b|\bexchange (?:your|their) life total"),
    ("text-changing (Mind Bend, 612)", "layer 3 text substitution", r"\bchange the text\b|\breplacing all instances\b|\btext-changing\b"),
    ("switch power and toughness", "layer 7d", r"\bswitch [^.]*power and toughness\b"),
    ("lose all abilities / Humility-style", "layer 6 + 7b dependency", r"\blos(?:e|es) all abilities\b"),
    ("becomes a copy / enters as a copy", "layer 1 copiable values", r"\b(?:becomes?|enters? as|enter as|as) a copy of\b|\bcopy of (?:target|another|a|enchanted|equipped|that|it)\b"),
    ("Opalescence/March-style type change with P/T by mana value", "layer 4 + 7b dependency", r"(?:is|are) (?:an? )?[\w ]*creatures? (?:in addition to (?:its|their) other types )?(?:and )?(?:has|have|with) (?:base )?power and (?:base )?toughness (?:each )?equal to (?:its|their) mana value"),
    ("dexterity (flip a card onto the table)", "physical action; not simulable as written", r"\bfrom a height of\b"),
    ("the game is a draw", "draw result", r"\bgame is a draw\b"),
    ("has the full text of another card (Volrath's Shapeshifter)", "layer 1/3 text import", r"\bfull text of\b"),
    ("legend rule doesn't apply / rule suppression", "SBA modification", r"\blegend rule\b"),
    ("affects cards in every zone (Painter's Servant, 'cards that aren't on the battlefield')", "characteristics outside the battlefield", r"\bcards that aren't on the battlefield\b|\bcards you own that aren't on the battlefield\b|\bin every zone\b|\bin all zones\b"),
    ("remove from combat / rearrange blocks (Sorrow's Path)", "combat state surgery", r"\bremove [^.]* from combat\b|\bblocks all creatures the other\b"),
    ("is every creature type / all colors", "type / color universes", r"\bevery creature type\b|\ball colors\b|\bis all creature types\b"),
    ("word-of-command style: you control a player / play their cards", "cast with another player's resources", r"\bcast (?:it|that card|spells?|cards?) [^.]* (?:from|in) (?:an opponent's|target opponent's|that player's|their) (?:hand|library|graveyard)|\bplay (?:it|that card|cards?|lands?) [^.]* from (?:an opponent's|that player's|target opponent's)"),
    ("you may cast/play cards exiled by another player (theft-cast)", "permission across ownership", r"\bmay (?:cast|play) (?:that card|it|them|those cards|cards exiled|the exiled)\b[^.]*\b(?:spend mana as though|for as long as|this turn)"),
    ("spend mana as though any type", "mana payment relaxation", r"\bas though it were mana of any (?:type|color)\b|\bmana of any type can be spent\b"),
    ("cast as though it had flash (timing permission)", "timing override", r"\bas though (?:it|they) had flash\b|\bany time you could cast an instant\b"),
    ("play with top card revealed / look at top card any time", "persistent hidden-info exception", r"\bplay with the top card\b|\blook at the top card of your library any time\b|\btop card of (?:your|each player's) library (?:is revealed|revealed)\b"),
    ("cast from graveyard / exile as a permission (static)", "zone-cast permission", r"\bmay (?:cast|play) [^.]* from (?:your graveyard|exile)\b"),
    ("outside the game / wish / sideboard", "outside-game zone", r"\boutside the game\b"),
    ("opening hand / start of game / deck-building", "pre-game actions", r"\bopening hand\b|\bbegin the game\b|\bstart the game\b|\bstarting hand\b|\bshuffle your deck to start\b|\bdeck\b(?!s)"),
    ("commander / command zone", "commander rules (903)", r"\bcommander\b|\bcommand zone\b"),
    ("conspiracy / draft matters", "draft-time effects", r"\bdraft(?:ed|ing)?\b|\bas you draft\b|\bhidden agenda\b|\bdouble agenda\b"),
    ("vote / council's dilemma / will of the council", "multi-player vote", r"\bvotes?\b"),
    ("piles (Fact or Fiction)", "partition decision", r"\bpiles?\b"),
    ("bid / auction", "multi-round bidding", r"\bbids?\b|\bhigh bid\b|\bbidding\b"),
    ("guess / secretly choose", "hidden simultaneous choice", r"\bguess(?:es)?\b|\bsecretly\b"),
    ("coin flip", "RNG; 'win the flip' outcomes", r"\bflips? (?:a|two|\w+) coins?\b|\bcoin\b"),
    ("die roll (d4/d6/d20 results tables)", "RNG with result tables", r"\brolls? (?:a|two|\w+) (?:d\d+|(?:six|four|twenty)-sided di(?:e|ce)|dice|die)\b|^\d+(?:—\d+|\+)? \|"),
    ("dungeon / venture / initiative", "dungeon card & room graph", r"\bventure into the dungeon\b|\binitiative\b|\bdungeon\b"),
    ("monarch", "player designation", r"\bmonarch\b"),
    ("day / night", "game-wide designation", r"\bdaybound\b|\bnightbound\b|\bit becomes (?:day|night)\b|\bday and night\b|\bif it's (?:day|night)\b"),
    ("the Ring tempts you", "ring-bearer emblem state", r"\bRing tempts you\b|\bRing-bearer\b"),
    ("speed / max speed", "player counter-like stat", r"\bstart your engines\b|\bmax speed\b|\byour speed\b"),
    ("emblems", "command-zone emblems", r"\bemblem\b"),
    ("phasing", "phased-out status (702.26)", r"\bphases? (?:out|in)\b|\bphasing\b"),
    ("banding / bands with other", "combat damage assignment by defender", r"\bbanding\b|\bbands with\b"),
    ("assign combat damage as though not blocked / by toughness", "combat damage rule changes", r"\bas though it weren't blocked\b|\bassigns? combat damage equal to its toughness\b|\bassigns? (?:no )?combat damage\b|\bdamage assignment\b"),
    ("redirect damage / damage dealt to X is dealt to Y", "damage redirection replacement", r"\bis dealt to [^.]* instead\b|\bredirect\b|\bdeals? that damage to\b"),
    ("damage can't be prevented", "prevention suppression", r"\bdamage can't be prevented\b|\bcan't be prevented\b"),
    ("regeneration", "regeneration shields (701.19)", r"\bregenerat"),
    ("protection from <quality>", "DEBT checks (702.16)", r"\bprotection from\b"),
    ("split second / can't be responded", "priority restriction", r"\bsplit second\b|\bcan't cast spells or activate abilities\b"),
    ("mana burn / mana doesn't empty", "mana pool persistence", r"\bdon't lose (?:this|unspent)\b|\bmana pools? don't empty\b|\buntil end of turn, you don't lose\b|\bdoesn't empty\b"),
    ("players can't gain life / life total can't change", "life-change prohibitions", r"\bcan't gain life\b|\blife total can't change\b|\bcan't lose life\b"),
    ("you may look at face-down / hidden zones of others", "information rights", r"\blook at (?:target|each|an) (?:player's|opponent's) hand\b|\bplay with (?:their|your) hands? revealed\b|\blook at face-down\b"),
    ("Stifle-like: counter target activated/triggered ability", "abilities on the stack as objects", r"\bcounter target (?:activated|triggered)? ?(?:or triggered )?abilit"),
    ("copy a spell / ability (may choose new targets)", "stack-object copy", r"\bcopy (?:target|that|it|each|the next)[^.]*(?:spell|abilit)|\bcopy it\b|\bchoose new targets\b"),
    ("change targets / redirect spell", "retargeting", r"\bchange the targets?\b|\bnew target\b"),
    ("gain control of a spell / ability", "stack control change", r"\bgain control of target (?:spell|activated|triggered)"),
    ("damage doesn't wear off / damage remains", "cleanup rule changes", r"\bdamage (?:doesn't|don't) wear off\b|\bremoved from (?:it|them) during (?:its|the) cleanup\b"),
    ("untap during other players' untap steps (Seedborn)", "turn-based action changes", r"\bduring each other player's untap step\b"),
    ("max hand size / no maximum", "cleanup rule change", r"\bmaximum hand size\b"),
    ("additional land drops", "land-play counter", r"\badditional lands?\b(?! card)"),
    ("choice memory on an object (the chosen color/type/name, noted) — common", "per-object memory slots", r"\bnotes?\b|\bnoted\b|\bchosen number\b|\bthe chosen\b"),
    ("random selection from zone (at random)", "uniform random pick", r"\bat random\b"),
    ("more than one player decides in sequence (starting with you / in turn order)", "APNAP sequential decisions", r"\bstarting with you\b|\bin turn order\b"),
    ("goad / must attack another player", "attack requirements by player", r"\bgoad|\battacks? each combat if able and attacks? a player other than you\b"),
    ("multiplayer: attack left/right, range of influence", "multiplayer-only rules (8xx)", r"\bto your (?:left|right)\b|\bchosen direction\b|\bnearest opponent\b|\brange of influence\b"),
    ("time counters / suspend / vanishing / fading", "counter-driven delays", r"\btime counters?\b|\bsuspend\b|\bvanishing\b|\bfading\b"),
    ("face-down: morph / manifest / cloak / disguise / foretell / plot", "face-down objects (708)", r"\bface[- ]down\b|\bmanifest\b|\bcloak\b|\bdisguise\b|\bmorph\b|\bforetell\b|\bforetold\b|\bplot\b"),
    ("double-faced / transform / meld / day-night", "two-face objects (712)", r"\btransform\b|\bmeld\b|\bconvert\b"),
    ("mutate / merged permanents", "merged object stacks (702.140)", r"\bmutates?\b|\bmerged\b"),
    ("stickers / attractions / contraptions (Un-mechanics in paper)", "special decks", r"\bAttraction\b|\bContraption\b|\bsticker\b|\bvisit\b"),
    ("cards in hand with abilities that work from hand / reveal from hand", "ability-from-hidden-zone", r"\breveal (?:SELF|it|this card) from your hand\b|\bas long as SELF is in your hand\b|\bfrom your hand\b[^.]*\bactivate\b"),
    ("library manipulation by other player (Jace/Lim-Dul's Vault-like)", "reorder decisions on other libraries", r"\bthe top \w+ cards of target (?:player's|opponent's) library\b[^.]*\bin any order\b|\brearrange\b"),
    ("change zones of whole hands/graveyards (wheel, Timetwister)", "mass zone moves", r"\bshuffles? (?:their|his or her|your) hand and graveyard into\b|\bdiscards? (?:their|your) hand\b[^.]*\bdraws?\b|\bshuffles? (?:their|your) hand into\b"),
    ("ownership vs control oddities (you own but don't control, owner's control)", "owner/controller split", r"\byou own but don't control\b|\bunder (?:its|their) owners?'s? control\b|\bthat you own\b"),
    ("copy of a card in another zone / cast copy (Isochron, Spellbinder)", "cast copies without moving cards", r"\bcast (?:a|the) copy\b|\bcopy (?:of )?(?:the|that|an?) (?:exiled|imprinted) card\b|\bcopy the exiled card\b"),
    ("devotion / storm count / party (derived counts with rules)", "derived game counts", r"\bdevotion\b|\bstorm\b|\bparty\b"),
    ("Rooms / unlock / Case / solved / Class levels", "multi-state permanents", r"\bunlock\b|\bsolved\b|\bTo solve\b|\bLevel \d\b"),
    ("villainous choice / tempting offer / join forces", "named multiplayer offers", r"\bvillainous choice\b|\btempting offer\b|\bjoin forces\b|\bfriend or foe\b"),
    ("devour / exploit / emerge / champion (sacrifice-linked)", "linked sacrifice references", r"\bdevour\b|\bexploits?\b|\bemerge\b|\bchampion\b"),
    ("shuffle a card into a random position / library position N", "positional library inserts", r"\b(?:second|third|fourth|fifth|sixth|seventh|\w+th) from the top\b|\bposition\b|\bjust beneath the top\b"),
]
ODDITY_RES = [(l, need, re.compile(r, re.M | re.I)) for l, need, r in ODDITIES]


def oddity_stats(faces: list[dict]) -> dict:
    cards = collections.defaultdict(set)
    ex: dict[str, list[str]] = collections.defaultdict(list)
    for f in faces:
        text = self_normalize(inventory.REMINDER.sub("", f["oracle_text"]), f["face_name"])
        for l, _, rx in ODDITY_RES:
            if rx.search(text):
                if f["oracle_id"] not in cards[l] and len(ex[l]) < 3:
                    ex[l].append(f["name"])
                cards[l].add(f["oracle_id"])
    ranked = sorted(cards.items(), key=lambda kv: len(kv[1]))
    union = {}
    for k in (10, 25, 50, 100):
        u = set()
        for l, v in cards.items():
            if len(v) <= k:
                u |= v
        union[k] = len(u)
    return {"by construct": {l: len(v) for l, v in sorted(cards.items(), key=lambda kv: -len(kv[1]))},
            "examples": dict(ex), "needs": {l: n for l, n, _ in ODDITIES},
            "rarest": [(l, len(v)) for l, v in ranked], "cards in constructs with <= k cards": union,
            "cards with any construct": len(set().union(*cards.values())),
            "_cards": {l: v for l, v in cards.items()}}


# ============================================================== verbs: frequencies and coverage

def verb_stats(abls: list[Ability]) -> dict:
    ab_count = collections.Counter()
    card_sets = collections.defaultdict(set)
    occ = collections.Counter()
    fam = {l: f for l, _, f in VERBS}
    card_verbs: dict[str, set] = collections.defaultdict(set)
    all_cards = set()
    ex = {}
    for a in abls:
        all_cards.add(a.card)
        if not a.effect:
            continue
        seen = set()
        for lbl, rx, _ in VERB_RES:
            k = len(rx.findall(a.effect))
            if k:
                occ[lbl] += k
                seen.add(lbl)
                card_sets[lbl].add(a.card)
                if len(a.raw) < 140:
                    ex.setdefault(lbl, f"{a.name}: {a.raw}")
        for v in seen:
            ab_count[v] += 1
        card_verbs[a.card] |= seen
    by_cards = sorted(card_sets, key=lambda v: -len(card_sets[v]))
    with_verbs = [c for c in all_cards if card_verbs.get(c)]
    cov = {}
    chosen = set()
    for i, v in enumerate(by_cards, 1):
        chosen.add(v)
        if i in (1, 2, 3, 5, 10, 15, 20, 30, 40, 50, 60, 70, 80) or i == len(by_cards):
            covered = sum(1 for c in with_verbs if card_verbs[c] <= chosen)
            cov[i] = round(100 * covered / len(with_verbs), 1)
    fam_cards = collections.defaultdict(set)
    for v, cs in card_sets.items():
        fam_cards[fam[v]] |= cs
    return {"occurrences": dict(occ.most_common()), "abilities": dict(ab_count.most_common()),
            "cards": {v: len(card_sets[v]) for v in by_cards}, "family": fam, "examples": ex,
            "cards with >=1 verb": len(with_verbs), "all cards": len(all_cards), "coverage by top-k verbs": cov,
            "family cards": {k: len(v) for k, v in sorted(fam_cards.items(), key=lambda kv: -len(kv[1]))}}


# ============================================================== unexplained clause heads (grammar worklist)

def head_residue(abls: list[Ability]) -> dict:
    """First word of every clause after stripping subjects; heads not matched by any verb regex are residue."""
    subj = re.compile(r"^(?:(?:then|if [^,]*,|otherwise,|instead,|unless [^,]*,|until end of turn,|this turn,)\s*)*"
                      r"(?:(?:you|each player|each opponent|target (?:player|opponent)|that player|its controller|its owner|SELF|it|they|"
                      r"each of them|that creature|enchanted creature|equipped creature|creatures you control|an opponent)\s+)?(?:may\s+)?", re.I)
    c = collections.Counter()
    ex: dict[str, str] = {}
    tot = 0
    for a in abls:
        if not a.effect or not is_one_shot(a):
            continue
        for s in sentences(a.effect):
            for cl in re.split(r",? then |; |, and (?=[a-z]+ )", s):
                cl = subj.sub("", cl.strip())
                if not cl:
                    continue
                tot += 1
                if not any(rx.search(cl) for _, rx, _ in VERB_RES):
                    c[cl.split(" ")[0].lower().strip(",.")] += 1
                    if len(ex) < 60 and cl.split(" ")[0].lower() not in ex:
                        ex[cl.split(" ")[0].lower()] = f"{a.name}: {cl[:120]}"
    return {"clauses": tot, "unmatched heads": dict(c.most_common(40)), "unmatched total": sum(c.values()), "examples": ex}


# ============================================================== long tail estimate

def long_tail(abls: list[Ability], faces: list[dict], odd: dict, conds: dict) -> dict:
    """For each card, the rarity of its rarest 'feature' (verb label, oddity, normalized condition shape,
    clause-head residue). A card is long-tail if its rarest feature occurs on <= k cards."""
    feat_cards: dict[str, set] = collections.defaultdict(set)
    for l, cs in odd["_cards"].items():
        for c in cs:
            feat_cards["odd:" + l].add(c)
    for a in abls:
        if not a.effect:
            continue
        for lbl, rx, _ in VERB_RES:
            if rx.search(a.effect):
                feat_cards["verb:" + lbl].add(a.card)
        for label, rx in COND_INTRO[:6]:
            for mm in re.finditer(rx, (a.trigger + ", " + a.effect) if a.trigger else a.effect):
                feat_cards["cond:" + gen_norm(mm.group(1))].add(a.card)
        # rare keyword actions / effect words: any lowercase word appearing in <= 3 cards is a 'rare word' feature
        for w in set(re.findall(r"\b[a-z][a-z'\-]{3,}\b", a.effect)):
            feat_cards["word:" + w].add(a.card)
    card_min: dict[str, int] = {}
    card_feat: dict[str, str] = {}
    for feat, cs in feat_cards.items():
        n = len(cs)
        for c in cs:
            if c not in card_min or n < card_min[c]:
                card_min[c] = n
                card_feat[c] = feat
    total = len({f["oracle_id"] for f in faces})
    dist = {}
    for k in (1, 2, 3, 5, 10, 20, 50):
        dist[k] = sum(1 for v in card_min.values() if v <= k)
    cond_only = {}
    for k in (1, 3, 5):
        cond_only[k] = sum(1 for c, v in card_min.items() if v <= k and card_feat[c].startswith(("verb:", "odd:", "word:")))
    return {"cards": total, "cards whose rarest feature is on <= k cards": dist,
            "excluding condition-text uniqueness": cond_only}


# ============================================================== main

def main() -> None:
    faces, abls = load_abilities()
    res = {
        "faces": len(faces), "cards": len({f["oracle_id"] for f in faces}), "abilities": len(abls),
        "ability kinds": dict(collections.Counter(a.kind for a in abls).most_common()),
    }
    one_shot = [a for a in abls if is_one_shot(a)]
    statics = [a for a in abls if is_static(a)]
    res["clauses (one-shot effects)"] = clause_stats(one_shot)
    res["clauses (static abilities)"] = clause_stats(statics)
    res["verbs"] = verb_stats(abls)
    res["clause-head residue"] = head_residue(abls)
    res["selectors"] = selector_stats(abls)
    res["back-references"] = backref_stats(abls)
    res["amounts"] = amount_stats(abls)
    res["conditions"] = condition_stats(abls)
    res["durations"] = duration_stats(abls)
    res["statics"] = static_stats(abls)
    res["replacement"] = replacement_stats(abls)
    res["choices"] = choice_stats(abls, faces)
    odd = oddity_stats(faces)
    res["long tail"] = long_tail(abls, faces, odd, res["conditions"])
    odd.pop("_cards")
    res["oddities"] = odd
    if "--no-report" in sys.argv:  # only the dumps below: leave the local research report untouched
        print(f"not writing {OUT_JSON}")
    else:
        OUT_JSON.parent.mkdir(parents=True, exist_ok=True)
        OUT_JSON.write_text(json.dumps(res, indent=1, ensure_ascii=False, default=str))
        print(f"wrote {OUT_JSON}")
    if "--tables" in sys.argv:
        print_tables(res)
    if "--dump-triggers" in sys.argv:
        out = ROOT / ".cache/oracle/triggers.jsonl"
        rows = [{"oracle_id": a.card, "face": a.name, "kind": a.kind, "text": a.trigger} for a in abls if a.trigger]
        out.write_text("".join(json.dumps(r, ensure_ascii=False) + "\n" for r in rows), encoding="utf-8")
        print(f"wrote {out} ({len(rows)} trigger conditions)")
    if "--dump-conditions" in sys.argv:
        out = ROOT / ".cache/oracle/conditions.jsonl"
        out.write_text("".join(json.dumps(r, ensure_ascii=False) + "\n" for r in COND_DUMP), encoding="utf-8")
        print(f"wrote {out} ({len(COND_DUMP)} conditions)")
    if "--dump-amounts" in sys.argv:
        out = ROOT / ".cache/oracle/amounts.jsonl"
        out.write_text("".join(json.dumps(r, ensure_ascii=False) + "\n" for r in AMOUNT_DUMP), encoding="utf-8")
        print(f"wrote {out} ({len(AMOUNT_DUMP)} amount expressions)")
    if "--dump-nps" in sys.argv:
        out = ROOT / ".cache/oracle/noun_phrases.jsonl"
        out.write_text("".join(json.dumps(r, ensure_ascii=False) + "\n" for r in NP_DUMP), encoding="utf-8")
        print(f"wrote {out} ({len(NP_DUMP)} noun phrases)")


def table(d: dict, head=("item", "count"), n=60, extra=None) -> str:
    rows = [f"| {' | '.join(head)} |", "|" + "---|" * len(head)]
    for k, v in list(d.items())[:n]:
        cells = [str(k).replace("|", "\\|"), str(v)]
        if extra:
            cells += [str(e.get(k, "")).replace("|", "\\|") for e in extra]
        rows.append("| " + " | ".join(cells) + " |")
    return "\n".join(rows)


def print_tables(res: dict) -> None:
    def sec(title, obj):
        print(f"\n\n######## {title}\n")
        if isinstance(obj, dict):
            for k, v in obj.items():
                if isinstance(v, dict) and v and not any(isinstance(x, (dict, list)) for x in v.values()):
                    print(f"\n### {k}\n")
                    print(table(v, n=80))
                else:
                    print(f"\n### {k}: {json.dumps(v, ensure_ascii=False, default=str)[:3000]}")
        else:
            print(obj)
    for k, v in res.items():
        sec(k, v)


if __name__ == "__main__":
    main()
