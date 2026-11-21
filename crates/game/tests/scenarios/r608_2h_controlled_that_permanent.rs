//! "If you controlled that permanent" (Boomerang Basics): asked of the permanent as it last
//! existed on the battlefield (608.2h), after it has been returned. Its controller then, not
//! its owner, is who controlled it; a card in a hand has no controller of its own as a
//! permanent (108.4a).
use crate::support::{cite, paid};
use game::{CardDef, Game, Move, ObjId, Target, Zone};

const BASICS: usize = 0;
const BEAR: usize = 1;
const CONTROL: usize = 2;

fn setup() -> Game {
    let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let defs = vec![
        CardDef::from_face(faces.iter().find(|f| f.face_name == "Boomerang Basics").unwrap()).unwrap(),
        CardDef::new("Bear", "{0}", "Creature — Bear", "", Some((2, 2))).unwrap(),
        CardDef::new("Control", "{0}", "Instant", "Gain control of target permanent.", None).unwrap(),
    ];
    for d in &defs {
        game::play::check(d).unwrap_or_else(|e| panic!("{}: {e:?}", d.name()));
    }
    let mut g = Game::new(defs, 2, 20);
    for p in 0..2 {
        for _ in 0..10 {
            g.put(p, BEAR, Zone::Library);
        }
    }
    g
}

fn resolve(g: &mut Game) {
    while !g.stack.is_empty() {
        g.act(g.priority, Move::Pass).unwrap();
    }
}

fn cast(g: &mut Game, which: usize, at: ObjId, spending: &str) {
    let card = g.put(0, which, Zone::Hand);
    g.players[0].pool.colors[1] = 1;
    g.act(0, paid(spending, Move::Cast { card, targets: vec![vec![Target::Object(at)]] })).unwrap();
    resolve(g);
}

#[test]
fn r608_2h_you_draw_for_your_own_permanent() {
    let mut g = setup();
    let bear = g.put(0, BEAR, Zone::Battlefield);
    cast(&mut g, BASICS, bear, "U");
    assert!(g.battlefield.is_empty());
    // the Bear and the card drawn
    assert_eq!(g.players[0].hand.len(), 2);
    cite!(["608.2h"]; assert_eq!(g.players[0].library.len(), 9));
}

#[test]
fn r608_2h_no_card_for_an_opponents_permanent() {
    let mut g = setup();
    let bear = g.put(1, BEAR, Zone::Battlefield);
    cast(&mut g, BASICS, bear, "U");
    assert!(g.battlefield.is_empty());
    assert_eq!((g.players[0].hand.len(), g.players[1].hand.len()), (0, 1));
    cite!(["608.2h"]; assert_eq!(g.players[0].library.len(), 10));
}

#[test]
fn r608_2h_a_stolen_permanent_was_yours_though_its_card_goes_to_its_owner() {
    let mut g = setup();
    let bear = g.put(1, BEAR, Zone::Battlefield);
    cast(&mut g, CONTROL, bear, "");
    assert_eq!(g.controller(bear), 0);
    cast(&mut g, BASICS, bear, "U");
    assert!(g.battlefield.is_empty());
    cite!(["400.3"]; assert_eq!((g.players[0].hand.len(), g.players[1].hand.len()), (1, 1)));
    cite!(["608.2h"]; assert_eq!(g.players[0].library.len(), 9));
}

#[test]
fn r608_2h_your_card_an_opponent_controls_was_not_yours() {
    let mut g = setup();
    let bear = g.put(0, BEAR, Zone::Battlefield);
    // player 1 takes it
    let card = g.put(1, CONTROL, Zone::Hand);
    g.players[1].pool.colors[1] = 1;
    g.priority = 1;
    g.act(1, Move::Cast { card, targets: vec![vec![Target::Object(bear)]] }).unwrap();
    resolve(&mut g);
    assert_eq!(g.controller(bear), 1);
    cast(&mut g, BASICS, bear, "U");
    assert!(g.battlefield.is_empty());
    // the Bear is back in player 0's hand, and nothing was drawn
    assert_eq!(g.players[0].hand.len(), 1);
    cite!(["608.2h"]; assert_eq!(g.players[0].library.len(), 10));
}
