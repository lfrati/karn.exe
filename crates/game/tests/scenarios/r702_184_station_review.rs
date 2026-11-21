//! Station (702.184) and station symbols (721).
//!
//! 702.184a: "Station" means "Tap another untapped creature you control: Put a number of
//! charge counters on this permanent equal to the tapped creature's power. Activate only as a
//! sorcery." Rulings (2025-07-25): a station permanent's abilities and whether it is a creature
//! follow its charge counters, also when it loses them; with negative power no counters are
//! put or removed; station can be activated past the last symbol.
use crate::support::cite;
use crate::support;
use support::*;

fn game(names: &[&str], extra: Vec<CardDef>) -> T {
    let mut defs: Vec<CardDef> = ["Wastes"].iter().chain(names).map(|n| oracle(n)).collect();
    defs.extend(extra);
    for d in &defs { game::play::check(d).unwrap_or_else(|e| panic!("{}: {}", d.name(), e.0)); }
    let mut g = Game::new(defs, 2, 20);
    for p in 0..2 { for _ in 0..10 { g.put(p, 0, Zone::Library); } }
    T::new(g)
}
fn put(t: &mut T, p: usize, name: &str, zone: Zone) -> ObjId {
    let index = t.g.defs.iter().position(|d| d.name() == name).unwrap_or_else(|| panic!("no {name}"));
    t.g.put(p, index, zone)
}
/// Identify the printed Station ability and its executable meaning.

fn station_ability(t: &T, card: ObjId) -> usize {
    let def = t.g.def(card).unwrap();
    let row = def.abilities().iter().position(|a| matches!(a,
        ir::Ability::Keywords(items) if items.iter().any(|i| i.keyword.name() == "Station")))
        .expect("the card's printed Station row");
    t.g.abilities(card).iter().enumerate().find_map(|(index, ability)|
        (def.ability_row(index) == row && matches!(&**ability, ir::Ability::Activated { .. })).then_some(index))
        .expect("the executable Station meaning of that row")
}
fn station(t: &mut T, card: ObjId, creature: ObjId) {
    let ability = station_ability(t, card);
    t.g.act(0, Move::Activate { source: card, ability, targets: vec![], paying: vec![vec![creature]] }).unwrap();
    while !t.g.stack.is_empty() { t.g.act(t.g.priority, Move::Pass).unwrap(); }
}
fn charge(t: &T, o: ObjId) -> u32 { t.g.obj(o).counters.get("charge").copied().unwrap_or(0) }
fn creature(t: &T, o: ObjId) -> bool { t.g.is_a(o, ir::CardType::Creature) }

// Rules: 702.184a, 302.6, 602.5a
// Given: a Scanship (itself tapped) and a Bears cast this turn.
// When: station is activated tapping the summoning-sick Bears.
// Expect: it is offered and resolves for 2 counters.
// Why: station's cost taps another creature, not {T} of the creature with the ability, so
// summoning sickness does not restrict it (302.6 covers only {T}/{Q} abilities of the sick
// creature itself); the Spacecraft need not be untapped since it is not tapped for the cost.
#[test]
fn r702_184a_a_summoning_sick_creature_may_be_tapped_for_station_of_a_tapped_spacecraft() {
    let mut t = game(&["Uthros Scanship", "Grizzly Bears"], vec![]);
    t.to_no_combat(0, Part::PrecombatMain);
    let ship = put(&mut t, 0, "Uthros Scanship", Zone::Battlefield);
    let bears = put(&mut t, 0, "Grizzly Bears", Zone::Hand);
    t.mana(0, "{1}{G}");
    t.g.act(0, crate::support::paid("CG",Move::Cast { card: bears, targets: vec![] })).unwrap();
    while !t.g.stack.is_empty() { t.g.act(t.g.priority, Move::Pass).unwrap(); }
    let bears = t.find_in_battlefield("Grizzly Bears");
    assert!(t.g.summoning_sick(bears), "setup: the Bears entered this turn");
    t.g.tap(ship);
    station(&mut t, ship, bears);
    cite!(["702.184a"]; assert_eq!((charge(&t, ship), t.g.obj(bears).tapped), (2, true)));
}

// Rules: 702.184a, 107.1b
// Given: an Ornithopter (0/2) and a Bears given -3/-0 (power -1).
// When: each is tapped for station.
// Expect: both activations are legal and tap the creature; no charge counters are put.
// Why: the cost is only tapping an untapped creature; "a number equal to its power" of 0 or a
// negative number puts none (ruling: "If the tapped creature has negative power, no charge
// counters are put onto or removed from the permanent with station").
#[test]
fn r702_184a_zero_or_negative_power_puts_no_counters_and_removes_none() {
    let weaken = CardDef::new("Weaken", "{0}", "Instant", "Target creature gets -3/-0 until end of turn.", None).unwrap();
    let mut t = game(&["Uthros Scanship", "Ornithopter", "Grizzly Bears"], vec![weaken]);
    let ship = put(&mut t, 0, "Uthros Scanship", Zone::Battlefield);
    let thopter = put(&mut t, 0, "Ornithopter", Zone::Battlefield);
    let bears = put(&mut t, 0, "Grizzly Bears", Zone::Battlefield);
    t.to_no_combat(0, Part::PrecombatMain);
    t.g.put_counters(ship, "charge", 3);
    station(&mut t, ship, thopter);
    cite!(["702.184a"]; case: "power 0"; assert_eq!((charge(&t, ship), t.g.obj(thopter).tapped), (3, true)));
    let weaken = put(&mut t, 0, "Weaken", Zone::Hand);
    t.g.act(0, Move::Cast { card: weaken, targets: vec![vec![Target::Object(bears)]] }).unwrap();
    while !t.g.stack.is_empty() { t.g.act(t.g.priority, Move::Pass).unwrap(); }
    assert_eq!(t.g.power(bears), Some(-1));
    station(&mut t, ship, bears);
    cite!(["702.184a", "107.1b"]; case: "power -1"; assert_eq!((charge(&t, ship), t.g.obj(bears).tapped), (3, true)));
}

// Rules: 721.2b, 721.2a, 122.1
// Given: a Scanship stationed to 8 (an artifact creature 4/4 with flying).
// When: one charge counter is removed, then station brings it back to 9 and more.
// Expect: at 7 it is no creature, has no flying and no power; back at 9 it is again; station
// is still offered past the last symbol.
// Why: 721.2b is "as long as this permanent has N or more charge counters"; the ruling says
// its abilities and being a creature change when it loses counters, and that station may be
// activated past its last symbol.
#[test]
fn r721_2b_losing_charge_counters_below_the_symbol_ends_the_creature_and_its_abilities() {
    let mut t = game(&["Uthros Scanship", "Grizzly Bears", "Hill Giant"], vec![]);
    let ship = put(&mut t, 0, "Uthros Scanship", Zone::Battlefield);
    let bears = put(&mut t, 0, "Grizzly Bears", Zone::Battlefield);
    let giant = put(&mut t, 0, "Hill Giant", Zone::Battlefield);
    t.to_no_combat(0, Part::PrecombatMain);
    t.g.put_counters(ship, "charge", 6);
    station(&mut t, ship, bears);
    assert!(creature(&t, ship) && t.g.has(ship, "Flying"));
    t.g.remove_counters(ship, "charge", 1);
    cite!(["721.2b"]; assert!(!creature(&t, ship), "7 counters: not a creature"));
    cite!(["721.2b"]; assert_eq!((t.g.power(ship), t.g.toughness(ship)), (None, None)));
    cite!(["721.2a"]; assert!(!t.g.has(ship, "Flying")));
    cite!(["721.2b"]; assert!(t.g.is_a(ship, ir::CardType::Artifact)));
    station(&mut t, ship, giant);
    cite!(["702.184a"]; assert_eq!(charge(&t, ship), 10));
    cite!(["721.2b"]; assert!(creature(&t, ship) && t.g.has(ship, "Flying") && t.g.power(ship) == Some(4)));
}

// Rules: 721.2a, 613.1f
// Given: a Lumen-Class Frigate at 2 counters ("2+ | Other creatures you control get +1/+1").
// When: its counters drop to 1.
// Expect: the anthem stops applying to the Bears.
// Why: the lower symbol is also "as long as"; its static ability is lost below 2.
#[test]
fn r721_2a_a_lower_symbols_static_ability_ends_below_its_threshold() {
    let mut t = game(&["Lumen-Class Frigate", "Grizzly Bears"], vec![]);
    let frigate = put(&mut t, 0, "Lumen-Class Frigate", Zone::Battlefield);
    let bears = put(&mut t, 0, "Grizzly Bears", Zone::Battlefield);
    t.to_no_combat(0, Part::PrecombatMain);
    station(&mut t, frigate, bears);
    cite!(["702.184a", "721.2a"]; assert_eq!((charge(&t, frigate), t.g.power(bears)), (2, Some(3))));
    t.g.remove_counters(frigate, "charge", 1);
    cite!(["721.2a"]; assert_eq!((t.g.power(bears), t.g.toughness(bears)), (Some(2), Some(2))));
}
