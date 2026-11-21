//! Rolling dice (CR 706) and flipping coins (705), as the effect and trigger grammars read them.
use grammar::effect::effects;
use grammar::kernel::Outcome;
use ir::{Action, Amount};
use oracle::token::tokenize;

fn sentence(text: &str) -> Outcome<ir::Sentence> {
    effects().sentence.parse_all(&tokenize(text))
}

/// The one reading of an effect sentence, and how it prints back.
fn read(text: &str) -> (ir::Sentence, String) {
    let s = &effects().sentence;
    match s.parse_all(&tokenize(text)) {
        Outcome::Accepted { value, .. } => {
            let printed = s.print_text(&value, true).unwrap_or_else(|| panic!("{text}: does not print: {value:#?}"));
            (value, printed)
        }
        other => panic!("{text}: {other:#?}"),
    }
}

fn declines(text: &str) {
    assert!(matches!(sentence(text), Outcome::Declined { .. }), "{text} must not read");
}

/// The roll a sentence says, as data.
fn roll(s: &ir::Sentence) -> serde_json::Value {
    fn find(v: &serde_json::Value) -> Option<&serde_json::Value> {
        match v {
            serde_json::Value::Object(m) => m.get("RollDie").or_else(|| m.values().find_map(find)),
            serde_json::Value::Array(a) => a.iter().find_map(find),
            _ => None,
        }
    }
    let json = serde_json::to_value(s).unwrap();
    find(&json).unwrap_or_else(|| panic!("no roll in {s:#?}")).clone()
}

/// Does the sentence say this roll?
fn rolls(text: &str, printed: &str, expected: Action) {
    let (s, p) = read(text);
    assert_eq!(p, printed, "{text}");
    assert_eq!(roll(&s), serde_json::to_value(&expected).unwrap()["RollDie"], "{text}");
}

/// CR 706.1a: an "N-sided die" is a dN; "die" or "dice" agrees with the count. "dN" is the
/// commoner spelling, so it prints.
#[test]
fn r706_1a_n_sided_dice() {
    for (text, printed, count, sides) in [
        ("Roll a six-sided die", "Roll a d6", Amount::Const(1), 6),
        ("Roll a four-sided die", "Roll a d4", Amount::Const(1), 4),
        ("Roll X six-sided dice", "Roll X d6", Amount::X, 6),
        ("Roll two twenty-sided dice", "Roll two d20", Amount::Const(2), 20),
        ("Roll a d20", "Roll a d20", Amount::Const(1), 20),
    ] {
        rolls(text, printed, Action::RollDie { count, sides, modifier: None, kept: None });
    }
    // no die has no sides; the kind is said once; "die" is one, "dice" more
    for text in [
        "Roll a d0",
        "Roll a zero-sided die",
        "Roll a seven-sided d20",
        "Roll two six-sided die",
        "Roll a six-sided dice",
    ] {
        declines(text);
    }
}

/// CR 706.2: the instruction may include modifiers that add to or subtract from the natural
/// result; a modifier is part of the roll (706.3b), a number and never mana.
#[test]
fn r706_2_modifiers_in_the_instruction() {
    use ir::action::RollModifier as M;
    // the modifier, read and printed back
    let modifier = |text: &str| {
        let (s, printed) = read(text);
        assert_eq!(printed, text);
        let m = roll(&s)["modifier"].clone();
        assert!(!m.is_null(), "{text}: no modifier");
        m
    };
    let added = modifier("Roll a d20 and add the number of cards in your hand")["Add"].clone();
    assert!(!added.is_null());
    assert_eq!(modifier("Roll a d20 and subtract the number of cards in your hand")["Subtract"], added);
    assert!(!modifier("Target player rolls a d20 and adds the number of cards in their hand")["Add"].is_null());
    for text in [
        "Exile target spell, then roll a d20 and add that spell's mana value",
        "Roll a d20 and add the greatest power among creatures you control",
        "Roll two d6 and subtract the number of creatures you control",
    ] {
        modifier(text);
    }
    let d20 = |modifier| Action::RollDie { count: Amount::Const(1), sides: 20, modifier: Some(modifier), kept: None };
    rolls("Roll a d20 and add X", "Roll a d20 and add X", d20(M::Add(Amount::X)));
    // mana is added as a second act: the roll has no modifier
    for text in ["Roll a d20 and add {R}", "Roll a d20 and add one mana of any color"] {
        let (s, printed) = read(text);
        assert_eq!(printed, text);
        assert!(roll(&s)["modifier"].is_null(), "{text}");
        assert!(serde_json::to_string(&s).unwrap().contains("AddMana"), "{text}");
    }
    for text in ["Roll a d20 and subtract {R}", "Roll a d20 and add", "Roll a d20 and subtract target creature"] {
        declines(text);
    }
}

/// CR 706.6: an ignored roll is considered never to have happened. "The lower roll" is one of
/// two; "all but the highest roll" leaves one of several; of two dice both keep the higher.
#[test]
fn r706_6_ignored_rolls() {
    use ir::action::KeptRolls;
    let highest = Some(KeptRolls { highest: true, count: Amount::Const(1) });
    let lowest = Some(KeptRolls { highest: false, count: Amount::Const(1) });
    let d20s = |count, kept| Action::RollDie { count, sides: 20, modifier: None, kept };
    for (text, printed, expected) in [
        ("Roll two d20 and ignore the lower roll", "Roll two d20 and ignore the lower roll", d20s(Amount::Const(2), highest.clone())),
        ("Roll two d20 and ignore the higher roll", "Roll two d20 and ignore the higher roll", d20s(Amount::Const(2), lowest.clone())),
        ("Roll two d20 and ignore all but the highest roll", "Roll two d20 and ignore the lower roll", d20s(Amount::Const(2), highest.clone())),
        ("Roll three d20 and ignore all but the highest roll", "Roll three d20 and ignore all but the highest roll", d20s(Amount::Const(3), highest.clone())),
        ("Roll X d20 and ignore all but the lowest roll", "Roll X d20 and ignore all but the lowest roll", d20s(Amount::X, lowest)),
    ] {
        rolls(text, printed, expected);
    }
    let (s, printed) = read("Target player rolls two d20 and ignores the lower roll");
    assert_eq!(printed, "Target player rolls two d20 and ignores the lower roll");
    assert_eq!(roll(&s)["kept"], serde_json::to_value(&highest).unwrap());
    // "the lower" is one of two; no roll of one die is ignored
    for text in [
        "Roll a d20 and ignore the lower roll",
        "Roll three d20 and ignore the lower roll",
        "Roll X d20 and ignore the lower roll",
        "Roll a d20 and ignore all but the highest roll",
        "Roll two d20 and ignore the lowest roll",
    ] {
        declines(text);
    }
}

/// CR 705.2: the player who flips a coin and calls it wins or loses the flip; losing is an event
/// of its own, never losing the game.
#[test]
fn r705_2_losing_a_coin_flip() {
    let a = &grammar::ability::abilities().ability;
    let deed = |text: &str| {
        let Outcome::Accepted { value, .. } = a.parse_all(&tokenize(text)) else { panic!("{text} must read") };
        assert_eq!(a.print_text(&value, true).as_deref(), Some(text));
        serde_json::to_value(&value).unwrap()["Triggered"]["trigger"]["Event"]["pattern"]["actions"][0]["Deed"]["verb"].clone()
    };
    assert_eq!(deed("Whenever you lose a coin flip, ~ deals 1 damage to any target."), "LoseFlip");
    assert_eq!(deed("Whenever an opponent loses a coin flip, draw a card."), "LoseFlip");
    assert_eq!(deed("Whenever you win a coin flip, draw a card."), "WinFlip");
    assert_eq!(deed("Whenever you lose the game, draw a card."), "LoseGame");
    for text in ["Whenever you lose a flip, draw a card.", "Whenever you lose a coin, draw a card."] {
        assert!(matches!(a.parse_all(&tokenize(text)), Outcome::Declined { .. }), "{text} must not read");
    }
}

/// CR 706.2: the natural result is the number the die shows before modifiers, the result the
/// number after them; a trigger may watch for either. A die's highest natural result is its
/// number of sides (706.1a), and no die shows 0.
#[test]
fn r706_2_watching_for_a_result() {
    use ir::event::{Quantity, RollResult, RollValue};
    let a = &grammar::ability::abilities().ability;
    let result = |text: &str| {
        let Outcome::Accepted { value, .. } = a.parse_all(&tokenize(text)) else { panic!("{text} must read") };
        assert_eq!(a.print_text(&value, true).as_deref(), Some(text));
        let deed = serde_json::to_value(&value).unwrap()["Triggered"]["trigger"]["Event"]["pattern"]["actions"][0]["Deed"].clone();
        assert_eq!(deed["verb"], "RollDice", "{text}");
        assert_eq!(deed["args"][0][0], "Result", "{text}");
        deed["args"][0][1]["Roll"].clone()
    };
    let is = |natural, value| serde_json::to_value(RollResult { natural, value }).unwrap();
    let n = |q| RollValue::Number(q);
    assert_eq!(
        result("When you roll a natural 20, return ~ from your graveyard to your hand."),
        is(true, n(Quantity::Exactly(Amount::Const(20))))
    );
    assert_eq!(result("Whenever you roll a 4 or higher, draw a card."), is(false, n(Quantity::AtLeast(Amount::Const(4)))));
    assert_eq!(result("Whenever an opponent rolls an 8 or higher, draw a card."), is(false, n(Quantity::AtLeast(Amount::Const(8)))));
    assert_eq!(result("Whenever you roll a die's highest natural result, draw a card."), is(true, RollValue::Highest));
    for text in [
        "Whenever you roll a natural 0, draw a card.",
        "Whenever you roll a 8, draw a card.",
        "Whenever you roll an natural 8, draw a card.",
        "Whenever you roll a natural, draw a card.",
    ] {
        assert!(matches!(a.parse_all(&tokenize(text)), Outcome::Declined { .. }), "{text} must not read");
    }
}
