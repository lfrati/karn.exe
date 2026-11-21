use super::*;
use crate::CardDef;
use crate::event::Event;
use crate::program::Program;
use crate::program::expr::linked;
use crate::select::{Ctx, Support};
use crate::state::Zone;
use ir::{Atom, CardType, Domain, ObjectSet};
use std::sync::Arc;

/// Three players and a source of player 0, in play. Defs: source, Bear (creature), Forest.
fn game() -> (Game, Frame) {
    let source = CardDef::new("Source", "{2}", "Creature — Wall", "", Some((0, 4))).unwrap();
    let bear = CardDef::new("Bear", "{1}{G}", "Creature — Bear", "", Some((2, 2))).unwrap();
    let forest = CardDef::new("Forest", "", "Basic Land — Forest", "", None).unwrap();
    let mut game = Game::new(vec![source, bear, forest], 3, 20);
    let source = game.put(0, 0, Zone::Battlefield);
    let frame = Frame::new(Arc::new(Program::new(Vec::new(), Vec::new()).unwrap()), Ctx::new(0, source));
    (game, frame)
}

fn each_player(game: &Game, f: &Frame) -> Operand {
    let support = Support::definition(game.def(f.ctx.object_source()).unwrap(), None);
    Operand::All(expr::Set::compile(&ObjectSet::of(Domain::Player), &support, &mut linked(&support)).unwrap())
}

fn exactly(game: &Game, f: &Frame, n: u32) -> Count {
    let support = Support::definition(game.def(f.ctx.object_source()).unwrap(), None);
    Count::Exactly(expr::Amount::compile(&ir::Amount::Const(n), &support, &mut linked(&support)).unwrap())
}

fn cards(game: &Game, f: &Frame) -> expr::Cards {
    let support = Support::definition(game.def(f.ctx.object_source()).unwrap(), None);
    expr::Cards::compile(&ObjectSet::of(Domain::Card), &support, &mut linked(&support)).unwrap()
}

fn creatures(game: &Game, f: &Frame) -> expr::Set {
    let support = Support::definition(game.def(f.ctx.object_source()).unwrap(), None);
    let set = ObjectSet { filter: ir::Filter::Atom(Atom::Type(CardType::Creature)), ..ObjectSet::of(Domain::Permanent) };
    expr::Set::compile(&set, &support, &mut linked(&support)).unwrap()
}

fn asked(decision: Option<Decision>) -> (PlayerId, Vec<ObjId>, usize) {
    match decision {
        Some(Decision::Choose { player, choice: Choice::Objects { among, count, .. } }) => (player, among, count),
        other => panic!("not a choice of objects: {other:?}"),
    }
}

fn discarded(game: &Game) -> Vec<PlayerId> {
    game.log.iter().filter_map(|e| if let Event::Discarded { player, .. } = e { Some(*player) } else { None }).collect()
}

#[test]
fn r701_9b_a_player_with_more_than_asked_chooses_and_the_cards_are_discarded() {
    let (mut game, f) = game();
    let hand = [game.put(0, 1, Zone::Hand), game.put(0, 2, Zone::Hand), game.put(0, 1, Zone::Hand)];
    let action = Action::discard(Operand::Controller, cards(&game, &f), exactly(&game, &f, 2));
    assert!(action.scalars().is_empty());
    let mut pending = Pending::new(&game, &f, &action);

    let (player, among, count) = asked(pending.advance(&mut game).unwrap());
    assert_eq!((player, among.as_slice(), count), (0, &hand[..], 2));
    // unanswered, it asks the same and does nothing
    assert_eq!(asked(pending.advance(&mut game).unwrap()).1, hand);
    // an answer the decision does not offer is not taken
    pending.answer(Answer::Objects(vec![hand[0]]));
    pending.answer(Answer::Yes);
    assert!(pending.advance(&mut game).unwrap().is_some());
    assert!(discarded(&game).is_empty() && !pending.done());
    // a choice in a hidden zone is nobody else's to see (101.4a)
    pending.answer(Answer::Objects(vec![hand[2], hand[0]]));
    assert!(pending.public().is_empty());

    // Selecting the discarded cards does not select their relative graveyard order
    // (404.3). The owner authors that separate, top-first permutation before movement.
    assert_eq!(pending.advance(&mut game).unwrap(), Some(Decision::Choose {
        player: 0, choice: Choice::OrderCards {
            to: crate::state::Placement::Zone(Zone::Graveyard), cards: vec![hand[2], hand[0]],
        },
    }));
    assert_eq!(game.players[0].hand, hand);
    assert!(discarded(&game).is_empty());
    game.answer_operation(0, Answer::Objects(vec![hand[0], hand[2]])).unwrap();
    let entered = game.complete_operation().unwrap().unwrap();
    game.deliver_operation(entered);
    let discarded_cards = game.complete_operation().unwrap().unwrap();
    pending.operation_complete(discarded_cards);
    assert!(pending.advance(&mut game).unwrap().is_none());
    assert!(pending.done() && !pending.short());
    assert_eq!(pending.chosen(), vec![hand[2], hand[0]]);
    assert_eq!(game.players[0].hand, vec![hand[1]]);
    assert_eq!(game.players[0].graveyard.len(), 2);
    assert_eq!(game.players[0].graveyard, vec![game.became(hand[2]).unwrap(), game.became(hand[0]).unwrap()]);
    assert_eq!(discarded(&game), vec![0, 0]);

    // done once: going on again repeats nothing
    let log = game.log.len();
    assert!(pending.advance(&mut game).unwrap().is_none());
    assert_eq!(game.log.len(), log);
    assert_eq!(game.players[0].hand, vec![hand[1]]);
}

#[test]
fn r609_3_with_no_more_than_asked_discards_what_there_is_without_a_selection() {
    let (mut game, f) = game();
    let card = game.put(0, 1, Zone::Hand);
    let action = Action::discard(Operand::Controller, cards(&game, &f), exactly(&game, &f, 2));
    let mut pending = Pending::new(&game, &f, &action);
    assert!(pending.advance(&mut game).unwrap().is_none());
    assert!(pending.short());
    assert_eq!(pending.chosen(), vec![card]);
    assert!(game.players[0].hand.is_empty());

    // All there is: no discard selection, but the owner still orders simultaneous
    // graveyard arrivals (404.3), independently of the cards' hand order.
    let hand = vec![game.put(1, 1, Zone::Hand), game.put(1, 2, Zone::Hand), game.put(1, 2, Zone::Hand)];
    let all = Action::discard(each_player(&game, &f), cards(&game, &f), Count::All);
    let mut pending = Pending::new(&game, &f, &all);
    assert_eq!(pending.advance(&mut game).unwrap(), Some(Decision::Choose {
        player: 1, choice: Choice::OrderCards {
            to: crate::state::Placement::Zone(Zone::Graveyard), cards: hand.clone(),
        },
    }));
    assert_eq!(game.players[1].hand, hand);
    game.answer_operation(1, Answer::Objects(vec![hand[1], hand[2], hand[0]])).unwrap();
    let entered = game.complete_operation().unwrap().unwrap();
    game.deliver_operation(entered);
    let discarded_cards = game.complete_operation().unwrap().unwrap();
    pending.operation_complete(discarded_cards);
    assert!(pending.advance(&mut game).unwrap().is_none());
    assert!(!pending.short());
    assert_eq!(pending.by_player(), vec![(0, Vec::new()), (1, hand.clone()), (2, Vec::new())]);
    assert_eq!(game.players[1].graveyard.len(), 3);
    assert_eq!(game.players[1].graveyard, vec![game.became(hand[0]).unwrap(), game.became(hand[2]).unwrap(), game.became(hand[1]).unwrap()]);
}

#[test]
fn r101_4_each_player_chooses_in_turn_order_before_anything_is_done() {
    let (mut game, f) = game();
    game.active = 1;
    let mine = [game.put(0, 1, Zone::Battlefield), game.put(0, 1, Zone::Battlefield)];
    let theirs = [game.put(1, 1, Zone::Battlefield), game.put(1, 1, Zone::Battlefield)];
    let only = game.put(2, 1, Zone::Battlefield);
    game.put(2, 2, Zone::Battlefield);
    let action = Action::sacrifice(each_player(&game, &f), creatures(&game, &f), exactly(&game, &f, 1));
    let mut pending = Pending::new(&game, &f, &action);

    // the active player first; "a creature" is of the creatures each controls
    let (player, among, count) = asked(pending.advance(&mut game).unwrap());
    assert_eq!((player, among.as_slice(), count), (1, &theirs[..], 1));
    pending.answer(Answer::Objects(vec![theirs[1]]));
    // player 2 has one creature and nothing to choose; player 0 has the source too
    let (player, among, _) = asked(pending.advance(&mut game).unwrap());
    assert_eq!((player, among), (0, vec![f.ctx.object_source(), mine[0], mine[1]]));
    // nothing has happened, and what is chosen of the battlefield is seen (101.4b)
    assert!(game.log.iter().all(|e| !matches!(e, Event::Sacrificed { .. })));
    assert_eq!(game.obj(theirs[1]).zone, Zone::Battlefield);
    assert_eq!(pending.public(), vec![
        PublicChoice::Objects { player: 1, verb: ir::Verb::Sacrifice, objects: vec![theirs[1]] },
        PublicChoice::Objects { player: 2, verb: ir::Verb::Sacrifice, objects: vec![only] },
    ]);

    // the game goes two ways from here
    let (mut other, mut other_pending) = (game.clone(), pending.clone());
    pending.answer(Answer::Objects(vec![mine[0]]));
    other_pending.answer(Answer::Objects(vec![mine[1]]));
    assert_ne!(pending, other_pending);
    assert!(pending.advance(&mut game).unwrap().is_none());
    assert!(other_pending.advance(&mut other).unwrap().is_none());

    let sacrificed = |g: &Game| -> Vec<(PlayerId, ObjId)> {
        g.log.iter().filter_map(|e| if let Event::Sacrificed { player, object } = e { Some((*player, *object)) } else { None }).collect()
    };
    assert_eq!(sacrificed(&game), vec![(1, theirs[1]), (2, only), (0, mine[0])]);
    assert_eq!(sacrificed(&other), vec![(1, theirs[1]), (2, only), (0, mine[1])]);
    assert_eq!(pending.by_player(), vec![(1, vec![theirs[1]]), (2, vec![only]), (0, vec![mine[0]])]);
    assert_eq!(game.obj(mine[1]).zone, Zone::Battlefield);
    assert_eq!(other.obj(mine[0]).zone, Zone::Battlefield);
    assert_eq!(game.obj(theirs[0]).zone, Zone::Battlefield);
    assert!(discarded(&game).is_empty());
}

#[test]
fn who_chooses_and_the_source_itself() {
    let (mut game, f) = game();
    let hand = [game.put(1, 1, Zone::Hand), game.put(1, 2, Zone::Hand)];
    // "of your choice": the controller of the effect chooses for the player who discards
    let action = Action::discard(each_player(&game, &f), cards(&game, &f), exactly(&game, &f, 1)).chosen_by(Chooser::Controller);
    let mut pending = Pending::new(&game, &f, &action);
    let (player, among, _) = asked(pending.advance(&mut game).unwrap());
    assert_eq!((player, among.as_slice()), (0, &hand[..]));
    pending.answer(Answer::Objects(vec![hand[1]]));
    assert!(pending.advance(&mut game).unwrap().is_none());
    assert_eq!(game.players[1].hand, vec![hand[0]]);
    assert_eq!(discarded(&game), vec![1]);

    // "a creature you control" is the effect's controller's, whoever acts (109.5): another
    // player controls none of those, and their own creatures are not what is described
    let (mine, theirs) = (game.put(0, 1, Zone::Battlefield), game.put(1, 1, Zone::Battlefield));
    let support = Support::definition(game.def(f.ctx.object_source()).unwrap(), None);
    let you = ir::Relation { who: Box::new(ir::Selector::new(ir::Quant::Definite(ir::Ref::You), ObjectSet::of(Domain::Player))), negated: false, past: false };
    let yours = ObjectSet { filter: ir::Filter::Atom(Atom::Type(CardType::Creature)), controller: Some(you), ..ObjectSet::of(Domain::Permanent) };
    let yours = expr::Set::compile(&yours, &support, &mut linked(&support)).unwrap();
    let action = Action::sacrifice(each_player(&game, &f), yours, exactly(&game, &f, 1));
    let mut pending = Pending::new(&game, &f, &action);
    let (player, among, _) = asked(pending.advance(&mut game).unwrap());
    assert_eq!((player, among), (0, vec![f.ctx.object_source(), mine]));
    pending.answer(Answer::Objects(vec![mine]));
    assert!(pending.advance(&mut game).unwrap().is_none());
    assert_eq!(pending.by_player(), vec![(0, vec![mine]), (1, Vec::new()), (2, Vec::new())]);
    assert!(pending.short());
    assert_eq!(game.obj(theirs).zone, Zone::Battlefield);

    // "sacrifice this creature": its controller can, another player cannot (701.21a)
    let source = f.ctx.object_source();
    let mut pending = Pending::new(&game, &f, &Action::sacrifice_source(each_player(&game, &f)));
    assert!(pending.advance(&mut game).unwrap().is_none());
    assert_eq!(pending.by_player(), vec![(0, vec![source]), (1, Vec::new()), (2, Vec::new())]);
    assert_eq!(game.obj(source).zone, Zone::Gone);
    assert_eq!(game.players[0].graveyard.len(), 2);
}
