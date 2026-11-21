//! Mana execution must not identify sources by printed card alone once their abilities differ.
use crate::support::{cite,paid,mana_pool};
use game::{CardDef, Game, Move, Target, Zone};

#[test]
fn r601_2g_mana_execution_distinguishes_identical_cards_with_different_granted_abilities() {
    for reverse in [false, true] {
        let defs = vec![
            CardDef::new("Bear", "", "Creature", "", Some((2, 2))).unwrap(),
            CardDef::new("Blue grant", "{0}", "Sorcery", "Until end of turn, target creature gains \"{G}, {T}: Add {U}{U}.\"", None).unwrap(),
            CardDef::new("Red grant", "{0}", "Sorcery", "Until end of turn, target creature gains \"{G}, {T}: Add {R}{R}.\"", None).unwrap(),
            CardDef::new("Red spell", "{R}", "Sorcery", "You gain 1 life.", None).unwrap(),
        ];
        for def in &defs { game::play::check(def).unwrap(); }
        let mut g = Game::new(defs, 2, 20);
        let (first, second) = (g.put(0, 0, Zone::Battlefield), g.put(0, 0, Zone::Battlefield));
        let (blue, red) = if reverse { (second, first) } else { (first, second) };
        for (which, target) in [(1, blue), (2, red)] {
            let card = g.put(0, which, Zone::Hand);
            g.act(0, Move::Cast { card, targets: vec![vec![Target::Object(target)]] }).unwrap();
            for _ in 0..2 { g.act(g.priority, Move::Pass).unwrap(); }
        }
        g.players[0].pool.colors[4] = 1;
        let card = g.put(0, 3, Zone::Hand);
        let step = |source| game::legal::Step { plan: vec![], x: None, life: vec![], with: Some(mana_pool("G")), source, ability: 0, paying: vec![], adding: vec![], chose: vec![], cost_branches: None };
        let before = format!("{g:?}");
        cite!(["601.2g"]; case: format!("reverse={reverse}"); refused("that mana is not in your mana pool"): g.act(0, Move::WithMana(vec![step(blue)], Box::new(paid("R",Move::Cast { card, targets: vec![] })))));
        assert_eq!(format!("{g:?}"), before);
        let plan = vec![step(red)];
        g.act(0, Move::WithMana(plan, Box::new(paid("R",Move::Cast { card, targets: vec![] })))).unwrap();
        cite!(["601.2g"]; case: format!("reverse={reverse}"); assert!(g.obj(red).tapped));
        cite!(["601.2g"]; case: format!("reverse={reverse}"); assert!(!g.obj(blue).tapped));
        for _ in 0..2 { g.act(g.priority, Move::Pass).unwrap(); }
        assert_eq!(g.players[0].life, 21);
    }
}
