//! Divided damage (601.2d) against the cards' rulings: Meteor Swarm's X targets,
//! Fire // Ice's halves, Fury evoked, Kuldotha Flamefiend's zero targets and lost sacrifice,
//! Fiery Justice's opponent in both groups.
use crate::support::cite;
use crate::support;
use game::announcement::CastCost;
use game::card::CastAs;
use support::*;

fn divided(division: Vec<Vec<u32>>, mv: Move) -> Move { Move::WithDivision(division, Box::new(mv)) }
fn objects(of: &[ObjId]) -> Vec<Target> { of.iter().map(|o| Target::Object(*o)).collect() }

/// Rules: 601.2d, 601.2b, 601.2c
/// Given: Meteor Swarm, "8 damage divided as you choose among X target creatures and/or
/// planeswalkers", and two of player 1's creatures.
/// When: X is 2 with 5 and 3; X is 1 with one target.
/// Expect: the division is dealt; one target gets all 8; X targets must be X.
/// Why: each target gets at least 1, so X is at most 8 (ruling); the count is X.
#[test]
fn r601_2d_meteor_swarm_divides_among_x_targets() {
    let t = Board::new(2).battlefield(1, &["Hill Giant", "Craw Wurm"]).hand(0, &["Meteor Swarm"]).build();
    let (giant, wurm) = (t.find_in_battlefield("Hill Giant"), t.find_in_battlefield("Craw Wurm"));
    let card = t.find_in(0, Zone::Hand, "Meteor Swarm");
    let swarm = |x: u32, at: &[ObjId], division: Vec<Vec<u32>>| divided(division, Move::WithX(x, Box::new(paid(&format!("{}RRR","C".repeat(x as usize)),Move::Cast { card, targets: vec![objects(at)] }))));

    let mut two = t.clone();
    two.mana(0, "{2}{R}{R}{R}").g.act(0, swarm(2, &[giant, wurm], vec![vec![3, 5]])).unwrap();
    assert!(matches!(two.settle(), Boundary::Question(1, Asked::OrderCards { .. })));
    // CR404.3: explicitly authored owner order, separate from selection.
    assert_eq!(two.g.decision, Some(game::state::Decision::Choose { player: 1, choice: game::state::Choice::OrderCards { to: game::state::Placement::Zone(Zone::Graveyard), cards: vec![giant,wurm] } }));
    two.g.act(1, Move::Choose(game::state::Answer::Objects(vec![wurm,giant]))).unwrap();
    two.resolve();
    cite!(["601.2d", "704.5g"]; assert!(!two.g.battlefield.contains(&giant) && !two.g.battlefield.contains(&wurm), "3 to the 3/3, 5 to the 6/4"));

    let mut one = t.clone();
    one.mana(0, "{1}{R}{R}{R}").g.act(0, swarm(1, &[wurm], vec![])).unwrap();
    one.resolve();
    cite!(["601.2d", "704.5g"]; assert!(!one.g.battlefield.contains(&wurm), "one target gets all 8"));

    let mut wrong = t.clone();
    cite!(["601.2c", "601.2b"]; refused("wrong number of targets: 1 chosen, it takes 2 (601.2c)"): wrong.mana(0, "{2}{R}{R}{R}").g.act(0, swarm(2, &[wurm], vec![])));
    let mut nine = t.clone();
    cite!(["601.2c"]; refused("there are only 2 legal targets, and it needs 9 (601.2c)"): nine.mana(0, "{9}{R}{R}{R}").g.act(0, swarm(9, &[giant, wurm], vec![vec![4, 4]])));
}

/// Rules: 601.2d, 709.3
/// Given: Fire // Ice; player 1's Grizzly Bears.
/// When: Fire is cast at the Bears and player 1 with 1 each; Ice is cast with a division.
/// Expect: Fire deals its division; Ice, which divides nothing, refuses one.
/// Why: only the half cast is on the stack (709.3); its text is what divides.
#[test]
fn r601_2d_fire_divides_and_ice_does_not() {
    let mut t = Board::new(2).battlefield(1, &["Grizzly Bears"]).hand(0, &["Fire // Ice"]).build();
    let bears = t.find_in_battlefield("Grizzly Bears");
    let card = t.find_in(0, Zone::Hand, "Fire // Ice");
    let half = |n: usize, targets: Vec<Target>, division: Vec<Vec<u32>>| divided(division, Move::CastAs(CastAs::Half(n), Box::new(paid(if n==0{"CR"}else{"CU"},Move::Cast { card, targets: vec![targets] }))));
    let mut ice = t.clone();
    cite!(["601.2d", "709.3"]; refused("those targets divide nothing (601.2d)"): ice.mana(0, "{1}{U}").g.act(0, half(1, vec![Target::Object(bears)], vec![vec![1]])));
    cite!(["601.2d", "709.3"]; ok: t.mana(0, "{1}{R}").g.act(0, half(0, vec![Target::Object(bears), Target::Player(1)], vec![vec![1, 1]])));
    t.resolve();
    cite!(["601.2d", "120.3a", "120.3e"]; assert_eq!((t.g.obj(bears).damage, t.g.players[1].life), (1, 19)));
}

/// Player 0's evoked Fury enters; its two triggers are put on the stack in either order (the
/// damage is dealt either way, with Fury's last known information), its targets and division
/// said as the damage trigger is put on the stack.
fn evoke_fury(t: &mut T, targets: Vec<Target>, division: Vec<Vec<u32>>) -> Result<(), String> {
    let card = t.find_in(0, Zone::Hand, "Fury");
    let red = t.find_in(0, Zone::Hand, "Lightning Bolt");
    let row = 2; // Fury prints double strike, its enters trigger, then evoke.
    t.g.act(0, Move::CastFor(CastCost::Alternative(row), Box::new(Move::WithCosts(vec![vec![red]], Box::new(Move::Cast { card, targets: vec![] }))))).unwrap();
    for _ in 0..20 {
        match t.g.asked() {
            Some((p, Asked::Order(abilities))) => t.g.act(p, Move::Order((0..abilities.len()).collect()))?,
            Some((p, Asked::Targets { .. })) => return t.g.act(p, divided(division, Move::ChooseTargets(vec![targets]))),
            Some((p, _)) if p == t.g.priority => t.g.act(p, Move::Pass)?,
            other => panic!("{other:?}"),
        }
    }
    panic!("no targets asked")
}

/// Rules: 601.2d, 603.3d, 702.74a
/// Given: Fury ("it deals 4 damage divided as you choose among any number of target creatures
/// and/or planeswalkers"), evoked by exiling Lightning Bolt; player 1's two creatures.
/// When: its damage trigger targets both, 1 and 3.
/// Expect: the Bears survive with 1 damage, the Hill Giant dies; five targets are refused.
/// Why: the division is said as the trigger is put on the stack; each gets at least 1, so at
/// most four targets (ruling).
#[test]
fn r603_3d_evoked_fury_divides_as_its_trigger_is_put_on_the_stack() {
    let t = Board::new(2).battlefield(1, &["Grizzly Bears", "Hill Giant"]).hand(0, &["Fury", "Lightning Bolt"]).build();
    let (bears, giant) = (t.find_in_battlefield("Grizzly Bears"), t.find_in_battlefield("Hill Giant"));
    let mut ok = t.clone();
    evoke_fury(&mut ok, objects(&[bears, giant]), vec![vec![1, 3]]).unwrap();
    ok.resolve();
    cite!(["603.3d", "601.2d"]; assert_eq!(ok.g.obj(bears).damage, 1));
    cite!(["603.3d", "601.2d", "704.5g"]; assert!(!ok.g.battlefield.contains(&giant)));
    cite!(["702.74a"]; assert!(ok.g.players[0].graveyard.iter().any(|o| ok.g.obj(*o).chars.name == "Fury"), "sacrificed"));
}

/// Rules: 601.2c, 601.2d
/// Given: evoked Fury and five creatures and/or planeswalkers it could target.
/// When: its trigger targets five of them.
/// Expect: refused, whatever the division.
/// Why: 4 damage with at least 1 each: at most four targets (ruling).
#[test]
fn r601_2d_fury_cant_have_more_targets_than_damage() {
    let mut t = Board::new(2).battlefield(1, &["Grizzly Bears", "Hill Giant", "Llanowar Elves", "Craw Wurm", "Savannah Lions"]).hand(0, &["Fury", "Lightning Bolt"]).build();
    let five: Vec<ObjId> = ["Grizzly Bears", "Hill Giant", "Llanowar Elves", "Craw Wurm", "Savannah Lions"].iter().map(|n| t.find_in_battlefield(n)).collect();
    cite!(["601.2c", "601.2d"]; refused("wrong number of targets: 5 chosen, it takes 0 to 4 (601.2c)"): evoke_fury(&mut t, objects(&five), vec![vec![1, 1, 1, 1, 0]]));
}

/// Player 0's Kuldotha Flamefiend has entered; its trigger's targets and division are said.
fn flamefiend(t: &mut T, targets: Vec<Target>, division: Vec<Vec<u32>>) {
    t.mana(0, "{4}{R}{R}").cast(0, "Kuldotha Flamefiend", vec![], crate::support::mana_pool("CCCCRR")).unwrap();
    for _ in 0..20 {
        match t.g.asked() {
            Some((p, Asked::Targets { .. })) => { t.g.act(p, divided(division, Move::ChooseTargets(vec![targets]))).unwrap(); return; }
            Some((p, _)) if p == t.g.priority => t.g.act(p, Move::Pass).unwrap(),
            other => panic!("{other:?}"),
        }
    }
    panic!("no targets asked")
}

/// Rules: 601.2d, 603.3d, 608.2b
/// Given: Kuldotha Flamefiend ("you may sacrifice an artifact. If you do, this creature deals
/// 4 damage divided as you choose among any number of targets") and player 0's Ornithopter.
/// When: its trigger has no targets.
/// Expect: it resolves: the sacrifice is still offered, and no damage is dealt.
/// Why: "any number" includes zero (ruling); with no targets it is not a targeted ability
/// whose targets are all illegal (608.2b).
#[test]
fn r608_2b_flamefiend_with_no_targets_still_offers_the_sacrifice() {
    let mut t = Board::new(2).battlefield(0, &["Ornithopter"]).hand(0, &["Kuldotha Flamefiend"]).build();
    let thopter = t.find_in_battlefield("Ornithopter");
    flamefiend(&mut t, vec![], vec![]);
    let Boundary::Question(0, asked) = t.settle() else { panic!("the sacrifice is offered: {:?}", t.asked()) };
    eprintln!("{asked:?}");
    t.g.act(0, Move::Choose(Answer::Objects(vec![thopter]))).or_else(|_| t.g.act(0, Move::Choose(Answer::Yes))).unwrap();
    t.settle();
    cite!(["608.2b"]; assert!(!t.g.battlefield.contains(&thopter), "sacrificed"));
    assert_eq!(t.g.players[1].life, 20);
}

/// Rules: 608.2b, 601.2d
/// Given: Kuldotha Flamefiend's trigger targets player 1's Grizzly Bears with all 4.
/// When: the Bears leave before it resolves.
/// Expect: it doesn't resolve: no sacrifice is offered, the Ornithopter stays.
/// Why: all its targets are illegal (608.2b); "you can't sacrifice an artifact" (ruling).
#[test]
fn r608_2b_flamefiend_whose_targets_are_gone_offers_no_sacrifice() {
    let mut t = Board::new(2).battlefield(0, &["Ornithopter"]).battlefield(1, &["Grizzly Bears"]).hand(0, &["Kuldotha Flamefiend", "Unsummon"]).build();
    let (thopter, bears) = (t.find_in_battlefield("Ornithopter"), t.find_in_battlefield("Grizzly Bears"));
    flamefiend(&mut t, vec![Target::Object(bears)], vec![]);
    let unsummon = t.find_in(0, Zone::Hand, "Unsummon");
    t.mana(0, "{U}").g.act(0, paid("U",Move::Cast { card: unsummon, targets: vec![vec![Target::Object(bears)]] })).unwrap();
    let boundary = t.settle();
    cite!(["608.2b"]; assert!(matches!(boundary, Boundary::Priority(_)), "nothing asked: {boundary:?}"));
    cite!(["608.2b"]; assert!(t.g.battlefield.contains(&thopter)));
}

/// Rules: 601.2d, 608.2b, 119.3
/// Given: Fiery Justice; player 1 at 5 life with Grizzly Bears.
/// When: it deals all 5 damage to player 1, who is also the opponent who gains 5 life; and,
/// separately, its damage targets only the Bears, which leave before it resolves.
/// Expect: player 1 ends at 5 and hasn't lost; with the Bears gone player 1 still gains 5.
/// Why: the life gain comes before state-based actions (ruling); the opponent is still a
/// legal target, so the spell resolves for the parts it can (608.2b).
#[test]
fn r608_2b_fiery_justice_heals_who_it_burned_and_survives_a_lost_target() {
    let mut t = Board::new(2).battlefield(1, &["Grizzly Bears"]).hand(0, &["Fiery Justice", "Unsummon"]).build();
    let bears = t.find_in_battlefield("Grizzly Bears");
    let card = t.find_in(0, Zone::Hand, "Fiery Justice");
    t.g.players[1].life = 5;
    let mut both = t.clone();
    both.mana(0, "{R}{G}{W}").g.act(0, paid("RGW",Move::Cast { card, targets: vec![vec![Target::Player(1)], vec![Target::Player(1)]] })).unwrap();
    both.resolve();
    cite!(["608.2c", "120.3a", "119.3"]; assert_eq!(both.g.players[1].life, 5));
    cite!(["704.3"]; assert!(both.g.winner().is_none()));

    let mut gone = t.clone();
    gone.mana(0, "{R}{G}{W}").g.act(0, paid("RGW",Move::Cast { card, targets: vec![vec![Target::Object(bears)], vec![Target::Player(1)]] })).unwrap();
    let unsummon = gone.find_in(0, Zone::Hand, "Unsummon");
    gone.mana(0, "{U}").g.act(0, paid("U",Move::Cast { card: unsummon, targets: vec![vec![Target::Object(bears)]] })).unwrap();
    gone.resolve();
    cite!(["608.2b"]; assert_eq!(gone.g.players[1].life, 10));
}
