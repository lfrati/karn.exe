//! Player targeting keywords have one executable static owner, with honest bounds.
use game::{CardDef, Game, Move, Target, Zone};
use game::select::Ctx;
use crate::support::cite;

fn permanent(name: &str, text: &str) -> CardDef {
    CardDef::new(name, "{0}", "Enchantment", text, None).unwrap()
}

#[test]
fn unsupported_player_grants_refuse_whole_rows() {
    for text in [
        "Players have hexproof.",
        "Each opponent has hexproof.",
        "You have flying.",
        "You have \"{T}: Draw a card.\"",
        "You and creatures you control have hexproof.",
        "You have hexproof and flying.",
        "You have hexproof and gain flying.",
        "You have hexproof for each creature you control.",
    ] {
        let d = permanent("Scope", text);
        let err = game::play::check(&d).expect_err("no accepted player characteristic no-op");
        assert!(err.0.starts_with("player characteristic:") || err.0.starts_with("player keyword:"), "{text}: {}", err.0);
    }
}

#[test]
fn lifetime_objects_hand_size_and_nontargeted_effects_stay_distinct() {
    let defs = vec![
        CardDef::new("Bolt", "{0}", "Instant", "This spell deals 3 damage to any target.", None).unwrap(),
        permanent("Ward", "You have hexproof."),
        CardDef::new("Burn", "{0}", "Instant", "This spell deals 1 damage to each opponent.", None).unwrap(),
        CardDef::new("Bear", "{0}", "Creature — Bear", "", Some((2, 2))).unwrap(),
        permanent("Grant", "Creatures you control have hexproof."),
        permanent("Book", "You have no maximum hand size."),
    ];
    for d in &defs { game::play::check(d).unwrap(); }
    let mut g = Game::new(defs, 2, 20);
    let source = g.put(0, 0, Zone::Hand);
    let ward = g.put(1, 1, Zone::Battlefield);
    let second = g.put(1, 1, Zone::Battlefield);
    let bear = g.put(1, 3, Zone::Battlefield);
    let grant = g.put(1, 4, Zone::Battlefield);
    g.put(1, 5, Zone::Battlefield);
    let by_opponent = Ctx::for_spell(0, source);
    let by_holder = Ctx::for_spell(1, source);
    cite!(["702.11c"]; assert!(!g.can_target(&by_opponent, Target::Player(1))));
    cite!(["702.11b"]; assert!(!g.can_target(&by_opponent, Target::Object(bear))));
    assert!(g.can_target(&by_opponent, Target::Object(ward)));
    cite!(["702.11c"]; assert!(g.can_target(&by_holder, Target::Player(1))));
    cite!(["702.11b"]; assert!(g.can_target(&by_holder, Target::Object(bear))));
    assert_eq!(g.maximum_hand_size(1), None);
    cite!(["402.2"]; assert_eq!(g.maximum_hand_size(0), Some(7)));
    let burn = g.put(0, 2, Zone::Hand);
    g.act(0, Move::Cast { card: burn, targets: vec![] }).unwrap();
    for _ in 0..3 { if g.stack.is_empty() { break; } g.act(g.priority, Move::Pass).unwrap(); }
    cite!(["115.10"]; assert_eq!(g.players[1].life, 19));
    g.exile(ward);
    assert!(!g.can_target(&by_opponent, Target::Player(1)));
    g.exile(second);
    cite!(["702.11c"]; assert!(g.can_target(&by_opponent, Target::Player(1))));
    assert!(!g.can_target(&by_opponent, Target::Object(bear)));
    g.exile(grant);
    cite!(["702.11b"]; assert!(g.can_target(&by_opponent, Target::Object(bear))));
}

#[test]
fn existing_static_conditions_and_shroud_use_the_restriction_path() {
    let defs = vec![
        CardDef::new("Bolt", "{0}", "Instant", "This spell deals 3 damage to any target.", None).unwrap(),
        permanent("Ward", "As long as you control an Island, you have hexproof."),
        CardDef::new("Island", "", "Basic Land — Island", "", None).unwrap(),
        permanent("Mask", "You have shroud."),
    ];
    for d in &defs { game::play::check(d).unwrap(); }
    let mut g = Game::new(defs, 2, 20);
    let spell = g.put(0, 0, Zone::Hand);
    g.put(1, 1, Zone::Battlefield);
    let from0 = Ctx::for_spell(0, spell);
    let from1 = Ctx::for_spell(1, spell);
    assert!(g.can_target(&from0, Target::Player(1)));
    let island = g.put(1, 2, Zone::Battlefield);
    cite!(["702.11c"]; assert!(!g.can_target(&from0, Target::Player(1))));
    cite!(["702.11c"]; assert!(g.can_target(&from1, Target::Player(1))));
    g.exile(island);
    assert!(g.can_target(&from0, Target::Player(1)));
    g.put(1, 3, Zone::Battlefield);
    cite!(["702.18a"]; assert!(!g.can_target(&from0, Target::Player(1))));
    cite!(["702.18a"]; assert!(!g.can_target(&from1, Target::Player(1))));
}
