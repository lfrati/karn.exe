//! Impending (702.176a): what it is while not a creature (208.3,
//! 613.1d timestamp order with other type-changing effects), and copies that weren't cast.
use crate::support::cite;
use game::announcement::CastCost;
use game::state::Decision;
use game::{CardDef, Game, Move, ObjId, Target, Zone};
use ir::{CardType, Part};

const MISTMOORS: usize = 0;
const ANIMATE: usize = 1;
const ANTHEM: usize = 2;
const COUNT: usize = 3;
const ISLAND: usize = 4;

fn game() -> Game {
    let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let real = |name: &str| CardDef::from_face(faces.iter().find(|f| f.name == name).unwrap()).unwrap();
    let c = |name, cost, types, text, pt| CardDef::new(name, cost, types, text, pt).expect(name);
    let defs = vec![
        real("Overlord of the Mistmoors"),
        c("Animate", "{0}", "Instant", "Target enchantment becomes a 4/4 Elemental creature in addition to its other types until end of turn.", None),
        c("Anthem", "{0}", "Enchantment", "Each other enchantment you control is a 4/4 creature in addition to its other types.", None),
        c("Count", "{0}", "Instant", "You gain 1 life for each creature you control.", None),
        c("Island", "", "Basic Land — Island", "", None),
    ];
    for d in &defs { game::play::check(d).unwrap_or_else(|e| panic!("{}: {}", d.name(), e.0)); }
    let mut g = Game::new(defs, 2, 20);
    for p in 0..2 { for _ in 0..20 { g.put(p, ISLAND, Zone::Library); } }
    while g.part != Part::PrecombatMain { g.act(g.priority, Move::Pass).unwrap(); }
    g
}
fn settle(g: &mut Game) {
    for _ in 0..100 {
        match g.decision.clone() {
            Some(Decision::Order { player, abilities }) => g.act(player, Move::Order((0..abilities.len()).collect())).unwrap(),
            Some(Decision::Choose { player, .. }) => g.act(player, Move::Choose(game::state::Answer::Yes)).unwrap(),
            Some(Decision::Targets { player, .. }) => panic!("targets for {player}"),
            Some(d) => panic!("unexpected decision {d:?}"),
            None if g.stack.is_empty() => return,
            None => g.act(g.priority, Move::Pass).unwrap(),
        }
    }
    panic!("did not settle");
}
fn impending(g: &mut Game) -> ObjId {
    let card = g.put(0, MISTMOORS, Zone::Hand);
    g.players[0].pool.colors[0] = 2;
    g.players[0].pool.colorless = 2;
    g.act(0, crate::support::paid("CCWW",Move::CastFor(CastCost::Alternative(0), Box::new(Move::Cast { card, targets: vec![] })))).unwrap();
    settle(g);
    *g.battlefield.iter().find(|o| g.obj(**o).card == Some(MISTMOORS)).unwrap()
}
fn cast(g: &mut Game, def: usize, target: Option<ObjId>) {
    let card = g.put(0, def, Zone::Hand);
    g.act(0, Move::Cast { card, targets: target.map(|t| vec![vec![Target::Object(t)]]).unwrap_or_default() }).unwrap();
    settle(g);
}
fn types(g: &Game, o: ObjId) -> (bool, Vec<String>) {
    let seen = g.seen(o, None);
    (seen.types.contains(&CardType::Creature), seen.subtypes.iter().map(|s| format!("{s:?}")).collect())
}

/// Not a creature, it has no power or toughness (208.3) and isn't one of "each creature you
/// control" (its two Insects are).
#[test]
fn r208_3_not_a_creature_it_has_no_power_and_is_not_counted() {
    let mut g = game();
    let o = impending(&mut g);
    cite!(["208.3", "702.176a"]; assert_eq!((g.power(o), g.toughness(o)), (None, None)));
    cast(&mut g, COUNT, None);
    cite!(["702.176a"]; assert_eq!(g.players[0].life, 22));
}

/// A later effect that makes it a creature applies after its not being one (613.7): it's a
/// 4/4 Elemental creature, its Avatar Horror subtypes still gone (205.1a). An earlier one
/// doesn't: the Overlord's own effect is later, and it isn't a creature.
#[test]
fn r613_7_a_later_animation_makes_it_a_creature_an_earlier_one_does_not() {
    let mut g = game();
    let o = impending(&mut g);
    cast(&mut g, ANIMATE, Some(o));
    let (creature, subtypes) = types(&g, o);
    cite!(["613.1d", "613.7"]; assert!(creature));
    cite!(["613.4b", "613.7"]; assert_eq!((g.power(o), g.toughness(o)), (Some(4), Some(4))));
    cite!(["205.1a"]; assert_eq!(subtypes.iter().filter(|s| s.contains("Avatar") || s.contains("Horror")).count(), 0, "{subtypes:?}"));
    cite!(["205.1a"]; assert!(subtypes.iter().any(|s| s.contains("Elemental")), "{subtypes:?}"));

    let mut g = game();
    let anthem = g.put(0, ANTHEM, Zone::Battlefield);
    let other = g.put(0, ANTHEM, Zone::Battlefield);
    // control: the anthems animate each other
    assert!(types(&g, anthem).0 && types(&g, other).0);
    let o = impending(&mut g);
    cite!(["613.7"]; assert!(!types(&g, o).0));
}

/// A Clone of an impending Overlord would be a 6/6 creature (707.2: counters and the paid
/// cost aren't copied), but entering as a copy is still refused, so no admitted card
/// reaches it; this records the boundary.
#[test]
fn r707_2_entering_as_a_copy_of_it_remains_refused() {
    let mirror = CardDef::new("Mirror", "{0}", "Enchantment", "You may have this enchantment enter as a copy of an enchantment on the battlefield.", None).unwrap();
    cite!(["707.2"]; refused("copy entry: controller chooses the source permanent"): game::play::check(&mirror).map_err(|e|e.0));
}

/// Neurok Transmuter, newly admitted with the type removal: "Until end of turn, target
/// artifact creature becomes blue and isn't an artifact." It stays a creature with its
/// creature types; an artifact subtype goes with the artifact type (205.1a); all of it lasts
/// until end of turn (514.2).
#[test]
fn r205_1a_neurok_transmuter_removes_the_artifact_type_until_end_of_turn() {
    let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let neurok = CardDef::from_face(faces.iter().find(|f| f.name == "Neurok Transmuter").unwrap()).unwrap();
    game::play::check(&neurok).unwrap();
    let golem = CardDef::new("Golem", "{0}", "Artifact Creature — Golem", "", Some((3, 3))).unwrap();
    let mut g = Game::new(vec![neurok, golem, CardDef::new("Island", "", "Basic Land — Island", "", None).unwrap()], 2, 20);
    for p in 0..2 { for _ in 0..20 { g.put(p, 2, Zone::Library); } }
    while g.part != Part::PrecombatMain { g.act(g.priority, Move::Pass).unwrap(); }
    let neurok = g.put(0, 0, Zone::Battlefield);
    let golem = g.put(0, 1, Zone::Battlefield);
    g.players[0].pool.colors[1] = 1;
    g.act(0, crate::support::paid("U",Move::Activate { source: neurok, ability: 1, targets: vec![vec![Target::Object(golem)]], paying: vec![] })).unwrap();
    settle(&mut g);
    let seen = g.seen(golem, None);
    cite!(["205.1a", "613.1d"]; assert!(!seen.types.contains(&CardType::Artifact) && seen.types.contains(&CardType::Creature)));
    cite!(["205.1a"]; assert!(seen.subtypes.iter().any(|s| format!("{s:?}").contains("Golem")), "{:?}", seen.subtypes));
    cite!(["613.1e"]; assert_eq!(seen.colors, vec![ir::Color::Blue]));
    let turn = g.turn;
    while g.turn == turn {
        match g.decision { Some(Decision::Attackers { player }) => g.act(player, Move::Attack(vec![])).unwrap(), _ => g.act(g.priority, Move::Pass).unwrap() }
    }
    cite!(["514.2"]; assert!(g.seen(golem, None).types.contains(&CardType::Artifact)));
}
