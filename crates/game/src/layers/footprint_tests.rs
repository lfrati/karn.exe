//! Conservative metadata and indexed/exact-reference rule comparisons.
use super::*;
use crate::{CardDef, Zone};
use crate::program::footprint::{EffectFootprint, Inputs, Properties as P};
fn card(text: &str) -> CardDef { CardDef::new("Fixture", "{0}", "Creature — Human",text,Some((2,2))).unwrap() }
fn footprint(text: &str, layer: Layer) -> EffectFootprint {
    let c=card(text);
    let statement=c.characteristics.iter().find_map(|r|r.result.as_ref().ok()).expect("compiled characteristic");
    statement.footprint(statement.parts().next().unwrap(),layer)
}
#[test]
fn metadata_keeps_implicit_membership_and_distinct_layer_outputs() {
    let f=footprint("This creature gets +1/+1 and has flying.",Layer::Abilities);
    assert!(f.reads.source.intersects(P::ABILITIES | P::CONTROL));
    assert!(f.reads.context.intersects(P::CONTINUATION));
    assert_eq!(f.writes.recipient,P::KEYWORDS);
    let pt=footprint("This creature gets +1/+1 and has flying.",Layer::ChangePt);
    assert_eq!(pt.writes.recipient,P::PT);
    assert!(!pt.may_depend_on(EffectFootprint { reads:Inputs::NONE, writes:Inputs::at(crate::program::footprint::Domain::Recipient,P::PT) }));
    assert!(f.may_depend_on(EffectFootprint { reads:Inputs::NONE, writes:Inputs::UNKNOWN }));
}
#[test]
fn negative_and_empty_domains_nested_values_and_type_side_effects_are_kept() {
    for text in ["Creatures without flying have vigilance.", "Creatures have haste as long as you control no creatures with flying."] {
        let f=footprint(text,Layer::Abilities);
        assert!(f.reads.objects.intersects(P::ABILITIES));
        assert!(f.reads.objects.intersects(P::COLLECTIONS));
    }
    let f=footprint("This creature gets +1/+1 for each creature card in your graveyard.",Layer::ChangePt);
    assert!(f.reads.objects.intersects(P::ZONE | P::COLLECTIONS | P::TYPES));
    let f=footprint("This creature gets +1/+1 as long as your life total is greater than your starting life total.",Layer::ChangePt);
    assert!(f.reads.players.intersects(P::LIFE));
    let f=footprint("This creature is a Plains.",Layer::Types);
    assert!(f.writes.recipient.intersects(P::ABILITIES));
}
#[test]
fn attachment_color_cda_and_unknown_metadata() {
    let aura=CardDef::new("Aura","{U}","Enchantment — Aura","Enchant creature\nEnchanted creature gets +1/+1.",None).unwrap();
    let f=aura.characteristics.iter().find_map(|r|r.result.as_ref().ok()).unwrap().footprint(0,Layer::ChangePt);
    assert!(f.reads.source.intersects(P::ATTACHMENT));
    assert!(crate::program::expr::Filter::Atom(crate::program::expr::Atom::Color(ir::Color::Blue)).inputs().all().intersects(P::COLORS));
    let f=footprint("Changeling",Layer::Types);
    assert_eq!(f.writes.recipient,P::TYPES);
    assert!(!f.reads.unknown);
    let filter=crate::program::expr::Filter::Atom(crate::program::expr::Atom::Chosen(crate::binding::Scalar::Local {slot:0,kind:crate::binding::Kind::Color}));
    assert!(filter.inputs().unknown);
}
#[test]
fn reference_mode_preserves_rules_across_generated_orderings_and_mutations() {
    let defs=vec![card(""),card("Creatures you control have vigilance."),card("Creatures with vigilance have flying."),card("All creatures lose all abilities and have base power and toughness 1/1."),card("Creatures you control get +1/+1 and have flying and indestructible."),card("Changeling")];
    for order in [[1,2,3,4],[4,3,2,1],[2,4,1,3],[3,1,4,2]] {
        let mut g=Game::new(defs.clone(),2,20);
        let bear=g.put(0,0,Zone::Battlefield);
        for index in order { g.put(0,index,Zone::Battlefield); }
        g.put(1,5,Zone::Hand);
        for _ in 0..2 {
            let expected=g.seen(bear,None);
            let reference=reference(||g.seen(bear,None));
            assert_eq!(expected,reference);
            g.obj_mut(bear).counters.insert("+1/+1".into(),2);
        }
    }
}
/// One panic-safe local reference mode: all exact trials, whole worlds, no read cache.
pub(crate) fn reference<R>(read:impl FnOnce()->R)->R {
    struct Reset(bool,bool);
    impl Drop for Reset { fn drop(&mut self) {TRIALS_ONLY.with(|t|t.set(self.0));WHOLE_WORLD.with(|t|t.set(self.1));} }
    let _reset=Reset(TRIALS_ONLY.with(|t|t.replace(true)),WHOLE_WORLD.with(|t|t.replace(true)));
    read()
}

#[test]
fn reference_mode_bypasses_populated_cache_and_restores_ordinary_reuse() {
    let mut g=Game::new(vec![card(""),card("Creatures you control get +1/+1.")],2,20);
    let bear=g.put(0,0,Zone::Battlefield);
    g.put(0,1,Zone::Battlefield);
    let expected=g.seen(bear,None);
    let computed=std::cell::Cell::new(false);
    let actual=reference(||g.query_seen(bear,None,|| {computed.set(true);g.seen(bear,None)}));
    assert!(computed.get(),"reference must not hit a previously cached world");
    assert_eq!(actual,expected);
    let reused=g.query_seen(bear,None,||panic!("ordinary reuse must resume after reference"));
    assert_eq!(reused,expected);
}

#[test]
fn indexed_identity_candidates_preserve_order_clones_and_full_results() {
    let mut g = Game::new(vec![card("This creature gets +1/+1."),card("")],2,20);
    let source = g.put(0,0,Zone::Battlefield);
    for _ in 0..12 { g.put(0,1,Zone::Battlefield); }
    for _ in 0..12 { g.put(1,1,Zone::Hand); }
    VISITS.with(|n|n.set(0));
    let expected: Vec<_> = g.battlefield.iter().map(|id|(*id,g.seen(*id,None))).collect();
    let indexed_visits = VISITS.with(|n|n.replace(0));
    let actual = reference(||g.battlefield.iter().map(|id|(*id,g.seen(*id,None))).collect::<Vec<_>>());
    let reference_visits = VISITS.with(|n|n.get());
    assert_eq!(actual,expected);
    assert!(indexed_visits < reference_visits);
    assert_eq!(g.seen(source,None).pt,Some((3,3)));
    let world = Working::before(expected);
    let clone = world.clone();
    assert_eq!(clone,world);
    for (slot,(id,seen)) in world.objects.iter().enumerate() {
        assert_eq!(world.slot(*id),Some(slot));
        assert_eq!(clone.get(*id),Some(seen));
    }
}

#[test]
fn keyword_gains_do_not_create_producing_membership_but_removal_and_grants_do() {
    let reader=footprint("Creatures you control have vigilance.",Layer::Abilities);
    let keyword=footprint("Creatures you control have flying.",Layer::Abilities);
    assert!(reader.reads.source.intersects(P::MEMBERSHIP));
    assert!(!reader.may_depend_on(keyword));
    assert!(reader.may_depend_on(footprint("All creatures lose all abilities.",Layer::Abilities)));
    assert!(reader.may_depend_on(footprint("Creatures have \"Other creatures you control have lifelink.\"",Layer::Abilities)));
    assert!(footprint("Creatures without flying have vigilance.",Layer::Abilities).may_depend_on(keyword));
    let mut g=Game::new(vec![card(""),card("Creatures you control have \"Other creatures you control have lifelink.\""),card("Creatures you control have vigilance.")],2,20);
    g.put(0,0,Zone::Battlefield); g.put(0,0,Zone::Battlefield);
    g.put(0,1,Zone::Battlefield); g.put(0,2,Zone::Battlefield);
    PAIRS.with(|n|n.set(0));
    let expected:Vec<_>=g.battlefield.iter().map(|on|g.seen(*on,None)).collect();
    let indexed=PAIRS.with(|n|n.replace(0));
    let actual=reference(||g.battlefield.iter().map(|on|g.seen(*on,None)).collect::<Vec<_>>());
    assert_eq!(actual,expected);
    assert!(indexed < PAIRS.with(|n|n.get()),"membership trials retained, keyword-only interactions pruned");
    assert!(expected[0].has(ir::Keyword::from_name("Lifelink").unwrap()));
    assert!(!expected[0].given.is_empty());
}

#[test]
fn indexed_pairs_preserve_negative_domains_new_writers_cycles_and_continuation() {
    let texts=["", "Creatures without flying have vigilance.", "Creatures without vigilance have flying.",
        "Creatures have trample as long as you control no creatures with flying.",
        "Creatures you control have \"Other creatures you control have lifelink.\"",
        "All creatures lose all abilities and have base power and toughness 1/1.",
        "Creatures you control get +1/+1 and have flying and indestructible.", "Changeling"];
    let defs=texts.iter().map(|text|card(text)).collect::<Vec<_>>();
    for order in [[1,2,3,4,5,6],[6,5,4,3,2,1],[5,1,4,2,6,3]] {
        let mut g=Game::new(defs.clone(),3,20);
        let bear=g.put(0,0,Zone::Battlefield);
        for i in order {g.put(0,i,Zone::Battlefield);}
        g.put(1,7,Zone::Hand);
        for round in 0..3 {
            for cut in std::iter::once(None).chain(LAYERS.into_iter().map(Some)) {
                let objects:Vec<_>=g.battlefield.iter().chain(&g.players[1].hand).copied().collect();
                let expected=objects.iter().map(|on|g.seen(*on,cut)).collect::<Vec<_>>();
                let actual=reference(||objects.iter().map(|on|g.seen(*on,cut)).collect::<Vec<_>>());
                assert_eq!(actual,expected,"order{order:?} round{round} cut{cut:?}");
                assert_eq!(g.clone().seen(bear,cut),g.seen(bear,cut));
            }
            g.put_counters(bear,"vigilance",1);
        }
    }
}

#[test]
fn potential_grant_registry_bounds_identity_without_testing_current_membership() {
    let aura=CardDef::new("Aura","{0}","Enchantment — Aura",
        "Enchant creature\nEnchanted creature has \"Other creatures you control have lifelink.\"",None).unwrap();
    crate::play::check(&aura).unwrap();
    let mut g=Game::new(vec![card(""),aura],2,20);
    let first=g.put(0,0,Zone::Battlefield);
    let second=g.put(0,0,Zone::Battlefield);
    let a=g.put(0,1,Zone::Battlefield); let b=g.put(0,1,Zone::Battlefield);
    assert!(g.attach(a,Target::Object(first))); assert!(g.attach(b,Target::Object(first)));
    POTENTIAL_GRANTS.with(|n|n.set(0));
    let expected=g.seen(first,None);
    let indexed=POTENTIAL_GRANTS.with(|n|n.replace(0));
    assert_eq!(reference(||g.seen(first,None)),expected);
    assert!(indexed < POTENTIAL_GRANTS.with(|n|n.get()));
    assert_eq!(expected.given.len(),2);
    assert!(g.seen(second,None).has(ir::Keyword::from_name("Lifelink").unwrap()));
    for action in 0..3 {
        if action==0 {assert!(g.attach(a,Target::Object(second)));}
        if action==1 {g.move_object(b,Zone::Hand);}
        if action==2 {g.continuous.push(Continuous {producer:None,change:Change::LosesAll,affected:vec![second],stamp:100,part:0,lifetime:Lifetime::Forever});}
        for on in g.battlefield.iter() {assert_eq!(g.seen(*on,None),reference(||g.seen(*on,None)));}
    }
}

#[test]
fn retained_exact_cuts_keep_full_grant_identity_and_recompute_continuation() {
    let defs=vec![card(""),CardDef::new("Monument","{0}","Artifact","Creatures you control get +2/+2 and have flying.",None).unwrap(),
        card("All creatures lose all abilities."),
        card("Creatures you control have \"Other creatures you control have lifelink.\"")];
    let mut g=Game::new(defs,2,20);
    let bear=g.put(0,0,Zone::Battlefield);
    g.put(0,1,Zone::Battlefield);
    g.put(1,2,Zone::Battlefield);
    let giver=g.put(0,3,Zone::Battlefield);
    for cut in [Some(Layer::DefinePt),None,Some(Layer::Colors)] {
        let expected=reference(||g.seen(bear,cut));
        let actual=g.seen(bear,cut);
        assert_eq!(actual,expected);
        assert_eq!(g.query_seen(bear,cut,||panic!("exact cut must survive repeated reads")),expected);
    }
    // A later cut applies the complete multilayer effect (613.6), never resumes
    // from the narrower displayed characteristics. The creature grant source is removed.
    assert_eq!(g.power(bear),Some(4));
    assert!(g.held(giver).is_empty());
    let removal=g.battlefield[2];
    g.move_object(removal,Zone::Graveyard);
    assert_eq!(g.power(bear),Some(4));
    let actual=g.seen(giver,None);
    assert!(!actual.given.is_empty());
    assert_eq!(actual,reference(||g.seen(giver,None)));
    let keys=g.held(giver);
    assert_eq!(keys,reference(||g.held(giver)));
    assert_eq!(keys,g.held(giver));
}

#[test]
fn mixed_constant_and_power_filtered_proposals_keep_exact_trials() {
    let defs=vec![card(""),card("Creatures you control get +1/+1."),
        card("Creatures you control get +2/+2."),
        card("Creatures with power 2 or greater get +1/+1.")];
    for def in &defs[1..] { assert!(def.characteristics.iter().any(|c|c.result.is_ok()),"compiled fixture"); }
    let mut g=Game::new(defs,2,20);
    for d in 0..4 {g.put(0,d,Zone::Battlefield);}
    PAIRS.with(|n|n.set(0));
    let actual:Vec<_>=g.battlefield.iter().map(|id|(*id,g.seen(*id,None))).collect();
    assert!(PAIRS.with(|n|n.get())>0,"power reader retains exact trials");
    for (id,seen) in actual {
        assert_eq!(seen.pt,Some((6,6)));
        assert_eq!(seen,reference(||g.seen(id,None)));
    }
}
#[test]
fn baseline_and_trial_source_existence_negative_edges_cycles_and_continuation_stay_exact() {
    let enchant=|text|CardDef::new("Modifier","{0}","Enchantment",text,None).unwrap();
    let defs=vec![card(""),enchant("Creatures you control have \"Creatures without flying have vigilance.\""),
        enchant("Creatures you control have flying."),enchant("All creatures lose all abilities."),
        enchant("Creatures have trample as long as you control no creatures with flying."),
        enchant("Creatures you control get +1/+1 and have flying and indestructible.")];
    for def in &defs[1..] { assert!(def.characteristics.iter().any(|c|c.result.is_ok()),"compiled fixture"); }
    for order in [[1,2,3,4,5],[3,2,1,5,4],[5,4,1,3,2]] {
        let mut g=Game::new(defs.clone(),2,20);
        g.put(0,0,Zone::Battlefield);g.put(1,0,Zone::Battlefield);
        for d in order {g.put(0,d,Zone::Battlefield);}
        for change in 0..3 {
            if change==1 {let id=g.battlefield[3];g.move_object(id,Zone::Hand);}
            if change==2 {let on=g.battlefield[0];g.continuous.push(Continuous {producer:None,change:Change::LosesAll,affected:vec![on],stamp:100,part:0,lifetime:Lifetime::Forever});}
            for id in &g.battlefield {assert_eq!(g.seen(*id,None),reference(||g.seen(*id,None)));}
        }
    }
}

#[test]
fn r122_1_player_checkeds_invalidate_cached_characteristics_and_clone_independently() {
    let f = footprint("This creature gets +1/+1 for each experience counter you have.",Layer::ChangePt);
    assert!(f.reads.players.intersects(P::COUNTERS));
    let mut g = Game::new(vec![card("This creature gets +1/+1 for each experience counter you have.")],2,20);
    let creature=g.put(0,0,Zone::Battlefield);
    let mut total = 0;
    for n in [0,1,3] {
        if n>0 {g.put_counters(crate::Target::Player(0),"experience",n);}
        total += n as i32;
        assert_eq!(g.seen(creature,None),reference(||g.seen(creature,None)));
        assert_eq!(g.seen(creature,None).pt,Some((2+total,2+total)));
    }
    let mut other=g.clone();
    other.remove_counters(crate::Target::Player(0),"experience",3);
    assert_eq!(other.seen(creature,None).pt,Some((3,3)));
    assert_eq!(other.seen(creature,None),reference(||other.seen(creature,None)));
    assert_eq!(g.seen(creature,None).pt,Some((6,6)));
}

#[test]
fn distinct_color_cda_depends_on_color_and_membership_changes() {
    let f = footprint("This creature's power is equal to the number of colors among permanents you control.", Layer::DefinePt);
    assert!(f.reads.objects.intersects(P::COLORS));
    assert!(f.reads.objects.intersects(P::CONTROL));
    assert!(f.reads.objects.intersects(P::COLLECTIONS));
    assert!(f.may_depend_on(EffectFootprint {
        reads: Inputs::NONE,
        writes: Inputs::at(crate::program::footprint::Domain::Recipient, P::COLORS),
    }));
}

#[test]
fn inverse_attachment_metadata_observes_source_types_edges_and_membership() {
    for text in ["Equipped creatures you control get +1/+1.", "Enchanted creatures you control get +1/+1.", "Creatures you control that aren't equipped get +1/+1."] {
        let f=footprint(text,Layer::ChangePt);
        for property in [P::TYPES,P::ATTACHMENT,P::IDENTITY,P::ZONE,P::COLLECTIONS] {
            assert!(f.reads.objects.intersects(property),"{text}: {property:?}");
        }
    }
}
