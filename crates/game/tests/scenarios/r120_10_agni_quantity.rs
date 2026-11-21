//! 120.10/608.2c/608.2i/106.4: exact Fight quantity and all red mana retention.
use crate::support::{cite, paid};
use game::{CardDef, Game, Move, ObjId, Zone};
use ir::Color;
fn native()->CardDef {
    let faces=oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let card=CardDef::from_faces(&CardDef::catalog(&faces)["The Last Agni Kai"]).unwrap();
    game::play::check(&card).unwrap();card
}
fn creature(name:&str,p:i32,t:i32,text:&str)->CardDef {CardDef::new(name,"{0}","Creature",text,Some((p,t))).unwrap()}
fn passes(g:&mut Game){for _ in 0..2 {g.act(g.priority,Move::Pass).unwrap();}}
fn cast(g:&mut Game,own:ObjId,other:ObjId) {
    let card=g.put(0,0,Zone::Hand);
    let mut payment=game::state::Pool::default();payment.colors[Color::Red as usize]=2;
    g.act(0,Move::Paying(payment,Box::new(Move::Cast {card,targets:vec![vec![own.into()],vec![other.into()]]}))).unwrap();passes(g);
}
#[test]
fn r120_10_native_agni_uses_opposing_fight_result_after_both_die() {
    let mut g=Game::new(vec![native(),creature("Own",6,1,""),creature("Other",2,2,"")],2,20);
    let own=g.put(0,1,Zone::Battlefield);let other=g.put(1,2,Zone::Battlefield);
    g.players[0].pool.colors[Color::Red as usize]=2;cast(&mut g,own,other);
    cite!(["120.10","608.2i"];assert_eq!(g.players[0].pool.colors[Color::Red as usize],4));
    assert!(!g.battlefield.contains(&own));assert!(!g.battlefield.contains(&other));
}
#[test]
fn r106_4_native_agni_retains_old_and_later_red_even_without_excess() {
    let mut g=Game::new(vec![native(),creature("Own",2,4,""),creature("Other",2,2,"")],2,20);
    let own=g.put(0,1,Zone::Battlefield);let other=g.put(1,2,Zone::Battlefield);
    g.players[0].pool.colors[Color::Red as usize]=5;g.players[0].pool.colors[Color::Blue as usize]=1;
    g.players[1].pool.colors[Color::Red as usize]=1;
    cast(&mut g,own,other);assert_eq!(g.players[0].pool.colors[Color::Red as usize],3);
    g.add_mana(0,ir::mana::ManaSymbol::Color(Color::Red));
    passes(&mut g);
    cite!(["106.4"];assert_eq!(g.players[0].pool.colors[Color::Red as usize],4));
    assert_eq!(g.players[0].pool.colors[Color::Blue as usize],0);
    assert_eq!(g.players[1].pool.colors[Color::Red as usize],0);
}

#[test]
fn r120_10_native_agni_freezes_marks_deathtouch_and_actual_prevention() {
    for (power,toughness,marked,deathtouch,prevented,expected) in [
        (3,8,0,true,0,2),(6,4,2,false,0,4),(6,2,0,false,3,1),(6,2,0,true,6,0)] {
        let shield=CardDef::new("Shield","{0}","Instant",&format!("Prevent the next {prevented} damage that would be dealt to target creature this turn."),None).unwrap();
        game::play::check(&shield).unwrap();
        let mut g=Game::new(vec![native(),creature("Own",power,10,if deathtouch {"Deathtouch"} else {""}),creature("Other",1,toughness,""),shield],2,20);
        let own=g.put(0,1,Zone::Battlefield);let other=g.put(1,2,Zone::Battlefield);
        if marked>0 {g.damage(own,other.into(),marked);}
        if prevented>0 {let shield=g.put(0,3,Zone::Hand);g.act(0,Move::Cast{card:shield,targets:vec![vec![other.into()]]}).unwrap();passes(&mut g);}
        g.players[0].pool.colors[Color::Red as usize]=2;cast(&mut g,own,other);
        cite!(["120.4b","120.10","702.2c"];assert_eq!(g.players[0].pool.colors[Color::Red as usize],expected));
    }
}
#[test]
fn r608_2b_native_agni_opposing_target_becomes_own_before_resolution() {
    let control=CardDef::new("Control","{0}","Instant","Gain control of target creature.",None).unwrap();game::play::check(&control).unwrap();
    let mut g=Game::new(vec![native(),creature("Own",6,8,""),creature("Other",2,2,""),control],2,20);
    let own=g.put(0,1,Zone::Battlefield);let other=g.put(1,2,Zone::Battlefield);
    let agni=g.put(0,0,Zone::Hand);g.players[0].pool.colors[Color::Red as usize]=2;
    g.act(0,paid("RR",Move::Cast{card:agni,targets:vec![vec![own.into()],vec![other.into()]]})).unwrap();
    let control=g.put(0,3,Zone::Hand);g.act(0,Move::Cast{card:control,targets:vec![vec![other.into()]]}).unwrap();passes(&mut g);passes(&mut g);
    cite!(["608.2b","701.14b"];assert_eq!(g.controller(other),0));assert!(g.battlefield.contains(&other));assert_eq!(g.obj(own).damage,0);
    assert_eq!(g.players[0].pool.colors[Color::Red as usize],0);
    g.add_mana(0,ir::mana::ManaSymbol::Color(Color::Red));passes(&mut g);
    assert_eq!(g.players[0].pool.colors[Color::Red as usize],1,"remaining legal target means final retention instruction still runs");
}
#[test]
fn r608_2b_native_agni_missing_fighter_cannot_supply_excess_but_partial_resolution_keeps_red() {
    for missing in [0,1,2] {
        let mut g=Game::new(vec![native(),creature("Own",6,8,""),creature("Other",2,2,"")],2,20);
        let own=g.put(0,1,Zone::Battlefield);let other=g.put(1,2,Zone::Battlefield);
        let card=g.put(0,0,Zone::Hand);g.players[0].pool.colors[Color::Red as usize]=2;
        g.act(0,paid("RR",Move::Cast{card,targets:vec![vec![own.into()],vec![other.into()]]})).unwrap();
        if missing!=1 {g.move_object(own,Zone::Graveyard);}if missing!=0 {g.move_object(other,Zone::Graveyard);}
        passes(&mut g);assert_eq!(g.players[0].pool.colors[Color::Red as usize],0);
        g.add_mana(0,ir::mana::ManaSymbol::Color(Color::Red));passes(&mut g);
        cite!(["608.2b","701.14b"];assert_eq!(g.players[0].pool.colors[Color::Red as usize],u32::from(missing!=2)));
    }
}
#[test]
fn r608_2c_agni_quantity_does_not_escape_branch_or_accept_other_conditions() {
    for text in [
        "Target creature you control fights target creature an opponent controls. If the creature the opponent controls is dealt excess damage this way, you gain 1 life. Add that much {R}.",
        "Target creature you control fights target creature an opponent controls. If the creature the opponent controls is dealt excess damage this way, you gain 1 life. Otherwise, add that much {R}.",
        "If you control an artifact, add that much {R}.",
        "Target creature an opponent controls fights target creature an opponent controls. If the creature the opponent controls is dealt excess damage this way, add that much {R}.",
        "Target creature you control fights target creature you control. If the creature the opponent controls is dealt excess damage this way, add that much {R}.",
    ] {
        let d=CardDef::new("Unbound","{0}","Instant",text,None).unwrap();
        let reason=if text.starts_with("Target creature an opponent") {"reference: ambiguous earlier subject producers"}
            else if text.contains("fights target creature you control") {"reference: controller qualification has no matching earlier participant"}
            else {"that much/many without a quantitative trigger event"};
        crate::support::refused(game::play::check(&d).map_err(|e|e.0),reason);
    }
}

fn until(g:&mut Game,done:impl Fn(&Game)->bool) {
    for _ in 0..100 {
        if done(g) {return;}
        match g.decision.clone() {
            Some(game::state::Decision::Attackers {player})=>g.act(player,Move::Attack(vec![])).unwrap(),
            Some(game::state::Decision::Blockers {player})=>g.act(player,Move::Block(vec![])).unwrap(),
            Some(other)=>panic!("unexpected {other:?}"),
            None=>g.act(g.priority,Move::Pass).unwrap(),
        }
    }
    panic!("phase not reached");
}
#[test]
fn r106_6_agni_keeps_restricted_red_provenance_before_and_after_until_cleanup() {
    let font=|name,color|CardDef::new(name,"{0}","Artifact",&format!("{{T}}: Add {{{color}}}. Spend this mana only to cast a creature spell."),None).unwrap();
    let spell=CardDef::new("Noncreature","{R}","Instant","You gain 1 life.",None).unwrap();
    let red_creature=CardDef::new("Red Creature","{R}","Creature","",Some((1,1))).unwrap();
    let mut g=Game::new(vec![native(),creature("Own",2,4,""),creature("Other",2,2,""),font("Red Font","R"),font("Blue Font","U"),spell,red_creature],2,20);
    for def in g.defs.iter() {game::play::check(def).unwrap();}
    for player in 0..2 {for _ in 0..10 {g.put(player,1,Zone::Library);}}
    let own=g.put(0,1,Zone::Battlefield);let other=g.put(1,2,Zone::Battlefield);
    let old_font=g.put(0,3,Zone::Battlefield);let later_font=g.put(0,3,Zone::Battlefield);let blue_font=g.put(0,4,Zone::Battlefield);
    for source in [old_font,blue_font] {g.act(0,Move::Activate{source,ability:0,targets:vec![],paying:vec![]}).unwrap();}
    let old=g.players[0].pool.restricted[0].clone();
    g.players[0].pool.colors[Color::Red as usize]=2;cast(&mut g,own,other);passes(&mut g);
    cite!(["106.4","106.6"];assert_eq!(g.players[0].pool.restricted,vec![old.clone()]));
    g.act(0,Move::Activate{source:later_font,ability:0,targets:vec![],paying:vec![]}).unwrap();
    let later=g.players[0].pool.restricted[1].clone();assert_eq!(later.source,later_font);
    let noncreature=g.put(0,5,Zone::Hand);let snapshot=g.players[0].pool.clone();
    let before = format!("{g:?}");
    let mut payment=game::state::Pool::default();payment.restricted.push(old.clone());
    let refusal=g.act(0,Move::Paying(payment.clone(),Box::new(Move::Cast{card:noncreature,targets:vec![]}))).unwrap_err();
    assert_eq!(refusal,"that mana can't be spent on that: its text restricts how it can be spent (106.6)");assert_eq!(g.players[0].pool,snapshot);
    assert_eq!(format!("{g:?}"), before);
    until(&mut g,|g|g.part==ir::Part::PostcombatMain && g.decision.is_none());
    let creature=g.put(0,6,Zone::Hand);g.act(0,Move::Paying(payment,Box::new(Move::Cast{card:creature,targets:vec![]}))).unwrap();passes(&mut g);
    assert_eq!(g.players[0].pool.restricted,vec![later.clone()],"legal creature payment retains the other source's restriction");
    until(&mut g,|g|g.part==ir::Part::PostcombatMain && g.decision.is_none());
    assert_eq!(g.players[0].pool.restricted,vec![later]);
    let turn=g.turn;until(&mut g,|g|g.turn!=turn);
    cite!(["514.2","106.4"];assert_eq!(g.players[0].pool.total(),0));
    g.add_mana(0,ir::mana::ManaSymbol::Color(Color::Red));passes(&mut g);
    assert_eq!(g.players[0].pool.total(),0,"next turn no longer retains red");
}
#[test]
fn r120_10_agni_never_uses_excess_dealt_only_to_the_own_fighter() {
    let mut g=Game::new(vec![native(),creature("Own",1,1,""),creature("Other",6,8,"")],2,20);
    let own=g.put(0,1,Zone::Battlefield);let other=g.put(1,2,Zone::Battlefield);
    g.players[0].pool.colors[Color::Red as usize]=2;cast(&mut g,own,other);
    cite!(["120.10","608.2c"];assert_eq!(g.players[0].pool.colors[Color::Red as usize],0));
}
