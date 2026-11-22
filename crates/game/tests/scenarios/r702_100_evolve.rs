//! Evolve compares both current creature characteristics at both intervening-if checks.
use crate::support::{cite, mana_pool, oracle, paid};
use game::{CardDef, Game, Move, Target, Zone};
fn pass(g: &mut Game) { for _ in 0..g.players.len() { g.act(g.priority, Move::Pass).unwrap(); } }
#[test]
fn r702_100a_power_or_toughness_suffices_but_equal_characteristics_do_not() {
    for (name, expected) in [("Shambleshark", true), ("Crocanura", true), ("Adaptive Snapjaw", false)] {
        let mut g = Game::new(vec![oracle(name), oracle("Grizzly Bears")], 2, 20);
        let source = g.put(0, 0, Zone::Battlefield);
            let bear = g.put(0, 1, Zone::Hand); g.players[0].pool = mana_pool("GG");
        g.act(0, paid("GG", Move::Cast { card: bear, targets: vec![] })).unwrap(); pass(&mut g);
        cite!(["702.100a", "603.4"]; case: name; assert_eq!(g.stack.len(), usize::from(expected)));
        if expected { pass(&mut g); }
        cite!(["702.100a"]; case: name; assert_eq!(g.obj(source).counters.get("+1/+1").copied().unwrap_or(0), u32::from(expected)));
    }
}
#[test]
fn r603_4_evolve_rechecks_after_a_response_changes_its_sources_power_and_toughness() {
    let mut g = Game::new(vec![oracle("Shambleshark"), oracle("Grizzly Bears"), oracle("Giant Growth")], 2, 20);
    let source = g.put(0, 0, Zone::Battlefield);
    let bear = g.put(0, 1, Zone::Hand); g.players[0].pool = mana_pool("GG");
    g.act(0, paid("GG", Move::Cast { card: bear, targets: vec![] })).unwrap(); pass(&mut g);
    let growth = g.put(0, 2, Zone::Hand); g.players[0].pool = mana_pool("G");
    g.act(0, paid("G", Move::Cast { card: growth, targets: vec![vec![Target::Object(source)]] })).unwrap();
    pass(&mut g); pass(&mut g);
    cite!(["603.4", "702.100a"]; assert!(g.obj(source).counters.is_empty()));
}
#[test]
fn r702_100a_only_own_entries_trigger_and_departed_creatures_keep_last_known_characteristics() {
    let mut g = Game::new(vec![oracle("Crocanura"), oracle("Grizzly Bears")], 2, 20);
    let source = g.put(0, 0, Zone::Battlefield);
    g.active = 1; g.act(0, Move::Pass).unwrap();
    let bear = g.put(1, 1, Zone::Hand); g.players[1].pool = mana_pool("GG");
    g.act(1, paid("GG", Move::Cast { card: bear, targets: vec![] })).unwrap(); pass(&mut g);
    cite!(["702.100a"]; assert!(g.stack.is_empty()));
    g.active = 0;
    if g.priority != 0 { g.act(g.priority, Move::Pass).unwrap(); }
    let own = g.put(0, 1, Zone::Hand); g.players[0].pool = mana_pool("GG");
    g.act(0, paid("GG", Move::Cast { card: own, targets: vec![] })).unwrap(); pass(&mut g);
    let entered = *g.battlefield.last().unwrap(); g.destroy(entered);
    pass(&mut g);
    cite!(["608.2h", "702.100a"]; assert_eq!(g.obj(source).counters.get("+1/+1"), Some(&1)));
}
#[test]
fn r702_100c_a_noncreature_with_evolve_cannot_be_outgrown() {
    let def = CardDef::new("Inactive evolver", "{0}", "Artifact", "Evolve", None).unwrap();
    game::play::check(&def).unwrap();
    let mut g = Game::new(vec![def, oracle("Grizzly Bears")], 2, 20);
    let source = g.put(0, 0, Zone::Battlefield);
    let bear = g.put(0, 1, Zone::Hand); g.players[0].pool = mana_pool("GG");
    g.act(0, paid("GG", Move::Cast { card: bear, targets: vec![] })).unwrap(); pass(&mut g);
    cite!(["702.100c"]; assert!(g.stack.is_empty() && g.obj(source).counters.is_empty()));
}
#[test]
fn r702_100a_granted_evolve_compares_its_recipient_and_each_instance_triggers_separately() {
    let mut g = Game::new(vec![oracle("Tyranid Prime"), oracle("Grizzly Bears"), oracle("Hill Giant")], 2, 20);
    let prime = g.put(0, 0, Zone::Battlefield);
    let bear = g.put(0, 1, Zone::Battlefield);
    let giant = g.put(0, 2, Zone::Hand); g.players[0].pool = mana_pool("RRRR");
    g.act(0, paid("RRRR", Move::Cast { card: giant, targets: vec![] })).unwrap(); pass(&mut g);
    g.act(0, Move::Order(vec![0, 1])).unwrap();
    pass(&mut g); pass(&mut g);
    cite!(["702.100a", "613.1f"]; assert_eq!(g.obj(prime).counters.get("+1/+1"), Some(&1)));
    cite!(["702.100a", "613.1f"]; assert_eq!(g.obj(bear).counters.get("+1/+1"), Some(&1)));
}
#[test]
fn evolve_event_consumers_remain_explicitly_refused() {
    // Uncited (admission): evolve event receipts need a successful counter producer hook.
    let def = CardDef::new("Unsupported evolve watcher", "{0}", "Creature", "Evolve\nWhenever this creature evolves, you gain 1 life.", Some((1, 1))).unwrap();
    crate::support::refused(game::play::check(&def).map_err(|e| e.0), "trigger: Evolve");
}
