//! Jace, Architect of Thought, each ability as its rules say: a delayed triggered ability
//! that watches until its controller's next turn (603.7b), cards separated into piles
//! (700.3), and a search of each player's library (701.23) followed by casting what was
//! exiled without paying (608.2g).
use crate::support::cite;
use game::legal::Asked;
use game::state::{Answer, Decision};
use game::{CardDef, Game, Move, ObjId, Target, Zone};
use ir::Part;

const JACE: usize = 0;
const ISLAND: usize = 1;
const BEAR: usize = 2;
const BOLT: usize = 3;
const GIANT: usize = 4;

fn game(players: usize) -> Game {
    let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let catalog = CardDef::catalog(&faces);
    let defs = vec![
        CardDef::from_faces(catalog.get("Jace, Architect of Thought").expect("in the corpus")).unwrap(),
        CardDef::new("Island", "", "Basic Land — Island", "", None).unwrap(),
        CardDef::new("Bear", "{1}{G}", "Creature — Bear", "", Some((2, 2))).unwrap(),
        CardDef::new("Lightning Bolt", "{R}", "Instant", "Lightning Bolt deals 3 damage to any target.", None).unwrap(),
        CardDef::new("Giant", "{3}{R}", "Creature — Giant", "", Some((3, 3))).unwrap(),
    ];
    for d in &defs {
        game::play::check(d).unwrap_or_else(|u| panic!("{}: {}", d.name(), u.0));
    }
    let mut g = Game::new(defs, players, 20);
    for player in 0..players { for _ in 0..12 { g.put(player, ISLAND, Zone::Library); } }
    g
}
fn jace(g: &mut Game, loyalty: u32) -> ObjId {
    let jace = g.put(0, JACE, Zone::Battlefield);
    let has = g.obj(jace).counters.get("loyalty").copied().unwrap_or(0);
    g.put_counters(jace, "loyalty", loyalty - has);
    jace
}
fn activate(g: &mut Game, jace: ObjId, ability: usize) {
    g.act(0, Move::Activate { source: jace, ability, targets: vec![], paying: vec![] }).unwrap();
    while g.asked().is_some_and(|(_, a)| a == Asked::Priority) && !g.stack.is_empty() { g.act(g.priority, Move::Pass).unwrap(); }
}
/// Pass, attacking with nothing, until this player's declaration of attackers.
fn to_attackers(g: &mut Game, player: usize) {
    for _ in 0..200 {
        match g.decision.clone() {
            Some(Decision::Attackers { player: p }) if p == player => return,
            Some(Decision::Attackers { player: p }) => g.act(p, Move::Attack(vec![])).unwrap(),
            _ => match g.asked().expect("the game goes on") {
                (p, Asked::Priority) => g.act(p, Move::Pass).unwrap(),
                (p, Asked::Blockers {..}) => g.act(p, Move::Block(vec![])).unwrap(),
                (_, asked) => panic!("unexpected decision while advancing this fixture: {asked:?}"),
            },
        }
    }
    panic!("no declaration of attackers by player {player}: {:?} {:?}", g.part, g.decision);
}
fn resolve(g: &mut Game) {
    while !g.stack.is_empty() { g.act(g.priority, Move::Pass).unwrap(); }
}

#[test]
fn r603_7b_until_your_next_turn_each_attacking_creature_of_an_opponent_gets_minus_one() {
    let mut g = game(2);
    let jace = jace(&mut g, 4);
    let (bear, giant, mine) = (g.put(1, BEAR, Zone::Battlefield), g.put(1, GIANT, Zone::Battlefield), g.put(0, BEAR, Zone::Battlefield));
    activate(&mut g, jace, 0);
    cite!(["606.4"]; assert_eq!(g.obj(jace).counters.get("loyalty"), Some(&5)));
    // its controller's own creature is not one an opponent controls
    to_attackers(&mut g, 0);
    g.act(0, Move::Attack(vec![(mine, Target::Player(1))])).unwrap();
    assert!(g.stack.is_empty());
    assert_eq!(g.power(mine), Some(2));
    // whichever of the opponent's creatures attack, whatever they attack: one ability each
    to_attackers(&mut g, 1);
    g.act(1, Move::Attack(vec![(bear, Target::Object(jace)), (giant, Target::Player(0))])).unwrap();
    let Some(Decision::Order { player, abilities }) = &g.decision else { panic!("expected trigger order: {:?}", g.decision) };
    cite!(["603.3b"]; assert_eq!((*player, abilities.len()), (0, 2)));
    assert!(abilities.iter().all(|trigger| trigger.controller == 0 && trigger.source == Some(jace)));
    g.act(0, Move::Order(vec![0, 1])).unwrap();
    cite!(["603.7b", "603.2c"]; assert_eq!(g.stack.len(), 2));
    resolve(&mut g);
    assert_eq!((g.power(bear), g.toughness(bear), g.power(giant), g.toughness(giant)), (Some(1), Some(2), Some(2), Some(3)));
    // "until end of turn": the creature is as printed afterwards; "until your next turn":
    // nothing watches in the opponent's turn after that one
    to_attackers(&mut g, 0);
    g.act(0, Move::Attack(vec![])).unwrap();
    cite!(["514.2"]; assert_eq!(g.power(bear), Some(2)));
    to_attackers(&mut g, 1);
    g.act(1, Move::Attack(vec![(bear, Target::Player(0))])).unwrap();
    // Uncited (card text): Jace's "Until your next turn, whenever a creature an opponent controls attacks": the stated duration (603.7b admits one) ended as player 0's turn began
    assert!(g.stack.is_empty());
    // Uncited (card text): as above, no trigger after "Until your next turn", so no "-1/-0 until end of turn"
    assert_eq!(g.power(bear), Some(2));
    assert_eq!(g.part, Part::DeclareAttackers);
}

#[test]
fn r603_7_the_ability_watches_after_jace_has_left() {
    let mut g = game(2);
    let jace = jace(&mut g, 4);
    let bear = g.put(1, BEAR, Zone::Battlefield);
    activate(&mut g, jace, 0);
    g.move_object(jace, Zone::Graveyard);
    to_attackers(&mut g, 1);
    g.act(1, Move::Attack(vec![(bear, Target::Player(0))])).unwrap();
    resolve(&mut g);
    cite!(["603.7"]; assert_eq!(g.power(bear), Some(1)));
}

/// Jace's second ability resolved up to its first question, with a Lightning Bolt, a Bear
/// and a Giant the top three cards of the library, the Bolt on top.
fn reveal(g: &mut Game) -> (ObjId, [ObjId; 3]) {
    let giant = g.put(0, GIANT, Zone::Library);
    let bear = g.put(0, BEAR, Zone::Library);
    let bolt = g.put(0, BOLT, Zone::Library);
    assert_eq!(g.top_cards(0, 3), vec![bolt, bear, giant]);
    let jace = jace(g, 4);
    activate(g, jace, 1);
    assert_eq!(g.obj(jace).counters.get("loyalty"), Some(&2));
    (jace, [bolt, bear, giant])
}
fn hand(g: &Game) -> Vec<usize> {
    let mut cards: Vec<usize> = g.players[0].hand.iter().filter_map(|o| g.obj(*o).card).collect();
    cards.sort();
    cards
}

#[test]
fn r700_3_an_opponent_separates_and_the_controller_takes_one_pile() {
    let mut g = game(2);
    let (_, [bolt, bear, giant]) = reveal(&mut g);
    // the one opponent separates; the cards stay in the library, revealed (700.3c)
    assert_eq!(g.asked(), Some((1, Asked::Select { among: vec![bolt, bear, giant], min: 0, max: 3 })));
    cite!(["700.3c"]; assert!([bolt, bear, giant].iter().all(|card| g.obj(*card).zone == Zone::Library && g.sees(1, *card))));
    // the opponent separates, not the controller
    crate::support::refused(g.act(0, Move::Choose(Answer::Objects(vec![bolt]))), "that decision is another player's");
    g.act(1, Move::Choose(Answer::Objects(vec![giant, bolt]))).unwrap();
    // each card is in exactly one pile (700.3a); the controller chooses
    cite!(["700.3a"]; assert_eq!(g.asked(), Some((0, Asked::Pile { piles: vec![vec![bolt, giant], vec![bear]] }))));
    cite!(["700.3"]; refused("choose one of the piles (700.3)"): g.act(0, Move::Choose(Answer::Pile(2))));
    crate::support::refused(g.act(1, Move::Choose(Answer::Pile(0))), "that decision is another player's");
    g.act(0, Move::Choose(Answer::Pile(0))).unwrap();
    while g.asked().is_some_and(|(_, a)| a != Asked::Priority) {
        let (player, asked) = g.asked().unwrap();
        let cards = match asked { Asked::OrderCards { to: game::state::Placement::Library(ir::action::LibraryPosition::Bottom), cards } => cards, other => panic!("expected bottom order: {other:?}") };
        g.act(player, Move::Choose(Answer::Objects(cards))).unwrap();
    }
    assert_eq!(hand(&g), vec![BOLT, GIANT]);
    assert_eq!(g.players[0].library.first().and_then(|o| g.obj(*o).card), Some(BEAR), "the other pile is on the bottom");
    assert_eq!(g.players[0].library.len(), 13);
}

#[test]
fn r700_3d_a_pile_can_be_empty_and_be_the_one_taken() {
    let mut g = game(2);
    reveal(&mut g);
    g.act(1, Move::Choose(Answer::Objects(vec![]))).unwrap();
    let Some((0, Asked::Pile { piles })) = g.asked() else { panic!("a pile is chosen: {:?}", g.asked()) };
    cite!(["700.3d"]; assert_eq!((piles[0].len(), piles[1].len()), (0, 3)));
    g.act(0, Move::Choose(Answer::Pile(0))).unwrap();
    // the three cards go to the bottom in an order their owner chooses (401.4)
    let (player, asked) = g.asked().unwrap();
    assert_eq!(player, 0);
    cite!(["401.4"]; assert_ne!(asked, Asked::Priority, "the order on the bottom is asked"));
    let cards = match asked { Asked::OrderCards { to: game::state::Placement::Library(ir::action::LibraryPosition::Bottom), cards } => cards, other => panic!("expected bottom order: {other:?}") };
    g.act(0, Move::Choose(Answer::Objects(cards))).unwrap();
    assert_eq!(g.asked().map(|(_, a)| a), Some(Asked::Priority));
    assert!(hand(&g).is_empty());
    let mut bottom: Vec<usize> = g.players[0].library[..3].iter().filter_map(|o| g.obj(*o).card).collect();
    bottom.sort();
    assert_eq!(bottom, vec![BEAR, BOLT, GIANT]);
}

#[test]
fn r700_3_the_controller_chooses_which_opponent_separates() {
    let mut g = game(3);
    let (_, [bolt, bear, giant]) = reveal(&mut g);
    cite!(["700.3"]; assert_eq!(g.asked(), Some((0, Asked::Player { among: vec![1, 2] }))));
    cite!(["608.2d"]; refused("choose one of the players offered (608.2d)"): g.act(0, Move::Choose(Answer::Player(0))));
    g.act(0, Move::Choose(Answer::Player(2))).unwrap();
    assert_eq!(g.asked(), Some((2, Asked::Select { among: vec![bolt, bear, giant], min: 0, max: 3 })));
    g.act(2, Move::Choose(Answer::Objects(vec![bolt, bear, giant]))).unwrap();
    g.act(0, Move::Choose(Answer::Pile(0))).unwrap();
    assert_eq!(hand(&g), vec![BEAR, BOLT, GIANT]);
    assert_eq!(g.asked().map(|(_, a)| a), Some(Asked::Priority));
}

#[test]
fn r701_23_each_library_is_searched_and_what_was_exiled_may_be_cast_for_nothing() {
    let mut g = game(3);
    let (bolt, bear, giant) = (g.put(0, BOLT, Zone::Library), g.put(1, BEAR, Zone::Library), g.put(2, GIANT, Zone::Library));
    for player in 0..3 { for _ in 0..3 { g.put(player, ISLAND, Zone::Library); } }
    let jace = jace(&mut g, 8);
    let from = g.log.end();
    activate(&mut g, jace, 2);
    // the controller searches each library in turn, their own among them (701.23)
    for (owner, card) in [(0, bolt), (1, bear), (2, giant)] {
        let Some((0, Asked::Search { cards, among, min: 0, max: 1 })) = g.asked() else { panic!("a search by the controller: {:?}", g.asked()) };
        assert!(cards.iter().all(|c| g.obj(*c).owner == owner) && cards.len() == 16);
        cite!(["701.23a"]; case: format!("library {owner}"); assert_eq!(among, vec![card], "the nonland card of that library"));
        cite!(["701.23a"]; case: format!("library {owner}"); assert!(g.sees(0, cards[0]) && !g.sees(1, cards[0]), "only the player searching looks"));
        // a card of a stated quality need not be found (701.23b): none of the third library
        g.act(0, Move::Choose(Answer::Objects(if owner == 2 { vec![] } else { vec![card] }))).unwrap();
    }
    // both are exiled before either is cast, and every player shuffled
    let Some((0, Asked::Cast { cards, .. })) = g.asked() else { panic!("an offer to cast: {:?}", g.asked()) };
    assert_eq!(cards.len(), 2);
    assert_eq!(g.exile.len(), 2);
    assert!(cards.iter().all(|c| g.obj(*c).zone == Zone::Exile));
    cite!(["701.23b"]; assert_eq!(g.obj(giant).zone, Zone::Library));
    let shuffled: Vec<usize> = g.log.since(from).iter().filter_map(|e| if let game::Event::Shuffled { player, .. } = e { Some(*player) } else { None }).collect();
    assert_eq!(shuffled, vec![0, 1, 2]);
    // another player's card is cast by Jace's controller, for nothing, at no speed of its own
    let theirs = *cards.iter().find(|c| g.obj(**c).owner == 1).unwrap();
    let mine = *cards.iter().find(|c| g.obj(**c).owner == 0).unwrap();
    g.act(0, Move::Cast { card: theirs, targets: vec![] }).unwrap();
    g.act(0, Move::Cast { card: mine, targets: vec![vec![Target::Player(2)]] }).unwrap();
    assert_eq!(g.asked().map(|(_, a)| a), Some(Asked::Priority));
    cite!(["608.2g"]; assert_eq!(g.stack.len(), 2));
    resolve(&mut g);
    assert_eq!(g.players[2].life, 17);
    let bear = *g.battlefield.iter().find(|o| g.obj(**o).card == Some(BEAR)).expect("the Bear resolved");
    cite!(["110.2"]; assert_eq!((g.obj(bear).controller, g.obj(bear).owner), (0, 1)));
    assert_eq!(g.obj(jace).zone, Zone::Gone);
}

#[test]
fn a_copy_taken_at_each_pile_decision_goes_on_by_itself() {
    let mut g = game(3);
    let (_, [bolt, bear, giant]) = reveal(&mut g);
    // at the choice of who separates
    let mut at_player = g.clone();
    assert_eq!(at_player.asked(), g.asked());
    g.act(0, Move::Choose(Answer::Player(1))).unwrap();
    at_player.act(0, Move::Choose(Answer::Player(2))).unwrap();
    assert_eq!(g.asked().map(|(p, _)| p), Some(1));
    assert_eq!(at_player.asked().map(|(p, _)| p), Some(2));
    // at the separation
    let mut at_separation = g.clone();
    assert_eq!(at_separation.asked(), g.asked());
    g.act(1, Move::Choose(Answer::Objects(vec![bolt]))).unwrap();
    at_separation.act(1, Move::Choose(Answer::Objects(vec![bear, giant, bolt]))).unwrap();
    assert_eq!(g.asked(), Some((0, Asked::Pile { piles: vec![vec![bolt], vec![bear, giant]] })));
    cite!(["700.3d"]; assert_eq!(at_separation.asked(), Some((0, Asked::Pile { piles: vec![vec![bolt, bear, giant], vec![]] }))));
    // at the choice of a pile: the copy takes the other one
    let mut at_pile = g.clone();
    assert_eq!(at_pile.asked(), g.asked());
    g.act(0, Move::Choose(Answer::Pile(0))).unwrap();
    at_pile.act(0, Move::Choose(Answer::Pile(1))).unwrap();
    // Each retained branch receives its own authored choices, with no defaults.
    assert_eq!(g.asked(), Some((0, Asked::OrderCards { to: game::state::Placement::Library(ir::action::LibraryPosition::Bottom), cards: vec![bear, giant] })));
    g.act(0, Move::Choose(Answer::Objects(vec![giant, bear]))).unwrap();
    at_separation.act(0, Move::Choose(Answer::Pile(0))).unwrap();
    at_player.act(2, Move::Choose(Answer::Objects(vec![]))).unwrap();
    at_player.act(0, Move::Choose(Answer::Pile(0))).unwrap();
    assert_eq!(at_player.asked(), Some((0, Asked::OrderCards { to: game::state::Placement::Library(ir::action::LibraryPosition::Bottom), cards: vec![bolt, bear, giant] })));
    at_player.act(0, Move::Choose(Answer::Objects(vec![giant, bear, bolt]))).unwrap();
    assert_eq!(hand(&g), vec![BOLT]);
    assert_eq!(hand(&at_pile), vec![BEAR, GIANT]);
    // the copies taken earlier were not answered by what was answered after them
    crate::support::refused(at_separation.act(0, Move::Choose(Answer::Pile(0))), "no decision is pending");
    assert_eq!(hand(&at_separation), vec![BEAR, BOLT, GIANT], "the authored choice took pile zero: all three");
    assert_eq!(hand(&at_player).len() + at_player.players[0].library.len(), 15, "player 2 separated; nothing was lost");
    assert_eq!(g.players[0].library.len(), 14);
}

#[test]
fn a_copy_taken_between_the_searches_resumes_at_the_same_library() {
    let mut g = game(3);
    let (bolt, bear, giant) = (g.put(0, BOLT, Zone::Library), g.put(1, BEAR, Zone::Library), g.put(2, GIANT, Zone::Library));
    let jace = jace(&mut g, 8);
    activate(&mut g, jace, 2);
    g.act(0, Move::Choose(Answer::Objects(vec![bolt]))).unwrap();
    // the second library is being searched; what was found in the first is kept
    let mut copy = g.clone();
    assert_eq!(copy.asked(), g.asked());
    let Some((0, Asked::Search { among, .. })) = g.asked() else { panic!("the second search: {:?}", g.asked()) };
    assert_eq!(among, vec![bear]);
    // an answer the question does not offer changes nothing, in either
    cite!(["701.23a"]; refused("a chosen card is not one the search can find, in the searched zone and matching its description (701.23a): choose between 0 and 1 distinct matching objects"): copy.act(0, Move::Choose(Answer::Objects(vec![giant]))));
    assert_eq!(copy.asked(), g.asked());
    g.act(0, Move::Choose(Answer::Objects(vec![bear]))).unwrap();
    copy.act(0, Move::Choose(Answer::Objects(vec![]))).unwrap();
    for game in [&mut g, &mut copy] {
        let Some((0, Asked::Search { among, .. })) = game.asked() else { panic!("the third search: {:?}", game.asked()) };
        assert_eq!(among, vec![giant]);
        game.act(0, Move::Choose(Answer::Objects(vec![giant]))).unwrap();
    }
    let offered = |game: &mut Game| match game.asked() { Some((0, Asked::Cast { cards, .. })) => cards.len(), other => panic!("an offer: {other:?}") };
    assert_eq!((offered(&mut g), offered(&mut copy)), (3, 2));
    assert_eq!((g.exile.len(), copy.exile.len()), (3, 2));
    cite!(["701.23b"]; assert_eq!(copy.obj(bear).zone, Zone::Library));
    // a copy taken during the offer casts on by itself
    let mut at_offer = g.clone();
    g.act(0, Move::Choose(Answer::No)).unwrap();
    let Some((0, Asked::Cast { cards, .. })) = at_offer.asked() else { unreachable!() };
    let theirs = *cards.iter().find(|c| at_offer.obj(**c).owner == 1).unwrap();
    at_offer.act(0, Move::Cast { card: theirs, targets: vec![] }).unwrap();
    // the ability is still being applied in the copy, with the spell cast above it
    cite!(["608.2g"]; assert_eq!((g.stack.len(), at_offer.stack.len()), (0, 2)));
    assert!(at_offer.stack.iter().any(|o| at_offer.obj(*o).card == Some(BEAR)));
    assert_eq!(g.exile.len(), 3);
    assert_eq!(offered(&mut at_offer), 2);
}

#[test]
fn r608_2g_a_card_not_cast_stays_exiled() {
    let mut g = game(2);
    let bolt = g.put(0, BOLT, Zone::Library);
    let jace = jace(&mut g, 8);
    activate(&mut g, jace, 2);
    g.act(0, Move::Choose(Answer::Objects(vec![bolt]))).unwrap();
    g.act(0, Move::Choose(Answer::Objects(vec![]))).unwrap();
    let Some((0, Asked::Cast { cards, .. })) = g.asked() else { panic!("an offer to cast: {:?}", g.asked()) };
    g.act(0, Move::Choose(Answer::No)).unwrap();
    assert_eq!(g.asked().map(|(_, a)| a), Some(Asked::Priority));
    assert_eq!(g.obj(cards[0]).zone, Zone::Exile);
    cite!(["601.3"]; refused("that card is not in your hand: it is in exile, and no rule or effect lets you cast it from there (601.3)"): g.act(0, Move::Cast { card: cards[0], targets: vec![vec![Target::Player(1)]] }));
}
