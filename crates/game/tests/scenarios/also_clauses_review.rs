//! "Also" clauses lowered as plain acts: "If [condition],
//! that creature also gains ...", "It also gets +1/+0 ... if [condition]", "Kithkin creatures
//! you control also gain ...". The "also" part happens only when its own condition holds, and
//! only to what it names; what comes before it happens either way (608.2c: the instructions
//! are followed in order, each as written).
//!
//! The cards under test are their Oracle text; the creatures of a type are test cards.
use crate::support::cite;
use crate::support;
use support::*;

fn game(names: &[&str], extra: Vec<CardDef>) -> T {
    let mut defs: Vec<CardDef> = ["Wastes"].iter().chain(names).map(|n| oracle(n)).collect();
    defs.extend(extra);
    for d in &defs { game::play::check(d).unwrap_or_else(|e| panic!("{}: {}", d.name(), e.0)); }
    let mut g = Game::new(defs, 2, 20);
    for p in 0..2 { for _ in 0..10 { g.put(p, 0, Zone::Library); } }
    let mut t = T::new(g);
    t.to_no_combat(0, Part::PrecombatMain);
    t
}
fn creature(name: &str, subtype: &str) -> CardDef {
    CardDef::new(name, "{1}", &format!("Creature — {subtype}"), "", Some((2, 2))).unwrap()
}
fn put(t: &mut T, p: usize, name: &str, zone: Zone) -> ObjId {
    let index = t.g.defs.iter().position(|d| d.name() == name).unwrap_or_else(|| panic!("no {name}"));
    t.g.put(p, index, zone)
}
fn resolve_all(t: &mut T) {
    while !t.g.stack.is_empty() { t.g.act(t.g.priority, Move::Pass).unwrap(); }
}
fn cast(t: &mut T, card: ObjId, targets: Vec<ObjId>, spending: &str) {
    t.g.act(0, paid(spending, Move::Cast { card, targets: targets.into_iter().map(|o| vec![Target::Object(o)]).collect() })).unwrap();
    resolve_all(t);
}

// Rules: 608.2c
// Given: Might of the Meek ("Target creature gains trample until end of turn. It also gets
// +1/+0 until end of turn if you control a Mouse. Draw a card."), with and without a Mouse.
// Expect: trample and the draw either way; +1/+0 only with a Mouse.
// Why: the trailing "if you control a Mouse" conditions the "also" sentence only.
#[test]
fn might_of_the_meek_also_gets_plus_one_only_if_you_control_a_mouse() {
    for mouse in [false, true] {
        let mut t = game(&["Might of the Meek", "Grizzly Bears"], vec![creature("Test Mouse", "Mouse")]);
        let bears = put(&mut t, 0, "Grizzly Bears", Zone::Battlefield);
        if mouse { put(&mut t, 0, "Test Mouse", Zone::Battlefield); }
        let might = put(&mut t, 0, "Might of the Meek", Zone::Hand);
        let hand = t.g.players[0].hand.len();
        t.mana(0, "{R}");
        cast(&mut t, might, vec![bears], "R");
        cite!(["608.2c"]; case: mouse; assert!(t.g.has(bears, "Trample")));
        cite!(["608.2c"]; case: mouse; assert_eq!((t.g.power(bears), t.g.toughness(bears)), (Some(2 + i32::from(mouse)), Some(2))));
        cite!(["608.2c"]; case: mouse; assert_eq!(t.g.players[0].hand.len(), hand, "cast one, drew one"));
    }
}

// Rules: 608.2c
// Given: Gallant Fowlknight (a Kithkin Knight) cast with a Bears and a Kithkin on the field:
// "creatures you control get +1/+0 until end of turn. Kithkin creatures you control also gain
// first strike until end of turn."
// Expect: all three get +1/+0; first strike only for the Kithkin and the Fowlknight.
// Why: the "also" sentence has its own subject, Kithkin creatures you control.
#[test]
fn gallant_fowlknight_also_gives_first_strike_to_kithkin_only() {
    let mut t = game(&["Gallant Fowlknight", "Grizzly Bears"], vec![creature("Test Kithkin", "Kithkin")]);
    let bears = put(&mut t, 0, "Grizzly Bears", Zone::Battlefield);
    let kithkin = put(&mut t, 0, "Test Kithkin", Zone::Battlefield);
    let knight = put(&mut t, 0, "Gallant Fowlknight", Zone::Hand);
    t.mana(0, "{3}{W}");
    cast(&mut t, knight, vec![], "CCCW");
    resolve_all(&mut t);
    let knight = t.find_in_battlefield("Gallant Fowlknight");
    cite!(["608.2c"]; assert_eq!((t.g.power(bears), t.g.power(kithkin), t.g.power(knight)), (Some(3), Some(3), Some(4))));
    cite!(["608.2c"]; assert!(!t.g.has(bears, "First strike"), "the Bears is no Kithkin"));
    cite!(["608.2c"]; assert!(t.g.has(kithkin, "First strike") && t.g.has(knight, "First strike")));
}

// Rules: 608.2c
// Given: Goatnap ("Gain control of target creature until end of turn. Untap that creature. It
// gains haste until end of turn. If that creature is a Goat, it also gets +3/+0 until end of
// turn.") at an opponent's tapped Bears, and at an opponent's tapped Goat.
// Expect: control, untapped, haste either way; +3/+0 only for the Goat.
#[test]
fn goatnap_also_gives_plus_three_only_to_a_goat() {
    for (name, power) in [("Grizzly Bears", 2), ("Test Goat", 5)] {
        let mut t = game(&["Goatnap", "Grizzly Bears"], vec![creature("Test Goat", "Goat")]);
        let target = put(&mut t, 1, name, Zone::Battlefield);
        t.g.tap(target);
        let goatnap = put(&mut t, 0, "Goatnap", Zone::Hand);
        t.mana(0, "{2}{R}");
        cast(&mut t, goatnap, vec![target], "CCR");
        cite!(["608.2c"]; case: name; assert_eq!((t.g.controller(target), t.g.obj(target).tapped, t.g.has(target, "Haste")), (0, false, true)));
        cite!(["608.2c"]; case: name; assert_eq!((t.g.power(target), t.g.toughness(target)), (Some(power), Some(2))));
    }
}

// Rules: 608.2c, 702.34a
// Given: Moment of Glory ("Put a +1/+1 counter on target creature you control. If this spell
// was cast from a graveyard, also put a +1/+1 counter on each other creature you control.")
// cast from the hand, then from the graveyard for its flashback cost.
// Expect: from the hand only the target gets a counter; from the graveyard every creature does.
#[test]
fn moment_of_glory_also_puts_counters_on_the_others_only_when_cast_from_a_graveyard() {
    for from_graveyard in [false, true] {
        let mut t = game(&["Moment of Glory", "Grizzly Bears", "Hill Giant"], vec![]);
        let bears = put(&mut t, 0, "Grizzly Bears", Zone::Battlefield);
        let giant = put(&mut t, 0, "Hill Giant", Zone::Battlefield);
        let card = put(&mut t, 0, "Moment of Glory", if from_graveyard { Zone::Graveyard } else { Zone::Hand });
        let targets = vec![vec![Target::Object(bears)]];
        if from_graveyard {
            t.mana(0, "{4}{W}");
            t.g.act(0, paid("CCCCW", Move::CastVia(game::announcement::CastRoute::Flashback, Box::new(Move::Cast { card, targets })))).unwrap();
        } else {
            t.mana(0, "{W}");
            t.g.act(0, paid("W", Move::Cast { card, targets })).unwrap();
        }
        resolve_all(&mut t);
        let count = |o: ObjId| t.g.obj(o).counters.get("+1/+1").copied().unwrap_or(0);
        cite!(["608.2c"]; case: from_graveyard; assert_eq!((count(bears), count(giant)), (1, u32::from(from_graveyard))));
    }
}

// Rules: 608.2c, 702.174j
// Given: Blooming Blast ("Blooming Blast deals 2 damage to target creature. If the gift was
// promised, Blooming Blast also deals 3 damage to that creature's controller.") at player 1's
// Hill Giant (3/3), without and with the gift promised to player 1.
// Expect: 2 damage to the Giant either way; player 1 loses 3 life (and gets a Treasure) only
// with the gift.
#[test]
fn blooming_blast_also_damages_the_controller_only_if_the_gift_was_promised() {
    for gift in [false, true] {
        let mut t = game(&["Blooming Blast", "Hill Giant"], vec![]);
        let giant = put(&mut t, 1, "Hill Giant", Zone::Battlefield);
        let blast = put(&mut t, 0, "Blooming Blast", Zone::Hand);
        t.mana(0, "{1}{R}");
        let cast = Move::Cast { card: blast, targets: vec![vec![Target::Object(giant)]] };
        t.g.act(0, paid("CR", if gift { Move::GiftTo(1, Box::new(cast)) } else { cast })).unwrap();
        resolve_all(&mut t);
        cite!(["608.2c"]; case: gift; assert_eq!(t.g.obj(giant).damage, 2));
        cite!(["608.2c", "702.174j"]; case: gift; assert_eq!(t.g.players[1].life, if gift { 17 } else { 20 }));
        cite!(["702.174j"]; case: gift; assert_eq!(t.g.battlefield.iter().filter(|o| t.g.obj(**o).token()).count(), usize::from(gift)));
    }
}

// Rules: 608.2c
// Given: Efflorescence ("Put two +1/+1 counters on target creature. Infusion — If you gained
// life this turn, that creature also gains trample and indestructible until end of turn."),
// without and after gaining life this turn.
// Expect: two counters either way; trample and indestructible only after gaining life.
#[test]
fn efflorescence_also_gives_trample_and_indestructible_only_if_you_gained_life() {
    let salve = CardDef::new("Salve", "{0}", "Instant", "You gain 1 life.", None).unwrap();
    for gained in [false, true] {
        let mut t = game(&["Efflorescence", "Grizzly Bears"], vec![salve.clone()]);
        let bears = put(&mut t, 0, "Grizzly Bears", Zone::Battlefield);
        if gained {
            let s = put(&mut t, 0, "Salve", Zone::Hand);
            cast(&mut t, s, vec![], "");
            assert_eq!(t.g.players[0].life, 21);
        }
        let card = put(&mut t, 0, "Efflorescence", Zone::Hand);
        t.mana(0, "{2}{G}");
        cast(&mut t, card, vec![bears], "CCG");
        cite!(["608.2c"]; case: gained; assert_eq!(t.g.obj(bears).counters.get("+1/+1").copied(), Some(2)));
        cite!(["608.2c"]; case: gained; assert_eq!((t.g.has(bears, "Trample"), t.g.has(bears, "Indestructible")), (gained, gained)));
    }
}

// Rules: 608.2c, 601.2h
// Given: Expressive Firedancer ("Whenever you cast an instant or sorcery spell, this creature
// gets +1/+1 until end of turn. If five or more mana was spent to cast that spell, this
// creature also gains double strike until end of turn."), with an instant costing {4}, then
// one costing {5}.
// Expect: +1/+1 for each; double strike only after the {5} one.
#[test]
fn expressive_firedancer_also_gains_double_strike_only_for_five_mana_spent() {
    let four = CardDef::new("Four", "{4}", "Instant", "You gain 1 life.", None).unwrap();
    let five = CardDef::new("Five", "{5}", "Instant", "You gain 1 life.", None).unwrap();
    let mut t = game(&["Expressive Firedancer"], vec![four, five]);
    let dancer = put(&mut t, 0, "Expressive Firedancer", Zone::Battlefield);
    let four = put(&mut t, 0, "Four", Zone::Hand);
    t.mana(0, "{4}");
    cast(&mut t, four, vec![], "CCCC");
    cite!(["608.2c"]; case: "four"; assert_eq!(t.g.power(dancer), Some(3)));
    cite!(["608.2c", "601.2h"]; case: "four"; assert!(!t.g.has(dancer, "Double strike")));
    let five = put(&mut t, 0, "Five", Zone::Hand);
    t.mana(0, "{5}");
    cast(&mut t, five, vec![], "CCCCC");
    cite!(["608.2c"]; case: "five"; assert_eq!(t.g.power(dancer), Some(4)));
    cite!(["608.2c", "601.2h"]; case: "five"; assert!(t.g.has(dancer, "Double strike")));
}

// Rules: 608.2c
// Given: Yip Yip! ("Target creature you control gets +2/+2 until end of turn. If that creature
// is an Ally, it also gains flying until end of turn.") at a Bears, and at an Ally.
// Expect: +2/+2 either way; flying only for the Ally.
#[test]
fn yip_yip_also_gives_flying_only_to_an_ally() {
    for name in ["Grizzly Bears", "Test Ally"] {
        let mut t = game(&["Yip Yip!", "Grizzly Bears"], vec![creature("Test Ally", "Human Ally")]);
        let target = put(&mut t, 0, name, Zone::Battlefield);
        let card = put(&mut t, 0, "Yip Yip!", Zone::Hand);
        t.mana(0, "{W}");
        cast(&mut t, card, vec![target], "W");
        cite!(["608.2c"]; case: name; assert_eq!(t.g.power(target), Some(4)));
        cite!(["608.2c"]; case: name; assert_eq!(t.g.has(target, "Flying"), name == "Test Ally"));
    }
}
