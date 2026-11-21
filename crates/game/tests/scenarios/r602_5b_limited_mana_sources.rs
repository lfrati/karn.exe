//! "Activate only once each turn" is of the object (602.5b): two permanents of one card are
//! not interchangeable for paying once one of them has used its ability. Explicit activation
//! and payment quotes check each supplied source independently (601.2g-h, 117.1).
use crate::support::{cite,mana_pool};
use game::legal::Step;
use game::{CardDef, Game, Move, ObjId, Zone};

fn card(name: &str, cost: &str, types: &str, text: &str) -> CardDef {
    let d = CardDef::new(name, cost, types, text, None).unwrap_or_else(|e| panic!("{name}: {e}"));
    game::play::check(&d).unwrap_or_else(|u| panic!("{name}: {}", u.0));
    d
}

const LIMITED: &str = "{1}: Add {C}{C}. Activate only once each turn.";

/// Two limited sources (of one card, or of two), one mana floating, a three-mana spell.
fn board(same_card: bool) -> (Game, ObjId, [ObjId; 2]) {
    let defs = vec![
        card("Source", "{0}", "Artifact", LIMITED),
        card("Other Source", "{0}", "Artifact", LIMITED),
        card("Spell", "{3}", "Sorcery", "You gain 1 life."),
    ];
    let mut g = Game::new(defs, 2, 20);
    let a = g.put(0, 0, Zone::Battlefield);
    let b = g.put(0, if same_card { 0 } else { 1 }, Zone::Battlefield);
    let spell = g.put(0, 2, Zone::Hand);
    g.players[0].pool.colorless = 1;
    (g, spell, [a, b])
}

#[test]
fn r602_5b_a_used_limited_source_does_not_stand_in_for_an_unused_one() {
    for same_card in [true, false] {
        let (g, spell, sources) = board(same_card);
        // the explicit payment: one mana into the first, one of its two into the second
        let mut paid = g.clone();
        let plan = sources.into_iter().map(|source| Step { plan: vec![], x: None, life: vec![], with: Some(mana_pool("C")), cost_branches: None, chose: vec![],  source, ability: 0, paying: vec![], adding: vec![] }).collect();
        paid.act(0, Move::WithMana(plan, Box::new(crate::support::paid("CCC",Move::Cast { card: spell, targets: vec![] })))).expect("payable");
        cite!(["601.2g", "601.2h"]; case: format!("same_card={same_card}"); assert_eq!(paid.stack.len(), 1));
        assert_eq!(paid.players[0].pool.total(), 0);
    }
}

#[test]
fn r602_5b_one_limited_source_alone_cannot_pay() {
    // One activation leaves2, short of3; a second activation violates its per-object limit.
    let defs = vec![card("Source", "{0}", "Artifact", LIMITED), card("Spell", "{3}", "Sorcery", "You gain 1 life.")];
    let mut g = Game::new(defs, 2, 20);
    let source = g.put(0, 0, Zone::Battlefield);
    let spell = g.put(0, 1, Zone::Hand);
    g.players[0].pool.colorless = 1;
    let step = Step { plan: vec![], x: None, life: vec![], with: Some(mana_pool("C")), source, ability: 0, paying: vec![], adding: vec![], chose: vec![], cost_branches: None };
    for (plan, reason) in [(vec![step.clone()], "that mana is not in your mana pool"), (vec![step.clone(), step], "that ability is activated only once each turn")] {
        let before = format!("{g:?}");
        cite!(["602.5b", "601.2h"]; refused(reason): g.act(0, Move::WithMana(plan, Box::new(crate::support::paid("CCC", Move::Cast { card: spell, targets: vec![] })))));
        assert_eq!(format!("{g:?}"), before);
    }
}
