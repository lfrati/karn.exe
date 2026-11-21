//! The worked examples of rule 605 (mana abilities), with real cards.
use crate::support;
use support::*;

/// 605.2: "{T}: Add {G} for each creature you control" (Gaea's Cradle) is still a mana
/// ability with no creatures: it resolves at once, without the stack, adding nothing. Once
/// the permanent is tapped it's still a mana ability, but its cost can't be paid.
#[test]
fn r605_2_example_gaeas_cradle_is_a_mana_ability_even_with_no_creatures() {
    let mut t = Board::new(2).battlefield(0, &["Gaea's Cradle"]).build();
    let cradle = t.find_in_battlefield("Gaea's Cradle");
    cite!(["605.2"]; ok: t.g.act(0, Move::Activate { source: cradle, ability: 0, targets: vec![], paying: vec![] }));
    t.expect().tapped("Gaea's Cradle", true).stack(&[]).holds(|g| g.players[0].pool.total() == 0, "no mana added").check();
    cite!(["107.5"]; refused("Gaea's Cradle can't pay {T} as it is (107.5, 118.3)"): t.g.act(0, Move::Activate { source: cradle, ability: 0, targets: vec![], paying: vec![] }));
}
