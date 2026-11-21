//! Chosen X in a life cost is announced and locked with the other casting costs.
use crate::support::{cite, paid};
use game::{CardDef, Game, Move, Zone};
use game::legal::Asked;
use game::simulation::Simulation;
use std::sync::OnceLock;
fn game() -> Game {
    static DEFS: OnceLock<Vec<CardDef>>=OnceLock::new();
    Game::new(DEFS.get_or_init(|| {
        let ds=vec![
            CardDef::new("Toxic Deluge","{2}{B}","Sorcery","As an additional cost to cast this spell, pay X life.\nAll creatures get -X/-X until end of turn.",None).unwrap(),
            CardDef::new("Bear","{2}","Creature — Bear","",Some((2,2))).unwrap(),
            CardDef::new("Giant","{4}","Creature — Giant","",Some((4,4))).unwrap(),
            CardDef::new("Gift","{0}","Instant","Exile the top card of your library. You may cast that card without paying its mana cost.",None).unwrap(),
            CardDef::new("Tax","{0}","Enchantment","Noncreature spells cost {1} more to cast.",None).unwrap(),
            CardDef::new("Cancel","{0}","Instant","Counter target spell.",None).unwrap(),
        ];for d in &ds {game::play::check(d).unwrap_or_else(|e|panic!("{}: {e:?}",d.name()));}ds
    }).clone(),2,20)
}
fn cast(card: game::ObjId,x:u32,spending:&str)->Move {paid(spending, Move::WithX(x,Box::new(Move::Cast {card,targets:vec![]})))}
fn resolve(g:&mut Game) {for _ in 0..g.players.len() {g.act(g.priority,Move::Pass).unwrap();}}
#[test]
fn r601_2b_x_life_is_listed_and_paid_once_and_governs_the_whole_effect() {
    for x in [0,2,3,4] {
        let mut g=game();let bear=g.put(0,1,Zone::Battlefield);let giant=g.put(1,2,Zone::Battlefield);
        let spell=g.put(0,0,Zone::Hand);g.players[0].pool.colors[2]=3;
        let mut sim=Simulation::new(g);let ask=sim.request().unwrap().unwrap().clone();
        sim.respond(ask.id,ask.player,cast(spell,x,"BBB")).unwrap();cite!(["118.3b"]; case: format!("X={x}"); assert_eq!(sim.game().unwrap().players[0].life,20-x as i32));
        let mut copy=sim.clone();
        for sim in [&mut sim,&mut copy] {
            for _ in 0..2 {let r=sim.request().unwrap().unwrap().clone();sim.respond(r.id,r.player,Move::Pass).unwrap();}
            let g=sim.game().unwrap();assert_eq!(g.battlefield.contains(&bear),x<2);assert_eq!(g.battlefield.contains(&giant),x<4);
            if x<4 {cite!(["107.3a"]; case: format!("X={x}"); assert_eq!(g.power(giant),Some(4-x as i32)));}
            assert_eq!(g.players[0].life,20-x as i32);
        }
    }
}
#[test]
fn r118_3_missing_x_unpayable_life_and_unpayable_mana_leave_state_unchanged() {
    let mut g=game();g.players[0].life=3;g.players[0].pool.colors[2]=3;let spell=g.put(0,0,Zone::Hand);
    let mut sim=Simulation::new(g);let r=sim.request().unwrap().unwrap().clone();let before=format!("{:?}",sim.game().unwrap());
    cite!(["107.3a", "601.2b"]; refused("that cost has {X}: announce the value of X (107.3a, 601.2b)"): sim.respond(r.id,r.player,Move::Cast {card:spell,targets:vec![]}));
    assert_eq!(format!("{:?}",sim.game().unwrap()),before);
    cite!(["119.4"]; refused("you can't pay 4 life with a life total of 3 (119.4)"): sim.respond(r.id,r.player,cast(spell,4,"BBB")));
    assert_eq!(format!("{:?}",sim.game().unwrap()),before);
    let mut g=game();g.players[0].pool.colors[2]=3;g.put(0,4,Zone::Battlefield);let spell=g.put(0,0,Zone::Hand);
    cite!(["118.3a", "601.2f"]; refused("that mana is not in your mana pool (118.3a)"): g.act(0,cast(spell,2,"CBBB")));assert_eq!(g.players[0].life,20);assert_eq!(g.players[0].pool.total(),3);
    g.players[0].pool.colorless=1;g.act(0,cast(spell,2,"CBBB")).unwrap();cite!(["118.3b"]; assert_eq!(g.players[0].life,18));
}
#[test]
fn r118_9d_waiving_the_mana_cost_still_allows_x_life_to_be_chosen_and_paid() {
    let mut g=game();let bear=g.put(1,1,Zone::Battlefield);g.put(0,0,Zone::Library);
    let gift=g.put(0,3,Zone::Hand);g.act(0,Move::Cast {card:gift,targets:vec![]}).unwrap();resolve(&mut g);
    let Some((0,Asked::Cast {cards}))=g.asked() else {panic!("{:?}",g.asked())};
    g.act(0,cast(cards[0],2,"")).unwrap();cite!(["118.9d"]; assert_eq!(g.players[0].life,18));assert_eq!(g.players[0].pool.total(),0);
    resolve(&mut g);cite!(["107.3a"]; assert!(!g.battlefield.contains(&bear)));
}
#[test]
fn r701_6b_countering_the_spell_does_not_refund_its_x_life_payment() {
    let mut g=game();g.players[0].pool.colors[2]=3;let spell=g.put(0,0,Zone::Hand);g.act(0,cast(spell,3,"BBB")).unwrap();
    let target=g.stack[0];let cancel=g.put(0,5,Zone::Hand);g.act(0,Move::Cast {card:cancel,targets:vec![vec![game::Target::Object(target)]]}).unwrap();resolve(&mut g);
    cite!(["701.6b"]; assert_eq!(g.players[0].life,17));assert!(g.stack.is_empty());
}
