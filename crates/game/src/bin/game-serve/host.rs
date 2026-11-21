//! Opt-in shared host. The input stream orders each game's FIFO; workers own games,
//! delivery and caches exclusively. Only immutable catalogs and output are shared.
use super::*;
use std::sync::{atomic::{AtomicU64, Ordering}, mpsc};

#[derive(Default)]
struct Session {
    game: Option<Simulation>,
    id: u64,
    delivery: Delivery,
}

fn answer(session: &Session, result: Result<Value, Error>) -> Value {
    match result {
        Ok(state) => json!({"ok":true,"state":state}),
        Err(error) => match session.game.as_ref().and_then(Simulation::fault) {
            Some(fault) => faulted(session.id, fault),
            None => error.said(),
        }
    }
}

fn handle(catalog: &Catalog, sessions: &mut BTreeMap<u64, Session>, serial: &AtomicU64, input: &Value) -> Value {
    let id = input["id"].as_u64();
    let key = input["game"].as_u64();
    let result = (|| -> Result<Value, Error> {
        let _id = id.ok_or("envelope needs numeric id")?;
        let key = key.ok_or("envelope needs numeric game")?;
        let request = &input["request"];
        let cmd = request["cmd"].as_str().ok_or("envelope needs request command")?;
        if cmd == "close" {
            let session = sessions.get(&key).ok_or_else(|| Error::unreferable("no game"))?;
            if request["session_id"].as_u64() != Some(session.id) { return Err(Error::stale("stale session_id")); }
            sessions.remove(&key);
            return Ok(json!({"ok":true,"state":null}));
        }
        if cmd == "new" || cmd == "start" {
            // Allocation precedes setup, but failed setup never publishes a new identity.
            let fresh = serial.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |n|n.checked_add(1)).map_err(|_| Error::unsupported("session ID exhausted"))?;
            let mut prepared = Session { id: fresh, ..Session::default() };
            let result = dispatch(catalog, &mut prepared.game, &mut prepared.id, &mut prepared.delivery, request);
            let succeeded = result.is_ok();
            let response = answer(&prepared, result);
            if succeeded { sessions.insert(key, prepared); }
            return Ok(response);
        }
        let session = sessions.get_mut(&key).ok_or_else(|| Error::unreferable("no game"))?;
        let result = dispatch(catalog, &mut session.game, &mut session.id, &mut session.delivery, request);
        Ok(answer(session, result))
    })();
    let mut result = result.unwrap_or_else(|error| error.said());
    result["id"] = json!(id); result["game"] = json!(key);
    result
}

pub(super) fn run(catalog: &Catalog, workers: usize) {
    let serial = AtomicU64::new(0);
    std::thread::scope(|scope| {
        let (output, receive) = mpsc::sync_channel::<Value>(256);
        scope.spawn(move || {
            let mut out = std::io::stdout().lock();
            writeln!(out,"{}",json!({"type":"ready","host":true,"workers":workers})).unwrap();
            out.flush().unwrap();
            while let Ok(first) = receive.recv() {
                writeln!(out,"{first}").unwrap();
                // Drain ready replies before flushing; never delay a lone reply.
                for reply in receive.try_iter() { writeln!(out,"{reply}").unwrap(); }
                if out.flush().is_err() { break; }
            }
        });
        let mut senders = Vec::new();
        for _ in 0..workers {
            let (send, receive) = mpsc::sync_channel::<Value>(256);
            senders.push(send);
            let output = output.clone(); let serial = &serial;
            scope.spawn(move || {
                let mut sessions = BTreeMap::new();
                while let Ok(input) = receive.recv() {
                    if output.send(handle(catalog, &mut sessions, serial, &input)).is_err() { break; }
                }
            });
        }
        for line in std::io::stdin().lock().lines() {
            let Ok(line) = line else { break; };
            if line.trim().is_empty() { continue; }
            match serde_json::from_str::<Value>(&line) {
                Ok(input) => {
                    let worker = input["game"].as_u64().unwrap_or(0) as usize % workers;
                    if senders[worker].send(input).is_err() { break; }
                }
                Err(error) => { if output.send(json!({"id":null,"game":null,"ok":false,"error":format!("not JSON: {error}"),"class":Class::Malformed.name()})).is_err() { break; } }
            }
        }
        drop(senders); drop(output);
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn identity_close_and_mutations_are_game_local() {
        let catalog = Catalog::new(BTreeMap::new()); let mut sessions = BTreeMap::new(); let serial = AtomicU64::new(0);
        let send = |sessions: &mut BTreeMap<u64, Session>, game, request| handle(&catalog,sessions,&serial,&json!({"id":9,"game":game,"request":request}));
        let a=send(&mut sessions,0,json!({"cmd":"new","players":[{},{}]}));
        let b=send(&mut sessions,1,json!({"cmd":"new","players":[{},{}]}));
        assert!(a["ok"]==true && b["ok"]==true); assert_ne!(a["state"]["session_id"],b["state"]["session_id"]);
        let pass=json!({"cmd":"act","session_id":a["state"]["session_id"],"request_id":a["state"]["request_id"],"player":a["state"]["asked"]["player"],"move":{"kind":"pass"}});
        assert_eq!(send(&mut sessions,1,pass.clone())["error"],"stale session_id");
        assert_eq!(send(&mut sessions,0,pass.clone())["ok"],true);
        assert_eq!(send(&mut sessions,0,pass)["ok"],false);
        assert_eq!(send(&mut sessions,0,json!({"cmd":"close","session_id":a["state"]["session_id"]}))["ok"],true);
        assert_eq!(send(&mut sessions,0,json!({"cmd":"state"}))["error"],"no game");
        let mut expected=b["state"].clone(); expected["events"]=json!([]); expected["log"]=json!([]);
        assert_eq!(send(&mut sessions,1,json!({"cmd":"state"}))["state"],expected);
    }
    #[test]
    fn failed_initial_response_preserves_the_entire_session() {
        let catalog = Catalog::new(BTreeMap::new());
        let mut sessions = BTreeMap::new();
        let serial = AtomicU64::new(0);
        let send = |sessions: &mut BTreeMap<u64, Session>, request| {
            handle(&catalog, sessions, &serial, &json!({"id":9, "game":7, "request":request}))
        };
        let setup = json!({"cmd":"new", "players":[{}, {}]});
        for invalid in [json!({"cmd":"new", "players":[{}, {}, {}], "viewer":999}),
                        json!({"cmd":"start", "decks":[[], [], []], "viewer":999})] {
            assert_eq!(send(&mut sessions, invalid.clone())["error"], "a viewer is a player or \"asked\", not 999");
            assert!(!sessions.contains_key(&7));
            let first = send(&mut sessions, setup.clone())["state"].clone();
            let before = format!("{:?}", sessions[&7].game.as_ref().unwrap());
            let delivery = sessions[&7].delivery.clone();
            assert_eq!(send(&mut sessions, invalid.clone())["error"], "a viewer is a player or \"asked\", not 999");
            assert_eq!(sessions[&7].id, first["session_id"].as_u64().unwrap());
            assert_eq!(sessions[&7].delivery, delivery);
            assert_eq!(format!("{:?}", sessions[&7].game.as_ref().unwrap()), before);
            let pass = json!({"cmd":"act", "session_id":first["session_id"], "request_id":first["request_id"],
                "player":first["asked"]["player"], "move":{"kind":"pass"}});
            assert_eq!(send(&mut sessions, pass.clone())["ok"], true);

            let fault = sessions.get_mut(&7).unwrap().game.as_mut().unwrap()
                .observe::<()>(|_| panic!("previous session private diagnostic")).unwrap_err();
            assert_eq!(send(&mut sessions, invalid)["error"], "a viewer is a player or \"asked\", not 999");
            assert_eq!(sessions[&7].game.as_ref().unwrap().fault(), Some(&fault));
            let replacement = send(&mut sessions, setup.clone());
            assert_eq!(replacement["ok"], true);
            assert_ne!(replacement["state"]["session_id"], first["session_id"]);
            assert_eq!(send(&mut sessions, pass)["error"], "stale session_id");
            sessions.remove(&7);
        }
    }
    /// Through the shared host: a request with a viewer it names no one by is refused
    /// with its class before the game, its delivery or its session change, and can be made again.
    #[test]
    fn refusals_say_their_class_and_an_invalid_viewer_is_refused_before_acting() {
        let catalog = Catalog::new(BTreeMap::new()); let mut sessions = BTreeMap::new(); let serial = AtomicU64::new(0);
        let send = |sessions: &mut BTreeMap<u64, Session>, request: Value| handle(&catalog,sessions,&serial,&json!({"id":4,"game":3,"request":request}));
        let refused = |reply: &Value| (reply["ok"].clone(), reply["class"].clone());
        assert_eq!(refused(&send(&mut sessions, json!({"cmd":"state"}))), (json!(false), json!("unreferable")));
        assert_eq!(refused(&handle(&catalog,&mut sessions,&serial,&json!({"game":3,"request":{"cmd":"state"}}))), (json!(false), json!("malformed")));
        let first = send(&mut sessions, json!({"cmd":"new","players":[{},{}]}))["state"].clone();
        let pass = json!({"cmd":"act","session_id":first["session_id"],"request_id":first["request_id"],
            "player":first["asked"]["player"],"move":{"kind":"pass"}});
        let before = (format!("{:?}", sessions[&3].game.as_ref().unwrap()), sessions[&3].delivery.clone(), sessions[&3].id);
        for (viewer, class) in [(json!(999), "unreferable"), (json!("nobody"), "malformed")] {
            let mut invalid = pass.clone(); invalid["viewer"] = viewer.clone();
            let reply = send(&mut sessions, invalid);
            assert_eq!(reply["error"], format!("a viewer is a player or \"asked\", not {viewer}"));
            assert_eq!((reply["class"].clone(), reply["id"].clone(), reply["game"].clone()), (json!(class), json!(4), json!(3)));
            assert_eq!((format!("{:?}", sessions[&3].game.as_ref().unwrap()), sessions[&3].delivery.clone(), sessions[&3].id), before);
        }
        let mut shown = pass.clone(); shown["viewer"] = json!("asked");
        let next = send(&mut sessions, shown);
        assert_eq!(next["ok"], true, "{next}");
        assert_ne!(next["state"]["request_id"], first["request_id"]);
        assert_eq!(refused(&send(&mut sessions, pass)), (json!(false), json!("stale")));
        let close = json!({"cmd":"close","session_id":first["session_id"].as_u64().unwrap() + 1});
        assert_eq!(refused(&send(&mut sessions, close)), (json!(false), json!("stale")));
    }

    #[test]
    fn shared_definitions_and_faults_preserve_other_sessions() {
        let faces=corpus::load(&corpus::default_path()).unwrap(); let catalog=Catalog::new(CardDef::catalog(&faces));
        let setup=json!({"cmd":"new","players":[{"hand":["Forest"]},{}]});
        let mut sessions=BTreeMap::new(); let serial=AtomicU64::new(0);
        for key in 0..2 { assert_eq!(handle(&catalog,&mut sessions,&serial,&json!({"id":key,"game":key,"request":setup}))["ok"],true); }
        let a=sessions[&0].game.as_ref().unwrap().game().unwrap();
        let b=sessions[&1].game.as_ref().unwrap().game().unwrap();
        assert!(std::sync::Arc::ptr_eq(&a.defs,&b.defs));
        let other_before = format!("{b:?}");
        let turn = a.turn;
        let fault = sessions.get_mut(&0).unwrap().game.as_mut().unwrap()
            .observe::<()>(|_| panic!("private diagnostic")).unwrap_err();
        assert_eq!(fault.message, "private diagnostic");
        assert_eq!(fault.stage, game::simulation::Stage::Observation);
        assert_eq!(fault.request_id, game::simulation::RequestId(1));
        assert_eq!((fault.player, fault.turn), (None, turn));
        assert_eq!(sessions[&0].game.as_ref().unwrap().game().unwrap_err(), fault);
        let failed=handle(&catalog,&mut sessions,&serial,&json!({"id":2,"game":0,"request":{"cmd":"state"}}));
        assert_eq!(failed["error"],"simulation fault"); assert!(!failed.to_string().contains("private diagnostic"));
        assert_eq!(failed["fault"], json!({"session_id":sessions[&0].id,"request_id":1,"stage":"Observation"}));
        assert_eq!(handle(&catalog,&mut sessions,&serial,&json!({"id":3,"game":1,"request":{"cmd":"state"}}))["ok"],true);
        assert_eq!(format!("{:?}", sessions[&1].game.as_ref().unwrap().game().unwrap()), other_before);
    }

}
