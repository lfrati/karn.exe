# Native payment boundary evidence

One shared boundary preserves MGN 5.8 prior-pool selections, generated mana and
leftovers for casts, activations and effect payments, without card handlers.
The payer's restricted riders apply to the whole announced object, so exact
aggregate selection plus independently checked symbol assignment preserves its
observable semantics; a payer with symbol-specific resource identities would be
the stronger alternative. Remaining vocabulary gaps are listed below.

The privileged native `quote_payment` command accepts the same `session_id`,
`request_id`, `player` and `move` as `act`. It executes an isolated clone, without
mutating the live rules state, RNG, handles, question or delivery buffers. Ordinary
execution does not collect payment audit records. The response contains:

- `move`: the exact quoted native input;
- `complete`, `error`: whether the main announcement reached completed spending;
- `payments`: attempts at actual locked spending boundaries. Each names its
  authored execution `path` (`[]` for the outer payment, `[i]` for plan entry i,
  `[i,j]` for that entry's child j), `prior_pool` before its own mana window,
  `requirement`, `pool`, `selected`, `left`, `player`, `kind`, `source` and
  authenticated child `symbol_indices`. An unfinished attempt has `left: null`;
- `productions`: actual production instructions, with their owning execution
  `path`, native activation index, structured executing ability `invocation`,
  direct/`triggered` distinction, monotonically ordered `sequence`, `source`,
  `player`, `mana` and complete `before`/`after` pools. Repeated activations of
  the same source have distinct paths; multiple instructions retain distinct
  execution sequence receipts;
- `events`: events from the isolated action;
- `prior_pools`, `pools`: full restriction identities at the outer boundaries;
- `symbol_indices`: locked-symbol to printed-symbol indices where the original
  selected cost can be authenticated after X and Phyrexian announcements; null
  when that provenance is missing. Generic reductions and minimum costs remain
  explicit in `requirement`.

Adapter procedure:

```python
# Build the entire native announcement and exact mana production choices first.
# Omit `mana` for the first quote; keep plan, objects, life and all announcements.
quote = send({"cmd": "quote_payment", "session_id": session,
              "request_id": request, "player": player, "move": move})
validate_production(pay, quote)
move["mana"] = select_payment(pay, quote["prior_pools"][actor], quote,
                              record_id=adapter._record_id,
                              restricted_lot=adapter._restricted_lot)
proof = send({"cmd": "quote_payment", "session_id": session,
              "request_id": request, "player": player, "move": move})
# Require proof.complete and preserve it with the original MGN pay.
after = send({"cmd": "act", "session_id": session, "request_id": request,
              "player": player, "move": move, "payment_expect": proof})
```

`actor` is the native acting player from the current question; `player` is its
decision maker. The actual `act` recomputes the proof and rejects a changed or
incomplete proof before applying the live action. If an earlier question or
unsupported execution prevents the main spend, its quote is incomplete; a
completed payment-plan ability alone is not a completed main payment.

`mana.restricted` retains its existing pre-plan indices. The explicit
`mana.restricted_after` selector resolves indices against a fresh isolated quote
of the actual post-plan pool, permitting generated restricted mana without
inventing its compiled restriction. The two selectors are mutually exclusive.
The final native explicit spending must successfully quote: restriction
eligibility remains the engine's responsibility (CR 106.6).

A fresh inverse frame may retain
`payment_selection = {pay: original_mgn_payment, quote: final_native_proof}` in
its AFTER frame. Retain engine/source/input provenance with it. The inverse
checks exact native input, selected totals, captured before/after pools,
activation/cast/production events, reconstructed ability/object/life inputs, and
plain/restricted resource conservation before preserving the authored selection.
A naked prior-pool override is refused. Legacy captures without this richer
input still refuse indistinguishable prior/new same-type mana with leftovers;
conservation alone does not reveal which generation was selected (CR 106.4).

Scope and open vocabulary questions:

- MGN `pay.pool` now selects unrestricted prior mana only. `pool_restricted`
  uses the same semantic lots as `players.mana_restricted`; allocation rows can
  name a restricted lot, while an omitted selector means plain mana. Distinct
  producers and production-time chosen values disambiguate otherwise identical
  wording. No native pool indices enter the record.
- `select_payment` accepts `restricted_lot(native_unit)`, which must authenticate
  complete Oracle/rules spending restrictions, riders, retention, producing
  source/ability and frozen choices. Native units retain immutable production
  evidence; the adapter authenticates the physical printing and projects its
  complete pinned Oracle paragraph. Copied/granted producers and additional
  terms without authenticated wording remain refused. A callback must not infer
  frozen choices from a source's later state. Keep the quote itself unchanged
  for `payment_expect`.
- Mana-ability entries carry recursive `pay`, `x` and `additional.choices`.
  Native `Step.plan`, `Step.x`, `Step.life` retain the same announcements as an
  ordinary activation. Every child runs through the existing `Payment`,
  `Commitment` and procedure driver. A leaf prepares its cost transactionally
  on entry, so an invalid later activation after an exposed question still asks
  its parent for correction. Cost commitments and nested resolutions may suspend;
  completed parts are retained across answers and clones, never replayed.
- The adapter binds each authored selection only when its named scoped receipt
  is reached. It then re-quotes the same input prefix. Source order, initial
  scope pools, exact native selectors, complete scope coverage, production
  invocations and final pools are authenticated before live execution. Neither
  child ancestry nor production ownership is reconstructed from event positions.
  Legacy inverse captures without scoped receipts retain their existing narrower
  flat checks; new native transactions always require scoped receipts.
- A spell announcement whose validated object costs await an owner `order_cards`
  answer has `complete:false` and a native `suspended` receipt. Its mana spend
  and already executed production remain fully authenticated; the receipt binds
  the exact retained root, announced cost/source facts, validated substitutions
  and pending owner/cards/placement. `payment_expect` re-quotes this exact
  boundary before live execution. No cast/activation completion event is invented.
  The record supplies the order as its next action; the existing commitment
  resumes once. Inverse conversion checks the actual captured pending order and
  announcement (excluding only the host-assigned request ID). Unpaid, unscoped
  or non-order interruptions remain incomplete and cannot use this boundary.
  This does not authenticate future mana production from an ability whose own
  unfinished costs have not yet allowed it to resolve. Other unfinished payment
  forms remain refused until they have their own truthful retained evidence.
- `pay.pool` names the pool when that particular mana window begins, including
  mana from earlier siblings. A child's own descendants produce new mana for
  that child. It is not also available to the outer cost after being spent.
  Same-type plain units have no age identity (106.4); prior/generated attribution
  remains feasible after every descendant spend. Ambiguous restricted origins
  refuse. Generated restricted indices resolve separately at each actual scope.
- Child X, Phyrexian life choices and printed-symbol allocations are retained and
  checked. Child symbol indices come from the actual activation cost, after its
  X/life announcements; absent provenance still refuses allocations. Native
  rules/wire tests exercise combined X/life, recursive restrictions and exact
  leftovers; scoped Python tests independently check child restricted allocation.
- Triggered mana instructions retain explicit trigger provenance in native
  receipts. An MGN payment whose completed quote includes such production still
  refuses: `abilities[].adding` cannot attribute another ability's production to
  its enabler. A Vernal Bloom/Forest witness checks that refusal without changing
  live state. A question before the outer locked spend also remains an incomplete
  quote; the adapter does not invent its answer to finish the payment.
- Boolean native `mana` is an informational activation classification, as the
  native wire parser treats only an object as a resource selector. Inversion
  ignores either Boolean value while binding the complete exact move and
  receipt. Other malformed selector types remain refused.
- Complex alternative/additional cost composition can lose printed-symbol
  identity. Allocation is refused when `symbol_indices` is unavailable, rather
  than interpreting final-symbol offsets as printed offsets.
- Per-symbol assignment is checked independently. Native spending currently
  remembers aggregate types and whole-object restricted riders; it has no
  symbol-dependent rider behavior. A future such mechanism needs a richer native
  allocation contract before this equivalence argument can be reused.
- Generated mana whose types can pay generic mana in several different ways
  requires explicit `allocation`; the helper does not choose an auto-payer's
  preferred remaining pool.

Native `mana_answers` receipts identify missing/unoffered choices at the actual
activation path. Corrections use only that invocation's authored output; they
never strip another source's answer or invent a triggered answer. An exact
source/AbilityKey reservation prevents illegal self-reentry until its mana
resolution completes (605.3c), while sibling reuse remains legal.
