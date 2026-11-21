//! A bare non-recipient "it"/"its"
//! whose only winning antecedent is a mixed (object-or-player) target must not fall back to
//! the named source (608.2c): such texts are refused. The source,
//! own-object, clause-possessor and object-only contrasts stay supported. Run with
//! `r608::r608_2c_mixed_reference_refusal_review::`.
use crate::support::cite;
use game::{CardDef, Event, Game, Move, ObjId, Target, Zone};

fn admitted(d: &CardDef) -> Result<(), String> { game::play::check(d).map_err(|e| e.0) }
fn sorcery(text: &str) -> CardDef { CardDef::new("Probe", "{0}", "Sorcery", text, None).unwrap() }
fn bear(name: &str, p: i32) -> CardDef { CardDef::new(name, "{0}", "Creature — Bear", "", Some((p, 9))).unwrap() }

/// Mine: Big 3/9; the opponent's: Wall 2/9. Targets by letter: O the Wall, M my Big, P the
/// opponent. Casts `d` from my hand and resolves it. Returns the game, (Big, Wall, spell).
fn run(d: CardDef, targets: &str) -> (Game, [ObjId; 3]) {
    let mut g = Game::new(vec![d, bear("Big", 3), bear("Wall", 2)], 2, 20);
    let (a, w) = (g.put(0, 1, Zone::Battlefield), g.put(1, 2, Zone::Battlefield));
    let c = g.put(0, 0, Zone::Hand);
    let targets = targets.chars().map(|k| vec![match k { 'O' => Target::Object(w), 'M' => Target::Object(a), _ => Target::Player(1) }]).collect();
    g.act(0, Move::Cast { card: c, targets }).unwrap();
    let s = *g.stack.last().unwrap();
    for _ in 0..3 { if g.stack.is_empty() { break; } g.act(g.priority, Move::Pass).unwrap(); }
    (g, [a, w, s])
}

/// Two uses that would silently read the spell: choosing the Wall,
/// "can't block" would apply to the spell and the gain would be 0.
#[test]
fn draft_uses_refuse_instead_of_reading_the_spell() {
    for text in ["This spell deals 1 damage to any target. It can't block this turn.",
                 "This spell deals 1 damage to any target. You gain life equal to its power."] {
        crate::support::refused(admitted(&sorcery(text)), "reference: unrefined mixed target");
    }
}

/// The same fallback through the same two `Binder::referent` call sites
/// (`Scoped::amount_subject`, `Emit::operand`), in other uses. Each would silently
/// read the spell after the Wall is chosen: the condition false (0 life), the counter
/// going nowhere, and I (the spell's controller) gaining the 2.
#[test]
fn same_fallback_in_predicates_action_objects_and_controller_refuses() {
    for text in ["This spell deals 1 damage to any target. If it's a creature, you gain 3 life.",
                 "This spell deals 1 damage to any target. Put a +1/+1 counter on it.",
                 "This spell deals 1 damage to any target. Its controller gains 2 life.",
                 "This spell deals 1 damage to any target. Its owner gains 2 life."] {
        crate::support::refused(admitted(&sorcery(text)), "reference: unrefined mixed target");
    }
}

/// An object-only candidate exists, but precedence would still pick the
/// spell (named after it), and the mixed target named with the spell would beat the spell.
/// Choosing my Big and then the Wall, the gain would be 0, the spell's power.
#[test]
fn source_chosen_over_a_later_mixed_target_refuses() {
    let text = "Tap target creature. This spell deals 1 damage to any target. You gain life equal to its power.";
    crate::support::refused(admitted(&sorcery(text)), "reference: unrefined mixed target");
}

/// Contrast: references that legitimately name the source or another antecedent stay supported.
#[test]
fn legitimate_antecedents_stay() {
    // An object-only target is the antecedent: I gain the Wall's power, 2.
    let (g, _) = run(sorcery("This spell deals 1 damage to target creature. You gain life equal to its power."), "O");
    cite!(["608.2c"]; assert_eq!(g.players[0].life, 22));
    // A later object-only target beats a mixed one named with the spell: Big's power, 3.
    let (g, _) = run(sorcery("This spell deals 1 damage to any target. Tap target creature. You gain life equal to its power."), "OM");
    cite!(["608.2c"]; assert_eq!(g.players[0].life, 23));
    // The clause's own subject owns "its": Big gets +3/+0.
    let (g, [a, ..]) = run(sorcery("This spell deals 1 damage to any target. Target creature you control gets +X/+0 until end of turn, where X is its power."), "OM");
    cite!(["608.2c"]; assert_eq!(g.power(a), Some(6)));
    // A replacement dealer "it" (Burst Lightning, Roil Eruption, Court of Ire, Frost Bite)
    // doesn't go through the fallback: the spell deals 4.
    let (g, [_, w, s]) = run(sorcery("This spell deals 2 damage to any target. If you control a creature, it deals 4 damage instead."), "O");
    let dealt: Vec<_> = g.log.iter().filter_map(|e| match e { Event::DamageDealt { source, to, amount, .. } => Some((*source, *to, *amount)), _ => None }).collect();
    cite!(["608.2c"]; assert_eq!(dealt, vec![(s, Target::Object(w), 4)]));
    // A source named again in the reference's own sentence (Beacon of Destruction).
    assert_eq!(admitted(&sorcery("This spell deals 5 damage to any target. Shuffle this spell into its owner's library.")), Ok(()));
    // A trigger's own object is an antecedent before the mixed target ("When this creature
    // enters, it deals …"): it stays admitted.
    let etb = CardDef::new("Probe", "{0}", "Creature — Bear", "When this creature enters, it deals 1 damage to any target. You gain life equal to its power.", Some((4, 4))).unwrap();
    assert_eq!(admitted(&etb), Ok(()));
    // Pinned contrasts: no admitted pinned card reaches the fallback.
    let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let catalog = CardDef::catalog(&faces);
    for name in ["Burst Lightning", "Roil Eruption", "Frost Bite", "Beacon of Destruction", "Make Mischief", "Pain for All"] {
        assert_eq!(admitted(&CardDef::from_faces(catalog.get(name).unwrap()).unwrap()), Ok(()), "{name}");
    }
}
