//! Finality is a counter-owned BF→graveyard replacement, not an ability (122.1h).
use crate::support::{cite, paid};
use game::{CardDef, Event, Game, Move, ObjId, Target, Zone};
use game::operation::ReplacementEffect;
use std::sync::OnceLock;

fn card(name: &str, types: &str, text: &str, pt: Option<(i32,i32)>) -> CardDef {
    let def=CardDef::new(name,"{0}",types,text,pt).unwrap();
    game::play::check(&def).unwrap_or_else(|e|panic!("{name}: {e:?}"));def
}
fn native(name: &str) -> CardDef {
    static FACES: OnceLock<Vec<oracle::corpus::Face>>=OnceLock::new();
    let faces=FACES.get_or_init(||oracle::corpus::load(&oracle::corpus::default_path()).unwrap());
    let def=CardDef::from_faces(CardDef::catalog(faces).get(name).unwrap()).unwrap();
    game::play::check(&def).unwrap_or_else(|e|panic!("{name}: {e:?}"));def
}
fn base(extra: Vec<CardDef>) -> Game {
    let mut defs=vec![card("Bear","Creature — Bear","",Some((2,2))),
        card("Destroy","Instant","Destroy target permanent.",None)];
    defs.extend(extra);Game::new(defs,2,20)
}
fn pass(g:&mut Game) { for _ in 0..2 { g.act(g.priority,Move::Pass).unwrap(); } }
fn cast(g:&mut Game,def:usize,target:ObjId) {
    let hand=g.put(0,def,Zone::Hand);
    g.act(0,Move::Cast{card:hand,targets:vec![vec![Target::Object(target)]]}).unwrap();pass(g);
}
fn finality(g:&mut Game,object:ObjId,count:u32) { g.put_counters(object,"finality",count); }

#[test]
fn r122_1h_zero_one_and_many_have_one_identical_replacement() {
    for count in [0,1,4] {
        let mut g=base(vec![]);let bear=g.put(0,0,Zone::Battlefield);finality(&mut g,bear,count);
        let from=g.log.end();cast(&mut g,1,bear);
        let destination=if count==0 {Zone::Graveyard}else{Zone::Exile};
        let members=if count==0 {&g.players[0].graveyard}else{&g.exile};
        let now=*members.iter().find(|id|g.obj(**id).card==Some(0)).unwrap();
        cite!(["122.1h", "122.2", "400.7"];assert_eq!(g.obj(now).zone,destination));
        assert!(g.counters(now).is_empty());
        assert_eq!(g.log.since(from).iter().filter(|e|matches!(e,Event::Exiled{object}if *object==bear)).count(),usize::from(count>0));
    }
}

#[test]
fn r122_1h_losing_all_abilities_does_not_disable_physical_counter_rule() {
    let mut g=base(vec![card("Silence","Instant","Target creature loses all abilities until end of turn.",None)]);
    let bear=g.put(0,0,Zone::Battlefield);finality(&mut g,bear,1);cast(&mut g,2,bear);cast(&mut g,1,bear);
    cite!(["122.1h", "613.1f"];assert_eq!(g.exile.len(),1));
    assert!(g.players[0].graveyard.iter().all(|id|g.obj(*id).card!=Some(0)));
}

#[test]
fn r122_1h_all_counter_removal_precedes_the_later_destroy() {
    let mut g=base(vec![card("Clear","Instant","Remove all counters from target permanent.",None)]);
    let bear=g.put(0,0,Zone::Battlefield);finality(&mut g,bear,3);cast(&mut g,2,bear);
    assert!(g.counters(bear).is_empty());cast(&mut g,1,bear);
    cite!(["122.1h", "122.1"];assert!(g.exile.is_empty()));
    assert!(g.players[0].graveyard.iter().any(|id|g.obj(*id).card==Some(0)));
}

#[test]
fn r122_1h_all_permanent_types_include_oracle_clause_noncreature_entry() {
    // Exact Yuna return clause; her separate static subject remains unsupported.
    let mut g=base(vec![card("Enchantment Return Fixture","Creature — Human Cleric","At the beginning of your end step, return up to one target enchantment card from your graveyard to the battlefield with a finality counter on it.",Some((3,5))),card("Relic","Enchantment","",None)]);
    let yuna=g.put(0,2,Zone::Battlefield);let relic=g.put(0,3,Zone::Graveyard);
    g.part=ir::Part::PostcombatMain;pass(&mut g);
    g.act(0,Move::ChooseTargets(vec![vec![Target::Object(relic)]])).unwrap();pass(&mut g);
    let relic=*g.battlefield.iter().find(|id|g.obj(**id).card==Some(3)).unwrap();
    cite!(["122.1h"];assert_eq!(g.counters(relic).get("finality"),Some(&1)));
    assert_eq!(g.obj(yuna).zone,Zone::Battlefield);cast(&mut g,1,relic);
    assert!(g.exile.iter().any(|id|g.obj(*id).card==Some(3)));
}

#[test]
fn r122_1h_bounce_and_reentry_create_counterless_object_that_dies_normally() {
    let mut g=base(vec![card("Bounce","Instant","Return target permanent to its owner's hand.",None)]);
    let bear=g.put(0,0,Zone::Battlefield);finality(&mut g,bear,2);cast(&mut g,2,bear);
    let hand=*g.players[0].hand.iter().find(|id|g.obj(**id).card==Some(0)).unwrap();
    assert!(g.counters(hand).is_empty());g.move_object(hand,Zone::Battlefield);
    let now=*g.battlefield.iter().find(|id|g.obj(**id).card==Some(0)).unwrap();
    cite!(["122.2", "400.7"];assert_ne!(bear,now));cast(&mut g,1,now);
    assert!(g.exile.is_empty());
    assert!(g.players[0].graveyard.iter().any(|id|g.obj(*id).card==Some(0)));
}

#[test]
fn r122_1h_counters_on_stack_or_graveyard_do_not_redirect() {
    let mut g=base(vec![card("Empty","Instant","You gain 1 life.",None)]);
    let hand=g.put(0,2,Zone::Hand);g.act(0,Move::Cast{card:hand,targets:vec![]}).unwrap();
    let spell=g.stack[0];finality(&mut g,spell,1);pass(&mut g);
    cite!(["122.1h"];assert!(g.exile.is_empty()));
    let grave=*g.players[0].graveyard.iter().find(|id|g.obj(**id).card==Some(2)).unwrap();finality(&mut g,grave,1);
    g.move_object(grave,Zone::Hand);assert_eq!(g.players[0].hand.len(),1);assert!(g.exile.is_empty());
}

#[test]
fn r122_1h_native_viper_enters_with_counter_then_sacrifice_is_not_death() {
    let mut g=base(vec![native("Soulcoil Viper"),
        card("Altar","Artifact","Sacrifice a creature: You gain 3 life.",None),
        card("Watcher","Enchantment","Whenever a creature dies, you gain 1 life.",None)]);
    let target=g.put(0,0,Zone::Graveyard);let viper=g.put(0,2,Zone::Battlefield);
    g.players[0].pool.colors[ir::Color::Black as usize]=1;
    g.act(0,paid("B",Move::Activate{source:viper,ability:0,targets:vec![vec![Target::Object(target)]],paying:vec![]})).unwrap();pass(&mut g);
    let bear=*g.battlefield.iter().find(|id|g.obj(**id).card==Some(0)).unwrap();
    assert_eq!(g.counters(bear).get("finality"),Some(&1));let altar=g.put(0,3,Zone::Battlefield);g.put(0,4,Zone::Battlefield);
    let from=g.log.end();
    g.act(0,Move::Activate{source:altar,ability:0,targets:vec![],paying:vec![vec![bear]]}).unwrap();
    cite!(["701.21a", "122.1h", "700.4"];assert_eq!(g.stack.len(),1));
    assert!(g.log.since(from).iter().any(|e|matches!(e,Event::Sacrificed{player:0,object}if *object==bear)));
    assert!(g.log.since(from).iter().any(|e|matches!(e,Event::Exiled{object}if *object==bear)));
    pass(&mut g);assert_eq!(g.players[0].life,23);
}

#[test]
fn r122_1h_simultaneous_sweep_only_counterless_creature_dies() {
    let mut g=base(vec![card("Sweep","Instant","Destroy all creatures.",None),
        card("Watcher","Enchantment","Whenever a creature dies, you gain 1 life.",None)]);
    g.put(0,3,Zone::Battlefield);let final_bear=g.put(0,0,Zone::Battlefield);g.put(0,0,Zone::Battlefield);finality(&mut g,final_bear,1);
    let hand=g.put(0,2,Zone::Hand);g.act(0,Move::Cast{card:hand,targets:vec![]}).unwrap();pass(&mut g);
    cite!(["122.1h", "700.4", "603.2c"];assert_eq!(g.stack.len(),1));
    assert_eq!(g.exile.len(),1);assert_eq!(g.players[0].graveyard.iter().filter(|id|g.obj(**id).card==Some(0)).count(),1);
    pass(&mut g);assert_eq!(g.players[0].life,21);
}

#[test]
fn r122_1h_indestructible_prevents_event_but_not_sacrifice() {
    let mut g=base(vec![card("Stone","Artifact","Indestructible",None)]);
    let stone=g.put(0,2,Zone::Battlefield);finality(&mut g,stone,1);cast(&mut g,1,stone);
    cite!(["702.12b", "614.7"];assert_eq!(g.obj(stone).zone,Zone::Battlefield));
    g.sacrifice(0,stone);cite!(["701.21a", "122.1h"];assert_eq!(g.exile.len(),1));
}

#[test]
fn r616_1_finality_and_own_library_redirect_are_distinct_and_frozen_for_clone() {
    let mut g=base(vec![card("Library Bear","Creature — Bear","If this creature would die, put it on the bottom of its owner's library instead.",Some((2,2)))]);
    let bear=g.put(0,2,Zone::Battlefield);finality(&mut g,bear,3);cast(&mut g,1,bear);
    let Some(game::state::Decision::Choose{player:0,choice:game::state::Choice::Replacement{among,..}})=&g.decision else {panic!("replacement choice")};
    cite!(["122.1h", "616.1"];assert_eq!(among.len(),2));
    let rule=among.iter().find(|id|id.effect==ReplacementEffect::Finality).unwrap().clone();
    let ability=among.iter().find(|id|id.ability().is_some()).unwrap().clone();
    assert_eq!(rule.source,bear);assert_eq!(g.obj(bear).zone,Zone::Battlefield);assert_eq!(g.counters(bear).get("finality"),Some(&3));
    let mut other=g.clone();g.act(0,Move::Choose(game::state::Answer::Replacement(rule))).unwrap();
    other.act(0,Move::Choose(game::state::Answer::Replacement(ability))).unwrap();
    cite!(["616.1f"];assert_eq!(g.exile.len(),1));assert!(g.players[0].library.is_empty());
    assert_eq!(other.players[0].library.len(),1);assert!(other.exile.is_empty());
}

#[test]
fn r616_1_two_exile_destinations_differ_when_oracle_clause_gains_life() {
    // Exact Darkness Crystal death replacement; its separate return entry is unsupported.
    let mut g=base(vec![card("Exile Rider Fixture","Legendary Artifact","If a nontoken creature an opponent controls would die, instead exile it and you gain 2 life.",None)]);g.put(0,2,Zone::Battlefield);
    let bear=g.put(1,0,Zone::Battlefield);finality(&mut g,bear,1);cast(&mut g,1,bear);
    let Some(game::state::Decision::Choose{player:1,choice:game::state::Choice::Replacement{among,..}})=&g.decision else {panic!("replacement choice by affected controller")};
    assert_eq!(among.len(),2);let rule=among.iter().find(|id|id.effect==ReplacementEffect::Finality).unwrap().clone();
    let crystal=among.iter().find(|id|id.ability().is_some()).unwrap().clone();
    let mut other=g.clone();g.act(1,Move::Choose(game::state::Answer::Replacement(rule))).unwrap();
    other.act(1,Move::Choose(game::state::Answer::Replacement(crystal))).unwrap();
    cite!(["616.1", "616.1f"];assert_eq!((g.players[0].life,other.players[0].life),(20,22)));
    assert_eq!(g.exile.len(),1);assert_eq!(other.exile.len(),1);
}

#[test]
fn r614_7_regeneration_prevents_the_graveyard_event_then_sacrifice_exiles() {
    let mut g=base(vec![]);let bear=g.put(0,0,Zone::Battlefield);finality(&mut g,bear,1);
    g.regenerate(bear);cast(&mut g,1,bear);
    cite!(["701.19a", "614.7", "122.1h"];assert_eq!(g.obj(bear).zone,Zone::Battlefield));
    assert!(g.obj(bear).tapped);assert_eq!(g.counters(bear).get("finality"),Some(&1));
    g.sacrifice(0,bear);assert_eq!(g.exile.len(),1);
}

#[test]
fn r101_4_simultaneous_departure_choices_precede_all_commits_in_apnap_order() {
    let mut g=Game::new(vec![card("Library Bear","Creature — Bear","If this creature would die, put it on the bottom of its owner's library instead.",Some((2,2))),
        card("Sweep","Instant","Destroy all creatures.",None)],3,20);
    g.active=1;g.priority=1;
    let bears=[g.put(0,0,Zone::Battlefield),g.put(1,0,Zone::Battlefield)];
    for bear in bears {finality(&mut g,bear,1);}
    let spell=g.put(1,1,Zone::Hand);g.act(1,Move::Cast{card:spell,targets:vec![]}).unwrap();
    for _ in 0..3 {g.act(g.priority,Move::Pass).unwrap();}
    for player in [1,0] {
        let Some(game::state::Decision::Choose{player:who,choice:game::state::Choice::Replacement{among,..}})=&g.decision else {panic!("replacement choice")};
        cite!(["101.4", "616.1"];assert_eq!(*who,player));assert_eq!(g.battlefield.len(),2);
        assert!(g.exile.is_empty());
        let chosen=among.iter().find(|id|if player==1 {id.effect==ReplacementEffect::Finality}else{id.ability().is_some()}).unwrap().clone();
        g.act(player,Move::Choose(game::state::Answer::Replacement(chosen))).unwrap();
    }
    assert_eq!(g.exile.len(),1);assert_eq!(g.obj(g.exile[0]).owner,1);
    assert_eq!(g.players[0].library.len(),1);assert!(g.battlefield.is_empty());
}
