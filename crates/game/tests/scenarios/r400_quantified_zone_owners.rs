use crate::support::{cite, paid};
use game::{CardDef,Game,Move,Zone,Target};
fn resolve(g:&mut Game){for _ in 0..20{if g.stack.is_empty(){return;}g.act(g.priority,Move::Pass).unwrap();}panic!("stack");}
fn defs()->Vec<CardDef>{
    let faces=oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let d=CardDef::from_face(faces.iter().find(|f|f.face_name=="Soul-Guide Lantern").unwrap()).unwrap();game::play::check(&d).unwrap();
    vec![d,CardDef::new("Card","{0}","Artifact","",None).unwrap()]
}
#[test]
fn r400_1_whole_zones_of_each_opponent_are_one_instruction_and_leave_own_graveyard_alone(){
    let mut g=Game::new(defs(),4,20);let lantern=g.put(0,0,Zone::Battlefield);let own=g.put(0,1,Zone::Graveyard);
    for p in 1..4 {for _ in 0..p {g.put(p,1,Zone::Graveyard);}}
    g.act(0,Move::Activate{source:lantern,ability:1,targets:vec![],paying:vec![]}).unwrap();
    let mut copy=g.clone();for branch in [&mut g,&mut copy] {resolve(branch);cite!(["806.1"]; assert!(branch.players[0].graveyard.contains(&own)));
        cite!(["701.21a"]; assert_eq!(branch.players[0].graveyard.len(),2,"sacrificed Lantern stays in its own graveyard"));
        for p in 1..4 {cite!(["806.1", "406.2"]; assert!(branch.players[p].graveyard.is_empty()));}assert_eq!(branch.exile.len(),6);
    }
    assert_eq!(format!("{g:?}"),format!("{copy:?}"));
}
#[test]
fn r603_3_lantern_entry_target_and_its_other_activation_keep_their_separate_behavior(){
    let mut g=Game::new(defs(),2,20);let grave=g.put(1,1,Zone::Graveyard);let card=g.put(0,0,Zone::Hand);g.players[0].pool.colorless=1;
    g.act(0,paid("C",Move::Cast{card,targets:vec![]})).unwrap();
    for _ in 0..2 {g.act(g.priority,Move::Pass).unwrap();}
    // Target choices for an ETB trigger belong to its controller when it is stacked.
    cite!(["603.3d"]; ok: g.act(0,Move::ChooseTargets(vec![vec![Target::Object(grave)]])));resolve(&mut g);
    cite!(["406.2"]; assert_eq!(g.exile.len(),1));
    let lantern=g.battlefield[0];g.put(0,1,Zone::Library);g.players[0].pool.colorless=1;
    g.act(0,paid("C",Move::Activate{source:lantern,ability:2,targets:vec![],paying:vec![]})).unwrap();resolve(&mut g);
    cite!(["121.1"]; assert_eq!(g.players[0].hand.len(),1));
}
