# Causal position premises

`start.causal` supplements observable `state` with enduring rule causes and
selected historical facts. It is not an engine save file. An adapter constructs
these facts directly, reads them back independently, and refuses facts it cannot
preserve. Replaying an unrecorded earlier action sequence does not implement
this contract. This amendment does **not** establish arbitrary-state restoration.

## Identity and provenance

`objects` maps author-chosen historical labels to a frozen `definition`, the
rules `zone` in which that object existed, and either its physical `card` or its
noncard `owner`. Definitions use shared `copy_definition` vocabulary. A separate
optional `lki` records effective characteristics, controller and status as last
known; copiable values do not establish LKI (707.2, 608.2h).

Optional `current` identifies a live initial object as that same incarnation.
It binds **once** at construction. A later zone change never rebinds the label
to the physical card's new incarnation (400.7). Without `current`, the object
is departed; setup neither places it in its old zone nor lets it act there.
Historical labels cannot collide with live object ids. The alias authenticates
identity, owner and zone, not equality with current copiable values. The frozen
producing definition can differ after copying or transforming without a zone
change; distinct producing occurrences may use different historical labels for
that same live incarnation. These labels grant no additional targeting rights
and supply neither current property queries nor event-time LKI. Native import
still refuses changed or copied source definitions outside its printed profile.

`producers` names individual resolving spell/ability occurrences, with their
`source`, frozen `ability` reference and resolution `controller`. Several effects
can share one producer. Distinct occurrences from one source have distinct
producer ids. Source identity, ability definition and occurrence controller
are separate facts (113.7a, 603.7d–e).

## Causes and history

`continuous_effects` is ordered by creation. Entries state their occurrence
`id`, `producer`, object/player/game recipients or rule-text applicability
`scope`, actual `changes` and `duration` (611.2). Additive and setting P/T,
control and ability changes have semantic forms. An `oracle` change/duration
quotes an exact instruction of its frozen producing ability; it is not arbitrary
executable text. Rule-text applicability and event forms likewise quote that
ability and retain referents through typed bindings. Validation checks quotation
provenance and references; semantic review must still establish that every
required referent and resolved value is present.

`delayed_triggers` describes outstanding subscriptions: `id`, `producer`,
one-based `clause` among the ability's delayed instructions, `when`, `once`,
`duration` and `retained` bindings. Binding kinds include target groups,
exiled/created/destroyed/discarded results, chosen values, event subjects/amounts,
payment outcomes and X. Values distinguish objects, players, numbers, text and
booleans. Chosen values name their semantic `choice`; target `group` is one-based
when needed. These are rule meanings, not compiler registers. A delayed return
keeps the particular exiled object, not whichever object that card later becomes
(603.7c, 400.7j).

Durations distinguish cleanup of an explicit current turn, indefinite lifetime,
the controller's next turn, control-dependent lifetime and exact other Oracle
wording. `when` distinguishes step subscriptions and quoted event conditions.
An outstanding next-end-step subscription imported during end-step priority
waits for a **future** step beginning. Setup does not repeat the current step's
beginning (603.7a).

Optional `timestamp_order` orders named object, counter-on-object and effect
timestamps. It establishes those timestamps only, not a turn schedule or
unlisted temporal state (613.7). `base_controllers` separates initial control
from resolving layered control effects.

`casts_this_turn` is an ordered complete inventory with each frozen spell
`source` and caster `player`. Optional `announcement` reuses cast vocabulary
for X, targets, modes, costs and payments: facts about an earlier announcement,
not instructions to cast again. Omitted permitted announcement choices are
unknown. Counts must equal `assumptions.spells_cast` for every seat. Restoration
does not execute spells or fire cast triggers (603.2).

Completeness is category-specific: complete cast facts do **not** assert that
other earlier events did not happen. The legacy empty-history premise cannot
justify guessed zero values for unrepresented categories. Adapters must reject
their consumers or implement additional causal facts.

## Karn profile and independent readback

Native import currently supports ordinary A-first two-player turns, empty stack
and combat, and priority in upkeep, draw, either main phase, beginning/end of
combat, or the end step. Native turn plans construct these steps directly without earlier turn
actions, entries, casts or targeting events.

Printed single-face spell producers install fixed one-creature effects with
constant additive/setting P/T, control or plain passive keywords, and cleanup or
indefinite lifetime. All enduring sibling instructions must be represented.
Sources remain retained independently of their physical cards. Native timestamp
import currently requires battlefield construction order followed by effect
creation order; other compositions refuse.

Delayed import supports fixed target-group exile results followed by fixed
returns at an unqualified future step, including fixed entry counters. Captures
normalize to the declared retained exile identities. The ordinary compiler's
type/dependency checker validates every needed value; unused earlier bookkeeping
cells are not invented choices. Duplicate/omitted sibling subscriptions refuse.
Split-object histories, including exiling a melded permanent and retaining both
cards, need further native construction. The notation permits multiple results;
Long Road Home's meld ruling is not a universal one-result bound.

Exact current casts support plain cast ordinals, including Monk of the Open
Hand. Native import refuses past announcement choices and historical X, and
consumers requiring other retrospective attributes/categories. An imported Blaze
must not become an X=0 cast for Rhino, Barreling Brute's mana-value query.

Setup's `ok.causal` independently reads subjects, changes, source identity,
duration, delayed timing/bindings and cast sources/casters from native state.
It compares the complete imported inventory, multiplicity and order before the
first action. Later readback tracks surviving imported causes. New causes during
play remain legal; until general export exists they mark
`causal.new_occurrences` unsupported. This scoped readback does not make the
later observable snapshot a complete resumable position.

## Scope of scenario construction

Use [ordinary scenario setup](scenario-setup.md) and recorded actions for new
interaction tests. Cast the spell, make the payment and resolve the effect to
establish its history and retained context. These optional causal premises remain
supported within the profile above; no extension to every native state owner is
planned.

The current constructor still refuses unrepresented histories, nonordinary turn
schedules, copied/noncard historical sources and suspended procedures. A concrete
test may justify a later bounded extension. Complete notation for recorded
actions and outcomes is a separate requirement from constructing every observed
state directly.
