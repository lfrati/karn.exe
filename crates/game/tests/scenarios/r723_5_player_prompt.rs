//! A player's prompt under turn control (723.5): the controller is asked and answers, about the
//! controlled player's cards; both see the question, and the prompt tells them apart.
use crate::support::cite;
use game::legal::Asked;
use game::prompt::Reason;
use game::simulation::{ResponseError, Simulation};
use game::{CardDef, Game, Move, Target, Zone};

fn card(name: &str, types: &str, text: &str, pt: Option<(i32, i32)>) -> CardDef {
    let d = CardDef::new(name, "{0}", types, text, pt).unwrap();
    game::play::check(&d).unwrap_or_else(|e| panic!("{name}: {}", e.0));
    d
}

/// Player 0 controls player 1's turn, in its precombat main phase.
fn controlled() -> (Simulation, game::ObjId, game::ObjId) {
    let control = "You gain control of target opponent during that player's next turn. After that turn, that player takes an extra turn.";
    let mut g = Game::new(vec![card("Control", "Sorcery", control, None), card("Bear", "Creature — Bear", "", Some((2, 2)))], 3, 20);
    for player in 0..3 { for _ in 0..10 { g.put(player, 1, Zone::Library); } }
    let spell = g.put(0, 0, Zone::Hand);
    g.act(0, Move::Cast { card: spell, targets: vec![vec![Target::Player(1)]] }).unwrap();
    for _ in 0..3 { g.act(g.decision_maker(g.priority), Move::Pass).unwrap(); }
    while g.turn < 2 || g.active != 1 {
        let mv = match &g.decision { Some(game::state::Decision::Attackers { .. }) => Move::Attack(vec![]), _ => Move::Pass };
        g.act(g.decision_maker(g.decision_actor()), mv).unwrap();
    }
    let victims = g.put(1, 1, Zone::Hand);
    let mine = g.put(0, 1, Zone::Hand);
    g.part = ir::Part::PrecombatMain;
    (Simulation::new(g), victims, mine)
}

#[test]
fn r723_5_the_controller_is_the_chooser_and_the_controlled_player_the_actor() {
    let (mut s, victims, mine) = controlled();
    let asked = s.prompt(0).unwrap();
    let decision = asked.decision.clone().expect("the controller answers");
    cite!(["723.5"]; assert_eq!((decision.chooser, decision.acting, decision.reason, decision.question.clone()), (0, 1, Reason::Priority, Asked::Priority)));
    cite!(["723.5"]; assert_eq!(asked.waiting_for, Some(0)));
    // the controller sees the controlled player's hand (723.4) as well as its own
    cite!(["723.4"]; assert!(asked.entities.contains_key(&victims) && asked.entities.contains_key(&mine)));
    let actor = s.prompt(1).unwrap();
    let theirs = actor.decision.expect("the controlled player is shown its own decision");
    assert_eq!((theirs.chooser, theirs.acting), (0, 1));
    cite!(["723.4"]; assert!(actor.entities.contains_key(&victims)));
    // Uncited (protocol): a seat that neither chooses nor acts gets no decision in its prompt, only whom the game waits for
    assert!(s.prompt(2).unwrap().decision.is_none(), "a third seat is told only whom the game waits for");
    // only the decision-maker answers, with the actor's cards
    assert!(matches!(s.answer(1, decision.id, Move::Pass), Err(ResponseError::WrongPlayer { expected: 0, received: 1 })));
    crate::support::refused(s.answer(0, decision.id, Move::Cast { card: mine, targets: vec![] }), "you cast only from your own (601.3)");
    cite!(["723.5"]; assert_eq!(s.answer(0, decision.id, Move::Cast { card: victims, targets: vec![] }), Ok(())));
    let spell = s.game().unwrap().stack[0];
    let occurrence = s.prompt(0).unwrap().entities[&spell].occurrence.clone().unwrap();
    cite!(["723.5"]; assert_eq!(occurrence.controller, 1, "the spell is the controlled player's (723.5)"));
}
