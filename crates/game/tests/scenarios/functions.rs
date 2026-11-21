//! Where an ability functions (CR 113.6): on the battlefield, or, for an ability that moves its
//! own card out of a zone, in that zone alone (113.6m). Every card is read from its printed
//! text.

use crate::support::{cite, paid};
use game::card::CardDef;
use game::play::check;
use game::state::{Answer, Decision};
use game::{Game, Move, ObjId, Target, Zone};
use ir::Part;

const MOUNTAIN: usize = 0;
const SWAMP: usize = 1;
const BEARS: usize = 2;
const BOLT: usize = 3;
const PHOENIX: usize = 4;
const SKELETON: usize = 5;
const STEAM: usize = 6;
const SQUEE: usize = 7;
const FOREST: usize = 8;

fn defs() -> Vec<CardDef> {
    let card = |name, cost, types, text, pt| CardDef::new(name, cost, types, text, pt).expect(name);
    vec![
        card("Mountain", "", "Basic Land — Mountain", "({T}: Add {R}.)", None),
        card("Swamp", "", "Basic Land — Swamp", "({T}: Add {B}.)", None),
        card("Grizzly Bears", "{1}{G}", "Creature — Bear", "", Some((2, 2))),
        card("Lightning Bolt", "{R}", "Instant", "Lightning Bolt deals 3 damage to any target.", None),
        card(
            "Chandra's Phoenix",
            "{1}{R}{R}",
            "Creature — Phoenix",
            "Flying\nHaste (This creature can attack and {T} as soon as it comes under your control.)\nWhenever an opponent is dealt damage by a red instant or sorcery spell you control or by a red planeswalker you control, return this card from your graveyard to your hand.",
            Some((2, 2)),
        ),
        card("Reassembling Skeleton", "{1}{B}", "Creature — Skeleton Warrior", "{1}{B}: Return this card from your graveyard to the battlefield tapped.", Some((1, 1))),
        card("Steam Blast", "{2}{R}", "Sorcery", "Steam Blast deals 2 damage to each creature and each player.", None),
        card(
            "Squee, Goblin Nabob",
            "{2}{R}",
            "Legendary Creature — Goblin",
            "At the beginning of your upkeep, you may return this card from your graveyard to your hand.",
            Some((1, 1)),
        ),
        card("Forest", "", "Basic Land — Forest", "({T}: Add {G}.)", None),
    ]
}

fn game() -> Game {
    Game::new(defs(), 2, 20)
}

fn mana(g: &mut Game, player: usize, lands: &[usize]) {
    for land in lands {
        let land = g.put(player, *land, Zone::Battlefield);
        g.act(player, Move::Activate { source: land, ability: 0, targets: vec![], paying: vec![] }).unwrap();
    }
}

fn pass_both(g: &mut Game) {
    let first = g.priority;
    g.act(first, Move::Pass).unwrap();
    g.act(1 - first, Move::Pass).unwrap();
}

/// Player 0 casts this card with the mana of these lands and these targets; it resolves.
fn cast(g: &mut Game, def: usize, lands: &[usize], targets: Vec<Vec<Target>>, spending: &str) {
    mana(g, 0, lands);
    let card = g.put(0, def, Zone::Hand);
    g.act(0, paid(spending, Move::Cast { card, targets })).unwrap();
    pass_both(g);
}

fn names(g: &Game, cards: &[ObjId]) -> Vec<String> {
    let mut names: Vec<String> = cards.iter().map(|c| g.obj(*c).chars.name.clone()).collect();
    names.sort();
    names
}

#[test]
fn every_card_of_the_tests_is_one_the_engine_runs() {
    for d in defs() {
        check(&d).unwrap_or_else(|u| panic!("{}: {}", d.name(), u.0));
    }
}

#[test]
fn r113_6m_an_ability_that_returns_its_card_from_the_graveyard_triggers_there() {
    // "Whenever an opponent is dealt damage by a red instant or sorcery spell you control …,
    // return this card from your graveyard to your hand."
    let mut g = game();
    let phoenix = g.put(0, PHOENIX, Zone::Graveyard);
    cast(&mut g, BOLT, &[MOUNTAIN], vec![vec![Target::Player(1)]], "R");
    assert_eq!(g.players[1].life, 17);
    // the ability triggered in the graveyard and is on the stack
    assert_eq!(g.stack.len(), 1);
    cite!(["113.6m"]; assert_eq!(g.obj(g.stack[0]).ability.and_then(|(source, _)| source), Some(phoenix)));
    pass_both(&mut g);
    cite!(["113.6m"]; assert_eq!(names(&g, &g.players[0].hand), ["Chandra's Phoenix"]));
    assert_eq!(names(&g, &g.players[0].graveyard), ["Lightning Bolt"]);
}

#[test]
fn r113_6m_it_does_not_function_on_the_battlefield() {
    let mut g = game();
    g.put(0, PHOENIX, Zone::Battlefield);
    cast(&mut g, BOLT, &[MOUNTAIN], vec![vec![Target::Player(1)]], "R");
    cite!(["113.6m"]; assert!(g.stack.is_empty(), "nothing triggered"));
}

#[test]
fn r113_6m_a_card_put_into_the_graveyard_does_not_see_what_happened_before_it_got_there() {
    // "Steam Blast deals 2 damage to each creature and each player.": the Phoenix on the
    // battlefield is dealt damage as the opponent is, and is put into the graveyard after,
    // as a new object (400.7) that was not there when the opponent was dealt damage
    let mut g = game();
    g.put(0, PHOENIX, Zone::Battlefield);
    cast(&mut g, STEAM, &[MOUNTAIN, MOUNTAIN, MOUNTAIN], vec![], "RRR");
    assert_eq!(g.players[1].life, 18);
    assert_eq!(names(&g, &g.players[0].graveyard), ["Chandra's Phoenix", "Steam Blast"]);
    cite!(["603.10", "400.7"]; assert!(g.stack.is_empty(), "nothing triggered"));
}

#[test]
fn r603_2_the_source_named_after_by_is_what_dealt_the_damage() {
    // dealt damage by a creature, or by a spell to something other than an opponent, does
    // not make it trigger
    let mut g = game();
    g.put(0, PHOENIX, Zone::Graveyard);
    let bears = g.put(1, BEARS, Zone::Battlefield);
    cast(&mut g, BOLT, &[MOUNTAIN], vec![vec![Target::Object(bears)]], "R");
    cast(&mut g, BOLT, &[MOUNTAIN], vec![vec![Target::Player(0)]], "R");
    cite!(["603.2"]; assert!(g.stack.is_empty(), "nothing triggered"));
    assert_eq!(g.players[0].life, 17);
    // the opponent's red spell dealing damage to that opponent: not a spell "you control"
    g.act(0, Move::Pass).unwrap();
    mana(&mut g, 1, &[MOUNTAIN]);
    let bolt = g.put(1, BOLT, Zone::Hand);
    g.act(1, paid("R", Move::Cast { card: bolt, targets: vec![vec![Target::Player(1)]] })).unwrap();
    pass_both(&mut g);
    assert_eq!(g.players[1].life, 17);
    cite!(["603.2"]; assert!(g.stack.is_empty(), "nothing triggered"));
}

#[test]
fn r113_6m_an_ability_that_returns_its_card_from_the_graveyard_is_activated_there() {
    // "{1}{B}: Return this card from your graveyard to the battlefield tapped."
    let mut g = game();
    let on_battlefield = g.put(0, SKELETON, Zone::Battlefield);
    let skeleton = g.put(0, SKELETON, Zone::Graveyard);
    mana(&mut g, 0, &[SWAMP, SWAMP]);
    let activate = |source| paid("BB", Move::Activate { source, ability: 0, targets: vec![], paying: vec![] });
    cite!(["113.6m"]; refused("that ability functions only in the zone it moves its card out of (113.6m)"): g.act(0, activate(on_battlefield)));
    g.act(0, activate(skeleton)).unwrap();
    pass_both(&mut g);
    let returned = *g.battlefield.iter().find(|o| **o != on_battlefield && g.obj(**o).chars.name == "Reassembling Skeleton").expect("returned");
    assert!(g.obj(returned).tapped);
    assert!(g.players[0].graveyard.is_empty());
    // the opponent can't activate it from its owner's graveyard
    let mut g = game();
    let skeleton = g.put(0, SKELETON, Zone::Graveyard);
    g.act(0, Move::Pass).unwrap();
    mana(&mut g, 1, &[SWAMP, SWAMP]);
    cite!(["108.4a", "602.2"]; refused("an ability of a card outside the battlefield is activated by its owner (108.4a, 602.2)"): g.act(1, activate(skeleton)));
}

#[test]
fn r113_6m_an_ability_that_returns_its_card_triggers_at_the_beginning_of_a_step_there() {
    // "At the beginning of your upkeep, you may return this card from your graveyard to your
    // hand."
    let mut g = game();
    g.put(0, SQUEE, Zone::Graveyard);
    g.put(1, FOREST, Zone::Library);
    g.put(0, FOREST, Zone::Library);
    // to player 0's next upkeep
    while !(g.turn == 3 && g.part == Part::Upkeep && !g.stack.is_empty()) {
        match g.decision.clone() {
            Some(Decision::Attackers { player }) => g.act(player, Move::Attack(vec![])).unwrap(),
            _ => g.act(g.priority, Move::Pass).unwrap(),
        }
    }
    pass_both(&mut g);
    g.act(0, Move::Choose(Answer::Yes)).unwrap();
    cite!(["113.6m"]; assert!(names(&g, &g.players[0].hand).contains(&"Squee, Goblin Nabob".to_string())));
}
