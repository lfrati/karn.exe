//! Deck compositions: upkeep sacrifice outcomes (118.12), entry triggers (603.6a),
//! and recipients/quantities locked by resolving continuous effects (611.2c, 608.2h).
use crate::support::cite;
use game::{CardDef, Game, Move, ObjId, Zone};
use game::state::{Answer, Choice, Decision};
use ir::Part;

const MONUMENT: usize = 0;
const CHOSEN: usize = 1;
const HOOF: usize = 2;
const BEAR: usize = 3;
const LAND: usize = 4;
fn game() -> Game {
    let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let catalog = CardDef::catalog(&faces);
    let mut defs: Vec<_> = ["Eldrazi Monument", "Nissa's Chosen", "Craterhoof Behemoth"].iter()
        .map(|name| CardDef::from_faces(catalog.get(*name).unwrap()).unwrap()).collect();
    defs.push(CardDef::new("Bear", "{0}", "Creature — Bear", "", Some((2, 2))).unwrap());
    defs.push(CardDef::new("Forest", "", "Basic Land — Forest", "", None).unwrap());
    for d in &defs { game::play::check(d).unwrap(); }
    let mut g = Game::new(defs, 3, 20);
    for p in 0..3 { for _ in 0..8 { g.put(p, LAND, Zone::Library); } }
    g
}
fn pass(g: &mut Game) { for _ in 0..3 { g.act(g.priority, Move::Pass).unwrap(); } }
fn advance(g: &mut Game) {
    match g.decision {
        Some(Decision::Attackers { player }) => g.act(player, Move::Attack(vec![])).unwrap(),
        _ => g.act(g.priority, Move::Pass).unwrap(),
    }
}
fn upkeep(g: &mut Game, player: usize) {
    for _ in 0..200 {
        if g.part == Part::Upkeep && g.active == player { return; }
        advance(g);
    }
    panic!("upkeep not reached");
}
#[test]
fn r118_12_replaced_sacrifice_satisfies_monument_upkeep_and_clones_keep_the_choice() {
    let mut g = game();
    let monument = g.put(0, MONUMENT, Zone::Battlefield);
    let chosen = g.put(0, CHOSEN, Zone::Battlefield);
    let bear = g.put(0, BEAR, Zone::Battlefield);
    let opponent = g.put(1, BEAR, Zone::Battlefield);
    assert_eq!(g.power(chosen), Some(3));
    assert!(g.has(chosen, "Flying") && g.has(chosen, "Indestructible"));
    assert!(!g.has(opponent, "Indestructible"));
    upkeep(&mut g, 0); pass(&mut g);
    assert!(matches!(g.decision, Some(Decision::Choose { player:0, choice:Choice::Objects { .. } })));
    crate::support::refused(g.act(1, Move::Choose(Answer::Objects(vec![chosen]))), "that decision is another player's");
    for (cards, why) in [(vec![], "choose exactly 1 of them to sacrifice (608.2d)"), (vec![opponent], "a chosen object is not among the offered objects to sacrifice (608.2d)")] {
        cite!(["608.2d"]; case: why; refused(why): g.act(0, Move::Choose(Answer::Objects(cards))));
    }
    let mut clone = g.clone();
    g.act(0, Move::Choose(Answer::Objects(vec![chosen]))).unwrap();
    clone.act(0, Move::Choose(Answer::Objects(vec![bear]))).unwrap();
    cite!(["118.12"]; assert!(g.battlefield.contains(&monument) && clone.battlefield.contains(&monument)));
    assert!(g.battlefield.contains(&bear));
    cite!(["614.1a"]; assert_eq!(g.obj(g.players[0].library[0]).card, Some(CHOSEN)));
    assert!(g.players[0].graveyard.is_empty());
    assert_eq!(clone.players[0].graveyard.len(), 1);
    assert!(clone.battlefield.contains(&chosen));
}
#[test]
fn r118_12_monument_sacrifices_itself_when_no_creature_can_be_sacrificed() {
    let mut g = game(); let monument = g.put(0, MONUMENT, Zone::Battlefield);
    g.put(1, BEAR, Zone::Battlefield);
    upkeep(&mut g, 0); pass(&mut g);
    cite!(["118.12", "701.21a"]; assert!(!g.battlefield.contains(&monument)));
    assert_eq!(g.players[0].graveyard.len(), 1);
    assert_eq!(g.obj(g.players[0].graveyard[0]).card, Some(MONUMENT));
    assert!(g.decision.is_none());
}
fn cast_hoof(g: &mut Game) -> ObjId {
    let card = g.put(0, HOOF, Zone::Hand); g.players[0].pool.colors[4] = 8;
    g.act(0, crate::support::paid("GGGGGGGG", Move::Cast { card, targets: vec![] })).unwrap(); pass(g);
    *g.battlefield.iter().find(|o| g.obj(**o).card == Some(HOOF)).unwrap()
}
#[test]
fn r611_2c_craterhoof_counts_at_resolution_and_retains_the_team_until_cleanup() {
    let mut g = game(); let bear = g.put(0, BEAR, Zone::Battlefield);
    let opponent = g.put(1, BEAR, Zone::Battlefield); let hoof = cast_hoof(&mut g);
    assert!(g.has(hoof, "Haste"));
    let added = g.put(0, BEAR, Zone::Battlefield); pass(&mut g);
    for o in [bear, added] { cite!(["608.2h"]; assert_eq!(g.power(o), Some(5))); assert!(g.has(o, "Trample")); }
    assert_eq!(g.power(hoof), Some(8)); assert_eq!(g.power(opponent), Some(2));
    let late = g.put(0, BEAR, Zone::Battlefield);
    cite!(["611.2c"]; assert_eq!(g.power(late), Some(2))); assert!(!g.has(late, "Trample"));
    g.exile(added); cite!(["608.2h"]; assert_eq!(g.power(bear), Some(5)));
    let turn = g.turn;
    for _ in 0..100 { if g.turn != turn { break; } advance(&mut g); }
    assert_ne!(g.turn, turn);
    cite!(["514.2"]; assert_eq!(g.power(bear), Some(2))); assert_eq!(g.power(hoof), Some(5));
    assert!(!g.has(bear, "Trample"));
}
#[test]
fn r113_7a_craterhoof_trigger_survives_departure_and_counts_the_remaining_creatures() {
    let mut g = game(); let bear = g.put(0, BEAR, Zone::Battlefield); let hoof = cast_hoof(&mut g);
    let mut clone = g.clone(); g.exile(hoof); pass(&mut g); pass(&mut clone);
    cite!(["113.7a", "608.2h"]; assert_eq!(g.power(bear), Some(3))); assert_eq!(clone.power(bear), Some(4));
    assert!(g.has(bear, "Trample") && clone.has(bear, "Trample"));
}
