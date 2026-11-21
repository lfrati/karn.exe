# MGN 1: Magic: the Gathering Game Notation

Status: **`mgn/1` is stable.** It is the format of this repository's integration tests and changes only by amending this document. It does not claim complete Magic representability: an adapter refuses what it cannot execute, and the initial position has deliberate limits (section 2.4).

MGN is a machine-first notation for games and sequences of Magic: the Gathering.
A record is a vetted sequence of game states: an initial position followed by
transitions, each a decision taken by one seat with the complete delta of the game
state up to the state in which the next recorded decision is taken, or an action
attempted and marked illegal. Nothing the rules force between two decisions is
written: passes, step changes, resolutions and the other answers that are the
only ones left are implied.
Rules engines replay records through adapters and authors write them from this
specification, the Comprehensive Rules and the Oracle text alone. This document is
normative for format `mgn/1`. The companion documents are the [JSON Schema](schema/mgn-1.schema.json),
the [adapter protocol](adapter-protocol.md), [verification](verification.md) and
the derived [text rendering](rendering.md); the worked examples are under
[examples](examples/). The validator is `scripts/mgn/validate.py`; it runs without
any engine. Rule citations are to the Comprehensive Rules edition the record pins;
the repository's current pin is the edition `resources/manifest.json` names
(`sources.rules.edition`).

Words: *must* states an obligation on the record or its producer; *seat* is a
player position (`A`, `B`, ...); *object* is a card, token or stack object;
*boundary* is a state in which a recorded entry is made, or the record's end
(section 1); *fact* is one entry of the projection.

## 1. The model

1. **A recorded decision** is an answer one seat gave to a question the rules
   asked it (section 5), other than the question's null answer, or an action
   it took with priority (playing a land, casting, activating, a special action,
   conceding). Section 5 lists every question kind, who answers it and when the
   rules ask it. Passing priority is never recorded.
2. **Implied answers.** A question whose null answer is unique is not written:
   when no entry answers it, the null answer was given (section 5.6). Everyone
   passed priority in every window with no recorded action (117.3d, 117.4),
   nobody attacked or blocked, the hand was kept, the "may" was declined,
   nothing was chosen where nothing may be chosen. Everything those answers
   caused is in the delta. An owner's otherwise unrepresented choice of order
   for cards put into a library or graveyard simultaneously (401.4, 404.3) is an
   `order_cards` decision (section 5.4). A final delta alone cannot communicate
   an order consumed by another effect before the next boundary.
3. **A transition** is `(seat, decision)` plus the **delta**: every fact of the
   projection that changed between the state in which the decision was taken
   and the state in which the next recorded decision is taken, or the state the
   record ends in. The delta includes the consequences of the implied answers,
   state-based actions (704.3), triggered abilities that were put on the stack
   and resolved without a question, resolutions of the stack when everyone
   passed, and the turn-based actions in between: untapping (502.3), the draw
   (504.1), combat damage (510.2), damage removal and the end of "until end of
   turn" effects at cleanup (514.2). Engine-internal intermediate steps are not
   recorded. A **boundary** is a state in which a recorded entry (a decision or
   an attempt) is made, or the record's end.
4. **An illegal attempt** is `(seat, action)` marked illegal. The game refuses it
   and the state is unchanged: the whole attempt is reversed before any of it
   takes effect, mana abilities activated for it included (733.1; a seat that
   wants mana in its pool activates the mana ability as a decision of its own,
   section 5.2), and the seat keeps priority (733.2). An action whose
   irreversible parts have happened (cards moved to or from a library, a
   shuffle, a reveal: 733.1) is not an attempt. A refused attempt is not an
   action taken, so it does not interrupt a run of passes (117.4). No rule
   reference is recorded. An attempt carries no delta when the next entry, a
   decision or another attempt, is made in the same state; otherwise the
   question it failed to answer was then given its null answer, and the attempt
   carries the delta and random outcomes of the stretch to the next boundary,
   like a decision. Of consecutive attempts only the last can carry a delta.
5. **The projection** (section 3) is the set of facts a record can state, in two
   tiers. A record declares its tier. Within the declared tier a delta is
   complete: everything it does not list is unchanged.
6. **The record is full-knowledge.** Libraries are ordered, every random outcome
   is written out, and hidden zones are recorded like public ones. There is no
   partial-knowledge form.
7. **Nothing is listed that was not done.** No legal actions, offered choices or
   their digests appear anywhere. The pending question (which seat must answer
   what kind of question about which object) is a fact of the state at every
   boundary.
8. **Identity is physical.** A card is named by an id the author chooses, fixed
   for the whole record, naming the card wherever it is. Tokens and stack objects
   are numbered by creation order among the objects some recorded state shows;
   an object created and gone inside one stretch takes no number unless a
   recorded boundary refers to its departed definition, and a number never
   recurs. Live tokens and newly introduced departed tokens first shown at one
   boundary share one sequence, numbered in the order
   of their definitions' canonical texts (section 7.1, compared as 7.1 sorts
   member names), then in creation order among equal definitions: the rules
   fix no order among the tokens one event creates, so two engines could not
   agree on one. Tokens with equal definitions created at once bind to an
   engine's objects by either bijection on first replay, and that binding
   holds for the rest of the record. Stack objects are numbered bottom first;
   preparation copies created in exile share the `S<n>` sequence, taking numbers
   when first captured, in creation order. Casting one preserves its `S<n>` id;
   emblems in creation order, those one event creates by the canonical texts
   of their definitions. There are no incarnation counters (400.7 is never
   written down; its consequences are visible as facts).
   A previously recorded object that ceased to exist may still be named as a
   source, copy origin or retained target (113.7a, 608.2b, 704.5d). Its last
   recorded definition supplies ability references; it gains no current zone
   or live facts. An action choosing an object still requires it to exist.
   When a boundary first needs a token that already ceased to exist, `departed`
   supplies its complete intrinsic definition and owner (section 3.1). This is
   a definition for provenance, never a live object or a selectable target.
   A standalone position may state this premise explicitly; an adapter may
   refuse a setup it cannot construct or authenticate.

## 2. The record

A record is one JSON document (RFC 8259, UTF-8) with these members. Members not
listed here are errors unless their name starts with `x-`; `x-` members are kept
and ignored by validation and comparison.

| Member | Required | Content |
| --- | --- | --- |
| `mgn` | yes | the string `"1"` |
| `tier` | yes | `1` or `2`: the projection tier whose facts the deltas state completely (section 3) |
| `pins` | yes | `rules`: `{effective, sha256}` of the Comprehensive Rules text; `oracle`: `{source, updated_at, sha256}` of the Oracle data snapshot (section 2.1) |
| `game` | yes | `format` (section 2.2), `life` (starting life total, 103.4; 20 unless the format says otherwise), `hand_size` (starting hand size, 103.5; 7), `seats`: the seats in clockwise seating order, each one capital letter; the turn order begins with the starting player and proceeds in that order (103.1), so `A` need not play first |
| `match` | no | `game` (number of the game in the match, from 1), `chooser` (the seat that chooses who plays first, 103.1: the loser of the previous game, or after a draw the seat that chose in that game), `sideboarding`: per seat `{out: [ids], in: [ids]}` between the previous game and this one (100.4) |
| `author` | yes | `kind`: `agent`, `person` or `tool`; `name`; for an agent also `provider`, `model` and optionally `version`; for a tool `version` |
| `producer` | no | the software that wrote the file: `{name, version}` |
| `lineage` | no | `supersedes`: the record identity of the record this one replaces; `upgrades`: the record identity of the tier-1 record this tier-2 record was upgraded from |
| `note` | no | free text for readers; ignored by comparison |
| `cards` | yes | the card table (section 2.3) |
| `start` | yes | the initial position (section 2.4) |
| `transitions` | yes | the ordered transitions (section 4); may be empty |
| `checkpoints` | no | transition index (as a string) to the SHA-256 of the explicit projection after that transition (section 7.3) |

### 2.1 Pins

`pins.rules.effective` is the effective date of the Comprehensive Rules edition
(`YYYY-MM-DD`) and `pins.rules.sha256` the lowercase hex SHA-256 of its text file.
`pins.oracle.source` names the dataset (`scryfall-oracle-cards`), `updated_at` its
timestamp as the dataset publishes it, and `sha256` the SHA-256 of the pinned face
file the record was written against. A replay is valid when every used card's face
digests (section 2.3) match the replaying side's Oracle data; a mismatch flags the
affected cards, not the record.

### 2.2 Formats and seats

`game.format` is one of `standard`, `pioneer`, `modern`, `legacy`, `vintage`,
`pauper`, `commander`, `brawl`, `limited`, `custom`. It names the format whose
rules the game is played under; for a record that starts from a position it
says which card pool and format rules apply and claims nothing about deck
construction. `custom` is for anything else, such as the short decks of the
worked examples. The schema allows any number of seats; a record with more than
two seats uses the multiplayer rules (section 800) and is outside what this
version's validator can check beyond structure.

### 2.3 Card table

`cards` maps each physical id to:

| Field | Required | Content |
| --- | --- | --- |
| `owner` | yes | the seat that started the game with the card in their deck or sideboard, or that brought it into the game from outside (108.3) |
| `oracle` | yes | the Oracle id (UUID) of the card |
| `name` | yes | the card's full name as the Oracle data prints it (both faces joined by ` // ` for multi-faced cards) |
| `faces` | no | per face, in face order, the SHA-256 of the canonical JSON of the face's pinned row restricted to `{name, mana_cost, type_line, oracle_text, colors, color_indicator, power, toughness, loyalty, defense, keywords}`, with the values exactly as the pinned Oracle data stores them (`name` is the face's own name; `colors` as letters such as `["W"]`, `keywords` as printed, `power` as a string), fields the row lacks omitted, and the three list fields sorted. The digest pins the text the record was written against; it is not spelled in MGN's vocabulary |
| `from` | no | `main` (default), `sideboard` or `command`: a sideboard card is outside the game (400.11a) until an effect brings it in (400.11b); a `command` card is a commander, in the command zone as the game begins (903.6), and belongs only in a record whose `game.format` is `commander` |

A physical id matches `^[A-Za-z][A-Za-z0-9_-]{0,31}$` and must not be a single
capital letter (a seat), the token form `T<n>`, the stack-object form `S<n>` or
the emblem form `E<n>`.
Descriptive ids (`forest1`, `bears`) are fine; the id is not compared across
records (section 7.2).

### 2.4 The initial position

The optional [`start.causal` amendment](causal-starts.md) adds enduring causes,
historical object labels and an exact current-turn cast inventory to the
observable position. Its explicit category-completeness rules supersede the
legacy empty-history premise for those starts. It does not establish arbitrary
resumable snapshots. The [scenario constructor](scenario-setup.md) covers ordinary
priority positions; recorded actions establish more involved contexts. Expressing
a transition does not require directly constructing its intermediate execution state.

`start.kind` is `decks` or `position`.

**`decks`.** The game begins before the starting player is determined (103.1).
Every `main` card of a seat is in that seat's deck, unordered; the first
transitions are the pregame questions of section 5.1 and the first `shuffle`
outcomes (103.3) give each library its order. The initial projection is: `turn`
`{number: 0, step: "pregame", active: null}`, `pending`
`{seat: <chooser>, kind: "choose_first"}`, `result` `{kind: "unfinished"}`, every
seat at `game.life`, every `main` card in its owner's library in the order of
the card table, which does not matter yet (the schema records no other), and
every `command` card in the command zone (903.6; 103.2c puts a commander there
only after the starting player is determined, but no decision can depend on
where it is before then, so the projection shows it there from the first
boundary). The chooser is `match.chooser`
when present; otherwise `start.random` holds one `starting_player` outcome
(section 6) whose `result` is the chooser.

**`position`.** The game is at a boundary: the state in which the record's
first entry is made. `start.state` is a full
projection (section 3.4) and `start.assumptions` states the facts outside the
projection that an engine needs to construct the position:

| Assumption | Default | Meaning |
| --- | --- | --- |
| `land_played` | `0` per seat | lands played this turn by each seat (305.2) |
| `summoning_sick` | `[]` | permanents their controller has not controlled continuously since the beginning of their most recent turn (302.6): a creature among them can neither attack nor activate an ability with {T} or {Q} in its cost unless it has haste (702.10b, 702.10c); every other permanent has been controlled that long |
| `spells_cast` | `0` per seat | spells cast this turn by each seat |
| `consecutive_passes` | `0` | priority passes in succession so far (117.4): from `0` to one less than the number of seats; it matters only when the record's first entry is an illegal attempt followed by an implied pass, since a recorded priority action resets it |

Known gaps of this version: the commander tax (903.8: how often each commander was cast from the
command zone), the commander designation of a card that has left the command zone (903.3) and
commander damage (903.10a) have no assumption and no tier, so a Commander record starts from the
`decks`; the companion revealed and the use of its special action (116.2g) likewise.

Nothing else is pending or in force: no triggered ability is waiting, no delayed
triggered ability exists (603.7a), no continuous effect applies beyond what the
projection's tier-2 facts show as final values, and the turn structure is the
ordinary one: the first seat in `game.seats` took turn 1 and the seats have
alternated since, nothing happened earlier in this turn beyond what the
assumptions state, and nothing of the previous turn is remembered. A start may state any final values (a 5/2 Leonin Skyhunter with
no visible cause); an engine that cannot construct them refuses the setup as
unsupported, so authors should prefer positions that ordinary play reaches. A
record whose premise needs more than this starts earlier and records the actions
that establish it. Those actions are part of the test, not a hidden setup
procedure. Direct setup places objects without entry events and states a position
already at the requested boundary; it does not repeat the current step's
turn-based action or trigger checks. For example, a draw-step priority setup
states the hand and library after the draw.

## 3. The projection

A fact is a JSON path in the state and its value. The projection has two tiers.
Both hold only facts the rules define and a judge could observe at the table.

### 3.1 Tier 1, macro

| Path | Value | Rules |
| --- | --- | --- |
| `turn` | `{number, step, active, occurrence?}`: `number` from 1 (0 before the game), `step` one of `untap upkeep draw precombat_main begin_combat declare_attackers declare_blockers first_strike_damage combat_damage end_combat postcombat_main end cleanup` (or `pregame`), `active` the active seat (null before the game), `occurrence` the number of steps with this `step` name that have begun in this turn counting this one, written only when greater than 1 (an effect added a phase or a step, or a cleanup step repeated) | 500.1, 500.8, 500.9, 501.1, 505.1a, 506.1, 510.4, 512.1, 514.3a |
| `pending` | `{seat, kind, about?}` or null when the game has ended: the question the game waits on (section 5) | 117.1 |
| `result` | `{kind: "unfinished"}`; `{kind: "win", winner, cause}` with `cause` one of `life` (704.5a), `draw_empty` (704.5b), `poison` (704.5c), `concession` (104.3a: the last opponent conceded), `effect` (an effect made a player win, 104.2b, or lose, 104.3e), `opponents_left` (104.2a: the last opponent left the game for another reason); `{kind: "draw", cause}` with `cause` one of `simultaneous` (104.4a), `loop` (104.4b), `effect` (104.4c) | 104.1 |
| `players.<seat>.life` | integer | 119.1 |
| `players.<seat>.left` | boolean, default false: the seat has left the game | 104.3a |
| `zones.<zone>` | the objects in the zone; ordered for `<seat>.library` (top first), `<seat>.graveyard` (oldest first, so the top card is last) and `stack` (bottom first, so the top object is last); any order for `<seat>.hand`, `battlefield`, `exile`, `command` | 400.1, 401.2, 404.2, 405.2 |
| `departed.<Tn>` | optional immutable `{token, owner}` definition for a token first referenced after it ceased to exist; `token` has the same complete definition shape as `objects.<id>.token`. No zone, controller or current characteristics are permitted. Omit the map when empty. | 113.7a, 603.6c, 704.5d |
| `objects.<id>.owner` | immutable seat for a generated token, copy or emblem; defaults to its controller when first introduced, and remains fixed when control changes. Full positions state the owner when it differs from the controller. Physical card ownership appears only in the card table. Stack abilities have only a controller (113.8) and cannot carry this field. | 111.2, 112.2–112.2a, 114.2 |
| `objects.<id>.controller` | seat; defined for permanents, stack objects and prepared copies in exile, default the owner | 108.4 |
| `objects.<id>.copy_of` | id of the object copied, default null | 707.2 |
| `objects.<id>.copy_definition` | a generated spell copy’s immutable intrinsic definition, stated at first appearance when its donor’s unchanged spell definition cannot establish it; `{oracle, face}` names a face from the pinned Oracle file, `{oracle, fused: true}` names both halves of a pinned fuse split spell, or the full characteristics below state a nonprinted definition. Never a definition for a copied ability | 707.2, 707.2b, 707.10 |
| `objects.<id>.copy_face` | explicit preparation-copy exception: `1` selects only the copiable prepare spell characteristics of `copy_of`, through its copy lineage; requires a complete pinned preparation face definition, not an arbitrary token definition. This is a normal single-faced copy, so its own `face` remains 0 | 722.3c, 707.9 |
| `objects.<id>.face` | which face is up, 0-based, default 0 (transformed double-faced permanents, modal double-faced cards cast as their back face, a spell cast as its Adventure, 715.3, or as an Omen, 720.3) | 712, 715, 720 |
| `objects.<id>.fused` | `true` for a fused split spell: both halves supply its characteristics, with `face` 0; absent otherwise. This is a cast selection, not a third face. Copies retain it without being cast again | 702.102, 709.4d, 707.10 |
| `objects.<id>.token` | for a token, its definition: `{name, types, supertypes?, subtypes?, colors, power?, toughness?, keywords?, text?}` as the creating effect defines it (the base for its tier-2 facts); when the effect names no name, the name is the subtypes plus "Token" (111.4): `Goblin Token`, `Clue Token` | 111.1, 111.4 |
| `objects.<id>.kind` | for a generated non-card object: `ability` or `copy`; a preparation copy may also remain in exile (722.3c); with `source` (the object whose ability it is) and `ability` (section 3.3) for an ability, `copy_of` for a copy. An ability on the stack has only the text of its ability and no other characteristics (405.4), so it carries no tier-2 facts; its `controller` is always written | 113.1c, 113.7, 405.4, 707.10 |
| `objects.<id>.targets` | for a stack object: groups of targets, one group per instance of the word "target" in its text, each target a seat or an object id | 601.2c, 115.1 |
| `objects.<id>.modes` | for a stack object: the modes chosen, 1-based in bullet order | 700.2 |
| `objects.<id>.x` | for a stack object: the value of X announced | 107.3a |
| `objects.<id>.division` | for a stack object: amounts divided among targets, aligned with `targets` | 601.2d |
| `objects.<id>.attacking` | a seat, planeswalker id or battle id the creature attacks; `""` for an attacking creature that no longer attacks anything (506.4c); default null | 508.1, 506.4c |
| `objects.<id>.blocked` | true once blockers were declared for the attacker, until it leaves combat; default false | 509.1h |
| `objects.<id>.blocking` | the attacking creatures this creature blocks, default `[]`; a creature it blocked that left combat is no longer listed, while the blocker stays in combat (506.4), which `in_combat` then states | 509.1, 506.4 |
| `objects.<id>.in_combat` | whether the permanent is in combat; stated only where it differs from "attacking is not null or blocking is not empty", such as a blocking creature whose attacker left (506.4) | 506.4 |
| `objects.<id>.cast` | for a cast spell: its announcement and payment choices, as the `cast` answer spelled them: `{cost?, permission?, additional?, convoke?, delve?}`; absent when none are stated. Generated spell copies retain resolution-relevant choices such as kicker and alternative costs under 707.10, but do not assert a fresh permission, convoke or delve payment | 601.2, 707.10 |
| `objects.<id>.chosen` | values chosen for the object as it entered or resolved and retained by a rule or effect ("as this enters, choose a color"): `{color: "red"}`, `{creature_type: "Elf"}`, `{card_name: "..."}`, `{basic_land_type: "Mountain"}`, `{player: "B"}`, `{number: 3}`; default `{}`; a value chosen as the object enters is a fact of the permanent, written in the delta that puts it onto the battlefield | 614.1c, 608.2d |
| `objects.<id>.emblem` | for an emblem: `{text}`, the ability text the creating effect gave it; emblems are `E<n>` in creation order, in the command zone, owned and controlled by the player who got them | 114.1, 114.2 |

### 3.2 Tier 2, observable micro

Recorded for permanents and for objects on the stack, and for a card in exile
or the command zone where a rule or effect puts a fact on it (counters on an
exiled card, a card exiled face down, a plotted or foretold card). A card in a
library, hand or graveyard carries a tier-2 fact only where an effect changes
a printed value, which is rare; nothing in a library carries one.

| Path | Value | Rules |
| --- | --- | --- |
| `objects.<id>.tapped`, `flipped`, `face_down`, `phased_out` | booleans, default false | 110.5 |
| `objects.<id>.counters` | `{kind: count}` with positive counts; kind as section 8.6 spells it; default `{}` | 122.1 |
| `objects.<id>.damage` | damage marked, default 0 | 120.6 |
| `objects.<id>.attached_to` | the object or seat this Aura, Equipment or Fortification is attached to, default null | 301.5a, 303.4 |
| `objects.<id>.power`, `toughness` | the final values after the layer system; default the printed values of the face up, or the token definition, or the copied values, for a creature and for an object that is not on the battlefield; a noncreature permanent has none, whatever its face prints (a Vehicle, a Spacecraft), and a station card has none off the battlefield, written `null` | 613.1, 208.3, 721.2c |
| `objects.<id>.colors`, `types`, `supertypes`, `subtypes` | final values as sorted lists; defaults as for power | 613.1, 105.1, 205.2a, 205.4a, 205.3 |
| `objects.<id>.keywords` | the final keyword abilities, each spelled as section 8.5 says; default the keyword abilities among the printed ones (ability words and keyword actions the Oracle data lists beside them are not abilities, 207.2c) | 702 |
| `objects.<id>.granted` | the text of abilities the object has that are not printed on it and not keywords, each as its Oracle wording; default `[]` | 109.3, 613.1 |
| `objects.<id>.designations` | the rules-defined designations the object has, sorted, spelled as the rules name them: `solved` (719.3b), `saddled` (702.171b), `suspected` (701.60b), `harnessed` (701.64b), `prepared` (722.3a), `plotted` (702.170a), `foretold` (702.143a), `left half unlocked`, `right half unlocked` (709.5c); default `[]` | as listed |
| `objects.<id>.level` | a Class permanent's level, default absent | 716.2b |
| `players.<seat>.counters` | `{kind: count}`, default `{}` (poison, energy, experience, rad ...) | 122.1 |
| `players.<seat>.mana` | mana in the pool at the boundary that carries no restriction or rider, `{W,U,B,R,G,C: count}`, default `{}`; restricted mana is listed under `mana_restricted` only | 106.4 |
| `players.<seat>.mana_restricted` | mana in the pool that carries a spending restriction or rider: `[{type, count, restriction, producer?, chosen?}]`: semantic mana lots as section 5.8 defines them, with complete restrictions, riders and retention in Oracle/rules wording; default `[]` | 106.6 |
| `players.<seat>.speed` | null (no speed) or an integer from 1 | 702.179b to 702.179f |
| `players.<seat>.max_hand_size` | nonnegative integer, default 7; `null` means no maximum hand size | 402.2 |
| `players.<seat>.designations` | rules-defined designations a player has, sorted: `enduring story` (702.195b); default `[]` | as listed |
| `game.monarch`, `game.initiative` | a seat or null | 725.1, 726.1 |
| `game.day_night` | `day`, `night` or null | 731.1 |

Outside both tiers, and therefore tested only through later deltas and illegal
attempts: the sources and durations of continuous effects, restrictions such as
"can't block", delayed triggered abilities (603.7a), summoning sickness (302.6),
and "this turn" memory such as spells cast or lands played.

A record declaring tier 1 must not state tier-2 facts. A record declaring tier 2
must state every tier-2 fact that changes. Where a printed value is not a
number (a `*` power defined by a characteristic-defining ability, 604.3), there
is no default and a tier-2 record states the final value whenever the object is
a permanent or on the stack.

A `departed` introduction must be needed by a source, copy origin or retained
reference at that same boundary. Its ID must be new across both live and
historical identities, use the next token number, and never become live.
Introductions remain unchanged in subsequent full projections; a delta contains
only new introductions. A definition already retained from an earlier live
boundary is reused and must not be introduced again. These rows contain the
intrinsic token definition, including a creating copy effect's defined
exceptions, rather than temporary characteristics or a physical donor's name
standing in for the token. If an acquired copy effect requires provenance this
shape cannot faithfully express, the adapter refuses it. No arbitrary event
history is recorded. A provisional projection discarded inside an implied
stretch commits neither its definition nor its number.

### 3.3 Referring to abilities

An ability of an object is `{face, paragraph, sub?}`: `face` the 0-based face the
ability is printed on, `paragraph` the 1-based paragraph of rules text (113.2c),
`sub` the 1-based position when keyword abilities are strung together on one
line (113.2c, 702). Paragraphs are the lines of the pinned Oracle text: a line
consisting only of reminder text gets no number and parenthesized reminder text
inside a line is ignored (207.2, 207.2a), so a basic land, whose only line is
reminder text, has no numbered ability and is referred to by rule (`305.6`);
the bullet lines of a modal ability belong to the paragraph that introduces them
(700.2); a card with no rules text has no numbered abilities. `sub` is used only
for a line made of two or more keyword abilities. A token's paragraphs are the
lines of its definition's `text`. A copy's abilities are those of the text it
copies (707.2), so a reference on a copy is numbered over the copied object's
text; an ability the copying card keeps or adds as an exception to the copy
("except it has ...", 707.9b) is referred to with `of: "card"`, numbered over
the copying card's own printed text. Those exceptions follow from the card and
are not restated as facts. An ability a rule grants is `{rule: "305.6"}` (a basic land's intrinsic
mana ability); an ability another object granted is `{granted_by: <id>, face,
paragraph, sub?}`, the paragraph being the granting object's. When an effect
acquires abilities from another object (Marvin, Murderous Mimic or Thranduil, the
Elvenking), use `{inherited_from: <effect reference>, granted_by: <effect reference>}`.
`inherited_from` names the declaring donor and its exact ability; `granted_by`
names the source and ability of the acquiring effect. Both require the
`{source, ability}` effect-reference form below and may recursively name inherited
abilities (a donor's rule-granted ability is `{source: <donor>, ability: {rule: ...}}`). The enclosing action or
stack object's `source` is still the object whose ability was activated (113.7),
not the donor. Keeping both branches distinguishes separately acquired restrictions
and linked families (602.5c, 607.5). References name the donor and granting source
at acquisition and their retained text, including copy exceptions; a native index,
stamp or a later donor face is not a portable substitute. A producer must refuse
an acquisition whose frozen text cannot be authenticated by these references.
This representation alone does not prove that the acquiring effect applies.
The inherent source-free triggered abilities use rule references with no `source`:
`{rule: "725.2", sub: 1}` is the monarch's end-step draw; `sub: 2` is its
combat-damage transfer. Initiative's three subscriptions use `{rule: "726.2", sub: n}`
in their printed order, and the speed subscription uses `{rule: "702.179d"}`.
Here `sub` selects a rule's inherent ability, not a printed keyword. Other rule
references have no `sub`. These references must omit `source`, including in an
ability instance and stack object; the stack controller is still mandatory and
is the controller when it triggered (725.2, 726.2, 702.179d). Ordinary abilities
must retain their source. Existing `for` subjects distinguish multiple pending
instances of the same source-free rule; a designation holder is never a source.
An ability on the
stack is referred to by its stack id. A triggered ability awaiting placement is
an *ability instance*: `{source, ability, for?}` where `for` lists the objects or
seats the triggering event concerned when two instances from the same source
would otherwise be indistinguishable. An *effect reference* names a replacement,
prevention, skip or destination effect by its source: `{source, ability}`, or
`{rule: "614.1c"}` for one the rules themselves create.

### 3.4 The full state

A full state (the `position` start, and the explicit projection a checkpoint
hashes) is an object with `turn`, `pending`, `result`, `players` (every seat with
at least `life`), `zones` (every zone of every seat, empty ones included;
`zones` is the only place that says where an object is) and `objects`, which
holds an entry only for objects with a fact that differs from its default, and
always for tokens (their definition) and stack objects that are not cards (their
`kind`). `game` may be omitted when every designation is null. Every card of the
card table is in exactly one zone, except `sideboard` cards, which are in none
until brought in.

Defaults depend on the Oracle data for printed values; a validator without that
data checks structure and consistency and leaves printed values to the engine.

## 4. Transitions and deltas

`transitions[i]` (0-based in the file, numbered from 1 in references: "after
transition 1" is the state after the first entry) is one of:

```json
{"seat": "A", "do": {"kind": "play_land", "card": "forest1"}, "delta": {...}, "random": [...], "note": "..."}
{"seat": "B", "attempt": {"kind": "declare_blockers", "blocks": [["bears", "lions"]]}, "illegal": true, "note": "..."}
```

`seat` must equal `pending.seat` of the state the transition starts from, and
`do.kind` (or `attempt.kind`) must answer `pending.kind` (section 5), except
for `concede`, which any seat may enter at any boundary. A record whose start
is a priority window therefore opens with a priority action (section 5.2): a
declaration or a resolution-time answer can only follow a question of its
kind, and the passes that lead to one are never written. `random`
lists the random outcomes consumed between this decision and the next recorded
one, in the order they were needed (section 6); it is omitted when empty. `note`
is free text, ignored by comparison. An illegal attempt has no `delta` and no
`random` when the next entry is made in the same state; otherwise it carries
the `delta` and `random` of the stretch to the next boundary (section 4.2), and
an empty delta is never written on an attempt. A `concede` may be entered by
any seat at any boundary, whoever is to answer (104.3a); in a two-player game
it ends the game with every object where it is (104.2a). A record may end at
any state: the last entry's delta runs, through any implied null answers the
author chooses to include, to a state that leaves a pending question, or a
result and no question.

### 4.1 The delta

```json
{"turn": {"number": 2, "step": "upkeep", "active": "B"},
 "pending": {"seat": "B", "kind": "priority"},
 "result": {"kind": "win", "winner": "A", "cause": "life"},
 "players": {"A": {"life": 17, "mana": {}}},
 "game": {"monarch": "A"},
 "zones": {"A.library": {"top": ["c12", "c40"]}, "A.graveyard": {"top": ["bears"]}},
 "objects": {"bears": {"zone": "A.graveyard"}, "T1": null, "elves": {"tapped": false, "damage": 0}}}
```

- `turn`, `pending` and `result` are written whole when they change.
- `players.<seat>.<fact>` and `game.<fact>` are written with their new value; a
  value that returns to its default is written explicitly (`"mana": {}`).
- `objects.<id>.<fact>` likewise. `objects.<id>: null` means the object ceased to
  exist: a token or copy outside the battlefield or stack (704.5d, 704.5e,
  707.10a), or an ability that finished resolving or was countered and left the
  stack (608.2n). A card never ceases to exist; it is always in a zone.
- `objects.<id>.zone` moves the object. A card moves only to its owner's library,
  hand or graveyard (400.3). A move into a library must say where with `at`:
  `"top"`, `"bottom"` or `{"from_top": n}` (n from 1; beyond the library's size
  it means the bottom, 401.7), unless a `shuffle` outcome of the same
  transition lists the card, which means it was shuffled in. A delta places at
  most one card by `from_top` into one library unless it gives the library's
  whole `order`. A move onto the stack or into a graveyard puts the object on
  top. When two or more objects move into the same ordered zone at the same
  place in one delta, `zones.<zone>.top` (or `bottom`) must list them in order
  (401.4, 404.3, 405.3), or the delta is ambiguous. This is the resulting
  fact, not a substitute for an `order_cards` decision when the owner chose it; the one exception is new
  stack objects (`S<n>`), which are put on the stack in the order of their
  numbers, lower first, because they are numbered as they are created. A `zone`
  fact equal to the object's current zone means it left and came back (exiled
  and returned by one effect): it is a new object (400.7) and its facts reset.
- `zones.<zone>.top` lists the objects now at the top of an ordered zone, from
  the top down (the first listed is the top, for a library, a graveyard or the
  stack alike); `zones.<zone>.bottom` lists the objects now at the bottom, the
  last listed being the bottom card. Both reorder without moving between zones
  (scry, surveil, "put them back in any order"). A full `order` (top first) may
  be given instead.
- A `shuffle` outcome in `random` sets the library's order as section 6 says; the
  delta repeats nothing of it. Found cards that stay in the library through a
  shuffle (701.24b) are not in the outcome's `order`; the delta's
  `zones.<library>.top` or `bottom` then says where the effect put them.
- When an object changes zone it is a new object (400.7): every tier-2 fact and
  every tier-1 object fact other than its zone resets to its default and the delta
  states only the new object's non-default facts (except a prepared copy cast
  from exile, whose copy definition persists as specified below; an Aura entering attached, a
  permanent entering tapped, a Clone entering as a copy). Status, counters and
  damage are therefore never written for an object leaving the battlefield (the
  validator refuses them). A `sideboard` card brought into the game (400.11b: a
  companion put into its owner's hand, 702.139a) moves from no zone into one.
- A new token is `objects.T<n>: {"zone": "battlefield", "token": {...}, "controller": <creator>}`
  with `n` the next unused token number (section 1: an object that appears in no
  recorded state takes none); the creator owns the token (111.2). A
  new stack ability is `objects.S<n>: {"zone": "stack", "kind": "ability",
  "source": ..., "ability": ..., "controller": ..., "targets"?: ...}` (603.3a); a
  copy of a spell or ability is `{"zone": "stack", "kind": "copy", "copy_of": ...,
  "controller": ...}`. A copied ability remains an ability, has the same source
  as its original, and has no owner or tier-2 characteristics (405.4, 707.10b).
  Its original ability and copied choices must be established by the record;
  the source card's characteristics are never substituted for the ability.
  A spell copy's actual copiable definition is frozen when the copy first
  appears, and copies of that copy inherit it (707.2b, 707.10). The original
  resolving, changing face or being cast again cannot alter a copy already made.
  An unchanged ordinary spell donor can supply the definition as a compact
  spelling. Otherwise the first appearance must include `copy_definition`:
  neither the donor's final characteristics nor a later observation may invent
  the missing definition. For example, Narset's Reversal copies Divination and
  returns it to hand in one resolution; the copy may first appear when another
  player responds afterwards, with `copy_definition: {oracle: <Divination id>,
  face: 0}`. This requires no invented intermediate decision.

  A full `copy_definition` states `name` (a string, or null for no name),
  `mana_cost` (a string of mana symbols, or null for no cost; `{0}` is a cost),
  `color_indicator`, `colors`, `types`, `supertypes`, `subtypes` (arrays, including
  empty arrays) and `text` (the full intrinsic rules text, including an empty
  string). It additionally states `power`, `toughness`, `loyalty` and `defense`
  when the copied definition has them, as strings so a characteristic-defining
  `*` remains a definition rather than a final numerical layer result.
  Missing characteristics in that latter group mean the object has none.
  A double-faced permanent spell instead uses `{faces: [front, back], face: 0
  or 1}`, where each face is a full intrinsic definition as above and `face`
  states the face up when copied, agreeing with the object's `face` fact.
  Both definitions are frozen, including copy
  exceptions, because the resulting spell and token retain both faces (707.10g).
  A compact Oracle definition likewise retains both pinned faces for a
  double-faced permanent spell; a single face of an Adventure remains a
  single-faced spell (715.3b).
  The full form expresses face-down values, token-defined spells and copy
  exceptions (707.2, 707.9); it never substitutes observed power/toughness,
  counters, later granted abilities or other noncopiable layer effects.
  Copying a face-down spell does not require the copy itself to be face down.
  Targets, modes, X, division and retained casting choices remain their separate
  facts under 707.10; this definition grants no new casting permission or payment.

  An explicit definition cannot contradict an authenticated unchanged donor,
  change after first appearance, or turn a copied ability into a spell.
  Subsequent full observations preserve the explicit definition; observations
  using the compact donor spelling need the earlier recorded context once that
  donor changes. Comparison treats equivalent explicit and currently derivable
  forms alike, without rewriting stored records or their checkpoint hashes.
  An adapter unable to authenticate the actual frozen definition reports
  unsupported; this is an execution limit, not a prohibition on authoring the
  full definition. This mechanism describes generated spell copies, not general
  permanent copy effects. A copy of a permanent spell that resolves becomes a new
  token (608.3f, 111.13): the copy is removed with `null` and `T<n>` appears. A
  new emblem is `objects.E<n>: {"zone": "command", "emblem": {"text": ...},
  "controller": <the player who got it>}` (114.2). Numbers are never reused
  within a record.
- A preparation copy is `objects.S<n>: {"zone": "exile", "kind": "copy",
  "copy_of": <prepared permanent>, "copy_face": 1, "controller": ...}` (722.3c).
  This is the only supported exception allowing an `S<n>` object in exile. A direct
  physical preparation card entering the battlefield with its own definition may
  create its prepared copy in that same transition (722.3c); this does not permit
  ambiguous folded capture through a changed copy lineage.
  Casting it moves that same id to `stack`; its `kind`, `copy_of`, `copy_face`
  and controller persist, while announcement choices are written as usual.
  A copy of that spell uses ordinary `kind: "copy", copy_of: <S id>` and
  inherits its normal preparation characteristics (722.3d). Resolving or losing
  the preparation link removes the appropriate copy with `null`; source history
  remains available to copies already made. The reader freezes the pinned Oracle
  identity and preparation face at creation (or the initial admissible position),
  and ordinary spell copies inherit that basis. Casting, later donor changes and
  cessation of an earlier copy do not change those characteristics (707.2b).
  A later observation loaded with its previous state retains the frozen basis.
  No alternative face is inferred from the source's currently displayed face.
  Source copy effects select the actual preparation card's characteristics;
  arbitrary token definitions without a full pinned alternative face are refused.
  A position without previous context asserts that its current donor lineage
  supplies the copied definition; unavailable history of a changed donor cannot
  be reconstructed and is unsupported under section 2.4. This freezes preparation
  copies and ordinary generated spell copies, not general permanent copy effects
  or all of 707.2b.

A delta is complete: after applying it to the state before the transition, the
result is the engine's projection at the next boundary, fact for fact, within the
declared tier.

A delta also agrees with its decision, as far as the next recorded state still
shows the objects concerned: a cast card has left where it was and, while it is
on the stack, carries the announced targets, modes, X, division and cast
choices (a spell that resolved within the stretch shows its effects instead);
a played land has left the hand, unless the next boundary is a question about
it asked as it enters (a replacement effect's "you may pay 2 life", 614.1c); an
activated ability that uses the stack is a new stack object with that source
and ability (602.2a) until it resolves, while a mana ability produces mana and
no object (605.3); declared attackers have their defenders as `attacking` and
declared blockers their attackers in `blocking` while combat lasts; discarded
or bottomed cards have moved; a `choose_targets`, `change_targets` or
`choose_modes` answer is the facts of the object the question was about while
it is still on the stack (one that resolved within the stretch shows its
effects instead; a `change_targets` asked about the object copying a spell,
707.10c, gives the targets of the copy the delta adds).

**Triggered abilities.** When abilities have triggered, they are put on the
stack one at a time in the chosen order, each with its modes and targets chosen
as it is put there (603.3b, 603.3c, 603.3d). The delta that puts them there
(the delta of the `order_triggers` answer, or of whatever transition made them
trigger when one player controls only one) creates the stack objects up to and
including the first one whose modes or targets must be chosen, and sets
`pending` to that question with `about.object` naming it; the object exists at
that boundary with its `targets` still empty. The delta of that answer sets the
facts chosen and creates the following stack objects up to the next such
question or to the end. An ability that finished resolving is removed with
`null` (608.2n).

### 4.2 The stretch between decisions

Between one recorded decision and the next, every question the game asked was
answered with its null answer (section 5.6): every seat passed priority in every
window it held, nothing attacked or blocked, hands were kept, "may" effects were
declined. The delta carries everything that followed: the resolutions of the
stack when all passed (117.4), the end of steps and phases and the turn-based
actions of the ones that began (502.3, 504.1, 510.2, 514.1 when nothing had to
be discarded, 514.2), the triggers that went on the stack and resolved with no
question, the state-based actions, and the next pending question. A delta may
be empty (`{}`) only when no fact changed between the two states.

A stretch is unambiguous because a state never recurs inside it: within a step
a round of passes resolves an object, which leaves the stack (117.4, 608.2n),
or ends the step with an empty stack (500.2), and the next step differs in
`turn` by its number, its step or its occurrence (an additional step or phase
of the same name in one turn, 500.8, 500.9, 514.3a, is told apart by
`turn.occurrence`). So "A held priority and acted again" and "everyone passed
and A acted later" never produce the same next state. The converter from the
engine's own records and the adapter that replays a record use the same rule:
give the null answer until the engine's state is the recorded state in which
the next decision is taken, the first such state being the one. The validator
has no engine; it checks that no null answer is written and that every entry
answers the question pending before it.

A stretch may be long: a turn in which nobody did anything is one delta with the
untap, the draw, the combat with no attackers and the cleanup. The worked
example `priority-and-steps.json` is such a stretch.

Only states in which an entry is made are recorded. A state nobody acts in,
such as a stack holding a spell and the ability it answered while everyone
passes, is in no record unless a seat makes an entry there; what happened in it
is visible through the delta. A question asked while an object resolves (a
search, a "may", a choice of objects, a choice made as the permanent the spell
becomes enters, 614.12a) is a boundary at which that object is
still on the stack: the delta that reaches it creates the object there, with
the number the creation order gives it, if no earlier recorded state showed it.
Likewise a question asked as an effect puts a card onto the battlefield (an
Aura's `attach`, a value chosen as it enters) is a boundary at which the card
is still in the zone it is leaving, and one asked as a token is created
(`choose_defender`) one at which the token does not exist yet; a land being
played stays in the hand through every question asked as it enters (spec 4.1).

An illegal attempt opens a stretch too. When the next entry, a recorded
decision or another attempt, is made in the same state, the attempt has no
delta. Otherwise the question was given its null answer (a refused block
followed by no blocks) and the attempt carries the delta and random outcomes
of the stretch to the next boundary, as `dogfood-triggers-and-flying.json`
shows; of consecutive attempts only the last can.

## 5. Questions and answers

The table gives every question kind of `mgn/1`: who answers, when the rules make
the question mandatory, the `about` of the pending question, and the answer. A
question whose condition holds is asked even when only one answer is legal,
such as a block declaration against a lone flyer; it is recorded only when an
answer other than its null answer was given (section 5.6). An adapter whose engine
asks a question the rules do not mandate (ordering a single trigger) answers it
itself; where its engine makes a forced choice without asking, it checks the
recorded answer against the engine's result. Answers refer to objects by id and
to players by seat. `about` carries exactly the members the table names for the
kind, and nothing for a kind whose column reads `—`.

### 5.1 Pregame

| Kind | Who | When | `about` | Answer |
| --- | --- | --- | --- | --- |
| `choose_first` | the chooser (`match.chooser`, or the `starting_player` outcome) | once, before the game (103.1) | — | `{first: seat}` |
| `companion` | each seat in turn order, starting player first | once per seat, before the shuffle, when the seat owns a `sideboard` card with companion whose condition its deck fulfills (103.2b, 702.139a) | — | `{card: id}` or `{card: null}` |
| `mulligan` | each seat in turn order, starting player first (103.5) | in each round, for every seat that has not yet kept, after the hands are drawn | — | `{keep: true}` or `{keep: false}` |
| `bottom` | a seat that just took a mulligan | after each mulligan taken: as many cards as that seat has taken mulligans (103.5); seats that mulliganed in the same round answer in APNAP order with the starting player first (101.4, 101.4e) | `{count: n}` | `{cards: [ids]}`, bottom-most last |
| `opening_hand_action` | each seat in turn order, starting player first | while the seat holds a card that allows an action from the opening hand (103.6) | — | `{card: id}` or `{done: true}` |

The shuffles (103.3) and the first draws (103.5) are the delta of the last
recorded answer before them: the `choose_first` answer, or the last recorded
`companion` answer when there are companion questions (103.2b comes before the
shuffle; a declined companion is implied, so the shuffles then belong to the
last answer that was written). Which companion a seat revealed, and whether it
has taken the companion special action (116.2g), are facts of no tier: a record
that takes the action therefore starts from the `decks`. Actions a card allows
"any time you could mulligan" (103.5b) have no question kind in this version.
In each
round, all declarations are made before any mulligan is taken (103.5); the
delta of the round's last declaration carries every mulligan of the round, one
`shuffle` outcome per seat that mulliganed, and sets `pending` to the first
`bottom` question; after the bottoms, the declarations of the seats that
mulliganed start the next round. The game begins in the delta of the last
pregame answer: `turn.number` 1 at the first boundary of the first turn, which
is the starting player's priority in the upkeep step (502.4, 503.1); the
starting player skips the first draw in a two-player game (103.8a).

### 5.2 Priority

`pending.kind` is `priority` whenever a seat has priority (117.1); `about` is
absent. Passing is the null answer and is never written (117.3d; section 5.6).
The recorded answers:

| Answer | Fields | Rules |
| --- | --- | --- |
| `play_land` | `card`; `face` when a multi-faced card is played as a face other than 0 | 305.1, 116.2a |
| `cast` | `card`; `fused: true` to cast both halves of a pinned split card with fuse from hand (702.102a), or `face` (the 0-based face cast when it is not the first: a modal double-faced card's back, an Adventure, 715.3, or an Omen, 720.3); `cost`, `permission`, `additional`, `convoke`, `delve` (section 5.7); `x`; `modes` (1-based); `targets` (groups); `division`; `pay` (section 5.8) | 601.2 |
| `activate` | `source`; `ability`; `x`; `modes`; `targets`; `division`; `additional.choices` (the 1-based cost alternatives, section 5.7); `adding`: present exactly when the ability is a mana ability (605.1a), listing all mana types it added and binding any choice of its output (section 5.8), since a mana ability uses no stack (605.3); `pay` | 602.2, 605.1a, 605.3 |
| `special_action` | `action`: `turn_face_up` (116.2b), `suspend` (116.2f), `foretell` (116.2h), `plot` (116.2k), `unlock` (116.2m, with `half` 0 or 1), `companion` (116.2g), `end_effect` (116.2c), `ignore_effect` (116.2d); `object` (the card or permanent); `pay` | 116.2 |
| `concede` | —; any seat may enter it at any boundary, whoever is to answer | 104.3a |

A fused spell has one announcement and payment for both halves' combined mana
cost. Targets, modes, divisions and printed cost-symbol indices follow the left
half before the right half; it resolves left instructions then right (702.102c-d).
`fused` excludes a nonzero `face` and is valid only for a split card with fuse cast
from hand. Its projected spell characteristics combine both halves (709.4d).
A spell copy freezes the fused selection of its donor (707.10); choosing to copy
that spell does not perform a second fused casting.

### 5.3 Combat

| Kind | Who | When | `about` | Answer |
| --- | --- | --- | --- | --- |
| `declare_attackers` | the active seat | in the declare attackers step when the active seat controls a creature (508.1a) | — | `{attacks: [[creature, defender], ...], pay?}`, defender a seat, planeswalker or battle; `[]` for no attack; `pay` (section 5.8) for costs to attack (508.1g) |
| `declare_blockers` | each defending seat | when the seat controls a creature and a creature is attacking them, a planeswalker they control or a battle they protect (509.1a) | — | `{blocks: [[blocker, attacker], ...], pay?}`; a creature blocking two creatures appears twice; `pay` for costs to block (509.1d) |
| `assign_combat_damage` | the creature's controller | for a blocked creature with two or more blockers or a blocker blocking two or more creatures (510.1c, 510.1d), and for a blocked attacker with trample, whose excess damage may be assigned to what it attacks (702.19b); the active seat's creatures are asked first, then the defending seat's, one question per creature (510.1a, 510.1b) | `{creature: id}` | `{assignments: [[target, amount], ...]}`: positive amounts summing to the damage the creature deals, each target a creature it is blocking or blocked by, or under trample the seat, planeswalker or battle it attacks, named as `attacking` names it; a creature not named is assigned none |

### 5.4 Triggers, replacements, state-based actions and resolution

| Kind | Who | When | `about` | Answer |
| --- | --- | --- | --- | --- |
| `order_triggers` | the controller | when one seat controls two or more triggered abilities awaiting placement (603.3b) | `{abilities: [instances]}`, in any order (compared as a multiset) | `{order: [instances]}` in the order they are put on the stack (the last resolves first); the new stack objects take their numbers in that order. Copies of one spell put on the stack at the same time (405.3) are interchangeable until their targets are chosen and need no question |
| `legend_rule` | the controller | when a seat controls two or more legendary permanents with the same name as state-based actions are checked (704.5j) | `{objects: [ids]}` | `{keep: id}`; the others go to their owners' graveyards in the same delta |
| `choose_replacement` | the affected object's controller or the affected player | when two or more replacement or prevention effects would modify one event (616.1), other than the two events with a kind of their own: a resolving spell's destination (`spell_destination`) and a step or phase that would be skipped (`skip`) | `{object: id or seat}`: the affected object or player, or, when the affected object does not exist yet (a token being created), the resolving object whose effect is being replaced | `{effect: effect reference}` |
| `skip` | the affected player (616.1), whose step or phase it is | when two or more skip effects would replace the step or phase about to begin (616.1); the question is asked before it begins, so `turn` still names the step or phase that just ended | — | `{effect: effect reference}`: the object and ability whose effect created the skip effect (614.10) |
| `choose_targets` | the controller | when a triggered ability with targets is put on the stack (603.3d), or an effect lets new targets be chosen | `{object: id}`: the stack object, which already exists with empty `targets` | `{targets: [[...]], division?}` |
| `choose_modes` | the controller | when a modal triggered ability is put on the stack (603.3c, 700.2b) | `{object: id}` | `{modes: [n]}` |
| `change_targets` | the player the effect names, or the controller of the effect ("change the target of target spell", 608.2c) | when an effect changes or lets a player change the targets of a spell or ability (115.7), or lets new targets be chosen for a copy of a spell (707.10c); a mandatory change with no other legal target leaves the target unchanged and asks nothing (115.7a) | `{object: id}`: the spell or ability whose targets change, or, for a copy, the resolving object that copies, since the copy is put onto the stack only once its targets are chosen (707.10c): the copy then appears in the answer's delta as `S<n>` with those targets | `{targets: [[...]]}` |
| `optional` | the player the text names | a "may" while an effect is applied (608.2d) or as a replacement effect applies ("you may pay 2 life; if you don't, it enters tapped", 614.1c); a "may pay" whose cost is not mana (life, a sacrifice) is `optional`, one whose cost is mana is `pay` | `{object: id}`: the resolving object, or the permanent that is entering | `{yes: bool}` |
| `choose_objects` | the player the text names | "choose", "sacrifice", "discard" and similar while an effect is applied (608.2d, 701.9b); a "may" that comes with objects to choose ("you may have it enter as a copy of a creature", "you may put a land card from your hand onto the battlefield") is one `choose_objects` whose answer may be empty, not an `optional` followed by a choice | `{object: id}` | `{objects: [ids]}` |
| `choose_player` | the player the text names | "choose a player", "an opponent" with several opponents, while an effect or a replacement effect applies | `{object: id}` | `{player: seat}` |
| `choose_number` | the player the text names | "choose a number" | `{object: id}` | `{number: n}` |
| `choose_value` | the player the text names | choose a color (105.1), creature type (205.3m), land type, card name (201.4a) or other rules-defined value, while an effect is applied or as the object enters (614.1c) | `{object: id, of: "color" / "creature_type" / "land_type" / "card_name" / "basic_land_type" / "card_type"}` | `{value: string}`; a value the object retains is also its `chosen` fact |
| `choose_mana` | the player who adds the mana | "add one mana of any color" and other mana choices while resolving (608.2d), except output already bound by `activate.adding` or `pay.abilities[].adding`; includes a triggered mana ability, which resolves at once without the stack (605.1b, 605.4a) | `{object: id}`: the resolving object; for a triggered mana ability its source | `{mana: ["R"]}` |
| `divide` | the controller | divide or distribute while resolving, when not announced on casting (601.2d) | `{object: id}` | `{amounts: [[target, n], ...]}` |
| `separate` | the player the text names | "separate them into piles" (700.3) | `{object: id, cards: [ids]}` | `{piles: [[ids], ...]}`, every card in exactly one pile, in the order the text names the piles |
| `pile` | the player the text names | "choose a pile" (700.3) | `{object: id}` | `{pile: n}` 1-based among the piles of the preceding `separate` answer, in its order |
| `attach` | the controller | an Aura entering without being cast and without a stated object (303.4f) | `{object: id}`: the Aura that is entering | `{to: id or seat}` |
| `choose_defender` | the controller | a creature put onto the battlefield attacking (508.4); asked even when one defender is possible (section 5) | `{object: id}`: the resolving object whose effect puts the creature onto the battlefield | `{defender: seat or id}` |
| `discard` | the active seat | at cleanup when the hand exceeds the maximum hand size (514.1) | `{count: n}` | `{cards: [ids]}` |
| `library_destination` | the player the text names | a card put into a library "on top or on the bottom" | `{object: id}`: the resolving spell or ability whose instruction moves the card | `{at: "top" / "bottom" / {from_top: n}}` |
| `order_cards` | the owner of the cards | when simultaneous movement leaves their order to that owner (401.4, 404.3), unless another answer already states that order | `{cards: [ids], zone: "A.library" or "A.graveyard", placement?}`; `placement` is required for a library and uses section 5.4's library-position vocabulary; absent for a graveyard | `{order: [ids]}`: permutation of those distinct physical cards, top first within the inserted group |
| `arrange` | the player the text names | scry (701.22a), surveil (701.25a), "look at the top N cards... put them back in any order": the whole instruction is an ordering of the cards looked at | `{object: id, cards: [ids]}` | `{top: [ids], away: [ids]}`: `top` back on top in order (first is the top), `away` to the bottom in order (last is the bottom) for scry, or to the graveyard for surveil, put there in the listed order so that the last listed is on top. When an effect takes some of the looked-at cards and puts the rest somewhere "in any order", the taking is a `choose_objects` answer; an otherwise unrepresented owner ordering of the rest is `order_cards`. If the instruction gives the arranging player the whole partition-and-order choice, `arrange` already conveys it and no duplicate `order_cards` is asked |
| `search` | the player the text names | search a zone (701.23a) | `{object: id}` | `{found: [ids]}`, possibly empty where 701.23b allows |
| `choose_alternative` | the player the text names | "[do A] or [do B]", "unless" with alternatives | `{object: id}` | `{alternative: n}` 1-based in printed clause order (see below), or `{decline: true}` |
| `pay` | the player the text names | "unless [player] pays [cost]" and "you may pay [cost]" while resolving (118.12), where the cost is mana; a non-mana "may pay" is `optional` | `{object: id}` | `{pay: payment}` (section 5.8) or `{decline: true}` |
| `cast_during_resolution` | the player the text names | an effect allows casting a spell while resolving (608.2g) | `{object: id}` | the fields of `cast` or `{decline: true}` |
| `spell_destination` | the controller | two or more effects would put a resolving spell somewhere other than its owner's graveyard (616.1) | `{object: id}`: the spell | `{effect: effect reference}`: the spell's own keyword ability (buyback 702.27, rebound 702.88, flashback 702.34a) by its paragraph, the ability of another object whose replacement effect applies, `{rule: "720.3d"}` for an Omen spell's shuffle, or `{rule: "715.3d"}` for an Adventure spell's exile |
| `holders` | the controller | proliferate (701.34a) | `{object: id}` | `{holders: [ids or seats]}` |
| `coin_call` | the flipping player | a coin flip that a player wins or loses (705.2) | `{object: id}` | `{call: "heads" / "tails"}` |
| `command_zone` | the owner | a commander was put into a graveyard or exile since state-based actions were last checked (903.9a, a state-based action: the card is in that zone at the boundary), or would be put into its owner's hand or library (903.9b, a replacement effect: the card is still in the zone it is leaving) | `{object: id}` | `{yes: bool}` |

For `choose_alternative`, number the actions in their printed order, before
translating "unless" under 118.12a: "[do A] unless [do B]" numbers A as 1 and
B as 2. Choosing the optional B action is written as `{alternative: 2}`;
declining B is the null answer and implies A. Neither `{decline: true}` nor
`{alternative: 1}` is written for that decline (section 5.6). For example,
Bandit's Talent's "discards two cards unless they discard a nonland card"
uses alternative 2 to choose the nonland discard; declining implies discarding
two cards. A mandatory "[do A] or [do B]" has no decline and writes the chosen
alternative. Numbering does not make an illegal or impossible action available
(608.2d).

### 5.5 Pending kinds

`pending.kind` is `priority` or one of the kinds of 5.1, 5.3 and 5.4. The answer's
`kind` must equal `pending.kind`, except under `priority`, where it is one of the
answers of 5.2. Several questions of one kind about one object in a row (two
"may" clauses of one ability) are told apart by their order alone.

Simultaneous card ordering is an actual answer, independent of an adapter's object
handles and independent of the later expected delta. `order_cards.order` is top
first, including for a graveyard whose full zone listing is oldest first. The
question is before the pending insertion: `about.zone` is the destination, and
`placement` is the position of the group's first card after insertion (top/bottom
or `{from_top: n}`), not the current location of its members. The
answer's delta performs the insertion in that order; a later effect may consume
some or all of those cards before the next boundary. An existing `arrange` answer
already specifies the order of its `top` and `away` partitions and needs no second
ordering question. Preserve an order the rules or effect fix rather than ask for
one; a zero- or one-card group has a unique implied order. Distinct physical cards
are not declared interchangeable merely because they have the same Oracle text.
An engine that lacks the required ordering boundary reports unsupported. An old
record whose automatic native order depended on object handles has an unrecorded
choice, not an authored portable ordering proof; retain its original expectation
and an explicit unresolved disposition rather than sorting it to make replay pass.

### 5.6 Null answers

A question with a null answer is recorded only when another answer was given;
the null answer is implied by the next entry (section 1). The null answer is
implied only where it is a legal answer: where a requirement (508.1d, 509.1c),
a mandatory change of target (115.7a) or a mandatory cast (608.2g) rules it
out, the question has no null answer at that point and its answer is written.
A question without a null answer is always recorded; an engine that asks one
at a point where the record has no entry diverges there, and the record is
incomplete.

| Kind | Null answer | Rules |
| --- | --- | --- |
| `priority` | pass | 117.3d |
| `declare_attackers` | no attackers | 508.1a |
| `declare_blockers` | no blockers | 509.1a |
| `mulligan` | keep | 103.5 |
| `companion` | none revealed | 103.2b |
| `opening_hand_action` | done | 103.6 |
| `optional` | no | 608.2d, 614.1c |
| `choose_objects` | none, where the text lets the player choose none ("up to", "any number", "may") | 608.2d |
| `choose_targets` | none, where the number of targets may be zero ("up to N targets", "any number of targets") | 601.2c, 603.3d |
| `choose_modes` | no mode, where the text allows none ("choose up to one", "up to X") | 603.3c, 700.2b |
| `change_targets` | the targets stay as they are, where the effect allows it ("you may choose new targets", 115.7d); a mandatory change has no null answer (115.7a) | 115.7 |
| `search` | nothing found, where a hidden zone is searched for cards with a stated quality | 701.23b |
| `order_cards` | the unique empty or one-card order; any larger owner-selected order is recorded | 401.4, 404.3 |
| `arrange` | the cards stay on top in their order, none put away | 701.22a, 701.25a |
| `pay` | decline | 118.12 |
| `choose_alternative` | decline, where declining is allowed | 118.12a |
| `cast_during_resolution` | decline, where the effect allows rather than instructs | 608.2g |
| `holders` | none | 701.34a |
| `command_zone` | no | 903.9a, 903.9b |

Every other kind has no null answer: `choose_first`, `bottom`, `assign_combat_damage`,
`order_triggers`, `legend_rule`, `choose_replacement`, `skip`,
`choose_player`, `choose_number`, `choose_value`, `choose_mana`, `divide`,
`separate`, `pile`, `attach`, `choose_defender`, `discard`, `library_destination`,
`spell_destination`, `coin_call`. A written null answer is a validation error
(`null_answer`), so that one game has one spelling.

### 5.7 Costs on casting

`cost` says what cost the spell is cast for (601.2b, 601.2f): omitted or
`{"kind": "mana"}` for the printed mana cost; `{"kind": "alternative", "keyword":
"flashback"}` for a keyword alternative cost of the card (flashback, madness,
morph, disguise, evoke, prowl, dash, blitz, emerge, mutate, surge, spectacle,
escape, foretell, plot, freerunning, harmonize, warp and the like, spelled as
section 8.5 spells keywords); `{"kind": "alternative", "source": id, "ability":
ref}` for an alternative cost another object offers; `{"kind": "alternative",
"effect": "without_paying"}` when an effect lets the spell be cast without
paying its mana cost (118.9). `additional` carries optional and additional costs: `kicker` (times
paid, 702.33), `buyback` (702.27), `entwine` (702.42), `replicate` (times, 702.56),
`paid` (optional additional costs paid, by keyword: `bargain`, `offspring`,
`casualty`, `gift`, `teamwork`), `gift` (the opponent promised, 702.174a),
`choices` (for each "[cost A] or [cost B]" in the cost, the 1-based alternative
paid, 601.2b).

Harmonize's optional tap is part of its cost choice:
`{"kind":"alternative","keyword":"harmonize","tapped":"bear"}`
(702.180). Omit `tapped` when no creature is tapped; it is not valid for another
keyword. `permission: {source, ability}` identifies the ability permitting a cast
from another zone or at another time (601.3), independently of the cost paid.
Its source may be a historically known object that has ceased to exist when
the resolving effect's permission persists (611.2a); the ability still names
that recorded source, never a later incarnation substituted by the adapter.
For example, retrace names the casting card's retrace ability here and records
the discarded land in `pay.objects`; retrace is not an alternative cost (702.81).

`convoke` lists `{object, mana}` entries, one per tapped creature; `mana` is
`W`, `U`, `B`, `R`, `G`, or `null` for one generic mana (702.51). It records the
allocation even when a multicolored creature could pay more than one color.
`delve` lists the cards exiled to pay generic mana (702.66). An object occurs
at most once in either list (118.10). Convoke, delve and harmonize selections
are not repeated in `pay.objects`; that list describes the remaining object
costs in their cost-text order. A spell copy is not cast or paid for (707.10);
its retained choices follow that rule rather than asserting a second payment.

### 5.8 Payment

A payment is explicit about every component, so that no answer means "whatever
the engine's auto-payer does". Unrestricted and restricted mana are selected
separately (106.6):

```json
{"abilities": [{"source": "filter", "ability": {"face": 0, "paragraph": 1},
                "adding": ["G", "G"], "x": 1,
                "additional": {"choices": [1]},
                "pay": {"pool": {"R": 1}, "objects": [["food1"]]}}],
 "pool": {"U": 1},
 "pool_restricted": [{"type": "G", "count": 1,
                      "restriction": "{T}: Add {G}. Spend this mana only to cast a creature spell.",
                      "producer": {"source": "grove", "ability": {"face": 0, "paragraph": 1}}}],
 "allocation": [{"symbol": 1, "mana": "G", "restricted": {
     "type": "G", "restriction": "{T}: Add {G}. Spend this mana only to cast a creature spell.",
     "producer": {"source": "grove", "ability": {"face": 0, "paragraph": 1}}}}]}
```

- `abilities`: the mana abilities activated while paying (601.2g), in order,
  each with its source, ability reference and the mana it added as types
  (`W U B R G C`). Each entry is an activation: `x` and `additional.choices`
  announce its cost choices using sections 5.2 and 5.7; its recursive `pay`
  records its own payment, including object costs, Phyrexian life choices and
  further mana abilities. A nested payment's prior pool is the pool when that
  activation begins, before its own nested mana abilities, not the outer
  payment's initial pool (602.2b, 605.3).
  Like `activate.adding`, the complete output binds any choice of its mana
  types; it has no separate `choose_mana` answer. A distinct spell or triggered
  ability whose output requires a choice asks its own question (section 5.4).
- `pool`: **unrestricted** mana spent that was already in the pool before this
  payment's `abilities`. It has the same type-count vocabulary as
  `players.<seat>.mana`. It never selects restricted mana of the same color.
- `pool_restricted`: restricted mana spent from that same prior pool. Each lot
  has `type`, positive `count`, and `restriction`: the complete applicable
  spending restriction, spending rider and retention terms in Oracle/rules
  wording. When all applicable terms come from one printed producing ability,
  `restriction` is its complete pinned Oracle paragraph, including the cost and
  mana-production clause, with reminder text removed as section 3.3 specifies.
  Use that paragraph's wording even when an engine renders equivalent text
  differently. Keep references such as "the chosen type" in the paragraph;
  frozen chosen values belong in `chosen`. This convention does not supply
  wording or provenance for additional restrictions, riders or retention terms.
  Distinct terms must not be combined. `producer` is the producing
  source incarnation and ability reference; `chosen` freezes the applicable
  chosen values at production using the existing chosen-values vocabulary.
  Include them when otherwise matching lots are not interchangeable. The same
  lot form is used by `players.<seat>.mana_restricted`. Selectors match facts,
  never positions or engine handles. A selector that still leaves distinct
  applicable conditions or riders ambiguous is insufficient evidence.
  Both lot arrays are counted multisets: array order and splitting one complete
  descriptor across several entries have no meaning. For comparison and explicit
  state checkpoint hashes, aggregate entries identical in every field except
  `count`, sum their counts, and sort by the RFC 8785 canonical descriptor
  (excluding `count`) in UTF-16 code-unit order. Never merge differing terms,
  producers or frozen chosen values. Count mismatches remain differences.
- `objects`: for each non-mana component of the cost that chooses objects
  (sacrifice, discard, exile, tap, return), in cost-text order, the objects chosen.
- `life`: for each Phyrexian mana symbol in printed order, whether it was paid
  with 2 life (107.4f).
- `allocation`: where the same mana could pay different symbols with different
  results (restricted mana, hybrid symbols), name the 0-based printed `symbol`
  and `mana` type. Add `restricted` with the semantic lot identity above
  (without `count`) when that unit is restricted; its `type` must equal `mana`.
  An omitted `restricted` selects unrestricted mana, never an arbitrary unit of
  that color. Repeat a symbol entry for each unit paying a numeric symbol.
  Selectors may name mana generated by this payment's abilities as well as
  selected prior mana; `pool_restricted` still specifies only the prior portion.

All fields are optional; omitted `abilities`, `pool` and `pool_restricted` mean
the cost had no mana component. A capture lacking the production-time
restriction or choice provenance must refuse an ambiguous lot selection;
current source characteristics and indistinguishable old/new mana do not
recover that evidence.

An explicitly printed zero mana cost is paid with `pay: {}`. Omitting `pay`
does not select an engine's automatic payment policy.

## 6. Random outcomes

Every random event is written out as an outcome in the `random` list of the
transition that consumed it (or in `start.random` for the starting player). No
seeds. Each outcome names its event so that an engine asking for randomness is
answered by key, not by position:

| Outcome | Fields | Rules |
| --- | --- | --- |
| `shuffle` | `seat`; `order`: every card of that seat's library immediately after the shuffle, top first (cards the same transition then draws or moves away are still listed; cards it puts into the library afterwards with `at`, found cards that stayed in the library out of the shuffle (701.24b) and an object given a fixed position (701.24g) are not); `scope?`: `{excluding: [ids]}` naming the found cards kept out, `{fixed: [[id, "top" / "bottom" / {from_top: n}]]}` naming the fixed object, which the delta places with `at`. For a face-down pile that is not a library, `pile: [ids]` instead of `seat` and `order` a permutation of it | 701.24a |
| `random_order` | `objects` (the set), `order` (a permutation of it) | "in a random order" |
| `random_choice` | `from` (the set), `chosen` (a subset) | "at random", 701.9b |
| `coin` | `caller?`, `call?`, `result` (`heads` / `tails`), `winner?` | 705.1, 705.2; a result forced by 705.3 is not an outcome |
| `die` | `sides`, `result` (the natural result, 706.2) | 706.1 |
| `starting_player` | `method` (`coin`, `die`, `other`), `result` (the seat that chooses, 103.1) | 103.1 |

A `shuffle` outcome's `order` is the library as the shuffle left it: every card
of the seat's library at that moment, once each, cards shuffled in by the same
transition included. The validator checks that every card in the library at the
end of the transition is either in `order` or placed after the shuffle with
`at`, and that `order` names only the seat's cards; which of the listed cards
were then drawn is for replay to check. A library of zero or one card still
records a `shuffle` (701.24e). Within one transition, outcomes are consumed in list order among those
with the same key; two shuffles of different libraries are matched by `seat`.

## 7. Canonical form and identities

### 7.1 Canonical JSON

The canonical form of a JSON value is RFC 8785 (JSON Canonicalization Scheme):
no whitespace, members sorted by name as UTF-16 code units, strings escaped only
where RFC 8785 requires, numbers as ECMAScript renders them. Records hold only
integers, so numbers are plain. A record may be stored in any RFC 8259
formatting (the worked examples are indented for reading); identities are
computed over the canonical form, never over the stored bytes.

### 7.2 Record identity and game identity

The **record identity** is the SHA-256 of the canonical form of the whole record,
written `sha256:<hex>`. Lineage, review log entries and reports cite it.

The **game identity** is the SHA-256 of the canonical form of the record's inputs
after relabeling, so that two correct producers who named their cards differently
and recorded the same game produce the same game identity, and a tier-1 record
and its tier-2 upgrade share one. The inputs are `{cards, start, transitions}`
where `cards` keeps only `owner` and `oracle`, `start` keeps only tier-1 facts of
its state, each transition keeps `seat`, `do` or `attempt` with `illegal`, and
`random`, and deltas, notes and `x-` members are dropped. Relabeling assigns
`c1, c2, ...` to physical ids in order of first appearance when scanning
`start.random`, then each transition's `do`/`attempt` and `random` in order, then
the start state's zones (seats in turn order, each seat's library, hand and
graveyard, then battlefield, stack, exile, command, unordered zones sorted by
`(owner, oracle, id)`), then the remaining cards sorted by `(owner, oracle)`.
Token and stack ids are already canonical.
Within a scanned JSON object, visit members in the canonical UTF-16 key order
of section 7.1; within an array, preserve its order. JSON member insertion
order or pretty-printing must not change the game identity. After relabeling,
normalize each payment's `pool_restricted` array as section 5.8 specifies,
including payments nested inside mana activations. Other action arrays retain
their defined order. Game identity follows the recorded first-appearance mapping;
it does not promise to deduplicate every semantically equivalent action history.
In particular, reordering lots that first introduce distinct producer cards can
change that mapping. Record identity continues to hash the complete stored record
without this payment normalization.

### 7.3 Checkpoints

`checkpoints["<n>"]` is the SHA-256 of the canonical **explicit projection**
after transition `n`: the full state of section 3.4 with every fact of the
declared tier written out for every permanent and stack object (defaults
included), `zones` listing every zone with unordered zones sorted, and `game`
present. Restricted mana lot arrays use the aggregation and order of section 5.8.
Checkpoints are optional and never replace deltas.

## 8. Spellings

Every value a record spells out of a closed set uses these spellings. The
validator derives the long lists from the pinned rules text and card data
(`scripts/mgn/vocabulary.py`).

1. **Seats**: one capital letter, `A` first in turn order.
2. **Steps**: as section 3.1 lists them.
3. **Zones**: `<seat>.library`, `<seat>.hand`, `<seat>.graveyard`, `battlefield`,
   `stack`, `exile`, `command`.
4. **Colors**: `white blue black red green` (105.1). Mana types in pools and
   payments: `W U B R G C`. Mana symbols where a symbol is meant: as printed with
   braces, `{G}`, `{2}`, `{X}`, `{W/U}`, `{2/W}`, `{C/G}`, `{G/P}`, `{W/U/P}`,
   `{S}` (107.4).
5. **Keyword abilities**: the names of section 702 of the pinned rules,
   lowercased, with their parameter as printed after the name: `flying`,
   `first strike`, `ward {2}`, `protection from red`, `equip {3}`, `enchant
   creature`, `islandwalk`. Keep repeated instances (113.2c), mana-symbol
   capitals and parameter case. A keyword that introduces an ability body,
   such as solved or exhaust, contributes its name; the body remains rules
   text, not a parameter. Printed defaults come from the full Oracle text,
   not its keyword search index. A historical observation that lost a cost
   cannot supply it without independent printed or captured evidence; such a
   conversion refuses instead of inventing a parameter. Card types: the fifteen of 205.2a in lowercase
   (`creature`, `land`, `kindred` ...); supertypes: the five of 205.4a in
   lowercase (`basic`, `legendary`, `ongoing`, `snow`, `world`); subtypes as the
   rules print them (`Forest`, `Elf`, `Aura`, `Equipment`, `Time Lord`); the
   schema lists the first two sets in full. Designations in lowercase as
   section 3.2 lists them.
6. **Counter kinds**: `+1/+1`, `-1/-1` and the like as printed (122.1a);
   otherwise the lowercase name the Oracle text uses: `stun`, `lore`, `loyalty`,
   `poison`, `charge`, `double strike`. The validator warns on a kind it does not
   find in the pinned card text and the rules.
7. **Card names**: as the Oracle data prints them.
8. **Dates**: `YYYY-MM-DD`; digests: 64 lowercase hex digits.

## 9. Review log

A record is immutable; its vetting is data next to it, in a `review.jsonl` file
whose lines are:

```json
{"record": "sha256:...", "reviewer": {"kind": "agent", "name": "...", "provider": "...", "model": "..."},
 "date": "2026-10-17", "scope": {"transitions": [1, 40]}, "tiers": [1, 2],
 "verdict": "vetted", "findings": [{"transition": 12, "note": "...", "rules": ["704.5g"]}],
 "receipt": "evidence/...", "supersedes": "sha256:..."}
```

`verdict` is `vetted` (every transition in scope checked against the rules and
Oracle text and found right), `defect` (a transition in scope is wrong; `findings`
say which and why), `inconclusive` (the reviewer could not decide; `findings` say
what is missing) or `replayed` (an engine replay passed; no semantic review
claimed). A record is admitted to a repository when a `vetted` entry covers every
transition at the record's tier. A corrected record supersedes the old one through
`lineage.supersedes` and the old record stays.

## 10. Validation

The validator (`uv run scripts/mgn/validate.py <record> [--oracle resources/oracle/cards.jsonl]`)
performs level L0 of [verification](verification.md) without an engine and prints
one problem per line as `<severity> <code> <path>: <message>`; the exit status
is 1 when any error is present. Codes:

| Code | Meaning |
| --- | --- |
| `schema` | the document does not match the JSON Schema (the message names the keyword and the path) |
| `duplicate_id` | a physical id is also a token or stack id form, or an id is defined twice |
| `unknown_reference` | an action, outcome or delta names an object or seat the record does not have at that point |
| `pins` | a pin is malformed or, with `--oracle`, the Oracle data does not match |
| `oracle` | with Oracle data: a card's `name` or `faces` do not match its `oracle` id |
| `start` | the initial position is inconsistent (a card in two zones, a card in no zone, a token in a library, an ordered zone listing an unknown object) |
| `pending_seat` | the transition's `seat` is not the pending seat |
| `answer_kind` | the action's kind does not answer the pending kind, or its content does not fit the question (a wrong count of cards, an `arrange` that does not partition the cards looked at, an `order_triggers` that is not a permutation of the abilities awaiting placement) |
| `pending` | a pending question lacks the `about` members its kind needs, carries members it does not, or lists fewer than two abilities to order |
| `tier_scope` | a tier-1 record states a tier-2 fact |
| `delta` | a delta is malformed: an unknown fact, a library move without `at`, an ambiguous order, a reorder naming an object outside the zone, `null` for a card, a card moved to another seat's hand, library or graveyard, a new token or stack object without its definition, kind, source or controller, a tier-2 fact on an ability, a blocker blocking a creature that is not attacking, an empty delta on an illegal attempt |
| `coherence` | the delta does not agree with the decision, where the next recorded state still shows the object: a cast card still where it was, or on the stack with other targets, modes, X or cast choices than announced; a played land still where it was, unless a question about it is pending (614.1c); an activated ability on the stack with other targets; declared attackers or blockers without their assignments while combat lasts; discarded or bottomed cards still where they were; chosen targets or modes not on the object asked about; trigger stack objects not in the answered order; a legend-rule loser still on the battlefield. A card the delta moves back into the zone it left is a new object (400.7) and counts as moved |
| `null_answer` | a recorded answer is its question's null answer, which the next entry implies (section 5.6) |
| `ability` | with Oracle data: an ability reference names a face the card does not have, a paragraph beyond its numbered paragraphs, or a `sub` on a line that is not a run of keyword abilities; an activation carries `adding` although its text adds no mana, or lacks it although the ability is a mana ability (605.1a) |
| `shuffle` | a card in the library is neither in the shuffle's `order` nor placed after it, or `order` repeats a card or names one that is not the seat's |
| `random` | a random outcome is inconsistent: an order that is not a permutation of its set, a choice outside its set, a coin whose winner does not follow from the call |
| `result` | the result and the pending question disagree (a finished game with a pending question, or the reverse), or the record ends finished with a wrong summary |
| `spelling` | a value outside its closed set or spelled otherwise than section 8 says (a color, step, zone, mana type, a keyword not in lowercase or not from section 702, a subtype the rules do not list) |
| `counter_kind` | (warning) a counter kind not found in the pinned card text or rules |
| `checkpoint` | a checkpoint names a transition the record does not have |

A record with no errors is well-formed and consistent. Whether its deltas are
right is what replay (L1 to L4) and vetting establish; the validator cannot know
that a Forest produces green mana.
