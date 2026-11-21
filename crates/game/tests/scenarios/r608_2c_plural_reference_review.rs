//! A singular "it"/"its" after a statically plural, object-capable target does not fall
//! back to the named source: the text is refused (608.2c).
//! Run with `r608::r608_2c_plural_reference_review::`.
//!
//! - The refusals, each with its reason.
//! - The boundaries the refusal leaves alone.
//! - Corpus evidence: after a plural target, pinned Oracle text names the source again
//!   (31 abilities). It never calls the source "it"; its 5 bare "it"s after a plural target
//!   are a token, a chosen card or a group member.
use crate::support::cite;
use game::{CardDef, Event, Game, Move, Target, Zone};

const PLURAL: &str = "reference: singular reference to a plural target";
fn def(text: &str) -> CardDef {
    if text.starts_with("This spell") { CardDef::new("Probe", "{0}", "Instant", text, None).unwrap() }
    else { CardDef::new("Probe", "{0}", "Creature — Bear", text, Some((4, 9))).unwrap() }
}
fn admitted(text: &str) -> Result<(), String> { game::play::check(&def(text)).map_err(|e| e.0) }

/// Each of these would otherwise be admitted, reading the source. They are
/// object-only and exact-two forms. The first-binding cache stores the plural refusal, in
/// both verb orders. Mixed singular plus plural proof prefers the plural reason.
#[test]
fn singular_it_after_a_plural_target_refuses_the_source_fallback() {
    for text in [
        "This spell deals 1 damage to each of up to two targets. It can't block this turn.",
        "This spell deals 1 damage to each of up to two targets. You gain life equal to its power.",
        "{T}: This creature deals 1 damage to each of up to two target creatures. You gain life equal to its power.",
        "{T}: This creature deals 1 damage to each of two target creatures. It gets +1/+0 until end of turn.",
        "{T}: This creature deals 1 damage to each of up to two target creatures and 1 damage to you. It gets +1/+0 until end of turn.",
        "{T}: Tap target land. This creature deals 1 damage to each of up to two target creatures. It gets +1/+0 until end of turn.",
        "{T}: This creature deals 1 damage to each of up to two targets. It deals 1 damage to you and gets +1/+0 until end of turn.",
        "{T}: This creature deals 1 damage to each of up to two targets. It gets +1/+0 until end of turn and deals 1 damage to you.",
        "This spell deals 2 damage to any target and 1 damage to each of up to two other targets. You gain life equal to its power.",
    ] {
        assert_eq!(admitted(text), Err(PLURAL.into()), "{text}");
    }
}

/// Contrast: nothing here is a source fallback that a plural
/// target outranks. The cases are:
/// - the source named again after the plural target;
/// - the direct dealer;
/// - a later token or a singular Local;
/// - an up-to-one target;
/// - a player-only plural group;
/// - two pinned cards.
#[test]
fn source_local_dealer_and_player_contrasts_stay_admitted() {
    for text in [
        "{T}: This creature deals 1 damage to each of up to two targets. It deals 1 damage to you.",
        "{T}: This creature deals 1 damage to each of up to two targets. This creature gets +1/+0 until end of turn.",
        "{T}: This creature deals 1 damage to each of up to two targets. Then this creature deals damage equal to its power to you.",
        "{T}: Tap up to two target creatures. Untap this creature. It gets +1/+0 until end of turn.",
        "{T}: This creature deals 1 damage to each of up to two target creatures. Create a 1/1 white Soldier creature token. It gains haste until end of turn.",
        "{T}: Tap up to two target creatures. Put a +1/+1 counter on target creature you control. It gains haste until end of turn.",
        "{T}: This creature deals 1 damage to up to one target creature. It can't block this turn.",
        "{T}: Up to two target players each draw a card. Put a +1/+1 counter on this creature. You gain life equal to its power.",
    ] {
        assert_eq!(admitted(text), Ok(()), "{text}");
    }
    let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let catalog = CardDef::catalog(&faces);
    for name in ["Adverse Conditions", "Wildest Dreams"] {
        let d = CardDef::from_faces(catalog.get(name).unwrap()).unwrap();
        assert_eq!(game::play::check(&d).map_err(|e| e.0), Ok(()), "{name}");
    }
}

/// The source still acts in the preserved forms: the direct dealer hits
/// its controller; the re-named source gets +1/+0; a player-only plural group leaves "its
/// power" to the countered source (4 + 1).
#[test]
fn preserved_source_reads_execute() {
    let run = |text: &str, targets: Vec<Target>| {
        let mut g = Game::new(vec![def(text)], 2, 20);
        let s = g.put(0, 0, Zone::Battlefield);
        g.act(0, Move::Activate { source: s, ability: 0, targets: vec![targets], paying: vec![] }).unwrap();
        for _ in 0..3 { if g.stack.is_empty() { break; } g.act(g.priority, Move::Pass).unwrap(); }
        let dealt: Vec<_> = g.log.iter().filter_map(|e| match e { Event::DamageDealt { source, to, amount, .. } => Some((*source, *to, *amount)), _ => None }).collect();
        (g, s, dealt)
    };
    let (_, s, dealt) = run("{T}: This creature deals 1 damage to each of up to two targets. It deals 1 damage to you.", vec![Target::Player(1)]);
    cite!(["608.2c"]; assert_eq!(dealt, vec![(s, Target::Player(1), 1), (s, Target::Player(0), 1)]));
    let (g, s, _) = run("{T}: This creature deals 1 damage to each of up to two targets. This creature gets +1/+0 until end of turn.", vec![Target::Player(1)]);
    assert_eq!(g.power(s), Some(5));
    let (g, _, _) = run("{T}: Up to two target players each draw a card. Put a +1/+1 counter on this creature. You gain life equal to its power.", vec![Target::Player(1)]);
    cite!(["608.2c"]; assert_eq!(g.players[0].life, 25));
}

/// A known limit, outside the named-source refusal. The own trigger
/// object's bare "it" after a plural target keeps the input fallback. It reads the entering
/// creature, the same object a named-source form would read but refuses. The asymmetry
/// is deliberate, since input fallbacks are not gated: this records the engine's limit,
/// not an expectation from the rules.
#[test]
fn own_trigger_object_after_a_plural_target_keeps_its_input_fallback() {
    assert_eq!(admitted("When this creature enters, tap up to two target creatures. It gets +1/+0 until end of turn."), Ok(()));
}
