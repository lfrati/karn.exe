//! "When this creature is turned face up" and "whenever a permanent is turned face up"
//! (708.8): a permanent turned face up is the same permanent, so nothing that triggers on
//! entering does, but what triggers on turning face up does, whether the special action or an
//! effect turned it, and it is looked at as it is face up. A manifested instant or sorcery is
//! revealed instead and nothing triggers (701.40g). Dog Walker, Pine Walker, Aphetto
//! Runecaster, Trail of Mystery, Secret Plans and Sumala Sentry are Oracle cards.
use crate::support::cite;
use crate::support;
use game::announcement::CastCost;
use game::face::Way;
use support::*;

type Text<'a> = (&'a str, &'a str, &'a str, &'a str, Option<(i32, i32)>);

/// Oracle cards by name, then test texts, with ten Wastes in each library.
pub(crate) fn game(names: &[&str], texts: &[Text]) -> T {
    let mut defs: Vec<CardDef> = names.iter().map(|n| oracle(n)).collect();
    for (name, cost, types, text, pt) in texts {
        let d = CardDef::new(name, cost, types, text, *pt).unwrap();
        game::play::check(&d).unwrap_or_else(|e| panic!("{name}: {}", e.0));
        defs.push(d);
    }
    defs.push(CardDef::new("Wastes", "", "Basic Land", "", None).unwrap());
    let mut g = Game::new(defs, 2, 20);
    let filler = g.defs.len() - 1;
    for p in 0..2 { for _ in 0..10 { g.put(p, filler, Zone::Library); } }
    T::new(g)
}
pub(crate) fn put(t: &mut T, p: usize, name: &str, zone: Zone) -> ObjId {
    let index = t.g.defs.iter().position(|d| d.name() == name).unwrap_or_else(|| panic!("no {name}"));
    t.g.put(p, index, zone)
}
/// Player `p`, who has priority in their main phase with an empty stack, casts this card face
/// down for {3} and it resolves: the face-down permanent.
pub(crate) fn face_down(t: &mut T, p: usize, name: &str) -> ObjId {
    let card = put(t, p, name, Zone::Hand);
    t.mana(p, "{3}");
    let row = t.g.def(card).unwrap().abilities().iter().position(|a| matches!(a,
        ir::Ability::Keywords(items) if items.iter().any(|i| matches!(i.keyword.name(), "Morph" | "Megamorph" | "Disguise"))))
        .expect("the fixture's printed face-down cost");
    let cost = CastCost::Alternative(row);
    t.g.act(p, paid("CCC", Move::CastFor(cost, Box::new(Move::Cast { card, targets: vec![] })))).unwrap();
    assert!(matches!(cost, CastCost::Alternative(_)));
    let before = t.g.battlefield.clone();
    t.resolve();
    *t.g.battlefield.iter().find(|o| !before.contains(o)).expect("it entered")
}
/// Player `p` manifests the top card of their library with Soul Summons, which resolves.
pub(crate) fn manifest(t: &mut T, p: usize, top: &str) -> ObjId {
    put(t, p, top, Zone::Library);
    let soul = put(t, p, "Soul Summons", Zone::Hand);
    t.mana(p, "{1}{W}");
    t.g.act(p, paid("CW", Move::Cast { card: soul, targets: vec![] })).unwrap();
    let before = t.g.battlefield.clone();
    t.resolve();
    *t.g.battlefield.iter().find(|o| !before.contains(o)).expect("it entered")
}
/// Its controller turns it face up this way, paying with this mana.
pub(crate) fn turn_up(t: &mut T, o: ObjId, way: Way, mana: &str) {
    let p = t.g.controller(o);
    while t.g.priority != p { t.g.act(t.g.priority, Move::Pass).unwrap(); }
    t.mana(p, mana);
    t.g.act(p, paid(mana, Move::TurnFaceUp(o, way))).unwrap();
    assert!(!t.g.face_down(o), "turned face up");
}
fn names(g: &Game, p: usize) -> Vec<String> {
    let mut v: Vec<String> = g.battlefield.iter().filter(|o| g.controller(**o) == p).map(|o| g.obj(*o).chars.name.clone()).collect();
    v.sort();
    v
}

#[test]
fn r708_8_its_own_trigger_fires_as_the_special_action_turns_it_up() {
    let mut t = game(&["Dog Walker"], &[]);
    let o = face_down(&mut t, 0, "Dog Walker");
    assert_eq!(names(&t.g, 0), vec![""], "no Dogs as it enters face down");
    turn_up(&mut t, o, Way::Disguise, "{R}{W}");
    cite!(["603.3"]; assert_eq!(t.g.stack.len(), 1, "the trigger is put on the stack the next time a player would receive priority"));
    cite!(["603.3a"]; assert_eq!(t.g.controller(t.g.stack[0]), 0));
    t.resolve();
    cite!(["603.10"]; assert_eq!(names(&t.g, 0), vec!["Dog Token", "Dog Token", "Dog Walker"]));
    assert!(t.g.battlefield.iter().filter(|o| t.g.obj(**o).chars.name == "Dog Token").all(|o| t.g.obj(*o).tapped));
}

#[test]
fn r708_8_an_effect_turning_it_up_triggers_it_for_its_controller() {
    let mut t = game(&["Dog Walker", "Break Open"], &[]);
    let o = face_down(&mut t, 0, "Dog Walker");
    t.g.act(0, Move::Pass).unwrap();
    let open = put(&mut t, 1, "Break Open", Zone::Hand);
    t.mana(1, "{1}{R}{2}");
    t.g.act(1, paid("CR", Move::Cast { card: open, targets: vec![vec![Target::Object(o)]] })).unwrap();
    // the disguised Walker has ward {2}, which the opponent pays
    assert!(matches!(t.settle(), Boundary::Question(1, Asked::Pay { .. })));
    t.g.act(1, Move::Choose(Answer::Pay { plan: vec![], with: Some(mana_pool("CC")), tapped: vec![] })).unwrap();
    t.resolve();
    assert!(!t.g.face_down(o));
    cite!(["603.3a"]; assert_eq!(names(&t.g, 0), vec!["Dog Token", "Dog Token", "Dog Walker"], "its controller's Dogs"));
    cite!(["603.3a"]; assert!(names(&t.g, 1).is_empty()));
}

#[test]
fn r708_8_a_watcher_sees_any_permanent_turned_up_and_701_40g_an_instant_is_not() {
    let mut t = game(&["Aphetto Runecaster", "Glacial Stalker", "Soul Summons", "Lightning Bolt", "Break Open"], &[]);
    put(&mut t, 1, "Aphetto Runecaster", Zone::Battlefield);
    let o = face_down(&mut t, 0, "Glacial Stalker");
    let hand = t.g.players[1].hand.len();
    turn_up(&mut t, o, Way::Morph, "{4}{U}");
    cite!(["603.2"]; assert_eq!(t.g.stack.len(), 1, "the opponent's Runecaster triggers"));
    t.g.act(0, Move::Pass).unwrap();
    t.g.act(1, Move::Pass).unwrap();
    assert!(matches!(t.g.asked(), Some((1, Asked::May))), "{:?}", t.g.asked());
    t.g.act(1, Move::Choose(Answer::Yes)).unwrap();
    assert_eq!(t.g.players[1].hand.len(), hand + 1);
    // a manifested Lightning Bolt turned up by an effect is revealed and stays face down
    let bolt = manifest(&mut t, 0, "Lightning Bolt");
    t.g.act(0, Move::Pass).unwrap();
    let open = put(&mut t, 1, "Break Open", Zone::Hand);
    t.mana(1, "{1}{R}");
    t.g.act(1, paid("CR", Move::Cast { card: open, targets: vec![vec![Target::Object(bolt)]] })).unwrap();
    let from = t.mark();
    t.resolve();
    cite!(["701.40g"]; assert!(t.g.face_down(bolt)));
    assert!(!t.g.log.since(from).iter().any(|e| matches!(e, game::Event::TurnedFaceUp { .. })));
    cite!(["701.40g"]; assert!(t.g.stack.is_empty(), "nothing triggered (701.40g)"));
}

#[test]
fn r708_8_that_creature_is_the_one_turned_up_and_its_own_ability_sees_itself() {
    let mut t = game(&["Pine Walker", "Glacial Stalker"], &[]);
    let walker = face_down(&mut t, 0, "Pine Walker");
    let stalker = face_down(&mut t, 0, "Glacial Stalker");
    t.g.tap(walker);
    t.g.tap(stalker);
    turn_up(&mut t, walker, Way::Morph, "{4}{G}");
    cite!(["110.5c"]; assert!(t.g.obj(walker).tapped, "turning up does not untap it"));
    t.resolve();
    cite!(["603.10"]; assert!(!t.g.obj(walker).tapped, "this creature, turned face up, untaps itself"));
    turn_up(&mut t, stalker, Way::Morph, "{4}{U}");
    t.resolve();
    assert!(!t.g.obj(stalker).tapped, "another creature you control: that creature");
}

#[test]
fn r708_8_a_face_down_permanent_you_control_is_matched_as_it_was_before_it_turned() {
    let mut t = game(&["Sumala Sentry", "Glacial Stalker", "Secret Plans"], &[]);
    let sentry = put(&mut t, 0, "Sumala Sentry", Zone::Battlefield);
    put(&mut t, 0, "Secret Plans", Zone::Battlefield);
    let o = face_down(&mut t, 0, "Glacial Stalker");
    assert_eq!(t.g.toughness(o), Some(3), "Secret Plans: +0/+1 while face down");
    let hand = t.g.players[0].hand.len();
    turn_up(&mut t, o, Way::Morph, "{4}{U}");
    let Some((0, Asked::Order(triggered))) = t.g.asked() else { panic!("{:?}", t.g.asked()) };
    assert_eq!(triggered.len(), 2, "both trigger");
    t.g.act(0, Move::Order(vec![0, 1])).unwrap();
    t.resolve();
    assert_eq!(t.g.obj(o).counters.get("+1/+1"), Some(&1), "a face-down permanent was turned face up");
    assert_eq!(t.g.obj(sentry).counters.get("+1/+1"), Some(&1));
    assert_eq!(t.g.players[0].hand.len(), hand + 1, "Secret Plans draws");
    assert_eq!((t.g.power(o), t.g.toughness(o)), (Some(5), Some(6)));
}

#[test]
fn r708_8_if_its_a_creature_is_asked_of_the_face_up_permanent() {
    let mut t = game(&["Trail of Mystery", "Glacial Stalker", "Zoetic Cavern"], &[]);
    // both enter face down before the Trail, whose other ability would search for them
    let stalker = face_down(&mut t, 0, "Glacial Stalker");
    let cavern = face_down(&mut t, 0, "Zoetic Cavern");
    put(&mut t, 0, "Trail of Mystery", Zone::Battlefield);
    turn_up(&mut t, stalker, Way::Morph, "{4}{U}");
    t.resolve();
    cite!(["603.4"]; assert_eq!((t.g.power(stalker), t.g.toughness(stalker)), (Some(6), Some(7)), "+2/+2"));
    // a land turned face up is not a creature: the trigger is not put on the stack
    turn_up(&mut t, cavern, Way::Morph, "{2}");
    cite!(["603.4", "603.10"]; assert!(t.g.stack.is_empty(), "if it's a creature: it is a land"));
}

#[test]
fn r708_10_a_face_down_copy_turned_up_has_the_copied_trigger() {
    let mut t = game(&["Dog Walker", "Grizzly Bears", "Soul Summons"], &[
        ("Forever", "{0}", "Instant", "Target creature becomes a copy of target creature.", None),
    ]);
    let o = manifest(&mut t, 0, "Grizzly Bears");
    let walker = put(&mut t, 1, "Dog Walker", Zone::Battlefield);
    let forever = put(&mut t, 0, "Forever", Zone::Hand);
    t.g.act(0, Move::Cast { card: forever, targets: vec![vec![Target::Object(o)], vec![Target::Object(walker)]] }).unwrap();
    t.resolve();
    assert!(t.g.face_down(o));
    // turned up for the Bears' mana cost, it is a Dog Walker with its trigger (708.10)
    turn_up(&mut t, o, Way::Manifested, "{1}{G}");
    cite!(["708.10"]; assert_eq!(t.g.obj(o).chars.name, "Dog Walker"));
    t.resolve();
    cite!(["708.10"]; assert_eq!(names(&t.g, 0), vec!["Dog Token", "Dog Token", "Dog Walker"]));
}
