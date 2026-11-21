# MGN: Magic: the Gathering Game Notation

MGN describes Magic decisions and their resulting game states independently of
an engine. Authors derive expectations from the rules; adapters replay them on
an engine. Review records which expectations have been vetted.

**The standard for integration tests.** `mgn/1` is an engine-agnostic notation: a
record describes a game in terms of the rules, so the same record can test any
engine through an adapter. Every integration test in this repository is an MGN
record under `tests/fixtures/mgn/`. Karn's adapter is the only one written so
far; an adapter for another engine needs only the
[adapter protocol](adapter-protocol.md). The format is stable and grows by
amending the [specification](spec.md) when a recording needs something it cannot
yet express. Every active record carries a `vetted` review of every transition
at its tier for its exact identity, and `test_mgn_runner` checks this. Start
here if you want to write a sequence or an adapter.

[Scenario setup](scenario-setup.md) places cards and sets ordinary turn/player
facts, then tests use recorded actions to establish the interaction. Restoring
arbitrary intermediate execution states is outside its scope.
[Causal positions](causal-starts.md) have a separate, limited profile for
declared effects and delayed instructions.

| Document | What it is |
| --- | --- |
| [spec.md](spec.md) | the normative specification of format `mgn/1`: record, projection, transitions, every question kind, random outcomes, canonical form, review log, validator codes |
| [schema/mgn-1.schema.json](schema/mgn-1.schema.json) | the JSON Schema of a record; [review-log](schema/review-log.schema.json), [divergence-report](schema/divergence-report.schema.json) and [adapter-protocol](schema/adapter-protocol.schema.json) schemas beside it |
| [verification.md](verification.md) | what replaying a record on an engine checks (levels L0 to L5), and the divergence report |
| [adapter-protocol.md](adapter-protocol.md) | the language-neutral protocol between the runner and an engine's adapter |
| [scenario-setup.md](scenario-setup.md) | build a starting board and zones, then play ordinary actions |
| [causal-starts.md](causal-starts.md) | optional causal premises, independent readback and Karn's bounded construction profile |
| [payment-boundary.md](payment-boundary.md) | the native `quote_payment` boundary through which the Karn adapter binds a recorded payment |
| [rendering.md](rendering.md) | the derived, one-to-one text form for reading and diffing, and the review-log query |
| [examples/](examples/) | worked examples with rule citations in their notes; [examples/negative/cases.json](examples/negative/cases.json) shows mistakes and the validator code each produces |

## Writing a record

You need the specification, the pinned Comprehensive Rules
(`resources/rules/`, the edition `resources/manifest.json` names) and the pinned Oracle text
(`resources/oracle/cards.jsonl`, Standard legality in
`resources/oracle/legal-standard.jsonl`). Nothing about any engine.

1. **Pick the position.** Most sequences start from a `position`: a decision
   boundary with the cards where you want them, usually a main phase with the
   stack empty and the active player holding priority, in which case the first
   entry is a priority action (a land played, a spell cast). To test a combat
   declaration, payment or resolution-time choice, start before the action and
   record the decisions that reach it. A
   complete game starts from the `decks`. The passes before the first entry are
   outside the record.
2. **Name the cards.** Any id you like (`forest1`, `bears`), fixed for the record,
   with its owner, Oracle id and name in the card table.
3. **Declare the tier.** Tier 1 states where everything is, who controls it, the
   stack, life, the pending question and the result. Tier 2 adds status,
   counters, damage, attachments, final characteristics and designations. Vetted
   records are tier 2; start at tier 1 if you would rather let an engine replay
   fill the micro facts in.
4. **Write the decisions.** One entry per decision a seat made: a land played,
   a spell cast, an ability activated, attackers or blockers declared, a target
   chosen, a trigger ordered, a card discarded at cleanup. Never write a pass,
   an empty declaration, a kept hand or a declined "may": those are implied by
   the next entry (spec section 5.6). Each entry carries the seat, the answer
   in the vocabulary of spec section 5, the random outcomes consumed before the
   next decision, and the complete delta to the state in which the next
   decision is taken: every fact that changed on the way, resolutions, steps
   and phases that ended, turn-based actions, triggers and state-based actions
   included, and nothing that did not change. Cite the rules in `note`.
5. **Mark illegal attempts** where the point of the sequence is that something
   is not allowed: `{"seat": ..., "attempt": {...}, "illegal": true}`. The
   accepted answer follows in the same state; when that answer is the null one
   (a refused block, then no blocks), the attempt itself carries the delta of
   the stretch that follows (spec section 4.2).
6. **Validate.**

```sh
uv run scripts/mgn/validate.py my-record.json --oracle resources/oracle/cards.jsonl --identity
```

The validator reports structure, spellings, references, the match between each
answer and the pending question, delta well-formedness, zone and shuffle
consistency, and the tier scope. It cannot know what a card does; whether the
deltas are right is what replay and review establish.

## Replaying a record on Karn

```sh
uv run scripts/mgn/runner.py my-record.json --engine target/fast/game-serve --out .cache/mgn-reports
uv run scripts/mgn/runner.py --active --out .cache/mgn-reports
```

`--active` selects the active records of `tests/fixtures/mgn/inventory.json`.
They must replay clean at every applicable level. `--all` selects every record
the inventory lists; the inventory lists only active records. The flags are
mutually exclusive.

The runner ([verification.md](verification.md)) validates the record, drives
the Karn adapter ([adapter-protocol.md](adapter-protocol.md), the section on
the Karn adapter) through every decision and attempt, moves through each
stretch by having the adapter give null answers until the engine's state is
the recorded one, compares fact by fact and writes one divergence report per
record. It never stops at a divergence; it stops when the engine can no longer
follow the record. A report names only the record's own ids and paths.

## Recording a game on Karn

```sh
uv run scripts/mgn/record.py --driver my-draft.json --tier 2 --out tests/fixtures/mgn/my-scenario
uv run scripts/mgn/show.py tests/fixtures/mgn/bant-ordered-arrivals/record.json --from 1 --to 9
```

The recorder (`scripts/mgn/record.py`) is the producer that writes what the
engine did: it has the runner replay a driver's decisions through the Karn
adapter, keeps the engine's projection at every boundary and writes every delta
anew from it (`scripts/mgn/delta.py`). The result is execution evidence, not
vetting: its `review.jsonl` starts with a `replayed` entry, and its tier-2 facts
are the engine's until a `vetted` entry covers them. The driver (`--driver`) is
a drafted record: the draft's deltas must be right at the draft's own tier,
since they locate the boundaries; recorded at tier 2 from a tier-1 draft, the
tier-2 facts the draft never stated are the engine's and `recording.json` lists
them as `supplied`; a tier-2 draft that states a fact differently from the
engine is a disagreement the rules must settle, refused unless the author
accepts the engine's value for that path with `--accept`. The draft is kept
beside the record as `draft.json`. The recorder fails
closed: a refused action, a boundary never reached, an accepted illegal
attempt, a difference at the start, a fact the engine could not project at a
boundary or could not confirm, each stops it with the reason; the facts the
adapter never projects are left at their defaults, listed in the record's
`x-not-projected` member and named by the `replayed` entry for a reviewer to
confirm. `--out` must be a new directory: a record is immutable, a successor
goes beside it. `show.py` prints a record for a reader, state by state with
card names and the top of each ordered zone marked, from the record's own
deltas, not from any engine.

## From a record to the repository

Validate; replay on the reference engine; if the record was tier 1, upgrade it
to tier 2 from the engine's projection (a new record with `lineage.upgrades`);
review both tiers against the rules and Oracle text; admit with a `review.jsonl`
entry naming the reviewer. Records are immutable; a corrected record supersedes
the old one.
