//! Review of blitz (702.152a), for what its own tests don't check: a dies trigger of the card
//! itself at the end step, a copy and a new object of a blitzed creature (its rulings), Jaxis's
//! token, the mana value of a spell cast for its blitz cost (118.9c) and an enters trigger.
//! Every card is read from its Oracle text.
use crate::support;
use game::announcement::CastCost;
use game::state::Decision;
use support::*;

#[test]
fn the_cards_of_these_tests_run() {
    for name in ["Caldaia Guardian", "Riveteers Requisitioner", "Jaxis, the Troublemaker", "Night Clubber", "Workshop Warchief", "Clone", "Cloudshift",
        "Prohibit", "Savannah Lions", "Hill Giant", "Grizzly Bears", "Mountain", "Island"] {
        // Uncited (admission): the engine runs each card of these tests.
        assert_eq!(game::play::check(&oracle(name)).err().map(|u| u.0), None, "{name}");
    }
}

fn named(t: &T, name: &str) -> Vec<ObjId> { t.g.battlefield.iter().copied().filter(|o| t.g.obj(*o).chars.name == name).collect() }
fn treasures(t: &T) -> usize { t.g.battlefield.iter().filter(|o| t.g.obj(**o).chars.name.starts_with("Treasure")).count() }

/// Pass until the stack is empty, ordering triggers as they come and answering "you may" with
/// yes and a choice of one object with `pick`.
fn resolve_all(t: &mut T, pick: Option<ObjId>) {
    for _ in 0..40 {
        if let Some(Decision::Order { player, abilities }) = t.g.decision.clone() {
            t.g.act(player, Move::Order((0..abilities.len()).collect())).unwrap();
            continue;
        }
        match t.g.asked() {
            Some((p, Asked::OrderCards { cards, .. })) => { t.answer(p, Answer::Objects(cards)).unwrap(); }
            Some((p, Asked::May)) => { t.answer(p, Answer::Yes).unwrap(); }
            Some((p, Asked::Select { among, .. } | Asked::Objects { among, .. })) => {
                let choice = pick.filter(|o| among.contains(o)).expect("the object to choose");
                t.answer(p, Answer::Objects(vec![choice])).unwrap();
            }
            Some((p, Asked::Priority)) if !t.g.stack.is_empty() => { t.g.act(p, Move::Pass).unwrap(); }
            Some((_, Asked::Priority)) => return,
            other => panic!("unexpected question {other:?}"),
        }
    }
    panic!("the stack did not empty");
}

/// Pass, declaring no attackers or blockers, through the beginning of the end step and its
/// triggers.
fn through_end_step_triggers(t: &mut T) {
    for _ in 0..200 {
        if t.g.part == Part::End && t.g.stack.is_empty() && matches!(t.g.asked(), Some((_, Asked::Priority))) { return; }
        if let Some(Decision::Order { player, abilities }) = t.g.decision.clone() {
            t.g.act(player, Move::Order((0..abilities.len()).collect())).unwrap();
            continue;
        }
        match t.g.decision.clone() {
            Some(Decision::Attackers { player }) => t.g.act(player, Move::Attack(vec![])).unwrap(),
            Some(Decision::Blockers { player }) => t.g.act(player, Move::Block(vec![])).unwrap(),
            _ => { let p = t.g.decision_maker(t.g.priority); t.g.act(p, Move::Pass).unwrap() }
        }
    }
    panic!("no end step");
}

/// Player 0 casts `name` from their hand for its blitz cost, `pool`; the alternative cost is
/// named by its row among the card's abilities.
fn blitz(t: &mut T, name: &str, row: usize, pool: &str) {
    let card = t.find_in(0, Zone::Hand, name);
    t.g.players[0].pool = mana_pool(pool);
    t.g.act(0, paid(pool, Move::CastFor(CastCost::Alternative(row), Box::new(Move::Cast { card, targets: vec![] })))).unwrap();
}

// Rules: 702.152a, 603.10a, 603.7
// Given: player 0 holds Caldaia Guardian (mana value 4).
// When: they cast it for its blitz cost {2}{G}, and the turn reaches its end step.
// Expect: it has haste; at the end step it is sacrificed, and both its own trigger (two
// Citizens) and blitz's (a card) trigger from its dying.
#[test]
fn r702_152a_caldaia_guardian_blitzed_makes_its_citizens_and_draws_as_it_is_sacrificed() {
    // Caldaia Guardian: "Whenever this creature or another creature you control with mana value
    // 4 or greater dies, create two 1/1 green and white Citizen creature tokens. / Blitz {2}{G}"
    let mut t = Board::new(2).fill(6).hand(0, &["Caldaia Guardian"]).build();
    blitz(&mut t, "Caldaia Guardian", 1, "CCG");
    resolve_all(&mut t, None);
    let guardian = t.find_in_battlefield("Caldaia Guardian");
    cite!(["702.152a"]; assert!(t.g.has(guardian, "Haste")));
    let hand = t.g.players[0].hand.len();
    through_end_step_triggers(&mut t);
    cite!(["702.152a", "701.21a"]; assert!(named(&t, "Caldaia Guardian").is_empty() && t.g.players[0].graveyard.iter().any(|o| t.g.obj(*o).chars.name == "Caldaia Guardian")));
    cite!(["603.10a", "702.152a"]; assert_eq!((named(&t, "Citizen Token").len(), t.g.players[0].hand.len()), (2, hand + 1)));
}

// Rules: 707.2, 702.152a
// Given: player 0's Riveteers Requisitioner, cast for its blitz cost; Clone in their hand.
// When: Clone enters as a copy of it, and the turn reaches its end step.
// Expect: the copy has no haste (its ruling: blitz's haste and draw are not copiable values);
// only the Requisitioner is sacrificed, making a Treasure and drawing one card; the copy stays.
#[test]
fn r707_2_a_copy_of_a_blitzed_creature_has_no_haste_and_is_not_sacrificed() {
    let mut t = Board::new(2).fill(6).hand(0, &["Riveteers Requisitioner", "Clone"]).build();
    blitz(&mut t, "Riveteers Requisitioner", 1, "CCR");
    resolve_all(&mut t, None);
    let requisitioner = t.find_in_battlefield("Riveteers Requisitioner");
    t.mana(0, "{3}{U}").cast(0, "Clone", vec![], mana_pool("CCCU")).unwrap();
    resolve_all(&mut t, Some(requisitioner));
    let copy = *named(&t, "Riveteers Requisitioner").iter().find(|o| **o != requisitioner).expect("Clone, a copy of it");
    cite!(["707.2", "702.152a"]; assert!(t.g.has(requisitioner, "Haste") && !t.g.has(copy, "Haste")));
    let hand = t.g.players[0].hand.len();
    through_end_step_triggers(&mut t);
    cite!(["707.2", "702.152a"]; assert_eq!(named(&t, "Riveteers Requisitioner"), [copy]));
    cite!(["702.152a"]; assert_eq!((treasures(&t), t.g.players[0].hand.len()), (1, hand + 1)));
}

// Rules: 400.7, 702.152a
// Given: player 0's Riveteers Requisitioner, cast for its blitz cost; Cloudshift in their hand.
// When: they Cloudshift it, and the turn reaches its end step.
// Expect: it returns as a new object, without haste, and is not sacrificed; it didn't die, so no
// Treasure is made.
#[test]
fn r400_7_a_blitzed_creature_that_leaves_and_returns_stays() {
    // Cloudshift: "Exile target creature you control, then return that card to the battlefield
    // under your control."
    let mut t = Board::new(2).fill(6).hand(0, &["Riveteers Requisitioner", "Cloudshift"]).build();
    blitz(&mut t, "Riveteers Requisitioner", 1, "CCR");
    resolve_all(&mut t, None);
    let requisitioner = t.find_in_battlefield("Riveteers Requisitioner");
    t.mana(0, "{W}").cast(0, "Cloudshift", vec![Tgt::Object(requisitioner)], mana_pool("W")).unwrap();
    resolve_all(&mut t, None);
    let returned = t.find_in_battlefield("Riveteers Requisitioner");
    cite!(["400.7"]; assert!(returned != requisitioner && !t.g.has(returned, "Haste")));
    through_end_step_triggers(&mut t);
    cite!(["400.7", "702.152a"]; assert_eq!((named(&t, "Riveteers Requisitioner"), treasures(&t)), (vec![returned], 0)));
}

// Rules: 115.1, 707.2, 603.7, 701.21a
// Given: Jaxis, the Troublemaker and Riveteers Requisitioner on player 0's battlefield; a
// Mountain and an Island in their hand.
// When: they activate Jaxis at itself, then at the Requisitioner discarding the Mountain, and the
// turn reaches its end step.
// Expect: Jaxis can't copy itself ("another"); the token copy has haste; at the end step it is
// sacrificed, making a Treasure (its copied dies trigger) and drawing a card; the Requisitioner
// stays.
#[test]
fn r707_2_jaxis_s_token_copy_has_haste_and_draws_when_it_is_sacrificed() {
    // Jaxis, the Troublemaker: "{R}, {T}, Discard a card: Create a token that's a copy of another
    // target creature you control. It gains haste and "When this token dies, draw a card."
    // Sacrifice it at the beginning of the next end step. Activate only as a sorcery."
    let mut t = Board::new(2).fill(6).named(0, Zone::Battlefield, "jaxis", "Jaxis, the Troublemaker")
        .named(0, Zone::Battlefield, "requisitioner", "Riveteers Requisitioner").hand(0, &["Mountain", "Island"]).build();
    let (jaxis, requisitioner) = (t.id("jaxis"), t.id("requisitioner"));
    let mountain = t.find_in(0, Zone::Hand, "Mountain");
    t.mana(0, "{R}");
    let activate = |target| paid("R", Move::Activate { source: jaxis, ability: 0, targets: vec![vec![Target::Object(target)]], paying: vec![vec![mountain]] });
    cite!(["115.1"]; refused("another"): t.g.clone().act(0, activate(jaxis)));
    t.g.act(0, activate(requisitioner)).unwrap();
    resolve_all(&mut t, None);
    let token = *named(&t, "Riveteers Requisitioner").iter().find(|o| **o != requisitioner).expect("the token");
    cite!(["707.2", "611.2c"]; assert!(t.g.has(token, "Haste")));
    let hand = t.g.players[0].hand.len();
    through_end_step_triggers(&mut t);
    cite!(["603.7", "701.21a"]; assert_eq!(named(&t, "Riveteers Requisitioner"), [requisitioner]));
    cite!(["707.2", "603.10a"]; assert_eq!((treasures(&t), t.g.players[0].hand.len()), (1, hand + 1)));
}

// Rules: 118.9c, 202.3, 608.2c
// Given: player 0 casts Riveteers Requisitioner ({1}{R}) for its blitz cost {2}{R}, or Workshop
// Warchief ({3}{G}{G}) for its blitz cost {4}{G}{G}.
// When: player 1 answers with Prohibit, unkicked at the Requisitioner and kicked at the Warchief.
// Expect: the mana value is the mana cost's, not the cost paid: the Requisitioner (2) is
// countered; the Warchief (5) is not, and enters.
#[test]
fn r118_9c_a_spell_cast_for_its_blitz_cost_keeps_its_mana_value() {
    for (name, row, pool, kicked, countered) in [("Riveteers Requisitioner", 1, "CCR", false, true), ("Workshop Warchief", 3, "CCCCGG", true, false)] {
        let mut t = Board::new(2).hand(0, &[name]).hand(1, &["Prohibit"]).build();
        blitz(&mut t, name, row, pool);
        let cast = t.g.stack[0];
        t.g.act(0, Move::Pass).unwrap();
        let card = t.find_in(1, Zone::Hand, "Prohibit");
        let (cost, pool) = if kicked { ("{3}{U}", "CCCU") } else { ("{1}{U}", "CU") };
        t.mana(1, cost);
        t.g.act(1, Move::Paying(mana_pool(pool), Box::new(Move::WithKicks(u32::from(kicked),
            Box::new(Move::Cast { card, targets: vec![vec![Target::Object(cast)]] }))))).unwrap();
        resolve_all(&mut t, None);
        cite!(["118.9c", "202.3"]; case: format!("{name}"); assert_eq!(named(&t, name).is_empty(), countered));
    }
}

// Rules: 702.152a, 611.2c, 704.5f
// Given: player 0's Grizzly Bears; player 1's Savannah Lions (2/1) and Hill Giant (3/3).
// When: player 0 casts Night Clubber for its blitz cost {2}{B}.
// Expect: its enters trigger gives player 1's creatures -1/-1: the Lions die and the Giant is
// 2/2; the Bears and the Night Clubber, which has haste, are 2/2.
#[test]
fn r702_152a_night_clubber_blitzed_shrinks_the_opponents_creatures() {
    // Night Clubber: "When this creature enters, creatures your opponents control get -1/-1
    // until end of turn. / Blitz {2}{B}"
    let mut t = Board::new(2).fill(6).hand(0, &["Night Clubber"]).battlefield(1, &["Savannah Lions", "Hill Giant"]).battlefield(0, &["Grizzly Bears"]).build();
    blitz(&mut t, "Night Clubber", 1, "CCB");
    resolve_all(&mut t, None);
    let (giant, bears, clubber) = (t.find_in_battlefield("Hill Giant"), t.find_in_battlefield("Grizzly Bears"), t.find_in_battlefield("Night Clubber"));
    cite!(["704.5f"]; assert!(named(&t, "Savannah Lions").is_empty()));
    cite!(["611.2c"]; assert_eq!([giant, bears, clubber].map(|o| (t.g.power(o), t.g.toughness(o))), [(Some(2), Some(2)); 3]));
    cite!(["702.152a"]; assert!(t.g.has(clubber, "Haste")));
}
