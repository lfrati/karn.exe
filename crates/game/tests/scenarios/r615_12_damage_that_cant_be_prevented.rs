//! "Pinpoint Avalanche deals 4 damage to target creature. The damage can't be prevented.": the
//! damage the act deals can't be prevented, and no prevention effect applies to it (615.12).
//! It is said of that damage only: other damage is prevented as usual. Said of an act done
//! instead ("If this spell was kicked, instead it deals 10 damage to that permanent or player
//! and the damage can't be prevented"), it is said only when that act is done.
use crate::support;
use support::*;

/// Pass until the stack is empty.
fn resolve(t: &mut T) {
    for _ in 0..8 {
        if t.g.stack.is_empty() { return; }
        t.pass_all();
    }
}

/// Player 1's `creature`, enchanted by player 0's Inviolability ("Prevent all damage that would
/// be dealt to enchanted creature."), and player 0's hand.
fn shielded(creature: &str, hand: &[&str]) -> (T, ObjId) { shielded_with(creature, hand, &[]) }

/// [`shielded`], with these cards in player 0's graveyard.
fn shielded_with(creature: &str, hand: &[&str], graveyard: &[&str]) -> (T, ObjId) {
    let mut hand = hand.to_vec();
    hand.push("Inviolability");
    let mut t = Board::new(2).hand(0, &hand).battlefield(1, &[creature]).graveyard(0, graveyard).fill(10).build();
    let target = t.find_in_battlefield(creature);
    t.mana(0, "{1}{W}").cast(0, "Inviolability", vec![Tgt::Object(target)], mana_pool("CW")).unwrap();
    resolve(&mut t);
    (t, target)
}

/// Cast `spell` from player 0's hand at `target`, kicked or not, and resolve it.
fn cast(t: &mut T, spell: &str, cost: &str, pool: &str, kicked: bool, target: Target) {
    let card = t.find_in(0, Zone::Hand, spell);
    t.mana(0, cost);
    t.g.act(0, Move::Paying(mana_pool(pool), Box::new(Move::WithKicks(u32::from(kicked),
        Box::new(Move::Cast { card, targets: vec![vec![target]] }))))).unwrap();
    resolve(t);
}

// Rules: 615.12, 615.1
// Given: player 1's Hill Giant (3/3) enchanted with Inviolability.
// When: player 0 casts Lightning Bolt, or Pinpoint Avalanche, at it.
// Expect: Lightning Bolt's 3 damage is prevented and the Giant lives; Pinpoint Avalanche's 4
// damage can't be prevented, and the Giant dies.
#[test]
fn r615_12_pinpoint_avalanches_damage_is_dealt_through_a_prevention_effect() {
    for (spell, cost, pool, dies) in [("Lightning Bolt", "{R}", "R", false), ("Pinpoint Avalanche", "{3}{R}{R}", "CCCRR", true)] {
        let (mut t, giant) = shielded("Hill Giant", &[spell]);
        cast(&mut t, spell, cost, pool, false, Target::Object(giant));
        cite!(["615.12", "615.1"]; case: spell; assert_eq!(t.g.battlefield.contains(&giant), !dies));
    }
}

// Rules: 615.12, 702.33d, 608.2c
// Given: player 1's Craw Wurm (6/4) enchanted with Inviolability.
// When: player 0 casts Urza's Rage at it, kicked or not.
// Expect: unkicked, its 3 damage is prevented and the Wurm lives; kicked, its 10 damage can't
// be prevented and the Wurm dies.
#[test]
fn r615_12_urzas_rage_kicked_deals_damage_that_cant_be_prevented() {
    for (kicked, dies) in [(false, false), (true, true)] {
        let (mut t, wurm) = shielded("Craw Wurm", &["Urza's Rage"]);
        let (cost, pool) = if kicked { ("{10}{R}{R}", "CCCCCCCCCCRR") } else { ("{2}{R}", "CCR") };
        cast(&mut t, "Urza's Rage", cost, pool, kicked, Target::Object(wurm));
        cite!(["615.12", "702.33d"]; case: format!("kicked {kicked}"); assert_eq!(t.g.battlefield.contains(&wurm), !dies));
    }
}

// Rules: 120.3a, 702.33d
// Given: Urza's Rage.
// When: player 0 casts it kicked at player 1.
// Expect: it deals 10 damage to that player, not 3.
#[test]
fn r615_12_urzas_rage_kicked_deals_ten_to_a_player() {
    let mut t = Board::new(2).hand(0, &["Urza's Rage"]).build();
    cast(&mut t, "Urza's Rage", "{10}{R}{R}", "CCCCCCCCCCRR", true, Target::Player(1));
    cite!(["120.3a", "702.33d"]; assert_eq!(t.g.players[1].life, 10));
}

// Rules: 615.12, 207.2c
// Given: player 1's Craw Wurm (6/4) enchanted with Inviolability; player 0's graveyard holds six
// or seven cards.
// When: player 0 casts Lightning Surge at it.
// Expect: with six, its 4 damage is prevented and the Wurm lives; with seven (threshold), its 6
// damage can't be prevented and the Wurm dies.
#[test]
fn r615_12_lightning_surge_with_threshold_deals_damage_that_cant_be_prevented() {
    for (cards, dies) in [(6, false), (7, true)] {
        let (mut t, wurm) = shielded_with("Craw Wurm", &["Lightning Surge"], &vec!["Forest"; cards]);
        cast(&mut t, "Lightning Surge", "{3}{R}{R}", "CCCRR", false, Target::Object(wurm));
        cite!(["615.12", "207.2c"]; case: format!("{cards} cards"); assert_eq!(t.g.battlefield.contains(&wurm), !dies));
    }
}

// Rules: 615.12, 101.6
// Given: player 1's Serra Angel (4/4) enchanted with Inviolability.
// When: player 0 casts Combust at it.
// Expect: its 5 damage can't be prevented, and the Angel dies.
#[test]
fn r615_12_combust_deals_damage_that_cant_be_prevented() {
    let (mut t, angel) = shielded("Serra Angel", &["Combust"]);
    cast(&mut t, "Combust", "{1}{R}", "CR", false, Target::Object(angel));
    cite!(["615.12"]; assert!(!t.g.battlefield.contains(&angel)));
}
