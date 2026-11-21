//! Face-down permanents (708): their characteristics are the face-down ones, also as copiable
//! values (708.2, 708.10); only their controller may look at them (708.5); and what they are is
//! revealed as they leave the battlefield, as a player leaves, and as the game ends (708.9).
//! Three views of one stay apart: the physical card, what it would be face up (respecting copy
//! effects) and what it is now.
use crate::support::cite;
use crate::support;
use game::face::{Concealment, Way};
use game::observation::Kind;
use game::snapshot::Snapshot;
use game::Event;
use support::*;

/// Oracle cards by name, then test texts, each at its index among the definitions.
fn game(names: &[&str], texts: &[(&str, &str, &str, &str, Option<(i32, i32)>)]) -> T {
    let mut defs: Vec<CardDef> = names.iter().map(|n| oracle(n)).collect();
    for (name, cost, types, text, pt) in texts {
        let d = CardDef::new(name, cost, types, text, *pt).unwrap();
        game::play::check(&d).unwrap_or_else(|e| panic!("{name}: {}", e.0));
        defs.push(d);
    }
    T::new(Game::new(defs, 2, 20))
}
fn put(t: &mut T, p: usize, name: &str, zone: Zone) -> ObjId {
    let index = t.g.defs.iter().position(|d| d.name() == name).unwrap_or_else(|| panic!("no {name}"));
    t.g.put(p, index, zone)
}
const FOREVER: (&str, &str, &str, &str, Option<(i32, i32)>) = ("Forever", "{0}", "Instant", "Target creature becomes a copy of target creature.", None);
const BORROW: (&str, &str, &str, &str, Option<(i32, i32)>) = ("Borrow", "{0}", "Instant", "Target creature becomes a copy of target creature until end of turn.", None);
const TUCK: (&str, &str, &str, &str, Option<(i32, i32)>) = ("Tuck", "{0}", "Instant", "Put target creature on top of its owner's library.", None);

/// Player `p` manifests the top card of their library with Soul Summons, which resolves.
fn manifest(t: &mut T, p: usize) -> ObjId {
    put(t, p, "Soul Summons", Zone::Hand);
    while t.g.priority != p { t.g.act(t.g.priority, Move::Pass).unwrap(); }
    let before: Vec<ObjId> = t.g.battlefield.clone();
    t.mana(p, "{1}{W}");
    t.cast(p, "Soul Summons", vec![], crate::support::mana_pool("CW")).unwrap();
    t.resolve();
    let new: Vec<ObjId> = t.g.battlefield.iter().copied().filter(|o| !before.contains(o) && t.g.face_down(*o)).collect();
    assert_eq!(new.len(), 1);
    new[0]
}
/// Player `p` casts this test-text or Oracle card from their hand on these targets, and
/// everything resolves.
fn cast(t: &mut T, p: usize, name: &str, targets: &[ObjId], spending: &str) {
    let card = put(t, p, name, Zone::Hand);
    while t.g.priority != p { t.g.act(t.g.priority, Move::Pass).unwrap(); }
    t.g.players[p].pool.colors = [9; 5];
    t.g.players[p].pool.colorless = 9;
    t.g.act(p, paid(spending, Move::Cast { card, targets: targets.iter().map(|o| vec![Target::Object(*o)]).collect() })).unwrap();
    t.resolve();
    t.g.players[p].pool = Default::default();
}

fn turn_up(t: &mut T, o: ObjId, cost: &str) {
    t.mana(t.g.controller(o), cost);
    t.g.act(t.g.controller(o), paid(cost, Move::TurnFaceUp(o, Way::Manifested))).unwrap();
}
fn view(g: &Game, viewer: Option<usize>, o: ObjId) -> Option<game::snapshot::Object> {
    Snapshot::of(g, viewer).objects.into_iter().find(|x| x.id == o)
}
/// The card a viewer's snapshot says a face-down permanent is, if it says.
fn card_seen(g: &Game, viewer: Option<usize>, o: ObjId) -> Option<String> {
    view(g, viewer, o).and_then(|x| x.face_down).and_then(|d| d.card).map(|c| c.name)
}
fn revealed(g: &Game, seat: usize, name: &str) -> Vec<(Kind, usize, ObjId)> {
    g.observations(seat).iter().flat_map(|d| d.cards.iter().filter(|c| c.name == name).map(|c| (d.kind, d.by, c.object))).collect()
}

// ------------------------------------------------------------------------- copy (707, 708.10)

#[test]
fn r708_10_a_face_down_copy_stays_a_2_2_and_turns_up_as_what_it_copied_for_its_own_cost() {

    let mut t = game(&["Soul Summons", "Grizzly Bears", "Shivan Dragon"], &[FOREVER]);
    put(&mut t, 0, "Grizzly Bears", Zone::Library);
    let o = manifest(&mut t, 0);
    let dragon = put(&mut t, 1, "Shivan Dragon", Zone::Battlefield);
    cast(&mut t, 0, "Forever", &[o, dragon], "");
    // what it is now: still the face-down 2/2 (708.10)
    cite!(["708.10"]; assert!(t.g.face_down(o)));
    assert_eq!(t.g.obj(o).chars.name, "");
    assert_eq!((t.g.power(o), t.g.toughness(o)), (Some(2), Some(2)));
    assert!(!t.g.has(o, "Flying"));
    // the physical card: what its controller sees, and what the special action asks
    assert_eq!(card_seen(&t.g, Some(0), o).as_deref(), Some("Grizzly Bears"));
    cite!(["118.3a"]; refused("the supplied mana does not pay that cost, all of it and no more"): unchanged(&mut t.g,paid("",Move::TurnFaceUp(o, Way::Manifested))));
    turn_up(&mut t, o, "{1}{G}");
    // what it is face up: the copy (708.10)
    cite!(["708.10"]; assert!(!t.g.face_down(o)));
    assert_eq!(t.g.obj(o).chars.name, "Shivan Dragon");
    assert_eq!((t.g.power(o), t.g.toughness(o)), (Some(5), Some(5)));
    assert!(t.g.has(o, "Flying"));
}

#[test]
fn r701_40g_a_manifested_instant_copying_a_creature_is_not_turned_up_and_is_revealed_as_itself() {

    let mut t = game(&["Soul Summons", "Opt", "Shivan Dragon", "Break Open"], &[FOREVER]);
    put(&mut t, 0, "Opt", Zone::Library);
    let o = manifest(&mut t, 0);
    let dragon = put(&mut t, 1, "Shivan Dragon", Zone::Battlefield);
    cast(&mut t, 0, "Forever", &[o, dragon], "");
    t.mana(0, "{9}{R}{R}");
    cite!(["701.40b"]; refused("only a creature card is turned face up for its mana cost (701.40b, 701.58b)"): unchanged(&mut t.g,paid("CCCCCCCCCRR",Move::TurnFaceUp(o, Way::Manifested))));
    t.g.players[0].pool = Default::default();
    for p in 0..2 { t.g.take_observations(p); }
    cast(&mut t, 1, "Break Open", &[o], "CR");
    assert!(t.g.face_down(o), "revealed and left face down");
    assert_eq!(revealed(&t.g, 1, "Opt"), vec![(Kind::Revealed, 0, o)], "named as the instant, shown by its controller");
    assert!(revealed(&t.g, 1, "Shivan Dragon").is_empty() && revealed(&t.g, 1, "").is_empty());
}

#[test]
fn r707_2_a_copy_effect_that_ends_while_face_down_leaves_it_its_own_card() {
    let mut t = game(&["Soul Summons", "Grizzly Bears", "Shivan Dragon"], &[BORROW]);
    for _ in 0..3 { put(&mut t, 0, "Grizzly Bears", Zone::Library); put(&mut t, 1, "Grizzly Bears", Zone::Library); }
    let o = manifest(&mut t, 0);
    let dragon = put(&mut t, 1, "Shivan Dragon", Zone::Battlefield);
    cast(&mut t, 0, "Borrow", &[o, dragon], "");
    assert_eq!(t.g.obj(o).chars.name, "");
    t.to_no_combat(1, Part::PrecombatMain);
    assert!(t.g.face_down(o));
    cite!(["708.2a"]; assert_eq!((t.g.obj(o).chars.name.as_str(), t.g.power(o)), ("", Some(2)), "still the face-down 2/2"));
    t.g.act(1, Move::Pass).unwrap();
    turn_up(&mut t, o, "{1}{G}");
    cite!(["611.2a", "708.8"]; assert_eq!(t.g.obj(o).chars.name, "Grizzly Bears", "the copy effect is over"));
    assert_eq!(t.g.power(o), Some(2));
}

#[test]
fn r708_2_a_clone_of_a_face_down_permanent_is_a_face_up_nameless_2_2_and_stays_one() {

    let mut t = game(&["Soul Summons", "Shivan Dragon", "Clone"], &[]);
    put(&mut t, 0, "Shivan Dragon", Zone::Library);
    let o = manifest(&mut t, 0);
    let clone = put(&mut t, 0, "Clone", Zone::Hand);
    t.mana(0, "{3}{U}");
    t.g.act(0, paid("CCCU", Move::Cast { card: clone, targets: vec![] })).unwrap();
    let boundary = t.settle();
    let Boundary::Question(0, _) = boundary else { panic!("Clone asks what to copy: {boundary:?}") };
    t.answer(0, Answer::Objects(vec![o])).unwrap();
    t.resolve();
    let copy = *t.g.battlefield.iter().find(|c| **c != o).unwrap();
    cite!(["707.2"]; assert!(!t.g.face_down(copy), "face-down status is not copied (707.2)"));
    cite!(["707.2", "708.2"]; assert_eq!(t.g.obj(copy).chars.name, ""));
    cite!(["707.2", "708.2"]; assert_eq!((t.g.power(copy), t.g.toughness(copy)), (Some(2), Some(2))));
    assert!(t.g.abilities(copy).is_empty());
    assert!(t.g.seen(copy, None).colors.is_empty());
    for viewer in [Some(0), Some(1), None] {
        let x = view(&t.g, viewer, copy).expect("public");
        assert!(x.face_down.is_none(), "not face down");
        assert_eq!(x.name, "");
    }
    t.mana(0, "{9}");
    cite!(["116.2b"]; refused("that permanent is face up (116.2b)"): unchanged(&mut t.g,paid("CCCCCCCCC",Move::TurnFaceUp(copy, Way::Manifested))));
    t.g.players[0].pool = Default::default();
    // the original turns up; the clone's values were fixed as it entered (707.2c)
    turn_up(&mut t, o, "{4}{R}{R}");
    assert_eq!(t.g.obj(o).chars.name, "Shivan Dragon");
    assert_eq!(t.g.obj(copy).chars.name, "");
    assert_eq!(t.g.power(copy), Some(2));
}

// -------------------------------------------------------------------------- characteristics

#[test]
fn r708_8_effects_counters_damage_attachments_and_combat_carry_over_turning_face_up() {
    let mut t = game(&["Soul Summons", "Grizzly Bears", "Giant Growth", "Bonesplitter"], &[]);
    put(&mut t, 0, "Grizzly Bears", Zone::Library);
    let o = manifest(&mut t, 0);
    t.g.put_counters(o, "+1/+1", 1);
    let blade = put(&mut t, 0, "Bonesplitter", Zone::Battlefield);
    assert!(t.g.attach(blade, Target::Object(o)));
    cast(&mut t, 0, "Giant Growth", &[o], "G");
    assert_eq!(t.g.power(o), Some(2 + 1 + 2 + 3));
    t.g.damage(blade, Target::Object(o), 1);
    turn_up(&mut t, o, "{1}{G}");
    assert_eq!(t.g.obj(o).chars.name, "Grizzly Bears");
    cite!(["708.8"]; assert_eq!((t.g.power(o), t.g.toughness(o)), (Some(8), Some(6)), "counter, equipment and pump"));
    cite!(["708.8"]; assert_eq!(t.g.obj(o).damage, 1));
    assert_eq!(t.g.attached_to(blade), Some(Target::Object(o)));
}

#[test]
fn r613_7f_turning_face_up_gives_it_a_new_timestamp() {
    // Its own static sets creatures' base power and toughness; an enchantment that arrived
    // after it was manifested sets them too. Turned face up later, its effect is the later.
    let mut t = game(&["Soul Summons"], &[
        ("Giantmaker", "{1}", "Creature — Giant", "Creatures you control have base power and toughness 3/3.", Some((1, 1))),
        ("Shrinker", "{0}", "Enchantment", "Creatures have base power and toughness 1/1.", None),
    ]);
    put(&mut t, 0, "Giantmaker", Zone::Library);
    let o = manifest(&mut t, 0);
    put(&mut t, 1, "Shrinker", Zone::Battlefield);
    assert_eq!(t.g.power(o), Some(1));
    turn_up(&mut t, o, "{1}");
    cite!(["613.7f"]; assert_eq!((t.g.power(o), t.g.toughness(o)), (Some(3), Some(3)), "its static is newer than the Shrinker (613.7f)"));
}

#[test]
fn r701_40b_the_card_is_asked_whatever_type_changing_effects_make_the_permanent() {
    // Song of the Dryads: the permanent is a colorless Forest land, not a creature; its card
    // still is a creature card (ruling 9dbce1581583f428).
    let mut t = game(&["Soul Summons", "Grizzly Bears", "Song of the Dryads"], &[]);
    put(&mut t, 0, "Grizzly Bears", Zone::Library);
    let o = manifest(&mut t, 0);
    cast(&mut t, 0, "Song of the Dryads", &[o], "CCG");
    assert!(!t.g.is_a(o, ir::CardType::Creature));
    turn_up(&mut t, o, "{1}{G}");
    cite!(["701.40b"]; assert!(!t.g.face_down(o)));
    assert_eq!(t.g.obj(o).chars.name, "Grizzly Bears");
    cite!(["708.8"]; assert!(!t.g.is_a(o, ir::CardType::Creature), "Song of the Dryads still applies"));
}

#[test]
fn r708_2a_a_face_down_permanent_cannot_transform() {
    let mut t = game(&["Soul Summons", "Daybreak Ranger"], &[("Turner", "{0}", "Instant", "Transform target creature.", None)]);
    put(&mut t, 0, "Daybreak Ranger", Zone::Library);
    let o = manifest(&mut t, 0);
    cast(&mut t, 0, "Turner", &[o], "");
    cite!(["712.15a"]; assert!(!t.g.obj(o).transformed && t.g.face_down(o)));
    assert_eq!((t.g.obj(o).chars.name.as_str(), t.g.power(o)), ("", Some(2)));
}

#[test]
fn r708_3_what_enters_face_down_is_asked_as_the_2_2_it_will_be() {
    // Root Maze taps entering artifacts and lands; a manifested Forest enters as a creature.
    let mut t = game(&["Soul Summons", "Forest", "Root Maze", "Shivan Dragon"], &[("Gate", "{0}", "Enchantment", "Creatures with mana value 4 or greater enter tapped.", None)]);
    put(&mut t, 0, "Forest", Zone::Library);
    put(&mut t, 1, "Root Maze", Zone::Battlefield);
    let o = manifest(&mut t, 0);
    cite!(["708.3", "614.12"]; assert!(!t.g.obj(o).tapped, "not a land as it enters"));
    put(&mut t, 0, "Shivan Dragon", Zone::Library);
    put(&mut t, 1, "Gate", Zone::Battlefield);
    let o = manifest(&mut t, 0);
    cite!(["708.3", "614.12"]; assert!(!t.g.obj(o).tapped, "mana value 0 as it enters"));
}

#[test]
fn r708_6_two_face_down_permanents_are_two_objects_known_by_how() {
    let mut t = game(&["Soul Summons", "Grizzly Bears", "Shivan Dragon"], &[]);
    put(&mut t, 0, "Grizzly Bears", Zone::Library);
    put(&mut t, 0, "Shivan Dragon", Zone::Library);
    let a = manifest(&mut t, 0);
    let b = manifest(&mut t, 0);
    assert_ne!(a, b);
    let seen = Snapshot::of(&t.g, Some(1));
    let down: Vec<_> = seen.objects.iter().filter(|x| x.face_down.is_some()).collect();
    assert_eq!(down.len(), 2);
    cite!(["708.6"]; assert!(down.iter().all(|x| x.face_down.as_ref().unwrap().by == Concealment::Manifested && x.face_down.as_ref().unwrap().card.is_none())));
    cite!(["708.5"]; assert_eq!(card_seen(&t.g, Some(0), a).as_deref(), Some("Shivan Dragon"), "the first manifested was the top card"));
    assert_eq!(card_seen(&t.g, Some(0), b).as_deref(), Some("Grizzly Bears"));
}

// ----------------------------------------------------------------------- access (708.5)

#[test]
fn r708_5_its_controller_sees_its_card_in_every_snapshot_and_no_one_else_does() {
    let mut t = game(&["Soul Summons", "Shivan Dragon"], &[]);
    put(&mut t, 0, "Shivan Dragon", Zone::Library);
    let o = manifest(&mut t, 0);
    for p in 0..2 { t.g.take_observations(p); }
    // draining what was shown does not take away what may be looked at now
    cite!(["708.5"]; assert_eq!(card_seen(&t.g, Some(0), o).as_deref(), Some("Shivan Dragon")));
    assert_eq!(card_seen(&t.g, None, o).as_deref(), Some("Shivan Dragon"), "the master view");
    let theirs = view(&t.g, Some(1), o).expect("the opponent perceives it");
    assert_eq!(theirs.name, "");
    cite!(["708.5"]; assert_eq!(theirs.face_down.as_ref().map(|d| (d.by, d.card.is_none())), Some((Concealment::Manifested, true))));
    assert_eq!(theirs.pt, Some((2, 2)));
    let mine = view(&t.g, Some(0), o).unwrap();
    assert_eq!(mine.name, "", "its name is the face-down one, for everyone");
    cite!(["708.5"]; assert!(t.g.sees(0, o) && !t.g.sees(1, o)));
    assert!(t.g.perceives(0, o) && t.g.perceives(1, o));
}

#[test]
fn r708_5_who_may_look_follows_control() {
    let mut t = game(&["Soul Summons", "Shivan Dragon", "Act of Treason"], &[]);
    for _ in 0..3 { put(&mut t, 0, "Soul Summons", Zone::Library); put(&mut t, 1, "Soul Summons", Zone::Library); }
    put(&mut t, 0, "Shivan Dragon", Zone::Library);
    let o = manifest(&mut t, 0);
    for p in 0..2 { t.g.take_observations(p); }
    t.to_no_combat(1, Part::PrecombatMain);
    cast(&mut t, 1, "Act of Treason", &[o], "CCR");
    assert_eq!(t.g.controller(o), 1);
    cite!(["708.5"]; assert_eq!(card_seen(&t.g, Some(1), o).as_deref(), Some("Shivan Dragon"), "its new controller looks"));
    cite!(["708.5"]; assert_eq!(card_seen(&t.g, Some(0), o), None, "its old controller may not look"));
    assert_eq!(revealed(&t.g, 1, "Shivan Dragon"), vec![(Kind::Looked, 1, o)], "looked as control was gained");
    assert!(revealed(&t.g, 0, "Shivan Dragon").is_empty());
    t.to_no_combat(0, Part::PrecombatMain);
    assert_eq!(t.g.controller(o), 0, "control returned");
    cite!(["708.5"]; assert_eq!(card_seen(&t.g, Some(0), o).as_deref(), Some("Shivan Dragon")));
    cite!(["708.5"]; assert_eq!(card_seen(&t.g, Some(1), o), None, "the opponent keeps only what they were shown"));
    assert_eq!(revealed(&t.g, 1, "Shivan Dragon").len(), 1);
}

#[test]
fn reality_shift_leaks_no_library_card_to_its_caster() {
    let mut t = game(&["Reality Shift", "Grizzly Bears", "Shivan Dragon"], &[]);
    put(&mut t, 1, "Shivan Dragon", Zone::Library);
    let bears = put(&mut t, 1, "Grizzly Bears", Zone::Battlefield);
    let from = t.mark();
    cast(&mut t, 0, "Reality Shift", &[bears], "CU");
    let o = *t.g.battlefield.iter().find(|c| t.g.face_down(**c)).unwrap();
    assert_eq!(t.g.controller(o), 1);
    cite!(["708.5"]; assert!(Snapshot::of(&t.g, Some(0)).objects.iter().all(|x| x.name != "Shivan Dragon" && x.face_down.as_ref().is_none_or(|d| d.card.is_none()))));
    assert!(revealed(&t.g, 0, "Shivan Dragon").is_empty());
    assert!(!t.g.log.since(from).iter().any(|e| matches!(e, Event::Revealed { .. })));
    cite!(["708.5"]; assert_eq!(card_seen(&t.g, Some(1), o).as_deref(), Some("Shivan Dragon")));
}

#[test]
fn r708_3_a_manifested_clone_asks_nothing_as_it_enters() {
    let mut t = game(&["Soul Summons", "Clone", "Grizzly Bears"], &[]);
    put(&mut t, 1, "Grizzly Bears", Zone::Battlefield);
    put(&mut t, 0, "Clone", Zone::Library);
    let o = manifest(&mut t, 0);
    cite!(["708.3"]; assert_eq!((t.g.obj(o).chars.name.as_str(), t.g.power(o)), ("", Some(2)), "its own as-enters copy choice does not exist (708.3)"));
}

#[test]
fn r616_1_ordering_entry_replacements_on_a_manifest_names_only_the_face_down_object() {
    let mut t = game(&["Soul Summons", "Shivan Dragon"], &[
        ("Tapper", "{0}", "Enchantment", "Creatures enter tapped.", None),
        ("Grower", "{0}", "Enchantment", "Creatures enter with an additional +1/+1 counter on them.", None),
    ]);
    let card = put(&mut t, 0, "Shivan Dragon", Zone::Library);
    put(&mut t, 1, "Tapper", Zone::Battlefield);
    put(&mut t, 1, "Grower", Zone::Battlefield);
    put(&mut t, 0, "Soul Summons", Zone::Hand);
    t.mana(0, "{1}{W}");
    t.cast(0, "Soul Summons", vec![], crate::support::mana_pool("CW")).unwrap();
    let mut asked_entering = Vec::new();
    loop {
        match t.settle() {
            Boundary::Question(p, Asked::Replacement { .. }) => {
                asked_entering.push((p, t.g.entering_card(), t.g.entering_object()));
                let Some((_, Asked::Replacement { among, .. })) = t.asked() else { unreachable!() };
                t.answer(p, Answer::Replacement(among[0].clone())).unwrap();
            }
            Boundary::Priority(_) => break,
            other => panic!("unexpected {other:?}"),
        }
    }
    let o = *t.g.battlefield.iter().find(|x| t.g.face_down(**x)).unwrap();
    assert!(t.g.obj(o).tapped && t.g.obj(o).counters.get("+1/+1") == Some(&1), "both applied");
    assert!(!asked_entering.is_empty(), "the order was asked");
    for (p, entering, on) in asked_entering {
        cite!(["616.1"]; assert_eq!(p, 0, "the affected object's controller orders them (616.1)"));
        assert_ne!(entering, Some(card), "the library card is not named");
        assert_eq!(entering, on, "only the face-down object it becomes");
        assert_eq!(on, Some(o));
    }
}

// --------------------------------------------------------------- revealing it (708.9, 701.40)

#[test]
fn r708_9_a_face_down_permanent_that_dies_is_revealed_by_its_owner_and_dies_as_a_nameless_2_2() {
    let mut t = game(&["Soul Summons", "Grizzly Bears", "Murder"], &[
        ("Mourner", "{0}", "Enchantment", "Whenever a creature dies, you gain life equal to its power.", None),
        ("Namer", "{0}", "Enchantment", "Whenever a creature named Grizzly Bears dies, you gain 5 life.", None),
    ]);
    put(&mut t, 0, "Grizzly Bears", Zone::Library);
    let o = manifest(&mut t, 0);
    put(&mut t, 1, "Mourner", Zone::Battlefield);
    put(&mut t, 1, "Namer", Zone::Battlefield);
    for p in 0..2 { t.g.take_observations(p); }
    let from = t.mark();
    cast(&mut t, 1, "Murder", &[o], "CBB");
    let card = *t.g.players[0].graveyard.last().unwrap();
    cite!(["400.7"]; assert_ne!(card, o, "a new object (400.7)"));
    assert_eq!(t.g.obj(card).chars.name, "Grizzly Bears", "face up in the graveyard");
    assert_eq!(t.g.players[1].life, 22, "it died as a 2/2 with no name");
    for seat in 0..2 { cite!(["708.9"]; case: seat; assert_eq!(revealed(&t.g, seat, "Grizzly Bears"), vec![(Kind::Revealed, 0, o)], "seat {seat}: its owner reveals the face-down object")); }
    assert!(t.g.log.since(from).iter().any(|e| matches!(e, Event::Revealed { player: 0, cards } if cards == &vec![o])));
}

#[test]
fn r708_9_a_face_down_copy_that_leaves_is_revealed_as_its_physical_card() {
    let mut t = game(&["Soul Summons", "Grizzly Bears", "Shivan Dragon", "Unsummon"], &[FOREVER]);
    put(&mut t, 0, "Grizzly Bears", Zone::Library);
    let o = manifest(&mut t, 0);
    let dragon = put(&mut t, 1, "Shivan Dragon", Zone::Battlefield);
    cast(&mut t, 0, "Forever", &[o, dragon], "");
    for p in 0..2 { t.g.take_observations(p); }
    cast(&mut t, 1, "Unsummon", &[o], "U");
    cite!(["708.9"]; assert_eq!(revealed(&t.g, 1, "Grizzly Bears"), vec![(Kind::Revealed, 0, o)]));
    assert!(revealed(&t.g, 1, "Shivan Dragon").is_empty());
    let card = *t.g.players[0].hand.last().unwrap();
    assert_eq!(t.g.obj(card).chars.name, "Grizzly Bears");
    assert!(view(&t.g, Some(1), card).is_none(), "private again in its owner's hand");
}

#[test]
fn r708_9_a_tucked_face_down_permanent_is_revealed_and_its_library_card_stays_hidden() {
    let mut t = game(&["Soul Summons", "Grizzly Bears"], &[TUCK]);
    put(&mut t, 0, "Grizzly Bears", Zone::Library);
    let o = manifest(&mut t, 0);
    cast(&mut t, 1, "Tuck", &[o], "");
    cite!(["708.9"]; assert_eq!(revealed(&t.g, 1, "Grizzly Bears"), vec![(Kind::Revealed, 0, o)]));
    let top = *t.g.players[0].library.last().unwrap();
    assert_eq!(t.g.obj(top).chars.name, "Grizzly Bears");
    for viewer in [Some(0), Some(1)] { assert!(view(&t.g, viewer, top).is_none(), "its position in the library is hidden"); }
}

#[test]
fn r708_9_turned_up_then_moved_before_the_seat_reads_still_names_what_was_shown() {
    let mut t = game(&["Soul Summons", "Grizzly Bears", "Break Open", "Unsummon", "Clone"], &[]);
    put(&mut t, 0, "Grizzly Bears", Zone::Library);
    let o = manifest(&mut t, 0);
    for p in 0..2 { t.g.take_observations(p); }
    cast(&mut t, 1, "Break Open", &[o], "CR");
    cast(&mut t, 1, "Unsummon", &[o], "U");
    let shown = t.g.take_observations(1);
    let names: Vec<_> = shown.iter().map(|d| (d.kind, d.by, d.cards[0].object, d.cards[0].name.clone())).collect();
    assert_eq!(names, vec![(Kind::Revealed, 0, o, "Grizzly Bears".to_string())], "one reveal as it turned up; leaving face up reveals nothing more");
}

#[test]
fn r708_9_at_the_end_of_the_game_every_face_down_permanent_is_revealed() {
    let mut t = game(&["Soul Summons", "Shivan Dragon", "Lightning Bolt"], &[]);
    put(&mut t, 0, "Shivan Dragon", Zone::Library);
    let o = manifest(&mut t, 0);
    for p in 0..2 { t.g.take_observations(p); }
    t.g.players[1].life = 3;
    t.expect_losses(&[1]);
    let bolt = put(&mut t, 0, "Lightning Bolt", Zone::Hand);
    t.mana(0, "{R}");
    t.g.act(0, paid("R", Move::Cast { card: bolt, targets: vec![vec![Target::Player(1)]] })).unwrap();
    t.settle();
    assert!(t.g.finished());
    cite!(["708.9"]; assert_eq!(revealed(&t.g, 1, "Shivan Dragon"), vec![(Kind::Revealed, 0, o)]));
}

#[test]
fn r708_9_a_player_who_leaves_reveals_the_face_down_permanents_they_own() {
    let mut t = game(&["Soul Summons", "Shivan Dragon", "Lightning Bolt"], &[]);
    let mut g = Game::new(t.g.defs.clone(), 3, 20);
    std::mem::swap(&mut t.g, &mut g);
    (t.g.active, t.g.priority) = (1, 1);
    put(&mut t, 1, "Shivan Dragon", Zone::Library);
    let o = manifest(&mut t, 1);
    for p in 0..3 { t.g.take_observations(p); }
    t.g.players[1].life = 3;
    t.expect_losses(&[1]);
    let bolt = put(&mut t, 0, "Lightning Bolt", Zone::Hand);
    while t.g.priority != 0 { t.g.act(t.g.priority, Move::Pass).unwrap(); }
    t.mana(0, "{R}");
    t.g.act(0, paid("R", Move::Cast { card: bolt, targets: vec![vec![Target::Player(1)]] })).unwrap();
    t.settle();
    assert!(!t.g.finished());
    assert!(t.g.players[1].lost);
    for seat in [0, 2] { cite!(["708.9"]; case: seat; assert_eq!(revealed(&t.g, seat, "Shivan Dragon"), vec![(Kind::Revealed, 1, o)], "seat {seat}")); }
}

#[track_caller]
fn unchanged(g: &mut Game, action: Move) -> Result<(), String> {
    let before=format!("{g:?}"); let result=g.act(0,action);
    assert_eq!(format!("{g:?}"),before); result
}
