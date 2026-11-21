# Roadmap

This document owns the order of work; GitHub issues own bounded work. The rules the work is
held to are in [AGENTS.md](../AGENTS.md).

## The aim

The work is the engine. It has two goals and one bar:

1. **Read every card.** The grammar reads the Oracle text of every card in the pinned corpus,
   whether or not the engine runs it yet.
2. **Run most of Magic.** Someone who brings a real game should usually find every card in both
   decks available.
3. **What runs is right.** Every transition the engine implements matches the Comprehensive
   Rules, the Oracle text and the rulings. What it cannot do, it refuses with a reason.

## Where it stands

The [README](../README.md#where-it-stands) carries the current numbers; `make readme-progress`
refreshes them. The queue comes from `scripts/blocker_report.py` ([the rule](#the-rule)).

The roster is `resources/decks/selected-decks.jsonl`: 2,012 published tournament lists from
1994 to 2026, across eleven constructed formats ([the roster](../resources/decks/README.md)).
Commander is not in it yet: it needs lists from a source that permits redistribution.

**A deck is the unit, because a deck needs every one of its cards.** The share of decks that
run rises slowly at first and then quickly, as the same missing cards are removed from many
decks.

**A card that runs is one the engine compiles.** That is not a proof of correct play. Defects
are fixed as they are found.

## The order of work

Two streams, both ordered by published decks.

### Engine: run what is read

A card named in a roster main deck that the grammar reads and the engine does not run stops at
a blocker, said in the engine's own words. The blocker report lists each blocker with the
roster decks it keeps from running and the decks it alone holds. The work is a long tail,
taken widest first.

### Grammar: read every card

This stream goes first: a roster main deck that holds an unread card cannot run whatever the
engine does.

The word where reading stops is not the unit of work. One word is the stop of many different
phrases, and a rule written for one card's sentence is a patch. The unit is a **family**: one
way of saying one kind of thing, read wherever it occurs. [The grammar](grammar.md) has the
method and the tool that finds the families.

A family is taken whole, most roster decks first. Cards no roster deck names still count
toward the first goal: a family's rule covers them too.

### The rule

- Take the blocker or the family that touches the most roster decks. Build it once, in its
  owning mechanism ([architecture map](architecture-map.md), [grammar](grammar.md)), with tests
  derived from the rules.
- After each merge, refresh the README numbers and rerun the blocker report. The report is the
  queue:

  ```sh
  uv run scripts/blocker_report.py --out .cache/blockers
  ```

- Removing one blocker often uncovers the next on the same card. A card counts when it runs.
- A defect found in a card that runs outranks new cards. It gets a reproduction, an issue and a
  fix.

## Out of scope

- Move search, playing policies and anything that chooses for a player. They are callers of
  the engine and live outside this repository.
- A second recording format. Integration tests are MGN records ([testing](testing.md)).
- Refreshing the pinned rules, cards or decks as a side effect of other work. The corpus is
  dated 2026-10-11 and the rules 2026-09-25; a refresh is its own piece of work.
