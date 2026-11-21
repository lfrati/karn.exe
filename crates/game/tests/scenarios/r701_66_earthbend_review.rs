//! Earthbend (701.66a): "return it to the battlefield tapped under your control" is
//! the earthbender's control even if another player controlled the land as it died; a land
//! earthbent twice returns once; a land flickered is exiled, and the return finds nothing
//! (400.7); the haste an effect gives appears in a seat's text (613.1f).
use crate::support::cite;
use game::{CardDef, Game, Move, ObjId, Target, Zone};
use ir::CardType;

const BENDER: usize = 0;
const FOREST: usize = 1;
const STEAL: usize = 2;
const MURDER: usize = 3;
const FLICKER: usize = 4;

fn game() -> Game {
    let c = |name, types, text| CardDef::new(name, "{0}", types, text, None).expect(name);
    let defs = vec![c("Bender", "Sorcery", "Earthbend 2."),
        CardDef::new("Forest", "", "Basic Land — Forest", "", None).unwrap(),
        c("Steal", "Instant", "Gain control of target permanent."),
        c("Murder", "Instant", "Destroy target creature."),
        c("Flicker", "Instant", "Exile target creature, then return it to the battlefield under its owner's control.")];
    for d in &defs { game::play::check(d).unwrap_or_else(|e| panic!("{}: {}", d.name(), e.0)); }
    let mut g = Game::new(defs, 2, 20);
    for p in 0..2 { for _ in 0..5 { g.put(p, FOREST, Zone::Library); } }
    while g.part != ir::Part::PrecombatMain { g.act(g.priority, Move::Pass).unwrap(); }
    g
}
fn settle(g: &mut Game) {
    while !g.stack.is_empty() || g.decision.is_some() {
        match g.decision.clone() {
            Some(game::state::Decision::Order { player, abilities }) => g.act(player, Move::Order((0..abilities.len()).collect())).unwrap(),
            Some(d) => panic!("{d:?}"),
            None => g.act(g.priority, Move::Pass).unwrap(),
        }
    }
}
fn cast(g: &mut Game, player: usize, card: usize, at: ObjId) {
    while g.priority != player { g.act(g.priority, Move::Pass).unwrap(); }
    let spell = g.put(player, card, Zone::Hand);
    g.act(player, Move::Cast { card: spell, targets: vec![vec![Target::Object(at)]] }).unwrap();
    settle(g);
}
fn forests(g: &Game) -> Vec<ObjId> { g.battlefield.iter().copied().filter(|o| g.obj(*o).chars.name == "Forest").collect() }

/// Player 1 steals player 0's earthbent Forest, then it dies: it returns under player 0's
/// control, the "you" of the earthbend.
#[test]
fn it_returns_under_the_earthbenders_control() {
    let mut g = game();
    let land = g.put(0, FOREST, Zone::Battlefield);
    cast(&mut g, 0, BENDER, land);
    cast(&mut g, 1, STEAL, land);
    assert_eq!(g.controller(land), 1);
    cast(&mut g, 1, MURDER, land);
    let back = forests(&g);
    assert_eq!(back.len(), 1);
    cite!(["701.66a"]; assert_eq!(g.controller(back[0]), 0));
    cite!(["701.66a"]; assert!(g.obj(back[0]).tapped));
}

/// Earthbent twice: 4 counters; when it dies both delayed abilities trigger, the first returns
/// it, the second finds the card gone from the graveyard: one Forest.
#[test]
fn earthbent_twice_it_returns_once() {
    let mut g = game();
    let land = g.put(0, FOREST, Zone::Battlefield);
    cast(&mut g, 0, BENDER, land);
    cast(&mut g, 0, BENDER, land);
    cite!(["701.66a"]; assert_eq!((g.power(land), g.toughness(land)), (Some(4), Some(4))));
    cast(&mut g, 0, MURDER, land);
    cite!(["603.7c", "400.7"]; assert_eq!(forests(&g).len(), 1));
    assert_eq!(g.players[0].graveyard.iter().filter(|o| g.obj(**o).chars.name == "Forest").count(), 0);
}

/// Flickered: put into exile, so the delayed ability triggers, but the land is already back
/// as a new object; it stays where it is, untapped, no longer a creature.
#[test]
fn a_flickered_land_is_not_returned_again() {
    let mut g = game();
    let land = g.put(0, FOREST, Zone::Battlefield);
    cast(&mut g, 0, BENDER, land);
    cast(&mut g, 0, FLICKER, land);
    let back = forests(&g);
    cite!(["603.7c", "400.7"]; assert_eq!(back.len(), 1));
    cite!(["400.7"]; assert!(!g.obj(back[0]).tapped && !g.is_a(back[0], CardType::Creature)));
    assert!(g.exile.is_empty());
}

/// A seat reads "Haste" in an earthbent land's text: a keyword an effect gives (613.1f).
#[test]
fn an_earthbent_lands_text_says_haste() {
    let mut g = game();
    let land = g.put(0, FOREST, Zone::Battlefield);
    cast(&mut g, 0, BENDER, land);
    let mut sim = game::simulation::Simulation::new(g);
    for seat in [0, 1] {
        let prompt = sim.prompt(seat).unwrap();
        let text = &prompt.entities[&land].text;
        cite!(["613.1f"]; case: seat; assert_eq!(text.iter().filter(|l| *l == "Haste").count(), 1, "seat {seat}: {text:?}"));
    }
}
