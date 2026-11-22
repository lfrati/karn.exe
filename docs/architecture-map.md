# Implemented architecture

Oracle text is normalized and read into typed IR. Card definitions compile that IR into
immutable programs and registrations. Games share those definitions but own their mutable
state, random generator, events and suspended execution. The lexicon shares role signatures
and vocabulary; separate lowering and runtime stages implement the behavior.

| Responsibility | Code |
| --- | --- |
| Pinned source data | [resources](../resources/README.md) |
| Action vocabulary and sorts | [ir/lexicon.rs](../crates/ir/src/lexicon.rs) |
| Typed syntax | [ir](../crates/ir/src/lib.rs) |
| Corpus, normalization and tokenization | [oracle](../crates/oracle/src/lib.rs) |
| Bidirectional grammar | [grammar](../crates/grammar/src/lib.rs); [how it is extended](grammar.md) |
| Immutable definition compilation and admission | [card.rs](../crates/game/src/card.rs), [play.rs](../crates/game/src/play.rs) |
| State and object incarnations | [state.rs](../crates/game/src/state.rs), [state/arena.rs](../crates/game/src/state/arena.rs) |
| Compilation, binding and execution frames | [program.rs](../crates/game/src/program.rs), [binding](../crates/game/src/binding.rs) |
| Suspended parent/child execution | [procedure.rs](../crates/game/src/procedure.rs) |
| Evaluated mutations and replacement proposals | [perform.rs](../crates/game/src/perform.rs), [operation.rs](../crates/game/src/operation.rs) |
| Events and triggers | [event.rs](../crates/game/src/event.rs), [trigger.rs](../crates/game/src/trigger.rs) |
| Effective characteristics and cached queries | [layers.rs](../crates/game/src/layers.rs), [query.rs](../crates/game/src/query.rs) |
| Costs, announcements and payment | [announcement.rs](../crates/game/src/announcement.rs), [payment.rs](../crates/game/src/payment.rs) |
| Explicit actions and pending choice constraints | [play.rs](../crates/game/src/play.rs), [legal.rs](../crates/game/src/legal.rs) |
| Managed player boundary | [simulation.rs](../crates/game/src/simulation.rs), [prompt.rs](../crates/game/src/prompt.rs) |
| JSON server and Python client | [game-serve.rs](../crates/game/src/bin/game-serve.rs), [play.py](../scripts/play.py) |
| Neutral records, assertions and Karn translation | [MGN specification](mgn/spec.md), [MGN adapter](../scripts/mgn/adapters/karn.py), [optional native audit](../crates/game/src/audit.rs) |

`Game.stack` is the Magic stack; `Game.procedures` holds rules continuations. A child owns
its pending decision and delivers its completed result to its immediate parent. Committed
instructions are not replayed to reconstruct progress. Replacement procedures retain proposals
and choices before commitment; triggers consume resulting events at the relevant boundary.

Rollback branches share immutable object slots and copy a slot on its first write. Trigger
delivery filters compiled subscriptions by event kind before querying occurrence context,
using the same current or historical battlefield view as the full match.

Zone changes create new object incarnations. References must distinguish current objects,
last-known information and captured results. Continuous effects query effective characteristics
through layers rather than overwriting printed definitions. Ordinary, partial-layer,
prospective-entry and historical views have different meanings. Timestamps given at once
are recorded as ranges with whose object received each, and applied in APNAP order
(613.7m); no player is asked for the order of their own: an action whose layer results
that choice could change is refused and undone. `SIMULTANEOUS_ORDER` in
[layers.rs](../crates/game/src/layers.rs) states exactly what is refused.

`Simulation` owns the gameplay boundary: correlated requests, chooser validation, accepted
commands and viewer projections. `Game` also exposes low-level state access for fixtures and
tooling; a direct state edit is not equivalent to a rules action. Player prompts expose only
permitted information, and player answers refer to labels visible to that seat. The server's
privileged setup/audit commands are a separate trust boundary. A refusal keeps its kind
(`simulation::Class`: illegal with its cited rules, unsupported, malformed, unreferable,
stale) from the game's `ActionError`/`ResponseError` to the wire, where it is `class`; a
fault is no refusal. Detailed wire commands are documented beside their implementation in
`game-serve.rs`.

The engine validates supplied actions and executes their rules consequences. Callers choose
spells, abilities, targets, modes, payment resources and answers. Pending questions expose
required choice domains and constraints; they do not propose a plan or select an answer.
Move enumeration, mana-source search, playing policies and automatic passing belong outside
the engine. A new mechanic therefore needs its rules and explicit action path, without
also extending a search or player policy.

Mana selection is explicit at every payment boundary: `Move::Paying` supplies the
outer pool, and `Step.with` supplies each planned mana ability's cost. `Step.plan`, `Step.x`
and `Step.life` retain that activation's own recursively authored mana window
and announcements. Both selectors use wire
`mana`. Omission means an empty selection; a nonzero residual mana cost refuses, as
a choice not made (601.2h) when the pool could pay it and as a short pool (118.3a)
when nothing chosen from it could.
The engine checks availability, restrictions and exact payment without choosing
units from the live pool. Quotes describe the supplied prefix and locked cost;
they do not propose a payment. MGN translates recorded choices to these inputs.
A new choice must work through the managed interface and Python client, as well as the rules
operation. Rules tests isolate the mechanism; the Python client tests and the MGN records
exercise its compositions, visibility and suspension through the actual client. See
[testing](testing.md).

The code and tests define current supported subsets. An IR variant or compiled card does not
establish every use, grant, replacement interaction or format rule. Preserve meaningful refusals
until implementing and testing the missing behavior. Track concrete gaps in
[GitHub Issues](https://github.com/lfrati/karn.exe/issues), in the order the
[roadmap](roadmap.md) gives.
