//! A cast of a card no rule or effect lets its player cast says where the card is and the rule
//! (601.3), instead of "that card is not in your hand".
use crate::support::cite;
use game::card::CastAs;
use game::{CardDef, Game, Move, Target, Zone};
use ir::Part;

fn game(names: &[&str], custom: Vec<CardDef>) -> Game {
    let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let mut defs: Vec<CardDef> = names.iter().map(|n| CardDef::from_face(faces.iter().find(|f| f.name == *n).unwrap()).unwrap()).collect();
    defs.extend(custom);
    defs.push(CardDef::new("Island", "", "Basic Land — Island", "", None).unwrap());
    for d in &defs { game::play::check(d).unwrap_or_else(|e| panic!("{}: {}", d.name(), e.0)); }
    let island = defs.len() - 1;
    let mut g = Game::new(defs, 2, 20);
    for p in 0..2 { for _ in 0..8 { g.put(p, island, Zone::Library); } }
    while g.part != Part::PrecombatMain { g.act(g.priority, Move::Pass).unwrap(); }
    g
}
fn c(name: &str, cost: &str, types: &str, text: &str, pt: Option<(i32, i32)>) -> CardDef { CardDef::new(name, cost, types, text, pt).expect(name) }
fn cast(g: &mut Game, card: game::ObjId) -> Result<(), String> { g.act(0, Move::Cast { card, targets: vec![] }) }

#[test]
fn a_card_elsewhere_says_where_it_is() {
    let mut g = game(&[], vec![c("Bear", "{0}", "Creature — Bear", "", Some((2, 2)))]);
    let exiled = g.put(0, 0, Zone::Exile);
    cite!(["601.3"]; assert_eq!(cast(&mut g, exiled), Err("that card is not in your hand: it is in exile, and no rule or effect lets you cast it from there (601.3)".into())));
    let dead = g.put(0, 0, Zone::Graveyard);
    cite!(["601.3"]; assert_eq!(cast(&mut g, dead), Err("that card is not in your hand: it is in a graveyard, and no rule or effect lets you cast it from there (601.3)".into())));
    let theirs = g.put(1, 0, Zone::Hand);
    cite!(["601.3"]; assert_eq!(cast(&mut g, theirs), Err("that card is in another player's hand; you cast only from your own (601.3)".into())));
    let land = g.put(0, 1, Zone::Exile);
    cite!(["305.1"]; assert_eq!(g.act(0, Move::PlayLand(land)), Err("that card is not in your hand: it is in exile, and no rule or effect lets you play it from there (305.1)".into())));
}

#[test]
fn an_adventurer_card_on_an_adventure_is_not_cast_as_its_adventure_again() {
    let adventurer = CardDef::adventurer(
        c("Giant", "{0}", "Creature — Giant", "", Some((4, 3))),
        c("Stomp", "{0}", "Instant — Adventure", "Stomp deals 2 damage to any target.", None)).unwrap();
    let mut g = game(&[], vec![adventurer]);
    let giant = g.put(0, 0, Zone::Hand);
    let stomp = |card| Move::CastAs(CastAs::Adventure, Box::new(Move::Cast { card, targets: vec![vec![Target::Player(1)]] }));
    g.act(0, stomp(giant)).unwrap();
    while !g.stack.is_empty() { g.act(g.priority, Move::Pass).unwrap(); }
    let exiled = *g.exile.last().expect("on an adventure");
    let again = g.act(0, stomp(exiled));
    assert_eq!(again, Err("it was exiled on an adventure: it may be cast as its creature or other main card, not as its Adventure (715.3d)".into()));
}
