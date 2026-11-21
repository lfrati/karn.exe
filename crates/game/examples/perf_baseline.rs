//! Small, dependency-free (beyond existing serde_json) core performance baseline.
//!
//! Run from the repository root:
//! cargo run --profile fast -p game --example perf_baseline -- --output .cache/architecture-baseline/fast.json
//! Use --help for workload controls. Use the same profile, machine and controls when comparing.
//! This measures fixed synthetic workloads, not full-match or player throughput.

use game::legal::Asked;
use game::simulation::Simulation;
use game::state::{Answer, Choice, Decision};
use game::{CardDef, Event, Game, Move, ObjId, Target, Zone};
use serde_json::{Value, json};
use std::hint::black_box;
use std::path::PathBuf;
use std::process::Command;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const HELP: &str = "Rust core baseline (run from repository root)\n\
  cargo run --profile fast -p game --example perf_baseline -- [options]\n\
  --output PATH    Write JSON (default: stdout)\n\
  --games N        Independent games per throughput sample (default: 128; minimum: 4)\n\
  --rounds N       Repeated timing samples (default: 5)\n\
  --clones N       Clone-and-drop operations per sample (default: 256)\n\
  --help          Show this text\n\
The fixed fixtures use no random choices or waiting on a player. Compilation and setup are timed\n\
separately. Throughput excludes initial cloning but includes thread startup/join for 1/2/4\n\
workers. Parallel games are independently initialized with one shared definition catalog.\n\
The raw turn workload retains history; managed turnover retires it through Simulation.\n\
Log capacity is only a shallow memory proxy; Linux RSS is whole-process memory including\n\
the allocator, definitions and harness, not allocated bytes.\n\
Run several times on an otherwise idle machine; short timings are noisy.";

struct Config {
    output: Option<PathBuf>,
    games: usize,
    rounds: usize,
    clones: usize,
}

fn config() -> Config {
    let mut c = Config { output: None, games: 128, rounds: 5, clones: 256 };
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        if arg == "--help" {
            println!("{HELP}");
            std::process::exit(0);
        }
        let value = args.next().expect("option requires a value; see --help");
        match arg.as_str() {
            "--output" => c.output = Some(value.into()),
            "--games" => c.games = value.parse().expect("--games requires a positive integer"),
            "--rounds" => c.rounds = value.parse().expect("--rounds requires a positive integer"),
            "--clones" => c.clones = value.parse().expect("--clones requires a positive integer"),
            _ => panic!("unknown option {arg}; see --help"),
        }
    }
    assert!(c.games >= 4 && c.rounds > 0 && c.clones > 0);
    c
}

fn command(program: &str, args: &[&str]) -> Option<String> {
    let output = Command::new(program).args(args).output().ok()?;
    output.status.success().then(|| String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

fn rss_kib() -> Option<u64> {
    std::fs::read_to_string("/proc/self/status")
        .ok()?
        .lines()
        .find_map(|line| line.strip_prefix("VmRSS:")?.split_whitespace().next()?.parse().ok())
}

fn metric(name: &str, unit: &str, operations: usize, samples: Vec<Duration>) -> Value {
    let raw: Vec<f64> = samples.iter().map(Duration::as_secs_f64).collect();
    let mut sorted = raw.clone();
    sorted.sort_by(f64::total_cmp);
    let median = sorted[sorted.len() / 2];
    json!({
        "name": name, "unit": unit, "operations_per_sample": operations,
        "sample_seconds": raw, "median_seconds": median,
        "median_ns_per_operation": median * 1e9 / operations as f64,
        "operations_per_second": operations as f64 / median,
    })
}

fn repeat(rounds: usize, mut work: impl FnMut()) -> Vec<Duration> {
    (0..rounds)
        .map(|_| {
            let start = Instant::now();
            work();
            start.elapsed()
        })
        .collect()
}

fn definitions() -> Vec<CardDef> {
    let defs = vec![
        CardDef::new("Mountain", "", "Basic Land — Mountain", "({T}: Add {R}.)", None).unwrap(),
        CardDef::new("Grizzly Bears", "{1}{G}", "Creature — Bear", "", Some((2, 2))).unwrap(),
        CardDef::new("Lightning Bolt", "{R}", "Instant", "Lightning Bolt deals 3 damage to any target.", None).unwrap(),
        CardDef::new("Optional life", "{0}", "Sorcery", "You may gain 1 life. You gain 2 life.", None).unwrap(),
    ];
    for def in &defs {
        game::play::check(def).unwrap();
    }
    defs
}

struct Fixture {
    base: Game,
    lands: Vec<ObjId>,
    spells: Vec<ObjId>,
}

fn fixture(defs: Arc<Vec<CardDef>>) -> Fixture {
    let mut base = Game::new(defs, 2, 20);
    let mut lands = Vec::new();
    for p in 0..2 {
        for _ in 0..4 {
            let land = base.put(p, 0, Zone::Battlefield);
            if p == 0 {
                lands.push(land);
            }
        }
        for _ in 0..4 {
            base.put(p, 1, Zone::Battlefield);
        }
        for i in 0..60 {
            base.put(p, i % 2, Zone::Library);
        }
    }
    let spells = (0..4).map(|_| base.put(0, 2, Zone::Hand)).collect();
    Fixture { base, lands, spells }
}

// Four real casts and resolutions, 16 accepted commands, without enumerating legal options.
fn advance(g: &mut Game, lands: &[ObjId], spells: &[ObjId]) {
    for (&land, &card) in lands.iter().zip(spells) {
        g.act(0, Move::Activate { source: land, ability: 0, targets: vec![], paying: vec![] }).unwrap();
        g.act(0, Move::Cast { card, targets: vec![vec![Target::Player(1)]] }).unwrap();
        g.act(0, Move::Pass).unwrap();
        g.act(1, Move::Pass).unwrap();
    }
    assert_eq!(g.players[1].life, 8);
    assert!(g.stack.is_empty() && g.decision.is_none());
    black_box(g);
}

fn paused(base: &Game) -> Game {
    let mut g = base.clone();
    let card = g.put(0, 3, Zone::Hand);
    g.act(0, Move::Cast { card, targets: vec![] }).unwrap();
    g.act(0, Move::Pass).unwrap();
    g.act(1, Move::Pass).unwrap();
    assert!(matches!(g.decision, Some(Decision::Choose { choice: Choice::May, .. })));
    assert_eq!(g.players[0].life, 20);
    g
}

fn memory_snapshot(g: &Game) -> Value {
    json!({
        "turn": g.turn, "events": g.log.len(), "event_capacity": g.log.capacity(),
        "events_committed": g.log.end().0, "events_retired": g.log.start().0,
        "retained_object_payloads": g.object_count(), "object_slots": g.object_slots(),
        "event_capacity_shallow_bytes": g.log.capacity() * size_of::<Event>(),
        "game_inline_bytes": size_of::<Game>(),
        // Exact inline sizes expose representation growth even when RSS reuses the
        // allocator's earlier pages. Vec payloads and shared definitions are excluded.
        "object_inline_bytes": size_of::<game::state::Object>(),
        "characteristics_inline_bytes": size_of::<game::layers::Seen>(),
        "ability_key_inline_bytes": size_of::<game::binding::AbilityKey>(),
        "visible_zone_objects": g.battlefield.len() + g.exile.len() + g.stack.len()
            + g.players.iter().map(|p| p.library.len() + p.hand.len() + p.graveyard.len()).sum::<usize>(),
        "process_rss_kib": rss_kib(),
    })
}

fn history(base: &Game) -> Value {
    let mut g = base.clone();
    // 572 cards per library in total, enough for the fixed 512-turn pass-only workload.
    for p in 0..2 {
        for _ in 0..512 {
            g.put(p, 0, Zone::Library);
        }
    }
    let mut checkpoints = Vec::new();
    let mut commands = 0;
    let start = Instant::now();
    for turn in [1, 16, 64, 256, 512] {
        while g.turn < turn {
            let (player, mv) = match &g.decision {
                None => (g.priority, Move::Pass),
                Some(Decision::Attackers { player }) => (*player, Move::Attack(vec![])),
                Some(Decision::Blockers { player }) => (*player, Move::Block(vec![])),
                Some(Decision::Discard { player, count }) => {
                    (*player, Move::Discard(g.players[*player].hand[..*count].to_vec()))
                }
                other => panic!("unexpected history-workload decision: {other:?}"),
            };
            g.act(player, mv).unwrap();
            commands += 1;
            assert!(!g.finished());
        }
        let mut point = memory_snapshot(&g);
        point["accepted_commands"] = json!(commands);
        point["clone_and_drop"] = metric(
            "history_clone_and_drop",
            "clone",
            16,
            repeat(3, || {
                for _ in 0..16 {
                    drop(black_box(g.clone()));
                }
            }),
        );
        checkpoints.push(point);
    }
    json!({"workload": "512 turns; pass priority, no attacks, discard first hand cards at cleanup; retain all history",
        "wall_seconds_including_checkpoints": start.elapsed().as_secs_f64(), "checkpoints": checkpoints})
}


// A constant live state with four zone changes each cycle: the creature blinks and the
// resolving spell returns itself to hand (400.7, 608.2m). Using a spell avoids measuring
// a growing current-turn activation ledger in this one long main phase.
fn managed_turnover(c: &Config) -> Value {
    let spell = CardDef::new("Turnover", "{0}", "Instant",
        "Exile target creature you control, then return that card to the battlefield under your control. Return this spell to its owner's hand.", None).unwrap();
    game::play::check(&spell).unwrap();
    let bear = CardDef::new("Bear", "{0}", "Creature — Bear", "", Some((2, 2))).unwrap();
    let mut game = Game::new(vec![spell, bear], 2, 20);
    game.put(0, 0, Zone::Hand);
    game.put(0, 1, Zone::Battlefield);
    let mut simulation = Simulation::new(game);
    let mut cycles = 0;
    let mut checkpoints = Vec::new();
    let mut peak_objects = 0;
    let mut peak_slots = 0;
    let mut peak_events = 0;
    let start = Instant::now();
    for through in [0, 16, 64, 256, 1024, 4096] {
        let before = cycles;
        let interval = Instant::now();
        while cycles < through {
            let game = simulation.game().unwrap();
            let (card, creature) = (game.players[0].hand[0], game.battlefield[0]);
            for action in [Move::Cast { card, targets: vec![vec![Target::Object(creature)]] }, Move::Pass, Move::Pass] {
                let request = simulation.request().unwrap().unwrap();
                assert_eq!(request.question, Asked::Priority);
                let (id, player) = (request.id, request.player);
                simulation.respond(id, player, action).unwrap();
                // A host takes every seat's disclosures independently of future decisions.
                for seat in 0..2 { drop(black_box(simulation.take_observations(seat).unwrap())); }
                let game = simulation.game().unwrap();
                peak_objects = peak_objects.max(game.object_count());
                peak_slots = peak_slots.max(game.object_slots());
                peak_events = peak_events.max(game.log.len());
            }
            let game = simulation.game().unwrap();
            assert_eq!((game.battlefield.len(), game.players[0].hand.len()), (1, 1));
            assert!(game.stack.is_empty() && game.exile.is_empty() && game.decision.is_none());
            assert!(game.players.iter().all(|player| player.graveyard.is_empty() && player.library.is_empty()));
            assert_ne!(game.battlefield[0], creature);
            assert_ne!(game.players[0].hand[0], card);
            cycles += 1;
        }
        let interval_seconds = interval.elapsed().as_secs_f64();
        // Keep the request shape equal at every checkpoint; no legal-option search.
        assert_eq!(simulation.request().unwrap().unwrap().question, Asked::Priority);
        let game = simulation.game().unwrap();
        let mut point = memory_snapshot(game);
        point["turnover_cycles"] = json!(cycles);
        point["accepted_commands"] = json!(cycles * 3);
        point["interval_accepted_commands"] = json!((cycles - before) * 3);
        point["interval_seconds"] = json!(interval_seconds);
        point["interval_ns_per_command"] = if cycles == before { Value::Null }
            else { json!(interval_seconds * 1e9 / ((cycles - before) * 3) as f64) };
        point["game_clone_and_drop"] = metric("managed_game_clone_and_drop", "clone", c.clones, repeat(c.rounds, || {
            for _ in 0..c.clones { drop(black_box(game.clone())); }
        }));
        point["simulation_clone_and_drop"] = metric("managed_simulation_clone_and_drop", "clone", c.clones, repeat(c.rounds, || {
            for _ in 0..c.clones { drop(black_box(simulation.clone())); }
        }));
        checkpoints.push(point);
    }
    json!({
        "workload": "4096 zero-cost blink/self-return casts in one main phase; two live cards; three correlated Simulation commands and four zone changes per cycle; consume each seat's disclosures after every response",
        "boundary": "Simulation::respond retires delivered events and collects unreferenced departed incarnations; raw Game APIs are never used after setup",
        "checkpoints": checkpoints,
        "peak_at_response_boundaries": {"retained_object_payloads": peak_objects, "object_slots": peak_slots, "events": peak_events},
        "wall_seconds_including_checkpoints": start.elapsed().as_secs_f64(),
        "limits": "Fixed live zones isolate incidental history. This is not elapsed-turn coverage, a paused-procedure bound, or a bound for genuinely growing live references. Timings include request handling, assertions, per-seat disclosure draining and count inspection."
    })
}

fn resident_population(base: &Game, count: usize) -> Value {
    let before = rss_kib();
    let games: Vec<Game> = (0..count).map(|_| base.clone()).collect();
    let during = rss_kib();
    black_box(&games);
    json!({"games": count, "process_rss_before_kib": before, "process_rss_while_alive_kib": during,
        "one_game_proxy": memory_snapshot(&games[0])})
}

fn main() {
    let c = config();
    let revision = command("git", &["rev-parse", "HEAD"]);
    let status = command("git", &["status", "--short"]);
    let compile = Instant::now();
    let defs = Arc::new(definitions());
    let compile_seconds = compile.elapsed().as_secs_f64();
    let setup = Instant::now();
    let f = fixture(defs.clone());
    let paused = paused(&f.base);
    let setup_seconds = setup.elapsed().as_secs_f64();

    // Untimed warm-up and executable sanity checks for the fixed scripts.
    advance(&mut f.base.clone(), &f.lands, &f.spells);
    let mut resumed = paused.clone();
    resumed.act(0, Move::Choose(Answer::Yes)).unwrap();
    assert_eq!(resumed.players[0].life, 23);
    assert!(resumed.decision.is_none() && resumed.stack.is_empty());

    let mut metrics = Vec::new();
    let samples = (0..c.rounds)
        .map(|_| {
            let mut games: Vec<_> = (0..c.games).map(|_| f.base.clone()).collect();
            let start = Instant::now();
            for g in &mut games {
                advance(g, &f.lands, &f.spells);
            }
            start.elapsed()
        })
        .collect();
    metrics.push(metric("direct_advancement_precloned", "accepted_command", c.games * 16, samples));

    // Each authored pass is a correlated request and response through the simulation.
    let passes = {
        // as many as the fixture allows before it asks something other than priority
        let mut probe = Simulation::new(f.base.clone());
        let mut passes = 0;
        while passes < 16 {
            let request = probe.request().unwrap().unwrap();
            if request.question != Asked::Priority { break; }
            let (id, player) = (request.id, request.player);
            probe.respond(id, player, Move::Pass).unwrap();
            passes += 1;
        }
        assert!(passes > 0);
        passes
    };
    let samples = (0..c.rounds)
        .map(|_| {
            let mut simulations: Vec<_> = (0..c.games).map(|_| Simulation::new(f.base.clone())).collect();
            let start = Instant::now();
            for simulation in &mut simulations {
                for _ in 0..passes {
                    let request = simulation.request().unwrap().unwrap();
                    let (id, player) = (request.id, request.player);
                    simulation.respond(id, player, Move::Pass).unwrap();
                }
            }
            start.elapsed()
        })
        .collect();
    metrics.push(metric("simulation_priority_request_and_pass", "pass", c.games * passes, samples));
    for (name, base) in [("active_clone_and_drop", &f.base), ("paused_clone_and_drop", &paused)] {
        metrics.push(metric(
            name,
            "clone",
            c.clones,
            repeat(c.rounds, || {
                for _ in 0..c.clones {
                    drop(black_box(base.clone()));
                }
            }),
        ));
    }
    let samples = (0..c.rounds)
        .map(|_| {
            let mut games: Vec<_> = (0..c.games).map(|_| paused.clone()).collect();
            let start = Instant::now();
            for g in &mut games {
                g.act(0, Move::Choose(Answer::Yes)).unwrap();
                assert_eq!(g.players[0].life, 23);
                assert!(g.decision.is_none() && g.stack.is_empty());
            }
            start.elapsed()
        })
        .collect();
    metrics.push(metric("paused_resume_precloned", "answer", c.games, samples));
    let mut parallel = Vec::new();
    for workers in [1, 2, 4] {
        let samples = (0..c.rounds)
            .map(|_| {
                let mut games: Vec<_> = (0..c.games).map(|_| fixture(defs.clone()).base).collect();
                let start = Instant::now();
                std::thread::scope(|scope| {
                    for chunk in games.chunks_mut(c.games.div_ceil(workers)) {
                        let (lands, spells) = (&f.lands, &f.spells);
                        scope.spawn(move || {
                            for g in chunk {
                                advance(g, lands, spells);
                            }
                        });
                    }
                });
                start.elapsed()
            })
            .collect();
        let mut result = metric(
            "independently_initialized_shared_catalog_games_thread_start_and_join_included",
            "four_spell_game_workload",
            c.games,
            samples,
        );
        result["workers"] = json!(workers);
        result["commands_per_game"] = json!(16);
        parallel.push(result);
    }
    let active_memory = resident_population(&f.base, c.games);
    let paused_memory = resident_population(&paused, c.games);
    let history = history(&f.base);
    let managed_turnover = managed_turnover(&c);
    let report = json!({
        "schema_version": 1,
        "source": {"revision": revision, "git_status_short": status,
            "note": "revision plus working tree status; commit the harness and engine changes for an exact checkout reproduction"},
        "run": {"unix_seconds": SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
            "argv": std::env::args().collect::<Vec<_>>(), "cwd": std::env::current_dir().ok(),
            "rustc": command("rustc", &["--version"]), "os": std::env::consts::OS,
            "arch": std::env::consts::ARCH, "available_parallelism": std::thread::available_parallelism().ok().map(|n| n.get()),
            "debug_assertions": cfg!(debug_assertions),
            "build_command": "cargo run --profile fast -p game --example perf_baseline -- --output .cache/architecture-baseline/fast.json",
            "profile_note": "argv records actual executable/profile directory; build_command is the documented reproduction command"},
        "config": {"games": c.games, "rounds": c.rounds, "clones": c.clones,
            "randomness": "none: fixed setup and commands; Game::new default RNG is never used"},
        "fixture": "2 players at 20 life; each has 4 Mountains, 4 Bears, 60 library cards; player 0 has 4 Bolts; paused variant adds optional life spell",
        "definitions_parse_compile_and_admission_seconds": compile_seconds,
        "fixture_and_paused_setup_seconds": setup_seconds,
        "metrics": metrics, "parallel": parallel,
        "memory": {"active_population": active_memory, "paused_population": paused_memory,
            "limits": "RSS is whole process, includes allocator retention and shared definitions, and may reuse earlier allocations. Event-capacity bytes exclude nested event allocations, objects, continuations and all other heaps. Neither is total bytes per game or allocation counts."},
        "history": history,
        "managed_turnover": managed_turnover,
        "limits": ["Synthetic microbenchmarks, not representative full matches or a speed claim.",
            "No waiting on a player, observation serialization, inference batching or scheduler is measured.",
            "Parallel games independently initialize identical mutable states from one shared immutable catalog, outside timing. They use a static partition across scoped threads, not an engine scheduler.",
            "Direct/parallel/resume samples exclude setup, cloning and final game destruction; clone samples include destruction.",
            "Lightweight correctness assertions and loop overhead are included. Samples are sequential and share allocator state."]
    });
    let json = serde_json::to_string_pretty(&report).unwrap();
    if let Some(path) = c.output {
        if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(&path, format!("{json}\n")).unwrap();
        eprintln!("Baseline written to {}", path.display());
    } else {
        println!("{json}");
    }
}
