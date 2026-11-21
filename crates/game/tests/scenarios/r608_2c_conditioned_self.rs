//! "{5}: If this land isn't a creature, it becomes a 2/4 Wizard creature with “…”. It's still
//! a land." (Great Hall of the Biblioplex): "it" is the land the opening condition names, and
//! nothing else was said before it. The condition is checked as the ability resolves (608.2c):
//! a land that is already a creature does not become one again.
use crate::support::{cite, paid};
use game::state::Pool;
use ir::mana::ManaSymbol;
use ir::Color;
use game::{CardDef, Game, Move, ObjId, Zone};
use ir::{CardType, Part};

const HALL: usize = 0;
const BOLT: usize = 1;
const BEAR: usize = 2;

fn game() -> Game {
    let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let hall = CardDef::from_face(faces.iter().find(|f| f.name == "Great Hall of the Biblioplex").unwrap()).unwrap();
    let defs = vec![
        hall,
        CardDef::new("Bolt", "{R}", "Instant", "You gain 1 life.", None).unwrap(),
        CardDef::new("Bear", "{R}", "Creature — Bear", "", Some((2, 2))).unwrap(),
        CardDef::new("Island", "", "Basic Land — Island", "", None).unwrap(),
    ];
    for d in &defs { game::play::check(d).unwrap_or_else(|e| panic!("{}: {}", d.name(), e.0)); }
    let mut g = Game::new(defs, 2, 20);
    for p in 0..2 { for _ in 0..5 { g.put(p, 3, Zone::Library); } }
    while g.part != Part::PrecombatMain { g.act(g.priority, Move::Pass).unwrap(); }
    g
}
fn resolve(g: &mut Game) {
    for _ in 0..30 {
        match g.decision.clone() {
            Some(d) => panic!("{d:?}"),
            None if g.stack.is_empty() => return,
            None => g.act(g.priority, Move::Pass).unwrap(),
        }
    }
    panic!("did not resolve");
}
fn animate(g: &mut Game, hall: ObjId) {
    g.players[0].pool.colorless += 5;
    g.act(0, paid("CCCCC", Move::Activate { source: hall, ability: 2, targets: vec![], paying: vec![] })).unwrap();
    resolve(g);
}

#[test]
fn r608_2c_it_is_the_land_the_condition_names() {
    let mut g = game();
    let hall = g.put(0, HALL, Zone::Battlefield);
    animate(&mut g, hall);
    cite!(["608.2c", "205.1b"]; assert!(g.is_a(hall, CardType::Creature) && g.is_a(hall, CardType::Land), "a creature, still a land"));
    assert_eq!((g.power(hall), g.toughness(hall)), (Some(2), Some(4)));
    // its granted trigger: an instant cast pumps it
    let bolt = g.put(0, BOLT, Zone::Hand);
    g.players[0].pool.colors[3] = 1;
    g.act(0, paid("R", Move::Cast { card: bolt, targets: vec![] })).unwrap();
    resolve(&mut g);
    assert_eq!(g.power(hall), Some(3));
    assert_eq!(g.players[0].life, 21);
}

#[test]
fn r608_2c_a_hall_that_is_already_a_creature_is_unchanged() {
    let mut g = game();
    let hall = g.put(0, HALL, Zone::Battlefield);
    animate(&mut g, hall);
    let bolt = g.put(0, BOLT, Zone::Hand);
    g.players[0].pool.colors[3] = 1;
    g.act(0, paid("R", Move::Cast { card: bolt, targets: vec![] })).unwrap();
    resolve(&mut g);
    assert_eq!(g.power(hall), Some(3));
    // a second activation finds it a creature: no new 2/4 effect resets anything
    animate(&mut g, hall);
    cite!(["608.2c"]; assert_eq!((g.power(hall), g.toughness(hall)), (Some(3), Some(4))));
}

#[test]
fn r106_6_its_colored_mana_casts_only_instants_and_sorceries() {
    let mut g = game();
    let hall = g.put(0, HALL, Zone::Battlefield);
    let activate = Move::Activate { source: hall, ability: 1, targets: vec![], paying: vec![] };
    g.act(0, Move::Adding(vec![vec![ManaSymbol::Color(Color::Red)]], Box::new(activate))).unwrap();
    cite!(["119.4"]; assert_eq!(g.players[0].life, 19, "paid 1 life"));
    let selected = Pool { restricted: vec![g.players[0].pool.restricted[0].clone()], ..Pool::default() };
    let bear = g.put(0, BEAR, Zone::Hand);
    cite!(["106.6"]; refused("that mana can't be spent on that: its text restricts how it can be spent (106.6)"): g.act(0, Move::Paying(selected.clone(), Box::new(Move::Cast { card: bear, targets: vec![] }))));
    let bolt = g.put(0, BOLT, Zone::Hand);
    cite!(["106.6"]; ok: g.act(0, Move::Paying(selected, Box::new(Move::Cast { card: bolt, targets: vec![] }))));
    resolve(&mut g);
    assert_eq!(g.players[0].life, 20);
}
