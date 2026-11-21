//! Named-source prohibitions share linked choices and the ordinary activation check.
use crate::support::{cite, paid};
use game::{CardDef,Game,Move,ObjId,Zone};
use game::state::{Answer,Choice,Decision};
use game::binding::{Kind,Value};
fn game()->Game {
 let faces=oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
 let defs=["Pithing Needle","Pack Rat","Forest","Reassembling Skeleton"].map(|name|CardDef::from_face(faces.iter().find(|f|f.name==name).unwrap()).unwrap()).to_vec();
 for d in &defs{game::play::check(d).unwrap_or_else(|e|panic!("{}: {}",d.name(),e.0));}
 Game::new(defs,3,20)
}
fn pass(g:&mut Game){for _ in 0..g.players.len(){g.act(g.priority,Move::Pass).unwrap();}}
fn enter(g:&mut Game,name:&str)->ObjId {
 let card=g.put(0,0,Zone::Hand);g.players[0].pool.colorless+=1;
 g.act(0,paid("C",Move::Cast{card,targets:vec![]})).unwrap();pass(g);
 assert!(matches!(g.decision,Some(Decision::Choose{player:0,choice:Choice::Value(Kind::CardName)})));
 crate::support::refused(g.act(0,Move::Pass), "a decision is pending: a choice of how the permanent enters the battlefield (614.12)");crate::support::refused(g.act(1,Move::Choose(Answer::Value(Value::CardName(name.into())))), "that decision is another player's");
 cite!(["201.4"]; refused("choose the name of a card in the Oracle card reference (201.4)"): g.act(0,Move::Choose(Answer::Value(Value::CardName("Not an Oracle card name".into())))));
 g.act(0,Move::Choose(Answer::Value(Value::CardName(name.into())))).unwrap();
 *g.battlefield.iter().rev().find(|o|g.obj(**o).card==Some(0)).unwrap()
}
fn rat(g:&mut Game,source:ObjId,discard:ObjId)->Result<(),String>{g.act(0,paid("BBB",Move::Activate{source,ability:1,targets:vec![],paying:vec![vec![discard]]}))}
#[test]
fn r602_5_a_named_source_is_not_activatable_until_the_prohibition_leaves(){
 let mut g=game();let source=g.put(0,1,Zone::Battlefield);let discard=g.put(0,2,Zone::Hand);
 let needle=enter(&mut g,"Pack Rat");g.players[0].pool.colors[2]=3;
 cite!(["602.5"]; refused("an effect prohibits activating that ability (602.5)"): rat(&mut g,source,discard));
 let clone=g.clone();for mut g in [g,clone]{
  g.destroy(needle);rat(&mut g,source,discard).unwrap();pass(&mut g);
  assert_eq!(g.battlefield.iter().filter(|o|g.obj(**o).chars.name=="Pack Rat").count(),2);
 }
}
#[test]
fn r605_1a_the_mana_exception_keeps_named_basic_land_mana_abilities(){
 let mut g=game();let forest=g.put(0,2,Zone::Battlefield);enter(&mut g,"Forest");
 g.act(0,Move::Activate { source: forest, ability: 0, targets: vec![], paying: vec![] }).unwrap();cite!(["605.1a"]; assert_eq!(g.players[0].pool.colors[4],1));
}
#[test]
fn r113_6m_named_source_restrictions_also_apply_to_graveyard_abilities(){
 let mut g=game();let card=g.put(0,3,Zone::Graveyard);let needle=enter(&mut g,"Reassembling Skeleton");
 g.players[0].pool.colors[2]=2;
 let activation=paid("BB",Move::Activate{source:card,ability:0,targets:vec![],paying:vec![]});
 cite!(["602.5"]; refused("an effect prohibits activating that ability (602.5)"): g.act(0,activation.clone()));
 g.destroy(needle);g.act(0,activation).unwrap();pass(&mut g);
 assert!(g.players[0].graveyard.iter().all(|o|g.obj(*o).card!=Some(3)));
 cite!(["113.6m"]; assert!(g.battlefield.iter().any(|o|g.obj(*o).card==Some(3) && g.obj(*o).tapped)));
}
#[test]
fn r201_4_a_name_not_in_the_current_game_is_still_a_legal_choice(){
 let mut g=game();let needle=enter(&mut g,"Black Lotus");
 cite!(["201.4"]; assert_eq!(g.obj(needle).choices[0].value,Value::CardName("Black Lotus".into())));
 let names=game::binding::options(Kind::CardName);
 assert!(names.contains(&Value::CardName("Forest".into())));cite!(["201.4"]; assert!(names.contains(&Value::CardName("Black Lotus".into()))));
}
