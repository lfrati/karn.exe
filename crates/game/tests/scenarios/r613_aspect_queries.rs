//! A question about a characteristic is settled once its layer has applied (CR 613.1): what
//! is asked as far as it reads ([`Aspect`]) is what the whole view says of it, on boards
//! where effects of every layer the engine applies meet.
use crate::support::cite;
use game::layers::Aspect;
use game::legal::Asked;
use game::{CardDef, Game, Move, ObjId, Target, Zone};
use ir::{CardType, Color, Keyword};

const NAMES: [&str; 30] = [
    "Bear", "Vigil", "Hawk", "Elf", "Rock", "Forest", "Goyf",
    "Anthem", "Fervor", "Shift", "Watch", "Wind", "Animation", "Humility", "Teacher", "Dark", "Wings", "Tribe",
    "Aura", "Animate", "Steal", "Blacken", "Plate", "Jump", "Strip", "Shrink",
    "Sweep", "Smelt", "Tithe", "Blade",
];
fn kw(name: &str) -> Keyword { Keyword::from_name(name).unwrap_or_else(|| panic!("{name}")) }
fn def(name: &str) -> usize { NAMES.iter().position(|n| *n == name).unwrap_or_else(|| panic!("{name}")) }

fn defs() -> Vec<CardDef> {
    let card = |name: &str, types, text, pt| {
        assert_eq!(NAMES[def(name)], name);
        CardDef::new(name, "{0}", types, text, pt).unwrap()
    };
    let defs = vec![
        card("Bear", "Creature — Bear", "", Some((2, 2))),
        card("Vigil", "Creature — Soldier", "Vigilance", Some((2, 2))),
        card("Hawk", "Creature — Bird", "Flying", Some((1, 1))),
        card("Elf", "Creature — Elf", "", Some((1, 1))),
        card("Rock", "Artifact", "", None),
        card("Forest", "Basic Land — Forest", "", None),
        card("Goyf", "Creature — Lhurgoyf", "This creature's power and toughness are each equal to the number of creatures you control.", Some((0, 0))),
        card("Anthem", "Enchantment", "Creatures you control get +1/+1.", None),
        card("Fervor", "Enchantment", "Creatures you control have haste.", None),
        card("Shift", "Enchantment", "Creatures with flying get +2/+0.\nCreatures without flying get -2/-0.", None),
        card("Watch", "Enchantment", "Creatures with vigilance have flying.", None),
        card("Wind", "Enchantment", "Creatures have trample as long as you control a creature with flying.", None),
        card("Animation", "Enchantment", "Noncreature artifacts become 3/3 artifact creatures.", None),
        card("Humility", "Enchantment", "All creatures lose all abilities and have base power and toughness 1/1.", None),
        card("Teacher", "Enchantment", "Creatures you control have \"Other creatures you control get +1/+1.\"", None),
        card("Dark", "Enchantment", "Black creatures get +1/+1.", None),
        card("Wings", "Enchantment", "Artifact creatures have flying.", None),
        card("Tribe", "Enchantment", "Each creature you control that's an Elf gets +1/+1.", None),
        card("Aura", "Enchantment — Aura", "Enchant creature\nEnchanted creature gets +2/+2 and has flying.", None),
        card("Animate", "Instant", "Target land becomes a 3/3 Elemental creature until end of turn. It's still a land.", None),
        card("Steal", "Instant", "Gain control of target creature until end of turn.", None),
        card("Blacken", "Instant", "Target creature becomes black until end of turn.", None),
        card("Plate", "Instant", "Target creature becomes an artifact in addition to its other types until end of turn.", None),
        card("Jump", "Instant", "Target creature gains flying until end of turn.", None),
        card("Strip", "Instant", "Target permanent loses all abilities until end of turn.", None),
        card("Shrink", "Instant", "Target creature has base power and toughness 1/1 until end of turn.", None),
        card("Sweep", "Sorcery", "Destroy all creatures with flying.", None),
        card("Smelt", "Sorcery", "Destroy all artifacts.", None),
        card("Tithe", "Sorcery", "You gain 1 life for each black creature you control.", None),
        card("Blade", "Artifact — Equipment", "Equipped creature gets +1/+1.\nEquip {0}", None),
    ];
    for d in &defs {
        game::play::check(d).unwrap_or_else(|u| panic!("{}: {}", d.name(), u.0));
    }
    defs
}

/// The objects every board has.
struct Base { bear: ObjId, vigil: ObjId, hawk: ObjId, elf: ObjId, forest: ObjId, theirs: ObjId }

fn base() -> (Game, Base) {
    let mut g = Game::new(defs(), 2, 20);
    for player in 0..2 {
        for name in ["Forest", "Goyf", "Bear", "Rock", "Hawk"] { g.put(player, def(name), Zone::Library); }
        g.put(player, def("Goyf"), Zone::Graveyard);
        g.put(player, def("Hawk"), Zone::Graveyard);
        g.put(player, def("Vigil"), Zone::Hand);
        g.put(player, def("Goyf"), Zone::Exile);
    }
    let mut on = |player, name| g.put(player, def(name), Zone::Battlefield);
    let base = Base { bear: on(0, "Bear"), vigil: on(0, "Vigil"), hawk: on(0, "Hawk"), elf: on(0, "Elf"), forest: on(0, "Forest"), theirs: on(1, "Bear") };
    on(0, "Rock");
    on(1, "Rock");
    on(1, "Hawk");
    on(1, "Forest");
    (g, base)
}
fn resolve(g: &mut Game) {
    while g.asked().is_some_and(|(_, a)| a == Asked::Priority) && !g.stack.is_empty() { g.act(g.priority, Move::Pass).unwrap(); }
}
fn cast(g: &mut Game, name: &str, target: Option<ObjId>) {
    let card = g.put(0, def(name), Zone::Hand);
    let targets = target.map(|t| vec![vec![Target::Object(t)]]).unwrap_or_default();
    g.act(0, Move::Cast { card, targets }).unwrap_or_else(|e| panic!("{name}: {e}"));
    resolve(g);
}

/// Each thing a board may have besides the base: a static ability, a resolved effect, counters.
const PIECES: [&str; 21] = [
    "Anthem", "Fervor", "Shift", "Watch", "Wind", "Animation", "Humility", "Teacher", "Dark", "Wings", "Tribe", "Goyf",
    "Aura", "Animate", "Steal", "Blacken", "Plate", "Jump", "Strip", "Shrink", "counters",
];
fn add(g: &mut Game, b: &Base, piece: &str) {
    match piece {
        "without Hawk" => { g.move_object(b.hawk, Zone::Graveyard); }
        "Aura" => cast(g, piece, Some(b.bear)),
        "Animate" => cast(g, piece, Some(b.forest)),
        "Steal" => cast(g, piece, Some(b.theirs)),
        "Blacken" => cast(g, piece, Some(b.elf)),
        "Plate" => cast(g, piece, Some(b.bear)),
        "Jump" => cast(g, piece, Some(b.elf)),
        "Strip" => cast(g, piece, Some(b.vigil)),
        "Shrink" => cast(g, piece, Some(b.hawk)),
        "counters" => {
            g.put_counters(b.bear, "flying", 1);
            g.put_counters(b.elf, "+1/+1", 2);
            g.put_counters(b.forest, "+1/+1", 1);
        }
        "Goyf" => { g.put(0, def("Goyf"), Zone::Battlefield); g.put(1, def("Goyf"), Zone::Battlefield); }
        permanent => { g.put(0, def(permanent), Zone::Battlefield); }
    }
}
fn board(pieces: &[&str]) -> (Game, Base) {
    let (mut g, b) = base();
    for piece in pieces { add(&mut g, &b, piece); }
    (g, b)
}
/// A board where an effect of every layer is at work at once.
const MEETING: [&str; 10] = ["Anthem", "Watch", "Animation", "Humility", "Teacher", "Animate", "Steal", "Blacken", "Plate", "counters"];
/// Nothing, each piece alone, groups whose effects read one another in both orders, and
/// that board.
fn boards() -> Vec<Vec<&'static str>> {
    let mut boards: Vec<Vec<&str>> = vec![Vec::new()];
    boards.extend(PIECES.iter().map(|piece| vec![*piece]));
    boards.extend([
        vec!["Watch", "Wind", "Shift"],
        vec!["Watch", "Wind", "without Hawk"],
        vec!["Humility", "Watch", "Anthem", "Teacher"],
        vec!["Teacher", "Humility", "Strip"],
        vec!["Animation", "Wings", "Shift", "Anthem"],
        vec!["Animate", "Anthem", "counters", "Goyf"],
        vec!["Blacken", "Dark", "Plate", "Wings", "Jump"],
        vec!["Steal", "Anthem", "Fervor", "Tribe", "Goyf"],
        vec!["Aura", "Shift", "Shrink", "counters"],
    ]);
    // the same groups in the other order: other timestamps
    let reversed: Vec<Vec<&str>> = boards.iter().filter(|pieces| pieces.len() > 1).map(|pieces| pieces.iter().rev().copied().collect()).collect();
    boards.extend(reversed);
    boards.push(MEETING.to_vec());
    boards
}
/// Every permanent, and one object of each name in each other zone.
fn everything(g: &Game) -> Vec<ObjId> {
    let mut kinds = Vec::new();
    let elsewhere = g.exile.iter().chain(&g.stack).chain(g.players.iter().flat_map(|p| p.hand.iter().chain(&p.library).chain(&p.graveyard)))
        .copied().filter(|o| { let kind = (g.obj(*o).chars.name.clone(), g.obj(*o).zone); !kinds.contains(&kind) && { kinds.push(kind); true } });
    g.battlefield.iter().copied().chain(elsewhere).collect()
}

#[test]
fn what_an_aspect_reads_is_what_the_whole_view_says() {
    for pieces in boards() {
        let (g, _) = board(&pieces);
        let expected: Vec<_> = everything(&g).into_iter().map(|o| (o, g.seen(o, None))).collect();
        for (o, full) in expected {
            let at = |what: &str| format!("{what} of {} ({:?}) with {pieces:?}", g.obj(o).chars.name, g.obj(o).zone);
            for aspect in [Aspect::Types, Aspect::Colors, Aspect::Abilities, Aspect::All] {
                let seen = g.seen_for(o, aspect);
                cite!(["613.1b", "613.1d"]; case: at("types"); assert_eq!((seen.controller, &seen.types, &seen.subtypes, &seen.supertypes), (full.controller, &full.types, &full.subtypes, &full.supertypes), "{}", at("types")));
                if aspect == Aspect::Types { continue; }
                cite!(["613.1e"]; case: at("colors"); assert_eq!(seen.colors, full.colors, "{}", at("colors")));
                if aspect == Aspect::Colors { continue; }
                cite!(["613.1f"]; case: at("abilities"); assert_eq!((&seen.keywords, seen.base_abilities, &seen.given), (&full.keywords, full.base_abilities, &full.given), "{}", at("abilities")));
                if aspect == Aspect::All { assert_eq!(seen, full, "{}", at("all")); }
            }
            // the questions the engine's own helpers ask
            for t in [CardType::Creature, CardType::Artifact, CardType::Land, CardType::Enchantment] {
                assert_eq!(g.is_a(o, t), full.is(t), "{}", at("is_a"));
            }
            for (name, k) in [("Flying", kw("Flying")), ("Haste", kw("Haste")), ("Trample", kw("Trample")), ("Vigilance", kw("Vigilance"))] {
                assert_eq!(g.has(o, name), full.has(k), "{}", at(name));
            }
        }
    }
}

#[test]
fn the_boards_are_ones_where_the_layers_meet() {
    // so that the equality above is not of boards where nothing happens
    let (g, b) = board(&MEETING);
    let full = |o| g.seen(o, None);
    cite!(["613.1d"]; assert!(full(b.forest).is(CardType::Creature) && full(b.forest).is(CardType::Land), "an animated land"));
    cite!(["613.1b"]; assert_eq!(full(b.theirs).controller, 0, "a creature whose control changed"));
    assert!(full(b.elf).colors.contains(&Color::Black));
    assert!(full(b.bear).is(CardType::Artifact));
    assert!(!full(b.vigil).base_abilities || full(b.vigil).keywords.is_empty(), "Humility removes printed abilities");
    let (g, b) = board(&["Wind", "Watch", "without Hawk"]);
    assert!(g.has(b.vigil, "Flying"));
    assert!(g.has(b.bear, "Trample"), "Wind depends on Watch granting the only friendly flying ability");
    let (g, b) = board(&["Watch", "Wind", "Shift", "Teacher", "Animation", "Wings", "counters"]);
    let full = |o| g.seen(o, None);
    assert!(full(b.vigil).has(kw("Flying")) && full(b.bear).has(kw("Trample")), "an ability given because of one given");
    assert!(full(b.bear).has(kw("Flying")), "a flying counter");
    assert!(!full(b.bear).given.is_empty(), "a static ability given by a static ability");
    let rock = *g.battlefield.iter().find(|o| g.obj(**o).chars.name == "Rock").unwrap();
    cite!(["613.1d", "613.1f"]; assert!(full(rock).is(CardType::Creature) && full(rock).has(kw("Flying")), "an artifact made a creature has what artifact creatures have"));
    cite!(["613.4c"]; assert_eq!(full(b.elf).pt, Some((1 + 2 - 2 + 4, 1 + 2 + 4)), "counters, Shift, and four other creatures teaching it, the Rock among them"));
}

/// What a spell does to the objects it describes is what the whole view says they are:
/// the selectors of a resolving effect ask through the same narrowed questions.
#[test]
fn a_resolving_effect_selects_by_what_the_whole_view_says() {
    // the small boards
    for pieces in boards().into_iter().filter(|pieces| pieces.len() <= 5) {
        let (g, _) = board(&pieces);
        let full: Vec<(ObjId, game::layers::Seen)> = g.battlefield.iter().map(|o| (*o, g.seen(*o, None))).collect();
        let named = |o: &ObjId| g.obj(*o).chars.name.clone();
        for (spell, hit) in [
            ("Sweep", &(|s: &game::layers::Seen| s.is(CardType::Creature) && s.has(kw("Flying"))) as &dyn Fn(&game::layers::Seen) -> bool),
            ("Smelt", &|s: &game::layers::Seen| s.is(CardType::Artifact)),
        ] {
            let mut after = g.clone();
            let from = after.log.end();
            let mut expected: Vec<ObjId> = full.iter().filter(|(_, s)| hit(s)).map(|(o, _)| *o).collect();
            cast(&mut after, spell, None);
            // CR404.3: author each owner's permutation from the pre-event selected
            // identities, never from the order offered by the pending prompt.
            for owner in 0..after.players.len() {
                let incoming: Vec<_> = expected.iter().copied().filter(|id|g.obj(*id).owner==owner).collect();
                if incoming.len()>1 {
                    assert_eq!(after.decision,Some(game::state::Decision::Choose{player:owner,choice:game::state::Choice::OrderCards{to:game::state::Placement::Zone(Zone::Graveyard),cards:incoming.clone()}}));
                    after.act(owner,Move::Choose(game::state::Answer::Objects(incoming.iter().rev().copied().collect()))).unwrap();
                }
            }
            let mut destroyed: Vec<ObjId> = after.log.since(from).iter().filter_map(|e| match e { game::Event::Destroyed { object, .. } => Some(*object), _ => None }).collect();
            destroyed.sort();
            expected.sort();
            assert_eq!(destroyed.iter().map(named).collect::<Vec<_>>(), expected.iter().map(named).collect::<Vec<_>>(), "{spell} with {pieces:?}");
            cite!(["613.1"]; case: format!("{spell} with {pieces:?}"); assert_eq!(destroyed, expected, "{spell} with {pieces:?}"));
        }
        let mut after = g.clone();
        cast(&mut after, "Tithe", None);
        let black = full.iter().filter(|(_, s)| s.controller == 0 && s.is(CardType::Creature) && s.colors.contains(&Color::Black)).count();
        cite!(["613.1"]; case: format!("Tithe with {pieces:?}"); assert_eq!(after.players[0].life, 20 + black as i32, "Tithe with {pieces:?}"));
    }
}

/// A target is legal by what the whole view says it is.
#[test]
fn a_target_is_legal_by_what_the_whole_view_says() {
    let (mut g, b) = board(&["Animate", "Animation", "Anthem", "Humility"]);
    let card = g.put(0, def("Jump"), Zone::Hand);
    for o in g.battlefield.clone() {
        let creature = g.seen(o, None).is(CardType::Creature);
        let mut tried = g.clone();
        let cast = tried.act(0, Move::Cast { card, targets: vec![vec![Target::Object(o)]] });
        if creature { cite!(["613.1"]; case: &g.obj(o).chars.name; ok: cast); }
        else { cite!(["613.1"]; case: &g.obj(o).chars.name; refused("it isn't a creature (115.1)"): cast); }
    }
    assert!(g.seen(b.forest, None).is(CardType::Creature));
}
