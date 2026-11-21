//! The worked examples of rule 603 (handling triggered abilities), with real cards.
use crate::support;
use support::*;

/// 603.6b, first example: "All lands are creatures" (Living Lands: "All Forests are 1/1
/// creatures that are still lands") makes a played Forest a creature as it enters, so it
/// triggers "Whenever another creature enters" (Soul Warden).
#[test]
fn r603_6b_example_a_land_played_under_living_lands_enters_as_a_creature() {
    let mut t = Board::new(2).battlefield(0, &["Living Lands", "Soul Warden"]).hand(0, &["Forest"]).build();
    let forest = t.find_in(0, Zone::Hand, "Forest");
    t.g.act(0, Move::PlayLand(forest)).unwrap();
    t.resolve();
    t.expect().rules(["603.6b"]).life(0, 21).pt("Forest", 1, 1).check();
}

/// 603.6b, second example: "All creatures lose all abilities" (Humility) removes a creature's
/// enters-the-battlefield ability as it enters (Elvish Visionary: "When this creature enters,
/// draw a card"), so it doesn't trigger.
#[test]
fn r603_6b_example_humility_removes_an_enters_ability_before_it_could_trigger() {
    let mut t = Board::new(2).battlefield(0, &["Humility"]).hand(0, &["Elvish Visionary"]).library(0, &["Forest"]).build();
    t.mana(0, "{1}{G}").cast(0, "Elvish Visionary", vec![], crate::support::mana_pool("CG")).unwrap();
    t.resolve();
    t.expect().rules(["603.6b"]).zone_exact(0, Zone::Hand, &[]).zone_exact(0, Zone::Library, &["Forest"])
        .pt("Elvish Visionary", 1, 1).stack(&[]).check();
}

/// 603.2e: "becomes tapped" triggers only when a permanent already on the battlefield goes
/// from untapped to tapped. Goblin Medics ("Whenever this creature becomes tapped, it deals 1
/// damage to any target") entering tapped under Authority of the Consuls ("Creatures your
/// opponents control enter tapped. Whenever a creature an opponent controls enters, you gain 1
/// life.") doesn't trigger; another Goblin Medics tapped by Icy
/// Manipulator does.
// Rules: 603.2e, 603.6b, 614.1c
// Given: one established Medics and a second copy in hand, under an opponent's entry effect.
// When: cast the new copy; separately tap the old one with an activated ability.
// Expect: only the second action produces the Medics target question; entry gains 1 life.
// Why: entering tapped is not a transition from an untapped battlefield object to tapped.
#[test]
fn r603_2e_example_entering_tapped_is_not_becoming_tapped() {
    let mut t = Board::new(2).battlefield(1, &["Authority of the Consuls"])
        .battlefield(0, &["Icy Manipulator"])
        .named(0, Zone::Battlefield, "old_medic", "Goblin Medics")
        .named(0, Zone::Hand, "new_card", "Goblin Medics").build();
    let mark = t.mark();
    t.mana(0, "{2}{R}").cast(0, "new_card", vec![], crate::support::mana_pool("CCR")).unwrap();
    t.resolve();
    let new = t.entered_since(mark, 0, Zone::Battlefield, "Goblin Medics");
    t.bind("new_medic", new);
    t.expect().zone_exact(0, Zone::Battlefield, &["Icy Manipulator", "Goblin Medics", "Goblin Medics"])
        .tapped("old_medic", false).life(1, 21).rules(["603.2e"]).tapped("new_medic", true).stack(&[]).check();
    t.mana(0, "{1}").activate(0, "Icy Manipulator", 0, vec!["old_medic".into()], crate::support::mana_pool("C")).unwrap();
    let Boundary::Question(0, Asked::Targets { source, slots }) = t.settle() else { panic!("expected Medics target question") };
    cite!(["603.2e"]; assert_eq!(source, Some(t.id("old_medic"))));
    assert_eq!(slots.len(), 1);
    t.g.act(0, Move::ChooseTargets(vec![vec![Target::Player(1)]])).unwrap();
    t.resolve();
    t.expect().life(1, 20).check();
}
