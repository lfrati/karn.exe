//! Delayed triggered abilities (CR 603.7): an effect makes an ability that triggers later.
//! Every card is read from its printed text.

use crate::support::{cite, paid};
use game::card::CardDef;
use game::play::check;
use game::state::Decision;
use game::{Game, Move, ObjId, Target, Zone};
use ir::Part;

const MOUNTAIN: usize = 0;
const BEARS: usize = 1;
const GIANT: usize = 2;
const BLOOD: usize = 3;
const SHOCK: usize = 4;
const BOLT: usize = 5;

fn defs() -> Vec<CardDef> {
    let card = |name, cost, types, text, pt| CardDef::new(name, cost, types, text, pt).expect(name);
    vec![
        card("Mountain", "", "Basic Land — Mountain", "({T}: Add {R}.)", None),
        card("Grizzly Bears", "{1}{G}", "Creature — Bear", "", Some((2, 2))),
        card("Hill Giant", "{3}{R}", "Creature — Giant", "", Some((3, 3))),
        card(
            "Searing Blood",
            "{R}{R}",
            "Instant",
            "Searing Blood deals 2 damage to target creature. When that creature dies this turn, Searing Blood deals 3 damage to the creature's controller.",
            None,
        ),
        card("Shock", "{R}", "Instant", "Shock deals 2 damage to any target.", None),
        card("Lightning Bolt", "{R}", "Instant", "Lightning Bolt deals 3 damage to any target.", None),
    ]
}

fn game() -> Game {
    let mut g = Game::new(defs(), 2, 20);
    for player in [0, 1] {
        for _ in 0..4 {
            g.put(player, MOUNTAIN, Zone::Library);
        }
    }
    g
}

fn pass_both(g: &mut Game) {
    let first = g.priority;
    g.act(first, Move::Pass).unwrap();
    g.act(1 - first, Move::Pass).unwrap();
}

/// Player 0 casts this card with the mana of that many Mountains at this target; it resolves.
fn cast(g: &mut Game, def: usize, mountains: usize, target: ObjId, spending: &str) {
    for _ in 0..mountains {
        let land = g.put(0, MOUNTAIN, Zone::Battlefield);
        g.act(0, Move::Activate { source: land, ability: 0, targets: vec![], paying: vec![] }).unwrap();
    }
    let card = g.put(0, def, Zone::Hand);
    g.act(0, paid(spending, Move::Cast { card, targets: vec![vec![Target::Object(target)]] })).unwrap();
    pass_both(g);
}

#[test]
fn every_card_of_the_tests_is_one_the_engine_runs() {
    for d in defs() {
        check(&d).unwrap_or_else(|u| panic!("{}: {}", d.name(), u.0));
    }
}

#[test]
fn r603_7a_an_effect_makes_an_ability_that_triggers_when_its_event_happens_later() {
    let mut g = game();
    let bears = g.put(1, BEARS, Zone::Battlefield);
    // "Searing Blood deals 2 damage to target creature. When that creature dies this turn,
    // Searing Blood deals 3 damage to the creature's controller."
    cast(&mut g, BLOOD, 2, bears, "RR");
    // the creature died as state-based actions were checked, after the spell had resolved:
    // the ability the spell made triggered and is on the stack, its controller the player
    // who controlled the spell (603.7d)
    cite!(["704.5g"]; assert_eq!(g.obj(bears).zone, Zone::Gone));
    let [ability] = g.stack[..] else { panic!("no ability on the stack: {:?}", g.stack) };
    cite!(["603.7d"]; assert_eq!(g.obj(ability).controller, 0));
    assert_eq!(g.players[1].life, 20);
    pass_both(&mut g);
    // "the creature's controller": the player who last controlled it (608.2h)
    cite!(["608.2h"]; assert_eq!((g.players[0].life, g.players[1].life), (20, 17)));
    assert!(g.stack.is_empty());
}

#[test]
fn r603_7b_with_a_stated_duration_it_triggers_for_that_long_and_no_longer() {
    // the creature survives the 2 damage and dies later in the turn
    let mut g = game();
    let giant = g.put(1, GIANT, Zone::Battlefield);
    cast(&mut g, BLOOD, 2, giant, "RR");
    assert_eq!(g.obj(giant).zone, Zone::Battlefield);
    assert!(g.stack.is_empty());
    cast(&mut g, SHOCK, 1, giant, "R");
    pass_both(&mut g);
    cite!(["603.7b"]; assert_eq!(g.players[1].life, 17));
    // "this turn": it dies in a later turn, and nothing triggers (514.2)
    let mut g = game();
    let giant = g.put(1, GIANT, Zone::Battlefield);
    cast(&mut g, BLOOD, 2, giant, "RR");
    while !(g.turn == 3 && g.part == Part::PrecombatMain) {
        match g.decision.clone() {
            Some(Decision::Attackers { player }) => g.act(player, Move::Attack(vec![])).unwrap(),
            _ => g.act(g.priority, Move::Pass).unwrap(),
        }
    }
    cast(&mut g, BOLT, 1, giant, "R");
    assert_eq!(g.obj(giant).zone, Zone::Gone);
    assert!(g.stack.is_empty());
    cite!(["603.7b", "514.2"]; assert_eq!(g.players[1].life, 20));
}

#[test]
fn r603_7c_it_is_of_the_particular_object_its_words_named_when_it_was_made() {
    let mut g = game();
    let giant = g.put(1, GIANT, Zone::Battlefield);
    let bears = g.put(1, BEARS, Zone::Battlefield);
    cast(&mut g, BLOOD, 2, giant, "RR");
    // another creature of that player dies this turn: not "that creature"
    cast(&mut g, SHOCK, 1, bears, "R");
    assert_eq!(g.obj(bears).zone, Zone::Gone);
    cite!(["603.7c"]; assert!(g.stack.is_empty()));
    assert_eq!(g.players[1].life, 20);
    // An unrelated death must not consume the pending trigger.
    cast(&mut g, SHOCK, 1, giant, "R");
    cite!(["603.7c"]; assert_eq!(g.stack.len(), 1));
    pass_both(&mut g);
    assert_eq!(g.players[1].life, 17);
}
