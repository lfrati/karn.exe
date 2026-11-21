//! Initial states are settled fixture premises, not synthetic entry or turn events.
use crate::support::cite;
use game::{CardDef, Game, Zone};
use ir::Part;

fn initial_advance(g:&mut Game, done:impl Fn(&Game)->bool) {
    for _ in 0..100 {
        if done(g) {return;}
        match g.decision.clone() {
            Some(game::state::Decision::Attackers{player})=>g.act(player,game::Move::Attack(vec![])).unwrap(),
            Some(game::state::Decision::Blockers{player})=>g.act(player,game::Move::Block(vec![])).unwrap(),
            None=>g.act(g.priority,game::Move::Pass).unwrap(),
            other=>panic!("unexpected decision {other:?}"),
        }
    }
    panic!("initial scenario did not reach expected boundary")
}

#[test]
fn r611_2_initial_growth_is_a_duration_bearing_cause_not_printed_power() {
    use serde_json::json;
    let growth=CardDef::new("Growth","{G}","Instant","Target creature gets +3/+3 until end of turn.",None).unwrap();
    let bear=CardDef::new("Bear","{2}","Creature — Bear","",Some((2,2))).unwrap();
    let mut g=Game::new(vec![growth,bear],2,20);
    let subject=g.put(0,1,Zone::Battlefield);
    let physical=g.put(0,0,Zone::Graveyard);
    let source=g.initial_continuous_source(physical);
    g.settle_initial_position(3,Part::PostcombatMain,0,0,0,&[(subject,0,false)]).unwrap();
    let changes=json!([{"add_pt":{"power":3,"toughness":3}}]);
    g.set_initial_continuous(source,1,0,vec![subject],&changes,&json!({"kind":"end_of_turn","turn":3})).unwrap();
    g.verify_initial_continuous_inventory().unwrap();
    cite!(["611.2a","613.4c"]; assert_eq!(g.audit_game_objects()["objects"][subject.0.to_string()]["properties"]["power"],5));
    assert_eq!(g.audit_game_objects()["continuous_effects"][0]["changes"],changes);
    g.exile(physical);g.collect_departed();
    cite!(["113.7a","611.2a"]; assert!(g.get_object(source).is_some()));
    initial_advance(&mut g,|g|g.turn==4);
    cite!(["514.2"]; assert_eq!(g.audit_game_objects()["objects"][subject.0.to_string()]["properties"]["power"],2));
    assert!(g.audit_game_objects()["continuous_effects"].as_array().unwrap().is_empty());
}

#[test]
fn r603_7_initial_delayed_exile_result_survives_source_departure_and_never_rebinds() {
    let blink=CardDef::new("Delayed blink","{1}{W}","Instant",
        "Exile target creature. At the beginning of the next end step, return that card to the battlefield under its owner's control with a +1/+1 counter on it.",None).unwrap();
    let bear=CardDef::new("Bear","{2}","Creature — Bear","",Some((2,2))).unwrap();
    for leave_exile in [false,true] {
        let mut g=Game::new(vec![blink.clone(),bear.clone()],2,20);
        let exiled=g.put(1,1,Zone::Exile);
        let physical=g.put(0,0,Zone::Graveyard);
        let source=g.initial_continuous_source(physical);
        g.settle_initial_position(3,Part::PostcombatMain,0,0,0,&[]).unwrap();
        g.set_initial_delayed_exile(source,1,0,1,&[exiled]).unwrap();
        let audit=g.audit_game_objects();
        assert_eq!(audit["delayed_triggers"][0]["producer"]["source"],source.0);
        assert_eq!(audit["delayed_triggers"][0]["retained"][0]["objects"][0],exiled.0);
        assert!(g.stack.is_empty());
        g.exile(physical);
        if leave_exile {g.move_object(exiled,Zone::Graveyard);let grave=g.players[1].graveyard[0];g.exile(grave);}
        g.collect_departed();
        cite!(["603.7d","113.7a"]; assert!(g.get_object(source).is_some()));
        initial_advance(&mut g,|g|g.part==Part::End && !g.stack.is_empty());
        g.act(0,game::Move::Pass).unwrap();g.act(1,game::Move::Pass).unwrap();
        if leave_exile {
            cite!(["603.7c","400.7"]; assert!(g.battlefield.is_empty()));
        } else {
            assert_eq!(g.battlefield.len(),1);let returned=g.battlefield[0];
            cite!(["400.7j","603.7c"]; assert_ne!(returned,exiled));
            assert_eq!(g.get_object(returned).unwrap().controller,1);
            assert_eq!(g.counters(returned).get("+1/+1"),Some(&1));
        }
        assert!(g.audit_game_objects()["delayed_triggers"].as_array().unwrap().is_empty());
    }
}

#[test]
fn r603_7a_direct_end_step_premise_does_not_repeat_the_step_beginning() {
    let blink=CardDef::new("Delayed blink","{0}","Instant",
        "Exile target creature. Return that card to the battlefield under its owner's control at the beginning of the next end step.",None).unwrap();
    let bear=CardDef::new("Bear","{2}","Creature","",Some((2,2))).unwrap();
    let mut g=Game::new(vec![blink,bear],2,20);
    let exiled=g.put(0,1,Zone::Exile);g.put(1,1,Zone::Library);
    let card=g.put(0,0,Zone::Graveyard);let source=g.initial_continuous_source(card);
    g.settle_initial_position(3,Part::End,0,0,0,&[]).unwrap();
    g.set_initial_delayed_exile(source,1,0,1,&[exiled]).unwrap();
    cite!(["603.7a"]; assert!(g.stack.is_empty()));
    initial_advance(&mut g,|g|g.turn==4 && g.part==Part::End && !g.stack.is_empty());
    cite!(["603.7a","603.7b"]; assert_eq!(g.turn,4));
    assert!(g.battlefield.is_empty());
    g.act(1,game::Move::Pass).unwrap();g.act(0,game::Move::Pass).unwrap();
    assert_eq!(g.battlefield.len(),1);
}

#[test]
fn r608_2i_imported_cast_ordinal_counts_frozen_spells_without_retriggering_them() {
    let faces=oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let monk=CardDef::from_face(faces.iter().find(|face|face.name=="Monk of the Open Hand").unwrap()).unwrap();
    let old=CardDef::new("Old spell","{G}","Instant","Target creature gets +3/+3 until end of turn.",None).unwrap();
    let new=CardDef::new("New spell","{0}","Sorcery","You gain 1 life.",None).unwrap();
    let mut g=Game::new(vec![monk,old,new],2,20);
    let permanent=g.put(0,0,Zone::Battlefield);let physical=g.put(0,1,Zone::Graveyard);
    let source=g.initial_continuous_source(physical);let cast=g.put(0,2,Zone::Hand);
    g.settle_initial_position(3,Part::PrecombatMain,0,0,0,&[(permanent,0,false)]).unwrap();
    g.set_initial_cast_history(&[(source,0)]).unwrap();
    assert!(g.stack.is_empty());assert!(g.counters(permanent).is_empty());
    assert_eq!(g.audit_casts_this_turn(),serde_json::json!([{"source":source.0,"player":0}]));
    g.retire_events(g.log.end());g.collect_departed();
    g.act(0,game::Move::Cast{card:cast,targets:vec![]}).unwrap();
    cite!(["603.2","608.2i"]; assert_eq!(g.stack.len(),2,"the new spell is the second spell, including the imported first cast"));
    g.act(0,game::Move::Pass).unwrap();g.act(1,game::Move::Pass).unwrap();
    assert_eq!(g.counters(permanent).get("+1/+1"),Some(&1));
    g.act(0,game::Move::Pass).unwrap();g.act(1,game::Move::Pass).unwrap();
    assert_eq!(g.players[0].life,21,"only the new spell resolves during this scenario");
    initial_advance(&mut g,|g|g.turn==4);
    assert_eq!(g.audit_casts_this_turn(),serde_json::json!([]));
}

#[test]
fn r608_2i_initial_casts_do_not_guess_historical_x() {
    let blaze=CardDef::new("Blaze","{X}{R}","Sorcery","This spell deals X damage to any target.",None).unwrap();
    let mut g=Game::new(vec![blaze],2,20);
    let physical=g.put(0,0,Zone::Graveyard);let source=g.initial_continuous_source(physical);
    crate::support::refused(g.set_initial_cast_history(&[(source,0)]),"initial cast inventory requires unrepresented historical X");
    crate::support::refused(g.set_initial_cast_history(&[(game::ObjId(u64::MAX),0)]),
        "initial cast inventory requires unique frozen spell incarnations and valid casters");
    assert_eq!(g.audit_casts_this_turn(),serde_json::json!([]));
}

#[test]
fn r603_7_initial_delayed_inventory_cannot_omit_or_duplicate_siblings() {
    let spell=CardDef::new("Two delayed instructions","{0}","Instant",
        "Exile target creature. Return that card to the battlefield under its owner's control at the beginning of the next end step. Return that card to the battlefield under your control at the beginning of the next end step.",None).unwrap();
    let bear=CardDef::new("Bear","{2}","Creature","",Some((2,2))).unwrap();
    let mut g=Game::new(vec![spell,bear],2,20);
    let physical=g.put(0,0,Zone::Graveyard);let source=g.initial_continuous_source(physical);
    let one=g.put(0,1,Zone::Exile);let two=g.put(1,1,Zone::Exile);
    crate::support::refused(g.set_initial_delayed_exile(source,1,0,1,&[one,two]),
        "initial retained exile group requires unrepresented split-object history");
    g.set_initial_delayed_exile(source,1,0,1,&[one]).unwrap();
    crate::support::refused(g.verify_initial_delayed_inventory(),"incomplete initial delayed producer inventory");
    crate::support::refused(g.set_initial_delayed_exile(source,1,0,1,&[one]),"duplicate initial delayed instruction occurrence");
    g.set_initial_delayed_exile(source,1,0,2,&[one]).unwrap();
    cite!(["603.7a"]; assert!(g.verify_initial_delayed_inventory().is_ok()));
}

#[test]
fn r603_2_initial_permanent_does_not_manufacture_an_entry_trigger() {
    let artifact=CardDef::new("Existing artifact","{2}","Artifact","When this artifact enters, draw a card.",None).unwrap();
    let mut g=Game::new(vec![artifact],2,20);
    let permanent=g.put(0,0,Zone::Battlefield);
    g.settle_initial_position(3,Part::PostcombatMain,0,0,0,&[(permanent,0,false)]).unwrap();
    assert_eq!((g.turn,g.part,g.priority),(3,Part::PostcombatMain,0));
    cite!(["603.2", "117.5"]; assert!(g.stack.is_empty())); assert!(g.players[0].hand.is_empty());
    assert_eq!((g.players[0].life,g.players[1].life),(20,20));
}

#[test]
fn r302_6_initial_control_duration_is_separate_from_owner_and_placement() {
    let turtle=CardDef::new("Turtle","{U}","Creature — Turtle","",Some((0,5))).unwrap();
    for this_turn in [false,true] {
        let mut g=Game::new(vec![turtle.clone()],2,20);
        let permanent=g.put(0,0,Zone::Battlefield);
        g.settle_initial_position(3,Part::PrecombatMain,0,1,1,&[(permanent,1,this_turn)]).unwrap();
        cite!(["110.2"]; assert_eq!(g.controller(permanent),1));
        assert_eq!(g.get_object(permanent).unwrap().owner,0);
        cite!(["302.6"]; assert_eq!(g.summoning_sick(permanent),this_turn));
    }
}

#[test]
fn r704_3_initial_priority_rejects_state_that_needs_repair() {
    let zero=CardDef::new("Zero toughness","{0}","Creature","",Some((1,0))).unwrap();
    let mut g=Game::new(vec![zero],2,20);
    let permanent=g.put(0,0,Zone::Battlefield);
    let error=g.settle_initial_position(3,Part::PrecombatMain,0,0,0,&[(permanent,0,false)]).unwrap_err();
    cite!(["117.5", "704.3", "704.5f"]; assert!(error.contains("not settled")));
    assert_eq!(g.turn,1,"failed initialization rolls back");
    assert_eq!(g.battlefield,vec![permanent]);
    assert!(g.players[0].graveyard.is_empty());
}

#[test]
fn r613_7_unrepresented_initial_static_timestamps_are_refused() {
    let lord=CardDef::new("Lord","{1}{G}","Creature","Other creatures you control get +1/+1.",Some((1,1))).unwrap();
    let mut g=Game::new(vec![lord],2,20);
    let permanent=g.put(0,0,Zone::Battlefield);
    cite!(["613.7"]; assert!(g.settle_initial_position(3,Part::PrecombatMain,0,0,0,&[(permanent,0,false)]).unwrap_err().contains("unsupported static")));
}

#[test]
fn r709_5c_initial_room_and_face_designations_are_not_inferred() {
    let half=|name|CardDef::new(name,"{1}{U}","Enchantment — Room","",None).unwrap();
    let room=CardDef::split([half("Left room"),half("Right room")]).unwrap();
    let front=CardDef::new("Front","{U}","Creature","",Some((1,1))).unwrap();
    let back=CardDef::new("Back","","Creature","",Some((2,2))).unwrap();
    let transforming=CardDef::transforming(front,back).unwrap();
    for def in [room,transforming] {
        let mut g=Game::new(vec![def],2,20);
        let permanent=g.put(0,0,Zone::Battlefield);
        cite!(["709.5c", "712.8a"]; assert!(g.settle_initial_position(3,Part::PrecombatMain,0,0,0,&[(permanent,0,false)])
            .unwrap_err().contains("unsupported face/unlocked")));
    }
}

#[test]
fn r607_2a_initial_link_names_exact_producer_and_exiled_incarnations_without_events() {
    let hunter=CardDef::new("Fiend Hunter","{1}{W}{W}","Creature — Human Cleric",
        "When this creature enters, you may exile another target creature.\nWhen this creature leaves the battlefield, return the exiled card to the battlefield under its owner's control.",Some((1,3))).unwrap();
    let turtle=CardDef::new("Turtle","{U}","Creature — Turtle","",Some((0,5))).unwrap();
    let mut g=Game::new(vec![hunter,turtle],2,20);
    let first=g.put(0,0,Zone::Battlefield);
    let second=g.put(0,0,Zone::Battlefield);
    let exiled=g.put(1,1,Zone::Exile);
    assert!(g.set_initial_linked_exile(first,2,exiled).unwrap_err().contains("not a printed linked writer"));
    g.set_initial_linked_exile(second,1,exiled).unwrap();
    g.settle_initial_position(3,Part::PrecombatMain,0,0,0,&[(first,0,false),(second,0,false)]).unwrap();
    let audit=g.audit_game_objects();
    cite!(["607.1", "607.2a"]; assert_eq!(audit["linked_exile"][0]["source"],second.0));
    assert_eq!(audit["linked_exile"][0]["ability"]["paragraph"],1);
    assert_eq!(audit["linked_exile"][0]["exiled"],exiled.0);
    cite!(["603.2"]; assert!(g.stack.is_empty()));
    assert_eq!(g.exile,vec![exiled]);
    assert!(g.audit_initial_history().is_ok());
    assert!(g.set_initial_linked_exile(first,1,exiled).unwrap_err().contains("endpoints"));
}

#[test]
fn r615_7_initial_remaining_prevention_is_a_resource_not_a_characteristic() {
    let healer=CardDef::new("Healer","{2}{W}","Creature — Human Cleric",
        "{T}: Prevent the next 2 damage that would be dealt to any target this turn.",Some((1,1))).unwrap();
    let mut g=Game::new(vec![healer],2,20);
    let source=g.put(0,0,Zone::Battlefield);
    let before=format!("{g:?}");
    crate::support::refused(g.set_initial_prevention(source,1,0,game::Target::Player(1),3),"invalid initial prevention recipient or remaining capacity");
    assert_eq!(format!("{g:?}"),before);
    g.set_initial_prevention(source,1,0,game::Target::Player(1),2).unwrap();
    g.settle_initial_position(3,Part::PrecombatMain,0,0,0,&[(source,0,false)]).unwrap();
    cite!(["603.2", "615.7"]; assert!(g.stack.is_empty()));
    let initial=g.audit_game_objects();
    assert_eq!(initial["prevention"][0]["remaining"],2);
    assert_eq!(initial["prevention"][0]["producer"]["source"],source.0);
    assert_eq!(initial["prevention"][0]["recipients"],serde_json::json!([{"player":1}]));
    g.damage(source,game::Target::Player(1),1);
    cite!(["615.7"]; assert_eq!(g.players[1].life,20));
    assert_eq!(g.audit_game_objects()["prevention"][0]["remaining"],1);
    g.damage(source,game::Target::Player(1),2);
    cite!(["615.7", "615.6"]; assert_eq!(g.players[1].life,19));
    assert_eq!(g.audit_game_objects()["prevention"],serde_json::json!([]));
}

#[test]
fn r110_2_r602_2b_initial_shield_recipient_uses_declared_controller_not_owner() {
    let ward=CardDef::new("Ward","{W}","Creature — Cleric",
        "{T}: Prevent the next 2 damage that would be dealt to target creature you control this turn.",Some((1,1))).unwrap();
    let turtle=CardDef::new("Turtle","{U}","Creature — Turtle","",Some((0,5))).unwrap();
    let mut g=Game::new(vec![ward,turtle],2,20);
    let source=g.put(0,0,Zone::Battlefield);
    let recipient=g.put(1,1,Zone::Battlefield);
    assert!(g.set_initial_prevention(source,1,0,game::Target::Object(recipient),2).unwrap_err().contains("illegal target"));
    g.settle_initial_position(3,Part::PrecombatMain,0,0,0,&[(source,0,false),(recipient,0,false)]).unwrap();
    cite!(["110.2", "602.2b", "601.2c"]; assert!(g.set_initial_prevention(source,1,0,game::Target::Object(recipient),2).is_ok()));
    assert_eq!(g.get_object(recipient).unwrap().owner,1);
    assert_eq!(g.controller(recipient),0);
}

#[test]
fn r122_1a_r704_5g_initial_counters_and_damage_are_checked_at_settled_priority() {
    let turtle=CardDef::new("Turtle","{U}","Creature — Turtle","",Some((0,5))).unwrap();
    for damage in [5,6] {
        let mut g=Game::new(vec![turtle.clone()],2,20);
        let turtle=g.put(1,0,Zone::Battlefield);
        g.set_initial_counters(turtle,"+1/+1",1);
        g.set_initial_damage(turtle,damage).unwrap();
        let result=g.settle_initial_position(3,Part::PrecombatMain,0,0,0,&[(turtle,1,false)]);
        if damage==5 {
            cite!(["122.1a"]; assert!(result.is_ok()));
            assert_eq!(g.audit_game_objects()["objects"][turtle.0.to_string()]["properties"]["toughness"],6);
        } else {
            cite!(["117.5", "704.5g"]; assert!(result.unwrap_err().contains("not settled")));
        }
    }
}

#[test]
fn r611_2_initial_resolved_control_keeps_base_order_and_history_without_casting() {
    use serde_json::json;
    let faces=oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let defs:Vec<_>=["Entrancing Melody","Act of Treason","Raging Goblin"].iter().map(|name|
        CardDef::from_face(faces.iter().find(|face|face.name==*name).unwrap()).unwrap()).collect();
    let mut g=Game::new(defs,2,20);
    let subject=g.put(0,2,Zone::Battlefield);
    let melody=g.put(1,0,Zone::Graveyard);let act=g.put(0,1,Zone::Graveyard);
    let melody_source=g.initial_continuous_source(melody);let act_source=g.initial_continuous_source(act);
    g.settle_initial_position(3,Part::PostcombatMain,0,0,0,&[(subject,0,true)]).unwrap();
    let before=format!("{g:?}");
    crate::support::refused(g.set_initial_continuous(melody,1,1,vec![subject],&json!([{"controller":1}]),&json!({"kind":"indefinite"})),"initial continuous effect requires one live creature subject");
    assert_eq!(format!("{g:?}"),before);
    g.set_initial_continuous(melody_source,1,1,vec![subject],&json!([{"controller":1}]),&json!({"kind":"indefinite"})).unwrap();
    let before=format!("{g:?}");
    crate::support::refused(g.set_initial_continuous(act_source,1,0,vec![subject],&json!([{"controller":1}]),&json!({"kind":"end_of_turn","turn":3})),"initial continuous semantics must match one unambiguous printed effect");
    assert_eq!(format!("{g:?}"),before);
    g.set_initial_continuous(act_source,1,0,vec![subject],&json!([{"controller":0}]),&json!({"kind":"end_of_turn","turn":3})).unwrap();
    g.set_initial_continuous(act_source,1,0,vec![subject],&json!([{"grant":"haste"}]),&json!({"kind":"end_of_turn","turn":3})).unwrap();
    g.set_tapped(subject,true);
    g.settle_initial_position(3,Part::PostcombatMain,0,0,0,&[(subject,0,true)]).unwrap();
    let audit=g.audit_game_objects();
    cite!(["613.7b"]; assert_eq!(g.controller(subject),0));
    assert_eq!(audit["base_controllers"][subject.0.to_string()],0);
    assert_eq!(audit["continuous_effects"].as_array().unwrap().len(),3);
    assert_eq!(audit["continuous_effects"][0]["producer"]["source"],melody_source.0);
    assert_ne!(melody_source,melody);
    cite!(["701.26", "603.2"]; assert!(g.get_object(subject).unwrap().tapped));
    assert!(g.stack.is_empty());
    cite!(["302.6"]; assert_eq!(g.audit_initial_history().unwrap()["control_since"][subject.0.to_string()],"this_turn"));
    g.exile(subject);g.collect_departed();
    let replacement=g.put(0,2,Zone::Battlefield);
    cite!(["400.7", "611.2c"]; assert_eq!(g.controller(replacement),0));
    assert!(g.audit_game_objects()["continuous_effects"].as_array().unwrap().is_empty());
}

// Rules: 602.5b, 702.177a, 400.7
// Counts name the exact printed ability/incarnation, not its current controller.
// Declared consumed limits do not manufacture prior activation events or payments.
#[test]
fn r602_5b_initial_activation_counts_keep_turn_and_incarnation_limits_distinct() {
    use game::audit::{ActivationUseLifetime::{Turn,Incarnation},InitialActivationUse};
    use game::Move;
    let faces=oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let defs:Vec<_>=["Vivi Ornitier","Draconautics Engineer","Brave Brawler"].iter().map(|name|
        CardDef::from_face(faces.iter().find(|face|face.name==*name).unwrap()).unwrap()).collect();
    let mut g=Game::new(defs,2,20);
    let vivi=g.put(0,0,Zone::Battlefield);let engineer=g.put(0,1,Zone::Battlefield);
    let brawler=g.put(0,2,Zone::Battlefield);
    g.settle_initial_position(3,Part::PrecombatMain,0,0,0,&[(vivi,0,false),(engineer,0,false),(brawler,0,false)]).unwrap();
    let usage=|source,paragraph,lifetime,count|InitialActivationUse{source,paragraph,lifetime,count};
    for (invalid,reason) in [
        (vec![usage(vivi,1,Turn,2)],"initial activation count exceeds the unmodified printed limit"),
        (vec![usage(vivi,1,Incarnation,1)],"initial activation lifetime differs from its printed limit"),
        (vec![usage(engineer,1,Turn,1)],"initial turn activation uses need their incarnation total"), // An exhaust use this turn also consumes its lifetime.
        (vec![usage(engineer,1,Incarnation,2)],"initial activation count exceeds the unmodified printed limit"),
        (vec![usage(vivi,2,Turn,1)],"activation use does not name a represented activated ability"), // Its other printed paragraph is triggered.
        (vec![usage(brawler,2,Incarnation,1)],"initial Power-up needs unrepresented entry-turn history"), // Power-up additionally needs entry-turn history.
    ] {
        let before=format!("{g:?}");
        crate::support::refused(g.set_initial_activation_uses(&invalid),reason);
        assert_eq!(format!("{g:?}"),before);
    }
    g.set_initial_activation_uses(&[usage(vivi,1,Turn,1),usage(engineer,1,Incarnation,1)]).unwrap();
    let audit=g.audit_game_objects();let uses=audit["activation_uses"].as_array().unwrap();
    assert_eq!(uses.len(),2);
    for (source,lifetime) in [(vivi,serde_json::json!({"kind":"turn","turn":3})),(engineer,serde_json::json!({"kind":"incarnation"}))] {
        let row=uses.iter().find(|row|row["ability"]["source"]==source.0).unwrap();
        assert_eq!(row["ability"]["paragraph"],1);assert_eq!(row["count"],1);assert_eq!(row["lifetime"],lifetime);
    }
    assert_eq!(g.audit_initial_history().unwrap()["ability_uses"],"represented_activations_only");
    assert!(g.log.is_empty());
    let activate=|source,ability|Move::Activate{source,ability,targets:vec![],paying:vec![]};
    cite!(["602.5b"]; refused("only once each turn"):act_unchanged(&mut g,activate(vivi,0)));
    g.players[0].pool.colors[ir::Color::Red as usize]=1;
    cite!(["602.5b", "702.177a"]; refused("only once for this object"):act_unchanged(&mut g,crate::support::paid("R",activate(engineer,0))));
    g.players[0].pool.colors[ir::Color::Red as usize]=1;g.players[0].pool.colorless=3;
    cite!(["602.5b", "702.177a"]; ok:g.act(0,crate::support::paid("CCCR",activate(engineer,1))));
    assert_eq!(g.audit_game_objects()["activation_uses"].as_array().unwrap().len(),4,"the second printed ability has its own turn and lifetime counts");
    crate::support::refused(g.audit_initial_history(),"initial causal history no longer representable");
}

#[track_caller]
fn act_unchanged(g:&mut Game,action:game::Move)->Result<(),String>{let before=format!("{g:?}");let result=g.act(0,action);assert_eq!(format!("{g:?}"),before);result}


#[test]
fn r503_1_r504_2_initial_priority_windows_do_not_execute_their_beginning() {
    let watcher=CardDef::new("Upkeep watcher","{0}","Artifact",
        "At the beginning of your upkeep, you gain 1 life.\nWhen this artifact enters, draw a card.",None).unwrap();
    let land=CardDef::new("Forest","","Basic Land — Forest","",None).unwrap();
    for part in [Part::Upkeep,Part::Draw,Part::PrecombatMain,Part::BeginningOfCombat,
        Part::EndOfCombat,Part::PostcombatMain,Part::End] {
        let mut g=Game::new(vec![watcher.clone(),land.clone()],2,20);
        let watcher=g.put(0,0,Zone::Battlefield);
        let top=g.put(0,1,Zone::Library);
        let hand=g.put(0,1,Zone::Hand);
        g.set_tapped(watcher,true);
        g.settle_initial_position(3,part,0,0,0,&[(watcher,0,false)]).unwrap();
        cite!(["503.1","504.2","603.2"]; assert!(g.stack.is_empty()));
        assert!(g.log.is_empty());
        assert_eq!(g.players[0].life,20);
        assert_eq!(g.players[0].hand,vec![hand]);
        assert_eq!(g.players[0].library,vec![top]);
        assert!(g.get_object(watcher).unwrap().tapped,"no earlier untap is executed");
        if part==Part::Upkeep {
            g.act(0,game::Move::Pass).unwrap();g.act(1,game::Move::Pass).unwrap();
            cite!(["504.1"]; assert_eq!(g.part,Part::Draw));
            assert_eq!(g.players[0].hand.len(),2,"explicit passes begin the next draw step");
            assert!(g.players[0].library.is_empty());
        } else if part==Part::Draw {
            g.act(0,game::Move::Pass).unwrap();g.act(1,game::Move::Pass).unwrap();
            assert_eq!(g.part,Part::PrecombatMain);
            cite!(["504.1","504.2"]; assert_eq!(g.players[0].hand,vec![hand],"the supplied draw-step state is already after its draw"));
        }
    }
}

#[test]
fn r103_8a_r500_3_initial_setup_refuses_skipped_and_unsupported_boundaries() {
    let mut g=Game::new(vec![],2,20);
    crate::support::refused(g.settle_initial_position(1,Part::Draw,0,0,0,&[]),
        "initial draw step is skipped on the first turn (103.8a)");
    // The ordinary empty-combat profile does not import declaration/damage
    // steps or exceptional cleanup priority; these are scoped setup refusals.
    for part in [Part::Untap,Part::Cleanup,Part::DeclareAttackers,Part::DeclareBlockers,Part::CombatDamage] {
        cite!(["500.3","508.1","509.1","510.1"];
            refused("unsupported initial priority/turn/history"):g.settle_initial_position(3,part,0,0,0,&[]));
    }
    assert_eq!(g.turn,1);
    g.combat.declared=true;
    crate::support::refused(g.settle_initial_position(3,Part::EndOfCombat,0,0,0,&[]),"unsupported initial priority/turn/history");
}

#[test]
fn r117_5_r704_5g_initial_causes_are_installed_before_settlement() {
    use serde_json::json;
    let growth=CardDef::new("Growth","{G}","Instant","Target creature gets +3/+3 until end of turn.",None).unwrap();
    let bear=CardDef::new("Bear","{2}","Creature — Bear","",Some((2,2))).unwrap();
    let mut g=Game::new(vec![growth,bear],2,20);
    let subject=g.put(0,1,Zone::Battlefield);
    let physical=g.put(0,0,Zone::Graveyard);
    g.set_initial_damage(subject,3).unwrap();
    g.prepare_initial_position(3,Part::Upkeep,0,0,0,&[(subject,0,false)]).unwrap();
    assert_eq!(g.battlefield,vec![subject],"no state-based action before causes exist");
    let source=g.initial_continuous_source(physical);
    g.set_initial_continuous(source,1,0,vec![subject],&json!([{"add_pt":{"power":3,"toughness":3}}]),
        &json!({"kind":"end_of_turn","turn":3})).unwrap();
    g.verify_initial_continuous_inventory().unwrap();
    cite!(["117.5","704.5g","613.4c"]; assert!(g.finish_initial_position().is_ok()));
    assert_eq!(g.battlefield,vec![subject]);
    assert_eq!(g.get_object(subject).unwrap().damage,3);
    assert!(g.log.is_empty());assert!(g.stack.is_empty());
}
