//! Changeling is a subtype CDA, not a printed expansion of its type line (702.73, 604.3).
use crate::support::cite;
use game::{CardDef, Game, Move, ObjId, Target, Zone};
use ir::Subtype;
use ir::vocab::SubtypeKind;
fn def(name: &str, kind: &str, text: &str, pt: Option<(i32,i32)>) -> CardDef {
    let d=CardDef::new(name,"{0}",kind,text,pt).unwrap_or_else(|e|panic!("{name}: {e}"));
    game::play::check(&d).unwrap_or_else(|e|panic!("{name}: {e:?}"));d
}
fn setup() -> Game {Game::new(vec![
    def("Shape","Creature — Shapeshifter","Changeling",Some((3,2))),
    def("Frog world","Enchantment","Creatures are Frogs.",None),
    def("Remove","Instant","Target creature loses all abilities until end of turn.",None),
    def("Frog","Instant","Target creature becomes a 1/1 blue Frog creature until end of turn.",None),
    def("Duplicate","Instant","Create a token that's a copy of target creature.",None),
    def("Mercenary","Sorcery","Create a 3/2 colorless Shapeshifter creature token with changeling.",None),
    def("Lose keyword","Instant","Target creature loses changeling until end of turn.",None),
    def("Forest","Basic Land — Forest","",None),
],3,20)}
fn all(g: &Game, o: ObjId) {
    let seen=g.seen(o,None);
    // "This object is every creature type" (702.73a): each creature type, and no other subtype
    for t in Subtype::of_kind(SubtypeKind::Creature) {cite!(["702.73a"]; case: t.name(); assert!(seen.subtypes.contains(t),"missing {}",t.name()));}
    cite!(["702.73a"]; assert_eq!(seen.subtypes.len(),Subtype::of_kind(SubtypeKind::Creature).len()));
    for t in Subtype::all().filter(|t|t.kind()!=SubtypeKind::Creature) {cite!(["702.73a"]; case: t.name(); assert!(!seen.subtypes.contains(&t),"extraneous {}",t.name()));}
}
fn pass(g: &mut Game) {for _ in 0..3 {g.act(g.priority,Move::Pass).unwrap();}}
fn cast(g: &mut Game, def: usize, target: Option<ObjId>) {
    let o=g.put(0,def,Zone::Hand);g.act(0,Move::Cast{card:o,targets:target.map(|t|vec![vec![Target::Object(t)]]).unwrap_or_default()}).unwrap();pass(g);
}
#[test]
fn r702_73a_every_shared_creature_type_functions_in_all_zones_and_on_stack() {
    let mut g=setup();
    for zone in [Zone::Hand,Zone::Library,Zone::Graveyard,Zone::Exile,Zone::Command,Zone::Battlefield] {
        let o=g.put(0,0,zone);all(&g,o);
        cite!(["702.73a", "604.3"]; case: format!("{zone:?}"); assert_eq!(g.seen(o,Some(game::layers::Layer::Colors)).subtypes.len(),Subtype::of_kind(SubtypeKind::Creature).len()));
        cite!(["604.3"]; case: format!("{zone:?}"); assert_eq!(g.obj(o).chars.subtypes,vec![Subtype::from_name("Shapeshifter").unwrap()]));
    }
    let o=g.put(0,0,Zone::Hand);g.act(0,Move::Cast{card:o,targets:vec![]}).unwrap();all(&g,*g.stack.last().unwrap());
}
#[test]
fn r613_3_subtype_cda_precedes_other_type_effects_in_both_timestamp_orders() {
    for first in [false,true] {
        let mut g=setup();if first {g.put(0,1,Zone::Battlefield);}let o=g.put(0,0,Zone::Battlefield);if !first {g.put(0,1,Zone::Battlefield);}
        cite!(["613.3"]; case: format!("frogs first {first}"); assert_eq!(g.seen(o,None).subtypes,vec![Subtype::from_name("Frog").unwrap()]));assert!(g.has(o,"Changeling"));
    }
}
#[test]
fn r613_1d_f_removing_changeling_does_not_undo_types_but_type_replacement_does() {
    for remove in [2,6] {
        let mut g=setup();let o=g.put(0,0,Zone::Battlefield);cast(&mut g,remove,Some(o));
        cite!(["613.1f"]; case: format!("remover {remove}"); assert!(!g.has(o,"Changeling")));all(&g,o);
        cast(&mut g,3,Some(o));cite!(["613.1d"]; case: format!("remover {remove}"); assert_eq!(g.seen(o,None).subtypes,vec![Subtype::from_name("Frog").unwrap()]));
        assert_eq!(g.power(o),Some(1));assert!(!g.has(o,"Changeling"));
    }
}
#[test]
fn r604_3a_created_token_and_copy_retain_definition_not_effective_type_override() {
    let mut g=setup();cast(&mut g,5,None);let original=g.battlefield[0];all(&g,original);
    assert!(g.def(original).is_some());assert_eq!((g.power(original),g.toughness(original)),(Some(3),Some(2)));
    cast(&mut g,2,Some(original));all(&g,original);cast(&mut g,3,Some(original));
    cite!(["613.1d"]; assert_eq!(g.seen(original,None).subtypes,vec![Subtype::from_name("Frog").unwrap()]));
    cast(&mut g,4,Some(original));let copy=*g.battlefield.last().unwrap();assert_ne!(copy,original);
    cite!(["707.2"]; assert!(g.has(copy,"Changeling")));all(&g,copy);cite!(["707.2"]; assert_eq!(g.power(copy),Some(3)));
}
#[test]
fn r604_3a_mixed_keyword_definitions_and_kindred_spell_share_cda_behavior() {
    let d=CardDef::new("Empty shape","{G}","Creature — Shapeshifter","Changeling, devoid, flying",Some((2,2))).unwrap();
    game::play::check(&d).unwrap();
    let spell=def("Kindred","Kindred Instant — Shapeshifter","Changeling, devoid\nYou gain 1 life.",None);
    let mut g=Game::new(vec![d,spell],3,20);let o=g.put(0,0,Zone::Hand);all(&g,o);
    cite!(["702.114a"]; assert!(g.seen(o,None).colors.is_empty()));assert!(g.has(o,"Flying"));
    let card=g.put(0,1,Zone::Hand);g.act(0,Move::Cast{card,targets:vec![]}).unwrap();all(&g,*g.stack.last().unwrap());pass(&mut g);assert_eq!(g.players[0].life,21);
}
#[test]
fn r205_3d_noncreature_nonkindred_object_cannot_gain_creature_types() {
    let d=def("Wrong kind","Artifact","Changeling",None);let mut g=Game::new(vec![d],3,20);let o=g.put(0,0,Zone::Battlefield);
    cite!(["205.3d"]; assert!(g.seen(o,None).subtypes.is_empty()));
}
#[test]
fn r604_3a_early_type_keyword_and_quoted_static_grants_remain_refused() {
    for text in ["Target creature gains changeling until end of turn.","Until end of turn, target creature gains \"This creature is every creature type.\""] {
        let d=CardDef::new("Refused","{0}","Instant",text,None).unwrap();let reason=if text.contains("gains changeling") { "gains: changeling requires earlier-layer ability membership (613.1d)" } else { "gains: a static ability that changes types, colors or definition membership (613.1d-f)" };
        crate::support::refused(game::play::check(&d).map_err(|e|e.0),reason);
    }
}
#[test]
fn r702_73a_announced_face_qualifies_for_each_supported_creature_type_permission() {
    for kind in ["Goblin", "Elf", "Time Lord"] {
        let permit=def("Permission","Artifact",&format!("Once during each of your turns, you may cast a {kind} spell from your graveyard."),None);
        let shape=def("Shape","Creature — Shapeshifter","Changeling",Some((3,2)));
        let other=def("Other","Creature — Bear","",Some((2,2)));
        let mut g=Game::new(vec![permit,shape,other],3,20);let source=g.put(0,0,Zone::Battlefield);let card=g.put(0,1,Zone::Graveyard);let wrong=g.put(0,2,Zone::Graveyard);
        cite!(["601.3"]; refused("that casting permission is unavailable (601.3)"): g.act(0,Move::CastVia(game::announcement::CastRoute::Permission { source, ability: 0 }, Box::new(Move::Cast{card:wrong,targets:vec![]}))));
        g.act(0,Move::CastVia(game::announcement::CastRoute::Permission { source, ability: 0 }, Box::new(Move::Cast{card,targets:vec![]}))).unwrap();all(&g,*g.stack.last().unwrap());
    }
}
