//! Rolling dice (CR 706) is read by the grammar but not run by the engine: a card that rolls, or
//! watches for a roll, stays refused, and the refusal names the roll.
use game::CardDef;

fn refusal(name: &str, cost: &str, types: &str, text: &str, pt: Option<(i32, i32)>) -> String {
    let d = CardDef::new(name, cost, types, text, pt).expect(name);
    match game::play::check(&d) {
        Ok(()) => panic!("{name} should be refused"),
        Err(u) => u.0,
    }
}

/// 706.1a: a roll of an "N-sided die".
#[test]
fn r706_1a_a_roll_is_refused() {
    let why = refusal(
        "Velukan Dragon",
        "{5}{R}{R}",
        "Creature — Dragon",
        "Flying\nWhenever this creature attacks or blocks, roll a six-sided die. This creature gets +X/+0 until end of turn, where X is the result minus 1.",
        Some((5, 5)),
    );
    assert!(why.contains("RollDie"), "{why}");
}

/// 706.2: a trigger on a die's natural result (Netherese Puzzle-Ward's second ability alone, so
/// its own roll is not what is refused).
#[test]
fn r706_2_a_trigger_on_a_natural_result_is_refused() {
    let why = refusal(
        "Netherese Puzzle-Ward",
        "{3}{U}",
        "Enchantment",
        "Perfect Illumination — Whenever you roll a die's highest natural result, draw a card.",
        None,
    );
    assert!(why.contains("RollDice"), "{why}");
}
