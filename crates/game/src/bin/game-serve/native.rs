//! Fixed matched streams through typed resident simulations. No JSON inside replay.
use super::*;
use replay::{execute, observe};
use std::sync::{Arc, Barrier, atomic::{AtomicBool, Ordering}};
use std::time::Instant;

fn cpu() -> f64 {
    let stat=std::fs::read_to_string("/proc/self/stat").unwrap();
    let fields:Vec<_>=stat.rsplit_once(')').unwrap().1.split_whitespace().collect();
    // Linux CLOCK_TICKS is queried once per boundary, outside replay.
    let ticks=std::process::Command::new("getconf").arg("CLK_TCK").output().unwrap();
    let ticks:f64=String::from_utf8(ticks.stdout).unwrap().trim().parse().unwrap();
    (fields[11].parse::<f64>().unwrap()+fields[12].parse::<f64>().unwrap())/ticks
}
fn rss() -> u64 {
    std::fs::read_to_string("/proc/self/status").unwrap().lines()
        .find_map(|s|s.strip_prefix("VmRSS:")?.split_whitespace().next()?.parse().ok()).unwrap()
}

pub(super) fn run(catalog: &Catalog, config: &Value) -> Result<Value,String> {
    let copies=config["copies"].as_u64().unwrap_or(256) as usize;
    let warmups=config["warmups"].as_u64().unwrap_or(3) as usize;
    let repeats=config["repeats"].as_u64().unwrap_or(3) as usize;
    if copies==0 || repeats==0 || warmups==0 {return Err("positive work/repeat limits required".into());}
    let paths:Vec<_>=list(config,"traces").iter().map(|p|p.as_str().ok_or("trace needs path")).collect::<Result<_,_>>()?;
    let mut prepared:Vec<_>=paths.iter().map(|p|replay::prepare(catalog,p)).collect::<Result<_,_>>()?;
    // Move explicitly declared read-only setup observations outside the timed stream.
    for (index,p) in prepared.iter_mut().enumerate() {
        let count=list(config,"setup_commands").get(index).and_then(Value::as_u64).unwrap_or(0) as usize;
        if count>p.steps.len() {return Err("setup_commands exceeds trace".into());}
        for step in &p.steps[..count] {
            if matches!(step.command,replay::Command::Act {..}) {return Err("setup_commands cannot include responses".into());}
            execute(&mut p.initial,&step.command)?;observe(&mut p.initial,step.viewer)?;
        }
        p.steps=Arc::new(p.steps[count..].to_vec());
    }
    let semantic:usize=list(config,"semantic_decisions").iter().map(|n|n.as_u64().unwrap_or(0) as usize).sum();
    if semantic==0 || list(config,"semantic_decisions").len()!=prepared.len() {return Err("need semantic counts for each trace".into());}
    for (count,p) in list(config,"semantic_decisions").iter().zip(&prepared) {
        if count.as_u64().is_none_or(|n|n==0 || n as usize>p.decisions) {return Err("semantic decisions must be positive and cannot exceed transitions".into());}
    }
    let transitions:usize=prepared.iter().map(|p|p.decisions).sum();
    let commands:usize=prepared.iter().map(|p|p.steps.len()).sum();
    let done=Arc::new(AtomicBool::new(false)); let stopped=done.clone();
    let sampler=std::thread::spawn(move || {
        let mut peak=0;
        while !stopped.load(Ordering::Relaxed) {peak=peak.max(rss());std::thread::sleep(std::time::Duration::from_millis(25));}
        peak.max(rss())
    });
    let result=(|| {
        let mut matrix=Vec::new();
        for workers in list(config,"workers") {
            let workers=workers.as_u64().filter(|n|(1..=256).contains(n)).ok_or("workers1..256")? as usize;
            let mut rows=Vec::new();
            for repetition in 0..warmups+repeats {
                let start=Barrier::new(workers+1);let end=Barrier::new(workers+1);let validate=Barrier::new(workers+1);
                let batch=std::thread::scope(|scope| -> Result<Value,String> {
                    let mut handles=Vec::new();
                    for _ in 0..workers {
                        let prepared=prepared.clone();let start=&start;let end=&end;let validate=&validate;
                        handles.push(scope.spawn(move || -> Result<f64,String> {
                            let mut finals=Vec::with_capacity(copies*prepared.len());let mut reset_seconds=0.;
                            start.wait();
                            let work=(|| {
                                for _ in 0..copies {for p in &prepared {
                                    let began=Instant::now();let mut sim=p.initial.clone();reset_seconds+=began.elapsed().as_secs_f64();
                                    for step in p.steps.iter() {execute(&mut sim,&step.command)?;observe(&mut sim,step.viewer)?;}
                                    finals.push((sim.snapshot(None).map_err(|e|e.to_string())?,p.expected.clone()));
                                }}
                                Ok::<(),String>(())
                            })();
                            end.wait();validate.wait();work?;
                            for (actual,expected) in finals {if actual!=expected {return Err("typed snapshot differs from validated stream".into());}}
                            Ok(reset_seconds)
                        }));
                    }
                    let before=cpu();let began=Instant::now();start.wait();end.wait();
                    let seconds=began.elapsed().as_secs_f64();let cpu_seconds=cpu()-before;
                    validate.wait();let mut resets=0.;
                    for h in handles {resets+=h.join().map_err(|_|"native worker panic")??;}
                    Ok(json!({"seconds":seconds,"cpu_seconds":cpu_seconds,"summed_worker_reset_seconds":resets,
                        "semantic_decisions":semantic*copies*workers,"transitions":transitions*copies*workers,
                        "typed_commands":commands*copies*workers,"semantic_decisions_s":(semantic*copies*workers) as f64/seconds,
                        "transitions_s":(transitions*copies*workers) as f64/seconds,"end_rss_kib":rss(),"all_final_snapshots_match":true}))
                })?;
                if repetition>=warmups {rows.push(batch);}
            }
            let mut rates:Vec<_>=rows.iter().map(|r|r["transitions_s"].as_f64().unwrap()).collect();rates.sort_by(f64::total_cmp);
            matrix.push(json!({"workers":workers,"runs":rows,"median_transitions_s":rates[rates.len()/2]}));
        }
        Ok::<Value,String>(json!({"matrix":matrix,"copies":copies,"warmups":warmups,"repeats":repeats,
            "semantic_per_mix":semantic,"transitions_per_mix":transitions,"commands_per_mix":commands,
            "timing":"Shared immutable catalog, independently owned simulations/caches. Predecoded same matched choices. Includes explicit responses, requests, typed viewer snapshots, disclosure delivery and initial-simulation clone/reset; excludes JSON/Python/transport/catalog/setup and final equality checks. Summed reset time is diagnostic, not subtracted from concurrent wall time.",
            "preparation":prepared.iter().zip(paths.iter()).map(|(p,path)|json!({"path":path,"seconds":p.preparation_seconds,"cold_setup":p.cold_setup,"shared_setup":p.shared_setup})).collect::<Vec<_>>()}))
    })();
    done.store(true,Ordering::Relaxed);let peak=sampler.join().map_err(|_|"RSS monitor panic")?;
    let mut result=result?;result["sampled_process_peak_rss_kib"]=json!(peak);
    if peak>config["rss_budget_kib"].as_u64().unwrap_or(8*1024*1024) {return Err(format!("sampled RSS budget exceeded: {peak} KiB"));}
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn typed_matched_runner_skips_discovery_and_preserves_action_snapshots() {
        let path=std::env::temp_dir().join(format!("matched-native-{}.jsonl",std::process::id()));
        std::fs::write(&path,concat!(
            "{\"cmd\":\"new\",\"players\":[{},{}],\"viewer\":\"asked\"}\n",
            "{\"cmd\":\"options\",\"session_id\":1,\"request_id\":1,\"player\":0,\"viewer\":\"asked\"}\n",
            "{\"cmd\":\"act\",\"session_id\":1,\"request_id\":1,\"player\":0,\"move\":{\"kind\":\"pass\"},\"viewer\":\"asked\"}\n"
        )).unwrap();
        let catalog=Catalog::new(BTreeMap::new());
        let mut config=json!({"traces":[path],"setup_commands":[0],"semantic_decisions":[1],"workers":[1,2],"copies":2,"warmups":1,"repeats":1});
        let report=run(&catalog,&config).unwrap();
        assert_eq!(report["commands_per_mix"],1);assert_eq!(report["transitions_per_mix"],1);
        for row in report["matrix"].as_array().unwrap() {assert_eq!(row["runs"][0]["all_final_snapshots_match"],true);}
        config["setup_commands"]=json!([1]);assert!(run(&catalog,&config).unwrap_err().contains("cannot include responses"));
        config["setup_commands"]=json!([0]);config["semantic_decisions"]=json!([2]);assert!(run(&catalog,&config).unwrap_err().contains("cannot exceed"));
        std::fs::remove_file(path).unwrap();
    }
}
