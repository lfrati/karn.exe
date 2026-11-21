//! Flashback with a cost that is not mana (702.34a, 118.9, 601.2f-h): "Flashback—Sacrifice a
//! Mountain." The card is cast from its owner's graveyard for that cost instead of its mana
//! cost, the sacrifice is paid as costs are, and it is exiled when it leaves the stack.
//! Every card is its printed Oracle text.
use crate::support::cite;
use game::announcement::CastRoute;
use game::card::CardDef;
use game::play::check;
use game::{Game, Move, ObjId, Target, Zone};
use ir::mana::ManaSymbol;
use ir::Color;

const DART: usize = 0;
const MOUNTAIN: usize = 1;
const TAIGA: usize = 2;
const FOREST: usize = 3;
const BEARS: usize = 4;
const DENY: usize = 5;
const ANALYSIS: usize = 6;
const DISCOUNT: usize = 7;

fn defs() -> Vec<CardDef> {
    let card = |name, cost, types, text, pt| CardDef::new(name, cost, types, text, pt).expect(name);
    vec![
        card("Lava Dart", "{R}", "Instant", "Lava Dart deals 1 damage to any target.\nFlashback—Sacrifice a Mountain. (You may cast this card from your graveyard for its flashback cost. Then exile it.)", None),
        card("Mountain", "", "Basic Land — Mountain", "({T}: Add {R}.)", None),
        // a nonbasic Mountain is a Mountain all the same (205.3i)
        card("Taiga", "", "Land — Mountain Forest", "({T}: Add {R} or {G}.)", None),
        card("Forest", "", "Basic Land — Forest", "({T}: Add {G}.)", None),
        card("Grizzly Bears", "{1}{G}", "Creature — Bear", "", Some((2, 2))),
        card("Deny", "{0}", "Instant", "Counter target spell.", None),
        card("Deep Analysis", "{3}{U}", "Sorcery", "Target player draws two cards.\nFlashback—{1}{U}, Pay 3 life. (You may cast this card from your graveyard for its flashback cost. Then exile it.)", None),
        card("Discount", "{0}", "Enchantment", "Instant spells you cast cost {1} less to cast.", None),
    ]
}

fn game() -> Game {
    let mut g = Game::new(defs(), 2, 20);
    for p in 0..2 { for _ in 0..5 { g.put(p, FOREST, Zone::Library); } }
    g
}

fn resolve(g: &mut Game) {
    for _ in 0..20 {
        if g.stack.is_empty() { return; }
        g.act(g.priority, Move::Pass).unwrap();
    }
}

fn dart_in_graveyard(g: &mut Game) -> ObjId { g.put(0, DART, Zone::Graveyard) }

fn flashback(g: &mut Game, dart: ObjId, sacrifice: ObjId, target: Target) -> Result<(), String> {
    g.act(0, Move::CastVia(CastRoute::Flashback, Box::new(Move::WithCosts(vec![vec![sacrifice]], Box::new(Move::Cast { card: dart, targets: vec![vec![target]] })))))
}



#[test]
fn every_card_of_the_tests_is_one_the_engine_runs() {
    for d in defs() {
        check(&d).unwrap_or_else(|u| panic!("{}: {}", d.name(), u.0));
    }
}

#[test]
fn r702_34a_from_the_hand_it_costs_its_mana_cost_and_goes_to_the_graveyard() {
    let mut g = game();
    let dart = g.put(0, DART, Zone::Hand);
    g.add_mana(0, ManaSymbol::Color(Color::Red));
    g.act(0, crate::support::paid("R",Move::Cast { card: dart, targets: vec![vec![Target::Player(1)]] })).unwrap();
    resolve(&mut g);
    assert_eq!(g.players[1].life, 19);
    cite!(["608.2n"]; assert_eq!(g.players[0].graveyard.len(), 1, "an ordinary instant goes to its owner's graveyard"));
}

#[test]
fn r702_34a_from_the_graveyard_it_costs_a_mountain_sacrificed_and_is_then_exiled() {
    for land in [MOUNTAIN, TAIGA] {
        let mut g = game();
        let dart = dart_in_graveyard(&mut g);
        let mountain = g.put(0, land, Zone::Battlefield);
        cite!(["702.34a"]; ok: flashback(&mut g, dart, mountain, Target::Player(1)));
        // the Mountain was sacrificed as the cost was paid, before it resolves (601.2h)
        cite!(["601.2h"]; assert!(g.battlefield.iter().all(|o| *o != mountain)));
        assert!(g.players[0].graveyard.iter().any(|o| g.obj(*o).chars.name == g.obj(mountain).chars.name));
        resolve(&mut g);
        assert_eq!(g.players[1].life, 19);
        cite!(["702.34a"]; assert!(g.exile.iter().any(|o| g.obj(*o).chars.name == "Lava Dart"), "exiled, not put into the graveyard"));
        assert!(g.players[0].graveyard.iter().all(|o| g.obj(*o).chars.name != "Lava Dart"));
    }
}

#[test]
fn r601_2h_a_cost_that_cannot_be_paid_casts_nothing_and_changes_nothing() {
    let mut g = game();
    let dart = dart_in_graveyard(&mut g);
    let forest = g.put(0, FOREST, Zone::Battlefield);
    // no Mountain: not listed, and a Forest is not a Mountain
    let before = (g.players[0].graveyard.clone(), g.battlefield.clone(), g.stack.clone());
    cite!(["601.2h"]; refused("Forest is not what that cost asks for or cannot pay it (701.21a, 118.3)"): flashback(&mut g, dart, forest, Target::Player(1)));
    cite!(["601.2h"]; refused("that selection does not satisfy the cost, which asks for exactly 1 card(s): 0 chosen (601.2h, 701.21a)"): g.act(0, Move::CastVia(CastRoute::Flashback, Box::new(Move::Cast { card: dart, targets: vec![vec![Target::Player(1)]] }))));
    assert_eq!((g.players[0].graveyard.clone(), g.battlefield.clone(), g.stack.clone()), before, "rolled back");
    // an opponent's Mountain is not one you can sacrifice (701.21a)
    let theirs = g.put(1, MOUNTAIN, Zone::Battlefield);
    cite!(["701.21a"]; refused("Mountain is not what that cost asks for or cannot pay it (701.21a, 118.3)"): flashback(&mut g, dart, theirs, Target::Player(1)));
    assert!(g.battlefield.contains(&theirs));
}

#[test]
fn r601_2c_its_target_is_chosen_as_it_is_cast_from_the_graveyard_too() {
    let mut g = game();
    let dart = dart_in_graveyard(&mut g);
    let mountain = g.put(0, MOUNTAIN, Zone::Battlefield);
    // a land is not a legal target for "any target" (115.4): refused, nothing sacrificed
    let forest = g.put(1, FOREST, Zone::Battlefield);
    cite!(["601.2c", "115.1"]; refused("an illegal target, Forest: it isn't a creature or a planeswalker or a battle or a player (115.1)"): flashback(&mut g, dart, mountain, Target::Object(forest)));
    assert!(g.battlefield.contains(&mountain));
    let bears = g.put(1, BEARS, Zone::Battlefield);
    cite!(["601.2c"]; ok: flashback(&mut g, dart, mountain, Target::Object(bears)));
    resolve(&mut g);
    assert_eq!(g.obj(bears).damage, 1);
}

#[test]
fn r702_34a_countered_it_is_exiled_too() {
    let mut g = game();
    let dart = dart_in_graveyard(&mut g);
    let mountain = g.put(0, MOUNTAIN, Zone::Battlefield);
    flashback(&mut g, dart, mountain, Target::Player(1)).unwrap();
    let spell = *g.stack.last().unwrap();
    g.act(0, Move::Pass).unwrap();
    let deny = g.put(1, DENY, Zone::Hand);
    g.act(1, Move::Cast { card: deny, targets: vec![vec![Target::Object(spell)]] }).unwrap();
    resolve(&mut g);
    assert_eq!(g.players[1].life, 20);
    cite!(["702.34a"]; assert!(g.exile.iter().any(|o| g.obj(*o).chars.name == "Lava Dart")));
    assert!(g.players[0].graveyard.iter().all(|o| g.obj(*o).chars.name != "Lava Dart"));
}

#[test]
fn r118_9_a_cost_reduction_leaves_the_sacrifice_and_a_mixed_flashback_cost_is_paid_whole() {
    // "cost {1} less" reduces generic mana only (601.2f): nothing of "Sacrifice a Mountain"
    let mut g = game();
    g.put(0, DISCOUNT, Zone::Battlefield);
    let dart = dart_in_graveyard(&mut g);
    let mountain = g.put(0, MOUNTAIN, Zone::Battlefield);
    flashback(&mut g, dart, mountain, Target::Player(1)).unwrap();
    resolve(&mut g);
    assert_eq!(g.players[1].life, 19);
    // Deep Analysis: "Flashback—{1}{U}, Pay 3 life." both parts
    let mut g = game();
    let analysis = g.put(0, ANALYSIS, Zone::Graveyard);
    g.add_mana(0, ManaSymbol::Color(Color::Blue));
    cite!(["601.2h"]; refused("the supplied mana does not pay that cost, all of it and no more"): act_unchanged(&mut g,crate::support::paid("U",Move::CastVia(CastRoute::Flashback, Box::new(Move::Cast { card: analysis, targets: vec![vec![Target::Player(0)]] })))));
    g.add_mana(0, ManaSymbol::Colorless);
    let hand = g.players[0].hand.len();
    g.act(0, crate::support::paid("CU",Move::CastVia(CastRoute::Flashback, Box::new(Move::Cast { card: analysis, targets: vec![vec![Target::Player(0)]] })))).unwrap();
    cite!(["601.2h"]; assert_eq!(g.players[0].life, 17, "3 life paid"));
    resolve(&mut g);
    assert_eq!(g.players[0].hand.len(), hand + 2);
    cite!(["702.34a"]; assert!(g.exile.iter().any(|o| g.obj(*o).chars.name == "Deep Analysis")));
}

#[track_caller]
fn act_unchanged(g:&mut Game,action:Move)->Result<(),String> {
 let before=format!("{g:?}");let result=g.act(0,action);assert_eq!(format!("{g:?}"),before);result
}
