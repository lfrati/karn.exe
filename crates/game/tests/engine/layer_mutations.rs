//! Revision boundaries are infrastructure for613/614 views, not changes to their rules.
use game::{CardDef, Game, Zone};
use game::layers::{Change, Continuous, Lifetime};
use std::sync::Arc;

fn life_game() -> (Game, game::ObjId) {
    let def = CardDef::new("Life reader", "{0}", "Creature — Bear",
        "This creature gets +1/+1 as long as your life total is greater than your starting life total.", Some((2, 2))).unwrap();
    game::play::check(&def).unwrap();
    let mut game = Game::new(vec![def], 2, 20);
    let bear = game.put(0, 0, Zone::Battlefield);
    (game, bear)
}

#[test]
fn direct_player_loans_invalidate_before_writing_and_reads_do_not_advance() {
    let (mut g, bear) = life_game();
    let read_revision = g.layer_revision();
    assert_eq!(g.power(bear), Some(2));
    assert_eq!(g.state_view().players[0].life, 20);
    assert_eq!(g.layer_revision(), read_revision);
    // Even an unused mutable loan invalidates before returning it.
    let _loan = &mut g.players[0];
    assert!(g.layer_revision() > read_revision);
    let before_write = g.layer_revision();
    g.players[0].life = 21;
    assert!(g.layer_revision() > before_write);
    assert_eq!(g.power(bear), Some(3));
    g.state_mut().players[0].life = 20;
    assert_eq!(g.power(bear), Some(2));
}

#[test]
fn direct_zone_catalog_and_effect_changes_are_seen_in_the_next_read() {
    let defs = vec![
        CardDef::new("Bear", "{0}", "Creature — Bear", "", Some((2, 2))).unwrap(),
        CardDef::new("Anthem", "{0}", "Enchantment", "Creatures you control get +1/+1.", None).unwrap(),
    ];
    let mut g = Game::new(defs, 2, 20);
    let bear = g.put(0, 0, Zone::Battlefield);
    let anthem = g.put(0, 1, Zone::Battlefield);
    assert_eq!(g.power(bear), Some(3));
    let revision = g.layer_revision();
    // Raw fixture membership editing is tracked even though it emits no game event.
    g.battlefield.retain(|on|*on != anthem);
    assert!(g.layer_revision() > revision);
    assert_eq!(g.power(bear), Some(2));
    g.battlefield.push(anthem);
    let revision = g.layer_revision();
    Arc::make_mut(&mut g.defs)[1] = CardDef::new("Anthem", "{0}", "Enchantment",
        "Creatures you control get +2/+2.", None).unwrap();
    assert!(g.layer_revision() > revision);
    assert_eq!(g.power(bear), Some(4));
    g.continuous.push(Continuous { producer:None, change: Change::Pt(3, 3), affected: vec![bear],
        stamp: 100, part: 0, lifetime: Lifetime::Forever });
    assert_eq!(g.power(bear), Some(7));
    g.continuous.clear();
    assert_eq!(g.power(bear), Some(4));
}

#[test]
fn replacing_a_guarded_fixture_view_invalidates_while_restoring_rules_state() {
    let (mut g, bear) = life_game();
    let before = g.state_view().clone();
    g.players[0].life = 21;
    assert_eq!(g.power(bear), Some(3));
    let changed = g.layer_revision();
    *g.state_mut() = before;
    assert!(g.layer_revision() > changed);
    assert_eq!(g.power(bear), Some(2));
}

#[test]
fn clone_mutation_and_catalog_replacement_do_not_change_the_source_view() {
    let (g, bear) = life_game();
    let original_revision = g.layer_revision();
    assert_eq!(g.power(bear), Some(2));
    let mut branch = g.clone();
    assert!(Arc::ptr_eq(&g.defs, &branch.defs));
    branch.players[0].life = 21;
    assert_eq!(branch.power(bear), Some(3));
    Arc::make_mut(&mut branch.defs)[0] = CardDef::new("Life reader", "{0}", "Creature — Bear",
        "This creature gets +4/+4.", Some((2, 2))).unwrap();
    assert_eq!(branch.power(bear), Some(6));
    assert_eq!(g.power(bear), Some(2));
    assert_eq!(g.players[0].life, 20);
    assert_eq!(g.layer_revision(), original_revision);
}
