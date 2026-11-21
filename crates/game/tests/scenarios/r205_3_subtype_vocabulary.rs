//! All-type additions preserve subtype identity and timestamp behavior (205.3, 613.7).
use crate::support::{cite, paid};
use game::{CardDef, Game, Move, Zone};
use game::layers::{Change, Continuous, Lifetime, Types};
use ir::{CardType, Subtype};
use ir::vocab::SubtypeKind;
#[test]
fn r205_3_repeated_full_mutavault_keeps_each_subtype_once_and_later_type_effects_still_apply() {
    let faces=oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let def=CardDef::from_face(faces.iter().find(|f|f.name=="Mutavault").unwrap()).unwrap();
    game::play::check(&def).unwrap();
    let mut g=Game::new(vec![def],2,20);let vault=g.put(0,0,Zone::Battlefield);
    let plains=Subtype::from_name("Plains").unwrap();
    g.continuous.push(Continuous { producer:None, change:Change::Types(Types {types:vec![CardType::Land],subtypes:vec![plains],every_creature_type:false,keeps:true}),affected:vec![vault],stamp:0,part:0,lifetime:Lifetime::Forever });
    let expected:Vec<_>=std::iter::once(plains).chain(Subtype::of_kind(SubtypeKind::Creature).iter().copied()).collect();
    g.players[0].pool.colorless=8;
    for _ in 0..8 {
        g.act(0,paid("C",Move::Activate{source:vault,ability:1,targets:vec![],paying:vec![]})).unwrap();
        g.act(0,Move::Pass).unwrap();g.act(1,Move::Pass).unwrap();
        cite!(["205.1b","205.3m"]; assert_eq!(g.seen(vault,None).subtypes,expected));
    }
    assert_eq!(g.seen(vault,None),g.clone().seen(vault,None));
    let stamp=g.continuous.iter().map(|e|e.stamp).max().unwrap()+1;
    let frog=Subtype::from_name("Frog").unwrap();
    g.continuous.push(Continuous { producer:None, change:Change::Types(Types {types:vec![CardType::Creature],subtypes:vec![frog],every_creature_type:false,keeps:false}),affected:vec![vault],stamp,part:0,lifetime:Lifetime::Forever });
    cite!(["205.1a","613.7"]; assert_eq!(g.seen(vault,None).subtypes,vec![frog]));
    cite!(["205.1a"]; assert_eq!(g.seen(vault,None).types,vec![CardType::Creature]));
    g.continuous.pop();
    assert_eq!(g.seen(vault,None).subtypes,expected);
}
