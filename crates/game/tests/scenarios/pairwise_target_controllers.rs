//! Exact two-creature controller inequality, on its full pinned consumer.
//! Expectations use the CR and Run Away Together's same-controller /
//! independently-illegal-member rulings.
use crate::support::{cite, paid};
use game::{CardDef, Game, Move, Target, Zone, Event};
use std::sync::LazyLock;
static FACES: LazyLock<Vec<oracle::corpus::Face>> = LazyLock::new(|| oracle::corpus::load(&oracle::corpus::default_path()).unwrap());
fn native(name: &str) -> CardDef { CardDef::from_faces(&CardDef::catalog(&FACES)[name]).unwrap() }
fn setup(players: usize) -> Game {
    let control = CardDef::new("Control", "{0}", "Instant", "Gain control of target creature.", None).unwrap();
    let mut g = Game::new(vec![native("Run Away Together"), native("Llanowar Elves"), native("Island"), native("Unsummon"), control,
        CardDef::new("Shield", "{0}", "Instant", "Target creature gains hexproof until end of turn.", None).unwrap()], players, 20);
    for p in 0..players { g.players[p].pool.colors = [10; 5]; }
    g
}
fn pass(g: &mut Game) { for _ in 0..g.players.len() { g.act(g.priority, Move::Pass).unwrap(); } }
fn cast(g: &mut Game, seat: usize, def: usize, targets: Vec<Target>, spending: &str) {
    let card = g.put(seat, def, Zone::Hand);
    g.priority = seat;
    g.act(seat, paid(spending, Move::Cast { card, targets: vec![targets] })).unwrap();
}
fn finished(g: &Game) {
    cite!(["608.2b", "608.2d"]; assert!(g.stack.is_empty() && g.decision.is_none() && g.resolving().is_none()));
}

#[test]
fn r601_2c_native_announcement_refusals_are_transactional_and_offers_are_jointly_legal() {
    let mut g = setup(2);
    let a = g.put(0, 1, Zone::Battlefield); let b = g.put(1, 1, Zone::Battlefield);
    let c = g.put(0, 1, Zone::Battlefield); let land = g.put(1, 2, Zone::Battlefield);
    let card = g.put(0, 0, Zone::Hand);
    // Uncited (admission): checks that the complete native definition compiles, not how it plays.
    game::play::check(&g.defs[0]).unwrap();
    for (targets, reason) in [(vec![], "target group"), (vec![vec![]], "wrong number"),
        (vec![vec![a.into()]], "wrong number"), (vec![vec![a.into(), b.into(), c.into()]], "wrong number"),
        (vec![vec![a.into(), a.into()]], "same target twice"), (vec![vec![a.into(), c.into()]], "different controllers"),
        (vec![vec![a.into(), land.into()]], "illegal target"), (vec![vec![a.into(), Target::Player(1)]], "illegal target"),
        (vec![vec![a.into()], vec![b.into()]], "target group")] {
        let before = format!("{g:?}");
        cite!(["115.1a", "115.3", "601.2c"]; assert!(g.act(0, Move::Cast { card, targets }).unwrap_err().contains(reason)));
        // Uncited (transaction contract): includes pools, zones, priority and events.
        assert_eq!(format!("{g:?}"), before);
    }
    g.act(0, paid("WU", Move::Cast { card, targets: vec![vec![a.into(), b.into()]] })).unwrap();
    cite!(["601.2c", "608.2c"]; assert!(g.battlefield.contains(&a) && g.battlefield.contains(&b) && g.decision.is_none()));
    pass(&mut g); finished(&g);
    cite!(["608.2c", "400.7"]; assert!(g.battlefield.contains(&c) && !g.battlefield.contains(&a) && !g.battlefield.contains(&b)));
    cite!(["608.2c"]; assert_eq!((g.players[0].hand.len(), g.players[1].hand.len()), (1,1)));
}

#[test]
fn r601_2c_same_controller_board_has_no_payable_pair() {
    let mut g = setup(2); let a=g.put(0,1,Zone::Battlefield); let b=g.put(0,1,Zone::Battlefield);
    let card = g.put(0,0,Zone::Hand);
    let before = format!("{g:?}");
    cite!(["601.2c"]; refused("different controllers"): g.act(0, paid("WU", Move::Cast { card, targets: vec![vec![a.into(), b.into()]] })));
    assert_eq!(format!("{g:?}"), before);
}

#[test]
fn r608_2b_same_current_controller_makes_both_illegal() {
    for changed in [0,1] {
        let mut g = setup(2); let a = g.put(0,1,Zone::Battlefield); let b = g.put(1,1,Zone::Battlefield);
        cast(&mut g,0,0,vec![a.into(),b.into()], "WU");
        cast(&mut g,1-changed,4,vec![if changed == 0 { a } else { b }.into()], ""); pass(&mut g);
        pass(&mut g); finished(&g);
        cite!(["608.2b"]; assert!(g.battlefield.contains(&a) && g.battlefield.contains(&b)));
        cite!(["608.2b"]; assert!(g.log.iter().any(|e| matches!(e,Event::Fizzled { .. }))));
    }
}

#[test]
fn r608_2b_departed_member_lki_allows_survivor_in_either_order() {
    for departed in [0,1] {
        let mut g = setup(2); let a = g.put(0,1,Zone::Battlefield); let b = g.put(1,1,Zone::Battlefield);
        let targets = [a,b]; cast(&mut g,0,0,targets.iter().copied().map(Into::into).collect(), "WU");
        cast(&mut g,0,3,vec![targets[departed].into()], "U"); pass(&mut g); pass(&mut g); finished(&g);
        cite!(["608.2b", "608.2h"]; assert!(g.battlefield.is_empty()));
        cite!(["608.2c"]; assert_eq!((g.players[0].hand.len(),g.players[1].hand.len()),(1,1)));
    }
}

#[test]
fn r608_2h_departure_lki_is_latest_control_not_announcement_or_owner() {
    for departure_seat in [0,2] {
        let mut g = setup(3); let a = g.put(0,1,Zone::Battlefield); let b = g.put(1,1,Zone::Battlefield);
        cast(&mut g,0,0,vec![a.into(),b.into()], "WU");
        cast(&mut g,departure_seat,4,vec![b.into()], ""); pass(&mut g);
        cast(&mut g,0,3,vec![b.into()], "U"); pass(&mut g);
        cite!(["608.2h"]; assert_eq!(g.controller(b),departure_seat));
        pass(&mut g); finished(&g);
        cite!(["608.2b", "608.2h"]; assert_eq!(g.battlefield.contains(&a),departure_seat == 0));
        cite!(["400.3"]; assert_eq!(g.players[1].hand.len(),1));
    }
}

#[test]
fn r400_7_same_card_reentry_cannot_replace_departed_target_or_its_lki() {
    let mut g = setup(3); let a = g.put(0,1,Zone::Battlefield); let b = g.put(1,1,Zone::Battlefield);
    cast(&mut g,0,0,vec![a.into(),b.into()], "WU"); cast(&mut g,0,3,vec![b.into()], "U"); pass(&mut g);
    let returned = g.players[1].hand[0];
    // Uncited (fixture): move the actual returned card, not a substitute definition.
    g.move_object(returned,Zone::Battlefield); let reentered = *g.battlefield.last().unwrap();
    cast(&mut g,0,4,vec![reentered.into()], ""); pass(&mut g);
    cite!(["400.7"]; assert_ne!(b,reentered));
    pass(&mut g); finished(&g);
    cite!(["400.7", "608.2b", "608.2h"]; assert_eq!(g.battlefield,vec![reentered]));
    cite!(["608.2c"]; assert_eq!(g.players[0].hand.len(),1));
}

#[test]
fn r608_2b_independent_hexproof_still_supplies_current_controller() {
    for same in [false,true] {
        let mut g = setup(2); let a = g.put(0,1,Zone::Battlefield); let b = g.put(1,1,Zone::Battlefield);
        cast(&mut g,0,0,vec![a.into(),b.into()], "WU");
        if same { cast(&mut g,1,4,vec![a.into()], ""); pass(&mut g); }
        cast(&mut g,1,5,vec![b.into()], ""); pass(&mut g); pass(&mut g); finished(&g);
        cite!(["608.2b", "702.11b"]; assert!(g.battlefield.contains(&b)));
        cite!(["608.2b", "608.2h"]; assert_eq!(g.battlefield.contains(&a),same));
    }
}

#[test]
fn r608_2b_both_departed_no_effect_and_no_resolution_choice() {
    let mut g=setup(2); let a=g.put(0,1,Zone::Battlefield);let b=g.put(1,1,Zone::Battlefield);
    cast(&mut g,0,0,vec![a.into(),b.into()], "WU");
    // Uncited (fixture): isolate the all-illegal resolution branch.
    g.move_object(a,Zone::Graveyard);g.move_object(b,Zone::Graveyard);
    pass(&mut g);finished(&g);
    cite!(["608.2b"]; assert!(g.log.iter().any(|e|matches!(e,Event::Fizzled{..}))));
    cite!(["608.2b"]; assert!(g.players.iter().all(|p|p.hand.is_empty())));
}

#[test]
fn r608_2c_multiplayer_pair_does_not_require_caster_and_returns_to_owners() {
    for swap in [false,true] {
        let mut g=setup(3); let a=g.put(0,1,Zone::Battlefield);let b=g.put(1,1,Zone::Battlefield);
        // Set up changed control through a resolving effect before announcement.
        cast(&mut g,2,4,vec![a.into()], "");pass(&mut g);
        cast(&mut g,0,0,if swap {vec![b.into(),a.into()]} else {vec![a.into(),b.into()]}, "WU");
        pass(&mut g);finished(&g);
        cite!(["601.2c", "608.2c"]; assert!(g.battlefield.is_empty()));
        cite!(["400.3"]; assert_eq!(g.players.iter().map(|p|p.hand.len()).collect::<Vec<_>>(),vec![1,1,0]));
    }
}

#[test]
fn excluded_optional_variable_and_other_relations_remain_refused() {
    // Uncited (implementation scope): no approximation of adjacent contracts.
    for text in ["Choose up to two target creatures controlled by different players. Return those creatures to their owners' hands.",
        "Choose two target creatures controlled by the same player. Return those creatures to their owners' hands.",
        "You may choose two target creatures controlled by different players. Return those creatures to their owners' hands."] {
        let card=CardDef::new("Excluded","{0}","Instant",text,None).unwrap();
        let reason=if text.starts_with("You may") {"target declaration: optional instruction or another chooser"}else{"description: Controllers"};
        crate::support::refused(game::play::check(&card).map_err(|e|e.0),reason);
    }
    crate::support::refused(game::play::check(&native("Kitsune, Dragon's Daughter")).map_err(|e|e.0),"program: action not lowered: Exchange");
}
