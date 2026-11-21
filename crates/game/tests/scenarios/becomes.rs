//! What a permanent becomes (CR 205.1, 613.1d, 613.4b, 208.3). Every card is read from its
//! printed text.

use crate::support::{cite, paid};
use game::card::CardDef;
use game::play::check;
use game::{Game, Move, Target, Zone};
use ir::CardType;

const MOUNTAIN: usize = 0;
const MUTAVAULT: usize = 1;
const SHOCK: usize = 2;

fn defs() -> Vec<CardDef> {
    let card = |name, cost, types, text, pt| CardDef::new(name, cost, types, text, pt).expect(name);
    vec![
        card("Mountain", "", "Basic Land — Mountain", "({T}: Add {R}.)", None),
        card("Mutavault", "", "Land", "{T}: Add {C}.\n{1}: Until end of turn, this land becomes a 2/2 creature with all creature types. It's still a land.", None),
        card("Shock", "{R}", "Instant", "Shock deals 2 damage to any target.", None),
    ]
}

#[test]
fn every_card_of_the_tests_is_one_the_engine_runs() {
    for d in defs() {
        check(&d).unwrap_or_else(|u| panic!("{}: {}", d.name(), u.0));
    }
}

#[test]
fn r205_1b_a_land_that_becomes_a_creature_and_is_still_a_land() {
    let mut g = Game::new(defs(), 2, 20);
    let vault = g.put(0, MUTAVAULT, Zone::Battlefield);
    // a noncreature permanent has no power or toughness (208.3), and can't be dealt damage
    cite!(["208.3"]; assert_eq!(g.power(vault), None));
    assert!(!g.is_a(vault, CardType::Creature));
    let mountain = g.put(0, MOUNTAIN, Zone::Battlefield);
    g.act(0, Move::Activate { source: mountain, ability: 0, targets: vec![], paying: vec![] }).unwrap();
    g.act(0, paid("R", Move::Activate { source: vault, ability: 1, targets: vec![], paying: vec![] })).unwrap();
    g.act(0, Move::Pass).unwrap();
    g.act(1, Move::Pass).unwrap();
    // a 2/2 creature (613.1d, 613.4b), still a land (205.1b), with every creature type (205.3m)
    cite!(["205.1b"]; assert!(g.is_a(vault, CardType::Creature) && g.is_a(vault, CardType::Land)));
    cite!(["613.4b"]; assert_eq!(g.power(vault), Some(2)));
    let seen = g.seen(vault, None);
    cite!(["205.3m"]; assert!(seen.subtypes.iter().any(|s| s.name() == "Goblin") && seen.subtypes.iter().any(|s| s.name() == "Elf")));
    // it is dealt damage as a creature is, and dies of it
    let mountain = g.put(0, MOUNTAIN, Zone::Battlefield);
    g.act(0, Move::Activate { source: mountain, ability: 0, targets: vec![], paying: vec![] }).unwrap();
    let shock = g.put(0, SHOCK, Zone::Hand);
    g.act(0, paid("R", Move::Cast { card: shock, targets: vec![vec![Target::Object(vault)]] })).unwrap();
    g.act(0, Move::Pass).unwrap();
    g.act(1, Move::Pass).unwrap();
    cite!(["704.5g"]; assert_eq!(g.obj(vault).zone, Zone::Gone));
    assert_eq!(g.players[0].graveyard.len(), 2);
}
