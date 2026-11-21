use crate::support::cite;
use game::{CardDef, Game, Move, Zone};
use ir::Color;
fn def(name: &str, cost: &str, kind: &str, text: &str, pt: Option<(i32,i32)>) -> CardDef {
    let d=CardDef::new(name,cost,kind,text,pt).unwrap();game::play::check(&d).unwrap();d
}
fn setup() -> Game {Game::new(vec![
    def("Empty color","{G}","Creature","Devoid",Some((2,2))),
    def("Blue world","{0}","Enchantment","Creatures are blue.",None),
    def("Colorless spell","{G}","Instant","Devoid\nYou gain 1 life.",None),
],2,20)}
#[test]
fn r702_114a_devoid_functions_in_every_zone_and_on_the_stack() {
    let mut g=setup();
    for zone in [Zone::Hand,Zone::Library,Zone::Graveyard,Zone::Exile,Zone::Command,Zone::Battlefield] {
        let o=g.put(0,0,zone);cite!(["702.114a"]; case: format!("{zone:?}"); assert!(g.seen(o,None).colors.is_empty(),"{zone:?}"));
    }
    let spell=g.put(0,2,Zone::Hand);g.players[0].pool.colors[4]=1;
    g.act(0,crate::support::paid("G",Move::Cast {card:spell,targets:vec![]})).unwrap();
    cite!(["702.114a"]; assert!(g.seen(*g.stack.last().unwrap(),None).colors.is_empty()));
    for _ in 0..2 {g.act(g.priority,Move::Pass).unwrap();}assert_eq!(g.players[0].life,21);
}
#[test]
fn r613_2_color_definitions_precede_other_color_effects_in_both_timestamp_orders() {
    for first in [true,false] {
        let mut g=setup();if first {g.put(0,1,Zone::Battlefield);}
        let creature=g.put(0,0,Zone::Battlefield);if !first {g.put(0,1,Zone::Battlefield);}
        cite!(["613.3"]; case: first; assert_eq!(g.seen(creature,None).colors,vec![Color::Blue]));
    }
}
#[test]
fn r604_3a_unimplemented_granted_devoid_is_refused_instead_of_being_inert() {
    let d=CardDef::new("Grant","{0}","Instant","Target creature gains devoid until end of turn.",None).unwrap();
    crate::support::refused(game::play::check(&d).map_err(|e|e.0), "keyword: Devoid");
}
