//! Fixed resolved effects need only their recipient; static rules retain the whole world.
use crate::support::cite;
use game::{CardDef,Game,Move,Target,Zone};
use ir::{CardType,Subtype};
use game::layers::{Change,Continuous,Lifetime,Types};
fn def(name:&str,kind:&str,text:&str,pt:Option<(i32,i32)>)->CardDef {
    let d=CardDef::new(name,"{0}",kind,text,pt).unwrap();game::play::check(&d).unwrap();d
}
fn pass(g:&mut Game){for _ in 0..g.players.len(){g.act(g.priority,Move::Pass).unwrap();}}
#[test]
fn r613_1_an_unrelated_static_does_not_change_fixed_effect_queries(){
    let mut g=Game::new(vec![def("Land","Land","",None),def("Unused","Enchantment","Creatures with flying get +1/+1.",None)],2,20);
    let land=g.put(0,0,Zone::Battlefield);let other=g.put(1,0,Zone::Battlefield);
    for _ in 0..60{g.put(0,0,Zone::Library);g.put(1,0,Zone::Library);}
    for stamp in 1..=4 {
        g.continuous.push(Continuous{producer:None,change:Change::Types(Types{types:vec![CardType::Creature],subtypes:vec![Subtype::from_name("Elf").unwrap()],every_creature_type:true,keeps:true}),affected:vec![land],stamp,part:0,lifetime:Lifetime::Forever});
        g.continuous.push(Continuous{producer:None,change:Change::BasePt(2,2),affected:vec![land],stamp,part:1,lifetime:Lifetime::Forever});
    }
    g.put_counters(land,"+1/+1",2);
    let mut with_static=g.clone();with_static.put(0,1,Zone::Battlefield);
    for object in std::iter::once(land).chain(std::iter::once(other)).chain(g.players.iter().flat_map(|p|p.library.iter().copied())) {
        cite!(["613.1"]; assert_eq!(g.seen(object,None),with_static.seen(object,None)));
    }
    cite!(["613.4b","613.4c"]; assert_eq!(g.power(land),Some(4)));assert_eq!(g.power(other),None);
}
#[test]
fn r613_1f_a_resolved_grant_of_a_static_rule_keeps_other_recipients_visible(){
    let mut g=Game::new(vec![def("Bear","Creature — Bear","",Some((2,2))),
        def("Grant","Instant","Until end of turn, target creature gains \"Other creatures you control get +1/+1.\"",None)],2,20);
    let source=g.put(0,0,Zone::Battlefield);let recipient=g.put(0,0,Zone::Battlefield);let opponent=g.put(1,0,Zone::Battlefield);
    let spell=g.put(0,1,Zone::Hand);g.act(0,Move::Cast{card:spell,targets:vec![vec![Target::Object(source)]]}).unwrap();pass(&mut g);
    cite!(["613.1f"]; assert_eq!((g.power(source),g.power(recipient),g.power(opponent)),(Some(2),Some(3),Some(2))));
    assert_eq!(g.seen(recipient,None),g.clone().seen(recipient,None));
    g.destroy(source);cite!(["611.3b"]; assert_eq!(g.power(recipient),Some(2)));
}

#[test]
fn r613_4a_multiple_definitions_read_earlier_types_and_current_membership_in_every_zone() {
    let faces=oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let rat=CardDef::from_face(faces.iter().find(|f|f.name=="Pack Rat").unwrap()).unwrap();
    game::play::check(&rat).unwrap();
    let mut g=Game::new(vec![rat,def("Bear","Creature — Bear","",Some((2,2))),
        def("Large","Enchantment","Creatures with power 2 or greater get +1/+1.",None)],2,20);
    let rat=g.put(0,0,Zone::Battlefield);
    let library=g.put(0,0,Zone::Library);let hand=g.put(0,0,Zone::Hand);
    let opponent=g.put(1,0,Zone::Battlefield);let bear=g.put(0,1,Zone::Battlefield);
    g.put(0,2,Zone::Battlefield);
    for o in [rat,library,hand,opponent]{cite!(["604.3"]; assert_eq!(g.power(o),Some(1)));}
    g.continuous.push(Continuous{producer:None,change:Change::Types(Types{types:vec![],subtypes:vec![Subtype::from_name("Rat").unwrap()],every_creature_type:false,keeps:true}),affected:vec![bear],stamp:99,part:0,lifetime:Lifetime::Forever});
    // Layer four changes the counted set; layer seven's later conditional pump sees
    // the computed definition. Hidden cards have the definition but not the pump.
    cite!(["613.1d","613.4a"]; assert_eq!((g.power(rat),g.power(library),g.power(hand),g.power(opponent)),(Some(3),Some(2),Some(2),Some(1))));
    for mut branch in [g.clone(),g] {
        branch.move_object(bear,Zone::Graveyard);
        cite!(["613.4a"]; assert_eq!((branch.power(rat),branch.power(library),branch.power(hand)),(Some(1),Some(1),Some(1))));
        branch.continuous.push(Continuous{producer:None,change:Change::LosesAll,affected:vec![rat],stamp:100,part:0,lifetime:Lifetime::Forever});
        cite!(["613.1f"]; assert_eq!(branch.power(rat),Some(0)));
        assert_eq!(branch.power(library),Some(1),"ability removal does not stop the permanent being counted");
    }
}
