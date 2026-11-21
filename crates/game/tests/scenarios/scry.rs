//! Scry and surveil (CR 701.22, 701.25): a player looks at the top cards of their library and
//! puts each back on top, in any order, or away. Every card is read from its printed text.

use crate::support::{cite, paid};
use game::card::CardDef;
use game::play::check;
use game::state::{Answer, Choice, Decision};
use game::{Event, Game, Move, ObjId, Zone};
use ir::Verb;

const MOUNTAIN: usize = 0;
const ISLAND: usize = 1;
const BEARS: usize = 2;
const SHOCK: usize = 3;
const JET: usize = 4;
const TEMPLE: usize = 5;
const CONSIDER: usize = 6;
const ADEPT: usize = 7;
const SPYBUG: usize = 8;
const OPT: usize = 9;

fn defs() -> Vec<CardDef> {
    let card = |name, cost, types, text, pt| CardDef::new(name, cost, types, text, pt).expect(name);
    vec![
        card("Mountain", "", "Basic Land — Mountain", "({T}: Add {R}.)", None),
        card("Island", "", "Basic Land — Island", "({T}: Add {U}.)", None),
        card("Grizzly Bears", "{1}{G}", "Creature — Bear", "", Some((2, 2))),
        card("Shock", "{R}", "Instant", "Shock deals 2 damage to any target.", None),
        card("Magma Jet", "{1}{R}", "Instant", "Magma Jet deals 2 damage to any target. Scry 2.", None),
        card(
            "Temple of Triumph",
            "",
            "Land",
            "This land enters tapped.\nWhen this land enters, scry 1. (Look at the top card of your library. You may put that card on the bottom.)\n{T}: Add {R} or {W}.",
            None,
        ),
        card("Consider", "{U}", "Instant", "Surveil 1. (Look at the top card of your library. You may put it into your graveyard.)\nDraw a card.", None),
        card("Flamespeaker Adept", "{2}{R}", "Creature — Human Shaman", "Whenever you scry, this creature gets +2/+0 and gains first strike until end of turn.", Some((2, 3))),
        card(
            "Dimir Spybug",
            "{U}{B}",
            "Creature — Insect",
            "Flying\nMenace (This creature can't be blocked except by two or more creatures.)\nWhenever you surveil, put a +1/+1 counter on this creature.",
            Some((1, 1)),
        ),
        card("Opt", "{U}", "Instant", "Scry 1. (Look at the top card of your library. You may put that card on the bottom.)\nDraw a card.", None),
    ]
}

/// A game in which player 0's library is these cards, the last of them on top.
fn game(library: &[usize]) -> Game {
    let mut g = Game::new(defs(), 2, 20);
    for def in library {
        g.put(0, *def, Zone::Library);
    }
    g
}

/// Player 0 casts this card with the mana of these lands, with these targets; both players
/// pass.
fn cast(g: &mut Game, def: usize, lands: &[usize], targets: Vec<Vec<game::Target>>, spending: &str) {
    for land in lands {
        let land = g.put(0, *land, Zone::Battlefield);
        g.act(0, Move::Activate { source: land, ability: 0, targets: vec![], paying: vec![] }).unwrap();
    }
    let card = g.put(0, def, Zone::Hand);
    g.act(0, paid(spending, Move::Cast { card, targets })).unwrap();
    g.act(0, Move::Pass).unwrap();
    g.act(1, Move::Pass).unwrap();
}

/// The cards of player 0's library by name, the top card first.
fn library(g: &Game) -> Vec<String> {
    g.players[0].library.iter().rev().map(|c| g.obj(*c).chars.name.clone()).collect()
}

/// What the game asks player 0 to put back: the cards looked at, the top card first.
fn looked_at(g: &Game) -> (Verb, Vec<ObjId>) {
    match &g.decision {
        Some(Decision::Choose { player: 0, choice: Choice::Arrange { verb, cards } }) => (*verb, cards.clone()),
        other => panic!("no scry or surveil asked: {other:?}"),
    }
}

#[test]
fn every_card_of_the_tests_is_one_the_engine_runs() {
    for d in defs() {
        check(&d).unwrap_or_else(|u| panic!("{}: {}", d.name(), u.0));
    }
}

#[test]
fn r701_22a_to_scry_is_to_look_at_the_top_cards_and_put_any_on_the_bottom_in_any_order() {
    // "Magma Jet deals 2 damage to any target. Scry 2.": the top two cards, the Shock on top
    let mut g = game(&[ISLAND, MOUNTAIN, BEARS, SHOCK]);
    cast(&mut g, JET, &[MOUNTAIN, MOUNTAIN], vec![vec![game::Target::Player(1)]], "RR");
    assert_eq!(g.players[1].life, 18);
    let (verb, cards) = looked_at(&g);
    assert_eq!(verb, Verb::Scry);
    let [shock, bears] = cards[..] else { panic!("two cards: {cards:?}") };
    assert_eq!(g.obj(shock).chars.name, "Shock");
    // the player who looks sees them; the other player does not (401.2)
    cite!(["701.20e"]; assert!(g.sees(0, shock) && !g.sees(1, shock)));
    // each of them once, on top or on the bottom
    let wrong = Answer::Arrange { top: vec![shock], away: vec![] };
    cite!(["701.22a"]; refused("put each of the cards looked at back on top or away, once (701.22a)"): g.act(0, Move::Choose(wrong)));
    let wrong = Answer::Arrange { top: vec![shock, shock], away: vec![] };
    cite!(["701.22a"]; refused("put each of the cards looked at back on top or away, once (701.22a)"): g.act(0, Move::Choose(wrong)));
    // the Bears back on top, the Shock to the bottom
    g.act(0, Move::Choose(Answer::Arrange { top: vec![bears], away: vec![shock] })).unwrap();
    cite!(["701.22a"]; assert_eq!(library(&g), ["Grizzly Bears", "Mountain", "Island", "Shock"]));
    // they stay in the library, the same objects (400.7 is of zone changes)
    assert_eq!(g.obj(bears).zone, Zone::Library);
    assert!(g.log.iter().any(|e| *e == Event::Arranged { player: 0, verb: Verb::Scry }));
    assert_eq!(g.obj(g.players[0].graveyard[0]).chars.name, "Magma Jet");

    // both on top, in the other order; then both on the bottom, the last the bottom card
    let mut g = game(&[ISLAND, BEARS, SHOCK]);
    cast(&mut g, JET, &[MOUNTAIN, MOUNTAIN], vec![vec![game::Target::Player(1)]], "RR");
    let (_, cards) = looked_at(&g);
    g.act(0, Move::Choose(Answer::Arrange { top: vec![cards[1], cards[0]], away: vec![] })).unwrap();
    cite!(["701.22a"]; assert_eq!(library(&g), ["Grizzly Bears", "Shock", "Island"]));
    let mut g = game(&[ISLAND, BEARS, SHOCK]);
    cast(&mut g, JET, &[MOUNTAIN, MOUNTAIN], vec![vec![game::Target::Player(1)]], "RR");
    let (_, cards) = looked_at(&g);
    g.act(0, Move::Choose(Answer::Arrange { top: vec![], away: vec![cards[0], cards[1]] })).unwrap();
    cite!(["701.22a"]; assert_eq!(library(&g), ["Island", "Shock", "Grizzly Bears"]));

    // with fewer cards in the library than that, those there are; with none, nothing to look
    // at, but still a scry event, and the card is then drawn from an empty library
    let mut g = game(&[BEARS]);
    cast(&mut g, JET, &[MOUNTAIN, MOUNTAIN], vec![vec![game::Target::Player(1)]], "RR");
    cite!(["609.3"]; assert_eq!(looked_at(&g).1.len(), 1));
    let mut g = game(&[]);
    cast(&mut g, OPT, &[ISLAND], vec![], "U");
    cite!(["609.3"]; assert!(g.decision.is_none()));
    cite!(["701.22d"]; assert!(g.log.iter().any(|e| *e == Event::Arranged { player: 0, verb: Verb::Scry }), "701.22b exempts scry 0, not a positive scry with an empty library"));
}

#[test]
fn r701_22a_a_land_that_scries_as_it_enters() {
    // "This land enters tapped. When this land enters, scry 1.": the ability triggers and
    // goes on the stack; the top card may go to the bottom
    let mut g = game(&[BEARS, SHOCK]);
    let temple = g.put(0, TEMPLE, Zone::Hand);
    g.act(0, Move::PlayLand(temple)).unwrap();
    let temple = *g.battlefield.last().unwrap();
    assert!(g.obj(temple).tapped);
    assert_eq!(g.stack.len(), 1);
    g.act(0, Move::Pass).unwrap();
    g.act(1, Move::Pass).unwrap();
    let (_, cards) = looked_at(&g);
    assert_eq!(cards.len(), 1);
    g.act(0, Move::Choose(Answer::Arrange { top: vec![], away: cards })).unwrap();
    cite!(["701.22a"]; assert_eq!(library(&g), ["Grizzly Bears", "Shock"]));
}

#[test]
fn r701_22d_an_ability_that_triggers_whenever_a_player_scries_triggers_once_it_is_done() {
    // "Whenever you scry, this creature gets +2/+0 and gains first strike until end of turn."
    let mut g = game(&[BEARS, SHOCK]);
    let adept = g.put(0, ADEPT, Zone::Battlefield);
    cast(&mut g, OPT, &[ISLAND], vec![], "U");
    // nothing has triggered while the player decides
    cite!(["701.22d"]; assert!(g.stack.is_empty() || g.obj(*g.stack.last().unwrap()).ability.is_none()));
    let (_, cards) = looked_at(&g);
    g.act(0, Move::Choose(Answer::Arrange { top: cards, away: vec![] })).unwrap();
    // Opt has drawn the Shock and gone; the ability is on the stack
    assert_eq!(g.obj(g.players[0].hand[0]).chars.name, "Shock");
    cite!(["701.22d"]; assert_eq!(g.stack.len(), 1));
    g.act(0, Move::Pass).unwrap();
    g.act(1, Move::Pass).unwrap();
    cite!(["701.22d"]; assert_eq!(g.power(adept), Some(4)));
    assert!(g.has(adept, "First strike"));
}

#[test]
fn r701_25a_to_surveil_is_to_look_at_the_top_cards_and_put_any_into_the_graveyard() {
    // "Surveil 1. Draw a card.": the Shock into the graveyard, then the Bears drawn
    let mut g = game(&[ISLAND, BEARS, SHOCK]);
    cast(&mut g, CONSIDER, &[ISLAND], vec![], "U");
    let (verb, cards) = looked_at(&g);
    assert_eq!(verb, Verb::Surveil);
    g.act(0, Move::Choose(Answer::Arrange { top: vec![], away: cards })).unwrap();
    let names = |cards: &[ObjId]| cards.iter().map(|c| g.obj(*c).chars.name.clone()).collect::<Vec<_>>();
    cite!(["701.25a"]; assert_eq!(names(&g.players[0].graveyard), ["Shock", "Consider"]));
    assert_eq!(names(&g.players[0].hand), ["Grizzly Bears"]);
    cite!(["701.25a"]; assert_eq!(library(&g), ["Island"]));
}

#[test]
fn r701_25d_an_ability_that_triggers_whenever_a_player_surveils() {
    // "Whenever you surveil, put a +1/+1 counter on this creature.": also when the card is
    // put back on top
    let mut g = game(&[BEARS, SHOCK]);
    let spybug = g.put(0, SPYBUG, Zone::Battlefield);
    cast(&mut g, CONSIDER, &[ISLAND], vec![], "U");
    let (_, cards) = looked_at(&g);
    g.act(0, Move::Choose(Answer::Arrange { top: cards, away: vec![] })).unwrap();
    g.act(0, Move::Pass).unwrap();
    g.act(1, Move::Pass).unwrap();
    cite!(["701.25d"]; assert_eq!(g.power(spybug), Some(2)));
    assert!(g.players[0].graveyard.iter().all(|c| g.obj(*c).chars.name == "Consider"));
}
