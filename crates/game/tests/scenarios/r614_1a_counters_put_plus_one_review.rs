//! Review of Winding Constrictor ("If one or more counters would be put on an artifact or
//! creature you control, that many plus one of each of those kinds of counters are put on that
//! permanent instead. If you would get one or more counters, you get that many plus one of each
//! of those kinds of counters instead."), against its rulings: two of them add two, a permanent
//! entering with counters gets one more, each instruction of an effect is replaced, any kind of
//! counter is, and each kind of a proliferate. Every card is read from its Oracle text.
use crate::support::*;
use game::legal::Asked;
use game::state::Decision;

#[test]
fn the_cards_of_these_tests_run() {
    for name in ["Winding Constrictor", "Battlegrowth", "Walking Ballista", "Lifecrafter's Gift", "Contagion Clasp", "Steady Progress",
        "Grizzly Bears", "Savannah Lions", "Hill Giant"] {
        // Uncited (admission): the engine runs each card of these tests.
        assert_eq!(game::play::check(&oracle(name)).err().map(|u| u.0), None, "{name}");
    }
}

fn count(t: &T, o: impl Into<Target>, kind: &str) -> u32 { t.g.counters(o.into()).get(kind).copied().unwrap_or(0) }

/// Until priority with an empty stack, answering trigger order, replacement order (the first
/// offered, 616.1) and a target of `target`.
fn settle(t: &mut T, target: Option<ObjId>) {
    for _ in 0..30 {
        if let Some(Decision::Order { player, abilities }) = t.g.decision.clone() {
            t.g.act(player, Move::Order((0..abilities.len()).collect())).unwrap();
            continue;
        }
        match t.settle() {
            Boundary::Question(p, Asked::Replacement { among, .. }) => { t.answer(p, Answer::Replacement(among[0].clone())).unwrap(); }
            Boundary::Question(p, Asked::Targets { .. }) if target.is_some() => { t.g.act(p, Move::ChooseTargets(vec![vec![Target::Object(target.unwrap())]])).unwrap(); }
            Boundary::Priority(_) => return,
            other => panic!("unexpected {other:?}"),
        }
    }
    panic!("the stack did not empty");
}

// Rules: 614.1a, 616.1
// Given: player 0's two Winding Constrictors and Grizzly Bears.
// When: player 0 casts Battlegrowth ("Put a +1/+1 counter on target creature.") at the Bears.
// Expect: each replaces the event in turn (616.1): three counters (its ruling).
#[test]
fn r616_1_two_winding_constrictors_add_two() {
    let mut t = Board::new(2).battlefield(0, &["Winding Constrictor", "Winding Constrictor", "Grizzly Bears"]).hand(0, &["Battlegrowth"]).build();
    let bears = t.find_in_battlefield("Grizzly Bears");
    t.mana(0, "{G}").cast(0, "Battlegrowth", vec![Tgt::Object(bears)], mana_pool("G")).unwrap();
    settle(&mut t, None);
    cite!(["614.1a", "616.1"]; assert_eq!(count(&t, bears, "+1/+1"), 3));
}

// Rules: 122.6, 614.1c
// Given: player 0's Winding Constrictor.
// When: player 0 casts Walking Ballista ("This creature enters with X +1/+1 counters on it.")
// with X 1.
// Expect: an artifact creature entering with counters gets one more (its ruling, 122.6): two.
#[test]
fn r122_6_a_creature_entering_with_counters_gets_one_more() {
    let mut t = Board::new(2).battlefield(0, &["Winding Constrictor"]).hand(0, &["Walking Ballista"]).build();
    let card = t.find_in(0, Zone::Hand, "Walking Ballista");
    t.mana(0, "{2}");
    t.g.act(0, paid("CC", Move::WithX(1, Box::new(Move::Cast { card, targets: vec![] })))).unwrap();
    settle(&mut t, None);
    let ballista = t.find_in_battlefield("Walking Ballista");
    cite!(["122.6", "614.1c"]; assert_eq!(count(&t, ballista, "+1/+1"), 2));
}

// Rules: 614.1a, 608.2c
// Given: player 0's Winding Constrictor, Grizzly Bears and Savannah Lions.
// When: player 0 casts Lifecrafter's Gift ("Put a +1/+1 counter on target creature, then put a
// +1/+1 counter on each creature you control with a +1/+1 counter on it.") at the Bears.
// Expect: each instruction is replaced (its ruling): the Bears gets two, then two more; the
// Lions, with none, gets none. The Constrictor has none either.
#[test]
fn r614_1a_each_instruction_to_put_counters_is_replaced() {
    let mut t = Board::new(2).battlefield(0, &["Winding Constrictor", "Grizzly Bears", "Savannah Lions"]).hand(0, &["Lifecrafter's Gift"]).build();
    let (bears, lions) = (t.find_in_battlefield("Grizzly Bears"), t.find_in_battlefield("Savannah Lions"));
    t.mana(0, "{3}{G}").cast(0, "Lifecrafter's Gift", vec![Tgt::Object(bears)], mana_pool("CCCG")).unwrap();
    settle(&mut t, None);
    cite!(["614.1a", "608.2c"]; assert_eq!((count(&t, bears, "+1/+1"), count(&t, lions, "+1/+1")), (4, 0)));
}

// Rules: 614.1a, 122.1
// Given: player 0's Winding Constrictor; Hill Giant, player 0's or player 1's.
// When: player 0 casts Contagion Clasp ("When this artifact enters, put a -1/-1 counter on target
// creature.") at the Giant.
// Expect: any kind of counter put on a creature its controller controls gets one more: their own
// Giant gets two -1/-1 counters, player 1's one.
#[test]
fn r614_1a_a_minus_one_counter_on_your_creature_gets_one_more() {
    for (owner, expected) in [(0, 2), (1, 1)] {
        let mut t = Board::new(2).battlefield(0, &["Winding Constrictor"]).battlefield(owner, &["Hill Giant"]).hand(0, &["Contagion Clasp"]).build();
        let giant = t.find_in_battlefield("Hill Giant");
        t.mana(0, "{2}").cast(0, "Contagion Clasp", vec![], mana_pool("CC")).unwrap();
        settle(&mut t, Some(giant));
        cite!(["614.1a", "122.1"]; case: format!("player {owner}'s Giant"); assert_eq!(count(&t, giant, "-1/-1"), expected));
    }
}

// Rules: 701.34a, 614.1a
// Given: player 0's Grizzly Bears with a +1/+1 counter, player 0 with a poison and an energy
// counter, player 1 with a poison counter; player 0's Winding Constrictor, or none.
// When: player 0 casts Steady Progress and proliferates all three.
// Expect: each kind given is one more for what player 0 controls and player 0 (its ruling): the
// Bears and each of player 0's kinds go to three; player 1's poison goes to two. Without it, all
// go to two.
#[test]
fn r701_34a_proliferate_gives_one_more_of_each_kind() {
    for constrictor in [true, false] {
        let mine: &[&str] = if constrictor { &["Winding Constrictor", "Grizzly Bears"] } else { &["Grizzly Bears"] };
        let mut t = Board::new(2).fill(8).battlefield(0, mine).hand(0, &["Steady Progress"]).build();
        let bears = t.find_in_battlefield("Grizzly Bears");
        t.g.put_counters(Target::Object(bears), "+1/+1", 1);
        t.g.put_counters(Target::Player(0), "poison", 1);
        t.g.put_counters(Target::Player(0), "energy", 1);
        t.g.put_counters(Target::Player(1), "poison", 1);
        t.mana(0, "{2}{U}").cast(0, "Steady Progress", vec![], mana_pool("CCU")).unwrap();
        let Boundary::Question(0, Asked::Holders { .. }) = t.settle() else { panic!("{:?}", t.asked()) };
        t.answer(0, Answer::Holders(vec![Target::Object(bears), Target::Player(0), Target::Player(1)])).unwrap();
        settle(&mut t, None);
        let mine = if constrictor { 3 } else { 2 };
        let got = (count(&t, bears, "+1/+1"), count(&t, Target::Player(0), "poison"), count(&t, Target::Player(0), "energy"), count(&t, Target::Player(1), "poison"));
        cite!(["701.34a", "614.1a"]; case: format!("constrictor {constrictor}"); assert_eq!(got, (mine, mine, mine, 2)));
    }
}
