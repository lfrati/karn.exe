//! Review of "you may choose a card" (608.2d): "if you don't" when it can't be done, a card put
//! from exile into a graveyard that a replacement keeps in exile (its ruling), two cards of two
//! opponents, an ability whose target is gone or never was (603.3d, 608.2b), a reflexive
//! trigger left without a target (603.12), and what a card put onto the battlefield does next.
//! Every card is read from its Oracle text.
use crate::support::*;
use game::legal::Asked;
use game::state::Decision;

#[test]
fn the_cards_of_these_tests_run() {
    for name in ["Entrails Feaster", "Mind Raker", "Rest in Peace", "Ulamog's Nullifier", "Masked Vandal", "Master Skald", "Young Necromancer",
        "Heated Argument", "Unsummon", "Grasping Tentacles", "Emperor of Bones", "Serra Angel", "Grizzly Bears", "Hill Giant", "Lightning Bolt",
        "Shock", "Sol Ring", "Ornithopter", "Mountain", "Island"] {
        // Uncited (admission): the engine runs each card of these tests.
        assert_eq!(game::play::check(&oracle(name)).err().map(|u| u.0), None, "{name}");
    }
}

/// The names of the cards in that player's zone, sorted.
fn names(t: &T, p: usize, zone: Zone) -> Vec<String> {
    let cards: Vec<ObjId> = match zone {
        Zone::Graveyard => t.g.players[p].graveyard.clone(),
        Zone::Exile => t.g.exile.iter().copied().filter(|o| t.g.obj(*o).owner == p).collect(),
        _ => unreachable!(),
    };
    let mut names: Vec<String> = cards.iter().map(|o| t.g.obj(*o).chars.name.clone()).collect();
    names.sort();
    names
}

/// Player 0 casts this card from their hand, paying `mana`, until a question or priority.
fn cast(t: &mut T, name: &str, targets: Vec<Tgt>, mana: &str) -> Boundary {
    let units: String = mana.trim_matches(|c| c == '{' || c == '}').split("}{").map(|s| match s.parse::<usize>() { Ok(n) => "C".repeat(n), Err(_) => s.to_string() }).collect();
    t.mana(0, mana).cast(0, name, targets, mana_pool(&units)).unwrap();
    t.settle()
}

/// What player 0 is offered to choose, `count` of them or none.
fn offered(boundary: Boundary, count: usize) -> Vec<ObjId> {
    match boundary {
        Boundary::Question(0, Asked::OptionalObjects { among, count: n, .. }) if n == count => among,
        other => panic!("expected an optional choice of {count}: {other:?}"),
    }
}

// Rules: 118.12, 608.2d
// Given: player 0's Entrails Feaster ("At the beginning of your upkeep, you may exile a creature
// card from a graveyard. If you do, put a +1/+1 counter on this creature. If you don't, tap this
// creature."); Grizzly Bears in player 1's graveyard, or no creature card anywhere.
// When: player 0's upkeep begins and they exile the Bears, decline, or have nothing to exile.
// Expect: exiling it, the Feaster gets a counter and stays untapped; declining, or with nothing
// to exile, it isn't done (118.12): the Feaster is tapped.
#[test]
fn r118_12_entrails_feaster_is_tapped_when_it_exiles_nothing() {
    for (bears, yes, counter, tapped) in [(true, true, 1, false), (true, false, 0, true), (false, true, 0, true)] {
        let graveyard: &[&str] = if bears { &["Grizzly Bears"] } else { &[] };
        let mut t = Board::new(2).fill(8).active(1).at(Part::End).battlefield(0, &["Entrails Feaster"]).graveyard(1, graveyard).build();
        let feaster = t.find_in_battlefield("Entrails Feaster");
        let mut asked = false;
        while !(t.g.active == 0 && t.g.part == Part::Draw) {
            match t.asked() {
                Some((p, Asked::OptionalObjects { among, .. })) => {
                    asked = true;
                    t.answer(p, Answer::OptionalObjects(yes.then(|| vec![among[0]]))).unwrap();
                }
                Some((p, Asked::Priority)) => { t.g.act(p, Move::Pass).unwrap(); }
                other => panic!("unexpected {other:?}"),
            }
        }
        let case = format!("bears {bears}, yes {yes}");
        cite!(["608.2d"]; case: case.clone(); assert_eq!(asked, bears));
        cite!(["118.12"]; case: case.clone(); assert_eq!((t.g.counters(feaster).get("+1/+1").copied().unwrap_or(0), t.g.obj(feaster).tapped), (counter, tapped)));
        let exiled: &[&str] = if bears && yes { &["Grizzly Bears"] } else { &[] };
        cite!(["608.2d"]; case: case; assert_eq!(names(&t, 1, Zone::Exile), exiled));
    }
}

// Rules: 608.2d, 614.1a, 400.7
// Given: player 0's Rest in Peace ("If a card or token would be put into a graveyard from
// anywhere, exile it instead."); player 1's Lightning Bolt in exile and Grizzly Bears in hand.
// When: player 0 casts Mind Raker ("you may put a card an opponent owns from exile into that
// player's graveyard. If you do, each opponent discards a card.") and puts the Bolt away.
// Expect: the Bolt is exiled instead, a new object (400.7), and it was done (its ruling): player 1
// discards the Bears, which is exiled too.
#[test]
fn r608_2d_mind_raker_puts_a_card_away_though_it_is_exiled_instead() {
    let mut t = Board::new(2).fill(5).hand(0, &["Mind Raker"]).battlefield(0, &["Rest in Peace"]).exile(1, &["Lightning Bolt"]).hand(1, &["Grizzly Bears"]).build();
    let bolt = t.find_in(1, Zone::Exile, "Lightning Bolt");
    cite!(["608.2d"]; assert_eq!(offered(cast(&mut t, "Mind Raker", vec![], "{3}{B}"), 1), vec![bolt]));
    t.answer(0, Answer::OptionalObjects(Some(vec![bolt]))).unwrap();
    t.settle();
    cite!(["400.7", "614.1a"]; assert!(!t.g.exile.contains(&bolt)));
    t.expect().rules(["608.2d", "614.1a"]).zone(1, Zone::Exile, &["Grizzly Bears", "Lightning Bolt"]).zone(1, Zone::Hand, &[]).zone(1, Zone::Graveyard, &[]).check();
}

// Rules: 608.2d, 603.3d, 701.6a
// Given: player 1 has cast Hill Giant; Lightning Bolt in player 1's exile, and Shock in player 2's
// or no third player.
// When: player 0 casts Ulamog's Nullifier ("When this creature enters, you may put two cards your
// opponents own from exile into their owners' graveyards. If you do, counter target spell.")
// at the Giant.
// Expect: with three players, the two cards may be of two opponents (its ruling), each goes to
// its owner's graveyard, and the Giant is countered; with one card, nothing is asked and the
// Giant resolves.
#[test]
fn r608_2d_ulamogs_nullifier_puts_two_cards_of_two_opponents_away_or_none() {
    for players in [3, 2] {
        let mut board = Board::new(players).fill(5).active(1).hand(0, &["Ulamog's Nullifier"]).hand(1, &["Hill Giant"]).exile(1, &["Lightning Bolt"]);
        if players == 3 { board = board.exile(2, &["Shock"]); }
        let mut t = board.build();
        t.mana(1, "{3}{R}").cast(1, "Hill Giant", vec![], mana_pool("CCCR")).unwrap();
        let giant = *t.g.stack.last().unwrap();
        while t.g.decision_maker(t.g.priority) != 0 { let p = t.g.decision_maker(t.g.priority); t.g.act(p, Move::Pass).unwrap(); }
        t.mana(0, "{2}{U}{B}").cast(0, "Ulamog's Nullifier", vec![], mana_pool("CCUB")).unwrap();
        let mut asked = false;
        loop {
            match t.asked() {
                Some((p, Asked::Targets { .. })) => { t.g.act(p, Move::ChooseTargets(vec![vec![Target::Object(giant)]])).unwrap(); }
                Some((p, Asked::OptionalObjects { among, count: 2, .. })) => { asked = true; t.answer(p, Answer::OptionalObjects(Some(among))).unwrap(); }
                Some((p, Asked::Priority)) if !t.g.stack.is_empty() => { t.g.act(p, Move::Pass).unwrap(); }
                Some((_, Asked::Priority)) => break,
                other => panic!("unexpected {other:?}"),
            }
        }
        cite!(["608.2d"]; case: format!("{players} players"); assert_eq!(asked, players == 3));
        if players == 3 {
            t.expect().rules(["608.2d", "701.6a"]).zone(1, Zone::Graveyard, &["Hill Giant", "Lightning Bolt"]).zone(2, Zone::Graveyard, &["Shock"]).check();
        } else {
            t.expect().rules(["608.2d", "118.12"]).zone(1, Zone::Battlefield, &["Hill Giant"]).zone(1, Zone::Exile, &["Lightning Bolt"]).check();
        }
    }
}

// Rules: 603.3d
// Given: Grizzly Bears and Hill Giant in player 0's graveyard; no artifact or enchantment anywhere.
// When: player 0 casts Masked Vandal ("you may exile a creature card from your graveyard. If you
// do, exile target artifact or enchantment an opponent controls.") and Master Skald ("... If you
// do, return target artifact or enchantment card from your graveyard to your hand.").
// Expect: neither trigger has a target, so neither is put on the stack (603.3d): nothing is
// asked and no card is exiled.
#[test]
fn r603_3d_an_ability_with_no_target_exiles_no_card() {
    let mut t = Board::new(2).fill(5).hand(0, &["Masked Vandal", "Master Skald"]).graveyard(0, &["Grizzly Bears", "Hill Giant"]).build();
    for (name, cost) in [("Masked Vandal", "{1}{G}"), ("Master Skald", "{4}{W}")] {
        let boundary = cast(&mut t, name, vec![], cost);
        cite!(["603.3d"]; case: name.to_string(); assert!(matches!(boundary, Boundary::Priority(0)), "{boundary:?}"));
        t.expect().rules(["603.3d"]).zone(0, Zone::Graveyard, &["Grizzly Bears", "Hill Giant"]).zone(0, Zone::Exile, &[]).check();
    }
}

// Rules: 603.12, 603.3d
// Given: Grizzly Bears, Mountain and Island in player 0's graveyard.
// When: player 0 casts Young Necromancer ("you may exile two cards from your graveyard. When you
// do, return target creature card from your graveyard to the battlefield.") and exiles the Bears
// and the Mountain.
// Expect: the reflexive trigger has no creature card to target (603.12, 603.3d): nothing
// returns, and the Island stays.
#[test]
fn r603_12_young_necromancer_exiling_the_creature_card_returns_nothing() {
    let mut t = Board::new(2).fill(5).hand(0, &["Young Necromancer"]).graveyard(0, &["Grizzly Bears", "Mountain", "Island"]).build();
    let (bears, mountain) = (t.find_in(0, Zone::Graveyard, "Grizzly Bears"), t.find_in(0, Zone::Graveyard, "Mountain"));
    cite!(["608.2d"]; assert_eq!(offered(cast(&mut t, "Young Necromancer", vec![], "{4}{B}"), 2).len(), 3));
    t.answer(0, Answer::OptionalObjects(Some(vec![bears, mountain]))).unwrap();
    let boundary = t.settle();
    cite!(["603.12", "603.3d"]; assert!(matches!(boundary, Boundary::Priority(0)), "{boundary:?}"));
    t.expect().rules(["603.12"]).zone(0, Zone::Graveyard, &["Island"]).zone(0, Zone::Exile, &["Grizzly Bears", "Mountain"])
        .zone(0, Zone::Battlefield, &["Young Necromancer"]).check();
}

// Rules: 608.2b
// Given: player 1's Serra Angel; Mountain in player 0's graveyard.
// When: player 0 casts Heated Argument at the Angel, and player 1 Unsummons it in response.
// Expect: its only target is illegal, so it doesn't resolve (its ruling): nothing is asked, the
// Mountain stays, and player 1 is dealt no damage.
#[test]
fn r608_2b_heated_argument_without_its_target_exiles_nothing() {
    let mut t = Board::new(2).fill(5).hand(0, &["Heated Argument"]).hand(1, &["Unsummon"]).graveyard(0, &["Mountain"]).battlefield(1, &["Serra Angel"]).build();
    let angel = t.find_in_battlefield("Serra Angel");
    t.mana(0, "{4}{R}").cast(0, "Heated Argument", vec![Tgt::Object(angel)], mana_pool("CCCCR")).unwrap();
    t.g.act(0, Move::Pass).unwrap();
    t.mana(1, "{U}").cast(1, "Unsummon", vec![Tgt::Object(angel)], mana_pool("U")).unwrap();
    let boundary = t.settle();
    cite!(["608.2b"]; assert!(matches!(boundary, Boundary::Priority(0)), "{boundary:?}"));
    t.expect().rules(["608.2b"]).zone(0, Zone::Graveyard, &["Heated Argument", "Mountain"]).zone(1, Zone::Hand, &["Serra Angel"]).life(1, 20).check();
}

// Rules: 608.2d, 110.2
// Given: player 1's library of Sol Ring, Grizzly Bears, Mountain, Ornithopter and Islands; an
// Ornithopter in player 0's graveyard.
// When: player 0 casts Grasping Tentacles ("Target opponent mills eight cards. You may put an
// artifact card from that player's graveyard onto the battlefield under your control.") at
// player 1 and takes the Sol Ring.
// Expect: the artifacts offered are player 1's only; the Ring is player 0's to control and
// still player 1's (110.2).
#[test]
fn r608_2d_grasping_tentacles_takes_an_artifact_of_that_players_graveyard() {
    let mut t = Board::new(2).fill(0).hand(0, &["Grasping Tentacles"]).library(0, &["Island", "Island", "Island"])
        .library(1, &["Sol Ring", "Grizzly Bears", "Mountain", "Ornithopter", "Island", "Island", "Island", "Island", "Island", "Island"])
        .graveyard(0, &["Ornithopter"]).build();
    let mut boundary = cast(&mut t, "Grasping Tentacles", vec![Tgt::Player(1)], "{1}{U}{B}");
    // the milled cards go to the graveyard in the order player 1 chooses
    if let Boundary::Question(1, Asked::OrderCards { cards, .. }) = boundary {
        t.answer(1, Answer::Objects(cards)).unwrap();
        boundary = t.settle();
    }
    let among = offered(boundary, 1);
    let mut theirs = vec![t.find_in(1, Zone::Graveyard, "Sol Ring"), t.find_in(1, Zone::Graveyard, "Ornithopter")];
    let mut got = among.clone();
    theirs.sort();
    got.sort();
    cite!(["608.2d"]; assert_eq!(got, theirs));
    let ring = t.find_in(1, Zone::Graveyard, "Sol Ring");
    t.answer(0, Answer::OptionalObjects(Some(vec![ring]))).unwrap();
    t.settle();
    let ring = t.find_in(0, Zone::Battlefield, "Sol Ring");
    cite!(["110.2"]; assert_eq!((t.g.controller(ring), t.g.obj(ring).owner), (0, 1)));
}

// Rules: 122.1h, 603.7
// Given: player 0's Emperor of Bones; Grizzly Bears in player 1's graveyard.
// When: at the beginning of combat it exiles the Bears; player 0 adapts it, putting the Bears onto
// the battlefield ("with a finality counter on it. It gains haste. Sacrifice it at the beginning
// of the next end step."), and the turn goes on.
// Expect: the Bears has haste; sacrificed at the end step, its finality counter exiles it
// instead (122.1h).
#[test]
fn r122_1h_emperor_of_bones_returns_a_creature_with_haste_and_exiles_it_at_the_end_step() {
    let mut t = Board::new(2).fill(5).battlefield(0, &["Emperor of Bones"]).graveyard(1, &["Grizzly Bears"]).build();
    let bears = t.find_in(1, Zone::Graveyard, "Grizzly Bears");
    t.g.act(0, Move::Pass).unwrap();
    t.g.act(1, Move::Pass).unwrap();
    t.g.act(0, Move::ChooseTargets(vec![vec![Target::Object(bears)]])).unwrap();
    t.resolve();
    let exiled = t.find_in(1, Zone::Exile, "Grizzly Bears");
    t.mana(0, "{1}{B}").activate(0, "Emperor of Bones", 1, vec![], mana_pool("{1}{B}")).unwrap();
    if let Boundary::Question(0, Asked::Objects { .. }) = t.settle() { t.answer(0, Answer::Objects(vec![exiled])).unwrap(); }
    t.settle();
    let returned = t.find_in(0, Zone::Battlefield, "Grizzly Bears");
    cite!(["611.2c"]; assert!(t.g.has(returned, "Haste")));
    while t.g.active == 0 {
        match t.g.decision.clone() {
            Some(Decision::Attackers { player }) => { t.g.act(player, Move::Attack(vec![])).unwrap(); }
            Some(Decision::Order { player, abilities }) => { t.g.act(player, Move::Order((0..abilities.len()).collect())).unwrap(); }
            _ => { let p = t.g.decision_maker(t.g.priority); t.g.act(p, Move::Pass).unwrap(); }
        }
    }
    t.expect().rules(["122.1h", "603.7"]).zone(1, Zone::Exile, &["Grizzly Bears"]).zone(1, Zone::Graveyard, &[]).zone(0, Zone::Battlefield, &["Emperor of Bones"]).check();
}
