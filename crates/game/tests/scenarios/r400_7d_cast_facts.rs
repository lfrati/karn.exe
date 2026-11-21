//! A permanent retains who cast its spell, but a later zone change clears the fact (400.7d).
use crate::support::{cite, paid};
use game::{CardDef, Game, Move, Target, Zone};
fn setup() -> Game {
    let defs = vec![
        CardDef::new(
            "Cast witness",
            "{0}",
            "Creature — Bear",
            "When this creature enters, if you cast it, you gain 1 life.",
            Some((2, 2)),
        )
        .unwrap(),
        CardDef::new(
            "Return witness",
            "{0}",
            "Sorcery",
            "Return target creature card from your graveyard to the battlefield.",
            None,
        )
        .unwrap(),
    ];
    for d in &defs {
        game::play::check(d).unwrap();
    }
    Game::new(defs, 2, 20)
}
fn resolve(g: &mut Game) {
    for _ in 0..2 {
        g.act(g.priority, Move::Pass).unwrap();
    }
}
#[test]
fn r400_7d_casting_information_survives_entry_but_not_reanimation() {
    let mut g = setup();
    let card = g.put(0, 0, Zone::Hand);
    g.act(0, Move::Cast { card, targets: vec![] }).unwrap();
    resolve(&mut g);
    assert_eq!(g.stack.len(), 1);
    resolve(&mut g);
    cite!(["400.7d"]; assert_eq!(g.players[0].life, 21));
    let permanent = g.battlefield[0];
    g.move_object(permanent, Zone::Graveyard);
    let dead = g.players[0].graveyard[0];
    let spell = g.put(0, 1, Zone::Hand);
    g.act(0, Move::Cast { card: spell, targets: vec![vec![Target::Object(dead)]] }).unwrap();
    resolve(&mut g);
    assert!(g.stack.is_empty());
    cite!(["400.7"]; assert_eq!(g.players[0].life, 21));
    assert_eq!(g.battlefield.len(), 1);
}
#[test]
fn r400_7d_putting_a_card_onto_the_battlefield_does_not_invent_a_caster() {
    let mut g = setup();
    let dead = g.put(0, 0, Zone::Graveyard);
    let spell = g.put(0, 1, Zone::Hand);
    g.act(0, Move::Cast { card: spell, targets: vec![vec![Target::Object(dead)]] }).unwrap();
    resolve(&mut g);
    assert!(g.stack.is_empty());
    cite!(["400.7d", "603.4"]; assert_eq!(g.players[0].life, 20));
    assert_eq!(g.battlefield.len(), 1);
}
#[test]
fn r400_7d_caster_is_distinct_from_a_later_controller() {
    let d = CardDef::new(
        "Cast witness",
        "{0}",
        "Creature — Bear",
        "{0}: If you cast this creature, you gain 1 life.",
        Some((2, 2)),
    )
    .unwrap();
    let steal = CardDef::new("Steal", "{0}", "Sorcery", "Gain control of target creature.", None).unwrap();
    for d in [&d, &steal] {
        game::play::check(d).unwrap();
    }
    let mut g = Game::new(vec![d, steal], 2, 20);
    let card = g.put(0, 0, Zone::Hand);
    g.act(0, Move::Cast { card, targets: vec![] }).unwrap();
    resolve(&mut g);
    let source = g.battlefield[0];
    g.act(0, Move::Activate { source, ability: 0, targets: vec![], paying: vec![] }).unwrap();
    resolve(&mut g);
    cite!(["400.7d"]; assert_eq!(g.players[0].life, 21));
    g.active = 1;
    g.priority = 1;
    let spell = g.put(1, 1, Zone::Hand);
    g.act(1, Move::Cast { card: spell, targets: vec![vec![Target::Object(source)]] }).unwrap();
    resolve(&mut g);
    g.act(1, Move::Activate { source, ability: 0, targets: vec![], paying: vec![] }).unwrap();
    resolve(&mut g);
    cite!(["400.7d"]; assert_eq!(g.players[1].life, 20));
}

// Rules: 201.5, 400.7d, 603.4, 603.3a, 109.5, 707.2, 707.9
#[test]
fn r400_7d_ran_and_shaw_personal_self_reference_keeps_cast_fact_and_trigger_controller() {
    // Ran and Shaw is one object. Its own plural proper name makes "them" a
    // personal self-reference, not a group or a player. The copied token was never cast.
    for mode in ["cast", "reanimate", "cast_then_change_control"] {
        let defs = vec![crate::support::oracle("Ran and Shaw"),
            CardDef::new("Grave Dragon", "{0}", "Creature — Dragon", "", Some((1, 1))).unwrap(),
            CardDef::new("Return witness", "{0}", "Sorcery", "Return target creature card from your graveyard to the battlefield.", None).unwrap(),
            CardDef::new("Borrow", "{0}", "Instant", "Gain control of target creature.", None).unwrap()];
        let mut g = Game::new(defs, 2, 20);
        g.part = ir::Part::PrecombatMain;
        for _ in 0..3 { g.put(0, 1, Zone::Graveyard); }
        if mode == "reanimate" {
            let ran = g.put(0, 0, Zone::Graveyard);
            let spell = g.put(0, 2, Zone::Hand);
            g.act(0, Move::Cast { card: spell, targets: vec![vec![Target::Object(ran)]] }).unwrap();
            resolve(&mut g);
            cite!(["400.7d", "603.4"]; assert!(g.stack.is_empty()));
        } else {
            let ran = g.put(0, 0, Zone::Hand);
            g.players[0].pool.colors[ir::Color::Red as usize] = 2;
            g.players[0].pool.colorless = 3;
            g.act(0, paid("CCCRR", Move::Cast { card: ran, targets: vec![] })).unwrap();
            resolve(&mut g);
            cite!(["201.5", "400.7d", "603.4"]; assert_eq!(g.stack.len(), 1));
            if mode == "cast_then_change_control" {
                let ran = g.battlefield[0];
                g.act(0, Move::Pass).unwrap();
                let borrow = g.put(1, 3, Zone::Hand);
                g.act(1, Move::Cast { card: borrow, targets: vec![vec![Target::Object(ran)]] }).unwrap();
                resolve(&mut g);
                cite!(["603.3a", "109.5"]; assert_eq!(g.controller(ran), 1));
                cite!(["603.3a"]; assert_eq!(g.stack.len(), 1));
            }
            resolve(&mut g);
        }
        let copies: Vec<_> = g.battlefield.iter().copied().filter(|card| g.obj(*card).token()).collect();
        cite!(["400.7d", "603.4"]; assert_eq!(g.battlefield.len(), if mode == "reanimate" { 1 } else { 2 }));
        cite!(["400.7d", "603.4"]; assert_eq!(copies.len(), usize::from(mode != "reanimate")));
        for copy in copies {
            cite!(["603.3a", "109.5"]; assert_eq!(g.controller(copy), 0));
            cite!(["707.2", "707.9"]; assert_eq!((g.obj(copy).chars.name.as_str(), g.power(copy), g.toughness(copy)), ("Ran and Shaw", Some(4), Some(4))));
            cite!(["707.9"]; assert!(!g.obj(copy).chars.supertypes.contains(&ir::Supertype::Legendary)));
        }
        cite!(["400.7d", "603.4"]; assert!(g.stack.is_empty(), "the uncast token's own entry must not create another copy"));
    }
}
