//! A cast refused for where the card is (601.3): the refusal must not
//! say no rule lets the card be cast from there when one does.
use crate::support::{cite,paid};
use game::{CardDef, Game, Move, Zone};
use ir::Part;

fn c(name: &str, cost: &str, types: &str, text: &str, pt: Option<(i32, i32)>) -> CardDef { CardDef::new(name, cost, types, text, pt).expect(name) }

/// A flashback card in the graveyard, cast the ordinary way: flashback (702.34a) is a rule that
/// lets it be cast from there, by its own route.
#[test]
fn a_flashback_card_cast_normally_from_the_graveyard() {
    let defs = vec![
        c("Think", "{0}", "Sorcery", "You gain 1 life.\nFlashback {0}", None),
        CardDef::new("Island", "", "Basic Land — Island", "", None).unwrap(),
    ];
    for d in &defs { game::play::check(d).unwrap_or_else(|e| panic!("{}: {}", d.name(), e.0)); }
    let mut g = Game::new(defs, 2, 20);
    for p in 0..2 { for _ in 0..8 { g.put(p, 1, Zone::Library); } }
    while g.part != Part::PrecombatMain { g.act(g.priority, Move::Pass).unwrap(); }
    let think = g.put(0, 0, Zone::Graveyard);
    // the supplied flashback route authorizes the graveyard cast
    cite!(["702.34a"]; ok: g.act(0, Move::CastVia(game::announcement::CastRoute::Flashback, Box::new(Move::Cast { card:think,targets:vec![] }))));
}

/// A card foretold this turn, cast at once: foretell lets it be cast from exile only after this
/// turn (702.143a). Whatever the refusal says, it must not claim no rule lets it be cast from
/// there.
#[test]
fn a_card_foretold_this_turn_cast_at_once() {
    let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let defs = vec![
        CardDef::from_face(faces.iter().find(|f| f.name == "Demon Bolt").unwrap()).unwrap(),
        CardDef::new("Island", "", "Basic Land — Island", "", None).unwrap(),
    ];
    let mut g = Game::new(defs, 2, 20);
    for p in 0..2 { for _ in 0..8 { g.put(p, 1, Zone::Library); } }
    while g.part != Part::PrecombatMain { g.act(g.priority, Move::Pass).unwrap(); }
    let bolt = g.put(0, 0, Zone::Hand);
    g.players[0].pool.colorless = 2;
    g.act(0, paid("CC",Move::Foretell(bolt))).unwrap();
    while !g.stack.is_empty() { g.act(g.priority, Move::Pass).unwrap(); }
    let card = *g.exile.last().unwrap();
    g.players[0].pool.colors[3] = 3;
    g.players[0].pool.colorless = 2;
    let said = g.act(0, Move::CastVia(game::announcement::CastRoute::Foretell, Box::new(Move::Cast { card, targets: vec![vec![game::Target::Player(1)]] }))).unwrap_err();
    eprintln!("foretold this turn: {said}");
    assert!(!said.contains("no rule or effect lets you cast it from there"), "{said}");
}
