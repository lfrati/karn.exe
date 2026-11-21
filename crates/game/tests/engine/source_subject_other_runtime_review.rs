//! Root "other" after a completed
//! clause. Run with `engine::source_subject_other_runtime_review::`.
//!
//! Each case has a 2/9 source, the opponent's 0/9 Wall as the target, and the controller's
//! 1/9 Bear; the controller has a Forest. Powers are read after resolution.
use game::state::Decision;
use game::{CardDef, Game, Move, Target, Zone};

fn admitted(text: &str) -> Result<(), String> { admitted_kind(text, "Creature — Bear") }
fn admitted_kind(text: &str, kind: &str) -> Result<(), String> {
    let pt = kind.starts_with("Creature").then_some((2, 9));
    game::play::check(&CardDef::new("Probe", "{0}", kind, text, pt).unwrap()).map_err(|e| e.0)
}

/// (source, Wall, Bear) powers once the ability has resolved.
fn powers(text: &str, kind: &str) -> (Option<i32>, Option<i32>, Option<i32>) {
    let pt = kind.starts_with("Creature").then_some((2, 9));
    let mut g = Game::new(vec![CardDef::new("Probe", "{0}", kind, text, pt).unwrap(),
        CardDef::new("Wall", "{0}", "Creature — Wall", "", Some((0, 9))).unwrap(),
        CardDef::new("Bear", "{0}", "Creature — Bear", "", Some((1, 9))).unwrap(),
        CardDef::new("Forest", "", "Basic Land — Forest", "", None).unwrap()], 2, 20);
    g.players[0].pool.colors = [10; 5];
    let (w, b) = (g.put(1, 1, Zone::Battlefield), g.put(0, 2, Zone::Battlefield));
    g.put(0, 3, Zone::Battlefield);
    let mut source = None;
    if text.starts_with("When") {
        let c = g.put(0, 0, Zone::Hand);
        g.act(0, Move::Cast { card: c, targets: vec![] }).unwrap();
    } else if kind == "Instant" {
        let c = g.put(0, 0, Zone::Hand);
        g.act(0, Move::Cast { card: c, targets: vec![vec![Target::Object(w)]] }).unwrap();
    } else {
        let p = g.put(0, 0, Zone::Battlefield);
        g.act(0, Move::Activate { source: p, ability: 0, targets: vec![vec![Target::Object(w)]], paying: vec![] }).unwrap();
        source = Some(p);
    }
    for _ in 0..20 {
        if let Some(Decision::Targets { .. }) = g.decision { g.act(0, Move::ChooseTargets(vec![vec![Target::Object(w)]])).unwrap(); continue; }
        if g.stack.is_empty() { break; }
        g.act(g.priority, Move::Pass).unwrap();
    }
    let source = source.or_else(|| g.battlefield.iter().copied().find(|o| g.obj(*o).chars.name == "Probe"));
    (source.and_then(|s| g.power(s)), g.power(w), g.power(b))
}

/// A completed Source or own-object subject is the one "each other
/// creature" excludes. An intervening target subject or controller clause replaces it, so
/// the target slot is excluded again. The spell nonmember keeps target semantics.
#[test]
fn completed_clause_subject_decides_the_excluded_object() {
    let c = "Creature — Bear";
    for (text, kind, expected) in [
        ("When this creature enters, it deals 1 damage to target creature. Each other creature gets +1/+0 until end of turn.", c, (2, 1, 2)),
        ("When this creature enters, it deals 1 damage to target creature and gains haste until end of turn. Each other creature gets +1/+0 until end of turn.", c, (2, 1, 2)),
        ("When this creature enters, it deals 1 damage to target creature. That creature gets -1/-0 until end of turn. Each other creature gets +1/+0 until end of turn.", c, (3, -1, 2)),
        ("When this creature enters, it deals 1 damage to target creature. You gain 1 life. Each other creature gets +1/+0 until end of turn.", c, (3, 0, 2)),
        ("When this creature enters, this creature deals 1 damage to target creature. If you control a Forest, this creature gains haste until end of turn. Otherwise, this creature gets +0/+1 until end of turn. Each other creature gets +1/+0 until end of turn.", c, (2, 1, 2)),
        ("{T}: This creature deals 1 damage to target creature. Each other creature you control gets +1/+0 until end of turn.", c, (2, 0, 2)),
        ("{T}: This creature deals 1 damage to target creature. Put a +1/+1 counter on each other creature.", c, (2, 1, 2)),
        ("{T}: Tap target creature. This creature gets +0/+1 until end of turn. Each other creature gets +1/+0 until end of turn.", c, (2, 1, 2)),
        ("{T}: This creature gets +0/+1 until end of turn. Tap target creature. Each other creature gets +1/+0 until end of turn.", c, (3, 0, 2)),
        ("This spell deals 1 damage to target creature. Each other creature gets +1/+0 until end of turn.", "Instant", (0, 0, 2)),
    ] {
        let (s, w, b) = powers(text, kind);
        let got = (s.unwrap_or(0), w.unwrap(), b.unwrap());
        // Uncited (implementation): no printed card has these shapes and English admits either antecedent; the engine's binding (the completed clause's subject, else the announced target) decides. Printed text reads "other" as other than the target (Public Execution, Nanogene Conversion, Echoing Equation) or the clause subject (Showstopping Surprise)
        assert_eq!(got, expected, "{text}");
    }
}

/// Both conservative refusals. A conditional arm whose subject differs from
/// the completed clause before it leaves the join unresolved. A complex root isn't
/// classified by type.
#[test]
fn divergent_joins_and_complex_roots_refuse() {
    for (text, reason) in [
        ("When this creature enters, it deals 1 damage to target creature. If you control a Forest, that creature gets -1/-0 until end of turn. Each other creature gets +1/+0 until end of turn.", "other: unresolved completed clause subject"),
        ("When this creature enters, tap target creature. If you control a Forest, this creature gains haste until end of turn. Each other creature gets +1/+0 until end of turn.", "other: unresolved completed clause subject"),
        ("{T}: This creature deals 1 damage to target creature. Each other nonartifact creature gets +1/+0 until end of turn.", "other: completed source compatibility not implemented"),
    ] {
        assert_eq!(admitted(text), Err(reason.into()), "{text}");
    }
}

/// A known limit, conservative and synthetic: this refusal is reached by no pinned
/// card.
/// - In an exhaustive if/otherwise exactly one arm runs. Both arms here are Source, so the
///   last completed clause is Source either way, and "other" could exclude the source.
/// - Instead the If arm is first joined with the state before the conditional, the target of
///   "tap target creature". That gives Unresolved before Otherwise is reached.
/// - When the clause before the "if" is also Source, the same text is admitted (first test).
/// - To admit this text (with the target excluded), the Otherwise arm should be joined
///   with the If arm alone.
#[test]
fn exhaustive_otherwise_with_source_arms_refuses_conservatively() {
    let text = "When this creature enters, tap target creature. If you control a Forest, this creature gains haste until end of turn. Otherwise, this creature gets +0/+1 until end of turn. Each other creature gets +1/+0 until end of turn.";
    assert_eq!(admitted(text), Err("other: unresolved completed clause subject".into()));
}
