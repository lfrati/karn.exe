//! Singular references after a plural target (608.2c): unbounded and X plural target
//! counts, plus preserved contrasts. Run with `r608::r608_2c_plural_reference_runtime_review::`.
use game::CardDef;

fn admitted(text: &str, kind: &str) -> Result<(), String> {
    let pt = kind.starts_with("Creature").then_some((2, 9));
    game::play::check(&CardDef::new("Probe", "{X}", kind, text, pt).unwrap()).map_err(|e| e.0)
}

/// A static maximum above one includes "any number of" and "X" target counts. Both forms
/// are refused instead of reading the spell's power.
#[test]
fn unbounded_and_x_plural_targets_refuse_the_source_fallback() {
    for text in [
        "This spell deals 1 damage to each of any number of target creatures. You gain life equal to its power.",
        "This spell deals 1 damage to each of X target creatures. You gain life equal to its power.",
    ] {
        assert_eq!(admitted(text, "Instant"), Err("reference: singular reference to a plural target".into()), "{text}");
    }
}

/// These stay admitted:
/// - an explicit source name ("this spell's", "This creature");
/// - a re-named or untapped source before "It";
/// - a singular Local (the land) next to a plural group;
/// - singular targets;
/// - "each other" after a plural damage clause.
#[test]
fn explicit_source_local_and_other_contrasts_stay_admitted() {
    let c = "Creature — Bear";
    for (text, kind) in [
        ("Tap up to two target creatures. You gain life equal to this spell's mana value.", "Instant"),
        ("{T}: Tap up to two target creatures. This creature gets +1/+0 until end of turn. It gains haste until end of turn.", c),
        ("{T}: Tap up to two target creatures. Untap this creature. It gains haste until end of turn.", c),
        ("{T}: Tap up to two target creatures and up to one target land. It gets +1/+0 until end of turn.", c),
        ("{T}: Tap target creature. It gets -1/-0 until end of turn.", c),
        ("{T}: This creature deals 1 damage to each of up to two target creatures. Each other creature gets +1/+0 until end of turn.", c),
    ] {
        assert_eq!(admitted(text, kind), Ok(()), "{text}");
    }
}
