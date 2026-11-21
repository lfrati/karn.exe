//! The worked examples of rule 704 (state-based actions), with real cards.
use crate::support;
use support::*;

/// 704.8: Young Wolf (undying) has one +1/+1 counter and three -1/-1 counters before
/// state-based actions. They're performed simultaneously: it dies as it last existed, with a
/// +1/+1 counter, so undying doesn't trigger. (The example's spell that puts the -1/-1
/// counters isn't one the engine runs; the board starts as the example says it is before
/// state-based actions.)
#[test]
fn r704_8_example_young_wolf_dies_with_a_counter_so_undying_does_not_return_it() {
    let mut t = Board::new(2).battlefield(0, &["Young Wolf"]).counters("Young Wolf", "+1/+1", 1).counters("Young Wolf", "-1/-1", 3).build();
    t.g.state_based_actions();
    t.expect().rules(["704.5f"]).zone(0, Zone::Battlefield, &[]).zone(0, Zone::Graveyard, &["Young Wolf"]).rules(["704.8", "702.93a"]).stack(&[]).check();
    t.pass_all();
    t.expect().rules(["704.8", "702.93a"]).zone(0, Zone::Battlefield, &[]).zone(0, Zone::Graveyard, &["Young Wolf"]).check();
}
