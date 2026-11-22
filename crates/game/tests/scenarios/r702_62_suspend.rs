//! Suspend's hand action, exile triggers and free-cast continuation use full Oracle cards.
use crate::support::{cite, mana_pool, oracle_read, paid};
use game::state::{Answer, Choice, Decision};
use game::{CardDef, Event, Game, Move, ObjId, Target, Zone};
use ir::Part;

const LAND: usize = 1;
const REMOVE: usize = 2;
const MOVE: usize = 3;
const STEAL: usize = 4;
const TAX: usize = 5;
const BAN: usize = 6;
const INSTANT: usize = 7;
const FURY: usize = 8;
const GOYF: usize = 9;
const DIFFUSION: usize = 10;

fn game(name: &str) -> Game {
    let card = |name, types, text| CardDef::new(name, "{0}", types, text, None).unwrap();
    let defs = vec![
        oracle_read(name), oracle_read("Mountain"),
        card("Counter remover", "Artifact", "{0}: Remove a time counter from target suspended card."),
        card("Zone mover", "Instant", "Put target card in exile into its owner's graveyard."),
        card("Steal", "Instant", "Gain control of target creature."),
        card("Tax", "Artifact", "Noncreature spells cost {1} more to cast."),
        card("Casting ban", "Enchantment", "You can't cast spells."),
        card("Instant suspend", "Instant", "Suspend 1—{0}\nYou gain 1 life."),
        oracle_read("Fury Charm"), oracle_read("Cosmogoyf"), oracle_read("Venser's Diffusion"),
    ];
    // Uncited (admission): complete pinned focus card and generic composition fixtures.
    for def in &defs { game::play::check(def).unwrap_or_else(|why| panic!("{}: {}", def.name(), why.0)); }
    let mut game = Game::new(defs, 2, 20);
    for player in 0..2 { for _ in 0..12 { game.put(player, LAND, Zone::Library); } }
    game
}
fn passes(game: &mut Game) {
    for _ in 0..2 { game.act(game.priority, Move::Pass).unwrap(); }
}
fn resolve(game: &mut Game) {
    while !game.stack.is_empty() { game.act(game.priority, Move::Pass).unwrap(); }
}
fn next_own_upkeep(game: &mut Game) {
    let turn = game.turn;
    for _ in 0..150 {
        if game.turn != turn && game.active == 0 && game.part == Part::Upkeep { return; }
        match game.decision.clone() {
            Some(Decision::Attackers { player }) => game.act(player, Move::Attack(vec![])).unwrap(),
            Some(other) => panic!("unexpected decision before upkeep: {other:?}"),
            None => game.act(game.priority, Move::Pass).unwrap(),
        }
    }
    panic!("did not reach next own upkeep");
}
fn suspended(game: &mut Game, count: u32) -> ObjId {
    let card = game.put(0, 0, Zone::Exile);
    game.put_counters(card, "time", count);
    card
}
fn upkeep_offer(game: &mut Game, card: ObjId) {
    next_own_upkeep(game);
    cite!(["702.62a", "603.3"]; assert_eq!(game.stack.len(), 1));
    passes(game);
    cite!(["702.62a", "603.3"]; assert_eq!(game.stack.len(), 1));
    cite!(["702.62b"]; assert!(!game.suspended(card)));
    passes(game);
    cite!(["702.62a", "608.2g"]; assert!(matches!(&game.decision,
        Some(Decision::Choose { player: 0, choice: Choice::Cast { cards } }) if cards == &[card])));
}

#[test]
fn r702_62a_rift_bolt_hand_action_pays_once_without_casting_and_retains_priority() {
    let mut game = game("Rift Bolt");
    let card = game.put(0, 0, Zone::Hand);
    game.players[0].pool = mana_pool("R");
    cite!(["702.62a", "116.2f"]; ok: game.act(0, paid("R", Move::Suspend(card))));
    let exiled = game.exile[0];
    cite!(["400.7"]; assert_ne!(exiled, card));
    cite!(["702.62a"]; assert_eq!(game.counters(exiled).get("time"), Some(&1)));
    cite!(["702.62b"]; assert!(game.suspended(exiled)));
    cite!(["702.62a"]; assert_eq!(game.players[0].pool.total(), 0));
    cite!(["116.3"]; assert_eq!(game.priority, 0));
    cite!(["702.62a"]; assert!(game.stack.is_empty() && !game.log.iter().any(|event| matches!(event, Event::Cast { .. }))));
    cite!(["702.62a"]; refused("suspend a card from your hand"): game.act(0, Move::Suspend(exiled)));
}

#[test]
fn r702_62c_special_action_checks_begin_cast_timing_and_prohibitions_without_mutating() {
    let mut game = game("Rift Bolt");
    let card = game.put(0, 0, Zone::Hand);
    game.players[0].pool = mana_pool("R");
    let instant = game.put(0, INSTANT, Zone::Hand);
    game.act(0, Move::Cast { card: instant, targets: vec![] }).unwrap();
    cite!(["116.2f", "702.62c"]; refused("suspend only when you could begin to cast that card"): game.act(0, paid("R", Move::Suspend(card))));
    let instant = game.put(0, INSTANT, Zone::Hand);
    cite!(["116.2f"]; ok: game.act(0, Move::Suspend(instant)));
    cite!(["702.62a"]; assert_eq!(game.stack.len(), 1));
    resolve(&mut game);
    game.put(0, BAN, Zone::Battlefield);
    cite!(["702.62c", "101.2"]; refused("an effect prohibits casting that spell"): game.act(0, paid("R", Move::Suspend(card))));
    cite!(["702.62c"]; assert!(game.players[0].hand.contains(&card)));
    cite!(["118.3a"]; assert_eq!(game.players[0].pool.total(), 1));
}

#[test]
fn r702_62a_lotus_bloom_with_no_mana_cost_can_suspend_for_zero_despite_spell_tax() {
    let mut game = game("Lotus Bloom");
    game.put(0, TAX, Zone::Battlefield);
    let card = game.put(0, 0, Zone::Hand);
    cite!(["202.1b", "118.6"]; refused("no mana cost"): game.act(0, Move::Cast { card, targets: vec![] }));
    cite!(["702.62a", "118.9d"]; ok: game.act(0, Move::Suspend(card)));
    let card = game.exile[0];
    cite!(["702.62a"]; assert_eq!(game.counters(card).get("time"), Some(&3)));
    cite!(["702.62a"]; assert!(game.stack.is_empty()));
}

#[test]
fn r702_62a_upkeep_last_counter_offers_a_targeted_free_cast_and_declining_expires_it() {
    let mut game = game("Rift Bolt");
    let card = suspended(&mut game, 1);
    upkeep_offer(&mut game, card);
    let mut declined = game.clone();
    cite!(["702.62a", "608.2g"]; ok: game.act(0, Move::Cast { card, targets: vec![vec![Target::Player(1)]] }));
    cite!(["702.62d"]; assert_eq!(game.players[0].pool.total(), 0));
    resolve(&mut game);
    // Uncited (card text): full Oracle Rift Bolt deals exactly 3 damage to the chosen player.
    assert_eq!(game.players[1].life, 17);
    cite!(["608.2m"]; assert_eq!(game.players[0].graveyard.len(), 1));
    cite!(["702.62a"]; ok: declined.act(0, Move::Choose(Answer::No)));
    cite!(["702.62a"]; assert_eq!(declined.obj(card).zone, Zone::Exile));
    next_own_upkeep(&mut declined);
    cite!(["702.62a", "702.62b"]; assert!(declined.stack.is_empty() && !declined.suspended(card)));
}

#[test]
fn r702_62d_free_cast_still_pays_spell_cost_increases_and_keeps_the_offer_on_refusal() {
    let mut game = game("Rift Bolt");
    game.put(0, TAX, Zone::Battlefield);
    let card = suspended(&mut game, 1);
    upkeep_offer(&mut game, card);
    cite!(["702.62d", "601.2f"]; refused("your mana pool cannot pay that cost"): game.act(0, Move::Cast { card, targets: vec![vec![Target::Player(1)]] }));
    game.players[0].pool = mana_pool("R");
    cite!(["702.62d", "601.2h"]; ok: game.act(0, paid("R", Move::Cast { card, targets: vec![vec![Target::Player(1)]] })));
    resolve(&mut game);
    cite!(["601.2h"]; assert_eq!(game.players[0].pool.total(), 0));
    // Uncited (card text): full Oracle Rift Bolt still deals its 3 damage.
    assert_eq!(game.players[1].life, 17);
}

#[test]
fn r702_62a_suspended_counter_targets_exclude_counterless_and_wrong_keyword_exile_cards() {
    let mut game = game("Rift Bolt");
    let card = suspended(&mut game, 1);
    let no_counter = game.put(0, 0, Zone::Exile);
    let no_suspend = game.put(0, LAND, Zone::Exile);
    game.put_counters(no_suspend, "time", 1);
    let remover = game.put(0, REMOVE, Zone::Battlefield);
    for target in [no_counter, no_suspend] {
        // Uncited (Oracle designation): only an actually suspended card meets this target.
        crate::support::refused(game.act(0, Move::Activate { source: remover, ability: 0,
            targets: vec![vec![target.into()]], paying: vec![] }), "target");
    }
    cite!(["702.62b", "602.2b"]; ok: game.act(0, Move::Activate { source: remover, ability: 0,
        targets: vec![vec![card.into()]], paying: vec![] }));
    passes(&mut game);
    cite!(["702.62a"]; assert_eq!(game.stack.len(), 1));
    passes(&mut game);
    cite!(["702.62a"]; assert!(matches!(game.decision, Some(Decision::Choose { choice: Choice::Cast { .. }, .. }))));
}

#[test]
fn r702_62a_last_counter_trigger_does_not_follow_a_departed_exile_incarnation() {
    let mut game = game("Rift Bolt");
    let card = suspended(&mut game, 1);
    next_own_upkeep(&mut game);
    passes(&mut game);
    let mover = game.put(0, MOVE, Zone::Hand);
    game.act(0, Move::Cast { card: mover, targets: vec![vec![card.into()]] }).unwrap();
    resolve(&mut game);
    cite!(["400.7", "603.4"]; assert!(game.decision.is_none() && game.exile.is_empty()));
    cite!(["400.7"]; assert_eq!(game.obj(card).zone, Zone::Gone));
}

#[test]
fn r702_62a_creature_cast_gets_haste_through_resolution_until_first_loss_of_control() {
    let mut game = game("Durkwood Baloth");
    let card = suspended(&mut game, 1);
    upkeep_offer(&mut game, card);
    game.act(0, Move::Cast { card, targets: vec![] }).unwrap();
    let spell = game.stack[0];
    cite!(["702.62a"]; assert!(game.has(spell, "Haste")));
    resolve(&mut game);
    let permanent = *game.battlefield.iter().find(|object| game.obj(**object).card == Some(0)).unwrap();
    cite!(["112.4", "400.7a", "702.62a"]; assert!(game.has(permanent, "Haste")));
    let steal = game.put(1, STEAL, Zone::Hand);
    if game.priority != 1 { game.act(game.priority, Move::Pass).unwrap(); }
    game.act(1, Move::Cast { card: steal, targets: vec![vec![permanent.into()]] }).unwrap();
    resolve(&mut game);
    cite!(["702.62a", "611.2b"]; assert!(!game.has(permanent, "Haste")));
    let regain = game.put(0, STEAL, Zone::Hand);
    if game.priority != 0 { game.act(game.priority, Move::Pass).unwrap(); }
    game.act(0, Move::Cast { card: regain, targets: vec![vec![permanent.into()]] }).unwrap();
    resolve(&mut game);
    cite!(["611.2b"]; assert_eq!(game.controller(permanent), 0));
    cite!(["611.2b", "702.62a"]; assert!(!game.has(permanent, "Haste")));
}

#[test]
fn r702_62a_greater_gargadon_activation_functions_only_in_exile_and_uses_real_sacrifice() {
    let mut game = game("Greater Gargadon");
    let card = suspended(&mut game, 1);
    let land = game.put(0, LAND, Zone::Battlefield);
    cite!(["113.6b", "702.62b", "602.2b"]; ok: game.act(0, Move::Activate {
        source: card, ability: 1, targets: vec![], paying: vec![vec![land]],
    }));
    cite!(["701.17a"]; assert_eq!(game.obj(land).zone, Zone::Gone));
    passes(&mut game);
    passes(&mut game);
    cite!(["702.62a"]; assert!(matches!(&game.decision, Some(Decision::Choose { choice: Choice::Cast { cards }, .. }) if cards == &[card])));
    game.act(0, Move::Cast { card, targets: vec![] }).unwrap();
    resolve(&mut game);
    let permanent = *game.battlefield.iter().find(|object| game.obj(**object).card == Some(0)).unwrap();
    let other = game.put(0, LAND, Zone::Battlefield);
    cite!(["113.6b"]; refused("that ability functions only"): game.act(0, Move::Activate {
        source: permanent, ability: 1, targets: vec![], paying: vec![vec![other]],
    }));
    cite!(["702.62a"]; assert!(game.has(permanent, "Haste")));
}

#[test]
fn fixed_suspend_oracle_family_compiles_while_variable_suspend_stays_refused() {
    // Uncited (admission): whole real cards, not isolated keyword rows.
    for name in ["Rift Bolt", "Ancestral Vision", "Lotus Bloom", "Sol Talisman", "Durkwood Baloth",
        "Errant Ephemeron", "Greater Gargadon", "Search for Tomorrow"] {
        let def = oracle_read(name);
        game::play::check(&def).unwrap_or_else(|why| panic!("{name}: {}", why.0));
    }
    // Uncited (admission): X-bound time and costs need their own explicit hand announcements.
    crate::support::refused(game::play::check(&oracle_read("Benalish Commander")).map_err(|why| why.0), "ability on a permanent: KeywordsWith");
}


#[test]
fn r702_62a_lotus_bloom_counts_three_own_upkeeps_then_casts_and_uses_its_real_mana_ability() {
    let mut game = game("Lotus Bloom");
    let card = game.put(0, 0, Zone::Hand);
    game.act(0, Move::Suspend(card)).unwrap();
    let card = game.exile[0];
    for remaining in [2, 1, 0] {
        next_own_upkeep(&mut game);
        passes(&mut game);
        cite!(["702.62a"]; assert_eq!(game.counters(card).get("time").copied().unwrap_or(0), remaining));
        cite!(["702.62a", "702.62b"]; assert_eq!(game.stack.len(), usize::from(remaining == 0)));
    }
    passes(&mut game);
    game.act(0, Move::Cast { card, targets: vec![] }).unwrap();
    resolve(&mut game);
    let permanent = *game.battlefield.iter().find(|object| game.obj(**object).card == Some(0)).unwrap();
    cite!(["702.62a"]; assert!(!game.has(permanent, "Haste")));
    game.act(0, Move::Adding(vec![vec![ir::mana::ManaSymbol::Color(ir::Color::Red); 3]], Box::new(Move::Activate {
        source: permanent, ability: 1, targets: vec![], paying: vec![],
    }))).unwrap();
    // Uncited (card text): full Oracle Lotus Bloom sacrifices itself for three mana of the explicit color.
    assert_eq!(game.players[0].pool.colors[3], 3);
    cite!(["605.3b", "701.17a"]; assert!(game.stack.is_empty() && game.obj(permanent).zone == Zone::Gone));
}

#[test]
fn r702_62a_fury_charm_removes_the_last_counter_from_a_real_suspended_card() {
    let mut game = game("Rift Bolt");
    let card = suspended(&mut game, 1);
    let charm = game.put(0, FURY, Zone::Hand);
    game.players[0].pool = mana_pool("RR");
    cite!(["601.2b", "702.62b"]; ok: game.act(0, paid("RR", Move::Modes(vec![2], Box::new(Move::Cast {
        card: charm, targets: vec![vec![card.into()]],
    })))));
    passes(&mut game);
    cite!(["122.1", "702.62a"]; assert!(game.counters(card).is_empty() && game.stack.len() == 1));
    passes(&mut game);
    cite!(["702.62a"]; assert!(matches!(&game.decision, Some(Decision::Choose { choice: Choice::Cast { cards }, .. }) if cards == &[card])));
}

#[test]
fn r122_2_vensers_diffusion_returns_the_suspended_card_without_removing_its_counters() {
    let mut game = game("Rift Bolt");
    let card = suspended(&mut game, 1);
    let diffusion = game.put(0, DIFFUSION, Zone::Hand);
    game.players[0].pool = mana_pool("UUU");
    game.act(0, paid("UUU", Move::Cast { card: diffusion, targets: vec![vec![card.into()]] })).unwrap();
    resolve(&mut game);
    cite!(["122.2", "400.7"]; assert_eq!(game.obj(card).zone, Zone::Gone));
    cite!(["122.2"]; assert!(!game.log.iter().any(|event| matches!(event, Event::CountersRemoved { from, .. } if *from == card.into()))));
    cite!(["702.62a"]; assert!(game.stack.is_empty() && game.decision.is_none()));
    cite!(["400.7"]; assert_eq!(game.players[0].hand.len(), 1));
}

#[test]
fn r604_3_cosmogoyf_counts_shared_exile_once_and_preserves_its_owner_filter_after_movement() {
    let mut game = game("Rift Bolt");
    let goyf = game.put(0, GOYF, Zone::Battlefield);
    let first = game.put(0, LAND, Zone::Exile);
    game.put(0, 0, Zone::Exile);
    game.put(1, LAND, Zone::Exile);
    // Uncited (card text): Cosmogoyf counts the two cards its controller owns, not the opponent's.
    cite!(["604.3"]; assert_eq!(game.seen(goyf, None).pt, Some((2, 3))));
    let mover = game.put(0, MOVE, Zone::Hand);
    game.act(0, Move::Cast { card: mover, targets: vec![vec![first.into()]] }).unwrap();
    resolve(&mut game);
    cite!(["604.3", "611.3a"]; assert_eq!(game.seen(goyf, None).pt, Some((1, 2))));
}


#[test]
fn r118_6a_ancestral_vision_replaces_its_absent_mana_cost_and_draws_three_for_the_target() {
    let mut game = game("Ancestral Vision");
    let card = suspended(&mut game, 1);
    upkeep_offer(&mut game, card);
    let before = game.players[1].hand.len();
    cite!(["118.6a", "702.62d"]; ok: game.act(0, Move::Cast { card, targets: vec![vec![Target::Player(1)]] }));
    resolve(&mut game);
    // Uncited (card text): Ancestral Vision makes its chosen player draw three cards.
    assert_eq!(game.players[1].hand.len(), before + 3);
}


#[test]
fn r702_62c_a_legendary_spell_cannot_suspend_without_its_required_legendary_permanent() {
    let spell = CardDef::new("Legendary suspender", "{2}", "Legendary Instant", "Suspend 1—{0}\nYou gain 1 life.", None).unwrap();
    let mut game = Game::new(vec![spell, oracle_read("Ith, High Arcanist")], 2, 20);
    let card = game.put(0, 0, Zone::Hand);
    cite!(["205.4e", "702.62c"]; refused("a legendary instant or sorcery is cast only"): game.act(0, Move::Suspend(card)));
    game.put(0, 1, Zone::Battlefield);
    cite!(["205.4e", "702.62c"]; ok: game.act(0, Move::Suspend(card)));
}
