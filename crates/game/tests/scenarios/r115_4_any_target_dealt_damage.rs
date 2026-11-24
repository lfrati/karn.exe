//! "any target that was dealt damage this turn": any target (115.4) is a creature, player,
//! planeswalker or battle, and the restriction is checked of each as it is chosen (601.2c) and
//! again as the spell resolves (608.2b). Needle Drop, "Needle Drop deals 1 damage to any target
//! that was dealt damage this turn. Draw a card."
use crate::support::{cite, paid};
use game::{CardDef, Game, Move, Target, Zone};
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

const NEEDLE: usize = 0;
const SHOCK: usize = 1;
const GIANT: usize = 2;

/// Before any damage, neither the opponent nor a creature is a legal target; after Shock, the
/// player it hit is, and Needle Drop deals them 1 damage and draws its caster a card.
#[test]
fn r115_4_needle_drop_targets_a_player_dealt_damage_this_turn() {
    let mut g = game(&["Needle Drop", "Shock", "Hill Giant"]);
    let giant = g.put(1, GIANT, Zone::Battlefield);
    mana(&mut g);
    let needle = g.put(0, NEEDLE, Zone::Hand);
    for target in [Target::Player(1), Target::Object(giant)] {
        cite!(["115.4", "601.2c"]; assert!(g.act(0, paid("R", Move::Cast { card: needle, targets: vec![vec![target]] })).unwrap_err().contains("illegal target")));
    }
    let shock = g.put(0, SHOCK, Zone::Hand);
    g.act(0, paid("R", Move::Cast { card: shock, targets: vec![vec![Target::Player(1)]] })).unwrap();
    resolve(&mut g);
    cite!(["120.3a"]; assert_eq!(g.players[1].life, 18));
    cite!(["115.4", "601.2c"]; assert!(g.act(0, paid("R", Move::Cast { card: needle, targets: vec![vec![Target::Object(giant)]] })).unwrap_err().contains("illegal target"), "the Giant was dealt no damage"));
    let hand = g.players[0].hand.len();
    g.act(0, paid("R", Move::Cast { card: needle, targets: vec![vec![Target::Player(1)]] })).unwrap();
    resolve(&mut g);
    cite!(["115.4", "120.3a", "121.1"]; assert_eq!((g.players[1].life, g.players[0].hand.len()), (17, hand)));
}

/// A creature dealt damage this turn is a legal target too: Shock and Needle Drop together kill
/// a 3/3.
#[test]
fn r115_4_needle_drop_finishes_a_creature_dealt_damage_this_turn() {
    let mut g = game(&["Needle Drop", "Shock", "Hill Giant"]);
    let giant = g.put(1, GIANT, Zone::Battlefield);
    mana(&mut g);
    let shock = g.put(0, SHOCK, Zone::Hand);
    g.act(0, paid("R", Move::Cast { card: shock, targets: vec![vec![Target::Object(giant)]] })).unwrap();
    resolve(&mut g);
    let needle = g.put(0, NEEDLE, Zone::Hand);
    g.act(0, paid("R", Move::Cast { card: needle, targets: vec![vec![Target::Object(giant)]] })).unwrap();
    resolve(&mut g);
    cite!(["115.4", "704.5g"]; assert!(!g.battlefield.contains(&giant)));
}
