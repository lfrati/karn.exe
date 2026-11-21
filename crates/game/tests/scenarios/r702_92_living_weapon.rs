//! Living weapon (702.92a) composes an ordinary entry trigger, token creation and attachment.
use crate::support::cite;
use game::{CardDef, Event, Game, Move, ObjId, Target, Zone};
use game::state::{Answer, Choice, Decision};
use ir::{Ability, Color};
const BATTERSKULL: &str = "Living weapon (When this Equipment enters, create a 0/0 black Phyrexian Germ creature token, then attach this to it.)\nEquipped creature gets +4/+4 and has vigilance and lifelink.\n{3}: Return this Equipment to its owner's hand.\nEquip {5}";
fn pass(g: &mut Game) { for _ in 0..g.players.len() { g.act(g.priority, Move::Pass).unwrap(); } }
fn card(name: &str, types: &str, text: &str, pt: Option<(i32,i32)>) -> CardDef { let d=CardDef::new(name,"{0}",types,text,pt).unwrap(); game::play::check(&d).unwrap(); d }
fn game() -> Game {
    let d = CardDef::new("Batterskull", "{5}", "Artifact — Equipment", BATTERSKULL, None).unwrap();
    game::play::check(&d).unwrap();
    let defs = vec![d,
        card("Bear","Creature — Bear","",Some((2,2))),
        card("Forest","Basic Land — Forest","",None),
        card("Hardened Scales","Enchantment","If one or more +1/+1 counters would be put on a creature you control, that many plus one +1/+1 counters are put on it instead.",None),
        card("Branching Evolution","Enchantment","If one or more +1/+1 counters would be put on a creature you control, twice that many +1/+1 counters are put on that creature instead.",None),
        card("Grumgully, the Generous","Legendary Creature — Goblin Shaman","Each other non-Human creature you control enters with an additional +1/+1 counter on it.",Some((3,3))),
        card("Veil","Enchantment","Creatures you control have shroud.",None),
    ];
    let mut g=Game::new(defs,2,20);for p in 0..2 {for _ in 0..8 {g.put(p,2,Zone::Library);}}g
}
fn cast(g: &mut Game) -> ObjId {
    let card=g.put(0,0,Zone::Hand);g.players[0].pool.colorless=5;
    g.act(0,crate::support::paid("CCCCC",Move::Cast{card,targets:vec![]})).unwrap();pass(g);
    *g.battlefield.iter().find(|o|g.obj(**o).card==Some(0)).unwrap()
}
fn germ(g: &Game) -> ObjId { *g.battlefield.iter().find(|o|g.obj(**o).token()).unwrap() }
fn activate(g: &mut Game, source: ObjId, row: usize, spending: &str, targets: Vec<Vec<Target>>) -> Result<(),String> {
    let ability=g.abilities(source).iter().enumerate().find_map(|(i,a)|(matches!(&**a,Ability::Activated{..})&&g.def(source).unwrap().ability_row(i)==row).then_some(i)).unwrap();
    g.act(0,crate::support::paid(spending,Move::Activate{source,ability,targets,paying:vec![]}))
}
fn advance(g: &mut Game, done: impl Fn(&Game)->bool) {
    for _ in 0..180 {if done(g){return;}match g.decision.clone(){
        Some(Decision::Attackers{player})=>g.act(player,Move::Attack(vec![])).unwrap(),
        Some(Decision::Blockers{player})=>g.act(player,Move::Block(vec![])).unwrap(),
        None=>g.act(g.priority,Move::Pass).unwrap(),other=>panic!("{other:?}"),
    }}panic!("did not advance");
}

#[test]
fn r702_92_full_batterskull_creates_and_equips_the_germ_before_state_based_actions() {
    let mut g=game();let equipment=cast(&mut g);
    cite!(["702.92a"]; assert_eq!(g.stack.len(),1));assert_eq!(g.battlefield.len(),1);assert!(g.has(equipment,"Living weapon"));
    pass(&mut g);let germ=germ(&g);
    cite!(["702.92a"]; assert_eq!(g.attached_to(equipment),Some(Target::Object(germ))));
    assert_eq!((g.power(germ),g.toughness(germ)),(Some(4),Some(4)));
    cite!(["702.92a"]; assert_eq!(g.obj(germ).chars.colors,vec![Color::Black]));
    for subtype in ["Phyrexian","Germ"] {assert!(g.obj(germ).chars.subtypes.iter().any(|s|s.name()==subtype));}
    assert!(g.has(germ,"Lifelink")&&g.has(germ,"Vigilance"));
    let turn=g.turn;
    advance(&mut g,|g|g.turn>turn&&matches!(g.decision,Some(Decision::Attackers{player:0})));
    g.act(0,Move::Attack(vec![(germ,Target::Player(1))])).unwrap();
    cite!(["702.20b"]; assert!(!g.obj(germ).tapped,"vigilance prevents attack tapping"));
    advance(&mut g,|g|g.players[1].life==16);
    cite!(["702.15b"]; assert_eq!(g.players[0].life,24,"combat damage gains life through lifelink"));
}

#[test]
fn r702_92_captured_living_weapon_survives_source_departure_and_creates_a_doomed_germ() {
    let mut g=game();let source=cast(&mut g);let clone=g.clone();
    for (mut branch,departed) in [(g,true),(clone,false)] {
        if departed {branch.destroy(source);}
        pass(&mut branch);
        cite!(["113.7a"]; case: format!("departed {departed}"); assert_eq!(branch.log.iter().filter(|e|matches!(e,Event::Created{..})).count(),1));
        cite!(["704.5f"]; case: format!("departed {departed}"); assert_eq!(branch.battlefield.iter().filter(|o|branch.obj(**o).token()).count(),usize::from(!departed)));
        assert!(branch.stack.is_empty());
    }
}

#[test]
fn r301_5_full_batterskull_equip_transfers_its_bonuses_and_the_old_germ_dies() {
    let mut g=game();let source=cast(&mut g);pass(&mut g);let old=germ(&g);
    let own=g.put(0,1,Zone::Battlefield);let other=g.put(1,1,Zone::Battlefield);
    g.players[0].pool.colorless=5;
    cite!(["702.6a", "115.1"]; refused("an illegal target, Bear: the target is one you control, and another player controls it (115.1)"): activate(&mut g,source,3,"CCCCC",vec![vec![Target::Object(other)]]));
    activate(&mut g,source,3,"CCCCC",vec![vec![Target::Object(own)]]).unwrap();pass(&mut g);
    cite!(["704.5f"]; assert!(!g.battlefield.contains(&old)));cite!(["702.6a"]; assert_eq!(g.attached_to(source),Some(Target::Object(own))));
    assert_eq!((g.power(own),g.toughness(own)),(Some(6),Some(6)));
    assert!(g.has(own,"Lifelink")&&g.has(own,"Vigilance"));
}

#[test]
fn r400_7_full_batterskull_bounces_then_casts_as_a_new_living_weapon_source() {
    let mut g=game();let source=cast(&mut g);pass(&mut g);let old_germ=germ(&g);
    g.players[0].pool.colorless=3;activate(&mut g,source,2,"CCC",vec![]).unwrap();pass(&mut g);
    assert!(g.battlefield.is_empty());assert!(!g.battlefield.contains(&old_germ));
    let card=g.players[0].hand[0];assert_eq!(g.obj(card).card,Some(0));
    g.players[0].pool.colorless=5;g.act(0,crate::support::paid("CCCCC",Move::Cast{card,targets:vec![]})).unwrap();pass(&mut g);pass(&mut g);
    let now=*g.battlefield.iter().find(|o|g.obj(**o).card==Some(0)).unwrap();cite!(["400.7"]; assert_ne!(now,source));
    cite!(["702.92a"]; assert_eq!(g.attached_to(now),Some(Target::Object(germ(&g)))));assert_ne!(germ(&g),old_germ);
    assert_eq!(g.log.iter().filter(|e|matches!(e,Event::Created{..})).count(),2);
}

#[test]
fn r701_3_living_weapon_attachment_does_not_target_its_shrouded_token() {
    let mut g=game();g.put(0,6,Zone::Battlefield);let source=cast(&mut g);pass(&mut g);
    let germ=germ(&g);assert!(g.has(germ,"Shroud"));cite!(["702.92a", "115.10"]; assert_eq!(g.attached_to(source),Some(Target::Object(germ))));
    cite!(["115.10a"]; assert!(!g.log.iter().any(|e|matches!(e,Event::BecameTarget{object:Target::Object(o),..} if *o==germ))));
}

#[test]
fn r616_1_living_weapon_retains_created_token_through_cloned_entry_replacement_questions() {
    let mut g=game();let scales=g.put(0,3,Zone::Battlefield);let double=g.put(0,4,Zone::Battlefield);g.put(0,5,Zone::Battlefield);
    let source=cast(&mut g);pass(&mut g);
    let among=match &g.decision {Some(Decision::Choose{player:0,choice:Choice::Replacement{among,count:1,..}})=>among.clone(),other=>panic!("{other:?}")};
    assert!(g.battlefield.iter().all(|o|!g.obj(*o).token()));
    let clone=g.clone();for (mut branch,first,amount) in [(g,scales,4),(clone,double,3)] {
        let chosen=among.iter().find(|r|r.source==first).unwrap().clone();
        cite!(["616.1"]; refused("that decision is another player's"): branch.act(1,Move::Choose(Answer::Replacement(chosen.clone()))));
        branch.act(0,Move::Choose(Answer::Replacement(chosen))).unwrap();let germ=germ(&branch);
        cite!(["616.1"]; case: format!("amount {amount}"); assert_eq!(branch.obj(germ).counters.get("+1/+1"),Some(&amount)));
        cite!(["702.92a"]; case: format!("amount {amount}"); assert_eq!(branch.attached_to(source),Some(Target::Object(germ))));
        assert_eq!(branch.power(germ),Some(4+amount as i32));
        assert_eq!(branch.log.iter().filter(|e|matches!(e,Event::Created{..})).count(),1);
        assert!(branch.stack.is_empty()&&branch.decision.is_none());
    }
}

#[test]
fn r603_3_multiple_living_weapon_instances_create_separate_tokens_and_attach_in_order() {
    let def=card("Witness","Artifact — Equipment","Living weapon, living weapon\nEquipped creature gets +1/+1.",None);
    let mut g=Game::new(vec![def],2,20);let card=g.put(0,0,Zone::Hand);
    g.act(0,Move::Cast{card,targets:vec![]}).unwrap();pass(&mut g);
    cite!(["603.3b"]; assert!(matches!(g.decision,Some(Decision::Order{player:0,..}))));g.act(0,Move::Order(vec![1,0])).unwrap();
    pass(&mut g);let first=germ(&g);pass(&mut g);let second=germ(&g);assert_ne!(first,second);
    cite!(["704.5f"]; assert!(!g.battlefield.contains(&first)));cite!(["113.2c", "702.92a"]; assert_eq!(g.log.iter().filter(|e|matches!(e,Event::Created{..})).count(),2));
}

#[test]
fn r702_92_granting_living_weapon_remains_refused_until_executable_keyword_grants_exist() {
    let def=CardDef::new("Unsupported","{0}","Instant","Target Equipment gains living weapon until end of turn.",None).unwrap();
    crate::support::refused(game::play::check(&def).map_err(|e|e.0),"keyword: Living Weapon");
}
