//! The worked examples of rule 611 (continuous effects), with real cards.
use crate::support;
use support::*;

/// 611.3c: with "White creatures get +1/+1" (Crusade) on the battlefield, a spell that creates
/// 1/1 white creatures (Raise the Alarm) creates 2/2s: a static ability applies as they enter.
#[test]
fn r611_3c_example_crusade_tokens_enter_as_two_two() {
    let mut t = Board::new(2).battlefield(0, &["Crusade"]).hand(0, &["Raise the Alarm"]).build();
    t.mana(0, "{1}{W}").cast(0, "Raise the Alarm", vec![], crate::support::mana_pool("CW")).unwrap();
    t.resolve();
    let soldiers: Vec<_> = t.g.battlefield.iter().copied().filter(|o| t.g.obj(*o).token()).collect();
    assert_eq!(soldiers.len(), 2);
    for s in soldiers { cite!(["611.3c"]; assert_eq!(t.g.seen(s, None).pt, Some((2, 2)))); }
}
