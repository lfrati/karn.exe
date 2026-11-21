//! What a player sees (CR 400.2): public zones, their own hand, and no library. Every card is
//! read from its printed text.

use crate::support::{cite, paid};
use game::card::CardDef;
use game::play::check;
use game::state::{Choice, Decision};
use game::{Game, Move, Zone};

const FOREST: usize = 0;
const SWAMP: usize = 1;
const BEARS: usize = 2;
const SALVAGE: usize = 3;
const UNSUMMON: usize = 4;
const ISLAND: usize = 5;

fn defs() -> Vec<CardDef> {
    let card = |name, cost, types, text, pt| CardDef::new(name, cost, types, text, pt).expect(name);
    vec![
        card("Forest", "", "Basic Land — Forest", "({T}: Add {G}.)", None),
        card("Swamp", "", "Basic Land — Swamp", "({T}: Add {B}.)", None),
        card("Grizzly Bears", "{1}{G}", "Creature — Bear", "", Some((2, 2))),
        card(
            "Grisly Salvage",
            "{B}{G}",
            "Instant",
            "Reveal the top five cards of your library. You may put a creature or land card from among them into your hand. Put the rest into your graveyard.",
            None,
        ),
        card("Unsummon", "{U}", "Instant", "Return target creature to its owner's hand.", None),
        card("Island", "", "Basic Land — Island", "({T}: Add {U}.)", None),
    ]
}

#[test]
fn every_card_of_the_tests_is_one_the_engine_runs() {
    for d in defs() {
        check(&d).unwrap_or_else(|u| panic!("{}: {}", d.name(), u.0));
    }
}

#[test]
fn r400_2_every_player_sees_the_cards_in_the_public_zones() {
    let mut g = Game::new(defs(), 2, 20);
    let permanent = g.put(0, BEARS, Zone::Battlefield);
    let buried = g.put(0, BEARS, Zone::Graveyard);
    let exiled = g.put(0, BEARS, Zone::Exile);
    let forest = g.put(0, FOREST, Zone::Battlefield);
    g.put(0, FOREST, Zone::Battlefield);
    g.act(0, Move::Activate { source: forest, ability: 0, targets: vec![], paying: vec![] }).unwrap();
    let other = *g.battlefield.last().unwrap();
    g.act(0, Move::Activate { source: other, ability: 0, targets: vec![], paying: vec![] }).unwrap();
    let spell = g.put(0, BEARS, Zone::Hand);
    cite!(["402.3"]; assert!(!g.sees(1, spell), "in its owner's hand"));
    g.act(0, paid("GG", Move::Cast { card: spell, targets: vec![] })).unwrap();
    let [on_stack] = g.stack[..] else { panic!("not cast") };
    for viewer in [0, 1] {
        for object in [permanent, buried, exiled, on_stack] {
            cite!(["400.2"]; case: format!("viewer {viewer}, {object:?}"); assert!(g.sees(viewer, object)));
        }
    }
    // what the card was in the hand is still what only its owner saw
    cite!(["402.3", "400.7"]; assert!(g.sees(0, spell) && !g.sees(1, spell)));
}

#[test]
fn r402_3_a_hand_is_looked_at_by_its_owner_alone() {
    let mut g = Game::new(defs(), 2, 20);
    let mine = g.put(0, BEARS, Zone::Hand);
    let theirs = g.put(1, BEARS, Zone::Hand);
    cite!(["402.3"]; assert!(g.sees(0, mine) && !g.sees(1, mine)));
    cite!(["402.3"]; assert!(g.sees(1, theirs) && !g.sees(0, theirs)));
    // a permanent every player saw is returned to a hand: the card there is one only its
    // owner sees
    let bears = g.put(1, BEARS, Zone::Battlefield);
    let island = g.put(0, ISLAND, Zone::Battlefield);
    g.act(0, Move::Activate { source: island, ability: 0, targets: vec![], paying: vec![] }).unwrap();
    let unsummon = g.put(0, UNSUMMON, Zone::Hand);
    g.act(0, paid("U", Move::Cast { card: unsummon, targets: vec![vec![game::Target::Object(bears)]] })).unwrap();
    g.act(0, Move::Pass).unwrap();
    g.act(1, Move::Pass).unwrap();
    let returned = *g.players[1].hand.last().unwrap();
    cite!(["400.2"]; assert!(g.sees(0, bears), "as it was on the battlefield"));
    cite!(["402.3"]; assert!(g.sees(1, returned) && !g.sees(0, returned)));
}

#[test]
fn r401_2_no_player_looks_at_the_cards_of_a_library() {
    let mut g = Game::new(defs(), 2, 20);
    let top = g.put(0, BEARS, Zone::Library);
    cite!(["401.2"]; assert!(!g.sees(0, top) && !g.sees(1, top)));
    // drawn, it is a card of its owner's hand; what it was in the library nobody saw
    g.draw(0, 1);
    let [drawn] = g.players[0].hand[..] else { panic!("not drawn") };
    cite!(["402.3"]; assert!(g.sees(0, drawn) && !g.sees(1, drawn)));
    cite!(["401.2"]; assert!(!g.sees(0, top) && !g.sees(1, top)));
}

#[test]
fn r701_20a_a_revealed_card_is_shown_to_all_players_while_the_effect_needs_it() {
    let mut g = Game::new(defs(), 2, 20);
    for def in [FOREST, BEARS, SWAMP, FOREST, BEARS, BEARS] {
        g.put(0, def, Zone::Library);
    }
    let library = g.players[0].library.clone();
    for land in [SWAMP, FOREST] {
        let land = g.put(0, land, Zone::Battlefield);
        g.act(0, Move::Activate { source: land, ability: 0, targets: vec![], paying: vec![] }).unwrap();
    }
    let salvage = g.put(0, SALVAGE, Zone::Hand);
    g.act(0, paid("BG", Move::Cast { card: salvage, targets: vec![] })).unwrap();
    g.act(0, Move::Pass).unwrap();
    g.act(1, Move::Pass).unwrap();
    // "Reveal the top five cards of your library. You may put …": the effect waits for the
    // choice with the five revealed; the sixth card from the top is not
    assert_eq!(g.decision, Some(Decision::Choose { player: 0, choice: Choice::May }));
    for viewer in [0, 1] {
        cite!(["701.20a"]; case: viewer; assert!(library[1..].iter().all(|c| g.sees(viewer, *c))));
        cite!(["401.2"]; case: viewer; assert!(!g.sees(viewer, library[0])));
    }
}
