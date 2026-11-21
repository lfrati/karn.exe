//! Regressions of the test harness itself.
//! The initial-position case follows the pre-SBA contract.
use crate::support::*;
use std::panic::{catch_unwind, AssertUnwindSafe};

// Uncited (harness contract): the panic must identify the authored invalid premise.
fn panics_with<R>(reason: &str, f: impl FnOnce() -> R) {
    let payload = match catch_unwind(AssertUnwindSafe(f)) {
        Ok(_) => panic!("expected harness panic: {reason}"),
        Err(payload) => payload,
    };
    let message = payload.downcast_ref::<String>().map(String::as_str)
        .or_else(|| payload.downcast_ref::<&str>().copied())
        .expect("harness panic has a textual diagnostic");
    assert!(message.contains(reason), "expected {reason:?}, got {message:?}");
}

/// Exact position setup intentionally permits a position before an SBA. It must neither
/// guess attachments/copies nor normalize that state. The position is set up as authored,
/// before the state-based actions it calls for (704.5m, 704.5f).
#[test]
fn an_explicit_unsettled_position_waits_for_the_engines_state_based_check() {
    let mut t=Board::new(2).battlefield(0,&["Pacifism","Clone","Grizzly Bears","Bonesplitter"])
        .hand(0,&["Giant Growth"]).build();
    assert!(matches!(t.settle(),Boundary::Priority(0)));
    t.expect().zone_exact(0,Zone::Battlefield,&["Pacifism","Clone","Grizzly Bears","Bonesplitter"]).check();
    assert!(t.g.log.is_empty());
    t.mana(0,"{G}").cast(0,"Giant Growth",vec![on(0,Zone::Battlefield,"Grizzly Bears")], crate::support::mana_pool("G")).unwrap();
    // Both SBAs happen together; the owner authors their graveyard order (704.3, 404.3).
    t.choose(0,&["Pacifism","Clone"]).unwrap();
    t.expect().zone_exact(0,Zone::Graveyard,&["Pacifism","Clone"])
        .zone_exact(0,Zone::Battlefield,&["Grizzly Bears","Bonesplitter"]).check();
}

/// A label that is also a card name would shadow name lookup in that scope: it is refused.
#[test]
fn a_label_cannot_shadow_a_card_name() {
    panics_with("label Island shadows a card name", || Board::new(2).named(0, Zone::Hand, "Island", "Forest").hand(0, &["Island"]).build());
}

/// Tokens are logged as Created, not ZoneChanged (111.1), so a token made
/// since a mark cannot be bound from the event result.
#[test]
fn a_token_created_since_a_mark_can_be_bound() {
    let mut t = Board::new(2).hand(0, &["Sprout"]).build();
    let mark = t.mark();
    t.mana(0, "{G}").cast(0, "Sprout", vec![], crate::support::mana_pool("G")).unwrap().resolve();
    let created = t.created_since(mark);
    assert_eq!(created.len(), 1);
    let token = created[0];
    t.bind("saproling", token).expect().pt("saproling", 1, 1).check();
    // Coincident copies have separate handles; name lookup stays ambiguous.
    let mut two = Board::new(2).hand(0, &["Raise the Alarm"]).build();
    let mark = two.mark();
    two.mana(0, "{1}{W}").cast(0, "Raise the Alarm", vec![], crate::support::mana_pool("CW")).unwrap().resolve();
    let created=two.created_since(mark);assert_eq!(created.len(),2);assert_ne!(created[0],created[1]);
    let name=two.g.obj(created[0]).chars.name.clone();
    panics_with(&format!("expected exactly one {name} in player 0's Battlefield"), || two.find_in(0,Zone::Battlefield,&name));
}

#[test]
fn counters_target_a_label_among_identical_copies() {
    let t = Board::new(2).named(0, Zone::Battlefield, "big", "Grizzly Bears").battlefield(0, &["Grizzly Bears"])
        .counters("big", "+1/+1", 2).build();
    t.expect().pt("big", 4, 4).check();
    panics_with("expected exactly one Grizzly Bears on the battlefield", || Board::new(2).battlefield(0, &["Grizzly Bears", "Grizzly Bears"]).counters("Grizzly Bears", "+1/+1", 1).build());
}

#[test]
fn activation_by_printed_index_refuses_a_non_activated_ability() {
    let mut t = Board::new(2).battlefield(0, &["Hardened Scales", "Grizzly Bears"]).build();
    let before = format!("{:?}", t.g);
    for (name, index) in [("Hardened Scales", 0), ("Grizzly Bears", 3)] {
        cite!(["602.1c"]; refused("that is not an activated ability of that permanent"): t.activate(0, name, index, vec![], mana_pool("")));
        assert_eq!(format!("{:?}", t.g), before);
    }
}

/// Positive controls: the privacy and observation assertions can fail, and do not drain.
#[test]
fn privacy_assertions_are_not_vacuous_and_observations_are_not_consumed() {
    let t = Board::new(2).named(0, Zone::Hand, "secret", "Island").named(0, Zone::Battlefield, "public", "Forest").build();
    t.expect().hidden_from(1, &["secret"]).check();
    panics_with("player 1 unexpectedly sees", || t.expect().hidden_from(1, &["public"]).check());
    panics_with("player 0 unexpectedly sees", || t.expect().hidden_from(0, &["secret"]).check());
    let mut m = Board::new(3).hand(0, &["Archaeomancer's Map"]).library(0, &["Plains", "Island"]).build();
    let plains = m.find_in(0, Zone::Library, "Plains");
    m.mana(0, "{2}{W}").cast(0, "Archaeomancer's Map", vec![], crate::support::mana_pool("CCW")).unwrap().pass_all().pass_all();
    let Some((0, Asked::Search { among, min: 0, max: 1, .. })) = m.asked() else { panic!("{:?}", m.asked()) };
    assert_eq!(among, vec![plains]);
    m.answer(0, Answer::Objects(vec![plains])).unwrap();
    for _ in 0..2 { m.expect().observations(2, &[&["Plains"]]).check(); }
    panics_with("player 2 observations", || m.expect().observations(2, &[]).check());
}
