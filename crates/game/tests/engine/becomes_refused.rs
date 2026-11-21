//! What the grammar reads but the engine does not run (CR 205.1b, 205.3m, 105.1, 613.4b): these
//! cards say more after "becomes" than the engine's layers know, and are refused.

use game::card::CardDef;
use game::play::check;

fn refused(name: &str, cost: &str, types: &str, text: &str, pt: Option<(i32, i32)>, reason: &str) {
    let d = CardDef::new(name, cost, types, text, pt).expect(name);
    // Uncited (admission contract): parsed text must name the unsupported mechanism.
    let error = check(&d).expect_err("this capability is explicitly unsupported");
    assert_eq!(error.0, reason, "{name}");
}

#[test]
fn r205_3m_a_class_of_types_listed_with_abilities_is_refused() {
    refused("Sanctuary", "", "Land", "{4}: This land becomes a 3/3 creature with vigilance and all creature types. It's still a land.", None, "becomes: several things after 'with'");
}

#[test]
fn r613_4b_a_setting_listed_with_abilities_is_refused() {
    refused(
        "Heart",
        "{2}{R}",
        "Creature — Human Shaman",
        "Sacrifice another creature: Until end of turn, this creature becomes a Dragon with base power and toughness 4/4, flying, and haste.",
        Some((2, 2)),
        "becomes: several things after 'with'",
    );
}

#[test]
fn r105_1_becoming_all_colors_is_refused() {
    refused("Basket", "{4}", "Artifact Creature — Scarecrow", "{1}: This creature becomes all colors until end of turn.", Some((3, 2)), "becomes: all colors");
}
