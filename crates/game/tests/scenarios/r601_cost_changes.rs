//! Increases then reductions of the announced total cost (601.2f, 118.7).
use crate::support::cite;
use game::{CardDef, Game, Move, ObjId, Zone};

fn card(name: &str, cost: &str, kind: &str, text: &str) -> CardDef {
    let d = CardDef::new(name, cost, kind, text, None).unwrap();
    game::play::check(&d).unwrap_or_else(|e| panic!("{name}: {}", e.0)); d
}
fn game(cost: &str, modifiers: &[&str]) -> (Game, ObjId) {
    let mut defs = vec![card("Spell", cost, "Instant", "You gain 1 life.")];
    defs.extend(modifiers.iter().enumerate().map(|(i,t)|card(&format!("Modifier {i}"),"{0}","Enchantment",t)));
    let mut g = Game::new(defs, 2, 20);
    for i in 1..=modifiers.len() { g.put(0, i, Zone::Battlefield); }
    let spell = g.put(0, 0, Zone::Hand);
    (g, spell)
}

fn cast(g: &mut Game, spell: ObjId, spending: Option<&str>) -> Result<(), String> {
    let action = Move::Cast { card: spell, targets: vec![] };
    g.act(0, spending.map_or_else(|| action.clone(), |units|crate::support::paid(units,action.clone())))
}

#[test]
fn r601_2f_increases_precede_reductions_regardless_of_timestamp() {
    for mods in [vec!["Spells cost {2} less to cast.", "Spells cost {1} more to cast."], vec!["Spells cost {1} more to cast.", "Spells cost {2} less to cast."]] {
        let (mut g, spell) = game("{0}", &mods);
        assert!(g.clone().act(g.priority, Move::Cast { card: spell, targets: vec![] }).is_ok());
        // {0}: +{1} then -{2}, whatever their order, leaves {0}; -{2} first would leave {1}
        cite!(["601.2f"]; ok: cast(&mut g, spell, None));
        cite!(["202.3"]; assert_eq!(g.obj(spell).chars.mana_value, 0, "printed mana value is not discounted"));
    }
}

#[test]
fn r118_7a_generic_reductions_preserve_colored_and_colorless_requirements() {
    let (mut g, spell) = game("{2}{G}{C}", &["Spells cost {9} less to cast."]);
    g.players[0].pool.colors[ir::Color::Green as usize] = 2;
    assert!(!g.clone().act(g.priority, crate::support::paid("CG",Move::Cast { card: spell, targets: vec![] })).is_ok());
    cite!(["118.7a"]; refused("your mana pool cannot pay that cost (118.3a): {G}{C} in total, {2} less than it was (601.2f)"): cast(&mut g, spell, None));
    g.players[0].pool.colorless = 1;
    assert!(g.clone().act(g.priority, crate::support::paid("CG",Move::Cast { card: spell, targets: vec![] })).is_ok());
    cast(&mut g, spell, Some("CG")).unwrap();
    // {G}{C} paid, one green left
    cite!(["118.7a"]; assert_eq!(g.players[0].pool.total(), 1));
}

#[test]
fn r601_2f_electromancer_filters_spell_kind_and_controller() {
    let d = CardDef::new("Goblin Electromancer", "{U}{R}", "Creature — Goblin Wizard", "Instant and sorcery spells you cast cost {1} less to cast.", Some((2,2))).unwrap();
    game::play::check(&d).unwrap();
    let mut g = Game::new(vec![d, card("Spell", "{1}", "Instant", "You gain 1 life."),
        CardDef::new("Creature", "{1}", "Creature", "", Some((1, 1))).unwrap()], 2, 20);
    g.put(1, 0, Zone::Battlefield);
    let spell = g.put(0, 1, Zone::Hand);
    assert!(!g.clone().act(g.priority, Move::Cast { card: spell, targets: vec![] }).is_ok());
    g.put(0, 0, Zone::Battlefield);
    let creature = g.put(0, 2, Zone::Hand);
    assert!(!g.clone().act(g.priority, Move::Cast { card: creature, targets: vec![] }).is_ok());
    // a creature receives no discount: the whole reason, as a substring would also match one
    // that names a change
    cite!(["601.2f"]; assert_eq!(cast(&mut g, creature, None), Err("your mana pool cannot pay that cost (118.3a): {1}".into())));
    assert!(g.clone().act(g.priority, Move::Cast { card: spell, targets: vec![] }).is_ok());
    cite!(["601.2f"]; ok: cast(&mut g, spell, None));
}

#[test]
fn r601_2f_the_same_query_controls_listing_and_mana_assignment() {
    let (mut g, spell) = game("{1}", &["Noncreature spells cost {1} more to cast."]);
    g.players[0].pool.colorless = 1;
    assert!(!g.clone().act(g.priority, crate::support::paid("CC",Move::Cast { card: spell, targets: vec![] })).is_ok());
    cite!(["601.2f"]; refused("your mana pool cannot pay that cost (118.3a): {2} in total, {1} more than it was (601.2f)"): cast(&mut g, spell, None));
    g.players[0].pool.colorless = 2;
    assert!(g.clone().act(g.priority, crate::support::paid("CC",Move::Cast { card: spell, targets: vec![] })).is_ok());
    cast(&mut g, spell, Some("CC")).unwrap();
    cite!(["601.2f"]; assert_eq!(g.players[0].pool.total(), 0));
}

#[test]
fn r118_7a_generic_discount_applies_to_the_chosen_two_hybrid_alternative() {
    let (mut g, spell) = game("{2/W}{2/W}", &["Spells cost {3} less to cast."]);
    g.players[0].pool.colors[ir::Color::Green as usize] = 1;
    assert!(g.clone().act(g.priority, crate::support::paid("G",Move::Cast { card: spell, targets: vec![] })).is_ok());
    cast(&mut g, spell, Some("G")).unwrap();
    // {2}{2} less {3}: one green pays the {1} left
    cite!(["118.7a", "107.4e"]; assert_eq!(g.players[0].pool.total(), 0));
    // The payer can instead choose both colored halves; the reduction then removes no
    // colored mana. Their exact proposed pool selects that equivalent (601.2b).
    let (mut g, spell) = game("{2/W}{2/W}", &["Spells cost {3} less to cast."]);
    g.players[0].pool.colors[ir::Color::White as usize] = 2;
    let with = g.players[0].pool.clone();
    cite!(["118.7a", "601.2b"]; ok: g.act(0, Move::Paying(with, Box::new(Move::Cast { card: spell, targets: vec![] }))));
    assert_eq!(g.players[0].pool.total(), 0);
}

#[test]
fn r601_2f_discount_is_locked_before_its_source_is_sacrificed_for_mana() {
    let source = card("Resource", "{0}", "Artifact", "Spells you cast cost {1} less to cast.\nSacrifice this artifact: Add {G}.");
    let spell = card("Spell", "{1}{G}", "Instant", "You gain 1 life.");
    let mut g = Game::new(vec![source, spell], 2, 20);
    let source = g.put(0, 0, Zone::Battlefield);
    let spell = g.put(0, 1, Zone::Hand);
    let plan = vec![game::legal::Step { plan: vec![], x: None, life: vec![], with: None, source, ability: 1, paying: vec![], adding: vec![], chose: vec![], cost_branches: None }];
    // the total, {G}, is determined (601.2f) before the source is sacrificed for its mana (601.2g)
    cite!(["601.2f", "601.2g"]; ok: g.act(0, Move::WithMana(plan, Box::new(crate::support::paid("G",Move::Cast { card: spell, targets: vec![] })))));
    assert!(!g.battlefield.contains(&source));
    assert_eq!(g.players[0].pool.total(), 0);
    assert_eq!(g.stack.len(), 1);
}

#[test]
fn r113_6d_self_reduction_is_static_and_counts_after_leaving_the_hand() {
    let d = card("Spell", "{3}", "Instant", "This spell costs {1} less to cast for each card in your hand.\nYou gain 1 life.");
    let mut g = Game::new(vec![d], 2, 20);
    let spell = g.put(0, 0, Zone::Hand);
    g.put(0, 0, Zone::Hand);
    g.players[0].pool.colorless = 1;
    cite!(["601.2f"]; assert!(!g.clone().act(g.priority, crate::support::paid("CC",Move::Cast { card: spell, targets: vec![] })).is_ok(), "only the other card remains in hand during 601.2f"));
    g.players[0].pool.colorless = 2;
    assert!(g.clone().act(g.priority, crate::support::paid("CC",Move::Cast { card: spell, targets: vec![] })).is_ok());
    cast(&mut g, spell, Some("CC")).unwrap();
    for _ in 0..2 { g.act(g.priority, Move::Pass).unwrap(); }
    assert_eq!(g.players[0].life, 21);
}

#[test]
fn r601_2f_reduction_applies_to_additional_costs_and_announced_x() {
    let reducer = card("Reducer", "{0}", "Artifact", "Spells you cast cost {2} less to cast.");
    let d = card("Spell", "{X}{G}", "Instant", "As an additional cost to cast this spell, pay {1}.\nYou gain X life.");
    let mut g = Game::new(vec![reducer, d], 2, 20);
    g.put(0, 0, Zone::Battlefield);
    let spell = g.put(0, 1, Zone::Hand);
    g.players[0].pool.colors[ir::Color::Green as usize] = 1;
    g.players[0].pool.colorless = 1;
    // {2}{G} and the additional {1}, less {2}: {1}{G}
    cite!(["601.2f"]; ok: g.act(0, Move::WithX(2, Box::new(crate::support::paid("CG",Move::Cast { card: spell, targets: vec![] })))));
    assert_eq!(g.players[0].pool.total(), 0);
    for _ in 0..2 { g.act(g.priority, Move::Pass).unwrap(); }
    assert_eq!(g.players[0].life, 22);
}

#[test]
fn r613_1f_removed_abilities_stop_providing_cost_reductions() {
    let reducer = CardDef::new("Reducer", "{0}", "Creature — Wizard", "Spells you cast cost {1} less to cast.\n{0}: This creature loses all abilities until end of turn.", Some((2,2))).unwrap();
    game::play::check(&reducer).unwrap();
    let spell = card("Spell", "{1}", "Instant", "You gain 1 life.");
    let mut g = Game::new(vec![reducer, spell], 2, 20);
    let source = g.put(0, 0, Zone::Battlefield);
    let spell = g.put(0, 1, Zone::Hand);
    assert!(g.clone().act(g.priority, Move::Cast { card: spell, targets: vec![] }).is_ok());
    g.act(0, Move::Activate { source, ability: 1, targets: vec![], paying: vec![] }).unwrap();
    for _ in 0..2 { g.act(g.priority, Move::Pass).unwrap(); }
    assert!(!g.clone().act(g.priority, Move::Cast { card: spell, targets: vec![] }).is_ok());
    // the whole reason: no reduction is named
    cite!(["613.1f", "601.2f"]; assert_eq!(cast(&mut g, spell, None), Err("your mana pool cannot pay that cost (118.3a): {1}".into())));
}

#[test]
fn r601_2f_heartless_summoning_reduces_the_spell_and_weakens_the_permanent() {
    let source = card("Heartless Summoning", "{1}{B}", "Enchantment", "Creature spells you cast cost {2} less to cast.\nCreatures you control get -1/-1.");
    let creature = CardDef::new("Creature", "{2}{G}", "Creature — Bear", "", Some((2,2))).unwrap();
    let mut g = Game::new(vec![source, creature], 2, 20);
    g.put(0, 0, Zone::Battlefield);
    let spell = g.put(0, 1, Zone::Hand);
    g.players[0].pool.colors[ir::Color::Green as usize] = 1;
    // Put Game into an ordinary main phase without advancing its empty libraries.
    g.part = ir::Part::PrecombatMain;
    assert!(g.clone().act(g.priority, crate::support::paid("G",Move::Cast { card: spell, targets: vec![] })).is_ok());
    cite!(["601.2f"]; ok: cast(&mut g, spell, Some("G")));
    for _ in 0..2 { g.act(g.priority, Move::Pass).unwrap(); }
    let creature = *g.battlefield.last().unwrap();
    assert_eq!(g.seen(creature, None).pt, Some((1,1)));
}

#[test]
fn r601_2f_target_dependent_costs_require_target_aware_offers() {
    let d = CardDef::new("Hinata, Dawn-Crowned", "{1}{U}{R}{W}", "Legendary Creature — Kirin Spirit", "Flying, trample\nSpells you cast cost {1} less to cast for each target.\nSpells your opponents cast cost {1} more to cast for each target.", Some((4,4))).unwrap();
    assert!(game::play::check(&d).unwrap_err().0.contains("counting announced targets"));
}

#[test]
fn r107_3c_a_defined_x_discount_reads_the_greatest_value_not_an_announced_x() {
    let d=CardDef::new("Sunderflock","{7}{U}{U}","Creature — Elemental","This spell costs {X} less to cast, where X is the greatest mana value among Elementals you control.\nFlying\nWhen this creature enters, if you cast it, return all non-Elemental creatures to their owners' hands.",Some((5,5))).unwrap();
    game::play::check(&d).unwrap();
    let elemental=CardDef::new("Elemental","{5}","Creature — Elemental","",Some((3,3))).unwrap();
    let bear=CardDef::new("Bear","{0}","Creature — Bear","",Some((2,2))).unwrap();
    let small=CardDef::new("Small elemental","{3}","Creature — Elemental","",Some((1,1))).unwrap();
    let mut g=Game::new(vec![d,elemental,bear,small],2,20);
    g.put(0,3,Zone::Battlefield); // first=3, greatest=5, sum=8
    let elemental=g.put(0,1,Zone::Battlefield);let bear=g.put(1,2,Zone::Battlefield);
    let card=g.put(0,0,Zone::Hand);g.players[0].pool.colors[ir::Color::Blue as usize]=2;
    assert!(!g.clone().act(g.priority, crate::support::paid("CCUU",Move::Cast { card: card, targets: vec![] })).is_ok());g.players[0].pool.colorless=2;assert!(g.clone().act(g.priority, crate::support::paid("CCUU",Move::Cast { card: card, targets: vec![] })).is_ok());
    // X is the greatest mana value, 5, not the first or the sum: {2}{U}{U} (107.3c)
    cast(&mut g, card, Some("CCUU")).unwrap();cite!(["107.3c", "601.2f"]; assert_eq!(g.players[0].pool.total(),0));
    for _ in 0..4 {g.act(g.priority,Move::Pass).unwrap();}
    assert!(g.battlefield.contains(&elemental));assert!(!g.battlefield.contains(&bear));
    assert_eq!(g.players[1].hand.len(),1);
}

#[test]
fn r118_7a_defined_discounts_handle_empty_sets_and_do_not_reduce_colored_mana() {
    let d=card("Discount","{2}{U}","Instant","This spell costs {X} less to cast, where X is the number of creatures you control.\nYou gain 1 life.");
    let bear=CardDef::new("Bear","{0}","Creature — Bear","",Some((2,2))).unwrap();
    let mut g=Game::new(vec![d,bear],2,20);let spell=g.put(0,0,Zone::Hand);
    g.players[0].pool.colors[ir::Color::Blue as usize]=1;assert!(!g.clone().act(g.priority, crate::support::paid("U",Move::Cast { card: spell, targets: vec![] })).is_ok());
    for _ in 0..5 {g.put(0,1,Zone::Battlefield);}
    assert!(g.clone().act(g.priority, crate::support::paid("U",Move::Cast { card: spell, targets: vec![] })).is_ok());g.players[0].pool.colors[ir::Color::Blue as usize]=0;
    assert!(!g.clone().act(g.priority, crate::support::paid("U",Move::Cast { card: spell, targets: vec![] })).is_ok());cite!(["118.7a"]; refused("your mana pool cannot pay that cost (118.3a): {U} in total, {2} less than it was (601.2f)"): cast(&mut g, spell, None));
    g.players[0].pool.colors[ir::Color::Blue as usize]=1;cite!(["118.7a"]; ok: cast(&mut g, spell, Some("U")));assert_eq!(g.players[0].pool.total(),0);
}

#[test]
fn r601_2f_an_unpayable_total_is_named_with_what_changed_it() {
    let (mut g, spell) = game("{1}{R}", &["Noncreature spells cost {1} more to cast."]);
    g.players[0].pool.colors[ir::Color::Red as usize] = 2;
    cite!(["118.3a", "601.2f"]; assert_eq!(cast(&mut g, spell, None), Err("your mana pool cannot pay that cost (118.3a): {2}{R} in total, {1} more than it was (601.2f)".into())));
    let (mut g, spell) = game("{3}{R}", &["Noncreature spells cost {1} more to cast.", "Spells cost {2} less to cast."]);
    g.players[0].pool.colors[ir::Color::Red as usize] = 2;
    cite!(["118.3a", "601.2f"]; assert_eq!(cast(&mut g, spell, None), Err("your mana pool cannot pay that cost (118.3a): {2}{R} in total, {1} more and {2} less than it was (601.2f)".into())));
    let (mut g, spell) = game("{1}{R}", &[]);
    cite!(["118.3a"]; assert_eq!(cast(&mut g, spell, None), Err("your mana pool cannot pay that cost (118.3a): {1}{R}".into())));
}
