//! The ordinary validity of a
//! dealer-exempt shared subject is decided where it was first bound, not after the first
//! verb has named more. Run with `r608::r608_2c_dealer_cache_context_review::`.
use game::{CardDef, Game, Move, Target, Zone};

fn creature(text: &str) -> CardDef { CardDef::new("Probe", "{0}", "Creature — Bear", text, Some((4, 9))).unwrap() }
fn admitted(text: &str) -> Result<(), String> { game::play::check(&creature(text)).map_err(|e| e.0) }

/// The first verb names "target creature". Revalidating "it" afterwards would find
/// that same-sentence, object-only target and let "gets" reuse the cached source subject.
/// It is refused, as are the reversed order and the form without the intervening target.
#[test]
fn intervening_target_does_not_validate_a_dealer_exempt_subject() {
    for text in ["{T}: This creature deals 1 damage to any target. It deals 1 damage to target creature and gets +1/+0 until end of turn.",
                 "{T}: This creature deals 1 damage to any target. It gets +1/+0 until end of turn and deals 1 damage to target creature.",
                 "{T}: This creature deals 1 damage to any target. It deals 1 damage to you and gets +1/+0 until end of turn."] {
        assert_eq!(admitted(text), Err("reference: unrefined mixed target".into()), "{text}");
    }
}

/// Contrast: shared subjects that aren't dealer-exempt mixed fallbacks stay admitted.
/// These are the explicit source in either verb order, the trigger's own "it", and a
/// creature-only earlier target. The last one also runs. There "it" is the targeted creature,
/// by object-only precedence: the Wall deals me 1 and gets +1/+0.
#[test]
fn legitimate_shared_subjects_stay() {
    for text in ["{T}: This creature deals 1 damage to any target and gets +1/+0 until end of turn.",
                 "{T}: This creature gets +1/+0 until end of turn and deals 1 damage to any target.",
                 "When this creature enters, it deals 1 damage to any target and gains haste until end of turn.",
                 "{T}: This creature deals 1 damage to target creature. It deals 1 damage to you and gets +1/+0 until end of turn."] {
        assert_eq!(admitted(text), Ok(()), "{text}");
    }
    let text = "{T}: This creature deals 1 damage to target creature. It deals 1 damage to you and gets +1/+0 until end of turn.";
    let wall = CardDef::new("Wall", "{0}", "Creature — Wall", "", Some((0, 9))).unwrap();
    let mut g = Game::new(vec![creature(text), wall], 2, 20);
    let s = g.put(0, 0, Zone::Battlefield);
    let w = g.put(1, 1, Zone::Battlefield);
    g.act(0, Move::Activate { source: s, ability: 0, targets: vec![vec![Target::Object(w)]], paying: vec![] }).unwrap();
    for _ in 0..3 { if g.stack.is_empty() { break; } g.act(g.priority, Move::Pass).unwrap(); }
    assert_eq!((g.power(s), g.power(w), g.obj(w).damage, g.players[0].life), (Some(4), Some(1), 1, 19));
}
