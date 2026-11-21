//! "Whenever a creature is destroyed", "whenever a creature is exiled": what was destroyed or
//! exiled has left the battlefield, and the trigger sees it as it last existed (603.10a).
use crate::support::cite;
use game::{CardDef, Game, Move, Target, Zone};

fn life_after(text: &str) -> i32 {
    let c = |name, types, text, pt| CardDef::new(name, "{0}", types, text, pt).unwrap();
    let defs = vec![
        c("Bear", "Creature — Bear", "", Some((2, 2))),
        c("Spell", "Sorcery", text, None),
        c("Watcher", "Enchantment", "Whenever a creature is exiled, you gain 1 life.\nWhenever a creature is destroyed, you gain 10 life.", None),
    ];
    for d in &defs { game::play::check(d).unwrap_or_else(|e| panic!("{}: {}", d.name(), e.0)); }
    let mut g = Game::new(defs, 2, 20);
    g.put(0, 2, Zone::Battlefield);
    let bear = g.put(1, 0, Zone::Battlefield);
    let spell = g.put(0, 1, Zone::Hand);
    g.act(0, Move::Cast { card: spell, targets: vec![vec![Target::Object(bear)]] }).unwrap();
    while !g.stack.is_empty() { g.act(g.priority, Move::Pass).unwrap(); }
    assert!(!g.battlefield.contains(&bear));
    g.players[0].life
}

#[test]
fn r603_10a_an_exiled_creature_is_seen_as_it_last_existed() {
    cite!(["603.10a"]; assert_eq!(life_after("Exile target creature."), 21));
}

#[test]
fn r603_10a_a_destroyed_creature_is_seen_as_it_last_existed() {
    cite!(["603.10a"]; assert_eq!(life_after("Destroy target creature."), 30));
}
