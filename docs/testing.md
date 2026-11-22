# Testing

The tests connect the Comprehensive Rules to what the engine does. There are three kinds:

- [Rules tests](#rules-tests), Rust: one rule or mechanism at a time, citing the rule.
- [Server tests](#engine-tests-through-the-json-server), Python: what a player is asked and told.
- [Integration tests](#integration-tests-mgn-records), MGN records: scenarios and whole games.

**An expectation comes from the rules.** It is derived from the pinned Comprehensive Rules, the
full Oracle text and the rulings (`resources/`), never copied from engine output. When a test and
the engine disagree, the rules decide which of the two is wrong. **A recorded game is never
edited**: a mistaken record is replaced by a new one.

`make check` runs it all, in order: `scripts/rules.py --check`; `cargo test --locked --workspace`
in the test profile, with its debug assertions and overflow checks; a `fast` build of `game-gate`,
`game-serve` and `grammar-gate`; and `scripts/tooling_tests.py`, every module of `scripts/tests/`.

## Rules tests

Rules tests are ordinary Rust `#[test]` functions in the modules of
`crates/game/tests/scenarios/` (one behaviour per file) and `crates/game/tests/engine/` (engine
contracts that no one rule section owns), built into a single test executable, `rules`. Every
`.rs` file in those folders is a module by its file name: a new file needs no `mod` line.

```sh
make test-rules                                                 # the whole executable
cargo test --locked -p game --test rules scenarios::<file>::    # one module
cargo test --locked -p game --test rules r603_                  # a name filter, not "rule 603"
uv run scripts/rules.py 701                                     # rule text and the tests citing it
```

**Citations.** A test is named after its primary rule (`r603_2e_...`) and cites, at each
expectation, the rules that decide it (`crates/game/tests/support/cite.rs`):

- `cite!(["601.2d", "704.5g"]; assert_eq!(a, b))`: an assertion;
- `cite!([…]; ok: action)`: an action that must succeed, yielding its value;
- `cite!([…]; refused("reason"): action)`: an action the engine must refuse for that reason;
- `t.expect().rules([…])`: the builder steps that follow it.

A `case: label;` after the rule list names the row of a table. An expectation that no numbered
rule decides (card text, a ruling, admission, protocol, an implementation contract) carries
`// Uncited (<kind>): <reason>`. `uv run scripts/rules.py --check` verifies the citations
without running the tests: it rejects one that is not a complete rule number of the pinned
rulebook, one in a `#[should_panic]` test or a `macro_rules!` body, and `.is_err()` as an
assertion operand. A refusal test passes only for the intended refusal: it names the reason or
asserts the exact error, and satisfies every other precondition of the action. Debug builds also
compare the game after a refused action with the game before it and panic on a difference.

**The harness** (`crates/game/tests/support/`) is ordinary Rust, with no scenario language;
`crates/game/tests/engine/support_examples.rs` shows it in use.

- *The initial position is exactly the declared position.* `Board` places real cards by Oracle
  name. Defaults: 20 life, player 0 active with priority in the first main phase, untapped
  permanents, empty unspecified zones, no counters. The builder performs no entry, trigger or
  state-based action: when entering is the subject, perform a real entry after setup.
- *A reference is one object.* Label identical cards (`named`). A label does not follow a zone
  change: bind the new object from what happened (`created_since`, `entered_since`).
- *Actions go through the engine.* `mana` fills a pool and taps nothing; casting, activating,
  passing and answering are real commands, and no helper invents a payment or an answer.
- *Expectations are explicit.* `zone_exact` is contents and multiplicity, `contains` membership,
  `zone_ordered` exact sequence. What is not asserted is unchecked.

```rust
let mut t = Board::new(2).named(0, Zone::Hand, "bolt", "Lightning Bolt")
    .named(1, Zone::Battlefield, "bear", "Grizzly Bears").build();
t.mana(0, "{R}").cast(0, "bolt", vec!["bear".into()], mana_pool("R")).unwrap();
t.resolve(); // Fails if a question or unexpected loss occurs.
t.expect().zone_exact(0, Zone::Graveyard, &["Lightning Bolt"])
    .zone_exact(1, Zone::Graveyard, &["Grizzly Bears"]).life(1, 20).stack(&[]).check();
```

## Engine tests through the JSON server

Shared client fixtures live in `scripts/tests/client_support.py`; scenario modules inherit
`ClientTestCase` rather than borrowing methods from other scenario test classes. Tests with
custom process lifetimes can use its individual helpers without inheriting its setup.

Modules of `scripts/tests/` start `game-serve` and drive it through `scripts/play.py`, a
JSON-lines client that sends explicit requests and never chooses an action. They check what a
player is asked and told: the question and its candidates, what each seat may see, the events
delivered, and a refusal's class and cited rules. The same folder holds the tests of the scripts
themselves and the MGN modules (`test_mgn_*`).

```sh
cargo build --locked --profile fast -p game --bin game-serve
uv run scripts/tooling_tests.py test_crew_client    # one module; with no name, all of them
```

## Integration tests: MGN records

Scenarios and whole games are recorded in [MGN](mgn/README.md), a notation that describes
decisions and game states independently of any engine ([specification](mgn/spec.md)). A record
holds an initial position and the decisions taken from it, and states the complete change of the
game state after each decision, so a replay detects any unintended change in what the engine
does. The records are under `tests/fixtures/mgn/`, listed in `tests/fixtures/mgn/inventory.json`.

```sh
cargo build --locked --profile fast -p game --bin game-serve
uv run scripts/mgn/runner.py --active --engine target/fast/game-serve --out .cache/mgn-active
make test-mgn                                        # build the server, run the MGN modules
```

A scenario starts from a declared position ([scenario setup](mgn/scenario-setup.md)). A complete
game starts from the decks and records the pregame and every shuffle. There are three:

| Game | Record | What it exercises |
| --- | --- | --- |
| Vivi | `vivi-decks` | Cast triggers, counters, power-based mana, responses, temporary effects across turns and a terminal winner |
| Torch | `torch-decks-keyword-paragraphs` | Bargain and Role costs, flurry/prowess, token/equipment lifecycle, damage-to-exile replacement and terminal combat |
| Bant | `bant-ordered-arrivals` | Search and later shuffles, cost/dies-trigger ordering, copies and linked exile, first strike/lifelink and terminal loss |

## Engine code coverage

Coverage is opt-in, outside `make check`. `scripts/code_coverage.py` needs `cargo-llvm-cov`
0.8.4, the `llvm-tools-preview` component of Rust 1.94.0, Linux `strace` and a `c++filt` that
demangles Rust. `OUT_DIR` is a new directory outside the checkout and `/tmp`; `TRACE.jsonl` is a
recorded trace of server requests, such as those in `tests/fixtures/performance/`.

```sh
make coverage COVERAGE_ARGS='--out OUT_DIR --native-trace TRACE.jsonl'
```

The Rust tests, every active MGN record and each trace are profiled apart; `report.json` lists
the engine functions, regions and lines each missed. Stable Rust 1.94 has no branch coverage.
