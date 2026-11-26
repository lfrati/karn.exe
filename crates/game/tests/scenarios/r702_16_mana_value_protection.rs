//! Protection from a mana value (702.16a): "protection from mana value 3 or greater" is a
//! quality of sources wherever they are; a spell's mana value counts the X chosen for it
//! (202.3e, 107.3). Every card is read from its Oracle text.
use crate::support::{cite, mana_pool, oracle, paid};
use game::{Game, Move, Target, Zone};

const SKULK: usize = 0;
const TITAN: usize = 1;
const BLAZE: usize = 2;
const BEARS: usize = 3;
const GIANT: usize = 4;
const SHOCK: usize = 5;
const SHATTER: usize = 6;

/// Mistmeadow Skulk ("Lifelink, protection from mana value 3 or greater", 1/1), Reaver Titan
/// ("Protection from mana value 3 or less"), Blaze ({X}{R}), Grizzly Bears (2), Hill Giant (4), Shock (1)
/// and Shatter ("Destroy target artifact.", 2).
fn setup() -> Game {
    let defs: Vec<_> = ["Mistmeadow Skulk", "Reaver Titan", "Blaze", "Grizzly Bears", "Hill Giant", "Shock", "Shatter", "Mountain"]
        .into_iter().map(oracle).collect();
    let mut g = Game::new(defs, 2, 20);
    for p in 0..2 { for _ in 0..5 { g.put(p, 7, Zone::Library); } }
    while g.part != ir::Part::PrecombatMain { g.act(g.priority, Move::Pass).unwrap(); }
    g
}

#[test]
fn the_cards_of_these_tests_run() {
    for name in ["Mistmeadow Skulk", "Reaver Titan"] {
        // Uncited (admission): the engine runs each card of these tests.
        assert_eq!(game::play::check(&oracle(name)).err().map(|u| u.0), None, "{name}");
    }
}

#[test]
fn r702_16a_a_spell_mana_value_counts_its_x() {
    // Ruling (Mistmeadow Skulk): "Mistmeadow Skulk can be chosen as a target for a Blaze ... if X
    // is 0 or 1, for example, but it can't be chosen as a target for a Blaze if X is 2 or more."
    let mut g = setup();
    let skulk = g.put(1, SKULK, Zone::Battlefield);
    let blaze = g.put(0, BLAZE, Zone::Hand);
    g.players[0].pool = mana_pool("RRR");
    let said = g.act(0, paid("RRR", Move::WithX(2, Box::new(Move::Cast { card: blaze, targets: vec![vec![Target::Object(skulk)]] }))));
    cite!(["702.16b", "202.3e"]; assert_eq!(said.err().as_deref(), Some("an illegal target, Mistmeadow Skulk: it has protection from that (702.16b)")));
    g.players[0].pool = mana_pool("RR");
    g.act(0, paid("RR", Move::WithX(1, Box::new(Move::Cast { card: blaze, targets: vec![vec![Target::Object(skulk)]] })))).unwrap();
    for _ in 0..2 { g.act(g.priority, Move::Pass).unwrap(); }
    cite!(["702.16b", "202.3e"]; assert!(!g.battlefield.contains(&skulk), "a 1/1 dealt 1 damage dies"));
}

#[test]
fn r702_16e_damage_from_a_source_of_that_mana_value_is_prevented() {
    let mut g = setup();
    let skulk = g.put(1, SKULK, Zone::Battlefield);
    let giant = g.put(0, GIANT, Zone::Battlefield);
    g.damage(giant, Target::Object(skulk), 1);
    cite!(["702.16e"]; assert_eq!(g.obj(skulk).damage, 0, "Hill Giant has mana value 4"));
    let shock = g.put(0, SHOCK, Zone::Battlefield);
    g.damage(shock, Target::Object(skulk), 1);
    cite!(["702.16e"]; assert_eq!(g.obj(skulk).damage, 1, "Shock has mana value 1"));
}

#[test]
fn r702_16f_it_cant_be_blocked_by_a_creature_of_that_mana_value() {
    let mut g = setup();
    let skulk = g.put(0, SKULK, Zone::Battlefield);
    let titan = g.put(0, TITAN, Zone::Battlefield);
    let giant = g.put(1, GIANT, Zone::Battlefield);
    let bears = g.put(1, BEARS, Zone::Battlefield);
    cite!(["702.16f"]; assert!(g.cant_block(giant, skulk, &[giant]) && !g.cant_block(bears, skulk, &[bears])));
    cite!(["702.16f"]; assert!(g.cant_block(bears, titan, &[bears]) && !g.cant_block(giant, titan, &[giant])));
}

#[test]
fn r702_16b_protection_from_mana_value_3_or_less() {
    // Reaver Titan, an artifact, can't be targeted by Shatter (mana value 2)
    let mut g = setup();
    let titan = g.put(1, TITAN, Zone::Battlefield);
    let shatter = g.put(0, SHATTER, Zone::Hand);
    g.players[0].pool = mana_pool("RR");
    let said = g.act(0, paid("RR", Move::Cast { card: shatter, targets: vec![vec![Target::Object(titan)]] }));
    cite!(["702.16b"]; assert_eq!(said.err().as_deref(), Some("an illegal target, Reaver Titan: it has protection from that (702.16b)")));
}
