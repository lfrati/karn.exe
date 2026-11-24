//! Escalate (702.120a): an additional cost paid once for each mode chosen beyond the first,
//! following the rules for paying additional costs (601.2f-h). Its cost may be an action:
//! Collective Brutality's discard and Collective Effort's tap of a creature.
//! Every card is read from its Oracle text.
use crate::support;
use support::*;

const BRUTALITY: &str = "Collective Brutality";
const EFFORT: &str = "Collective Effort";

fn modal(modes: &[usize], mv: Move) -> Move { Move::Modes(modes.to_vec(), Box::new(mv)) }
fn costs(t: &T, labels: &[&[&str]], mv: Move) -> Move {
    Move::WithCosts(labels.iter().map(|group| group.iter().map(|l| t.id(l)).collect()).collect(), Box::new(mv))
}

/// Player 0 has Collective Brutality and two cards to discard; player 1 has two instants and a
/// creature card in hand and a Grizzly Bears on the battlefield.
fn brutality() -> T {
    Board::new(2).named(0, Zone::Hand, "brutality", BRUTALITY)
        .named(0, Zone::Hand, "island", "Island").named(0, Zone::Hand, "forest", "Forest")
        .named(1, Zone::Hand, "bolt", "Lightning Bolt").named(1, Zone::Hand, "shock", "Shock").named(1, Zone::Hand, "bear card", "Grizzly Bears")
        .named(1, Zone::Battlefield, "bear", "Grizzly Bears").build()
}

fn all_three(t: &T) -> Move {
    Move::Cast { card: t.id("brutality"), targets: vec![vec![Target::Player(1)], vec![Target::Object(t.id("bear"))], vec![Target::Player(1)]] }
}

#[test]
fn the_cards_of_these_tests_run() {
    for name in [BRUTALITY, EFFORT] {
        // Uncited (admission): the engine runs each card of these tests.
        assert_eq!(game::play::check(&oracle(name)).err().map(|u| u.0), None, "{name}");
    }
}

#[test]
fn r702_120a_three_modes_discard_two_cards() {
    // "Escalate—Discard a card. Choose one or more — • Target opponent reveals their hand. You
    // choose an instant or sorcery card from it. That player discards that card. • Target
    // creature gets -2/-2 until end of turn. • Target opponent loses 2 life and you gain 2 life."
    let mut t = brutality();
    let cast = modal(&[0, 1, 2], all_three(&t));
    let cast = costs(&t, &[&["island"], &["forest"]], cast);
    t.mana(0, "{1}{B}");
    cite!(["702.120a", "601.2f", "601.2h"]; ok: t.g.act(0, paid("CB", cast)));
    t.expect().rules(["702.120a", "601.2h"])
        .zone_exact(0, Zone::Graveyard, &["Island", "Forest"]).zone_exact(0, Zone::Hand, &[]).stack(&[BRUTALITY]).check();
    t.pass_all();
    let asked = t.asked();
    cite!(["608.2d"]; assert!(matches!(asked, Some((0, Asked::Objects { count: 1, ref among, .. })) if among.len() == 2), "the caster chooses one of the two instants: {asked:?}"));
    t.choose(0, &["Shock"]).unwrap();
    t.resolve();
    t.expect().rules(["700.2", "608.2c"])
        .zone_exact(0, Zone::Graveyard, &["Island", "Forest", BRUTALITY])
        .zone_exact(1, Zone::Graveyard, &["Shock", "Grizzly Bears"])
        .zone_exact(1, Zone::Hand, &["Lightning Bolt", "Grizzly Bears"])
        .life(0, 22).life(1, 18).stack(&[]).check();
}

#[test]
fn r702_120a_each_mode_beyond_the_first_needs_its_own_discard() {
    let mut t = brutality();
    t.mana(0, "{1}{B}");
    let before = t.state();
    // three modes and one card discarded: the total cost is two discards (601.2f)
    let one = costs(&t, &[&["island"]], modal(&[0, 1, 2], all_three(&t)));
    cite!(["702.120a", "601.2h"]; refused("that selection does not satisfy the cost, which asks for exactly 1 card(s): 0 chosen (601.2h, 701.9a)"):
        t.g.act(0, paid("CB", one)));
    // and the same card can't be discarded twice
    let twice = costs(&t, &[&["island"], &["island"]], modal(&[0, 1, 2], all_three(&t)));
    cite!(["702.120a", "118.10"]; refused("Island pays for one thing (118.10)"): t.g.act(0, paid("CB", twice)));
    // Uncited (implementation contract): a refused cast changes nothing.
    assert_eq!(t.state(), before);
}

#[test]
fn r702_120a_one_mode_pays_no_escalate_cost() {
    let mut t = brutality();
    let cast = modal(&[2], Move::Cast { card: t.id("brutality"), targets: vec![vec![Target::Player(1)]] });
    t.mana(0, "{1}{B}");
    cite!(["702.120a"]; ok: t.g.act(0, paid("CB", cast)));
    t.expect().rules(["702.120a"]).zone_exact(0, Zone::Hand, &["Island", "Forest"]).zone_exact(0, Zone::Graveyard, &[]).check();
    t.resolve();
    t.expect().life(0, 22).life(1, 18).zone_exact(0, Zone::Hand, &["Island", "Forest"]).check();
}

#[test]
fn r702_120a_two_modes_discard_one_card() {
    let mut t = brutality();
    let cast = modal(&[1, 2], Move::Cast { card: t.id("brutality"), targets: vec![vec![Target::Object(t.id("bear"))], vec![Target::Player(1)]] });
    let cast = costs(&t, &[&["forest"]], cast);
    t.mana(0, "{1}{B}");
    cite!(["702.120a"]; ok: t.g.act(0, paid("CB", cast)));
    t.resolve();
    t.expect().rules(["702.120a", "704.5f"])
        .zone_exact(0, Zone::Hand, &["Island"]).zone_exact(0, Zone::Graveyard, &["Forest", BRUTALITY])
        .zone_exact(1, Zone::Graveyard, &["Grizzly Bears"]).zone_exact(1, Zone::Hand, &["Lightning Bolt", "Shock", "Grizzly Bears"])
        .life(0, 22).life(1, 18).check();
}

/// Collective Effort: "Escalate—Tap an untapped creature you control. Choose one or more —
/// • Destroy target creature with power 4 or greater. • Destroy target enchantment. • Put a
/// +1/+1 counter on each creature target player controls."
fn effort() -> T {
    Board::new(2).named(0, Zone::Hand, "effort", EFFORT).named(0, Zone::Hand, "bears card", "Grizzly Bears")
        .named(0, Zone::Battlefield, "tapped", "Llanowar Elves")
        .named(1, Zone::Battlefield, "anthem", "Glorious Anthem").build()
}

#[test]
fn r702_120a_a_creature_that_just_arrived_is_tapped_for_escalate() {
    // "You can tap any untapped creature you control to pay the escalate cost, including one
    // you haven't controlled continuously since the beginning of the turn." (ruling)
    let mut t = effort();
    t.g.set_tapped(t.id("tapped"), true);
    t.mana(0, "{1}{G}").cast(0, "bears card", vec![], mana_pool("CG")).unwrap();
    t.resolve();
    let bears = t.find_in_battlefield("Grizzly Bears");
    t.bind("bears", bears);
    let cast = modal(&[1, 2], Move::Cast { card: t.id("effort"), targets: vec![vec![Target::Object(t.id("anthem"))], vec![Target::Player(0)]] });
    // an already tapped creature can't be tapped to pay it
    let tapped = costs(&t, &[&["tapped"]], cast.clone());
    t.mana(0, "{1}{W}{W}");
    cite!(["702.120a", "601.2h"]; refused("Llanowar Elves is not what that cost asks for or cannot pay it (701.26, 118.3)"): t.g.act(0, paid("CWW", tapped)));
    let with_bears = costs(&t, &[&["bears"]], cast);
    cite!(["702.120a", "601.2h"]; ok: t.g.act(0, paid("CWW", with_bears)));
    t.expect().rules(["702.120a"]).tapped("Grizzly Bears", true).check();
    t.resolve();
    t.expect().rules(["700.2", "608.2c"])
        .zone_exact(1, Zone::Graveyard, &["Glorious Anthem"])
        .counters("Grizzly Bears", "+1/+1", 1).counters("Llanowar Elves", "+1/+1", 1).check();
}

#[test]
fn r702_120a_two_modes_beyond_the_first_tap_two_creatures() {
    let mut t = Board::new(2).named(0, Zone::Hand, "effort", EFFORT)
        .named(0, Zone::Battlefield, "a", "Grizzly Bears").named(0, Zone::Battlefield, "b", "Llanowar Elves")
        .named(1, Zone::Battlefield, "anthem", "Glorious Anthem").named(1, Zone::Battlefield, "wurm", "Craw Wurm").build();
    let cast = modal(&[0, 1, 2], Move::Cast { card: t.id("effort"), targets: vec![vec![Target::Object(t.id("wurm"))], vec![Target::Object(t.id("anthem"))], vec![Target::Player(0)]] });
    let one = costs(&t, &[&["a"]], cast.clone());
    t.mana(0, "{1}{W}{W}");
    cite!(["702.120a", "601.2h"]; refused("that selection does not satisfy the cost, which asks for exactly 1 permanent(s): 0 chosen (601.2h, 701.26)"):
        t.g.act(0, paid("CWW", one)));
    let both = costs(&t, &[&["a"], &["b"]], cast);
    cite!(["702.120a", "601.2h"]; ok: t.g.act(0, paid("CWW", both)));
    t.resolve();
    t.expect().rules(["702.120a", "700.2"]).tapped("Grizzly Bears", true).tapped("Llanowar Elves", true)
        .zone_exact(1, Zone::Graveyard, &["Craw Wurm", "Glorious Anthem"]).zone_exact(1, Zone::Battlefield, &[])
        .counters("Grizzly Bears", "+1/+1", 1).counters("Llanowar Elves", "+1/+1", 1).check();
}
