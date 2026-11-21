//! The numbers a text counts, rule by rule: X where the text says what it is (CR 107.3c), "for
//! each", what is counted and when (608.2h, 611.2d, 611.3a), and what is asked of an object
//! that has left the battlefield (its last known information). Every card is read from its
//! printed text.

use crate::support::{cite, paid};
use game::card::CardDef;
use game::play::check;
use game::state::Decision;
use game::{Game, Move, ObjId, Target, Zone};

const FOREST: usize = 0;
const SWAMP: usize = 1;
const MOUNTAIN: usize = 2;
const BEARS: usize = 3;
const GIANT: usize = 4;
const SOL_RING: usize = 5;
const CORRUPTION: usize = 6;
const MIGHT: usize = 7;
const SCOURGE: usize = 8;
const SERVANT: usize = 9;
const SHOCK: usize = 10;
const CLEAR_SHOT: usize = 11;
const SPROUTING: usize = 12;
const SHRINK: usize = 13;
const ARMOR: usize = 14;

fn defs() -> Vec<CardDef> {
    let card = |name, cost, types, text, pt| CardDef::new(name, cost, types, text, pt).expect(name);
    vec![
        card("Forest", "", "Basic Land — Forest", "({T}: Add {G}.)", None),
        card("Swamp", "", "Basic Land — Swamp", "({T}: Add {B}.)", None),
        card("Mountain", "", "Basic Land — Mountain", "({T}: Add {R}.)", None),
        card("Grizzly Bears", "{1}{G}", "Creature — Bear", "", Some((2, 2))),
        card("Hill Giant", "{3}{R}", "Creature — Giant", "", Some((3, 3))),
        card("Sol Ring", "{1}", "Artifact", "{T}: Add {C}{C}.", None),
        card(
            "Monumental Corruption",
            "{3}{B}{B}",
            "Sorcery",
            "Target player draws X cards and loses X life, where X is the number of artifacts you control.",
            None,
        ),
        card("Might of the Masses", "{G}", "Instant", "Target creature gets +1/+1 until end of turn for each creature you control.", None),
        card(
            "Scourge of Geier Reach",
            "{3}{R}{R}",
            "Creature — Elemental",
            "This creature gets +1/+1 for each creature your opponents control.",
            Some((3, 3)),
        ),
        card(
            "Servant of the Scale",
            "{G}",
            "Creature — Human Soldier",
            "This creature enters with a +1/+1 counter on it.\nWhen this creature dies, put X +1/+1 counters on target creature you control, where X is the number of +1/+1 counters on this creature.",
            Some((0, 0)),
        ),
        card("Shock", "{R}", "Instant", "Shock deals 2 damage to any target.", None),
        card(
            "Clear Shot",
            "{2}{G}",
            "Instant",
            "Target creature you control gets +1/+1 until end of turn. It deals damage equal to its power to target creature you don't control.",
            None,
        ),
        card(
            "Fungal Sprouting",
            "{3}{G}",
            "Sorcery",
            "Create X 1/1 green Saproling creature tokens, where X is the greatest power among creatures you control.",
            None,
        ),
        card("Shrink", "{G}", "Instant", "Target creature gets -5/-0 until end of turn.", None),
        card(
            "Living Armor",
            "{4}",
            "Artifact",
            "{T}, Sacrifice this artifact: Put X +0/+1 counters on target creature, where X is that creature's mana value.",
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

/// Player 0 casts this card with the mana of these lands at these targets, and it resolves.
fn cast(g: &mut Game, def: usize, lands: &[usize], targets: Vec<Vec<Target>>, spending: &str) {
    for land in lands {
        let land = g.put(0, *land, Zone::Battlefield);
        g.act(0, Move::Activate { source: land, ability: 0, targets: vec![], paying: vec![] }).unwrap();
    }
    let card = g.put(0, def, Zone::Hand);
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
fn r613_8a_a_static_number_counted_from_power_is_not_run_yet() {
    // the effect depends on whatever else changes power in its layer: refused, not guessed
    let grub = CardDef::new(
        "Carrion Grub",
        "{3}{B}",
        "Creature — Insect",
        "This creature gets +X/+0, where X is the greatest power among creature cards in your graveyard.\nWhen this creature enters, mill four cards. (Put the top four cards of your library into your graveyard.)",
        Some((0, 5)),
    )
    .unwrap();
    assert!(check(&grub).unwrap_err().0.contains("power or toughness"));
}

#[test]
fn r106_3_counted_mana_with_fixed_mana_and_activation_costs_is_admitted() {
    let stronghold = CardDef::new("Cabal Stronghold", "", "Land", "{T}: Add {C}.\n{3}, {T}: Add {B} for each basic Swamp you control.", None).unwrap();
    check(&stronghold).unwrap();
}

#[test]
fn r107_1b_a_result_that_would_be_negative_is_zero() {
    // a power can be less than zero; damage equal to it can't: the Bears at -3/2, then -2/3
    // with Clear Shot's +1/+1, deal no damage
    let mut g = game();
    let bears = g.put(0, BEARS, Zone::Battlefield);
    let giant = g.put(1, GIANT, Zone::Battlefield);
    cast(&mut g, SHRINK, &[FOREST], vec![vec![Target::Object(bears)]], "G");
    assert_eq!(pt(&g, bears), (-3, 2));
    cast(&mut g, CLEAR_SHOT, &[FOREST, FOREST, FOREST], vec![vec![Target::Object(bears)], vec![Target::Object(giant)]], "GGG");
    assert_eq!(pt(&g, bears), (-2, 3));
    cite!(["107.1b"]; assert_eq!((g.obj(giant).zone, g.obj(giant).damage), (Zone::Battlefield, 0)));
}

#[test]
fn r107_3c_x_has_the_value_the_text_defines() {
    // "Target player draws X cards and loses X life, where X is the number of artifacts you
    // control.": two, in both of its acts
    let mut g = game();
    for _ in 0..4 {
        g.put(1, FOREST, Zone::Library);
    }
    g.put(0, SOL_RING, Zone::Battlefield);
    g.put(0, SOL_RING, Zone::Battlefield);
    g.put(1, SOL_RING, Zone::Battlefield);
    cast(&mut g, CORRUPTION, &[SWAMP, SWAMP, SWAMP, SWAMP, SWAMP], vec![vec![Target::Player(1)]], "BBBBB");
    cite!(["107.3c"]; assert_eq!((g.players[1].hand.len(), g.players[1].life), (2, 18)));
    // "Create X 1/1 green Saproling creature tokens, where X is the greatest power among
    // creatures you control."
    let mut g = game();
    g.put(0, BEARS, Zone::Battlefield);
    g.put(0, GIANT, Zone::Battlefield);
    g.put(1, SCOURGE, Zone::Battlefield);
    cast(&mut g, SPROUTING, &[FOREST, FOREST, FOREST, FOREST], vec![], "GGGG");
    cite!(["107.3c"]; assert_eq!(g.battlefield.iter().filter(|o| g.obj(**o).chars.name == "Saproling Token").count(), 3));
}

#[test]
fn r611_2d_a_variable_in_a_continuous_effect_is_determined_once_on_resolution() {
    // "Target creature gets +1/+1 until end of turn for each creature you control.": two as
    // it resolves; a third creature afterwards changes nothing
    let mut g = game();
    let bears = g.put(0, BEARS, Zone::Battlefield);
    g.put(0, GIANT, Zone::Battlefield);
    g.put(1, GIANT, Zone::Battlefield);
    cast(&mut g, MIGHT, &[FOREST], vec![vec![Target::Object(bears)]], "G");
    cite!(["611.2d"]; assert_eq!(pt(&g, bears), (4, 4)));
    g.put(0, GIANT, Zone::Battlefield);
    cite!(["611.2d"]; assert_eq!(pt(&g, bears), (4, 4)));
}

#[test]
fn r611_3a_what_a_static_ability_counts_is_counted_at_any_given_moment() {
    // "This creature gets +1/+1 for each creature your opponents control."
    let mut g = game();
    let scourge = g.put(0, SCOURGE, Zone::Battlefield);
    g.put(0, BEARS, Zone::Battlefield);
    assert_eq!(pt(&g, scourge), (3, 3), "its controller's creatures are not counted");
    let theirs = g.put(1, BEARS, Zone::Battlefield);
    g.put(1, GIANT, Zone::Battlefield);
    cite!(["611.3a"]; assert_eq!(pt(&g, scourge), (5, 5)));
    cast(&mut g, SHOCK, &[MOUNTAIN], vec![vec![Target::Object(theirs)]], "R");
    cite!(["611.3a"]; assert_eq!(pt(&g, scourge), (4, 4), "one of them has died"));
}

#[test]
fn r608_2h_an_object_that_has_left_its_zone_is_asked_as_it_last_was() {
    // "When this creature dies, put X +1/+1 counters on target creature you control, where X
    // is the number of +1/+1 counters on this creature.": the one it entered with, counted on
    // the creature as it last existed on the battlefield
    let mut g = game();
    let bears = g.put(0, BEARS, Zone::Battlefield);
    // (put onto the battlefield as it would have entered)
    let servant = g.put(0, SERVANT, Zone::Battlefield);
    g.as_entered(servant);
    assert_eq!(pt(&g, servant), (1, 1));
    cast(&mut g, SHOCK, &[MOUNTAIN], vec![vec![Target::Object(servant)]], "R");
    assert!(matches!(g.decision, Some(Decision::Targets { player: 0, .. })), "it died, and its ability triggered");
    g.act(0, Move::ChooseTargets(vec![vec![Target::Object(bears)]])).unwrap();
    all_pass(&mut g);
    cite!(["608.2h"]; assert_eq!(g.obj(bears).counters.get("+1/+1"), Some(&1)));
    assert_eq!(pt(&g, bears), (3, 3));
}

#[test]
fn r608_2h_a_number_said_of_the_acts_own_target_is_of_that_target() {
    // "Put X +0/+1 counters on target creature, where X is that creature's mana value.": the
    // Bears' two, not the four of the artifact whose ability it is
    let mut g = game();
    let bears = g.put(0, BEARS, Zone::Battlefield);
    let armor = g.put(0, ARMOR, Zone::Battlefield);
    g.act(0, Move::Activate { source: armor, ability: 0, targets: vec![vec![Target::Object(bears)]], paying: vec![] }).unwrap();
    all_pass(&mut g);
    cite!(["608.2h"]; assert_eq!(g.obj(bears).counters.get("+0/+1"), Some(&2)));
    assert_eq!(pt(&g, bears), (2, 4));
}

#[test]
fn r608_2h_an_object_in_its_zone_is_asked_as_it_is() {
    // "Target creature you control gets +1/+1 until end of turn. It deals damage equal to
    // its power to target creature you don't control.": its power with the +1/+1
    let mut g = game();
    let bears = g.put(0, BEARS, Zone::Battlefield);
    let giant = g.put(1, GIANT, Zone::Battlefield);
    cast(&mut g, CLEAR_SHOT, &[FOREST, FOREST, FOREST], vec![vec![Target::Object(bears)], vec![Target::Object(giant)]], "GGG");
    cite!(["608.2h"]; assert_eq!(g.obj(giant).zone, Zone::Gone, "3 damage to a 3/3"));
    assert_eq!(pt(&g, bears), (3, 3));
}
