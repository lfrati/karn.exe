//! Public predefined-token initialization must work without a preceding card load.
use game::CardDef;
use ir::token::Predefined;
use std::process::{Command, Stdio};
use std::sync::{Arc, Barrier};
use std::time::{Duration, Instant};

fn clue() -> &'static CardDef {
    let def = game::token::definition(Predefined::Clue).expect("CR 111.10f Clue is supported");
    assert!(def.is(ir::CardType::Artifact));
    assert_eq!(def.mana_value(), 0);
    assert_eq!(def.pt(), None);
    assert_eq!(def.abilities().len(), 1);
    game::play::check(def).unwrap();
    def
}

#[test]
fn r111_10f_cold_warm_compilation_and_concurrent_initialization() {
    const CASE: &str = "KARN_TOKEN_INITIALIZATION_CASE";
    if let Ok(case) = std::env::var(CASE) {
        match case.as_str() {
            "direct" => {
                let first = clue();
                assert!(std::ptr::eq(first, clue()));
                let refused = game::token::definition(Predefined::Gold).expect_err("Gold remains unsupported");
                assert_eq!(refused.0, "token: Gold (111.10): a predefined token the engine does not create yet");
                assert!(std::ptr::eq(first, clue()));
            }
            "compile" => {
                let maker = CardDef::new("Clue maker", "{0}", "Sorcery", "Create a Clue token.", None).unwrap();
                game::play::check(&maker).unwrap();
                clue();
            }
            "concurrent" => {
                let start = Arc::new(Barrier::new(8));
                let threads: Vec<_> = (0..8).map(|n| {
                    let start = start.clone();
                    std::thread::spawn(move || {
                        start.wait();
                        if n % 2 == 0 {
                            let def = CardDef::new("Clue maker", "{0}", "Sorcery", "Create a Clue token.", None).unwrap();
                            game::play::check(&def).unwrap();
                        }
                        clue() as *const CardDef as usize
                    })
                }).collect();
                let pointers: Vec<_> = threads.into_iter().map(|t| t.join().unwrap()).collect();
                assert!(pointers.iter().all(|p| *p == pointers[0]));
            }
            _ => panic!("unknown initialization case"),
        }
        return;
    }
    for case in ["direct", "compile", "concurrent"] {
        let mut child = Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "engine::token_initialization::r111_10f_cold_warm_compilation_and_concurrent_initialization", "--nocapture"])
            .env(CASE, case).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().unwrap();
        let deadline = Instant::now() + Duration::from_secs(15);
        loop {
            if child.try_wait().unwrap().is_some() { break; }
            if Instant::now() >= deadline {
                child.kill().unwrap();
                let output = child.wait_with_output().unwrap();
                panic!("{case} initialization timed out: {}{}", String::from_utf8_lossy(&output.stdout), String::from_utf8_lossy(&output.stderr));
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        let output = child.wait_with_output().unwrap();
        assert!(output.status.success(), "{case}: {}{}", String::from_utf8_lossy(&output.stdout), String::from_utf8_lossy(&output.stderr));
        assert!(String::from_utf8_lossy(&output.stdout).contains("test result: ok. 1 passed;"),
            "{case}: subprocess did not execute its one named test: {}", String::from_utf8_lossy(&output.stdout));
    }
}
