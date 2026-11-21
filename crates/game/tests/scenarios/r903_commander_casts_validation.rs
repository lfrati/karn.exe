//! Commander color-identity mana (903.4f) and
//! "If you control a commander, you may cast this spell without paying its mana cost" (118.9,
//! 903.3d). Real cards only.
use crate::support;
use game::announcement::CastCost;
use ir::{Color, mana::ManaSymbol};
use support::*;

const YARGLE: &str = "Yargle, Glutton of Urborg";

/// The commander of player `p`, moved from the command zone onto the battlefield.
fn deploy(t: &mut T, p: usize, name: &str) -> ObjId {
    let c = t.find_in(p, Zone::Command, name);
    t.g.move_object(c, Zone::Battlefield);
    t.find_in(p, Zone::Battlefield, name)
}



fn free(card: ObjId, targets: Vec<Target>) -> Move {
    Move::CastFor(CastCost::Alternative(0), Box::new(Move::Cast { card, targets: targets.into_iter().map(|t| vec![t]).collect() }))
}

#[test]
fn r118_9_the_free_cast_is_optional_and_the_mana_cost_stays_available() {
    let mut t = Board::new(3).commander(0, YARGLE).hand(0, &["Deadly Rollick"]).battlefield(1, &["Grizzly Bears"]).build();
    deploy(&mut t, 0, YARGLE);
    let card = t.find_in(0, Zone::Hand, "Deadly Rollick");
    let target = t.find_in(1, Zone::Battlefield, "Grizzly Bears");
    let mut free_cast=t.g.clone();
    cite!(["118.9"]; ok: free_cast.act(0,free(card,vec![Target::Object(target)])));
    for _ in 0..3 {free_cast.act(free_cast.priority,Move::Pass).unwrap();}
    assert!(free_cast.stack.is_empty());
    assert!(free_cast.exile.iter().any(|o|free_cast.obj(*o).chars.name=="Grizzly Bears"));
    assert_eq!(free_cast.players[0].pool.total(),0);
    cite!(["118.3a"]; refused("that mana is not in your mana pool"): act_unchanged(&mut t.g,0,paid("CCCB",Move::Cast { card, targets: vec![vec![Target::Object(target)]] })));
    t.mana(0, "{3}{B}");
    let mut free_cast=t.g.clone();
    cite!(["118.9"]; ok: free_cast.act(0,free(card,vec![Target::Object(target)])));
    for _ in 0..3 {free_cast.act(free_cast.priority,Move::Pass).unwrap();}
    assert!(free_cast.stack.is_empty());
    assert!(free_cast.exile.iter().any(|o|free_cast.obj(*o).chars.name=="Grizzly Bears"));
    assert_eq!(free_cast.players[0].pool.total(),4);
    t.cast(0, "Deadly Rollick", vec!["Grizzly Bears".into()], crate::support::mana_pool("CCCB")).unwrap();
    assert_eq!(t.g.players[0].pool.total(), 0, "the ordinary cast paid {{3}}{{B}}");
    t.resolve();
    t.expect().contains(1, Zone::Exile, "Grizzly Bears").check();
}

#[test]
fn r903_9a_a_free_rollick_may_exile_the_commander_that_enabled_it() {
    // The condition is checked as the cost is chosen (601.2b, 601.2f); the spell then resolves
    // whatever happens to the commander, and the exiled commander may go to the command zone.
    let mut t = Board::new(3).commander(0, YARGLE).hand(0, &["Deadly Rollick"]).build();
    let leader = deploy(&mut t, 0, YARGLE);
    let card = t.find_in(0, Zone::Hand, "Deadly Rollick");
    t.g.act(0, free(card, vec![Target::Object(leader)])).unwrap();
    t.pass_all();
    cite!(["903.9a"]; assert!(matches!(t.g.decision, Some(Decision::Choose { player: 0, choice: Choice::CommandZone { .. } })), "{:?}", t.g.decision));
    t.answer(0, Answer::Yes).unwrap();
    t.expect().contains(0, Zone::Command, YARGLE).contains(0, Zone::Graveyard, "Deadly Rollick").check();
    assert_eq!(t.g.players[0].pool.total(), 0);
}

#[test]
fn r903_3d_control_of_a_stolen_commander_ends_with_the_turn() {
    let mut t = Board::new(3).commander(1, YARGLE).hand(0, &["Act of Treason", "Deadly Rollick"]).hand(1, &["Deadly Rollick"])
        .battlefield(2, &["Grizzly Bears"]).build();
    let leader = deploy(&mut t, 1, YARGLE);
    let mine = t.find_in(0, Zone::Hand, "Deadly Rollick");
    cite!(["903.3d"]; refused("the condition for that alternative cost is not true"): act_unchanged(&mut t.g,0,free(mine, vec![Target::Object(leader)])));
    t.mana(0, "{2}{R}").cast(0, "Act of Treason", vec![YARGLE.into()], crate::support::mana_pool("CCR")).unwrap();
    t.resolve();
    assert_eq!(t.g.controller(leader), 0);
    cite!(["903.3d"]; ok: t.g.clone().act(0, free(mine, vec![Target::Object(leader)])));
    // the effect ends at the cleanup step (514.2): on player 1's upkeep, player 1 controls it again
    t.to_no_combat(1, Part::Upkeep);
    assert_eq!(t.g.controller(leader), 1);
    let theirs = t.find_in(1, Zone::Hand, "Deadly Rollick");
    cite!(["903.3d"]; ok: t.g.clone().act(1, free(theirs, vec![Target::Object(leader)])));
    t.g.act(1, Move::Pass).unwrap();
    assert_eq!(t.g.priority, 2);
    t.g.act(2, Move::Pass).unwrap();
    // turn order 1, 2, 0: whoever holds priority next, player 0 must not get the free cast
    while t.g.priority != 0 { t.g.act(t.g.priority, Move::Pass).unwrap(); }
    cite!(["903.3d", "514.2"]; refused("the condition for that alternative cost is not true"): act_unchanged(&mut t.g,0,free(mine, vec![Target::Object(leader)])));
}

#[test]
fn r118_9_fierce_guardianship_is_free_on_an_opponents_turn() {
    let mut t = Board::new(2).life(40).commander(1, "Adéwalé, Breaker of Chains").hand(0, &["Lightning Bolt"]).hand(1, &["Fierce Guardianship"]).build();
    deploy(&mut t, 1, "Adéwalé, Breaker of Chains");
    t.mana(0, "{R}").cast(0, "Lightning Bolt", vec![player(1)], crate::support::mana_pool("R")).unwrap();
    t.g.act(0, Move::Pass).unwrap();
    let card = t.find_in(1, Zone::Hand, "Fierce Guardianship");
    let bolt = t.find("Lightning Bolt");
    cite!(["118.9"]; ok: t.g.act(1, free(card, vec![Target::Object(bolt)])));
    t.resolve();
    t.expect().life(1, 40).contains(0, Zone::Graveyard, "Lightning Bolt").contains(1, Zone::Graveyard, "Fierce Guardianship").check();
}

#[test]
fn r903_4f_commanders_sphere_offers_the_identity_and_sacrifices_to_draw() {
    let mut t = Board::new(2).commander(0, "Adéwalé, Breaker of Chains").battlefield(0, &["Commander's Sphere"]).library(0, &["Island"]).build();
    let sphere = t.find_in_battlefield("Commander's Sphere");
    for color in [Color::Blue, Color::Black, Color::White, Color::Red, Color::Green] {
        let action = Move::Adding(vec![vec![ManaSymbol::Color(color)]], Box::new(Move::Activate { source: sphere, ability: 0, targets: vec![], paying: vec![] }));
        cite!(["903.4"]; assert_eq!(t.g.clone().act(0, action).is_ok(), matches!(color, Color::Blue | Color::Black)));
    }
    let hand = t.g.players[0].hand.len();
    t.g.act(0, Move::Activate { source: sphere, ability: 1, targets: vec![], paying: vec![] }).unwrap();
    t.resolve();
    assert_eq!(t.g.players[0].hand.len(), hand + 1);
    t.expect().zone(0, Zone::Hand, &["Island"]).contains(0, Zone::Graveyard, "Commander's Sphere").check();
}

#[track_caller]
fn act_unchanged(g:&mut Game,player:usize,action:Move)->Result<(),String> {
 let before=format!("{g:?}");let result=g.act(player,action);assert_eq!(format!("{g:?}"),before);result
}
