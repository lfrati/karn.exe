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

You are the orchestrator; the lanes do the building. Close the previous round first if its
review has not landed. Then move the tag to where the round starts (`git tag -f round-start
origin/main`), take the baseline (`uv run scripts/blocker_report.py --out .cache/round-baseline`)
and rewrite *This round* from it. Start one builder per lane as a subagent, each in its own
worktree, and a fifth, `quality`, alongside them. Quality fixes the wrong play the last review
left open, merges duplication, and reads the cards each builder commit newly runs. While the
builders work, integrate what they push, check their newly running cards, and give a lane
its next targets from a fresh report before its list runs out. Keep them working until the
user explicitly stops them, then run the review pass in a fresh session. If you cannot start
subagents, take Lane 1 yourself.

### This round

The numbers are from the blocker report at `6ca6da8` (round-start, 2026-11-24). They estimate
what a target unlocks. A card can show a second blocker once the first is gone.

#### Lane 1: keywords (worktree `keywords`)

Each keyword is declared once, in `crates/game/src/keyword.rs` and `keyword/declaration.rs`,
and its entry in rule 702 is the specification. A keyword whose rule is one sentence about its
permanent, as flying's is, may need only a row in `SENTENCES`. Unearth, Bestow and Overload
are Lane 4's.

In order, with the cards each alone blocks and the roster decks it completes: Escalate
(Collective Brutality, 3 decks); Soulbond (Silverblade Paladin, 3 decks, with its static
"Selector" subject); Outlast (10 cards, 2 decks); Cumulative Upkeep beyond generic mana (26,
1); the Equip variants still refused (17, 1); For Mirrodin! (10, 1); Spectacle (9, 1). Then by
cards blocked alone: Splice 22, Backup 18, Encore 18, Prototype 15, Awaken 14, Embalm 14 with
Eternalize 11, Unleash 13, Reconfigure 13, Devour 13, Umbra Armor 12, Riot 12, Scavenge 12,
Squad 12, Champion 11, Graft 11, Cipher 11, Blitz 10, Reinforce 10, Jump-Start 9, Emerge 9,
Enlist 9. After these, the report's `keyword:` labels by cards blocked alone. Banding,
Partner, Mutate, Daybound and Nightbound need larger machinery and can wait.

Some tests use today's refusals as examples: `crates/game/tests/engine/admission_blockers.rs`
expects a keyword refusal. When a keyword lands, point them at one that is still refused.

*Skipped:*

- Echo moved to Lane 4: printed fixed mana costs use observed per-player acquisitions and actual upkeep boundaries, including repeated upkeeps; a nonmana echo cost ("Echo—Discard a card.") is paid or declined at the same upkeep trigger; grants and unproved positioned history remain refused.
- Unearth moved to Lane 4: printed unearth returns the card with haste and exiles it at the next end step; a resolved rule exiles the permanent instead of any other departure from the battlefield until it leaves (400.7), and a pronoun for a source the effect returned names the permanent it became (400.7j). Granted unearth (Dregscape Sliver, Sedris) remains refused.
- Cascade moved to Lane 4: printed instances share the retained library scan with Discover, check the resulting spell's mana value and return uncast exiles in a captured random order; grants remain refused.
- Suspend moved to Lane 4: fixed-count printed suspend now has its hand action, exile-counter scope and last-counter cast; variable announcements and grants remain refused.
- Overload moved to Lane 4: printed overload reads the card's text a second time with each "target" changed to "each" (702.96a) and casts that definition for its overload cost, chosen before targets are (601.2b); a copy of an overloaded spell is overloaded (707.10). A changed text the grammar does not read, or one that still targets, is refused; granted overload remains refused.
- Miracle moved to Lane 4: printed mana/X costs pause the first actual draw for an explicit reveal and link their exact hand incarnation and price to a trigger. Visibility spans the original occurrence through resolution; grants, ambiguous same-row prices, unknown positioned first draws and deferred announcement-time disclosure remain refused.
- Bestow moved to Lane 4: printed bestow casts a bestowed Aura spell (a layer-4 effect of the object, copied with a spell), resolves as a creature with an illegal target and becomes a creature, as the same object, once unattached; it ceases to be bestowed at the next state-based check rather than at the instant it is unattached, and phasing (702.103g) is not represented.
- Myriad moved to Lane 4: printed myriad has its controller choose at once the opponents other than the defending player to make a copy for, creates those copies tapped and attacking each its player or a planeswalker they control in one event (508.4), and exiles them at end of combat only if any were made; each instance triggers separately (702.116b). The same per-opponent creation reads "for each opponent, create … tapped and attacking that player" (Adeline, Stampede Surfer), and a token copy can enter tapped and attacking (Flamerush Rider, Loki). Battle Angels of Tyr, Hammers of Moradin and Elturel Survivors stay refused for their other text, and a copy of "the exiled card" (Phantom Steed) for its referent.
- Winning and losing by an effect moved to Lane 4: "you win the game" ends the game at once, every other player losing it (104.1, 104.2b), and "target player loses the game" makes that player lose at once (104.3e), leaving a game that goes on (800.4a); nothing more of a program is done once the game is over. Several players winning at once is refused, and so is a player who controls the resolving spell or ability losing a game that goes on (refused as it happens).

Cumulative upkeep currently supports fixed generic mana only; colored, life and choice-bearing repeated costs remain refused.

#### Lane 2: deck closers (worktree `closers`)

Mechanisms in the program and binding owner (`crates/game/src/program.rs` and `program/`),
each the last thing keeping several roster decks from running. One builder: these all meet in
the same files.

The order is greedy: each card is the one that completes the most roster decks once every
card before it runs (decks completed in brackets). Chain Lightning (6), with Chain of Vapor,
Plasma and Smog; Vendilion Clique (4); True-Name Nemesis (5); Hymn to Tourach (4), with the
49 cards that select at random; Mishra's Bauble (4); Dragon's Rage Channeler (4); Prohibit and
Lead the Stampede (4 each); Urza's Rage and Gideon Jura (3 each). Then Tamiyo, the Moon Sage;
Voice of Resurgence; Cunning Wish; Vedalken Shackles; Blightning; Counterbalance; Chrome Mox;
Threads of Disloyalty; Gempalm Incinerator; Glimpse of Nature; Tasigur, the Golden Fang;
Garruk Relentless; Liliana, the Last Hope; Fires of Invention; Rising Waters; Demonfire; Glint
Hawk Idol; Tribal Flames; Gaddock Teeg; Surgical Extraction; Merrow Reejerey; Urza's Bauble;
Pillar of Flame (2 to 3 each). Chain Lightning and Urza's Rage were skipped last round; their
notes below say what they need, and they still close the most decks. After the list, recompute
the greedy order from a fresh report, or sort `deck-labels.jsonl` by `sole_decks`.

- [#33](https://github.com/lfrati/karn.exe/issues/33): copy exceptions discard a Room's
  halves. This is known wrong play.

The widest labels are many mechanisms under one name: `input reference: nothing before it
supplies the required participant/cardinality` holds 236 cards and `program: control flow not
lowered` 341. Take a family of cards inside one, not the label.

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

Take the family each of these stops in, by decks completed: at "of" (Winding Constrictor,
Cemetery Illuminator; completes 3, held by 15 decks); "monarch" (Azure Fleet Admiral, 2);
"library" (Aven Mindcensor, Gonti); "votes" (Council's Judgment, 2); "," (Pelt Collector, held
by 13); "instead" (Abundance); "until" (Mosswood Dreadknight); "as" (Aluren); "and" (Lunar
Convocation); "or" (Crystal Shard, Turnabout); "/" (Anje, Maid of Dishonor); Ghostfire Blade,
Improvisation Capstone, Master of the Wild Hunt, Twinferno; then "." (held by 24). After
these, take unread roster cards from the report. A card that newly reads must run right or be
refused by the engine; check what the compiler does with each.

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

Take Bestow (29 cards; Boon Satyr completes 3 decks), Unearth (49 cards, 2 decks), Overload
(18 cards; Counterflux, 2 decks), then Myriad and the grants Cascade, Suspend, Miracle and Echo
still refuse, and Echo's nonmana costs. Then the report's mechanism-shaped labels: delayed
triggers on a relative turn or a recurring step (40 cards), keywords granted by a program
(149), continuous changes to players (38).
Read each mechanism in the pinned Ironsmith checkout before designing it, then build the
missing capability named in the skip notes. The two-attempt skip rule does not apply:
stay with each mechanism until actual Oracle cards run with tested behavior. Publish useful
steps that pass `make check`, continuing immediately after a structural step while its cards
remain refused. Continue until the user explicitly stops the builders.

#### Quality (worktree `quality`)

Alongside the builders, from the review of the round before: the wrong play it left open
(Aurora Phoenix's "spell with cascade", Deep-Sea Kraken's "it", "while it's exiled" on
Dinosaurs on a Spaceship and Veiling Oddity); the cards it refused to support properly
(Underworld Cerberus, Loaming Shaman, Increasing Confusion, Rally the Righteous); the
duplication it found (Cascade and Discover's scans, name-checked Suspend haste and Warp,
Rampage/Afflict/Bushido, Echo's JSON search), with unchanged programs and blocker report to
show behaviour kept. Then the newly running cards of each builder commit that their builder
did not test.

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
