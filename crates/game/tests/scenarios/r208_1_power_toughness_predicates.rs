//! Power and toughness as predicates of a thing (208.1: "2/3 means the object has power 2 and
//! toughness 3"). A "1/1 creature" is one whose power is 1 and whose toughness is 1, now; its
//! "total power and toughness" is their sum. The cards the grammar reads this way and the
//! engine runs, on their Oracle text and rulings.
use crate::support::{Board, Boundary, T, cite, mana_pool, on};
use game::state::Answer;
use game::{Move, Target, Zone};
use ir::Part;

fn pt(t: &T, name: &str) -> (Option<i32>, Option<i32>) {
    let o = t.find(name);
    (t.g.power(o), t.g.toughness(o))
}

/// Cut Down: "Destroy target creature with total power and toughness 5 or less." Ruling of
/// 2022-09-09: "the total power and toughness of a 3/2 creature is 5".
#[test]
fn r208_1_cut_down_takes_a_creature_whose_power_and_toughness_total_five_or_less() {
    let mut t = Board::new(2).battlefield(1, &["Balduvian Barbarians", "Hill Giant"]).hand(0, &["Cut Down"]).build();
    t.mana(0, "{B}");
    // a 3/3 totals six
    cite!(["115.1", "208.1"]; refused("an illegal target, Hill Giant"): t.cast(0, "Cut Down", vec![on(1, Zone::Battlefield, "Hill Giant")], mana_pool("B")).map(|_| ()));
    let barbarians = t.find("Balduvian Barbarians");
    t.cast(0, "Cut Down", vec![on(1, Zone::Battlefield, "Balduvian Barbarians")], mana_pool("B")).unwrap();
    t.resolve();
    cite!(["701.8a", "208.1"]; assert!(!t.g.battlefield.contains(&barbarians) && t.g.players[1].graveyard.len() == 1));
    assert_eq!(t.g.battlefield.len(), 1, "the Giant stays");
}

#[test]
fn r608_2b_cut_down_does_nothing_to_a_creature_that_outgrew_it() {
    let mut t = Board::new(2).battlefield(1, &["Grizzly Bears"]).hand(0, &["Cut Down"]).hand(1, &["Giant Growth"]).build();
    t.mana(0, "{B}").mana(1, "{G}");
    let bears = t.find("Grizzly Bears");
    t.cast(0, "Cut Down", vec![on(1, Zone::Battlefield, "Grizzly Bears")], mana_pool("B")).unwrap();
    // in response its controller makes it 5/5: ten in total
    t.g.act(0, Move::Pass).unwrap();
    t.cast(1, "Giant Growth", vec![on(1, Zone::Battlefield, "Grizzly Bears")], mana_pool("G")).unwrap();
    t.resolve();
    cite!(["608.2b"]; assert!(t.g.battlefield.contains(&bears), "its target is no longer legal as it would resolve"));
    assert_eq!(pt(&t, "Grizzly Bears"), (Some(5), Some(5)));
    cite!(["608.2b"]; assert_eq!(t.g.players[0].graveyard.len(), 1, "the spell left the stack"));
}

/// Pendelhaven: "{T}: Target 1/1 creature gets +1/+2 until end of turn." Ruling of 2004-10-04:
/// being 1/1 is a targeting requirement, "checked when announcing and again right before
/// resolving".
#[test]
fn r115_1_pendelhaven_targets_a_creature_that_is_one_one_and_only_that() {
    const PUMP: usize = 1;
    // a 2/2, a 2/1 and a 1/2: each number must be 1
    let mut t = Board::new(2).battlefield(0, &["Pendelhaven", "Llanowar Elves", "Grizzly Bears", "Savannah Lions", "Squire"]).build();
    for other in ["Grizzly Bears", "Savannah Lions", "Squire"] {
        cite!(["115.1", "208.1"]; case: other; refused(&format!("an illegal target, {other}")): t.activate(0, "Pendelhaven", PUMP, vec![on(0, Zone::Battlefield, other)], mana_pool("")).map(|_| ()));
    }
    t.activate(0, "Pendelhaven", PUMP, vec![on(0, Zone::Battlefield, "Llanowar Elves")], mana_pool("")).unwrap();
    t.resolve();
    cite!(["208.1", "611.2a"]; assert_eq!(pt(&t, "Llanowar Elves"), (Some(2), Some(3))));
}

#[test]
fn r608_2b_pendelhaven_does_nothing_to_a_creature_that_is_no_longer_one_one() {
    let mut t = Board::new(2).battlefield(0, &["Pendelhaven", "Llanowar Elves"]).hand(0, &["Giant Growth"]).build();
    t.mana(0, "{G}");
    t.activate(0, "Pendelhaven", 1, vec![on(0, Zone::Battlefield, "Llanowar Elves")], mana_pool("")).unwrap();
    t.cast(0, "Giant Growth", vec![on(0, Zone::Battlefield, "Llanowar Elves")], mana_pool("G")).unwrap();
    t.resolve();
    cite!(["608.2b"]; assert_eq!(pt(&t, "Llanowar Elves"), (Some(4), Some(4)), "only the Growth applied"));
}

/// Pendelhaven Elder: "{T}: Each 1/1 creature you control gets +1/+2 until end of turn."
/// Rulings of 2021-03-19: it affects only the 1/1 creatures you control as it resolves, by
/// their actual power and toughness.
#[test]
fn r611_2c_pendelhaven_elder_raises_each_one_one_creature_its_controller_controls() {
    let mut t = Board::new(2).battlefield(0, &["Pendelhaven Elder", "Llanowar Elves", "Grizzly Bears", "Squire"]).battlefield(1, &["Raging Goblin"]).build();
    t.activate(0, "Pendelhaven Elder", 0, vec![], mana_pool("")).unwrap();
    t.resolve();
    // the Elder is 1/1 itself; a 2/2, a 1/2 and an opponent's 1/1 are not among them
    cite!(["611.2c", "208.1"]; assert_eq!(
        ["Pendelhaven Elder", "Llanowar Elves", "Grizzly Bears", "Squire", "Raging Goblin"].map(|name| pt(&t, name)),
        [(Some(2), Some(3)), (Some(2), Some(3)), (Some(2), Some(2)), (Some(1), Some(2)), (Some(1), Some(1))]
    ));

    // by its actual power and toughness: an Elf printed 1/1 that is 4/4 now is not one
    let mut t = Board::new(2).battlefield(0, &["Pendelhaven Elder", "Llanowar Elves"]).hand(0, &["Giant Growth"]).build();
    t.mana(0, "{G}");
    t.cast(0, "Giant Growth", vec![on(0, Zone::Battlefield, "Llanowar Elves")], mana_pool("G")).unwrap();
    t.resolve();
    t.activate(0, "Pendelhaven Elder", 0, vec![], mana_pool("")).unwrap();
    t.resolve();
    cite!(["611.2c", "208.1"]; assert_eq!(pt(&t, "Llanowar Elves"), (Some(4), Some(4))));
}

/// Aegis of the Meek: "{1}, {T}: Target 1/1 creature gets +1/+2 until end of turn."
#[test]
fn r115_1_aegis_of_the_meek_targets_a_one_one_creature() {
    let mut t = Board::new(2).battlefield(0, &["Aegis of the Meek", "Llanowar Elves", "Grizzly Bears"]).build();
    t.mana(0, "{1}");
    cite!(["115.1", "208.1"]; refused("an illegal target, Grizzly Bears"): t.activate(0, "Aegis of the Meek", 0, vec![on(0, Zone::Battlefield, "Grizzly Bears")], mana_pool("{1}")).map(|_| ()));
    t.activate(0, "Aegis of the Meek", 0, vec![on(0, Zone::Battlefield, "Llanowar Elves")], mana_pool("{1}")).unwrap();
    t.resolve();
    cite!(["208.1", "611.2a"]; assert_eq!(pt(&t, "Llanowar Elves"), (Some(2), Some(3))));
}

/// Lovestruck Beast: "This creature can't attack unless you control a 1/1 creature." An
/// opponent's is not one "you control". Rulings of 2019-10-04: the 1/1 need not attack, and
/// the Beast reduced to 1/1 is itself one.
#[test]
fn r508_1c_lovestruck_beast_attacks_only_while_its_controller_controls_a_one_one_creature() {
    let to_attackers = |t: &mut T| {
        t.pass_all();
        t.pass_all();
        assert_eq!(t.g.part, Part::DeclareAttackers);
    };
    // no 1/1 creature of its controller's: a 2/2 of theirs and an opponent's 1/1 are none
    let mut t = Board::new(2).battlefield(0, &["Lovestruck Beast", "Grizzly Bears"]).battlefield(1, &["Raging Goblin"]).build();
    let beast = t.find("Lovestruck Beast");
    to_attackers(&mut t);
    cite!(["508.1c"]; refused("Lovestruck Beast can't attack that (508.1c)"): t.g.act(0, Move::Attack(vec![(beast, Target::Player(1))])));
    // with one, the Beast attacks alone
    let mut t = Board::new(2).battlefield(0, &["Lovestruck Beast", "Llanowar Elves"]).build();
    let beast = t.find("Lovestruck Beast");
    to_attackers(&mut t);
    cite!(["508.1c"]; ok: t.g.act(0, Move::Attack(vec![(beast, Target::Player(1))])));
    // a 5/5 given -4/-4 is a 1/1 creature its controller controls
    let mut t = Board::new(2).battlefield(0, &["Lovestruck Beast"]).hand(0, &["Grasp of Darkness"]).build();
    t.mana(0, "{B}{B}");
    t.cast(0, "Grasp of Darkness", vec![on(0, Zone::Battlefield, "Lovestruck Beast")], mana_pool("BB")).unwrap();
    t.resolve();
    assert_eq!(pt(&t, "Lovestruck Beast"), (Some(1), Some(1)));
    let beast = t.find("Lovestruck Beast");
    to_attackers(&mut t);
    cite!(["508.1c"]; ok: t.g.act(0, Move::Attack(vec![(beast, Target::Player(1))])));
}

/// Sword of the Meek: "Whenever a 1/1 creature you control enters, you may return this card
/// from your graveyard to the battlefield, then attach it to that creature." "It" is the
/// object the card became on the battlefield, which the effect can find (400.7j). The engine
/// does not follow a moved card to what it became, so it refuses the card rather than return
/// the Sword unattached.
#[test]
fn r400_7j_sword_of_the_meek_is_refused_until_the_card_it_returns_is_followed() {
    let sword = crate::support::oracle_read("Sword of the Meek");
    cite!(["400.7j"]; refused("attach: a pronoun for the card this effect moved (400.7j)"): game::play::check(&sword).map_err(|unsupported| unsupported.0));
}

/// Sigil Captain: "Whenever a creature you control enters, if that creature is 1/1, put two
/// +1/+1 counters on it." Ruling of 2009-05-01: an intervening "if": it triggers only for a
/// 1/1, and does nothing unless the creature is still 1/1 as it resolves.
#[test]
fn r603_4_sigil_captain_counts_a_creature_that_is_one_one_as_it_enters_and_as_it_resolves() {
    let mut t = Board::new(2).battlefield(0, &["Sigil Captain"]).hand(0, &["Grizzly Bears", "Llanowar Elves"]).build();
    t.mana(0, "{1}{G}{G}");
    t.cast(0, "Grizzly Bears", vec![], mana_pool("{1}{G}")).unwrap();
    t.pass_all();
    cite!(["603.4"]; assert!(t.g.stack.is_empty() && t.find_in(0, Zone::Battlefield, "Grizzly Bears") == t.find("Grizzly Bears"), "a 2/2 entering does not trigger it"));
    assert_eq!(pt(&t, "Grizzly Bears"), (Some(2), Some(2)));
    t.cast(0, "Llanowar Elves", vec![], mana_pool("G")).unwrap();
    t.pass_all();
    cite!(["603.4"]; assert_eq!(t.g.stack.len(), 1, "a 1/1 entering triggers it"));
    t.resolve();
    cite!(["603.4", "122.1a"]; assert_eq!(pt(&t, "Llanowar Elves"), (Some(3), Some(3))));

    // a static boost counts as the creature enters: a 1/1 under Glorious Anthem enters 2/2
    let mut t = Board::new(2).battlefield(0, &["Sigil Captain", "Glorious Anthem"]).hand(0, &["Llanowar Elves"]).build();
    t.mana(0, "{G}");
    t.cast(0, "Llanowar Elves", vec![], mana_pool("G")).unwrap();
    t.pass_all();
    cite!(["603.4"]; assert!(t.g.stack.is_empty(), "it enters as a 2/2"));
    assert_eq!(pt(&t, "Llanowar Elves"), (Some(2), Some(2)));

    // two Captains: both trigger; the second finds a 3/3 and does nothing
    let mut t = Board::new(2).battlefield(0, &["Sigil Captain", "Sigil Captain"]).hand(0, &["Llanowar Elves"]).build();
    t.mana(0, "{G}");
    t.cast(0, "Llanowar Elves", vec![], mana_pool("G")).unwrap();
    assert!(matches!(t.settle(), Boundary::Question(0, game::legal::Asked::Order(triggered)) if triggered.len() == 2), "both trigger (603.3b)");
    t.g.act(0, Move::Order(vec![0, 1])).unwrap();
    t.resolve();
    cite!(["603.4"]; assert_eq!(pt(&t, "Llanowar Elves"), (Some(3), Some(3))));
}

/// Meek Attack: "{1}{R}: You may put a creature card with total power and toughness 5 or less
/// from your hand onto the battlefield. That creature gains haste. At the beginning of the
/// next end step, sacrifice that creature."
#[test]
fn r208_1_meek_attack_puts_a_creature_card_whose_power_and_toughness_total_five_or_less() {
    let mut t = Board::new(2).fill(5).battlefield(0, &["Meek Attack"]).hand(0, &["Hill Giant", "Hurloon Minotaur"]).build();
    t.mana(0, "{1}{R}");
    t.activate(0, "Meek Attack", 0, vec![], mana_pool("{1}{R}")).unwrap();
    let (giant, in_hand) = (t.find_in(0, Zone::Hand, "Hill Giant"), t.find_in(0, Zone::Hand, "Hurloon Minotaur"));
    let Boundary::Question(0, game::legal::Asked::OptionalObjects { among, .. }) = t.settle() else { panic!("its controller is asked which card") };
    // a 3/3 card totals six; a 2/3 totals five
    cite!(["208.1", "608.2d"]; assert_eq!(among, vec![in_hand]));
    cite!(["608.2d"]; refused("a chosen object is not among the offered objects"): t.answer(0, Answer::OptionalObjects(Some(vec![giant]))).map(|_| ()));
    t.answer(0, Answer::OptionalObjects(Some(vec![in_hand]))).unwrap();
    t.resolve();
    let minotaur = t.find_in(0, Zone::Battlefield, "Hurloon Minotaur");
    assert!(t.g.has(minotaur, "Haste"), "\"That creature gains haste.\"");
    // sacrificed as the next end step begins; it could have attacked on the way
    t.to_no_combat(0, Part::End);
    t.resolve();
    cite!(["603.7", "701.21a"]; assert!(!t.g.battlefield.contains(&minotaur) && t.g.players[0].graveyard.len() == 1));

    // "You may": declined, nothing is put onto the battlefield
    let mut t = Board::new(2).fill(5).battlefield(0, &["Meek Attack"]).hand(0, &["Hurloon Minotaur"]).build();
    t.mana(0, "{1}{R}");
    t.activate(0, "Meek Attack", 0, vec![], mana_pool("{1}{R}")).unwrap();
    assert!(matches!(t.settle(), Boundary::Question(0, game::legal::Asked::OptionalObjects { .. })));
    t.answer(0, Answer::OptionalObjects(None)).unwrap();
    t.resolve();
    cite!(["608.2d"]; assert_eq!((t.g.players[0].hand.len(), t.g.battlefield.len()), (1, 1)));
}

/// Pelt Collector: "Whenever another creature you control enters or dies, if that creature's
/// power is greater than this creature's, put a +1/+1 counter on this creature. As long as this
/// creature has three or more +1/+1 counters on it, it has trample." "This creature's" is its
/// power, said once.
fn pelt_counters(t: &T) -> u32 { t.g.obj(t.find("Pelt Collector")).counters.get("+1/+1").copied().unwrap_or(0) }

/// Ruling of 2018-10-05: a creature with power less than or equal to Pelt Collector's entering
/// does not trigger it at all.
#[test]
fn r603_4_pelt_collector_grows_only_for_a_creature_of_greater_power() {
    let mut t = Board::new(2).battlefield(0, &["Pelt Collector"]).hand(0, &["Llanowar Elves", "Grizzly Bears"]).build();
    t.mana(0, "{G}");
    t.cast(0, "Llanowar Elves", vec![], mana_pool("G")).unwrap();
    t.pass_all();
    cite!(["603.4"]; assert!(t.g.stack.is_empty(), "a 1/1 is not greater than a 1/1"));
    t.mana(0, "{1}{G}");
    t.cast(0, "Grizzly Bears", vec![], mana_pool("CG")).unwrap();
    t.pass_all();
    cite!(["603.4"]; assert_eq!(t.g.stack.len(), 1, "a 2/2 is"));
    t.resolve();
    cite!(["603.4", "122.1"]; assert_eq!((pelt_counters(&t), pt(&t, "Pelt Collector")), (1, (Some(2), Some(2)))));
}

/// Ruling of 2018-10-05: a creature that died is compared by its power as it last existed on the
/// battlefield; an opponent's creature dying is not one "you control".
#[test]
fn r603_10a_pelt_collector_compares_a_creature_that_died_as_it_last_existed() {
    let mut t = Board::new(2).battlefield(0, &["Pelt Collector", "Grizzly Bears"]).battlefield(1, &["Hill Giant"]).hand(0, &["Shock", "Lightning Bolt"]).build();
    t.mana(0, "{R}");
    t.cast(0, "Shock", vec![on(0, Zone::Battlefield, "Grizzly Bears")], mana_pool("R")).unwrap();
    t.resolve();
    cite!(["603.10a", "603.4", "122.1"]; assert_eq!(pelt_counters(&t), 1));
    t.mana(0, "{R}");
    t.cast(0, "Lightning Bolt", vec![on(1, Zone::Battlefield, "Hill Giant")], mana_pool("R")).unwrap();
    t.pass_all();
    cite!(["603.2"]; assert!(t.g.stack.is_empty() && t.g.players[1].graveyard.len() == 1, "the Giant was not the Collector's controller's"));
    assert_eq!(pelt_counters(&t), 1);
}

/// Ruling of 2018-10-05: if Pelt Collector's power has been raised to the entering creature's or
/// more as the ability resolves, it gets no counter.
#[test]
fn r603_4_pelt_collector_checks_again_as_the_ability_resolves() {
    let mut t = Board::new(2).battlefield(0, &["Pelt Collector"]).hand(0, &["Grizzly Bears", "Giant Growth"]).build();
    t.mana(0, "{1}{G}");
    t.cast(0, "Grizzly Bears", vec![], mana_pool("CG")).unwrap();
    t.pass_all();
    assert_eq!(t.g.stack.len(), 1);
    t.mana(0, "{G}");
    t.cast(0, "Giant Growth", vec![on(0, Zone::Battlefield, "Pelt Collector")], mana_pool("G")).unwrap();
    t.resolve();
    cite!(["603.4"]; assert_eq!((pelt_counters(&t), pt(&t, "Pelt Collector")), (0, (Some(4), Some(4)))));
}

#[test]
fn r611_3a_pelt_collector_has_trample_with_three_counters() {
    let t = Board::new(2).battlefield(0, &["Pelt Collector"]).counters("Pelt Collector", "+1/+1", 3).build();
    cite!(["611.3a", "702.19a"]; assert!(t.g.has(t.find("Pelt Collector"), "Trample")));
    let t = Board::new(2).battlefield(0, &["Pelt Collector"]).counters("Pelt Collector", "+1/+1", 2).build();
    cite!(["611.3a"]; assert!(!t.g.has(t.find("Pelt Collector"), "Trample")));
}
