//! What a player was shown is theirs once shown, whether or not a question follows (CR 400.2,
//! 701.20, 701.22–23): each showing is kept as it was, for the seats entitled to it, until
//! each has taken it.

use crate::support::cite;
use game::observation::{Disclosure, Kind};
use game::legal::Asked;
use game::simulation::Simulation;
use game::state::Answer;
use game::{CardDef, Game, Move, ObjId, Zone};
use std::sync::Arc;

const LOOK: usize = 0;
const REVEAL: usize = 1;
const SCRY: usize = 2;
const SEARCH: usize = 3;
const BURY: usize = 4;
const FOREST: usize = 5;
const BEAR: usize = 6;
const WOLF: usize = 7;

fn defs() -> Vec<CardDef> {
    let card = |name: &str, types: &str, text: &str, pt| {
        let def = CardDef::new(name, "{0}", types, text, pt).unwrap();
        game::play::check(&def).unwrap_or_else(|e| panic!("{name}: {e:?}"));
        def
    };
    vec![
        card("Glance", "Instant", "Look at the top two cards of your library.", None),
        card("Show", "Instant", "Reveal the top card of your library.", None),
        card("Foresee", "Instant", "Scry 2.", None),
        card("Tutor", "Sorcery", "Search your library for a card, put it into your hand, then shuffle.", None),
        card("Bury", "Instant", "Look at the top two cards of your library. Put them into your graveyard.", None),
        card("Forest", "Basic Land — Forest", "", None),
        card("Bear", "Creature — Bear", "", Some((2, 2))),
        card("Wolf", "Creature — Wolf", "", Some((3, 3))),
    ]
}

/// Player 0's library, from the bottom: Forest, Bear, Wolf (the top card).
fn game() -> (Game, [ObjId; 3]) {
    let mut g = Game::new(defs(), 2, 20);
    let library = [FOREST, BEAR, WOLF].map(|card| g.put(0, card, Zone::Library));
    g.put(1, FOREST, Zone::Library);
    (g, library)
}

fn cast(g: &mut Game, card: usize) {
    let spell = g.put(0, card, Zone::Hand);
    g.act(0, Move::Cast { card: spell, targets: vec![] }).unwrap();
    g.act(0, Move::Pass).unwrap();
    g.act(1, Move::Pass).unwrap();
}

fn names(d: &Disclosure) -> Vec<&str> {
    d.cards.iter().map(|c| c.name.as_str()).collect()
}

#[test]
fn r400_2_a_look_that_no_question_follows_is_delivered_to_the_player_who_looked_alone() {
    let (mut g, [_, bear, wolf]) = game();
    cast(&mut g, LOOK);
    // the effect is over and nothing is asked: priority again
    assert!(matches!(g.asked(), Some((0, Asked::Priority))));
    assert!(g.stack.is_empty() && g.resolving().is_none());
    let [looked] = g.observations(0) else { panic!("one look: {:?}", g.observations(0)) };
    assert_eq!((looked.kind, looked.by), (Kind::Looked, 0));
    cite!(["608.2c"]; assert_eq!(names(looked), ["Wolf", "Bear"]));
    assert_eq!(looked.cards.iter().map(|c| (c.object, c.owner, c.zone, c.card)).collect::<Vec<_>>(),
        [(wolf, 0, Zone::Library, Some(WOLF)), (bear, 0, Zone::Library, Some(BEAR))]);
    cite!(["400.2"]; assert!(g.observations(1).is_empty(), "the opponent learns nothing of a private look"));
    // what was learned is no permission to look at the library now
    assert!(!g.sees(0, wolf) && !g.sees(0, bear) && !g.sees(1, wolf));
}

#[test]
fn r701_20a_a_reveal_that_ends_before_the_next_request_is_delivered_to_every_seat() {
    let (mut g, [_, _, wolf]) = game();
    cast(&mut g, REVEAL);
    assert!(g.resolving().is_none());
    let (mine, theirs) = (g.observations(0).to_vec(), g.observations(1).to_vec());
    cite!(["701.20a"]; assert_eq!(mine.len(), 1));
    assert_eq!(mine, theirs);
    assert!(Arc::ptr_eq(&mine[0], &theirs[0]), "one showing, shared");
    cite!(["701.20a"]; assert_eq!((mine[0].kind, mine[0].by, names(&mine[0])), (Kind::Revealed, 0, vec!["Wolf"])));
    // the reveal is over: the card is hidden where it is (701.20b)
    cite!(["701.20a"]; assert!(!g.sees(1, wolf) && !g.sees(0, wolf)));
}

#[test]
fn a_disclosure_is_what_was_shown_then_and_survives_moves_and_copies() {
    let (mut g, [_, bear, wolf]) = game();
    cast(&mut g, BURY);
    // CR404.3: explicitly authored owner order, separate from selection.
    assert_eq!(g.decision, Some(game::state::Decision::Choose { player: 0, choice: game::state::Choice::OrderCards { to: game::state::Placement::Zone(Zone::Graveyard), cards: vec![wolf,bear] } }));
    g.act(0, Move::Choose(game::state::Answer::Objects(vec![bear,wolf]))).unwrap();
    let before = g.observations(0).to_vec();
    assert_eq!(before.len(), 1);
    assert_eq!(names(&before[0]), ["Wolf", "Bear"]);
    // the cards it shows have since gone to the graveyard as new objects (400.7)
    cite!(["400.7"]; assert_eq!(g.obj(wolf).zone, Zone::Gone));
    cite!(["608.2c"]; assert_eq!(g.players[0].graveyard.len(), 3));
    assert_eq!(before[0].cards.iter().map(|c| (c.object, c.zone)).collect::<Vec<_>>(), [(wolf, Zone::Library), (bear, Zone::Library)]);
    // more of the game goes by, and a copy is made: both still have it, unchanged
    cast(&mut g, REVEAL);
    let mut copy = g.clone();
    assert_eq!(copy.observations(0), g.observations(0));
    assert_eq!(g.observations(0)[0], before[0]);
    assert_eq!(g.observations(0).len(), 2);
    assert!(g.observations(0)[0].seq < g.observations(0)[1].seq);
    // taking from the copy takes nothing from the game it was copied from
    assert_eq!(copy.take_observations(0).len(), 2);
    assert!(copy.observations(0).is_empty());
    assert_eq!(g.observations(0).len(), 2);
}

#[test]
fn taking_delivers_once_to_that_seat_and_leaves_the_other_seats_alone() {
    let (mut g, [_, _, wolf]) = game();
    cast(&mut g, LOOK);
    cast(&mut g, REVEAL);
    assert_eq!(g.observations(0).len(), 2);
    assert_eq!(g.observations(1).len(), 1);
    let taken = g.take_observations(0);
    assert_eq!(taken.iter().map(|d| d.kind).collect::<Vec<_>>(), [Kind::Looked, Kind::Revealed]);
    assert!(g.take_observations(0).is_empty(), "nothing is delivered twice");
    assert!(g.observations(0).is_empty());
    // the other seat still has the reveal, and only the reveal
    let theirs = g.take_observations(1);
    cite!(["701.20a"]; assert_eq!(theirs.iter().map(|d| (d.kind, names(d))).collect::<Vec<_>>(), [(Kind::Revealed, vec!["Wolf"])]));
    assert!(g.take_observations(1).is_empty());
    // having taken a showing is no way to look at the card again
    assert!(!g.sees(0, wolf) && !g.sees(1, wolf));
    assert!(g.take_observations(7).is_empty() && g.observations(7).is_empty(), "no such seat was shown anything");
    // a later showing is delivered by itself
    cast(&mut g, REVEAL);
    assert_eq!(g.take_observations(0).len(), 1);
}

#[test]
fn r701_22a_the_cards_a_player_scries_are_disclosed_once_as_they_are_asked() {
    let (mut g, [_, bear, wolf]) = game();
    cast(&mut g, SCRY);
    assert!(matches!(g.asked(), Some((0, Asked::Arrange { .. }))));
    // while they decide, the frame still lets them see the cards; the opponent never
    cite!(["701.22a", "400.2"]; assert!(g.sees(0, wolf) && !g.sees(1, wolf)));
    cite!(["701.22a"]; assert_eq!(g.observations(0).iter().map(|d| (d.kind, names(d))).collect::<Vec<_>>(), [(Kind::Looked, vec!["Wolf", "Bear"])]));
    cite!(["400.2"]; assert!(g.observations(1).is_empty()));
    // a refused answer and asking again show nothing more
    cite!(["701.22a"]; refused("put each of the cards looked at back on top or away, once (701.22a)"): g.act(0, Move::Choose(Answer::Arrange { top: vec![wolf], away: vec![] })));
    g.asked();
    assert_eq!(g.observations(0).len(), 1);
    g.act(0, Move::Choose(Answer::Arrange { top: vec![bear], away: vec![wolf] })).unwrap();
    assert_eq!(g.observations(0).len(), 1);
    assert!(g.observations(1).is_empty());
    assert!(!g.sees(0, bear), "the look ended with the scry");
}

#[test]
fn r701_23a_a_search_discloses_the_library_to_the_searcher_alone() {
    let (mut g, [forest, bear, wolf]) = game();
    cast(&mut g, SEARCH);
    assert!(matches!(g.asked(), Some((0, Asked::Search { .. }))));
    let [searched] = g.observations(0) else { panic!("one look") };
    cite!(["701.23a"]; assert_eq!(searched.kind, Kind::Looked));
    let mut seen: Vec<ObjId> = searched.cards.iter().map(|c| c.object).collect();
    seen.sort();
    cite!(["701.23a"]; assert_eq!(seen, [forest, bear, wolf]));
    cite!(["400.2"]; assert!(g.observations(1).is_empty()));
    g.act(0, Move::Choose(Answer::Objects(vec![bear]))).unwrap();
    assert_eq!(g.observations(0).len(), 1);
    cite!(["400.2"]; assert!(g.observations(1).is_empty(), "the card found was not revealed"));
}

#[test]
fn a_simulation_delivers_observations_without_a_request_and_copies_keep_what_waits() {
    let (mut g, _) = game();
    let spell = g.put(0, LOOK, Zone::Hand);
    let mut simulation = Simulation::new(g);
    for (player, action) in [(0, Move::Cast { card: spell, targets: vec![] }), (0, Move::Pass), (1, Move::Pass)] {
        let id = simulation.request().unwrap().unwrap().id;
        simulation.respond(id, player, action).unwrap();
    }
    let request = simulation.request().unwrap().unwrap().clone();
    assert_eq!(request.question, Asked::Priority);
    let mut branch = simulation.clone();
    assert!(simulation.take_observations(1).unwrap().is_empty());
    let taken = simulation.take_observations(0).unwrap();
    assert_eq!(taken.iter().map(|d| names(d)).collect::<Vec<_>>(), [["Wolf", "Bear"]]);
    assert!(simulation.take_observations(0).unwrap().is_empty());
    // taking answers nothing: the same request stands
    assert_eq!(simulation.request().unwrap(), Some(&request));
    // the copy made before has it still, and delivers it by itself
    assert_eq!(branch.take_observations(0).unwrap(), taken);
    // a copy made after does not carry what was delivered
    assert!(simulation.clone().take_observations(0).unwrap().is_empty());
}
