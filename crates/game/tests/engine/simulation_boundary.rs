use game::{CardDef, Game, Move, Zone};
use game::legal::Asked;
use game::simulation::{ResponseError, Simulation};
use game::state::Answer;

#[test]
fn r117_3d_priority_passes_are_correlated_even_when_priority_returns_to_the_same_player() {
    let mut simulation = Simulation::new(Game::new(vec![], 2, 20));
    let first = simulation.request().unwrap().unwrap().clone();
    assert_eq!(simulation.request().unwrap(), Some(&first));
    assert!(matches!(first.question, Asked::Priority));
    simulation.respond(first.id, first.player, Move::Pass).unwrap();
    let second = simulation.request().unwrap().unwrap().clone();
    simulation.respond(second.id, second.player, Move::Pass).unwrap();
    let third = simulation.request().unwrap().unwrap().clone();
    assert_eq!(first.player, third.player);
    assert!(third.id > second.id && second.id > first.id);
    let before = format!("{:?}", simulation.game().unwrap());
    assert_eq!(simulation.respond(first.id, first.player, Move::Pass),
        Err(ResponseError::Stale { expected: third.id, received: first.id }));
    assert_eq!(format!("{:?}", simulation.game().unwrap()), before);
    assert_eq!(simulation.request().unwrap(), Some(&third));
}

fn two_questions() -> Simulation {
    let spell = CardDef::new("Two decisions", "{0}", "Instant", "You may gain 1 life. You may gain 1 life.", None).unwrap();
    game::play::check(&spell).unwrap();
    let mut game = Game::new(vec![spell], 2, 20);
    let card = game.put(0, 0, Zone::Hand);
    let mut simulation = Simulation::new(game);
    let id = simulation.request().unwrap().unwrap().id;
    simulation.respond(id, 0, Move::Cast { card, targets: vec![] }).unwrap();
    for player in 0..2 {
        let id = simulation.request().unwrap().unwrap().id;
        simulation.respond(id, player, Move::Pass).unwrap();
    }
    assert!(matches!(simulation.request().unwrap().unwrap().question, Asked::May));
    simulation
}

#[test]
fn r608_2d_identical_successive_questions_reject_replay_and_invalid_answers_keep_the_request() {
    let mut simulation = two_questions();
    let first = simulation.request().unwrap().unwrap().clone();
    let before = format!("{:?}", simulation.game().unwrap());
    assert_eq!(simulation.respond(first.id, 1, Move::Choose(Answer::Yes)),
        Err(ResponseError::WrongPlayer { expected: 0, received: 1 }));
    crate::support::refused(simulation.respond(first.id, 0, Move::Pass), "a decision is pending: a choice of the effect being applied (608.2d)");
    assert_eq!(format!("{:?}", simulation.game().unwrap()), before);
    assert_eq!(simulation.request().unwrap(), Some(&first));
    simulation.respond(first.id, 0, Move::Choose(Answer::Yes)).unwrap();
    let second = simulation.request().unwrap().unwrap().clone();
    assert_eq!(second.question, first.question);
    assert!(second.id > first.id);
    assert_eq!(simulation.game().unwrap().players[0].life, 21);
    assert!(matches!(simulation.respond(first.id, 0, Move::Choose(Answer::Yes)), Err(ResponseError::Stale { .. })));
    assert_eq!(simulation.game().unwrap().players[0].life, 21);
    assert_eq!(simulation.request().unwrap(), Some(&second));
    simulation.respond(second.id, 0, Move::Choose(Answer::No)).unwrap();
    assert_eq!(simulation.game().unwrap().players[0].life, 21);
}

#[test]
fn r608_2d_paused_clones_keep_correlation_and_continue_independently_without_replaying_effects() {
    let mut simulation = two_questions();
    let first = simulation.request().unwrap().unwrap().id;
    simulation.respond(first, 0, Move::Choose(Answer::Yes)).unwrap();
    let mut branch = simulation.clone();
    let pending = simulation.request().unwrap().unwrap().clone();
    assert_eq!(branch.request().unwrap(), Some(&pending));
    simulation.respond(pending.id, 0, Move::Choose(Answer::Yes)).unwrap();
    assert_eq!(branch.game().unwrap().players[0].life, 21);
    branch.respond(pending.id, 0, Move::Choose(Answer::Yes)).unwrap();
    assert_eq!(simulation.game().unwrap().players[0].life, 22);
    assert_eq!(format!("{:?}", simulation.game().unwrap()), format!("{:?}", branch.game().unwrap()));
    assert_eq!(simulation.request().unwrap(), branch.request().unwrap());
}

#[test]
fn a_finished_game_has_no_request_and_is_not_a_fault() {
    let mut game = Game::new(vec![], 2, 20);
    game.players[1].lost = true;
    let mut simulation = Simulation::new(game);
    assert_eq!(simulation.request().unwrap(), None);
    assert_eq!(simulation.respond(game::simulation::RequestId(1), 0, Move::Pass), Err(ResponseError::Finished));
    assert!(simulation.fault().is_none());
}

#[test]
fn setup_faults_discard_partial_games_without_affecting_other_simulations() {
    use game::simulation::SetupError;
    let mut healthy = Simulation::from_setup(|| Ok::<_, String>(Game::new(vec![], 2, 20))).unwrap();
    let first = healthy.request().unwrap().unwrap().clone();
    let failed = Simulation::from_setup(|| -> Result<Game, String> {
        let mut partial = Game::new(vec![], 2, 20);
        partial.players[0].life = 1;
        panic!("controlled setup failure");
    });
    assert_eq!(failed.unwrap_err(), SetupError::Fault { message: "controlled setup failure".into() });
    assert_eq!(healthy.request().unwrap(), Some(&first));
    healthy.respond(first.id, first.player, Move::Pass).unwrap();
    assert_eq!(healthy.game().unwrap().players[0].life, 20);
    let rejected = Simulation::from_setup(|| Err::<Game, _>("invalid deck"));
    assert_eq!(rejected.unwrap_err(), SetupError::Rejected("invalid deck"));
}

#[test]
fn observation_failures_terminate_only_the_affected_simulation() {

    use game::simulation::Stage;
    let mut failed = Simulation::new(Game::new(vec![], 2, 20));
    let pending = failed.request().unwrap().unwrap().clone();
    let mut healthy = failed.clone();
    assert_eq!(failed.observe(|g| g.players[0].life).unwrap(), 20);
    assert_eq!(failed.request().unwrap(), Some(&pending));
    let fault = failed.observe(|_| panic!("controlled projection failure")).unwrap_err();
    assert_eq!(fault.stage, Stage::Observation);
    assert_eq!(fault.request_id, pending.id);
    assert_eq!(failed.observe(|_| panic!("must not execute again")).unwrap_err(), fault);
    assert_eq!(failed.request().unwrap_err(), fault);
    assert_eq!(failed.respond(pending.id, pending.player, Move::Pass), Err(ResponseError::Fault(fault.clone())));
    assert_eq!(failed.game().unwrap_err(),fault);
    healthy.respond(pending.id, pending.player, Move::Pass).unwrap();
    assert_eq!(healthy.observe(|g| g.players[0].life).unwrap(), 20);
}

fn with_a_free_spell() -> (Simulation, game::ObjId) {
    let spell = CardDef::new("Free", "{0}", "Instant", "You gain 1 life.", None).unwrap();
    game::play::check(&spell).unwrap();
    let mut game = Game::new(vec![spell], 2, 20);
    let card = game.put(0, 0, Zone::Hand);
    (Simulation::new(game), card)
}

#[test]
fn r117_3d_asking_who_has_priority_and_passing_search_for_no_move() {
    let (mut simulation, _) = with_a_free_spell();
    // a whole turn of both players passing, each pass a correlated answer
    let turn = simulation.game().unwrap().turn;
    let mut passes = 0;
    while simulation.game().unwrap().turn == turn {
        let request = simulation.request().unwrap().unwrap().clone();
        assert_eq!(request.question, Asked::Priority);
        simulation.respond(request.id, request.player, Move::Pass).unwrap();
        passes += 1;
        assert!(passes <= 100, "the turn does not end by passing");
    }
    assert!(passes >= 2);

}

#[test]
fn r601_2_a_move_made_without_the_list_is_checked_as_one_from_it() {
    let (mut simulation, card) = with_a_free_spell();
    let request = simulation.request().unwrap().unwrap().clone();
    let before = format!("{:?}", simulation.game().unwrap());
    // not a land, and not this player's to cast: refused, and the request stands
    crate::support::refused(simulation.respond(request.id, 0, Move::PlayLand(card)), "that is not a land");
    assert_eq!(simulation.respond(request.id, 1, Move::Cast { card, targets: vec![] }),
        Err(ResponseError::WrongPlayer { expected: 0, received: 1 }));
    assert_eq!(format!("{:?}", simulation.game().unwrap()), before);
    assert_eq!(simulation.request().unwrap(), Some(&request));
    simulation.respond(request.id, 0, Move::Cast { card, targets: vec![] }).unwrap();
    assert_eq!(simulation.game().unwrap().stack.len(), 1);
}
