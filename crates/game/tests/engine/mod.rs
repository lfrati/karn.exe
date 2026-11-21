//! Engine tests that no one rule section owns: admission refusals, deck and whole-card
//! compositions, compiled programs, events, selectors and simulation.
//! Part of the shared rules test executable. Every `.rs` file here is a module by its file
//! name, found by `crates/test-modules`: a new file needs no `mod` line.

test_modules::engine!();
