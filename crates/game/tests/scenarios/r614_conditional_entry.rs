//! Conditional entry replacements use the ordinary condition and prospective entry view.
use crate::support::{cite, paid};
use game::{CardDef, Game, Move, Target, Zone};
fn crab() -> CardDef {
    CardDef::new("Eddymurk Crab","{5}{U}{U}","Creature — Elemental Crab","Flash\nThis spell costs {1} less to cast for each instant and sorcery card in your graveyard.\nThis creature enters tapped if it's not your turn.\nWhen this creature enters, tap up to two target creatures.",Some((5,5))).unwrap()
}
fn resolve(g: &mut Game) {
    for _ in 0..g.players.len() {
        g.act(g.priority, Move::Pass).unwrap();
    }
}
#[test]
fn r614_1c_conditional_tapped_entry_is_checked_on_each_controllers_turn() {
    for active in 0..3 {
        let d = crab();
        game::play::check(&d).unwrap();
        let bear = CardDef::new("Bear", "{0}", "Creature — Bear", "", Some((2, 2))).unwrap();
        let mut g = Game::new(vec![d, bear], 3, 20);
        g.active = active;
        g.priority = 0;
        let a = g.put(1, 1, Zone::Battlefield);
        let b = g.put(2, 1, Zone::Battlefield);
        let card = g.put(0, 0, Zone::Hand);
        g.players[0].pool.colors[ir::Color::Blue as usize] = 7;
        g.act(0, paid("UUUUUUU", Move::Cast { card, targets: vec![] })).unwrap();
        resolve(&mut g);
        let crab = *g.battlefield.iter().find(|o| g.obj(**o).card == Some(0)).unwrap();
        cite!(["614.1d"]; case: format!("active {active}"); assert_eq!(g.obj(crab).tapped, active != 0));
        g.act(0, Move::ChooseTargets(vec![vec![Target::Object(a), Target::Object(b)]])).unwrap();
        resolve(&mut g);
        assert!(g.obj(a).tapped && g.obj(b).tapped);
    }
}
#[test]
fn r614_12_conditional_entry_and_graveyard_cost_reduction_compose() {
    let d = crab();
    game::play::check(&d).unwrap();
    let spell = CardDef::new("Shock", "{R}", "Instant", "Shock deals 2 damage to any target.", None).unwrap();
    let mut g = Game::new(vec![d, spell], 2, 20);
    g.active = 1;
    g.priority = 0;
    for _ in 0..5 {
        g.put(0, 1, Zone::Graveyard);
    }
    let card = g.put(0, 0, Zone::Hand);
    g.players[0].pool.colors[ir::Color::Blue as usize] = 2;
    g.act(0, paid("UU", Move::Cast { card, targets: vec![] })).unwrap();
    resolve(&mut g);
    let crab = g.battlefield[0];
    cite!(["614.1d"]; assert!(g.obj(crab).tapped));
    cite!(["601.2f"]; assert_eq!(g.players[0].pool.total(), 0));
    g.act(0, Move::ChooseTargets(vec![vec![]])).unwrap();
    resolve(&mut g);
}
