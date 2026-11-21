//! A game started from a record of one played elsewhere: each deck's shuffle (103.3,
//! 701.24a) has the outcome the record says, given as positions into the submitted deck list,
//! top first. Each player then draws seven from the top (103.5). Outcomes of later shuffles,
//! a mulligan's among them (103.5), are matched to the player who shuffles, whatever their
//! place in the list a response is given: players who take a mulligan in the same round
//! shuffle in turn order.
use crate::support::cite;
use game::simulation::Simulation;
use game::start::{Initial, ShuffleOrder};
use game::{CardDef, Game, Move, ObjId};

/// Ten cards, each named for its place in the deck list ("Card 3" at position 3).
fn defs() -> Vec<CardDef> {
    (0..10).map(|i| CardDef::new(&format!("Card {i}"), "", "Land", "", None).unwrap()).collect()
}

fn names(g: &Game, ids: impl IntoIterator<Item = ObjId>) -> Vec<String> {
    ids.into_iter().map(|id| g.obj(id).chars.name.clone()).collect()
}

fn named(positions: &[usize]) -> Vec<String> { positions.iter().map(|i| format!("Card {i}")).collect() }

/// The library top first.
fn library(g: &Game, player: usize) -> Vec<ObjId> { g.players[player].library.iter().rev().copied().collect() }

fn started(orders: &[Vec<usize>]) -> Result<Game, game::start::Refused> {
    let decks = vec![(0..10).collect::<Vec<_>>(); 2];
    Game::start_from(defs(), &decks, &[None, None], 20, 0, Initial::Orders(orders))
}

#[test]
fn r103_3_each_library_is_in_the_recorded_order_and_the_hand_is_its_top_seven() {
    let orders = vec![vec![9, 8, 7, 6, 5, 4, 3, 2, 1, 0], vec![3, 1, 4, 0, 5, 9, 2, 6, 8, 7]];
    let g = started(&orders).unwrap();
    for (player, order) in orders.iter().enumerate() {
        // 103.5: seven cards are drawn from the top; the rest stay in the recorded order
        cite!(["103.3", "701.24a"]; assert_eq!(names(&g, library(&g, player)), named(&order[7..])));
        let mut hand = names(&g, g.players[player].hand.iter().copied());
        let mut top = named(&order[..7]);
        hand.sort(); top.sort();
        cite!(["103.5"]; assert_eq!(hand, top));
    }
    cite!(["103.8"]; assert!(!g.begun(), "the mulligans precede the first turn"));
}

#[test]
fn r103_3_an_order_that_is_not_a_permutation_of_the_deck_is_refused() {
    let whole: Vec<usize> = (0..10).collect();
    // a record that says no outcome of these shuffles describes no game: malformed setup input
    let refused = |orders: Vec<Vec<usize>>| {
        let refused = started(&orders).map(|_| ()).unwrap_err();
        assert_eq!(refused.class, game::simulation::Class::Malformed, "{refused}");
        refused.why
    };
    assert_eq!(refused(vec![whole.clone()]), "orders gives one library order per player");
    assert!(refused(vec![whole.clone(), vec![0; 10]]).contains("not a permutation"));
    assert!(refused(vec![whole.clone(), (0..9).collect()]).contains("not a permutation"));
    assert!(refused(vec![whole.clone(), (1..11).collect()]).contains("not a permutation"));
}

#[test]
fn r103_5_mulligan_shuffles_take_each_players_outcome_whatever_its_place_in_the_list() {
    let identity: Vec<usize> = (0..10).collect();
    let mut sim = Simulation::new(started(&[identity.clone(), identity]).unwrap());
    // the starting player declares first (103.5)
    let first = sim.request().unwrap().unwrap().clone();
    assert_eq!(first.player, 0);
    sim.respond(first.id, 0, Move::Mulligan).unwrap();
    let second = sim.request().unwrap().unwrap().clone();
    assert_eq!(second.player, 1);
    // each player's whole deck, named by its current objects, in a recorded order
    let g = sim.game().unwrap();
    let cards = |p: usize| -> Vec<ObjId> {
        let mut all: Vec<ObjId> = library(g, p);
        all.extend(g.players[p].hand.iter().copied());
        all.sort_by_key(|id| g.obj(*id).chars.name.clone());
        all
    };
    let (zero, one) = (cards(0), cards(1));
    let order0: Vec<usize> = vec![2, 4, 6, 8, 0, 1, 3, 5, 7, 9];
    let order1: Vec<usize> = vec![9, 7, 5, 3, 1, 0, 2, 4, 6, 8];
    // player 1's outcome first, though player 0 shuffles first (turn order, 103.5)
    let orders = vec![
        ShuffleOrder { player: 1, order: order1.iter().map(|i| one[*i]).collect() },
        ShuffleOrder { player: 0, order: order0.iter().map(|i| zero[*i]).collect() },
    ];
    cite!(["103.5", "701.24a"]; ok: sim.respond_with_shuffles(second.id, 1, Move::Mulligan, orders).map_err(|e| e.to_string()));
    let g = sim.game().unwrap();
    for (player, order) in [(0, &order0), (1, &order1)] {
        cite!(["103.5"]; assert_eq!(names(g, library(g, player)), named(&order[7..])));
        let mut hand = names(g, g.players[player].hand.iter().copied());
        let mut top = named(&order[..7]);
        hand.sort(); top.sort();
        cite!(["103.5"]; assert_eq!(hand, top));
    }
}
