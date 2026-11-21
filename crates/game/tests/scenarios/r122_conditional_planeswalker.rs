//! Full Kaito face: conditional characteristics, emblems, life-loss history and stun.
use crate::support::cite;
use game::{CardDef,Game,Move,Target,Zone};
use game::state::{Answer,Choice,Decision};
use ir::CardType;
fn kaito()->CardDef {let fs=oracle::corpus::load(&oracle::corpus::default_path()).unwrap();let f=fs.iter().find(|f|f.face_name=="Kaito, Bane of Nightmares").unwrap();let d=CardDef::from_face(f).unwrap();game::play::check(&d).unwrap();d}
fn setup()->Game {let ds=vec![kaito(),CardDef::new("Bear","{0}","Creature — Ninja","",Some((2,2))).unwrap()];Game::new(ds,3,20)}
fn pass(g:&mut Game){for _ in 0..g.players.len(){g.act(g.decision_maker(g.priority),Move::Pass).unwrap();}}
fn next(g:&mut Game){let turn=g.turn;for _ in 0..100 {if g.turn!=turn{return;}let mv=match &g.decision{Some(Decision::Attackers{..})=>Move::Attack(vec![]),Some(Decision::Discard{player,count})=>Move::Discard(g.players[*player].hand[..*count].to_vec()),None=>Move::Pass,x=>panic!("{x:?}")};g.act(g.decision_maker(g.decision_actor()),mv).unwrap();}panic!("turn");}
#[test]fn r205_1a_r613_nested_conditions_remove_planeswalker_type_but_keep_loyalty_and_abilities(){
 let mut g=setup();let k=g.put(0,0,Zone::Battlefield);g.set_initial_counters(k,"loyalty",4);g.put(1,1,Zone::Library);let source=g.put(1,1,Zone::Battlefield);
 cite!(["205.1a"]; assert_eq!(g.obj(k).counters["loyalty"],4));assert!(g.is_a(k,CardType::Creature));cite!(["205.1a"]; assert!(!g.is_a(k,CardType::Planeswalker)));assert_eq!(g.seen(k,None).pt,Some((3,4)));
 cite!(["702.11b"]; assert!(!g.can_target(&game::select::Ctx::new(1,source),Target::Object(k))));assert!(g.can_target(&game::select::Ctx::new(0,source),Target::Object(k)));
 g.damage(source,Target::Object(k),2);cite!(["120.3e"]; assert_eq!((g.obj(k).damage,g.obj(k).counters["loyalty"]),(2,4)));
 next(&mut g);assert!(!g.is_a(k,CardType::Creature));cite!(["611.3a"]; assert!(g.is_a(k,CardType::Planeswalker)));assert!(g.can_target(&game::select::Ctx::new(1,source),Target::Object(k)));
 g.damage(source,Target::Object(k),1);cite!(["120.3c"]; assert_eq!(g.obj(k).counters["loyalty"],3));
}
#[test]fn r606_loyalty_works_while_creature_and_emblem_modifies_ninjas_after_source_leaves(){
 let mut g=setup();let k=g.put(0,0,Zone::Battlefield);g.set_initial_counters(k,"loyalty",4);g.act(0,Move::Activate{source:k,ability:2,targets:vec![],paying:vec![]}).unwrap();pass(&mut g);
 cite!(["606.4"]; assert_eq!(g.obj(k).counters["loyalty"],5));assert_eq!(g.seen(k,None).pt,Some((4,5)));cite!(["114.2"]; assert_eq!(g.command.len(),1));
 cite!(["606.3"]; refused("a loyalty ability of that permanent has already been activated this turn (606.3)"): g.act(0,Move::Activate{source:k,ability:3,targets:vec![],paying:vec![]}));g.exile(k);let own=g.put(0,1,Zone::Battlefield);let other=g.put(1,1,Zone::Battlefield);cite!(["114.4"]; assert_eq!(g.seen(own,None).pt,Some((3,3))));assert_eq!(g.seen(other,None).pt,Some((2,2)));
}
#[test]fn r119_3_turn_history_counts_opponents_once_despite_gains_and_survives_surveil_pause(){
 let mut g=setup();let k=g.put(0,0,Zone::Battlefield);g.set_initial_counters(k,"loyalty",4);
 g.put(0,1,Zone::Library);g.put(0,1,Zone::Library); // surveil two, then draw two
 g.put(1,1,Zone::Library); // next turn
 g.lose_life(0,3).unwrap();g.lose_life(1,1).unwrap();g.gain_life(1,5).unwrap();g.pay_life(1,2).unwrap();let source=g.put(0,1,Zone::Battlefield);g.damage(source,Target::Player(2),1);
 cite!(["119.3","119.4"]; assert_eq!(g.life_lost_this_turn(1),3));g.act(0,Move::Activate{source:k,ability:3,targets:vec![],paying:vec![]}).unwrap();pass(&mut g);
 let Some(Decision::Choose{choice:Choice::Arrange{cards,..},..})=g.decision.clone() else{panic!("surveil");};let mut clone=g.clone();
 for g in [&mut g,&mut clone]{g.act(0,Move::Choose(Answer::Arrange{top:cards.clone(),away:vec![]})).unwrap();cite!(["119.2"]; assert_eq!(g.players[0].hand.len(),2));next(g);assert_eq!(g.life_lost_this_turn(1),0);}
 assert_eq!(format!("{g:?}"),format!("{clone:?}"));
}
#[test]fn r122_1d_stun_replaces_each_actual_untap_attempt_with_one_counter_removal(){
 let mut g=setup();let k=g.put(0,0,Zone::Battlefield);g.set_initial_counters(k,"loyalty",4);g.put(1,1,Zone::Library);let target=g.put(1,1,Zone::Battlefield);
 g.act(0,Move::Activate{source:k,ability:4,targets:vec![vec![Target::Object(target)]],paying:vec![]}).unwrap();g.exile(k);pass(&mut g);assert!(g.obj(target).tapped);assert_eq!(g.obj(target).counters["stun"],2);
 next(&mut g);cite!(["122.1d"]; assert!(g.obj(target).tapped));cite!(["122.1d"]; assert_eq!(g.obj(target).counters["stun"],1));g.untap(target);assert!(g.obj(target).tapped);cite!(["122.1d"]; assert_eq!(g.obj(target).counters.get("stun").copied().unwrap_or(0),0));g.untap(target);assert!(!g.obj(target).tapped);
 g.put_counters(target,"stun",2);g.untap(target);cite!(["122.1d","701.26b"]; assert_eq!(g.obj(target).counters["stun"],2,"untapped permanent has no untap event to replace"));
}
