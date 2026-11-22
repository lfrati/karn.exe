//! Actual draw-time reveal, linked casting price, and bounded public visibility (702.94).
use crate::support::{cite, mana_pool, oracle_read, paid};
use game::announcement::CastCost;
use game::state::{Answer, Choice, Decision};
use game::{CardDef, Event, Game, Move, ObjId, Target, Zone};

fn setup() -> Game {
    let defs = vec![oracle_read("Thunderous Wrath"), oracle_read("Mountain"), oracle_read("Entreat the Angels"),
        CardDef::new("Draw two", "{0}", "Instant", "Draw two cards.", None).unwrap(),
        CardDef::new("Counter ability", "{0}", "Instant", "Counter target activated or triggered ability.", None).unwrap(),
        CardDef::new("Copy ability", "{0}", "Artifact", "{0}: Copy target activated or triggered ability.", None).unwrap(), oracle_read("Disdainful Stroke"),
        CardDef::new("Mana and draw", "{0}", "Enchantment", "Whenever you tap a land for mana, add {R} and draw a card.", None).unwrap(), oracle_read("Thought Reflection"), oracle_read("Stinkweed Imp")];
    // Uncited (admission): the full pinned cards and generic composition fixtures.
    for def in &defs { game::play::check(def).unwrap_or_else(|why| panic!("{}: {}", def.name(), why.0)); }
    let mut game = Game::new(defs, 2, 20);
    for player in 0..2 { for _ in 0..10 { game.put(player, 1, Zone::Library); } }
    game
}
fn passes(game: &mut Game) { for _ in 0..2 { game.act(game.priority, Move::Pass).unwrap(); } }
fn drawn(game: &Game) -> ObjId {
    let Some(Decision::Choose { player: 0, choice: Choice::Miracle { card, .. } }) = &game.decision else { panic!("{:?}", game.decision) };
    *card
}
fn reveal(game: &mut Game) -> ObjId {
    let card = drawn(game); game.act(0, Move::Choose(Answer::Yes)).unwrap(); card
}
fn cast_wrath(card: ObjId) -> Move {
    paid("R", Move::CastFor(CastCost::Alternative(1), Box::new(Move::Cast { card, targets: vec![vec![Target::Player(1)]] })))
}

#[test]
fn r702_94a_actual_thunderous_wrath_suspends_a_real_multi_draw_then_casts_from_hand() {
    let mut game = setup();
    let second = game.put(0, 1, Zone::Library);
    game.put(0, 0, Zone::Library);
    let draw = game.put(0, 3, Zone::Hand);
    game.act(0, Move::Cast { card: draw, targets: vec![] }).unwrap(); passes(&mut game);
    let card = drawn(&game);
    cite!(["121.2", "702.94a"]; assert_eq!(game.obj(second).zone, Zone::Library));
    cite!(["702.94a", "702.94b"]; assert!(!game.sees(1, card)));
    let before = format!("{game:?}");
    cite!(["117.1", "702.94a"]; refused("that decision is another player's"): game.act(1, Move::Choose(Answer::Yes)));
    assert_eq!(format!("{game:?}"), before);
    reveal(&mut game);
    cite!(["121.2", "702.94a"]; assert_eq!(game.players[0].hand.len(), 2));
    cite!(["702.94b"]; assert!(game.sees(1, card)));
    cite!(["702.94a", "603.3"]; assert_eq!(game.stack.len(), 1));
    passes(&mut game);
    cite!(["702.94a", "608.2g"]; assert!(matches!(&game.decision, Some(Decision::Choose { choice: Choice::Cast { cards }, .. }) if cards == &[card])));
    cite!(["702.94b"]; assert!(game.sees(1, card)));
    game.players[0].pool = mana_pool("R");
    cite!(["702.94a", "118.9a"]; ok: game.act(0, cast_wrath(card)));
    let spell = game.stack[0];
    cite!(["702.94a", "601.2a"]; assert_eq!(game.obj(spell).chars.name, "Thunderous Wrath"));
    passes(&mut game);
    cite!(["702.94a", "608.2c"]; assert_eq!(game.players[1].life, 15));
    cite!(["121.2", "702.94a"]; assert_eq!(game.log.iter().filter(|event| matches!(event, Event::Drew { player: 0, .. })).count(), 2));
}

#[test]
fn r702_94a_declining_the_reveal_never_opens_a_cast_or_discloses_the_card() {
    let mut game = setup(); game.put(0, 0, Zone::Library); game.draw(0, 1);
    let card = drawn(&game); game.act(0, Move::Choose(Answer::No)).unwrap();
    cite!(["702.94a"]; assert!(game.stack.is_empty()));
    cite!(["702.94a", "402.3"]; assert!(!game.sees(1, card)));
    cite!(["702.94a"]; assert!(game.observations(1).is_empty()));
    game.players[0].pool = mana_pool("R");
    let before = format!("{game:?}");
    cite!(["702.94a", "118.9"]; refused("that cost is paid only for a cast its ability offers (702.94a)"): game.act(0, cast_wrath(card)));
    assert_eq!(format!("{game:?}"), before);
}

#[test]
fn r702_94b_declining_the_linked_cast_ends_current_visibility_but_keeps_the_disclosure() {
    let mut game = setup(); game.put(0, 0, Zone::Library); game.draw(0, 1);
    let card = reveal(&mut game); passes(&mut game);
    cite!(["702.94b"]; assert!(game.sees(1, card)));
    game.act(0, Move::Choose(Answer::No)).unwrap();
    cite!(["702.94b"]; assert!(!game.sees(1, card)));
    cite!(["702.94a"]; assert_eq!(game.obj(card).zone, Zone::Hand));
    cite!(["701.20a"]; assert_eq!(game.observations(1).len(), 1));
}

#[test]
fn r702_94a_only_the_first_actual_draw_can_be_revealed_even_on_an_opponents_turn() {
    for first in [true, false] {
        let mut game = setup();
        game.active = 1;
        if !first { game.draw(0, 1); }
        game.put(0, 0, Zone::Library); game.draw(0, 1);
        if first {
            let card = drawn(&game);
            cite!(["702.94a"]; assert_eq!(game.obj(card).owner, 0));
            reveal(&mut game); passes(&mut game);
            game.players[0].pool = mana_pool("R");
            cite!(["702.94a", "608.2g"]; ok: game.act(0, cast_wrath(card)));
            passes(&mut game);
            cite!(["702.94a"]; assert_eq!(game.players[1].life, 15));
        } else {
            cite!(["702.94a"]; assert!(game.decision.is_none()));
            cite!(["702.94a"]; assert!(game.stack.is_empty()));
        }
    }
}

#[test]
fn r702_94a_entreat_the_angels_locks_x_in_the_linked_price_and_preserves_printed_mana_value() {
    let mut game = setup(); game.put(0, 2, Zone::Library); game.draw(0, 1);
    let card = reveal(&mut game); passes(&mut game);
    game.players[0].pool = mana_pool("CCWW");
    cite!(["702.94a", "107.3a", "601.2b"]; ok: game.act(0, paid("CCWW", Move::WithX(2,
        Box::new(Move::CastFor(CastCost::Alternative(1), Box::new(Move::Cast { card, targets: vec![] })))))));
    let spell = game.stack[0];
    cite!(["107.3a"]; assert_eq!(game.obj(spell).x, Some(2)));
    let mut counter = game.clone(); counter.act(0, Move::Pass).unwrap();
    let stroke = counter.put(1, 6, Zone::Hand); counter.players[1].pool = mana_pool("CU");
    // Entreat's printed {X}{X}{W}{W}{W} has value 7; the {X}{W}{W} price does not replace it.
    cite!(["202.3e", "118.9c"]; ok: counter.act(1, paid("CU", Move::Cast { card: stroke, targets: vec![vec![Target::Object(spell)]] })));
    passes(&mut game);
    cite!(["702.94a", "107.3a"]; assert_eq!(game.battlefield.len(), 2));
    cite!(["702.94a"]; assert!(game.battlefield.iter().all(|id| game.seen(*id, None).pt == Some((4, 4)))));
}

#[test]
fn r702_94b_countering_the_original_ends_its_reveal_even_while_a_copy_can_still_offer_the_cast() {
    for controller in 0..2 {
    let mut game = setup(); game.put(0, 0, Zone::Library); game.draw(0, 1);
    let card = reveal(&mut game); let original = game.stack[0];
    let copier = game.put(controller, 5, Zone::Battlefield);
    if controller == 1 { game.act(0, Move::Pass).unwrap(); }
    game.act(controller, Move::Activate { source: copier, ability: 0, targets: vec![vec![Target::Object(original)]], paying: vec![] }).unwrap();
    passes(&mut game);
    cite!(["707.10"]; assert_eq!(game.stack.len(), 2));
    let counter = game.put(0, 4, Zone::Hand);
    game.act(0, Move::Cast { card: counter, targets: vec![vec![Target::Object(original)]] }).unwrap(); passes(&mut game);
    cite!(["702.94b", "707.10"]; assert!(!game.sees(1, card)));
    cite!(["707.10"]; assert_eq!(game.stack.len(), 1));
    passes(&mut game);
    cite!(["702.94a", "707.10"]; assert!(matches!(&game.decision, Some(Decision::Choose { player, choice: Choice::Cast { cards } }) if *player == controller && cards == &[card])));
    game.players[controller].pool = mana_pool("R");
    cite!(["702.94a", "707.10"]; ok: game.act(controller, cast_wrath(card)));
    passes(&mut game);
    cite!(["702.94a"]; assert_eq!(game.players[1].life, 15));
    }
}

#[test]
fn r702_94b_countering_the_trigger_ends_visibility_and_does_not_leave_the_alternative_cost_live() {
    let mut game = setup(); game.put(0, 0, Zone::Library); game.draw(0, 1);
    let card = reveal(&mut game); let trigger = game.stack[0];
    let counter = game.put(0, 4, Zone::Hand);
    game.act(0, Move::Cast { card: counter, targets: vec![vec![Target::Object(trigger)]] }).unwrap(); passes(&mut game);
    cite!(["702.94b"]; assert!(!game.sees(1, card)));
    game.players[0].pool = mana_pool("R");
    let before = format!("{game:?}");
    cite!(["702.94a"]; refused("that cost is paid only for a cast its ability offers (702.94a)"): game.act(0, cast_wrath(card)));
    assert_eq!(format!("{game:?}"), before);
}

#[test]
fn r702_94a_independent_printed_instances_offer_only_the_price_used_for_their_reveal() {
    let def = CardDef::new("Two prices", "{5}{R}", "Instant",
        "This spell deals 1 damage to any target.\nMiracle {R}\nMiracle {1}{R}", None).unwrap();
    game::play::check(&def).unwrap();
    let mut game = Game::new(vec![def, oracle_read("Mountain")], 2, 20);
    for player in 0..2 { game.put(player, 1, Zone::Library); }
    game.put(0, 0, Zone::Library); game.draw(0, 1);
    let card = drawn(&game);
    game.act(0, Move::Choose(Answer::No)).unwrap();
    cite!(["113.2c", "702.94a"]; assert!(matches!(&game.decision, Some(Decision::Choose { choice: Choice::Miracle { card: again, .. }, .. }) if *again == card)));
    reveal(&mut game); passes(&mut game);
    game.players[0].pool = mana_pool("CR");
    let before = format!("{game:?}");
    cite!(["607.1", "702.94a"]; refused("the effect lets that card be cast only for the cost it offers (118.9a)"): game.act(0, cast_wrath(card)));
    assert_eq!(format!("{game:?}"), before);
    cite!(["113.2c", "702.94a"]; ok: game.act(0, paid("CR", Move::CastFor(CastCost::Alternative(2),
        Box::new(Move::Cast { card, targets: vec![vec![Target::Player(1)]] })))));
    passes(&mut game);
    cite!(["702.94a"]; assert_eq!(game.players[1].life, 19));
}

#[test]
fn r702_94a_declared_positions_do_not_fabricate_first_draw_history() {
    let mut game = setup();
    game.prepare_initial_position(3, ir::Part::PrecombatMain, 0, 0, 0, &[]).unwrap();
    game.finish_initial_position().unwrap();
    game.put(0, 0, Zone::Library);
    let draw = game.put(0, 3, Zone::Hand);
    game.act(0, Move::Cast { card: draw, targets: vec![] }).unwrap(); game.act(0, Move::Pass).unwrap();
    let before = format!("{game:?}");
    let error = game.act(1, Move::Pass).unwrap_err().to_string();
    cite!(["702.94a"]; assert!(error.contains("declared position does not supply earlier draws"), "{error}"));
    assert_eq!(format!("{game:?}"), before);
}

#[test]
fn r121_8_draws_during_a_cast_require_deferred_disclosure_and_refuse_without_changing_state() {
    let mut game = setup();
    game.put(0, 7, Zone::Battlefield);
    let source = game.put(0, 1, Zone::Battlefield);
    game.put(0, 2, Zone::Library);
    let wrath = game.put(0, 0, Zone::Hand);
    game.players[0].pool = mana_pool("CCCC");
    let before = format!("{game:?}");
    let error = game.act(0, paid("CCCCRR", Move::WithMana(vec![game::legal::Step { source, ability: 0, paying: vec![], adding: vec![], plan: vec![], x: None, life: vec![], with: None, chose: vec![], cost_branches: None }],
        Box::new(Move::Cast { card: wrath, targets: vec![vec![Target::Player(1)]] })))).unwrap_err().to_string();
    cite!(["121.8"]; assert!(error.contains("deferred draw disclosure"), "{error}"));
    assert_eq!(format!("{game:?}"), before);
}

#[test]
fn r103_5_opening_hands_precede_any_miracle_window_and_the_first_real_turn_draw_still_qualifies() {
    let defs = vec![oracle_read("Thunderous Wrath")];
    let mut game = Game::start(defs, &[vec![0; 12], vec![0; 12]], 20, 0, 17);
    cite!(["103.5", "702.94a"]; assert!(matches!(game.decision, Some(Decision::Mulligan { .. }))));
    cite!(["103.5"]; assert_eq!(game.players[0].hand.len(), 7));
    cite!(["103.5"]; assert_eq!(game.players[1].hand.len(), 7));
    for _ in 0..2 { let Some(Decision::Mulligan { player }) = game.decision else { panic!("mulligan") }; game.act(player, Move::Keep).unwrap(); }
    for _ in 0..150 {
        if let Some(Decision::Choose { player, choice: Choice::Miracle { .. } }) = &game.decision {
            cite!(["103.8a", "702.94a"]; assert_eq!((*player, game.turn, game.active), (1, 2, 1)));
            return;
        }
        match game.decision {
            Some(Decision::Attackers { player }) => game.act(player, Move::Attack(vec![])).unwrap(),
            None => game.act(game.priority, Move::Pass).unwrap(),
            ref other => panic!("unexpected pregame or turn decision {other:?}"),
        }
    }
    panic!("no first turn-based draw");
}

#[test]
fn r702_94a_simulation_clones_keep_the_exact_unfinished_draw_and_independent_reveal_answers() {
    use game::legal::Asked;
    use game::simulation::Simulation;
    let mut game = setup(); game.put(0, 0, Zone::Library); game.draw(0, 2);
    let card = drawn(&game); let library = game.players[0].library.len();
    let mut reveal = Simulation::new(game); let mut decline = reveal.clone();
    for (simulation, yes) in [(&mut reveal, true), (&mut decline, false)] {
        let request = simulation.request().unwrap().unwrap().clone();
        cite!(["702.94a"]; assert!(matches!(request.question, Asked::Miracle { card: drawn, .. } if drawn == card)));
        cite!(["702.94a", "402.3"]; assert!(!simulation.snapshot(Some(1)).unwrap().objects.iter().any(|object| object.id == card)));
        simulation.respond(request.id, request.player, Move::Choose(if yes { Answer::Yes } else { Answer::No })).unwrap();
        let game = simulation.game().unwrap();
        cite!(["121.2", "702.94a"]; assert_eq!(game.players[0].library.len(), library - 1));
        // Simulation retires delivered events; the new event retains its real ordinal.
        cite!(["121.2"]; assert!(game.log.iter().any(|event| matches!(event, Event::Drew { player: 0, ordinal: 2, .. }))));
        cite!(["121.2"]; assert_eq!(game.players[0].hand.len(), 2));
        cite!(["702.94a", "702.94b"]; assert_eq!(game.sees(1, card), yes));
        cite!(["702.94a"]; assert_eq!(game.stack.len(), usize::from(yes)));
    }
}

#[test]
fn r702_94a_a_departure_and_return_do_not_transfer_the_linked_cast_to_the_new_hand_incarnation() {
    let mut game = setup(); game.put(0, 0, Zone::Library); game.draw(0, 1);
    let card = reveal(&mut game);
    game.move_object(card, Zone::Graveyard);
    let graveyard = *game.players[0].graveyard.last().unwrap();
    game.move_object(graveyard, Zone::Hand);
    let returned = *game.players[0].hand.last().unwrap();
    cite!(["400.7", "702.94b"]; assert_ne!(card, returned));
    cite!(["702.94b"]; assert!(!game.sees(1, returned)));
    passes(&mut game);
    if let Some(Decision::Choose { choice: Choice::Cast { cards }, .. }) = &game.decision {
        cite!(["400.7", "702.94a"]; assert!(!cards.contains(&returned)));
        game.act(0, Move::Choose(Answer::No)).unwrap();
    }
    game.players[0].pool = mana_pool("R");
    let before = format!("{game:?}");
    cite!(["400.7", "702.94a"]; refused("that cost is paid only for a cast its ability offers (702.94a)"): game.act(0, cast_wrath(returned)));
    assert_eq!(format!("{game:?}"), before);
}

#[test]
fn r614_5_a_replacement_added_multi_draw_finishes_the_first_reveal_before_its_sibling_and_parent() {
    let mut game = setup(); game.put(0, 8, Zone::Battlefield);
    let second = game.put(0, 1, Zone::Library); game.put(0, 0, Zone::Library);
    game.draw(0, 1);
    cite!(["121.2", "614.5", "702.94a"]; assert_eq!(game.obj(second).zone, Zone::Library));
    reveal(&mut game);
    cite!(["121.2", "614.5"]; assert_eq!(game.log.iter().filter(|event| matches!(event, Event::Drew { player: 0, .. })).count(), 2));
    cite!(["702.94a"]; assert_eq!(game.stack.len(), 1));
    cite!(["121.2"]; assert_eq!(game.players[0].hand.len(), 2));
}

#[test]
fn r702_52a_a_dredged_card_is_not_a_draw_and_the_next_actual_draw_can_use_miracle() {
    let mut game = setup(); game.put(0, 9, Zone::Graveyard);
    game.put(0, 0, Zone::Library); for _ in 0..5 { game.put(0, 1, Zone::Library); }
    game.draw(0, 2);
    let Some(Decision::Choose { choice: Choice::Replacement { among, .. }, .. }) = &game.decision else { panic!("dredge choice") };
    let replacement = among[0].clone();
    game.act(0, Move::Choose(Answer::Replacement(replacement))).unwrap();
    let Some(Decision::Choose { choice: Choice::OrderCards { cards, .. }, .. }) = &game.decision else { panic!("milled card order") };
    let order = cards.clone(); game.act(0, Move::Choose(Answer::Objects(order))).unwrap();
    let card = drawn(&game);
    cite!(["702.52a", "614.6", "702.94a"]; assert_eq!(game.obj(card).chars.name, "Thunderous Wrath"));
    cite!(["702.52a", "121.1"]; assert_eq!(game.log.iter().filter(|event| matches!(event, Event::Drew { player: 0, .. })).count(), 1));
    cite!(["702.52a"]; assert!(game.players[0].hand.iter().any(|id| game.obj(*id).chars.name == "Stinkweed Imp")));
    reveal(&mut game);
    cite!(["702.94a"]; assert_eq!(game.stack.len(), 1));
}
