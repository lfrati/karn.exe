//! "You may exile it. If you do, …" (118.12): an optional payment of an object the text names.
//! It is offered only where it can be made, and "if you do" asks whether it was made: not
//! whether the player said yes.

use crate::support::{cite,paid};
use game::card::CardDef;
use game::play::check;
use game::state::{Answer, Choice, Decision};
use game::{Game, Move, ObjId, Target, Zone};
use ir::{Color, ManaSymbol};

const RECTOR: usize = 0;
const SEAL: usize = 1;
const BOLT: usize = 2;
const RAISE: usize = 3;
const PLAINS: usize = 4;

fn game() -> Game {
    let card = |name: &str, cost: &str, types: &str, text: &str, pt| CardDef::new(name, cost, types, text, pt).expect(name);
    let defs = vec![
        card("Academy Rector", "{3}{W}", "Creature — Human Cleric", "When this creature dies, you may exile it. If you do, search your library for an enchantment card, put that card onto the battlefield, then shuffle.", Some((1, 2))),
        card("Seal", "{W}", "Enchantment", "", None),
        card("Lightning Bolt", "{R}", "Instant", "Lightning Bolt deals 3 damage to any target.", None),
        // Synthetic instant return permits the deliberate response to the Rector trigger.
        card("Instant return", "{B}", "Instant", "Return target creature card from your graveyard to your hand.", None),
        card("Plains", "", "Basic Land — Plains", "", None),
    ];
    for d in &defs {
        check(d).unwrap_or_else(|u| panic!("{}: {}", d.name(), u.0));
    }
    Game::new(defs, 2, 20)
}

fn pass_all(g: &mut Game) {
    for _ in 0..g.players.len() {
        g.act(g.priority, Move::Pass).unwrap();
    }
}

/// The Rector has died and its trigger is on the stack. Returns the enchantment in the
/// library and the Rector's card in the graveyard.
fn died() -> (Game, ObjId, ObjId) {
    let mut g = game();
    for _ in 0..3 {
        g.put(0, PLAINS, Zone::Library);
    }
    let seal = g.put(0, SEAL, Zone::Library);
    let rector = g.put(0, RECTOR, Zone::Battlefield);
    g.add_mana(0, ManaSymbol::Color(Color::Red));
    let bolt = g.put(0, BOLT, Zone::Hand);
    g.act(0, paid("R",Move::Cast { card: bolt, targets: vec![vec![Target::Object(rector)]] })).unwrap();
    pass_all(&mut g);
    assert_eq!(g.stack.len(), 1, "the dies trigger");
    let [buried] = g.players[0].graveyard.iter().copied().filter(|o| g.obj(*o).card == Some(RECTOR)).collect::<Vec<_>>()[..] else { panic!("the Rector is in the graveyard") };
    (g, seal, buried)
}

#[test]
fn r118_12_exiling_it_is_what_if_you_do_asks() {
    let (mut g, seal, buried) = died();
    pass_all(&mut g);
    assert_eq!(g.decision, Some(Decision::Choose { player: 0, choice: Choice::May }));
    g.act(0, Move::Choose(Answer::Yes)).unwrap();
    cite!(["118.12"]; assert_eq!(g.obj(buried).zone, Zone::Gone, "exiled before the search"));
    assert_eq!(g.exile.iter().filter(|o| g.obj(**o).card == Some(RECTOR)).count(), 1);
    g.act(0, Move::Choose(Answer::Objects(vec![seal]))).unwrap();
    cite!(["118.12"]; assert!(g.battlefield.iter().any(|o| g.obj(*o).card == Some(SEAL))));
    assert!(g.decision.is_none() && g.resolving().is_none());
}

#[test]
fn r118_12_declining_exiles_nothing_and_does_not_search() {
    let (mut g, seal, buried) = died();
    pass_all(&mut g);
    g.act(0, Move::Choose(Answer::No)).unwrap();
    cite!(["118.12"]; assert!(g.decision.is_none() && g.resolving().is_none(), "no search is offered"));
    assert_eq!(g.obj(buried).zone, Zone::Graveyard);
    cite!(["118.12"]; assert_eq!(g.obj(seal).zone, Zone::Library));
}

#[test]
fn r118_3_a_rector_that_left_the_graveyard_is_not_offered_and_nothing_is_searched() {
    let (mut g, seal, buried) = died();
    // in response, its controller returns it to hand: "it" is no longer there to exile
    g.add_mana(0, ManaSymbol::Color(Color::Black));
    let raise = g.put(0, RAISE, Zone::Hand);
    g.act(0, paid("B",Move::Cast { card: raise, targets: vec![vec![Target::Object(buried)]] })).unwrap();
    pass_all(&mut g);
    assert_eq!(g.obj(buried).zone, Zone::Gone);
    assert_eq!(g.stack.len(), 1, "the trigger still waits");
    pass_all(&mut g);
    cite!(["118.3"]; assert!(g.decision.is_none() && g.resolving().is_none(), "an impossible payment is not offered"));
    assert!(g.stack.is_empty());
    assert!(g.exile.is_empty());
    cite!(["118.12"]; assert_eq!(g.obj(seal).zone, Zone::Library, "nothing was paid: nothing is searched"));
    assert_eq!(g.players[0].hand.iter().filter(|o| g.obj(**o).card == Some(RECTOR)).count(), 1);
}
