//! Derived results for one unchanged branch revision. Tracked mutable lending
//! invalidates before state can change; enforced typed loans preserve only certified
//! unaffected exact cuts/families. Speculative clones start with an empty cache.
//! Games remain Send but are not Sync: independent games run on separate threads.
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use crate::{Game, ObjId};
use crate::state::LayerRevision;
use crate::program::footprint::{Inputs, Properties as P};

// Structural discovery includes absence: a new source/face/grant/domain member must
// invalidate even when no old effect or CDA was found. Domains remain wildcards.
const DISCOVERY: P = P::IDENTITY; // expanded by discovery_reads to avoid const operator calls
fn discovery_reads() -> P { DISCOVERY | P::ZONE | P::CATALOG | P::MEMBERSHIP | P::COLLECTIONS | P::TIMESTAMP | P::LIFETIME | P::VIEW | P::CONTINUATION }
pub(crate) fn characteristic_reads() -> Inputs {
    Inputs::at(crate::program::footprint::Domain::Objects, discovery_reads() | P::CONTROL | P::OWNER | P::TYPES | P::COLORS | P::ABILITIES | P::PT | P::COUNTERS | P::ATTACHMENT | P::NAME)
}
fn cut_index(before: Option<Layer>) -> usize { before.map_or(7, |l|l as usize) }
use crate::layers::{Layer, Seen};

type Key = (ObjId, Option<u8>);
#[derive(Debug)]
pub(crate) struct Cache {
    revision: Cell<Option<LayerRevision>>,
    values: RefCell<HashMap<Key, Seen>>,
    reads: RefCell<[Option<Inputs>; 8]>,
    reverse: RefCell<[u8; 64]>,
    unknown: Cell<u8>,
    no_effects: RefCell<Vec<Option<Layer>>>,
    off_zone_definitions: RefCell<Option<Vec<ObjId>>>,
    /// How many answers an order of simultaneous timestamps decided (613.7m); none of
    /// them, nor any answer computed while one was found, is kept.
    order_dependent: Cell<u32>,
}
impl Default for Cache {
    fn default() -> Self { Self { revision:Cell::new(None), values:Default::default(), reads:RefCell::new([None;8]), reverse:RefCell::new([0;64]), unknown:Cell::new(0), no_effects:Default::default(), off_zone_definitions:Default::default(), order_dependent:Cell::new(0) } }
}
impl Cache {
    pub(crate) fn invalidate(&mut self) {
        #[cfg(feature = "profile")]
        crate::profile::count(crate::profile::Count::QueryInvalidations, 1);
        self.revision.set(None);
        self.values.get_mut().clear();
        *self.reads.get_mut() = [None;8];
        *self.reverse.get_mut() = [0;64];
        self.unknown.set(0);
        self.no_effects.get_mut().clear();
        *self.off_zone_definitions.get_mut() = None;
    }
    /// Called only by enforced field-specific mutation methods. Unknown/generic state
    /// lending always uses invalidate instead. Retag all certified unaffected families.
    pub(crate) fn changed(&mut self, writes: P, revision: LayerRevision) {
        let mut dirty = self.unknown.get();
        let mut bits = writes.bits();
        while bits != 0 {
            let bit = bits.trailing_zeros() as usize;
            dirty |= self.reverse.get_mut()[bit];
            bits &= bits-1;
        }
        let before = self.values.get_mut().len();
        #[cfg(feature = "profile")]
        let proofs_before=self.no_effects.get_mut().len() + usize::from(self.off_zone_definitions.get_mut().is_some());
        if dirty!=0 { self.values.get_mut().retain(|(_,cut),_| dirty & (1 << cut.map_or(7,usize::from)) == 0); }
        #[cfg(feature = "profile")]
        {
            crate::profile::count(crate::profile::Count::QueryRetained, self.values.get_mut().len());
            crate::profile::count(crate::profile::Count::QueryDiscarded, before-self.values.get_mut().len());
            crate::profile::count(crate::profile::Count::TypedChanges, 1);
        }
        #[cfg(not(feature = "profile"))] let _ = before;
        if dirty!=0 {
            for cut in 0..8 { if dirty & (1<<cut) != 0 { self.reads.get_mut()[cut]=None; } }
            self.unknown.set(self.unknown.get() & !dirty);
            for readers in self.reverse.get_mut() { *readers &= !dirty; }
        }
        if writes.intersects(discovery_reads()) {
            self.no_effects.get_mut().clear();
            *self.off_zone_definitions.get_mut()=None;
        }
        // This proof scans restriction/effective registration inputs independently of
        // characteristics. No complete general subscription contract is claimed yet.
        #[cfg(feature = "profile")] {
            let proofs_after=self.no_effects.get_mut().len()+usize::from(self.off_zone_definitions.get_mut().is_some());
            crate::profile::count(crate::profile::Count::ProofsRetained,proofs_after);
            crate::profile::count(crate::profile::Count::ProofsDiscarded,proofs_before-proofs_after);
        }
        self.revision.set(Some(revision));
    }

}
// A clone can immediately be mutated independently of the borrowed source game.
impl Clone for Cache { fn clone(&self) -> Self { Self::default() } }
impl Game {
    pub(crate) fn subscribe_cut(&self, before: Option<Layer>, inputs: Inputs) {
        if !self.sharing_queries() { return; }
        let at=cut_index(before);
        let previous=self.query_cache.reads.borrow()[at].unwrap_or(Inputs::NONE);
        let all=previous | inputs;
        if all==previous { return; }
        self.query_cache.reads.borrow_mut()[at]=Some(all);
        let mask=1u8<<at;
        if all.unknown {self.query_cache.unknown.set(self.query_cache.unknown.get() | mask);}
        let mut bits=all.all().bits() & !previous.all().bits();
        let mut reverse=self.query_cache.reverse.borrow_mut();
        while bits!=0 { let bit=bits.trailing_zeros() as usize; reverse[bit] |= mask; bits &= bits-1; }
    }
    /// Immutable source discovery can be shared across layer cuts in this revision.
    /// Keep only defining sources, not every card in every hidden zone.
    pub(crate) fn defining_sources(&self, compute: impl FnOnce() -> Vec<ObjId>) -> Vec<ObjId> {
        if !self.sharing_queries() { return compute(); }
        if let Some(sources) = &*self.query_cache.off_zone_definitions.borrow() { return sources.clone(); }
        let sources = compute();
        *self.query_cache.off_zone_definitions.borrow_mut() = Some(sources.clone());
        sources
    }

    /// Discovery's empty result is common to all objects of an ordinary game view.
    /// Counters and printed characteristics remain object-specific and are read normally.
    pub(crate) fn no_layer_effects(&self, before: Option<Layer>) -> bool {
        self.sharing_queries() && self.prospective.is_none() && self.query_cache.no_effects.borrow().contains(&before)
    }
    pub(crate) fn remember_no_effects(&self, before: Option<Layer>) {
        if self.sharing_queries() && self.prospective.is_none() { self.query_cache.no_effects.borrow_mut().push(before); }
    }

    /// An answer that the players' order of simultaneous timestamps decides (613.7m):
    /// refuse the action asking it, and keep nothing computed while it was found.
    pub(crate) fn order_dependent(&self, why: crate::Unsupported) {
        self.query_cache.order_dependent.set(self.query_cache.order_dependent.get() + 1);
        self.defer_refusal(why);
    }
    pub(crate) fn order_dependence(&self) -> u32 { self.query_cache.order_dependent.get() }
    /// Was no answer decided by an order of simultaneous timestamps since `then`?
    pub(crate) fn order_free_since(&self, then: u32) -> bool { self.order_dependence() == then }

    pub(crate) fn sharing_queries(&self) -> bool {
        #[cfg(test)]
        if crate::layers::trials_only() { return false; }
        if self.prospective.is_some() { return false; }
        let revision = self.layer_revision();
        match self.query_cache.revision.get() {
            Some(cached) => assert_eq!(cached, revision, "mutation bypassed query invalidation"),
            None => self.query_cache.revision.set(Some(revision)),
        }
        true
    }

    /// A completed ordinary layer world answers every object it contains at this cut.
    /// Entry views are deliberately excluded: the entrant contributes its own statics
    /// only when querying itself (614.12), so that world is not a shared current view.
    pub(crate) fn remember_world(&self, before: Option<Layer>, objects: Vec<(ObjId, Seen)>) {
        if !self.sharing_queries() || self.prospective.is_some() { return; }
        let cut = before.map(|l| l as u8);
        self.query_cache.values.borrow_mut().extend(objects.into_iter().map(|(o, seen)| ((o, cut), seen)));
    }

    pub(crate) fn query_seen(&self, object: ObjId, before: Option<Layer>, compute: impl FnOnce() -> Seen) -> Seen {
        if !self.sharing_queries() || self.obj(object).zone == crate::Zone::Gone {
            #[cfg(feature = "profile")]
            crate::profile::count(crate::profile::Count::QueryMissUnscoped, 1);
            return compute();
        }
        let key = (object, before.map(|l| l as u8));
        if let Some(seen) = self.query_cache.values.borrow().get(&key) {
            #[cfg(feature = "profile")]
            crate::profile::count(crate::profile::Count::QueryHits, 1);
            return seen.clone();
        }
        #[cfg(feature = "profile")]
        crate::profile::count(crate::profile::Count::QueryMissScoped, 1);
        // Evaluation may issue other queries; never retain a RefCell borrow across it.
        let decided = self.order_dependence();
        let seen = compute();
        if !self.order_free_since(decided) { return seen; }
        if self.query_cache.reads.borrow()[cut_index(before)].is_none() { self.subscribe_cut(before,Inputs::UNKNOWN); }
        self.query_cache.values.borrow_mut().insert(key, seen.clone());
        seen
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CardDef, Zone};
    fn game() -> (Game, ObjId) {
        let mut g = Game::new(vec![
            CardDef::new("Bear", "{0}", "Creature — Bear", "", Some((2,2))).unwrap(),
            CardDef::new("Anthem", "{0}", "Enchantment", "Creatures you control get +1/+1.", None).unwrap(),
        ], 2, 20);
        let bear = g.put(0, 0, Zone::Battlefield);
        (g, bear)
    }
    #[test]
    fn r613_queries_reuse_only_the_same_object_and_layer_view() {
        let (mut g, bear) = game();
        g.put(0, 1, Zone::Battlefield);
        assert_eq!(g.seen(bear, Some(Layer::Colors)).pt, Some((2,2)));
        assert_eq!(g.seen(bear, None).pt, Some((3,3)));
        let reused = g.query_seen(bear, None, || panic!("same query should be shared"));
        assert_eq!(reused.pt, Some((3,3)));
        assert_eq!(g.seen(bear, None).pt, Some((3,3)));
        assert!(!g.query_cache.values.borrow().is_empty());
        assert_eq!(g.query_seen(bear, None, || panic!("unchanged revision must reuse" )).pt, Some((3,3)));
        g.put(0, 1, Zone::Battlefield);
        assert_eq!(g.seen(bear, None).pt, Some((4,4)));
    }
    #[test]
    fn r113_6a_new_off_zone_defining_sources_are_seen_after_mutation_and_in_a_clone() {
        let normal = CardDef::new("Green", "{G}", "Creature — Bear", "", Some((2,2))).unwrap();
        let devoid = CardDef::new("Colorless", "{G}", "Creature — Eldrazi", "Devoid", Some((2,2))).unwrap();
        let mut g = Game::new(vec![normal, devoid], 2, 20);
        let green = g.put(0, 0, Zone::Hand);
        assert_eq!(g.seen(green,None).colors, vec![ir::Color::Green]);
        let mut copy = g.clone();
        let card = copy.put(0, 1, Zone::Library);
        assert!(copy.seen(card,None).colors.is_empty());
        assert_eq!(g.seen(green,None).colors, vec![ir::Color::Green]);
        let card = g.put(0, 1, Zone::Graveyard);
        assert!(g.seen(card,None).colors.is_empty());
    }

    #[test]
    fn r613_a_clone_never_inherits_cached_results() {
        let (g, bear) = game();
        assert_eq!(g.seen(bear, None).pt, Some((2,2)));
        let mut branch = g.clone();
        assert!(branch.query_cache.values.borrow().is_empty());
        branch.put(0, 1, Zone::Battlefield);
        assert_eq!(branch.seen(bear, None).pt, Some((3,3)));
        assert_eq!(g.seen(bear, None).pt, Some((2,2)));
    }
    #[test]
    fn r613_one_completed_world_supplies_other_objects_in_the_same_revision() {
        let (mut g, bear) = game();
        let other = g.put(1, 0, Zone::Battlefield);
        let anthem = g.put(0, 1, Zone::Battlefield);
        let expected = g.seen(other, None);
        assert_eq!(g.seen(bear, None).pt, Some((3,3)));
        assert_eq!(g.query_seen(other, None, || panic!("already computed by that world")), expected);
        assert!(g.query_seen(anthem, None, || panic!("already computed by that world")).pt.is_none());
    }
    #[test]
    fn a_world_extended_with_an_unlisted_fixture_is_not_shared() {
        let (mut g, bear) = game();
        g.put(0, 1, Zone::Battlefield);
        let stray = g.put(0, 0, Zone::Hand);
        g.players[0].hand.clear();
        g.seen(stray, None);
        assert!(!g.query_cache.values.borrow().contains_key(&(bear, None)));
        assert_eq!(g.seen(bear, None).pt, Some((3,3)));
    }
    #[test]
    fn r122_shared_absence_of_effects_keeps_each_objects_counters() {
        let (mut g, bear) = game();
        let other = g.put(0, 0, Zone::Battlefield);
        g.obj_mut(other).counters.insert("+1/+1".into(), 2);
        g.obj_mut(other).counters.insert("flying".into(), 1);
        assert_eq!(g.seen(bear, None).pt, Some((2,2)));
        assert!(g.no_layer_effects(None));
        let seen = g.seen(other, None);
        assert_eq!(seen.pt, Some((4,4)));
        assert!(seen.has(ir::Keyword::from_name("Flying").unwrap()));
        assert!(!g.query_cache.no_effects.borrow().is_empty());
        g.put(0, 1, Zone::Battlefield);
        assert_eq!(g.seen(other, None).pt, Some((5,5)));
    }
    #[test]
    fn completed_query_results_survive_unwind() {
        let (g, bear) = game();
        let error = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            g.seen(bear, None);
            panic!("test unwinding after a query");
        }));
        assert_eq!(error.unwrap_err().downcast_ref::<&str>(), Some(&"test unwinding after a query"));
        assert!(!g.query_cache.values.borrow().is_empty());
        assert_eq!(g.query_seen(bear, None, || panic!("completed answer survives unwind")).pt, Some((2,2)));
    }
    #[test]
    fn mutable_lending_drops_every_cached_proof_before_any_write() {
        let (mut g, bear) = game();
        let seen = g.seen(bear, None);
        g.query_cache.values.borrow_mut().insert((bear, None), seen);
        g.query_cache.no_effects.borrow_mut().push(None);
        *g.query_cache.off_zone_definitions.borrow_mut() = Some(vec![bear]);
        let before = g.layer_revision();
        let _unchanged_loan = g.state_mut();
        assert!(g.layer_revision() > before);
        assert!(g.query_cache.values.borrow().is_empty());
        assert!(g.query_cache.no_effects.borrow().is_empty());
        assert_eq!(*g.query_cache.off_zone_definitions.borrow(), None);
    }

    #[test]
    fn adopting_an_older_state_advances_revision_and_never_adopts_cached_answers() {
        let (mut g, bear) = game();
        let saved = g.clone();
        g.put(0, 1, Zone::Battlefield);
        assert_eq!(g.power(bear), Some(3));
        let before = g.layer_revision();
        g.adopt(saved);
        assert!(g.layer_revision() > before);
        assert!(g.query_cache.values.borrow().is_empty());
        assert_eq!(g.power(bear), Some(2));
    }

    #[test]
    fn repeated_reads_build_one_world_per_exact_cut() {
        let (mut g, bear) = game();
        g.put(0, 1, Zone::Battlefield);
        std::sync::Arc::make_mut(&mut g.defs)[1] = CardDef::new("Anthem", "{0}", "Enchantment", "Creatures you control get +1/+1 and have flying.", None).unwrap();
        let builds = || crate::layers::BUILDS.with(|n|n.get());
        let start = builds();
        assert_eq!(g.power(bear), Some(3));
        assert_eq!(builds(), start + 1);
        for _ in 0..3 {
            assert_eq!(g.power(bear), Some(3));
        }
        assert_eq!(builds(), start + 1, "no world between separate reads");
        g.seen(bear, Some(Layer::DefinePt));
        assert_eq!(builds(), start + 2, "a different cut evaluates from base");
        g.seen(bear, Some(Layer::DefinePt));
        assert_eq!(builds(), start + 2);
        let _loan = g.state_mut();
        assert_eq!(g.power(bear), Some(3));
        assert_eq!(builds(), start + 3, "even an unchanged loan drops the world");
    }

    #[test]
    fn prospective_views_bypass_every_ordinary_proof_family() {
        let (mut g, bear) = game();
        g.seen(bear,None);
        g.defining_sources(||vec![bear]);
        g.prospective = Some((bear,None));
        for _ in 0..2 {
            assert_eq!(g.query_seen(bear,None,||g.base_seen(bear)).pt,Some((2,2)));
            assert_eq!(g.defining_sources(||vec![]), Vec::<ObjId>::new());
            g.remember_no_effects(None);
            assert!(!g.no_layer_effects(None));
        }
        assert!(g.query_cache.values.borrow().is_empty());
        assert!(g.query_cache.no_effects.borrow().is_empty());
        assert_eq!(*g.query_cache.off_zone_definitions.borrow(),None);
        assert_eq!(g.query_cache.revision.get(),None);
    }

    #[test]
    fn retained_proofs_survive_reads_but_not_no_change_loans() {
        let (mut g, bear) = game();
        assert_eq!(g.defining_sources(||vec![bear]),vec![bear]);
        g.remember_no_effects(Some(Layer::Colors));
        assert_eq!(g.defining_sources(||panic!("retained discovery")),vec![bear]);
        assert!(g.no_layer_effects(Some(Layer::Colors)));
        assert!(!g.no_layer_effects(None));
        let _loan = g.state_mut();
        assert_eq!(g.defining_sources(||vec![]),Vec::<ObjId>::new());
        assert!(!g.no_layer_effects(Some(Layer::Colors)));
    }

    fn rule_game(text: &str) -> (Game,ObjId) {
        let def=CardDef::new("Reader","{0}","Creature — Bear",text,Some((2,2))).unwrap();
        crate::play::check(&def).unwrap();
        let mut g=Game::new(vec![def],2,20);
        let o=g.put(0,0,crate::Zone::Battlefield);
        (g,o)
    }
    #[test]
    fn typed_life_change_dirties_only_subscribed_cuts_and_keeps_negative_discovery() {
        let (mut g,o)=rule_game("This creature gets +1/+1 as long as your life total is greater than your starting life total.");
        assert_eq!(g.power(o),Some(2));
        let lower=g.seen(o,Some(Layer::DefinePt));
        assert!(g.no_layer_effects(Some(Layer::DefinePt)));
        let revision=g.layer_revision();
        *g.life_total_mut(0)=21;
        assert!(g.layer_revision()>revision);
        assert!(!g.query_cache.values.borrow().contains_key(&(o,None)));
        assert_eq!(g.query_seen(o,Some(Layer::DefinePt),||panic!("unaffected cut")),lower);
        assert!(g.no_layer_effects(Some(Layer::DefinePt)));
        assert_eq!(g.power(o),Some(3));
        for life in [20,21,20,21] {
            *g.life_total_mut(0)=life;
            assert_eq!(g.seen(o,None),crate::layers::footprint_tests::reference(||g.seen(o,None)));
        }
    }
    #[test]
    fn administrative_delivery_retains_known_answers_and_invalidates_unknown_reads() {
        let (mut g,o)=game(); g.put(0,1,crate::Zone::Battlefield);
        let expected=g.seen(o,None);
        let sources=g.defining_sources(||vec![]);
        let revision=g.layer_revision();
        assert!(g.take_observations(0).is_empty());
        assert!(g.layer_revision()>revision);
        assert_eq!(g.query_seen(o,None,||panic!("administrative input is unrelated")),expected);
        assert_eq!(g.defining_sources(||panic!("unaffected discovery")),sources);
        assert!(g.clone().query_cache.values.borrow().is_empty());
        g.subscribe_cut(None,Inputs::UNKNOWN);
        g.take_observations(0);
        assert!(g.query_cache.values.borrow().is_empty(),"unknown reads drop even on typed admin lending");
    }
    #[test]
    fn tapped_negative_domains_recompute_when_an_empty_match_becomes_nonempty() {
        let (mut g,o)=rule_game("This creature gets +1/+1 for each tapped creature you control.");
        assert_eq!(g.power(o),Some(2));
        *g.tapped_mut(o)=true;
        assert_eq!(g.power(o),Some(3));
        assert_eq!(g.seen(o,None),crate::layers::footprint_tests::reference(||g.seen(o,None)));
        *g.tapped_mut(o)=false;
        assert_eq!(g.power(o),Some(2));
    }
    #[test]
    fn new_counter_keyword_changes_negative_selectors_and_enabled_writers() {
        let (mut g,o)=rule_game("Creatures have haste as long as you control no creatures with flying.");
        let haste=ir::Keyword::from_name("Haste").unwrap();
        assert!(g.seen(o,None).has(haste));
        g.counter_fields_mut(o).0.insert("flying".into(),1);
        assert!(!g.seen(o,None).has(haste));
        assert_eq!(g.seen(o,None),crate::layers::footprint_tests::reference(||g.seen(o,None)));
        g.counter_fields_mut(o).0.clear();
        assert!(g.seen(o,None).has(haste));
    }
    #[test]
    fn potential_grants_subscribe_even_when_their_condition_currently_fails() {
        let (mut g,o)=rule_game("Creatures you control have \"This creature gets +1/+1 as long as your life total is greater than your starting life total.\"");
        let before=g.seen(o,None);
        assert_eq!(before.pt,Some((2,2)));
        assert_eq!(before.given.len(),1);
        *g.life_total_mut(0)=21;
        let after=g.seen(o,None);
        assert_eq!(after.pt,Some((3,3)));
        assert_eq!(before.given,after.given,"grant identity remains exact");
        assert_eq!(after,crate::layers::footprint_tests::reference(||g.seen(o,None)));
    }
    #[test]
    fn unrelated_mana_changes_and_repeated_dirtying_keep_cache_storage_bounded() {
        let (mut g,o)=rule_game("This creature gets +1/+1 as long as your life total is greater than your starting life total.");
        let expected=g.seen(o,None);
        g.add_mana(0,ir::mana::ManaSymbol::Colorless);
        assert_eq!(g.query_seen(o,None,||panic!("no mana input")),expected);
        for life in 0..100 {
            *g.life_total_mut(0)=life;
            g.seen(o,None);
            assert!(g.query_cache.values.borrow().len()<=1);
            assert_eq!(g.query_cache.reads.borrow().iter().flatten().count(),1);
        }
        let _generic=g.state_mut();
        assert_eq!(*g.query_cache.reads.borrow(),[None;8]);
        assert_eq!(*g.query_cache.reverse.borrow(),[0;64]);
        assert_eq!(g.query_cache.unknown.get(),0);
    }

    #[test]
    fn bounded_mixed_mutations_match_reference_at_every_cached_cut() {
        let texts = [
            "Creatures you control have \"This creature gets +1/+1 as long as your life total is greater than your starting life total.\"",
            "Creatures have haste as long as you control no creatures with flying.",
            "Creatures you control get +1/+1 for each tapped creature you control.",
        ];
        let mut defs=vec![CardDef::new("Bear","{0}","Creature — Bear","",Some((2,2))).unwrap()];
        for text in texts {
            let def=CardDef::new("Modifier","{0}","Enchantment",text,None).unwrap();
            assert!(def.characteristics.iter().any(|c|c.result.is_ok()));
            defs.push(def);
        }
        let cuts=[Some(Layer::Control),Some(Layer::Types),Some(Layer::Colors),
            Some(Layer::Abilities),Some(Layer::DefinePt),Some(Layer::SetPt),Some(Layer::ChangePt),None];
        for order in [[1,2,3],[3,2,1]] {
            let mut g=Game::new(defs.clone(),2,20);
            let a=g.put(0,0,Zone::Battlefield);
            let b=g.put(1,0,Zone::Battlefield);
            for d in order {g.put(0,d,Zone::Battlefield);}
            for step in 0..8 {
                match step {
                    0=>{},
                    1=>{*g.life_total_mut(0)=21;},
                    2=>{*g.tapped_mut(a)=true;},
                    3=>{g.counter_fields_mut(a).0.insert("flying".into(),1);},
                    4=>{g.add_mana(0,ir::mana::ManaSymbol::Colorless);g.take_observations(0);},
                    5=>{g.counter_fields_mut(a).0.clear();*g.life_total_mut(0)=20;},
                    6=>{g=g.clone();*g.tapped_mut(a)=false;},
                    7=>{g.obj_mut(a).controller=1;},
                    _=>unreachable!(),
                }
                for cut in cuts {
                    // Prime/persist an ordinary answer, then force a fresh exact world.
                    for id in [a,b] {
                        let actual=g.seen(id,cut);
                        let expected=crate::layers::footprint_tests::reference(||g.seen(id,cut));
                        assert_eq!(actual,expected,"order={order:?}, step={step}, cut={cut:?}");
                        assert_eq!(actual,g.seen(id,cut));
                    }
                }
            }
        }
    }

}
