//! Synthetic scaling probes. Fixtures and validation stay outside measured work;
//! these supplement the immutable complete-game replays, never replace them.
use game::{CardDef, Game, Move, Zone};
use serde_json::{Value, json};
use std::{hint::black_box, time::Instant};

fn card(name: &str, types: &str, text: &str, pt: Option<(i32, i32)>) -> Result<CardDef, String> {
    let def = CardDef::new(name, "", types, text, pt)?;
    game::play::check(&def).map_err(|e| e.0)?;
    Ok(def)
}

fn summary(name: &str, size: usize, players: usize, operations: usize, mut seconds: Vec<f64>) -> Value {
    let samples = seconds.clone();
    seconds.sort_by(f64::total_cmp);
    let median = seconds[seconds.len() / 2];
    json!({"name":name,"objects":size,"players":players,"operations":operations,
        "runs_seconds":samples,"median_seconds":median,"ns_per_operation":median * 1e9 / operations as f64,
        "status":"passed"})
}

fn run(size: usize, players: usize, runs: usize, iterations: usize) -> Result<Vec<Value>, String> {
    let land = card("Benchmark land", "Land", "{T}: Add {G}.", None)?;
    let mut fixture = Game::new(vec![land], players, 40);
    let mut ids = Vec::new();
    for n in 0..size { ids.push(fixture.put(n % players, 0, Zone::Library)); }
    let mut rows = Vec::new();
    for write in [false, true] {
        let mut seconds = Vec::new();
        for _ in 0..runs {
            let began = Instant::now();
            for _ in 0..iterations {
                let mut branch = black_box(&fixture).clone();
                if write { branch.set_tapped(ids[0], true); }
                black_box(branch);
            }
            seconds.push(began.elapsed().as_secs_f64());
            if fixture.obj(ids[0]).tapped { return Err("clone mutation reached the original".into()); }
        }
        rows.push(summary(if write { "clone_and_first_write" } else { "clone" }, size, players, iterations, seconds));
    }

    // CR 613: four independent +1/+1 effects make the printed 2/2 a 6/6.
    let bear = card("Benchmark creature", "Creature — Bear", "", Some((2, 2)))?;
    let anthem = card("Benchmark anthem", "Enchantment", "Creatures you control get +1/+1.", None)?;
    let mut board = Game::new(vec![bear, anthem], players, 40);
    let creatures: Vec<_> = (0..size).map(|_| board.put(0, 0, Zone::Battlefield)).collect();
    for _ in 0..4 { board.put(0, 1, Zone::Battlefield); }
    for mode in ["cold", "warm", "after_mana", "after_counter"] {
        let mut seconds = Vec::new();
        #[cfg(feature="profile")]
        let mut profiles = Vec::new();
        for _ in 0..runs {
            let mut branch = board.clone();
            if mode != "cold" { for id in &creatures { black_box(branch.power(*id)); } }
            if mode == "after_mana" {branch.add_mana(0,ir::mana::ManaSymbol::Colorless);}
            if mode == "after_counter" {branch.put_counters(creatures[0],"+1/+1",1);}
            #[cfg(feature="profile")]
            game::profile::reset();
            let began = Instant::now();
            let total: i32 = creatures.iter().map(|id| black_box(branch.power(*id).unwrap())).sum();
            seconds.push(began.elapsed().as_secs_f64());
            #[cfg(feature="profile")]
            profiles.push(game::profile::take());
            if total != 6 * size as i32 + i32::from(mode == "after_counter") { return Err("layer probe has an incorrect characteristic result".into()); }
        }
        let row = summary(&format!("characteristics_{mode}_four_anthems"), size, players, size, seconds);
        #[cfg(feature="profile")]
        let row = {let mut row=row;row["internal_profiles"]=json!(profiles);row};
        rows.push(row);
    }

    // CR 613.8: the earlier flying effect depends on the later vigilance
    // grant. A timestamp-only pass would miss flying on every creature.
    let bear = card("Benchmark creature", "Creature — Bear", "", Some((2, 2)))?;
    let dependent = card("Benchmark dependent", "Enchantment", "Creatures with vigilance have flying.", None)?;
    let producer = card("Benchmark producer", "Enchantment", "Creatures you control have vigilance.", None)?;
    let mut board = Game::new(vec![bear, dependent, producer], players, 40);
    let creatures: Vec<_> = (0..size).map(|_| board.put(0, 0, Zone::Battlefield)).collect();
    board.put(0, 1, Zone::Battlefield);
    board.put(0, 2, Zone::Battlefield);
    for mode in ["cold", "warm", "after_mana", "after_producer_leaves"] {
        let mut seconds = Vec::new();
        #[cfg(feature="profile")]
        let mut profiles = Vec::new();
        for _ in 0..runs {
            let mut branch = board.clone();
            if mode != "cold" { for id in &creatures { black_box(branch.has(*id, "Flying")); } }
            if mode == "after_mana" {branch.add_mana(0,ir::mana::ManaSymbol::Colorless);}
            // Removing the producer is relevant to this dependency chain.
            if mode == "after_producer_leaves" {branch.move_object(*board.battlefield.last().unwrap(),Zone::Exile);}
            #[cfg(feature="profile")]
            game::profile::reset();
            let began = Instant::now();
            let flying = creatures.iter().filter(|id| black_box(branch.has(**id, "Flying"))).count();
            seconds.push(began.elapsed().as_secs_f64());
            #[cfg(feature="profile")]
            profiles.push(game::profile::take());
            if flying != if mode == "after_producer_leaves" {0} else {size} { return Err("dependency probe lost the granted flying ability".into()); }
        }
        let row = summary(&format!("characteristics_{mode}_dependency_chain"), size, players, size, seconds);
        #[cfg(feature="profile")]
        let row = {let mut row=row;row["internal_profiles"]=json!(profiles);row};
        rows.push(row);
    }

    // CR 605/603: explicitly activate twenty lands; one watcher triggers for
    // each tap for mana. The other sources subscribe only to attacks.
    let idle = card("Benchmark idle watcher", "Land", "{T}: Add {G}.\nWhenever a creature attacks, you gain 1 life.", None)?;
    let active = card("Benchmark mana watcher", "Enchantment", "Whenever a land is tapped for mana, you gain 1 life.", None)?;
    let mut board = Game::new(vec![idle, active], players, 40);
    let lands: Vec<_> = (0..size).map(|_| board.put(0, 0, Zone::Battlefield)).collect();
    board.put(0, 1, Zone::Battlefield);
    let activations = size.min(20);
    let mut seconds = Vec::new();
    for _ in 0..runs {
        let mut branch = board.clone();
        let began = Instant::now();
        for source in &lands[..activations] {
            branch.act(0, Move::Activate { source:*source, ability:0, targets:vec![], paying:vec![] }).map_err(|e| e.to_string())?;
        }
        seconds.push(began.elapsed().as_secs_f64());
        if branch.players[0].pool.total() != activations as u32 || branch.stack.len() != activations
            || !lands[..activations].iter().all(|id| branch.obj(*id).tapped)
        { return Err("trigger probe changed mana, taps or trigger multiplicity".into()); }
        if !board.stack.is_empty() || lands.iter().any(|id| board.obj(*id).tapped) {
            return Err("trigger probe changed the original branch".into());
        }
    }
    rows.push(summary("mana_actions_with_idle_watchers", size, players, activations, seconds));
    Ok(rows)
}

// Event count is independent of object count. The pending-log fixture has no
// delivered history; the retained fixture delivers once, then clears only the
// already-consumed raw log through its documented low-level fixture operation.
fn history(events: usize, players: usize, runs: usize, steps: usize) -> Result<Vec<Value>, String> {
    let land = card("History mana source", "Land", "{T}: Add {G}.", None)?;
    let watcher = card("History watcher", "Enchantment", "Whenever you gain life for the first time each turn, you gain 1 life.", None)?;
    let creature = card("History creature", "Creature", "This creature gets +1/+1 as long as you gained life this turn.", Some((2,2)))?;
    let mut rows = Vec::new();
    for watchers in [0,16] {
        for queries in [0,16] {
            let mut board = Game::new(vec![land.clone(),watcher.clone(),creature.clone()],players,40);
            let source = board.put(0,0,Zone::Battlefield);
            let creatures: Vec<_> = (0..queries).map(|_|board.put(0,2,Zone::Battlefield)).collect();
            // Initial history fixture: no gain replacement or active watcher exists.
            // Install equivalent committed gains in bulk; do not time O(N²) setup
            // through the conditional creature's repeated characteristic queries.
            board.players[0].life += events as i32;
            for _ in 0..events {board.log.push(game::event::Event::LifeGained {player:0,amount:1});}
            if watchers==0 && queries==0 {
                for retained in [false,true] {
                    let mut fixture = board.clone();
                    if retained {
                        fixture.act(0,Move::Pass).map_err(|e|e.to_string())?;
                        fixture.log.clear();
                    }
                    let mut seconds = Vec::new();
                    for _ in 0..runs {
                        let began = Instant::now();
                        for _ in 0..steps {black_box(black_box(&fixture).clone());}
                        seconds.push(began.elapsed().as_secs_f64());
                    }
                    let mut row = summary(if retained {"retained_history_clone"} else {"pending_log_clone"},1,players,steps,seconds);
                    row["events"]=json!(events);
                    row["log_events"]=json!(fixture.log.len());
                    rows.push(row);
                }
            }
            // No watcher is present during history setup. Later gains are not
            // the first (603.2), so measured moves must leave the stack empty.
            board.act(0,Move::Pass).map_err(|e|e.to_string())?;
            board.log.clear();
            for _ in 0..watchers {board.put(0,1,Zone::Battlefield);}
            board.priority=0; // fixture setup, outside timing
            for fork_every in [0,1,8] {
                let mut seconds=Vec::new();
                #[cfg(feature="profile")]
                let mut profiles=Vec::<Value>::new();
                for _ in 0..runs {
                    let mut branch=board.clone();
                    #[cfg(feature="profile")]
                    game::profile::reset();
                    let began=Instant::now();
                    for step in 0..steps {
                        if fork_every>0 && step%fork_every==0 {branch=black_box(&branch).clone();}
                        branch.set_tapped(source,false);
                        branch.gain_life(0,1).map_err(|e|e.0)?;
                        branch.act(0,Move::Activate {source,ability:0,targets:vec![],paying:vec![]}).map_err(|e|e.to_string())?;
                        for id in &creatures {black_box(branch.power(*id));}
                    }
                    seconds.push(began.elapsed().as_secs_f64());
                    #[cfg(feature="profile")]
                    profiles.push(game::profile::take());
                    if branch.players[0].life!=40+events as i32+steps as i32 || branch.players[0].pool.total()!=steps as u32
                        || !branch.stack.is_empty() || branch.decision.is_some()
                        || creatures.iter().any(|id|branch.power(*id)!=Some(3))
                    {return Err("history probe changed life, mana, first-occurrence triggers or conditional power".into());}
                    if board.players[0].life!=40+events as i32 || board.players[0].pool.total()!=0 || board.obj(source).tapped {
                        return Err("history probe changed its original branch".into());
                    }
                }
                let mut row=summary("history_actions",1+watchers+queries,players,steps,seconds);
                row["events"]=json!(events);row["watchers"]=json!(watchers);row["queries_per_step"]=json!(queries);row["fork_every"]=json!(fork_every);
                #[cfg(feature="profile")]
                if !profiles.is_empty() {row["internal_profiles"]=json!(profiles);}
                rows.push(row);
            }
        }
    }
    Ok(rows)
}

fn main() {
    let result = (|| -> Result<Value, String> {
        let args: Vec<_> = std::env::args().skip(1).collect();
        let [path] = args.as_slice() else { return Err("usage: game-scale CONFIG.json".into()); };
        let config: Value = serde_json::from_str(&std::fs::read_to_string(path).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
        let number = |key: &str, default| config.get(key).map_or(Ok(default), |n| n.as_u64().and_then(|n| usize::try_from(n).ok()).ok_or_else(|| format!("{key} must be a positive integer")));
        let (players, runs, iterations) = (number("players", 4)?, number("runs", 9)?, number("iterations", 200)?);
        let sizes: Vec<usize> = config["objects"].as_array().ok_or("objects must be an array")?.iter()
            .map(|n| n.as_u64().and_then(|n| usize::try_from(n).ok()).filter(|n| *n > 0 && *n <= 100_000).ok_or("object counts must be in 1..100000"))
            .collect::<Result<_,_>>()?;
        if players < 2 || players > 16 || runs == 0 || iterations == 0 || sizes.is_empty() { return Err("invalid players, runs, iterations or empty objects".into()); }
        let mut rows = Vec::new();
        for size in sizes {
            match run(size, players, runs, iterations) {
                Ok(mut values) => rows.append(&mut values),
                Err(error) => rows.push(json!({"objects":size,"status":"failed","error":error})),
            }
        }
        if let Some(events) = config.get("history_events") {
            let events=events.as_array().ok_or("history_events must be an array")?;
            let steps=number("history_steps",8)?;
            if steps==0 || steps>10_000 {return Err("history_steps must be in 1..10000".into());}
            for count in events {
                let count=count.as_u64().filter(|n|*n>0 && *n<=100_000).ok_or("history events must be in 1..100000")? as usize;
                match history(count,players,runs,steps) {
                    Ok(mut values)=>rows.append(&mut values),
                    Err(error)=>rows.push(json!({"events":count,"status":"failed","error":error})),
                }
            }
        }
        Ok(json!({"schema":1,"instrumented":cfg!(feature="profile"),"config":config,"workloads":rows,"workload_source":include_str!("game-scale.rs"),
            "sizes":"objects counts primary objects; the anthem, dependency and trigger fixtures add four, two and one effect sources respectively",
            "timing":"Single-threaded synthetic scaling probes. Setup and validation excluded; clone-and-first-write includes both operations. Cold characteristic fixtures cloned outside timing; warm caches primed outside timing. Mana actions include validation and trigger processing; no viewer projection or transport. These rates are not complete-game action throughput."}))
    })();
    match result { Ok(report) => println!("{report}"), Err(error) => { eprintln!("{error}"); std::process::exit(1); } }
}
