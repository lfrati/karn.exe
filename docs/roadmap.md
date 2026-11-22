# Roadmap

This document owns the order of work; GitHub issues own bounded work. How we work is in
[AGENTS.md](../AGENTS.md).

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
refreshes them. The queue comes from `scripts/blocker_report.py`
([choosing what comes next](#choosing-what-comes-next)).

The roster is `resources/decks/selected-decks.jsonl`: 2,012 published tournament lists from
1994 to 2026, across eleven constructed formats ([the roster](../resources/decks/README.md)).
Three authored Commander targets are pinned separately in `resources/decks/commander.jsonl`;
they do not change the tournament denominator.

**A deck is the unit, because a deck needs every one of its cards.** The median roster deck
is four cards short, and those cards sit behind hundreds of different blockers, so decks
arrive roughly one per card fixed.

**A card that runs is one the engine compiles.** That is not a proof of correct play. Defects
are fixed as they are found.

## The order of work

This part is written for the agent doing the work. You were given a lane, the review pass or
the whole round. A round is one builder per lane working in parallel, then one review pass.
The record is the commit log and the README counts; nothing else needs writing up.

Integrate and publish ready committed work within 30 minutes, using one grouped make check,
without waiting for newer unfinished work.

### If you are a builder

Work in your lane's worktree (`keywords`, `closers`, `grammar` or `mechanisms`). Make it if it is missing:

```sh
git fetch origin
git worktree add ~/git/karn-wt/<lane> -b <lane> origin/main
cd ~/git/karn-wt/<lane>
```

It builds into its own `target/`, so the first build is cold. Run builds and tests through
`~/git/karn-wt/bin/slot`, which shares the machine between the lanes.

For each target:

1. Take the next target in the lane.
2. Build it in the owner the [architecture map](architecture-map.md) names. Ironsmith
   (`~/git/ironsmith`) shows one way each mechanism can be modelled; look when it saves time.
3. Test it on real cards, with expectations from their Oracle text and the rules.
4. Run the blocker report against the snapshot taken before the change. `newly_admitted` is
   what was gained; `no_longer_admitted` should be empty.
5. Rebase on `origin/main`, run `make check` and push to `main`. One commit per target, with
   the cards that now run in its message.

A target that has not moved a card after two real attempts is skipped: add a line under the
lane's *Skipped* saying what it needs, and take the next one.

```sh
uv run scripts/blocker_report.py --out .cache/before                       # before a target, about 30 s
uv run scripts/blocker_report.py --out .cache/after --compare .cache/before/admission.jsonl
cat .cache/after/admission-compare.json                                    # newly_admitted, no_longer_admitted
~/git/karn-wt/bin/slot make check                                          # builds and tests go through slot
git fetch origin && git rebase origin/main                                 # then make check again if main moved
git push origin HEAD:main
```

A wrong-play defect met on the way is fixed if it is small. Otherwise it gets an issue with a
reproduction, and the work goes on.

The listed targets are starting priorities. After they are done or skipped, carry on from the
blocker report as the lane describes, in small coherent batches, until the user explicitly
stops the builders.

### If you are doing the review pass

Do it once the user stops the builders, from the main checkout after `git pull`, in a session
that wrote none of the round. The round began at the `round-start` tag.

Read `round-start..origin/main` and fix what you find: duplicated mechanisms, patches for one
card, cards that run but play wrong. A card you cannot make right goes back to refused. Then
refresh the README counts (`make readme-progress`), compare speed with `round-start`
([performance](performance-replay.md)) and push. Finish by removing each lane's worktree and
branch: `git worktree remove ~/git/karn-wt/<lane>` and `git branch -d <lane>`.

### If you were given the whole round

Move the tag to where the round starts (`git tag -f round-start origin/main`), start one
builder per lane as a subagent, keep them working until the user explicitly stops them, and
then run the review pass in a fresh one. If you cannot start subagents, take Lane 1 yourself.

### This round

The numbers are from the blocker report at `80dae61` (2026-11-22). They estimate what a target
unlocks. A card can show a second blocker once the first is gone.

#### Lane 1: keywords (worktree `keywords`)

132 keywords are recognised and not implemented. They block 2,161 cards, and for 1,464 of
those the keyword is the only known blocker. Each keyword is declared once, in
`crates/game/src/keyword.rs` and `keyword/declaration.rs`, and its entry in rule 702 is the
specification. A keyword whose rule is one sentence about its permanent, as flying's is, may
need only a row in `SENTENCES`.

| Keyword | Cards it alone blocks | Roster decks it touches |
| --- | ---: | ---: |
| Fear | 50 | 29 |
| Intimidate | 25 | 6 |
| Shadow | 35 | 25 |
| Horsemanship | 28 | 0 |
| Exalted | 36 | 44 |
| Flanking | 20 | 18 |
| Bushido | 28 | 4 |
| Bloodthirst | 20 | 4 |
| Echo | 48 | 31 |
| Cumulative Upkeep | 40 | 5 |
| Affinity | 56 | 48 |
| Dash | 19 | 33 |
| Unearth | 49 | 26 |
| Storm | 27 | 83 |
| Cascade | 30 | 32 |
| Suspend | 33 | 63 |
| Overload | 18 | 29 |
| Miracle | 14 | 31 |

That is 576 cards if every estimate holds. If the first three keywords gain less than half of
their numbers, the estimate behind this lane is wrong: say so in the commit message.

After these, take the `keyword:` labels of the report by cards blocked alone (Bestow,
Soulshift, Myriad, Splice, Improvise, Aftermath and on). Banding, Partner, Mutate, Daybound
and Nightbound need larger machinery and can wait.

Some tests use today's refusals as examples: `crates/game/tests/engine/admission_blockers.rs`
expects `keyword: Fear`. When a keyword lands, point them at one that is still refused.

*Skipped:*

- Echo moved to Lane 4: printed fixed mana costs use observed per-player acquisitions and actual upkeep boundaries, including repeated upkeeps; nonmana costs, grants and unproved positioned history remain refused.
- Unearth: the graveyard activation, haste and delayed exile lower, but the full rule and replacement adaptation still need an all-departure exile replacement.
- Cascade moved to Lane 4: printed instances share the retained library scan with Discover, check the resulting spell's mana value and return uncast exiles in a captured random order; grants remain refused.
- Suspend moved to Lane 4: fixed-count printed suspend now has its hand action, exile-counter scope and last-counter cast; variable announcements and grants remain refused.
- Overload: rewriting a target-free effect lowers, but the full rule needs announcement-time text substitution and selection of that executable before targets are chosen.
- Miracle moved to Lane 4: printed mana/X costs pause the first actual draw for an explicit reveal and link their exact hand incarnation and price to a trigger. Visibility spans the original occurrence through resolution; grants, ambiguous same-row prices, unknown positioned first draws and deferred announcement-time disclosure remain refused.
- Bestow: cost-only casting cannot declare Aura targets, and the alternative-characteristics owner accepts only Adventure/Omen instants or sorceries; needs a bestowed state through casting, illegal-target resolution and unattachment, preserving the permanent's incarnation.
- Myriad: full and split rule expansions stop at mixed referent defenders and attacking-copy lowering; needs a per-opponent optional copy loop that preserves the chosen player/planeswalker constraint and captures the resulting group for delayed exile.

Cumulative upkeep currently supports fixed generic mana only; colored, life and choice-bearing repeated costs remain refused.

#### Lane 2: deck closers (worktree `closers`)

Current priority, in order: Liliana of the Veil; Atraxa, Grand Unifier; The Wandering
Emperor; Jegantha, the Wellspring; Time Vault; Intuition; Hydroid Krasis; Once Upon a Time;
Tishana's Tidebinder; Preacher of the Schism; Agadeem's Awakening; Sylvan Library; Oath of
Druids; Heart of Kiran. These newly readable cards need engine consumers. Then take the
report's blocker families ranked by decks completed (`sole_decks`), continuing until the
user explicitly says stop. Publish coherent completed batches along the way.

Mechanisms in the program and binding owner (`crates/game/src/program.rs` and `program/`),
each the last thing keeping several roster decks from running. One builder: these all meet in
the same files.

Completed foundations:

- [#33](https://github.com/lfrati/karn.exe/issues/33): copy exceptions discard a Room's
  halves. This is known wrong play.
- The report sorts blockers by the decks they touch, which does not predict what they
  complete. Sort `deck-labels.jsonl` by decks completed (`sole_decks`) and count the cards
  each label alone blocks over the whole corpus, so later targets come from the report.

Then, by the report's label:

| Blocker | Cards such as | Decks it completes, about |
| --- | --- | ---: |
| result group: no earlier instruction did what "Some(Reveal) this way" names | Delver of Secrets | 6 |
| optional compound instruction by another chooser not lowered | Chain Lightning, Chain of Vapor | 5 |
| selection not lowered: Indefinite(UpTo(Const(2))) | Collected Company | 5 |
| program: lead not lowered: Modifier | Urza's Rage, Gideon Jura | 5 |
| reference: the exiled card, of the ability that exiles it | Expressive Iteration | 5 |
| mana: of Chosen | Utopia Sprawl, Coldsteel Heart | 5 |
| program: a cast offered to another player | Spell Queller | 4 |
| description: ThatType | Distant Melody, Tsabo's Decree | 4 |
| modifier: ForEach of Counter | Circular Logic | 4 |
| noun phrase: an unsupported zone or zone owner | Crime // Punishment, Relic of Progenitus | 4 |
| program: optional compound clause or other chooser not lowered | Browbeat, Vexing Devil | 3 |
| token: Those | Conqueror's Pledge, Gather the Townsfolk | 3 |

The widest labels are many mechanisms under one name: `input reference: nothing before it
supplies the required participant/cardinality` holds 278 cards and `program: control flow not
lowered` 394. Take a family of cards inside one, not the label.

This lane is slower than the keywords: each target is a different mechanism. It is here
because it is what moves the deck count.

*Skipped:*

- Time Vault: direct replacement lowering and ordinary optional-skip composition both stop
  before a resumable proposed turn. [#37](https://github.com/lfrati/karn.exe/issues/37) tracks
  retaining that candidate, competing skips and the follow-up in the next actual turn.

- Oath of Druids: typed chooser/predicate lowering and an actor-owned optional reveal attempt
  still stop at moving the matched creature and the other revealed cards to two destinations.
  [#38](https://github.com/lfrati/karn.exe/issues/38) tracks the retained chooser/result partition;
  incomplete experiments were removed.

- Chain Lightning / Chain of Vapor: two narrowed copy probes still require optional
  copying, an explicit copy controller and one-target retargeting; existing stack-copy
  execution assigns the effect controller. Both complete cards remain refused.
- Leading modifiers (Urza's Rage / Gideon Jura): narrowed damage and attack-requirement
  probes need per-damage prevention scope and next-turn rules requiring a particular
  defender. Neither maps safely to the current global prevention or turn lifetimes.
- Browbeat / Vexing Devil: named-player and any-player damage offers both need a
  resumable optional damage action with per-player outcomes. Object-payment outcomes
  do not represent that choice. Both complete cards remain refused.

#### Lane 3: grammar (worktree `grammar`)

Unread phrasing families, in `crates/grammar` only. 839 roster decks hold an unread card and
581 of them hold exactly one. Reading a card does not make it run: it moves the card into the
other lanes' queues.

Read the family each of these belongs to, most roster decks first: Liliana of the Veil (41),
Atraxa, Grand Unifier (38), The Wandering Emperor (31), Jegantha, the Wellspring (29), Time
Vault (24), Intuition (22), Hydroid Krasis (21), Once Upon a Time (21), Tishana's Tidebinder
(20), Preacher of the Schism (19), Agadeem's Awakening (18), Sylvan Library (17), Oath of
Druids (16), Heart of Kiran (15). After these, take unread roster cards from the report.

The word where reading stops is not the unit of work. One word is the stop of many different
phrases, and a rule written for one card's sentence is a patch. The unit is a **family**: one
way of saying one kind of thing, read wherever it occurs. [The grammar](grammar.md) has the
method and the tool that finds the families. Cards no roster deck names still count toward
the first goal: a family's rule covers them too.

*Skipped:*

- Drain Life: direct capped-gain and scalar-minimum decompositions both stop in the amount
  grammar. Needs receiver-specific ceilings and pre-damage life/loyalty snapshots; a current
  statistic or a sum of recipient limits would change its meaning.

#### Lane 4: mechanisms (worktree `mechanisms`)

Take Suspend, Cascade, Echo, Miracle, Bestow, Overload and Unearth, in that order.
Read each mechanism in the pinned Ironsmith checkout before designing it, then build the
missing capability named in Lane 1's skip notes. The two-attempt skip rule does not apply:
stay with each mechanism until actual Oracle cards run with tested behavior. Publish useful
steps that pass `make check`, continuing immediately after a structural step while its cards
remain refused. Continue until the user explicitly stops the builders.

#### Not this round

The Commander targets, [#34](https://github.com/lfrati/karn.exe/issues/34) (MGN cannot say
which flashback cost was chosen) and [#35](https://github.com/lfrati/karn.exe/issues/35)
(`make progress` reports no attributed rules from its snapshot).

### Choosing what comes next

- For decks, take the blocker that completes the most roster decks. For cards, take the
  blocker that alone keeps the most cards from running. The blocker report gives both.
- Removing one blocker often uncovers the next on the same card. A card counts when it runs.
- A defect in a card that runs outranks new cards.

## Out of scope

- Move search, playing policies and anything that chooses for a player. They are callers of
  the engine and live outside this repository.
- A second recording format. Integration tests are MGN records ([testing](testing.md)).
- Refreshing the pinned rules, cards or decks as a side effect of other work. The corpus is
  dated 2026-10-11 and the rules 2026-09-25; a refresh is its own piece of work.
