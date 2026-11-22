# Working rules

karn is a rules engine for Magic: The Gathering. These are the few rules that keep it correct.
Everything else is ordinary engineering judgment.

## Correctness

- **The rules decide.** Every transition the engine implements matches the pinned
  Comprehensive Rules (the edition `resources/manifest.json` names), the full Oracle text and
  the rulings in `resources/oracle/`. An illegal move is refused with the state unchanged.
- **Refuse, never approximate.** Text or behaviour the engine does not support is refused with
  a reason. A card that runs wrongly is a defect, not coverage, and fixing it comes before
  adding cards.
- **Expectations come from the rules**, the Oracle text and the rulings, never from what the
  engine currently does. When a test and the engine disagree, check the rules; do not weaken
  the assertion. A recorded game is never edited: a mistaken one is replaced.
- **Count honestly.** Progress is the README's counts, refreshed with `make readme-progress`.
  A card that runs is one the engine compiles: say so wherever a count could be taken for a
  claim of correct play.

## Design

- **Build a mechanism once, where it belongs** ([architecture map](docs/architecture-map.md)).
  A genuinely unique card may use a narrow, visible handler, held to the same tests.
- **Keep the architecture small and coherent.** Other engines are references for mechanisms
  and edge cases, not templates for our structure. Adapt insights into the existing owners;
  uphold Karn's code quality even when the reference implementation does not.
- **A grammar rule covers a family of phrasing, never one card's sentence**
  ([grammar](docs/grammar.md)).
- **The engine evaluates one fully specified move at a time.** It never searches, lists moves,
  fills in a choice, passes priority for a player or answers "would this be legal?". Those
  belong to its callers.
- **Speed is a primary target.** A change to a hot path comes with a matched before and after
  measurement ([performance](docs/performance-replay.md)).

## Sources

- Rules, cards, rulings and the deck roster are pinned in [resources/](resources/README.md).
  Builds and tests never fetch.
- Never copy code from another engine, and never take an expected result from one. Forge is
  GPL-3.0; this repository is MIT.
- For card batches, trace a specific blocker's grammar, reference binding, lowering and
  runtime in the reference engine. Identify the missing Karn mechanism, derive assertions
  from pinned sources, implement it in its owner, then rescan and report compilation gains.

## Checks

```sh
make check                                                           # everything
cargo test --locked -p game --test rules scenarios::<file>::        # one rules module
uv run scripts/rules.py 701                                          # tests citing a rule
uv run scripts/blocker_report.py --out .cache/blockers               # what to work on next
```

New rules tests go in a module of `crates/game/tests/scenarios/` and cite the rule they check
(`cite!`). See [testing](docs/testing.md).
