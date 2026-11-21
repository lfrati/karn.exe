//! Invalid complete actions roll back proposed mana activations and their triggers.
use crate::support::{cite,paid};
use game::{CardDef, Game, Move, Zone};
use ir::mana::ManaSymbol;
use ir::Color;

fn card(name: &str, cost: &str, kind: &str, text: &str, pt: Option<(i32, i32)>) -> CardDef {
    let d = CardDef::new(name, cost, kind, text, pt).unwrap();
    game::play::check(&d).unwrap();
    d
}

#[test]
fn r601_2g_an_unpayable_supplied_cast_rolls_back_sacrifices_and_their_triggers() {
    let mut g = Game::new(vec![
        card("Altar", "{0}", "Artifact", "Sacrifice a creature: Add {G}{G}.", None),
        card("Bearer", "{0}", "Creature", "When this creature dies, you gain 2 life.", Some((2, 2))),
        card("Affordable", "{G}{G}", "Sorcery", "You gain 3 life.", None),
        card("Expensive", "{G}{G}{G}", "Sorcery", "You gain 4 life.", None),
    ], 2, 20);
    let altar = g.put(0, 0, Zone::Battlefield);
    let bearer = g.put(0, 1, Zone::Battlefield);
    let affordable = g.put(0, 2, Zone::Hand);
    let expensive = g.put(0, 3, Zone::Hand);
    let step = game::legal::Step { plan: vec![], x: None, life: vec![], with: None, source: altar, ability: 0, paying: vec![vec![bearer]], adding: vec![], chose: vec![], cost_branches: None };
    let cast = |card| Move::WithMana(vec![step.clone()], Box::new(paid("GG",Move::Cast { card, targets: vec![] })));
    let before = format!("{g:?}");
    for _ in 0..2 {
        cite!(["118.3a", "733.1"]; refused("the supplied mana does not pay that cost, all of it and no more"): g.act(0, cast(expensive)));
        assert_eq!(format!("{g:?}"), before, "the refused action rolls back its sacrifice and trigger");
    }
    assert!(g.battlefield.contains(&bearer));
    cite!(["601.2g"]; ok: g.act(0, cast(affordable)));
    assert!(!g.battlefield.contains(&bearer));
    cite!(["603.3"]; assert_eq!(g.stack.len(), 2));
    assert_eq!(g.players[0].life, 20, "the death trigger waits above the completed spell");
    for _ in 0..2 {g.act(g.priority,Move::Pass).unwrap();}
    cite!(["603.3"]; assert_eq!(g.players[0].life,22));
    assert_eq!(g.stack.len(),1);
    for _ in 0..2 {g.act(g.priority,Move::Pass).unwrap();}
    assert_eq!(g.players[0].life,25);
    assert!(g.stack.is_empty());
}

#[test]
fn r733_1_a_rejected_live_mana_answer_still_rolls_back_its_costs() {
    let mut g = Game::new(vec![card("Source", "{0}", "Artifact",
        "{T}: You gain 2 life. Add one mana of any color.", None)], 2, 20);
    let source = g.put(0, 0, Zone::Battlefield);
    let before = format!("{g:?}");
    let activate = Move::Activate { source, ability: 0, targets: vec![], paying: vec![] };
    cite!(["608.2d"]; refused("that ability doesn't offer the mana said for it: say one of what it offers (608.2d)"): g.act(0, Move::Adding(vec![vec![ManaSymbol::Color(Color::Green), ManaSymbol::Color(Color::Green)]], Box::new(activate.clone()))));
    assert_eq!(format!("{g:?}"), before);
    g.act(0, Move::Adding(vec![vec![ManaSymbol::Color(Color::Green)]], Box::new(activate))).unwrap();
    assert!(g.obj(source).tapped);
    assert_eq!(g.players[0].life, 22);
    cite!(["605.3b"]; assert_eq!(g.players[0].pool.total(), 1));
}
