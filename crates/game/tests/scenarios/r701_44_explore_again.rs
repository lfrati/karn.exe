//! "It explores, then it explores again" (Jadelight Ranger, 701.44a): two explores, one after
//! the other, the second seeing the library the first left. Each is its own explore event
//! (701.44d), so "whenever this creature explores" triggers twice. "Again" restates an act
//! the sentence did before; nothing else reads it.
use crate::support::cite;
use game::state::{Answer, Choice, Decision};
use game::{CardDef, Event, Game, Move, ObjId, Zone};

const RANGER: usize = 0;
const LAND: usize = 1;
const SPELL: usize = 2;
const WALKER: usize = 3;

fn game() -> Game {
    let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let real = |name: &str| CardDef::from_face(faces.iter().find(|f| f.name == name).unwrap()).unwrap();
    let c = |name, types, text, pt| CardDef::new(name, "{0}", types, text, pt).expect(name);
    let defs = vec![
        real("Jadelight Ranger"),
        c("Land", "Basic Land — Forest", "", None),
        c("Spell", "Instant", "You gain 1 life.", None),
        real("Wildgrowth Walker"),
    ];
    for d in &defs { game::play::check(d).unwrap_or_else(|e| panic!("{}: {}", d.name(), e.0)); }
    Game::new(defs, 2, 20)
}
fn explored(g: &Game) -> usize { g.log.iter().filter(|e| matches!(e, Event::Explored { .. })).count() }
fn plus(g: &Game, o: ObjId) -> u32 { g.obj(o).counters.get("+1/+1").copied().unwrap_or(0) }
fn in_hand(g: &Game, card: usize) -> usize { g.players[0].hand.iter().filter(|o| g.obj(**o).card == Some(card)).count() }
/// Cast the ranger and resolve everything, answering each "may" (put the card in the
/// graveyard) with `mill`.
fn ranger(g: &mut Game, mill: bool) -> ObjId {
    let card = g.put(0, RANGER, Zone::Hand);
    g.players[0].pool.colors[4] = 2;
    g.players[0].pool.colorless = 1;
    g.act(0, crate::support::paid("CGG", Move::Cast { card, targets: vec![] })).unwrap();
    for _ in 0..40 {
        match g.decision.clone() {
            Some(Decision::Choose { player: 0, choice: Choice::May }) => g.act(0, Move::Choose(if mill { Answer::Yes } else { Answer::No })).unwrap(),
            Some(d) => panic!("{d:?}"),
            None if g.stack.is_empty() => break,
            None => g.act(g.priority, Move::Pass).unwrap(),
        }
    }
    *g.battlefield.iter().find(|o| g.obj(**o).card == Some(RANGER)).unwrap()
}

#[test]
fn r701_44a_two_lands_both_go_to_hand() {
    let mut g = game();
    g.put(0, LAND, Zone::Library);
    g.put(0, LAND, Zone::Library);
    let r = ranger(&mut g, false);
    cite!(["701.44a", "608.2c"]; assert_eq!((in_hand(&g, LAND), plus(&g, r), explored(&g)), (2, 0, 2)));
}

#[test]
fn r701_44a_a_kept_nonland_is_seen_again_by_the_second_explore() {
    let mut g = game();
    g.put(0, LAND, Zone::Library);
    let spell = g.put(0, SPELL, Zone::Library);
    let r = ranger(&mut g, false);
    // the spell stays on top, so each explore reveals it and gives a counter
    cite!(["701.44a", "608.2c"]; assert_eq!((plus(&g, r), explored(&g)), (2, 2)));
    cite!(["701.44a"]; assert_eq!(g.players[0].library.last(), Some(&spell)));
    assert_eq!(in_hand(&g, LAND), 0);
}

#[test]
fn r701_44a_a_milled_nonland_uncovers_a_land_for_the_second() {
    let mut g = game();
    g.put(0, LAND, Zone::Library);
    g.put(0, SPELL, Zone::Library);
    let r = ranger(&mut g, true);
    cite!(["701.44a", "608.2c"]; assert_eq!((plus(&g, r), in_hand(&g, LAND), explored(&g)), (1, 1, 2)));
    cite!(["701.44a"]; assert_eq!(g.players[0].graveyard.iter().filter(|o| g.obj(**o).card == Some(SPELL)).count(), 1));
}

#[test]
fn r701_44d_each_explore_triggers_separately() {
    let mut g = game();
    g.put(0, WALKER, Zone::Battlefield);
    g.put(0, LAND, Zone::Library);
    g.put(0, LAND, Zone::Library);
    ranger(&mut g, false);
    // Wildgrowth Walker: "Whenever a creature you control explores, put a +1/+1 counter on
    // this creature and you gain 3 life."
    cite!(["701.44b", "603.2"]; assert_eq!(g.players[0].life, 26));
}

#[test]
fn again_needs_an_act_it_restates() {
    for text in ["Draw a card again.", "You gain 1 life, then draw a card again."] {
        let def = CardDef::new("Again", "{0}", "Instant", text, None).unwrap();
        crate::support::refused(game::play::check(&def).map_err(|e|e.0),"modifier: Again of Draw");
    }
}
