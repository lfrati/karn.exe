//! "if this creature has fewer than three +1/+1 counters on it" is an intervening "if" clause
//! (603.4) counting counters (122.1): three is not fewer than three. Runaway Steam-Kin,
//! "Whenever you cast a red spell, if this creature has fewer than three +1/+1 counters on it,
//! put a +1/+1 counter on this creature. Remove three +1/+1 counters from this creature: Add
//! {R}{R}{R}."; Adaptive Training Post, the same with charge counters on instant and sorcery
//! spells.
use crate::support::{cite, paid};
use game::state::Pool;
use game::{CardDef, Game, Move, ObjId, Target, Zone};
use ir::Part;

/// Oracle cards by name, with ten Wastes in each library, in player 0's precombat main phase.
fn game(names: &[&str]) -> Game {
    let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let catalog = CardDef::catalog(&faces);
    let defs: Vec<_> = names.iter().chain(["Wastes"].iter()).map(|n| {
        let d = CardDef::from_faces(&catalog[*n]).unwrap_or_else(|e| panic!("{n}: {e:?}"));
        game::play::check(&d).unwrap_or_else(|e| panic!("{n}: {}", e.0));
        d
    }).collect();
    let wastes = defs.len() - 1;
    let mut g = Game::new(defs, 2, 20);
    for p in 0..2 { for _ in 0..10 { g.put(p, wastes, Zone::Library); } }
    while g.part != Part::PrecombatMain { g.act(g.priority, Move::Pass).unwrap(); }
    g
}
fn mana(g: &mut Game) { g.players[0].pool.colors = [9; 5]; g.players[0].pool.colorless = 9; }
fn resolve(g: &mut Game) {
    for _ in 0..30 {
        match g.decision.clone() {
            None if g.stack.is_empty() => return,
            None => g.act(g.priority, Move::Pass).unwrap(),
            Some(d) => panic!("{d:?}"),
        }
    }
}
fn counters(g: &Game, o: ObjId, kind: &str) -> u32 { g.obj(o).counters.get(kind).copied().unwrap_or(0) }

const KIN: usize = 0;
const SHOCK: usize = 1;
const GROWTH: usize = 2;

/// Each red spell puts a counter on Runaway Steam-Kin while it has two or fewer; with three it
/// doesn't trigger, and a green spell never does. Removing the three adds {R}{R}{R}.
#[test]
fn r603_4_runaway_steam_kin_grows_while_it_has_fewer_than_three_counters() {
    let mut g = game(&["Runaway Steam-Kin", "Shock", "Giant Growth"]);
    let kin = g.put(0, KIN, Zone::Battlefield);
    let growth = g.put(0, GROWTH, Zone::Hand);
    mana(&mut g);
    g.act(0, paid("G", Move::Cast { card: growth, targets: vec![vec![Target::Object(kin)]] })).unwrap();
    cite!(["603.2"]; assert_eq!(g.stack.len(), 1, "a green spell is no red spell"));
    resolve(&mut g);
    for n in 1..=3 {
        let shock = g.put(0, SHOCK, Zone::Hand);
        g.act(0, paid("R", Move::Cast { card: shock, targets: vec![vec![Target::Player(1)]] })).unwrap();
        resolve(&mut g);
        cite!(["603.4", "122.1"]; assert_eq!(counters(&g, kin, "+1/+1"), n));
    }
    let shock = g.put(0, SHOCK, Zone::Hand);
    g.act(0, paid("R", Move::Cast { card: shock, targets: vec![vec![Target::Player(1)]] })).unwrap();
    cite!(["603.4"]; assert_eq!(g.stack.len(), 1, "three counters are not fewer than three"));
    resolve(&mut g);
    cite!(["603.4"]; assert_eq!(counters(&g, kin, "+1/+1"), 3));

    g.players[0].pool = Pool::default();
    g.act(0, Move::Activate { source: kin, ability: 1, targets: vec![], paying: vec![] }).unwrap();
    cite!(["605.3b", "122.8"]; assert_eq!((g.players[0].pool.colors[3], g.players[0].pool.total(), counters(&g, kin, "+1/+1")), (3, 3, 0)));
}

/// With two counters, two red spells cast in a row each trigger; the first trigger to resolve
/// puts the third counter, and the second then finds three and does nothing (603.4).
#[test]
fn r603_4_the_condition_is_checked_again_as_the_trigger_resolves() {
    let mut g = game(&["Runaway Steam-Kin", "Shock", "Giant Growth"]);
    let kin = g.put(0, KIN, Zone::Battlefield);
    g.put_counters(kin, "+1/+1", 2);
    mana(&mut g);
    for _ in 0..2 {
        let shock = g.put(0, SHOCK, Zone::Hand);
        g.act(0, paid("R", Move::Cast { card: shock, targets: vec![vec![Target::Player(1)]] })).unwrap();
    }
    cite!(["603.4"]; assert_eq!(g.stack.len(), 4, "both triggered"));
    resolve(&mut g);
    cite!(["603.4"]; assert_eq!(counters(&g, kin, "+1/+1"), 3));
}

const POST: usize = 0;

/// Adaptive Training Post gets a charge counter for each instant spell while it has two or
/// fewer, and none at three.
#[test]
fn r603_4_adaptive_training_post_charges_up_to_three() {
    let mut g = game(&["Adaptive Training Post", "Shock"]);
    let post = g.put(0, POST, Zone::Battlefield);
    mana(&mut g);
    for n in [1, 2, 3, 3] {
        let shock = g.put(0, SHOCK, Zone::Hand);
        g.act(0, paid("R", Move::Cast { card: shock, targets: vec![vec![Target::Player(1)]] })).unwrap();
        resolve(&mut g);
        cite!(["603.4", "122.1"]; assert_eq!(counters(&g, post, "charge"), n));
    }
    cite!(["120.3"]; assert_eq!(g.players[1].life, 12));
}
