//! Coin-flip instructions must produce their own called result (705.2), not a generic
//! number or a prior instruction's outcome. Expected behavior is pinned Oracle/CR.
use crate::support::{Board, T, cite};
use game::{Move, Zone};
use game::start::{CoinFace, RandomOutcome};

fn crypt() -> T {
    Board::new(2).battlefield(0, &["Mana Crypt"]).fill(8).build()
}

#[test]
fn r705_2_mana_crypt_is_admitted_as_a_complete_card() {
    let mut t = crypt();
    let source = t.find_in(0, Zone::Battlefield, "Mana Crypt");
    // Its full mana ability is also usable; the upkeep ability is not discarded to
    // make a partial card appear runnable.
    t.g.act(0, Move::Activate {source, ability: 1, targets: vec![], paying: vec![]}).unwrap();
    cite!(["106.4", "605.3b"]; assert_eq!(t.g.players[0].pool.colorless, 2));
}

fn at_call() -> game::simulation::Simulation {
    let mut t = crypt();
    for _ in 0..400 {
        if matches!(t.g.asked(), Some((0, game::legal::Asked::CoinCall))) {
            return game::simulation::Simulation::new(t.g);
        }
        match t.g.decision.clone() {
            Some(game::state::Decision::Attackers {player}) => t.g.act(player, Move::Attack(vec![])).unwrap(),
            Some(game::state::Decision::Blockers {player}) => t.g.act(player, Move::Block(vec![])).unwrap(),
            _ => t.g.act(t.g.priority, Move::Pass).unwrap(),
        }
    }
    panic!("Mana Crypt never asked its controller for a call");
}

#[test]
fn r705_2_only_the_calling_player_loses_a_mismatched_flip_and_crypt_only_damages_on_loss() {
    let before = at_call();
    for (call, face, life) in [(CoinFace::Heads, CoinFace::Heads, 20), (CoinFace::Heads, CoinFace::Tails, 17),
                              (CoinFace::Tails, CoinFace::Tails, 20), (CoinFace::Tails, CoinFace::Heads, 17)] {
        // Fork while suspended before the physical result. Each branch gets its own
        // trusted outcome and executes the producer/consumer once.
        let mut sim = before.clone();
        let request = sim.request().unwrap().unwrap().clone();
        cite!(["705.2"]; assert_eq!(sim.game().unwrap().players[0].life, 20));
        assert!(!sim.game().unwrap().log.iter().any(|e|matches!(e,game::Event::CoinFlipped {..})));
        sim.respond_with_random(request.id, 0, Move::Choose(game::state::Answer::CoinCall(call)),
            vec![RandomOutcome::Coin {player: 0, face}], false).unwrap();
        let g = sim.game().unwrap();
        cite!(["705.2", "120.3a"]; assert_eq!(g.players[0].life, life));
        cite!(["705.2"]; assert_eq!(g.players[1].life, 20));
        let results = g.log.iter().filter_map(|e|match e {game::Event::CoinFlipped {result,..}=>Some(result),_=>None}).collect::<Vec<_>>();
        assert_eq!(results.len(), 1);
        cite!(["705.2"]; assert_eq!(results[0].won(), Some(call == face)));
        assert_eq!(results[0].player, 0);
        assert!(g.stack.is_empty());
    }
}

#[test]
fn bad_coin_receipts_restore_the_pending_call_and_a_corrected_receipt_resumes() {
    let start = at_call();
    let right = RandomOutcome::Coin {player: 0, face: CoinFace::Tails};
    for receipts in [vec![], vec![RandomOutcome::Coin {player: 1, face: CoinFace::Tails}],
                     vec![RandomOutcome::Shuffle {player: 0, order: vec![]}], vec![right.clone(), right.clone()]] {
        let mut sim = start.clone();
        let request = sim.request().unwrap().unwrap().clone();
        let before = format!("{:?}", sim.game().unwrap());
        let snapshot = sim.snapshot(None).unwrap();
        let action = Move::Choose(game::state::Answer::CoinCall(CoinFace::Heads));
        let error = sim.respond_with_random(request.id, 0, action.clone(), receipts, false).unwrap_err();
        assert!(matches!(error,game::simulation::ResponseError::ReplayInput(_)), "{error}");
        assert!(sim.fault().is_none());
        assert_eq!(sim.request().unwrap().unwrap(), &request);
        assert_eq!(sim.snapshot(None).unwrap(), snapshot);
        assert_eq!(format!("{:?}", sim.game().unwrap()), before);
        sim.respond_with_random(request.id, 0, action, vec![right.clone()], false).unwrap();
        cite!(["705.2", "120.3a"]; assert_eq!(sim.game().unwrap().players[0].life, 17));
    }
}

#[test]
fn r705_1_capture_seeded_branches_and_explicit_replay_agree_on_the_physical_result() {
    let original = at_call();
    let mut recorded = original.clone();
    let mut seeded = original.clone();
    let mut replay = original;
    let request = recorded.request().unwrap().unwrap().clone();
    let action = Move::Choose(game::state::Answer::CoinCall(CoinFace::Tails));
    let receipts = recorded.respond_capturing_random(request.id, 0, action.clone()).unwrap();
    let [game::start::CapturedRandom::Coin {player,face}] = receipts.as_slice() else {panic!("one physical coin receipt")};
    assert_eq!(*player, 0);
    seeded.respond(request.id, 0, action.clone()).unwrap();
    replay.respond_with_random(request.id, 0, action, vec![RandomOutcome::Coin {player: 0, face: *face}], false).unwrap();
    let expected_life = if *face == CoinFace::Tails {20} else {17};
    for sim in [&mut recorded, &mut seeded, &mut replay] {
        let g = sim.game().unwrap();
        cite!(["705.2", "120.3a"]; assert_eq!(g.players[0].life, expected_life));
        let flips = g.log.iter().filter_map(|e| match e {game::Event::CoinFlipped {result,..} => Some(*result),_=>None}).collect::<Vec<_>>();
        assert_eq!(flips, [game::start::CoinFlip {player: 0, call: Some(CoinFace::Tails), face: *face}]);
    }
}

#[test]
fn unused_coin_receipts_keep_their_original_occurrence_indices_and_faces() {
    let mut sim = at_call();
    let request = sim.request().unwrap().unwrap().clone();
    let unused = sim.respond_with_random(request.id, 0, Move::Choose(game::state::Answer::CoinCall(CoinFace::Heads)),
        vec![RandomOutcome::Coin {player: 1, face: CoinFace::Heads}, RandomOutcome::Coin {player: 0, face: CoinFace::Tails},
             RandomOutcome::Coin {player: 1, face: CoinFace::Tails}], true).unwrap();
    assert_eq!(unused.iter().map(|r|r.input_index).collect::<Vec<_>>(), [0,2]);
    assert!(matches!(unused[0].outcome,RandomOutcome::Coin {player: 1, face: CoinFace::Heads}));
    assert!(matches!(unused[1].outcome,RandomOutcome::Coin {player: 1, face: CoinFace::Tails}));
    cite!(["705.2", "120.3a"]; assert_eq!(sim.game().unwrap().players[0].life, 17));
}

#[test]
fn r113_7a_bottle_of_suleiman_can_deal_its_loss_damage_after_its_source_was_sacrificed() {
    let mut t = Board::new(2).battlefield(0, &["Bottle of Suleiman"]).fill(8).build();
    let bottle = t.find_in(0, Zone::Battlefield, "Bottle of Suleiman");
    t.g.players[0].pool = crate::support::mana_pool("C");
    t.g.act(0, crate::support::paid("C", Move::Activate {source: bottle, ability: 0, targets: vec![], paying: vec![]})).unwrap();
    cite!(["601.2h", "400.7"]; assert_eq!(t.g.obj(bottle).zone, Zone::Gone));
    t.pass_all();
    let mut sim = game::simulation::Simulation::new(t.g);
    let request = sim.request().unwrap().unwrap().clone();
    assert_eq!(request.question, game::legal::Asked::CoinCall);
    sim.respond_with_random(request.id, 0, Move::Choose(game::state::Answer::CoinCall(CoinFace::Heads)),
        vec![RandomOutcome::Coin {player: 0, face: CoinFace::Tails}], false).unwrap();
    cite!(["113.7a", "705.2", "120.3a"]; assert_eq!(sim.game().unwrap().players[0].life, 15));
}

#[test]
fn r608_2c_two_flips_bind_their_own_results_across_separate_call_suspensions() {
    let spell = game::CardDef::new("Two separate flips", "{0}", "Instant",
        "Flip a coin. If you win the flip, you gain 1 life. Flip a coin. If you lose the flip, you lose 2 life.", None).unwrap();
    game::play::check(&spell).unwrap();
    let mut g = game::Game::new(vec![spell], 2, 20);
    let card = g.put(0, 0, Zone::Hand);
    g.act(0, Move::Cast {card, targets: vec![]}).unwrap();
    let mut t = T::new(g);
    t.pass_all();
    let mut sim = game::simulation::Simulation::new(t.g);
    let first = sim.request().unwrap().unwrap().clone();
    sim.respond_with_random(first.id, 0, Move::Choose(game::state::Answer::CoinCall(CoinFace::Heads)),
        vec![RandomOutcome::Coin {player: 0, face: CoinFace::Heads}], false).unwrap();
    cite!(["705.2", "119.3"]; assert_eq!(sim.game().unwrap().players[0].life, 21));
    let second = sim.request().unwrap().unwrap().clone();
    assert_eq!(second.question, game::legal::Asked::CoinCall);
    let mut fork = sim.clone();
    for sim in [&mut sim, &mut fork] {
        sim.respond_with_random(second.id, 0, Move::Choose(game::state::Answer::CoinCall(CoinFace::Heads)),
            vec![RandomOutcome::Coin {player: 0, face: CoinFace::Tails}], false).unwrap();
        // Reusing the first flip would leave 21 life; replaying its gain would leave 20.
        cite!(["608.2c", "705.2", "119.3"]; assert_eq!(sim.game().unwrap().players[0].life, 19));
        assert!(sim.game().unwrap().stack.is_empty());
    }
}

#[test]
fn r608_2c_molten_birth_keeps_its_tokens_when_the_later_coin_call_resumes() {
    let mut t = Board::new(2).hand(0, &["Molten Birth"]).fill(8).build();
    let spell = t.find_in(0, Zone::Hand, "Molten Birth");
    t.g.players[0].pool = crate::support::mana_pool("RRR");
    t.g.act(0, crate::support::paid("RRR", Move::Cast {card: spell, targets: vec![]})).unwrap();
    t.pass_all();
    let mut sim = game::simulation::Simulation::new(t.g);
    let request = sim.request().unwrap().unwrap().clone();
    assert_eq!(request.question, game::legal::Asked::CoinCall);
    cite!(["608.2c", "701.7a"]; assert_eq!(sim.game().unwrap().battlefield.len(), 2));
    let mut fork = sim.clone();
    for sim in [&mut sim, &mut fork] {
        sim.respond_with_random(request.id, 0, Move::Choose(game::state::Answer::CoinCall(CoinFace::Heads)),
            vec![RandomOutcome::Coin {player: 0, face: CoinFace::Heads}], false).unwrap();
        cite!(["608.2c", "701.7a"]; assert_eq!(sim.game().unwrap().battlefield.len(), 2));
        cite!(["705.2", "608.2c"]; assert_eq!(sim.game().unwrap().players[0].hand.len(), 1));
        assert_eq!(sim.game().unwrap().obj(sim.game().unwrap().players[0].hand[0]).chars.name, "Molten Birth");
    }
}

#[test]
fn r705_2_a_face_only_result_has_neither_a_winning_nor_a_losing_event() {
    // Check the shared result/event contract, without claiming execution support for
    // cards whose heads/tails instructions are still outside the grammar/runtime.
    for face in [CoinFace::Heads, CoinFace::Tails] {
        let result = game::start::CoinFlip {player: 0, call: None, face};
        cite!(["705.2"]; assert_eq!(result.won(), None));
        let event = game::Event::CoinFlipped {source: game::ObjId(0), result};
        cite!(["705.2"]; assert_eq!(event.verb(), None));
    }
}

#[test]
fn missing_or_unimplemented_coin_producers_are_refused_instead_of_reusing_a_result() {
    for (text, reason) in [
        ("If you lose the flip, you lose 3 life.", "coin outcome: no preceding local flip producer"),
        ("You may flip a coin. If you lose the flip, you lose 3 life.",
         "coin flip: only an unconditional single called flip by the controller is implemented"),
        ("Flip a coin. If you win the flip, flip a coin. If you lose the flip, you lose 3 life.",
         "coin flip: only an unconditional single called flip by the controller is implemented"),
    ] {
        let def = game::CardDef::new("Unimplemented flip scope", "{0}", "Instant", text, None).unwrap();
        crate::support::refused(game::play::check(&def).map_err(|e|e.0), reason);
    }
}
