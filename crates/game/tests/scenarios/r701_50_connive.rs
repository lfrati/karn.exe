//! Connive is retained per permanent, including its departed incarnation and last controller.
use crate::support::cite;
use game::{CardDef,Game,Move,Zone,ObjId,Target};
use game::state::{Answer,Choice,Decision};
use ir::Color;
fn full()->CardDef {let fs=oracle::corpus::load(&oracle::corpus::default_path()).unwrap();let c=CardDef::catalog(&fs);let d=CardDef::from_faces(&c["Lethal Scheme"]).unwrap();game::play::check(&d).unwrap_or_else(|e|panic!("{}: {:?}",e.0,d.abilities()));d}
fn defs()->Vec<CardDef> {vec![full(),CardDef::new("Convoker","{B}","Creature — Human","When this creature connives, you gain 2 life.",Some((2,2))).unwrap(),CardDef::new("Land","","Basic Land — Island","",None).unwrap(),CardDef::new("Card","{0}","Instant","You gain 1 life.",None).unwrap()]}
fn pass(g:&mut Game){for _ in 0..g.players.len(){g.act(g.priority,Move::Pass).unwrap();}}
fn cast(g:&mut Game,target:ObjId,convokers:&[ObjId], spending:&str) {
    let card=g.put(0,0,Zone::Hand);let mut allocation=vec![Some(Color::Black);convokers.len().min(2)];allocation.resize(convokers.len(),None);
    g.players[0].pool.colors[Color::Black as usize]=4-convokers.len() as u32;
    g.act(0,crate::support::paid(spending, Move::WithConvoke(allocation,Box::new(Move::WithCosts(convokers.iter().map(|o|vec![*o]).collect(),Box::new(Move::Cast{card,targets:vec![vec![Target::Object(target)]]})))))).unwrap();
}
#[test]
fn r701_50a_full_scheme_destroys_then_orders_private_draw_discard_and_counters() {
    let mut g=Game::new(defs(),3,20);let a=g.put(0,1,Zone::Battlefield);let b=g.put(0,1,Zone::Battlefield);let target=g.put(1,1,Zone::Battlefield);
    g.put(0,2,Zone::Hand);for _ in 0..4 {g.put(0,3,Zone::Library);}cast(&mut g,target,&[a,b],"BB");pass(&mut g);
    assert_eq!(g.obj(target).zone,Zone::Gone);cite!(["701.50c"]; assert!(matches!(g.decision,Some(Decision::Choose{player:0,choice:Choice::Objects{verb:ir::Verb::Connive,..}}))));
    crate::support::refused(g.act(1,Move::Choose(Answer::Objects(vec![b]))), "that decision is another player's");g.act(0,Move::Choose(Answer::Objects(vec![b]))).unwrap();
    let nonland=*g.players[0].hand.iter().find(|o|g.obj(**o).card==Some(3)).unwrap();let clone=g.clone();
    for mut g in [g,clone] {
        assert!(matches!(g.decision,Some(Decision::Choose{choice:Choice::Objects{verb:ir::Verb::Discard,..},..})));
        g.act(0,Move::Choose(Answer::Objects(vec![nonland]))).unwrap();cite!(["701.50a"]; assert_eq!(g.obj(b).counters.get("+1/+1"),Some(&1)));assert!(g.obj(a).counters.is_empty());
        let land=g.players[0].hand.iter().copied().find(|o|g.obj(*o).card==Some(2)).unwrap();g.act(0,Move::Choose(Answer::Objects(vec![land]))).unwrap();
        cite!(["701.50a"]; assert!(g.obj(a).counters.is_empty()));assert_eq!(g.log.iter().filter(|e|matches!(e,game::Event::Connived{..})).count(),2);
        assert!(matches!(g.decision,Some(Decision::Order{..})));g.act(0,Move::Order(vec![0,1])).unwrap();
        assert_eq!(g.stack.len(),2);pass(&mut g);pass(&mut g);cite!(["603.2"]; assert_eq!(g.players[0].life,24));
    }
}
#[test]
fn r701_50b_destroyed_convoker_still_connives_and_self_triggers_without_countering_a_successor() {
    let mut g=Game::new(defs(),3,20);let old=g.put(0,1,Zone::Battlefield);for _ in 0..3 {g.put(0,3,Zone::Library);}g.put(0,2,Zone::Hand);
    cast(&mut g,old,&[old],"BBB");pass(&mut g);assert_eq!(g.obj(old).zone,Zone::Gone);
    let nonland=g.players[0].hand.iter().copied().find(|o|g.obj(*o).card==Some(3)).unwrap();g.act(0,Move::Choose(Answer::Objects(vec![nonland]))).unwrap();
    assert_eq!(g.battlefield.len(),0);cite!(["701.50b"]; assert_eq!(g.stack.len(),1));pass(&mut g);cite!(["701.50b"]; assert_eq!(g.players[0].life,22));
}
#[test]
fn r701_50a_counter_replacements_suspend_and_clones_never_replay_draw_or_discard() {
    let mut defs=defs();
    defs.push(CardDef::new("Scales","{G}","Enchantment","If one or more +1/+1 counters would be put on a creature you control, that many plus one +1/+1 counters are put on it instead.",None).unwrap());
    defs.push(CardDef::new("Evolution","{2}{G}","Enchantment","If one or more +1/+1 counters would be put on a creature you control, twice that many +1/+1 counters are put on that creature instead.",None).unwrap());
    let mut g=Game::new(defs,3,20);let a=g.put(0,1,Zone::Battlefield);let target=g.put(1,1,Zone::Battlefield);g.put(0,4,Zone::Battlefield);g.put(0,5,Zone::Battlefield);
    for _ in 0..3 {g.put(0,3,Zone::Library);}g.put(0,2,Zone::Hand);cast(&mut g,target,&[a],"BBB");pass(&mut g);
    let card=g.players[0].hand.iter().copied().find(|o|g.obj(*o).card==Some(3)).unwrap();g.act(0,Move::Choose(Answer::Objects(vec![card]))).unwrap();
    let Some(Decision::Choose{choice:Choice::Replacement{among,..},..})=&g.decision else{panic!("{:?}",g.decision)};
    let effect=among.iter().find(|e|g.obj(e.source).card==Some(5)).unwrap().clone();let clone=g.clone();let library=g.players[0].library.len();
    for mut g in [g,clone] {g.act(0,Move::Choose(Answer::Replacement(effect.clone()))).unwrap();cite!(["616.1"]; assert_eq!(g.obj(a).counters.get("+1/+1"),Some(&3)));assert_eq!(g.players[0].library.len(),library);cite!(["701.50a"]; assert_eq!(g.log.iter().filter(|e|matches!(e,game::Event::Discarded{..})).count(),1));assert_eq!(g.log.iter().filter(|e|matches!(e,game::Event::Connived{..})).count(),1);}
}
#[test]
fn r701_50a_actual_replaced_nonland_discard_counts_but_prevented_discard_does_not() {
    for prohibit in [false,true] {
        let defs=vec![CardDef::new("Connive","{0}","Instant","Target creature connives.",None).unwrap(),CardDef::new("Bear","{G}","Creature","",Some((2,2))).unwrap(),CardDef::new("Replaced","{G}","Creature","If a spell or ability an opponent controls causes you to discard this card, put it onto the battlefield instead of putting it into your graveyard.",Some((2,2))).unwrap(),CardDef::new("Restriction","{0}","Artifact","Creature cards in hands can't enter the battlefield.",None).unwrap(),CardDef::new("Land","","Land","",None).unwrap()];
        let mut g=Game::new(defs,3,20);let a=g.put(1,1,Zone::Battlefield);let hand=g.put(1,2,Zone::Hand);g.put(1,4,Zone::Library);if prohibit{g.put(0,3,Zone::Battlefield);}
        let card=g.put(0,0,Zone::Hand);g.act(0,Move::Cast{card,targets:vec![vec![Target::Object(a)]]}).unwrap();pass(&mut g);
        assert!(matches!(g.decision,Some(Decision::Choose{player:1,..})));g.act(1,Move::Choose(Answer::Objects(vec![hand]))).unwrap();
        cite!(["701.50a"]; assert_eq!(g.obj(a).counters.get("+1/+1").copied().unwrap_or(0),u32::from(!prohibit)));assert_eq!(g.players[1].hand.contains(&hand),prohibit);assert_eq!(g.log.iter().filter(|e|matches!(e,game::Event::Discarded{..})).count(),usize::from(!prohibit));assert_eq!(g.log.iter().filter(|e|matches!(e,game::Event::Connived{..})).count(),1);
    }
}
#[test]
fn r701_50c_current_controllers_connive_in_apnap_order_and_gone_objects_use_lki() {
    let mut ds=defs();ds[1]=CardDef::new("Convoker","{B}","Creature","",Some((2,2))).unwrap();ds.push(CardDef::new("Control","{0}","Instant","Gain control of target creature.",None).unwrap());
    let mut g=Game::new(ds,3,20);let a=g.put(0,1,Zone::Battlefield);let b=g.put(0,1,Zone::Battlefield);let target=g.put(2,1,Zone::Battlefield);
    for player in 0..3{g.put(player,2,Zone::Hand);for _ in 0..3{g.put(player,3,Zone::Library);}}
    cast(&mut g,target,&[a,b],"BB");g.act(0,Move::Pass).unwrap();let steal=g.put(1,4,Zone::Hand);g.act(1,Move::Cast{card:steal,targets:vec![vec![Target::Object(b)]]}).unwrap();pass(&mut g);assert_eq!(g.controller(b),1);
    g.move_object(b,Zone::Graveyard);assert_eq!(g.controller(b),1);pass(&mut g);
    cite!(["701.50c"]; assert!(matches!(g.decision,Some(Decision::Choose{player:0,..}))));let land=g.players[0].hand.iter().copied().find(|o|g.obj(*o).card==Some(2)).unwrap();g.act(0,Move::Choose(Answer::Objects(vec![land]))).unwrap();
    cite!(["701.50c","701.50b"]; assert!(matches!(g.decision,Some(Decision::Choose{player:1,..}))));let nonland=g.players[1].hand.iter().copied().find(|o|g.obj(*o).card==Some(3)).unwrap();g.act(1,Move::Choose(Answer::Objects(vec![nonland]))).unwrap();assert!(g.obj(a).counters.is_empty());cite!(["701.50b"]; assert_eq!(g.log.iter().filter(|e|matches!(e,game::Event::Connived{player:1,..})).count(),1));
}
#[test]
fn r701_50f_impossible_draw_and_discard_still_complete_connive() {
    let mut ds=defs();ds.push(CardDef::new("Skip","{0}","Enchantment","If you would draw a card, draw zero cards instead.",None).unwrap());
    let mut g=Game::new(ds,3,20);let a=g.put(0,1,Zone::Battlefield);let target=g.put(1,1,Zone::Battlefield);g.put(0,4,Zone::Battlefield);g.put(0,3,Zone::Library);
    cast(&mut g,target,&[a],"BBB");pass(&mut g);assert!(g.decision.is_none());assert_eq!(g.players[0].library.len(),1);assert!(g.players[0].hand.is_empty());assert!(g.obj(a).counters.is_empty());cite!(["701.50f"]; assert_eq!(g.log.iter().filter(|e|matches!(e,game::Event::Connived{..})).count(),1));assert_eq!(g.stack.len(),1);
}
#[test]
fn r707_10_spell_copy_has_no_creatures_that_convoked_the_copy() {
    let mut ds=defs();ds.push(CardDef::new("Fork","{0}","Instant","Copy target spell. You may choose new targets for the copy.",None).unwrap());
    let mut g=Game::new(ds,3,20);let a=g.put(0,1,Zone::Battlefield);let target=g.put(1,1,Zone::Battlefield);let other=g.put(2,1,Zone::Battlefield);g.put(0,2,Zone::Hand);for _ in 0..3{g.put(0,3,Zone::Library);}
    cast(&mut g,target,&[a],"BBB");let original=g.stack[0];let fork=g.put(0,4,Zone::Hand);g.act(0,Move::Cast{card:fork,targets:vec![vec![Target::Object(original)]]}).unwrap();pass(&mut g);
    g.act(0,Move::Choose(Answer::Targets(vec![vec![Target::Object(other)]]))).unwrap();pass(&mut g);assert_eq!(g.obj(other).zone,Zone::Gone);assert!(g.decision.is_none());assert_eq!(g.players[0].library.len(),3);cite!(["707.10"]; assert!(!g.log.iter().any(|e|matches!(e,game::Event::Connived{..}))));
    pass(&mut g);assert!(matches!(g.decision,Some(Decision::Choose{choice:Choice::Objects{verb:ir::Verb::Discard,..},..})));
}
#[test]
fn r701_50a_draw_replacements_pause_clones_before_the_single_discard() {
    let mut ds=defs();ds.push(CardDef::new("Double A","{0}","Enchantment","If you would draw a card, draw two cards instead.",None).unwrap());ds.push(CardDef::new("Double B","{0}","Enchantment","If you would draw a card, draw two cards instead.",None).unwrap());
    let mut g=Game::new(ds,3,20);let a=g.put(0,1,Zone::Battlefield);let target=g.put(1,1,Zone::Battlefield);g.put(0,4,Zone::Battlefield);g.put(0,5,Zone::Battlefield);for _ in 0..8{g.put(0,3,Zone::Library);}g.put(0,2,Zone::Hand);
    cast(&mut g,target,&[a],"BBB");pass(&mut g);let Some(Decision::Choose{choice:Choice::Replacement{among,..},..})=&g.decision else{panic!("{:?}",g.decision)};let first=among[0].clone();let clone=g.clone();
    for mut g in [g,clone] {g.act(0,Move::Choose(Answer::Replacement(first.clone()))).unwrap();assert_eq!(g.players[0].library.len(),4);let card=g.players[0].hand.iter().copied().find(|o|g.obj(*o).card==Some(3)).unwrap();g.act(0,Move::Choose(Answer::Objects(vec![card]))).unwrap();cite!(["701.50a"]; assert_eq!(g.obj(a).counters.get("+1/+1"),Some(&1)));assert_eq!(g.players[0].library.len(),4);assert_eq!(g.log.iter().filter(|e|matches!(e,game::Event::Connived{..})).count(),1);}
}
#[test]
fn r701_50b_returned_convoker_is_a_new_object_and_receives_no_old_connive_counter() {
    let mut g=Game::new(defs(),3,20);let a=g.put(0,1,Zone::Battlefield);let target=g.put(1,1,Zone::Battlefield);g.put(0,2,Zone::Hand);for _ in 0..3{g.put(0,3,Zone::Library);}cast(&mut g,target,&[a],"BBB");
    g.move_object(a,Zone::Graveyard);let grave=g.players[0].graveyard[0];g.move_object(grave,Zone::Battlefield);let successor=*g.battlefield.last().unwrap();assert_ne!(a,successor);pass(&mut g);
    let card=g.players[0].hand.iter().copied().find(|o|g.obj(*o).card==Some(3)).unwrap();g.act(0,Move::Choose(Answer::Objects(vec![card]))).unwrap();cite!(["400.7"]; assert!(g.obj(successor).counters.is_empty()));cite!(["701.50b"]; assert!(g.log.iter().any(|e|matches!(e,game::Event::Connived{object,..} if *object==a))));
}
#[test]
fn r701_50_creature_type_changes_do_not_remove_retained_convokers() {
    let mut ds=defs();ds.push(CardDef::new("Change","{0}","Instant","Target creature becomes an artifact until end of turn.",None).unwrap());let mut g=Game::new(ds,3,20);let a=g.put(0,1,Zone::Battlefield);let target=g.put(1,1,Zone::Battlefield);g.put(0,2,Zone::Hand);for _ in 0..3{g.put(0,3,Zone::Library);}cast(&mut g,target,&[a],"BBB");let change=g.put(0,4,Zone::Hand);g.act(0,Move::Cast{card:change,targets:vec![vec![Target::Object(a)]]}).unwrap();pass(&mut g);assert!(!g.is_a(a,ir::CardType::Creature));pass(&mut g);
    let card=g.players[0].hand.iter().copied().find(|o|g.obj(*o).card==Some(3)).unwrap();g.act(0,Move::Choose(Answer::Objects(vec![card]))).unwrap();cite!(["701.50a"]; assert_eq!(g.obj(a).counters.get("+1/+1"),Some(&1)));
}
