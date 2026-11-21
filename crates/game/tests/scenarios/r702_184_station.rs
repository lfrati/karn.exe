//! Station (702.184) and station cards (721).
//!
//! 702.184a: "Station" means "Tap another untapped creature you control: Put a number of
//! charge counters on this permanent equal to the tapped creature's power. Activate only as a
//! sorcery."
//!
//! 721.2a: "{N+}[abilities]" means "As long as this permanent has N or more charge counters on
//! it, it has [abilities]." 721.2b: "{N+}[abilities][P/T]" means "As long as this permanent has
//! N or more charge counters on it, it has [abilities] and is a creature with base power and
//! toughness [P/T] in addition to its other types." 721.2c: While in any zone other than the
//! battlefield, station cards do not have power or toughness. 721.4: each station card has its
//! station ability at all times.
//!
//! Every card is read from its Oracle text.
use crate::support::cite;
use crate::support;
use support::*;

fn game(names: &[&str]) -> T {
    let defs: Vec<CardDef> = ["Wastes"].iter().chain(names).map(|n| oracle(n)).collect();
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


fn station(t: &mut T, card: ObjId, creature: ObjId) {
    let ability = 3 /* Uthros: entry0, Station1, symbol2, derived Station3 */;
    t.g.act(0, Move::Activate { source: card, ability, targets: vec![], paying: vec![vec![creature]] }).unwrap();
    t.g.act(0, Move::Pass).unwrap();
    t.g.act(1, Move::Pass).unwrap();
}
fn charge(t: &T, o: ObjId) -> u32 { t.g.obj(o).counters.get("charge").copied().unwrap_or(0) }
fn creature(t: &T, o: ObjId) -> bool { t.g.is_a(o, ir::CardType::Creature) }

#[test]
fn r702_184a_stationing_puts_charge_counters_equal_to_the_tapped_creatures_power() {
    // Uthros Scanship: "8+ | Flying", 4/4
    let mut t = game(&["Uthros Scanship", "Craw Wurm", "Grizzly Bears"]);
    let in_hand = put(&mut t, 0, "Uthros Scanship", Zone::Hand);
    cite!(["721.2c"]; assert_eq!((t.g.power(in_hand), t.g.toughness(in_hand)), (None, None), "no power or toughness off the battlefield"));
    let ship = put(&mut t, 0, "Uthros Scanship", Zone::Battlefield);
    let wurm = put(&mut t, 0, "Craw Wurm", Zone::Battlefield);
    let bears = put(&mut t, 0, "Grizzly Bears", Zone::Battlefield);
    t.to_no_combat(0, Part::PrecombatMain);
    cite!(["721.2b"]; assert!(!creature(&t, ship) && t.g.power(ship).is_none(), "below 8: not a creature"));
    station(&mut t, ship, wurm);
    cite!(["702.184a"]; assert_eq!((charge(&t, ship), t.g.obj(wurm).tapped), (6, true)));
    cite!(["721.2b"]; assert!(!creature(&t, ship) && !t.g.has(ship, "Flying")));
    station(&mut t, ship, bears);
    cite!(["702.184a"]; assert_eq!(charge(&t, ship), 8));
    cite!(["721.2b"]; assert!(creature(&t, ship) && t.g.has(ship, "Flying")));
    cite!(["721.2b"]; assert_eq!((t.g.power(ship), t.g.toughness(ship)), (Some(4), Some(4))));
    cite!(["721.2b"]; assert!(t.g.is_a(ship, ir::CardType::Artifact), "in addition to its other types"));
    assert!(t.g.obj(ship).chars.subtypes.iter().any(|s| format!("{s:?}").contains("Spacecraft")));
}

#[test]
fn r702_184a_another_untapped_creature_you_control_only_as_a_sorcery() {
    let mut t = game(&["Uthros Scanship", "Grizzly Bears"]);
    let ship = put(&mut t, 0, "Uthros Scanship", Zone::Battlefield);
    t.to_no_combat(0, Part::PrecombatMain);
    let ability = 3 /* Uthros: entry0, Station1, symbol2, derived Station3 */;
    cite!(["702.184a"]; refused("that selection does not satisfy the cost, which asks for exactly 1 card(s): 0 chosen"): unchanged(&mut t.g, |g| g.act(0, Move::Activate { source: ship, ability, targets: vec![], paying: vec![vec![]] })));
    let bears = put(&mut t, 0, "Grizzly Bears", Zone::Battlefield);
    let theirs = put(&mut t, 1, "Grizzly Bears", Zone::Battlefield);
    cite!(["702.184a"]; refused("Grizzly Bears is not what that cost asks for or cannot pay it"): unchanged(&mut t.g, |g| g.act(0, Move::Activate { source: ship, ability, targets: vec![], paying: vec![vec![theirs]] })));
    t.g.tap(bears);
    cite!(["702.184a"]; refused("Grizzly Bears is not what that cost asks for or cannot pay it"): unchanged(&mut t.g, |g| g.act(0, Move::Activate { source: ship, ability, targets: vec![], paying: vec![vec![bears]] })));
    let _ = theirs;
    // at 8+ it is a creature, and still taps another
    let other = put(&mut t, 0, "Grizzly Bears", Zone::Battlefield);
    t.g.put_counters(ship, "charge", 8);
    assert!(creature(&t, ship));
    cite!(["702.184a"]; refused("Uthros Scanship is not what that cost asks for or cannot pay it"): unchanged(&mut t.g, |g| g.act(0, Move::Activate { source: ship, ability, targets: vec![], paying: vec![vec![ship]] })));
    let mut allowed = t.g.clone();
    cite!(["702.184a"]; ok: allowed.act(0, Move::Activate { source: ship, ability, targets: vec![], paying: vec![vec![other]] }));
    assert!(allowed.obj(other).tapped);
    allowed.act(0, Move::Pass).unwrap();
    allowed.act(1, Move::Pass).unwrap();
    assert_eq!(allowed.obj(ship).counters.get("charge"), Some(&10));
    // only as a sorcery: not in the upkeep, not on another player's turn (307.5)
    t.to_no_combat(1, Part::PrecombatMain);
    t.g.act(1, Move::Pass).unwrap();
    assert_eq!(t.g.priority, 0);
    cite!(["702.184a", "307.5"]; refused("that ability is activated only as a sorcery"): unchanged(&mut t.g, |g| g.act(0, Move::Activate { source: ship, ability, targets: vec![], paying: vec![vec![other]] })));
    t.to_no_combat(0, Part::Upkeep);
    assert_eq!((t.g.part, t.g.priority), (Part::Upkeep, 0));
    cite!(["702.184a", "307.5"]; refused("that ability is activated only as a sorcery"): unchanged(&mut t.g, |g| g.act(0, Move::Activate { source: ship, ability, targets: vec![], paying: vec![vec![other]] })));
}

#[test]
fn r702_184a_the_power_is_the_tapped_creatures_as_it_last_was() {
    // the creature dies before the ability resolves: its last known power (608.2h)
    let mut t = game(&["Uthros Scanship", "Hill Giant", "Lightning Bolt"]);
    let ship = put(&mut t, 0, "Uthros Scanship", Zone::Battlefield);
    let giant = put(&mut t, 0, "Hill Giant", Zone::Battlefield);
    t.to_no_combat(0, Part::PrecombatMain);
    let ability = 3 /* Uthros: entry0, Station1, symbol2, derived Station3 */;
    t.g.act(0, Move::Activate { source: ship, ability, targets: vec![], paying: vec![vec![giant]] }).unwrap();
    let bolt = put(&mut t, 0, "Lightning Bolt", Zone::Hand);
    t.g.add_mana(0, ir::mana::ManaSymbol::Color(ir::Color::Red));
    t.g.act(0, crate::support::paid("R",Move::Cast { card: bolt, targets: vec![vec![Target::Object(giant)]] })).unwrap();
    while !t.g.stack.is_empty() { t.g.act(t.g.priority, Move::Pass).unwrap(); }
    assert_eq!(t.g.obj(giant).zone, Zone::Gone);
    cite!(["608.2h"]; assert_eq!(charge(&t, ship), 3));
}

#[test]
fn r721_2a_a_lower_symbol_gives_its_statement_without_the_power_and_toughness_box() {
    // Lumen-Class Frigate: "2+ | Other creatures you control get +1/+1." and "12+ | Flying,
    // lifelink", 3/5
    let mut t = game(&["Lumen-Class Frigate", "Grizzly Bears"]);
    let frigate = put(&mut t, 0, "Lumen-Class Frigate", Zone::Battlefield);
    let bears = put(&mut t, 0, "Grizzly Bears", Zone::Battlefield);
    t.to_no_combat(0, Part::PrecombatMain);
    assert_eq!(t.g.power(bears), Some(2));
    t.g.put_counters(frigate, "charge", 2);
    cite!(["721.2a"]; assert_eq!((t.g.power(bears), t.g.toughness(bears)), (Some(3), Some(3))));
    cite!(["721.2b"]; assert!(!creature(&t, frigate), "the box is the 12+ symbol's"));
    t.g.put_counters(frigate, "charge", 10);
    cite!(["721.2b"]; assert_eq!((t.g.power(frigate), t.g.toughness(frigate)), (Some(3), Some(5))));
    cite!(["721.2a"]; assert!(t.g.has(frigate, "Flying") && t.g.has(frigate, "Lifelink")));
    cite!(["721.2a"]; assert_eq!(t.g.power(bears), Some(3), "both symbols apply"));
    assert_eq!(t.g.power(frigate), Some(3), "other creatures only");
}

#[test]
fn r721_2a_a_planet_gains_its_ability_and_never_becomes_a_creature() {
    // Evendo, Waking Haven: "12+ | {G}, {T}: Add {G} for each creature you control."
    let mut t = game(&["Evendo, Waking Haven", "Grizzly Bears"]);
    let evendo = put(&mut t, 0, "Evendo, Waking Haven", Zone::Battlefield);
    put(&mut t, 0, "Grizzly Bears", Zone::Battlefield);
    t.to_no_combat(0, Part::PrecombatMain);
    let before = t.g.abilities(evendo).len();
    t.g.put_counters(evendo, "charge", 11);
    assert_eq!(t.g.abilities(evendo).len(), before);
    t.g.put_counters(evendo, "charge", 1);
    cite!(["721.2a"]; assert_eq!(t.g.abilities(evendo).len(), before + 1));
    cite!(["721.2a"]; assert!(!creature(&t, evendo) && t.g.power(evendo).is_none()));
}

#[track_caller]
fn unchanged(g: &mut Game, action: impl FnOnce(&mut Game) -> Result<(), String>) -> Result<(), String> {
    let before=format!("{g:?}");
    let result=action(g);
    assert_eq!(format!("{g:?}"),before);
    result
}
