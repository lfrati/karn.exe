//! Control changes compose with replacement choices, pre-event trigger views and source LKI.
use crate::support::{cite, paid};
use game::{CardDef, Game, Move, Target, Zone};
use game::state::{Answer, Choice, Decision};
fn card(name: &str, kind: &str, text: &str, pt: Option<(i32,i32)>) -> CardDef {
 let d=CardDef::new(name,"{0}",kind,text,pt).unwrap();game::play::check(&d).unwrap();d
}
fn pass(g:&mut Game){for _ in 0..g.players.iter().filter(|p| !p.lost).count(){g.act(g.priority,Move::Pass).unwrap();}}
fn cast(g:&mut Game,p:usize,def:usize,targets:Vec<Vec<Target>>,spending:&str){let c=g.put(p,def,Zone::Hand);g.priority=p;g.players[p].pool.colors=[10;5];g.act(p,paid(spending,Move::Cast{card:c,targets})).unwrap();pass(g);}
#[test]
fn stolen_creature_counter_replacement_asks_current_controller(){
 let mut g=Game::new(vec![
 card("Steal","Instant","Gain control of target creature.",None),
 card("Bear","Creature — Bear","",Some((2,2))),
 card("Scales","Enchantment","If one or more +1/+1 counters would be put on a creature you control, that many plus one +1/+1 counters are put on it instead.",None),
 card("Evolution","Enchantment","If one or more +1/+1 counters would be put on a creature you control, twice that many +1/+1 counters are put on that creature instead.",None),
 card("Grow","Instant","Put a +1/+1 counter on target creature.",None)],2,20);
 let b=g.put(1,1,Zone::Battlefield);g.put(0,2,Zone::Battlefield);g.put(0,3,Zone::Battlefield);
 cast(&mut g,0,0,vec![vec![Target::Object(b)]], "");assert_eq!(g.controller(b),0);
 cast(&mut g,0,4,vec![vec![Target::Object(b)]], "");
 let Some(Decision::Choose { player: 0, choice: Choice::Replacement { among, .. } }) = &g.decision else { panic!("{:?}", g.decision) };
 let scales = among.iter().find(|effect| g.obj(effect.source).card == Some(2)).unwrap().clone();
 cite!(["616.1"]; refused("that decision is another player's"): g.clone().act(1, Move::Choose(Answer::Replacement(scales.clone()))));
 for mut branch in [g.clone(), g] {
     branch.act(0, Move::Choose(Answer::Replacement(scales.clone()))).unwrap();
     cite!(["616.1","616.1f"]; assert_eq!(branch.obj(b).counters.get("+1/+1"), Some(&4)));
 }
}
fn thief() -> CardDef {let faces=oracle::corpus::load(&oracle::corpus::default_path()).unwrap();let d=CardDef::from_face(faces.iter().find(|f|f.name=="Master Thief").unwrap()).unwrap();game::play::check(&d).unwrap();d}
#[test]
fn returned_watcher_uses_predeparture_controller(){
 let mut g=Game::new(vec![thief(),card("Watcher","Artifact","Whenever a creature you control dies, you gain 1 life.",None),card("Kill","Instant","Destroy target creature.",None)],2,20);
 let relic=g.put(1,1,Zone::Battlefield);cast(&mut g,0,0,vec![], "UUUU");
 let source=*g.battlefield.iter().find(|o|g.obj(**o).card==Some(0)).unwrap();
 g.act(0,Move::ChooseTargets(vec![vec![Target::Object(relic)]])).unwrap();pass(&mut g);assert_eq!(g.controller(relic),0);
 cast(&mut g,0,2,vec![vec![Target::Object(source)]], "");assert_eq!(g.controller(relic),1);pass(&mut g);
 cite!(["603.10a","603.3a"]; assert_eq!((g.players[0].life,g.players[1].life),(21,20)));
}
#[test]
fn departed_owner_preserves_stolen_damage_sources_controller(){
 let mut g=Game::new(vec![card("Steal","Instant","Gain control of target creature.",None),
 card("Pinger","Creature — Human","Lifelink\n{0}: This creature deals 1 damage to target player.",Some((2,2)))],3,20);
 let source=g.put(1,1,Zone::Battlefield);cast(&mut g,0,0,vec![vec![Target::Object(source)]], "");
 g.act(0,Move::Activate{source,ability:1,targets:vec![vec![Target::Player(2)]],paying:vec![]}).unwrap();
 g.players[1].life=0;g.act(g.priority,Move::Pass).unwrap();assert!(g.players[1].lost);
 while !g.stack.is_empty(){g.act(g.priority,Move::Pass).unwrap();}
 cite!(["113.7a"]; assert_eq!(g.players[2].life,19));
 cite!(["702.15b","113.7a"]; assert_eq!(g.players[0].life,21,"lifelink uses the departed source’s last controller"));
}

#[test]
fn surviving_watcher_keeps_its_predeparture_granted_ability() {
    let mut g = Game::new(vec![
        card("Giver", "Creature — Human", "Other creatures you control have \"Whenever a creature dies, you gain 1 life.\"", Some((2, 2))),
        card("Bear", "Creature — Bear", "", Some((2, 2))),
        card("Kill", "Instant", "Destroy target creature.", None),
    ], 2, 20);
    let giver = g.put(0, 0, Zone::Battlefield);
    g.put(0, 1, Zone::Battlefield);
    cast(&mut g, 0, 2, vec![vec![Target::Object(giver)]], "");
    cite!(["603.10a"]; assert_eq!(g.stack.len(), 1, "the surviving bear had the death watcher before the departure"));
    for mut branch in [g.clone(), g] {
        branch.retire_events(branch.log.end());
        branch.collect_departed();
        pass(&mut branch);
        cite!(["113.7a"]; assert_eq!(branch.players[0].life, 21, "the captured grant survives source departure and cloning"));
    }
}

#[test]
fn surviving_watcher_does_not_retroactively_regain_its_death_ability() {
    let mut g = Game::new(vec![
        card("Suppressor", "Creature — Human", "Other creatures lose all abilities.", Some((2, 2))),
        card("Watcher", "Creature — Bear", "Whenever a creature dies, you gain 1 life.", Some((2, 2))),
        card("Kill", "Instant", "Destroy target creature.", None),
    ], 2, 20);
    let suppressor = g.put(0, 0, Zone::Battlefield);
    g.put(0, 1, Zone::Battlefield);
    cast(&mut g, 0, 2, vec![vec![Target::Object(suppressor)]], "");
    cite!(["603.10a"]; assert!(g.stack.is_empty(), "the watcher had no abilities immediately before the death"));
}

#[test]
fn intervening_condition_counts_the_predeparture_battlefield_only_when_triggering() {
    let mut g = Game::new(vec![
        card("Watcher", "Artifact", "Whenever a creature dies, if you control two or more creatures, you gain 1 life.", None),
        card("Bear", "Creature — Bear", "", Some((2, 2))),
        card("Kill", "Instant", "Destroy target creature.", None),
    ], 2, 20);
    g.put(0, 0, Zone::Battlefield);
    let bear = g.put(0, 1, Zone::Battlefield);
    g.put(0, 1, Zone::Battlefield);
    cast(&mut g, 0, 2, vec![vec![Target::Object(bear)]], "");
    cite!(["603.4","603.10a"]; assert_eq!(g.stack.len(), 1, "both creatures existed when the look-back condition was checked"));
    let mut restored = g.clone();
    restored.put(0, 1, Zone::Battlefield);
    pass(&mut g);
    pass(&mut restored);
    cite!(["603.4"]; assert_eq!(g.players[0].life, 20, "resolution checks the current battlefield"));
    cite!(["603.4"]; assert_eq!(restored.players[0].life, 21));
}
