//! Granted ward (702.21a, 613.1f): whose opponents a granted ward watches, a grant
//! removed by an ability-removing effect in timestamp order (613.1f, 613.7), ward granted to a
//! noncreature permanent, ward costs that sacrifice a permanent of a kind a static ability
//! makes, and an Equipment moved with its ward trigger on the stack.
use crate::support::cite;
use game::card::CardDef;
use game::legal::Asked;
use game::play::check;
use game::state::Answer;
use game::{Game, Move, ObjId, Target, Zone};
use ir::mana::ManaSymbol;

const BEAR: usize = 0;
const SHOCK: usize = 1;
const BOOTS: usize = 2;
const STRENGTH: usize = 3;
const HARDLIGHT: usize = 4;
const YGRA: usize = 5;
const SUPPRESSOR: usize = 6;
const STRIP: usize = 7;
const THREATEN: usize = 8;
const TRINKET: usize = 9;
const DISENCHANT: usize = 10;
const MOUNTAIN: usize = 11;

fn defs() -> Vec<CardDef> {
    let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let catalog = CardDef::catalog(&faces);
    let real = |name: &str| CardDef::from_faces(catalog.get(name).unwrap()).unwrap();
    let card = |name, types, text, pt| CardDef::new(name, "{0}", types, text, pt).expect(name);
    vec![
        card("Bear", "Creature — Bear", "", Some((2, 2))),
        card("Shock", "Instant", "Shock deals 2 damage to any target.", None),
        real("Lavaspur Boots"),
        real("Super Strength"),
        real("Hardlight Containment"),
        real("Ygra, Eater of All"),
        card("Suppressor", "Artifact", "All creatures lose all abilities.", None),
        card("Strip", "Instant", "Target creature loses all abilities until end of turn.", None),
        card("Threaten", "Instant", "Gain control of target creature until end of turn.", None),
        card("Trinket", "Artifact", "", None),
        card("Disenchant", "Instant", "Destroy target artifact or enchantment.", None),
        CardDef::new("Mountain", "", "Basic Land — Mountain", "({T}: Add {R}.)", None).unwrap(),
    ]
}

fn game() -> Game {
    let mut g = Game::new(defs(), 2, 20);
    for p in 0..2 { for _ in 0..10 { g.put(p, MOUNTAIN, Zone::Library); } }
    g
}
fn give(g: &mut Game, by: usize) { while g.priority != by { g.act(g.priority, Move::Pass).unwrap(); } }
fn cast(g: &mut Game, by: usize, def: usize, target: Target) -> ObjId {
    give(g, by);
    let card = g.put(by, def, Zone::Hand);
    g.act(by, Move::Cast { card, targets: vec![vec![target]] }).unwrap();
    *g.stack.last().unwrap()
}
fn until_asked(g: &mut Game) { while g.decision.is_none() && !g.stack.is_empty() { g.act(g.priority, Move::Pass).unwrap(); } }
fn triggers(g: &Game) -> usize { g.stack.iter().filter(|o| g.obj(**o).ability.is_some()).count() }
fn colorless(g: &mut Game, player: usize, n: usize) { for _ in 0..n { g.add_mana(player, ManaSymbol::Colorless); } }

#[test]
fn r702_21a_the_reviewed_cards_are_admitted() {
    for d in defs() { check(&d).unwrap_or_else(|u| panic!("{}: {}", d.name(), u.0)); }
}

#[test]
fn r702_21a_an_aura_granting_ward_to_an_opponents_creature_watches_that_creatures_opponents() {
    // Super Strength cast by player 1 on player 0's Bear: the ward is the Bear's, so it is
    // player 0's opponents (player 1, the Aura's controller) whose spells it counters
    let mut g = game();
    let bear = g.put(0, BEAR, Zone::Battlefield);
    let aura = g.put(1, STRENGTH, Zone::Battlefield);
    assert!(g.attach(aura, Target::Object(bear)));
    cast(&mut g, 1, SHOCK, Target::Object(bear));
    cite!(["702.21a"]; case: "the Aura's controller targets"; assert_eq!(triggers(&g), 1));
    until_asked(&mut g);
    g.act(1, Move::Choose(Answer::No)).unwrap();
    until_asked(&mut g);
    cast(&mut g, 0, SHOCK, Target::Object(bear));
    cite!(["702.21a"]; case: "the creature's controller targets"; assert_eq!(triggers(&g), 0));
}

#[test]
fn r702_21a_a_granted_ward_follows_its_creatures_controller_after_a_control_change() {
    let mut g = game();
    let bear = g.put(0, BEAR, Zone::Battlefield);
    let boots = g.put(0, BOOTS, Zone::Battlefield);
    assert!(g.attach(boots, Target::Object(bear)));
    cast(&mut g, 1, THREATEN, Target::Object(bear));
    // Threaten itself is player 1's: ward triggers; player 1 pays {1}
    colorless(&mut g, 1, 1);
    until_asked(&mut g);
    assert!(matches!(g.asked(),Some((1,Asked::Pay{..}))));
    g.act(1, Move::Choose(Answer::Pay { plan: vec![], with: Some(crate::support::mana_pool("C")), tapped: vec![] })).unwrap();
    until_asked(&mut g);
    assert_eq!(g.controller(bear), 1, "Threaten resolved");
    // now player 0 is the creature's opponent; player 1 controls it
    cast(&mut g, 0, SHOCK, Target::Object(bear));
    cite!(["702.21a"]; case: "the former controller targets"; assert_eq!(triggers(&g), 1));
}

#[test]
fn r613_1f_losing_all_abilities_removes_a_granted_ward_in_timestamp_order() {
    // granted, then removed by a later effect: no ward
    let mut g = game();
    let bear = g.put(0, BEAR, Zone::Battlefield);
    let boots = g.put(0, BOOTS, Zone::Battlefield);
    assert!(g.attach(boots, Target::Object(bear)));
    g.put(1, SUPPRESSOR, Zone::Battlefield);
    cast(&mut g, 1, SHOCK, Target::Object(bear));
    cite!(["613.1f", "613.7"]; case: "static removal after the grant"; assert_eq!(triggers(&g), 0));
    // a resolved "loses all abilities" after the grant
    let mut g = game();
    let bear = g.put(0, BEAR, Zone::Battlefield);
    let boots = g.put(0, BOOTS, Zone::Battlefield);
    assert!(g.attach(boots, Target::Object(bear)));
    colorless(&mut g, 1, 1);
    cast(&mut g, 1, STRIP, Target::Object(bear));
    until_asked(&mut g);
    assert!(matches!(g.asked(),Some((1,Asked::Pay{..}))));
    g.act(1, Move::Choose(Answer::Pay { plan: vec![], with: Some(crate::support::mana_pool("C")), tapped: vec![] })).unwrap();
    until_asked(&mut g);
    cast(&mut g, 1, SHOCK, Target::Object(bear));
    cite!(["613.1f", "613.7"]; case: "resolved removal after the grant"; assert_eq!(triggers(&g), 0));
    // removed first, then granted by Equipment attached later (613.7e): it has ward
    let mut g = game();
    let bear = g.put(0, BEAR, Zone::Battlefield);
    g.put(1, SUPPRESSOR, Zone::Battlefield);
    let boots = g.put(0, BOOTS, Zone::Battlefield);
    assert!(g.attach(boots, Target::Object(bear)));
    cast(&mut g, 1, SHOCK, Target::Object(bear));
    cite!(["613.1f", "613.7e"]; case: "grant after the removal"; assert_eq!(triggers(&g), 1));
}

#[test]
fn r702_21a_ward_granted_to_an_artifact_by_hardlight_containment() {
    let mut g = game();
    let trinket = g.put(0, TRINKET, Zone::Battlefield);
    let aura = g.put(0, HARDLIGHT, Zone::Battlefield);
    assert!(g.attach(aura, Target::Object(trinket)));
    cast(&mut g, 1, DISENCHANT, Target::Object(trinket));
    cite!(["702.21a"]; assert_eq!(triggers(&g), 1, "the enchanted artifact has ward {{1}}"));
    until_asked(&mut g);
    cite!(["118.3"]; refused("that mana is not in your mana pool"): act_unchanged(&mut g,1,Move::Choose(Answer::Pay { plan: vec![], with: Some(crate::support::mana_pool("C")), tapped: vec![] })));
    g.act(1, Move::Choose(Answer::No)).unwrap();
    cite!(["702.21a", "118.3"]; assert!(g.stack.is_empty() && g.battlefield.contains(&trinket), "declined: countered"));
}

#[test]
fn r702_21a_ygra_ward_is_paid_by_sacrificing_a_creature_its_ability_makes_a_food() {
    let mut g = game();
    let ygra = g.put(0, YGRA, Zone::Battlefield);
    let bear = g.put(1, BEAR, Zone::Battlefield);
    let kept = g.put(1, BEAR, Zone::Battlefield);
    cast(&mut g, 1, SHOCK, Target::Object(ygra));
    let spell = *g.stack.iter().find(|o| g.obj(**o).ability.is_none()).unwrap();
    until_asked(&mut g);
    // "Other creatures are Food artifacts": player 1's Bear is a Food
    cite!(["702.21a", "205.3g"]; assert!(matches!(g.asked(), Some((1, Asked::May))), "{:?}", g.asked()));
    g.act(1, Move::Choose(Answer::Yes)).unwrap();
    assert!(matches!(g.asked(),Some((1,Asked::Objects{..}))),"{:?}",g.asked());
    g.act(1, Move::Choose(Answer::Objects(vec![bear]))).unwrap();
    cite!(["701.21a"]; assert!(!g.battlefield.contains(&bear) && g.battlefield.contains(&kept) && g.stack.contains(&spell)));
    // Ygra saw a Food put into a graveyard from the battlefield: two counters, and Shock's 2
    // damage doesn't kill a 8/8
    cite!(["603.2"]; assert_eq!(triggers(&g), 1));
    until_asked(&mut g);
    cite!(["603.2"]; assert_eq!(g.counters(Target::Object(ygra)).get("+1/+1").copied(), Some(2)));
}

#[test]
fn r113_7a_ward_triggered_by_equipment_still_counters_after_it_moves_to_another_creature() {
    let mut g = game();
    let bear = g.put(0, BEAR, Zone::Battlefield);
    let other = g.put(0, BEAR, Zone::Battlefield);
    let boots = g.put(0, BOOTS, Zone::Battlefield);
    assert!(g.attach(boots, Target::Object(bear)));
    cast(&mut g, 1, SHOCK, Target::Object(bear));
    assert_eq!(triggers(&g), 1);
    assert!(g.attach(boots, Target::Object(other)));
    until_asked(&mut g);
    g.act(1,Move::Choose(Answer::No)).unwrap();
    cite!(["113.7a", "702.21a"]; assert!(g.stack.is_empty() && g.battlefield.contains(&bear), "no mana: countered, {:?}", g.asked()));
}

#[test]
fn r613_1f_losing_ward_removes_a_granted_instance_and_its_trigger() {
    let text = "Target creature loses ward until end of turn.";
    let mut defs = defs();
    defs.push(CardDef::new("Unward", "{0}", "Instant", text, None).unwrap());
    // not yet admitted: losing one keyword that is a triggered ability is refused, so the
    // granted trigger can't outlive its keyword. Once admitted, the rest checks it does not.
    if let Err(why) = check(&defs[defs.len() - 1]) { assert_eq!(why.0, "keyword: Ward"); return; }
    let mut g = Game::new(defs, 2, 20);
    for p in 0..2 { for _ in 0..10 { g.put(p, MOUNTAIN, Zone::Library); } }
    let bear = g.put(0, BEAR, Zone::Battlefield);
    let boots = g.put(0, BOOTS, Zone::Battlefield);
    assert!(g.attach(boots, Target::Object(bear)));
    // cast by its controller: no ward trigger for its own spell
    cast(&mut g, 0, MOUNTAIN + 1, Target::Object(bear));
    until_asked(&mut g);
    cast(&mut g, 1, SHOCK, Target::Object(bear));
    cite!(["613.1f"]; assert_eq!(triggers(&g), 0, "the granted ward is lost, its trigger with it"));
}

#[track_caller]
fn act_unchanged(g:&mut Game,player:usize,action:Move)->Result<(),String>{
 let before=format!("{g:?}");let result=g.act(player,action);assert_eq!(format!("{g:?}"),before);result
}
