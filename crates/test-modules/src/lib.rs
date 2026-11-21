//! The modules of the game crate's `rules` test executable: `scenarios!()` and `engine!()`
//! declare `#[path = "…"] mod name;` for each `name.rs` of `crates/game/tests/scenarios/` and
//! `crates/game/tests/engine/` (but `mod.rs`), so a new test file needs no `mod` line.
//!
//! A crate of its own, a dev-dependency of `game`, rather than `game`'s build script: a build
//! script that reran on every test edit would recompile the `game` library with it.

#![forbid(unsafe_code)]

include!(concat!(env!("OUT_DIR"), "/modules.rs"));
