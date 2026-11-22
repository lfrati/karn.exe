# MGN adapter protocol

An adapter makes one rules engine play MGN records. The runner (the verifier,
or a producer recording a game) talks to the adapter in JSON messages, one per
line, over the adapter's standard input and output or any byte stream, so that
an engine written in any language can be benchmarked. Messages are defined by
[schema/adapter-protocol.schema.json](schema/adapter-protocol.schema.json).

Every request from the runner carries an integer `id`; every reply carries the
same `id`. An adapter may send interim messages with that `id` before its final
reply (random requests). Unknown members are a `protocol` error.

## Messages

| Runner sends | Adapter replies | Meaning |
| --- | --- | --- |
| `{"id", "hello": {"mgn": "1"}}` | `{"id", "capabilities": {...}}` | what the engine can do (below) |
| `{"id", "setup": {"tier", "pins", "game", "match"?, "cards", "start"}}` | `{"id", "ok": {"state", "causal"?}}` or `{"id", "error"}` | construct the initial position (`position`) or load the decks (`decks`); `state` is the explicit projection at the first boundary; a causal start also requires independently read-back causal premises |
| `{"id", "act": {"n", "seat", "do"}}` | interim `{"id", "random": {"kind", "seat"?, "objects"?, "from"?}}` answered by `{"id", "outcome": <random outcome> or null}`; final `{"id", "ok": {"state", "unsupported"?}}` or `{"id", "refused": {...}}` | play the decision of transition `n`; the reply's `state` is the explicit projection right after it (before any implied answer), `unsupported` the fact paths the adapter cannot project |
| `{"id", "default": {"until"?: {"turn", "step", "occurrence"?, "active", "seat"}}}` | interim random requests as for `act`; final `{"id", "ok": {"state", "asked"?, "random_unused"?}}` or `{"id", "refused": {...}}` | give the pending question its null answer (spec 5.6) and reply with the projection; with `until`, keep giving null answers while the pending question is a priority window and nothing new is on the stack, stopping at the first state at that turn, step, occurrence and seat (`active` names the turn's active seat, which the engine's skip-ahead needs) or earlier; `asked` names the pending question when the adapter did not answer it: one with no null answer (a cleanup discard, 514.1), or one whose null answer is illegal there (an attack or block requirement, 508.1d, 509.1c) |
| `{"id", "attempt": {"n", "seat", "do"}}` | `{"id", "refused": {...}}`, or `{"id", "ok": {"state", "restored"}}` | try an action the record marks illegal; `restored: true` means the adapter undid an accepted attempt and the engine is back at the boundary |
| `{"id", "project": {"tier"}}` | `{"id", "ok": {"state", "unsupported"?}}` | the current explicit projection at that tier |
| `{"id", "end": {}}` | `{"id", "ok": {}}` | release the game |

`capabilities` is `{"mgn": "1", "engine": {"name", "version"}, "adapter": {"name",
"version"}, "tiers": [1, 2], "starts": ["decks", "position"], "kinds": [question
kinds the engine can be asked and answers it accepts], "random": [outcome kinds it
can consume], "formats": ["standard", ...], "unsupported_facts"?: [paths]}`. A
record that needs anything not listed is inconclusive from the first transition
that needs it; the runner still drives the rest where it can.
`unsupported_facts` declares once the projection paths the engine does not
model at all (`game.initiative`, `players.*.designations`, with `*` standing
for one path segment): the adapter omits them from every `state`, the runner
leaves them out of the comparison and lists them in the report's
`unsupported`, and a record that states a non-default value for one of them is
inconclusive at that transition. Facts missing from one state only (a value the
adapter could not read for one object) are listed in that reply's
`unsupported` instead.

For `start.causal`, setup must preserve and read back the declared causes,
including their identity, multiplicity, ordering and lifetime. Copying the
request into the response is not verification. An adapter that cannot construct
a required cause returns `unsupported_start`; an inconsistent readback is a
fault. See [causal-starts.md](causal-starts.md) for the contract and Karn's
bounded import profile. Later causal readback may report surviving imported
causes without claiming a complete export of newly created causes.

## Errors and refusals

`error` (setup) and `refused` (act, attempt) carry `{"class", "code", "message",
"path"?}`. `class` is `illegal` (the engine judges the action illegal at this
boundary), `unsupported` (a capability is missing) or `fault` (the engine or the
adapter failed). `code` is one of:

| Code | Class | Meaning |
| --- | --- | --- |
| `illegal_action` | illegal | the action is not legal in the engine's state |
| `unknown_reference` | illegal | the action names an object the engine does not have where the action needs it |
| `unsupported_start` | unsupported | the start form, or a fact of the position, cannot be constructed |
| `unsupported_card` | unsupported | a card of the card table is not implemented |
| `unsupported_kind` | unsupported | the question kind or answer form is not implemented |
| `unsupported_fact` | unsupported | a fact of the declared tier cannot be projected (also listed under `unsupported` in `ok` replies) |
| `unsupported_random` | unsupported | the random outcome kind cannot be consumed |
| `unsupported_format` | unsupported | the game format |
| `pins_mismatch` | unsupported | the engine's rules or Oracle data differ from the pins; `path` names the card where a face differs |
| `random_missing` | fault | the engine needed a random outcome the runner could not supply |
| `random_mismatch` | fault | the supplied outcome does not fit (a shuffle order that is not the engine's library) |
| `engine_fault` | fault | an internal failure |
| `protocol` | fault | a malformed or unexpected message |
| `timeout` | fault | the engine did not answer in time |

An adapter fails closed: a fact it cannot project is reported under
`unsupported`, never dropped or defaulted; a prompt it cannot map is
`unsupported_kind`, never guessed. Prompts the rules do not mandate (ordering a
single trigger) it answers itself. Under `default` it answers whatever question
is pending with its null answer, which is how the runner moves through the
stretch between two recorded decisions; an engine that makes such a choice
without asking needs nothing from the adapter there. The adapter may use the
engine's own skip-ahead facilities to serve a `default` with `until`, as long
as it reports the state at every representable stop the runner asked for.
A boundary referring to a token that already ceased to exist carries its
explicit `departed` definition when no recorded live history supplies it
(spec 1.8 and 3.1). This is an ordinary representable boundary: `default`
stops at the new stack object, including when a real decision can respond to
it. No exception permits skipping that decision. The adapter authenticates the
intrinsic definition and owner from retained native provenance; an unavailable
or incompatible definition is unsupported. Provisional definitions and numbers
are discarded when the runner continues an implied stretch without recording
that boundary. Every returned state receives full observation validation.

## Randomness

When the engine needs a random result, the adapter asks the runner by key:
`{"kind": "shuffle", "seat": "A"}`, `{"kind": "random_order", "objects": [...]}`,
`{"kind": "random_choice", "from": [...]}`, `{"kind": "coin"}`, `{"kind": "die",
"sides": 6}`, `{"kind": "starting_player"}`. The runner answers with the matching
outcome of the current transition (first unconsumed with that key) or `null`.
For a `shuffle`, the outcome's `order` lists the library as the shuffle left it
(spec section 6): the cards a search set aside (701.24b) and the cards placed
afterwards with `at` are not in it, and the engine must keep the former where
they are and put the latter where the record says. No engine-side seed is ever
used in a replay.

An engine that takes outcomes ahead of a response rather than asking for them
as it goes (Karn's `act` takes all random outcomes of the response up front) is
served the same way: the adapter asks before each engine command for every
outcome of each supported key, repeatedly until the runner returns `null`, and hands the engine what it got,
and returns in the reply's `random_unused` the outcomes the engine did not use,
in the order it received them; the runner puts them back at the head of their
key's queue. An outcome still unconsumed when the next transition begins is a
divergence at L1 (an outcome the engine never asked for); a random event with
no outcome left is a `random_missing` fault, never a seeded draw.

Karn prefetches `random_order` without its set, then the native execution authenticates
the exact physical members at the operation. Missing or malformed native receipts
return a transactional replay-input error, reported as `random_mismatch`; they cannot
satisfy an illegal-action expectation. Unused input indices identify exact occurrences,
including repeated same-seat shuffles and consumption outside a list prefix.

Karn also implements `coin_call` and called `coin` receipts (705.2). The caller supplies
heads or tails before the adapter requests the physical result. A receipt's optional
`caller`, `call` and `winner` must agree with the actual call; they cannot change that
choice or its consequence. Native replay receives only `kind:"coin"`, the player and
`result:"heads"` or `"tails"`, through the same transactional receipt scope as orders.
Missing, extra and mismatched receipts preserve the pending call. Face-only card
instructions, multiple-coin instructions and coin replacements remain unsupported.

## The explicit projection

`state` in replies is the full projection of spec section 3.4 written out as
section 7.3 defines for checkpoints: every zone of every seat, every fact of the
declared tier for every permanent and stack object, `game` present, with the
record's own ids (the adapter keeps the mapping between record ids and engine
objects; tokens, stack objects and emblems take their numbers as spec 1.8
says, when the runner first accepts a state showing them as a boundary, so an
object that appears and goes inside a stretch takes none unless a recorded
boundary needs its departed definition). Optional `departed` rows use the same
strict shape as the record state schema; existing implicit history is reused,
and an empty map is omitted. Paths the adapter
cannot project are listed in `unsupported` and omitted from `state`. A reply to
`act`, `attempt` or `default` may carry `renewed`: the record's cards that are
new objects since the previous reply (400.7), whether they changed zone or left
a zone and came back within the request, which two states alone would not
show; a producer writes them as moved, the runner ignores them.

## The Karn adapter

`scripts/mgn/adapters/karn.py` speaks this protocol for `game-serve`
(`uv run scripts/mgn/adapters/karn.py --engine target/fast/game-serve`; the
runner starts it itself). What it does, and where it bridges the engine:

- **Setup** constructs ordinary priority positions directly through the engine's
  `new`: upkeep, draw, either main phase, beginning/end of combat, or end step.
  Stack, combat and command zone must be empty. Cards, ordered zones and declared
  object/player facts are installed together; setup does not untap, draw, drain
  mana, enter permanents or replay step beginnings. A draw-step position is
  after its draw; the skipped first-turn draw step cannot be constructed (103.8a).
  All supported causes are installed before checking that the board is settled
  for priority (117.5/704.3). The assumptions of spec 2.4 supply land plays,
  control durations and pass count. The first seat took turn 1 and turns
  alternate, so the active seat must agree with turn parity. The supported
  continuous effects, delayed instructions and cast history are described in
  [causal-starts.md](causal-starts.md). For new tests, use
  [scenario setup](scenario-setup.md), then record actions to establish effects
  and choices. Existing combat-declaration and cleanup-discard starts retain a
  bounded path from precombat main using null answers and declared attackers
  (508.1); it is refused once blockers were declared, with
  attackers shown before their declaration, or when a question without a null
  answer is asked on the way; `consecutive_passes` is applied in that main
  phase, so the passes played on the way set the count at the position.
  This path is not a constructor for arbitrary suspended actions.
  A `decks` start keeps the pregame in the adapter
  until the starting player is chosen and the companions revealed (103.1,
  103.2b); the engine's `start` then takes the recorded shuffle outcomes as the
  deck orders (103.3), the starting player, the life total, the companions and,
  for Commander, the commanders (903.6, 903.7), and a mulligan's shuffle
  consumes the outcome recorded for that seat. Every start sets the engine's
  `ask_forced`, so that a question the rules have a player answer is asked even
  with one possible answer (a creature entering attacking with one defender,
  508.4; a block declaration with no legal block, 509.1a). Everything else is
  `unsupported_start` with the path. Cards bind to the engine's physical
  identities zone by zone; a card keeps its id across zone changes (400.7 is
  the engine's business); a companion binds as the engine reveals it, and a
  copy of a spell as it is listed on the stack.
- **Decisions** translate to the engine's moves: ability references by the
  row the engine prints for that paragraph (a dual land's two intrinsic mana
  abilities told apart by the mana added, 305.6); payments as the engine's
  mana plan; the mana a mana ability added is said only where the engine says
  the ability offers a choice (605.3b). A concession is the engine's `concede`
  from any seat (104.3a); the separation into piles is its labelled `select`
  and the opening-hand action its `select` before the game began; a `skip` or
  `spell_destination` effect is matched to the engine's entry by its rule or by
  its source and the ability row of the paragraph named, the source by its
  physical identity, so a resolved spell's effect is found (400.7); the spell a copy
  copies is the one the copying effect targeted, remembered from the question
  about the copy's targets (707.10c), or else the one spell of that name on
  the stack; a trigger instance carries `for` only where two instances from
  the same source and ability would otherwise be indistinguishable (spec 3.3).
  Forced mana-trigger production remains attributed to that trigger, separately
  from the activated land. A real triggered colour choice suspends the unpaid
  announcement; each `choose_mana` answers that question, and the final answer
  completes the authored payment against the actual resulting pool. Evidence
  preserves the initial action and every continuation separately. No colour,
  later activation or spend is inferred on the player's behalf.
  The supported suspended path is a root spell payment whose immediate mana
  activation causes the choice. A later activation with nonempty authored
  payment needs its post-choice cost boundary; unfinished deeper ancestors
  need retained intermediate cost boundaries. Both currently refuse rather
  than guessing those costs. The notation's recursive payment vocabulary
  already describes them; these are adapter/evidence limits.
- **Refusals** of a move or quote take their class from the class the engine
  says (`game-serve`'s `class`, and for a quoted move its proof's
  `error_class`): `illegal` is `illegal_action`, `unreferable`
  (an object, player, label or request the requester cannot refer to) is
  `unknown_reference`, `unsupported` is `unsupported_kind`, and `malformed` or
  `stale` (a request the adapter should not have sent) is a `protocol` fault.
  `replay_input` receipts are `random_mismatch`; a reply with no class, or one
  carrying the engine's `fault`, is `engine_fault`. Only a refusal of class
  `illegal` (the engine's `illegal` or `unreferable`) satisfies an `attempt` or
  ends a stretch's null answers, so neither is passed by an engine that lacks
  the capability or by a malformed or stale request.
- **Stretches** (`default` with `until`) send explicit null answers. They
  stop at every new object on the stack and at every question of
  another kind.
  Whether the target is past is judged by the position of each step among the
  steps begun in its turn, not by the order of 500.1, which an added phase or
  step breaks (500.8, 500.9). A null answer is given by sending it: where the
  engine refuses it as illegal (a block requirement, 509.1c; a cast an effect
  instructs rather than allows, 608.2g) the question has no null answer there
  (spec 5.6) and the reply names it in `asked`; the engine's prompts carry no
  flag for either, and a refused move changes nothing.
- **The projection** reads the privileged `state`, `audit_game` and
  `library_order`: the combat damage step is `first_strike_damage` when it is
  the first of two (510.4; `audit_game` reports `damage_steps` and
  `first_strikers`); a spell's power, toughness and keywords on the stack come
  from the engine's effective characteristics (208.3, 702.34a); `keywords` are
  the engine's held keywords, which include the keyword lines of the object's
  current text for keywords the engine does not track (kicker, plot, equip),
  plus a keyword the engine reads as its ability text ("Solved —", 702.169a);
  a token's definition is the engine's base characteristics, with the rules'
  own wording for a predefined token (111.10); `turn.occurrence` is counted
  from the engine's `began` events, turn by turn; `face` is the face whose
  name the object bears, or the back of a transformed permanent (712); a
  spell's `cast` facts come from the costs the engine retained as it was
  announced (601.2b: kicker, buyback, entwine, replicate, the optional cost
  paid, the gift, a keyword alternative cost, and completed convoke/delve
  substitutions), never from the answer the adapter sent. Restricted mana uses
  authenticated producing abilities and frozen choices, projected with the
  pinned Oracle paragraph and counted-lot normalization of spec 5.8. A copied
  or granted mana producer, spending trigger or retention condition without
  complete native evidence remains unsupported; it is never relabeled as
  unrestricted mana. `result.cause` is `life` or `poison` where the loser's state
  shows it (704.5a, 704.5c) and `result` is unsupported otherwise, since the
  engine says who lost but not why; the object a resolution-time question is
  about is the top of the stack, the one permanent that just entered, or the
  land being played when the question arose within the engine's response to
  the land play (a replacement effect as it enters, 614.1c), and unsupported
  otherwise. A stretch that ran through an
  earlier combat phase of the same turn unobserved leaves that turn's combat
  damage labels unsupported.
- **An accepted attempt** is undone by replaying the adapter's journal (setup,
  decisions, stretches and the outcomes they consumed) into a fresh engine
  process, and the reply says `restored: true`.
- **Declared unsupported** (`unsupported_facts`): the initiative, day and
  night, phasing, flipped cards and player designations are not projected;
  `in_combat` is not exposed apart from the attackers and blockers (506.4).
  This list describes this adapter, not the vocabulary or every native engine
  capability. Missing evidence for a particular object's casting route, grant
  or restricted mana lot is reported at that fact's path. An optional replacement choice is
  `unsupported_kind`. `choose_player` and `choose_number` are translated but no
  two-seat game on this engine reaches them (it asks the player only among
  several opponents, and no admitted card asks a number); `choose_alternative`
  is translated and no admitted Standard card asks it.

An `order_cards` action conveys an owner's actual simultaneous library/graveyard
ordering (401.4, 404.3). An adapter whose engine cannot expose or accept that
boundary must report `unsupported_kind`; it must not read an expected delta or
choose an order from native object handles. Karn's native `order_cards` prompt names
the offered incoming cards and destination; its explicit top-first permutation maps
directly to this action. A generic object-selection prompt is not ordering evidence.
An instructed private rearrangement such as Second Sight uses `arrange`, even
when the library belongs to another player. Karn authenticates the instructed
looker, complete looked-at group and destination from resolution context;
ownership alone does not determine either the chooser or visibility.
