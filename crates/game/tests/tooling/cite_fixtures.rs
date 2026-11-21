//! `cite!` (support/cite.rs) under conditions a rules test never creates.
#![allow(unexpected_cfgs)] // `cite_compile_fail` is set only by the compile-fail tool test
use crate::support::*;

/// With `CITE_FIXTURE=receipt_failure` and receipts that can't be written: the cited
/// assertion fails, its receipt can't be recorded, and the test still fails normally (not an
/// abort), with the failure to record visible. Otherwise it does nothing.
#[test]
fn receipt_failure() {
    if std::env::var("CITE_FIXTURE").as_deref() != Ok("receipt_failure") { return; }
    cite!(["100.1"]; assert_eq!(1, 2, "a deliberate failure"));
}

/// With `CITE_FIXTURE=receipt_unwritable_pass`: a cited assertion that passes, its receipt
/// unwritable; the run must not count as complete.
#[test]
fn receipt_unwritable_pass() {
    if std::env::var("CITE_FIXTURE").as_deref() != Ok("receipt_unwritable_pass") { return; }
    cite!(["100.1"]; assert_eq!(1, 1));
}

/// A cited refusal is not satisfied by a simulation's fault, even one whose message has the
/// reason's words (engine/test_support_contract.rs proves this of the uncited form). It always
/// runs: the expectation fails on purpose, so its receipt here, ignored as a fixture's, credits
/// and fails no rule.
#[test]
fn cited_refusal_rejects_a_fault() {

    use game::simulation::{ResponseError, Simulation};
    use std::panic::{catch_unwind, AssertUnwindSafe};
    let reason = "no decision is pending";
    let mut sim = Simulation::new(Game::new(vec![], 2, 20));
    let r = sim.request().unwrap().unwrap().clone();
    let _ = sim.observe(|_| -> () { panic!("{reason}") });
    let fault = sim.respond(r.id, r.player, Move::Choose(Answer::Yes));
    assert!(matches!(&fault, Err(ResponseError::Fault(f)) if f.message.contains(reason)), "{fault:?}");
    let payload=catch_unwind(AssertUnwindSafe(|| cite!(["117.1"]; refused(reason): fault))).expect_err("fault must not earn a refusal receipt");
    let message=payload.downcast_ref::<String>().map(String::as_str).or_else(||payload.downcast_ref::<&str>().copied()).expect("text assertion payload");
    assert!(message.contains("expected a refusal") && message.contains("not Fault"),"{message}");
}

/// Compiled only with `--cfg cite_compile_fail`: neither a bare condition nor a bare result
/// is an assertion, so neither may be cited (each must be refused as not `()`).
#[cfg(cite_compile_fail)]
#[allow(unused)]
fn not_assertions() {
    let condition = 1 == 2;
    cite!(["100.1"]; condition);
    let result: Result<(), String> = Err("refused".into());
    cite!(["100.1"]; result);
}
