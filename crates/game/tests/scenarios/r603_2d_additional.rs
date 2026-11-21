//! Additional ordinary trigger occurrences share identity, but choose/resolve separately (603.2d).
use crate::support::cite;
use game::{CardDef, Game, Move, ObjId, Target, Zone};
use game::state::{Answer, Choice, Decision};

const DELNEY: &str = "Creatures you control with power 2 or less can't be blocked by creatures with power 3 or greater.\nIf a triggered ability of a creature you control with power 2 or less triggers, that ability triggers an additional time.";
fn card(name: &str, kind: &str, text: &str, pt: Option<(i32,i32)>) -> CardDef { CardDef::new(name,"{0}",kind,text,pt).unwrap_or_else(|e|panic!("{name}: {text}: {e}")) }
fn setup(text: &str, copies: usize) -> (Game, ObjId) {
    let defs=vec![card("Delney, Streetwise Lookout",if copies>1 {"Creature — Human Scout"} else {"Legendary Creature — Human Scout"},DELNEY,Some((2,2))),
        card("Watcher","Creature",text,Some((2,3))),card("Spell","Instant","You gain 1 life.",None),
        card("Forest","Basic Land — Forest","",None),card("Growth","Instant","Target creature gets +1/+1 until end of turn.",None),
        card("Strip","Instant","Target creature loses all abilities until end of turn.",None),
        card("Steal","Instant","You gain control of target creature.",None),
        card("Keep","Enchantment","",None)];
    for def in &defs { game::play::check(def).unwrap_or_else(|e| panic!("{}: {e:?}",def.name())); }
    let mut g=Game::new(defs,3,20);g.put(0,7,Zone::Battlefield);
    for _ in 0..copies { g.put(0,0,Zone::Battlefield); }
    let watcher=g.put(0,1,Zone::Battlefield);
    for p in 0..3 {for _ in 0..20 {g.put(p,3,Zone::Library);}}
    (g,watcher)
}
fn order(g: &mut Game) {
    if let Some(Decision::Order{player,ref abilities})=g.decision {g.act(player,Move::Order((0..abilities.len()).collect())).unwrap();}
}
fn pass(g: &mut Game) { for _ in 0..3 {g.act(g.priority,Move::Pass).unwrap();} }
fn cast(g: &mut Game, def: usize, target: Option<ObjId>) {
    while g.priority!=0 {g.act(g.priority,Move::Pass).unwrap();}
    let card=g.put(0,def,Zone::Hand);
    g.act(0,Move::Cast{card,targets:target.map(|o|vec![vec![Target::Object(o)]]).unwrap_or_default()}).unwrap();
}
#[test]
fn r603_2d_overlapping_instances_add_and_do_not_recurse() {
    for n in 0..4 {
        let (mut g,_)=setup("Whenever you cast a spell, you gain 1 life.",n);
        cast(&mut g,2,None);order(&mut g);cite!(["603.2d"]; case: format!("{n} modifiers"); assert_eq!(g.stack.len(),n+2));
        while !g.stack.is_empty(){pass(&mut g);}
        assert_eq!(g.players[0].life,22+n as i32);
    }
}
#[test]
fn r603_2d_trigger_caps_and_action_caps_consume_distinct_moments() {
    let (mut g,_)=setup("Whenever you cast a spell, you gain 1 life. This ability triggers only once each turn.",2);
    // Uncited (card text): "This ability triggers only once each turn" caps the additional occurrences (603.2d) too, and the second cast that turn adds none; no rule or ruling covers the combination.
    cast(&mut g,2,None);assert_eq!(g.stack.len(),2);cast(&mut g,2,None);assert_eq!(g.stack.len(),3);
    let (mut g,_)=setup("Whenever you cast a spell, you may pay {0}. If you do, you gain 1 life. Do this only once each turn.",1);
    cast(&mut g,2,None);order(&mut g);cite!(["603.2d", "603.2h"]; assert_eq!(g.stack.len(),3));pass(&mut g);
    assert!(matches!(g.decision,Some(Decision::Choose{choice:Choice::Pay{..},..})));
    g.act(0,Move::Choose(Answer::No)).unwrap();pass(&mut g);
    let mut clone=g.clone();
    for game in [&mut g,&mut clone] {
        game.act(0,Move::Choose(Answer::Pay { plan: vec![], with: None, tapped: vec![] })).unwrap();assert_eq!(game.players[0].life,21);
        pass(game);cast(game,2,None);assert_eq!(game.stack.len(),1);
    }
    let (mut g,_)=setup("Whenever you cast a spell, you may pay {0}. If you do, you gain 1 life. Do this only once each turn.",1);
    cast(&mut g,2,None);order(&mut g);pass(&mut g);g.act(0,Move::Choose(Answer::Pay { plan: vec![], with: None, tapped: vec![] })).unwrap();
    // Uncited (card text): "Do this only once each turn" limits the action itself, so the other pending occurrence cannot take it; 603.2h decides only triggering.
    pass(&mut g);assert!(g.decision.is_none(),"other pending occurrence rechecks completed action");assert_eq!(g.players[0].life,21);
}
#[test]
fn r603_2d_capture_uses_power_and_modifier_membership_without_rechecking_pending_occurrences() {
    let (mut g,watcher)=setup("Whenever you cast a spell, you gain 1 life.",1);
    cast(&mut g,4,Some(watcher));order(&mut g);cite!(["603.2d"]; assert_eq!(g.stack.len(),3));
    while !g.stack.is_empty(){pass(&mut g);}assert_eq!(g.power(watcher),Some(3));
    cast(&mut g,2,None);cite!(["603.2d"]; assert_eq!(g.stack.len(),2));
    let (mut g,_)=setup("Whenever you cast a spell, you gain 1 life.",1);
    let delney=g.battlefield[1];cast(&mut g,5,Some(delney));order(&mut g);
    while !g.stack.is_empty(){pass(&mut g);}
    cast(&mut g,2,None);cite!(["603.2d"]; assert_eq!(g.stack.len(),2));
}
#[test]
fn r603_2d_unsupported_event_causes_and_mixed_statements_are_refused() {
    for text in ["If a creature dying causes a triggered ability of a permanent you control to trigger, it triggers an additional time.",
        "If a triggered ability of a creature you control triggers, that ability triggers an additional time. You gain 1 life."] {
        let def=card("Refused","Enchantment",text,None);
        let reason=if text.contains("dying causes") {"additional triggers: event-qualified cause"} else {"additional triggers: mixed statement"};
        crate::support::refused(game::play::check(&def).map_err(|e|e.0),reason);
    }
}
fn extra(g: &mut Game, def: CardDef) -> usize {
    game::play::check(&def).unwrap_or_else(|e|panic!("{}: {e:?}",def.name()));
    let defs=std::sync::Arc::make_mut(&mut g.defs);let index=defs.len();defs.push(def);index
}
#[test]
fn r603_2d_each_prowess_and_given_trigger_keeps_its_own_ability_identity() {
    let (mut g,_)=setup("Prowess, prowess",1);cast(&mut g,2,None);
    let Some(Decision::Order{ref abilities,..})=g.decision else {panic!("{:?}",g.decision)};
    cite!(["603.2d"]; assert_eq!(abilities.len(),4));order(&mut g);assert_eq!(g.stack.len(),5);
    while !g.stack.is_empty(){pass(&mut g);}
    assert_eq!(g.power(g.battlefield[2]),Some(6));
    let (mut g,watcher)=setup("",1);
    let grant=extra(&mut g,card("Grant","Instant","Until end of turn, target creature gains \"Whenever you cast a spell, you gain 1 life.\"",None));
    cast(&mut g,grant,Some(watcher));pass(&mut g);cast(&mut g,grant,Some(watcher));order(&mut g);
    while !g.stack.is_empty(){pass(&mut g);}
    cast(&mut g,2,None);order(&mut g);cite!(["603.2d"]; assert_eq!(g.stack.len(),5,"two separate given instances each twice"));
}
#[test]
fn r603_2d_death_uses_the_whole_pre_event_world_including_departed_modifier() {
    let (mut g,watcher)=setup("When this creature dies, you gain 1 life.",1);
    let wipe=extra(&mut g,card("Wipe","Sorcery","Destroy all creatures.",None));
    let delney = g.battlefield[1];
    cast(&mut g,wipe,None);pass(&mut g);
    // CR404.3: the owner explicitly orders these simultaneous graveyard arrivals.
    assert_eq!(g.decision, Some(game::state::Decision::Choose { player: 0, choice: game::state::Choice::OrderCards { to: game::state::Placement::Zone(Zone::Graveyard), cards: vec![delney, watcher] } }));
    g.act(0, Move::Choose(game::state::Answer::Objects(vec![watcher, delney]))).unwrap();
    order(&mut g);
    assert_eq!(g.obj(watcher).zone,Zone::Gone);cite!(["603.2d", "603.10a"]; assert_eq!(g.stack.len(),2));
    while !g.stack.is_empty(){pass(&mut g);}assert_eq!(g.players[0].life,22);
}
#[test]
fn r603_2d_effective_controller_determines_eligibility() {
    let (mut g,watcher)=setup("Whenever a player casts a spell, you gain 1 life.",1);
    while g.priority!=1 {g.act(g.priority,Move::Pass).unwrap();}
    let steal=g.put(1,6,Zone::Hand);g.act(1,Move::Cast{card:steal,targets:vec![vec![Target::Object(watcher)]]}).unwrap();order(&mut g);
    while !g.stack.is_empty(){pass(&mut g);}
    assert_eq!(g.controller(watcher),1);cast(&mut g,2,None);cite!(["603.2d"]; assert_eq!(g.stack.len(),2));
    cite!(["603.3a"]; assert_eq!(g.controller(*g.stack.last().unwrap()),1));
}
#[test]
fn r603_2d_delayed_abilities_created_by_a_creature_are_not_multiplied() {
    let (mut g,watcher)=setup("{0}: Exile target creature. Return it to the battlefield under its owner's control at the beginning of the next end step.",1);
    let small=extra(&mut g,card("Small","Creature","",Some((1,1))));let target=g.put(0,small,Zone::Battlefield);
    g.act(0,Move::Activate{source:watcher,ability:0,targets:vec![vec![Target::Object(target)]],paying:vec![]}).unwrap();pass(&mut g);
    for _ in 0..80 {
        if !g.stack.is_empty(){break;}
        match g.decision {Some(Decision::Attackers{player})=>g.act(player,Move::Attack(vec![])).unwrap(),_=>g.act(g.priority,Move::Pass).unwrap()};
    }
    cite!(["603.2d"]; assert_eq!(g.stack.len(),1));pass(&mut g);assert!(g.exile.is_empty());
    let reflexive=card("Refused","Creature","{0}: Surveil 2. When you do, you gain 1 life.",Some((2,2)));
    // Uncited (admission): surveil is not a retained reflexive producer.
    crate::support::refused(game::play::check(&reflexive).map_err(|e|e.0), "reflexive: no earlier payment producer");
}
#[test]
fn r603_2d_grouped_life_loss_propagates_all_amounts_to_every_occurrence() {
    let (mut g,_)=setup("Whenever one or more opponents lose life, you gain that much life.",1);
    let loss=extra(&mut g,card("Loss","Sorcery","Each opponent loses 2 life.",None));
    cast(&mut g,loss,None);pass(&mut g);order(&mut g);
    cite!(["603.2d"]; assert_eq!(g.stack.len(),2));pass(&mut g);pass(&mut g);cite!(["603.2d"]; assert_eq!(g.players[0].life,28));
}
#[test]
fn r603_2d_step_and_state_triggers_get_additional_occurrences_without_retriggering_while_pending() {
    let (mut g,_)=setup("At the beginning of your end step, you gain 1 life.",1);
    for _ in 0..80 {
        if !g.stack.is_empty() || matches!(g.decision,Some(Decision::Order{..})){break;}
        match g.decision {Some(Decision::Attackers{player})=>g.act(player,Move::Attack(vec![])).unwrap(),_=>g.act(g.priority,Move::Pass).unwrap()};
    }
    order(&mut g);cite!(["603.2d"]; assert_eq!(g.stack.len(),2));pass(&mut g);pass(&mut g);assert_eq!(g.players[0].life,22);
    let (mut g,watcher)=setup("When you control no Islands, sacrifice this creature.",1);
    g.act(0,Move::Pass).unwrap();order(&mut g);cite!(["603.2d", "603.8"]; assert_eq!(g.stack.len(),2));
    pass(&mut g);assert_eq!(g.obj(watcher).zone,Zone::Gone);assert_eq!(g.stack.len(),1);
    pass(&mut g);assert!(g.stack.is_empty());
}
#[test]
fn r603_2d_granted_modifier_instances_apply_only_while_effectively_present() {
    let (mut g,watcher)=setup("Whenever you cast a spell, you gain 1 life.",0);
    let grant=extra(&mut g,card("Grant modifier","Instant","Until end of turn, target creature gains \"If a triggered ability of a creature you control with power 2 or less triggers, that ability triggers an additional time.\"",None));
    for _ in 0..2 {cast(&mut g,grant,Some(watcher));order(&mut g);while !g.stack.is_empty(){pass(&mut g);}}
    cast(&mut g,2,None);order(&mut g);cite!(["603.2d"]; assert_eq!(g.stack.len(),4,"both granted modifier instances contribute once"));
    while !g.stack.is_empty(){pass(&mut g);}
    cast(&mut g,5,Some(watcher));order(&mut g);while !g.stack.is_empty(){pass(&mut g);}
    cast(&mut g,2,None);assert_eq!(g.stack.len(),1,"removed modifier and ordinary abilities no longer function");
}
#[test]
fn r603_2d_entry_eligibility_uses_power_after_entry_replacements() {
    for counters in [false,true] {
        let text=if counters {"When this creature enters, you gain 1 life.\nThis creature enters with a +1/+1 counter on it."} else {"When this creature enters, you gain 1 life."};
        let (mut g,_)=setup(text,1);cast(&mut g,1,None);pass(&mut g);order(&mut g);
        cite!(["603.2d", "603.10"]; case: format!("counters {counters}"); assert_eq!(g.stack.len(),if counters{1}else{2}));
    }
}
#[test]
fn r603_2d_turn_reset_and_source_incarnation_preserve_cap_identity() {
    let (mut g,watcher)=setup("Whenever you cast a spell, you gain 1 life. This ability triggers only once each turn.",1);
    cast(&mut g,2,None);while !g.stack.is_empty(){pass(&mut g);}
    g.move_object(watcher,Zone::Hand);let next=*g.players[0].hand.last().unwrap();g.move_object(next,Zone::Battlefield);
    cast(&mut g,2,None);cite!(["400.7"]; assert_eq!(g.stack.len(),2,"new incarnation has its own one-occurrence cap"));while !g.stack.is_empty(){pass(&mut g);}
    for _ in 0..100 {
        if g.active==1 {break;}
        match g.decision {Some(Decision::Attackers{player})=>g.act(player,Move::Attack(vec![])).unwrap(),_=>g.act(g.priority,Move::Pass).unwrap()};
    }
    assert_eq!(g.active,1);cast(&mut g,2,None);assert_eq!(g.stack.len(),2,"next player's turn resets the cap without multiplication");
}
