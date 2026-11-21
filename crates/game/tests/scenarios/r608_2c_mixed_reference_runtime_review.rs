//! The damage-dealer exemption from the mixed-target refusal, as it runs (608.2c). Run with
//! `r608::r608_2c_mixed_reference_runtime_review::`.
use crate::support::cite;
use game::{CardDef, Event, Game, Move, ObjId, Target, Zone};

fn creature(text: &str) -> CardDef { CardDef::new("Probe", "{0}", "Creature — Bear", text, Some((4, 9))).unwrap() }
fn admitted(d: &CardDef) -> Result<(), String> { game::play::check(d).map_err(|e| e.0) }
fn dealt(g: &Game) -> Vec<(ObjId, Target, u32)> {
    g.log.iter().filter_map(|e| match e { Event::DamageDealt { source, to, amount, .. } => Some((*source, *to, *amount)), _ => None }).collect()
}

/// The dealer exemption must not reach a non-dealer verb through the cached
/// shared clause subject. Admission must not depend on verb order: the reversed order,
/// the non-dealer verb alone and the dealer-first order are all refused, so in none does
/// the source get +1/+0.
#[test]
fn dealer_exemption_does_not_reach_a_shared_non_dealer_verb() {
    for text in ["{T}: This creature deals 1 damage to any target. It gets +1/+0 until end of turn and deals 1 damage to you.",
                 "{T}: This creature deals 1 damage to any target. It gets +1/+0 until end of turn."] {
        assert_eq!(admitted(&creature(text)), Err("reference: unrefined mixed target".into()), "{text}");
    }
    let text = "{T}: This creature deals 1 damage to any target. It deals 1 damage to you and gets +1/+0 until end of turn.";
    assert_eq!(admitted(&creature(text)), Err("reference: unrefined mixed target".into()), "{text}");
}

/// Contrast: the direct dealer and the possessors its clause owns keep the source.
#[test]
fn direct_dealer_and_its_clause_possessors_keep_the_source() {
    for (text, expected) in [
        ("{T}: This creature deals 1 damage to any target. It deals 1 damage to you.", vec![(Target::Player(1), 1), (Target::Player(0), 1)]),
        ("{T}: This creature deals 1 damage to any target. Then it deals damage equal to its power to you.", vec![(Target::Player(1), 1), (Target::Player(0), 4)]),
        ("{T}: This creature deals 1 damage to any target. Then it deals 1 damage to its controller.", vec![(Target::Player(1), 1), (Target::Player(0), 1)]),
    ] {
        let mut g = Game::new(vec![creature(text)], 2, 20);
        let s = g.put(0, 0, Zone::Battlefield);
        g.act(0, Move::Activate { source: s, ability: 0, targets: vec![vec![Target::Player(1)]], paying: vec![] }).unwrap();
        for _ in 0..3 { if g.stack.is_empty() { break; } g.act(g.priority, Move::Pass).unwrap(); }
        cite!(["608.2c"]; case: text; assert_eq!(dealt(&g), expected.into_iter().map(|(to, n)| (s, to, n)).collect::<Vec<_>>(), "{text}"));
    }
}
