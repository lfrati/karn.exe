use crate::support::{cite, paid};
use game::{CardDef, Game, Move, Target, Zone};
use game::state::{Answer,Decision};
fn sphere() -> CardDef {
    let faces=oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let face=faces.iter().find(|f|f.name=="Detention Sphere").unwrap();
    let d=CardDef::from_face(face).unwrap();game::play::check(&d).unwrap();d
}
#[test]
fn r607_2a_sphere_exiles_same_named_permanents_then_returns_them_to_their_owners() {
    let bear=CardDef::new("Grizzly Bears","{0}","Creature — Bear","",Some((2,2))).unwrap();
    let mut g=Game::new(vec![sphere(),bear,CardDef::new("Remove","{0}","Instant","Destroy target nonland permanent.",None).unwrap()],3,20);
    let a=g.put(1,1,Zone::Battlefield);let b=g.put(2,1,Zone::Battlefield);
    let card=g.put(0,0,Zone::Hand);g.players[0].pool.colors=[1,1,0,0,0];g.players[0].pool.colorless=1;
    g.act(0,paid("CWU",Move::Cast{card,targets:vec![]})).unwrap();pass(&mut g);
    assert!(matches!(g.decision,Some(Decision::Targets{..})),"{:?}",g.decision);
    g.act(0,Move::ChooseTargets(vec![vec![Target::Object(a)]])).unwrap();
    pass(&mut g);g.act(0,Move::Choose(Answer::Yes)).unwrap();
    cite!(["201.2a"]; assert_eq!(g.exile.len(),2));assert_eq!(g.obj(b).zone,Zone::Gone);
    let source=g.battlefield[0];let remove=g.put(0,2,Zone::Hand);
    g.act(0,Move::Cast{card:remove,targets:vec![vec![Target::Object(source)]]}).unwrap();
    pass(&mut g);pass(&mut g);
    cite!(["607.2a"]; assert!(g.exile.is_empty()));cite!(["607.2a"]; assert_eq!(g.battlefield.len(),2));
    assert!(g.battlefield.iter().all(|o|g.obj(*o).owner==g.obj(*o).controller));
}
fn pass(g:&mut Game){for _ in 0..g.players.len(){g.act(g.priority,Move::Pass).unwrap();}}

#[test]
fn r201_5_named_restriction_excludes_every_sphere_and_optional_exile_can_be_declined() {
    let bear=CardDef::new("Grizzly Bears","{0}","Creature — Bear","",Some((2,2))).unwrap();
    let land=CardDef::new("Forest","","Basic Land — Forest","",None).unwrap();
    let mut g=Game::new(vec![sphere(),bear,land],2,20);
    let other=g.put(1,0,Zone::Battlefield);let victim=g.put(1,1,Zone::Battlefield);let land=g.put(1,2,Zone::Battlefield);
    let card=g.put(0,0,Zone::Hand);g.players[0].pool.colors=[1,1,0,0,0];g.players[0].pool.colorless=1;
    g.act(0,paid("CWU",Move::Cast{card,targets:vec![]})).unwrap();pass(&mut g);let source=*g.battlefield.last().unwrap();
    for (forbidden,why) in [(other,"an illegal target, Detention Sphere: it doesn't match the target's description (115.1)"),
        (source,"an illegal target, Detention Sphere: it doesn't match the target's description (115.1)"),(land,"an illegal target, Forest: it is a land (115.1)")] {
        cite!(["115.1"]; case: why; refused(why): g.act(0,Move::ChooseTargets(vec![vec![Target::Object(forbidden)]])));
    }
    g.act(0,Move::ChooseTargets(vec![vec![Target::Object(victim)]])).unwrap();pass(&mut g);
    g.act(0,Move::Choose(Answer::No)).unwrap();cite!(["603.5"]; assert!(g.exile.is_empty()));assert_eq!(g.battlefield.len(),4);
}
#[test]
fn r201_2_a_literal_name_query_matches_copied_values() {
    let clone=CardDef::new("Clone","{0}","Creature — Shapeshifter","You may have this creature enter as a copy of any creature on the battlefield.",Some((0,0))).unwrap();
    let bear=CardDef::new("Grizzly Bears","{0}","Creature — Bear","",Some((2,2))).unwrap();
    let query=CardDef::new("Query","{0}","Instant","Destroy target creature named Grizzly Bears.",None).unwrap();
    for d in [&clone,&bear,&query] {game::play::check(d).unwrap();}
    let mut g=Game::new(vec![clone,bear,query],2,20);let original=g.put(1,1,Zone::Battlefield);
    let card=g.put(0,0,Zone::Hand);g.act(0,Move::Cast{card,targets:vec![]}).unwrap();pass(&mut g);
    g.act(0,Move::Choose(Answer::Objects(vec![original]))).unwrap();let copy=*g.battlefield.last().unwrap();
    let spell=g.put(0,2,Zone::Hand);g.act(0,Move::Cast{card:spell,targets:vec![vec![Target::Object(copy)]]}).unwrap();pass(&mut g);
    cite!(["707.2", "201.2a"]; assert_eq!(g.battlefield,vec![original]));
}
