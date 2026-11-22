//! Replay already-chosen actions through the typed player boundary. JSON decoding, card
//! compilation and game initialization happen before timing; responses, requests,
//! seat snapshots and disclosure delivery remain inside. No player or pipe runs in the loop.
use super::*;
use game::state::{Choice, Decision};
use std::{hint::black_box, time::Instant};

#[derive(Clone)]
pub(super) enum Command {
    Act { id: RequestId, player: usize, action: Move, random: Option<(Vec<game::start::RandomOutcome>, bool)> },
    State,
}
#[derive(Clone)]
pub(super) struct Step { pub(super) command: Command, pub(super) viewer: Option<usize> }

pub(super) fn execute(sim: &mut Simulation, command: &Command) -> Result<(), String> {
    #[cfg(feature = "profile")]
    let _profile = matches!(command, Command::Act { .. }).then(|| game::profile::span(game::profile::Scope::Respond));
    match command {
        Command::Act { id, player, action, random } => match random {
            Some((orders, keep_unused)) => sim.respond_with_random(*id, *player, action.clone(), orders.clone(), *keep_unused).map(|_| ()),
            None => sim.respond(*id, *player, action.clone()),
        }.map_err(|e| e.to_string())?,
        Command::State => {}
    }
    Ok(())
}
pub(super) fn observe(sim: &mut Simulation, viewer: Option<usize>) -> Result<(), String> {
    #[cfg(feature = "profile")]
    let _profile = game::profile::span(game::profile::Scope::Projection);
    black_box(sim.request().map_err(|e| e.to_string())?);
    black_box(sim.snapshot(viewer).map_err(|e| e.to_string())?);
    if let Some(seat) = viewer { black_box(sim.take_observations(seat).map_err(|e| e.to_string())?); }
    Ok(())
}

#[derive(Clone)]
pub(super) struct Prepared {
    pub(super) initial: Simulation,
    pub(super) steps: std::sync::Arc<Vec<Step>>,
    pub(super) expected: std::sync::Arc<game::snapshot::Snapshot>,
    pub(super) decisions: usize,
    pub(super) skipped_discovery: usize,
    pub(super) preparation_seconds: f64,
    pub(super) cold_setup: f64,
    pub(super) shared_setup: f64,
}
pub(super) fn prepare(catalog: &Catalog, path: &str) -> Result<Prepared,String> {
    let preparation = Instant::now();
    let lines = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    let inputs: Vec<Value> = lines.lines().filter(|s| !s.trim().is_empty()).map(serde_json::from_str).collect::<Result<_, _>>().map_err(|e| e.to_string())?;
    let setup = inputs.first().ok_or("empty trace")?;
    let start = || match setup["cmd"].as_str() {
        Some("start") => start_game(catalog, setup),
        Some("new") => new_game(catalog, setup),
        _ => Err("trace must begin with start or new".into()),
    };
    let cold = Instant::now();
    let game = start()?;
    let cold_setup = cold.elapsed().as_secs_f64();
    let warm = Instant::now();
    let warmed = start()?;
    let shared_setup = warm.elapsed().as_secs_f64();
    assert!(std::sync::Arc::ptr_eq(&game.defs, &warmed.defs));
    drop(warmed);
    let mut initial = Simulation::new(game);
    let chooser = initial.request().map_err(|e| e.to_string())?.map(|r| r.player);
    let seat = read_viewer(initial.game().map_err(|e| e.to_string())?, setup)?.seat(chooser);
    observe(&mut initial, seat)?;
    let mut sim = initial.clone();
    let mut steps = Vec::new();
    let mut decisions = 0;
    let mut skipped_discovery = 0;
    for input in &inputs[1..] {
        let id = || input["request_id"].as_u64().map(RequestId).ok_or("missing request_id");
        let player = || input["player"].as_u64().map(|p| p as usize).ok_or("missing player");
        let command = match input["cmd"].as_str() {
            Some("act") => {
                let player = player()?;
                let g = sim.game().map_err(|e| e.to_string())?;
                let pool = &g.players.get(g.decision_actor()).ok_or("no such player")?.pool;
                let offered = match &g.decision {
                    Some(Decision::Choose { choice: Choice::Replacement { among, .. }, .. }) => among.as_slice(),
                    _ => &[],
                };
                let action = read_payment_move(g, player, input.get("move").ok_or("missing move")?, pool, offered)?;
                // Authenticate evidence only when it selects retained audit behavior.
                // Other audit metadata is outside this supplied-action benchmark.
                // Quotes and JSON stay in preparation, outside timed execution.
                let action=if input.pointer("/payment_expect/suspended/stage").and_then(Value::as_str)==Some("mana_choice") {
                    let actual=payment_quote(g,player,&input["move"])?;
                    if actual["complete"]!=false || actual.pointer("/suspended/stage").and_then(Value::as_str)!=Some("mana_choice")
                        || actual!=input["payment_expect"] {
                        return Err("payment quote changed or is incomplete; action was not applied".into());
                    }
                    Move::AuditPayment(Box::new(action))
                } else {action};
                if input.get("random_outcomes").is_some() && input.get("shuffles").is_some() {
                    return Err("random_outcomes and shuffles are exclusive".into());
                }
                let random = if let Some(outcomes) = input.get("random_outcomes") {
                    Some((read_random_outcomes(outcomes)?, input["random_outcomes_keep_unused"] == true))
                } else {
                    input.get("shuffles").map(|orders| read_shuffles(orders).map(|orders|
                        (orders.into_iter().map(Into::into).collect(), input["shuffles_keep_unused"] == true))).transpose()?
                };
                decisions += 1;
                Command::Act { id: id()?, player, action, random }
            }
            Some("options") => { skipped_discovery += 1; continue; },
            Some("state") => Command::State,
            _ => return Err("typed replay supports one setup followed by act/options/state".into()),
        };
        if !matches!(command, Command::State) && input["session_id"] != 1 {
            return Err("typed replay requires the first session".into());
        }
        execute(&mut sim, &command)?;
        let chooser = sim.request().map_err(|e| e.to_string())?.map(|r| r.player);
        let seat = read_viewer(sim.game().map_err(|e| e.to_string())?, input)?.seat(chooser);
        observe(&mut sim, seat)?;
        steps.push(Step { command, viewer: seat });
    }
    let expected = sim.snapshot(None).map_err(|e| e.to_string())?;
    let preparation_seconds = preparation.elapsed().as_secs_f64();
    Ok(Prepared {initial,steps:std::sync::Arc::new(steps),expected,decisions,skipped_discovery,preparation_seconds,cold_setup,shared_setup})
}
pub(super) fn run(catalog: &Catalog, path: &str, runs: usize) -> Result<Value, String> {
    if runs == 0 { return Err("--runs must be positive".into()); }
    let Prepared {initial,steps,expected,decisions,skipped_discovery,preparation_seconds,cold_setup,shared_setup} = prepare(catalog,path)?;
    let mut seconds = Vec::new();
    let mut command_ms: BTreeMap<&str, Vec<f64>> = BTreeMap::new();
    for kind in ["act", "state"] { command_ms.insert(kind, Vec::with_capacity(steps.len() * runs)); }
    #[cfg(feature = "profile")]
    let mut profiles = Vec::new();
    for _ in 0..runs {
        let mut sim = initial.clone();
        #[cfg(feature = "profile")]
        game::profile::reset();
        let began = Instant::now();
        for step in steps.iter() {
            let command_began = Instant::now();
            execute(&mut sim, &step.command)?;
            observe(&mut sim, step.viewer)?;
            let kind = match step.command { Command::Act { .. } => "act", Command::State => "state" };
            command_ms.get_mut(kind).unwrap().push(command_began.elapsed().as_secs_f64() * 1000.0);
        }
        seconds.push(began.elapsed().as_secs_f64());
        #[cfg(feature = "profile")]
        profiles.push(game::profile::take());
        let actual = sim.snapshot(None).map_err(|e| e.to_string())?;
        if actual != expected { return Err("typed replay final snapshot changed".into()); }
    }
    let mut sorted = seconds.clone(); sorted.sort_by(f64::total_cmp);
    let median = if runs % 2 == 1 { sorted[runs/2] } else { (sorted[runs/2-1] + sorted[runs/2])/2.0 };
    let latency: BTreeMap<_, _> = command_ms.into_iter().filter(|(_, v)| !v.is_empty()).map(|(kind, mut samples)| {
        samples.sort_by(f64::total_cmp);
        (kind, json!({"median_ms":samples[samples.len()/2], "p99_ms":samples[((samples.len() as f64 * 0.99).ceil() as usize - 1).min(samples.len()-1)], "max_ms":samples.last()}))
    }).collect();
    let report = json!({
        "trace":path, "actions":decisions, "scope":"explicit-actions", "skipped_discovery_queries":skipped_discovery, "commands":steps.len(), "runs_seconds":seconds,
        "median_seconds":median, "command_with_observation_latency":latency, "decisions_per_second":decisions as f64/median,
        "cold_setup_seconds":cold_setup, "shared_catalog_setup_seconds":shared_setup,
        "preparation_seconds":preparation_seconds,
        "timing":"One single-threaded game: validated responses, requests, typed viewer snapshots and disclosure delivery. Predecoded actions are copied on submission. Excludes player time, JSON, pipes, compilation, setup and cloning the initial fixture. Includes recorded repeated state requests; excludes privileged audit export. Final typed snapshots agree; use perf_replay.py for full response-hash checks."
    });
    #[cfg(feature = "profile")]
    let report = { let mut report = report; report["internal_profiles"] = json!(profiles); report };
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn typed_prepare_preserves_two_real_triggered_mana_choices() {
        // CR 601.2g-h/605.4a: each enchanted Forest adds G; its controller
        // separately chooses U and R. Hill Giant's 3R spends exactly GGUR once.
        // Establish the Auras by actual casts, as in the vetted MGN witness.
        let faces=corpus::load(&corpus::default_path()).unwrap();
        let catalog=Catalog::new(CardDef::catalog(&faces));
        let mut objects=serde_json::Map::new();
        for (id,name) in [("forest","Forest"),("f2","Forest"),("f3","Forest"),("f4","Forest"),
            ("fertile","Fertile Ground"),("fertile2","Fertile Ground"),("giant","Hill Giant")] {
            objects.insert(id.into(),json!({"name":name,"owner":"A","controller":"A","tapped":false,"counters":{},"damage":0}));
        }
        let setup=json!({"cmd":"new","ask_forced":true,"position":{
            "timing":{"turn":3,"step":"precombat_main","active":"A"},"next":{"kind":"priority","chooser":"A"},
            "players":{"A":{"life":20,"mana":{"G":2}},"B":{"life":20,"mana":{}}},
            "zones":{"A.library":[],"B.library":[],"A.hand":["fertile","fertile2","giant"],"B.hand":[],
                "A.graveyard":[],"B.graveyard":[],"battlefield":["forest","f2","f3","f4"],"stack":[],"exile":[],"command":[]},
            "objects":objects,
            "history":{"turn_structure":"ordinary_A_first","pending":"none","effects":"intrinsic_only","turn_events":"empty",
                "previous_turn_spells":0,"land_plays_this_turn":{"A":0,"B":0},"consecutive_passes":0,"ability_uses":"none",
                "control_since":{"forest":"before_turn","f2":"before_turn","f3":"before_turn","f4":"before_turn"}},
            "causal":{"base_controllers":{},"continuous_effects":[],"linked_exile":[],"prevention":[]}}});
        let (mut host,mut session,mut delivery)=(None,0,Delivery::default());
        dispatch(&catalog,&mut host,&mut session,&mut delivery,&setup).unwrap();
        let mut inputs=vec![setup];
        let mut submit=|host:&mut Option<Simulation>,mv:Value| {
            let request=host.as_mut().unwrap().request().unwrap().unwrap().clone();
            let mut input=json!({"cmd":"act","session_id":1,"request_id":request.id.0,"player":request.player,"move":mv});
            let quote=payment_quote(host.as_ref().unwrap().game().unwrap(),request.player,&input["move"]).unwrap();
            if quote.get("suspended").is_some() || input["move"].get("pending_payment").is_some() { input["payment_expect"]=quote; }
            let result=dispatch(&catalog,host,&mut session,&mut delivery,&input).unwrap();
            inputs.push(input);
            result
        };
        let forests=host.as_ref().unwrap().game().unwrap().battlefield.clone();
        let hand_card=|host:&Option<Simulation>,name:&str| {
            let g=host.as_ref().unwrap().game().unwrap();
            g.players[0].hand.iter().copied().find(|id|g.def(*id).is_some_and(|def|def.name()==name)).unwrap().0
        };
        let aura=hand_card(&host,"Fertile Ground");
        submit(&mut host,json!({"kind":"cast","card":aura,"targets":[[{"object":forests[0].0}]],
            "plan":[{"source":forests[1].0,"ability":0,"mana":{}},{"source":forests[2].0,"ability":0,"mana":{}}],"mana":{"G":2}}));
        submit(&mut host,json!({"kind":"pass"}));submit(&mut host,json!({"kind":"pass"}));
        let aura=hand_card(&host,"Fertile Ground");
        submit(&mut host,json!({"kind":"cast","card":aura,"targets":[[{"object":forests[3].0}]],"mana":{"G":2}}));
        submit(&mut host,json!({"kind":"pass"}));submit(&mut host,json!({"kind":"pass"}));
        let card=hand_card(&host,"Hill Giant");
        submit(&mut host,json!({"kind":"cast","card":card,
            "plan":[{"source":forests[0].0,"ability":0,"mana":{}},{"source":forests[3].0,"ability":0,"mana":{}}]}));
        let middle=submit(&mut host,json!({"kind":"choose","answer":{"mana":["U"]}}));
        assert_eq!(middle["players"][0]["pool"]["G"],2);
        assert_eq!(middle["players"][0]["pool"]["U"],1);
        assert_eq!(middle["asked"]["kind"],"mana");
        let giant=host.as_ref().unwrap().game().unwrap().stack[0].0;
        let final_state=submit(&mut host,json!({"kind":"choose","answer":{"mana":["R"]},
            "pending_payment":{"source":giant,"path":[],"mana":{"G":2,"U":1,"R":1}}}));
        assert_eq!(final_state["players"][0]["pool"]["G"],0);
        assert_eq!(final_state["players"][0]["pool"]["U"],0);
        assert_eq!(final_state["players"][0]["pool"]["R"],0);
        assert_eq!(final_state["events"].as_array().unwrap().iter().filter(|e|e["event"]=="cast").count(),1);
        let cache=std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../.cache/replay-test-inputs");
        std::fs::create_dir_all(&cache).unwrap();
        let path=cache.join(format!("triggered-payment-{}.jsonl",std::process::id()));
        let write=|values:&[Value]|std::fs::write(&path,values.iter().map(|v|format!("{v}\n")).collect::<String>()).unwrap();
        write(&inputs);
        let prepared=prepare(&catalog,path.to_str().unwrap()).unwrap();
        assert_eq!(prepared.decisions,9);
        assert_eq!(prepared.steps.iter().filter(|step|matches!(&step.command,Command::Act {action:Move::AuditPayment(_),..})).count(),2);
        let mut typed=prepared.initial.clone();
        for step in prepared.steps.iter() {execute(&mut typed,&step.command).unwrap();observe(&mut typed,step.viewer).unwrap();}
        assert_eq!(typed.snapshot(None).unwrap(),prepared.expected);
        assert_eq!(typed.game().unwrap().players[0].pool.total(),0);
        assert_eq!(typed.game().unwrap().stack,vec![game::ObjId(giant)]);
        assert_eq!(typed.game().unwrap().audit_game_objects()["objects"][giant.to_string()]["announced"]["cast"]["completed"],true);
        // Completed receipts do not select typed behavior. Their historical
        // audit shape must not become a new precondition on supplied actions.
        inputs[1]["payment_expect"]=json!({"complete":true,"historical_receipt":"unused"});
        write(&inputs);
        let with_unused_receipt=prepare(&catalog,path.to_str().unwrap()).unwrap();
        assert_eq!(with_unused_receipt.expected,prepared.expected);
        let mut forged=inputs;
        forged[7]["payment_expect"]["suspended"]["source"]=json!(999);
        write(&forged);
        let error=prepare(&catalog,path.to_str().unwrap()).err().expect("changed quote must refuse before typed execution");
        assert_eq!(error,"payment quote changed or is incomplete; action was not applied");
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn typed_replay_applies_the_recorded_shuffle_outcome() {
        // CR 701.20: the trusted replay order fixes the shuffle outcome; it must
        // survive preparation rather than being replaced by the engine's RNG.
        let spell = CardDef::new("Shuffle", "{0}", "Instant", "Search your library for a land card, put it onto the battlefield, then shuffle.", None).unwrap();
        let blank = CardDef::new("Land", "", "Land", "", None).unwrap();
        let mut game = Game::new(vec![spell, blank], 2, 20);
        let card = game.put(0, 0, Zone::Hand);
        let cards: Vec<_> = (0..3).map(|_| game.put(0, 1, Zone::Library)).collect();
        let mut sim = Simulation::new(game);
        let submit = |sim: &mut Simulation, action, random| {
            let request = sim.request().unwrap().unwrap().clone();
            execute(sim, &Command::Act { id: request.id, player: request.player, action, random }).unwrap();
        };
        submit(&mut sim, Move::Cast { card, targets: vec![] }, None);
        submit(&mut sim, Move::Pass, None);
        submit(&mut sim, Move::Pass, None);
        let top_first = vec![cards[1], cards[0], cards[2]];
        let random = game::start::RandomOutcome { kind: game::start::RandomKind::Shuffle,
            player: Some(0), order: top_first.clone() };
        submit(&mut sim, Move::Choose(game::state::Answer::Objects(vec![])), Some((vec![random], false)));
        let mut bottom_first = top_first;
        bottom_first.reverse();
        assert_eq!(sim.game().unwrap().players[0].library, bottom_first);
    }
}
