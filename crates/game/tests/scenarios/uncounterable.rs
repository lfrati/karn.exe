//! Stack-only static abilities (113.6g) and their distinction from battlefield statics.
use crate::support::{cite, paid};
use game::{CardDef, Event, Game, Move, ObjId, Target, Zone};
use ir::{Color, mana::ManaSymbol};

fn game() -> Game {
    let defs = vec![
        CardDef::new("Sentinel", "{0}", "Creature — Golem", "This spell can't be countered.", Some((2, 2))).unwrap(),
        CardDef::new("Bear", "{0}", "Creature — Bear", "", Some((2, 2))).unwrap(),
        CardDef::new("Denial", "{0}", "Instant", "Counter target spell. You gain 1 life.", None).unwrap(),
        CardDef::new("Supreme Verdict", "{1}{W}{W}{U}", "Sorcery", "This spell can't be countered.\nDestroy all creatures.", None).unwrap(),
        CardDef::new("Shelter", "{0}", "Enchantment", "Creature spells you control can't be countered.", None).unwrap(),
        CardDef::new("Void", "{0}", "Instant", "Exile target spell.", None).unwrap(),
        CardDef::new("Conditional", "{0}", "Creature — Golem", "This spell can't be countered as long as you control an Island.", Some((2, 2))).unwrap(),
        CardDef::new("Island", "", "Basic Land — Island", "", None).unwrap(),
    ];
    for def in &defs { game::play::check(def).unwrap_or_else(|u| panic!("{}: {}", def.name(), u.0)); }
    Game::new(defs, 2, 20)
}

fn cast(g: &mut Game, def: usize, target: Option<ObjId>, spending: &str) -> ObjId {
    let card = g.put(g.priority, def, Zone::Hand);
    g.act(g.priority, paid(spending, Move::Cast { card, targets: target.map(|t| vec![vec![Target::Object(t)]]).unwrap_or_default() })).unwrap();
    *g.stack.last().unwrap()
}

fn pass(g: &mut Game) {
    for _ in 0..2 { g.act(g.priority, Move::Pass).unwrap(); }
}

fn respond(g: &mut Game, def: usize, target: ObjId) {
    g.act(g.priority, Move::Pass).unwrap();
    cast(g, def, Some(target), "");
    pass(g);
}

#[test]
fn r113_6g_a_permanent_spells_own_prohibition_functions_on_the_stack() {
    let mut g = game();
    let sentinel = cast(&mut g, 0, None, "");
    respond(&mut g, 2, sentinel);
    cite!(["113.6g"]; assert_eq!(g.stack, [sentinel]));
    cite!(["608.2c"]; assert_eq!(g.players[1].life, 21));
    cite!(["113.6g"]; assert!(!g.log.contains(&Event::Countered { object: sentinel })));
    pass(&mut g);
    assert_eq!(g.battlefield.len(), 1);
    let bear = cast(&mut g, 1, None, "");
    respond(&mut g, 2, bear);
    cite!(["701.6a"]; assert!(g.stack.is_empty(), "the Sentinel does not protect another spell"));
}

#[test]
fn r113_6g_supreme_verdict_is_uncounterable_before_its_instructions_resolve() {
    let mut g = game();
    g.put(0, 1, Zone::Battlefield);
    g.put(1, 1, Zone::Battlefield);
    for color in [Color::White, Color::White, Color::Blue, Color::Blue] {
        g.add_mana(0, ManaSymbol::Color(color));
    }
    let verdict = cast(&mut g, 3, None, "WWUU");
    respond(&mut g, 2, verdict);
    cite!(["113.6g"]; assert_eq!(g.stack, [verdict]));
    assert_eq!(g.battlefield.len(), 2, "the instruction has not resolved yet");
    pass(&mut g);
    cite!(["701.8a"]; assert!(g.battlefield.is_empty()));
    assert!(g.players[0].graveyard.iter().any(|o| g.obj(*o).chars.name == "Supreme Verdict"));
}

#[test]
fn r113_6_a_general_countering_prohibition_functions_only_on_the_battlefield() {
    for zone in [Zone::Hand, Zone::Graveyard, Zone::Battlefield] {
        let mut g = game();
        let shelter = g.put(0, 4, zone);
        let bear = cast(&mut g, 1, None, "");
        respond(&mut g, 2, bear);
        cite!(["113.6"]; case: format!("{zone:?}"); assert_eq!(g.stack.contains(&bear), zone == Zone::Battlefield));
        if zone == Zone::Battlefield {
            g.exile(shelter);
            respond(&mut g, 2, bear);
            cite!(["611.3b"]; assert!(g.stack.is_empty(), "the static effect ends when its source leaves"));
        }
    }
}

#[test]
fn r101_2_a_prohibition_on_countering_does_not_prevent_exile() {
    let mut g = game();
    let sentinel = cast(&mut g, 0, None, "");
    respond(&mut g, 5, sentinel);
    cite!(["101.2"]; assert!(g.stack.is_empty()));
    cite!(["101.2"]; assert_eq!(g.obj(g.exile[0]).chars.name, "Sentinel"));
}

#[test]
fn r611_3a_a_stack_statics_condition_is_evaluated_when_countering_would_happen() {
    for remove_land in [false, true] {
        let mut g = game();
        let island = g.put(0, 7, Zone::Battlefield);
        let spell = cast(&mut g, 6, None, "");
        g.act(0, Move::Pass).unwrap();
        cast(&mut g, 2, Some(spell), "");
        if remove_land { g.exile(island); }
        pass(&mut g);
        cite!(["611.3a"]; case: remove_land; assert_eq!(g.stack.contains(&spell), !remove_land));
    }
}
