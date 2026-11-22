# How we work

karn is a rules engine for Magic: The Gathering. The aim is to read every card and run most of
Magic, quickly and correctly.

Speed, quality and correctness are what we strive for, not gates everything must pass before
it moves. Reaching them takes judgment. When holding one of them rigidly would stall the work,
make progress and say what was traded.

## What we care about

- **Progress is cards and decks that run.** The README carries the counts
  (`make readme-progress`); the blocker report says what to take next.
- **A card that plays wrong costs more than one that is refused.** The counts cannot tell them
  apart, so nothing flags it later. When a behaviour is in doubt, check it if that is cheap;
  if it is not, refuse that part with a reason and move on.
- **The rules are the tiebreaker.** The pinned Comprehensive Rules, Oracle text and rulings in
  [resources/](resources/README.md) settle what is right when a test, the engine or a
  reference engine disagree.
- **Ironsmith is a working reference** ([source](https://github.com/Chiplis/ironsmith),
  checked out at `~/git/ironsmith`). Read it, port from it and cross-check against it.
- **Shared mechanisms keep the engine small and fast.** Prefer one implementation in the owner
  the [architecture map](docs/architecture-map.md) names, and grammar that reads a family of
  phrasing ([grammar](docs/grammar.md)). A narrow version that works today is a fine first
  step.

## The process

Builders add mechanisms and keep moving. A review pass closes each round: it cleans up
structure and duplication, and looks hardest at the cards that newly run. So finish the
mechanism before polishing it, and note what you skipped and why. Speed is measured once per
round ([performance](docs/performance-replay.md)), not per change. The
[roadmap](docs/roadmap.md) has the steps and the current lanes.

## Checks

```sh
make check                                                           # everything
cargo test --locked -p game --test rules scenarios::<file>::        # one rules module
uv run scripts/rules.py 701                                          # tests citing a rule
uv run scripts/blocker_report.py --out .cache/blockers               # what to work on next
```

New rules tests go in a module of `crates/game/tests/scenarios/` and cite the rule they check
(`cite!`). See [testing](docs/testing.md).
