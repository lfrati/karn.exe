use crate::support::{cite, paid};
use game::{CardDef, Game, Move, Target, Zone};
fn card(name: &str, cost: &str, types: &str, text: &str, pt: Option<(i32,i32)>) -> CardDef {
    let d=CardDef::new(name,cost,types,text,pt).unwrap();game::play::check(&d).unwrap();d
}
fn game() -> Game {
    Game::new(vec![
        card("Watcher","{0}","Creature — Elf","Whenever this creature becomes the target of a spell or ability, you gain 2 life.\nSacrifice this creature: Add {G}.",Some((2,2))),
        card("Two clauses","{G}","Instant","Target creature gets +1/+1 until end of turn. Target creature gains trample until end of turn.",None),
        card("Choice","{0}","Sorcery","Sacrifice a creature. You gain 1 life.",None),
    ],2,20)
}
fn pass(g: &mut Game) {g.act(g.priority,Move::Pass).unwrap();g.act(g.priority,Move::Pass).unwrap();}
#[test]
fn r603_2d_targeting_twice_in_one_announcement_triggers_once() {
    let mut g=game();let w=g.put(0,0,Zone::Battlefield);let spell=g.put(0,1,Zone::Hand);g.players[0].pool.colors[4]=1;
    cite!(["115.3"]; ok: g.act(0,paid("G",Move::Cast{card:spell,targets:vec![vec![Target::Object(w)],vec![Target::Object(w)]]})));
    cite!(["603.2c"]; assert_eq!(g.stack.len(),2));pass(&mut g);cite!(["603.2c"]; assert_eq!(g.players[0].life,22));
    pass(&mut g);assert_eq!(g.power(w),Some(3));
}
#[test]
fn r601_2c_targeting_trigger_is_captured_before_its_watcher_pays_for_the_spell() {
    let mut g=game();let w=g.put(0,0,Zone::Battlefield);let spell=g.put(0,1,Zone::Hand);
    let cast=paid("G",Move::Cast{card:spell,targets:vec![vec![Target::Object(w)],vec![Target::Object(w)]]});
    let before = format!("{g:?}");
    cite!(["118.3a"]; refused("that mana is not in your mana pool (118.3a)"): g.act(0,cast.clone()));assert_eq!(g.players[0].life,20);assert!(g.stack.is_empty());
    assert_eq!(format!("{g:?}"), before);
    g.act(0,Move::WithMana(vec![game::legal::Step { plan: vec![], x: None, life: vec![], with: None, cost_branches: None, chose: vec![], source:w,ability:1,paying:vec![],adding:vec![]}],Box::new(cast))).unwrap();
    assert!(!g.battlefield.contains(&w));cite!(["601.2c"]; assert_eq!(g.stack.len(),2));
    pass(&mut g);assert_eq!(g.players[0].life,22);pass(&mut g);
}
#[test]
fn r115_10_choosing_without_targeting_does_not_trigger() {
    let mut g=game();g.put(0,0,Zone::Battlefield);let spell=g.put(0,2,Zone::Hand);
    g.act(0,Move::Cast{card:spell,targets:vec![]}).unwrap();pass(&mut g);
    cite!(["115.10a"]; assert_eq!(g.players[0].life,21));assert!(g.stack.is_empty());
}

#[test]
fn r115_1_the_targeting_object_must_be_a_spell_or_ability() {
    assert_eq!(CardDef::new("Nonsense","{0}","Creature — Bear","When this creature becomes the target of a player, sacrifice it.",Some((2,2))).expect_err("a targeting source is a spell or ability"),"text the grammar does not read");
}
