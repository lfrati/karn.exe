//! "Look at target player's hand. You may choose a nonland card from it. If you do, …": whether
//! to choose and which card are one choice, made as the spell or ability resolves (608.2d),
//! among the cards that fit. "If you do" asks whether a card was chosen (608.2c); "if you
//! don't" asks the same question, though a discard in between is a later outcome.
use crate::support;
use support::*;

/// The resolving controller is asked for at most one of these cards.
fn offered(t: &mut T, p: usize) -> Vec<ObjId> {
    let Some((asked, Asked::OptionalObjects { among, count: 1, .. })) = t.asked() else { panic!("an optional card is asked: {:?}", t.asked()) };
    assert_eq!(asked, p, "player {p} chooses");
    among
}
fn hand(t: &T, p: usize) -> Vec<String> {
    let mut names: Vec<_> = t.g.players[p].hand.iter().map(|o| t.g.obj(*o).chars.name.clone()).collect();
    names.sort();
    names
}

// Rules: 608.2c, 608.2d, 701.20a
// Given: Vendilion Clique cast with flash; the opponent holds Lightning Bolt and a Mountain,
// an Island on top of their library.
// When: its enters trigger targets the opponent and its controller chooses Lightning Bolt.
// Expect: only the nonland card is offered; the opponent reveals it, puts it on the bottom of
// their library and draws the Island.
#[test]
fn r608_2d_vendilion_clique_puts_the_chosen_card_on_the_bottom_and_its_owner_draws() {
    let mut t = Board::new(2).hand(0, &["Vendilion Clique"]).hand(1, &["Lightning Bolt", "Mountain"])
        .library(1, &["Island", "Forest"]).build();
    t.mana(0, "{1}{U}{U}").cast(0, "Vendilion Clique", vec![], mana_pool("CUU")).unwrap();
    t.pass_all();
    let Some((0, Asked::Targets { .. })) = t.asked() else { panic!("its trigger asks for a target: {:?}", t.asked()) };
    t.g.act(0, Move::ChooseTargets(vec![vec![Target::Player(1)]])).unwrap();
    t.pass_all();
    let bolt = t.find_in(1, Zone::Hand, "Lightning Bolt");
    cite!(["608.2d"]; assert_eq!(offered(&mut t, 0), vec![bolt], "a land is not a nonland card"));
    t.answer(0, Answer::OptionalObjects(Some(vec![bolt]))).unwrap();
    t.resolve();
    cite!(["701.20a"]; assert!(t.g.log.iter().any(|e| matches!(e, game::Event::Revealed { cards, .. } if cards == &vec![bolt]))));
    t.expect().rules(["608.2c"]).zone_ordered(1, Zone::Library, &["Forest", "Lightning Bolt"]).check();
    cite!(["121.1"]; assert_eq!(hand(&t, 1), vec!["Island", "Mountain"]));
}

// Rules: 608.2c, 608.2d
// Given: the same Vendilion Clique trigger.
// When: its controller chooses no card.
// Expect: nothing is put on the bottom and the opponent draws nothing.
#[test]
fn r608_2c_vendilion_clique_choosing_nothing_leaves_the_hand_and_draws_nothing() {
    let mut t = Board::new(2).hand(0, &["Vendilion Clique"]).hand(1, &["Lightning Bolt"]).library(1, &["Island"]).build();
    t.mana(0, "{1}{U}{U}").cast(0, "Vendilion Clique", vec![], mana_pool("CUU")).unwrap();
    t.pass_all();
    t.g.act(0, Move::ChooseTargets(vec![vec![Target::Player(1)]])).unwrap();
    t.pass_all();
    offered(&mut t, 0);
    cite!(["608.2d"]; ok: t.answer(0, Answer::OptionalObjects(None)));
    t.resolve();
    cite!(["608.2c"]; assert_eq!((hand(&t, 1), t.g.players[1].library.len()), (vec!["Lightning Bolt".to_string()], 1)));
}

// Rules: 608.2c
// Given: Nightsnare aimed at an opponent holding Lightning Bolt, Giant Growth and a Mountain.
// When: its caster chooses Lightning Bolt; then, in a second game, chooses nothing.
// Expect: the chosen card alone is discarded; choosing nothing, that player discards two
// cards of their choice.
#[test]
fn r608_2c_nightsnare_discards_the_chosen_card_or_two_of_the_players_choice() {
    let board = || Board::new(2).hand(0, &["Nightsnare"]).hand(1, &["Lightning Bolt", "Giant Growth", "Mountain"]).build();
    let mut t = board();
    t.mana(0, "{3}{B}").cast(0, "Nightsnare", vec![player(1)], mana_pool("CCCB")).unwrap();
    t.pass_all();
    let bolt = t.find_in(1, Zone::Hand, "Lightning Bolt");
    assert_eq!(offered(&mut t, 0).len(), 2, "the two nonland cards");
    t.answer(0, Answer::OptionalObjects(Some(vec![bolt]))).unwrap();
    t.resolve();
    cite!(["608.2c"]; assert_eq!(hand(&t, 1), vec!["Giant Growth", "Mountain"]));

    let mut t = board();
    t.mana(0, "{3}{B}").cast(0, "Nightsnare", vec![player(1)], mana_pool("CCCB")).unwrap();
    t.pass_all();
    offered(&mut t, 0);
    t.answer(0, Answer::OptionalObjects(None)).unwrap();
    let Some((1, Asked::Objects { count: 2, .. })) = t.asked() else { panic!("the opponent picks two discards: {:?}", t.asked()) };
    cite!(["608.2c"]; ok: t.choose(1, &["Lightning Bolt", "Mountain"]));
    if let Some((1, Asked::OrderCards { .. })) = t.asked() { t.choose(1, &["Lightning Bolt", "Mountain"]).unwrap(); }
    t.resolve();
    assert_eq!(hand(&t, 1), vec!["Giant Growth"]);
}

// Rules: 608.2c
// Given: Reckoner Shakedown aimed at an opponent holding Lightning Bolt; its caster controls
// Grizzly Bears.
// When: its caster chooses nothing.
// Expect: no card is discarded and the Bears get two +1/+1 counters.
#[test]
fn r608_2c_reckoner_shakedown_choosing_nothing_puts_two_counters() {
    let mut t = Board::new(2).hand(0, &["Reckoner Shakedown"]).battlefield(0, &["Grizzly Bears"]).hand(1, &["Lightning Bolt"]).build();
    t.mana(0, "{2}{B}").cast(0, "Reckoner Shakedown", vec![player(1)], mana_pool("CCB")).unwrap();
    t.pass_all();
    offered(&mut t, 0);
    t.answer(0, Answer::OptionalObjects(None)).unwrap();
    t.resolve();
    t.expect().rules(["608.2c"]).counters("Grizzly Bears", "+1/+1", 2).zone(1, Zone::Hand, &["Lightning Bolt"]).check();
}

// Rules: 608.2c
// Given: Reckoner Shakedown aimed at an opponent holding Lightning Bolt; its caster controls
// Grizzly Bears.
// When: its caster chooses Lightning Bolt.
// Expect: the opponent discards it and no counters are put.
#[test]
fn r608_2c_reckoner_shakedown_the_chosen_card_is_discarded_and_no_counters() {
    let mut t = Board::new(2).hand(0, &["Reckoner Shakedown"]).battlefield(0, &["Grizzly Bears"]).hand(1, &["Lightning Bolt"]).build();
    t.mana(0, "{2}{B}").cast(0, "Reckoner Shakedown", vec![player(1)], mana_pool("CCB")).unwrap();
    t.pass_all();
    let bolt = t.find_in(1, Zone::Hand, "Lightning Bolt");
    t.answer(0, Answer::OptionalObjects(Some(vec![bolt]))).unwrap();
    t.resolve();
    t.expect().rules(["608.2c"]).counters("Grizzly Bears", "+1/+1", 0).zone(1, Zone::Graveyard, &["Lightning Bolt"]).check();
}

// Rules: 608.2c, 608.2d
// Given: Extract the Truth, first mode, aimed at an opponent holding Grizzly Bears and
// Lightning Bolt.
// When: its caster chooses nothing, though the Bears could be chosen.
// Expect: the Bears alone are offered and nothing is discarded (its ruling: "you don't have to
// choose a card").
#[test]
fn r608_2d_extract_the_truth_may_choose_no_card() {
    let mut t = Board::new(2).hand(0, &["Extract the Truth"]).hand(1, &["Grizzly Bears", "Lightning Bolt"]).build();
    t.mana(0, "{1}{B}");
    let spell = t.find_in(0, Zone::Hand, "Extract the Truth");
    t.g.act(0, paid("CB", Move::Modes(vec![0], Box::new(Move::Cast { card: spell, targets: vec![vec![Target::Player(1)]] })))).unwrap();
    t.pass_all();
    let bears = t.find_in(1, Zone::Hand, "Grizzly Bears");
    cite!(["608.2d"]; assert_eq!(offered(&mut t, 0), vec![bears]));
    t.answer(0, Answer::OptionalObjects(None)).unwrap();
    t.resolve();
    cite!(["608.2c"]; assert_eq!(hand(&t, 1), vec!["Grizzly Bears", "Lightning Bolt"]));
}

// Rules: 608.2c
// Given: Specter's Shriek aimed at an opponent holding Lightning Bolt; its caster holds a
// Swamp.
// When: its caster chooses Lightning Bolt.
// Expect: the opponent exiles it; a nonblack card was exiled, so the caster exiles the Swamp.
#[test]
fn r608_2c_specters_shriek_a_nonblack_exile_costs_its_caster_a_card() {
    let mut t = Board::new(2).hand(0, &["Specter's Shriek", "Swamp"]).hand(1, &["Lightning Bolt"]).build();
    t.mana(0, "{B}").cast(0, "Specter's Shriek", vec![player(1)], mana_pool("B")).unwrap();
    t.pass_all();
    let bolt = t.find_in(1, Zone::Hand, "Lightning Bolt");
    t.answer(0, Answer::OptionalObjects(Some(vec![bolt]))).unwrap();
    t.resolve();
    t.expect().rules(["608.2c"]).zone(1, Zone::Exile, &["Lightning Bolt"]).zone(0, Zone::Exile, &["Swamp"]).check();
}
