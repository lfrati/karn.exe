//! The abilities layer applied in timestamp order because nothing in it can depend on
//! anything else in it (613.8a) is the layer the dependency trials order: every board here
//! is asked both ways, and the proof is made only where it may be.
use super::{PROVEN, Seen, TRIALS_ONLY};
use crate::card::CardDef;
use crate::event::Target;
use crate::legal::Asked;
use crate::play::Move;
use crate::state::{Game, ObjId, Zone};

const NAMES: [&str; 18] = [
    "Bear", "Vigil", "Hawk", "Golem", "Rock",
    "Fervor", "Splicer", "Watch", "Wind", "Guard", "Humility", "Monument", "Captain", "Wings", "Animation",
    "Jump", "Strip", "Grant",
];
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
        card("Golem", "Artifact Creature — Golem", "", Some((3, 3))),
        card("Rock", "Artifact", "", None),
        // read no ability
        card("Fervor", "Enchantment", "Creatures you control have haste.", None),
        card("Splicer", "Creature — Human", "Golems you control have first strike.", Some((1, 1))),
        // read abilities: of what they are said of, of when they hold
        card("Watch", "Enchantment", "Creatures with vigilance have flying.", None),
        card("Wind", "Enchantment", "Creatures have trample as long as you control a creature with flying.", None),
        // a condition that reads none
        card("Guard", "Enchantment", "Creatures you control have reach as long as you control an artifact.", None),
        // takes the abilities others are stated by
        card("Humility", "Enchantment", "All creatures lose all abilities and have base power and toughness 1/1.", None),
        // continues in a later layer what it began in this one (613.6)
        card("Monument", "Artifact", "Creatures you control get +1/+1 and have flying and indestructible.", None),
        card("Captain", "Creature — Soldier", "Flying\nCreatures you control have vigilance.", Some((2, 2))),
        card("Wings", "Enchantment", "Artifact creatures have flying.", None),
        card("Animation", "Enchantment", "Noncreature artifacts become 3/3 artifact creatures.", None),
        card("Jump", "Instant", "Target creature gains flying until end of turn.", None),
        card("Strip", "Instant", "Target permanent loses all abilities until end of turn.", None),
        card("Grant", "Enchantment", "Creatures you control have \"Other creatures you control have lifelink.\"", None),
    ];
    for d in &defs {
        crate::play::check(d).unwrap_or_else(|u| panic!("{}: {}", d.name(), u.0));
    }
    defs
}

/// What every board has, and what each board adds of `pieces`, in the order given.
fn board(pieces: &[&str]) -> (Game, Vec<ObjId>) {
    let mut g = Game::new(defs(), 2, 20);
    for player in 0..2 {
        for name in ["Bear", "Hawk", "Rock"] { g.put(player, def(name), Zone::Library); }
    }
    let mut objects: Vec<ObjId> = [(0, "Bear"), (0, "Vigil"), (0, "Hawk"), (0, "Golem"), (0, "Rock"), (1, "Bear"), (1, "Golem")]
        .into_iter().map(|(player, name)| g.put(player, def(name), Zone::Battlefield)).collect();
    for name in pieces {
        match *name {
            // the spells: at what they are named with
            "Jump" => cast(&mut g, "Jump", objects[0]),
            "Strip bear" => cast(&mut g, "Strip", objects[0]),
            "Strip source" => {
                let source = objects.iter().copied().find(|o| ["Captain", "Splicer"].contains(&g.obj(*o).chars.name.as_str()));
                cast(&mut g, "Strip", source.unwrap_or(objects[1]));
            }
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

/// Every object's whole view, and how many abilities layers were proven independent.
fn views(g: &Game, objects: &[ObjId], trials_only: bool) -> (Vec<Seen>, usize) {
    TRIALS_ONLY.with(|t| t.set(trials_only));
    PROVEN.with(|n| n.set(0));
    // Each question deliberately evaluates afresh: this test counts proof executions,
    // rather than measuring the ordinary revision cache added in stage4.
    let views = objects.iter().map(|o| g.clone().seen(*o, None)).collect();
    TRIALS_ONLY.with(|t| t.set(false));
    (views, PROVEN.with(|n| n.replace(0)))
}

/// Both ways agree; says whether the proof was made for every layer asked, for none, or
/// for some (`None`).
fn agree(pieces: &[&str]) -> Option<bool> {
    let (g, objects) = board(pieces);
    let (trials, never) = views(&g, &objects, true);
    let (proven, times) = views(&g, &objects, false);
    assert_eq!(never, 0, "{pieces:?}: forced trials prove nothing");
    for ((o, a), b) in objects.iter().zip(&proven).zip(&trials) {
        assert_eq!(a, b, "{} on {pieces:?}", g.obj(*o).chars.name);
    }
    // every object is asked once, and each asking meets the same effects in the layer
    match times {
        0 => Some(false),
        n if n == objects.len() => Some(true),
        _ => None,
    }
}

const PIECES: [&str; 13] = ["Fervor", "Splicer", "Watch", "Wind", "Guard", "Humility", "Monument", "Captain", "Wings", "Animation", "Jump", "Strip bear", "Strip source"];

#[test]
fn r613_8a_the_abilities_layer_proven_independent_is_the_layer_the_trials_order() {
    // every pair and every triple around each piece, in both timestamp orders
    let mut boards = 0;
    for a in 0..PIECES.len() {
        for b in a..PIECES.len() {
            for c in [None, Some((a + b + 1) % PIECES.len()), Some((a * 3 + b * 5 + 2) % PIECES.len())] {
                let mut pieces = vec![PIECES[a]];
                if b != a { pieces.push(PIECES[b]); }
                if let Some(c) = c && c != a && c != b { pieces.push(PIECES[c]); }
                agree(&pieces);
                pieces.reverse();
                agree(&pieces);
                boards += 2;
            }
        }
    }
    assert!(boards > 500);
    // everything at once, each way round
    agree(&PIECES);
    let mut reversed = PIECES.to_vec();
    reversed.reverse();
    agree(&reversed);
}

#[test]
fn r613_8a_the_proof_is_made_where_nothing_reads_or_removes_an_ability() {
    for pieces in [
        &["Fervor"][..], &["Splicer", "Fervor"], &["Guard", "Fervor"], &["Monument", "Captain", "Fervor"],
        &["Wings", "Animation", "Splicer"], &["Fervor", "Jump"],
        // losing all abilities ends nothing here: the bear states none of these
        &["Captain", "Fervor", "Strip bear"],
    ] {
        assert_eq!(agree(pieces), Some(true), "{pieces:?}");
    }
}

#[test]
fn r613_8a_what_reads_or_removes_an_ability_is_left_to_the_trials() {
    for pieces in [
        // said of creatures with an ability; holding while one has an ability
        &["Watch", "Captain"][..], &["Fervor", "Watch"], &["Wind", "Fervor"],
        // all abilities taken: by a static ability, and by a spell from a source asked here
        &["Humility", "Fervor"], &["Captain", "Fervor", "Strip source"], &["Splicer", "Strip source"],
    ] {
        assert_eq!(agree(pieces), Some(false), "{pieces:?}");
    }
}

#[test]
fn r613_8a_the_proven_layer_gives_what_the_rules_say() {
    let kw = |name: &str| ir::Keyword::from_name(name).unwrap();
    // 613.6: the monument continues to what it applied to; both of the captain's are said
    let (g, objects) = board(&["Monument", "Captain", "Fervor", "Guard"]);
    let bear = g.seen(objects[0], None);
    for name in ["Flying", "Indestructible", "Vigilance", "Haste", "Reach"] { assert!(bear.has(kw(name)), "{name}"); }
    assert_eq!(bear.pt, Some((3, 3)));
    assert!(!g.seen(objects[5], None).has(kw("Haste")), "theirs is not one you control");
    // 613.1f after 613.1d: the golem's first strike reaches an artifact animated as a
    // creature only if it is a Golem, and wings reach it because it is an artifact creature
    let (g, objects) = board(&["Animation", "Wings", "Splicer"]);
    assert!(g.seen(objects[4], None).has(kw("Flying")) && !g.seen(objects[4], None).has(kw("First strike")));
    assert!(g.seen(objects[3], None).has(kw("First strike")) && !g.seen(objects[6], None).has(kw("First strike")));
    // the spell took the bear's abilities before the later static ability gave it haste
    let (g, objects) = board(&["Strip bear", "Fervor"]);
    assert!(g.seen(objects[0], None).has(kw("Haste")));
}

#[test]
fn r613_8a_granted_keyword_statics_always_use_membership_dependency_trials() {
    for pieces in [vec!["Grant"], vec!["Grant", "Humility"], vec!["Humility", "Grant"],
        vec!["Grant", "Strip bear"], vec!["Grant", "Grant"]] {
        assert_eq!(agree(&pieces), Some(false), "{pieces:?}");
    }
}
