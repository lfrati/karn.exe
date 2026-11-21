//! Ponder and Flow State, their Oracle text: card selection from the top of the library
//! (608.2d), a private order for cards put back or on the bottom (401.4), an optional shuffle
//! (701.24), a draw after the order is set, and Flow State's "If …, instead …" read as its
//! spell resolves (608.2c, 608.2e).
use crate::support::{cite, paid};
use game::legal::Asked;
use game::simulation::Simulation;
use game::state::Answer;
use game::{CardDef, Event, Game, Move, ObjId, Zone};
use ir::{Color, mana::ManaSymbol};
use std::sync::OnceLock;

const PONDER: usize = 0;
const FLOW: usize = 1;
const SHOCK: usize = 2;
const DIVINATION: usize = 3;
// the library: lands, each its own card, to tell where each went
const ISLAND: usize = 4;
const FOREST: usize = 5;
const PLAINS: usize = 6;
const SWAMP: usize = 7;
const MOUNTAIN: usize = 8;
const AUGURY: usize = 9;

fn defs() -> Vec<CardDef> {
    static FACES: OnceLock<Vec<oracle::corpus::Face>> = OnceLock::new();
    let faces = FACES.get_or_init(|| oracle::corpus::load(&oracle::corpus::default_path()).unwrap());
    let oracle = |name: &str| {
        let def = CardDef::from_faces(CardDef::catalog(faces).get(name).unwrap()).unwrap();
        game::play::check(&def).unwrap();
        def
    };
    let land = |name: &str| CardDef::new(name, "", &format!("Basic Land — {name}"), "", None).unwrap();
    vec![
        oracle("Ponder"),
        oracle("Flow State"),
        CardDef::new("Shock", "{R}", "Instant", "Shock deals 2 damage to any target.", None).unwrap(),
        CardDef::new("Divination", "{2}{U}", "Sorcery", "Draw two cards.", None).unwrap(),
        land("Island"), land("Forest"), land("Plains"), land("Swamp"), land("Mountain"),
        oracle("Elemental Augury"),
    ]
}

/// Player 0's exact library, top first, and graveyard.
fn game(library: &[usize], graveyard: &[usize]) -> Game {
    let mut g = Game::new(defs(), 2, 20);
    for &card in library.iter().rev() { g.put(0, card, Zone::Library); }
    for &card in graveyard { g.put(0, card, Zone::Graveyard); }
    g
}

/// Cast the spell; `before` runs while it is on the stack; then it resolves up to its first
/// question.
fn cast_then(g: &mut Game, spell: usize, before: impl FnOnce(&mut Game), spending: &str) {
    let card = g.put(0, spell, Zone::Hand);
    for _ in 0..2 { g.add_mana(0, ManaSymbol::Color(Color::Blue)); }
    g.act(0, paid(spending, Move::Cast { card, targets: vec![] })).unwrap();
    before(g);
    for _ in 0..g.players.len() { g.act(g.priority, Move::Pass).unwrap(); }
}
fn cast(g: &mut Game, spell: usize, spending: &str) { cast_then(g, spell, |_| {}, spending); }

/// The controller's question, if any is pending other than priority.
fn asked(g: &mut Game) -> Option<Asked> {
    match g.asked() {
        Some((0, Asked::Priority)) | None => None,
        Some((0, q)) => Some(q),
        Some((p, q)) => panic!("player {p} asked {q:?}"),
    }
}
/// The controller's object question: the cards among which, and how many.
fn objects(g: &mut Game) -> (Vec<ObjId>, usize) {
    match asked(g) {
        Some(Asked::Objects { among, count, .. }) => (among, count),
        Some(Asked::OrderCards { cards, .. }) => { let count=cards.len(); (cards,count) },
        other => panic!("expected an object question: {other:?}"),
    }
}
/// The cards of `among` that are these cards, in this order.
fn pick(g: &Game, among: &[ObjId], want: &[usize]) -> Vec<ObjId> {
    want.iter().map(|w| *among.iter().find(|o| g.obj(**o).card == Some(*w)).unwrap()).collect()
}
/// Answer the object question with these cards of `among`, in this order.
fn order(g: &mut Game, among: &[ObjId], want: &[usize]) {
    let chosen = pick(g, among, want);
    g.act(0, Move::Choose(Answer::Objects(chosen))).unwrap();
}
fn may(g: &mut Game, yes: bool) {
    let q = asked(g);
    assert!(matches!(q, Some(Asked::May)), "{q:?}");
    g.act(0, Move::Choose(if yes { Answer::Yes } else { Answer::No })).unwrap();
    if !yes { assert!(!g.log.iter().any(|e| matches!(e, Event::Shuffled { .. })), "declining must not shuffle"); }
}
fn cards(g: &Game, of: &[ObjId]) -> Vec<usize> { of.iter().map(|o| g.obj(*o).card.unwrap()).collect() }
fn library(g: &Game) -> Vec<usize> { g.players[0].library.iter().rev().map(|o| g.obj(*o).card.unwrap()).collect() }
fn hand(g: &Game) -> Vec<usize> { let mut h = cards(g, &g.players[0].hand); h.sort(); h }
fn done(g: &mut Game) {
    let q = asked(g);
    assert!(g.stack.is_empty() && q.is_none(), "{q:?}");
    assert!(g.observations(1).is_empty(), "private inspection/order is never delivered to the other seat");
}

// ---------------------------------------------------------------- Ponder

#[test]
fn r401_4_ponder_orders_the_top_three_privately_then_draws_the_new_top() {
    let mut g = game(&[ISLAND, FOREST, PLAINS, SWAMP], &[]);
    cast(&mut g, PONDER, "U");
    let (among, count) = objects(&mut g);
    cite!(["608.2d"]; assert_eq!((cards(&g, &among), count), (vec![ISLAND, FOREST, PLAINS], 3), "all three, in an order"));
    for &card in &among { assert!(g.sees(0, card) && !g.sees(1, card), "looked at by its controller alone"); }
    assert!(g.observations(1).is_empty());
    // an order that is not of all three, or not by them, is refused
    cite!(["608.2d"]; refused("order every offered card exactly once (401.4, 404.3)"): g.act(0, Move::Choose(Answer::Objects(among[..2].to_vec()))));
    crate::support::refused(g.act(1, Move::Choose(Answer::Objects(among.clone()))), "that decision is another player's");
    order(&mut g, &among, &[PLAINS, ISLAND, FOREST]);
    may(&mut g, false);
    done(&mut g);
    cite!(["121.1"]; assert_eq!(hand(&g), vec![PLAINS], "the draw takes the card put on top"));
    cite!(["401.4"]; assert_eq!(library(&g), vec![ISLAND, FOREST, SWAMP]));
    for &o in &g.players[0].library { cite!(["401.4"]; assert!(!g.sees(1, o), "the order stays private")); }
}

#[test]
fn r701_24_ponder_may_shuffle_before_the_draw() {
    let mut g = game(&[ISLAND, FOREST, PLAINS, SWAMP, MOUNTAIN], &[]);
    cast(&mut g, PONDER, "U");
    let (among, _) = objects(&mut g);
    g.act(0, Move::Choose(Answer::Objects(among))).unwrap();
    may(&mut g, true);
    done(&mut g);
    assert_eq!(g.players[0].hand.len(), 1);
    let shuffle = g.log.iter().position(|e| matches!(e, Event::Shuffled { player: 0, .. })).expect("accepted shuffle occurred");
    let draw = g.log.iter().position(|e| matches!(e, Event::Drew { player: 0, .. })).expect("a card was drawn");
    cite!(["608.2c"]; assert!(shuffle < draw, "shuffle before drawing"));
    let mut all = hand(&g);
    all.extend(library(&g));
    all.sort();
    assert_eq!(all, vec![ISLAND, FOREST, PLAINS, SWAMP, MOUNTAIN], "a shuffle of the same cards, then one drawn");
    for &o in &g.players[0].library { cite!(["701.24a"]; assert!(!g.sees(0, o) && !g.sees(1, o), "a shuffled library is unknown to all")); }
}

#[test]
fn r609_3_ponder_with_short_and_empty_libraries() {
    // two cards: both ordered, then the top one drawn
    let mut g = game(&[ISLAND, FOREST], &[]);
    cast(&mut g, PONDER, "U");
    let (among, count) = objects(&mut g);
    cite!(["609.3"]; assert_eq!(count, 2));
    order(&mut g, &among, &[FOREST, ISLAND]);
    may(&mut g, false);
    done(&mut g);
    assert_eq!((hand(&g), library(&g)), (vec![FOREST], vec![ISLAND]));
    // one card: nothing to order
    let mut g = game(&[ISLAND], &[]);
    cast(&mut g, PONDER, "U");
    may(&mut g, false);
    done(&mut g);
    cite!(["609.3"]; assert_eq!((hand(&g), library(&g)), (vec![ISLAND], vec![])));
    // none: no order, the shuffle is still offered, and the draw from an empty library loses
    // the game when state-based actions are checked (704.5b)
    let mut g = game(&[], &[]);
    cast(&mut g, PONDER, "U");
    may(&mut g, false);
    assert!(g.players[0].hand.is_empty());
    cite!(["704.5b"]; assert!(g.players[0].lost, "drew from an empty library"));
}

#[test]
fn ponder_clone_resumes_its_own_order_and_shuffle_question() {
    let mut g = game(&[ISLAND, FOREST, PLAINS, SWAMP], &[]);
    cast(&mut g, PONDER, "U");
    let (among, _) = objects(&mut g);
    let mut fork = g.clone();
    order(&mut g, &among, &[FOREST, ISLAND, PLAINS]);
    order(&mut fork, &among, &[PLAINS, FOREST, ISLAND]);
    let mut again = fork.clone();
    may(&mut g, false);
    may(&mut fork, false);
    assert_eq!((hand(&g), library(&g)), (vec![FOREST], vec![ISLAND, PLAINS, SWAMP]));
    assert_eq!((hand(&fork), library(&fork)), (vec![PLAINS], vec![FOREST, ISLAND, SWAMP]));
    // resumed at the shuffle question: the order already set is kept
    may(&mut again, false);
    assert_eq!((hand(&again), library(&again)), (vec![PLAINS], vec![FOREST, ISLAND, SWAMP]));
    assert_eq!(again.observations(0).len(), 1, "resumption does not look again");
}

#[test]
fn r401_4_put_back_into_another_players_library_is_ordered_by_the_player_told_to() {
    // Elemental Augury: "{3}: Look at the top three cards of target player's library, then put
    // them back in any order." Its controller orders the target's top three; the owner is
    // neither asked nor shown them.
    let mut g = Game::new(defs(), 2, 20);
    for &card in [ISLAND, FOREST, PLAINS, SWAMP].iter().rev() { g.put(1, card, Zone::Library); }
    let augury = g.put(0, AUGURY, Zone::Battlefield);
    for _ in 0..3 { g.add_mana(0, ManaSymbol::Colorless); }
    g.act(0, paid("CCC", Move::Activate { source: augury, ability: 0, targets: vec![vec![game::Target::Player(1)]], paying: vec![] })).unwrap();
    for _ in 0..2 { g.act(g.priority, Move::Pass).unwrap(); }
    let (among, count) = objects(&mut g);
    cite!(["608.2d"]; assert_eq!((cards(&g, &among), count), (vec![ISLAND, FOREST, PLAINS], 3)));
    for &card in &among { assert!(g.sees(0, card) && !g.sees(1, card)); }
    cite!(["101.1"]; refused("that decision is another player's"): g.act(1, Move::Choose(Answer::Objects(among.clone()))));
    order(&mut g, &among, &[PLAINS, FOREST, ISLAND]);
    done(&mut g);
    let theirs: Vec<usize> = g.players[1].library.iter().rev().map(|o| g.obj(*o).card.unwrap()).collect();
    assert_eq!(theirs, vec![PLAINS, FOREST, ISLAND, SWAMP]);
    for &o in &g.players[1].library { assert!(!g.sees(1, o), "the owner is not shown the order"); }
}

// ---------------------------------------------------------------- Flow State

#[test]
fn r608_2d_flow_state_without_both_puts_one_into_hand_and_orders_the_rest_on_the_bottom() {
    // only an instant: the condition needs an instant card and a sorcery card
    let mut g = game(&[ISLAND, FOREST, PLAINS, SWAMP], &[SHOCK]);
    cast(&mut g, FLOW, "UU");
    let (among, count) = objects(&mut g);
    cite!(["608.2d"]; assert_eq!((cards(&g, &among), count), (vec![ISLAND, FOREST, PLAINS], 1)));
    for &card in &among { assert!(g.sees(0, card) && !g.sees(1, card)); }
    cite!(["608.2d"]; refused("choose exactly 1 of them to put (608.2d)"): g.act(0, Move::Choose(Answer::Objects(among[..2].to_vec()))));
    order(&mut g, &among, &[FOREST]);
    // the other two, ordered for the bottom
    let (rest, count) = objects(&mut g);
    let mut left = cards(&g, &rest);
    left.sort();
    assert_eq!((count, left), (2, vec![ISLAND, PLAINS]));
    order(&mut g, &rest, &[PLAINS, ISLAND]);
    done(&mut g);
    assert_eq!(hand(&g), vec![FOREST]);
    cite!(["401.4"]; assert_eq!(library(&g), vec![SWAMP, PLAINS, ISLAND], "the first said is the upper of the two on the bottom"));
    assert!(!g.sees(1, g.players[0].hand[0]), "into a hidden hand");
}

#[test]
fn r608_2e_flow_state_with_an_instant_and_a_sorcery_instead_puts_two_into_hand() {
    let mut g = game(&[ISLAND, FOREST, PLAINS, SWAMP], &[SHOCK, DIVINATION]);
    cast(&mut g, FLOW, "UU");
    let (among, count) = objects(&mut g);
    cite!(["608.2c"]; assert_eq!(count, 2, "instead two"));
    cite!(["608.2c"]; refused("choose exactly 2 of them to put (608.2d)"): g.act(0, Move::Choose(Answer::Objects(among[..1].to_vec()))));
    order(&mut g, &among, &[ISLAND, PLAINS]);
    // one card left for the bottom: no order to ask, and the replaced instruction is not done
    done(&mut g);
    cite!(["608.2c"]; assert_eq!(hand(&g), vec![ISLAND, PLAINS]));
    assert_eq!(library(&g), vec![SWAMP, FOREST]);
}

#[test]
fn r608_2c_flow_state_reads_the_graveyard_as_it_resolves() {
    // only an instant as it is cast; a sorcery card arrives before it resolves: two
    let mut g = game(&[ISLAND, FOREST, PLAINS, SWAMP], &[SHOCK]);
    cast_then(&mut g, FLOW, |g| { g.put(0, DIVINATION, Zone::Graveyard); }, "UU");
    cite!(["608.2h"]; assert_eq!(objects(&mut g).1, 2));
}

#[test]
fn r609_3_flow_state_with_short_and_empty_libraries() {
    // two cards, both conditions met: both into hand, nothing to the bottom
    let mut g = game(&[ISLAND, FOREST], &[SHOCK, DIVINATION]);
    cast(&mut g, FLOW, "UU");
    if let Some(Asked::Objects { among, .. }) = asked(&mut g) { g.act(0, Move::Choose(Answer::Objects(among))).unwrap(); }
    done(&mut g);
    cite!(["609.3"]; assert_eq!((hand(&g), library(&g)), (vec![ISLAND, FOREST], vec![])));
    // two cards, one into hand: the other to the bottom without an order question
    let mut g = game(&[ISLAND, FOREST], &[]);
    cast(&mut g, FLOW, "UU");
    let (among, count) = objects(&mut g);
    assert_eq!(count, 1);
    order(&mut g, &among, &[FOREST]);
    done(&mut g);
    cite!(["609.3"]; assert_eq!((hand(&g), library(&g)), (vec![FOREST], vec![ISLAND])));
    // none: nothing is asked, nothing moves, and nobody loses
    let mut g = game(&[], &[SHOCK, DIVINATION]);
    cast(&mut g, FLOW, "UU");
    done(&mut g);
    cite!(["609.3"]; assert!(g.players[0].hand.is_empty() && !g.players[0].lost));
}

#[test]
fn flow_state_clone_resumes_between_selection_and_bottom_order() {
    let mut g = game(&[ISLAND, FOREST, PLAINS, SWAMP], &[]);
    cast(&mut g, FLOW, "UU");
    let (among, _) = objects(&mut g);
    order(&mut g, &among, &[ISLAND]);
    let (rest, _) = objects(&mut g);
    let mut fork = g.clone();
    order(&mut g, &rest, &[FOREST, PLAINS]);
    order(&mut fork, &rest, &[PLAINS, FOREST]);
    assert_eq!(library(&g), vec![SWAMP, FOREST, PLAINS]);
    assert_eq!(library(&fork), vec![SWAMP, PLAINS, FOREST]);
    assert_eq!((hand(&g), hand(&fork)), (vec![ISLAND], vec![ISLAND]), "the first selection is not asked again");
}

// ---------------------------------------------------------------- managed Simulation

#[test]
fn simulation_asks_only_the_controller_and_discloses_only_to_them() {
    for (spell, spending) in [(PONDER, "U"), (FLOW, "UU")] {
        let mut g = game(&[ISLAND, FOREST, PLAINS, SWAMP], &[SHOCK, DIVINATION]);
        let card = g.put(0, spell, Zone::Hand);
        for _ in 0..2 { g.add_mana(0, ManaSymbol::Color(Color::Blue)); }
        let mut sim = Simulation::new(g);
        let r = sim.request().unwrap().unwrap().clone();
        sim.respond(r.id, 0, paid(spending, Move::Cast { card, targets: vec![] })).unwrap();
        // both pass; the spell resolves to its first question
        let r = loop {
            let r = sim.request().unwrap().unwrap().clone();
            if !matches!(r.question, Asked::Priority) { break r; }
            sim.respond(r.id, r.player, Move::Pass).unwrap();
        };
        cite!(["608.2d"]; case: format!("spell {spell}"); assert_eq!(r.player, 0));
        let (among, count) = match r.question.clone() {
            Asked::Objects { among, count, .. } => (among,count),
            Asked::OrderCards { cards, .. } => {let count=cards.len(); (cards,count)},
            other => panic!("{other:?}"),
        };
        assert_eq!(sim.respond(r.id, 1, Move::Choose(Answer::Objects(among[..count].to_vec()))), Err(game::simulation::ResponseError::WrongPlayer { expected: 0, received: 1 }), "only its controller answers");
        assert!(sim.take_observations(1).unwrap().is_empty(), "the opponent is shown nothing");
        assert!(!sim.take_observations(0).unwrap().is_empty(), "the controller saw the top three");
        let want: &[usize] = if spell == PONDER { &[ISLAND, FOREST, PLAINS] } else { &[ISLAND, FOREST] };
        let chosen=pick(sim.game().unwrap(), &among, want);
        sim.respond(r.id, 0, Move::Choose(Answer::Objects(chosen))).unwrap();
        // the rest of the questions, answered plainly, end at a priority request
        loop {
            let r = sim.request().unwrap().unwrap().clone();
            match r.question {
                Asked::Priority => break,
                Asked::May => sim.respond(r.id, 0, Move::Choose(Answer::No)).unwrap(),
                Asked::Objects { among, count, .. } => sim.respond(r.id, 0, Move::Choose(Answer::Objects(among[..count].to_vec()))).unwrap(),
                other => panic!("{other:?}"),
            }
        }
        assert!(sim.fault().is_none());
        assert!(sim.take_observations(1).unwrap().is_empty(), "no later choice leaks private cards");
        let in_hand = sim.observe(|g| g.players[0].hand.len()).unwrap();
        assert_eq!(in_hand, if spell == PONDER { 1 } else { 2 });
    }
}
