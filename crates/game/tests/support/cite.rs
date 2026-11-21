//! Rule evidence attributed to individual expectations.
//!
//! `cite!(["601.2d", "603.3d"]; assert_eq!(a, b))` runs the assertion unchanged (its own
//! failure message, left/right values included, and fail-fast panic) and, when it completes,
//! records an execution receipt for those rules: the expectation's source location, the
//! enclosing test as libtest names its thread, an optional case label, and the outcome. A
//! failing expectation records `failed` while the panic unwinds, then propagates unchanged.
//!
//! Forms, each optionally after `case: label;` (a table row's label):
//! - `cite!([ids]; assert…!(…))`: an assertion, an expression of type `()`; a bare `bool` or
//!   `Result` is refused at compile time, so an expectation is never credited unchecked.
//! - `cite!([ids]; ok: expr)`: an action that must succeed (`Result::Ok`); yields the value.
//! - `cite!([ids]; refused("reason"): expr)`: an action that must fail (`Result::Err`) with
//!   that reason in its message. The reason is the diagnostic the test intends; it need not
//!   mention the cited rules, which may be an interaction.
//!
//! Rule IDs are complete numbered rules of the pinned rulebook; row-supplied IDs are any
//! string expressions (the static scan reports them as resolved only at run time).
//!
//! Receipts are written only when `RULE_RECEIPTS` names a file, as JSON lines, through one
//! lock for the whole test process; a write failure fails the expectation. With `RULE_RUN`
//! each receipt carries that run's identity. Without `RULE_RECEIPTS` nothing is written and
//! `cite!` is the plain assertion. Success is credited by the reporter only together with
//! the enclosing test's own passing outcome in the same run (`scripts/rules.py`).
use std::io::Write;
use std::sync::Mutex;

static RECEIPTS: Mutex<Option<std::fs::File>> = Mutex::new(None);

/// Record one expectation's outcome, if receipts are being collected.
#[doc(hidden)]
pub fn record(rules: &[String], case: Option<&str>, at: &'static std::panic::Location<'static>, outcome: &str) {
    let Some(path) = std::env::var_os("RULE_RECEIPTS") else { return };
    let thread = std::thread::current();
    let line = serde_json::json!({
        "run": std::env::var("RULE_RUN").ok(),
        "test": thread.name().filter(|n| *n != "main"),
        "file": at.file(), "line": at.line(), "column": at.column(),
        "rules": rules, "case": case, "outcome": outcome,
    });
    // a poisoned lock is a write that panicked: still write, the file is line-oriented
    let mut file = RECEIPTS.lock().unwrap_or_else(|e| e.into_inner());
    let written = match &mut *file {
        Some(f) => Ok(f),
        empty => std::fs::OpenOptions::new().create(true).append(true).open(&path).map(|f| empty.insert(f)),
    }
    .and_then(|f| writeln!(f, "{line}").and_then(|()| f.flush()));
    if let Err(e) = written {
        // The runner treats this marker as an incomplete run. A panic while a failing
        // expectation unwinds would abort the whole test process, so it is only printed then.
        let message = format!("{FAILED_RECEIPTS} {}: {e}", path.to_string_lossy());
        eprintln!("{message}");
        drop(file);
        if !std::thread::panicking() { panic!("{message}"); }
    }
}

/// What a test process prints when a receipt could not be written (see `scripts/rule_evidence.py`).
pub const FAILED_RECEIPTS: &str = "RULE RECEIPTS NOT WRITTEN";

/// Records `failed` if the expectation unwinds before it completes.
#[doc(hidden)]
pub struct Pending<'a> {
    pub rules: &'a [String],
    pub case: Option<&'a str>,
    pub at: &'static std::panic::Location<'static>,
    pub done: bool,
}
impl Drop for Pending<'_> {
    fn drop(&mut self) {
        if !self.done { record(self.rules, self.case, self.at, "failed"); }
    }
}
impl Pending<'_> {
    pub fn passed(mut self) {
        self.done = true;
        record(self.rules, self.case, self.at, "passed");
    }
}

#[doc(hidden)]
#[track_caller]
pub fn here() -> &'static std::panic::Location<'static> { std::panic::Location::caller() }

#[doc(hidden)]
pub fn ids<I: IntoIterator<Item = S>, S: Into<String>>(ids: I) -> Vec<String> {
    let ids: Vec<String> = ids.into_iter().map(Into::into).collect();
    assert!(!ids.is_empty(), "cite! names at least one rule");
    ids
}

/// An error that may be a refusal by the game, and its reason if it is. `Game::act`'s error
/// is taken as one: it has no category, so only its reason distinguishes it (an engine
/// invariant should panic instead, as the copy target metadata check does). Of a
/// [`game::simulation::ResponseError`] only `Illegal` is: a fault, a stale request or another
/// seat's answer never satisfies an expected refusal, whatever its message says; nor does
/// `Passed`, after which the game has gone on.
pub trait Refusal: std::fmt::Debug {
    fn reason(&self) -> Option<&str>;
}
impl Refusal for String {
    fn reason(&self) -> Option<&str> { Some(self) }
}
impl Refusal for game::simulation::ResponseError {
    fn reason(&self) -> Option<&str> {
        match self { Self::Illegal(why) => Some(why), _ => None }
    }
}
/// Of a setup refusal only an illegal one is: a deck the rules do not allow, not a setup the
/// engine does not carry out or setup input that describes no game.
impl Refusal for game::start::Refused {
    fn reason(&self) -> Option<&str> { (self.class == game::simulation::Class::Illegal).then_some(self.why.as_str()) }
}

#[doc(hidden)]
#[track_caller]
pub fn refused<V, E: Refusal>(result: Result<V, E>, reason: &str, rules: &[String]) {
    // an empty reason is in every error: any refusal would pass
    assert!(!reason.trim().is_empty(), "cite! refused(...) names its reason [rules {}]", rules.join(", "));
    match result {
        Err(e) => match e.reason() {
            Some(why) if why.contains(reason) => {}
            Some(why) => panic!("refused, but not for {reason:?} [rules {}]: {why}", rules.join(", ")),
            None => panic!("expected a refusal for {reason:?} [rules {}], not {e:?}", rules.join(", ")),
        },
        Ok(_) => panic!("expected a refusal for {reason:?} [rules {}], it succeeded", rules.join(", ")),
    }
}

#[doc(hidden)]
pub fn accepted<V, E: std::fmt::Display>(result: Result<V, E>, rules: &[String]) -> V {
    result.unwrap_or_else(|e| panic!("expected success [rules {}]: {e}", rules.join(", ")))
}

/// See the module documentation.
macro_rules! cite {
    ([$($id:expr),+ $(,)?]; case: $case:expr; $($rest:tt)+) => {{
        let case: String = $case.to_string();
        $crate::support::cite::cite_inner!([$($id),+]; Some(case.as_str()); $($rest)+)
    }};
    ([$($id:expr),+ $(,)?]; $($rest:tt)+) => {
        $crate::support::cite::cite_inner!([$($id),+]; None; $($rest)+)
    };
}

#[doc(hidden)]
macro_rules! cite_inner {
    ([$($id:expr),+]; $case:expr; ok: $e:expr) => {{
        let rules = $crate::support::cite::ids([$($id),+]);
        let pending = $crate::support::cite::Pending { rules: &rules, case: $case, at: $crate::support::cite::here(), done: false };
        let value = $crate::support::cite::accepted($e, &rules);
        pending.passed();
        value
    }};
    ([$($id:expr),+]; $case:expr; refused($reason:expr): $e:expr) => {{
        let rules = $crate::support::cite::ids([$($id),+]);
        let pending = $crate::support::cite::Pending { rules: &rules, case: $case, at: $crate::support::cite::here(), done: false };
        $crate::support::cite::refused($e, $reason, &rules);
        pending.passed();
    }};
    ([$($id:expr),+]; $case:expr; $assertion:expr) => {{
        let rules = $crate::support::cite::ids([$($id),+]);
        let pending = $crate::support::cite::Pending { rules: &rules, case: $case, at: $crate::support::cite::here(), done: false };
        // an assertion is `()`: a bare comparison or an unchecked `Result` does not compile
        let () = $assertion;
        pending.passed();
    }};
}

pub(crate) use cite;
pub(crate) use cite_inner;
