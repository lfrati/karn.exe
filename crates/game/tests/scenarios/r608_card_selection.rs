//! Resolution selections (608.2d), private library order (401.4), and reveal (701.20).
use crate::support::{cite, paid};
use game::{CardDef, Game, Move, ObjId, Zone};
use game::state::{Answer, Choice, Decision};
use ir::{Color, mana::ManaSymbol};

const SLEIGHT: &str = "Look at the top two cards of your library. Put one of them into your hand and the other on the bottom of your library.";
const STIRRINGS: &str = "Look at the top five cards of your library. You may reveal a colorless card from among them and put it into your hand. Then put the rest on the bottom of your library in any order.";
const BRAINSTORM: &str = "Draw three cards, then put two cards from your hand on top of your library in any order.";

fn game(text: &str, library: &[usize]) -> Game {
    let spell = CardDef::new("Selection witness", "{U}", "Sorcery", text, None).unwrap();
    game::play::check(&spell).unwrap();
    let mut g = Game::new(vec![spell,
        CardDef::new("Island", "", "Basic Land — Island", "", None).unwrap(),
        CardDef::new("Forest", "", "Basic Land — Forest", "", None).unwrap(),
        CardDef::new("Shock", "{R}", "Instant", "Shock deals 2 damage to any target.", None).unwrap(),
    ], 3, 20);
    for &card in library.iter().rev() { g.put(0, card, Zone::Library); }
    for player in 1..3 { g.put(player, 1, Zone::Library); }
    g
}
fn cast(g: &mut Game) {
    let card = g.put(0, 0, Zone::Hand);
    g.add_mana(0, ManaSymbol::Color(Color::Blue));
    g.act(0, paid("U", Move::Cast { card, targets: vec![] })).unwrap();
    for _ in 0..g.players.len() { g.act(g.priority, Move::Pass).unwrap(); }
}
fn choice(g: &Game) -> (Vec<ObjId>, usize) {
    match &g.decision {
        Some(Decision::Choose { player: 0, choice: Choice::Objects { among, count, .. } }) => (among.clone(), *count),
        other => panic!("expected controller's object choice: {other:?}"),
    }
}
fn order_choice(g: &Game, position: ir::action::LibraryPosition) -> (Vec<ObjId>, usize) {
    match &g.decision {
        Some(Decision::Choose { player: 0, choice: Choice::OrderCards { to: game::state::Placement::Library(at), cards } }) if *at == position => (cards.clone(), cards.len()),
        other => panic!("expected private library order: {other:?}"),
    }
}
fn answer(g: &mut Game, cards: Vec<ObjId>) { g.act(0, Move::Choose(Answer::Objects(cards))).unwrap(); }
fn library(g: &Game) -> Vec<usize> { g.players[0].library.iter().rev().map(|c| g.obj(*c).card.unwrap()).collect() }
fn hand(g: &Game) -> Vec<usize> { g.players[0].hand.iter().map(|c| g.obj(*c).card.unwrap()).collect() }

#[test]
fn r608_2d_sleight_selects_one_and_moves_only_the_other_of_the_looked_at_pair() {
    let mut g = game(SLEIGHT, &[1, 2, 3]);
    cast(&mut g);
    let (among, count) = choice(&g);
    assert_eq!(count, 1);
    assert_eq!(among.len(), 2);
    for &card in &among { assert!(g.sees(0, card)); cite!(["701.20e"]; case: format!("{card:?}"); assert!(!g.sees(1, card))); }
    assert_eq!(g.observations(0).len(), 1);
    assert!(g.observations(1).is_empty() && g.observations(2).is_empty());
    let before = g.log.len();
    for (player, reply) in [(1, Move::Choose(Answer::Objects(vec![among[0]]))), (0, Move::Pass),
        (0, Move::Choose(Answer::Objects(vec![]))), (0, Move::Choose(Answer::Objects(among.clone())))] {
        let why = match (player, &reply) { (1, _) => "that decision is another player's", (_, Move::Pass) => "a decision is pending: a choice of the effect being applied (608.2d)", _ => "choose exactly 1 of them to put (608.2d)" };
        crate::support::refused(g.act(player, reply), why);
        assert_eq!(g.log.len(), before);
    }
    let mut fork = g.clone();
    answer(&mut g, vec![among[0]]);
    answer(&mut fork, vec![among[1]]);
    cite!(["608.2d"]; assert_eq!((hand(&g), library(&g)), (vec![1], vec![3, 2])));
    cite!(["608.2d"]; assert_eq!((hand(&fork), library(&fork)), (vec![2], vec![3, 1])));
    cite!(["608.2c"]; assert_eq!(g.observations(0).len(), 1, "resumption does not look again"));
    assert!(g.stack.is_empty() && g.decision.is_none());
}

#[test]
fn r609_3_sleight_handles_short_and_empty_libraries() {
    for cards in [vec![], vec![1]] {
        let mut g = game(SLEIGHT, &cards);
        cast(&mut g);
        cite!(["609.3"]; case: format!("{cards:?}"); assert_eq!(hand(&g), cards));
        assert!(g.players[0].library.is_empty());
        assert!(g.stack.is_empty() && g.decision.is_none());
    }
}

#[test]
fn r701_20_stirrings_reveals_only_the_selected_colorless_card_and_orders_the_rest() {
    let mut g = game(STIRRINGS, &[1, 3, 2, 3, 3, 3]);
    cast(&mut g);
    assert!(matches!(g.decision, Some(Decision::Choose { player: 0, choice: Choice::May })));
    cite!(["701.20e"]; assert!(g.observations(1).is_empty()));
    g.act(0, Move::Choose(Answer::Yes)).unwrap();
    let (among, count) = choice(&g);
    assert_eq!((among.len(),count), (2,1));
    let chosen = among[1];
    let red = g.players[0].library.iter().find(|c| g.obj(**c).card == Some(3)).copied().unwrap();
    cite!(["608.2d"]; refused("a chosen object is not among the offered objects to put (608.2d)"): g.act(0, Move::Choose(Answer::Objects(vec![red]))));
    answer(&mut g, vec![chosen]);
    let (rest, count) = order_choice(&g, ir::action::LibraryPosition::Bottom);
    assert_eq!(count, 4);
    assert_eq!(hand(&g), vec![2]);
    assert_eq!(g.observations(1).len(), 1);
    assert_eq!(g.observations(1)[0].cards.len(), 1);
    cite!(["701.20a"]; assert_eq!(g.observations(1)[0].cards[0].object, chosen));
    assert_eq!(g.observations(0).len(), 2);
    let mut fork = g.clone();
    let reversed = rest.iter().rev().copied().collect::<Vec<_>>();
    answer(&mut g, reversed.clone());
    answer(&mut fork, rest.clone());
    assert_eq!(g.observations(0).len(), 2, "ordering does not replay the look or reveal");
    assert_eq!(hand(&g), vec![2]);
    cite!(["401.4"]; assert_ne!(library(&g), library(&fork)));
    assert!(g.stack.is_empty() && g.decision.is_none());
}

#[test]
fn r608_2d_stirrings_decline_keeps_every_looked_at_card_and_no_eligible_card_skips_the_offer() {
    for (cards, offers) in [(vec![1,3,2],true), (vec![3,3],false), (vec![],false)] {
        let mut g = game(STIRRINGS, &cards);
        cast(&mut g);
        if offers {
            assert!(matches!(g.decision, Some(Decision::Choose { choice: Choice::May, .. })));
            g.act(0, Move::Choose(Answer::No)).unwrap();
        }
        if cards.len() > 1 {
            let (among, count) = order_choice(&g, ir::action::LibraryPosition::Bottom);
            assert_eq!((among.len(),count),(cards.len(),cards.len()));
            answer(&mut g, among);
        }
        assert!(hand(&g).is_empty());
        cite!(["608.2d"]; case: format!("{cards:?}"); assert_eq!(library(&g), cards));
        cite!(["701.20e"]; case: format!("{cards:?}"); assert!(g.observations(1).is_empty()));
        assert!(g.stack.is_empty() && g.decision.is_none());
    }
}

#[test]
fn r401_4_brainstorm_selects_from_the_whole_current_hand_then_privately_orders() {
    let mut g = game(BRAINSTORM, &[1,2,3,1]);
    let old = g.put(0, 3, Zone::Hand);
    cast(&mut g);
    let (among, count) = choice(&g);
    assert_eq!((among.len(),count),(4,2));
    cite!(["608.2d"]; assert!(among.contains(&old)));
    cite!(["608.2c"]; assert_eq!(g.players[0].library.len(), 1, "draw completed exactly once"));
    let forest = among.iter().copied().find(|c| g.obj(*c).card == Some(2)).unwrap();
    let before = g.log.len();
    cite!(["608.2d"]; refused("choose each object to put once (608.2d)"): g.act(0, Move::Choose(Answer::Objects(vec![old,old]))));
    crate::support::refused(g.act(1, Move::Choose(Answer::Objects(vec![old,forest]))), "that decision is another player's");
    cite!(["608.2d"]; refused("a decision is pending: a choice of the effect being applied (608.2d)"): g.act(0, Move::Pass));
    assert_eq!(g.log.len(), before);
    assert_eq!(g.players[0].library.len(), 1);
    answer(&mut g, vec![old,forest]);
    let (order,count) = order_choice(&g, ir::action::LibraryPosition::Top);
    assert_eq!((order.len(),count),(2,2));
    assert_eq!(g.players[0].hand.len(), 4, "placement awaits its order");
    let mut fork = g.clone();
    answer(&mut g, vec![forest,old]);
    answer(&mut fork, vec![old,forest]);
    cite!(["401.4"]; assert_eq!(library(&g), vec![2,3,1]));
    cite!(["401.4"]; assert_eq!(library(&fork), vec![3,2,1]));
    assert_eq!(g.players[0].hand.len(), 2);
    cite!(["401.4"]; assert!(g.observations(1).is_empty() && g.observations(2).is_empty()));
    assert!(g.stack.is_empty() && g.decision.is_none());
}

#[test]
fn r608_2d_unbound_card_partitives_are_refused() {
    for text in ["Put one of them into your hand.", "Put the other on the bottom of your library."] {
        let def = CardDef::new("Unbound", "{U}", "Sorcery", text, None).unwrap();
        // Uncited (admission): no preceding instruction binds either partitive reference.
        crate::support::refused(game::play::check(&def).map_err(|e|e.0), "no earlier instruction produces what it names");
    }
}

#[test]
fn r609_3_hand_selection_moves_as_many_as_exist_without_inventing_cards() {
    let text = "Put two cards from your hand on top of your library in any order.";
    for count in 0..=2 {
        let mut g = game(text, &[3]);
        for _ in 0..count { g.put(0, 1, Zone::Hand); }
        cast(&mut g);
        if count == 2 {
            let (among, n) = order_choice(&g, ir::action::LibraryPosition::Top);
            assert_eq!(n, 2);
            answer(&mut g, among);
        }
        assert!(hand(&g).is_empty());
        cite!(["609.3"]; case: format!("{count}"); assert_eq!(g.players[0].library.len(), count + 1));
        assert!(g.stack.is_empty() && g.decision.is_none());
    }
}

#[test]
fn r608_2d_stirrings_one_eligible_card_still_allows_declining_but_needs_no_selection() {
    let mut g = game(STIRRINGS, &[1,3]);
    cast(&mut g);
    assert!(matches!(g.decision, Some(Decision::Choose { choice: Choice::May, .. })));
    g.act(0, Move::Choose(Answer::Yes)).unwrap();
    cite!(["608.2d"]; assert_eq!(hand(&g), vec![1]));
    assert_eq!(library(&g), vec![3]);
    assert!(g.stack.is_empty() && g.decision.is_none());
    cite!(["701.20a"]; assert_eq!(g.observations(1)[0].cards.len(), 1));
}

#[test]
fn r608_2c_remainder_excludes_a_selected_card_even_when_it_stays_in_the_library() {
    // Targeted composition: selection cannot be reconstructed from zone departures.
    let text = "Look at the top three cards of your library. You may reveal a land card from among them and gain 1 life. Put the rest on the bottom of your library in any order.";
    for yes in [false, true] {
        let mut g = game(text, &[1,2,3,3]);
        cast(&mut g);
        g.act(0, Move::Choose(if yes { Answer::Yes } else { Answer::No })).unwrap();
        if yes {
            let (among, _) = choice(&g);
            answer(&mut g, vec![among[0]]);
        }
        let (rest,count) = order_choice(&g, ir::action::LibraryPosition::Bottom);
        cite!(["608.2c"]; case: format!("yes={yes}"); assert_eq!(count, if yes { 2 } else { 3 }));
        answer(&mut g, rest);
        cite!(["608.2c"]; case: format!("yes={yes}"); assert_eq!(library(&g), if yes { vec![1,3,2,3] } else { vec![3,1,2,3] }));
        assert_eq!(g.players[0].life, if yes { 21 } else { 20 });
        assert!(g.stack.is_empty());
    }
}

#[test]
fn r401_4_one_selected_card_can_return_to_top_without_moving_the_others() {
    let mut g = game("Look at the top three cards of your library. Put one of them back on top of your library.", &[1,2,3,3]);
    cast(&mut g);
    let (among,count) = choice(&g);
    cite!(["608.2d"]; assert_eq!((among.len(),count),(3,1)));
    answer(&mut g, vec![among[1]]);
    cite!(["701.20e", "701.20b"]; assert_eq!(library(&g), vec![2,1,3,3]));
    assert!(hand(&g).is_empty());
    assert!(g.stack.is_empty() && g.decision.is_none());
    cite!(["701.20e"]; assert!(g.observations(1).is_empty()));
}
