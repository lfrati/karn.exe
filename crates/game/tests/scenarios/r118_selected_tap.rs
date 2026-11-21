use crate::support::{cite, paid};
use game::{CardDef,Game,Move,Zone};
use game::simulation::Simulation;
use ir::{Color,mana::ManaSymbol};
fn setup() -> Game {
    let faces=oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let drum=CardDef::from_face(faces.iter().find(|f|f.face_name=="Springleaf Drum").unwrap()).unwrap();
    game::play::check(&drum).unwrap();
    Game::new(vec![drum,
        CardDef::new("Creature","{0}","Artifact Creature","",Some((1,1))).unwrap(),
        CardDef::new("Green spell","{G}","Sorcery","You gain 1 life.",None).unwrap(),
        CardDef::new("Two green","{G}{G}","Sorcery","You gain 2 life.",None).unwrap(),
    ],2,20)
}
fn activate(source: game::ObjId, creature:game::ObjId) -> Move {
    Move::Adding(vec![vec![ManaSymbol::Color(Color::Green)]],Box::new(Move::Activate {
        source,ability:0,targets:vec![],paying:vec![vec![creature]],
    }))
}
#[test]
fn r302_6_selected_tapping_allows_a_new_creature_and_rejects_tapped_or_opponent_resources() {
    let mut g=setup();let drum=g.put(0,0,Zone::Battlefield);
    let hand=g.put(0,1,Zone::Hand);g.move_object(hand,Zone::Battlefield);let own=*g.battlefield.last().unwrap();let other=g.put(1,1,Zone::Battlefield);
    cite!(["118.3"]; refused("Creature is not what that cost asks for"): g.act(0,activate(drum,other)));
    assert!(!g.obj(drum).tapped);assert!(!g.obj(other).tapped);
    g.tap(own);cite!(["118.3"]; refused("Creature is not what that cost asks for"): g.act(0,activate(drum,own)));g.untap(own);
    assert!(g.summoning_sick(own));
    // summoning sickness limits only a creature's own {T} abilities (302.6), not this cost
    cite!(["302.6"]; ok: g.act(0,activate(drum,own)));
    assert!(g.obj(drum).tapped&&g.obj(own).tapped);assert_eq!(g.players[0].pool.colors[4],1);
    cite!(["118.3"]; refused("Creature is not what that cost asks for"): g.act(0,activate(drum,own)));assert_eq!(g.players[0].pool.colors[4],1);
}
#[test]
fn r118_3_one_creature_cannot_pay_two_drums_and_simulation_executes_the_listed_plan() {
    let mut g=setup();let first=g.put(0,0,Zone::Battlefield);let second=g.put(0,0,Zone::Battlefield);
    let creature=g.put(0,1,Zone::Battlefield);let green=g.put(0,2,Zone::Hand);let two=g.put(0,3,Zone::Hand);
    let mut sim=Simulation::new(g);let req=sim.request().unwrap().unwrap().clone();
    let step = |source| game::legal::Step { plan: vec![], x: None, life: vec![], with: None, source, ability: 0, paying: vec![vec![creature]], adding: vec![vec![ManaSymbol::Color(Color::Green)]], chose: vec![], cost_branches: None };
    let before = format!("{:?}", sim.game().unwrap());
    cite!(["118.3", "118.10"]; refused("Creature is not what that cost asks for"): sim.respond(req.id, 0, paid("GG", Move::WithMana(vec![step(first), step(second)], Box::new(Move::Cast { card: two, targets: vec![] })))));
    assert_eq!(format!("{:?}", sim.game().unwrap()), before);
    let plan = vec![step(first)];
    sim.respond(req.id,0,paid("G", Move::WithMana(plan,Box::new(Move::Cast{card:green,targets:vec![]})))).unwrap();
    for _ in 0..2 {let req=sim.request().unwrap().unwrap().clone();sim.respond(req.id,req.player,Move::Pass).unwrap();}
    let g=sim.game().unwrap();cite!(["117.4", "608.2c"]; assert_eq!(g.players[0].life,21));cite!(["601.2g"]; assert!(g.obj(creature).tapped));
}
#[test]
fn r118_3_tap_symbol_and_selected_tap_cannot_spend_the_same_tap() {
    let d=CardDef::new("Self tap","{0}","Creature","{T}, Tap an untapped creature you control: Add {G}.",Some((1,1))).unwrap();
    game::play::check(&d).unwrap();let mut g=Game::new(vec![d],2,20);let o=g.put(0,0,Zone::Battlefield);g.turn+=1;
    cite!(["118.10"]; refused("Self tap pays for one thing (118.10)"): g.act(0,Move::Activate{source:o,ability:0,targets:vec![],paying:vec![vec![o]]}));
    assert!(!g.obj(o).tapped);
}
#[test]
fn r118_3_mixed_selected_tap_and_movement_costs_remain_explicitly_refused() {
    let d=CardDef::new("Mixed","{0}","Artifact","Tap an untapped creature you control, Sacrifice a creature: Add {G}.",None).unwrap();
    let error=game::play::check(&d).unwrap_err();assert!(error.0.contains("selected tapping"),"{}",error.0);
}
