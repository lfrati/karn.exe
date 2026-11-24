//! Unleash (702.98a): "You may have this permanent enter with an additional +1/+1 counter on
//! it" and "This permanent can't block as long as it has a +1/+1 counter on it." Every card
//! is read from its Oracle text.
use crate::support;
use support::*;

#[test]
fn the_cards_of_these_tests_run() {
    for name in ["Rakdos Cackler", "Zombify", "Mosquito Guard", "Llanowar Elves"] {
        // Uncited (admission): the engine runs each card of these tests.
        assert_eq!(game::play::check(&oracle(name)).err().map(|u| u.0), None, "{name}");
    }
}

/// Cast Rakdos Cackler (1/1, "Unleash") and answer its unleash choice as it resolves.
fn cackler(t: &mut T, counter: bool) -> ObjId {
    t.mana(0, "{B}");
    t.cast(0, "Rakdos Cackler", vec![], mana_pool("B")).unwrap();
    let boundary = t.settle();
    cite!(["702.98a", "614.12a"]; assert!(matches!(boundary, Boundary::Question(0, Asked::May)), "{boundary:?}"));
    t.answer(0, if counter { Answer::Yes } else { Answer::No }).unwrap();
    t.resolve();
    t.find_in_battlefield("Rakdos Cackler")
}

/// Player 1 attacks player 0 with its Grizzly Bears; player 0, who has a Llanowar Elves
/// that could block, is to declare blockers.
fn to_blockers(t: &mut T) -> ObjId {
    t.to_no_combat(1, Part::BeginningOfCombat);
    let bears = t.find_in(1, Zone::Battlefield, "Grizzly Bears");
    while t.g.decision != Some(Decision::Attackers { player: 1 }) { t.g.act(t.g.priority, Move::Pass).unwrap(); }
    t.g.act(1, Move::Attack(vec![(bears, Target::Player(0))])).unwrap();
    while t.g.decision != Some(Decision::Blockers { player: 0 }) { t.g.act(t.g.priority, Move::Pass).unwrap(); }
    bears
}

#[test]
fn r702_98a_with_the_counter_it_cannot_block() {
    let mut t = Board::new(2).fill(5).hand(0, &["Rakdos Cackler"]).battlefield(0, &["Llanowar Elves"]).battlefield(1, &["Grizzly Bears"]).build();
    let cackler = cackler(&mut t, true);
    t.expect().rules(["702.98a", "614.1c"]).counters("Rakdos Cackler", "+1/+1", 1).pt("Rakdos Cackler", 2, 2).check();
    let bears = to_blockers(&mut t);
    cite!(["702.98a", "509.1b"]; refused("can't block"): t.g.act(0, Move::Block(vec![(cackler, bears)])));
}

#[test]
fn r702_98a_without_it_it_can_block() {
    let mut t = Board::new(2).fill(5).hand(0, &["Rakdos Cackler"]).battlefield(0, &["Llanowar Elves"]).battlefield(1, &["Grizzly Bears"]).build();
    let cackler = cackler(&mut t, false);
    t.expect().rules(["702.98a"]).counters("Rakdos Cackler", "+1/+1", 0).pt("Rakdos Cackler", 1, 1).check();
    let bears = to_blockers(&mut t);
    cite!(["702.98a", "509.1a"]; ok: t.g.act(0, Move::Block(vec![(cackler, bears)])));
}

#[test]
fn r702_98a_any_counter_keeps_it_from_blocking() {
    // Ruling: "A creature with unleash can't block if it has any +1/+1 counter on it, not
    // just one put on it by the unleash ability." Mosquito Guard: "Reinforce 1—{1}{W}"
    let mut t = Board::new(2).fill(5).hand(0, &["Rakdos Cackler", "Mosquito Guard"]).battlefield(0, &["Llanowar Elves"]).battlefield(1, &["Grizzly Bears"]).build();
    let cackler = cackler(&mut t, false);
    let guard = t.find_in(0, Zone::Hand, "Mosquito Guard");
    let ability = t.g.abilities(guard).iter().position(|a| matches!(&**a, ir::Ability::Activated { .. })).expect("reinforce");
    t.mana(0, "{1}{W}");
    t.g.act(0, paid("CW", Move::Activate { source: guard, ability, targets: vec![vec![Target::Object(cackler)]], paying: vec![] })).unwrap();
    t.resolve();
    t.expect().rules(["702.77a"]).counters("Rakdos Cackler", "+1/+1", 1).check();
    let bears = to_blockers(&mut t);
    cite!(["702.98a", "509.1b"]; refused("can't block"): t.g.act(0, Move::Block(vec![(cackler, bears)])));
}

#[test]
fn r702_98a_it_applies_wherever_the_permanent_enters_from() {
    // Ruling: "The unleash ability applies no matter where the creature is entering the
    // battlefield from." Zombify: "Return target creature card from your graveyard to the
    // battlefield."
    let mut t = Board::new(2).graveyard(0, &["Rakdos Cackler"]).hand(0, &["Zombify"]).build();
    t.mana(0, "{3}{B}");
    t.cast(0, "Zombify", vec!["Rakdos Cackler".into()], mana_pool("CCCB")).unwrap();
    let boundary = t.settle();
    cite!(["702.98a", "614.12a"]; assert!(matches!(boundary, Boundary::Question(0, Asked::May)), "{boundary:?}"));
    t.answer(0, Answer::Yes).unwrap();
    t.resolve();
    t.expect().rules(["702.98a", "614.1c"]).counters("Rakdos Cackler", "+1/+1", 1).pt("Rakdos Cackler", 2, 2).check();
}
