use std::sync::Arc;
use game::{CardDef, Game, Move, Zone};
use game::state::Answer;

#[test]
fn independent_games_share_programs_but_not_waiting_choices() {
    let defs = Arc::new(vec![CardDef::new("Choice", "{0}", "Sorcery",
        "You may gain 1 life. You may gain 2 life.", None).unwrap()]);
    let mut first = Game::new(defs.clone(), 2, 20);
    let mut second = Game::new(defs.clone(), 2, 20);
    assert!(Arc::ptr_eq(&first.defs, &second.defs));
    assert!(Arc::ptr_eq(&first.defs, &defs));
    for game in [&mut first, &mut second] {
        let card = game.put(0, 0, Zone::Hand);
        game.act(0, Move::Cast { card, targets: vec![] }).unwrap();
        game.act(0, Move::Pass).unwrap();
        game.act(1, Move::Pass).unwrap();
    }
    let untouched = format!("{second:?}");
    first.act(0, Move::Choose(Answer::Yes)).unwrap();
    assert_eq!(first.players[0].life, 21);
    assert_eq!(format!("{second:?}"), untouched);
    second.act(0, Move::Choose(Answer::No)).unwrap();
    assert_eq!(second.players[0].life, 20);
    first.act(0, Move::Choose(Answer::No)).unwrap();
    second.act(0, Move::Choose(Answer::Yes)).unwrap();
    assert_eq!(first.players[0].life, 21);
    assert_eq!(second.players[0].life, 22);
}

#[test]
fn independently_started_games_share_catalog_and_reproduce_seeded_opening() {
    let defs = Arc::new(vec![
        CardDef::new("Forest", "", "Basic Land — Forest", "", None).unwrap(),
        CardDef::new("Plains", "", "Basic Land — Plains", "", None).unwrap(),
    ]);
    let deck: Vec<_> = (0..40).map(|i| i % 2).collect();
    let decks = vec![deck.clone(), deck];
    let first = Game::start(defs.clone(), &decks, 20, 0, 73);
    let second = Game::start(defs.clone(), &decks, 20, 0, 73);
    assert!(Arc::ptr_eq(&first.defs, &second.defs));
    assert!(Arc::ptr_eq(&first.defs, &defs));
    assert_eq!(format!("{first:?}"), format!("{second:?}"));
}
