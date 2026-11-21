//! Searches of zones other than your own library (701.23a): another player's graveyard, hand
//! and library at once, for cards with a chosen name (201.4), "up to four" or "any number" of
//! them all told; the searcher looks at every card of the hidden zones searched (701.23a),
//! need not find what a stated quality describes (701.23b), and the cards found are exiled
//! before that player shuffles. "Draws a card for each card exiled from their hand this way"
//! counts the cards that left that zone (400.7, 608.2c). Every card is read from its Oracle
//! text.

use crate::support::cite;
use game::binding::Value;
use game::observation::Kind;
use game::state::{Answer, Choice, Decision};
use game::{CardDef, Game, Move, ObjId, Target, Zone};

const EGO: usize = 0;
const BOLT: usize = 1;
const ISLAND: usize = 2;
const DISPOSSESS: usize = 3;
const VENDETTA: usize = 4;
const BRAIN: usize = 5;
const SHOCK: usize = 6;
const ORNITHOPTER: usize = 7;
const TALENTS: usize = 8;
const END: usize = 9;
const BEARS: usize = 10;
const PICK: usize = 11;

fn game() -> Game {
    let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let catalog = CardDef::catalog(&faces);
    let defs: Vec<_> = ["Unmoored Ego", "Lightning Bolt", "Island", "Dispossess", "Ancient Vendetta", "The Stone Brain", "Shock", "Ornithopter", "Test of Talents", "The End", "Grizzly Bears", "Pick the Brain"]
        .map(|n| CardDef::from_faces(&catalog[n]).unwrap()).to_vec();
    for d in &defs { game::play::check(d).unwrap_or_else(|e| panic!("{}: {}", d.name(), e.0)); }
    let mut g = Game::new(defs, 2, 20);
    for p in 0..2 { for _ in 0..6 { g.put(p, ISLAND, Zone::Library); } }
    while g.part != ir::Part::PrecombatMain { g.act(g.priority, Move::Pass).unwrap(); }
    g
}

fn pass(g: &mut Game) { for _ in 0..g.players.len() { g.act(g.priority, Move::Pass).unwrap(); } }
fn name(n: &str) -> Move { Move::Choose(Answer::Value(Value::CardName(n.into()))) }
fn named(g: &Game, cards: &[ObjId], n: &str) -> usize { cards.iter().filter(|o| g.obj(**o).chars.name == n).count() }
fn exiled(g: &Game, n: &str) -> usize { named(g, &g.exile, n) }

/// Player 0 casts this sorcery at player 1 with plenty of mana, and names `card`.
fn cast_at_opponent(g: &mut Game, spell: usize, card: &str) {
    let spending = match spell { EGO => "WUB", DISPOSSESS => "WWB", VENDETTA => "WWWB", _ => panic!("fixture spell") };
    let c = g.put(0, spell, Zone::Hand);
    g.players[0].pool.colors = [3, 3, 3, 3, 3];
    g.act(0, crate::support::paid(spending, Move::Cast { card: c, targets: vec![vec![Target::Player(1)]] })).unwrap();
    pass(g);
    assert!(matches!(g.decision, Some(Decision::Choose { player: 0, choice: Choice::Name(_) | Choice::Value(game::binding::Kind::CardName) })), "{:?}", g.decision);
    g.act(0, name(card)).unwrap();
}

/// Player 1's Bolts: one in the graveyard, two in hand, two in the library.
fn bolts(g: &mut Game) -> (Vec<ObjId>, Vec<ObjId>, Vec<ObjId>) {
    let grave = vec![g.put(1, BOLT, Zone::Graveyard)];
    let hand = vec![g.put(1, BOLT, Zone::Hand), g.put(1, BOLT, Zone::Hand)];
    let library = vec![g.put(1, BOLT, Zone::Library), g.put(1, BOLT, Zone::Library)];
    (grave, hand, library)
}

fn search(g: &Game) -> (Vec<ObjId>, Vec<ObjId>, usize, usize) {
    let Some(Decision::Choose { player: 0, choice: Choice::Search { cards, among, min, max } }) = g.decision.clone() else { panic!("the search: {:?}", g.decision) };
    (cards, among, min, max)
}

#[test]
fn r701_23a_unmoored_ego_searches_graveyard_hand_and_library_for_up_to_four_in_all() {
    // "Choose a card name. Search target opponent's graveyard, hand, and library for up to four
    // cards with that name and exile them. That player shuffles, then draws a card for each card
    // exiled from their hand this way."
    let mut g = game();
    let (grave, hand, library) = bolts(&mut g);
    let island = g.put(1, ISLAND, Zone::Hand);
    cast_at_opponent(&mut g, EGO, "Lightning Bolt");
    let (cards, among, min, max) = search(&g);
    let all: Vec<_> = grave.iter().chain(&hand).chain(&library).copied().collect();
    cite!(["701.23a"]; assert!(all.iter().all(|o| among.contains(o)) && among.len() == 5, "every Bolt of the three zones: {among:?}"));
    cite!(["701.23a"]; assert!(cards.contains(&island), "every card of the zones searched is looked at"));
    cite!(["701.23b", "107.1c"]; assert_eq!((min, max), (0, 4), "up to four all told"));
    // the searcher looked at the hidden cards of the hand and library (701.23a)
    let looked: Vec<ObjId> = g.observations(0).iter().filter(|d| d.kind == Kind::Looked).flat_map(|d| d.cards.iter().map(|c| c.object)).collect();
    cite!(["701.23a"]; assert!(hand.iter().chain(&library).chain([&island]).all(|o| looked.contains(o)), "{looked:?}"));
    cite!(["107.1c"]; refused("choose between 0 and 4"): g.act(0, Move::Choose(Answer::Objects(all.clone()))));
    let chosen = vec![grave[0], hand[0], hand[1], library[0]];
    g.act(0, Move::Choose(Answer::Objects(chosen))).unwrap();
    cite!(["701.23a"]; assert_eq!(exiled(&g, "Lightning Bolt"), 4));
    // two exiled from the hand: two cards drawn; the Island stays in hand
    cite!(["608.2c", "400.7"]; assert_eq!(g.players[1].hand.len(), 3, "the Island and two drawn"));
    assert!(g.players[1].hand.contains(&island));
    cite!(["701.23a"]; assert_eq!(named(&g, &g.players[1].library, "Lightning Bolt") + named(&g, &g.players[1].hand, "Lightning Bolt"), 1, "the Bolt left in the library"));
}

#[test]
fn r701_23b_finding_none_still_shuffles_and_draws_nothing() {
    let mut g = game();
    let (_, hand, _) = bolts(&mut g);
    cast_at_opponent(&mut g, EGO, "Lightning Bolt");
    g.act(0, Move::Choose(Answer::Objects(vec![]))).unwrap();
    cite!(["701.23b"]; assert_eq!(exiled(&g, "Lightning Bolt"), 0));
    cite!(["608.2c"]; assert_eq!(g.players[1].hand, hand, "nothing exiled from the hand: nothing drawn"));
    assert!(g.decision.is_none() && g.stack.is_empty());
}

#[test]
fn r701_23a_cards_from_the_graveyard_and_library_draw_nothing() {
    let mut g = game();
    let (grave, hand, library) = bolts(&mut g);
    cast_at_opponent(&mut g, EGO, "Lightning Bolt");
    g.act(0, Move::Choose(Answer::Objects(vec![grave[0], library[0], library[1]]))).unwrap();
    cite!(["608.2c", "400.7"]; assert_eq!(g.players[1].hand, hand, "none from the hand: no draw"));
    assert_eq!(exiled(&g, "Lightning Bolt"), 3);
}

#[test]
fn r701_23a_empty_zones_offer_nothing_and_the_spell_resolves() {
    let mut g = game();
    g.players[1].library.clear();
    cast_at_opponent(&mut g, VENDETTA, "Lightning Bolt");
    if let Some(Decision::Choose { choice: Choice::Search { among, .. }, .. }) = &g.decision {
        assert!(among.is_empty());
        g.act(0, Move::Choose(Answer::Objects(vec![]))).unwrap();
    }
    cite!(["701.23a"]; assert!(g.decision.is_none() && g.stack.is_empty()));
}

#[test]
fn r201_4_dispossess_finds_any_number_with_the_chosen_artifact_name() {
    // "Choose an artifact card name. Search target opponent's graveyard, hand, and library for
    // any number of cards with the chosen name and exile them. Then that player shuffles."
    let mut g = game();
    let thopters = [g.put(1, ORNITHOPTER, Zone::Graveyard), g.put(1, ORNITHOPTER, Zone::Hand), g.put(1, ORNITHOPTER, Zone::Library),
        g.put(1, ORNITHOPTER, Zone::Library), g.put(1, ORNITHOPTER, Zone::Library)];
    g.put(1, BOLT, Zone::Hand);
    let c = g.put(0, DISPOSSESS, Zone::Hand);
    let spending = "WWB";
    g.players[0].pool.colors = [3, 3, 3, 3, 3];
    g.act(0, crate::support::paid(spending, Move::Cast { card: c, targets: vec![vec![Target::Player(1)]] })).unwrap();
    pass(&mut g);
    cite!(["201.4a"]; refused("choose the name of a card with the characteristics the choice says"): g.act(0, name("Lightning Bolt")));
    g.act(0, name("Ornithopter")).unwrap();
    let (_, among, min, max) = search(&g);
    cite!(["701.23a"]; assert_eq!((among.len(), min, max), (5, 0, 5)));
    g.act(0, Move::Choose(Answer::Objects(thopters.to_vec()))).unwrap();
    cite!(["701.23a"]; assert_eq!(exiled(&g, "Ornithopter"), 5));
    cite!(["701.23a"]; assert_eq!(g.players[1].hand.len(), 1, "Dispossess draws nothing; the Bolt stays"));
}

#[test]
fn r701_23a_the_stone_brain_counts_only_cards_exiled_from_the_hand() {
    // "{2}, {T}, Exile The Stone Brain: Choose a card name. Search target opponent's graveyard,
    // hand, and library for up to four cards with that name and exile them. That player
    // shuffles, then draws a card for each card exiled from their hand this way."
    let mut g = game();
    let (_, hand, library) = bolts(&mut g);
    let brain = g.put(0, BRAIN, Zone::Battlefield);
    g.players[0].pool.colorless = 2;
    g.act(0, crate::support::paid("CC", Move::Activate { source: brain, ability: 0, targets: vec![vec![Target::Player(1)]], paying: vec![] })).unwrap();
    pass(&mut g);
    g.act(0, name("Lightning Bolt")).unwrap();
    g.act(0, Move::Choose(Answer::Objects(vec![hand[0], library[0]]))).unwrap();
    cite!(["608.2c", "400.7"]; assert_eq!(g.players[1].hand.len(), 2, "one Bolt left in hand, one drawn"));
    assert!(g.players[1].hand.contains(&hand[1]));
}

#[test]
fn r701_23a_test_of_talents_searches_the_countered_spells_controller_for_its_name() {
    // "Counter target instant or sorcery spell. Search its controller's graveyard, hand, and
    // library for any number of cards with the same name as that spell and exile them. That
    // player shuffles, then draws a card for each card exiled from their hand this way."
    let mut g = game();
    let in_hand = g.put(1, BOLT, Zone::Hand);
    let shock = g.put(1, SHOCK, Zone::Hand);
    let in_library = g.put(1, BOLT, Zone::Library);
    let bolt = g.put(1, BOLT, Zone::Hand);
    g.act(0, Move::Pass).unwrap();
    g.players[1].pool.colors = [0, 0, 0, 1, 0];
    g.act(1, crate::support::paid("R", Move::Cast { card: bolt, targets: vec![vec![Target::Player(0)]] })).unwrap();
    let spell = *g.stack.last().unwrap();
    g.act(1, Move::Pass).unwrap();
    let talents = g.put(0, TALENTS, Zone::Hand);
    g.players[0].pool.colors = [0, 2, 0, 0, 0];
    g.act(0, crate::support::paid("UU", Move::Cast { card: talents, targets: vec![vec![Target::Object(spell)]] })).unwrap();
    pass(&mut g);
    let (_, among, _, _) = search(&g);
    let countered = g.players[1].graveyard.iter().copied().find(|o| g.obj(*o).chars.name == "Lightning Bolt").expect("the countered Bolt");
    // the countered card is in the graveyard by then, and has that name (701.23a, 201.4)
    cite!(["701.23a", "608.2h"]; assert_eq!(among.len(), 3, "{among:?}"));
    assert!(among.contains(&countered) && among.contains(&in_hand) && among.contains(&in_library) && !among.contains(&shock));
    g.act(0, Move::Choose(Answer::Objects(among))).unwrap();
    cite!(["701.23a"]; assert_eq!(exiled(&g, "Lightning Bolt"), 3));
    cite!(["608.2c", "400.7"]; assert_eq!(g.players[1].hand.len(), 2, "the Shock, and one drawn for the Bolt from hand"));
    assert_eq!(g.players[0].life, 20);
}

#[test]
fn r701_23a_the_end_searches_the_exiled_creatures_controller_for_its_name() {
    // "Exile target creature or planeswalker. Search its controller's graveyard, hand, and
    // library for any number of cards with the same name as that permanent and exile them.
    // That player shuffles, then draws a card for each card exiled from their hand this way."
    let mut g = game();
    let bears = g.put(1, BEARS, Zone::Battlefield);
    let more = [g.put(1, BEARS, Zone::Graveyard), g.put(1, BEARS, Zone::Hand), g.put(1, BEARS, Zone::Library)];
    let end = g.put(0, END, Zone::Hand);
    g.players[0].pool.colors = [0, 0, 4, 0, 0];
    g.act(0, crate::support::paid("BBBB", Move::Cast { card: end, targets: vec![vec![Target::Object(bears)]] })).unwrap();
    pass(&mut g);
    let (_, among, _, _) = search(&g);
    cite!(["701.23a", "608.2h"]; assert_eq!(among.len(), 3, "the other three, by the exiled permanent's name"));
    assert!(more.iter().all(|o| among.contains(o)));
    g.act(0, Move::Choose(Answer::Objects(vec![more[1]]))).unwrap();
    cite!(["701.23a"]; assert_eq!(exiled(&g, "Grizzly Bears"), 2));
    cite!(["608.2c"]; assert_eq!(g.players[1].hand.len(), 1, "one drawn for the one from hand"));
}

/// Player 0 casts Pick the Brain at player 1, who reveals their hand; player 0 chooses `card`.
fn pick_the_brain(g: &mut Game, card: ObjId) {
    let c = g.put(0, PICK, Zone::Hand);
    let spending = "BBB";
    g.players[0].pool.colors = [0, 0, 3, 0, 0];
    g.act(0, crate::support::paid(spending, Move::Cast { card: c, targets: vec![vec![Target::Player(1)]] })).unwrap();
    pass(g);
    if let Some(Decision::Choose { player, choice: Choice::Objects { among, .. } | Choice::Select { among, .. } }) = g.decision.clone() {
        cite!(["608.2d"]; assert_eq!(player, 0, "you choose a nonland card from it"));
        assert!(among.contains(&card) && among.iter().all(|o| g.obj(*o).chars.name != "Island"));
        g.act(0, Move::Choose(Answer::Objects(vec![card]))).unwrap();
    }
}

#[test]
fn r608_2c_pick_the_brain_without_delirium_exiles_the_chosen_card_only() {
    // "Target opponent reveals their hand. You choose a nonland card from it and exile that
    // card. / Delirium — If there are four or more card types among cards in your graveyard,
    // search that player's graveyard, hand, and library for any number of cards with the same
    // name as the exiled card, exile those cards, then that player shuffles."
    let mut g = game();
    let (_, hand, _) = bolts(&mut g);
    g.put(1, BEARS, Zone::Hand);
    pick_the_brain(&mut g, hand[0]);
    cite!(["608.2c"]; assert_eq!(exiled(&g, "Lightning Bolt"), 1));
    assert_eq!(named(&g, &g.players[1].hand, "Lightning Bolt"), 1);
    assert!(g.decision.is_none() && g.stack.is_empty(), "no search without delirium: {:?}", g.decision);
}

#[test]
fn r608_2c_pick_the_brain_with_delirium_searches_for_the_exiled_cards_name() {
    // the second paragraph names the first's target and the card it exiled (608.2c)
    let mut g = game();
    for card in [ISLAND, BEARS, SHOCK, ORNITHOPTER] { g.put(0, card, Zone::Graveyard); }
    let (grave, hand, library) = bolts(&mut g);
    let bears = g.put(1, BEARS, Zone::Hand);
    pick_the_brain(&mut g, hand[0]);
    cite!(["608.2c"]; assert_eq!(exiled(&g, "Lightning Bolt"), 1, "the chosen card first"));
    let (cards, among, min, max) = search(&g);
    cite!(["701.23a"]; assert!(cards.contains(&library[0]) && cards.contains(&bears) && cards.contains(&grave[0])));
    cite!(["201.4"]; assert_eq!(among.len(), 4, "the other Bolts: with the same name as the exiled card"));
    cite!(["701.23b"]; assert_eq!((min, max), (0, 4)));
    g.act(0, Move::Choose(Answer::Objects(among))).unwrap();
    cite!(["608.2c"]; assert_eq!(exiled(&g, "Lightning Bolt"), 5));
    assert_eq!(named(&g, &g.players[1].hand, "Grizzly Bears"), 1);
    cite!(["701.24a"]; assert!(g.log.iter().any(|e| matches!(e, game::Event::Shuffled { player: 1, .. }))));
}
