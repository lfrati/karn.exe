//! Populate and untargeted copy choices compose with copiable values and resumable entry.
use crate::support::cite::cite;
use game::{CardDef, Game, Move, Target, Zone, Event, ObjId};
use game::state::{Answer, Choice, Decision};

fn game() -> Game {
    let c = |name, types, text, pt| CardDef::new(name, "{0}", types, text, pt).unwrap();
    let defs = vec![
        c("Populate", "Instant", "Populate.", None),
        c("Soldiers", "Instant", "Create a 1/1 white Soldier creature token.", None),
        c("Spirits", "Instant", "Create a 2/2 white Spirit creature token with flying and shroud.", None),
        c("Grow", "Instant", "Creatures you control get +3/+3 until end of turn.", None),
        c("Chooser", "Creature — Human", "As this creature enters, choose a color.", Some((2, 2))),
        c("Copy and tap", "Instant", "Create a token that's a copy of a creature you control. Tap it. You gain 1 life.", None),
        c("Bear", "Creature — Bear", "", Some((2, 2))),
    ];
    for d in &defs { game::play::check(d).unwrap_or_else(|e| panic!("{}: {}", d.name(), e.0)); }
    Game::new(defs, 2, 20)
}
fn pass(g: &mut Game) { for _ in 0..2 { g.act(g.priority, Move::Pass).unwrap(); } }
fn cast(g: &mut Game, def: usize) {
    let card = g.put(0, def, Zone::Hand);
    g.act(0, Move::Cast { card, targets: vec![] }).unwrap();
    pass(g);
}
fn copy_choice(g: &Game) -> Vec<ObjId> {
    let Some(Decision::Choose { player: 0, choice: Choice::Objects { verb: ir::Verb::CreateCopy, among, count: 1, .. } }) = g.decision.clone() else { panic!("{:?}", g.decision) };
    among
}

#[test]
fn r707_2_populate_copies_values_without_counters_taps_or_later_effects() {
    // Rules: 701.36a, 707.2, 608.2d, 702.18a
    let mut g = game();
    cast(&mut g, 1);
    cast(&mut g, 2);
    let spirit = *g.battlefield.iter().find(|o| g.has(**o, "Shroud")).unwrap();
    g.put_counters(spirit, "+1/+1", 2);
    g.set_tapped(spirit, true);
    cast(&mut g, 3);
    // Setup sanity: counters and a later effect changed the original before copying.
    assert_eq!(g.power(spirit), Some(7));
    let before = g.battlefield.clone();
    cast(&mut g, 0);
    let among = copy_choice(&g);
    cite!(["701.36a", "702.18a"]; assert!(among.contains(&spirit), "a populate choice does not target"));
    cite!(["701.36a"]; assert!(!g.log.iter().any(|e| matches!(e, Event::BecameTarget { object: Target::Object(o), .. } if *o == spirit))));
    g.act(0, Move::Choose(Answer::Objects(vec![spirit]))).unwrap();
    let made = *g.battlefield.iter().find(|o| !before.contains(o)).unwrap();
    cite!(["707.2"]; assert_eq!(g.power(made), Some(2)));
    cite!(["707.2a"]; assert!(g.has(made, "Flying") && g.has(made, "Shroud")));
    cite!(["707.2"]; assert!(g.obj(made).counters.is_empty()));
    cite!(["707.2", "110.5b"]; assert!(!g.obj(made).tapped));
    cite!(["111.2"]; assert_eq!(g.obj(made).owner, 0));
}

#[test]
fn r701_36a_the_original_is_chosen_at_resolution_including_tokens_created_in_response() {
    // Rules: 701.36a, 608.2d
    let mut g = game();
    let population = g.put(0, 0, Zone::Hand);
    g.act(0, Move::Cast { card: population, targets: vec![] }).unwrap();
    cast(&mut g, 1);
    cast(&mut g, 2);
    // Setup: populate is still waiting below the token-creation responses.
    assert_eq!(g.stack.len(), 1);
    pass(&mut g);
    let among = copy_choice(&g);
    cite!(["701.36a", "608.2d"]; assert_eq!(among.len(), 2));
    let spirit = *among.iter().find(|o| g.has(**o, "Flying")).unwrap();
    g.act(0, Move::Choose(Answer::Objects(vec![spirit]))).unwrap();
    cite!(["701.36a", "707.2a"]; assert_eq!(g.battlefield.iter().filter(|o| g.has(**o, "Flying")).count(), 2));
}

#[test]
fn r707_6_a_chosen_originals_copy_makes_its_own_entry_choice_and_later_it_names_the_copy() {
    // Rules: 707.6, 608.2c, 608.2d
    let mut g = game();
    let original = g.put(0, 4, Zone::Hand);
    g.act(0, Move::Cast { card: original, targets: vec![] }).unwrap();
    pass(&mut g);
    g.act(0, Move::Choose(Answer::Value(game::binding::Value::Color(ir::Color::Red)))).unwrap();
    let original = g.battlefield[0];
    g.put(0, 6, Zone::Battlefield);
    cast(&mut g, 5);
    cite!(["608.2d"]; assert!(copy_choice(&g).contains(&original)));
    g.act(0, Move::Choose(Answer::Objects(vec![original]))).unwrap();
    cite!(["707.6"]; assert!(g.decision.is_some(), "the copied entry choice pauses creation"));
    cite!(["707.6", "608.2c"]; assert_eq!(g.battlefield.len(), 2));
    cite!(["707.6", "608.2c"]; assert_eq!(g.players[0].life, 20));
    let mut clone = g.clone();
    for (g, color) in [(&mut g, ir::Color::Blue), (&mut clone, ir::Color::Green)] {
        g.act(0, Move::Choose(Answer::Value(game::binding::Value::Color(color)))).unwrap();
        let made = *g.battlefield.iter().find(|o| g.obj(**o).token()).unwrap();
        // which object "it" names is Oracle-text reading, which no rule states (608.2c orders
        // instructions): uncited, as in r400_3_named_zone_owners_review
        cite!(["608.2c"]; case: format!("entry color {color:?}"); assert!(g.obj(made).tapped, "later it names the token, not its chosen original"));
        assert!(!g.obj(original).tapped);
        cite!(["608.2c"]; case: format!("entry color {color:?}"); assert_eq!(g.players[0].life, 21, "then it gains 1 life"));
        cite!(["707.6", "608.2c"]; case: format!("entry color {color:?}"); assert_eq!(g.log.iter().filter(|e| matches!(e, Event::Created { .. })).count(), 1));
    }
}

#[test]
fn r608_2d_unsupported_copy_choosers_and_optional_populate_are_refused() {
    // These are implementation admission limits for otherwise legal instructions.
    // Broad refusal guards stay uncited rather than crediting them as rule evidence.
    for text in ["You may populate.", "Each opponent populates.",
                 "You may create a token that's a copy of a creature you control.",
                 "Target opponent creates a token that's a copy of a creature they control."] {
        let d=CardDef::new("Unsupported", "{0}", "Sorcery", text, None).unwrap();
        crate::support::refused(game::play::check(&d).map_err(|e|e.0),"chosen copy original: one mandatory choice by the controller");
    }
}
