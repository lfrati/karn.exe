//! Megamorph (702.37b): a morph ability whose permanent, turned face up by paying its
//! megamorph cost, gets a +1/+1 counter as part of turning face up. Only paying that cost
//! does it: a manifested megamorph card turned up for its mana cost, or a megamorph
//! permanent turned up by an effect, gets none. The counter is put by the ordinary counter
//! operation and is on the permanent before anything triggered by its turning looks at it.
//! Salt Road Ambushers ({3}{G} 3/3, megamorph {3}{G}{G}), Ainok Survivalist ({1}{G} 2/1,
//! megamorph {1}{G}), Soul Summons and Break Open are Oracle cards.
use crate::support::cite;
use crate::support;
use game::announcement::CastCost;
use game::face::{Concealment, Way};
use support::*;

type Text<'a> = (&'a str, &'a str, &'a str, &'a str, Option<(i32, i32)>);

const SCALES_TEXT: &str = "If one or more +1/+1 counters would be put on a creature you control, that many plus one +1/+1 counters are put on it instead.";

/// Oracle cards by name, then test texts, with ten Wastes in each library.
fn game(names: &[&str], texts: &[Text]) -> T {
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
fn put(t: &mut T, p: usize, name: &str, zone: Zone) -> ObjId {
    let index = t.g.defs.iter().position(|d| d.name() == name).unwrap_or_else(|| panic!("no {name}"));
    t.g.put(p, index, zone)
}
/// Player 0 casts this card face down for {3} and it resolves: the face-down permanent.
fn face_down(t: &mut T, name: &str) -> ObjId {
    let card = put(t, 0, name, Zone::Hand);
    t.mana(0, "{3}");
    let row = t.g.def(card).unwrap().abilities().iter().position(|a| matches!(a,
        ir::Ability::Keywords(items) if items.iter().any(|i| i.keyword.name() == "Megamorph"))).unwrap();
    t.g.act(0, paid("CCC",Move::CastFor(CastCost::Alternative(row), Box::new(Move::Cast { card, targets: vec![] })))).unwrap();
    cite!(["702.37b"]; assert_eq!(t.g.face(*t.g.stack.last().unwrap()), game::face::Face::Down(game::face::Down { by: Concealment::Morph })));
    let before = t.g.battlefield.clone();
    t.resolve();
    *t.g.battlefield.iter().find(|o| !before.contains(o)).expect("it entered")
}
/// Player 0 manifests the top card of their library with Soul Summons, which resolves.
fn manifest(t: &mut T, top: &str) -> ObjId {
    put(t, 0, top, Zone::Library);
    let soul = put(t, 0, "Soul Summons", Zone::Hand);
    t.mana(0, "{1}{W}");
    t.g.act(0, paid("CW",Move::Cast { card: soul, targets: vec![] })).unwrap();
    let before = t.g.battlefield.clone();
    t.resolve();
    *t.g.battlefield.iter().find(|o| !before.contains(o)).expect("it entered")
}
fn turn_up(t: &mut T, o: ObjId, way: Way, mana: &str, spending: &str) {
    while t.g.priority != 0 { t.g.act(t.g.priority, Move::Pass).unwrap(); }
    t.mana(0, mana);
    t.g.act(0, paid(spending,Move::TurnFaceUp(o, way))).unwrap();
}
fn counters(t: &T, o: ObjId) -> u32 {
    t.g.obj(o).counters.get("+1/+1").copied().unwrap_or(0)
}

#[test]
fn r702_37b_paying_its_megamorph_cost_puts_a_counter_on_it_as_it_turns_up() {
    let mut t = game(&["Ainok Survivalist"], &[]);
    let o = face_down(&mut t, "Ainok Survivalist");
    assert_eq!(counters(&t, o), 0);
    let from = t.mark();
    turn_up(&mut t, o, Way::Morph, "{1}{G}", "CG");
    assert!(!t.g.face_down(o));
    cite!(["702.37b"]; assert_eq!(counters(&t, o), 1, "part of the special action: no priority between (ruling)"));
    assert_eq!((t.g.power(o), t.g.toughness(o)), (Some(3), Some(2)));
    let log = t.g.log.since(from);
    let up = log.iter().position(|e| matches!(e, game::Event::TurnedFaceUp { object } if *object == o)).expect("turned up");
    let put = log.iter().position(|e| matches!(e, game::Event::CountersPut { .. })).expect("counter put");
    cite!(["702.37b"]; assert!(up < put, "the face-up permanent gets it"));
}

#[test]
fn r702_37b_the_counter_is_put_by_the_ordinary_counter_operation() {
    let mut t = game(&["Ainok Survivalist"], &[("Hardened Scales", "{G}", "Enchantment", SCALES_TEXT, None)]);
    put(&mut t, 0, "Hardened Scales", Zone::Battlefield);
    let o = face_down(&mut t, "Ainok Survivalist");
    turn_up(&mut t, o, Way::Morph, "{1}{G}", "CG");
    cite!(["702.37b", "614.1a"]; assert_eq!(counters(&t, o), 2, "that many plus one"));
}

#[test]
fn r702_37b_a_manifested_megamorph_card_gets_one_only_for_its_megamorph_cost() {
    // Ruling: turned face up for its mana cost, it gets no counter (701.40b); for its
    // megamorph cost (701.40c), it does.
    let mut t = game(&["Ainok Survivalist", "Soul Summons"], &[]);
    let paid_mana = manifest(&mut t, "Ainok Survivalist");
    turn_up(&mut t, paid_mana, Way::Manifested, "{1}{G}", "CG");
    assert!(!t.g.face_down(paid_mana));
    cite!(["701.40b", "702.37b"]; assert_eq!(counters(&t, paid_mana), 0, "its mana cost is not its megamorph cost"));
    let paid_megamorph = manifest(&mut t, "Ainok Survivalist");
    turn_up(&mut t, paid_megamorph, Way::Morph, "{1}{G}", "CG");
    cite!(["701.40c", "702.37b"]; assert_eq!(counters(&t, paid_megamorph), 1));
}

#[test]
fn r702_37b_turned_up_by_an_effect_it_gets_none() {
    let mut t = game(&["Ainok Survivalist", "Break Open"], &[]);
    let o = face_down(&mut t, "Ainok Survivalist");
    t.g.act(0, Move::Pass).unwrap();
    let open = put(&mut t, 1, "Break Open", Zone::Hand);
    t.mana(1, "{1}{R}");
    t.g.act(1, paid("CR",Move::Cast { card: open, targets: vec![vec![Target::Object(o)]] })).unwrap();
    t.resolve();
    assert!(!t.g.face_down(o));
    cite!(["702.37b"]; assert_eq!(counters(&t, o), 0, "no megamorph cost was paid"));
}

#[test]
fn r702_37b_what_triggers_on_its_turning_sees_the_counter() {
    let mut t = game(&["Salt Road Ambushers"], &[(
        "Counter Watcher", "{1}", "Enchantment",
        "Whenever a permanent you control is turned face up, if it has a +1/+1 counter on it, you gain 3 life.", None,
    )]);
    put(&mut t, 0, "Salt Road Ambushers", Zone::Battlefield);
    put(&mut t, 0, "Counter Watcher", Zone::Battlefield);
    // another Ambushers: its own ability watches only other permanents
    let o = face_down(&mut t, "Salt Road Ambushers");
    let life = t.g.players[0].life;
    turn_up(&mut t, o, Way::Morph, "{3}{G}{G}", "CCCGG");
    let Some((0, Asked::Order(triggered))) = t.g.asked() else { panic!("{:?}", t.g.asked()) };
    cite!(["702.37b", "603.4"]; assert_eq!(triggered.len(), 2, "the Ambushers and the intervening if, which the counter satisfies"));
    t.g.act(0, Move::Order(vec![0, 1])).unwrap();
    t.resolve();
    assert_eq!(counters(&t, o), 3, "one for megamorph, two from the Ambushers");
    cite!(["603.4"]; assert_eq!(t.g.players[0].life, life + 3));
    assert_eq!((t.g.power(o), t.g.toughness(o)), (Some(6), Some(6)));
}

#[test]
fn r702_37b_a_megamorph_permanent_has_a_morph_ability() {
    // "Megamorph is a variant of the morph ability" (ruling): Backslide's target. Turned face
    // down and up again for its megamorph cost, it gets another counter.
    let mut t = game(&["Ainok Survivalist", "Backslide"], &[]);
    let o = face_down(&mut t, "Ainok Survivalist");
    turn_up(&mut t, o, Way::Morph, "{1}{G}", "CG");
    t.resolve();
    let backslide = put(&mut t, 0, "Backslide", Zone::Hand);
    t.mana(0, "{1}{U}");
    t.g.act(0, paid("CU",Move::Cast { card: backslide, targets: vec![vec![Target::Object(o)]] })).unwrap();
    t.resolve();
    assert!(t.g.face_down(o));
    cite!(["708.2a"]; assert_eq!((t.g.power(o), t.g.toughness(o)), (Some(3), Some(3)), "a 2/2 with its counter"));
    turn_up(&mut t, o, Way::Morph, "{1}{G}", "CG");
    cite!(["702.37b"]; assert_eq!(counters(&t, o), 2));
}
