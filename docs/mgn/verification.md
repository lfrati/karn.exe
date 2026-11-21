# MGN verification

Replaying a record on an engine compares the engine's projection with the
record's deltas at every boundary. Six levels, each yielding pass, fail or
inconclusive per transition; a report that never names anything engine-shaped.

## Levels

| Level | Name | What must agree | Needs an engine |
| --- | --- | --- | --- |
| L0 | Validate | the record is well-formed and consistent (spec section 10) | no |
| L1 | Drive | the initial position constructs with declared causal premises read back exactly, or the decks and pregame transitions play out; every recorded action is accepted; every illegal attempt is refused with the state unchanged; random outcomes are consumed exactly through the keyed handshake; the terminal result and its cause | yes |
| L2 | Boundary | after every transition: `turn` and `pending`, the two tier-1 facts that say where the game is | yes |
| L3 | Tier 1 | every other tier-1 fact the delta states, and no tier-1 fact changed that the delta does not list | yes |
| L4 | Tier 2 | likewise for tier-2 facts, for records declaring tier 2 | yes |
| L5 | Checkpoints | the hashes of the explicit projection at the declared transitions, when the record carries them | yes |

**Pass** at a level for a transition: every fact of that level agrees. **Fail**:
a fact differs (expected from the record, actual from the engine). **Inconclusive**:
the adapter cannot project a fact of that level (it declared the path
unsupported), or the engine declared the question kind, random form or start
form unsupported; an inconclusive fact is never a pass.

## The runner

The runner reads the record, speaks the [adapter protocol](adapter-protocol.md)
to one adapter. Before transitions, a causal start requires a structurally valid
`ok.causal` setup readback. Every declared causal category is compared exactly,
including source and subject identities, duration, occurrence multiplicity and
list order. Optional undeclared historical categories make no empty-history
claim. Missing readback stops at setup as `unsupported_start` (inconclusive); a
malformed reply is a protocol fault; changed declared premises are L1 divergences
at setup and stop before any action. Adapters remain responsible for independent
native readback: the generic comparison cannot detect an adapter echoing input.

For each transition, the runner:

1. sends the decision (or the attempt) with the seat and the transition index;
2. answers the engine's random requests from the transition's `random` list by
   key (kind, seat, objects), in list order among equal keys; an outcome the
   engine never asked for, or a request the record cannot answer, fails L1;
3. advances through the stretch to the next recorded decision: while the
   engine's projection differs from the expected state (the record's delta
   applied to the runner's own state) on a tier-1 fact, it has the adapter give
   the pending question its null answer (spec section 5.6: pass priority, no
   attackers, keep, decline) and compares again. The first state agreeing on
   every tier-1 fact is the boundary (spec 4.2), and its tier-2 differences are
   reported at L4. The search stops without such a state when the engine's
   `turn` is past the boundary's (a later turn; in the same turn, a step the
   runner saw begin after the boundary's step and occurrence, or, where neither
   step is one an effect added, a later step in the order of 500.1: added steps
   and phases have no fixed place in that order, 500.8, 500.9, so with them the
   turn number is the bound), when a question without a null answer is asked,
   when the game ends, or at a state that agrees on `turn` and `pending` with
   a priority window over an empty stack or a combat declaration: null answers
   alone cannot bring that seat the same question again in that step (a pass
   ends the window and nothing resolves, 117.4, 500.2; attackers and blockers
   are declared once per step, 508.1a, 509.1a), so that state is the boundary
   and its tier-1 differences are reported there rather than by running past
   it; a question asked while an object resolves may recur (two "may" clauses
   of one ability, spec 5.5), so the search goes on past one. Otherwise the differences are reported at the state
   that agreed on `turn` and `pending` with the fewest tier-1 differences, the
   first on a tie, or, when no state agreed, at the state where the search
   stopped: a stretch that never reaches the expected `turn` and `pending` (the
   engine skips a step, or hands priority out in another order) is a divergence
   at L2; a question without a null answer that the record does not answer is
   a divergence at L1 (the record is incomplete, or the engine asks something
   the rules do not), and the run stops there;
4. compares expected and actual fact by fact at levels L2 to L4 at the
   boundary, and the checkpoint hash at L5 when present;
5. records every difference with its path, expected and actual value, marking
   the first difference at each level.

The engine's own skip-ahead facilities (a seat yielding priority until a given
step) are an optimization the adapter may use inside step 3, as long as it
stops at every state the runner needs to see: whenever something goes on the
stack, and at the step and seat of the expected boundary.

During transitions the runner does not abort on a divergence: the game goes on from the engine's
state, so later divergences may be consequences of the first (they are reported
with `first: false`). It also stops when causal setup premises are missing or disagree. During
transitions, it stops only when the engine can no longer follow the record: an action the record has as taken was refused, or an attempt the
record marks illegal was accepted and the adapter could not restore the state.
Where the engine resolves a forced choice itself (a question without a null
answer whose only legal answer the record carries), the adapter answers it with
the recorded answer and checks the engine's result; a prompt the engine asks
that the rules do not mandate is answered by the adapter alone. A batch run
keeps one report per record.

## The divergence report

A report is one JSON document ([schema](schema/divergence-report.schema.json)):

```json
{"mgn_report": "1",
 "record": "sha256:...", "game": "sha256:...",
 "engine": {"name": "karn", "version": "0.1.0+f3c661e3"}, "adapter": {"name": "karn-mgn", "version": "1"},
 "tier": 2,
 "levels": {"L0": "pass", "L1": "pass", "L2": "pass", "L3": "fail", "L4": "inconclusive", "L5": "pass"},
 "transitions": 365, "replayed": 365, "stopped_at": null,
 "divergences": [
   {"after": 134, "level": "L3", "path": "zones.A.library", "expected": ["c12", "c40"], "actual": ["c40", "c12"], "first": true},
   {"after": 135, "level": "L3", "path": "objects.c12.zone", "expected": "A.hand", "actual": "A.library", "first": false}
 ],
 "inconclusive": [{"after": 1, "level": "L4", "path": "objects.*.granted", "reason": "unsupported by the adapter"}],
 "refusals": [{"after": 207, "class": "illegal", "code": "illegal_action", "message": "..."}],
 "usage": {"seconds": 12.4}}
```

- `after` is the transition index (from 1) after which the state was compared;
  for refusals, the transition that was refused. An illegal attempt refused as
  expected produces no entry; one accepted produces a `divergences` entry at L1
  with `path` `attempt`, `expected` `"refused"` and `actual` `"accepted"`.
- `class` is `illegal` (the engine refused an action the record has as taken: a
  fail at L1), `unsupported` (inconclusive from that transition on) or `fault`
  (the engine failed; not a verdict on the record).
- `levels` summarizes: `fail` if any transition failed at that level,
  otherwise `inconclusive` if any was inconclusive, otherwise `pass`;
  `not_applicable` for L4 on a tier-1 record and for L5 without checkpoints,
  and for every engine level when L0 failed.
- `unsupported` lists once the projection paths the adapter declared it does
  not project at all (`unsupported_facts` of its capabilities); they are left
  out of the comparison, and a transition whose record states a non-default
  value for one of them is `inconclusive` there. `usage.notes` carries the
  runner's remarks (an attempt it could not undo, a question the engine asked
  with no answer in the record).
- Paths are the projection's paths of spec section 3 with the record's own ids;
  values are the record's spellings. Nothing in a report refers to an engine's
  handles, prompts or internal state.

## Running it

```sh
uv run scripts/mgn/runner.py tests/fixtures/mgn/bant-ordered-arrivals/record.json --engine target/fast/game-serve --out .cache/mgn-reports
uv run scripts/mgn/runner.py --all --out .cache/mgn-reports
```

The runner starts the Karn adapter itself (`--adapter` names another adapter
command line), prints one summary line per record and writes `report.json`
per record with an `index.json` beside them; `--all` replays every record of
`tests/fixtures/mgn/inventory.json`. The exit status is 1 when any level failed.

The recorder (`scripts/mgn/record.py`, [README](README.md)) is the runner
turned producer: it replays a driver record the same way and keeps the engine's
explicit projection at every boundary it reaches (the `_boundary` hook of the
run) and the cards the adapter reports as new objects (`renewed`, 400.7), then
writes a record whose deltas are the differences between consecutive
projections. The facts the adapter declares it never projects are left at their
defaults and listed in the record's `x-not-projected` member; a fact it could
not project at one boundary, or could not confirm, stops the recording. A
recording from a draft needs L0 to L3 to pass, takes the tier-2 facts a tier-1
draft never stated from the engine (listed as `supplied`), and refuses a tier-2
fact the draft states differently unless the author accepts the engine's value.
No divergence at the start is tolerated. The written record is replayed once
more before it is saved, and
its `review.jsonl` starts with a `replayed` entry naming the facts left at
their defaults.
