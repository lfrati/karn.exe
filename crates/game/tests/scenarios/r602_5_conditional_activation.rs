//! Conditional activation uses the same compiled predicates as other rule queries (602.5b).
use crate::support::{cite,paid};
use game::{CardDef, Game, Move, ObjId, Zone};
use game::state::Decision;

fn def(name: &str, kind: &str, text: &str, pt: Option<(i32,i32)>) -> CardDef {
    let d=CardDef::new(name,"{0}",kind,text,pt).unwrap();
    game::play::check(&d).unwrap_or_else(|e|panic!("{name}: {}",e.0)); d
}
fn pass(g:&mut Game) { for _ in 0..2 {g.act(g.priority,Move::Pass).unwrap();} }
fn activate(source:ObjId) -> Move {paid("U",Move::Activate{source,ability:1,targets:vec![],paying:vec![]})}

#[test]
fn r602_5b_metalcraft_is_checked_at_activation_and_not_again_on_resolution() {
    let sphinx=def("Argent Sphinx","Creature — Sphinx","Flying\nMetalcraft — {U}: Exile this creature. Return it to the battlefield under your control at the beginning of the next end step. Activate only if you control three or more artifacts.",Some((4,3)));
    let mut g=Game::new(vec![sphinx,def("Relic","Artifact","",None),def("Island","Basic Land — Island","",None)],2,20);
    let source=g.put(0,0,Zone::Battlefield);
    g.put(0,1,Zone::Battlefield); g.put(0,1,Zone::Battlefield);
    g.players[0].pool.colors[ir::Color::Blue as usize]=1;

    cite!(["602.5"]; assert!(!g.clone().act(0, activate(source)).is_ok()));
    cite!(["602.5"]; refused("the activation condition does not hold (602.5)"): g.act(0,activate(source)));
    assert_eq!(g.players[0].pool.total(),1);
    let third=g.put(0,1,Zone::Battlefield);
    cite!(["602.5"]; assert!(g.clone().act(0, activate(source)).is_ok())); g.act(0,activate(source)).unwrap();
    g.move_object(third,Zone::Graveyard);
    pass(&mut g);
    assert!(!g.battlefield.contains(&source)); assert_eq!(g.exile.len(),1);
    for _ in 0..100 {
        if g.part==ir::Part::End && !g.stack.is_empty() {break;}
        match g.decision.clone() {
            Some(Decision::Attackers{player})=>g.act(player,Move::Attack(vec![])).unwrap(),
            Some(Decision::Blockers{player})=>g.act(player,Move::Block(vec![])).unwrap(),
            None=>g.act(g.priority,Move::Pass).unwrap(),
            other=>panic!("unexpected {other:?}"),
        }
    }
    assert_eq!(g.part,ir::Part::End); assert!(!g.stack.is_empty()); pass(&mut g);
    cite!(["603.7a"]; assert!(g.exile.is_empty()));
    let returned=*g.battlefield.last().unwrap();
    cite!(["400.7"]; assert_ne!(returned,source)); assert_eq!(g.controller(returned),0);
    assert_eq!(g.obj(returned).chars.name, "Argent Sphinx");
    let mut names: Vec<_> = g.battlefield.iter().map(|o| g.obj(*o).chars.name.as_str()).collect();
    names.sort();
    assert_eq!(names, ["Argent Sphinx", "Relic", "Relic"]);
}

fn mana_game(text: &str, first: bool) -> (Game,ObjId) {
    let a=def("Conditional","Artifact",text,None);
    let b=def("Unconditional","Artifact","{T}: Add {G}.",None);
    let spell=CardDef::new("Spell","{G}{G}","Instant","You gain 1 life.",None).unwrap();
    let mut g=Game::new(vec![a,b,spell],2,20);
    g.put(0,0,Zone::Battlefield); g.put(0,if first {1}else{0},Zone::Battlefield);
    let spell=g.put(0,2,Zone::Hand); (g,spell)
}
#[test]
fn r601_2g_conditional_mana_sources_cannot_both_use_a_pre_payment_predicate() {
    let (mut g,spell)=mana_game("{T}: Add {G}. Activate only if you control no tapped artifacts.",false);
    let sources = g.battlefield.clone();
    for order in [[sources[0], sources[1]], [sources[1], sources[0]]] {
        let plan = order.into_iter().map(|source| game::legal::Step { plan: vec![], x: None, life: vec![], with: None, source, ability: 0, paying: vec![], adding: vec![], chose: vec![], cost_branches: None }).collect();
        let before = format!("{g:?}");
        cite!(["602.5", "601.2h"]; refused("the activation condition does not hold"): g.act(0, Move::WithMana(plan, Box::new(paid("GG", Move::Cast { card: spell, targets: vec![] })))));
        assert_eq!(format!("{g:?}"), before);
    }
}
#[test]
fn r601_2g_a_mana_source_can_become_available_during_payment() {
    let (mut g,spell)=mana_game("{T}: Add {G}. Activate only if you control a tapped artifact.",true);
    let plan = [g.battlefield[1], g.battlefield[0]].into_iter().map(|source| game::legal::Step { plan: vec![], x: None, life: vec![], with: None, source, ability: 0, paying: vec![], adding: vec![], chose: vec![], cost_branches: None }).collect();
    g.act(0,Move::WithMana(plan,Box::new(paid("GG",Move::Cast{card:spell,targets:vec![]})))).unwrap();
    assert_eq!(g.players[0].pool.total(),0); pass(&mut g); assert_eq!(g.players[0].life,21);
}

#[test]
fn r613_1d_rusted_relic_gains_and_loses_its_creature_characteristics() {
    let relic=def("Rusted Relic","Artifact","Metalcraft — This artifact is a 5/5 Golem artifact creature as long as you control three or more artifacts.",None);
    let mut g=Game::new(vec![relic,def("Trinket","Artifact","",None)],2,20);
    let relic=g.put(0,0,Zone::Battlefield); g.put(0,1,Zone::Battlefield);
    assert_eq!(g.seen(relic,None).pt,None);
    let third=g.put(0,1,Zone::Battlefield);
    cite!(["613.4b"]; assert_eq!(g.seen(relic,None).pt,Some((5,5))));
    assert!(g.seen(relic,None).types.contains(&ir::CardType::Artifact));
    cite!(["613.1d"]; assert!(g.seen(relic,None).types.contains(&ir::CardType::Creature)));
    cite!(["613.1d"]; assert!(g.seen(relic,None).subtypes.contains(&ir::Subtype::from_name("Golem").unwrap())));
    g.put_counters(relic,"+1/+1",2);
    cite!(["613.4c"]; assert_eq!(g.seen(relic,None).pt,Some((7,7))));
    g.move_object(third,Zone::Graveyard);
    cite!(["611.3a", "208.3"]; assert_eq!(g.seen(relic,None).pt,None));
    g.put(0,1,Zone::Battlefield);
    assert_eq!(g.seen(relic,None).pt,Some((7,7)));
}
