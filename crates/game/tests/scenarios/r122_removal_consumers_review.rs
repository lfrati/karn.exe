//! Full pinned cards whose effects remove counters,
//! each run to its counter-removal consequence.
use crate::support::{cite, paid};
use game::state::{Answer, Choice, Decision};
use game::{CardDef, Game, Move, ObjId, Target, Zone};
use ir::CardType;

const THING: usize = 0;
const DEPTHS: usize = 1;
const STONE: usize = 2;
const LAYER: usize = 3;
const ARIXMETHES: usize = 4;
const AUNTIE: usize = 5;
const BEARS: usize = 6;
const ISLAND: usize = 7;
const SPARK: usize = 8;
const KILL: usize = 9;

fn game() -> Game {
    let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let catalog = CardDef::catalog(&faces);
    let mut defs: Vec<CardDef> = ["Thing in the Ice // Awoken Horror", "Dark Depths", "Oblivion Stone", "Mine Layer",
        "Arixmethes, Slumbering Isle", "Heirloom Auntie", "Grizzly Bears", "Island"]
        .iter()
        .map(|n| CardDef::from_faces(catalog.get(n).unwrap_or_else(|| panic!("{n}"))).unwrap())
        .collect();
    defs.push(CardDef::new("Spark", "{0}", "Instant", "You gain 1 life.", None).unwrap());
    defs.push(CardDef::new("Kill", "{0}", "Instant", "Destroy target creature.", None).unwrap());
    for d in &defs { game::play::check(d).unwrap_or_else(|e| panic!("{}: {}", d.name(), e.0)); }
    let mut g = Game::new(defs, 2, 20);
    g.players[0].pool.colors = [20; 5];
    for _ in 0..3 { g.put(0, ISLAND, Zone::Library); }
    g
}
/// Resolve the stack, ordering triggers as listed, saying yes to a "may" and keeping
/// surveilled cards on top. Returns how many "may" were asked.
fn settle(g: &mut Game) -> usize {
    let mut asked = 0;
    for _ in 0..40 {
        match g.decision.clone() {
            Some(Decision::Order { player, abilities }) => { g.act(player, Move::Order((0..abilities.len()).collect())).unwrap(); continue }
            Some(Decision::Choose { player, choice: Choice::Arrange { cards, .. } }) => { g.act(player, Move::Choose(Answer::Arrange { top: cards, away: vec![] })).unwrap(); continue }
            Some(Decision::Choose { player, .. }) => { asked += 1; g.act(player, Move::Choose(Answer::Yes)).unwrap(); continue }
            _ => {}
        }
        if g.stack.is_empty() { return asked }
        g.act(g.priority, Move::Pass).unwrap();
    }
    panic!("the stack did not settle")
}
fn cast(g: &mut Game, def: usize, targets: Vec<Vec<Target>>) -> usize {
    let card = g.put(0, def, Zone::Hand);
    g.act(0, Move::Cast { card, targets }).unwrap();
    settle(g)
}
fn on(g: &Game, o: ObjId) -> bool { g.battlefield.contains(&o) }

/// The last ice counter goes, so it transforms and returns every non-Horror creature.
#[test]
fn thing_in_the_ice_transforms_when_its_last_counter_is_removed() {
    let mut g = game();
    let thing = g.put(0, THING, Zone::Battlefield);
    g.put_counters(thing, "ice", 1);
    let bears = g.put(1, BEARS, Zone::Battlefield);
    cast(&mut g, SPARK, vec![]);
    // Uncited (card text): Thing in the Ice's "Whenever you cast an instant or sorcery spell, remove an ice counter from this creature."
    assert_eq!(g.counters(thing).get("ice"), None);
    // Uncited (card text): Awoken Horror's "return all non-Horror creatures to their owners' hands"; it is a Kraken Horror
    assert!(on(&g, thing) && !on(&g, bears));
    cite!(["701.27a", "712.8e"]; assert_eq!(g.seen(thing, None).pt, Some((7, 8)), "Awoken Horror"));
}

#[test]
fn dark_depths_without_ice_becomes_marit_lage() {
    let mut g = game();
    let depths = g.put(0, DEPTHS, Zone::Battlefield);
    g.put_counters(depths, "ice", 1);
    g.act(0, paid("GGG", Move::Activate { source: depths, ability: 1, targets: vec![], paying: vec![] })).unwrap();
    settle(&mut g);
    cite!(["603.8"]; assert!(!on(&g, depths)));
    let [lage] = g.battlefield[..] else { panic!("{:?}", g.battlefield) };
    let seen = g.seen(lage, None);
    // Uncited (card text): Dark Depths' "create Marit Lage, a legendary 20/20 black Avatar creature token"
    assert_eq!((seen.pt, seen.types.clone()), (Some((20, 20)), vec![CardType::Creature]));
}

/// Destroys what has no fate counter, then removes the fate counters it spared.
#[test]
fn oblivion_stone_spares_fate_counters_then_removes_them() {
    let mut g = game();
    let stone = g.put(0, STONE, Zone::Battlefield);
    let kept = g.put(0, BEARS, Zone::Battlefield);
    g.put_counters(kept, "fate", 1);
    let gone = g.put(1, BEARS, Zone::Battlefield);
    g.act(0, paid("GGGGG", Move::Activate { source: stone, ability: 1, targets: vec![], paying: vec![] })).unwrap();
    settle(&mut g);
    // Uncited (card text): Oblivion Stone's "Sacrifice this artifact: Destroy each nonland permanent without a fate counter on it"
    assert_eq!(g.battlefield, vec![kept]);
    // Uncited (card text): as above, the cost "Sacrifice this artifact" and "Destroy each nonland permanent without a fate counter on it"
    assert!(!on(&g, gone) && !on(&g, stone));
    // Uncited (card text): "then remove all fate counters from all permanents"
    assert_eq!(g.counters(kept).get("fate"), None);
}

#[test]
fn mine_layer_leaving_removes_every_mine_counter() {
    let mut g = game();
    let layer = g.put(0, LAYER, Zone::Battlefield);
    let mine = g.put(0, ISLAND, Zone::Battlefield);
    let theirs = g.put(1, ISLAND, Zone::Battlefield);
    g.put_counters(mine, "mine", 1);
    g.put_counters(theirs, "mine", 2);
    cast(&mut g, KILL, vec![vec![Target::Object(layer)]]);
    assert!(!on(&g, layer));
    cite!(["603.10a"]; assert_eq!((g.counters(mine).get("mine"), g.counters(theirs).get("mine")), (None, None)));
}

/// With its last slumber counter removed it is no longer a land but a creature; with
/// none left, there is nothing it may remove.
#[test]
fn arixmethes_may_remove_its_last_slumber_counter_and_then_wakes() {
    let mut g = game();
    let ari = g.put(0, ARIXMETHES, Zone::Battlefield);
    g.remove_counters(ari, "slumber", 5);
    g.put_counters(ari, "slumber", 1);
    assert_eq!(cast(&mut g, SPARK, vec![]), 1);
    assert_eq!(g.counters(ari).get("slumber"), None);
    cite!(["611.3a"]; assert_eq!(g.seen(ari, None).types, vec![CardType::Creature]));
    cite!(["608.2d"]; assert_eq!(cast(&mut g, SPARK, vec![]), 0, "asked to remove a counter it doesn't have"));
    assert!(on(&g, ari));
}

#[test]
fn heirloom_auntie_surveils_then_removes_a_counter() {
    let mut g = game();
    let auntie = g.put(0, AUNTIE, Zone::Battlefield);
    g.remove_counters(auntie, "-1/-1", 2);
    g.put_counters(auntie, "-1/-1", 1);
    let bears = g.put(0, BEARS, Zone::Battlefield);
    cast(&mut g, KILL, vec![vec![Target::Object(bears)]]);
    assert_eq!(g.counters(auntie).get("-1/-1"), None);
    cite!(["122.1a"]; assert_eq!(g.seen(auntie, None).pt, Some((4, 4))));
}
