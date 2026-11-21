//! Wither's damage results
//! (702.80a, 120.3d) beside the other abilities of the same source. Wither changes only how
//! damage to a creature is processed: it is still damage, so deathtouch destroys (702.2b), and
//! lifelink gains (702.15b). A source that has left the battlefield deals damage with its last
//! known abilities (702.80b, 120.3, 702.15c).
use crate::support::cite;
use game::{CardDef, Event, Game, Move, Target, Zone};

fn def(name: &str, text: &str, pt: (i32, i32)) -> CardDef {
    let d = CardDef::new(name, "{0}", "Creature", text, Some(pt)).unwrap();
    game::play::check(&d).unwrap_or_else(|e| panic!("{text}: {}", e.0));
    d
}
fn game() -> Game {
    let defs = vec![
        def("Blight", "Wither\nDeathtouch\nLifelink", (1, 1)),
        def("Wall", "", (5, 5)),
        def("Martyr", "Wither\nLifelink\nSacrifice this creature: This creature deals 2 damage to target creature.", (1, 1)),
    ];
    Game::new(defs, 2, 20)
}
fn settle(g: &mut Game) {
    g.state_based_actions();
    for _ in 0..20 {
        assert!(g.decision.is_none(), "{:?}", g.decision);
        if g.stack.is_empty() { return; }
        g.act(g.priority, Move::Pass).unwrap();
    }
    panic!("the stack did not settle")
}

#[test]
fn r702_2b_wither_damage_from_a_deathtouch_source_is_counters_and_still_destroys() {
    let mut g = game();
    let blight = g.put(0, 0, Zone::Battlefield);
    let wall = g.put(1, 1, Zone::Battlefield);
    let from = g.log.iter().count();
    g.damage(blight, Target::Object(wall), 1);
    cite!(["702.80a", "120.3d"]; assert_eq!((g.counters(wall).get("-1/-1"), g.obj(wall).damage), (Some(&1), 0)));
    cite!(["702.15b"]; assert_eq!(g.players[0].life, 21, "lifelink"));
    settle(&mut g);
    cite!(["704.5h"]; assert_eq!(g.obj(wall).zone, Zone::Gone, "destroyed by deathtouch (704.5h)"));
    let events: Vec<_> = g.log.iter().skip(from).filter(|e| matches!(e, Event::DamageDealt { .. } | Event::CountersPut { .. } | Event::LifeGained { .. })).collect();
    // CR120.4b precedes result processing, but CR120.4c/d does not order the
    // serialized result records. CR603.10 observes the completed event: the
    // recipient is 4/4 after the counter, before deathtouch's later SBA.
    cite!(["120.4b", "120.4c", "120.4d"]; assert_eq!(events.len(), 3));
    cite!(["120.4b"]; assert!(matches!(events[0], Event::DamageDealt { source, to: Target::Object(on), amount: 1, combat: false } if *source == blight && *on == wall)));
    cite!(["120.3d", "603.10", "122.6"]; assert!(events.iter().any(|e| matches!(e,
        Event::CountersPut { on: Target::Object(on), kind, count: 1, before: 0, by: 0, recipient: Some(seen) }
        if *on == wall && kind == "-1/-1" && seen.pt == Some((4, 4)) && seen.controller == 1
    )), "{events:?}"));
    cite!(["120.3f", "702.15b"]; assert!(events.iter().any(|e| matches!(e, Event::LifeGained { player: 0, amount: 1 })), "{events:?}"));
}

#[test]
fn r702_80_a_sacrificed_source_deals_wither_and_lifelink_damage_with_last_known_abilities() {
    let mut g = game();
    let martyr = g.put(0, 2, Zone::Battlefield);
    let wall = g.put(1, 1, Zone::Battlefield);
    g.act(0, Move::Activate { source: martyr, ability: 2, targets: vec![vec![Target::Object(wall)]], paying: vec![] }).unwrap();
    assert_eq!(g.obj(martyr).zone, Zone::Gone);
    settle(&mut g);
    cite!(["702.80b"]; assert_eq!((g.counters(wall).get("-1/-1"), g.obj(wall).damage), (Some(&2), 0)));
    assert_eq!(g.seen(wall, None).pt, Some((3, 3)));
    cite!(["702.15c"]; assert_eq!(g.players[0].life, 22));
}
