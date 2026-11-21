//! Job select is retained token entry followed by attachment (702.182a).
use crate::support::cite;
use game::{CardDef,Game,Move,Target,Zone};
use game::state::{Answer,Choice,Decision};
fn setup()->Game {
    let card=|name,text|CardDef::new(name,"{0}","Artifact — Equipment",text,None).unwrap();
    let defs=vec![card("Job Equipment","Job select\nEquipped creature gets +2/+2."),
        CardDef::new("Scales","{0}","Enchantment","If one or more +1/+1 counters would be put on a creature you control, that many plus one +1/+1 counters are put on it instead.",None).unwrap(),
        CardDef::new("Evolution","{0}","Enchantment","If one or more +1/+1 counters would be put on a creature you control, twice that many +1/+1 counters are put on that creature instead.",None).unwrap(),
        CardDef::new("Grumgully","{0}","Legendary Creature — Goblin Shaman","Each other non-Human creature you control enters with an additional +1/+1 counter on it.",Some((3,3))).unwrap()];
    for d in &defs {game::play::check(d).unwrap_or_else(|e|panic!("{}: {e:?}",d.name()));}
    Game::new(defs,2,20)
}
fn pass(g:&mut Game){for _ in 0..2 {g.act(g.priority,Move::Pass).unwrap();}}
fn cast(g:&mut Game){let card=g.put(0,0,Zone::Hand);g.act(0,Move::Cast{card,targets:vec![]}).unwrap();pass(g);}
#[test]
fn hero_is_colorless_one_one_created_once_and_attached_before_priority(){
    let mut g=setup();cast(&mut g);pass(&mut g);
    let hero=*g.battlefield.iter().find(|o|g.obj(**o).token()).unwrap();
    cite!(["702.182a"]; assert!(g.seen(hero,None).colors.is_empty()));cite!(["702.182a"]; assert!(g.seen(hero,None).subtypes.iter().any(|s|s.name()=="Hero")));
    assert_eq!((g.power(hero),g.toughness(hero)),(Some(3),Some(3)));
    let equipment=*g.battlefield.iter().find(|o|g.obj(**o).card==Some(0)).unwrap();cite!(["702.182a"]; assert_eq!(g.attached_to(equipment),Some(Target::Object(hero))));
}
#[test]
fn entry_replacement_choice_and_clone_do_not_repeat_token_creation_or_attachment(){
    let mut g=setup();let scales=g.put(0,1,Zone::Battlefield);let evolution=g.put(0,2,Zone::Battlefield);g.put(0,3,Zone::Battlefield);
    cast(&mut g);pass(&mut g);
    let among=match &g.decision {Some(Decision::Choose{player:0,choice:Choice::Replacement{among,..}})=>among.clone(),other=>panic!("{other:?}")};
    let clone=g.clone();for (mut branch,first,amount) in [(g,scales,4),(clone,evolution,3)] {
        let chosen=among.iter().find(|r|r.source==first).unwrap().clone();
        cite!(["616.1"]; refused("that decision is another player's"): branch.act(1,Move::Choose(Answer::Replacement(chosen.clone()))));
        branch.act(0,Move::Choose(Answer::Replacement(chosen))).unwrap();
        let heroes:Vec<_>=branch.battlefield.iter().copied().filter(|o|branch.obj(*o).token()).collect();cite!(["702.182a"]; assert_eq!(heroes.len(),1));
        let hero=heroes[0];cite!(["616.1"]; assert_eq!(branch.obj(hero).counters.get("+1/+1"),Some(&amount)));assert_eq!(branch.power(hero),Some(3+amount as i32));
        let equipment=*branch.battlefield.iter().find(|o|branch.obj(**o).card==Some(0)).unwrap();cite!(["702.182a"]; assert_eq!(branch.attached_to(equipment),Some(Target::Object(hero))));
        assert!(branch.stack.is_empty()&&branch.decision.is_none());
    }
}
