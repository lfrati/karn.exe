//! Layer-two control composes with later layers, captured occurrences and turn rules.
use crate::support::{cite, paid};
use game::{CardDef, Game, Move, ObjId, Target, Zone};
fn card(name: &str, kind: &str, text: &str, pt: Option<(i32,i32)>) -> CardDef {
    let d = CardDef::new(name,"{0}",kind,text,pt).unwrap(); game::play::check(&d).unwrap(); d
}
fn pass(g: &mut Game) { for _ in 0..g.players.iter().filter(|p| !p.lost).count() { g.act(g.priority,Move::Pass).unwrap(); } }
fn cast(g: &mut Game, p: usize, def: usize, to: ObjId, spending: &str) {
    let card = g.put(p,def,Zone::Hand); g.priority=p; g.players[p].pool.colors=[10;5];
    g.act(p, paid(spending, Move::Cast {card, targets:vec![vec![Target::Object(to)]]})).unwrap(); pass(g);
}
fn bear() -> CardDef { card("Bear","Creature — Bear","",Some((2,2))) }
fn steal(temporary:bool) -> CardDef { card("Control","Instant",if temporary {"Gain control of target creature until end of turn."} else {"Gain control of target creature."},None) }
#[test]
fn r613_1b_full_act_of_aggression_changes_control_untaps_and_gives_haste() {
    let faces=oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let d=CardDef::from_face(faces.iter().find(|f| f.name=="Act of Aggression").unwrap()).unwrap();
    game::play::check(&d).unwrap();
    let mut g=Game::new(vec![d,bear()],2,20); let b=g.put(1,1,Zone::Battlefield); g.tap(b);
    cast(&mut g,0,0,b, "WWWRR");
    cite!(["613.1b"]; assert_eq!(g.controller(b),0)); assert_eq!(g.obj(b).owner,1); assert!(!g.obj(b).tapped);
    assert!(g.has(b,"Haste")); cite!(["702.10b"]; assert!(g.can_attack(b).is_ok()));
    while g.active==0 {
        if matches!(g.decision,Some(game::state::Decision::Attackers { .. })) { g.act(g.active,Move::Attack(vec![])).unwrap(); }
        else { g.act(g.priority,Move::Pass).unwrap(); }
    }
    cite!(["514.2"]; assert_eq!(g.controller(b),1)); cite!(["514.2"]; assert!(!g.has(b,"Haste"))); cite!(["302.6"]; assert!(!g.summoning_sick(b)));
}
#[test]
fn r613_1b_current_controller_feeds_later_layer_subjects_without_recursion() {
    let lord=card("Lord","Creature — Bear","Creatures you control get +1/+1.",Some((2,2)));
    let mut g=Game::new(vec![steal(false),lord,bear()],2,20);
    let lord=g.put(1,1,Zone::Battlefield); let theirs=g.put(1,2,Zone::Battlefield); let ours=g.put(0,2,Zone::Battlefield);
    assert_eq!(g.power(theirs),Some(3)); assert_eq!(g.power(ours),Some(2));
    cast(&mut g,0,0,lord, "");
    cite!(["613.1b"]; assert_eq!(g.power(theirs),Some(2))); cite!(["613.1b"]; assert_eq!(g.power(ours),Some(3))); assert_eq!(g.power(lord),Some(3));
}
#[test]
fn r113_7a_control_changes_do_not_change_an_already_created_abilitys_controller() {
    let d=card("Speaker","Creature — Bear","{0}: You gain 1 life.\nWhen this creature dies, you gain 2 life.",Some((2,2)));
    let kill=card("Kill","Instant","Destroy target creature.",None);
    let mut g=Game::new(vec![steal(false),d,kill],2,20); let b=g.put(1,1,Zone::Battlefield);
    g.priority=1; g.act(1,Move::Activate {source:b,ability:0,targets:vec![],paying:vec![]}).unwrap();
    cast(&mut g,0,0,b, ""); assert_eq!(g.controller(b),0); pass(&mut g);
    cite!(["113.8"]; assert_eq!(g.players[1].life,21)); assert_eq!(g.players[0].life,20);
    let mut asked=g.clone(); asked.priority=1;
    cite!(["602.2"]; refused("only an object's controller can activate its ability (602.2)"): asked.act(1,Move::Activate {source:b,ability:0,targets:vec![],paying:vec![]}));
    cast(&mut g,0,2,b, ""); pass(&mut g);
    cite!(["603.3a","603.10a"]; assert_eq!(g.players[0].life,22)); assert_eq!(g.controller(b),0); // departed incarnation LKI
    assert_eq!(g.players[1].graveyard.len(),1);
}
#[test]
fn r613_7_later_control_wins_and_expiration_restores_the_previous_effect() {
    let mut g=Game::new(vec![steal(false),steal(true),bear()],3,20); let b=g.put(2,2,Zone::Battlefield);
    cast(&mut g,0,0,b, ""); cast(&mut g,1,1,b, "");
    cite!(["613.7"]; assert_eq!(g.controller(b),1));
    let cloned=g.clone();
    for mut g in [g,cloned] {
        while g.active==0 {
        if matches!(g.decision,Some(game::state::Decision::Attackers { .. })) { g.act(g.active,Move::Attack(vec![])).unwrap(); }
        else { g.act(g.priority,Move::Pass).unwrap(); }
    }
        cite!(["514.2","613.7"]; assert_eq!(g.controller(b),0)); assert_eq!(g.obj(b).owner,2);
    }
}
#[test]
fn r506_4_a_control_change_removes_combatants_and_resets_control_duration() {
    let mut g=Game::new(vec![steal(false),bear()],2,20); let b=g.put(1,1,Zone::Battlefield);
    g.combat.attackers.push((b,Target::Player(0)));
    cast(&mut g,0,0,b, "");
    cite!(["506.4"]; assert!(!g.combat.attacking(b))); assert!(g.summoning_sick(b)); cite!(["302.6"]; refused("that creature has not been under your control since your turn began (302.6)"): g.can_attack(b));
}
#[test]
fn r800_4a_departure_ends_effects_giving_the_loser_control_before_exiling_objects() {
    let mut g=Game::new(vec![steal(false),bear()],3,20); let b=g.put(2,1,Zone::Battlefield);
    cast(&mut g,1,0,b, ""); assert_eq!(g.controller(b),1);
    g.players[1].life=0; g.act(g.priority,Move::Pass).unwrap();
    assert!(g.players[1].lost); cite!(["800.4a"]; assert_eq!(g.obj(b).zone,Zone::Battlefield)); cite!(["800.4a"]; assert_eq!(g.controller(b),2));
}
fn thief() -> CardDef {
    let faces=oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let d=CardDef::from_face(faces.iter().find(|f| f.name=="Master Thief").unwrap()).unwrap();
    game::play::check(&d).unwrap_or_else(|e|panic!("{}: {:?}",e.0,d.abilities()));d
}
fn theft() -> (Game,ObjId,ObjId) {
    let mut g=Game::new(vec![thief(),card("Relic","Artifact","{T}: Add {G}.",None),steal(true)],3,20);
    let relic=g.put(1,1,Zone::Battlefield);let spell=g.put(0,0,Zone::Hand);g.players[0].pool.colors=[10;5];
    g.act(0,paid("WWUU", Move::Cast {card:spell,targets:vec![]})).unwrap();pass(&mut g);
    let source=*g.battlefield.iter().find(|o|g.obj(**o).card==Some(0)).unwrap();
    g.act(0,Move::ChooseTargets(vec![vec![Target::Object(relic)]])).unwrap();
    (g,source,relic)
}
#[test]
fn r611_2b_full_master_thief_ends_on_source_departure_and_does_not_follow_a_new_incarnation() {
    let (mut g,source,relic)=theft();pass(&mut g);assert_eq!(g.controller(relic),0);
    g.act(0,Move::Activate { source: relic, ability: 0, targets: vec![], paying: vec![] }).unwrap();
    g.exile(source);cite!(["611.2a"]; assert_eq!(g.controller(relic),1));
    g.put(0,0,Zone::Battlefield);cite!(["400.7"]; assert_eq!(g.controller(relic),1));
    let clone=g.clone();assert_eq!(clone.controller(relic),1);
}
#[test]
fn r611_2b_a_duration_that_ended_before_resolution_never_begins() {
    let (mut g,source,relic)=theft();g.exile(source);pass(&mut g);cite!(["611.2b"]; assert_eq!(g.controller(relic),1));
}
#[test]
fn r611_2b_losing_source_control_ends_the_effect_permanently_even_if_control_returns() {
    let (mut g,source,relic)=theft();pass(&mut g);assert_eq!(g.controller(relic),0);
    cast(&mut g,2,2,source, "");assert_eq!(g.controller(source),2);cite!(["611.2a"]; assert_eq!(g.controller(relic),1));
    while g.active==0 {g.act(g.priority,Move::Pass).unwrap();}
    assert_eq!(g.controller(source),0);assert_eq!(g.controller(relic),1);
}

#[test]
fn r613_1b_permanent_control_support_does_not_admit_spell_control() {
    let d=CardDef::new("Spell control","{0}","Instant","Gain control of target spell.",None).unwrap();
    crate::support::refused(game::play::check(&d).map_err(|e|e.0),"gain control: only permanents, not spell occurrences");
}
