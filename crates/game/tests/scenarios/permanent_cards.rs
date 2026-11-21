//! Permanent cards (110.4a) describe types, not cards that are already on the battlefield.
use crate::support::{cite, paid};
use game::legal::Asked;
use game::state::{Answer, Choice, Decision};
use game::{CardDef, Game, Move, Target, Zone};

fn card(name: &str, types: &str, text: &str) -> CardDef {
    CardDef::new(name, "{0}", types, text, types.contains("Creature").then_some((2, 2))).unwrap()
}

#[test]
fn r110_4a_permanent_cards_have_one_of_six_types_and_kindred_depends_on_its_other_type() {
    let mut defs = vec![card("Restore", "Sorcery", "Return target permanent card from your graveyard to the battlefield.")];
    for (name, types) in [
        ("Artifact", "Artifact"), ("Battle", "Battle"), ("Creature", "Creature — Bear"),
        ("Enchantment", "Enchantment"), ("Land", "Land"), ("Planeswalker", "Planeswalker"),
        ("Instant", "Instant"), ("Sorcery", "Sorcery"),
        ("Kindred Instant", "Kindred Instant — Elf"), ("Kindred Enchantment", "Kindred Enchantment — Elf"),
        ("Artifact Creature", "Artifact Creature — Golem"),
    ] { defs.push(card(name, types, "")); }
    game::play::check(&defs[0]).unwrap();
    // Classification does not claim support for resolving planeswalkers or battles.
    let mut g = Game::new(defs, 2, 20);
    for def in 1..g.defs.len() { g.put(0, def, Zone::Graveyard); }
    g.put(1, 1, Zone::Graveyard);
    g.put(0, 1, Zone::Battlefield);
    let spell = g.put(0, 0, Zone::Hand);
    for target in g.players[0].graveyard.iter().copied() {
        let permanent = ["Artifact", "Artifact Creature", "Battle", "Creature", "Enchantment", "Kindred Enchantment", "Land", "Planeswalker"].contains(&g.obj(target).chars.name.as_str());
        let result = g.clone().act(0, Move::Cast { card: spell, targets: vec![vec![Target::Object(target)]] });
        cite!(["110.4", "110.4a"]; case: g.obj(target).chars.name.clone(); assert_eq!(result.is_ok(), permanent));
    }
    let instant = g.players[0].graveyard[6];
    cite!(["110.4a"]; refused("an illegal target, Instant: it isn't a permanent card (115.1)"): g.act(0, Move::Cast { card: spell, targets: vec![vec![Target::Object(instant)]] }));
    assert_eq!(g.obj(spell).zone, Zone::Hand);
}

#[test]
fn r110_4a_a_search_for_a_permanent_card_finds_a_land_but_not_an_instant() {
    let defs = vec![
        card("Seek", "Sorcery", "Search your library for a permanent card, reveal it, put it into your hand, then shuffle."),
        card("Land", "Land", ""), card("Instant", "Instant", ""),
    ];
    for d in &defs { game::play::check(d).unwrap(); }
    let mut g = Game::new(defs, 2, 20);
    let land = g.put(0, 1, Zone::Library);
    g.put(0, 2, Zone::Library);
    let spell = g.put(0, 0, Zone::Hand);
    g.act(0, Move::Cast { card: spell, targets: vec![] }).unwrap();
    for _ in 0..2 { g.act(g.priority, Move::Pass).unwrap(); }
    cite!(["110.4a"]; assert!(matches!(&g.decision, Some(Decision::Choose { choice: Choice::Search { among, min: 0, max: 1, .. }, .. }) if *among == vec![land])));
    g.act(0, Move::Choose(Answer::Objects(vec![land]))).unwrap();
    assert_eq!(g.obj(g.players[0].hand[0]).chars.name, "Land");
}

#[test]
fn r110_4a_sun_titan_returns_a_low_cost_permanent_card_including_a_land() {
    let defs = vec![
        CardDef::new("Sun Titan", "{4}{W}{W}", "Creature — Giant", "Vigilance\nWhenever this creature enters or attacks, you may return target permanent card with mana value 3 or less from your graveyard to the battlefield.", Some((6, 6))).unwrap(),
        CardDef::new("Plains", "", "Basic Land — Plains", "", None).unwrap(),
        CardDef::new("Large", "{4}", "Artifact", "", None).unwrap(),
        card("Small", "Instant", ""),
    ];
    for d in &defs { game::play::check(d).unwrap(); }
    let mut g = Game::new(defs, 2, 20);
    for _ in 0..6 {
        let land = g.put(0, 1, Zone::Battlefield);
        g.act(0, Move::Activate { source: land, ability: 0, targets: vec![], paying: vec![] }).unwrap();
    }
    let land = g.put(0, 1, Zone::Graveyard);
    g.put(0, 2, Zone::Graveyard);
    g.put(0, 3, Zone::Graveyard);
    g.put(1, 1, Zone::Graveyard);
    let spell = g.put(0, 0, Zone::Hand);
    g.act(0, paid("WWWWWW", Move::Cast { card: spell, targets: vec![] })).unwrap();
    for _ in 0..2 { g.act(g.priority, Move::Pass).unwrap(); }
    let Some((0, Asked::Targets { slots, .. })) = g.asked() else { panic!("Sun Titan's trigger should ask for a target") };
    cite!(["110.4a"]; assert_eq!(slots[0].among, vec![Target::Object(land)]));
    g.act(0, Move::ChooseTargets(vec![vec![Target::Object(land)]])).unwrap();
    for _ in 0..2 { g.act(g.priority, Move::Pass).unwrap(); }
    g.act(0, Move::Choose(Answer::Yes)).unwrap();
    assert_eq!(g.battlefield.len(), 8);
    assert_eq!(g.obj(land).zone, Zone::Gone);
    assert_eq!(g.players[0].graveyard.len(), 2);
    assert_eq!(g.players[1].graveyard.len(), 1);
}

#[test]
fn r110_4a_a_permanent_card_in_a_graveyard_is_not_a_permanent_to_destroy() {
    // Uncited (grammar): destroy operates on battlefield permanents, not graveyard cards.
    crate::support::refused(CardDef::new("Wrong", "{0}", "Instant", "Destroy target permanent card from a graveyard.", None),"text the grammar does not read");
}
