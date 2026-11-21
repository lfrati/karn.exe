//! Rebound replaces successful resolution only, retaining the exile incarnation for one upkeep.
use crate::support::cite;
use game::{CardDef,Game,Move,Zone,ObjId,Target,Event};
use game::state::{Answer,Choice,Decision};
use ir::Part;
fn full(name:&str)->CardDef {let fs=oracle::corpus::load(&oracle::corpus::default_path()).unwrap();let cat=CardDef::catalog(&fs);let d=CardDef::from_faces(&cat[name]).unwrap();game::play::check(&d).unwrap_or_else(|e|panic!("{name}: {}",e.0));d}
fn setup()->Game {let mut g=Game::new(vec![full("Transpose"),CardDef::new("Land","","Basic Land — Island","",None).unwrap(),CardDef::new("Fork","{0}","Instant","Copy target spell.",None).unwrap(),CardDef::new("Counter","{0}","Instant","Counter target spell.",None).unwrap(),full("Teferi, Time Raveler"),CardDef::new("Offer","{0}","Instant","Exile target card from a graveyard. You may cast that card without paying its mana cost.",None).unwrap()],3,20);for p in 0..3{for _ in 0..16{g.put(p,1,Zone::Library);}}g}
fn stocked(defs:Vec<CardDef>)->Game {let mut g=Game::new(defs,3,20);for p in 0..3{for _ in 0..12{g.put(p,0,Zone::Library);}}g}
fn pass(g:&mut Game){for _ in 0..g.players.len(){g.act(g.priority,Move::Pass).unwrap();}}
fn cast(g:&mut Game)->ObjId {let card=g.put(0,0,Zone::Hand);g.put(0,1,Zone::Hand);g.players[0].pool.colors[2]=3;g.act(0,crate::support::paid("BBB",Move::Cast{card,targets:vec![]})).unwrap();g.stack[0]}
fn discard(g:&mut Game) {let Some(Decision::Choose{player,choice:Choice::Objects{verb:ir::Verb::Discard,among,..}})=&g.decision else{panic!("{:?}",g.decision)};let player=*player;let card=among[0];g.act(player,Move::Choose(Answer::Objects(vec![card]))).unwrap();}
fn upkeep(g:&mut Game){let turn=g.turn;for _ in 0..160{if g.turn>turn&&g.active==0&&g.part==Part::Upkeep{return;}assert!(g.stack.is_empty(),"unexpected trigger before controller upkeep");match g.decision {Some(Decision::Attackers{player})=>g.act(player,Move::Attack(vec![])).unwrap(),Some(Decision::Blockers{player})=>g.act(player,Move::Block(vec![])).unwrap(),_=>{g.act(g.priority,Move::Pass).unwrap();}}}panic!("no upkeep");}
fn exiled(g:&Game)->ObjId {*g.exile.iter().find(|o|g.obj(**o).card==Some(0)).unwrap()}
#[test]
fn r702_88a_full_transpose_retains_private_discard_and_hand_only_token_then_free_casts_once() {
    let mut g=setup();let source=cast(&mut g);pass(&mut g);let copy=g.clone();
    for mut g in [g,copy] {discard(&mut g);assert_eq!(g.players[0].life,19);cite!(["702.88a"]; assert_eq!(g.log.iter().filter(|e|matches!(e,Event::Exiled{object} if *object==source)).count(),1));assert_eq!(g.battlefield.len(),1);let token=g.battlefield[0];assert_eq!(g.obj(token).chars.pt,Some((0,1)));let card=exiled(&g);
        upkeep(&mut g);assert_eq!(g.stack.len(),1);cite!(["702.88a","603.7f"]; assert_eq!(g.obj(g.stack[0]).ability.unwrap().0,Some(source)));pass(&mut g);cite!(["702.88a"]; assert!(matches!(g.decision,Some(Decision::Choose{player:0,choice:Choice::Cast{..}}))));
        crate::support::refused(g.act(1,Move::Cast{card,targets:vec![]}), "that decision is another player's");g.act(0,Move::Cast{card,targets:vec![]}).unwrap();assert_eq!(g.stack.len(),2,"Wizard trigger above the free spell");assert_eq!(g.players[0].pool.total(),0);
        pass(&mut g);assert_eq!((g.players[1].life,g.players[2].life),(19,19));pass(&mut g);discard(&mut g);assert_eq!(g.players[0].life,18);assert_eq!(g.battlefield.len(),1);assert!(g.exile.is_empty());cite!(["702.88a","608.2n"]; assert!(g.players[0].graveyard.iter().any(|o|g.obj(*o).card==Some(0))));
    }
}
#[test]
fn r702_88a_countered_transpose_never_draws_creates_or_exiles() {
    let mut g=setup();let source=cast(&mut g);let library=g.players[0].library.len();let counter=g.put(0,3,Zone::Hand);g.act(0,Move::Cast{card:counter,targets:vec![vec![Target::Object(source)]]}).unwrap();pass(&mut g);assert_eq!(g.players[0].library.len(),library);assert_eq!(g.players[0].life,20);cite!(["701.6a","702.88a"]; assert!(g.battlefield.is_empty()&&g.exile.is_empty()));
}
#[test]
fn r707_10_transpose_copy_has_no_hand_origin_or_rebound() {
    let mut g=setup();let source=cast(&mut g);let fork=g.put(0,2,Zone::Hand);g.act(0,Move::Cast{card:fork,targets:vec![vec![Target::Object(source)]]}).unwrap();pass(&mut g);pass(&mut g);discard(&mut g);cite!(["707.10","702.88a"]; assert!(g.battlefield.is_empty()&&g.exile.is_empty()));assert_eq!(g.players[0].life,19);pass(&mut g);discard(&mut g);assert_eq!(g.battlefield.len(),1);
}
#[test]
fn r603_7c_departed_then_returned_exile_card_is_not_the_rebound_incarnation() {
    let mut g=setup();cast(&mut g);pass(&mut g);discard(&mut g);let old=exiled(&g);g.move_object(old,Zone::Graveyard);let grave=*g.players[0].graveyard.iter().find(|o|g.obj(**o).card==Some(0)).unwrap();g.move_object(grave,Zone::Exile);let new=exiled(&g);assert_ne!(old,new);
    upkeep(&mut g);pass(&mut g);cite!(["603.7c","400.7"]; assert!(g.decision.is_none()));assert_eq!(g.obj(new).zone,Zone::Exile);
}
#[test]
fn r702_88a_decline_consumes_only_attempt_and_explicit_later_cast_has_no_permission() {
    let mut g=setup();cast(&mut g);pass(&mut g);discard(&mut g);let card=exiled(&g);upkeep(&mut g);pass(&mut g);g.act(0,Move::Choose(Answer::No)).unwrap();cite!(["702.88a"]; assert_eq!(g.obj(card).zone,Zone::Exile));cite!(["702.88a","601.3"]; refused("that card is not in your hand: it is in exile, and no rule or effect lets you cast it from there (601.3)"): g.act(0,Move::Cast{card,targets:vec![]}));upkeep(&mut g);cite!(["603.7b"]; assert!(g.stack.is_empty()));
}
#[test]
fn r101_2_teferi_restriction_refuses_the_rebound_cast_without_revoking_exile() {
    let mut g=setup();cast(&mut g);pass(&mut g);discard(&mut g);let teferi=g.put(1,4,Zone::Battlefield);g.put_counters(teferi,"loyalty",4);let card=exiled(&g);upkeep(&mut g);pass(&mut g);let before=format!("{g:?}");cite!(["601.3"]; refused("an effect permits casting spells only at sorcery timing (307.5, 601.3)"): g.act(0,Move::Cast{card,targets:vec![]}));assert_eq!(format!("{g:?}"),before);g.act(0,Move::Choose(Answer::No)).unwrap();cite!(["702.88a"]; assert_eq!(g.obj(card).zone,Zone::Exile));
}
#[test]
fn r702_88a_fizzled_targeted_spell_has_no_rebound() {
    let d=CardDef::new("Echo spell","{0}","Instant","Destroy target creature.\nRebound",None).unwrap();let bear=CardDef::new("Bear","{G}","Creature","",Some((2,2))).unwrap();let mut g=stocked(vec![d,bear]);let target=g.put(1,1,Zone::Battlefield);let card=g.put(0,0,Zone::Hand);g.act(0,Move::Cast{card,targets:vec![vec![Target::Object(target)]]}).unwrap();g.move_object(target,Zone::Graveyard);pass(&mut g);cite!(["608.2b","702.88a"]; assert!(g.exile.is_empty()));upkeep(&mut g);assert!(g.stack.is_empty());
}
#[test]
fn r702_88a_exile_origin_transpose_has_no_hand_token_or_further_rebound() {
    let mut g=setup();let transpose=g.put(0,0,Zone::Graveyard);g.put(0,1,Zone::Hand);let bridge=g.put(0,5,Zone::Hand);g.act(0,Move::Cast{card:bridge,targets:vec![vec![Target::Object(transpose)]]}).unwrap();pass(&mut g);let card=exiled(&g);g.act(0,Move::Cast{card,targets:vec![]}).unwrap();pass(&mut g);discard(&mut g);assert_eq!(g.players[0].life,19);cite!(["702.88a"]; assert!(g.battlefield.is_empty()&&g.exile.is_empty()));upkeep(&mut g);assert!(g.stack.is_empty());
}
#[test]
fn r702_88b_free_cast_x_is_zero_and_additional_costs_are_still_required() {
    for additional in [false,true] {
        let d=if additional{CardDef::new("Echo spell","{0}","Instant","As an additional cost to cast this spell, sacrifice a creature.\nYou gain 1 life.\nRebound",None).unwrap()}else{CardDef::new("Echo spell","{X}","Instant","You draw X cards.\nRebound",None).unwrap()};game::play::check(&d).unwrap();
        let mut g=stocked(vec![d,CardDef::new("Bear","{G}","Creature","",Some((2,2))).unwrap()]);let card=g.put(0,0,Zone::Hand);let cast=Move::Cast{card,targets:vec![]};let cast=if additional{let bear=g.put(0,1,Zone::Battlefield);Move::WithCosts(vec![vec![bear]],Box::new(cast))}else{Move::WithX(0,Box::new(cast))};g.act(0,cast).unwrap();pass(&mut g);let card=exiled(&g);upkeep(&mut g);pass(&mut g);
        if additional{cite!(["118.9d"]; refused("that selection does not satisfy the cost, which asks for exactly 1 card(s): 0 chosen (601.2h, 701.21a)"): g.act(0,Move::Cast{card,targets:vec![]}));}else{cite!(["107.3b"]; refused("X is 0 for a spell cast without paying its mana cost (107.3b)"): g.act(0,Move::WithX(1,Box::new(Move::Cast{card,targets:vec![]}))));}
        if additional{let bear=g.put(0,1,Zone::Battlefield);cite!(["702.88b","118.9d"]; ok: g.act(0,Move::WithCosts(vec![vec![bear]],Box::new(Move::Cast{card,targets:vec![]}))));assert_eq!(g.obj(bear).zone,Zone::Gone);}else{cite!(["107.3b"]; ok: g.act(0,Move::WithX(0,Box::new(Move::Cast{card,targets:vec![]}))));}
        pass(&mut g);assert!(g.exile.is_empty());
    }
}
#[test]
fn r702_88a_no_legal_targets_leaves_the_exiled_card_without_another_attempt() {
    let d=CardDef::new("Echo spell","{0}","Instant","Destroy target creature.\nRebound",None).unwrap();let bear=CardDef::new("Bear","{G}","Creature","",Some((2,2))).unwrap();let mut g=stocked(vec![d,bear]);let target=g.put(1,1,Zone::Battlefield);let card=g.put(0,0,Zone::Hand);g.act(0,Move::Cast{card,targets:vec![vec![Target::Object(target)]]}).unwrap();pass(&mut g);let card=exiled(&g);upkeep(&mut g);pass(&mut g);cite!(["601.2c","115.1"]; refused("an illegal target, Bear: it isn't a permanent on the battlefield (115.1)"): g.act(0,Move::Cast{card,targets:vec![vec![Target::Object(target)]]}));crate::support::refused(g.act(0,Move::Cast{card,targets:vec![]}), "that has 1 target group(s), 0 given");g.act(0,Move::Choose(Answer::No)).unwrap();assert_eq!(g.obj(card).zone,Zone::Exile);upkeep(&mut g);cite!(["603.7b"]; assert!(g.stack.is_empty()));
}
#[test]
fn r608_2m_spell_moved_by_its_own_instructions_gets_no_rebound_replacement() {
    let d=CardDef::new("Echo spell","{0}","Instant","Exile this spell. You gain 1 life.\nRebound",None).unwrap();let mut g=stocked(vec![d]);let card=g.put(0,0,Zone::Hand);g.act(0,Move::Cast{card,targets:vec![]}).unwrap();pass(&mut g);cite!(["608.2m"]; assert_eq!(g.players[0].life,21));assert_eq!(g.exile.len(),1);upkeep(&mut g);cite!(["702.88a"]; assert!(g.stack.is_empty()));
}
#[test]
fn r603_7a_hand_cast_during_own_upkeep_waits_for_the_next_upkeep() {
    let mut g=setup();upkeep(&mut g);cast(&mut g);pass(&mut g);discard(&mut g);cite!(["603.7a"]; assert!(g.stack.is_empty()));upkeep(&mut g);cite!(["702.88a"]; assert_eq!(g.stack.len(),1));let clone=g.clone();
    for mut g in [g,clone]{pass(&mut g);let clone=g.clone();for mut g in [g,clone]{g.act(0,Move::Choose(Answer::No)).unwrap();assert_eq!(g.exile.len(),1);}}
}
#[test]
fn r702_88c_multiple_rebound_instances_are_redundant() {
    let d=CardDef::new("Echo spell","{0}","Instant","You gain 1 life.\nRebound\nRebound",None).unwrap();let mut g=stocked(vec![d]);let card=g.put(0,0,Zone::Hand);g.act(0,Move::Cast{card,targets:vec![]}).unwrap();pass(&mut g);assert_eq!(g.exile.len(),1);upkeep(&mut g);cite!(["702.88c"]; assert_eq!(g.stack.len(),1));pass(&mut g);g.act(0,Move::Choose(Answer::No)).unwrap();assert!(g.stack.is_empty());
}

#[test]
fn r702_88a_layered_rebound_grant_exiles_and_retains_its_upkeep_cast_after_grant_removal() {
    let echo = CardDef::new("Echo spell", "{0}", "Instant", "You gain 1 life.", None).unwrap();
    game::play::check(&echo).unwrap();
    let mut g = stocked(vec![echo, full("Cast Through Time")]);
    let grant = g.put(0, 1, Zone::Battlefield);
    let card = g.put(0, 0, Zone::Hand);
    g.act(0, Move::Cast { card, targets: vec![] }).unwrap();
    let spell = g.stack[0];
    cite!(["613.1f"]; assert!(g.has(spell, "Rebound"), "the layered grant applies to the stack spell"));
    let copy = g.clone();
    for mut g in [g, copy] {
        pass(&mut g);
        assert_eq!(g.players[0].life, 21);
        let card = exiled(&g);
        g.destroy(grant);
        upkeep(&mut g);
        assert_eq!(g.stack.len(), 1, "the delayed cast survives loss of the grant");
        pass(&mut g);
        g.act(0, Move::Cast { card, targets: vec![] }).unwrap();
        assert!(!g.has(g.stack[0], "Rebound"));
        pass(&mut g);
        assert_eq!(g.players[0].life, 22);
        cite!(["702.88a"]; assert!(g.exile.is_empty()));
    }
}
/// The rebound cast's prompt says the delayed ability rebound created (702.88a), not the
/// spell's first line (Ephemerate). That delayed triggered ability
/// is what triggered, so on the stack it "has the text of the ability that created it"
/// (603.3, 405.4): its own "at the beginning of your next upkeep" sentence. The rebound
/// keyword, whose replacement made it (603.7a), is said as what created it.
#[test]
fn r702_88a_the_rebound_cast_prompt_names_rebounds_delayed_ability() {
    let mut g=setup();cast(&mut g);pass(&mut g);discard(&mut g);upkeep(&mut g);
    let rebound="At the beginning of your next upkeep, you may cast this card from exile without paying its mana cost.";
    let mut s=game::simulation::Simulation::new(g.clone());let p=s.prompt(0).unwrap();
    let on_stack=p.entities[p.view.stack.last().unwrap()].occurrence.as_ref().unwrap();
    cite!(["603.3","702.88a"]; assert_eq!(on_stack.text.as_deref(),Some(rebound)));
    cite!(["603.7a","702.88a"]; assert_eq!(on_stack.created_by.as_deref(),Some("Rebound")));
    pass(&mut g);
    let mut s=game::simulation::Simulation::new(g);let p=s.prompt(0).unwrap();
    let asked=p.decision.as_ref().unwrap().occurrence.as_ref().unwrap();
    cite!(["603.3","702.88a"]; assert_eq!(asked.text.as_deref(),Some(rebound)));
    cite!(["603.7a","702.88a"]; assert_eq!(asked.created_by.as_deref(),Some("Rebound")));
}
/// Rebound an effect gave the spell (Cast Through Time: "Instant and sorcery spells you
/// control have rebound", 613.1f) is the ability whose replacement created the delayed one
/// (702.88a, 603.7a), as printed rebound is: the prompt says "Rebound" created it, though
/// no definition prints it, and still once the grant is gone, since the delayed ability
/// exists independently of it (113.7a, 603.7c).
#[test]
fn r702_88a_a_granted_rebounds_delayed_ability_says_rebound_created_it() {
    let echo = CardDef::new("Echo spell", "{0}", "Instant", "You gain 1 life.", None).unwrap();
    game::play::check(&echo).unwrap();
    let mut g = stocked(vec![echo, full("Cast Through Time")]);
    let grant = g.put(0, 1, Zone::Battlefield);
    let card = g.put(0, 0, Zone::Hand);
    g.act(0, Move::Cast { card, targets: vec![] }).unwrap();
    pass(&mut g);
    g.destroy(grant);
    upkeep(&mut g);
    let rebound = "At the beginning of your next upkeep, you may cast this card from exile without paying its mana cost.";
    for seat in 0..g.players.len() {
        let p = game::simulation::Simulation::new(g.clone()).prompt(seat).unwrap();
        let on_stack = p.entities[p.view.stack.last().unwrap()].occurrence.as_ref().unwrap();
        cite!(["603.3","702.88a"]; assert_eq!(on_stack.text.as_deref(), Some(rebound), "p{seat}"));
        cite!(["603.7a","702.88a","613.1f"]; assert_eq!(on_stack.created_by.as_deref(), Some("Rebound"), "p{seat}"));
    }
    pass(&mut g);
    let p = game::simulation::Simulation::new(g).prompt(0).unwrap();
    let asked = p.decision.as_ref().unwrap().occurrence.as_ref().unwrap();
    cite!(["603.3","702.88a"]; assert_eq!(asked.text.as_deref(), Some(rebound)));
    cite!(["603.7a","702.88a","613.1f"]; assert_eq!(asked.created_by.as_deref(), Some("Rebound")));
}
