//! Evoke with several printed instances (113.2c, 702.74a).
use crate::support::cite;
use game::announcement::CastCost;
use game::state::Decision;
use game::{CardDef, Game, Move, Zone};
use ir::Part;

/// Cast the card in hand (`row`: that alternative cost, None: its mana cost) with five
/// colorless available; the most objects the stack held at once, and the game after.
fn cast(def: &CardDef, row: Option<usize>, spending: &str) -> (usize, Game) {
    let mut g = Game::new(vec![def.clone()], 2, 20);
    for p in 0..2 { for _ in 0..5 { g.put(p, 0, Zone::Library); } }
    while g.part != Part::PrecombatMain { g.act(g.priority, Move::Pass).unwrap(); }
    let card = g.put(0, 0, Zone::Hand);
    g.players[0].pool.colorless = 5;
    let spell = Move::Cast { card, targets: vec![] };
    g.act(0, crate::support::paid(spending,match row { Some(row) => Move::CastFor(CastCost::Alternative(row), Box::new(spell)), None => spell })).unwrap();
    let mut triggers = 0;
    for _ in 0..40 {
        match g.decision.clone() {
            Some(Decision::Order { player, abilities }) => { g.act(player, Move::Order((0..abilities.len()).collect())).unwrap(); }
            Some(d) => panic!("{d:?}"),
            None if g.stack.is_empty() => return (triggers, g),
            None => { triggers = triggers.max(g.stack.len()); g.act(g.priority, Move::Pass).unwrap(); }
        }
    }
    panic!("did not settle");
}

#[test]
fn r113_2c_only_the_paid_instance_triggers_and_a_hard_cast_stays() {
    // Rules: 113.2c, 702.74a, 603.4
    let def = CardDef::new("Twin recheck", "{5}", "Creature", "Evoke {0}\nEvoke {1}", Some((2, 2))).unwrap();
    game::play::check(&def).unwrap();
    for row in [0, 1] {
        let (stack, g) = cast(&def, Some(row), if row == 0 { "" } else { "C" });
        cite!(["603.4"]; assert_eq!(stack, 1, "row {row}: one sacrifice trigger, the paid instance's (603.4)"));
        cite!(["702.74a"]; case: format!("row {row}"); assert!(g.battlefield.is_empty(), "row {row}"));
    }
    let (stack, g) = cast(&def, None, "CCCCC");
    cite!(["603.4"]; assert_eq!(stack, 1, "only the creature spell itself was on the stack"));
    cite!(["702.74a"]; assert_eq!(g.battlefield.len(), 1, "a creature cast for its mana cost stays"));
}

#[test]
fn r702_74a_multiple_evoke_cost_reference_requires_unique_instance() {
    // The rule's own sacrifice trigger refers to its instance (113.2c).
    // Other unqualified cost-reading text lacks a supported unique row binding.
    let def = CardDef::new("Twin reader", "{5}", "Creature",
        "Evoke {0}\nEvoke {1}\nWhen this creature enters, if its evoke cost was paid, you gain 3 life.", Some((2, 2))).unwrap();
    crate::support::refused(game::play::check(&def).map_err(|e|e.0),"condition: CastWas");
}

#[test]
fn r702_74a_one_instance_still_reads_other_text_naming_its_evoke_cost() {
    let def = CardDef::new("Single reader", "{5}", "Creature",
        "Evoke {0}\nWhen this creature enters, if its evoke cost was paid, you gain 3 life.", Some((2, 2))).unwrap();
    game::play::check(&def).unwrap();
    let (_, g) = cast(&def, Some(0), "");
    cite!(["603.4"]; assert_eq!(g.players[0].life, 23));
    cite!(["702.74a"]; assert!(g.battlefield.is_empty()));
    let (_, g) = cast(&def, None, "CCCCC");
    cite!(["603.4"]; assert_eq!(g.players[0].life, 20));
    cite!(["702.74a"]; assert_eq!(g.battlefield.len(), 1));
}
