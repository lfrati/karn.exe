//! Measurement only: resident independently owned simulations, immutable shared catalogs.
//! Fixed supplied actions are replayed independently across workers.
use super::*;
use replay::{Prepared, execute, observe};
use std::sync::{
    Arc, Barrier,
    atomic::{AtomicBool, Ordering},
};
use std::time::{Duration, Instant};

fn rss() -> u64 {
    std::fs::read_to_string("/proc/self/status")
        .unwrap_or_default()
        .lines()
        .find_map(|l| l.strip_prefix("VmRSS:")?.split_whitespace().next()?.parse().ok())
        .unwrap_or(0)
}
fn cpu() -> f64 {
    // Linux process CPU includes all worker threads, sampled outside the hot loop.
    let text = std::fs::read_to_string("/proc/self/stat").unwrap_or_default();
    let fields: Vec<_> = text.rsplit_once(')').map(|(_, tail)| tail.split_whitespace().collect()).unwrap_or_default();
    let ticks = std::process::Command::new("getconf")
        .arg("CLK_TCK")
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .and_then(|s| s.trim().parse::<f64>().ok())
        .unwrap_or(100.);
    [11, 12].iter().map(|i| fields.get(*i).and_then(|s| s.parse::<f64>().ok()).unwrap_or(0.)).sum::<f64>() / ticks
}
pub(super) fn run(
    catalog: &Catalog,
    paths: &[String],
    workers: usize,
    copies: usize,
) -> Result<Value, String> {
    if paths.is_empty() || workers == 0 || copies == 0 {
        return Err("need paths and positive workers/copies".into());
    }
    let preparation = Instant::now();
    let fixtures: Vec<Prepared> = paths.iter().map(|p| replay::prepare(catalog, p)).collect::<Result<_, _>>()?;
    let prepared_seconds = preparation.elapsed().as_secs_f64();
    let idle_rss = rss();
    let barrier = Arc::new(Barrier::new(workers + 1));
    let done = Arc::new(AtomicBool::new(false));
    let sample_done = done.clone();
    let monitor = std::thread::spawn(move || {
        let mut peak = rss();
        while !sample_done.load(Ordering::Relaxed) {
            peak = peak.max(rss());
            std::thread::sleep(Duration::from_millis(5));
        }
        peak.max(rss())
    });
    let mut handles = Vec::new();
    for worker in 0..workers {
        let local = fixtures.clone();
        for (original,copy) in fixtures.iter().zip(&local) {
            assert!(Arc::ptr_eq(&original.initial.game().unwrap().defs,&copy.initial.game().unwrap().defs));
        }
        let paths = paths.to_vec();
        let barrier = barrier.clone();
        handles.push(std::thread::spawn(move||{
            barrier.wait();let mut rows=Vec::new();
            for copy in 0..copies {for (path,fixture) in paths.iter().zip(&local) {
                let mut sim=fixture.initial.clone();let began=Instant::now();
                let result=(||{for step in fixture.steps.iter(){execute(&mut sim,&step.command)?;observe(&mut sim,step.viewer)?;}let actual=sim.snapshot(None).map_err(|e|e.to_string())?;if actual!=fixture.expected{return Err("final snapshot differs".to_owned());}Ok(())})();
                rows.push(json!({"kind":"replay","worker":worker,"copy":copy,"path":path,"seconds":began.elapsed().as_secs_f64(),"actions":fixture.decisions,"commands":fixture.steps.len(),"ok":result.is_ok(),"error":result.err()}));
            }}rows
        }));
    }
    let replay_cpu_start = cpu();
    let replay_began = Instant::now();
    barrier.wait();
    let mut replays = Vec::new();
    for h in handles {
        replays.extend(h.join().map_err(|_| "worker panicked")?);
    }
    let replay_seconds = replay_began.elapsed().as_secs_f64();
    let replay_cpu = cpu() - replay_cpu_start;
    done.store(true, Ordering::Relaxed);
    let peak = monitor.join().map_err(|_| "monitor panicked")?;
    let high_water: Option<u64> = std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|s| s.lines().find_map(|l| l.strip_prefix("VmHWM:")?.split_whitespace().next()?.parse().ok()));
    let decisions: usize =
        replays.iter().filter(|r| r["ok"] == true).map(|r| r["actions"].as_u64().unwrap() as usize).sum();
    Ok(
        json!({"workers":workers,"copies_per_worker":copies,"preparation_seconds":prepared_seconds,"idle_prepared_rss_kib":idle_rss,"peak_rss_kib":peak,"total_process_peak_rss_kib":high_water,"incremental_peak_kib_per_worker":peak.saturating_sub(idle_rss) as f64/workers as f64,
        "replay":{"wall_seconds":replay_seconds,"cpu_seconds":replay_cpu,"decisions_per_second":decisions as f64/replay_seconds,"completions_per_second":replays.iter().filter(|r|r["ok"]==true).count() as f64/replay_seconds,"results":replays},
        "scope":"explicit-actions", "timing":"Independent games replay supplied actions and typed observations. Historical discovery requests are skipped during preparation. Includes fixture cloning and thread synchronization; excludes setup and player policy."}),
    )
}
