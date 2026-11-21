//! 601.2d: a spell or ability that divides or distributes an amount among its targets has
//! the division announced with the targets; each gets at least one. It is kept as the spell
//! or ability is (603.3d, 707.10, 115.7f), and a target no longer legal gets nothing of it
//! (608.2b). Arc Lightning: "Arc Lightning deals 3 damage divided as you choose among one,
//! two, or three targets." Grove's Bounty: "Distribute X +1/+1 counters among any number of
//! target creatures you control."
use crate::support::cite;
use crate::support;
use support::*;
use game::card::CastAs;

fn divided(division: Vec<Vec<u32>>, mv: Move) -> Move { Move::WithDivision(division, Box::new(mv)) }

fn arc(t: &mut T, targets: Vec<Target>, division: Vec<Vec<u32>>) -> Result<(), String> {
    let card = t.find_in(0, Zone::Hand, "Arc Lightning");
    t.mana(0, "{2}{R}");
    t.g.act(0, divided(division, paid("CCR",Move::Cast { card, targets: vec![targets] })))
}

/// Divided damage destroyed it: its share was lethal (704.5g).
fn destroyed(t: &T, o: ObjId, why: &str) {
    cite!(["601.2d", "704.5g"]; assert!(!t.g.battlefield.contains(&o), "{why}"));
}

fn board() -> (T, ObjId, ObjId) {
    let t = Board::new(2).battlefield(1, &["Grizzly Bears", "Hill Giant"]).hand(0, &["Arc Lightning", "Unsummon", "Shock"]).build();
    let (bears, giant) = (t.find_in_battlefield("Grizzly Bears"), t.find_in_battlefield("Hill Giant"));
    (t, bears, giant)
}

#[test]
fn r601_2d_damage_is_divided_as_announced() {
    let (mut t, bears, giant) = board();
    cite!(["601.2d"]; ok: arc(&mut t, vec![Target::Object(bears), Target::Object(giant), Target::Player(1)], vec![vec![1, 1, 1]]));
    cite!(["601.2d"]; assert_eq!(t.g.obj(t.g.stack[0]).division, vec![vec![1, 1, 1]]));
    t.resolve();
    cite!(["601.2d", "120.3a", "120.3e"]; assert_eq!((t.g.obj(bears).damage, t.g.obj(giant).damage, t.g.players[1].life), (1, 1, 19)));

    let (mut t, bears, giant) = board();
    arc(&mut t, vec![Target::Object(giant), Target::Object(bears)], vec![vec![1, 2]]).unwrap();
    t.resolve();
    cite!(["601.2d", "120.3e"]; assert_eq!(t.g.obj(giant).damage, 1, "the first target chosen gets the first amount"));
    destroyed(&t, bears, "2 damage to the Bears");
}

#[test]
fn r601_2d_one_target_gets_all_of_it() {
    let (mut t, _, giant) = board();
    arc(&mut t, vec![Target::Object(giant)], vec![]).unwrap();
    t.resolve();
    destroyed(&t, giant, "all 3 to the Hill Giant, said or not");
}

#[test]
fn r601_2d_a_division_that_is_not_one_is_refused() {
    let (mut t, bears, giant) = board();
    let two = vec![Target::Object(bears), Target::Object(giant)];
    let four = vec![Target::Object(bears), Target::Object(giant), Target::Player(0), Target::Player(1)];
    for (targets, division, why, rule) in [
        (&two, vec![], "divide 3 among the 2 targets: say what each gets (601.2d)", "601.2d"),
        (&two, vec![vec![2, 2]], "the division adds up to 4, and 3 is divided (601.2d)", "601.2d"),
        (&two, vec![vec![3, 0]], "each target gets at least one of what is divided (601.2d)", "601.2d"),
        (&two, vec![vec![1, 1, 1]], "3 amount(s) for 2 targets: one for each (601.2d)", "601.2d"),
        // each gets at least one: at most three targets for 3 damage
        (&four, vec![vec![1, 1, 1, 0]], "wrong number of targets: 4 chosen, it takes 1 to 3 (601.2c)", "601.2c"),
    ] {
        cite!([rule]; case: format!("{division:?}"); refused(why): arc(&mut t.clone(), targets.clone(), division.clone()));
    }
    // Shock divides nothing
    let shock = t.find_in(0, Zone::Hand, "Shock");
    t.mana(0, "{R}");
    cite!(["601.2d"]; refused("those targets divide nothing (601.2d)"): t.g.act(0, divided(vec![vec![2]], paid("R",Move::Cast { card: shock, targets: vec![vec![Target::Object(bears)]] }))));
}

#[test]
fn r608_2b_an_illegal_targets_share_is_not_dealt() {
    let (mut t, bears, giant) = board();
    arc(&mut t, vec![Target::Object(bears), Target::Object(giant)], vec![vec![2, 1]]).unwrap();
    // in response, the Bears return to their owner's hand
    let unsummon = t.find_in(0, Zone::Hand, "Unsummon");
    t.mana(0, "{U}").g.act(0, paid("U",Move::Cast { card: unsummon, targets: vec![vec![Target::Object(bears)]] })).unwrap();
    t.resolve();
    cite!(["608.2b", "601.2d"]; assert_eq!(t.g.obj(giant).damage, 1, "the Hill Giant gets its 1, not the Bears' 2"));
}

#[test]
fn r601_2d_counters_are_distributed_among_any_number() {
    let mut t = Board::new(2).battlefield(0, &["Grizzly Bears", "Hill Giant"]).hand(0, &["Elusive Otter"]).build();
    let (bears, giant) = (t.find_in_battlefield("Grizzly Bears"), t.find_in_battlefield("Hill Giant"));
    let otter = t.find_in(0, Zone::Hand, "Elusive Otter");
    let bounty = |x: u32, targets: Vec<Target>, division: Vec<Vec<u32>>| divided(division,
        Move::WithX(x, Box::new(Move::CastAs(CastAs::Adventure, Box::new(paid(&format!("{}G","C".repeat(x as usize)),Move::Cast { card: otter, targets: vec![targets] }))))));
    // X is 0: none can get at least one, so no target is chosen
    let mut none = t.clone();
    cite!(["601.2c", "601.2d"]; refused("wrong number of targets: 1 chosen, it takes 0 (601.2c)"): none.mana(0, "{G}").g.act(0, bounty(0, vec![Target::Object(bears)], vec![])));
    cite!(["601.2c", "601.2d"]; ok: none.g.act(0, bounty(0, vec![], vec![])));
    t.mana(0, "{3}{G}").g.act(0, bounty(3, vec![Target::Object(bears), Target::Object(giant)], vec![vec![2, 1]])).unwrap();
    t.resolve();
    cite!(["601.2d", "122.1"]; assert_eq!((t.g.obj(bears).counters.get("+1/+1"), t.g.obj(giant).counters.get("+1/+1")), (Some(&2), Some(&1))));
    t.expect().rules(["613.4c"]).pt("Grizzly Bears", 4, 4).pt("Hill Giant", 4, 4).check();
}

/// A triggered ability's division is announced with its targets as it is put on the stack
/// (603.3d). Bogardan Hellkite: "When this creature enters, it deals 5 damage divided as you
/// choose among any number of targets."
#[test]
fn r603_3d_a_triggered_abilitys_division_is_said_with_its_targets() {
    let mut t = Board::new(2).battlefield(1, &["Grizzly Bears", "Hill Giant"]).hand(0, &["Bogardan Hellkite"]).build();
    let (bears, giant) = (t.find_in_battlefield("Grizzly Bears"), t.find_in_battlefield("Hill Giant"));
    t.mana(0, "{6}{R}{R}").cast(0, "Bogardan Hellkite", vec![], crate::support::mana_pool("CCCCCCRR")).unwrap();
    let Boundary::Question(0, _) = t.settle() else { panic!("its targets are asked: {:?}", t.asked()) };
    let Some((0, Asked::Targets { slots, .. })) = t.asked() else { panic!("{:?}", t.asked()) };
    cite!(["603.3d", "601.2d"]; assert_eq!((slots[0].max, slots[0].divide), (5, Some(5)), "the Bears, the Hill Giant, the Hellkite and both players"));
    let targets = vec![vec![Target::Object(bears), Target::Object(giant), Target::Player(1)]];
    cite!(["603.3d", "601.2d"]; refused("divide 5 among the 3 targets: say what each gets (601.2d)"): t.g.clone().act(0, Move::ChooseTargets(targets.clone())));
    cite!(["603.3d", "601.2d"]; ok: t.g.act(0, divided(vec![vec![2, 2, 1]], Move::ChooseTargets(targets))));
    t.resolve();
    destroyed(&t, bears, "2 damage to the Bears");
    cite!(["603.3d", "601.2d"]; assert_eq!((t.g.obj(giant).damage, t.g.players[1].life), (2, 19)));
}

/// An activated ability's division is announced as it is activated (602.2b). Arc Mage:
/// "{2}{R}, {T}, Discard a card: This creature deals 2 damage divided as you choose among one
/// or two targets."
#[test]
fn r602_2b_an_activated_abilitys_division_is_said_as_it_is_activated() {
    let mut t = Board::new(2).battlefield(0, &["Arc Mage"]).battlefield(1, &["Grizzly Bears"]).hand(0, &["Shock"]).build();
    let (mage, bears) = (t.find_in_battlefield("Arc Mage"), t.find_in_battlefield("Grizzly Bears"));
    let shock = t.find_in(0, Zone::Hand, "Shock");
    t.mana(0, "{2}{R}");
    let activate = Move::Activate { source: mage, ability: 0, targets: vec![vec![Target::Object(bears), Target::Player(1)]], paying: vec![vec![shock]] };
    cite!(["602.2b", "601.2d"]; ok: t.g.act(0, divided(vec![vec![1, 1]], paid("CCR",activate))));
    t.resolve();
    cite!(["602.2b", "601.2d"]; assert_eq!((t.g.obj(bears).damage, t.g.players[1].life), (1, 19)));
}

/// A copy copies the division (707.10), which new targets do not change (115.7f).
/// Twincast: "Copy target instant or sorcery spell. You may choose new targets for the copy."
#[test]
fn r707_10_a_copy_keeps_the_division() {
    let mut t = Board::new(2).battlefield(1, &["Grizzly Bears", "Hill Giant"]).hand(0, &["Arc Lightning", "Twincast"]).build();
    let (bears, giant) = (t.find_in_battlefield("Grizzly Bears"), t.find_in_battlefield("Hill Giant"));
    arc(&mut t, vec![Target::Object(giant), Target::Player(1)], vec![vec![2, 1]]).unwrap();
    let spell = t.g.stack[0];
    let twincast = t.find_in(0, Zone::Hand, "Twincast");
    t.mana(0, "{U}{U}").g.act(0, paid("UU",Move::Cast { card: twincast, targets: vec![vec![Target::Object(spell)]] })).unwrap();
    t.g.act(0, Move::Pass).unwrap();
    t.g.act(1, Move::Pass).unwrap();
    // new targets for the copy: the Bears for the Hill Giant, the division as it was
    let Some((0, Asked::Retarget { original, .. })) = t.asked() else { panic!("{:?}", t.asked()) };
    cite!(["707.10"]; assert_eq!(original, vec![vec![Target::Object(giant), Target::Player(1)]]));
    t.g.act(0, Move::Choose(game::state::Answer::Targets(vec![vec![Target::Object(bears), Target::Player(1)]]))).unwrap();
    let copy = t.g.stack[1];
    cite!(["707.10", "115.7f"]; assert_eq!(t.g.obj(copy).division, vec![vec![2, 1]]));
    t.resolve();
    destroyed(&t, bears, "the copy's 2 to the Bears");
    cite!(["707.10", "115.7f"]; assert_eq!((t.g.obj(giant).damage, t.g.players[1].life), (2, 18)));
}

/// Of two target groups only the first divides: the second says no division.
/// Fiery Justice: "Fiery Justice deals 5 damage divided as you choose among any number of
/// targets. Target opponent gains 5 life."
#[test]
fn r601_2d_a_group_that_divides_nothing_gets_no_division() {
    let mut t = Board::new(2).battlefield(1, &["Grizzly Bears", "Hill Giant"]).hand(0, &["Fiery Justice"]).build();
    let (bears, giant) = (t.find_in_battlefield("Grizzly Bears"), t.find_in_battlefield("Hill Giant"));
    let card = t.find_in(0, Zone::Hand, "Fiery Justice");
    let targets = vec![vec![Target::Object(bears), Target::Object(giant)], vec![Target::Player(1)]];
    t.mana(0, "{R}{G}{W}");
    cite!(["601.2d"]; refused("those targets divide nothing (601.2d)"): t.g.clone().act(0, divided(vec![vec![2, 3], vec![5]], paid("RGW",Move::Cast { card, targets: targets.clone() }))));
    t.g.act(0, divided(vec![vec![2, 3]], paid("RGW",Move::Cast { card, targets }))).unwrap();
    assert!(matches!(t.settle(), Boundary::Question(1, Asked::OrderCards { .. })));
    // CR404.3: explicitly authored owner order, separate from selection.
    assert_eq!(t.g.decision, Some(game::state::Decision::Choose { player: 1, choice: game::state::Choice::OrderCards { to: game::state::Placement::Zone(Zone::Graveyard), cards: vec![bears,giant] } }));
    t.g.act(1, Move::Choose(game::state::Answer::Objects(vec![giant,bears]))).unwrap();
    t.resolve();
    cite!(["601.2d", "704.5g"]; assert!(!t.g.battlefield.contains(&bears) && !t.g.battlefield.contains(&giant)));
    cite!(["601.2d", "119.3"]; assert_eq!(t.g.players[1].life, 25));
}
