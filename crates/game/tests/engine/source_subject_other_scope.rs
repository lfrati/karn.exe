//! Bounded compatibility and clause lifetime of the root "other" binding. The
//! straight-line expectations are in source_subject_other_review.
use game::{CardDef, Event, Game, Move, Target, Zone};
use game::state::Decision;
use crate::support::cite;

fn check(types: &str, text: &str) -> Result<(), String> {
    let d = if types.contains("Planeswalker") { CardDef::planeswalker("Probe", "{0}", types, text, 3).unwrap() }
        else { CardDef::new("Probe", "{0}", types, text, types.contains("Creature").then_some((1, 5))).unwrap() };
    game::play::check(&d).map_err(|e| e.0)
}

#[test]
fn unknown_granted_source_refuses_only_the_new_competing_proof() {
    let competing = "Creatures you control have \"{T}: This creature deals 1 damage to any target. Each other creature you control deals 1 damage to it.\"";
    assert_eq!(check("Creature — Bear", competing), Err("other: completed source compatibility not implemented".into()));
    assert_eq!(check("Creature — Bear", "Creatures you control have \"{T}: This creature deals 1 damage to any target.\""), Ok(()));
    assert_eq!(check("Creature — Bear", "Creatures you control have \"{T}: Each other creature you control gets +1/+1 until end of turn.\""), Ok(()));
}

#[test]
fn complex_root_does_not_claim_source_membership() {
    assert_eq!(check("Creature — Bear", "{T}: This creature deals 1 damage to any target. Each other red creature you control gets +1/+1 until end of turn."), Err("other: completed source compatibility not implemented".into()));
}

#[test]
fn conditional_outgoing_context_is_preserved_or_explicitly_unresolved() {
    assert_eq!(check("Creature — Bear", "When this creature enters, this creature deals 1 damage to any target. If you control a Plains, this creature gets +1/+1 until end of turn. Each other creature you control gets +1/+1 until end of turn."), Ok(()));
    for text in [
        "When this creature enters, this creature deals 1 damage to any target. If you control a Plains, you gain 1 life. Each other creature you control gets +1/+1 until end of turn.",
        "When this creature enters, if you control a Plains, this creature deals 1 damage to any target. Each other creature you control gets +1/+1 until end of turn.",
    ] {
        assert_eq!(check("Creature — Bear", text), Err("other: unresolved completed clause subject".into()), "{text}");
    }
}

fn resolve(text: &str) -> (Game, game::ObjId, game::ObjId) {
    let probe = CardDef::new("Probe", "{0}", "Creature — Bear", text, Some((1, 5))).unwrap();
    game::play::check(&probe).unwrap();
    let bear = CardDef::new("Bear", "{0}", "Creature — Bear", "", Some((3, 9))).unwrap();
    let mut g = Game::new(vec![probe, bear], 2, 20);
    let mine = g.put(0, 1, Zone::Battlefield);
    let c = g.put(0, 0, Zone::Hand);
    g.act(0, Move::Cast { card: c, targets: vec![] }).unwrap();
    for _ in 0..8 {
        if matches!(g.decision, Some(Decision::Targets { .. })) { g.act(0, Move::ChooseTargets(vec![vec![Target::Player(1)]])).unwrap(); }
        else if g.stack.is_empty() { break; }
        else { g.act(g.priority, Move::Pass).unwrap(); }
    }
    let source = g.battlefield.iter().copied().find(|o| *o != mine).unwrap();
    (g, source, mine)
}

#[test]
fn completed_controller_clause_replaces_source_context() {
    let (g, source, mine) = resolve("When this creature enters, this creature deals 1 damage to any target. You gain 1 life. Each other creature you control deals 1 damage to it.");
    let dealt: Vec<_> = g.log.iter().filter_map(|e| match e { Event::DamageDealt { source, to, amount, .. } => Some((*source, *to, *amount)), _ => None }).collect();
    // Uncited (implementation): no printed card has this shape; "each other creature you control deals 1 damage to it" reads coherently with the target as antecedent, which the engine's binding (completed clause subject "you", else the announced target) picks, as printed Public Execution, Nanogene Conversion and Echoing Equation read "other"
    assert_eq!(dealt, vec![(source, Target::Player(1), 1), (mine, Target::Player(1), 1), (source, Target::Player(1), 1)]);
    cite!(["120.3a"]; assert_eq!((g.players[0].life, g.players[1].life), (21, 17)));
}

#[test]
fn joined_verbs_complete_as_one_source_clause() {
    let (g, source, mine) = resolve("When this creature enters, this creature gets +1/+0 until end of turn and deals 1 damage to any target. Each other creature you control deals 1 damage to it.");
    let dealt: Vec<_> = g.log.iter().filter_map(|e| match e { Event::DamageDealt { source, to, amount, .. } => Some((*source, *to, *amount)), _ => None }).collect();
    // Uncited (card text): "this creature gets +1/+0 ... and deals 1 damage to any target. Each other creature you control deals 1 damage to it": other than this creature
    assert_eq!(dealt, vec![(source, Target::Player(1), 1), (mine, Target::Player(1), 1)]);
}

#[test]
fn sibling_does_not_inherit_taken_arm_and_join_does_not_drop_it() {
    let text = "When this creature enters, this creature deals 1 damage to any target. If you control a Plains, you gain 1 life. Otherwise, each other creature you control gets +1/+1 until end of turn.";
    // The else body inherits the dominating source, not the controller from the other arm.
    assert_eq!(check("Creature — Bear", text), Ok(()));
    let after = format!("{text} Each other creature you control gets +1/+1 until end of turn.");
    assert_eq!(check("Creature — Bear", &after), Err("other: unresolved completed clause subject".into()));
}

#[test]
fn lexical_spell_and_planeswalker_nonmembers_keep_target_binding_after_a_join() {
    for (types, source) in [("Instant", "This spell"), ("Planeswalker — Garruk", "{0}: This planeswalker")] {
        let text = format!("{source} deals 1 damage to target creature. If you control a Plains, you gain 1 life. Each other creature you control gets +1/+1 until end of turn.");
        assert_eq!(check(types, &text), Ok(()), "{text}");
    }
}
