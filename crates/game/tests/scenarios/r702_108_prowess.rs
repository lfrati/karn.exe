//! Printed prowess is an independent cast trigger (702.108), composed with history/costs.
use crate::support::cite;
use game::{CardDef, Game, Move, Zone};
use game::state::Decision;

fn def(name: &str, cost: &str, types: &str, text: &str, pt: Option<(i32,i32)>) -> CardDef {
    let d = CardDef::new(name,cost,types,text,pt).unwrap();
    game::play::check(&d).unwrap_or_else(|e|panic!("{name}: {}",e.0)); d
}
fn fixture(text: &str) -> Game {
    let mut g=Game::new(vec![
        def("Lyse Hext","{1}{W}{U}","Legendary Creature — Human Rebel Monk",text,Some((2,2))),
        def("Spell","{1}","Instant","You gain 1 life.",None),
        def("Bear","{0}","Creature — Bear","",Some((2,2))),
        def("Island","","Basic Land — Island","",None),
    ],2,20);
    for p in 0..2 {for _ in 0..8 {g.put(p,3,Zone::Library);}} g
}
fn pass(g: &mut Game) {for _ in 0..2 {g.act(g.priority,Move::Pass).unwrap();}}
fn cast(g: &mut Game, player: usize, index: usize, spending: &str) {
    let card=g.put(player,index,Zone::Hand);
    g.act(player,crate::support::paid(spending,Move::Cast{card,targets:vec![]})).unwrap();
}
const LYSE: &str="Prowess (Whenever you cast a noncreature spell, this creature gets +1/+1 until end of turn.)\nNoncreature spells you cast cost {1} less to cast.\nAs long as you've cast two or more noncreature spells this turn, Lyse Hext has double strike.";

#[test]
fn r702_108_full_lyse_combines_discount_prowess_and_completed_noncreature_history() {
    let mut g=fixture(LYSE);let source=g.put(0,0,Zone::Battlefield);
    cast(&mut g,0,1,"");cite!(["702.108a"]; assert_eq!(g.stack.len(),2));
    assert_eq!(g.power(source),Some(2));assert!(!g.has(source,"Double strike"));
    pass(&mut g);cite!(["702.108a"]; assert_eq!(g.power(source),Some(3)));pass(&mut g);
    cast(&mut g,0,2,"");cite!(["702.108a"]; assert_eq!(g.stack.len(),1));pass(&mut g);
    cast(&mut g,0,1,"");assert_eq!(g.stack.len(),2);cite!(["611.3a"]; assert!(g.has(source,"Double strike")));
    pass(&mut g);cite!(["702.108a"]; assert_eq!((g.power(source),g.toughness(source)),(Some(4),Some(4))));pass(&mut g);
    assert_eq!(g.players[0].life,22);
    let turn=g.turn;
    for _ in 0..100 {if g.turn>turn {break;}match g.decision.clone(){
        Some(Decision::Attackers{player})=>g.act(player,Move::Attack(vec![])).unwrap(),
        Some(Decision::Blockers{player})=>g.act(player,Move::Block(vec![])).unwrap(),
        None=>g.act(g.priority,Move::Pass).unwrap(),other=>panic!("{other:?}"),
    }}
    assert!(g.turn>turn);cite!(["514.2"]; assert_eq!(g.power(source),Some(2)));assert!(!g.has(source,"Double strike"));
}

#[test]
fn r702_108b_multiple_instances_resolve_separately_and_keep_source_identity() {
    let mut g=fixture("Prowess\nProwess");let source=g.put(0,0,Zone::Battlefield);
    g.players[0].pool.colorless=1;cast(&mut g,0,1,"C");
    cite!(["603.3b"]; assert!(matches!(g.decision,Some(Decision::Order{player:0,..}))));
    g.act(0,Move::Order(vec![1,0])).unwrap();cite!(["702.108b"]; assert_eq!(g.stack.len(),3));
    pass(&mut g);assert_eq!(g.power(source),Some(3));
    let mut departed=g.clone();departed.destroy(source);let replacement=departed.put(0,0,Zone::Battlefield);
    pass(&mut departed);cite!(["400.7"]; assert_eq!(departed.power(replacement),Some(2)));
    pass(&mut g);cite!(["702.108b"]; assert_eq!(g.power(source),Some(4)));
}

#[test]
fn r702_108a_creature_and_other_players_casts_do_not_trigger_prowess() {
    let mut g=fixture("Prowess");g.put(1,0,Zone::Battlefield);
    g.players[0].pool.colorless=1;cast(&mut g,0,1,"C");cite!(["702.108a"]; assert_eq!(g.stack.len(),1));pass(&mut g);
    g.put(0,0,Zone::Battlefield);cast(&mut g,0,2,"");cite!(["702.108a"]; assert_eq!(g.stack.len(),1));
}
