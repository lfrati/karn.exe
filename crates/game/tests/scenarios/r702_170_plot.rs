//! Plot pays now without using the stack; its owner casts later under a separate permission.
use crate::support::cite;
use game::state::Answer;
use game::{CardDef, Event, Game, Move, ObjId, Zone};
use ir::Part;
fn setup() -> Game {
    let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let defs = vec![
        CardDef::from_face(faces.iter().find(|f| f.face_name == "Highway Robbery").unwrap()).unwrap(),
        CardDef::new("Mountain", "", "Basic Land — Mountain", "", None).unwrap(),
        CardDef::new("Plotted instant", "{4}{R}", "Instant", "You gain 1 life.\nPlot {1}{R}", None).unwrap(),
        CardDef::new("Tax", "{0}", "Artifact", "Noncreature spells cost {1} more to cast.", None).unwrap(),
        CardDef::new("Flash watcher", "{0}", "Artifact", "Whenever you cast a spell, you gain 1 life.", None).unwrap(),
    ];
    let mut defs = defs;
    defs.push(CardDef::new("Mana creature", "{0}", "Creature", "{T}: Put a +1/+1 counter on this creature. Add X mana of any one color, where X is the number of +1/+1 counters on this creature.", Some((1,1))).unwrap());
    defs.push(CardDef::new("Dredger", "{0}", "Creature", "Dredge 1", Some((1, 1))).unwrap());
    defs.push(
        CardDef::new("Plot watcher", "{0}", "Artifact", "Whenever you plot a card, you gain 1 life.", None).unwrap(),
    );
    for d in &defs {
        game::play::check(d).unwrap_or_else(|e| panic!("{}: {e:?}", d.name()));
    }
    let mut g = Game::new(defs, 2, 20);
    for p in 0..2 {
        for _ in 0..12 {
            g.put(p, 1, Zone::Library);
        }
    }
    g
}
fn passes(g: &mut Game) {
    for _ in 0..2 {
        g.act(g.priority, Move::Pass).unwrap();
    }
}
fn plot(g: &mut Game, def: usize) -> ObjId {
    let card = g.put(0, def, Zone::Hand);
    g.players[0].pool.colors[3] = 2;
    g.act(0, crate::support::paid("RR",Move::Plot(card))).unwrap();
    *g.exile.last().unwrap()
}
fn next_own_main(g: &mut Game) {
    let turn = g.turn;
    while g.turn == turn || g.active != 0 || g.part != Part::PrecombatMain {
        g.act(g.priority, Move::Pass).unwrap();
    }
}
#[test]
fn r702_170a_plot_pays_now_without_casting_and_the_action_retains_priority() {
    let mut g = setup();
    g.put(0, 4, Zone::Battlefield);
    let card = plot(&mut g, 0);
    cite!(["702.170a"]; assert_eq!(g.players[0].pool.total(), 0));
    cite!(["702.170b"]; assert!(g.stack.is_empty()));
    cite!(["116.3"]; assert_eq!(g.priority, 0));
    assert_eq!(g.log.iter().filter(|e| matches!(e, Event::Plotted { .. })).count(), 1);
    cite!(["702.170b"]; assert!(!g.log.iter().any(|e| matches!(e, Event::Cast { .. } | Event::Activated { .. }))));
    cite!(["702.170d"]; refused("the effect that lets you cast that card doesn't yet (601.3)"): g.act(0, Move::Cast { card, targets: vec![] }));
    assert_eq!(g.players[0].life, 20);
}
#[test]
fn r702_170d_the_plotted_spell_is_free_on_a_later_turn_and_resolves_normally() {
    let mut g = setup();
    let card = plot(&mut g, 0);
    next_own_main(&mut g);
    assert_eq!(g.players[0].pool.total(), 0);
    // Give the discard a real choice before its options are captured on resolution.
    g.put(0, 2, Zone::Hand);
    cite!(["702.170d"]; ok: g.act(0, Move::Cast { card, targets: vec![] }));
    passes(&mut g);
    let discard = g.players[0].hand[0];
    let hand = g.players[0].hand.len();
    let library = g.players[0].library.len();
    g.act(0, Move::Choose(Answer::Alternative(0))).unwrap();
    assert!(matches!(g.decision, Some(game::state::Decision::Choose { player: 0, choice: game::state::Choice::Objects { .. } })));
    g.act(0, Move::Choose(Answer::Objects(vec![discard]))).unwrap();
    assert_eq!(g.obj(discard).zone, Zone::Gone);
    assert!(g.players[0].graveyard.iter().any(|o| g.obj(*o).card == Some(1)));
    assert_eq!(g.players[0].hand.len(), hand - 1 + 2);
    assert_eq!(g.players[0].library.len(), library - 2);
    assert!(g.players[0].graveyard.iter().any(|o| g.obj(*o).card == Some(0)));
    assert!(g.exile.is_empty());
}
#[test]
fn r702_170d_plot_casting_requires_sorcery_timing_even_for_an_instant() {
    let mut g = setup();
    let card = plot(&mut g, 2);
    while g.active == 0 {
        g.act(g.priority, Move::Pass).unwrap();
    }
    while g.priority != 0 {
        g.act(g.priority, Move::Pass).unwrap();
    }
    cite!(["702.170d"]; refused("the effect that lets you cast that card doesn't yet (601.3)"): g.act(0, Move::Cast { card, targets: vec![] }));
    next_own_main(&mut g);
    cite!(["702.170d"]; ok: g.act(0, Move::Cast { card, targets: vec![] }));
    passes(&mut g);
    assert_eq!(g.players[0].life, 21);
}
#[test]
fn r702_170a_unpaid_or_wrong_timing_plot_proposals_leave_the_card_in_hand() {
    let mut g = setup();
    let card = g.put(0, 0, Zone::Hand);
    cite!(["118.3a"]; refused("your mana pool cannot pay that cost (118.3a): {1}{R}"): g.act(0, Move::Plot(card)));
    assert!(g.players[0].hand.contains(&card));
    assert!(g.exile.is_empty());
    g.players[0].pool.colors[3] = 2;
    let spell = g.put(0, 2, Zone::Hand);
    g.players[0].pool.colors[3] = 5;
    g.act(0, crate::support::paid("RRRRR",Move::Cast { card: spell, targets: vec![] })).unwrap();
    g.players[0].pool.colors[3] = 2;
    cite!(["702.170a"]; refused("plot only in your main phase with an empty stack (702.170a)"): g.act(0, Move::Plot(card)));
    assert!(g.players[0].hand.contains(&card));
}
#[test]
fn r118_9d_plot_cost_is_not_a_spell_cost_but_later_free_casting_pays_increases() {
    let mut g = setup();
    g.put(0, 3, Zone::Battlefield);
    let card = plot(&mut g, 2);
    next_own_main(&mut g);
    cite!(["601.2f"]; refused("your mana pool cannot pay that cost (118.3a): {1} in total, {1} more than it was (601.2f)"): g.act(0, Move::Cast { card, targets: vec![] }));
    g.players[0].pool.colors[3] = 1;
    g.act(0, crate::support::paid("R",Move::Cast { card, targets: vec![] })).unwrap();
    cite!(["118.9d"]; assert_eq!(g.players[0].pool.total(), 0));
    passes(&mut g);
    assert_eq!(g.players[0].life, 21);
}
#[test]
fn r400_7_plot_permission_belongs_to_one_exile_incarnation_and_clones_are_independent() {
    let mut g = setup();
    let card = plot(&mut g, 2);
    let mut copy = g.clone();
    g.move_object(card, Zone::Hand);
    let hand = *g.players[0].hand.last().unwrap();
    g.exile(hand);
    let new = *g.exile.last().unwrap();
    next_own_main(&mut g);
    cite!(["400.7"]; refused("that card is not in your hand: it is in exile, and no rule or effect lets you cast it from there (601.3)"): g.act(0, Move::Cast { card: new, targets: vec![] }));
    next_own_main(&mut copy);
    copy.act(0, Move::Cast { card, targets: vec![] }).unwrap();
    passes(&mut copy);
    assert_eq!(copy.players[0].life, 21);
}
#[test]
fn r702_170a_explicit_mana_abilities_pay_the_plot_cost() {
    let mut g = setup();
    let card = g.put(0, 0, Zone::Hand);
    let first = g.put(0, 1, Zone::Battlefield);
    let second = g.put(0, 1, Zone::Battlefield);
    let plan = [first, second].into_iter().map(|source| game::legal::Step { plan: vec![], x: None, life: vec![], with: None,
        source, ability: 0, paying: vec![], adding: vec![], chose: vec![], cost_branches: None,
    }).collect();
    cite!(["702.170a"]; ok: g.act(0, crate::support::paid("RR",Move::WithMana(plan, Box::new(Move::Plot(card))))));
    cite!(["702.170b"]; assert!(g.stack.is_empty()));
    assert_eq!(g.exile.len(), 1);
}

#[test]
fn r702_170a_plot_resumes_after_a_mana_child_without_repeating_exile_or_payment() {
    let mut g = setup();
    let card = g.put(0, 0, Zone::Hand);
    let mana = g.put(0, 5, Zone::Battlefield);
    g.put_counters(mana, "+1/+1", 1);
    g.put(0, 7, Zone::Battlefield);
    let plan = vec![game::legal::Step { plan: vec![], x: None, life: vec![], with: None, cost_branches: None, chose: vec![],  source: mana, ability: 0, paying: vec![], adding: vec![] }];
    g.act(0, Move::WithMana(plan, Box::new(Move::Plot(card)))).unwrap();
    assert!(g.decision.is_some());
    crate::support::refused(g.act(0, Move::Pass), "a decision is pending: a choice of the effect being applied (608.2d)");
    assert!(!g.log.iter().any(|e| matches!(e, Event::Plotted { .. })));
    let mut copy = g.clone();
    for g in [&mut g, &mut copy] {
        g.act(0, Move::Choose(Answer::Mana(vec![ir::mana::ManaSymbol::Color(ir::Color::Red); 2]))).unwrap();
        g.act(0, Move::FinishPayment { plan: vec![], paying: vec![], with: Some(crate::support::mana_pool("RR")) }).unwrap();
        assert_eq!(g.exile.len(), 1);
        cite!(["702.170a"]; assert_eq!(g.players[0].pool.total(), 0));
        assert_eq!(g.log.iter().filter(|e| matches!(e, Event::Plotted { .. })).count(), 1);
        assert_eq!(g.stack.len(), 1);
        passes(g);
        cite!(["702.170e"]; assert_eq!(g.players[0].life, 21));
    }
}

#[test]
fn r733_1_reversing_an_unfinished_plot_restores_hand_but_keeps_its_mana_child() {
    let mut g = setup();
    let card = g.put(0, 0, Zone::Hand);
    let mana = g.put(0, 5, Zone::Battlefield);
    g.put_counters(mana, "+1/+1", 1);
    let plan = vec![game::legal::Step { plan: vec![], x: None, life: vec![], with: None, cost_branches: None, chose: vec![],  source: mana, ability: 0, paying: vec![], adding: vec![] }];
    g.act(0, Move::WithMana(plan, Box::new(Move::Plot(card)))).unwrap();
    let exiled = g.exile[0];
    g.act(0, Move::Choose(Answer::Mana(vec![ir::mana::ManaSymbol::Color(ir::Color::Green); 2]))).unwrap();
    assert!(matches!(g.decision, Some(game::state::Decision::Payment { .. })));
    assert_eq!(g.plotted(exiled), None);
    crate::support::refused(g.act(0, Move::Pass), "a decision is pending: complete the announced payment (601.2g-h)");
    let mut copy = g.clone();
    for g in [&mut g, &mut copy] {
        g.act(0, Move::ReversePayment).unwrap();
        assert!(g.players[0].hand.contains(&card));
        assert!(g.exile.is_empty());
        assert_eq!(g.priority, 0);
        assert_eq!(g.players[0].pool.colors[4], 2);
        assert!(g.obj(mana).tapped);
        assert_eq!(g.obj(mana).counters["+1/+1"], 2);
        assert!(!g.log.iter().any(|e| matches!(e, Event::Plotted { .. })));
        assert!(g.stack.is_empty());
    }
}

#[test]
fn r118_9_free_plot_casting_keeps_mandatory_additional_costs_and_sets_x_to_zero() {
    let def = CardDef::new(
        "Plotted payment",
        "{X}{R}",
        "Sorcery",
        "As an additional cost to cast this spell, discard a card.\nYou gain X life.\nPlot {1}{R}",
        None,
    )
    .unwrap();
    game::play::check(&def).unwrap();
    let blank = CardDef::new("Mountain", "", "Basic Land — Mountain", "", None).unwrap();
    let mut g = Game::new(vec![def, blank], 2, 20);
    for p in 0..2 {
        for _ in 0..12 {
            g.put(p, 1, Zone::Library);
        }
    }
    let card = plot(&mut g, 0);
    next_own_main(&mut g);
    cite!(["601.2h"]; refused("that selection does not satisfy the cost, which asks for exactly 1 card(s): 0 chosen (601.2h, 701.9a)"): g.act(0, Move::Cast { card, targets: vec![] }));
    let discard = g.players[0].hand[0];
    let paid = Move::WithCosts(vec![vec![discard]], Box::new(Move::Cast { card, targets: vec![] }));
    cite!(["107.3b"]; refused("X is 0 for a spell cast without paying its mana cost (107.3b)"): g.act(0, Move::WithX(3, Box::new(paid.clone()))));
    g.act(0, paid).unwrap();
    passes(&mut g);
    cite!(["107.3b"]; assert_eq!(g.players[0].life, 20));
    cite!(["118.9d"]; assert_eq!(g.players[0].graveyard.len(), 2));
}
