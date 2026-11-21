//! Native audit facts derive from accepted announcements and committed payments.
//! Expected choices follow CR 601.2b/h, 702.34, 702.51, 702.66, 702.180 and 707.10.
use crate::support::{cite,paid};
use game::{CardDef, Game, Move, ObjId, Target, Zone};
use game::announcement::CastRoute;
use serde_json::{json,Value};
fn card(name:&str,cost:&str,types:&str,text:&str)->CardDef {
    let def=CardDef::new(name,cost,types,text,types.contains("Creature").then_some((2,2))).unwrap();
    game::play::check(&def).unwrap();def
}
fn audit(g:&Game,id:ObjId)->Value {g.audit_game_objects()["objects"][id.0.to_string()]["announced"]["cast"].clone()}
fn cast(id:ObjId)->Move {Move::Cast{card:id,targets:vec![]}}
fn resolve(g:&mut Game) {for _ in 0..2 {g.act(g.priority,Move::Pass).unwrap();}}
#[test]
fn r601_2b_audit_retains_announced_flashback_route_and_copy_is_not_a_payment() {
    let mut g=Game::new(vec![card("Recall","{1}","Instant","You gain 1 life.\nFlashback {0}"),
        card("Mirror","{0}","Instant","Copy target spell.")],2,20);
    let source=g.put(0,0,Zone::Graveyard);g.act(0,Move::CastVia(CastRoute::Flashback,Box::new(cast(source)))).unwrap();let spell=g.stack[0];
    cite!(["601.2b","702.34a"]; assert_eq!(audit(&g,spell),json!({"route":{"kind":"flashback"},"permission":null,"completed":true,"payment":{"convoke":[],"delve":[]}})));
    let mirror=g.put(0,1,Zone::Hand);g.act(0,Move::Cast{card:mirror,targets:vec![vec![Target::Object(spell)]]}).unwrap();resolve(&mut g);
    let copy=*g.stack.last().unwrap();assert_ne!(copy,spell);
    cite!(["707.10"]; assert_eq!(audit(&g,copy),json!({"route":{"kind":"flashback"},"permission":null,"completed":false,"payment":null})));
}
#[test]
fn r702_51_audit_retains_each_accepted_creature_and_generic_or_colored_allocation() {
    let mut g=Game::new(vec![card("Spell","{1}{B}","Instant","Convoke\nYou gain 1 life."),card("Black","{B}","Creature","")],2,20);
    let spell=g.put(0,0,Zone::Hand);let a=g.put(0,1,Zone::Battlefield);let b=g.put(0,1,Zone::Battlefield);
    let action=|objects|Move::WithConvoke(vec![Some(ir::Color::Black),None],Box::new(Move::WithCosts(objects,Box::new(cast(spell)))));
    let state_before=format!("{g:?}");
    let before=g.audit_game_objects();
    cite!(["702.51a","118.3"]; refused("Black pays for one thing (118.10)"): g.act(0,action(vec![vec![a],vec![a]])));
    assert_eq!(format!("{g:?}"),state_before);
    cite!(["601.2h","118.10"]; assert_eq!(g.audit_game_objects(),before));
    g.act(0,action(vec![vec![a],vec![b]])).unwrap();
    assert!(g.obj(a).tapped && g.obj(b).tapped);
    assert_eq!(g.players[0].pool.total(),0);
    cite!(["702.51a","702.51b"]; assert_eq!(audit(&g,g.stack[0])["payment"],json!({"convoke":[{"object":a.0,"color":"B"},{"object":b.0,"color":null}],"delve":[]})));
}
#[test]
fn r702_66_audit_delve_group_is_distinct_from_a_discard_additional_cost() {
    let mut g=Game::new(vec![card("Spell","{1}","Instant","Delve\nAs an additional cost to cast this spell, discard a card.\nYou gain 1 life."),card("Bear","{0}","Creature","")],2,20);
    let spell=g.put(0,0,Zone::Hand);let discard=g.put(0,1,Zone::Hand);let delve=g.put(0,1,Zone::Graveyard);
    g.act(0,Move::WithDelve(1,Box::new(Move::WithCosts(vec![vec![discard],vec![delve]],Box::new(cast(spell)))))).unwrap();
    cite!(["702.66a","702.66b","601.2h"]; assert_eq!(audit(&g,g.stack[0])["payment"],json!({"convoke":[],"delve":[delve.0]})));
}
#[test]
fn r702_180_audit_harmonize_distinguishes_optional_tap_and_normal_cast() {
    for route in [CastRoute::Normal,CastRoute::Harmonize{tapped:None},CastRoute::Harmonize{tapped:Some(ObjId(0))}] {
        let mut g=Game::new(vec![card("Spell","{0}","Instant","You gain 1 life.\nHarmonize {2}"),card("Bear","{0}","Creature","")],2,20);
        let bear=g.put(0,1,Zone::Battlefield);
        let route=if matches!(route,CastRoute::Harmonize{tapped:Some(_)}) {CastRoute::Harmonize{tapped:Some(bear)}} else {route};
        let spell=g.put(0,0,if route==CastRoute::Normal {Zone::Hand}else{Zone::Graveyard});g.players[0].pool.colorless=2;
        g.act(0,paid(if matches!(route,CastRoute::Harmonize{tapped:None}){"CC"}else{""},Move::CastVia(route,Box::new(cast(spell))))).unwrap();
        let expected=match route {CastRoute::Normal=>json!({"kind":"normal"}),CastRoute::Harmonize{tapped}=>json!({"kind":"harmonize","tapped":tapped.map(|id|id.0)}),_=>unreachable!()};
        cite!(["702.180a","702.180b"]; assert_eq!(audit(&g,g.stack[0])["route"],expected));
        assert_eq!(audit(&g,g.stack[0])["completed"],true);
    }
}
#[test]
fn r400_7d_audit_survives_resolution_and_resets_on_next_zone_change() {
    let mut g=Game::new(vec![card("Bear","{0}","Creature","")],2,20);
    let bear=g.put(0,0,Zone::Hand);g.act(0,cast(bear)).unwrap();resolve(&mut g);let permanent=g.battlefield[0];
    cite!(["400.7d"]; assert_eq!(audit(&g,permanent)["completed"],true));
    g.move_object(permanent,Zone::Graveyard);let grave=g.players[0].graveyard[0];
    cite!(["400.7"]; assert_eq!(audit(&g,grave),Value::Null));
}
#[test]
fn r601_3_audit_permission_owner_is_frozen_when_source_changes_incarnation() {
    let mut g=Game::new(vec![card("Permit","{0}","Artifact","Once during each of your turns, you may cast a permanent spell with mana value 2 or less from your graveyard."),card("Bear","{0}","Creature","")],2,20);
    let permit=g.put(0,0,Zone::Battlefield);let bear=g.put(0,1,Zone::Graveyard);
    g.act(0,Move::CastVia(CastRoute::Permission{source:permit,ability:0},Box::new(cast(bear)))).unwrap();let spell=g.stack[0];
    let before=audit(&g,spell);
    cite!(["601.3","601.2b"]; assert_eq!(before["route"],json!({"kind":"permission","source":permit.0,"ability":0})));
    assert_eq!(before["permission"]["source"],permit.0);
    assert_eq!(before["permission"]["definition"],"Permit");
    assert_eq!(before["permission"]["paragraph"],1);
    assert_eq!(before["permission"]["definition_identity"],json!({"card":0,"face":"Whole"}));
    g.move_object(permit,Zone::Graveyard);
    cite!(["400.7","601.2b"]; assert_eq!(audit(&g,spell),before));
}

#[test]
fn r707_10_copy_audits_inherited_kicker_without_claiming_payment() {
    let mut g=Game::new(vec![card("Recall","{0}","Instant","Kicker {0}\nYou gain 1 life."),
        card("Mirror","{0}","Instant","Copy target spell.")],2,20);
    let source=g.put(0,0,Zone::Hand);
    g.act(0,Move::WithKicks(1,Box::new(cast(source)))).unwrap();let spell=g.stack[0];
    let mirror=g.put(0,1,Zone::Hand);
    g.act(0,Move::Cast{card:mirror,targets:vec![vec![Target::Object(spell)]]}).unwrap();resolve(&mut g);
    let copy=*g.stack.last().unwrap();
    let retained=|g:&Game|g.audit_game_objects()["objects"][copy.0.to_string()]["announced"]["costs"].clone();
    let expected=json!({"kicks":1,"optional":null,"alternative":null,"buyback":false,
        "entwine":false,"replicate":0,"gift":null});
    cite!(["707.10"]; assert_eq!(retained(&g),expected));
    assert_eq!(audit(&g,copy)["completed"],false);
    assert_eq!(audit(&g,copy)["payment"],Value::Null);
    g.move_object(spell,Zone::Graveyard);
    cite!(["707.2b","707.10"]; assert_eq!(retained(&g),expected));
}
