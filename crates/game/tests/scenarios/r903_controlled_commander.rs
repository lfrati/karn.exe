//! Commander-dependent alternative costs use the existing cost pipeline and effective controller.
use crate::support::{cite, paid};
use game::{CardDef,Game,Move,ObjId,Target,Zone};
use game::announcement::CastCost;
use std::sync::{Arc,OnceLock};

const ROLLICK:usize=0; const GUARDIANSHIP:usize=1; const LEADER:usize=2;
const BEAR:usize=3; const TAX:usize=4; const STEAL:usize=5; const COPY:usize=6; const LAND:usize=7;
fn defs()->Arc<Vec<CardDef>> {
    static DEFS:OnceLock<Arc<Vec<CardDef>>>=OnceLock::new();
    DEFS.get_or_init(|| {
        let faces=oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
        Arc::new(["Deadly Rollick","Fierce Guardianship","Yargle, Glutton of Urborg","Grizzly Bears",
            "Sphere of Resistance","Act of Treason","Irenicus's Vile Duplication","Swamp"].iter().map(|name| {
            let d=CardDef::from_face(faces.iter().find(|f|f.name==*name).unwrap()).unwrap();
            game::play::check(&d).unwrap_or_else(|e|panic!("{name}: {e:?}"));d
        }).collect())
    }).clone()
}
fn game(owner:usize,zone:Zone)->(Game,ObjId) {
    let mut g=Game::new(defs(),3,40);
    for p in 0..3 { for _ in 0..5 {g.put(p,LAND,Zone::Library);} }
    let c=g.put(owner,LEADER,Zone::Command);g.designate_commander(c).unwrap();
    if zone!=Zone::Command {g.move_object(c,zone);}
    if matches!(zone,Zone::Graveyard|Zone::Exile) {
        g.state_based_actions();g.act(owner,Move::Choose(game::state::Answer::No)).unwrap();
    }
    let c=g.battlefield.iter().chain(&g.stack).chain(&g.command).chain(&g.exile)
        .chain(&g.players[owner].hand).chain(&g.players[owner].graveyard).chain(&g.players[owner].library)
        .copied().find(|c|g.is_commander(*c)).unwrap();
    (g,c)
}

fn free(card:ObjId,target:ObjId)->Move {
    Move::CastFor(CastCost::Alternative(0),Box::new(Move::Cast{card,targets:vec![vec![Target::Object(target)]]}))
}
fn resolve(g:&mut Game) {for _ in 0..g.players.len(){g.act(g.priority,Move::Pass).unwrap();}}

#[test]
fn r903_3_free_cast_requires_a_controlled_designated_permanent_not_a_commander_in_another_zone() {
    for zone in [Zone::Command,Zone::Hand,Zone::Library,Zone::Graveyard,Zone::Exile,Zone::Stack,Zone::Battlefield] {
        let (mut g,_)=game(0,zone);
        let target=g.put(1,BEAR,Zone::Battlefield);let card=g.put(0,ROLLICK,Zone::Hand);
        cite!(["903.3d"]; case: format!("{zone:?}"); assert_eq!(g.clone().act(0,free(card,target)).is_ok(),zone==Zone::Battlefield,"{zone:?}: {:?}",g.clone().act(0,free(card,target))));
        let result=g.act(0,free(card,target));
        if zone==Zone::Battlefield { result.unwrap();resolve(&mut g);assert!(!g.battlefield.contains(&target)); }
        else {cite!(["903.3d"]; case: format!("{zone:?}"); refused("the condition for that alternative cost is not true (118.9, 601.2b)"): result);assert_eq!(g.obj(card).zone,Zone::Hand);}
    }
}

#[test]
fn r903_3_control_of_an_opponents_commander_qualifies_but_copying_it_does_not() {
    let (mut g,leader)=game(1,Zone::Battlefield);
    let target=g.put(2,BEAR,Zone::Battlefield);let card=g.put(0,ROLLICK,Zone::Hand);
    assert!(!g.clone().act(0,free(card,target)).is_ok());
    let steal=g.put(0,STEAL,Zone::Hand);g.players[0].pool.colors[3]=3;
    g.act(0,paid("RRR",Move::Cast{card:steal,targets:vec![vec![Target::Object(leader)]]})).unwrap();resolve(&mut g);
    assert_eq!(g.controller(leader),0);cite!(["903.3d"]; assert!(g.clone().act(0,free(card,target)).is_ok()));
    let copy=g.put(0,COPY,Zone::Hand);g.players[0].pool.colors[1]=4;
    g.act(0,paid("UUUU",Move::Cast{card:copy,targets:vec![vec![Target::Object(leader)]]})).unwrap();resolve(&mut g);
    assert!(g.battlefield.iter().any(|c|g.obj(*c).token()));
    let mut without_original=g.clone();without_original.move_object(leader,Zone::Command);
    cite!(["903.3", "903.3d"]; assert!(!without_original.clone().act(0,free(card,target)).is_ok(),"a controlled copy is not the designated card"));
    g.act(0,free(card,target)).unwrap();resolve(&mut g);assert!(!g.battlefield.contains(&target));
}

#[test]
fn r118_9_free_guardianship_keeps_its_target_restriction_and_still_pays_tax() {
    let (mut g,_)=game(0,Zone::Battlefield);
    let noncreature=g.put(1,TAX,Zone::Stack);let creature=g.put(1,BEAR,Zone::Stack);
    let card=g.put(0,GUARDIANSHIP,Zone::Hand);g.put(2,TAX,Zone::Battlefield);
    assert!(!g.clone().act(0,paid("C",free(card,noncreature))).is_ok());cite!(["118.9d", "601.2f"]; refused("that mana is not in your mana pool (118.3a)"): g.act(0,paid("C",free(card,noncreature))));
    g.players[0].pool.colorless=1;assert!(g.clone().act(0,paid("C",free(card,noncreature))).is_ok());
    cite!(["115.1"]; refused("an illegal target, Grizzly Bears: it is a creature (115.1)"): g.act(0,paid("C",free(card,creature))));assert_eq!(g.players[0].pool.total(),1);
    g.act(0,paid("C",free(card,noncreature))).unwrap();cite!(["118.9d"]; assert_eq!(g.players[0].pool.total(),0));
    resolve(&mut g);cite!(["701.6a"]; assert!(!g.stack.contains(&noncreature)));assert!(g.stack.contains(&creature));
}

#[test]
fn r118_9_mana_cost_still_works_without_a_commander_on_the_battlefield() {
    let (mut g,_)=game(0,Zone::Command);let target=g.put(1,BEAR,Zone::Battlefield);
    let card=g.put(0,ROLLICK,Zone::Hand);g.players[0].pool.colors[2]=4;
    cite!(["903.3d"]; assert!(!g.clone().act(0,free(card,target)).is_ok()));
    cite!(["118.9"]; ok: g.act(0,paid("BBBB",Move::Cast{card,targets:vec![vec![Target::Object(target)]]})));
    assert_eq!(g.players[0].pool.total(),0);resolve(&mut g);assert!(!g.battlefield.contains(&target));
}
