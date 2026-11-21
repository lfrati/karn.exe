//! 120.10/603.2c/603.12: recipient excess facts and exact reflexive occurrences.
use crate::support::{cite, paid};
use game::{CardDef,Game,Move,ObjId,Zone};
use game::state::Decision;
fn native(name:&str)->CardDef {
    let faces=oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let d=CardDef::from_faces(&CardDef::catalog(&faces)[name]).unwrap();game::play::check(&d).unwrap();d
}
fn creature(name:&str,p:i32,t:i32,text:&str)->CardDef {CardDef::new(name,"{0}","Creature",text,Some((p,t))).unwrap()}
fn passes(g:&mut Game){for _ in 0..2 {g.act(g.priority,Move::Pass).unwrap();}}
fn treasures(g:&Game)->usize {g.battlefield.iter().filter(|o|g.obj(**o).chars.name=="Treasure Token").count()}
fn cast_rampage(g:&mut Game,own:ObjId,other:ObjId){
    g.players[0].pool.colors=[3;5];let card=g.put(0,0,Zone::Hand);
    g.act(0,paid("R",Move::Cast{card,targets:vec![vec![own.into()],vec![other.into()]]})).unwrap();passes(g);
}
#[test]
fn r603_12_native_rampage_declares_artifact_target_only_after_qualifying_fight() {
    let mut g=Game::new(vec![native("Rhino's Rampage"),creature("Own",2,1,""),creature("Other",1,2,""),
        CardDef::new("Artifact","{3}","Artifact","",None).unwrap()],2,20);
    let own=g.put(0,1,Zone::Battlefield);let other=g.put(1,2,Zone::Battlefield);let artifact=g.put(1,3,Zone::Battlefield);
    cast_rampage(&mut g,own,other);
    cite!(["603.12","603.3d"];assert!(matches!(g.decision,Some(Decision::Targets{..}))));
    assert!(g.battlefield.contains(&artifact));
    g.act(0,Move::ChooseTargets(vec![vec![artifact.into()]])).unwrap();passes(&mut g);
    assert!(!g.battlefield.contains(&artifact));assert!(!g.battlefield.contains(&other));
    assert!(!g.battlefield.contains(&own),"the reflexive result survives both fighting creatures dying");
}
#[test]
fn r603_12_native_rampage_no_excess_has_no_artifact_trigger() {
    let mut g=Game::new(vec![native("Rhino's Rampage"),creature("Own",1,3,""),creature("Other",1,2,""),
        CardDef::new("Artifact","{3}","Artifact","",None).unwrap()],2,20);
    let own=g.put(0,1,Zone::Battlefield);let other=g.put(1,2,Zone::Battlefield);let artifact=g.put(1,3,Zone::Battlefield);
    cast_rampage(&mut g,own,other);
    cite!(["120.10","603.12"];assert!(g.decision.is_none()));assert!(g.stack.is_empty());assert!(g.battlefield.contains(&artifact));
}
#[test]
fn r120_9_unsettled_source_qualified_and_quantitative_excess_trigger_remain_refused() {
    for text in ["Whenever a creature is dealt excess damage by this creature, you gain 1 life.",
        "Whenever one or more creatures are dealt excess damage, you gain that much life."] {
        let d=creature("Unsupported",1,1,text);
        let reason=if text.contains("by this creature") {"excess trigger: source-qualified aggregate requires explicit allocation semantics"}
            else {"that much/many without a quantitative trigger event"};
        crate::support::refused(game::play::check(&d).map_err(|e|e.0),reason);
    }
}

#[test]
fn r603_3_native_galleon_entry_damage_captures_before_death_and_control_change() {
    let control=CardDef::new("Control","{0}","Instant","Gain control of target artifact.",None).unwrap();
    game::play::check(&control).unwrap();
    let mut g=Game::new(vec![native("Magmatic Galleon"),creature("Victim",1,2,""),control],2,20);
    let victim=g.put(1,1,Zone::Battlefield);let card=g.put(0,0,Zone::Hand);
    g.players[0].pool.colors=[5;5];g.act(0,paid("RRRRR",Move::Cast{card,targets:vec![]})).unwrap();passes(&mut g);
    let galleon=*g.battlefield.iter().find(|o|g.obj(**o).chars.name=="Magmatic Galleon").unwrap();
    g.act(0,Move::ChooseTargets(vec![vec![victim.into()]])).unwrap();passes(&mut g);
    cite!(["120.10","603.2","603.3a"];assert!(!g.battlefield.contains(&victim)));
    assert_eq!(g.stack.len(),1,"Treasure trigger was captured before lethal-damage SBA");
    g.act(0,Move::Pass).unwrap();let steal=g.put(1,2,Zone::Hand);
    g.act(1,Move::Cast{card:steal,targets:vec![vec![galleon.into()]]}).unwrap();passes(&mut g);
    assert_eq!(g.controller(galleon),1);passes(&mut g);
    assert_eq!(treasures(&g),1);
    let treasure=*g.battlefield.iter().find(|o|g.obj(**o).chars.name=="Treasure Token").unwrap();
    cite!(["603.3a","113.7a"];assert_eq!(g.controller(treasure),0));
}
#[test]
fn r603_12_native_rampage_deathtouch_and_optional_zero_artifact_target() {
    let mut g=Game::new(vec![native("Rhino's Rampage"),creature("Own",1,3,"Deathtouch"),creature("Other",1,8,""),
        CardDef::new("Artifact","{3}","Artifact","",None).unwrap()],2,20);
    let own=g.put(0,1,Zone::Battlefield);let other=g.put(1,2,Zone::Battlefield);let artifact=g.put(1,3,Zone::Battlefield);
    cast_rampage(&mut g,own,other);
    cite!(["120.10","702.2c","603.12"];assert!(matches!(g.decision,Some(Decision::Targets{..}))));
    g.act(0,Move::ChooseTargets(vec![vec![]])).unwrap();passes(&mut g);
    assert!(!g.battlefield.contains(&other));assert!(g.battlefield.contains(&artifact));
}

#[test]
fn r608_2h_qualified_recipient_needs_one_proven_earlier_participant() {
    for (participants,reason) in [
        ("Target creature you control fights target creature you control.","controller qualification"),
        ("Target creature an opponent controls fights target creature an opponent controls.","ambiguous earlier subject")
    ] {
        let text=format!("{participants} When excess damage is dealt to the creature an opponent controls this way, destroy up to one target noncreature artifact with mana value 3 or less.");
        let d=CardDef::new("Ambiguous","{0}","Sorcery",&text,None).unwrap();
        let error=game::play::check(&d).unwrap_err();
        cite!(["608.2h","603.12"];assert!(error.0.contains(reason),"{error:?}"));
    }
}

#[test]
fn r608_2h_native_lost_in_maze_filters_mixed_target_group_at_resolution() {
    let mut g=Game::new(vec![native("Lost in the Maze"),creature("Bear",2,2,"")],2,20);
    let own=g.put(0,1,Zone::Battlefield);let other=g.put(1,1,Zone::Battlefield);
    g.players[0].pool.colorless=2;g.players[0].pool.colors[ir::Color::Blue as usize]=2;
    let card=g.put(0,0,Zone::Hand);
    g.act(0,paid("CCUU",Move::WithX(2,Box::new(Move::Cast{card,targets:vec![]})))).unwrap();passes(&mut g);
    g.act(0,Move::ChooseTargets(vec![vec![own.into(),other.into()]])).unwrap();passes(&mut g);
    cite!(["107.3m","608.2h"];assert!(g.obj(own).tapped && g.obj(other).tapped));
    assert!(g.counters(own).is_empty());assert_eq!(g.counters(other).get("stun"),Some(&1));
}
#[test]
fn r608_2h_qualified_that_reference_filters_existing_single_target() {
    let spell=CardDef::new("Tap and restrict","{0}","Instant","Tap target creature. Put a stun counter on that creature you don't control.",None).unwrap();
    game::play::check(&spell).unwrap();
    for player in [0,1] {
        let mut g=Game::new(vec![spell.clone(),creature("Bear",2,2,"")],2,20);
        let bear=g.put(player,1,Zone::Battlefield);let card=g.put(0,0,Zone::Hand);
        g.act(0,Move::Cast{card,targets:vec![vec![bear.into()]]}).unwrap();passes(&mut g);
        cite!(["608.2h"];assert!(g.obj(bear).tapped));
        assert_eq!(g.counters(bear).get("stun"),if player==1 {Some(&1)}else{None});
    }
}

#[test]
fn r608_2h_qualified_reference_reads_control_after_earlier_instruction() {
    let spell=CardDef::new("Tap then take","{0}","Instant","Tap target creature. Gain control of that creature. Put a stun counter on that creature you don't control.",None).unwrap();
    game::play::check(&spell).unwrap();
    let mut g=Game::new(vec![spell,creature("Bear",2,2,"")],2,20);
    let bear=g.put(1,1,Zone::Battlefield);let card=g.put(0,0,Zone::Hand);
    g.act(0,Move::Cast{card,targets:vec![vec![bear.into()]]}).unwrap();passes(&mut g);
    cite!(["608.2c","608.2h"];assert_eq!(g.controller(bear),0));
    assert!(g.obj(bear).tapped);assert!(g.counters(bear).is_empty());
}
