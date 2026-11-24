//! Riot (702.136a): "You may have this permanent enter with an additional +1/+1 counter on
//! it. If you don't, it gains haste." A replacement effect its controller chooses with as the
//! permanent enters (614.1c, 614.12a). Every card is read from its Oracle text but one, made
//! here for a ruling no printed card shows.
use crate::support;
use support::*;

#[test]
fn the_cards_of_these_tests_run() {
    for name in ["Zhur-Taa Goblin", "Ghor-Clan Wrecker", "Zombify"] {
        // Uncited (admission): the engine runs each card of these tests.
        assert_eq!(game::play::check(&oracle(name)).err().map(|u| u.0), None, "{name}");
    }
}

/// Cast Zhur-Taa Goblin (2/2, "Riot") and answer its riot choice as it resolves.
fn goblin(counter: bool) -> (T, ObjId) {
    let mut t = Board::new(2).fill(5).hand(0, &["Zhur-Taa Goblin"]).build();
    t.mana(0, "{R}{G}");
    t.cast(0, "Zhur-Taa Goblin", vec![], mana_pool("RG")).unwrap();
    // the choice is made as it enters, as the spell resolves (614.12a)
    let boundary = t.settle();
    cite!(["702.136a", "614.12a"]; assert!(matches!(boundary, Boundary::Question(0, Asked::May)), "{boundary:?}"));
    t.answer(0, if counter { Answer::Yes } else { Answer::No }).unwrap();
    t.resolve();
    let goblin = t.find_in_battlefield("Zhur-Taa Goblin");
    (t, goblin)
}

#[test]
fn r702_136a_it_may_enter_with_an_additional_counter() {
    let (t, goblin) = goblin(true);
    t.expect().rules(["702.136a", "614.1c"]).counters("Zhur-Taa Goblin", "+1/+1", 1).pt("Zhur-Taa Goblin", 3, 3).check();
    cite!(["702.136a"]; assert!(!t.g.has(goblin, "Haste"), "with the counter it does not gain haste"));
}

#[test]
fn r702_136a_if_it_does_not_it_gains_haste_indefinitely() {
    let (mut t, goblin) = goblin(false);
    t.expect().rules(["702.136a"]).counters("Zhur-Taa Goblin", "+1/+1", 0).pt("Zhur-Taa Goblin", 2, 2).check();
    cite!(["702.136a", "702.10b"]; assert!(t.g.has(goblin, "Haste")));
    // Ruling: "If you choose for a creature with riot to gain haste, it gains haste
    // indefinitely. It doesn't lose it as the turn ends."
    t.to_no_combat(1, Part::PrecombatMain);
    t.to_no_combat(0, Part::PrecombatMain);
    cite!(["702.136a", "611.2a"]; assert!(t.g.has(t.find_in_battlefield("Zhur-Taa Goblin"), "Haste")));
}

#[test]
fn r702_136a_it_applies_wherever_the_permanent_enters_from() {
    // Zombify: "Return target creature card from your graveyard to the battlefield."
    let mut t = Board::new(2).graveyard(0, &["Ghor-Clan Wrecker"]).hand(0, &["Zombify"]).build();
    t.mana(0, "{3}{B}");
    t.cast(0, "Zombify", vec!["Ghor-Clan Wrecker".into()], mana_pool("CCCB")).unwrap();
    let boundary = t.settle();
    cite!(["702.136a", "614.12a"]; assert!(matches!(boundary, Boundary::Question(0, Asked::May)), "{boundary:?}"));
    t.answer(0, Answer::Yes).unwrap();
    t.resolve();
    t.expect().rules(["702.136a", "614.1c"]).counters("Ghor-Clan Wrecker", "+1/+1", 1).pt("Ghor-Clan Wrecker", 3, 3).check();
}

#[test]
fn r702_136a_one_that_cannot_have_the_counter_gains_haste() {
    // Ruling: "If a creature entering the battlefield has riot but can't have a +1/+1 counter
    // put onto it, it gains haste."
    let rioter = CardDef::new("Unmarked Rioter", "{R}", "Creature — Goblin", "Riot\nThis creature can't have counters put on it.", Some((1, 1))).unwrap();
    assert_eq!(game::play::check(&rioter).err().map(|u| u.0), None);
    let mut g = Game::new(vec![rioter], 2, 20);
    g.add_mana(0, ir::mana::ManaSymbol::Color(ir::Color::Red));
    let card = g.put(0, 0, Zone::Hand);
    g.act(0, paid("R", Move::Cast { card, targets: vec![] })).unwrap();
    g.act(0, Move::Pass).unwrap();
    g.act(1, Move::Pass).unwrap();
    cite!(["614.12a"]; assert!(matches!(g.asked(), Some((0, Asked::May))), "{:?}", g.asked()));
    g.act(0, Move::Choose(Answer::Yes)).unwrap();
    let [rioter] = g.battlefield[..] else { panic!("it entered: {:?}", g.battlefield) };
    cite!(["702.136a", "122.1"]; assert!(g.counters(rioter).is_empty()));
    cite!(["702.136a"]; assert!(g.has(rioter, "Haste")));
}
