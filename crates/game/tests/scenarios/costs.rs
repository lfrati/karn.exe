//! Costs, mana and the casting and activating that pays them (CR 106, 107, 113, 115, 118, 119,
//! 121, 202, 305, 601, 602, 605), rule by rule. Every card is read from its printed text.

use crate::support::{cite, paid};
use game::card::CardDef;
use game::play::check;
use game::state::{Decision, Pool};
use game::{Event, Game, Move, ObjId, Target, Zone};
use ir::{Color, Part, Sort};

const FOREST: usize = 0;
const BEARS: usize = 1;
const PYROMANCER: usize = 2;
const ELVES: usize = 3;
const SWAMP: usize = 4;
const GREED: usize = 5;
const RUSALKA: usize = 6;
const EGG: usize = 7;
const PLAINS: usize = 8;
const MOUNTAIN: usize = 9;
const ORNITHOPTER: usize = 10;
const SENTRY: usize = 11;
const ROTEROTHOPTER: usize = 12;
const LEAD_ASTRAY: usize = 13;
const SEEDS: usize = 14;
const CINDERMAW: usize = 15;
const SEAT: usize = 16;
const ARBOR: usize = 17;
const BOLT: usize = 18;

fn defs() -> Vec<CardDef> {
    let card = |name, cost, types, text, pt| CardDef::new(name, cost, types, text, pt).expect(name);
    vec![
        card("Forest", "", "Basic Land — Forest", "({T}: Add {G}.)", None),
        card("Grizzly Bears", "{1}{G}", "Creature — Bear", "", Some((2, 2))),
        card("Prodigal Pyromancer", "{2}{R}", "Creature — Human Wizard", "{T}: This creature deals 1 damage to any target.", Some((1, 1))),
        card("Llanowar Elves", "{G}", "Creature — Elf Druid", "{T}: Add {G}.", Some((1, 1))),
        card("Swamp", "", "Basic Land — Swamp", "({T}: Add {B}.)", None),
        card("Greed", "{3}{B}", "Enchantment", "{B}, Pay 2 life: Draw a card.", None),
        card("Starved Rusalka", "{G}", "Creature — Spirit", "{G}, Sacrifice a creature: You gain 1 life.", Some((1, 1))),
        card(
            "Darkwater Egg",
            "{1}",
            "Artifact",
            "{2}, {T}, Sacrifice this artifact: Add {U}{B}. Draw a card. (Activate only as an instant.)",
            None,
        ),
        card("Plains", "", "Basic Land — Plains", "({T}: Add {W}.)", None),
        card("Mountain", "", "Basic Land — Mountain", "({T}: Add {R}.)", None),
        card("Ornithopter", "{0}", "Artifact Creature — Thopter", "Flying", Some((0, 2))),
        card(
            "Safehold Sentry",
            "{1}{W}",
            "Creature — Elf Warrior",
            "{2}{W}, {Q}: This creature gets +0/+2 until end of turn. ({Q} is the untap symbol.)",
            Some((2, 2)),
        ),
        card(
            "Roterothopter",
            "{1}",
            "Artifact Creature — Thopter",
            "Flying\n{2}: This creature gets +1/+0 until end of turn. Activate no more than twice each turn.",
            Some((0, 2)),
        ),
        card("Lead Astray", "{1}{W}", "Instant", "Tap up to two target creatures.", None),
        card(
            "Seeds of Strength",
            "{G}{W}",
            "Instant",
            "Target creature gets +1/+1 until end of turn.\nTarget creature gets +1/+1 until end of turn.\nTarget creature gets +1/+1 until end of turn.",
            None,
        ),
        card(
            "Giant Cindermaw",
            "{2}{R}",
            "Creature — Dinosaur Beast",
            "Trample (This creature can deal excess combat damage to the player or planeswalker it's attacking.)\nPlayers can't gain life.",
            Some((4, 3)),
        ),
        card("Seat of the Synod", "", "Artifact Land", "{T}: Add {U}.", None),
        card(
            "Dryad Arbor",
            "",
            "Land Creature — Forest Dryad",
            "(This land isn't a spell, it's affected by summoning sickness, and it has \"{T}: Add {G}.\")",
            Some((1, 1)),
        ),
        card("Lightning Bolt", "{R}", "Instant", "Lightning Bolt deals 3 damage to any target.", None),
    ]
}

fn game() -> Game {
    Game::new(defs(), 2, 20)
}

fn object(o: ObjId) -> Target {
    Target::Object(o)
}

/// All players pass in succession once.
fn all_pass(g: &mut Game) {
    for _ in 0..g.players.len() {
        g.act(g.priority, Move::Pass).unwrap();
    }
}

/// Pass until the game is as asked, declaring no attackers when attackers are to be declared.
fn pass_until(g: &mut Game, done: impl Fn(&Game) -> bool) {
    while !done(g) {
        match g.decision {
            Some(Decision::Attackers { player }) => g.act(player, Move::Attack(vec![])).unwrap(),
            _ => g.act(g.priority, Move::Pass).unwrap(),
        }
    }
}

/// Put lands on the battlefield for a player and tap them all for mana.
fn mana(g: &mut Game, player: usize, lands: &[usize]) {
    for land in lands {
        let l = g.put(player, *land, Zone::Battlefield);
        g.act(player, Move::Activate { source: l, ability: 0, targets: vec![], paying: vec![] }).unwrap();
    }
}

/// Activate an ability of a permanent with these targets, paying with these choices.
fn activate(g: &mut Game, player: usize, source: ObjId, ability: usize, targets: Vec<Vec<Target>>, paying: Vec<Vec<ObjId>>, spending: &str) -> Result<(), String> {
    g.act(player, paid(spending, Move::Activate { source, ability, targets, paying }))
}

fn cast(g: &mut Game, player: usize, card: ObjId, targets: Vec<Vec<Target>>, spending: &str) -> Result<(), String> {
    g.act(player, paid(spending, Move::Cast { card, targets }))
}

fn green(g: &Game, player: usize) -> u32 {
    g.players[player].pool.colors[Color::Green as usize]
}

/// A card the engine is to refuse, read from its printed text.
fn refused(name: &str, cost: &str, types: &str, text: &str, pt: Option<(i32, i32)>) -> String {
    let def = CardDef::new(name, cost, types, text, pt).expect("the grammar reads it");
    check(&def).expect_err("the engine refuses it").0
}

#[test]
fn every_card_of_the_tests_is_one_the_engine_runs() {
    for d in defs() {
        check(&d).unwrap_or_else(|u| panic!("{}: {}", d.name(), u.0));
    }
}

#[test]
fn r106_12_to_tap_a_permanent_for_mana_is_to_activate_its_mana_ability_with_the_tap_symbol() {
    let mut g = game();
    let elves = g.put(0, ELVES, Zone::Battlefield);
    cite!(["106.12"]; ok: g.act(0, Move::Activate { source: elves, ability: 0, targets: vec![], paying: vec![] }));
    // the ability is activated: its {T} is paid and its mana added at once
    assert!(g.obj(elves).tapped);
    cite!(["106.12"]; assert!(g.log.contains(&Event::Activated { source: elves, by: 0, ability: None, keyword: None })));
    assert_eq!(green(&g, 0), 1);

}

#[test]
fn r106_4_added_mana_stays_in_the_pool_until_spent_or_the_step_or_phase_ends() {
    let mut g = game();
    mana(&mut g, 0, &[FOREST, FOREST, FOREST]);
    assert_eq!(green(&g, 0), 3);
    let elves = g.put(0, ELVES, Zone::Hand);
    // it can be used to pay a cost at once
    cast(&mut g, 0, elves, vec![], "G").unwrap();
    cite!(["106.4"]; assert_eq!(green(&g, 0), 2));
    // or stay unspent: through the spell's resolving, while the phase goes on
    all_pass(&mut g);
    cite!(["106.4"]; assert_eq!((g.part, g.stack.len(), green(&g, 0)), (Part::PrecombatMain, 0, 2)));
    // the pool empties as the phase ends
    all_pass(&mut g);
    assert_eq!(g.part, Part::BeginningOfCombat);
    cite!(["106.4"]; assert_eq!(g.players[0].pool, Pool::default()));
}

#[test]
fn r107_5_the_tap_symbol_taps_an_untapped_permanent_whose_creature_has_been_controlled_since_the_turn_began() {
    let mut g = game();
    let pyromancer = g.put(0, PYROMANCER, Zone::Battlefield);
    // "Tap this permanent"
    activate(&mut g, 0, pyromancer, 0, vec![vec![Target::Player(1)]], vec![], "").unwrap();
    cite!(["107.5"]; assert!(g.obj(pyromancer).tapped));
    assert!(g.log.contains(&Event::Tapped { object: pyromancer }));
    // a permanent that's already tapped can't be tapped again to pay the cost
    cite!(["107.5"]; refused("Prodigal Pyromancer can't pay {T} as it is (107.5, 118.3)"): activate(&mut g, 0, pyromancer, 0, vec![vec![Target::Player(1)]], vec![], ""));
    assert_eq!(g.stack.len(), 1);
    // a land likewise
    let forest = g.put(0, FOREST, Zone::Battlefield);
    g.act(0, Move::Activate { source: forest, ability: 0, targets: vec![], paying: vec![] }).unwrap();
    cite!(["107.5"]; refused("Forest can't pay {T} as it is (107.5, 118.3)"): g.act(0, Move::Activate { source: forest, ability: 0, targets: vec![], paying: vec![] }));
    all_pass(&mut g);
    // a creature that has not been under its controller's control since their turn began
    let elves = g.put(0, ELVES, Zone::Hand);
    cast(&mut g, 0, elves, vec![], "G").unwrap();
    all_pass(&mut g);
    let elves = *g.battlefield.last().expect("the Elves arrived");
    assert_eq!(g.obj(elves).chars.name, "Llanowar Elves");
    cite!(["602.5a"]; refused("Llanowar Elves has not been under your control since your turn began (602.5a)"): g.act(0, Move::Activate { source: elves, ability: 0, targets: vec![], paying: vec![] }));
    assert!(!g.obj(elves).tapped);
}

#[test]
fn r107_6_the_untap_symbol_untaps_a_tapped_permanent_whose_creature_has_been_controlled_since_the_turn_began() {
    let mut g = game();
    let sentry = g.put(0, SENTRY, Zone::Battlefield);
    mana(&mut g, 0, &[PLAINS, PLAINS, PLAINS]);
    // a permanent that's already untapped can't be untapped again to pay the cost
    cite!(["107.6"]; refused("Safehold Sentry can't pay {Q} as it is (107.6, 118.3)"): activate(&mut g, 0, sentry, 0, vec![], vec![], "WWW"));
    cite!(["107.6"]; assert_eq!((g.players[0].pool.total(), g.stack.len()), (3, 0), "nothing was paid"));
    // "Untap this permanent"
    g.tap(sentry);
    activate(&mut g, 0, sentry, 0, vec![], vec![], "WWW").unwrap();
    cite!(["107.6"]; assert!(!g.obj(sentry).tapped));
    assert!(g.log.contains(&Event::Untapped { object: sentry }));
    assert_eq!(g.players[0].pool.total(), 0);
    all_pass(&mut g);
    assert_eq!(g.toughness(sentry), Some(4));
    // a creature that has not been under its controller's control since their turn began
    let mut g = game();
    let card = g.put(0, SENTRY, Zone::Hand);
    mana(&mut g, 0, &[PLAINS, PLAINS]);
    cast(&mut g, 0, card, vec![], "WW").unwrap();
    all_pass(&mut g);
    let sentry = *g.battlefield.last().expect("the Sentry arrived");
    assert_eq!(g.obj(sentry).chars.name, "Safehold Sentry");
    g.tap(sentry);
    mana(&mut g, 0, &[PLAINS, PLAINS, PLAINS]);
    let refused = activate(&mut g, 0, sentry, 0, vec![], vec![], "WWW").unwrap_err();
    cite!(["602.5a"]; assert!(refused.contains("Safehold Sentry has not been under your control since your turn began (602.5a)"), "{refused}"));
    // the rules' refusal, not a limit of the engine's
    assert!(!refused.contains("engine"), "{refused}");
    assert!(g.obj(sentry).tapped);
}

#[test]
fn r113_3b_an_activated_ability_is_activated_whenever_its_player_has_priority_and_waits_on_the_stack() {
    let mut g = game();
    let pyromancer = g.put(1, PYROMANCER, Zone::Battlefield);
    let bolt = g.put(0, BOLT, Zone::Hand);
    mana(&mut g, 0, &[MOUNTAIN]);
    cast(&mut g, 0, bolt, vec![vec![Target::Player(1)]], "R").unwrap();
    // not while another player has priority
    cite!(["113.3b"]; refused("that player does not have priority"): activate(&mut g, 1, pyromancer, 0, vec![vec![Target::Player(0)]], vec![], ""));
    // in another player's turn, with a spell on the stack: whenever they have priority
    g.act(0, Move::Pass).unwrap();
    cite!(["113.3b"]; ok: activate(&mut g, 1, pyromancer, 0, vec![vec![Target::Player(0)]], vec![], ""));
    assert_eq!(g.stack.len(), 2);
    let ability = *g.stack.last().unwrap();
    assert!(g.is(object(ability), Sort::Ability));
    cite!(["113.3b"]; assert_eq!((g.players[0].life, g.players[1].life), (20, 20), "nothing has happened yet"));
    // it remains on the stack until it resolves
    all_pass(&mut g);
    cite!(["113.3b"]; assert_eq!((g.players[0].life, g.players[1].life, g.stack.len()), (19, 20, 1)));
    assert!(!g.stack.contains(&ability));
    all_pass(&mut g);
    assert_eq!((g.players[1].life, g.stack.len()), (17, 0));
}

#[test]
fn r113_6_abilities_of_a_permanent_card_function_only_on_the_battlefield() {
    let mut g = game();
    let in_hand = g.put(0, PYROMANCER, Zone::Hand);
    let in_graveyard = g.put(0, PYROMANCER, Zone::Graveyard);
    // an activated ability
    for p in [in_hand, in_graveyard] {
        cite!(["113.6"]; refused("an ability of a permanent on the battlefield is activated (113.6)"): activate(&mut g, 0, p, 0, vec![vec![Target::Player(1)]], vec![], ""));
    }
    // a static ability: "Players can't gain life." in a hand or a graveyard stops nothing
    g.put(1, CINDERMAW, Zone::Hand);
    g.put(0, CINDERMAW, Zone::Graveyard);
    let rusalka = g.put(0, RUSALKA, Zone::Battlefield);
    let bears = [g.put(0, BEARS, Zone::Battlefield), g.put(0, BEARS, Zone::Battlefield)];
    mana(&mut g, 0, &[FOREST]);
    activate(&mut g, 0, rusalka, 0, vec![], vec![vec![bears[0]]], "G").unwrap();
    all_pass(&mut g);
    cite!(["113.6"]; assert_eq!(g.players[0].life, 21));
    // on the battlefield it does
    g.put(1, CINDERMAW, Zone::Battlefield);
    mana(&mut g, 0, &[FOREST]);
    activate(&mut g, 0, rusalka, 0, vec![], vec![vec![bears[1]]], "G").unwrap();
    all_pass(&mut g);
    cite!(["113.6"]; assert_eq!(g.players[0].life, 21));
}

#[test]
fn r115_3_the_same_target_is_chosen_once_for_each_word_target() {
    let mut g = game();
    let bears = g.put(1, BEARS, Zone::Battlefield);
    let elves = g.put(1, ELVES, Zone::Battlefield);
    let astray = g.put(0, LEAD_ASTRAY, Zone::Hand);
    mana(&mut g, 0, &[PLAINS, PLAINS]);
    // "Tap up to two target creatures.": one "target", so not the same creature twice
    cite!(["115.3"]; refused("the same target twice for one \"target\" (115.3)"): cast(&mut g, 0, astray, vec![vec![object(bears), object(bears)]], "WW"));
    assert_eq!((g.players[0].pool.total(), g.stack.len()), (2, 0), "nothing was cast");
    cast(&mut g, 0, astray, vec![vec![object(bears), object(elves)]], "WW").unwrap();
    all_pass(&mut g);
    assert!(g.obj(bears).tapped && g.obj(elves).tapped);
    // three "target"s: the same creature once for each
    let seeds = g.put(0, SEEDS, Zone::Hand);
    mana(&mut g, 0, &[FOREST, PLAINS]);
    let mine = g.put(0, BEARS, Zone::Battlefield);
    cite!(["115.3"]; ok: cast(&mut g, 0, seeds, vec![vec![object(mine)], vec![object(mine)], vec![object(mine)]], "GW"));
    all_pass(&mut g);
    cite!(["115.3"]; assert_eq!((g.power(mine), g.toughness(mine)), (Some(5), Some(5))));
}

#[test]
fn r118_1_to_pay_a_cost_a_player_carries_out_its_instructions() {
    // "{2}, {T}, Sacrifice this artifact": mana removed, the artifact tapped, then sacrificed
    let mut g = game();
    let egg = g.put(0, EGG, Zone::Battlefield);
    g.put(0, BEARS, Zone::Library);
    mana(&mut g, 0, &[FOREST, FOREST]);
    activate(&mut g, 0, egg, 0, vec![], vec![], "GG").unwrap();
    assert_eq!(g.players[0].pool.total(), 0);
    cite!(["118.1"]; assert!(g.log.contains(&Event::Tapped { object: egg })));
    cite!(["118.1"]; assert!(g.log.contains(&Event::Sacrificed { player: 0, object: egg })));
    assert!(!g.battlefield.contains(&egg));
    // "Pay 2 life": the player loses that much life
    let mut g = game();
    let greed = g.put(0, GREED, Zone::Battlefield);
    g.put(0, BEARS, Zone::Library);
    mana(&mut g, 0, &[SWAMP]);
    activate(&mut g, 0, greed, 0, vec![], vec![], "B").unwrap();
    assert!(g.log.contains(&Event::LifeLost { player: 0, amount: 2 }));
    cite!(["118.1", "119.4"]; assert_eq!(g.players[0].life, 18));
}

#[test]
fn r118_10_one_payment_pays_the_cost_of_only_one_ability() {
    // "a player can't sacrifice just one creature to activate the activated abilities of two
    // permanents that each require sacrificing a creature as a cost"
    let mut g = game();
    let first = g.put(0, RUSALKA, Zone::Battlefield);
    let second = g.put(0, RUSALKA, Zone::Battlefield);
    let bears = g.put(0, BEARS, Zone::Battlefield);
    mana(&mut g, 0, &[FOREST, FOREST]);
    activate(&mut g, 0, first, 0, vec![], vec![vec![bears]], "G").unwrap();
    cite!(["118.10"]; refused("Grizzly Bears is not what that cost asks for or cannot pay it (701.21a, 118.3)"): activate(&mut g, 0, second, 0, vec![], vec![vec![bears]], "G"));
    assert_eq!((green(&g, 0), g.stack.len()), (1, 1), "the second was not paid");
    all_pass(&mut g);
    assert_eq!(g.players[0].life, 21);
}

#[test]
fn r118_3a_paying_mana_removes_it_from_the_pool_and_zero_can_always_be_paid() {
    // {0} with an empty pool
    let mut g = game();
    let ornithopter = g.put(0, ORNITHOPTER, Zone::Hand);
    assert_eq!(g.players[0].pool.total(), 0);
    cite!(["118.3a"]; ok: cast(&mut g, 0, ornithopter, vec![], ""));
    assert_eq!(g.stack.len(), 1);
    all_pass(&mut g);
    // {1}{G} out of {G}{G}{G}: the mana is removed, the excess remains
    let bears = g.put(0, BEARS, Zone::Hand);
    mana(&mut g, 0, &[FOREST]);
    cite!(["118.3a"]; refused("that mana is not in your mana pool (118.3a)"): cast(&mut g, 0, bears, vec![], "GG"));
    assert_eq!((green(&g, 0), g.players[0].hand.len()), (1, 1), "nothing was paid");
    mana(&mut g, 0, &[FOREST, FOREST]);
    cast(&mut g, 0, bears, vec![], "GG").unwrap();
    cite!(["118.3a"]; assert_eq!(green(&g, 0), 1));
}

#[test]
fn r119_4b_a_player_can_always_pay_0_life() {
    // no card asks for 0 life; the engine's own question, asked of a life total of 0 and less
    let mut g = Game::new(defs(), 2, 0);
    cite!(["119.4b"]; assert!(g.can_pay_life(0, 0)));
    cite!(["119.4"]; assert!(!g.can_pay_life(0, 1)));
    g.players[0].life = -3;
    cite!(["119.4b"]; assert!(g.can_pay_life(0, 0)));
    cite!(["119.4b"]; assert!(g.pay_life(0, 0).unwrap()));
    assert_eq!(g.players[0].life, -3);
}

#[test]
fn r101_2_a_player_who_cant_gain_life_gains_none() {
    // "Players can't gain life." against "You gain 1 life.": for either player
    for player in [0, 1] {
        let mut g = game();
        g.put(1 - player, CINDERMAW, Zone::Battlefield);
        if player == 1 {
            g.act(0, Move::Pass).unwrap();
        }
        let rusalka = g.put(player, RUSALKA, Zone::Battlefield);
        let bears = g.put(player, BEARS, Zone::Battlefield);
        mana(&mut g, player, &[FOREST]);
        activate(&mut g, player, rusalka, 0, vec![], vec![vec![bears]], "G").unwrap();
        all_pass(&mut g);
        assert_eq!(g.stack.len(), 0, "the ability resolved");
        cite!(["101.2"]; case: player; assert_eq!(g.players[player].life, 20));
        cite!(["101.2"]; case: player; assert!(!g.log.iter().any(|e| matches!(e, Event::LifeGained { .. }))));
    }
}

#[test]
fn r121_1_a_player_draws_by_putting_the_top_card_of_their_library_into_their_hand() {
    // as part of an effect
    let mut g = game();
    let greed = g.put(0, GREED, Zone::Battlefield);
    g.put(0, FOREST, Zone::Library);
    g.put(0, BEARS, Zone::Library);
    mana(&mut g, 0, &[SWAMP]);
    activate(&mut g, 0, greed, 0, vec![], vec![], "B").unwrap();
    all_pass(&mut g);
    let [card] = *g.players[0].hand.as_slice() else { panic!("one card drawn") };
    cite!(["121.1"]; assert_eq!(g.obj(card).chars.name, "Grizzly Bears", "the top card"));
    assert!(g.log.contains(&Event::Drew { player: 0, card, ordinal: 1, in_draw_step: None }));
    let [left] = *g.players[0].library.as_slice() else { panic!("one card left") };
    assert_eq!(g.obj(left).chars.name, "Forest");
    // as the turn-based action of the draw step
    let mut g = game();
    g.put(1, FOREST, Zone::Library);
    g.put(1, BEARS, Zone::Library);
    pass_until(&mut g, |g| g.turn == 2 && g.part == Part::Draw);
    let [card] = *g.players[1].hand.as_slice() else { panic!("one card drawn") };
    cite!(["121.1", "504.1"]; assert_eq!(g.obj(card).chars.name, "Grizzly Bears"));
    assert_eq!(g.players[1].library.len(), 1);
}

#[test]
fn r601_2_casting_takes_the_card_to_the_stack_and_pays_its_costs_or_nothing_happens() {
    let mut g = game();
    let bolt = g.put(0, BOLT, Zone::Hand);
    let bears = g.put(1, BEARS, Zone::Battlefield);
    // unable to comply with a step: the game is as it was before the casting was proposed
    let before = (g.players[0].hand.clone(), g.stack.clone(), g.players[0].pool.clone(), g.log.len());
    cite!(["601.2h", "118.3"]; refused("that mana is not in your mana pool (118.3a)"): cast(&mut g, 0, bolt, vec![vec![object(bears)]], "R")); // no mana
    mana(&mut g, 0, &[MOUNTAIN]);
    let before_paying = (g.players[0].hand.clone(), g.stack.clone(), g.players[0].pool.clone(), g.log.len());
    cite!(["601.2c"]; refused("that has 1 target group(s), 0 given"): cast(&mut g, 0, bolt, vec![], "R")); // no target
    cite!(["601.2", "733.1"]; assert_eq!((g.players[0].hand.clone(), g.stack.clone(), g.players[0].pool.clone(), g.log.len()), before_paying));
    cite!(["601.2", "733.1"]; assert_eq!((before.0, before.1), (g.players[0].hand.clone(), g.stack.clone())));
    // from the hand to the stack, its cost paid
    cite!(["601.2"]; ok: cast(&mut g, 0, bolt, vec![vec![object(bears)]], "R"));
    let [spell] = *g.stack.as_slice() else { panic!("one spell on the stack") };
    assert!(g.is(object(spell), Sort::Spell));
    assert!(g.log.contains(&Event::Cast { spell, by: 0 }));
    cite!(["601.2"]; assert_eq!((g.players[0].hand.len(), g.players[0].pool.total()), (0, 0)));
    // so that it eventually resolves
    all_pass(&mut g);
    cite!(["601.2"]; assert!(g.log.contains(&Event::Resolved { spell })));
    assert!(!g.battlefield.contains(&bears));
}

#[test]
fn r601_2b_a_spell_whose_casting_asks_for_an_announcement_the_engine_does_not_make_is_refused() {
    // A second variable remains unsupported. Hybrid equivalents are covered by positive
    // payment tests in hybrid_mana.rs, the announced value of X by r107_3_announced_x.rs,
    // and life or mana for Phyrexian symbols by r107_4f_phyrexian_mana.rs.
    check(&CardDef::new("Blaze", "{X}{R}", "Sorcery", "Blaze deals X damage to any target.", None).unwrap()).unwrap();
    let x = refused("Two variables", "{X}{Y}{R}", "Sorcery", "Two variables deals X damage to any target.", None);
    check(&CardDef::new(
        "Gut Shot",
        "{R/P}",
        "Instant",
        "({R/P} can be paid with either {R} or 2 life.)\nGut Shot deals 1 damage to any target.",
        None,
    ).unwrap()).unwrap();
    assert!(x.contains("cost: Var"), "{x}");
}

#[test]
fn r601_2f_the_total_cost_is_usually_the_mana_cost() {
    let mut g = game();
    let bears = g.put(0, BEARS, Zone::Hand);
    // {1}{G}: one green is not all of it
    mana(&mut g, 0, &[FOREST]);
    cite!(["601.2f", "118.3"]; refused("that mana is not in your mana pool (118.3a)"): cast(&mut g, 0, bears, vec![], "GG"));
    assert_eq!(green(&g, 0), 1);
    mana(&mut g, 0, &[SWAMP, SWAMP]);
    cast(&mut g, 0, bears, vec![], "BG").unwrap();
    cite!(["601.2f"]; assert_eq!(g.players[0].pool.total(), 1, "two of three paid"));
    // Additional costs compose with this mana payment; their payment and rollback
    // are exercised in additional_costs.rs (118.8, 601.2h).
    let splinters = CardDef::new(
        "Bone Splinters",
        "{B}",
        "Sorcery",
        "As an additional cost to cast this spell, sacrifice a creature.\nDestroy target creature.",
        None,
    ).unwrap();
    game::play::check(&splinters).unwrap();
}

#[test]
fn r602_1b_activation_instructions_are_not_part_of_the_effect_and_function_at_all_times() {
    // "{2}: This creature gets +1/+0 until end of turn. Activate no more than twice each turn."
    let mut g = game();
    let thopter = g.put(0, ROTEROTHOPTER, Zone::Battlefield);
    for player in [0, 1] {
        g.put(player, FOREST, Zone::Library);
    }
    mana(&mut g, 0, &[FOREST, FOREST, FOREST, FOREST, FOREST, FOREST]);
    // obeyed as it is activated, while the first is still on the stack
    activate(&mut g, 0, thopter, 1, vec![], vec![], "GG").unwrap();
    activate(&mut g, 0, thopter, 1, vec![], vec![], "GG").unwrap();
    cite!(["602.1b"]; refused("that ability is activated no more than 2 times each turn (602.1b)"): activate(&mut g, 0, thopter, 1, vec![], vec![], "GG"));
    assert_eq!((g.stack.len(), green(&g, 0)), (2, 2));
    // resolving, each does what its effect says and nothing of the instruction
    all_pass(&mut g);
    all_pass(&mut g);
    cite!(["602.1b"]; assert_eq!((g.stack.len(), g.power(thopter), g.toughness(thopter)), (0, Some(2), Some(2))));
    // "each turn": a later one allows it again (its controller's next turn, in the upkeep)
    pass_until(&mut g, |g| g.turn == 3 && g.part == Part::Upkeep);
    assert_eq!((g.active, g.priority, g.power(thopter)), (0, 0, Some(0)));
    mana(&mut g, 0, &[FOREST, FOREST, FOREST, FOREST]);
    activate(&mut g, 0, thopter, 1, vec![], vec![], "GG").unwrap();
    cite!(["602.1b"]; ok: activate(&mut g, 0, thopter, 1, vec![], vec![], "GG"));
    assert_eq!(g.stack.len(), 2);
}

#[test]
fn r602_1c_only_an_activated_ability_is_activated() {
    let mut g = game();
    let bears = g.put(0, BEARS, Zone::Battlefield);
    let thopter = g.put(0, ROTEROTHOPTER, Zone::Battlefield);
    let cindermaw = g.put(0, CINDERMAW, Zone::Battlefield);
    mana(&mut g, 0, &[FOREST, FOREST]);
    // no ability, a keyword ability (flying), a static ability ("Players can't gain life.")
    for (source, ability) in [(bears, 0), (thopter, 0), (cindermaw, 0), (cindermaw, 1)] {
        cite!(["602.1c"]; refused("that is not an activated ability of that permanent (602.1c)"): activate(&mut g, 0, source, ability, vec![], vec![], ""));
    }
    assert_eq!((g.stack.len(), g.players[0].pool.total()), (0, 2));
    cite!(["602.1c"]; ok: activate(&mut g, 0, thopter, 1, vec![], vec![], "GG"));
    assert_eq!(g.stack.len(), 1);
}

#[test]
fn r602_2a_an_activated_ability_is_created_on_top_of_the_stack_controlled_by_who_activated_it() {
    let mut g = game();
    let pyromancer = g.put(1, PYROMANCER, Zone::Battlefield);
    let bolt = g.put(0, BOLT, Zone::Hand);
    mana(&mut g, 0, &[MOUNTAIN]);
    cast(&mut g, 0, bolt, vec![vec![Target::Player(1)]], "R").unwrap();
    g.act(0, Move::Pass).unwrap();
    activate(&mut g, 1, pyromancer, 0, vec![vec![Target::Player(0)]], vec![], "").unwrap();
    // the topmost object on the stack: not a card, nor a spell
    let [spell, ability] = *g.stack.as_slice() else { panic!("two objects on the stack") };
    assert!(g.is(object(spell), Sort::Spell));
    assert!(g.is(object(ability), Sort::Ability));
    cite!(["602.2a"]; assert!(!g.is(object(ability), Sort::Spell) && !g.is(object(ability), Sort::Card)));
    // it has the text of the ability that created it, and its controller is who activated it
    cite!(["602.2a"]; assert_eq!(g.obj(ability).ability, Some((Some(pyromancer), 0))));
    cite!(["602.2a"]; assert_eq!(g.obj(ability).controller, 1));
    assert!(g.log.contains(&Event::Activated { source: pyromancer, by: 1, ability: Some(ability), keyword: None }));
    // it remains on the stack until it resolves
    assert!(g.stack.contains(&ability));
    all_pass(&mut g);
    assert_eq!(g.stack, vec![spell]);
    assert_eq!(g.obj(ability).zone, Zone::Gone);
    assert_eq!(g.players[0].life, 19);
}

#[test]
fn r602_2b_an_ability_is_activated_as_a_spell_is_cast_with_its_activation_cost_for_a_mana_cost() {
    let mut g = game();
    let pyromancer = g.put(0, PYROMANCER, Zone::Battlefield);
    let forest = g.put(0, FOREST, Zone::Battlefield);
    // targets are chosen as for a spell (601.2c): a land is not "any target"
    cite!(["601.2c", "115.1"]; refused("an illegal target, Forest: it isn't a creature or a planeswalker or a battle or a player (115.1)"): activate(&mut g, 0, pyromancer, 0, vec![vec![object(forest)]], vec![], ""));
    cite!(["601.2c"]; refused("wrong number of targets: 2 chosen, it takes 1 (601.2c)"): activate(&mut g, 0, pyromancer, 0, vec![vec![Target::Player(1), Target::Player(1)]], vec![], ""));
    assert!(!g.obj(pyromancer).tapped, "nothing was paid");
    // the activation cost is determined and paid as a spell's mana cost (601.2f–h): "{2}, {T},
    // Sacrifice this artifact" with mana the player chooses, all of it and no more
    let egg = g.put(0, EGG, Zone::Battlefield);
    g.put(0, BEARS, Zone::Library);
    g.act(0, Move::Activate { source: forest, ability: 0, targets: vec![], paying: vec![] }).unwrap();
    cite!(["118.3a"]; refused("that mana is not in your mana pool (118.3a)"): activate(&mut g, 0, egg, 0, vec![], vec![], "GG"));
    mana(&mut g, 0, &[SWAMP, PLAINS]);
    let activation = Move::Activate { source: egg, ability: 0, targets: vec![], paying: vec![] };
    let mut white_and_black = Pool::default();
    white_and_black.colors[Color::White as usize] = 1;
    white_and_black.colors[Color::Black as usize] = 1;
    let mut green_only = Pool::default();
    green_only.colors[Color::Green as usize] = 1;
    cite!(["601.2h"]; refused("the supplied mana does not pay that cost, all of it and no more (601.2h, 118.3a)"): g.act(0, Move::Paying(green_only, Box::new(activation.clone()))));
    cite!(["602.2b", "601.2h"]; ok: g.act(0, Move::Paying(white_and_black, Box::new(activation))));
    assert_eq!((green(&g, 0), g.players[0].pool.total()), (1, 1));
}

#[test]
fn r602_5_a_player_cant_begin_to_activate_a_prohibited_ability() {
    let mut g = game();
    let thopter = g.put(0, ROTEROTHOPTER, Zone::Battlefield);
    mana(&mut g, 0, &[FOREST, FOREST, FOREST, FOREST, FOREST, FOREST]);
    for _ in 0..2 {
        activate(&mut g, 0, thopter, 1, vec![], vec![], "GG").unwrap();
    }
    // "Activate no more than twice each turn.": a third is not begun, nothing is paid
    let log = g.log.len();
    cite!(["602.5"]; refused("that ability is activated no more than 2 times each turn (602.1b)"): activate(&mut g, 0, thopter, 1, vec![], vec![], "GG"));
    assert_eq!((g.log.len(), g.stack.len(), green(&g, 0)), (log, 2, 2));
    // A compiled prohibition also blocks mana abilities unless it explicitly excepts them.
    let mut definitions = defs();
    let silence = definitions.len();
    definitions.push(CardDef::new("Stony Silence", "{1}{W}", "Enchantment",
        "Activated abilities of artifacts can't be activated.", None).unwrap());
    check(&definitions[silence]).unwrap();
    let mut g = Game::new(definitions, 3, 20);
    let thopter = g.put(0, ROTEROTHOPTER, Zone::Battlefield);
    let land = g.put(0, SEAT, Zone::Battlefield);
    mana(&mut g, 0, &[FOREST, FOREST]);
    let silence = g.put(1, silence, Zone::Battlefield);
    let before = format!("{g:?}");
    cite!(["602.5"]; refused("an effect prohibits activating that ability (602.5)"): activate(&mut g, 0, thopter, 1, vec![], vec![], "GG"));
    // the Seat has a mana ability with {T}, prohibited: refused for that, not as if it had none
    cite!(["602.5"]; refused("an effect prohibits activating that ability (602.5)"): g.act(0, Move::Activate { source: land, ability: 0, targets: vec![], paying: vec![] }));
    cite!(["602.5"]; assert_eq!(format!("{g:?}"), before, "no costs are paid for a prohibited activation"));
    g.destroy(silence);
    cite!(["602.5"]; ok: activate(&mut g, 0, thopter, 1, vec![], vec![], "GG"));
    cite!(["602.5"]; ok: g.act(0, Move::Activate { source: land, ability: 0, targets: vec![], paying: vec![] }));

}

#[test]
fn r605_3a_a_mana_ability_is_activated_whenever_its_player_has_priority() {
    let mut g = game();
    let forest = g.put(1, FOREST, Zone::Battlefield);
    let elves = g.put(1, ELVES, Zone::Battlefield);
    let bolt = g.put(0, BOLT, Zone::Hand);
    mana(&mut g, 0, &[MOUNTAIN]);
    cast(&mut g, 0, bolt, vec![vec![Target::Player(1)]], "R").unwrap();
    // not without priority
    cite!(["605.3a"]; refused("that player does not have priority"): g.act(1, Move::Activate { source: forest, ability: 0, targets: vec![], paying: vec![] }));
    // in another player's turn, with a spell on the stack
    g.act(0, Move::Pass).unwrap();
    cite!(["605.3a"]; ok: g.act(1, Move::Activate { source: forest, ability: 0, targets: vec![], paying: vec![] }));
    assert_eq!((green(&g, 1), g.stack.len()), (1, 1));
    g.act(1, Move::Pass).unwrap();
    // in a step that is not a main phase
    pass_until(&mut g, |g| g.part == Part::BeginningOfCombat);
    g.act(0, Move::Pass).unwrap();
    cite!(["605.3a"]; ok: activate(&mut g, 1, elves, 0, vec![], vec![], ""));
    assert_eq!(green(&g, 1), 1);
    // in the middle of casting a spell or activating an ability is not here: mana is added
    // before what it pays for is cast or activated (crates/game/src/activate.rs)
}

#[test]
fn r202_1b_a_land_has_no_mana_cost_and_is_played_without_paying_any() {
    let defs = defs();
    // no mana cost, which is not a cost of {0}
    cite!(["202.1b"]; assert!(defs[FOREST].cost().is_none() && defs[SEAT].cost().is_none()));
    assert!(defs[ORNITHOPTER].cost().is_some());
    assert_eq!(defs[ORNITHOPTER].mana_value(), 0);
    let mut g = game();
    let forest = g.put(0, FOREST, Zone::Hand);
    let ornithopter = g.put(0, ORNITHOPTER, Zone::Hand);
    // a land is played without paying any costs
    cite!(["305.1"]; ok: g.act(0, Move::PlayLand(forest)));
    assert_eq!(g.players[0].pool.total(), 0);
    assert!(g.log.iter().any(|e| matches!(e, Event::LandPlayed { by: 0, .. })));
    // {0} is paid with nothing
    cast(&mut g, 0, ornithopter, vec![], "").unwrap();
    assert_eq!(g.stack.len(), 1);
}

#[test]
fn r305_9_a_land_of_another_card_type_too_is_played_only_as_a_land() {
    for land in [SEAT, ARBOR] {
        let mut g = game();
        let card = g.put(0, land, Zone::Hand);
        mana(&mut g, 0, &[FOREST, FOREST]);
        cite!(["305.9"]; refused("a land is played, not cast (305.9)"): cast(&mut g, 0, card, vec![], ""));
        assert_eq!(g.stack.len(), 0);
        cite!(["305.9"]; case: g.defs[land].name(); ok: g.act(0, Move::PlayLand(card)));
        assert_eq!(g.players[0].hand.len(), 0);
        assert!(g.battlefield.iter().any(|o| g.obj(*o).chars.name == g.defs[land].name()));
    }
}
