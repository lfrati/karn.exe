//! Distinct colors are a union of current effective colors, not a permanent count (105.2–3).
use crate::support::{cite, paid};
use game::{CardDef, Game, Move, Target, Zone};
use std::sync::OnceLock;

fn native(name: &str) -> CardDef {
    static FACES: OnceLock<Vec<oracle::corpus::Face>> = OnceLock::new();
    let faces = FACES.get_or_init(|| oracle::corpus::load(&oracle::corpus::default_path()).unwrap());
    CardDef::from_faces(&CardDef::catalog(faces)[name]).unwrap()
}
fn permanent(name: &str, cost: &str) -> CardDef {
    CardDef::new(name, cost, "Artifact", "", None).unwrap()
}
fn gain() -> CardDef {
    CardDef::new("Union", "{0}", "Instant", "You gain life equal to the number of colors among permanents you control.", None).unwrap()
}
fn game(defs: Vec<CardDef>) -> Game {
    for d in &defs { game::play::check(d).unwrap_or_else(|e| panic!("{}: {}", d.name(), e.0)); }
    Game::new(defs, 2, 20)
}
fn pass(g: &mut Game) { for _ in 0..2 { g.act(g.priority, Move::Pass).unwrap(); } }
fn settle(g: &mut Game) { for _ in 0..20 { if g.stack.is_empty() { return; } pass(g); } panic!("unsettled"); }
fn cast(g: &mut Game, index: usize, mana: &str) {
    let card = g.put(0, index, Zone::Hand);
    g.act(0, paid(mana, Move::Cast { card, targets: vec![] })).unwrap(); settle(g);
}

#[test]
fn r105_2_colorless_and_empty_sets_contribute_zero() {
    let mut g = game(vec![gain(), permanent("Model", "{5}")]);
    cast(&mut g, 0, ""); g.put(0, 1, Zone::Battlefield); cast(&mut g, 0, "");
    cite!(["105.2c", "608.2h"]; assert_eq!(g.players[0].life, 20));
}
#[test]
fn r105_2_overlapping_multicolored_objects_are_a_union() {
    let mut g = game(vec![gain(), permanent("First", "{W}{U}"), permanent("Second", "{U}{B}"), permanent("Foreign", "{R}{G}")]);
    g.put(0, 1, Zone::Battlefield); g.put(0, 1, Zone::Battlefield); g.put(0, 2, Zone::Battlefield); g.put(1, 3, Zone::Battlefield);
    cast(&mut g, 0, "");
    cite!(["105.2", "202.2c"]; assert_eq!(g.players[0].life, 23));
}
#[test]
fn r105_2_all_five_colors_on_one_object_count_five() {
    let mut g = game(vec![gain(), permanent("Spectrum", "{W}{U}{B}{R}{G}")]);
    g.put(0, 1, Zone::Battlefield); cast(&mut g, 0, "");
    cite!(["105.2", "105.4"]; assert_eq!(g.players[0].life, 25));
}
#[test]
fn r202_2d_hybrid_colors_do_not_depend_on_the_mana_paid() {
    let mut g = game(vec![native("Glister Bairn"), native("Luminollusk")]);
    g.players[0].pool.colors[ir::Color::Green as usize] = 5; cast(&mut g, 0, "GGGGG");
    g.players[0].pool.colors[ir::Color::Green as usize] = 4; cast(&mut g, 1, "GGGG");
    cite!(["202.2d", "608.2h"]; assert_eq!(g.players[0].life, 22));
}
#[test]
fn r608_2h_native_entry_quantity_reads_resolution_state() {
    let mut g = game(vec![native("Luminollusk"), permanent("Blue", "{U}")]);
    let blue = g.put(0, 1, Zone::Battlefield);
    g.players[0].pool.colors[ir::Color::Green as usize] = 4;
    let card = g.put(0, 0, Zone::Hand);
    g.act(0, paid("GGGG", Move::Cast { card, targets: vec![] })).unwrap(); pass(&mut g);
    assert_eq!(g.stack.len(), 1); // the enter trigger, after the permanent spell resolved
    g.move_object(blue, Zone::Graveyard); settle(&mut g);
    cite!(["608.2h"]; assert_eq!(g.players[0].life, 21));
}
#[test]
fn r604_3_native_characteristic_definition_updates_in_all_zones() {
    let mut g = game(vec![native("Squawkroaster"), permanent("Blue", "{U}"), permanent("White", "{W}")]);
    let blue = g.put(0, 1, Zone::Battlefield);
    let squawk = g.put(0, 0, Zone::Graveyard);
    cite!(["604.3", "113.6a"]; assert_eq!(g.power(squawk), Some(1)));
    g.put(0, 2, Zone::Battlefield); assert_eq!(g.power(squawk), Some(2));
    g.move_object(blue, Zone::Graveyard); assert_eq!(g.power(squawk), Some(1));
}
#[test]
fn r601_2f_native_generic_cost_reduction_counts_colors_not_objects() {
    let mut g = game(vec![native("Wildvine Pummeler"), permanent("Hybrid", "{G/U}")]);
    g.put(0, 1, Zone::Battlefield); g.put(0, 1, Zone::Battlefield);
    g.players[0].pool.colors[ir::Color::Green as usize] = 5;
    cast(&mut g, 0, "GGGGG");
    cite!(["601.2f", "118.7a", "202.2d"]; assert_eq!(g.players[0].pool.colors[ir::Color::Green as usize], 0));
}
#[test]
fn r105_3_supported_color_changes_use_effective_characteristics() {
    let change = CardDef::new("Paint", "{0}", "Instant", "Target creature becomes red until end of turn.", None).unwrap();
    let bear = CardDef::new("Bear", "{G}", "Creature — Bear", "", Some((2, 2))).unwrap();
    let mut g = game(vec![gain(), change, bear, permanent("Green", "{G}")]);
    let b = g.put(0, 2, Zone::Battlefield); g.put(0, 3, Zone::Battlefield);
    let card = g.put(0, 1, Zone::Hand);
    g.act(0, Move::Cast { card, targets: vec![vec![Target::Object(b)]] }).unwrap(); settle(&mut g); cast(&mut g, 0, "");
    cite!(["105.3", "613.1e"]; assert_eq!(g.players[0].life, 22));
}

#[test]
fn r601_2f_color_reduction_locks_before_a_colored_mana_source_is_sacrificed() {
    let rock = CardDef::new("Spent Color", "{U}", "Artifact", "{T}, Sacrifice this artifact: Add {C}{C}{C}{C}{C}.", None).unwrap();
    let mut g = game(vec![native("Wildvine Pummeler"), rock]);
    let source = g.put(0, 1, Zone::Battlefield);
    let card = g.put(0, 0, Zone::Hand);
    g.players[0].pool.colors[ir::Color::Green as usize] = 1;
    let step = game::legal::Step { plan: vec![], x: None, life: vec![], with: None, cost_branches: None, source, ability: 0, chose: vec![], paying: vec![], adding: vec![] };
    g.act(0, Move::WithMana(vec![step], Box::new(paid("CCCCCG", Move::Cast { card, targets: vec![] })))).unwrap();
    settle(&mut g);
    cite!(["601.2f", "601.2g", "601.2h"]; assert_eq!(g.players[0].pool.total(), 0));
    assert!(g.battlefield.iter().any(|o| g.obj(*o).chars.name == "Wildvine Pummeler"));
    assert!(!g.battlefield.contains(&source));
}
#[test]
fn r105_2_native_other_artifact_filter_excludes_the_five_color_source() {
    let mut g = game(vec![native("Karn, Gilded Guardian"), permanent("Hybrid", "{W/U}")]);
    for _ in 0..5 { g.put(0, 1, Zone::Library); }
    g.put(0, 1, Zone::Battlefield); g.put(0, 1, Zone::Battlefield);
    let source = g.put(0, 0, Zone::Hand); g.move_object(source, Zone::Battlefield);
    g.act(g.priority, Move::Pass).unwrap(); settle(&mut g);
    cite!(["105.2", "202.2d", "608.2h"]; assert_eq!(g.players[0].hand.len(), 2));
}
#[test]
fn unmodeled_distinct_characteristics_remain_explicitly_refused() {
    // colors, card types, permanent types, basic land types, powers and mana values are
    // counted (r305_6_basic_land_types_among); kinds of counters are not
    let native = native("Hundred-Battle Veteran");
    let error = game::play::check(&native).unwrap_err();
    assert!(error.0.contains("amount: Distinct"), "{error:?}");
}

#[test]
fn r613_1e_effective_colors_feed_native_power_cda() {
    let paint = CardDef::new("Paint", "{0}", "Instant", "Target creature becomes red until end of turn.", None).unwrap();
    let blue = CardDef::new("Blue Bear", "{U}", "Creature — Bear", "", Some((2, 2))).unwrap();
    let mut g = game(vec![native("Squawkroaster"), paint, blue]);
    let source = g.put(0, 0, Zone::Battlefield); let bear = g.put(0, 2, Zone::Battlefield);
    assert_eq!(g.power(source), Some(2));
    let card = g.put(0, 1, Zone::Hand);
    g.act(0, Move::Cast { card, targets: vec![vec![Target::Object(bear)]] }).unwrap(); settle(&mut g);
    cite!(["105.3", "613.1e", "613.1g"]; assert_eq!(g.power(source), Some(1)));
}
#[test]
fn r603_3d_triggered_target_bound_reads_state_at_stack_placement() {
    let effect = CardDef::new("Change Before Placement", "{0}", "Instant", "Return target creature card from your graveyard to the battlefield. Exile all artifacts.", None).unwrap();
    let bear = CardDef::new("Model Bear", "{0}", "Creature — Bear", "", Some((2, 2))).unwrap();
    let mut g = game(vec![native("Prismabasher"), effect, permanent("White", "{W}"), permanent("Blue", "{U}"), bear]);
    g.put(0, 2, Zone::Battlefield); g.put(0, 3, Zone::Battlefield);
    let bears: Vec<_> = (0..3).map(|_| g.put(0, 4, Zone::Battlefield)).collect();
    let prism = g.put(0, 0, Zone::Graveyard); let card = g.put(0, 1, Zone::Hand);
    g.act(0, Move::Cast { card, targets: vec![vec![Target::Object(prism)]] }).unwrap(); pass(&mut g);
    assert!(matches!(g.decision, Some(game::state::Decision::Targets { player: 0, .. })), "{:?}", g.decision);
    // 603.2: triggering does nothing; 603.3d imports target selection under 601.2c.
    // Ruling 5b6c99f1343d1acc says "when ... triggers"; interpreted with 603.3d/601.2c,
    // this means target announcement, not a saved value during the triggering event.
    // No card text creates an exception. Only green remains at stack placement.
    let before = format!("{g:?}");
    cite!(["603.3d","601.2c"]; refused("wrong number of targets: 3 chosen, it takes 0 to 1"): g.act(0, Move::ChooseTargets(vec![bears.iter().copied().map(Target::Object).collect()])));
    assert_eq!(format!("{g:?}"), before);
    g.act(0, Move::ChooseTargets(vec![vec![Target::Object(bears[0])]])).unwrap();
    settle(&mut g);
    cite!(["603.2", "603.3d", "601.2c", "608.2h"]; assert_eq!(g.power(bears[0]), Some(3)));
    cite!(["603.2", "603.3d", "601.2c", "608.2h"]; assert_eq!(g.power(bears[1]), Some(2)));
    cite!(["603.2", "603.3d", "601.2c", "608.2h"]; assert_eq!(g.power(bears[2]), Some(2)));
}

#[test]
fn r603_3d_color_bounds_are_admitted_for_ordinary_and_triggered_placement() {
    game::play::check(&native("Prismabasher")).unwrap();
    let body = "Up to X target creatures you control get +X/+X until end of turn, where X is the number of colors among permanents you control.";
    for text in [format!("When you next cast a spell this turn, {}", body.replacen("Up", "up", 1)),
                 format!("You may pay {{1}}. When you do, {}", body.replacen("Up", "up", 1))] {
        let card = CardDef::new("Trigger Bounds", "{0}", "Instant", &text, None).unwrap_or_else(|e| panic!("{text}: {e}"));
        game::play::check(&card).unwrap();
    }
    // Spell and activated bounds belong to ordinary announcement, not trigger time.
    for (kind, text) in [("Instant", body.to_string()), ("Artifact", format!("{{0}}: {body}"))] {
        let card = CardDef::new("Announcement Bounds", "{0}", kind, &text, None).unwrap();
        game::play::check(&card).unwrap();
    }
}

#[test]
fn r601_2c_placement_fixes_target_count_but_not_the_resolution_bonus() {
    let effect = CardDef::new("Return Prism", "{0}", "Instant", "Return target creature card from your graveyard to the battlefield.", None).unwrap();
    let bear = CardDef::new("Model Bear", "{0}", "Creature — Bear", "", Some((2, 2))).unwrap();
    let mut g = game(vec![native("Prismabasher"), effect, permanent("White", "{W}"), permanent("Blue", "{U}"), bear]);
    let white = g.put(0, 2, Zone::Battlefield); let blue = g.put(0, 3, Zone::Battlefield);
    let bears: Vec<_> = (0..3).map(|_| g.put(0, 4, Zone::Battlefield)).collect();
    let prism = g.put(0, 0, Zone::Graveyard); let card = g.put(0, 1, Zone::Hand);
    g.act(0, Move::Cast { card, targets: vec![vec![Target::Object(prism)]] }).unwrap(); pass(&mut g);
    g.act(0, Move::ChooseTargets(vec![bears.iter().copied().map(Target::Object).collect()])).unwrap();
    g.move_object(white, Zone::Graveyard); g.move_object(blue, Zone::Graveyard);
    settle(&mut g);
    // Choosing three is retained; the live resolution quantity is now only green.
    for bear in bears {
        cite!(["601.2c", "603.3d", "608.2h"]; assert_eq!(g.power(bear), Some(3)));
    }
}

#[test]
fn r603_7_delayed_color_bound_does_not_capture_registration_state() {
    let registration = CardDef::new("Later Union", "{0}", "Instant", "When you next cast a spell this turn, up to X target creatures you control get +X/+X until end of turn, where X is the number of colors among permanents you control.", None).unwrap();
    let nothing = CardDef::new("Nothing", "{0}", "Instant", "You gain 0 life.", None).unwrap();
    let bear = CardDef::new("Model Bear", "{0}", "Creature — Bear", "", Some((2, 2))).unwrap();
    let mut g = game(vec![registration, nothing, permanent("White", "{W}"), permanent("Blue", "{U}"), bear]);
    cast(&mut g, 0, ""); // No colors when the delayed subscription is created.
    let white = g.put(0, 2, Zone::Battlefield); let blue = g.put(0, 3, Zone::Battlefield);
    let bears: Vec<_> = (0..2).map(|_| g.put(0, 4, Zone::Battlefield)).collect();
    let card = g.put(0, 1, Zone::Hand);
    g.act(0, Move::Cast { card, targets: vec![] }).unwrap();
    g.act(0, Move::ChooseTargets(vec![bears.iter().copied().map(Target::Object).collect()])).unwrap();
    g.move_object(white, Zone::Graveyard); g.move_object(blue, Zone::Graveyard);
    settle(&mut g);
    for bear in bears {
        cite!(["603.7a", "603.3d", "601.2c", "608.2h"]; assert_eq!(g.power(bear), Some(2)));
    }
}

#[test]
fn r107_3m_native_entry_target_bound_retains_casting_x() {
    let bear = CardDef::new("Model Bear", "{0}", "Creature — Bear", "", Some((2, 2))).unwrap();
    let mut g = game(vec![native("Lost in the Maze"), bear]);
    let bears: Vec<_> = (0..3).map(|_| g.put(1, 1, Zone::Battlefield)).collect();
    g.players[0].pool.colorless = 3; g.players[0].pool.colors[ir::Color::Blue as usize] = 2;
    let card = g.put(0, 0, Zone::Hand);
    g.act(0, paid("CCCUU", Move::WithX(3, Box::new(Move::Cast { card, targets: vec![] })))).unwrap(); pass(&mut g);
    // The new permanent's mana-cost X is zero, but its ETB uses the spell's chosen X.
    let before = format!("{g:?}");
    cite!(["107.3m","603.3d"]; refused("wrong number of targets: 1 chosen, it takes 3"): g.act(0, Move::ChooseTargets(vec![vec![Target::Object(bears[0])]])));
    assert_eq!(format!("{g:?}"), before);
    g.act(0, Move::ChooseTargets(vec![bears.iter().copied().map(Target::Object).collect()])).unwrap(); settle(&mut g);
    for bear in bears {
        cite!(["107.3g", "107.3m", "603.3d"]; assert!(g.obj(bear).tapped));
        cite!(["107.3g", "107.3m", "603.3d"]; assert_eq!(g.counters(bear).get("stun"), Some(&1)));
    }
}

#[test]
fn r603_12_reflexive_color_bound_reads_state_after_its_producing_action() {
    let effect = CardDef::new("Reflexive Union", "{0}", "Instant", "Exile all artifacts. When you do, up to X target creatures you control get +X/+X until end of turn, where X is the number of colors among permanents you control.", None).unwrap();
    let bear = CardDef::new("Green Bear", "{G}", "Creature — Bear", "", Some((2, 2))).unwrap();
    let mut g = game(vec![effect, permanent("White", "{W}"), permanent("Blue", "{U}"), bear]);
    g.put(0, 1, Zone::Battlefield); g.put(0, 2, Zone::Battlefield);
    let bears: Vec<_> = (0..3).map(|_| g.put(0, 3, Zone::Battlefield)).collect();
    let card = g.put(0, 0, Zone::Hand);
    g.act(0, Move::Cast { card, targets: vec![] }).unwrap(); pass(&mut g);
    let before = format!("{g:?}");
    cite!(["603.12","603.3d","601.2c"]; refused("wrong number of targets: 3 chosen, it takes 0 to 1"): g.act(0, Move::ChooseTargets(vec![bears.iter().copied().map(Target::Object).collect()])));
    assert_eq!(format!("{g:?}"), before);
    g.act(0, Move::ChooseTargets(vec![vec![Target::Object(bears[0])]])).unwrap(); settle(&mut g);
    cite!(["603.12", "603.3d", "601.2c", "608.2h"]; assert_eq!(g.power(bears[0]), Some(3)));
    cite!(["603.12", "603.3d", "601.2c", "608.2h"]; assert_eq!(g.power(bears[1]), Some(2)));
    cite!(["603.12", "603.3d", "601.2c", "608.2h"]; assert_eq!(g.power(bears[2]), Some(2)));
}
