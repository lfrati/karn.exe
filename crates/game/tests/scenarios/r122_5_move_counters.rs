//! To move a counter is to remove it from one object and put it onto a second (122.5).
//! Nothing moves when the two are the same object, when the first has no such counter, or
//! when either is gone. Every card is read from its Oracle text.
use crate::support::{cite, mana_pool, oracle, paid};
use game::{Game, Move, ObjId, Target, Zone};

const RACK: usize = 0;
const TRANSFER: usize = 1;
const DAGHATAR: usize = 2;
const BEARS: usize = 3;

/// Weapon Rack, Fate Transfer, Daghatar the Adamant and Grizzly Bears, with libraries to draw
/// from.
fn setup() -> Game {
    let defs: Vec<_> = ["Weapon Rack", "Fate Transfer", "Daghatar the Adamant", "Grizzly Bears", "Forest"]
        .into_iter().map(oracle).collect();
    let mut g = Game::new(defs, 2, 20);
    for p in 0..2 { for _ in 0..5 { g.put(p, 4, Zone::Library); } }
    while g.part != ir::Part::PrecombatMain { g.act(g.priority, Move::Pass).unwrap(); }
    g
}
fn resolve(g: &mut Game) { for _ in 0..2 { g.act(g.priority, Move::Pass).unwrap(); } }
/// Player 0 casts this card for this mana and it resolves.
fn cast(g: &mut Game, card: usize, mana: &str) -> ObjId {
    let object = g.put(0, card, Zone::Hand);
    g.players[0].pool = mana_pool(mana);
    g.act(0, paid(mana, Move::Cast { card: object, targets: vec![] })).unwrap();
    resolve(g);
    *g.battlefield.iter().rev().find(|o| g.obj(**o).card == Some(card)).unwrap()
}
fn plus(g: &Game, o: ObjId) -> u32 { g.counters(o).get("+1/+1").copied().unwrap_or(0) }
/// Player 0 activates Daghatar's last ability: "Move a +1/+1 counter from target creature onto
/// a second target creature."
fn daghatar(g: &mut Game, daghatar: ObjId, from: ObjId, onto: ObjId) -> Result<(), String> {
    g.players[0].pool = mana_pool("GGG");
    let targets = vec![vec![Target::Object(from)], vec![Target::Object(onto)]];
    g.act(0, paid("GGG", Move::Activate { source: daghatar, ability: 2, targets, paying: vec![] })).map(|_| ()).map_err(|e| e.to_string())
}

#[test]
fn the_cards_of_these_tests_run() {
    for name in ["Weapon Rack", "Fate Transfer", "Daghatar the Adamant", "Simic Fluxmage", "Explorer's Cache", "Gatewatch Beacon"] {
        // Uncited (admission): the engine runs each card of these tests.
        assert_eq!(game::play::check(&oracle(name)).err().map(|u| u.0), None, "{name}");
    }
}

#[test]
fn r122_5_weapon_rack_moves_a_counter_until_it_has_none() {
    let mut g = setup();
    let rack = cast(&mut g, RACK, "GGGG");
    cite!(["614.1c"]; assert_eq!(plus(&g, rack), 3));
    let bears = g.put(0, BEARS, Zone::Battlefield);
    for moved in 1..=4u32 {
        g.act(0, Move::Activate { source: rack, ability: 1, targets: vec![vec![Target::Object(bears)]], paying: vec![] }).unwrap();
        resolve(&mut g);
        // Ruling: "Once Weapon Rack runs out of +1/+1 counters, it remains on the battlefield.
        // You can activate its last ability, but it won't do anything."
        cite!(["122.5"]; assert_eq!((plus(&g, rack), plus(&g, bears)), (3 - moved.min(3), moved.min(3))));
        g.untap(rack);
    }
    cite!(["122.5"]; assert!(g.battlefield.contains(&rack)));
}

#[test]
fn r122_5_fate_transfer_moves_every_kind_of_counter() {
    // Ruling: "Fate Transfer moves any kind of counters, not just -1/-1 counters."
    let mut g = setup();
    let dag = cast(&mut g, DAGHATAR, "WWWW");
    let bears = g.put(0, BEARS, Zone::Battlefield);
    g.set_initial_counters(dag, "charge", 2);
    let spell = g.put(0, TRANSFER, Zone::Hand);
    g.players[0].pool = mana_pool("UU");
    g.act(0, paid("UU", Move::Cast { card: spell, targets: vec![vec![Target::Object(dag)], vec![Target::Object(bears)]] })).unwrap();
    resolve(&mut g);
    cite!(["122.5"]; assert_eq!(g.counters(bears).iter().map(|(k, n)| (k.as_str(), *n)).collect::<Vec<_>>(), vec![("+1/+1", 4), ("charge", 2)]));
    // the 0/0 left with no counters is put into its owner's graveyard
    cite!(["122.5", "704.5f"]; assert!(!g.battlefield.contains(&dag)));
}

#[test]
fn r115_1_fate_transfer_moves_onto_another_creature_than_the_first() {
    // "another target creature" differs from the first target (115.1, 601.2c)
    let mut g = setup();
    let dag = cast(&mut g, DAGHATAR, "WWWW");
    let spell = g.put(0, TRANSFER, Zone::Hand);
    g.players[0].pool = mana_pool("UU");
    let said = g.act(0, paid("UU", Move::Cast { card: spell, targets: vec![vec![Target::Object(dag)], vec![Target::Object(dag)]] })).unwrap_err();
    cite!(["115.1", "601.2c"]; assert_eq!(said, "another target must differ from the preceding targets (115.1, 601.2c)"));
}

#[test]
fn r122_5_daghatar_moves_a_counter_between_two_other_creatures() {
    let mut g = setup();
    let dag = cast(&mut g, DAGHATAR, "WWWW");
    cite!(["614.1c"]; assert_eq!(plus(&g, dag), 4));
    let (first, second) = (g.put(0, BEARS, Zone::Battlefield), g.put(1, BEARS, Zone::Battlefield));
    g.put_counters(first, "+1/+1", 1);
    daghatar(&mut g, dag, first, second).unwrap();
    resolve(&mut g);
    cite!(["122.5"]; assert_eq!((plus(&g, dag), plus(&g, first), plus(&g, second)), (4, 0, 1)));
    // Ruling: "Either one may be Daghatar the Adamant."
    g.untap(dag);
    daghatar(&mut g, dag, dag, first).unwrap();
    resolve(&mut g);
    cite!(["122.5"]; assert_eq!((plus(&g, dag), plus(&g, first)), (3, 1)));
}

#[test]
fn r115_1_daghatar_s_two_targets_are_different_creatures() {
    // Ruling: "The two targets of the last ability must be different creatures."
    let mut g = setup();
    let dag = cast(&mut g, DAGHATAR, "WWWW");
    let said = daghatar(&mut g, dag, dag, dag).unwrap_err();
    cite!(["115.1", "601.2c"]; assert_eq!(said, "another target must differ from the preceding targets (115.1, 601.2c)"));
}

#[test]
fn r608_2b_nothing_moves_once_the_second_target_is_gone() {
    // Ruling: "The +1/+1 counter is moved only if both targets are still legal as the ability
    // resolves."
    let mut g = setup();
    let dag = cast(&mut g, DAGHATAR, "WWWW");
    let bears = g.put(1, BEARS, Zone::Battlefield);
    daghatar(&mut g, dag, dag, bears).unwrap();
    g.destroy(bears);
    resolve(&mut g);
    cite!(["608.2b", "122.5"]; assert_eq!(plus(&g, dag), 4));
}
