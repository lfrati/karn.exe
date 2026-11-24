//! Review of "the damage can't be prevented" (615.12), for what its own tests don't check: raid
//! as the condition, threshold counted with the spell cast by flashback (its ruling), a
//! replacement effect that isn't prevention, and a prevention shield the damage doesn't use up.
//! Every card is read from its Oracle text.
use crate::support;
use game::announcement::CastRoute;
use game::state::Decision;
use support::*;

#[test]
fn the_cards_of_these_tests_run() {
    for name in ["Arrow Storm", "Lightning Surge", "Pinpoint Avalanche", "Urza's Rage", "Inviolability", "Furnace of Rath", "Mending Hands",
        "Lightning Bolt", "Craw Wurm", "Colossal Dreadmaw", "Grizzly Bears", "Mountain"] {
        // Uncited (admission): the engine runs each card of these tests.
        assert_eq!(game::play::check(&oracle(name)).err().map(|u| u.0), None, "{name}");
    }
}

/// Pass until the stack is empty.
fn resolve(t: &mut T) {
    for _ in 0..8 {
        if t.g.stack.is_empty() { return; }
        t.pass_all();
    }
}

/// Player 1's `creature`, enchanted by player 0's Inviolability ("Prevent all damage that would
/// be dealt to enchanted creature."); player 0's hand, graveyard and battlefield.
fn shielded(creature: &str, hand: &[&str], graveyard: &[&str], battlefield: &[&str]) -> (T, ObjId) {
    let mut hand = hand.to_vec();
    hand.push("Inviolability");
    let mut t = Board::new(2).hand(0, &hand).battlefield(1, &[creature]).battlefield(0, battlefield).graveyard(0, graveyard).fill(10).build();
    let target = t.find_in_battlefield(creature);
    t.mana(0, "{1}{W}").cast(0, "Inviolability", vec![Tgt::Object(target)], mana_pool("CW")).unwrap();
    resolve(&mut t);
    (t, target)
}

/// Cast `spell`, from player 0's hand or by `route`, at `target`, paying `pool`, and resolve it.
fn cast(t: &mut T, spell: &str, zone: Zone, route: Option<CastRoute>, pool: &str, target: Target) {
    let card = t.find_in(0, zone, spell);
    t.g.players[0].pool = mana_pool(pool);
    let cast = Move::Cast { card, targets: vec![vec![target]] };
    let cast = match route { Some(route) => Move::CastVia(route, Box::new(cast)), None => cast };
    t.g.act(0, paid(pool, cast)).unwrap();
    resolve(t);
}

// Rules: 615.12, 207.2c, 508.1
// Given: player 1's Craw Wurm (6/4) enchanted with Inviolability; player 0's Grizzly Bears.
// When: player 0 casts Arrow Storm at the Wurm in their first main phase, or after the Bears
// attacked.
// Expect: without raid its 4 damage is prevented and the Wurm lives; with raid its 5 damage
// can't be prevented and the Wurm dies.
#[test]
fn r615_12_arrow_storm_with_raid_deals_damage_that_cant_be_prevented() {
    // Arrow Storm: "Arrow Storm deals 4 damage to any target. / Raid — If you attacked this turn,
    // instead Arrow Storm deals 5 damage to that permanent or player and the damage can't be
    // prevented."
    for raid in [false, true] {
        let (mut t, wurm) = shielded("Craw Wurm", &["Arrow Storm"], &[], &["Grizzly Bears"]);
        if raid {
            let bears = t.find_in_battlefield("Grizzly Bears");
            for _ in 0..40 {
                if matches!(t.g.decision, Some(Decision::Attackers { .. })) { break; }
                let p = t.g.decision_maker(t.g.priority);
                t.g.act(p, Move::Pass).unwrap();
            }
            t.g.act(0, Move::Attack(vec![(bears, Target::Player(1))])).unwrap();
            for _ in 0..40 {
                if t.g.part == Part::PostcombatMain { break; }
                match t.g.decision.clone() {
                    Some(Decision::Blockers { player }) => t.g.act(player, Move::Block(vec![])).unwrap(),
                    _ => { let p = t.g.decision_maker(t.g.priority); t.g.act(p, Move::Pass).unwrap() }
                }
            }
        }
        cast(&mut t, "Arrow Storm", Zone::Hand, None, "CCCRR", Target::Object(wurm));
        cite!(["615.12", "207.2c"]; case: format!("raid {raid}"); assert_eq!(t.g.battlefield.contains(&wurm), !raid));
    }
}

// Rules: 615.12, 207.2c, 702.34a
// Given: player 1's Craw Wurm (6/4) enchanted with Inviolability; Lightning Surge in player 0's
// graveyard, with six or seven other cards.
// When: player 0 casts Lightning Surge by flashback at the Wurm.
// Expect: it is on the stack, not in the graveyard, as it resolves (its ruling): with six others
// there is no threshold, its 4 damage is prevented and the Wurm lives; with seven its 6 damage
// can't be prevented and the Wurm dies. Either way it is exiled.
#[test]
fn r615_12_lightning_surge_cast_by_flashback_does_not_count_itself_for_threshold() {
    // Lightning Surge: "Lightning Surge deals 4 damage to any target. / Threshold — If there are
    // seven or more cards in your graveyard, instead Lightning Surge deals 6 damage to that
    // permanent or player and the damage can't be prevented. / Flashback {5}{R}{R}"
    for others in [6, 7] {
        let mut graveyard = vec!["Lightning Surge"];
        graveyard.extend(std::iter::repeat_n("Mountain", others));
        let (mut t, wurm) = shielded("Craw Wurm", &[], &graveyard, &[]);
        cast(&mut t, "Lightning Surge", Zone::Graveyard, Some(CastRoute::Flashback), "CCCCCRR", Target::Object(wurm));
        cite!(["615.12", "207.2c"]; case: format!("{others} other cards"); assert_eq!(t.g.battlefield.contains(&wurm), others < 7));
        cite!(["702.34a"]; case: format!("{others} other cards"); assert!(t.g.exile.iter().any(|o| t.g.obj(*o).chars.name == "Lightning Surge")));
    }
}

// Rules: 615.12, 614.1a, 615.1a
// Given: player 1's Colossal Dreadmaw (6/6) enchanted with Inviolability; Furnace of Rath on
// player 0's battlefield, or not.
// When: player 0 casts Pinpoint Avalanche at it.
// Expect: only prevention is skipped: Furnace of Rath, a replacement, still doubles the damage
// to 8 and the Dreadmaw dies; without it, it takes 4 and lives.
#[test]
fn r615_12_damage_that_cant_be_prevented_is_still_doubled() {
    // Furnace of Rath: "If a source would deal damage to a permanent or player, it deals double
    // that damage to that permanent or player instead."
    for furnace in [false, true] {
        let battlefield: &[&str] = if furnace { &["Furnace of Rath"] } else { &[] };
        let (mut t, dreadmaw) = shielded("Colossal Dreadmaw", &["Pinpoint Avalanche"], &[], battlefield);
        cast(&mut t, "Pinpoint Avalanche", Zone::Hand, None, "CCCRR", Target::Object(dreadmaw));
        let damage = if furnace { None } else { Some(4) };
        cite!(["615.12", "614.1a"]; case: format!("furnace {furnace}");
            assert_eq!(t.g.battlefield.contains(&dreadmaw).then(|| t.g.obj(dreadmaw).damage), damage));
    }
}

// Rules: 615.12, 615.1a, 702.33d
// Given: Mending Hands on player 1 ("Prevent the next 4 damage that would be dealt to any target
// this turn").
// When: player 0 casts Urza's Rage kicked at player 1, then Lightning Bolt.
// Expect: the 10 damage can't be prevented, and doesn't use up the shield: player 1 goes to 10,
// and Lightning Bolt's 3 damage is then prevented.
#[test]
fn r615_12_damage_that_cant_be_prevented_does_not_use_up_a_shield() {
    let mut t = Board::new(2).fill(14).hand(0, &["Mending Hands", "Urza's Rage", "Lightning Bolt"]).build();
    cast(&mut t, "Mending Hands", Zone::Hand, None, "W", Target::Player(1));
    let rage = t.find_in(0, Zone::Hand, "Urza's Rage");
    t.mana(0, "{10}{R}{R}");
    t.g.act(0, Move::Paying(mana_pool("CCCCCCCCCCRR"), Box::new(Move::WithKicks(1,
        Box::new(Move::Cast { card: rage, targets: vec![vec![Target::Player(1)]] }))))).unwrap();
    resolve(&mut t);
    cite!(["615.12", "702.33d"]; assert_eq!(t.g.players[1].life, 10));
    cast(&mut t, "Lightning Bolt", Zone::Hand, None, "R", Target::Player(1));
    cite!(["615.12", "615.1a"]; assert_eq!(t.g.players[1].life, 10));
}
