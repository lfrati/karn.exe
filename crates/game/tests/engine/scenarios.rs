//! Games played through the engine with real card text: the text is read by the grammar and
//! run from its parsed form. Each test sets a board up, makes the moves a player would, and
//! checks the state and the events.

use crate::support::cite;
use game::card::CardDef;
use game::play::check;
use game::{Event, Game, Move, ObjId, Target, Zone};
use ir::Verb;

const MOUNTAIN: usize = 0;
const FOREST: usize = 1;
const BEARS: usize = 2;
const SHOCK: usize = 3;
const BOLT: usize = 4;
const MURDER: usize = 5;
const DIVINATION: usize = 6;
const MERCY: usize = 7;
const UNSUMMON: usize = 8;
const ALARM: usize = 9;
const JUDGMENT: usize = 10;
const SCOUR: usize = 11;
const GROWTH: usize = 12;
const GIANT: usize = 13;
const PLAINS: usize = 14;
const ISLAND: usize = 15;
const SWAMP: usize = 16;
const HAUNTING: usize = 17;

fn defs() -> Vec<CardDef> {
    let card = |name, cost, types, text, pt| CardDef::new(name, cost, types, text, pt).expect(name);
    vec![
        card("Mountain", "", "Basic Land — Mountain", "({T}: Add {R}.)", None),
        card("Forest", "", "Basic Land — Forest", "({T}: Add {G}.)", None),
        card("Grizzly Bears", "{1}{G}", "Creature — Bear", "", Some((2, 2))),
        card("Shock", "{R}", "Instant", "Shock deals 2 damage to any target.", None),
        card("Lightning Bolt", "{R}", "Instant", "Lightning Bolt deals 3 damage to any target.", None),
        card("Murder", "{1}{B}{B}", "Instant", "Destroy target creature.", None),
        card("Divination", "{2}{U}", "Sorcery", "Draw two cards.", None),
        card("Angel's Mercy", "{2}{W}{W}", "Instant", "You gain 7 life.", None),
        card("Unsummon", "{U}", "Instant", "Return target creature to its owner's hand.", None),
        card("Raise the Alarm", "{1}{W}", "Instant", "Create two 1/1 white Soldier creature tokens.", None),
        card("Day of Judgment", "{2}{W}{W}", "Sorcery", "Destroy all creatures.", None),
        card("Tome Scour", "{U}", "Sorcery", "Target player mills five cards.", None),
        card("Battlegrowth", "{G}", "Instant", "Put a +1/+1 counter on target creature.", None),
        card("Hill Giant", "{3}{R}", "Creature — Giant", "", Some((3, 3))),
        card("Plains", "", "Basic Land — Plains", "({T}: Add {W}.)", None),
        card("Island", "", "Basic Land — Island", "({T}: Add {U}.)", None),
        card("Swamp", "", "Basic Land — Swamp", "({T}: Add {B}.)", None),
        card("Midnight Haunting", "{2}{W}", "Instant", "Create two 1/1 white Spirit creature tokens with flying.", None),
    ]
}

/// A two-player game at 20 life; player 0 is active and has priority.
fn game() -> Game {
    Game::new(defs(), 2, 20)
}

/// Put lands on the battlefield for a player and tap them all for mana.
fn mana(g: &mut Game, player: usize, lands: &[usize]) {
    for land in lands {
        let l = g.put(player, *land, Zone::Battlefield);
        g.act(player, Move::Activate { source: l, ability: 0, targets: vec![], paying: vec![] }).unwrap();
    }
}

/// Cast a card from hand and have both players pass so that it resolves.
fn cast_and_resolve(g: &mut Game, player: usize, card: ObjId, targets: Vec<Vec<Target>>, spending: &str) {
    g.act(player, crate::support::paid(spending, Move::Cast { card, targets })).unwrap();
    g.act(player, Move::Pass).unwrap();
    g.act(1 - player, Move::Pass).unwrap();
}

/// The player with priority passes until the next turn begins or the game ends: each step and
/// phase ends when all players pass in succession with the stack empty (500.2).
fn pass_to_next_turn(g: &mut Game) {
    let turn = g.turn;
    while g.turn == turn && g.winner().is_none() {
        g.act(g.priority, Move::Pass).unwrap();
    }
}

fn verbs(g: &Game) -> Vec<Verb> {
    g.log.iter().filter_map(Event::verb).collect()
}

#[test]
fn every_card_of_the_tests_is_one_the_engine_runs() {
    for d in defs() {
        check(&d).unwrap_or_else(|u| panic!("{}: {}", d.name(), u.0));
    }
}

#[test]
fn r120_3a_damage_dealt_to_a_player_causes_that_much_life_loss() {
    let mut g = game();
    mana(&mut g, 0, &[MOUNTAIN]);
    let shock = g.put(0, SHOCK, Zone::Hand);
    cast_and_resolve(&mut g, 0, shock, vec![vec![Target::Player(1)]], "R");
    cite!(["120.3a"]; assert_eq!(g.players[1].life, 18));
    assert!(g.stack.is_empty());
    cite!(["608.2n"]; assert_eq!(g.players[0].graveyard.len(), 1, "the spell is in its owner's graveyard (608.2n)"));
    assert!(verbs(&g).contains(&Verb::DealDamage));
}

#[test]
fn r704_5g_a_creature_dealt_lethal_damage_is_destroyed() {
    let mut g = game();
    let bears = g.put(1, BEARS, Zone::Battlefield);
    mana(&mut g, 0, &[MOUNTAIN]);
    let bolt = g.put(0, BOLT, Zone::Hand);
    cast_and_resolve(&mut g, 0, bolt, vec![vec![Target::Object(bears)]], "R");
    assert!(g.battlefield.iter().all(|o| g.obj(*o).chars.name != "Grizzly Bears"));
    assert_eq!(g.players[1].graveyard.len(), 1);
    cite!(["400.7"]; assert_eq!(g.obj(bears).zone, Zone::Gone, "what left is a memory; a new object is in the graveyard (400.7)"));
    cite!(["704.5g"]; assert!(g.log.contains(&Event::Destroyed { object: bears }), "destroyed by the state-based action (704.5g)"));
}

#[test]
fn r120_6_damage_stays_marked_on_a_creature_until_the_cleanup_step() {
    let mut g = game();
    let giant = g.put(1, GIANT, Zone::Battlefield);
    mana(&mut g, 0, &[MOUNTAIN]);
    let shock = g.put(0, SHOCK, Zone::Hand);
    cast_and_resolve(&mut g, 0, shock, vec![vec![Target::Object(giant)]], "R");
    cite!(["120.3e"]; assert_eq!(g.obj(giant).damage, 2));
    assert_eq!(g.obj(giant).zone, Zone::Battlefield);
    g.act(0, Move::Pass).unwrap();
    g.act(1, Move::Pass).unwrap();
    assert_eq!(g.active, 0, "with an empty stack and both passing, the phase ends, not the turn (500.2)");
    cite!(["120.6"]; assert_eq!(g.obj(giant).damage, 2, "damage stays marked until the cleanup step (120.6)"));
    pass_to_next_turn(&mut g);
    assert_eq!(g.active, 1);
    cite!(["514.2"]; assert_eq!(g.obj(giant).damage, 0));
}

#[test]
fn r601_2c_a_target_is_an_object_or_player_the_text_describes() {
    let mut g = game();
    let bears = g.put(1, BEARS, Zone::Battlefield);
    mana(&mut g, 0, &[SWAMP, SWAMP, SWAMP]);
    let murder = g.put(0, MURDER, Zone::Hand);
    let at_player = g.act(0, Move::Cast { card: murder, targets: vec![vec![Target::Player(1)]] });
    cite!(["115.1"]; assert_eq!(at_player, Err("an illegal target, that player: a player isn't a permanent (115.1)".to_string())));
    let land = g.battlefield[1];
    let at_land = g.act(0, Move::Cast { card: murder, targets: vec![vec![Target::Object(land)]] });
    cite!(["115.1"]; refused("an illegal target, Swamp: it isn't a creature (115.1)"): at_land);
    cite!(["601.2c"]; refused("that has 1 target group(s), 0 given"): g.act(0, Move::Cast { card: murder, targets: vec![] }));
    cast_and_resolve(&mut g, 0, murder, vec![vec![Target::Object(bears)]], "BBB");
    cite!(["701.8a"]; assert_eq!(g.obj(bears).zone, Zone::Gone));
    assert!(verbs(&g).contains(&Verb::Destroy));
}

#[test]
fn r601_2h_the_total_cost_is_paid_with_no_partial_payment() {
    let mut g = game();
    mana(&mut g, 0, &[FOREST]);
    let shock = g.put(0, SHOCK, Zone::Hand);
    let r = g.act(0, crate::support::paid("G", Move::Cast { card: shock, targets: vec![vec![Target::Player(1)]] }));
    cite!(["118.3a"]; assert_eq!(r, Err("the supplied mana does not pay that cost, all of it and no more (601.2h, 118.3a)".to_string())));
    assert_eq!(g.players[0].hand, vec![shock], "a refused move changes nothing");
    assert_eq!(g.players[0].pool.total(), 1);
}

#[test]
fn r608_3a_a_permanent_spell_becomes_a_permanent_as_it_resolves() {
    let mut g = game();
    mana(&mut g, 0, &[FOREST, FOREST]);
    let bears = g.put(0, BEARS, Zone::Hand);
    cast_and_resolve(&mut g, 0, bears, vec![], "GG");
    let on_bf: Vec<_> = g.battlefield.iter().filter(|o| g.obj(**o).chars.name == "Grizzly Bears").collect();
    cite!(["608.3a"]; assert_eq!(on_bf.len(), 1));
    assert_eq!(g.power(*on_bf[0]), Some(2));
    assert_eq!(g.players[0].pool.total(), 0);
}

#[test]
fn r117_1_only_the_player_with_priority_casts_a_spell() {
    let mut g = game();
    let divination = g.put(1, DIVINATION, Zone::Hand);
    let bears = g.put(0, BEARS, Zone::Hand);
    crate::support::refused(g.act(1, Move::Cast { card: divination, targets: vec![] }), "that player does not have priority");
    g.act(0, Move::Pass).unwrap();
    mana(&mut g, 1, &[ISLAND, ISLAND, ISLAND]);
    let r = g.act(1, Move::Cast { card: divination, targets: vec![] });
    // a sorcery on another player's turn
    cite!(["307.1", "117.1a"]; refused("that is cast in a main phase of your own turn with the stack empty (117.1a)"): r);
    cite!(["601.3"]; refused("that card is in another player's hand; you cast only from your own (601.3)"): g.act(1, Move::Cast { card: bears, targets: vec![] }));
}

#[test]
fn r305_2_a_player_plays_one_land_during_their_turn() {
    let mut g = game();
    let (a, b) = (g.put(0, FOREST, Zone::Hand), g.put(0, FOREST, Zone::Hand));
    g.act(0, Move::PlayLand(a)).unwrap();
    cite!(["305.2"]; refused("you have played a land this turn (305.2)"): g.act(0, Move::PlayLand(b)));
    assert_eq!(g.battlefield.len(), 1);
}

#[test]
fn r704_5b_a_player_who_drew_from_an_empty_library_loses_the_game() {
    let mut g = game();
    for _ in 0..3 {
        g.put(0, FOREST, Zone::Library);
    }
    mana(&mut g, 0, &[ISLAND, ISLAND, ISLAND]);
    let divination = g.put(0, DIVINATION, Zone::Hand);
    cast_and_resolve(&mut g, 0, divination, vec![], "UUU");
    assert_eq!(g.players[0].hand.len(), 2);
    assert_eq!(g.players[0].library.len(), 1);
    assert_eq!(verbs(&g).iter().filter(|v| **v == Verb::Draw).count(), 2);
    // player 1 has no library: the draw of their draw step loses them the game (504.1, 704.5b)
    pass_to_next_turn(&mut g);
    while g.winner().is_none() {
        g.act(g.priority, Move::Pass).unwrap();
    }
    cite!(["704.5b"]; assert!(g.players[1].lost));
    assert_eq!(g.winner(), Some(0));
    crate::support::refused(g.act(0, Move::Pass), "the game is over");
}

#[test]
fn r119_3_a_player_who_gains_life_has_their_life_total_adjusted() {
    let mut g = game();
    mana(&mut g, 0, &[PLAINS, PLAINS, PLAINS, PLAINS]);
    let mercy = g.put(0, MERCY, Zone::Hand);
    cast_and_resolve(&mut g, 0, mercy, vec![], "WWWW");
    cite!(["119.3"]; assert_eq!(g.players[0].life, 27));
    assert!(verbs(&g).contains(&Verb::GainLife));
}

#[test]
fn r701_17a_a_player_mills_cards_from_the_top_of_their_library() {
    let mut g = game();
    let library:Vec<_>=(0..7).map(|_|g.put(1,FOREST,Zone::Library)).collect();
    mana(&mut g, 0, &[ISLAND, ISLAND]);
    let scour = g.put(0, SCOUR, Zone::Hand);
    cast_and_resolve(&mut g, 0, scour, vec![vec![Target::Player(1)]], "U");
    // The owner explicitly puts the five top cards in this oldest-first graveyard order.
    g.act(1,Move::Choose(game::state::Answer::Objects(vec![library[6],library[5],library[4],library[3],library[2]]))).unwrap();
    cite!(["701.17a"]; assert_eq!((g.players[1].library.len(), g.players[1].graveyard.len()), (2, 5)));
    assert!(verbs(&g).contains(&Verb::Mill));
}

#[test]
fn r122_1a_a_plus_one_counter_adds_to_power_and_toughness() {
    let mut g = game();
    let bears = g.put(0, BEARS, Zone::Battlefield);
    mana(&mut g, 0, &[FOREST]);
    let growth = g.put(0, GROWTH, Zone::Hand);
    cast_and_resolve(&mut g, 0, growth, vec![vec![Target::Object(bears)]], "G");
    cite!(["122.1a"]; assert_eq!((g.power(bears), g.toughness(bears)), (Some(3), Some(3))));
    assert!(verbs(&g).contains(&Verb::PutCounters));
}

#[test]
fn r111_4_a_created_token_has_the_characteristics_its_text_gives_it() {
    let mut g = game();
    mana(&mut g, 0, &[PLAINS, PLAINS]);
    let alarm = g.put(0, ALARM, Zone::Hand);
    cast_and_resolve(&mut g, 0, alarm, vec![], "WW");
    let soldiers: Vec<_> = g.battlefield.iter().copied().filter(|o| g.obj(*o).token()).collect();
    assert_eq!(soldiers.len(), 2);
    // its name is its subtype plus the word "Token" when the text gives it no other (111.4)
    cite!(["111.4"]; assert_eq!(g.obj(soldiers[0]).chars.name, "Soldier Token"));
    cite!(["111.3"]; assert_eq!(g.power(soldiers[0]), Some(1)));
    assert!(verbs(&g).contains(&Verb::Create));
}

#[test]
fn r111_3_a_token_has_the_abilities_the_effect_that_creates_it_defines() {
    // "Create two 1/1 white Spirit creature tokens with flying."
    let mut g = game();
    mana(&mut g, 0, &[PLAINS, PLAINS, PLAINS]);
    let haunting = g.put(0, HAUNTING, Zone::Hand);
    cast_and_resolve(&mut g, 0, haunting, vec![], "WWW");
    let spirits: Vec<_> = g.battlefield.iter().copied().filter(|o| g.obj(*o).token()).collect();
    assert_eq!(spirits.len(), 2);
    for spirit in spirits {
        assert_eq!(g.obj(spirit).chars.name, "Spirit Token");
        cite!(["111.3"]; assert!(g.has(spirit, "Flying")));
        assert!(!g.has(spirit, "Haste"));
    }
}

#[test]
fn r400_7_an_object_that_changes_zones_is_a_new_object() {
    let mut g = game();
    let bears = g.put(0, BEARS, Zone::Battlefield);
    g.put_counters(bears, "+1/+1", 1);
    mana(&mut g, 0, &[ISLAND]);
    let unsummon = g.put(0, UNSUMMON, Zone::Hand);
    cast_and_resolve(&mut g, 0, unsummon, vec![vec![Target::Object(bears)]], "U");
    cite!(["400.7"]; assert_eq!(g.obj(bears).zone, Zone::Gone));
    let back = *g.players[0].hand.last().unwrap();
    assert_eq!(g.obj(back).chars.name, "Grizzly Bears");
    cite!(["400.7"]; assert!(g.obj(back).counters.is_empty(), "with no memory of the counter"));
    assert!(verbs(&g).contains(&Verb::Move));
}

#[test]
fn r111_7_a_token_that_leaves_the_battlefield_ceases_to_exist() {
    let mut g = game();
    g.put(0, BEARS, Zone::Battlefield);
    g.put(1, GIANT, Zone::Battlefield);
    mana(&mut g, 0, &[PLAINS, PLAINS, PLAINS, PLAINS, PLAINS, PLAINS]);
    let alarm = g.put(0, ALARM, Zone::Hand);
    cast_and_resolve(&mut g, 0, alarm, vec![], "WW");
    let judgment = g.put(0, JUDGMENT, Zone::Hand);
    cast_and_resolve(&mut g, 0, judgment, vec![], "WWWW");
    assert!(g.battlefield.iter().all(|o| !g.obj(*o).is(ir::CardType::Creature)));
    cite!(["111.7"]; assert_eq!(g.players[0].graveyard.len(), 3, "the Bears and two spells: tokens are in no graveyard (111.7)"));
    assert_eq!(g.players[1].graveyard.len(), 1);
    cite!(["701.8a"]; assert_eq!(g.log.iter().filter(|e| matches!(e, Event::Destroyed { .. })).count(), 4));
}

#[test]
fn r608_2b_a_spell_whose_every_target_is_illegal_doesnt_resolve() {
    let mut g = game();
    let bears = g.put(1, BEARS, Zone::Battlefield);
    mana(&mut g, 0, &[MOUNTAIN, MOUNTAIN]);
    let (shock, bolt) = (g.put(0, SHOCK, Zone::Hand), g.put(0, BOLT, Zone::Hand));
    // both at the Bears: the Bolt, cast last, resolves first and destroys them
    g.act(0, crate::support::paid("R", Move::Cast { card: shock, targets: vec![vec![Target::Object(bears)]] })).unwrap();
    g.act(0, crate::support::paid("R", Move::Cast { card: bolt, targets: vec![vec![Target::Object(bears)]] })).unwrap();
    g.act(0, Move::Pass).unwrap();
    g.act(1, Move::Pass).unwrap();
    assert_eq!(g.stack.len(), 1, "the Shock is still on the stack");
    g.act(0, Move::Pass).unwrap();
    g.act(1, Move::Pass).unwrap();
    cite!(["608.2b"]; assert!(g.log.iter().any(|e| matches!(e, Event::Fizzled { .. })), "its target is illegal (608.2b)"));
    cite!(["608.2b"]; assert_eq!(g.players[0].graveyard.len(), 2));
}

#[test]
fn text_the_engine_does_not_run_is_refused_with_the_reason() {
    for (text, why) in [
        // several mana chosen each for itself, as Oracle says it; "two mana of any color" is no
        // card's wording, and the grammar does not read it (106.1b)
        ("Add two mana in any combination of colors for each creature you control.", "variable color choices"),
        ("Target creature gets +3/+3 until your next turn.", "duration"),
        ("Discard any number of cards at random.", "random"),
    ] {
        let d = CardDef::new("Test", "{1}", "Instant", text, None).expect(text);
        let u = check(&d).expect_err(text);
        assert!(u.0.contains(why), "{text}: {}", u.0);
    }
}
