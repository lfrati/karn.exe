//! What objects are and what is done to them, rule by rule: life totals (103.4, 119), the
//! owner and controller of a permanent (110.2), tokens (111), abilities on the stack (113),
//! damage and its results (120), color, mana value and the type line (202, 205), zones (400),
//! the last part of resolving (608.2n), doing what is possible (609.3), the actions create,
//! destroy and mill (701), the zones lifelink and deathtouch function from (702.2d, 702.15d),
//! reach (702.17b), and the counters, Rooms and Adventures the engine refuses (122.1, 709.5,
//! 715.3). Every card is read from its printed text.

use crate::support::cite;
use game::card::CardDef;
use game::play::check;
use game::state::Decision;
use game::{Event, Game, Move, ObjId, Target, Zone};
use ir::{CardType, Color, Part, Sort, Subtype, Supertype};

const FOREST: usize = 0;
const MOUNTAIN: usize = 1;
const SWAMP: usize = 2;
const ISLAND: usize = 3;
const PLAINS: usize = 4;
const BEARS: usize = 5;
const GIANT: usize = 6;
const BOLT: usize = 7;
const SHOCK: usize = 8;
const PYROMANCER: usize = 9;
const VISIONARY: usize = 10;
const WARDEN: usize = 11;
const ALARM: usize = 12;
const MURDER: usize = 13;
const MIND_ROT: usize = 14;
const SCOUR: usize = 15;
const RITUAL: usize = 16;
const DOOM_BLADE: usize = 17;
const GILACORN: usize = 18;
const ENFORCERS: usize = 19;
const ELIXIR: usize = 20;
const HATCHLING: usize = 21;
const HUNTERS: usize = 22;
const CROW: usize = 23;
const ARCHERS: usize = 24;
const GOLLUM: usize = 25;
const SPHERE: usize = 26;
const WATCHDOG: usize = 27;
const RUSALKA: usize = 28;
const DUELIST: usize = 29;
const SPIRIT: usize = 30;
const SOL_RING: usize = 31;

fn defs() -> Vec<CardDef> {
    let card = |name, cost, types, text, pt| CardDef::new(name, cost, types, text, pt).expect(name);
    vec![
        card("Forest", "", "Basic Land — Forest", "({T}: Add {G}.)", None),
        card("Mountain", "", "Basic Land — Mountain", "({T}: Add {R}.)", None),
        card("Swamp", "", "Basic Land — Swamp", "({T}: Add {B}.)", None),
        card("Island", "", "Basic Land — Island", "({T}: Add {U}.)", None),
        card("Plains", "", "Basic Land — Plains", "({T}: Add {W}.)", None),
        card("Grizzly Bears", "{1}{G}", "Creature — Bear", "", Some((2, 2))),
        card("Hill Giant", "{3}{R}", "Creature — Giant", "", Some((3, 3))),
        card("Lightning Bolt", "{R}", "Instant", "Lightning Bolt deals 3 damage to any target.", None),
        card("Shock", "{R}", "Instant", "Shock deals 2 damage to any target.", None),
        card("Prodigal Pyromancer", "{2}{R}", "Creature — Human Wizard", "{T}: This creature deals 1 damage to any target.", Some((1, 1))),
        card("Elvish Visionary", "{1}{G}", "Creature — Elf Shaman", "When this creature enters, draw a card.", Some((1, 1))),
        card("Soul Warden", "{W}", "Creature — Human Cleric", "Whenever another creature enters, you gain 1 life.", Some((1, 1))),
        card("Raise the Alarm", "{1}{W}", "Instant", "Create two 1/1 white Soldier creature tokens.", None),
        card("Murder", "{1}{B}{B}", "Instant", "Destroy target creature.", None),
        card("Mind Rot", "{2}{B}", "Sorcery", "Target player discards two cards.", None),
        card("Tome Scour", "{U}", "Sorcery", "Target player mills five cards.", None),
        card("Ritual of Soot", "{2}{B}{B}", "Sorcery", "Destroy all creatures with mana value 3 or less.", None),
        card("Doom Blade", "{1}{B}", "Instant", "Destroy target nonblack creature.", None),
        card(
            "Gilacorn",
            "{B}",
            "Creature — Lizard",
            "Deathtouch (Any amount of damage this deals to a creature is enough to destroy it.)",
            Some((1, 1)),
        ),
        card(
            "Merciless Enforcers",
            "{1}{B}",
            "Creature — Human Mercenary Villain",
            "Lifelink\n{3}{B}: This creature deals 1 damage to each opponent.",
            Some((2, 1)),
        ),
        card(
            "Cosmos Elixir",
            "{4}",
            "Artifact",
            "At the beginning of your end step, draw a card if your life total is greater than your starting life total. Otherwise, you gain 2 life.",
            None,
        ),
        card("Kraken Hatchling", "{U}", "Creature — Kraken", "", Some((0, 4))),
        card(
            "Sun-Crowned Hunters",
            "{4}{R}{R}",
            "Creature — Dinosaur",
            "Enrage — Whenever this creature is dealt damage, it deals 3 damage to target opponent or planeswalker.",
            Some((5, 4)),
        ),
        card(
            "Storm Crow",
            "{1}{U}",
            "Creature — Bird",
            "Flying (This creature can't be blocked except by creatures with flying or reach.)",
            Some((1, 2)),
        ),
        card("Keen-Eyed Archers", "{2}{W}", "Creature — Elf Archer", "Reach (This creature can block creatures with flying.)", Some((2, 2))),
        card(
            "Gollum, Silent Slinker",
            "{3}{B}",
            "Legendary Creature — Halfling Horror",
            "Menace (This creature can't be blocked except by two or more creatures.)",
            Some((4, 3)),
        ),
        card("Shield Sphere", "{0}", "Artifact Creature — Wall", "Defender\nWhenever this creature blocks, put a -0/-1 counter on it.", Some((0, 6))),
        card(
            "Faithful Watchdog",
            "{G}{W}",
            "Creature — Dog",
            "Vigilance\nThis creature enters with three +1/+1 counters on it.",
            Some((0, 0)),
        ),
        card("Starved Rusalka", "{G}", "Creature — Spirit", "{G}, Sacrifice a creature: You gain 1 life.", Some((1, 1))),
        card("Heartflame Duelist", "{1}{W}", "Creature — Human Knight", "Instant and sorcery spells you control have lifelink.", Some((3, 1))),
        card(
            "Pestilent Spirit",
            "{2}{B}",
            "Creature — Spirit",
            "Menace, deathtouch\nInstant and sorcery spells you control have deathtouch. (Any amount of damage they deal to a creature is enough to destroy it.)",
            Some((3, 2)),
        ),
        card("Sol Ring", "{1}", "Artifact", "{T}: Add {C}{C}.", None),
    ]
}

/// A two-player game at 20 life in player 0's first precombat main phase.
fn game() -> Game {
    Game::new(defs(), 2, 20)
}

fn object(o: ObjId) -> Target {
    Target::Object(o)
}

fn all_pass(g: &mut Game) {
    for _ in 0..g.players.len() {
        g.act(g.priority, Move::Pass).unwrap();
    }
}

/// Put lands on the battlefield for a player and tap them all for mana.
fn mana(g: &mut Game, player: usize, lands: &[usize]) {
    for land in lands {
        let l = g.put(player, *land, Zone::Battlefield);
        g.act(player, Move::Activate { source: l, ability: 0, targets: vec![], paying: vec![] }).unwrap();
    }
}

/// A player casts a card from their hand with the mana of these lands, and all pass so that
/// it resolves.
fn cast(g: &mut Game, player: usize, def: usize, lands: &[usize], targets: Vec<Vec<Target>>, spending: &str) -> ObjId {
    mana(g, player, lands);
    let card = g.put(player, def, Zone::Hand);
    g.act(player, crate::support::paid(spending, Move::Cast { card, targets })).unwrap();
    all_pass(g);
    card
}

fn permanents(g: &Game, name: &str) -> Vec<ObjId> {
    g.battlefield.iter().copied().filter(|o| g.obj(*o).chars.name == name).collect()
}

fn destroyed(g: &Game, o: ObjId) -> bool {
    g.log.contains(&Event::Destroyed { object: o })
}

/// Pass from the precombat main phase to the declaration of attackers.
fn to_attackers(g: &mut Game) {
    all_pass(g);
    all_pass(g);
    assert_eq!(g.part, Part::DeclareAttackers);
}

/// Pass until a decision is asked or something is on the stack.
fn pass_until_something(g: &mut Game) {
    for _ in 0..60 {
        if g.decision.is_some() || !g.stack.is_empty() {
            return;
        }
        g.act(g.priority, Move::Pass).unwrap();
    }
    panic!("nothing happened");
}

#[test]
fn every_card_of_the_tests_is_one_the_engine_runs() {
    for d in defs() {
        check(&d).unwrap_or_else(|u| panic!("{}: {}", d.name(), u.0));
    }
}

#[test]
fn r103_4_each_player_begins_the_game_with_a_starting_life_total() {
    // "draw a card if your life total is greater than your starting life total. Otherwise,
    // you gain 2 life.": the starting life total is the one the game began with, whatever the
    // life total has been since; "some variant games have different starting life totals"
    let end_step = |starting: i32, lost: u32, gained: u32| {
        let mut g = Game::new(defs(), 2, starting);
        assert_eq!(g.starting_life, starting);
        g.put(0, BEARS, Zone::Library);
        g.put(0, ELIXIR, Zone::Battlefield);
        g.lose_life(0, lost).unwrap();
        g.gain_life(0, gained).unwrap();
        assert_eq!(g.starting_life, starting, "a life total that changes is not the starting one");
        pass_until_something(&mut g);
        assert_eq!((g.part, g.stack.len()), (Part::End, 1));
        all_pass(&mut g);
        (g.players[0].life, g.players[0].hand.len())
    };
    cite!(["103.4"]; assert_eq!(end_step(20, 5, 6), (21, 1), "21 is greater than the starting 20: a card"));
    cite!(["103.4"]; assert_eq!(end_step(20, 5, 4), (21, 0), "19 is not: 2 life"));
    cite!(["103.4"]; assert_eq!(end_step(30, 5, 1), (28, 0), "26 is greater than 20 but not than the starting 30"));
}

#[test]
fn r119_1_each_player_begins_the_game_with_a_life_total_of_their_own() {
    let mut g = Game::new(defs(), 4, 20);
    cite!(["119.1"]; assert!(g.players.iter().all(|p| p.life == 20)));
    cast(&mut g, 0, BOLT, &[MOUNTAIN], vec![vec![Target::Player(2)]], "R");
    let lives: Vec<i32> = g.players.iter().map(|p| p.life).collect();
    cite!(["120.3a"]; assert_eq!(lives, vec![20, 20, 17, 20], "damage to one player changes that player's life total alone"));
}

#[test]
fn r110_2_a_permanent_is_controlled_by_the_player_under_whose_control_it_entered() {
    let mut g = game();
    let card = cast(&mut g, 0, BEARS, &[FOREST, FOREST], vec![], "GG");
    assert_eq!(g.obj(card).owner, 0);
    let [bears] = permanents(&g, "Grizzly Bears")[..] else { panic!("one Bears") };
    // its owner is the owner of the card that represents it, its controller who cast it
    cite!(["110.2"]; assert_eq!((g.obj(bears).owner, g.obj(bears).controller), (0, 0)));
    // an instant player 1 casts in player 0's turn: what it creates enters under player 1's
    // control, and every permanent has a controller
    g.act(0, Move::Pass).unwrap();
    mana(&mut g, 1, &[PLAINS, PLAINS]);
    let alarm = g.put(1, ALARM, Zone::Hand);
    g.act(1, crate::support::paid("WW", Move::Cast { card: alarm, targets: vec![] })).unwrap();
    all_pass(&mut g);
    let soldiers = permanents(&g, "Soldier Token");
    assert_eq!(soldiers.len(), 2);
    cite!(["111.2"]; assert!(soldiers.iter().all(|s| g.obj(*s).controller == 1)));
    cite!(["110.2"]; assert!(g.battlefield.iter().all(|o| g.obj(*o).controller < g.players.len())));
}

#[test]
fn r111_1_a_token_is_a_permanent_that_no_card_represents() {
    let mut g = game();
    cast(&mut g, 0, ALARM, &[PLAINS, PLAINS], vec![], "WW");
    let soldiers = permanents(&g, "Soldier Token");
    cite!(["701.7a"]; assert_eq!(soldiers.len(), 2, "the effect put tokens onto the battlefield"));
    for s in soldiers {
        cite!(["111.1"]; assert!(g.obj(s).token() && g.def(s).is_none(), "no card represents it"));
        cite!(["111.1"]; assert!(g.is(object(s), Sort::Permanent)));
        assert!(g.is(object(s), Sort::Of(CardType::Creature)));
        assert!(!g.is(object(s), Sort::Card));
    }
    // the only card is the spell, in its owner's graveyard
    assert_eq!((g.players[0].graveyard.len(), g.players[0].hand.len(), g.players[0].library.len()), (1, 0, 0));
}

#[test]
fn r111_3_a_token_has_only_the_characteristics_the_effect_that_created_it_defines() {
    // "Create two 1/1 white Soldier creature tokens.": a 1/1, white, a creature, a Soldier;
    // no mana cost, no supertype, no ability
    let mut g = game();
    cast(&mut g, 0, ALARM, &[PLAINS, PLAINS], vec![], "WW");
    let soldier = permanents(&g, "Soldier Token")[0];
    let chars = &g.obj(soldier).chars;
    cite!(["111.3"]; assert_eq!(chars.types, vec![CardType::Creature]));
    assert_eq!(chars.subtypes, vec![Subtype::from_name("Soldier").unwrap()]);
    cite!(["111.3"]; assert_eq!(chars.colors, vec![Color::White]));
    assert!(chars.supertypes.is_empty());
    cite!(["202.3a"]; assert_eq!(chars.mana_value, 0));
    cite!(["111.3"]; assert_eq!((g.power(soldier), g.toughness(soldier)), (Some(1), Some(1))));
    cite!(["111.3"]; assert!(g.abilities(soldier).is_empty()));
    assert!(g.seen(soldier, None).keywords.is_empty());
}

#[test]
fn r113_1c_an_activated_or_triggered_ability_on_the_stack_is_an_object() {
    let mut g = game();
    g.put(0, BEARS, Zone::Library);
    let pyromancer = g.put(0, PYROMANCER, Zone::Battlefield);
    g.act(0, Move::Activate { source: pyromancer, ability: 0, targets: vec![vec![Target::Player(1)]], paying: vec![] }).unwrap();
    let [activated] = g.stack[..] else { panic!("one object on the stack") };
    let o = g.obj(activated);
    cite!(["113.1c"]; assert_eq!((o.zone, o.controller, o.ability), (Zone::Stack, 0, Some((Some(pyromancer), 0)))));
    assert_eq!(o.targets, vec![vec![Target::Player(1)]], "an object with targets of its own");
    cite!(["113.1c"]; assert!(g.is(object(activated), Sort::Ability)));
    assert!(!g.is(object(activated), Sort::Spell) && !g.is(object(activated), Sort::Card) && !g.is(object(activated), Sort::Permanent));
    all_pass(&mut g);
    // a triggered ability: "When this creature enters, draw a card."
    cast(&mut g, 0, VISIONARY, &[FOREST, FOREST], vec![], "GG");
    let [visionary] = permanents(&g, "Elvish Visionary")[..] else { panic!("it entered") };
    let [triggered] = g.stack[..] else { panic!("its ability on the stack") };
    assert_ne!(triggered, visionary, "another object than its source");
    cite!(["603.3a"]; assert_eq!((g.obj(triggered).controller, g.obj(triggered).ability), (0, Some((Some(visionary), 0)))));
    cite!(["113.1c"]; assert!(g.is(object(triggered), Sort::Ability)));
}

#[test]
fn r113_3c_a_triggered_ability_stays_on_the_stack_until_it_resolves_or_leaves_it() {
    // "Whenever another creature enters, you gain 1 life.": two tokens enter, so the trigger
    // event occurs twice, and each ability is put on the stack before a player receives
    // priority
    let mut g = game();
    let warden = g.put(0, WARDEN, Zone::Battlefield);
    cast(&mut g, 0, ALARM, &[PLAINS, PLAINS], vec![], "WW");
    let Some(Decision::Order { player, abilities }) = &g.decision else { panic!("expected trigger order: {:?}", g.decision) };
    cite!(["603.3b"]; assert_eq!((*player, abilities.len()), (0, 2)));
    assert!(abilities.iter().all(|trigger| trigger.controller == 0 && trigger.source == Some(warden)));
    g.act(0, Move::Order(vec![0, 1])).unwrap();
    cite!(["113.3c"]; assert_eq!(g.stack.len(), 2));
    assert_eq!(g.priority, 0);
    // a spell cast above them resolves first; they stay there
    cast(&mut g, 0, SHOCK, &[MOUNTAIN], vec![vec![Target::Player(1)]], "R");
    cite!(["113.3c", "405.5"]; assert_eq!((g.players[1].life, g.stack.len(), g.players[0].life), (18, 2, 20)));
    all_pass(&mut g);
    assert_eq!((g.stack.len(), g.players[0].life), (1, 21));
    all_pass(&mut g);
    cite!(["113.3c"]; assert_eq!((g.stack.len(), g.players[0].life), (0, 22)));
}

#[test]
fn r120_3_the_results_of_damage_depend_on_what_it_is_dealt_to_and_its_source() {
    let mut g = game();
    let hatchling = g.put(1, HATCHLING, Zone::Battlefield);
    // to a player: life loss (120.3a)
    cast(&mut g, 0, BOLT, &[MOUNTAIN], vec![vec![Target::Player(1)]], "R");
    cite!(["120.3a"]; assert_eq!(g.players[1].life, 17));
    assert!(g.log.contains(&Event::LifeLost { player: 1, amount: 3 }));
    // to a creature: damage marked on it, and no player loses life (120.3e)
    let lost = g.log.iter().filter(|e| matches!(e, Event::LifeLost { .. })).count();
    cast(&mut g, 0, BOLT, &[MOUNTAIN], vec![vec![object(hatchling)]], "R");
    cite!(["120.3e"]; assert_eq!(g.obj(hatchling).damage, 3));
    cite!(["120.3e"]; assert_eq!(g.log.iter().filter(|e| matches!(e, Event::LifeLost { .. })).count(), lost));
    assert_eq!(g.players[1].life, 17);
    // from a source with lifelink, also life gained by its controller (120.3f)
    let enforcers = g.put(0, ENFORCERS, Zone::Battlefield);
    mana(&mut g, 0, &[SWAMP, SWAMP, SWAMP, SWAMP]);
    g.act(0, crate::support::paid("BBBB", Move::Activate { source: enforcers, ability: 1, targets: vec![], paying: vec![] })).unwrap();
    all_pass(&mut g);
    cite!(["120.3f"]; assert_eq!((g.players[0].life, g.players[1].life), (21, 16)));
}

#[test]
fn r120_3f_damage_from_a_source_with_lifelink_also_gains_its_controller_that_much_life() {
    // "Lifelink\n{3}{B}: This creature deals 1 damage to each opponent.": not combat damage,
    // and in addition to the life the opponent loses
    let mut g = game();
    let enforcers = g.put(0, ENFORCERS, Zone::Battlefield);
    mana(&mut g, 0, &[SWAMP, SWAMP, SWAMP, SWAMP]);
    g.act(0, crate::support::paid("BBBB", Move::Activate { source: enforcers, ability: 1, targets: vec![], paying: vec![] })).unwrap();
    all_pass(&mut g);
    assert!(g.log.contains(&Event::DamageDealt { source: enforcers, to: Target::Player(1), amount: 1, combat: false }));
    cite!(["120.3f"]; assert_eq!((g.players[0].life, g.players[1].life), (21, 19)));
    cite!(["120.3f"]; assert!(g.log.contains(&Event::LifeGained { player: 0, amount: 1 })));
    // the source's controller, whoever activated nothing: player 1's Enforcers dealt damage
    // gains player 1 the life
    let mut g = game();
    let theirs = g.put(1, ENFORCERS, Zone::Battlefield);
    let bears = g.put(0, BEARS, Zone::Battlefield);
    g.damage(theirs, object(bears), 2);
    cite!(["120.3f", "702.15b"]; assert_eq!((g.players[0].life, g.players[1].life), (20, 22)));
    cite!(["120.3e"]; assert_eq!(g.obj(bears).damage, 2, "and the damage is marked as well"));
}

#[test]
fn r120_8_a_source_that_would_deal_0_damage_deals_no_damage() {
    // a 0/4 blocked by "Enrage — Whenever this creature is dealt damage, …": no damage is
    // dealt to the Hunters, so their ability does not trigger
    let mut g = game();
    let hatchling = g.put(0, HATCHLING, Zone::Battlefield);
    let hunters = g.put(1, HUNTERS, Zone::Battlefield);
    to_attackers(&mut g);
    g.act(0, Move::Attack(vec![(hatchling, Target::Player(1))])).unwrap();
    all_pass(&mut g);
    g.act(1, Move::Block(vec![(hunters, hatchling)])).unwrap();
    all_pass(&mut g);
    assert_eq!(g.part, Part::CombatDamage);
    cite!(["120.8"]; assert!(!g.log.iter().any(|e| matches!(e, Event::DamageDealt { source, .. } if *source == hatchling))));
    assert_eq!(g.obj(hunters).damage, 0);
    cite!(["120.8"]; assert!(g.stack.is_empty() && g.decision.is_none(), "nothing triggered"));
    assert_eq!(g.players[0].life, 20);
    // the creature dealt damage by the Hunters died
    cite!(["704.5g"]; assert!(destroyed(&g, hatchling)));
}

#[test]
fn r202_2_an_object_is_the_colors_of_the_mana_symbols_in_its_mana_cost() {
    let mut g = game();
    let colors = |g: &mut Game, def| {
        let o = g.put(0, def, Zone::Hand);
        g.obj(o).chars.colors.clone()
    };
    cite!(["202.2"]; assert_eq!(colors(&mut g, BEARS), vec![Color::Green]));
    cite!(["202.2"]; assert_eq!(colors(&mut g, BOLT), vec![Color::Red]));
    cite!(["202.2c"]; assert_eq!(colors(&mut g, WATCHDOG), vec![Color::Green, Color::White], "two colored symbols: both colors (202.2c)"));
    cite!(["202.2b"]; assert_eq!(colors(&mut g, SOL_RING), vec![], "no colored mana symbol: colorless (202.2b)"));
    cite!(["202.2b"]; assert_eq!(colors(&mut g, MOUNTAIN), vec![], "no mana cost at all"));
    // "Destroy target nonblack creature.": {B} makes a creature black
    let gilacorn = g.put(1, GILACORN, Zone::Battlefield);
    let bears = g.put(1, BEARS, Zone::Battlefield);
    mana(&mut g, 0, &[SWAMP, SWAMP]);
    let blade = g.put(0, DOOM_BLADE, Zone::Hand);
    cite!(["115.1"]; refused("an illegal target, Gilacorn: it is black (115.1)"): g.act(0, crate::support::paid("BB", Move::Cast { card: blade, targets: vec![vec![object(gilacorn)]] })));
    g.act(0, crate::support::paid("BB", Move::Cast { card: blade, targets: vec![vec![object(bears)]] })).unwrap();
    all_pass(&mut g);
    assert!(destroyed(&g, bears));
}

#[test]
fn r202_3_the_mana_value_of_an_object_is_the_total_amount_of_mana_in_its_mana_cost() {
    let mut g = game();
    let value = |g: &mut Game, def| {
        let o = g.put(0, def, Zone::Hand);
        g.obj(o).chars.mana_value
    };
    cite!(["202.3"]; assert_eq!(value(&mut g, BEARS), 2));
    cite!(["202.3"]; assert_eq!(value(&mut g, GIANT), 4));
    cite!(["202.3"]; assert_eq!(value(&mut g, WATCHDOG), 2, "regardless of color"));
    assert_eq!(value(&mut g, RITUAL), 4);
    cite!(["202.3a"]; assert_eq!(value(&mut g, MOUNTAIN), 0, "no mana cost: 0 (202.3a)"));
    // "Destroy all creatures with mana value 3 or less.": the tokens have no mana cost
    let mut g = game();
    let bears = g.put(1, BEARS, Zone::Battlefield);
    let gilacorn = g.put(1, GILACORN, Zone::Battlefield);
    let giant = g.put(1, GIANT, Zone::Battlefield);
    cast(&mut g, 0, ALARM, &[PLAINS, PLAINS], vec![], "WW");
    cast(&mut g, 0, RITUAL, &[SWAMP, SWAMP, SWAMP, SWAMP], vec![], "BBBB");
    // Only the physical cards need their owner's order; tokens are not cards (111.6, 404.3).
    g.act(1,Move::Choose(game::state::Answer::Objects(vec![bears,gilacorn]))).unwrap();
    cite!(["202.3"]; assert!(destroyed(&g, bears) && destroyed(&g, gilacorn)));
    cite!(["202.3a"]; assert_eq!(g.battlefield.iter().filter(|o| g.obj(**o).is(CardType::Creature)).copied().collect::<Vec<_>>(), vec![giant]));
}

#[test]
fn r205_1_the_type_line_holds_the_card_types_subtypes_and_supertypes() {
    let parts = |def: &CardDef| (def.supertypes().to_vec(), def.types().to_vec(), def.subtypes().to_vec());
    let d = defs();
    let sub = |s| Subtype::from_name(s).unwrap();
    cite!(["205.1"]; assert_eq!(parts(&d[GOLLUM]), (vec![Supertype::Legendary], vec![CardType::Creature], vec![sub("Halfling"), sub("Horror")])));
    cite!(["205.1"]; assert_eq!(parts(&d[SPHERE]), (vec![], vec![CardType::Artifact, CardType::Creature], vec![sub("Wall")])));
    cite!(["205.1"]; assert_eq!(parts(&d[MOUNTAIN]), (vec![Supertype::Basic], vec![CardType::Land], vec![sub("Mountain")])));
    assert_eq!(parts(&d[BOLT]), (vec![], vec![CardType::Instant], vec![]));
    // and the object the card is has them
    let mut g = game();
    let gollum = g.put(0, GOLLUM, Zone::Battlefield);
    cite!(["205.1"]; assert_eq!(g.obj(gollum).chars.supertypes, vec![Supertype::Legendary]));
    assert!(g.is(object(gollum), Sort::Of(CardType::Creature)));
}

#[test]
fn r400_1_each_player_has_their_own_library_hand_and_graveyard_the_other_zones_are_shared() {
    let mut g = game();
    let (lib0, lib1) = (g.put(0, BEARS, Zone::Library), g.put(1, BEARS, Zone::Library));
    let (hand0, hand1) = (g.put(0, BEARS, Zone::Hand), g.put(1, BEARS, Zone::Hand));
    let (mine, theirs) = (g.put(0, GIANT, Zone::Battlefield), g.put(1, GIANT, Zone::Battlefield));
    cite!(["400.1"]; assert_eq!((g.players[0].library.clone(), g.players[1].library.clone()), (vec![lib0], vec![lib1])));
    cite!(["400.1"]; assert_eq!((g.players[0].hand.clone(), g.players[1].hand.clone()), (vec![hand0], vec![hand1])));
    cite!(["400.1"]; assert_eq!(g.battlefield, vec![mine, theirs], "one battlefield for both"));
    // a card in a library, a hand, a graveyard or exile is a card
    assert!([lib0, hand1].iter().all(|o| g.is(object(*o), Sort::Card)));
    // player 0's Murder destroys player 1's Giant: into player 1's graveyard
    cast(&mut g, 0, MURDER, &[SWAMP, SWAMP, SWAMP], vec![vec![object(theirs)]], "BBB");
    cite!(["400.1"]; assert_eq!((g.players[0].graveyard.len(), g.players[1].graveyard.len()), (1, 1)));
    // one exile for both
    g.exile(hand1);
    g.exile(mine);
    cite!(["400.1"]; assert_eq!(g.exile.len(), 2));
    assert!(g.exile.iter().all(|o| g.obj(*o).zone == Zone::Exile && g.is(object(*o), Sort::Card)));
}

#[test]
fn r608_2n_a_resolved_spell_goes_to_its_owners_graveyard_and_an_ability_ceases_to_exist() {
    let mut g = game();
    let shock = cast(&mut g, 0, SHOCK, &[MOUNTAIN], vec![vec![Target::Player(1)]], "R");
    cite!(["608.2n"]; assert!(g.stack.is_empty()));
    let [card] = g.players[0].graveyard[..] else { panic!("the Shock in its owner's graveyard") };
    cite!(["608.2n"]; assert_eq!(g.obj(card).chars.name, "Shock"));
    assert_eq!(g.obj(shock).zone, Zone::Gone);
    assert!(g.log.iter().any(|e| matches!(e, Event::ZoneChanged { now: Some(c), from: Zone::Stack, to: Zone::Graveyard, .. } if *c == card)));
    // an ability is removed from the stack and goes nowhere
    let pyromancer = g.put(0, PYROMANCER, Zone::Battlefield);
    g.act(0, Move::Activate { source: pyromancer, ability: 0, targets: vec![vec![Target::Player(1)]], paying: vec![] }).unwrap();
    let ability = g.stack[0];
    all_pass(&mut g);
    assert_eq!(g.players[1].life, 17);
    assert!(g.stack.is_empty());
    cite!(["608.2n"]; assert_eq!(g.obj(ability).zone, Zone::Gone));
    cite!(["608.2n"]; assert_eq!(g.players[0].graveyard.len(), 1, "no graveyard has it"));
    assert!(g.exile.is_empty() && !g.battlefield.contains(&ability));
}

#[test]
fn r609_3_an_effect_that_attempts_the_impossible_does_as_much_as_possible() {
    // "Target player discards two cards." of a player with one card in hand: that one
    let mut g = game();
    g.put(1, BEARS, Zone::Hand);
    cast(&mut g, 0, MIND_ROT, &[SWAMP, SWAMP, SWAMP], vec![vec![Target::Player(1)]], "BBB");
    assert_eq!(g.decision, None, "nothing to choose");
    cite!(["609.3"]; assert!(g.players[1].hand.is_empty()));
    assert_eq!(g.players[1].graveyard.len(), 1);
    cite!(["609.3"]; assert_eq!(g.log.iter().filter(|e| matches!(e, Event::Discarded { player: 1, .. })).count(), 1));
    // and of a player with none, nothing; the spell still resolves
    let mut g = game();
    cast(&mut g, 0, MIND_ROT, &[SWAMP, SWAMP, SWAMP], vec![vec![Target::Player(1)]], "BBB");
    cite!(["609.3"]; assert!(g.log.iter().any(|e| matches!(e, Event::Resolved { .. }))));
    cite!(["609.3"]; assert!(!g.log.iter().any(|e| matches!(e, Event::Discarded { .. }))));
}

#[test]
fn r701_7_to_create_tokens_is_to_put_that_many_with_those_characteristics_onto_the_battlefield() {
    // "Create two 1/1 white Soldier creature tokens.": two creatures enter the battlefield,
    // which the opponent's "Whenever another creature enters" sees twice
    let mut g = game();
    let warden = g.put(1, WARDEN, Zone::Battlefield);
    cast(&mut g, 0, ALARM, &[PLAINS, PLAINS], vec![], "WW");
    let soldiers = permanents(&g, "Soldier Token");
    cite!(["701.7a"]; assert_eq!(soldiers.len(), 2));
    for s in &soldiers {
        assert!(g.log.contains(&Event::Created { token: *s }));
        cite!(["701.7a"]; assert_eq!((g.obj(*s).zone, g.obj(*s).controller), (Zone::Battlefield, 0)));
        cite!(["701.7a"]; assert_eq!((g.power(*s), g.obj(*s).chars.colors.clone()), (Some(1), vec![Color::White])));
    }
    let Some(Decision::Order { player, abilities }) = &g.decision else { panic!("expected trigger order: {:?}", g.decision) };
    cite!(["603.3b"]; assert_eq!((*player, abilities.len()), (1, 2)));
    assert!(abilities.iter().all(|trigger| trigger.controller == 1 && trigger.source == Some(warden)));
    g.act(1, Move::Order(vec![0, 1])).unwrap();
    cite!(["603.2"]; assert_eq!(g.stack.len(), 2));
    all_pass(&mut g);
    all_pass(&mut g);
    assert_eq!(g.players[1].life, 22);
}

#[test]
fn r701_8_a_permanent_is_destroyed_only_by_destroy_or_by_lethal_damage_or_deathtouch() {
    let mut g = game();
    // an effect that says "destroy"
    let bears = g.put(1, BEARS, Zone::Battlefield);
    cast(&mut g, 0, MURDER, &[SWAMP, SWAMP, SWAMP], vec![vec![object(bears)]], "BBB");
    cite!(["701.8a"]; assert!(destroyed(&g, bears)));
    // the state-based action for lethal damage (704.5g)
    let giant = g.put(1, GIANT, Zone::Battlefield);
    cast(&mut g, 0, BOLT, &[MOUNTAIN], vec![vec![object(giant)]], "R");
    cite!(["704.5g"]; assert!(destroyed(&g, giant)));
    // and for damage from a source with deathtouch (704.5h)
    let other = g.put(1, GIANT, Zone::Battlefield);
    let gilacorn = g.put(0, GILACORN, Zone::Battlefield);
    g.damage(gilacorn, object(other), 1);
    g.state_based_actions();
    cite!(["704.5h"]; assert!(destroyed(&g, other)));
    // sacrificed: put into the graveyard, not destroyed
    let rusalka = g.put(0, RUSALKA, Zone::Battlefield);
    let mine = g.put(0, BEARS, Zone::Battlefield);
    mana(&mut g, 0, &[FOREST]);
    g.act(0, crate::support::paid("G", Move::Activate { source: rusalka, ability: 0, targets: vec![], paying: vec![vec![mine]] })).unwrap();
    all_pass(&mut g);
    assert_eq!(g.obj(mine).zone, Zone::Gone);
    cite!(["701.8b"]; assert!(!destroyed(&g, mine)));
    // toughness 0 (704.5f): put into the graveyard, not destroyed
    let hatchling = g.put(0, HATCHLING, Zone::Battlefield);
    g.put_counters(hatchling, "-1/-1", 4);
    g.state_based_actions();
    cite!(["704.5f"]; assert_eq!(g.obj(hatchling).zone, Zone::Gone));
    cite!(["701.8b"]; assert!(!destroyed(&g, hatchling)));
}

#[test]
fn r701_17_a_player_cant_mill_more_cards_than_are_in_their_library() {
    // "Target player mills five cards." with three: as many as possible (701.17b), and to mill
    // is not to draw, so the empty library loses nobody the game
    let mut g = game();
    let milled:Vec<_>=(0..3).map(|_|g.put(1,BEARS,Zone::Library)).collect();
    cast(&mut g, 0, SCOUR, &[ISLAND], vec![vec![Target::Player(1)]], "U");
    g.act(1,Move::Choose(game::state::Answer::Objects(vec![milled[2],milled[1],milled[0]]))).unwrap();
    cite!(["701.17b"]; assert_eq!((g.players[1].library.len(), g.players[1].graveyard.len()), (0, 3)));
    cite!(["701.17b"]; assert_eq!(g.log.iter().filter(|e| matches!(e, Event::Milled { player: 1, .. })).count(), 3));
    cite!(["704.5b"]; assert!(!g.players[1].lost));
    assert_eq!(g.priority, 0);
}

#[test]
fn r702_15d_lifelink_functions_whatever_zone_its_object_deals_damage_from() {
    // "Instant and sorcery spells you control have lifelink.": the Shock deals its damage from
    // the stack, with lifelink
    let mut g = game();
    g.put(0, DUELIST, Zone::Battlefield);
    mana(&mut g, 0, &[MOUNTAIN]);
    let shock = g.put(0, SHOCK, Zone::Hand);
    g.act(0, crate::support::paid("R", Move::Cast { card: shock, targets: vec![vec![Target::Player(1)]] })).unwrap();
    let spell = g.stack[0];
    assert!(g.has(spell, "Lifelink"), "the spell on the stack has lifelink");
    all_pass(&mut g);
    cite!(["702.15d"]; assert_eq!((g.players[0].life, g.players[1].life), (22, 18)));
}

#[test]
fn r702_2d_deathtouch_functions_whatever_zone_its_object_deals_damage_from() {
    // "Instant and sorcery spells you control have deathtouch.": 2 damage from the Shock on
    // the stack destroys a 3/3
    let mut g = game();
    g.put(0, SPIRIT, Zone::Battlefield);
    let giant = g.put(1, GIANT, Zone::Battlefield);
    cast(&mut g, 0, SHOCK, &[MOUNTAIN], vec![vec![object(giant)]], "R");
    cite!(["702.2d"]; assert!(destroyed(&g, giant)));
}

#[test]
fn r702_17b_reach_lets_a_creature_block_one_with_flying() {
    let mut g = game();
    let crow = g.put(0, CROW, Zone::Battlefield);
    let (bears, archers) = (g.put(1, BEARS, Zone::Battlefield), g.put(1, ARCHERS, Zone::Battlefield));
    to_attackers(&mut g);
    g.act(0, Move::Attack(vec![(crow, Target::Player(1))])).unwrap();
    all_pass(&mut g);
    assert_eq!(g.decision, Some(Decision::Blockers { player: 1 }));
    cite!(["702.9b", "509.1b"]; refused("Grizzly Bears can't block Storm Crow: Storm Crow has flying (702.9b, 509.1b)"): g.act(1, Move::Block(vec![(bears, crow)])));
    cite!(["702.17b"]; ok: g.act(1, Move::Block(vec![(archers, crow)])));
    // reach is not flying: a creature with reach is blocked by one without either
    let mut g = game();
    let archers = g.put(0, ARCHERS, Zone::Battlefield);
    let bears = g.put(1, BEARS, Zone::Battlefield);
    to_attackers(&mut g);
    g.act(0, Move::Attack(vec![(archers, Target::Player(1))])).unwrap();
    all_pass(&mut g);
    cite!(["509.1a", "509.1b"]; ok: g.act(1, Move::Block(vec![(bears, archers)])));
}

#[test]
fn r122_1c_a_shield_counter_is_a_replacement_and_prevention_effect_the_engine_does_not_apply() {
    // the engine applies neither effect yet: a text that puts a shield counter is refused
    let veteran = CardDef::new(
        "Brokers Veteran",
        "{1}{U}",
        "Creature — Human Soldier",
        "When this creature dies, put a shield counter on target creature you control. (If it would be dealt damage or destroyed, remove a shield counter from it instead.)",
        Some((2, 1)),
    )
    .expect("the grammar reads it");
    assert!(check(&veteran).unwrap_err().0.contains("shield"));
}

#[test]
fn r122_1j_a_hone_counter_on_an_equipment_gives_the_equipped_creature_plus_one_power() {
    // the engine has no Equipment attached to creatures: a text that puts a hone counter is
    // refused
    let dwalin = CardDef::new(
        "Dwalin, Weaponmaster",
        "{1}{R/W}",
        "Legendary Creature — Dwarf Warrior",
        "First strike\nWhenever Dwalin enters or attacks, put a hone counter on each Equipment you control. (Each hone counter on an Equipment grants +1/+0 to equipped creature.)",
        Some((2, 1)),
    )
    .expect("the grammar reads it");
    assert!(check(&dwalin).unwrap_err().0.contains("hone"));
}

#[test]
fn r709_5c_a_half_of_a_room_is_locked_unless_it_has_its_unlocked_designation() {
    // one half's text alone is not what the permanent does: the engine has no designations,
    // and refuses a Room
    let room = CardDef::new(
        "Funeral Room",
        "{2}{B}",
        "Enchantment — Room",
        "Whenever a creature you control dies, each opponent loses 1 life and you gain 1 life.\n(You may cast either half. That door unlocks on the battlefield. As a sorcery, you may pay the mana cost of a locked door to unlock it.)",
        None,
    )
    .expect("the grammar reads it");
    assert!(check(&room).unwrap_err().0.contains("Room"));
}

#[test]
fn r715_2_an_orphan_adventure_definition_requires_its_normal_characteristics() {
    // The alternative alone cannot supply the normal card that returns from exile.
    // Paired casting/resolution behavior is tested in r715_adventure.rs.
    let boon = CardDef::new(
        "Oaken Boon",
        "{3}{G}",
        "Sorcery — Adventure",
        "Put two +1/+1 counters on target creature. (Then exile this card. You may cast the creature later from exile.)",
        None,
    )
    .expect("the grammar reads it");
    assert!(check(&boon).unwrap_err().0.contains("Adventure"));
}
