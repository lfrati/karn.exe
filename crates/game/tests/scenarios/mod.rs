//! Rules scenarios: one file per behaviour, in one flat directory. Every `.rs` file here is a
//! module by its file name, found by `crates/test-modules`: a new file needs no `mod` line.

test_modules::scenarios!();
