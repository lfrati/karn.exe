use crate::support::cite;
use game::{CardDef,Game,Move,Target,Zone};
use game::state::Decision;
fn def(name:&str,types:&str,text:&str,pt:Option<(i32,i32)>)->CardDef {
    let d=CardDef::new(name,"{0}",types,text,pt).unwrap();game::play::check(&d).unwrap();d
}
fn pass(g:&mut Game){for _ in 0..g.players.len(){g.act(g.priority,Move::Pass).unwrap();}}
#[test]
fn r109_5_trigger_target_belongs_to_the_damaged_player_not_any_opponent(){
    let faces=oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let satyr=CardDef::from_face(faces.iter().find(|f|f.name=="Satyr Firedancer").unwrap()).unwrap();game::play::check(&satyr).unwrap();
    let bear=def("Bear","Creature — Bear","",Some((2,4)));
    let bolt=def("Bolt","Instant","Bolt deals 3 damage to target player.",None);
    let mut g=Game::new(vec![satyr,bear,bolt],3,20);g.put(0,0,Zone::Battlefield);
    let own=g.put(0,1,Zone::Battlefield);let a=g.put(1,1,Zone::Battlefield);let b=g.put(2,1,Zone::Battlefield);
    let spell=g.put(0,2,Zone::Hand);g.act(0,Move::Cast{card:spell,targets:vec![vec![Target::Player(1)]]}).unwrap();pass(&mut g);
    assert!(matches!(g.decision,Some(Decision::Targets{..})));
    let (_,game::legal::Asked::Targets{slots,..})=g.asked().unwrap() else {panic!("target offers")};
    cite!(["603.3d"]; assert_eq!(slots[0].among,vec![Target::Object(a)]));
    for (invalid, who) in [(own, "you control it"), (b, "another player controls it")]{cite!(["115.1"]; case: who; refused(&format!("an illegal target, Bear: the target is one the player it names controls, and {who} (115.1)")): g.act(0,Move::ChooseTargets(vec![vec![Target::Object(invalid)]])));}
    let cloned=g.clone();
    g.act(0,Move::ChooseTargets(vec![vec![Target::Object(a)]])).unwrap();pass(&mut g);
    let mut cloned=cloned;cloned.act(0,Move::ChooseTargets(vec![vec![Target::Object(a)]])).unwrap();pass(&mut cloned);
    assert_eq!(cloned.obj(a).damage,3);
    cite!(["120.3e"]; assert_eq!(g.obj(a).damage,3));assert_eq!(g.obj(b).damage,0);assert_eq!(g.obj(own).damage,0);
}
#[test]
fn r108_3_owner_predicate_is_independent_of_current_controller(){
    let bear=def("Bear","Creature — Bear","",Some((2,2)));
    let bring=def("Bring","Sorcery","Return target creature card from a graveyard to the battlefield under your control.",None);
    let blink=def("Blink","Instant","Exile target permanent you own. Return it to the battlefield under your control at the beginning of the next end step.",None);
    let mut g=Game::new(vec![bear,bring,blink],2,20);let owned=g.put(0,0,Zone::Battlefield);let grave=g.put(1,0,Zone::Graveyard);
    let spell=g.put(0,1,Zone::Hand);g.act(0,Move::Cast{card:spell,targets:vec![vec![Target::Object(grave)]]}).unwrap();pass(&mut g);
    let borrowed=*g.battlefield.last().unwrap();cite!(["108.3", "110.2"]; assert_eq!((g.obj(borrowed).owner,g.obj(borrowed).controller),(1,0)));
    let blink=g.put(0,2,Zone::Hand);cite!(["108.3", "115.1"]; refused("an illegal target, Bear: the target is one you own, and another player owns it (115.1)"): g.act(0,Move::Cast{card:blink,targets:vec![vec![Target::Object(borrowed)]]}));
    cite!(["108.3"]; ok: g.act(0,Move::Cast{card:blink,targets:vec![vec![Target::Object(owned)]]}));pass(&mut g);
    assert_eq!(g.battlefield,vec![borrowed]);assert_eq!(g.exile.len(),1);
}
