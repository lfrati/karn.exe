//! "If a permanent dealt damage by Torch the Tower would die this turn, exile it
//! instead." Damage prevented was not dealt (615.1, 120.1); a permanent that left and came back
//! is a new object never dealt damage (400.7); a planeswalker put into a graveyard for having
//! no loyalty dies (700.4, 704.5i). And "it deals ... to that ..." where "it" is the spell:
//! Burst Lightning.
use crate::support::{cite, paid};
use game::legal::Asked;
use game::state::Answer;
use game::{CardDef, Event, Game, Move, ObjId, Target, Zone};
use ir::Part;

const TORCH: usize = 0;
const GIANT: usize = 1;
const MURDER: usize = 2;
const SHIELD: usize = 3;
const FLICKER: usize = 4;
const WALKER: usize = 5;
const RAGE: usize = 6; // Burst Lightning
const ISLAND: usize = 7;

fn game() -> Game {
    let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let real = |name: &str| CardDef::from_face(faces.iter().find(|f| f.face_name == name).unwrap()).unwrap();
    let c = |name, cost, types, text, pt| CardDef::new(name, cost, types, text, pt).expect(name);
    let walker = CardDef::planeswalker("Walker", "{0}", "Legendary Planeswalker — Jace", "+1: You gain 1 life.", 2).unwrap();
    let defs = vec![
        c("Torch the Tower", "{R}", "Instant", "Torch the Tower deals 2 damage to target creature or planeswalker.\nIf a permanent dealt damage by Torch the Tower would die this turn, exile it instead.", None),
        c("Giant", "{0}", "Creature — Giant", "", Some((3, 3))),
        c("Murder", "{0}", "Instant", "Destroy target creature.", None),
        c("Shield", "{0}", "Instant", "Prevent all damage that would be dealt to target creature this turn.", None),
        c("Flicker", "{0}", "Instant", "Exile target creature, then return it to the battlefield under its owner's control.", None),
        walker,
        real("Burst Lightning"),
        c("Island", "", "Basic Land — Island", "", None),
    ];
    for d in &defs { game::play::check(d).unwrap_or_else(|e| panic!("{}: {}", d.name(), e.0)); }
    let mut g = Game::new(defs, 2, 20);
    for p in 0..2 { for _ in 0..10 { g.put(p, ISLAND, Zone::Library); } }
    while g.part != Part::PrecombatMain { g.act(g.priority, Move::Pass).unwrap(); }
    g
}

fn resolve(g: &mut Game) {
    for _ in 0..40 {
        match g.asked() {
            Some((p, Asked::Arrange { cards, .. })) => g.act(p, Move::Choose(Answer::Arrange { top: cards, away: vec![] })).unwrap(),
            _ if g.stack.is_empty() => return,
            _ => g.act(g.priority, Move::Pass).unwrap(),
        }
    }
    panic!("the stack did not empty: {:?}", g.asked());
}
fn cast_with(g: &mut Game, card: usize, target: Target, kicks: u32, spending: &str) {
    let spell = g.put(0, card, Zone::Hand);
    g.players[0].pool.colors = [9; 5];
    g.players[0].pool.colorless = 9;
    let cast = Move::Cast { card: spell, targets: vec![vec![target]] };
    g.act(0, paid(spending, if kicks > 0 { Move::WithKicks(kicks, Box::new(cast)) } else { cast })).unwrap();
    resolve(g);
}
fn cast(g: &mut Game, card: usize, target: ObjId, spending: &str) { cast_with(g, card, Target::Object(target), 0, spending) }
fn exiled(g: &Game, card: usize) -> bool { g.exile.iter().any(|o| g.obj(*o).card == Some(card)) }
fn buried(g: &Game, card: usize) -> bool { g.players[1].graveyard.iter().any(|o| g.obj(*o).card == Some(card)) }
fn dealt(g: &Game) -> Vec<u32> {
    g.log.iter().filter_map(|e| if let Event::DamageDealt { amount, .. } = e { Some(*amount) } else { None }).collect()
}

#[test]
fn prevented_damage_was_not_dealt() {
    let mut g = game();
    let giant = g.put(1, GIANT, Zone::Battlefield);
    cast(&mut g, SHIELD, giant, "");
    cast(&mut g, TORCH, giant, "R");
    cite!(["615.1a"]; assert_eq!(g.obj(giant).damage, 0));
    cast(&mut g, MURDER, giant, "");
    cite!(["615.1a"]; assert!(buried(&g, GIANT) && !exiled(&g, GIANT)));
}

#[test]
fn a_creature_that_left_and_returned_is_a_new_object() {
    let mut g = game();
    let giant = g.put(1, GIANT, Zone::Battlefield);
    cast(&mut g, TORCH, giant, "R");
    cast(&mut g, FLICKER, giant, "");
    let back = *g.battlefield.iter().find(|o| g.obj(**o).card == Some(GIANT)).unwrap();
    cite!(["400.7"]; assert_ne!(back, giant));
    cast(&mut g, MURDER, back, "");
    cite!(["400.7"]; assert!(buried(&g, GIANT) && !exiled(&g, GIANT)));
}

#[test]
fn a_planeswalker_with_no_loyalty_dies_and_is_exiled() {
    let mut g = game();
    let walker = g.put(1, WALKER, Zone::Battlefield);
    if g.obj(walker).counters.get("loyalty").is_none() { g.put_counters(walker, "loyalty", 2); }
    cast(&mut g, TORCH, walker, "R");
    cite!(["704.5i", "700.4", "614.1a"]; assert!(exiled(&g, WALKER) && !buried(&g, WALKER)));
}

/// "If this spell was kicked, it deals 4 damage to that permanent or player instead": the
/// spell deals 4, to a player as to a creature, and not 2 as well.
#[test]
fn burst_lightning_kicked_deals_four_by_itself() {
    let mut g = game();
    cast_with(&mut g, RAGE, Target::Player(1), 1, "CCCCR");
    assert_eq!(g.players[1].life, 16);
    cite!(["608.2c", "702.33d"]; assert_eq!(dealt(&g), [4]));
    let giant = g.put(1, GIANT, Zone::Battlefield);
    cast_with(&mut g, RAGE, Target::Object(giant), 1, "CCCCR");
    assert!(buried(&g, GIANT));
    cite!(["608.2c", "702.33d"]; assert_eq!(dealt(&g), [4, 4]));
}
