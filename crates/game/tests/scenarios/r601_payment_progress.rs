//! Supplied mana actions can change the conditions of later mana abilities.
use crate::support::{cite, mana_pool, paid};
use game::{CardDef, Game, Move, Zone};
use game::legal::Step;

#[test]
fn r601_2g_repeated_mana_side_effects_can_enable_another_source() {
    let defs: Vec<_> = [
        ("Drain", "{0}", "Artifact", "{1}: Add {C}. Each opponent loses 1 life."),
        ("Enabled", "{0}", "Artifact", "{T}: Add {C}{C}{C}. Activate only if an opponent has 17 or less life."),
        ("Spell", "{3}", "Sorcery", "You gain 1 life."),
    ].into_iter().map(|(name, cost, kind, text)| {
        let def = CardDef::new(name, cost, kind, text, None).unwrap();
        game::play::check(&def).unwrap();
        def
    }).collect();
    let mut g = Game::new(defs, 2, 20);
    let drain = g.put(0, 0, Zone::Battlefield);
    let enabled = g.put(0, 1, Zone::Battlefield);
    let spell = g.put(0, 2, Zone::Hand);
    g.players[0].pool.colorless = 1;
    let mut explicit = g.clone();
    let plan = [drain, drain, drain, enabled].into_iter().map(|source| Step { plan: vec![], x: None, life: vec![], with: if source == drain { Some(mana_pool("C")) } else { None }, cost_branches: None, chose: vec![],
        source, ability: 0, paying: vec![], adding: vec![],
    }).collect();
    explicit.act(0, paid("CCC", Move::WithMana(plan, Box::new(Move::Cast { card: spell, targets: vec![] })))).unwrap();
    cite!(["605.3b"]; assert_eq!(explicit.players[1].life, 17));
    cite!(["601.2g", "601.2h"]; assert_eq!(explicit.stack.len(), 1));
}
