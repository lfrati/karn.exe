//! The worked examples of rule 602 (activating activated abilities), with real cards.
use crate::support;
use support::*;

/// 602.1a: the activation cost of "{2}, {T}: You gain 1 life" (Fountain of Youth) is two mana
/// of any type plus tapping the permanent.
#[test]
fn r602_1a_example_the_cost_is_two_mana_plus_tapping() {
    let mut t = Board::new(2).battlefield(0, &["Fountain of Youth"]).build();
    t.mana(0, "{1}");
    cite!(["602.1a", "118.3a"]; refused("that mana is not in your mana pool (118.3a)"): t.activate(0, "Fountain of Youth", 0, vec![], crate::support::mana_pool("CC")));
    cite!(["602.1a"]; ok: t.mana(0, "{G}").activate(0, "Fountain of Youth", 0, vec![], crate::support::mana_pool("CG")));
    t.expect().rules(["602.1a"]).tapped("Fountain of Youth", true).holds(|g| g.players[0].pool.total() == 0, "both mana spent").check();
    t.resolve();
    t.expect().life(0, 21).check();
    t.mana(0, "{2}");
    cite!(["107.5"]; refused("Fountain of Youth can't pay {T} as it is (107.5, 118.3)"): t.activate(0, "Fountain of Youth", 0, vec![], crate::support::mana_pool("CC")));
}
