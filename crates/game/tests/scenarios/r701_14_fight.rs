//! Fight (701.14): each of two creatures deals damage equal to its power to the other, at
//! once, and not as combat damage.
use crate::support::cite;
use game::{CardDef, Game, Move, ObjId, Target, Zone};

const PREDATOR: usize = 0;
const PREY_UPON: usize = 1;
const BEAR: usize = 2;
const GIANT: usize = 3;
const SHATTER: usize = 4;
const TOUCH: usize = 5;

fn defs() -> Vec<CardDef> {
    vec![
        CardDef::new("Nightfall Predator", "{2}{G}", "Creature — Werewolf", "{R}, {T}: This creature fights target creature.", Some((4, 4))).unwrap(),
        CardDef::new("Prey Upon", "{G}", "Sorcery", "Target creature you control fights target creature you don't control.", None).unwrap(),
        CardDef::new("Bear", "{1}{G}", "Creature — Bear", "", Some((2, 2))).unwrap(),
        CardDef::new("Giant", "{3}{R}", "Creature — Giant", "", Some((3, 5))).unwrap(),
        CardDef::new("Murder", "{1}{B}{B}", "Instant", "Destroy target creature.", None).unwrap(),
        CardDef::new("Typhoid Rats", "{B}", "Creature — Rat", "Deathtouch", Some((1, 1))).unwrap(),
    ]
}
fn game() -> Game {
    let defs = defs();
    for d in &defs {
        game::play::check(d).unwrap_or_else(|u| panic!("{}: {}", d.name(), u.0));
    }
    Game::new(defs, 2, 20)
}
fn resolve(g: &mut Game) {
    while !g.stack.is_empty() { g.act(g.priority, Move::Pass).unwrap(); }
}
fn cast(g: &mut Game, player: usize, card: usize, targets: Vec<Vec<Target>>) -> ObjId {
    let spell = g.put(player, card, Zone::Hand);
    g.players[player].pool.colors = [9, 9, 9, 9, 9];
    g.act(player, crate::support::paid(["WWG", "G", "WG", "WWWR", "WBB", "B"][card], Move::Cast { card: spell, targets })).unwrap();
    spell
}
fn fight(g: &mut Game, predator: ObjId, with: ObjId) {
    g.players[0].pool.colors = [9, 9, 9, 9, 9];
    g.act(0, crate::support::paid("R", Move::Activate { source: predator, ability: 0, targets: vec![vec![Target::Object(with)]], paying: vec![] })).unwrap();
}

#[test]
fn r701_14a_each_deals_damage_equal_to_its_power_to_the_other() {
    let mut g = game();
    let predator = g.put(0, PREDATOR, Zone::Battlefield);
    let giant = g.put(1, GIANT, Zone::Battlefield);
    fight(&mut g, predator, giant);
    resolve(&mut g);
    cite!(["701.14a"]; assert_eq!((g.obj(predator).damage, g.obj(giant).damage), (3, 4)));
    assert!(g.battlefield.contains(&predator) && g.battlefield.contains(&giant));
}

#[test]
fn r701_14a_two_targets_fight_and_both_can_die() {
    let mut g = game();
    let bear = g.put(0, BEAR, Zone::Battlefield);
    let rats = g.put(1, TOUCH, Zone::Battlefield);
    cast(&mut g, 0, PREY_UPON, vec![vec![Target::Object(bear)], vec![Target::Object(rats)]]);
    resolve(&mut g);
    // the damage is dealt at once: the Rats' deathtouch kills the Bear that kills it
    cite!(["701.14a", "704.5g", "704.5h"]; assert!(!g.battlefield.contains(&bear) && !g.battlefield.contains(&rats)));
    assert_eq!((g.players[0].graveyard.len(), g.players[1].graveyard.len()), (2, 1));
}

#[test]
fn r701_14b_neither_fights_when_one_is_gone() {
    let mut g = game();
    let predator = g.put(0, PREDATOR, Zone::Battlefield);
    let bear = g.put(0, BEAR, Zone::Battlefield);
    let giant = g.put(1, GIANT, Zone::Battlefield);
    // one of two targets has left: the other is still a legal target, and nothing is dealt
    cast(&mut g, 0, PREY_UPON, vec![vec![Target::Object(bear)], vec![Target::Object(giant)]]);
    g.act(0, Move::Pass).unwrap();
    cast(&mut g, 1, SHATTER, vec![vec![Target::Object(bear)]]);
    resolve(&mut g);
    cite!(["701.14b"]; assert_eq!(g.obj(giant).damage, 0));
    // the creature whose ability it is has left: its target is dealt nothing
    fight(&mut g, predator, giant);
    g.act(0, Move::Pass).unwrap();
    cast(&mut g, 1, SHATTER, vec![vec![Target::Object(predator)]]);
    resolve(&mut g);
    assert!(!g.battlefield.contains(&predator));
    cite!(["701.14b"]; assert_eq!(g.obj(giant).damage, 0));
}

#[test]
fn r701_14c_a_creature_that_fights_itself_is_dealt_twice_its_power() {
    let mut g = game();
    let predator = g.put(0, PREDATOR, Zone::Battlefield);
    g.put_counters(predator, "+1/+1", 5);
    fight(&mut g, predator, predator);
    resolve(&mut g);
    cite!(["701.14c"]; assert_eq!(g.obj(predator).damage, 18, "a 9/9 deals 9 to itself twice"));
    cite!(["704.5g"]; assert!(!g.battlefield.contains(&predator)));
}
