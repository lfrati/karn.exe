//! A trigger condition binds the participants its typed event roles guarantee (608.2k).
use crate::support::cite;
use game::{CardDef, Game, Move, Target, Zone};

fn watcher(text: &str) -> CardDef {
    CardDef::new("Witness", "{0}", "Creature — Human", text, Some((2, 4))).unwrap()
}
fn pass(g: &mut Game) {
    for _ in 0..g.players.iter().filter(|p| !p.lost).count() {
        g.act(g.priority, Move::Pass).unwrap();
    }
}
#[test]
fn r608_2k_damage_recipient_is_bound_from_the_typed_event_role() {
    let d = watcher("Whenever this creature deals damage to a player, that player loses 2 life.");
    game::play::check(&d).unwrap();
    let mut g = Game::new(vec![d], 3, 20);
    let source = g.put(0, 0, Zone::Battlefield);
    g.damage(source, Target::Player(2), 1);
    g.act(0, Move::Pass).unwrap();
    cite!(["603.2"]; assert_eq!(g.stack.len(), 1));
    pass(&mut g);
    cite!(["608.2k"]; assert_eq!(g.players.iter().map(|p| p.life).collect::<Vec<_>>(), vec![20, 20, 17]));
}
#[test]
fn r603_2c_separate_triggers_retain_separate_damaged_players() {
    let d = watcher("Whenever this creature deals damage to a player, that player loses 2 life.");
    game::play::check(&d).unwrap();
    let mut g = Game::new(vec![d], 3, 20);
    let source = g.put(0, 0, Zone::Battlefield);
    g.damage(source, Target::Player(1), 1);
    g.act(0, Move::Pass).unwrap();
    g.damage(source, Target::Player(2), 1);
    g.act(g.priority, Move::Pass).unwrap();
    cite!(["603.2c"]; assert_eq!(g.stack.len(), 2));
    pass(&mut g);
    cite!(["608.2k"]; assert_eq!((g.players[1].life, g.players[2].life), (19, 17)));
    pass(&mut g);
    cite!(["608.2k"]; assert_eq!((g.players[1].life, g.players[2].life), (17, 17)));
}
#[test]
fn r113_7a_a_bound_damage_player_survives_the_trigger_source_leaving() {
    let d = watcher("Whenever this creature deals damage to a player, that player loses 2 life.");
    game::play::check(&d).unwrap();
    let mut g = Game::new(vec![d], 3, 20);
    let source = g.put(0, 0, Zone::Battlefield);
    g.damage(source, Target::Player(1), 1);
    g.act(0, Move::Pass).unwrap();
    g.destroy(source);
    pass(&mut g);
    cite!(["113.7a"]; assert_eq!(g.players[1].life, 17));
}
#[test]
fn r608_2k_trigger_references_require_a_participant_in_every_event_alternative() {
    for trigger in [
        "this creature deals damage",
        "this creature deals damage to a creature",
        "this creature deals damage to a player or planeswalker",
        "this creature deals damage to a player or attacks",
        "a creature or a player is dealt damage",
    ] {
        let d = watcher(&format!("Whenever {trigger}, that player loses 2 life."));
        let err = game::play::check(&d).unwrap_err();
        assert!(err.0.contains("nothing before it"), "{trigger}: {}", err.0);
    }
}

fn sword(war: bool) -> (Game, game::ObjId, game::ObjId, game::ObjId) {
    let (name, text) = if war {
        (
            "Sword of War and Peace",
            "Equipped creature gets +2/+2 and has protection from red and from white.\nWhenever equipped creature deals combat damage to a player, this Equipment deals damage to that player equal to the number of cards in their hand and you gain 1 life for each card in your hand.\nEquip {2}",
        )
    } else {
        (
            "Sword of Feast and Famine",
            "Equipped creature gets +2/+2 and has protection from black and from green.\nWhenever equipped creature deals combat damage to a player, that player discards a card and you untap all lands you control.\nEquip {2}",
        )
    };
    let defs = vec![
        CardDef::new(name, "{3}", "Artifact — Equipment", text, None).unwrap(),
        CardDef::new("Bear", "{0}", "Creature — Bear", "", Some((2, 2))).unwrap(),
        CardDef::new("Forest", "", "Basic Land — Forest", "", None).unwrap(),
    ];
    for d in &defs {
        game::play::check(d).unwrap_or_else(|u| panic!("{}: {}", d.name(), u.0));
    }
    let mut g = Game::new(defs, 3, 20);
    let sword = g.put(0, 0, Zone::Battlefield);
    let bear = g.put(0, 1, Zone::Battlefield);
    let land = g.put(0, 2, Zone::Battlefield);
    assert!(g.attach(sword, Target::Object(bear)));
    for _ in 0..2 {
        g.put(0, 2, Zone::Hand);
    }
    for _ in 0..3 {
        g.put(2, 2, Zone::Hand);
    }
    (g, sword, bear, land)
}
fn hit(g: &mut Game, bear: game::ObjId) {
    pass(g);
    pass(g);
    assert_eq!(g.part, ir::Part::DeclareAttackers);
    g.act(0, Move::Attack(vec![(bear, Target::Player(2))])).unwrap();
    pass(g);
    while let Some(game::state::Decision::Blockers { player }) = g.decision {
        g.act(player, Move::Block(vec![])).unwrap();
    }
    pass(g);
    assert_eq!(g.part, ir::Part::CombatDamage);
    assert_eq!(g.stack.len(), 1);
    assert_eq!(g.players[2].life, 16);
}
#[test]
fn r608_2k_feast_and_famine_binds_the_damaged_player_across_their_discard_choice() {
    use game::state::{Answer, Choice, Decision};
    let (mut g, _, bear, land) = sword(false);
    g.tap(land);
    hit(&mut g, bear);
    pass(&mut g);
    cite!(["608.2k", "701.9b"]; assert!(matches!(g.decision, Some(Decision::Choose { player: 2, choice: Choice::Objects { .. }, .. }))));
    cite!(["608.2c"]; assert!(g.obj(land).tapped, "instructions after the discard still wait"));
    let card = g.players[2].hand[1];
    g.act(2, Move::Choose(Answer::Objects(vec![card]))).unwrap();
    cite!(["608.2k"]; assert_eq!(g.players[2].hand.len(), 2));
    assert_eq!(g.players[0].hand.len(), 2);
    assert_eq!(g.players[1].hand.len(), 0);
    cite!(["608.2c"]; assert!(!g.obj(land).tapped));
}
#[test]
fn r608_2h_war_and_peace_counts_the_bound_players_current_hand_after_source_loss() {
    let (mut g, sword, bear, _) = sword(true);
    hit(&mut g, bear);
    g.destroy(sword);
    g.put(2, 2, Zone::Hand);
    pass(&mut g);
    cite!(["608.2h", "113.7a"]; assert_eq!(g.players.iter().map(|p| p.life).collect::<Vec<_>>(), vec![22, 20, 12]));
    assert_eq!(g.power(bear), Some(2));
}
#[test]
fn r603_2g_swords_do_not_trigger_from_noncombat_damage() {
    for war in [false, true] {
        let (mut g, _, bear, _) = sword(war);
        g.damage(bear, Target::Player(2), 1);
        g.act(0, Move::Pass).unwrap();
        cite!(["603.2"]; case: war; assert!(g.stack.is_empty()));
        assert!(g.decision.is_none());
    }
}

#[test]
fn r508_3a_an_explicitly_named_attack_defender_is_available_to_the_trigger() {
    let d = watcher("Whenever this creature attacks a player, that player loses 1 life.");
    game::play::check(&d).unwrap();
    let mut g = Game::new(vec![d], 3, 20);
    let source = g.put(0, 0, Zone::Battlefield);
    pass(&mut g);
    pass(&mut g);
    g.act(0, Move::Attack(vec![(source, Target::Player(2))])).unwrap();
    cite!(["508.3a"]; assert_eq!(g.stack.len(), 1));
    pass(&mut g);
    cite!(["508.3a", "608.2k"]; assert_eq!(g.players.iter().map(|p| p.life).collect::<Vec<_>>(), vec![20, 20, 19]));
}
#[test]
fn r608_2k_a_player_reference_can_be_shared_by_all_alternative_actions() {
    let d = watcher("Whenever this creature attacks a player or deals damage to a player, that player loses 1 life.");
    game::play::check(&d).unwrap();
    let mut g = Game::new(vec![d], 3, 20);
    let source = g.put(0, 0, Zone::Battlefield);
    g.damage(source, Target::Player(1), 1);
    g.act(0, Move::Pass).unwrap();
    pass(&mut g);
    cite!(["608.2k"]; assert_eq!((g.players[1].life, g.players[2].life), (18, 20)));
}

/// Changing control through the real spell path establishes distinct physical owners.
fn take_control(g: &mut Game, player: usize, object: game::ObjId, definition: usize) {
    for _ in 0..g.players.len() {
        if g.priority == player { break; }
        g.act(g.priority, Move::Pass).unwrap();
    }
    let card = g.put(player, definition, Zone::Hand);
    g.act(player, Move::Cast { card, targets: vec![vec![Target::Object(object)]] }).unwrap();
    pass(g);
}

// Rules: 701.21a, 603.2, 603.3a, 608.2k, 120.3a, 113.7a
#[test]
fn r608_2k_tracker_binds_sacrificing_player_not_owner_or_source_controller_after_departure() {
    for source_leaves in [false, true] {
        let defs = vec![crate::support::oracle("Vengeful Tracker"),
            CardDef::new("Relic", "{0}", "Artifact Creature — Construct", "", Some((1, 1))).unwrap(),
            CardDef::new("Borrow", "{0}", "Instant", "Gain control of target creature until end of turn.", None).unwrap()];
        let mut g = Game::new(defs, 3, 20);
        let tracker = g.put(2, 0, Zone::Battlefield);
        let relic = g.put(0, 1, Zone::Battlefield);
        take_control(&mut g, 0, tracker, 2);
        take_control(&mut g, 1, relic, 2);
        // Uncited (fixture): actual control spells established all three distinct seats.
        assert_eq!((g.obj(tracker).owner, g.controller(tracker), g.obj(relic).owner, g.controller(relic)), (2, 0, 0, 1));
        g.sacrifice(1, relic);
        g.act(g.priority, Move::Pass).unwrap();
        cite!(["603.2", "701.21a"]; assert_eq!(g.stack.len(), 1));
        if source_leaves { g.destroy(tracker); }
        pass(&mut g);
        cite!(["608.2k", "120.3a", "113.7a"]; assert_eq!(g.players.iter().map(|p| p.life).collect::<Vec<_>>(), [20, 18, 20]));
        cite!(["701.21a"]; assert_eq!(g.players[0].graveyard.len(), 2, "Relic and player 0's Borrow spell belong to player 0"));
        cite!(["608.2k", "113.7a"]; assert_eq!(g.log.iter().filter_map(|event| match event {
            game::Event::DamageDealt { source, to, amount, combat: false } => Some((*source, *to, *amount)),
            _ => None,
        }).collect::<Vec<_>>(), [(tracker, Target::Player(1), 2)]));
    }
}

// Rules: 603.2, 603.3, 608.2k, 120.3a, 701.21a
#[test]
fn r608_2k_separate_sacrifices_retain_their_own_multiplayer_actor() {
    let defs = vec![crate::support::oracle("Vengeful Tracker"),
        CardDef::new("Relic", "{0}", "Artifact", "", None).unwrap()];
    let mut g = Game::new(defs, 3, 20);
    let tracker = g.put(0, 0, Zone::Battlefield);
    for actor in [1, 2] {
        let relic = g.put(actor, 1, Zone::Battlefield);
        g.sacrifice(actor, relic);
        g.act(g.priority, Move::Pass).unwrap();
        cite!(["603.2"]; assert_eq!(g.stack.len(), actor));
    }
    // Each occurrence was stacked separately, so the second sacrifice resolves first.
    pass(&mut g);
    cite!(["608.2k", "120.3a"]; assert_eq!(g.players.iter().map(|p| p.life).collect::<Vec<_>>(), [20, 20, 18]));
    pass(&mut g);
    cite!(["608.2k", "120.3a"]; assert_eq!(g.players.iter().map(|p| p.life).collect::<Vec<_>>(), [20, 18, 18]));
    cite!(["603.2", "608.2k"]; assert_eq!(g.log.iter().filter_map(|event| match event {
        game::Event::DamageDealt { source, to, amount, combat: false } => Some((*source, *to, *amount)),
        _ => None,
    }).collect::<Vec<_>>(), [(tracker, Target::Player(2), 2), (tracker, Target::Player(1), 2)]));
}

// Rules: 608.2c, 608.2k, 120.3a, 119.3
#[test]
fn r608_2k_personal_input_preserves_player_kind_for_a_following_clause() {
    // A direct composition isolates Binder propagation beyond the native Tracker text.
    let d = watcher("Whenever an opponent sacrifices an artifact, this creature deals 2 damage to them. They gain 1 life.");
    // Uncited (admission): only this shared player antecedent composition is asserted.
    game::play::check(&d).unwrap();
    let mut g = Game::new(vec![d, CardDef::new("Relic", "{0}", "Artifact", "", None).unwrap()], 2, 20);
    g.put(0, 0, Zone::Battlefield);
    let relic = g.put(1, 1, Zone::Battlefield);
    g.sacrifice(1, relic); g.act(0, Move::Pass).unwrap();
    pass(&mut g);
    cite!(["608.2c", "608.2k", "120.3a", "119.3"]; assert_eq!((g.players[0].life, g.players[1].life), (20, 19)));
}
