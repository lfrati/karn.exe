use super::*;
use crate::CardDef;
use crate::event::Event;
use crate::state::Zone;

/// Three players, player 0 active. Players 0 and 2 have three cards in their library;
/// player 1 has none.
fn game() -> Game {
    let card = |name: &str| CardDef::new(name, "", "Basic Land — Forest", "", None).unwrap();
    let mut game = Game::new(vec![card("One"), card("Two"), card("Three")], 3, 20);
    for player in [0, 2] {
        for def in 0..3 {
            game.put(player, def, Zone::Library);
        }
    }
    game
}

fn arranged(game: &Game) -> Vec<(PlayerId, Verb)> {
    game.log.iter().filter_map(|e| match e {
        Event::Arranged { player, verb } => Some((*player, *verb)),
        _ => None,
    }).collect()
}

fn asked(decision: crate::Res<Option<Decision>>) -> (PlayerId, Verb, Vec<ObjId>) {
    match decision.expect("supported") {
        Some(Decision::Choose { player, choice: Choice::Arrange { verb, cards } }) => (player, verb, cards),
        other => panic!("an arrangement is asked for: {other:?}"),
    }
}

#[test]
fn r701_22c_players_decide_in_turn_order_and_the_cards_move_at_the_same_time() {
    let mut g = game();
    let before = format!("{g:?}");
    let (mine, theirs) = (g.top_cards(0, 2), g.top_cards(2, 2));
    // named out of turn order, and one of them twice
    let mut pending = Pending::new(&g, vec![2, 0, 2], Verb::Scry, 2);
    assert_eq!(asked(pending.advance(&mut g)), (0, Verb::Scry, mine.clone()));
    // the same question until it is answered; what is not an arrangement of those cards is not taken
    pending.answer(Answer::Arrange { top: vec![mine[0]], away: vec![] });
    pending.answer(Answer::Arrange { top: theirs.clone(), away: vec![] });
    pending.answer(Answer::Yes);
    assert_eq!(asked(pending.advance(&mut g)), (0, Verb::Scry, mine.clone()));
    pending.answer(Answer::Arrange { top: vec![], away: vec![mine[0], mine[1]] });
    // the first player's cards have not moved while the next one decides
    assert_eq!(asked(pending.advance(&mut g)), (2, Verb::Scry, theirs.clone()));
    assert_eq!(format!("{g:?}"), before, "nothing is done before every player has decided");
    assert!(!pending.done());

    // the same procedure, answered two ways: what the first player decided is kept in each
    for swap in [false, true] {
        let (mut g, mut pending) = (g.clone(), pending.clone());
        let top = if swap { vec![theirs[1], theirs[0]] } else { theirs.clone() };
        pending.answer(Answer::Arrange { top: top.clone(), away: vec![] });
        assert_eq!(pending.advance(&mut g), Ok(None));
        assert!(pending.done());
        assert_eq!(arranged(&g), [(0, Verb::Scry), (2, Verb::Scry)]);
        assert_eq!(g.top_cards(2, 2), top);
        // both of player 0's on the bottom, the last of them the bottom card
        assert_eq!(g.players[0].library[..2], [mine[1], mine[0]]);
        assert_eq!(g.players[0].library.len(), 3);
        // done once
        assert_eq!(pending.advance(&mut g), Ok(None));
        assert_eq!(arranged(&g).len(), 2);
    }
    assert_eq!(format!("{g:?}"), before);
}

#[test]
fn r701_22b_scrying_zero_is_not_done_and_an_empty_library_is_still_scried() {
    let mut g = game();
    let before = format!("{g:?}");
    let mut pending = Pending::new(&g, vec![0, 1, 2], Verb::Scry, 0);
    assert_eq!(pending.advance(&mut g), Ok(None));
    assert_eq!(format!("{g:?}"), before, "no event (701.22b)");

    // player 1 has no card to look at: nothing is asked of them, and they have scried
    let mut pending = Pending::new(&g, vec![1], Verb::Scry, 2);
    assert_eq!(pending.advance(&mut g), Ok(None));
    assert_eq!(arranged(&g), [(1, Verb::Scry)]);

    // fewer cards than the number: those there are
    let mut pending = Pending::new(&g, vec![0], Verb::Surveil, 5);
    let (player, verb, cards) = asked(pending.advance(&mut g));
    assert_eq!((player, verb, cards.len()), (0, Verb::Surveil, 3));
    pending.answer(Answer::Arrange { top: vec![cards[2]], away: vec![cards[0], cards[1]] });
    assert_eq!(pending.advance(&mut g), Ok(None));
    assert_eq!(g.players[0].library, [cards[2]]);
    assert_eq!(g.players[0].graveyard.len(), 2);
    assert_eq!(arranged(&g), [(1, Verb::Scry), (0, Verb::Surveil)]);
}
