//! Persistent Commander facts use physical cards, not definition indices or incarnation handles.
use crate::support::{cite, paid};
use game::{CardDef, Game, Move, ObjId, Target, Zone};

fn setup() -> (Game, ObjId) {
    let defs = vec![
        CardDef::new("Leader", "{2}", "Legendary Creature — Human", "", Some((2, 2))).unwrap(),
        CardDef::new("Discount", "{0}", "Enchantment", "Creature spells cost {1} less to cast.", None).unwrap(),
        CardDef::new("Copy", "{0}", "Instant", "Create a token that's a copy of target creature, except it isn't legendary.", None).unwrap(),
        CardDef::new("Island", "", "Basic Land — Island", "", None).unwrap(),
    ];
    for def in &defs { game::play::check(def).unwrap(); }
    let mut g = Game::new(defs, 3, 40);
    for p in 0..3 { for _ in 0..15 { g.put(p, 3, Zone::Library); } }
    let leader = g.put(0, 0, Zone::Command);
    g.designate_commander(leader).unwrap();
    (g, leader)
}
fn move_to(g: &mut Game, card: ObjId, zone: Zone) -> ObjId {
    let owner = g.obj(card).owner;
    g.move_object(card, zone);
    *match zone {
        Zone::Hand => &g.players[owner].hand,
        Zone::Library => &g.players[owner].library,
        Zone::Graveyard => &g.players[owner].graveyard,
        Zone::Battlefield => &g.battlefield,
        Zone::Command => &g.command,
        Zone::Exile => &g.exile,
        Zone::Stack => &g.stack,
        Zone::Gone => unreachable!(),
    }.last().unwrap()
}
fn cast(g: &mut Game, card: ObjId, units: &str) { g.act(0, paid(units, Move::Cast { card, targets: vec![] })).unwrap(); }
fn resolve(g: &mut Game) { while !g.stack.is_empty() { g.act(g.priority, Move::Pass).unwrap(); } }


#[test]
fn r903_3_designation_survives_zone_changes_but_not_copies_or_another_card_of_the_same_name() {
    let (mut g, original) = setup();
    let other = g.put(1, 0, Zone::Battlefield);
    cite!(["903.3"]; assert!(!g.is_commander(other)));
    let mut leader = original;
    for zone in [Zone::Hand, Zone::Library, Zone::Graveyard, Zone::Exile, Zone::Command, Zone::Battlefield] {
        leader = move_to(&mut g, leader, zone);
        cite!(["903.3"]; case: format!("{zone:?}"); assert!(g.is_commander(leader)));
        assert_ne!(leader, original);
    }
    let copy = g.put(0, 2, Zone::Hand);
    g.act(0, Move::Cast { card: copy, targets: vec![vec![Target::Object(leader)]] }).unwrap();
    resolve(&mut g);
    let token = *g.battlefield.last().unwrap();
    assert!(g.obj(token).token());
    cite!(["903.3"]; assert!(!g.is_commander(token)));
    g.retire_events(g.log.end());
    g.collect_departed();
    assert!(g.get_object(original).is_none());
    assert!(g.is_commander(leader));
    assert!(g.clone().is_commander(leader));
}

#[test]
fn r903_8_command_zone_casts_use_normal_timing_owner_and_shared_cost_queries() {
    let (mut g, leader) = setup();
    g.players[0].pool.colorless = 1;
    assert!(!g.clone().act(0, paid("C", Move::Cast { card: leader, targets: vec![] })).is_ok());
    cite!(["118.3a"]; refused("the supplied mana does not pay that cost, all of it and no more (601.2h, 118.3a)"): g.act(0, paid("C", Move::Cast { card: leader, targets: vec![] })));
    assert_eq!(g.commander_casts(leader, 0), 0);
    assert_eq!(g.obj(leader).zone, Zone::Command);
    g.players[0].pool.colorless = 2;
    assert!(g.clone().act(0, paid("CC", Move::Cast { card: leader, targets: vec![] })).is_ok());
    cast(&mut g, leader, "CC");
    let spell = *g.stack.last().unwrap();
    cite!(["903.8"]; assert_eq!(g.commander_casts(spell, 0), 1));
    assert_eq!(g.players[0].pool.total(), 0);
    assert!(g.counter(spell)); // counted even though it never resolves
    let grave = g.players[0].graveyard[0];
    let leader = move_to(&mut g, grave, Zone::Command);
    g.players[0].pool.colorless = 3;
    cite!(["903.8", "601.2h"]; refused("the supplied mana does not pay that cost, all of it and no more (601.2h, 118.3a)"): g.act(0, paid("CCC", Move::Cast { card: leader, targets: vec![] })));
    assert_eq!(g.players[0].pool.colorless, 3);
    assert_eq!(g.commander_casts(leader, 0), 1);
    g.put(0, 1, Zone::Battlefield); // {2} + {2} tax - {1} reduction
    cite!(["903.8", "601.2f"]; assert!(g.clone().act(0, paid("CCC", Move::Cast { card: leader, targets: vec![] })).is_ok()));
    cast(&mut g, leader, "CCC");
    let spell = *g.stack.last().unwrap();
    assert_eq!(g.commander_casts(spell, 0), 2);
    assert_eq!(g.players[0].pool.total(), 0);
    resolve(&mut g);
    let leader = *g.battlefield.last().unwrap();
    let hand = move_to(&mut g, leader, Zone::Hand);
    g.players[0].pool.colorless = 1;
    cast(&mut g, hand, "C"); // no command-zone tax from the hand
    cite!(["903.8"]; assert_eq!(g.commander_casts(*g.stack.last().unwrap(), 0), 2));
}

#[test]
fn r903_8_opponents_and_undesignated_command_zone_cards_have_no_cast_permission() {
    let (mut g, leader) = setup();
    let ordinary = g.put(0, 0, Zone::Command);
    g.players[0].pool.colorless = 10;
    cite!(["903.8"]; assert!(!g.clone().act(0, paid("CC", Move::Cast { card: ordinary, targets: vec![] })).is_ok()));
    g.act(0, Move::Pass).unwrap();
    g.players[1].pool.colorless = 10;
    cite!(["903.8", "601.3"]; refused("that card is not in your hand: it is in the command zone, and no rule or effect lets you cast it from there (601.3)"): g.act(1, paid("CC", Move::Cast { card: leader, targets: vec![] })));
    assert_eq!(g.commander_casts(leader, 1), 0);
}

#[test]
fn r903_9a_graveyard_and_exile_offers_are_owned_resumable_and_only_once_per_arrival() {
    use game::simulation::Simulation;
    use game::state::{Answer, Choice, Decision};
    for zone in [Zone::Graveyard, Zone::Exile] {
        let (mut g, leader) = setup();
        let arrived = move_to(&mut g, leader, zone);
        assert!(g.state_based_actions());
        cite!(["903.9a"]; case: format!("{zone:?}"); assert!(matches!(g.decision, Some(Decision::Choose { player: 0, choice: Choice::CommandZone { object } }) if object == arrived)));
        cite!(["903.9a"]; case: format!("{zone:?}"); assert_eq!(g.obj(arrived).zone, zone, "the card actually entered that zone before the SBA choice"));
        let mut sim = Simulation::new(g);
        let request = sim.request().unwrap().unwrap().clone();
        use game::simulation::ResponseError;
        // the owner's choice is pending: no one passes, and only the owner answers
        cite!(["903.9a"]; refused("a decision is pending: whether the commander goes to the command zone (903.9a)"): sim.respond(request.id, 0, Move::Pass));
        for player in 1..3 { assert_eq!(sim.respond(request.id, player, Move::Pass), Err(ResponseError::WrongPlayer { expected: 0, received: player })); }
        assert_eq!(sim.respond(request.id, 1, Move::Choose(Answer::Yes)), Err(ResponseError::WrongPlayer { expected: 0, received: 1 }));
        let mut decline = sim.clone();
        sim.respond(request.id, 0, Move::Choose(Answer::Yes)).unwrap();
        assert_eq!(sim.game().unwrap().command.len(), 1);
        assert!(sim.game().unwrap().is_commander(sim.game().unwrap().command[0]));
        assert!(matches!(sim.respond(request.id, 0, Move::Choose(Answer::Yes)), Err(ResponseError::Stale { received, .. }) if received == request.id));
        decline.respond(request.id, 0, Move::Choose(Answer::No)).unwrap();
        cite!(["903.9a"]; case: format!("{zone:?}"); assert_eq!(decline.game().unwrap().obj(arrived).zone, zone));
        assert!(!matches!(decline.game().unwrap().decision, Some(Decision::Choose { choice: Choice::CommandZone { .. }, .. })));
        // Raw fixture access creates a later arrival, which gets its own one-time offer.
        let mut g = decline.game().unwrap().clone();
        assert!(!g.state_based_actions());
        let battlefield = move_to(&mut g, arrived, Zone::Battlefield);
        let again = move_to(&mut g, battlefield, zone);
        assert!(g.state_based_actions());
        cite!(["903.9a", "400.7"]; case: format!("{zone:?}"); assert!(matches!(g.decision, Some(Decision::Choose { choice: Choice::CommandZone { object }, .. }) if object == again)));
    }
}

#[test]
fn r101_4_and_903_9a_all_return_choices_precede_simultaneous_moves_in_apnap_order() {
    use game::state::{Answer, Choice, Decision};
    let (mut g, leader) = setup();
    let second = g.put(1, 0, Zone::Command);
    g.designate_commander(second).unwrap();
    let first = move_to(&mut g, leader, Zone::Graveyard);
    let second = move_to(&mut g, second, Zone::Exile);
    g.active = 1;
    g.priority = 1;
    g.state_based_actions();
    cite!(["101.4"]; assert!(matches!(g.decision, Some(Decision::Choose { player: 1, choice: Choice::CommandZone { .. } }))));
    g.act(1, Move::Choose(Answer::Yes)).unwrap();
    cite!(["101.4"]; assert_eq!(g.obj(second).zone, Zone::Exile, "collect choices before any return"));
    cite!(["101.4"]; assert!(matches!(g.decision, Some(Decision::Choose { player: 0, choice: Choice::CommandZone { .. } }))));
    g.act(0, Move::Choose(Answer::Yes)).unwrap();
    assert_eq!(g.obj(first).zone, Zone::Gone);
    assert_eq!(g.obj(second).zone, Zone::Gone);
    cite!(["903.9a"]; assert_eq!(g.command.len(), 2));
}

#[test]
fn r101_4_commander_choices_and_legend_choices_share_apnap_order() {
    use game::state::{Answer, Choice, Decision};
    let (mut g, leader) = setup();
    let legend = g.put(0, 0, Zone::Battlefield);
    g.put(0, 0, Zone::Battlefield);
    let commander = g.put(1, 0, Zone::Command);
    g.designate_commander(commander).unwrap();
    move_to(&mut g, commander, Zone::Graveyard);
    g.state_based_actions();
    cite!(["101.4"]; assert!(matches!(g.decision, Some(Decision::Choose { player: 0, choice: Choice::Objects { .. } }))));
    g.act(0, Move::Choose(Answer::Objects(vec![legend]))).unwrap();
    cite!(["101.4"]; assert!(matches!(g.decision, Some(Decision::Choose { player: 1, choice: Choice::CommandZone { .. } }))));
    g.act(1, Move::Choose(Answer::No)).unwrap();
    cite!(["704.5j"]; assert_eq!(g.battlefield, vec![legend]));
    assert_eq!(g.command, vec![leader]);
}

#[test]
fn r733_1_failed_command_zone_payment_restores_the_original_card_without_incrementing_tax() {
    use game::state::{Answer, Choice, Decision};
    use game::legal::Step;
    use ir::mana::ManaSymbol;
    let definitions = vec![
        CardDef::new("Leader", "{4}", "Legendary Creature — Human", "", Some((2, 2))).unwrap(),
        CardDef::new("Mana creature", "{0}", "Creature — Elf", "{T}: You gain 2 life. Put a +1/+1 counter on this creature. Add X mana of any one color, where X is the number of +1/+1 counters on this creature.", Some((1, 1))).unwrap(),
        CardDef::new("Scales", "{G}", "Enchantment", "If one or more +1/+1 counters would be put on a creature you control, that many plus one +1/+1 counters are put on it instead.", None).unwrap(),
        CardDef::new("Evolution", "{2}{G}", "Enchantment", "If one or more +1/+1 counters would be put on a creature you control, twice that many +1/+1 counters are put on that creature instead.", None).unwrap(),
    ];
    let mut g = Game::new(definitions, 2, 40);
    let leader = g.put(0, 0, Zone::Command);
    g.designate_commander(leader).unwrap();
    let mana = g.put(0, 1, Zone::Battlefield);
    g.put(0, 2, Zone::Battlefield);
    let evolution = g.put(0, 3, Zone::Battlefield);
    let step = Step { plan: vec![], x: None, life: vec![], with: None, cost_branches: None, chose: vec![],  source: mana, ability: 0, paying: vec![], adding: vec![vec![ManaSymbol::Color(ir::Color::Green); 3]] };
    g.act(0, Move::WithMana(vec![step], Box::new(Move::Cast { card: leader, targets: vec![] }))).unwrap();
    let spell = g.stack[0];
    assert_eq!(g.commander_casts(spell, 0), 0, "announcement is not a completed cast");
    let Some(Decision::Choose { choice: Choice::Replacement { among, .. }, .. }) = &g.decision else { panic!("{:?}", g.decision) };
    let replacement = among.iter().find(|r| r.source == evolution).unwrap().clone();
    g.act(0, Move::Choose(Answer::Replacement(replacement))).unwrap();
    assert!(matches!(g.decision, Some(Decision::Payment { .. })));
    let mut clone = g.clone();
    for game in [&mut g, &mut clone] {
        game.act(0, Move::ReversePayment).unwrap();
        cite!(["733.1"]; assert_eq!(game.command, vec![leader]));
        assert!(game.is_commander(leader));
        cite!(["733.1", "903.8"]; assert_eq!(game.commander_casts(leader, 0), 0));
        assert_eq!(game.players[0].life, 42);
        assert_eq!(game.players[0].pool.total(), 3);
        game.add_mana(0, ManaSymbol::Colorless);
        cast(game, leader, "CGGG");
        cite!(["903.8"]; assert_eq!(game.commander_casts(game.stack[0], 0), 1));
        assert_ne!(game.stack[0], spell);
    }
}
