//! Myriad (702.116): for each opponent other than defending player its controller chooses, a
//! token copy enters tapped and attacking that player or a planeswalker they control, and the
//! tokens are exiled at end of combat.
use crate::support::{cite, oracle};
use game::{Game, Move, ObjId, Target, Zone};
use game::state::{Answer, Decision};
use ir::Part;

fn attack(g: &mut Game, attacks: Vec<(ObjId, Target)>) {
    for _ in 0..40 {
        if matches!(g.decision, Some(Decision::Attackers { player: 0 })) {
            g.act(0, Move::Attack(attacks)).unwrap();
            if let Some(Decision::Order { player, abilities }) = g.decision.clone() { g.act(player, Move::Order((0..abilities.len()).collect())).unwrap(); }
            return;
        }
        g.act(g.priority, Move::Pass).unwrap();
    }
    panic!("no attack declaration");
}

/// Everyone passes until the top of the stack asks a choice or has resolved.
fn resolve(g: &mut Game) {
    let depth = g.stack.len();
    while g.decision.is_none() && g.stack.len() == depth && depth > 0 { g.act(g.priority, Move::Pass).unwrap(); }
}

/// One step of the game on: no blocks are declared, and otherwise everyone passes.
fn step(g: &mut Game) {
    match g.decision.clone() {
        Some(Decision::Blockers { player }) => g.act(player, Move::Block(vec![])).unwrap(),
        _ => g.act(g.priority, Move::Pass).unwrap(),
    };
}

fn tokens(g: &Game) -> Vec<ObjId> { g.battlefield.iter().copied().filter(|o| g.obj(*o).token()).collect() }

fn attacked(g: &Game, o: ObjId) -> Option<Target> { g.combat.attackers.iter().find(|(a, _)| *a == o).map(|(_, t)| *t) }

/// Four players: Warchief Giant attacks player 1; player 3 controls Jace Beleren.
fn four() -> (Game, ObjId, ObjId) {
    let mut g = Game::new(vec![oracle("Warchief Giant"), oracle("Jace Beleren")], 4, 20);
    let giant = g.put(0, 0, Zone::Battlefield);
    let jace = g.put(3, 1, Zone::Battlefield);
    g.put_counters(jace, "loyalty", 3);
    attack(&mut g, vec![(giant, Target::Player(1))]);
    (g, giant, jace)
}

#[test]
fn r702_116a_a_token_for_each_other_opponent_attacking_that_player_or_their_planeswalker() {
    let (mut g, giant, jace) = four();
    cite!(["702.116a", "603.2"]; assert_eq!(g.stack.len(), 1));
    resolve(&mut g);
    // the opponents other than defending player are offered, in turn order
    let Some(Decision::Choose { player: 0, choice: game::state::Choice::Holders { among, .. } }) = g.decision.clone() else {
        panic!("myriad asks which opponents to make a token for: {:?}", g.decision)
    };
    cite!(["702.116a"]; assert_eq!(among, vec![Target::Player(2), Target::Player(3)]));
    g.act(0, Move::Choose(Answer::Holders(vec![Target::Player(2), Target::Player(3)]))).unwrap();
    // the token for player 3 attacks them or Jace; player 2's has only player 2 to attack
    let Some(Decision::Choose { player: 0, choice: game::state::Choice::Defender { among, .. } }) = g.decision.clone() else {
        panic!("the token for player 3 asks what it attacks: {:?}", g.decision)
    };
    cite!(["702.116a", "508.4"]; assert_eq!(among, vec![Target::Player(3), Target::Object(jace)]));
    g.act(0, Move::Choose(Answer::Defender(Target::Object(jace)))).unwrap();
    let made = tokens(&g);
    cite!(["702.116a", "707.2"]; assert_eq!(made.len(), 2));
    for token in &made {
        cite!(["702.116a", "707.2"]; assert_eq!(g.obj(*token).chars.name, "Warchief Giant"));
        cite!(["702.116a"]; assert!(g.obj(*token).tapped));
    }
    let targets: Vec<_> = made.iter().map(|t| attacked(&g, *t)).collect();
    cite!(["702.116a", "508.4"]; assert!(targets.contains(&Some(Target::Player(2))) && targets.contains(&Some(Target::Object(jace)))));
    cite!(["702.116a"]; assert_eq!(attacked(&g, giant), Some(Target::Player(1))));
}

#[test]
fn r702_116a_you_may_make_a_token_for_only_some_of_them() {
    let (mut g, _, _) = four();
    resolve(&mut g);
    g.act(0, Move::Choose(Answer::Holders(vec![Target::Player(2)]))).unwrap();
    let made = tokens(&g);
    cite!(["702.116a"]; assert_eq!(made.len(), 1));
    cite!(["702.116a", "508.4"]; assert_eq!(attacked(&g, made[0]), Some(Target::Player(2))));
}

#[test]
fn r702_116a_the_tokens_are_exiled_at_end_of_combat() {
    let (mut g, giant, _) = four();
    resolve(&mut g);
    g.act(0, Move::Choose(Answer::Holders(vec![Target::Player(2)]))).unwrap();
    assert_eq!(tokens(&g).len(), 1);
    // the delayed trigger is put on the stack at the beginning of the end of combat step
    while g.part != Part::EndOfCombat { step(&mut g); }
    cite!(["702.116a", "603.7"]; assert_eq!(g.stack.len(), 1));
    cite!(["702.116a"]; assert_eq!(tokens(&g).len(), 1));
    while !g.stack.is_empty() { step(&mut g); }
    cite!(["702.116a", "603.7"]; assert!(tokens(&g).is_empty()));
    cite!(["702.116a"]; assert_eq!(g.obj(giant).zone, Zone::Battlefield));
}

#[test]
fn r702_116a_none_chosen_makes_no_token_and_no_delayed_trigger() {
    let (mut g, _, _) = four();
    resolve(&mut g);
    g.act(0, Move::Choose(Answer::Holders(vec![]))).unwrap();
    cite!(["702.116a"]; assert!(tokens(&g).is_empty()));
    let mut triggered = false;
    for _ in 0..20 {
        if g.part == Part::EndOfCombat && !g.stack.is_empty() { triggered = true; }
        if g.part == Part::PostcombatMain { break; }
        step(&mut g);
    }
    cite!(["702.116a", "603.7"]; assert!(!triggered));
}

#[test]
fn r702_116a_against_one_opponent_there_is_no_one_to_make_a_token_for() {
    let mut g = Game::new(vec![oracle("Warchief Giant")], 2, 20);
    let giant = g.put(0, 0, Zone::Battlefield);
    attack(&mut g, vec![(giant, Target::Player(1))]);
    resolve(&mut g);
    cite!(["702.116a"]; assert!(!matches!(g.decision, Some(Decision::Choose { .. }))));
    cite!(["702.116a"]; assert!(tokens(&g).is_empty()));
}

#[test]
fn r702_116b_each_instance_triggers_separately() {
    let mut g = Game::new(vec![oracle("Scurry of Squirrels")], 3, 20);
    let squirrels = g.put(0, 0, Zone::Battlefield);
    attack(&mut g, vec![(squirrels, Target::Player(1))]);
    cite!(["702.116b"]; assert_eq!(g.stack.len(), 2));
    for _ in 0..2 {
        resolve(&mut g);
        g.act(0, Move::Choose(Answer::Holders(vec![Target::Player(2)]))).unwrap();
    }
    let made = tokens(&g);
    cite!(["702.116b"]; assert_eq!(made.len(), 2));
    for token in made { cite!(["702.116b", "508.4"]; assert_eq!(attacked(&g, token), Some(Target::Player(2)))); }
}

#[test]
fn r508_4_for_each_opponent_a_token_attacking_that_player_or_a_planeswalker_they_control() {
    // Adeline, Resplendent Cathar: "Whenever you attack, for each opponent, create a 1/1 white
    // Human creature token that's tapped and attacking that player or a planeswalker they
    // control." Each opponent gets one, the attacked player too, all made at once.
    let mut g = Game::new(vec![oracle("Adeline, Resplendent Cathar"), oracle("Jace Beleren")], 3, 20);
    let adeline = g.put(0, 0, Zone::Battlefield);
    let jace = g.put(2, 1, Zone::Battlefield);
    g.put_counters(jace, "loyalty", 3);
    attack(&mut g, vec![(adeline, Target::Player(1))]);
    resolve(&mut g);
    let Some(Decision::Choose { player: 0, choice: game::state::Choice::Defender { among, .. } }) = g.decision.clone() else {
        panic!("the token for player 2 asks what it attacks: {:?}", g.decision)
    };
    cite!(["508.4"]; assert_eq!(among, vec![Target::Player(2), Target::Object(jace)]));
    g.act(0, Move::Choose(Answer::Defender(Target::Object(jace)))).unwrap();
    let made = tokens(&g);
    cite!(["508.4", "111.1"]; assert_eq!(made.len(), 2));
    let targets: Vec<_> = made.iter().map(|t| attacked(&g, *t)).collect();
    cite!(["508.4"]; assert!(targets.contains(&Some(Target::Player(1))) && targets.contains(&Some(Target::Object(jace)))));
    for token in made { cite!(["508.4"]; assert!(g.obj(token).tapped)); }
}

#[test]
fn r508_4_a_token_copy_put_onto_the_battlefield_attacking_attacks_what_its_controller_chooses() {
    // Flamerush Rider: "Whenever this creature attacks, create a token that's a copy of another
    // target attacking creature and that's tapped and attacking. Exile the token at end of
    // combat." Against one opponent, it attacks that player.
    let mut g = Game::new(vec![oracle("Flamerush Rider"), oracle("Grizzly Bears")], 2, 20);
    let rider = g.put(0, 0, Zone::Battlefield);
    let bears = g.put(0, 1, Zone::Battlefield);
    attack(&mut g, vec![(rider, Target::Player(1)), (bears, Target::Player(1))]);
    cite!(["603.3d"]; assert!(matches!(g.decision, Some(Decision::Targets { player: 0, .. })), "{:?}", g.decision));
    g.act(0, Move::ChooseTargets(vec![vec![Target::Object(bears)]])).unwrap();
    resolve(&mut g);
    let made = tokens(&g);
    // a copy of the 2/2 Bears, not of the 2/3 Rider
    cite!(["707.2", "111.1"]; assert!(made.len() == 1 && g.power(made[0]) == Some(2) && g.toughness(made[0]) == Some(2)));
    cite!(["508.4"]; assert_eq!(attacked(&g, made[0]), Some(Target::Player(1))));
    cite!(["508.4"]; assert!(g.obj(made[0]).tapped));
    while !(g.part == Part::EndOfCombat && !g.stack.is_empty()) { step(&mut g); }
    cite!(["603.7a"]; assert_eq!(tokens(&g).len(), 1));
    while !g.stack.is_empty() { step(&mut g); }
    cite!(["603.7a", "111.7"]; assert!(tokens(&g).is_empty()));
    cite!(["603.7a"]; assert!(g.battlefield.contains(&rider) && g.battlefield.contains(&bears)));
}
