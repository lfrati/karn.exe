//! Harmonize (702.180) and warp (702.185).
use crate::support::cite;
use game::announcement::{CastCost, CastRoute};
use game::state::Decision;
use game::{CardDef, Game, Move, ObjId, Target, Zone};
use ir::Part;

const WARPER: usize = 0;
const CHORUS: usize = 1;
const ELF: usize = 2;
const STEAL: usize = 3;
const COUNTER: usize = 4;
const SHOCK: usize = 5;
const FOREST: usize = 6;

fn game() -> Game {
    let c = |name, cost, types, text, pt| CardDef::new(name, cost, types, text, pt).expect(name);
    let defs = vec![
        c("Warper", "{3}{G}", "Creature — Beast", "Warp {G}", Some((2, 2))),
        c("Chorus", "{2}{U}", "Sorcery", "You gain 1 life.\nHarmonize {4}{U}", None),
        c("Elf", "{0}", "Creature — Elf", "{T}: Add {U}.", Some((3, 1))),
        c("Steal", "{0}", "Instant", "Gain control of target creature until end of turn.", None),
        c("Counter", "{0}", "Instant", "Counter target spell.", None),
        c("Shock", "{0}", "Instant", "Shock deals 2 damage to target creature.", None),
        c("Forest", "", "Basic Land — Forest", "", None),
    ];
    for d in &defs { game::play::check(d).unwrap_or_else(|e| panic!("{}: {}", d.name(), e.0)); }
    let mut g = Game::new(defs, 2, 20);
    for p in 0..2 { for _ in 0..10 { g.put(p, FOREST, Zone::Library); } }
    while g.part != Part::PrecombatMain { g.act(g.priority, Move::Pass).unwrap(); }
    g
}

fn warp(g: &mut Game) -> ObjId {
    let card = g.put(0, WARPER, Zone::Hand);
    g.players[0].pool.colors[4] = 1;
    let row = 0; // The fixture prints Warp as its first ability.
    g.act(0, crate::support::paid("G",Move::CastFor(CastCost::Alternative(row), Box::new(Move::Cast { card, targets: vec![] })))).unwrap();
    card
}
fn step(g: &mut Game) {
    match g.decision.clone() {
        Some(Decision::Attackers { player }) => g.act(player, Move::Attack(vec![])).unwrap(),
        Some(Decision::Blockers { player }) => g.act(player, Move::Block(vec![])).unwrap(),
        Some(d) => panic!("unexpected decision {d:?}"),
        None => g.act(g.priority, Move::Pass).unwrap(),
    }
}
fn settle(g: &mut Game) {
    for _ in 0..20 { if g.decision.is_none() && g.stack.is_empty() { return; } step(g); }
    panic!("did not settle");
}
/// On to the next turn's precombat main phase of `player`.
fn main_of(g: &mut Game, player: usize) {
    let turn = g.turn;
    for _ in 0..400 {
        if g.turn != turn && g.active == player && g.part == Part::PrecombatMain && g.decision.is_none() && g.stack.is_empty() { return; }
        step(g);
    }
    panic!("no main phase");
}
fn in_exile(g: &Game, def: usize) -> Option<ObjId> { g.exile.iter().copied().find(|o| g.obj(*o).card == Some(def)) }
/// `player` casts an instant from their hand at `targets`, with priority passed to them.
fn respond(g: &mut Game, player: usize, def: usize, targets: Vec<Vec<Target>>) {
    let card = g.put(player, def, Zone::Hand);
    while g.priority != player { g.act(g.priority, Move::Pass).unwrap(); }
    g.act(player, Move::Cast { card, targets }).unwrap();
}

/// 702.185a: the delayed trigger exiles "the permanent this spell becomes", whoever controls
/// it; then "its owner" may cast it, not the player who controlled it.
#[test]
fn review_warp_exiles_a_stolen_permanent_and_only_its_owner_casts_it() {
    let mut g = game();
    warp(&mut g);
    settle(&mut g);
    let beast = *g.battlefield.last().unwrap();
    respond(&mut g, 1, STEAL, vec![vec![Target::Object(beast)]]);
    settle(&mut g);
    assert_eq!(g.controller(beast), 1);
    main_of(&mut g, 1);
    let exiled = in_exile(&g, WARPER).expect("exiled though another player controlled it");
    g.players[1].pool.colors[4] = 4;
    cite!(["702.185a", "601.3"]; refused("an effect lets another player cast that card, not you (601.3)"): g.act(1, Move::Cast { card: exiled, targets: vec![] }));
    main_of(&mut g, 0);
    g.players[0].pool.colors[4] = 4;
    cite!(["702.185a"]; ok: g.act(0, crate::support::paid("GGGG",Move::Cast { card: exiled, targets: vec![] })));
}

/// A countered warp spell never becomes a permanent: no exile, no permission (702.185a).
#[test]
fn review_a_countered_warp_spell_stays_in_the_graveyard() {
    let mut g = game();
    let card = warp(&mut g);
    let spell = *g.stack.last().unwrap();
    respond(&mut g, 1, COUNTER, vec![vec![Target::Object(spell)]]);
    settle(&mut g);
    let _ = card;
    assert!(g.players[0].graveyard.iter().any(|o| g.obj(*o).card == Some(WARPER)));
    main_of(&mut g, 1);
    cite!(["702.185a"]; assert!(g.players[0].graveyard.iter().any(|o| g.obj(*o).card == Some(WARPER)), "not exiled from the graveyard"));
    assert!(in_exile(&g, WARPER).is_none());
}

/// A warped creature that dies first is a new object in the graveyard and stays there (400.7).
#[test]
fn review_a_warped_creature_that_died_is_not_exiled() {
    let mut g = game();
    warp(&mut g);
    settle(&mut g);
    let beast = *g.battlefield.last().unwrap();
    respond(&mut g, 1, SHOCK, vec![vec![Target::Object(beast)]]);
    settle(&mut g);
    assert!(!g.battlefield.contains(&beast));
    main_of(&mut g, 1);
    assert!(g.players[0].graveyard.iter().any(|o| g.obj(*o).card == Some(WARPER)));
    cite!(["400.7"]; assert!(in_exile(&g, WARPER).is_none()));
}

fn harmonize(card: ObjId, tapped: Option<ObjId>) -> Move {
    Move::CastVia(CastRoute::Harmonize { tapped }, Box::new(Move::Cast { card, targets: vec![] }))
}

/// Tapping for harmonize is not a {T} cost: a creature that came under your control this turn
/// can be tapped (302.6 is about {T} and {Q} costs of its own abilities).
#[test]
fn review_harmonize_taps_a_summoning_sick_creature() {
    let mut g = game();
    let elf = g.put(0, ELF, Zone::Hand);
    g.act(0, Move::Cast { card: elf, targets: vec![] }).unwrap();
    settle(&mut g);
    let elf = *g.battlefield.last().unwrap();
    assert!(g.summoning_sick(elf));
    let card = g.put(0, CHORUS, Zone::Graveyard);
    g.players[0].pool.colors[1] = 1;
    g.players[0].pool.colorless = 1;
    cite!(["702.180a"]; ok: g.act(0, crate::support::paid("CU",harmonize(card, Some(elf)))));
    cite!(["702.180b"]; assert!(g.obj(elf).tapped));
}

/// A creature already tapped for mana is not an untapped creature to tap (702.180a), and a
/// refused cast leaves nothing tapped.
#[test]
fn review_harmonize_needs_an_untapped_creature_and_a_refusal_taps_nothing() {
    let mut g = game();
    let elf = g.put(0, ELF, Zone::Battlefield);
    let card = g.put(0, CHORUS, Zone::Graveyard);
    // too little mana: refused, untapped
    g.players[0].pool.colors[1] = 1;
    cite!(["118.3a"]; refused("the supplied mana does not pay that cost, all of it and no more (601.2h, 118.3a)"): g.act(0, crate::support::paid("U",harmonize(card, Some(elf)))));
    assert!(!g.obj(elf).tapped);
    g.players[0].pool.colors[1] = 0;
    g.act(0, Move::Activate { source: elf, ability: 0, targets: vec![], paying: vec![] }).unwrap();
    g.players[0].pool.colorless = 1;
    cite!(["702.180a"]; refused("the creature tapped for harmonize is an untapped creature you control (702.180b)"): g.act(0, harmonize(card, Some(elf))));
}

/// One creature cannot be tapped both for harmonize and to convoke the same spell: once tapped
/// for one, it is not untapped for the other (702.180a, 702.51a).
#[test]
fn review_harmonize_and_convoke_cannot_tap_one_creature_twice() {
    let mut g = game();
    let def = CardDef::new("Choir", "{2}{U}", "Sorcery", "Convoke\nYou gain 1 life.\nHarmonize {4}{U}", None).unwrap();
    game::play::check(&def).unwrap();
    let index = g.defs.len();
    std::sync::Arc::make_mut(&mut g.defs).push(def);
    let elf = g.put(0, ELF, Zone::Battlefield);
    let card = g.put(0, index, Zone::Graveyard);
    // {4}{U} less 3 for the elf's power is {1}{U}; convoking the elf again for the {1}
    g.players[0].pool.colors[1] = 1;
    let both = Move::WithConvoke(vec![None], Box::new(Move::WithCosts(vec![vec![elf]],
        Box::new(harmonize(card, Some(elf))))));
    let both_inner = Move::CastVia(CastRoute::Harmonize { tapped: Some(elf) }, Box::new(Move::WithConvoke(vec![None],
        Box::new(Move::WithCosts(vec![vec![elf]], Box::new(Move::Cast { card, targets: vec![] }))))));
    let before = format!("{g:?}");
    let a = g.act(0, both);
    let b = g.act(0, both_inner);
    cite!(["118.10"]; refused("Elf pays for one thing (118.10)"): a);
    cite!(["118.10"]; refused("Elf pays for one thing (118.10)"): b);
    assert_eq!(format!("{g:?}"), before);
    // a second creature convokes the {1}: one of the two forms is accepted
    let other = g.put(0, ELF, Zone::Battlefield);
    let outer = Move::WithConvoke(vec![None], Box::new(Move::WithCosts(vec![vec![other]], Box::new(harmonize(card, Some(elf))))));
    let inner = Move::CastVia(CastRoute::Harmonize { tapped: Some(elf) }, Box::new(Move::WithConvoke(vec![None],
        Box::new(Move::WithCosts(vec![vec![other]], Box::new(Move::Cast { card, targets: vec![] }))))));
    let (x, y) = (g.clone().act(0, crate::support::paid("U",outer)), g.clone().act(0, crate::support::paid("U",inner)));
    cite!(["702.51a", "702.180a"]; assert!(x.is_ok() || y.is_ok(), "{x:?} {y:?}"));
}
