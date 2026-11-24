//! "The sacrificed creature", "the revealed cards": a participle names what an instruction or a
//! cost of the same text did (608.2c, 608.2h). Its noun may be the plainer one: "Sacrifice a
//! Goblin creature: … the sacrificed creature's power". And a condition said after its act,
//! "counter target spell if its mana value is 2 or less", asks of that target as the spell
//! resolves (608.2c).
use crate::support;
use support::*;

fn names(g: &Game, cards: &[ObjId]) -> Vec<String> {
    let mut names: Vec<_> = cards.iter().map(|o| g.obj(*o).chars.name.clone()).collect();
    names.sort();
    names
}

/// Pass, answering only the questions a test does not ask about: the order of cards put on the
/// bottom of a library or into a graveyard.
fn settle(t: &mut T) {
    for _ in 0..12 {
        match t.asked() {
            Some((p, Asked::OrderCards { cards, .. })) => { t.answer(p, Answer::Objects(cards)).unwrap(); }
            Some((_, Asked::Priority)) | None if !t.g.stack.is_empty() => { t.pass_all(); }
            _ => return,
        }
    }
}

/// Player 0 casts the creature spell; player 1 answers with Prohibit, kicked or not.
fn prohibit(spell: &str, cost: &str, pool: &str, kicked: bool) -> (T, ObjId) {
    let mut t = Board::new(2).hand(0, &[spell]).hand(1, &["Prohibit"]).build();
    t.mana(0, cost).cast(0, spell, vec![], mana_pool(pool)).unwrap();
    let cast = t.g.stack[0];
    t.g.act(0, Move::Pass).unwrap();
    let card = t.find_in(1, Zone::Hand, "Prohibit");
    let (cost, pool) = if kicked { ("{3}{U}", "CCCU") } else { ("{1}{U}", "CU") };
    t.mana(1, cost);
    t.g.act(1, Move::Paying(mana_pool(pool), Box::new(Move::WithKicks(u32::from(kicked),
        Box::new(Move::Cast { card, targets: vec![vec![Target::Object(cast)]] }))))).unwrap();
    settle(&mut t);
    (t, cast)
}

// Rules: 608.2c, 702.33d
// Given: Prohibit aimed at a creature spell of mana value 2, 3, 4 or 6, kicked or not.
// When: it resolves.
// Expect: unkicked, it counters the spell only if its mana value is 2 or less; kicked, only if
// it is 4 or less. A spell it doesn't counter resolves.
#[test]
fn r608_2c_prohibit_counters_by_mana_value_and_more_when_kicked() {
    for (spell, cost, pool, kicked, countered) in [
        ("Grizzly Bears", "{1}{G}", "CG", false, true),
        ("Centaur Courser", "{2}{G}", "CCG", false, false),
        ("Centaur Courser", "{2}{G}", "CCG", true, true),
        ("Hill Giant", "{3}{R}", "CCCR", true, true),
        ("Craw Wurm", "{4}{G}{G}", "CCCCGG", true, false),
    ] {
        let (t, _) = prohibit(spell, cost, pool, kicked);
        let on_battlefield = t.g.battlefield.iter().any(|o| t.g.obj(*o).chars.name == spell);
        let in_graveyard = t.g.players[0].graveyard.iter().any(|o| t.g.obj(*o).chars.name == spell);
        cite!(["608.2c", "702.33d"]; case: format!("{spell}, kicked {kicked}");
            assert_eq!((in_graveyard, on_battlefield, t.g.stack.is_empty()), (countered, !countered, true)));
    }
}

// Rules: 608.2c, 701.20a
// Given: Lead the Stampede; the top five cards of its caster's library are Grizzly Bears, a
// Mountain, Centaur Courser, an Island and Hill Giant; a Forest is sixth.
// When: it resolves and its caster reveals Grizzly Bears and Hill Giant.
// Expect: the revealed cards go to its caster's hand; the other three go to the bottom, so the
// Forest is on top.
#[test]
fn r608_2c_lead_the_stampede_puts_the_revealed_cards_into_its_casters_hand() {
    let mut t = Board::new(2).hand(0, &["Lead the Stampede"])
        .library(0, &["Grizzly Bears", "Mountain", "Centaur Courser", "Island", "Hill Giant", "Forest"]).build();
    t.mana(0, "{2}{G}").cast(0, "Lead the Stampede", vec![], mana_pool("CCG")).unwrap();
    t.pass_all();
    // "You may reveal any number": whether to is asked first
    if let Some((0, Asked::May)) = t.asked() { t.answer(0, Answer::Yes).unwrap(); }
    let among = match t.asked() {
        Some((0, Asked::Select { among, .. } | Asked::Objects { among, .. })) => among,
        other => panic!("which creature cards to reveal is asked: {other:?}"),
    };
    cite!(["608.2c"]; assert_eq!(names(&t.g, &among), ["Centaur Courser", "Grizzly Bears", "Hill Giant"], "only the creature cards"));
    let picked: Vec<_> = among.iter().copied().filter(|o| t.g.obj(*o).chars.name != "Centaur Courser").collect();
    t.answer(0, Answer::Objects(picked)).unwrap();
    settle(&mut t);
    let hand = t.g.players[0].hand.clone();
    cite!(["608.2c"]; assert_eq!(names(&t.g, &hand), ["Grizzly Bears", "Hill Giant"]));
    let top = *t.g.players[0].library.last().expect("a library");
    t.expect().rules(["401.4"]).holds(|g| g.obj(top).chars.name == "Forest" && g.players[0].library.len() == 4, "the rest are on the bottom").check();
}

// Rules: 608.2h, 118.3
// Given: Airdrop Condor and Goblin Piker (a 2/1 Goblin).
// When: its controller pays {1}{R} and sacrifices Goblin Piker, aiming at player 1.
// Expect: it deals 2 damage, the sacrificed creature's power, though the cost said "a Goblin
// creature".
#[test]
fn r608_2h_airdrop_condor_deals_the_sacrificed_creatures_power() {
    let mut t = Board::new(2).battlefield(0, &["Airdrop Condor", "Goblin Piker"]).build();
    let condor = t.find_in_battlefield("Airdrop Condor");
    let piker = t.find_in_battlefield("Goblin Piker");
    let ability = t.g.abilities(condor).iter().position(|a| matches!(&**a, ir::Ability::Activated { .. })).expect("its activated ability");
    t.mana(0, "{1}{R}");
    t.g.act(0, Move::Paying(mana_pool("CR"), Box::new(Move::Activate { source: condor, ability, targets: vec![vec![Target::Player(1)]], paying: vec![vec![piker]] }))).unwrap();
    t.pass_all();
    t.expect().rules(["608.2h"]).life(1, 18).holds(|g| g.players[0].graveyard.len() == 1, "Goblin Piker was sacrificed").check();
}

// Rules: 608.2h, 601.2f
// Given: Induce Despair aimed at Centaur Courser (3/3); its caster reveals Grizzly Bears (mana
// value 2) or Hill Giant (mana value 4) from their hand as it is cast.
// When: it resolves.
// Expect: the Courser gets -2/-2 and lives, or gets -4/-4 and dies. The revealed card stays in
// its owner's hand.
#[test]
fn r608_2h_induce_despair_reads_the_revealed_cards_mana_value() {
    for (revealed, dies) in [("Grizzly Bears", false), ("Hill Giant", true)] {
        let mut t = Board::new(2).hand(0, &["Induce Despair", revealed]).battlefield(1, &["Centaur Courser"]).build();
        let card = t.find_in(0, Zone::Hand, "Induce Despair");
        let shown = t.find_in(0, Zone::Hand, revealed);
        let courser = t.find_in_battlefield("Centaur Courser");
        t.mana(0, "{2}{B}");
        t.g.act(0, Move::Paying(mana_pool("CCB"), Box::new(Move::WithCosts(vec![vec![shown]],
            Box::new(Move::Cast { card, targets: vec![vec![Target::Object(courser)]] }))))).unwrap();
        t.pass_all();
        let alive = t.g.battlefield.contains(&courser);
        cite!(["608.2h"]; case: revealed.to_string(); assert_eq!((alive, t.g.players[0].hand.contains(&shown)), (!dies, true)));
        if !dies { t.expect().rules(["608.2h"]).pt("Centaur Courser", 1, 1).check(); }
    }
}

// Rules: 608.2h, 603.5
// Given: Feed the Pack and Hurloon Minotaur (2/3) on its controller's side, in their postcombat
// main phase.
// When: their end step begins and they sacrifice the Minotaur, or decline.
// Expect: three 2/2 Wolf tokens, the sacrificed creature's toughness, not its power; declined,
// none, and the Minotaur stays.
#[test]
fn r608_2h_feed_the_pack_counts_the_sacrificed_creatures_toughness() {
    for sacrifice in [true, false] {
        let mut t = Board::new(2).battlefield(0, &["Feed the Pack", "Hurloon Minotaur"]).fill(5).at(Part::PostcombatMain).build();
        let minotaur = t.find_in_battlefield("Hurloon Minotaur");
        // to the end step, and the trigger to resolution
        for _ in 0..4 {
            if matches!(t.asked(), Some((_, Asked::May))) { break; }
            t.pass_all();
        }
        let Some((0, Asked::May)) = t.asked() else { panic!("whether to sacrifice is asked: {:?}", t.asked()) };
        t.answer(0, if sacrifice { Answer::Yes } else { Answer::No }).unwrap();
        if let Some((0, Asked::Objects { .. } | Asked::OptionalObjects { .. })) = t.asked() {
            t.answer(0, Answer::Objects(vec![minotaur])).unwrap();
        }
        settle(&mut t);
        let wolves = t.g.battlefield.iter().filter(|o| t.g.obj(**o).chars.name == "Wolf Token").count();
        cite!(["608.2h", "603.5"]; case: format!("sacrifice {sacrifice}");
            assert_eq!((t.g.battlefield.contains(&minotaur), wolves), (!sacrifice, if sacrifice { 3 } else { 0 })));
    }
}

// Rules: 608.2h, 603.12
// Given: Tip the Scales; its caster controls Hurloon Minotaur (2/3) and Grizzly Bears (2/2), the
// other player Hill Giant (3/3) and Craw Wurm (6/4).
// When: it resolves and its caster sacrifices the Minotaur; the reflexive trigger resolves.
// Expect: all creatures get -3/-3, the sacrificed creature's toughness: the Bears and the Giant
// die, and the Wurm is 3/1.
#[test]
fn r608_2h_tip_the_scales_reads_the_sacrificed_creatures_toughness_when_it_triggers() {
    let mut t = Board::new(2).hand(0, &["Tip the Scales"]).battlefield(0, &["Hurloon Minotaur", "Grizzly Bears"])
        .battlefield(1, &["Hill Giant", "Craw Wurm"]).build();
    let minotaur = t.find_in_battlefield("Hurloon Minotaur");
    t.mana(0, "{2}{B}").cast(0, "Tip the Scales", vec![], mana_pool("CCB")).unwrap();
    t.pass_all();
    match t.asked() {
        Some((0, Asked::Objects { among, .. } | Asked::Select { among, .. })) => assert!(among.contains(&minotaur)),
        other => panic!("which creature to sacrifice is asked: {other:?}"),
    }
    t.answer(0, Answer::Objects(vec![minotaur])).unwrap();
    settle(&mut t);
    let on: Vec<String> = { let mut v: Vec<_> = t.g.battlefield.iter().map(|o| t.g.obj(*o).chars.name.clone()).collect(); v.sort(); v };
    cite!(["608.2h", "603.12"]; assert_eq!(on, ["Craw Wurm"]));
    t.expect().rules(["608.2h"]).pt("Craw Wurm", 3, 1).check();
}

// Rules: 608.2c, 702.33d
// Given: Overload aimed at Mind Stone (mana value 2), Icy Manipulator (4), Gilded Lotus (5) or
// Akroma's Memorial (7), kicked or not.
// When: it resolves.
// Expect: unkicked, it destroys the artifact only if its mana value is 2 or less; kicked, only
// if it is 5 or less.
#[test]
fn r608_2c_overload_destroys_by_mana_value_and_more_when_kicked() {
    for (artifact, kicked, destroyed) in [
        ("Mind Stone", false, true),
        ("Icy Manipulator", false, false),
        ("Icy Manipulator", true, true),
        ("Gilded Lotus", true, true),
        ("Akroma's Memorial", true, false),
    ] {
        let mut t = Board::new(2).hand(0, &["Overload"]).battlefield(1, &[artifact]).build();
        let target = t.find_in_battlefield(artifact);
        let card = t.find_in(0, Zone::Hand, "Overload");
        let (cost, pool) = if kicked { ("{2}{R}", "CCR") } else { ("{R}", "R") };
        t.mana(0, cost);
        t.g.act(0, Move::Paying(mana_pool(pool), Box::new(Move::WithKicks(u32::from(kicked),
            Box::new(Move::Cast { card, targets: vec![vec![Target::Object(target)]] }))))).unwrap();
        settle(&mut t);
        cite!(["608.2c", "702.33d"]; case: format!("{artifact}, kicked {kicked}");
            assert_eq!(t.g.battlefield.contains(&target), !destroyed));
    }
}
