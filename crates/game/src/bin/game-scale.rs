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
    for warm in [false, true] {
        let mut seconds = Vec::new();
        for _ in 0..runs {
            let branch = board.clone();
            if warm { for id in &creatures { black_box(branch.power(*id)); } }
            let began = Instant::now();
            let total: i32 = creatures.iter().map(|id| black_box(branch.power(*id).unwrap())).sum();
            seconds.push(began.elapsed().as_secs_f64());
            if total != 6 * size as i32 { return Err("layer probe has an incorrect characteristic result".into()); }
        }
        rows.push(summary(if warm { "characteristics_warm_four_anthems" } else { "characteristics_cold_four_anthems" }, size, players, size, seconds));
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
    for warm in [false, true] {
        let mut seconds = Vec::new();
        for _ in 0..runs {
            let branch = board.clone();
            if warm { for id in &creatures { black_box(branch.has(*id, "Flying")); } }
            let began = Instant::now();
            let flying = creatures.iter().filter(|id| black_box(branch.has(**id, "Flying"))).count();
            seconds.push(began.elapsed().as_secs_f64());
            if flying != size { return Err("dependency probe lost the granted flying ability".into()); }
        }
        rows.push(summary(if warm { "characteristics_warm_dependency_chain" } else { "characteristics_cold_dependency_chain" }, size, players, size, seconds));
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
        Ok(json!({"schema":1,"config":config,"workloads":rows,"workload_source":include_str!("game-scale.rs"),
            "sizes":"objects counts primary objects; the anthem, dependency and trigger fixtures add four, two and one effect sources respectively",
            "timing":"Single-threaded synthetic scaling probes. Setup and validation excluded; clone-and-first-write includes both operations. Cold characteristic fixtures cloned outside timing; warm caches primed outside timing. Mana actions include validation and trigger processing; no viewer projection or transport. These rates are not complete-game action throughput."}))
    })();
    match result { Ok(report) => println!("{report}"), Err(error) => { eprintln!("{error}"); std::process::exit(1); } }
}
