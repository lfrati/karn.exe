use crate::support::{cite, paid};
use game::{CardDef, Game, Move, ObjId, Target, Zone};
use game::state::{Answer, Choice, Decision};
fn rat() -> CardDef {
    let faces=oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let face=faces.iter().find(|f| f.name=="Pack Rat").unwrap();
    let d=CardDef::from_face(face).unwrap();game::play::check(&d).unwrap();d
}
fn card(name:&str, types:&str,text:&str,pt:Option<(i32,i32)>) -> CardDef {
    let d=CardDef::new(name,"{0}",types,text,pt).unwrap();game::play::check(&d).unwrap();d
}
fn game() -> Game {
    Game::new(vec![rat(),card("Bear","Creature — Bear","",Some((2,2))),
        card("Kill","Instant","Destroy target creature.",None),
        card("Set","Instant","Target creature has base power and toughness 4/4 until end of turn.",None)],2,20)
}
fn pass(g:&mut Game) {for _ in 0..g.players.len(){g.act(g.priority,Move::Pass).unwrap();}}
fn multiply(g:&mut Game,rat:ObjId) {
    let discard=g.put(0,1,Zone::Hand);g.players[0].pool.colors[2]=3;
    g.act(0,paid("BBB",Move::Activate{source:rat,ability:1,targets:vec![],paying:vec![vec![discard]]})).unwrap();
}
#[test]
fn r707_2_pack_rat_tokens_copy_its_dynamic_definition_and_activation() {
    let mut g=game();let original=g.put(0,0,Zone::Battlefield);
    assert_eq!(g.power(original),Some(1));multiply(&mut g,original);pass(&mut g);
    let token=*g.battlefield.last().unwrap();assert!(g.obj(token).token());
    cite!(["707.2"]; assert_eq!(g.def(token).unwrap().name(),"Pack Rat"));cite!(["707.2"]; assert_eq!(g.obj(token).chars.mana_value,2));
    cite!(["604.3"]; assert_eq!((g.power(original),g.power(token)),(Some(2),Some(2))));
    multiply(&mut g,token);pass(&mut g);cite!(["707.2"]; assert_eq!(g.battlefield.len(),3));
    for rat in &g.battlefield {assert_eq!(g.power(*rat),Some(3));}
}
#[test]
fn r608_2h_a_departed_source_still_supplies_its_copiable_values() {
    let mut g=game();let original=g.put(0,0,Zone::Battlefield);multiply(&mut g,original);
    g.act(0,Move::Pass).unwrap();let kill=g.put(1,2,Zone::Hand);
    g.act(1,Move::Cast{card:kill,targets:vec![vec![Target::Object(original)]]}).unwrap();pass(&mut g);
    assert!(g.battlefield.is_empty());pass(&mut g);
    let token=g.battlefield[0];assert!(g.obj(token).token());cite!(["608.2h"]; assert_eq!(g.power(token),Some(1)));
    cite!(["608.2h", "707.2"]; assert_eq!(g.obj(token).chars.name,"Pack Rat"));
}
#[test]
fn r113_6a_characteristic_definitions_function_in_every_zone() {
    let mut g=game();let rat=g.put(0,0,Zone::Battlefield);
    for zone in [Zone::Hand,Zone::Library,Zone::Graveyard,Zone::Exile] {
        let other=g.put(0,0,zone);cite!(["113.6a"]; case: format!("{zone:?}"); assert_eq!(g.power(other),Some(1),"{zone:?}"));
    }
    g.put(1,0,Zone::Battlefield);assert_eq!(g.power(rat),Some(1),"only your Rats");
    multiply(&mut g,rat);pass(&mut g);
    for other in g.players[0].library.iter().chain(&g.players[0].graveyard).chain(&g.exile) {
        if g.obj(*other).card==Some(0){cite!(["113.6a"]; assert_eq!(g.power(*other),Some(2)));}
    }
}
#[test]
fn r613_4a_characteristic_definition_precedes_setting_and_counters() {
    let mut g=game();let rat=g.put(0,0,Zone::Battlefield);g.put_counters(rat,"+1/+1",2);
    let set=g.put(0,3,Zone::Hand);g.act(0,Move::Cast{card:set,targets:vec![vec![Target::Object(rat)]]}).unwrap();pass(&mut g);
    cite!(["613.4a", "613.4b", "613.4c"]; assert_eq!(g.power(rat),Some(6)));multiply(&mut g,rat);pass(&mut g);
    let token=*g.battlefield.last().unwrap();assert_eq!(g.power(token),Some(2));cite!(["613.4b"]; assert_eq!(g.power(rat),Some(6)));
}
#[test]
fn r707_6_copied_tokens_make_new_entry_choices_and_resume_their_creator_once() {
    let model=card("Chosen","Creature — Human","As this creature enters, choose a color.\n{0}: Create a token that's a copy of this creature. Tap it. You gain 1 life.",Some((2,2)));
    let mut g=Game::new(vec![model],2,20);let source=g.put(0,0,Zone::Hand);
    g.act(0,Move::Cast{card:source,targets:vec![]}).unwrap();pass(&mut g);
    g.act(0,Move::Choose(Answer::Value(game::binding::Value::Color(ir::Color::Red)))).unwrap();
    let source=g.battlefield[0];g.act(0,Move::Activate{source,ability:1,targets:vec![],paying:vec![]}).unwrap();pass(&mut g);
    cite!(["707.6"]; assert!(matches!(g.decision,Some(Decision::Choose{choice:Choice::Value(_),..}))));
    assert_eq!(g.battlefield.len(),1);assert_eq!(g.players[0].life,20);cite!(["614.12"]; refused("a decision is pending: a choice of how the permanent enters the battlefield (614.12)"): g.act(0,Move::Pass));
    let clone=g.clone();
    for mut game in [g,clone] {
        game.act(0,Move::Choose(Answer::Value(game::binding::Value::Color(ir::Color::Blue)))).unwrap();
        cite!(["707.6"]; assert_eq!(game.battlefield.len(),2));assert!(game.obj(game.battlefield[1]).token());cite!(["608.2c"]; assert_eq!(game.players[0].life,21));
        assert_eq!(game.log.iter().filter(|e|matches!(e,game::Event::Created{..})).count(),1);
        assert!(!game.obj(source).tapped, "the creator is not its result");
        assert!(game.obj(game.battlefield[1]).tapped, "the result survives its entry choice");
    }
}

#[test]
fn r111_3_copied_tokens_keep_their_definition_until_cessation() {
    let mut g=game();let original=g.put(0,0,Zone::Battlefield);
    multiply(&mut g,original);pass(&mut g);let token=*g.battlefield.last().unwrap();
    g.move_object(token,Zone::Graveyard);let departed=*g.players[0].graveyard.last().unwrap();
    assert_eq!(g.def(departed).unwrap().name(),"Pack Rat");
    cite!(["113.6a"]; assert_eq!(g.power(departed),Some(1),"its defining ability still functions in the graveyard"));
    g.move_object(departed,Zone::Battlefield);cite!(["111.8"]; assert_eq!(g.obj(departed).zone,Zone::Graveyard,"111.8: token cannot return"));
    g.act(0,Move::Pass).unwrap();cite!(["704.5d"]; assert!(!g.players[0].graveyard.contains(&departed)));
}
#[test]
fn r613_1f_removing_the_defining_ability_leaves_zero_base_toughness() {
    let strip=card("Strip","Instant","Target creature loses all abilities until end of turn.",None);
    let mut g=Game::new(vec![rat(),strip],2,20);
    let original=g.put(0,0,Zone::Battlefield);let spell=g.put(0,1,Zone::Hand);
    g.act(0,Move::Cast{card:spell,targets:vec![vec![Target::Object(original)]]}).unwrap();pass(&mut g);
    cite!(["613.1f", "704.5f"]; assert!(g.battlefield.is_empty(),"layer 6 removes the layer 7a definition; 208.2a supplies zero"));
    let departed=g.players[0].graveyard.iter().find(|o|g.obj(**o).card==Some(0)).unwrap();
    assert_eq!(g.power(*departed),Some(0));
}
#[test]
fn r707_2_target_copy_uses_ordinary_entry_counters_and_triggers() {
    let model=card("Model","Creature — Human","This creature enters with two +1/+1 counters on it.\nWhen this creature enters, you gain 3 life.",Some((1,1)));
    let create=card("Duplicate","Instant","Create a token that's a copy of target creature.",None);
    let mut g=Game::new(vec![model,create],2,20);let source=g.put(0,0,Zone::Battlefield);
    g.put_counters(source,"+1/+1",4);let spell=g.put(0,1,Zone::Hand);
    g.act(0,Move::Cast{card:spell,targets:vec![vec![Target::Object(source)]]}).unwrap();pass(&mut g);
    let token=*g.battlefield.last().unwrap();cite!(["707.2", "614.1c"]; assert_eq!(g.obj(token).counters.get("+1/+1"),Some(&2)));
    assert_eq!(g.power(token),Some(3));assert_eq!(g.players[0].life,20);pass(&mut g);
    cite!(["603.6a"]; assert_eq!(g.players[0].life,23,"new token's own entry trigger resolves normally"));
}
#[test]
fn r608_2b_illegal_copy_target_creates_nothing() {
    let mut g=Game::new(vec![card("Model","Creature — Human","",Some((1,1))),
        card("Duplicate","Instant","Create a token that's a copy of target creature.",None)],2,20);
    let source=g.put(0,0,Zone::Battlefield);let spell=g.put(0,1,Zone::Hand);
    g.act(0,Move::Cast{card:spell,targets:vec![vec![Target::Object(source)]]}).unwrap();
    g.move_object(source,Zone::Graveyard);pass(&mut g);
    assert!(g.battlefield.is_empty());cite!(["608.2b"]; assert!(!g.log.iter().any(|e|matches!(e,game::Event::Created{..}))));
}
