//! Harmonize (702.180a): "You may cast this card from your graveyard by paying [cost] rather
//! than paying its mana cost. If the harmonize cost was paid, exile this card instead of
//! putting it anywhere else any time it would leave the stack." As it is cast, its caster may
//! tap an untapped creature they control, chosen with the route (601.2b); that creature's
//! power reduces only the generic mana of the total cost, after every increase (702.180a,
//! 601.2f, TDM release notes).
use crate::support::cite;
use game::announcement::{CastCost, CastRoute};
use game::{CardDef, Game, Move, ObjId, Target, Zone};
use ir::Part;

const STORIES: usize = 0;
const CHORUS: usize = 1;
const BEAR: usize = 2;
const GIANT: usize = 3;
const ISLAND: usize = 4;
const TAX: usize = 5;
const BOLT: usize = 6;
const WALL: usize = 7;
const SPELL: usize = 8;

fn game() -> Game {
    let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let real = |name: &str| CardDef::from_face(faces.iter().find(|f| f.name == name).unwrap()).unwrap();
    let c = |name, cost, types, text, pt| CardDef::new(name, cost, types, text, pt).expect(name);
    let defs = vec![
        real("Winternight Stories"),
        c("Chorus", "{2}{U}", "Sorcery", "You gain 1 life.\nHarmonize {4}{U}", None),
        c("Bear", "{0}", "Creature — Bear", "", Some((3, 2))),
        c("Giant", "{0}", "Creature — Giant", "", Some((6, 6))),
        c("Island", "", "Basic Land — Island", "", None),
        c("Tax", "{0}", "Artifact", "Noncreature spells cost {1} more to cast.", None),
        c("Harmonized Bolt", "{R}", "Instant", "Harmonize {2}{R}\nHarmonized Bolt deals 3 damage to target creature.", None),
        c("Wall", "{0}", "Creature — Wall", "", Some((-1, 4))),
        c("Spell", "{0}", "Instant", "You gain 1 life.", None),
    ];
    for d in &defs { game::play::check(d).unwrap_or_else(|e| panic!("{}: {}", d.name(), e.0)); }
    let mut g = Game::new(defs, 2, 20);
    for p in 0..2 { for _ in 0..10 { g.put(p, ISLAND, Zone::Library); } }
    while g.part != Part::PrecombatMain { g.act(g.priority, Move::Pass).unwrap(); }
    g
}
fn harmonize(card: ObjId, tapped: Option<ObjId>) -> Move {
    Move::CastVia(CastRoute::Harmonize { tapped }, Box::new(Move::Cast { card, targets: vec![] }))
}
fn settle(g: &mut Game) {
    for _ in 0..20 {
        match &g.decision {
            Some(d) => panic!("unexpected decision {d:?}"),
            None if g.stack.is_empty() => return,
            None => g.act(g.priority, Move::Pass).unwrap(),
        }
    }
    panic!("the stack did not settle");
}
fn blue(g: &mut Game, n: u32) { g.players[0].pool.colors[1] = n; }
/// A refused move changes nothing.
fn refused(g: &mut Game, mv: Move) -> String {
    let before = format!("{g:?}");
    let e = g.act(0, mv).unwrap_err();
    assert_eq!(format!("{g:?}"), before, "{e}");
    e
}

#[test]
fn r702_180a_cast_from_a_graveyard_for_its_harmonize_cost_then_exiled() {
    let mut g = game();
    let card = g.put(0, CHORUS, Zone::Graveyard);
    blue(&mut g, 5);
    g.act(0, crate::support::paid("UUUUU",harmonize(card, None))).unwrap();
    cite!(["702.180a"]; assert_eq!(g.players[0].pool.total(), 0, "{{4}}{{U}} paid"));
    settle(&mut g);
    assert_eq!(g.players[0].life, 21);
    cite!(["702.180a"]; assert!(g.players[0].graveyard.is_empty()));
    assert_eq!(g.exile.len(), 1);
    cite!(["702.180a"]; assert_eq!(g.obj(g.exile[0]).card, Some(CHORUS)));
}

#[test]
fn r702_180a_harmonize_is_only_from_a_graveyard_and_its_normal_cast_is_unchanged() {
    let mut g = game();
    let card = g.put(0, CHORUS, Zone::Hand);
    blue(&mut g, 5);
    let e = refused(&mut g, harmonize(card, None));
    cite!(["702.180a"]; assert_eq!(e, "that card cannot be cast using harmonize (702.180a)"));
    // from the hand, its mana cost, and to the graveyard as usual
    g.act(0, crate::support::paid("UUU",Move::Cast { card, targets: vec![] })).unwrap();
    cite!(["601.2f"]; assert_eq!(g.players[0].pool.total(), 2));
    settle(&mut g);
    cite!(["608.2n"]; assert_eq!(g.players[0].graveyard.len(), 1));
    assert!(g.exile.is_empty());
}

#[test]
fn r702_180a_the_tapped_creatures_power_reduces_generic_mana() {
    let mut g = game();
    let card = g.put(0, CHORUS, Zone::Graveyard);
    let bear = g.put(0, BEAR, Zone::Battlefield);
    blue(&mut g, 2);
    g.act(0, crate::support::paid("UU",harmonize(card, Some(bear)))).unwrap();
    cite!(["702.180a"]; assert_eq!(g.players[0].pool.total(), 0, "{{1}}{{U}}: 3 of the {{4}} paid by tapping a 3-power creature"));
    cite!(["702.180b"]; assert!(g.obj(bear).tapped, "tapped as the cost is paid"));
    settle(&mut g);
    assert_eq!(g.players[0].life, 21);
    assert_eq!(g.exile.len(), 1);
}

#[test]
fn r702_180a_only_generic_mana_is_reduced() {
    let mut g = game();
    let card = g.put(0, CHORUS, Zone::Graveyard);
    let giant = g.put(0, GIANT, Zone::Battlefield);
    // power 6 against {4}{U}: the {U} remains
    let e = refused(&mut g, harmonize(card, Some(giant)));
    cite!(["702.180a", "118.3a"]; assert_eq!(e, "your mana pool cannot pay that cost (118.3a): {U} in total, {4} less than it was (601.2f)"));
    blue(&mut g, 1);
    cite!(["702.180a"]; ok: g.act(0, crate::support::paid("U",harmonize(card, Some(giant)))));
    assert_eq!(g.players[0].pool.total(), 0);
    assert!(g.obj(giant).tapped);
}

#[test]
fn r601_2f_increases_apply_before_the_harmonize_reduction() {
    // {4}{U} plus {1} is {5}{U}; tapping a 6-power creature then leaves {U}, not {1}{U}
    let mut g = game();
    g.put(1, TAX, Zone::Battlefield);
    let card = g.put(0, CHORUS, Zone::Graveyard);
    let giant = g.put(0, GIANT, Zone::Battlefield);
    blue(&mut g, 1);
    cite!(["601.2f", "702.180a"]; ok: g.act(0, crate::support::paid("U",harmonize(card, Some(giant)))));
    assert_eq!(g.players[0].pool.total(), 0);
    // and without tapping, the tax is paid on top of the harmonize cost
    let mut g = game();
    g.put(1, TAX, Zone::Battlefield);
    let card = g.put(0, CHORUS, Zone::Graveyard);
    blue(&mut g, 5);
    refused(&mut g, crate::support::paid("UUUUU",harmonize(card, None)));
    blue(&mut g, 6);
    g.act(0, crate::support::paid("UUUUUU",harmonize(card, None))).unwrap();
    cite!(["118.9d"]; assert_eq!(g.players[0].pool.total(), 0));
}

#[test]
fn r107_1b_a_creature_with_negative_power_reduces_nothing() {
    let mut g = game();
    let card = g.put(0, CHORUS, Zone::Graveyard);
    let wall = g.put(0, WALL, Zone::Battlefield);
    blue(&mut g, 5);
    g.act(0, crate::support::paid("UUUUU",harmonize(card, Some(wall)))).unwrap();
    cite!(["107.1b"]; assert_eq!(g.players[0].pool.total(), 0, "the whole {{4}}{{U}} is paid"));
    cite!(["702.180b"]; assert!(g.obj(wall).tapped, "it is still tapped: the choice was made"));
}

#[test]
fn r702_180b_the_creature_is_an_untapped_creature_its_caster_controls() {
    let mut g = game();
    let card = g.put(0, CHORUS, Zone::Graveyard);
    let theirs = g.put(1, GIANT, Zone::Battlefield);
    let tapped = g.put(0, GIANT, Zone::Battlefield);
    g.tap(tapped);
    let island = g.put(0, ISLAND, Zone::Battlefield);
    let in_hand = g.put(0, GIANT, Zone::Hand);
    blue(&mut g, 5);
    for not in [theirs, tapped, island, in_hand] {
        let e = refused(&mut g, harmonize(card, Some(not)));
        cite!(["702.180b"]; case: format!("{not:?}"); assert_eq!(e, "the creature tapped for harmonize is an untapped creature you control (702.180b)"));
    }
    // a creature that came under its control this turn may be tapped: it is not {T} (302.6)
    let fresh = g.put(0, BEAR, Zone::Hand);
    g.act(0, Move::Cast { card: fresh, targets: vec![] }).unwrap();
    settle(&mut g);
    let fresh = *g.battlefield.last().unwrap();
    assert!(g.summoning_sick(fresh));
    blue(&mut g, 2);
    cite!(["702.180a"]; ok: g.act(0, crate::support::paid("UU",harmonize(card, Some(fresh)))));
    assert!(g.obj(fresh).tapped);
}

#[test]
fn r702_180a_authored_casts_select_each_untapped_creature_or_none() {
    let mut g = game();
    let card = g.put(0, CHORUS, Zone::Graveyard);
    let bear = g.put(0, BEAR, Zone::Battlefield);
    let giant = g.put(0, GIANT, Zone::Battlefield);
    let tapped = g.put(0, GIANT, Zone::Battlefield);
    g.tap(tapped);
    g.put(1, GIANT, Zone::Battlefield);
    blue(&mut g, 5);
    for (selected, units) in [(None, "UUUUU"), (Some(bear), "UU"), (Some(giant), "U")] {
        cite!(["702.180a"]; ok: g.clone().act(0, crate::support::paid(units, harmonize(card, selected))));
    }
    cite!(["702.180a"]; refused("the creature tapped for harmonize is an untapped creature you control"): unchanged(&mut g, |g| g.act(0, crate::support::paid("U", harmonize(card, Some(tapped))))));
    blue(&mut g, 1);
    for selected in [None, Some(bear)] {
        cite!(["702.180a", "601.2h"]; refused("the supplied mana does not pay that cost, all of it and no more"): unchanged(&mut g, |g| g.act(0, crate::support::paid("U",harmonize(card, selected)))));
    }
    cite!(["702.180a"]; ok: g.act(0, crate::support::paid("U",harmonize(card, Some(giant)))));

}

#[test]
fn r118_9a_no_other_alternative_cost_with_harmonize() {
    let mut g = game();
    let card = g.put(0, CHORUS, Zone::Graveyard);
    blue(&mut g, 5);
    let e = refused(&mut g, Move::CastFor(CastCost::Alternative(1), Box::new(harmonize(card, None))));
    cite!(["118.9a"]; assert_eq!(e, "only one alternative cost is paid for a spell (118.9a)"));
}

#[test]
fn r117_1a_harmonize_keeps_normal_timing() {
    let mut g = game();
    let card = g.put(0, CHORUS, Zone::Graveyard);
    blue(&mut g, 5);
    // a sorcery is not cast with a spell on the stack
    let spell = g.put(0, SPELL, Zone::Hand);
    g.act(0, Move::Cast { card: spell, targets: vec![] }).unwrap();
    let e = g.act(0, harmonize(card, None)).unwrap_err();
    cite!(["117.1a"]; assert_eq!(e, "that is cast in a main phase of your own turn with the stack empty (117.1a)"));
}

#[test]
fn r702_180a_a_harmonized_spell_that_fails_to_resolve_is_exiled() {
    // its only target is gone as it resolves: it does not resolve (608.2b), and leaves the
    // stack for exile rather than its owner's graveyard
    let mut g = game();
    let card = g.put(0, BOLT, Zone::Graveyard);
    let target = g.put(1, BEAR, Zone::Battlefield);
    g.players[0].pool.colors[3] = 3;
    g.act(0, crate::support::paid("RRR",Move::CastVia(CastRoute::Harmonize { tapped: None }, Box::new(Move::Cast { card, targets: vec![vec![Target::Object(target)]] })))).unwrap();
    assert_eq!(g.players[0].pool.total(), 0);
    g.move_object(target, Zone::Hand);
    settle(&mut g);
    cite!(["702.180a", "608.2b"]; assert!(g.players[0].graveyard.is_empty()));
    assert_eq!(g.exile.len(), 1);
    cite!(["702.180a"]; assert_eq!(g.obj(g.exile[0]).card, Some(BOLT)));
}

#[test]
fn r702_180_winternight_stories_casts_from_a_graveyard_for_its_reduced_harmonize_cost() {
    let mut g = game();
    let card = g.put(0, STORIES, Zone::Graveyard);
    let bear = g.put(0, BEAR, Zone::Battlefield);
    blue(&mut g, 2);
    g.act(0, crate::support::paid("UU",harmonize(card, Some(bear)))).unwrap();
    cite!(["702.180a"]; assert_eq!(g.players[0].pool.total(), 0));
    assert!(matches!(g.decision, None));
    assert_eq!(g.stack.len(), 1);
    assert_eq!(g.obj(g.stack[0]).card, Some(STORIES));
}

#[track_caller]
fn unchanged(g: &mut Game, action: impl FnOnce(&mut Game) -> Result<(), String>) -> Result<(), String> {
    let before=format!("{g:?}");
    let result=action(g);
    assert_eq!(format!("{g:?}"),before);
    result
}
