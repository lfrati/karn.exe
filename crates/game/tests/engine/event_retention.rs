use crate::support::cite;
use game::{CardDef, Event, Game, Move, Zone};
use game::state::{Answer, Choice, Decision};

#[test]
fn r106_12a_mana_outcome_survives_retired_events_and_a_paused_child() {
    let defs = vec![
        CardDef::new("Source", "{0}", "Creature", "{T}: Add {G}. Put a +1/+1 counter on this creature.", Some((1, 1))).unwrap(),
        CardDef::new("Scale", "{0}", "Enchantment", "If one or more +1/+1 counters would be put on a creature you control, that many plus one +1/+1 counters are put on it instead.", None).unwrap(),
        CardDef::new("Double", "{0}", "Enchantment", "If one or more +1/+1 counters would be put on a creature you control, twice that many +1/+1 counters are put on that creature instead.", None).unwrap(),
    ];
    for def in &defs { game::play::check(def).unwrap(); }
    let mut game = Game::new(defs, 2, 20);
    let source = game.put(0, 0, Zone::Battlefield);
    game.put(0, 1, Zone::Battlefield);
    game.put(0, 2, Zone::Battlefield);
    game.turn = 2;
    game.act(0, Move::Activate { source, ability: 0, targets: vec![], paying: vec![] }).unwrap();
    let among = match &game.decision {
        Some(Decision::Choose { choice: Choice::Replacement { among, .. }, .. }) => among.clone(),
        other => panic!("replacement question: {other:?}"),
    };
    assert_eq!(game.players[0].pool.total(), 1);
    let acknowledged = game.log.end();
    assert!(game.retire_events(acknowledged) > 0);
    assert!(!game.log.iter().any(|e| matches!(e, Event::ManaAdded { .. })));
    for (mut branch, chosen) in [(game.clone(), among[0].clone()), (game, among[1].clone())] {
        branch.act(0, Move::Choose(Answer::Replacement(chosen))).unwrap();
        assert_eq!(branch.players[0].pool.total(), 1, "committed mana was not replayed");
        assert_eq!(branch.log.iter().filter(|e| matches!(e, Event::TappedForMana { object, .. } if *object == source)).count(), 1);
        branch.retire_events(branch.log.end());
        assert!(branch.log.is_empty());
        assert!(branch.log.end() > acknowledged);
    }
}

#[test]
fn r603_2_retirement_never_discards_events_before_trigger_matching() {
    let watcher = CardDef::new("Watcher", "{0}", "Creature", "Whenever a creature dies, you gain 1 life.", Some((1, 1))).unwrap();
    let mut game = Game::new(vec![watcher], 2, 20);
    let object = game.put(0, 0, Zone::Battlefield);
    game.destroy(object);
    let end = game.log.end();
    assert_eq!(game.retire_events(end), 0);
    game.act(0, Move::Pass).unwrap();
    assert_eq!(game.stack.len(), 1);
    assert!(game.retire_events(end) > 0);
    for _ in 0..2 { game.act(game.priority, Move::Pass).unwrap(); }
    cite!(["603.10a"]; assert_eq!(game.players[0].life, 21, "captured departure trigger survives event retirement"));
}

#[test]
fn event_sequences_do_not_reset_when_delivery_batches_are_retired() {
    let card = CardDef::new("Blank", "", "Land", "", None).unwrap();
    let mut game = Game::new(vec![card], 2, 20);
    for player in 0..2 { for _ in 0..100 { game.put(player, 0, Zone::Library); } }
    let mut previous = game.log.end();
    while game.turn < 40 {
        let (player, action) = match &game.decision {
            None => (game.priority, Move::Pass),
            Some(Decision::Attackers { player }) => (*player, Move::Attack(vec![])),
            Some(Decision::Blockers { player }) => (*player, Move::Block(vec![])),
            Some(Decision::Discard { player, count }) => (*player, Move::Discard(game.players[*player].hand[..*count].to_vec())),
            other => panic!("unexpected question: {other:?}"),
        };
        game.act(player, action).unwrap();
        let end = game.log.end();
        assert!(end >= previous);
        assert!(game.log.len() < 20, "only the current completed response is retained");
        game.retire_events(end);
        if matches!(game.decision, Some(Decision::Discard { .. })) {
            assert_eq!(&*game.log, &[Event::Began { part: ir::Part::Cleanup }], "cleanup event still awaits its discard action");
        } else {
            assert!(game.log.is_empty());
        }
        assert_eq!(game.clone().log.end(), end);
        previous = end;
    }
    assert!(previous.0 > 500);
}

#[test]
fn simulation_retires_preceding_responses_without_reusing_their_sequence_numbers() {
    use game::simulation::Simulation;
    let blank = CardDef::new("Blank", "", "Land", "", None).unwrap();
    let mut game = Game::new(vec![blank], 2, 20);
    for p in 0..2 { for _ in 0..50 { game.put(p, 0, Zone::Library); } }
    let mut sim = Simulation::new(game);
    let mut recorded = Vec::new();
    let mut published = sim.game().unwrap().log.start();
    while sim.game().unwrap().turn < 20 {
        let request = sim.request().unwrap().unwrap().clone();
        let g = sim.game().unwrap();
        recorded.extend_from_slice(g.log.since(published));
        published = g.log.end();
        let action = match &g.decision {
            None => Move::Pass,
            Some(Decision::Attackers { .. }) => Move::Attack(vec![]),
            Some(Decision::Blockers { .. }) => Move::Block(vec![]),
            Some(Decision::Discard { player, count }) => Move::Discard(g.players[*player].hand[..*count].to_vec()),
            other => panic!("unexpected question: {other:?}"),
        };
        sim.respond(request.id, request.player, action).unwrap();
        assert!(sim.game().unwrap().log.len() < 20);
    }
    assert!(recorded.len() > 200, "the host can retain a complete record outside cloned games");
    assert!(sim.game().unwrap().log.start().0 > 200);
}
