//! A layer world that holds only what an effect can change answers as the world of every
//! card of every zone does (613.1): for each object, wherever it is, at every cut, alone
//! and within one read of the game.
use super::{HELD, LAYERS, Layer, Seen, WHOLE_WORLD};
use crate::card::CardDef;
use crate::event::Target;
use crate::legal::Asked;
use crate::play::Move;
use crate::state::{Game, ObjId, Zone};

const NAMES: [&str; 17] = [
    "Bear", "Hawk", "Rock", "Goyf", "Mortal",
    "Anthem", "Fervor", "Humility", "Animation", "Teacher", "Dark", "Night", "Hoard",
    "Blacken", "Jump", "Shrink", "Strip",
];
fn def(name: &str) -> usize { NAMES.iter().position(|n| *n == name).unwrap_or_else(|| panic!("{name}")) }

fn defs() -> Vec<CardDef> {
    let card = |name: &str, types, text, pt| {
        assert_eq!(NAMES[def(name)], name);
        CardDef::new(name, "{0}", types, text, pt).unwrap()
    };
    let defs = vec![
        card("Bear", "Creature — Bear", "", Some((2, 2))),
        card("Hawk", "Creature — Bird", "Flying", Some((1, 1))),
        card("Rock", "Artifact", "", None),
        // defined in every zone (604.3), by what is on the battlefield and by what is not
        card("Goyf", "Creature — Lhurgoyf", "This creature's power and toughness are each equal to the number of creatures you control.", Some((0, 0))),
        card("Mortal", "Creature — Lhurgoyf", "This creature's power and toughness are each equal to the number of creature cards in your graveyard.", Some((0, 0))),
        card("Anthem", "Enchantment", "Creatures you control get +1/+1.", None),
        card("Fervor", "Enchantment", "Creatures you control have haste.", None),
        card("Humility", "Enchantment", "All creatures lose all abilities and have base power and toughness 1/1.", None),
        card("Animation", "Enchantment", "Noncreature artifacts become 3/3 artifact creatures.", None),
        card("Teacher", "Enchantment", "Creatures you control have \"Other creatures you control get +1/+1.\"", None),
        card("Dark", "Enchantment", "Black creatures get +1/+1.", None),
        // a color said of what is described: of cards in every zone it describes
        card("Night", "Enchantment", "Noncreature artifacts become 2/2 black artifact creatures.", None),
        // reads cards no effect changes
        card("Hoard", "Enchantment", "Creatures you control get +1/+1 for each creature card in your graveyard.", None),
        card("Blacken", "Instant", "Target creature becomes black until end of turn.", None),
        card("Jump", "Instant", "Target creature gains flying until end of turn.", None),
        card("Shrink", "Instant", "Target creature has base power and toughness 1/1 until end of turn.", None),
        card("Strip", "Instant", "Target permanent loses all abilities until end of turn.", None),
    ];
    for d in &defs {
        crate::play::check(d).unwrap_or_else(|u| panic!("{}: {}", d.name(), u.0));
    }
    defs
}

/// Cards in every zone of both players, and `pieces` on the battlefield in that order.
fn board(pieces: &[&str]) -> (Game, Vec<ObjId>) {
    let mut g = Game::new(defs(), 2, 20);
    let mut objects = Vec::new();
    for player in 0..2 {
        for zone in [Zone::Library, Zone::Hand, Zone::Graveyard, Zone::Exile] {
            for name in ["Bear", "Goyf", "Mortal", "Rock"] { objects.push(g.put(player, def(name), zone)); }
        }
        for name in ["Bear", "Hawk", "Rock", "Goyf", "Mortal"] { objects.push(g.put(player, def(name), Zone::Battlefield)); }
    }
    let (bear, goyf) = (objects[16], objects[19]);
    for name in pieces {
        match *name {
            "Blacken" | "Jump" | "Shrink" | "Strip" => cast(&mut g, name, bear),
            "Strip goyf" => cast(&mut g, "Strip", goyf),
            name => objects.push(g.put(0, def(name), Zone::Battlefield)),
        }
    }
    (g, objects)
}
fn cast(g: &mut Game, name: &str, target: ObjId) {
    let card = g.put(0, def(name), Zone::Hand);
    g.act(0, Move::Cast { card, targets: vec![vec![Target::Object(target)]] }).unwrap_or_else(|e| panic!("{name}: {e}"));
    while g.asked().is_some_and(|(_, a)| a == Asked::Priority) && !g.stack.is_empty() { g.act(g.priority, Move::Pass).unwrap(); }
}

fn cuts() -> Vec<Option<Layer>> { std::iter::once(None).chain(LAYERS.into_iter().map(Some)).collect() }

/// Every object at every cut, each asked alone.
fn alone(g: &Game, objects: &[ObjId], whole: bool) -> Vec<Seen> {
    WHOLE_WORLD.with(|w| w.set(whole));
    let views = cuts().into_iter().flat_map(|cut| objects.iter().map(move |o| g.seen(*o, cut))).collect();
    WHOLE_WORLD.with(|w| w.set(false));
    views
}

/// Every object at every cut, within one read of the game, asked from either end.
fn shared(g: &Game, objects: &[ObjId], backwards: bool) -> Vec<Seen> {
    cuts().into_iter().flat_map(|cut| {
        let mut order: Vec<_> = objects.to_vec();
        if backwards { order.reverse(); }
        let mut views: Vec<_> = order.iter().map(|o| g.seen(*o, cut)).collect();
        if backwards { views.reverse(); }
        views
    }).collect()
}

fn agree(g: &Game, objects: &[ObjId], at: &str) {
    let whole = alone(g, objects, true);
    let asked = objects.len() * cuts().len();
    assert_eq!(whole.len(), asked);
    for (how, held) in [("alone", alone(g, objects, false)), ("shared", shared(g, objects, false)), ("shared backwards", shared(g, objects, true))] {
        for (i, (a, b)) in held.iter().zip(&whole).enumerate() {
            let o = g.obj(objects[i % objects.len()]);
            assert_eq!(a, b, "{} in {:?} at cut {:?}, {how}, on {at}", o.chars.name, o.zone, cuts()[i / objects.len()]);
        }
    }
}

const PIECES: [&str; 13] = ["Anthem", "Fervor", "Humility", "Animation", "Teacher", "Dark", "Night", "Hoard", "Blacken", "Jump", "Shrink", "Strip", "Strip goyf"];

#[test]
fn r613_1_a_world_of_what_effects_can_change_answers_as_the_world_of_every_card() {
    let (g, objects) = board(&[]);
    agree(&g, &objects, "nothing");
    for a in 0..PIECES.len() {
        for b in a..PIECES.len() {
            let mut pieces = vec![PIECES[a]];
            if b != a { pieces.push(PIECES[b]); }
            let (g, objects) = board(&pieces);
            agree(&g, &objects, &format!("{pieces:?}"));
        }
    }
    let (g, objects) = board(&PIECES);
    agree(&g, &objects, "everything");
}

#[test]
fn r613_1_a_permanent_about_to_enter_is_asked_in_the_same_world() {
    for pieces in [&["Anthem", "Fervor"][..], &["Humility", "Teacher", "Jump"], &["Night", "Dark"], &["Hoard", "Animation", "Shrink"]] {
        let (mut g, mut objects) = board(pieces);
        // a Bear from a hand and a Goyf from a library
        for entrant in [objects[4], objects[1]] {
            g.prospective = Some((entrant, None));
            agree(&g, &objects, &format!("{pieces:?} entering"));
        }
        // and as the spell it was on the stack
        let was = g.put(0, def("Goyf"), Zone::Stack);
        objects.push(was);
        g.prospective = Some((objects[5], Some(was)));
        agree(&g, &objects, &format!("{pieces:?} entering from a spell"));
    }
}

#[test]
fn r613_1_what_is_asked_from_outside_the_zones_a_world_holds_joins_it() {
    let black = ir::Color::Black;
    for pieces in [&["Blacken", "Jump", "Anthem"][..], &["Blacken", "Shrink", "Fervor", "Dark"], &["Blacken", "Strip", "Hoard"]] {
        // a permanent the battlefield does not list: the effects that name it still apply
        let (mut g, objects) = board(pieces);
        let bear = objects[16];
        g.battlefield.retain(|o| *o != bear);
        agree(&g, &objects, &format!("{pieces:?}, unlisted permanent"));
        assert!(g.seen(bear, None).colors.contains(&black), "{pieces:?}");
        // a spell the stack does not list
        let (mut g, mut objects) = board(pieces);
        let spell = g.put(0, def("Goyf"), Zone::Stack);
        g.stack.retain(|o| *o != spell);
        objects.push(spell);
        agree(&g, &objects, &format!("{pieces:?}, unlisted spell"));
        // a card in the command zone a resolved effect colors (the layer gives a color anywhere)
        let (mut g, objects) = board(pieces);
        g.battlefield.retain(|o| *o != bear);
        g.obj_mut(bear).zone = Zone::Command;
        agree(&g, &objects, &format!("{pieces:?}, command zone"));
        assert!(g.seen(bear, None).colors.contains(&black), "{pieces:?}");
    }
}

#[test]
fn r613_1_the_world_holds_the_other_zones_only_where_an_effect_can_reach_them() {
    let size = |pieces: &[&str], of: usize| {
        let (g, objects) = board(pieces);
        HELD.with(|held| held.set(0));
        g.seen(objects[of], None);
        (HELD.with(|held| held.replace(0)), objects.len())
    };
    // permanents, and the eight cards elsewhere whose own text defines them (604.3)
    let (held, all) = size(&["Anthem"], 16);
    assert_eq!(held, 10 + 1 + 16, "{held} of {all}");
    // a card elsewhere that states nothing is as it is, with no world at all
    assert_eq!(size(&["Anthem"], 0).0, 0);
    // a color said of what is described may be said of any card: every zone is held
    let (held, all) = size(&["Night"], 16);
    assert_eq!(held, all);
}

#[test]
fn r604_3_what_defines_itself_elsewhere_reads_what_is_as_the_effects_leave_it() {
    // the graveyard's Goyf counts the creatures its owner controls as the layers make
    // them: an artifact animated in layer 4 is one
    let (g, objects) = board(&["Animation"]);
    let goyf = objects[9];
    assert_eq!((g.obj(goyf).zone, g.obj(goyf).chars.name.as_str()), (Zone::Graveyard, "Goyf"));
    // Bear, Hawk, Goyf, Mortal and the animated Rock
    assert_eq!(g.seen(goyf, None).pt, Some((5, 5)));
    assert_eq!((g.seen(objects[16], None).pt, g.seen(goyf, None).pt), (Some((2, 2)), Some((5, 5))));
}
