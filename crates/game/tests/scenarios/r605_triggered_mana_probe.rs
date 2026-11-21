//! Triggered mana abilities (605.1b, 605.4a) against "tap for mana"
//! (106.12, 106.12a). Badgermole Cub's ruling (2025-10-02): it triggers only when a creature's
//! own mana ability with {T} in its cost is activated, not for a creature tapped to pay
//! another object's mana ability (Springleaf Drum), nor for one tapped for convoke. High
//! Tide's ruling (2022-12-08): an Island that entered after it resolved is affected too.
use crate::support;
use support::*;
use ir::mana::ManaSymbol;
use ir::{CardType, Color};

/// Badgermole Cub enters from player 0's hand and earthbends the Forest: the Forest is now
/// a hasty land creature, so tapping it for mana is tapping a creature for mana.
fn cub_earthbends_forest(t: &mut T) -> ObjId {
    let forest = t.find_in_battlefield("Forest");
    t.mana(0, "{1}{G}").cast(0, "Badgermole Cub", vec![], crate::support::mana_pool("CG")).unwrap();
    let Boundary::Question(0, _) = t.settle() else { panic!("the earthbend target is asked: {:?}", t.asked()) };
    t.g.act(0, Move::ChooseTargets(vec![vec![Target::Object(forest)]])).unwrap();
    t.resolve();
    assert!(t.g.is_a(forest, CardType::Creature) && t.g.is_a(forest, CardType::Land));
    forest
}

#[test]
fn r106_12_an_earthbent_land_tapped_for_mana_is_a_creature_tapped_for_mana() {
    let mut t = Board::new(2).battlefield(0, &["Forest"]).hand(0, &["Badgermole Cub"]).build();
    let forest = cub_earthbends_forest(&mut t);
    t.g.act(0, Move::Activate { source: forest, ability: 0, targets: vec![], paying: vec![] }).unwrap();
    cite!(["106.12a"]; assert_eq!(t.g.players[0].pool.colors[4], 2, "the Forest's {{G}} and the Cub's additional {{G}}"));
    cite!(["605.4a"]; assert!(t.g.stack.is_empty()));
}

/// Nissa's "whenever you tap a Forest for mana" and the Cub's "a creature" both trigger on
/// one tap of an earthbent Forest: three {G}.
#[test]
fn r605_4a_two_triggered_mana_abilities_from_one_tap_both_resolve() {
    // with loyalty, so that she stays on the battlefield (704.5i)
    let mut t = Board::new(2).battlefield(0, &["Forest", "Nissa, Who Shakes the World"]).counters("Nissa, Who Shakes the World", "loyalty", 5)
        .hand(0, &["Badgermole Cub"]).build();
    let forest = cub_earthbends_forest(&mut t);
    t.g.act(0, Move::Activate { source: forest, ability: 0, targets: vec![], paying: vec![] }).unwrap();
    cite!(["605.4a"]; assert_eq!(t.g.players[0].pool.colors[4], 3));
    cite!(["605.4a"]; assert!(t.g.stack.is_empty()));
}

/// The supplied payment includes the Cub's {G} for an earthbent land: it alone pays {1}{G}.
#[test]
fn r601_2g_an_earthbent_land_alone_pays_two_with_the_cub() {
    let mut t = Board::new(2).battlefield(0, &["Forest"]).hand(0, &["Badgermole Cub", "Grizzly Bears"]).build();
    let forest = cub_earthbends_forest(&mut t);
    let bears = t.find_in(0, Zone::Hand, "Grizzly Bears");
    let plan = vec![game::legal::Step { plan: vec![], x: None, life: vec![], with: None, source: forest, ability: 0, paying: vec![], adding: vec![], chose: vec![], cost_branches: None }];
    cite!(["601.2h"]; ok: t.g.act(0, Move::WithMana(plan, Box::new(paid("GG",Move::Cast { card: bears, targets: vec![] })))));
    t.expect().stack(&["Grizzly Bears"]).check();
    assert_eq!(t.g.players[0].pool.total(), 0);
}

/// Springleaf Drum: "{T}, Tap an untapped creature you control: Add one mana of any color."
/// The Elves are tapped to pay the Drum's cost, not for mana: the Cub doesn't trigger.
#[test]
fn r106_12_a_creature_tapped_for_another_objects_mana_ability_is_not_tapped_for_mana() {
    let mut t = Board::new(2).battlefield(0, &["Badgermole Cub", "Springleaf Drum", "Llanowar Elves"]).build();
    let (drum, elves) = (t.find_in_battlefield("Springleaf Drum"), t.find_in_battlefield("Llanowar Elves"));
    t.g.act(0, Move::Adding(vec![vec![ManaSymbol::Color(Color::Red)]],
        Box::new(Move::Activate { source: drum, ability: 0, targets: vec![], paying: vec![vec![elves]] }))).unwrap();
    assert!(t.g.obj(elves).tapped);
    cite!(["106.12"]; assert_eq!((t.g.players[0].pool.colors[3], t.g.players[0].pool.colors[4]), (1, 0), "only the Drum's {{R}}"));
}

/// Convoke taps a creature to pay a cost; it adds no mana and is not tapping it for mana.
#[test]
fn r702_51a_a_creature_tapped_for_convoke_is_not_tapped_for_mana() {
    let defs = vec![
        oracle("Badgermole Cub"),
        CardDef::new("Gather", "{2}{G}", "Instant", "Convoke\nYou gain 1 life.", None).unwrap(),
        oracle("Llanowar Elves"),
    ];
    for d in &defs { game::play::check(d).unwrap(); }
    let mut g = Game::new(defs, 2, 20);
    for p in 0..2 { for _ in 0..5 { g.put(p, 2, Zone::Library); } }
    while g.part != ir::Part::PrecombatMain { g.act(g.priority, Move::Pass).unwrap(); }
    g.put(0, 0, Zone::Battlefield);
    let elves = g.put(0, 2, Zone::Battlefield);
    let card = g.put(0, 1, Zone::Hand);
    g.players[0].pool.colors[4] = 2;
    g.act(0, Move::WithConvoke(vec![None], Box::new(Move::WithCosts(vec![vec![elves]], Box::new(paid("GG",Move::Cast { card, targets: vec![] })))))).unwrap();
    assert!(g.obj(elves).tapped);
    cite!(["702.51a", "106.12"]; assert_eq!(g.players[0].pool.total(), 0, "the Cub added nothing for the convoked Elves"));
}

/// High Tide: "whenever a player taps an Island for mana, that player adds an additional
/// {U}" — an opponent too; two High Tides add two; it ends with the turn.
#[test]
fn r605_4a_high_tide_is_for_every_player_stacks_and_ends_with_the_turn() {
    let mut t = Board::new(2).fill(10).battlefield(0, &["Island", "Island", "Island", "Island"]).battlefield(1, &["Island"])
        .named(0, Zone::Hand, "tide1", "High Tide").named(0, Zone::Hand, "tide2", "High Tide").build();
    for label in ["tide1", "tide2"] {
        let card = t.id(label);
        t.mana(0, "{U}").g.act(0, paid("U",Move::Cast { card, targets: vec![] })).unwrap();
        t.resolve();
    }
    let theirs = t.find_in(1, Zone::Battlefield, "Island");
    let mine: Vec<ObjId> = t.g.battlefield.iter().copied().filter(|&o| t.g.obj(o).controller == 0).collect();
    t.g.act(0, Move::Activate { source: mine[0], ability: 0, targets: vec![], paying: vec![] }).unwrap();
    cite!(["605.4a"]; assert_eq!(t.g.players[0].pool.colors[1], 3, "its {{U}} and one from each High Tide"));
    t.g.act(0, Move::Pass).unwrap();
    t.g.act(1, Move::Activate { source: theirs, ability: 0, targets: vec![], paying: vec![] }).unwrap();
    assert_eq!(t.g.players[1].pool.colors[1], 3, "a player: the opponent's Island too");
    t.g.act(1, Move::Pass).unwrap();
    t.to_no_combat(1, ir::Part::PrecombatMain);
    let theirs = t.find_in(1, Zone::Battlefield, "Island");
    t.g.act(1, Move::Activate { source: theirs, ability: 0, targets: vec![], paying: vec![] }).unwrap();
    cite!(["611.2a"]; assert_eq!(t.g.players[1].pool.colors[1], 1, "until end of turn: next turn one {{U}}"));
}

/// Wild Growth: "Whenever enchanted land is tapped for mana": a land tapped for an ability
/// that adds no mana is not tapped for mana (106.12).
#[test]
fn r605_1b_a_land_tapped_by_a_non_mana_ability_does_not_trigger_it() {
    let defs = vec![
        CardDef::new("Watchtower", "", "Land", "{T}: Add {G}.\n{T}: You gain 1 life.", None).unwrap(),
        oracle("Wild Growth"),
        CardDef::new("Forest", "", "Basic Land — Forest", "", None).unwrap(),
    ];
    for d in &defs { game::play::check(d).unwrap(); }
    let mut g = Game::new(defs, 2, 20);
    for p in 0..2 { for _ in 0..5 { g.put(p, 2, Zone::Library); } }
    while g.part != ir::Part::PrecombatMain { g.act(g.priority, Move::Pass).unwrap(); }
    let tower = g.put(0, 0, Zone::Battlefield);
    let pay = g.put(0, 2, Zone::Battlefield);
    g.act(0, Move::Activate { source: pay, ability: 0, targets: vec![], paying: vec![] }).unwrap();
    let aura = g.put(0, 1, Zone::Hand);
    g.act(0, paid("G",Move::Cast { card: aura, targets: vec![vec![Target::Object(tower)]] })).unwrap();
    while !g.stack.is_empty() { g.act(g.priority, Move::Pass).unwrap(); }
    g.act(0, Move::Activate { source: tower, ability: 1, targets: vec![], paying: vec![] }).unwrap();
    assert!(g.obj(tower).tapped);
    cite!(["106.12"]; assert_eq!(g.players[0].pool.total(), 0, "tapped, but not for mana: no additional {{G}}"));
}

/// A mana ability, triggered mana abilities with it, leaves priority with the player who
/// activated it (117.3c, 605.4a): the non-active player taps an Island under High Tide.
#[test]
fn r117_3c_the_player_who_taps_for_mana_keeps_priority() {
    for tide in [false, true] {
        let mut t = Board::new(2).fill(10).battlefield(0, &["Island"]).battlefield(1, &["Island"]).hand(0, &["High Tide"]).build();
        if tide {
            t.mana(0, "{U}").cast(0, "High Tide", vec![], crate::support::mana_pool("U")).unwrap();
            t.resolve();
        }
        t.g.act(0, Move::Pass).unwrap();
        let theirs = t.find_in(1, Zone::Battlefield, "Island");
        t.g.act(1, Move::Activate { source: theirs, ability: 0, targets: vec![], paying: vec![] }).unwrap();
        assert_eq!(t.g.players[1].pool.colors[1], if tide { 2 } else { 1 });
        cite!(["117.3c"]; case: format!("High Tide {tide}"); assert_eq!(t.g.priority, 1, "High Tide {tide}: player 1 still has priority"));
    }
}
