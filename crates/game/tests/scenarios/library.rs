//! Cards revealed from a library and where an effect then puts them (CR 701.20, 608.2c–d).
//! Every card is read from its printed text.

use crate::support::{cite, paid};
use game::card::CardDef;
use game::play::check;
use game::state::{Answer, Choice, Decision};
use game::{Event, Game, Move, ObjId, Zone};
use ir::Part;

const FOREST: usize = 0;
const SWAMP: usize = 1;
const BEARS: usize = 2;
const SHOCK: usize = 3;
const MULCH: usize = 4;
const SALVAGE: usize = 5;
const SEER: usize = 6;

fn defs() -> Vec<CardDef> {
    let card = |name, cost, types, text, pt| CardDef::new(name, cost, types, text, pt).expect(name);
    vec![
        card("Forest", "", "Basic Land — Forest", "({T}: Add {G}.)", None),
        card("Swamp", "", "Basic Land — Swamp", "({T}: Add {B}.)", None),
        card("Grizzly Bears", "{1}{G}", "Creature — Bear", "", Some((2, 2))),
        card("Shock", "{R}", "Instant", "Shock deals 2 damage to any target.", None),
        card(
            "Mulch",
            "{1}{G}",
            "Sorcery",
            "Reveal the top four cards of your library. Put all land cards revealed this way into your hand and the rest into your graveyard.",
            None,
        ),
        card(
            "Grisly Salvage",
            "{B}{G}",
            "Instant",
            "Reveal the top five cards of your library. You may put a creature or land card from among them into your hand. Put the rest into your graveyard.",
            None,
        ),
        card(
            "Pain Seer",
            "{1}{B}",
            "Creature — Human Wizard",
            "Inspired — Whenever this creature becomes untapped, reveal the top card of your library and put that card into your hand. You lose life equal to that card's mana value.",
            Some((2, 2)),
        ),
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

/// Player 0 casts this card with the mana of these lands; both players pass.
fn cast(g: &mut Game, def: usize, lands: &[usize], spending: &str) {
    for land in lands {
        let land = g.put(0, *land, Zone::Battlefield);
        g.act(0, Move::Activate { source: land, ability: 0, targets: vec![], paying: vec![] }).unwrap();
    }
    let card = g.put(0, def, Zone::Hand);
    g.act(0, paid(spending, Move::Cast { card, targets: vec![] })).unwrap();
    g.act(0, Move::Pass).unwrap();
    g.act(1, Move::Pass).unwrap();
}

fn names(g: &Game, cards: &[ObjId]) -> Vec<String> {
    let mut names: Vec<String> = cards.iter().map(|c| g.obj(*c).chars.name.clone()).collect();
    names.sort();
    names
}

#[test]
fn r701_20d_reveal_does_not_follow_a_card_that_became_a_new_object_in_a_shuffle() {
    let mut g = game(&[FOREST]);
    let old = g.players[0].library[0];
    let def = g.defs.len();
    std::sync::Arc::make_mut(&mut g.defs).push(CardDef::new("Hidden", "{0}", "Instant", "Reveal the top card of your library. Shuffle. Reveal that card.", None).unwrap());
    cast(&mut g, def, &[], "");
    let reveals: Vec<_> = g.log.iter().filter_map(|e| match e { Event::Revealed { cards, .. } => Some(cards.clone()), _ => None }).collect();
    cite!(["701.20d"]; assert_eq!(reveals, vec![vec![old]], "the disappeared object cannot be revealed again"));
    let now = g.players[0].library[0];
    cite!(["701.20d"]; assert_ne!(now, old));
    assert!(!g.sees(0, now) && !g.sees(1, now));
}

#[test]
fn r701_20c_a_currently_revealed_card_can_be_revealed_again() {
    let mut g = game(&[FOREST]);
    let card = g.players[0].library[0];
    let def = g.defs.len();
    std::sync::Arc::make_mut(&mut g.defs).push(CardDef::new("Show", "{0}", "Instant", "Reveal the top card of your library. Reveal that card.", None).unwrap());
    cast(&mut g, def, &[], "");
    let reveals: Vec<_> = g.log.iter().filter_map(|e| match e { Event::Revealed { cards, .. } => Some(cards.clone()), _ => None }).collect();
    cite!(["701.20c"]; assert_eq!(reveals, vec![vec![card], vec![card]]));
    cite!(["701.20b"]; assert_eq!(g.players[0].library, vec![card]));
}

#[test]
fn every_card_of_the_tests_is_one_the_engine_runs() {
    for d in defs() {
        check(&d).unwrap_or_else(|u| panic!("{}: {}", d.name(), u.0));
    }
}

#[test]
fn r701_20b_revealing_a_card_does_not_move_it_and_the_effect_then_says_where_each_goes() {
    // "Reveal the top four cards of your library. Put all land cards revealed this way into
    // your hand and the rest into your graveyard.": the fifth card from the top is not one
    let mut g = game(&[SWAMP, BEARS, FOREST, SHOCK, FOREST]);
    let incoming = vec![g.players[0].library[1], g.players[0].library[3]];
    cast(&mut g, MULCH, &[FOREST, FOREST], "GG");
    cite!(["404.3"]; ok: g.act(0, Move::Choose(Answer::Objects(incoming))));
    let revealed = g.log.iter().find_map(|e| if let Event::Revealed { player: 0, cards } = e { Some(cards.len()) } else { None });
    assert_eq!(revealed, Some(4));
    cite!(["608.2c"]; assert_eq!(names(&g, &g.players[0].hand), ["Forest", "Forest"]));
    cite!(["608.2c"]; assert_eq!(names(&g, &g.players[0].graveyard), ["Grizzly Bears", "Mulch", "Shock"]));
    assert_eq!(names(&g, &g.players[0].library), ["Swamp"]);
    // a library with fewer cards than that: as many as there are
    let mut g = game(&[BEARS, FOREST]);
    cast(&mut g, MULCH, &[FOREST, FOREST], "GG");
    assert_eq!(names(&g, &g.players[0].hand), ["Forest"]);
    assert!(g.players[0].library.is_empty());
}

#[test]
fn r608_2c_the_card_an_instruction_revealed_is_what_that_card_then_is() {
    // "Whenever this creature becomes untapped, reveal the top card of your library and put
    // that card into your hand. You lose life equal to that card's mana value.": the card
    // revealed, and not the creature the trigger condition named
    let mut g = game(&[FOREST, FOREST, FOREST, SHOCK]);
    for _ in 0..4 {
        g.put(1, FOREST, Zone::Library);
    }
    let seer = g.put(0, SEER, Zone::Battlefield);
    g.set_tapped(seer, true);
    // its controller's next untap step, in the third turn
    while !(g.turn == 3 && g.part == Part::Upkeep) {
        match g.decision.clone() {
            Some(Decision::Attackers { player }) => g.act(player, Move::Attack(vec![])).unwrap(),
            _ => g.act(g.priority, Move::Pass).unwrap(),
        }
    }
    assert!(!g.obj(seer).tapped);
    assert!(g.players[0].hand.is_empty());
    g.act(0, Move::Pass).unwrap();
    g.act(1, Move::Pass).unwrap();
    cite!(["608.2c"]; assert_eq!(names(&g, &g.players[0].hand), ["Shock"]));
    cite!(["608.2c"]; assert_eq!(g.players[0].life, 19));
    assert_eq!(g.obj(seer).zone, Zone::Battlefield);
    assert!(g.log.iter().any(|e| matches!(e, Event::Revealed { player: 0, cards } if cards.len() == 1)));
}

#[test]
fn r608_2d_a_card_is_chosen_from_among_those_revealed() {
    // "You may put a creature or land card from among them into your hand. Put the rest into
    // your graveyard.": yes or no, then which of the cards that are one
    let library = [FOREST, SHOCK, BEARS, SWAMP, SHOCK, BEARS];
    let mut g = game(&library);
    let original = g.players[0].library.clone();
    cast(&mut g, SALVAGE, &[SWAMP, FOREST], "BG");
    assert_eq!(g.decision, Some(Decision::Choose { player: 0, choice: Choice::May { possible: true } }));
    g.act(0, Move::Choose(Answer::Yes)).unwrap();
    let Some(Decision::Choose { player: 0, choice: Choice::Objects { among, count: 1, .. } }) = g.decision.clone() else {
        panic!("not asked which card: {:?}", g.decision)
    };
    cite!(["608.2d"]; assert_eq!(names(&g, &among), ["Grizzly Bears", "Grizzly Bears", "Swamp"], "the Shocks are neither, the sixth card was not revealed"));
    let swamp = *among.iter().find(|c| g.obj(**c).chars.name == "Swamp").unwrap();
    g.act(0, Move::Choose(Answer::Objects(vec![swamp]))).unwrap();
    cite!(["404.3"]; ok: g.act(0, Move::Choose(Answer::Objects(vec![original[1], original[2], original[4], original[5]]))));
    assert_eq!(names(&g, &g.players[0].hand), ["Swamp"]);
    assert_eq!(names(&g, &g.players[0].graveyard), ["Grisly Salvage", "Grizzly Bears", "Grizzly Bears", "Shock", "Shock"]);
    assert_eq!(names(&g, &g.players[0].library), ["Forest"]);
    // declined: all five to the graveyard
    let mut g = game(&library);
    let incoming = g.players[0].library[1..].to_vec();
    cast(&mut g, SALVAGE, &[SWAMP, FOREST], "BG");
    g.act(0, Move::Choose(Answer::No)).unwrap();
    cite!(["404.3"]; ok: g.act(0, Move::Choose(Answer::Objects(incoming))));
    assert!(g.players[0].hand.is_empty());
    assert_eq!(g.players[0].graveyard.len(), 6);
    // none of them is one: nothing is offered (an impossible option can't be chosen)
    let mut g = game(&[SHOCK, SHOCK, SHOCK, SHOCK, SHOCK]);
    let incoming = g.players[0].library.clone();
    cast(&mut g, SALVAGE, &[SWAMP, FOREST], "BG");
    cite!(["404.3"]; ok: g.act(0, Move::Choose(Answer::Objects(incoming))));
    cite!(["608.2d"]; assert!(g.decision.is_none()));
    assert_eq!(g.players[0].graveyard.len(), 6);
}
