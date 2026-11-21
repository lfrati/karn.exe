//! What the grammar reads of life totals but the engine does not run stays refused, and the
//! refusal names what is not run.
use game::CardDef;

fn refusal(name: &str, cost: &str, types: &str, text: &str, pt: Option<(i32, i32)>) -> String {
    let d = CardDef::new(name, cost, types, text, pt).expect(name);
    match game::play::check(&d) {
        Ok(()) => panic!("{name} should be refused"),
        Err(u) => u.0,
    }
}

#[test]
fn r701_10d_doubling_a_life_total_is_refused() {
    let why = refusal("Revenge", "{4}{W}", "Sorcery", "Double your life total.", None);
    assert!(why.contains("DoubleLife"), "{why}");
}

#[test]
fn r701_12g_exchanging_a_life_total_with_a_stat_is_refused() {
    let why = refusal("Tree of Redemption", "{3}{G}", "Creature — Plant", "Defender\n{T}: Exchange your life total with this creature's toughness.", Some((0, 13)));
    assert!(why.contains("ExchangeLifeWithStat"), "{why}");
}

#[test]
fn r119_1_the_highest_life_total_among_players_is_refused() {
    let why = refusal(
        "Repay in Kind",
        "{5}{B}{B}",
        "Sorcery",
        "Each player's life total becomes the lowest life total among all players.",
        None,
    );
    assert!(why.contains("LifeTotal"), "{why}");
    let why = refusal("Equalize", "{1}{W}", "Sorcery", "You gain life equal to the highest life total among your opponents.", None);
    assert!(why.contains("LifeAggregate"), "{why}");
}

#[test]
fn r614_1a_a_life_floor_against_damage_is_refused() {
    let why = refusal(
        "Ali from Cairo",
        "{2}{R}{R}",
        "Creature — Human",
        "Damage that would reduce your life total to less than 1 reduces it to 1 instead.",
        Some((0, 1)),
    );
    assert!(why.contains("DamageLifeFloor"), "{why}");
}
