"""The MGN projection as a mutable state: construction from a record's start, delta application,
consistency checks and the explicit form that checkpoints hash.

Ordered zones are kept top-first internally for every zone; the spec's listing orders
(graveyard oldest-first, stack bottom-first) are produced only when a full state is written.
"""
from __future__ import annotations

from copy import deepcopy
from dataclasses import dataclass, field
from typing import Literal
import re

from .abilities import ability_object_refs, source_free_rule
from . import copy_definition
from .lots import normalize_state_lots
from .keywords import normalize_keyword
from .vocabulary import CARD_ID, EMBLEM_ID, STACK_ID, TOKEN_ID, SEAT_ZONES, SHARED_ZONES


TIER1_OBJECT = {"owner", "controller", "copy_of", "copy_face", "copy_definition", "face", "fused", "token", "emblem", "kind", "source", "ability", "targets", "modes",
                "x", "division", "cast", "chosen", "attacking", "blocked", "blocking", "in_combat"}
TIER2_OBJECT = {"tapped", "flipped", "face_down", "phased_out", "counters", "damage", "attached_to", "power",
                "toughness", "colors", "types", "supertypes", "subtypes", "keywords", "granted", "designations", "level"}
TIER1_PLAYER = {"life", "left"}
TIER2_PLAYER = {"counters", "mana", "mana_restricted", "speed", "max_hand_size", "designations"}
GAME_FACTS = {"monarch", "initiative", "day_night"}  # all tier 2
OBJECT_DEFAULTS = {"copy_of": None, "face": 0, "fused": False, "targets": [], "modes": [], "division": [], "chosen": {},
                   "attacking": None, "blocked": False, "blocking": [], "tapped": False, "flipped": False,
                   "face_down": False, "phased_out": False, "counters": {}, "damage": 0, "attached_to": None,
                   "granted": [], "designations": []}
PLAYER_DEFAULTS = {"left": False, "counters": {}, "mana": {}, "mana_restricted": [], "speed": None,
                   "max_hand_size": 7, "designations": []}
GAME_DEFAULTS = {"monarch": None, "initiative": None, "day_night": None}
# the `about` members each pending kind carries (spec section 5); kinds not listed carry none
ABOUT = {
    "bottom": {"count"}, "discard": {"count"}, "assign_combat_damage": {"creature"},
    "order_triggers": {"abilities"}, "legend_rule": {"objects"}, "arrange": {"object", "cards"},
    "order_cards": {"cards", "zone"}, "separate": {"object", "cards"}, "choose_value": {"object", "of"},
}
for _kind in ("choose_replacement", "choose_targets", "choose_modes", "change_targets", "optional", "choose_objects",
              "choose_player", "choose_number", "choose_mana", "divide", "pile", "attach", "choose_defender",
              "library_destination", "search", "choose_alternative", "pay", "cast_during_resolution",
              "spell_destination", "holders", "coin_call", "command_zone"):
    ABOUT[_kind] = {"object"}


@dataclass(frozen=True)
class Problem:
    severity: str  # "error" or "warning"
    code: str
    path: str
    message: str

    def __str__(self):
        return f"{self.severity} {self.code} {self.path or '/'}: {self.message}"


def is_ordered(zone: str) -> bool:
    return zone == "stack" or zone.endswith(".library") or zone.endswith(".graveyard")


def zone_names(seats) -> list[str]:
    names = []
    for seat in seats:
        names.extend(f"{seat}.{zone}" for zone in SEAT_ZONES)
    names.extend(SHARED_ZONES)
    return names


@dataclass
class State:
    seats: list[str]
    tier: int
    cards: dict
    turn: dict
    pending: dict | None
    result: dict
    players: dict = field(default_factory=dict)
    game: dict = field(default_factory=dict)
    zones: dict = field(default_factory=dict)  # every zone, top-first where ordered
    objects: dict = field(default_factory=dict)  # non-default facts only
    historical: dict = field(default_factory=dict)  # recorded definitions, never live facts
    departed: dict = field(default_factory=dict)  # explicit never-live-at-a-boundary token introductions
    # Frozen generated spell-copy characteristics, or an ability with no
    # characteristics (405.4, 707.10). Never infer a spell from its source card.
    # None records an unavailable basis; later source changes cannot repair its history.
    copy_bases: dict[str, tuple[str, int | Literal["fused"]] | dict | Literal["ability"] | None] = field(default_factory=dict)
    # Unchanged-donor definitions checked against explicit definitions at L0.
    copy_authentications: dict = field(default_factory=dict)
    tokens_created: int = 0
    stack_created: int = 0
    emblems_created: int = 0
    # insertions of this delta into ordered zones, for the ambiguity rule and shuffles
    _inserted: dict = field(default_factory=dict)
    # objects this delta's zone reorders name, placed after a shuffle of the same transition
    _reordered: dict = field(default_factory=dict)

    # ----- construction -----

    @classmethod
    def from_record(cls, record: dict, problems: list[Problem]) -> "State":
        seats = list(record["game"]["seats"])
        start = record["start"]
        if start["kind"] == "decks":
            return cls._from_decks(record, seats, problems)
        return cls._from_position(record, seats, problems)

    @classmethod
    def from_observation(cls, record: dict, problems: list[Problem], previous: "State | None" = None) -> "State":
        """Load a later full projection with only its recorded history as context.

        A ceased token may remain the source of an ability (113.7a, 704.5d).
        This is not a constructor for extra premises in an initial position.
        """
        return cls._from_position(record, list(record['game']['seats']), problems, previous)

    @classmethod
    def _from_decks(cls, record, seats, problems) -> "State":
        chooser = record.get("match", {}).get("chooser")
        outcomes = record["start"].get("random", [])
        starters = [o for o in outcomes if o["kind"] == "starting_player"]
        if chooser is None:
            if len(starters) != 1:
                problems.append(Problem("error", "start", "/start", "a decks start needs match.chooser or exactly one starting_player outcome"))
                chooser = seats[0]
            else:
                chooser = starters[0]["result"]
        elif starters:
            problems.append(Problem("error", "start", "/start/random", "match.chooser is given, so no starting_player outcome is needed"))
        if chooser not in seats:
            problems.append(Problem("error", "unknown_reference", "/start", f"chooser {chooser!r} is not a seat"))
        state = cls(seats=seats, tier=record["tier"], cards=record["cards"],
                    turn={"number": 0, "step": "pregame", "active": None},
                    pending={"seat": chooser, "kind": "choose_first"}, result={"kind": "unfinished"})
        state.players = {seat: {"life": record["game"]["life"]} for seat in seats}
        state.zones = {name: [] for name in zone_names(seats)}
        for card_id, card in record["cards"].items():
            if card.get("from", "main") == "main":
                state.zones[f"{card['owner']}.library"].append(card_id)
            elif card.get("from") == "command":
                state.zones["command"].append(card_id)  # a commander begins in the command zone (903.6)
                if record["game"].get("format") != "commander":
                    problems.append(Problem("error", "start", f"/cards/{card_id}/from", "a card in the command zone as the game begins needs game.format commander (903.6)"))
        for outcome in outcomes:
            if outcome["kind"] != "starting_player":
                problems.append(Problem("error", "start", "/start/random", f"only a starting_player outcome belongs in start.random, not {outcome['kind']}"))
        return state

    @classmethod
    def _from_position(cls, record, seats, problems, previous=None) -> "State":
        given = record["start"]["state"]
        turn = deepcopy(given["turn"])
        if turn.get("occurrence") == 1:
            del turn["occurrence"]  # the default, written out only in the explicit form (spec 3.1)
        state = cls(seats=seats, tier=record["tier"], cards=record["cards"], turn=turn,
                    pending=deepcopy(given["pending"]), result=deepcopy(given["result"]))
        if previous is not None:
            state.copy_bases = deepcopy(previous.copy_bases)
            state.copy_authentications = deepcopy(previous.copy_authentications)
            state.departed = deepcopy(previous.departed)
            state.historical = deepcopy(previous.historical)
            state.historical.update(deepcopy(previous.objects))
            state.tokens_created = previous.tokens_created
            state.stack_created = previous.stack_created
            state.emblems_created = previous.emblems_created
        path = "/start/state"
        # An explicitly labelled historical incarnation is never a live object.
        # Its frozen definition remains available to later delayed abilities even
        # when the physical source changes zone or acquires different values.
        for label, historical in record["start"].get("causal", {}).get("objects", {}).items():
            owner = historical.get("owner") or record["cards"].get(historical.get("card"), {}).get("owner")
            state.historical[label] = {"copy_definition": deepcopy(historical["definition"]), "owner": owner}
        for seat in seats:
            if seat not in given["players"]:
                problems.append(Problem("error", "start", f"{path}/players", f"seat {seat} has no player entry"))
        for seat, facts in given["players"].items():
            if seat not in seats:
                problems.append(Problem("error", "unknown_reference", f"{path}/players/{seat}", "not a seat of this game"))
                continue
            state.players[seat] = {}
            for fact, value in facts.items():
                state._set_player(seat, fact, value, f"{path}/players/{seat}/{fact}", problems)
        for fact, value in given.get("game", {}).items():
            state._set_game(fact, value, f"{path}/game/{fact}", problems)
        state.zones = {name: [] for name in zone_names(seats)}
        placed = {}
        for zone, members in given["zones"].items():
            if zone not in state.zones:
                problems.append(Problem("error", "unknown_reference", f"{path}/zones/{zone}", "not a zone of this game"))
                continue
            for member in members:
                if member in placed:
                    problems.append(Problem("error", "start", f"{path}/zones/{zone}", f"{member} is also in {placed[member]}"))
                placed[member] = zone
            if zone.endswith(".graveyard") or zone == "stack":
                members = list(reversed(members))  # spec lists graveyards oldest-first and the stack bottom-first
            state.zones[zone] = list(members)
        for card_id, card in record["cards"].items():
            if card_id not in placed and card.get("from", "main") in ("main", "command"):
                problems.append(Problem("error", "start", f"{path}/zones", f"card {card_id} is in no zone"))
            if card_id in placed and card.get("from") == "sideboard":
                problems.append(Problem("error", "start", f"{path}/zones", f"sideboard card {card_id} is in a zone; it is outside the game until brought in"))
        for member, zone in placed.items():
            if member in record["cards"]:
                continue
            if TOKEN_ID.match(member):
                if previous is not None and member in previous.historical and not previous.exists(member):
                    problems.append(Problem("error", "start", f"{path}/zones/{zone}", "a departed token identity cannot become live again"))
                state.tokens_created = max(state.tokens_created, int(member[1:]))
                if "token" not in given.get("objects", {}).get(member, {}):
                    problems.append(Problem("error", "start", f"{path}/objects/{member}", "a token needs its definition"))
            elif EMBLEM_ID.match(member):
                state.emblems_created = max(state.emblems_created, int(member[1:]))
                if zone != "command":
                    problems.append(Problem("error", "start", f"{path}/zones/{zone}", f"emblem {member} outside the command zone (114.2)"))
                if "emblem" not in given.get("objects", {}).get(member, {}):
                    problems.append(Problem("error", "start", f"{path}/objects/{member}", "an emblem needs its definition (emblem)"))
            elif STACK_ID.match(member):
                state.stack_created = max(state.stack_created, int(member[1:]))
                if zone not in ("stack", "exile"):
                    problems.append(Problem("error", "start", f"{path}/zones/{zone}", f"generated spell object {member} outside the stack or prepared exile"))
                if "kind" not in given.get("objects", {}).get(member, {}):
                    problems.append(Problem("error", "start", f"{path}/objects/{member}", "a stack object that is not a card needs its kind"))
            else:
                problems.append(Problem("error", "unknown_reference", f"{path}/zones/{zone}", f"{member} is not a card of this record"))
        if previous is not None:
            new_tokens = sorted(int(object_id[1:]) for object_id in set(placed) | set(given.get("departed", {}))
                                if TOKEN_ID.fullmatch(object_id) and not previous.known(object_id))
            if new_tokens != list(range(previous.tokens_created + 1, previous.tokens_created + len(new_tokens) + 1)):
                problems.append(Problem("error", "start", path, "new live and departed tokens must share consecutive first-use numbers"))
        introduced = []
        for object_id, facts in given.get("departed", {}).items():
            if state._introduce_departed(object_id, facts, f"{path}/departed/{object_id}", problems,
                                         initial=True, existing=previous is not None):
                introduced.append(object_id)
        for object_id, facts in given.get("objects", {}).items():
            if object_id not in placed:
                problems.append(Problem("error", "start", f"{path}/objects/{object_id}", "has facts but is in no zone"))
                continue
            state.objects[object_id] = {}
            if (previous is not None and previous.exists(object_id) and object_id not in state.cards
                    and not previous.is_ability(object_id)):
                state.objects[object_id]["owner"] = previous.owner_of(object_id)
            for fact, value in facts.items():
                state._set_object(object_id, fact, value, f"{path}/objects/{object_id}/{fact}", problems)
            state._check_new_object(object_id, f"{path}/objects/{object_id}", problems)
            if not state.objects.get(object_id):
                state.objects.pop(object_id, None)
        changed = previous._copy_definition_changes(state) if previous is not None else set()
        prepared_entries = {object_id for object_id in changed
                            if previous is not None and previous.zone_of(object_id) != "battlefield"
                            and state.zone_of(object_id) == "battlefield"}
        state._freeze_copy_bases(path, problems, changed, prepared_entries)
        state._check_player_references(path, problems)
        if state.pending is not None:
            state._check_pending(state.pending, f"{path}/pending", problems)
        state._check_combat(path, problems)
        state._check_result(path, problems)
        causal_used=set()
        causal=record["start"].get("causal",{})
        for producer in causal.get("producers",[]):
            causal_used.add(producer["source"])
            causal_used.update(ability_object_refs(producer["ability"]))
        for effect in causal.get("delayed_triggers",[]):
            for binding in effect.get("retained",[]):
                causal_used.update(value["object"] for value in binding["values"] if "object" in value)
        state._check_departed_use(introduced, path, problems, causal_used)
        return state

    # ----- queries -----

    def zone_of(self, object_id: str) -> str | None:
        for zone, members in self.zones.items():
            if object_id in members:
                return zone
        return None

    def exists(self, object_id: str) -> bool:
        return self.zone_of(object_id) is not None

    def owner_of(self, object_id: str) -> str | None:
        if object_id in self.cards:
            return self.cards[object_id]["owner"]
        if self.is_ability(object_id):
            return None
        facts = self.objects.get(object_id, {})
        return facts.get("owner") or facts.get("controller")

    def is_ability(self, object_id: str) -> bool:
        return (self.reference_facts(object_id).get("kind") == "ability"
                or self.copy_bases.get(object_id) == "ability")

    def known(self, object_id: str) -> bool:
        """An identity already established by the record, possibly now departed."""
        return object_id in self.cards or self.exists(object_id) or object_id in self.historical

    def reference_facts(self, object_id: str) -> dict:
        """Text/source lookup may use a departed object's last recorded definition."""
        return self.objects.get(object_id, {}) if self.exists(object_id) else self.historical.get(object_id, {})

    def fact(self, object_id: str, name: str):
        facts = self.objects.get(object_id, {})
        if name in facts:
            return facts[name]
        if name == "fused":
            basis = self.copy_bases.get(object_id)
            if isinstance(basis, tuple):
                return basis[1] == "fused"
        if name == "controller":
            return self.owner_of(object_id)
        return OBJECT_DEFAULTS.get(name)

    def listing(self, zone: str) -> list[str]:
        """The zone as the spec lists it."""
        members = self.zones[zone]
        if zone.endswith(".graveyard") or zone == "stack":
            return list(reversed(members))
        if zone.endswith(".library"):
            return list(members)
        return sorted(members)

    # ----- facts -----

    def _tier_ok(self, fact, tier2: set, path, problems) -> bool:
        if self.tier == 1 and fact in tier2:
            problems.append(Problem("error", "tier_scope", path, f"{fact} is a tier-2 fact; this record declares tier 1"))
            return False
        return True

    def _set_player(self, seat, fact, value, path, problems):
        if fact not in TIER1_PLAYER | TIER2_PLAYER:
            problems.append(Problem("error", "delta", path, f"unknown player fact {fact}"))
            return
        if not self._tier_ok(fact, TIER2_PLAYER, path, problems):
            return
        facts = self.players.setdefault(seat, {})
        if fact in PLAYER_DEFAULTS and value == PLAYER_DEFAULTS[fact]:
            facts.pop(fact, None)
        else:
            facts[fact] = deepcopy(value)

    def _set_game(self, fact, value, path, problems):
        if fact not in GAME_FACTS:
            problems.append(Problem("error", "delta", path, f"unknown game fact {fact}"))
            return
        if not self._tier_ok(fact, GAME_FACTS, path, problems):
            return
        if value is not None and fact in ("monarch", "initiative") and value not in self.seats:
            problems.append(Problem("error", "unknown_reference", path, f"{value!r} is not a seat"))
        if value == GAME_DEFAULTS[fact]:
            self.game.pop(fact, None)
        else:
            self.game[fact] = value

    def _set_object(self, object_id, fact, value, path, problems):
        if fact not in TIER1_OBJECT | TIER2_OBJECT | {"owner"}:
            problems.append(Problem("error", "delta", path, f"unknown object fact {fact}"))
            return
        if not self._tier_ok(fact, TIER2_OBJECT, path, problems):
            return
        if fact in ("controller", "owner") and value not in self.seats:
            problems.append(Problem("error", "unknown_reference", path, f"{value!r} is not a seat"))
        current = self.objects.get(object_id, {})
        if fact == "copy_definition":
            established = self.copy_bases.get(object_id)
            if ("copy_definition" in current and value != current["copy_definition"]
                    or established is not None and copy_definition.basis(value) != established):
                problems.append(Problem("error", "delta", path, "a spell copy's intrinsic definition is immutable (707.2b)"))
                return
        if ((fact == "source" and source_free_rule(current.get("ability", {})))
                or (fact == "ability" and source_free_rule(value) and "source" in current)):
            problems.append(Problem("error", "delta", path, "an intrinsically source-free rule ability cannot acquire a source"))
            return
        if fact == "owner":
            if self.objects.get(object_id, {}).get("kind") == "ability":
                problems.append(Problem("error", "delta", path, "a stack ability has a controller, not an owner (113.8)"))
                return
            if object_id in self.cards:
                problems.append(Problem("error", "delta", path, "a physical card's owner is defined only in the card table (108.3)"))
                return
            established = self.objects.get(object_id, {}).get("owner")
            if established is not None and value != established:
                problems.append(Problem("error", "delta", path, "a generated object's owner cannot change (111.2, 112.2, 114.2)"))
                return
        for ref in self._object_refs(fact, value):
            if ref not in self.seats and not self.known(ref):
                problems.append(Problem("error", "unknown_reference", path, f"{ref!r} is not an object or seat of this record"))
        facts = self.objects.setdefault(object_id, {})
        default = OBJECT_DEFAULTS.get(fact)
        if fact == "controller" and value == self.owner_of(object_id) and object_id in self.cards:
            facts.pop(fact, None)
        elif fact in OBJECT_DEFAULTS and value == default:
            facts.pop(fact, None)
        else:
            facts[fact] = deepcopy(value)

    @staticmethod
    def _object_refs(fact, value) -> list[str]:
        if fact in ("copy_of", "attached_to", "attacking") and isinstance(value, str) and value:
            return [value]
        if fact == "chosen" and isinstance(value, dict) and isinstance(value.get("player"), str):
            return [value["player"]]
        if fact == "blocking":
            return list(value)
        if fact == "targets":
            return [t for group in value for t in group]
        if fact == "source":
            return [value]
        if fact == "ability" and isinstance(value, dict):
            return ability_object_refs(value)
        return []

    def _introduce_departed(self, object_id, facts, path, problems, *, initial=False, existing=False):
        """An explicit definition is provenance, never zone membership or live facts."""
        from .schema import Validator, load_schema
        schema = load_schema("mgn-1.schema.json")
        errors = Validator(schema).errors(facts, schema["$defs"]["departed_definition"], path)
        if not TOKEN_ID.fullmatch(object_id) or object_id in self.cards:
            problems.append(Problem("error", "delta", path, "departed introductions name token IDs only"))
            return False
        if errors:
            problems.extend(Problem("error", "schema", error.path, error.message) for error in errors)
            return False
        if facts["owner"] not in self.seats:
            problems.append(Problem("error", "unknown_reference", f"{path}/owner", "not a seat of this game"))
            return False
        if object_id in self.departed and existing:
            if self.departed[object_id] != facts:
                problems.append(Problem("error", "delta", path, "a departed definition is immutable"))
            return False
        if self.known(object_id):
            problems.append(Problem("error", "delta", path, "a departed introduction cannot replace a live or previously known identity"))
            return False
        number = int(object_id[1:])
        if not initial and number != self.tokens_created + 1:
            problems.append(Problem("error", "delta", path, f"new token must be T{self.tokens_created + 1} (first recorded use)"))
        self.tokens_created = max(self.tokens_created, number)
        self.departed[object_id] = deepcopy(facts)
        self.historical[object_id] = deepcopy(facts)
        return True

    def _check_departed_use(self, introduced, path, problems, additional=()):
        # Only historical-reference fields qualify. A live action/choice or an
        # unrelated declaration cannot create an identity merely by naming it.
        used = set(additional)
        for facts in self.objects.values():
            for fact in ("source", "copy_of", "ability", "targets"):
                if fact in facts:
                    used.update(self._object_refs(fact, facts[fact]))
        if self.pending:
            for instance in self.pending.get("about", {}).get("abilities", []):
                if "source" in instance: used.add(instance["source"])
                used.update(ability_object_refs(instance.get("ability", {})))
        for object_id in introduced:
            if object_id not in used:
                problems.append(Problem("error", "delta", f"{path}/departed/{object_id}", "a new departed definition needs an actual provenance reference at this boundary"))

    # ----- deltas -----

    def apply(self, delta: dict, random_outcomes: list, path: str, problems: list[Problem]):
        """Applies one transition's delta and random outcomes; appends problems found."""
        # A folded transition cannot reveal whether a newly copied definition
        # preceded or followed a donor's incarnation/characteristic change.
        changed = {object_id for object_id, facts in delta.get("objects", {}).items()
                   if self.exists(object_id) and (facts is None or
                       {"zone", "face", "fused", "copy_of", "copy_face", "token", "face_down"}.intersection(facts))}
        prepared_entries = {object_id for object_id, facts in delta.get("objects", {}).items()
                            if facts is not None and facts.get("zone") == "battlefield"
                            and self.zone_of(object_id) != "battlefield"}
        self._inserted = {}
        self._reordered = {zone: set(i for key in ("top", "bottom", "order") for i in reorder.get(key, []))
                           for zone, reorder in delta.get("zones", {}).items()}
        if "turn" in delta:
            turn = delta["turn"]
            if turn["active"] is not None and turn["active"] not in self.seats:
                problems.append(Problem("error", "unknown_reference", f"{path}/turn/active", f"{turn['active']!r} is not a seat"))
            self.turn = deepcopy(turn)
        if "pending" in delta:
            self.pending = deepcopy(delta["pending"])  # checked after the objects of this delta exist
        if "result" in delta:
            self.result = deepcopy(delta["result"])
            if self.result.get("winner") is not None and self.result["winner"] not in self.seats:
                problems.append(Problem("error", "unknown_reference", f"{path}/result/winner", "not a seat"))
        for seat, facts in delta.get("players", {}).items():
            if seat not in self.seats:
                problems.append(Problem("error", "unknown_reference", f"{path}/players/{seat}", "not a seat of this game"))
                continue
            for fact, value in facts.items():
                self._set_player(seat, fact, value, f"{path}/players/{seat}/{fact}", problems)
        for fact, value in delta.get("game", {}).items():
            self._set_game(fact, value, f"{path}/game/{fact}", problems)
        objects = delta.get("objects", {})
        # member order in JSON is not canonical: process cards first, then generated objects in the
        # order of their numbers, so that creation-order checks do not depend on how the file was written
        def processing_order(object_id):
            for rank, pattern in ((1, TOKEN_ID), (2, STACK_ID), (3, EMBLEM_ID)):
                if pattern.match(object_id):
                    return (rank, int(object_id[1:]), object_id)
            return (0, 0, object_id)
        objects = {object_id: objects[object_id] for object_id in sorted(objects, key=processing_order)}
        # Zone changes and historical introductions share the token number sequence.
        introduced = []
        departed = delta.get("departed", {})
        for object_id in sorted(set(objects) | set(departed), key=processing_order):
            if object_id in departed:
                if self._introduce_departed(object_id, departed[object_id], f"{path}/departed/{object_id}", problems):
                    introduced.append(object_id)
            if object_id not in objects:
                continue
            facts = objects[object_id]
            opath = f"{path}/objects/{object_id}"
            if facts is None:
                self._remove(object_id, opath, problems)
            elif "zone" in facts:
                if self.zone_of(object_id) == "battlefield" and facts["zone"] != "battlefield":
                    stray = sorted(f for f in facts if f in TIER2_OBJECT or f in ("attacking", "blocked", "blocking", "in_combat"))
                    if stray:
                        problems.append(Problem("error", "delta", opath, f"an object leaving the battlefield is a new object (400.7): its status, counters, damage and combat facts are not written ({', '.join(stray)})"))
                self._move(object_id, facts["zone"], facts.get("at"), opath, problems)
            elif "at" in facts:
                problems.append(Problem("error", "delta", f"{opath}/at", "at needs a zone move into a library"))
            elif not self.exists(object_id):
                if object_id in self.cards:
                    problems.append(Problem("error", "delta", opath, "facts for a card that is in no zone"))
                elif TOKEN_ID.match(object_id) or STACK_ID.match(object_id):
                    problems.append(Problem("error", "delta", opath, "a new token or stack object needs its zone"))
                else:
                    problems.append(Problem("error", "unknown_reference", opath, "not an object that exists at this point"))
        for object_id, facts in objects.items():
            if facts is None or not self.exists(object_id):
                continue
            opath = f"{path}/objects/{object_id}"
            for fact, value in facts.items():
                if fact in ("zone", "at"):
                    continue
                self._set_object(object_id, fact, value, f"{opath}/{fact}", problems)
            if "zone" in facts and object_id not in self.cards:
                self._check_new_object(object_id, opath, problems)
            if self.objects.get(object_id, {}).get("kind") == "ability":
                stray = sorted(f for f in self.objects[object_id] if f in TIER2_OBJECT)
                if stray:
                    problems.append(Problem("error", "delta", opath, f"an ability on the stack has no characteristics (405.4): {', '.join(stray)}"))
        # shuffles first: a reorder of the same delta then places the cards it names (found cards, 701.24b)
        last_shuffle = {o['seat']: i for i, o in enumerate(random_outcomes)
                        if o['kind'] == 'shuffle' and 'seat' in o}
        for index, outcome in enumerate(random_outcomes):
            # A folded stretch can shuffle a library repeatedly, with moves in
            # between. Its delta has final zones, so only the last order can
            # constrain those zones. Every outcome still checks identities and
            # duplicates; the engine checks its exact intermediate membership.
            self._random(outcome, f"{path}/random/{index}", problems,
                         apply_order=outcome.get('seat') not in last_shuffle or last_shuffle[outcome['seat']] == index)
        for zone, reorder in delta.get("zones", {}).items():
            self._reorder(zone, reorder, f"{path}/zones/{zone}", problems)
        self._freeze_copy_bases(path, problems, changed, prepared_entries)
        self._check_ambiguity(path, problems)
        self._check_player_references(path, problems)
        if self.pending is not None:
            self._check_pending(self.pending, f"{path}/pending", problems)
        self._check_combat(path, problems)
        self._check_result(path, problems)
        self._check_departed_use(introduced, path, problems)

    def _check_new_object(self, object_id, path, problems):
        """A token, emblem or stack object that just appeared has what spec 4.1 requires of it."""
        facts = self.objects.get(object_id, {})
        if EMBLEM_ID.match(object_id):
            if "emblem" not in facts:
                problems.append(Problem("error", "delta", path, "a new emblem needs its definition (emblem)"))
            if "controller" not in facts:
                problems.append(Problem("error", "delta", path, "a new emblem needs its controller, the player who got it (114.2)"))
            else:
                facts.setdefault("owner", facts["controller"])
        elif TOKEN_ID.match(object_id):
            if "token" not in facts:
                problems.append(Problem("error", "delta", path, "a new token needs its definition (token)"))
            if "controller" not in facts:
                problems.append(Problem("error", "delta", path, "a new token needs its controller, its creator (111.2)"))
            else:
                facts.setdefault("owner", facts["controller"])
        elif STACK_ID.match(object_id):
            kind = facts.get("kind")
            if kind is None:
                problems.append(Problem("error", "delta", path, "a new stack object needs its kind (ability or copy)"))
            elif kind == "ability":
                source_free = source_free_rule(facts.get("ability", {}))
                if source_free and "source" in facts:
                    problems.append(Problem("error", "delta", path, "an intrinsically source-free rule ability cannot have a source"))
                for needed in (("ability", "controller") if source_free else ("source", "ability", "controller")):
                    if needed not in facts:
                        problems.append(Problem("error", "delta", path, f"a new ability on the stack needs its {needed} (113.7, 603.3a)"))
            else:
                for needed in ("copy_of", "controller"):
                    if needed not in facts:
                        problems.append(Problem("error", "delta", path, f"a new copy on the stack needs its {needed} (707.10)"))
            zone = self.zone_of(object_id)
            if zone != "stack" and not (zone == "exile" and kind == "copy" and facts.get("copy_face") == 1):
                problems.append(Problem("error", "delta", path, "only a prepared copy with copy_face 1 may remain outside the stack, in exile (722.3c)"))
            if "copy_face" in facts and (kind != "copy" or not facts.get("copy_of") or facts["copy_face"] != 1):
                problems.append(Problem("error", "delta", path, "copy_face 1 needs a copy and its source (722.3c)"))
            if kind == "ability":
                if "owner" in facts:
                    problems.append(Problem("error", "delta", f"{path}/owner", "a stack ability has a controller, not an owner (113.8)"))
                    facts.pop("owner")
            elif kind != "copy" and "controller" in facts:
                facts.setdefault("owner", facts["controller"])

    def _check_combat(self, path, problems):
        battlefield = set(self.zones.get("battlefield", []))
        for object_id, facts in self.objects.items():
            if facts.get("attacking") is not None and object_id not in battlefield:
                problems.append(Problem("error", "delta", f"{path}/objects/{object_id}/attacking", "only a permanent attacks"))
            if facts.get("blocked") and facts.get("attacking") is None:
                problems.append(Problem("error", "delta", f"{path}/objects/{object_id}/blocked", "only an attacking creature is blocked (509.1h)"))
            for attacker in facts.get("blocking", []):
                if attacker not in battlefield or self.fact(attacker, "attacking") is None:
                    problems.append(Problem("error", "delta", f"{path}/objects/{object_id}/blocking", f"{attacker} is not an attacking creature"))

    def _remove(self, object_id, path, problems):
        if object_id in self.cards:
            problems.append(Problem("error", "delta", path, "a card cannot cease to exist; move it to a zone"))
            return
        zone = self.zone_of(object_id)
        if zone is None:
            problems.append(Problem("error", "unknown_reference", path, "not an object that exists at this point"))
            return
        self.zones[zone].remove(object_id)
        self.historical[object_id] = deepcopy(self.objects.get(object_id, {}))
        self.objects.pop(object_id, None)

    def _move(self, object_id, zone, at, path, problems):
        if zone not in self.zones:
            problems.append(Problem("error", "unknown_reference", f"{path}/zone", f"{zone!r} is not a zone of this game"))
            return
        current = self.zone_of(object_id)
        if current is None:
            if object_id not in self.cards and object_id in self.historical:
                problems.append(Problem("error", "delta", path, "a departed generated identity cannot become live again"))
                return
            if object_id in self.cards:
                if self.cards[object_id].get("from") != "sideboard":
                    problems.append(Problem("error", "unknown_reference", path, "card is in no zone; this should not happen"))
            elif TOKEN_ID.match(object_id):
                expected = f"T{self.tokens_created + 1}"
                if object_id != expected:
                    problems.append(Problem("error", "delta", path, f"new token must be {expected} (creation order)"))
                self.tokens_created = max(self.tokens_created, int(object_id[1:]))
            elif EMBLEM_ID.match(object_id):
                expected = f"E{self.emblems_created + 1}"
                if object_id != expected:
                    problems.append(Problem("error", "delta", path, f"new emblem must be {expected} (creation order)"))
                if zone != "command":
                    problems.append(Problem("error", "delta", f"{path}/zone", "an emblem is put into the command zone (114.2)"))
                self.emblems_created = max(self.emblems_created, int(object_id[1:]))
            elif STACK_ID.match(object_id):
                expected = f"S{self.stack_created + 1}"
                if object_id != expected:
                    problems.append(Problem("error", "delta", path, f"new stack object must be {expected} (creation order)"))
                if zone not in ("stack", "exile"):
                    problems.append(Problem("error", "delta", f"{path}/zone", "a generated spell object is created on the stack or in prepared exile (722.3c)"))
                self.stack_created = max(self.stack_created, int(object_id[1:]))
            else:
                problems.append(Problem("error", "unknown_reference", path, f"{object_id!r} is not a card of this record"))
                return
        else:
            self.zones[current].remove(object_id)
        owner = self.owner_of(object_id)
        previous = self.objects.pop(object_id, {})  # a new object (400.7)
        if object_id not in self.cards and owner is not None and not self.is_ability(object_id):
            self.objects[object_id] = {"owner": owner}
        if current == "exile" and zone == "stack" and previous.get("kind") == "copy" and "copy_face" in previous:
            # Casting the prepared copy preserves its defined characteristics (707.12, 722.3c).
            self.objects[object_id] = {key: deepcopy(previous[key]) for key in
                                      ("kind", "copy_of", "copy_face", "copy_definition", "controller", "owner") if key in previous}
        if "." in zone and owner is not None and zone.split(".")[0] != owner:
            problems.append(Problem("error", "delta", f"{path}/zone", f"a card goes only to its owner's library, hand or graveyard (400.3); {object_id} belongs to {owner}"))
        members = self.zones[zone]
        if zone.endswith(".library"):
            if at is None:
                self._inserted.setdefault(zone, []).append((object_id, None))
                members.insert(0, object_id)  # placed by a shuffle of this transition, or an error
            elif at == "top":
                members.insert(0, object_id)
                self._inserted.setdefault(zone, []).append((object_id, at))
            elif at == "bottom":
                members.append(object_id)
                self._inserted.setdefault(zone, []).append((object_id, at))
            else:
                n = at["from_top"]
                members.insert(min(n - 1, len(members)), object_id)  # beyond the library: the bottom (401.7)
                self._inserted.setdefault(zone, []).append((object_id, at))
        else:
            if at is not None:
                problems.append(Problem("error", "delta", f"{path}/at", "at applies only to a move into a library"))
            if is_ordered(zone):
                members.insert(0, object_id)  # on top
                self._inserted.setdefault(zone, []).append((object_id, "top"))
            else:
                members.append(object_id)

    def _reorder(self, zone, reorder, path, problems):
        if zone not in self.zones or not is_ordered(zone):
            problems.append(Problem("error", "delta", path, "not an ordered zone of this game"))
            return
        members = self.zones[zone]
        for key in ("top", "bottom", "order"):
            if key not in reorder:
                continue
            ids = reorder[key]
            missing = [i for i in ids if i not in members]
            if missing:
                problems.append(Problem("error", "delta", f"{path}/{key}", f"not in {zone}: {', '.join(missing)}"))
                continue
            if key == "order":
                if set(ids) != set(members):
                    problems.append(Problem("error", "delta", f"{path}/order", "order must list every object of the zone"))
                    continue
                self.zones[zone] = list(ids)
            elif key == "top":
                self.zones[zone] = list(ids) + [m for m in members if m not in ids]
            else:
                self.zones[zone] = [m for m in members if m not in ids] + list(ids)
            members = self.zones[zone]
        self._inserted.pop(zone, None)  # an explicit order settles any ambiguity

    def _random(self, outcome, path, problems, apply_order=True):
        kind = outcome["kind"]
        if kind == "shuffle":
            order = outcome["order"]
            if "pile" in outcome:
                if sorted(order) != sorted(outcome["pile"]):
                    problems.append(Problem("error", "shuffle", f"{path}/order", "not a permutation of the pile"))
                for card in outcome["pile"]:
                    if not self.known(card):
                        problems.append(Problem("error", "unknown_reference", f"{path}/pile", f"{card!r} is unknown"))
                return
            seat = outcome["seat"]
            if seat not in self.seats:
                problems.append(Problem("error", "unknown_reference", f"{path}/seat", "not a seat"))
                return
            zone = f"{seat}.library"
            members = self.zones[zone]
            if len(order) != len(set(order)):
                problems.append(Problem("error", "shuffle", f"{path}/order", "a card is listed twice"))
                return
            foreign = [c for c in order if c not in self.cards or self.cards[c]["owner"] != seat]
            if foreign:
                problems.append(Problem("error", "shuffle", f"{path}/order", f"not cards of {seat}: {', '.join(foreign)}"))
                return
            if not apply_order:
                return
            placed_after = [(i, at) for i, at in self._inserted.get(zone, []) if i not in order and at is not None]
            unplaced = [i for i, at in self._inserted.get(zone, []) if i not in order and at is None]
            if unplaced:
                problems.append(Problem("error", "delta", path, f"moved into {zone} without at and not in the shuffle order: {', '.join(unplaced)}"))
            after = {i for i, _ in placed_after} | self._reordered.get(zone, set())
            stray = [c for c in members if c not in order and c not in after]
            if stray:
                problems.append(Problem("error", "shuffle", f"{path}/order", f"in the library but neither in the shuffle order nor placed after it: {', '.join(stray)}"))
                return
            kept = [c for c in members if c not in order and c in self._reordered.get(zone, set())]  # found cards, placed by the reorder
            new = kept + [c for c in order if c in members]
            for object_id, at in placed_after:
                if at == "top":
                    new.insert(0, object_id)
                elif at == "bottom":
                    new.append(object_id)
                else:
                    new.insert(at["from_top"] - 1, object_id)
            self.zones[zone] = new
            self._inserted.pop(zone, None)
        elif kind == "random_order":
            if sorted(outcome["order"]) != sorted(outcome["objects"]):
                problems.append(Problem("error", "random", f"{path}/order", "not a permutation of objects"))
            for ref in outcome["objects"]:
                if ref not in self.seats and not self.known(ref):
                    problems.append(Problem("error", "unknown_reference", f"{path}/objects", f"{ref!r} is unknown"))
        elif kind == "random_choice":
            if not set(outcome["chosen"]) <= set(outcome["from"]):
                problems.append(Problem("error", "random", f"{path}/chosen", "chosen outside from"))
            for ref in outcome["from"]:
                if ref not in self.seats and not self.known(ref):
                    problems.append(Problem("error", "unknown_reference", f"{path}/from", f"{ref!r} is unknown"))
        elif kind == "coin":
            if "call" in outcome and "caller" in outcome and "winner" in outcome:
                expected = outcome["caller"] if outcome["call"] == outcome["result"] else None
                if outcome["winner"] != expected:
                    problems.append(Problem("error", "random", f"{path}/winner", "the winner does not follow from the call and the result (705.2)"))
            for key in ("caller", "winner"):
                if outcome.get(key) is not None and outcome[key] not in self.seats:
                    problems.append(Problem("error", "unknown_reference", f"{path}/{key}", "not a seat"))
        elif kind == "die":
            if outcome["result"] > outcome["sides"]:
                problems.append(Problem("error", "random", f"{path}/result", "result above the number of sides"))
        elif kind == "starting_player":
            problems.append(Problem("error", "random", path, "starting_player belongs in start.random"))

    def _check_ambiguity(self, path, problems):
        for zone, inserted in self._inserted.items():
            unplaced = [object_id for object_id, at in inserted if at is None]
            if unplaced:
                problems.append(Problem("error", "delta", f"{path}/objects", f"moved into {zone} without at, and no shuffle of that library in this transition lists them: {', '.join(unplaced)}"))
            places = {}
            positioned = [object_id for object_id, at in inserted if isinstance(at, dict)]
            if len(positioned) > 1:
                problems.append(Problem("error", "delta", f"{path}/objects", f"{', '.join(positioned)} are placed by from_top into {zone} in one delta; give the library's whole order instead"))
            for object_id, at in inserted:
                if at is None:
                    continue
                key = at if not isinstance(at, dict) else ("from_top", at["from_top"])
                places.setdefault(key, []).append(object_id)
            for key, ids in places.items():
                if len(ids) <= 1:
                    continue
                if zone == "stack" and all(STACK_ID.match(i) for i in ids):
                    # new stack objects go on the stack in the order of their numbers (spec 4.1)
                    members = self.zones[zone]
                    front = sorted(ids, key=lambda i: -int(i[1:]))
                    self.zones[zone] = front + [m for m in members if m not in ids]
                    continue
                problems.append(Problem("error", "delta", f"{path}/objects", f"{', '.join(ids)} enter {zone} at the same place; zones.{zone}.top (or bottom) must give their order"))
        self._inserted = {}

    def _check_ability_instance(self, instance, path, problems):
        # Sources and acquired provenance can outlive their objects (113.7a).
        for ref in ([instance["source"]] if "source" in instance else []) + ability_object_refs(instance["ability"]):
            if not self.known(ref):
                problems.append(Problem("error", "unknown_reference", path, f"{ref!r} is unknown"))
        for ref in instance.get("for", []):
            if ref not in self.seats and not self.known(ref):
                problems.append(Problem("error", "unknown_reference", f"{path}/for", f"{ref!r} is unknown"))

    def _check_player_references(self, path, problems):
        # Run after zones and objects load, including objects introduced by this delta.
        # Mana and its spending conditions survive their producer (106.4, 106.6).
        for seat, facts in self.players.items():
            for index, lot in enumerate(facts.get("mana_restricted", [])):
                lpath = f"{path}/players/{seat}/mana_restricted/{index}"
                if "producer" in lot:
                    self._check_ability_instance(lot["producer"], f"{lpath}/producer", problems)
                chosen = lot.get("chosen", {})
                if "player" in chosen and chosen["player"] not in self.seats:
                    problems.append(Problem("error", "unknown_reference", f"{lpath}/chosen/player", "not a seat of this game"))

    def _check_pending(self, pending, path, problems):
        if pending["seat"] not in self.seats:
            problems.append(Problem("error", "unknown_reference", f"{path}/seat", "not a seat"))
        about = pending.get("about", {})
        allowed = ABOUT.get(pending["kind"], set())
        if pending["kind"] == "order_cards":
            zone = about.get("zone", "")
            if zone.endswith(".library"):
                allowed = allowed | {"placement"}
            if zone not in self.zones or not zone.startswith(pending["seat"] + "."):
                problems.append(Problem("error", "pending", f"{path}/about/zone", "the ordering owner chooses for their own library or graveyard (401.4, 404.3)"))
            cards = about.get("cards", [])
            if len(cards) != len(set(cards)) or any(card not in self.cards or self.owner_of(card) != pending["seat"] for card in cards):
                problems.append(Problem("error", "pending", f"{path}/about/cards", "order_cards names distinct physical cards owned by the chooser"))
        extra = sorted(set(about) - allowed)
        missing = sorted(allowed - set(about))
        if extra:
            problems.append(Problem("error", "pending", f"{path}/about", f"{pending['kind']} carries no {', '.join(extra)}"))
        if missing:
            problems.append(Problem("error", "pending", f"{path}/about", f"{pending['kind']} needs about.{', about.'.join(missing)}"))
        if pending["kind"] == "order_triggers" and len(about.get("abilities", [])) < 2:
            problems.append(Problem("error", "pending", f"{path}/about/abilities", "ordering is asked only for two or more triggered abilities (603.3b)"))
        for key in ("object", "creature"):
            if key in about and about[key] not in self.seats and not self.known(about[key]):
                problems.append(Problem("error", "unknown_reference", f"{path}/about/{key}", f"{about[key]!r} is unknown"))
        for card in about.get("cards", []):
            if not self.known(card):
                problems.append(Problem("error", "unknown_reference", f"{path}/about/cards", f"{card!r} is unknown"))
        for i, instance in enumerate(about.get("abilities", [])):
            self._check_ability_instance(instance, f"{path}/about/abilities/{i}", problems)

    def _check_result(self, path, problems):
        finished = self.result["kind"] != "unfinished"
        if finished and self.pending is not None:
            problems.append(Problem("error", "result", path, "the game has ended but a question is pending"))
        if not finished and self.pending is None:
            problems.append(Problem("error", "result", path, "no question is pending but the game has not ended"))

    # ----- explicit form -----

    def explicit(self, oracle=None) -> dict:
        """The full projection with every fact of the declared tier written out (spec 7.3)."""
        players = {}
        for seat in self.seats:
            facts = {"life": self.players.get(seat, {}).get("life"), "left": self.players.get(seat, {}).get("left", False)}
            if self.tier == 2:
                for fact in sorted(TIER2_PLAYER):
                    facts[fact] = deepcopy(self.players.get(seat, {}).get(fact, PLAYER_DEFAULTS[fact]))
            players[seat] = facts
        turn = deepcopy(self.turn)
        turn["occurrence"] = turn.get("occurrence", 1)  # the first step of its name in the turn unless written
        out = {"turn": turn, "pending": deepcopy(self.pending), "result": deepcopy(self.result),
               "players": players, "zones": {zone: self.listing(zone) for zone in zone_names(self.seats)}, "objects": {}}
        if self.tier == 2:
            out["game"] = {fact: self.game.get(fact) for fact in sorted(GAME_FACTS)}
        for zone, members in self.zones.items():
            for object_id in members:
                facts = self._explicit_object(object_id, zone, oracle)
                if facts:
                    out["objects"][object_id] = facts
        if self.departed:
            out["departed"] = deepcopy(self.departed)
        return normalize_state_lots(out)

    def _copy_card(self, object_id, unavailable=()):
        """Find the pinned card supplying copiable faces, rather than a copy's own printing."""
        seen = set()
        while object_id not in seen:
            if object_id in unavailable:
                return None
            seen.add(object_id)
            source = self.reference_facts(object_id).get("copy_of")
            if source:
                object_id = source
            else:
                return self.cards.get(object_id)
        return None

    def _copy_definition_changes(self, later):
        """Potentially ambiguous donor changes between two full observations."""
        return {object_id for members in self.zones.values() for object_id in members
                if self.zone_of(object_id) != later.zone_of(object_id) or any(
                    self.reference_facts(object_id).get(key) != later.reference_facts(object_id).get(key)
                    for key in ("face", "fused", "copy_of", "copy_face", "token", "face_down"))}

    def _freeze_copy_bases(self, path, problems, changed=(), prepared_entries=()):
        """Capture generated copies once (707.2b, 707.10), preserving known history.

        Explicit intrinsic definitions cover folded/changed donors and nonprinted
        spells. The compact spelling requires an unchanged ordinary spell donor.
        General permanent copy effects remain outside this narrow mechanism.
        """
        def source_basis(object_id, seen):
            facts = self.reference_facts(object_id)
            if facts.get("face_down") or "token" in facts:
                return None  # The full explicit intrinsic definition is required.
            if object_id in self.copy_bases:
                return self.copy_bases[object_id]
            if object_id in seen or object_id in changed or self.zone_of(object_id) != "stack":
                return None
            if STACK_ID.match(object_id) and facts.get("kind") == "copy":
                return capture(object_id, seen)
            if facts.get("copy_of"):
                return source_basis(facts["copy_of"], seen | {object_id})
            if object_id in self.cards:
                return (self.cards[object_id]["oracle"], "fused" if facts.get("fused") else facts.get("face", 0))
            return None

        def capture(object_id, seen):
            if object_id in self.copy_bases:
                explicit = self.reference_facts(object_id).get("copy_definition")
                if explicit is not None and copy_definition.basis(explicit) != self.copy_bases[object_id]:
                    problems.append(Problem("error", "delta", f"{path}/objects/{object_id}/copy_definition",
                        "a spell copy's intrinsic definition is immutable (707.2b)"))
                return self.copy_bases[object_id]
            if object_id in seen:
                return None
            facts = self.reference_facts(object_id)
            explicit = facts.get("copy_definition")
            donor = facts.get("copy_of")
            authenticated = source_basis(donor, seen | {object_id}) if donor else None
            if explicit is not None and "copy_face" in facts:
                source_facts = self.reference_facts(donor)
                own_entry = (donor in prepared_entries and donor in self.cards
                             and not source_facts.get("copy_of") and not source_facts.get("face_down"))
                unavailable = set(changed) - {donor} if own_entry else changed
                card = self._copy_card(donor, unavailable)
                authenticated = (card["oracle"], facts["copy_face"]) if card else None
            if explicit is not None and authenticated == "ability":
                problems.append(Problem("error", "delta", f"{path}/objects/{object_id}/copy_definition",
                    "a copy of an ability has no spell definition (405.4, 707.10b)"))
                basis = "ability"
            elif explicit is not None:
                basis = copy_definition.basis(explicit)
                if authenticated is not None:
                    self.copy_authentications[object_id] = deepcopy(authenticated)
                    if isinstance(basis, tuple) and isinstance(authenticated, tuple) and basis != authenticated:
                        problems.append(Problem("error", "coherence", f"{path}/objects/{object_id}/copy_definition",
                            "copy definition contradicts its unchanged donor (707.2)"))
            elif "copy_face" in facts:
                source = facts.get("copy_of")
                # CR 722.3c creates the exile copy as the permanent enters
                # prepared. A direct physical source entering with its own
                # definition supplies the new permanent's prepare spell, even
                # though that zone move is ambiguous for ordinary spell copies.
                source_facts = self.reference_facts(source)
                own_entry = (source in prepared_entries and source in self.cards
                             and not source_facts.get("copy_of") and not source_facts.get("face_down"))
                unavailable = set(changed) - {source} if own_entry else changed
                card = self._copy_card(source, unavailable)
                basis = (card["oracle"], facts["copy_face"]) if card else None
            else:
                source = facts.get("copy_of")
                basis = source_basis(source, seen | {object_id}) if source else None
                if basis is None:
                    problems.append(Problem("error", "unsupported", f"{path}/objects/{object_id}/copy_of",
                        "spell copy needs an unchanged spell definition or a previously frozen copy basis; missing or folded donor history is unsupported (707.2b, 707.10)"))
            self.copy_bases[object_id] = basis
            return basis

        # Remember an ability even after it leaves the stack. Its copy is an
        # ability in its own right, with no card characteristics (707.10b).
        for object_id, facts in self.objects.items():
            if "copy_definition" in facts and (not STACK_ID.fullmatch(object_id) or facts.get("kind") != "copy"):
                problems.append(Problem("error", "delta", f"{path}/objects/{object_id}/copy_definition",
                    "copy_definition belongs only to a generated spell copy"))
            if facts.get("kind") == "ability":
                self.copy_bases[object_id] = "ability"
        for object_id, facts in self.objects.items():
            if STACK_ID.match(object_id) and facts.get("kind") == "copy":
                capture(object_id, set())
                if self.is_ability(object_id):
                    if "owner" in facts:
                        problems.append(Problem("error", "delta", f"{path}/objects/{object_id}/owner",
                            "a copied ability has a controller, not an owner (113.8, 707.10b)"))
                        facts.pop("owner")
                    stray = sorted(TIER2_OBJECT.intersection(facts))
                    if stray:
                        problems.append(Problem("error", "delta", f"{path}/objects/{object_id}",
                            f"a copied ability has no characteristics (405.4, 707.10b): {', '.join(stray)}"))
                elif "controller" in facts:
                    facts.setdefault("owner", facts["controller"])

    @staticmethod
    def _token_printed(token):
        base = {"colors": sorted(token.get("colors", [])), "types": sorted(token.get("types", [])),
                "supertypes": sorted(token.get("supertypes", [])), "subtypes": sorted(token.get("subtypes", [])),
                "keywords": sorted(normalize_keyword(k) for k in token.get("keywords", []))}
        for key in ("power", "toughness"):
            if key in token:
                base[key] = token[key]
        return base

    def _printed(self, object_id, oracle, _seen=None) -> dict | None:
        """Printed or defined base characteristics: a card's face, a token's definition, a copy's original."""
        seen = set() if _seen is None else _seen
        if object_id in seen:
            return None
        seen.add(object_id)
        facts = self.reference_facts(object_id)
        if object_id in self.copy_bases:
            basis = self.copy_bases[object_id]
            if isinstance(basis, dict):
                return copy_definition.printed(basis, oracle)
            if isinstance(basis, tuple) and oracle is not None:
                oracle_id, face = basis
                if face == "fused":
                    return oracle.printed_fused(oracle_id) if oracle_id in oracle and oracle.has_fuse(oracle_id) else None
                if oracle_id in oracle and 0 <= face < oracle.face_count(oracle_id):
                    if "copy_face" not in facts or (face == 1 and oracle.faces[oracle_id][0].raw.get("layout") == "prepare"):
                        return oracle.printed(oracle_id, face)
            return None
        if "copy_definition" in facts:
            return copy_definition.printed(copy_definition.basis(facts["copy_definition"]), oracle)
        if facts.get("copy_of"):
            return self._printed(facts["copy_of"], oracle, seen)
        if "token" in facts:
            return self._token_printed(facts["token"])
        if object_id in self.cards and oracle is not None and self.cards[object_id]["oracle"] in oracle:
            if facts.get("fused"):
                return oracle.printed_fused(self.cards[object_id]["oracle"]) if oracle.has_fuse(self.cards[object_id]["oracle"]) else None
            return oracle.printed(self.cards[object_id]["oracle"], facts.get("face", 0))
        return None

    def _explicit_object(self, object_id, zone, oracle) -> dict:
        facts = self.objects.get(object_id, {})
        out = {}
        permanent = zone == "battlefield"
        on_stack = zone == "stack"
        if self.fact(object_id, "fused"):
            out["fused"] = True
        for key in ("token", "emblem", "kind", "source", "ability", "cast", "copy_face", "copy_definition", "fused"):
            if key in facts:
                out[key] = deepcopy(facts[key])
        if "owner" in facts and not self.is_ability(object_id):
            out["owner"] = facts["owner"]
        if permanent or on_stack or "emblem" in facts or "copy_face" in facts:
            out["controller"] = self.fact(object_id, "controller")
        if permanent or on_stack or "copy_face" in facts:
            out["copy_of"] = facts.get("copy_of")
            out["face"] = facts.get("face", 0)
            out["chosen"] = deepcopy(facts.get("chosen", {}))
        if on_stack:
            for key in ("targets", "modes", "x", "division"):
                if key == "x":
                    if "x" in facts:
                        out["x"] = facts["x"]
                else:
                    out[key] = deepcopy(facts.get(key, OBJECT_DEFAULTS[key]))
        if permanent:
            out["attacking"] = facts.get("attacking")
            out["blocked"] = facts.get("blocked", False)
            out["blocking"] = list(facts.get("blocking", []))
            out["in_combat"] = facts.get("in_combat", out["attacking"] is not None or bool(out["blocking"]))
        if self.tier == 2 and (permanent or on_stack) and not self.is_ability(object_id):
            if on_stack and facts.get("face_down"):
                out["face_down"] = True
            if permanent:
                for key in ("tapped", "flipped", "face_down", "phased_out", "damage", "attached_to", "designations"):
                    out[key] = deepcopy(facts.get(key, OBJECT_DEFAULTS[key]))
                if "level" in facts:
                    out["level"] = facts["level"]
            out["counters"] = deepcopy(facts.get("counters", {}))
            out["granted"] = list(facts.get("granted", []))
            base = self._printed(object_id, oracle) or {}
            for key in ("colors", "types", "supertypes", "subtypes", "keywords"):
                if key in facts:
                    out[key] = deepcopy(facts[key])
                elif key in base and not isinstance(base[key], str):
                    out[key] = deepcopy(base[key])
            # power and toughness default to the printed values for a creature and for an object
            # not on the battlefield; a noncreature permanent has none, whatever its face prints
            # (208.3), nor has a station card off the battlefield (721.2c)
            none = (permanent and "creature" not in out.get("types", [])) or (not permanent and "station" in base.get("keywords", []))
            for key in ("power", "toughness"):
                if key in facts:
                    if facts[key] is not None or key in base:
                        out[key] = deepcopy(facts[key])
                elif key in base and not isinstance(base[key], str):
                    out[key] = None if none else deepcopy(base[key])
        elif self.tier == 2 and not self.is_ability(object_id):
            for key in TIER2_OBJECT:
                if key in facts:
                    out[key] = deepcopy(facts[key])
        return out
