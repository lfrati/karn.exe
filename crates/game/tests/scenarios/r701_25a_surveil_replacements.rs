//! Surveil (701.25a) puts some of the cards looked at into the graveyard and the rest back
//! on top of the library as one action. Putting a card into the graveyard is an event
//! replacement effects apply to (614.1a): where two with different outcomes would apply, the
//! card's owner chooses, a card in a library having no controller (616.1, 616.1e); once one
//! has applied, the other no longer does (616.1f). Darksteel Colossus's and Progenitus's own
//! "reveal it and shuffle it into its owner's library instead" shuffles that library in the
//! same event that puts the other cards on top, and those stay on top in the order chosen
//! (701.24g); two such cards shuffle it twice (701.24f). Every card is read from its Oracle
//! text.
use crate::support::{cite, oracle, paid, refused};
use game::binding::AbilityKey;
use game::operation::ReplacementId;
use game::simulation::{ResponseError, Simulation};
use game::start::{RandomOutcome, ShuffleOrder, ShuffleScope};
use game::state::{Answer, Choice, Decision};
use game::{Event, Game, Move, ObjId, Target, Zone};
use ir::Verb;

const TRUTHS: usize = 0;
const INKMAGE: usize = 1;
const COLOSSUS: usize = 2;
const PROGENITUS: usize = 3;
const PEACE: usize = 4;
const LEYLINE: usize = 5;
const PLAINS: usize = 6;
const ISLAND: usize = 7;
const FOREST: usize = 8;
const MOUNTAIN: usize = 9;
const SWAMP: usize = 10;
const GAZE: usize = 11;

/// A game in which player 0's library is these cards, the first of them on top; the cards,
/// in that order.
fn game(top_first: &[usize]) -> (Game, Vec<ObjId>) { game_of(0, top_first) }
/// A game in which this player's library is these cards, the first of them on top.
fn game_of(player: usize, top_first: &[usize]) -> (Game, Vec<ObjId>) {
    let defs = Vec::from(["Cruel Truths", "Cephalid Inkmage", "Darksteel Colossus", "Progenitus", "Rest in Peace",
        "Leyline of the Void", "Plains", "Island", "Forest", "Mountain", "Swamp", "Otherworldly Gaze"].map(oracle));
    let mut g = Game::new(defs, 2, 20);
    let mut cards: Vec<ObjId> = top_first.iter().rev().map(|def| g.put(player, *def, Zone::Library)).collect();
    cards.reverse();
    (g, cards)
}

/// Player 0 casts this spell from their hand with the mana of these lands, spending these
/// units, and both players pass: it resolves, and its surveil asks where the cards looked at
/// go.
fn resolve(g: &mut Game, spell: usize, lands: &[usize], spending: &str) {
    for land in lands {
        let land = g.put(0, *land, Zone::Battlefield);
        g.act(0, Move::Activate { source: land, ability: 0, targets: vec![], paying: vec![] }).unwrap();
    }
    let card = g.put(0, spell, Zone::Hand);
    g.act(0, paid(spending, Move::Cast { card, targets: vec![] })).unwrap();
    g.act(0, Move::Pass).unwrap();
    g.act(1, Move::Pass).unwrap();
}

/// Cruel Truths {3}{B}: "Surveil 2, then draw two cards. You lose 2 life."
fn cruel_truths(g: &mut Game) { resolve(g, TRUTHS, &[SWAMP, ISLAND, ISLAND, ISLAND], "BUUU"); }

fn names(g: &Game, cards: impl DoubleEndedIterator<Item = ObjId>) -> Vec<String> {
    cards.map(|c| g.obj(c).chars.name.clone()).collect()
}
/// Player 0's library by name, the top card first.
fn library(g: &Game) -> Vec<String> { library_of(g, 0) }
fn library_of(g: &Game, player: usize) -> Vec<String> { names(g, g.players[player].library.iter().rev().copied()) }
fn hand(g: &Game) -> Vec<String> { names(g, g.players[0].hand.iter().copied()) }
/// Player 0's graveyard by name, the top card last.
fn graveyard(g: &Game) -> Vec<String> { names(g, g.players[0].graveyard.iter().copied()) }

/// Where this object is now, following its zone changes (400.7).
fn now(g: &Game, was: ObjId) -> Zone {
    let step = |was: ObjId| g.log.iter().rev().find_map(|e| match e { Event::ZoneChanged { was: w, now, to, .. } if *w == was => Some((*to, *now)), _ => None });
    let mut at = step(was).expect("it moved");
    while let (_, Some(next)) = at && let Some(further) = step(next) { at = further; }
    at.0
}
fn shuffles(g: &Game) -> usize { shuffles_of(g, 0) }
fn shuffles_of(g: &Game, of: usize) -> usize { g.log.iter().filter(|e| matches!(e, Event::Shuffled { player, .. } if *player == of)).count() }
fn revealed(g: &Game, card: ObjId) -> bool { g.log.iter().any(|e| matches!(e, Event::Revealed { cards, .. } if cards.contains(&card))) }
fn surveilled(g: &Game) -> usize { g.log.iter().filter(|e| **e == Event::Arranged { player: 0, verb: Verb::Surveil }).count() }

/// The replacement effect of this card's printed ability (614.1a).
fn effect(card: ObjId, ability: usize) -> ReplacementId { ReplacementId::from_ability(card, AbilityKey::printed(ability), 0) }
/// The replacement effects `player` is asked to choose among for `card`'s move into a
/// graveyard (616.1); panics on any other question.
fn offered(g: &Game, player: usize, card: ObjId) -> Vec<ReplacementId> {
    match &g.decision {
        Some(Decision::Choose { player: p, choice: Choice::Replacement { on: Target::Object(on), kind, count: 1, optional: false, among, .. } })
            if *p == player && *on == card && kind == "graveyard" => among.clone(),
        other => panic!("player {player} is not asked which replacement applies to {card:?}: {other:?}"),
    }
}
fn same(a: &[ReplacementId], b: &[ReplacementId]) -> bool { a.len() == b.len() && a.iter().all(|r| b.contains(r)) }

/// Answer the current request with this, the next shuffles of player 0's library having
/// these outcomes, each the library top first.
fn shuffled(g: Game, answer: Answer, outcomes: Vec<Vec<ObjId>>) -> (Simulation, Result<(), ResponseError>) {
    let mut sim = Simulation::new(g);
    let request = sim.request().unwrap().unwrap().clone();
    assert_eq!(request.player, 0);
    let orders = outcomes.into_iter().map(|order| ShuffleOrder { player: 0, order }).collect();
    let result = sim.respond_with_shuffles(request.id, 0, Move::Choose(answer), orders);
    (sim, result)
}

#[test]
fn every_card_of_the_tests_is_one_the_engine_runs() {
    let _ = game(&[]);
}

#[test]
fn r701_24g_scoped_receipts_check_fixed_cards_at_consumption_after_zone_changes() {
    let (mut g, cards) = game(&[COLOSSUS, PLAINS, ISLAND, FOREST, MOUNTAIN]);
    let [colossus, plains, island, forest, mountain] = cards[..] else {unreachable!()};
    cruel_truths(&mut g);
    for position in [ir::action::LibraryPosition::Top, ir::action::LibraryPosition::FromTop(1)] {
        let mut sim = Simulation::new(g.clone());
        let request = sim.request().unwrap().unwrap().clone();
        let outcome = RandomOutcome::ScopedShuffle {player:0, order:vec![island,colossus,mountain,forest],
            scope:ShuffleScope {excluding:vec![], fixed:vec![(plains,position)]}};
        sim.respond_with_random(request.id, 0, Move::Choose(Answer::Arrange {top:vec![plains],away:vec![colossus]}),
            vec![outcome], false).unwrap();
        let after = sim.game().unwrap();
        cite!(["701.24g", "701.25a", "608.2c"]; assert_eq!(hand(after), ["Plains", "Island"]));
        cite!(["701.24g"]; assert_eq!(library(after), ["Darksteel Colossus", "Mountain", "Forest"]));
        cite!(["614.6"]; assert_eq!(graveyard(after), ["Cruel Truths"]));
    }
}

#[test]
fn r701_24g_bad_scope_preserves_the_game_and_does_not_satisfy_a_rules_refusal() {
    let (mut g, cards) = game(&[COLOSSUS, PLAINS, ISLAND, FOREST, MOUNTAIN]);
    let [colossus, plains, island, forest, mountain] = cards[..] else {unreachable!()};
    cruel_truths(&mut g);
    let source = *g.stack.last().unwrap();
    let before = format!("{g:?}");
    for fixed in [vec![(plains,ir::action::LibraryPosition::Bottom)],
                  vec![(source,ir::action::LibraryPosition::Top)], vec![]] {
        let mut sim = Simulation::new(g.clone());
        let request = sim.request().unwrap().unwrap().clone();
        let outcome = RandomOutcome::ScopedShuffle {player:0, order:vec![island,colossus,mountain,forest],
            scope:ShuffleScope {excluding:vec![],fixed}};
        let result = sim.respond_with_random(request.id, 0,
            Move::Choose(Answer::Arrange {top:vec![plains],away:vec![colossus]}), vec![outcome], false);
        // This is corrupt replay input, not a move Magic forbids.
        assert!(matches!(result,Err(ResponseError::ReplayInput(_))), "{result:?}");
        assert_eq!(format!("{:?}", sim.game().unwrap()), before);
        assert_eq!(sim.request().unwrap(), Some(&request));
        sim.respond_with_random(request.id, 0, Move::Choose(Answer::Arrange {top:vec![plains],away:vec![colossus]}),
            vec![RandomOutcome::ScopedShuffle {player:0, order:vec![island,colossus,mountain,forest],
                scope:ShuffleScope {excluding:vec![],fixed:vec![(plains,ir::action::LibraryPosition::Top)]}}], false).unwrap();
        cite!(["701.24g"]; assert_eq!(hand(sim.game().unwrap()), ["Plains", "Island"]));
    }
}

#[test]
fn r400_7_unused_scope_rebinds_its_cards_and_keeps_its_original_input_index() {
    let (mut g, cards) = game(&[COLOSSUS, PLAINS, ISLAND, FOREST, MOUNTAIN]);
    let [colossus, plains, island, forest, mountain] = cards[..] else {unreachable!()};
    cruel_truths(&mut g);
    let physical = g.physical_card_identity(plains).unwrap();
    let mut sim = Simulation::new(g);
    let request = sim.request().unwrap().unwrap().clone();
    let outcome = RandomOutcome::ScopedShuffle {player:0, order:vec![island,colossus,mountain,forest],
        scope:ShuffleScope {excluding:vec![],fixed:vec![(plains,ir::action::LibraryPosition::Top)]}};
    let unused = sim.respond_with_random(request.id, 0,
        Move::Choose(Answer::Arrange {top:vec![plains],away:vec![colossus]}), vec![outcome.clone(),outcome], true).unwrap();
    assert_eq!(unused.len(), 1);
    assert_eq!(unused[0].input_index, 1);
    let RandomOutcome::ScopedShuffle {scope,..} = &unused[0].outcome else {panic!("scope must survive")};
    let (current,position) = scope.fixed[0];
    assert_eq!(position, ir::action::LibraryPosition::Top);
    cite!(["400.7"]; assert_ne!(current, plains));
    cite!(["400.7"]; assert_eq!(sim.game().unwrap().physical_card_identity(current), Some(physical)));
    cite!(["701.24g"]; assert_eq!(sim.game().unwrap().obj(current).zone, Zone::Hand));
}

#[test]
fn r701_24g_colossus_shuffled_in_by_its_own_replacement_leaves_the_card_kept_on_top() {
    let (mut g, cards) = game(&[COLOSSUS, PLAINS, ISLAND, FOREST, MOUNTAIN]);
    let [colossus, plains, island, forest, mountain] = cards[..] else { unreachable!() };
    cruel_truths(&mut g);
    cite!(["701.25a"]; assert_eq!(g.decision, Some(Decision::Choose { player: 0, choice: Choice::Arrange { verb: Verb::Surveil, cards: vec![colossus, plains] } })));
    // each card looked at once, on top or into the graveyard
    let before = format!("{g:?}");
    cite!(["701.25a"]; refused("put each of the cards looked at back on top or away, once (701.25a)"):
        g.act(0, Move::Choose(Answer::Arrange { top: vec![plains, colossus], away: vec![colossus] })));
    assert_eq!(format!("{g:?}"), before);
    let answer = Answer::Arrange { top: vec![plains], away: vec![colossus] };

    // Only its own replacement applies: no one is asked (616.1). The library is shuffled with
    // the Plains on top: an outcome that moves the Plains is not one this shuffle can have
    // (701.24g), and is refused whole.
    let (sim, result) = shuffled(g.clone(), answer.clone(), vec![vec![island, colossus, plains, forest, mountain]]);
    cite!(["701.24g"]; assert!(matches!(&result, Err(ResponseError::ReplayInput(why)) if why.0.contains("set-aside")), "{result:?}"));
    assert_eq!(format!("{:?}", sim.game().unwrap()), before);

    // the rest, the Colossus among them, in the order the shuffle gives them
    let (sim, result) = shuffled(g.clone(), answer.clone(), vec![vec![island, colossus, mountain, forest]]);
    result.unwrap();
    let after = sim.game().unwrap();
    cite!(["614.6"]; assert_eq!(now(after, colossus), Zone::Library));
    cite!(["614.6"]; assert_eq!(graveyard(after), ["Cruel Truths"], "it was never put into a graveyard"));
    cite!(["701.24a"]; assert!(revealed(after, colossus) && shuffles(after) == 1));
    // then the two cards drawn are the Plains, kept on top, and the next card of the shuffle
    cite!(["701.24g", "701.25a"]; assert_eq!(hand(after), ["Plains", "Island"]));
    cite!(["701.24g"]; assert_eq!(library(after), ["Darksteel Colossus", "Mountain", "Forest"]));
    cite!(["701.25d"]; assert_eq!(surveilled(after), 1));
    assert_eq!(after.players[0].life, 18);

    // the outcome may also name the Plains, where it is
    let (sim, result) = shuffled(g.clone(), answer.clone(), vec![vec![plains, mountain, island, colossus, forest]]);
    result.unwrap();
    cite!(["701.24g"]; assert_eq!(hand(sim.game().unwrap()), ["Plains", "Mountain"]));

    // and with the game's own random order: the Plains is drawn first all the same
    g.act(0, Move::Choose(answer)).unwrap();
    cite!(["701.24g"]; assert_eq!(hand(&g)[0], "Plains"));
    let mut rest = library(&g);
    rest.push(hand(&g)[1].clone());
    rest.sort();
    assert_eq!(rest, ["Darksteel Colossus", "Forest", "Island", "Mountain"]);
}

#[test]
fn r701_24g_the_cards_kept_on_top_stay_in_the_order_chosen() {
    // Cephalid Inkmage {2}{U}: "When this creature enters, surveil 3."
    let (mut g, cards) = game(&[COLOSSUS, PLAINS, ISLAND, FOREST, MOUNTAIN]);
    let [colossus, plains, island, forest, mountain] = cards[..] else { unreachable!() };
    resolve(&mut g, INKMAGE, &[ISLAND, ISLAND, ISLAND], "UUU");
    // its enters trigger resolves
    g.act(0, Move::Pass).unwrap();
    g.act(1, Move::Pass).unwrap();
    cite!(["701.25a"]; assert_eq!(g.decision, Some(Decision::Choose { player: 0, choice: Choice::Arrange { verb: Verb::Surveil, cards: vec![colossus, plains, island] } })));
    // the Island on top, the Plains under it
    let answer = Answer::Arrange { top: vec![island, plains], away: vec![colossus] };
    let (sim, result) = shuffled(g.clone(), answer.clone(), vec![vec![forest, colossus, mountain]]);
    result.unwrap();
    let after = sim.game().unwrap();
    cite!(["701.24g"]; assert_eq!(library(after), ["Island", "Plains", "Forest", "Darksteel Colossus", "Mountain"]));
    assert_eq!(surveilled(after), 1);
    // a shuffle outcome that keeps them on top but in the other order is no outcome of it either
    let (_, result) = shuffled(g, answer, vec![vec![plains, island, forest, colossus, mountain]]);
    cite!(["701.24g"]; assert!(matches!(&result, Err(ResponseError::ReplayInput(_))), "{result:?}"));
}

#[test]
fn r701_24f_two_cards_shuffled_in_by_their_own_replacements_shuffle_twice_around_the_card_kept_on_top() {
    let (mut g, cards) = game(&[COLOSSUS, PROGENITUS, PLAINS, ISLAND, FOREST]);
    let [colossus, progenitus, plains, island, forest] = cards[..] else { unreachable!() };
    resolve(&mut g, INKMAGE, &[ISLAND, ISLAND, ISLAND], "UUU");
    g.act(0, Move::Pass).unwrap();
    g.act(1, Move::Pass).unwrap();
    cite!(["701.25a"]; assert_eq!(g.decision, Some(Decision::Choose { player: 0, choice: Choice::Arrange { verb: Verb::Surveil, cards: vec![colossus, progenitus, plains] } })));
    // each has only its own replacement: no one is asked (616.1)
    let answer = Answer::Arrange { top: vec![plains], away: vec![colossus, progenitus] };
    // Each modified event shuffles the library: two shuffles at the same time (701.24f), each
    // with the Plains on top. One outcome is not enough for both, and is refused whole.
    let before = format!("{g:?}");
    let (sim, result) = shuffled(g.clone(), answer.clone(), vec![vec![island, colossus, forest, progenitus]]);
    cite!(["701.24f"]; assert!(matches!(&result, Err(ResponseError::ReplayInput(why)) if why.0.contains("extra shuffle")), "{result:?}"));
    assert_eq!(format!("{:?}", sim.game().unwrap()), before);

    let (sim, result) = shuffled(g, answer, vec![vec![island, colossus, forest, progenitus], vec![forest, progenitus, island, colossus]]);
    result.unwrap();
    let after = sim.game().unwrap();
    cite!(["614.6"]; assert_eq!((now(after, colossus), now(after, progenitus)), (Zone::Library, Zone::Library)));
    cite!(["701.24a"]; assert!(revealed(after, colossus) && revealed(after, progenitus)));
    cite!(["701.24f"]; assert_eq!(shuffles(after), 2));
    // the second outcome under the Plains
    cite!(["701.24g"]; assert_eq!(library(after), ["Plains", "Forest", "Progenitus", "Island", "Darksteel Colossus"]));
    assert!(after.players[0].graveyard.is_empty());
    cite!(["701.25d"]; assert_eq!(surveilled(after), 1));
}

#[test]
fn r616_1_the_owner_chooses_between_colossus_and_an_opponents_rest_in_peace() {
    let (mut g, cards) = game(&[COLOSSUS, PLAINS, ISLAND, FOREST, MOUNTAIN]);
    let [colossus, plains, island, forest, mountain] = cards[..] else { unreachable!() };
    let peace = g.put(1, PEACE, Zone::Battlefield);
    cruel_truths(&mut g);
    g.act(0, Move::Choose(Answer::Arrange { top: vec![plains], away: vec![colossus] })).unwrap();
    // its owner chooses: in a library it has no controller (616.1)
    let among = offered(&g, 0, colossus);
    let (own, exile) = (effect(colossus, 2), effect(peace, 1));
    cite!(["616.1", "616.1e"]; assert!(same(&among, &[own.clone(), exile.clone()]), "{among:?}"));
    // nothing has moved while the owner chooses: the surveil is one action (701.25a)
    cite!(["701.25a"]; assert_eq!(library(&g), ["Darksteel Colossus", "Plains", "Island", "Forest", "Mountain"]));
    assert_eq!(surveilled(&g), 0);
    assert!(g.players[0].hand.is_empty());
    // not Rest in Peace's controller; not an effect that does not apply, nor another answer
    let before = format!("{g:?}");
    cite!(["616.1"]; refused("that decision is another player's"): g.act(1, Move::Choose(Answer::Replacement(exile.clone()))));
    cite!(["616.1"]; refused("choose one of the replacement effects that apply (616.1)"): g.act(0, Move::Choose(Answer::Replacement(effect(peace, 0)))));
    refused(g.act(0, Move::Choose(Answer::Yes)), "choose one of the replacement effects that apply (616.1)");
    assert_eq!(format!("{g:?}"), before);

    // Rest in Peace's: it is exiled, and its own replacement no longer applies (616.1f)
    let mut exiled = g.clone();
    exiled.act(0, Move::Choose(Answer::Replacement(exile))).unwrap();
    cite!(["614.6", "616.1f"]; assert_eq!(now(&exiled, colossus), Zone::Exile));
    cite!(["616.1f"]; assert!(!revealed(&exiled, colossus) && shuffles(&exiled) == 0));
    assert!(exiled.log.iter().any(|e| matches!(e, Event::Exiled { object } if *object == colossus)));
    cite!(["701.25a"]; assert_eq!(hand(&exiled), ["Plains", "Island"]));
    assert_eq!(library(&exiled), ["Forest", "Mountain"]);
    // Cruel Truths is a card put into a graveyard too (614.1a)
    assert!(exiled.players[0].graveyard.is_empty());
    assert_eq!(surveilled(&exiled), 1);

    // its own: it is shuffled into the library, where Rest in Peace does not apply (616.1f),
    // the Plains staying on top (701.24g)
    let (sim, result) = shuffled(g, Answer::Replacement(own), vec![vec![island, colossus, mountain, forest]]);
    result.unwrap();
    let after = sim.game().unwrap();
    cite!(["616.1f"]; assert_eq!(now(after, colossus), Zone::Library));
    cite!(["701.24a"]; assert!(revealed(after, colossus) && shuffles(after) == 1));
    cite!(["701.24g"]; assert_eq!(hand(after), ["Plains", "Island"]));
    assert_eq!(library(after), ["Darksteel Colossus", "Mountain", "Forest"]);
    assert_eq!(surveilled(after), 1);
}

#[test]
fn r616_1_an_opponents_leyline_of_the_void_is_chosen_against_by_the_cards_owner() {
    let (mut g, cards) = game(&[COLOSSUS, PLAINS, ISLAND, FOREST, MOUNTAIN]);
    let [colossus, plains, island, forest, mountain] = cards[..] else { unreachable!() };
    let leyline = g.put(1, LEYLINE, Zone::Battlefield);
    cruel_truths(&mut g);
    g.act(0, Move::Choose(Answer::Arrange { top: vec![plains], away: vec![colossus] })).unwrap();
    // "If a card would be put into an opponent's graveyard from anywhere, exile it instead."
    let among = offered(&g, 0, colossus);
    let (own, exile) = (effect(colossus, 2), effect(leyline, 1));
    cite!(["616.1", "616.1e"]; assert!(same(&among, &[own.clone(), exile.clone()]), "{among:?}"));
    let mut sim = Simulation::new(g.clone());
    let request = sim.request().unwrap().unwrap().clone();
    cite!(["616.1"]; assert!(matches!(sim.respond(request.id, 1, Move::Choose(Answer::Replacement(exile.clone()))),
        Err(ResponseError::WrongPlayer { expected: 0, received: 1 }))));

    let mut exiled = g.clone();
    exiled.act(0, Move::Choose(Answer::Replacement(exile))).unwrap();
    cite!(["616.1f"]; assert_eq!(now(&exiled, colossus), Zone::Exile));
    cite!(["616.1f"]; assert_eq!(shuffles(&exiled), 0));
    cite!(["701.25a"]; assert_eq!(hand(&exiled), ["Plains", "Island"]));

    let (sim, result) = shuffled(g, Answer::Replacement(own), vec![vec![mountain, island, colossus, forest]]);
    result.unwrap();
    let after = sim.game().unwrap();
    cite!(["616.1f"]; assert_eq!(now(after, colossus), Zone::Library));
    cite!(["701.24g"]; assert_eq!(hand(after), ["Plains", "Mountain"]));
    assert_eq!(library(after), ["Island", "Darksteel Colossus", "Forest"]);

    // a Leyline of the Void of the surveilling player's own does not apply to their own
    // graveyard: nothing to choose, its own replacement applies
    let (mut g, cards) = game(&[COLOSSUS, PLAINS, ISLAND, FOREST, MOUNTAIN]);
    let [colossus, plains, island, forest, mountain] = cards[..] else { unreachable!() };
    g.put(0, LEYLINE, Zone::Battlefield);
    cruel_truths(&mut g);
    let (sim, result) = shuffled(g, Answer::Arrange { top: vec![plains], away: vec![colossus] }, vec![vec![forest, colossus, island, mountain]]);
    result.unwrap();
    let after = sim.game().unwrap();
    cite!(["614.1a", "616.1"]; case: "its owner's own Leyline"; assert_eq!(now(after, colossus), Zone::Library));
    cite!(["701.24g"]; case: "its owner's own Leyline"; assert_eq!(hand(after), ["Plains", "Forest"]));
}

#[test]
fn r616_1_a_player_surveilling_on_another_players_turn_chooses_against_that_players_rest_in_peace() {
    // Otherworldly Gaze {U}, an instant: "Surveil 3."
    let (mut g, cards) = game_of(1, &[COLOSSUS, PLAINS, ISLAND, FOREST]);
    let [colossus, plains, island, _] = cards[..] else { unreachable!() };
    let peace = g.put(0, PEACE, Zone::Battlefield);
    assert_eq!(g.active, 0);
    // the active player passes, and player 1 casts it
    g.act(0, Move::Pass).unwrap();
    let land = g.put(1, ISLAND, Zone::Battlefield);
    g.act(1, Move::Activate { source: land, ability: 0, targets: vec![], paying: vec![] }).unwrap();
    let gaze = g.put(1, GAZE, Zone::Hand);
    g.act(1, paid("U", Move::Cast { card: gaze, targets: vec![] })).unwrap();
    g.act(1, Move::Pass).unwrap();
    g.act(0, Move::Pass).unwrap();
    cite!(["701.25a"]; assert_eq!(g.decision, Some(Decision::Choose { player: 1, choice: Choice::Arrange { verb: Verb::Surveil, cards: vec![colossus, plains, island] } })));
    g.act(1, Move::Choose(Answer::Arrange { top: vec![island, plains], away: vec![colossus] })).unwrap();
    // its owner chooses, not the active player, who controls Rest in Peace (616.1)
    let among = offered(&g, 1, colossus);
    let (own, exile) = (effect(colossus, 2), effect(peace, 1));
    cite!(["616.1", "616.1e"]; assert!(same(&among, &[own.clone(), exile.clone()]), "{among:?}"));
    let before = format!("{g:?}");
    cite!(["616.1"]; refused("that decision is another player's"): g.act(0, Move::Choose(Answer::Replacement(exile.clone()))));
    assert_eq!(format!("{g:?}"), before);

    let mut exiled = g.clone();
    exiled.act(1, Move::Choose(Answer::Replacement(exile))).unwrap();
    cite!(["616.1f"]; assert_eq!(now(&exiled, colossus), Zone::Exile));
    cite!(["616.1f"]; assert_eq!(shuffles_of(&exiled, 1), 0));
    cite!(["701.25a"]; assert_eq!(library_of(&exiled, 1), ["Island", "Plains", "Forest"]));

    g.act(1, Move::Choose(Answer::Replacement(own))).unwrap();
    cite!(["616.1f"]; assert_eq!(now(&g, colossus), Zone::Library));
    cite!(["701.24a"]; assert_eq!(shuffles_of(&g, 1), 1));
    // whatever the shuffle's outcome, the Island and the Plains under it stay on top
    cite!(["701.24g"]; assert_eq!(library_of(&g, 1)[..2], ["Island", "Plains"]));
    assert_eq!(g.players[1].library.len(), 4);
}

#[test]
fn r616_1_two_cards_surveilled_under_rest_in_peace_are_each_chosen_for_before_either_moves() {
    let (mut g, cards) = game(&[PROGENITUS, COLOSSUS, PLAINS, ISLAND, FOREST]);
    let [progenitus, colossus, plains, island, forest] = cards[..] else { unreachable!() };
    let peace = g.put(0, PEACE, Zone::Battlefield);
    cruel_truths(&mut g);
    g.act(0, Move::Choose(Answer::Arrange { top: vec![], away: vec![progenitus, colossus] })).unwrap();
    // its owner chooses for each, in the order they choose (101.4c); the engine asks for each in turn
    let (first, second) = match &g.decision {
        Some(Decision::Choose { choice: Choice::Replacement { on: Target::Object(on), .. }, .. }) if *on == progenitus => (progenitus, colossus),
        _ => (colossus, progenitus),
    };
    let own = |card| effect(card, if card == progenitus { 1 } else { 2 });
    let decide = |card| if card == progenitus { own(card) } else { effect(peace, 1) };
    cite!(["616.1"]; assert!(same(&offered(&g, 0, first), &[own(first), effect(peace, 1)])));
    g.act(0, Move::Choose(Answer::Replacement(decide(first)))).unwrap();
    cite!(["616.1"]; assert!(same(&offered(&g, 0, second), &[own(second), effect(peace, 1)])));
    cite!(["701.25a"]; assert_eq!(library(&g), ["Progenitus", "Darksteel Colossus", "Plains", "Island", "Forest"]));
    // Progenitus is shuffled in, the Colossus exiled: one shuffle
    let (sim, result) = shuffled(g, Answer::Replacement(decide(second)), vec![vec![island, progenitus, plains, forest]]);
    result.unwrap();
    let after = sim.game().unwrap();
    cite!(["616.1f"]; assert_eq!((now(after, progenitus), now(after, colossus)), (Zone::Library, Zone::Exile)));
    cite!(["701.24a"]; assert!(revealed(after, progenitus) && !revealed(after, colossus) && shuffles(after) == 1));
    assert_eq!(hand(after), ["Island", "Progenitus"]);
    assert_eq!(library(after), ["Plains", "Forest"]);
    assert!(after.players[0].graveyard.is_empty());
}

#[test]
fn r404_3_cards_surveilled_into_the_graveyard_go_in_the_order_given_with_no_other_question() {
    // the surveilling player owns the cards, and the order they give them in is the order
    // they arrange them in (404.3): the bottom card first here
    for (away, expected) in [([0, 1], ["Plains", "Island", "Cruel Truths"]), ([1, 0], ["Island", "Plains", "Cruel Truths"])] {
        let (mut g, cards) = game(&[PLAINS, ISLAND, FOREST, MOUNTAIN]);
        cruel_truths(&mut g);
        let away = away.map(|i| cards[i]).to_vec();
        g.act(0, Move::Choose(Answer::Arrange { top: vec![], away })).unwrap();
        // each onto the ones before it: the last given is the top card (404.3)
        cite!(["404.3"]; assert_eq!(graveyard(&g), expected));
        cite!(["701.25a"]; assert_eq!(hand(&g), ["Forest", "Mountain"]));
        assert!(!matches!(g.decision, Some(Decision::Choose { choice: Choice::OrderCards { .. }, .. })));
    }
}
