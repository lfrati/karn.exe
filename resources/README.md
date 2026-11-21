# Pinned Magic source data

The [deck roster](decks/README.md) is a separate pinned input with its own manifest. It is
independent of the four rules/Oracle source files below.

These plain-text inputs are committed so a fresh clone can build and test offline.
`manifest.json` records provenance, SHA-256 hashes, counts and exact byte sizes.
Rules text comes from Wizards of the Coast; Oracle text and rulings come from
[Scryfall bulk data](https://scryfall.com/docs/api/bulk-data).
As of 2026-10-11, the [Wizards rules text](https://magic.wizards.com/en/rules) and the
projected Scryfall ruling records and associations are byte-identical to the pinned files.
Exact source URLs, timestamps and hashes are recorded in the manifest.

- `rules/MagicCompRules-<edition>.txt`: the pinned Comprehensive Rules, unchanged. The edition
  (YYYYMMDD) is named once, as `sources.rules.edition` in `manifest.json`; tools derive the file
  and its effective date from it (`scripts/pinned.py`, `oracle::corpus::rules_path`).
- `oracle/cards.jsonl`: one row per card **face**, identified by `(oracle_id, face)`.
  The corpus excludes non-card layouts, funny sets and cards with no paper-format legality.
  Absent `face` means 0; absent `layout` means `normal`; absent Oracle text or mana cost
  means no text or cost; absent keywords means none. Missing optional characteristics are
  undefined. `{0}` and printed zero power/toughness are retained. Explicit empty color
  arrays are retained, including invalid empty color indicators for readers to reject.
  Every Standard face is identical to the Scryfall bulk dated 2026-10-11. Across the full
  corpus, only keyword ordering on non-Standard Keldon Halberdier differs. The manifest
  records the compared source and output hashes.
- `oracle/legal-standard.jsonl`: `{oracle_id,name}` for every card Scryfall listed as legal
  in Standard (not banned, not merely not illegal) in the bulk the list was made from, one row
  per card, by name; melded faces are left out, as they are no deck entries. `make progress`
  measures engine admission over this pool. Refresh it alone with
  `python3 scripts/fetch_resources.py --legal-only BULK` from a raw Scryfall card bulk; a full
  refresh rewrites it from the same bulk as the cards. The dated tabletop snapshot is
  **2026-10-11T09:01:56.463+00:00**, reconciled against Wizards' sets and bans of that
  date; the pool has 5,163 cards. This date identifies the measured legality pool, not
  implementation correctness or future live legality.
- `oracle/rulings.jsonl`: `{id,date,source,text}`; identical date/source/text records
  are shared. IDs are stable content hashes, not line numbers. Identical wording published
  on different dates remains separate, preserving the source metadata.
- `oracle/card_rulings.jsonl`: `[oracle_id,[ruling_id,...]]` for cards with rulings.
  All references resolve into the other two files. Rulings are evidence to interpret
  against the pinned rules edition, not an infallible substitute for those rules.

The corpus has 32,025 cards / 32,872 faces. Cards are 12.8 MB, rules 0.98 MB, rulings
9.52 MB and associations 2.27 MB: about **25.6 MB** total before Git compression. The
35,752 distinct dated/source ruling records represent 75,908 card associations. The
manifest is the current machine-readable measurement.

Refresh deliberately with `python3 scripts/fetch_resources.py`. This downloads current
Scryfall cards and rulings, filters them, and retains the committed CR edition. To replace
the CR input explicitly, supply `--rules-source PATH` or `--rules-url URL`; changing
editions also requires updating the pin in the script and its consumers. Downloads are
consumed without retaining a raw archive. Local `--cards-source` (filtered faces) and
`--rulings-source` (raw Scryfall data) allow offline imports.

For a dated offline legality import, add `--legal-source-metadata METADATA.json` and
`--standard-scope SCOPE.json`. Metadata is the Scryfall bulk-data object augmented with
`retrieved_at` (a timezone-bearing timestamp) and `sha256` of the exact downloaded bytes,
including gzip compression if present. The importer verifies that hash before writing.
The reviewed scope uses the `sources.legal_standard.scope` structure in the manifest:
`effective_at`, `platform`, `sources`, `legal_set_codes`, `banned_cards`, and concise
reconciliation evidence. The importer checks tabletop availability and the complete ban
set; review the official set/rotation evidence and reprint membership before
supplying the scope. The manifest records the download URL, bulk identity/update time,
retrieval time, exact input hash, filtering policy, output pool hash/count, and official
source URLs/hashes. Undated local imports stay undated; a raw file's mtime is not evidence.
Automatic downloads capture their retrieval and update metadata, but establish a reviewed
tabletop effective date only when a reviewed scope is supplied. Reports display that date
(or `undated`) and bind it to the pool's manifest hash. No build or test fetches these inputs.

After refreshing, run `python3 scripts/card_names.py`, regenerate grammar inventories
with `python3 scripts/inventory.py` and
`python3 scripts/corpus_structure.py --dump-nps --dump-amounts --dump-conditions --dump-triggers`, then run
the workspace tests and grammar gates. Review content and admission changes before
committing the new snapshot. Generated inventories, reports and build outputs remain in
`.cache/` or `target/`; they do not belong here.
