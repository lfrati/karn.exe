//! Echo checks historical control acquisitions between actual upkeep boundaries.
use crate::support::{cite, mana_pool, oracle_read, paid};
use game::state::{Answer, Choice, Decision};
use game::{CardDef, Event, Game, Move, ObjId, Zone};
use ir::Part;

fn setup() -> Game {
    let defs = vec![oracle_read("Pouncing Jaguar"), oracle_read("Mountain"),
        CardDef::new("Steal", "{0}", "Instant", "Gain control of target creature.", None).unwrap(),
        CardDef::new("Copier", "{0}", "Artifact", "{0}: Copy target activated or triggered ability.", None).unwrap(),
        oracle_read("Cloudshift"), oracle_read("Sphinx of the Second Sun")];
    // Uncited (admission): full focus card plus generic control/copy composition fixtures.
    for def in &defs { game::play::check(def).unwrap_or_else(|why| panic!("{}: {}", def.name(), why.0)); }
    let mut game = Game::new(defs, 2, 20);
    for player in 0..2 { for _ in 0..15 { game.put(player, 1, Zone::Library); } }
    game
}
fn passes(game: &mut Game) { for _ in 0..2 { game.act(game.priority, Move::Pass).unwrap(); } }
fn cast_jaguar(game: &mut Game) -> ObjId {
    let card = game.put(0, 0, Zone::Hand); game.players[0].pool = mana_pool("G");
    game.act(0, paid("G", Move::Cast { card, targets: vec![] })).unwrap(); passes(game);
    game.battlefield[0]
}
fn next_upkeep(game: &mut Game, player: usize) {
    let turn = game.turn;
    for _ in 0..200 {
        if game.turn > turn && game.active == player && game.part == Part::Upkeep { return; }
        match game.decision.clone() {
            Some(Decision::Attackers { player }) => game.act(player, Move::Attack(vec![])).unwrap(),
            None => game.act(game.priority, Move::Pass).unwrap(),
            other => panic!("unexpected decision {other:?}"),
        }
    }
    panic!("did not reach next own upkeep");
}
fn main_phase(game: &mut Game) {
    for _ in 0..20 {
        if game.part == Part::PrecombatMain { return; }
        game.act(game.priority, Move::Pass).unwrap();
    }
    panic!("did not reach main phase");
}
fn payment(game: &Game, player: usize) {
    cite!(["702.30a", "118.12"]; assert!(matches!(&game.decision,
        Some(Decision::Choose { player: who, choice: Choice::Pay { .. } }) if *who == player)));
}
fn pay(game: &mut Game, player: usize, units: &str) {
    game.players[player].pool = mana_pool(units);
    game.act(player, Move::Choose(Answer::Pay { plan: vec![], with: Some(mana_pool(units)), tapped: vec![] })).unwrap();
}
fn priority(game: &mut Game, player: usize) {
    if game.priority != player { game.act(game.priority, Move::Pass).unwrap(); }
    assert_eq!(game.priority, player);
}
fn steal(game: &mut Game, player: usize, target: ObjId) {
    priority(game, player); let card = game.put(player, 2, Zone::Hand);
    game.act(player, Move::Cast { card, targets: vec![vec![target.into()]] }).unwrap(); passes(game);
}

#[test]
fn r702_30a_first_upkeep_offers_the_fixed_cost_and_paying_does_not_repeat_next_upkeep() {
    let mut game = setup(); let permanent = cast_jaguar(&mut game); next_upkeep(&mut game, 0);
    cite!(["702.30a", "603.3"]; assert_eq!(game.stack.len(), 1)); passes(&mut game); payment(&game, 0);
    let mut declined = game.clone(); pay(&mut game, 0, "G");
    cite!(["702.30a", "601.2h"]; assert!(game.battlefield.contains(&permanent) && game.players[0].pool.total() == 0));
    next_upkeep(&mut game, 0);
    cite!(["702.30a", "603.4"]; assert!(game.stack.is_empty()));
    cite!(["702.30a", "118.12"]; ok: declined.act(0, Move::Choose(Answer::No)));
    cite!(["701.21a"]; assert!(!declined.battlefield.contains(&permanent)));
    cite!(["701.21a"]; assert!(declined.log.iter().any(|event| matches!(event, Event::Sacrificed { player: 0, object } if *object == permanent))));
}

#[test]
fn r702_30a_losing_and_regaining_control_after_a_paid_echo_creates_a_new_upkeep_obligation() {
    let mut game = setup(); let permanent = cast_jaguar(&mut game); next_upkeep(&mut game, 0); passes(&mut game); pay(&mut game, 0, "G");
    main_phase(&mut game); steal(&mut game, 1, permanent); steal(&mut game, 0, permanent);
    cite!(["702.30a", "110.2"]; assert_eq!(game.controller(permanent), 0));
    next_upkeep(&mut game, 0);
    cite!(["702.30a", "603.4"]; assert_eq!(game.stack.len(), 1)); passes(&mut game); payment(&game, 0);
    game.act(0, Move::Choose(Answer::No)).unwrap();
    cite!(["701.21a"]; assert!(!game.battlefield.contains(&permanent)));
}

#[test]
fn r603_4_echo_acquisition_remains_true_after_control_changes_but_only_the_controller_can_sacrifice() {
    let mut game = setup(); let permanent = cast_jaguar(&mut game); next_upkeep(&mut game, 0);
    steal(&mut game, 1, permanent);
    cite!(["702.30a", "603.4"]; assert_eq!(game.stack.len(), 1)); passes(&mut game); payment(&game, 0);
    cite!(["702.30a", "118.12"]; ok: game.act(0, Move::Choose(Answer::No)));
    cite!(["701.21a"]; assert!(game.battlefield.contains(&permanent) && game.controller(permanent) == 1));
    next_upkeep(&mut game, 1);
    cite!(["702.30a"]; assert_eq!(game.stack.len(), 1)); passes(&mut game); payment(&game, 1);
}

#[test]
fn r707_10_copied_echo_uses_its_controllers_observed_acquisition_even_after_it_lost_control() {
    let mut game = setup(); let permanent = cast_jaguar(&mut game); next_upkeep(&mut game, 0);
    steal(&mut game, 1, permanent); steal(&mut game, 0, permanent);
    let original = game.stack[0]; let copier = game.put(1, 3, Zone::Battlefield); priority(&mut game, 1);
    game.act(1, Move::Activate { source: copier, ability: 0, targets: vec![vec![original.into()]], paying: vec![] }).unwrap(); passes(&mut game);
    cite!(["707.10", "707.10b"]; assert_eq!(game.stack.len(), 2)); passes(&mut game); payment(&game, 1);
    cite!(["702.30a", "603.4"]; ok: game.act(1, Move::Choose(Answer::No)));
    cite!(["701.21a"]; assert!(game.battlefield.contains(&permanent) && game.controller(permanent) == 0));
    passes(&mut game); payment(&game, 0); game.act(0, Move::Choose(Answer::No)).unwrap();
    cite!(["701.21a"]; assert!(!game.battlefield.contains(&permanent)));
}

#[test]
fn r400_7_echo_keeps_the_departed_incarnations_history_and_does_not_sacrifice_the_returned_creature() {
    let mut game = setup(); let permanent = cast_jaguar(&mut game); next_upkeep(&mut game, 0);
    let blink = game.put(0, 4, Zone::Hand); game.players[0].pool = mana_pool("W");
    game.act(0, paid("W", Move::Cast { card: blink, targets: vec![vec![permanent.into()]] })).unwrap(); passes(&mut game);
    let returned = game.battlefield[0];
    cite!(["400.7"]; assert_ne!(returned, permanent)); passes(&mut game); payment(&game, 0);
    game.act(0, Move::Choose(Answer::No)).unwrap();
    cite!(["400.7", "701.21a"]; assert!(game.battlefield.contains(&returned)));
    next_upkeep(&mut game, 0);
    cite!(["702.30a", "400.7"]; assert_eq!(game.stack.len(), 1));
}

#[test]
fn r118_12_zero_echo_is_an_explicit_optional_payment_and_can_be_declined() {
    let mut game = setup(); let mut defs = game.defs.to_vec();
    defs[0] = CardDef::new("Zero echo", "{0}", "Artifact", "Echo {0}", None).unwrap(); game = Game::new(defs, 2, 20);
    for player in 0..2 { for _ in 0..10 { game.put(player, 1, Zone::Library); } }
    let card = game.put(0, 0, Zone::Hand); game.act(0, Move::Cast { card, targets: vec![] }).unwrap(); passes(&mut game);
    let permanent = game.battlefield[0]; next_upkeep(&mut game, 0); passes(&mut game); payment(&game, 0);
    let mut paid = game.clone(); pay(&mut paid, 0, "");
    cite!(["118.12", "118.5"]; assert!(paid.battlefield.contains(&permanent)));
    cite!(["118.12", "702.30a"]; ok: game.act(0, Move::Choose(Answer::No)));
    cite!(["701.21a"]; assert!(!game.battlefield.contains(&permanent)));
}

#[test]
fn positioned_echo_without_control_acquisition_history_is_refused() {
    let mut game = setup(); game.put(0, 0, Zone::Battlefield);
    // Uncited (implementation boundary): relative turn/sickness setup does not prove Echo's history.
    crate::support::refused(game.try_begin().map_err(|why| why.to_string()), "requires known control acquisition/upkeep history");
}

#[test]
fn fixed_echo_oracle_family_compiles_while_nonmana_and_other_history_queries_stay_refused() {
    for name in ["Pouncing Jaguar", "Albino Troll", "Goblin Patrol", "Urza's Blueprints", "Mogg War Marshal", "Avalanche Riders"] {
        // Uncited (admission): full pinned focus and consumer cards.
        game::play::check(&oracle_read(name)).unwrap_or_else(|why| panic!("{name}: {}", why.0));
    }
    let nonmana = CardDef::new("Nonmana echo", "{0}", "Creature", "Echo—Discard a card.", Some((1, 1))).unwrap();
    // Uncited (implementation boundaries): only fixed mana Echo and its own-upkeep window are in this step.
    crate::support::refused(game::play::check(&nonmana).map_err(|why| why.0), "keyword: Echo");
    let other_window = CardDef::new("Other window", "{0}", "Creature", "At the beginning of your end step, if this permanent came under your control this turn, you gain 1 life.", Some((1, 1))).unwrap();
    crate::support::refused(game::play::check(&other_window).map_err(|why| why.0), "control acquisition:");
    crate::support::refused(game::play::check(&oracle_read("Erg Raiders")).map_err(|why| why.0), "program: control flow not lowered");
}

#[test]
fn r702_30a_real_sphinx_additional_beginning_phase_does_not_repeat_echo_in_the_same_turn() {
    let mut game = setup(); let permanent = cast_jaguar(&mut game); next_upkeep(&mut game, 0); passes(&mut game); pay(&mut game, 0, "G");
    main_phase(&mut game); let turn = game.turn;
    let sphinx = game.put(0, 5, Zone::Hand); game.players[0].pool = mana_pool("CCCCCCUU");
    game.act(0, paid("CCCCCCUU", Move::Cast { card: sphinx, targets: vec![] })).unwrap(); passes(&mut game);
    let sphinx = *game.battlefield.iter().find(|object| game.obj(**object).chars.name == "Sphinx of the Second Sun").unwrap();
    let casts = game.players[0].game_casts;
    for _ in 0..100 {
        if game.part == Part::Upkeep { break; }
        match game.decision.clone() {
            Some(Decision::Attackers { player }) => game.act(player, Move::Attack(vec![])).unwrap(),
            None => game.act(game.priority, Move::Pass).unwrap(),
            other => panic!("unexpected question {other:?}"),
        }
    }
    cite!(["500.8", "503.1"]; assert_eq!((game.turn, game.part), (turn, Part::Upkeep)));
    cite!(["702.30a", "603.4"]; assert!(game.stack.is_empty() && game.decision.is_none()));
    cite!(["702.30a"]; assert!(game.battlefield.contains(&permanent)));
    cite!(["302.6", "500.8"]; assert!(game.summoning_sick(sphinx)));
    cite!(["500.8", "608.2i"]; assert_eq!(game.players[0].game_casts, casts));
    cite!(["500.8"]; assert_eq!(game.players[0].turn_began, turn));
    let hand = game.players[0].hand.len(); passes(&mut game);
    cite!(["500.8", "504.1"]; assert_eq!((game.turn, game.part, game.players[0].hand.len()), (turn, Part::Draw, hand + 1)));
    passes(&mut game);
    cite!(["500.8"]; assert_eq!((game.turn, game.part), (turn, Part::End)));
}

#[test]
fn r603_4_copied_echo_does_nothing_for_a_player_who_never_controlled_this_incarnation() {
    let mut game = setup(); cast_jaguar(&mut game); next_upkeep(&mut game, 0);
    let original = game.stack[0]; let copier = game.put(1, 3, Zone::Battlefield); priority(&mut game, 1);
    game.act(1, Move::Activate { source: copier, ability: 0, targets: vec![vec![original.into()]], paying: vec![] }).unwrap(); passes(&mut game);
    cite!(["707.10", "707.10b"]; assert_eq!(game.stack.len(), 2)); passes(&mut game);
    cite!(["603.4", "702.30a"]; assert!(game.decision.is_none()));
    cite!(["603.4", "707.10"]; assert_eq!(game.stack, vec![original]));
    passes(&mut game); payment(&game, 0);
}

#[test]
fn first_turn_extra_beginning_refuses_without_inventing_draw_step_behavior() {
    let phase = CardDef::new("Beginning", "{0}", "Sorcery", "After this phase, there is an additional beginning phase.", None).unwrap();
    game::play::check(&phase).unwrap(); let mut game = Game::new(vec![phase], 2, 20);
    let card = game.put(0, 0, Zone::Hand); game.act(0, Move::Cast { card, targets: vec![] }).unwrap(); game.act(0, Move::Pass).unwrap();
    let before = format!("{game:?}");
    // Uncited (implementation boundary): first-turn additional draw omission is not inferred.
    crate::support::refused(game.act(1, Move::Pass), "starting player's first turn");
    assert_eq!(format!("{game:?}"), before);
}

#[test]
fn declared_position_does_not_reuse_unprovided_runtime_control_history() {
    let mut game = setup(); let permanent = cast_jaguar(&mut game); let before = format!("{game:?}");
    // Uncited (setup boundary): a declared settled position is not a saved-game restoration.
    crate::support::refused(game.settle_initial_position(5, Part::PrecombatMain, 0, 0, 0, &[(permanent, 0, false)]),
        "requires known control acquisition/upkeep history");
    assert_eq!(format!("{game:?}"), before);
}

#[test]
fn r603_4_copied_echo_uses_another_players_last_upkeep_not_its_previous_upkeep() {
    let mut game = setup(); let permanent = cast_jaguar(&mut game); steal(&mut game, 1, permanent);
    next_upkeep(&mut game, 1); passes(&mut game); payment(&game, 1); pay(&mut game, 1, "G"); main_phase(&mut game);
    steal(&mut game, 0, permanent); next_upkeep(&mut game, 0);
    let original = game.stack[0]; let copier = game.put(1, 3, Zone::Battlefield); priority(&mut game, 1);
    game.act(1, Move::Activate { source: copier, ability: 0, targets: vec![vec![original.into()]], paying: vec![] }).unwrap(); passes(&mut game);
    cite!(["707.10", "707.10b"]; assert_eq!(game.stack.len(), 2)); passes(&mut game);
    cite!(["603.4", "702.30a"]; assert!(game.decision.is_none()));
    cite!(["603.4", "702.30a"]; assert_eq!(game.stack, vec![original]));
}
