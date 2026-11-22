//! Cost disclosures belong to each spell/ability on the stack (701.20a), rather
//! than a mutable visibility flag on the revealed card.
use crate::support::{Board, cite, mana_pool, paid};
use game::{Move, Zone};

#[test]
fn r701_20a_two_costs_revealing_the_same_dragon_keep_independent_grants() {
    let mut t = Board::new(2)
        .named(0, Zone::Hand, "first", "Caustic Exhale")
        .named(0, Zone::Hand, "second", "Caustic Exhale")
        .hand(0, &["Shivan Dragon"]).battlefield(1, &["Grizzly Bears"]).build();
    let dragon = t.find_in(0, Zone::Hand, "Shivan Dragon");
    let bear = t.find_in(1, Zone::Battlefield, "Grizzly Bears");
    t.g.players[0].pool = mana_pool("BB");
    for label in ["first", "second"] {
        let card = t.id(label);
        // Oracle: choose the behold branch of "behold a Dragon or pay {1}".
        t.g.act(0, paid("B", Move::WithCostChoice(vec![0], Box::new(Move::WithCosts(vec![vec![dragon]],
            Box::new(Move::Cast { card, targets: vec![vec![game::Target::Object(bear)]] })))))).unwrap();
        cite!(["701.20a"]; assert!(t.g.sees(1, dragon)));
    }
    let mut fork = t.g.clone();
    for g in [&mut t.g, &mut fork] {
        for _ in 0..2 { g.act(g.priority, Move::Pass).unwrap(); }
        assert_eq!(g.stack.len(), 1);
        cite!(["701.20a"]; assert!(g.sees(1, dragon), "the earlier spell still grants visibility"));
        // The first Exhale's target died. It leaves without resolving (608.2b).
        for _ in 0..2 { g.act(g.priority, Move::Pass).unwrap(); }
        cite!(["608.2b", "701.20a"]; assert!(g.stack.is_empty() && !g.sees(1, dragon)));
    }
}

#[test]
fn r701_20a_a_card_revealed_for_an_activated_cost_stays_public_until_the_ability_leaves() {
    let source = game::CardDef::new("Speaker", "{0}", "Artifact",
        "Reveal a creature card from your hand: You gain 1 life.", None).unwrap();
    let card = game::CardDef::new("Witness", "{0}", "Creature — Bear", "", Some((2, 2))).unwrap();
    for def in [&source, &card] { game::play::check(def).unwrap(); }
    let mut g = game::Game::new(vec![source, card], 2, 20);
    let source = g.put(0, 0, Zone::Battlefield);
    let card = g.put(0, 1, Zone::Hand);
    g.act(0, Move::Activate { source, ability: 0, targets: vec![], paying: vec![vec![card]] }).unwrap();
    cite!(["701.20a"]; assert!(g.sees(1, card) && g.stack.len() == 1));
    for _ in 0..2 { g.act(g.priority, Move::Pass).unwrap(); }
    assert_eq!(g.players[0].life, 21);
    cite!(["701.20a"]; assert!(g.stack.is_empty() && !g.sees(1, card)));
}
