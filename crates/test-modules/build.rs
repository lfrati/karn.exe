//! Lists the test files of `crates/game/tests/scenarios/` and `crates/game/tests/engine/` as
//! the exported macros `scenarios!` and `engine!`, in `$OUT_DIR/modules.rs`.
use std::path::{Path, PathBuf};

fn main() {
    let manifest = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("cargo sets CARGO_MANIFEST_DIR"));
    let tests = manifest.join("../game/tests").canonicalize().expect("crates/game/tests");
    let mut out = String::new();
    for folder in ["scenarios", "engine"] {
        let dir = tests.join(folder);
        // a directory is rescanned when any file in it changes, a new one included
        println!("cargo:rerun-if-changed={}", dir.display());
        out.push_str(&format!("/// A module for each test file of `tests/{folder}/`.\n#[macro_export]\nmacro_rules! {folder} {{\n    () => {{\n"));
        for (name, path) in files(&dir) {
            out.push_str(&format!("        #[path = {path:?}]\n        mod {name};\n"));
        }
        out.push_str("    };\n}\n");
    }
    let target = PathBuf::from(std::env::var("OUT_DIR").expect("cargo sets OUT_DIR")).join("modules.rs");
    std::fs::write(target, out).expect("write the test module list");
}

/// Each `name.rs` of `dir` but its `mod.rs`, by name, with its absolute path.
fn files(dir: &Path) -> Vec<(String, String)> {
    let mut files: Vec<(String, String)> = std::fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", dir.display()))
        .map(|entry| entry.expect("a directory entry").path())
        .filter(|path| path.extension().is_some_and(|x| x == "rs") && path.file_stem().is_some_and(|s| s != "mod"))
        .map(|path| {
            let name = path.file_stem().and_then(|s| s.to_str()).expect("a UTF-8 file name").to_string();
            assert!(
                name.starts_with(|c: char| c.is_ascii_alphabetic() || c == '_') && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_'),
                "{}: a test file's name must be a Rust module name", path.display()
            );
            (name, path.to_str().expect("a UTF-8 path").to_string())
        })
        .collect();
    files.sort();
    files
}
