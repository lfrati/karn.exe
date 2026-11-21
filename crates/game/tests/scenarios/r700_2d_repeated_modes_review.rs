//! Independent repeated-mode bounds, target groups and local-result composition.
use crate::support::cite;
use game::{CardDef, Game, Move, Target, Zone};
use ir::Part;

fn spell(text: &str) -> CardDef {
    CardDef::new("Repeated", "{0}", "Sorcery", text, None).unwrap()
}

#[test]
fn r700_2d_unbounded_repeated_choices_are_not_admitted_with_a_false_finite_bound() {
    // Rules: 700.2a, 700.2d
    // Without distinctness, "any number" and "one or more" have no finite maximum.
    // The bounded repeated-mode slice must refuse them, rather than cap them at row count.
    let mut admitted = Vec::new();
    for header in ["Choose any number", "Choose one or more"] {
        let text = format!("{header}. You may choose the same mode more than once.\n• You gain 1 life.\n• You gain 2 life.");
        let d = spell(&text);
        if game::play::check(&d).is_ok() { admitted.push(header); }
    }
    assert!(admitted.is_empty(), "unbounded choices admitted with a false row-count maximum: {admitted:?}");
}

#[test]
fn r700_2d_targets_are_distinct_within_each_instance_but_can_repeat_across_instances() {
    // Rules: 700.2d, 115.3, 601.2c
    let d = spell("Choose three. You may choose the same mode more than once.\n• This spell deals 1 damage to two target creatures.\n• You gain 1 life.");
    game::play::check(&d).unwrap();
    let bear = CardDef::new("Bear", "{0}", "Creature — Bear", "", Some((2, 2))).unwrap();
    let mut g = Game::new(vec![d, bear], 2, 20);
    let a = g.put(1, 1, Zone::Battlefield);
    let b = g.put(1, 1, Zone::Battlefield);
    let card = g.put(0, 0, Zone::Hand);
    let arrivals = vec![a,b];
    let a = Target::Object(a);
    let b = Target::Object(b);
    let cast = |targets| Move::Modes(vec![0, 0, 0], Box::new(Move::Cast { card, targets }));
    cite!(["115.3"]; refused("the same target twice for one \"target\" (115.3)"): g.act(0, cast(vec![vec![a, a], vec![a, b], vec![a, b]])));
    cite!(["700.2d"]; ok: g.act(0, cast(vec![vec![a, b]; 3])));
    for _ in 0..2 { g.act(g.priority, Move::Pass).unwrap(); }
    // CR404.3: the owner explicitly orders these simultaneous graveyard arrivals.
    assert_eq!(g.decision, Some(game::state::Decision::Choose { player: 1, choice: game::state::Choice::OrderCards { to: game::state::Placement::Zone(Zone::Graveyard), cards: arrivals.clone() } }));
    g.act(1, Move::Choose(game::state::Answer::Objects(vec![arrivals[1],arrivals[0]]))).unwrap();
    cite!(["700.2d"]; assert!(g.battlefield.is_empty(), "both creatures receive all three damage instances"));
    assert_eq!(g.players[1].graveyard.len(), 2);
}

#[test]
fn r700_2d_each_repeated_creation_has_its_own_later_reference() {
    // Rules: 700.2d, 302.6, 702.10b
    let d = spell("Choose three. You may choose the same mode more than once.\n• Create a 1/1 green Squirrel creature token. It gains haste until end of turn.\n• You gain 1 life.");
    game::play::check(&d).unwrap();
    let mut g = Game::new(vec![d], 2, 20);
    let card = g.put(0, 0, Zone::Hand);
    g.act(0, Move::Modes(vec![0, 0, 0], Box::new(Move::Cast { card, targets: vec![] }))).unwrap();
    for _ in 0..2 { g.act(g.priority, Move::Pass).unwrap(); }
    cite!(["700.2d"]; assert_eq!(g.battlefield.len(), 3));
    for _ in 0..12 {
        if g.part == Part::DeclareAttackers { break; }
        g.act(g.priority, Move::Pass).unwrap();
    }
    assert_eq!(g.part, Part::DeclareAttackers);
    let attacks = g.battlefield.iter().map(|o| (*o, Target::Player(1))).collect();
    cite!(["702.10b"]; ok: g.act(0, Move::Attack(attacks)));
}

#[test]
fn r700_2d_five_choices_have_separate_targets_and_a_finite_listing() {
    // Rules: 700.2a, 700.2c, 700.2d
    let d = spell("Choose five. You may choose the same mode more than once.\n• Target player gains 1 life.\n• Target player gains 2 life.\n• Target player gains 3 life.\n• Target player gains 4 life.\n• Target player gains 5 life.");
    game::play::check(&d).unwrap();
    let mut g = Game::new(vec![d], 2, 20);
    let card = g.put(0, 0, Zone::Hand);
    g.act(0, Move::Modes(vec![4; 5], Box::new(Move::Cast { card, targets: vec![vec![Target::Player(1)]; 5] }))).unwrap();
    for _ in 0..2 { g.act(g.priority, Move::Pass).unwrap(); }
    cite!(["700.2d"]; assert_eq!(g.players[1].life, 45));
}
