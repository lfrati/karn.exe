//! The source of a delayed triggered ability made by a spell is that spell (603.7d). When the
//! ability later says the spell deals damage, the spell has left the stack: it is the spell as
//! it last existed there that deals it (113.7a, 608.2h, 400.7), and triggers that ask what
//! dealt the damage see a spell of its colors, types and controller.
//! Every card is read from its printed text; the blue copy and the artifact are contrasts.

use crate::support::{cite, paid};
use game::card::CardDef;
use game::play::check;
use game::{Game, Move, ObjId, Target, Zone};

const MOUNTAIN: usize = 0;
const ISLAND: usize = 1;
const BEARS: usize = 2;
const BLOOD: usize = 3;
const PHOENIX: usize = 4;
const FRIGID: usize = 5;
const TOTEM: usize = 6;
const RITE: usize = 7;
const FIELD: usize = 8;
const FANATIC: usize = 9;
const TEPHRADERM: usize = 10;

const DELAYED: &str = "deals 2 damage to target creature. When that creature dies this turn, ";

fn defs() -> Vec<CardDef> {
    let card = |name, cost, types, text: String, pt| CardDef::new(name, cost, types, &text, pt).expect(name);
    vec![
        card("Mountain", "", "Basic Land — Mountain", "({T}: Add {R}.)".into(), None),
        card("Island", "", "Basic Land — Island", "({T}: Add {U}.)".into(), None),
        card("Grizzly Bears", "{1}{G}", "Creature — Bear", String::new(), Some((2, 2))),
        card(
            "Searing Blood",
            "{R}{R}",
            "Instant",
            "Searing Blood deals 2 damage to target creature. When that creature dies this turn, Searing Blood deals 3 damage to the creature's controller.".into(),
            None,
        ),
        card(
            "Chandra's Phoenix",
            "{1}{R}{R}",
            "Creature — Phoenix",
            "Flying\nHaste (This creature can attack and {T} as soon as it comes under your control.)\nWhenever an opponent is dealt damage by a red instant or sorcery spell you control or by a red planeswalker you control, return this card from your graveyard to your hand.".into(),
            Some((2, 2)),
        ),
        // contrasts: the same words on a blue instant, on a red artifact's ability, and on a
        // red sorcery
        card("Frigid Blood", "{U}{U}", "Instant", format!("Frigid Blood {DELAYED}Frigid Blood deals 3 damage to the creature's controller."), None),
        card("Ember Totem", "{R}", "Artifact", format!("{{T}}: This artifact {DELAYED}this artifact deals 3 damage to the creature's controller."), None),
        card("Searing Rite", "{R}{R}", "Sorcery", format!("Searing Rite {DELAYED}Searing Rite deals 3 damage to the creature's controller."), None),
        card("Dissipation Field", "{2}{U}{U}", "Enchantment", "Whenever a permanent deals damage to you, return it to its owner's hand.".into(), None),
        card("Mogg Fanatic", "{R}", "Creature — Goblin", "Sacrifice this creature: It deals 1 damage to any target.".into(), Some((1, 1))),
        card(
            "Tephraderm",
            "{4}{R}",
            "Creature — Beast",
            "Whenever a creature deals damage to this creature, this creature deals that much damage to that creature.\nWhenever a spell deals damage to this creature, this creature deals that much damage to that spell's controller.".into(),
            Some((4, 5)),
        ),
    ]
}

fn game() -> Game {
    let mut g = Game::new(defs(), 2, 20);
    for player in [0, 1] {
        for _ in 0..4 {
            g.put(player, MOUNTAIN, Zone::Library);
        }
    }
    g
}

fn pass_both(g: &mut Game) {
    let first = g.priority;
    g.act(first, Move::Pass).unwrap();
    g.act(1 - first, Move::Pass).unwrap();
}

/// Player 0 casts this card with the mana of two of these lands at this creature; it resolves
/// and the creature, dealt 2 damage, dies: the delayed ability is on the stack.
fn kill(g: &mut Game, def: usize, land: usize, target: ObjId, spending: &str) {
    for _ in 0..2 {
        let land = g.put(0, land, Zone::Battlefield);
        g.act(0, Move::Activate { source: land, ability: 0, targets: vec![], paying: vec![] }).unwrap();
    }
    let card = g.put(0, def, Zone::Hand);
    g.act(0, paid(spending, Move::Cast { card, targets: vec![vec![Target::Object(target)]] })).unwrap();
    pass_both(g);
    assert_eq!(g.obj(target).zone, Zone::Gone);
    assert_eq!(g.stack.len(), 1, "the delayed ability triggered");
}

fn names(g: &Game, cards: &[ObjId]) -> Vec<String> {
    let mut names: Vec<String> = cards.iter().map(|c| g.obj(*c).chars.name.clone()).collect();
    names.sort();
    names
}

/// Where the triggered abilities on the stack come from.
fn sources(g: &Game) -> Vec<Option<ObjId>> {
    g.stack.iter().map(|o| g.obj(*o).ability.and_then(|(source, _)| source)).collect()
}

#[test]
fn every_card_of_the_tests_is_one_the_engine_runs() {
    for d in defs() {
        check(&d).unwrap_or_else(|u| panic!("{}: {}", d.name(), u.0));
    }
}

#[test]
fn r603_7d_the_spell_that_made_it_deals_the_damage_as_it_last_existed() {
    // "Whenever an opponent is dealt damage by a red instant or sorcery spell you control …":
    // Searing Blood has resolved and is a card in the graveyard when its delayed ability deals
    // the 3 damage; the red instant spell player 0 controlled deals it (113.7a, 608.2h)
    let mut g = game();
    let phoenix = g.put(0, PHOENIX, Zone::Graveyard);
    let bears = g.put(1, BEARS, Zone::Battlefield);
    kill(&mut g, BLOOD, MOUNTAIN, bears, "RR");
    assert_eq!(names(&g, &g.players[0].graveyard), ["Chandra's Phoenix", "Searing Blood"]);
    pass_both(&mut g);
    assert_eq!((g.players[0].life, g.players[1].life), (20, 17));
    // it triggered once, in the graveyard
    cite!(["603.7d", "608.2h"]; assert_eq!(sources(&g), [Some(phoenix)]));
    pass_both(&mut g);
    assert_eq!(names(&g, &g.players[0].hand), ["Chandra's Phoenix"]);
    assert_eq!(names(&g, &g.players[0].graveyard), ["Searing Blood"]);
    assert!(g.stack.is_empty());
}

#[test]
fn r603_7d_a_sorcery_that_made_it_is_one_too() {
    let mut g = game();
    let phoenix = g.put(0, PHOENIX, Zone::Graveyard);
    let bears = g.put(1, BEARS, Zone::Battlefield);
    kill(&mut g, RITE, MOUNTAIN, bears, "RR");
    pass_both(&mut g);
    assert_eq!(g.players[1].life, 17);
    cite!(["603.7d"]; assert_eq!(sources(&g), [Some(phoenix)]));
}

#[test]
fn r603_7d_the_spell_is_controlled_by_who_controlled_it_as_it_resolved() {
    // player 0's Searing Blood kills player 0's own creature and deals 3 damage to player 0:
    // player 1's Phoenix sees its opponent dealt damage, by a spell player 1 did not control;
    // player 0's Phoenix sees player 0, who is not its controller's opponent
    let mut g = game();
    g.put(0, PHOENIX, Zone::Graveyard);
    g.put(1, PHOENIX, Zone::Graveyard);
    let bears = g.put(0, BEARS, Zone::Battlefield);
    kill(&mut g, BLOOD, MOUNTAIN, bears, "RR");
    pass_both(&mut g);
    assert_eq!((g.players[0].life, g.players[1].life), (17, 20));
    cite!(["603.7d"]; assert!(g.stack.is_empty(), "nothing triggered: {:?}", sources(&g)));
}

#[test]
fn r603_7d_a_spell_of_another_color_is_not_red() {
    let mut g = game();
    g.put(0, PHOENIX, Zone::Graveyard);
    let bears = g.put(1, BEARS, Zone::Battlefield);
    kill(&mut g, FRIGID, ISLAND, bears, "UU");
    pass_both(&mut g);
    assert_eq!(g.players[1].life, 17);
    cite!(["603.7d"]; assert!(g.stack.is_empty(), "nothing triggered: {:?}", sources(&g)));
}

#[test]
fn r603_7e_made_by_an_artifacts_ability_the_artifact_is_its_source_and_no_spell() {
    // the source of a delayed ability made by an activated ability is that ability's source
    // (603.7e): a red artifact, not a red instant or sorcery spell
    let mut g = game();
    g.put(0, PHOENIX, Zone::Graveyard);
    let totem = g.put(0, TOTEM, Zone::Battlefield);
    let bears = g.put(1, BEARS, Zone::Battlefield);
    g.act(0, Move::Activate { source: totem, ability: 0, targets: vec![vec![Target::Object(bears)]], paying: vec![] }).unwrap();
    pass_both(&mut g);
    assert_eq!(g.obj(bears).zone, Zone::Gone);
    assert_eq!(g.stack.len(), 1, "the delayed ability triggered");
    pass_both(&mut g);
    assert_eq!(g.players[1].life, 17);
    cite!(["603.7e"]; assert!(g.stack.is_empty(), "nothing triggered: {:?}", sources(&g)));
}

#[test]
fn r400_7_the_spell_as_it_last_existed_is_no_permanent() {
    // "Whenever a permanent deals damage to you, …": player 1's Dissipation Field does not see
    // Searing Blood's delayed damage as dealt by a permanent; the spell never was one, and the
    // card it became in the graveyard is a new object (400.7)
    let mut g = game();
    g.put(1, FIELD, Zone::Battlefield);
    let bears = g.put(1, BEARS, Zone::Battlefield);
    kill(&mut g, BLOOD, MOUNTAIN, bears, "RR");
    pass_both(&mut g);
    assert_eq!(g.players[1].life, 17);
    cite!(["400.7", "608.2h"]; assert!(g.stack.is_empty(), "nothing triggered: {:?}", sources(&g)));
}

#[test]
fn r608_2h_a_permanent_that_has_left_deals_damage_as_the_permanent_it_last_was() {
    // "Sacrifice this creature: It deals 1 damage to any target.": the Fanatic is gone when its
    // ability resolves; the creature it last was deals the damage (113.7a, 608.2h). Tephraderm
    // sees a creature deal damage to it, and no spell.
    let mut g = game();
    let tephraderm = g.put(1, TEPHRADERM, Zone::Battlefield);
    let fanatic = g.put(0, FANATIC, Zone::Battlefield);
    g.act(0, Move::Activate { source: fanatic, ability: 0, targets: vec![vec![Target::Object(tephraderm)]], paying: vec![] }).unwrap();
    pass_both(&mut g);
    assert_eq!(g.obj(fanatic).zone, Zone::Gone);
    assert_eq!(g.obj(tephraderm).damage, 1);
    let [ability] = g.stack[..] else { panic!("one ability triggered: {:?}", sources(&g)) };
    cite!(["113.7a", "608.2h"]; assert_eq!(g.obj(ability).ability, Some((Some(tephraderm), 0)), "the creature ability, not the spell one"));
    pass_both(&mut g);
    assert_eq!((g.players[0].life, g.players[1].life), (20, 20));
    assert!(g.stack.is_empty());
}
