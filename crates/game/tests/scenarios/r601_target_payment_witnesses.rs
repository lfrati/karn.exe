//! Announced targets determine the evidence cost before mana abilities are paid (601.2c/f/g).
use crate::support::{cite,paid};
use game::{CardDef, Game, Move, ObjId, Target, Zone};
use game::legal::Step;
fn game()->Game {
    let faces=oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let urgent=CardDef::from_face(faces.iter().find(|f|f.name=="Urgent Necropsy").unwrap()).unwrap();
    let card=|n,c,t,s,pt|CardDef::new(n,c,t,s,pt).unwrap();
    let defs=vec![urgent,
        card("Resource","{4}","Artifact Creature — Bear","Sacrifice this creature: Add {C}.",Some((4,4))),
        card("Four","{4}","Creature — Bear","",Some((4,4))),
        card("Two","{2}","Enchantment","",None),
        card("Zero","{0}","Artifact Creature — Thopter","",Some((1,1)))];
    for d in &defs {game::play::check(d).unwrap_or_else(|e|panic!("{}: {}",d.name(),e.0));}
    let mut g=Game::new(defs,2,20);g.players[0].pool.colors[2]=2;g.players[0].pool.colors[4]=1;g.players[0].pool.colorless=1;g
}
fn cast(spell: ObjId, targets: Vec<Vec<Target>>, evidence: Vec<ObjId>) -> Move {
    Move::WithCosts(vec![evidence], Box::new(paid("CBBG",Move::Cast { card: spell, targets })))
}
fn pass(g: &mut Game) { for _ in 0..2 { g.act(g.priority, Move::Pass).unwrap(); } }
#[test]
fn r701_59b_positive_targets_need_evidence_but_zero_and_no_targets_do_not() {
    let mut g=game(); let positive=g.put(1,1,Zone::Battlefield); let zero=g.put(1,4,Zone::Battlefield); let spell=g.put(0,0,Zone::Hand);
    let targets=|object|vec![vec![Target::Object(object)],vec![],vec![],vec![]];
    cite!(["701.59b", "118.3"]; refused("the chosen cards have total mana value less than 4"): act_unchanged(&mut g,cast(spell,targets(positive),vec![])));
    let mut zero_target=g.clone();
    cite!(["701.59a", "601.2c"]; ok: zero_target.act(0,cast(spell,targets(zero),vec![])));
    pass(&mut zero_target);
    cite!(["608.2c"]; assert!(!zero_target.battlefield.contains(&zero) && zero_target.battlefield.contains(&positive)));
    assert!(zero_target.exile.is_empty() && zero_target.stack.is_empty());
    cite!(["601.2c"]; ok: g.act(0,cast(spell,vec![vec![];4],vec![])));
    pass(&mut g);
    cite!(["608.2c"]; assert!(g.battlefield.contains(&zero) && g.battlefield.contains(&positive)));
    assert!(g.exile.is_empty() && g.stack.is_empty());
}
#[test]
fn r601_2f_evidence_uses_the_total_of_distinct_targeted_permanents() {
    let mut g=game(); let resource=g.put(1,1,Zone::Battlefield); let enchantment=g.put(1,3,Zone::Battlefield);
    let four=g.put(0,2,Zone::Graveyard); let two=g.put(0,3,Zone::Graveyard); let spell=g.put(0,0,Zone::Hand);
    // Urgent Necropsy's four groups are artifact, creature, enchantment, planeswalker.
    // The same artifact creature in two groups contributes its mana value only once.
    for (targets,evidence,minimum) in [
        (vec![vec![];4],vec![],0),
        (vec![vec![],vec![],vec![Target::Object(enchantment)],vec![]],vec![two],2),
        (vec![vec![Target::Object(resource)],vec![Target::Object(resource)],vec![],vec![]],vec![four],4),
        (vec![vec![Target::Object(resource)],vec![Target::Object(resource)],vec![Target::Object(enchantment)],vec![]],vec![four,two],6),
    ] {
        if !evidence.is_empty() {
            let reason=format!("the chosen cards have total mana value less than {minimum}");
            cite!(["601.2f", "701.59a"]; refused(&reason): act_unchanged(&mut g,cast(spell,targets.clone(),evidence[..evidence.len()-1].to_vec())));
        }
        let evidence_count=evidence.len();
        let mut trial=g.clone();
        cite!(["115.3", "601.2f", "701.59a"]; ok: trial.act(0,cast(spell,targets.clone(),evidence)));
        cite!(["701.59a"]; assert_eq!(trial.exile.len(),evidence_count));
        assert_eq!(trial.players[0].pool.total(),0);
        pass(&mut trial);
        if minimum == 6 {
            cite!(["404.3"]; ok: trial.act(1, Move::Choose(game::state::Answer::Objects(vec![resource, enchantment]))));
        }
        for object in [resource,enchantment] {
            let targeted=targets.iter().flatten().any(|t|*t==Target::Object(object));
            cite!(["608.2c"]; assert_eq!(trial.battlefield.contains(&object),!targeted));
        }
        assert!(trial.stack.is_empty());
    }
}
#[test]
fn r601_2f_evidence_cost_stays_locked_when_target_is_sacrificed_for_mana() {
    let mut g=game(); g.players[0].pool.colorless=0;
    let resource=g.put(0,1,Zone::Battlefield); let four=g.put(0,2,Zone::Graveyard); let spell=g.put(0,0,Zone::Hand);
    let targets=vec![vec![Target::Object(resource)],vec![Target::Object(resource)],vec![],vec![]];
    let plan=vec![Step { plan: vec![], x: None, life: vec![], with: None, source:resource, ability:0, paying:vec![], adding:vec![], chose:vec![], cost_branches:None }];
    cite!(["601.2f", "601.2h"]; refused("the chosen cards have total mana value less than 4"): act_unchanged(&mut g,Move::WithMana(plan.clone(),Box::new(cast(spell,targets.clone(),vec![])))));
    cite!(["601.2f", "601.2g", "701.59a"]; ok: g.act(0,Move::WithMana(plan,Box::new(cast(spell,targets,vec![four])))));
    cite!(["601.2f", "601.2h"]; assert!(!g.battlefield.contains(&resource) && g.obj(four).zone==Zone::Gone));
    cite!(["701.59a"]; assert_eq!(g.exile.len(),1)); pass(&mut g);
    cite!(["608.2b"]; assert!(g.stack.is_empty()));
}

#[track_caller]
fn act_unchanged(g: &mut Game, action: Move) -> Result<(), String> {
    let before=format!("{g:?}");
    let result=g.act(0,action);
    assert_eq!(format!("{g:?}"),before);
    result
}
