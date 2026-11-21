# The deck roster

`selected-decks.jsonl` is the roster: **2,012 published constructed deck lists**, one per line,
retrieved on 2026-10-08, with event dates from 1994 to 2026. It is the measure of the second
goal ([roadmap](../../docs/roadmap.md)): a deck counts when the engine runs every card of its
main deck. `manifest.json` records the snapshot date, the counts below and the file's hash.

| Format | Lists |
|---|---:|
| Standard | 974 |
| Modern | 228 |
| Block | 168 |
| Pauper | 135 |
| Legacy | 126 |
| Extended | 105 |
| Vintage | 100 |
| Historic | 80 |
| Pioneer | 50 |
| Explorer | 25 |
| Premodern | 21 |

The lists name 8,119 distinct cards across main decks, sideboards and companions. Whole
physical cards are the unit: a double-faced, split or adventure card contributes its published
quantity once.

## Where the lists come from

- **1,451** from [MTGTop8](https://www.mtgtop8.com/) event pages, 17 of them hand-picked.
- **561** from the [Wizards decklist index](https://magic.gg/decklists): Pro Tours, World
  Championships and other premier events.

The roster was chosen from a larger collection of published lists so that every card seen in
that collection stays represented, every format and year observed has a list, and no list is
redundant under those rules. The collection and the selection tooling are not part of this
repository; the roster is a pinned input, like the cards and the rules.

Commander is not in the roster yet: it needs lists from a source that permits redistribution.

## What a row holds

Exact `main` and `sideboard` quantities, a `companion` where one is designated, the `format`,
the event and its date, the `source` URL, the retrieval date, and the hash of the source
document the list was read from. `name_normalizations` records every change from a published
spelling to the Oracle name. Nothing is substituted to make a deck run.

35 rows carry an explicit review flag (`source_issues` or `names_absent_from_pinned_corpus`):
names the pinned corpus does not hold, such as digital-only cards and misspellings, or
quantities the publisher got wrong. They stay in the roster as published and export refuses
them.

## Using a list

Export a list to a deck file, with its companion written beside it:

```sh
uv run scripts/deck_archive.py export mtgtop8:1989:214162 --output .cache/splinter-twin.dck
```

The lists are made of Wizards card names and belong to their publishers; see
[NOTICE](../../NOTICE).
