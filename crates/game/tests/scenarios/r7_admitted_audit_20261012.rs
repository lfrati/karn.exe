//! A fixed selection of cards the engine runs; focused checks of clauses, not of whole cards.
use crate::support::{cite, oracle, paid};
use game::card::CastAs;
use game::legal::Asked;
use game::{CardDef, Game, Move, ObjId, Target, Zone};
use ir::Part;

fn setup(native: &str) -> Game {
    let defs = vec![oracle(native),
        CardDef::new("Forest", "", "Basic Land — Forest", "", None).unwrap(),
        CardDef::new("Bear", "{0}", "Creature — Bear", "", Some((2, 2))).unwrap(),
        CardDef::new("Gift", "{0}", "Instant", "Target player gains 1 life.", None).unwrap(),
        CardDef::new("Growth", "{0}", "Instant", "Target creature gets +1/+1 until end of turn.", None).unwrap(),
        CardDef::new("Counter", "{0}", "Instant", "Counter target spell.", None).unwrap()];
    let mut g = Game::new(defs, 2, 20);
    for p in 0..2 { for _ in 0..30 { g.put(p, 1, Zone::Library); } }
    g
}
fn cast(g: &mut Game, player: usize, def: usize, targets: Vec<Vec<Target>>) {
    while g.priority != player { g.act(g.priority, Move::Pass).unwrap(); }
    let card = g.put(player, def, Zone::Hand);
    g.act(player, Move::Cast { card, targets }).unwrap();
}
fn resolve(g: &mut Game) { for _ in 0..2 { g.act(g.priority, Move::Pass).unwrap(); } }
fn settle(g: &mut Game) { for _ in 0..12 { if g.stack.is_empty() { return; } resolve(g); } panic!("stack not settled"); }
fn progress(g: &mut Game) {
    let (p, asked) = g.asked().unwrap();
    let action = match asked { Asked::Priority => Move::Pass, Asked::Attackers(_) => Move::Attack(vec![]), other => panic!("unexpected {other:?}") };
    g.act(p, action).unwrap();
}
fn next_turn(g: &mut Game) { let turn = g.turn; for _ in 0..100 { if g.turn != turn { return; } progress(g); } panic!("turn did not advance"); }
fn treasure_ids(g: &Game) -> Vec<ObjId> { g.battlefield.iter().copied().filter(|&id| g.def(id).unwrap().name() == "Treasure Token").collect() }

#[test]
fn r700_13_magda_tapped_treasure_quota_is_per_turn_not_per_target_or_own_turn() {
    let mut g = setup("Magda, the Hoardmaster"); g.put(0, 0, Zone::Battlefield);
    cast(&mut g, 0, 3, vec![vec![Target::Player(1)]]);
    cite!(["700.13", "603.3"]; assert_eq!(g.stack.len(), 2));
    resolve(&mut g);
    let treasures = treasure_ids(&g);
    cite!(["111.10a"]; assert_eq!(treasures.len(), 1));
    assert!(g.obj(treasures[0]).token()); assert!(g.obj(treasures[0]).tapped);
    // The crime trigger resolves before the spell that targeted the opponent.
    assert_eq!(g.players[1].life, 20); settle(&mut g);
    cast(&mut g, 0, 3, vec![vec![Target::Player(1)]]);
    cite!(["603.2"]; assert_eq!(g.stack.len(), 1)); settle(&mut g);
    assert_eq!(treasure_ids(&g).len(), 1);
    next_turn(&mut g); assert_eq!(g.active, 1);
    cast(&mut g, 0, 3, vec![vec![Target::Player(1)]]); settle(&mut g);
    cite!(["603.2"]; assert_eq!(treasure_ids(&g).len(), 2));
}

#[test]
fn r118_magda_sacrifices_three_treasures_even_tapped_and_creates_exact_dragon() {

    let mut g = setup("Magda, the Hoardmaster"); let magda = g.put(0, 0, Zone::Battlefield);
    for turn in 0..3 {
        if turn > 0 { next_turn(&mut g); }
        cast(&mut g, 0, 3, vec![vec![Target::Player(1)]]); settle(&mut g);
    }
    while g.part != Part::PrecombatMain { progress(&mut g); }
    assert_eq!(g.active, 0);
    let treasures = treasure_ids(&g); assert_eq!(treasures.len(), 3);
    for &id in &treasures { g.tap(id); }
    let before = format!("{g:?}");
    cite!(["118.3"]; refused("that selection does not satisfy the cost, which asks for exactly 3 permanent(s): 2 chosen"): g.act(0, Move::Activate { source: magda, ability: 1, targets: vec![], paying: vec![treasures[..2].to_vec()] }));
    cite!(["118.3", "733.1"]; assert_eq!(format!("{g:?}"), before));
    // Three legal payment objects remain, but Magda's own sorcery restriction forbids combat.
    resolve(&mut g); assert_eq!(g.part,Part::BeginningOfCombat);
    cite!(["602.5d"]; refused("that ability is activated only as a sorcery: in a main phase of your own turn with the stack empty (602.5d)"): unchanged(&mut g,Move::Activate{source:magda,ability:1,targets:vec![],paying:vec![treasures.clone()]}));
    cite!(["602.5d"]; assert_eq!(treasure_ids(&g),treasures));
    while g.part != Part::PostcombatMain { progress(&mut g); }
    g.act(0, Move::Activate { source: magda, ability: 1, targets: vec![], paying: vec![treasures] }).unwrap();
    cite!(["602.2b", "118.11"]; assert!(treasure_ids(&g).is_empty()));
    settle(&mut g);
    let dragon = *g.battlefield.iter().find(|&&id| id != magda).unwrap();
    cite!(["111.3"]; assert_eq!((g.power(dragon), g.toughness(dragon)), (Some(4), Some(4))));
    assert!(g.obj(dragon).token()); assert!(g.has(dragon, "Flying")); assert!(g.has(dragon, "Haste"));
    assert!(g.seen(dragon, None).is(ir::CardType::Creature));
    assert_eq!(g.seen(dragon,None).colors,vec![ir::Color::Red]);
    assert_eq!(g.seen(dragon,None).subtypes.iter().map(|s|s.name()).collect::<Vec<_>>(),vec!["Scorpion","Dragon"]);
}

#[test]
fn r715_meager_meal_optional_creature_and_required_player_resolve_and_grant_only_normal_route() {

    let mut g = setup("Gollum, Silent Slinker // Meager Meal");
    let card = g.put(0, 0, Zone::Hand); g.players[0].pool.colors[2] = 1;
    g.act(0, paid("B",Move::CastAs(CastAs::Adventure, Box::new(Move::Cast { card, targets: vec![vec![], vec![Target::Player(0)]] })))).unwrap();
    cite!(["715.3b"]; assert_eq!(g.def(g.stack[0]).unwrap().name(), "Meager Meal"));
    assert_eq!(g.obj(g.stack[0]).chars.mana_value, 1);
    settle(&mut g); assert_eq!(g.players[0].life, 22);
    let exiled = g.exile[0];
    cite!(["715.3d"]; assert_eq!(g.permission(exiled), Some((0, true))));
    assert_eq!(g.obj(exiled).chars.mana_value, 4);
    g.players[0].pool.colors[2] = 1; g.players[0].pool.colorless = 3;
    cite!(["715.3d"]; refused("it was exiled on an adventure: it may be cast as its creature or other main card, not as its Adventure (715.3d)"): unchanged(&mut g,paid("B",Move::CastAs(CastAs::Adventure,Box::new(Move::Cast {card:exiled,targets:vec![vec![],vec![Target::Player(0)]]})))));
    g.act(0, paid("CCCB",Move::Cast { card: exiled, targets: vec![] })).unwrap(); settle(&mut g);
    let permanent = g.battlefield[0];
    cite!(["715.4"]; assert_eq!((g.power(permanent),g.toughness(permanent)),(Some(4),Some(3))));
    assert!(g.has(permanent,"Menace"));
}

#[test]
fn r608_2b_meager_meal_lost_creature_target_does_not_fizzle_legal_player_or_affect_successor() {
    let mut g = setup("Gollum, Silent Slinker // Meager Meal"); let bear = g.put(0, 2, Zone::Battlefield);
    let card = g.put(0, 0, Zone::Hand); g.players[0].pool.colors[2] = 1;
    g.act(0, Move::CastAs(CastAs::Adventure, Box::new(paid("B", Move::Cast {card, targets:vec![vec![Target::Object(bear)],vec![Target::Player(0)]]})))).unwrap();
    g.move_object(bear,Zone::Hand); let hand = *g.players[0].hand.last().unwrap();
    g.move_object(hand,Zone::Battlefield); let successor = *g.battlefield.last().unwrap();
    assert_ne!(successor,bear); settle(&mut g);
    cite!(["608.2b", "400.7"]; assert!(g.obj(successor).counters.is_empty()));
    assert_eq!(g.players[0].life,22);
    cite!(["715.3d"]; assert_eq!(g.permission(g.exile[0]),Some((0,true))));
}

#[test]
fn r603_snooping_page_repartee_precedes_spell_and_survives_target_departure_and_counterspell() {
    let mut g = setup("Snooping Page"); let page = g.put(0,0,Zone::Battlefield);
    let bear = g.put(1,2,Zone::Battlefield); let blocker = g.put(1,2,Zone::Battlefield);
    cast(&mut g,0,4,vec![vec![Target::Object(bear)]]);
    cite!(["603.2", "603.3"]; assert_eq!(g.stack.len(),2));
    let spell = g.stack[0];
    g.move_object(bear,Zone::Graveyard);
    cast(&mut g,1,5,vec![vec![Target::Object(spell)]]); resolve(&mut g);
    settle(&mut g);
    cite!(["113.7a", "115.9b"]; assert!(g.cant_block(blocker,page,&[blocker])));
    next_turn(&mut g);
    cite!(["611.2a"]; assert!(!g.cant_block(blocker,page,&[blocker])));
}

#[test]
fn r120_snooping_page_noncombat_damage_is_not_combat_damage_trigger() {
    let mut g = setup("Snooping Page"); let page = g.put(0,0,Zone::Battlefield);
    let hand = g.players[0].hand.len(); g.damage(page,Target::Player(1),2); settle(&mut g);
    cite!(["120.2", "603.2"]; assert_eq!((g.players[0].hand.len(),g.players[0].life,g.players[1].life),(hand,20,18)));
}

#[test]
fn r510_snooping_page_actual_combat_damage_draws_once_and_loses_one_life() {
    let mut g = setup("Snooping Page"); let page = g.put(0,0,Zone::Battlefield);
    resolve(&mut g); resolve(&mut g);
    assert_eq!(g.part,Part::DeclareAttackers);
    g.act(0,Move::Attack(vec![(page,Target::Player(1))])).unwrap(); resolve(&mut g);
    assert_eq!(g.part,Part::DeclareBlockers);
    if matches!(g.asked(),Some((1,Asked::Blockers { .. }))) { g.act(1,Move::Block(vec![])).unwrap(); }
    resolve(&mut g);
    cite!(["510.2", "603.2"]; assert_eq!((g.players[0].hand.len(),g.players[0].life,g.players[1].life),(0,20,18)));
    assert_eq!(g.stack.len(),1); settle(&mut g);
    cite!(["121.1", "119.3"]; assert_eq!((g.players[0].hand.len(),g.players[0].life),(1,19)));
}

#[test]
fn r122_meager_meal_places_counter_and_player_gains_life() {
    let mut g = setup("Gollum, Silent Slinker // Meager Meal"); let bear = g.put(0,2,Zone::Battlefield);
    let card = g.put(0,0,Zone::Hand); g.players[0].pool.colors[2]=1;
    g.act(0,Move::CastAs(CastAs::Adventure,Box::new(paid("B", Move::Cast{card,targets:vec![vec![Target::Object(bear)],vec![Target::Player(1)]]})))).unwrap();
    settle(&mut g);
    cite!(["122.1a", "608.2c"]; assert_eq!(g.obj(bear).counters.get("+1/+1"),Some(&1)));
    assert_eq!(g.power(bear),Some(3)); assert_eq!(g.players[1].life,22);
    assert_eq!(g.permission(g.exile[0]),Some((0,true)));
}

#[test]
fn r715_countered_native_adventure_and_arbitrary_exile_have_no_adventure_permission() {
    let mut g = setup("Gollum, Silent Slinker // Meager Meal");
    let card = g.put(0,0,Zone::Hand); g.players[0].pool.colors[2]=1;
    g.act(0,Move::CastAs(CastAs::Adventure,Box::new(paid("B", Move::Cast{card,targets:vec![vec![],vec![Target::Player(0)]]})))).unwrap();
    let spell = g.stack[0]; cast(&mut g,1,5,vec![vec![Target::Object(spell)]]); settle(&mut g);
    cite!(["715.3d", "701.6a"]; assert!(g.exile.is_empty()));
    assert_eq!(g.players[0].life,20);
    let grave = g.players[0].graveyard[0]; g.move_object(grave,Zone::Exile);
    cite!(["715.3d", "400.7"]; assert_eq!(g.permission(g.exile[0]),None));
}

/// CR715.3c–d and Gollum ruling b9dbbcdc95f6a0c5: the copied Adventure is
/// exiled on successful resolution, then ceases to exist (704.5e). Its effect
/// still happens; it grants no lasting physical-card permission.
#[test]
fn r715_3c_native_meager_meal_copy_resolves_to_exile_not_graveyard() {
    let mut g = setup("Gollum, Silent Slinker // Meager Meal");
    let mirror = g.defs.len();
    std::sync::Arc::make_mut(&mut g.defs).push(
        CardDef::new("Mirror", "{0}", "Instant", "Copy target instant or sorcery spell.", None).unwrap());
    let card = g.put(0, 0, Zone::Hand);
    g.players[0].pool.colors[2] = 1;
    g.act(0, Move::CastAs(CastAs::Adventure, Box::new(paid("B", Move::Cast {
        card, targets: vec![vec![], vec![Target::Player(0)]],
    })))).unwrap();
    let original = g.stack[0];
    cast(&mut g, 0, mirror, vec![vec![Target::Object(original)]]);
    resolve(&mut g);
    let copy = *g.stack.last().unwrap();
    assert_ne!(copy, original);
    assert_eq!(g.def(copy).unwrap().name(), "Meager Meal");
    assert_eq!(g.obj(copy).chars.mana_value, 1);
    let from = g.log.end();
    resolve(&mut g);
    cite!(["715.3c", "715.3d"]; assert_eq!(g.players[0].life, 22));
    let destinations: Vec<_> = g.log.since(from).iter().filter_map(|e| match e {
        game::Event::ZoneChanged { was, from: Zone::Stack, to, .. } if *was == copy => Some(*to),
        _ => None,
    }).collect();
    cite!(["715.3c", "715.3d"]; assert_eq!(destinations, vec![Zone::Exile]));
    cite!(["701.13a", "715.3c", "715.3d"]; assert_eq!(g.log.since(from).iter().filter(|e|
        matches!(e, game::Event::Exiled { object } if *object == copy)).count(), 1));
    assert!(!g.log.since(from).iter().any(|e|
        matches!(e, game::Event::ZoneChanged { was, to: Zone::Graveyard, .. } if *was == copy)));
    cite!(["704.5e"]; assert!(g.exile.is_empty()));
    assert_eq!(g.permission(copy), None);
    assert_eq!(g.stack, vec![original]);
    assert_eq!(g.obj(original).zone, Zone::Stack);
    let from = g.log.end();
    resolve(&mut g);
    cite!(["701.13a", "715.3d"]; assert_eq!(g.log.since(from).iter().filter(|e|
        matches!(e, game::Event::Exiled { object } if *object == original)).count(), 1));
    assert_eq!(g.players[0].life, 24);
    assert_eq!(g.exile.len(), 1);
    assert_eq!(g.permission(g.exile[0]), Some((0, true)));
}

#[track_caller]
fn unchanged(g: &mut Game, action: Move) -> Result<(), String> {
    let before=format!("{g:?}"); let result=g.act(0,action);
    assert_eq!(format!("{g:?}"),before); result
}
