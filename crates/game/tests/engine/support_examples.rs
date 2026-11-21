//! The shared test support in use: a board by names, actions, and the expected outcome as
//! an explicit next state or as properties.
use crate::support;
use support::*;

// Rules: 120.3e, 704.5g, 400.7
// Given: a 2/2 creature and a spell that deals 3 damage.
// When: cast the spell targeting the creature and resolve it.
// Expect: both cards in their owners' graveyards, no life change or other position change.
// Why: marked lethal damage destroys the creature at the following state-based check.
#[test]
fn explicit_next_state_lightning_bolt_kills_grizzly_bears() {
    let mut t = Board::new(2).battlefield(0, &["Mountain"]).named(0, Zone::Hand, "bolt", "Lightning Bolt")
        .named(1, Zone::Battlefield, "bear", "Grizzly Bears").build();
    let before = t.state();
    assert!(before.players.iter().all(|p| p.library == 0), "undeclared libraries are empty");
    t.mana(0, "{R}").cast(0, "bolt", vec!["bear".into()], crate::support::mana_pool("R")).unwrap();
    t.resolve();
    let mut want = before;
    want.players[0].hand.clear();
    want.players[0].graveyard = vec!["Lightning Bolt".into()];
    want.players[1].battlefield.clear();
    want.players[1].graveyard = vec!["Grizzly Bears".into()];
    assert_eq!(t.state(), want);
    t.expect().zone_exact(0, Zone::Graveyard, &["Lightning Bolt"])
        .zone_exact(1, Zone::Graveyard, &["Grizzly Bears"]).life(1, 20).stack(&[]).check();
}

#[test]
fn properties_frantic_search_untaps_up_to_three_lands_of_any_player() {
    let mut t = Board::new(2).hand(0, &["Frantic Search"]).battlefield(0, &["Island", "Island", "Island"])
        .tapped(1, &["Swamp"]).library(0, &["Forest", "Plains"]).build();
    t.mana(0, "{2}{U}").cast(0, "Frantic Search", vec![], crate::support::mana_pool("CCU")).unwrap();
    t.pass_all();
    // The whole hand is discarded; its owner still chooses the graveyard order (404.3).
    t.choose(0,&["Forest","Plains"]).unwrap();
    // Then the lands are chosen.
    let Some((0, Asked::Select { min: 0, max: 1, among })) = t.asked() else { panic!("{:?}", t.asked()) };
    assert_eq!(among.len(),1, "the untapped Islands cannot be chosen for this action");
    t.choose(0, &["Swamp"]).unwrap();
    t.expect()
        .tapped("Swamp", false)
        .zone(0, Zone::Hand, &[])
        .zone(0, Zone::Graveyard, &["Forest", "Plains", "Frantic Search"])
        .check();
}
