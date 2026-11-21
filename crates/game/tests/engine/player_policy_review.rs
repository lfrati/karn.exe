//! Explicit input failures preserve state and terminal faults.
use game::simulation::{ResponseError, Simulation};
use game::{CardDef, Game, Move, Target, Zone};

#[test]
fn an_action_after_a_fault_still_reports_the_terminal_fault() {
    let def = CardDef::new("Bear", "{0}", "Creature — Bear", "", Some((2, 2))).unwrap();
    let mut sim = Simulation::new(Game::new(vec![def], 2, 20));
    let request = sim.request().unwrap().unwrap().clone();
    // The public observation boundary contains an unexpected failure and discards
    // the game. Subsequent operations must keep reporting that terminal failure.
    let fault = sim.observe::<()>(|_| panic!("independent fault probe")).unwrap_err();
    let result = sim.respond(request.id, request.player, Move::Pass);
    assert_eq!(result, Err(ResponseError::Fault(fault)));
}

#[test]
fn hidden_object_answers_are_refused_without_exposing_or_changing_the_hidden_card() {
    let def = CardDef::new("Spell", "{0}", "Instant", "You gain 1 life.", None).unwrap();
    let mut game = Game::new(vec![def], 2, 20);
    let hidden = game.put(1, 0, Zone::Hand);
    let mut sim = Simulation::new(game);
    let prompt = sim.prompt(0).unwrap();
    assert!(!prompt.entities.contains_key(&hidden));
    let request = sim.request().unwrap().unwrap().clone();
    let before = sim.snapshot(None).unwrap();
    let cast = Move::Cast { card: hidden, targets: vec![] };
    for _ in 0..4 {
        assert_eq!(sim.answer(0, request.id, cast.clone()), Err(ResponseError::Unreferable(Target::Object(hidden))));
        assert_eq!(sim.snapshot(None).unwrap(), before);
        assert!(!sim.prompt(0).unwrap().entities.contains_key(&hidden));
        assert_eq!(sim.request().unwrap().unwrap().id, request.id);
    }
}
