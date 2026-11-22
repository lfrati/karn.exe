//! Tests of the test harness's semantics: fixtures must not manufacture rule evidence.
use crate::support::*;
use std::panic::{catch_unwind, AssertUnwindSafe};

/// An expected refusal is the game's refusal for that reason: a simulation's fault, stale
/// request or wrong seat never satisfies one, even when its message has the reason's words.
#[test]
fn only_the_games_refusal_satisfies_an_expected_refusal() {

    use game::simulation::{ResponseError, Simulation};
    // A land outside a main phase is a rules refusal; an absent decision is not.
    let reason = "305.1";
    let forest = game::CardDef::new("Forest", "", "Basic Land — Forest", "", None).unwrap();
    let mut g = Game::new(vec![forest], 2, 20);
    let card = g.put(0, 0, Zone::Hand);
    g.part = Part::Upkeep;
    let mut sim = Simulation::new(g);
    let r = sim.request().unwrap().unwrap().clone();
    refused(sim.respond(r.id, r.player, Move::PlayLand(card)), reason);
    panics_for("refused, but not for",|| refused(sim.respond(r.id, r.player, Move::PlayLand(card)), "the game is over"));
    panics_for("not NoDecision", || refused(sim.respond(r.id, r.player, Move::Choose(Answer::Yes)), "no decision is pending"));
    let stale = sim.respond(game::simulation::RequestId(r.id.0 + 1), r.player, Move::Pass);
    assert!(matches!(stale, Err(ResponseError::Stale { .. })));
    panics_for("not Stale",|| refused(stale, "request"));
    let other = sim.respond(r.id, 1 - r.player, Move::Pass);
    assert!(matches!(other, Err(ResponseError::WrongPlayer { .. })));
    panics_for("not WrongPlayer",|| refused(other, "request"));
    let mut failed = sim.clone();
    let _ = failed.observe(|_| -> () { panic!("{reason}") });
    let fault = failed.respond(r.id, r.player, Move::Choose(Answer::Yes));
    assert!(matches!(&fault, Err(ResponseError::Fault(f)) if f.message.contains(reason)), "{fault:?}");
    // the cited form is proved in tooling/cite_fixtures.rs, where its failing receipt credits
    // no rule
    panics_for("not Fault",|| refused(fault, reason));
}

#[test]
fn fixture_counters_are_exact_and_emit_no_actions() {
    let t=Board::new(2).active(1).battlefield(0,&["Hardened Scales","Spike Feeder","Jace Beleren"])
        .counters("Spike Feeder","+1/+1",2).build();
    t.expect().counters("Spike Feeder","+1/+1",2).counters("Jace Beleren","loyalty",0).check();
    assert!(t.g.log.is_empty()); assert_eq!((t.g.active,t.g.priority),(1,1));
    let naked=Board::new(2).battlefield(0,&["Spike Feeder"]).build();
    naked.expect().counters("Spike Feeder","+1/+1",0).pt("Spike Feeder",0,0).check();
}

#[test]
fn setup_does_not_perform_entry_and_unresolved_value_choices_are_rejected() {

    let t=Board::new(2).battlefield(1,&["Authority of the Consuls"])
        .battlefield(0,&["Grizzly Bears"]).tapped(0,&["Choked Estuary"]).build();
    t.expect().tapped("Grizzly Bears",false).tapped("Choked Estuary",true).life(1,20).check();
    assert!(t.g.log.is_empty()); assert!(t.g.stack.is_empty());
    panics_for("Voice of All needs an established entry choice",|| Board::new(2).battlefield(0,&["Voice of All"]).build());
}

#[test]
fn labels_distinguish_copies_and_ambiguous_names_never_pick_one() {

    let t=Board::new(2).named(0,Zone::Battlefield,"mine","Grizzly Bears")
        .named(1,Zone::Battlefield,"theirs","Grizzly Bears").build();
    assert_ne!(t.id("mine"),t.id("theirs"));
    panics_for("expected exactly one Grizzly Bears",|| t.find("Grizzly Bears"));
    assert_eq!(t.find_in(1,Zone::Battlefield,"Grizzly Bears"),t.id("theirs"));
    let copies=Board::new(2).battlefield(0,&["Island","Island"]).build();
    copies.expect().zone_exact(0,Zone::Battlefield,&["Island","Island"]).check();
    panics_for("expected exactly one Island in player 0's Battlefield",|| copies.find_in(0,Zone::Battlefield,"Island"));
}

#[test]
fn departed_labels_are_stale_for_actions_and_current_assertions() {

    let mut t=Board::new(2).named(0,Zone::Hand,"bear_card","Grizzly Bears").build();
    let old=t.id("bear_card");let mark=t.mark();
    t.mana(0,"{1}{G}").cast(0,"bear_card",vec![], crate::support::mana_pool("CG")).unwrap();t.resolve();
    panics_for("stale incarnation",|| t.id("bear_card"));
    panics_for("stale incarnation",|| t.expect().tapped("bear_card",false).check());
    let new=t.entered_since(mark,0,Zone::Battlefield,"Grizzly Bears");assert_ne!(old,new);
    t.bind("bear",new);t.expect().tapped("bear",false).check();
    let clone=t.clone();assert_eq!(clone.id("bear"),new);
    t.g.log.clear();
    panics_for("event range must still be retained",|| t.entered_since(mark,0,Zone::Battlefield,"Grizzly Bears"));
}

#[test]
fn an_empty_stack_does_not_hide_an_entry_question() {

    let mut t=Board::new(2).hand(0,&["Temple Garden"]).build();
    let card=t.find_in(0,Zone::Hand,"Temple Garden");t.g.act(0,Move::PlayLand(card)).unwrap();
    assert!(t.g.stack.is_empty());
    assert!(matches!(t.settle(),Boundary::Question(0,Asked::May)));
    panics_for("unexpected resolution boundary Question(0, May)",|| { t.resolve(); });
    t.answer(0,Answer::No).unwrap();assert!(matches!(t.settle(),Boundary::Priority(0)));
    t.expect().tapped("Temple Garden",true).check();
}

#[test]
fn actual_loss_is_never_suppressed_and_must_be_expected() {

    let mut t=Board::new(2).hand(0,&["Divination"]).build();
    t.mana(0,"{2}{U}").cast(0,"Divination",vec![], crate::support::mana_pool("CCU")).unwrap();
    let mut expected=t.clone();expected.expect_losses(&[0]);
    panics_for("unexpected eliminated players [0]",|| t.settle());
    assert!(t.g.players[0].lost,"the driver did not prevent the actual loss");
    assert!(matches!(expected.settle(),Boundary::Ended(Some(1))));expected.expect().check();
}

#[test]
fn exact_order_is_distinct_from_membership_and_multiset() {

    let t=Board::new(2).library(0,&["Island","Forest","Island"]).build();
    t.expect().zone_exact(0,Zone::Library,&["Forest","Island","Island"])
        .zone_ordered(0,Zone::Library,&["Island","Forest","Island"]).check();
    panics_for("player 0 Library order",|| t.expect().zone_ordered(0,Zone::Library,&["Forest","Island","Island"]).check());
}

#[test]
fn created_tokens_have_individual_result_handles_without_zone_changes() {
    let mut t=Board::new(2).hand(0,&["Raise the Alarm"]).build();
    let mark=t.mark();t.mana(0,"{1}{W}").cast(0,"Raise the Alarm",vec![], crate::support::mana_pool("CW")).unwrap();t.resolve();
    let tokens=t.created_since(mark);assert_eq!(tokens.len(),2);assert_ne!(tokens[0],tokens[1]);
    t.bind("first",tokens[0]).bind("second",tokens[1]);
    t.expect().pt("first",1,1).pt("second",1,1).check();
}

#[test]
fn stack_scopes_use_controller_and_global_stack_remains_available() {

    let mut t = Board::new(2).hand(0, &["Lightning Bolt"]).build();
    let mark = t.mark();
    t.mana(0, "{R}").cast(0, "Lightning Bolt", vec![player(1)], crate::support::mana_pool("R")).unwrap();
    let spell = t.find_in(0, Zone::Stack, "Lightning Bolt");
    t.bind("spell", spell);
    assert_eq!(t.entered_since(mark, 0, Zone::Stack, "Lightning Bolt"), spell);
    t.expect().zone_exact(0, Zone::Stack, &["Lightning Bolt"])
        .zone_exact(1, Zone::Stack, &[]).stack(&["Lightning Bolt"]).check();
    for name in ["Lightning Bolt", "spell"] {
        panics_for(if name=="spell" {"label spell is not in player 1's Stack"} else {"expected exactly one Lightning Bolt in player 1's Stack"},|| t.find_in(1, Zone::Stack, name));
    }
    panics_for("expected exactly one Lightning Bolt entering player 1's Stack",|| t.entered_since(mark, 1, Zone::Stack, "Lightning Bolt"));
    panics_for("player 1 Stack is []",|| t.expect().zone_exact(1, Zone::Stack, &["Lightning Bolt"]).check());
}

/// These harness tests require the intended assertion, not merely any unwind.
#[track_caller]
fn panics_for<R>(reason:&str,action:impl FnOnce()->R){
    let payload=match catch_unwind(AssertUnwindSafe(action)){Ok(_)=>panic!("expected assertion: {reason}"),Err(payload)=>payload};
    let message=payload.downcast_ref::<String>().map(String::as_str).or_else(||payload.downcast_ref::<&str>().copied()).expect("text assertion payload");
    assert!(message.contains(reason),"expected {reason:?}, got {message:?}");
}
