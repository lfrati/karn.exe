//! Completed destruction -> qualifying old identity/former controller -> one Create.
//! Pinned Terastodon/Mogg text, 608.2f/h, 700.4, 400.7j; creation is 701.7 in the pin.
use crate::support::cite;
use game::{CardDef, Event, Game, Move, Target, Zone};
use game::state::{Answer, Choice, Decision};
const TERASTODON: &str = "When this creature enters, you may destroy up to three target noncreature permanents. For each permanent put into a graveyard this way, its controller creates a 3/3 green Elephant creature token.";
const MOGG: &str = "Destroy all creatures target player controls. For each creature that died this way, that player creates two 1/1 red Goblin creature tokens.";
fn setup(players: usize) -> Game {
    let defs = vec![
        CardDef::new("Terastodon", "{6}{G}{G}", "Creature — Elephant", TERASTODON, Some((9,9))).unwrap(),
        CardDef::new("Mogg Infestation", "{3}{R}{R}", "Sorcery", MOGG, None).unwrap(),
        CardDef::new("Artifact", "{0}", "Artifact", "", None).unwrap(),
        CardDef::new("Shield", "{0}", "Artifact", "Indestructible", None).unwrap(),
        CardDef::new("Replaced", "{0}", "Enchantment", "If this enchantment would die, put it on the bottom of its owner's library instead.", None).unwrap(),
        CardDef::new("Bear", "{0}", "Creature — Bear", "", Some((2,2))).unwrap(),
        CardDef::new("Shield Bear", "{0}", "Creature — Bear", "Indestructible", Some((2,2))).unwrap(),
        CardDef::new("Steal", "{0}", "Instant", "Gain control of target permanent.", None).unwrap(),
    ];
    for d in &defs { game::play::check(d).unwrap_or_else(|e| panic!("{}: {} {:?}",d.name(),e.0,d.abilities())); }
    Game::new(defs,players,20)
}
fn cast(g: &mut Game, index: usize, targets: Vec<Vec<Target>>) {
    let card = g.put(0,index,Zone::Hand); g.players[0].pool.colors = [10;5];
    g.act(0,crate::support::paid(["WWWWWWGG", "WWWRR", "", "", "", "", "", ""][index], Move::Cast { card,targets })).unwrap();
}
fn finish(g: &mut Game, trigger_targets: &[Target], consent: bool) {
    finish_ordered(g,trigger_targets,consent,&[]);
}
fn finish_ordered(g: &mut Game, trigger_targets: &[Target], consent: bool, orders: &[(usize, Vec<game::ObjId>)]) {
    let mut orders=orders.iter();
    for _ in 0..30 {
        match &g.decision {
            Some(Decision::Targets { .. }) => g.act(0,Move::ChooseTargets(vec![trigger_targets.to_vec()])).unwrap(),
            Some(Decision::Choose { player, choice: Choice::May, .. }) => g.act(*player, Move::Choose(if consent {Answer::Yes} else {Answer::No})).unwrap(),
            Some(Decision::Choose {player,choice:Choice::OrderCards{..}}) => {
                let (owner,cards)=orders.next().expect("an authored incoming order");assert_eq!(player,owner);
                cite!(["404.3"]; ok: g.act(*player,Move::Choose(Answer::Objects(cards.clone()))));
            }
            Some(d) => panic!("unexpected {d:?}"),
            None if g.stack.is_empty() => return,
            None => g.act(g.priority,Move::Pass).unwrap(),
        }
    }
    panic!("resolution did not finish");
}
fn tokens(g: &Game, name: &str) -> Vec<usize> {
    let mut counts=vec![0;g.players.len()];
    for o in &g.battlefield {if g.obj(*o).token() && g.obj(*o).chars.name==format!("{name} Token") {counts[g.controller(*o)]+=1;}}
    counts
}
#[test]
fn full_terastodon_allocates_per_controller_and_empty_or_declined_results_create_nothing() {
    for consent in [false,true] {
        let mut g=setup(3); let a=g.put(0,2,Zone::Battlefield); let b=g.put(1,2,Zone::Battlefield); let c=g.put(1,2,Zone::Battlefield);
        cast(&mut g,0,vec![]);finish_ordered(&mut g,&[a.into(),b.into(),c.into()],consent,&[(1,vec![b,c])]);
        cite!(["608.2h", "701.7a"]; case: consent; assert_eq!(tokens(&g,"Elephant"),if consent {vec![1,2,0]} else {vec![0,0,0]}));
        assert_eq!(g.log.iter().filter(|e|matches!(e,Event::Created{..})).count(), if consent {3} else {0});
    }
    let mut g=setup(2);cast(&mut g,0,vec![]);finish(&mut g,&[],true);cite!(["608.2c"]; assert_eq!(tokens(&g,"Elephant"),vec![0,0]));
}
#[test]
fn full_mogg_uses_retained_target_player_and_only_actual_creature_deaths() {
    let mut g=setup(2);let mine=g.put(0,5,Zone::Battlefield);g.put(1,5,Zone::Battlefield);let shield=g.put(1,6,Zone::Battlefield);
    let regenerated=g.put(1,5,Zone::Battlefield);g.regenerate(regenerated);
    let chars=g.obj(mine).chars.clone();g.create_token(1,chars,false);
    cast(&mut g,1,vec![vec![Target::Player(1)]]);finish(&mut g,&[],true);
    cite!(["608.2c", "700.4"]; assert_eq!(tokens(&g,"Goblin"),vec![0,4]));assert!(g.battlefield.contains(&mine));cite!(["702.12b"]; assert!(g.battlefield.contains(&shield)));cite!(["701.19a"]; assert!(g.battlefield.contains(&regenerated)));
}
#[test]
fn destruction_order_suspension_clones_and_delivers_creation_once() {
    let mut g=setup(2);let a=g.put(1,4,Zone::Battlefield);let b=g.put(1,4,Zone::Battlefield);let artifact=g.put(0,2,Zone::Battlefield);
    cast(&mut g,0,vec![]);
    for _ in 0..20 {
        match &g.decision {
            Some(Decision::Targets { .. }) => g.act(0,Move::ChooseTargets(vec![vec![a.into(),b.into(),artifact.into()]])).unwrap(),
            Some(Decision::Choose { choice:Choice::May, .. }) => g.act(0,Move::Choose(Answer::Yes)).unwrap(),
            Some(Decision::Choose { player:1,choice:Choice::OrderCards{..},.. }) => break,
            Some(d)=>panic!("unexpected {d:?}"),
            None=>g.act(g.priority,Move::Pass).unwrap(),
        }
    }
    cite!(["401.4"]; assert!(matches!(g.decision,Some(Decision::Choose{player:1,choice:Choice::OrderCards{..},..}))));
    assert_eq!(tokens(&g,"Elephant"),vec![0,0]);assert!(g.battlefield.contains(&artifact));
    let mut copy=g.clone();for (game,order) in [(&mut g,vec![a,b]),(&mut copy,vec![b,a])] {
        game.retire_events(game.log.end());assert_eq!(game.collect_departed(),0);
        game.act(1,Move::Choose(Answer::Objects(order))).unwrap();finish(game,&[],true);
        cite!(["608.2h", "614.6"]; assert_eq!(tokens(game,"Elephant"),vec![1,0]));cite!(["614.6"]; assert_eq!(game.players[1].library.len(),2));
        assert_eq!(game.log.iter().filter(|e|matches!(e,Event::Destroyed{..})).count(),3);
        assert_eq!(game.log.iter().filter(|e|matches!(e,Event::Created{..})).count(),1);
    }
}

#[test]
fn former_controller_not_owner_and_noncreature_token_death() {
    let mut g=setup(2);let stolen=g.put(1,2,Zone::Battlefield);
    cast(&mut g,7,vec![vec![stolen.into()]]);finish(&mut g,&[],true);
    assert_eq!(g.controller(stolen),0);
    let token=g.create_token(1,g.obj(stolen).chars.clone(),false);
    cast(&mut g,0,vec![]);finish(&mut g,&[stolen.into(),token.into()],true);
    cite!(["608.2h"]; assert_eq!(tokens(&g,"Elephant"),vec![1,1]));
}
#[test]
fn prevented_and_replaced_destructions_do_not_allocate_tokens() {
    let mut g=setup(2);let shield=g.put(1,3,Zone::Battlefield);let regenerated=g.put(1,2,Zone::Battlefield);g.regenerate(regenerated);let replaced=g.put(1,4,Zone::Battlefield);
    cast(&mut g,0,vec![]);finish(&mut g,&[shield.into(),regenerated.into(),replaced.into()],true);
    cite!(["702.12b", "701.19a", "614.6"]; assert_eq!(tokens(&g,"Elephant"),vec![0,0]));assert!(g.battlefield.contains(&shield));assert!(g.battlefield.contains(&regenerated));assert_eq!(g.players[1].library.len(),1);
    let mut g=setup(2);g.put(1,6,Zone::Battlefield);cast(&mut g,1,vec![vec![Target::Player(1)]]);finish(&mut g,&[],true);cite!(["702.12b"]; assert_eq!(tokens(&g,"Goblin"),vec![0,0]));
}
#[test]
fn result_creation_requires_one_completed_destruction_producer() {
    for text in [
        "For each creature that died this way, its controller creates a 1/1 red Goblin creature token.",
        "Destroy target creature. Destroy target artifact. For each permanent put into a graveyard this way, its controller creates a 1/1 red Goblin creature token.",
    ] {
        let d=CardDef::new("Probe","{0}","Sorcery",text,None).unwrap();
        let err=game::play::check(&d).unwrap_err();assert!(err.0.contains("destruction result: absent or ambiguous producer"),"{}",err.0);
    }
}
