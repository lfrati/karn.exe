//! Intervening-if conditions refer to the player captured by the triggering occurrence.
use crate::support::cite;
use crate::support;
use support::{Board,T,Boundary};
use game::{Move,Zone};
use game::state::{Answer,Choice,Decision};
use game::legal::Asked;

fn board(opponent_lands:usize,hand:&[&str])->T {
    let lands=vec!["Island";opponent_lands];
    let mut t=Board::new(3).active(1).battlefield(0,&["Archaeomancer's Map","Plains"])
        .battlefield(1,&lands).battlefield(2,&["Swamp","Swamp","Swamp","Swamp"])
        .hand(0,hand).hand(1,&["Island"]).build();
    let mark=t.mark();
    let land=t.find_in(1,Zone::Hand,"Island");t.g.act(1,Move::PlayLand(land)).unwrap();
    let entered=t.entered_since(mark,1,Zone::Battlefield,"Island");t.bind("opponent_land",entered);t
}

#[test]
fn r603_4_archaeomancers_map_compares_the_entering_lands_controller_not_any_opponent() {
    for n in [0,1,2] {
        let t=board(n,&["Plains"]);
        cite!(["603.4"]; case: format!("{n} lands"); assert_eq!(t.g.stack.len(),usize::from(n>0),"another opponent's many lands are irrelevant"));
    }
}

#[test]
fn r603_4_the_intervening_condition_is_checked_again_before_resolution() {
    let mut t=board(1,&["Plains"]);assert_eq!(t.g.stack.len(),1);
    let plains=t.g.obj(t.find_in(0,Zone::Hand,"Plains")).card.unwrap();
    t.g.put(0,plains,Zone::Battlefield); // state immediately before the pending trigger resolves
    t.pass_all();cite!(["603.4"]; assert!(t.g.decision.is_none()));assert!(t.g.stack.is_empty());
    cite!(["603.4"]; assert_eq!(t.g.players[0].hand.len(),1));
}

#[test]
fn r603_3_triggered_player_identity_survives_the_entering_lands_departure() {
    for lands in [1, 2] {
        let mut t=board(lands,&["Plains"]);
        let entered=t.id("opponent_land");t.g.move_object(entered,Zone::Graveyard);
        t.pass_all();
        if lands == 1 {
            // Captured player 1 now ties us; player 2 still has four lands. "Any opponent"
            // would incorrectly keep the condition true after the triggering land leaves.
            cite!(["603.4"]; case: format!("{lands} lands"); assert!(t.g.decision.is_none()));assert!(t.g.stack.is_empty());
            t.expect().zone_exact(0,Zone::Hand,&["Plains"]).check();
        } else {
            cite!(["603.4"]; case: format!("{lands} lands"); assert!(matches!(t.g.decision,Some(Decision::Choose{player:0,choice:Choice::OptionalObjects{..}}))));
            let land = t.find_in(0,Zone::Hand,"Plains");
            crate::support::refused(t.answer(1,Answer::OptionalObjects(Some(vec![land]))), "that decision is another player's");
            t.answer(0,Answer::OptionalObjects(Some(vec![land]))).unwrap();
            assert_eq!(t.g.battlefield.iter().filter(|o|t.g.controller(**o)==0).count(),3);
            cite!(["305.4"]; case: format!("{lands} lands"); assert!(!t.g.players[0].land_played,"putting a land is not a land play"));
        }
    }
}

// Rules: 603.4, 608.2d, 614.12a, 701.20b, 402.3
// Given: an opponent has more lands and the controller holds two lands, one requiring a reveal.
// When: accept the optional move with that land, pause at entry, then answer in two clones.
// Expect: private choices, no premature movement/disclosure, independent reveal/decline outcomes.
// Why: resolution owns the entry procedure; revealing is public but does not move the shown card.
#[test]
fn r608_2_private_hand_selection_then_entry_choice_resume_without_replaying_the_trigger() {
    let mut t=board(1,&["Choked Estuary","Island"]);
    let land=t.find_in(0,Zone::Hand,"Choked Estuary"); let island=t.find_in(0,Zone::Hand,"Island");
    t.bind("land",land).bind("reveal",island);
    let Boundary::Question(0,Asked::OptionalObjects{count:1,among,..})=t.settle() else { panic!("expected private optional land selection") };
    cite!(["608.2d"]; assert_eq!(among,vec![land,island]));
    for seat in [1,2] { t.expect().rules(["402.3"]).hidden_from(seat,&["land","reveal"]).observations(seat,&[]).check(); }
    let other=t.id("opponent_land");cite!(["608.2d"]; refused("a chosen object is not among the offered objects to put (608.2d)"): t.answer(0,Answer::OptionalObjects(Some(vec![other]))));
    t.answer(0,Answer::OptionalObjects(Some(vec![land]))).unwrap();
    let Boundary::Question(0,Asked::Select{min:0,max:1,among,..})=t.settle() else { panic!("expected entry reveal selection") };
    cite!(["614.12a"]; assert_eq!(among,vec![island]));
    cite!(["614.12"]; refused("a decision is pending: a choice of how the permanent enters the battlefield (614.12)"): t.g.act(1,Move::Pass));
    for seat in [1,2] { t.expect().rules(["402.3"]).hidden_from(seat,&["land","reveal"]).observations(seat,&[]).rules(["614.12a"]).zone_exact(0,Zone::Hand,&["Choked Estuary","Island"]).check(); }
    for (mut t, reveal) in [(t.clone(), true), (t, false)] {
        t.choose(0,if reveal { &["reveal"] } else { &[] }).unwrap();
        assert!(matches!(t.settle(),Boundary::Priority(1)));
        t.expect().zone_exact(0,Zone::Hand,&["Island"]).stack(&[]).rules(["614.12a"]).tapped("Choked Estuary",!reveal).check();
        for seat in [1,2] { t.expect().rules(["701.20a", "402.3"]).observations(seat,if reveal { &[&["Island"]] } else { &[] }).check(); }
        cite!(["701.20b"]; case: format!("reveal {reveal}"); assert_eq!(t.id("reveal"),island,"a reveal preserves the shown incarnation"));
        cite!(["305.4"]; case: format!("reveal {reveal}"); assert!(!t.g.players[0].land_played));
    }
}

#[test]
fn r608_2_the_optional_move_can_be_declined_and_an_empty_hand_needs_no_choice() {
    let mut t=board(1,&["Plains"]);t.pass_all();t.answer(0,Answer::OptionalObjects(None)).unwrap();
    assert_eq!(t.g.players[0].hand.len(),1);assert!(t.g.stack.is_empty());assert!(t.g.observations(2).is_empty());
    let mut empty=board(1,&[]);empty.pass_all();
    cite!(["608.2d"]; assert!(empty.g.decision.is_none()));assert!(empty.g.stack.is_empty());
}

#[test]
fn r701_23_the_maps_full_entry_ability_finds_only_basic_plains_and_reveals_them() {
    let mut t=Board::new(3).hand(0,&["Archaeomancer's Map"])
        .library(0,&["Plains","Snow-Covered Plains","Tundra","Island"]).build();
    t.mana(0,"{2}{W}").cast(0,"Archaeomancer's Map",vec![], crate::support::mana_pool("CCW")).unwrap().pass_all().pass_all();
    let Some((0,Asked::Search{among,..}))=t.g.asked() else{panic!("{:?}",t.g.asked())};
    cite!(["701.23a"]; assert_eq!(among.len(),2));
    t.choose(0,&["Plains","Snow-Covered Plains"]).unwrap();
    assert_eq!(t.g.players[0].hand.len(),2);assert!(t.g.stack.is_empty());
    let revealed:Vec<_>=t.g.observations(2).iter().flat_map(|d|d.cards.iter().map(|c|c.name.as_str())).collect();
    cite!(["701.20a"]; assert_eq!(revealed.len(),2));assert!(revealed.contains(&"Plains"));assert!(revealed.contains(&"Snow-Covered Plains"));
}
