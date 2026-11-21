//! Waterbend (701.67).
//!
//! 701.67a: "“Waterbend [cost]” means “Pay [cost]. For each generic mana in that cost, you may
//! tap an untapped artifact or creature you control rather than pay that mana.”" 701.67b: only
//! the generic mana of the waterbend cost may be paid that way when it is part of a larger total
//! cost. The cards' rulings (2025-10-02): a creature tapped for waterbend need not have been
//! controlled since the turn began (302.6 restricts only {T}/{Q} abilities); one tapped for mana
//! while paying can't be tapped again for waterbend; "Even if the cost of the spell or ability
//! that waterbend is part of increases or decreases, you still may only tap artifacts or
//! creatures to pay for costs up to the amount of the generic mana in the waterbend cost."
use crate::support::cite;
use game::legal::Step;
use game::state::{Answer, Choice, Decision};
use game::{CardDef, Game, Move, ObjId, Target, Zone};
use ir::Part;

const NAMES: [&str; 9] = ["Island", "Geyser Leaper", "Benevolent River Spirit", "Grizzly Bears", "Waterbending Lesson",
    "The Unagi of Kyoshi Island", "Shock", "Llanowar Elves", "Thalia, Guardian of Thraben"];
const ISLAND: usize = 0;
const LEAPER: usize = 1;
const SPIRIT: usize = 2;
const BEARS: usize = 3;
const LESSON: usize = 4;
const UNAGI: usize = 5;
const SHOCK: usize = 6;
const ELVES: usize = 7;
const THALIA: usize = 8;
const SPRING: usize = 9;
const SURGE: usize = 10;
const SPRITE: usize = 11;
const REDUCER: usize = 12;

fn game() -> Game {
    let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let catalog = CardDef::catalog(&faces);
    let mut defs: Vec<_> = NAMES.map(|n| CardDef::from_faces(&catalog[n]).unwrap()).to_vec();
    defs.push(CardDef::new("Spring", "{U}", "Artifact", "Waterbend {X}: You gain X life.", None).unwrap());
    defs.push(CardDef::new("Surge", "{2}{U}", "Sorcery", "As an additional cost to cast this spell, waterbend {2}.\nDraw a card.", None).unwrap());
    defs.push(CardDef::new("Sprite", "{0}", "Creature — Faerie", "", Some((1, 1))).unwrap());
    defs.push(CardDef::new("Reducer", "{0}", "Artifact", "Creature spells you cast cost {1} less to cast.", None).unwrap());
    for d in &defs { game::play::check(d).unwrap_or_else(|e| panic!("{}: {}", d.name(), e.0)); }
    let mut g = Game::new(defs, 2, 20);
    for p in 0..2 { for _ in 0..10 { g.put(p, ISLAND, Zone::Library); } }
    while g.part != Part::PrecombatMain { g.act(g.priority, Move::Pass).unwrap(); }
    g
}
fn empty(g: &mut Game, p: usize) { g.players[p].pool.colors = [0; 5]; g.players[p].pool.colorless = 0; }
fn resolve(g: &mut Game) { while !g.stack.is_empty() && g.decision.is_none() { g.act(g.priority, Move::Pass).unwrap(); } }

/// A creature cast this turn: not under its controller's control since the turn began.
fn cast_sprite(g: &mut Game) -> ObjId {
    let card = g.put(0, SPRITE, Zone::Hand);
    g.act(0, Move::Cast { card, targets: vec![] }).unwrap();
    resolve(g);
    let sprite = *g.battlefield.last().unwrap();
    assert_eq!(g.obj(sprite).chars.name, "Sprite");
    sprite
}
/// How many each listed cast of this card taps for its waterbend cost.

fn pay(plan: Vec<Step>, tapped: Vec<ObjId>, spending: &str) -> Move { Move::Choose(Answer::Pay { plan, with: Some(crate::support::mana_pool(spending)), tapped }) }
fn elves_step(elves: ObjId) -> Step {  Step { plan: vec![], x: None, life: vec![], with: None, cost_branches: None, chose: vec![], source: elves, ability: 0, paying: vec![], adding: vec![] } }

// Rules: 701.67a, 302.6
// Given: Geyser Leaper ("Waterbend {4}: ...") and a Sprite, both entered this turn; two Bears
// controlled since the turn began; no mana.
// When: the Leaper's ability is activated tapping all four.
// Expect: activated; all four tapped.
// Why: waterbend taps as a cost, not a {T} ability of the tapped creature; 302.6 doesn't apply
// (ruling 0d47dccdb89c3cae).
#[test]
fn r701_67a_summoning_sick_creatures_and_the_source_itself_may_be_tapped() {
    let mut g = game();
    empty(&mut g, 0);
    g.players[0].pool.colors[1] = 5;
    let card = g.put(0, LEAPER, Zone::Hand);
    g.act(0, crate::support::paid("UUUUU", Move::Cast { card, targets: vec![] })).unwrap();
    resolve(&mut g);
    let leaper = *g.battlefield.last().unwrap();
    let sprite = cast_sprite(&mut g);
    assert!(g.summoning_sick(leaper) && g.summoning_sick(sprite), "the setup: both entered this turn");
    let bears: Vec<_> = (0..2).map(|_| g.put(0, BEARS, Zone::Battlefield)).collect();
    empty(&mut g, 0);
    let index = 1 /* printed Flying, then Waterbend activation */;
    let tapped = vec![leaper, sprite, bears[0], bears[1]];
    cite!(["701.67a", "302.6"]; ok: g.act(0, Move::Activate { source: leaper, ability: index, targets: vec![], paying: vec![tapped.clone()] }));
    cite!(["701.67a"]; assert!(tapped.iter().all(|o| g.obj(*o).tapped)));
    assert_eq!(g.stack.len(), 1);
}

// Rules: 701.67a, 601.2g, 601.2h
// Given: Geyser Leaper, Llanowar Elves ("{T}: Add {G}") and two Bears; no mana.
// When: the Elves' mana ability is activated for the payment and the Elves are also listed
// among the three selected for the waterbend cost.
// Expect: refused, nothing tapped; Elves for mana with Leaper and both Bears tapped is fine.
// Why: mana abilities are activated before costs are paid (601.2g-h): the Elves are tapped by
// then and can't be tapped for waterbend (ruling ae89793f17b93ed2).
#[test]
fn r701_67a_a_creature_tapped_for_mana_cant_also_be_tapped_for_waterbend() {
    let mut g = game();
    empty(&mut g, 0);
    let leaper = g.put(0, LEAPER, Zone::Battlefield);
    let elves = g.put(0, ELVES, Zone::Battlefield);
    let bears: Vec<_> = (0..2).map(|_| g.put(0, BEARS, Zone::Battlefield)).collect();
    let index = 1 /* printed Flying, then Waterbend activation */;
    let with = |tapped: Vec<ObjId>| crate::support::paid("G", Move::WithMana(vec![elves_step(elves)], Box::new(Move::Activate { source: leaper, ability: index, targets: vec![], paying: vec![tapped] })));
    cite!(["701.67a", "601.2g"]; refused("Llanowar Elves is not what that cost asks for or cannot pay it"): unchanged(&mut g, |g| g.act(0, with(vec![leaper, elves, bears[0]]))));
    cite!(["701.67a"]; assert!(![leaper, elves, bears[0], bears[1]].iter().any(|o| g.obj(*o).tapped) && g.stack.is_empty(), "nothing of the refused activation"));
    cite!(["701.67a", "601.2g"]; ok: g.act(0, with(vec![leaper, bears[0], bears[1]])));
    assert!(g.obj(elves).tapped && g.players[0].pool.total() == 0);
}

// Rules: 701.67a, 107.3a
// Given: "Waterbend {X}: You gain X life." and an untapped Bears; no mana.
// When: activated with X = 0 tapping nothing, and with X = 0 tapping the Bears.
// Expect: the first is activated (a {0} cost), the second refused.
// Why: waterbend {0} has no generic mana for a tapped creature to pay instead of.
#[test]
fn r701_67a_waterbend_x_with_x_zero() {
    let mut g = game();
    empty(&mut g, 0);
    let spring = g.put(0, SPRING, Zone::Battlefield);
    let bears = g.put(0, BEARS, Zone::Battlefield);
    let with_x = |x, tapped: Vec<ObjId>| Move::WithX(x, Box::new(Move::Activate { source: spring, ability: 0, targets: vec![], paying: vec![tapped] }));
    cite!(["701.67a", "107.3a"]; refused("a waterbend cost taps at most one artifact or creature for each generic mana in it, 0: 1 chosen"): unchanged(&mut g, |g| g.act(0, with_x(0, vec![bears]))));
    cite!(["701.67a"]; assert!(!g.obj(bears).tapped));
    cite!(["701.67a", "107.3a"]; ok: g.act(0, with_x(0, vec![])));
    resolve(&mut g);
    cite!(["701.67a"]; assert_eq!((g.players[0].life, g.obj(bears).tapped), (20, false)));
}

// Rules: 701.67b, 601.2f
// Given: Thalia ("Noncreature spells cost {1} more to cast.") and Surge {2}{U} with "waterbend
// {2}" as an additional cost: total {5}{U}, two of it the waterbend cost's; three Bears.
// When: cast tapping three Bears with {2}{U} in the pool; then two Bears with {3}{U}.
// Expect: the first refused, the second cast.
// Why: only the waterbend cost's {2} may be paid by tapping, even when the total cost increases
// (ruling 550ebdd0386d9471).
#[test]
fn r701_67b_a_cost_increase_adds_no_taps() {
    let mut g = game();
    empty(&mut g, 0);
    g.put(0, THALIA, Zone::Battlefield);
    let surge = g.put(0, SURGE, Zone::Hand);
    let bears: Vec<_> = (0..3).map(|_| g.put(0, BEARS, Zone::Battlefield)).collect();
    let cast = |g: &mut Game, tapped: &[ObjId], spending: &str| g.act(0, crate::support::paid(spending, Move::WithCosts(vec![tapped.to_vec()], Box::new(Move::Cast { card: surge, targets: vec![] }))));
    g.players[0].pool.colors[1] = 1;
    g.players[0].pool.colorless = 2;
    cite!(["701.67b"]; refused("a waterbend cost taps at most one artifact or creature for each generic mana in it, 2: 3 chosen"): unchanged(&mut g, |g| cast(g, &bears, "CCU")));
    cite!(["701.67b"]; assert!(bears.iter().all(|b| !g.obj(*b).tapped) && g.stack.is_empty()));
    g.players[0].pool.colorless = 3;
    cite!(["701.67b", "601.2f"]; ok: cast(&mut g, &bears[..2], "CCCU"));
    assert_eq!(g.players[0].pool.total(), 0, "{{3}}{{U}} of mana paid the rest");
}

// Rules: 701.67b, 601.2f
// Given: Benevolent River Spirit {U}{U} with waterbend {5} as an additional cost, and "Creature
// spells you cast cost {1} less to cast.": the total cost is {4}{U}{U}; six Bears; {U}{U}.
// When: cast tapping five Bears; then tapping four.
// Expect: five refused (the total has four generic mana), four cast.
// Why: each tapped creature pays one generic mana of the total cost rather than paying it; with
// four, no generic mana is left for a fifth (701.67a-b, ruling 464c52f16806b050).
#[test]
fn r701_67b_a_cost_reduction_leaves_fewer_to_tap() {
    let mut g = game();
    empty(&mut g, 0);
    g.put(0, REDUCER, Zone::Battlefield);
    let spirit = g.put(0, SPIRIT, Zone::Hand);
    let bears: Vec<_> = (0..6).map(|_| g.put(0, BEARS, Zone::Battlefield)).collect();
    g.players[0].pool.colors[1] = 2;
    let cast = |g: &mut Game, tapped: &[ObjId]| g.act(0, crate::support::paid("UU", Move::WithCosts(vec![tapped.to_vec()], Box::new(Move::Cast { card: spirit, targets: vec![] }))));
    cite!(["701.67b"]; refused("the supplied mana does not pay that cost, all of it and no more"): unchanged(&mut g, |g| cast(g, &bears[..5])));
    cite!(["701.67b"]; assert!(bears.iter().all(|b| !g.obj(*b).tapped) && g.stack.is_empty()));
    cite!(["701.67b", "601.2f"]; ok: cast(&mut g, &bears[..4]));
    cite!(["701.67b"]; assert_eq!(bears.iter().filter(|b| g.obj(**b).tapped).count(), 4));
    assert_eq!(g.players[0].pool.total(), 0);
}

fn lesson(g: &mut Game) {
    // Waterbending Lesson {3}{U}: "Draw three cards. Then discard a card unless you waterbend {2}."
    let lesson = g.put(0, LESSON, Zone::Hand);
    empty(g, 0);
    g.players[0].pool.colors[1] = 1;
    g.players[0].pool.colorless = 3;
    g.act(0, crate::support::paid("CCCU", Move::Cast { card: lesson, targets: vec![] })).unwrap();
    g.act(0, Move::Pass).unwrap();
    g.act(1, Move::Pass).unwrap();
    assert!(matches!(g.decision, Some(Decision::Choose { player: 0, choice: Choice::Pay { waterbend: Some(_), .. } })), "{:?}", g.decision);
}

// Rules: 701.67a, 118.12
// Given: Waterbending Lesson resolving ("unless you waterbend {2}"); the caster has one untapped
// Bears, a tapped Bears, an untapped Island and the opponent an untapped Bears; no mana.
// When: answered tapping their own Bears and each of the others in turn.
// Expect: each refused and the question still asked; their own Bears and the Island's mana pay.
// Why: only untapped artifacts and creatures the payer controls may be tapped; a land is
// tapped for its mana, not as one of them.
#[test]
fn r701_67a_unless_you_waterbend_only_your_untapped_artifacts_and_creatures() {
    let mut g = game();
    let mine = g.put(0, BEARS, Zone::Battlefield);
    let tapped = g.put(0, BEARS, Zone::Battlefield);
    g.tap(tapped);
    let island = g.put(0, ISLAND, Zone::Battlefield);
    let theirs = g.put(1, BEARS, Zone::Battlefield);
    lesson(&mut g);
    for (case, other) in [("tapped", tapped), ("an opponent's", theirs), ("a land", island)] {
        cite!(["701.67a"]; case: case; refused("is not what that cost asks for or cannot pay it"): unchanged(&mut g, |g| g.act(0, pay(vec![], vec![mine, other], ""))));
        cite!(["701.67a"]; case: case; assert!(!g.obj(mine).tapped && !g.obj(theirs).tapped && !g.obj(island).tapped));
        assert!(matches!(g.decision, Some(Decision::Choose { player: 0, choice: Choice::Pay { .. } })), "{case}: still asked");
    }
    let island_step = Step { plan: vec![], x: None, life: vec![], with: None, cost_branches: None, chose: vec![], source: island, ability: 0, paying: vec![], adding: vec![] };
    cite!(["701.67a", "118.12"]; ok: g.act(0, pay(vec![island_step], vec![mine], "U")));
    cite!(["701.67a"]; assert!(g.obj(mine).tapped && g.obj(island).tapped && g.decision.is_none(), "paid: no discard"));
}

// Rules: 701.67a, 118.12, 601.2g
// Given: Waterbending Lesson resolving; the caster has Llanowar Elves and one Bears, no mana.
// When: answered with the Elves' mana ability in the plan and the Elves also tapped for the
// waterbend cost (one selected creature plus the generated G).
// Expect: refused; the Elves for {G} and the Bears tapped pays.
// Why: the Elves are tapped for mana before the cost is paid (ruling ae89793f17b93ed2).
#[test]
fn r701_67a_unless_you_waterbend_a_mana_creature_pays_once() {
    let mut g = game();
    let elves = g.put(0, ELVES, Zone::Battlefield);
    let bears = g.put(0, BEARS, Zone::Battlefield);
    lesson(&mut g);
    cite!(["701.67a", "601.2g"]; refused("Llanowar Elves is not what that cost asks for or cannot pay it"): unchanged(&mut g, |g| g.act(0, pay(vec![elves_step(elves)], vec![elves], "G"))));
    cite!(["701.67a"]; assert!(!g.obj(elves).tapped && !g.obj(bears).tapped));
    cite!(["701.67a", "118.12"]; ok: g.act(0, pay(vec![elves_step(elves)], vec![bears], "G")));
    cite!(["701.67a"]; assert!(g.obj(elves).tapped && g.obj(bears).tapped && g.decision.is_none()));
}

// Rules: 702.21a, 701.67a, 118.12
// Given: The Unagi of Kyoshi Island ("Ward—Waterbend {4}") controlled by player 0, who also
// controls two Bears; player 1 targets it with Shock and has three Bears and {R} left.
// When: player 1 answers the ward payment tapping player 0's Bears, then their own.
// Expect: player 0's Bears refused; player 1's three Bears and {R} pay; Shock resolves.
// Why: the player who pays the waterbend cost taps artifacts and creatures they control.
#[test]
fn r702_21a_ward_waterbend_taps_the_payers_creatures_not_the_wards() {
    let mut g = game();
    let unagi = g.put(0, UNAGI, Zone::Battlefield);
    let shock = g.put(1, SHOCK, Zone::Hand);
    let theirs: Vec<ObjId> = (0..3).map(|_| g.put(1, BEARS, Zone::Battlefield)).collect();
    let mine: Vec<ObjId> = (0..2).map(|_| g.put(0, BEARS, Zone::Battlefield)).collect();
    g.act(0, Move::Pass).unwrap();
    g.players[1].pool.colors[3] = 2;
    g.act(1, crate::support::paid("R", Move::Cast { card: shock, targets: vec![vec![Target::Object(unagi)]] })).unwrap();
    while !matches!(g.decision, Some(Decision::Choose { choice: Choice::Pay { .. }, .. })) { g.act(g.priority, Move::Pass).unwrap(); }
    cite!(["702.21a", "118.12"]; assert!(matches!(g.decision, Some(Decision::Choose { player: 1, .. }))));
    cite!(["701.67a", "702.21a"]; refused("Grizzly Bears is not what that cost asks for or cannot pay it"): unchanged(&mut g, |g| g.act(1, pay(vec![], vec![mine[0], mine[1], theirs[0]], "R"))));
    cite!(["701.67a", "702.21a"]; refused("that decision is another player's"): unchanged(&mut g, |g| g.act(0, pay(vec![], theirs.clone(), "R"))));
    assert!(mine.iter().chain(&theirs).all(|b| !g.obj(*b).tapped));
    cite!(["701.67a", "702.21a"]; ok: g.act(1, pay(vec![], theirs.clone(), "R")));
    resolve(&mut g);
    cite!(["701.67a"]; assert!(theirs.iter().all(|b| g.obj(*b).tapped) && mine.iter().all(|b| !g.obj(*b).tapped)));
    cite!(["702.21a"]; assert_eq!(g.obj(unagi).damage, 2));
    assert_eq!(g.players[1].pool.total(), 0);
}

#[track_caller]
fn unchanged(g: &mut Game, action: impl FnOnce(&mut Game) -> Result<(), String>) -> Result<(), String> {
    let before = format!("{g:?}");
    let result = action(g);
    assert_eq!(format!("{g:?}"), before);
    result
}
