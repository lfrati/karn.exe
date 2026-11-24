//! Soulbond (702.95a): two triggered abilities. One pairs this creature, as it enters, with
//! another unpaired creature its controller controls; the other pairs another creature that
//! enters under that player's control with this one, while it is unpaired. A creature is
//! paired with one other only (702.95d), and the two stay paired for as long as both remain
//! creatures on the battlefield under that player's control (702.95e). Every card is read
//! from its Oracle text.
use crate::support;
use support::*;

const PALADIN: &str = "Silverblade Paladin";
const BEARS: &str = "Grizzly Bears";
const ELVES: &str = "Llanowar Elves";

/// Cast a creature card from player 0's hand and let it resolve: what its entering
/// triggers waits on the stack.
fn enter(t: &mut T, name: &str, cost: &str, pool: &str) {
    t.mana(0, cost).cast(0, name, vec![], mana_pool(pool)).unwrap();
    t.pass_all();
}

/// Resolve the soulbond ability on the stack, answering its "you may".
fn pair(t: &mut T, yes: bool) {
    let boundary = t.settle();
    cite!(["702.95a"]; assert!(matches!(boundary, Boundary::Question(0, Asked::May)), "the controller may pair them: {boundary:?}"));
    t.answer(0, if yes { Answer::Yes } else { Answer::No }).unwrap();
    t.resolve();
}

fn double_strike(t: &T, name: &str) -> bool { t.g.has(t.find_in_battlefield(name), "Double Strike") }

/// Silverblade Paladin enters, and is paired with the Grizzly Bears player 0 controls.
fn paired() -> T {
    let mut t = Board::new(2).fill(10).hand(0, &[PALADIN]).battlefield(0, &[BEARS]).hand(0, &[ELVES, "Shock"]).hand(1, &["Act of Treason"]).build();
    enter(&mut t, PALADIN, "{1}{W}{W}", "CWW");
    pair(&mut t, true);
    t
}

#[test]
fn the_cards_of_these_tests_run() {
    for name in [PALADIN, "Wolfir Silverheart", "Trusted Forcemage"] {
        // Uncited (admission): the engine runs each card of these tests.
        assert_eq!(game::play::check(&oracle(name)).err().map(|u| u.0), None, "{name}");
    }
}

#[test]
fn r702_95a_entering_it_pairs_with_another_creature_you_control() {
    // Silverblade Paladin: "Soulbond. As long as this creature is paired with another
    // creature, both creatures have double strike."
    let mut t = Board::new(2).hand(0, &[PALADIN]).battlefield(0, &[BEARS]).build();
    cite!(["702.95b"]; assert!(!double_strike(&t, BEARS)));
    enter(&mut t, PALADIN, "{1}{W}{W}", "CWW");
    t.expect().rules(["702.95a", "603.2"]).stack(&[PALADIN]).check();
    cite!(["702.95b"]; assert!(!double_strike(&t, PALADIN), "not paired before its ability resolves"));
    pair(&mut t, true);
    cite!(["702.95a", "702.95b"]; assert!(double_strike(&t, PALADIN) && double_strike(&t, BEARS), "both creatures have double strike"));
}

#[test]
fn r702_95a_a_creature_entering_pairs_with_it() {
    let mut t = Board::new(2).battlefield(0, &[PALADIN]).hand(0, &[BEARS]).build();
    enter(&mut t, BEARS, "{1}{G}", "CG");
    t.expect().rules(["702.95a", "603.2"]).stack(&[PALADIN]).check();
    pair(&mut t, true);
    cite!(["702.95a", "702.95b"]; assert!(double_strike(&t, PALADIN) && double_strike(&t, BEARS)));
}

#[test]
fn r702_95a_with_no_other_unpaired_creature_it_does_not_trigger() {
    // "When this creature enters, if you control both this creature and another creature and
    // both are unpaired, ..." (an intervening "if", 603.4)
    let mut t = Board::new(2).hand(0, &[PALADIN]).battlefield(1, &[BEARS]).build();
    enter(&mut t, PALADIN, "{1}{W}{W}", "CWW");
    t.expect().rules(["702.95a", "603.4"]).stack(&[]).check();
    cite!(["702.95a"]; assert!(!double_strike(&t, PALADIN) && !double_strike(&t, BEARS), "an opponent's creature is not paired with it"));
}

#[test]
fn r702_95a_declined_both_stay_unpaired_and_it_may_pair_later() {
    let mut t = Board::new(2).hand(0, &[PALADIN, ELVES]).battlefield(0, &[BEARS]).build();
    enter(&mut t, PALADIN, "{1}{W}{W}", "CWW");
    pair(&mut t, false);
    cite!(["702.95a"]; assert!(!double_strike(&t, PALADIN) && !double_strike(&t, BEARS)));
    // still unpaired, it pairs with the next creature that enters
    enter(&mut t, ELVES, "{G}", "G");
    t.expect().rules(["702.95a"]).stack(&[PALADIN]).check();
    pair(&mut t, true);
    cite!(["702.95a", "702.95b"]; assert!(double_strike(&t, PALADIN) && double_strike(&t, ELVES) && !double_strike(&t, BEARS)));
}

#[test]
fn r702_95d_a_paired_creature_is_paired_with_no_other() {
    let mut t = paired();
    enter(&mut t, ELVES, "{G}", "G");
    // the second ability's "if ... both are unpaired" is false (603.4)
    t.expect().rules(["702.95d", "603.4"]).stack(&[]).check();
    cite!(["702.95d"]; assert!(double_strike(&t, PALADIN) && double_strike(&t, BEARS) && !double_strike(&t, ELVES)));
}

#[test]
fn r702_95e_a_creature_that_leaves_the_battlefield_unpairs_the_other() {
    let mut t = paired();
    cite!(["702.95b"]; assert!(double_strike(&t, PALADIN)));
    let bears = t.find_in_battlefield(BEARS);
    t.mana(0, "{R}").cast(0, "Shock", vec![Tgt::Object(bears)], mana_pool("R")).unwrap();
    t.resolve();
    t.expect().rules(["704.5g"]).zone_exact(0, Zone::Graveyard, &["Shock", BEARS]).check();
    cite!(["702.95e"]; assert!(!double_strike(&t, PALADIN), "unpaired once the Bears left"));
    // unpaired, it is paired again with the next creature that enters
    enter(&mut t, ELVES, "{G}", "G");
    t.expect().rules(["702.95e", "702.95a"]).stack(&[PALADIN]).check();
    pair(&mut t, true);
    cite!(["702.95a"]; assert!(double_strike(&t, PALADIN) && double_strike(&t, ELVES)));
}

#[test]
fn r702_95e_another_player_gaining_control_unpairs_them_for_good() {
    // Act of Treason: "Gain control of target creature until end of turn. Untap that
    // creature. It gains haste until end of turn."
    let mut t = paired();
    t.to_no_combat(1, Part::PrecombatMain);
    let bears = t.find_in_battlefield(BEARS);
    t.mana(1, "{2}{R}").cast(1, "Act of Treason", vec![Tgt::Object(bears)], mana_pool("CCR")).unwrap();
    t.resolve();
    cite!(["702.95e"]; assert!(!double_strike(&t, PALADIN) && !double_strike(&t, BEARS), "another player gained control of the Bears"));
    // control of the Bears returns as the turn ends (514.2); the pair stays broken
    t.to_no_combat(0, Part::PrecombatMain);
    t.expect().rules(["514.2"]).holds(|g| g.controller(bears) == 0, "the Bears are player 0's again").check();
    cite!(["702.95e"]; assert!(!double_strike(&t, PALADIN) && !double_strike(&t, BEARS), "unpaired for good"));
}

#[test]
fn r702_95c_a_creature_gone_as_it_resolves_is_not_paired() {
    // Shock the Bears in response: the pair it would make is not made (702.95c)
    let mut t = Board::new(2).hand(0, &[PALADIN, "Shock"]).battlefield(0, &[BEARS]).hand(0, &[ELVES]).build();
    enter(&mut t, PALADIN, "{1}{W}{W}", "CWW");
    t.expect().stack(&[PALADIN]).check();
    let bears = t.find_in_battlefield(BEARS);
    t.mana(0, "{R}").cast(0, "Shock", vec![Tgt::Object(bears)], mana_pool("R")).unwrap();
    t.pass_all();
    t.expect().rules(["704.5g"]).zone_exact(0, Zone::Graveyard, &["Shock", BEARS]).stack(&[PALADIN]).check();
    // "if you control both this creature and another creature and both are unpaired" is now
    // false: the ability does nothing (603.4)
    t.resolve();
    cite!(["702.95c", "603.4"]; assert!(!double_strike(&t, PALADIN)));
    // and it is still unpaired
    enter(&mut t, ELVES, "{G}", "G");
    pair(&mut t, true);
    cite!(["702.95a"]; assert!(double_strike(&t, PALADIN) && double_strike(&t, ELVES)));
}

#[test]
fn r702_95b_each_of_those_creatures_is_this_one_and_the_one_it_is_paired_with() {
    // Wolfir Silverheart: "As long as this creature is paired with another creature, each of
    // those creatures gets +4/+4."
    let mut t = Board::new(2).hand(0, &["Wolfir Silverheart"]).battlefield(0, &[BEARS, ELVES]).build();
    enter(&mut t, "Wolfir Silverheart", "{3}{G}{G}", "CCCGG");
    let boundary = t.settle();
    cite!(["702.95a"]; assert!(matches!(boundary, Boundary::Question(0, Asked::May)), "{boundary:?}"));
    t.answer(0, Answer::Yes).unwrap();
    // the controller chooses which of the two (608.2d)
    let asked = t.asked();
    cite!(["702.95a", "608.2d"]; assert!(matches!(asked, Some((0, Asked::Objects { count: 1, ref among, .. })) if among.len() == 2), "{asked:?}"));
    t.choose(0, &[ELVES]).unwrap();
    t.resolve();
    t.expect().rules(["702.95b", "613.4c"]).pt("Wolfir Silverheart", 8, 8).pt(ELVES, 5, 5).pt(BEARS, 2, 2).check();
}

#[test]
fn r702_95e_the_pair_breaks_as_a_creature_is_exiled_and_is_made_again_as_it_returns() {
    // Deadeye Navigator: "As long as Deadeye Navigator is paired with another creature, each
    // of those creatures has "{1}{U}: Exile this creature, then return it to the battlefield
    // under your control."" Ruling: "the creature will be exiled, the pair will immediately be
    // broken, and then the card will be returned to the battlefield. Deadeye Navigator's
    // soulbond ability triggers when that card enters the battlefield and the pair can then be
    // reunited."
    let mut t = Board::new(2).hand(0, &["Deadeye Navigator"]).battlefield(0, &[BEARS]).build();
    enter(&mut t, "Deadeye Navigator", "{4}{U}{U}", "CCCCUU");
    pair(&mut t, true);
    let bears = t.find_in_battlefield(BEARS);
    let blink = t.g.abilities(bears).iter().position(|a| matches!(&**a, ir::Ability::Activated { .. }));
    cite!(["702.95b", "613.1f"]; assert!(blink.is_some(), "the Bears have the ability while paired"));
    t.mana(0, "{1}{U}");
    cite!(["602.2"]; ok: t.g.act(0, paid("CU", Move::Activate { source: bears, ability: blink.unwrap(), targets: vec![], paying: vec![] })));
    t.pass_all();
    // the returned Bears are a new object, unpaired (400.7, 702.95e); the Navigator's second
    // ability triggers for it
    t.expect().rules(["702.95e", "702.95a"]).stack(&["Deadeye Navigator"]).check();
    let returned = t.find_in_battlefield(BEARS);
    cite!(["400.7", "702.95e"]; assert!(returned != bears && !t.g.abilities(returned).iter().any(|a| matches!(&**a, ir::Ability::Activated { .. })),
        "the returned Bears are not paired"));
    pair(&mut t, true);
    cite!(["702.95a"]; assert!(t.g.abilities(returned).iter().any(|a| matches!(&**a, ir::Ability::Activated { .. })), "paired again"));
}

#[test]
fn r702_95e_a_creature_unpaired_loses_the_bonus_at_once() {
    // Druid's Familiar: "each of those creatures gets +2/+2." Ruling: "If Druid's Familiar
    // becomes unpaired, it will immediately lose the +2/+2 bonus. If this causes it to have
    // damage marked on it equal to it or greater than its new toughness, it will be destroyed.
    // The same is true for the creature it was paired with."
    let mut t = Board::new(2).hand(0, &["Druid's Familiar", "Lightning Bolt", "Unsummon"]).battlefield(0, &[BEARS]).build();
    enter(&mut t, "Druid's Familiar", "{3}{G}", "CCCG");
    pair(&mut t, true);
    t.expect().rules(["702.95b"]).pt(BEARS, 4, 4).pt("Druid's Familiar", 4, 4).check();
    let bears = t.find_in_battlefield(BEARS);
    t.mana(0, "{R}").cast(0, "Lightning Bolt", vec![Tgt::Object(bears)], mana_pool("R")).unwrap();
    t.resolve();
    t.expect().rules(["704.5g"]).zone_exact(0, Zone::Battlefield, &[BEARS, "Druid's Familiar"]).check();
    let familiar = t.find_in_battlefield("Druid's Familiar");
    t.mana(0, "{U}").cast(0, "Unsummon", vec![Tgt::Object(familiar)], mana_pool("U")).unwrap();
    t.resolve();
    // the Bears are 2/2 again with 3 damage marked on them
    t.expect().rules(["702.95e", "704.5g"]).zone_exact(0, Zone::Battlefield, &[])
        .zone_exact(0, Zone::Graveyard, &["Lightning Bolt", "Unsummon", BEARS]).zone_exact(0, Zone::Hand, &["Druid's Familiar"]).check();
}

#[test]
fn r603_10a_a_paired_creature_that_dies_had_the_ability_it_was_given() {
    // Doom Weaver: "each of those creatures has "When this creature dies, draw cards equal to
    // its power."" Ruling: "Use the creature's power as it last existed on the battlefield."
    let mut t = Board::new(2).fill(5).hand(0, &["Doom Weaver", "Lightning Bolt"]).battlefield(0, &[BEARS]).build();
    enter(&mut t, "Doom Weaver", "{4}{B}{B}", "CCCCBB");
    pair(&mut t, true);
    let bears = t.find_in_battlefield(BEARS);
    t.mana(0, "{R}").cast(0, "Lightning Bolt", vec![Tgt::Object(bears)], mana_pool("R")).unwrap();
    t.pass_all();
    // it died paired, with the ability: it triggers (603.10a)
    t.expect().rules(["603.10a", "702.95e"]).stack(&[BEARS]).zone_exact(0, Zone::Hand, &[]).check();
    t.resolve();
    t.expect().rules(["603.10a", "608.2h"]).holds(|g| g.players[0].hand.len() == 2, "two cards drawn, the Bears' power").check();
}
