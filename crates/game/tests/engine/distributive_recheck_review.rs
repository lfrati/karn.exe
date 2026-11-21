//! The per-dealer root "other" refusal and its
//! contrasts, plus the residual plain-plural source. Run with `engine::distributive_recheck_review::`.
use crate::support::cite;
use game::{CardDef, Event, Game, Move, ObjId, Target, Zone};

fn creature(name: &str, p: i32, t: i32, text: &str) -> CardDef { CardDef::new(name, "{0}", "Creature — Beast", text, Some((p, t))).unwrap() }
fn admitted(ty: &str, text: &str) -> Result<(), String> {
    let pt = ty.starts_with("Creature").then_some((1, 4));
    game::play::check(&CardDef::new("Probe", "{0}", ty, text, pt).unwrap()).map_err(|e| e.0)
}

#[test]
fn per_dealer_root_other_recipients_are_refused() {
    for text in [
        "Each creature you control deals damage equal to its power to each other creature.",
        "Each creature you control deals 1 damage to each other creature.",
        "Each creature deals 1 damage to each other creature.",
        "Each creature you control deals 1 damage to all other creatures.",
        "Each creature you control deals 1 damage to each other creature and each opponent.",
        "Up to two target creatures you control each deal 1 damage to each other creature.",
    ] {
        assert_eq!(admitted("Sorcery", text), Err("distributive damage: per-source other recipient set not implemented".into()), "{text}");
    }
}

#[test]
fn neighbouring_shapes_stay_admitted() {
    for (ty, text) in [
        ("Sorcery", "Up to two target creatures you control each deal damage equal to their power to another target creature."),
        ("Sorcery", "Each creature you control deals damage equal to its power to another target creature."),
        ("Sorcery", "Each creature you control deals 1 damage to each creature."),
        ("Sorcery", "Each creature you control deals 1 damage to each creature your opponents control."),
        ("Sorcery", "Target creature you control deals damage equal to its power to each other creature."),
        ("Creature — Beast", "{T}: This creature deals 1 damage to each other creature."),
        ("Creature — Beast", "{T}: Each other creature you control deals 1 damage to target creature."),
    ] {
        admitted(ty, text).unwrap_or_else(|e| panic!("{text}: {e}"));
    }
}

/// Plain/all plural sources distribute damage too (120.3). Their own power must bind
/// each member rather than falling through to the recipient.
#[test]
fn plain_plural_source_reads_each_members_own_power() {
    for text in ["Creatures you control deal damage equal to their power to each creature your opponents control.",
                 "All creatures you control deal damage equal to their power to each creature your opponents control."] {
        let d = CardDef::new("Probe", "{0}", "Sorcery", text, None).unwrap();
        game::play::check(&d).unwrap();
        let mut g = Game::new(vec![d, creature("Big", 3, 5, ""), creature("Small", 2, 9, ""), creature("Wall", 1, 20, "")], 2, 20);
        let a = g.put(0, 1, Zone::Battlefield);
        let b = g.put(0, 2, Zone::Battlefield);
        let w = g.put(1, 3, Zone::Battlefield);
        let c = g.put(0, 0, Zone::Hand);
        g.act(0, Move::Cast { card: c, targets: vec![] }).unwrap();
        for _ in 0..3 { if g.stack.is_empty() { break; } g.act(g.priority, Move::Pass).unwrap(); }
        let dealt: Vec<(ObjId, Target, u32)> = g.log.iter().filter_map(|e| match e {
            Event::DamageDealt { source, to, amount, .. } => Some((*source, *to, *amount)),
            _ => None,
        }).collect();
        // each dealing creature is its damage's source (120.1) and its quantity is read as the effect applies (608.2h); "its/their" naming each dealer is the card text's English
        cite!(["120.1", "608.2h"]; case: text; assert_eq!(dealt, vec![(a, Target::Object(w), 3), (b, Target::Object(w), 2)], "{text}"));
    }
}


/// Plain/all source groups share the same refused dealer-relative "other" and member/player
/// boundaries; the fixed or explicit amounts remain independent.
#[test]
fn plain_plural_boundaries_and_explicit_quantities() {
    for subject in ["Creatures you control", "All creatures you control"] {
        for amount in ["1 damage", "damage equal to their power"] {
            let text = format!("{subject} deal {amount} to each other creature.");
            assert_eq!(admitted("Sorcery", &text), Err(
                "distributive damage: per-source other recipient set not implemented".into()), "{text}");
        }
        for recipient in ["target player", "any target", "each opponent"] {
            let text = format!("{subject} deal damage equal to their power to {recipient}.");
            assert_eq!(admitted("Sorcery", &text), Err(
                "per-source object quantity in damage a player may be dealt".into()), "{text}");
        }
        for text in [format!("{subject} deal 2 damage to each opponent."),
            format!("{subject} deal damage equal to the total power of creatures you control to target creature."),
            format!("{subject} deal damage equal to the power of that creature to target creature.")] {
            admitted("Sorcery", &text).unwrap_or_else(|e| panic!("{text}: {e}"));
        }
    }
}
