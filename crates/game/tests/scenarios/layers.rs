//! Continuous effects (CR 611) and the layers they are applied in (613), rule by rule: what a
//! permanent is now. Every card is read from its printed text.

use crate::support::{cite, paid};
use game::card::CardDef;
use game::play::check;
use game::{Event, Game, Move, ObjId, Target, Zone};
use ir::Part;

const FOREST: usize = 0;
const BEARS: usize = 1;
const GROWTH: usize = 2;
const ANTHEM: usize = 3;
const BATTLEGROWTH: usize = 4;
const DIMINISH: usize = 5;
const GIGANTOMANCER: usize = 6;
const DISFIGURE: usize = 7;
const SUNBLADE: usize = 8;
const SHIFT: usize = 9;
const JUMP: usize = 10;
const FERVOR: usize = 11;
const KAVU: usize = 12;
const SWAMP: usize = 13;
const TRASHMASTER: usize = 14;
const FANATIC: usize = 15;
const CROW: usize = 16;
const PLAINS: usize = 17;
const ISLAND: usize = 18;
const ENGINEER: usize = 19;
const MYR: usize = 20;
const PERSECUTION: usize = 21;

fn defs() -> Vec<CardDef> {
    let card = |name, cost, types, text, pt| CardDef::new(name, cost, types, text, pt).expect(name);
    vec![
        card("Forest", "", "Basic Land — Forest", "({T}: Add {G}.)", None),
        card("Grizzly Bears", "{1}{G}", "Creature — Bear", "", Some((2, 2))),
        card("Giant Growth", "{G}", "Instant", "Target creature gets +3/+3 until end of turn.", None),
        card("Glorious Anthem", "{1}{W}{W}", "Enchantment", "Creatures you control get +1/+1.", None),
        card("Battlegrowth", "{G}", "Instant", "Put a +1/+1 counter on target creature.", None),
        card("Diminish", "{U}", "Instant", "Target creature has base power and toughness 1/1 until end of turn.", None),
        card(
            "Gigantomancer",
            "{7}{G}",
            "Creature — Human Shaman",
            "{1}: Target creature you control has base power and toughness 7/7 until end of turn.",
            Some((1, 1)),
        ),
        card("Disfigure", "{B}", "Instant", "Target creature gets -2/-2 until end of turn.", None),
        card(
            "Sunblade Elf",
            "{G}",
            "Creature — Elf Warrior",
            "This creature gets +1/+1 as long as you control a Plains.\n{4}{W}: Creatures you control get +1/+1 until end of turn.",
            Some((1, 1)),
        ),
        card("Gravitational Shift", "{3}{U}{U}", "Enchantment", "Creatures with flying get +2/+0.\nCreatures without flying get -2/-0.", None),
        card("Jump", "{U}", "Instant", "Target creature gains flying until end of turn.", None),
        card("Fervor", "{2}{R}", "Enchantment", "Creatures you control have haste.", None),
        card("Mire Kavu", "{3}{R}", "Creature — Kavu", "This creature gets +1/+1 as long as you control a Swamp.", Some((3, 2))),
        card("Swamp", "", "Basic Land — Swamp", "({T}: Add {B}.)", None),
        card(
            "Goblin Trashmaster",
            "{2}{R}{R}",
            "Creature — Goblin Warrior",
            "Other Goblins you control get +1/+1.\nSacrifice a Goblin: Destroy target artifact.",
            Some((3, 3)),
        ),
        card("Mogg Fanatic", "{R}", "Creature — Goblin", "Sacrifice this creature: It deals 1 damage to any target.", Some((1, 1))),
        card("Storm Crow", "{1}{U}", "Creature — Bird", "Flying", Some((1, 2))),
        card("Plains", "", "Basic Land — Plains", "({T}: Add {W}.)", None),
        card("Island", "", "Basic Land — Island", "({T}: Add {U}.)", None),
        card(
            "Aerial Engineer",
            "{2}{W}{U}",
            "Creature — Human Artificer",
            "As long as you control an artifact, this creature gets +2/+0 and has flying.",
            Some((2, 4)),
        ),
        card("Alpha Myr", "{2}", "Artifact Creature — Myr", "", Some((2, 1))),
        card(
            "Zealous Persecution",
            "{W}{B}",
            "Instant",
            "Until end of turn, creatures you control get +1/+1 and creatures your opponents control get -1/-1.",
            None,
        ),
    ]
}

fn game() -> Game {
    Game::new(defs(), 2, 20)
}

fn all_pass(g: &mut Game) {
    for _ in 0..g.players.len() {
        g.act(g.priority, Move::Pass).unwrap();
    }
}

/// Player 0 casts this card with mana of this land type, at this target if any, and it resolves.
fn cast(g: &mut Game, def: usize, land: usize, lands: usize, target: Option<ObjId>, spending: &str) {
    let card = g.put(0, def, Zone::Hand);
    for _ in 0..lands {
        let l = g.put(0, land, Zone::Battlefield);
        g.act(0, Move::Activate { source: l, ability: 0, targets: vec![], paying: vec![] }).unwrap();
    }
    let targets = target.map(|t| vec![vec![Target::Object(t)]]).unwrap_or_default();
    g.act(0, paid(spending, Move::Cast { card, targets })).unwrap();
    all_pass(g);
}

fn pt(g: &Game, o: ObjId) -> (i32, i32) {
    (g.power(o).unwrap(), g.toughness(o).unwrap())
}

#[test]
fn every_card_of_the_tests_is_one_the_engine_runs() {
    for d in defs() {
        check(&d).unwrap_or_else(|u| panic!("{}: {}", d.name(), u.0));
    }
}

#[test]
fn r613_4c_effects_and_counters_that_change_power_and_toughness_add_up() {
    let mut g = game();
    let bears = g.put(0, BEARS, Zone::Battlefield);
    assert_eq!(pt(&g, bears), (2, 2));
    g.put(0, ANTHEM, Zone::Battlefield);
    cite!(["611.3a"]; assert_eq!(pt(&g, bears), (3, 3), "a static ability's effect applies at once (611.3a)"));
    cast(&mut g, BATTLEGROWTH, FOREST, 1, Some(bears), "G");
    cite!(["122.1a"]; assert_eq!(pt(&g, bears), (4, 4), "a +1/+1 counter (122.1a)"));
    cast(&mut g, GROWTH, FOREST, 1, Some(bears), "G");
    cite!(["613.4c"]; assert_eq!(pt(&g, bears), (7, 7)));
    assert_eq!(g.obj(bears).chars.pt, Some((2, 2)), "what is printed does not change");
}

#[test]
fn r613_4b_power_and_toughness_are_set_before_they_are_changed() {
    // whichever was cast first: the 1/1 is set in layer 7b, +3/+3 and the counter in 7c
    let mut g = game();
    let bears = g.put(0, BEARS, Zone::Battlefield);
    cast(&mut g, GROWTH, FOREST, 1, Some(bears), "G");
    cast(&mut g, BATTLEGROWTH, FOREST, 1, Some(bears), "G");
    cast(&mut g, DIMINISH, ISLAND, 1, Some(bears), "U");
    cite!(["613.4b", "613.4c"]; assert_eq!(pt(&g, bears), (5, 5), "1/1, then +3/+3 and a counter"));
}

#[test]
fn r613_7_in_one_layer_the_later_timestamp_is_applied_last() {
    let seven = |g: &mut Game, mancer: ObjId, bears: ObjId| {
        let forest = g.put(0, FOREST, Zone::Battlefield);
        g.act(0, Move::Activate { source: forest, ability: 0, targets: vec![], paying: vec![] }).unwrap();
        g.act(0, paid("G", Move::Activate { source: mancer, ability: 0, targets: vec![vec![Target::Object(bears)]], paying: vec![] })).unwrap();
        all_pass(g);
    };
    let mut g = game();
    let (bears, mancer) = (g.put(0, BEARS, Zone::Battlefield), g.put(0, GIGANTOMANCER, Zone::Battlefield));
    cast(&mut g, DIMINISH, ISLAND, 1, Some(bears), "U");
    seven(&mut g, mancer, bears);
    cite!(["613.7"]; assert_eq!(pt(&g, bears), (7, 7)));
    let mut g = game();
    let (bears, mancer) = (g.put(0, BEARS, Zone::Battlefield), g.put(0, GIGANTOMANCER, Zone::Battlefield));
    seven(&mut g, mancer, bears);
    cast(&mut g, DIMINISH, ISLAND, 1, Some(bears), "U");
    cite!(["613.7"]; assert_eq!(pt(&g, bears), (1, 1)));
}

#[test]
fn r611_2a_an_effect_lasts_as_long_as_its_sentence_states() {
    // "Until end of turn, creatures you control get +1/+1 and creatures your opponents control
    // get -1/-1.": the duration the sentence begins with is of both things it says
    let mut g = game();
    g.put(0, BEARS, Zone::Library);
    g.put(1, BEARS, Zone::Library);
    let (mine, theirs) = (g.put(0, BEARS, Zone::Battlefield), g.put(1, BEARS, Zone::Battlefield));
    let card = g.put(0, PERSECUTION, Zone::Hand);
    for land in [PLAINS, SWAMP] {
        let l = g.put(0, land, Zone::Battlefield);
        g.act(0, Move::Activate { source: l, ability: 0, targets: vec![], paying: vec![] }).unwrap();
    }
    g.act(0, paid("WB", Move::Cast { card, targets: vec![] })).unwrap();
    all_pass(&mut g);
    assert_eq!((pt(&g, mine), pt(&g, theirs)), ((3, 3), (1, 1)));
    for _ in 0..60 {
        if g.active == 1 {
            break;
        }
        if g.decision.is_some() {
            g.act(0, Move::Attack(vec![])).unwrap();
        } else {
            g.act(g.priority, Move::Pass).unwrap();
        }
    }
    assert_eq!(g.active, 1);
    cite!(["611.2a", "514.2"]; assert_eq!((pt(&g, mine), pt(&g, theirs)), ((2, 2), (2, 2)), "both ended with the turn"));
    assert!(g.continuous.is_empty());
}

#[test]
fn r514_2_until_end_of_turn_ends_in_the_cleanup_step() {
    let mut g = game();
    g.put(0, BEARS, Zone::Library);
    g.put(1, BEARS, Zone::Library);
    let bears = g.put(0, BEARS, Zone::Battlefield);
    cast(&mut g, GROWTH, FOREST, 1, Some(bears), "G");
    cast(&mut g, BATTLEGROWTH, FOREST, 1, Some(bears), "G");
    assert_eq!(pt(&g, bears), (6, 6));
    let mut saw_end = false;
    for _ in 0..60 {
        if g.active == 1 {
            break;
        }
        if g.part == Part::End {
            saw_end = true;
            cite!(["514.2"]; assert_eq!(pt(&g, bears), (6, 6), "the effect lasts through the end step"));
        }
        if g.decision.is_some() {
            g.act(0, Move::Attack(vec![])).unwrap();
        } else {
            g.act(g.priority, Move::Pass).unwrap();
        }
    }
    assert_eq!(g.active, 1);
    assert!(saw_end, "the test must reach the end step before cleanup");
    cite!(["514.2"]; assert_eq!(pt(&g, bears), (3, 3), "the +3/+3 ended; the counter stays"));
    assert!(g.continuous.is_empty());
}

#[test]
fn r611_2c_a_resolved_effect_is_of_the_objects_it_affected_when_it_began() {
    // "{4}{W}: Creatures you control get +1/+1 until end of turn."
    let mut g = game();
    let elf = g.put(0, SUNBLADE, Zone::Battlefield);
    let bears = g.put(0, BEARS, Zone::Battlefield);
    let theirs = g.put(1, BEARS, Zone::Battlefield);
    for land in [FOREST, FOREST, FOREST, FOREST, PLAINS] {
        let l = g.put(0, land, Zone::Battlefield);
        g.act(0, Move::Activate { source: l, ability: 0, targets: vec![], paying: vec![] }).unwrap();
    }
    assert_eq!(pt(&g, elf), (2, 2), "it controls a Plains");
    g.act(0, paid("GGGGW", Move::Activate { source: elf, ability: 1, targets: vec![], paying: vec![] })).unwrap();
    all_pass(&mut g);
    assert_eq!((pt(&g, elf), pt(&g, bears), pt(&g, theirs)), ((3, 3), (3, 3), (2, 2)));
    // a creature that comes under its controller's control afterwards is not affected
    let late = g.put(0, BEARS, Zone::Battlefield);
    cite!(["611.2c"]; assert_eq!(pt(&g, late), (2, 2)));
    // while a static ability's effect is of whatever its text describes at the moment (611.3a)
    g.put(0, ANTHEM, Zone::Battlefield);
    cite!(["611.3a"]; assert_eq!((pt(&g, bears), pt(&g, late), pt(&g, theirs)), ((4, 4), (3, 3), (2, 2))));
}

#[test]
fn r611_3a_a_static_abilitys_effect_is_not_locked_in() {
    // "As long as you control an artifact, this creature gets +2/+0 and has flying.": the
    // condition in front of the sentence governs everything it states, asked at each moment
    let mut g = game();
    let engineer = g.put(0, ENGINEER, Zone::Battlefield);
    assert_eq!((pt(&g, engineer), g.has(engineer, "Flying")), ((2, 4), false));
    let theirs = g.put(1, MYR, Zone::Battlefield);
    assert_eq!((pt(&g, engineer), g.has(engineer, "Flying")), ((2, 4), false), "an opponent's artifact");
    let myr = g.put(0, MYR, Zone::Battlefield);
    cite!(["611.3a"]; assert_eq!((pt(&g, engineer), g.has(engineer, "Flying")), ((4, 4), true)));
    g.destroy(myr);
    cite!(["611.3a"]; assert_eq!((pt(&g, engineer), g.has(engineer, "Flying")), ((2, 4), false)));
    let _ = theirs;
}

#[test]
fn r611_3b_a_static_abilitys_effect_applies_while_its_permanent_is_on_the_battlefield() {
    let mut g = game();
    let bears = g.put(0, BEARS, Zone::Battlefield);
    let anthem = g.put(0, ANTHEM, Zone::Battlefield);
    assert_eq!(pt(&g, bears), (3, 3));
    g.destroy(anthem);
    cite!(["611.3b"]; assert_eq!(pt(&g, bears), (2, 2)));
    // "as long as you control a Swamp": asked each time
    let kavu = g.put(0, KAVU, Zone::Battlefield);
    assert_eq!(pt(&g, kavu), (3, 2));
    let swamp = g.put(0, SWAMP, Zone::Battlefield);
    cite!(["611.3a"]; assert_eq!(pt(&g, kavu), (4, 3)));
    g.destroy(swamp);
    cite!(["611.3a"]; assert_eq!(pt(&g, kavu), (3, 2)));
    // "Other Goblins you control": not itself, not another player's
    let master = g.put(0, TRASHMASTER, Zone::Battlefield);
    let (mine, theirs) = (g.put(0, FANATIC, Zone::Battlefield), g.put(1, FANATIC, Zone::Battlefield));
    assert_eq!((pt(&g, master), pt(&g, mine), pt(&g, theirs), pt(&g, bears)), ((3, 3), (2, 2), (1, 1), (2, 2)));
}

#[test]
fn r704_5f_a_creature_with_toughness_0_or_less_is_put_into_its_owners_graveyard() {
    let mut g = game();
    let bears = g.put(0, BEARS, Zone::Battlefield);
    let anthem = g.put(0, ANTHEM, Zone::Battlefield);
    cast(&mut g, DISFIGURE, SWAMP, 1, Some(bears), "B");
    assert_eq!(pt(&g, bears), (1, 1));
    assert!(g.battlefield.contains(&bears));
    // the Anthem leaves: its toughness is 0 the next time state-based actions are checked
    g.destroy(anthem);
    g.act(0, Move::Pass).unwrap();
    cite!(["704.5f"]; assert!(!g.battlefield.contains(&bears)));
    cite!(["704.5f"]; assert!(!g.log.contains(&Event::Destroyed { object: bears }), "put there, not destroyed"));
    assert!(g.players[0].graveyard.iter().any(|o| g.obj(*o).card == Some(BEARS)));
}

#[test]
fn r613_1_an_effect_sees_what_the_layers_before_its_own_have_done() {
    // "Creatures with flying get +2/+0. Creatures without flying get -2/-0.": flying is given
    // in layer 6, power is changed in layer 7c
    let mut g = game();
    let bears = g.put(0, BEARS, Zone::Battlefield);
    let crow = g.put(1, CROW, Zone::Battlefield);
    g.put(1, SHIFT, Zone::Battlefield);
    assert_eq!((pt(&g, bears), pt(&g, crow)), ((0, 2), (3, 2)));
    cast(&mut g, JUMP, ISLAND, 1, Some(bears), "U");
    cite!(["613.1"]; assert_eq!(pt(&g, bears), (4, 2), "it has flying now, so the other sentence is the one about it"));
}

#[test]
fn r613_1f_an_ability_a_permanent_is_given_is_one_it_has() {
    // "Creatures you control have haste.": a creature that came this turn attacks (702.10b)
    let mut g = game();
    g.put(0, FERVOR, Zone::Battlefield);
    cast(&mut g, BEARS, FOREST, 2, None, "GG");
    let bears = *g.battlefield.last().unwrap();
    all_pass(&mut g);
    all_pass(&mut g);
    assert_eq!(g.part, Part::DeclareAttackers);
    cite!(["613.1f", "702.10b"]; ok: g.act(0, Move::Attack(vec![(bears, Target::Player(1))])));

    // "gains flying": it can't be blocked except by creatures with flying or reach (702.9b)
    let mut g = game();
    let bears = g.put(0, BEARS, Zone::Battlefield);
    let (ground, flier) = (g.put(1, BEARS, Zone::Battlefield), g.put(1, CROW, Zone::Battlefield));
    cast(&mut g, JUMP, ISLAND, 1, Some(bears), "U");
    all_pass(&mut g);
    all_pass(&mut g);
    g.act(0, Move::Attack(vec![(bears, Target::Player(1))])).unwrap();
    all_pass(&mut g);
    cite!(["702.9b", "509.1b"]; refused("Grizzly Bears can't block Grizzly Bears: Grizzly Bears has flying (702.9b, 509.1b)"): g.act(1, Move::Block(vec![(ground, bears)])));
    cite!(["702.9b"]; ok: g.act(1, Move::Block(vec![(flier, bears)])));
}
