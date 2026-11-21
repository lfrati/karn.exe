//! Graveyard cards have abilities even when those abilities do not function there.
use crate::support::{cite, paid};
use game::{CardDef, Game, Move, ObjId, Zone};
use std::sync::{Arc, OnceLock};

fn definitions() -> Vec<CardDef> {
    static DEFS: OnceLock<Vec<CardDef>> = OnceLock::new();
    DEFS.get_or_init(|| {
        let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
        let catalog = CardDef::catalog(&faces);
        let mut defs: Vec<_> = ["Thranduil, the Elvenking", "Greenbelt Guardian", "Llanowar Elves", "Stoic Grove-Guide", "Morcant's Eyes"]
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
fn activate(source: ObjId, ability: usize) -> Move { Move::Activate { source, ability, targets: vec![], paying: vec![] } }
fn settle(g: &mut Game) {
    for _ in 0..20 {
        if g.stack.is_empty() { return; }
        for _ in 0..2 { g.act(g.priority, Move::Pass).unwrap(); }
    }
    panic!("unsettled stack");
}

#[test]
fn r113_6_graveyard_donor_ability_functions_on_the_borrower_not_the_donor() {
    let mut g = game();
    let source = g.put(0, 0, Zone::Battlefield);
    let donor = g.put(0, 1, Zone::Graveyard);
    g.put(1, 1, Zone::Graveyard);
    cite!(["113.10c", "400.3"]; assert_eq!(g.abilities(source).len(), 4)); // own static/trigger + two Guardian rows
    g.players[0].pool.colorless = 3;
    g.players[0].pool.colors[ir::Color::Green as usize] = 1;
    cite!(["113.6"]; refused("battlefield"): g.act(0, paid("CCCG",activate(donor, 1))));
    g.act(0, paid("CCCG",activate(source, 3))).unwrap();
    // Membership can disappear without invalidating the captured activated occurrence.
    g.move_object(donor, Zone::Exile);
    cite!(["113.7a"]; assert_eq!(g.abilities(source).len(), 2));
    settle(&mut g);
    cite!(["201.5b", "113.7a"]; assert_eq!(g.obj(source).counters.get("+1/+1"), Some(&3)));
}

#[test]
fn r602_5c_new_graveyard_incarnation_supplies_a_new_exhaust_instance() {
    let mut g = game();
    let source = g.put(0, 0, Zone::Battlefield);
    let mut donor = g.put(0, 1, Zone::Graveyard);
    for generation in 0..2 {
        g.players[0].pool.colorless = 3;
        g.players[0].pool.colors[ir::Color::Green as usize] = 2;
        g.act(0, paid("CCCG",activate(source, 3))).unwrap(); settle(&mut g);
        g.players[0].pool.colorless = 3;
        cite!(["702.177a"]; refused("only once"): g.act(0, paid("CCCG",activate(source, 3))));
        let mut cloned = g.clone();
        cite!(["602.5b"]; refused("only once"): cloned.act(0, paid("CCCG",activate(source, 3))));
        if generation == 0 {
            g.move_object(donor, Zone::Exile);
            donor = *g.exile.last().unwrap();
            g.move_object(donor, Zone::Graveyard);
            donor = *g.players[0].graveyard.last().unwrap();
        }
    }
    cite!(["400.7", "602.5c"]; assert_eq!(g.obj(source).counters.get("+1/+1"), Some(&6)));
}

#[test]
fn r201_5b_inherited_mana_taps_the_borrower_not_the_graveyard_card() {
    let mut g = game();
    let source = g.put(0, 0, Zone::Battlefield);
    let donor = g.put(0, 2, Zone::Graveyard);
    cite!(["113.6", "605.3b"]; ok: g.act(0, activate(source, 2)));
    cite!(["201.5b"]; assert!(g.obj(source).tapped));
    assert!(!g.obj(donor).tapped);
    assert_eq!(g.players[0].pool.colors[ir::Color::Green as usize], 1);
    assert!(g.stack.is_empty());
}

#[test]
fn r113_6m_graveyard_specific_exile_cost_is_not_usable_on_battlefield_borrower() {
    let mut g = game();
    let source = g.put(0, 0, Zone::Battlefield);
    let donor = g.put(0, 3, Zone::Graveyard);
    assert_eq!(g.abilities(source).len(), 3);
    g.players[0].pool.colorless = 1;
    g.players[0].pool.colors[ir::Color::Green as usize] = 1;
    cite!(["113.6m"]; refused("zone"): g.act(0, paid("CG",activate(source, 2))));
    assert_eq!(g.obj(source).zone, Zone::Battlefield);
    assert_eq!(g.obj(donor).zone, Zone::Graveyard);
}

#[test]
fn r201_5b_noncreature_elf_card_cost_and_late_quantity_use_borrower_context() {
    let mut g = game();
    let source = g.put(0, 0, Zone::Battlefield);
    g.put(0, 4, Zone::Graveyard);
    // The upkeep Surveil trigger is not inherited; only the sacrifice activation is.
    cite!(["113.10c"]; assert_eq!(g.abilities(source).len(), 3));
    g.players[0].pool.colorless = 4;
    g.players[0].pool.colors[ir::Color::Green as usize] = 2;
    cite!(["201.5b"]; ok: g.act(0, paid("CCCCGG",activate(source, 2))));
    assert_eq!(g.battlefield.len(), 0);
    settle(&mut g);
    // Morcant's Eyes plus sacrificed Thranduil: two Elf cards at resolution.
    cite!(["201.5b", "608.2h"]; assert_eq!(g.battlefield.len(), 2));
    for token in &g.battlefield { assert!(g.obj(*token).token()); }
}

fn add(g: &mut Game, def: CardDef) -> usize {
    game::play::check(&def).unwrap_or_else(|e| panic!("{}: {e:?}", def.name()));
    let index = g.defs.len();
    Arc::make_mut(&mut g.defs).push(def);
    index
}
fn cast_on(g: &mut Game, definition: usize, targets: Vec<Vec<game::Target>>) {
    let card = g.put(0, definition, Zone::Hand);
    g.act(0, Move::Cast { card, targets }).unwrap(); settle(g);
}

#[test]
fn r109_1_elf_token_in_graveyard_is_not_an_elf_card_donor() {
    let mut g = game();
    let source = g.put(0, 0, Zone::Battlefield);
    let model = g.put(1, 1, Zone::Battlefield);
    let mirror = add(&mut g, CardDef::new("Mirror", "{0}", "Instant", "Create a token that's a copy of target creature.", None).unwrap());
    cast_on(&mut g, mirror, vec![vec![game::Target::Object(model)]]);
    let token = *g.battlefield.iter().find(|o| g.obj(**o).token()).unwrap();
    g.move_object(token, Zone::Graveyard);
    // Before the next SBA, the token is still present in the graveyard. It is not a card.
    assert!(g.players[0].graveyard.iter().any(|o| g.obj(*o).token()));
    cite!(["109.1", "111.6"]; assert_eq!(g.abilities(source).len(), 2));
}

#[test]
fn r400_7_graveyard_donor_does_not_keep_its_battlefield_copy_characteristics() {
    let mut g = game();
    let source = g.put(0, 0, Zone::Battlefield);
    let donor = g.put(0, 1, Zone::Battlefield);
    let model = g.put(1, 2, Zone::Battlefield);
    let mirror = add(&mut g, CardDef::new("Mirror", "{0}", "Instant", "Target creature becomes a copy of target creature.", None).unwrap());
    cast_on(&mut g, mirror, vec![vec![game::Target::Object(donor)], vec![game::Target::Object(model)]]);
    assert_eq!(g.obj(donor).chars.name, "Llanowar Elves");
    g.move_object(donor, Zone::Graveyard);
    let card = *g.players[0].graveyard.last().unwrap();
    cite!(["400.7"]; assert_eq!(g.obj(card).chars.name, "Greenbelt Guardian"));
    cite!(["400.7"]; assert_eq!(g.abilities(source).len(), 4));
    g.players[0].pool.colorless = 3;
    g.players[0].pool.colors[ir::Color::Green as usize] = 1;
    g.act(0, paid("CCCG",activate(source, 3))).unwrap(); settle(&mut g);
    assert_eq!(g.obj(source).counters.get("+1/+1"), Some(&3));
}

#[test]
fn r712_8a_graveyard_membership_and_abilities_use_the_normal_front_face() {
    let mut g = game();
    let source = g.put(0, 0, Zone::Battlefield);
    let front = CardDef::new("Front Elf", "{0}", "Creature — Elf", "{0}: Transform this creature.\n{0}: You gain 1 life.", Some((2, 2))).unwrap();
    let back = CardDef::new("Back Human", "", "Creature — Human", "{0}: You gain 3 life.", Some((2, 2))).unwrap();
    let index = add(&mut g, CardDef::transforming(front, back).unwrap());
    let donor = g.put(0, index, Zone::Battlefield);
    g.act(0, activate(donor, 0)).unwrap(); settle(&mut g);
    assert!(g.obj(donor).transformed);
    g.move_object(donor, Zone::Graveyard);
    let card = *g.players[0].graveyard.last().unwrap();
    cite!(["712.8a"]; assert!(!g.obj(card).transformed));
    assert_eq!(g.obj(card).chars.name, "Front Elf");
    cite!(["712.8a", "113.10c"]; assert_eq!(g.abilities(source).len(), 4));
    g.act(0, activate(source, 3)).unwrap(); settle(&mut g);
    assert_eq!(g.players[0].life, 21);
}

#[test]
fn r702_73a_changeling_card_is_an_elf_in_the_same_partial_graveyard_world() {
    let mut g = game();
    let source = g.put(0, 0, Zone::Battlefield);
    let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let catalog = CardDef::catalog(&faces);
    let index = add(&mut g, CardDef::from_faces(catalog.get("Three Tree Mascot").unwrap()).unwrap());
    g.put(0, index, Zone::Graveyard);
    cite!(["113.6a", "702.73a"]; assert_eq!(g.abilities(source).len(), 3));
    g.players[0].pool.colorless = 1;
    // Single colored mana needs an explicit choice for this mana ability.
    let mv = Move::Adding(vec![vec![ir::mana::ManaSymbol::Color(ir::Color::Blue)]], Box::new(paid("C",activate(source, 2))));
    g.act(0, mv).unwrap();
    assert_eq!(g.players[0].pool.colors[ir::Color::Blue as usize], 1);
}

#[test]
fn r113_6j_self_discard_cost_cannot_be_paid_by_battlefield_borrower() {
    let mut g = game();
    let source = g.put(0, 0, Zone::Battlefield);
    let index = add(&mut g, CardDef::new("Elf Card", "{0}", "Creature — Elf", "Discard this card: You gain 1 life.", Some((1, 1))).unwrap());
    g.put(0, index, Zone::Graveyard);
    assert_eq!(g.abilities(source).len(), 3);
    cite!(["113.6j", "113.6m"]; refused("zone"): g.act(0, activate(source, 2)));
    assert_eq!(g.players[0].life, 20);
}

#[test]
fn unmodeled_private_mixed_and_linked_card_donor_domains_remain_refused() {
    for zone in ["your hand", "your library", "your hand or graveyard", "exile"] {
        let text = format!("This creature has all activated abilities of all Elf cards in {zone}.");
        let def = CardDef::new("Unmodeled Donors", "{0}", "Creature", &text, Some((1, 1))).unwrap();
        let reason=if zone=="exile" {"noun phrase: an unsupported zone or zone owner"}
            else {"inherited card abilities: donor domain must be a fixed graveyard"};
        crate::support::refused(game::play::check(&def).map_err(|e|e.0),reason);
    }
    let def = CardDef::new("Unmodeled Removal", "{0}", "Enchantment", "All cards in graveyards lose all abilities.", None).unwrap();
    // Reading cards does not newly model all-zone ability removal.
    crate::support::refused(game::play::check(&def).map_err(|e|e.0),"loses: ability removal outside a permanent scope");
}

#[test]
fn r400_3_graveyard_owner_overrides_the_donors_former_controller() {
    let mut g = game();
    let source = g.put(0, 0, Zone::Battlefield);
    let own = g.put(0, 1, Zone::Battlefield);
    let foreign = g.put(1, 2, Zone::Battlefield);
    let control = add(&mut g, CardDef::new("Control", "{0}", "Instant", "Gain control of target creature.", None).unwrap());
    // The opponent controls our card before it dies; we control their card.
    g.act(0, Move::Pass).unwrap();
    let spell = g.put(1, control, Zone::Hand);
    g.act(1, Move::Cast { card: spell, targets: vec![vec![game::Target::Object(own)]] }).unwrap(); settle(&mut g);
    cast_on(&mut g, control, vec![vec![game::Target::Object(foreign)]]);
    assert_eq!(g.controller(own), 1);
    assert_eq!(g.controller(foreign), 0);
    g.move_object(own, Zone::Graveyard);
    g.move_object(foreign, Zone::Graveyard);
    cite!(["400.3", "109.5"]; assert_eq!(g.abilities(source).len(), 4));
    g.players[0].pool.colorless = 3;
    g.players[0].pool.colors[ir::Color::Green as usize] = 1;
    g.act(0, paid("CCCG",activate(source, 3))).unwrap(); settle(&mut g);
    assert_eq!(g.obj(source).counters.get("+1/+1"), Some(&3));
}

// Synthetic linked Elf pair, exercised through native Thranduil. The Flash suppression
// and removal spells provide real rules transitions while older activations are stacked.
fn archive_fixture() -> (Game, ObjId, ObjId, usize, usize, usize) {
    let mut g = game();
    let pair = add(&mut g, CardDef::new("Archive Elf", "{0}", "Creature — Elf",
        "{0}: Exile target creature card from your graveyard.\n{0}: Return the exiled card to its owner's hand.\nAt the beginning of your upkeep, return the exiled card to its owner's hand.", Some((3, 3))).unwrap());
    let payload = add(&mut g, CardDef::new("Stored Bear", "{0}", "Creature — Bear", "", Some((2, 2))).unwrap());
    let strip = add(&mut g, CardDef::new("Suppressor", "{0}", "Enchantment", "Flash\nAll creatures lose all abilities.", None).unwrap());
    let remove = add(&mut g, CardDef::new("Remove Suppressor", "{0}", "Instant", "Destroy target enchantment.", None).unwrap());
    let exile = add(&mut g, CardDef::new("Source Departure", "{0}", "Instant", "Exile target creature.", None).unwrap());
    let source = g.put(0, 0, Zone::Battlefield);
    g.put(0, pair, Zone::Graveyard);
    let card = g.put(0, payload, Zone::Graveyard);
    (g, source, card, strip, remove, exile)
}
fn archive_exile(g: &mut Game, source: ObjId, card: ObjId) {
    g.act(0, Move::Activate { source, ability: 2, targets: vec![vec![game::Target::Object(card)]], paying: vec![] }).unwrap();
}
fn cast_top(g: &mut Game, definition: usize, targets: Vec<Vec<game::Target>>) {
    let card = g.put(0, definition, Zone::Hand);
    g.act(0, Move::Cast { card, targets }).unwrap();
    for _ in 0..2 { g.act(g.priority, Move::Pass).unwrap(); }
}
fn assert_archived(g: &Game) {
    cite!(["607.5"]; assert!(g.players[0].hand.is_empty()));
    cite!(["607.5"]; assert_eq!(g.exile.iter().filter(|id| g.obj(**id).chars.name == "Stored Bear").count(), 1));
}

// Ruling 813733ac37b0c076: unlike once-use history, a gained linked pair's
// link ends when those abilities are lost. Regaining the same acquisition is not
// permission to access its earlier exiled card (607.5).
#[test]
fn r607_5_inherited_link_ends_on_loss_even_when_the_same_pair_returns() {
    let (mut g, source, card, strip, remove, _) = archive_fixture();
    archive_exile(&mut g, source, card); settle(&mut g);
    cast_on(&mut g, strip, vec![]);
    let suppressor = *g.battlefield.last().unwrap();
    cite!(["613.1f"]; assert!(g.abilities(source).is_empty()));
    cast_on(&mut g, remove, vec![vec![game::Target::Object(suppressor)]]);
    cite!(["113.10c"]; assert_eq!(g.abilities(source).len(), 4));
    g.act(0, activate(source, 3)).unwrap(); settle(&mut g);
    assert_archived(&g);
    // A new writer belongs to the regained relationship, not the ended one.
    let fresh_def = add(&mut g, CardDef::new("Fresh Fox", "{0}", "Creature — Fox", "", Some((2, 2))).unwrap());
    let fresh = g.put(0, fresh_def, Zone::Graveyard);
    archive_exile(&mut g, source, fresh); settle(&mut g);
    g.act(0, activate(source, 3)).unwrap(); settle(&mut g);
    cite!(["607.5"]; assert_eq!(g.players[0].hand.iter().map(|id| g.obj(*id).chars.name.as_str()).collect::<Vec<_>>(), ["Fresh Fox"]));
    cite!(["607.5"]; assert_eq!(g.exile.iter().filter(|id| g.obj(**id).chars.name == "Stored Bear").count(), 1));
}

// 113.7a preserves the occurrence, not information from a still-live source. 608.2h
// therefore cannot revive a relationship ended by live suppression/regain.
#[test]
fn r608_2h_stacked_link_reader_cannot_revive_a_live_source_ended_link() {
    let (mut g, source, card, strip, remove, _) = archive_fixture();
    archive_exile(&mut g, source, card); settle(&mut g);
    g.act(0, activate(source, 3)).unwrap();
    cast_top(&mut g, strip, vec![]);
    let suppressor = *g.battlefield.last().unwrap();
    assert!(g.abilities(source).is_empty());
    cast_top(&mut g, remove, vec![vec![game::Target::Object(suppressor)]]);
    assert_eq!(g.stack.len(), 1); // Only the previously announced linked reader.
    settle(&mut g);
    assert_archived(&g);
}

// The departed source's exact LKI retains a relationship intact when it left;
// this is different from losing the pair while the source remains on the battlefield.
#[test]
fn r608_2h_stacked_link_reader_uses_intact_departed_source_relationship() {
    let (mut g, source, card, _, _, exile) = archive_fixture();
    archive_exile(&mut g, source, card); settle(&mut g);
    g.act(0, activate(source, 3)).unwrap();
    cast_top(&mut g, exile, vec![vec![game::Target::Object(source)]]);
    assert_eq!(g.stack.len(), 1);
    settle(&mut g);
    cite!(["113.7a", "608.2h"]; assert_eq!(g.players[0].hand.iter().map(|id| g.obj(*id).chars.name.as_str()).collect::<Vec<_>>(), ["Stored Bear"]));
}

// An old writer still performs its instruction, but its ended relationship is not
// the relationship acquired after suppression (113.7a/607.5).
#[test]
fn r607_5_late_old_link_writer_cannot_populate_the_regained_pair() {
    let (mut g, source, card, strip, remove, _) = archive_fixture();
    archive_exile(&mut g, source, card);
    cast_top(&mut g, strip, vec![]);
    let suppressor = *g.battlefield.last().unwrap();
    cast_top(&mut g, remove, vec![vec![game::Target::Object(suppressor)]]);
    assert_eq!(g.stack.len(), 1);
    settle(&mut g); // The old writer still successfully exiles Stored Bear.
    assert_archived(&g);
    g.act(0, activate(source, 3)).unwrap(); settle(&mut g);
    assert_archived(&g);
}

// Departure cannot turn an already-ended relationship into valid LKI.
#[test]
fn r608_2h_departure_after_link_loss_cannot_resurrect_old_data() {
    let (mut g, source, card, strip, remove, exile) = archive_fixture();
    archive_exile(&mut g, source, card); settle(&mut g);
    cast_on(&mut g, strip, vec![]);
    let suppressor = *g.battlefield.last().unwrap();
    cast_on(&mut g, remove, vec![vec![game::Target::Object(suppressor)]]);
    g.act(0, activate(source, 3)).unwrap();
    cast_top(&mut g, exile, vec![vec![game::Target::Object(source)]]);
    settle(&mut g);
    assert_archived(&g);
}

// Two written acts are not simultaneous merely because they resolve in one spell.
#[test]
fn r607_5_loss_and_regain_between_acts_in_one_resolution_ends_the_link() {
    use game::Target;
    let mut g = game();
    let borrower = add(&mut g, CardDef::new("Untapped Borrower", "{0}", "Creature",
        "This creature has all activated abilities of other untapped creatures you control.", Some((3, 3))).unwrap());
    let pair = add(&mut g, CardDef::new("Archive Bear", "{0}", "Creature",
        "{0}: Exile target creature card from your graveyard.\n{0}: Return the exiled card to its owner's hand.", Some((3, 3))).unwrap());
    let pulse = add(&mut g, CardDef::new("Pulse", "{0}", "Instant", "Tap target creature. Untap that creature.", None).unwrap());
    let source = g.put(0, borrower, Zone::Battlefield);
    let donor = g.put(0, pair, Zone::Battlefield);
    let card = g.put(0, 1, Zone::Graveyard);
    g.act(0, Move::Activate { source, ability: 1, targets: vec![vec![Target::Object(card)]], paying: vec![] }).unwrap();
    settle(&mut g);
    cast_on(&mut g, pulse, vec![vec![Target::Object(donor)]]);
    assert!(!g.obj(donor).tapped);
    assert_eq!(g.abilities(source).len(), 3);
    g.act(0, activate(source, 2)).unwrap(); settle(&mut g);
    cite!(["607.5", "608.2c"]; assert!(g.players[0].hand.is_empty()));
    assert_eq!(g.exile.iter().filter(|id| g.obj(**id).chars.name == "Greenbelt Guardian").count(), 1);
}

// A linked target predicate needs the current acquisition during offers/legality, before
// executable capture; its retained context subsequently keeps that exact generation.
#[test]
fn r607_5_acquired_linked_target_is_legal_before_program_capture() {
    use game::Target;
    let mut g = game();
    let pair = add(&mut g, CardDef::new("Target Archive Elf", "{0}", "Creature — Elf",
        "{0}: Exile target creature card from your graveyard.\n{0}: Return target creature card exiled with this creature to its owner's hand.", Some((3, 3))).unwrap());
    let payload = add(&mut g, CardDef::new("Stored Bear", "{0}", "Creature — Bear", "", Some((2, 2))).unwrap());
    let source = g.put(0, 0, Zone::Battlefield);
    g.put(0, pair, Zone::Graveyard);
    let card = g.put(0, payload, Zone::Graveyard);
    archive_exile(&mut g, source, card); settle(&mut g);
    let exiled = *g.exile.last().unwrap();
    cite!(["607.5", "602.2b", "601.2c"]; ok: g.act(0, Move::Activate { source, ability: 3, targets: vec![vec![Target::Object(exiled)]], paying: vec![] }));
    settle(&mut g);
    assert_eq!(g.players[0].hand.iter().map(|id| g.obj(*id).chars.name.as_str()).collect::<Vec<_>>(), ["Stored Bear"]);
}

// Two completed mana activations within one spell payment can remove and restore
// donor membership. There is no player-priority sample between them (605.3b).
#[test]
fn r607_5_nested_mana_payment_cannot_hide_an_intervening_link_loss() {
    use game::{Target, legal::Step};
    let mut g = game();
    let borrower = add(&mut g, CardDef::new("Untapped Borrower", "{0}", "Creature",
        "This creature has all activated abilities of other untapped creatures you control.", Some((3, 3))).unwrap());
    let pair = add(&mut g, CardDef::new("Mana Archive Bear", "{0}", "Creature",
        "{0}: Exile target creature card from your graveyard.\n{0}: Return the exiled card to its owner's hand.\n{T}: Add {G}.\n{Q}: Add {G}.", Some((3, 3))).unwrap());
    let spell = add(&mut g, CardDef::new("Mana Payment", "{2}", "Instant", "You gain 1 life.", None).unwrap());
    let source = g.put(0, borrower, Zone::Battlefield);
    let donor = g.put(0, pair, Zone::Battlefield);
    let card = g.put(0, 1, Zone::Graveyard);
    g.act(0, Move::Activate { source, ability: 1, targets: vec![vec![Target::Object(card)]], paying: vec![] }).unwrap(); settle(&mut g);
    let spell = g.put(0, spell, Zone::Hand);
    let step = |ability| Step { plan: vec![], x: None, life: vec![], with: None, cost_branches: None, source: donor, ability, paying: vec![], adding: vec![], chose: vec![] };
    cite!(["605.3b", "601.2g"]; ok: g.act(0, Move::WithMana(vec![step(2), step(3)], Box::new(paid("GG",Move::Cast { card: spell, targets: vec![] })))));
    settle(&mut g);
    assert!(!g.obj(donor).tapped);
    assert_eq!(g.players[0].life, 21);
    g.act(0, activate(source, 2)).unwrap(); settle(&mut g);
    cite!(["607.5"]; assert!(g.players[0].hand.is_empty()));
    assert_eq!(g.exile.iter().filter(|id| g.obj(**id).chars.name == "Greenbelt Guardian").count(), 1);
}
