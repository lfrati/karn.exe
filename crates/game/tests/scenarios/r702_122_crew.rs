//! Crew (702.122): "Crew N" means "Tap any number of other untapped creatures you control with
//! total power N or greater: This permanent becomes an artifact creature until end of turn."
//! A Vehicle has its printed power and toughness only while it is a creature (301.7a-b, 208.3).
//! A creature tapped to pay that cost crews the Vehicle (702.122b-c); the Vehicle "becomes
//! crewed" as the ability resolves (702.122e).
use crate::support::cite;
use crate::support;
use game::legal::Asked;
use game::Event;
use support::*;

type Text = (&'static str, &'static str, &'static str, &'static str, Option<(i32, i32)>);
const BIG: Text = ("Crew three vehicle", "{1}", "Artifact — Vehicle", "Crew 3", Some((5, 5)));
const CREWED: Text = ("Crewed watcher", "{1}", "Artifact — Vehicle", "Whenever this Vehicle becomes crewed, you gain 1 life.\nCrew 1", Some((2, 2)));

/// Oracle cards by name, then test texts; each library holds ten Wastes. Player 0's
/// precombat main phase.
fn game(names: &[&str], texts: &[Text]) -> T {
    let mut defs: Vec<CardDef> = ["Wastes"].iter().chain(names).map(|n| oracle(n)).collect();
    for (name, cost, types, text, pt) in texts {
        let d = CardDef::new(name, cost, types, text, *pt).unwrap();
        game::play::check(&d).unwrap_or_else(|e| panic!("{name}: {}", e.0));
        defs.push(d);
    }
    let mut g = Game::new(defs, 2, 20);
    for p in 0..2 { for _ in 0..10 { g.put(p, 0, Zone::Library); } }
    T::new(g)
}
fn put(t: &mut T, p: usize, name: &str, zone: Zone) -> ObjId {
    let index = t.g.defs.iter().position(|d| d.name() == name).unwrap_or_else(|| panic!("no {name}"));
    t.g.put(p, index, zone)
}

fn crew(t: &mut T, vehicle: ObjId, ability: usize, creatures: &[ObjId]) -> Result<(), String> {
    t.g.act(0, Move::Activate { source: vehicle, ability, targets: vec![], paying: vec![creatures.to_vec()] })
}
fn creature(t: &T, o: ObjId) -> bool { t.g.is_a(o, ir::CardType::Creature) }
fn resolve(t: &mut T) { t.g.act(0, Move::Pass).unwrap(); t.g.act(1, Move::Pass).unwrap(); }

#[test]
fn r702_122a_crewing_taps_creatures_and_makes_the_vehicle_an_artifact_creature_until_end_of_turn() {
    let mut t = game(&["Smuggler's Copter", "Grizzly Bears"], &[]);
    let copter = put(&mut t, 0, "Smuggler's Copter", Zone::Battlefield);
    let bears = put(&mut t, 0, "Grizzly Bears", Zone::Battlefield);
    t.to_no_combat(0, Part::PrecombatMain);
    // 301.7a, 208.3: a noncreature Vehicle has no power or toughness
    cite!(["208.3", "301.7a"]; assert!(!creature(&t, copter)));
    assert_eq!(t.g.power(copter), None);
    crew(&mut t, copter, 3, &[bears]).unwrap();
    assert!(t.g.obj(bears).tapped, "the crewing creature is tapped as a cost");
    assert!(!creature(&t, copter), "the ability uses the stack");
    resolve(&mut t);
    // 301.7b: it immediately has its printed power and toughness
    cite!(["702.122a"]; assert!(creature(&t, copter) && t.g.is_a(copter, ir::CardType::Artifact)));
    cite!(["301.7b"]; assert_eq!((t.g.power(copter), t.g.toughness(copter)), (Some(3), Some(3))));
    t.to_no_combat(1, Part::PrecombatMain);
    cite!(["514.2"]; assert!(!creature(&t, copter), "until end of turn"));
    assert_eq!(t.g.power(copter), None);
}

#[test]
fn r702_122a_the_tapped_creatures_need_total_power_n_or_greater() {
    let mut t = game(&["Grizzly Bears", "Memnite", "Ornithopter"], &[BIG]);
    let vehicle = put(&mut t, 0, "Crew three vehicle", Zone::Battlefield);
    let bears = put(&mut t, 0, "Grizzly Bears", Zone::Battlefield);
    let memnite = put(&mut t, 0, "Memnite", Zone::Battlefield);
    let thopter = put(&mut t, 0, "Ornithopter", Zone::Battlefield);
    t.to_no_combat(0, Part::PrecombatMain);
    cite!(["702.122a"]; refused("that selection does not satisfy the cost: the chosen creatures have total power less than 3 (601.2h)"): crew(&mut t, vehicle, 1, &[bears, thopter]));
    assert!(!t.g.obj(bears).tapped, "a refused activation taps nothing");
    // more than enough is enough, and a creature with no power may be among them
    crew(&mut t, vehicle, 1, &[bears, memnite, thopter]).unwrap();
    cite!(["702.122a"]; assert!([bears, memnite, thopter].iter().all(|c| t.g.obj(*c).tapped)));
    resolve(&mut t);
    assert_eq!(t.g.power(vehicle), Some(5));
}

#[test]
fn r702_122a_not_enough_power_cannot_pay_crew() {
    let mut t = game(&["Memnite", "Ornithopter"], &[BIG]);
    let vehicle = put(&mut t, 0, "Crew three vehicle", Zone::Battlefield);
    let memnite = put(&mut t, 0, "Memnite", Zone::Battlefield);
    let thopter = put(&mut t, 0, "Ornithopter", Zone::Battlefield);
    t.to_no_combat(0, Part::PrecombatMain);
    let before = format!("{:?}", t.g);
    cite!(["702.122a"]; refused("chosen creatures have total power less than 3"): crew(&mut t, vehicle, 1, &[memnite, thopter]));
    assert_eq!(format!("{:?}", t.g), before);
}

#[test]
fn r702_122a_only_other_untapped_creatures_you_control_crew() {
    let mut t = game(&["Smuggler's Copter", "Grizzly Bears", "Memnite"], &[]);
    let copter = put(&mut t, 0, "Smuggler's Copter", Zone::Battlefield);
    let bears = put(&mut t, 0, "Grizzly Bears", Zone::Battlefield);
    let tapped = put(&mut t, 0, "Memnite", Zone::Battlefield);
    let theirs = put(&mut t, 1, "Grizzly Bears", Zone::Battlefield);
    t.g.set_tapped(tapped, true);
    t.to_no_combat(0, Part::PrecombatMain);
    cite!(["702.122a"]; refused("Memnite is not what that cost asks for or cannot pay it (701.26, 118.3)"): crew(&mut t, copter, 3, &[tapped]));
    cite!(["702.122a"]; refused("Grizzly Bears is not what that cost asks for or cannot pay it (701.26, 118.3)"): crew(&mut t, copter, 3, &[theirs]));
    crew(&mut t, copter, 3, &[bears]).unwrap();
    resolve(&mut t);
    // a crewed Vehicle is a creature, but not another creature that can crew it
    cite!(["702.122a"]; refused("Smuggler's Copter is not what that cost asks for or cannot pay it (701.26, 118.3)"): crew(&mut t, copter, 3, &[copter]));
}

#[test]
fn r702_122a_a_creature_that_just_came_under_your_control_crews() {
    // Crewing taps a creature as a cost without the tap symbol: 302.6 does not apply.
    let mut t = game(&["Smuggler's Copter", "Memnite"], &[]);
    let copter = put(&mut t, 0, "Smuggler's Copter", Zone::Battlefield);
    let memnite = put(&mut t, 0, "Memnite", Zone::Hand);
    t.to_no_combat(0, Part::PrecombatMain);
    t.g.act(0, Move::Cast { card: memnite, targets: vec![] }).unwrap();
    resolve(&mut t);
    let memnite = t.find_in(0, Zone::Battlefield, "Memnite");
    assert!(t.g.summoning_sick(memnite));
    cite!(["702.122a", "302.6"]; ok: crew(&mut t, copter, 3, &[memnite]));
    resolve(&mut t);
    assert!(creature(&t, copter));
}

#[test]
fn r702_122b_each_creature_that_crews_triggers_its_ability_before_the_crew_ability_resolves() {
    let mut t = game(&["Smuggler's Copter", "Veteran Motorist", "Speedway Fanatic"], &[]);
    let copter = put(&mut t, 0, "Smuggler's Copter", Zone::Battlefield);
    let motorist = put(&mut t, 0, "Veteran Motorist", Zone::Battlefield);
    let fanatic = put(&mut t, 0, "Speedway Fanatic", Zone::Battlefield);
    t.to_no_combat(0, Part::PrecombatMain);
    let mark = t.mark();
    crew(&mut t, copter, 3, &[motorist, fanatic]).unwrap();
    cite!(["702.122b"]; assert!(t.g.log.since(mark).iter().any(|e| matches!(e, Event::Crews { vehicle, creatures } if *vehicle == copter && creatures.len() == 2))));
    // both trigger, and go on the stack above the crew ability in the order their controller
    // chooses (603.3b), so they resolve first
    let Some((0, Asked::Order(abilities))) = t.asked() else { panic!("two triggers to order") };
    cite!(["603.3b"]; assert_eq!(abilities.len(), 2));
    t.g.act(0, Move::Order(vec![0, 1])).unwrap();
    assert_eq!(t.g.stack.len(), 3, "the crew ability and two crews triggers");
    resolve(&mut t);
    resolve(&mut t);
    resolve(&mut t);
    assert!(t.g.stack.is_empty());
    // "that Vehicle": the one it crewed. It is a creature now, so it has the +1/+1 and haste
    cite!(["208.3a"]; assert_eq!((t.g.power(copter), t.g.toughness(copter)), (Some(4), Some(4))));
    assert!(t.g.has(copter, "Haste"));
}

#[test]
fn r702_122e_a_vehicle_becomes_crewed_once_as_its_crew_ability_resolves() {
    let mut t = game(&["Grizzly Bears", "Memnite"], &[CREWED]);
    let vehicle = put(&mut t, 0, "Crewed watcher", Zone::Battlefield);
    let bears = put(&mut t, 0, "Grizzly Bears", Zone::Battlefield);
    let memnite = put(&mut t, 0, "Memnite", Zone::Battlefield);
    t.to_no_combat(0, Part::PrecombatMain);
    crew(&mut t, vehicle, 2, &[bears, memnite]).unwrap();
    cite!(["702.122e"]; assert_eq!(t.g.stack.len(), 1, "it does not become crewed by paying the cost"));
    let mark = t.mark();
    resolve(&mut t);
    cite!(["702.122e"]; assert!(t.g.log.since(mark).iter().any(|e| matches!(e, Event::BecameCrewed { vehicle: v, by } if *v == vehicle && by.len() == 2))));
    cite!(["702.122e"]; assert_eq!(t.g.stack.len(), 1, "one trigger, however many creatures crewed it"));
    resolve(&mut t);
    t.expect().life(0, 21).check();
}

#[test]
fn r702_122e_a_crew_ability_that_leaves_the_stack_unresolved_does_not_crew() {
    let mut t = game(&["Grizzly Bears", "Stifle", "Island"], &[CREWED]);
    let vehicle = put(&mut t, 0, "Crewed watcher", Zone::Battlefield);
    let bears = put(&mut t, 0, "Grizzly Bears", Zone::Battlefield);
    let stifle = put(&mut t, 1, "Stifle", Zone::Hand);
    t.to_no_combat(0, Part::PrecombatMain);
    crew(&mut t, vehicle, 2, &[bears]).unwrap();
    // the creature crewed it as the cost was paid (702.122c), but it did not become crewed
    cite!(["702.122c"]; assert!(t.g.log.iter().any(|e| matches!(e, Event::Crews { .. }))));
    let ability = *t.g.stack.last().unwrap();
    t.g.act(0, Move::Pass).unwrap();
    t.mana(1, "{U}");
    t.g.act(1, crate::support::paid("U",Move::Cast { card: stifle, targets: vec![vec![Target::Object(ability)]] })).unwrap();
    t.g.act(1, Move::Pass).unwrap();
    t.g.act(0, Move::Pass).unwrap();
    assert!(t.g.stack.is_empty(), "Stifle countered the crew ability");
    cite!(["701.6a"]; assert!(!creature(&t, vehicle)));
    cite!(["702.122e"]; assert!(!t.g.log.iter().any(|e| matches!(e, Event::BecameCrewed { .. }))));
    t.expect().life(0, 20).check();
}

#[test]
fn r702_122e_mobilizer_mech_animates_another_vehicle_when_it_becomes_crewed() {
    let mut t = game(&["Mobilizer Mech", "Smuggler's Copter", "Grizzly Bears", "Memnite"], &[]);
    let mech = put(&mut t, 0, "Mobilizer Mech", Zone::Battlefield);
    let copter = put(&mut t, 0, "Smuggler's Copter", Zone::Battlefield);
    let bears = put(&mut t, 0, "Grizzly Bears", Zone::Battlefield);
    let memnite = put(&mut t, 0, "Memnite", Zone::Battlefield);
    t.to_no_combat(0, Part::PrecombatMain);
    crew(&mut t, mech, 3, &[bears, memnite]).unwrap();
    resolve(&mut t);
    assert!(creature(&t, mech));
    // the trigger targets "up to one other target Vehicle you control": not the Mech
    let Some((0, Asked::Targets { slots, .. })) = t.asked() else { panic!("its targets are chosen") };
    assert_eq!(slots[0].among, vec![Target::Object(copter)]);
    t.g.act(0, Move::ChooseTargets(vec![vec![Target::Object(copter)]])).unwrap();
    assert_eq!(t.g.stack.len(), 1);
    resolve(&mut t);
    cite!(["702.122e"]; assert!(creature(&t, copter)));
    cite!(["301.7b"]; assert_eq!(t.g.power(copter), Some(3)));
}
