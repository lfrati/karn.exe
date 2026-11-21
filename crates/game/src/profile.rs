//! Opt-in diagnostic counters. Never compiled into normal builds.
//! Timings are nested: inclusive columns overlap; exclusive columns do not.
//! Thread-local accounting keeps independent games free of a shared profiler lock.
use std::{cell::RefCell, time::Instant};
use serde_json::{json, Value};

#[derive(Clone, Copy)]
#[repr(usize)]
pub enum Scope { Respond, Projection, Seen, Discover, World, Dependencies, Triggers, Watchers, Serialize, CompileCatalog, SetupGame }
const SCOPES: [&str; 11] = ["respond", "projection", "seen", "discover_effects", "build_world", "dependencies", "triggers", "watchers", "serialize_write", "compile_catalog", "setup_game"];
#[derive(Clone, Copy)]
#[repr(usize)]
pub enum Count { Worlds, WorldObjects, Evaluations, CandidateObjects, DependencySnapshots, DependencyPairs, Graphs, SourceVisits, TriggerEvents, WatcherVisits, SeenInTriggers, SeenInProjection, AbilityGraphs, DefineGraphs, SetGraphs, ChangeGraphs, QueryHits, QueryMissScoped, QueryMissUnscoped, QueryInvalidations, QueryRetained, QueryDiscarded, TypedChanges, ProofsRetained, ProofsDiscarded, BaselineEvaluations, TrialEvaluations }
const COUNTS: [&str; 27] = ["worlds", "world_objects", "effect_evaluations", "effect_candidate_objects", "dependency_object_snapshots", "dependency_pair_tests", "dependency_graphs", "effect_source_visits", "trigger_events", "watcher_visits", "seen_in_triggers", "seen_in_projection", "ability_graphs", "define_pt_graphs", "set_pt_graphs", "change_pt_graphs", "query_hits", "query_misses_scoped", "query_misses_unscoped", "query_invalidations", "query_retained_answers", "query_discarded_answers", "typed_layer_changes", "query_retained_proofs", "query_discarded_proofs", "graph_baseline_evaluations", "exact_trial_evaluations"];
#[derive(Default, Clone, Copy)]
struct Timing { calls: u64, inclusive: u64, exclusive: u64 }
struct Stats { timing: [Timing; SCOPES.len()], counts: [u64; COUNTS.len()], stack: Vec<(usize, u64)> }
impl Default for Stats {
    fn default() -> Self { Self { timing: [Timing::default(); SCOPES.len()], counts: [0; COUNTS.len()], stack: Vec::new() } }
}
thread_local! { static STATS: RefCell<Stats> = RefCell::new(Stats::default()); }
pub struct Span { start: Instant }
pub fn span(scope: Scope) -> Span {
    STATS.with_borrow_mut(|s| {
        if matches!(scope, Scope::Seen) {
            for (outer, counter) in [(Scope::Triggers, Count::SeenInTriggers), (Scope::Projection, Count::SeenInProjection)] {
                if s.stack.iter().any(|(id,_)| *id == outer as usize) { s.counts[counter as usize] += 1; }
            }
        }
        s.stack.push((scope as usize, 0));
    });
    Span { start: Instant::now() }
}
impl Drop for Span {
    fn drop(&mut self) {
        let ns = self.start.elapsed().as_nanos() as u64;
        STATS.with_borrow_mut(|s| {
            let (id, children) = s.stack.pop().expect("balanced profiling scope");
            let t = &mut s.timing[id]; t.calls += 1; t.inclusive += ns;
            t.exclusive += ns.saturating_sub(children);
            if let Some((_, child)) = s.stack.last_mut() { *child += ns; }
        });
    }
}
pub fn count(which: Count, n: usize) { STATS.with_borrow_mut(|s| s.counts[which as usize] += n as u64); }
pub fn reset() { STATS.with_borrow_mut(|s| { assert!(s.stack.is_empty()); *s = Stats::default(); }); }
pub fn take() -> Value {
    STATS.with_borrow_mut(|s| {
        assert!(s.stack.is_empty());
        let timing: serde_json::Map<String, Value> = SCOPES.iter().zip(&s.timing).map(|(name,t)|
            ((*name).into(), json!({"calls":t.calls,"inclusive_ns":t.inclusive,"exclusive_ns":t.exclusive}))).collect();
        let counts: serde_json::Map<String, Value> = COUNTS.iter().zip(&s.counts).map(|(k,v)| ((*k).into(), json!(v))).collect();
        json!({"timing": timing, "counts": counts})
    })
}
