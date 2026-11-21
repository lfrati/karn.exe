use crate::support::{cite, paid};
use game::{CardDef,Game,Move,ObjId,Zone};
use game::state::{Answer,Choice,Decision};
use ir::Part;
fn card(name:&str,types:&str,text:&str,pt:Option<(i32,i32)>)->CardDef {
    let d=CardDef::new(name,"{0}",types,text,pt).unwrap();game::play::check(&d).unwrap();d
}
fn game()->Game {
    let faces=oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let d=CardDef::from_face(faces.iter().find(|f|f.name=="Aetherling").unwrap()).unwrap();
    game::play::check(&d).unwrap();
    let land=card("Island","Basic Land — Island","",None);
    Game::new(vec![d,land],2,20)
}
fn pass(g:&mut Game){for _ in 0..g.players.len(){g.act(g.priority,Move::Pass).unwrap();}}
fn advance(g:&mut Game,done:impl Fn(&Game)->bool){
    for _ in 0..150 {if done(g){return;}match g.decision.clone(){
        Some(Decision::Attackers{player})=>g.act(player,Move::Attack(vec![])).unwrap(),
        Some(Decision::Blockers{player})=>g.act(player,Move::Block(vec![])).unwrap(),
        None=>g.act(g.priority,Move::Pass).unwrap(),other=>panic!("unexpected {other:?}"),
    }}panic!("did not reach requested step");
}
fn blink(g:&mut Game,source:ObjId){g.players[0].pool.colors[1]+=1;g.act(0,paid("U",Move::Activate{source,ability:0,targets:vec![],paying:vec![]})).unwrap();pass(g);}
#[test]
fn r603_7_next_end_step_returns_the_exact_exiled_incarnation_once() {
    let mut g=game();g.put(1,1,Zone::Library); // Player 1 draws once before the second end step.
    let source=g.put(0,0,Zone::Battlefield);blink(&mut g,source);
    assert!(g.battlefield.is_empty());assert_eq!(g.exile.len(),1);
    g.retire_events(g.log.end());g.collect_departed();
    advance(&mut g,|g|g.part==Part::End&&!g.stack.is_empty());
    cite!(["603.7b"]; assert_eq!(g.turn, 1, "the trigger belongs to this turn's end step"));
    let clone=g.clone();for mut g in [g,clone]{pass(&mut g);assert!(g.exile.is_empty());assert_eq!(g.battlefield.len(),1);assert_ne!(g.battlefield[0],source);
        let returned=g.battlefield[0];advance(&mut g,|g|g.turn>1&&g.part==Part::End);assert_eq!(g.battlefield,vec![returned]);cite!(["603.7b"]; assert!(g.stack.is_empty()));}
}
#[test]
fn r603_7b_registration_during_end_step_waits_for_the_next_turns_end_step(){
    let mut g=game();g.put(1,1,Zone::Library); // The next turn's draw.
    let source=g.put(0,0,Zone::Battlefield);advance(&mut g,|g|g.part==Part::End);
    let turn=g.turn;blink(&mut g,source);assert!(g.stack.is_empty());assert_eq!(g.exile.len(),1);
    advance(&mut g,|g|g.turn>turn&&g.part==Part::End&&!g.stack.is_empty());
    cite!(["603.7a"]; assert_eq!(g.turn, turn + 1, "not the controller's next end step"));
    pass(&mut g);assert_eq!(g.battlefield.len(),1);
}
#[test]
fn r400_7_return_does_not_follow_a_card_that_left_exile(){
    let mut g=game();let source=g.put(0,0,Zone::Battlefield);blink(&mut g,source);let exiled=g.exile[0];
    g.move_object(exiled,Zone::Graveyard);let grave=g.players[0].graveyard[0];g.exile(grave);let changed=g.exile[0];
    advance(&mut g,|g|g.part==Part::End&&!g.stack.is_empty());pass(&mut g);
    cite!(["603.7c"]; assert_eq!(g.exile,vec![changed]));cite!(["603.7c"]; assert!(g.battlefield.is_empty()));
}
#[test]
fn r603_7_delayed_return_owns_its_entry_decision_and_resumes_once(){
    let def=card("Chooser","Creature — Human","As this creature enters, choose a color.\n{0}: Exile this creature. Return it to the battlefield under its owner's control at the beginning of the next end step.",Some((2,2)));
    let mut g=Game::new(vec![def],2,20);let source=g.put(0,0,Zone::Battlefield);
    g.act(0,Move::Activate{source,ability:1,targets:vec![],paying:vec![]}).unwrap();pass(&mut g);
    advance(&mut g,|g|g.part==Part::End&&!g.stack.is_empty());pass(&mut g);
    cite!(["614.12a"]; assert!(matches!(g.decision,Some(Decision::Choose{choice:Choice::Value(_),..}))));cite!(["614.12a"]; refused("a decision is pending: a choice of how the permanent enters the battlefield (614.12)"): g.act(0,Move::Pass));
    let clone=g.clone();for mut g in [g,clone]{g.act(0,Move::Choose(Answer::Value(game::binding::Value::Color(ir::Color::Blue)))).unwrap();assert_eq!(g.battlefield.len(),1);assert!(g.exile.is_empty());assert!(g.stack.is_empty());}
}
