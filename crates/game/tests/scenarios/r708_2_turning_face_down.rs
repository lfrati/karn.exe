//! Turning a face-up permanent face down (708.2a): an effect that lists no characteristics
//! makes it a 2/2 face-down creature with no text, no name, no subtypes and no mana cost.
//! It is the same permanent, with a new timestamp (613.7f); its counters, damage and tapped
//! status stay. A face-down permanent (708.2b) or a double-faced one (712.16) is not turned,
//! and nothing happens. It is turned face up again only by what it is face up: its morph
//! cost, or not at all without one. An effect that lists characteristics is refused by name.
//! Backslide, Wall of Deceit, Obscuring Aether, Pine Walker, Glacial Stalker and Grizzly
//! Bears are Oracle cards.
use crate::support::{cite, paid};
use game::announcement::CastCost;
use crate::support;
use game::face::{Concealment, Way};
use ir::CardType;
use support::*;

type Text<'a> = (&'a str, &'a str, &'a str, &'a str, Option<(i32, i32)>);

const TURN: Text = ("Turn Down", "{0}", "Instant", "Turn target creature face down.", None);

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
/// Player 0 casts this spell from their hand at this target with this mana, and it resolves.
fn cast_at(t: &mut T, name: &str, mana: &str, target: ObjId) {
    let card = put(t, 0, name, Zone::Hand);
    t.mana(0, mana);
    t.g.act(0, paid(mana, Move::Cast { card, targets: vec![vec![Target::Object(target)]] })).unwrap();
    t.resolve();
}

fn by(t: &T, o: ObjId) -> Option<Concealment> {
    match t.g.face(o) { game::face::Face::Down(down) => Some(down.by), game::face::Face::Up => None }
}

#[test]
fn r708_2a_it_becomes_a_nameless_2_2_and_is_the_same_permanent() {
    let mut t = game(&["Backslide", "Pine Walker"], &[]);
    let walker = put(&mut t, 0, "Pine Walker", Zone::Battlefield);
    t.g.set_initial_counters(walker, "+1/+1", 1);
    t.g.tap(walker);
    let from = t.mark();
    cast_at(&mut t, "Backslide", "{1}{U}", walker);
    assert_eq!(by(&t, walker), Some(Concealment::TurnedDown));
    let o = t.g.obj(walker);
    cite!(["708.2a"]; assert_eq!(o.chars.name, ""));
    cite!(["708.2a"]; assert!(o.chars.subtypes.is_empty() && t.g.def(walker).and_then(|d| d.cost()).is_none()));
    cite!(["708.2a"]; assert_eq!((t.g.power(walker), t.g.toughness(walker)), (Some(3), Some(3)), "a 2/2 with its +1/+1 counter"));
    assert!(t.g.obj(walker).tapped, "still tapped");
    cite!(["708.2a"]; assert!(t.g.abilities(walker).is_empty(), "no text"));
    assert!(t.g.log.since(from).iter().any(|e| matches!(e, game::Event::TurnedFaceDown { object } if *object == walker)));
    // only its controller may look at it (708.5)
    cite!(["708.5"]; assert!(t.g.sees(0, walker) && !t.g.sees(1, walker)));
    let theirs = game::snapshot::Snapshot::of(&t.g, Some(1)).objects.into_iter().find(|o| o.id == walker).expect("perceived");
    assert_eq!(theirs.face_down.as_ref().map(|d| (d.by, d.card.is_none())), Some((Concealment::TurnedDown, true)));
}

#[test]
fn r708_2a_with_morph_face_up_it_is_turned_up_for_its_morph_cost_and_triggers() {

    let mut t = game(&["Backslide", "Pine Walker"], &[]);
    let walker = put(&mut t, 0, "Pine Walker", Zone::Battlefield);
    cast_at(&mut t, "Backslide", "{1}{U}", walker);
    cite!(["118.3a"]; refused("that mana is not in your mana pool"): unchanged(&mut t.g,paid("CCCCG", Move::TurnFaceUp(walker, Way::Morph))));
    t.mana(0, "{4}{G}");
    cite!(["701.40b"]; refused("only a manifested permanent is turned face up for its card's mana cost"): unchanged(&mut t.g,paid("CCCCG",Move::TurnFaceUp(walker, Way::Manifested))));
    t.g.tap(walker);
    t.g.act(0, paid("CCCCG", Move::TurnFaceUp(walker, Way::Morph))).unwrap();
    cite!(["702.37e"]; assert_eq!(t.g.obj(walker).chars.name, "Pine Walker"));
    t.resolve();
    assert!(!t.g.obj(walker).tapped, "its turned-face-up trigger untapped it");
}

#[test]
fn r708_2a_without_morph_it_has_no_way_to_turn_face_up() {

    // Obscuring Aether's ruling: once face down, it can't be turned face up.
    let mut t = game(&["Obscuring Aether"], &[]);
    let aether = put(&mut t, 0, "Obscuring Aether", Zone::Battlefield);
    t.mana(0, "{1}{G}");
    t.activate(0, "Obscuring Aether", 1, vec![], crate::support::mana_pool("CG")).unwrap();
    t.resolve();
    assert_eq!(by(&t, aether), Some(Concealment::TurnedDown));
    cite!(["708.2a"]; assert!(t.g.obj(aether).chars.types.contains(&CardType::Creature), "it becomes a 2/2 creature"));
    assert!(!t.g.obj(aether).chars.types.contains(&CardType::Enchantment));
    cite!(["708.2a"]; assert_eq!((t.g.power(aether), t.g.toughness(aether)), (Some(2), Some(2))));
    t.mana(0, "{G}{G}{G}{G}{G}{G}");
    cite!(["702.37e"]; refused("that permanent would have no such cost if it were face up (702.37e)"): unchanged(&mut t.g,paid("GGGGGG",Move::TurnFaceUp(aether, Way::Morph))));
}

#[test]
fn r708_2a_its_own_ability_turns_it_down_and_its_morph_cost_turns_it_back() {

    let mut t = game(&["Wall of Deceit"], &[]);
    let wall = put(&mut t, 0, "Wall of Deceit", Zone::Battlefield);
    t.mana(0, "{3}");
    t.activate(0, "Wall of Deceit", 1, vec![], crate::support::mana_pool("CCC")).unwrap();
    t.resolve();
    cite!(["708.2a"]; assert_eq!((t.g.power(wall), t.g.toughness(wall)), (Some(2), Some(2)), "it loses defender and is a 2/2"));
    t.mana(0, "{U}");
    cite!(["701.40b"]; refused("only a manifested permanent is turned face up for its card's mana cost"): unchanged(&mut t.g,paid("U",Move::TurnFaceUp(wall, Way::Manifested))));
    t.g.act(0, paid("U", Move::TurnFaceUp(wall, Way::Morph))).unwrap();
    cite!(["702.37e"]; assert_eq!((t.g.power(wall), t.g.toughness(wall)), (Some(0), Some(5))));
}

#[test]
fn r708_2b_a_face_down_permanent_is_not_turned_again() {
    let mut t = game(&["Glacial Stalker"], &[TURN]);
    let card = put(&mut t, 0, "Glacial Stalker", Zone::Hand);
    t.mana(0, "{3}");
    let cost = CastCost::Alternative(0); // Glacial Stalker prints Morph first.
    t.g.act(0, paid("CCC", Move::CastFor(cost, Box::new(Move::Cast { card, targets: vec![] })))).unwrap();
    t.resolve();
    let stalker = *t.g.battlefield.last().unwrap();
    let stamp = t.g.obj(stalker).stamp;
    let from = t.mark();
    cast_at(&mut t, "Turn Down", "", stalker);
    cite!(["708.2b"]; assert_eq!(by(&t, stalker), Some(Concealment::Morph), "nothing happens: still a morph"));
    cite!(["708.2b"]; assert_eq!(t.g.obj(stalker).stamp, stamp, "no new timestamp"));
    assert!(!t.g.log.since(from).iter().any(|e| matches!(e, game::Event::TurnedFaceDown { .. })));
}

#[test]
fn r712_16_a_double_faced_permanent_is_not_turned_face_down() {
    let front = CardDef::new("Daybound Wolf", "{1}{G}", "Creature — Wolf", "", Some((2, 2))).unwrap();
    let back = CardDef::new("Nightbound Wolf", "", "Creature — Wolf", "", Some((4, 4))).unwrap();
    let dfc = CardDef::transforming(front, back).unwrap();
    let turn = CardDef::new(TURN.0, TURN.1, TURN.2, TURN.3, TURN.4).unwrap();
    let mut g = Game::new(vec![dfc, turn, oracle("Grizzly Bears")], 2, 20);
    let wolf = g.put(0, 0, Zone::Battlefield);
    let bears = g.put(0, 2, Zone::Battlefield);
    let mut t = T::new(g);
    let spell = t.g.put(0, 1, Zone::Hand);
    t.g.act(0, Move::Cast { card: spell, targets: vec![vec![Target::Object(wolf)]] }).unwrap();
    t.resolve();
    cite!(["712.16"]; assert!(!t.g.face_down(wolf), "nothing happens"));
    cite!(["712.16"]; assert_eq!(t.g.obj(wolf).chars.name, "Daybound Wolf"));
    let spell = t.g.put(0, 1, Zone::Hand);
    t.g.act(0, Move::Cast { card: spell, targets: vec![vec![Target::Object(bears)]] }).unwrap();
    t.resolve();
    cite!(["708.2a"]; assert!(t.g.face_down(bears), "a single-faced one is"));
}

#[test]
fn r708_2_an_effect_that_lists_characteristics_is_refused_by_name() {
    for name in ["Cyber Conversion", "Plant Tadpoles", "Illithid Harvester"] {
        let refused = game::play::check(&oracle_read(name)).expect_err(name).0;
        assert!(refused.starts_with("turn face down: characteristics the effect lists"), "{name}: {refused}");
    }
}

#[track_caller]
fn unchanged(g: &mut Game, action: Move) -> Result<(), String> {
    let before=format!("{g:?}"); let result=g.act(0,action);
    assert_eq!(format!("{g:?}"),before); result
}
