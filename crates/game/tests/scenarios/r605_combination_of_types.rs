//! "Add X mana in any combination of {U} and/or {R}" (106.1a, 605.3b): each mana one of the
//! named types. A few combinations are one choice; many, the type of each mana in turn.
//! "Activate only during your turn and only once each turn" (602.5). Vivi Ornitier.
use crate::support::{cite,paid};
use game::{CardDef, Event, Game, Move, ObjId, Zone};
use ir::Color;
use ir::Part;
use ir::mana::ManaSymbol;

const VIVI: usize = 0;
const OPT: usize = 1;
const BEAR: usize = 2;
const ISLAND: usize = 3;
const OGRE: usize = 4;

fn game() -> Game {
    let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let real = |name: &str| CardDef::from_faces(&CardDef::catalog(&faces)[name]).unwrap();
    let defs = vec![real("Vivi Ornitier"), real("Opt"), CardDef::new("Bear", "{0}", "Creature — Bear", "", Some((2, 2))).unwrap(), real("Island"),
        CardDef::new("Ogre", "{1}{R}", "Creature — Ogre", "", Some((3, 2))).unwrap()];
    for d in &defs { game::play::check(d).unwrap_or_else(|e| panic!("{}: {}", d.name(), e.0)); }
    let mut g = Game::new(defs, 2, 20);
    for p in 0..2 { for _ in 0..10 { g.put(p, ISLAND, Zone::Library); } }
    while g.part != Part::PrecombatMain { g.act(g.priority, Move::Pass).unwrap(); }
    g
}
fn u() -> ManaSymbol { ManaSymbol::Color(Color::Blue) }
fn r() -> ManaSymbol { ManaSymbol::Color(Color::Red) }
fn add(g: &mut Game, vivi: ObjId, adding: Vec<Vec<ManaSymbol>>) -> Result<(), String> {
    g.act(0, Move::Adding(adding, Box::new(Move::Activate { source: vivi, ability: 0, targets: vec![], paying: vec![] })))
}
fn added(g: &Game, source: ObjId) -> Vec<Vec<ManaSymbol>> {
    g.log.iter().filter_map(|e| match e { Event::ManaAdded { source: s, mana, .. } if *s == source => Some(mana.clone()), _ => None }).collect()
}
fn counters(g: &mut Game, o: ObjId, n: u32) { g.put_counters(o, "+1/+1", n); }

#[test]
fn r106_1a_two_mana_is_one_choice_among_blue_and_red_combinations() {
    let mut g = game();
    let vivi = g.put(0, VIVI, Zone::Battlefield);
    counters(&mut g, vivi, 2);
    cite!(["608.2d"]; refused("that ability doesn't offer the mana said for it: say one of what it offers (608.2d)"): add(&mut g, vivi, vec![vec![ManaSymbol::Color(Color::Green), u()]]));
    for mana in [vec![u(), u()], vec![u(), r()], vec![r(), r()]] {
        let mut attempt = g.clone();
        cite!(["106.1a", "608.2d"]; case: format!("{mana:?}"); ok: add(&mut attempt, vivi, vec![mana]));
    }
    add(&mut g, vivi, vec![vec![u(), r()]]).unwrap();
    assert_eq!(added(&g, vivi), [vec![u(), r()]]);
    cite!(["106.4"]; assert_eq!((g.players[0].pool.colors[1], g.players[0].pool.colors[3]), (1, 1)));
}

#[test]
fn r605_3b_with_many_combinations_each_mana_is_blue_or_red_in_turn() {
    let mut g = game();
    let vivi = g.put(0, VIVI, Zone::Battlefield);
    // power 20: 21 combinations, more than are listed
    counters(&mut g, vivi, 20);
    let mut adding = vec![vec![u()]; 7];
    adding.extend(vec![vec![r()]; 13]);
    add(&mut g, vivi, adding).unwrap();
    cite!(["106.4"]; assert_eq!((g.players[0].pool.colors[1], g.players[0].pool.colors[3]), (7, 13)));
}

#[test]
fn r602_5_only_during_its_controllers_turn_and_once_each_turn() {
    let mut g = game();
    let vivi = g.put(0, VIVI, Zone::Battlefield);
    counters(&mut g, vivi, 1);
    add(&mut g, vivi, vec![vec![r()]]).unwrap();
    cite!(["602.1b"]; refused("that ability is activated only once each turn (602.1b)"): add(&mut g, vivi, vec![vec![r()]]));
    // the opponent's turn
    let step = |g: &mut Game| match g.decision {
        Some(game::state::Decision::Attackers { player }) => g.act(player, Move::Attack(vec![])).unwrap(),
        Some(game::state::Decision::Blockers { player }) => g.act(player, Move::Block(vec![])).unwrap(),
        _ => g.act(g.priority, Move::Pass).unwrap(),
    };
    while g.active == 0 { step(&mut g); }
    while g.priority != 0 || g.decision.is_some() { step(&mut g); }
    cite!(["602.5b"]; refused("the activation condition does not hold (602.5)"): add(&mut g, vivi, vec![vec![r()]]));
}

#[test]
fn r603_2_a_noncreature_spell_grows_vivi_and_pings_each_opponent() {
    let mut g = game();
    let vivi = g.put(0, VIVI, Zone::Battlefield);
    let opt = g.put(0, OPT, Zone::Hand);
    g.players[0].pool.colors[1] = 1;
    g.act(0, paid("U",Move::Cast { card: opt, targets: vec![] })).unwrap();
    while !g.stack.is_empty() {
        match g.asked() {
            Some((p, game::legal::Asked::Arrange { cards, .. })) => g.act(p, Move::Choose(game::state::Answer::Arrange { top: cards, away: vec![] })).unwrap(),
            _ => g.act(g.priority, Move::Pass).unwrap(),
        }
    }
    cite!(["603.2"]; assert_eq!(g.obj(vivi).counters.get("+1/+1"), Some(&1)));
    cite!(["603.2"]; assert_eq!(g.players[1].life, 19));
    // a creature spell is not one
    let bear = g.put(0, BEAR, Zone::Hand);
    g.act(0, Move::Cast { card: bear, targets: vec![] }).unwrap();
    while !g.stack.is_empty() { g.act(g.priority, Move::Pass).unwrap(); }
    assert_eq!(g.obj(vivi).counters.get("+1/+1"), Some(&1));
}

/// A plan says Vivi's mana in any order: a pool's mana has none (106.4), so {R}{U} is the
/// offered {U}{R}. Mana it does not offer refuses the cast rather than being set aside.
#[test]
fn r605_3b_a_plans_mana_is_the_same_in_any_order_and_unoffered_mana_is_refused() {
    let mut g = game();
    let vivi = g.put(0, VIVI, Zone::Battlefield);
    counters(&mut g, vivi, 2);
    let ogre = g.put(0, OGRE, Zone::Hand);
    let cast = |adding: Vec<Vec<ManaSymbol>>| Move::WithMana(
        vec![game::legal::Step { plan: vec![], x: None, life: vec![], with: None, cost_branches: None, chose: vec![],  source: vivi, ability: 0, paying: vec![], adding }],
        Box::new(paid("UR",Move::Cast { card: ogre, targets: vec![] })));
    let logged = g.log.len();
    let green = ManaSymbol::Color(Color::Green);
    cite!(["608.2d"]; refused("that mana ability offers no such choice of mana (608.2d)"): g.act(0, cast(vec![vec![green, r()]])));
    assert_eq!((g.log.len(), g.obj(ogre).zone), (logged, Zone::Hand), "a refused cast commits nothing");
    assert!(g.decision.is_none());
    cite!(["601.2g"]; ok: g.act(0, cast(vec![vec![r(), u()]])));
    assert!(g.decision.is_none(), "no separate choice of mana follows");
    assert_eq!(added(&g, vivi), [vec![u(), r()]]);
}
