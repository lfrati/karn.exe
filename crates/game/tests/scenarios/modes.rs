//! Modal spells and abilities (CR 700.2): the modes are chosen as the spell is cast or the
//! triggered ability put on the stack, and it then has the instructions of the modes chosen.
//! Every card is read from its printed text.

use crate::support::{cite, paid};
use game::card::CardDef;
use game::legal::Asked;
use game::play::check;
use game::state::Decision;
use game::{Game, Move, Target, Zone};

const MOUNTAIN: usize = 0;
const PLAINS: usize = 1;
const FOREST: usize = 2;
const BEARS: usize = 3;
const CHARM: usize = 4;
const WINTERFLAME: usize = 5;
const KNIGHT: usize = 6;
const CLARION: usize = 7;
const SIGNET: usize = 8;
const ISLAND: usize = 9;

fn defs() -> Vec<CardDef> {
    let card = |name, cost, types, text, pt| CardDef::new(name, cost, types, text, pt).expect(name);
    vec![
        card("Mountain", "", "Basic Land — Mountain", "({T}: Add {R}.)", None),
        card("Plains", "", "Basic Land — Plains", "({T}: Add {W}.)", None),
        card("Forest", "", "Basic Land — Forest", "({T}: Add {G}.)", None),
        card("Grizzly Bears", "{1}{G}", "Creature — Bear", "", Some((2, 2))),
        card(
            "Boros Charm",
            "{R}{W}",
            "Instant",
            "Choose one —\n• Boros Charm deals 4 damage to target player or planeswalker.\n• Permanents you control gain indestructible until end of turn.\n• Target creature gains double strike until end of turn.",
            None,
        ),
        card(
            "Winterflame",
            "{1}{U}{R}",
            "Instant",
            "Choose one or both —\n• Tap target creature.\n• Winterflame deals 2 damage to target creature.",
            None,
        ),
        card(
            "Knight of Autumn",
            "{1}{G}{W}",
            "Creature — Dryad Knight",
            "When this creature enters, choose one —\n• Put two +1/+1 counters on this creature.\n• Destroy target artifact or enchantment.\n• You gain 4 life.",
            Some((2, 1)),
        ),
        card(
            "Deafening Clarion",
            "{1}{R}{W}",
            "Sorcery",
            "Choose one or both —\n• Deafening Clarion deals 3 damage to each creature.\n• Creatures you control gain lifelink until end of turn.",
            None,
        ),
        card("Mind Stone", "{2}", "Artifact", "{T}: Add {C}.", None),
        card("Island", "", "Basic Land — Island", "({T}: Add {U}.)", None),
    ]
}

fn game() -> Game {
    Game::new(defs(), 2, 20)
}

/// Player 0 taps lands of these kinds for mana.
fn mana(g: &mut Game, lands: &[usize]) {
    for land in lands {
        let land = g.put(0, *land, Zone::Battlefield);
        g.act(0, Move::Activate { source: land, ability: 0, targets: vec![], paying: vec![] }).unwrap();
    }
}

fn pass(g: &mut Game) {
    g.act(0, Move::Pass).unwrap();
    g.act(1, Move::Pass).unwrap();
}

fn modal(modes: &[usize], mv: Move) -> Move {
    Move::Modes(modes.to_vec(), Box::new(mv))
}

#[test]
fn every_card_of_the_tests_is_one_the_engine_runs() {
    for d in defs() {
        check(&d).unwrap_or_else(|u| panic!("{}: {}", d.name(), u.0));
    }
}

#[test]
fn r700_2a_the_modes_are_chosen_as_the_spell_is_cast() {
    // "Choose one —": the spell is cast with one of its modes, and does what that one says
    let mut g = game();
    mana(&mut g, &[MOUNTAIN, PLAINS]);
    let charm = g.put(0, CHARM, Zone::Hand);
    let to_player = paid("RW", Move::Cast { card: charm, targets: vec![vec![Target::Player(1)]] });
    cite!(["700.2a"]; refused("choose 1 to 1 of its modes (700.2a)"): g.act(0, to_player.clone()));
    cite!(["700.2a"]; refused("choose 1 to 1 of its modes (700.2a)"): g.act(0, modal(&[0, 1], to_player.clone())));
    cite!(["700.2a"]; refused("a mode the ability does not have"): g.act(0, modal(&[3], to_player.clone())));
    g.act(0, modal(&[0], to_player)).unwrap();
    let spell = *g.stack.last().unwrap();
    cite!(["700.2a"]; assert_eq!(g.obj(spell).modes, vec![0]));
    pass(&mut g);
    cite!(["700.2a"]; assert_eq!(g.players[1].life, 16));

    // the second mode: what it says and only that
    let mut g = game();
    let bears = g.put(0, BEARS, Zone::Battlefield);
    let opponents = g.put(1, BEARS, Zone::Battlefield);
    mana(&mut g, &[MOUNTAIN, PLAINS]);
    let charm = g.put(0, CHARM, Zone::Hand);
    g.act(0, modal(&[1], paid("RW", Move::Cast { card: charm, targets: vec![] }))).unwrap();
    pass(&mut g);
    assert!(g.has(bears, "Indestructible") && g.has(g.battlefield[2], "Indestructible"));
    assert!(!g.has(opponents, "Indestructible"));
    cite!(["700.2a"]; assert_eq!(g.players[1].life, 20));
}

#[test]
fn r700_2a_a_mode_that_would_be_illegal_cant_be_chosen() {
    // "Target creature gains double strike until end of turn.": with no creature there is no
    // legal target, and that mode is not listed; the others are
    let mut g = game();
    mana(&mut g, &[MOUNTAIN, PLAINS]);
    let charm = g.put(0, CHARM, Zone::Hand);
    cite!(["700.2a"]; refused("there is no legal target for it (601.2c)"): g.act(0, modal(&[2], paid("RW", Move::Cast { card: charm, targets: vec![vec![]] }))));
    let bears = g.put(1, BEARS, Zone::Battlefield);
    g.act(0, modal(&[2], paid("RW", Move::Cast { card: charm, targets: vec![vec![Target::Object(bears)]] }))).unwrap();
    pass(&mut g);
    assert!(g.has(bears, "Double strike"));
}

#[test]
fn r700_2c_a_spell_targets_only_what_the_modes_chosen_target() {
    // "Choose one or both — • Tap target creature. • Winterflame deals 2 damage to target
    // creature.": one target for one mode, two for both, in the order the modes are printed
    let mut g = game();
    let a = g.put(1, BEARS, Zone::Battlefield);
    let b = g.put(1, BEARS, Zone::Battlefield);
    mana(&mut g, &[MOUNTAIN, ISLAND, MOUNTAIN]);
    let flame = g.put(0, WINTERFLAME, Zone::Hand);
    let one = paid("RRU", Move::Cast { card: flame, targets: vec![vec![Target::Object(a)]] });
    crate::support::refused(g.act(0, modal(&[0, 1], one.clone())), "that has 2 target group(s), 1 given");
    cite!(["700.2c"]; refused("wrong number of targets: 0 chosen, it takes 1 (601.2c)"): g.act(0, modal(&[0, 1], paid("RRU", Move::Cast { card: flame, targets: vec![vec![Target::Object(a)], vec![]] }))));
    // chosen the other way round, the modes are still those printed, in that order
    let both = paid("RRU", Move::Cast { card: flame, targets: vec![vec![Target::Object(a)], vec![Target::Object(b)]] });
    g.act(0, modal(&[1, 0], both)).unwrap();
    pass(&mut g);
    assert!(g.obj(a).tapped && g.obj(a).damage == 0);
    assert!(!g.obj(b).tapped && g.obj(b).damage == 2);
    // the damage mode alone: its one target
    let mut g = game();
    let a = g.put(1, BEARS, Zone::Battlefield);
    mana(&mut g, &[MOUNTAIN, ISLAND, MOUNTAIN]);
    let flame = g.put(0, WINTERFLAME, Zone::Hand);
    cite!(["700.2c"]; ok: g.act(0, modal(&[1], paid("RRU", Move::Cast { card: flame, targets: vec![vec![Target::Object(a)]] }))));
    pass(&mut g);
    cite!(["700.2c"]; assert!(!g.obj(a).tapped && g.obj(a).damage == 2));
}

#[test]
fn r700_2d_a_mode_is_chosen_once_and_the_modes_are_followed_in_the_order_printed() {
    // "Deafening Clarion deals 3 damage to each creature. / Creatures you control gain
    // lifelink until end of turn.": both, the damage first
    let mut g = game();
    mana(&mut g, &[MOUNTAIN, PLAINS, MOUNTAIN]);
    let clarion = g.put(0, CLARION, Zone::Hand);
    cite!(["700.2d"]; refused("a mode is chosen once (700.2d)"): g.act(0, modal(&[0, 0], paid("RRW", Move::Cast { card: clarion, targets: vec![] }))));
    let bears = g.put(0, BEARS, Zone::Battlefield);
    let survivor = g.put(0, BEARS, Zone::Battlefield);
    g.put_counters(survivor, "+1/+1", 2);
    g.act(0, modal(&[1, 0], paid("RRW", Move::Cast { card: clarion, targets: vec![] }))).unwrap();
    cite!(["608.2c"]; assert_eq!(g.obj(*g.stack.last().unwrap()).modes, vec![0, 1]));
    pass(&mut g);
    // Both modes finish before SBAs destroy the Bears (704.3).
    assert_eq!(g.obj(bears).zone, Zone::Gone);
    assert_eq!(g.obj(survivor).zone, Zone::Battlefield);
    assert_eq!(g.obj(survivor).damage, 3);
    assert!(g.has(survivor, "Lifelink"));
    assert_eq!(g.players[0].life, 20, "Clarion, not a creature, dealt the damage");
}

#[test]
fn r603_3c_a_modal_triggered_ability_has_its_modes_chosen_as_it_goes_on_the_stack() {
    // "When this creature enters, choose one — • Put two +1/+1 counters on this creature.
    // • Destroy target artifact or enchantment. • You gain 4 life."
    let mut g = game();
    mana(&mut g, &[FOREST, PLAINS, FOREST]);
    let knight = g.put(0, KNIGHT, Zone::Hand);
    g.act(0, paid("GGW", Move::Cast { card: knight, targets: vec![] })).unwrap();
    pass(&mut g);
    cite!(["603.3c"]; assert!(matches!(g.decision, Some(Decision::Modes { player: 0, .. }))));
    // with no artifact or enchantment, the second mode can't be chosen (700.2b)
    let Some((0, Asked::Modes { options, .. })) = g.asked() else { panic!("the modes are asked") };
    cite!(["700.2b"]; assert_eq!(options.iter().map(|(m, _)| m.clone()).collect::<Vec<_>>(), vec![vec![0], vec![2]]));
    cite!(["603.3c"]; refused("a decision is pending: choose the modes of the triggered ability and their targets (603.3c)"): g.act(0, Move::ChooseTargets(vec![])));
    cite!(["603.3c"]; refused("there is no legal target for it (601.2c)"): g.act(0, modal(&[1], Move::ChooseTargets(vec![vec![]]))));
    g.act(0, modal(&[2], Move::ChooseTargets(vec![]))).unwrap();
    pass(&mut g);
    assert_eq!(g.players[0].life, 24);
    // with an artifact there, the second mode destroys it
    let mut g = game();
    let stone = g.put(1, SIGNET, Zone::Battlefield);
    mana(&mut g, &[FOREST, PLAINS, FOREST]);
    let knight = g.put(0, KNIGHT, Zone::Hand);
    g.act(0, paid("GGW", Move::Cast { card: knight, targets: vec![] })).unwrap();
    pass(&mut g);
    let Some((0, Asked::Modes { options, .. })) = g.asked() else { panic!("the modes are asked") };
    cite!(["700.2b"]; assert_eq!(options.len(), 3));
    g.act(0, modal(&[1], Move::ChooseTargets(vec![vec![Target::Object(stone)]]))).unwrap();
    pass(&mut g);
    assert_eq!(g.obj(stone).zone, Zone::Gone);
    // and the first puts the counters on the Knight
    let mut g = game();
    mana(&mut g, &[FOREST, PLAINS, FOREST]);
    let knight = g.put(0, KNIGHT, Zone::Hand);
    g.act(0, paid("GGW", Move::Cast { card: knight, targets: vec![] })).unwrap();
    pass(&mut g);
    g.act(0, modal(&[0], Move::ChooseTargets(vec![]))).unwrap();
    pass(&mut g);
    let knight = *g.battlefield.iter().find(|o| g.obj(**o).chars.name == "Knight of Autumn").unwrap();
    assert_eq!(g.power(knight), Some(4));
}

#[test]
fn r700_2_a_permanent_spell_does_not_choose_the_modes_of_its_abilities() {
    let mut g = game();
    mana(&mut g, &[FOREST, PLAINS, FOREST]);
    let card = g.put(0, KNIGHT, Zone::Hand);
    let cast = paid("GGW", Move::Cast { card, targets: vec![] });
    cite!(["700.2"]; refused("that spell is not modal: no mode is chosen (700.2)"): g.act(0, Move::Modes(vec![0], Box::new(cast))));
    assert!(g.players[0].hand.contains(&card) && g.stack.is_empty());
}
