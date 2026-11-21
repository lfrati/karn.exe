//! Convoke substitutes creature taps for locked mana; it does not produce mana.
use crate::support::cite;
use game::{CardDef,Game,Move,Zone,ObjId};
use ir::Color;
fn spell(cost:&str)->CardDef {let d=CardDef::new("Spell",cost,"Instant","Convoke\nYou gain 1 life.",None).unwrap();game::play::check(&d).unwrap();d}
fn creature(cost:&str,text:&str)->CardDef {CardDef::new("Creature",cost,"Creature",text,Some((2,2))).unwrap()}
fn cast(card:ObjId,allocation:Vec<Option<Color>>,creatures:&[ObjId])->Move {Move::WithConvoke(allocation,Box::new(Move::WithCosts(creatures.iter().map(|o|vec![*o]).collect(),Box::new(Move::Cast{card,targets:vec![]}))))}
fn fresh(g:&mut Game,index:usize)->ObjId {let card=g.put(0,index,Zone::Hand);g.move_object(card,Zone::Battlefield);*g.battlefield.last().unwrap()}
#[test]
fn r702_51a_colored_generic_and_summoning_sick_resources_pay_without_mana() {
    let mut g=Game::new(vec![spell("{2}{B}{B}"),creature("{B}",""),creature("{U}","")],3,20);
    let card=g.put(0,0,Zone::Hand);let black=[fresh(&mut g,1),fresh(&mut g,1)];let blue=[g.put(0,2,Zone::Battlefield),g.put(0,2,Zone::Battlefield)];
    assert!(g.summoning_sick(black[0]));
    g.act(0,cast(card,vec![Some(Color::Black),Some(Color::Black),None,None],&[black[0],black[1],blue[0],blue[1]])).unwrap();
    for o in black.into_iter().chain(blue) {cite!(["702.51a"]; assert!(g.obj(o).tapped));}
    assert_eq!(g.players[0].pool.total(),0);assert_eq!(g.obj(g.stack[0]).chars.mana_value,4);
    cite!(["702.51a"]; assert!(!g.log.iter().any(|e|matches!(e,game::Event::ManaAdded{..}|game::Event::TappedForMana{..}))));
}
#[test]
fn r702_51a_wrong_color_reused_tapped_or_excess_creatures_reject_transactionally() {
    let mut g=Game::new(vec![spell("{1}{B}"),creature("{U}",""),creature("{B}","")],3,20);let card=g.put(0,0,Zone::Hand);
    let blue=g.put(0,1,Zone::Battlefield);let black=g.put(0,2,Zone::Battlefield);
    for (allocation,objects,why) in [(vec![Some(Color::Black),None],vec![blue,black],"Creature is not what that cost asks for or cannot pay it (701.26, 118.3)"),(vec![Some(Color::Black),None],vec![black,black],"Creature pays for one thing (118.10)"),(vec![Some(Color::Black),None,None],vec![black,blue,black],"Creature pays for one thing (118.10)")] {
        cite!(["702.51a"]; case: format!("{allocation:?} {objects:?}"); refused(why): g.act(0,cast(card,allocation,&objects)));assert!(g.players[0].hand.contains(&card));assert!(!g.obj(black).tapped);assert!(!g.obj(blue).tapped);
    }
    g.tap(black);cite!(["702.51a"]; refused("Creature is not what that cost asks for or cannot pay it (701.26, 118.3)"): g.act(0,cast(card,vec![Some(Color::Black),None],&[black,blue])));
}
#[test]
fn r702_51b_hybrid_choice_and_cost_reduction_precede_substitution() {
    let tax=CardDef::new("Reduction","{0}","Artifact","Spells you cast cost {1} less to cast.",None).unwrap();
    let mut g=Game::new(vec![spell("{2}{W/U}"),creature("{U}{B}",""),tax],3,20);g.put(0,2,Zone::Battlefield);
    let card=g.put(0,0,Zone::Hand);let a=g.put(0,1,Zone::Battlefield);let b=g.put(0,1,Zone::Battlefield);
    cite!(["702.51b"]; ok: g.act(0,cast(card,vec![Some(Color::Blue),None],&[a,b])));assert_eq!(g.obj(g.stack[0]).chars.mana_value,3);
}
#[test]
fn r702_51a_mana_tap_cannot_double_as_convoke() {
    let mut g=Game::new(vec![spell("{1}{B}"),creature("{B}","Haste\n{T}: Add {B}."),creature("{B}","")],3,20);
    let card=g.put(0,0,Zone::Hand);let mana=g.put(0,1,Zone::Battlefield);let other=g.put(0,2,Zone::Battlefield);
    let plan=vec![game::legal::Step { plan: vec![], x: None, life: vec![], with: None, cost_branches: None, chose: vec![], source:mana,ability:1,paying:vec![],adding:vec![]}];
    let said=Move::WithMana(plan.clone(),Box::new(cast(card,vec![None],&[mana])));
    cite!(["702.51a"]; refused("Creature is not what that cost asks for or cannot pay it (701.26, 118.3)"): g.act(0,said));assert!(g.players[0].hand.contains(&card));assert!(!g.obj(mana).tapped);assert_eq!(g.players[0].pool.total(),0);
    g.act(0,crate::support::paid("B",Move::WithMana(plan,Box::new(cast(card,vec![None],&[other]))))).unwrap();assert!(g.obj(mana).tapped);cite!(["702.51a"]; assert!(g.obj(other).tapped));
    assert_eq!(g.players[0].pool.total(),0);
}
#[test]
fn r702_51a_effective_colors_after_mana_production_control_offers_and_acceptance() {
    let painter=creature("{U}","{0}: This creature becomes black until end of turn. Add {B}.");
    let mut g=Game::new(vec![spell("{B}{B}"),painter],3,20);let card=g.put(0,0,Zone::Hand);let source=g.put(0,1,Zone::Battlefield);
    let plan = vec![game::legal::Step { plan: vec![], x: None, life: vec![], with: None, source, ability: 0, paying: vec![], adding: vec![], chose: vec![], cost_branches: None }];
    g.act(0,crate::support::paid("B",Move::WithMana(plan,Box::new(cast(card,vec![Some(Color::Black)],&[source]))))).unwrap();cite!(["702.51a"]; assert!(g.obj(source).tapped));assert_eq!(g.players[0].pool.total(),0);
}
#[test]
fn r702_51a_colorless_cost_cannot_be_convoked_and_cost_increases_are_locked_first() {
    let mut g=Game::new(vec![spell("{C}"),creature("{B}","")],3,20);let card=g.put(0,0,Zone::Hand);let a=g.put(0,1,Zone::Battlefield);
    cite!(["702.51a"]; refused("your mana pool cannot pay that cost (118.3a): {C}"): g.act(0,cast(card,vec![None],&[a])));assert!(!g.obj(a).tapped);
    let tax=CardDef::new("Tax","{0}","Artifact","Spells you cast cost {1} more to cast.",None).unwrap();
    let mut g=Game::new(vec![spell("{B}"),creature("{B}",""),tax],3,20);let a=g.put(0,1,Zone::Battlefield);let b=g.put(0,1,Zone::Battlefield);g.put(0,2,Zone::Battlefield);let card=g.put(0,0,Zone::Hand);
    cite!(["601.2f"]; refused("your mana pool cannot pay that cost (118.3a): {1}{B} in total, {1} more than it was (601.2f)"): g.act(0,cast(card,vec![Some(Color::Black)],&[a])));cite!(["702.51b"]; ok: g.act(0,cast(card,vec![Some(Color::Black),None],&[a,b])));assert_eq!(g.obj(g.stack[0]).chars.mana_value,1);
}
#[test]
fn r733_1_suspended_and_reversed_payment_never_taps_or_publishes_convokers() {
    let defs=vec![spell("{5}"),creature("{G}","{T}: Put a +1/+1 counter on this creature. Add X mana of any one color, where X is the number of +1/+1 counters on this creature."),creature("{G}",""),CardDef::new("Scales","{G}","Enchantment","If one or more +1/+1 counters would be put on a creature you control, that many plus one +1/+1 counters are put on it instead.",None).unwrap(),CardDef::new("Evolution","{2}{G}","Enchantment","If one or more +1/+1 counters would be put on a creature you control, twice that many +1/+1 counters are put on that creature instead.",None).unwrap()];
    let mut g=Game::new(defs,3,20);let card=g.put(0,0,Zone::Hand);let mana=g.put(0,1,Zone::Battlefield);let a=g.put(0,2,Zone::Battlefield);g.put(0,3,Zone::Battlefield);g.put(0,4,Zone::Battlefield);
    let step=game::legal::Step { plan: vec![], x: None, life: vec![], with: None, cost_branches: None, chose: vec![], source:mana,ability:0,paying:vec![],adding:vec![vec![ir::mana::ManaSymbol::Color(Color::Green);3]]};g.act(0,Move::WithMana(vec![step],Box::new(cast(card,vec![None],&[a])))).unwrap();
    let Some(game::state::Decision::Choose{choice:game::state::Choice::Replacement{among,..},..})=&g.decision else{panic!("{:?}",g.decision)};
    let effect=among.iter().find(|e|g.obj(e.source).card==Some(4)).unwrap().clone();g.act(0,Move::Choose(game::state::Answer::Replacement(effect))).unwrap();assert!(matches!(g.decision,Some(game::state::Decision::Payment{..})));assert!(!g.obj(a).tapped);
    let mut clone=g.clone();g.act(0,Move::ReversePayment).unwrap();cite!(["733.1"]; assert!(g.players[0].hand.contains(&card)));assert!(!g.obj(a).tapped);assert!(g.obj(mana).tapped);assert_eq!(g.players[0].pool.total(),3);assert!(!g.log.iter().any(|e|matches!(e,game::Event::Cast{..})));
    clone.players[0].pool.colorless=1;clone.act(0,Move::FinishPayment{plan:vec![],paying:vec![vec![a]],with:Some(crate::support::mana_pool("GGGC"))}).unwrap();assert!(clone.obj(a).tapped);cite!(["601.2i"]; assert_eq!(clone.log.iter().filter(|e|matches!(e,game::Event::Cast{..})).count(),1));
}
#[test]
fn r702_51a_mixed_tap_and_zone_costs_retain_the_independent_resource_refusal() {
    let d=CardDef::new("Mixed","{B}","Instant","Convoke\nAs an additional cost to cast this spell, sacrifice a creature.\nYou gain 1 life.",None).unwrap();
    crate::support::refused(game::play::check(&d).map_err(|e|e.0),"convoke combined with a zone-changing object cost requires independent resource kinds");
}
#[test]
fn r702_51b_kicker_alternative_cost_and_final_minimum_compose_with_convoke() {
    let faces=oracle::corpus::load(&oracle::corpus::default_path()).unwrap();let sphere=CardDef::from_face(faces.iter().find(|f|f.face_name=="Trinisphere").unwrap()).unwrap();
    let alternative=CardDef::new("Alternative","{6}","Instant","Convoke\nYou may pay {B} rather than pay this spell's mana cost.\nYou gain 1 life.",None).unwrap();
    let kicked=CardDef::new("Kicked","{B}","Instant","Convoke\nKicker {2}\nYou gain 1 life.",None).unwrap();
    for (definition,alternative) in [(alternative,true),(kicked,false)] {
        game::play::check(&definition).unwrap();let mut g=Game::new(vec![definition,creature("{B}",""),sphere.clone()],3,20);let card=g.put(0,0,Zone::Hand);let a=g.put(0,1,Zone::Battlefield);let b=g.put(0,1,Zone::Battlefield);let c=g.put(0,1,Zone::Battlefield);g.put(0,2,Zone::Battlefield);
        let said=cast(card,vec![Some(Color::Black),None,None],&[a,b,c]);let said=if alternative{Move::CastFor(game::announcement::CastCost::Alternative(1),Box::new(said))}else{Move::WithKicks(1,Box::new(said))};
        g.act(0,said).unwrap();assert_eq!(g.players[0].pool.total(),0);cite!(["702.51b"]; assert!(g.obj(a).tapped&&g.obj(b).tapped&&g.obj(c).tapped));
    }
}
#[test]
fn r702_51a_sacrificed_mana_source_cannot_also_convoke() {
    let mut g=Game::new(vec![spell("{1}{B}"),creature("{B}","Sacrifice this creature: Add {B}."),creature("{B}","")],3,20);let card=g.put(0,0,Zone::Hand);let mana=g.put(0,1,Zone::Battlefield);let other=g.put(0,2,Zone::Battlefield);
    let plan=vec![game::legal::Step { plan: vec![], x: None, life: vec![], with: None, cost_branches: None, chose: vec![], source:mana,ability:0,paying:vec![],adding:vec![]}];cite!(["702.51a"]; refused("Creature is not what that cost asks for or cannot pay it (701.26, 118.3)"): g.act(0,Move::WithMana(plan.clone(),Box::new(cast(card,vec![None],&[mana])))));assert_eq!(g.obj(mana).zone,Zone::Battlefield);
    g.act(0,crate::support::paid("B",Move::WithMana(plan,Box::new(cast(card,vec![None],&[other]))))).unwrap();assert_eq!(g.obj(mana).zone,Zone::Gone);cite!(["702.51a"]; assert!(g.obj(other).tapped));
}
