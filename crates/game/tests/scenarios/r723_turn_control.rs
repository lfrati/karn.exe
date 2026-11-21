//! Turn control uses a different decision-maker, never a different rules actor (723.3–5).
use crate::support::{cite, paid};
use game::simulation::Simulation;
use game::state::{Answer, Decision};
use game::{CardDef, Game, Move, ObjId, Target, Zone};
const CONTROL: &str = "You gain control of target opponent during that player's next turn. After that turn, that player takes an extra turn.";
fn card(name: &str, types: &str, text: &str, pt: Option<(i32, i32)>) -> CardDef {
    let d = CardDef::new(name, "{0}", types, text, pt).unwrap();
    game::play::check(&d).unwrap_or_else(|e| panic!("{name}: {}", e.0));
    d
}
fn emrakul() -> CardDef {
    let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let d = CardDef::from_face(faces.iter().find(|f| f.name == "Emrakul, the Promised End").unwrap()).unwrap();
    game::play::check(&d).unwrap();
    d
}
fn pass(g: &mut Game) {
    for _ in 0..g.players.iter().filter(|p| !p.lost).count() {
        g.act(g.decision_maker(g.priority), Move::Pass).unwrap();
    }
}
fn next(g: &mut Game) {
    let turn = g.turn;
    for _ in 0..80 {
        if g.turn != turn {
            return;
        }
        let action = match &g.decision {
            Some(Decision::Attackers { .. }) => Move::Attack(vec![]),
            Some(Decision::Discard { .. }) => panic!("caller must author cleanup discard"),
            None => Move::Pass,
            other => panic!("unexpected {other:?}"),
        };
        g.act(g.decision_maker(g.decision_actor()), action).unwrap();
    }
    panic!("no next turn");
}
fn setup(players: usize) -> Game {
    let ds = vec![
        card("Control", "Sorcery", CONTROL, None),
        card("Bear", "Creature — Bear", "", Some((2, 2))),
        card("Extra", "Instant", "Target player takes an extra turn after this one.", None),
    ];
    let mut g = Game::new(ds, players, 20);
    for player in 0..players {
        for _ in 0..20 {
            g.put(player, 1, Zone::Library);
        }
    }
    g
}
fn control(g: &mut Game, player: usize) {
    let spell = g.put(g.priority, 0, Zone::Hand);
    g.act(g.decision_maker(g.priority), Move::Cast { card: spell, targets: vec![vec![Target::Player(player)]] })
        .unwrap();
    pass(g);
}
#[test]
fn r723_1_control_next_turn_and_extra_turn_restore_multiplayer_normal_order() {
    let mut g = setup(3);
    control(&mut g, 1);
    assert_eq!(g.decision_maker(1), 1);
    next(&mut g);
    cite!(["723.1", "723.3"]; assert_eq!((g.active, g.decision_maker(1)), (1, 0)));
    cite!(["723.5"]; refused("that player's decisions are controlled by another player (723.5)"): g.act(1, Move::Pass));
    g.act(0, Move::Pass).unwrap();
    assert_eq!(g.priority, 2);
    next(&mut g);
    cite!(["723.1", "500.7"]; assert_eq!((g.active, g.decision_maker(1)), (1, 1)));
    next(&mut g);
    cite!(["500.7"]; assert_eq!(g.active, 2));
    next(&mut g);
    assert_eq!(g.active, 0);
}
#[test]
fn r723_emrakul_full_text_admits() {
    emrakul();
}

fn cast(g: &mut Game, def: usize, targets: Vec<Vec<Target>>) -> ObjId {
    let actor = g.priority;
    let card = g.put(actor, def, Zone::Hand);
    g.act(g.decision_maker(actor), Move::Cast { card, targets }).unwrap();
    *g.stack.last().unwrap()
}
fn add(g: &mut Game, d: CardDef) -> usize {
    let i = g.defs.len();
    std::sync::Arc::make_mut(&mut g.defs).push(d);
    i
}
#[test]
fn r500_7_later_extra_turns_go_first_and_normal_order_survives_nested_extras() {
    let mut g = setup(3);
    control(&mut g, 1);
    cast(&mut g, 2, vec![vec![Target::Player(1)]]);
    pass(&mut g);
    next(&mut g);
    cite!(["723.1", "500.7"]; assert_eq!((g.active, g.decision_maker(1)), (1, 0), "next actual turn is an extra turn"));
    cast(&mut g, 2, vec![vec![Target::Player(2)]]);
    pass(&mut g);
    next(&mut g);
    cite!(["500.7"]; assert_eq!((g.active, g.decision_maker(1)), (2, 1)));
    next(&mut g);
    cite!(["500.7"]; assert_eq!(g.active, 1, "compensating extra turn"));
    next(&mut g);
    cite!(["500.7"]; assert_eq!(g.active, 1, "ordinary turn order resumes after original player 0"));
    next(&mut g);
    assert_eq!(g.active, 2);
}
#[test]
fn r723_1a_latest_controller_wins_and_both_compensating_turns_remain() {
    let mut g = setup(3);
    control(&mut g, 2);
    next(&mut g);
    assert_eq!(g.active, 1);
    g.part = ir::Part::PrecombatMain;
    control(&mut g, 2);
    let mut copy = g.clone();
    for g in [&mut g, &mut copy] {
        next(g);
        cite!(["723.1a"]; assert_eq!((g.active, g.decision_maker(2)), (2, 1)));
        next(g);
        cite!(["723.1"]; assert_eq!((g.active, g.decision_maker(2)), (2, 2)));
        next(g);
        cite!(["500.7"]; assert_eq!(g.active, 2));
        next(g);
        assert_eq!(g.active, 0);
    }
    assert_eq!(format!("{g:?}"), format!("{copy:?}"));
}
#[test]
fn r800_4_departing_controller_ends_authority_and_departing_victim_skips_scheduled_turns() {
    for during in [false, true] {
        let mut g = setup(3);
        control(&mut g, 1);
        if during {
            next(&mut g);
            assert_eq!(g.decision_maker(1), 0);
        }
        g.players[0].lost = true;
        if g.priority == 0 {
            g.priority = 1;
        }
        if !during {
            g.part = ir::Part::End;
        }
        if !during {
            next(&mut g);
        }
        cite!(["800.4a"]; case: format!("during {during}"); assert_eq!((g.active, g.decision_maker(1)), (1, 1)));
        next(&mut g);
        assert_eq!(g.active, 1);
        next(&mut g);
        assert_eq!(g.active, 2);
    }
    let mut g = setup(3);
    control(&mut g, 1);
    g.players[1].lost = true;
    next(&mut g);
    cite!(["800.4k"]; assert_eq!(g.active, 2));
    next(&mut g);
    assert_eq!(g.active, 0);
}
#[test]
fn r723_5_simulation_uses_controllers_answers_but_victims_cards_and_resources() {
    let mut g = setup(3);
    control(&mut g, 1);
    next(&mut g);
    let costly = add(&mut g, CardDef::new("Costly", "{2}", "Creature — Bear", "", Some((2, 2))).unwrap());
    let victim = g.put(1, costly, Zone::Hand);
    let mine = g.put(0, costly, Zone::Hand);
    g.players[1].pool.colorless = 2;
    g.players[0].pool.colorless = 5;
    // Reach main phase without spending either seat's mana in the test fixture.
    g.part = ir::Part::PrecombatMain;
    let mut sim = Simulation::new(g);
    let r = sim.request().unwrap().unwrap().clone();
    cite!(["723.5"]; assert_eq!((r.player, r.acting), (0, 1)));
    assert_eq!(sim.respond(r.id, 1, Move::Pass), Err(game::simulation::ResponseError::WrongPlayer { expected: 0, received: 1 }));
    // player 0 acts for player 1 (723.5): player 1's cards, not player 0's own
    crate::support::refused(sim.respond(r.id, 0, paid("CC",Move::Cast { card: mine, targets: vec![] })), "you cast only from your own (601.3)");
    sim.respond(r.id, 0, paid("CC",Move::Cast { card: victim, targets: vec![] })).unwrap();
    let game = sim.game().unwrap();
    assert_eq!(game.players[0].pool.colorless, 5);
    cite!(["723.5a"]; assert_eq!(game.players[1].pool.colorless, 0));
    let spell = game.stack[0];
    cite!(["723.3"]; assert_eq!((game.obj(spell).owner, game.controller(spell)), (1, 1)));
    assert_eq!(sim.request().unwrap().unwrap().acting, 1);
    let mut copy = sim.clone();
    for sim in [&mut sim, &mut copy] {
        for _ in 0..3 {
            let r = sim.request().unwrap().unwrap().clone();
            sim.respond(r.id, r.player, Move::Pass).unwrap();
        }
    }
    assert_eq!(*sim.snapshot(None).unwrap(), *copy.snapshot(None).unwrap());
}
#[test]
fn r723_4_hand_and_private_search_information_is_shared_then_access_expires() {
    let mut g = setup(3);
    let mine = g.put(0, 1, Zone::Hand);
    let victim = g.put(1, 1, Zone::Hand);
    let third = g.put(2, 1, Zone::Hand);
    let search = add(
        &mut g,
        card(
            "Tutor",
            "Instant",
            "Search your library for a creature card, reveal it, put it into your hand, then shuffle.",
            None,
        ),
    );
    control(&mut g, 1);
    next(&mut g);
    cite!(["723.4"]; assert!(g.sees(0, mine) && g.sees(0, victim) && !g.sees(0, third)));
    cite!(["723.4"]; assert!(g.sees(1, victim) && !g.sees(1, mine)));
    cite!(["402.3"]; assert!(!g.sees(2, victim)));
    let top = g.players[1].library[0];
    assert!(!g.sees(0, top));
    cast(&mut g, search, vec![]);
    pass(&mut g);
    assert!(matches!(g.decision, Some(Decision::Choose { player: 1, .. })));
    cite!(["723.4"]; assert!(g.sees(0, top) && g.sees(1, top) && !g.sees(2, top)));
    cite!(["723.5"]; refused("that player's decisions are controlled by another player (723.5)"): g.act(1, Move::Choose(Answer::Objects(vec![]))));
    g.act(0, Move::Choose(Answer::Objects(vec![]))).unwrap();
    assert!(!g.sees(0, top));
    next(&mut g);
    cite!(["723.1", "723.4"]; assert!(!g.sees(0, victim) && g.sees(1, victim)));
}
#[test]
fn r723_4_look_disclosures_keep_both_entitled_recipients_after_control_ends() {
    let mut g = setup(3);
    let look = add(&mut g, card("Inspection", "Instant", "Look at the top card of your library.", None));
    control(&mut g, 1);
    next(&mut g);
    cast(&mut g, look, vec![]);
    pass(&mut g);
    next(&mut g);
    let mut sim = Simulation::new(g);
    let a = sim.take_observations(0).unwrap();
    let b = sim.take_observations(1).unwrap();
    assert!(!a.is_empty());
    cite!(["723.4"]; assert_eq!(a, b));
    cite!(["701.20e", "723.4"]; assert!(sim.take_observations(2).unwrap().is_empty()));
}
#[test]
fn r601_2f_emrakul_counts_each_graveyard_type_once_including_kindred_and_ignores_opponent() {
    let mut g = setup(3);
    let em = add(&mut g, emrakul());
    let multi = add(&mut g, card("Multi", "Artifact Creature — Golem", "", Some((1, 1))));
    let kindred = add(&mut g, card("Kindred", "Kindred Sorcery — Eldrazi", "", None));
    g.put(0, multi, Zone::Graveyard);
    g.put(0, multi, Zone::Graveyard);
    g.put(0, kindred, Zone::Graveyard);
    g.put(1, 2, Zone::Graveyard); // opponent's instant is not a fifth type
    let card = g.put(0, em, Zone::Hand);
    g.players[0].pool.colorless = 8;
    let before = format!("{g:?}");
    cite!(["601.2f"]; refused("the supplied mana does not pay that cost, all of it and no more (601.2h, 118.3a)"): g.act(0, paid("CCCCCCCC",Move::Cast { card, targets: vec![] })));
    assert_eq!(format!("{g:?}"), before);
    g.players[0].pool.colorless = 9;
    g.act(0, paid("CCCCCCCCC",Move::Cast { card, targets: vec![] })).unwrap();
    cite!(["601.2f"]; assert_eq!(g.players[0].pool.colorless, 0));
    assert!(matches!(g.decision, Some(Decision::Targets { player: 0, .. })));
    cite!(["115.1"]; refused("an illegal target, you: you aren't an opponent (115.1)"): g.act(0, Move::ChooseTargets(vec![vec![Target::Player(0)]])));
    g.act(0, Move::ChooseTargets(vec![vec![Target::Player(2)]])).unwrap();
    let spell = g.stack.iter().copied().find(|s| g.obj(*s).ability.is_none()).unwrap();
    assert!(g.counter(spell)); // cast ability is independent of the spell
    let mut copy = g.clone();
    for g in [&mut g, &mut copy] {
        pass(g);
        next(g);
        assert_eq!(g.active, 1);
        next(g);
        cite!(["113.7a", "723.1"]; assert_eq!((g.active, g.decision_maker(2)), (2, 0)));
        next(g);
        cite!(["500.7"]; assert_eq!((g.active, g.decision_maker(2)), (2, 2)));
        next(g);
        assert_eq!(g.active, 0);
    }
}
#[test]
fn r603_emrakul_countered_trigger_and_put_onto_battlefield_do_not_schedule_control() {
    let mut g = setup(2);
    let em = add(&mut g, emrakul());
    g.put(0, em, Zone::Battlefield);
    assert!(g.stack.is_empty());
    let card = g.put(0, em, Zone::Hand);
    g.players[0].pool.colorless = 13;
    g.act(0, paid("CCCCCCCCCCCCC",Move::Cast { card, targets: vec![] })).unwrap();
    g.act(0, Move::ChooseTargets(vec![vec![Target::Player(1)]])).unwrap();
    let trigger = *g.stack.last().unwrap();
    assert!(g.counter(trigger));
    let spell = g.stack[0];
    assert!(g.counter(spell));
    next(&mut g);
    cite!(["701.6a"]; assert_eq!((g.active, g.decision_maker(1)), (1, 1)));
    next(&mut g);
    assert_eq!(g.active, 0);
}
#[test]
fn r702_16_emrakul_protection_tests_source_types_and_last_known_information() {
    let mut g = setup(2);
    let em = add(&mut g, emrakul());
    let guard = g.put(0, em, Zone::Battlefield);
    let instant = g.put(1, 2, Zone::Stack);
    let sorcery = g.put(1, 0, Zone::Stack);
    let bear = g.put(1, 1, Zone::Battlefield);
    cite!(["702.16b"]; assert!(!g.can_target(&game::select::Ctx::new(1, instant), Target::Object(guard))));
    cite!(["702.16b"]; assert!(g.can_target(&game::select::Ctx::new(1, sorcery), Target::Object(guard))));
    g.damage(instant, Target::Object(guard), 4);
    cite!(["702.16e"]; assert_eq!(g.obj(guard).damage, 0));
    g.exile(instant);
    g.damage(instant, Target::Object(guard), 4);
    cite!(["702.16a", "702.16e"]; assert_eq!(g.obj(guard).damage, 0));
    g.damage(bear, Target::Object(guard), 1);
    g.damage(sorcery, Target::Object(guard), 1);
    cite!(["702.16e"]; assert_eq!(g.obj(guard).damage, 2));
    cite!(["702.9b"]; assert!(g.cant_block(bear, guard, &[bear]), "flying is the ordinary keyword query"));
    let spell = g.put(0, em, Zone::Stack);
    cite!(["113.6"]; assert!(
        g.can_target(&game::select::Ctx::new(1, instant), Target::Object(spell)),
        "protection functions only on battlefield"
    ));
}
#[test]
fn admission_refuses_control_durations_and_turn_references_not_implemented() {

    for text in [
        "You gain control of target opponent.",
        "You gain control of target opponent until end of turn.",
        "You gain control of target opponent during your next turn.",
        "You gain control of two target opponents during that player's next turn.",
        "After that turn, you take an extra turn.",
    ] {
        let d = CardDef::new("Unsupported", "{0}", "Sorcery", text, None).unwrap();
        let reason=if text.contains("for each") {"modifier: ForEach of Keyword"} else if text.starts_with("After") || text.starts_with("If") || text.contains("beginning of the next end step") {"that turn: no preceding future-turn anchor"} else if text.contains("two target") {"player control: controller chooses one target player's next turn"} else if text.contains("during your") {"player control: duration must name the controlled player's next turn"} else {"player control: during that player's next turn"};
        crate::support::refused(game::play::check(&d).map_err(|e|e.0),reason);
    }
}
#[test]
fn r723_5_controller_makes_attack_and_damage_assignments_while_defender_keeps_their_choices() {
    let mut g = setup(3);
    let em = add(&mut g, emrakul());
    let attacker = g.put(1, em, Zone::Battlefield);
    let reach = add(&mut g, card("Spider", "Creature — Spider", "Reach", Some((3, 3))));
    let blocker = g.put(2, reach, Zone::Battlefield);
    control(&mut g, 1);
    next(&mut g);
    while !matches!(g.decision, Some(Decision::Attackers { .. })) {
        g.act(g.decision_maker(g.priority), Move::Pass).unwrap();
    }
    cite!(["723.5"]; refused("that player's decisions are controlled by another player (723.5)"): g.act(1, Move::Attack(vec![])));
    cite!(["723.5"]; ok: g.act(0, Move::Attack(vec![(attacker, Target::Player(2))])));
    pass(&mut g);
    assert!(matches!(g.decision, Some(Decision::Blockers { player: 2 })));
    cite!(["723.5"]; refused("that decision is another player's"): g.act(0, Move::Block(vec![])));
    g.act(2, Move::Block(vec![(blocker, attacker)])).unwrap();
    pass(&mut g);
    assert!(matches!(g.decision, Some(Decision::Divide { player: 1, .. })));
    cite!(["723.5"]; ok: g.act(0, Move::Divide(vec![(Target::Object(blocker), 3), (Target::Player(2), 10)])));
    cite!(["702.19b"]; assert_eq!(g.players[2].life, 10));
    assert_eq!(g.obj(attacker).damage, 3);
    cite!(["723.3"]; assert_eq!(g.controller(attacker), 1));
}
#[test]
fn r723_5_paused_private_choice_clones_and_accepts_the_decision_makers_explicit_answer() {
    let mut g = setup(3);
    control(&mut g, 1);
    next(&mut g);
    let scry = add(&mut g, card("Consideration", "Instant", "Scry 2. You may draw a card.", None));
    cast(&mut g, scry, vec![]);
    pass(&mut g);
    let mut sim = Simulation::new(g);
    let r = sim.request().unwrap().unwrap().clone();
    cite!(["723.5"]; assert_eq!((r.player, r.acting), (0, 1)));
    assert!(matches!(r.question, game::legal::Asked::Arrange { .. }));
    let mut copy = sim.clone();
    for sim in [&mut sim, &mut copy] {
        let game::legal::Asked::Arrange { cards, .. } = &r.question else { panic!("scry arrangement") };
        sim.respond(r.id, 0, Move::Choose(Answer::Arrange { top: cards.clone(), away: vec![] })).unwrap();
        let ask = sim.request().unwrap().unwrap().clone();
        cite!(["723.5"]; assert_eq!((ask.player, ask.acting), (0, 1)));
        assert_eq!(sim.respond(ask.id, 1, Move::Choose(Answer::No)), Err(game::simulation::ResponseError::WrongPlayer { expected: 0, received: 1 }));
        sim.respond(ask.id, 0, Move::Choose(Answer::No)).unwrap();
    }
    assert_eq!(*sim.snapshot(None).unwrap(), *copy.snapshot(None).unwrap());
}
#[test]
fn r500_7_counted_extra_turns_and_apnap_recipients_are_scheduled_in_creation_order() {
    let mut g = setup(3);
    let extra = add(&mut g, card("Everyone", "Sorcery", "Each player takes two extra turns after this one.", None));
    cast(&mut g, extra, vec![]);
    pass(&mut g);
    let mut order = Vec::new();
    for _ in 0..7 {
        next(&mut g);
        order.push(g.active);
    }
    cite!(["500.7"]; assert_eq!(order, [2, 2, 1, 1, 0, 0, 1]));
}

fn sim_next(sim: &mut Simulation) {
    let turn = sim.game().unwrap().turn;
    for _ in 0..100 {
        if sim.game().unwrap().turn != turn {
            return;
        }
        let r = sim.request().unwrap().unwrap().clone();
        let mv = match &r.question {
            game::legal::Asked::Priority => Move::Pass,
            game::legal::Asked::Attackers(_) => Move::Attack(vec![]),
            q => panic!("unexpected {q:?}"),
        };
        sim.respond(r.id, r.player, mv).unwrap();
    }
    panic!("no next turn");
}
#[test]
fn r723_emrakul_pending_control_retains_no_source_after_managed_collection_and_clones() {
    let mut g = setup(4);
    let em = add(&mut g, emrakul());
    let card = g.put(0, em, Zone::Hand);
    g.players[0].pool.colorless = 13;
    g.act(0, paid("CCCCCCCCCCCCC",Move::Cast { card, targets: vec![] })).unwrap();
    g.act(0, Move::ChooseTargets(vec![vec![Target::Player(3)]])).unwrap();
    let spell = g.stack.iter().copied().find(|s| g.obj(*s).ability.is_none()).unwrap();
    assert!(g.counter(spell));
    let mut sim = Simulation::new(g);
    for _ in 0..4 {
        let r = sim.request().unwrap().unwrap().clone();
        sim.respond(r.id, r.player, Move::Pass).unwrap();
    }
    // Casting history keeps this and the following turn. After both intervening turns
    // begin, only the source-independent scheduled effect remains.
    sim_next(&mut sim);
    sim_next(&mut sim);
    assert_eq!(sim.game().unwrap().active, 2);
    assert!(sim.game().unwrap().get_object(spell).is_none());
    let mut copy = sim.clone();
    for sim in [&mut sim, &mut copy] {
        sim_next(sim);
        cite!(["723.1"]; assert_eq!((sim.game().unwrap().active, sim.request().unwrap().unwrap().player), (3, 0)));
        sim_next(sim);
        cite!(["500.7"]; assert_eq!((sim.game().unwrap().active, sim.request().unwrap().unwrap().player), (3, 3)));
        sim_next(sim);
        assert_eq!(sim.game().unwrap().active, 0);
    }
    assert_eq!(*sim.snapshot(None).unwrap(), *copy.snapshot(None).unwrap());
}
#[test]
fn admission_refuses_future_turn_anchors_that_escape_their_resolution_or_branch() {

    for text in [
        "If you control a creature, you gain control of target opponent during that player's next turn. After that turn, that player takes an extra turn.",
        "You gain control of target opponent during that player's next turn. That player takes an extra turn after that turn at the beginning of the next end step.",
        "Target player takes an extra turn after this one for each creature you control.",
    ] {
        let d = CardDef::new("Unsupported", "{0}", "Sorcery", text, None).unwrap();
        let reason=if text.contains("for each") {"modifier: ForEach of Keyword"} else if text.starts_with("After") || text.starts_with("If") || text.contains("beginning of the next end step") {"that turn: no preceding future-turn anchor"} else if text.contains("two target") {"player control: controller chooses one target player's next turn"} else if text.contains("during your") {"player control: duration must name the controlled player's next turn"} else {"player control: during that player's next turn"};
        crate::support::refused(game::play::check(&d).map_err(|e|e.0),reason);
    }
}


#[test]
fn r723_5_controller_answers_target_free_trigger_modes_for_controlled_player() {

    let mut g = Game::new(vec![
        card("Control", "Sorcery", CONTROL, None),
        card("Watcher", "Creature", "Flash\nWhen this creature enters, choose up to one —\n• You gain 2 life.\n• You gain 3 life.", Some((2,2))),
    ], 2, 20);
    for p in 0..2 { for _ in 0..12 { g.put(p,1,Zone::Library); } }
    control(&mut g,1);
    next(&mut g);
    assert_eq!((g.active,g.decision_maker(1)),(1,0));
    let watcher = g.put(1,1,Zone::Hand);
    g.act(0,Move::Cast { card:watcher, targets:vec![] }).unwrap();
    pass(&mut g);
    assert!(matches!(g.decision,Some(Decision::Modes { player:1,.. })));
    let mut declined = g.clone();
    let before = format!("{g:?}");
    cite!(["723.5"]; refused("that player's decisions are controlled by another player (723.5)"): g.act(1,Move::ChooseModes(vec![0])));
    assert_eq!(format!("{g:?}"),before);
    g.act(0,Move::ChooseModes(vec![0])).unwrap();
    cite!(["603.3c","723.5"]; assert!(g.decision.is_none()));
    assert_eq!(g.obj(g.stack[0]).controller,1);
    pass(&mut g);
    cite!(["109.5","608.2c"]; assert_eq!((g.players[0].life,g.players[1].life),(20,22)));
    declined.act(0,Move::ChooseModes(vec![])).unwrap();
    cite!(["603.3c","723.5"]; assert!(declined.decision.is_none() && declined.stack.is_empty()));
    assert_eq!((declined.players[0].life,declined.players[1].life),(20,20));
}
