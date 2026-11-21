//! Granted statics enter the same layer's dependency order through actual membership (613.8).
use crate::support::{cite, paid};
use game::{CardDef,Game,Move,ObjId,Target,Zone};
fn game()->Game {
    let faces=oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let full=CardDef::from_face(faces.iter().find(|f|f.face_name=="Dancer's Chakrams").unwrap()).unwrap();
    let defs=vec![full,
        CardDef::new("Wearer","{0}","Creature — Bear","",Some((2,2))).unwrap(),
        CardDef::new("Commander","{0}","Legendary Creature — Hero","",Some((2,2))).unwrap(),
        CardDef::new("Forest","","Basic Land — Forest","",None).unwrap(),
        CardDef::new("Erase","{0}","Instant","Target creature loses all abilities until end of turn.",None).unwrap(),
        CardDef::new("Steal","{0}","Instant","Gain control of target creature.",None).unwrap(),
        CardDef::new("Remove Keyword","{0}","Instant","Target creature loses lifelink until end of turn.",None).unwrap(),
        CardDef::new("Granting Charm","{0}","Instant","Until end of turn, target creature gains \"Other creatures you control get +1/+1 and have flying.\"",None).unwrap()];
    for d in &defs {game::play::check(d).unwrap_or_else(|e|panic!("{}: {e:?}",d.name()));}
    let mut g=Game::new(defs,3,20);for p in 0..3 {for _ in 0..8 {g.put(p,3,Zone::Library);}}g
}
fn pass(g:&mut Game){for _ in 0..3 {g.act(g.priority,Move::Pass).unwrap();}}
fn cast_at(g:&mut Game,player:usize,def:usize,target:ObjId){
    while g.priority!=player {g.act(g.priority,Move::Pass).unwrap();}
    let card=g.put(player,def,Zone::Hand);g.act(player,Move::Cast{card,targets:vec![vec![Target::Object(target)]]}).unwrap();pass(g);
}
fn commander(g:&mut Game,player:usize)->ObjId {
    let card=g.put(player,2,Zone::Command);g.designate_commander(card).unwrap();g.move_object(card,Zone::Battlefield);
    *g.battlefield.iter().find(|o|g.obj(**o).card==Some(2)&&g.obj(**o).owner==player).unwrap()
}
fn attach(g:&mut Game,owner:usize,wearer:ObjId)->ObjId {let source=g.put(owner,0,Zone::Battlefield);assert!(g.attach(source,Target::Object(wearer)));source}
#[test]
fn full_chakrams_jobs_hero_performer_lifelink_and_other_commander_bonus(){
    let mut g=game();let commander=commander(&mut g,0);let card=g.put(0,0,Zone::Hand);g.players[0].pool.colorless=3;g.add_mana(0,ir::mana::ManaSymbol::Color(ir::Color::White));
    g.act(0,paid("CCCW",Move::Cast{card,targets:vec![]})).unwrap();pass(&mut g);pass(&mut g);
    let hero=*g.battlefield.iter().find(|o|g.obj(**o).token()).unwrap();let seen=g.seen(hero,None);
    assert!(seen.colors.is_empty());assert!(seen.subtypes.iter().any(|s|s.name()=="Hero"));assert!(seen.subtypes.iter().any(|s|s.name()=="Performer"));
    assert_eq!((g.power(hero),g.toughness(hero)),(Some(3),Some(3)));assert!(g.has(hero,"Lifelink"));
    assert_eq!(g.power(commander),Some(4));cite!(["613.8b"];assert!(g.has(commander,"Lifelink")));
    let eq=*g.battlefield.iter().find(|o|g.obj(**o).card==Some(0)).unwrap();cite!(["702.182a"];assert_eq!(g.attached_to(eq),Some(Target::Object(hero))));
}
#[test]
fn wearer_controller_determines_bonus_and_wearer_commander_is_other_exclusion(){
    let mut g=game();let ours=commander(&mut g,0);let theirs=commander(&mut g,1);let wearer=g.put(1,1,Zone::Battlefield);let equipment=attach(&mut g,0,wearer);
    assert_eq!(g.power(ours),Some(2));assert!(!g.has(ours,"Lifelink"));cite!(["109.5"];assert_eq!(g.power(theirs),Some(4)));assert!(g.has(theirs,"Lifelink"));
    cast_at(&mut g,0,5,wearer);cite!(["109.5"];assert_eq!(g.power(ours),Some(4)));assert_eq!(g.power(theirs),Some(2));assert!(!g.has(theirs,"Lifelink"));
    assert!(g.attach(equipment,Target::Object(ours)));assert_eq!(g.power(ours),Some(4),"wearer's direct +2 only, not its other-commanders bonus");
    assert!(!g.seen(wearer,None).subtypes.iter().any(|s|s.name()=="Performer"));assert!(g.has(ours,"Lifelink"));
}
#[test]
fn overlapping_instances_and_source_loss_do_not_leave_phantom_grants(){
    let mut g=game();let commander=commander(&mut g,0);let wearer=g.put(0,1,Zone::Battlefield);let first=attach(&mut g,0,wearer);let second=attach(&mut g,0,wearer);
    assert_eq!(g.power(commander),Some(6));assert_eq!(g.seen(wearer,None).given.len(),2);
    g.move_object(first,Zone::Hand);cite!(["611.3b"];assert_eq!(g.power(commander),Some(4)));assert_eq!(g.seen(wearer,None).given.len(),1);assert!(g.has(commander,"Lifelink"));
    g.move_object(second,Zone::Hand);cite!(["611.3b"];assert_eq!(g.power(commander),Some(2)));assert!(!g.has(commander,"Lifelink"));assert!(g.seen(wearer,None).given.is_empty());
}
#[test]
fn equip_listing_command_and_late_ability_removal_follow_real_membership(){
    let mut g=game();let commander=commander(&mut g,0);let old=g.put(0,1,Zone::Battlefield);let new=g.put(0,1,Zone::Battlefield);let eq=attach(&mut g,0,old);
    g.players[0].pool.colorless=3;
    let option=4; // Three printed rows, then job select and equip contributions.
    cite!(["702.6a"];ok:g.act(0,paid("CCC",Move::Activate{source:eq,ability:option,targets:vec![vec![Target::Object(new)]],paying:vec![]})));pass(&mut g);
    assert_eq!(g.power(old),Some(2));cite!(["702.6a"];assert_eq!(g.power(new),Some(4)));assert_eq!(g.power(commander),Some(4));assert!(g.has(commander,"Lifelink"));
    cast_at(&mut g,0,4,new);assert!(g.seen(new,None).given.is_empty());cite!(["613.1f"];assert!(!g.has(new,"Lifelink")));assert_eq!(g.power(new),Some(4));
    cite!(["613.1f"];assert_eq!(g.power(commander),Some(2)));assert!(!g.has(commander,"Lifelink"));
    assert!(!g.has(old,"Lifelink"));
}
#[test]
fn resolved_grant_waits_for_its_producer_and_later_removal_wins(){
    let mut g=game();let wearer=g.put(0,1,Zone::Battlefield);let other=g.put(0,1,Zone::Battlefield);
    cast_at(&mut g,0,7,wearer);assert_eq!(g.power(other),Some(3));cite!(["613.8b"];assert!(g.has(other,"Flying")));assert_eq!(g.power(wearer),Some(2));
    cast_at(&mut g,0,4,wearer);assert_eq!(g.power(other),Some(2));cite!(["613.7"];assert!(!g.has(other,"Flying")));
}
#[test]
fn recipient_keyword_removal_and_later_reattachment_obey_timestamps(){
    let mut g=game();let commander=commander(&mut g,0);let wearer=g.put(0,1,Zone::Battlefield);let equipment=attach(&mut g,0,wearer);
    cast_at(&mut g,0,6,commander);cite!(["613.7"];assert!(!g.has(commander,"Lifelink")));assert_eq!(g.power(commander),Some(4));
    let next=g.put(0,1,Zone::Battlefield);g.attach(equipment,Target::Object(next));cite!(["613.7e"];assert!(g.has(commander,"Lifelink")));assert_eq!(g.power(commander),Some(4));
}
#[test]
fn early_layer_and_recursive_definition_grants_stay_refused(){
    for text in ["Creatures you control are blue.","Creatures you control lose all abilities.","Creatures you control have \"{T}: Add {G}.\""] {
        let definition=CardDef::new("Boundary","{0}","Enchantment",text,None).unwrap();
        crate::support::refused(game::ability::template(&definition.abilities()[0]).map_err(|e|e.0),"gains: a static ability that changes types, colors or definition membership (613.1d-f)");
    }
}
#[test]
fn commander_designation_is_physical_and_unzoned_general_cards_stay_refused(){
    let mut g=game();let commander=commander(&mut g,0);let wearer=g.put(0,1,Zone::Battlefield);attach(&mut g,0,wearer);
    let copy=g.create_token(0,g.obj(commander).chars.clone(),false);
    cite!(["903.3"];assert!(!g.is_commander(copy)));assert_eq!(g.power(copy),Some(2));assert!(!g.has(copy,"Lifelink"));
    assert_eq!(g.power(commander),Some(4));assert!(g.has(commander,"Lifelink"));
    for text in ["Other cards you control get +1/+1.","Other commanders get +1/+1."] {
        let definition=CardDef::new("Boundary","{0}","Enchantment",text,None).unwrap();
        crate::support::refused(game::play::check(&definition).map_err(|e|e.0),"noun phrase: Card");
    }
}
