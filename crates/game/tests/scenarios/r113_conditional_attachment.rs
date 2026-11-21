//! An attached creature's conditional granted triggers have its own controller (113.7a).
use crate::support::cite;
use game::{CardDef, Game, Move, ObjId, Target, Zone};
use game::state::{Answer, Choice, Decision};
fn game()->Game {
    let faces=oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let helm=CardDef::from_face(faces.iter().find(|f|f.face_name=="Helm of the Ghastlord").unwrap()).unwrap();
    let defs=vec![helm,
        CardDef::new("Pinger","{U}{B}","Creature — Wizard","{0}: This creature deals 1 damage to any target.",Some((2,2))).unwrap(),
        CardDef::new("Forest","","Basic Land — Forest","",None).unwrap(),
        CardDef::new("Blue Paint","{0}","Instant","Target creature becomes blue until end of turn.",None).unwrap(),
        CardDef::new("Fade","{0}","Instant","Target creature loses all abilities until end of turn.",None).unwrap()];
    for d in &defs {game::play::check(d).unwrap_or_else(|e|panic!("{}: {e:?}",d.name()));}
    let mut g=Game::new(defs,3,20);
    for p in 0..3 {for _ in 0..8 {g.put(p,2,Zone::Library);}}
    g
}
fn pass(g:&mut Game){for _ in 0..3 {g.act(g.priority,Move::Pass).unwrap();}}
fn attached(g:&mut Game,owner:usize,controller:usize)->(ObjId,ObjId){
    let creature=g.put(controller,1,Zone::Battlefield);let helm=g.put(owner,0,Zone::Battlefield);
    assert!(g.attach(helm,Target::Object(creature)));(helm,creature)
}
fn ping(g:&mut Game,player:usize,creature:ObjId,to:usize){
    while g.priority!=player {g.act(g.priority,Move::Pass).unwrap();}
    g.act(player,Move::Activate{source:creature,ability:0,targets:vec![vec![Target::Player(to)]],paying:vec![]}).unwrap();pass(g);
    if let Some(Decision::Order{player,abilities})=g.decision.clone(){g.act(player,Move::Order((0..abilities.len()).collect())).unwrap();}
}
#[test]
fn full_helm_two_colors_grant_two_noncombat_triggers_to_creature_controller(){
    let mut g=game();let (_,creature)=attached(&mut g,0,1);g.put(2,2,Zone::Hand);
    assert_eq!((g.power(creature),g.toughness(creature)),(Some(4),Some(4)));
    ping(&mut g,1,creature,2);
    assert_eq!(g.stack.len(),2);cite!(["603.3a"]; assert!(g.stack.iter().all(|o|g.obj(*o).controller==1)));
    let before=g.players[1].hand.len();
    for _ in 0..2 {pass(&mut g);if let Some(Decision::Choose{player,choice:Choice::Objects{..}})=g.decision.clone(){
        assert_eq!(player,2);let card=g.players[2].hand[0];g.act(player,Move::Choose(Answer::Objects(vec![card]))).unwrap();
    }}
    cite!(["109.5"]; assert_eq!(g.players[1].hand.len(),before+1));assert!(g.players[0].hand.is_empty());assert!(g.players[2].hand.is_empty());
}
#[test]
fn changing_color_changes_bonus_and_membership_without_removing_pending_trigger(){
    let mut g=game();let (helm,creature)=attached(&mut g,0,0);g.put(1,2,Zone::Hand);ping(&mut g,0,creature,1);
    let paint=g.put(0,3,Zone::Hand);g.act(0,Move::Cast{card:paint,targets:vec![vec![Target::Object(creature)]]}).unwrap();pass(&mut g);
    cite!(["611.3a"]; assert_eq!(g.power(creature),Some(3)));
    g.move_object(helm,Zone::Hand);
    for _ in 0..2 {pass(&mut g);if let Some(Decision::Choose{player,choice:Choice::Objects{..}})=g.decision.clone(){let card=g.players[player].hand[0];g.act(player,Move::Choose(Answer::Objects(vec![card]))).unwrap();}}
    cite!(["113.7a"]; assert!(g.players[1].hand.is_empty()));cite!(["113.7a"]; assert_eq!(g.players[0].hand.len(),2)); // Helm and drawn Forest
}
#[test]
fn damage_to_same_controller_is_not_opponent_and_attachment_moves_grant(){
    let mut g=game();let (helm,creature)=attached(&mut g,0,0);ping(&mut g,0,creature,0);cite!(["806.1"]; assert!(g.stack.is_empty()));
    let next=g.put(1,1,Zone::Battlefield);assert!(g.attach(helm,Target::Object(next)));
    cite!(["303.4m"]; assert_eq!(g.power(creature),Some(2)));cite!(["303.4m"]; assert_eq!(g.power(next),Some(4)));
    ping(&mut g,0,creature,1);cite!(["303.4m"]; assert!(g.stack.is_empty()));ping(&mut g,1,next,0);cite!(["603.3a", "806.1"]; assert_eq!(g.stack.len(),2));
}
#[test]
fn overlapping_helms_have_distinct_instances_and_removal_hides_all_grants(){
    let mut g=game();let (_,creature)=attached(&mut g,0,0);let other=g.put(0,0,Zone::Battlefield);g.attach(other,Target::Object(creature));
    assert_eq!(g.power(creature),Some(6));ping(&mut g,0,creature,1);cite!(["113.2c"]; assert_eq!(g.stack.len(),4));
    let fade=g.put(0,4,Zone::Hand);g.act(0,Move::Cast{card:fade,targets:vec![vec![Target::Object(creature)]]}).unwrap();pass(&mut g);
    cite!(["613.1f"]; assert!(g.abilities(creature).is_empty()));assert_eq!(g.power(creature),Some(6));cite!(["113.7a"]; assert_eq!(g.stack.len(),4));
}
