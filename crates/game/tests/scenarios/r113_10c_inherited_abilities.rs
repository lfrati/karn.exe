//! Dynamic battlefield inheritance preserves exact acquired activated instances.
use crate::support::{cite, paid};
use game::{CardDef, Game, Move, ObjId, Zone};
use std::sync::{Arc, OnceLock};

fn definitions() -> Vec<CardDef> {
    static DEFS: OnceLock<Vec<CardDef>> = OnceLock::new();
    DEFS.get_or_init(|| {
        let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
        let catalog = CardDef::catalog(&faces);
        let mut defs: Vec<_> = ["Marvin, Murderous Mimic", "Draconautics Engineer", "Afterburner Expert", "Loot, the Pathfinder", "Brave Brawler"]
            .iter().map(|name| CardDef::from_faces(catalog.get(name).unwrap()).unwrap_or_else(|e| panic!("{name}: {e}"))).collect();
        defs.push(CardDef::new("Forest", "", "Basic Land — Forest", "", None).unwrap());
        for def in &defs { game::play::check(def).unwrap_or_else(|e| panic!("{}: {e:?}", def.name())); }
        defs
    }).clone()
}
fn game() -> Game {
    let mut g = Game::new(definitions(), 2, 20);
    for p in 0..2 { for _ in 0..8 { g.put(p, 5, Zone::Library); } }
    g
}
fn activate(source: ObjId, ability: usize, spending: &str) -> Move { paid(spending, Move::Activate { source, ability, targets: vec![], paying: vec![] }) }
fn settle(g: &mut Game) {
    for _ in 0..20 {
        if g.stack.is_empty() { return; }
        for _ in 0..2 { g.act(g.priority, Move::Pass).unwrap(); }
    }
    panic!("unsettled stack");
}

// Native Marvin/Engineer. Each donor has two distinct Exhaust abilities; the borrowed
// acquisition is independent of the donor's own use and the other donor's identical row.
#[test]
fn r602_5c_two_identical_donors_are_distinct_from_each_other_and_the_borrower() {
    let mut g = game();
    let marvin = g.put(0, 0, Zone::Battlefield);
    let first = g.put(0, 1, Zone::Battlefield);
    let _second = g.put(0, 1, Zone::Battlefield);
    cite!(["113.10c"]; assert_eq!(g.abilities(marvin).len(), 5)); // own static row, then two per donor
    g.players[0].pool.colors[ir::Color::Red as usize] = 3;
    g.act(0, activate(first, 0, "R")).unwrap(); settle(&mut g);
    cite!(["602.5c", "702.177a"]; ok: g.act(0, activate(marvin, 1, "R")));
    settle(&mut g);
    cite!(["702.177a"]; refused("only once"): g.act(0, activate(marvin, 1, "R")));
    cite!(["602.5c"]; ok: g.act(0, activate(marvin, 3, "R")));
    settle(&mut g);
    g.players[0].pool.colors[ir::Color::Red as usize] = 1;
    cite!(["702.177a"]; refused("only once"): g.act(0, activate(marvin, 3, "R")));
}

// Native Expert's self-referential counter effect must apply to Marvin, not the donor.
#[test]
fn r201_5b_inherited_self_name_and_exhaust_effect_use_the_borrower() {
    let mut g = game();
    let marvin = g.put(0, 0, Zone::Battlefield);
    let expert = g.put(0, 2, Zone::Battlefield);
    g.players[0].pool.colorless = 2;
    g.players[0].pool.colors[ir::Color::Green as usize] = 2;
    g.act(0, activate(marvin, 1, "CCGG")).unwrap(); settle(&mut g);
    cite!(["201.5b"]; assert_eq!(g.obj(marvin).counters.get("+1/+1"), Some(&2)));
    cite!(["201.5b"]; assert!(g.obj(expert).counters.is_empty()));
}

// The inference from 113.10c/611.3/613.7 is that membership suppression does not create a
// new static effect/acquisition. An unrelated definition-bank append is storage only.
#[test]
fn r602_5b_suppression_and_unrelated_definition_addition_do_not_refresh_inherited_use() {
    let mut g = game();
    let marvin = g.put(0, 0, Zone::Battlefield);
    let _donor = g.put(0, 1, Zone::Battlefield);
    g.players[0].pool.colors[ir::Color::Red as usize] = 2;
    g.act(0, activate(marvin, 1, "R")).unwrap(); settle(&mut g);
    let strip = CardDef::new("Suppressor", "{0}", "Enchantment", "All creatures lose all abilities.", None).unwrap();
    game::play::check(&strip).unwrap();
    let index = g.defs.len(); Arc::make_mut(&mut g.defs).push(strip);
    let suppressor = g.put(0, index, Zone::Battlefield);
    cite!(["613.1f"]; assert!(g.abilities(marvin).is_empty()));
    g.move_object(suppressor, Zone::Graveyard);
    cite!(["113.10c", "611.3a", "613.7a"]; assert_eq!(g.abilities(marvin).len(), 3));
    cite!(["602.5b", "702.177a"]; refused("only once"): g.act(0, activate(marvin, 1, "R")));
}

// Removing an earlier donor changes public ordinals, not the later donor's acquisition.
#[test]
fn r602_5b_donor_list_shifts_preserve_use_and_reentry_creates_a_new_instance() {
    let mut g = game();
    let marvin = g.put(0, 0, Zone::Battlefield);
    let first = g.put(0, 1, Zone::Battlefield);
    let second = g.put(0, 1, Zone::Battlefield);
    g.players[0].pool.colors[ir::Color::Red as usize] = 3;
    g.act(0, activate(marvin, 3, "R")).unwrap(); settle(&mut g);
    g.move_object(first, Zone::Graveyard);
    cite!(["602.5b", "702.177a"]; refused("only once"): g.act(0, activate(marvin, 1, "R")));
    g.move_object(second, Zone::Hand);
    let returned = *g.players[0].hand.last().unwrap();
    g.move_object(returned, Zone::Battlefield);
    cite!(["400.7", "602.5c"]; ok: g.act(0, activate(marvin, 1, "R")));
    settle(&mut g);
}

// The captured program has a borrower source, even after the donor disappears.
#[test]
fn r113_7a_retained_inherited_activation_survives_donor_departure() {
    let mut g = game();
    let marvin = g.put(0, 0, Zone::Battlefield);
    let expert = g.put(0, 2, Zone::Battlefield);
    g.players[0].pool.colorless = 2;
    g.players[0].pool.colors[ir::Color::Green as usize] = 2;
    g.act(0, activate(marvin, 1, "CCGG")).unwrap();
    g.move_object(expert, Zone::Graveyard);
    assert_eq!(g.abilities(marvin).len(), 1);
    settle(&mut g);
    cite!(["113.7a", "201.5b"]; assert_eq!(g.obj(marvin).counters.get("+1/+1"), Some(&2)));
}

// Entry history and the mana cost belong to Marvin (2), not the Brawler donor (1W).
#[test]
fn r702_193a_inherited_power_up_uses_borrower_entry_and_current_mana_cost() {
    let mut g = game();
    let in_hand = g.put(0, 0, Zone::Hand);
    g.move_object(in_hand, Zone::Battlefield);
    let marvin = *g.battlefield.last().unwrap();
    g.put(0, 4, Zone::Battlefield);
    g.players[0].pool.colorless = 3;
    let before=format!("{g:?}");
    cite!(["702.193a","118.7a","118.3a"]; refused("the supplied mana does not pay that cost, all of it and no more"): g.act(0, activate(marvin, 1, "CCC")));
    assert_eq!(format!("{g:?}"),before);
    g.players[0].pool.colorless = 2;
    g.players[0].pool.colors[ir::Color::White as usize] = 1;
    cite!(["702.193a"]; ok: g.act(0, activate(marvin, 1, "CCW")));
    settle(&mut g);
    cite!(["201.5b"]; assert_eq!(g.obj(marvin).counters.get("+1/+1"), Some(&2)));
    assert_eq!(g.players[0].pool.colorless, 0);
    assert_eq!(g.players[0].pool.colors[ir::Color::White as usize], 0);
}

// Synthetic layer-six dependency loop: each static applies once in timestamp order.
#[test]
fn r613_8_inheritance_cycles_use_timestamp_order_without_recursive_closure() {
    for reverse in [false, true] {
        let mut g = game();
        let text = |life| format!("This creature has all activated abilities of other creatures you control that don't have the same name as this creature.\n{{0}}: You gain {life} life.");
        let index = g.defs.len();
        let a = CardDef::new("First Mimic", "{2}", "Creature", &text(1), Some((2, 2))).unwrap();
        let b = CardDef::new("Second Mimic", "{2}", "Creature", &text(2), Some((2, 2))).unwrap();
        for def in [&a, &b] { game::play::check(def).unwrap(); }
        Arc::make_mut(&mut g.defs).extend([a, b]);
        let (older, newer) = if reverse {
            (g.put(0, index + 1, Zone::Battlefield), g.put(0, index, Zone::Battlefield))
        } else { (g.put(0, index, Zone::Battlefield), g.put(0, index + 1, Zone::Battlefield)) };
        cite!(["613.8"]; assert_eq!(g.abilities(older).len(), 3));
        cite!(["613.8"]; assert_eq!(g.abilities(newer).len(), 4));
        let life = if reverse { 1 } else { 2 };
        g.act(0, activate(newer, 3, "")).unwrap(); settle(&mut g);
        cite!(["613.8"]; assert_eq!(g.players[0].life, 20 + life));
    }
}

#[test]
fn r201_5b_inherited_mana_activation_taps_borrower_and_uses_its_payment_context() {
    let mut g = game();
    let marvin = g.put(0, 0, Zone::Battlefield);
    let loot = g.put(0, 3, Zone::Battlefield);
    g.players[0].pool.colors[ir::Color::Green as usize] = 1;
    let mv = Move::Adding(vec![vec![ir::mana::ManaSymbol::Color(ir::Color::Red); 3]], Box::new(activate(marvin, 1, "G")));
    cite!(["201.5b", "605.3b"]; ok: g.act(0, mv));
    assert!(g.stack.is_empty());
    cite!(["201.5b"]; assert!(g.obj(marvin).tapped));
    cite!(["201.5b"]; assert!(!g.obj(loot).tapped));
    assert_eq!(g.players[0].pool.colors[ir::Color::Red as usize], 3);
    assert_eq!(g.players[0].pool.colors[ir::Color::Green as usize], 0);
}

#[test]
fn r113_10c_donor_scope_excludes_same_name_and_opponents() {
    let mut g = game();
    let marvin = g.put(0, 0, Zone::Battlefield);
    g.put(0, 0, Zone::Battlefield);
    g.put(1, 1, Zone::Battlefield);
    cite!(["113.10c", "109.5"]; assert_eq!(g.abilities(marvin).len(), 1));
}

fn add(g: &mut Game, def: CardDef) -> usize {
    game::play::check(&def).unwrap();
    let index = g.defs.len();
    Arc::make_mut(&mut g.defs).push(def);
    index
}
fn cast_on(g: &mut Game, player: usize, definition: usize, targets: Vec<Vec<game::Target>>) {
    while g.priority != player { g.act(g.priority, Move::Pass).unwrap(); }
    let card = g.put(player, definition, Zone::Hand);
    g.act(player, Move::Cast { card, targets }).unwrap(); settle(g);
}

// Control changes membership, while the same granting/donor origins remain acquisitions.
#[test]
fn r602_5b_borrower_and_donor_control_changes_preserve_acquired_use() {
    use game::Target;
    let mut g = game();
    let marvin = g.put(0, 0, Zone::Battlefield);
    let engineer = g.put(0, 1, Zone::Battlefield);
    let control = add(&mut g, CardDef::new("Borrow", "{0}", "Instant", "Gain control of target creature.", None).unwrap());
    g.players[0].pool.colors[ir::Color::Red as usize] = 2;
    g.act(0, activate(marvin, 1, "R")).unwrap(); settle(&mut g);
    cast_on(&mut g, 1, control, vec![vec![Target::Object(marvin)]]);
    assert_eq!(g.abilities(marvin).len(), 1); // its donor is still controlled by p0
    cast_on(&mut g, 1, control, vec![vec![Target::Object(engineer)]]);
    assert_eq!(g.abilities(marvin).len(), 3);
    g.players[1].pool.colors[ir::Color::Red as usize] = 1;
    while g.priority != 1 { g.act(g.priority, Move::Pass).unwrap(); }
    cite!(["602.5b", "113.10c", "611.3a"]; refused("only once"): g.act(1, activate(marvin, 1, "R")));
}

// Recopying a donor is a genuinely new donor ability acquisition, unlike reevaluation.
#[test]
fn r602_5c_new_donor_copy_acquisition_supplies_new_inherited_instance() {
    use game::Target;
    let mut g = game();
    let marvin = g.put(0, 0, Zone::Battlefield);
    let donor = g.put(0, 1, Zone::Battlefield);
    let model = g.put(1, 1, Zone::Battlefield);
    let copy = add(&mut g, CardDef::new("Recopy", "{0}", "Instant", "Target creature becomes a copy of target creature.", None).unwrap());
    let acquire = |g: &mut Game| cast_on(g, 0, copy, vec![vec![Target::Object(donor)], vec![Target::Object(model)]]);
    for _ in 0..2 {
        acquire(&mut g);
        g.players[0].pool.colors[ir::Color::Red as usize] = 2;
        cite!(["602.5c", "707.2"]; ok: g.act(0, activate(marvin, 1, "R")));
        settle(&mut g);
        cite!(["702.177a"]; refused("only once"): g.act(0, activate(marvin, 1, "R")));
    }
}

// 607.5 links the two acquired activated rows to each other, using borrower records.
#[test]
fn r607_5_acquired_activated_pair_does_not_import_donor_exile_records() {
    use game::Target;
    let mut g = game();
    let pair = add(&mut g, CardDef::new("Archive Bear", "{0}", "Creature", "{0}: Exile target creature card from your graveyard.\n{0}: Return the exiled card to its owner's hand.", Some((3, 3))).unwrap());
    let marvin = g.put(0, 0, Zone::Battlefield);
    let donor = g.put(0, pair, Zone::Battlefield);
    let donor_card = g.put(0, 2, Zone::Graveyard);
    let borrower_card = g.put(0, 4, Zone::Graveyard);
    let mv = |source, ability, card| Move::Activate { source, ability, targets: vec![vec![Target::Object(card)]], paying: vec![] };
    g.act(0, mv(donor, 0, donor_card)).unwrap(); settle(&mut g);
    g.act(0, activate(marvin, 2, "")).unwrap(); settle(&mut g);
    cite!(["607.5"]; assert!(g.players[0].hand.is_empty()));
    g.act(0, mv(marvin, 1, borrower_card)).unwrap(); settle(&mut g);
    g.act(0, activate(marvin, 2, "")).unwrap(); settle(&mut g);
    cite!(["607.5"]; assert_eq!(g.players[0].hand.iter().map(|id| g.obj(*id).chars.name.as_str()).collect::<Vec<_>>(), ["Brave Brawler"]));
    cite!(["607.5"]; assert_eq!(g.exile.iter().filter(|id| g.obj(**id).chars.name == "Afterburner Expert").count(), 1));
}

#[test]
fn r113_3_inheritance_does_not_duplicate_donor_static_cost_reductions() {
    let mut g = game();
    let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let scholar = add(&mut g, CardDef::from_faces(CardDef::catalog(&faces).get("Boom Scholar").unwrap()).unwrap());
    g.put(0, 0, Zone::Battlefield);
    g.put(0, scholar, Zone::Battlefield);
    let engineer = g.put(0, 1, Zone::Battlefield);
    g.players[0].pool.colors[ir::Color::Red as usize] = 1;
    // Engineer's3R is reduced by the real Scholar's2 exactly once, to1R.
    let before=format!("{g:?}");
    cite!(["113.3b","113.3d","113.10","118.7a","118.3a"]; refused("the supplied mana does not pay that cost, all of it and no more"): g.act(0, activate(engineer, 1, "R")));
    assert_eq!(format!("{g:?}"),before);
    g.players[0].pool.colorless = 1;
    cite!(["113.3b","113.3d","113.10","118.7a"]; ok: g.act(0, activate(engineer, 1, "CR")));
    settle(&mut g);
    assert_eq!(g.players[0].pool.colorless, 0);
}

// A modified copied token has an immutable Derived definition. Independent creations
// differ by donor incarnation, while reevaluation of the same face retains used history.
#[test]
fn r602_5c_derived_token_donors_retain_identity_and_distinguish_new_creations() {
    use game::Target;
    let mut g = game();
    let marvin = g.put(0, 0, Zone::Battlefield);
    let model = g.put(1, 1, Zone::Battlefield);
    let mirror = add(&mut g, CardDef::new("Small Mirror", "{0}", "Instant", "Create a token that's a copy of target creature, except it's 1/1.", None).unwrap());
    cast_on(&mut g, 0, mirror, vec![vec![Target::Object(model)]]);
    g.players[0].pool.colors[ir::Color::Red as usize] = 3;
    g.act(0, activate(marvin, 1, "R")).unwrap(); settle(&mut g);
    cite!(["602.5b", "702.177a"]; refused("only once"): g.act(0, activate(marvin, 1, "R")));
    cast_on(&mut g, 0, mirror, vec![vec![Target::Object(model)]]);
    cite!(["602.5c", "707.2"]; ok: g.act(0, activate(marvin, 3, "R")));
    settle(&mut g);
}

// The granting static ability itself has a new acquisition after another copy effect.
#[test]
fn r602_5c_new_borrower_copy_acquisition_creates_new_grant_identity() {
    use game::Target;
    let mut g = game();
    let marvin = g.put(0, 0, Zone::Battlefield);
    let model = g.put(1, 0, Zone::Battlefield);
    g.put(0, 1, Zone::Battlefield);
    let copy = add(&mut g, CardDef::new("Recopy Mimic", "{0}", "Instant", "Target creature becomes a copy of target creature.", None).unwrap());
    for acquisition in 0..3 {
        if acquisition != 0 { cast_on(&mut g, 0, copy, vec![vec![Target::Object(marvin)], vec![Target::Object(model)]]); }
        g.players[0].pool.colors[ir::Color::Red as usize] = 2;
        cite!(["602.5c", "707.2"]; ok: g.act(0, activate(marvin, 1, "R")));
        settle(&mut g);
        cite!(["702.177a"]; refused("only once"): g.act(0, activate(marvin, 1, "R")));
    }
}
