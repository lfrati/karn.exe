//! Starting the game (CR 103), rule by rule: libraries, opening hands, mulligans, the first
//! turn. Every card is read from its printed text.

use crate::support::cite;
use game::card::CardDef;
use game::state::Decision;
use game::{Game, Move, ObjId};
use ir::Part;

const FOREST: usize = 0;
const BEARS: usize = 1;
const MOUNTAIN: usize = 2;
const GIANT: usize = 3;

fn defs() -> Vec<CardDef> {
    let card = |name, cost, types, text, pt| CardDef::new(name, cost, types, text, pt).expect(name);
    vec![
        card("Forest", "", "Basic Land — Forest", "({T}: Add {G}.)", None),
        card("Grizzly Bears", "{1}{G}", "Creature — Bear", "", Some((2, 2))),
        card("Mountain", "", "Basic Land — Mountain", "({T}: Add {R}.)", None),
        card("Hill Giant", "{3}{R}", "Creature — Giant", "", Some((3, 3))),
    ]
}

/// A deck of forty: seventeen of the land, twenty-three of the creature.
fn deck(land: usize, creature: usize) -> Vec<usize> {
    std::iter::repeat_n(land, 17).chain(std::iter::repeat_n(creature, 23)).collect()
}

fn start(players: usize, first: usize, seed: u64) -> Game {
    let decks: Vec<Vec<usize>> = (0..players).map(|p| if p % 2 == 0 { deck(FOREST, BEARS) } else { deck(MOUNTAIN, GIANT) }).collect();
    Game::start(defs(), &decks, 20, first, seed)
}

fn names(g: &Game, cards: &[ObjId]) -> Vec<String> {
    cards.iter().map(|c| g.obj(*c).chars.name.clone()).collect()
}

fn asked_mulligan(g: &Game) -> usize {
    match g.decision {
        Some(Decision::Mulligan { player }) => player,
        ref other => panic!("not asked about a mulligan: {other:?}"),
    }
}

#[test]
fn r103_3_each_players_deck_is_shuffled_and_becomes_their_library() {
    let g = start(2, 0, 7);
    for p in 0..2 {
        let cards: Vec<ObjId> = g.players[p].library.iter().chain(&g.players[p].hand).copied().collect();
        assert_eq!(cards.len(), 40);
        let lands = names(&g, &cards).iter().filter(|n| *n == "Forest" || *n == "Mountain").count();
        cite!(["103.3"]; assert_eq!(lands, 17, "the cards of the deck, no more and no fewer"));
    }
    // a random order: not the order the deck was listed in, the same for the same seed, and
    // another for another
    let library = |g: &Game| names(g, &g.players[0].library);
    let listed: Vec<String> = deck(FOREST, BEARS).iter().map(|d| defs()[*d].name().to_string()).collect();
    cite!(["103.3"]; assert_ne!(library(&g), listed[..33]));
    assert_eq!(library(&g), library(&start(2, 0, 7)));
    assert_ne!(library(&g), library(&start(2, 0, 8)));
}

#[test]
fn r103_4_each_player_begins_with_the_starting_life_total() {
    let g = start(2, 0, 1);
    cite!(["103.4"]; assert_eq!((g.players[0].life, g.players[1].life), (20, 20)));
}

#[test]
fn r103_5_each_player_draws_seven_and_declares_in_turn_order_from_the_starting_player() {
    let mut g = start(2, 1, 3);
    cite!(["103.5"]; assert_eq!((g.players[0].hand.len(), g.players[1].hand.len()), (7, 7)));
    assert!(!g.begun());
    cite!(["103.5"]; assert_eq!(asked_mulligan(&g), 1, "the starting player declares first"));
    cite!(["103.5"]; refused("that decision is another player's"): g.act(0, Move::Keep));
    crate::support::refused(g.act(1, Move::Pass), "a decision is pending: whether to take a mulligan (103.5)");
    g.act(1, Move::Keep).unwrap();
    cite!(["103.5"]; assert_eq!(asked_mulligan(&g), 0));
    g.act(0, Move::Keep).unwrap();
    assert!(g.begun());
    assert_eq!((g.players[0].hand.len(), g.players[1].hand.len()), (7, 7));
}

#[test]
fn r103_5_a_mulligan_is_a_new_hand_of_seven_less_one_card_for_each_taken() {
    let mut g = start(2, 0, 5);
    g.act(0, Move::Mulligan).unwrap();
    // all players declare before any mulligan is taken
    cite!(["103.5"]; assert_eq!(g.players[0].hand.len(), 7));
    assert_eq!(asked_mulligan(&g), 1);
    g.act(1, Move::Keep).unwrap();
    // a new hand of seven, then one card to the bottom of the library
    cite!(["103.5"]; assert_eq!(g.decision, Some(Decision::Bottom { player: 0, count: 1 })));
    assert_eq!((g.players[0].hand.len(), g.players[0].library.len()), (7, 33));
    let hand = g.players[0].hand.clone();
    cite!(["103.5"]; refused("put exactly 1 card of your hand on the bottom of your library (103.5)"): g.act(0, Move::Bottom(vec![])));
    cite!(["103.5"]; refused("put exactly 1 card of your hand on the bottom of your library (103.5)"): g.act(0, Move::Bottom(vec![hand[0], hand[0]])));
    let was = g.obj(hand[2]).chars.name.clone();
    g.act(0, Move::Bottom(vec![hand[2]])).unwrap();
    assert_eq!((g.players[0].hand.len(), g.players[0].library.len()), (6, 34));
    cite!(["103.5"]; assert_eq!(g.obj(g.players[0].library[0]).chars.name, was, "on the bottom"));
    // the player who kept is not asked again; the other may take another
    cite!(["103.5"]; assert_eq!(asked_mulligan(&g), 0));
    g.act(0, Move::Mulligan).unwrap();
    cite!(["103.5"]; assert_eq!(g.decision, Some(Decision::Bottom { player: 0, count: 2 })));
    let hand = g.players[0].hand.clone();
    g.act(0, Move::Bottom(vec![hand[0], hand[1]])).unwrap();
    assert_eq!(g.players[0].hand.len(), 5);
    g.act(0, Move::Keep).unwrap();
    assert!(g.begun());
    assert_eq!((g.players[0].hand.len(), g.players[1].hand.len()), (5, 7));
    assert_eq!(g.players[0].hand.len() + g.players[0].library.len(), 40);
}

#[test]
fn r103_5_no_mulligan_once_the_opening_hand_would_be_zero_cards() {
    let mut g = start(2, 0, 9);
    g.act(0, Move::Keep).unwrap();
    for taken in 1..=7 {
        assert_eq!(asked_mulligan(&g), 1);
        g.act(1, Move::Mulligan).unwrap();
        assert_eq!(g.decision, Some(Decision::Bottom { player: 1, count: taken }));
        let hand = g.players[1].hand.clone();
        g.act(1, Move::Bottom(hand[..taken].to_vec())).unwrap();
    }
    // seven taken: an opening hand of no cards, and no eighth is offered
    cite!(["103.5"]; assert!(g.begun()));
    assert_eq!((g.players[1].hand.len(), g.players[1].library.len()), (0, 40));
}

#[test]
fn r103_5c_in_a_multiplayer_game_the_first_mulligan_does_not_count() {
    let mut g = start(3, 0, 11);
    g.act(0, Move::Mulligan).unwrap();
    g.act(1, Move::Keep).unwrap();
    g.act(2, Move::Keep).unwrap();
    // seven new cards and none to the bottom
    cite!(["103.5c"]; assert_eq!(asked_mulligan(&g), 0));
    cite!(["103.5c"]; assert_eq!(g.players[0].hand.len(), 7));
    g.act(0, Move::Mulligan).unwrap();
    cite!(["103.5c"]; assert_eq!(g.decision, Some(Decision::Bottom { player: 0, count: 1 })));
}

#[test]
fn r103_8_the_starting_player_takes_the_first_turn() {
    let mut g = start(2, 1, 13);
    g.act(1, Move::Keep).unwrap();
    g.act(0, Move::Keep).unwrap();
    cite!(["103.8"]; assert_eq!((g.turn, g.active, g.priority), (1, 1, 1)));
    cite!(["502.4"]; assert_eq!(g.part, Part::Upkeep, "the untap step has no priority (502.4)"));
}

#[test]
fn r103_8a_in_a_two_player_game_the_player_who_plays_first_skips_their_first_draw_step() {
    let mut g = start(2, 0, 17);
    g.act(0, Move::Keep).unwrap();
    g.act(1, Move::Keep).unwrap();
    g.act(0, Move::Pass).unwrap();
    g.act(1, Move::Pass).unwrap();
    assert_eq!(g.part, Part::PrecombatMain);
    cite!(["103.8a"]; assert_eq!(g.players[0].hand.len(), 7, "no card drawn"));
    // the second player draws in theirs
    for _ in 0..60 {
        if g.active == 1 && g.part == Part::PrecombatMain {
            break;
        }
        match g.decision {
            Some(Decision::Attackers { .. }) => g.act(0, Move::Attack(vec![])).unwrap(),
            Some(Decision::Discard { .. }) => panic!("seven cards are not too many"),
            _ => g.act(g.priority, Move::Pass).unwrap(),
        }
    }
    cite!(["103.8a"]; assert_eq!((g.active, g.players[1].hand.len()), (1, 8)));
}

#[test]
fn r103_8c_each_multiplayer_seat_draws_on_its_first_turn_from_any_starting_seat() {
    for players in [3, 4] {
        for first in 0..players {
            let mut g = start(players, first, 23);
            for offset in 0..players { g.act((first + offset) % players, Move::Keep).unwrap(); }
            let mut seen = vec![false; players];
            for _ in 0..400 {
                if g.part == Part::PrecombatMain && !seen[g.active] {
                    cite!(["103.8c"]; case: format!("players={players}, first={first}, active={}", g.active); assert_eq!(g.players[g.active].hand.len(), 8, "first={first}, active={}", g.active));
                    seen[g.active] = true;
                    if seen.iter().all(|done| *done) { break; }
                }
                match g.decision.clone() {
                    Some(Decision::Attackers { player }) => g.act(player, Move::Attack(vec![])).unwrap(),
                    Some(Decision::Discard { player, count }) => {
                        let cards = g.players[player].hand[..count].to_vec();
                        g.act(player, Move::Discard(cards)).unwrap();
                    }
                    None => g.act(g.priority, Move::Pass).unwrap(),
                    other => panic!("unexpected decision: {other:?}"),
                }
            }
            assert!(seen.iter().all(|done| *done), "each initial seat took its first turn");
        }
    }
}

#[test]
fn r103_5c_four_players_keep_independent_mulligan_histories_and_replay_their_choices() {
    let mut g = start(4, 2, 29);
    for (player, choice) in [(2, Move::Mulligan), (3, Move::Keep), (0, Move::Mulligan), (1, Move::Mulligan)] {
        assert_eq!(asked_mulligan(&g), player);
        g.act(player, choice).unwrap();
    }
    cite!(["103.5"]; assert!(g.players.iter().all(|p| p.hand.len() == 7)));
    let mut replay = g.clone();
    for game in [&mut g, &mut replay] {
        game.act(2, Move::Keep).unwrap();
        game.act(0, Move::Mulligan).unwrap();
        game.act(1, Move::Mulligan).unwrap();
        for player in [0, 1] {
            cite!(["103.5c"]; case: format!("player {player}"); assert_eq!(game.decision, Some(Decision::Bottom { player, count: 1 })));
            let before = format!("{game:?}");
            let wrong = game.players[(player + 1) % 4].hand[0];
            cite!(["103.5"]; refused("put exactly 1 card of your hand on the bottom of your library (103.5)"): game.act(player, Move::Bottom(vec![wrong])));
            assert_eq!(format!("{game:?}"), before, "invalid answers are atomic");
            let card = game.players[player].hand[0];
            for viewer in 0..4 { assert_eq!(game.sees(viewer, card), viewer == player); }
            game.act(player, Move::Bottom(vec![card])).unwrap();
        }
        cite!(["103.5"]; assert_eq!(asked_mulligan(game), 0, "players who kept are not asked again"));
        game.act(0, Move::Keep).unwrap();
        game.act(1, Move::Keep).unwrap();
        assert!(game.begun());
        cite!(["103.5c"]; assert_eq!(game.players.iter().map(|p| p.hand.len()).collect::<Vec<_>>(), [6, 6, 7, 7]));
        for p in &game.players { assert_eq!(p.library.len() + p.hand.len(), 40); }
    }
    assert_eq!(format!("{g:?}"), format!("{replay:?}"));
}

/// What one seed orders, pinned: a change that orders it otherwise is another random-order
/// identity (`game::start::RANDOM`), and a seed recorded with the old one no longer names the
/// same game.
#[test]
fn r103_3_a_seed_orders_a_deck_as_the_named_random_identity_does() {
    assert_eq!(game::start::RANDOM, "splitmix64-fisher-yates/1");
    let g = start(2, 0, 21);
    let order = |p: usize| -> String {
        let cards: Vec<ObjId> = g.players[p].hand.iter().chain(&g.players[p].library).copied().collect();
        names(&g, &cards).iter().map(|n| &n[..1]).collect()
    };
    // G(rizzly Bears)/F(orest) and H(ill Giant)/M(ountain): hand as drawn, then the library from the top
    assert_eq!(order(0), "GGGGFFGGGGFGGGFGGFGGFFFFGGGFGFGFFGFFFGGF");
    assert_eq!(order(1), "HMHHMHHMHMHMMHHHMHHMHMHHMMMHMMMHHHHHMHMH");
}
