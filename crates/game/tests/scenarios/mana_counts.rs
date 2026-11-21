//! Mana quantities are determined when the effect is applied, after costs (106.3, 608.2h).
use crate::support::{cite, paid};
use game::legal::Step;
use game::{CardDef, Game, Move, ObjId, Zone};
use ir::Color;

fn game(text: &str, types: &str) -> Game {
    let defs = vec![
        CardDef::new("Source", "{0}", types, text, types.contains("Creature").then_some((2, 2))).unwrap(),
        CardDef::new("Elf", "{0}", "Creature — Elf", "", Some((1, 1))).unwrap(),
        CardDef::new("Forest", "", "Basic Land — Forest", "", None).unwrap(),
        CardDef::new("Reward", "{G}", "Sorcery", "You gain 1 life.", None).unwrap(),
    ];
    for d in &defs { game::play::check(d).unwrap_or_else(|u| panic!("{}: {}", d.name(), u.0)); }
    Game::new(defs, 3, 20)
}

fn green(g: &Game) -> u32 { g.players[0].pool.colors[Color::Green as usize] }

fn activate(g: &mut Game, source: ObjId) {
    g.act(0, Move::Activate { source, ability: 0, targets: vec![], paying: vec![] }).unwrap();
}

fn step(source: ObjId, ability: usize) -> Step { Step { plan: vec![], x: None, life: vec![], with: None, source, ability, paying: vec![], adding: vec![], chose: vec![], cost_branches: None } }

fn follow(g: &mut Game, steps: Vec<Step>) {
    for s in steps {
        assert!(s.adding.is_empty());
        g.act(0, Move::Activate { source: s.source, ability: s.ability, targets: vec![], paying: s.paying }).unwrap();
    }
}

#[test]
fn r106_3_fixed_mana_is_multiplied_by_the_current_matching_objects() {
    let mut g = game("{T}: Add {G} for each Elf you control.", "Creature — Elf Druid");
    let source = g.put(0, 0, Zone::Battlefield);
    g.put(0, 1, Zone::Battlefield);
    g.put(1, 1, Zone::Battlefield);
    g.put(2, 1, Zone::Battlefield);
    g.put(0, 1, Zone::Graveyard);
    g.put(0, 1, Zone::Hand);
    activate(&mut g, source);
    cite!(["608.2h"]; assert_eq!(green(&g), 2));
    cite!(["605.3b"]; assert!(g.stack.is_empty(), "a mana ability resolves immediately"));
    assert_eq!(g.priority, 0);
}

#[test]
fn r608_2h_mana_counts_after_tapping_or_sacrificing_its_source_to_pay_costs() {
    for (text, expected) in [
        ("{T}: Add {G} for each tapped land you control.", 1),
        ("{T}: Add {G} for each untapped land you control.", 0),
        ("Sacrifice this land: Add {G} for each land you control.", 0),
    ] {
        let mut g = game(text, "Land");
        let source = g.put(0, 0, Zone::Battlefield);
        activate(&mut g, source);
        cite!(["608.2h"]; case: text; assert_eq!(green(&g), expected, "{text}"));
    }
}

#[test]
fn r601_2g_a_mana_plan_can_use_a_quantity_that_increases_when_its_cost_is_paid() {
    let mut g = game("{T}: Add {G} for each tapped land you control.", "Land");
    let source = g.put(0, 0, Zone::Battlefield);
    let card = g.put(0, 3, Zone::Hand);
    let steps = vec![step(source, 0)];
    assert_eq!(steps.len(), 1);
    cite!(["601.2g"]; assert_eq!(steps[0].source, source));
    follow(&mut g, steps);
    cite!(["608.2h"]; assert_eq!(green(&g), 1));
    g.act(0, paid("G", Move::Cast { card, targets: vec![] })).unwrap();
}

#[test]
fn r601_2g_a_mana_plan_does_not_spend_a_quantity_removed_by_its_cost() {
    let mut g = game("Sacrifice this land: Add {G} for each land you control.", "Land");
    let source = g.put(0, 0, Zone::Battlefield);
    let card = g.put(0, 3, Zone::Hand);
    let before = format!("{g:?}");
    cite!(["601.2g", "608.2h", "118.3a"]; refused("that mana is not in your mana pool"): g.act(0, paid("G", Move::WithMana(vec![step(source, 0)], Box::new(Move::Cast { card, targets: vec![] })))));
    assert_eq!(format!("{g:?}"), before);
    assert_eq!(g.obj(source).zone, Zone::Battlefield, "planning does not pay costs");
    assert_eq!(green(&g), 0);
}

#[test]
fn r608_2h_conditional_production_sees_its_own_tap_cost() {
    let mut g = game("{T}: Add {C}. If this land is tapped, instead add {G}.", "Land");
    let source = g.put(0, 0, Zone::Battlefield);
    let card = g.put(0, 3, Zone::Hand);
    // 605.3b: the source has already paid {T} when its self-replacement is applied.
    let steps = vec![step(source, 0)];
    follow(&mut g, steps);
    cite!(["605.3b", "608.2h", "614.15"]; assert_eq!((green(&g), g.players[0].pool.colorless), (1, 0)));
    assert!(g.obj(source).tapped);
    g.act(0, paid("G", Move::Cast { card, targets: vec![] })).unwrap();
}

#[test]
fn r608_2h_conditional_production_sees_the_counter_removed_for_its_cost() {
    let mut g = game("{T}, Remove a luck counter from this land: Add {C}. If this land has a luck counter on it, instead add {G}.", "Land");
    let source = g.put(0, 0, Zone::Battlefield);
    g.set_initial_counters(source, "luck", 1);
    let card = g.put(0, 3, Zone::Hand);
    let before = format!("{g:?}");
    cite!(["608.2h", "614.15", "118.3a"]; refused("that mana is not in your mana pool"): g.act(0, paid("G", Move::WithMana(vec![step(source, 0)], Box::new(Move::Cast { card, targets: vec![] })))));
    assert_eq!(format!("{g:?}"), before);
    activate(&mut g, source);
    cite!(["608.2h", "614.15"]; assert_eq!((green(&g), g.players[0].pool.colorless), (0, 1)));
}

#[test]
fn r611_3a_conditional_production_sees_power_changed_indirectly_by_its_tap() {
    let mut g = game("As long as this creature is tapped, this creature gets +1/+1.\n{T}: Add {C}. If this creature has power 3 or greater, instead add {G}.", "Creature — Elf");
    let source = g.put(0, 0, Zone::Battlefield);
    g.put(0, 3, Zone::Hand);
    g.turn = 2;
    assert_eq!(g.power(source), Some(2));
    let steps = vec![step(source, 1)];
    follow(&mut g, steps);
    cite!(["611.3a", "608.2h", "614.15"]; assert_eq!((g.power(source), green(&g), g.players[0].pool.colorless), (Some(3), 1, 0)));
}

#[test]
fn r106_3_a_spell_adds_all_symbols_for_each_counted_object_at_resolution() {
    let mut g = game("Add {G}{G} for each creature you control.", "Instant");
    let card = g.put(0, 0, Zone::Hand);
    g.act(0, Move::Cast { card, targets: vec![] }).unwrap();
    for _ in 0..2 { g.put(0, 1, Zone::Battlefield); }
    for _ in 0..3 { g.act(g.priority, Move::Pass).unwrap(); }
    cite!(["608.2h"]; assert_eq!(green(&g), 4, "the count is from resolution, not casting"));
}

#[test]
fn r106_3_unit_any_color_scales_and_activation_asks_the_color_of_each() {
    let d = CardDef::new("Choice", "", "Land", "{T}: Add one mana of any color for each land you control.", None).unwrap();
    game::play::check(&d).unwrap();
    let mut g=Game::new(vec![d],2,20);
    let source=g.put(0,0,Zone::Battlefield);
    for _ in 0..3 { g.put(0,0,Zone::Battlefield); }
    let before = format!("{g:?}");
    let activate = Move::Activate{source,ability:0,targets:vec![],paying:vec![]};
    cite!(["608.2d"]; refused("that ability offers a choice of mana: say which mana it adds (608.2d)"): g.act(0,activate.clone()));
    assert_eq!(format!("{g:?}"), before);
    let one = |c: Color| vec![ir::mana::ManaSymbol::Color(c)];
    g.act(0, Move::Adding(vec![one(Color::Red), one(Color::Green), one(Color::Green), one(Color::White)], Box::new(activate))).unwrap();
    assert_eq!(g.players[0].pool.colors, [1, 0, 0, 1, 2]);
    // Uncited (grammar): only players can add mana; this subject is not read.
    assert_eq!(CardDef::new("Nonsense", "{0}", "Instant", "Target creature adds {G} for each land you control.", None).expect_err("unsupported mana-producing subject"), "text the grammar does not read");
}

#[test]
fn r608_2h_cabal_stronghold_counts_only_its_controllers_basic_swamps_after_paying() {
    let defs = vec![
        CardDef::new("Cabal Stronghold", "", "Land", "{T}: Add {C}.\n{3}, {T}: Add {B} for each basic Swamp you control.", None).unwrap(),
        CardDef::new("Swamp", "", "Basic Land — Swamp", "", None).unwrap(),
        CardDef::new("Nonbasic", "", "Land — Swamp", "", None).unwrap(),
    ];
    for count in [0, 2, 4] {
        let mut g = Game::new(defs.clone(), 2, 20);
        let source = g.put(0, 0, Zone::Battlefield);
        for _ in 0..count { g.put(0, 1, Zone::Battlefield); }
        g.put(0, 2, Zone::Battlefield);
        g.put(1, 1, Zone::Battlefield);
        g.players[0].pool.colorless = 3;
        g.act(0, paid("CCC", Move::Activate { source, ability: 1, targets: vec![], paying: vec![] })).unwrap();
        cite!(["608.2h"]; case: count; assert_eq!(g.players[0].pool.colors[Color::Black as usize], count));
        assert_eq!(g.players[0].pool.colorless, 0);
        assert!(g.obj(source).tapped && g.stack.is_empty());
    }
}

#[test]
fn r106_3_elvish_archdruid_composes_counted_mana_with_its_other_elves_effect() {
    let archdruid = CardDef::new("Elvish Archdruid", "{1}{G}{G}", "Creature — Elf Druid", "Other Elf creatures you control get +1/+1.\n{T}: Add {G} for each Elf you control.", Some((2, 2))).unwrap();
    game::play::check(&archdruid).unwrap();
    let mut g = game("{T}: Add {G} for each Elf you control.", "Creature — Elf Druid");
    std::sync::Arc::make_mut(&mut g.defs)[0] = archdruid;
    let source = g.put(0, 0, Zone::Battlefield);
    let elf = g.put(0, 1, Zone::Battlefield);
    let theirs = g.put(1, 1, Zone::Battlefield);
    assert_eq!((g.power(source), g.power(elf), g.power(theirs)), (Some(2), Some(2), Some(1)));
    g.act(0, Move::Activate { source, ability: 1, targets: vec![], paying: vec![] }).unwrap();
    cite!(["608.2h"]; assert_eq!(green(&g), 2));
}
