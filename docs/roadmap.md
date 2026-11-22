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

### Current execution plan

The [execution tracker](https://github.com/lfrati/karn.exe/issues/18) carries task status.
The first engine/benchmark pass is implemented and validated:
shared object slots, early trigger filtering, four-player scaling and typed replay
profiling. Bounded choices and cost-source references are implemented too. This foundation
pass landed in [6e28b7b](https://github.com/lfrati/karn.exe/commit/6e28b7b).

The initial core correctness queue is complete:
[#1, “other” binding](https://github.com/lfrati/karn.exe/issues/1),
[#3, private optional choices](https://github.com/lfrati/karn.exe/issues/3),
[#4, Room copies](https://github.com/lfrati/karn.exe/issues/4),
[#5, conditional self-flash](https://github.com/lfrati/karn.exe/issues/5),
[#6, Adventure replacement choice](https://github.com/lfrati/karn.exe/issues/6), and
[#7, cost-reveal lifetime](https://github.com/lfrati/karn.exe/issues/7).
The #6/#7 batch passes the full repository check: 4,757 rules tests and 1,113 tooling
tests. Matched release replays show no material regression. All 32,872 admission rows,
including refusals, remain unchanged; these repairs add no compilation coverage.

An independent Room copy-exception rebuilding defect is tracked separately in
[#33](https://github.com/lfrati/karn.exe/issues/33), outside the initial queue.

Historical card predicates now retain the immediately preceding movement's actual input
([#27](https://github.com/lfrati/karn.exe/issues/27)), reusing ordinary program slots and
departure characteristics. Scavenging Ooze, Cling to Dust and General's Enforcer now compile:
three whole-card gains, with no compilation losses and no additional complete roster main
decks. Deluge of the Dead's face also compiles; Invasion of Innistrad still refuses, so that
is not a whole-card gain. The batch passes `make check`: 4,766 rules tests and 1,115 tooling
tests. Other historical predicates still need their own supported facts and producer proof.
Repeated matched release replay medians are 0.6–1.8% lower than the baseline; no speedup
is claimed. The profiling task below retains these measurements.

Counter results and destination self-replacements now share the existing departure batch
([#28](https://github.com/lfrati/karn.exe/issues/28)). Remand, Memory Lapse, Lapse of Certainty
and Spell Crumple add four compiled whole cards without losses. Remand unlocks one roster
main deck: Celso Zampere's 2006 BUG Control list. `make check` passes 4,780 rules tests and
1,118 tooling tests. Matched release replay medians are 0.2–2.0% lower than the prior batch;
the measurements remain evidence for #31. Hinder's library-position choice remains refused.

Granted flashback now uses ordinary continuous keyword changes and the existing cost
compiler ([#29](https://github.com/lfrati/karn.exe/issues/29)). Snapcaster Mage, Past in Flames
and eight other whole cards now compile, with no losses. Snapcaster unlocks three roster
main decks: Sammy Tukeman's UWR Control, Jason Chung's Blue Moon and Alex Mitas's Jeskai
Control lists. The batch passes `make check`: 4,792 rules tests and 1,122 tooling tests.
Matched release replay medians are 0.2–1.0% lower than the previous batch; no speedup is
claimed. Literal granted flashback prices remain refused. Distinct prices require an
explicit native choice; MGN's missing representation is tracked independently in
[#34](https://github.com/lfrati/karn.exe/issues/34).

Called coin flips now use typed local results and the existing seeded/replayed randomness
([#30](https://github.com/lfrati/karn.exe/issues/30)). Mana Crypt, Stitch in Time and eighteen
other whole cards now compile, without losses. Ral, Monsoon Mage's front face compiles, but
the full card still refuses. Complete roster main decks remain 84. Validation covers 4,802
rules tests and 1,126 tooling cases across the batch and a focused rerun of the updated
capability assertion. Matched release replay medians are 1.0–2.8% lower than the prior batch;
the raw results remain evidence for #31. Optional/conditional producers, grouped/repeated
flips, face-only card instructions and coin replacements remain refused.

**The initial four coverage mechanisms and the grammar/pinning batch are implemented.
Next are native refusal boundaries [#13](https://github.com/lfrati/karn.exe/issues/13) and
hosting reliability [#20](https://github.com/lfrati/karn.exe/issues/20).** Ability text, MGN
repairs, the Commander corpus and profiling remain in the initial execution queue.

Re-rank coverage candidates against fresh blockers when the evidence justifies it. Trace
the specific reference grammar, binding, lowering and runtime; identify Karn's missing
producer, consumer or operation; derive assertions from pinned Oracle, rulings and CR;
implement once in the existing owner; rescan. Broad blocker labels are not promised card
unlocks, and another engine's structure is not a template for Karn.

A batch needs full-card regressions, relevant client/visibility/suspension checks, required
repository checks and a fresh admission report. Hot-path changes need matched before/after
measurements. Report actual whole-card/deck compilation changes and remaining refusals,
including any reductions needed to stop claiming unsupported cards.

Supporting work is separate from that sequence: accurate ability text
([#2](https://github.com/lfrati/karn.exe/issues/2)), native refusal integrity
([#13](https://github.com/lfrati/karn.exe/issues/13)), MGN adapter repairs
([#14](https://github.com/lfrati/karn.exe/issues/14)), hosting reliability
([#20](https://github.com/lfrati/karn.exe/issues/20)). Take them when they help the current
batch or repair the affected interface; they are not one serial prerequisite chain.

The [benchmark default matrix fix](https://github.com/lfrati/karn.exe/issues/15) is complete:
implicit worker counts fit the physical cores available under affinity, while explicit
oversized requests still refuse unless the requested SMT sharing is possible.

The [rules-selection cache fix](https://github.com/lfrati/karn.exe/issues/25) now invalidates
grammar dumps when the manifest or pinning reader changes. The
[two effects ambiguities](https://github.com/lfrati/karn.exe/issues/16) now select canonical
free-card cast and next-turn player-control forms, with family and round-trip regressions.
`make progress` passes all 5,505 Rust workspace tests (4,802 rules tests), 1,127 tooling
tests and all seven grammar gates. The admission rows, including refusals, are unchanged.
The snapshot receipt-path reporting defect found during that run is tracked separately;
its missing attribution does not mean the passing tests did not run.

For Commander, [#17](https://github.com/lfrati/karn.exe/issues/17) starts with a small
representative corpus we author or can include, counting commander(s) too. A bulk archive
is not a prerequisite, and the tournament roster and synthetic four-player benchmarks do
not measure Commander deck coverage.

Further performance work is [one profiling investigation](https://github.com/lfrati/karn.exe/issues/31).
Selective characteristic reuse and long-turn history cost are hypotheses; implement another
cache or index only if Karn's measurements justify a sound, small change. A measured no-go
is a valid result.

### Standing priorities

Two coverage streams, ordered by the target decks. The correctness queue above comes first.

### Engine: run what is read

A card named in a roster main deck that the grammar reads and the engine does not run stops at
a blocker, said in the engine's own words. The blocker report lists each blocker with the
roster decks it keeps from running and the decks it alone holds. The work is a long tail,
taken widest first.

### Grammar: read every card

Resolve an unread phrasing family before downstream engine work for the same target cards.
A roster main deck that holds an unread card cannot compile in full.

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
