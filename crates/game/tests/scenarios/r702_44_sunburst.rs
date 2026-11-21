//! Sunburst counts colors actually paid, and its entry replacements participate in 616.1.
use game::{CardDef, Game, Move, ObjId, Zone};
use game::state::{Answer, Choice, Decision};
use crate::support::cite;
fn def(name: &str, cost: &str, ty: &str, text: &str, pt: Option<(i32,i32)>) -> CardDef {
    let d=CardDef::new(name,cost,ty,text,pt).unwrap(); game::play::check(&d).unwrap_or_else(|e|panic!("{name}: {e:?}")); d
}
fn explosives() -> CardDef { def("Engineered Explosives","{X}","Artifact","Sunburst (This artifact enters with a charge counter on it for each color of mana spent to cast it.)\n{2}, Sacrifice this artifact: Destroy each nonland permanent with mana value equal to the number of charge counters on this artifact.",None) }
fn resolve(g:&mut Game) { for _ in 0..g.players.len() {g.act(g.priority,Move::Pass).unwrap();} }
fn cast(g:&mut Game,index:usize,x:u32,spending:&str) -> ObjId {
    let card=g.put(0,index,Zone::Hand);
    g.act(0,crate::support::paid(spending,Move::WithX(x,Box::new(Move::Cast {card,targets:vec![]})))).unwrap();
    resolve(g); card
}
#[test]
fn r702_44_explosives_counts_paid_colors_not_x_and_uses_departed_counter_lki() {
    let mut g=Game::new(vec![explosives(),def("Two","{2}","Artifact","",None),def("Three","{3}","Creature — Bear","",Some((2,2))),def("Land","","Land","",None)],2,20);
    let two=g.put(1,1,Zone::Battlefield);let three=g.put(1,2,Zone::Battlefield);let land=g.put(1,3,Zone::Battlefield);
    g.players[0].pool.colors=[2,1,0,0,0];cast(&mut g,0,3,"WWU");
    let bomb=*g.battlefield.last().unwrap();cite!(["702.44a"]; assert_eq!(g.obj(bomb).counters.get("charge"),Some(&2)));
    g.players[0].pool.colorless=2;
    g.act(0,crate::support::paid("CC",Move::Activate {source:bomb,ability:1,targets:vec![],paying:vec![]})).unwrap();
    assert!(!g.battlefield.contains(&bomb));let mut copy=g.clone();
    for g in [&mut g,&mut copy] {resolve(g);cite!(["608.2h", "113.7a"]; assert!(!g.battlefield.contains(&two)));assert!(g.battlefield.contains(&three));assert!(g.battlefield.contains(&land));}
}
#[test]
fn r702_44b_colorless_zero_and_noncast_entry_supply_no_colors() {
    for amount in [0,3] {
        let mut g=Game::new(vec![explosives()],2,20);g.players[0].pool.colorless=amount;cast(&mut g,0,amount,if amount == 0 { "" } else { "CCC" });
        cite!(["702.44b"]; assert!(g.obj(g.battlefield[0]).counters.is_empty()));
    }
    let mut g=Game::new(vec![explosives(),def("Open the Vaults","{0}","Sorcery","Return all artifact and enchantment cards from all graveyards to the battlefield under their owners' control.",None)],2,20);
    g.players[0].pool.colors=[1,1,0,0,0];cast(&mut g,0,2,"WU");let bomb=g.battlefield[0];g.move_object(bomb,Zone::Graveyard);
    let spell=g.put(0,1,Zone::Hand);g.act(0,Move::Cast {card:spell,targets:vec![]}).unwrap();resolve(&mut g);
    cite!(["702.44b"]; assert!(g.obj(g.battlefield[0]).counters.is_empty()));
}
#[test]
fn r702_44a_ignores_type_changing_effects_when_selecting_counter_kind() {
    let animator=def("Animation","{0}","Enchantment","Noncreature artifacts become 3/3 artifact creatures.",None);
    let mut g=Game::new(vec![explosives(),animator,def("Anthem","{0}","Enchantment","Creatures you control get +1/+1.",None)],2,20);
    g.put(0,1,Zone::Battlefield);g.put(0,2,Zone::Battlefield);g.players[0].pool.colors=[1,1,0,0,0];cast(&mut g,0,2,"WU");
    let bomb=*g.battlefield.last().unwrap();cite!(["702.44a"]; assert_eq!(g.obj(bomb).counters.get("charge"),Some(&2)));cite!(["702.44a"]; assert!(!g.obj(bomb).counters.contains_key("+1/+1")));
}
#[test]
fn r702_44d_multiple_instances_and_counter_replacements_have_independent_identities() {
    let sun=def("Sun creature","{X}","Artifact Creature — Construct","Sunburst, sunburst",Some((1,1)));
    let scales=def("Hardened Scales","{G}","Enchantment","If one or more +1/+1 counters would be put on a creature you control, that many plus one +1/+1 counters are put on it instead.",None);
    let mut g=Game::new(vec![sun,scales],2,20);g.put(0,1,Zone::Battlefield);g.players[0].pool.colors=[1,1,0,0,0];cast(&mut g,0,2,"WU");
    // Choose both sunburst additions before scaling the resulting four counters.
    let mut chosen=Vec::new();
    while let Some(Decision::Choose {player,choice:Choice::Replacement {ref among,..}})=g.decision {
        let pick=among.iter().find(|id| id.ability().unwrap().index==0 && id.source!=g.battlefield[0]).unwrap_or(&among[0]).clone();
        chosen.push(pick.clone());g.act(player,Move::Choose(Answer::Replacement(pick))).unwrap();
    }
    let creature=*g.battlefield.last().unwrap();cite!(["702.44d", "122.6"]; assert_eq!(g.obj(creature).counters.get("+1/+1"),Some(&5)));
    cite!(["616.1"]; assert!(chosen.len()>=2));
}
#[test]
fn r702_44_granted_sunburst_stays_refused_until_granted_replacements_exist() {
    let d=CardDef::new("Grant","{0}","Enchantment","Artifacts you control have sunburst.",None).unwrap();
    crate::support::refused(game::play::check(&d).map_err(|e|e.0),"keyword: Sunburst");
}
