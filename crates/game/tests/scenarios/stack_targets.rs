//! Spells are objects on the stack (112.1); target discovery must agree with casting.
use crate::support::cite;
use game::{CardDef, Game, Move, Target, Zone};

fn game() -> Game {
    let defs = vec![
        CardDef::new("Life", "{0}", "Instant", "You gain 2 life.", None).unwrap(),
        CardDef::new("Void", "{0}", "Instant", "Exile target spell.", None).unwrap(),
        CardDef::new("Bear", "{0}", "Creature — Bear", "", Some((2, 2))).unwrap(),
        CardDef::new("Ability", "{0}", "Artifact", "{0}: You gain 1 life.", None).unwrap(),
    ];
    for def in &defs {
        game::play::check(def).unwrap();
    }
    Game::new(defs, 2, 20)
}

#[test]
fn r112_1_spell_targets_are_offered_and_the_offered_cast_executes() {
    let mut g = game();
    let life = g.put(0, 0, Zone::Hand);
    g.act(0, Move::Cast { card: life, targets: vec![] }).unwrap();
    let spell = g.stack[0];
    let exile = g.put(0, 1, Zone::Hand);
    cite!(["115.2", "601.2c"]; ok: g.act(0, Move::Cast { card: exile, targets: vec![vec![Target::Object(spell)]] }));
    for _ in 0..2 { g.act(g.priority, Move::Pass).unwrap(); }
    assert!(g.stack.is_empty());
    cite!(["112.1"]; assert_eq!(g.players[0].life, 20));
    assert_eq!(g.exile.len(), 1);
    cite!(["701.13a"]; assert_eq!(g.obj(g.exile[0]).chars.name, "Life"));
}

#[test]
fn r112_1_a_spell_target_is_neither_a_permanent_nor_an_activated_ability() {
    let mut g = game();
    let bear = g.put(0, 2, Zone::Battlefield);
    let source = g.put(0, 3, Zone::Battlefield);
    g.act(0, Move::Activate { source, ability: 0, targets: vec![], paying: vec![] }).unwrap();
    let ability = g.stack[0];
    let exile = g.put(0, 1, Zone::Hand);
    for (target, why) in [
        (Target::Object(bear), "an illegal target, Bear: it isn't a spell (115.1)"),
        (Target::Object(ability), "an illegal target, Ability: it isn't a spell (115.1)"),
        (Target::Player(1), "an illegal target, that player: a player isn't a spell (115.1)"),
    ] {
        cite!(["112.1", "115.1"]; case: why; refused(why): g.act(0, Move::Cast { card: exile, targets: vec![vec![target]] }));
        assert_eq!(g.players[0].hand, [exile]);
        assert_eq!(g.stack, [ability]);
    }

}
