# MGN text rendering and review queries

`scripts/mgn/render.py` renders records as derived, line-oriented text and parses
that text back to canonical JSON. JSON and [the specification](spec.md) remain
normative. Rendering neither validates Magic semantics nor supplies missing facts.
It preserves the complete record, including unknown extension members, all pins,
notes, empty values, ordering of arrays and illegal attempts with or without a
following implied stretch. Canonical JSON bytes and record identity survive the
round trip; the original JSON whitespace and member order do not.

The text form gives authors, reviewers and diffs a readable, lossless view of
every record and example. It is a small indented mapping with JSON leaf values:
an abbreviated action language would duplicate the action/payment vocabulary and
need escape syntax for extensions. No game semantics or engine behavior is
implemented here.

## Commands

```sh
uv run scripts/mgn/render.py docs/mgn/examples/priority-and-steps.json --out .cache/example.mgn
uv run scripts/mgn/render.py .cache/example.mgn --parse --out .cache/example.json
```

Without `--out`, output goes to stdout. Parsing writes canonical JSON and one
terminating newline. Errors go to stderr with exit status 1. The separate
`validate.py` command checks record structure and consistency. `show.py` provides
a different, state-by-state reading view; its output is not this lossless format.

## Shape

This excerpt shows the syntax; omitted record members are not implicitly supplied:

```text
MGN-TEXT 1
mgn = "1"
tier = 2
note = "The delta includes all implied passes and resolutions. CR 117.4."
start:
  kind = "position"
  state:
    turn:
      active = "A"
      number = 3
      step = "precombat_main"
    pending:
      seat = "A"
      kind = "priority"
transitions:
  1:
    seat = "A"
    do:
      kind = "cast"
      card = "murder"
      targets = [["turtle"]]
    delta:
      pending:
        seat = "B"
        kind = "priority"
      objects:
        murder:
          targets = [["turtle"]]
          zone = "stack"
  2:
    seat = "B"
    attempt:
      kind = "play_land"
      card = "swamp"
    illegal = true
```

Each transition is one recorded decision or illegal attempt. Passes, empty
declarations, keeps and other null answers remain folded into deltas exactly as
the normative JSON states (spec sections 4 and 5.6). Pins are complete values,
never abbreviated digests.

## Grammar and mapping

The first meaningful line is exactly `MGN-TEXT 1`. Blank lines and standalone
comments whose first non-space character is `#` are ignored. Comments are
presentation only: to preserve an authored annotation, use a JSON `note` member.
Inline comments are rejected; `#` inside a JSON string is ordinary string data.

Each subsequent line is one of:

```text
name = JSON-value
name:
  child = JSON-value
```

Names matching `[A-Za-z_][A-Za-z0-9_.-]*` may be bare. Every other name is a
JSON string, including empty names and numeric names. Values use JSON syntax,
including quoted strings and comma-separated arrays; JSON values must have a
canonical representation. Finite floating-point values retain a decimal point or
exponent in text, including inside arrays: `1e+20` stays a float when parsed,
even though canonical JSON spells that value as `100000000000000000000`.
Canonical validation still applies before rendering and after parsing; arbitrary
integer inputs beyond the canonical helper's range remain rejected unless a
large JSON integer token is exactly representable as a finite double or is
already that double's canonical spelling. Thus canonical JSON can also be
reloaded without silently rounding arbitrary large integer inputs.
Duplicate member names, nonfinite numbers and lone
surrogates are rejected. Strings escape embedded newlines, so a value occupies
one physical line. Unicode, including literal U+2028 and U+2029, is preserved.

A block means a JSON object. Indentation is exactly two spaces per level; tabs,
indentation jumps and empty blocks are errors. Write empty objects and arrays as
`= {}` and `= []`. The parser also accepts inline JSON objects as values. The
renderer expands nonempty objects and keeps arrays as JSON values, including
arrays containing objects. It writes members in a deterministic order, placing
record metadata first and decisions before their deltas; other names sort by
UTF-16 code units. This order affects presentation only.

The sole special block is the root `transitions:` array. Its entries have the
form `1:`, `2:`, ... with object members indented underneath, consecutively
numbered from 1. An empty transition object can be written `1 = {}`; an empty
array is `transitions = []`. These numbers are derived array indices and are not
stored members. Nested extension members named `transitions` remain ordinary
objects. The parser requires an `mgn = "1"` root and an array of transition
objects, but leaves semantic record validation to `validate.py`. Syntax errors
identify their physical line number.

## Append-only review queries

```sh
uv run scripts/mgn/reviews.py tests/fixtures/mgn/bant-ordered-arrivals/review.jsonl \
  --record-file tests/fixtures/mgn/bant-ordered-arrivals/record.json --verdict vetted --from 1 --to 9
```

`reviews.py` reads one or more review logs in argument order and emits matching
entries as canonical JSON lines. Each output object has `source` (the supplied
log path), `line` (the physical 1-based line number) and `entry` (the complete
original review entry, retaining reviewer, receipt, supersedes, tiers, findings
and scope). It never edits logs, deduplicates entries, resolves supersession or
promotes replay evidence to semantic vetting.

`--record sha256:...` matches the exact record identity; `--record-file` derives
that identity from canonical record content. Repeat `--verdict` to match any of
`vetted`, `defect`, `inconclusive` and `replayed`. `--from` and `--to` select
review scopes that intersect the inclusive requested transition range. Either
endpoint can be omitted; endpoints must be nonnegative and ordered. Scope `all`
intersects every range. Returned scopes and findings are never clipped to the
query range, and a matching entry does not imply that every transition requested
has been vetted. Transition 0 remains available for start-state scope.

Every line is checked against `review-log.schema.json`, including entries that
filters would exclude. Reversed scopes, duplicate JSON names, blank entries and
malformed JSON refuse the query with source path and line. A terminal newline is
allowed. No matches produces empty output and successful exit status. Review
log order, conflicting verdicts and all earlier evidence remain visible.

## Tests

`scripts/tests/test_mgn_render.py` checks canonical byte equality, record identity
and deterministic re-rendering for every checked-in example, fixture `record.json`
and `draft.json`, with negative examples retained rather than repaired. Focused
cases cover extension metadata, full pins, Unicode, notes, comments, empty values,
finite floating-point boundaries and malformed syntax. Canonical tests also
reload the RFC 8785 number table and reject unsafe noncanonical integer inputs. `test_mgn_reviews.py` checks intersecting scopes, identity
and verdict filters, provenance, duplicates, append stability and malformed logs.
These checks establish tooling preservation and query behavior, not game vetting.
