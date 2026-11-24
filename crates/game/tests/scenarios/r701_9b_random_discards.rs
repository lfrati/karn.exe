//! "Target player discards two cards at random": no player chooses which cards are discarded
//! (701.9b); they are picked at random among the cards in that hand, each as likely as any
//! other, as the instruction is followed (608.2c). With no more cards than the instruction
//! asks for, every one is discarded and nothing is random.
use crate::support;
use game::simulation::Simulation;
use game::start::{CapturedRandom, RandomKind, RandomOutcome};
use support::*;

fn hand(g: &Game, p: usize) -> Vec<String> {
    let mut names: Vec<_> = g.players[p].hand.iter().map(|o| g.obj(*o).chars.name.clone()).collect();
    names.sort();
    names
}
fn graveyard(g: &Game, p: usize) -> Vec<String> {
    let mut names: Vec<_> = g.players[p].graveyard.iter().map(|o| g.obj(*o).chars.name.clone()).collect();
    names.sort();
    names
}
/// Hymn to Tourach cast at player 1 and passed by its caster: player 1's pass resolves it.
fn hymn(cards: &[&str]) -> T {
    let mut t = Board::new(2).hand(0, &["Hymn to Tourach"]).hand(1, cards).build();
    t.mana(0, "{B}{B}").cast(0, "Hymn to Tourach", vec![player(1)], mana_pool("BB")).unwrap();
    t.g.act(0, Move::Pass).unwrap();
    t
}
/// Two cards discarded at once: their owner orders them in the graveyard (404.3).
fn order_graveyard(sim: &mut Simulation) {
    let request = sim.request().unwrap().unwrap().clone();
    let Asked::OrderCards { cards, .. } = &request.question else { panic!("the graveyard order is asked: {:?}", request.question) };
    let pending = match sim.respond(request.id, request.player, Move::Choose(Answer::Objects(cards.clone()))) {
        Ok(()) => return,
        Err(error) => error,
    };
    panic!("the graveyard order was refused: {pending:?}");
}

// Rules: 701.9b, 608.2c
// Given: Hymn to Tourach aimed at player 1, who holds Lightning Bolt, Giant Growth and a
// Mountain; the random outcome is the order Mountain, Lightning Bolt, Giant Growth.
// When: it resolves.
// Expect: the first two of that order are discarded; no player chose them.
#[test]
fn r701_9b_hymn_to_tourach_discards_the_two_cards_picked_at_random() {
    let t = hymn(&["Lightning Bolt", "Giant Growth", "Mountain"]);
    let [bolt, growth, mountain] = ["Lightning Bolt", "Giant Growth", "Mountain"].map(|n| t.find_in(1, Zone::Hand, n));
    let mut sim = Simulation::new(t.g);
    let request = sim.request().unwrap().unwrap().clone();
    assert_eq!(request.player, 1);
    let receipt = RandomOutcome::RandomOrder { player: Some(1), order: vec![mountain, bolt, growth] };
    let unused = sim.respond_with_random(request.id, 1, Move::Pass, vec![receipt], false).unwrap();
    cite!(["701.9b"]; assert!(unused.is_empty(), "the random pick consumed its outcome"));
    if matches!(sim.request().unwrap().unwrap().question, Asked::OrderCards { .. }) { order_graveyard(&mut sim); }
    let g = sim.game().unwrap();
    cite!(["701.9b", "608.2c"]; assert_eq!((graveyard(g, 1), hand(g, 1)),
        (vec!["Lightning Bolt".to_string(), "Mountain".to_string()], vec!["Giant Growth".to_string()])));
}

// Rules: 701.9b
// Given: Hymn to Tourach aimed at player 1, who holds three cards.
// When: it resolves with the engine's own random numbers.
// Expect: one random outcome of all three cards is drawn, two of them are discarded and one
// stays; no one is asked which.
#[test]
fn r701_9b_hymn_to_tourach_draws_one_random_order_of_the_whole_hand() {
    let t = hymn(&["Lightning Bolt", "Giant Growth", "Mountain"]);
    let mut sim = Simulation::new(t.g);
    let request = sim.request().unwrap().unwrap().clone();
    let captured = sim.respond_capturing_random(request.id, 1, Move::Pass).unwrap();
    let [CapturedRandom::Order { kind: RandomKind::RandomOrder, player: 1, position: None, order }] = captured.as_slice() else {
        panic!("one random order of player 1's cards: {captured:?}")
    };
    cite!(["701.9b"]; assert_eq!(order.len(), 3, "every card of the hand can be picked"));
    let question = sim.request().unwrap().unwrap().question.clone();
    cite!(["701.9b"]; assert!(!matches!(question, Asked::Objects { .. } | Asked::OptionalObjects { .. }), "no one chooses: {question:?}"));
    if matches!(question, Asked::OrderCards { .. }) { order_graveyard(&mut sim); }
    let g = sim.game().unwrap();
    assert_eq!((g.players[1].graveyard.len(), g.players[1].hand.len()), (2, 1));
}

// Rules: 701.9b, 609.3
// Given: Hymn to Tourach aimed at player 1, who holds only Lightning Bolt.
// When: it resolves.
// Expect: the Bolt is discarded and nothing random happens.
#[test]
fn r609_3_hymn_to_tourach_with_one_card_discards_it_without_a_random_outcome() {
    let t = hymn(&["Lightning Bolt"]);
    let mut sim = Simulation::new(t.g);
    let request = sim.request().unwrap().unwrap().clone();
    let captured = sim.respond_capturing_random(request.id, 1, Move::Pass).unwrap();
    cite!(["609.3"]; assert!(captured.is_empty(), "{captured:?}"));
    let g = sim.game().unwrap();
    assert_eq!((graveyard(g, 1), hand(g, 1)), (vec!["Lightning Bolt".to_string(), ], vec![]));
}

// Rules: 701.9b, 101.4
// Given: Burning Inquiry; its caster holds a Mountain, player 1 holds two cards; every
// library holds Wastes.
// When: it resolves.
// Expect: each player draws three cards then discards three at random: the caster all four
// but one, player 1 all five but two; no one is asked which.
#[test]
fn r701_9b_burning_inquiry_each_player_discards_three_at_random() {
    let mut t = Board::new(2).hand(0, &["Burning Inquiry", "Mountain"]).hand(1, &["Lightning Bolt", "Giant Growth"]).fill(5).build();
    t.mana(0, "{R}").cast(0, "Burning Inquiry", vec![], mana_pool("R")).unwrap();
    t.pass_all();
    for _ in 0..8 {
        match t.asked() {
            Some((p, Asked::OrderCards { cards, .. })) => { t.answer(p, Answer::Objects(cards)).unwrap(); }
            Some((_, Asked::Priority)) | None => break,
            Some((p, other)) => panic!("player {p} is asked {other:?}"),
        }
    }
    cite!(["701.9b"]; assert_eq!((t.g.players[0].hand.len(), t.g.players[1].hand.len()), (1, 2)));
    assert_eq!((t.g.players[0].graveyard.len(), t.g.players[1].graveyard.len()), (4, 3), "Burning Inquiry and three discards; three discards");
}

// Rules: 118.12, 701.9b
// Given: Balduvian Horde's enters trigger; its controller holds Lightning Bolt and Giant
// Growth.
// When: its controller chooses to discard.
// Expect: one of the two cards, picked at random, is discarded and the Horde stays.
#[test]
fn r118_12_balduvian_horde_a_random_discard_keeps_it() {
    let mut t = Board::new(2).hand(0, &["Balduvian Horde", "Lightning Bolt", "Giant Growth"]).build();
    t.mana(0, "{2}{R}{R}").cast(0, "Balduvian Horde", vec![], mana_pool("CCRR")).unwrap();
    t.pass_all();
    t.pass_all();
    let Some((0, Asked::May)) = t.asked() else { panic!("whether to discard is asked: {:?}", t.asked()) };
    t.answer(0, Answer::Yes).unwrap();
    t.resolve();
    cite!(["701.9b"]; assert_eq!((t.g.players[0].hand.len(), t.g.players[0].graveyard.len()), (1, 1)));
    t.expect().rules(["118.12"]).zone(0, Zone::Battlefield, &["Balduvian Horde"]).check();
}

/// Pass until a declaration of attackers is asked.
fn to_attackers(t: &mut T) {
    for _ in 0..20 {
        if matches!(t.g.decision, Some(Decision::Attackers { .. })) { return; }
        t.g.act(t.g.decision_maker(t.g.priority), Move::Pass).unwrap();
    }
    panic!("no declaration of attackers");
}

// Rules: 701.9b, 603.5
// Given: Gallia of the Endless Dance and two Grizzly Bears; its controller holds Lightning
// Bolt and Giant Growth.
// When: all three attack and its controller chooses to discard.
// Expect: one of the two cards, picked at random, is discarded (no one picks it), then two
// cards are drawn.
#[test]
fn r701_9b_gallia_may_discard_a_card_at_random_and_then_draws_two() {
    let mut t = Board::new(2).battlefield(0, &["Gallia of the Endless Dance", "Grizzly Bears", "Grizzly Bears"])
        .hand(0, &["Lightning Bolt", "Giant Growth"]).fill(5).build();
    to_attackers(&mut t);
    let attackers: Vec<_> = t.g.battlefield.iter().copied().filter(|o| t.g.controller(*o) == 0).map(|o| (o, Target::Player(1))).collect();
    t.g.act(0, Move::Attack(attackers)).unwrap();
    t.pass_all();
    let Some((0, Asked::May)) = t.asked() else { panic!("whether to discard is asked: {:?}", t.asked()) };
    t.answer(0, Answer::Yes).unwrap();
    cite!(["701.9b"]; assert!(!matches!(t.asked(), Some((_, Asked::Objects { .. } | Asked::OptionalObjects { .. }))), "{:?}", t.asked()));
    t.expect().rules(["701.9b", "603.5"]).holds(|g| g.players[0].graveyard.len() == 1 && g.players[0].hand.len() == 3, "one discarded, two drawn").check();
}

// Rules: 701.9b, 614.1a
// Given: Whispers of Emrakul aimed at player 1, who holds three cards; its caster's graveyard
// holds a land, an instant, a creature and an artifact card, or only a land.
// When: it resolves.
// Expect: with delirium two cards are discarded at random instead of one; without, one.
#[test]
fn r701_9b_whispers_of_emrakul_discards_two_at_random_with_delirium() {
    for (graveyard, discarded) in [(&["Mountain", "Lightning Bolt", "Grizzly Bears", "Sol Ring"][..], 2), (&["Mountain"][..], 1)] {
        let mut t = Board::new(2).hand(0, &["Whispers of Emrakul"]).graveyard(0, graveyard)
            .hand(1, &["Lightning Bolt", "Giant Growth", "Mountain"]).build();
        t.mana(0, "{1}{B}").cast(0, "Whispers of Emrakul", vec![player(1)], mana_pool("CB")).unwrap();
        t.pass_all();
        for _ in 0..3 {
            match t.asked() {
                Some((p, Asked::OrderCards { cards, .. })) => { t.answer(p, Answer::Objects(cards)).unwrap(); }
                _ => break,
            }
        }
        cite!(["701.9b"]; case: format!("{graveyard:?}"); assert_eq!((t.g.players[1].graveyard.len(), t.g.players[1].hand.len()), (discarded, 3 - discarded)));
    }
}

// Rules: 701.9b, 608.2c
// Given: Stupor aimed at player 1, who holds Lightning Bolt, Giant Growth and a Mountain.
// When: it resolves.
// Expect: one card is discarded at random first; then player 1 chooses one of the two left.
#[test]
fn r608_2c_stupor_discards_at_random_then_by_choice() {
    let mut t = Board::new(2).hand(0, &["Stupor"]).hand(1, &["Lightning Bolt", "Giant Growth", "Mountain"]).build();
    t.mana(0, "{2}{B}").cast(0, "Stupor", vec![player(1)], mana_pool("CCB")).unwrap();
    t.pass_all();
    let Some((1, Asked::Objects { among, .. })) = t.asked() else { panic!("player 1 chooses a discard: {:?}", t.asked()) };
    cite!(["608.2c"]; assert_eq!((among.len(), t.g.players[1].graveyard.len()), (2, 1), "one card already discarded at random"));
    t.answer(1, Answer::Objects(vec![among[0]])).unwrap();
    for _ in 0..3 {
        match t.asked() {
            Some((p, Asked::OrderCards { cards, .. })) => { t.answer(p, Answer::Objects(cards)).unwrap(); }
            _ => break,
        }
    }
    t.expect().rules(["701.9b"]).holds(|g| g.players[1].graveyard.len() == 2 && g.players[1].hand.len() == 1, "two discarded").check();
}

// Rules: 701.9b, 608.2c
// Given: Urgoros, the Empty One (4/3) attacks player 1, who holds no cards.
// When: it deals combat damage to player 1.
// Expect: player 1 can't discard a card at random, so Urgoros's controller draws a card.
#[test]
fn r608_2c_urgoros_draws_when_the_player_cant_discard() {
    let mut t = Board::new(2).battlefield(0, &["Urgoros, the Empty One"]).fill(5).build();
    to_attackers(&mut t);
    let urgoros = t.find_in_battlefield("Urgoros, the Empty One");
    t.g.act(0, Move::Attack(vec![(urgoros, Target::Player(1))])).unwrap();
    for _ in 0..20 {
        if t.g.players[0].hand.len() == 1 { break; }
        match t.g.decision.clone() {
            Some(Decision::Blockers { player }) => { t.g.act(player, Move::Block(vec![])).unwrap(); }
            _ => { t.g.act(t.g.decision_maker(t.g.priority), Move::Pass).unwrap(); }
        }
    }
    t.expect().rules(["608.2c"]).life(1, 16).holds(|g| g.players[0].hand.len() == 1, "its controller drew").check();
}
