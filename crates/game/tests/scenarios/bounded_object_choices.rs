//! CR 118.12/608.2d: bounded object choices retain consent and actual results.
use crate::support::{cite, oracle};
use game::{CardDef, Game, Move, Zone};
use game::state::{Answer, Choice, Decision};

fn resolve_until_question(g: &mut Game) {
    for _ in 0..12 {
        if g.decision.is_some() || g.stack.is_empty() { return; }
        g.act(g.priority, Move::Pass).unwrap();
    }
    panic!("resolution did not reach its boundary");
}

fn setup(text: &str, held: usize) -> (Game, Vec<game::ObjId>) {
    let spell = CardDef::new("Bounded witness", "{0}", "Sorcery", text, None).unwrap();
    game::play::check(&spell).unwrap();
    let mut g = Game::new(vec![spell, oracle("Forest")], 2, 20);
    let cards = (0..held).map(|_| g.put(0, 1, Zone::Hand)).collect();
    for _ in 0..5 { g.put(0, 1, Zone::Library); }
    let card = g.put(0, 0, Zone::Hand);
    g.act(0, Move::Cast { card, targets: vec![] }).unwrap();
    resolve_until_question(&mut g);
    (g, cards)
}

fn finish_order(g: &mut Game, chosen: &[game::ObjId]) {
    if matches!(g.decision, Some(Decision::Choose { choice: Choice::OrderCards { .. }, .. })) {
        g.act(0, Move::Choose(Answer::Objects(chosen.to_vec()))).unwrap();
    }
}

#[test]
fn r608_2d_bounded_discard_chooses_zero_one_or_two_and_draws_the_actual_count() {
    for held in 0..=3 {
        for n in 0..=held.min(2) {
            let (mut g, cards) = setup("You may discard up to two cards. If you do, draw that many cards.", held);
            g.act(0, Move::Choose(Answer::Yes)).unwrap();
            if held > 0 {
                cite!(["608.2d"]; assert!(matches!(g.decision, Some(Decision::Choose { choice: Choice::ConstrainedObjects { .. }, .. }))));
                g.act(0, Move::Choose(Answer::Objects(cards[..n].to_vec()))).unwrap();
                finish_order(&mut g, &cards[..n]);
            }
            cite!(["701.9a", "608.2c"]; assert_eq!(g.players[0].hand.len(), held));
            cite!(["608.2c"]; assert_eq!(g.players[0].library.len(), 5 - n));
            assert!(g.resolving().is_none(), "{:?}", g.decision);
        }
    }
}

#[test]
fn r608_2d_bounded_choice_rejects_excess_duplicates_and_other_players_without_changes() {
    let (mut g, cards) = setup("You may discard up to two cards. If you do, draw that many cards.", 3);
    g.act(0, Move::Choose(Answer::Yes)).unwrap();
    let before = format!("{g:?}");
    for (player, chosen) in [(0, cards.clone()), (0, vec![cards[0], cards[0]]), (1, vec![cards[0]])] {
        let expected = if player == 0 { "choose up to 2 distinct offered objects (608.2d)" } else { "that decision is another player's" };
        assert_eq!(g.act(player, Move::Choose(Answer::Objects(chosen))).unwrap_err(), expected);
        cite!(["608.2d", "701.9b"]; assert_eq!(format!("{g:?}"), before));
    }
    let mut fork = g.clone();
    g.act(0, Move::Choose(Answer::Objects(vec![]))).unwrap();
    fork.act(0, Move::Choose(Answer::Objects(vec![cards[0]]))).unwrap();
    cite!(["608.2c"]; assert_eq!((g.players[0].library.len(), fork.players[0].library.len()), (5, 4)));
}

#[test]
fn r118_12_accepting_an_empty_optional_cost_is_distinct_from_declining_it() {
    let (mut g, _) = setup("You may discard up to two cards. If you do, you gain 1 life.", 0);
    let mut declined = g.clone();
    g.act(0, Move::Choose(Answer::Yes)).unwrap();
    declined.act(0, Move::Choose(Answer::No)).unwrap();
    cite!(["118.12"]; assert_eq!((g.players[0].life, declined.players[0].life), (21, 20)));
}

#[test]
fn r701_9b_any_number_uses_the_same_bounded_choice_mechanism() {
    let (mut g, cards) = setup("Discard any number of cards, then draw that many cards.", 3);
    g.act(0, Move::Choose(Answer::Objects(cards.clone()))).unwrap();
    finish_order(&mut g, &cards);
    cite!(["701.9b", "608.2c"]; assert_eq!((g.players[0].hand.len(), g.players[0].library.len()), (3, 2)));
}

#[test]
fn r714_2b_fables_second_chapter_uses_its_full_pinned_definition() {
    for n in 0..=2 {
        let mut g = Game::new(vec![oracle("Fable of the Mirror-Breaker"), oracle("Forest")], 2, 20);
        let saga = g.put(0, 0, Zone::Battlefield);
        g.set_initial_counters(saga, "lore", 1);
        let cards: Vec<_> = (0..3).map(|_| g.put(0, 1, Zone::Hand)).collect();
        for _ in 0..4 { g.put(0, 1, Zone::Library); }
        g.put_counters(saga, "lore", 1);
        g.act(0, Move::Pass).unwrap();
        resolve_until_question(&mut g);
        g.act(0, Move::Choose(Answer::Yes)).unwrap();
        g.act(0, Move::Choose(Answer::Objects(cards[..n].to_vec()))).unwrap();
        finish_order(&mut g, &cards[..n]);
        cite!(["714.2b", "608.2c"]; assert_eq!((g.players[0].hand.len(), g.players[0].library.len()), (3, 4-n)));
    }
}

#[test]
fn r701_21a_bounded_sacrifices_choose_a_subset_and_preserve_the_other_permanents() {
    let spell = CardDef::new("Bounded witness", "{0}", "Sorcery", "Sacrifice up to two creatures.", None).unwrap();
    game::play::check(&spell).unwrap();
    let mut g = Game::new(vec![spell, oracle("Grizzly Bears")], 2, 20);
    let bears: Vec<_> = (0..3).map(|_| g.put(0, 1, Zone::Battlefield)).collect();
    let spell = g.put(0, 0, Zone::Hand);
    g.act(0, Move::Cast { card: spell, targets: vec![] }).unwrap();
    resolve_until_question(&mut g);
    g.act(0, Move::Choose(Answer::Objects(bears[..2].to_vec()))).unwrap();
    finish_order(&mut g, &bears[..2]);
    cite!(["701.21a", "608.2d"]; assert_eq!(g.battlefield, vec![bears[2]]));
}

#[test]
fn r608_2e_bounded_multiplayer_choices_are_collected_before_any_discard_commits() {
    let spell = CardDef::new("Bounded witness", "{0}", "Sorcery", "Each player discards up to one card.", None).unwrap();
    game::play::check(&spell).unwrap();
    let mut g = Game::new(vec![spell, oracle("Forest")], 3, 20);
    let cards: Vec<_> = (0..3).map(|p| g.put(p, 1, Zone::Hand)).collect();
    let spell = g.put(0, 0, Zone::Hand);
    g.act(0, Move::Cast { card: spell, targets: vec![] }).unwrap();
    resolve_until_question(&mut g);
    g.act(0, Move::Choose(Answer::Objects(vec![cards[0]]))).unwrap();
    cite!(["608.2e"]; assert_eq!(g.players[0].hand, vec![cards[0]]));
    g.act(1, Move::Choose(Answer::Objects(vec![]))).unwrap();
    g.act(2, Move::Choose(Answer::Objects(vec![cards[2]]))).unwrap();
    cite!(["608.2e", "701.9a"]; assert_eq!(g.players.iter().map(|p|p.hand.len()).collect::<Vec<_>>(), [0, 1, 0]));
}

#[test]
fn r614_6_discards_replaced_by_exile_still_supply_the_original_discard_count() {
    let spell = CardDef::new("Bounded witness", "{0}", "Sorcery", "Discard up to two cards, then draw that many cards.", None).unwrap();
    let replacement = CardDef::new("Exile witness", "", "Enchantment", "If a card would be put into a graveyard from anywhere, exile it instead.", None).unwrap();
    for def in [&spell, &replacement] { game::play::check(def).unwrap(); }
    let mut g = Game::new(vec![spell, replacement, oracle("Forest")], 2, 20);
    g.put(0, 1, Zone::Battlefield);
    let held = g.put(0, 2, Zone::Hand);
    for _ in 0..3 { g.put(0, 2, Zone::Library); }
    let spell = g.put(0, 0, Zone::Hand);
    g.act(0, Move::Cast { card: spell, targets: vec![] }).unwrap();
    resolve_until_question(&mut g);
    g.act(0, Move::Choose(Answer::Objects(vec![held]))).unwrap();
    cite!(["614.6", "701.9a"]; assert_eq!(g.players[0].library.len(), 2));
    assert!(g.players[0].graveyard.is_empty());
    assert_eq!(g.exile.len(), 2, "the discarded card and the resolved spell");
}
